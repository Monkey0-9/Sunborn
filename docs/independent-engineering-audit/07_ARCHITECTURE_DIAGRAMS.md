# 07 — Architecture Diagrams (as built)

These reflect the code, not the README.

## 1. Crate dependency graph (workspace)

```mermaid
graph TD
  GW[sunborn-gateway]
  CORE[sunborn-core]
  ID[sunborn-identity]
  CACHE[sunborn-cache]
  SEM[sunborn-semantic]
  INV[sunborn-invalidation]
  EXEC[sunborn-execution]
  DB[sunborn-db]
  RB[apps/sunborn-rebuild]

  GW --> CORE
  GW --> ID
  GW --> CACHE
  GW --> SEM
  GW --> INV
  GW --> EXEC
  GW --> DB
  RB --> CORE
  RB --> DB
  RB --> CACHE
  RB --> SEM
```

## 2. Request path (default composition)

```mermaid
flowchart TD
  C[Client] --> R[Axum Router 40 routes]
  R --> A{Auth middleware}
  A -->|bypass| H[/health /ready /metrics /v1/auth/login/]
  A -->|ok| CAN[Canonicalize]
  CAN --> F[FinOps reserve]
  F --> L1{L1 exact lookup}
  L1 -->|hit| CERT{ReuseCertificate + epoch}
  CERT -->|valid| RET[Return cached]
  CERT -->|invalid| SEM
  L1 -->|miss| SEM{Semantic enabled?}
  SEM -->|no| ORIG
  SEM -->|yes| GRD[Structural fingerprint]
  GRD --> VEC[Vector search: memory brute force]
  VEC --> CONF[Conformal threshold]
  CONF --> ENT[Entailment score]
  ENT --> ORIG[Origin provider: Mock by default]
  ORIG --> PERSIST[Persist + issue certificate]
  PERSIST --> RET
```

## 3. Storage / durability reality

```mermaid
flowchart LR
  subgraph Defaults
    M1[MemoryExactCache GDSF]
    M2[MemoryVectorStore]
    M3[MemoryDatabaseStore]
    M4[MemoryExecutionStore]
    M5[In-memory outbox mpsc]
  end
  subgraph Optional["Opt-in adapters (real network)"]
    P[(PostgreSQL)]
    D[(Dragonfly/Redis)]
    Q[(Qdrant REST)]
    K[AWS KMS / Vault]
    N[Neo4j HTTP]
  end
  M1 -.->|switch backend=dragonfly| D
  M2 -.->|switch vector_backend=qdrant| Q
  M3 -.->|switch database_backend=postgres| P
  M5 -.->|dead code: outbox_events| P
  K -.->|selected by KMS_PROVIDER| K
  N -.->|no env path| N
```

## 4. Trust boundaries

```mermaid
flowchart LR
  U[Untrusted client] -->|API key / cookie| GW[Gateway]
  GW -->|bearer| LLM[LLM provider]
  GW -->|TLS| PG[(Postgres)]
  GW -->|TLS| RD[(Redis/Dragonfly)]
  GW -->|TLS| QD[(Qdrant)]
  GW -->|TLS| KMS[KMS/Vault]
  subgraph Weak["Weak boundary"]
    REQ[Client-supplied dependency_snapshots + knowledge_epoch]
  end
  REQ -->|baked into certificate| GW
```

## 5. Frontend topology

```mermaid
flowchart TD
  B[Browser] --> PAGES[Next.js pages: /, explorer, knowledge, execution, finops, governance, learning, security, ceremony, login]
  PAGES --> API[src/lib/api.ts ~45 methods]
  API --> BFF[/api/proxy catch-all/]
  BFF -->|reachable| GW[Gateway :8080]
  BFF -->|unreachable, non-strict| SIM[standalone-simulator.ts 698 LOC]
  PAGES --> AUTH[/api/auth/]
  AUTH -->|unreachable, non-strict| DEVBYPASS[accept any credential]
```
