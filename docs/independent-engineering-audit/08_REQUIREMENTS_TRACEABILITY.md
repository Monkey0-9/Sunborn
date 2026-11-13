# 08 — Requirements Traceability

Reconstructed requirements (from README + surviving docs) mapped to implementation evidence and verdict. This replaces the four competing traceability matrices found in `docs/audit`, `docs/architecture`, and `docs/master`.

| Req ID | Requirement (as claimed) | Implementation evidence | Verdict |
|---|---|---|---|
| R-01 | OpenAI-compatible `/v1/chat/completions` reverse proxy | `handlers.rs:904`; `lib.rs` route | VERIFIED |
| R-02 | Anthropic-compatible `/v1/messages` | `handlers.rs:1069` | VERIFIED |
| R-03 | Tier-1 exact cache, sub-ms | GDSF memory cache; `handlers` path | WORKING (not <1ms measured) |
| R-04 | Distributed exact cache (DragonflyDB) | `dragonfly.rs` real client | IMPLEMENTED, opt-in |
| R-05 | Tier-2 semantic cache | `decision.rs`, `vector_store.rs`, `conformal.rs` | PARTIAL (LSH default, no ANN) |
| R-06 | Qdrant HNSW vector search | Qdrant REST adapter | IMPLEMENTED, opt-in; no client HNSW |
| R-07 | Structural AST guardrails | `guardrails.rs` token heuristics | PARTIAL (not AST) |
| R-08 | Conformal risk control α≤0.001 | seeded thresholds; rigorous calibrator unwired | PARTIAL / UNVERIFIED |
| R-09 | Cross-encoder NLI | `StructuralEntailmentScorer`; no backend impls | NOT IMPLEMENTED (named) |
| R-10 | Tier-3 Merkle DAG execution memoization | `merkle.rs`, `planner.rs` | IMPLEMENTED, in-memory |
| R-11 | RFC 6962 inclusion proofs | `merkle.rs:191-260` | IMPLEMENTED (style) |
| R-12 | ReuseCertificate verification | `certificate.rs` | PARTIAL (self-attested evidence) |
| R-13 | Per-tenant AEAD envelope encryption | `crypto_shred.rs:1102-1189` | VERIFIED |
| R-14 | AWS KMS / Vault Transit | real adapters, selectable | IMPLEMENTED, opt-in |
| R-15 | Crypto-shredding O(1) | key deletion + zeroize | VERIFIED |
| R-16 | Dual-admin quorum | second API key header | PARTIAL (not cryptographic) |
| R-17 | Atomic FinOps reservations | `finops.rs`, wired `handlers.rs` | VERIFIED |
| R-18 | Transactional outbox + DLQ | in-memory only; Postgres methods dead | PARTIAL / IN-MEMORY |
| R-19 | Bounded graceful shutdown | `main.rs:109-138` (15s default) | VERIFIED (claim says 30s — mismatch) |
| R-20 | Roaring bitmap lineage + transitive purge | `bitmap_lineage.rs`, `graph_cache.rs` | PARTIAL (in-memory direct edges; lineage dropped) |
| R-21 | Distributed cluster sync | `cluster_sync.rs` broadcast; not wired | NOT INTEGRATED |
| R-22 | Neo4j graph store | client real; never constructed | UNWIRED |
| R-23 | PostgreSQL persistence | real sqlx + DDL | VERIFIED (non-default, silent fallback) |
| R-24 | Prometheus metrics | process-local atomics | PARTIAL |
| R-25 | Liveness/readiness probes | `/health`, `/ready` | VERIFIED |
| R-26 | Python SDK sync+async | `sdk/python` real | VERIFIED (README example broken) |
| R-27 | Agent tool memoization | `sunborn_tool` real | VERIFIED |
| R-28 | Next.js control plane | 10 routes | PARTIAL (much fabricated UI) |
| R-29 | WCAG AAA/AA accessibility | source-grep tests | NOT VERIFIED (violations found) |
| R-30 | 11-gate CI | `verify.sh` exists; CI does not call it | PARTIAL |
| R-31 | 49 endpoints 0 drift | 48 paths/49 ops | VERIFIED count; root pointer broken |
| R-32 | Multi-region failover | design sketches only | NOT IMPLEMENTED |
| R-33 | 571k/3.2k req/s benchmarks | no supporting artifact | UNVERIFIED/FALSE |
| R-34 | 0.0% false hits | vacuous (reuse=0) | CONTRADICTED |

## Traceability note

The repo's existing `docs/audit/REQUIREMENT_TRACEABILITY.md` cites `crates/sunborn-gateway/src/router.rs`, which **does not exist** (routes are in `lib.rs`). `docs/verification/CLAIM_REGISTER.md` and `docs/engineering/CLAIM_AND_EVIDENCE_REGISTER.md` assign the same `CLM-0x` IDs to different claims. Both are superseded by this table.
