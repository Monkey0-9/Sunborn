# Initial Baseline Autopsy (5.2 / 10.0 Prototype State)

**Assessment Date:** 2026-11-13  
**Auditor:** Independent Principal Systems Architecture Audit Team  
**Evaluation Standard:** Tier-1 Big Tech Production Verification  

---

## 1. Baseline Summary

The pre-transformation Sunborn repository represented a **hardened functional prototype (5.2/10)**. While it featured impressive low-level micro-benchmarks in isolated in-memory modules (e.g. L1 GDSF cache achieving ~1M ops/s), it suffered from critical architectural gaps when evaluated as an enterprise-grade, distributed, sovereign AI gateway:

```
+----------------------------------------------------------------------------------------------------+
|                                    HISTORICAL 5.2/10 BASELINE BREAKDOWN                            |
+----+--------------------------------+-------+------------------------------------------------------+
| No | Dimension                      | Score | Baseline Vulnerabilities & Deficiencies              |
+----+--------------------------------+-------+------------------------------------------------------+
| 1  | Multi-Replica Consistency      | 3.5   | InMemoryClusterTransport hardcoded; no pod sync      |
| 2  | Invalidation Safety            | 4.5   | Independent deletes; silent error swallows (let _ =) |
| 3  | Cross-Encoder / NLI Claims     | 2.0   | Claimed 300M model; was actually regex heuristics    |
| 4  | Database Migration Authority   | 5.0   | Split authority: raw SQL string vs migrations/ files |
| 5  | Persistent Monotonic Epochs    | 4.0   | Stored in volatile RAM; lost on process restart      |
| 6  | Outbox Background Execution    | 4.5   | Outbox records inserted but never flushed by worker  |
| 7  | Frontend Truth & Verification  | 4.0   | Mock standalone-simulator generating fake charts     |
| 8  | Cryptographic Domain Separation| 6.0   | Merkle tree lacked RFC 6962 interior node prefixes   |
| 9  | Conformal Prediction Rigor     | 5.0   | Calibrator was dead code; unstratified static gate   |
| 10 | Accessibility & WCAG 2.2 AA    | 3.0   | Color contrast failures, missing ARIA landmark roles |
+----+--------------------------------+-------+------------------------------------------------------+
```

---

## 2. Detailed Root-Cause Investigations

### 2.1 The Distributed Split-Brain Vulnerability
In `crates/sunborn-gateway/src/state.rs`, the cluster synchronization engine was instantiated as:
```rust
// HISTORICAL CODE (VULNERABLE)
let cluster_transport = Arc::new(InMemoryClusterTransport::new(pod_id));
```
Because every gateway container instance ran its own isolated in-memory message bus, invalidation events published by Pod A were never delivered to Pod B. In a multi-replica Kubernetes deployment, cached prompt responses on Pod B remained active indefinitely after source data mutation, violating strict monotonic freshness.

### 2.2 Silent Deletion Drops in Cache Invalidation
In `crates/sunborn-gateway/src/handlers.rs`:
```rust
// HISTORICAL CODE (VULNERABLE)
let _ = state.vector_store.delete_point(&point_id).await;
let _ = state.dependency_graph.remove_node(&node_id).await;
```
If Qdrant or Redis timed out, the errors were discarded silently with `let _ = `. The caller received a `200 OK` response while stale vector embeddings remained indexed, leading to high-risk semantic cache hits on invalidated context.

### 2.3 The "Vibe-Code" Neural Cross-Encoder Claim
The documentation claimed Sunborn deployed a *"300M parameter neural cross-encoder NLI model with sub-millisecond inference"*. Source code inspection of `crates/sunborn-semantic/src/guardrails.rs` revealed that the NLI component was actually a collection of regex string matching rules with zero machine learning inference. This presented a severe compliance and truth-in-engineering breach.
