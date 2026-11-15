# Site Reliability Engineering (SRE) & Observability Audit

**Metrics Standard:** Prometheus Exposition Format (`/v1/metrics`)  
**Tracing Engine:** OpenTelemetry Distributed Context Propagation (`traceparent`)  
**Health Probes:** Kubernetes Liveness (`/v1/health/liveness`) & Readiness (`/v1/health/readiness`)  
**SLO Target:** 99.95% Availability, P99 Gateway Latency $< 15\,	ext{ms}$  

---

## 1. Prometheus Telemetry Architecture

The `/v1/metrics` endpoint exposes real-time operational indicators:
- `sunborn_exact_cache_hits_total`: Counter of L1 in-memory exact hits.
- `sunborn_semantic_cache_hits_total`: Counter of verified semantic reuse hits.
- `sunborn_cache_misses_total`: Counter of origin forwardings.
- `sunborn_gateway_request_duration_seconds`: Histogram of end-to-end response times.
- `sunborn_tokens_avoided_total`: Cumulative count of avoided LLM tokens.
- `sunborn_invalidation_events_total`: Total count of orchestrated invalidation events.

---

## 2. Operational Health Probes

```rust
// Hardened readiness probe in lib.rs
pub async fn readiness_check(state: &AppState) -> Result<()> {
    // 1. Verify PostgreSQL connection pool responds
    state.db.ping().await?;
    // 2. Verify L1 cache is accepting writes
    state.cache.ping().await?;
    // 3. Verify cryptographic signing key is loaded in memory
    if state.key_ring.is_empty() {
        return Err(SunbornError::internal("Signing key ring uninitialized"));
    }
    Ok(())
}
```
