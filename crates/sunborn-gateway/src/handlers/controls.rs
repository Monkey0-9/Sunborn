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
    /// Both limits, or neither. Omitting both with a `tier` applies that tier's
    /// policy defaults — the upgrade path, which changes a tenant in place.
    #[serde(default)]
    pub daily_requests: Option<u64>,
    #[serde(default)]
    pub monthly_requests: Option<u64>,
}

/// Operator control for daily/monthly tenant quotas. Enrollment is explicit;
/// a tenant without a quota row keeps its existing admission behavior.
///
/// Two modes:
/// * explicit limits (`daily_requests` + `monthly_requests`) — the original
///   behavior, unchanged, for a bespoke pilot envelope;
/// * tier policy (`tier` only) — applies the tier's quota, model allowlist,
///   retention, and support level to the existing tenant. The tenant ID, keys,
///   cache namespace, and audit history are untouched, so an upgrade is a policy
///   write rather than a data migration.
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

    match (payload.daily_requests, payload.monthly_requests) {
        (Some(daily_requests), Some(monthly_requests)) => {
            if !quota_values_fit_database(daily_requests)
                || !quota_values_fit_database(monthly_requests)
            {
                return Err(SunbornError::invalid_payload(
                    "daily_requests and monthly_requests must fit a signed 64-bit counter",
                )
                .into());
            }

            let limits = sunborn_db::TenantQuotaLimits::new(
                payload.tenant_id.clone(),
                payload.tier.unwrap_or_else(|| "free".to_string()),
                daily_requests,
                monthly_requests,
            );
            state
                .quota_ledger
                .set_tenant_limits(limits)
                .await
                .map_err(quota_control_error)?;

            Ok(Json(json!({
                "status": "updated",
                "tenant_id": payload.tenant_id,
                "applied": "explicit_limits",
                "daily_requests": daily_requests,
                "monthly_requests": monthly_requests,
                "enforcement_scope": state.quota_ledger.enforcement_scope(),
            })))
        }
        (None, None) => {
            let tier_name = payload.tier.as_deref().ok_or_else(|| {
                SunbornError::invalid_payload(
                    "provide daily_requests and monthly_requests, or a tier to apply its policy",
                )
            })?;
            let policy = crate::tiers::TierPolicy::resolve(tier_name)
                .map_err(SunbornError::invalid_payload)?;

            // Persist the defaults before enrolling the quota, for the same
            // reason provisioning does: a failed durable policy write must not
            // leave this replica enforcing limits the cluster cannot see.
            // `identity_verified: false` and `invited_by: None` are "unchanged"
            // in the upsert, not a revocation.
            state
                .postgres_db
                .apply_tenant_tier(&sunborn_db::TenantTierAssignment {
                    tenant_id: payload.tenant_id.clone(),
                    tier: policy.tier.as_str().to_string(),
                    retention_days: policy.retention_days,
                    support_level: policy.support_level.as_str().to_string(),
                    identity_verified: false,
                    invited_by: None,
                    allowed_models: policy.allowed_models.clone(),
                })
                .await
                .map_err(AppError::from)?;
            state
                .quota_ledger
                .set_tenant_limits(policy.quota_limits(&payload.tenant_id))
                .await
                .map_err(quota_control_error)?;

            Ok(Json(json!({
                "status": "updated",
                "tenant_id": payload.tenant_id,
                "applied": "tier_policy",
                "tier": policy.tier.as_str(),
                "daily_requests": policy.daily_requests,
                "monthly_requests": policy.monthly_requests,
                "allowed_models": policy.allowed_models,
                "retention_days": policy.retention_days,
                "support_level": policy.support_level.as_str(),
                "enforcement_scope": state.quota_ledger.enforcement_scope(),
            })))
        }
        _ => Err(SunbornError::invalid_payload(
            "daily_requests and monthly_requests must be provided together",
        )
        .into()),
    }
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
