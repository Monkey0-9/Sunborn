# SUNBORN — 4-Person Execution Model

# SUNBORN — 4-PERSON EXECUTION MODEL

## Team model

Four primary roles with shared review. No workstream is a silo.

### Member 1 — Platform / Backend Lead

Primary: Rust gateway, API contracts, orchestration, identity/auth, backend integration.
Secondary: cache engine, security, CI/CD.

### Member 2 — Distributed Systems / Data Lead

Primary: PostgreSQL, Dragonfly, Qdrant, knowledge storage, events/invalidation, performance/capacity.
Secondary: backend, SRE.

### Member 3 — AI / Cache Intelligence / Research Lead

Primary: canonicalization, embeddings, semantic equivalence, reranking, decision/risk, admission, execution reuse, experiments/statistics.
Secondary: backend, benchmarks, security evaluation.

### Member 4 — SRE / Security / ResearchOps Lead

Primary: CI/CD, supply chain, observability, security, tenancy validation, chaos, Kubernetes/DR, reproducibility/publication operations.
Secondary: data, research, release acceptance.

## Review matrix

No member reviews only their own work.

Backend → Distributed Systems + Security/SRE
Data/cache → Backend + Research
AI/research → Backend + ResearchOps/Security
Infrastructure/security → Backend + Distributed Systems
Publication → all four

Critical architecture/security/data-model changes require two reviewers.

## Workstream ownership

WS-00 Integration: M1 primary, M4 secondary
WS-01 Architecture: M1 primary, all reviewers
WS-02 DevEx/CI: M4 primary, M1 secondary
WS-03 Backend/API: M1 primary, M3 secondary
WS-04 Frontend: M1 primary, M4 secondary
WS-05 Data/Storage: M2 primary, M1 secondary
WS-06 Cache/Decision: M3 primary, M2 secondary
WS-07 Execution/Re-use: M3 primary, M1 secondary
WS-08 Knowledge/Freshness: M2 primary, M3 secondary
WS-09 QA/Security: M4 primary, M3 secondary
WS-10 SRE/Deployment/DR: M4 primary, M2 secondary
WS-11 Research/Benchmarking: M3 primary, M4 secondary
WS-12 Reproducibility/Publication: M4 primary, M3 secondary

## PR operating model

Branch format: <type>/<linear-id>-<short-description>
PR title: LINEAR-ID: concise change

Every PR must contain:

 1. Linear issue
 2. Problem
 3. Design/approach
 4. Changes
 5. Tests
 6. Benchmark impact
 7. Security impact
 8. Observability impact
 9. Migration/rollback impact
10. Evidence
11. Reviewer checklist

Merge rules:

* protected main
* required CI
* no direct pushes
* one conceptual change per PR
* squash merge
* critical paths require two reviewers
* failed benchmark/security gates block merge

## Execution cadence

Use two-week Linear cycles after all four members are added.

Day 1: planning + dependency verification
Days 2–8: implementation
Days 9–10: integration/testing
Day 11: benchmark/security/reliability evidence
Day 12: review/fixes
Day 13: integration
Day 14: cycle review

Split an issue when it cannot produce reviewable evidence in one cycle.

## Start gate

Before Phase 1 implementation:

* four members added
* GitHub connected
* CODEOWNERS installed
* PR template installed
* branch protection enabled
* CI skeleton green
* repository structure agreed
* Phase 0 architecture review approved

Phase 0 is NO-CODING except explicitly approved foundation/research/documentation tooling.

## Completion rule

A work item is Done only when implementation, tests, integration, required failure/security evidence, benchmark evidence, observability, documentation, review and reproducibility requirements are satisfied.
