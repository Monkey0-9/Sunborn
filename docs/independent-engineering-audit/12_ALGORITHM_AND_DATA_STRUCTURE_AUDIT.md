# 12 — Algorithm & Data-Structure Audit

---

## 1. Exact cache eviction

- **GDSF** (`gdsf.rs:40-186`): `BTreeSet<PriorityEntry>` keyed on frozen f64 bits, global inflation clock, frequency bump, byte-budget admission, TTL.
  - Time: O(log n) insert/evict; Space: O(n).
  - Correctness: genuine GDSF.
- **Legacy path** (`memory.rs:120-140`): evicts by oldest `created_at` — O(n) scan.
- **Claimed W-TinyLFU is absent.** Rename or implement.

## 2. Vector similarity

- `cosine_similarity` (`embedding.rs:6-24`): O(d), numerically stable (norm guard).
- `MemoryVectorStore::search` (`vector_store.rs:94-150`): O(n·d) scan + sort O(n log n). No ANN/HNSW.
- Qdrant delegated search (opt-in) uses server-side indexing.
- **Claimed HNSW client-side index is absent.**

## 3. Roaring lineage

- `DashMap<u64, RoaringTreemap>` (`bitmap_lineage.rs:15-83`).
- Union is `|=` over compressed 64-bit containers → O(containers).
- **Defect:** `invalidate_document` removes the map entry (`:46-54`), so repeated mutations lose lineage (F-017).
- Memory claim "1.8 bytes/edge" is from a Python constant (`graph_invalidation_deep_benchmark.py:298`), not measured on this structure.

## 4. Merkle tree / inclusion proofs

- `merkle.rs`: domain-separated tags (0x00 leaf, 0x01 interior), 64-bit LE length framing, sorted parents.
- Odd-tail duplication for tree construction (`:191-260`).
- Verify is O(log n). Tamper test exists (`:355-375`).
- Correct for the implemented contract; not byte-exact RFC 6962 audit path (leaves pre-hashed externally).

## 5. DAG planner

- Kahn topological sort with cycle, duplicate, dangling-dep detection (`planner.rs:194-253`): O(V+E).
- Effect nodes excluded from reuse (`:88-115`) — correct safety property.
- `splice_dag` cached-prefix pruning with flat latency estimate (`merkle.rs:174`) — not algorithmic, just a reported constant.

## 6. Conformal calibration

- Per-stratum threshold lookup O(1); recalibration O(k log k) over observations (sort/quantile).
- Clamp `[0.70,0.99]` — can suppress legitimate reuse under distribution shift.
- Rigorous weighted-risk scan is O(n) but unwired.

## 7. Rate limiting / velocity

- Sliding-window spend controller; reservation map keyed by tenant; settle/release O(1).
- Correct sanitization of float extremes.

## 8. Invalidation outbox

- `mpsc` channel with 10ms backpressure; retry loop with exponential backoff; DLQ `Vec`.
- **Durability:** process-local; lost on restart.
- Invariant "do not clear dependency edges unless all keys acked" is implemented.
- Postgres outbox methods (dead) would give durability but are uncalled.

## 9. Search/ranking

- No dedicated ranking model; semantic ranking is cosine order from vector store.
- Conformal threshold acts as acceptance, not ranking.

## 10. Complexity claims vs reality

| Claim | Reality |
|---|---|
| Sub-50µs transitive purge | Roaring union on direct map; no transitive closure in-memory |
| 1.8 bytes/edge | formula constant, not measured |
| HNSW search | brute force |
| W-TinyLFU | GDSF |
| O(1) crypto shred | correct (key deletion) |

## Verdict

Core data structures (Roaring, Merkle, GDSF, Kahn planner) are competently implemented. Several named algorithms in the public narrative are not the ones in the code. Algorithm score: **6/10** (good building blocks, misleading labels, one lineage bug).
