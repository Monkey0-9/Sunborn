# Sunborn System Architecture & Subsystem Topologies

This document provides definitive Mermaid architectural diagrams depicting Sunborn's end-to-end data pipelines, cryptographic trust boundaries, and execution topologies.

---

## 1. End-to-End Ingress & Multi-Tier Resolution Topology

```mermaid
sequenceDiagram
    autonumber
    actor Client as Agent SDK / Client
    participant BFF as Next.js BFF Proxy
    participant GW as Axum Gateway
    participant Guard as Tier C1.5 Guardrails
    participant L1 as L1 FinOps GDSF Cache
    participant L2 as L2 Redis/Dragonfly
    participant L3 as L3 Semantic Conformal
    participant Router as Radix KV Router
    participant LLM as Upstream LLM (OpenAI/Anthropic)
    participant Outbox as Postgres Outbox & DLQ

    Client->>BFF: POST /v1/chat/completions (Bearer Key)
    BFF->>GW: Proxy Request (CSRF & Rate Limit Checked)
    GW->>GW: Atomic Pre-Flight Budget Reservation
    GW->>Guard: Extract StructuralFingerprint(Prompt)
    
    alt Structural Mismatch (Negation/Number Inversion)
        Guard-->>GW: Polarity / Numeric Mismatch Flag
        GW->>Router: Route Direct to Origin (Bypass Vector Cache)
    else Structural Fingerprint Matches
        GW->>L1: Lookup Exact Canonical Hash
        alt L1 Hit
            L1-->>GW: Return Cached AEAD Payload (0.2 ms)
        else L1 Miss
            GW->>L2: Lookup Distributed Store
            alt L2 Hit
                L2-->>GW: Return Cached Payload (1.5 ms)
            else L2 Miss
                GW->>L3: Search Vector Store & Compute Conformal Non-Conformity
                alt Non-Conformity <= Calibrated Bound
                    L3-->>GW: Return Semantic Hit & Verify Proof-Carrying Token (4.2 ms)
                else Risk Exceeds Bound / Miss
                    L3-->>GW: Abstain (Fail Safe to Upstream)
                end
            end
        end
    end

    alt Cache Hit Resolved
        GW->>GW: Decrypt Payload in Memory via Enveloped TDEK
        GW-->>BFF: Return Response with ReuseCertificate
        BFF-->>Client: 200 OK (Cache Hit)
    else Cache Miss / Abstention
        GW->>Router: Match Prompt Prefix for KV Cache Affinity
        Router->>LLM: Dispatch Query to Optimal LLM Instance
        LLM-->>GW: Stream / Return Fresh Completion
        GW->>Outbox: Record FinOps Ledger & Outbox Mutation
        GW->>L1: Populate GDSF Cache & Vector Store
        GW-->>BFF: 200 OK (Fresh Generation)
        BFF-->>Client: 200 OK
    end
```

---

## 2. Merkle-DAG Spliced Execution Engine

```mermaid
flowchart TD
    subgraph Input Agent DAG
        N1[Node 1: Parse Input<br/>PureDeterministic] --> N2[Node 2: Query DB<br/>ReadIdempotent TTL=300s]
        N2 --> N3[Node 3: Transfer Funds<br/>SideEffectingMutator]
        N3 --> N4[Node 4: Format Email<br/>PureDeterministic]
    end

    subgraph Topological Classification & Splicing
        P1[Prefix Splicer] -->|Evaluate Ancestors| Classify{Side-Effect Mutator in Ancestry?}
        Classify -- No --> CachePrefix[Prefix Eligible for Hydration]
        Classify -- Yes --> ExecTail[Tail Must Re-Execute]
    end

    subgraph Spliced Execution Plan
        CachePrefix --> H1[Hydrate Node 1 from Merkle Cache]
        CachePrefix --> H2[Hydrate Node 2 if Lease Valid]
        ExecTail --> E3[Execute Node 3 against Live Upstream]
        ExecTail --> E4[Execute Node 4 with Fresh State]
    end

    subgraph Cryptographic Attestation
        H1 --> Proof[Domain-Separated Merkle Inclusion Proof<br/>0x00 Leaf / 0x01 Branch]
        H2 --> Proof
        Proof --> AuditToken[Machine-Verifiable Attestation Certificate]
    end
```

---

## 3. Data Sovereignty & Cryptographic Shredding State Machine

```mermaid
stateDiagram-v2
    [*] --> Provisioned: POST /v1/governance/provision
    Provisioned --> Active: Generate AES-256-GCM TDEK wrapped under Master KEK (AWS KMS / Vault)
    
    state Active {
        [*] --> Ingesting
        Ingesting --> EncryptingAtRest: Envelope Encrypt Payloads with TDEK
        EncryptingAtRest --> ServingHits: Memory Decrypt on Read Path
        ServingHits --> Ingesting
    }

    Active --> RevokedCredentials: POST /v1/auth/credentials/revoke
    RevokedCredentials --> Active: Rotate Credentials (Historical Data Preserved)

    Active --> CryptoShredding: POST /v1/governance/shred (GDPR Article 17)
    
    state CryptoShredding {
        ZeroizeMemory: Memory Zeroization (zeroize::Zeroize on in-flight keys)
        DeleteTDEK: DELETE FROM tenant_wrapped_keys in PostgreSQL
        PurgeL1L2: Prefix Purge L1 memory and L2 Redis
        DropVectors: Delete tenant vector namespace
        
        ZeroizeMemory --> DeleteTDEK
        DeleteTDEK --> PurgeL1L2
        PurgeL1L2 --> DropVectors
    }

    CryptoShredding --> IrreversiblyShredded: O(1) Ciphertext Cryptographically Inaccessible
    IrreversiblyShredded --> [*]
```

---

## 4. Transitive Invalidation Topology with Outbox & DLQ

```mermaid
flowchart LR
    subgraph Knowledge Mutation
        DocUpdate[Document / Chunk Mutation] --> OutboxEvent[Postgres outbox_events Record]
    end

    subgraph Invalidation Pipeline
        OutboxEvent --> Engine[InvalidationEngine]
        Engine --> Channel{Bounded Channel<br/>Capacity=1024}
        Channel -- Normal --> Worker[Outbox Worker]
        Channel -- Saturated --> Backpressure[10ms Backpressure Wait / Reserve]
        Backpressure -- Timeout --> Reject[Fail Closed / Log Alarm]
    end

    subgraph Dependency Resolution
        Worker --> Lineage[CompressedLineageGraph<br/>Roaring Treemaps]
        Lineage --> SIMD[SIMD Bitwise OR Union<br/>O(k) AVX2 Operation]
        SIMD --> EvictList[100,000+ Affected Cache Keys]
    end

    subgraph Multi-Store Eviction
        EvictList --> PurgeCache[Purge L1 GDSF & L2 Redis]
        EvictList --> PurgeVector[Delete Vector Embeddings]
        Worker -- Persistent Failure --> DLQ[Dead Letter Queue<br/>DeadLetterItem]
        DLQ --> Replay[POST /v1/lifecycle/sweep Replay]
        Replay --> Channel
    end
```

---

## 5. PostgreSQL 12-Table Relational Schema ERD

```mermaid
erDiagram
    tenants ||--o{ applications : owns
    tenants ||--o{ api_credentials : issues
    tenants ||--o{ tenant_wrapped_keys : possesses
    tenants ||--o{ durable_cache_entries : partitions
    tenants ||--o{ finops_ledger : records
    tenants ||--o{ outbox_events : logs
    tenants ||--o{ invalidation_outbox : buffers
    tenants ||--o{ knowledge_sources : manages
    knowledge_sources ||--o{ document_versions : tracks
    knowledge_sources ||--o{ dependency_edges : relates
    durable_cache_entries ||--o{ dependency_edges : invalidates
    tenants ||--o{ decision_records : audits
```
