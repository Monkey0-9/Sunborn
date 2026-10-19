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

5. **Gate 5: SemanticCacheBench, SemanticCacheBench-T & Signed Provenance**
   * Runs `benchmarks/semantic_cache_bench.py`: Evaluates 124 static queries across 8 semantic categories.
   * Runs `benchmarks/capacity_soak.py`: 11,000 requests processed under Zipfian distribution with zero memory leaks and atomic TDEK zeroization ($42\,\mu\text{s}$).
   * Runs `benchmarks/semantic_cache_bench_t.py`: Evaluates stateful temporal mutations across 6 sequential phases.
   * Computes SHA-256 hashes of `benchmark_run.json` and `run_manifest.json`.
   * Generates `research/results/reproduction_certificate.json` (authenticated with HMAC-SHA-256 when `SUNBORN_ATTESTATION_KEY` is configured, or honest `LOCAL_DEVELOPMENT_INTEGRITY_CHECK` unauthenticated canonical digest when omitted).
   * Independently verifies the certificate, artifact digests, and git provenance using `research/verify_certificate.py`.

---

## 5. Artifact Output Files

All output files are saved in `research/results/`:
* `benchmark_run.json`: Evaluates B0, B1, B2, and B3 on 124 queries with 95% Wilson score confidence intervals.
* `run_manifest.json`: Full stateful temporal execution traces and mutation proofs.
* `reproduction_certificate.json`: Cryptographically signed attestation verifying environment, git tree SHA, provenance hashes, and zero false-hit invariants.
