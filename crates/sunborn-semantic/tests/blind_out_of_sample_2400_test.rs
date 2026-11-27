use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sunborn_semantic::StructuralFingerprint;

#[derive(Debug, Deserialize)]
struct BlindDataset {
    total_pairs: usize,
    equivalent_pairs: usize,
    hard_negative_pairs: usize,
    pairs: Vec<BlindPair>,
}

#[derive(Debug, Deserialize)]
struct BlindPair {
    id: String,
    category: String,
    expected_equivalent: bool,
    query_a: String,
    query_b: String,
}

#[derive(Debug, Serialize)]
struct BlindEvalSummary {
    total_pairs: usize,
    tp: usize,
    fp: usize,
    tn: usize,
    fn_count: usize,
    precision: f64,
    recall: f64,
    fpr_observed: f64,
    clopper_pearson_999_upper_bound: f64,
    false_positives: Vec<String>,
}

/// Exact Clopper-Pearson upper confidence bound at confidence level (1 - alpha).
/// For fp == 0 and n negatives, the exact upper bound is 1 - alpha^(1/n).
fn clopper_pearson_upper_bound(fp: usize, n: usize, alpha: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if fp == 0 {
        return 1.0 - alpha.powf(1.0 / (n as f64));
    }
    if fp >= n {
        return 1.0;
    }
    // Conservative bisection over binomial tail sum P(X <= fp | n, p) = alpha
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let mut cdf = 0.0_f64;
        let mut term = (1.0 - mid).powi(n as i32);
        cdf += term;
        for k in 1..=fp {
            term *= ((n - k + 1) as f64 / (k as f64)) * (mid / (1.0 - mid));
            cdf += term;
        }
        if cdf > alpha {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

#[test]
fn evaluate_frozen_guardrails_on_blind_2400_out_of_sample_corpus() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dataset_path = manifest_dir
        .join("..")
        .join("..")
        .join("research")
        .join("calibration")
        .join("blind_out_of_sample_2400.json");
    let raw =
        std::fs::read_to_string(&dataset_path).expect("blind_out_of_sample_2400.json must exist");
    let dataset: BlindDataset = serde_json::from_str(&raw).expect("valid blind dataset json");

    assert_eq!(dataset.total_pairs, 2400);
    assert_eq!(dataset.equivalent_pairs, 400);
    assert_eq!(dataset.hard_negative_pairs, 2000);

    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut tn = 0usize;
    let mut fn_count = 0usize;
    let mut false_positives = Vec::new();

    for pair in &dataset.pairs {
        let fp_a = StructuralFingerprint::extract(&pair.query_a);
        let fp_b = StructuralFingerprint::extract(&pair.query_b);
        let accepted = fp_a.matches(&fp_b);
        match (pair.expected_equivalent, accepted) {
            (true, true) => tp += 1,
            (true, false) => fn_count += 1,
            (false, true) => {
                fp += 1;
                false_positives.push(format!("{}:{}", pair.category, pair.id));
            }
            (false, false) => tn += 1,
        }
    }

    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        0.0
    };
    let recall = if tp + fn_count > 0 {
        tp as f64 / (tp + fn_count) as f64
    } else {
        0.0
    };
    let n_neg = fp + tn;
    let fpr_observed = fp as f64 / n_neg as f64;
    let cp_999 = clopper_pearson_upper_bound(fp, n_neg, 0.001);

    let summary = BlindEvalSummary {
        total_pairs: dataset.total_pairs,
        tp,
        fp,
        tn,
        fn_count,
        precision,
        recall,
        fpr_observed,
        clopper_pearson_999_upper_bound: cp_999,
        false_positives: false_positives.clone(),
    };

    if std::env::var("SUNBORN_WRITE_EVIDENCE").as_deref() == Ok("1") {
        let out_path = manifest_dir
            .join("..")
            .join("..")
            .join("research")
            .join("results")
            .join("blind_2400_eval_run.json");
        std::fs::write(&out_path, serde_json::to_string_pretty(&summary).unwrap())
            .expect("write blind_2400_eval_run.json");
    }

    println!(
        "Blind N=2400 Eval: TP={tp}, FP={fp}, TN={tn}, FN={fn_count}, Precision={precision:.4}, Recall={recall:.4}, CP-99.9% upper={cp_999:.6}"
    );
    assert_eq!(
        fp, 0,
        "Frozen structural guardrails must reject all 2000 blind adversarial negatives; leaked: {:?}",
        false_positives
    );
    assert!(recall >= 0.70, "Expected recall >= 0.70, got {recall:.4}");
}
