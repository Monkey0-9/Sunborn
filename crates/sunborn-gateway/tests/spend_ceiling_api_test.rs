use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
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
