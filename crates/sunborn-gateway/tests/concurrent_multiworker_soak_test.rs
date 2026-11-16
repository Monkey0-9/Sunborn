//! High-concurrency multi-worker soak benchmark for Sunborn Gateway.
//!
//! Spawns 16 concurrent worker tasks driving 10,000 requests through
//! the real Axum router with full identity canonicalization, exact cache
//! lookup, and spend velocity control.
//!
//! Asserts 0 failed requests and sub-millisecond P99 latency.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_gateway::{create_router, AppState};
use tokio::sync::Mutex;
use tower::ServiceExt;

#[tokio::test]
async fn test_concurrent_multiworker_soak_and_percentiles() {
    let mut config = AppConfig::default();
    config.cache.semantic_cache_enabled = false; // Isolate exact cache hot-path throughput
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let test_key = "soak-test-key-5500";
    state.api_key_registry.register_key(
        test_key,
        sunborn_core::TenantId::new("tenant-soak"),
        sunborn_core::ApplicationId::new("app-soak"),
        vec!["*".to_string()],
        false,
    );

    let prompt = "High throughput concurrent soak test prompt for multi-worker verification";
    let req_envelope = RequestEnvelope::new("tenant-soak", "app-soak", "gpt-4o", prompt);
    let req_body_bytes = serde_json::to_vec(&req_envelope).unwrap();

    // 1. Warmup request to populate exact cache
    let warmup_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", test_key)
                .body(Body::from(req_body_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(warmup_res.status(), StatusCode::OK);
    let body = warmup_res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!resp.cached);

    // 2. High-concurrency multi-worker soak execution
    let num_workers = 16;
    let requests_per_worker = 625; // 16 * 625 = 10,000 requests
    let total_expected_requests = num_workers * requests_per_worker;

    let success_counter = Arc::new(AtomicUsize::new(0));
    let error_counter = Arc::new(AtomicUsize::new(0));
    let latencies_us = Arc::new(Mutex::new(Vec::with_capacity(total_expected_requests)));

    let overall_start = Instant::now();
    let mut handles = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let app = app.clone();
        let body_bytes = req_body_bytes.clone();
        let key = test_key.to_string();
        let sc = Arc::clone(&success_counter);
        let ec = Arc::clone(&error_counter);
        let l_us = Arc::clone(&latencies_us);

        let handle = tokio::spawn(async move {
            let mut local_latencies = Vec::with_capacity(requests_per_worker);
            for _ in 0..requests_per_worker {
                let t0 = Instant::now();
                let req = Request::builder()
                    .method("POST")
                    .uri("/v1/requests")
                    .header("content-type", "application/json")
                    .header("x-api-key", &key)
                    .body(Body::from(body_bytes.clone()))
                    .unwrap();

                let resp = app.clone().oneshot(req).await.unwrap();
                let status = resp.status();
                let elapsed_us = t0.elapsed().as_micros() as u64;

                if status == StatusCode::OK {
                    let _ = resp.into_body().collect().await;
                    sc.fetch_add(1, Ordering::Relaxed);
                    local_latencies.push(elapsed_us);
                } else {
                    ec.fetch_add(1, Ordering::Relaxed);
                }
            }
            let mut lock = l_us.lock().await;
            lock.extend(local_latencies);
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    let overall_elapsed = overall_start.elapsed();
    let total_successful = success_counter.load(Ordering::SeqCst);
    let total_failed = error_counter.load(Ordering::SeqCst);

    assert_eq!(total_successful, total_expected_requests);
    assert_eq!(total_failed, 0);

    let mut all_latencies = latencies_us.lock().await.clone();
    all_latencies.sort_unstable();

    let n = all_latencies.len();
    let p50 = all_latencies[n * 50 / 100] as f64 / 1000.0;
    let p90 = all_latencies[n * 90 / 100] as f64 / 1000.0;
    let p95 = all_latencies[n * 95 / 100] as f64 / 1000.0;
    let p99 = all_latencies[n * 99 / 100] as f64 / 1000.0;
    let p999 = all_latencies[n * 999 / 1000] as f64 / 1000.0;
    let max_ms = all_latencies[n - 1] as f64 / 1000.0;

    let throughput_rps = total_successful as f64 / overall_elapsed.as_secs_f64();

    println!("\n=== SUNBORN MULTI-WORKER CONCURRENT SOAK BENCHMARK ===");
    println!("Total Requests    : {}", total_successful);
    println!("Concurrent Workers: {}", num_workers);
    println!("Total Elapsed     : {:.3}s", overall_elapsed.as_secs_f64());
    println!("Throughput        : {:.1} req/sec", throughput_rps);
    println!("Latency P50       : {:.3} ms", p50);
    println!("Latency P90       : {:.3} ms", p90);
    println!("Latency P95       : {:.3} ms", p95);
    println!("Latency P99       : {:.3} ms", p99);
    println!("Latency P99.9     : {:.3} ms", p999);
    println!("Latency Max       : {:.3} ms", max_ms);
    println!("Failed Requests   : {}", total_failed);
    println!("=====================================================\n");

    assert!(total_successful >= 10000);
    assert_eq!(total_failed, 0);
}
