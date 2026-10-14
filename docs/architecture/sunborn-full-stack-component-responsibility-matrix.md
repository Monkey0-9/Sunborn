# Sunborn Full-Stack Component & Responsibility Matrix

# Full-Stack Responsibility Matrix

| Layer | Responsibility | Source of truth | Primary evidence |
| Frontend | Control/observability/admin/research UI | Backend APIs | E2E/accessibility |
| Gateway | Trust boundary and request enforcement | Auth/policy state | Security/integration |
| Canonical | Stable semantic request representation | Versioned schema | Property/regression |
| Identity | Deterministic cache identity | Version registry | Collision/version tests |
| Decision | Reuse policy | Policy/model/version state | Decision correctness |
| Exact cache | Fast exact reuse | PostgreSQL metadata + authoritative origin | Latency/failure |
| Semantic cache | Candidate retrieval | Qdrant derived index | Precision/recall |
| Execution cache | Structured artifact reuse | PostgreSQL/object metadata + origin | DAG correctness |
| Knowledge | Dependency/lineage | Source/document versions | Invalidation benchmark |
| Events | State-change transport | Outbox + authoritative data | Replay/idempotency |
| Freshness | Validity state | Versions/time/dependencies | Freshness tests |
| Observability | Telemetry | Telemetry systems | Trace/metric coverage |
| ResearchOps | Experiments/evidence | DVC/MLflow/object store | Reproduction |
| CI/CD | Quality/release gate | Git/GitHub + artifacts | Workflow runs |
| Deployment | Runtime orchestration | Kubernetes config | Rollout/rollback |
| Governance | Policy/audit/lifecycle | PostgreSQL + controlled artifacts | Audit evidence |

## Architectural invariant

No accelerator is a correctness source. PostgreSQL and authoritative origin systems remain the durable truth; caches, vector indexes and graphs are derived and rebuildable.
