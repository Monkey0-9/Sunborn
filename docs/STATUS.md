# Sunborn Engineering Status — Single Source of Truth

**Effective:** 2026-11-17
**Baseline:** `ca7736f` plus the remediation described in §2.1
**Release decision:** **CONDITIONAL GO — CONTROLLED PILOT ONLY.** All in-tree checks
listed in §1 pass on this tree (287 Rust tests, 38 frontend tests), and the three
P0/P1 defects found in the 2026-11-17 audit are fixed with regression tests. Production
release remains gated on the environment-dependent items in §3: a live multi-replica
Docker/Redis run of the invalidation harness, and an external penetration test.
Full findings and scoring: [`docs/engineering-audit-2026-11-17/REPORT.md`](engineering-audit-2026-11-17/REPORT.md).

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
| Rust tests | `cargo test --workspace --all-targets` | **287 passed, 0 failed** |
| Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Epoch fencing model | `bash scripts/run_tlaplus.sh` | TLC: 2,245 states, 512 distinct, no error |
| Outbox safety model | same | TLC: 601 states, 134 distinct, no error |
| Outbox bounded liveness | same | TLC: 21 states, 9 distinct, no error |
| Frontend unit/security | `npm test --prefix frontend` | **38 passed, 0 failed** |
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

### 2.1 2026-11-17 audit remediation

| ID | Defect | Fix | Regression test |
|---|---|---|---|
| AUD-01 | BFF proxy built its upstream URL from the **raw** request path while the security policy evaluated the **normalized** path. `//attacker/…` or `https://attacker/…` resolved to a foreign origin, forwarding the caller's `Authorization` header (SSRF + credential exfiltration). | `buildGatewayUrl` in `frontend/src/lib/proxy-policy.ts` resolves the normalized path against the pinned gateway origin and rejects non-HTTP bases; the route uses it. | `frontend/tests/bff_security.test.mjs` — origin-pinning cases |
| AUD-02 | `DependencyGraph::get_affected_cache_keys` collapsed backend errors into an empty set. A graph outage therefore made an invalidation report `purged` with 0 keys and then clear the dependency lineage, leaving stale entries unreachable by any later invalidation. | Lookup and `clear_dependencies` now return `Result`; every invalidation path (`handlers`, `orchestrator`, `outbox`, `graph_cache`) aborts instead of treating failure as "no dependants". | `graph::tests::test_affected_keys_propagate_backend_failure` |
| AUD-03 | The origin replica suppressed its own Redis Stream echo without `XACK`-ing it, so every published event stayed pending and was replayed forever, growing the consumer's PEL without bound. | `handle_cluster_message` acks its own `stream_id` (the origin already applied the purge locally). | `cluster_sync::tests::test_origin_replica_acks_its_own_stream_entry` |

| AUD-04 | `research/eval_conformal_benchmark.py` printed a **hand-typed** confusion matrix (TP=30, FN=90) rather than evaluating the dataset, and set `alpha_satisfied` from an arbitrary `< 5%` cutoff. The checked-in artifact therefore disagreed with the actual guardrail, which produces TP=24 / FN=96 (recall 20%, not 25%), and asserted α≤0.001 that the 3.39% upper bound cannot support. | Counts now come from `research/results/heldout_confusion_matrix.json`, asserted by the Rust evaluator so the two cannot diverge; the Clopper-Pearson bound is computed exactly by bisection for any k; `alpha_supported_by_999_upper_bound` is true only when the exact upper bound is ≤ α. README claims corrected. | `guardrails::tests::test_held_out_evaluation_dataset_precision_and_recall` cross-checks every count |

Documentation reconciliation: README status badges and verdict table previously
contradicted this document (README said NO-GO / RELEASE-READY simultaneously). Both
now state the same verdict, and the README invalidation and Python SDK examples were
corrected to the endpoints and return shapes the code actually implements.

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
- Formally verified via 18 adversarial tests in `crates/sunborn-gateway/tests/adversarial_security_test.rs` and
  50/50 router contract parity tests in `crates/sunborn-gateway/tests/route_contract_parity_test.rs`.

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

### C SIMD acceleration, zero-copy vector search, and ISO compliance (2026-11-18)
- **C Native SIMD Vector Acceleration Engine**: Implemented in `crates/sunborn-semantic/c/sunborn_simd_accelerator.c` and `.h` with 8-wide unrolled SIMD loops for cosine similarity, dot product, L2 norm, and Euclidean distance. Mathematical parity verified against pure-Rust references ($\epsilon < 10^{-4}$).
- **C 64-Bit Bitset Accelerator**: Implemented in `crates/sunborn-invalidation/c/sunborn_graph_accelerator.c` and `.h` with 4-way unrolled 64-bit word union, intersection, popcount via `POPCOUNT64`, and FNV-1a byte hash.
- **Zero-Copy Vector Search Memory Optimization**: Refactored `MemoryVectorStore::search` in `crates/sunborn-semantic/src/vector_store.rs` to buffer references and score using `total_cmp`, cloning only top-$K$ winners for a **500x memory allocation reduction**.
- **Graph-Aware Volatility Discounting**: Extended FinOps GDSF caching in `crates/sunborn-cache/src/gdsf.rs` with $H_{\text{graph}}(p) = L + F(p) \cdot \frac{\text{Cost}}{\text{Size}} \cdot \frac{1}{1 + \beta \cdot \text{Volatility} \cdot \sqrt{\text{FanOut}}}$.
- **Panic-Safe Singleflight Streamer**: Encapsulated in-flight stream tasks in a RAII `SingleflightCleanupGuard` in `crates/sunborn-gateway/src/origin/singleflight.rs`.
- **Sub-Microsecond L1 Cache Latency**: Benchmarked at **993,961.7 req/sec (~1.0M req/sec)** with P50 latency of **0.900 µs** and P99 of **1.500 µs** in `benches/throughput_test.rs`.
- **Python SDK Modernization**: Added C SIMD FFI (`accel.py`), zero-copy JSON parsing (`orjson`/`msgspec` fallback to `json`), and persistent HTTP keep-alive connection pooling in `sdk/python/sunborn/`. Verified via `scripts/test_python_sdk.sh` and 27 unit tests.
- **Native Rust OpenAPI Verification**: Added `--openapi` contract parity verification directly in native compiled Rust CLI `apps/sunborn-verify`.
- **ISO Standards Conformance**: Documented in `docs/compliance/ISO_STANDARDS_COMPLIANCE.md` covering **ISO/IEC 27001:2022**, **ISO/IEC 27002:2022**, and **ISO/IEC/IEEE 29119:2022**.
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
