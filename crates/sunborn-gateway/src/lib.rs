//! # Sunborn Gateway
//!
//! High-throughput asynchronous Axum HTTP reverse proxy, OpenAI/Anthropic protocol translator,
//! and multi-tenant semantic cache router for the Sunborn platform.
//!
//! ## Quick Start
//!
//! ```rust
//! # #[tokio::main]
//! # async fn main() {
//! use sunborn_gateway::{create_router, AppState};
//! use sunborn_core::AppConfig;
//!
//! let config = AppConfig::load_from_env();
//! let state = AppState::new(config);
//! let _router = create_router(state);
//! # }
//! ```

pub mod autonomic;
pub mod cache_lifecycle;
pub mod execution_runtime;
pub mod finops;
pub mod five_gates;
pub mod handlers;
pub mod learning;
pub mod limits;
pub mod middleware;
pub mod origin;
pub mod quota;
pub mod retrieval_cache;
pub mod routing;
pub mod shadow;
pub mod state;

use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::{
    http::{header, HeaderName, Method},
    routing::{get, post},
    Router,
};
pub use state::AppState;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Baseline security headers applied to every response.
///
/// The ingress in `deploy/k8s` sets a comparable set at the edge; this layer
/// guarantees the invariants hold for direct gateway traffic, local installs,
/// and any deployment that exposes the gateway without that ingress.
/// HSTS is only sent when the request arrived over TLS (directly or via a
/// trusted proxy reporting `x-forwarded-proto: https`), because advertising it
/// on a plain-HTTP listener would strand local development on a poisoned
/// host cache.
async fn security_headers(req: Request<axum::body::Body>, next: Next) -> Response {
    let is_https = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(|proto| proto.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
        || req.uri().scheme_str() == Some("https");

    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        header::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        header::HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        header::HeaderValue::from_static("same-site"),
    );
    if is_https {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            header::HeaderValue::from_static("max-age=63072000; includeSubDomains"),
        );
    }

    response
}

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
            HeaderName::from_static("x-request-id"),
        ]);

    let rate_limit_state = state.clone();

    Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        .route("/version", get(handlers::version))
        .route("/v1/auth/login", post(handlers::login))
        .route("/v1/auth/session", get(handlers::verify_session))
        .route(
            "/v1/auth/credentials",
            post(handlers::create_api_credential),
        )
        .route(
            "/v1/auth/credentials/revoke",
            post(handlers::revoke_api_credential),
        )
        .route(
            "/v1/auth/credentials/rotate",
            post(handlers::rotate_api_credential),
        )
        .route("/v1/requests", post(handlers::execute_request))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .route("/v1/messages", post(handlers::anthropic_messages))
        .route("/v1/invalidation", post(handlers::trigger_invalidation))
        .route("/v1/dependencies", post(handlers::record_dependency))
        .route("/v1/cache/graph/stats", get(handlers::get_graph_stats))
        .route(
            "/v1/cache/graph/lineage/{*key}",
            get(handlers::get_graph_lineage),
        )
        .route(
            "/v1/cache/graph/invalidate",
            post(handlers::invalidate_graph_rtc),
        )
        .route(
            "/v1/caches/{*key}",
            get(handlers::get_cache).delete(handlers::delete_cache),
        )
        // Multi-Relational Knowledge Graph & Causal Invalidation Routes
        .route("/v1/graph/nodes", post(handlers::add_graph_node))
        .route(
            "/v1/graph/nodes/{*id}",
            get(handlers::get_graph_node).delete(handlers::delete_graph_node),
        )
        .route("/v1/graph/edges", post(handlers::add_graph_edge))
        .route(
            "/v1/graph/blast-radius",
            post(handlers::calculate_graph_blast_radius),
        )
        .route(
            "/v1/graph/invalidate",
            post(handlers::invalidate_graph_causal),
        )
        .route("/v1/graph/explain/{*id}", get(handlers::explain_graph_node))
        .route(
            "/v1/graph/communities",
            get(handlers::get_graph_communities),
        )
        .route("/v1/graph/god-nodes", get(handlers::get_graph_god_nodes))
        .route(
            "/v1/graph/subgraph/{*id}",
            get(handlers::get_graph_subgraph),
        )
        .route("/v1/graph/topology", get(handlers::get_graph_topology))
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
        .route("/v1/governance/provision", post(handlers::provision_tenant))
        .route(
            "/v1/governance/spend-ceiling",
            post(handlers::set_spend_ceiling),
        )
        .route(
            "/v1/governance/tenant-quota",
            post(handlers::set_tenant_quota),
        )
        .route("/v1/governance/free-pool", post(handlers::set_free_pool))
        .route(
            "/v1/governance/attest",
            post(handlers::generate_merkle_attestation),
        )
        .route(
            "/v1/governance/verify-proof",
            post(handlers::verify_merkle_attestation),
        )
        // Sovereign Indian Governance & Compliance Routes
        .route(
            "/v1/governance/dpdp/consent",
            post(handlers::record_dpdp_consent),
        )
        .route(
            "/v1/governance/dpdp/erasure",
            post(handlers::execute_dpdp_erasure),
        )
        .route("/v1/governance/dpdp/audit", get(handlers::get_dpdp_audit))
        .route(
            "/v1/governance/cert-in/incidents",
            get(handlers::get_cert_in_incidents),
        )
        .route(
            "/v1/governance/rbi/localization",
            get(handlers::verify_rbi_data_localization),
        )
        .route(
            "/v1/ceremony/public-keys",
            get(handlers::ceremony_public_keys),
        )
        .route(
            "/v1/speculative/predict",
            post(handlers::predict_speculative),
        )
        .route(
            "/v1/cluster/sync/status",
            get(handlers::get_cluster_sync_status),
        )
        .route("/v1/cluster/status", get(handlers::get_cluster_sync_status))
        .route(
            "/v1/autonomic/incidents",
            get(handlers::get_autonomic_status),
        )
        .route("/v1/autonomic/status", get(handlers::get_autonomic_status))
        .route("/v1/finops/report", get(handlers::get_finops_report))
        .route("/v1/routing/radix/inspect", get(handlers::radix_inspect))
        .route("/v1/routing/radix/route", post(handlers::radix_route))
        .route("/v1/lifecycle/sweep", post(handlers::sweep_lifecycle))
        .route("/v1/lifecycle/retain", post(handlers::retain_lifecycle))
        .route(
            "/v1/lifecycle/outbox/replay",
            post(handlers::replay_outbox_dlq),
        )
        .route("/v1/retrieval/cache", post(handlers::rag_retrieval_cache))
        .route("/v1/security/evaluate", post(handlers::security_evaluate))
        .route("/v1/security/audit", get(handlers::security_audit))
        .route(
            "/v1/learning/feedback",
            post(handlers::record_learning_feedback),
        )
        .route(
            "/v1/learning/policies",
            get(handlers::get_learning_policies),
        )
        .route(
            "/v1/learning/evaluate",
            post(handlers::evaluate_learning_policy),
        )
        .route(
            "/v1/learning/calibrate",
            post(handlers::calibrate_learning_policy),
        )
        .route("/v1/learning/canary", post(handlers::start_learning_canary))
        .route(
            "/v1/learning/promote",
            post(handlers::promote_learning_policy),
        )
        .route(
            "/v1/learning/rollback",
            post(handlers::rollback_learning_policy),
        )
        .route("/v1/local/status", get(handlers::get_local_status))
        .route("/", get(handlers::get_local_cockpit_html))
        .route("/cockpit", get(handlers::get_local_cockpit_html))
        .route("/v1/local/providers", post(handlers::save_local_provider))
        .route(
            "/v1/local/providers/{name}",
            axum::routing::delete(handlers::delete_local_provider),
        )
        .route("/v1/local/config", post(handlers::configure_local_settings))
        .layer(axum::middleware::from_fn_with_state(
            rate_limit_state,
            middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn(middleware::auth::auth_middleware))
        // Must stay outside auth: it exists to charge requests that fail
        // authentication, which never reach the tenant limiter below. Axum
        // applies later layers outside earlier ones, so this placement is
        // load-bearing, not cosmetic.
        .layer(axum::middleware::from_fn(
            middleware::rate_limit::pre_auth_rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn(
            middleware::trace::request_id_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
        // Last layer applied is the outermost, so these headers also cover
        // CORS preflights and early 4xx/413 rejections.
        .layer(axum::middleware::from_fn(security_headers))
        .with_state(state)
}
