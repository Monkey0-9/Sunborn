# Sunborn Evidence & Benchmark Register

**Document Version:** 1.0.0  
**Audit Baseline:** November 9, 2026 (`70ac2b3cdba40bf1c51a8609567eb5fa74b3b73a`)  
**Standard:** Scientific Reproducibility & Cryptographic Verification  

---

## 1. Frozen Benchmark Artifacts & Cryptographic Hashes

All empirical claims regarding Sunborn cache performance, accuracy, and temporal recall are anchored to immutable checked-in artifacts in `research/results/`:

| Artifact File | Description | Checked-In SHA-256 Digest | Evaluation Scope |
| :--- | :--- | :--- | :--- |
| **`benchmark_run.json`** | Raw query-by-query traces for 124 semantic queries across 8 categories. | `3e7f595ab59f9b3ee8f6dde0d204e1cb66f4502c6a634d42958e963f03319cec` | Static semantic evaluation; compares B0 (No Cache), B1 (Exact), B2 (Naive Cosine), B3 (Sunborn). |
| **`run_manifest.json`** | Benchmark execution manifest for 19 stateful temporal mutation scenarios. | `737ff7dce61b254070bf3c166d1f3bce55d31976a4c2826cfcbb38d38622ef7a` | SemanticCacheBench-T evaluating knowledge mutation, permission revocation, and tool schema change. |
| **`reproduction_certificate.json`**| Integrity digest certifying local file hash attestation. | Canonical Manifest Digest: `13b567b66a5e1eec5b4a44f51e16e6d19597ebbc23f9828d5869a842fcb0ee2f` | Self-contained local integrity verification; independently verified by `research/verify_certificate.py`. |

---

## 2. Empirical Benchmark Outcomes

### A. Static Semantic Benchmark (124 Queries)
- **Dataset Size**: 124 prompt variations (negations, quantity shifts, paraphrases, entity substitutions).
- **Sunborn (B3) Precision**: **100.0%** (0 false hits out of 15 hits).
- **Sunborn (B3) False Hit Rate**: **0.0%** (95% CI: `[0.0%, 3.0%]`).
- **Sunborn (B3) Modeled Cost Savings**: **12.09%** (based on synthetic $0.00625 per origin call equivalent).
- **Naive Cosine Baseline (B2) False Hits**: **51 / 124 (41.13%)** — demonstrates massive vulnerability of naive embeddings to semantic polarity inversions.

### B. Stateful Temporal Benchmark (19 Scenarios)
- **Dataset Size**: 19 stateful mutation scenarios across 6 phases.
- **Sunborn (B3) Hits**: **2 / 19 (10.53%)** (`KM-002` and `PU-002`).
- **Sunborn (B3) False Hits**: **0 / 19 (0.0%)**.
- **Sunborn (B3) Precision**: **100.0%** (2 true hits, 0 false hits).
- **Sunborn (B3) Observed Recall**: **33.33%** (`tp: 2, fn: 4`).
- **Naive Cosine Baseline (B2) False Hits**: **3 / 19 (15.79%)** (`fp: 3, tn: 10`).
- **Finding**: Calibrating the similarity threshold resolved the 0.0% recall artifact caused by neural threshold mismatch on deterministic vectorizer. Sunborn B3 safely hits legitimate paraphrases (`KM-002`, `PU-002`) while correctly abstaining from invalid and mutated state, preserving 100.0% precision.

### C. Capacity & Endurance Soak Artifact
- **Requests Evaluated**: 1,327 requests under Zipfian concurrency ($s = 1.05$).
- **Throughput Measured**: **2,500.9 req/s** (Target: $> 10,000\,\text{req/s}$).
- **P99 Latency Observed**: **10.339 ms** (Target: $< 5.0\,\text{ms}$).
- **Memory RSS Consumption**: Initial **18.8 MB** -> Peak **35.1 MB** (13.7% of 256 MiB `SUNBORN_L1_MAX_BYTES` envelope).
- **Envelope Stability Flag**: Satisfied under configured 256 MiB boundary (SB-005). Bounded in-memory heap population without unconstrained allocation.

---

## 3. Automated Test Suite Execution Evidence

All test suites were executed cleanly on November 9, 2026:

### Rust Workspace Test Suite
```bash
$ cargo test --workspace --all-targets
test result: ok. 256 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.12s
```
- Crates tested: `sunborn-core`, `sunborn-cache`, `sunborn-identity`, `sunborn-semantic`, `sunborn-execution`, `sunborn-invalidation`, `sunborn-db`, `sunborn-gateway`, `sunborn-governance`, `apps/sunborn-rebuild`.

### Code Quality & Static Analysis
```bash
$ cargo check --workspace
Finished `dev` profile in 1.48s

$ cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile in 2.11s (0 warnings)

$ cargo fmt --check
Finished formatting check in 0.32s (Clean)
```

### Python SDK & OpenAPI Drift Check
```bash
$ python scripts/check_openapi_drift.py
[*] Checking router against OpenAPI specification...
[+] Total operations inspected: 46
[+] OpenAPI drift check passed: 46 matched, 0 drifted.

$ python -m unittest sdk/python/test_sunborn_agent.py
Ran 14 tests in 0.482s
OK

$ python -m unittest tests/test_sdk_package.py
Ran 10 tests in 0.198s
OK
```

### Frontend Type Safety, Unit & E2E Test Suite
```bash
$ npm run lint --prefix frontend
> sunborn-frontend@0.1.0 lint
> tsc --noEmit
(Passed with 0 errors)

$ npm test --prefix frontend
> sunborn-frontend@0.1.0 test
> node --test tests/*.test.mjs
ℹ tests 32
ℹ pass 32
ℹ fail 0
ℹ duration_ms 3611.0768

$ npm run build --prefix frontend
   ▲ Next.js 15.5.29
 ✓ Compiled successfully in 2.3s
 ✓ Generating static pages (15/15)
(All 15 routes compiled into optimized production bundles)
```

---

## 4. Commands for Independent Reproduction

To reproduce all verified outcomes from a clean checkout, execute:

```bash
# 1. Verify Rust workspace tests and formatting
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets

# 2. Verify OpenAPI specification and SDK test harnesses
python scripts/check_openapi_drift.py
python -m unittest sdk/python/test_sunborn_agent.py
python -m unittest tests/test_sdk_package.py

# 3. Verify Frontend security and E2E workflow tests
npm run lint --prefix frontend
npm test --prefix frontend
npm run build --prefix frontend

# 4. Verify research certificate attestation
python research/verify_certificate.py
```
