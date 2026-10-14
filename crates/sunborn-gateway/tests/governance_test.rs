use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_cache::CacheEntry;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_core::AppConfig;
use sunborn_db::models::{DurableCacheRecord, TenantRecord};
use sunborn_db::DatabaseStore;
use sunborn_execution::{ExecutionCacheStore, ToolResult};
use sunborn_gateway::{create_router, AppState};
use sunborn_semantic::{VectorEntry, VectorStore};
use tower::ServiceExt;

#[tokio::test]
async fn test_tenant_cascade_offboarding_zero_lingering_data() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let target_tenant = "tenant-gdpr-offboard";
    let tenant_id = TenantId::new(target_tenant);
    let app_id = ApplicationId::new("app-test");

    // 1. Populate data across ALL 4 system tiers for this tenant:
    // a) PostgreSQL Tenant & Durable Cache Record
    state
        .db
        .insert_tenant(TenantRecord {
            id: target_tenant.into(),
            name: "Offboarding Enterprise Corp".into(),
            status: "active".into(),
            rate_limit_rps: 100,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    state
        .db
        .insert_cache_entry(DurableCacheRecord {
            key: format!("tenant:{}:key1", target_tenant),
            tenant_id: target_tenant.into(),
            application_id: "app-test".into(),
            model: "gpt-4o".into(),
            policy_version: "v1".into(),
            hash: "hash-123".into(),
            prompt_canonical: "test prompt".into(),
            content: "sensitive content".into(),
            hit_count: 5,
            created_at: chrono::Utc::now(),
            expires_at: None,
            last_accessed_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // b) Exact Cache Tier
    let exact_key = format!("tenant:{}:key1", target_tenant);
    state
        .cache
        .set(CacheEntry::new(
            &exact_key,
            "cached sensitive content",
            Some(3600),
        ))
        .await
        .unwrap();

    // c) Semantic Vector Store Tier
    state
        .vector_store
        .insert(VectorEntry {
            id: exact_key.clone(),
            tenant_id: tenant_id.clone(),
            application_id: app_id.clone(),
            prompt: "test prompt".into(),
            content: "cached sensitive content".into(),
            vector: vec![0.1; 384],
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // d) Agent DAG Execution Cache Tier
    let sig = "sig-offboard-tool";
    state
        .execution_store
        .set(
            &tenant_id,
            sig,
            &ToolResult {
                tool_name: "calculator".into(),
                signature: sig.into(),
                output: serde_json::json!({"secret": "data"}),
                is_error: false,
                execution_latency_ms: 10,
                created_at: chrono::Utc::now(),
            },
            Some(3600),
        )
        .await
        .unwrap();

    // Verify all tiers contain data before offboarding
    assert!(state.db.get_tenant(target_tenant).await.unwrap().is_some());
    assert_eq!(state.vector_store.count(&tenant_id).await.unwrap(), 1);
    assert!(state.cache.exists(&exact_key).await.unwrap());
    assert!(state
        .execution_store
        .get(&tenant_id, sig)
        .await
        .unwrap()
        .is_some());

    // 2. EXECUTE GDPR ARTICLE 17 CASCADE OFFBOARDING
    let delete_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/tenants/{}", target_tenant))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_res.status(), StatusCode::OK);
    let body = delete_res.into_body().collect().await.unwrap().to_bytes();
    let resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(resp["status"], "erased");
    assert_eq!(resp["deleted_db_entries"], 1);

    // 3. CRITICAL VERIFICATION: Zero lingering records across all tiers
    assert!(
        state.db.get_tenant(target_tenant).await.unwrap().is_none(),
        "Tenant record must be completely deleted from DB"
    );
    assert!(
        !state.cache.exists(&exact_key).await.unwrap(),
        "Exact cache entry must be eradicated"
    );
    assert_eq!(
        state.vector_store.count(&tenant_id).await.unwrap(),
        0,
        "Semantic vector entries must be zero"
    );
    assert!(
        state
            .execution_store
            .get(&tenant_id, sig)
            .await
            .unwrap()
            .is_none(),
        "Agent tool execution records must be purged"
    );
}

#[tokio::test]
async fn test_lifecycle_sweep_expired_entries() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    // Seed expired cache entry (TTL 0s)
    let expired_entry = CacheEntry::new("expired:key:1", "old content", Some(0));
    // Seed non-expired cache entry
    let active_entry = CacheEntry::new("active:key:1", "fresh content", Some(3600));

    state.cache.set(expired_entry).await.unwrap();
    state.cache.set(active_entry).await.unwrap();

    // Trigger lifecycle sweep endpoint
    let sweep_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/lifecycle/sweep")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(sweep_res.status(), StatusCode::OK);
    let body = sweep_res.into_body().collect().await.unwrap().to_bytes();
    let resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(resp["status"], "completed");
    assert!(resp["swept_entries"].as_u64().unwrap() >= 1);

    assert!(!state.cache.exists("expired:key:1").await.unwrap());
    assert!(state.cache.exists("active:key:1").await.unwrap());
}
