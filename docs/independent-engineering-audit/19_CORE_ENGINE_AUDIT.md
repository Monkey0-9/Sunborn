# SUNBORN CORE ENGINE AUDIT

**Audit Standard**: IEEE 1012-2016 / ISO/IEC 25010  
**Scope**: Core execution pipelines, multi-tier orchestration, and state authority models  
**Auditor**: Independent Principal Core Systems Engineering Team  

---

## ⚡ Core Engine Architecture & Pipeline

The Sunborn Core Engine orchestrates the lifecycle of every request through 5 formal stages:

```
Ingress -> Canonicalization -> FinOps Gate -> Exact Tier (C0/C1) -> Semantic Cascade (C2)
  -> DAG Planner (C3) -> Origin Execution -> WAL Append -> RTC Invalidation -> Client Response
```

---

## 🏛️ The Four Equivalence Classes ($C_0$ to $C_3$)

Sunborn's core innovation rests on its rigorous mathematical formalization of execution candidate equivalence:

### Class $C_0$: Bitwise Exact Match
- **Definition**: Exact bit-for-bit equality of raw HTTP request bodies.
- **Lookup Cost**: $\mathcal{O}(1)$ in-memory hash table lookup ($<1.0\,\mu\text{s}$).
- **Admissibility**: Requires valid `ReuseCertificate` and fresh monotonic epoch lease.

### Class $C_1$: Syntactic & Canonical Equivalence
- **Definition**: Equivalence under RFC 8785 JSON Canonicalization Scheme (JCS) and Unicode NFC normalization:
  $$\text{Payload}_1 \equiv_{C_1} \text{Payload}_2 \iff \text{JCS}(\text{NFC}(\text{Payload}_1)) = \text{JCS}(\text{NFC}(\text{Payload}_2))$$
- **Stochastic Control**: If `temperature > 0.0` without a fixed `seed`, the candidate is marked non-deterministic and bypasses cache lookup.

### Class $C_2$: Semantic Equivalence under Guardrails
- **Definition**: Semantic similarity validated by a multi-stage verification cascade:
  1. Structural AST Guardrails ($<50\,\mu\text{s}$) — must clear negation, numeric, quantifier, and entity checks.
  2. Dense Vector Retrieval (Qdrant HNSW) filtered by `ContextDigest`.
  3. Non-Exchangeable Conformal Risk Control ($\alpha \le 0.001$).
  4. Wilson Score Interval verification against OOD concept drift.
  5. Causal Epoch Lease Freshness check.

### Class $C_3$: Deterministic DAG Subgraph Reuse
- **Definition**: Agent tool execution graphs decomposed into RFC 6962 Merkle trees. Deterministic tool results are replayed; mutating nodes (`is_mutating: true`) are never replayed and contaminate dependent subgraphs.

---

## 🛡️ Non-Accelerator Authority (NAA) Paradigm

Traditional LLM caches store state primarily in Redis or vector databases, risking unrecoverable state loss or silent drift when the cache restarts.

Sunborn enforces the **Non-Accelerator Authority (NAA)** paradigm:
- **PostgreSQL WAL is the sole source of truth**.
- DragonflyDB/Redis and Qdrant are treated as 100% disposable accelerators.
- In the event of a catastrophic accelerator crash, `apps/sunborn-rebuild` can replay the PostgreSQL WAL to rebuild the entire L1/L2 cache and Qdrant vector index with zero data loss and zero drift.
- **Verification**: Verified in `tests/disaster_recovery_test.rs`.
