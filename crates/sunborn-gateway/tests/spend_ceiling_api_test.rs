use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, limits::RequestLimits, AppState};
use tower::ServiceExt;

fn state_with_keys() -> AppState {
    let state = AppState::new(AppConfig::default());
    state.api_key_registry.register_key(
        "admin-spend-key",
        sunborn_core::TenantId::new("tenant-platform-admin"),
        sunborn_core::ApplicationId::new("app-platform"),
        vec!["*".to_string()],
        true,
    );
    state.api_key_registry.register_key(
        "user-spend-key",
        sunborn_core::TenantId::new("tenant-regular"),
        sunborn_core::ApplicationId::new("app-regular"),
        vec!["*".to_string()],
        false,
    );
    state
}

#[tokio::test]
async fn admin_can_set_a_durable_spend_ceiling() {
    let state = state_with_keys();
    let app = create_router(state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/spend-ceiling")
                .header("content-type", "application/json")
                .header("x-api-key", "admin-spend-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tenant_id": "tenant-capped",
                        "ceiling_usd": 12.5
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "updated");
    assert_eq!(json["tenant_id"], "tenant-capped");
    assert_eq!(json["enforcement_scope"], "process_local");

    assert_eq!(
        state
            .spend_velocity_controller
            .ceiling_for_tenant("tenant-capped"),
        12.5
    );
}

#[tokio::test]
async fn admin_can_enroll_a_tenant_quota_and_free_pool() {
    let state = state_with_keys();
    let app = create_router(state.clone());

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/tenant-quota")
                .header("content-type", "application/json")
                .header("x-api-key", "admin-spend-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tenant_id": "tenant-quota",
                        "tier": "free",
                        "daily_requests": 2,
                        "monthly_requests": 10
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert!(state.quota_ledger.reserve("tenant-quota").await.is_ok());
    assert!(state.quota_ledger.reserve("tenant-quota").await.is_ok());
    assert!(state.quota_ledger.reserve("tenant-quota").await.is_err());

    let pool = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/free-pool")
                .header("content-type", "application/json")
                .header("x-api-key", "admin-spend-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "daily_requests": 100,
                        "monthly_requests": 1000
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pool.status(), StatusCode::OK);

    let forbidden = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/tenant-quota")
                .header("content-type", "application/json")
                .header("x-api-key", "user-spend-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tenant_id": "tenant-quota",
                        "daily_requests": 100,
                        "monthly_requests": 1000
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_requests_never_exceed_the_enrolled_quota() {
    let state = state_with_keys();
    state
        .quota_ledger
        .set_tenant_limits(sunborn_db::TenantQuotaLimits::new(
            "tenant-regular",
            "free",
            5,
            100,
        ))
        .await
        .expect("enroll tenant");
    let app = create_router(state.clone());

    let mut handles = Vec::new();
    for _ in 0..30 {
        let app = app.clone();
        handles.push(tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/requests")
                    .header("content-type", "application/json")
                    .header("x-api-key", "user-spend-key")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "tenant_id": "",
                            "application_id": "",
                            "model": "gpt-4o",
                            "prompt": "What is the capital of France?"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }));
    }

    let mut admitted = 0;
    let mut rejected = 0;
    for handle in handles {
        match handle.await.expect("request task joins") {
            StatusCode::OK => admitted += 1,
            StatusCode::TOO_MANY_REQUESTS => rejected += 1,
            other => panic!("unexpected status under quota race: {other}"),
        }
    }
    assert_eq!(admitted, 5, "exactly the daily limit must be admitted");
    assert_eq!(
        rejected, 25,
        "every request past the boundary must be rejected"
    );
}

#[tokio::test]
async fn oversized_prompt_is_rejected_before_origin_dispatch() {
    let mut state = state_with_keys();
    state.request_limits = Arc::new(RequestLimits::new(100, 50));
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "user-spend-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tenant_id": "",
                        "application_id": "",
                        "model": "gpt-4o",
                        "prompt": "x".repeat(200)
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("maximum prompt size"),
        "unexpected detail: {json}"
    );
}

#[tokio::test]
async fn non_admin_and_invalid_ceilings_are_rejected() {
    let state = state_with_keys();
    let app = create_router(state);

    let build = |key: &str, ceiling: f64| {
        Request::builder()
            .method("POST")
            .uri("/v1/governance/spend-ceiling")
            .header("content-type", "application/json")
            .header("x-api-key", key)
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "tenant_id": "tenant-capped",
                    "ceiling_usd": ceiling
                }))
                .unwrap(),
            ))
            .unwrap()
    };

    let forbidden = app
        .clone()
        .oneshot(build("user-spend-key", 5.0))
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let negative = app
        .clone()
        .oneshot(build("admin-spend-key", -1.0))
        .await
        .unwrap();
    assert_eq!(negative.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_failed_durable_spend_ceiling_leaves_local_state_untouched() {
    let state = state_with_keys();
    let tenant = "tenant-fail-test";

    // Set initial local ceiling to 50.0
    state
        .spend_velocity_controller
        .set_tenant_ceiling(tenant, 50.0);
    assert_eq!(
        state.spend_velocity_controller.ceiling_for_tenant(tenant),
        50.0
    );

    // With an unreachable durable database, set_ceiling will fail on durable write
    let unreachable_db = sunborn_db::PostgresDatabaseStore::new(
        "postgres://postgres:invalid@127.0.0.1:54329/sunborn",
    );
    let ledger = sunborn_gateway::finops::SpendLedger::new(
        state.spend_velocity_controller.clone(),
        Some(Arc::new(unreachable_db)),
    );

    let res = ledger.set_ceiling(tenant, 100.0).await;
    assert!(
        res.is_err(),
        "Durable write to unreachable database must fail"
    );
    // Local ceiling MUST NOT have been updated to 100.0!
    assert_eq!(
        state.spend_velocity_controller.ceiling_for_tenant(tenant),
        50.0,
        "Local ceiling must remain at prior value when durable write fails"
    );
}
