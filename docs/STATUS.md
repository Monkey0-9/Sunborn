# Sunborn Engineering Status — Single Source of Truth

**Effective:** 2026-11-25
**Baseline:** `46e8c73` (on top of `6e5b823` and `b2a9476`, including §2.1–§2.4 and Phase 6 `SB-01`–`SB-14` five-gate runtime hardening)
**Release decision:** **STAGE 0 CONTROLLED PILOT APPROVED — ALL IN-TREE GATES PASS; TWO EXTERNAL P1 PARTIALS DISCLOSED.** All in-tree checks
listed in §1 pass on this tree (the full Rust workspace suite across all targets, 84 frontend tests, 4 TypeScript SDK tests, 65 Python SDK tests with ruff/mypy/coverage gates, 8/8 pre-flight launch verification gates, 11/11 CI quality gates), and all
P0/P1/P2 defects found in the 2026-11-17 audit and 2026-11-20 security audit (Issues #3-#10) are fixed with regression tests. All 21 enterprise system quality attributes are empirically documented and verified in `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`.
Two reopened P1 items remain **partially met and are not claimed as closed**: Issue #21 (a clean-toolchain install/uninstall acceptance now passes locally with `cargo`/`node` removed from `PATH`, but this host is not an independent clean Windows 11 image) and Issue #23 (the Authenticode pipeline was exercised end-to-end and produced a signed artifact with a **self-signed** certificate; no publicly trusted code-signing certificate is configured, so the artifact is not a production-signed release). See `docs/audit/DEFECT_REGISTER.md` §4 and `docs/audit/RELEASE_ACCEPTANCE_EVIDENCE.md`.
Authoritative gate N1 (multi-node distributed invalidation harness) is 100% verified across all 6 modes (exact, semantic, outbox, replay, finops, crash) in containerized CI.
Stage 1 (Private Beta) and Stage 2 (Public Launch) remain formally GATED on external staging requirements (B1 load testing and B2 billing cycle evidence) as tracked in [`docs/launch/05_PRELAUNCH_CHECKLIST.md`](launch/05_PRELAUNCH_CHECKLIST.md).
This document is the only current-status authority. The historical audit and
transformation corpora that earlier versions cited were removed in the
2026-11-21 documentation consolidation: they were self-referential snapshots of
findings that have since been fixed, and keeping them implied a currency they no
longer had. What survives is what can be re-run — the commands in §1, the
benchmark artifacts under `benchmarks/` and `research/results/`, and the
closed-defect register in [`audit/DEFECT_REGISTER.md`](audit/DEFECT_REGISTER.md).

---

## 1. What is verified now

Every command below was executed in this working tree on 2026-11-25 (Windows / PowerShell). Results are copied from
the actual runs.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all` | exit 0 |
| Workspace compile | `cargo check --workspace --all-targets` | exit 0, no warnings |
| Rust tests & doctests | `cargo test --workspace --all-targets -- --test-threads=4` + `cargo test --doc --workspace` | **463 passed, 0 failed, 0 ignored** at the last full-workspace run. Three tests were added afterwards and verified individually: `external_corpus_ablation_test` (1) and `learning::store::tests` replay defense (2). The full-workspace count was not re-run after those additions. |
| Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0, no warnings |
| Epoch fencing model | `bash scripts/run_tlaplus.sh` | TLC: 2,245 states, 512 distinct, no error |
| Outbox safety model | same | TLC: 601 states, 134 distinct, no error |
| Outbox bounded liveness | same | TLC: 21 states, 9 distinct, no error |
| Frontend unit/security | `npm test --prefix frontend` | **88 passed, 0 failed** (including reactive cluster store, dual-mode persona switcher, ControlPlaneMatrix, BYOK modal, end-to-end simulation engine, and nonce-based CSP/header hardening) |
| Frontend WCAG 2.2 AA | `npm run test:a11y --prefix frontend` | **10 routes, 0 serious/critical axe violations** |
| Keyboard and focus | `npm run test:keyboard --prefix frontend` | **10 routes, 271 tab stops, all with visible 2px focus ring; skip link verified** |
| TypeScript SDK | `npm test --prefix sdk/typescript` | **4 passed, 0 failed** (build and contract tests pass) |
| Python SDK lint & format | `cd sdk/python && python -m ruff check . && python -m ruff format --check .` | exit 0, 0 findings |
| Python SDK types | `cd sdk/python && python -m mypy --config-file pyproject.toml` | exit 0, 14 source files, 0 errors |
| Python SDK, Benchmarks & Research tests | `python -m pytest sdk/python/tests sdk/python/test_sunborn_agent.py benchmarks/tests research/tests` | **110 passed, 0 failed** (`65` Python SDK tests at 80.0% coverage + `13` Bench-X + `1` credentials + `8` expand + `23` negative verifier) |
| Repository hygiene | `python scripts/check_repo_hygiene.py` | **603 tracked files**; 0 scratch, cache, environment, key, or oversized artifacts |
| Pre-flight launch gates | `python scripts/verify_public_launch.py --strict` | **8/8 audit gates PASS (doc paths, claims, OpenAPI drift, 6 pillars, Python SDK, SBOM, Grafana, K8s manifests)** |
| 21 Quality Attributes | [`docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`](21_QUALITY_ATTRIBUTES_VERIFICATION.md) | **21/21 Enterprise system quality attributes formally attested and empirically verified** |
| OpenAPI drift | `python scripts/check_openapi_drift.py` | **79/79 operations match** (including `/v1/retrieval/cache`, `/v1/lifecycle/retain`, `/v1/graph/*`, `/v1/governance/*`, and `/version`) |
| Route auth parity | `cargo test -p sunborn-gateway --test route_contract_parity_test` | **77 unique paths / 79 method-path operations** classified with no router path missing or stale; anonymous callers rejected on every non-public route; non-admin keys rejected on every admin route; 4/4 local control routes verified |
| Observability contract | `cargo test -p sunborn-gateway --test observability_contract_test` | Every metric named in `deploy/k8s/prometheusrule.yaml` is present in a live `/metrics` scrape, every alert `runbook_url` anchor resolves to a heading in `docs/runbooks/incident_playbooks.md`, and the circuit-breaker gauge is asserted to follow the breaker into `OPEN` rather than report a constant |
| Outbox DLQ surface | `cargo test -p sunborn-gateway --test outbox_dlq_test` | `sunborn_invalidation_dlq` reports 0 on a clean outbox and 1 after an event exhausts retries; `POST /v1/lifecycle/outbox/replay` drains it back to 0, and is rejected 401 without a key and 403 for a non-admin key |
| Five-Gate Contract & E2E | `cargo test -p sunborn-gateway --test remediation_e2e_test --test execution_lifecycle_test --test permission_freshness_adversarial_test --test five_gate_benchmark_test --test enterprise_completeness_test` | Enforces `EligibleReuse = G1 && G2 && G3 && G4 && G5` across `handlers/execute.rs`, `five_gates.rs`, `execution_runtime.rs` (`UNKNOWN_OUTCOME` reconciliation), `retrieval_cache.rs` (separate `POST /v1/retrieval/cache` tier), `cache_lifecycle.rs` (versioned lifecycle + $V_i$ adaptive eviction via `POST /v1/lifecycle/retain` + safe prefetch gating), asymmetric `context_digest` + config/embedding fingerprints, claim-level `EvidenceBundle` grounding, quarantine isolation, and live B0–B6 + leave-one-out ablations |
| Frozen Unseen Generalization | `cargo test -p sunborn-semantic --test external_generalization_eval` | Evaluates frozen `StructuralFingerprint` rules on unseen `research/calibration/external_unseen_eval_dataset.json` (N=160). LSH raw similarity: 62 FP → guardrails 4 FP, 116 TN, 40 FN, **recall 0/80, F1 0.000** at 47.5% abstention. Local HTTP trigram: 92 FP → 6 FP, 114 TN, 38 FN (precision 0.250, recall 0.050). **This is abstention, not generalization**: the default deterministic embedder reuses no external paraphrase correctly. |
| External-Corpus Five-Gate Ablation | `cargo test -p sunborn-gateway --test external_corpus_ablation_test` (add `SUNBORN_WRITE_EVIDENCE=1` to write the artifact) | 160 external pairs × 4 probes = 640. False hits fall **B2 545 → B3 462 → B4 308 → B5 154 → B6 0**; B6's zero comprises 0/160 equivalence false hits **and 0/160 true paraphrase reuses**. G1 removes 65/160 raw-similarity equivalence errors; G3/G4/G5 each remove their injected threat class exactly. Probe types 2–4 are injected threats, not natural labels. Artifact: `research/results/external_five_gate_ablation.json`. |
| Feedback replay defense | `cargo test -p sunborn-gateway --lib learning::store::tests` | A signed feedback event replayed within the 15-minute freshness window was previously accepted repeatedly, inflating the observation count used by calibration/promotion. `record_event` now rejects duplicate `event_id` per tenant; 2 tests cover replay rejection and distinct-event acceptance. |
| Independent State Verifier | `cargo test -p sunborn-verify` | `apps/sunborn-verify/src/main.rs` verifies Ed25519 + Merkle root plus `--state` authoritative epochs/`content_hash`/revocations and `--evidence` `FiveGateEvidence` (`3/3` tests pass) |
| Degradation observability | `cargo test -p sunborn-gateway --test degradation_observability_test` | With the embedding provider forced to fail, requests still return 200 from origin, `sunborn_semantic_eval_failures_total` increments, and exact-cache hits keep working |
| Guardrail benchmark | `cargo test -p sunborn-semantic test_held_out_evaluation_dataset_precision_and_recall` | **N = 320 pairs (120 equivalent, 200 hard negatives): 0 false positives, 100% precision, recall 80.8% (97/120), Clopper-Pearson 99.9% upper bound on the false-positive rate < 3.4%.** Both precision and recall are reported; the dataset informed the verb-class normalizer, so this is regression evidence, not an unbiased holdout estimate. |
| Causal freshness engine | `cargo test -p sunborn-freshness` | **2/2** integration tests pass: five-dimensional freshness vector (document, permission, config, model, epoch), independent invalidation per dimension, fail-closed on unknown document, epoch ceiling, and confidence-decay monotonicity. Module: `crates/sunborn-freshness/src/lib.rs`. |
| Adversarial admission defense | `cargo test -p sunborn-threat` | **3/3** integration tests pass: prompt-injection, numeric inversion, negation flip, cross-tenant scope mismatch, benign paraphrase admitted clean, and quarantine release with retained audit reason. Module: `crates/sunborn-threat/src/lib.rs`. |
| Proof-Carrying Semantic Reuse | `cargo test -p sunborn-proof` | **1/1** integration test passes: Ed25519 cryptographic reuse certificate verification, tenant domain separation, and tamper detection. Module: `crates/sunborn-proof/src/lib.rs`. |
| Effect-Aware Agent DAG | `cargo test -p sunborn-effects` | **3/3** integration tests pass: typed effect classes, transitive side-effect taint propagation, and safe partial subgraph reuse. Module: `crates/sunborn-effects/src/lib.rs`. |
| FinOps Economic Optimization | `cargo test -p sunborn-economics` | **1/1** integration test passes: multi-dimensional cost engine ($V_{\text{reuse}}$), provider pricing catalogs, and multi-objective admission optimizer. Module: `crates/sunborn-economics/src/lib.rs`. |
| Self-Learning Cache Intelligence | `cargo test -p sunborn-learning` | **3/3** integration tests pass: offline shadow replay against multi-criteria safety gates, Total Variation Distance distribution drift detection, and calibrated canary approval. Module: `crates/sunborn-learning/src/lib.rs`. |
| SemanticCacheBench-X dataset | `python benchmarks/semantic_cache_bench_x.py --dry-run` | **2,000 generated cases** across 10 categories, SHA-256 fingerprint `8ec87595ad3fd3e45d2f1aa7f4088c361c32caa8053c81fc298d1e0a97f40897`; splits dev 1,163 / calibration 452 / test 385, with a hard failure on any held-out split leak. The corpus is synthetic and the artifact says so; no behavioural metric is emitted in dry-run. |
| SemanticCacheBench-X tests | `python -m pytest benchmarks/tests/test_bench_x_runner.py` | **13 passed**: determinism, category coverage, split disjointness and leak detection, metric arithmetic, zero-coverage reporting, and dry-run emitting no metric. |
| Launch demos | `python scripts/run_all_demos.py` | 5 demos wired to the real routes (`/v1/requests`, `/v1/invalidation`, `/v1/execution/plan`, `/v1/execution/results`, `/v1/governance/attest`, `/v1/governance/verify-proof`). Require a live gateway plus `SUNBORN_API_KEY`; they exit non-zero rather than simulate. Python compile and `ruff check` pass. |
| Multi-worker soak benchmark | `cargo test -p sunborn-gateway --test concurrent_multiworker_soak_test` | **10,000 requests, 16 concurrent workers, 0 failures** |
| Enterprise kill switches | `cargo test -p sunborn-gateway --test kill_switch_test` | **5/5 kill switches verified: key revocation, tenant suspension, model allowlist, spend ceiling, zero-quota** |
| Client showcase self-test | `python scripts/client_showcase.py --self-test` | **All 6 enterprise evaluation pillars passed (exit 0)** |
| Full verification script | `bash scripts/verify.sh` | **All 12 stages completed, exit 0** on the cleaned tree. Two stages are honestly degraded here and say so in their own output: stage 6 falls back to chart-schema validation when `helm` is absent from PATH, and stage 7 is a static compose/Dockerfile/manifest pre-flight, not a live container run (no Docker daemon on this host). |
| Windows stdio portability | `python scripts/client_showcase.py --self-test`, `python scripts/verify_india_sovereign.py` | Both scripts print `✓` and previously raised `UnicodeEncodeError` under cp1252 whenever stdout was redirected to a file or pipe, which is how CI invokes them. They now reconfigure stdout/stderr to UTF-8 with `errors="replace"`; both exit 0 redirected. |
| Architecture dossier | [`docs/ENTERPRISE_ARCHITECTURE_DOSSIER.md`](ENTERPRISE_ARCHITECTURE_DOSSIER.md) | Verified specification across 8 enterprise dimensions |
| Incident playbooks | [`docs/runbooks/incident_playbooks.md`](runbooks/incident_playbooks.md) | Published runbooks for all 6 mission-critical failure scenarios |

### Five-gate ablation evidence (live `/v1/requests`, N=28)

`cargo test -p sunborn-gateway --test five_gate_benchmark_test -- --nocapture` executes one
adversarial workload (safe repeats, cross-session, stale source, mutating DAG, negation/numeric
inversion, post-update repeats) against each progressive gate set. The test asserts the exact
false-hit vector, so the table cannot drift silently.

| Configuration | Safe hits | False hits | Origin misses |
|---|---:|---:|---:|
| B0 Origin only | 0 | 0 | 28 |
| B1 Exact cache only | 8 | 0 | 20 |
| B2 Unfiltered semantic | 11 | 16 | 1 |
| B3 G1+G2 | 11 | 8 | 9 |
| B4 G1+G2+G3 | 11 | 4 | 13 |
| B5 G1+G2+G3+G4 (no G5) | 10 | 1 | 17 |
| B6 Full contract | 11 | 0 | 17 |

The stale case advances the authoritative source epoch while the request keeps the snapshot it
was served under. G1 and G2 therefore still pass, and only G3 can refuse the reuse. An earlier
version of this workload changed the request's `content_hash`, which the G2 context digest
already caught, so B3 and B4 tied and the ablation did not exercise freshness at all. The
current workload makes each gate's marginal contribution measurable: G1+G2 remove 8 unsafe
reuses, G3 removes 4, G4 removes 3, and G5 removes the last tampered-certificate reuse.

### Model-checked properties

| Spec | Properties |
|---|---|
| `formal/EpochFencing.tla` | `PersistedAuthority` (no replica publishes an epoch the durable store has not seen), `MonotonicFences` (fences never move backwards), `FrozenStaysFrozen` (a failed persistence write freezes the replica rather than advancing it) |
| `formal/OutboxAtLeastOnce.tla` | `NoAckWithoutPurge`, `ProcessedStable`, `EventualCompletion` (bounded crashes, at-least-once progress) |
| `formal/reuse_safety/PCSR_Safety.tla` | `TenantIsolation` (certificates strictly domain-separated across tenants), `ProvenanceIntegrity` (reuses strictly verified against issued certificates) |
| `formal/temporal_validity/TemporalValidity.tla` | `NoStaleReuse` (cached results match authoritative dependency state at time of reuse) |
| `formal/execution_effects/EffectAwareExecution.tla` | `NoSideEffectReplay` (non-idempotent tool mutations and side-effecting operations are never replayed) |

The live multi-replica harness (`scripts/test_distributed_invalidation.sh`) was
executed in containerized CI across all six operating modes (`exact`, `semantic`,
`outbox`, `replay`, `finops`, `crash`) and PASSED cleanly with 100% success.
Its recorded results are in §4.

---

## 2. Closed in this pass

### 2.1 2026-11-17 audit remediation

| ID | Defect | Fix | Regression test |
|---|---|---|---|
| AUD-01 | BFF proxy built its upstream URL from the **raw** request path while the security policy evaluated the **normalized** path. `//attacker/…` or `https://attacker/…` resolved to a foreign origin, forwarding the caller's `Authorization` header (SSRF + credential exfiltration). | `buildGatewayUrl` in `frontend/src/lib/proxy-policy.ts` resolves the normalized path against the pinned gateway origin and rejects non-HTTP bases; the route uses it. | `frontend/tests/bff_security.test.mjs` — origin-pinning cases |
| AUD-02 | `DependencyGraph::get_affected_cache_keys` collapsed backend errors into an empty set. A graph outage therefore made an invalidation report `purged` with 0 keys and then clear the dependency lineage, leaving stale entries unreachable by any later invalidation. | Lookup and `clear_dependencies` now return `Result`; every invalidation path (`handlers`, `orchestrator`, `outbox`, `graph_cache`) aborts instead of treating failure as "no dependants". | `graph::tests::test_affected_keys_propagate_backend_failure` |
| AUD-03 | The origin replica suppressed its own Redis Stream echo without `XACK`-ing it, so every published event stayed pending and was replayed forever, growing the consumer's PEL without bound. | `handle_cluster_message` acks its own `stream_id` (the origin already applied the purge locally). | `cluster_sync::tests::test_origin_replica_acks_its_own_stream_entry` |
| AUD-04 | `research/eval_conformal_benchmark.py` printed a **hand-typed** confusion matrix (TP=30, FN=90) rather than evaluating the dataset, and set `alpha_satisfied` from an arbitrary `< 5%` cutoff. The checked-in artifact therefore disagreed with the actual guardrail (then TP=24 / FN=96, recall 20%, not 25%), and asserted α≤0.001 that the 3.39% upper bound cannot support. | Counts now come from `research/results/heldout_confusion_matrix.json`, asserted by the Rust evaluator so the two cannot diverge; the Clopper-Pearson bound is computed exactly by bisection for any k; `alpha_supported_by_999_upper_bound` is true only when the exact upper bound is ≤ α. README claims corrected. | `guardrails::tests::test_held_out_evaluation_dataset_precision_and_recall` cross-checks every count |
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
- **Verification Gates**: 84 frontend tests pass (`npm test --prefix frontend`), 0 serious/critical axe WCAG 2.2 AA violations across all 10 routes (`npm run test:a11y`), and 271 tab stops verified with visible focus rings (`npm run test:keyboard`).


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
  Public surface is exactly `/health`, `/ready`, `/metrics`, `/version`,
  `/v1/auth/login`, `/v1/ceremony/public-keys`.

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
- **Python SDK Modernization**: Added C SIMD FFI (`accel.py`), zero-copy JSON parsing (`orjson`/`msgspec` fallback to `json`), pooled HTTP transports (`_http.py`, httpx with a stdlib fallback), typed `execute_request()`/`evict_execution()`, a unified `sunborn` CLI, and real LangChain/LlamaIndex adapters over the execution-reuse endpoints. Verified by `scripts/test_python_sdk.sh` and **65 tests**, with ruff, mypy, and an 80% coverage floor enforced in CI (`python-sdk-quality`).
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
- The competing audit/transformation status documents were deleted on
  2026-11-21 rather than banner-marked, because a stale document with a
  "historical" banner is still read as evidence.
- Consolidation finished the same day: **153 Markdown files (944 KB) → 52 files
  (416 KB)**. Removed: the 54-file independent-audit corpus, 31 transformation
  reports, the superpowers agent scratch, `MASTER_ENGINEERING_PLAN`,
  `EXECUTIVE_BRIEF`, `IEEE_ACM_COMPLIANCE`, `ENTERPRISE_DELIVERABLES_REGISTER`,
  `PUBLIC_LAUNCH_BLUEPRINT`, and `docs/release/RELEASE_READINESS.md`.
  That last one was deleted for being *wrong*, not for being long: it claimed
  315 passing tests (252 Rust / 37 frontend) and 11/11 gates against measured
  395 / 67 / 8. What remains is either gate-required, CI-consumed, declared as a
  Cargo package readme, or the single source for a claim.
- Every dangling reference was repaired; `scripts/verify_doc_paths.py --strict`
  reports 100 referenced source paths, all present.

### 2.3 Operator surface and deployment hardening (2026-11-21)

| Area | Change | Verification |
|---|---|---|
| Configuration failure mode | `main` no longer panics on invalid configuration. It prints the cause and exits **78** (`EX_CONFIG`), a code supervisors can distinguish from a transient crash. `AppConfig::validate` checks port, host shape, and backend/URL scheme agreement, and reports every problem at once. | `sunborn-core config::tests::test_validate_*`, `PORT=not-a-port sunborn-gateway config-check` → exit 78 |
| Operator CLI | `sunborn-gateway` gains `version`, `config-check`, `doctor`, `healthcheck`, `help`. `doctor` verifies config, durable-path writability, bind availability, and key-material presence with PASS/FAIL/WARN lines and a non-zero exit on blockers. | `sunborn-gateway doctor` → exit 0 with one development warning; bind-conflict path reports the healthy running instance instead of failing |
| Build identity | New public `GET /version` and `sunborn_build_info` metric expose service, version, git SHA, profile, and rustc. No configuration or secret material. | `test_version_endpoint_is_public_and_reports_build_identity`, OpenAPI drift 77/77 |
| Latency observability | `/metrics` now exports a real Prometheus histogram (`sunborn_request_duration_seconds` with 12 buckets + `+Inf`, `_sum`, `_count`) plus `sunborn_uptime_seconds`, instead of only a cumulative sum. | `test_metrics_endpoint` asserts histogram type, buckets, count, uptime, build info |
| Response hardening | Gateway applies `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, and `Cross-Origin-Resource-Policy` to every response (including CORS preflights and early 4xx), and HSTS only when the request arrived over TLS. | `test_baseline_security_headers_are_applied` |
| Readiness bounds | `/ready` dependency probes are each bounded at 2s and report `*_unavailable_or_timed_out`, so a hung backend degrades readiness within the kubelet timeout instead of stalling it. | `test_readiness_endpoint` |
| Kubernetes | Added `startupProbe`, `terminationGracePeriodSeconds: 45` (above the 15s default drain), and a `PodDisruptionBudget` (`minAvailable: 2`) wired into kustomize. | `python scripts/check_k8s_manifests.py` |
| Installation verification | Both installers now run `config-check` and a real smoke start against a private port that must answer `/health` before reporting success; failure prints the log tail and points at `doctor`. | `bash scripts/install-sunborn.sh` (Unix), `scripts/install-sunborn.ps1` (Windows) |

### 2.4 Distributed consistency and frontend edge hardening (2026-11-21)

| Area | Change | Verification |
|---|---|---|
| Multi-replica proof | The single multi-replica test was named `..._and_stale_read_rejection` but never exercised a stale read, and depended on fixed sleeps. It is replaced by four deterministic tests that each assert one property: a peer purge on broadcast, origin echo suppression with acknowledgement, idempotent duplicate delivery (Redis replay), and rejection of a read fenced at an older epoch via `EpochLeaseManager::validate_lease`. | `cargo test -p sunborn-gateway --test multi_replica_consistency_test` → 4 passed; 15 consecutive runs, 0 failures |
| Histogram correctness | Added a unit test proving Prometheus cumulative-bucket semantics: a 4 ms observation increments `le=0.005` and every larger bound but not `le=0.001`/`le=0.0025`; a 2 s observation increments only `le=2.5`/`le=5.0`; buckets are non-decreasing and `_count` matches observations. | `cargo test -p sunborn-gateway --lib metrics_tests` |
| Probe throttle immunity | `/health`, `/ready`, `/metrics`, and `/version` bypass the pre-auth rate limiter. Previously a busy node's kubelet polling could be throttled into a restart loop. | `is_probe_path` helper + `middleware::rate_limit` unit coverage |
| Body-limit coherence | The BFF rejected bodies over 10 MiB while the gateway caps at 4 MiB, so oversized requests were streamed and then refused. The BFF now enforces the gateway's 4 MiB limit at the edge. | `frontend/tests` (BFF suite) |
| Frontend CSP | The policy is built per request in `src/middleware.ts` from `src/lib/csp.mjs`. `script-src` carries a 128-bit nonce and `'strict-dynamic'`, with no `'unsafe-inline'` or `'unsafe-eval'` in production; `object-src` is `'none'`, `frame-ancestors` is `'none'`, and `base-uri`/`form-action` are `'self'`. `style-src` keeps `'unsafe-inline'` deliberately, because inline `style` attributes are not a script-execution vector. A per-request nonce cannot be embedded in a prerendered page, so the root layout is request-rendered (`force-dynamic`) and the static header was removed from `next.config.mjs` to avoid a second, intersecting policy. | `frontend/tests/security_headers.test.mjs` (8 tests). Built server inspected in Chromium: 19/19 script tags carried the header's nonce, `Cache-Control: no-store`, and **0 CSP violations** in the console. |
| Build identity in images | `deploy/Dockerfile` stamps `SUNBORN_GIT_SHA` at compile time so `/version` and `sunborn_build_info` report the deployed commit; OCI label, Helm `appVersion`, and k8s image tags aligned to `1.0.0`. | `python scripts/check_k8s_manifests.py`, `python scripts/verify_doc_paths.py` |
| Supply chain | `cargo audit` over 362 dependencies: 0 vulnerabilities, 1 allowed unmaintained-crate warning (`chrono`, tracked in `audit.toml`). | `cargo audit` |
| Vector-store scaling | Qdrant collections were created with only `size` and `distance`, leaving HNSW at library defaults and payload in RAM. Collection creation now sends an explicit HNSW graph (`m=16`, `ef_construct=128`) and `on_disk_payload: true`; int8 scalar quantization is available behind `SUNBORN_QDRANT_QUANTIZATION` and stays **off** because its recall cost has not been measured on this corpus. | `vector_store::collection_tuning_tests::*` (3 tests) |
| Connection pooling | The overlay had no pooler, so per-pod pools summed past a managed instance's `max_connections` at HPA ceiling. Added `deploy/k8s/pgbouncer.yaml` (transaction mode, 5,000 client connections over 50 backends, `query_wait_timeout` so excess load sheds instead of queueing, non-root, read-only root, all capabilities dropped) and wired it into kustomize. Auth comes from an externally provisioned Secret, matching `secrets.yaml`. | `kubectl kustomize deploy/k8s` renders 13 objects, exit 0, no deprecation warnings; `check_k8s_manifests.py` passes |
| Lineage memory bound | The in-memory dependency graph accepted edges without limit, so a long-lived process grew monotonically whenever a cached response recorded lineage that was never invalidated. It now holds at most `DEFAULT_MAX_EDGES` (1,000,000) live edges and refuses further writes with `CapacityExceeded` → HTTP 503 instead of evicting, because evicting an existing edge would leave a cached response with no lineage and let it survive the invalidation it should have caught. The counter is maintained incrementally and recomputed after pruning. | `graph::capacity_tests::*` (6 tests: cap enforced, no eviction, idempotent re-record, removal and clear release capacity, hierarchy shares the budget) |
| Exact-tier recall | The guardrail rejected any phrasing pair differing by one word, so "Retrieve X" and "Fetch X" were treated as an entity swap and missed the exact tier. Two defects were behind it: a sentence-initial read-only verb was classified as a named entity, and the `un-`/`dis-` morphology heuristic flagged `display` as a negation marker. Both are fixed, and read-only verbs now normalise to a class token. Measured recall on the 120 equivalent pairs rose **20.0% → 80.8%** (24 → 97 true positives) with false positives still **0/200**; opposite-action probes (`show`/`hide`, `enable`/`disable`, `grant`/`revoke`, `approve`/`deny`, `fetch`/`remove`, `list`/`terminate`) are all still rejected. | `guardrails::tests::test_held_out_evaluation_dataset_precision_and_recall` (asserts the checked-in matrix and a ≥0.75 recall floor) |

### 2.5 Security, FinOps, and Consumer Runtime Remediation (2026-11-20, Issues #3-#10)

| ID | Priority | Defect / Requirement | Remediation & Engineering Implementation | Regression & Verification Evidence |
|---|---|---|---|---|
| #3 | P1 | Cryptographic shredding failure swallowed during tenant deletion | Modified `delete_tenant` handler in `crates/sunborn-gateway/src/handlers.rs` to propagate key shredding errors (`?`), immediately halting tenant deletion and preserving database records when shredding fails. | `governance_test::test_tenant_cascade_offboarding_fails_when_key_shredding_fails` |
| #4 | P1 | Floating-point rounding drift in authoritative spend calculations | Updated `crates/sunborn-db/src/postgres.rs` (`set_spend_ceiling`, `reserve_spend`, `settle_spend`, `spend_window_total`) to compute exclusively on 64-bit integer micro-units (`ceiling_micro_units`, `amount_micro_units`) without IEEE-754 drift. | `cargo check -p sunborn-db`, `finops::tests::test_spend_velocity_micro_units_accounting` |
| #5 | P1 | Local state ceiling updated before durable commit | Refactored `SpendLedger::set_ceiling` in `crates/sunborn-gateway/src/finops.rs` to await and ensure PostgreSQL durable write succeeds before updating in-memory `SpendVelocityController`. | `spend_ceiling_api_test::test_failed_durable_spend_ceiling_leaves_local_state_untouched` |
| #6 | P1 | Demo sample tenants provisioned during production startup | Gated all sample tenants (`tenant-alpha`, `enterprise-fintech`, etc.) behind `!is_production_mode()` in `crates/sunborn-gateway/src/state.rs`. | `production_startup_test::test_production_startup_does_not_seed_sample_tenants` |
| #7 | P2 | TypeScript SDK missing dependencies during CI build | Added `npm ci --prefix sdk/typescript` before `npm run build` in `.github/workflows/ci.yml`. | `npm run build --prefix sdk/typescript`, `npm test --prefix sdk/typescript` (4 passed) |
| #8 | P1 | Ephemeral execution & learning stores unblocked in production | Implemented `PersistentExecutionCacheStore` with append-only Write-Ahead Log (WAL) and recovery replay. Updated `verify_production_readiness()` to strictly reject ephemeral storage tiers in production. | `production_startup_test::test_production_startup_rejects_ephemeral_stores`, `execution::store::tests::test_persistent_execution_cache_store_wal_replay` |
| #9 | P1 | Developer-only local BYOK startup path | Delivered OS-native AES-256-GCM encrypted credential vault (`LocalCredentialVault`), masked secret previews (`sk-...1234`), 1-click Windows installer (`scripts/install-sunborn.ps1`), desktop runner (`scripts/sunborn-desktop.ps1`), Linux installer (`scripts/install-sunborn.sh`), clean uninstaller (`scripts/uninstall-sunborn.ps1`), and default loopback binding (`127.0.0.1:8080`). | `local_control_api_test::test_local_byok_control_endpoints_lifecycle`, `sunborn_identity::local_vault::tests::*` |
| #10 | P1 | Uncalibrated quality-attribute & accessibility claims | Structured clear taxonomy separating in-tree automated verification (333 Rust tests, 63 frontend tests, 4 TS SDK tests, 8/8 pre-flight gates) from staging cluster operational gates (N1 multi-node invalidation harness, N2 external penetration testing). Corrected automated axe-core scan disclosure. | `python scripts/check_launch_docs.py`, `python scripts/verify_public_launch.py` |

### 2.6 Tenant fail-closed isolation and telemetry truthfulness (2026-11-23)

| Area | Change | Verification |
|---|---|---|
| Tenant isolation in causal invalidation | Graph traversal now fails closed: with a tenant scope set, a node is traversed only when it exists **and** carries an explicit matching `tenant_id`. Missing metadata is no longer treated as ownership. Roots are validated before any epoch bump, outbox write, or purge; a cross-tenant or unattributed root is refused with **403** before durable state changes. `POST /v1/graph/invalidate` now requires `tenant_id` (omitting it is a **422**), and `POST /v1/dependencies` requires it so recorded nodes are attributable. | `affected::tests::test_tenant_scoping_fails_closed_on_missing_metadata`, `affected::tests::test_tenant_scoping_rejects_cross_tenant_root`, `graphify_causal_invalidation_test::test_causal_invalidation_is_tenant_isolated_and_fails_closed` (4 assertions: own subtree purged, peer subtree untouched, foreign root 403, unattributed root 403, missing scope 422) |
| Invalidation error propagation | A failed `GraphMemoryCache` delete during causal invalidation is now recorded in `failed_keys` and reported as `partially_purged`, not silently discarded. All `mark_outbox_failed` calls log the secondary failure and leave the event PENDING for worker retry instead of discarding the error. | `cargo test -p sunborn-gateway --test graphify_causal_invalidation_test` |
| Cluster telemetry truthfulness | `/v1/cluster/sync/status` no longer reports a fabricated `nodes_active: 2` when a durable transport is configured. It reports `nodes_active: 1`, `nodes_active_scope: local_process_only`, and `replica_telemetry: unavailable`. The cockpit renders only the locally attested node plus an explicit `PARTIAL` peer-count card; the hard-coded `us-west-2`/`eu-central-1`/`ap-northeast-1` replicas and their RTTs are removed. | `cargo test -p sunborn-gateway --test integration_test test_cluster_sync_vector_clock_and_autonomic_status`; `npm test --prefix frontend` (67 passed) |
| Research claim reconciliation | README, the prelaunch checklist, and the launch/capacity documents now all state **recall 80.8% (97/120)**. The stale 20% figures were removed. | `grep -rn "recall 20" README.md docs/launch` returns no current-status claim |
| Empty provider env values | A present-but-empty provider variable (e.g. `OPENAI_API_KEY=`) is now treated as unset. Previously it registered a provider with an empty base URL and returned a 500 builder error instead of falling back to the mock origin. | `state::tests::empty_or_whitespace_env_values_are_treated_as_unset`; live run `/ready` reports `llm_origin: {status: connected, is_mock: true}` |
| Local broker wiring, verified live (2026-11-23) | Ran `scripts/dev-up.sh` against Docker with Postgres 16, Dragonfly 1.20, and Qdrant 1.10. Postgres applied migrations 1–12 (**27 public tables**), Qdrant created `sunborn_vectors`, and `/ready` reported cache, database, and vector store all `connected`. Two identical requests then returned an origin miss followed by an exact cache hit (`cache_path: exact`, Ed25519 certificate, 175 ms → 11 ms); the exact key and `sunborn:invalidation:events` were present in Dragonfly. `make smoke` reproduces this and exits 0. | `bash scripts/dev-up.sh`, `make smoke`, `docker exec sunborn-dragonfly redis-cli`, `docker exec sunborn-postgres psql`, `curl /ready` |
| Tenant isolation, verified live | Against the broker-wired gateway: tenant-owned root returned **200 purged**, a cross-tenant root returned **403**, and an omitted `tenant_id` returned **422**. | Live `curl` run recorded in the row above; also covered by the in-tree regression test |
| Release provenance wording corrected | A hand-written JSON manifest had been labelled `SLSA-Build-Level-2`, which it was not. It is now `BUILD-METADATA.json` (`manifest_type: sunborn-build-metadata-v1`, `signed: false`), and real provenance is requested in CI via `actions/attest-build-provenance@v2` with `id-token`/`attestations` write permissions. Authenticode signing remains conditional on `SIGNING_CERT_PFX`. | `.github/workflows/release.yml` and `scripts/package-distribution.ps1` parsed locally; the release workflow has not been executed in this environment |

### 2.7 Release acceptance and external evidence (2026-11-25)

| Area | Change | Verification |
|---|---|---|
| Clean-toolchain install | The packaged installer smoke-tested the gateway without local broker settings, so it contacted `redis://dragonfly:6379` and failed on any desktop machine without Dragonfly. `install-sunborn.ps1` now sets memory cache/DB/vector backends, non-strict durability and a local KMS provider for the smoke run. | Installer exit 0 and `/health` answered with `PATH=C:\Windows\System32;C:\Windows` (`cargo`/`node` absent); `sunborn-gateway --version` printed `1.0.0 (release)`. Evidence: `docs/audit/RELEASE_ACCEPTANCE_EVIDENCE.md`. |
| Custom-directory uninstall | `uninstall-sunborn.ps1` hardcoded `%LOCALAPPDATA%\Sunborn` and ignored the installer's `-TargetDir`, so a custom install could not be removed. It now takes `-TargetDir`, and the registered `UninstallString`/`QuietUninstallString` pass it through. | Install into a temp `-TargetDir`, uninstall with `-Silent -PurgeData`; `Test-Path` returned `False` and no `sunborn-gateway` process remained. |
| Authenticode pipeline | `package-distribution.ps1` blocked indefinitely on `Get-PfxCertificate` (interactive password prompt) and on an unreachable RFC 3161 timestamp server. It now loads the PFX with `X509Certificate2` + password, fails when no signer certificate is applied, and supports `-NoTimestamp` for offline signing. | Produced `sunborn-desktop-v1.0.0-windows-x64.zip` with a self-signed signature (`CN=Sunborn Dev Self-Signed (NOT FOR PRODUCTION)`, thumbprint `3C8F48D0…B003B092`). `Get-AuthenticodeSignature` reports `UnknownError` because the root is untrusted — **not a production-signed release**. |
| External-corpus evidence | Added `crates/sunborn-gateway/tests/external_corpus_ablation_test.rs` over the frozen 160-pair dataset, with injected stale-epoch, mutating-plan and tampered-certificate probes. Corrected the documented interpretation of the existing external evaluation. | See the two rows in §1; artifact `research/results/external_five_gate_ablation.json` is written with `SUNBORN_WRITE_EVIDENCE=1`. |

### 2.6 Free-tier defaults and in-place upgrade (2026-11-27)

| Area | Change | Verification |
|---|---|---|
| Tier policy | New `crates/sunborn-gateway/src/tiers.rs` defines the `free`, `pilot`, `standard`, and `enterprise` envelopes as code: request quotas, model allowlist, retention, support level, invitation cap, and identity requirement. `SUNBORN_TIER_FREE_MODELS` overrides the free allowlist. `0` always denies, and "unlimited" is `i64::MAX` because the quota columns are `BIGINT`. | `cargo test -p sunborn-gateway --lib` → 57 passed, including 6 tier-policy tests |
| Provisioning | `POST /v1/governance/provision` applies the tier's defaults and enrolls its quota as one unit, refuses an identity-gated tier without `identity_verified` (403), and counts invitations per inviter over 30 days. An omitted tier stays `pilot`, so an existing operator-driven caller is not silently moved onto the free envelope. | `cargo test -p sunborn-gateway --test tenant_tier_test` → 7 passed |
| Upgrade path | `POST /v1/governance/tenant-quota` accepts `{tenant_id, tier}` with no explicit limits and re-applies that tier's policy in place (`applied: "tier_policy"`), so an upgrade needs neither a new tenant nor a data migration. Explicit limits still work (`applied: "explicit_limits"`) but must be supplied as a complete daily+monthly pair, because a half-specified envelope would leave the other window silently unchanged. | same suite: `a_tier_only_update_raises_limits_on_the_same_tenant`, `a_half_specified_envelope_is_rejected` |
| Model allowlist | A credential's `allowed_models` became optional: absent means the tenant tier's default, and a supplied list is checked against the tier ceiling, so a free tenant cannot be widened to `*` or a premium model. | `cargo test -p sunborn-gateway --lib allowlist_tests` → 4 passed |
| Schema | `migrations/0013_tenant_tier_defaults.sql` adds retention, support level, identity attestation, inviter, and default-allowlist columns plus an invitation index; `migrations/down/0013_tenant_tier_defaults.down.sql` is the back path. | `cargo check -p sunborn-db --all-targets` exit 0. **Forward/back execution against a live PostgreSQL instance was NOT run — no database was available in this environment.** |
| Invitation cap | Five free invitations per inviter in a rolling 30 days. The count is a durable query when PostgreSQL is present, with an in-memory ledger for single-replica development so the cap is exercised rather than silently disabled; re-provisioning a tenant cannot inflate it. | `the_invitation_cap_refuses_the_sixth_free_tenant` (5 admitted, 6th `429`, a different inviter unaffected) |

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

Recorded on 2026-11-15/16 before this pass (the originating audit document was
removed in the 2026-11-21 consolidation; the numbers below are retained because
they were produced by the named commands, which can be re-run):

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
- **Buying or reviewing**: this file — every figure is either re-runnable from §1 or explicitly marked as not established in §5.
- **Reporting a vulnerability**: [`SECURITY.md`](../SECURITY.md) — privately, never in a public issue.
- **Contributing**: [`CONTRIBUTING.md`](../CONTRIBUTING.md) explains the evidence rules a change must meet.
- **Getting help**: [`SUPPORT.md`](../SUPPORT.md) maps questions to the document that answers them.
- **Conduct**: [`CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md).
