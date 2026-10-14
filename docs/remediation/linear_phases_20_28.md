# Sunborn Remediation: Phase 20–28 Execution Control Registry

> **Purpose**: Resolves the Linear Free-Tier issue creation blocker by providing an authoritative, Git-tracked issue-level execution backlog for Phases 20 through 28.  
> Every phase here maps 1:1 with the established Linear milestones and WS-00–WS-12 workstream hierarchy.

---

### PHASE 20 — FinOps Economics & Net Savings
* **Parent**: `WS-10 | SRE, Deployment & Disaster Recovery` / `WS-01`
* **Objective**: Measure and enforce net economic savings, token avoidance accounting, and dynamic cache admission budgeting.
* **Prerequisites**: Phase 19 (Observability & Telemetry).
* **Deliverables**:
  1. `crates/sunborn-core/src/finops.rs`: Pricing models for GPT-4o, Claude 3.5 Sonnet, and custom models.
  2. Amortized RAM storage cost vs token avoidance cost calculator.
  3. Dynamic admission filter rejecting candidates with expected negative net utility.
* **Acceptance Criteria**:
  - `FinOpsEngine::break_even_hit_rate` computes exact mathematical break-even point.
  - Zero admission of queries whose lifetime storage cost exceeds gross token savings.
* **Evidence**: Automated unit tests in `crates/sunborn-core/src/finops.rs`.

---

### PHASE 21 — Chaos & Reliability Engineering
* **Parent**: `WS-09 | QA, Security & Validation`
* **Objective**: Validate graceful degradation and zero-downtime origin fallback during external infrastructure outages.
* **Prerequisites**: Phase 20.
* **Deliverables**:
  1. Circuit-breaker middleware for DragonflyDB and Qdrant outages.
  2. Chaos test simulating Redis connection drop: gateway falls back to origin execution without returning HTTP 500.
* **Acceptance Criteria**:
  - 100% request success rate during synthetic cache backend crash.
* **Evidence**: Chaos integration test suite.

---

### PHASE 22 — Kubernetes Production Deployment
* **Parent**: `WS-10 | SRE, Deployment & Disaster Recovery`
* **Objective**: Package immutable container artifacts, Helm charts, and multi-replica deployment manifests.
* **Prerequisites**: Phase 21.
* **Deliverables**:
  1. `deploy/k8s/`: Deployment, Service, Ingress, and HorizontalPodAutoscaler (HPA) manifests.
  2. Multi-stage scratch/distroless Dockerfile for `sunborn-gateway`.
  3. Readiness & liveness probe configurations mapped to `/ready` and `/health`.
* **Acceptance Criteria**:
  - Clean container build under 35MB; passes vulnerability scanning with zero critical CVEs.
* **Evidence**: Dockerfile and Kubernetes manifests in `deploy/`.

---

### PHASE 23 — Disaster Recovery & Rebuild
* **Parent**: `WS-10 | SRE, Deployment & Disaster Recovery`
* **Objective**: Prove the Non-Accelerator Authority invariant by reconstructing Dragonfly exact cache and Qdrant vector index from PostgreSQL.
* **Prerequisites**: Phase 22.
* **Deliverables**:
  1. Rebuild CLI tool: `apps/sunborn-rebuild` scanning `cache_entries` in PostgreSQL and repopulating Dragonfly and Qdrant.
  2. Point-in-time recovery runbook.
* **Acceptance Criteria**:
  - 100% derived index restoration from PostgreSQL source of truth with zero data loss.
* **Evidence**: Disaster recovery replay test.

---

### PHASE 24 — Performance Engineering
* **Parent**: `WS-09 | QA, Security & Validation`
* **Objective**: Profile and optimize critical hot paths to meet p99 latency SLOs (<2ms exact, <25ms semantic).
* **Prerequisites**: Phase 23.
* **Deliverables**:
  1. Memory flamegraphs and CPU profiling of SHA-256 canonical hashing.
  2. Lock contention elimination using reader-writer locks and lock-free atomic counters.
* **Acceptance Criteria**:
  - Benchmark proves throughput > 10,000 requests/sec per gateway node on modern CPU.
* **Evidence**: Benchmark report and flamegraph profiles.

---

### PHASE 25 — Capacity Engineering
* **Parent**: `WS-10 | SRE, Deployment & Disaster Recovery`
* **Objective**: Validate saturation behavior, soak testing (24h continuous load), and memory leak absence.
* **Prerequisites**: Phase 24.
* **Deliverables**:
  1. Long-running soak test harness.
  2. Bounded memory eviction policy verification under 100,000 continuous entries.
* **Acceptance Criteria**:
  - RSS memory flatlines at configured cache capacity without unbounded growth.
* **Evidence**: Memory utilization metrics graph.

---

### PHASE 26 — Governance, Compliance & Data Lifecycle
* **Parent**: `WS-01 | Architecture & Governance`
* **Objective**: Enforce GDPR/CCPA tenant data deletion, retention TTLs, and cryptographic audit logs.
* **Prerequisites**: Phase 25.
* **Deliverables**:
  1. Cascade deletion API: `DELETE /v1/tenants/:id` deleting tenant data across PostgreSQL, Dragonfly, and Qdrant.
  2. Automated TTL expiration sweeper.
* **Acceptance Criteria**:
  - Verified zero lingering records following tenant offboarding request.
* **Evidence**: Data lifecycle compliance test suite.

---

### PHASE 27 — Production Readiness & Acceptance
* **Parent**: `WS-00 | Integration & Vertical Slices`
* **Objective**: Independent technical review, operational readiness checklist, and production cutover rehearsal.
* **Prerequisites**: Phase 26.
* **Deliverables**:
  1. Production operational runbook (`docs/runbooks/operations.md`).
  2. Signed-off release acceptance record.
* **Acceptance Criteria**:
  - All 28 upstream phase gates verified with passing tests and reproducible evidence.
* **Evidence**: Production readiness sign-off document.

---

### PHASE 28 — IEEE/ACM Research Publication Package
* **Parent**: `WS-12 | Reproducibility, Acceptance & Publication`
* **Objective**: Package the scientific methodology, empirical evaluation against Baselines B0–B6, and LaTeX manuscript for peer review.
* **Prerequisites**: Phase 27.
* **Deliverables**:
  1. Academic manuscript: `research/paper/sunborn_ieee.tex`.
  2. Reproducibility package: `research/reproduce.sh` executing deterministic evaluation on clean hardware.
* **Acceptance Criteria**:
  - Independent reproduction script produces statistically identical results matching paper tables.
* **Evidence**: Artifact evaluation package and LaTeX paper.
