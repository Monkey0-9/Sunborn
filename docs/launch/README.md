# Launch Package

Sunborn is brought to market in stages. This directory is the authoritative launch
surface. It states what is authorized now, what must be true before each next stage,
how the free tier is designed, what service levels are promised, and what the trust
posture is.

Read in order:

1. [Launch plan](01_LAUNCH_PLAN.md) - stages, gates, risks, rollback.
2. [Free-tier architecture](02_FREE_TIER_ARCHITECTURE.md) - tenancy, quotas, abuse
   controls, cost containment.
3. [Capacity and SLOs](03_CAPACITY_AND_SLO.md) - measured baseline, planning model,
   service levels, error budget.
4. [Trust center](04_TRUST_CENTER.md) - data handling, encryption, compliance status,
   disclosure.
5. [Prelaunch checklist](05_PRELAUNCH_CHECKLIST.md) - the items that must be closed,
   each tied to an artifact.

---

## Current status

| | |
|---|---|
| **Authorized today** | Controlled pilot with named design partners |
| **Not authorized today** | Public sign-up, unattended multi-tenant traffic, multi-region |
| **Blocking public launch** | Live multi-replica invalidation run; external penetration test |
| **Status authority** | [`docs/STATUS.md`](../STATUS.md) |

The single most important sentence in this package: **a green build is not a launch.**
The build proves the code does what the tests say. A launch additionally requires that
the environment-dependent gates have run, that capacity has been measured at the
target load, and that the trust claims have been checked against the evidence.

---

## What this package is for

It exists so that three audiences can get an honest answer quickly:

- **A design partner** asks: what can I rely on today, and what happens when something
  fails?
- **An operator** asks: what do I deploy, what do I watch, and when do I roll back?
- **A reviewer or board member** asks: what is claimed, what is proven, and what is
  still open?

Every answer links to an artifact or is labeled as a planning assumption. There is no
third category.

---

## What this package will not do

These are commitments about how we communicate, not features:

- **No headcount claims.** The quality of the system is demonstrated by its evidence,
  not by a roster. Nothing in this repository asserts a team size, and no launch
  material will.
- **No borrowed credibility.** No customer logo, quotation, certification mark, or
  partnership is displayed without written permission and a document that supports it.
- **No competitor disparagement.** Comparisons are between architectural strategies and
  measured behaviors, not between companies. Where a comparison is useful it names the
  strategy, not a vendor.
- **No unmeasured scale claims.** Figures such as "millions of users" are targets with a
  model behind them, labeled as such, until they are measurements.
- **No silent retreat from a claim.** If a claim is withdrawn, the withdrawal is visible
  in [`CHANGELOG.md`](../../CHANGELOG.md) with the reason.

The project's credibility comes from the opposite of inflation: every number has a
home, and the places where the evidence is thin are stated next to the claim.
