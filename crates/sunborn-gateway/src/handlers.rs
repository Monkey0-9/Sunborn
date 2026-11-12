/* trunk-ignore-all(rustfmt) */
use crate::middleware::auth::SecurityContext;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use sunborn_cache::CacheEntry;
use sunborn_core::{
    finops::{FinOpsEngine, ModelPricing},
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, ResponseEnvelope, SunbornError,
    TenantId,
};
use sunborn_execution::ExecutionCacheStore;
use sunborn_identity::{EncryptedEnvelope, IdentityEngine};
use sunborn_invalidation::InvalidationEvent;
use sunborn_semantic::{EmbeddingProvider, VectorEntry, VectorStore};
use uuid::Uuid;

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static EXACT_HITS: AtomicU64 = AtomicU64::new(0);
static SEMANTIC_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static RATE_LIMITED_TOTAL: AtomicU64 = AtomicU64::new(0);
static SPEND_VELOCITY_REJECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static REQUEST_DURATION_MS_SUM: AtomicU64 = AtomicU64::new(0);

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "sunborn-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now()
    }))
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let cache_res = state.cache.exists("__readiness_probe__").await;
    let db_res = state.db.get_tenant("__readiness_probe__").await;
    let vector_res = state
        .vector_store
        .count(&TenantId::new("__readiness_probe__"))
        .await;
    let llm_res = state.origin_provider.health_check().await;
    let embed_res = state.selected_embedder.health_check().await;

    let is_prod = sunborn_core::is_production_mode();

    let cache_ok = cache_res.is_ok();
    let db_ok = db_res.is_ok();
    let vector_ok = vector_res.is_ok();
    let llm_ok = llm_res.is_ok() && (!is_prod || !state.origin_provider.is_mock());
    let embed_ok = embed_res.is_ok() && (!is_prod || !state.selected_embedder.is_deterministic());

    let all_ok = cache_ok && db_ok && vector_ok && llm_ok && embed_ok;

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let report = json!({
        "status": if all_ok { "ready" } else { "degraded" },
        "components": {
            "cache": {
                "status": if cache_ok { "connected" } else { "unavailable" },
                "error": cache_res.err().map(|_| "cache_unavailable".to_string())
            },
            "database": {
                "status": if db_ok { "connected" } else { "unavailable" },
                "error": db_res.err().map(|_| "database_unavailable".to_string())
            },
            "vector_store": {
                "status": if vector_ok { "connected" } else { "unavailable" },
                "error": vector_res.err().map(|e| e.to_string())
            },
            "llm_origin": {
                "status": if llm_ok { "connected" } else { "unavailable" },
                "is_mock": state.origin_provider.is_mock(),
                "error": llm_res.err().map(|e| e.to_string())
            },
            "embedding_provider": {
                "status": if embed_ok { "connected" } else { "unavailable" },
                "is_deterministic": state.selected_embedder.is_deterministic(),
                "error": embed_res.err().map(|e| e.to_string())
            },
            "semantic_engine": {
                "status": if vector_ok && embed_ok { "ready" } else { "degraded" }
            },
            "invalidation_bus": {
                "status": if cache_ok && vector_ok { "ready" } else { "degraded" }
            }
        }
    });

    (status_code, Json(report))
}

pub async fn metrics() -> impl IntoResponse {
    let requests = REQUESTS_TOTAL.load(Ordering::Relaxed);
    let exact = EXACT_HITS.load(Ordering::Relaxed);
    let semantic = SEMANTIC_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let rate_limited = RATE_LIMITED_TOTAL.load(Ordering::Relaxed);
    let spend_rejected = SPEND_VELOCITY_REJECTIONS_TOTAL.load(Ordering::Relaxed);
    let duration_ms = REQUEST_DURATION_MS_SUM.load(Ordering::Relaxed);

    let output = format!(
        "# HELP sunborn_requests_total Total number of processed AI requests\n\
         # TYPE sunborn_requests_total counter\n\
         sunborn_requests_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_cache_hits_total Total cache hits partitioned by tier\n\
         # TYPE sunborn_cache_hits_total counter\n\
         sunborn_cache_hits_total{{scope=\"process_local\",tier=\"exact\"}} {}\n\
         sunborn_cache_hits_total{{scope=\"process_local\",tier=\"semantic\"}} {}\n\
         # HELP sunborn_cache_misses_total Total cache misses\n\
         # TYPE sunborn_cache_misses_total counter\n\
         sunborn_cache_misses_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_rate_limit_exceeded_total Total rate limited and quota exceeded requests\n\
         # TYPE sunborn_rate_limit_exceeded_total counter\n\
         sunborn_rate_limit_exceeded_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_spend_velocity_exceeded_total Total requests rejected due to spend velocity budget limits\n\
         # TYPE sunborn_spend_velocity_exceeded_total counter\n\
         sunborn_spend_velocity_exceeded_total{{scope=\"process_local\"}} {}\n\
         # HELP sunborn_request_duration_seconds_total Total request processing duration in seconds\n\
         # TYPE sunborn_request_duration_seconds_total counter\n\
         sunborn_request_duration_seconds_total{{scope=\"process_local\"}} {:.6}\n\
         # HELP sunborn_telemetry_source Telemetry origin scope indicator\n\
         # TYPE sunborn_telemetry_source gauge\n\
         sunborn_telemetry_source{{cluster=\"gateway-primary\",source=\"process_local_worker\"}} 1\n",
        requests, exact, semantic, misses, rate_limited, spend_rejected, (duration_ms as f64) / 1000.0
    );

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        output,
    )
}

pub fn parse_envelope_from_stored(content: &str) -> Option<EncryptedEnvelope> {
    if let Some(hex_str) = content.strip_prefix("sunborn:enc:v1:") {
        if let Ok(bytes) = hex::decode(hex_str) {
            if let Ok(env) = serde_json::from_slice::<EncryptedEnvelope>(&bytes) {
                return Some(env);
            }
        }
    }
    None
}

pub async fn verify_session(
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized("Unauthorized session").into());
    }
    Ok(Json(serde_json::json!({
        "authenticated": true,
        "tenant_id": sec_ctx.tenant_id.0,
        "application_id": sec_ctx.application_id.0,
        "is_admin": sec_ctx.is_admin,
    })))
}

pub async fn execute_request_inner(
    state: &AppState,
    sec_ctx: &SecurityContext,
    mut req: RequestEnvelope,
) -> Result<ResponseEnvelope, AppError> {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();

    // 1. Enforce zero-trust tenant identity (fail-closed):
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized(
            "Authentication required: missing or invalid API key",
        )
        .into());
    }

    // Authenticated context: strictly prevent tenant spoofing
    if req.tenant_id.0.is_empty() {
        req.tenant_id = sec_ctx.tenant_id.clone();
    } else if req.tenant_id != sec_ctx.tenant_id {
        return Err(SunbornError::unauthorized(format!(
            "Tenant spoofing rejected: authenticated context is '{}', but request payload specified '{}'",
            sec_ctx.tenant_id.0, req.tenant_id.0
        )).into());
    }

    if req.application_id.0.is_empty() {
        req.application_id = sec_ctx.application_id.clone();
    } else if req.application_id != sec_ctx.application_id {
        return Err(SunbornError::unauthorized(format!(
            "Application spoofing rejected: authenticated context is '{}', but request payload specified '{}'",
            sec_ctx.application_id.0, req.application_id.0
        ))
        .into());
    }

    // Enforce model authorization permissions
    if !sec_ctx.allows_model(&req.model) {
        return Err(SunbornError::unauthorized(format!(
            "Model '{}' is not authorized for tenant '{}'",
            req.model, sec_ctx.tenant_id.0
        ))
        .into());
    }

    // Zero-trust cryptographic shredding enforcement: reject immediately if tenant is shredded
    if state.crypto_registry.is_shredded(&req.tenant_id) {
        return Err(SunbornError::forbidden(format!(
            "Tenant '{}' has been cryptographically shredded; access is permanently decommissioned",
            req.tenant_id.0
        ))
        .into());
    }

    // 2. Validate payload boundaries and constraints
    req.validate().map_err(AppError::from)?;

    // Snapshot fine-grained lineage watermark lease for transactional write-behind fencing
    let lineage_keys: Vec<String> = req
        .dependency_snapshots
        .iter()
        .map(|d| d.source_id.to_string())
        .collect();
    let watermark_read_view = state.watermark_fencer.snapshot_read_view(&lineage_keys);

    // Enforce tenant spend velocity ceiling via atomic pre-authorization reservation (K-007)
    let pricing = ModelPricing::for_model(&req.model);
    let estimated_prompt_tokens = req.prompt.split_whitespace().count() as u32;
    let estimated_completion_tokens = 500u32;
    let estimated_cost = FinOpsEngine::calculate_token_cost_avoided(
        &pricing,
        estimated_prompt_tokens,
        estimated_completion_tokens,
    );

    let reservation_id = match state
        .spend_velocity_controller
        .reserve_spend(&req.tenant_id.0, estimated_cost)
    {
        Ok(id) => id,
        Err((committed, ceiling)) => {
            SPEND_VELOCITY_REJECTIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
            RATE_LIMITED_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(SunbornError::rate_limit(format!(
                "Spend velocity ceiling of ${:.2} exceeded for tenant '{}' (total committed: ${:.2})",
                ceiling, req.tenant_id.0, committed
            ))
            .into());
        }
    };

    // 3. Compute deterministic Cache Identity (C1)
    let identity = IdentityEngine::compute_identity(&req);
    tracing::info!(
        request_id = %req.request_id.0,
        tenant = %req.tenant_id.0,
        cache_key = %identity.key,
        "Evaluating multi-tier cache pipeline"
    );

    // 4. Exact Cache Lookup (C0 / C1)
    if state.config.cache.exact_cache_enabled {
        if let Ok(Some(cached_entry)) = state.cache.get(&identity.key).await {
            EXACT_HITS.fetch_add(1, Ordering::Relaxed);
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::info!(
                request_id = %req.request_id.0,
                cache_key = %identity.key,
                "Exact cache HIT (C0/C1)"
            );

            let cert = match cached_entry.metadata.get("certificate").and_then(|v| {
                serde_json::from_value::<sunborn_core::ReuseCertificate>(v.clone()).ok()
            }) {
                Some(c) => c,
                None => {
                    tracing::warn!(
                        request_id = %req.request_id.0,
                        cache_key = %identity.key,
                        "Exact cache entry lacks a valid cryptographic ReuseCertificate -> Abstaining to Revalidate"
                    );
                    state
                        .spend_velocity_controller
                        .release_reservation(&req.tenant_id.0, reservation_id);
                    return Ok(ResponseEnvelope {
                        request_id: req.request_id,
                        tenant_id: req.tenant_id,
                        application_id: req.application_id,
                        decision: DecisionRecord {
                            action: DecisionAction::Revalidate,
                            cache_path: CachePath::Exact,
                            confidence: 0.5,
                            risk_score: 1.0,
                            reason_code: "CERTIFICATE_MISSING_OR_INVALID".to_string(),
                            latency_saved_ms: None,
                            policy_version: Some(
                                state.learning_controller.get_active_policy_version(),
                            ),
                            policy_id: Some(state.learning_controller.get_active_policy_id()),
                        },
                        content: cached_entry.content,
                        cached: false,
                        cache_key: Some(identity.key),
                        created_at: Utc::now(),
                        duration_ms,
                        certificate: None,
                    });
                }
            };

            // Enforce proof-carrying admissibility contract at runtime:
            let source_epochs: Vec<(uuid::Uuid, u64)> = req
                .dependency_snapshots
                .iter()
                .map(|s| (s.source_id, s.snapshot_epoch))
                .collect();
            if let Err(admissibility_err) = cert.verify_admissibility(
                &state.learning_hmac_secret,
                &req.tenant_id,
                &req.application_id,
                &req.model,
                Some(&identity.hash),
                Utc::now(),
                req.knowledge_epoch.unwrap_or(1),
                &source_epochs,
            ) {
                tracing::warn!(
                    request_id = %req.request_id.0,
                    error = %admissibility_err,
                    "Exact cache candidate failed proof-carrying admissibility verification -> Evicting entry and falling through to origin"
                );
                let cache_clone = state.cache.clone();
                let key_to_evict = identity.key.clone();
                tokio::spawn(async move {
                    let _ = cache_clone.delete(&key_to_evict).await;
                });
                // Fall through to semantic cache or origin execution (do NOT return unverified content)
            } else {
                let certificate = Some(cert);

                // Authenticated Decryption on Read Path (ER-001)
                let envelope_opt = cached_entry
                    .metadata
                    .get("encrypted_envelope")
                    .and_then(|v| serde_json::from_value::<EncryptedEnvelope>(v.clone()).ok())
                    .or_else(|| parse_envelope_from_stored(&cached_entry.content));

                let content = if let Some(envelope) = envelope_opt {
                    match state.crypto_registry.decrypt(&envelope, "cache_admission") {
                        Ok(plain_bytes) => {
                            String::from_utf8(plain_bytes).unwrap_or(cached_entry.content)
                        }
                        Err(err) => {
                            tracing::warn!(tenant = %req.tenant_id.0, error = %err, "TDEK decryption failed or key shredded -> Rejecting cached entry");
                            state
                                .spend_velocity_controller
                                .release_reservation(&req.tenant_id.0, reservation_id);
                            return Err(SunbornError::forbidden(format!(
                                "Tenant '{}' key shredded or unprovisioned",
                                req.tenant_id.0
                            ))
                            .into());
                        }
                    }
                } else if cached_entry.content.starts_with("sunborn:enc:v1:") {
                    state
                        .spend_velocity_controller
                        .release_reservation(&req.tenant_id.0, reservation_id);
                    return Err(SunbornError::forbidden(format!(
                        "Tenant '{}' key shredded or unprovisioned",
                        req.tenant_id.0
                    ))
                    .into());
                } else {
                    cached_entry.content
                };

                state
                    .spend_velocity_controller
                    .release_reservation(&req.tenant_id.0, reservation_id);
                REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
                return Ok(ResponseEnvelope {
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
                        policy_version: Some(state.learning_controller.get_active_policy_version()),
                        policy_id: Some(state.learning_controller.get_active_policy_id()),
                    },
                    content,
                    cached: true,
                    cache_key: Some(identity.key),
                    created_at: Utc::now(),
                    duration_ms,
                    certificate,
                });
            }
        }
    }

    // 5. Semantic Cache Candidate Evaluation (C2) - Dynamically governed by Learning Controller
    if state.config.cache.semantic_cache_enabled
        && state.circuit_breaker.status().await != sunborn_cache::CircuitStatus::Open
    {
        'semantic_eval: {
            let req_hash =
                u64::from_str_radix(&identity.hash[..16.min(identity.hash.len())], 16).unwrap_or(0);
            let weights = state
                .learning_controller
                .select_policy_for_request(req_hash);
            if let Ok(outcome) = state
                .decision_engine
                .evaluate_with_thresholds(
                    &req,
                    weights.min_similarity_threshold,
                    weights.max_risk_threshold,
                )
                .await
            {
                if outcome.decision.action == DecisionAction::SafeHit {
                    if let Some(content) = outcome.matched_content {
                        SEMANTIC_HITS.fetch_add(1, Ordering::Relaxed);
                        let duration_ms = start.elapsed().as_millis() as u64;
                        tracing::info!(
                            request_id = %req.request_id.0,
                            confidence = outcome.decision.confidence,
                            "Semantic cache HIT (C2)"
                        );

                        let mut decision = outcome.decision;
                        decision.policy_id = Some(state.learning_controller.get_active_policy_id());
                        decision.policy_version =
                            Some(state.learning_controller.get_active_policy_version());

                        let cert = match outcome.matched_certificate {
                            Some(c) => c,
                            None => {
                                tracing::warn!(
                                    request_id = %req.request_id.0,
                                    "Semantic cache match lacks a verified ReuseCertificate -> Treating as MISS and falling through to origin"
                                );
                                break 'semantic_eval;
                            }
                        };

                        // 1. Authoritative Server-Side Multi-Source Lease Check (Finding 2)
                        let is_fresh = state
                            .epoch_lease_manager
                            .validate_multi_source_leases(&cert.source_snapshots)
                            .await;
                        if !is_fresh {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                "Semantic cache candidate dependencies modified in authoritative epoch registry -> Treating as MISS and falling through to origin"
                            );
                            break 'semantic_eval;
                        }

                        // Runtime proof-carrying admissibility check:
                        let mut source_epochs: Vec<(uuid::Uuid, u64)> = req
                            .dependency_snapshots
                            .iter()
                            .map(|s| (s.source_id, s.snapshot_epoch))
                            .collect();
                        for snap in &cert.source_snapshots {
                            if !source_epochs.iter().any(|(id, _)| *id == snap.source_id) {
                                source_epochs.push((snap.source_id, snap.snapshot_epoch));
                            }
                        }
                        if let Err(admissibility_err) = cert.verify_admissibility(
                            &state.learning_hmac_secret,
                            &req.tenant_id,
                            &req.application_id,
                            &req.model,
                            None,
                            Utc::now(),
                            req.knowledge_epoch.unwrap_or(1),
                            &source_epochs,
                        ) {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                error = %admissibility_err,
                                "Semantic cache candidate failed proof-carrying admissibility verification -> Treating as MISS and falling through to origin"
                            );
                            break 'semantic_eval;
                        }

                        let certificate = Some(cert);

                        // Authenticated Decryption on Semantic Read Path (K-004)
                        let envelope_opt = outcome
                            .matched_metadata
                            .as_ref()
                            .and_then(|m| m.get("encrypted_envelope"))
                            .and_then(|v| {
                                serde_json::from_value::<EncryptedEnvelope>(v.clone()).ok()
                            })
                            .or_else(|| parse_envelope_from_stored(&content));

                        let decrypted_content = if let Some(envelope) = envelope_opt {
                            match state.crypto_registry.decrypt(&envelope, "cache_admission") {
                                Ok(plain_bytes) => {
                                    String::from_utf8(plain_bytes).unwrap_or(content)
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        tenant = %req.tenant_id.0,
                                        error = %err,
                                        "TDEK decryption failed on semantic hit or key shredded -> Rejecting cached entry"
                                    );
                                    state
                                        .spend_velocity_controller
                                        .release_reservation(&req.tenant_id.0, reservation_id);
                                    return Err(SunbornError::forbidden(format!(
                                        "Tenant '{}' key shredded or unprovisioned",
                                        req.tenant_id.0
                                    ))
                                    .into());
                                }
                            }
                        } else if content.starts_with("sunborn:enc:v1:") {
                            state
                                .spend_velocity_controller
                                .release_reservation(&req.tenant_id.0, reservation_id);
                            return Err(SunbornError::forbidden(format!(
                                "Tenant '{}' key shredded or unprovisioned",
                                req.tenant_id.0
                            ))
                            .into());
                        } else {
                            content
                        };

                        state
                            .spend_velocity_controller
                            .release_reservation(&req.tenant_id.0, reservation_id);

                        REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
                        return Ok(ResponseEnvelope {
                            request_id: req.request_id,
                            tenant_id: req.tenant_id,
                            application_id: req.application_id,
                            decision,
                            content: decrypted_content,
                            cached: true,
                            cache_key: outcome.matched_id,
                            created_at: Utc::now(),
                            duration_ms,
                            certificate,
                        });
                    }
                }
            }
        }
    }

    // 6. Cache MISS -> Origin Execution
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        request_id = %req.request_id.0,
        "Cache MISS -> Forwarding to origin provider"
    );

    let origin_result = match state.origin_provider.execute(&req).await {
        Ok(res) => res,
        Err(err) => {
            state
                .spend_velocity_controller
                .release_reservation(&req.tenant_id.0, reservation_id);
            return Err(err.into());
        }
    };
    let origin_content = origin_result.content;

    let pricing = ModelPricing::for_model(&req.model);
    let actual_cost = FinOpsEngine::calculate_token_cost_avoided(
        &pricing,
        origin_result.prompt_tokens,
        origin_result.completion_tokens,
    );
    // Settle the atomic reservation with actual origin cost
    state
        .spend_velocity_controller
        .settle_spend(&req.tenant_id.0, reservation_id, actual_cost);

    let origin_ledger = sunborn_db::models::FinOpsLedgerRecord {
        tenant_id: req.tenant_id.0.clone(),
        application_id: req.application_id.0.clone(),
        model: req.model.clone(),
        cache_tier: "origin".to_string(),
        prompt_tokens_avoided: 0,
        completion_tokens_avoided: 0,
        cost_avoided_usd: 0.0,
        storage_cost_usd: 0.0,
        net_savings_usd: -actual_cost,
        recorded_at: Utc::now(),
    };
    let _ = state.db.record_finops_ledger(origin_ledger).await;

    // 7. Cache Admission Engine (Transactional Freshness Fence):
    let is_lease_valid = state
        .epoch_lease_manager
        .validate_multi_source_leases(&req.dependency_snapshots)
        .await;
    let is_watermark_valid = state
        .watermark_fencer
        .validate_and_commit(&watermark_read_view);

    if !is_lease_valid || !is_watermark_valid {
        tracing::warn!(
            request_id = %req.request_id.0,
            is_lease_valid = %is_lease_valid,
            is_watermark_valid = %is_watermark_valid,
            "Transactional freshness fence violation detected prior to cache writeback -> Aborting stale cache admission"
        );
    } else {
        let prompt_risk = sunborn_semantic::DecisionEngine::<
            std::sync::Arc<dyn sunborn_semantic::EmbeddingProvider>,
            std::sync::Arc<dyn sunborn_semantic::VectorStore>,
        >::assess_risk(&req.prompt);

        let admission_cert = sunborn_core::ReuseCertificate::new_signed(
            req.tenant_id.clone(),
            req.application_id.clone(),
            &identity.hash,
            &req.model,
            req.prompt.split_whitespace().count() as u32,
            req.dependency_snapshots.clone(),
            sunborn_core::SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.0,
                calibrated_risk_bound: 0.25,
                heuristic_risk_score: prompt_risk,
            },
            sunborn_core::ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now()
                    + chrono::Duration::seconds(state.config.cache.default_ttl_secs as i64),
                monotonic_epoch_lease: req.knowledge_epoch.unwrap_or(1),
            },
            &state.learning_hmac_secret,
        );

        // Authenticated Encryption on Write Path (K-003: Fail-closed on encryption failure)
        let maybe_envelope = if state.crypto_registry.is_shredded(&req.tenant_id) {
            tracing::warn!(tenant = %req.tenant_id.0, "Tenant key shredded -> Aborting cache writeback (fail-closed)");
            None
        } else {
            if !state.crypto_registry.is_active(&req.tenant_id) {
                let _ = state.crypto_registry.ensure_provisioned(&req.tenant_id);
            }
            match state.crypto_registry.encrypt(
                &req.tenant_id,
                origin_content.as_bytes(),
                "cache_admission",
            ) {
                Ok(env) => Some(env),
                Err(err) => {
                    tracing::warn!(tenant = %req.tenant_id.0, error = %err, "TDEK encryption failed on admission -> Aborting cache writeback (fail-closed)");
                    None
                }
            }
        };

        if let Some(env) = maybe_envelope {
            let stored_content = format!(
                "sunborn:enc:v1:{}",
                hex::encode(serde_json::to_vec(&env).unwrap_or_default())
            );
            let envelope_meta = serde_json::to_value(&env).ok();

            // a) Save in exact cache with embedded certificate metadata
            let mut entry = CacheEntry::new(
                &identity.key,
                &stored_content,
                Some(state.config.cache.default_ttl_secs),
            );
            if let Ok(cert_val) = serde_json::to_value(&admission_cert) {
                entry.metadata["certificate"] = cert_val;
            }
            if let Some(ref env_val) = envelope_meta {
                entry.metadata["encrypted_envelope"] = env_val.clone();
            }
            let _ = state.cache.set(entry.clone()).await;

            // Bind into knowledge dependency graph and graph memory cache
            for snap in &req.dependency_snapshots {
                let _ = state
                    .dependency_graph
                    .record_dependency(snap.source_id, &identity.key)
                    .await;
            }
            if let Some(ref gmc) = state.graph_memory_cache {
                let source_ids: Vec<Uuid> = req
                    .dependency_snapshots
                    .iter()
                    .map(|s| s.source_id)
                    .collect();
                let doc_hashes: Vec<u64> = req
                    .dependency_snapshots
                    .iter()
                    .map(|s| {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        std::hash::Hash::hash(&s.source_id, &mut hasher);
                        std::hash::Hasher::finish(&hasher)
                    })
                    .collect();
                let record_id = {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&identity.key, &mut hasher);
                    std::hash::Hasher::finish(&hasher)
                };
                let _ = gmc
                    .set_with_lineage(entry, &source_ids, &doc_hashes, record_id)
                    .await;
            }

            // b) Admit into semantic vector store
            if let Ok(vector) = state.selected_embedder.embed(&req.prompt).await {
                let mut meta = serde_json::json!({
                    "model": req.model,
                    "certificate": serde_json::to_value(&admission_cert).unwrap_or_default(),
                });
                if let Some(ref env_val) = envelope_meta {
                    meta["encrypted_envelope"] = env_val.clone();
                }
                let _ = state
                    .vector_store
                    .insert(VectorEntry {
                        id: identity.key.clone(),
                        tenant_id: req.tenant_id.clone(),
                        application_id: req.application_id.clone(),
                        prompt: req.prompt.clone(),
                        content: stored_content.clone(),
                        vector,
                        metadata: meta,
                        created_at: Utc::now(),
                    })
                    .await;
            }

            // c) Save to authoritative PostgreSQL system of record
            let db_res = state
                .db
                .insert_cache_entry(sunborn_db::models::DurableCacheRecord {
                    key: identity.key.clone(),
                    tenant_id: req.tenant_id.0.clone(),
                    application_id: req.application_id.0.clone(),
                    model: req.model.clone(),
                    policy_version: "v1".to_string(),
                    hash: identity.hash.clone(),
                    prompt_canonical: req.prompt.clone(),
                    content: stored_content.clone(),
                    hit_count: 0,
                    created_at: Utc::now(),
                    expires_at: Some(
                        Utc::now()
                            + chrono::Duration::seconds(state.config.cache.default_ttl_secs as i64),
                    ),
                    last_accessed_at: Utc::now(),
                })
                .await;

            if let Err(e) = db_res {
                tracing::error!(
                    key = %identity.key,
                    tenant = %req.tenant_id.0,
                    error = %e,
                    "Authoritative database persistence failed on cache admission"
                );
                let is_prod = sunborn_core::is_production_mode();
                if is_prod && state.config.cache.strict_durability {
                    return Err(SunbornError::database(format!(
                        "Strict durability violation: failed to persist authoritative cache entry to database: {e}"
                    ))
                    .into());
                }
            }
        } else {
            tracing::info!(tenant = %req.tenant_id.0, "Cache admission skipped: encryption unavailable, fail-closed enforced");
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    let resp = ResponseEnvelope {
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
            policy_version: Some(state.learning_controller.get_active_policy_version()),
            policy_id: Some(state.learning_controller.get_active_policy_id()),
        },
        content: origin_content,
        cached: false,
        cache_key: Some(identity.key),
        created_at: Utc::now(),
        duration_ms,
        certificate: None,
    };
    REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
    Ok(resp)
}

pub async fn execute_request(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(req): Json<RequestEnvelope>,
) -> Result<Json<ResponseEnvelope>, AppError> {
    let resp = execute_request_inner(&state, &sec_ctx, req).await?;
    Ok(Json(resp))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub response_format: Option<serde_json::Value>,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
    pub sunborn_cached: bool,
    pub sunborn_tier: String,
}

pub async fn chat_completions(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(chat_req): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, AppError> {
    if chat_req.messages.is_empty() {
        return Err(SunbornError::bad_request("Messages array cannot be empty").into());
    }

    let system_prompt = chat_req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    let prompt = chat_req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_else(|| {
            chat_req
                .messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        });

    let mut req = RequestEnvelope::new(
        sec_ctx.tenant_id.0.clone(),
        sec_ctx.application_id.0.clone(),
        chat_req.model.clone(),
        prompt,
    );
    req.messages = chat_req
        .messages
        .iter()
        .map(|m| sunborn_core::model::ChatMessage::new(&m.role, &m.content))
        .collect();
    req.system_prompt = system_prompt;
    req.temperature = chat_req.temperature;
    req.top_p = chat_req.top_p;
    req.max_tokens = chat_req.max_tokens;
    if let Some(stop) = chat_req.stop {
        req.stop_sequences = stop;
    }
    if let Some(tools) = chat_req.tools {
        for t in tools {
            if let Some(name) = t
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                let desc = t
                    .get("function")
                    .and_then(|f| f.get("description"))
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
                let params = t
                    .get("function")
                    .and_then(|f| f.get("parameters"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                req.tools.push(sunborn_core::model::ToolDefinition {
                    name: name.to_string(),
                    description: desc,
                    parameters: params,
                });
            }
        }
    }
    if let Some(rf) = chat_req.response_format {
        req.response_format = Some(rf.to_string());
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;

    let prompt_total_chars: usize = chat_req.messages.iter().map(|m| m.content.len()).sum();
    let prompt_tokens = (prompt_total_chars / 4).max(1) as u32;
    let completion_tokens = (resp_envelope.content.len() / 4).max(1) as u32;

    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact".to_string(),
            CachePath::Semantic => "c2_semantic".to_string(),
            _ => "cached".to_string(),
        }
    } else {
        "origin".to_string()
    };

    Ok(Json(ChatCompletionResponse {
        id: format!("chatcmpl-sunborn-{}", resp_envelope.request_id.0),
        object: "chat.completion".to_string(),
        created: Utc::now().timestamp(),
        model: chat_req.model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: resp_envelope.content,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: ChatUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        sunborn_cached: resp_envelope.cached,
        sunborn_tier: tier_str,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessageItem {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessageItem>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system: Option<serde_json::Value>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<AnthropicContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
    pub sunborn_cached: bool,
    pub sunborn_tier: String,
}

pub async fn anthropic_messages(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(anthropic_req): Json<AnthropicMessagesRequest>,
) -> Result<Json<AnthropicMessagesResponse>, AppError> {
    if anthropic_req.messages.is_empty() {
        return Err(SunbornError::bad_request("Messages array cannot be empty").into());
    }

    let system_prompt = anthropic_req.system.as_ref().and_then(|s| {
        if let Some(text) = s.as_str() {
            Some(text.to_string())
        } else if let Some(arr) = s.as_array() {
            let combined: Vec<String> = arr
                .iter()
                .filter_map(|b| {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                })
                .collect();
            if combined.is_empty() {
                None
            } else {
                Some(combined.join("\n"))
            }
        } else {
            None
        }
    });

    let mut user_prompt = String::new();
    let mut core_messages = Vec::new();

    for m in &anthropic_req.messages {
        let text_content = if let Some(text) = m.content.as_str() {
            text.to_string()
        } else if let Some(arr) = m.content.as_array() {
            arr.iter()
                .filter_map(|b| {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            m.content.to_string()
        };

        if m.role == "user" {
            user_prompt = text_content.clone();
        }
        core_messages.push(sunborn_core::model::ChatMessage::new(
            &m.role,
            &text_content,
        ));
    }

    if user_prompt.is_empty() {
        user_prompt = core_messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content.as_deref().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut req = RequestEnvelope::new(
        sec_ctx.tenant_id.0.clone(),
        sec_ctx.application_id.0.clone(),
        anthropic_req.model.clone(),
        user_prompt,
    );
    req.messages = core_messages;
    req.system_prompt = system_prompt;
    req.temperature = anthropic_req.temperature;
    req.top_p = anthropic_req.top_p;
    req.max_tokens = anthropic_req.max_tokens;
    if let Some(stop) = anthropic_req.stop_sequences {
        req.stop_sequences = stop;
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;

    let prompt_total_chars: usize = anthropic_req
        .messages
        .iter()
        .map(|m| m.content.to_string().len())
        .sum();
    let input_tokens = (prompt_total_chars / 4).max(1) as u32;
    let output_tokens = (resp_envelope.content.len() / 4).max(1) as u32;

    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact".to_string(),
            CachePath::Semantic => "c2_semantic".to_string(),
            _ => "cached".to_string(),
        }
    } else {
        "origin".to_string()
    };

    Ok(Json(AnthropicMessagesResponse {
        id: format!("msg_sunborn_{}", resp_envelope.request_id.0),
        msg_type: "message".to_string(),
        role: "assistant".to_string(),
        model: anthropic_req.model,
        content: vec![AnthropicContentBlock {
            block_type: "text".to_string(),
            text: resp_envelope.content,
        }],
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens,
            output_tokens,
        },
        sunborn_cached: resp_envelope.cached,
        sunborn_tier: tier_str,
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
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<InvalidationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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

    // Immediate Real-Time Causal (RTC) cache purge on graph memory cache
    if let Some(ref gmc) = state.graph_memory_cache {
        let _ = gmc.invalidate_source_rtc(&payload.source_id).await;
    }

    let bumped_epoch = state
        .epoch_lease_manager
        .bump_epoch(payload.source_id)
        .await;

    let bumped_watermark = state
        .watermark_fencer
        .advance_watermark(&payload.source_id.to_string());

    Ok(Json(json!({
        "status": "processed",
        "source_id": payload.source_id,
        "bumped_epoch": bumped_epoch,
        "bumped_watermark": bumped_watermark,
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
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<DependencyRecordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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

#[derive(Deserialize)]
pub struct GraphRtcInvalidateRequest {
    #[serde(default)]
    pub source_ids: Vec<Uuid>,
    #[serde(default)]
    pub doc_hashes: Vec<u64>,
}

pub async fn get_graph_stats(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let (total_sources, total_edges) = state.dependency_graph.get_graph_stats().await;
    let (doc_count, total_assocs) = if let Some(ref gmc) = state.graph_memory_cache {
        (
            gmc.lineage_graph().document_count(),
            gmc.lineage_graph().total_associations(),
        )
    } else {
        (0, 0)
    };

    Ok(Json(json!({
        "status": "healthy",
        "graph_dependency_cache_enabled": state.graph_memory_cache.is_some(),
        "total_sources": total_sources,
        "total_edges": total_edges,
        "total_doc_hashes": doc_count,
        "total_doc_associations": total_assocs
    })))
}

pub async fn get_graph_lineage(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let upstream_sources = state.dependency_graph.get_upstream_sources(&key).await;
    let is_cached = state.cache.exists(&key).await.unwrap_or(false);

    Ok(Json(json!({
        "cache_key": key,
        "is_cached": is_cached,
        "upstream_sources": upstream_sources,
        "upstream_source_count": upstream_sources.len()
    })))
}

pub async fn invalidate_graph_rtc(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<GraphRtcInvalidateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let mut purged_keys = Vec::new();
    let mut affected_doc_records_count = 0;

    for src in &payload.source_ids {
        if let Some(ref gmc) = state.graph_memory_cache {
            if let Ok(keys) = gmc.invalidate_source_rtc(src).await {
                purged_keys.extend(keys);
            }
        } else {
            let keys = state.dependency_graph.get_affected_cache_keys(src).await;
            for k in &keys {
                let _ = state.cache.delete(k).await;
            }
            state.dependency_graph.clear_dependencies(src).await;
            purged_keys.extend(keys);
        }
        state.epoch_lease_manager.bump_epoch(*src).await;
        state.watermark_fencer.advance_watermark(&src.to_string());
    }

    if !payload.doc_hashes.is_empty() {
        if let Some(ref gmc) = state.graph_memory_cache {
            let affected = gmc.invalidate_docs_rtc(&payload.doc_hashes);
            affected_doc_records_count = affected.len() as usize;
        }
    }

    purged_keys.sort();
    purged_keys.dedup();

    Ok(Json(json!({
        "status": "purged",
        "purged_cache_keys": purged_keys,
        "purged_count": purged_keys.len(),
        "affected_doc_records_count": affected_doc_records_count
    })))
}

pub async fn get_cache(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<CacheEntry>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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
    Extension(sec_ctx): Extension<SecurityContext>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureResidualCounts {
    pub db: usize,
    pub exact_cache: usize,
    pub vector_store: usize,
    pub execution_store: usize,
    pub learning_store: usize,
}

pub async fn verify_multi_store_erasure_integrity(
    state: &AppState,
    tenant_id: &str,
) -> (bool, ErasureResidualCounts) {
    let tenant = TenantId::new(tenant_id);
    let mut verification_failed = false;

    let db_res = match state.db.count_tenant_entries(tenant_id).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Database count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let prefix_sunborn = format!("sunborn:{}:", tenant_id);
    let prefix_legacy = format!("tenant:{}:", tenant_id);
    let exact_res_1 = match state.cache.count_by_prefix(&prefix_sunborn).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Cache prefix check failed during erasure verification");
            verification_failed = true;
            1
        }
    };
    let exact_res_2 = match state.cache.count_by_prefix(&prefix_legacy).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Legacy cache prefix check failed during erasure verification");
            verification_failed = true;
            1
        }
    };
    let exact_res = exact_res_1 + exact_res_2;

    let vector_res = match state.vector_store.count(&tenant).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Vector store count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let execution_res = match state.execution_store.count_by_tenant(&tenant).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!(tenant = %tenant_id, error = %err, "Execution store count check failed during erasure verification");
            verification_failed = true;
            1
        }
    };

    let learning_res = state.learning_store.tenant_event_count(&tenant);

    let residuals = ErasureResidualCounts {
        db: db_res,
        exact_cache: exact_res,
        vector_store: vector_res,
        execution_store: execution_res,
        learning_store: learning_res,
    };

    let all_zero = !verification_failed
        && db_res == 0
        && exact_res == 0
        && vector_res == 0
        && execution_res == 0
        && learning_res == 0;

    if !all_zero {
        tracing::warn!(
            tenant = %tenant_id,
            ?residuals,
            verification_failed,
            "Multi-store erasure integrity verification FAILED: residual records or check errors found"
        );
    } else {
        tracing::info!(
            tenant = %tenant_id,
            "Multi-store erasure integrity verification SUCCEEDED: 0 residual records across all 5 tiers"
        );
    }

    (all_zero, residuals)
}

pub async fn delete_tenant(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

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
    let prefix_sunborn = format!("sunborn:{}:", tenant_id);
    let exact_deleted_sunborn = state
        .cache
        .delete_by_prefix(&prefix_sunborn)
        .await
        .map_err(AppError::from)?;
    let prefix_legacy = format!("tenant:{}:", tenant_id);
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
    let learning_deleted = state.learning_store.purge_tenant(&tenant);
    let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);

    // 1. Independent Read-After-Delete Verification Pass
    let (verified_erasure, residuals) =
        verify_multi_store_erasure_integrity(&state, &tenant_id).await;
    let final_status = if verified_erasure {
        "erased"
    } else {
        "partial_failure"
    };

    tracing::info!(
        tenant = %tenant_id,
        shred_duration_nanos,
        db_deleted,
        exact_deleted,
        vector_deleted,
        execution_deleted,
        learning_deleted,
        verified_multi_store_erasure = verified_erasure,
        "Completed tenant cascade offboarding"
    );

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "status": final_status,
        "cryptographic_shred_time_ns": shred_duration_nanos,
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

pub async fn trigger_rebuild(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    let entries = state
        .db
        .scan_cache_entries(100000, 0)
        .await
        .map_err(AppError::from)?;
    let total = entries.len();
    let mut exact_restored = 0;
    let mut vector_restored = 0;

    for record in entries {
        // 1. Repopulate exact cache
        let ttl_secs = record
            .expires_at
            .map(|exp| (exp - Utc::now()).num_seconds().max(1) as u64)
            .unwrap_or(state.config.cache.default_ttl_secs);

        let prompt_risk = sunborn_semantic::DecisionEngine::<
            std::sync::Arc<dyn sunborn_semantic::EmbeddingProvider>,
            std::sync::Arc<dyn sunborn_semantic::VectorStore>,
        >::assess_risk(&record.prompt_canonical);

        let req_hash = record.hash.split(':').next_back().unwrap_or(&record.hash);

        let restored_cert = sunborn_core::ReuseCertificate::new_signed(
            TenantId::new(record.tenant_id.clone()),
            sunborn_core::model::ApplicationId::new(record.application_id.clone()),
            req_hash,
            &record.model,
            record.prompt_canonical.split_whitespace().count() as u32,
            vec![],
            sunborn_core::SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.0,
                calibrated_risk_bound: 0.25,
                heuristic_risk_score: prompt_risk,
            },
            sunborn_core::ValidityWindow {
                issued_at: record.created_at,
                expires_at: record
                    .expires_at
                    .unwrap_or_else(|| Utc::now() + chrono::Duration::seconds(ttl_secs as i64)),
                monotonic_epoch_lease: 1,
            },
            &state.learning_hmac_secret,
        );

        let mut entry = CacheEntry::new(&record.key, &record.content, Some(ttl_secs));
        if let Ok(cert_val) = serde_json::to_value(&restored_cert) {
            entry.metadata["certificate"] = cert_val.clone();
        }
        if let Some(env) = parse_envelope_from_stored(&record.content) {
            if let Ok(env_val) = serde_json::to_value(&env) {
                entry.metadata["encrypted_envelope"] = env_val;
            }
        }
        if state.cache.set(entry).await.is_ok() {
            exact_restored += 1;
        }

        // 2. Repopulate vector store
        if let Ok(vec) = state
            .selected_embedder
            .embed(&record.prompt_canonical)
            .await
        {
            let mut meta = serde_json::json!({
                "model": record.model,
                "certificate": serde_json::to_value(&restored_cert).unwrap_or_default(),
            });
            if let Some(env) = parse_envelope_from_stored(&record.content) {
                if let Ok(env_val) = serde_json::to_value(&env) {
                    meta["encrypted_envelope"] = env_val;
                }
            }
            let ventry = VectorEntry {
                id: record.key.clone(),
                tenant_id: TenantId::new(record.tenant_id),
                application_id: sunborn_core::model::ApplicationId::new(record.application_id),
                prompt: record.prompt_canonical,
                content: record.content,
                vector: vec,
                metadata: meta,
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
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
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

pub async fn chaos_trip(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    state.circuit_breaker.trip().await;
    Ok(Json(json!({
        "status": "tripped",
        "circuit_status": "Open"
    })))
}

pub async fn chaos_reset(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let quorum_key = headers
        .get("x-quorum-admin-key")
        .and_then(|v| v.to_str().ok());
    sec_ctx
        .require_admin_quorum(quorum_key, &state.api_key_registry)
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;
    state.circuit_breaker.reset().await;
    Ok(Json(json!({
        "status": "reset",
        "circuit_status": "Closed"
    })))
}

#[derive(Deserialize)]
pub struct ShredTenantPayload {
    pub tenant_id: String,
}

#[derive(serde::Deserialize)]
pub struct ProvisionTenantPayload {
    pub tenant_id: String,
}

pub async fn provision_tenant(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<ProvisionTenantPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let tenant = TenantId::new(&payload.tenant_id);
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

    Ok(Json(serde_json::json!({
        "status": "provisioned",
        "tenant_id": payload.tenant_id,
        "kms_provider": provider_name,
        "durable_persistence": true,
    })))
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
    let learning_deleted = state.learning_store.purge_tenant(&tenant);
    let _credentials_revoked = state.api_key_registry.revoke_tenant_keys(&tenant);

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

    let exact_hits = EXACT_HITS.load(Ordering::Relaxed);
    let semantic_hits = SEMANTIC_HITS.load(Ordering::Relaxed);
    let total_hits = exact_hits + semantic_hits;

    let gpt4o_gross_per_hit = FinOpsEngine::calculate_token_cost_avoided(&gpt4o, 1000, 500);
    let gpt4o_saving_per_hit = FinOpsEngine::calculate_net_savings(&gpt4o, 1000, 500);
    let claude_saving_per_hit = FinOpsEngine::calculate_net_savings(&claude, 1000, 500);

    let embed_cost_per_query = FinOpsEngine::LOCAL_EMBED_COST_PER_CALL;
    let cache_overhead_per_query = FinOpsEngine::CACHE_LOOKUP_OVERHEAD_PER_CALL;

    // Honest empirical calculation: when total_hits is 0, total savings is $0.00
    let total_gpt4o_gross_savings = gpt4o_gross_per_hit * (total_hits as f64);
    let total_gpt4o_net_savings = gpt4o_saving_per_hit * (total_hits as f64);
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
            "gpt4o_gross_cost_avoided_per_query_usd": gpt4o_gross_per_hit,
            "gpt4o_cost_avoided_per_query_usd": gpt4o_saving_per_hit,
            "gpt4o_net_savings_per_query_usd": gpt4o_saving_per_hit,
            "embedding_inference_cost_per_query_usd": embed_cost_per_query,
            "cache_lookup_overhead_cost_per_query_usd": cache_overhead_per_query,
            "claude_cost_avoided_per_query_usd": claude_saving_per_hit,
            "total_estimated_gross_savings_usd": total_gpt4o_gross_savings,
            "total_estimated_net_savings_usd": total_gpt4o_net_savings,
            "total_estimated_savings_usd": total_gpt4o_net_savings,
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

#[derive(Deserialize)]
pub struct AttestationPayload {
    pub leaf_hashes: Vec<String>,
}

pub async fn generate_merkle_attestation(
    Json(payload): Json<AttestationPayload>,
) -> impl IntoResponse {
    let (root, proofs) =
        sunborn_execution::MerkleExecutionEngine::build_merkle_tree(&payload.leaf_hashes);
    Json(json!({
        "status": "attested",
        "root_hash": root,
        "leaf_count": payload.leaf_hashes.len(),
        "proofs": proofs
    }))
}

#[derive(Deserialize)]
pub struct VerifyProofPayload {
    #[serde(default)]
    pub proof: Option<sunborn_execution::MerkleInclusionProof>,
    #[serde(default)]
    pub root_hash: Option<String>,
    #[serde(default)]
    pub merkle_root: Option<String>,
    #[serde(default)]
    pub leaf_hash: Option<String>,
    #[serde(default)]
    pub path: Option<Vec<sunborn_execution::MerkleProofStep>>,
}

pub async fn verify_merkle_attestation(
    Json(payload): Json<VerifyProofPayload>,
) -> impl IntoResponse {
    let proof_opt = payload.proof.or_else(|| {
        let root = payload.root_hash.or(payload.merkle_root)?;
        let leaf = payload.leaf_hash?;
        let path = payload.path.unwrap_or_default();
        Some(sunborn_execution::MerkleInclusionProof {
            leaf_hash: leaf,
            root_hash: root,
            path,
        })
    });

    let (is_valid, root, leaf) = match proof_opt {
        Some(p) => {
            let valid = p.verify();
            (valid, p.root_hash, p.leaf_hash)
        }
        None => (false, String::new(), String::new()),
    };

    Json(json!({
        "is_valid": is_valid,
        "valid": is_valid,
        "root_hash": root,
        "leaf_hash": leaf
    }))
}

#[derive(Deserialize)]
pub struct SpeculativePredictPayload {
    pub current_action: String,
    pub record_transition_to: Option<String>,
}

pub async fn predict_speculative(
    State(state): State<AppState>,
    Json(payload): Json<SpeculativePredictPayload>,
) -> impl IntoResponse {
    if let Some(next_action) = &payload.record_transition_to {
        state
            .predictive_engine
            .record_transition(&payload.current_action, next_action);
    }
    let predictions = state
        .predictive_engine
        .predict_next(&payload.current_action);
    Json(json!({
        "current_action": payload.current_action,
        "predictions": predictions,
        "speculative_cache_warmed": !predictions.is_empty()
    }))
}

pub async fn get_cluster_sync_status(State(state): State<AppState>) -> impl IntoResponse {
    let vc = state.vector_clock.read().unwrap().clone();
    let region = std::env::var("SUNBORN_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let kms_provider = state.crypto_registry.provider().provider_name();
    Json(json!({
        "node_id": "gateway-primary-node-1",
        "region": region,
        "status_observed_at": Utc::now().to_rfc3339(),
        "mode": "standalone_authoritative",
        "nodes_active": 1,
        "cluster_state": "single-node",
        "vector_clock": vc,
        "anti_entropy_protocol": "single-node-epoch",
        "consensus_mode": "single_node_authoritative",
        "kms_provider": kms_provider,
        "spend_velocity_enforcement": "active_enforced"
    }))
}

pub async fn get_autonomic_status(State(state): State<AppState>) -> impl IntoResponse {
    let incidents = state.autonomic_controller.list_incidents();
    let circuit_breaker_status = state.circuit_breaker.status().await;
    Json(json!({
        "autonomic_health": "operational",
        "remediation_mode": "continuous-closed-loop",
        "circuit_breaker_status": format!("{:?}", circuit_breaker_status),
        "total_incidents_resolved": incidents.len(),
        "recent_incidents": incidents
    }))
}

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

        (
            status,
            Json(json!({
                "type": format!("https://sunborn.ai/errors/{:?}", self.0.code).to_lowercase(),
                "title": format!("{:?}", self.0.code),
                "status": status.as_u16(),
                "detail": self.0.message.clone(),
                "error": self.0,
            })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct SecurityEvaluateRequest {
    pub prompt: String,
    pub model: Option<String>,
}

pub async fn security_evaluate(
    State(_state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<SecurityEvaluateRequest>,
) -> impl IntoResponse {
    let lower = payload.prompt.to_lowercase();
    let is_injection = lower.contains("ignore previous")
        || lower.contains("ignore all instructions")
        || lower.contains("reveal internal system")
        || lower.contains("reveal system prompt")
        || lower.contains("bypass security")
        || lower.contains("drop table")
        || lower.contains("dan mode")
        || lower.contains("jailbreak");

    let is_threat = is_injection;
    let rule_matched = if is_injection {
        "OWASP-LLM-01: System Prompt Extraction & Privilege Hijack (Heuristic Match)"
    } else {
        "CLEAN_PASS: Verification Assured"
    };
    let action = if is_threat {
        "DROPPED_BEFORE_CACHE_EVALUATION"
    } else {
        "FORWARDED_TO_EXACT_TIER"
    };
    let heuristic_score = if is_threat { 1.0 } else { 0.0 };

    Json(json!({
        "threatDetected": is_threat,
        "ruleMatched": rule_matched,
        "action": action,
        "evaluationEngine": "heuristic_rule_matcher",
        "detectionMethod": "keyword_pattern_matching",
        "heuristicRiskScore": heuristic_score,
        "conformalScore": heuristic_score,
        "tenantId": sec_ctx.tenant_id.0,
        "timestamp": Utc::now()
    }))
}

pub async fn security_audit(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> impl IntoResponse {
    let records = state.db.scan_cache_entries(10, 0).await.unwrap_or_default();
    let mut audit_events = Vec::new();

    audit_events.push(json!({
        "id": format!("sec_{}", Uuid::new_v4().to_string().chars().take(8).collect::<String>()),
        "event": "ZERO_TRUST_VERIFICATION",
        "origin": "Gateway Ingress",
        "detail": format!("Authenticated tenant {} (application {})", sec_ctx.tenant_id.0, sec_ctx.application_id.0),
        "status": "200_OK",
        "time": "Just now"
    }));

    for rec in records.iter().take(3) {
        audit_events.push(json!({
            "id": format!("sec_{}", Uuid::new_v4().to_string().chars().take(8).collect::<String>()),
            "event": "DURABLE_CACHE_RECORD",
            "origin": "PostgreSQL Store",
            "detail": format!("Cache key {} (hits: {})", rec.key, rec.hit_count),
            "status": "ACTIVE",
            "time": rec.created_at.to_rfc3339()
        }));
    }

    Json(json!({
        "tenant_id": sec_ctx.tenant_id.0,
        "events": audit_events
    }))
}

// -----------------------------------------------------------------------------
// Learning Controller Endpoints (Safe Adaptive Reuse Layers 1-5)
// -----------------------------------------------------------------------------

pub async fn record_learning_feedback(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(event): Json<crate::learning::VerifiedFeedbackEvent>,
) -> Result<impl IntoResponse, AppError> {
    if event.tenant_id != sec_ctx.tenant_id && !sec_ctx.is_admin {
        return Err(SunbornError::unauthorized(
            "Cross-tenant feedback injection forbidden: tenant_id mismatch",
        )
        .into());
    }

    // P0-4: Verify HMAC-SHA256 signature against server HMAC secret
    event
        .verify_signature(&state.learning_hmac_secret)
        .map_err(AppError::from)?;

    // Timestamp freshness verification: reject feedback older than 15 minutes (900 seconds)
    // or skewed into the future by more than 60 seconds
    let now = Utc::now();
    let age_secs = (now - event.timestamp).num_seconds();
    if !(-60..=900).contains(&age_secs) {
        return Err(SunbornError::validation_error(format!(
            "Feedback timestamp expired or skewed beyond acceptable window: age {}s",
            age_secs
        ))
        .into());
    }

    state
        .learning_store
        .record_event(event)
        .map_err(AppError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "FEEDBACK_RECORDED",
            "timestamp": Utc::now()
        })),
    ))
}

pub async fn get_learning_policies(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> impl IntoResponse {
    let active = state.learning_controller.get_active_policy();
    let canary = state.learning_controller.get_canary_status();
    let tenant_events_count = state.learning_store.tenant_event_count(&sec_ctx.tenant_id);
    let total_events = state.learning_store.total_events();
    let audit_trail = state.learning_controller.get_audit_trail();

    Json(json!({
        "tenant_id": sec_ctx.tenant_id.0,
        "active_policy": active,
        "canary_policy": canary.as_ref().map(|(p, pct)| json!({ "policy": p, "percentage": pct })),
        "tenant_feedback_events": tenant_events_count,
        "total_feedback_events": total_events,
        "audit_trail": audit_trail
    }))
}

#[derive(Deserialize)]
pub struct CalibrateRequest {
    pub version: u32,
}

pub async fn calibrate_learning_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<CalibrateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let events = state.learning_store.get_tenant_events(&sec_ctx.tenant_id);
    let candidate = crate::learning::OfflineCalibrator::calibrate_policy(&events, payload.version)
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "POLICY_CALIBRATED",
        "candidate_policy": candidate
    })))
}

#[derive(Deserialize)]
pub struct EvaluatePolicyRequest {
    pub candidate_policy: crate::learning::CandidatePolicy,
}

pub async fn evaluate_learning_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<EvaluatePolicyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let events = state.learning_store.get_tenant_events(&sec_ctx.tenant_id);
    let active_weights = state.learning_controller.get_active_weights();
    let metrics = crate::learning::CounterfactualShadowEvaluator::evaluate_counterfactual(
        &events,
        &active_weights,
        &payload.candidate_policy,
    );

    Ok(Json(json!({
        "status": "COUNTERFACTUAL_EVALUATION_COMPLETED",
        "metrics": metrics
    })))
}

#[derive(Deserialize)]
pub struct StartCanaryRequest {
    pub candidate_policy: crate::learning::CandidatePolicy,
    pub canary_percentage: u8,
}

pub async fn start_learning_canary(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<StartCanaryRequest>,
) -> Result<impl IntoResponse, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    state
        .learning_controller
        .start_canary(
            payload.candidate_policy,
            payload.canary_percentage,
            &sec_ctx.tenant_id.0,
        )
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "CANARY_ROLLOUT_INITIATED",
        "canary_percentage": payload.canary_percentage,
        "timestamp": Utc::now()
    })))
}

pub async fn promote_learning_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    let promoted = state
        .learning_controller
        .complete_promotion(&sec_ctx.tenant_id.0)
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "POLICY_FULLY_PROMOTED",
        "promoted_policy": promoted,
        "timestamp": Utc::now()
    })))
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    pub reason: String,
}

pub async fn rollback_learning_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<RollbackRequest>,
) -> Result<impl IntoResponse, AppError> {
    sec_ctx
        .require_admin()
        .map_err(|e| AppError::from(SunbornError::forbidden(e)))?;

    state
        .learning_controller
        .trigger_rollback(
            crate::learning::RollbackTrigger::ManualIntervention,
            &payload.reason,
        )
        .map_err(AppError::from)?;

    Ok(Json(json!({
        "status": "POLICY_ROLLED_BACK",
        "reason": payload.reason,
        "timestamp": Utc::now()
    })))
}
