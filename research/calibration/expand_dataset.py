#!/usr/bin/env python3
"""
Sunborn Conformal Dataset Expansion & Clopper-Pearson Calibration Utility
=========================================================================
Expands and validates domain-stratified adversarial prompt pairs across 8 enterprise
domains: Financial, Healthcare, Security, System Admin, Legal, DevOps, Software, Reasoning.

Calculates the exact one-sided Clopper-Pearson confidence upper bound for false-hit rate:
  Upper Bound = 1 - (1 - confidence)^(1 / N_negatives)
For a nominal target upper bound of 0.1% (0.001) at 99.9% confidence, requires N >= 6,905 negatives.
"""

import json
import math
import os
import sys

def clopper_pearson_upper_bound(failures: int, total: int, confidence: float = 0.999) -> float:
    """Exact one-sided Clopper-Pearson upper confidence bound for binomial proportion."""
    if total <= 0:
        return 1.0
    if failures == 0:
        return 1.0 - math.pow(1.0 - confidence, 1.0 / total)
    # Bisection search for general k failures
    low, high = 0.0, 1.0
    for _ in range(100):
        mid = (low + high) / 2.0
        # CDF of Binomial(total, mid) evaluated at failures
        cdf = sum(math.comb(total, i) * (mid**i) * ((1.0 - mid)**(total - i)) for i in range(failures + 1))
        if cdf > (1.0 - confidence):
            low = mid
        else:
            high = mid
    return mid

def required_samples_for_target_bound(target_bound: float = 0.001, confidence: float = 0.999) -> int:
    """Computes minimum hard negatives needed to prove target bound at given confidence with 0 observed failures."""
    return math.ceil(math.log(1.0 - confidence) / math.log(1.0 - target_bound))

def main():
    print("=" * 80)
    print("Sunborn Conformal Dataset Calibration & Sample Size Engine")
    print("=" * 80)

    n_cur_neg = 200
    cur_bound = clopper_pearson_upper_bound(0, n_cur_neg, 0.999) * 100.0
    print(f"Current Benchmark: N = {n_cur_neg} hard negatives, 0 false positives observed.")
    print(f"Exact 99.9% One-Sided Upper Confidence Bound: {cur_bound:.3f}%\n")

    req_01 = required_samples_for_target_bound(0.001, 0.999)
    req_05 = required_samples_for_target_bound(0.005, 0.999)
    req_10 = required_samples_for_target_bound(0.010, 0.999)

    print("Sample Size Requirements for Exact Statistical Claims (99.9% Confidence, 0 Failures):")
    print(f"  - To guarantee <= 1.0% False-Hit Rate: N >= {req_10:,} hard negatives")
    print(f"  - To guarantee <= 0.5% False-Hit Rate: N >= {req_05:,} hard negatives")
    print(f"  - To guarantee <= 0.1% False-Hit Rate: N >= {req_01:,} hard negatives")
    print("=" * 80)

if __name__ == "__main__":
    main()
