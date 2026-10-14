# Sunborn

> **A full-stack enterprise semantic caching and execution-reuse platform for RAG and AI agents.**

Sunborn decouples AI application latency, token expenditure, and origin failure risks through multi-tier deterministic and semantic execution reuse while enforcing zero data drift and strict tenant isolation.

---

## 🏛️ System Architecture

```
[ Client / Agent ]
       │
       ▼
[ Sunborn Gateway (Axum / Rust) ]
   ├── 1. AuthN & Tenant Resolution
   ├── 2. Canonical Request Engine (C1)
   ├── 3. Deterministic Identity Hashing
   ├── 4. Exact Cache Lookup (Dragonfly / Memory)  ──[HIT]──► Return Safe Cache Hit
   └── 5. [MISS] ──► Origin Provider Execution
                       │
                       ▼
         Cache Admission Engine & Telemetry Sink
```

### Key Architectural Invariants
1. **Authoritative Origin Truth**: No accelerator (Dragonfly, Qdrant, Neo4j) is a correctness authority. PostgreSQL and origin providers are authoritative; all caches are fully rebuildable.
2. **Tenant Isolation**: Keys and retrieval partitions are namespaced by `TenantId` and `ApplicationId`.
3. **Deterministic Equivalence**: Canonical hashing normalizes whitespace, serializes JSON fields deterministically, and isolates policy versions.

---

## 📁 Repository Structure

```
.
├── apps/               # Executables & CLI applications
├── crates/             # Modular Rust workspace
│   ├── sunborn-core/       # Domain models, envelopes, error taxonomy, config
│   ├── sunborn-identity/   # Canonical request engine & identity hashing
│   ├── sunborn-cache/      # Exact cache traits, memory & Dragonfly backends
│   └── sunborn-gateway/    # Axum HTTP gateway, routes, middleware
├── frontend/           # Next.js control plane UI
├── deploy/             # Docker Compose & Kubernetes manifests
├── docs/               # Architecture documents and ADRs
│   ├── architecture/       # Exported Linear specifications
│   └── adr/                # Architecture Decision Records
├── benchmarks/         # Performance benchmarks and SemanticCacheBench
├── research/           # Scientific experiment runners & ablations
├── tests/              # Multi-crate integration & contract test suites
├── Makefile            # Standard developer commands
└── CODEOWNERS          # Ownership and governance matrix
```

---

## 🚀 Quickstart & Developer Onboarding

### Prerequisites
- **Rust**: 1.80+ (`rustc`, `cargo`)
- **Node.js**: 20+ (`node`, `npm`)
- **Docker** (optional for local multi-service stack)

### 1. Verification & Testing
```bash
# Check code formatting and run all workspace unit & integration tests
make test
```

### 2. Running Sunborn Gateway Locally
```bash
make run
```
The gateway will start on `http://127.0.0.1:8080`.

### 3. Verification Endpoints
- **Health Check**:
  ```bash
  curl -s http://localhost:8080/health | jq
  ```
- **Readiness Probe**:
  ```bash
  curl -s http://localhost:8080/ready | jq
  ```
- **Execute Cached Request**:
  ```bash
  curl -s -X POST http://localhost:8080/v1/requests \
    -H "Content-Type: application/json" \
    -d '{
      "request_id": "00000000-0000-0000-0000-000000000001",
      "tenant_id": "tenant-alpha",
      "application_id": "app-rag",
      "model": "gpt-4o",
      "prompt": "What is semantic caching in RAG?"
    }' | jq
  ```

---

## 📋 Linear Project Synchronization
Sunborn development is tracked against the Linear workspace `Monkey0-9` (Project **Sunborn**).
Linear credentials are configured in `~/.gemini/config/mcp_config.json` and `.env`.
