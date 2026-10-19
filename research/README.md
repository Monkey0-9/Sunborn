# Sunborn Research Package & Artifact Evaluation Guide

> **Research Preprint**: *Sunborn: Deterministic Equivalence, Multi-Tier Semantic Caching, and Partial Execution Reuse for Agentic RAG Systems*.

---

## 1. Artifact Contents
* `paper/sunborn_ieee.tex`: Source LaTeX preprint manuscript and empirical evaluation.
* `reproduce.sh`: Automated 5-gate end-to-end reproducibility script reproducing all empirical tables, latency metrics, and cryptographic attestation certificates.
* `hypothesis_register.md`: Scientific hypothesis registry tracking controlled ablations against Baselines B0–B6.
* `results/benchmark_run.json`: Empirical results for SemanticCacheBench (124 queries across 8 categories).
* `results/run_manifest.json`: Immutable execution manifest for SemanticCacheBench-T (19 stateful temporal mutation scenarios).
* `results/reproduction_certificate.json`: HMAC-SHA-256 signed cryptographic attestation certificate verifying benchmark provenance.
* `../benchmarks/semantic_cache_bench.py`: SemanticCacheBench dataset runner.
* `../benchmarks/temporal_bench.py`: SemanticCacheBench-T stateful temporal mutation runner.
* `../benchmarks/capacity_soak.py`: 3-stage continuous capacity and memory stability simulator.

---

## 2. Reproduction Instructions
To execute the one-command reproduction suite:
```bash
./research/reproduce.sh
```

Expected Outcome:
* **[1/5] Compilation & Linter Hygiene**: 100% clean build, zero compiler warnings, zero Clippy warnings (`-D warnings`).
* **[2/5] Core Invariants & Unit Verification**: 100% pass across all 9 workspace crates.
* **[3/5] Integration & Lifecycle Contract**: All multi-tier lifecycle endpoints and DAG reuse flows verified.
---

## 3. Scientific Evidence Bounds & Production Telemetry Demarcation
To preserve empirical integrity and prevent confabulation of offline research artifacts with runtime production telemetry:

1. **Frozen Dataset vs. Live Production**:
   - `SemanticCacheBench` ($N = 124$ queries across 8 semantic categories, SHA-256 fingerprint: `61a0794c51d312d441bbeaf5ea8e3b9d1958186307606fa177093cc90bce534b`) and `SemanticCacheBench-T` ($N = 19$ stateful mutation scenarios) represent **frozen experimental evaluation suites**.
   - The observed **0.0% false-hit rate** demonstrates the mathematical properties of Sunborn's calibrated Verification Cascade on these frozen suites with a two-sided 95% Wilson confidence interval of $[0.0\%, 3.0\%]$ ($[0.0\%, 16.82\%]$ for temporal-T).
   - These offline benchmarks do **NOT** claim an uncalibrated zero false-hit rate on arbitrary, unseen production traffic without runtime conformal calibration.

2. **Synthetic Cost Model Demarcation**:
   - The reported token cost savings ($23.38\%$ in SemanticCacheBench, $10.52\%$ in SemanticCacheBench-T) are computed using an **explicit synthetic cost estimation model** assuming standard GPT-4o pricing schedule ($0.00625 USD per origin call equivalent, $0.000001 USD per cache lookup).
   - Modeled cost reductions are algorithmic estimates under this pricing baseline; they must **never** be presented as audited commercial provider invoices or actual customer expenditure reductions.

3. **Live Production Telemetry**:
   - In production deployments, real-time cache hit ratios, token avoided counters, and P99 latency are gathered dynamically from active gateway ingress nodes via `/v1/finops/report` and Prometheus scrapers.
   - When live telemetry is unmeasured or Prometheus exporters are unattached, the UI and API truthfully report `Not measured` rather than displaying static demonstration values.

