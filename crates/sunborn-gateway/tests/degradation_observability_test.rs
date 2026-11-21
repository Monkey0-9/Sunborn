//! Failure-mode tests for the semantic tier.
//!
//! The gateway is designed to degrade rather than fail: if embeddings or the
//! vector store are unavailable, a request falls through to the origin instead
//! of returning 5xx. That trade-off is correct for availability but dangerous
//! for operations, because a fully degraded cache tier looks healthy from the
//! outside. These tests pin both halves: the request still succeeds, and the
//! degradation is counted.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_gateway::{create_router, handlers, AppState};
use sunborn_semantic::{DecisionConfig, DecisionEngine, EmbeddingProvider, Vector};
use tower::ServiceExt;

/// An embedding backend that is always down, standing in for an unreachable
/// model server or a Qdrant timeout.
struct UnavailableEmbedder;

#[async_trait::async_trait]
impl EmbeddingProvider for UnavailableEmbedder {
    async fn embed(&self, _text: &str) -> sunborn_core::Result<Vector> {
        Err(sunborn_core::SunbornError::internal(
            "embedding backend unavailable (injected)",
        ))
    }

    fn is_deterministic(&self) -> bool {
        false
    }
}

fn request_body(prompt: &str) -> Vec<u8> {
    serde_json::to_vec(&RequestEnvelope::new(
        "tenant-degraded",
        "app-degraded",
        "gpt-4o",
        prompt,
    ))
    .expect("serialize request")
}

#[tokio::test]
async fn semantic_outage_degrades_to_origin_and_is_counted() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);

    // Replace only the embedder: the vector store and everything else stay real
    // so the failure travels the production path.
    state.decision_engine = Arc::new(DecisionEngine::new(
        Arc::new(UnavailableEmbedder) as Arc<dyn EmbeddingProvider>,
        state.vector_store.clone(),
        DecisionConfig::default(),
    ));

    let app = create_router(state.clone());
    state.api_key_registry.register_key(
        "test-key-degraded",
        sunborn_core::TenantId::new("tenant-degraded"),
        sunborn_core::ApplicationId::new("app-degraded"),
        vec!["*".to_string()],
        true,
    );

    let failures_before = handlers::semantic_eval_failures();

    // A prompt that is not in the exact cache, so the semantic tier is reached.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-degraded")
                .body(Body::from(request_body(
                    "Summarise the quarterly retention report for the board",
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a semantic-tier outage must not fail the request"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(
        !resp.cached,
        "degraded request cannot be served as a semantic hit"
    );
    assert_eq!(
        resp.decision.action,
        sunborn_core::DecisionAction::OriginExecute
    );

    assert!(
        handlers::semantic_eval_failures() > failures_before,
        "the degradation must increment sunborn_semantic_eval_failures_total"
    );

    // And it must be visible on the scrape endpoint, not just in the counter.
    let metrics = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    let text = String::from_utf8(
        metrics
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(
        text.contains("sunborn_semantic_eval_failures_total"),
        "metric family missing from /metrics"
    );
    let value = text
        .lines()
        .find(|l| l.starts_with("sunborn_semantic_eval_failures_total{") && !l.starts_with('#'))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse::<u64>().ok())
        .expect("counter sample present");
    assert!(value >= 1, "counter should be at least 1, got {value}");
}

#[tokio::test]
async fn exact_cache_still_serves_when_semantic_tier_is_down() {
    let config = AppConfig::default();
    let mut state = AppState::new(config);
    let app = create_router(state.clone());
    state.api_key_registry.register_key(
        "test-key-exact",
        sunborn_core::TenantId::new("tenant-exact"),
        sunborn_core::ApplicationId::new("app-exact"),
        vec!["*".to_string()],
        true,
    );

    let payload = serde_json::to_vec(&RequestEnvelope::new(
        "tenant-exact",
        "app-exact",
        "gpt-4o",
        "Repeatable prompt for exact cache coverage",
    ))
    .unwrap();

    let send = |app: axum::Router, body: Vec<u8>| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-exact")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
    };

    let first = send(app.clone(), payload.clone()).await;
    assert_eq!(first.status(), StatusCode::OK);

    // Now break the embedder. The exact-cache hit must not depend on it.
    state.decision_engine = Arc::new(DecisionEngine::new(
        Arc::new(UnavailableEmbedder) as Arc<dyn EmbeddingProvider>,
        state.vector_store.clone(),
        DecisionConfig::default(),
    ));
    // Rebuild the router so it observes the replaced engine (Arc swap needs a
    // fresh router because handlers clone the Arc at construction time).
    let app = create_router(state.clone());

    let second = send(app, payload).await;
    assert_eq!(second.status(), StatusCode::OK);
    let body = second.into_body().collect().await.unwrap().to_bytes();
    let resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(resp.cached, "exact-cache hit must survive embedder outage");
}

/// Auth is decided from the raw request path. If a non-canonical spelling of a
/// protected route slipped past the middleware, it would reach a handler
/// without a key. None of these spellings may return success.
#[tokio::test]
async fn non_canonical_paths_cannot_bypass_auth() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let body = serde_json::to_vec(&RequestEnvelope::new(
        "tenant-bypass",
        "app-bypass",
        "gpt-4o",
        "attempted auth bypass",
    ))
    .unwrap();

    for path in [
        "//v1/requests",
        "/v1//requests",
        "/v1/auth/../v1/requests",
        "/v1/Auth/login",
        "/./v1/requests",
        "/v1/requests/",
        "/v1/requests?x=1",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            !res.status().is_success(),
            "unauthenticated request to {path} returned {} — possible auth bypass",
            res.status()
        );
        // Nothing leaked either: an error body must not carry a response payload.
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
            assert!(
                json.get("result").is_none(),
                "unauthenticated response for {path} contained a result payload"
            );
        }
    }
}
