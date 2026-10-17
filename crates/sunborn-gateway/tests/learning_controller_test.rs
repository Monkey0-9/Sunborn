use axum::body::Body;
use chrono::Utc;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use sunborn_core::{AppConfig, ApplicationId, RequestEnvelope, ResponseEnvelope, TenantId};
use sunborn_gateway::{
    create_router,
    learning::{
        calibration::CalibrationReport, CandidatePolicy, CounterfactualShadowEvaluator,
        OfflineCalibrator, PolicyPromotionController, PolicyWeights, RollbackTrigger,
        TenantLearningStore, VerifiedFeedbackEvent, VerifiedOutcome,
    },
    AppState,
};
use tower::ServiceExt;

#[test]
fn test_layer1_feedback_hmac_signature_verification_and_tamper_detection() {
    let secret = b"test-secret-key-for-hmac";
    let event_id = "ev-1001";
    let tenant_id = TenantId::new("tenant-learning-1");
    let cache_key = "cache:key:sha256:123";
    let outcome = VerifiedOutcome::VerifiedCorrect;
    let ts = Utc::now();

    let valid_signature =
        VerifiedFeedbackEvent::sign(event_id, &tenant_id, cache_key, &outcome, &ts, secret);

    let mut event = VerifiedFeedbackEvent {
        event_id: event_id.to_string(),
        tenant_id: tenant_id.clone(),
        application_id: ApplicationId::new("app-test"),
        cache_key: cache_key.to_string(),
        decision_type: "exact_hit".to_string(),
        prompt_hash: "hash_abc".to_string(),
        similarity_score: Some(0.95),
        conformal_risk_score: Some(0.01),
        latency_ms: 1.2,
        tokens_saved: 50,
        cost_saved_usd: 0.001,
        verified_outcome: outcome,
        verification_source: "origin_execution".to_string(),
        hmac_signature: valid_signature.clone(),
        timestamp: ts,
    };

    assert!(event.verify_signature(secret).is_ok());
    assert!(event.is_trustworthy());

    // 1. Tamper with signature
    event.hmac_signature = "bad_signature_value".to_string();
    assert!(event.verify_signature(secret).is_err());
    event.hmac_signature = valid_signature.clone();

    // 2. Tamper with outcome (flip outcome without valid HMAC)
    event.verified_outcome = VerifiedOutcome::VerifiedFalseHit;
    assert!(event.verify_signature(secret).is_err());
    event.verified_outcome = VerifiedOutcome::VerifiedCorrect;

    // 3. Tamper with cache_key
    event.cache_key = "cache:key:sha256:tampered".to_string();
    assert!(event.verify_signature(secret).is_err());
    event.cache_key = cache_key.to_string();

    // 4. Tamper with tenant_id
    event.tenant_id = TenantId::new("tenant-attacker");
    assert!(event.verify_signature(secret).is_err());
    event.tenant_id = tenant_id;

    // 5. Verify untrusted sources and opinions
    event.hmac_signature = valid_signature;
    event.verified_outcome = VerifiedOutcome::UnverifiedOpinion;
    assert!(!event.is_trustworthy());

    event.verified_outcome = VerifiedOutcome::VerifiedCorrect;
    event.verification_source = "untrusted_client".to_string();
    assert!(!event.is_trustworthy());
}

#[test]
fn test_layer2_tenant_safe_store_isolation_capacity_and_gdpr() {
    // Test store with capacity 5 to verify eviction bounding
    let store = Arc::new(TenantLearningStore::new(5));
    let t1 = TenantId::new("tenant-alpha");
    let t2 = TenantId::new("tenant-beta");

    // Insert 10 events for tenant 1; store should cap at 5
    for i in 0..10 {
        let ev = VerifiedFeedbackEvent {
            event_id: format!("ev-t1-{}", i),
            tenant_id: t1.clone(),
            application_id: ApplicationId::new("app-t1"),
            cache_key: format!("cache-key-{}", i),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: format!("prompt-{}", i),
            similarity_score: Some(0.90 + (i as f32 * 0.005)),
            conformal_risk_score: Some(0.05),
            latency_ms: 3.0,
            tokens_saved: 100,
            cost_saved_usd: 0.002,
            verified_outcome: if i % 2 == 0 {
                VerifiedOutcome::VerifiedCorrect
            } else {
                VerifiedOutcome::VerifiedFalseHit
            },
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        };
        assert!(store.record_event(ev).is_ok());
    }

    // Insert 3 events for tenant 2
    for i in 0..3 {
        let ev = VerifiedFeedbackEvent {
            event_id: format!("ev-t2-{}", i),
            tenant_id: t2.clone(),
            application_id: ApplicationId::new("app-t2"),
            cache_key: format!("cache-key-beta-{}", i),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: format!("prompt-beta-{}", i),
            similarity_score: Some(0.88),
            conformal_risk_score: Some(0.02),
            latency_ms: 2.1,
            tokens_saved: 80,
            cost_saved_usd: 0.0015,
            verified_outcome: VerifiedOutcome::VerifiedCorrect,
            verification_source: "shadow_eval".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        };
        assert!(store.record_event(ev).is_ok());
    }

    // Bounded capacity check: tenant alpha capped at 5
    assert_eq!(store.get_tenant_events(&t1).len(), 5);
    assert_eq!(store.tenant_event_count(&t1), 5);
    // Tenant beta has 3
    assert_eq!(store.get_tenant_events(&t2).len(), 3);
    assert_eq!(store.tenant_event_count(&t2), 3);
    assert_eq!(store.total_events(), 8);

    // GDPR purge of tenant alpha
    let purged = store.purge_tenant(&t1);
    assert_eq!(purged, 5);
    assert_eq!(store.get_tenant_events(&t1).len(), 0);
    assert_eq!(store.tenant_event_count(&t1), 0);
    // Tenant beta remains completely intact
    assert_eq!(store.get_tenant_events(&t2).len(), 3);
    assert_eq!(store.tenant_event_count(&t2), 3);
    assert_eq!(store.total_events(), 3);
}

#[test]
fn test_layer2_poison_defense_and_untrusted_feedback_rejection() {
    let store = Arc::new(TenantLearningStore::new(100));
    let tenant_id = TenantId::new("tenant-poison-test");

    // 1. Untrusted opinion rejection
    let untrusted_ev = VerifiedFeedbackEvent {
        event_id: "ev-untrusted-1".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: ApplicationId::new("app-test"),
        cache_key: "cache:key:1".to_string(),
        decision_type: "exact_hit".to_string(),
        prompt_hash: "hash_1".to_string(),
        similarity_score: Some(0.95),
        conformal_risk_score: Some(0.01),
        latency_ms: 1.0,
        tokens_saved: 10,
        cost_saved_usd: 0.0001,
        verified_outcome: VerifiedOutcome::UnverifiedOpinion,
        verification_source: "origin_execution".to_string(),
        hmac_signature: "sig".to_string(),
        timestamp: Utc::now(),
    };
    let err = store.record_event(untrusted_ev);
    assert!(err.is_err());

    // 2. Untrusted source rejection
    let untrusted_src = VerifiedFeedbackEvent {
        event_id: "ev-untrusted-2".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: ApplicationId::new("app-test"),
        cache_key: "cache:key:2".to_string(),
        decision_type: "exact_hit".to_string(),
        prompt_hash: "hash_2".to_string(),
        similarity_score: Some(0.95),
        conformal_risk_score: Some(0.01),
        latency_ms: 1.0,
        tokens_saved: 10,
        cost_saved_usd: 0.0001,
        verified_outcome: VerifiedOutcome::VerifiedCorrect,
        verification_source: "untrusted_third_party".to_string(),
        hmac_signature: "sig".to_string(),
        timestamp: Utc::now(),
    };
    let err = store.record_event(untrusted_src);
    assert!(err.is_err());

    // 3. Poison defense: record 1 initial VerifiedCorrect event for cache key 'poison-key'
    let target_key = "poison-target-cache-key";
    let base_ev = VerifiedFeedbackEvent {
        event_id: "ev-base".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: ApplicationId::new("app-test"),
        cache_key: target_key.to_string(),
        decision_type: "semantic_hit".to_string(),
        prompt_hash: "hash_target".to_string(),
        similarity_score: Some(0.92),
        conformal_risk_score: Some(0.03),
        latency_ms: 2.0,
        tokens_saved: 50,
        cost_saved_usd: 0.001,
        verified_outcome: VerifiedOutcome::VerifiedCorrect,
        verification_source: "origin_execution".to_string(),
        hmac_signature: "sig".to_string(),
        timestamp: Utc::now(),
    };
    assert!(store.record_event(base_ev).is_ok());

    // Now insert 5 contradictory (VerifiedFalseHit) events for the same cache key
    for i in 0..5 {
        let contradictory_ev = VerifiedFeedbackEvent {
            event_id: format!("ev-contradictory-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-test"),
            cache_key: target_key.to_string(),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: "hash_target".to_string(),
            similarity_score: Some(0.92),
            conformal_risk_score: Some(0.03),
            latency_ms: 2.0,
            tokens_saved: 50,
            cost_saved_usd: 0.001,
            verified_outcome: VerifiedOutcome::VerifiedFalseHit,
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        };
        assert!(store.record_event(contradictory_ev).is_ok());
    }

    // Attempt to insert an event with VerifiedCorrect (which now has 5 contradictory VerifiedFalseHit events)
    let poison_ev = VerifiedFeedbackEvent {
        event_id: "ev-poison-attempt".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: ApplicationId::new("app-test"),
        cache_key: target_key.to_string(),
        decision_type: "semantic_hit".to_string(),
        prompt_hash: "hash_target".to_string(),
        similarity_score: Some(0.92),
        conformal_risk_score: Some(0.03),
        latency_ms: 2.0,
        tokens_saved: 50,
        cost_saved_usd: 0.001,
        verified_outcome: VerifiedOutcome::VerifiedCorrect,
        verification_source: "origin_execution".to_string(),
        hmac_signature: "sig".to_string(),
        timestamp: Utc::now(),
    };
    let poison_res = store.record_event(poison_ev);
    assert!(poison_res.is_err());
    let err_msg = poison_res.err().unwrap().to_string();
    assert!(err_msg.contains("Poisoning defense triggered"));
}

#[test]
fn test_layer3_offline_calibration_on_held_out_splits() {
    let tenant_id = TenantId::new("tenant-calib");
    let mut events = Vec::new();

    for i in 0..100 {
        let is_false_hit = i < 5; // 5% false hits in dataset
        events.push(VerifiedFeedbackEvent {
            event_id: format!("ev-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-calib"),
            cache_key: format!("key-{}", i),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: format!("prompt-{}", i),
            similarity_score: Some(if is_false_hit { 0.86 } else { 0.94 }),
            conformal_risk_score: Some(if is_false_hit { 0.35 } else { 0.05 }),
            latency_ms: 2.5,
            tokens_saved: 120,
            cost_saved_usd: 0.003,
            verified_outcome: if is_false_hit {
                VerifiedOutcome::VerifiedFalseHit
            } else {
                VerifiedOutcome::VerifiedCorrect
            },
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        });
    }

    let candidate = OfflineCalibrator::calibrate_policy(&events, 2)
        .expect("offline calibration should succeed on >=10 samples");

    assert_eq!(candidate.version, 2);
    assert_eq!(candidate.evaluation_report.total_samples, 100);
    assert_eq!(candidate.evaluation_report.training_samples, 70);
    assert_eq!(candidate.evaluation_report.held_out_samples, 30);
    assert!(candidate.evaluation_report.is_statistically_sound);
    assert!(candidate.weights.min_similarity_threshold >= 0.85);
}

#[test]
fn test_layer3_offline_calibration_safety_gates() {
    let tenant_id = TenantId::new("tenant-calib-gate");

    // 1. Minimum sample size gate: <10 samples must be rejected
    let mut small_events = Vec::new();
    for i in 0..8 {
        small_events.push(VerifiedFeedbackEvent {
            event_id: format!("ev-small-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-test"),
            cache_key: format!("key-{}", i),
            decision_type: "exact_hit".to_string(),
            prompt_hash: format!("hash-{}", i),
            similarity_score: Some(0.95),
            conformal_risk_score: Some(0.01),
            latency_ms: 1.0,
            tokens_saved: 20,
            cost_saved_usd: 0.0005,
            verified_outcome: VerifiedOutcome::VerifiedCorrect,
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        });
    }
    let calib_err = OfflineCalibrator::calibrate_policy(&small_events, 2);
    assert!(calib_err.is_err());
    assert!(calib_err
        .err()
        .unwrap()
        .to_string()
        .contains("Insufficient sample size"));

    // 2. Statistical soundness failure: excessive false hits in held-out set
    let mut high_error_events = Vec::new();
    for i in 0..20 {
        // High false hit count: 50% false hits
        let is_false_hit = i % 2 == 0;
        high_error_events.push(VerifiedFeedbackEvent {
            event_id: format!("ev-err-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-test"),
            cache_key: format!("key-err-{}", i),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: format!("hash-err-{}", i),
            similarity_score: Some(0.99), // High similarity to bypass threshold
            conformal_risk_score: Some(0.01),
            latency_ms: 2.0,
            tokens_saved: 50,
            cost_saved_usd: 0.001,
            verified_outcome: if is_false_hit {
                VerifiedOutcome::VerifiedFalseHit
            } else {
                VerifiedOutcome::VerifiedCorrect
            },
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        });
    }

    let candidate = OfflineCalibrator::calibrate_policy(&high_error_events, 3)
        .expect("calibration computation succeeds");
    // With 50% false hits on held-out data, statistical soundness must be false (false hit rate > 1%)
    assert!(!candidate.evaluation_report.is_statistically_sound);

    // 3. SB-P0-01: Reject empty held-out scored denominator
    let mut unscored_events = Vec::new();
    for i in 0..15 {
        unscored_events.push(VerifiedFeedbackEvent {
            event_id: format!("ev-unscored-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-test"),
            cache_key: format!("key-{}", i),
            decision_type: "exact_hit".to_string(),
            prompt_hash: format!("hash-{}", i),
            similarity_score: if i < 10 { Some(0.92) } else { None }, // held-out split (indices 10..15) has None
            conformal_risk_score: Some(0.01),
            latency_ms: 1.0,
            tokens_saved: 20,
            cost_saved_usd: 0.0005,
            verified_outcome: VerifiedOutcome::VerifiedCorrect,
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        });
    }
    let empty_held_out_err = OfflineCalibrator::calibrate_policy(&unscored_events, 3);
    assert!(empty_held_out_err.is_err());
    let err_str = empty_held_out_err.err().unwrap().to_string();
    assert!(err_str.contains("Held-out validation split contains no scored events"));
}

#[test]
fn test_layer4_counterfactual_shadow_evaluator() {
    let active_weights = PolicyWeights {
        min_similarity_threshold: 0.85,
        max_risk_threshold: 0.30,
        ..Default::default()
    };

    let candidate_policy = CandidatePolicy {
        policy_id: "policy-v2-eval".to_string(),
        version: 2,
        weights: PolicyWeights {
            min_similarity_threshold: 0.92,
            max_risk_threshold: 0.20,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.1,
            estimated_cost_reduction_pct: 12.5,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };

    let tenant_id = TenantId::new("tenant-shadow");
    let mut events = Vec::new();

    // Feedback events where similarity is 0.88 with high risk (active admits false hit, candidate abstains)
    for i in 0..20 {
        events.push(VerifiedFeedbackEvent {
            event_id: format!("ev-sh-{}", i),
            tenant_id: tenant_id.clone(),
            application_id: ApplicationId::new("app-shadow"),
            cache_key: format!("k-{}", i),
            decision_type: "semantic_hit".to_string(),
            prompt_hash: format!("p-{}", i),
            similarity_score: Some(0.88),
            conformal_risk_score: Some(0.25),
            latency_ms: 3.1,
            tokens_saved: 100,
            cost_saved_usd: 0.002,
            verified_outcome: VerifiedOutcome::VerifiedFalseHit,
            verification_source: "origin_execution".to_string(),
            hmac_signature: "sig".to_string(),
            timestamp: Utc::now(),
        });
    }

    let metrics = CounterfactualShadowEvaluator::evaluate_counterfactual(
        &events,
        &active_weights,
        &candidate_policy,
    );

    assert_eq!(metrics.total_evaluated, 20);
    assert_eq!(metrics.active_policy_false_hits, 20);
    assert_eq!(metrics.candidate_policy_false_hits, 0);
    assert_eq!(metrics.candidate_abstentions, 20);
    assert_eq!(metrics.false_hit_reduction_pct, 100.0);
    assert!(metrics.is_candidate_superior);
}

#[test]
fn test_layer5_canary_rollout_invariants_and_routing() {
    let controller = PolicyPromotionController::new();

    // 1. Reject candidate policy failing statistical soundness
    let unsound_candidate = CandidatePolicy {
        policy_id: "policy-unsound".to_string(),
        version: 2,
        weights: PolicyWeights::default(),
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.05,
            held_out_abstention_rate: 0.0,
            estimated_cost_reduction_pct: 0.0,
            is_statistically_sound: false,
        },
        is_promoted: false,
    };
    let err = controller.start_canary(unsound_candidate, 10, "admin");
    assert!(err.is_err());
    assert!(err
        .err()
        .unwrap()
        .to_string()
        .contains("statistical soundness"));

    // 2. Reject candidate policy violating min_similarity_threshold security invariant (<0.75)
    let unsafe_threshold_candidate = CandidatePolicy {
        policy_id: "policy-unsafe-threshold".to_string(),
        version: 2,
        weights: PolicyWeights {
            min_similarity_threshold: 0.70, // Violates invariant
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.0,
            estimated_cost_reduction_pct: 10.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };
    let err = controller.start_canary(unsafe_threshold_candidate, 10, "admin");
    assert!(err.is_err());
    assert!(err
        .err()
        .unwrap()
        .to_string()
        .contains("minimum similarity threshold cannot be below 0.75"));

    // 3. Canary percentage clamping: requesting 50% clamps to 25% max canary limit
    let valid_candidate = CandidatePolicy {
        policy_id: "policy-v2-clamped".to_string(),
        version: 2,
        weights: PolicyWeights {
            min_similarity_threshold: 0.92,
            max_risk_threshold: 0.20,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.02,
            estimated_cost_reduction_pct: 12.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };
    controller
        .start_canary(valid_candidate.clone(), 50, "admin")
        .expect("canary start succeeds");

    let canary_status = controller.get_canary_status().unwrap();
    assert_eq!(canary_status.1, 25); // Clamped to 25%

    // 4. Test deterministic canary routing via select_policy_for_request
    // Hashes where hash % 100 < 25 receive canary weights (min_similarity_threshold = 0.92)
    let canary_routed_weights = controller.select_policy_for_request(10);
    assert_eq!(canary_routed_weights.min_similarity_threshold, 0.92);

    // Hashes where hash % 100 >= 25 receive baseline weights (min_similarity_threshold = 0.88)
    let baseline_routed_weights = controller.select_policy_for_request(85);
    assert_eq!(baseline_routed_weights.min_similarity_threshold, 0.88);
}

#[test]
fn test_layer5_canary_rollout_promotion_and_automated_rollback() {
    let controller = PolicyPromotionController::new();

    let candidate = CandidatePolicy {
        policy_id: "policy-v2-canary".to_string(),
        version: 2,
        weights: PolicyWeights {
            min_similarity_threshold: 0.90,
            max_risk_threshold: 0.20,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.05,
            estimated_cost_reduction_pct: 15.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };

    // Staged canary rollout at 15%
    controller
        .start_canary(candidate.clone(), 15, "test-runner")
        .expect("canary start should succeed");

    let canary_status = controller.get_canary_status();
    assert!(canary_status.is_some());
    let (active_canary, pct) = canary_status.unwrap();
    assert_eq!(active_canary.policy_id, "policy-v2-canary");
    assert_eq!(pct, 15);

    // Rollback trigger (e.g. false hit detected in canary)
    controller
        .trigger_rollback(
            RollbackTrigger::FalseHitDetected,
            "simulated drift detection",
        )
        .expect("rollback should succeed");

    assert!(controller.get_canary_status().is_none());
    assert_eq!(
        controller.get_active_policy().policy_id,
        "policy-v1-baseline"
    );

    let audit = controller.get_audit_trail();
    assert!(audit.iter().any(|l| l.action == "ROLLED_BACK"));

    // Full promotion lifecycle test:
    // Start canary again and promote to production
    controller
        .start_canary(candidate, 20, "deployer")
        .expect("canary restart succeeds");
    let promoted = controller
        .complete_promotion("admin-promoter")
        .expect("full promotion succeeds");
    assert!(promoted.is_promoted);
    assert!(controller.get_canary_status().is_none());
    assert_eq!(controller.get_active_policy().policy_id, "policy-v2-canary");
    assert_eq!(
        controller.get_active_weights().min_similarity_threshold,
        0.90
    );

    let audit_after = controller.get_audit_trail();
    assert!(audit_after.iter().any(|l| l.action == "FULLY_PROMOTED"));

    // Attempting complete_promotion when no canary is active must fail
    assert!(controller.complete_promotion("admin-promoter").is_err());
}

#[tokio::test]
async fn test_learning_http_api_endpoints_full_lifecycle() {
    let state = AppState::new(sunborn_core::AppConfig::default());
    let app = create_router(state.clone());

    let admin_tenant = TenantId::new("tenant-system");
    let user_tenant = TenantId::new("tenant-regular");

    // Register test admin key
    state.api_key_registry.register_key(
        "test-key-admin-learning",
        admin_tenant.clone(),
        ApplicationId::new("app-admin"),
        vec!["admin".to_string(), "read".to_string(), "write".to_string()],
        true,
    );
    let admin_auth = "Bearer test-key-admin-learning";

    // Register regular (non-admin) key
    state.api_key_registry.register_key(
        "test-key-regular-user",
        user_tenant.clone(),
        ApplicationId::new("app-user"),
        vec!["read".to_string(), "write".to_string()],
        false,
    );
    let user_auth = "Bearer test-key-regular-user";

    // 1. GET /v1/learning/policies
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/learning/policies")
                .header("Authorization", admin_auth)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["active_policy"]["policy_id"], "policy-v1-baseline");

    // 2. Cross-tenant feedback injection check:
    // Non-admin user trying to submit feedback for admin_tenant must be rejected with 401 Unauthorized
    let cross_tenant_payload = json!({
        "event_id": "ev-cross-1",
        "tenant_id": "tenant-system",
        "application_id": "app-admin",
        "cache_key": "cache:cross:1",
        "decision_type": "exact_hit",
        "prompt_hash": "hash_cross",
        "similarity_score": 0.98,
        "conformal_risk_score": 0.01,
        "latency_ms": 1.5,
        "tokens_saved": 40,
        "cost_saved_usd": 0.001,
        "verified_outcome": "VerifiedCorrect",
        "verification_source": "origin_execution",
        "hmac_signature": "mock_sig",
        "timestamp": Utc::now()
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/feedback")
                .header("Authorization", user_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(cross_tenant_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2b. Invalid HMAC signature rejection check:
    let bad_sig_payload = json!({
        "event_id": "ev-bad-sig-1",
        "tenant_id": "tenant-system",
        "application_id": "app-admin",
        "cache_key": "cache:bad:1",
        "decision_type": "exact_hit",
        "prompt_hash": "hash_bad",
        "similarity_score": 0.98,
        "conformal_risk_score": 0.01,
        "latency_ms": 1.5,
        "tokens_saved": 40,
        "cost_saved_usd": 0.001,
        "verified_outcome": "VerifiedCorrect",
        "verification_source": "origin_execution",
        "hmac_signature": "invalid_forged_hmac_signature",
        "timestamp": Utc::now()
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/feedback")
                .header("Authorization", admin_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(bad_sig_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 3. Record 12 verified feedback events for admin_tenant to enable calibration
    let secret = &state.learning_hmac_secret;
    for i in 0..12 {
        let event_id = format!("ev-api-{}", i);
        let cache_key = format!("cache:test:{}", i);
        let outcome = VerifiedOutcome::VerifiedCorrect;
        let ts = Utc::now();
        let sig = VerifiedFeedbackEvent::sign(
            &event_id,
            &admin_tenant,
            &cache_key,
            &outcome,
            &ts,
            secret,
        );

        let fb_payload = json!({
            "event_id": event_id,
            "tenant_id": "tenant-system",
            "application_id": "app-admin",
            "cache_key": cache_key,
            "decision_type": "semantic_hit",
            "prompt_hash": format!("hash_{}", i),
            "similarity_score": 0.96,
            "conformal_risk_score": 0.01,
            "latency_ms": 1.5,
            "tokens_saved": 40,
            "cost_saved_usd": 0.001,
            "verified_outcome": "VerifiedCorrect",
            "verification_source": "origin_execution",
            "hmac_signature": sig,
            "timestamp": ts
        });

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/learning/feedback")
                    .header("Authorization", admin_auth)
                    .header("Content-Type", "application/json")
                    .body(Body::from(fb_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::CREATED);
    }

    // 4. POST /v1/learning/calibrate
    let calib_req = json!({ "version": 2 });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/calibrate")
                .header("Authorization", admin_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(calib_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let calib_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(calib_resp["status"], "POLICY_CALIBRATED");
    let candidate = calib_resp["candidate_policy"].clone();

    // 5. POST /v1/learning/evaluate (counterfactual shadow evaluation)
    let eval_req = json!({ "candidate_policy": candidate });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/evaluate")
                .header("Authorization", admin_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(eval_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let eval_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(eval_resp["status"], "COUNTERFACTUAL_EVALUATION_COMPLETED");

    // 6. Security gate: non-admin trying to start canary rollout must be rejected with 403 Forbidden
    let canary_payload = json!({
        "candidate_policy": candidate,
        "canary_percentage": 10
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/canary")
                .header("Authorization", user_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(canary_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 7. Admin starts canary rollout
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/canary")
                .header("Authorization", admin_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(canary_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // 8. Security gate: non-admin trying to promote policy must be rejected with 403 Forbidden
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/promote")
                .header("Authorization", user_auth)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 9. Admin promotes candidate policy to 100% production
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/promote")
                .header("Authorization", admin_auth)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let promote_resp: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(promote_resp["status"], "POLICY_FULLY_PROMOTED");

    // 10. Security gate: non-admin trying to trigger rollback must be rejected with 403 Forbidden
    let rollback_payload = json!({
        "reason": "Test emergency rollback verification via API"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/rollback")
                .header("Authorization", user_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(rollback_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 11. Admin triggers emergency rollback
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/rollback")
                .header("Authorization", admin_auth)
                .header("Content-Type", "application/json")
                .body(Body::from(rollback_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_live_policy_driven_decision_engine_dynamic_threshold_evaluation() {
    let mut config = AppConfig::default();
    config.cache.semantic_cache_enabled = true;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let tenant = TenantId::new("tenant-policy-test");
    sunborn_gateway::middleware::auth::get_global_registry().register_key(
        "test-key-policy-test",
        tenant.clone(),
        ApplicationId::new("app-default"),
        vec!["*".to_string()],
        true,
    );
    let auth_header = "Bearer test-key-policy-test";

    // 1. Initial request: populate cache and vector store with base query
    let initial_req = RequestEnvelope::new(
        "tenant-policy-test",
        "app-default",
        "gpt-4o",
        "Explain the fundamentals of quantum computing algorithms",
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&initial_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let initial_resp: ResponseEnvelope = serde_json::from_slice(&body).unwrap();
    assert!(!initial_resp.cached);

    // 2. Candidate query that has high similarity (e.g. ~0.85-0.92) but not 1.0 exact match
    let similar_req = RequestEnvelope::new(
        "tenant-policy-test",
        "app-default",
        "gpt-4o",
        "Explain the fundamentals of quantum computing principles",
    );

    // Check similarity between the two embeddings using deterministic embedder
    let v1 = state
        .selected_embedder
        .embed(&initial_req.prompt)
        .await
        .unwrap();
    let v2 = state
        .selected_embedder
        .embed(&similar_req.prompt)
        .await
        .unwrap();
    let sim = sunborn_semantic::embedding::cosine_similarity(&v1, &v2);
    assert!(
        sim > 0.75 && sim < 1.0,
        "Similarity should be in intermediate range: {}",
        sim
    );

    // 3. Set a strict policy requiring similarity >= 0.99 (higher than sim)
    let strict_candidate = CandidatePolicy {
        policy_id: "policy-strict-eval".to_string(),
        version: 10,
        weights: PolicyWeights {
            min_similarity_threshold: 0.99,
            max_risk_threshold: 0.20,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.05,
            estimated_cost_reduction_pct: 10.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };
    state
        .learning_controller
        .start_canary(strict_candidate, 100, "test-runner")
        .unwrap();
    state
        .learning_controller
        .complete_promotion("test-runner")
        .unwrap();

    // Query must MISS under strict policy
    let res_strict = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&similar_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body_strict = res_strict.into_body().collect().await.unwrap().to_bytes();
    let resp_strict: ResponseEnvelope = serde_json::from_slice(&body_strict).unwrap();
    assert!(
        !resp_strict.cached,
        "Should miss when similarity {} < threshold 0.99",
        sim
    );

    // 4. Update and promote a permissive policy requiring similarity <= 0.75
    let permissive_candidate = CandidatePolicy {
        policy_id: "policy-permissive-eval".to_string(),
        version: 11,
        weights: PolicyWeights {
            min_similarity_threshold: 0.75,
            max_risk_threshold: 0.30,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 50,
            training_samples: 35,
            held_out_samples: 15,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_abstention_rate: 0.02,
            estimated_cost_reduction_pct: 35.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    };
    state
        .learning_controller
        .start_canary(permissive_candidate, 100, "test-runner")
        .unwrap();
    state
        .learning_controller
        .complete_promotion("test-runner")
        .unwrap();

    // Fresh similar query that is not yet in exact cache
    let permissive_req = RequestEnvelope::new(
        "tenant-policy-test",
        "app-default",
        "gpt-4o",
        "Explain the fundamentals of quantum computing architectures",
    );
    let v_perm = state
        .selected_embedder
        .embed(&permissive_req.prompt)
        .await
        .unwrap();
    let sim_perm = sunborn_semantic::embedding::cosine_similarity(&v1, &v_perm);
    assert!(
        sim_perm >= 0.75,
        "Permissive query similarity {} must be >= 0.75",
        sim_perm
    );

    // Query must now HIT semantic cache under permissive policy
    let res_permissive = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&permissive_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body_permissive = res_permissive
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let resp_permissive: ResponseEnvelope = serde_json::from_slice(&body_permissive).unwrap();
    assert!(
        resp_permissive.cached,
        "Should hit when similarity {} >= threshold 0.75",
        sim_perm
    );
    assert_eq!(
        resp_permissive.decision.cache_path,
        sunborn_core::CachePath::Semantic
    );
}
