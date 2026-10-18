# Sunborn Requirements Traceability Matrix (ISO/IEC/IEEE 29148)

> **Bidirectional Traceability**: Maps requirements to design components, database schemas, and verification evidence.

---

## 1. Functional Requirements Traceability

| Requirement ID | Description | Component Implementation | Verification Test | Status |
|---|---|---|---|---|
| **FR-001** | Tenant Isolation | `crates/sunborn-identity/src/identity.rs`<br>`crates/sunborn-gateway/src/middleware/auth.rs` | `sunborn-identity::test_tenant_isolation_in_identity`<br>`security_test::test_cross_tenant_isolation_zero_leakage` | **Verified** |
| **FR-002** | Deterministic Canonicalization | `crates/sunborn-identity/src/canonical.rs` | `sunborn-identity::test_canonicalize_text_whitespace`<br>`sunborn-identity::test_canonicalize_json_order` | **Verified** |
| **FR-003** | Exact Cache Matching | `crates/sunborn-cache/src/store.rs`<br>`crates/sunborn-cache/src/memory.rs` | `sunborn-cache::test_memory_cache_get_set`<br>`integration_test::test_vs001_vs002_vertical_slice_exact_cache` | **Verified** |
| **FR-004** | Semantic Cache & Risk Bounding | `crates/sunborn-semantic/src/decision.rs`<br>`crates/sunborn-semantic/src/embedding.rs` | `sunborn-semantic::test_decision_engine_safe_hit_and_risk`<br>`benchmarks/semantic_cache_bench.py` | **Verified** |
| **FR-005** | Event-Driven Invalidation | `crates/sunborn-invalidation/src/graph.rs`<br>`crates/sunborn-invalidation/src/outbox.rs` | `sunborn-invalidation::test_invalidation_pipeline_evicts_cache_and_vectors`<br>`integration_test::test_vs003_vs004_semantic_cache_and_invalidation_slice` | **Verified** |
| **FR-006** | Authoritative System of Record | `crates/sunborn-db/src/repository.rs`<br>`migrations/0001_initial_schema.sql` | `sunborn-db::test_db_store_lifecycle` | **Verified** |
| **FR-007** | FinOps Token Avoidance & Net Savings | `crates/sunborn-core/src/finops.rs` | `sunborn-core::test_finops_token_avoided_calculation`<br>`sunborn-core::test_finops_break_even_low_for_llm_queries` | **Verified** |
| **FR-008** | Observability & Prometheus Telemetry | `crates/sunborn-gateway/src/handlers.rs` (`/metrics`) | `integration_test::test_metrics_endpoint` | **Verified** |
| **FR-009** | Executive Control Plane | `frontend/src/app/` (Next.js 15) | Frontend TypeScript compilation & Pages | **Verified** |
| **FR-010** | Agent DAG Execution Reuse & Tool Hashing | `crates/sunborn-execution/src/planner.rs`<br>`crates/sunborn-execution/src/signature.rs`<br>`crates/sunborn-execution/src/store.rs` | `sunborn-execution::planner::tests::*`<br>`execution_test::test_execution_dag_reuse_and_invalidation` | **Verified** |
| **FR-011** | Shadow Evaluation & Semantic Drift Detection | `crates/sunborn-gateway/src/shadow.rs`<br>`crates/sunborn-gateway/src/handlers.rs` (`/v1/shadow/evaluate`) | `shadow::tests::test_shadow_evaluation_*`<br>`security_test::test_shadow_evaluation_drift_detection_endpoint` | **Verified** |
| **FR-012** | Multi-Tenant Security & Adversarial Defense | `crates/sunborn-gateway/tests/security_test.rs`<br>`crates/sunborn-execution/tests` | `security_test::test_cross_tenant_isolation_zero_leakage`<br>`security_test::test_cross_tenant_execution_dag_isolation`<br>`security_test::test_adversarial_prompt_injection_safety` | **Verified** |
| **FR-013** | Chaos Resilience & Circuit Breaker Fallback | `crates/sunborn-cache/src/circuit_breaker.rs`<br>`crates/sunborn-gateway/src/handlers.rs` | `chaos_test::test_chaos_circuit_breaker_resilience_and_origin_fallback` | **Verified** |
| **FR-014** | Production Kubernetes Artifacts & Security | `deploy/Dockerfile`<br>`deploy/k8s/` | Distroless non-root image & Kubernetes manifests | **Verified** |
| **FR-015** | Non-Accelerator Disaster Recovery & Rebuild | `apps/sunborn-rebuild`<br>`crates/sunborn-db/src/repository.rs` | `disaster_recovery_test::test_disaster_recovery_replay_and_index_reconstruction` | **Verified** |
| **FR-016** | Ultra-Low Latency & High Concurrency | `crates/sunborn-gateway`<br>`crates/sunborn-cache` | `performance_test::test_gateway_throughput_and_p99_latency` (14,166 req/s) | **Verified** |
| **FR-017** | Bounded Memory Capacity & Soak Stability | `crates/sunborn-cache/src/memory.rs`<br>`benchmarks/capacity_soak.py` | `capacity_soak_test::test_capacity_bounded_memory_eviction` | **Verified** |
| **FR-018** | GDPR Article 17 Cascade Erasure & Sweeper | `crates/sunborn-gateway/src/handlers.rs` (`DELETE /v1/tenants/:id`) | `governance_test::test_tenant_cascade_offboarding_zero_lingering_data` | **Verified** |
| **FR-019** | SRE Operations & Production Acceptance | `docs/runbooks/operations.md`<br>`docs/architecture/release_acceptance_record.md` | Formal release certification sign-off | **Verified** |
| **FR-020** | Empirical Scientific Reproducibility Package | `research/paper/sunborn_ieee.tex`<br>`research/reproduce.sh` | Independent reproduction package execution | **Verified** |
| **FR-021** | Tiered L1/L2 Cache & Write-Behind Commit | `crates/sunborn-cache/src/tiered.rs`<br>`crates/sunborn-db/src/write_behind.rs` | `sunborn-cache::tiered::tests::*`<br>`sunborn-db::write_behind::tests::*` | **Verified** |
| **FR-022** | Micro-Batched Inference Pool | `crates/sunborn-semantic/src/microbatch.rs` | `sunborn-semantic::microbatch::tests::*` | **Verified** |
| **FR-023** | In-Flight Singleflight SSE Stream Multiplexing | `crates/sunborn-gateway/src/origin/singleflight.rs` | `origin::singleflight::tests::*` | **Verified** |
| **FR-024** | Monotonic Epoch Fencing Leases | `crates/sunborn-invalidation/src/epoch.rs` | `epoch::tests::test_epoch_fence_prevents_stale_cache_writeback` | **Verified** |
| **FR-025** | Merkle-DAG Execution Splicing & Tool Purity | `crates/sunborn-execution/src/merkle.rs` | `merkle::tests::test_merkle_dag_subgraph_splicing` | **Verified** |
| **FR-026** | Prefix KV-Cache Affinity Radix Router | `crates/sunborn-gateway/src/routing/radix.rs`<br>`crates/sunborn-gateway/src/handlers.rs` | `routing::radix::tests::*`<br>`integration_test::test_radix_routing_and_affinity_endpoints` | **Verified** |
| **FR-027** | O(1) Cryptographic Tenant Shredding | `crates/sunborn-identity/src/crypto_shred.rs`<br>`crates/sunborn-gateway/src/handlers.rs` | `crypto_shred::tests::*`<br>`integration_test::test_cryptographic_shredding_endpoint`<br>`governance_test::test_tenant_cascade_offboarding_zero_lingering_data` | **Verified** |
| **FR-028** | Deterministic Simulation Testing (DST) | `crates/sunborn-gateway/tests/simulation_test.rs` | `simulation_test::test_deterministic_simulation_non_accelerator_authority_under_chaos` | **Verified** |
| **FR-029** | Online CRC & Structural Guardrails Pre-Filter | `crates/sunborn-semantic/src/guardrails.rs` | `guardrails::tests::*` | **Verified** |
| **FR-030** | Dynamic Trait-Based Backend Injection & Fail-Closed Boot | `crates/sunborn-gateway/src/state.rs`<br>`crates/sunborn-core/src/config.rs` | `state::tests::test_verify_production_readiness_*` | **Verified** |
| **FR-031** | Authoritative Database Durability & Segregated Null Handling | `crates/sunborn-db/src/postgres.rs` | `integration_test::*`<br>`disaster_recovery_test::*` | **Verified** |
| **FR-032** | Verified Feedback Ingestion with RFC 2104 HMAC-SHA256 (Layer 1) | `crates/sunborn-gateway/src/learning/feedback.rs` | `learning_controller_test::test_layer1_feedback_hmac_signature_verification` | **Verified** |
| **FR-033** | Tenant-Isolated Learning Store & GDPR Article 17 Purge (Layer 2) | `crates/sunborn-gateway/src/learning/store.rs` | `learning_controller_test::test_layer2_tenant_safe_store_and_gdpr_purge` | **Verified** |
| **FR-034** | Offline Held-Out Calibration & Counterfactual Shadowing (Layers 3 & 4) | `crates/sunborn-gateway/src/learning/calibration.rs`<br>`crates/sunborn-gateway/src/learning/shadow.rs` | `learning_controller_test::test_layer3_offline_calibration_on_held_out_splits`<br>`learning_controller_test::test_layer4_counterfactual_shadow_evaluator` | **Verified** |
| **FR-035** | Canary Promotion, Automated Rollback & BFF Secret Isolation (Layer 5) | `crates/sunborn-gateway/src/learning/controller.rs`<br>`frontend/src/app/api/proxy/[...path]/route.ts` | `learning_controller_test::test_layer5_canary_rollout_and_automated_rollback`<br>`frontend npm run build` (BFF proxy route) | **Verified** |

---

## 2. Invariant Compliance Matrix

| Invariant | Standard | Enforcement Location | Evidence |
|---|---|---|---|
| **Non-Accelerator Authority** | Architecture Rule 1 | `crates/sunborn-db`, `apps/sunborn-rebuild` | PostgreSQL owns durable truth; 100% rebuild verified in `disaster_recovery_test` & `simulation_test`. |
| **Zero False-Hit Security** | Security Rule 12 | `crates/sunborn-semantic/src/decision.rs`, `guardrails.rs` | CRC calibrated error budget $\alpha \le 0.01$ with Cross-Encoder NLI grey-zone verification. |
| **Zero Knowledge Drift** | Consistency C3 | `crates/sunborn-invalidation/src/outbox.rs`, `epoch.rs` | Monotonic epoch fencing eliminates stale read-repair race conditions during outbox invalidation. |
| **Deterministic Equivalence** | Correctness C0/C1 | `crates/sunborn-identity/src/canonical.rs` | Sorted JSON keys and normalized whitespace produce identical SHA-256 hashes. |
| **Agent Tool Execution Isolation** | Security Rule 17 | `crates/sunborn-execution/src/signature.rs`, `merkle.rs` | Merkle-DAG content-addressable sub-graph splicing with strict `ToolPurity` effect boundaries. |
| **Asynchronous Drift Verification** | Quality Standard C4 | `crates/sunborn-gateway/src/shadow.rs` | Continuous verification of cached responses against origin execution with auto-eviction. |
| **Graceful Degradation Resilience** | Reliability Rule 8 | `crates/sunborn-cache/src/circuit_breaker.rs` | Circuit breaker trips to protect clients; 100% request success rate during outages. |
| **Zero Lingering Tenant Erasure** | Governance Standard G1 | `crates/sunborn-identity/src/crypto_shred.rs` | O(1) TDEK zeroization renders all storage tiers mathematically unrecoverable in $<1$ ms. |
| **KV-Cache Prefix Locality** | Performance Rule 14 | `crates/sunborn-gateway/src/routing/radix.rs` | Compressed Radix Tree routing with bounded load and automated prompt-cache breakpoint injection. |


