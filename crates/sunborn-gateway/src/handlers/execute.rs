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
            tracing::error!(tenant = %req.tenant_id.0, error = %detail, "Spend ledger unavailable; failing closed");
            return Err(SunbornError::cache_unavailable(detail).into());
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

            let cert_opt = cached_entry.metadata.get("certificate").and_then(|v| {
                serde_json::from_value::<sunborn_core::ReuseCertificate>(v.clone()).ok()
            });

            if let Some(cert) = cert_opt {
                // 1. Authoritative Server-Side Multi-Source Lease Check (SB-002)
                let is_fresh = state
                    .epoch_lease_manager
                    .validate_multi_source_leases(&cert.source_snapshots)
                    .await;

                if !is_fresh {
                    tracing::warn!(
                        request_id = %req.request_id.0,
                        cache_key = %identity.key,
                        "Exact cache candidate dependencies modified in authoritative epoch registry -> Evicting entry and falling through to origin"
                    );
                    let cache_clone = state.cache.clone();
                    let key_to_evict = identity.key.clone();
                    tokio::spawn(async move {
                        let _ = cache_clone.delete(&key_to_evict).await;
                    });
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
                            .and_then(|v| {
                                serde_json::from_value::<EncryptedEnvelope>(v.clone()).ok()
                            })
                            .or_else(|| parse_envelope_from_stored(&cached_entry.content));

                        let content = if let Some(envelope) = envelope_opt {
                            match state.crypto_registry.decrypt(&envelope, "cache_admission") {
                                Ok(plain_bytes) => {
                                    String::from_utf8(plain_bytes).unwrap_or(cached_entry.content)
                                }
                                Err(err) => {
                                    tracing::warn!(tenant = %req.tenant_id.0, error = %err, "TDEK decryption failed or key shredded -> Rejecting cached entry");
                                    state
                                        .spend_ledger
                                        .release(&req.tenant_id.0, reservation_id)
                                        .await;
                                    return Err(SunbornError::forbidden(format!(
                                        "Tenant '{}' key shredded or unprovisioned",
                                        req.tenant_id.0
                                    ))
                                    .into());
                                }
                            }
                        } else if cached_entry.content.starts_with("sunborn:enc:v1:") {
                            state
                                .spend_ledger
                                .release(&req.tenant_id.0, reservation_id)
                                .await;
                            return Err(SunbornError::forbidden(format!(
                                "Tenant '{}' key shredded or unprovisioned",
                                req.tenant_id.0
                            ))
                            .into());
                        } else {
                            cached_entry.content
                        };

                        state
                            .spend_ledger
                            .release(&req.tenant_id.0, reservation_id)
                            .await;
                        REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
                        observe_request_latency(duration_ms);
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
                                    state.learning_controller.get_active_policy_version(),
                                ),
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
            } else {
                tracing::warn!(
                    request_id = %req.request_id.0,
                    cache_key = %identity.key,
                    "Exact cache entry lacks a valid cryptographic ReuseCertificate -> Evicting entry and falling through to origin"
                );
                let cache_clone = state.cache.clone();
                let key_to_evict = identity.key.clone();
                tokio::spawn(async move {
                    let _ = cache_clone.delete(&key_to_evict).await;
                });
                // Fall through to semantic cache or origin execution (do NOT return unverified content)
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
                                        .spend_ledger
                                        .release(&req.tenant_id.0, reservation_id)
                                        .await;
                                    return Err(SunbornError::forbidden(format!(
                                        "Tenant '{}' key shredded or unprovisioned",
                                        req.tenant_id.0
                                    ))
                                    .into());
                                }
                            }
                        } else if content.starts_with("sunborn:enc:v1:") {
                            state
                                .spend_ledger
                                .release(&req.tenant_id.0, reservation_id)
                                .await;
                            return Err(SunbornError::forbidden(format!(
                                "Tenant '{}' key shredded or unprovisioned",
                                req.tenant_id.0
                            ))
                            .into());
                        } else {
                            content
                        };

                        state
                            .spend_ledger
                            .release(&req.tenant_id.0, reservation_id)
                            .await;

                        REQUEST_DURATION_MS_SUM.fetch_add(duration_ms, Ordering::Relaxed);
                        observe_request_latency(duration_ms);
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

    let mut origin_req = req.clone();
    state.request_limits.apply_completion_cap(&mut origin_req);
    let origin_result = match state.origin_provider.execute(&origin_req).await {
        Ok(res) => res,
        Err(err) => {
            state
                .spend_ledger
                .release(&req.tenant_id.0, reservation_id)
                .await;
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

            // Stage 2: Save in exact cache with embedded certificate metadata
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

            match state.cache.set(entry.clone()).await {
                Ok(_) => {
                    // Stage 3: Bind into knowledge dependency graph and graph memory cache (P0-01)
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
                    }
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

                    // Stage 4: Admit into semantic vector store
                    if let Ok(vector) = state.selected_embedder.embed(&req.prompt).await {
                        let mut meta = serde_json::json!({
                            "model": req.model,
                            "certificate": serde_json::to_value(&admission_cert).unwrap_or_default(),
                        });
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
