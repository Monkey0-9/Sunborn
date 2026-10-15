pub mod handlers;
pub mod origin;
pub mod routing;
pub mod shadow;
pub mod state;

use axum::{
    routing::{get, post},
    Router,
};
pub use state::AppState;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        .route("/v1/requests", post(handlers::execute_request))
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
        .route("/v1/finops/report", get(handlers::get_finops_report))
        .route("/v1/routing/radix/inspect", get(handlers::radix_inspect))
        .route("/v1/routing/radix/route", post(handlers::radix_route))
        .route("/v1/lifecycle/sweep", post(handlers::sweep_lifecycle))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
