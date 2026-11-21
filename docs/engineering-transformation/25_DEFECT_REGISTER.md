# Master Defect Register (Remediation Tracking D-01 through D-12 and Issues #3 through #10)

**Total Audit Defects:** 20  
**Remediated & Closed:** 20 / 20 (100% Resolution Rate)  
**Open P0 / P1 Defects:** 0  
**Authoritative Status:** Stage 0 Controlled Pilot Approved; Stage 1/2 Gated on External Staging.

---

## 1. Audit Remediation Defect Ledger (D-01 through D-12)

```
=========================================================================================================
                          SUNBORN REMEDIATION MASTER DEFECT LEDGER (PHASE 1)
=========================================================================================================
ID    Severity  Status  Component        Summary & Remediation                              Verified Test
---------------------------------------------------------------------------------------------------------
D-01  P0 Crit   CLOSED  Invalidation     Atomic two-phase purge across cache & Qdrant       test_orchestrator
D-02  P0 Crit   CLOSED  Distributed      Redis Streams cluster synchronization wired        test_multi_replica
D-03  P0 Crit   CLOSED  Persistence      Persistent PostgreSQL monotonic epoch authority    test_tenant_keys
D-04  P0 Crit   CLOSED  AI / ML          AST Guardrails & Ambiguity Abstention (P=1.0)      test_held_out_eval
D-05  P1 High   CLOSED  Database         Unified sequential migrations (_sunborn_migrations)test_db_lifecycle
D-06  P1 High   CLOSED  Persistence      Durable outbox background flusher with DLQ         test_outbox_replay
D-07  P1 High   CLOSED  Concurrency      Eliminated Tokio reactor panic in listeners        cargo test --doc
D-08  P1 High   CLOSED  Cryptography     RFC 6962 domain separation in Merkle trees         test_merkle_boundary
D-09  P1 High   CLOSED  Frontend         Purged fake client simulator; added real BFF       ceremony_status.test
D-10  P2 Med    CLOSED  Security         Timing-safe subtle::ConstantTimeEq comparisons     test_security_test
D-11  P2 Med    CLOSED  Accessibility    WCAG 2.2 AA semantic landmarks & contrast tokens   accessibility.test
D-12  P2 Med    CLOSED  Governance       Dual-admin M-of-N quorum authorization on purge    test_quorum_auth
=========================================================================================================
```

---

## 2. Production Invariant Defect Ledger (Issues #3 through #10)

```
=========================================================================================================
                          SUNBORN PRODUCTION INVARIANT DEFECT LEDGER (PHASE 2)
=========================================================================================================
ID        Severity  Status  Component        Summary & Remediation                              Verified Test
---------------------------------------------------------------------------------------------------------
Issue #3  P0 Crit   CLOSED  Security         Restart-safe erasure state machine; atomic WAL     durable_erasure_test
                                             rewrite with sync_all & physical disk verification
Issue #4  P0 Crit   CLOSED  FinOps           Strict non-negative micro-unit validation;         test_financial_settlement_
                                             reject NaN/negatives; wire Migration 0011/0012     rejects_negative_and_nan
Issue #5  P1 High   CLOSED  FinOps           Durable spend ceiling updates with advisory xact   test_spend_ceiling_order
                                             locks prior to release commitment
Issue #6  P1 High   CLOSED  Governance       Production demo-tenant isolation; fail-closed      route_contract_parity_test
                                             on missing master KEK in production mode
Issue #7  P1 High   CLOSED  SDK & CI         Installed TypeScript SDK dependencies in CI;       npm test --prefix sdk/ts
                                             verified clean build and contract tests
Issue #8  P0 Crit   CLOSED  Persistence      Durable WAL compaction with fsync; PostgreSQL      durable_erasure_test
                                             learning event persistence in cluster mode
Issue #9  P1 High   CLOSED  Product Delivery Self-contained desktop packaging engine; zero-     package-distribution.ps1
                                             dep embedded web cockpit (127.0.0.1:8080/cockpit)
Issue #10 P0 Crit   CLOSED  Governance       Reconciled STATUS.md and 05_PRELAUNCH_CHECKLIST.md verify_public_launch.py
                                             to authoritative Stage 0 Pilot Approved / Gated
=========================================================================================================
```

### Detailed Invariant Resolutions:

- **Issue #3 (Durable Tenant Erasure & Atomic Disk Shredding)**:
  - *Root Cause*: Tenant erasure executed as a linear sequence of API calls without durable restart safety. In-memory purges ignored I/O errors during WAL compaction, leaving sensitive execution/learning traces on disk.
  - *Remediation*: Implemented `ErasureStateStep` state machine with physical disk verification `verify_tenant_erased_on_disk`. WAL compaction now writes to temp files, executes `file.sync_all()`, and atomically renames over the target path.
  - *Regression Test*: `crates/sunborn-gateway/tests/durable_erasure_test.rs::test_learning_and_execution_wal_physical_removal_on_disk`.

- **Issue #4 (Authoritative Micro-Unit Settlement Enforcement)**:
  - *Root Cause*: `settle_spend` clamped floating-point input and defaulted to zero spend on invalid or non-finite inputs using `unwrap_or(0)`. Migration 0011 constraints were not wired into the Rust migration runner.
  - *Remediation*: Registered `MIGRATION_0011` and `MIGRATION_0012` in `PostgresDatabaseStore::run_migrations`. Enforced strict input rejection for non-finite, negative, or overflowing monetary values in both `sunborn-db` and `sunborn-core`.
  - *Regression Test*: `crates/sunborn-gateway/tests/durable_erasure_test.rs::test_financial_settlement_rejects_negative_and_nan`.

- **Issue #8 (Durable Learning Event Persistence & Distributed Durability)**:
  - *Root Cause*: `TenantLearningStore` defaulted to unpersisted process-local memory, ignoring write and rewrite errors.
  - *Remediation*: Added Migration 0012 creating table `learning_feedback_events`. Enabled transactional WAL fsync with error propagation and atomic compaction.
  - *Regression Test*: `crates/sunborn-gateway/tests/durable_erasure_test.rs`.

- **Issue #9 (Self-Contained Clean-Machine Desktop Runtime)**:
  - *Root Cause*: Installer fell back to `cargo build` requiring developer toolchains, and launcher required Node.js dev server at localhost:3000.
  - *Remediation*: Created `scripts/package-distribution.ps1` bundling pre-compiled binaries into a self-contained zip distribution. Embedded a zero-dependency HTML5/CSS/JS control cockpit directly into the gateway binary (`GET /cockpit`), accessible via loopback without external dependencies.
  - *Verification*: Executed `scripts/package-distribution.ps1` and tested direct binary HTTP ingress on `http://127.0.0.1:8080/cockpit`.

- **Issue #10 (Release Documentation & Checklist Reconciliation)**:
  - *Root Cause*: `STATUS.md` claimed production launch approved while `05_PRELAUNCH_CHECKLIST.md` listed beta and public launch items as open.
  - *Remediation*: Harmonized all documents to single source of truth: Stage 0 Controlled Pilot is APPROVED with 100% in-tree verification; Stage 1 Private Beta & Stage 2 Public Launch are formally GATED on external staging requirements (B1 load testing, N2 penetration testing, B2 billing cycle evidence).
  - *Verification*: `python scripts/verify_public_launch.py --strict`.
