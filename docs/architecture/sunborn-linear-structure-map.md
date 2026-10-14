# SUNBORN — Linear Structure Map

# SUNBORN — LINEAR STRUCTURE MAP

## The one rule that makes the project easy to understand

**Parent = WHO owns the work.**
**Milestone = WHEN in the 0–28 execution sequence the work happens.**
**Label = WHAT kind of work it is.**
**Dependency = WHAT must finish before it can start.**
**GitHub PR = HOW the implementation is delivered.**

Never use random parent/phase combinations.

## Top-level hierarchy

SUN-000 — Master Execution Control
├── PHASE 00 … PHASE 28 — phase gates / sequence
├── WS-00 — Integration & Vertical Slices
├── WS-01 — Architecture & Governance
├── WS-02 — DevEx, GitHub & CI/CD
├── WS-03 — Backend & API
├── WS-04 — Frontend Control Plane
├── WS-05 — Data & Storage
├── WS-06 — Cache & Decision Intelligence
├── WS-07 — Execution Graph & Reuse
├── WS-08 — Knowledge, Freshness & Invalidation
├── WS-09 — QA, Security & Validation
├── WS-10 — SRE, Deployment & DR
├── WS-11 — Research, Benchmarking & Scientific Validation
└── WS-12 — Reproducibility, Acceptance & Publication

## Leaf issue structure

Example:

WS-06 | Cache & Decision Intelligence
└── CACHE-INT-009 Decision state machine
    Milestone: PHASE 7 — Cache Decision Engine
    Labels: type:engineering + area:decision
    Dependency: semantic-cache interfaces
    Output: focused GitHub PR + tests + evidence

This means:

* Ownership is shown by the parent.
* Timeline is shown by the milestone.
* Work category is shown by labels.
* Blocking sequence is shown by dependencies.

## Phase structure

Phase milestones are chronological gates. Use the milestone board for the A→Z roadmap.

Phase 0:
Architecture / requirements / research contract; no coding.

Phase 1:
Developer platform / Rust foundation / local environments / CI foundations.

Phase 2:
Gateway identity / AuthN/AuthZ / tenancy / rate limits / request security.

Phase 3:
Canonical request / deterministic serialization / equivalence.

Phase 4:
Versioned cache identity / model and policy state.

Phase 5:
Dragonfly exact cache.

Phase 6:
Qdrant semantic cache.

Phase 7:
Decision / confidence / risk / policy.

Phase 8:
Admission.

Phase 9:
Execution DAG / structured artifacts.

Phase 10:
Knowledge dependency graph.

Phase 11:
Events / outbox / invalidation.

Phase 12:
Freshness / consistency / temporal validity.

Phase 13:
Shadow evaluation / drift.

Phase 14:
Benchmark and dataset platform.

Phase 15:
Baselines.

Phase 16:
Research experiments / ablations.

Phase 17:
Security adversarial validation.

Phase 18:
Multi-tenancy isolation.

Phase 19:
Observability / SRE telemetry.

Phase 20:
FinOps / real net savings.

Phase 21:
Chaos / reliability.

Phase 22:
Kubernetes.

Phase 23:
Disaster recovery.

Phase 24:
Performance.

Phase 25:
Capacity.

Phase 26:
Governance / lifecycle.

Phase 27:
Production readiness.

Phase 28:
IEEE/ACM publication and artifact release.

## How to use Linear

### Roadmap view

Group by milestone. This answers: “What happens next?”

### Ownership view

Group by parent/workstream. This answers: “Who owns this?”

### Research view

Filter parent = WS-11 and labels type:research/type:experiment. This answers: “What are we proving?”

### PR view

Filter labels type:engineering/type:security/type:benchmark and state needs-review. This answers: “What needs code review?”

### Risk view

Filter blocked + gate:security + gate:benchmark + gate:research.

## Rules

 1. No leaf issue without a parent workstream.
 2. No implementation issue without a milestone.
 3. No major architecture decision without ADR evidence.
 4. No research claim without an experiment/result.
 5. No benchmark result without dataset/config/version provenance.
 6. No security closure without attack/fix/retest evidence.
 7. No PR without a Linear issue.
 8. One conceptual change per PR.
 9. Do not create duplicate issues to represent the same work.
10. Do not add technology without a spike/kill-test and decision record.

## Why this is structured

Linear now has two independent dimensions:

* **Hierarchy**: Master → Workstream → Issue.
* **Sequence**: Phase 0 → Phase 28.

This prevents cross-cutting frontend/backend/security/research work from being scattered while still preserving a strict implementation order.
