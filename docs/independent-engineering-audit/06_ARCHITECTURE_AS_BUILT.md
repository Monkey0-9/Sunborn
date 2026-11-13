# 06 — Architecture As Built

This describes the system the code actually constructs, not the intended architecture.

---

## 1. Runtime composition (`AppState::new`, `crates/sunborn-gateway/src/state.rs:64-149`)

| Concern | Selected by default | Alternative (opt-in) |
|---|---|---|
| Exact cache (L1) | `MemoryExactCache` with GDSF byte budget 256 MiB (`:66-70`) | `DragonflyCacheStore` if `backend=dragonfly` (`:117-125`) |
| Vector store | `MemoryVectorStore` (brute-force cosine) (`:80-94`) | `QdrantVectorStore` if `vector_backend=qdrant` (`:86-94`) |
| Embeddings | `DeterministicEmbeddingProvider` (FNV-1a LSH) (`:102-115`) | `HttpEmbeddingProvider` if `SUNBORN_EMBEDDING_URL` |
| Database | `MemoryDatabaseStore` (`:96,127-135`) | `PostgresDatabaseStore` if `database_backend=postgres` |
| LLM origin | `MockOriginProvider` (`:298`) | `HttpLlmOriginProvider` if `SUNBORN_LLM_ORIGIN_URL`/`OPENAI_API_BASE` |
| Dependency graph | `DependencyGraph::new()` in-memory (`:142`) | Neo4j exists but no env path constructs it |
| Execution store | `MemoryExecutionCacheStore` (`:149`) | none |
| Graph memory cache | Disabled (`config.rs:50`, `state.rs:321-329`) | enabled via flag |

**Consequence:** a default `cargo run` is a fully simulated stack.

## 2. Request lifecycle (verified paths)

```
Client (OpenAI/Anthropic wire)
  -> Axum router (lib.rs:35-175, 40 routes)
  -> auth middleware (middleware/auth.rs; /health,/ready,/metrics,/v1/auth/login bypass)
  -> canonicalize request (10-D identity)
  -> FinOps reserve (SpendVelocityController; handlers.rs:249)
  -> L1 exact lookup (selected cache)
       hit -> certificate + epoch validation -> return (release reservation)
  -> semantic cascade (if enabled):
       structural fingerprint -> vector search -> conformal threshold -> entailment scorer
  -> on miss: origin provider (Mock by default) -> persist -> issue admission certificate -> cache
  -> settle reservation with real/derived token cost
```

## 3. State and trust boundaries

- **Trust boundary 1:** client → gateway (API key / session, tenant/app headers).
- **Trust boundary 2:** gateway → external LLM/embeddings (bearer key, TLS via rustls).
- **Trust boundary 3:** gateway → Postgres/Redis/Qdrant/KMS.
- **Weak boundary:** the certificate's freshness/eveidence inputs originate in the request body (`handlers.rs:657,671`); the verifier trusts issuer-supplied evidence flags rather than independently measured values.

## 4. Data layer

`migrations/0001-0003` create: tenants, durable_cache_entries, outbox_events, audit_events, tenant_wrapped_keys, api_credentials, finops_ledger, prefix_affinity_records, decision_records, applications, knowledge_sources, document_versions, dependency_edges, invalidation_outbox. `PostgresDatabaseStore` implements CRUD with `sqlx` binds. **No WAL tailing / logical replication exists** despite the "WAL engine" label.

## 5. Concurrency model

- Tokio async throughout; `DashMap` for lineage; `RwLock<HashMap>` for auth/execution stores; `AtomicU64` process-local metrics.
- `Singleflight` for origin dedupe; `CircuitBreakerCache` wraps L1.
- Invalidation outbox is an in-process `mpsc` channel + `Mutex<Vec<DLQ>>` (lost on restart).
- `WriteBehindFlusher` is implemented but never instantiated; request path writes synchronously.

## 6. Intended vs actual (delta)

| Intended (README) | Actual |
|---|---|
| Qdrant HNSW by default | memory brute force by default; Qdrant REST optional |
| PostgreSQL WAL authority | plain OLTP; in-memory fallback unless strict |
| Durable transactional outbox | in-memory outbox; Postgres outbox methods are dead code |
| Distributed cluster sync | `tokio::broadcast` only; not wired to gateway |
| Neo4j transitive invalidation | in-memory graph returns direct edges only |
| Cross-encoder NLI | cosine→probability table; no backend impls |
| W-TinyLFU | GDSF + oldest eviction |
| AST guardrails | token/vocabulary heuristics |
| Conformal α≤0.001 online | seeded thresholds; rigorous calibrator unwired |

## 7. Architectural verdict

Layering and modularity are good (clean crate boundaries, trait-based adapters). The critical gap is **wiring**: several well-implemented durable/distributed components are not on the runtime path, and the default composition selects simulated components. Architecture score: **6.5/10** (strong shape, weak production wiring).
