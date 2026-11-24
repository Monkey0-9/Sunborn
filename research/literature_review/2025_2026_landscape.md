# 2025–2026 AI Execution Reuse & Semantic Caching Landscape

This document establishes the scientific baseline for Sunborn's research contributions, explicitly identifying prior art, recent 2025–2026 breakthroughs, and the unaddressed research gaps Sunborn targets.

---

## 1. Classical Systems & Industrial Solutions

| Technology | Approach & Architecture | Core Limitations & Gaps |
| :--- | :--- | :--- |
| **Redis Semantic Cache** | Vector-based similarity search (RediSearch/HNSW). Exact & approximate embedding matches. | Similarity does not equal correctness. No context-binding, zero cryptographic attestation, coarse TTL invalidation. |
| **GPTCache** | Embedding pre-filters, configurable vector stores (Milvus, FAISS), rankers. | Client-side/local library; lacks distributed consistency, tenant-isolation verification, and transitive agent DAG support. |
| **OpenTelemetry (OTel)** | Distributed tracing specifications for spans, metrics, and GenAI semantic conventions. | Observability format only; does not provide execution-reuse decision planes or proof verification. |

---

## 2. Recent Academic Research (2025–2026)

### 2.1 KV-Cache Serving & GPU-Level Optimizations
* **CacheBlend (EuroSys 2025)**: Reuses precomputed KV caches for non-prefix RAG chunks; selectively recomputes affected tokens (reported 2.2–3.3× TTFT reduction).
  - *Distinction from Sunborn*: CacheBlend operates at the GPU KV-tensor layer inside the serving engine. Sunborn operates as an application-level control plane governing execution reuse, external tool DAGs, and multi-tenant authorization.
* **Mooncake (USENIX FAST 2025)**: KVIter distributed disaggregated KV-cache cluster architecture.
  - *Distinction from Sunborn*: Focuses on raw I/O transfer speed across disaggregated GPU memory pools, not semantic equivalence or causal dependency verification.

### 2.2 Semantic Correctness & Calibration
* **Closing the Operational Gap in Semantic Caching (EMNLP 2026)**: Introduces P-CHR AUC and Operational Retention Rate to evaluate deployment-aware degradation.
  - *Sunborn Adoption*: Sunborn adopts Operational Retention Rate and strict held-out splits to prevent calibration leakage.
* **RACE (IEEE 2026)**: Online semantic risk budgeting and calibration via conformal prediction.
  - *Distinction from Sunborn*: RACE addresses threshold tuning in an isolated mathematical model. Sunborn extends this to verifiable distributed certificates and multi-tenant execution policies.
* **FreshCache (2026)**: Risk-constrained freshness-aware semantic caching estimating staleness risk.
  - *Sunborn Extension*: FreshCache relies on probabilistic staleness estimation. Sunborn implements *causal dependency invalidation* with a 5-dimensional deterministic predicate ($D_t, P_t, A_t, M_t, E_t$).
* **DKC-LLM (IEEE 2026)**: Dynamic knowledge semantic cache and drift compensation.
  - *Sunborn Extension*: Sunborn formalizes drift detection via Total Variation Distance over semantic categories and couples it with safety-gated shadow replay.

---

## 3. The Unaddressed Industry Gaps Sunborn Solves

1. **Similarity $\ne$ Correctness**: Existing systems assume high cosine similarity implies safe reuse. Negation ("not"), numeric changes ("2025" vs "2026"), and polarity inversion cause catastrophic false hits without structural guardrails.
2. **TTL Inadequacy**: Expiration by seconds fails when authoritative documents, permissions, or system prompts change instantaneously.
3. **Agent Workflow Memoization**: Current caches only store final query-response pairs, repeating expensive multi-step tool workflows (read, search, compute, transact).
4. **Lack of Verifiable Accountability**: Cache hits are served on unverified assertions of cache servers without tamper-evident, independent client verification.
