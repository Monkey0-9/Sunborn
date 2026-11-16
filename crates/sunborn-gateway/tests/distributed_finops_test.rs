//! Verifies that spend enforcement never silently degrades to a process-local
//! budget when the durable ledger is configured but unreachable.
//!
//! This test mutates process-wide environment variables, so it lives in its own
//! test binary and must remain the only test in this file.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn distributed_ledger_fails_closed_when_store_is_unreachable() {
    std::env::set_var("SUNBORN_DB_BACKEND", "postgres");
    std::env::set_var(
        "POSTGRES_URL",
        "postgres://sunborn:sunborn@127.0.0.1:1/sunborn?connect_timeout=1",
    );

    let state = AppState::new(AppConfig::default());
    assert!(
        state.spend_ledger.is_distributed(),
        "postgres backend must enable the durable spend ledger"
    );
    assert_eq!(
        state.spend_ledger.enforcement_scope(),
        "cluster_distributed"
    );

    let tenant = "tenant-distributed-finops";
    let key = "key-distributed-finops";
    state.api_key_registry.register_key(
        key,
        sunborn_core::TenantId::new(tenant),
        sunborn_core::ApplicationId::new("app-distributed-finops"),
        vec!["*".to_string()],
        false,
    );

    let app = create_router(state.clone());
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key)
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "request_id": "00000000-0000-0000-0000-0000000000aa",
                        "tenant_id": tenant,
                        "application_id": "app-distributed-finops",
                        "model": "gpt-4o",
                        "prompt": "Spend control must not fail open",
                        "dependency_snapshots": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Not 429 (which would mean "budget exhausted") and not 200 (which would
    // mean the request ran without shared budget enforcement): the gateway must
    // refuse to dispatch and report the dependency failure.
    assert_ne!(res.status(), StatusCode::OK);
    assert_ne!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}
