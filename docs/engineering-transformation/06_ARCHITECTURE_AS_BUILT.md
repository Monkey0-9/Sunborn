# Architecture As-Built & System Topology

**Gateway Architecture:** Multi-Tier Sovereign AI Acceleration Gateway  
**Primary Language:** Rust (Tokio 1.x, Axum 0.8)  
**Database Authority:** PostgreSQL 16 (`_sunborn_migrations` tracking)  
**Vector Database:** Qdrant Distributed HNSW Engine  
**Lineage Tracking:** 64-bit SIMD Roaring Bitmap Bitsets  

---

## 1. System Component Topology

```mermaid
flowchart TD
    Client(["Enterprise Client / AI Agent"]) -->|HTTP/2 / TLS 1.3| Ingress["Axum Gateway Ingress (Port 8080)"]

    subgraph GatewayStack["Sunborn Sovereign Gateway Engine"]
        Ingress --> AuthMW["Auth & Dual-Admin Quorum Middleware"]
        AuthMW --> FinOps["FinOps Velocity & Spend Controller"]
        FinOps --> Canon["RFC 8785 Canonicalizer & Context Digest"]
        
        Canon --> ExactL1{"Tier-1 L1 Exact Cache<br/>(GDSF In-Memory)"}
        ExactL1 -->|Cache Hit<br/>P99 1.7µs| RetL1["Return Cached Response + Ed25519 Cert"]
        
        ExactL1 -->|Cache Miss| SemGate{"Tier-2 Semantic Pipeline<br/>(Structural AST + Conformal)"}
        
        SemGate -->|AST Invariant Pass &<br/>Conformal Risk &le; 0.001| RetSem["Return Semantic Reuse Response"]
        
        SemGate -->|Abstain / Miss| RadixRouter["Radix KV & Provider Arbitrage Router"]
    end

    subgraph StorageLayer["Durable Multi-Tier Storage"]
        RadixRouter -->|Forward Request| Provider["LLM Providers (OpenAI, Anthropic, Gemini, Grok)"]
        Provider --> CacheWrite["Cache Admission Engine"]
        
        CacheWrite --> ExactL1
        CacheWrite -->|Async Set| Dragonfly["L2 Dragonfly / Redis Cache"]
        CacheWrite -->|Deterministic UUID5| Qdrant[("Qdrant Vector DB")]
        CacheWrite -->|Append Ledger| Postgres[("PostgreSQL 16 Store")]
        
        Postgres --> OutboxTable["Transactional Outbox Table"]
        OutboxTable --> FlusherWorker["Background Outbox Flusher"]
    end

    subgraph InvalidationBus["Distributed Invalidation Topology"]
        MutateReq["Source Data Mutation"] --> Orchestrator["InvalidationOrchestrator"]
        Orchestrator -->|Phase 1: Local Purge| ExactL1
        Orchestrator -->|Phase 1: Vector Purge| Qdrant
        Orchestrator -->|Phase 1: SIMD Union| RoaringLineage["Roaring Bitmap Lineage"]
        Orchestrator -->|Phase 2: Redis Stream| RedisPubSub["Cluster Transport Bus"]
        RedisPubSub --> ReplicaPod["Replica Pod B Listener"]
        ReplicaPod --> EvictL1["Evict L1 Cache Entry"]
    end
```

---

## 2. Invalidation Orchestrator Execution Flow

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Tenant Admin
    participant Orch as InvalidationOrchestrator
    participant L1 as Exact Cache (L1)
    participant Qdr as Qdrant Vector Store
    participant Graph as Dependency Graph / Lineage
    participant Bus as Cluster Sync Bus (Redis Streams)

    Admin->>Orch: purge_sources(tenant_id, [source_ids])
    Note over Orch: Increment Monotonic Epoch & Record Watermark
    
    rect rgb(240, 248, 255)
    Note over Orch,Graph: Phase 1: Local Multi-Store Purge
    Orch->>Graph: resolve_transitive_dependents(source_ids)
    Graph-->>Orch: return dependent_keys
    
    Orch->>L1: delete_by_prefix / keys(dependent_keys)
    L1-->>Orch: exact_purged_count
    
    Orch->>Qdr: delete_points_by_uuid5(dependent_keys)
    Qdr-->>Orch: vector_purged_count
    
    alt If any store deletion fails
        Orch->>Graph: PRESERVE dependency edges (no silent drops)
        Orch-->>Admin: PurgeReport(status: PartialFailure, failed_keys)
    else All store deletions succeed
        Orch->>Graph: remove_source_edges(source_ids)
    end
    end

    rect rgb(255, 250, 240)
    Note over Orch,Bus: Phase 2: Distributed Cluster Broadcast
    Orch->>Bus: publish_invalidation(tenant_id, source_ids, epoch)
    Bus-->>Replica: Deliver Cluster Message
    Replica->>Replica: Evict Local Memory Cache & Advance Epoch
    end

    Orch-->>Admin: PurgeReport(status: Success, records_purged, duration_us)
```
