pub mod autonomic;
pub mod handlers;
pub mod middleware;
pub mod origin;
pub mod routing;
pub mod shadow;
pub mod state;

use axum::{
    http::{header, HeaderName, Method},
    routing::{get, post},
    Router,
};
pub use state::AppState;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    let allowed_origins: Vec<header::HeaderValue> =
        if let Ok(origins) = std::env::var("SUNBORN_CORS_ALLOWED_ORIGINS") {
            origins
                .split(',')
                .filter_map(|s| s.trim().parse::<header::HeaderValue>().ok())
                .collect()
        } else {
            vec![
                header::HeaderValue::from_static("http://localhost:3000"),
                header::HeaderValue::from_static("http://127.0.0.1:3000"),
            ]
        };

    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-tenant-id"),
            HeaderName::from_static("x-application-id"),
        ]);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        .route("/v1/requests", post(handlers::execute_request))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .route("/v1/invalidation", post(handlers::trigger_invalidation))
        .route("/v1/dependencies", post(handlers::record_dependency))
        .route(
            "/v1/caches/{*key}",
            get(handlers::get_cache).delete(handlers::delete_cache),
        )
        .route("/v1/execution/plan", post(handlers::plan_execution))
        .route("/v1/execution/results", post(handlers::record_tool_result))
        .route("/v1/execution/evict", post(handlers::evict_execution))
        .route("/v1/shadow/evaluate", post(handlers::shadow_evaluate))
        .route("/v1/chaos/status", get(handlers::chaos_status))
        .route("/v1/chaos/trip", post(handlers::chaos_trip))
        .route("/v1/chaos/reset", post(handlers::chaos_reset))
        .route("/v1/rebuild", post(handlers::trigger_rebuild))
        .route(
            "/v1/tenants/{id}",
            axum::routing::delete(handlers::delete_tenant),
        )
        .route("/v1/governance/shred", post(handlers::crypto_shred_tenant))
        .route(
            "/v1/governance/attest",
            post(handlers::generate_merkle_attestation),
        )
        .route(
            "/v1/governance/verify-proof",
            post(handlers::verify_merkle_attestation),
        )
        .route(
            "/v1/speculative/predict",
            post(handlers::predict_speculative),
        )
        .route(
            "/v1/cluster/sync/status",
            get(handlers::get_cluster_sync_status),
        )
        .route(
            "/v1/autonomic/incidents",
            get(handlers::get_autonomic_status),
        )
        .route("/v1/finops/report", get(handlers::get_finops_report))
        .route("/v1/routing/radix/inspect", get(handlers::radix_inspect))
        .route("/v1/routing/radix/route", post(handlers::radix_route))
        .route("/v1/lifecycle/sweep", post(handlers::sweep_lifecycle))
        .layer(axum::middleware::from_fn(middleware::auth::auth_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
