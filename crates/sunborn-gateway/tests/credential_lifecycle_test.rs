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
async fn test_credential_lifecycle_create_rotate_revoke_and_shred() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let admin_key = "admin-lifecycle-primary";
    let quorum_key = "admin-lifecycle-secondary";

    state.api_key_registry.register_key(
        admin_key,
        sunborn_core::TenantId::new("admin-tenant"),
        sunborn_core::ApplicationId::new("admin-ops"),
        vec!["*".to_string()],
        true,
    );
    state.api_key_registry.register_key(
        quorum_key,
        sunborn_core::TenantId::new("admin-sec"),
        sunborn_core::ApplicationId::new("admin-ops-sec"),
        vec!["*".to_string()],
        true,
    );

    // 1. Create a new API credential for tenant-lifecycle-1
    let create_payload = serde_json::json!({
        "tenant_id": "tenant-lifecycle-1",
        "application_id": "app-lifecycle-1",
        "allowed_models": ["gpt-4o", "claude-3-5-sonnet"],
        "is_admin": false,
        "rate_limit_rps": 50
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .header("x-quorum-admin-key", quorum_key)
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let create_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(create_json["status"], "created");
    let raw_key = create_json["raw_key"].as_str().unwrap().to_string();
    let key_hash = create_json["key_hash"].as_str().unwrap().to_string();
    assert!(raw_key.starts_with("sb_live_"));

    // 2. Validate that the newly created key can authenticate
    let check_req = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/test-key-1")
                .header("x-api-key", &raw_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Cache miss returns 404 or empty json, but NOT 401 Unauthorized
    assert_ne!(check_req.status(), StatusCode::UNAUTHORIZED);

    // 3. Rotate the credential
    let rotate_payload = serde_json::json!({
        "old_key_hash": key_hash,
        "tenant_id": "tenant-lifecycle-1",
        "application_id": "app-lifecycle-1",
        "allowed_models": ["gpt-4o"],
        "is_admin": false
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials/rotate")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .header("x-quorum-admin-key", quorum_key)
                .body(Body::from(rotate_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let rotate_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(rotate_json["status"], "rotated");
    let new_raw_key = rotate_json["new_raw_key"].as_str().unwrap().to_string();
    let new_key_hash = rotate_json["new_key_hash"].as_str().unwrap().to_string();
    assert_ne!(raw_key, new_raw_key);

    // 4. OLD key must immediately be rejected with 401 Unauthorized
    let old_key_req = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/test-key-1")
                .header("x-api-key", &raw_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_key_req.status(), StatusCode::UNAUTHORIZED);

    // 5. NEW key succeeds
    let new_key_req = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/test-key-1")
                .header("x-api-key", &new_raw_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(new_key_req.status(), StatusCode::UNAUTHORIZED);

    // 6. Revoke the new key via /v1/auth/credentials/revoke
    let revoke_payload = serde_json::json!({
        "key_hash": new_key_hash
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials/revoke")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .header("x-quorum-admin-key", quorum_key)
                .body(Body::from(revoke_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // 7. NEW key is now also rejected with 401 Unauthorized
    let revoked_req = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/caches/test-key-1")
                .header("x-api-key", &new_raw_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_req.status(), StatusCode::UNAUTHORIZED);
}
