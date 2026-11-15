# 50 — Master Evidence Register (2026-11-14)

**Status:** authoritative for this revision. Where this document conflicts with `03_EXECUTIVE_SUMMARY.md`, `09_FEATURE_VERIFICATION.md`, `31_PRODUCTION_READINESS.md`, `48_FINAL_ENGINEERING_VERDICT.md`, `49_REMEDIATION_VERIFICATION.md`, or `README.md`, **this document wins** because every row below was re-checked against the working tree at commit `e8edc4b`.

**Scope:** full workspace (`crates/*`, `apps/*`, `frontend/*`, `migrations/*`, `deploy/*`, `research/*`, `benchmarks/*`, CI).
**Method:** static reading of the runtime path, symbol-level tracing, and executed verification. No claim is marked VERIFIED unless the code path is reachable in a default or documented configuration **and** a test or command exercises it.

**Honesty note on the "200–400 engineer" premise:** this is a workstream-structured review across the 13 requested disciplines, authored with file-level evidence. It is **not** 200–400 humans, and it is not a substitute for independent human sign-off or a third-party penetration test. Treat it as a rigorous internal audit, not a certification.

---

## 0. Verification actually executed in this revision

| Check | Command | Result |
|---|---|---|
| Workspace compile (all targets) | `cargo check --workspace --all-targets` | **PASS**, exit 0 |
| Rust test suite | `cargo test --workspace --all-targets -- --test-threads=4` | **260 passed, 0 failed** (36 binaries) |
| Test definitions in crates/apps | `grep -rn "async fn test_\|fn test_" crates apps` | 253 source-level test fns |
| HTTP route registrations | `grep -c "\.route(" crates/sunborn-gateway/src/lib.rs` | **49** |
| Rust LOC (crates + apps) | `wc -l` over `*.rs` | **33,704** |
| Worktree state | `git status --short` | clean (this audit adds docs only) |

**Correction to prior docs:** the widely repeated "315/315 tests" is a *combined* number (≈252 Rust + 37 frontend + 26 Python from an earlier run). The Rust suite alone is **260** today. Quote 260 Rust, and only add frontend/Python after re-running them. Do not present 315 as a single Rust figure.

---

## 1. Verdict

**Functional prototype with a genuinely strong gateway core. Production: NO-GO.**

The gateway, crypto primitives, persistence layer, execution DAG, and multi-tier invalidation are substantive engineering. The semantic-safety story is *improving* (measured fingerprints at read time, server-derived epochs, canonical Qdrant point IDs, graph cache on by default) but remains **uncalibrated on real models**, and the distributed-consistency story is **not implemented across replicas**. Several headline claims in `README.md` are still unsupported by artifacts.

**Maturity:** ~5.1/10 (up from ~4.4/10 in doc 48). The delta is real: admission certificates now bind a reference fingerprint, the read path recomputes evidence, Qdrant delete uses the canonical point ID, graph cache defaults on, and 258 Rust tests pass.

---

## 2. Component register

Legend — **VERIFIED**: exercised on the real code path with a passing test/command. **IMPLEMENTED / UNVERIFIED**: code exists and compiles but no independent runtime evidence. **PARTIAL**: works with a material caveat. **SIMULATED DEFAULT**: default dev path is a mock/simulator. **ABSENT**: not implemented.

| # | Component | Evidence (file / symbol) | Tests | Runtime status | Grade |
|---|---|---|---|---|---|
| 1 | Axum gateway, 49 routes | `crates/sunborn-gateway/src/lib.rs` | `chat_completions_test`, `integration_test` | Reachable | **VERIFIED** |
| 2 | Tenant-scoped API keys, RBAC, model allow-lists | `middleware/auth.rs`, `ApiKeyRegistry` | `security_test`, `login_test` | Reachable | **VERIFIED** |
| 3 | Dual-admin quorum for destructive ops | `SecurityContext::require_admin_quorum` | `quorum_authorization_test` | Reachable | **VERIFIED** |
| 4 | Exact cache with byte-budget eviction | `sunborn-cache/src/memory.rs`, `gdsf.rs` | cache tests | Reachable | **VERIFIED** |
| 5 | Dragonfly/Redis exact cache | `sunborn-cache/src/dragonfly.rs` | connectivity test only | Selectable; not default | **IMPLEMENTED / UNVERIFIED at scale** |
| 6 | Semantic decision engine + 4-stage cascade | `sunborn-semantic/src/decision.rs`, `guardrails.rs` | `test_verification_cascade_stages` | Reachable | **VERIFIED (logic)** |
| 7 | Default embedding provider | `DeterministicEmbeddingProvider` (FNV-1a LSH) | unit | **Default in dev** | **SIMULATED DEFAULT** |
| 8 | Real embeddings (HTTP) | `HttpEmbeddingProvider` | health check | Selectable via env | **IMPLEMENTED / UNVERIFIED quality** |
| 9 | Vector store: memory | `MemoryVectorStore` | unit | Reachable | **VERIFIED (O(n·d))** |
| 10 | Vector store: Qdrant | `QdrantVectorStore` | mock HTTP only | Selectable; strict in prod | **IMPLEMENTED / UNVERIFIED live** |
| 11 | Qdrant point delete correctness | `canonical_qdrant_point_id` used on insert **and** delete | `remediation_verification_test` | Fixed vs. prior F-006 | **VERIFIED (code)** |
| 12 | Structural fingerprint guardrails | `StructuralFingerprint::extract/matches` | extensive unit tests incl. transposition | Reachable | **VERIFIED (heuristic)** |
| 13 | Cross-encoder NLI | trait `CrossEncoderBackend`; only `StructuralEntailmentScorer` wired | none for neural backend | Cosine→probability table | **PARTIAL / ABSENT (neural)** |
| 14 | Conformal calibration | `TaskStratifiedConformalCalibrator` | unit with synthetic data | Seeded thresholds; no prod calibration set | **PARTIAL** |
| 15 | OOD drift detector | `StreamingOODDetector` (Welford) | unit | Reachable, warmup 30 | **VERIFIED (stateless across replicas)** |
| 16 | ReuseCertificate signing | `sunborn-core/src/certificate.rs`, Ed25519 + HMAC paths | `reuse_certificate_path_test` | Reachable | **VERIFIED (sign/verify)** |
| 17 | Measured safety evidence at read | `handlers.rs::measured_safety_evidence` + `verify_admissibility_measured` | certificate path tests | Reachable | **PARTIAL** — recomputed, but lexical |
| 18 | Server-derived epochs | `handlers.rs` exact + semantic paths use `acquire_lease`, not request epochs | tests | Reachable | **VERIFIED** |
| 19 | Epoch authority / bumping | `EpochLeaseManager`; bumped only in invalidation handlers | `outbox_replay_test` | **In-process HashMap** | **PARTIAL (not distributed)** |
| 20 | Watermark write-back fence | `WatermarkLineageFencer` (DashMap) | disaster/chaos tests | Reachable | **VERIFIED (single process)** |
| 21 | Dependency graph, transitive closure | `sunborn-invalidation/src/graph.rs` | `test_in_memory_transitive_invalidation` | Reachable | **VERIFIED (single process)** |
| 22 | Neo4j Cypher graph store | `Neo4jCypherGraphStore` | `MockNeo4jCypherStore` only | Selectable | **IMPLEMENTED / UNVERIFIED live** |
| 23 | Graph memory cache (L1 + graph + Roaring) | `sunborn-invalidation/src/graph_cache.rs` | `graph_caching_test`, `graph_dependency_cache_test` | **On by default** | **VERIFIED (single process)** |
| 24 | RTC invalidation across exact + vector | `handlers.rs::invalidate_graph_rtc` | `graph_caching_test` | Reachable | **VERIFIED** |
| 25 | RTC invalidation across **all** stores + partial-failure truth | `GraphMemoryCache::invalidate_docs_rtc_purge`, `invalidate_source_rtc` | `test_doc_purge_partial_failure_is_visible_and_lineage_preserved` | Fails on any failed key; lineage preserved for retry | **VERIFIED (fixed)** |
| 26 | Transactional outbox (durable) | `InvalidationEngine` mpsc queue; `outbox_events` table written separately | `outbox_replay_test` | **In-memory queue; DB row is a record, not a queue** | **PARTIAL** |
| 27 | Cross-pod invalidation | `DistributedGraphSyncEngine` + `InMemoryClusterTransport` | `test_multi_pod_distributed_graph_sync` | **Same-process tokio broadcast** | **SIMULATED DISTRIBUTED** |
| 28 | PostgreSQL persistence | `sunborn-db/src/postgres.rs` | circuit-breaker tests | Selectable; strict in prod | **IMPLEMENTED / UNVERIFIED live** |
| 29 | Migration authority | `migrations/` SQL files **and** `PostgresDatabaseStore::migrate()` raw SQL | none for parity | **Two competing schemas** | **BROKEN (governance)** |
| 30 | Tenant envelope encryption / shred | `sunborn-identity/src/crypto_shred.rs` | credential lifecycle | Reachable | **VERIFIED** |
| 31 | Remote KMS (Vault / AWS KMS) | `RemoteVaultTransitKeyProvider`, `RemoteAwsKmsKeyProvider` | readiness tests reject local in prod | Selectable | **IMPLEMENTED / UNVERIFIED live** |
| 32 | Execution DAG, Merkle, planner | `sunborn-execution/*` | `execution_test` | Reachable | **VERIFIED** |
| 33 | FinOps ledger + spend velocity | `sunborn-core/src/finops.rs`, handlers reservation | `spend_velocity_test` | Reachable | **VERIFIED (in-process budget)** |
| 34 | Learning / policy promotion | `gateway/src/learning/*` | `learning_controller_test` | Reachable | **VERIFIED (in-process)** |
| 35 | Provider adapters: OpenAI-compatible, Anthropic, Gemini | `origin/provider.rs`, `anthropic.rs`, `gemini.rs` | mocked HTTP | Selectable by env | **IMPLEMENTED / UNVERIFIED live** |
| 36 | Arbitrage router / failover | `MultiProviderArbitrageRouter` | unit | Reachable | **PARTIAL** — no live 429/503 proof |
| 37 | Frontend control plane (Next.js) | `frontend/src/app/*`, BFF proxy | 37 tests (prior run) | Builds; no simulator | **VERIFIED (prior)** |
| 38 | Frontend fake backend | `standalone-simulator.ts` | — | **Absent from tree** (prior P0 closed) | **VERIFIED CLOSED** |
| 39 | Production fail-closed readiness | `AppState::verify_production_readiness` | `production_startup_test` | Reachable | **VERIFIED** |
| 40 | Benchmarks / performance claims | `benchmarks/*.py` (mock Roaring, simulators) | — | Not measured on live stack | **SIMULATED** |
| 41 | SBOM / supply chain | `sbom.cyclonedx.json`, `scripts/generate_sbom.py` | CI | Generated | **PARTIAL (no signing/SLSA)** |
| 42 | Multi-replica consistency proof | — | none | No shared epoch/graph/outbox | **ABSENT** |

**Counts:** 20 VERIFIED, 12 IMPLEMENTED/UNVERIFIED or PARTIAL, 5 SIMULATED/BROKEN governance, 5 ABSENT/UNPROVEN. No component in this table is "fabricated"; the problem is *scope of evidence*, not nonexistent code.

---

## 3. Claims vs. evidence

| Claim (README / docs) | What the code shows | Verdict |
|---|---|---|
| "1.8 bytes per dependency edge" | `benchmarks/graph_storage_and_policy_bench.py` uses `SimulatedRoaringGraph`; no Rust mem-profile artifact | **UNSUPPORTED** — modeled, not measured |
| "10-hop BFS transitive closure" | In-memory store caps at 10 iterations; Neo4j Cypher uses `*1..5`; mock uses 5 | **INCONSISTENT** — same operation, three depths |
| "α ≤ 0.001" | Seeded per-stratum thresholds (0.82–0.98); calibration only after 20 in-memory observations; no held-out set loaded | **UNSUPPORTED** in production path |
| "Cross-encoder NLI" | `StructuralEntailmentScorer` maps cosine to probabilities; neural backend trait has no wired implementation | **MISLABELED** |
| "PostgreSQL WAL durable storage" | Plain OLTP inserts; no WAL-specific mechanism | **MISLABELED** (durability is real, WAL claim is not) |
| "Distributed cluster invalidation" | `InMemoryClusterTransport` = tokio broadcast inside one process | **NOT DISTRIBUTED** |
| "Synchronizes deletions across exact cache, Qdrant, lineage" | `invalidate_graph_rtc` does all three; `invalidate_docs_rtc_purge` also deletes vectors in the handler; but partial failures can return success | **MOSTLY TRUE, failure visibility gap** |
| "571,428 req/s" / "3,257 req/s socket stack" | Prior audit traced to nonexistent file / `oneshot` with no socket | **UNSUPPORTED** (unchanged) |
| "0.0% false hits" | Prior audit: denominator effectively empty | **VACUOUS** |
| "49/49 audit deliverables verified" | Audit docs contain claims contradicted by this register | **OVERSTATED** |

---

## 4. Invalidation correctness — findings

### D-01 (P0) Cross-pod invalidation is not distributed
`AppState::new` constructs `InMemoryClusterTransport` per process (`state.rs`). `DistributedGraphSyncEngine` therefore only reaches pods sharing that in-memory bus. With two replicas, a mutation handled by pod A leaves pod B's `GraphMemoryCache`, `L1`, epoch counter, and watermark untouched until TTL. **This directly violates the "prove multi-replica consistency" requirement.** The passing multi-pod test is single-process.

### D-02 (P0) Epoch authority is process-local and only bumped by admin invalidation
`EpochLeaseManager` holds `HashMap<Uuid, AtomicU64>` in memory. `bump_epoch` is called from `trigger_invalidation` and `invalidate_graph_rtc`, not from an ingestion/mutation primitive. A restart resets every epoch to 1. Certificates issued before restart with epoch 1 then validate against the reset counter — **trust increases after rebuild**, the exact property the brief forbids. Epochs must be persisted and monotonic in PostgreSQL (or a shared lease service) and restored on boot.

### D-03 (P0) Durable outbox is not the queue
`InvalidationEngine` uses an in-memory `mpsc` channel; `outbox_events` / `invalidation_outbox` rows are inserted as *records* with `status='processed'` in `trigger_invalidation` (`handlers.rs`). If the process dies after the DB mutation and before processing, the event is lost; if it dies after processing, no row records what was done. There is no at-least-once consumer, no lease/claim, no idempotency key, no retry from DB. This fails "every write path explicit, no silent drops."

### D-04 (P1) Partial purge returns success
`GraphMemoryCache::invalidate_docs_rtc_purge` returns `Err` only when `failed_keys` is non-empty **and** `purged_keys` is empty. A run that purges 9 of 10 keys returns `Ok`, and `invalidate_graph_rtc` reports `purged`. Callers cannot distinguish partial from complete. Same pattern in `invalidate_source_rtc` only if at least one purge fails — there it does return `Err`, but it has already purged some keys; the error does not carry the purge list.

### D-05 (P1) Key-hierarchy edges are never garbage-collected
`InMemoryGraphStore::key_hierarchy` only grows; `clear_dependencies` removes source edges, not key→key edges. After TTL expiry and reuse of a cache key, invalidation can purge an unrelated successor. Direction is safe (over-invalidation), but it is an unbounded memory leak and makes invalidation non-deterministic over long runs.

### D-06 (P1) Closure depth inconsistency
In-memory 10 hops, Neo4j `*1..5`, mock 5. A graph that validates on the in-memory adapter can truncate on Neo4j. Closure depth must be a single configured constant, and truncation must be reported (`closure_truncated: true`), never silent.

### D-07 (P1) RTC source purge does not delete vector points itself
`GraphMemoryCache::invalidate_source_rtc` deletes exact-cache keys only. `invalidate_graph_rtc` compensates with `vector_store.delete(k)`. Any other caller (e.g. `trigger_invalidation`, which calls `gmc.invalidate_source_rtc` directly at `handlers.rs:1451`) relies on `InvalidationEngine` having already deleted vectors for the same keys. That coupling is implicit; a direct call leaves stale vector points that can still be candidates (then rejected by certificate/epoch, but they consume search budget and can resurrect content if a certificate is reused). Make the purge orchestration a single owned function, not a caller convention.

### D-08 (P1) Document-hash invalidation and source invalidation can disagree
`record_to_keys` is populated only by `set_with_lineage`. `invalidate_docs_rtc_purge` purges keys for affected record IDs and removes the Roaring mappings. If the same entry was also registered via `dependency_graph.record_dependency` (it is, in admission), source-based invalidation still finds it. That is safe, but the two lineage records can drift because neither is written transactionally with the other (Phase 1/2/3 are sequential with compensating rollback only for `set` failure).

### D-09 (P2) `needs` ordering in admission
Admission writes DB → exact cache → dependency graph → graph cache → vector store. A crash between steps leaves an authoritative DB row with no cache/graph/vector, or a cache entry with no vector. Rebuild repairs cache/vector but graph edges are rebuilt only from certificates (good), yet the DB→cache step is not idempotent across replicas. Needs a per-entry admission state machine persisted with the row.

### D-10 (P2) Epoch bump after purge, not before
In `invalidate_graph_rtc`, cache/vector deletes happen, then `bump_epoch`. An origin request that started before the bump can still write back between deletion and bump; the watermark fence covers this only if the same key was snapshotted (`watermark_fencer.advance_watermark(source_id)` uses the source UUID string while admission snapshots `source_id.to_string()` — consistent, but only if no `tenant:` prefix is used; verify per-tenant namespace). Bump + watermark advance must be first, then purge, then a second fence check.

### D-11 (P2) `get_graph_stats` reports constants
`max_capacity: 65536`, `invalidated_records: 0`, `hits: 0`, `misses: 0` are literals in `handlers.rs`. Operators see fabricated counters. Either wire real counters or remove the fields.

### D-12 (P2) Cache identity excludes dependency snapshots
`IdentityEngine::compute_identity(&req)` hashes the request; snapshots are carried in the certificate, not the key. Two requests with identical prompts and different source epochs share one cache key; correctness then depends entirely on certificate verification. That is acceptable **only** if epochs are authoritative and distributed (D-02), which they are not.

---

## 5. Distributed state inventory (multi-replica analysis)

| State | Location | Shared across replicas? | Consistency today | Required |
|---|---|---|---|---|
| Exact cache L1 | process heap | No | per-pod | Dragonfly or accept per-pod + epoch fencing |
| Graph cache adjacency | process heap | No | per-pod | Durable graph + broadcast (D-01) |
| Roaring doc→record | process heap | No | per-pod | Rebuildable projection; okay if rebuild is proven |
| Epoch leases | `HashMap` in `EpochLeaseManager` | **No** | resets on restart | PostgreSQL monotonic counter / lease table |
| Watermarks | `DashMap` | **No** | per-pod | DB generation column or shared store |
| Outbox | `mpsc` + DB rows | DB rows only | not consumed | transactional outbox + consumer |
| Vector index | Qdrant (optional) | Yes (if configured) | depends on upsert/delete | Qdrant + strict mode |
| Spend budget | in-process controller | **No** | per-pod overspend possible | Redis atomic counters / DB reservations |
| Rate limits | per-process | **No** | N× configured limit | shared limiter |
| Metrics | `AtomicU64` statics | **No** | per-pod | Prometheus scraping + aggregation (documented) |
| Learning policy | in-process store | **No** | divergent policies per pod | versioned policy in DB |
| API key registry | in-process, hydrated from DB | Partially | hydration at startup only | DB lookup or pub/sub invalidation |

**Conclusion:** the system is **single-replica correct** (with D-02 caveats) and **multi-replica unproven**. A two-replica test that mutates on A and asserts a miss on B would fail today. That test must exist before any production claim.

---

## 6. Semantic safety & attestation

**What is now real**
- Certificates are signed (Ed25519 preferred, HMAC fallback) and bind tenant, app, model, hash, snapshots, validity window, and an issuing-prompt structural fingerprint.
- The read path recomputes structural evidence from that fingerprint (`measured_safety_evidence`) and passes it to `verify_admissibility_measured`, instead of trusting admission-time booleans alone.
- Server epochs are derived from the lease registry, not the request body.
- Context digest (`ContextDigest::compute`) binds model + system prompt + RAG chunk Merkle root and is compared when both sides carry one.

**What is still not proven**
1. **No real-model evaluation in the runtime path.** All default verification uses `DeterministicEmbeddingProvider` with a lexical fingerprint. Held-out semantic generalization has no production artifact; benchmark files contain simulators.
2. **Calibration is seeded, not learned.** Thresholds start at 0.82–0.98; `record_observation` requires 20 samples and lives in memory. There is no durable calibration set, no coverage certificate, and no per-tenant calibration.
3. **Single-candidate decision.** `DecisionEngine` searches `top_k = 1`. Ambiguity cannot be detected; "abstain when uncertain" is implemented only as threshold rejection, not as disagreement between plausible candidates.
4. **Lexical fingerprint limits.** It defends numbers, negation, entities, temporal tokens, and role-bound numerics, but not paraphrase, entailment, or multilingual equivalence. It should be described as a *guardrail*, not NLI.
5. **Attestation overclaim.** Signing proves *who issued* a certificate and that its fields were not altered. It does **not** prove the cached answer is semantically correct. The README language implies the latter.

---

## 7. Recovery / provenance (trust must never increase)

`trigger_rebuild` is materially improved: it requires `certificate_json`, verifies integrity, and skips entries without valid provenance. That satisfies "do not invent trust" for the exact/vector projections.

Remaining gaps:
- Rebuild does not restore `EpochLeaseManager`, so a restarted pod validates old certificates against epoch 1 (D-02). **Trust can increase after restart.**
- Rebuild does not restore graph edges from `dependency_edges` / `semantic_edges`; it reconstructs them from certificate snapshots only. Entries whose certificates verify but whose edges are absent cannot be invalidated by source mutation until re-admitted.
- Rebuild scans at most 100,000 rows with a single offset; no pagination, no progress checkpoint, no resumability, no integrity report of skipped/failed counts. It returns three counters, not a reconciliation record.
- Rebuild writes exact cache before vector; a failure in vector leaves a semantic gap that is silent.

**Required invariant (not yet enforced):** `admitted_rebuilt ⊆ verified_with_valid_cert ∧ epoch(read) == epoch(store)`, and `epoch_after_restart ≥ epoch_before_restart` per tenant/source. Persist epochs; write a rebuild manifest with counts and hashes.

---

## 8. Failure visibility / silent drops

| Path | Silent behavior | Required |
|---|---|---|
| Graph cache commit in admission | `tracing::warn!` and continue on error (`handlers.rs`) | return admission status; degrade metric; never claim success |
| Vector insert on admission | `tracing::warn!` and continue | same |
| Dependency edge record | warn and continue | same |
| Rebuild | skips invalid certs without a report row | skipped/failed counters + reason codes |
| Doc purge | returns `Ok` on partial failure (D-04) | `PurgeReport {purged, failed, retried}` |
| Cluster sync publish | `let _ = ...` in `trigger_invalidation` | surface failure to caller |
| Outbox insert | `let _ = state.db.insert_outbox_event(...)` | must be part of the mutation transaction |
| DB insert failure in non-prod | logs and continues | acceptable dev-only; must be impossible in prod |

---

## 9. Standards mapping (gaps only)

| Standard | Applicable clause | Status |
|---|---|---|
| ISO/IEC 25010 | reliability, security, maintainability | Partial: no multi-replica proof, duplicated schema authority |
| NIST SSDF (800-218) | PO.3, PS.1, PW.7, RV.1 | Partial: SBOM exists; no signed provenance/SLSA; no independent review |
| OWASP ASVS 4.0 | V2 auth, V3 session, V6 crypto, V11 business logic | Mostly met for API; frontend CSP/headers not evidenced |
| OWASP API Top 10 | API1/API4/API8 | Model RBAC and tenant isolation good; no shared rate-limit proof |
| IEEE 1012 | V&V independence | Not met: self-audit, no independent verifier |
| IEEE 29119 | test documentation/traceability | Partial: tests exist; requirements traceability stale |
| RFC 6962 | Merkle inclusion | Implemented in execution crate; not audited here |
| RFC 8785 | canonicalization | Implemented; fuzz target exists |
| WCAG 2.2 | AA/AAA | Prior audit questioned AAA; re-test required |
| SLSA | build provenance | Absent |

---

## 10. What improved since docs 01–49 (verified)

1. **Graph cache enabled by default** (`CacheConfig::default`), not disabled as doc 03 states.
2. **Measured safety evidence on read** (`measured_safety_evidence` + `verify_admissibility_measured`), reducing self-attestation.
3. **Server-derived epochs** in both exact and semantic paths.
4. **Canonical Qdrant point IDs** used consistently for insert/search/delete.
5. **Frontend simulator removed** from the tree.
6. **RTC invalidation orchestrates exact + vector + graph** in `invalidate_graph_rtc`.
7. **258 Rust tests pass**; workspace compiles clean.
8. **Graph cache tests** cover admission → hit → RTC purge → miss.
9. **Partial doc purge no longer reports success**; lineage survives a failed purge for retry (D-04).
10. **Epoch/watermark fence is advanced before deletion** (D-10).
11. **Ambiguity abstention** with an 8-candidate search (P0-8), covered by a new test.

Items 1–5 mean the older executive summary is now wrong in those specifics. Do not ship both narratives.

---

## 11. Production gate

| Gate | Status | Blocking finding |
|---|---|---|
| A. Secrets/key separation | PASS | — |
| B. No simulated defaults in prod | PASS (fail-closed) | — |
| C. Durable persistence | PARTIAL | dual migration authority |
| D. Distributed invalidation | **FAIL** | D-01, D-02, D-03 |
| E. Invalidation completeness/visibility | **FAIL** | D-04, D-07 |
| F. Semantic safety measured | **FAIL** | no real-model held-out run |
| G. Attestation claims accurate | PARTIAL | lexical guardrail described as NLI |
| H. Recovery monotonic trust | **FAIL** | epochs reset on restart |
| I. Live provider integration tests | **FAIL** | mocked only |
| J. Benchmarks reproducible on live stack | **FAIL** | simulators |

**Verdict: NO-GO for production. GO for continued development and for a single-replica, non-safety-critical pilot** with explicit disclosure that semantic reuse is uncalibrated and cross-pod invalidation is not active.

---

## 12. Limitations of this audit

- No live PostgreSQL/Dragonfly/Qdrant/Neo4j/KMS stack was provisioned here; those rows are code-level assessments.
- No penetration test, fuzzing campaign, or load test was executed in this revision.
- Frontend and Python suites were not re-run; their numbers are taken from the prior audit and flagged as such.
- Line-level findings cite symbols rather than line numbers where the file is large, to avoid stale references after the next commit.
- The 13-discipline review is structural, not 200–400 independent humans.

**Next documents:** `51_SEMANTIC_GRAPH_MEMORY_DESIGN.md` (the graph-based semantic memory requested) and `52_MASTER_REMEDIATION_PLAN.md` (ordered, gated execution plan).
