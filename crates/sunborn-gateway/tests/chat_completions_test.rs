use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_openai_chat_completions_miss_then_exact_hit() {
    let mut config = AppConfig::default();
    config.auth.enforce_auth = true;
    let state = AppState::new(config);
    let app = create_router(state);

    let chat_payload = json!({
        "model": "gpt-4o",
        "messages": [
            { "role": "system", "content": "You are an enterprise AI assistant." },
            { "role": "user", "content": "Explain deterministic execution reuse in distributed caching." }
        ],
        "temperature": 0.2
    });
    let payload_bytes = serde_json::to_vec(&chat_payload).unwrap();

    // 1. First Call: Cache MISS (Forwarded to Origin)
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(payload_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::OK);
    let body1 = res1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body1).unwrap();

    assert_eq!(json1["object"], "chat.completion");
    assert_eq!(json1["model"], "gpt-4o");
    assert_eq!(json1["sunborn_cached"], false);
    assert_eq!(json1["sunborn_tier"], "origin");
    assert_eq!(json1["choices"][0]["message"]["role"], "assistant");
    assert!(!json1["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .is_empty());

    // 2. Second Call: Cache HIT (Exact C0/C1)
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(payload_bytes))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::OK);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();

    assert_eq!(json2["sunborn_cached"], true, "Second call must be cached");
    assert_eq!(json2["sunborn_tier"], "c1_exact");
    assert_eq!(
        json2["choices"][0]["message"]["content"],
        json1["choices"][0]["message"]["content"]
    );
}

#[tokio::test]
async fn test_chat_completions_rejects_empty_messages() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let empty_payload = json!({
        "model": "gpt-4o",
        "messages": []
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-dev-key")
                .body(Body::from(serde_json::to_vec(&empty_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_finops_zero_hits_reports_zero_savings() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/finops/report")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let total_hits = json["metrics"]["total_hits"].as_u64().unwrap_or(0);
    if total_hits == 0 {
        assert_eq!(
            json["economics"]["total_estimated_savings_usd"], 0.0,
            "Zero hits must report exactly $0.00 savings"
        );
    } else {
        assert!(
            json["economics"]["total_estimated_savings_usd"]
                .as_f64()
                .unwrap_or(0.0)
                >= 0.0
        );
    }
}

#[tokio::test]
async fn test_admin_lockdown_on_destructive_endpoints() {
    let mut config = AppConfig::default();
    config.auth.enforce_auth = true;
    let state = AppState::new(config);
    let app = create_router(state);

    // 1. Non-admin credential calling /v1/chaos/trip -> 403 Forbidden
    let res_forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", "test-key-alpha") // Non-admin key
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_forbidden.status(),
        StatusCode::FORBIDDEN,
        "Non-admin key must be forbidden from tripping chaos breaker"
    );

    // 2. Admin credential calling /v1/chaos/trip -> 200 OK
    let res_allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chaos/trip")
                .header("x-api-key", "sunborn-admin-key") // Admin key
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_allowed.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_request_validation_bounds() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Temperature > 2.0 must be rejected
    let invalid_temp = json!({
        "tenant_id": "system-admin",
        "application_id": "admin-ops",
        "model": "gpt-4o",
        "prompt": "Valid prompt text",
        "temperature": 3.5
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(serde_json::to_vec(&invalid_temp).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
