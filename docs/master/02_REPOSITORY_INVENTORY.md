# Sunborn Repository Inventory & Dependency Architecture

**Document Version:** 2.0.0  
**Audit Baseline:** Commit `7f1f161`  
**Total Tracked Files:** 360  
**Languages:** Rust (96), Markdown (101), TypeScript/React (34), Python (26), YAML (32), JSON (18), Shell/PowerShell (10)

---

## 1. Rust Workspace Catalog (9 Members)

| Workspace Member | Path | Primary Responsibilities | Direct Dependencies |
|---|---|---|---|
| `sunborn-core` | [`crates/sunborn-core`](file:///C:/Sunborn/crates/sunborn-core) | Domain models, Request/Response envelopes, `ReuseCertificate`, FinOps utility, VectorClock | `serde`, `tokio`, `chrono`, `sha2`, `uuid` |
| `sunborn-identity` | [`crates/sunborn-identity`](file:///C:/Sunborn/crates/sunborn-identity) | RFC 8785 JSON canonicalization, PII redaction vault, AES-256-GCM AEAD, AWS KMS / Vault / HSM | `aes-gcm`, `zeroize`, `aws-sdk-kms`, `sha2`, `regex` |
| `sunborn-cache` | [`crates/sunborn-cache`](file:///C:/Sunborn/crates/sunborn-cache) | Lock-free L1 cache, FinOps GDSF eviction, S3-FIFO, DragonflyDB L2 driver, circuit breakers | `redis`, `dashmap`, `tokio`, `async-trait` |
| `sunborn-semantic` | [`crates/sunborn-semantic`](file:///C:/Sunborn/crates/sunborn-semantic) | Qdrant HNSW vector client, Tier C1.5 structural guardrails, Cross-Encoder NLI, Conformal Risk | `reqwest`, `sha2`, `hex`, `tokio`, `async-trait` |
| `sunborn-invalidation`| [`crates/sunborn-invalidation`](file:///C:/Sunborn/crates/sunborn-invalidation) | Knowledge DAG graph, Roaring Bitmap lineage, monotonic epoch leases, durable outbox | `roaring`, `uuid`, `chrono`, `tokio`, `dashmap` |
| `sunborn-db` | [`crates/sunborn-db`](file:///C:/Sunborn/crates/sunborn-db) | PostgreSQL schema authority, migrations 0001-0003, group-commit write-behind buffer | `sqlx` (postgres only), `tokio`, `chrono` |
| `sunborn-execution` | [`crates/sunborn-execution`](file:///C:/Sunborn/crates/sunborn-execution) | Execution DAG planning, topological cycle detection, Merkle sub-DAG reuse, tool memoization | `sha2`, `hex`, `serde`, `chrono` |
| `sunborn-gateway` | [`crates/sunborn-gateway`](file:///C:/Sunborn/crates/sunborn-gateway) | Axum 0.8 HTTP API gateway, OpenAI/Anthropic wire compatibility, Radix router, singleflight | `axum`, `tower`, `tower-http`, `tracing`, `tokio` |
| `sunborn-rebuild` | [`apps/sunborn-rebuild`](file:///C:/Sunborn/apps/sunborn-rebuild) | Standalone cold-start disaster-recovery and index reconstruction application | `sqlx`, `tokio`, `tracing`, `clap` |

---

## 2. Frontend & SDK Catalog

### Frontend Application
- **Path**: [`frontend/`](file:///C:/Sunborn/frontend)
- **Framework**: Next.js 15.5.29 App Router + React 19 + Tailwind CSS + Lucide Icons
- **Key Modules**:
  - `src/app/page.tsx`: Executive dashboard with real-time throughput metrics and cluster capacity modeler.
  - `src/app/api/proxy/[...path]/route.ts`: Server-side BFF reverse proxy with deny-by-default RBAC and CSRF protection.
  - `src/app/api/auth/route.ts`: Fail-closed session authentication and cookie management.
  - `src/components/simulator/stress-simulator.tsx`: Multi-region cluster fleet sizing and load modeling engine.
  - `src/app/ceremony/`, `execution/`, `explorer/`, `finops/`, `governance/`, `knowledge/`, `learning/`, `security/`: 8 operational views.

### Python SDK
- **Path**: [`sdk/python/`](file:///C:/Sunborn/sdk/python)
- **Package Name**: `sunborn` (v0.1.0rc1)
- **Key Classes**:
  - `SunbornClient`: Synchronous client with connection pooling, retries, and OpenAI-compatible method signatures.
  - `AsyncSunbornClient`: Asynchronous asyncio-compatible client with async context manager support.
  - `@sunborn_tool`: Function decorator for deterministic tool execution DAG memoization and Merkle attestation.

---

## 3. Deployment, Infrastructure & Research Artifacts

### Deployment
- [`deploy/Dockerfile`](file:///C:/Sunborn/deploy/Dockerfile): Multi-stage Rust builder and Distroless `nonroot` (uid 65532) minimal runtime (<35MB).
- [`deploy/Dockerfile.frontend`](file:///C:/Sunborn/deploy/Dockerfile.frontend): Multi-stage Next.js runner with non-root node user.
- [`deploy/helm/sunborn/`](file:///C:/Sunborn/deploy/helm/sunborn): Production Helm chart with HPA, PDB, NetworkPolicies, ServiceMonitors.
- [`deploy/k8s/`](file:///C:/Sunborn/deploy/k8s): Kustomize-compatible production manifests with zero placeholder secrets.

### Research & Verification
- [`research/results/`](file:///C:/Sunborn/research/results): Held-out 124-query benchmark datasets, run manifests, and signed reproduction certificates.
- [`research/verify_certificate.py`](file:///C:/Sunborn/research/verify_certificate.py): Independent cryptographic and git-tree provenance verifier.
- [`benchmarks/`](file:///C:/Sunborn/benchmarks): Adversarial evaluation suite (39 scenarios) and graph storage churn simulation.
- [`scripts/verify.ps1`](file:///C:/Sunborn/scripts/verify.ps1) & [`scripts/verify.sh`](file:///C:/Sunborn/scripts/verify.sh): Master 11-stage verification pipeline.
