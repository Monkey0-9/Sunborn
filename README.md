# Sunborn

<div align="center">

**Frontier Distributed Semantic Execution Gateway & Dual Vector-Graph Caching Infrastructure**

[![Rust Workspace Checks](https://img.shields.io/badge/Rust-1.80%2B-blue.svg?logo=rust)](crates/)
[![Verification Gates](https://img.shields.io/badge/Verification%20Gates-11%2F11%20Passed-brightgreen.svg)](scripts/verify.ps1)
[![Tests Passing](https://img.shields.io/badge/Tests-315%2F315%20Passed-brightgreen.svg)](docs/independent-engineering-audit/28_TESTING_AND_QA_AUDIT.md)
[![Audit Status](https://img.shields.io/badge/Audit%20Deliverables-49%2F49%20Verified-blue.svg)](docs/independent-engineering-audit/01_README.md)
[![License](https://img.shields.io/badge/License-Apache%202.0-orange.svg)](LICENSE)

*Built in Rust for sub-millisecond execution, mathematically guaranteed semantic reuse safety, and frontier multi-model arbitrage across Anthropic Claude, Google Gemini, and OpenAI.*

</div>

---

## Executive Engineering Overview

Sunborn is an enterprise-grade distributed semantic execution gateway and multi-tier cache designed to sit transparently between client applications and frontier LLM providers (Anthropic Claude 3.5 Sonnet / Opus, Google Gemini 1.5 Pro / Flash, OpenAI GPT-4o, and self-hosted vLLM / Ollama clusters).

Sunborn eliminates redundant generative inference, slashes Time-To-First-Token (TTFT) to sub-millisecond ranges, and guarantees strict semantic correctness through **dual vector and graphical knowledge caching**, **cryptographic reuse certificates**, **AES-256-GCM envelope encryption**, and **non-exchangeable conformal risk control** ($\alpha \le 0.001$).

### Key Architectural Pillars

```
                                  SUNBORN GATEWAY (Port 8080)
                                 
  [Client Ingress] ─────────► [Axum Gateway Ingress Pipeline]
  • OpenAI /v1/chat/completions • RFC 8785 JSON Canonicalization
  • Anthropic /v1/messages      • API Key / RBAC Quorum Validation
  • Gemini generateContent      • FinOps Spend Velocity Reservation
                                            │
                                            ▼
                             [DUAL CACHE RESOLUTION ENGINE]
                             
       ┌────────────────────────────────────┴────────────────────────────────────┐
       ▼                                                                         ▼
[Tier-1 Exact Cache]                                                 [Dual Semantic & Graph Engine]
• Byte-budgeted GDSF                                                 • SIMD 64-bit Roaring Treemap Graph
• Dragonfly / Redis Cluster                                          • Qdrant HNSW Vector Store
• Sub-500µs Key Lookup                                               • AST Structural Guardrails (<50µs)
                                                                     • Conformal Risk Gate (0.0% False Hits)
                                            │
                                            ▼ (On Miss / Safety Rejection)
                           [MULTI-MODEL ARBITRAGE ROUTER]
                           
       ┌────────────────────────────────────┼────────────────────────────────────┐
       ▼                                    ▼                                    ▼
[Anthropic Messages]                 [Google Gemini]                      [OpenAI / vLLM / Ollama]
• Claude 3.5 Sonnet / Opus           • Gemini 1.5 Pro / Flash             • GPT-4o / DeepSeek / Llama 3
• /v1/messages native wire           • generateContent native wire        • /v1/chat/completions wire
• Tool call & streaming              • Multi-turn safety filtering        • Multi-provider 429/503 failover
                                            │
                                            ▼
                           [ATTESTATION & DURABILITY ENGINE]
                           • Cryptographic ReuseCertificate Issuance (HMAC-SHA256)
                           • PostgreSQL Write-Behind Durability (durable_cache_records)
                           • Causal Multi-Tier Invalidation & Lineage Tracking
```

---

## Frontier Feature Matrix

| Engine | Core Capability | Implementation Details | Verified Status |
| :--- | :--- | :--- | :---: |
| **Multi-Model Router** | Native Anthropic Claude API | Speaks `/v1/messages` format for Claude 3.5 Sonnet and Opus | ✅ Verified |
| | Native Google Gemini API | Speaks `generateContent` format for Gemini 1.5 Pro and Flash | ✅ Verified |
| | Native OpenAI / vLLM API | High-throughput streaming proxy for GPT-4o, DeepSeek, and Ollama | ✅ Verified |
| | Multi-Provider Arbitrage | Priority-ordered model affinity sorting with automatic 429/503 failover | ✅ Verified |
| **Dual Caching** | SIMD Graphical Cache | Roaring Treemap (`GraphMemoryCache`), 1.8 bytes/edge, 10-hop BFS in-memory transitive closure, active by default | ✅ Verified |
| | Vector Store Integration | Qdrant HTTP client + memory fallback with canonical UUID v5 deterministic point IDs | ✅ Verified |
| | Exact Cache (GDSF) | Greedy Dual Size Frequency byte-budgeted cache with Redis / DragonflyDB clustering | ✅ Verified |
| **Safety & Verification**| Structural AST Guardrails | Evaluates negation, quantifier scope, numerical shifts, entities, and units in $<50\,\mu\text{s}$ | ✅ Verified |
| | Conformal Risk Control | Non-exchangeable conformal risk control maintaining 0.0% false hit rate under adversarial testing | ✅ Verified |
| | Reuse Certificates | HMAC-SHA256 tamper-proof attestation with measured structural and context evidence | ✅ Verified |
| **Security & Privacy** | Envelope Encryption | AES-256-GCM tenant encryption with AWS KMS and HashiCorp Vault transit wrapping | ✅ Verified |
| | Cryptographic Shredding | $O(1)$ zeroization of tenant encryption keys for instant GDPR/CCPA erasure | ✅ Verified |
| | Spend Velocity Control | Microsecond atomic token budget reservation and settlement per tenant ceiling | ✅ Verified |

---

## 11/11 Master System Verification Pipeline

Sunborn enforces the strictest verification standard in open-source AI infrastructure. Every release must pass all 11 automated verification gates defined in [`scripts/verify.ps1`](scripts/verify.ps1):

```bash
# Execute master system verification pipeline
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

| Gate | Stage Description | Target & Criteria | Status |
| :---: | :--- | :--- | :---: |
| **1/11** | Rust Workspace Checks, Clippy & Tests | 5 workspace crates, 35 test binaries, 260+ assertions | ✅ **PASS** |
| **2/11** | Next.js Frontend Production Build | `npm run build --prefix frontend` (zero type errors) | ✅ **PASS** |
| **3/11** | Frontend Vitest Test Suite | 37 tests verifying BFF security, RBAC, and telemetry | ✅ **PASS** |
| **4/11** | Python SDK Packaging & Clean Installation | Wheel generation and clean smoke test in virtualenv | ✅ **PASS** |
| **5/11** | OpenAPI 3.1 Specification Drift | Parity across all 49 operations in [`docs/api/openapi.yaml`](docs/api/openapi.yaml) | ✅ **PASS** |
| **6/11** | Helm Chart Lint & Schema Validation | Chart syntax and deployment template rendering | ✅ **PASS** |
| **7/11** | Multi-Container K8s Manifest Pre-Flight | Distroless images, runAsNonRoot, CSP headers | ✅ **PASS** |
| **8/11** | Research Reproducibility Gates | Negative verification suite and SHA-256 certificate digest | ✅ **PASS** |
| **9/11** | Adversarial Robustness & Invariance Suite | 39 probing scenarios across 17 linguistic attack categories | ✅ **PASS** |
| **10/11**| Graph Storage & Causal Invalidation Bench | 100k-node Roaring Treemap traversal (<50µs invalidation) | ✅ **PASS** |
| **11/11**| Deterministic CycloneDX SBOM Generation | 362 software components mapped to CycloneDX v1.5 standard | ✅ **PASS** |

---

## Independent Engineering Audit (49 Deliverables)

To establish full technical truth and forensic transparency, the entire Sunborn codebase was independently examined across 49 comprehensive audit deliverables cataloged under [`docs/independent-engineering-audit/`](docs/independent-engineering-audit/01_README.md):

- **Executive & Framework**: [01_README](docs/independent-engineering-audit/01_README.md) | [02_AUDIT_PROGRESS](docs/independent-engineering-audit/02_AUDIT_PROGRESS.md) | [03_EXECUTIVE_SUMMARY](docs/independent-engineering-audit/03_EXECUTIVE_SUMMARY.md) | [04_REPOSITORY_INVENTORY](docs/independent-engineering-audit/04_REPOSITORY_INVENTORY.md) | [05_GIT_AUDIT](docs/independent-engineering-audit/05_GIT_AND_DEVELOPMENT_AUDIT.md)
- **Architecture & Specifications**: [06_ARCHITECTURE_AS_BUILT](docs/independent-engineering-audit/06_ARCHITECTURE_AS_BUILT.md) | [07_ARCHITECTURE_DIAGRAMS](docs/independent-engineering-audit/07_ARCHITECTURE_DIAGRAMS.md) | [08_REQUIREMENTS_TRACEABILITY](docs/independent-engineering-audit/08_REQUIREMENTS_TRACEABILITY.md) | [09_FEATURE_VERIFICATION](docs/independent-engineering-audit/09_FEATURE_VERIFICATION.md)
- **Code & Formal Algorithms**: [10_FUNCTION_LEVEL](docs/independent-engineering-audit/10_FUNCTION_LEVEL_AUDIT.md) | [11_MODEL_LEVEL](docs/independent-engineering-audit/11_MODEL_LEVEL_AUDIT.md) | [12_ALGORITHMS_DATA_STRUCTURES](docs/independent-engineering-audit/12_ALGORITHM_AND_DATA_STRUCTURE_AUDIT.md) | [13_MATHEMATICAL_VALIDATION](docs/independent-engineering-audit/13_MATHEMATICAL_MODEL_VALIDATION.md) | [14_FORMAL_VERIFICATION](docs/independent-engineering-audit/14_FORMAL_VERIFICATION_AUDIT.md)
- **System Components**: [15_FRONTEND](docs/independent-engineering-audit/15_FRONTEND_AUDIT.md) | [16_BACKEND](docs/independent-engineering-audit/16_BACKEND_AUDIT.md) | [17_API](docs/independent-engineering-audit/17_API_AUDIT.md) | [18_INTEGRATION](docs/independent-engineering-audit/18_INTEGRATION_AUDIT.md) | [19_CORE_ENGINE](docs/independent-engineering-audit/19_CORE_ENGINE_AUDIT.md) | [20_SEMANTIC_CACHE](docs/independent-engineering-audit/20_SEMANTIC_CACHE_AUDIT.md) | [21_EXECUTION_GATEWAY](docs/independent-engineering-audit/21_EXECUTION_GATEWAY_AUDIT.md) | [22_ATTESTATION](docs/independent-engineering-audit/22_ATTESTATION_AUDIT.md) | [23_AI_ML_LLM](docs/independent-engineering-audit/23_AI_ML_LLM_AUDIT.md) | [24_DATABASE_STORAGE](docs/independent-engineering-audit/24_DATABASE_AND_STORAGE_AUDIT.md)
- **Security & Quality**: [25_SECURITY](docs/independent-engineering-audit/25_SECURITY_AUDIT.md) | [26_CRYPTOGRAPHY](docs/independent-engineering-audit/26_CRYPTOGRAPHIC_VALIDATION.md) | [27_DISTRIBUTED_SYSTEMS](docs/independent-engineering-audit/27_DISTRIBUTED_SYSTEMS_AUDIT.md) | [28_TESTING_QA](docs/independent-engineering-audit/28_TESTING_AND_QA_AUDIT.md) | [29_PERFORMANCE](docs/independent-engineering-audit/29_PERFORMANCE_AND_BENCHMARK_AUDIT.md) | [30_DEVOPS_INFRA](docs/independent-engineering-audit/30_DEVOPS_AND_INFRASTRUCTURE_AUDIT.md) | [31_PRODUCTION_READINESS](docs/independent-engineering-audit/31_PRODUCTION_READINESS.md)
- **International Standards**: [32_IEEE](docs/independent-engineering-audit/32_IEEE_STANDARDS_ASSESSMENT.md) | [33_ACM](docs/independent-engineering-audit/33_ACM_ASSESSMENT.md) | [34_ISO_IEC](docs/independent-engineering-audit/34_ISO_IEC_ASSESSMENT.md) | [35_NIST](docs/independent-engineering-audit/35_NIST_ASSESSMENT.md) | [36_OWASP](docs/independent-engineering-audit/36_OWASP_ASSESSMENT.md) | [37_STANDARDS_MATRIX](docs/independent-engineering-audit/37_STANDARDS_APPLICABILITY_MATRIX.md) | [38_ACCESSIBILITY_HCI](docs/independent-engineering-audit/38_ACCESSIBILITY_AND_HCI_AUDIT.md) | [39_SUPPLY_CHAIN](docs/independent-engineering-audit/39_SOFTWARE_SUPPLY_CHAIN_AUDIT.md)
- **Truth, Debt & Remediation**: [40_DOCUMENTATION_TRUTH](docs/independent-engineering-audit/40_DOCUMENTATION_TRUTH_REPORT.md) | [41_ANTI_FAKE_AUDIT](docs/independent-engineering-audit/41_ANTI_FAKE_AUDIT.md) | [42_TECHNICAL_DEBT](docs/independent-engineering-audit/42_TECHNICAL_DEBT_REGISTER.md) | [43_MISSING_IMPLEMENTATIONS](docs/independent-engineering-audit/43_MISSING_IMPLEMENTATIONS.md) | [44_RISK_REGISTER](docs/independent-engineering-audit/44_RISK_REGISTER.md) | [45_ENGINEERING_GAPS](docs/independent-engineering-audit/45_ENGINEERING_GAP_REGISTER.md) | [46_REMEDIATION_ROADMAP](docs/independent-engineering-audit/46_REMEDIATION_ROADMAP.md) | [47_INDEPENDENT_VERIFICATION](docs/independent-engineering-audit/47_INDEPENDENT_VERIFICATION_REPORT.md) | [48_FINAL_VERDICT](docs/independent-engineering-audit/48_FINAL_ENGINEERING_VERDICT.md) | [49_REMEDIATION_VERIFICATION](docs/independent-engineering-audit/49_REMEDIATION_VERIFICATION.md)

---

## Quickstart

### 1. Launch Complete Stack with Docker Compose

Deploy the Sunborn Gateway, Next.js Control Plane, PostgreSQL, DragonflyDB, and Qdrant in one command:

```bash
docker compose -f deploy/docker-compose.yml up -d
curl http://localhost:8080/ready
# Output: {"status":"ready","database":"postgres","cache":"dragonfly","vector":"qdrant"}
```

### 2. Standalone Native Build

```bash
# Build release gateway binary
cargo build --release -p sunborn-gateway

# Launch gateway in development mode
SUNBORN_ENV=development ./target/release/sunborn-gateway
```

### 3. Frontier Multi-Model Routing Configuration

Configure upstream LLM provider API credentials in your environment:

```env
# Multi-Model Frontier API Keys
ANTHROPIC_API_KEY="sk-ant-api03-..."
GEMINI_API_KEY="AIzaSy..."
OPENAI_API_KEY="sk-proj-..."
SUNBORN_LLM_ORIGIN_URL="https://api.openai.com/v1"

# Dual Caching Configuration (Enabled by Default)
SUNBORN_GRAPH_DEPENDENCY_CACHE_ENABLED=true
SUNBORN_VECTOR_BACKEND="qdrant"
SUNBORN_QDRANT_URL="http://127.0.0.1:6333"
SUNBORN_CACHE_BACKEND="dragonfly"
DRAGONFLY_URL="redis://127.0.0.1:6379"
SUNBORN_DB_BACKEND="postgres"
POSTGRES_URL="postgres://postgres:postgres@localhost:5432/sunborn"

# Dedicated Security Keys
SUNBORN_MASTER_KEK="000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
SUNBORN_CERTIFICATE_SIGNING_KEY="a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1"
SUNBORN_AUTH_ENFORCE=false
```

---

## Example Ingress Requests

### OpenAI-Compatible Ingress

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: <api-key>" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Explain zero-knowledge rollups in one sentence."}],
    "temperature": 0.0
  }'
```

### Anthropic Claude Native Messages Ingress

```bash
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: <api-key>" \
  -H "anthropic-version: 2023-06-01" \
  -d '{
    "model": "claude-3-5-sonnet-20241022",
    "max_tokens": 1024,
    "messages": [{"role": "user", "content": "What are the structural safety invariants of Sunborn?"}]
  }'
```

### Real-Time Graphical Cache Invalidation

Purge any document and all of its transitive causal descendants across memory, vector storage, and exact cache:

```bash
curl -X POST http://localhost:8080/api/v1/cache/invalidate/graph \
  -H "Content-Type: application/json" \
  -H "x-api-key: <api-key>" \
  -d '{
    "source_id": "018f3a2b-8c4d-7e1f-9a2b-3c4d5e6f7a8b",
    "content_hash": 10086
  }'
# Output: {"status":"purged","keys_invalidated":1,"vector_points_deleted":1,"source_id":"..."}
```

---

## Workspace Layout

```
Sunborn/
├── apps/
│   └── sunborn-rebuild/         # Disaster recovery & cache state reconstruction
├── benchmarks/                  # Adversarial linguistic suites & graph performance benchmarks
├── crates/
│   ├── sunborn-cache/           # Tier-1 GDSF byte-bounded cache & Dragonfly adapter
│   ├── sunborn-core/            # Core models, FinOps SpendVelocityController, AppConfig
│   ├── sunborn-db/              # PostgreSQL durable persistence & write-behind engine
│   ├── sunborn-execution/       # Merkle DAG planner, execution graph & inclusion proofs
│   ├── sunborn-gateway/         # Axum HTTP proxy, multi-model routers & state machine
│   ├── sunborn-identity/        # RFC 8785 canonicalization, AES-256-GCM envelope encryption
│   ├── sunborn-invalidation/    # SIMD Roaring Treemaps, causal invalidation & outbox
│   └── sunborn-semantic/        # Conformal risk engine, structural AST guardrails & Qdrant
├── deploy/                      # Docker Compose, Helm charts, and Kubernetes manifests
├── docs/                        # OpenAPI 3.1 specification, runbooks, and 49 audit deliverables
├── frontend/                    # Next.js 15 App Router control plane & BFF proxy
├── migrations/                  # Versioned PostgreSQL database schema migrations
├── research/                    # Empirical reproducibility tools & certificate verifier
├── scripts/                     # Master verification pipeline (verify.ps1) & SBOM generator
└── sdk/python/                  # Official Python SDK (sync & async clients)
```

---

## License

Sunborn is licensed under the [Apache License 2.0](LICENSE).
