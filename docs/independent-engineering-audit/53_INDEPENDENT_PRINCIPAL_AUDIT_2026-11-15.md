# 53 — Independent Principal Engineering Audit (2026-11-15)

**Classification: FUNCTIONAL PROTOTYPE / SINGLE-REPLICA PILOT. Production: NO-GO.**
**Evidence date:** 2026-11-15
**Branch audited:** `engineering/sunborn-95-quality-transformation` (working tree)
**Audit mode:** read-only code review + executed verification. No code was modified, committed, or pushed.

> **Honesty note on the "200–400 senior engineers" premise.** This report is a
> workstream-structured review across the requested disciplines, performed by one
> agent against the working tree. It is **not** 200–400 independent humans, not a
> third-party penetration test, and not a certification. Every status below is
> tied to code or an executed command; where evidence is absent, the status says so.

---

## 0. Scope and method

**Scope:** all workspace crates (`sunborn-core`, `-cache`, `-identity`, `-db`,
`-execution`, `-invalidation`, `-semantic`, `-gateway`), both binaries
(`sunborn-rebuild`, `sunborn-verify`), the Next.js control plane/BFF, migrations,
research benchmarks/calibration data, tests, CI, and the 32-document
`docs/engineering-transformation/` suite, reconciled against the earlier
`docs/independent-engineering-audit/` suite.

**Method:**
1. Inventory the tree, manifests, and dependency graph.
2. Trace every headline claim to a reachable runtime path and a test or artifact.
3. Run the project's own gates (Rust tests, frontend tests, Python SDK tests,
   formatting, clippy, `cargo audit`).
4. Record what is implemented, tested, verified, partial, broken, or absent.

**Not performed:** live PostgreSQL/Dragonfly/Qdrant/Neo4j/KMS provisioning,
multi-host network tests, penetration testing, fuzzing campaign, long-duration
soak, independent human review, formal verification (TLA+/Alloy/Coq).

---

## 1. Verification actually executed

| Check | Command | Result |
|---|---|---|
| Rust workspace compile + all targets | `cargo test --workspace --all-targets` | **PASS**, exit 0 |
| Rust tests | same | **270 passed, 0 failed, 0 ignored** (40 binaries) |
| Frontend suite | `npm test --prefix frontend` | **PASS**, 37/37 |
| Python SDK — package tests | `python -m unittest discover -s sdk/python/tests` | **PASS**, 5/5 |
| Python SDK — legacy agent tests | `python -m unittest sdk/python/test_sunborn_agent.py` | **PASS**, 21/21 |
| Formatting | `cargo fmt --all -- --check` | **PASS**, exit 0 |
| Clippy (strict) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **PASS**, 0 errors, 0 warnings |
| Dependency advisories | `cargo audit --json` | **PASS**, 0 vulnerabilities, 0 warnings |
| Route registrations | `grep -c '.route(' gateway/src/lib.rs` | **49** |
| Tracked files | `git ls-files \| wc -l` | **391** |
| Rust LOC (crates + apps) | `wc -l` over `*.rs` | **35,204** |

The build/test claims in `28_TEST_EXECUTION_EVIDENCE.md` are broadly reproducible:
the Rust count is now 270 (docs say "260+"), frontend is 37, and Python is 26.
**This is the strongest part of the repository and should be stated plainly.**

---

## 2. Executive verdict

Sunborn is a substantive Rust LLM gateway with real authentication, exact caching,
FinOps accounting, envelope encryption, an execution DAG, a dependency graph, and
a broad test suite. It is **not** a 9.71/10 production platform. The 2026-11-15
transformation documents repeatedly claim closure of issues that remain open in
the code, and the repository still contains the prior independent audit that rated
the same system ~5.1/10 and NO-GO.

**The decisive gap is not compile quality; it is evidence quality.** Several
"world-class" claims are proven only by same-process tests, self-authored fixtures,
or mocks. The system is credible for a single-replica, non-safety-critical pilot
with explicit disclosure. It is not safe to represent semantic reuse as calibrated
or multi-replica invalidation as proven.

**Honest maturity score: 5.3 / 10 (±0.5).** Score movement from 5.1 is real but
small: tests and formatting are clean, production secret gates exist, and the
frontend no longer fabricates telemetry. The distributed, semantic, recovery, and
documentation-truth gaps prevent a higher score.

---

## 3. Evidence register — component status

Legend: **VERIFIED** = reachable on a real path with a passing test/command.
**PARTIAL** = works with a material caveat. **UNVERIFIED** = code exists, no
independent runtime evidence. **SIMULATED** = test/mock only. **ABSENT** = not
implemented.

| # | Component | Evidence | Verification | Status |
|---|---|---|---|---|
| 1 | Axum gateway, 49 routes | `gateway/src/lib.rs` | route count + integration tests | **VERIFIED (single process)** |
| 2 | API-key auth, tenant/app RBAC, model allow-lists | `middleware/auth.rs` | `security_test`, `login_test` | **VERIFIED** |
| 3 | Dual-admin quorum on destructive routes | `auth.rs::require_admin_quorum` | `quorum_authorization_test` | **PARTIAL** — two shared keys, not signatures; opt-in outside production |
| 4 | L1 exact cache, byte-budget GDSF | `cache/src/memory.rs`, `gdsf.rs` | cache tests | **VERIFIED** |
| 5 | Dragonfly/Redis exact cache | `cache/src/dragonfly.rs` | unit; no live Redis test | **UNVERIFIED (live)** |
| 6 | Semantic decision cascade | `semantic/src/decision.rs` | `test_verification_cascade_stages` | **VERIFIED (logic)** |
| 7 | Default embeddings | `DeterministicEmbeddingProvider` (FNV-1a LSH) | unit | **SIMULATED DEFAULT (dev)** |
| 8 | HTTP embeddings | `semantic/src/embedding.rs` | no live provider test | **UNVERIFIED** |
| 9 | Memory vector store | `vector_store.rs` | unit | **VERIFIED (O(n·d))** |
| 10 | Qdrant adapter | `vector_store.rs` | mock/no live Qdrant | **UNVERIFIED (live)** |
| 11 | Qdrant point delete | canonical UUID on insert/delete | `remediation_verification_test` | **VERIFIED (code)** |
| 12 | Structural guardrails | `guardrails.rs::StructuralFingerprint` | extensive unit tests | **VERIFIED (heuristic, not AST)** |
| 13 | Cross-encoder / NLI | `StructuralEntailmentScorer` only | none for a neural backend | **ABSENT (neural)** |
| 14 | Conformal calibration | `conformal.rs`, seeded thresholds | synthetic unit tests | **PARTIAL** |
| 15 | OOD drift detector | Welford detector | unit | **PARTIAL (process-local)** |
| 16 | ReuseCertificate sign/verify | `core/src/certificate.rs` | `reuse_certificate_path_test` | **VERIFIED (Ed25519)** |
| 17 | Measured safety evidence at read | `handlers.rs` | certificate-path tests | **PARTIAL** — lexical, not semantic |
| 18 | Server-derived epochs | `handlers.rs::acquire_lease` | tests | **VERIFIED (single process)** |
| 19 | Epoch durability / authority | `invalidation/src/epoch.rs`, `db/src/postgres.rs` | unit with mock DB | **PARTIAL / BROKEN across replicas** |
| 20 | Watermark write-back fence | `watermark.rs` | chaos/disaster tests | **VERIFIED (single process)** |
| 21 | Dependency graph + transitive closure | `graph.rs` | unit | **VERIFIED (single process)** |
| 22 | Neo4j lineage | `graph.rs` | mock Cypher only | **UNVERIFIED (live)** |
| 23 | Graph memory cache + Roaring lineage | `graph_cache.rs` | graph tests | **VERIFIED (single process)** |
| 24 | RTC purge across exact + vector + graph | `handlers.rs::invalidate_graph_rtc` | graph tests | **PARTIAL** — not atomic, partial errors |
| 25 | Unified invalidation orchestrator | `orchestrator.rs` | `test_invalidation_orchestrator_purge_sources` | **PARTIAL** — sequential best-effort |
| 26 | Durable transactional outbox | `state.rs` worker, `outbox_events` table | `outbox_replay_test` | **BROKEN** — not at-least-once |
| 27 | Cross-pod invalidation | `cluster_sync.rs` | in-memory bus test | **SIMULATED DISTRIBUTED** |
| 28 | PostgreSQL persistence | `db/src/postgres.rs` | circuit-breaker/unit | **UNVERIFIED (live)** |
| 29 | Migration authority | `migrations/*.sql` embedded via `include_str!` | none for drift | **PARTIAL** — checksum recorded, never compared |
| 30 | Envelope encryption / crypto-shred | `identity/src/crypto_shred.rs` | 23 identity tests | **VERIFIED** |
| 31 | Vault / AWS KMS providers | `crypto_shred.rs` | flag/lifecycle tests, no live KMS | **UNVERIFIED (live)** |
| 32 | Execution DAG, Merkle, planner | `execution/*` | `execution_test` | **VERIFIED** |
| 33 | FinOps ledger / spend velocity | `core/src/finops.rs`, handlers | `spend_velocity_test` | **PARTIAL (process-local budget)** |
| 34 | Learning / policy promotion | `gateway/src/learning/*` | `learning_controller_test` | **VERIFIED (in-process)** |
| 35 | Provider adapters (OpenAI-compatible, Anthropic, Gemini, Grok-compatible) | `origin/*` | **mocked** `provider_integration_test` | **UNVERIFIED (live)** |
| 36 | Arbitrage / failover | `origin/provider.rs` | mock 429 failover | **PARTIAL** — no live 429/503 proof |
| 37 | Next.js control plane / BFF | `frontend/src/app/api` | 37 tests incl. E2E | **VERIFIED (local)** |
| 38 | WCAG 2.2 AA | `frontend/tests/accessibility_wcag.test.mjs` | static token/landmark assertions | **PARTIAL** — not axe/browser conformance |
| 39 | Production fail-closed readiness | `state.rs::verify_production_readiness` | `production_startup_test` | **VERIFIED (config gates)** |
| 40 | Disaster rebuild | `apps/sunborn-rebuild` | `disaster_recovery_test` (in-memory) | **PARTIAL** — trust gaps below |
| 41 | Benchmarks / performance claims | `cache/benches`, `gateway/tests/performance_test.rs` | executed | **PARTIAL** — microbenchmark only |
| 42 | SBOM / supply chain | `sbom.cyclonedx.json` | `cargo audit` 0 | **PARTIAL** — no signing/SLSA |
| 43 | Multi-replica consistency proof | — | **in-memory, same process** | **ABSENT** |
| 44 | Live-infra integration suite | — | none in CI | **ABSENT** |
| 45 | Formal verification (TLA+/Alloy) | — | none | **ABSENT** |

---

## 4. Claims vs. evidence

| Claim (2026-11-15 docs) | What the code actually shows | Verdict |
|---|---|---|
| **9.71/10 across 40 dimensions** | Self-assigned in one commit (`492fc57`); the same repository's prior audit rates ~5.1/10; no external reviewer | **UNSUPPORTED** |
| **"Atomic two-phase purge"** | Sequential deletes with best-effort error collection; no prepare/commit, no rollback, no retry journal; `PurgeReport` is discarded on error (`orchestrator.rs`). `purge_documents` sets `bumped_epoch: 1`, `watermark_advanced: false` (`orchestrator.rs:221-222`) | **OVERSTATED** |
| **"Durable transactional outbox, at-least-once"** | Worker calls `let _ = orch_clone.purge_sources(...)` and then `mark_outbox_processed` unconditionally (`state.rs:544-548`); admin invalidation inserts `status: "processed"` (`handlers.rs:1445`). No claim/lease/idempotency. | **BROKEN** |
| **"Persistent monotonic epoch authority"** | `persist_epoch` errors are swallowed with `let _ =` (`epoch.rs:140`); DB upsert is last-writer-wins, not `GREATEST`; hydration is fire-and-forget (`state.rs:191`); no cross-replica refresh | **BROKEN across replicas** |
| **"Redis Streams cluster transport"** | Actually Redis **Pub/Sub** (`PUBLISH`/`SUBSCRIBE`), which is at-most-once and has no replay; factory silently falls back to in-memory (`cluster_sync.rs:218`); multi-replica test uses `InMemoryClusterTransport` (`multi_replica_consistency_test.rs:13`) | **MISLABELED / SIMULATED** |
| **"100% precision on held-out data"** | 12 pairs authored in-repo, embedded with `include_str!` into a unit test; 5 EQUIVALENT / 7 HARD_NEGATIVE, but doc table says TP=3, TN=8, FN=1 (`15_AI_ML...md:24-27`), which does not match the labels; test asserts only FP=0 and recall ≥ 0.20; the doc's own Wilson lower bound is **43.8%** | **UNSUPPORTED / SELF-REFERENTIAL** |
| **"Structural AST guardrails"** | Whitespace tokenization + keyword/prefix vocabularies; no parser or AST library. `StructuralEntailmentScorer` remains a cosine→probability heuristic and the reason code is still `STAGE2_CROSS_ENCODER_CONTRADICTION_DETECTED` | **MISLABELED** |
| **"Dual-admin cryptographic signature verification"** | Two static API keys supplied as headers; hash strings compared with `==`; enforced by default only in production (`auth.rs:55-82`) | **OVERSTATED** |
| **"1.03M ops/s GDSF L1"** | Benchmark uses `MemoryExactCache::new()` (`benches/throughput_test.rs:7`), i.e. the plain `HashMap` path, not `with_byte_budget`/GDSF; one key, single thread, no eviction, no network | **MISATTRIBUTED** |
| **"14,500 req/s, P99 < 4.8 ms gateway"** | `performance_test.rs` measures in-process `oneshot` calls; it prints throughput but asserts only P99 (<5 ms release, <15 ms debug) | **UNSUPPORTED** |
| **"Singular migration authority"** | SQL files are the compile-time source via `include_str!` (good), but the runner reads an existing checksum and never compares it, so silent migration drift is possible (`postgres.rs:239`) | **PARTIAL** |
| **"344 files, ~42,500 LOC; moka/qdrant-client/tokenizers/petgraph/... "** | 391 tracked files; 35,204 Rust LOC; none of `moka`, `qdrant-client`, `tokenizers`, `petgraph`, `deadpool-postgres`, `prometheus`, `clap`, `hyper` appear in any manifest | **INACCURATE INVENTORY** |
| **"Zero CVEs"** | `cargo audit --json` → 0 vulnerabilities, 0 warnings | **VERIFIED** |
| **"Zero clippy warnings / formatted"** | clippy 0/0; `cargo fmt --check` clean | **VERIFIED** |
| **"No fake frontend telemetry"** | 37 tests pass; BFF fail-closed and simulator-removal tests exist | **VERIFIED (local)** |
| **"Trust never increases after rebuild"** | Rebuild derives a verifying key from an optional secret and otherwise falls back to a **hardcoded deterministic seed** (`apps/sunborn-rebuild/src/main.rs:117`); in non-production it admits unsigned records; epochs/graph edges are not restored | **NOT ENFORCED** |

---

## 5. Critical findings (P0)

### P0-1 — The durable outbox can silently lose invalidations
- `state.rs:534-548` polls `outbox_events` every 500 ms, calls
  `let _ = orch_clone.purge_sources(...)`, then `let _ = mark_outbox_processed(...)`.
  A failed purge is therefore marked processed and never retried.
- `handlers.rs:1445` writes admin invalidation rows with `status: "processed"`, so
  those rows are not pending and the worker never consumes them.
- There is no `SELECT ... FOR UPDATE SKIP LOCKED`, no lease/claim, no idempotency
  key, no attempt counter, and no DLQ row in the database.
- **Contradicts:** "at-least-once", "no silent drops", "durable transactional outbox".
- **Required:** one DB transaction writing mutation + PENDING event; claim with
  `FOR UPDATE SKIP LOCKED`; process; mark PROCESSED only on full success; persist
  DLQ with payload/reason; crash-window test (kill between claim and ack).

### P0-2 — Epochs are neither durable nor globally monotonic
- `epoch.rs:140`: `let _ = p.persist_epoch(...)` — a DB failure still reports a bumped
  epoch to the caller, then disappears on restart.
- `postgres.rs::persist_epoch` uses `ON CONFLICT DO UPDATE SET current_epoch = EXCLUDED`,
  not `GREATEST(current_epoch + 1, excluded)`. Two replicas hydrated at 5 both write 6;
  one bump is lost.
- `state.rs:191` hydrates asynchronously and ignores failure; requests can be served
  before hydration completes.
- **Contradicts:** "persistent monotonic epoch authority", "prevents stale writeback
  across restarts", "multi-replica consistency".
- **Required:** atomic DB increment returning the new epoch; fail the request if the
  persistence write fails; synchronous hydration at startup; cross-replica bump
  propagation; two-replica concurrent-bump test and restart test.

### P0-3 — Multi-replica invalidation is simulated, not proven
- The "proof" test constructs two engines in one process over
  `InMemoryClusterTransport` (`multi_replica_consistency_test.rs:13`).
- `create_cluster_transport` returns in-memory silently when Redis is unavailable
  (`cluster_sync.rs:218`), with no error and no readiness check.
- Redis Pub/Sub is at-most-once: a pod disconnected during publish never learns of
  the invalidation; there is no stream, consumer group, cursor, or replay.
- **Contradicts:** "proved multi-replica consistency", "Redis Streams".
- **Required:** Redis Streams + consumer group + per-pod ack/cursor (or a durable
  log); production startup fails if the cluster bus is not durable; live two-process
  test with a forced disconnect/reconnect and missed-message replay.

### P0-4 — Semantic safety evidence is self-authored and insufficient
- The "held-out" dataset is 12 hand-written pairs in the same repository, compiled
  into the unit test with `include_str!`; it is neither independent nor hidden.
- `15_AI_ML...md` reports TP=3, FP=0, TN=8, FN=1, but the dataset contains
  5 EQUIVALENT and 7 HARD_NEGATIVE pairs. The reported confusion matrix cannot be
  derived from the file.
- The doc prints a Wilson 95% lower bound of **43.8%** while claiming 100% precision.
  With 3 positives, that interval is consistent with poor real-world precision.
- The test asserts `false_positives == 0` and `recall >= 0.20` only. It does not
  assert a lower confidence bound, utility, or abstention rate.
- The verifier is lexical token/vocabulary matching, not AST parsing or entailment;
  default dev embeddings are LSH hashes, and no real-model evaluation exists.
- **Required:** external/pre-registered held-out set (n ≥ 300, balanced strata,
  multiple annotators), real embeddings/model, Clopper–Pearson lower bound on
  precision meeting a stated target, utility measurement (hit rate, cost saved,
  abstention), and a report of the lower bound — not the point estimate. Rename
  "AST" unless a real parser is introduced.

### P0-5 — Rebuild does not enforce trust monotonicity
- `apps/sunborn-rebuild/src/main.rs:97-120`: if no public key is configured, it
  derives a verifying key from `SUNBORN_CERTIFICATE_SIGNING_KEY`, and if that is
  absent, from the literal `default-sunborn-deterministic-attestation-seed`.
  In production this can validate certificates signed with a public default key.
- The gateway's production readiness does require a strong certificate signing key
  (`state.rs:813-825`), but the rebuild CLI never calls that gate.
- In non-production, records without a certificate are rebuilt anyway and marked
  `provenance_verified: false`; the doc's universal "trust never increases" claim
  is therefore false.
- Epochs are not restored; graph edges are not rebuilt from durable edges; there is
  no manifest of skipped/failed/reason counts; vector failure after cache insert
  leaves a partial recovery.
- **Required:** production rebuild refuses to start without an explicit public key
  (and rejects the default seed); restore epochs before admitting; rebuild edges;
  emit a signed reconciliation manifest; property test
  `admitted_rebuilt ⊆ verified_cert ∧ epoch_read == epoch_store`.

### P0-6 — "Atomic" purge is best-effort and loses failure detail
- `InvalidationOrchestrator::purge_sources` deletes cache, then vector, then edge,
  per key, with no transaction, prepare phase, rollback, or retry journal.
- On any failure it returns `SunbornError::internal("... N store failures")` and
  **drops the `PurgeReport`**, so callers cannot see which keys failed
  (`orchestrator.rs`).
- Edge removal checks the entire `failed_keys` vector, so a failure for one source
  can prevent cleanup for another source's key.
- `purge_documents` does not bump the epoch/watermark at all and broadcasts epoch 1.
- **Required:** return structured failure (`PurgeReport`) in the error; persist a
  purge intent; two-phase commit or compensating retry; epoch/watermark fence in
  every purge path; partial-failure test asserting exact failed keys and retention.

---

## 6. High findings (P1)

1. **Provider adapters are not integration-tested.** OpenAI-compatible, Anthropic,
   Gemini, and Grok (OpenAI-compatible endpoint) adapters exist, but
   `provider_integration_test.rs` uses `MockProviderWithStatus`. There are no
   recorded-fixture contract tests, no live gated tests, and no schema validation
   for provider error bodies. The 429 failover test proves the router, not the wire.
2. **No live infrastructure tests.** No test starts PostgreSQL, Dragonfly, Qdrant,
   Neo4j, Vault, or KMS. `production_startup_test` points `POSTGRES_URL` at an
   unreachable value to prove fail-closed behavior; that is useful but not a live
   integration test.
3. **Migration drift is undetected.** The runner selects `checksum` for an existing
   version and never compares it to the embedded SQL (`postgres.rs:239`). Editing an
   applied migration is silent.
4. **Quorum is not cryptographic and not constant-time.** Two bearer API keys in
   headers; hashes compared with `==` (`auth.rs:73`). D-10's "all auth checks use
   `subtle::ConstantTimeEq`" is overstated.
5. **API key storage is unsalted SHA-256.** `ApiKeyRegistry::hash_key` uses a single
   fast SHA-256. If the credential store leaks, low-entropy keys are cheap to
   brute-force. Prefer a keyed hash (HMAC with a server pepper) or Argon2id.
6. **Per-process coordination.** Rate limits, spend velocity, learning policy,
   watermarks, and epochs are process-local. Horizontal scaling multiplies limits
   and can overspend; the docs acknowledge only epochs.
7. **Memory vector search is O(n·d).** The default vector store is an in-process
   linear scan; Qdrant is optional and unverified live. There is no ANN fallback.
8. **Benchmark claims are not production measurements.** The L1 bench is a
   single-key, single-thread HashMap loop (and not GDSF). The gateway test is
   in-process `oneshot` with no socket, TLS, upstream call, or multi-tenant load.
   The 14,500 req/s and 4.8 ms P99 figures are not produced by any executed gate.
9. **"Capacity soak" is not a soak.** `capacity_soak_test` finishes in ~0.11 s.
10. **Supply chain is partial.** SBOM exists and `cargo audit` is clean, but there
    is no artifact signing, SLSA provenance, or pinned-action policy; CI workflows
    use tags (`actions/checkout@v4`) rather than commit SHAs.
11. **Attestation semantics are narrower than the docs imply.** Ed25519 proves
    issuer integrity, not answer correctness. The certificate "Merkle root" is a
    custom 5-leaf construction, not RFC 6962; the execution tree uses domain
    separation but duplicates odd leaves rather than promoting them.
12. **Unsafe-code claim is effectively true.** The only match is a test string
    (`policy-unsafe-threshold`); production Rust contains no `unsafe` blocks.
    This is a genuine strength.

---

## 7. Distributed-state inventory

| State | Location | Shared across replicas? | Current consistency | Required |
|---|---|---|---|---|
| L1 exact cache | process heap | No | per-pod | Dragonfly or epoch fencing |
| Graph adjacency | process heap | No | per-pod | durable graph + stream |
| Roaring doc→record | process heap | No | per-pod | rebuildable projection |
| Epochs | `HashMap<Uuid, AtomicU64>` | **No** | resets/last-writer-wins | DB atomic counter + propagation |
| Watermarks | `DashMap` | **No** | per-pod | shared generation column |
| Outbox | mpsc + DB rows | Partially | worker ignores failures | claim/lease/ack + DLQ |
| Vector index | Qdrant (optional) | Yes if configured | unverified live | live tests + backups |
| Spend budget | in-process | **No** | per-pod overspend | Redis/DB atomic reservations |
| Rate limits | in-process | **No** | N× configured | shared limiter |
| Metrics | atomics | **No** | per-pod | Prometheus aggregation |
| Learning policy | in-process | **No** | divergent | versioned DB policy |
| API keys | in-process + startup hydrate | Partially | stale until restart | DB lookup/pub-sub invalidation |

**Conclusion:** single-replica behavior is plausible; multi-replica consistency is
not demonstrated. The two-replica test must use separate processes and a durable
transport before any production claim.

---

## 8. Workstream scorecard

Scores are directional, evidence-weighted, and deliberately conservative.
They are **not** a substitute for independent review.

| Workstream | Score | Basis |
|---|---:|---|
| Backend / API | 6.5 | 49 real routes, auth, typed errors, tests; outbox/purge correctness gaps |
| Frontend / product | 6.0 | Real BFF, fail-closed tests, no simulator; WCAG evidence is static |
| Distributed systems | 3.0 | In-memory "cluster", non-atomic epochs, no live multi-process proof |
| Database / storage | 5.0 | Real SQL, migrations embedded; no live DB test, no drift check, outbox broken |
| Security | 5.5 | Auth/quorum/prod gates good; unsalted hashes, per-process limits, mocked live paths |
| Cryptography | 6.0 | AES-GCM, Ed25519, zeroize, canonicalization are real; attestation overclaims |
| AI/ML research | 3.0 | Lexical heuristic, 12-pair self-authored fixture, seeded calibration |
| SRE / observability | 4.5 | Health/metrics exist; per-pod metrics, no live runbooks exercised |
| QA / testing | 6.0 | 270+37+26 pass, clippy/fmt clean; tests mostly same-process/mocked |
| Performance | 4.0 | Credible microbenchmark; no credible end-to-end/live throughput evidence |
| Technical product / docs truth | 3.0 | 9.71 self-score, mutually inconsistent audits, wrong inventory/deps |
| Formal methods | 1.5 | No TLA+/Alloy/refinement proofs |
| Supply chain / DevOps | 5.0 | SBOM + audit clean; no signing/SLSA, unpinned actions |
| **Overall (weighted)** | **5.3** | **Functional prototype; single-replica pilot only** |

---

## 9. Standards mapping (gaps only)

| Standard | Clause / concern | Status |
|---|---|---|
| ISO/IEC 25010 | reliability, security, maintainability | Partial — no multi-replica proof; duplicated narrative |
| NIST SP 800-218 (SSDF) | PO.3, PS.1, PW.7, RV.1 | Partial — SBOM/audit; no signed provenance or independent review |
| OWASP ASVS 4.0 | V2, V3, V6, V11 | API auth strong; session/CSP and shared limiting not evidenced |
| OWASP API Top 10 | API1/API4/API8 | Tenant isolation/model RBAC good; no shared limit proof |
| IEEE 1012 | V&V independence | Not met — self-audit; no independent verifier |
| IEEE 29119 / 730 | test documentation/traceability | Partial — tests exist; traceability tables cite stale names |
| RFC 8785 | JSON canonicalization | Implemented with fuzz tests (good) |
| RFC 6962 | Merkle inclusion | Partial — domain separation present, odd-leaf strategy differs |
| WCAG 2.2 AA | accessibility | Partial — static assertions, not full conformance audit |
| SLSA | build provenance | Absent |
| GDPR Art. 17 | crypto-shredding | Implemented and tested (good) |

---

## 10. Ordered remediation plan

### P0 — must close before any production claim
1. **Outbox:** transactional write, `SKIP LOCKED` claim, ack only on success,
   DB-backed DLQ, idempotency key, crash-window test.
2. **Epochs:** atomic DB increment returning the new value; fail closed on persist
   error; synchronous hydration; propagate bumps; restart + two-replica tests.
3. **Cluster:** Redis Streams consumer group with cursor/ack; production startup
   fails without durable bus; live two-process disconnect/replay test.
4. **Semantic evidence:** external held-out set, real model, pre-registered
   threshold, Clopper–Pearson lower bound, utility/abstention report; rename AST.
5. **Rebuild:** explicit public key required in production; reject default seed;
   restore epochs/edges; signed reconciliation manifest; trust-monotonicity test.
6. **Purge:** return `PurgeReport` in errors; intent journal + retry; epoch/watermark
   before delete in every path; exact partial-failure assertions.

### P1 — production hardening
7. Live integration suites (testcontainers) for PostgreSQL, Dragonfly, Qdrant,
   Neo4j, Vault/KMS; provider contract fixtures + gated live tests.
8. Migration checksum comparison and refuse-on-drift.
9. Shared rate limiting and spend reservations; per-pod metrics aggregation.
10. Keyed/Argon2id API-key storage; constant-time quorum comparison; real MFA/
    signature quorum if "cryptographic sign-off" is claimed.
11. ANN backend or explicit disclosure that semantic search is linear.
12. End-to-end load test over a real socket with upstream latency; publish
    reproducible artifacts; stop quoting in-process numbers as gateway capacity.
13. SBOM signing, SLSA provenance, pinned GitHub Actions.

### P2 — truth and sustainability
14. Collapse the two competing audit narratives into one SSOT; retire or annotate
    the 9.71 claims; correct file/LOC/dependency inventories.
15. Replace static WCAG checks with axe/Playwright conformance runs.
16. Add a long-duration soak (hours), fault injection with real processes, and a
    documented restore drill.
17. Add formal invariants (TLA+ or Alloy) for epoch monotonicity, outbox
    at-least-once, and trust monotonicity; model-check them in CI.

---

## 11. Production readiness decision

| Gate | Status | Blocking finding |
|---|---|---|
| A. Secrets / key separation | PASS | Production readiness rejects weak/default keys |
| B. No simulated defaults in prod | PASS | Mock origin, LSH embeddings, memory backends rejected |
| C. Durable persistence | PARTIAL | No live DB test; migration drift unchecked |
| D. Distributed invalidation | **FAIL** | P0-2, P0-3 |
| E. Invalidation completeness/visibility | **FAIL** | P0-1, P0-6 |
| F. Semantic safety measured | **FAIL** | P0-4 |
| G. Attestation claims accurate | PARTIAL | Issuer integrity is real; correctness is not proven |
| H. Recovery trust monotonicity | **FAIL** | P0-5 |
| I. Live provider integration tests | **FAIL** | Mocked only |
| J. Reproducible live benchmarks | **FAIL** | Microbenchmark/in-process only |

**Decision: NO-GO for production.**

**Permitted:** continued development and a single-replica, non-safety-critical pilot
with written disclosure that semantic reuse is uncalibrated, cross-pod invalidation
is not active, and rebuild trust depends on operator key configuration.

---

## 12. What is genuinely good (do not lose this)

- 270 Rust tests, 37 frontend tests, 26 Python tests pass; clippy and fmt clean.
- Real Ed25519 signing/verification with key separation and production secret gates.
- AES-256-GCM envelope encryption, zeroization, and tested crypto-shredding.
- RFC 8785 canonicalization with fuzz tests.
- Deterministic Qdrant point IDs used consistently for insert and delete.
- Graph cache + Roaring lineage + exact/vector purge wired on the main path.
- Frontend BFF fails closed and no longer fabricates live telemetry.
- `cargo audit` clean with a zero-suppression policy.

These are the foundations to build on. The path to a defensible production system is
not more self-scoring; it is closing the six P0s with independent, reproducible
evidence and deleting claims the code cannot support.

---

## 13. Audit limitations

- No live infrastructure was provisioned; all storage/KMS/provider rows are
  code-level assessments.
- No penetration test, fuzzing campaign, or multi-hour soak was run.
- Tests were executed on Windows/Git Bash; CI runs on Ubuntu, so platform-specific
  differences remain possible.
- The 35k LOC were reviewed by tracing runtime-critical paths and running the full
  suite, not by line-by-line review of every helper.
- The "simulated organisation" is a workstream structure, not independent humans.
- No files were committed or pushed. `git status` was clean before this report;
  only this document was added.
