//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

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

fn decrypt_stored_content(
    state: &AppState,
    tenant_id: &TenantId,
    stored: &str,
    metadata: Option<&serde_json::Value>,
) -> std::result::Result<String, sunborn_core::SunbornError> {
    let envelope_opt = metadata
        .and_then(|m| m.get("encrypted_envelope"))
        .and_then(|v| serde_json::from_value::<EncryptedEnvelope>(v.clone()).ok())
        .or_else(|| parse_envelope_from_stored(stored));

    if let Some(envelope) = envelope_opt {
        match state.crypto_registry.decrypt(&envelope, "cache_admission") {
            Ok(plain_bytes) => {
                Ok(String::from_utf8(plain_bytes).unwrap_or_else(|_| stored.to_string()))
            }
            Err(err) => {
                tracing::warn!(tenant = %tenant_id.0, error = %err, "TDEK decryption failed or key shredded");
                Err(SunbornError::forbidden(format!(
                    "Tenant '{}' key shredded or unprovisioned",
                    tenant_id.0
                )))
            }
        }
    } else if stored.starts_with("sunborn:enc:v1:") {
        Err(SunbornError::forbidden(format!(
            "Tenant '{}' key shredded or unprovisioned",
            tenant_id.0
        )))
    } else {
        Ok(stored.to_string())
    }
}

fn apply_request_scope_metadata(
    state: &AppState,
    req: &RequestEnvelope,
    meta: &mut serde_json::Value,
) {
    if let Some(session_id) = req.metadata.get("session_id").and_then(|v| v.as_str()) {
        meta["session_id"] = json!(session_id);
    }
    if let Some(user_id) = req.metadata.get("user_id").and_then(|v| v.as_str()) {
        meta["user_id"] = json!(user_id);
    }
    if let Some(ctx_digest) = sunborn_identity::IdentityEngine::compute_context_digest(req) {
        meta["context_digest"] = json!(ctx_digest);
    }
    meta["config_fingerprint"] =
        json!(sunborn_identity::IdentityEngine::compute_config_fingerprint(req));
    let emb_fp = state.selected_embedder.embedding_fingerprint();
    let (emb_model_id, emb_model_version) = sunborn_semantic::embedding_model_parts(&emb_fp);
    meta["embedding_fingerprint"] = json!(emb_fp);
    meta["embedding_model_id"] = json!(emb_model_id);
    meta["embedding_model_version"] = json!(emb_model_version);
}

fn emit_otel_gate_telemetry(
    ast_us: u64,
    conformal_score: f32,
    saved_micropaises: u64,
    cache_tier: &str,
    embedding_model_id: &str,
) {
    tracing::info!(
        "sunborn.gate.ast_us" = ast_us,
        "sunborn.gate.conformal_score" = conformal_score,
        "sunborn.saved_micropaises" = saved_micropaises,
        "sunborn.cache.tier" = cache_tier,
        "sunborn.embedding.model_id" = embedding_model_id,
        "OTel gate telemetry recorded"
    );
}

fn record_learning_telemetry(
    state: &AppState,
    req: &RequestEnvelope,
    similarity_score: f32,
    was_reused: bool,
    latency_cache_ms: f64,
    latency_origin_ms: f64,
) {
    let mut window = sunborn_core::sync::write_lock(&state.telemetry_window);
    window.record(sunborn_learning::ExecutionRecord {
        query_id: req.request_id.0.to_string(),
        tenant_id: req.tenant_id.0.clone(),
        application_id: req.application_id.0.clone(),
        semantic_category: req
            .metadata
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_string(),
        similarity_score,
        was_reused,
        reported_false_hit: false,
        latency_origin_ms,
        latency_cache_ms,
        token_cost_saved_usd: if was_reused { 0.002 } else { 0.0 },
        timestamp_epoch_ms: Utc::now().timestamp_millis(),
    });
}

/// Evaluates the 5-dimensional temporal validity vector (document, permission,
/// config, model, epoch) and linear confidence decay against `TemporalValidityEngine`.
fn is_temporally_fresh(
    state: &AppState,
    metadata: &serde_json::Value,
    tenant_id: &str,
    model: &str,
    source_snapshots: &[sunborn_core::SourceVersionSnapshot],
) -> bool {
    let Some(vec_val) = metadata.get("temporal_vector") else {
        return true;
    };
    let Ok(vector) = serde_json::from_value::<sunborn_freshness::FreshnessVector>(vec_val.clone())
    else {
        return true;
    };

    let created_at = metadata
        .get("temporal_created_at")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let decay_policy = sunborn_freshness::ConfidenceDecayPolicy::Linear {
        ttl_seconds: state.config.cache.default_ttl_secs.max(1),
    };

    // 1. Check tenant permission dimension + confidence decay
    let perm_uuid = AppState::tenant_permission_scope_uuid(tenant_id);
    let perm_state = state.temporal_engine.get_or_init_state(perm_uuid);
    let perm_vec = sunborn_freshness::FreshnessVector {
        required_doc_version: perm_state.document_version,
        required_perm_version: vector.required_perm_version,
        required_config_version: perm_state.config_version,
        required_model_version: perm_state.model_version,
        max_staleness_epoch: u64::MAX,
    };
    if !state
        .temporal_engine
        .evaluate_freshness_with_decay(
            &perm_uuid,
            &perm_vec,
            perm_state.epoch,
            created_at,
            Utc::now(),
            decay_policy,
            0.05,
        )
        .is_fresh()
    {
        return false;
    }

    // 2. Check global config / policy version dimension
    let cfg_uuid = AppState::global_config_scope_uuid();
    let cfg_state = state.temporal_engine.get_or_init_state(cfg_uuid);
    if cfg_state.config_version != vector.required_config_version {
        return false;
    }

    // 3. Check model version dimension
    let model_uuid = AppState::model_scope_uuid(model);
    let model_state = state.temporal_engine.get_or_init_state(model_uuid);
    if model_state.model_version != vector.required_model_version {
        return false;
    }

    // 4. Check per-source document versions if registered in TemporalValidityEngine
    for snap in source_snapshots {
        if let Some(doc_state) = state.temporal_engine.get_state(&snap.source_id) {
            if doc_state.document_version > snap.snapshot_epoch {
                return false;
            }
        }
    }

    true
}

/// Verifies cached response against authoritative `source_evidences` if provided in request metadata,
/// enforcing ACL scope, claim-level contradiction checks, and fail-closed `require_grounding`.
fn verify_response_grounding(req: &RequestEnvelope, response_content: &str) -> Result<(), String> {
    let require_grounding = req
        .metadata
        .get("require_grounding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let requester_scope = req
        .metadata
        .get("acl_scope")
        .and_then(|v| v.as_str())
        .or_else(|| req.metadata.get("user_id").and_then(|v| v.as_str()));

    let evidences: Vec<sunborn_semantic::SourceEvidence> = req
        .metadata
        .get("source_evidences")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let (_bundle, attestation) = sunborn_semantic::SourceGroundingVerifier::build_and_verify_bundle(
        &req.tenant_id.0,
        &req.application_id.0,
        response_content,
        &evidences,
        requester_scope,
        require_grounding,
    );
    if attestation.grounded {
        Ok(())
    } else {
        Err(attestation.reason)
    }
}

/// Spawns a bounded background shadow evaluation + online conformal calibration +
/// signed learning feedback emission task.
#[allow(clippy::too_many_arguments)]
fn spawn_shadow_evaluation_pipeline(
    state: AppState,
    tenant_id: TenantId,
    application_id: sunborn_core::ApplicationId,
    cache_key: String,
    prompt: String,
    prompt_hash: String,
    cached_content: String,
    origin_content: String,
    decision_type: String,
    is_canary: bool,
    latency_ms: f64,
    similarity_hint: Option<f32>,
    evict_on_drift: bool,
) {
    let Ok(permit) = state.shadow_semaphore.clone().try_acquire_owned() else {
        return;
    };
    tokio::spawn(async move {
        let _permit = permit;
        let shadow_req = crate::shadow::ShadowEvaluationRequest {
            tenant_id: tenant_id.clone(),
            application_id: application_id.clone(),
            cache_key: cache_key.clone(),
            cached_content,
            origin_content,
            similarity_threshold: None,
        };
        if let Ok(res) = state
            .shadow_evaluator
            .evaluate_drift_with_options(
                &shadow_req,
                &state.cache,
                &state.vector_store,
                evict_on_drift,
            )
            .await
        {
            let outcome = if res.is_drift_detected {
                crate::learning::VerifiedOutcome::VerifiedFalseHit
            } else {
                crate::learning::VerifiedOutcome::VerifiedCorrect
            };
            let mut fb = crate::learning::VerifiedFeedbackEvent {
                event_id: format!("shadow-{}", Uuid::new_v4()),
                tenant_id: tenant_id.clone(),
                application_id,
                cache_key: cache_key.clone(),
                decision_type,
                prompt_hash,
                similarity_score: similarity_hint.or(Some(res.similarity_score)),
                conformal_risk_score: Some(if res.is_drift_detected { 1.0 } else { 0.0 }),
                latency_ms,
                tokens_saved: 0,
                cost_saved_usd: 0.0,
                verified_outcome: outcome,
                verification_source: "shadow_eval".to_string(),
                hmac_signature: String::new(),
                timestamp: Utc::now(),
            };
            fb.sign_event(&state.learning_hmac_secret);
            let _ = state.learning_store.record_event(fb);

            if res.is_drift_detected && evict_on_drift {
                state
                    .paraphrase_clusters
                    .remove_member_and_recalibrate(&cache_key);
                if let Some(trigger) = state
                    .learning_controller
                    .record_canary_observation(is_canary, latency_ms, false, true)
                {
                    state.autonomic_controller.record_recovery(
                        "canary_policy_monitor",
                        &format!("Shadow drift detection triggered {:?} rollback", trigger),
                        "Autonomously rolled back canary policy to baseline",
                    );
                    state
                        .temporal_engine
                        .bump_config_version(AppState::global_config_scope_uuid());
                }
            } else if !res.is_drift_detected {
                state
                    .decision_engine
                    .record_verified_observation(&prompt, res.similarity_score as f64)
                    .await;
            }
        }
    });
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

    // Zero-trust tenant suspension kill switch: reject immediately if tenant is suspended
    if let Ok(Some(tenant_rec)) = state.db.get_tenant(&req.tenant_id.0).await {
        if tenant_rec.status.eq_ignore_ascii_case("suspended") {
            return Err(SunbornError::forbidden(format!(
                "Tenant '{}' is suspended; operational access is decommissioned",
                req.tenant_id.0
            ))
            .into());
        }
    }

    // 2. Validate payload boundaries and enforce server-side size/token caps
    req.validate().map_err(AppError::from)?;
    state
        .request_limits
        .enforce(&mut req)
        .map_err(AppError::from)?;

    // Count the request against the tenant's daily/monthly quota and the
    // aggregate free pool before any origin spend is reserved. Cache hits still
    // count as requests; only origin spend is released on a hit.
    if let Err(err) = state.quota_ledger.reserve(&req.tenant_id.0).await {
        match err {
            QuotaError::Exceeded {
                scope,
                window,
                limit,
                used,
                retry_after_secs,
            } => {
                QUOTA_REJECTIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
                RATE_LIMITED_TOTAL.fetch_add(1, Ordering::Relaxed);
                return Err(SunbornError::rate_limit(format!(
                    "{} quota exceeded for tenant '{}': {} of {} requests used in the current {}; retry after {}s",
                    scope, req.tenant_id.0, used, limit, window, retry_after_secs
                ))
                .into());
            }
            QuotaError::Unavailable(detail) => {
                tracing::error!(tenant = %req.tenant_id.0, error = %detail, "Quota store unavailable; failing closed");
                return Err(SunbornError::cache_unavailable(detail).into());
            }
        }
    }

    // Two-phase token-aware rate limiting (RPM/RPD/TPM/TPD/concurrency)
    let token_reservation = state
        .token_limiter
        .pre_commit(&req.tenant_id.0)
        .map_err(|err| {
            RATE_LIMITED_TOTAL.fetch_add(1, Ordering::Relaxed);
            AppError::from(SunbornError::rate_limit(err.to_string()))
        })?;

    // Declarative CEL policy evaluation
    let cel_eval = state
        .cel_engine
        .evaluate(&crate::cel_policy::PolicyContext {
            tenant_id: req.tenant_id.0.clone(),
            application_id: req.application_id.0.clone(),
            model: req.model.clone(),
            is_admin: sec_ctx.is_admin,
            mcp_tool_name: None,
            mcp_server_name: None,
            headers: std::collections::HashMap::new(),
            body_text: if state.cel_engine.combined_requirements().needs_body {
                Some(req.prompt.clone())
            } else {
                None
            },
        });
    if !cel_eval.allowed {
        return Err(SunbornError::forbidden(cel_eval.reason).into());
    }

    // Input-phase content guardrail & PII redaction
    match state
        .guardrail_chain
        .evaluate(crate::guardrails::GuardrailPhase::Input, &req.prompt)
    {
        crate::guardrails::GuardrailOutcome::Allow => {}
        crate::guardrails::GuardrailOutcome::Redact {
            redacted_text,
            findings,
        } => {
            tracing::info!(
                tenant = %req.tenant_id.0,
                ?findings,
                "Input guardrail redacted sensitive PII/secrets before processing"
            );
            req.prompt = redacted_text;
        }
        crate::guardrails::GuardrailOutcome::Block { rule_id, reason } => {
            return Err(SunbornError::forbidden(format!(
                "Input blocked by guardrail '{}': {}",
                rule_id, reason
            ))
            .into());
        }
    }

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
    let estimated_completion_tokens = state.request_limits.estimated_completion_tokens(&req);
    let estimated_cost = FinOpsEngine::calculate_token_cost_avoided(
        &pricing,
        estimated_prompt_tokens,
        estimated_completion_tokens,
    );

    let reservation_id = match state
        .spend_ledger
        .reserve(&req.tenant_id.0, estimated_cost)
        .await
    {
        Ok(id) => id,
        Err(crate::finops::SpendLedgerError::CeilingExceeded { committed, ceiling }) => {
            SPEND_VELOCITY_REJECTIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
            RATE_LIMITED_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(SunbornError::rate_limit(format!(
                "Spend velocity ceiling of ${:.2} exceeded for tenant '{}' (total committed: ${:.2})",
                ceiling, req.tenant_id.0, committed
            ))
            .into());
        }
        Err(crate::finops::SpendLedgerError::Unavailable(detail)) => {
            record_spend_ledger_error();
            tracing::error!(tenant = %req.tenant_id.0, error = %detail, "Spend ledger unavailable; failing closed");
            return Err(SunbornError::cache_unavailable(detail).into());
        }
    };

    // 3. Compute deterministic Cache Identity (C1) and pre-evaluate G1 & G4
    let gate_eval_start = Instant::now();
    let identity = IdentityEngine::compute_identity(&req);
    let g1_identity = sunborn_core::GateCheckResult::pass("G1_CANONICAL_IDENTITY_VERIFIED");
    let g4_eval = crate::five_gates::FiveGateEvaluator::evaluate_g4_effects(&req);
    let mut shadow_mode_hit: Option<(String, String, f32)> = None;
    tracing::info!(
        request_id = %req.request_id.0,
        tenant = %req.tenant_id.0,
        cache_key = %identity.key,
        g4_passed = g4_eval.passed,
        "Evaluating multi-tier cache pipeline"
    );

    // 4. Exact Cache Lookup (C0 / C1)
    if g4_eval.passed
        && state.config.cache.exact_cache_enabled
        && !state.lifecycle_manager.is_quarantined(&identity.key)
        && !state.quarantine_pool.is_quarantined(&identity.key)
    {
        if let Ok(Some(cached_entry)) = state.cache.get(&identity.key).await {
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::info!(
                request_id = %req.request_id.0,
                cache_key = %identity.key,
                "Exact cache candidate found (C0/C1)"
            );

            let cert_opt = cached_entry.metadata.get("certificate").and_then(|v| {
                serde_json::from_value::<sunborn_core::ReuseCertificate>(v.clone()).ok()
            });

            if let Some(cert) = cert_opt {
                let mut g2_eval = crate::five_gates::FiveGateEvaluator::evaluate_g2_context(
                    &req,
                    sec_ctx,
                    Some(&cached_entry.metadata),
                );
                if g2_eval.passed {
                    if let Some(cand_cfg) = cached_entry
                        .metadata
                        .get("config_fingerprint")
                        .and_then(|v| v.as_str())
                    {
                        if cand_cfg
                            != sunborn_identity::IdentityEngine::compute_config_fingerprint(&req)
                        {
                            g2_eval = sunborn_core::GateCheckResult::fail(
                                "G2_CONFIG_FINGERPRINT_MISMATCH",
                            );
                        }
                    }
                }
                if !g2_eval.passed {
                    tracing::warn!(
                        request_id = %req.request_id.0,
                        cache_key = %identity.key,
                        reason = %g2_eval.reason_code,
                        "Exact cache candidate failed G2 context/authorization gate -> Falling through to origin"
                    );
                } else {
                    // 1. Authoritative Server-Side Multi-Source Lease Check (SB-002)
                    let is_fresh = state
                        .epoch_lease_manager
                        .validate_multi_source_leases(&cert.source_snapshots)
                        .await
                        && sunborn_semantic::SourceGroundingVerifier::verify_source_snapshots(
                            &req.dependency_snapshots,
                            &cert.source_snapshots,
                        )
                        .is_ok()
                        && is_temporally_fresh(
                            state,
                            &cached_entry.metadata,
                            &req.tenant_id.0,
                            &req.model,
                            &cert.source_snapshots,
                        );

                    if !is_fresh {
                        tracing::warn!(
                            request_id = %req.request_id.0,
                            cache_key = %identity.key,
                            "Exact cache candidate dependencies modified in authoritative epoch registry -> Evicting entry and falling through to origin"
                        );
                        let _ = state.cache.delete(&identity.key).await;
                    } else {
                        // 2. Enforce proof-carrying admissibility contract at runtime.
                        // Epochs come from the authoritative registry, never the request.
                        // Preserve the request's declared dependency set (so omissions,
                        // duplicates, and orphaned cert deps are still rejected) but take
                        // every epoch from the authoritative registry, not the request.
                        let mut server_epochs: Vec<(uuid::Uuid, u64)> = Vec::new();
                        for snap in &req.dependency_snapshots {
                            server_epochs.push((
                                snap.source_id,
                                state
                                    .epoch_lease_manager
                                    .acquire_lease(snap.source_id)
                                    .await,
                            ));
                        }
                        if server_epochs.is_empty() && !cert.source_snapshots.is_empty() {
                            for snap in &cert.source_snapshots {
                                server_epochs.push((
                                    snap.source_id,
                                    state
                                        .epoch_lease_manager
                                        .acquire_lease(snap.source_id)
                                        .await,
                                ));
                            }
                        }
                        let server_knowledge_epoch =
                            server_epochs.iter().map(|(_, e)| *e).max().unwrap_or(1);

                        let verification_key = match cert.signature_algorithm {
                            sunborn_core::SignatureAlgorithm::Ed25519 => {
                                state.attestation_verifying_key.as_bytes().as_slice()
                            }
                            sunborn_core::SignatureAlgorithm::HmacSha256 => {
                                state.certificate_signing_key.as_slice()
                            }
                        };

                        let measured = measured_safety_evidence(&cert, &req.prompt, 0.25);
                        let admissibility = match &measured {
                            Some(evidence) => cert.verify_admissibility_measured(
                                verification_key,
                                &req.tenant_id,
                                &req.application_id,
                                &req.model,
                                Some(&identity.hash),
                                Utc::now(),
                                server_knowledge_epoch,
                                &server_epochs,
                                evidence,
                            ),
                            None => cert.verify_admissibility(
                                verification_key,
                                &req.tenant_id,
                                &req.application_id,
                                &req.model,
                                Some(&identity.hash),
                                Utc::now(),
                                server_knowledge_epoch,
                                &server_epochs,
                            ),
                        };
                        if let Err(admissibility_err) = admissibility {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                error = %admissibility_err,
                                "Exact cache candidate failed proof-carrying admissibility verification -> Evicting entry and falling through to origin"
                            );
                            let _ = state.cache.delete(&identity.key).await;
                            // Fall through to semantic cache or origin execution (do NOT return unverified content)
                        } else {
                            let certificate = Some(cert);

                            let decrypted = decrypt_stored_content(
                                state,
                                &req.tenant_id,
                                &cached_entry.content,
                                Some(&cached_entry.metadata),
                            );
                            let content = match decrypted {
                                Ok(plain) => plain,
                                Err(err) => {
                                    state
                                        .spend_ledger
                                        .release(&req.tenant_id.0, reservation_id)
                                        .await;
                                    return Err(err.into());
                                }
                            };

                            if let Err(ground_err) = verify_response_grounding(&req, &content) {
                                tracing::warn!(
                                    request_id = %req.request_id.0,
                                    cache_key = %identity.key,
                                    reason = %ground_err,
                                    "Exact cache candidate failed source-evidence grounding verification -> Falling through to origin"
                                );
                                if ground_err.contains("CONTRADICTS") {
                                    state.lifecycle_manager.transition(
                                        &identity.key,
                                        crate::cache_lifecycle::CacheLifecycleState::Quarantined,
                                        &ground_err,
                                    );
                                    state.quarantine_pool.quarantine(
                                        sunborn_threat::quarantine::QuarantineEntry {
                                            key: identity.key.clone(),
                                            original_query: req.prompt.clone(),
                                            attack_query: content.clone(),
                                            threat_category: "grounding_contradiction".to_string(),
                                            anomaly_score: 1.0,
                                            quarantined_at: Utc::now(),
                                        },
                                    );
                                }
                                // Fall through to semantic/origin
                            } else {
                                let five_gates = crate::five_gates::FiveGateEvaluator::synthesize(
                                    g1_identity.clone(),
                                    g2_eval,
                                    sunborn_core::GateCheckResult::pass(
                                        "G3_EXACT_FRESH_AND_GROUNDED",
                                    ),
                                    g4_eval.clone(),
                                    sunborn_core::GateCheckResult::pass("G5_CERTIFICATE_VERIFIED"),
                                );
                                if !five_gates.eligible_reuse {
                                    tracing::warn!(
                                        request_id = %req.request_id.0,
                                        cache_key = %identity.key,
                                        "Exact cache candidate rejected by unified five-gate contract"
                                    );
                                } else {
                                    let ast_us = gate_eval_start.elapsed().as_micros() as u64;
                                    let saved_micropaises =
                                        (estimated_cost * 85.0 * 1_000_000.0).round() as u64;
                                    emit_otel_gate_telemetry(
                                        ast_us,
                                        1.0,
                                        saved_micropaises,
                                        "exact",
                                        &state.selected_embedder.embedding_fingerprint(),
                                    );
                                    if state.shadow_mode {
                                        shadow_mode_hit =
                                            Some((identity.key.clone(), content.clone(), 1.0));
                                    } else {
                                        EXACT_HITS.fetch_add(1, Ordering::Relaxed);
                                        state
                                            .lifecycle_manager
                                            .record_timing("exact_hit", duration_ms as f64);
                                        state
                                            .spend_ledger
                                            .release(&req.tenant_id.0, reservation_id)
                                            .await;
                                        REQUEST_DURATION_MS_SUM
                                            .fetch_add(duration_ms, Ordering::Relaxed);
                                        observe_request_latency(duration_ms);
                                        record_learning_telemetry(
                                            state,
                                            &req,
                                            1.0,
                                            true,
                                            duration_ms as f64,
                                            150.0,
                                        );
                                        let eq_delay = crate::cache_lifecycle::CacheLifecycleManager::timing_equalizer_delay_ms(
                                            state.timing_floor_ms,
                                            duration_ms,
                                        );
                                        if eq_delay > 0 {
                                            tokio::time::sleep(std::time::Duration::from_millis(
                                                eq_delay,
                                            ))
                                            .await;
                                        }
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
                                                policy_version: Some(
                                                    state
                                                        .learning_controller
                                                        .get_active_policy_version(),
                                                ),
                                                policy_id: Some(
                                                    state
                                                        .learning_controller
                                                        .get_active_policy_id(),
                                                ),
                                                five_gate_evidence: Some(five_gates),
                                            },
                                            content,
                                            cached: true,
                                            cache_key: Some(identity.key),
                                            created_at: Utc::now(),
                                            duration_ms,
                                            certificate,
                                            tool_calls: None,
                                            finish_reason: Some("stop".to_string()),
                                            prompt_tokens: None,
                                            completion_tokens: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                tracing::warn!(
                    request_id = %req.request_id.0,
                    cache_key = %identity.key,
                    "Exact cache entry lacks a valid cryptographic ReuseCertificate -> Evicting entry and falling through to origin"
                );
                let _ = state.cache.delete(&identity.key).await;
                // Fall through to semantic cache or origin execution (do NOT return unverified content)
            }
        }
    }

    // 5. Semantic Cache Candidate Evaluation (C2) - Dynamically governed by Learning Controller
    let req_hash =
        u64::from_str_radix(&identity.hash[..16.min(identity.hash.len())], 16).unwrap_or(0);
    let policy_sel = state
        .learning_controller
        .select_policy_decision_for_request(req_hash);
    let mut shadow_revalidation_candidate: Option<(
        String,
        String,
        Option<serde_json::Value>,
        f32,
    )> = None;

    if g4_eval.passed
        && state.config.cache.semantic_cache_enabled
        && state.circuit_breaker.status().await != sunborn_cache::CircuitStatus::Open
    {
        'semantic_eval: {
            let weights = &policy_sel.weights;
            let conformal_floor = if state.shadow_evaluator.observation_count() >= 30 {
                state.shadow_evaluator.calibrated_threshold()
            } else {
                0.0
            };
            let effective_similarity_threshold =
                weights.min_similarity_threshold.max(conformal_floor);
            let eval_result = state
                .decision_engine
                .evaluate_with_thresholds(
                    &req,
                    effective_similarity_threshold,
                    weights.max_risk_threshold,
                )
                .await;
            // An embedding or vector-store outage degrades the request to origin
            // instead of failing it. That is the right availability trade-off, but
            // it is invisible without this signal: the request still returns 200,
            // so nothing else would reveal that the semantic tier is gone.
            if let Err(err) = &eval_result {
                record_semantic_eval_failure();
                tracing::warn!(
                    request_id = %req.request_id.0,
                    error = %err,
                    "Semantic evaluation failed; degrading to exact-cache miss plus origin"
                );
            }
            if let Ok(outcome) = eval_result {
                if outcome.decision.action == DecisionAction::Revalidate {
                    if let (Some(mid), Some(mcontent)) =
                        (outcome.matched_id.clone(), outcome.matched_content.clone())
                    {
                        shadow_revalidation_candidate = Some((
                            mid,
                            mcontent,
                            outcome.matched_metadata.clone(),
                            outcome.decision.confidence,
                        ));
                    }
                } else if outcome.decision.action == DecisionAction::SafeHit {
                    if let Some(content) = outcome.matched_content {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let mut decision = outcome.decision;
                        decision.policy_id = Some(policy_sel.policy_id.clone());
                        decision.policy_version = Some(policy_sel.version);

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

                        // 1. Authoritative Server-Side Multi-Source Lease, Content-Hash, and 5D Temporal Check
                        let meta_ref = outcome
                            .matched_metadata
                            .as_ref()
                            .unwrap_or(&serde_json::Value::Null);
                        let is_fresh = state
                            .epoch_lease_manager
                            .validate_multi_source_leases(&cert.source_snapshots)
                            .await
                            && sunborn_semantic::SourceGroundingVerifier::verify_source_snapshots(
                                &req.dependency_snapshots,
                                &cert.source_snapshots,
                            )
                            .is_ok()
                            && is_temporally_fresh(
                                state,
                                meta_ref,
                                &req.tenant_id.0,
                                &req.model,
                                &cert.source_snapshots,
                            );
                        if !is_fresh {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                "Semantic cache candidate dependencies/temporal state stale -> Treating as MISS and falling through to origin"
                            );
                            if let Some(ref stale_id) = outcome.matched_id {
                                let _ = state.cache.delete(stale_id).await;
                                let _ = state.vector_store.delete(stale_id).await;
                            }
                            break 'semantic_eval;
                        }

                        // Runtime proof-carrying admissibility check, with server-derived epochs.
                        // Keep the request's dependency set; replace only the epochs.
                        let mut server_epochs: Vec<(uuid::Uuid, u64)> = Vec::new();
                        for snap in &req.dependency_snapshots {
                            server_epochs.push((
                                snap.source_id,
                                state
                                    .epoch_lease_manager
                                    .acquire_lease(snap.source_id)
                                    .await,
                            ));
                        }
                        if server_epochs.is_empty() && !cert.source_snapshots.is_empty() {
                            for snap in &cert.source_snapshots {
                                server_epochs.push((
                                    snap.source_id,
                                    state
                                        .epoch_lease_manager
                                        .acquire_lease(snap.source_id)
                                        .await,
                                ));
                            }
                        }
                        let server_knowledge_epoch =
                            server_epochs.iter().map(|(_, e)| *e).max().unwrap_or(1);

                        let verification_key = match cert.signature_algorithm {
                            sunborn_core::SignatureAlgorithm::Ed25519 => {
                                state.attestation_verifying_key.as_bytes().as_slice()
                            }
                            sunborn_core::SignatureAlgorithm::HmacSha256 => {
                                state.certificate_signing_key.as_slice()
                            }
                        };

                        let measured = measured_safety_evidence(&cert, &req.prompt, 0.25);
                        let admissibility = match &measured {
                            Some(evidence) => cert.verify_admissibility_measured(
                                verification_key,
                                &req.tenant_id,
                                &req.application_id,
                                &req.model,
                                None,
                                Utc::now(),
                                server_knowledge_epoch,
                                &server_epochs,
                                evidence,
                            ),
                            None => cert.verify_admissibility(
                                verification_key,
                                &req.tenant_id,
                                &req.application_id,
                                &req.model,
                                None,
                                Utc::now(),
                                server_knowledge_epoch,
                                &server_epochs,
                            ),
                        };
                        if let Err(admissibility_err) = admissibility {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                error = %admissibility_err,
                                "Semantic cache candidate failed proof-carrying admissibility verification -> Treating as MISS and falling through to origin"
                            );
                            break 'semantic_eval;
                        }

                        let certificate = Some(cert);

                        // Authenticated Decryption on Semantic Read Path (K-004)
                        let decrypted_content = match decrypt_stored_content(
                            state,
                            &req.tenant_id,
                            &content,
                            outcome.matched_metadata.as_ref(),
                        ) {
                            Ok(plain) => plain,
                            Err(err) => {
                                state
                                    .spend_ledger
                                    .release(&req.tenant_id.0, reservation_id)
                                    .await;
                                return Err(err.into());
                            }
                        };

                        if let Some(ref mid) = outcome.matched_id {
                            if state.lifecycle_manager.is_quarantined(mid)
                                || state.quarantine_pool.is_quarantined(mid)
                            {
                                tracing::warn!(
                                    request_id = %req.request_id.0,
                                    matched_id = %mid,
                                    "Semantic cache candidate is QUARANTINED -> Falling through to origin"
                                );
                                break 'semantic_eval;
                            }
                            if let Some(cand_prompt) =
                                meta_ref.get("prompt_text").and_then(|v| v.as_str())
                            {
                                let threat_res = state.threat_engine.inspect_admission(
                                    cand_prompt,
                                    &req.prompt,
                                    decision.confidence,
                                );
                                if threat_res.is_flagged_for_quarantine() {
                                    tracing::warn!(
                                        request_id = %req.request_id.0,
                                        matched_id = %mid,
                                        threat_category = %threat_res.threat_category(),
                                        "CacheThreatEngine flagged adversarial poisoning discrepancy -> Quarantining candidate"
                                    );
                                    state.lifecycle_manager.transition(
                                        mid,
                                        crate::cache_lifecycle::CacheLifecycleState::Quarantined,
                                        &threat_res.reason,
                                    );
                                    state.quarantine_pool.quarantine(
                                        sunborn_threat::quarantine::QuarantineEntry {
                                            key: mid.clone(),
                                            original_query: cand_prompt.to_string(),
                                            attack_query: req.prompt.clone(),
                                            threat_category: threat_res
                                                .threat_category()
                                                .to_string(),
                                            anomaly_score: threat_res.anomaly_score,
                                            quarantined_at: Utc::now(),
                                        },
                                    );
                                    break 'semantic_eval;
                                }
                            }
                        }

                        if let Err(ground_err) = verify_response_grounding(&req, &decrypted_content)
                        {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                reason = %ground_err,
                                "Semantic cache candidate failed source-evidence grounding -> Falling through to origin"
                            );
                            if ground_err.contains("CONTRADICTS") {
                                if let Some(ref mid) = outcome.matched_id {
                                    state.lifecycle_manager.transition(
                                        mid,
                                        crate::cache_lifecycle::CacheLifecycleState::Quarantined,
                                        &ground_err,
                                    );
                                    state.quarantine_pool.quarantine(
                                        sunborn_threat::quarantine::QuarantineEntry {
                                            key: mid.clone(),
                                            original_query: req.prompt.clone(),
                                            attack_query: decrypted_content.clone(),
                                            threat_category: "grounding_contradiction".to_string(),
                                            anomaly_score: 1.0,
                                            quarantined_at: Utc::now(),
                                        },
                                    );
                                }
                            }
                            break 'semantic_eval;
                        }

                        let g2_sem = crate::five_gates::FiveGateEvaluator::evaluate_g2_context(
                            &req,
                            sec_ctx,
                            Some(meta_ref),
                        );
                        let g1_sem = sunborn_core::GateCheckResult::pass(format!(
                            "G1_SEMANTIC_MATCH_{:.3}",
                            decision.confidence
                        ));
                        let g3_sem =
                            sunborn_core::GateCheckResult::pass("G3_SEMANTIC_FRESH_AND_GROUNDED");
                        let g5_sem = sunborn_core::GateCheckResult::pass(format!(
                            "G5_CERT_AND_POLICY_{}",
                            policy_sel.policy_id
                        ));
                        let five_gates = crate::five_gates::FiveGateEvaluator::synthesize(
                            g1_sem,
                            g2_sem,
                            g3_sem,
                            g4_eval.clone(),
                            g5_sem,
                        );
                        if !five_gates.eligible_reuse {
                            tracing::warn!(
                                request_id = %req.request_id.0,
                                g2_reason = %five_gates.g2_context_authorization.reason_code,
                                "Semantic cache candidate failed unified five-gate contract -> Falling through to origin"
                            );
                            break 'semantic_eval;
                        }
                        decision.five_gate_evidence = Some(five_gates);

                        let ast_us = gate_eval_start.elapsed().as_micros() as u64;
                        let saved_micropaises =
                            (estimated_cost * 85.0 * 1_000_000.0).round() as u64;
                        emit_otel_gate_telemetry(
                            ast_us,
                            decision.confidence,
                            saved_micropaises,
                            "semantic",
                            &state.selected_embedder.embedding_fingerprint(),
                        );
                        if state.shadow_mode {
                            shadow_mode_hit = Some((
                                outcome
                                    .matched_id
                                    .clone()
                                    .unwrap_or_else(|| identity.key.clone()),
                                decrypted_content.clone(),
                                decision.confidence,
                            ));
                        } else {
                            SEMANTIC_HITS.fetch_add(1, Ordering::Relaxed);
                            state
                                .lifecycle_manager
                                .record_timing("semantic_hit", duration_ms as f64);
                            record_learning_telemetry(
                                state,
                                &req,
                                decision.confidence,
                                true,
                                duration_ms as f64,
                                150.0,
                            );
                            tracing::info!(
                                request_id = %req.request_id.0,
                                confidence = decision.confidence,
                                "Semantic cache HIT (C2)"
                            );

                            if let Some(trigger) =
                                state.learning_controller.record_canary_observation(
                                    policy_sel.is_canary,
                                    duration_ms as f64,
                                    false,
                                    false,
                                )
                            {
                                state.autonomic_controller.record_recovery(
                                    "canary_policy_monitor",
                                    &format!("Canary semantic hit SLA violation ({:?})", trigger),
                                    "Autonomously rolled back canary policy to baseline",
                                );
                            }

                            // Controlled background shadow sampling on semantic hits
                            if state.shadow_sample_rate > 0.0
                                && (req_hash % 1000) < ((state.shadow_sample_rate * 1000.0) as u64)
                            {
                                if let Some(ref matched_key) = outcome.matched_id {
                                    let state_bg = state.clone();
                                    let mut origin_probe_req = req.clone();
                                    state
                                        .request_limits
                                        .apply_completion_cap(&mut origin_probe_req);
                                    let cached_plain = decrypted_content.clone();
                                    let matched_key_bg = matched_key.clone();
                                    let req_hash_hex = identity.hash.clone();
                                    let is_canary = policy_sel.is_canary;
                                    let conf = decision.confidence;
                                    if let Ok(permit) =
                                        state.shadow_semaphore.clone().try_acquire_owned()
                                    {
                                        tokio::spawn(async move {
                                            let _permit = permit;
                                            let t0 = Instant::now();
                                            if let Ok(origin_res) = state_bg
                                                .origin_provider
                                                .execute(&origin_probe_req)
                                                .await
                                            {
                                                let lat = t0.elapsed().as_millis() as f64;
                                                spawn_shadow_evaluation_pipeline(
                                                    state_bg,
                                                    origin_probe_req.tenant_id,
                                                    origin_probe_req.application_id,
                                                    matched_key_bg,
                                                    origin_probe_req.prompt,
                                                    req_hash_hex,
                                                    cached_plain,
                                                    origin_res.content,
                                                    if is_canary {
                                                        "canary_semantic_hit".to_string()
                                                    } else {
                                                        "semantic_hit".to_string()
                                                    },
                                                    is_canary,
                                                    lat,
                                                    Some(conf),
                                                    true,
                                                );
                                            }
                                        });
                                    }
                                }
                            }

                            state
                                .spend_ledger
                                .release(&req.tenant_id.0, reservation_id)
                                .await;

                            REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
                            observe_request_latency(duration_ms);
                            let eq_delay = crate::cache_lifecycle::CacheLifecycleManager::timing_equalizer_delay_ms(
                                state.timing_floor_ms,
                                duration_ms,
                            );
                            if eq_delay > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(eq_delay))
                                    .await;
                            }
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
                                tool_calls: None,
                                finish_reason: Some("stop".to_string()),
                                prompt_tokens: None,
                                completion_tokens: None,
                            });
                        }
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

    let mut origin_req = req.clone();
    state.request_limits.apply_completion_cap(&mut origin_req);
    let origin_result = match state.origin_provider.execute(&origin_req).await {
        Ok(res) => res,
        Err(err) => {
            if let Some(trigger) = state.learning_controller.record_canary_observation(
                policy_sel.is_canary,
                start.elapsed().as_millis() as f64,
                true,
                false,
            ) {
                state.autonomic_controller.record_recovery(
                    "canary_policy_monitor",
                    &format!("Canary origin error rate spike triggered {:?}", trigger),
                    "Autonomously rolled back canary policy to baseline",
                );
            }
            state
                .spend_ledger
                .release(&req.tenant_id.0, reservation_id)
                .await;
            return Err(err.into());
        }
    };
    let origin_tool_calls = origin_result.tool_calls.clone();
    let origin_finish_reason = origin_result.finish_reason.clone();
    let origin_prompt_tokens = origin_result.prompt_tokens;
    let origin_completion_tokens = origin_result.completion_tokens;
    let mut origin_content = origin_result.content;

    // Commit actual token usage to two-phase TPM/TPD limiter
    token_reservation.commit_tokens(origin_prompt_tokens, origin_completion_tokens);

    // Output-phase content guardrail & PII redaction before cache admission
    match state
        .guardrail_chain
        .evaluate(crate::guardrails::GuardrailPhase::Output, &origin_content)
    {
        crate::guardrails::GuardrailOutcome::Allow => {}
        crate::guardrails::GuardrailOutcome::Redact {
            redacted_text,
            findings,
        } => {
            tracing::info!(
                tenant = %req.tenant_id.0,
                ?findings,
                "Output guardrail redacted sensitive PII/secrets before cache admission"
            );
            origin_content = redacted_text;
        }
        crate::guardrails::GuardrailOutcome::Block { rule_id, reason } => {
            return Err(SunbornError::forbidden(format!(
                "Output blocked by guardrail '{}': {}",
                rule_id, reason
            ))
            .into());
        }
    }

    // Shadow Mode asynchronous comparison & per-tenant/per-route calibration accounting
    let is_shadow_hit = shadow_mode_hit.is_some();
    if let Some((shadow_key, shadow_cached, shadow_conf)) = shadow_mode_hit {
        let shadow_state = state.clone();
        let shadow_tenant = req.tenant_id.clone();
        let shadow_app = req.application_id.clone();
        let shadow_model = req.model.clone();
        let shadow_prompt = req.prompt.clone();
        let shadow_hash = identity.hash.clone();
        let shadow_origin = origin_content.clone();
        let potential_saved_micropaises = (estimated_cost * 85.0 * 1_000_000.0).round() as u64;
        let shadow_lat = start.elapsed().as_millis() as f64;
        if let Ok(permit) = state.shadow_semaphore.clone().try_acquire_owned() {
            tokio::spawn(async move {
                let _permit = permit;
                let eval_req = crate::shadow::ShadowEvaluationRequest {
                    tenant_id: shadow_tenant.clone(),
                    application_id: shadow_app.clone(),
                    cache_key: shadow_key.clone(),
                    cached_content: shadow_cached.clone(),
                    origin_content: shadow_origin.clone(),
                    similarity_threshold: None,
                };
                if let Ok(res) = shadow_state
                    .shadow_evaluator
                    .evaluate_drift_with_options(
                        &eval_req,
                        &shadow_state.cache,
                        &shadow_state.vector_store,
                        false,
                    )
                    .await
                {
                    shadow_state.shadow_evaluator.record_route_shadow_outcome(
                        &shadow_tenant.0,
                        &shadow_model,
                        res.is_drift_detected,
                        potential_saved_micropaises,
                    );
                }
                spawn_shadow_evaluation_pipeline(
                    shadow_state,
                    shadow_tenant,
                    shadow_app,
                    shadow_key,
                    shadow_prompt,
                    shadow_hash,
                    shadow_cached,
                    shadow_origin,
                    "shadow_mode".to_string(),
                    false,
                    shadow_lat,
                    Some(shadow_conf),
                    true,
                );
            });
        }
    }

    // Automatic Shadow Evaluation on Revalidation Candidates (zero extra origin calls):
    if let Some((cand_id, cand_stored, cand_meta, cand_conf)) = shadow_revalidation_candidate {
        if let Ok(cand_plain) =
            decrypt_stored_content(state, &req.tenant_id, &cand_stored, cand_meta.as_ref())
        {
            spawn_shadow_evaluation_pipeline(
                state.clone(),
                req.tenant_id.clone(),
                req.application_id.clone(),
                cand_id,
                req.prompt.clone(),
                identity.hash.clone(),
                cand_plain,
                origin_content.clone(),
                if policy_sel.is_canary {
                    "canary_semantic_revalidate".to_string()
                } else {
                    "semantic_revalidate".to_string()
                },
                policy_sel.is_canary,
                start.elapsed().as_millis() as f64,
                Some(cand_conf),
                false,
            );
        }
    } else {
        let _ = state.learning_controller.record_canary_observation(
            policy_sel.is_canary,
            start.elapsed().as_millis() as f64,
            false,
            false,
        );
    }

    let pricing = ModelPricing::for_model(&req.model);
    let actual_cost = FinOpsEngine::calculate_token_cost_avoided(
        &pricing,
        origin_result.prompt_tokens,
        origin_result.completion_tokens,
    );
    // Settle the atomic reservation with actual origin cost
    if let Err(err) = state
        .spend_ledger
        .settle(&req.tenant_id.0, reservation_id, actual_cost)
        .await
    {
        tracing::error!(
            tenant = %req.tenant_id.0,
            reservation = %reservation_id,
            error = ?err,
            "CRITICAL: Failed to settle spend reservation in ledger; retaining pending reservation for reconciliation"
        );
    }

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

    let mut origin_admitted = false;
    let write_threat =
        state
            .threat_engine
            .inspect_write_admission(&identity.key, &req.prompt, &origin_content);
    if !is_lease_valid || !is_watermark_valid || !g4_eval.passed || !write_threat.admitted {
        tracing::warn!(
            request_id = %req.request_id.0,
            is_lease_valid = %is_lease_valid,
            is_watermark_valid = %is_watermark_valid,
            g4_passed = %g4_eval.passed,
            write_threat_admitted = %write_threat.admitted,
            "Freshness fence, G4 effect safety, or write-side poisoning defense blocked cache writeback -> Aborting cache admission"
        );
    } else {
        let prompt_risk = sunborn_semantic::DecisionEngine::<
            std::sync::Arc<dyn sunborn_semantic::EmbeddingProvider>,
            std::sync::Arc<dyn sunborn_semantic::VectorStore>,
        >::assess_risk(&req.prompt);

        // Server-derived source epochs: never trust request-supplied epochs.
        let mut server_snapshots: Vec<sunborn_core::SourceVersionSnapshot> = Vec::new();
        for snap in &req.dependency_snapshots {
            let epoch = state
                .epoch_lease_manager
                .acquire_lease(snap.source_id)
                .await;
            server_snapshots.push(sunborn_core::SourceVersionSnapshot {
                source_id: snap.source_id,
                snapshot_epoch: epoch,
                content_hash: snap.content_hash.clone(),
            });
        }
        let server_knowledge_epoch = server_snapshots
            .iter()
            .map(|s| s.snapshot_epoch)
            .max()
            .unwrap_or(1);

        // Bind the issuing prompt's structural fingerprint so reuse is measured
        // against the candidate prompt at read time.
        let reference_fingerprint = serde_json::to_string(
            &sunborn_semantic::StructuralFingerprint::extract(&req.prompt),
        )
        .unwrap_or_default();

        let admission_cert = if let Some(ref ed_sk) = state.attestation_signing_key {
            let mut cert = sunborn_core::ReuseCertificate::new_signed_ed25519(
                req.tenant_id.clone(),
                req.application_id.clone(),
                &identity.hash,
                &req.model,
                req.prompt.split_whitespace().count() as u32,
                server_snapshots,
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
                    monotonic_epoch_lease: server_knowledge_epoch,
                },
                ed_sk,
                &state.attestation_key_id,
            );
            cert.attach_reference_fingerprint_ed25519(reference_fingerprint, ed_sk);
            cert
        } else {
            let mut cert = sunborn_core::ReuseCertificate::new_signed(
                req.tenant_id.clone(),
                req.application_id.clone(),
                &identity.hash,
                &req.model,
                req.prompt.split_whitespace().count() as u32,
                server_snapshots,
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
                    monotonic_epoch_lease: server_knowledge_epoch,
                },
                &state.certificate_signing_key,
            );
            cert.attach_reference_fingerprint(
                reference_fingerprint,
                &state.certificate_signing_key,
            );
            cert
        };

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

            // Transactional Cache Admission State Machine (P0-05)
            // Stage 1: Save to authoritative PostgreSQL system of record with certificate provenance (P0-03)
            let cert_json = serde_json::to_string(&admission_cert).ok();
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
                    certificate_json: cert_json,
                })
                .await;

            if let Err(ref e) = db_res {
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

            // Stage 2: Save in exact cache with embedded certificate, 5D temporal vector, and envelope metadata
            let perm_state = state
                .temporal_engine
                .get_or_init_state(AppState::tenant_permission_scope_uuid(&req.tenant_id.0));
            let cfg_state = state
                .temporal_engine
                .get_or_init_state(AppState::global_config_scope_uuid());
            let model_state = state
                .temporal_engine
                .get_or_init_state(AppState::model_scope_uuid(&req.model));
            let mut max_doc_ver = 1u64;
            for snap in &req.dependency_snapshots {
                let st = state.temporal_engine.get_or_init_state(snap.source_id);
                max_doc_ver = max_doc_ver.max(st.document_version);
            }
            let temporal_vector = sunborn_freshness::FreshnessVector {
                required_doc_version: max_doc_ver,
                required_perm_version: perm_state.permission_version,
                required_config_version: cfg_state.config_version,
                required_model_version: model_state.model_version,
                max_staleness_epoch: server_knowledge_epoch,
            };
            let temporal_created_at = Utc::now().to_rfc3339();

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
            if let Ok(tv_val) = serde_json::to_value(temporal_vector) {
                entry.metadata["temporal_vector"] = tv_val;
            }
            entry.metadata["temporal_created_at"] = json!(temporal_created_at);
            apply_request_scope_metadata(state, &req, &mut entry.metadata);

            match state.cache.set(entry.clone()).await {
                Ok(_) => {
                    origin_admitted = true;
                    state.lifecycle_manager.admit_versioned(
                        &identity.key,
                        &req.tenant_id,
                        actual_cost,
                        (1.0 - prompt_risk as f64).clamp(0.0, 1.0),
                        0.1,
                        stored_content.len(),
                    );
                    // Stage 3: Bind into knowledge dependency graph, graph memory cache, and Graphify multi-relational graph (P0-01)
                    let cache_key_id = identity.key.clone();
                    let cache_node = sunborn_invalidation::GraphifyNode::new(
                        cache_key_id.clone(),
                        state.graphify_graph.next_numeric_id(),
                        identity.key.clone(),
                        sunborn_invalidation::GraphifyNodeType::CacheEntry,
                    )
                    .with_metadata(json!({
                        "tenant_id": sec_ctx.tenant_id.0,
                        "cache_key": identity.key,
                    }));
                    state.graphify_graph.add_node(cache_node);

                    for snap in &req.dependency_snapshots {
                        if let Err(e) = state
                            .dependency_graph
                            .record_dependency(snap.source_id, &identity.key)
                            .await
                        {
                            tracing::warn!(
                                key = %identity.key,
                                source_id = %snap.source_id,
                                error = %e,
                                "Failed to record knowledge dependency edge"
                            );
                        }

                        let source_str = snap.source_id.to_string();
                        if state.graphify_graph.get_node(&source_str).is_none() {
                            let doc_node = sunborn_invalidation::GraphifyNode::new(
                                source_str.clone(),
                                state.graphify_graph.next_numeric_id(),
                                format!("doc::{}", source_str),
                                sunborn_invalidation::GraphifyNodeType::Document,
                            )
                            .with_metadata(json!({
                                "tenant_id": sec_ctx.tenant_id.0,
                                "source_id": source_str,
                            }));
                            state.graphify_graph.add_node(doc_node);
                        }
                        state.graphify_graph.link(
                            cache_key_id.clone(),
                            source_str,
                            sunborn_invalidation::GraphifyRelation::DependsOn,
                        );
                    }
                    crate::handlers::graph::persist_graphify_if_configured(
                        state.graphify_graph.clone(),
                    );
                    if let Some(ref gmc) = state.graph_memory_cache {
                        let source_ids: Vec<Uuid> = req
                            .dependency_snapshots
                            .iter()
                            .map(|s| s.source_id)
                            .collect();
                        // Derive doc_hashes by combining source_id and content_hash deterministically
                        let doc_hashes: Vec<u64> = req
                            .dependency_snapshots
                            .iter()
                            .map(|s| {
                                let mut bytes = Vec::with_capacity(64);
                                bytes.extend_from_slice(s.source_id.as_bytes());
                                bytes.extend_from_slice(s.content_hash.as_bytes());
                                sunborn_invalidation::deterministic_hash_64(&bytes)
                            })
                            .collect();
                        let record_id =
                            sunborn_invalidation::deterministic_hash_64(identity.key.as_bytes());
                        if let Err(e) = gmc
                            .set_with_lineage(entry, &source_ids, &doc_hashes, record_id)
                            .await
                        {
                            tracing::warn!(
                                key = %identity.key,
                                error = %e,
                                "Failed to commit entry to graph memory cache with lineage"
                            );
                        }
                    }

                    // Stage 4: Admit into persistent paraphrase cluster & semantic vector store
                    if let Ok(vector) = state.selected_embedder.embed(&req.prompt).await {
                        let (cluster_id, _) = state.paraphrase_clusters.assign_or_create(
                            &req.tenant_id,
                            &req.application_id,
                            &identity.key,
                            &req.prompt,
                            &vector,
                            0.85,
                        );
                        let mut meta = serde_json::json!({
                            "model": req.model,
                            "prompt_text": req.prompt,
                            "certificate": serde_json::to_value(&admission_cert).unwrap_or_default(),
                            "paraphrase_cluster_id": cluster_id,
                            "temporal_vector": serde_json::to_value(temporal_vector).unwrap_or_default(),
                            "temporal_created_at": temporal_created_at,
                        });
                        apply_request_scope_metadata(state, &req, &mut meta);
                        if let Some(ref env_val) = envelope_meta {
                            meta["encrypted_envelope"] = env_val.clone();
                        }
                        if let Err(e) = state
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
                            .await
                        {
                            tracing::warn!(
                                key = %identity.key,
                                error = %e,
                                "Vector store insertion failed on cache admission"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        key = %identity.key,
                        error = %e,
                        "Exact cache insertion failed on cache admission"
                    );
                }
            }
        } else {
            tracing::info!(tenant = %req.tenant_id.0, "Cache admission skipped: encryption unavailable, fail-closed enforced");
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    state
        .lifecycle_manager
        .record_timing("origin_miss", duration_ms as f64);
    record_learning_telemetry(
        state,
        &req,
        0.0,
        false,
        duration_ms as f64,
        duration_ms as f64,
    );

    let origin_g2 = crate::five_gates::FiveGateEvaluator::evaluate_g2_context(&req, sec_ctx, None);
    let origin_g3 = if is_lease_valid && is_watermark_valid {
        sunborn_core::GateCheckResult::pass("G3_FRESHNESS_FENCE_VALID")
    } else {
        sunborn_core::GateCheckResult::fail("G3_STALE_FENCE_ABORTED_ADMISSION")
    };
    let origin_g5 = if origin_admitted {
        sunborn_core::GateCheckResult::pass("G5_ORIGIN_ADMITTED_WITH_CERT")
    } else {
        sunborn_core::GateCheckResult::fail("G5_CACHE_ADMISSION_SKIPPED")
    };
    let origin_five_gates = crate::five_gates::FiveGateEvaluator::synthesize(
        g1_identity,
        origin_g2,
        origin_g3,
        g4_eval,
        origin_g5,
    );

    let resp = ResponseEnvelope {
        request_id: req.request_id,
        tenant_id: req.tenant_id,
        application_id: req.application_id,
        decision: DecisionRecord {
            action: DecisionAction::OriginExecute,
            cache_path: CachePath::Miss,
            confidence: 1.0,
            risk_score: 0.0,
            reason_code: if is_shadow_hit {
                "SHADOW_MODE_ORIGIN_EXECUTED".to_string()
            } else {
                "NO_CACHE_ENTRY_FOUND".to_string()
            },
            latency_saved_ms: None,
            policy_version: Some(state.learning_controller.get_active_policy_version()),
            policy_id: Some(state.learning_controller.get_active_policy_id()),
            five_gate_evidence: Some(origin_five_gates),
        },
        content: origin_content,
        cached: false,
        cache_key: Some(identity.key),
        created_at: Utc::now(),
        duration_ms,
        certificate: None,
        tool_calls: origin_tool_calls,
        finish_reason: origin_finish_reason,
        prompt_tokens: Some(origin_prompt_tokens),
        completion_tokens: Some(origin_completion_tokens),
    };
    REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
    observe_request_latency(duration_ms);
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
