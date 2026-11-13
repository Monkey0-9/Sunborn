# 09 — Feature Verification Matrix

Statuses use the vocabulary in `01_README.md`. "Runtime" reflects the code path, not marketing.

| # | Feature | Implementation | Runtime | Evidence |
|---|---|---|---|---|
| 1 | OpenAI chat completions proxy | COMPLETE | VERIFIED WORKING | `handlers.rs:904` |
| 2 | Anthropic messages proxy | COMPLETE | VERIFIED WORKING | `handlers.rs:1069` |
| 3 | L1 exact cache | COMPLETE | WORKING WITH LIMITATIONS (GDSF, not TinyLFU) | `gdsf.rs`, `state.rs:66-70` |
| 4 | L2 Dragonfly/Redis cache | COMPLETE | WORKING (opt-in; fallback masks failure) | `dragonfly.rs` |
| 5 | Vector search (memory) | COMPLETE | WORKING (brute force) | `vector_store.rs:94-150` |
| 6 | Vector search (Qdrant) | COMPLETE | WORKING (opt-in; delete bug F-006) | `vector_store.rs:169-585` |
| 7 | Embeddings (HTTP) | COMPLETE | WORKING when configured | `embedding.rs:174-233` |
| 8 | Embeddings (default) | HARDCODED (LSH hash) | SIMULATED | `embedding.rs:54-140` |
| 9 | Structural guardrails | PARTIAL (token heuristics) | WORKING | `guardrails.rs` |
| 10 | Conformal risk control | PARTIAL | WORKING (seeded thresholds) | `conformal.rs:78-123` |
| 11 | Rigorous conformal calibrator | COMPLETE but UNWIRED | DEAD CODE | `conformal.rs:205-327` |
| 12 | Cross-encoder NLI | NOT IMPLEMENTED (named) | — | `guardrails.rs:595-637` |
| 13 | ReuseCertificate | PARTIAL (self-attested) | WORKING WITH LIMITATIONS | `certificate.rs`, `handlers.rs:651-674` |
| 14 | Epoch lease fencing | PARTIAL (admin-only bumps) | WORKING | `epoch.rs`, `handlers.rs:1235,1369` |
| 15 | Roaring lineage | COMPLETE | WORKING (lineage dropped after purge) | `bitmap_lineage.rs:46-65` |
| 16 | Atomic two-phase admission | COMPLETE | WORKING | `graph_cache.rs:89-146` |
| 17 | Transitive invalidation (in-memory) | PARTIAL (direct edges) | WORKING WITH LIMITATIONS | `graph.rs` |
| 18 | Transitive invalidation (Neo4j) | COMPLETE | UNWIRED | `graph.rs:158-291` |
| 19 | Durable Postgres outbox | COMPLETE methods | DEAD CODE | `postgres.rs` outbox methods |
| 20 | In-memory outbox + DLQ | COMPLETE | WORKING (not durable) | `outbox.rs` |
| 21 | Distributed cluster sync | COMPLETE | UNWIRED | `cluster_sync.rs` |
| 22 | Merkle DAG planner | COMPLETE | WORKING | `planner.rs`, `merkle.rs` |
| 23 | Inclusion proofs | COMPLETE | WORKING | `merkle.rs:191-260` |
| 24 | Execution memoization | COMPLETE | WORKING (in-memory) | `store.rs` |
| 25 | Write-behind flusher | COMPLETE | UNWIRED | `write_behind.rs` |
| 26 | AEAD envelope encryption | COMPLETE | VERIFIED WORKING | `crypto_shred.rs` |
| 27 | Real KMS/Vault | COMPLETE | WORKING (opt-in) | `crypto_shred.rs` |
| 28 | Crypto-shredding | COMPLETE | VERIFIED WORKING | `crypto_shred.rs` |
| 29 | Dual-admin quorum | PARTIAL | WORKING | `middleware/auth.rs:55-85` |
| 30 | FinOps reservations | COMPLETE | VERIFIED WORKING | `finops.rs`, `handlers.rs:249` |
| 31 | Token avoidance accounting | COMPLETE | WORKING | `finops.rs` |
| 32 | Graceful shutdown | COMPLETE | VERIFIED WORKING (15s, not 30s) | `main.rs` |
| 33 | Postgres persistence | COMPLETE | WORKING (opt-in; silent fallback) | `postgres.rs` |
| 34 | Metrics | PARTIAL (process-local) | WORKING WITH LIMITATIONS | `handlers.rs:27-33` |
| 35 | Health/readiness | COMPLETE | VERIFIED WORKING | `main.rs`, `lib.rs` |
| 36 | Python SDK | COMPLETE | VERIFIED WORKING (26 tests OK) | `sdk/python` |
| 37 | Agent memoization | COMPLETE | WORKING | `client.py:346-489` |
| 38 | Next.js control plane | PARTIAL | WORKING WITH LIMITATIONS | `frontend/src` |
| 39 | Real frontend data | PARTIAL | MOCKED by default | `standalone-simulator.ts` |
| 40 | Frontend auth | PARTIAL | BROKEN (dev bypass F-001) | `api/auth/route.ts` |
| 41 | WCAG AA/AAA | NOT IMPLEMENTED (verified) | FAILING | 38 |
| 42 | 11-gate CI | PARTIAL | NOT INTEGRATED | `scripts/verify.sh` vs `ci.yml` |
| 43 | OpenAPI contract | COMPLETE | WORKING (root pointer broken) | `docs/api/openapi.yaml` |
| 44 | Multi-region failover | NOT IMPLEMENTED | DESIGN ONLY | `deploy/k8s/multi-region` |
| 45 | Real benchmarks | PARTIAL | NOT REPRODUCIBLE | 29 |

## Summary counts

- Enumerated features: **45**.
- Verified working (no material limitation): **18** (40%).
- Working with limitations: **14** (31%).
- Simulated/mocked/unwired/dead: **8** (18%).
- Not implemented / design only: **4** (9%).
- Failing: **1** (2%).

These counts are the basis for the final-verdict percentages (see 48 §Methodology). A feature counts as "verified working" only if the runtime path executes it with a real backend and a test or reproducible artifact supports it.
