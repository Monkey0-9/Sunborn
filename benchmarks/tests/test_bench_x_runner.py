#!/usr/bin/env python3
"""Tests for the SemanticCacheBench-X runner.

Covers the properties that make the benchmark trustworthy: determinism, the
held-out split, category coverage, metric arithmetic, and the fact that a dry
run emits no behavioural metric.
"""

import json
import sys
from pathlib import Path

import pytest

BENCH_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCH_DIR))

import semantic_cache_bench_x as bench  # noqa: E402


def test_dataset_is_deterministic():
    a = bench.generate_dataset(per_category=20)
    b = bench.generate_dataset(per_category=20)
    assert bench.dataset_fingerprint(a) == bench.dataset_fingerprint(b)
    assert a == b


def test_all_ten_categories_present():
    cases = bench.generate_dataset(per_category=10)
    counts = bench.category_counts(cases)
    assert set(counts) == set(bench.CATEGORIES)
    assert len(bench.CATEGORIES) == 10
    assert all(n == 10 for n in counts.values())
    assert len(cases) == 100


def test_splits_are_disjoint_and_cover_all_cases():
    cases = bench.generate_dataset(per_category=50)
    bench.validate_splits(cases)
    counts = bench.split_counts(cases)
    assert sum(counts.values()) == len(cases)
    # Every split must actually be populated at this size.
    assert all(n > 0 for n in counts.values())


def test_split_leak_is_detected():
    leaky = [
        {"id": "a", "category": "paraphrase", "split": "dev", "prompt": "Same question"},
        {"id": "b", "category": "paraphrase", "split": "test", "prompt": "same question"},
    ]
    with pytest.raises(ValueError, match="held-out split leak"):
        bench.validate_splits(leaky)


def test_only_paraphrase_is_labelled_safely_reusable():
    cases = bench.generate_dataset(per_category=10)
    for case in cases:
        expected = case["category"] in bench.SAFE_CATEGORIES
        assert case["expected_reuse"] is expected, case["id"]


def test_cases_carry_required_fields():
    required = {"id", "category", "split", "prompt", "variant", "expected_reuse"}
    for case in bench.generate_dataset(per_category=5):
        assert required <= set(case), case["id"]
        assert case["split"] in bench.SPLIT_BUCKETS


def test_metrics_arithmetic_on_known_records():
    records = [
        {"category": "paraphrase", "expected_reuse": True, "served_reuse": True, "certificate_present": True},
        {"category": "paraphrase", "expected_reuse": True, "served_reuse": False, "certificate_present": False},
        {"category": "numeric", "expected_reuse": False, "served_reuse": False, "certificate_present": False},
        {"category": "temporal", "expected_reuse": False, "served_reuse": True, "certificate_present": True},
    ]
    m = bench.compute_metrics(records)
    # 2 reuses served, 1 of them safe -> 50% precision.
    assert m["reuses_served"] == 2
    assert m["safe_reuse_precision_percent"] == 50.0
    # 1 unsafe reuse out of 4 cases -> 25%.
    assert m["unsafe_reuse_rate_percent"] == 25.0
    # 2 of 4 cases reused -> 50% coverage.
    assert m["reuse_coverage_percent"] == 50.0
    # The one temporal case was reused -> 100% stale reuse.
    assert m["stale_reuse_rate_percent"] == 100.0
    # No authorization cases -> 0, not a division error.
    assert m["authorization_violation_rate_percent"] == 0.0
    # Both served reuses carried evidence -> 100%.
    assert m["evidence_verification_rate_percent"] == 100.0


def test_perfect_precision_with_zero_coverage_is_reported_together():
    records = [
        {"category": "paraphrase", "expected_reuse": True, "served_reuse": False, "certificate_present": False},
        {"category": "numeric", "expected_reuse": False, "served_reuse": False, "certificate_present": False},
    ]
    m = bench.compute_metrics(records)
    # The benchmark must not let a system that never reuses look good.
    assert m["reuse_coverage_percent"] == 0.0
    assert m["reuses_served"] == 0
    assert m["safe_reuse_precision_percent"] == 0.0


def test_compute_metrics_rejects_empty():
    with pytest.raises(ValueError, match="no records"):
        bench.compute_metrics([])


def test_wilson_interval_bounds():
    lo, hi = bench.wilson_interval(0, 100)
    assert lo == 0.0 and hi < 5.0
    lo, hi = bench.wilson_interval(100, 100)
    assert hi == 100.0 and lo > 95.0
    assert bench.wilson_interval(0, 0) == [0.0, 0.0]


def test_dry_run_writes_dataset_but_no_metrics(tmp_path):
    payload = bench.run_dry(bench.generate_dataset(per_category=5), tmp_path)
    artifact = tmp_path / "semantic_cache_bench_x_dataset.json"
    assert artifact.exists()
    written = json.loads(artifact.read_text(encoding="utf-8"))
    assert written["synthetic"] is True
    assert written["total_cases"] == 50
    assert written["fingerprint_sha256"] == payload["fingerprint_sha256"]
    # A dry run must never claim a measured behavioural metric.
    assert "metrics" not in written
    assert "measured" not in written


def test_cli_dry_run_exits_zero(tmp_path, capsys):
    code = bench.main(["--dry-run", "--per-category", "10", "--dataset-dir", str(tmp_path)])
    assert code == 0
    out = capsys.readouterr().out
    assert "not measured" in out
    assert "cases            : 100" in out


def test_cli_live_requires_api_key(tmp_path):
    with pytest.raises(SystemExit):
        bench.main(["--live", "--per-category", "1", "--out-dir", str(tmp_path)])
