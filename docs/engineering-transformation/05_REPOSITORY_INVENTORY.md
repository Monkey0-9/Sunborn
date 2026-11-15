# Repository Inventory & Component Architecture

**Total Tracked Files:** 344  
**Source Code Lines:** ~42,500 LOC  
**Rust Workspace Crates:** 8 crates + 2 standalone binaries  
**Frontend Framework:** Next.js 15 (React 19, TypeScript)  
**SDK Package:** Python 3.10+ (Sync & Async)  

---

## 1. Workspace Crate Architecture

| Crate / Application | Path | LOC | Role & Responsibility | Critical Dependencies |
| :--- | :--- | :--- | :--- | :--- |
| `sunborn-core` | `crates/sunborn-core` | 3,850 | Core domain models, FinOps spend velocity controller, RFC 8785 canonicalization, Ed25519 certificates | `ed25519-dalek`, `serde`, `uuid`, `chrono` |
| `sunborn-cache` | `crates/sunborn-cache` | 2,120 | L1 in-memory GDSF cache, L2 Dragonfly/Redis async client, circuit breaker | `moka`, `redis`, `dashmap`, `tokio` |
| `sunborn-identity` | `crates/sunborn-identity` | 2,450 | AES-256-GCM envelope encryption, tenant DEK wrapping, cryptographic shredding, PII tokenizer | `aes-gcm`, `ring`, `subtle`, `zeroize` |
| `sunborn-execution` | `crates/sunborn-execution` | 2,980 | Merkle DAG execution planner, topological sort, inclusion proofs, cycle detection | `sha2`, `petgraph`, `serde_json` |
| `sunborn-invalidation` | `crates/sunborn-invalidation` | 4,200 | Two-phase atomic invalidation orchestrator, SIMD Roaring bitmap lineage, epoch lease fencer | `roaring`, `redis`, `tokio`, `async-trait` |
| `sunborn-semantic` | `crates/sunborn-semantic` | 4,950 | Structural AST guardrails, Top-8 ambiguity abstention, conformal risk controller, Qdrant store | `qdrant-client`, `reqwest`, `tokenizers` |
| `sunborn-db` | `crates/sunborn-db` | 2,800 | PostgreSQL repository, singular migration runner (`_sunborn_migrations`), write-behind buffer | `sqlx`, `deadpool-postgres`, `tokio` |
| `sunborn-gateway` | `crates/sunborn-gateway` | 8,900 | Axum 0.8 HTTP gateway, multi-provider router, singleflight streamer, auth middleware | `axum`, `tower`, `hyper`, `prometheus` |
| `sunborn-rebuild` | `apps/sunborn-rebuild` | 1,150 | Sovereign disaster recovery CLI, trust-monotonic state reconstruction | `clap`, `tokio`, `tracing` |
| `sunborn-verify` | `apps/sunborn-verify` | 980 | Independent zero-trust cryptographic verification CLI | `clap`, `ed25519-dalek`, `sha2` |

---

## 2. Directory Structure Breakdown

```
Sunborn/
├── .github/                      # CI/CD workflows (ci.yml, release.yml, security.yml)
├── apps/                         # Standalone operational command-line binaries
│   ├── sunborn-rebuild/          # Disaster recovery reconstruction utility
│   └── sunborn-verify/           # Standalone zero-trust verification binary
├── crates/                       # Modular Rust workspace libraries
│   ├── sunborn-cache/            # Tiered exact caching (GDSF + Dragonfly)
│   ├── sunborn-core/             # Shared primitives, FinOps, attestation
│   ├── sunborn-db/               # PostgreSQL storage & singular migrations
│   ├── sunborn-execution/        # Merkle DAG planning & memoization
│   ├── sunborn-gateway/          # Axum HTTP reverse proxy & routing
│   ├── sunborn-identity/         # Envelope cryptography & shredding
│   ├── sunborn-invalidation/     # Orchestrator, Roaring lineage, epochs
│   └── sunborn-semantic/         # AST guardrails & conformal risk control
├── docs/                         # Authoritative engineering documentation
│   ├── engineering-transformation/ # The 32 transformation reports + inventories
│   └── independent-engineering-audit/ # Baseline 5.2/10 audit records
├── frontend/                     # Next.js 15 App Router control plane & BFF
├── migrations/                   # Sequential SQL DDL migrations (0001-0005)
├── research/                     # Scientific evaluation, held-out dataset, papers
├── sbom.cyclonedx.json           # CycloneDX Software Bill of Materials
└── sdk/python/                   # Production Python client package
```
