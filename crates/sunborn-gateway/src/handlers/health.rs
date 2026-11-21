//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

/// Build identity, exposed by `/health` and `/version`.
fn build_metadata() -> serde_json::Value {
    json!({
        "service": "sunborn-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "git_sha": option_env!("SUNBORN_GIT_SHA").unwrap_or("unknown"),
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "rustc": option_env!("SUNBORN_RUSTC_VERSION").unwrap_or("unknown"),
    })
}

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "sunborn-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now()
    }))
}

/// Machine-readable deployment identity. Public by design: it exposes build
/// metadata only, never configuration or secrets.
pub async fn version() -> impl IntoResponse {
    let mut body = build_metadata();
    body["timestamp"] = json!(Utc::now());
    Json(body)
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    // Each dependency is bounded. A hung backend must surface as "degraded"
    // within the kubelet's probe timeout, not stall the probe indefinitely.
    const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

    let cache_res =
        tokio::time::timeout(PROBE_TIMEOUT, state.cache.exists("__readiness_probe__")).await;
    let db_res =
        tokio::time::timeout(PROBE_TIMEOUT, state.db.get_tenant("__readiness_probe__")).await;
    let vector_res = tokio::time::timeout(
        PROBE_TIMEOUT,
        state
            .vector_store
            .count(&TenantId::new("__readiness_probe__")),
    )
    .await;
    let llm_res = tokio::time::timeout(PROBE_TIMEOUT, state.origin_provider.health_check()).await;
    let embed_res =
        tokio::time::timeout(PROBE_TIMEOUT, state.selected_embedder.health_check()).await;

    let is_prod = sunborn_core::is_production_mode();

    let cache_ok = matches!(cache_res, Ok(Ok(_)));
    let db_ok = matches!(db_res, Ok(Ok(_)));
    let vector_ok = matches!(vector_res, Ok(Ok(_)));
    let llm_ok = matches!(llm_res, Ok(Ok(()))) && (!is_prod || !state.origin_provider.is_mock());
    let embed_ok = matches!(embed_res, Ok(Ok(())))
        && (!is_prod || !state.selected_embedder.is_deterministic());

    let all_ok = cache_ok && db_ok && vector_ok && llm_ok && embed_ok;

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let report = json!({
        "status": if all_ok { "ready" } else { "degraded" },
        "components": {
            "cache": {
                "status": if cache_ok { "connected" } else { "unavailable" },
                "error": if cache_ok { None } else { Some("cache_unavailable_or_timed_out") }
            },
            "database": {
                "status": if db_ok { "connected" } else { "unavailable" },
                "error": if db_ok { None } else { Some("database_unavailable_or_timed_out") }
            },
            "vector_store": {
                "status": if vector_ok { "connected" } else { "unavailable" },
                "error": if vector_ok { None } else { Some("vector_store_unavailable_or_timed_out") }
            },
            "llm_origin": {
                "status": if llm_ok { "connected" } else { "unavailable" },
                "is_mock": state.origin_provider.is_mock(),
                "error": if llm_ok { None } else { Some("llm_origin_unavailable_or_timed_out") }
            },
            "embedding_provider": {
                "status": if embed_ok { "connected" } else { "unavailable" },
                "is_deterministic": state.selected_embedder.is_deterministic(),
                "error": if embed_ok { None } else { Some("embedding_provider_unavailable_or_timed_out") }
            },
            "semantic_engine": {
                "status": if vector_ok && embed_ok { "ready" } else { "degraded" }
            },
            "invalidation_bus": {
                "status": if cache_ok && vector_ok { "ready" } else { "degraded" }
            },
            "execution_cache": {
                "status": "ready",
                "storage_tier": state.execution_store.storage_tier()
            },
            "learning_store": {
                "status": "ready",
                "storage_tier": state.learning_store.storage_tier()
            }
        }
    });

    (status_code, Json(report))
}

pub async fn metrics() -> impl IntoResponse {
    let requests = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let exact = EXACT_HITS.load(Ordering::Relaxed);
    let semantic = SEMANTIC_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let rate_limited = RATE_LIMITED_TOTAL.load(Ordering::Relaxed);
    let spend_rejected = SPEND_VELOCITY_REJECTIONS_TOTAL.load(Ordering::Relaxed);
    let quota_rejected = QUOTA_REJECTIONS_TOTAL.load(Ordering::Relaxed);
    let duration_ms = REQUEST_DURATION_MS_SUM.load(Ordering::Relaxed);
    let observations = REQUEST_OBSERVATIONS.load(Ordering::Relaxed);
    let uptime_seconds = process_uptime_seconds();

    // Render cumulative histogram buckets. Counters are already cumulative
    // because each observation increments every bucket at or above its value.
    let mut histogram = String::new();
    for (idx, bound) in LATENCY_BUCKETS_SECS.iter().enumerate() {
        let count = LATENCY_BUCKET_COUNTS[idx].load(Ordering::Relaxed);
        histogram.push_str(&format!(
            "sunborn_request_duration_seconds_bucket{{scope=\"process_local\",le=\"{bound}\"}} {count}\n"
        ));
    }
    histogram.push_str(&format!(
        "sunborn_request_duration_seconds_bucket{{scope=\"process_local\",le=\"+Inf\"}} {observations}\n"
    ));
    histogram.push_str(&format!(
        "sunborn_request_duration_seconds_count{{scope=\"process_local\"}} {observations}\n"
    ));
    histogram.push_str(&format!(
        "sunborn_request_duration_seconds_sum{{scope=\"process_local\"}} {:.6}\n",
        (duration_ms as f64) / 1000.0
    ));

    let build = build_metadata();
    let version = build["version"].as_str().unwrap_or("unknown");
    let git_sha = build["git_sha"].as_str().unwrap_or("unknown");

    let output = format!(
        "# HELP sunborn_requests_total Total number of processed AI requests\n\
         # TYPE sunborn_requests_total counter\n\
         sunborn_requests_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_cache_hits_total Total cache hits partitioned by tier\n\
         # TYPE sunborn_cache_hits_total counter\n\
         sunborn_cache_hits_total{{scope=\"process_local\",tier=\"exact\"}} {}\n\
         sunborn_cache_hits_total{{scope=\"process_local\",tier=\"semantic\"}} {}\n\
         # HELP sunborn_cache_misses_total Total cache misses\n\
         # TYPE sunborn_cache_misses_total counter\n\
         sunborn_cache_misses_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_rate_limit_exceeded_total Total rate limited and quota exceeded requests\n\
         # TYPE sunborn_rate_limit_exceeded_total counter\n\
         sunborn_rate_limit_exceeded_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_spend_velocity_exceeded_total Total requests rejected due to spend velocity budget limits\n\
         # TYPE sunborn_spend_velocity_exceeded_total counter\n\
         sunborn_spend_velocity_exceeded_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_quota_exceeded_total Total requests rejected by daily/monthly or free-pool quotas\n\
         # TYPE sunborn_quota_exceeded_total counter\n\
         sunborn_quota_exceeded_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_telemetry_source Telemetry origin scope indicator\n\
         # TYPE sunborn_telemetry_source gauge\n\
         sunborn_telemetry_source{{cluster=\"gateway-primary\",source=\"process_local_worker\"}} 1\n",
        requests,
        exact,
        semantic,
        misses,
        rate_limited,
        spend_rejected,
        quota_rejected
    );

    let output = format!(
        "{output}\
         # HELP sunborn_request_duration_seconds Histogram of end-to-end request latency\n\
         # TYPE sunborn_request_duration_seconds histogram\n\
         {histogram}\
         # HELP sunborn_uptime_seconds Process uptime in seconds\n\
         # TYPE sunborn_uptime_seconds gauge\n\
         sunborn_uptime_seconds{{scope=\"process_local\"}} {uptime_seconds:.3}\n\
         # HELP sunborn_build_info Build identity of the running gateway\n\
         # TYPE sunborn_build_info gauge\n\
         sunborn_build_info{{version=\"{version}\",git_sha=\"{git_sha}\",profile=\"{profile}\"}} 1\n",
        profile = build["build_profile"].as_str().unwrap_or("unknown")
    );

    (
        StatusCode::OK,
        [
            ("content-type", "text/plain; version=0.0.4"),
            (
                "cache-control",
                "public, max-age=5, stale-while-revalidate=30",
            ),
        ],
        output,
    )
}
