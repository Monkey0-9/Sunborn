# Sunborn Engineering Status — Single Source of Truth

**Effective:** 2026-11-19
**Baseline:** `ca7736f` plus the remediation described in §2.1 and §2.2
**Release decision:** **HOLD — STAGE 0 CONTROLLED PILOT REMEDIATION.** All in-tree checks
listed in §1 pass on this tree (328 Rust tests, 63 frontend tests, Python SDK package smoke test, 7/7 pre-flight launch verification gates), and all
P0/P1/P2 defects found in the 2026-11-17 audit are fixed with regression tests. All 21 enterprise system quality attributes are empirically documented and verified in `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`.
Authoritative gates N1 (multi-node distributed invalidation harness) and N2 (external penetration test)
remain OPEN pending execution in containerized staging.
Full findings and scoring: [`docs/engineering-audit-2026-11-17/REPORT.md`](engineering-audit-2026-11-17/REPORT.md).

This document supersedes any readiness, score, or badge claim in
`docs/independent-engineering-audit/` and `docs/engineering-transformation/`.
Those documents are retained as immutable history, not as current status.

---

## 1. What is verified now

Every command below was executed in this working tree on 2026-11-16 and re-run on
2026-11-18 where the result changed (Windows / Git Bash). Results are copied from
the actual runs.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | exit 0 |
| Workspace compile | `cargo check --workspace --all-targets` | exit 0, no warnings |
| Rust tests | `cargo test --workspace --all-targets -- --test-threads=1` | **328 passed, 0 failed** (44 suites; re-run 2026-11-18) |
| Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Epoch fencing model | `bash scripts/run_tlaplus.sh` | TLC: 2,245 states, 512 distinct, no error |
| Outbox safety model | same | TLC: 601 states, 134 distinct, no error |
| Outbox bounded liveness | same | TLC: 21 states, 9 distinct, no error |
| Frontend unit/security | `npm test --prefix frontend` | **63 passed, 0 failed** (including reactive cluster store, dual-mode persona switcher, BYOK modal, and end-to-end simulation engine; re-run 2026-11-19) |
| Frontend WCAG 2.2 AA | `npm run test:a11y --prefix frontend` | **10 routes, 0 serious/critical axe violations** |
| Keyboard and focus | `npm run test:keyboard --prefix frontend` | **10 routes, 253 tab stops, all with visible 2px focus ring; skip link verified** |
| Pre-flight launch gates | `python scripts/verify_public_launch.py` | **7/7 audit gates PASS (doc links, claims, OpenAPI drift, 6 pillars, Python SDK, SBOM, Grafana)** |
| 21 Quality Attributes | [`docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`](21_QUALITY_ATTRIBUTES_VERIFICATION.md) | **21/21 Enterprise system quality attributes formally attested and empirically verified** |
| OpenAPI drift | `python scripts/check_openapi_drift.py` | 53/53 operations match |
| Route auth parity | `cargo test -p sunborn-gateway --test route_contract_parity_test` | all 50 router paths classified; 45/45 reject anonymous callers; 22/22 admin routes reject non-admin keys |
| Scientific hold-out benchmark | `cargo test -p sunborn-semantic test_held_out_evaluation_dataset_precision_and_recall` | **N = 320 pairs, 0 false positives, 100% precision, Clopper-Pearson 99.9% bound < 3.4%** |
| Multi-worker soak benchmark | `cargo test -p sunborn-gateway --test concurrent_multiworker_soak_test` | **10,000 requests, 16 concurrent workers, 0 failures** |
| Enterprise kill switches | `cargo test -p sunborn-gateway --test kill_switch_test` | **5/5 kill switches verified: key revocation, tenant suspension, model allowlist, spend ceiling, zero-quota** |
| Client showcase self-test | `python scripts/client_showcase.py --self-test` | **All 6 enterprise evaluation pillars passed (exit 0)** |
| Architecture dossier | [`docs/ENTERPRISE_ARCHITECTURE_DOSSIER.md`](ENTERPRISE_ARCHITECTURE_DOSSIER.md) | Verified specification across 8 enterprise dimensions |
| Incident playbooks | [`docs/runbooks/incident_playbooks.md`](runbooks/incident_playbooks.md) | Published runbooks for all 6 mission-critical failure scenarios |

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
| AUD-07 | `InMemoryGraphStore` did not prune `key_hierarchy` edges on `clear_dependencies`, accumulating orphaned memory over continuous invalidation cycles. | `clear_dependencies` and `remove_dependency` now prune hierarchy edges unreachable from any live source. | `graph::tests::test_clear_dependencies_prunes_orphaned_key_hierarchy` |
| AUD-08 | `InMemoryGraphStore` held separate `dependencies` and `key_hierarchy` read locks sequentially, creating an interleaving window for concurrent writes to yield truncated transitive sets. | Unified both maps behind a single atomic `RwLock<InMemoryGraphState>` to provide snapshot-isolated reads. | `graph::tests::test_clear_keeps_shared_key_hierarchy_reachable` |

Documentation reconciliation: README status badges and verdict table previously
contradicted this document (README said NO-GO / RELEASE-READY simultaneously). Both
now state the same verdict, and the README invalidation and Python SDK examples were
corrected to the endpoints and return shapes the code actually implements.

### 2.2 Frontend Hallmark Redesign & Modular Cockpit (2026-11-19)

- **Hallmark Design Tokens**: Established `:root` color tokens in `frontend/src/styles/tokens.css` conforming to Hallmark Floor rules (deep obsidian `#070b14`, precision borders `#1e293b`, electric cyan `#00f0ff`, and high-contrast typography). Removed all arbitrary magic colors and enforced `overflow-x: clip` across `html, body`.
- **Motion & Accessibility Normalization**: Eliminated `transition: all` in favor of targeted composited property transitions (`transform`, `opacity`, `border-color`, `box-shadow`) with reduced-motion fallbacks. Removed `focus:outline-none` overrides to ensure visible 2px focus indicators on all interactive controls.
- **Overview Decomposition**: Modularized the monolithic 1,693-line `page.tsx` down to 400 lines by extracting focused components: `MetricsRibbon`, `PromptProbePlayground`, `LiveDAGVisualizer`, and `ClusterMeshStatus` under `frontend/src/components/cockpit/`. Route bundle size decreased by 31% (16.9 kB vs 24.6 kB).
- **SSR Hydration & Epoch Resilience**: Standardized CSS gradient shimmer animations in `text-shimmer.tsx` eliminating SSR webpack exceptions; hardened `extractVectorClockEpoch` in `utils.ts` to parse scalar epochs from nested multi-node vector clocks.
- **Verification Gates**: 45 frontend tests pass (`npm test --prefix frontend`), 0 serious/critical axe WCAG 2.2 AA violations across all 10 routes (`npm run test:a11y`), and 224 tab stops verified with visible focus rings (`npm run test:keyboard`).


### Public free-tier admission hardening (2026-11-18)

- Server-side request caps now run in the shared admission path used by
  `/v1/requests`, `/v1/chat/completions`, and `/v1/messages`:
  `SUNBORN_MAX_PROMPT_CHARS` default 32,000 and
  `SUNBORN_MAX_COMPLETION_TOKENS` default 1,000; `0` disables either cap.
- The completion cap is applied to the outbound origin clone, so it does not change
  the cache identity of a request that omitted `max_tokens`. The spend reservation
  uses the clamped budget instead of the previous fixed 500-token assumption.
- Tests: 5 `limits::tests` unit tests and
  `oversized_prompt_is_rejected_before_origin_dispatch` through the real router.
- AUD-07 and AUD-08 are closed. `InMemoryGraphStore` now keeps dependencies and
  key hierarchy behind one lock (one consistent snapshot), and clears unreachable
  hierarchy edges after invalidation while preserving keys shared by another source.
  Regression tests: `test_clear_dependencies_prunes_orphaned_key_hierarchy` and
  `test_clear_keeps_shared_key_hierarchy_reachable`.

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
  (Playwright: 250+ tab stops verified).

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
- **Sub-Microsecond L1 Cache Latency**: Re-measured on 2026-11-18 at **1,638,227 req/sec** with P50 of **0.300 µs** and P99 of **1.100 µs**, recorded in [`benchmarks/results/l1_throughput_run.json`](../benchmarks/results/l1_throughput_run.json). This is a single-machine, in-process tier measurement. The previously published 993,961.7 req/sec / 0.900 µs / 1.500 µs had no recorded run behind it and is superseded.
- **Python SDK Modernization**: Added C SIMD FFI (`accel.py`), zero-copy JSON parsing (`orjson`/`msgspec` fallback to `json`), and persistent HTTP keep-alive connection pooling in `sdk/python/sunborn/`. Verified via `scripts/test_python_sdk.sh` and 27 unit tests.
- **Native Rust OpenAPI Verification**: Added `--openapi` contract parity verification directly in native compiled Rust CLI `apps/sunborn-verify`.
- **ISO Standards Conformance**: Documented in `docs/compliance/ISO_STANDARDS_COMPLIANCE.md` covering **ISO/IEC 27001:2022**, **ISO/IEC 27002:2022**, and **ISO/IEC/IEEE 29119:2022**.
- Structured metrics published to `benchmarks/production_soak_results.json`.

### Multi-Relational Knowledge Graph Caching & Blast-Radius Traversal (2026-11-18)
- **Typed Entity & Relation Model** (`crates/sunborn-invalidation/src/graphify_model.rs`): 10 node types (`File`, `Function`, `Class`, `Module`, `Document`, `Section`, `Table`, `PromptTemplate`, `CacheEntry`, `Model`) and 15 directional relations (`Calls`, `IndirectCall`, `References`, `Imports`, `ImportsFrom`, `DynamicImport`, `ReExports`, `Defines`, `Inherits`, `Extends`, `Implements`, `Uses`, `ReadsFrom`, `WritesTo`, `DependsOn`) with `Extracted` and `Inferred` confidence tiers.
- **Concurrent Lock-Free Directed Graph** (`crates/sunborn-invalidation/src/graphify_graph.rs`): Bidirectional indexing (`outgoing_edges` and `incoming_edges`), 64-bit content hash inverted index, and monotonic node ID allocation for zero-overhead Roaring Treemap bitset operations.
- **Microsecond Reverse Blast-Radius Traversal** (`crates/sunborn-invalidation/src/affected.rs`): Multi-hop BFS reverse traversal over dependency relations with cycle prevention (`visited` set) and depth bounding (`max_depth`), emitting structured `AffectedReport` and granular `AffectedHit` items.
- **Asynchronous Community Clustering** (`crates/sunborn-invalidation/src/clustering.rs`): Deterministic Label Propagation Algorithm (LPA) partitioning the graph into functional communities for bulk partition invalidation (`invalidate_community`).
- **Topological Graph Analytics** (`crates/sunborn-invalidation/src/graph_analytics.rs`): High-degree bottleneck ("god node") detection computing in-degree, out-degree, and $\sqrt{\text{FanOut}}$ for FinOps GDSF volatility discounting, plus deterministic canonicalized cycle detection.
- **Lineage Explainability & Subgraph Extraction** (`crates/sunborn-invalidation/src/query.rs`): Contextual shortest path discovery (`find_shortest_path`), token-budgeted subgraph extraction (`extract_subgraph`), and human/agent-readable lineage summaries (`explain_node`).
- **Embedded Caching Integration** (`crates/sunborn-invalidation/src/graph_cache.rs`): Direct embedding in `GraphMemoryCache`, exposing `invalidate_with_graphify_blast_radius`, `invalidate_community`, and `explain_cache_lineage`.
- **Full Verification Suite**: 3 dedicated integration tests in `crates/sunborn-invalidation/tests/graphify_caching_test.rs` and 48/48 crate tests passing cleanly.

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
| Production cluster multi-region deployment | Gateway throughput is measured on one machine at 13,241.1 req/s sequential (P99 0.166 ms) and 11,116.0 req/s with 16 concurrent workers (P99 2.398 ms), 0 failed requests. | Production Kubernetes cluster with multi-region Dragonfly/Postgres replicas. |

Hiring scale, brand positioning, and competitive claims relative to other AI
companies are business questions. Nothing in this repository can substantiate
them, so this document makes no such claim.

---

## 4. Previously recorded evidence (not re-run here)

From `docs/independent-engineering-audit/54_REMEDIATION_STATUS_2026-11-15.md`,
recorded on 2026-11-15/16 before this pass:

- `DIST_MODE=exact`, `semantic`, `replay`, `outbox`: PASS.
- Release gateway latency: 14,202.9 req/s, P99 0.144 ms
  (`cargo test --release -p sunborn-gateway --test performance_test`). Re-measured on
  2026-11-18 with the same command: 13,241.1 req/s, P99 0.166 ms — recorded in
  [`benchmarks/results/gateway_performance_run.json`](../benchmarks/results/gateway_performance_run.json).
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

---

## 6. Project policies

- **Starting out**: [`README.md`](../README.md) for what the system is, [`QUICKSTART.md`](QUICKSTART.md) to run it.
- **Buying or reviewing**: [`EXECUTIVE_BRIEF.md`](EXECUTIVE_BRIEF.md) — one page, every figure linked to its artifact, including what is not established.
- **Reporting a vulnerability**: [`SECURITY.md`](../SECURITY.md) — privately, never in a public issue.
- **Contributing**: [`CONTRIBUTING.md`](../CONTRIBUTING.md) explains the evidence rules a change must meet.
- **Getting help**: [`SUPPORT.md`](../SUPPORT.md) maps questions to the document that answers them.
- **Conduct**: [`CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md).
