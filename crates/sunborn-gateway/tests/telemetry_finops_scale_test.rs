//! Comprehensive Telemetry, FinOps, and Concurrency Scale Integration Suite.
//!
//! Validates:
//! 1. Multi-model frontier FinOps economics, cost avoidance metrics, and Indic GST breakdown.
//! 2. Autonomic self-healing controller status, incident recording, and circuit breaker telemetry.
//! 3. Honest cluster mesh topology and replica synchronization reporting.
//! 4. Full Prometheus `/metrics` exposition format compliance and metric key integrity.
//! 5. Authentication and authorization fail-closed enforcement across telemetry routes.
//! 6. High-concurrency multi-worker scale benchmarks asserting zero dropped requests.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
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

    // Verify Cache-Control header
    let cache_control = response
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(cache_control.contains("max-age"));

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "active");

    // Metrics block
    let metrics = &json["metrics"];
    assert!(metrics["exact_hits"].is_number());
    assert!(metrics["semantic_hits"].is_number());
    assert!(metrics["total_hits"].is_number());
    assert!(metrics["requests_total"].is_number());

    // Economics block
    let econ = &json["economics"];

    // 1. Verify all frontier model families are represented with positive cost avoidance
    let gpt4o_gross = econ["gpt4o_gross_cost_avoided_per_query_usd"]
        .as_f64()
        .expect("gpt4o gross cost avoided");
    let gpt4o_net = econ["gpt4o_net_savings_per_query_usd"]
        .as_f64()
        .expect("gpt4o net savings");
    assert!(gpt4o_gross > 0.0);
    assert!(gpt4o_net > 0.0);
    assert!(
        gpt4o_gross >= gpt4o_net,
        "Gross cost avoided must equal or exceed net savings"
    );

    assert!(econ["claude_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["grok_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["gemini_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);
    assert!(econ["sarvam_cost_avoided_per_query_usd"].as_f64().unwrap() > 0.0);

    // 2. Verify local overhead accounting
    assert!(
        econ["embedding_inference_cost_per_query_usd"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        econ["cache_lookup_overhead_cost_per_query_usd"]
            .as_f64()
            .unwrap()
            > 0.0
    );

    // 3. Verify Sovereign Indic Currency and GST compliance
    let inr_rate = econ["inr_exchange_rate"].as_f64().expect("INR rate");
    assert_eq!(inr_rate, 84.50);
    assert_eq!(econ["gst_applicable_rate_percent"].as_f64().unwrap(), 18.0);
    assert_eq!(econ["gst_sac_code"].as_str().unwrap(), "998313");
    assert!(econ["gst_breakdown"].is_object());

    // 4. Verify break-even hit rate threshold
    let break_even = econ["break_even_hit_rate_threshold"]
        .as_f64()
        .expect("break-even hit rate");
    assert!((0.0..=1.0).contains(&break_even));
}

#[tokio::test]
async fn test_finops_unauthorized_access_rejected() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Request without API key header
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/finops/report")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Request with invalid API key header
    let response_invalid = app
        .oneshot(
            Request::builder()
                .uri("/v1/finops/report")
                .header("x-api-key", "invalid-unregistered-key-xyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_invalid.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_autonomic_circuit_breaker_telemetry() {
    let state = AppState::new(AppConfig::default());

    // Record an automated self-healing recovery action
    state.autonomic_controller.record_recovery(
        "qdrant_vector_store",
        "transient_network_timeout",
        "circuit_breaker_half_open_reroute",
    );

    let app = create_router(state);

    let response = app
        .clone()
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
    assert!(json["total_incidents_resolved"].as_u64().unwrap() >= 1);

    let recent = json["recent_incidents"]
        .as_array()
        .expect("recent incidents");
    assert!(!recent.is_empty());
    assert_eq!(recent[0]["component"], "qdrant_vector_store");
    assert_eq!(recent[0]["trigger_reason"], "transient_network_timeout");
    assert_eq!(
        recent[0]["recovery_action"],
        "circuit_breaker_half_open_reroute"
    );

    // Verify alias endpoint /v1/autonomic/incidents
    let response_alias = app
        .oneshot(
            Request::builder()
                .uri("/v1/autonomic/incidents")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_alias.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_autonomic_unauthorized_access_rejected() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Request without API key header
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/autonomic/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Request with invalid API key header
    let response_invalid = app
        .oneshot(
            Request::builder()
                .uri("/v1/autonomic/incidents")
                .header("x-api-key", "bogus-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_invalid.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_cluster_mesh_sync_telemetry() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/cluster/sync/status")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Truthful topology checks: no fabricated multi-region metrics
    assert_eq!(json["nodes_active"], 1);
    assert_eq!(json["nodes_active_scope"], "local_process_only");
    assert_eq!(json["replica_telemetry"], "unavailable");
    assert!(json["node_id"].is_string());
    assert!(json["region"].is_string());
    assert_eq!(json["spend_velocity_enforcement"], "active_enforced");
    assert!(json["vector_clock"].is_object());

    // Test alias endpoint /v1/cluster/status
    let response_alias = app
        .oneshot(
            Request::builder()
                .uri("/v1/cluster/status")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_alias.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_prometheus_metrics_export_integrity() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Public /metrics probe requires zero credentials
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();

    // Verify key telemetry metrics and Prometheus metadata types
    assert!(metrics_text.contains("# HELP sunborn_requests_total"));
    assert!(metrics_text.contains("# TYPE sunborn_requests_total counter"));
    assert!(metrics_text.contains("sunborn_requests_total{scope=\"process_local\"}"));

    assert!(
        metrics_text.contains("sunborn_cache_hits_total{scope=\"process_local\",tier=\"exact\"}")
    );
    assert!(metrics_text
        .contains("sunborn_cache_hits_total{scope=\"process_local\",tier=\"semantic\"}"));
    assert!(metrics_text.contains("sunborn_cache_misses_total{scope=\"process_local\"}"));
    assert!(metrics_text.contains("sunborn_rate_limit_exceeded_total{scope=\"process_local\"}"));
    assert!(metrics_text.contains("sunborn_spend_velocity_exceeded_total{scope=\"process_local\"}"));
    assert!(metrics_text.contains("sunborn_quota_exceeded_total{scope=\"process_local\"}"));
    assert!(metrics_text.contains(
        "sunborn_telemetry_source{cluster=\"gateway-primary\",source=\"process_local_worker\"} 1"
    ));
    assert!(metrics_text.contains("sunborn_request_duration_seconds_bucket"));
    assert!(metrics_text.contains("sunborn_request_duration_seconds_count"));
    assert!(metrics_text.contains("sunborn_request_duration_seconds_sum"));

    // Verify method safety: POST to /metrics returns 405 Method Not Allowed
    let response_post = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_post.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_concurrent_multiworker_telemetry_scale() {
    let state = AppState::new(AppConfig::default());
    let workers = 8;
    let requests_per_worker = 50;
    let total_expected_requests = workers * requests_per_worker;

    let successful_requests = Arc::new(AtomicUsize::new(0));
    let t0 = Instant::now();

    let mut handles = Vec::with_capacity(workers);

    for worker_id in 0..workers {
        let state_clone = state.clone();
        let success_counter = successful_requests.clone();

        handles.push(tokio::spawn(async move {
            let app = create_router(state_clone);

            for req_idx in 0..requests_per_worker {
                // Interleave requests across various telemetry surfaces
                let uri = match (worker_id + req_idx) % 4 {
                    0 => "/metrics",
                    1 => "/v1/finops/report",
                    2 => "/v1/autonomic/status",
                    _ => "/v1/cluster/sync/status",
                };

                let mut req_builder = Request::builder().uri(uri);
                if uri != "/metrics" {
                    req_builder = req_builder.header("x-api-key", "sunborn-admin-key");
                }

                let response = app
                    .clone()
                    .oneshot(req_builder.body(Body::empty()).unwrap())
                    .await
                    .expect("Router oneshot execution failed");

                if response.status() == StatusCode::OK {
                    success_counter.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Worker task panicked");
    }

    let elapsed = t0.elapsed();
    let total_success = successful_requests.load(Ordering::SeqCst);

    assert_eq!(
        total_success, total_expected_requests,
        "All concurrent telemetry requests must complete with HTTP 200 OK"
    );

    let rps = (total_success as f64) / elapsed.as_secs_f64().max(0.001);
    assert!(
        rps > 100.0,
        "Concurrent scale throughput must exceed minimum target: {rps:.2} req/sec"
    );
}
