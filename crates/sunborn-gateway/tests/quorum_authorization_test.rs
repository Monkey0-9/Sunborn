use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_adversarial_dual_admin_quorum_authorization_enforcement() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let primary_admin = "admin-primary-key-secret";
    let secondary_admin = "admin-secondary-key-secret";
    let non_admin_key = "test-key-alpha";

    state.api_key_registry.register_key(
        primary_admin,
        sunborn_core::TenantId::new("admin-tenant"),
        sunborn_core::ApplicationId::new("admin-app"),
        vec!["*".to_string()],
        true,
    );

    state.api_key_registry.register_key(
        secondary_admin,
        sunborn_core::TenantId::new("admin-tenant-sec"),
        sunborn_core::ApplicationId::new("admin-app-sec"),
        vec!["*".to_string()],
        true,
    );

    // 1. Without SUNBORN_ADMIN_QUORUM_REQUIRED set, single admin can trip chaos
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", primary_admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    state.circuit_breaker.reset().await;

    // 2. Enable strict dual-admin quorum authorization
    std::env::set_var("SUNBORN_ADMIN_QUORUM_REQUIRED", "true");

    // Case A: Missing quorum header on high-consequence operation
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", primary_admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "Missing quorum header must be rejected with 403"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("Quorum authorization required"));

    // Case B: Self-quorum attempt (primary admin provides their own key as secondary)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", primary_admin)
                .header("x-quorum-admin-key", primary_admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "Self-quorum attempt must be rejected with 403"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("Second administrator key must be distinct"));

    // Case C: Non-admin key provided as quorum key
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", primary_admin)
                .header("x-quorum-admin-key", non_admin_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "Non-admin secondary key must fail quorum validation"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("Invalid or unauthorized secondary administrator"));

    // Case D: Valid distinct secondary administrator key -> Success
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", primary_admin)
                .header("x-quorum-admin-key", secondary_admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Valid distinct dual-admin quorum authorization must succeed"
    );

    // Case E: Destructive tenant crypto-shredding with quorum
    // First provision the tenant key in crypto_registry
    let shred_tenant = sunborn_core::TenantId::new("test-shred-target");
    state
        .crypto_registry
        .provision_tenant(shred_tenant, [0x42u8; 32]);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/shred")
                .header("content-type", "application/json")
                .header("x-api-key", primary_admin)
                .header("x-quorum-admin-key", secondary_admin)
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tenant_id": "test-shred-target"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Crypto-shredding with dual-admin quorum must succeed"
    );

    // Case F: Delete tenant with quorum
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/tenants/test-shred-target")
                .header("x-api-key", primary_admin)
                .header("x-quorum-admin-key", secondary_admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Delete tenant with dual-admin quorum must succeed"
    );

    // Clean up environment variable
    std::env::remove_var("SUNBORN_ADMIN_QUORUM_REQUIRED");
}
