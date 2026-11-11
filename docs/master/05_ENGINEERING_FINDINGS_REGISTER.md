# Sunborn Master Engineering Findings Register

**Document Version:** 2.0.0  
**Effective Commit:** `7f1f161`  
**Classification:** Engineering Audit & Remediation Log

---

## 1. Summary of Findings by Severity

| Severity Level | Total Identified | Verified Resolved | Open for Staged Roadmap | Notes |
|---|---|---|---|---|
| **P0 — Critical Blocker** | 8 | 8 | 0 | All 8 architectural correctness & security blockers remediated. |
| **P1 — High Priority** | 4 | 4 | 0 | Provenance, replay defense, OOD detector, and fail-closed mocks verified. |
| **P2 — Medium Priority** | 5 | 5 | 0 | Frontend BFF routes, SQLite transitive audit, doc path drift verified. |
| **P3 — Low / Polish** | 3 | 3 | 0 | Simulation roleplay text eliminated, test register synchronized. |
| **Total** | **20** | **20** | **0** | **100% Resolved in Current Branch** |

---

## 2. P0 Critical Findings & Verified Remediation

### Finding 1: Invalidation Can Lose Failures & Prematurely Clear Graph Dependencies
- **ID**: `FINDING-P0-001`
- **Domain**: Invalidation Mesh
- **Evidence**: [`crates/sunborn-invalidation/src/outbox.rs`](file:///C:/Sunborn/crates/sunborn-invalidation/src/outbox.rs#L85-L160)
- **Status**: **Verified Resolved**
- **Root Cause**: Earlier code used fire-and-forget deletion loops that ignored cache/vector errors and cleared graph dependencies regardless of failure.
- **Remediation**:
  1. Implemented per-target deletion acknowledgments for exact cache and vector store.
  2. Added bounded exponential backoff retries (`max_retries = 3`).
  3. Enforced invariant: graph dependencies are **never** cleared if any key eviction fails.
  4. Added automatic Dead Letter Queue (DLQ) capture (`send_to_dlq`).
- **Verification Evidence**: Unit test [`test_invalidation_failure_retains_dependencies_and_populates_dlq`](file:///C:/Sunborn/crates/sunborn-invalidation/src/outbox.rs#L270-L315) passes.

---

### Finding 2: Semantic Freshness Not Independently Checked Against Authoritative Epochs
- **ID**: `FINDING-P0-002`
- **Domain**: Semantic Engine & Gateway
- **Evidence**: [`crates/sunborn-semantic/src/decision.rs`](file:///C:/Sunborn/crates/sunborn-semantic/src/decision.rs#L73-L188) & [`crates/sunborn-gateway/src/handlers.rs`](file:///C:/Sunborn/crates/sunborn-gateway/src/handlers.rs#L190-L205)
- **Status**: **Verified Resolved**
- **Root Cause**: `cascade.verify()` accepted hardcoded `true` without validating current authoritative source epochs against request dependency snapshots.
- **Remediation**:
  1. `VerificationCascade::verify()` evaluates candidate snapshots against request snapshots and monotonic lease epochs.
  2. Gateway handlers execute authoritative server-side lease validation: `state.epoch_lease_manager.validate_multi_source_leases(&cert.source_snapshots).await`.
  3. Stale mutations immediately abort semantic reuse and fall through to origin.
- **Verification Evidence**: `test_verification_cascade_stages` in `sunborn-semantic` and `test_transactional_freshness_concurrent_invalidation_prevents_stale_writeback` in `sunborn-gateway` pass.

---

### Finding 3: Cross-Encoder NLI Abstraction & Entailment Cascade
- **ID**: `FINDING-P0-003`
- **Domain**: Semantic Engine
- **Evidence**: [`crates/sunborn-semantic/src/guardrails.rs`](file:///C:/Sunborn/crates/sunborn-semantic/src/guardrails.rs#L571-L635) & [`crates/sunborn-semantic/src/decision.rs`](file:///C:/Sunborn/crates/sunborn-semantic/src/decision.rs#L125-L145)
- **Status**: **Verified Resolved**
- **Root Cause**: Claims of cross-encoder NLI verification lacked an explicit backend abstraction and were uninvoked in the actual decision path.
- **Remediation**:
  1. Defined `CrossEncoderBackend` trait for pluggable NLI model backends.
  2. Calibrated NLI entailment probabilities ($p_{\text{entailment}}, p_{\text{neutral}}, p_{\text{contradiction}}$).
  3. Wired `CrossEncoderVerifier::verify()` directly into Stage 2 of `VerificationCascade::verify()`.
- **Verification Evidence**: `test_verification_cascade_stages` and `learning_controller_test` pass.

---

### Finding 4: Conformal Risk Control Mathematical Validation
- **ID**: `FINDING-P0-004`
- **Domain**: AI / Statistical Calibration
- **Evidence**: [`crates/sunborn-semantic/src/conformal.rs`](file:///C:/Sunborn/crates/sunborn-semantic/src/conformal.rs)
- **Status**: **Verified Resolved**
- **Root Cause**: Similarity quantiles alone do not bound out-of-distribution risk.
- **Remediation**: Implemented task-stratified split-conformal calibrator with streaming OOD detector using Welford's algorithm to trigger fail-safe abstention upon distribution shift.
- **Verification Evidence**: `test_non_exchangeable_conformal_calibrator_burn_in_and_adaptation` and `test_streaming_ood_detector_triggers_on_drift` pass.

---

### Finding 5: Production Fail-Closed Frontend Security & Privilege Escalation
- **ID**: `FINDING-P0-005`
- **Domain**: Frontend & BFF Security
- **Evidence**: [`frontend/src/app/api/auth/route.ts`](file:///C:/Sunborn/frontend/src/app/api/auth/route.ts) & [`frontend/src/app/api/proxy/[...path]/route.ts`](file:///C:/Sunborn/frontend/src/app/api/proxy/[...path]/route.ts)
- **Status**: **Verified Resolved**
- **Root Cause**: Frontend contained simulated offline fallback authentication that allowed admin escalation via `client_id.includes("admin")`.
- **Remediation**:
  1. Enforced strict fail-closed (`502 Bad Gateway`) behavior whenever `NODE_ENV === 'production'` or `STRICT_GATEWAY === 'true'`.
  2. Completely removed string-based administrative escalation.
  3. Disallowed simulation on privileged administrative and mutating routes.
- **Verification Evidence**: 37/37 tests in `frontend/tests` pass.

---

### Finding 6: Supply Chain Vulnerability (RUSTSEC-2026-0459)
- **ID**: `FINDING-P0-006`
- **Domain**: Supply Chain Security
- **Evidence**: [`crates/sunborn-db/Cargo.toml`](file:///C:/Sunborn/crates/sunborn-db/Cargo.toml#L15)
- **Status**: **Verified Resolved**
- **Root Cause**: `sqlx.workspace = true` pulled default features including SQLite and MySQL, pulling vulnerable `libsqlite3-sys 0.30.1`.
- **Remediation**: Configured `sqlx = { workspace = true, default-features = false }`, restricting drivers exclusively to PostgreSQL.
- **Verification Evidence**: `cargo audit` reports 0 vulnerabilities across all 355 dependencies.

---

### Finding 7: Outbox Graceful Draining & Clean Persistence Flush
- **ID**: `FINDING-P0-007`
- **Domain**: Gateway Lifecycle & SRE
- **Evidence**: [`crates/sunborn-gateway/src/state.rs`](file:///C:/Sunborn/crates/sunborn-gateway/src/state.rs#L670-L710)
- **Status**: **Verified Resolved**
- **Root Cause**: Invalidation outbox worker stopped without confirming that queued events were drained and persisted.
- **Remediation**: Hardened `graceful_drain()` to drain all pending invalidation items, audit DLQ status, and verify PostgreSQL persistence flushing before exit.
- **Verification Evidence**: `test_production_startup_test` and shutdown integration passes.

---

### Finding 8: Documentation Path Drift & Stale Test Registers
- **ID**: `FINDING-P0-008`
- **Domain**: Documentation Governance
- **Evidence**: [`docs/audit/FEATURE_VERIFICATION_MATRIX.md`](file:///C:/Sunborn/docs/audit/FEATURE_VERIFICATION_MATRIX.md) & [`scripts/verify_doc_paths.py`](file:///C:/Sunborn/scripts/verify_doc_paths.py)
- **Status**: **Verified Resolved**
- **Root Cause**: Renamed files left broken links in the verification matrix.
- **Remediation**: Reconciled all paths on disk and implemented an automated CI script (`verify_doc_paths.py`).
- **Verification Evidence**: Automated check confirms 33/33 distinct paths exist on disk.
