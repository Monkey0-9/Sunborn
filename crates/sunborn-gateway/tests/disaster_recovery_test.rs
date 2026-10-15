use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_cache::ExactCacheStore;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_db::models::TenantRecord;
use sunborn_gateway::{create_router, AppState};
use sunborn_semantic::VectorStore;
use tower::ServiceExt;

#[tokio::test]
async fn test_disaster_recovery_replay_and_index_reconstruction() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    // 0. Register key for test tenant
    state.api_key_registry.register_key(
        "test-key-dr",
        sunborn_core::TenantId::new("tenant-dr"),
        sunborn_core::ApplicationId::new("app-rebuild"),
        vec!["*".to_string()],
        true,
    );

    // 1. Onboard tenant
    state
        .db
        .insert_tenant(TenantRecord {
            id: "tenant-dr".into(),
            name: "Disaster Recovery Testing Corp".into(),
            status: "active".into(),
            rate_limit_rps: 100,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // 2. Perform initial request -> Origin miss -> Admitted to both memory cache, vector store, and authoritative PostgreSQL
    let req_payload = RequestEnvelope::new(
        "tenant-dr",
        "app-rebuild",
        "gpt-4o",
        "Explain non-accelerator authority in Sunborn",
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-dr")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);

    // Verify durable record exists in PostgreSQL store
    let durable_count = state.db.count_cache_entries().await.unwrap();
    assert_eq!(
        durable_count, 1,
        "PostgreSQL must store durable cache entry"
    );

    // 3. DISASTER EVENT: Simulate total catastrophic failure of accelerator tiers
    // Both memory exact cache and vector store are wiped clean
    let _ = state.memory_cache.delete_by_prefix("").await;
    let _ = state
        .vector_store
        .delete_by_tenant(&sunborn_core::TenantId::new("tenant-dr"))
        .await;

    // Verify cache is wiped (misses if queried)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/some_nonexistent_key")
                .header("x-api-key", "test-key-dr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 4. TRIGGER RECONSTRUCTION: Run the rebuild endpoint
    let rebuild_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/rebuild")
                .header("x-api-key", "test-key-dr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rebuild_res.status(), StatusCode::OK);
    let rebuild_body = rebuild_res.into_body().collect().await.unwrap().to_bytes();
    let rebuild_json: Value = serde_json::from_slice(&rebuild_body).unwrap();
    assert_eq!(rebuild_json["status"], "completed");
    assert_eq!(rebuild_json["rebuilt_exact_cache"], 1);
    assert_eq!(rebuild_json["rebuilt_vectors"], 1);

    // 5. POST-RECOVERY VERIFICATION:
    // Execute identical request -> Must hit reconstructed exact cache with 0 origin execution
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-dr")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(
        resp.cached,
        "Post-rebuild request must hit reconstructed cache"
    );
    assert_eq!(resp.decision.action, sunborn_core::DecisionAction::SafeHit);
    assert_eq!(resp.decision.cache_path, sunborn_core::CachePath::Exact);
}
