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
async fn test_finops_multi_model_frontier_telemetry() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/finops/report")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "active");
    let econ = &json["economics"];

    // Verify all frontier model families are represented in economics
    assert!(
        econ["gpt4o_gross_cost_avoided_per_query_usd"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(econ["claude_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["grok_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["gemini_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["sarvam_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn test_autonomic_circuit_breaker_telemetry() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/autonomic/status")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["autonomic_health"], "operational");
    assert_eq!(json["remediation_mode"], "continuous-closed-loop");
    assert!(json["circuit_breaker_status"].as_str().is_some());
}

#[tokio::test]
async fn test_prometheus_metrics_export_integrity() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();

    // Verify key telemetry metrics are present
    assert!(metrics_text.contains("sunborn_build_info"));
    assert!(metrics_text.contains("sunborn_"));
}
