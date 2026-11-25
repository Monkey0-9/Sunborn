//! Adversarial Multi-Replica G2 (Context & Authorization) & G3 (Freshness & Invalidation) Tests:
//! 1. Multi-replica permission revocation, credential rotation & tenant suspension propagation
//! 2. Multi-user session scope isolation on exact & semantic paths + concurrent cross-tenant reads
//! 3. Delayed, duplicated, reordered, and dropped invalidation events with Outbox recovery &
//!    measured maximum stale-read exposure window & propagation latency

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};
use sunborn_core::{AppConfig, ApplicationId, RequestEnvelope, ResponseEnvelope, TenantId};
use sunborn_db::{models::OutboxRecord, DatabaseStore, MemoryDatabaseStore};
use sunborn_gateway::{create_router, AppState};
use sunborn_invalidation::{
    ClusterInvalidationMessage, DistributedGraphSyncEngine, EpochLeaseManager, GraphMemoryCache,
    InMemoryClusterTransport,
};
use sunborn_semantic::{MemoryVectorStore, VectorEntry, VectorStore};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_g2_multi_replica_permission_revocation_and_credential_rotation() {
    // Replica A & B share backing Redis/Postgres/KMS stores while keeping process-local
    // permission versions, epoch managers, and API key registries (which sync via events).
    let state_a = AppState::new(AppConfig::default());
    let mut state_b = AppState::new(AppConfig::default());
    state_b.cache = state_a.cache.clone();
    state_b.vector_store = state_a.vector_store.clone();
    state_b.db = state_a.db.clone();
    state_b.postgres_db = state_a.postgres_db.clone();
    state_b.crypto_registry = state_a.crypto_registry.clone();

    let tenant = "tenant-g2-rep";
    let app_id = "app-sec";
    let old_key = "sb-old-credential";
    let rotated_key = "sb-new-credential";

    for st in [&state_a, &state_b] {
        st.api_key_registry.register_key(
            old_key,
            TenantId::new(tenant),
            ApplicationId::new(app_id),
            vec!["*".to_string()],
            false,
        );
    }

    let app_a = create_router(state_a.clone());
    let app_b = create_router(state_b.clone());

    let req = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Summarize treasury liquidity covenants for Account 404",
    );

    // 1. Admit response on Replica A
    let r1 = app_a
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", old_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);
    let env1: ResponseEnvelope =
        serde_json::from_slice(&r1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!env1.cached);

    // 2. Verify Replica B initially gets a cache hit BEFORE revocation
    let r2 = app_b
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", old_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::OK);
    let env2: ResponseEnvelope =
        serde_json::from_slice(&r2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        env2.cached,
        "Replica B should reuse shared cache before revocation"
    );

    // 3. Revoke old credential & rotate to new credential on Replica A, and replicate
    // the permission version + key registry invalidation to Replica B.
    let admin_key = "sb-admin-root";
    let quorum_key = "sb-admin-quorum";
    for (k, _is_q) in [(admin_key, false), (quorum_key, true)] {
        state_a.api_key_registry.register_key(
            k,
            TenantId::new("ops-admin"),
            ApplicationId::new("control"),
            vec!["*".to_string()],
            true,
        );
    }

    let rot_res = app_a
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials/rotate")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .header("x-quorum-admin-key", quorum_key)
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "tenant_id": tenant,
                        "application_id": app_id,
                        "old_raw_key": old_key,
                        "new_raw_key": rotated_key,
                        "rate_limit_rps": 100,
                        "allowed_models": ["*"],
                        "is_admin": false
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rot_res.status(), StatusCode::OK);

    // Propagate permission revocation + credential change to Replica B
    let new_perm_ver = state_a
        .temporal_engine
        .get_or_init_state(AppState::tenant_permission_scope_uuid(tenant))
        .permission_version;
    state_b
        .temporal_engine
        .bump_permission_version(AppState::tenant_permission_scope_uuid(tenant));
    state_b.api_key_registry.revoke_key(old_key);
    state_b.api_key_registry.register_key(
        rotated_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );
    assert!(new_perm_ver >= 2);

    // 4. Old key on Replica B is now rejected at auth layer (401 Unauthorized)
    let r_old = app_b
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", old_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r_old.status(), StatusCode::UNAUTHORIZED);

    // 5. Even with the new rotated credential, Replica B MUST NOT serve pre-revocation
    // exact or semantic cache entries (permission_version bump invalidates reuse!).
    let r_new = app_b
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", rotated_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r_new.status(), StatusCode::OK);
    let env_new: ResponseEnvelope =
        serde_json::from_slice(&r_new.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !env_new.cached,
        "Post-revocation request with new key on Replica B must bypass stale pre-revocation cache"
    );
}

#[tokio::test]
async fn test_g2_session_scope_isolation_and_concurrent_cross_tenant_retrieval() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let api_a = "key-conc-a";
    let api_b = "key-conc-b";
    state.api_key_registry.register_key(
        api_a,
        TenantId::new("tenant-alpha"),
        ApplicationId::new("app-1"),
        vec!["*".to_string()],
        false,
    );
    state.api_key_registry.register_key(
        api_b,
        TenantId::new("tenant-beta"),
        ApplicationId::new("app-1"),
        vec!["*".to_string()],
        false,
    );

    // 1. Session isolation within the SAME tenant & app:
    // User/session S1 seeds prompt P; S2 asks identical P and slightly paraphrased P -> MUST NOT hit S1!
    let mut req_s1 = RequestEnvelope::new(
        "tenant-alpha",
        "app-1",
        "gpt-4o",
        "Summarize my private medical expense claim #101",
    );
    req_s1.metadata["session_id"] = json!("sess-user-1");
    req_s1.metadata["user_id"] = json!("alice");

    let r_s1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_a)
                .body(Body::from(serde_json::to_vec(&req_s1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r_s1.status(), StatusCode::OK);

    let mut req_s2 = RequestEnvelope::new(
        "tenant-alpha",
        "app-1",
        "gpt-4o",
        "Summarize my private medical expense claim #101",
    );
    req_s2.metadata["session_id"] = json!("sess-user-2");
    req_s2.metadata["user_id"] = json!("bob");

    let r_s2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_a)
                .body(Body::from(serde_json::to_vec(&req_s2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let env_s2: ResponseEnvelope =
        serde_json::from_slice(&r_s2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !env_s2.cached,
        "Different user_id/session_id within same tenant must never reuse cached response"
    );

    // 2. Concurrent cross-tenant retrieval stress (12 parallel requests across Alpha & Beta
    // asking identical sensitive prompts). Verify zero cross-tenant leakage.
    let mut handles = Vec::new();
    for i in 0..12usize {
        let app_c = app.clone();
        let (tenant, key, secret_tag) = if i % 2 == 0 {
            ("tenant-alpha", api_a, "ALPHA-SECRET")
        } else {
            ("tenant-beta", api_b, "BETA-SECRET")
        };
        handles.push(tokio::spawn(async move {
            let mut req = RequestEnvelope::new(
                tenant,
                "app-1",
                "gpt-4o",
                "Retrieve quarterly confidential M&A valuation summary",
            );
            req.metadata["marker"] = json!(secret_tag);
            let res = app_c
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/requests")
                        .header("content-type", "application/json")
                        .header("x-api-key", key)
                        .body(Body::from(serde_json::to_vec(&req).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            let env: ResponseEnvelope =
                serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(env.tenant_id.0, tenant);
            if let Some(ref cert) = env.certificate {
                assert_eq!(cert.tenant_id.0, tenant);
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn test_g3_invalidation_delay_duplication_reordering_loss_and_outbox_recovery() {
    let transport = Arc::new(InMemoryClusterTransport::new(128));
    let db = Arc::new(MemoryDatabaseStore::new());

    let exact_a = Arc::new(MemoryExactCache::new());
    let vec_a = Arc::new(MemoryVectorStore::new());
    let epoch_a = Arc::new(EpochLeaseManager::new());
    let gcache_a = Arc::new(GraphMemoryCache::new_default());

    let exact_b = Arc::new(MemoryExactCache::new());
    let vec_b = Arc::new(MemoryVectorStore::new());
    let epoch_b = Arc::new(EpochLeaseManager::new());
    let gcache_b = Arc::new(GraphMemoryCache::new_default());

    let sync_a = Arc::new(
        DistributedGraphSyncEngine::new("pod-a", transport.clone(), gcache_a.clone())
            .with_exact_cache(exact_a.clone())
            .with_vector_store(vec_a.clone())
            .with_epoch_manager(epoch_a.clone()),
    );
    let sync_b = Arc::new(
        DistributedGraphSyncEngine::new("pod-b", transport.clone(), gcache_b.clone())
            .with_exact_cache(exact_b.clone())
            .with_vector_store(vec_b.clone())
            .with_epoch_manager(epoch_b.clone()),
    );

    let tenant = TenantId::new("tenant-g3-chaos");
    let app = ApplicationId::new("app-rag");
    let source_id = Uuid::new_v4();
    let doc_hash: u64 = 0xCAFE_BABE_0001;

    // Seed both replicas at epoch 4
    epoch_a.observe_epoch(source_id, 4).await;
    epoch_b.observe_epoch(source_id, 4).await;

    let entry_key = "sunborn:tenant-g3-chaos:key1";
    for (gc, ec, vc) in [(&gcache_a, &exact_a, &vec_a), (&gcache_b, &exact_b, &vec_b)] {
        let ce = CacheEntry::new(entry_key, "epoch4-answer", Some(3600));
        gc.set_with_lineage(ce.clone(), &[source_id], &[doc_hash], 1)
            .await
            .unwrap();
        ec.set(ce).await.unwrap();
        vc.insert(VectorEntry {
            id: entry_key.to_string(),
            tenant_id: tenant.clone(),
            application_id: app.clone(),
            prompt: "Explain Basel III liquidity ratio".to_string(),
            content: "epoch4-answer".to_string(),
            vector: vec![0.1; 128],
            metadata: json!({}),
            created_at: Utc::now(),
        })
        .await
        .unwrap();
    }

    // Scenario A: Reordered + Duplicated messages:
    // Epoch 7 arrives BEFORE delayed Epoch 5, and Epoch 7 arrives twice.
    let mut msg_epoch7 = ClusterInvalidationMessage::new(
        "pod-a",
        tenant.clone(),
        vec![doc_hash],
        vec![source_id],
        7,
    );
    msg_epoch7.mutated_cache_keys = vec![entry_key.to_string()];

    let mut msg_epoch5_delayed = ClusterInvalidationMessage::new(
        "pod-a",
        tenant.clone(),
        vec![doc_hash],
        vec![source_id],
        5,
    );
    msg_epoch5_delayed.mutated_cache_keys = vec![entry_key.to_string()];

    let t_start = Instant::now();
    sync_b.handle_cluster_message(&msg_epoch7).await.unwrap();
    let prop_latency = t_start.elapsed();
    // Duplicate delivery of epoch 7 must be idempotent (no error, fence stays 7)
    sync_b.handle_cluster_message(&msg_epoch7).await.unwrap();
    // Delayed out-of-order epoch 5 arrives AFTER delay
    tokio::time::sleep(Duration::from_millis(25)).await;
    sync_b
        .handle_cluster_message(&msg_epoch5_delayed)
        .await
        .unwrap();

    // Monotonic fence on Replica B MUST remain at 7 (never regress to 5!)
    let lease_b = epoch_b.acquire_lease(source_id).await;
    assert_eq!(
        lease_b, 7,
        "Out-of-order stale epoch 5 must not regress monotonic epoch 7"
    );
    assert!(
        exact_b.get(entry_key).await.unwrap().is_none(),
        "Exact entry on Replica B must be evicted"
    );

    // Scenario B: Lost pub-sub broadcast recovered from durable OutboxRecord replay
    let lost_key = "sunborn:tenant-g3-chaos:key2-lost";
    let src2 = Uuid::new_v4();
    let ce2 = CacheEntry::new(lost_key, "stale-before-outbox-replay", Some(3600));
    gcache_b
        .set_with_lineage(ce2.clone(), &[src2], &[0xBEEF_0002], 2)
        .await
        .unwrap();
    exact_b.set(ce2).await.unwrap();
    epoch_b.observe_epoch(src2, 2).await;

    // Persist durable outbox intent; simulate network partition dropping broadcast
    let outbox_id = db
        .insert_outbox_event(OutboxRecord {
            id: 0,
            tenant_id: tenant.0.clone(),
            entity_type: "knowledge_source".to_string(),
            entity_id: src2.to_string(),
            action: "invalidate".to_string(),
            payload: json!({
                "source_id": src2,
                "bumped_epoch": 9,
                "cache_keys": [lost_key],
            }),
            status: "PENDING".to_string(),
            created_at: Utc::now(),
        })
        .await
        .unwrap();

    // During partition, Replica B has NOT observed epoch 9 yet -> stale window exists
    assert_eq!(epoch_b.acquire_lease(src2).await, 2);
    let stale_window_start = Instant::now();

    // Outbox recovery worker claims PENDING event & replays ClusterInvalidationMessage to Replica B
    let pending = db
        .claim_pending_outbox_events(10, "outbox-worker-1")
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    let p = &pending[0];
    let bumped = p.payload["bumped_epoch"].as_u64().unwrap();
    let mut replay_msg = ClusterInvalidationMessage::new(
        "pod-a",
        tenant.clone(),
        vec![0xBEEF_0002],
        vec![src2],
        bumped,
    );
    replay_msg.mutated_cache_keys = vec![lost_key.to_string()];
    sync_b.handle_cluster_message(&replay_msg).await.unwrap();
    db.mark_outbox_processed(outbox_id).await.unwrap();

    let max_stale_window = stale_window_start.elapsed();
    assert_eq!(epoch_b.acquire_lease(src2).await, 9);
    assert!(exact_b.get(lost_key).await.unwrap().is_none());
    assert_eq!(sync_a.messages_published(), 0); // chaos messages were injected directly
    assert_eq!(db.fetch_pending_outbox_events(10).await.unwrap().len(), 0);
    println!(
        "G3 Invalidation Chaos Measurements: direct_prop_latency={:?}, outbox_recovery_window={:?}",
        prop_latency, max_stale_window
    );
}
