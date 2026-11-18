//! Route contract parity and authorization coverage.
//!
//! Two properties are enforced here:
//!
//! 1. Every route declared in `src/lib.rs` is listed in the contract table below,
//!    so a new endpoint cannot be added without explicitly classifying its
//!    authorization model.
//! 2. Every non-public route rejects unauthenticated callers, and every
//!    administrative route rejects a merely authenticated (non-admin) caller.
//!
//! The table is compared against the actual router source, not against
//! documentation, so it cannot silently drift.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sunborn_core::{AppConfig, ApplicationId, TenantId};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    /// Reachable without credentials by design.
    Public,
    /// Any valid API key.
    Tenant,
    /// Administrator key, or m-of-n administrator quorum.
    Admin,
}

struct Route {
    method: &'static str,
    path: &'static str,
    access: Access,
}

const ROUTES: &[Route] = &[
    // Probes and authentication entry points.
    Route {
        method: "GET",
        path: "/health",
        access: Access::Public,
    },
    Route {
        method: "GET",
        path: "/ready",
        access: Access::Public,
    },
    Route {
        method: "GET",
        path: "/metrics",
        access: Access::Public,
    },
    Route {
        method: "POST",
        path: "/v1/auth/login",
        access: Access::Public,
    },
    Route {
        method: "GET",
        path: "/v1/ceremony/public-keys",
        access: Access::Public,
    },
    Route {
        method: "GET",
        path: "/v1/auth/session",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/auth/credentials",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/auth/credentials/revoke",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/auth/credentials/rotate",
        access: Access::Admin,
    },
    // Inference and cache serving.
    Route {
        method: "POST",
        path: "/v1/requests",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/chat/completions",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/messages",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/caches/{*key}",
        access: Access::Admin,
    },
    Route {
        method: "DELETE",
        path: "/v1/caches/{*key}",
        access: Access::Admin,
    },
    // Invalidation and dependency graph.
    Route {
        method: "POST",
        path: "/v1/invalidation",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/dependencies",
        access: Access::Admin,
    },
    Route {
        method: "GET",
        path: "/v1/cache/graph/stats",
        access: Access::Admin,
    },
    Route {
        method: "GET",
        path: "/v1/cache/graph/lineage/{*key}",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/cache/graph/invalidate",
        access: Access::Admin,
    },
    // Execution graph.
    Route {
        method: "POST",
        path: "/v1/execution/plan",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/execution/results",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/execution/evict",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/shadow/evaluate",
        access: Access::Tenant,
    },
    // Operations and resilience controls.
    Route {
        method: "GET",
        path: "/v1/chaos/status",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/chaos/trip",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/chaos/reset",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/rebuild",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/lifecycle/sweep",
        access: Access::Admin,
    },
    Route {
        method: "GET",
        path: "/v1/cluster/status",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/cluster/sync/status",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/autonomic/status",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/autonomic/incidents",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/finops/report",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/security/audit",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/security/evaluate",
        access: Access::Tenant,
    },
    // Governance and tenant lifecycle.
    Route {
        method: "DELETE",
        path: "/v1/tenants/{id}",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/governance/provision",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/governance/shred",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/governance/attest",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/governance/verify-proof",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/governance/spend-ceiling",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/governance/tenant-quota",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/governance/free-pool",
        access: Access::Admin,
    },
    // Speculation and routing.
    Route {
        method: "POST",
        path: "/v1/speculative/predict",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/routing/radix/inspect",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/routing/radix/route",
        access: Access::Tenant,
    },
    // Learning controller.
    Route {
        method: "POST",
        path: "/v1/learning/feedback",
        access: Access::Tenant,
    },
    Route {
        method: "GET",
        path: "/v1/learning/policies",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/learning/evaluate",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/learning/calibrate",
        access: Access::Tenant,
    },
    Route {
        method: "POST",
        path: "/v1/learning/canary",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/learning/promote",
        access: Access::Admin,
    },
    Route {
        method: "POST",
        path: "/v1/learning/rollback",
        access: Access::Admin,
    },
];

/// Extract every literal path passed to `.route(...)` in the router source.
fn declared_paths() -> Vec<String> {
    const SRC: &str = include_str!("../src/lib.rs");
    let mut paths = Vec::new();
    let mut rest = SRC;
    while let Some(idx) = rest.find(".route(") {
        rest = &rest[idx + ".route(".len()..];
        let Some(start) = rest.find('"') else { break };
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        paths.push(after[..end].to_string());
    }
    paths.sort();
    paths.dedup();
    paths
}

fn concrete(path: &str) -> String {
    path.replace("{*key}", "probe-key")
        .replace("{key}", "probe-key")
        .replace("{id}", "tenant-probe")
}

fn app_state() -> AppState {
    let state = AppState::new(AppConfig::default());
    state.api_key_registry.register_key(
        "route-admin-key",
        TenantId::new("tenant-route-admin"),
        ApplicationId::new("app-route"),
        vec!["*".to_string()],
        true,
    );
    state.api_key_registry.register_key(
        "route-tenant-key",
        TenantId::new("tenant-route-user"),
        ApplicationId::new("app-route"),
        vec!["*".to_string()],
        false,
    );
    state
}

fn request(route: &Route, key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(route.method)
        .uri(concrete(route.path))
        .header("content-type", "application/json");
    if let Some(key) = key {
        builder = builder.header("x-api-key", key);
    }
    builder.body(Body::from("{}")).unwrap()
}

async fn status_for(route: &Route, key: Option<&str>) -> StatusCode {
    let response = create_router(app_state())
        .oneshot(request(route, key))
        .await
        .expect("router is infallible");
    response.status()
}

#[test]
fn every_router_path_is_classified_in_the_contract_table() {
    let declared = declared_paths();
    let mut classified: Vec<String> = ROUTES.iter().map(|r| r.path.to_string()).collect();
    classified.sort();
    classified.dedup();

    let missing: Vec<&String> = declared
        .iter()
        .filter(|p| !classified.contains(p))
        .collect();
    let stale: Vec<&String> = classified
        .iter()
        .filter(|p| !declared.contains(p))
        .collect();

    assert!(
        missing.is_empty(),
        "routes exist in the router but are missing from the authorization contract table: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "the authorization contract table lists paths the router no longer serves: {stale:?}"
    );
    assert_eq!(
        declared.len(),
        classified.len(),
        "every router path must appear in the contract table (paths served by several\nmethods, like /v1/caches/{{*key}}, get one row per method and are counted once)"
    );
}

#[tokio::test]
async fn unauthenticated_callers_are_rejected_everywhere_but_public_endpoints() {
    for route in ROUTES {
        let status = status_for(route, None).await;
        match route.access {
            Access::Public => assert!(
                !status.is_server_error(),
                "public endpoint {} {} should not fail with {status}",
                route.method,
                route.path
            ),
            Access::Tenant | Access::Admin => assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "{} {} must reject callers with no credentials, got {status}",
                route.method,
                route.path
            ),
        }
    }
}

#[tokio::test]
async fn authenticated_non_admin_callers_cannot_reach_administrative_routes() {
    for route in ROUTES.iter().filter(|r| r.access == Access::Admin) {
        let status = status_for(route, Some("route-tenant-key")).await;
        assert!(
            !status.is_success(),
            "{} {} must not succeed for a non-admin key, got {status}",
            route.method,
            route.path
        );
    }
}

#[tokio::test]
async fn a_valid_tenant_key_is_accepted_by_tenant_routes() {
    for route in ROUTES.iter().filter(|r| r.access == Access::Tenant) {
        let status = status_for(route, Some("route-tenant-key")).await;
        assert_ne!(
            status,
            StatusCode::UNAUTHORIZED,
            "{} {} rejected a valid tenant key",
            route.method,
            route.path
        );
        assert_ne!(
            status,
            StatusCode::FORBIDDEN,
            "{} {} must not require admin privileges",
            route.method,
            route.path
        );
    }
}
