# Sunborn Requirements Traceability Matrix (ISO/IEC/IEEE 29148)

> **Bidirectional Traceability**: Maps requirements to design components, database schemas, and verification evidence.

---

## 1. Functional Requirements Traceability

| Requirement ID | Description | Component Implementation | Verification Test | Status |
|---|---|---|---|---|
| **FR-001** | Tenant Isolation | `crates/sunborn-identity/src/identity.rs`<br>`crates/sunborn-gateway/src/middleware/auth.rs` | `sunborn-identity::test_tenant_isolation_in_identity`<br>`sunborn-semantic::test_vector_search_with_tenant_isolation` | **Verified** |
| **FR-002** | Deterministic Canonicalization | `crates/sunborn-identity/src/canonical.rs` | `sunborn-identity::test_canonicalize_text_whitespace`<br>`sunborn-identity::test_canonicalize_json_order` | **Verified** |
| **FR-003** | Exact Cache Matching | `crates/sunborn-cache/src/store.rs`<br>`crates/sunborn-cache/src/memory.rs` | `sunborn-cache::test_memory_cache_get_set`<br>`integration_test::test_vs001_vs002_vertical_slice_exact_cache` | **Verified** |
| **FR-004** | Semantic Cache & Risk Bounding | `crates/sunborn-semantic/src/decision.rs`<br>`crates/sunborn-semantic/src/embedding.rs` | `sunborn-semantic::test_decision_engine_safe_hit_and_risk`<br>`benchmarks/semantic_cache_bench.py` | **Verified** |
| **FR-005** | Event-Driven Invalidation | `crates/sunborn-invalidation/src/graph.rs`<br>`crates/sunborn-invalidation/src/outbox.rs` | `sunborn-invalidation::test_invalidation_pipeline_evicts_cache_and_vectors`<br>`integration_test::test_vs003_vs004_semantic_cache_and_invalidation_slice` | **Verified** |
| **FR-006** | Authoritative System of Record | `crates/sunborn-db/src/repository.rs`<br>`migrations/0001_initial_schema.sql` | `sunborn-db::test_db_store_lifecycle` | **Verified** |
| **FR-007** | FinOps Token Avoidance & Net Savings | `crates/sunborn-core/src/finops.rs` | `sunborn-core::test_finops_token_avoided_calculation`<br>`sunborn-core::test_finops_break_even_low_for_llm_queries` | **Verified** |
| **FR-008** | Observability & Prometheus Telemetry | `crates/sunborn-gateway/src/handlers.rs` (`/metrics`) | `integration_test::test_metrics_endpoint` | **Verified** |
| **FR-009** | Executive Control Plane | `frontend/src/app/` (Next.js 15) | Frontend TypeScript compilation (`npm run build`) | **Verified** |

---

## 2. Invariant Compliance Matrix

| Invariant | Standard | Enforcement Location | Evidence |
|---|---|---|---|
| **Non-Accelerator Authority** | Architecture Rule 1 | `crates/sunborn-db`, `migrations/0001_initial_schema.sql` | PostgreSQL owns durable truth; caches derived. |
| **Zero False-Hit Security** | Security Rule 12 | `crates/sunborn-semantic/src/decision.rs` | Risk score > 0.25 forces revalidation; 0% false hits on benchmark. |
| **Zero Knowledge Drift** | Consistency C3 | `crates/sunborn-invalidation/src/outbox.rs` | Automatic eviction of all dependent exact and vector keys upon document update. |
| **Deterministic Equivalence** | Correctness C0/C1 | `crates/sunborn-identity/src/canonical.rs` | Sorted JSON keys and normalized whitespace produce identical SHA-256 hashes. |
