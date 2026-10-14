# Sunborn 30-Cycle Execution Roadmap

# 30-Cycle Execution Roadmap

Each cycle is two weeks. Parallel tracks are BUILD, RESEARCH, VALIDATION and DOCUMENTATION.

## C0–C2 Foundation

C0: Linear/GitHub/PR rules, labels, repo skeleton, environments.
C1: requirements, NFR/SLO, architecture, threat/failure model, ADR register.
C2: Rust/Next.js/Python foundations, local stack, CI, benchmark and research harness.

## C3–C6 Trusted request path

C3: gateway API/auth/tenant.
C4: authorization/limits/security middleware/integration tests.
C5: canonical request + deterministic normalization/serialization.
C6: semantic equivalence interface, fingerprinting and adversarial canonical suite.

## C7–C10 Data/cache fabric

C7: PostgreSQL control plane + cache identity.
C8: Dragonfly exact cache.
C9: Qdrant + embeddings.
C10: filtering + reranking + semantic cache.

## C11–C13 Decision/reuse

C11: decision inputs, confidence, risk and policy.
C12: admission + explainability.
C13: execution DAG + structured artifacts + partial reuse.

## C14–C16 Knowledge correctness

C14: source/document versions, dependencies, lineage.
C15: outbox + Redpanda events + invalidation.
C16: freshness state machine + revalidation + shadow evaluation.

## C17–C20 Enterprise hardening

C17: security/tenant isolation/poisoning.
C18: observability/FinOps.
C19: chaos/failure matrix.
C20: Kubernetes/Helm/rollback/DR.

## C21–C25 Scientific validation

C21: SemanticCacheBench + B0–B6 baselines.
C22: adaptive threshold/confidence/risk experiments.
C23: knowledge-aware/temporal/execution experiments.
C24: security/poisoning/tenant experiments.
C25: scale, capacity, ablation and statistics.

## C26–C29 Finalization

C26: reproducibility package + clean-machine reproduction.
C27: production acceptance and final security/DR/benchmark evidence.
C28: paper + claim/evidence audit.
C29: release artifact + benchmark publication + final retrospective.

## Cycle exit

No cycle closes with hidden work. Carryover is explicit in Linear. Every milestone exit requires evidence.
