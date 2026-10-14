# Sunborn — Complete System Pipelines

## 1. Request / response pipeline

Client → Edge → AuthN → AuthZ → Tenant resolution → Input validation → Security screening → Canonicalization → Cache identity → Exact cache → Semantic candidate retrieval → Metadata filtering → Reranking → Confidence/risk/freshness/policy evaluation → Action.

Actions:

* SAFE_HIT → return cached response
* REVALIDATE → validate affected state then return/recompute
* PARTIAL_REUSE → execute only invalid execution nodes
* ORIGIN_EXECUTION → full RAG/LLM/tool path
* REJECT → security/policy denial

## 2. Origin execution pipeline

Request → Retrieval plan → Retrieval → Evidence validation → Tool calls (authorized) → Transformations → Structured execution artifact graph → LLM synthesis → Output validation → Provenance capture → Cache admission.

## 3. Cache admission pipeline

Origin result → correctness check → security classification → freshness assessment → dependency capture → reuse-value estimate → policy → CACHE / SHORT_TTL / ISOLATED_CACHE / REVALIDATE_ONLY / DO_NOT_CACHE.

## 4. Knowledge update pipeline

Source change → ingestion/versioning → source event → durable event log → idempotent consumer → dependency resolution → affected cache set → invalidate → revalidate/recompute → reconciliation → telemetry.

## 5. Model/tool/policy change pipeline

Version registry update → compatibility evaluation → impacted identities → selective invalidation or compatibility transition → revalidation → cache warm/rebuild where safe.

## 6. Shadow evaluation pipeline

Sample safe hit → asynchronous fresh execution → compare cached/fresh quality, retrieval, factual/semantic agreement, citations, safety and freshness → drift score → policy response.

## 7. Security pipeline

Request/document/event → threat detection → classify → allow/deny/quarantine → audit → if vulnerability is found: attack fixture → mitigation → regression test → retest → close.

## 8. Frontend control pipeline

Browser → authentication/session → authorized API → backend read model → sanitized/paginated data → visualization.
Administrative mutation:
UI → confirmation → authorized backend API → transaction/event → audit → refreshed read model.

## 9. Research pipeline

Research question → literature → gap → hypothesis → experiment manifest → DVC dataset → code commit → model/config/version capture → MLflow run → raw output → statistical analysis → result classification → ADR/implementation decision → paper evidence.

## 10. CI pipeline

Commit → format → lint → unit → property → integration → contract → frontend type/build/test → SAST/CodeQL → dependency/secret scan → container build → SBOM → image scan → benchmark smoke → artifact publication → PR review.

## 11. CD pipeline

Merged main → build immutable version → staging deploy → health checks → migration compatibility → integration suite → performance/security gate → production-like acceptance → canary/rolling → monitor → promote or rollback.

## 12. Backup/DR pipeline

Scheduled backup → integrity verification → retention → restore rehearsal → authoritative-state recovery → event replay → derived-store rebuild → consistency verification → service restoration.

## 13. Data ownership

PostgreSQL = source of truth.
Dragonfly/Qdrant/Neo4j = derived/accelerator state.
Redpanda = event history/replay transport.
Object storage = large immutable artifacts.
Observability stores = telemetry, not business truth.

## 14. Global failure principle

Any accelerator failure must fall back to a safe authoritative path unless the requested action itself is a security/correctness violation, in which case fail closed.

## 15. Global evidence principle

Every pipeline has owner, test suite, telemetry, failure behavior, runbook and Linear issue lineage.
