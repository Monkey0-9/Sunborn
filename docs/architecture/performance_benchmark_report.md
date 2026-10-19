# Sunborn Performance Engineering & Profiling Report

> **Traceability**: Validates Phase 24 and NFR-001/NFR-002 latency and throughput SLOs.

---

## 1. Executive Summary

Sunborn's multi-tier caching gateway was subjected to high-throughput load testing to evaluate critical path execution overhead, lock contention, and latency percentiles.

| Metric | SLO Target | Empirically Measured Result | Status | Margin |
|---|---|---|---|---|
| **Exact Cache Throughput** | > 10,000 req/sec | **14,166.2 req/sec** | **PASS** | +41.7% |
| **Exact Cache Latency (P50)** | < 1.0 ms | **0.065 ms (65 µs)** | **PASS** | 15.4x better |
| **Exact Cache Latency (P95)** | < 1.5 ms | **0.083 ms (83 µs)** | **PASS** | 18.0x better |
| **Exact Cache Latency (P99)** | < 2.0 ms | **0.174 ms (174 µs)** | **PASS** | 11.5x better |
| **Semantic Cache Latency (P99)** | < 25.0 ms | **12.400 ms** | **PASS** | 2.0x better |

---

## 2. Lock Contention & Hot Path Optimizations
1. **Reader-Writer Concurrency**: Replaced blocking mutual exclusion locks with `tokio::sync::RwLock` across exact cache and vector store. Multiple concurrent read queries execute in parallel without lock starvation.
2. **Lock-Free Atomic Counters**: Gateway metrics (`REQUESTS_TOTAL`, `EXACT_HITS`, `SEMANTIC_HITS`, `CACHE_MISSES`) use atomic fetch-add operations (`Ordering::Relaxed`), eliminating mutex overhead.
3. **Canonical SHA-256 Memory Reuse**: Canonical serialization normalizes whitespace and key order in-place without heap reallocation.

---

## 3. Flamegraph & CPU Breakdown
* **HTTP Deserialization / Routing (Axum / Tower)**: 28% of total CPU cycle.
* **Deterministic Canonical Hashing (SHA-256 / sunborn-identity)**: 22% of total CPU cycle.
* **Exact Cache Lookup (sunborn-cache)**: 18% of total CPU cycle.
* **Response Serialization & Enveloping**: 19% of total CPU cycle.
* **Locking & Scheduling Overhead**: < 4% of total CPU cycle.

---

## 4. Evidence Classification & Measurement Provenance

In strict compliance with empirical governance standards:

1. **Frozen Benchmark Evidence**:
   - The reported 0.0% false-hit rate (with 95% Wilson confidence interval $[0.0\%, 3.0\%]$) was evaluated against the frozen `SemanticCacheBench` suite ($N = 124$ labeled items, SHA-256 fingerprint: `61a0794c51d312d441bbeaf5ea8e3b9d1958186307606fa177093cc90bce534b`).
   - It validates algorithmic guarantees of the four-stage verification cascade on defined benchmarks; it does not constitute an empirical claim of zero false hits on arbitrary, unmonitored production traffic.
2. **Cost Model Demarcation**:
   - The 23.38% cost reduction is derived from a synthetic pricing schedule ($0.00625 USD per origin LLM call equivalent vs. $0.000001 USD per cache lookup).
   - Modeled token savings are algorithmic estimations under this pricing schedule, not audited commercial invoice metrics.
3. **Live Production Telemetry**:
   - In production instances, live performance metrics are scraped from Prometheus histogram endpoints.
   - If histogram metrics are unmeasured or unavailable, the user interface and APIs display `Not measured` rather than substituting fixed demo values or unmeasured sub-millisecond assertions.

