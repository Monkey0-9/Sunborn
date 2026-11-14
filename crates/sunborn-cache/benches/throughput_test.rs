use std::time::Instant;
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};

#[tokio::main]
async fn main() {
    println!("=== Sunborn Cache In-Memory Engine L1 Throughput Benchmark ===");
    let cache = MemoryExactCache::new();
    let entry = CacheEntry::new(
        "bench-key",
        "Deterministic benchmark payload for high-throughput in-process exact cache evaluation",
        Some(3600),
    );
    cache.set(entry).await.expect("Cache pre-population failed");

    let iterations = 100_000;
    let mut latencies_us = Vec::with_capacity(iterations);

    let start = Instant::now();
    for _ in 0..iterations {
        let t0 = Instant::now();
        let val = cache.get("bench-key").await.expect("Lookup error");
        assert!(val.is_some());
        latencies_us.push(t0.elapsed().as_nanos() as u64);
    }
    let elapsed = start.elapsed();
    let rps = (iterations as f64) / elapsed.as_secs_f64();

    latencies_us.sort_unstable();
    let p50_ns = latencies_us[iterations * 50 / 100];
    let p95_ns = latencies_us[iterations * 95 / 100];
    let p99_ns = latencies_us[iterations * 99 / 100];

    println!("Total Operations : {}", iterations);
    println!("Total Elapsed    : {:.3}s", elapsed.as_secs_f64());
    println!("Throughput       : {:.1} req/sec", rps);
    println!("Latency P50      : {:.3} µs", p50_ns as f64 / 1000.0);
    println!("Latency P95      : {:.3} µs", p95_ns as f64 / 1000.0);
    println!("Latency P99      : {:.3} µs", p99_ns as f64 / 1000.0);
    println!("---------------------------------------------------------------");
    println!("Benchmark Completed Successfully.");
}
