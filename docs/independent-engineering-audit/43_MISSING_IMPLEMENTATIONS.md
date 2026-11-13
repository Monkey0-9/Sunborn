# 43 — Missing Implementations

Capabilities claimed or implied that do **not** exist (vs. exist-but-unwired, which are noted).

## 1. Not implemented at all

| # | Missing | Evidence |
|---|---|---|
| M-01 | Client-side HNSW/ANN index | no ANN crate; brute-force `vector_store.rs:94-150` |
| M-02 | W-TinyLFU eviction | GDSF only (`gdsf.rs`) |
| M-03 | AST/grammar guardrails | token heuristics (`guardrails.rs`) |
| M-04 | Cross-encoder NLI model | cosine table; zero backend impls |
| M-05 | PostgreSQL WAL tailing/replication | plain OLTP (`postgres.rs`) |
| M-06 | Independent certificate verifier (asymmetric) | symmetric HMAC only |
| M-07 | Multi-region failover | design sketches |
| M-08 | General rate limiting / request size caps | absent |
| M-09 | Idempotency keys on mutations | absent |
| M-10 | Content-Security-Policy | absent (`next.config.mjs`) |
| M-11 | OIDC/MFA for operator console | API keys only |
| M-12 | Backup/restore automation | runbooks manual |
| M-13 | OpenTelemetry exporter | spec only |
| M-14 | Container image scanning | none |
| M-15 | cosign signing / SLSA attestation | none |
| M-16 | Real embedding model default | LSH hashing |
| M-17 | Reranker model | none |
| M-18 | Embedding/model version in cache key | absent |

## 2. Implemented but not wired (missing integration)

| # | Component | Evidence |
|---|---|---|
| M-20 | Durable Postgres outbox | 3 methods, no callers |
| M-21 | `WriteBehindFlusher` | never instantiated |
| M-22 | `DistributedGraphSyncEngine` | no gateway reference; broadcast only |
| M-23 | Neo4j store | no env path |
| M-24 | `NonExchangeableConformalCalibrator` | doc says shadow only |
| M-25 | `MicroBatchEmbedder` | unused |
| M-26 | `Pkcs11HsmKeyProvider` | test double |
| M-27 | `ConformalRiskController` | used only in its own test |
| M-28 | `CrossEncoderBackend` | no impl |
| M-29 | Ingest-driven epoch bump | only admin endpoints |

## 3. Documented but nonexistent

| # | Item | Evidence |
|---|---|---|
| M-30 | `POST /v1/admin/tenants/{id}/crypto-shred` | actual `/v1/governance/shred` |
| M-31 | `crates/sunborn-gateway/src/router.rs` | routes in `lib.rs` |
| M-32 | `sunborn-governance` crate | not in workspace |
| M-33 | `scripts/run_fullstack.*` | do not exist |
| M-34 | `throughput_test.rs` | does not exist |
| M-35 | `x-sunborn-cache` / `x-sunborn-latency-ms` headers | not set |

## Priority

P0-missing for production: M-01/16 (real retrieval), M-06 (independent verification), M-20/21/22 (durability/distribution), M-08/09/10 (hardening), M-15 (supply chain).
