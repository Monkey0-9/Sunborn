use std::collections::HashMap;
use sunborn_learning::{
    CachePolicy, DriftDetector, ExecutionRecord, SafetyGate, ShadowEvaluator, TelemetryWindow,
    WorkloadDistribution,
};

#[test]
fn test_shadow_evaluation_rejects_unsafe_policy() {
    let mut window = TelemetryWindow::new();
    // Populate 30 queries: 20 safe paraphrases at similarity 0.94, 10 negations at similarity 0.85
    for i in 0..20 {
        window.record(ExecutionRecord {
            query_id: format!("q-safe-{}", i),
            tenant_id: "tenant-a".into(),
            application_id: "app-1".into(),
            semantic_category: "paraphrase".into(),
            similarity_score: 0.94,
            was_reused: true,
            reported_false_hit: false,
            latency_origin_ms: 1200.0,
            latency_cache_ms: 5.0,
            token_cost_saved_usd: 0.015,
            timestamp_epoch_ms: 1700000000 + i as i64,
        });
    }
    for i in 0..10 {
        window.record(ExecutionRecord {
            query_id: format!("q-neg-{}", i),
            tenant_id: "tenant-a".into(),
            application_id: "app-1".into(),
            semantic_category: "negation".into(),
            similarity_score: 0.85,
            was_reused: false,
            reported_false_hit: true,
            latency_origin_ms: 1100.0,
            latency_cache_ms: 5.0,
            token_cost_saved_usd: 0.0,
            timestamp_epoch_ms: 1700000000 + i as i64,
        });
    }

    let evaluator = ShadowEvaluator::new();
    let gate = SafetyGate {
        max_tolerable_error_rate: 0.01,
        min_acceptable_hit_rate: 0.10,
        min_evaluation_samples: 25,
    };

    // Unsafe policy candidate lowering threshold to 0.80 will cause all 10 negations to hit
    let unsafe_policy = CachePolicy {
        version: 2,
        similarity_threshold: 0.80,
        ttl_seconds: 3600,
        risk_budget: 0.10,
        max_admission_anomaly: 0.50,
    };

    let result = evaluator.evaluate(&unsafe_policy, window.records(), &gate);
    assert!(
        !result.approved,
        "Unsafe policy must be rejected by safety gate"
    );
    assert!(
        result.projected_error_rate > 0.01,
        "Projected error rate should exceed gate"
    );
    assert!(!result.rejection_reasons.is_empty());
}

#[test]
fn test_shadow_evaluation_approves_safe_calibrated_policy() {
    let mut window = TelemetryWindow::new();
    for i in 0..25 {
        window.record(ExecutionRecord {
            query_id: format!("q-safe-{}", i),
            tenant_id: "tenant-a".into(),
            application_id: "app-1".into(),
            semantic_category: "paraphrase".into(),
            similarity_score: 0.95,
            was_reused: true,
            reported_false_hit: false,
            latency_origin_ms: 1000.0,
            latency_cache_ms: 4.0,
            token_cost_saved_usd: 0.01,
            timestamp_epoch_ms: 1700000000 + i as i64,
        });
    }

    let evaluator = ShadowEvaluator::new();
    let gate = SafetyGate::default();

    let safe_policy = CachePolicy {
        version: 2,
        similarity_threshold: 0.93,
        ttl_seconds: 3600,
        risk_budget: 0.02,
        max_admission_anomaly: 0.35,
    };

    let result = evaluator.evaluate(&safe_policy, window.records(), &gate);
    assert!(
        result.approved,
        "Safe policy must pass shadow gate: {:?}",
        result.rejection_reasons
    );
    assert_eq!(result.projected_error_rate, 0.0);
    assert!(result.projected_hit_rate >= 0.05);
}

#[test]
fn test_drift_detector_flags_workload_shift() {
    let mut baseline_counts = HashMap::new();
    baseline_counts.insert("paraphrase".into(), 80);
    baseline_counts.insert("negation".into(), 10);
    baseline_counts.insert("numeric".into(), 10);
    let baseline = WorkloadDistribution::from_counts(&baseline_counts);

    let detector = DriftDetector::new(baseline, 0.25);

    // Minor fluctuation: TVD should be small
    let mut minor_counts = HashMap::new();
    minor_counts.insert("paraphrase".into(), 78);
    minor_counts.insert("negation".into(), 11);
    minor_counts.insert("numeric".into(), 11);
    let minor = WorkloadDistribution::from_counts(&minor_counts);
    assert!(!detector.detect_drift(&minor));

    // Major shift: surge in adversarial queries
    let mut shifted_counts = HashMap::new();
    shifted_counts.insert("paraphrase".into(), 20);
    shifted_counts.insert("adversarial".into(), 60);
    shifted_counts.insert("negation".into(), 20);
    let shifted = WorkloadDistribution::from_counts(&shifted_counts);
    assert!(
        detector.detect_drift(&shifted),
        "Major workload shift must trigger drift detection"
    );
}
