# 10 — Function-Level Audit

This samples every load-bearing function family. Full per-function columns are in `inventories/functions.csv`. Format: name · file:line · purpose · complexity · correctness · notes.

---

## 1. `sunborn-core`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `ReuseCertificate::new_signed` | `certificate.rs` (Merkle over 5 leaves) | Issue signed reuse proof | Real HMAC-SHA-256 + length-prefixed Merkle. **Input evidence is caller-supplied** (see F-004). |
| `ReuseCertificate::validate_for_reuse` | `certificate.rs:454-462` | Verify 10 gates | Enforces structural/negation/numeric/entity flags — which the issuer hardcodes true/false. |
| `FinOpsEngine` | `finops.rs:204-360` | Budget reservation/settle | Correct NaN/inf/negative sanitization; reservation released on all paths (verified in handlers). |
| `SpendVelocityController` | `finops.rs` | Sliding-window limiting | Real. |
| `AppConfig::load_from_env` | `config.rs:210` | Load config | Only production `panic!`; `try_load_from_env` returns Result. |

## 2. `sunborn-identity`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `encrypt`/`decrypt` | `crypto_shred.rs:1102-1189` | AES-256-GCM envelope | Correct 96-bit nonce, AAD `tenant:context`, tag check. |
| `ZeroizingKey` wrap/unwrap | `crypto_shred.rs:116-171` | KEK wrapping | Zeroize + Drop. |
| `VaultTransitKeyProvider` | `crypto_shred.rs:254-...` | Vault adapter | **Documented as "local envelope test double"** in one type; real HTTP path exists separately. |
| `AwsKmsKeyProvider` | `crypto_shred.rs:395-...` | KMS adapter | Real `KmsClient::encrypt/decrypt` when selected. |
| `normalize_unicode_nfc` | `canonical.rs:8-66` | NFC normalize | **Hardcoded 40-pair composition table**, not NFC. |
| `canonicalize_json` | `canonical.rs:96` | Canonical JSON | Key-sorting only; RFC 8785 variant is separate (`:119`). |
| `run_blocking_async` | `crypto_shred.rs:530-575` | Sync→async bridge | Spins/`block_in_place`; risk under load. |

## 3. `sunborn-cache`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `MemoryExactCache::insert/get` | `memory.rs` | L1 store | GDSF byte-budget; oldest-`created_at` fallback path. |
| GDSF eviction | `gdsf.rs:40-186` | Utility eviction | Genuine GDSF; **not W-TinyLFU**. |
| `DragonflyCacheStore::delete_by_prefix` | `dragonfly.rs:214-244` | Prefix delete | Uses `KEYS` (O(N) blocking) — F-016. |
| `count_by_prefix` | `dragonfly.rs:246-...` | Prefix count | Same `KEYS` issue. |
| `CircuitBreakerCache` | `circuit_breaker.rs` | Failover wrapper | Real. |

## 4. `sunborn-semantic`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `cosine_similarity` | `embedding.rs:6-24` | Cosine | Correct. |
| `LshRandomProjectionEmbeddingProvider::embed` | `embedding.rs:54-140` | Default embed | FNV-1a over unigrams+3-grams → 384d. Not semantic. |
| `HttpEmbeddingProvider::embed` | `embedding.rs:174-252` | Real embeddings | Correct call; **silent LSH fallback** outside production (`:242-251`). |
| `MemoryVectorStore::search` | `vector_store.rs:94-150` | Search | Full scan + sort; O(n·d). |
| `QdrantVectorStore::insert` | `vector_store.rs:281-353` | Upsert | Hashes id to UUID. |
| `QdrantVectorStore::delete` | `vector_store.rs:469-501` | Delete | Sends raw id → **mismatch, F-006**; returns `Ok(true)` on failure when non-strict. |
| `StructuralFingerprint::extract` | `guardrails.rs:33-495` | "AST" guardrails | Token heuristics + vocabularies; no parser. |
| `StructuralEntailmentScorer::score` | `guardrails.rs:599-637` | "Cross-encoder NLI" | Cosine→probability lookup; no model. |
| `TaskStratifiedConformalCalibrator::get_threshold` | `conformal.rs:103-123` | Risk threshold | Seeded constants; recalibrates after ≥20 obs. |
| `NonExchangeableConformalCalibrator` | `conformal.rs:205-327` | Rigorous calibrator | **Unwired; doc says shadow research only.** |
| `VerificationCascade::verify` | `decision.rs:73-193` | Decision path | Real cascade; risk pre-gate is keyword list. |

## 5. `sunborn-invalidation`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `CompressedLineageGraph::invalidate_document` | `bitmap_lineage.rs:46-54` | Purge | Removes doc→records map → lineage lost (F-017). |
| `GraphMemoryCache::set_with_lineage` | `graph_cache.rs:89-146` | 3-phase admission | Genuine compensating rollback. |
| `EpochLeaseManager::validate_multi_source_leases` | `epoch.rs:76-150` | Lease fencing | Real; bumps only via admin endpoints. |
| `WatermarkLineageFencer::validate_and_commit` | `watermark.rs:32-98` | Watermark | Real. |
| `InvalidationEngine::process_event` | `outbox.rs:134-227` | Retry + DLQ | In-memory; invariant "never clear deps unless acked" honored. |
| `DistributedGraphSyncEngine` | `cluster_sync.rs:108-238` | Cluster sync | `tokio::broadcast`; **not wired**. |
| `InMemoryGraphStore::get_affected_cache_keys` | `graph.rs` | Invalidation lookup | Direct edges only; no transitive walk. |

## 6. `sunborn-execution`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `splice_dag` | `merkle.rs:131-189` | Reuse cached prefix | Latency saving is a flat `+=150` constant (`:174`) — cosmetic metric. |
| `build_merkle_tree` / `verify` | `merkle.rs:191-260`, `:59-82` | Inclusion proof | Correct domain separation; self-duplication for odd tails. |
| `ExecutionPlanner::plan_reuse` | `planner.rs:194-253` | Plan reuse | Kahn topo sort, cycle/dangling detection, effect gating. |

## 7. `sunborn-db`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `PostgresDatabaseStore::new` | `postgres.rs:176-360` | Pool + DDL | Real sqlx, 15 tables. |
| CRUD methods | `postgres.rs:386-1185` | Persistence | Parameterized binds; **falls back to memory on error unless strict**. |
| `insert_outbox_event`/`fetch_pending_outbox_events`/`mark_outbox_processed` | `postgres.rs:775-858` | Durable outbox | **No callers anywhere** (dead). |
| `WriteBehindFlusher` | `write_behind.rs:18-230` | Group commit | Unwired; `Drop` aborts worker without drain. |

## 8. `sunborn-gateway`

| Function | File:line | Purpose | Notes |
|---|---|---|---|
| `create_router` | `lib.rs:35-175` | 40 routes | Real. |
| `chat_completions` / `anthropic_messages` | `handlers.rs:904,1069` | Protocol translation | Real; both funnel to `execute_request_inner`. |
| `execute_request_inner` | `handlers.rs:980` | Core pipeline | Real; contains F-004/F-005. |
| `authenticate` / `require_admin` / quorum | `middleware/auth.rs:38-450` | Auth | Real hashing, RBAC, dual-admin. |
| `graceful_drain` | `state.rs:411-457` | Shutdown drain | Drains in-memory outbox; warns DLQ. |
| `MockOriginProvider::complete` | `origin/provider.rs:293-322` | Default origin | Returns a formatted string — **simulated**. |

## 9. Cross-cutting defects

- Silent fallbacks (F-010) appear in 4 crates.
- Metrics are per-process atomics, presented as Prometheus series.
- No function-level formal contracts on the safety path (see 14).
