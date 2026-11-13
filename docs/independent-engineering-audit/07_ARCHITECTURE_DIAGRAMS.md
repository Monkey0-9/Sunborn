# FORMAL ARCHITECTURE & EXECUTION DIAGRAMS

**Audit Standard**: IEEE 42010-2011 / ISO/IEC 42010  
**Target Architecture**: Sunborn Systems Engineering Architecture  

---

## 1. Master Request Execution Lifecycle

```mermaid
flowchart TD
    Client["Client / Agent / SDK<br/>(OpenAI or Anthropic wire format)"] --> Gateway["Sunborn Gateway (Axum Reverse Proxy)"]

    subgraph Ingress["1. Ingress & Canonical Identity"]
        Gateway --> Auth["RBAC/ABAC Auth & Credential Check"]
        Auth --> Canonicalizer["RFC 8785 JSON Canonicalizer (JCS + NFC)"]
        Canonicalizer --> FinOpsGate["Atomic FinOps Spend Reservation Gate"]
    end

    subgraph Tier1["2. Tier-1 Exact Match Cache (P99 < 0.5 ms)"]
        FinOpsGate --> ExactLookup{"Exact Hash Match?<br/>(W-TinyLFU / DragonflyDB)"}
        ExactLookup -- "Exact Hit" --> CertGate{"ReuseCertificate &<br/>Epoch Lease Valid?"}
        CertGate -- "Valid & Fresh" --> DecryptPayload["AES-256-GCM Decrypt (TDEK)"]
        DecryptPayload --> ReturnHit["Return Cached Response<br/>(HTTP 200, HIT_EXACT)"]
        CertGate -- "Invalid / Stale" --> EvictExact["Evict Stale Key &<br/>Fall Through"]
    end

    subgraph Tier2["3. Tier-2 Semantic Verification Cascade"]
        ExactLookup -- "Exact Miss" --> StructuralAST{"Structural AST Guardrails<br/>Negation, Numerics, Quantifiers?"}
        EvictExact --> StructuralAST
        StructuralAST -- "Inversion Detected" --> FallthroughOrigin["Bypass Semantic Cache"]
        StructuralAST -- "Structure Match" --> VectorSearch["Qdrant HNSW Vector Search"]
        VectorSearch --> ConformalGate{"Conformal Risk Control<br/>Task Stratum Risk <= Alpha?"}
        ConformalGate -- "Risk Exceeded" --> NLIEntailment{"Structural Entailment /<br/>Cross-Encoder NLI"}
        NLIEntailment -- "Contradiction / Drift" --> FallthroughOrigin
        NLIEntailment -- "Entailment Confirmed" --> EpochCheck{"Causal Epoch Gate<br/>Authoritative Freshness Valid?"}
        ConformalGate -- "Safe Conformal Hit" --> EpochCheck
        EpochCheck -- "Fresh" --> ReturnSemantic["Return Semantic Hit<br/>(HTTP 200, HIT_SEMANTIC)"]
        EpochCheck -- "Stale Mutation" --> FallthroughOrigin
    end

    subgraph Tier3["4. Tier-3 Execution DAG Memoization"]
        FallthroughOrigin --> MerkleCheck{"Deterministic Agent Tool?<br/>(RFC 6962 Merkle DAG)"}
        MerkleCheck -- "Sub-graph Pure" --> ReturnDAG["Return Replayed Tool Output"]
        MerkleCheck -- "Mutating / Miss" --> OriginCall["Call Upstream Origin LLM<br/>(OpenAI / Anthropic / Bedrock)"]
    end

    subgraph Durability["5. Authoritative Persistence & Invalidation"]
        OriginCall --> AppendWAL["Append to PostgreSQL WAL<br/>(System of Record)"]
        AppendWAL --> OutboxQueue["Transactional Outbox Queue<br/>(Durable DLQ Replay)"]
        OutboxQueue --> GraphInvalidate["Roaring Bitmap Lineage Graph<br/>(SIMD Transitive Invalidation)"]
        OutboxQueue --> AsyncWarm["Asynchronously Warm L1 / L2 Cache"]
    end

    ReturnHit --> Client
    ReturnSemantic --> Client
    ReturnDAG --> Client
    AsyncWarm --> Client
```

---

## 2. Multi-Stage Semantic Verification Cascade

```mermaid
flowchart LR
    Query["Incoming Prompt"] --> AST["1. Structural AST Tokenizer"]
    
    subgraph Guardrails["Microsecond Guardrails (<50 µs)"]
        AST --> Polarity["Negation Polarity Check"]
        AST --> Numerics["Numeric Sequence Extractor"]
        AST --> Quantifiers["Quantifier Scope (`all`, `none`)"]
        AST --> Entities["Proper Noun Entity Extractor"]
    end

    Polarity -- "Drift Detected" --> Reject["REJECT (Pass to Origin)"]
    Numerics -- "Transposition Detected" --> Reject
    Quantifiers -- "Scope Mismatch" --> Reject
    Entities -- "Entity Alteration" --> Reject

    Polarity -- "Pass" --> Vector["2. Qdrant HNSW Vector Search"]
    Numerics -- "Pass" --> Vector
    Quantifiers -- "Pass" --> Vector
    Entities -- "Pass" --> Vector

    subgraph StatisticalConfidence["Statistical Risk Engine"]
        Vector --> CRC["3. Conformal Risk Control (Alpha <= 0.001)"]
        CRC --> Wilson["4. Wilson Score 95% Confidence Interval"]
    end

    Wilson -- "In-Distribution (Coverage OK)" --> CausalGate["5. Monotonic Epoch Lease Gate"]
    Wilson -- "Out-of-Distribution Drift" --> CrossEncoder["Pluggable Cross-Encoder NLI"]
    CrossEncoder -- "Entailment Prob >= 0.99" --> CausalGate
    CrossEncoder -- "Neutral / Contradict" --> Reject

    CausalGate -- "Epoch Unmodified" --> CacheHit["SAFE SEMANTIC CACHE HIT"]
    CausalGate -- "Source Mutated" --> Reject
```

---

## 3. Real-Time Causal (RTC) Lineage Graph & Transitive Invalidation

```mermaid
flowchart TD
    subgraph Knowledge["Upstream Documents & Databases"]
        DocA["Document A (e.g., Compliance v2.4)"]
        DocB["Document B (e.g., Treasury Rates v1.0)"]
        DocC["Document C (e.g., Client KYC)"]
    end

    subgraph LineageGraph["64-Bit Roaring Treemap Lineage Graph (1.8 bytes / edge)"]
        HashA(("Doc Hash 0xA1"))
        HashB(("Doc Hash 0xB4"))
        HashC(("Doc Hash 0xC7"))
        DocA --> HashA
        DocB --> HashB
        DocC --> HashC

        Rec1[["Execution Record 1<br/>(Portfolio Valuation)"]]
        Rec2[["Execution Record 2<br/>(Liquidity Forecast)"]]
        Rec3[["Execution Record 3<br/>(Risk Score)"]]

        HashA --> Rec1
        HashB --> Rec1
        HashB --> Rec2
        HashC --> Rec3
    end

    subgraph RTCInvalidation["Transitive SIMD Bitset Invalidation (<50 µs)"]
        Mutation["Document B Mutated!<br/>(Treasury Rates v2.0)"] --> UnionOp["Roaring Bitmap SIMD Union<br/>(Locate all transitive dependents)"]
        UnionOp -.-> EvictRec1["Evict Rec 1 from L1/L2 Cache"]
        UnionOp -.-> EvictRec2["Evict Rec 2 from L1/L2 Cache"]
        EvictRec1 --> SafeOrigin["Subsequent requests bypass cache<br/>and fetch fresh origin answers"]
    end
```

---

## 4. Non-Accelerator Authority (NAA) & Disaster Recovery Flow

```mermaid
sequenceDiagram
    autonumber
    actor Admin as System Administrator / SRE
    participant Rebuild as apps/sunborn-rebuild (CLI Utility)
    participant PG as PostgreSQL (Authoritative WAL)
    participant Redis as DragonflyDB / Redis (Ephemeral Accelerator)
    participant Qdrant as Qdrant HNSW (Ephemeral Accelerator)

    Note over Redis,Qdrant: Disaster Scenario: Complete Memory/Disk Loss
    Admin->>Rebuild: Execute sunborn-rebuild --from-wal
    Rebuild->>PG: Acquire snapshot isolation read lock
    PG-->>Rebuild: Stream authoritative execution_records & knowledge_documents
    loop Stream Execution Records
        Rebuild->>Rebuild: Verify cryptographic ReuseCertificate HMAC
        Rebuild->>Redis: Re-populate L1/L2 exact hash entries
        Rebuild->>Qdrant: Re-index dense vector embeddings + ContextDigest
    end
    Rebuild->>Admin: Rebuild complete (100% state restored from PG WAL)
    Note over PG,Qdrant: Zero data drift; accelerators fully reconstructed
```
