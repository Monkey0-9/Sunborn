//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "sunborn-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now()
    }))
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let cache_res = state.cache.exists("__readiness_probe__").await;
    let db_res = state.db.get_tenant("__readiness_probe__").await;
    let vector_res = state
        .vector_store
        .count(&TenantId::new("__readiness_probe__"))
        .await;
    let llm_res = state.origin_provider.health_check().await;
    let embed_res = state.selected_embedder.health_check().await;

    let is_prod = sunborn_core::is_production_mode();

    let cache_ok = cache_res.is_ok();
    let db_ok = db_res.is_ok();
    let vector_ok = vector_res.is_ok();
    let llm_ok = llm_res.is_ok() && (!is_prod || !state.origin_provider.is_mock());
    let embed_ok = embed_res.is_ok() && (!is_prod || !state.selected_embedder.is_deterministic());

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
                "error": cache_res.err().map(|_| "cache_unavailable".to_string())
            },
            "database": {
                "status": if db_ok { "connected" } else { "unavailable" },
                "error": db_res.err().map(|_| "database_unavailable".to_string())
            },
            "vector_store": {
                "status": if vector_ok { "connected" } else { "unavailable" },
                "error": vector_res.err().map(|e| e.to_string())
            },
            "llm_origin": {
                "status": if llm_ok { "connected" } else { "unavailable" },
                "is_mock": state.origin_provider.is_mock(),
                "error": llm_res.err().map(|e| e.to_string())
            },
            "embedding_provider": {
                "status": if embed_ok { "connected" } else { "unavailable" },
                "is_deterministic": state.selected_embedder.is_deterministic(),
                "error": embed_res.err().map(|e| e.to_string())
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
         # HELP sunborn_request_duration_seconds_total Total request processing duration in seconds\n
         # TYPE sunborn_request_duration_seconds_total counter\n\
         sunborn_request_duration_seconds_total{{scope=\"process_local\"}} {:.6}\n\
         # HELP sunborn_telemetry_source Telemetry origin scope indicator\n\
         # TYPE sunborn_telemetry_source gauge\n\
         sunborn_telemetry_source{{cluster=\"gateway-primary\",source=\"process_local_worker\"}} 1\n",
        requests,
        exact,
        semantic,
        misses,
        rate_limited,
        spend_rejected,
        quota_rejected,
        (duration_ms as f64) / 1000.0
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
