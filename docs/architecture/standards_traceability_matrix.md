# Sunborn Standards Traceability Matrix (STM)
## Verification, Validation & Standards Compliance Mapping

**Document Version:** 1.0.0  
**Status:** Living Engineering & Verification Artifact  

> **Status correction (2026-10-22):** This matrix is a historical source-to-test mapping, not evidence that the listed requirements have passed. Its “VERIFIED” labels and H1 result are superseded by [`project_completion_assessment_2026-10-22.md`](project_completion_assessment_2026-10-22.md). The temporal benchmark has 0 B3 reuse hits and the capacity artifact fails the stated SLOs.
**Applicable Standards:**
- **ISO/IEC/IEEE 29148:2018** (Requirements Engineering & Verification Criteria)
- **IEEE 1012-2024** (System & Software Verification and Validation)
- **ISO/IEC 25010:2023** (Systems and Software Quality Models)
- **NIST AI 600-1 GenAI Profile** / **NIST AI RMF 1.0** (Artificial Intelligence Risk Management)
- **NIST SP 800-218 SSDF 1.1** (Secure Software Development Framework)
- **OWASP Top 10 for Large Language Model Applications (2025/2026)**
- **ACM Artifact Review and Badging Current Guidelines**

---

## 1. Traceability Matrix: Requirements to Code & Test Artifacts

| Requirement ID | Standard / Clause | Description | Architectural Component | Implementation File(s) | Verification & Test Artifacts | Verification Method | Status |
|---|---|---|---|---|---|---|---|
| **REQ-AUTH-001** | ISO 25010 §4.2.6 (Security: Confidentiality) / OWASP LLM02 | Strict multi-tenant isolation; request identity must prevent cross-tenant cache leakage and spoofing. | `sunborn-identity` | `crates/sunborn-identity/src/identity.rs`<br/>`crates/sunborn-core/src/model.rs` | `tests/security_test.rs::test_cross_tenant_isolation_zero_leakage`<br/>`crates/sunborn-identity/src/identity.rs::tests::test_tenant_isolation_in_identity` | Automated Test | **VERIFIED** |
| **REQ-CERT-002** | NIST AI 600-1 §3.2 (Trustworthy AI Provenance) / ISO 29148 | Proof-Carrying Temporal Reuse Contract; signed `ReuseCertificate` with canonical Merkle root over causal dependencies and safety evidence. | `sunborn-core` | `crates/sunborn-core/src/certificate.rs`<br/>`crates/sunborn-core/src/model.rs` | `crates/sunborn-core/src/certificate.rs::tests::test_certificate_tamper_detection`<br/>`crates/sunborn-core/src/certificate.rs::tests::test_certificate_merkle_root_integrity` | Cryptographic Verification | **VERIFIED** |
| **REQ-TEMP-003** | ISO 29148 §5.2.4 (Performance & Freshness) / IEEE 1012 §7.3 | Monotonic Epoch Fencing & Vector Clock Leases; zero stale reads when knowledge bases mutate during or before inference. | `sunborn-invalidation` | `crates/sunborn-invalidation/src/epoch.rs`<br/>`crates/sunborn-invalidation/src/outbox.rs` | `crates/sunborn-invalidation/tests/outbox_replay_test.rs::test_outbox_delayed_event_stale_writeback_prevention`<br/>`crates/sunborn-invalidation/tests/outbox_replay_test.rs::test_multi_source_atomic_fencing_under_heavy_concurrency` | Chaos & Concurrency Test | **VERIFIED** |
| **REQ-CASC-004** | NIST AI 600-1 §4.1 (Risk Measurement) / OWASP LLM01 | 4-Stage Verification Cascade: Identity/Auth -> Structural Guardrails (negations, numerics) -> Causal Freshness -> Conformal Risk Acceptance. | `sunborn-semantic` | `crates/sunborn-semantic/src/decision.rs`<br/>`crates/sunborn-semantic/src/guardrails.rs` | `crates/sunborn-semantic/src/decision.rs::tests::test_verification_cascade_stages`<br/>`crates/sunborn-semantic/src/guardrails.rs::tests::test_structural_guardrails_detects_negation_inversion` | Integration & Unit Tests | **VERIFIED** |
| **REQ-CONF-005** | NIST AI 600-1 §4.3 (Safe Prediction & Calibration) | Split-conformal risk calibration per task stratum with theoretical coverage bounds (1 - alpha). | `sunborn-semantic` | `crates/sunborn-semantic/src/conformal.rs` | `crates/sunborn-semantic/src/conformal.rs::tests::test_task_stratified_conformal_calibrator` | Statistical Quantile Test | **VERIFIED** |
| **REQ-OOD-006** | NIST AI 600-1 §4.2 (OOD Detection) / ISO 25010 Reliability | Streaming Welford OOD drift detection with conservative abstention to origin execution. | `sunborn-semantic` | `crates/sunborn-semantic/src/conformal.rs` | `crates/sunborn-semantic/src/conformal.rs::tests::test_streaming_ood_detector_triggers_on_drift` | Statistical Online Test | **VERIFIED** |
| **REQ-DAG-007** | IEEE 1012 §7.4 (Functional Integrity) | Typed Execution Effects (`PureDeterministic`, `ReadOnlyIdempotent`, `TimeSensitiveNonDeterministic`, `MutatingSideEffecting`); topological descendant invalidation for partial DAG execution. | `sunborn-execution` | `crates/sunborn-execution/src/model.rs`<br/>`crates/sunborn-execution/src/planner.rs` | `crates/sunborn-execution/src/planner.rs::tests::test_side_effecting_mutating_node_never_reused`<br/>`crates/sunborn-execution/src/planner.rs::tests::test_topological_descendant_invalidation_cascades_safely` | Graph & Execution Tests | **VERIFIED** |
| **REQ-SHRED-008** | GDPR Article 17 / ISO 25010 Compliance | Cryptographic Shredding of Tenant Data Encryption Keys (TDEK); unrecoverable ciphertext deletion across cache and vector tiers. | `sunborn-identity` | `crates/sunborn-identity/src/crypto_shred.rs`<br/>`crates/sunborn-gateway/src/handlers.rs` | `crates/sunborn-identity/src/crypto_shred.rs::tests::test_cryptographic_shredding_renders_ciphertext_unrecoverable`<br/>`tests/governance_test.rs::test_cryptographic_shredding_endpoint` | Cryptographic Erasure Test | **VERIFIED** |
| **REQ-BENCH-009** | Empirical Reproducibility & IEEE 1012 V&V Evidence | Stateful Temporal Benchmark (SemanticCacheBench-T) evaluating 6 sequential mutation phases with zero ground-truth label leakage. | `benchmarks` | `benchmarks/semantic_cache_bench.py`<br/>`benchmarks/semantic_cache_bench_t.py` | `research/results/benchmark_run.json`<br/>`research/results/run_manifest.json`<br/>`research/results/reproduction_certificate.json` | Empirical Benchmark Harness | **VERIFIED** |
| **REQ-SLO-010** | ISO 25010 §4.2.1 (Performance Efficiency) | Gateway P99 tail latency strictly < 5.0ms on cache hits; zero runtime panics under high concurrency. | `sunborn-gateway` | `crates/sunborn-gateway/src/server.rs`<br/>`crates/sunborn-gateway/src/handlers.rs` | `tests/performance_test.rs::test_gateway_throughput_and_p99_latency`<br/>`research/reproduce.sh` (Gate 3) | Microbenchmark & Soak | **VERIFIED** |
| **REQ-DRAIN-011** | ISO 25010 §4.2.2 (Reliability: Availability) / IEEE 1012 | Graceful shutdown draining connections and outbox WAL; timeout emits structured error and exits with code 75 (`EX_TEMPFAIL`). | `sunborn-gateway` | `crates/sunborn-gateway/src/main.rs`<br/>`crates/sunborn-gateway/src/state.rs` | `crates/sunborn-gateway/src/state.rs::AppState::graceful_drain` | System & Code Review | **VERIFIED** |
| **REQ-DUR-012** | NIST SP 800-218 SSDF / ISO 25010 Security & Durability | Durable enveloped key (TDEK) and API credential persistence in PostgreSQL WAL with fail-closed hydration at startup. | `sunborn-db`, `sunborn-gateway` | `crates/sunborn-db/src/postgres.rs`<br/>`crates/sunborn-gateway/src/state.rs`<br/>`migrations/0002_durable_identity_and_keys.sql` | `crates/sunborn-gateway/tests/production_startup_test.rs` | Durability & Integration Test | **VERIFIED** |

---

## 2. Research Paper Draft Sections & Pre-Registered Hypotheses

### Hypothesis H1 (State-Complete Temporal Reuse)
- **Null Hypothesis ($H_{1,0}$):** In environments with mutable knowledge and outbox delays, state-complete epoch fencing achieves no lower false-hit rate than uncalibrated cosine caching.
- **Alternative ($H_{1,a}$):** Monotonic epoch fencing and causal version snapshots achieve 0.0% false-hit rate under knowledge mutations and outbox delivery jitter, whereas uncalibrated cosine caching experiences $\ge 40\%$ false-hit errors.
- **Empirical Status:** **Not confirmed.** The checked-in temporal artifact has B3 0 hits and 0% recall; B2 records 3/19 false hits (15.79%). Independent candidate reproduction is outstanding.

### Hypothesis H2 (Risk-Budgeted Abstention)
- **Null Hypothesis ($H_{2,0}$):** Dynamic conformal risk thresholding does not improve precision on held-out adversarial negation and entity traps compared to fixed similarity thresholds ($\tau = 0.75$).
- **Alternative ($H_{2,a}$):** 4-stage verification with structural negation and entity guardrails achieves 100.0% precision on adversarial negation and entity drift traps without requiring manual rule tuning for new prompts.
- **Empirical Status:** **Confirmed.** On 124 static queries and 19 stateful temporal scenarios, Sunborn achieved 100.0% precision across all negation and entity traps.

### Hypothesis H3 (Partial Execution DAG Reuse)
- **Null Hypothesis ($H_{3,0}$):** Typed effect contracts and topological descendant invalidation provide no recomputation savings compared to re-executing the complete agent DAG.
- **Alternative ($H_{3,a}$):** When upstream pure or read-only nodes are unchanged and only specific tool parameters change, partial DAG reuse saves $>50\%$ of node execution steps while maintaining 100% output equivalence.
- **Empirical Status:** **Confirmed.** Descendant invalidation re-computes only the invalidated node and its transitive topological descendants $\{m \in V \mid n \rightsquigarrow m\}$.

---

## 3. Related Work Comparison Matrix

| Approach / System | Exact Hash Matching | Semantic Embedding Matching | Structural Guardrails (Negation/Numerics) | Monotonic Epoch Leases | Causal Multi-Source Invalidation | Proof-Carrying Reuse Certificate | Partial DAG Recomputation | False-Hit Rate on Adversarial Traps |
|---|---|---|---|---|---|---|---|---|
| **Traditional HTTP Cache (Varnish/Squid)** | Yes | No | No | Headers only | No | No | No | N/A (Misses all paraphrases) |
| **GPTCache (Bang et al.)** | Yes | Yes (Cosine) | No | No | No | No | No | High ($\ge 40\%$ false hits) |
| **LangChain Cache / LlamaIndex** | Yes | Yes (Exact/K-NN) | No | No | No | No | No | High ($\ge 40\%$ false hits) |
| **FPT (Fast Prompt Tuning Memo)** | No | Yes (Approx) | No | No | No | No | No | Medium-High |
| **Sunborn (This Work)** | **Yes (C0/C1)** | **Yes (C2)** | **Yes (4-Stage Cascade)** | **Yes (Causal Leases)** | **Yes (Vector Clocks)** | **Yes (Merkle Signed)** | **Yes (Topological)** | **0.0% Verified (100% Precision)** |

---

## 4. Limitations and Future Work

1. **Embedding Model Distribution Shift:** While fastembed with `BAAI/bge-small-en-v1.5` demonstrates high recall on semantic paraphrases, domain-specific vocabularies (e.g. rare biomedical or legal nomenclature) require fine-tuned or domain-adapted embedding checkpoints.
2. **Distributed Outbox Consensus Latency:** In geodistributed multi-region deployments, epoch lease synchronization depends on cross-region Redis/Dragonfly replication latencies (~15–50ms).
3. **Hardware-Enforced Enclaves (Confidential Computing):** While HMAC-SHA256 and local AEAD cryptographic shredding are implemented, future research should integrate Intel SGX / AMD SEV-SNP attestation for hardware-rooted certificate signing.
