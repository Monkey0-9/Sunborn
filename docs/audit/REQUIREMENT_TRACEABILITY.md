# Sunborn Requirement Traceability Matrix (RTM)

**Document Version:** 1.0.0  
**Audit Baseline:** November 9, 2026 (`70ac2b3cdba40bf1c51a8609567eb5fa74b3b73a`)  
**Traceability Rule:** Every functional and non-functional requirement must link directly to an architectural specification, code implementation, test file, and verifiable evidence artifact.

---

## 1. Security, Cryptography & Tenant Isolation

| Req ID | Requirement Description | Architectural Spec | Code Implementation Path | Automated Test File | Verifiable Evidence | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-SEC-01** | Multi-tenant cryptographic data isolation using per-tenant AEAD DEKs. | ADR-002: Envelope Encryption | `crates/sunborn-identity/src/crypto.rs` | `crates/sunborn-identity/tests/crypto_tests.rs` | Unit test validates AES-256-GCM ciphertext output. | `tested` |
| **REQ-SEC-02** | GDPR Article 17 compliant $\mathcal{O}(1)$ cryptographic shredding via TDEK destruction. | ADR-005: Cryptographic Shredding | `crates/sunborn-identity/src/shred.rs` | `crates/sunborn-db/tests/shred_tests.rs` | SQLx integration test verifies permanent key zeroization in PostgreSQL. | `verified in integration` |
| **REQ-SEC-03** | Enterprise KMS integration (AWS KMS & HashiCorp Vault Transit). | ADR-006: KMS Adapters | `crates/sunborn-identity/src/kms/` | Mock provider tests | Trait implementations defined; live cloud KMS calls mock-only. | `implemented` |
| **REQ-SEC-04** | Fail-closed credential validation and privileged route protection. | Security Whitepaper Sec 3 | `crates/sunborn-gateway/src/middleware/auth.rs` | `crates/sunborn-gateway/tests/auth_tests.rs` | Negative test verifies 401 Unauthorized on forged/missing tokens. | `verified in integration` |
| **REQ-SEC-05** | Dual-admin quorum threshold approval for destructive deletions. | ADR-008: Quorum Governance | `crates/sunborn-governance/src/quorum.rs` | `crates/sunborn-governance/tests/quorum_tests.rs` | Unit test validates rejection when quorum threshold $< 2$. | `tested` |
| **REQ-SEC-06** | Browser BFF proxy CSRF origin validation and header sanitization. | BFF Security Contract | `frontend/src/app/api/proxy/[...path]/route.ts` | `frontend/tests/proxy-security.test.mjs` | Node test harness validates 403 Forbidden on origin mismatch. | `verified in integration` |

---

## 2. Invariant Caching & Decision Engine

| Req ID | Requirement Description | Architectural Spec | Code Implementation Path | Automated Test File | Verifiable Evidence | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-CACHE-01** | Sub-millisecond Tier-1 exact match caching via W-TinyLFU eviction. | Autonomic Invariance Spec | `crates/sunborn-cache/src/l1.rs` | `crates/sunborn-cache/tests/l1_tests.rs` | Unit test verifies $< 15\,\mu\text{s}$ hit latency and frequency admission. | `tested` |
| **REQ-CACHE-02** | Conformal semantic caching bounded by risk error threshold ($\alpha \le 0.001$). | Research Paper Sec 4 | `crates/sunborn-semantic/src/conformal.rs` | `research/results/benchmark_run.json` | 124-query benchmark verifies 0 false hits on B3 baseline. | `tested` |
| **REQ-CACHE-03** | Structural guardrails for prompt negation polarity and numeric literal shift. | Research Paper Sec 5 | `crates/sunborn-semantic/src/guardrails.rs` | `crates/sunborn-semantic/tests/guardrail_tests.rs` | Unit tests verify immediate rejection of inverted polarity prompts. | `tested` |
| **REQ-CACHE-04** | Dynamic temporal caching with calibrated sliding-window validity. | Temporal Invariance Spec | `crates/sunborn-invalidation/src/temporal.rs` | `benchmarks/semantic_cache_bench_t.py` | 19-scenario temporal run recorded in `run_manifest.json` (0% recall). | `tested` |
| **REQ-CACHE-05** | Agent execution DAG memoization with Merkle inclusion proofs. | Execution Invariance Spec | `crates/sunborn-execution/src/dag.rs` | `crates/sunborn-execution/tests/dag_tests.rs` | Integration test verifies intermediate node reuse and cycle detection. | `tested` |
| **REQ-CACHE-06** | 10-dimensional request canonicalization preserving invariant identity. | RFC 001: 10-D Identity | `crates/sunborn-identity/src/canonical.rs` | `crates/sunborn-identity/tests/canonical_tests.rs` | Unit tests verify hash stability across key ordering and whitespace. | `tested` |

---

## 3. Persistence, Invalidation & Lineage

| Req ID | Requirement Description | Architectural Spec | Code Implementation Path | Automated Test File | Verifiable Evidence | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-INV-01** | Transactional outbox pattern for atomic event staging in PostgreSQL. | ADR-003: Outbox Pattern | `crates/sunborn-invalidation/src/outbox.rs` | `crates/sunborn-invalidation/tests/outbox_tests.rs` | Transaction test verifies outbox row insertion on state mutation. | `tested` |
| **REQ-INV-02** | Roaring bitmap dependency lineage tracking for instant cache purging. | Invalidation Lineage Spec | `crates/sunborn-invalidation/src/lineage.rs` | `crates/sunborn-invalidation/tests/lineage_tests.rs` | Unit test validates multi-key transitive invalidation in $< 1\,\text{ms}$. | `tested` |
| **REQ-INV-03** | Cold cache reconstruction and disaster recovery from PostgreSQL WAL. | DR Runbook 001 | `apps/sunborn-rebuild/src/main.rs` | `apps/sunborn-rebuild/tests/rebuild_tests.rs` | Integration test verifies rebuild of in-memory cache entries. | `tested` |
| **REQ-INV-04** | Schema migration versioning with idempotent DDL definitions. | Database Migration Plan | `migrations/0001-0003` | SQLx migration runner | Clean schema creation verified across PostgreSQL test database. | `verified in integration` |

---

## 4. FinOps & Spend Controls

| Req ID | Requirement Description | Architectural Spec | Code Implementation Path | Automated Test File | Verifiable Evidence | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FIN-01** | Accurate token avoidance accounting and net dollar savings tracking. | FinOps Accounting Spec | `crates/sunborn-core/src/finops.rs` | `crates/sunborn-core/tests/finops_tests.rs` | Ledger test verifies calculation of net USD savings from token avoided. | `verified in integration` |
| **REQ-FIN-02** | Atomic pre-request budget reservation and post-execution settlement. | ADR-009: Budget Lifecycle | `crates/sunborn-core/src/budget.rs` | Specification in ADR-009 | Implementation currently underway; post-execution spend check tested. | `implemented` |
| **REQ-FIN-03** | Spend velocity limiting with sliding window rate counters. | Rate Limiting Policy | `crates/sunborn-gateway/src/middleware/rate_limit.rs` | Gateway rate limit tests | Unit test verifies 429 Too Many Requests when spend burst exceeds cap. | `tested` |

---

## 5. Gateway Ingress, API & Protocols

| Req ID | Requirement Description | Architectural Spec | Code Implementation Path | Automated Test File | Verifiable Evidence | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-GW-01** | High-performance async HTTP gateway with Axum router. | Gateway Architecture Spec | `crates/sunborn-gateway/src/router.rs` | `scripts/check_openapi_drift.py` | 46/46 operations matched with zero schema drift. | `verified in integration` |
| **REQ-GW-02** | Offline standalone simulation mode for developer evaluation. | Frontend Architecture Spec | `frontend/src/lib/standalone-simulator.ts` | `frontend/tests/e2e-workflows.test.mjs` | 32/32 tests passing covering all 46 OpenAPI routes. | `verified in integration` |
| **REQ-GW-03** | Python client SDK with drop-in OpenAI-compatible client adapter. | SDK Integration Spec | `sdk/python/sunborn/client.py` | `sdk/python/test_sunborn_agent.py` | 24 tests passing across sync and async client invocations. | `verified in integration` |
| **REQ-GW-04** | Prometheus metrics exposition for latency, hit ratios, and evictions. | Observability Architecture | `crates/sunborn-gateway/src/metrics.rs` | Metric handler integration tests | `/metrics` endpoint exports Prometheus exposition format. | `verified in integration` |
| **REQ-GW-05** | Multi-region cross-cluster ingress and automatic failover. | Multi-Region Runbook | `deploy/helm/` | None | Documented runbooks only; zero automated multi-region tests. | `unverified` |
