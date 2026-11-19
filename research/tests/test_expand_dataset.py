#!/usr/bin/env python3
"""
Unit tests for Sunborn Conformal Dataset Expansion & Clopper-Pearson Calibration Utility.
"""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "calibration")))
from expand_dataset import (
    binomial_cdf,
    clopper_pearson_upper_bound,
    required_samples_for_target_bound,
    load_dataset,
    validate_dataset,
    generate_expanded_dataset,
    save_dataset,
    DEFAULT_DATASET_PATH,
)


class TestExpandDataset(unittest.TestCase):
    def test_clopper_pearson_exact_zero_failures(self):
        # N=200, 0 failures, 99.9% confidence
        bound = clopper_pearson_upper_bound(0, 200, 0.999)
        self.assertAlmostEqual(bound * 100.0, 3.3949, places=3)

    def test_clopper_pearson_edge_cases(self):
        # Total <= 0
        self.assertEqual(clopper_pearson_upper_bound(0, 0), 1.0)
        self.assertEqual(clopper_pearson_upper_bound(0, -10), 1.0)
        # Failures >= total
        self.assertEqual(clopper_pearson_upper_bound(5, 5), 1.0)
        self.assertEqual(clopper_pearson_upper_bound(10, 5), 1.0)

    def test_clopper_pearson_invalid_inputs(self):
        with self.assertRaises(ValueError):
            clopper_pearson_upper_bound(-1, 100)
        with self.assertRaises(ValueError):
            clopper_pearson_upper_bound(0, 100, confidence=0.0)
        with self.assertRaises(ValueError):
            clopper_pearson_upper_bound(0, 100, confidence=1.0)

    def test_clopper_pearson_large_n_no_overflow(self):
        # Ensure logsumexp prevents float overflow when n=5,000, k=50
        bound = clopper_pearson_upper_bound(50, 5000, 0.999)
        self.assertTrue(0.0 < bound < 1.0)

    def test_required_samples_exact_formula(self):
        # 0.1% target bound at 99.9% confidence requires 6,905 hard negatives
        self.assertEqual(required_samples_for_target_bound(0.001, 0.999), 6905)
        # 1.0% target bound at 99.9% confidence requires 688 hard negatives
        self.assertEqual(required_samples_for_target_bound(0.010, 0.999), 688)

    def test_required_samples_invalid_inputs(self):
        with self.assertRaises(ValueError):
            required_samples_for_target_bound(0.0, 0.999)
        with self.assertRaises(ValueError):
            required_samples_for_target_bound(1.0, 0.999)
        with self.assertRaises(ValueError):
            required_samples_for_target_bound(0.001, 0.0)
        with self.assertRaises(ValueError):
            required_samples_for_target_bound(0.001, 1.0)

    def test_load_and_validate_default_dataset(self):
        self.assertTrue(os.path.isfile(DEFAULT_DATASET_PATH))
        data = load_dataset(DEFAULT_DATASET_PATH)
        val = validate_dataset(data)
        self.assertTrue(val["is_valid"], f"Validation failed with errors: {val.get('errors')}")
        self.assertEqual(val["total_samples"], 320)
        self.assertEqual(val["hard_negatives"], 200)

    def test_synthetic_expansion_preserves_invariants(self):
        base_data = load_dataset(DEFAULT_DATASET_PATH)
        expanded = generate_expanded_dataset(base_data, target_samples=500, seed=123)
        self.assertEqual(len(expanded["evaluation_strata"]), 500)
        val = validate_dataset(expanded)
        self.assertTrue(val["is_valid"], f"Expanded validation failed: {val.get('errors')}")
        self.assertEqual(val["total_samples"], 500)


if __name__ == "__main__":
    unittest.main()
