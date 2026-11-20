# Sunborn End-to-End Audit Remediation & Product Delivery Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all 8 GitHub audit issues (#3 to #10), implement durable execution and learning persistence, deliver a consumer-facing local BYOK installer with native OS credential vault storage, fix TypeScript SDK CI, reconcile all public claims with strict evidence classifications, and pass the master system verification pipeline.

**Architecture:** Multi-phased release strategy enforcing fail-closed security, exact 64-bit micro-unit integer financial ledger calculations, durable write-before-local consistency, process-isolated local runtime with Windows Credential Manager / SecretService DPAPI integration, and transparent evidence demarcation between in-tree formal verification and staging operational gates.

**Tech Stack:** Rust 1.80+ (Axum, Tokio, SQLx, Ring, AES-256-GCM), Next.js 15 (React 19, Tailwind CSS), TypeScript SDK, Python 3.10+, PowerShell / Bash, Windows Credential API / DPAPI, PostgreSQL 16, DragonflyDB (Redis 7), Qdrant.

---

## Global Constraints

- **No Data Loss on Restart:** Features advertised as durable must persist across process and machine restarts. Ephemeral in-memory modes must clearly disclose data volatility.
- **Fail-Closed Security:** Deletion and cryptographic shredding must never swallow errors. Weak or demo keys must never be seeded in production (`SUNBORN_ENV=production`).
- **Zero Browser Secret Exposure:** Provider API keys must never be stored in browser `localStorage` or exposed over frontend HTTP responses; secrets must reside exclusively in OS native credential vaults or encrypted server state.
- **Micro-Unit Financial Precision:** All authoritative database spend reservations, ledger entries, and rolling-window totals must execute over fixed-precision 64-bit integer micro-units (`MicroUnits(i64)`, 1 USD = 1,000,000 micro-units).
- **Localhost Ingress by Default:** Personal and local desktop installations must bind strictly to `127.0.0.1`, not `0.0.0.0`.

---

## Review Focus

1. **Cryptographic shredding error propagation:** Tenant deletion request when KMS or memory key shredding fails must abort immediately with HTTP 500/404 and leave existing tenant records intact for audit/forensics.
2. **Durable spend ceiling write failure:** When database connection drops while setting a spend ceiling, local in-memory ceiling must remain unchanged at its prior value.
3. **Integer micro-unit arithmetic overflow & precision:** SQL aggregations must sum `amount_micro_units` and compare against `ceiling_micro_units` without floating-point cast drift.
4. **Local installer uninstallation cleanliness:** Uninstalling on Windows or Linux must cleanly purge application binaries while offering an interactive prompt or flag to preserve/archive local caches and credentials.
5. **Production readiness enforcement of durability:** A production startup with ephemeral execution store or ephemeral learning store must fail closed before opening ingress traffic.

---

## Detailed Task Breakdown

### Task 1: Fix Tenant Shredding Fail-Closed (#3)
- [ ] Modify `crates/sunborn-gateway/src/handlers.rs` to propagate error in `delete_tenant`.
- [ ] Update `crates/sunborn-gateway/tests/governance_test.rs` to provision keys before offboarding.
- [ ] Add regression test `test_tenant_cascade_offboarding_fails_when_key_shredding_fails`.
- [ ] Verify with `cargo test -p sunborn-gateway --test governance_test`.

### Task 2: Implement Micro-Unit Authoritative Spend Accounting (#4)
- [ ] Modify `crates/sunborn-db/src/postgres.rs` to use `amount_micro_units` and `ceiling_micro_units` in `reserve_spend`, `settle_spend`, and `spend_window_total`.
- [ ] Expose micro-unit typed methods on `PostgresDatabase`.
- [ ] Add unit test verifying exact integer accounting and zero floating-point drift.
- [ ] Verify with `cargo test -p sunborn-db`.

### Task 3: Enforce Durable Spend Ceilings First (#5)
- [ ] Modify `DistributedSpendLedger::set_ceiling` in `crates/sunborn-gateway/src/finops.rs` to commit to durable store before updating local state.
- [ ] Add regression test in `crates/sunborn-gateway/tests/spend_ceiling_api_test.rs` asserting local state consistency when durable write fails.
- [ ] Verify with `cargo test -p sunborn-gateway --test spend_ceiling_api_test`.

### Task 4: Remove Sample Tenant Provisioning from Production (#6)
- [ ] Modify `AppState::new()` in `crates/sunborn-gateway/src/state.rs` to prevent seeding sample demo tenants when `is_production_mode()` is true.
- [ ] Add test verifying that in production mode, demo tenants (`tenant-alpha`, etc.) are not provisioned.
- [ ] Verify with `cargo test -p sunborn-gateway --test security_test`.

### Task 5: Implement Durable Execution & Learning Stores (#8)
- [ ] Create `PersistentExecutionCacheStore` in `crates/sunborn-execution/src/wal_store.rs`.
- [ ] Update `AppState::verify_production_readiness()` to reject ephemeral execution & learning stores in production.
- [ ] Update `/ready` probe to report actual storage tiers.
- [ ] Add tests for persistence across restarts and readiness rejection.
- [ ] Verify with `cargo test -p sunborn-execution` and `cargo test -p sunborn-gateway`.

### Task 6: Secure Local Credential Vault & BYOK Runtime (#9)
- [ ] Create `crates/sunborn-identity/src/local_vault.rs` implementing OS credential storage with DPAPI/Keyring and secret redaction.
- [ ] Add local control endpoints in `crates/sunborn-gateway/src/handlers.rs` (`/v1/local/providers`, `/v1/local/status`, `/v1/local/config`).
- [ ] Update `crates/sunborn-core/src/config.rs` and `main.rs` to bind `127.0.0.1` by default for personal/local mode.
- [ ] Build Windows installer (`scripts/install-sunborn.ps1`), uninstaller (`scripts/uninstall-sunborn.ps1`), Linux installer (`scripts/install-sunborn.sh`), and desktop launcher (`scripts/sunborn-desktop.ps1`).
- [ ] Update README with 60-second consumer BYOK installer instructions.

### Task 7: Fix TypeScript SDK CI Workflow (#7)
- [ ] Add `npm ci --prefix sdk/typescript` to `.github/workflows/ci.yml`.
- [ ] Test TypeScript SDK build and test locally.

### Task 8: Align Quality-Attribute & Accessibility Claims (#10)
- [ ] Update `README.md`, `docs/STATUS.md`, `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`, and `docs/launch/05_PRELAUNCH_CHECKLIST.md`.
- [ ] Enforce strict evidence classification between in-tree tested vs staged/environment-dependent gates.
- [ ] Verify with `python scripts/check_launch_docs.py` and `python scripts/verify_public_launch.py`.

### Task 9: Master System Acceptance & Verification
- [ ] Execute `cargo test --workspace --all-targets`.
- [ ] Execute `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] Execute `npm test --prefix frontend`.
- [ ] Execute `npm test --prefix sdk/typescript`.
- [ ] Execute `powershell scripts/verify.ps1`.
- [ ] Generate comprehensive walkthrough artifact.
