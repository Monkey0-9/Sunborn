use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use sunborn_core::config::AppConfig;
use sunborn_core::finops::SpendVelocityController;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_execution::{ExecutionCacheStore, PersistentExecutionCacheStore, ToolResult};
use sunborn_gateway::{
    create_router,
    learning::{TenantLearningStore, VerifiedFeedbackEvent, VerifiedOutcome},
    AppState,
};
use tower::ServiceExt;

#[tokio::test]
async fn test_learning_and_execution_wal_physical_removal_on_disk() {
    let tmp_dir = std::env::temp_dir().join(format!("sunborn-wal-test-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let learning_wal = tmp_dir.join("learning.wal");
    let execution_wal = tmp_dir.join("execution.wal");

    let tenant = TenantId::new("tenant-wal-test");
    let app_id = ApplicationId::new("app-test");

    // 1. Initialize learning store with durable WAL
    let learning_store = Arc::new(TenantLearningStore::new_with_wal(
        500,
        Some(learning_wal.clone()),
    ));
    assert!(learning_store.is_durable());

    let event = VerifiedFeedbackEvent {
        event_id: "ev-test-1".to_string(),
        tenant_id: tenant.clone(),
        application_id: app_id.clone(),
        cache_key: "sunborn:tenant-wal-test:k1".to_string(),
        decision_type: "exact_hit".to_string(),
        prompt_hash: "hash-wal-1".to_string(),
        similarity_score: Some(0.99),
        conformal_risk_score: Some(0.01),
        tokens_saved: 120,
        cost_saved_usd: 0.002,
        latency_ms: 1.5,
        verified_outcome: VerifiedOutcome::VerifiedCorrect,
        verification_source: "origin_execution".to_string(),
        hmac_signature: "sig123".to_string(),
        timestamp: chrono::Utc::now(),
    };
    learning_store.record_event(event).unwrap();
    assert_eq!(learning_store.tenant_event_count(&tenant), 1);
    assert!(learning_wal.exists());
    assert!(!learning_store.verify_tenant_erased_on_disk(&tenant));

    // 2. Initialize execution store with durable WAL
    let execution_store = Arc::new(PersistentExecutionCacheStore::new_with_wal(&execution_wal));
    let tool_res = ToolResult {
        tool_name: "calculator".into(),
        signature: "sig-calc-1".into(),
        output: serde_json::json!({"res": 42}),
        is_error: false,
        execution_latency_ms: 5,
        created_at: chrono::Utc::now(),
    };
    execution_store
        .set(&tenant, "sig-calc-1", &tool_res, Some(3600))
        .await
        .unwrap();
    assert_eq!(execution_store.count_by_tenant(&tenant).await.unwrap(), 1);
    assert!(execution_wal.exists());
    assert!(!execution_store.verify_tenant_erased_on_disk(&tenant));

    // 3. Purge tenant data
    let purged_learning = learning_store.purge_tenant(&tenant).unwrap();
    assert_eq!(purged_learning, 1);
    assert_eq!(learning_store.tenant_event_count(&tenant), 0);
    // Assert on-disk WAL is physically shredded of this tenant's data
    assert!(learning_store.verify_tenant_erased_on_disk(&tenant));

    let evicted_exec = execution_store.evict_by_tenant(&tenant).await.unwrap();
    assert_eq!(evicted_exec, 1);
    assert_eq!(execution_store.count_by_tenant(&tenant).await.unwrap(), 0);
    // Assert on-disk execution WAL has zero remnants
    assert!(execution_store.verify_tenant_erased_on_disk(&tenant));

    // Inspect the actual WAL files directly
    let learning_content = std::fs::read_to_string(&learning_wal).unwrap();
    assert!(!learning_content.contains("tenant-wal-test"));

    let execution_content = std::fs::read_to_string(&execution_wal).unwrap();
    assert!(!execution_content.contains("tenant-wal-test"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[tokio::test]
async fn test_financial_settlement_rejects_negative_and_nan() {
    let controller = SpendVelocityController::default();
    let tenant = "tenant-fin-test";
    controller.set_tenant_ceiling(tenant, 100.0);

    let res_id = controller.reserve_spend(tenant, 10.0).unwrap();

    // Settle with NaN should be rejected, not silently recorded as zero spend
    let after_nan = controller.settle_spend(tenant, res_id, f64::NAN);
    assert_eq!(after_nan, 0.0);

    let res_id2 = controller.reserve_spend(tenant, 10.0).unwrap();
    // Settle with negative amount should be rejected
    let after_neg = controller.settle_spend(tenant, res_id2, -5.0);
    assert_eq!(after_neg, 0.0);

    // Valid settlement succeeds and reflects true spend
    let res_id3 = controller.reserve_spend(tenant, 10.0).unwrap();
    let after_valid = controller.settle_spend(tenant, res_id3, 7.5);
    assert_eq!(after_valid, 7.5);
    assert_eq!(controller.current_spend(tenant), 7.5);
}

#[tokio::test]
async fn test_end_to_end_durable_erasure_with_disk_verification() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let tenant = "tenant-e2e-erasure";
    let tenant_id = TenantId::new(tenant);
    let app_id = ApplicationId::new("app-e2e");

    let _ = state
        .crypto_registry
        .provision_tenant_secure(tenant_id.clone());

    // Record feedback event
    let event = VerifiedFeedbackEvent {
        event_id: "ev-e2e-1".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: app_id.clone(),
        cache_key: format!("sunborn:{}:k", tenant),
        decision_type: "exact_hit".to_string(),
        prompt_hash: "hash-1".to_string(),
        similarity_score: Some(1.0),
        conformal_risk_score: Some(0.0),
        tokens_saved: 10,
        cost_saved_usd: 0.001,
        latency_ms: 1.2,
        verified_outcome: VerifiedOutcome::VerifiedCorrect,
        verification_source: "origin_execution".to_string(),
        hmac_signature: "mock_sig".to_string(),
        timestamp: chrono::Utc::now(),
    };
    state.learning_store.record_event(event).unwrap();
    assert_eq!(state.learning_store.tenant_event_count(&tenant_id), 1);

    // Call DELETE endpoint
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/tenants/{}", tenant))
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["status"], "erased");
    assert_eq!(val["verified_multi_store_erasure"], true);
    assert_eq!(val["residual_records"]["learning_store"], 0);
}
