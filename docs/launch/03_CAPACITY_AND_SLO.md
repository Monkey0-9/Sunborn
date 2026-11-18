# Capacity and Service Levels

**Owner:** SRE and platform
**Status:** measured single-node baseline; beta capacity model and SLOs are proposals
**Authority for current status:** [`docs/STATUS.md`](../STATUS.md)

Part of the [launch package](README.md).

---

## 1. Scope and the rule for this document

This document separates three things that are usually mixed together:

1. **Measured baseline** — numbers produced by a checked-in artifact on a named
   machine, with the measurement conditions stated.
2. **Planning model** — arithmetic that turns the baseline into replica and cost
   estimates. Every non-measured input is labeled as an assumption.
3. **Service-level objectives** — internal targets for the beta and public stages.
   They are not contractual today, and no hosted service currently offers an SLA.

If a figure in launch material cannot be traced to section 2 or labeled as an
assumption from section 4, it is not a capacity claim. It is a guess with a decimal
point.

---

## 2. Measured baseline

All figures below are single-machine, in-process measurements from the artifacts
named. They are not end-to-end figures: there is no TCP, no TLS, and no network hop.

| Measurement | Result | Conditions | Artifact |
|---|---|---|---|
| L1 exact cache, sequential get | 2,543,687 req/s · P99 1 µs | 10,000 calls; 1 µs timer, so sub-microsecond reads report as 0 | [`benchmarks/results/gateway_performance_run.json`](../../benchmarks/results/gateway_performance_run.json) |
| L1 exact cache, mixed get/insert | 1,638,227 req/s · P50 0.300 µs · P99 1.100 µs | 100,000 operations; release bench profile | [`benchmarks/results/l1_throughput_run.json`](../../benchmarks/results/l1_throughput_run.json) |
| Full gateway stack, sequential | 13,241.1 req/s · P50 0.070 ms · P95 0.099 ms · P99 0.166 ms | 5,000 requests; auth, routing, canonicalization, exact-cache admission; in-process `oneshot` | [`benchmarks/results/gateway_performance_run.json`](../../benchmarks/results/gateway_performance_run.json) |
| Full gateway stack, 16 workers | 11,116.0 req/s · P50 1.368 ms · P99 2.398 ms · 0 failures | 10,000 requests; arrives over 16 concurrent Tokio workers | [`benchmarks/production_soak_results.json`](../../benchmarks/production_soak_results.json) |

Machine for all four rows: Windows 11, release profile, Rust 1.96.0, Intel64 Family 6
Model 183, 20 logical cores. Absolute values are properties of that machine. The ratio
between the L1 tier and the full stack travels better than the absolute numbers do.

Two cautions that should travel with every quotation:

- **The concurrent run is a burst, not an endurance test.** It completes in under a
  second and says nothing about memory growth over hours.
- **The sequential and concurrent runs differ by design.** Sixteen workers move P50
  from 0.070 ms to 1.368 ms. Quoting the sequential P50 as a concurrent expectation
  would be a misuse of the artifact.

---

## 3. What the baseline does not cover

The beta capacity model needs measurements this repository does not yet have. These
are not footnotes; they are the difference between a model and a wish.

| Missing measurement | Why it matters | How to close it |
|---|---|---|
| Sustained load for at least 60 minutes | Memory growth, connection-pool exhaustion, and GC-like pauses are invisible in a one-second burst | B1 load test |
| Network and TLS overhead | A real deployment adds at least one hop, TLS termination, and often a proxy | Staging load test at the target topology |
| PostgreSQL latency under concurrent writes | Spend reservation and settlement are transactional and advisory-locked | B1 with production-like database |
| Dragonfly/Redis Streams backlog and replay | Invalidation is at-least-once with an outbox; backlog changes freshness, not only CPU | B4 sustained mutation |
| Qdrant search latency at the target collection size | Vector search cost grows with collection size and filter cardinality | B1 with a production-sized collection |
| Embedding provider latency and rate limits | A semantic path that waits on embeddings can dominate P99 | B1 with the configured provider |
| Upstream provider limits | Origin quota, not gateway CPU, may be the binding constraint | Provider quota review plus B1 |
| Bytes per cache entry and per vector point | Memory sizing and eviction behavior need a measured distribution, not an average | Cache memory profile |
| Hit-rate mix at beta | Every capacity number depends on the exact/semantic/miss split | B1 with the recorded request mix |
| Cold-start time | A new replica that takes minutes to become ready changes the failure budget | Startup measurement |

Until these exist, the model below is explicitly a planning tool.

---

## 4. Planning model

### 4.1 Replica throughput

Use the measured 16-worker figure, derate it for network and dependency latency, then
reserve headroom:

```
R_eff = R_measured × d × (1 - h)

R_measured = 11,116 req/s      (single machine, in-process, 16 workers)
d          = 0.25              (derating assumption, unvalidated)
h          = 0.25              (headroom for one replica loss and burst)
R_eff      = 11,116 × 0.25 × 0.75 ≈ 2,084 req/s per replica
```

The derating factor is the most important number in this document and it is an
assumption. It must be replaced by a staging measurement before B1. If the measured
derated value is lower, the replica count rises; the formula does not change.

### 4.2 Replica count

```
N = ceil(P_peak / R_eff) + 1
```

The `+1` is the N+1 rule: one replica may be unavailable without violating the
latency objective. The Helm defaults are 3 replicas, autoscaling to 10 at 75% CPU and
80% memory ([`deploy/helm/sunborn/values.yaml`](../../deploy/helm/sunborn/values.yaml)).
Those are deployment defaults, not a capacity proof. CPU is a weak signal for a
gateway that may be waiting on PostgreSQL, Redis, Qdrant, or an upstream provider;
the autoscaling policy should be revisited after B1.

Worked example, explicitly illustrative: a beta peak of 5,000 req/s gives
`ceil(5,000 / 2,084) + 1 = 4` replicas. The same arithmetic at 10,000 req/s gives 6.
Neither number is a commitment until the request mix and derating are measured.

### 4.3 Request mix and dependency budget

Capacity is not a single number. Each request takes one of three paths:

| Path | Gateway work | External work | Latency driver |
|---|---|---|---|
| Exact hit | Canonicalize, authenticate, lookup, decrypt, attest | Cache read | Gateway and cache |
| Semantic hit | Guardrails, embedding, vector search, causal check, attest | Embedding provider, Qdrant, graph store | Embedding and vector search |
| Miss | Guardrails plus origin dispatch and admission | Everything above plus the model provider | Upstream model |

The planning mix must be recorded **before** the load test. A test that discovers its
mix after the fact is a demonstration, not a measurement. The current evidence gives a
32% verified hit rate on a 25-query comparative workload and 20% recall on the held-out
set; neither is a forecast for beta traffic
([`benchmarks/results/competitive_benchmark_run.json`](../../benchmarks/results/competitive_benchmark_run.json),
[`research/results/heldout_generalization_run.json`](../../research/results/heldout_generalization_run.json)).

### 4.4 Free-tier pool sizing

```
Cost_day  ≤ min( requests_day × avg_cost_per_request,
                 tokens_day   × blended_price_per_token,
                 velocity_ceiling × minutes_per_day )
Pool_cost ≤ Σ Cost_day over free tenants, and Pool_cost ≤ G
```

`G` is the global free-pool budget for the period. The per-tenant limit and the global
limit are separate controls: many tenants each inside their limit can still exceed the
pool. See [`02_FREE_TIER_ARCHITECTURE.md`](02_FREE_TIER_ARCHITECTURE.md) section 6 for
the proposed envelope and the arithmetic.

### 4.5 Memory and storage

There is no measured bytes-per-entry distribution in this repository. Sizing cache
memory from a guessed average is how a gateway reaches its byte budget in production
and starts evicting the entries it depends on. Before beta:

1. measure the entry-size distribution for the target workload;
2. set the Greedy Dual Size Frequency (GDSF) byte budget from the high percentile,
   not the mean;
3. measure Qdrant storage per point at the target dimension and filter cardinality;
4. leave headroom for the invalidation graph, which is roughly 1.8 bytes per edge in
   the accelerator design but has not been measured at production scale.

---

## 5. Service-level objectives

These are **internal beta objectives**, not a published SLA. A target becomes a
commitment only when a contract says so and the evidence register shows it has been
met for the stated period. The indicators are service-level indicators (SLIs); the
targets are service-level objectives (SLOs).

### 5.1 Service-level indicators

| SLI | Definition | Beta objective | Public target | Measurement |
|---|---|---|---|---|
| Availability | Non-`5xx` responses ÷ all eligible responses; `4xx` quota rejections are excluded | 99.5% per 30 days | 99.9% per 30 days | Gateway metrics and synthetic probes |
| Gateway overhead | Time inside the gateway excluding origin and network; P99 | ≤ 5 ms | ≤ 5 ms | Histogram in `/metrics` |
| Exact-hit latency | Client-observed P99 for a request served from the exact cache | ≤ 50 ms | ≤ 50 ms | Synthetic probe |
| Miss-added latency | Gateway time added to an origin response; P99 | ≤ 5 ms | ≤ 5 ms | Histogram comparison |
| Freshness | Mutation commit to confirmed purge completion; P99 | ≤ 60 s | ≤ 60 s | B4 harness |
| Stale reads | Reads served from an entry whose dependency mutated before the read | 0 observed | 0 observed | Harness and incident review |
| Correctness | Confirmed false hits ÷ confirmed reuses | 0 observed | 0 observed | Held-out evaluation plus incident review |
| Quota accuracy | Rejections that should have been admitted, and admissions that should have been rejected | 0 | 0 | Counter reconciliation |

A 99.5% monthly objective permits 3 h 36 min of unavailability in a 30-day month.
A 99.9% objective permits 43 min 12 s. Those budgets are for planning; they are not
currently promised to anyone.

### 5.2 Error-budget policy

| Budget state | Action |
|---|---|
| More than 50% remaining | Normal release cadence |
| 25–50% remaining | Review recent changes; no capacity-reducing deploy without SRE sign-off |
| 10–25% remaining | Feature freeze for the affected surface; reliability work takes priority |
| Exhausted | Freeze launches, roll back the suspected change, and record the review in the incident log |

The error budget is computed from the same SLIs above. A budget that cannot be
computed from an artifact is not an error budget.

### 5.3 Correctness objective, stated honestly

The measured result is 0 false hits on 200 hard negatives. That is not the same as a
false-hit rate below 0.1%: the exact Clopper–Pearson 99.9% upper bound is 3.39%.
Capacity and SLO documents must not convert an observed zero into a risk guarantee.
The objective is "zero confirmed false hits, with the upper bound reported", and the
number to improve is recall, which is 20% on the held-out set.

---

## 6. Load test required for B1

B1 asks for a load test at twice the projected beta peak with zero correctness
regressions. The test plan must be recorded before execution:

| Element | Requirement |
|---|---|
| Projection | Written peak RPS, tenant count, and request mix, with the derivation |
| Load | 2× the projected peak for at least 60 minutes after a 10-minute ramp |
| Mix | Exact-hit, semantic-hit, and miss proportions fixed in advance |
| Data | Production-sized cache, vector collection, and lineage graph |
| Dependencies | PostgreSQL, Dragonfly/Redis, Qdrant, and the configured embedding provider |
| Faults | One replica removed mid-run; one dependency restart if the stage scope allows |
| Correctness | False-hit count, stale-read count, invalidation lag, and quota-accuracy counters |
| Latency | P50, P95, P99 for each path, not only aggregate |
| Errors | HTTP status distribution, including 429 and 503 |
| Artifact | A new JSON report under `benchmarks/results/` with the environment and command |

Pass criteria: zero confirmed correctness regressions, error rate within the beta
objective excluding injected faults, P99 within the latency objectives, and capacity
headroom for one replica loss. A report that omits the mix or the environment is not
evidence.

### 6.1 Local quota rehearsal (not B1)

A two-process rehearsal against a local bundled PostgreSQL validated the quota path on
loopback HTTP with a debug build. It is recorded at
[`benchmarks/results/local_quota_rehearsal.json`](../../benchmarks/results/local_quota_rehearsal.json).

The rehearsal observed:

- a 2/day tenant limit returning `200`, `200` (exact cache hit), then `429`, with
  durable day and month counters at 2;
- a shared 4/day limit returning `200` from replica A, `200` from replica B, then `429`
  from replica A;
- a 1/day free pool returning `200`, then `429`, then `200` after the pool was raised,
  with tenant and pool counters both at 2, showing the denied request rolled back.

This is **not B1 evidence**. It does not include the 2× projection, a 60-minute run,
production-sized data, TLS/ingress, fault injection, or racing requests at the limit
boundary. Those remain open.

---

## 7. Capacity risks and triggers

| Risk | Early warning | First response |
|---|---|---|
| Database connection exhaustion | Pool wait time; `503` rate; reservation latency | Reduce replica concurrency; increase pool; scale database |
| Redis Streams backlog | Pending entries; invalidation lag | Scale consumers; inspect outbox; fail closed if freshness SLO is breached |
| Qdrant latency or memory | Search P99; collection size; segment count | Reduce top-K; add replicas; re-shard |
| Embedding provider throttling | 429 from provider; semantic-path P99 | Shed semantic path to abstain; never serve an unverified hit |
| Upstream provider quota | Origin 429/5xx; failover rate | Fail over; reduce free-tier admission; notify tenants |
| Cache memory pressure | Eviction rate; hit-rate drop | Raise byte budget or lower admission; do not disable eviction |
| Singleflight memory growth | In-flight map size; pod memory | Bound the map; shed duplicate streams |
| Log volume | Ingest cost; disk pressure | Sample; keep decisions and identifiers, not prompt bodies |
| Abuse spike | New tenants with correlated fingerprints; spend velocity | Suspend and review; lower pool ceiling |

Every risk above has a named owner function in
[`01_LAUNCH_PLAN.md`](01_LAUNCH_PLAN.md) section 4. A trigger without an owner is an
observation.

---

## 8. Reproduce the baseline

```bash
cargo test --release -p sunborn-gateway --test performance_test -- --nocapture
cargo bench -p sunborn-cache --bench throughput_test
cargo test --release -p sunborn-gateway --test concurrent_multiworker_soak_test -- --nocapture
```

The first command writes the gateway report; the second writes the L1 report; the third
produces the concurrent burst figures. Run them on the hardware you intend to quote.
Expect different absolute values, and treat the ratio between configurations as the
finding.

---

## 9. Open measurements before any public claim

- [ ] Derating factor `d` measured on staging at the target topology.
- [ ] Sustained 60-minute run with memory and connection-pool telemetry.
- [ ] Entry-size and vector-point-size distributions recorded.
- [ ] Dependency latency budgets measured, not assumed.
- [ ] Request mix fixed and recorded with the B1 report.
- [ ] Autoscaling policy validated against a load model rather than CPU alone.
- [ ] Error-budget computation implemented from the SLIs in section 5.
- [ ] Freshness SLO measured under sustained mutation (B4).
- [ ] Global free-pool reconciliation completed for a full cycle (B2).

Until these are checked, the only defensible statement is the one this document opens
with: the baseline is measured, and the model is a plan.
