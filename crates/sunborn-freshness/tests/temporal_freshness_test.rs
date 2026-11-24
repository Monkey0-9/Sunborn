use chrono::{Duration, Utc};
use sunborn_freshness::invalidation_engine::DependencyGraphInvalidator;
use sunborn_freshness::temporal_model::{
    CausalEvaluationResult, ConfidenceDecayPolicy, FreshnessVector, TemporalState,
    TemporalValidityEngine,
};
use uuid::Uuid;

#[test]
fn test_multi_dimensional_freshness_decay_and_invalidation() {
    let doc_id = Uuid::new_v4();
    let engine = TemporalValidityEngine::new();

    let state = TemporalState {
        document_version: 1,
        permission_version: 1,
        config_version: 1,
        model_version: 1,
        epoch: 100,
    };
    engine.register_state(doc_id, state);

    let vector = FreshnessVector {
        required_doc_version: 1,
        required_perm_version: 1,
        required_config_version: 1,
        required_model_version: 1,
        max_staleness_epoch: 105,
    };

    assert!(engine.evaluate_freshness(&doc_id, &vector, 102).is_fresh());

    // Update document version to 2 -> stale document
    engine.update_document_version(doc_id, 2);
    let res = engine.evaluate_freshness(&doc_id, &vector, 102);
    assert!(!res.is_fresh());
    assert_eq!(
        res,
        CausalEvaluationResult::StaleDocument {
            expected: 1,
            current: 2
        }
    );

    // Permission change test
    let perm_doc = Uuid::new_v4();
    engine.register_state(perm_doc, state);
    engine.update_permission_version(perm_doc, 3);
    assert_eq!(
        engine.evaluate_freshness(&perm_doc, &vector, 102),
        CausalEvaluationResult::StalePermission {
            expected: 1,
            current: 3
        }
    );

    // Epoch expired test
    let epoch_doc = Uuid::new_v4();
    engine.register_state(epoch_doc, state);
    assert_eq!(
        engine.evaluate_freshness(&epoch_doc, &vector, 110),
        CausalEvaluationResult::ExpiredEpoch {
            max_epoch: 105,
            current_epoch: 110
        }
    );
}

#[test]
fn test_dependency_invalidation_and_confidence_decay() {
    let invalidator = DependencyGraphInvalidator::new();
    let doc_id = Uuid::new_v4();

    invalidator.record_dependency(doc_id, "cache_key_query1");
    invalidator.record_dependency(doc_id, "cache_key_query2");
    assert_eq!(invalidator.count_dependents(&doc_id), 2);

    let affected = invalidator.invalidate_source(&doc_id);
    assert_eq!(affected.len(), 2);
    assert_eq!(invalidator.count_dependents(&doc_id), 0);

    // Confidence decay test
    let now = Utc::now();
    let created = now - Duration::seconds(30);
    let confidence = TemporalValidityEngine::compute_confidence(
        created,
        now,
        ConfidenceDecayPolicy::Linear { ttl_seconds: 60 },
    );
    assert!((confidence - 0.5).abs() < 0.05);
}
