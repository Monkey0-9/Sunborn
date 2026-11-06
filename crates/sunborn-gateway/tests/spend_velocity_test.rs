use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_spend_velocity_hard_stop_and_tenant_isolation() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant_a = "tenant-spend-a";
    let tenant_b = "tenant-spend-b";
    let key_a = "key-spend-tenant-a";
    let key_b = "key-spend-tenant-b";

    state.api_key_registry.register_key(
        key_a,
        sunborn_core::TenantId::new(tenant_a),
        sunborn_core::ApplicationId::new("app-spend-a"),
        vec!["*".to_string()],
        false,
    );
    state.api_key_registry.register_key(
        key_b,
        sunborn_core::TenantId::new(tenant_b),
        sunborn_core::ApplicationId::new("app-spend-b"),
        vec!["*".to_string()],
        false,
    );

    // Set a very tight ceiling for tenant A ($0.0001)
    state
        .spend_velocity_controller
        .set_tenant_ceiling(tenant_a, 0.0001);
    // Tenant B has high ceiling ($50.0)
    state
        .spend_velocity_controller
        .set_tenant_ceiling(tenant_b, 50.0);

    let req_payload_a = serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000001",
        "tenant_id": tenant_a,
        "application_id": "app-spend-a",
        "model": "gpt-4o",
        "prompt": "Explain quantum computing in detail",
        "dependency_snapshots": []
    });

    // 1. First request for tenant A: Origin call occurs (cost ~ $0.00625 for GPT-4o), which completes
    // and records expenditure, pushing tenant A spend over its $0.005 ceiling.
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_a)
                .body(Body::from(serde_json::to_vec(&req_payload_a).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::OK);

    // Verify tenant A spend has exceeded ceiling
    let current_spend = state.spend_velocity_controller.current_spend(tenant_a);
    assert!(
        current_spend > 0.0001,
        "Tenant A spend should exceed $0.0001 after first origin call (actual: {current_spend})"
    );

    // 2. Second request for tenant A must immediately receive HTTP 429 Too Many Requests (hard stop)
    let req_payload_a2 = serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000002",
        "tenant_id": tenant_a,
        "application_id": "app-spend-a",
        "model": "gpt-4o",
        "prompt": "Explain general relativity in detail",
        "dependency_snapshots": []
    });

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_a)
                .body(Body::from(serde_json::to_vec(&req_payload_a2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res2.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "Subsequent requests after exceeding spend velocity ceiling must fail with HTTP 429"
    );

    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    let detail = json2["detail"].as_str().unwrap_or("");
    assert!(
        detail.contains("Spend velocity ceiling of $0.01 exceeded")
            || detail.contains("Spend velocity ceiling"),
        "Error detail must state spend velocity ceiling exceeded: {detail}"
    );

    // 3. Tenant B is completely unaffected (Tenant Isolation)
    let req_payload_b = serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000003",
        "tenant_id": tenant_b,
        "application_id": "app-spend-b",
        "model": "gpt-4o",
        "prompt": "Tenant B request should succeed",
        "dependency_snapshots": []
    });

    let res_b = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_b)
                .body(Body::from(serde_json::to_vec(&req_payload_b).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_b.status(),
        StatusCode::OK,
        "Tenant B must not be throttled by Tenant A hitting their ceiling"
    );

    // 4. Reset tenant A spend -> Requests succeed again
    state.spend_velocity_controller.reset_tenant(tenant_a);
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_a)
                .body(Body::from(serde_json::to_vec(&req_payload_a2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res3.status(),
        StatusCode::OK,
        "After resetting tenant spend, requests must succeed again"
    );
}
