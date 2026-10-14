# Sunborn Backend, API & Pipeline Architecture

# Backend & API Architecture

## Core runtime

Rust modular backend. Axum/HTTP boundary; internal interfaces are explicit and versioned. gRPC may be introduced only when justified by measured internal service boundaries.

## API domains

/auth
/tenants
/applications
/requests
/caches
/decisions
/executions
/knowledge
/invalidation
/models
/policies
/audit
/observability
/benchmarks
/experiments
/releases
/health

## Request pipeline

 1. Authenticate.
 2. Authorize.
 3. Resolve tenant and application.
 4. Validate payload.
 5. Apply security policy.
 6. Canonicalize.
 7. Build cache identity.
 8. Exact lookup.
 9. Semantic candidate retrieval.
10. Rerank.
11. Compute freshness/confidence/risk/policy.
12. Decide action.
13. Return cached result or execute origin.
14. Validate origin.
15. Admit artifacts.
16. Persist metadata/dependencies.
17. Emit events/telemetry.

## Internal interfaces

CacheStore
VectorStore
EmbeddingProvider
Reranker
ModelProvider
ToolProvider
KnowledgeStore
EventBus
PolicyEngine
DecisionEngine
ArtifactStore
AuditSink
TelemetrySink

## Failure rules

Bound every network call with timeout/cancellation.
Retries are selective and idempotent.
Circuit-break unreliable dependencies.
Never fail open across a security boundary.
Cache paths may fail to origin; origin remains authoritative.

## Testing

unit + property + integration + contract + E2E + failure injection + load + security regression.
