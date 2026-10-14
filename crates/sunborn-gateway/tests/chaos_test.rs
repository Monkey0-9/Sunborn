use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_chaos_circuit_breaker_resilience_and_origin_fallback() {
    let config = AppConfig::default();
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let req_payload = RequestEnvelope::new(
        "tenant-resilience",
        "app-chaos",
        "gpt-4o",
        "What is chaos engineering?",
    );

    // 1. Initial request to seed cache
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);

    // 2. Second request should hit exact cache
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(resp.cached);
    assert_eq!(resp.decision.action, sunborn_core::DecisionAction::SafeHit);

    // 3. CHAOS INJECTION: Simulate cache cluster crash / trip circuit breaker
    state.circuit_breaker.trip().await;

    // Check chaos status endpoint
    let status_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/chaos/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_res.status(), StatusCode::OK);
    let status_body = status_res.into_body().collect().await.unwrap().to_bytes();
    let status_json: Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["circuit_status"], "Open");

    // 4. Send request during active cache outage
    // CRITICAL ACCEPTANCE CRITERIA: Gateway must NOT return HTTP 500;
    // it must gracefully fall back to origin provider with 100% success rate.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Requests must never fail with HTTP 500 during cache outages"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);
    assert_eq!(
        resp.decision.action,
        sunborn_core::DecisionAction::OriginExecute
    );

    // 5. RESTORATION: Backend recovers, circuit is reset
    state.circuit_breaker.reset().await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(resp.cached);
}
