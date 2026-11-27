//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Deserialize)]
pub struct ShredTenantPayload {
    pub tenant_id: String,
}

#[derive(serde::Deserialize)]
pub struct ProvisionTenantPayload {
    pub tenant_id: String,
    /// Tier whose defaults are applied. Omitted means `pilot`: an existing
    /// operator-driven caller keeps its generous limits instead of being
    /// silently moved onto the free envelope.
    #[serde(default)]
    pub tier: Option<String>,
    /// Attestation that identity verification completed. Required by
    /// identity-gated tiers and ignored by the others.
    #[serde(default)]
    pub identity_verified: bool,
    /// Who provisioned the tenant. The per-inviter cap is counted against it.
    #[serde(default)]
    pub invited_by: Option<String>,
}

pub async fn provision_tenant(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<ProvisionTenantPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    if payload.tenant_id.trim().is_empty() {
        return Err(SunbornError::invalid_payload("tenant_id is required").into());
    }

    let tier_name = payload.tier.as_deref().unwrap_or("pilot");
    let policy =
        crate::tiers::TierPolicy::resolve(tier_name).map_err(SunbornError::invalid_payload)?;

    let tenant = TenantId::new(&payload.tenant_id);

    // Idempotency: provisioning an existing tenant must not rotate its TDEK.
    // Rotating silently would make every previously encrypted cache entry and
    // certificate unreadable on other replicas. It must also not re-apply tier
    // admission checks: an existing tenant is already authorized, and re-running
    // provisioning should stay a no-op rather than start failing once an
    // invitation window closes.
    if let Some((wrapped, provider_name)) = state
        .postgres_db
        .fetch_wrapped_tdek(&payload.tenant_id)
        .await
        .map_err(AppError::from)?
    {
        state
            .crypto_registry
            .unwrap_and_provision(tenant, &wrapped)
            .map_err(|e| {
                AppError::from(SunbornError::internal(format!(
                    "Existing tenant key could not be loaded: {e}"
                )))
            })?;
        let applied_tier = state
            .postgres_db
            .get_tenant_tier_defaults(&payload.tenant_id)
            .await
            .ok()
            .flatten()
            .map(|(tier, _)| tier);
        return Ok(Json(serde_json::json!({
            "status": "already_provisioned",
            "tenant_id": payload.tenant_id,
            "kms_provider": provider_name,
            "durable_persistence": true,
            "tier": applied_tier,
        })));
    }

    if policy.requires_verified_identity && !payload.identity_verified {
        return Err(SunbornError::forbidden(format!(
            "tier '{}' requires identity_verified: true at provisioning",
            policy.tier.as_str()
        ))
        .into());
    }

    if let (Some(cap), Some(inviter)) = (policy.invitation_cap, payload.invited_by.as_deref()) {
        let since = Utc::now() - chrono::Duration::days(30);
        let used = state
            .postgres_db
            .count_recent_invitations(inviter, since)
            .await
            .map_err(AppError::from)?;
        if used >= i64::from(cap) {
            return Err(SunbornError::rate_limit(format!(
                "invitation cap reached for '{inviter}': {used}/{cap} in the last 30 days"
            ))
            .into());
        }
    }

    state
        .crypto_registry
        .provision_tenant_secure(tenant.clone())
        .map_err(|e| AppError::from(SunbornError::internal(format!("Provisioning failed: {e}"))))?;

    // Wrap the tenant key under the active KMS provider
    let wrapped_tdek = state
        .crypto_registry
        .wrap_tenant_key(&tenant)
        .map_err(|e| AppError::from(SunbornError::internal(format!("Wrapping failed: {e}"))))?;

    let provider_name = state.crypto_registry.provider().provider_name();

    // Persist durable wrapped TDEK to PostgreSQL WAL
    state
        .postgres_db
        .persist_wrapped_tdek(&payload.tenant_id, &wrapped_tdek, provider_name)
        .await
        .map_err(AppError::from)?;

    // Apply the tier as one unit: the persisted defaults and the enrolled quota
    // must agree. The durable tier write happens first, so a failure there never
    // leaves the replica enforcing a limit the rest of the cluster cannot see.
    state
        .postgres_db
        .apply_tenant_tier(&sunborn_db::TenantTierAssignment {
            tenant_id: payload.tenant_id.clone(),
            tier: policy.tier.as_str().to_string(),
            retention_days: policy.retention_days,
            support_level: policy.support_level.as_str().to_string(),
            identity_verified: payload.identity_verified,
            invited_by: payload.invited_by.clone(),
            allowed_models: policy.allowed_models.clone(),
        })
        .await
        .map_err(AppError::from)?;
    state
        .quota_ledger
        .set_tenant_limits(policy.quota_limits(&payload.tenant_id))
        .await
        .map_err(tier_quota_error)?;

    Ok(Json(serde_json::json!({
        "status": "provisioned",
        "tenant_id": payload.tenant_id,
        "kms_provider": provider_name,
        "durable_persistence": true,
        "tier": policy.tier.as_str(),
        "daily_requests": policy.daily_requests,
        "monthly_requests": policy.monthly_requests,
        "allowed_models": policy.allowed_models,
        "retention_days": policy.retention_days,
        "support_level": policy.support_level.as_str(),
        "identity_verified": payload.identity_verified,
        "invitation_cap": policy.invitation_cap,
        "quota_enforcement_scope": state.quota_ledger.enforcement_scope(),
    })))
}

fn tier_quota_error(err: QuotaError) -> AppError {
    match err {
        QuotaError::Unavailable(detail) => AppError::from(SunbornError::cache_unavailable(detail)),
        QuotaError::Exceeded { .. } => AppError::from(SunbornError::invalid_payload(
            "tier quota cannot be applied: the tenant already exceeds a limit",
        )),
    }
}

pub async fn crypto_shred_tenant(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ShredTenantPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let tenant = TenantId::new(&payload.tenant_id);
    let shred_duration = state.crypto_registry.shred_tenant(&tenant).map_err(|e| {
        AppError::from(SunbornError::internal(format!(
            "Crypto shredding failed: {e}"
        )))
    })?;

    let prefix_sunborn = format!("sunborn:{}:", payload.tenant_id);
    let exact_deleted_sunborn = state
        .cache
        .delete_by_prefix(&prefix_sunborn)
        .await
        .map_err(AppError::from)?;
    let prefix_legacy = format!("tenant:{}:", payload.tenant_id);
    let exact_deleted_legacy = state
        .cache
        .delete_by_prefix(&prefix_legacy)
        .await
        .map_err(AppError::from)?;
    let exact_deleted = exact_deleted_sunborn + exact_deleted_legacy;

    let vector_deleted = state
        .vector_store
        .delete_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    let execution_deleted = state
        .execution_store
        .evict_by_tenant(&tenant)
        .await
        .map_err(AppError::from)?;
    let db_deleted = state
        .db
        .delete_tenant_data(&payload.tenant_id)
        .await
        .map_err(AppError::from)?;
    let learning_deleted = state
        .learning_store
        .purge_tenant(&tenant)
        .map_err(AppError::from)?;
    let _clusters_purged = state.paraphrase_clusters.purge_tenant(&tenant);
    state
        .temporal_engine
        .bump_permission_version(AppState::tenant_permission_scope_uuid(&payload.tenant_id));
    let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);
    state.quota_ledger.remove_tenant(&payload.tenant_id).await;

    // Independent Read-After-Delete Verification Pass
    let (verified_erasure, residuals) =
        verify_multi_store_erasure_integrity(&state, &payload.tenant_id).await;
    let final_status = if verified_erasure {
        "shredded"
    } else {
        "partial_failure"
    };

    Ok(Json(json!({
        "tenant_id": payload.tenant_id,
        "status": final_status,
        "shred_latency_nanos": shred_duration.as_nanos(),
        "deleted_db_entries": db_deleted,
        "deleted_exact_cache": exact_deleted,
        "deleted_vectors": vector_deleted,
        "deleted_execution_tools": execution_deleted,
        "deleted_learning_events": learning_deleted,
        "verified_multi_store_erasure": verified_erasure,
        "residual_records": {
            "db": residuals.db,
            "exact_cache": residuals.exact_cache,
            "vector_store": residuals.vector_store,
            "execution_store": residuals.execution_store,
            "learning_store": residuals.learning_store,
        }
    })))
}

pub async fn get_finops_report() -> impl IntoResponse {
    let gpt4o = ModelPricing::gpt4o();
    let claude = ModelPricing::claude35_sonnet();
    let sarvam = ModelPricing::sarvam_2b();
    let grok = ModelPricing::grok2();
    let gemini = ModelPricing::gemini15_pro();

    let exact_hits = EXACT_HITS.load(Ordering::Relaxed);
    let semantic_hits = SEMANTIC_HITS.load(Ordering::Relaxed);
    let total_hits = exact_hits + semantic_hits;

    let gpt4o_gross_per_hit = FinOpsEngine::calculate_token_cost_avoided(&gpt4o, 1000, 500);
    let gpt4o_saving_per_hit = FinOpsEngine::calculate_net_savings(&gpt4o, 1000, 500);
    let claude_saving_per_hit = FinOpsEngine::calculate_net_savings(&claude, 1000, 500);
    let sarvam_saving_per_hit = FinOpsEngine::calculate_net_savings(&sarvam, 1000, 500);
    let grok_saving_per_hit = FinOpsEngine::calculate_net_savings(&grok, 1000, 500);
    let gemini_saving_per_hit = FinOpsEngine::calculate_net_savings(&gemini, 1000, 500);

    let embed_cost_per_query = FinOpsEngine::LOCAL_EMBED_COST_PER_CALL;
    let cache_overhead_per_query = FinOpsEngine::CACHE_LOOKUP_OVERHEAD_PER_CALL;

    // Honest empirical calculation: when total_hits is 0, total savings is $0.00
    let total_gpt4o_gross_savings = gpt4o_gross_per_hit * (total_hits as f64);
    let total_gpt4o_net_savings = gpt4o_saving_per_hit * (total_hits as f64);
    let break_even_gpt4o = FinOpsEngine::break_even_hit_rate(&gpt4o, 1000, 500, 4096, 24.0);

    // Sovereign Indic currency & GST metrics
    let inr_rate = 84.50;
    let total_net_savings_inr = total_gpt4o_net_savings * inr_rate;
    let total_net_savings_mp =
        MicroPaise::from_inr(total_net_savings_inr).unwrap_or(MicroPaise::ZERO);
    let gst_breakdown = GstBreakdown::calculate(total_net_savings_inr, false, None);

    (
        [(
            "cache-control",
            "public, max-age=5, stale-while-revalidate=30",
        )],
        Json(json!({
            "status": "active",
            "metrics": {
                "exact_hits": exact_hits,
                "semantic_hits": semantic_hits,
                "total_hits": total_hits,
                "requests_total": REQUESTS_TOTAL.load(Ordering::Relaxed)
            },
            "economics": {
                "gpt4o_gross_cost_avoided_per_query_usd": gpt4o_gross_per_hit,
                "gpt4o_cost_avoided_per_query_usd": gpt4o_saving_per_hit,
                "gpt4o_net_savings_per_query_usd": gpt4o_saving_per_hit,
                "embedding_inference_cost_per_query_usd": embed_cost_per_query,
                "cache_lookup_overhead_cost_per_query_usd": cache_overhead_per_query,
                "claude_cost_avoided_per_query_usd": claude_saving_per_hit,
                "grok_cost_avoided_per_query_usd": grok_saving_per_hit,
                "gemini_cost_avoided_per_query_usd": gemini_saving_per_hit,
                "sarvam_cost_avoided_per_query_usd": sarvam_saving_per_hit,
                "total_estimated_gross_savings_usd": total_gpt4o_gross_savings,
                "total_estimated_net_savings_usd": total_gpt4o_net_savings,
                "total_estimated_savings_usd": total_gpt4o_net_savings,
                "total_estimated_savings_inr": total_net_savings_inr,
                "total_estimated_savings_inr_formatted": format_inr_currency(total_net_savings_inr),
                "total_estimated_savings_inr_compact": format_lakhs_crores(total_net_savings_inr),
                "inr_exchange_rate": inr_rate,
                "micro_paise_avoided": total_net_savings_mp.0,
                "gst_applicable_rate_percent": 18.0,
                "gst_sac_code": GstBreakdown::SAC_CODE,
                "gst_breakdown": gst_breakdown,
                "break_even_hit_rate_threshold": break_even_gpt4o
            }
        })),
    )
}
