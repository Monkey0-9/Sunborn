# Sunborn

**A semantic execution gateway and multi-tier cache for LLM applications, written in Rust.**

> ## Current status: functional prototype — production **NO-GO**
> Sunborn's gateway is real and tested. Its semantic-safety and distributed
> guarantees are **not yet established**: several components are opt-in or
> unwired, and the default runtime is simulated. Every claim in this README is
> evidence-linked to the independent audit in
> [`docs/independent-engineering-audit/`](docs/independent-engineering-audit/01_README.md).
> Do not deploy this to production until the P0/P1 items in
> [`46_REMEDIATION_ROADMAP.md`](docs/independent-engineering-audit/46_REMEDIATION_ROADMAP.md)
> are closed.

---

## What Sunborn is

A transparent reverse proxy that sits between client applications and LLM
providers (OpenAI, Anthropic, self-hosted vLLM). It aims to reduce latency and
token cost by reusing prior executions, while refusing to serve semantically
unsafe reuse.

The design thesis is sound and worth building: a cached answer should only be
reused when the system can *prove* the prompt is equivalent and the underlying
knowledge has not changed. Sunborn implements the machinery for that thesis —
certificates, epoch fencing, lineage graphs — but the current implementation
does not yet measure all of what it attests.

## What is actually implemented and verified

| Capability | State | Evidence |
|---|---|---|
| OpenAI-compatible `/v1/chat/completions` proxy | Verified working | `crates/sunborn-gateway/src/handlers.rs:904` |
| Anthropic-compatible `/v1/messages` | Verified working | `handlers.rs:1069` |
| Tier-1 exact cache (GDSF, byte-budgeted) | Working | `crates/sunborn-cache/src/gdsf.rs` |
| Redis / DragonflyDB distributed cache | Working (opt-in) | `crates/sunborn-cache/src/dragonfly.rs` |
| PostgreSQL persistence (sqlx) | Working (opt-in) | `crates/sunborn-db/src/postgres.rs` |
| AES-256-GCM per-tenant envelope encryption | Verified working | `crates/sunborn-identity/src/crypto_shred.rs` |
| O(1) cryptographic shredding | Verified working | `crypto_shred.rs` |
| AWS KMS / Vault Transit adapters | Working (opt-in) | `crypto_shred.rs` |
| Atomic FinOps budget reservations | Verified working | `crates/sunborn-core/src/finops.rs` |
| Merkle DAG planner + inclusion proofs | Working | `crates/sunborn-execution/src/merkle.rs` |
| Roaring-bitmap lineage + 2-phase admission | Working | `crates/sunborn-invalidation/src/` |
| Graceful shutdown with bounded drain | Verified working | `crates/sunborn-gateway/src/main.rs` |
| Python SDK (sync + async) | Verified working | `sdk/python` |

**Test suite (executed 2026-11-13): 315 passing, 0 failing** —
252 Rust (`cargo test --workspace --all-targets`), 37 frontend (`npm test`),
26 Python (`unittest`).

## What is not yet real (read this before trusting the cache)

- **Default origin is a mock.** Without `SUNBORN_LLM_ORIGIN_URL`, the gateway
  returns a formatted placeholder string (`MockOriginProvider`).
- **Default embeddings are not semantic.** Without `SUNBORN_EMBEDDING_URL`,
  embeddings are FNV-1a hashes (`LshRandomProjectionEmbeddingProvider`).
- **Default storage is in-memory.** `backend`, `vector_backend`, and
  `database_backend` default to `memory`; the graph cache is disabled.
- **Certificate evidence is currently asserted, not measured.** The admission
  path hardcodes structural/negation/numeric/entity checks; the verifier checks
  those same flags. This is a P0 and is being fixed.
- **Freshness inputs are client-supplied.** Dependency snapshots and epochs come
  from the request body; the epoch registry is advanced only by admin endpoints.
- **Several durable/distributed components are implemented but not wired:**
  the PostgreSQL outbox, `DistributedGraphSyncEngine`, the Neo4j store, and the
  write-behind flusher.
- **Named techniques that are not the named technique:** there is no client-side
  HNSW index (brute-force cosine, or Qdrant when enabled), no W-TinyLFU (GDSF),
  no AST parser (token guardrails), no cross-encoder NLI model (a calibrated
  structural scorer), and no PostgreSQL WAL tailing (ordinary OLTP).

Full detail: [`41_ANTI_FAKE_AUDIT.md`](docs/independent-engineering-audit/41_ANTI_FAKE_AUDIT.md)
and [`43_MISSING_IMPLEMENTATIONS.md`](docs/independent-engineering-audit/43_MISSING_IMPLEMENTATIONS.md).

## Architecture (as built)

```
Client (OpenAI / Anthropic wire format)
  → Axum gateway (40 routes)
  → auth/RBAC → canonicalize → FinOps reserve
  → L1 exact cache (GDSF / Dragonfly opt-in)
  → semantic cascade (structural guardrails → vector search → conformal threshold → entailment)
  → origin provider (HTTP, or Mock by default)
  → persist (Postgres opt-in) → issue certificate → cache
```

See [`06_ARCHITECTURE_AS_BUILT.md`](docs/independent-engineering-audit/06_ARCHITECTURE_AS_BUILT.md)
and [`07_ARCHITECTURE_DIAGRAMS.md`](docs/independent-engineering-audit/07_ARCHITECTURE_DIAGRAMS.md).

## Quickstart

### Docker Compose (full local stack)

```bash
docker compose -f deploy/docker-compose.yml up -d
curl http://localhost:8080/ready
# → {"status":"ready","database":"postgres","cache":"dragonfly","vector":"qdrant"}
```

### Standalone gateway

```bash
cargo build --release -p sunborn-gateway
SUNBORN_ENV=development ./target/release/sunborn-gateway
```

See [`docs/QUICKSTART.md`](docs/QUICKSTART.md) for environment variables and the
frontend console.

### First request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: <your-api-key>" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}]}'
```

Note: without `SUNBORN_LLM_ORIGIN_URL` the response comes from the mock origin.

## Configuration that makes it real

Set these to leave simulation behind:

| Variable | Effect |
|---|---|
| `SUNBORN_LLM_ORIGIN_URL` | Real upstream LLM (OpenAI-compatible) |
| `SUNBORN_EMBEDDING_URL` | Real embeddings endpoint |
| `SUNBORN_VECTOR_BACKEND=qdrant` + `SUNBORN_VECTOR_URL` | Qdrant vector store |
| `SUNBORN_CACHE_BACKEND=dragonfly` | Dragonfly/Redis exact cache |
| `SUNBORN_DB_BACKEND=postgres` + `POSTGRES_URL` | PostgreSQL persistence |
| `SUNBORN_KMS_PROVIDER=aws-kms` or `vault` | Remote key management |
| `SUNBORN_STRICT_DURABILITY=true` | Fail instead of silent in-memory fallback |

## Performance

**No benchmark number is endorsed at this time.** Earlier figures (e.g.
571,428 req/s, 3,257 req/s, 2.15 MB) were not reproducible from checked-in
artifacts and have been removed. The only checked-in capacity artifact reports
**2,500.9 req/s at P99 10.339 ms**, which is below the project's own target.
Reproducible benchmarking is tracked as P3 in the remediation roadmap.

See [`29_PERFORMANCE_AND_BENCHMARK_AUDIT.md`](docs/independent-engineering-audit/29_PERFORMANCE_AND_BENCHMARK_AUDIT.md).

## Workspace layout

```
crates/
  sunborn-core/          domain models, FinOps, certificates
  sunborn-identity/      canonicalization, AEAD envelope encryption, KMS
  sunborn-cache/         L1 GDSF cache, Dragonfly/Redis adapter
  sunborn-semantic/      guardrails, vector store, conformal risk, embeddings
  sunborn-execution/     Merkle DAG planner, inclusion proofs
  sunborn-invalidation/  Roaring lineage, epoch fencing, outbox
  sunborn-db/            PostgreSQL persistence, write-behind
  sunborn-gateway/       Axum reverse proxy, auth, routing
apps/sunborn-rebuild/    disaster-recovery cache rebuild engine
frontend/                Next.js 15 control plane
sdk/python/              Python SDK (sync + async)
deploy/                  Docker Compose, Helm, Kubernetes
docs/                    QUICKSTART, ADRs, runbooks, OpenAPI, independent audit
```

## API contract

OpenAPI 3.1 spec: [`docs/api/openapi.yaml`](docs/api/openapi.yaml) (48 paths / 49 operations).
Drift check: `python scripts/check_openapi_drift.py`.

## Documentation

- [Quickstart](docs/QUICKSTART.md)
- [OpenAPI spec](docs/api/openapi.yaml)
- [Architecture decision record](docs/adr/ADR-001-architecture-baseline.md)
- [Runbooks](docs/runbooks/operations.md)
- [Independent engineering audit](docs/independent-engineering-audit/01_README.md)
- [Final verdict](docs/independent-engineering-audit/48_FINAL_ENGINEERING_VERDICT.md)
- [Remediation roadmap](docs/independent-engineering-audit/46_REMEDIATION_ROADMAP.md)

## Contributing

Every quantified claim must cite a reproducible artifact. Benchmarks must print
measured values or fail. See the audit's engineering-standards assessment.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
