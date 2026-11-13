# 41 — Anti-Fake Audit

Forensic inventory of mocked, simulated, hardcoded, and misleading behavior. Verdict vocabulary from `01_README.md`.

---

## 1. Simulated runtime (default)

| Item | Location | Type |
|---|---|---|
| Origin LLM returns a formatted string | `origin/provider.rs:293-322`, selected `state.rs:298` | MOCKED |
| Embeddings are FNV-1a LSH | `embedding.rs:54-140`, selected `state.rs:114` | HARDCODED |
| DB/cache/vector default memory | `config.rs:44-50`, `state.rs:86-142` | SIMULATED |
| Graph cache disabled by default | `config.rs:50`, `state.rs:321-329` | PLACEHOLDER |

## 2. Frontend fake backend

| Item | Location | Type |
|---|---|---|
| 698-LOC simulator intercepts routes in dev | `lib/standalone-simulator.ts`, `proxy/route.ts:163-190` | MOCKED |
| Cache hit by `length % 2 === 0` | `standalone-simulator.ts:167-190` | HARDCODED |
| Proof verify always `is_valid: true` | `:212-222` | HARDCODED |
| Merkle root literal `7a9b1c3d...` | `:196-210` | HARDCODED |
| Credentials `sb_live_${Math.random()}` | `:492-520` | HARDCODED |
| Session `authenticated:true, is_admin:true` | `:~600` | HARDCODED |
| Dev auth accepts any credential | `api/auth/route.ts:152-176` | SIMULATED |
| Plaintext secret stored as session token | `api/auth/route.ts:132,168` | HARDCODED |
| `security/page.tsx` mints fake keys on failure | `:145-172` | HARDCODED |
| Overview "live telemetry" literals | `app/page.tsx:201-225,741-831,1646-1680` | HARDCODED |
| Explorer cache directory literals | `app/explorer/page.tsx:52-108` | HARDCODED |
| FinOps CSV/JSON export literals | `app/finops/page.tsx:110-200,239` | HARDCODED |
| TDEK registry fake tenants/backends | `app/governance/page.tsx:505-585` | HARDCODED |
| Fleet modeler "63 Passing Tests" | `components/simulator/stress-simulator.tsx:40-125` | HARDCODED |
| DAG proof initialized `true` | `app/execution/page.tsx:20-22` | HARDCODED |

## 3. Benchmark fabrication

| Item | Location | Type |
|---|---|---|
| `571,428 req/s` cites nonexistent `throughput_test.rs` | `README.md:469` | UNVERIFIED |
| `3,257 req/s` "socket" from `oneshot` | `performance_test.rs:101-113` vs README | CONTRADICTED |
| `2.15 MB` = `edges*1.8` | `graph_invalidation_deep_benchmark.py:298` | HARDCODED |
| `23.76 MB` = `edges*24` | `:295` | HARDCODED |
| `0.0% false hits` with reuse 0 | `adversarial_evaluation_suite.py:820-824` | VACUOUS |
| Hardcoded "75 scenarios" print | `heldout_semantic_generalization.py:399` | HARDCODED |
| Unconditional "100% STABLE (PASS)" | `capacity_soak.py:474-475` | HARDCODED |
| Neo4j latency/memory constants | `graph_storage_and_policy_bench.py:231-232,264-268` | HARDCODED |
| "Criterion microbenchmarks" | `docs/master/10:11` | FALSE |

## 4. Self-attesting proof

| Item | Location | Type |
|---|---|---|
| Evidence flags hardcoded true | `handlers.rs:658-666` | HARDCODED |
| Verifier checks those flags | `certificate.rs:454-462` | CIRCULAR |
| Client-supplied freshness | `handlers.rs:657,671` | UNVERIFIED |

## 5. Dead-but-advertised components

| Component | Location | Status |
|---|---|---|
| Durable Postgres outbox | `postgres.rs` outbox methods | DEAD CODE |
| Distributed cluster sync | `cluster_sync.rs` | UNWIRED |
| Neo4j store | `graph.rs:158-291` | UNWIRED |
| Write-behind flusher | `write_behind.rs` | UNWIRED |
| Rigorous conformal calibrator | `conformal.rs:205-327` | UNWIRED |
| Cross-encoder backend trait | `guardrails.rs:575-581` | NO IMPL |
| Micro-batch embedder | `microbatch.rs` | UNUSED |
| PKCS#11 provider | `crypto_shred.rs:319` | TEST DOUBLE |

## 6. Legitimate test doubles (not deceptive)

- `MockOriginProvider` and mock servers in `production_startup_test.rs` — clearly test-only.
- `MockNeo4jCypherStore` for Cypher syntax tests — clearly named.
- `stress-simulator.tsx` banner "No network traffic is sent" — honest.
- `ceremony/page.tsx` — explicitly demo, test prevents fabricated numbers.

These are acceptable; the deceptive items are those shipped on a default user/developer path without disclosure (§2).

## 7. Verdict

Deceptive-surface count: **~20** hardcoded/mocked artifacts on default paths. This is the core reason the project cannot be represented as production-ready. Remediation in `46` P3/P4.
