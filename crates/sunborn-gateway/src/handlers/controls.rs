//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct SpendCeilingRequest {
    pub tenant_id: String,
    pub ceiling_usd: f64,
}

/// Operator control for the spend ceiling. With the Postgres backend active the
/// ceiling is written to the durable ledger, so every replica enforces it.
pub async fn set_spend_ceiling(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<SpendCeilingRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    if payload.tenant_id.trim().is_empty() {
        return Err(SunbornError::invalid_payload("tenant_id is required").into());
    }
    if !payload.ceiling_usd.is_finite() || payload.ceiling_usd < 0.0 {
        return Err(SunbornError::invalid_payload(
            "ceiling_usd must be a finite non-negative value",
        )
        .into());
    }

    state
        .spend_ledger
        .set_ceiling(&payload.tenant_id, payload.ceiling_usd)
        .await
        .map_err(|err| match err {
            crate::finops::SpendLedgerError::Unavailable(detail) => {
                AppError::from(SunbornError::cache_unavailable(detail))
            }
            crate::finops::SpendLedgerError::CeilingExceeded { committed, ceiling } => {
                AppError::from(SunbornError::invalid_payload(format!(
                    "rejected ceiling update (committed: {committed}, ceiling: {ceiling})"
                )))
            }
        })?;

    Ok(Json(json!({
        "status": "updated",
        "tenant_id": payload.tenant_id,
        "ceiling_usd": payload.ceiling_usd,
        "enforcement_scope": state.spend_ledger.enforcement_scope(),
    })))
}

#[derive(Deserialize)]
pub struct TenantQuotaRequest {
    pub tenant_id: String,
    pub tier: Option<String>,
    pub daily_requests: u64,
    pub monthly_requests: u64,
}

/// Operator control for daily/monthly tenant quotas. Enrollment is explicit;
/// a tenant without a quota row keeps its existing admission behavior.
pub async fn set_tenant_quota(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<TenantQuotaRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    if payload.tenant_id.trim().is_empty() {
        return Err(SunbornError::invalid_payload("tenant_id is required").into());
    }
    if !quota_values_fit_database(payload.daily_requests)
        || !quota_values_fit_database(payload.monthly_requests)
    {
        return Err(SunbornError::invalid_payload(
            "daily_requests and monthly_requests must fit a signed 64-bit counter",
        )
        .into());
    }

    let limits = sunborn_db::TenantQuotaLimits::new(
        payload.tenant_id.clone(),
        payload.tier.unwrap_or_else(|| "free".to_string()),
        payload.daily_requests,
        payload.monthly_requests,
    );
    state
        .quota_ledger
        .set_tenant_limits(limits)
        .await
        .map_err(quota_control_error)?;

    Ok(Json(json!({
        "status": "updated",
        "tenant_id": payload.tenant_id,
        "daily_requests": payload.daily_requests,
        "monthly_requests": payload.monthly_requests,
        "enforcement_scope": state.quota_ledger.enforcement_scope(),
    })))
}

#[derive(Deserialize)]
pub struct FreePoolRequest {
    pub daily_requests: u64,
    pub monthly_requests: u64,
}

/// Operator control for the aggregate free-pool ceiling.
pub async fn set_free_pool(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<FreePoolRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    if !quota_values_fit_database(payload.daily_requests)
        || !quota_values_fit_database(payload.monthly_requests)
    {
        return Err(SunbornError::invalid_payload(
            "daily_requests and monthly_requests must fit a signed 64-bit counter",
        )
        .into());
    }

    state
        .quota_ledger
        .set_free_pool_limits(sunborn_db::FreePoolLimits::new(
            payload.daily_requests,
            payload.monthly_requests,
        ))
        .await
        .map_err(quota_control_error)?;

    Ok(Json(json!({
        "status": "updated",
        "daily_requests": payload.daily_requests,
        "monthly_requests": payload.monthly_requests,
        "enforcement_scope": state.quota_ledger.enforcement_scope(),
    })))
}

fn quota_values_fit_database(value: u64) -> bool {
    value <= i64::MAX as u64
}

fn quota_control_error(err: QuotaError) -> AppError {
    match err {
        QuotaError::Unavailable(detail) => AppError::from(SunbornError::cache_unavailable(detail)),
        QuotaError::Exceeded { .. } => AppError::from(SunbornError::invalid_payload(
            "quota control cannot be applied while a limit is exceeded",
        )),
    }
}
