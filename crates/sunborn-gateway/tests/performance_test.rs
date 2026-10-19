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
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-key-perf",
        sunborn_core::TenantId::new("tenant-perf"),
        sunborn_core::ApplicationId::new("app-bench"),
        vec!["*".to_string()],
        false,
    );

    let req_payload = RequestEnvelope::new(
        "tenant-perf",
        "app-bench",
        "gpt-4o",
        "Benchmark prompt for high-throughput exact cache validation",
    );
    let req_bytes = serde_json::to_vec(&req_payload).unwrap();

    // 1. Stage 1: In-Process CPU Engine Overhead (Zero-Network L1 Baseline)
    let entry = sunborn_cache::CacheEntry::new(
        "perf-l1-key",
        "Benchmark prompt for high-throughput exact cache validation",
        Some(3600),
    );
    state.cache.set(entry).await.unwrap();

    let l1_iterations = 10000;
    let mut l1_latencies_us = Vec::with_capacity(l1_iterations);
    let l1_start = Instant::now();
    for _ in 0..l1_iterations {
        let t0 = Instant::now();
        let val = state.cache.get("perf-l1-key").await.unwrap();
        assert!(val.is_some());
        l1_latencies_us.push(t0.elapsed().as_micros() as u64);
    }
    let l1_elapsed = l1_start.elapsed();
    let l1_rps = (l1_iterations as f64) / l1_elapsed.as_secs_f64();
    l1_latencies_us.sort_unstable();
    let l1_p50 = l1_latencies_us[l1_iterations * 50 / 100];
    let l1_p95 = l1_latencies_us[l1_iterations * 95 / 100];
    let l1_p99 = l1_latencies_us[l1_iterations * 99 / 100];

    println!(
        "\n--- [Table I] In-Process CPU Engine Overhead (Zero-Network L1 Baseline) ---\n\
         Throughput:     {:.1} req/sec\n\
         Latency P50:    {} µs ({:.3} ms)\n\
         Latency P95:    {} µs ({:.3} ms)\n\
         Latency P99:    {} µs ({:.3} ms)\n\
         --------------------------------------------------------------------------",
        l1_rps,
        l1_p50,
        l1_p50 as f64 / 1000.0,
        l1_p95,
        l1_p95 as f64 / 1000.0,
        l1_p99,
        l1_p99 as f64 / 1000.0
    );

    // 2. Warmup request to populate cache
    let warmup_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-perf")
                .body(Body::from(req_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(warmup_res.status(), StatusCode::OK);
    let body = warmup_res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);

    // 3. Stage 2: Full Axum Gateway Stack (Auth, Routing, JCS Canonicalization, Cache)
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
                    .header("x-api-key", "test-key-perf")
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
        "\n--- [Table II] Full Axum Gateway Stack (Auth, Routing, JCS Canonicalization, Cache) ---\n\
         Total Requests: {}\n\
         Total Elapsed:  {:.2?}\n\
         Throughput:     {:.1} req/sec\n\
         Latency P50:    {} µs ({:.3} ms)\n\
         Latency P95:    {} µs ({:.3} ms)\n\
         Latency P99:    {} µs ({:.3} ms)\n\
         --------------------------------------------------------------------------------------",
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

    // Verify p99 latency SLO < 5ms (5000 µs)
    assert!(
        p99_us < 5000,
        "P99 latency must be under 5ms (actual: {} µs)",
        p99_us
    );
}
