//! External-Corpus Five-Gate Ablation (B2–B6).
//!
//! Unlike `five_gate_benchmark_test.rs`, the prompt text here does not come from a
//! hand-authored adversarial workload. It is read verbatim from
//! `research/calibration/external_unseen_eval_dataset.json` (N=160 pairs across
//! fintech, healthcare, legal, developer, multilingual and trap categories), which was
//! authored against frozen `StructuralFingerprint` rules and is never used for tuning.
//!
//! Four probes are evaluated per pair:
//!   1. equivalence probe — prompt B against a cache entry seeded with prompt A.
//!      A hit is safe only when `expected_equivalent` is true.
//!   2. stale probe — identical text, but the authoritative source epoch has advanced
//!      while the certificate still cites epoch 1. Always unsafe.
//!   3. mutating probe — the follow-up carries a mutating execution DAG. Always unsafe.
//!   4. tampered probe — the cached certificate signature is corrupted. Always unsafe.
//!
//! This isolates what each gate removes on external text: G1 (equivalence/guardrails),
//! G3 (freshness), G4 (effects), G5 (signature). G2 is held satisfied by construction
//! (same session and context digest) because this corpus does not model sessions; the
//! cross-session behaviour is covered by the in-tree benchmark.
//!
//! Honest framing: probe types 2–4 are *injected* threats, not natural corpus labels.
//! Only the equivalence probes measure the corpus as authored. The ablation therefore
//! measures gate mechanism coverage on external text, not a population false-hit rate.

use serde_json::{json, Value};
use std::sync::Arc;
use sunborn_core::{
    certificate::{SafetyEvidenceRecord, SourceVersionSnapshot, ValidityWindow},
    ApplicationId, DecisionAction, RequestEnvelope, ReuseCertificate, TenantId,
};
use sunborn_execution::{ExecutionEffect, ExecutionNode, ExecutionPlan, ToolCall};
use sunborn_gateway::five_gates::FiveGateEvaluator;
use sunborn_semantic::{
    cosine_similarity, DecisionConfig, DecisionEngine, DeterministicEmbeddingProvider,
    EmbeddingProvider, MemoryVectorStore, SourceGroundingVerifier, StructuralFingerprint,
    VectorEntry, VectorStore,
};
use uuid::Uuid;

struct Level {
    name: &'static str,
    g1: bool,
    g3: bool,
    g4: bool,
    g5: bool,
}

const LEVELS: [Level; 5] = [
    Level {
        name: "B2 (raw similarity)",
        g1: false,
        g3: false,
        g4: false,
        g5: false,
    },
    Level {
        name: "B3 (G1 only)",
        g1: true,
        g3: false,
        g4: false,
        g5: false,
    },
    Level {
        name: "B4 (G1+G3)",
        g1: true,
        g3: true,
        g4: false,
        g5: false,
    },
    Level {
        name: "B5 (G1+G3+G4)",
        g1: true,
        g3: true,
        g4: true,
        g5: false,
    },
    Level {
        name: "B6 (G1+G3+G4+G5)",
        g1: true,
        g3: true,
        g4: true,
        g5: true,
    },
];

#[derive(Default, Clone, Copy)]
struct Tally {
    safe_hits: usize,
    false_hits: usize,
    misses: usize,
}

fn mutating_plan() -> ExecutionPlan {
    let mut plan = ExecutionPlan::new();
    plan.add_node(
        ExecutionNode::new(
            "pay",
            "Transfer",
            vec![],
            Some(ToolCall {
                tool_name: "wire".to_string(),
                tool_version: "1.0".to_string(),
                arguments: json!({ "amount": 250 }),
                context_hash: None,
            }),
            false,
        )
        .with_effect(ExecutionEffect::MutatingSideEffecting {
            operation_type: "external_action".to_string(),
            idempotency_key: String::new(),
        }),
    );
    plan
}

#[tokio::test]
async fn external_corpus_five_gate_ablation() {
    let raw = include_str!("../../../research/calibration/external_unseen_eval_dataset.json");
    let parsed: Value = serde_json::from_str(raw).expect("valid external dataset");
    let pairs = parsed["pairs"].as_array().expect("pairs array");
    assert_eq!(
        pairs.len(),
        160,
        "external corpus must be the frozen N=160 split"
    );

    let tenant = "tenant-ext";
    let app_id = "app-ext";
    let doc_id = Uuid::new_v4();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
    let verifying_key = signing_key.verifying_key();

    let sec_ctx = sunborn_gateway::middleware::auth::SecurityContext {
        tenant_id: TenantId::new(tenant),
        application_id: ApplicationId::new(app_id),
        api_key_hash: "ext-bench-key-hash".to_string(),
        allowed_models: vec!["*".to_string()],
        is_admin: false,
        is_authenticated: true,
    };

    let mut tallies = vec![Tally::default(); LEVELS.len()];
    let mut equivalence_false_hits = vec![0usize; LEVELS.len()];
    let mut per_category: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();

    for pair in pairs {
        let id = pair["id"].as_str().unwrap_or("?");
        let category = pair["category"].as_str().unwrap_or("unknown");
        let prompt_a = pair["prompt_a"].as_str().expect("prompt_a");
        let prompt_b = pair["prompt_b"].as_str().expect("prompt_b");
        let expected_equivalent = pair["expected_equivalent"].as_bool().expect("label");

        let mut req_a = RequestEnvelope::new(tenant, app_id, "gpt-4o", prompt_a);
        req_a.metadata["session_id"] = json!("sess-ext");
        req_a.dependency_snapshots = vec![SourceVersionSnapshot {
            source_id: doc_id,
            snapshot_epoch: 1,
            content_hash: "sha256:external-v1".to_string(),
        }];

        let mut req_b = req_a.clone();
        req_b.prompt = prompt_b.to_string();

        let context_digest = sunborn_identity::IdentityEngine::compute_context_digest(&req_a);
        let reference_fingerprint =
            serde_json::to_string(&StructuralFingerprint::extract(prompt_a)).unwrap_or_default();

        let embedder = DeterministicEmbeddingProvider::new();
        let vec_a = embedder.embed(prompt_a).await.unwrap();
        let vec_b = embedder.embed(prompt_b).await.unwrap();

        for (level_idx, level) in LEVELS.iter().enumerate() {
            let vstore: Arc<dyn VectorStore> = Arc::new(MemoryVectorStore::new());

            let mut cert = ReuseCertificate::new_signed_ed25519(
                TenantId::new(tenant),
                ApplicationId::new(app_id),
                "ext-cache-key",
                "gpt-4o",
                prompt_a.split_whitespace().count() as u32,
                req_a.dependency_snapshots.clone(),
                SafetyEvidenceRecord {
                    structural_hash_match: true,
                    negation_polarity_match: true,
                    numeric_literal_match: true,
                    entity_drift_detected: false,
                    non_conformity_score: 0.01,
                    calibrated_risk_bound: 0.05,
                    heuristic_risk_score: 0.0,
                },
                ValidityWindow {
                    issued_at: chrono::Utc::now(),
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                    monotonic_epoch_lease: 1,
                },
                &signing_key,
                "k-ext",
            );
            cert.attach_reference_fingerprint_ed25519(reference_fingerprint.clone(), &signing_key);

            // Probe layout: (label, expected safe, authoritative epoch, mutating, tampered)
            let probes: [(&str, bool, u64, bool, bool); 4] = [
                ("equivalence", expected_equivalent, 1, false, false),
                ("stale", false, 2, false, false),
                ("mutating", false, 1, true, false),
                ("tampered", false, 1, false, true),
            ];

            for (probe, is_safe, authoritative_epoch, mutating, tampered) in probes {
                let mut stored_cert = cert.clone();
                if tampered {
                    stored_cert.signature = "tampered-signature-deadbeef".to_string();
                }
                vstore
                    .insert(VectorEntry {
                        id: format!("ext-{id}"),
                        tenant_id: TenantId::new(tenant),
                        application_id: ApplicationId::new(app_id),
                        prompt: prompt_a.to_string(),
                        content: "cached external answer".to_string(),
                        vector: vec_a.clone(),
                        metadata: json!({
                            "session_id": "sess-ext",
                            "context_digest": context_digest,
                            "certificate": stored_cert,
                            "embedding_fingerprint": embedder.embedding_fingerprint(),
                        }),
                        created_at: chrono::Utc::now(),
                    })
                    .await
                    .unwrap();

                // Freshness, effect and signature threats are exact repeats of the cached
                // prompt by construction: otherwise G1 would refuse them for equivalence
                // reasons and the later gates would never be exercised.
                let mut req = if probe == "equivalence" {
                    req_b.clone()
                } else {
                    req_a.clone()
                };
                if mutating {
                    req.metadata["execution_plan"] = serde_json::to_value(mutating_plan()).unwrap();
                }

                let qv = if probe == "equivalence" {
                    &vec_b
                } else {
                    &vec_a
                };
                let hit = if !level.g1 {
                    // Raw similarity only. No guardrails, context, freshness, effects or certs.
                    let res = vstore
                        .search(
                            &TenantId::new(tenant),
                            &ApplicationId::new(app_id),
                            qv,
                            1,
                            0.70,
                        )
                        .await
                        .unwrap();
                    !res.is_empty() && cosine_similarity(qv, &res[0].entry.vector) >= 0.70
                } else {
                    let eval_embedder: Arc<dyn EmbeddingProvider> =
                        Arc::new(DeterministicEmbeddingProvider::new());
                    let engine = DecisionEngine::new(
                        eval_embedder,
                        vstore.clone(),
                        DecisionConfig::default(),
                    );
                    let out = engine
                        .evaluate_with_thresholds(&req, 0.70, 0.25)
                        .await
                        .unwrap();
                    let g1_ok = out.decision.action == DecisionAction::SafeHit;
                    let meta = out.matched_metadata.as_ref();
                    let g2_ok = meta
                        .map(|m| {
                            FiveGateEvaluator::evaluate_g2_context(&req, &sec_ctx, Some(m)).passed
                        })
                        .unwrap_or(false);
                    let g3_ok = if level.g3 {
                        out.matched_certificate
                            .as_ref()
                            .map(|c| {
                                SourceGroundingVerifier::verify_source_snapshots(
                                    &req.dependency_snapshots,
                                    &c.source_snapshots,
                                )
                                .is_ok()
                                    && c.source_snapshots
                                        .iter()
                                        .all(|s| s.snapshot_epoch >= authoritative_epoch)
                            })
                            .unwrap_or(false)
                    } else {
                        true
                    };
                    let g4_ok = if level.g4 {
                        FiveGateEvaluator::evaluate_g4_effects(&req).passed
                    } else {
                        true
                    };
                    let g5_ok = if level.g5 {
                        out.matched_certificate
                            .as_ref()
                            .map(|c| c.verify_signature_ed25519(&verifying_key).is_ok())
                            .unwrap_or(false)
                    } else {
                        true
                    };
                    g1_ok && g2_ok && g3_ok && g4_ok && g5_ok
                };

                let t = &mut tallies[level_idx];
                if hit {
                    if is_safe {
                        t.safe_hits += 1;
                    } else {
                        t.false_hits += 1;
                        if probe == "equivalence" {
                            equivalence_false_hits[level_idx] += 1;
                            per_category.entry(category.to_string()).or_insert((0, 0)).0 += 1;
                        }
                    }
                } else {
                    t.misses += 1;
                }
            }
        }
    }

    // The external corpus is 160 pairs: 80 equivalent and 80 traps by design.
    let equivalence_probes = 160usize;
    let total_probes = equivalence_probes * 4;

    println!(
        "\n=== External-Corpus Five-Gate Ablation (N=160 pairs, {} probes) ===",
        total_probes
    );
    for (idx, level) in LEVELS.iter().enumerate() {
        let t = tallies[idx];
        let eq_fp = equivalence_false_hits[idx];
        println!(
            "{:<26} safe_hits={:>4} false_hits={:>4} misses={:>4} | equivalence-only FP={}/{}",
            level.name, t.safe_hits, t.false_hits, t.misses, eq_fp, equivalence_probes
        );
    }

    // Invariants that make the ablation meaningful rather than merely printed.
    // 1. Guardrails (G1) must strictly reduce external false hits versus raw similarity.
    assert!(
        tallies[1].false_hits < tallies[0].false_hits,
        "G1 guardrails must strictly reduce external false hits ({} -> {})",
        tallies[0].false_hits,
        tallies[1].false_hits
    );
    // 2. Each later gate must strictly remove its injected threat class.
    for idx in 2..LEVELS.len() {
        assert!(
            tallies[idx].false_hits < tallies[idx - 1].false_hits,
            "{} must strictly reduce false hits versus {} ({} -> {})",
            LEVELS[idx].name,
            LEVELS[idx - 1].name,
            tallies[idx - 1].false_hits,
            tallies[idx].false_hits
        );
    }
    // 3. Full contract must remove every injected freshness/effect/signature hit, so the
    //    only residual false hits are G1 equivalence errors on external text.
    assert_eq!(
        tallies[4].false_hits, equivalence_false_hits[4],
        "B6 residual false hits must come only from equivalence probes, not injected threats"
    );

    let residual_fp = equivalence_false_hits[4];
    println!(
        "Residual external equivalence false hits under the full contract: {}/{} ({:.2}%)",
        residual_fp,
        equivalence_probes,
        (residual_fp as f64 / equivalence_probes as f64) * 100.0
    );
    println!(
        "Raw-similarity (B2) external equivalence false-hit categories: {:?}",
        per_category
    );

    if std::env::var("SUNBORN_WRITE_EVIDENCE").as_deref() == Ok("1") {
        let sha = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(raw.as_bytes());
            hex::encode(h.finalize())
        };
        let mut rows = Vec::new();
        for (idx, level) in LEVELS.iter().enumerate() {
            rows.push(json!({
                "level": level.name,
                "safe_hits": tallies[idx].safe_hits,
                "false_hits": tallies[idx].false_hits,
                "misses": tallies[idx].misses,
                "equivalence_only_false_hits": equivalence_false_hits[idx],
            }));
        }
        let artifact = json!({
            "dataset": "research/calibration/external_unseen_eval_dataset.json",
            "dataset_sha256": sha,
            "dataset_pairs": 160,
            "probes_per_pair": 4,
            "certificate_probes_injected": ["stale_epoch", "mutating_plan", "tampered_signature"],
            "levels": rows,
            "residual_equivalence_false_hits": residual_fp,
            "residual_equivalence_false_hit_rate_pct": (residual_fp as f64 / equivalence_probes as f64) * 100.0,
            "raw_similarity_equivalence_false_hit_categories": per_category,
            "scope": "Gate mechanism coverage on external prompt text. Probe types 2-4 are injected threats; only equivalence probes measure the corpus as authored. Not a population false-hit rate and not production evidence.",
        });
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../research/results/external_five_gate_ablation.json");
        std::fs::write(&out, serde_json::to_vec_pretty(&artifact).unwrap()).unwrap();
        println!("Wrote {}", out.display());
    }
}
