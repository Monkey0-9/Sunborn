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
    /// Omitted means "use the tenant tier's default allowlist". A supplied list
    /// is a ceiling check against that default, not an override of it.
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rps: i32,
    #[serde(default)]
    pub expires_in_secs: Option<u64>,
}

/// Resolves the effective allowlist for a new credential.
///
/// A tier default of `["*"]` is unrestricted, so a requested list (or the
/// implicit `*`) is accepted unchanged. A restricted tier list is a ceiling: an
/// operator cannot widen a free tenant to a premium model by asking for `*`, and
/// a model outside the tier is refused rather than silently dropped.
fn resolve_allowed_models(
    requested: Option<Vec<String>>,
    tier_defaults: Option<(String, Vec<String>)>,
) -> Result<Vec<String>, SunbornError> {
    let Some((_, defaults)) = tier_defaults else {
        return Ok(requested.unwrap_or_else(|| vec!["*".to_string()]));
    };
    if defaults.iter().any(|model| model == "*") {
        return Ok(requested.unwrap_or_else(|| vec!["*".to_string()]));
    }
    let effective = requested.unwrap_or_else(|| defaults.clone());
    for model in &effective {
        if model == "*" || !defaults.iter().any(|allowed| allowed == model) {
            return Err(SunbornError::forbidden(format!(
                "model '{model}' is not permitted by the tenant's tier"
            )));
        }
    }
    Ok(effective)
}

fn default_rate_limit() -> i32 {
    100
}

#[cfg(test)]
mod allowlist_tests {
    use super::*;

    fn restricted(models: &[&str]) -> Option<(String, Vec<String>)> {
        Some((
            "free".to_string(),
            models.iter().map(|model| (*model).to_string()).collect(),
        ))
    }

    #[test]
    fn unrestricted_tier_accepts_any_request() {
        let defaults = Some(("standard".to_string(), vec!["*".to_string()]));
        assert_eq!(
            resolve_allowed_models(None, defaults.clone()).expect("implicit default"),
            vec!["*".to_string()]
        );
        assert_eq!(
            resolve_allowed_models(Some(vec!["gpt-4o".to_string()]), defaults)
                .expect("explicit model"),
            vec!["gpt-4o".to_string()]
        );
    }

    #[test]
    fn restricted_tier_inherits_its_defaults_when_unspecified() {
        let resolved =
            resolve_allowed_models(None, restricted(&["gpt-4o-mini", "gemini-1.5-flash"]))
                .expect("tier defaults apply");
        assert_eq!(resolved, vec!["gpt-4o-mini", "gemini-1.5-flash"]);
    }

    #[test]
    fn restricted_tier_refuses_to_be_widened() {
        let defaults = restricted(&["gpt-4o-mini"]);
        assert!(
            resolve_allowed_models(Some(vec!["*".to_string()]), defaults.clone()).is_err(),
            "a wildcard request must not escape a restricted tier"
        );
        assert!(
            resolve_allowed_models(Some(vec!["gpt-4o".to_string()]), defaults.clone()).is_err(),
            "a premium model must be refused, not silently dropped"
        );
        assert!(resolve_allowed_models(Some(vec!["gpt-4o-mini".to_string()]), defaults).is_ok());
    }

    #[test]
    fn missing_tenant_row_keeps_the_legacy_unrestricted_default() {
        assert_eq!(
            resolve_allowed_models(None, None).expect("pre-migration tenant"),
            vec!["*".to_string()]
        );
    }
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
    /// Omitted means "use the tenant tier's default allowlist". A supplied list
    /// is a ceiling check against that default, not an override of it.
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
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

    let tier_defaults = state
        .postgres_db
        .get_tenant_tier_defaults(&payload.tenant_id)
        .await
        .map_err(AppError::from)?;
    let allowed_models = resolve_allowed_models(payload.allowed_models, tier_defaults)?;

    let new_rec = sunborn_db::NewApiCredentialRecord {
        key_hash: key_hash.clone(),
        tenant_id: payload.tenant_id.clone(),
        application_id: payload.application_id.clone(),
        rate_limit_rps: payload.rate_limit_rps,
        allowed_models: allowed_models.clone(),
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
            allowed_models,
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
        state
            .temporal_engine
            .bump_permission_version(AppState::tenant_permission_scope_uuid(&tenant_id));
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
        .temporal_engine
        .bump_permission_version(AppState::tenant_permission_scope_uuid(&payload.tenant_id));
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

    let tier_defaults = state
        .postgres_db
        .get_tenant_tier_defaults(&payload.tenant_id)
        .await
        .map_err(AppError::from)?;
    let allowed_models = resolve_allowed_models(payload.allowed_models, tier_defaults)?;

    let new_rec = sunborn_db::NewApiCredentialRecord {
        key_hash: new_hash.clone(),
        tenant_id: payload.tenant_id.clone(),
        application_id: payload.application_id.clone(),
        rate_limit_rps: payload.rate_limit_rps,
        allowed_models: allowed_models.clone(),
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
            allowed_models,
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
