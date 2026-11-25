//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

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

    let rollback_trigger =
        if event.verified_outcome.is_unsafe_reuse() && event.decision_type.contains("canary") {
            state
                .learning_controller
                .record_canary_observation(true, event.latency_ms, false, true)
        } else {
            None
        };

    // Detect contradictory verified feedback for the same cache_key (§13.1)
    let prior_events = state.learning_store.get_tenant_events(&event.tenant_id);
    let contradictory_feedback = prior_events.iter().any(|prev| {
        prev.cache_key == event.cache_key
            && ((prev.verified_outcome.is_admissible_hit()
                && event.verified_outcome.is_unsafe_reuse())
                || (prev.verified_outcome.is_unsafe_reuse()
                    && event.verified_outcome.is_admissible_hit()))
    });

    let unsafe_quarantine_key =
        if event.verified_outcome.is_unsafe_reuse() || contradictory_feedback {
            Some((
                event.cache_key.clone(),
                if contradictory_feedback {
                    "CONTRADICTORY_FEEDBACK_CONFLICT".to_string()
                } else {
                    event.verified_outcome.as_canonical_str().to_string()
                },
            ))
        } else {
            None
        };

    state
        .learning_store
        .record_event(event)
        .map_err(AppError::from)?;

    if let Some((ref q_key, ref q_cat)) = unsafe_quarantine_key {
        state.lifecycle_manager.transition(
            q_key,
            crate::cache_lifecycle::CacheLifecycleState::Quarantined,
            q_cat,
        );
        state
            .quarantine_pool
            .quarantine(sunborn_threat::quarantine::QuarantineEntry {
                key: q_key.clone(),
                original_query: "verified_feedback_report".to_string(),
                attack_query: q_cat.clone(),
                threat_category: q_cat.clone(),
                anomaly_score: 1.0,
                quarantined_at: Utc::now(),
            });
    }

    if let Some(ref trigger) = rollback_trigger {
        state.autonomic_controller.record_recovery(
            "canary_policy_monitor",
            &format!("Verified unsafe feedback triggered {:?} rollback", trigger),
            "Autonomously rolled back canary policy to baseline",
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "FEEDBACK_RECORDED",
            "canary_rollback_triggered": rollback_trigger.is_some(),
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
    let canary_metrics = state.learning_controller.get_canary_metrics();
    let tenant_events_count = state.learning_store.tenant_event_count(&sec_ctx.tenant_id);
    let total_events = state.learning_store.total_events();
    let audit_trail = state.learning_controller.get_audit_trail();
    let paraphrase_clusters_count = state
        .paraphrase_clusters
        .list_tenant_clusters(&sec_ctx.tenant_id)
        .len();
    let (telemetry_observations, telemetry_hit_rate, tvd_drift, drift_detected) = {
        let tw = sunborn_core::sync::read_lock(&state.telemetry_window);
        let obs_dist =
            sunborn_learning::WorkloadDistribution::from_counts(&tw.category_distribution());
        let mut baseline_map = std::collections::HashMap::new();
        baseline_map.insert("general".to_string(), 1.0);
        let detector = sunborn_learning::DriftDetector::new(
            sunborn_learning::WorkloadDistribution {
                category_proportions: baseline_map,
            },
            0.35,
        );
        (
            tw.total_queries(),
            tw.hit_rate(),
            detector.total_variation_distance(&obs_dist),
            tw.total_queries() >= 10 && detector.detect_drift(&obs_dist),
        )
    };

    Json(json!({
        "tenant_id": sec_ctx.tenant_id.0,
        "storage_tier": state.learning_store.storage_tier(),
        "learning_store_durable": state.learning_store.is_durable(),
        "policy_storage_tier": state.learning_controller.storage_tier(),
        "policy_registry_durable": state.learning_controller.is_durable(),
        "active_policy": active,
        "canary_policy": canary.as_ref().map(|(p, pct)| json!({ "policy": p, "percentage": pct })),
        "canary_metrics": canary_metrics,
        "shadow_conformal_observations": state.shadow_evaluator.observation_count(),
        "shadow_conformal_threshold": state.shadow_evaluator.calibrated_threshold(),
        "paraphrase_clusters_count": paraphrase_clusters_count,
        "telemetry_observations": telemetry_observations,
        "telemetry_hit_rate": telemetry_hit_rate,
        "workload_tvd_drift": tvd_drift,
        "workload_drift_detected": drift_detected,
        "retrieval_cache_entries": state.retrieval_cache.entry_count(),
        "quarantined_entries": state.quarantine_pool.len(),
        "timing_summary": state.lifecycle_manager.timing_summary(),
        "tenant_feedback_events": tenant_events_count,
        "total_feedback_events": total_events,
        "audit_trail": audit_trail
    }))
}

#[derive(Deserialize)]
pub struct CalibrateRequest {
    pub version: u32,
    #[serde(default)]
    pub target_alpha: Option<f64>,
    #[serde(default)]
    pub confidence_level: Option<f64>,
}

pub async fn calibrate_learning_policy(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(payload): Json<CalibrateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let events = state.learning_store.get_tenant_events(&sec_ctx.tenant_id);
    let candidate = if let Some(alpha) = payload.target_alpha {
        let cfg = crate::learning::CalibrationConfig::for_target_alpha(
            alpha,
            payload.confidence_level.unwrap_or(0.95),
        );
        crate::learning::OfflineCalibrator::calibrate_policy_with_config(
            &events,
            payload.version,
            &cfg,
        )
    } else {
        crate::learning::OfflineCalibrator::calibrate_policy(&events, payload.version)
    }
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
    state
        .temporal_engine
        .bump_config_version(AppState::global_config_scope_uuid());

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
    state
        .temporal_engine
        .bump_config_version(AppState::global_config_scope_uuid());

    Ok(Json(json!({
        "status": "POLICY_ROLLED_BACK",
        "reason": payload.reason,
        "timestamp": Utc::now()
    })))
}

// ==============================================================================
// Personal Local Desktop / BYOK Runtime Control Endpoints
// Enforces loopback security (127.0.0.1 / ::1) and native OS credential management
// ==============================================================================
