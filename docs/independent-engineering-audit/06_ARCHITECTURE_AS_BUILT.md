# ARCHITECTURE AS-BUILT & SYSTEM TOPOLOGY

**Audit Standard**: IEEE 42010-2011 / ISO/IEC/IEEE 42010 (Architecture Description)  
**Target Repository**: `Sunborn (Rust Axum Workspace)`  
**Audit Entity**: Independent Principal Systems Architecture Review Board  
**Target Date**: 2026-11-13  

---

## 🏛️ Executive Architectural Summary

Sunborn is engineered as an **Axum-based asynchronous reverse-proxy execution gateway and multi-tier semantic cache** sitting between client applications (agents, SDKs, web dashboards) and upstream LLM providers (OpenAI, Anthropic, Bedrock, vLLM).

The system implements a **5-tier execution and storage hierarchy** governed by the **Non-Accelerator Authority (NAA)** principle:
1. **Tier-0 / Ingress**: Axum reverse-proxy with tower middleware, RBAC/ABAC authorization, and streaming singleflight multiplexing.
2. **Tier-1 / Exact Cache (C0/C1)**: In-process W-TinyLFU memory cache backed by distributed DragonflyDB/Redis for cluster-wide persistence.
3. **Tier-2 / Semantic Cascade (C2)**: Structural AST guardrails followed by Qdrant HNSW vector search, bounded by Conformal Risk Control ($\alpha \le 0.001$) and ContextDigest context-binding.
4. **Tier-3 / Execution DAG (C3)**: RFC 6962 Merkle tree execution planner memoizing deterministic tool subgraphs and preventing replay of mutating side-effects.
5. **Tier-4 / Causal Knowledge Invalidation (C4)**: SIMD 64-bit Roaring Bitmaps (`CompressedLineageGraph`) with monotonic epoch lease fencing.
6. **Tier-5 / Authoritative WAL Persistence**: PostgreSQL append-only write-ahead log with asynchronous group write-behind batching.

---

## 🏗️ Layer-by-Layer Architectural Census

### 1. Ingress & Canonical Identity Layer (`sunborn-identity`)
- **JSON Canonicalization Scheme (JCS)**: Adheres to RFC 8785. Strips extraneous whitespace, normalizes dictionary key ordering, and applies Unicode Normalization Form C (NFC).
- **Canonical Envelope Hashing**:
  $$\mathcal{H}(r) = \text{SHA256}(\text{TenantId} \parallel \text{AppId} \parallel \text{Model} \parallel \text{JCS}(\text{NFC}(r)) \parallel \text{Gate}(\text{temp}, \text{top\_p}, \text{seed}))$$
- **Stochastic Gate**: Prompts with $\text{temperature} > 0.0$ without a fixed deterministic `seed` bypass cache reuse unless specifically overridden by tenant policy.
- **Envelope Encryption**: Tenant payloads are encrypted at rest using AES-256-GCM with unique 96-bit random nonces. Tenant Data Encryption Keys (TDEKs) are wrapped under master keys managed via AWS KMS or HashiCorp Vault Transit.

### 2. Multi-Tier Cache Layer (`sunborn-cache`)
- **Memory Tier**: Concurrent `DashMap` wrapping an eviction policy engine with configurable capacity limits (e.g., 65,536 entries).
- **Greedy-Dual-Size-Frequency (GDSF) Eviction**: Cost-aware byte-bounded eviction prioritizing cached items with high avoidance value and small byte footprint.
- **Distributed Accelerator (DragonflyDB/Redis)**: Connected via connection managers with circuit-breaker protection (trips on consecutive timeouts, gracefully falls through to memory or origin).
- **ReuseCertificate Verification**: On exact cache read path, the gateway strictly verifies the cryptographic HMAC-SHA256 signature and multi-source monotonic epoch lease of the certificate before returning cached data.

### 3. Semantic Cascade Layer (`sunborn-semantic`)
- **Structural AST Guardrails**: Pre-filters queries in $<50\,\mu\text{s}$ before invoking vector search:
  - *Negation Inversion*: Detects polarity shifts (`securely store` vs `bypass security`).
  - *Numeric Transposition*: Extracts and compares sorted numeric sequences (`$1,000` to `$4,812` vs `$4,812` to `$1,000`).
  - *Quantifier Scope Shifts*: Detects quantifier alterations (`all`, `none`, `some`, `only`).
  - *Entity Transposition*: Compares capitalized proper nouns and entity markers.
- **Vector Search (Qdrant HNSW)**: Multi-tenant vector retrieval filtered by tenant namespace and 32-byte `ContextDigest` lineage hashes.
- **Conformal Risk Control (CRC)**: Non-exchangeable split-conformal prediction providing distribution-free guarantees on false-reuse error rates ($\alpha \le 0.001$) calibrated across task strata (Finance, Healthcare, Administration, Security).
- **Wilson Score Interval**: Computes analytical 95% confidence bounds on empirical coverage to detect out-of-distribution (OOD) query drift.

### 4. Agent Execution DAG Memoization (`sunborn-execution`)
- **Merkle DAG Planner**: Decomposes agent multi-step execution graphs into RFC 6962 Merkle trees.
- **Transitive Lineage & Side-Effect Contamination**: Nodes marked with mutating side-effects (`is_mutating: true`) are never reused and contaminate all downstream dependent subgraphs.
- **TTL Envelope Inheritance**: Child nodes inherit the minimum TTL of all upstream input dependencies.

### 5. Causal Lineage & Invalidation Engine (`sunborn-invalidation`)
- **Compressed Roaring Bitmaps**: SIMD-accelerated 64-bit Roaring Treemaps (`CompressedLineageGraph`) mapping document hashes to sets of dependent execution record IDs. Achieves 1.8 bytes per edge.
- **Real-Time Causal (RTC) Invalidation**: Evaluates transitive closures over 352,000+ edges in $<50\,\mu\text{s}$, purging downstream cache entries across exact, semantic, and vector tiers.
- **Atomic Two-Phase Admission**: `GraphMemoryCache::set_with_lineage` registers document dependencies before committing cache entries, with automatic compensation rollback on partial failures.
- **Monotonic Epoch Fencing**: Each document is governed by a monotonically increasing epoch version. Writebacks from concurrent requests are rejected if the document epoch advanced while the LLM call was in flight.

### 6. Persistence & Non-Accelerator Authority (`sunborn-db`)
- **System of Record**: PostgreSQL WAL is the sole authoritative system of record. Redis/Dragonfly and Qdrant are treated as ephemeral accelerators.
- **Group Write-Behind Buffer**: Asynchronous write-behind worker coalesces execution traces into microbatches, maintaining ordered WAL hydration.
- **Disaster Recovery**: `apps/sunborn-rebuild` allows full state reconstruction of in-memory and accelerator indexes directly from PostgreSQL tables (`execution_records`, `tenants`, `knowledge_documents`).

---

## 🧩 Architectural Coupling & Boundary Audit

| Boundary | Observed Implementation | Coupling Level | Architectural Verdict |
| :--- | :--- | :---: | :--- |
| **Gateway -> Cache** | Abstracted via `CacheStore` trait (`MemoryCache`, `DragonflyCache`, `TieredCache`) | Low (Polymorphic) | ✅ Excellent isolation |
| **Gateway -> Vector Store** | Abstracted via `VectorStore` trait (`QdrantStore`, `MockVectorStore`) | Low (Pluggable) | ✅ Clean abstraction |
| **Gateway -> DB WAL** | Direct asynchronous channel to `WriteBehindBuffer` backed by `PostgresStore` | Low (Non-blocking) | ✅ Protects request path |
| **Gateway -> KMS** | Dynamic provider selection (`AwsKmsProvider`, `VaultTransitProvider`, `LocalAeadProvider`) | Low (Fail-closed) | ✅ Enforces key separation |
| **Frontend -> Gateway** | Next.js 15 Server-Side BFF (`/api/proxy/[...path]`) | Controlled BFF | ✅ Protects backend topology |
