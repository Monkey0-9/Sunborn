//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct ExecutionPlanRequest {
    pub tenant_id: String,
    pub application_id: String,
    pub plan: sunborn_execution::ExecutionPlan,
}

pub async fn plan_execution(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(mut payload): Json<ExecutionPlanRequest>,
) -> Result<Json<sunborn_execution::PlanEvaluation>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    if !sec_ctx.is_admin {
        if payload.tenant_id.is_empty() {
            payload.tenant_id = sec_ctx.tenant_id.0.clone();
        } else if payload.tenant_id != sec_ctx.tenant_id.0 {
            return Err(SunbornError::forbidden(format!(
                "Tenant mismatch: authenticated as '{}', cannot access '{}'",
                sec_ctx.tenant_id.0, payload.tenant_id
            ))
            .into());
        }
    }

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
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(mut payload): Json<RecordToolResultRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    if !sec_ctx.is_admin {
        if payload.tenant_id.is_empty() {
            payload.tenant_id = sec_ctx.tenant_id.0.clone();
        } else if payload.tenant_id != sec_ctx.tenant_id.0 {
            return Err(SunbornError::forbidden(format!(
                "Tenant mismatch: authenticated as '{}', cannot access '{}'",
                sec_ctx.tenant_id.0, payload.tenant_id
            ))
            .into());
        }
    }

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
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<EvictToolRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(mut payload): Json<crate::shadow::ShadowEvaluationRequest>,
) -> Result<Json<crate::shadow::ShadowEvaluationResult>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    if !sec_ctx.is_admin {
        if payload.tenant_id.0.is_empty() {
            payload.tenant_id = sec_ctx.tenant_id.clone();
        } else if payload.tenant_id.0 != sec_ctx.tenant_id.0 {
            return Err(SunbornError::forbidden(format!(
                "Tenant mismatch: authenticated as '{}', cannot access '{}'",
                sec_ctx.tenant_id.0, payload.tenant_id.0
            ))
            .into());
        }
    }

    let result = state
        .shadow_evaluator
        .evaluate_drift(&payload, &state.cache, &state.vector_store)
        .await
        .map_err(AppError::from)?;
    Ok(Json(result))
}
