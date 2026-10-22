# SUNBORN — Linear Structure Map

> **Historical planning snapshot.** “VERIFIED” labels in this map are not current implementation or phase-closure evidence. Linear's `MON-458` record reports that Phase 20–28 issue-level controls are missing. Use [`project_completion_assessment_2026-10-22.md`](project_completion_assessment_2026-10-22.md) and the release acceptance record for current status.

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
Scientific publication manuscript and empirical artifact release.

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

---

## October 21, 2026 Master Completion Audit & Institutional-Readiness Mapping (MON-458 / MON-526)

The findings from the **Master Completion Audit and Institutional-Readiness Plan (October 21, 2026)** are mapped directly into the authoritative Linear hierarchy and phase milestones:

| Issue Identifier | Parent Workstream | Chronological Milestone | Scope / Invariant Enforced | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AUDIT-P0-001** | `WS-09` (QA, Security & Validation) | `PHASE 18` — Multi-Tenancy Isolation | Route-by-route authorization matrix (`docs/security/authorization_matrix.md`); tenant identity strictly from verified credentials in `SecurityContext`. | **VERIFIED** |
| **AUDIT-P0-002** | `WS-05` (Data & Storage) | `PHASE 23` — Disaster Recovery | Authoritative state inventory; PostgreSQL durable schema for wrapped TDEKs and API credentials (`migrations/0002_durable_identity_and_keys.sql`). | **VERIFIED** |
| **AUDIT-P0-003** | `WS-06` (Cache & Decision Intelligence) | `PHASE 07` — Cache Decision Engine | Proof-carrying admissibility contract; invalid/expired/tampered reuse certificates fail safely by evicting entry and falling through to fresh origin execution. | **VERIFIED** |
| **AUDIT-P0-004** | `WS-01` (Architecture & Governance) | `PHASE 26` — Governance / Lifecycle | Cryptographic key separation (KEK vs Admin vs HMAC); fail-closed Helm secret rendering without default production database URLs. | **VERIFIED** |
| **AUDIT-P0-005** | `WS-11` (Research, Benchmarking) | `PHASE 28` — Scientific Publication | Transparent empirical evidence bounds; Wilson score confidence intervals ($N=124$); honest unauthenticated local integrity digest verification. | **VERIFIED** |
| **AUDIT-P1-001** | `WS-03` (Backend & API) | `PHASE 02` — Gateway Identity & API | Full route/schema/auth parity between OpenAPI 3.1 (`docs/api/openapi.yaml`) and Axum handlers; RFC 7807 problem details across all 37 endpoints. | **VERIFIED** |
| **AUDIT-P1-002** | `WS-03` (Backend & API) | `PHASE 01` — Developer Platform | Python SDK session lifecycle enforcement (`is_closed`), prohibiting post-close requests with `RuntimeError`; clean wheel installation. | **VERIFIED** |
| **AUDIT-P1-003** | `WS-10` (SRE, Deployment & DR) | `PHASE 21` — Chaos / Reliability | Graceful shutdown `state.graceful_drain()` flushing outbox WAL; drain deadline timeout logs error and exits with code 75 (`EX_TEMPFAIL`). | **VERIFIED** |
| **AUDIT-P1-004** | `WS-09` (QA, Security & Validation) | `PHASE 02` — Gateway Identity & Auth | Authenticated tenant rate limiting extracting tenant from `SecurityContext`; returns `x-ratelimit-limit`, `x-ratelimit-remaining`, `x-ratelimit-reset`, and `retry-after`. | **VERIFIED** |
| **AUDIT-P1-005** | `WS-10` (SRE, Deployment & DR) | `PHASE 19` — Observability / Telemetry | Correlated structured logging (`x-request-id`); Prometheus metric scraping (`/metrics`) with bounded tenant cardinality to prevent label explosion. | **VERIFIED** |
| **AUDIT-P1-006** | `WS-10` (SRE, Deployment & DR) | `PHASE 22` — Kubernetes | PodDisruptionBudget (`minAvailable: 1`), non-root container profile (`UID 1000`), NetworkPolicy, and values JSON schema validation. | **VERIFIED** |

