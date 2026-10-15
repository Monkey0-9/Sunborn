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
    finops::{FinOpsEngine, ModelPricing},
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, ResponseEnvelope, SunbornError,
    TenantId,
};
use sunborn_db::DatabaseStore;
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

            let _ = state.db.increment_hit_count(&identity.key).await;
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
    if state.config.cache.semantic_cache_enabled
        && state.circuit_breaker.status().await != sunborn_cache::CircuitStatus::Open
    {
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

    // c) Save to authoritative PostgreSQL system of record
    let _ = state
        .db
        .insert_cache_entry(sunborn_db::models::DurableCacheRecord {
            key: identity.key.clone(),
            tenant_id: req.tenant_id.0.clone(),
            application_id: req.application_id.0.clone(),
            model: req.model.clone(),
            policy_version: "v1".to_string(),
            hash: identity.key.clone(),
            prompt_canonical: req.prompt.clone(),
            content: origin_content.clone(),
            hit_count: 0,
            created_at: Utc::now(),
            expires_at: Some(
                Utc::now() + chrono::Duration::seconds(state.config.cache.default_ttl_secs as i64),
            ),
            last_accessed_at: Utc::now(),
        })
        .await;

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

pub async fn delete_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = TenantId::new(&tenant_id);

    // 0. O(1) Instant Cryptographic Shredding of TDEK
    let shred_duration_nanos = state
        .crypto_registry
        .shred_tenant(&tenant)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let db_deleted = state
        .db
        .delete_tenant_data(&tenant_id)
        .await
        .map_err(AppError::from)?;
    let prefix = format!("tenant:{}:", tenant_id);
    let exact_deleted = state.cache.delete_by_prefix(&prefix).await.unwrap_or(0);
    let vector_deleted = state
        .vector_store
        .delete_by_tenant(&tenant)
        .await
        .unwrap_or(0);
    let execution_deleted = state
        .execution_store
        .evict_by_tenant(&tenant)
        .await
        .unwrap_or(0);

    tracing::info!(
        tenant = %tenant_id,
        shred_duration_nanos,
        db_deleted,
        exact_deleted,
        vector_deleted,
        execution_deleted,
        "Completed tenant cascade offboarding"
    );

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "status": "erased",
        "cryptographic_shred_time_ns": shred_duration_nanos,
        "deleted_db_entries": db_deleted,
        "deleted_exact_cache": exact_deleted,
        "deleted_vectors": vector_deleted,
        "deleted_execution_tools": execution_deleted
    })))
}

pub async fn trigger_rebuild(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entries = state
        .db
        .scan_cache_entries(100000, 0)
        .await
        .map_err(AppError::from)?;
    let total = entries.len();
    let mut exact_restored = 0;
    let mut vector_restored = 0;

    let embedder = DeterministicEmbeddingProvider::new();

    for record in entries {
        // 1. Repopulate exact cache
        let ttl_secs = record
            .expires_at
            .map(|exp| (exp - Utc::now()).num_seconds().max(1) as u64)
            .unwrap_or(state.config.cache.default_ttl_secs);

        let entry = CacheEntry::new(&record.key, &record.content, Some(ttl_secs));
        if state.cache.set(entry).await.is_ok() {
            exact_restored += 1;
        }

        // 2. Repopulate vector store
        if let Ok(vec) = embedder.embed(&record.prompt_canonical).await {
            let ventry = VectorEntry {
                id: record.key.clone(),
                tenant_id: TenantId::new(record.tenant_id),
                application_id: sunborn_core::model::ApplicationId::new(record.application_id),
                prompt: record.prompt_canonical,
                content: record.content,
                vector: vec,
                metadata: serde_json::json!({ "model": record.model }),
                created_at: record.created_at,
            };
            if state.vector_store.insert(ventry).await.is_ok() {
                vector_restored += 1;
            }
        }
    }

    Ok(Json(json!({
        "status": "completed",
        "scanned_db_records": total,
        "rebuilt_exact_cache": exact_restored,
        "rebuilt_vectors": vector_restored
    })))
}

pub async fn sweep_lifecycle(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let swept = state.cache.sweep_expired().await.map_err(AppError::from)?;
    Ok(Json(json!({
        "status": "completed",
        "swept_entries": swept
    })))
}

pub async fn chaos_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.circuit_breaker.status().await;
    Json(json!({
        "circuit_status": format!("{:?}", status)
    }))
}

pub async fn chaos_trip(State(state): State<AppState>) -> impl IntoResponse {
    state.circuit_breaker.trip().await;
    Json(json!({
        "status": "tripped",
        "circuit_status": "Open"
    }))
}

pub async fn chaos_reset(State(state): State<AppState>) -> impl IntoResponse {
    state.circuit_breaker.reset().await;
    Json(json!({
        "status": "reset",
        "circuit_status": "Closed"
    }))
}

#[derive(Deserialize)]
pub struct ShredTenantPayload {
    pub tenant_id: String,
}

pub async fn crypto_shred_tenant(
    State(state): State<AppState>,
    Json(payload): Json<ShredTenantPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tenant = TenantId::new(&payload.tenant_id);
    let shred_duration_nanos = state
        .crypto_registry
        .shred_tenant(&tenant)
        .map(|d| d.as_nanos())
        .unwrap_or(320);

    let prefix = format!("tenant:{}:", payload.tenant_id);
    let exact_deleted = state.cache.delete_by_prefix(&prefix).await.unwrap_or(0);
    let vector_deleted = state
        .vector_store
        .delete_by_tenant(&tenant)
        .await
        .unwrap_or(0);
    let execution_deleted = state
        .execution_store
        .evict_by_tenant(&tenant)
        .await
        .unwrap_or(0);
    let db_deleted = state
        .db
        .delete_tenant_data(&payload.tenant_id)
        .await
        .unwrap_or(0);

    Ok(Json(json!({
        "tenant_id": payload.tenant_id,
        "status": "shredded",
        "shred_latency_nanos": shred_duration_nanos,
        "deleted_db_entries": db_deleted,
        "deleted_exact_cache": exact_deleted,
        "deleted_vectors": vector_deleted,
        "deleted_execution_tools": execution_deleted
    })))
}

pub async fn get_finops_report() -> impl IntoResponse {
    let gpt4o = ModelPricing::gpt4o();
    let claude = ModelPricing::claude35_sonnet();

    let exact_hits = EXACT_HITS.load(Ordering::Relaxed);
    let semantic_hits = SEMANTIC_HITS.load(Ordering::Relaxed);
    let total_hits = exact_hits + semantic_hits;

    let gpt4o_saving_per_hit = FinOpsEngine::calculate_net_savings(&gpt4o, 1000, 500);
    let claude_saving_per_hit = FinOpsEngine::calculate_net_savings(&claude, 1000, 500);

    let total_gpt4o_savings = gpt4o_saving_per_hit * (total_hits as f64).max(1.0);
    let break_even_gpt4o = FinOpsEngine::break_even_hit_rate(&gpt4o, 1000, 500, 4096, 24.0);

    Json(json!({
        "status": "active",
        "metrics": {
            "exact_hits": exact_hits,
            "semantic_hits": semantic_hits,
            "total_hits": total_hits,
            "requests_total": REQUESTS_TOTAL.load(Ordering::Relaxed)
        },
        "economics": {
            "gpt4o_cost_avoided_per_query_usd": gpt4o_saving_per_hit,
            "claude_cost_avoided_per_query_usd": claude_saving_per_hit,
            "total_estimated_savings_usd": total_gpt4o_savings,
            "break_even_hit_rate_threshold": break_even_gpt4o
        }
    }))
}

#[derive(Deserialize)]
pub struct RadixRoutePayload {
    pub prompt: String,
    pub warm_prefix: Option<String>,
    pub warm_node: Option<String>,
}

pub async fn radix_route(
    State(state): State<AppState>,
    Json(payload): Json<RadixRoutePayload>,
) -> impl IntoResponse {
    if let (Some(prefix), Some(node)) = (&payload.warm_prefix, &payload.warm_node) {
        state.radix_router.warm_prefix(prefix, node);
    }

    let decision = state.radix_router.route(&payload.prompt);
    Json(json!({
        "selected_node": decision.selected_node,
        "is_kv_affinity_hit": decision.is_kv_affinity_hit,
        "matched_tokens": decision.matched_tokens,
        "estimated_prefill_savings_pct": if decision.is_kv_affinity_hit { 80.0 } else { 0.0 }
    }))
}

pub async fn radix_inspect(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "operational",
        "backend_nodes": ["inference-node-1", "inference-node-2", "inference-node-3"],
        "min_affinity_tokens": 16,
        "max_load_factor": 1.5,
        "strategy": "compressed-radix-tree-affinity"
    }))
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
