# Sunborn Research Package & Artifact Evaluation Guide

> **Research Preprint**: *Sunborn: Deterministic Equivalence, Multi-Tier Semantic Caching, and Partial Execution Reuse for Agentic RAG Systems*.

---

## 1. Artifact Contents
* `paper/sunborn_ieee.tex`: Source LaTeX preprint manuscript and empirical evaluation.
* `reproduce.sh`: Automated 5-gate end-to-end reproducibility script reproducing all empirical tables, latency metrics, live gateway RPC traces, and cryptographic attestation certificates.
* `hypothesis_register.md`: Scientific hypothesis registry tracking controlled ablations against Baselines B0–B6.
* `results/benchmark_run.json`: Empirical results for SemanticCacheBench (124 queries across 8 categories, executed via live gateway RPC).
* `results/run_manifest.json`: Immutable execution manifest for SemanticCacheBench-T (19 stateful temporal mutation scenarios).
* `results/reproduction_certificate.json`: Local integrity record. The checked-in certificate is explicitly unauthenticated and tied to its recorded repository revision.
* `verify_certificate.py`: Independent certificate and artifact provenance verifier with explicit 4-state outcome semantics.
* `tests/test_verifier_negative.py`: Negative verification test suite proving strict rejection of offline artifacts, corrupted traces, and mismatched keys.
* `../benchmarks/semantic_cache_bench.py`: SemanticCacheBench dataset runner (uses a lightweight embedding backend by default; `--use-fastembed` explicitly opts into loading FastEmbed; supports strict live gateway RPC via `--require-live` and developer offline simulation via `--allow-offline`).
* `../benchmarks/temporal_bench.py`: SemanticCacheBench-T stateful temporal mutation runner.
* `../benchmarks/capacity_soak.py`: 3-stage local HTTP capacity and memory harness.

> **Evidence status:** The checked-in research artifacts were generated at commit `6bf5d40`, not the current review candidate. They have not been independently reproduced. Their results are bounded to the recorded local runs; they are not production SLO evidence.

---

## 2. Reproduction Instructions
To execute the one-command reproduction suite:
```bash
./research/reproduce.sh
```

Expected Outcome:
* **[1/5] Compilation & Linter Hygiene**: 100% clean build, zero compiler warnings, zero Clippy warnings (`-D warnings`).
* **[2/5] Core Invariants & Unit Verification**: 100% pass across all 9 workspace crates, Python SDK (`test_sunborn_agent.py`), and Frontend BFF test suites.
* **[3/5] In-Process Engine vs. Cluster Latency Separation**: Run latency checks and report measured results; the checked-in capacity artifact fails the documented P99 target.
* **[4/5] Security, Chaos, Governance & Disaster Recovery**: Run local automated checks. They do not establish zero cross-tenant leakage or production disaster-recovery guarantees without staging and independent review.
* **[5/5] Live Gateway SemanticCacheBench, SemanticCacheBench-T & Cryptographic Verification**:
  - Live Sunborn Gateway daemon started at `http://127.0.0.1:8080` with a strict 5.0s health-check deadline (fails closed if unhealthy).
  - All 124 benchmark queries dispatched over live HTTP RPC to the gateway (`execution_mode: "live_gateway_rpc"`, `provenance_type: "LIVE_GATEWAY_DECISION_PATH"`).
  - Exact query completeness enforced: observed query IDs must match the expected 124-query dataset set (zero missing, unexpected, or duplicate IDs).
  - Canonical dataset contract verification: each evaluation trace's category and ground-truth label are AST-extracted and asserted against the canonical versioned dataset.
  - Meaningful semantic reuse enforced: requires true positive reuses > 0 (prevents degenerate zero-reuse cache pass).
  - Fail-closed Git provenance: any repository revision or tree mismatch immediately returns `INVALID` (explicit `--no-git` flags reduced verification scope).
  - Baseline fields are recomputed from raw traces and cost models. The current checked-in static artifact reports B2 false hits 51/124 (41.13%), not 60/124.
  - Metrics cross-verification: all 10 certificate empirical metrics recomputed directly from raw source artifacts (`benchmark_run.json` and `run_manifest.json`) and verified identical with zero-tolerance exact equality.
  - Negative test suite (`research/tests/test_verifier_negative.py`, 23 tests) executed, proving that offline simulation artifacts, corrupted traces, arbitrary ID replacements, ground-truth label tampering, category tampering, baseline summary mutations, metric mismatches, and tampered Git hashes are strictly rejected.
  - Independent certificate verification (`research/verify_certificate.py`) evaluated under explicit 4-state outcomes (`AUTHENTICATED`, `INTEGRITY_VERIFIED_UNAUTHENTICATED`, `AUTHENTICATION_UNVERIFIED`, or `INVALID`).
---

## 3. Scientific Evidence Bounds & Production Telemetry Demarcation
To preserve empirical integrity and prevent confabulation of offline research artifacts with runtime production telemetry:

1. **Frozen Dataset vs. Live Production**:
   - `SemanticCacheBench` ($N = 124$ queries across 8 semantic categories, SHA-256 fingerprint: `61a0794c51d312d441bbeaf5ea8e3b9d1958186307606fa177093cc90bce534b`) and `SemanticCacheBench-T` ($N = 19$ stateful mutation scenarios) represent **frozen experimental evaluation suites**.
   - The static artifact records 0 B3 false hits on 124 queries, with 15 B3 hits. The temporal artifact records 0 B3 hits and 0 B3 false hits on 19 scenarios, so it provides no evidence of useful temporal reuse. These finite observations do not establish mathematical or production-wide guarantees.
   - These offline benchmarks do **NOT** claim an uncalibrated zero false-hit rate on arbitrary, unseen production traffic without runtime conformal calibration.

2. **Synthetic Cost Model Demarcation**:
   - The static artifact's 12.09% modeled savings and temporal artifact's 0% savings use an **explicit synthetic cost estimation model** ($0.00625 per origin call equivalent, $0.000001 per cache lookup).
   - Modeled cost reductions are algorithmic estimates under this pricing baseline; they must **never** be presented as audited commercial provider invoices or actual customer expenditure reductions.

3. **Live Production Telemetry**:
   - In production deployments, real-time cache hit ratios, token avoided counters, and P99 latency are gathered dynamically from active gateway ingress nodes via `/v1/finops/report` and Prometheus scrapers.
   - When live telemetry is unmeasured or Prometheus exporters are unattached, the UI and API truthfully report `Not measured` rather than displaying static demonstration values.
