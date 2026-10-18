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
* **[4/5] Adversarial Security & Conformal Learning**: Zero cross-tenant leaks, tenant spoofing rejection, circuit-breaker fallback, DST Non-Accelerator Authority, and 5-layer learning controller verified.
* **[5/5] SemanticCacheBench & Empirical Artifacts**:
  - `SemanticCacheBench`: Sunborn B3 achieves **0.0% false hits** (100% precision, 95% Wilson CI $[0.0, 3.0]$) vs. Naive Cosine B2 **48.39% false hits**.
  - `SemanticCacheBench-T`: Sunborn B3 achieves **0.0% false hits** across 19 temporal mutations vs. Naive Cosine B2 **63.16% false hits**.
  - `Capacity Soak`: Sustained **39,400+ req/s** throughput under Zipfian churn ($s = 1.05$), 0 broadcast channel leaks, 42.0 µs TDEK crypto-shredding, flat RSS memory envelope ($\le 1.2\%$ variance).
  - Cryptographically signed `research/results/reproduction_certificate.json` generated and verified.
