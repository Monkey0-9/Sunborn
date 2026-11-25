//! Reproducible benchmark measuring embedding separation quality on the
//! pre-registered 320-pair evaluation dataset (`held_out_eval_dataset.json`).
//!
//! Explicitly quantifies:
//! 1. Cosine similarity distribution of `DeterministicEmbeddingProvider` (LSH random projection)
//!    across EQUIVALENT paraphrases vs HARD_NEGATIVE traps.
//! 2. Why embeddings alone (without structural guardrails) suffer false positives on negation,
//!    numeric, entity, and temporal inversions, and what recall/precision LSH achieves with
//!    `StructuralFingerprint` enforcement.

use sunborn_semantic::{
    cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, StructuralFingerprint,
};

#[tokio::test]
async fn benchmark_deterministic_embedding_separation_and_guardrail_precision() {
    let manifest_str = include_str!("../../../research/calibration/held_out_eval_dataset.json");
    let v: serde_json::Value = serde_json::from_str(manifest_str).expect("Valid evaluation JSON");
    let pairs = v["evaluation_strata"].as_array().expect("Array of strata");
    assert_eq!(pairs.len(), 320);

    let embedder = DeterministicEmbeddingProvider::new();

    let mut eq_sims = Vec::new();
    let mut neg_sims = Vec::new();
    let mut lsh_only_fp_at_85 = 0usize;
    let mut lsh_only_tp_at_85 = 0usize;
    let mut guarded_fp_at_75 = 0usize;
    let mut guarded_tp_at_75 = 0usize;

    for item in pairs {
        let p_a = item["prompt_a"].as_str().unwrap();
        let p_b = item["prompt_b"].as_str().unwrap();
        let label = item["label"].as_str().unwrap();

        let va = embedder.embed(p_a).await.unwrap();
        let vb = embedder.embed(p_b).await.unwrap();
        let sim = cosine_similarity(&va, &vb);

        let fpa = StructuralFingerprint::extract(p_a);
        let fpb = StructuralFingerprint::extract(p_b);
        let structural_ok = fpa.matches(&fpb);

        match label {
            "EQUIVALENT" => {
                eq_sims.push(sim);
                if sim >= 0.85 {
                    lsh_only_tp_at_85 += 1;
                }
                if sim >= 0.75 && structural_ok {
                    guarded_tp_at_75 += 1;
                }
            }
            "HARD_NEGATIVE" => {
                neg_sims.push(sim);
                if sim >= 0.85 {
                    lsh_only_fp_at_85 += 1;
                }
                if sim >= 0.75 && structural_ok {
                    guarded_fp_at_75 += 1;
                }
            }
            _ => {}
        }
    }

    let eq_mean = eq_sims.iter().sum::<f32>() / eq_sims.len() as f32;
    let neg_mean = neg_sims.iter().sum::<f32>() / neg_sims.len() as f32;

    println!(
        "Deterministic LSH Embedding Quality Benchmark (N=320):\n\
         - EQUIVALENT mean cosine similarity:    {eq_mean:.4} (n={})\n\
         - HARD_NEGATIVE mean cosine similarity: {neg_mean:.4} (n={})\n\
         - LSH-only (>=0.85) TP={lsh_only_tp_at_85}, FP={lsh_only_fp_at_85}\n\
         - LSH + Structural (>=0.75) TP={guarded_tp_at_75}, FP={guarded_fp_at_75}",
        eq_sims.len(),
        neg_sims.len()
    );

    // LSH-only MUST exhibit false positives on hard negatives (proving why structural guardrails
    // are mandatory and why LSH alone is insufficient for semantic safety).
    assert!(
        lsh_only_fp_at_85 > 0,
        "Expected LSH-only similarity >= 0.85 to have false positives on near-identical hard negatives"
    );

    // Combined with StructuralFingerprint, zero hard-negative false positives occur!
    assert_eq!(
        guarded_fp_at_75, 0,
        "LSH + StructuralFingerprint must achieve 0 false positives on all 200 hard negatives"
    );
    assert!(
        guarded_tp_at_75 > 0,
        "LSH + StructuralFingerprint must retain positive recall on equivalent pairs"
    );
}
