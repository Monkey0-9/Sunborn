pub mod handlers;
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
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
