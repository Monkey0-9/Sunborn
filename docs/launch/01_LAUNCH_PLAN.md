# Launch Plan

**Owner:** release authority (see [`GOVERNANCE.md`](../../GOVERNANCE.md) section 5)
**Status:** controlled pilot authorized; public stages gated
**Authority for current status:** [`docs/STATUS.md`](../STATUS.md)

Part of the [launch package](README.md).

---

## 1. Thesis

Most gateways compete on hit rate. That is the easy number to move and the easiest way
to serve a wrong answer. Sunborn's position is narrower and harder: **reuse is granted
only when it can be proven safe**, and the system abstains when it cannot. The product
that results is valuable for exactly the workloads where a wrong cached answer is
expensive - regulated processes, financial operations, clinical and security workflows,
and any agent that acts on its output rather than only displaying it.

The launch therefore does not promise "the most cache hits". It promises a defensible
decision at every reuse, a complete record of why the decision was made, and a
predictable path upstream when the answer is not safe to reuse.

---

## 2. Stages

| Stage | Audience | Authorized by | Entry gate |
|---|---|---|---|
| **0. Controlled pilot** | Named design partners | Current verdict | Met (see [`docs/STATUS.md`](../STATUS.md) section 1) |
| **1. Private beta** | Invitation only, bounded population | New verdict after N1-N4 | Live distributed harness; external penetration test; pilot operability; P0/P1 closed |
| **2. Public free tier** | Self-serve, rate-limited | New verdict after B1-B5 | Load test; cost ceiling held; abuse red-team; freshness SLO; support load |
| **3. Paid and enterprise** | Teams that outgrow the free tier | New verdict after L1-L5 | SLO quarter; capacity headroom; failover drill; trust-center review; rollback drill |

Stage criteria are defined in [`ROADMAP.md`](../../ROADMAP.md). A stage does not begin
because a date arrived. It begins because its gate passed and the verdict changed.

---

## 3. Go / No-Go

The release authority records one verdict per stage. A GO requires every line below to
be true and linked to evidence.

| # | Requirement | Evidence | Status |
|---|---|---|---|
| G1 | All in-tree verification gates pass, or a skipped gate is named with its reason | [`scripts/verify.sh`](../../scripts/verify.sh) output | Pass for in-tree gates; environment gates named |
| G2 | Live multi-replica invalidation harness passes all six modes | Recorded harness run | **Open** |
| G3 | External penetration test complete; no open critical or high | Signed report; closed-findings register | **Open** |
| G4 | Capacity measured at the stage's target load, with headroom for one replica failure | Load-test report | **Open for stage 2+** |
| G5 | Free-pool spend stays under its ceiling for a full cycle | FinOps ledger export | **Open for stage 2+** |
| G6 | Abuse controls survive a red-team pass | Red-team report | **Open for stage 2+** |
| G7 | Trust-center claims match the evidence register | Review record | **Open for stage 2+** |
| G8 | Rollback executed in staging within the target window | Drill record | **Open for stage 3** |

No-Go is the default. A missing artifact is a No-Go, not a MAYBE.

---

## 4. Risk register

Risks are ranked by the product of likelihood and blast radius. Each has a named
control and an early warning signal. Owners are functions, consistent with
[`GOVERNANCE.md`](../../GOVERNANCE.md) section 9.

| Risk | Why it matters | Control | Early warning | Owner |
|---|---|---|---|---|
| **False-hit regression** | A wrong reused answer is the product's defining failure | Structural guardrails, causal check, abstention; held-out test asserts counts | Precision drop in evaluation; any confirmed false hit | Research + Backend |
| **Stale read after mutation** | Cached answer outlives the document it depends on | Causal invalidation, fail-closed on graph error | Stale-read count > 0 in harness | Backend |
| **Upstream cost overrun on free tier** | Free traffic can consume unbounded provider spend | Per-tenant and global ceilings; model allowlist; token caps; cache-first | Spend velocity approaching ceiling | SRE + FinOps |
| **Quota evasion and abuse** | Free tier invites key farming and enumeration | Verified identity, rotation controls, anomaly detection, revocation | Spike in new tenants with correlated fingerprints | Security |
| **Dependency outage** | Cache, database, or vector store down | Fail closed with 503; no unguarded fallback | Readiness probe failures; error-rate rise | SRE |
| **Capacity surprise at launch** | A launch spike degrades every tenant | Load shedding with Retry-After; measured headroom; HPA from load model | Queue depth; P99 rise; 429 rate | SRE |
| **Support saturation** | Free tier generates disproportionate support load | Tiered support; self-serve diagnostics; documented runbooks | Ticket age; repeat-contact rate | Support + Docs |
| **Trust overclaim** | A certification or scale claim that cannot be supported | Evidence register; trust-center review gate | Any claim without a linked artifact | Docs + Security |
| **Provider policy change** | An upstream changes terms or wire behavior | Provider abstraction; contract tests; documented fallback | Contract-test drift; provider notices | Backend + Architecture |

The register is reviewed at every verdict change and after every incident.

---

## 5. Rollback and communications

- **Trigger authority.** The on-call engineer may trigger rollback without prior
  approval. The decision is documented immediately afterward.
- **Target.** Return to the last known-good release within the window stated in the
  release record. Data compatibility is stated per release; a rollback that cannot read
  the current schema is not a valid rollback and must be caught in staging.
- **Communication.** Status page first, then affected tenants by the channel they
  configured. The first message states impact and the safe action; detail follows.
- **No quiet rollback.** A rollback is recorded in [`CHANGELOG.md`](../../CHANGELOG.md)
  with the reason. A silent revert repeats the mistake.

---

## 6. Decision log

Every stage transition is a dated entry. The log is append-only and lives here.

| Date | Decision | Authority | Evidence |
|---|---|---|---|
| 2026-11-17 | CONDITIONAL GO - controlled pilot | Release authority | [`docs/STATUS.md`](../STATUS.md) |
| _pending_ | Stage 1 private beta | _pending_ | Requires N1-N4 |

An entry is added when the decision is made, not when the stage is announced.

---

## 7. What "launch" means for this product

A launch is not a landing page going live. For a system in the request path it means:

1. A named population may send real traffic.
2. The failure modes have been exercised, not only designed.
3. The operator can detect, contain, and roll back without the authors present.
4. The published claims are true at the published scope.

Until all four hold, the correct word is **pilot**.
