use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use sunborn_cache::CacheEntry;
use sunborn_core::{
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, ResponseEnvelope, SunbornError,
    TenantId,
};
use sunborn_execution::ExecutionCacheStore;
use sunborn_identity::IdentityEngine;
use sunborn_invalidation::InvalidationEvent;
use sunborn_semantic::{
    DeterministicEmbeddingProvider, EmbeddingProvider, VectorEntry, VectorStore,
};
use uuid::Uuid;

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static EXACT_HITS: AtomicU64 = AtomicU64::new(0);
static SEMANTIC_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "sunborn-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now()
    }))
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match state.cache.exists("__readiness_probe__").await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "cache": "connected",
                "semantic_engine": "ready",
                "invalidation_bus": "ready"
            })),
        ),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unavailable",
                "error": err.to_string()
            })),
        ),
    }
}

pub async fn metrics() -> impl IntoResponse {
    let requests = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let exact = EXACT_HITS.load(Ordering::Relaxed);
    let semantic = SEMANTIC_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);

    let output = format!(
        "# HELP sunborn_requests_total Total number of processed AI requests\n\
         # TYPE sunborn_requests_total counter\n\
         sunborn_requests_total {}\n\
         # HELP sunborn_cache_hits_total Total cache hits partitioned by tier\n\
         # TYPE sunborn_cache_hits_total counter\n\
         sunborn_cache_hits_total{{tier=\"exact\"}} {}\n\
         sunborn_cache_hits_total{{tier=\"semantic\"}} {}\n\
         # HELP sunborn_cache_misses_total Total cache misses\n\
         # TYPE sunborn_cache_misses_total counter\n\
         sunborn_cache_misses_total {}\n",
        requests, exact, semantic, misses
    );

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        output,
    )
}

pub async fn execute_request(
    State(state): State<AppState>,
    Json(req): Json<RequestEnvelope>,
) -> Result<Json<ResponseEnvelope>, AppError> {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();

    // 1. Validate payload
    if req.prompt.trim().is_empty() {
        return Err(SunbornError::invalid_payload("Prompt must not be empty").into());
    }

    // 2. Compute deterministic Cache Identity (C1)
    let identity = IdentityEngine::compute_identity(&req);
    tracing::info!(
        request_id = %req.request_id.0,
        tenant = %req.tenant_id.0,
        cache_key = %identity.key,
        "Evaluating multi-tier cache pipeline"
    );

    // 3. Exact Cache Lookup (C0 / C1)
    if state.config.cache.exact_cache_enabled {
        if let Ok(Some(cached_entry)) = state.cache.get(&identity.key).await {
            EXACT_HITS.fetch_add(1, Ordering::Relaxed);
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::info!(
                request_id = %req.request_id.0,
                cache_key = %identity.key,
                "Exact cache HIT (C0/C1)"
            );

            return Ok(Json(ResponseEnvelope {
                request_id: req.request_id,
                tenant_id: req.tenant_id,
                application_id: req.application_id,
                decision: DecisionRecord {
                    action: DecisionAction::SafeHit,
                    cache_path: CachePath::Exact,
                    confidence: 1.0,
                    risk_score: 0.0,
                    reason_code: "EXACT_CANONICAL_MATCH".to_string(),
                    latency_saved_ms: Some(150),
                },
                content: cached_entry.content,
                cached: true,
                cache_key: Some(identity.key),
                created_at: Utc::now(),
                duration_ms,
            }));
        }
    }

    // 4. Semantic Cache Candidate Evaluation (C2)
    if state.config.cache.semantic_cache_enabled {
        if let Ok(outcome) = state.decision_engine.evaluate(&req).await {
            if outcome.decision.action == DecisionAction::SafeHit {
                if let Some(content) = outcome.matched_content {
                    SEMANTIC_HITS.fetch_add(1, Ordering::Relaxed);
                    let duration_ms = start.elapsed().as_millis() as u64;
                    tracing::info!(
                        request_id = %req.request_id.0,
                        confidence = outcome.decision.confidence,
                        "Semantic cache HIT (C2)"
                    );

                    return Ok(Json(ResponseEnvelope {
                        request_id: req.request_id,
                        tenant_id: req.tenant_id,
                        application_id: req.application_id,
                        decision: outcome.decision,
                        content,
                        cached: true,
                        cache_key: outcome.matched_id,
                        created_at: Utc::now(),
                        duration_ms,
                    }));
                }
            }
        }
    }

    // 5. Cache MISS -> Origin Execution
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        request_id = %req.request_id.0,
        "Cache MISS -> Forwarding to origin provider"
    );

    let origin_content = format!(
        "[Origin Response from {} for tenant {}]: {}",
        req.model, req.tenant_id.0, req.prompt
    );

    // 6. Cache Admission Engine:
    // a) Save in exact cache
    let entry = CacheEntry::new(
        &identity.key,
        &origin_content,
        Some(state.config.cache.default_ttl_secs),
    );
    let _ = state.cache.set(entry).await;

    // b) Admit into semantic vector store
    let embedder = DeterministicEmbeddingProvider::new();
    if let Ok(vector) = embedder.embed(&req.prompt).await {
        let _ = state
            .vector_store
            .insert(VectorEntry {
                id: identity.key.clone(),
                tenant_id: req.tenant_id.clone(),
                application_id: req.application_id.clone(),
                prompt: req.prompt.clone(),
                content: origin_content.clone(),
                vector,
                metadata: serde_json::json!({ "model": req.model }),
                created_at: Utc::now(),
            })
            .await;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(Json(ResponseEnvelope {
        request_id: req.request_id,
        tenant_id: req.tenant_id,
        application_id: req.application_id,
        decision: DecisionRecord {
            action: DecisionAction::OriginExecute,
            cache_path: CachePath::Miss,
            confidence: 1.0,
            risk_score: 0.0,
            reason_code: "NO_CACHE_ENTRY_FOUND".to_string(),
            latency_saved_ms: None,
        },
        content: origin_content,
        cached: false,
        cache_key: Some(identity.key),
        created_at: Utc::now(),
        duration_ms,
    }))
}

#[derive(Deserialize)]
pub struct InvalidationRequest {
    pub source_id: Uuid,
    pub tenant_id: String,
    pub version_tag: String,
}

pub async fn trigger_invalidation(
    State(state): State<AppState>,
    Json(payload): Json<InvalidationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let event = InvalidationEvent {
        id: Uuid::new_v4(),
        tenant_id: TenantId::new(payload.tenant_id),
        source_id: payload.source_id,
        version_tag: payload.version_tag,
        timestamp: Utc::now(),
    };

    state
        .invalidation_engine
        .publish(event)
        .await
        .map_err(AppError::from)?;

    let evicted_count = state
        .invalidation_engine
        .process_next()
        .await
        .map_err(AppError::from)?
        .unwrap_or(0);

    Ok(Json(json!({
        "status": "processed",
        "source_id": payload.source_id,
        "evicted_cache_count": evicted_count
    })))
}

#[derive(Deserialize)]
pub struct DependencyRecordRequest {
    pub source_id: Uuid,
    pub cache_key: String,
}

pub async fn record_dependency(
    State(state): State<AppState>,
    Json(payload): Json<DependencyRecordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .dependency_graph
        .record_dependency(payload.source_id, &payload.cache_key)
        .await
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "recorded",
        "source_id": payload.source_id,
        "cache_key": payload.cache_key
    })))
}

pub async fn get_cache(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<CacheEntry>, AppError> {
    match state.cache.get(&key).await {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(SunbornError::new(
            sunborn_core::ErrorCode::CacheMiss,
            format!("Cache key '{}' not found", key),
        )
        .into()),
        Err(err) => Err(err.into()),
    }
}

pub async fn delete_cache(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = state.cache.delete(&key).await.map_err(AppError::from)?;
    let _ = state.vector_store.delete(&key).await;
    Ok(Json(json!({ "key": key, "deleted": deleted })))
}

#[derive(Deserialize)]
pub struct ExecutionPlanRequest {
    pub tenant_id: String,
    pub application_id: String,
    pub plan: sunborn_execution::ExecutionPlan,
}

pub async fn plan_execution(
    State(state): State<AppState>,
    Json(payload): Json<ExecutionPlanRequest>,
) -> Result<Json<sunborn_execution::PlanEvaluation>, AppError> {
    let tenant = TenantId::new(payload.tenant_id);
    let app = sunborn_core::model::ApplicationId::new(payload.application_id);
    let eval = sunborn_execution::ExecutionPlanner::plan_reuse(
        &tenant,
        &app,
        &payload.plan,
        &*state.execution_store,
    )
    .await
    .map_err(AppError::from)?;

    Ok(Json(eval))
}

#[derive(Deserialize)]
pub struct RecordToolResultRequest {
    pub tenant_id: String,
    pub application_id: String,
    pub tool_name: String,
    pub tool_version: String,
    pub arguments: serde_json::Value,
    pub context_hash: Option<String>,
    pub output: serde_json::Value,
    pub is_error: bool,
    pub execution_latency_ms: u64,
    pub ttl_secs: Option<u64>,
}

pub async fn record_tool_result(
    State(state): State<AppState>,
    Json(payload): Json<RecordToolResultRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = TenantId::new(payload.tenant_id);
    let app = sunborn_core::model::ApplicationId::new(payload.application_id);
    let sig = sunborn_execution::ToolSignatureEngine::compute_signature(
        &tenant,
        &app,
        &payload.tool_name,
        &payload.tool_version,
        &payload.arguments,
        payload.context_hash.as_deref(),
    )
    .map_err(AppError::from)?;

    let result = sunborn_execution::ToolResult {
        tool_name: payload.tool_name,
        signature: sig.clone(),
        output: payload.output,
        is_error: payload.is_error,
        execution_latency_ms: payload.execution_latency_ms,
        created_at: Utc::now(),
    };

    state
        .execution_store
        .set(&tenant, &sig, &result, payload.ttl_secs)
        .await
        .map_err(AppError::from)?;

    Ok(Json(json!({ "signature": sig, "status": "stored" })))
}

#[derive(Deserialize)]
pub struct EvictToolRequest {
    pub tenant_id: String,
    pub tool_name: Option<String>,
    pub signature: Option<String>,
}

pub async fn evict_execution(
    State(state): State<AppState>,
    Json(payload): Json<EvictToolRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = TenantId::new(payload.tenant_id);
    if let Some(sig) = payload.signature {
        let evicted = state
            .execution_store
            .evict(&tenant, &sig)
            .await
            .map_err(AppError::from)?;
        return Ok(Json(json!({ "evicted": evicted })));
    }

    if let Some(tool) = payload.tool_name {
        let count = state
            .execution_store
            .evict_by_tool(&tenant, &tool)
            .await
            .map_err(AppError::from)?;
        return Ok(Json(json!({ "evicted_count": count })));
    }

    Err(SunbornError::invalid_payload("Must provide either tool_name or signature").into())
}

pub async fn shadow_evaluate(
    State(state): State<AppState>,
    Json(payload): Json<crate::shadow::ShadowEvaluationRequest>,
) -> Result<Json<crate::shadow::ShadowEvaluationResult>, AppError> {
    let result = state
        .shadow_evaluator
        .evaluate_drift(&payload, &state.cache, &state.vector_store)
        .await
        .map_err(AppError::from)?;
    Ok(Json(result))
}

pub struct AppError(SunbornError);

impl From<SunbornError> for AppError {
    fn from(err: SunbornError) -> Self {
        Self(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self.0.code {
            sunborn_core::ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            sunborn_core::ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            sunborn_core::ErrorCode::InvalidPayload => StatusCode::BAD_REQUEST,
            sunborn_core::ErrorCode::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            sunborn_core::ErrorCode::CacheMiss => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(json!({ "error": self.0 }))).into_response()
    }
}
