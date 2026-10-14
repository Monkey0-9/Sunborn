# ADR-001: Sunborn Architecture Baseline and Invariant Freeze

## Status
Accepted (Phase 00 Freeze)

## Context
Sunborn is an enterprise-grade semantic caching and execution-reuse platform for LLMs, RAG, and agentic workflows. To prevent consistency hazards, data loss, and architectural drift, the foundational invariants and technology selections must be frozen prior to full execution.

## Invariant 1: Non-Accelerator Authority
- **PostgreSQL** is the sole authoritative system of record for all persistent control-plane state, tenants, policies, metadata, and audit records.
- **Dragonfly**, **Qdrant**, and **Neo4j** are strictly derived accelerators. If an accelerator fails or becomes desynchronized, the authoritative origin remains uncorrupted and rebuildable.

## Invariant 2: Multi-Tier Cache Equivalence
- **C0**: Strict exact canonical match.
- **C1**: Deterministic structural equivalence (whitespace, sorted parameters, typed envelopes).
- **C2**: Semantic embedding candidate retrieval bounded by confidence and risk models.
- **C3**: Freshness and knowledge-invalidation state-machine validation.
- **C4**: Shadow execution and drift scoring.
- **C5**: Degraded fallback.

## Invariant 3: Tenant Isolation Boundary
All cache identities, storage keys, and retrieval partitions MUST contain tenant and application scopes (`sunborn:{tenant_id}:{application_id}:...`). Cross-tenant pollution is strictly prohibited.

## Decision
1. **Core Runtime**: Rust with Axum HTTP framework for modularity, low-latency execution, and deterministic memory safety.
2. **Exact Cache**: Dragonfly / Redis-compatible memory store with configurable TTLs.
3. **Semantic Index**: Qdrant vector index for candidate retrieval.
4. **Event Backbone**: Redpanda/Kafka event log for decoupled cache invalidation and lineage tracking.
5. **Control Plane**: Next.js App Router with TypeScript.
