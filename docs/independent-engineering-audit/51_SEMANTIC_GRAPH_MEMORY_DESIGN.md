# 51 — Semantic Graph Memory Design

**Status:** design + staged implementation plan. **Not implemented.** Nothing in this document may be cited as shipped capability until the acceptance tests in §11 pass.

**Goal:** make Sunborn's graph cache link *genuinely related* memory — same question asked differently, same entity under an alias, same answer derived from a changed document — without ever letting a semantic link authorize reuse. Links exist to **find** and **invalidate**, never to **trust**.

---

## 1. Why the current graph cache is not semantic memory

`GraphMemoryCache` today stores two things:

1. `source_id → cache_key` edges in `DependencyGraph` (lineage).
2. `doc_hash → record_id` bitmaps in `CompressedLineageGraph` (Roaring).

That is **provenance**, not semantics. It cannot answer "which cached answers talk about the same entity/intent as this one?" and it has no entity, alias, contradiction, or supersession model. It also lives entirely in one process (D-01).

The design below adds a semantic layer **beside** the existing provenance layer, reusing the same key→key traversal (`record_key_dependency` + closure BFS) so invalidation remains one mechanism.

---

## 2. Non-goals and hard rules

- **Links never authorize reuse.** Reuse is authorized only by the verification cascade + a valid `ReuseCertificate` + authoritative epochs.
- **Similarity is a candidate generator, not a safety proof.** Cosine ≥ τ proposes; structural fingerprint + context digest + epoch decide.
- **Over-invalidation is allowed; under-invalidation is not.** When uncertain, purge more.
- **No cross-tenant edges, ever.** Tenant is part of every node/edge primary key and every query.
- **No new trust after rebuild.** A rebuilt graph may contain fewer edges and never more confidence than the durable record.
- **Every write is explicit and reported.** No `let _ =` on the semantic write path.

---

## 3. Model

### 3.1 Nodes

| Node | Identity | Purpose |
|---|---|---|
| `PromptNode` | `cache_key` (existing identity hash) | A cached request/answer pair |
| `IntentCluster` | `uuid_v5(tenant, app, model, context_digest, fingerprint_hash)` | Canonical "same question" group |
| `Entity` | `uuid_v5(tenant, canonical_name, type)` | Person/org/product/account/date/etc. |
| `SourceChunk` | `uuid_v5(source_id, content_hash, chunk_index)` | RAG/knowledge unit |
| `AnswerVersion` | `uuid_v5(cache_key, content_hash)` | Immutable answer body; supports supersession |
| `PolicyVersion` | existing learning policy id | Ties reuse decisions to policy |
| `TenantScope` | `tenant_id` | Mandatory scope node for every edge |

### 3.2 Edges

| Edge | From → To | Semantics | Confidence |
|---|---|---|---|
| `DERIVED_FROM` | AnswerVersion → SourceChunk | Provenance | 1.0 (explicit) |
| `MENTIONS` | PromptNode/AnswerVersion → Entity | Entity appears with a role | 0.5–1.0 (extractor) |
| `ALIAS_OF` | Entity → Entity | Canonicalization (`IBM` ↔ `International Business Machines`) | 1.0 if curated, else ≤0.9 |
| `SAME_INTENT` | PromptNode → IntentCluster | Membership, not pair explosion | calibrated similarity band |
| `SUPERSEDES` | AnswerVersion → AnswerVersion | Newer answer replaces older | 1.0 |
| `CONTRADICTS` | AnswerVersion → AnswerVersion | Structural inversion detected | 1.0 (hard block on reuse) |
| `SAME_NUMERIC_ROLE` | PromptNode → PromptNode | Identical role-bound numbers | 1.0 |
| `SCOPED_TO` | Any → TenantScope | Isolation | 1.0 |

**Degree caps:** `SAME_INTENT` membership is stored via one `IntentCluster` node (bounded fan-out per prompt: ≤ 4 cluster edges). `MENTIONS` per node ≤ 64. `ALIAS_OF` per entity ≤ 16. This avoids the O(n²) pair graph that naive "link similar things" designs create.

### 3.3 What "same semantics" means here (strict, ordered)

A candidate may join an `IntentCluster` only if **all** hold:

1. Same `tenant_id`, `application_id`, `model`, and (if present) `context_digest`.
2. Structural fingerprint match:
   - negation polarity + antonym tokens equal,
   - quantifier scopes equal,
   - sorted numeric literals equal,
   - role-bound numeric tokens equal (or both empty),
   - temporal tokens equal,
   - entity token sets equal after alias canonicalization.
3. Embedding cosine ≥ `τ(stratum)` from the **calibrated** threshold table, not a constant.
4. Not `CONTRADICTS` any member of the cluster.
5. Answer content hashes differ only if the differing answers are explicitly `SUPERSEDES`-ordered; otherwise the cluster is marked **ambiguous** and reuse abstains.

If any condition is unknown (no fingerprint, no context digest, uncalibrated stratum), the node joins **no cluster** and is eligible only for exact-cache reuse.

---

## 4. Write path (admission)

Triggered after a successful origin execution, inside the existing admission state machine.

```
1. fingerprint := StructuralFingerprint::extract(prompt)
2. entities    := extract_entities(prompt) -> canonicalize via ALIAS_OF
3. digest      := ContextDigest::compute(model, system_prompt, rag_chunk_ids)
4. cluster_key := H(tenant, app, model, digest, fingerprint.fingerprint_hash)
5. BEGIN TX
   a. upsert semantic_nodes(PromptNode, AnswerVersion, SourceChunk, Entity...)
   b. upsert IntentCluster(cluster_key); insert SAME_INTENT(prompt -> cluster)
   c. insert DERIVED_FROM(answer -> each source chunk), MENTIONS(...)
   d. insert semantic_outbox(tenant, epoch, op='admit', payload_hash)
   e. update tenant_epochs SET hwm = hwm + 1 (same tx)
   COMMIT
6. best-effort projections (cache/graph/vector) with explicit per-target status
7. if any projection fails -> mark admission `degraded` in DB, emit metric, do NOT report success
```

**Why cluster membership rather than pairwise links:** bounded degree, deterministic representative, and one traversal hop for "all paraphrases of this question". The cluster is a *routing index*, not a trust assertion.

**Determinism:** `cluster_key` uses canonical JSON + SHA-256 (RFC 8785 already in the repo), so two replicas compute the same cluster without coordination.

---

## 5. Read path

```
1. exact cache (unchanged, certificate-gated)
2. vector search top_k = 8 (not 1) within tenant/app
3. for each candidate:
     - require certificate + authoritative epochs
     - recompute measured safety evidence vs candidate prompt
     - require same cluster or explicit SAME_NUMERIC_ROLE
4. ambiguity rule:
     - if two candidates have similarity within δ=0.01 AND different content_hash
       AND both pass structural checks -> ABSTAIN (execute origin)
5. reuse only if exactly one candidate passes; otherwise miss
```

`top_k = 8` and the ambiguity rule implement "abstain when uncertain". Abstention is recorded as `DecisionAction::Revalidate` with `reason_code = AMBIGUOUS_SEMANTIC_CLUSTER`, so it is measurable.

---

## 6. Invalidation path (the core correctness work)

On a knowledge mutation for source `S` with doc hashes `D`:

```
1. epoch := tx: UPDATE tenant_epochs SET hwm = hwm + 1 RETURNING hwm
            INSERT semantic_outbox(op='invalidate', source=S, docs=D, epoch=epoch)
   COMMIT                                  -- authority first, durable
2. keys := dependency_graph.get_affected_cache_keys(S)        -- provenance closure
3. records := lineage.invalidate_documents(D)                  -- Roaring union
4. keys := keys ∪ record_to_keys(records)
5. seeds := keys
6. semantic := key_closure(seeds, max_hops=1)                  -- SAME_INTENT siblings
7. purge exact + vector + graph-cache for (keys ∪ semantic)
8. bump in-memory watermarks for S, tenant-scoped
9. write purge report {requested, purged, failed, retried, closure_hops, truncated}
10. if failed non-empty -> return ERROR (partial is never success)
```

Rules:
- **Bump epoch before deleting** (fixes D-10).
- **One owned orchestrator** (`invalidation::orchestrator`) deletes exact + vector + graph; callers cannot pick a subset (fixes D-07).
- **Closure depth is one config constant**, reported when truncated (fixes D-06).
- **Semantic closure is 1 hop** from a cluster: enough to catch paraphrases, bounded enough to avoid mass eviction.
- **Entity-driven invalidation:** if the changed chunk mentions entity `E`, also purge keys with `MENTIONS → E` **and** a fingerprint containing a changed numeric/date token. This is an optional second wave behind a flag, because it is the highest over-invalidation risk.
- **Tombstones** for purged `AnswerVersion`s; a rebuild must not resurrect a tombstoned content hash.

---

## 7. Multi-replica consistency

| Concern | Design |
|---|---|
| Epoch authority | `tenant_epochs(tenant_id, hwm)` in PostgreSQL; increments in the mutation tx; read path caches for ≤ 1s with monotonic guard |
| Event fan-out | transactional outbox → Redis Streams (or Dragonfly pub/sub with a durable stream) → per-pod consumer |
| Idempotency | consumer key `(tenant_id, epoch, op, payload_hash)` in `applied_invalidation`; duplicate delivery is a no-op |
| Ordering | per-tenant epoch monotonic; consumer rejects `epoch ≤ applied_hwm` |
| Pod-local acceleration | each pod projects into `GraphMemoryCache`; a missed event is repaired by a periodic reconciliation (compare `tenant_epochs.hwm` vs. applied) |
| Recovery | on boot: load hwm, replay outbox from `applied_hwm`, then rebuild projections; do not serve semantic reuse until replay completes |
| Proof | test with 2+ real gateway processes and a shared Redis/Postgres: mutate on A, read on B, assert miss within ≤ 1 reconciliation interval |

This replaces `InMemoryClusterTransport` as the production implementation; it stays as a test double only.

---

## 8. Storage

PostgreSQL is authoritative. Neo4j is an optional projection; the Roaring bitmap remains a rebuildable hot index.

```sql
CREATE TABLE semantic_nodes (
    node_id          UUID PRIMARY KEY,
    tenant_id        VARCHAR(128) NOT NULL,
    application_id   VARCHAR(128) NOT NULL,
    model            VARCHAR(128),
    kind             VARCHAR(32)  NOT NULL,   -- prompt|cluster|entity|chunk|answer|policy
    cache_key        VARCHAR(512),
    cluster_id       UUID,
    fingerprint_hash CHAR(64),
    context_digest   CHAR(64),
    canonical_name   TEXT,
    content_hash     CHAR(64),
    metadata         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    tombstoned_at    TIMESTAMPTZ
);
CREATE INDEX idx_semantic_nodes_scope ON semantic_nodes(tenant_id, application_id, kind);
CREATE UNIQUE INDEX uq_semantic_node_natural
  ON semantic_nodes(tenant_id, kind, COALESCE(cache_key,''), COALESCE(canonical_name,''), COALESCE(content_hash,''));

CREATE TABLE semantic_edges (
    tenant_id   VARCHAR(128) NOT NULL,
    src_id      UUID NOT NULL REFERENCES semantic_nodes(node_id) ON DELETE CASCADE,
    dst_id      UUID NOT NULL REFERENCES semantic_nodes(node_id) ON DELETE CASCADE,
    kind        VARCHAR(32) NOT NULL,
    confidence  REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    evidence    CHAR(64) NOT NULL,            -- hash of the derivation, not free text
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, src_id, dst_id, kind)
);
CREATE INDEX idx_semantic_edges_dst ON semantic_edges(tenant_id, dst_id, kind);

CREATE TABLE tenant_epochs (
    tenant_id VARCHAR(128) PRIMARY KEY,
    hwm       BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE semantic_outbox (
    tenant_id   VARCHAR(128) NOT NULL,
    epoch       BIGINT NOT NULL,
    op          VARCHAR(32) NOT NULL,
    payload     JSONB NOT NULL,
    payload_hash CHAR(64) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, epoch)
);

CREATE TABLE applied_invalidation (
    tenant_id VARCHAR(128) NOT NULL,
    epoch     BIGINT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, epoch)
);
```

**Migration authority:** these tables go into **one** ordered migration set. Today `migrations/*.sql` and `PostgresDatabaseStore::migrate()` both create schema (D-01 in doc 52). Consolidate before adding tables; do not add a third path.

---

## 9. Rust API (reference shape)

Add to `KnowledgeGraphStore` (all three impls):

```rust
pub trait KnowledgeGraphStore {
    // existing...
    async fn get_key_closure(&self, seeds: &[String], max_hops: usize) -> Result<Vec<String>>;
    async fn record_semantic_link(
        &self, child_key: &str, parent_key: &str, confidence: f32,
    ) -> Result<()>;
    async fn semantic_neighbors(&self, key: &str) -> Result<Vec<String>>;
}
```

Add to `GraphMemoryCache`:

```rust
pub struct PurgeReport {
    pub requested: usize,
    pub purged: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub closure_hops: usize,
    pub truncated: bool,
}

impl GraphMemoryCache {
    /// Symmetric, degree-capped, same-tenant only. Idempotent.
    pub async fn link_same_intent(
        &self, key: &str, peers: &[String], max_degree: usize,
    ) -> Result<usize>;

    /// Exact purge closure used by the invalidation orchestrator.
    pub async fn purge_with_semantic_closure(
        &self, seeds: &[String],
    ) -> Result<PurgeReport>;
}
```

Notes:
- `get_key_closure` is the missing primitive: `InMemoryGraphStore` traverses `key_hierarchy`; `Neo4jCypherGraphStore` uses a parameterized `MATCH (c:CacheKey)-[:SAME_INTENT*1..N]->(p)`; `MockNeo4jCypherStore` advances its in-memory hierarchy. Depth is passed in, not hardcoded.
- `link_same_intent` records both directions (child→parent and parent→child) so invalidating either side reaches the cluster; cap is enforced by refusing new edges beyond `max_degree` and logging a `degree_capped` counter.
- `purge_with_semantic_closure` is the **only** production purge entry point; the old methods delegate to it.

---

## 10. Invariants and tests

| ID | Invariant | Test |
|---|---|---|
| I1 | Every edge is tenant-scoped; no query can cross tenants | property test with random tenants; assert empty cross-tenant closure |
| I2 | Reuse requires a valid certificate + current epoch, regardless of cluster membership | forge cluster edge; assert still miss |
| I3 | Source mutation purges all provenance descendants and 1-hop semantic siblings | randomized DAG + clusters; assert purge set ⊇ expected closure |
| I4 | Partial purge returns error and a report; never `Ok` | injected delete failure; assert `Err` + `failed` non-empty |
| I5 | Closure depth is constant across backends; truncation is reported | same graph on memory + mock Neo4j; compare sets and `truncated` |
| I6 | Rebuild never raises confidence or epoch | rebuild after epoch restore; assert `hwm_after ≥ hwm_before`, edges ⊆ durable, no tombstone resurrection |
| I7 | Duplicate invalidation events are idempotent | deliver same event 3×; assert one effect |
| I8 | Degree caps hold under concurrent admission | N concurrent writers; assert degree ≤ cap, no deadlock |

Add property tests to `crates/sunborn-gateway/tests/property_invariants_test.rs` and new `graph_semantic_test.rs`.

---

## 11. Evaluation protocol (measure before claiming)

**Datasets**
- Held-out paraphrase set (≥ 2,000 pairs) per stratum, never used for calibration.
- Adversarial inversion set (≥ 500 pairs): negation, numeric transposition, entity swap, temporal shift, quantifier scope.
- Real traffic shadow sample (anonymized) for drift.

**Metrics**
- False-reuse rate (unsafe hit / reuse attempts), with Wilson 95% upper bound ≤ α for the claimed stratum.
- Abstention rate, hit rate, candidate recall@8.
- Entity-resolution precision/recall for `ALIAS_OF`/`MENTIONS`.
- p50/p99 added latency; invalidation propagation p99 across 2 pods.
- Purge completeness (expected keys vs. purged) = 100%.

**Acceptance**
- Adversarial false reuse = 0 on the 500-pair suite (hard gate).
- Held-out false reuse upper bound ≤ 0.001 for SecuritySensitive, ≤ 0.01 for Financial/Healthcare (matching the seeded α story), **with a published artifact** containing dataset hash, code commit, and run manifest.
- 2-pod invalidation propagation ≤ 250 ms p99; zero stale reads in a 10-minute soak.
- No `Ok` on partial purge in fault injection.

If any gate fails, the claim is lowered to the measured number. Do not ship the alpha number as marketing.

---

## 12. Rollout plan

| Phase | Scope | Exit criteria |
|---|---|---|
| G0 | Schema + single migration authority + epoch persistence | migrations reconcile; `cargo check`; rebuild restores hwm |
| G1 | Single-node semantic nodes/edges, cluster admission, ambiguity abstention | I1–I4, I8 pass; 2,000-pair offline eval |
| G2 | Owned invalidation orchestrator + purge reports + closure depth constant | I3, I4, I5 pass; fault injection shows no silent partials |
| G3 | Durable outbox + Redis Streams consumer + 2-pod reconciliation | I7; 2-pod soak ≤ 250 ms; no stale reads |
| G4 | Evaluation artifact + docs + feature flag default-on | §11 acceptance; README claims only measured numbers |

**Rollback:** `SUNBORN_SEMANTIC_GRAPH_ENABLED=false` must degrade to today's provenance-only graph, with no schema rollback required (projection is additive). Purge orchestrator stays on regardless of the flag.

---

## 13. Risks

| Risk | Mitigation |
|---|---|
| Over-invalidation kills hit rate | 1-hop cap, cluster granularity, purge metrics, canary tenants |
| Entity extraction errors create wrong links | links never authorize; confidence floor; alias curation; eval of link quality separately |
| Cluster drift over time | re-cluster on fingerprint change; decay `last_seen_at`; max age |
| Distributed consumer lag | reconciliation loop against `tenant_epochs.hwm`; serve exact-only until caught up |
| Cost of graph writes | batch within admission tx; no per-read writes; cluster node bounds fan-out |
| Another "designed but unwired" feature | G0–G4 each require tests that fail before the change and pass after; no doc claim before artifact |

**Bottom line:** this design makes the graph cache useful for *finding and invalidating* semantically related memory while keeping trust anchored in certificates and epochs. It is deliberately conservative: it would rather miss a reuse than serve a stale one.
