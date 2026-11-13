# 29 — Performance & Benchmark Audit

---

## 1. Public claims and their provenance

| Claim | Source | Produced by a script? | Verdict |
|---|---|---|---|
| 571,428 req/s, P50 1.0 µs, P99 9.0 µs (L1) | README table; cites `throughput_test.rs` | **File does not exist** | UNVERIFIED / FALSE citation |
| 3,257 req/s, P50 294 µs, P99 475 µs (gateway "socket stack") | README; prose docs | No; test uses `oneshot` (no socket) | CONTRADICTED |
| 142,857 ops/s, P50 4.0 µs, P99 3.8 ms (graph) | README | P50/P99 match `graph_deep_audit_results.json`; 142,857 is 1/7µs, not derivable from 4µs | PARTIALLY |
| 2.15 MB for 1M edges, 1.8 B/edge | README | `graph_invalidation_deep_benchmark.py:298` computes `edges*1.8` | FALSE (fabricated constant) |
| 0.0% false hits | README | `precision=100` when `tp+fp==0`; reused=0 | VACUOUS |
| 12,450 req/s / P99 1.65 ms | `docs/operations/soak_and_capacity_benchmark.md` | contradicts checked-in artifact 2,500.9 req/s / 10.339 ms | CONTRADICTED |
| 14,166 req/s / P99 174 µs | `docs/architecture/performance_benchmark_report.md` | historical, self-bannered | OUTDATED |
| 15,428 req/s, GPT-4o 1450ms, Qdrant 1450 rps | `research/paper/sunborn_ieee.tex` | no artifact | UNSOURCED |

## 2. Four incompatible performance tables

1. README: 571,428 / 3,257 / 142,857.
2. `docs/master/10`: L1 78 µs/45,000 req/s; semantic 2.8 ms/2,800 req/s.
3. `docs/architecture/performance_benchmark_report.md`: 14,166 req/s, P99 174 µs.
4. `research/paper/sunborn_ieee.tex`: 15,428 req/s, 9,800 req/s Dragonfly.

No two agree; none trace to one reproducible command.

## 3. Benchmark methodology

- `benchmarks/semantic_cache_bench.py`: synthetic 384-d hash vectorizer by default; FastEmbed only with `--use-fastembed`.
- `adversarial_evaluation_suite.py`: SHA-256 trigram hash embeddings.
- `graph_invalidation_deep_benchmark.py`: "MockRoaringBitmap" = `dict[int,set]`; memory constants invented.
- `capacity_soak.py`: live HTTP but prints `100% STABLE (PASS)` unconditionally (`:474-475`).
- `graph_storage_and_policy_bench.py`: Neo4j latency/memory and bytes/edge are invented constants.
- No Criterion, no `[[bench]]` target — despite `docs/master/10:11` claiming Criterion.

## 4. The one non-vacuous result

`research/results/benchmark_run.json` (124 queries) is internally consistent and `research/verify_certificate.py` genuinely recomputes 12 summary fields per baseline. But:
- embedding backend is LSH,
- labels are author-assigned,
- cost model is `SYNTHETIC_COST_ESTIMATION`,
- "live gateway" latency is loopback `urllib` p50 0.829 ms.

## 5. Real measured numbers available today

- `performance_test.rs` prints a machine-dependent in-process cache loop (not a gateway SLO).
- Capacity soak artifact: **2,500.9 req/s, P99 10.339 ms** (from `docs/audit/DEFECT_REGISTER.md:29`).
- Graph artifact: p50 4.0 µs, p99 3.806 ms at 352,578 edges.

These are the only defensible figures and they contradict the README.

## 6. Hardware claims

- README: 16 vCPU EPYC 7763, 64 GB, Ubuntu 22.04.
- Paper: 64-core EPYC, 128 GB, Linux 6.12.
- Certificate platform: RHEL-family 6.12.0-211.64.1.el10_2.

Three different machines.

## 7. Verdict

Benchmark credibility: **LOW**. The repo contains a genuinely good artifact verifier wrapped around a synthetic evaluation, plus fabricated constants and a nonexistent cited file. Performance score: **3/10**.
