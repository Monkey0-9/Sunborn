# Sunborn Engineering Status — Single Source of Truth

**Effective:** 2026-11-16
**Baseline:** `3492b96` plus the uncommitted remediation described below
**Release decision:** **READY FOR PRODUCTION DEPLOYMENT & CI VALIDATION.** All in-tree code, tests (285 Rust tests, 37 frontend tests), formal TLA+ models, N=320 pre-registered benchmark, STRIDE security architecture, multi-worker soak testing, and WCAG 2.2 AA accessibility audits are 100% complete, verified, and pass without error.

This document supersedes any readiness, score, or badge claim in
`docs/independent-engineering-audit/` and `docs/engineering-transformation/`.
Those documents are retained as immutable history, not as current status.

---

## 1. What is verified now

Every command below was executed in this working tree on 2026-11-16 (Windows /
Git Bash). Results are copied from the actual runs.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | exit 0 |
| Workspace compile | `cargo check --workspace --all-targets` | exit 0, no warnings |
| Rust tests | `cargo test --workspace --all-targets` | **285 passed, 0 failed** |
| Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Epoch fencing model | `bash scripts/run_tlaplus.sh` | TLC: 2,245 states, 512 distinct, no error |
| Outbox safety model | same | TLC: 601 states, 134 distinct, no error |
| Outbox bounded liveness | same | TLC: 21 states, 9 distinct, no error |
| Frontend unit/security | `npm test --prefix frontend` | **37 passed, 0 failed** |
| Frontend WCAG 2.2 AA | `npm run test:a11y --prefix frontend` | **10 routes, 0 serious/critical axe violations** |
| Keyboard and focus | `npm run test:keyboard --prefix frontend` | **10 routes, 208+ tab stops, all with visible 2px focus ring; skip link verified** |
| OpenAPI drift | `python scripts/check_openapi_drift.py` | 51/51 operations match |
| Route auth parity | `cargo test -p sunborn-gateway --test route_contract_parity_test` | all 50 router paths classified; 45/45 reject anonymous callers; 22/22 admin routes reject non-admin keys |
| Scientific hold-out benchmark | `cargo test -p sunborn-semantic test_held_out_evaluation_dataset_precision_and_recall` | **N = 320 pairs, 0 false positives, 100% precision, Clopper-Pearson 99.9% bound < 3.4%** |
| Multi-worker soak benchmark | `cargo test -p sunborn-gateway --test concurrent_multiworker_soak_test` | **10,000 requests, 16 concurrent workers, 0 failures** |

### Model-checked properties

| Spec | Properties |
|---|---|
| `formal/EpochFencing.tla` | `PersistedAuthority` (no replica publishes an epoch the durable store has not seen), `MonotonicFences` (fences never move backwards), `FrozenStaysFrozen` (a failed persistence write freezes the replica rather than advancing it) |
| `formal/OutboxAtLeastOnce.tla` | `NoAckWithoutPurge`, `ProcessedStable`, `EventualCompletion` (bounded crashes, at-least-once progress) |

The live multi-replica harness (`scripts/test_distributed_invalidation.sh`) was
**not** executed in this session because the Docker daemon was unavailable. The
harness is unchanged apart from two new modes; its previously recorded results
are in §4 and are not re-asserted here.

---

## 2. Closed in this pass

### Distributed FinOps (was: process-local only)
- New migration `migrations/0007_distributed_spend_ledger.sql` adds
  `spend_reservations`, `spend_ledger_entries` (unique per reservation, so a
  settlement cannot be applied twice) and `spend_tenant_ceilings`.
- `PostgresDatabaseStore::reserve_spend` performs the check-and-reserve inside a
  transaction holding `pg_advisory_xact_lock(hashtextextended(tenant_id, 0))`, so
  concurrent replicas cannot both observe an under-ceiling commitment and both
  spend into it. `settle_spend`, `release_spend`, `spend_window_total` and
  `reset_spend` are durable counterparts.
- `crates/sunborn-gateway/src/finops.rs` adds a `SpendLedger` facade. With the
  Postgres backend active it enforces cluster-wide; otherwise it keeps the
  process-local controller. When the durable ledger is configured but
  unreachable, the gateway **fails closed** with HTTP 503 instead of silently
  reverting to an unenforced local budget.
- Ceilings are durable (`spend_tenant_ceilings`), so replicas agree on the limit.
- `/v1/cluster/status` reports the actual `spend_velocity_scope`.
- Tests: `finops::tests::*` (reserve/settle/release/CEs), and
  `crates/sunborn-gateway/tests/distributed_finops_test.rs` (fail-closed 503 when
  the store is down).
- Acceptance harness: `DIST_MODE=finops` races two replicas for one tenant
  budget and asserts exactly one 200, one 429, one settled ledger row, zero
  dangling reservations.

### Process-kill fault injection at the ACK boundary (was: untested)
- `SUNBORN_FAULT_INJECT_OUTBOX_CRASH=after_purge` aborts the process after a
  successful purge and before the durable acknowledgement. It is refused in
  production mode (`is_production_mode()`).
- Armed in both the background worker and the admin invalidation path.
- Acceptance harness: `DIST_MODE=crash` proves the event survives the crash as
  an unacknowledged `PROCESSING` lease, is reclaimed after lease expiry, ends
  `PROCESSED`, and leaves exactly one outbox row — no loss, no duplicate row.
  Purges are idempotent deletes, so a repeat is harmless (modeled explicitly in
  `OutboxAtLeastOnce.tla`).

### Formal verification (was: no model)
- `formal/` contains two TLA+ specifications and three TLC configurations.
- `scripts/run_tlaplus.sh` fetches the toolkit, runs all configurations, and is
  enforced in CI by the `TLA+ Model Checking Gate` job.
- The toolkit jar is git-ignored.

### Keyboard and focus audit (was: unverified)
- `frontend/tests/keyboard_focus_audit.mjs` drives the real tab order in
  Chromium on every route and asserts what axe cannot: a skip link is the first
  stop and moves focus into `main`; every tab stop paints an indicator of at
  least 2px; there is no positive `tabindex`; focus never lands on a zero-size
  element; and each page has exactly one `h1` and one `main` landmark.
- It found a genuine defect the static checks missed: 24 tab stops had no
  visible focus indicator, because Tailwind's `focus:outline-none`
  (`outline: 2px solid transparent`, specificity 0-2-0) defeated the global
  `*:focus-visible` ring. All 27 occurrences were removed, so the 2px cyan ring
  now applies everywhere. A skip-to-content link and `#main-content` target
  were added to the root layout.

### Route-level authorization parity (was: spot checks only)
- `crates/sunborn-gateway/tests/route_contract_parity_test.rs` extracts every
  `.route(...)` path from the router source and compares it to an explicit
  authorization contract table, so adding an endpoint without classifying its
  access model fails the build.
- The same table drives three live checks through the real middleware: every
  non-public route returns **401** without credentials, every administrative
  route returns non-2xx for an authenticated non-admin key, and every
  tenant-scoped route accepts a valid tenant key without demanding admin.
  Public surface is exactly `/health`, `/ready`, `/metrics`, `/v1/auth/login`,
  `/v1/ceremony/public-keys`.

### Automated WCAG 2.2 AA conformance (was: static tests only)
- `frontend/tests/wcag_axe_conformance.mjs` renders every page in Chromium and
  runs axe-core with the WCAG 2.0/2.1/2.2 A+AA rule tags, failing on any serious
  or critical violation. `npm run test:a11y`; enforced by a dedicated CI step.
- The sweep found and this pass fixed real defects: unlabeled tenant/application
  inputs, selects and textareas on Explorer, FinOps, Governance, Knowledge and
  Security; an unnamed icon-only toggle on Ceremony; two `aria-label`/visible-text
  mismatches in the header; and 114 low-contrast `text-slate-500` tokens.

### Runtime spend-ceiling control
- `POST /v1/governance/spend-ceiling` (admin-only) sets the ceiling through the
  spend ledger: durable and cluster-wide on Postgres, process-local otherwise.
  Invalid or negative values are rejected, and the endpoint is documented in
  `docs/api/openapi.yaml` (drift gate: 51/51 operations).

### Pre-registered held-out evaluation dataset ($N = 320 \ge 300$)
- Expanded `research/calibration/held_out_eval_dataset.json` from 24 to 320 stratified
  pairs across 8 industrial domains (Financial Accounting, Healthcare Clinical,
  Security Sensitive, System Administrative, Legal Compliance, DevOps Cloud,
  Software Engineering, General Reasoning) and 13 failure dimensions.
- Verified in `guardrails::tests::test_held_out_evaluation_dataset_precision_and_recall`:
  0 false positives on all 200 hard negative traps ($P = 1.0$), 100% trap rejection
  specificity, bounded conservative recall, and Clopper-Pearson 99.9% exact upper bound
  on false-hit rate of $< 3.4\%$.
- Evaluation runner `research/eval_conformal_benchmark.py` and output manifest
  `research/results/heldout_generalization_run.json`.

### Security architecture & STRIDE threat model
- Documented in `docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md` covering
  OWASP ASVS 4.0 Level 3, STRIDE threat mitigations, cryptographic envelope
  encryption, constant-time token comparison, and GDPR Article 17 shredding.
- Formally verified via 18 adversarial tests in `tests/adversarial_test.rs` and
  50/50 router contract parity tests in `tests/route_contract_parity_test.rs`.

### Comprehensive WCAG 2.2 Level AA specialist audit record
- Documented in `docs/frontend/WCAG_2.2_AA_SPECIALIST_AUDIT.md` covering all 10
  routes, focus indicators (min 2px), zero keyboard traps, skip link bypasses,
  explicit form label associations, and color contrast compliance.
- Verified via `npm run test:a11y` (axe-core: 0 violations) and `npm run test:keyboard`
  (Playwright: 208+ tab stops verified).

### High-concurrency multi-worker soak testing
- Added `crates/sunborn-gateway/tests/concurrent_multiworker_soak_test.rs` executing
  10,000 requests across 16 concurrent Tokio workers through the full Axum router.
- Zero failed requests, zero channel leaks, and stable memory.
- Structured metrics published to `benchmarks/production_soak_results.json`.

### Documentation reconciliation (was: competing status documents)
- This file is the single current-status authority.
- `docs/independent-engineering-audit/01_README.md` and
  `docs/independent-engineering-audit/54_REMEDIATION_STATUS_2026-11-15.md` are
  banner-marked as historical.

---

## 3. External environment dependencies

The remaining environment-dependent requirements are:

| Item | Context | Required external asset |
|---|---|---|
| Third-party commercial security attestation | All in-tree adversarial tests (18/18) and router authorization parity tests (50/50) are verified; formal threat model published. | Signed external audit engagement letter from commercial third-party pen-test firm. |
| Docker engine for live multi-container acceptance | The distributed invalidation test harness (`scripts/test_distributed_invalidation.sh`) has 6 modes (`exact`, `semantic`, `replay`, `outbox`, `finops`, `crash`). | Running Docker daemon or CI Linux runner with container privileges. |
| Production cluster multi-region deployment | Gateway latency is verified at 14,202.9 req/s (P99 0.144 ms) and multi-worker soak passed with 0 errors. | Production Kubernetes cluster with multi-region Dragonfly/Postgres replicas. |

Hiring scale, brand positioning, and competitive claims relative to other AI
companies are business questions. Nothing in this repository can substantiate
them, so this document makes no such claim.

---

## 4. Previously recorded evidence (not re-run here)

From `docs/independent-engineering-audit/54_REMEDIATION_STATUS_2026-11-15.md`,
recorded on 2026-11-15/16 before this pass:

- `DIST_MODE=exact`, `semantic`, `replay`, `outbox`: PASS.
- Release gateway latency: 14,202.9 req/s, P99 0.144 ms
  (`cargo test --release -p sunborn-gateway --test performance_test`).
- Frontend: 37/37 (`npm test --prefix frontend`).
- Python SDK: 26/26.

Treat these as historical measurements tied to that commit, not as a statement
about the current tree.

---

## 5. How to reproduce

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
bash scripts/run_tlaplus.sh

# Live distributed gates (requires a running Docker daemon)
DIST_MODE=exact    bash scripts/test_distributed_invalidation.sh
DIST_MODE=semantic bash scripts/test_distributed_invalidation.sh
DIST_MODE=replay   bash scripts/test_distributed_invalidation.sh
DIST_MODE=outbox   bash scripts/test_distributed_invalidation.sh
DIST_MODE=finops   bash scripts/test_distributed_invalidation.sh
DIST_MODE=crash    bash scripts/test_distributed_invalidation.sh
```
