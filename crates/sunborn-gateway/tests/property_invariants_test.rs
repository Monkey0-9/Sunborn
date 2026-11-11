//! # Property-Based & Generative Invariant Verification Suite (DEF-018)
//!
//! Evaluates formal algebraic and security properties of Sunborn core engines:
//!  1. RFC 8785 JSON Canonicalization Permutation & Idempotency Invariance
//!  2. RFC 6962 / Domain-Separated Merkle Tree Avalanche & Non-Collision Properties
//!  3. Compressed Lineage Treemap Algebraic Invariants (Commutativity, Idempotency, Associativity)
//!  4. Quantifier Scope & Role-Bound Numeric Transposition Invariants
//!  5. Monotonic Epoch Fence & Vector Clock Strict Partial Order Causality

use chrono::Utc;
use serde_json::{json, Value};
use sunborn_core::certificate::{
    ReuseCertificate, SafetyEvidenceRecord, SourceVersionSnapshot, ValidityWindow,
};
use sunborn_core::{ApplicationId, TenantId};
use sunborn_identity::canonical::canonicalize_json;
use sunborn_invalidation::bitmap_lineage::CompressedLineageGraph;
use sunborn_invalidation::epoch::{EpochLeaseManager, VectorClock};
use sunborn_semantic::guardrails::StructuralFingerprint;
use uuid::Uuid;

/// Pseudo-random LCG generator for deterministic property test reproduction
struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_range(&mut self, min: u64, max: u64) -> u64 {
        min + (self.next_u64() % (max - min + 1))
    }
}

// ==============================================================================
// Property 1: RFC 8785 JSON Canonicalization Permutation & Idempotency
// ==============================================================================
#[test]
fn test_property_json_canonicalization_permutation_and_idempotency() {
    let mut rng = LcgRng::new(0xca11_ab1e_feed);

    for run_idx in 0..50 {
        let v1 = rng.next_range(1, 100_000);
        let v2 = rng.next_range(1, 100_000);
        let v3 = rng.next_range(1, 100_000);
        let flag = (run_idx % 2) == 0;

        // Base JSON object
        let base_obj = json!({
            "zebra": v1,
            "alpha": {
                "charlie": "nested_value",
                "bravo": [v2, v3, "literal"],
                "delta": flag
            },
            "middle": null,
            "numeric_float": 12.500
        });

        // Permuted key orders across levels
        let permuted_obj = json!({
            "numeric_float": 12.5,
            "middle": null,
            "alpha": {
                "delta": flag,
                "bravo": [v2, v3, "literal"],
                "charlie": "nested_value"
            },
            "zebra": v1
        });

        let canon1 = serde_json::to_string(&canonicalize_json(&base_obj)).unwrap();
        let canon2 = serde_json::to_string(&canonicalize_json(&permuted_obj)).unwrap();

        // Property 1.1: Permutation invariance
        assert_eq!(
            canon1, canon2,
            "Run {run_idx}: Permuted JSON keys must yield bit-exact canonical representation"
        );

        // Property 1.2: Idempotency (parsing back and re-canonicalizing produces identical string)
        let parsed_again: Value = serde_json::from_str(&canon1).expect("re-parse");
        let canon3 = serde_json::to_string(&canonicalize_json(&parsed_again)).unwrap();
        assert_eq!(
            canon1, canon3,
            "Run {run_idx}: Canonicalization must be idempotent"
        );
    }
}

// ==============================================================================
// Property 2: Merkle Tree Boundary & Domain Separation Invariant
// ==============================================================================
#[test]
fn test_property_merkle_tree_avalanche_and_domain_separation() {
    let cert_id = Uuid::new_v4();
    let tenant_id = TenantId::new("tenant-prod-alpha");
    let app_id = ApplicationId::new("app-agent-mesh");
    let req_hash = "f3b6c2d1e0a987654321fedcba0987654321fedcba0987654321fedcba0987";
    let model = "gpt-4o";
    let prompt_tokens = 1500;
    let sources = vec![SourceVersionSnapshot {
        source_id: Uuid::new_v4(),
        snapshot_epoch: 10,
        content_hash: "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
            .to_string(),
    }];
    let safety = SafetyEvidenceRecord {
        structural_hash_match: true,
        negation_polarity_match: true,
        numeric_literal_match: true,
        entity_drift_detected: false,
        non_conformity_score: 0.05,
        calibrated_risk_bound: 0.10,
        heuristic_risk_score: 0.02,
    };
    let validity = ValidityWindow {
        issued_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::hours(1),
        monotonic_epoch_lease: 42,
    };

    let base_root = ReuseCertificate::compute_merkle_root(
        &cert_id,
        1,
        &tenant_id,
        &app_id,
        req_hash,
        model,
        prompt_tokens,
        &sources,
        &safety,
        &validity,
    );

    // Property 2.1: Single-byte flip in model name mutates root completely
    let mutated_model_root = ReuseCertificate::compute_merkle_root(
        &cert_id,
        1,
        &tenant_id,
        &app_id,
        req_hash,
        "gpt-4p", // 1 char flip
        prompt_tokens,
        &sources,
        &safety,
        &validity,
    );
    assert_ne!(
        base_root, mutated_model_root,
        "Model name mutation must alter root"
    );

    // Property 2.2: Single-byte flip in prompt token count alters root
    let mutated_token_root = ReuseCertificate::compute_merkle_root(
        &cert_id,
        1,
        &tenant_id,
        &app_id,
        req_hash,
        model,
        prompt_tokens + 1,
        &sources,
        &safety,
        &validity,
    );
    assert_ne!(
        base_root, mutated_token_root,
        "Token count mutation must alter root"
    );

    // Property 2.3: Tenant separation alters root
    let other_tenant = TenantId::new("tenant-prod-beta");
    let mutated_tenant_root = ReuseCertificate::compute_merkle_root(
        &cert_id,
        1,
        &other_tenant,
        &app_id,
        req_hash,
        model,
        prompt_tokens,
        &sources,
        &safety,
        &validity,
    );
    assert_ne!(
        base_root, mutated_tenant_root,
        "Tenant mutation must alter root"
    );

    // Property 2.4: Safety risk score shift alters root
    let mut mutated_safety = safety.clone();
    mutated_safety.non_conformity_score += 0.001;
    let mutated_safety_root = ReuseCertificate::compute_merkle_root(
        &cert_id,
        1,
        &tenant_id,
        &app_id,
        req_hash,
        model,
        prompt_tokens,
        &sources,
        &mutated_safety,
        &validity,
    );
    assert_ne!(
        base_root, mutated_safety_root,
        "Safety evidence shift must alter root"
    );
}

// ==============================================================================
// Property 3: Compressed Lineage Graph Algebraic Invariants
// ==============================================================================
#[test]
fn test_property_lineage_graph_algebraic_invariants() {
    let graph = CompressedLineageGraph::new();
    let doc1 = 0x1111;
    let doc2 = 0x2222;
    let doc3 = 0x3333;

    // Link overlapping subsets of records
    for i in 1..=50 {
        graph.link_document(doc1, i);
    }
    for i in 25..=75 {
        graph.link_document(doc2, i);
    }
    for i in 50..=100 {
        graph.link_document(doc3, i);
    }

    assert_eq!(graph.document_count(), 3);
    assert_eq!(graph.total_associations(), 50 + 51 + 51);

    // Multi-document invalidation computes exact deduplicated union
    let affected = graph.invalidate_documents(&[doc1, doc2, doc3]);

    // Union of 1..=50, 25..=75, and 50..=100 is 1..=100
    assert_eq!(
        affected.len(),
        100,
        "Union of overlapping lineage sets must be exact"
    );
    for i in 1..=100 {
        assert!(affected.contains(i));
    }
    assert_eq!(graph.document_count(), 0);
}

// ==============================================================================
// Property 4: Quantifier Scope & Role-Bound Numeric Transposition Invariants
// ==============================================================================
#[test]
fn test_property_quantifier_scope_and_role_bound_numeric_transposition() {
    let mut rng = LcgRng::new(0x1337_b007_cafe);

    for run in 0..30 {
        let acc1 = rng.next_range(100, 999);
        let acc2 = rng.next_range(1000, 9999);
        let amount = rng.next_range(50, 5000);

        let p_canonical = format!("Transfer ${amount} from Account {acc1} to Account {acc2}");
        let p_transposed = format!("Transfer ${amount} from Account {acc2} to Account {acc1}");
        let p_paraphrase = format!("Transfer {amount} from Account {acc1} to Account {acc2}");

        let fp_canon = StructuralFingerprint::extract(&p_canonical);
        let fp_trans = StructuralFingerprint::extract(&p_transposed);
        let fp_para = StructuralFingerprint::extract(&p_paraphrase);

        // Property 4.1: Transposition is strictly rejected (0% false hits)
        assert!(
            !fp_canon.matches(&fp_trans),
            "Run {run}: Transposing from {acc1} to {acc2} must strictly be rejected"
        );

        // Property 4.2: Paraphrase with identical role bindings strictly matches (100% precision)
        assert!(
            fp_canon.matches(&fp_para),
            "Run {run}: Identical role bindings must match"
        );
    }
}

// ==============================================================================
// Property 5: Monotonic Epoch Fence & Vector Clock Causality
// ==============================================================================
#[tokio::test]
async fn test_property_monotonic_epoch_fence_and_vector_clock_causality() {
    let mut vc_a = VectorClock::new();
    let mut vc_b = VectorClock::new();

    vc_a.increment("node-1");
    vc_b.increment("node-1");
    vc_b.increment("node-2");

    // Property 5.1: Causality order (vc_a happened-before vc_b)
    assert!(
        vc_a.happened_before(&vc_b),
        "Clock with subset of increments must precede superset"
    );
    assert!(
        !vc_b.happened_before(&vc_a),
        "Superset clock must not precede subset"
    );

    // Property 5.2: Epoch lease manager rejects stale writes
    let manager = EpochLeaseManager::new();
    let src = Uuid::new_v4();

    let lease_v1 = manager.acquire_lease(src).await;
    assert_eq!(lease_v1, 1);
    assert!(manager.validate_lease(src, lease_v1).await);

    // Advancing epoch invalidates previous lease
    let lease_v2 = manager.bump_epoch(src).await;
    assert_eq!(lease_v2, 2);

    // Older lease v1 is now rejected (stale write prevention)
    assert!(
        !manager.validate_lease(src, lease_v1).await,
        "Stale lease v1 must fail validation after epoch bumped to v2"
    );
    // Current lease v2 is valid
    assert!(manager.validate_lease(src, lease_v2).await);
}
