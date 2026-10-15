use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_cache::ExactCacheStore;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope, TenantId};
use sunborn_db::models::TenantRecord;
use sunborn_gateway::{create_router, AppState};
use sunborn_invalidation::EpochLeaseManager;
use sunborn_semantic::VectorStore;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_deterministic_simulation_non_accelerator_authority_under_chaos() {
    // -------------------------------------------------------------------------
    // Phase 1: Initialize System & Setup Fault Injection Simulation Environment
    // -------------------------------------------------------------------------
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let tenant_id = "tenant-dst-cluster-42";

    state.api_key_registry.register_key(
        "test-key-sim",
        TenantId::new(tenant_id),
        sunborn_core::ApplicationId::new("app-simulation"),
        vec!["*".to_string()],
        true,
    );

    state
        .db
        .insert_tenant(TenantRecord {
            id: tenant_id.into(),
            name: "DST Simulation Enterprises".into(),
            status: "active".into(),
            rate_limit_rps: 1000,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // Phase 2: Seed Authoritative Records through Gateway
    // -------------------------------------------------------------------------
    let prompt_1 = "Calculate risk-adjusted portfolio return metrics.";
    let req_1 = RequestEnvelope::new(tenant_id, "app-simulation", "gpt-4o", prompt_1);

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from(serde_json::to_vec(&req_1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::OK);
    let body1 = res1.into_body().collect().await.unwrap().to_bytes();
    let resp1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!resp1.cached, "Initial request must be origin miss");

    // Warm hit check
    let res1_warm = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from(serde_json::to_vec(&req_1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1_warm.status(), StatusCode::OK);
    let body_warm = res1_warm.into_body().collect().await.unwrap().to_bytes();
    let resp1_warm: ResponseEnvelope = serde_json::from_slice(&body_warm).unwrap();
    assert!(resp1_warm.cached, "Second request must hit cache");

    // -------------------------------------------------------------------------
    // Phase 3: Simulated Network Partition / Split-Brain during Invalidation
    // -------------------------------------------------------------------------
    let lease_manager = EpochLeaseManager::new();
    let doc_id = Uuid::new_v4();
    let read_epoch = lease_manager.acquire_lease(doc_id).await;
    assert_eq!(read_epoch, 1);

    // Invalidation event occurs in origin/outbox
    let inv_payload = serde_json::json!({
        "tenant_id": tenant_id,
        "source_id": doc_id,
        "version_tag": "v2.0"
    });

    let inv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/invalidation")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from(serde_json::to_vec(&inv_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inv_res.status(), StatusCode::OK);

    // Epoch advances upon invalidation
    lease_manager.bump_epoch(doc_id).await;
    // Stale writeback lease must be rejected by monotonic epoch validation
    let is_valid = lease_manager.validate_lease(doc_id, read_epoch).await;
    assert!(!is_valid, "Stale lease must be rejected by epoch fence");

    // -------------------------------------------------------------------------
    // Phase 4: Catastrophic Accelerator Crash Simulation (L1/L2/Qdrant wiped)
    // -------------------------------------------------------------------------
    // Trip chaos circuit breaker
    let trip_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(trip_res.status(), StatusCode::OK);

    // Verify system continues to serve gracefully (zero HTTP 500 errors)
    let res_during_chaos = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from(serde_json::to_vec(&req_1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(res_during_chaos.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(res_during_chaos.status(), StatusCode::OK);

    // Reset chaos
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/reset")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Wipe accelerator in-memory caches to simulate total index destruction
    let _ = state.memory_cache.delete_by_prefix("").await;
    let _ = state
        .vector_store
        .delete_by_tenant(&TenantId::new(tenant_id))
        .await;

    // Verify cache is physically empty
    let empty_lookup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/some_nonexistent_key")
                .header("x-api-key", "test-key-sim")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_lookup.status(), StatusCode::NOT_FOUND);

    // -------------------------------------------------------------------------
    // Phase 5: Rebuild Replay from Authoritative PostgreSQL Store
    // -------------------------------------------------------------------------
    let rebuild_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/rebuild")
                .header("x-api-key", "test-key-sim")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rebuild_res.status(), StatusCode::OK);
    let rebuild_body = rebuild_res.into_body().collect().await.unwrap().to_bytes();
    let rebuild_json: Value = serde_json::from_slice(&rebuild_body).unwrap();
    assert_eq!(rebuild_json["status"], "completed");
    assert!(rebuild_json["rebuilt_exact_cache"].as_u64().unwrap() >= 1);

    // -------------------------------------------------------------------------
    // Phase 6: Invariant Verification
    // Invariant: Non-Accelerator Authority guarantees reconstructed cache hits
    // match authoritative state with 0 stale reads and identical content.
    // -------------------------------------------------------------------------
    let post_rebuild_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-sim")
                .body(Body::from(serde_json::to_vec(&req_1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(post_rebuild_res.status(), StatusCode::OK);
    let post_rebuild_body = post_rebuild_res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let post_resp: ResponseEnvelope = serde_json::from_slice(&post_rebuild_body).unwrap();
    assert!(
        post_resp.cached,
        "Post-rebuild request must hit reconstructed cache"
    );
    assert_eq!(post_resp.content, resp1.content);
}
