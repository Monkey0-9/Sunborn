# Sunborn Data Flow & Trust Boundary Specification

**Document Version:** 2.0.0  
**Effective Commit:** `7f1f161`  
**Classification:** Security Architecture & Network Boundaries

---

## 1. Network Topologies & Trust Perimeters

```mermaid
flowchart TB
    subgraph UntrustedZone ["Untrusted Client Boundary"]
        Browser["Next.js Browser Client"]
        ClientApp["Third-Party AI Agent / Python SDK"]
    end

    subgraph DmzZone ["DMZ / BFF Layer"]
        BFF["Next.js BFF Proxy (:3000)"]
    end

    subgraph InternalMesh ["Internal Cluster Service Mesh (mTLS / Non-Root)"]
        Gateway["Sunborn Gateway Ingress (:8080)"]
        CacheL2["DragonflyDB Cluster (:6379)"]
        VectorDB["Qdrant Vector DB (:6333)"]
        AuthDB[("PostgreSQL Authoritative DB (:5432)")]
    end

    subgraph CryptoBoundary ["Cryptographic Enclave / KMS"]
        KMS["AWS KMS / HashiCorp Vault Transit"]
    end

    subgraph UpstreamProviders ["External LLM Providers"]
        OpenAI["OpenAI / Anthropic APIs (TLS 1.3)"]
    end

    Browser -->|Session Cookie + CSRF| BFF
    BFF -->|Internal API Token| Gateway
    ClientApp -->|Bearer API Key| Gateway

    Gateway <-->|Envelope Wrapping| KMS
    Gateway <-->|Redis RESP3| CacheL2
    Gateway <-->|REST / gRPC| VectorDB
    Gateway <-->|Async SQLx Pool| AuthDB
    Gateway -->|HTTPS + Retries| OpenAI
```

---

## 2. Request Lifecycle & Sequence Flows

### Flow 1: Exact Cache Hit ($C_1 / C_2$)
```mermaid
sequenceDiagram
    autonumber
    actor Client
    participant GW as Gateway
    participant PII as PII Vault
    participant FinOps as Spend Controller
    participant L1 as Exact Cache (L1/L2)

    Client->>GW: POST /v1/chat/completions (Bearer Key)
    GW->>GW: Authenticate API Key & Tenant ID
    GW->>PII: Tokenize PII (SSN, Email, Keys)
    GW->>FinOps: Reserve Estimated Budget
    GW->>L1: Lookup Canonical Hash Digest
    L1-->>GW: Exact Match Hit (Encrypted TDEK Envelope)
    GW->>GW: Authenticated AEAD Decrypt In-Memory
    GW->>FinOps: Release Reservation (Zero Origin Spend)
    GW->>PII: Hydrate Ephemeral Response Tokens
    GW-->>Client: 200 OK (X-Sunborn-Cache: HIT, ReuseCertificate)
```

### Flow 2: Semantic Cache Hit ($C_4$) with Authoritative Lease Verification
```mermaid
sequenceDiagram
    autonumber
    actor Client
    participant GW as Gateway
    participant Guard as Structural Guardrails
    participant Vec as Qdrant Vector Store
    participant NLI as Cross-Encoder & Conformal
    participant Epoch as Authoritative Epoch Registry

    Client->>GW: POST /v1/chat/completions
    GW->>Vec: Search Nearest Neighbor (Cosine Distance)
    Vec-->>GW: Candidate Entry + Certificate
    GW->>Guard: Verify Structural Fingerprint (Negation, Numbers, Transposition)
    Guard-->>GW: Structural Invariants Matched
    GW->>NLI: Joint Cross-Encoder NLI Evaluation (p_entailment >= 0.50)
    NLI-->>GW: Calibrated Entailment Confirmed
    GW->>Epoch: Validate Multi-Source Epoch Leases
    Epoch-->>GW: Leases Fresh & Unmodified
    GW-->>Client: 200 OK (X-Sunborn-Cache: SEMANTIC-HIT)
```

### Flow 3: Cache Miss & Upstream Execution
```mermaid
sequenceDiagram
    autonumber
    actor Client
    participant GW as Gateway
    participant FinOps as Spend Controller
    participant Single as Singleflight Coalescer
    participant Origin as LLM Provider
    participant DB as PostgreSQL Outbox

    Client->>GW: POST /v1/chat/completions
    GW->>Single: Acquire Execution Channel for Prompt Key
    Single->>Origin: Dispatch HTTPS Call (/chat/completions)
    Origin-->>Single: Model Completion (Tokens: P=120, C=85)
    Single-->>GW: Result Envelope
    GW->>FinOps: Settle Actual Token Spend to Ledger
    GW->>DB: Write-Behind Async Hydration (L1/L2/Vector/Postgres)
    GW-->>Client: 200 OK (X-Sunborn-Cache: MISS)
```

### Flow 4: Upstream Invalidation Cascade & DLQ Capture
```mermaid
sequenceDiagram
    autonumber
    participant Source as Knowledge Source Webhook
    participant Outbox as Invalidation Outbox Worker
    participant Graph as Dependency DAG / Roaring Index
    participant Cache as L1/L2 Cache Store
    participant Vec as Qdrant Vector Store
    participant DLQ as Dead Letter Queue

    Source->>Outbox: Invalidation Event (source_id: Uuid)
    Outbox->>Graph: Query Transitive Affected Keys
    Graph-->>Outbox: Affected Keys: [key_1, key_2]
    Outbox->>Cache: Delete exact entries
    Outbox->>Vec: Delete vector entries
    alt Eviction Fails (Timeout / Network Error)
        Outbox->>Outbox: Retry with Exponential Backoff
        Outbox->>DLQ: Capture Unresolved Event to DLQ
        Note over Outbox,Graph: Graph dependencies RETAINED (Never cleared prematurely!)
    else Eviction Acknowledged
        Outbox->>Graph: Clear Dependencies & Advance Epoch
    end
```
