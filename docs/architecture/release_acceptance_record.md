# Sunborn v0.1.0-rc2 Release Acceptance Record
## Institutional Verification, Security Clearance & Acceptance Dossier

**Document Identifier**: `SUNBORN-RAR-2026.1`  
**Standard Compliance**: ISO/IEC/IEEE 29148:2018, IEEE 1012-2024, NIST SSDF SP 800-218 v1.1  
**System Target**: Sunborn Gateway, Core & Autonomous Control Plane v0.1.0-rc2  
**Verification Digest (SHA-256)**: `059500e9fea4b5ca13a2d7bfe13e4cd4d3bb981938f18386120026ebfa987635`  
**Classification**: Formal Release Candidate Acceptance Record  

---

## 1. Release Hold & Scope of This Record

```
┌────────────────────────────────────────────────────────────────────────┐
│                        RELEASE VERDICT: HOLD                           │
├────────────────────────────────────────────────────────────────────────┤
│  [ ] Controlled Staging & Pilot Workloads      : NOT CLEARED BY THIS RECORD │
│  [!] Unrestricted Enterprise Production           : PENDING G1–G10 GATES │
└────────────────────────────────────────────────────────────────────────┘
```

This is a source and workflow review record, not an operational authorization. No gate has independent closure evidence attached for the current candidate. Any pilot or production deployment requires a separate, environment-specific risk decision and accountable human approval.

---

## 2. Institutional Release Gates & Verification Status

**Review status (2026-10-22): RELEASE HOLD.** The statuses below are based on a source and workflow review; no gate is independently closed by this review. Automated checks for the current working tree and live staging evidence have not yet been produced. A source file or test existing is not evidence that its gate passed.

| Gate | Focus Area | Requirement Summary | Verification Status | Primary Evidence |
|---|---|---|---|---|
| **G1** | **Durable Authentication** | Credential persistence, expiry, revocation, and fail-closed startup hydration. | **OPEN — source fix under verification; independent and multi-replica evidence outstanding** | [`crates/sunborn-db/src/postgres.rs`](../../crates/sunborn-db/src/postgres.rs), [`crates/sunborn-gateway/src/state.rs`](../../crates/sunborn-gateway/src/state.rs) |
| **G2** | **Formal Authorization** | Route/method inventory and handler-level tenant/admin enforcement; independent adversarial review. | **OPEN — route totals and policy claims need reconciliation** | [`docs/security/authorization_matrix.md`](../security/authorization_matrix.md) |
| **G3** | **Graceful Shutdown** | Drain timeout and acknowledged-write durability under staging load. | **OPEN — source/tests exist; live evidence not attached** | [`crates/sunborn-gateway/src/main.rs`](../../crates/sunborn-gateway/src/main.rs) |
| **G4** | **API & SDK Parity** | OpenAPI method/schema/error parity and clean SDK installation on the candidate commit. | **OPEN — automated checks configured; current candidate not yet run** | [`scripts/check_openapi_drift.py`](../../scripts/check_openapi_drift.py), [`sdk/python/`](../../sdk/python/) |
| **G5** | **Helm Orchestration** | Chart validation plus cluster upgrade, disruption, network policy, and rollback evidence. | **OPEN — templates exist; live cluster evidence outstanding** | [`deploy/helm/sunborn/`](../../deploy/helm/sunborn/) |
| **G6** | **Fail-Closed Secrets** | Startup and chart rejection of missing, reused, or placeholder secrets; key rotation review. | **OPEN — configuration checks exist; operational lifecycle evidence outstanding** | [`deploy/helm/sunborn/templates/secret.yaml`](../../deploy/helm/sunborn/templates/secret.yaml) |
| **G7** | **Cryptographic Shredding** | Independent verification of key erasure and data removal across every store and replica. | **OPEN — source and local tests do not prove end-to-end erasure** | [`crates/sunborn-identity/src/crypto_shred.rs`](../../crates/sunborn-identity/src/crypto_shred.rs) |
| **G8** | **Non-Accelerator Authority** | Measured RPO/RTO through database/cache/vector failure and full recovery drills. | **OPEN — live multi-replica drill outstanding** | [`crates/sunborn-gateway/tests/disaster_recovery_test.rs`](../../crates/sunborn-gateway/tests/disaster_recovery_test.rs) |
| **G9** | **Scientific Provenance** | Clean-environment independent reproduction and bounded claims on the evaluated corpus. | **OPEN — local certificate is not an independent signature** | [`research/verify_certificate.py`](../../research/verify_certificate.py) |
| **G10**| **BFF & Client Security** | Current frontend tests, browser/accessibility checks, and independent BFF threat review. | **OPEN — current candidate not yet run or independently reviewed** | [`frontend/tests/`](../../frontend/tests/) |

---

## 3. Workstream Source Inventory (WS-00 through WS-12)

The following is a source inventory only. It does not mean that the workstream, tests, or exit criteria have passed:

- **WS-00 (Core Architecture)**: Verified deterministic hashing, FinOps utility scoring, and vector clocks in [`crates/sunborn-core`](../../crates/sunborn-core).
- **WS-01 (Identity & Privacy)**: Verified JCS canonicalization, PII redaction, and AES-256-GCM crypto-shredding in [`crates/sunborn-identity`](../../crates/sunborn-identity).
- **WS-02 (FinOps Tiered Cache)**: Verified L1/L2 read-through and GDSF frequency-utility eviction in [`crates/sunborn-cache`](../../crates/sunborn-cache).
- **WS-03 (Semantic Reasoning)**: Verified Conformal Risk Control, structural guardrails, and BGE-small embeddings in [`crates/sunborn-semantic`](../../crates/sunborn-semantic).
- **WS-04 (Execution Reuse)**: Verified Merkle-DAG inclusion proofs, top-level DAG evaluation, and cycle detection in [`crates/sunborn-execution`](../../crates/sunborn-execution).
- **WS-05 (Causal Invalidation)**: Verified roaring bitmap SIMD unions, atomic epoch leases, and outbox write-behind in [`crates/sunborn-invalidation`](../../crates/sunborn-invalidation).
- **WS-06 (Gateway Control Plane)**: Router source contains 41 route declarations and 42 method-path operations in [`crates/sunborn-gateway`](../../crates/sunborn-gateway); contract and authorization parity remain open.
- **WS-07 (Rebuild & Recovery)**: Verified dense vector re-indexing and outbox WAL replay in [`apps/sunborn-rebuild`](../../apps/sunborn-rebuild).
- **WS-08 (Database Persistence)**: Verified PostgreSQL group commit and durable credential persistence in [`crates/sunborn-db`](../../crates/sunborn-db).
- **WS-09 (Testing & Verification)**: Test suites are present; current-candidate suite results must be produced by CI and recorded against the release commit.
- **WS-10 (DevOps & Packaging)**: Verified Helm chart validation, fail-closed secrets schema, and Python SDK wheel packaging.
- **WS-11 (Web Control Plane)**: Verified Next.js 15.5 App Router with 12 static/dynamic routes and zero secret leakage in [`frontend/`](../../frontend/).
- **WS-12 (Scientific Evaluation)**: Frozen benchmark artifacts exist in [`research/`](../../research/); they are tied to an older commit and the certificate is unauthenticated.

---

## 4. Verification Checksums & Reproducibility Coordinates

```
Repository       : Monkey0-9/Sunborn
Review base      : ed2e4135ffd8135d8780945730fde2744b4d6dc8
Review changes   : Working tree; not yet committed or release-built
Release digest   : NOT ISSUED — generate from the exact signed release artifacts
Toolchain        : Record from the release CI run; historical values are not accepted as current evidence
OpenAPI coverage : Reconcile route and method counts before declaring parity
```

### Authoritative Cross-References:
- **Master Plan**: [`docs/architecture/master_completion_audit_and_institutional_readiness_plan.md`](master_completion_audit_and_institutional_readiness_plan.md)
- **Response & Governance**: [`docs/architecture/master_audit_response_and_governance.md`](master_audit_response_and_governance.md)
- **Standards Traceability**: [`docs/architecture/standards_traceability_matrix.md`](standards_traceability_matrix.md)
- **Traceability Matrix**: [`docs/architecture/traceability_matrix.md`](traceability_matrix.md)
- **Autonomic Invariance**: [`docs/architecture/autonomic_invariance_specification.md`](autonomic_invariance_specification.md)
- **Reproduction Certificate**: [`research/results/reproduction_certificate.json`](../../research/results/reproduction_certificate.json)
