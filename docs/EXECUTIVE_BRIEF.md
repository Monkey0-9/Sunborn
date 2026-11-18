# Sunborn — Executive Brief

_One page. Every number below links to a checked-in artifact you can re-run. Last verified 2026-11-18._

## What it is

A gateway that sits between an application and a language model. It answers from cache when it can show
that a request is semantically equivalent to one it already answered, and forwards to the origin model
when it cannot. Three tiers: exact match, semantic match with structural guardrails, and causal
invalidation that purges downstream entries when a source changes. The core is Rust; there is a Next.js
console and a Python SDK.

## The decision being asked for

Approve a **controlled pilot on one non-critical workload**, with a defined rollback path. This is not a
request to put it in the production critical path.

## What is actually measured

| # | Claim | Measurement | Artifact | What it does not mean |
| :-- | :--- | :--- | :--- | :--- |
| 1 | L1 cache tier throughput | 1,638,227 req/s · P50 0.300 µs · P99 1.100 µs | [`benchmarks/results/l1_throughput_run.json`](../benchmarks/results/l1_throughput_run.json) | In-process tier on one machine. Not an end-to-end gateway or network figure. |
| 2 | Gateway throughput | 13,241.1 req/s sequential · P50 0.070 ms · P99 0.166 ms (5,000 requests); 11,116.0 req/s · P50 1.368 ms · P99 2.398 ms with 16 concurrent workers (10,000 requests, 0 failures) | [`benchmarks/results/gateway_performance_run.json`](../benchmarks/results/gateway_performance_run.json), [`benchmarks/production_soak_results.json`](../benchmarks/production_soak_results.json) | In-process router on one machine, no network hop. Concurrency costs latency here: 16 workers move P50 from 0.070 ms to 1.368 ms. |
| 3 | Held-out decision safety | 320 pre-registered pairs: 24 true positives, **0 false positives**, 200 true negatives, 96 false negatives. Precision 100% (95% lower bound 86.2%), recall 20% (13.8–28.0%) | [`research/results/heldout_generalization_run.json`](../research/results/heldout_generalization_run.json) | When it reused a cached answer in this set it was right every time. It also declined 96 of 120 legitimate reuses. |
| 4 | Quality-adjusted cost | 25 queries. Direct origin: $0.1806. Conventional cosine cache: $0.0146 (**91.9% less**) with 13 of 23 reuses wrong and 5 of 6 post-mutation reads stale. Sunborn: $0.1229 (**32.0% less**) with 0 wrong reuses and 0 stale reads. | [`benchmarks/results/competitive_benchmark_run.json`](../benchmarks/results/competitive_benchmark_run.json) | Real spend on a small synthetic workload with a fixed token-cost model, not an invoice. |
| 5 | Engineering coverage | 362 automated tests (297 Rust across 42 suites, 38 frontend, 27 Python); 12-gate pipeline green 2026-11-18 | [`docs/STATUS.md`](STATUS.md) | Tests prove regression safety, not market fit. |
| 6 | Invalidation correctness | TLA+ models of epoch fencing and the outbox protocol checked in CI; two-replica Redis Streams harness across six modes in CI | [`scripts/run_tlaplus.sh`](../scripts/run_tlaplus.sh), [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) | Model checking bounds a protocol design; it does not prove a deployment. |
| 7 | Supply chain | Deterministic CycloneDX SBOM, 369 components, reproducible `sha256=a1dec469c72d` | [`sbom.cyclonedx.json`](../sbom.cyclonedx.json) | A complete inventory, not an absence of vulnerabilities. |

## What is not established

- **Recall is 20%.** This is a precision-first system today. Cost savings are bounded by how often it can
  safely reuse, and the honest savings number for your workload will be lower than row 4 until recall rises.
- **A conformal risk guarantee of α ≤ 0.001 is not established.** The 99.9% Clopper–Pearson upper bound on
  the false-positive rate is 3.39%, so the dataset cannot yet support that claim.
- **No external penetration test** has been performed. The security posture is fail-closed by construction
  and reviewed internally.
- **Latency is a hit-rate story.** On a miss you pay origin latency plus roughly 2 ms of gateway overhead.
  The wins in row 4 come from hits; there is no win on a cold workload.
- **Distributed modes run in CI containers**, not yet on real multi-node infrastructure with real failure
  injection.
- **No SLA and no hosted service.** Support expectations are in [`SUPPORT.md`](../SUPPORT.md).

## The one interpretation worth taking away

Row 4 is the argument. The cheapest configuration in that table spends about 8× less than Sunborn — because
it answers wrong: 13 of its 23 reuses were incorrect, and after a data mutation 5 of 6 reads returned stale
values. For financial or clinical text that is not a saving; it is an incident with a discount.

So the question to put to any caching layer, including this one, is not "what is your hit rate?" but
"what happens when you are wrong, and how do we know?" Here: abstain and call the origin (rows 3, 4), purge
causally on mutation (row 6), and publish the rate at which you abstain rather than only the rate at which
you hit.

## Verify it yourself

```bash
cargo test --workspace --all-targets          # 297 tests
python scripts/verify_doc_paths.py --strict   # every path claim resolves
bash scripts/verify.sh                        # all 12 gates
```

Row 1 reproduces with `cargo bench -p sunborn-cache --bench throughput_test`; row 4 with
`python benchmarks/competitive_evaluation.py`. Throughput numbers are properties of the machine — expect
different absolute values on your hardware, and treat the ratio between configurations as the finding.

## Current verdict

**CONDITIONAL GO — controlled pilot.** Production is blocked on a live multi-replica invalidation run on
real infrastructure and an external penetration test. [`docs/STATUS.md`](STATUS.md) is the single source of
truth for release status and supersedes any readiness claim in older audit documents.
