# Sunborn Artifact Appendix: Empirical Evaluation & Reproducibility Guide

This artifact appendix conforms to the **ACM Artifact Review and Badging Policy (v1.1)** and **IEEE Reproducibility Standards** for the paper:
> *"Sunborn: Deterministic Equivalence, Multi-Tier Semantic Caching, and Partial Execution Reuse for Agentic RAG Systems"*

---

## 1. Artifact Identification & Badging Targets

* **Artifacts Available**: The complete source code, cryptographic verification harnesses, benchmark datasets, and evaluation scripts are available in the repository.
* **Artifacts Evaluated — Functional**: Automated test suites covering all 9 Rust crates, Next.js frontend BFF security, and Python SDK pass 100% with strict compiler linter gates (`cargo clippy -D warnings`).
* **Results Reproduced**: Running `./research/reproduce.sh` dynamically executes all 5 verification gates, regenerates raw empirical manifests (`benchmark_run.json`, `run_manifest.json`), and cryptographically verifies the resulting signed attestation certificate (`reproduction_certificate.json`) in constant time.

---

## 2. Hardware & Environment Prerequisites

| Specification | Minimum Requirement | Verified Benchmark Host |
| :--- | :--- | :--- |
| **Operating System** | Linux (kernel $\ge 5.15$, e.g., Ubuntu 22.04+, RHEL/Rocky 9+, Fedora 40+) | Linux 6.12 (x86_64) |
| **Processor Architecture** | x86_64 (AVX2 recommended) or aarch64 | x86_64 AMD EPYC / Intel Xeon |
| **Memory (RAM)** | 8 GB minimum (16 GB recommended for full soak test) | 16+ GB RAM |
| **Disk Storage** | 5 GB free disk space | NVMe SSD |
| **Rust Toolchain** | Rust stable ($\ge 1.80$, tested on `rustc 1.99.0`) | `cargo`, `rustc` |
| **Python Runtime** | Python 3.10+ (tested on Python 3.13) with `fastembed` (optional) | Python 3.13 |
| **Node.js Runtime** | Node.js 18+ (tested on Node 20 / 22) for frontend tests | Node.js 20+ |

---

## 3. Step-by-Step Reproduction Instructions

From the root of the repository:

```bash
# 1. Ensure execution permissions
chmod +x ./research/reproduce.sh

# 2. Execute the full empirical reproducibility suite
./research/reproduce.sh
```

### Expected Execution Time:
* **Total Runtime**: 15 to 30 seconds on standard modern CPU.
* **Network Requirement**: Zero external network access required (100% self-contained headless execution).

---

## 4. Verification Gates Breakdown

The automated reproduction script executes five rigorous sequential validation gates:

1. **Gate 1: Code Formatting & Strict Linting**
   * Executes `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
   * Guarantees zero compiler warnings, zero dead code, and standard Rust formatting.

2. **Gate 2: Unit, Integration & SDK Test Matrix**
   * Runs `cargo test --workspace --all-targets` across all 9 crates:
     * `sunborn-core`: Cryptographic Merkle Merkle tree canonicalization, HMAC-SHA256 reuse certificates, FinOps cost avoidance math.
     * `sunborn-identity`: RFC 8785 JSON canonicalization, Unicode NFC normalization, non-deterministic sampling gates, constant-time HMAC verification.
     * `sunborn-cache`: FinOps-Weighted Greedy-Dual-Size-Frequency (GDSF) byte-bounded eviction, circuit-breaker fallback.
     * `sunborn-semantic`: Non-Exchangeable Conformal Risk Calibrators, ContextDigest lineage binding, Locality-Sensitive Hashing (LSH) random projection.
     * `sunborn-execution`: Transitive DAG purity, TTL, and lineage envelope propagation, topological cycle detection.
     * `sunborn-invalidation`: Compressed Roaring Bitmap lineage, vector clocks, and per-lineage watermark generation fencing.
     * `sunborn-gateway`: Handlers, routing, and disaster recovery rebuild.
   * Runs Python SDK test suite: `python3 -m unittest sdk/python/test_sunborn_agent.py`.
   * Runs Frontend BFF security suite: `npm test --prefix frontend` (14/14 tests passing).

3. **Gate 3: In-Process Engine vs. Cluster Latency Separation**
   * Runs `cargo test --test performance_test -- --nocapture`.
   * Validates local gateway latency SLO ($P99 < 5{,}000\,\mu\text{s}$).

4. **Gate 4: Security, Chaos, Governance, and Disaster Recovery**
   * Runs `security_test`, `chaos_test`, `governance_test`, `disaster_recovery_test`, `simulation_test`, `learning_controller_test`, `production_startup_test`, and `adversarial_security_test`.
   * Confirms zero cross-tenant leakage, fail-closed production startup, and 100% index rebuild from PostgreSQL durable records.

5. **Gate 5: Live Gateway SemanticCacheBench, SemanticCacheBench-T & Cryptographic Provenance**
   * Starts Sunborn Gateway daemon at `http://127.0.0.1:8080` with strict 5.0s health-check deadline (fails closed on timeout).
   * Runs `benchmarks/semantic_cache_bench.py --require-live`: Dispatches all 124 static queries across 8 semantic categories over live HTTP RPC to the gateway (`provenance_type: "LIVE_GATEWAY_DECISION_PATH"`, all traces with `execution_mode: "live_gateway_rpc"`).
   * Enforces exact query completeness: matches observed trace IDs with the versioned benchmark dataset (zero missing, unexpected, or duplicated IDs).
   * Enforces non-zero true positive reuse ($TP \ge 15$, preventing degenerate zero-reuse passes).
   * Enforces fail-closed Git provenance verification (any repository commit or tree mismatch fails closed as `INVALID`; explicit `--no-git` flags reduced scope).
   * Enforces canonical dataset contracts (AST-extracted category and ground_truth per query) and comprehensive 12-field baseline summary derivation across B3 and B2 in `benchmark_run.json` and temporal B3 and B2 in `run_manifest.json` (Naive B2 precision is 40.00% and false-hit rate is 48.39%).
   * Recomputes and cross-checks all 10 certificate empirical metrics directly from the raw source artifacts (`benchmark_run.json` and `run_manifest.json`), requiring zero-tolerance exact equality.
   * Runs `benchmarks/capacity_soak.py`: 11,000 requests processed under Zipfian distribution with zero memory leaks and atomic TDEK zeroization ($42\,\mu\text{s}$).
   * Runs `benchmarks/semantic_cache_bench_t.py`: Evaluates stateful temporal mutations across 6 sequential phases.
   * Runs negative verification test suite `research/tests/test_verifier_negative.py` (23 tests): Formally proves that offline simulation artifacts, incomplete traces, duplicate query IDs, arbitrary ID substitutions, ground-truth label tampering, category tampering, baseline summary mutations, Git hash mismatches, and unauthenticated keys are strictly rejected under the live gate.
   * Computes canonical SHA-256 digests and hashes of `benchmark_run.json` and `run_manifest.json`.
   * Generates `research/results/reproduction_certificate.json` (signed HMAC-SHA-256 when `SUNBORN_ATTESTATION_KEY` is set, or honest `LOCAL_DEVELOPMENT_INTEGRITY_CHECK` unauthenticated canonical digest when omitted).
   * Independently verifies the certificate and artifacts using `research/verify_certificate.py` with explicit 4-state outcomes:
     - `AUTHENTICATED`: Cryptographic HMAC-SHA-256 signature verified against trusted secret key.
     - `INTEGRITY_VERIFIED_UNAUTHENTICATED`: Canonical manifest digest and artifact hashes verified; no signature claimed.
     - `AUTHENTICATION_UNVERIFIED`: Signature present in certificate, but no trusted verification key supplied in environment (exit code 2).
     - `INVALID`: Any digest mismatch, missing file, incorrect git tree, trace irregularity, or `--require-auth` failure (exit code 1).

---

## 5. Artifact Output Files

All output files are saved in `research/results/`:
* `benchmark_run.json`: Evaluates B0, B1, B2, and B3 on 124 queries with 95% Wilson score confidence intervals.
* `run_manifest.json`: Full stateful temporal execution traces and mutation proofs.
* `reproduction_certificate.json`: Cryptographically signed attestation verifying environment, git tree SHA, provenance hashes, and zero false-hit invariants.
