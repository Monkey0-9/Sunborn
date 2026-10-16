use axum::body::Body;
use chrono::Utc;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use sunborn_core::{ApplicationId, TenantId};
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
fn test_layer1_feedback_hmac_signature_verification() {
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
        hmac_signature: valid_signature,
        timestamp: ts,
    };

    assert!(event.verify_signature(secret).is_ok());
    assert!(event.is_trustworthy());

    // Tamper with signature
    event.hmac_signature = "bad_signature_value".to_string();
    assert!(event.verify_signature(secret).is_err());
}

#[test]
fn test_layer2_tenant_safe_store_and_gdpr_purge() {
    let store = Arc::new(TenantLearningStore::new(100));
    let t1 = TenantId::new("tenant-alpha");
    let t2 = TenantId::new("tenant-beta");

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

    for i in 0..5 {
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

    assert_eq!(store.get_tenant_events(&t1).len(), 10);
    assert_eq!(store.get_tenant_events(&t2).len(), 5);
    assert_eq!(store.total_events(), 15);

    // GDPR purge of tenant alpha
    let purged = store.purge_tenant(&t1);
    assert_eq!(purged, 10);
    assert_eq!(store.get_tenant_events(&t1).len(), 0);
    // Tenant beta remains completely intact
    assert_eq!(store.get_tenant_events(&t2).len(), 5);
    assert_eq!(store.total_events(), 5);
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
    assert!(candidate.evaluation_report.total_samples == 100);
    assert!(candidate.evaluation_report.training_samples == 70);
    assert!(candidate.evaluation_report.held_out_samples == 30);
    assert!(candidate.evaluation_report.is_statistically_sound);
    assert!(candidate.weights.min_similarity_threshold >= 0.85);
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
fn test_layer5_canary_rollout_and_automated_rollback() {
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
}

#[tokio::test]
async fn test_learning_http_api_endpoints() {
    let state = AppState::new(sunborn_core::AppConfig::default());
    let app = create_router(state.clone());

    // Register test admin key
    state.api_key_registry.register_key(
        "test-key-admin-learning",
        TenantId::new("tenant-system"),
        ApplicationId::new("app-admin"),
        vec!["admin".to_string(), "read".to_string(), "write".to_string()],
        true,
    );
    let auth_header = "Bearer test-key-admin-learning";

    // 1. GET /v1/learning/policies
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/learning/policies")
                .header("Authorization", auth_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["active_policy"]["policy_id"], "policy-v1-baseline");

    // 2. POST /v1/learning/feedback
    let fb_payload = json!({
        "event_id": "ev-api-1",
        "tenant_id": "tenant-system",
        "application_id": "app-admin",
        "cache_key": "cache:test:1",
        "decision_type": "exact_hit",
        "prompt_hash": "hash_123",
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
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(fb_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. POST /v1/learning/canary
    let canary_payload = json!({
        "candidate_policy": {
            "policy_id": "policy-v2-api-canary",
            "version": 2,
            "weights": {
                "min_similarity_threshold": 0.90,
                "max_risk_threshold": 0.20,
                "conformal_alpha": 0.01,
                "prefetch_confidence_threshold": 0.70,
                "eviction_staleness_weight": 0.40,
                "eviction_cost_weight": 0.60
            },
            "created_at": Utc::now(),
            "evaluation_report": {
                "total_samples": 40,
                "training_samples": 28,
                "held_out_samples": 12,
                "training_false_hit_rate": 0.0,
                "held_out_false_hit_rate": 0.0,
                "held_out_abstention_rate": 0.02,
                "estimated_cost_reduction_pct": 14.0,
                "is_statistically_sound": true
            },
            "is_promoted": false
        },
        "canary_percentage": 10
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/canary")
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(canary_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // 4. POST /v1/learning/rollback
    let rollback_payload = json!({
        "reason": "Test emergency rollback verification via API"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/learning/rollback")
                .header("Authorization", auth_header)
                .header("Content-Type", "application/json")
                .body(Body::from(rollback_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}
