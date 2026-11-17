#!/usr/bin/env python3
"""
Sunborn 320-Sample Held-Out Benchmark & Conformal Risk Evaluation
================================================================
Statistical analysis of the held-out semantic guardrail evaluation.

The confusion matrix is NOT typed in here. It is read from
`research/results/heldout_confusion_matrix.json`, which is produced and
asserted by the Rust evaluator
(`guardrails::tests::test_held_out_evaluation_dataset_precision_and_recall`).
If the checked-in matrix and the guardrail disagree, that Rust test fails, so
these statistics can never silently describe a different code version.

Calculates:
- Precision and recall (Wilson score 95% intervals)
- Empirical false-hit rate (Wilson 95% interval)
- Clopper-Pearson exact 99.9% upper bound on the false-hit rate
- Whether that upper bound supports the configured alpha

Reporting an `alpha_satisfied` flag from a point estimate of zero would be
unsound: a zero observed false-hit rate is compatible with a false-hit rate
well above alpha at small sample sizes. The flag is only true when the exact
99.9% upper confidence bound is itself at or below alpha.
"""

import json
import math
import os
import sys
from typing import Tuple

MATRIX_PATH = os.path.join("research", "results", "heldout_confusion_matrix.json")
DATASET_PATH = os.path.join("research", "calibration", "held_out_eval_dataset.json")
OUT_PATH = os.path.join("research", "results", "heldout_generalization_run.json")


def wilson_interval(successes: int, total: int, z: float = 1.96) -> Tuple[float, float]:
    if total <= 0:
        return (0.0, 0.0)
    p_hat = successes / total
    denom = 1.0 + (z**2) / total
    centre = (p_hat + (z**2) / (2 * total)) / denom
    margin = (z / denom) * math.sqrt(
        max(0.0, (p_hat * (1.0 - p_hat) / total) + (z**2) / (4 * (total**2)))
    )
    return (
        max(0.0, round((centre - margin) * 100.0, 2)),
        min(100.0, round((centre + margin) * 100.0, 2)),
    )


def _binomial_cdf(k: int, n: int, p: float) -> float:
    """P(X <= k) for X ~ Binomial(n, p), evaluated directly."""
    if p <= 0.0:
        return 1.0
    if p >= 1.0:
        return 0.0
    total = 0.0
    for i in range(0, k + 1):
        total += math.comb(n, i) * (p**i) * ((1.0 - p) ** (n - i))
    return total


def clopper_pearson_upper(k: int, n: int, confidence: float = 0.999) -> float:
    """Exact Clopper-Pearson upper binomial bound, in percent.

    Solves P(X <= k; n, p) = 1 - confidence for p by bisection, which is exact
    for every k (not only k = 0).
    """
    if n <= 0:
        return 0.0
    alpha = 1.0 - confidence
    if k >= n:
        return 100.0
    # Find the largest p whose CDF is still at or below alpha.
    lo, hi = 0.0, 1.0
    for _ in range(200):
        mid = (lo + hi) / 2.0
        if _binomial_cdf(k, n, mid) > alpha:
            lo = mid
        else:
            hi = mid
    return round(hi * 100.0, 4)


def main() -> int:
    with open(MATRIX_PATH, "r", encoding="utf-8") as f:
        matrix = json.load(f)
    with open(DATASET_PATH, "r", encoding="utf-8") as f:
        dataset = json.load(f)

    meta = dataset["dataset_metadata"]
    total = len(dataset["evaluation_strata"])

    tp = int(matrix["true_positives"])
    fp = int(matrix["false_positives"])
    tn = int(matrix["true_negatives"])
    fn = int(matrix["false_negatives"])

    if tp + fp + tn + fn != total:
        print(
            f"ERROR: confusion matrix totals {tp + fp + tn + fn} but the dataset has {total} pairs",
            file=sys.stderr,
        )
        return 1

    alpha = 0.001  # configured conformal risk target
    equivalent_total = tp + fn
    hard_neg_total = tn + fp

    precision = (tp / (tp + fp)) * 100.0 if (tp + fp) > 0 else 0.0
    recall = (tp / equivalent_total) * 100.0 if equivalent_total > 0 else 0.0
    false_positive_rate = (fp / hard_neg_total) * 100.0 if hard_neg_total > 0 else 0.0
    specificity = (tn / hard_neg_total) * 100.0 if hard_neg_total > 0 else 0.0

    prec_wilson_95 = wilson_interval(tp, tp + fp, 1.96)
    rec_wilson_95 = wilson_interval(tp, tp + fn, 1.96)
    fpr_wilson_95 = wilson_interval(fp, hard_neg_total, 1.96)
    clopper_upper_999 = clopper_pearson_upper(fp, hard_neg_total, 0.999)
    alpha_pct = alpha * 100.0

    print("=" * 80)
    print(f"SUNBORN SCIENTIFIC VALIDATION: HELD-OUT GENERALIZATION BENCHMARK (N = {total})")
    print(f"Version: {meta.get('version')} | Source matrix: {matrix.get('evaluator')}")
    print("=" * 80)
    print(f"Ground Truth Equivalent : {equivalent_total}")
    print(f"Hard Negative Traps     : {hard_neg_total}")
    print("-" * 80)
    print(f"True Positives (TP)     : {tp}")
    print(f"True Negatives (TN)     : {tn}")
    print(f"False Positives (FP)    : {fp}")
    print(f"False Negatives (FN)    : {fn} (conservative abstentions routed to origin)")
    print("-" * 80)
    print(f"Safety Precision        : {precision:.2f}% [95% CI: {prec_wilson_95[0]}%, {prec_wilson_95[1]}%]")
    print(f"Guardrail Specificity   : {specificity:.2f}%")
    print(f"Conservative Recall     : {recall:.2f}% [95% CI: {rec_wilson_95[0]}%, {rec_wilson_95[1]}%]")
    print(f"Empirical False-Hit Rate: {false_positive_rate:.4f}% [95% CI: {fpr_wilson_95[0]}, {fpr_wilson_95[1]}]%")
    print(f"Clopper-Pearson 99.9% Upper Bound on False-Hit Rate: {clopper_upper_999}%")
    print(f"Configured alpha        : {alpha_pct}%")
    print(
        "Alpha supported by 99.9% upper bound: "
        f"{'YES' if clopper_upper_999 <= alpha_pct else 'NO'}"
    )
    if clopper_upper_999 > alpha_pct:
        print(
            "NOTE: a zero observed false-hit rate is not evidence that the true\n"
            "      rate is <= alpha. More held-out negatives are required before\n"
            "      an alpha-level guarantee can be asserted."
        )
    print("=" * 80)

    report = {
        "evaluation_name": "Sunborn Pre-Registered Held-Out Benchmark",
        "sample_size": total,
        "source_confusion_matrix": MATRIX_PATH,
        "metrics": {
            "true_positives": tp,
            "false_positives": fp,
            "true_negatives": tn,
            "false_negatives": fn,
            "precision_pct": precision,
            "precision_wilson_95_ci": list(prec_wilson_95),
            "recall_pct": recall,
            "recall_wilson_95_ci": list(rec_wilson_95),
            "false_positive_rate_pct": false_positive_rate,
            "false_positive_rate_wilson_95_ci": list(fpr_wilson_95),
            "clopper_pearson_999_upper_bound_pct": clopper_upper_999,
            "conformal_risk_alpha_bound": alpha,
            "alpha_supported_by_999_upper_bound": clopper_upper_999 <= alpha_pct,
        },
        "strata_breakdown": meta.get("strata_breakdown", {}),
    }

    with open(OUT_PATH, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"Report exported to {OUT_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
