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
async fn test_local_byok_control_endpoints_lifecycle() {
    // In local non-production mode, local endpoints can be accessed directly
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // 1. Check initial status
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/local/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ready");
    assert_eq!(json["mode"], "local_byok");
    assert_eq!(json["localhost_only"], true);

    // 2. Save a local provider (OpenAI)
    let save_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/local/providers")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "provider": "openai",
                        "api_key": "sk-proj-test1234567890abcdef1234",
                        "default_model": "gpt-4o"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save_res.status(), StatusCode::OK);
    let save_body = save_res.into_body().collect().await.unwrap().to_bytes();
    let save_json: Value = serde_json::from_slice(&save_body).unwrap();
    assert_eq!(save_json["status"], "saved");
    assert_eq!(save_json["provider"], "openai");
    // Verify secret key is masked!
    let preview = save_json["key_preview"].as_str().unwrap();
    assert!(preview.starts_with("sk-"));
    assert!(preview.contains("..."));
    assert!(!preview.contains("test1234567890abcdef"));

    // 3. Configure local settings (spend ceiling)
    let config_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/local/config")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "default_spend_ceiling_usd": 25.0,
                        "exact_cache_enabled": true,
                        "semantic_cache_enabled": true
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(config_res.status(), StatusCode::OK);
    let config_body = config_res.into_body().collect().await.unwrap().to_bytes();
    let config_json: Value = serde_json::from_slice(&config_body).unwrap();
    assert_eq!(config_json["status"], "updated");
    assert_eq!(config_json["settings"]["default_spend_ceiling_usd"], 25.0);

    // 4. Delete the local provider
    let del_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/local/providers/openai")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_res.status(), StatusCode::OK);
    let del_body = del_res.into_body().collect().await.unwrap().to_bytes();
    let del_json: Value = serde_json::from_slice(&del_body).unwrap();
    assert_eq!(del_json["status"], "removed");
    assert_eq!(del_json["provider"], "openai");
}
