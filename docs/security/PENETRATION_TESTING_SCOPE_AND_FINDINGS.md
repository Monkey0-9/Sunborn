# Sunborn Enterprise Security Assessment & Penetration Testing Register

**Engagement Type:** In-Tree Engineering Adversarial Red-Team & White-Box Penetration Test  
**Scope:** Distributed Semantic Gateway, Cryptographic Key Custody, Multi-Tenant Isolation, BFF Proxy Ingress, FinOps Micro-Unit Accounting  
**Standard:** OWASP Top 10 API Security Risks (2023), NIST SP 800-115, CERT-In Guidelines  
**In-Tree Vector Status:** ALL 10 IN-TREE ATTACK VECTORS REMEDIATED & VERIFIED (0 Critical, 0 High, 0 Medium Open via automated regression tests)  
**External Assurance Notice:** An independently signed external penetration-test report from an accredited third-party commercial cybersecurity audit firm is an external operational prerequisite for Stage 2 Public Launch and remains tracked as an open external gate (Gate N2 / Stage 2).

---

## 1. Executive Summary

A comprehensive threat modeling and penetration testing audit was performed against Sunborn's network ingress, authentication handlers, cryptographic shredding lifecycle, and database persistence layers. Ten key attack vectors were probed. Every identified vulnerability has been remediated with automated regression tests enforcing prevention. This document constitutes the authoritative in-tree defect and remediation ledger for engineering validation; commercial third-party accreditation requires the formal independent audit engagement report specified in `docs/STATUS.md` §3.

---

## 2. Vulnerability Assessment & Findings Ledger

```
========================================================================================================================
                                      PENETRATION TESTING FINDINGS LEDGER
========================================================================================================================
ID        Vector                     Severity  Status   Remediation & Architectural Defense              Verified Test
------------------------------------------------------------------------------------------------------------------------
PEN-01    BFF Proxy SSRF Exfiltration CRITICAL  CLOSED   Normalized path origin-pinning; rejects          bff_security.test.mjs
                                                        protocol-relative // foreign URL bases
PEN-02    Cache Poisoning Injection   CRITICAL  CLOSED   AST structural guardrails & 5-strike             test_held_out_eval
                                                        contradiction defense in learning store
PEN-03    Timing Side-Channel on Keys HIGH      CLOSED   Constant-time comparison via subtle::           test_security_test
                                                        ConstantTimeEq on tokens and HMACs
PEN-04    Residual Tenant Data Leak   HIGH      CLOSED   Atomic temp-file WAL compaction &                durable_erasure_test
                                                        physical disk verification (zero remnants)
PEN-05    Floating-Point Ledger Drift HIGH      CLOSED   Authoritative 64-bit integer micro-units         test_financial_settlement_
                                                        with non-negative database constraints           rejects_negative_and_nan
PEN-06    Unauthorized Admin Purge    HIGH      CLOSED   Dual-control M-of-N quorum admin key             test_quorum_auth
                                                        enforcement on destructive endpoints
PEN-07    Advisory Lock Hijack / DOS  MEDIUM    CLOSED   Session-scoped transaction locks                distributed_finops_test
                                                        (pg_advisory_xact_lock) with timeouts
PEN-08    Epoch Replay Desync         MEDIUM    CLOSED   Persistent PostgreSQL monotonic epoch            test_multi_replica
                                                        authority and vector clocks
PEN-09    Unauthenticated Local BYOK  MEDIUM    CLOSED   Loopback binding (127.0.0.1) isolation;          route_contract_parity_test
                                                        production mode strictly requires API key
PEN-10    Outbox Message Loss         MEDIUM    CLOSED   Transactional lease claims with FOR UPDATE      test_outbox_replay
                                                        SKIP LOCKED and dead-letter queue
========================================================================================================================
```

---

## 3. Threat Matrix Verification

1. **Cryptographic Key Shredding & Data Destruction**:
   Verified by `crates/sunborn-gateway/tests/durable_erasure_test.rs` and `governance_test.rs`. When a tenant deletion is requested, the TDEK is zeroized in memory and purged from PostgreSQL, rendering all existing ciphertext irrecoverable. The WAL rewriting cleans disk files with `sync_all` fsync.

2. **Prompt Injection & Adversarial Negation Bypass**:
   Verified by `crates/sunborn-semantic/src/guardrails.rs`. Structural AST guardrails inspect entity bindings and agglutinative negation prefixes across 12 languages, ensuring semantically conflicting prompts never produce false cache hits.

3. **Financial Spend Evasion**:
   Verified by `crates/sunborn-db/src/postgres.rs` and `durable_erasure_test.rs`. Monetary settlements with non-finite or negative inputs are rejected at the gate rather than accepted or clamped to zero.
