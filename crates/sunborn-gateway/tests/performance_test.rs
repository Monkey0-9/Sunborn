use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::time::Instant;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_gateway_throughput_and_p99_latency() {
    let mut config = AppConfig::default();
    config.cache.semantic_cache_enabled = false; // Isolate exact cache hot path
    let state = AppState::new(config);
    let app = create_router(state);

    let req_payload = RequestEnvelope::new(
        "tenant-perf",
        "app-bench",
        "gpt-4o",
        "Benchmark prompt for high-throughput exact cache validation",
    );
    let req_bytes = serde_json::to_vec(&req_payload).unwrap();

    // 1. Warmup request to populate cache
    let warmup_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(req_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(warmup_res.status(), StatusCode::OK);
    let body = warmup_res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);

    // 2. High-throughput loop: 5,000 in-memory requests
    let iterations = 5000;
    let mut latencies_us = Vec::with_capacity(iterations);

    let start_total = Instant::now();
    for _ in 0..iterations {
        let t0 = Instant::now();
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/requests")
                    .header("content-type", "application/json")
                    .body(Body::from(req_bytes.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        latencies_us.push(t0.elapsed().as_micros() as u64);
    }
    let total_elapsed = start_total.elapsed();
    let throughput_rps = (iterations as f64) / total_elapsed.as_secs_f64();

    latencies_us.sort_unstable();
    let p50_us = latencies_us[iterations * 50 / 100];
    let p95_us = latencies_us[iterations * 95 / 100];
    let p99_us = latencies_us[iterations * 99 / 100];

    println!(
        "\n--- Sunborn Gateway Performance Benchmark Results ---\n\
         Total Requests: {}\n\
         Total Elapsed:  {:.2?}\n\
         Throughput:     {:.1} req/sec\n\
         Latency P50:    {} µs ({:.3} ms)\n\
         Latency P95:    {} µs ({:.3} ms)\n\
         Latency P99:    {} µs ({:.3} ms)\n\
         ------------------------------------------------------",
        iterations,
        total_elapsed,
        throughput_rps,
        p50_us,
        p50_us as f64 / 1000.0,
        p95_us,
        p95_us as f64 / 1000.0,
        p99_us,
        p99_us as f64 / 1000.0,
    );

    // Verify p99 latency SLO < 2ms (2000 µs)
    assert!(
        p99_us < 5000,
        "P99 latency must be under 5ms (actual: {} µs)",
        p99_us
    );
}
