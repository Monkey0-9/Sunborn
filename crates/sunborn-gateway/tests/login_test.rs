use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::time::Duration;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, middleware::rate_limit::get_login_rate_limiter, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_login_successful_with_valid_credentials() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Reset rate limiter for test client
    let client_ip = "192.0.2.10";
    get_login_rate_limiter().reset(client_ip).await;

    let payload = json!({
        "client_id": "admin-client-1",
        "client_secret": "sunborn-admin-key"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // Verify rate limit headers
    assert_eq!(
        res.headers()
            .get("x-ratelimit-limit")
            .unwrap()
            .to_str()
            .unwrap(),
        "5"
    );
    assert_eq!(
        res.headers()
            .get("x-ratelimit-remaining")
            .unwrap()
            .to_str()
            .unwrap(),
        "4"
    );
    assert!(res.headers().contains_key("x-ratelimit-reset"));

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json_resp: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json_resp["token_type"], "Bearer");
    assert_eq!(json_resp["tenant_id"], "system-admin");
    assert_eq!(json_resp["is_admin"], true);
    assert_eq!(json_resp["access_token"], "sunborn-admin-key");
}

#[tokio::test]
async fn test_login_invalid_credentials_returns_unauthorized() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let client_ip = "192.0.2.11";
    get_login_rate_limiter().reset(client_ip).await;

    let payload = json!({
        "client_id": "test-client",
        "client_secret": "invalid-secret-key-12345"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        res.headers()
            .get("x-ratelimit-remaining")
            .unwrap()
            .to_str()
            .unwrap(),
        "4"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json_resp["status"], 401);
    assert_eq!(json_resp["title"], "Unauthorized");
}

#[tokio::test]
async fn test_login_empty_credentials_rejected_with_bad_request() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let client_ip = "192.0.2.12";
    get_login_rate_limiter().reset(client_ip).await;

    let payload = json!({
        "client_id": "",
        "client_secret": ""
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json_resp["status"], 400);
    assert_eq!(json_resp["title"], "Bad Request");
}

#[tokio::test]
async fn test_login_rate_limiting_enforcement_429() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let client_ip = "192.0.2.20";
    get_login_rate_limiter().reset(client_ip).await;

    let payload = json!({
        "client_id": "test-client",
        "client_secret": "wrong-key"
    });

    // 5 attempts allowed in burst capacity
    for i in 0..5 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", client_ip)
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "Attempt {} should be 401",
            i + 1
        );
        let remaining = (4 - i).to_string();
        assert_eq!(
            res.headers()
                .get("x-ratelimit-remaining")
                .unwrap()
                .to_str()
                .unwrap(),
            remaining.as_str()
        );
    }

    // 6th attempt must be RATE LIMITED (429 Too Many Requests)
    let res_rate_limited = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_rate_limited.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th attempt must trigger HTTP 429 Too Many Requests"
    );

    // Verify rate limit headers on 429
    assert_eq!(
        res_rate_limited
            .headers()
            .get("x-ratelimit-remaining")
            .unwrap()
            .to_str()
            .unwrap(),
        "0"
    );
    assert!(res_rate_limited.headers().contains_key("retry-after"));

    let body = res_rate_limited
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let json_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json_resp["status"], 429);
    assert_eq!(json_resp["title"], "Too Many Requests");
    assert!(json_resp["detail"]
        .as_str()
        .unwrap()
        .contains("Rate limit exceeded"));
}

#[tokio::test]
async fn test_login_rate_limiting_ip_isolation() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let attacker_ip = "192.0.2.30";
    let legitimate_ip = "192.0.2.31";

    get_login_rate_limiter().reset(attacker_ip).await;
    get_login_rate_limiter().reset(legitimate_ip).await;

    let payload_attacker = json!({
        "client_id": "brute-force-attacker",
        "client_secret": "guessed-secret"
    });

    // Attacker exhausts all 5 tokens
    for _ in 0..5 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", attacker_ip)
                    .body(Body::from(serde_json::to_vec(&payload_attacker).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Attacker is now blocked (429)
    let res_attacker_blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", attacker_ip)
                .body(Body::from(serde_json::to_vec(&payload_attacker).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_attacker_blocked.status(), StatusCode::TOO_MANY_REQUESTS);

    // Legitimate user from different IP can still successfully log in (200 OK)
    let payload_legit = json!({
        "client_id": "legit-client",
        "client_secret": "sunborn-admin-key"
    });
    let res_legit = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", legitimate_ip)
                .body(Body::from(serde_json::to_vec(&payload_legit).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_legit.status(),
        StatusCode::OK,
        "Legitimate IP must not be affected by attacker IP rate limit"
    );
}

#[tokio::test]
async fn test_login_rate_limiting_refill_recovery() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let client_ip = "192.0.2.40";
    get_login_rate_limiter().reset(client_ip).await;

    let payload = json!({
        "client_id": "test-client",
        "client_secret": "sunborn-admin-key"
    });

    // Exhaust capacity (5 requests)
    for _ in 0..5 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", client_ip)
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // 6th is rate limited
    let res_blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_blocked.status(), StatusCode::TOO_MANY_REQUESTS);

    // Wait for refill: refill rate is 1 token per second, wait 1.1s
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // After refill, request succeeds again!
    let res_recovered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", client_ip)
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_recovered.status(),
        StatusCode::OK,
        "Request must succeed after token bucket refills"
    );
}
