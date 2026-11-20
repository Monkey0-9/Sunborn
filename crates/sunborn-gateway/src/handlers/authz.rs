//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginRequest {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub tenant_id: String,
    pub application_id: String,
    pub is_admin: bool,
}

fn attach_rate_limit_headers(
    resp: &mut Response,
    status: &crate::middleware::rate_limit::RateLimitStatus,
) {
    let headers = resp.headers_mut();
    if let Ok(val) = status.limit.to_string().parse() {
        headers.insert("X-RateLimit-Limit", val);
    }
    if let Ok(val) = status.remaining.to_string().parse() {
        headers.insert("X-RateLimit-Remaining", val);
    }
    if let Ok(val) = status.reset_secs.to_string().parse() {
        headers.insert("X-RateLimit-Reset", val);
    }
}

pub async fn login(headers: axum::http::HeaderMap, Json(payload): Json<LoginRequest>) -> Response {
    let client_key = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| {
            if !payload.client_id.trim().is_empty() {
                format!("client:{}", payload.client_id.trim())
            } else {
                "unknown-client".to_string()
            }
        });

    let rate_limiter = crate::middleware::rate_limit::get_login_rate_limiter();
    let status = rate_limiter.check_login_attempt(&client_key).await;

    if !status.allowed {
        let mut resp = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "type": "https://sunborn.ai/errors/rate-limit-exceeded",
                "title": "Too Many Requests",
                "status": 429,
                "detail": format!(
                    "Rate limit exceeded for authentication attempts from '{}'. Please retry after {} seconds.",
                    client_key, status.reset_secs
                )
            })),
        )
            .into_response();
        attach_rate_limit_headers(&mut resp, &status);
        if let Ok(val) = status.reset_secs.to_string().parse() {
            resp.headers_mut().insert("Retry-After", val);
        }
        return resp;
    }

    if payload.client_id.trim().is_empty() || payload.client_secret.trim().is_empty() {
        let mut resp = (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "type": "https://sunborn.ai/errors/bad-request",
                "title": "Bad Request",
                "status": 400,
                "detail": "client_id and client_secret are required"
            })),
        )
            .into_response();
        attach_rate_limit_headers(&mut resp, &status);
        return resp;
    }

    let registry = crate::middleware::auth::get_global_registry();
    if let Some(cred) = registry.validate_key(&payload.client_secret) {
        if let Some(ref req_tenant) = payload.tenant_id {
            if cred.tenant_id.0 != *req_tenant {
                let mut resp = (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "type": "https://sunborn.ai/errors/unauthorized",
                        "title": "Unauthorized",
                        "status": 401,
                        "detail": "Tenant ID does not match credentials"
                    })),
                )
                    .into_response();
                attach_rate_limit_headers(&mut resp, &status);
                return resp;
            }
        }

        let mut resp = (
            StatusCode::OK,
            Json(LoginResponse {
                access_token: payload.client_secret.clone(),
                token_type: "Bearer".to_string(),
                expires_in: 86400,
                tenant_id: cred.tenant_id.0,
                application_id: cred.application_id.0,
                is_admin: cred.is_admin,
            }),
        )
            .into_response();
        attach_rate_limit_headers(&mut resp, &status);
        return resp;
    }

    let mut resp = (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "type": "https://sunborn.ai/errors/unauthorized",
            "title": "Unauthorized",
            "status": 401,
            "detail": "Invalid client_id or client_secret"
        })),
    )
        .into_response();
    attach_rate_limit_headers(&mut resp, &status);
    resp
}

#[derive(Debug, Deserialize)]
pub struct CreateCredentialRequest {
    pub tenant_id: String,
    pub application_id: String,
    #[serde(default)]
    pub raw_key: Option<String>,
    #[serde(default = "default_allowed_models")]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rps: i32,
    #[serde(default)]
    pub expires_in_secs: Option<u64>,
}

fn default_allowed_models() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_rate_limit() -> i32 {
    100
}

#[derive(Debug, Deserialize)]
pub struct RevokeCredentialRequest {
    #[serde(default)]
    pub key_hash: Option<String>,
    #[serde(default)]
    pub raw_key: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RotateCredentialRequest {
    #[serde(default)]
    pub old_key_hash: Option<String>,
    #[serde(default)]
    pub old_raw_key: Option<String>,
    pub tenant_id: String,
    pub application_id: String,
    #[serde(default)]
    pub new_raw_key: Option<String>,
    #[serde(default = "default_allowed_models")]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rps: i32,
    #[serde(default)]
    pub expires_in_secs: Option<u64>,
}

pub async fn create_api_credential(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<CreateCredentialRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let raw_key = payload.raw_key.unwrap_or_else(|| {
        format!(
            "sb_live_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        )
    });
    let key_hash = crate::middleware::auth::ApiKeyRegistry::hash_key(&raw_key);
    let expires_at = payload
        .expires_in_secs
        .map(|s| Utc::now() + chrono::Duration::seconds(s as i64));

    let new_rec = sunborn_db::NewApiCredentialRecord {
        key_hash: key_hash.clone(),
        tenant_id: payload.tenant_id.clone(),
        application_id: payload.application_id.clone(),
        rate_limit_rps: payload.rate_limit_rps,
        allowed_models: payload.allowed_models.clone(),
        is_admin: payload.is_admin,
        expires_at,
    };
    state
        .postgres_db
        .persist_api_credential(&new_rec)
        .await
        .map_err(AppError::from)?;

    state
        .api_key_registry
        .register_credential(crate::middleware::auth::ApiCredential {
            tenant_id: TenantId::new(&payload.tenant_id),
            application_id: sunborn_core::ApplicationId::new(&payload.application_id),
            key_hash: key_hash.clone(),
            rate_limit_rps: payload.rate_limit_rps as u32,
            allowed_models: payload.allowed_models,
            is_admin: payload.is_admin,
            expires_at,
        });

    Ok(Json(json!({
        "status": "created",
        "raw_key": raw_key,
        "key_hash": key_hash,
        "tenant_id": payload.tenant_id,
        "application_id": payload.application_id,
        "is_admin": payload.is_admin,
        "expires_at": expires_at,
    })))
}

pub async fn revoke_api_credential(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<RevokeCredentialRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    if let Some(tenant_id) = payload.tenant_id {
        let tid = TenantId::new(&tenant_id);
        let revoked_in_mem = state.api_key_registry.revoke_tenant_keys(&tid);
        let db_revoked = state
            .postgres_db
            .revoke_tenant_credentials(&tenant_id)
            .await
            .map_err(AppError::from)?;
        return Ok(Json(json!({
            "status": "revoked_tenant_credentials",
            "tenant_id": tenant_id,
            "revoked_count": revoked_in_mem,
            "durable_revoked_count": db_revoked
        })));
    }

    let key_hash = if let Some(hash) = payload.key_hash {
        hash
    } else if let Some(raw) = payload.raw_key {
        crate::middleware::auth::ApiKeyRegistry::hash_key(&raw)
    } else {
        return Err(AppError::from(SunbornError::invalid_payload(
            "Either key_hash, raw_key, or tenant_id must be provided to revoke credential",
        )));
    };

    let removed_from_mem = state.api_key_registry.revoke_key_hash(&key_hash);
    state
        .postgres_db
        .revoke_api_credential(&key_hash)
        .await
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "revoked",
        "key_hash": key_hash,
        "revoked_from_memory": removed_from_mem,
    })))
}

pub async fn rotate_api_credential(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<RotateCredentialRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let old_hash = if let Some(hash) = payload.old_key_hash {
        hash
    } else if let Some(raw) = payload.old_raw_key {
        crate::middleware::auth::ApiKeyRegistry::hash_key(&raw)
    } else {
        return Err(AppError::from(SunbornError::invalid_payload(
            "Either old_key_hash or old_raw_key must be provided for rotation",
        )));
    };

    // 1. Revoke old credential in-memory and in durable storage
    state.api_key_registry.revoke_key_hash(&old_hash);
    state
        .postgres_db
        .revoke_api_credential(&old_hash)
        .await
        .map_err(AppError::from)?;

    // 2. Issue new credential
    let new_raw = payload.new_raw_key.unwrap_or_else(|| {
        format!(
            "sb_live_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        )
    });
    let new_hash = crate::middleware::auth::ApiKeyRegistry::hash_key(&new_raw);
    let expires_at = payload
        .expires_in_secs
        .map(|s| Utc::now() + chrono::Duration::seconds(s as i64));

    let new_rec = sunborn_db::NewApiCredentialRecord {
        key_hash: new_hash.clone(),
        tenant_id: payload.tenant_id.clone(),
        application_id: payload.application_id.clone(),
        rate_limit_rps: payload.rate_limit_rps,
        allowed_models: payload.allowed_models.clone(),
        is_admin: payload.is_admin,
        expires_at,
    };
    state
        .postgres_db
        .persist_api_credential(&new_rec)
        .await
        .map_err(AppError::from)?;

    state
        .api_key_registry
        .register_credential(crate::middleware::auth::ApiCredential {
            tenant_id: TenantId::new(&payload.tenant_id),
            application_id: sunborn_core::ApplicationId::new(&payload.application_id),
            key_hash: new_hash.clone(),
            rate_limit_rps: payload.rate_limit_rps as u32,
            allowed_models: payload.allowed_models,
            is_admin: payload.is_admin,
            expires_at,
        });

    Ok(Json(json!({
        "status": "rotated",
        "old_key_hash": old_hash,
        "new_raw_key": new_raw,
        "new_key_hash": new_hash,
        "tenant_id": payload.tenant_id,
        "application_id": payload.application_id,
        "expires_at": expires_at,
    })))
}
