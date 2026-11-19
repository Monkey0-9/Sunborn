# Roadmap

This roadmap is organized by **exit criteria, not dates**. A phase begins when the
previous phase's criteria are met and the release authority records that they are met in
[`docs/STATUS.md`](docs/STATUS.md). A date shown anywhere in this project is a target,
not a commitment, until the gate in front of it has passed.

The current release decision is **HOLD — STAGE 0 CONTROLLED PILOT REMEDIATION** ([`docs/STATUS.md`](docs/STATUS.md) and [`docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`](docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md)), with all 21 enterprise quality attributes verified and in-tree checks passing, while environment-dependent production gates N1-N4 remain open. Read that first: it defines the verified guarantees and operating boundaries.

---

## Principles

1. **Correctness before reach.** Reach that arrives before the safety evidence is reach
   that has to be withdrawn. Cache reuse is a risk decision, and the roadmap treats it
   that way.
2. **Gates, not wishes.** Each phase lists what must be true to enter it. If a gate is
   not met, the phase does not start; it does not start quietly with a smaller scope.
3. **No silent scope creep.** A feature that is not in the current phase is not built
   "while we are in there."
4. **Every phase leaves an artifact.** A phase is not complete because it feels
   complete. It is complete when the evidence it promised exists and is linked.

---

## Now - controlled pilot

**Authorized scope:** named design partners, non-public endpoints, operator-supervised
traffic.

**Already true (see [`docs/STATUS.md`](docs/STATUS.md) section 1):**

- Workspace builds, formats, and lints clean.
- Rust, frontend, and Python test suites pass.
- Formal models pass their TLC configurations.
- Held-out evaluation is pre-registered and asserted by a test.
- Route authorization parity is enforced at build time.

**Exit criteria to leave this phase:**

| # | Criterion | Evidence required |
|---|---|---|
| N1 | Live multi-replica invalidation harness passes all six modes on a real container runtime | Recorded run of [`scripts/test_distributed_invalidation.sh`](scripts/test_distributed_invalidation.sh) for `exact`, `semantic`, `replay`, `outbox`, `finops`, `crash` |
| N2 | External penetration test completed, findings triaged, no open critical or high | Signed engagement report and a closed-findings register |
| N3 | Pilot operators can run the system from the documentation alone | Onboarding transcript or pilot runbook sign-off |
| N4 | Every open P0/P1 from the current audit is closed with a regression test | Updated defect register |

**Explicitly not in this phase:** public sign-up, unattended multi-tenant traffic,
multi-region deployment.

---

## Next - private beta

**Entry gate:** N1-N4 complete and a new verdict recorded.

**Scope:** a bounded, invitation-only population on the managed service, with the
free-tier mechanics exercised at low volume before they are opened.

**Work:**

- Implement the free-tier architecture in
  [`docs/launch/02_FREE_TIER_ARCHITECTURE.md`](docs/launch/02_FREE_TIER_ARCHITECTURE.md):
  tenant class, quotas, global cost ceiling, abuse controls, and a clear upgrade path.
- Validate the capacity and SLO model in
  [`docs/launch/03_CAPACITY_AND_SLO.md`](docs/launch/03_CAPACITY_AND_SLO.md) at the
  beta load, and replace every planning assumption in it with a measurement or label it
  as still an assumption.
- Publish operational runbooks for the new failure modes: quota exhaustion, upstream
  provider outage, cache-store outage, and vector-store outage.
- Stand up a public status page and an incident communication path.

**Exit criteria to leave this phase:**

| # | Criterion | Evidence required |
|---|---|---|
| B1 | A load test at twice the projected beta peak, with zero correctness regressions | Load-test report with latency, error rate, and false-hit counts |
| B2 | Global free-pool spend stays under its ceiling for a full billing cycle | FinOps ledger export and reconciliation |
| B3 | Abuse controls hold against a red-team pass (key rotation, quota evasion, enumeration) | Red-team report and closed findings |
| B4 | Cache invalidation freshness meets its SLO under sustained mutation | Sustained-invalidation run with stale-read count |
| B5 | Support load is within the published response targets | Support queue metrics for the beta period |

---

## Later - public free tier

**Entry gate:** B1-B5 complete and a new verdict recorded.

**Scope:** self-serve access at public scale, with the free tier as the front door and a
paid path for teams that outgrow it.

**Work:**

- Multi-region deployment with regional data residency where required.
- Autoscaling policies derived from the measured load model, not from a guess.
- A published service-level objective with an error-budget policy.
- Self-serve onboarding, key management, and usage transparency.
- A pricing and packaging surface that does not change the safety guarantees of the
  free tier.

**Exit criteria for calling it launched:**

| # | Criterion | Evidence required |
|---|---|---|
| L1 | The SLOs in the capacity document have been met for a full quarter | SLO report |
| L2 | Capacity headroom is at least the modeled peak plus one replica per region | Capacity review |
| L3 | A regional failover has been exercised, not just designed | Failover drill record |
| L4 | Trust-center claims match the evidence register, with no certification implied that does not exist | Trust-center review against [`docs/launch/04_TRUST_CENTER.md`](docs/launch/04_TRUST_CENTER.md) |
| L5 | A rollback from the current release to the previous release has been executed in staging | Rollback drill record |

---

## Not planned

The following are deliberately excluded. They are not backlog items waiting for
capacity; they are decisions.

- **Similarity-only reuse.** Returning a cached answer because a vector score crossed a
  threshold, without structural and causal checks, is the failure mode this project
  exists to prevent.
- **Silent degradation.** Falling back to an unguarded cache, or to a local unenforced
  budget, when a dependency is down.
- **Certification theater.** Displaying a certification mark before an accredited body
  has issued it.
- **Growth claims without artifacts.** Headcount, customer logos, or market-position
  statements that the repository cannot substantiate.
- **Features that trade correctness for a headline number.** If a change improves a
  benchmark by weakening a guardrail, it is rejected.

---

## How to read progress

| Signal | Means |
|---|---|
| A phase gate is recorded in [`docs/STATUS.md`](docs/STATUS.md) | The phase may begin |
| A criterion has a linked artifact | The criterion is met |
| A criterion has prose but no artifact | The criterion is not met |
| An older document says a phase is complete | Check [`docs/STATUS.md`](docs/STATUS.md); history is not status |

The roadmap is reviewed whenever the verdict changes, and at least once per release
cycle. Changes follow the governance in [`GOVERNANCE.md`](GOVERNANCE.md).
