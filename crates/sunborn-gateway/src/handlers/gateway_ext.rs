//! Extended Gateway Control & Agentic Protocol Handlers:
//! - Virtual Keys (`/v1/virtual-keys`)
//! - Guardrails (`/v1/guardrails/config`)
//! - CEL Policy Engine (`/v1/policies/cel`)
//! - Token-Aware Rate Limits (`/v1/governance/token-limits`)
//! - MCP Multiplexing Gateway (`/mcp`, `/v1/mcp/servers`)
//! - A2A Agent Gateway (`/.well-known/agent.json`, `/v1/a2a/agents`, `/a2a/tasks/send`)

use super::*;
use crate::a2a_gateway::{A2aTaskRequest, AgentCard};
use crate::guardrails::GuardrailConfig;
use crate::mcp_gateway::{JsonRpcRequest, JsonRpcResponse, McpServerConfig};
use crate::token_limits::TokenQuotaPolicy;
use crate::virtual_keys::CreateVirtualKeyRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTokenLimitRequest {
    pub tenant_id: String,
    #[serde(flatten)]
    pub policy: TokenQuotaPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCelPolicyRequest {
    pub id: String,
    pub expression: String,
    #[serde(default = "default_deny")]
    pub effect: String,
}

fn default_deny() -> String {
    "deny".to_string()
}

pub async fn create_virtual_key(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(req): Json<CreateVirtualKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    let created = state.virtual_keys.create(req);
    Ok(Json(serde_json::to_value(created).unwrap_or_default()))
}

pub async fn list_virtual_keys(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    let filter = if sec_ctx.is_admin {
        None
    } else {
        Some(sec_ctx.tenant_id.0.as_str())
    };
    let keys = state.virtual_keys.list(filter);
    Ok(Json(json!({ "virtual_keys": keys })))
}

pub async fn revoke_virtual_key(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    if !state.virtual_keys.revoke(&id) {
        return Err(SunbornError::bad_request(format!("Virtual key '{}' not found", id)).into());
    }
    Ok(Json(json!({ "revoked": true, "id": id })))
}

pub async fn get_guardrails_config(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<GuardrailConfig>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    Ok(Json(state.guardrail_chain.get_config()))
}

pub async fn set_guardrails_config(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(cfg): Json<GuardrailConfig>,
) -> Result<Json<GuardrailConfig>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    state.guardrail_chain.update_config(cfg.clone());
    Ok(Json(cfg))
}

pub async fn list_cel_policies(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    let rules = state.cel_engine.list_rules();
    let reqs = state.cel_engine.combined_requirements();
    Ok(Json(json!({ "rules": rules, "requirements": reqs })))
}

pub async fn add_cel_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(req): Json<AddCelPolicyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    let rule = state
        .cel_engine
        .add_rule(req.id, req.expression, req.effect);
    Ok(Json(serde_json::to_value(rule).unwrap_or_default()))
}

pub async fn delete_cel_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    if !state.cel_engine.remove_rule(&id) {
        return Err(SunbornError::bad_request(format!("CEL policy '{}' not found", id)).into());
    }
    Ok(Json(json!({ "deleted": true, "id": id })))
}

pub async fn set_token_limits(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(req): Json<SetTokenLimitRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    state
        .token_limiter
        .set_policy(&req.tenant_id, req.policy.clone());
    Ok(Json(json!({
        "tenant_id": req.tenant_id,
        "policy": req.policy,
    })))
}

pub async fn get_token_limits(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(tenant_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    if !sec_ctx.is_admin && sec_ctx.tenant_id.0 != tenant_id {
        return Err(SunbornError::forbidden("Cannot inspect another tenant's token limits").into());
    }
    let status = state.token_limiter.status(&tenant_id);
    Ok(Json(json!({ "tenant_id": tenant_id, "status": status })))
}

pub async fn mcp_rpc(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(rpc): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    let resp = state
        .mcp_gateway
        .handle_rpc(
            rpc,
            &sec_ctx.tenant_id.0,
            &sec_ctx.application_id.0,
            &state.cel_engine,
        )
        .await;
    Ok(Json(resp))
}

pub async fn list_mcp_servers(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    Ok(Json(json!({ "servers": state.mcp_gateway.list_servers() })))
}

pub async fn register_mcp_server(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(cfg): Json<McpServerConfig>,
) -> Result<Json<McpServerConfig>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    let reg = state
        .mcp_gateway
        .register_server(cfg)
        .map_err(SunbornError::bad_request)?;
    Ok(Json(reg))
}

pub async fn delete_mcp_server(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    if !state.mcp_gateway.remove_server(&name) {
        return Err(SunbornError::bad_request(format!("MCP server '{}' not found", name)).into());
    }
    Ok(Json(json!({ "deleted": true, "name": name })))
}

pub async fn a2a_well_known(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.a2a_gateway.well_known_card())
}

pub async fn list_a2a_agents(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    Ok(Json(json!({ "agents": state.a2a_gateway.list_agents() })))
}

pub async fn register_a2a_agent(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(card): Json<AgentCard>,
) -> Result<Json<AgentCard>, AppError> {
    sec_ctx.require_admin().map_err(SunbornError::forbidden)?;
    let reg = state
        .a2a_gateway
        .register_agent(card)
        .map_err(SunbornError::bad_request)?;
    Ok(Json(reg))
}

pub async fn a2a_send_task(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(req): Json<A2aTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Authentication required").into());
    }
    let resp = state
        .a2a_gateway
        .send_task(req)
        .await
        .map_err(SunbornError::origin)?;
    Ok(Json(resp))
}
