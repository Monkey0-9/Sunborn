# 52 — Master Remediation Plan

**Basis:** `50_MASTER_EVIDENCE_REGISTER.md` (findings D-01…D-12, component register) and `51_SEMANTIC_GRAPH_MEMORY_DESIGN.md`.
**Rule:** a task is done only when its **acceptance test** fails before the change and passes after, in CI, with the artifact committed. No task is closed by a doc edit.

**Ordering principle:** correctness → durability → distribution → measurement → claims. Do not tune hit rate before invalidation is provably complete.

## Implementation status (2026-11-14, this revision)

Landed and covered by tests:

- **D-04 closed:** `invalidate_docs_rtc_purge` now returns `Err` when **any** key fails, and `CompressedLineageGraph::peek_documents` preserves doc lineage until every deletion is acknowledged, so partial purges are visible and retryable. Test: `test_doc_purge_partial_failure_is_visible_and_lineage_preserved`.
- **D-10 closed:** `trigger_invalidation` and `invalidate_graph_rtc` advance epoch + watermark **before** deleting, closing the re-admission window.
- **D-06 closed:** Neo4j closure is `*1..10` and the mock traverses 10 hops, matching the in-memory adapter (test assertion updated).
- **P0-8 (single-node part) done:** `DecisionConfig::candidate_limit = 8` and `ambiguity_delta = 0.01`; near-tied candidates with different content return `AMBIGUOUS_SEMANTIC_CANDIDATES` instead of reusing. Test: `test_decision_engine_abstains_on_ambiguous_candidates`.
- **P0-10 partial:** README test badge corrected to the measured Rust count and pointed at the master register; unsupported performance/NLI claims still need removal.

Still open (not faked): durable epoch storage (P0-2), transactional outbox consumer (P0-3), Redis Streams transport (P0-4), single migration authority (P0-1), rebuild manifest/pagination (P0-6), calibration artifact (P0-7), live provider tests (P0-9), two-replica CI (P0-11), frontend security pass (P0-12).

---

## P0 — blocks any production claim (target: 3 weeks)

| ID | Task | Files / symbols | Acceptance test | Effort |
|---|---|---|---|---|
| P0-1 | **Single migration authority.** Delete the embedded DDL from `PostgresDatabaseStore::migrate`; use one ordered migrator with a version table; reconcile the 4 SQL files (column widths, `tenant_wrapped_keys` vs legacy `tenant_encryption_keys`, all 15+ tables). | `crates/sunborn-db/src/postgres.rs`, `migrations/*` | Fresh DB + upgrade DB both migrate; schema hash matches a committed expected schema; CI fails on drift | 3 d |
| P0-2 | **Persist epoch authority.** Add `tenant_source_epochs(tenant_id, source_id, epoch)`; bump in the same tx as the mutation/outbox; hydrate `EpochLeaseManager` at boot; reject epochs below stored hwm. | `sunborn-invalidation/src/epoch.rs`, `sunborn-db`, startup | Restart test: issue cert at epoch N, restart, assert old cert rejected unless N == stored; monotonic property test | 4 d |
| P0-3 | **Real transactional outbox.** Mutation + outbox row in one tx; consumer claims rows with `FOR UPDATE SKIP LOCKED`; idempotency key; retry/backoff; DLQ in DB (not memory). Remove `status='processed'` fabrication. | `outbox.rs`, `handlers.rs::trigger_invalidation`, `postgres.rs` | Kill process between commit and processing; restart; event is applied exactly once; duplicate delivery no-op | 5 d |
| P0-4 | **Replace `InMemoryClusterTransport` in production.** Redis Streams (or Dragonfly) transport + per-pod consumer + reconciliation against hwm. Keep in-memory as test double behind `#[cfg(test)]` or explicit dev flag. | `cluster_sync.rs`, `state.rs` | 2 real processes, shared Redis: mutate on A → read on B misses within 250 ms p99; 10-min soak zero stale | 5 d |
| P0-5 | **One owned invalidation orchestrator.** Purge exact + vector + graph in one function; return `PurgeReport`; partial ⇒ error; bump epoch/watermark **before** delete; constant closure depth. | new `sunborn-invalidation/src/orchestrator.rs`; `handlers.rs` | Fault-injected vector delete fails: API returns non-2xx with `failed[]`; no `purged` status; D-04/D-07/D-10 tests | 3 d |
| P0-6 | **Recovery monotonicity.** Rebuild restores epochs first, requires valid cert, excludes tombstones, paginates with checkpoints, writes a manifest (counts, skipped reasons, hashes). | `handlers.rs::trigger_rebuild`, `apps/sunborn-rebuild` | Property: `admitted ⊆ verified ∧ epoch_after ≥ epoch_before`; rebuild twice is idempotent; corrupt cert skipped and reported | 3 d |
| P0-7 | **Stop claiming semantic safety without measurement.** Load a durable calibration set per stratum; fail startup in prod if absent; replace seeded thresholds. | `conformal.rs`, `decision.rs`, config | Startup refuses prod without calibration artifact; threshold provenance logged per decision | 4 d |
| P0-8 | **Candidate set + abstention.** `top_k ≥ 8`; ambiguity rule; `AMBIGUOUS_SEMANTIC_CLUSTER` decision; never reuse on disagreement. | `decision.rs`, handlers | Two plausible candidates with different hashes ⇒ miss; single candidate still hits | 2 d |
| P0-9 | **Live provider integration tests.** Recorded-fixture tests plus opt-in live tests for OpenAI-compatible, Anthropic, Gemini; assert wire format, auth headers, 429/503 failover. | `origin/*`, new live-provider test module | CI (fixtures) always; nightly (live) with secrets; failover proven | 4 d |
| P0-10 | **Truth pass on README/docs.** Mark simulated numbers, remove unsupported 571k/1.8B/NLI/WAL/distributed claims or attach artifacts. Add a "Measured / Modeled / Aspirational" legend. | `README.md`, `docs/release`, audit docs | Doc-path + claim-lint script passes; no number without a linked artifact | 2 d |
| P0-11 | **Multi-replica test in CI (kind or docker-compose).** 2 gateways + Postgres + Redis + Qdrant; run the invalidation and duplicate-admission scenarios. | `deploy/`, `scripts/verify.sh`, CI | Test runs on every PR; currently would fail (good — it proves the gap) | 3 d |
| P0-12 | **Frontend/API security re-verification.** CSP/headers, cookie flags, CSRF, dev-login gating, no key minting on failure. | `frontend/src/app/api/*`, `next.config` | Security tests + manual checklist; prior P0s re-run | 2 d |

**P0 exit gate:** P0-1…P0-6 and P0-9/P0-11 green in CI; P0-7/P0-8 produce a signed evaluation artifact; P0-10 removes overclaims. Until then: **no production, no "GA", no alpha guarantee in public copy.**

---

## P1 — correctness/scale hardening (target: 3 weeks)

| ID | Task | Acceptance | Effort |
|---|---|---|---|
| P1-1 | Key-hierarchy GC: remove key→key edges on purge/TTL; add edge counters | Long-run soak: edge count bounded; no over-purge after key reuse | 2 d |
| P1-2 | Unify closure depth + truncation reporting across memory/Neo4j/mock | Same graph, same result on all three; `truncated` asserted | 1 d |
| P1-3 | Admission state machine persisted per entry (`pending/admitted/degraded/failed`) | Crash between stages leaves `pending`; reconciler completes or rolls back; no false success | 3 d |
| P1-4 | Real counters for graph stats (remove literals 65536/0/0/0) | Metrics reflect operations; no hardcoded fields | 1 d |
| P1-5 | Tenant-scoped watermarks and epochs (no bare UUID key) | Cross-tenant property test: mutation in A never fences B | 1 d |
| P1-6 | Vector delete on every purge path (single orchestrator) | Stale-vector test: purge source, `all_entries` shows none for key | 1 d |
| P1-7 | Durable calibration + shadow feedback loop (HMAC verified) | Feedback replay produces same threshold; tamper rejected | 3 d |
| P1-8 | Neural NLI backend (ONNX DeBERTa/MiniLM) behind `CrossEncoderBackend`, with model hash pinning | Adversarial NLI suite beats structural baseline; model hash verified at load | 5 d |
| P1-9 | ANN index or require Qdrant in prod (remove O(n) memory path from prod readiness) | Prod refuses memory vectors; Qdrant recall@10 measured | 2 d |
| P1-10 | Real benchmarks: criterion for Rust hot paths; live-stack harness with signed run manifest | Published artifact with commit, hardware, dataset hash; replaces modeled numbers | 5 d |
| P1-11 | SBOM signing + SLSA provenance in release workflow | `cosign verify` / provenance attestation in CI | 2 d |
| P1-12 | Semantic graph phases G0–G4 from doc 51 | §10 invariants + §11 evaluation gates | 15 d |

---

## P2 — maturity (target: ongoing)

| ID | Task | Acceptance |
|---|---|---|
| P2-1 | Collapse the 50-doc audit set into one SSOT + appendices; archive superseded docs | One index; no contradictory verdicts |
| P2-2 | Requirements traceability regenerated from tests (not hand-written) | Every R-x maps to a test id |
| P2-3 | Chaos suite: pod kill, Redis partition, Qdrant downtime, Postgres failover | Documented SLOs hold or degrade explicitly |
| P2-4 | Per-tenant SLOs + error budgets; metrics aggregation across pods | Dashboard + alert rules committed |
| P2-5 | Cost model validation against real invoices | Ledger reconciled within tolerance |
| P2-6 | Accessibility re-test (WCAG 2.2 AA) with automated + manual evidence | Report with tool output |
| P2-7 | Independent external review (security + crypto) | Signed report; findings tracked |

---

## Workstream ownership (13 disciplines)

| Workstream | Owns | P0 items |
|---|---|---|
| Backend | handlers, state, API contracts | P0-5, P0-8 |
| Frontend | BFF, auth UX, CSP | P0-12 |
| Distributed systems | outbox, cluster sync, reconciliation | P0-3, P0-4, P0-11 |
| Database | migrations, epochs, indexes | P0-1, P0-2 |
| Security | auth, tenant isolation, secrets | P0-12 |
| Cryptography | certificates, KMS, key separation | P0-6 (verify), P2-7 |
| AI/ML research | calibration, NLI, evaluation | P0-7, P0-8, P1-7/8 |
| SRE/DevOps | CI, deploy, observability, SLSA | P0-11, P1-10/11 |
| QA | acceptance tests, property tests, fault injection | all P0 tests |
| Performance | criterion, live harness | P1-10 |
| Technical product | claim hygiene, release gating | P0-10 |
| Formal methods | invariants (I1–I8), monotonicity proof | P0-2/6 |
| Data governance | retention, tombstones, shredding | P0-6 |

**Review gate:** each P0 requires one owner + one reviewer from a different workstream; security/crypto changes require two. Record reviewer names in the PR.

---

## 4-week sequence

| Week | Deliverables |
|---|---|
| 1 | P0-1 (migrations), P0-2 (epochs), P0-5 (orchestrator), failing 2-pod test added |
| 2 | P0-3 (outbox), P0-4 (streams), P0-6 (rebuild), P0-11 CI wiring |
| 3 | P0-7/8 (calibration + abstention), P0-9 (providers), P0-12 (frontend) |
| 4 | P0-10 (truth pass), full gate run, evaluation artifact, go/no-go review |

Semantic graph G0–G4 runs in parallel from week 2 (schema after P0-1, distribution after P0-4).

---

## CI gates to add

1. `cargo test --workspace --all-targets` (already passing: 258).
2. Two-replica integration test (P0-11) — required, currently missing.
3. Schema drift check: fresh vs. migrated DB schema hash.
4. Purge-completeness fault injection.
5. Claim lint: README numbers must link to an artifact or carry `[modeled]`.
6. Doc-path check (`scripts/verify_doc_paths.py`) extended to new docs.
7. `cargo audit` / `cargo deny` without suppressed advisories.
8. Live-provider fixtures; nightly live run.

---

## Definition of done for "Production Ready"

- Every P0 closed with a test that fails on the previous commit.
- 2-pod soak: zero stale reads for 30 minutes; invalidation p99 ≤ 250 ms.
- Restart drill: no certificate gains validity; epochs monotonic.
- Partial purge impossible by construction (orchestrator returns report and fails closed).
- Semantic reuse measured on held-out + adversarial data; published artifact with dataset/code hashes; claims match numbers.
- Migrations: one authority, forward + rollback tested.
- Providers: live integration tests for at least two providers, failover proven.
- Docs: one SSOT, measured/modeled separated, no unsupported superlatives.
- External security review completed or explicitly scheduled with residual risk accepted in writing.

---

## Push checklist (for the human, since this audit does not push)

Files added by this revision:
- `docs/independent-engineering-audit/50_MASTER_EVIDENCE_REGISTER.md`
- `docs/independent-engineering-audit/51_SEMANTIC_GRAPH_MEMORY_DESIGN.md`
- `docs/independent-engineering-audit/52_MASTER_REMEDIATION_PLAN.md`
- `docs/independent-engineering-audit/inventories/evidence_register_2026-11-14.csv`
- pointer note appended to `docs/independent-engineering-audit/01_README.md`

Before pushing:
1. `cargo check --workspace --all-targets` (expect exit 0).
2. `cargo test --workspace --all-targets` (expect 258 passed).
3. `python scripts/verify_doc_paths.py` (expect success).
4. Read the diff; these docs intentionally contradict older docs — the pointer in `01_README.md` explains which wins.

**Not done here (deliberately):** no implementation of the semantic graph, no migration edits, no production claim changes. Those are P0/P1 tasks above and need code review, not a doc commit.
