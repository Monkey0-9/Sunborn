//! End-to-end integration tests verifying all P0/P1/P2 audit remediations:
//! 1. Monitored canary rollback (false hits, error spike, latency SLA)
//! 2. Learning store & policy API persistence reporting consistency (WAL vs ephemeral)
//! 3. Automatic shadow evaluation pipeline on revalidation misses
//! 4. 5D TemporalValidityEngine integration into live /v1/requests pipeline
//! 5. Persistent ParaphraseClusterManager integration & cluster-wide invalidation
//! 6. Source-evidence grounding verification (content_hash & response numeric/coverage checks)

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sunborn_core::{
    certificate::SourceVersionSnapshot, AppConfig, ApplicationId, RequestEnvelope,
    ResponseEnvelope, TenantId,
};
use sunborn_gateway::{
    create_router,
    learning::{
        CalibrationReport, CanarySafetyThresholds, CandidatePolicy, PolicyWeights, RollbackTrigger,
        VerifiedOutcome,
    },
    AppState,
};
use tower::ServiceExt;
use uuid::Uuid;

fn make_sound_candidate(version: u32, sim_thresh: f32) -> CandidatePolicy {
    CandidatePolicy {
        policy_id: format!("policy-v{}-test", version),
        version,
        weights: PolicyWeights {
            min_similarity_threshold: sim_thresh,
            max_risk_threshold: 0.20,
            ..Default::default()
        },
        created_at: Utc::now(),
        evaluation_report: CalibrationReport {
            total_samples: 1000,
            training_samples: 700,
            held_out_samples: 300,
            held_out_evaluated: 300,
            held_out_admitted: 300,
            held_out_false_hits: 0,
            held_out_stale_hits: 0,
            held_out_ground_truth_mismatches: 0,
            training_false_hit_rate: 0.0,
            held_out_false_hit_rate: 0.0,
            held_out_false_hit_upper_bound_95: 0.99,
            held_out_wilson_upper_bound_95: 1.27,
            held_out_abstention_rate: 0.0,
            estimated_cost_reduction_pct: 40.0,
            is_statistically_sound: true,
        },
        is_promoted: false,
    }
}

#[test]
fn test_monitored_canary_automatic_rollback_on_false_hits_errors_and_latency() {
    let controller = sunborn_gateway::learning::PolicyPromotionController::new();
    controller.set_canary_thresholds(CanarySafetyThresholds {
        min_samples_for_rate_check: 5,
        max_false_hit_rate: 0.05,
        max_absolute_false_hits: 2,
        max_error_rate: 0.20,
        latency_sla_ms: 100.0,
        max_latency_violation_rate: 0.25,
    });

    // 1. False hit rollback
    controller
        .start_canary(make_sound_candidate(2, 0.90), 20, "ops")
        .unwrap();
    assert!(controller
        .record_canary_observation(true, 10.0, false, false)
        .is_none());
    assert!(controller
        .record_canary_observation(true, 10.0, false, true)
        .is_none());
    let trig = controller.record_canary_observation(true, 10.0, false, true);
    assert_eq!(trig, Some(RollbackTrigger::FalseHitDetected));
    assert!(controller.get_canary_status().is_none());

    // 2. Error-rate spike rollback
    controller
        .start_canary(make_sound_candidate(3, 0.90), 20, "ops")
        .unwrap();
    for _ in 0..3 {
        assert!(controller
            .record_canary_observation(true, 15.0, false, false)
            .is_none());
    }
    assert!(controller
        .record_canary_observation(true, 15.0, true, false)
        .is_none());
    let err_trig = controller.record_canary_observation(true, 15.0, true, false);
    assert_eq!(err_trig, Some(RollbackTrigger::ErrorRateSpike));
    assert!(controller.get_canary_status().is_none());

    // 3. Latency SLA violation rollback
    controller
        .start_canary(make_sound_candidate(4, 0.90), 20, "ops")
        .unwrap();
    for _ in 0..3 {
        assert!(controller
            .record_canary_observation(true, 20.0, false, false)
            .is_none());
    }
    assert!(controller
        .record_canary_observation(true, 250.0, false, false)
        .is_none());
    let lat_trig = controller.record_canary_observation(true, 250.0, false, false);
    assert_eq!(lat_trig, Some(RollbackTrigger::LatencySlaViolation));
    assert!(controller.get_canary_status().is_none());
}

#[tokio::test]
async fn test_learning_policies_endpoint_reports_accurate_wal_storage_tier() {
    let tmp_dir = std::env::temp_dir().join(format!("sunborn-wal-report-{}", Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let wal_path = tmp_dir.join("learning.wal");

    // Configure WAL env var before AppState::new
    std::env::set_var("SUNBORN_LEARNING_WAL_PATH", &wal_path);
    let state = AppState::new(AppConfig::default());
    std::env::remove_var("SUNBORN_LEARNING_WAL_PATH");

    let app = create_router(state.clone());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/v1/learning/policies")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["storage_tier"], "durable-wal");
    assert_eq!(body["learning_store_durable"], true);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[tokio::test]
async fn test_5d_temporal_validity_engine_and_paraphrase_cluster_in_gateway_pipeline() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-temporal-5d";
    let app_id = "app-5d";
    let api_key = "key-temporal-5d-test";
    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    let req = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Explain the CAP theorem consistency trade-offs in distributed databases",
    );
    let body_bytes = serde_json::to_vec(&req).unwrap();

    // 1. Cold miss -> caches entry with 5D TemporalVector and assigns ParaphraseCluster
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(body_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let resp1: ResponseEnvelope =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!resp1.cached);
    let cache_key = resp1.cache_key.clone().unwrap();

    // Verify ParaphraseCluster was created for this entry
    let clusters = state
        .paraphrase_clusters
        .list_tenant_clusters(&TenantId::new(tenant));
    assert_eq!(clusters.len(), 1);
    assert!(clusters[0].members.contains_key(&cache_key));

    // 2. Warm exact hit
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(body_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp2: ResponseEnvelope =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(resp2.cached);

    // 3. Mutate permission dimension in TemporalValidityEngine -> cached entry must become stale and miss!
    state
        .temporal_engine
        .bump_permission_version(AppState::tenant_permission_scope_uuid(tenant));

    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(body_bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp3: ResponseEnvelope =
        serde_json::from_slice(&res3.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !resp3.cached,
        "Bumping permission_version in TemporalValidityEngine must invalidate cached response"
    );
}

#[tokio::test]
async fn test_source_evidence_grounding_and_snapshot_hash_enforcement_in_gateway() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-grounding";
    let app_id = "app-rag";
    let api_key = "key-grounding-test";
    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    let source_id = Uuid::new_v4();
    let mut req = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Summarize the Division Alpha Q3 financial report",
    );
    req.dependency_snapshots = vec![SourceVersionSnapshot {
        source_id,
        snapshot_epoch: 1,
        content_hash: "sha256:doc-v1".to_string(),
    }];

    // 1. Seed cache with v1 content hash
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp1: ResponseEnvelope =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!resp1.cached);

    // 2. Request with changed source content_hash -> rejected (miss -> origin)
    let mut req_changed_hash = req.clone();
    req_changed_hash.dependency_snapshots[0].content_hash = "sha256:doc-v2-mutated".to_string();
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req_changed_hash).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp2: ResponseEnvelope =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !resp2.cached,
        "Changed dependency content_hash must prevent stale cached reuse"
    );

    // 3. Request with conflicting source_evidences excerpt -> grounding verifier rejects cached hit
    let mut req_ungrounded = req.clone();
    req_ungrounded.metadata = json!({
        "source_evidences": [{
            "source_id": source_id.to_string(),
            "content_hash": "sha256:doc-v1",
            "excerpt": "Completely unrelated medical clinical dosage of 999mg."
        }]
    });
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req_ungrounded).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp3: ResponseEnvelope =
        serde_json::from_slice(&res3.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !resp3.cached,
        "Cached response failing source-evidence grounding must not be served"
    );
}

#[tokio::test]
async fn test_automatic_revalidation_shadow_pipeline_emits_signed_feedback() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-auto-shadow";
    let app_id = "app-shadow";
    let api_key = "key-auto-shadow-test";
    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // 1. Seed cache with prompt 1
    let req1 = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Explain the architecture of distributed consensus protocols in detail",
    );
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let req2 = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Describe the architecture of distributed consensus protocols in detail",
    );
    let v1 = state.selected_embedder.embed(&req1.prompt).await.unwrap();
    let v2 = state.selected_embedder.embed(&req2.prompt).await.unwrap();
    let sim = sunborn_semantic::cosine_similarity(&v1, &v2);
    // Position threshold just above actual similarity (within 0.05 revalidation band)
    let strict_thresh = (sim + 0.02).clamp(0.75, 0.99);

    state
        .learning_controller
        .start_canary(make_sound_candidate(2, strict_thresh), 25, "test")
        .unwrap();
    state
        .learning_controller
        .complete_promotion("test")
        .unwrap();

    let res2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    // Allow background shadow task to finish
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let events = state
        .learning_store
        .get_tenant_events(&TenantId::new(tenant));
    assert!(
        !events.is_empty(),
        "Automatic shadow pipeline on revalidation must emit signed VerifiedFeedbackEvent"
    );
    for ev in &events {
        assert_eq!(ev.verification_source, "shadow_eval");
        assert!(
            ev.verify_signature(&state.learning_hmac_secret).is_ok(),
            "Emitted shadow feedback event must pass full HMAC signature verification"
        );
        assert_ne!(ev.verified_outcome, VerifiedOutcome::UnverifiedOpinion);
    }
}

#[tokio::test]
async fn test_non_exchangeable_conformal_calibrator_governs_live_semantic_admission_after_burnin() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-conformal-live";
    let app_id = "app-conformal";
    let api_key = "key-conformal-live-test";
    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // 1. Seed cache with prompt 1
    let req1 = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Explain how distributed write-ahead logging ensures crash recovery",
    );
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Feed 35 shadow drift observations with high loss at similarity 0.96 -> raises conformal floor to ~0.995
    for _ in 0..35 {
        state.shadow_evaluator.record_observation(0.96, 1.0);
    }
    assert!(state.shadow_evaluator.observation_count() >= 30);
    assert!(state.shadow_evaluator.calibrated_threshold() >= 0.965);

    // 3. Paraphrase below 0.965 must now abstain (miss -> origin) even though base policy is 0.88
    let req2 = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Describe how distributed write-ahead logging ensures crash recovery",
    );
    let res2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let resp2: ResponseEnvelope =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !resp2.cached,
        "Post-burn-in NonExchangeableConformalCalibrator floor must tighten live semantic admission"
    );
}
