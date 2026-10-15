# Sunborn

> **High-Performance Multi-Tenant Semantic Caching and Execution-Reuse Platform for AI Agents & LLM Applications.**

Sunborn decouples AI application latency, token expenditure, and origin failure risks through multi-tier deterministic and semantic execution reuse while mathematically enforcing zero false-hit data drift and strict zero-trust tenant isolation.

---

## 🏛️ System Architecture

```
[ AI Agent / OpenAI Client / Control Plane ]
                     │
                     ▼
       [ Sunborn Gateway (Axum / Rust) ]
   ├── Zero-Trust AuthN & Identity (Fail-Closed API Key Validation)
   ├── Canonical 10-D Identity Engine (Prompt, Tools, Temperature, Epoch)
   ├── Radix KV-Cache Affinity Router (Llama-3 / Claude Breakpoints)
   │
   ├── Tier 0/1: Exact Match Hot Path (< 120 µs, 10,000+ req/s)
   │   └── Circuit-Breaker Cache (DragonflyDB / In-Memory W-TinyLFU)
   │
   ├── Tier 2: Risk-Calibrated Semantic Reuse (Conformal Risk Control)
   │   └── Qdrant Vector Store + Negation & Numeric Inversion Guardrails
   │
   ├── Tier 3: Agentic Execution DAG Reuse (Deterministic Tool Subgraphs)
   │   └── Merkle Inclusion Proofs + Ephemeral Tool Memoization
   │
   └── Tier 4: Authoritative Durability & Reconstruction
       └── PostgreSQL Outbox & Event-Ordered Hydration (Non-Accelerator Authority)
```

### Core Architectural Guarantees
1. **Non-Accelerator Authority**: In-memory and vector acceleration tiers (DragonflyDB, Qdrant) are accelerators, never correctness authorities. PostgreSQL and origin providers are authoritative; all caches are deterministically rebuildable via `POST /v1/rebuild`.
2. **Zero-Trust Identity & Fail-Closed Boundaries**: Every protected route strictly mandates cryptographic API key validation. Tenant identity is resolved from server-side credentials—caller-supplied headers (`x-tenant-id`) alone cannot establish identity.
3. **State-Complete Equivalence**: Identity hashing incorporates tenant, application, model, policy version, canonicalized system and user prompts, sorted tool schemas, tool choices, response formats, temperatures, top-p, and retrieval hashes.
4. **Conformal Risk-Calibrated Semantic Reuse**: Mathematically verified 0.0% false-hit rate on adversarial semantic traps (negations, mutating clauses, numeric shifts) via structural guardrails and conformal risk controllers.
5. **O(1) Cryptographic Erasure (GDPR Article 17)**: Instant tenant offboarding via atomic zeroization of the 256-bit Tenant Data Encryption Key (TDEK), instantly rendering all cached ciphertext unrecoverable in sub-millisecond time.

---

## 🚀 Quickstart

### Prerequisites
- **Rust**: 1.80+ (`rustc`, `cargo`)
- **Python**: 3.10+ (for benchmarks, reproduction, and agent SDK)
- **Node.js**: 20+ (for Next.js frontend)

### 1. Build and Test the Workspace
```bash
# Run complete test matrix across all crates
cargo test --workspace --all-targets

# Verify code formatting and lint standards
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

### 2. Run the Gateway Locally
```bash
# Launch the Sunborn gateway on port 8080
cargo run -p sunborn-gateway
```

### 3. Verify Health & Service Readiness
```bash
# Health probe
curl -s http://localhost:8080/health | jq

# Readiness probe
curl -s http://localhost:8080/ready | jq

# Metrics scraper
curl -s http://localhost:8080/metrics
```

---

## ⚡ OpenAI-Compatible Drop-In Proxy

Sunborn provides a native, OpenAI-compliant `POST /v1/chat/completions` endpoint. Any agent framework (LangChain, LlamaIndex, CrewAI, AutoGen, OpenAI SDK) can connect by changing the base URL.

### Python OpenAI SDK Integration

```python
from openai import OpenAI

# Connect to Sunborn Gateway
client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="key-enterprise-fintech"  # Provisioned Sunborn API Key
)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a quantitative financial assistant."},
        {"role": "user", "content": "Calculate the Black-Scholes formula for European calls."}
    ],
    temperature=0.0
)

print(response.choices[0].message.content)

# On repeat queries: response received in < 1ms with sunborn_cached == True
```

### cURL OpenAI Example

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: key-enterprise-fintech" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a quantitative assistant."},
      {"role": "user", "content": "Explain Black-Scholes in 1 sentence."}
    ],
    "temperature": 0.0
  }' | jq
```

### Native Python SDK (`sdk/python/sunborn_agent.py`)

```python
from sdk.python.sunborn_agent import SunbornAgentClient

client = SunbornAgentClient(
    base_url="http://localhost:8080",
    api_key="sunborn-admin-key"
)

# 1. Chat Completion with automatic caching
resp = client.chat_completion(
    model="gpt-4o",
    prompt="What are the capital requirements under Basel III?",
    system_prompt="You are a banking compliance expert.",
    temperature=0.0
)
print(f"Content: {resp['choices'][0]['message']['content']}")
print(f"Cached : {resp['sunborn_cached']} (Tier: {resp['sunborn_tier']})")

# 2. Inspect Live FinOps Cost Savings
report = client.get_finops_report()
print(f"Total Avoided Spend: ${report['economics']['total_estimated_savings_usd']}")
```

---

## 📁 Repository Structure

```
.
├── apps/
│   ├── sunborn-gateway/     # High-throughput Axum HTTP/REST Gateway
│   └── sunborn-rebuild/     # CLI offline index reconstructor & hydration tool
├── crates/                  # Modular Rust Workspace (8 Crates)
│   ├── sunborn-core/        # Domain primitives, finops, cluster vector clocks, config
│   ├── sunborn-identity/    # Canonical request engine, 10-D identity, AES-256-GCM AEAD shredding
│   ├── sunborn-cache/       # Tiered exact cache, W-TinyLFU, Dragonfly adapter, circuit breaker
│   ├── sunborn-semantic/    # Micro-batched embedder, Qdrant adapter, conformal risk engine
│   ├── sunborn-execution/   # Agent execution DAG planner, Merkle proofs, tool memoization
│   ├── sunborn-invalidation/# Dependency DAG graph, monotonic epoch lease manager, outbox
│   └── sunborn-db/          # PostgreSQL store, write-behind group committer, migrations
├── frontend/                # Next.js 15 + TypeScript Control Plane & Telemetry Dashboard
├── sdk/python/              # Native Python SDK with OpenAI-compatible agent integration
├── benchmarks/              # SemanticCacheBench & Zipfian Capacity Soak suites
├── research/                # IEEE/ACM artifact evaluation & reproducibility package
├── deploy/                  # Docker Compose, Dragonfly, Qdrant & Kubernetes manifests
└── docs/                    # Architecture Decision Records (ADRs) & Integration Guides
```

---

## 🔒 Security & RBAC Contract

Sunborn enforces strict fail-closed authorization:

| Endpoint | Method | Required Scope | Description |
| :--- | :--- | :--- | :--- |
| `/health`, `/ready`, `/metrics` | `GET` | Public | Kubernetes liveness, readiness, and Prometheus scrapers |
| `/v1/chat/completions` | `POST` | Authenticated | OpenAI-compatible chat completion proxy |
| `/v1/requests` | `POST` | Authenticated | Native request envelope execution |
| `/v1/execution/plan` | `POST` | Authenticated | DAG execution reuse planner |
| `/v1/execution/results` | `POST` | Authenticated | Record deterministic tool output |
| `/v1/finops/report` | `GET` | Authenticated | Live token savings & break-even analytics |
| `/v1/routing/radix/*` | `GET`/`POST` | Authenticated | Radix KV-cache router inspection & routing |
| `/v1/governance/attest` | `POST` | Authenticated | Merkle execution inclusion attestation |
| `/v1/governance/verify-proof` | `POST` | Authenticated | Cryptographic Merkle proof verification |
| `/v1/caches/{key}` | `DELETE` | **Admin Only** | Evict specific exact cache entry |
| `/v1/invalidation` | `POST` | **Admin Only** | Trigger cascade invalidation by knowledge source |
| `/v1/execution/evict` | `POST` | **Admin Only** | Evict tool signatures by tool name or signature |
| `/v1/tenants/{id}` | `DELETE` | **Admin Only** | Complete GDPR Article 17 tenant cascade deletion |
| `/v1/governance/shred` | `POST` | **Admin Only** | Atomic O(1) AES-256-GCM cryptographic shredding |
| `/v1/rebuild` | `POST` | **Admin Only** | Hydrate accelerators from authoritative PostgreSQL |
| `/v1/chaos/trip`, `/reset` | `POST` | **Admin Only** | Fault-injection circuit breaker testing |
| `/v1/lifecycle/sweep` | `POST` | **Admin Only** | Evict TTL-expired entries across memory tiers |

---

## ⚙️ Configuration & Environment Variables

| Variable | Default | Description |
| :--- | :--- | :--- |
| `PORT` | `8080` | Gateway HTTP listen port |
| `HOST` | `0.0.0.0` | Gateway listen address |
| `SUNBORN_ENV` | `development` | Set to `production` to enforce strict environment key loading |
| `SUNBORN_ADMIN_KEY` | *(None)* | Secure master admin credential provisioned at boot |
| `SUNBORN_API_KEYS` | *(None)* | Key registry: `key:tenant:app:is_admin` comma-separated or JSON array |
| `SUNBORN_CACHE_BACKEND` | `memory` | Cache tier: `memory` (W-TinyLFU) or `dragonfly` |
| `DRAGONFLY_URL` | `redis://127.0.0.1:6379` | DragonflyDB Redis-compatible connection string |
| `SUNBORN_VECTOR_BACKEND` | `memory` | Vector tier: `memory` or `qdrant` |
| `QDRANT_URL` | `http://127.0.0.1:6334` | Qdrant gRPC / HTTP endpoint |
| `SUNBORN_DB_BACKEND` | `memory` | Authoritative DB: `memory` or `postgres` |
| `DATABASE_URL` | `postgres://...` | Authoritative PostgreSQL connection string |
| `SUNBORN_CORS_ALLOWED_ORIGINS` | `http://localhost:3000` | Comma-separated list of permitted web origins |

---

## 🔬 Scientific Reproducibility (IEEE/ACM Artifact Package)

Sunborn includes an automated artifact evaluation package (`research/reproduce.sh`) that dynamically validates all empirical performance baselines, security boundaries, and conformal risk guarantees:

```bash
# Run the complete reproduction evaluation suite
./research/reproduce.sh
```

### What `reproduce.sh` Executes Freshly:
1. **Workspace Quality**: Enforces `cargo fmt -- --check` and `cargo clippy -D warnings`.
2. **Test Matrix**: Executes all 60 unit, integration, and contract tests across all 8 crates.
3. **Gateway Performance**: Benchmarks exact cache hot path (verifying throughput > 10,000 req/s, P99 < 5ms).
4. **Adversarial Security**: Asserts fail-closed authentication, tenant spoofing rejection, cross-tenant isolation, and 403 Forbidden enforcement on all destructive endpoints.
5. **Fresh Semantic Benchmark (`SemanticCacheBench`)**: Evaluates 4 baselines (B0 No Cache, B1 Exact Hash, B2 Naive Cosine, B3 Sunborn Calibrated) on fresh runs:
   - **B3 False-Hit Rate**: Strictly asserts `false_hit_rate_pct == 0.0%` (0 false hits).
   - **B3 Precision**: Strictly asserts `precision_pct == 100.0%`.
   - **B2 Vulnerability**: Confirms that naive fixed cosine thresholds suffer high error rates on adversarial negations.
6. **Capacity Soak**: 10,000-request Zipfian churn test with live GDPR tenant crypto-shredding and memory RSS envelope verification.
7. **Certificate Generation**: Writes verifiable JSON evidence to `research/results/reproduction_certificate.json`.

---

## 💻 Frontend Control Plane

Sunborn includes an enterprise Next.js telemetry dashboard:

```bash
cd frontend
npm install
npm run dev
```

Visit `http://localhost:3000` to inspect:
- **Telemetry Overview**: Real-time hit ratios, token avoided economics, and request throughput.
- **Cache Explorer**: Inspection of deterministic keys, TTL, and cache tiers.
- **Agent Execution DAGs**: Interactive visualization of reusable agent tool subgraphs.
- **Governance & Crypto Shredding**: One-click GDPR Article 17 tenant offboarding.
- **Security & RBAC**: Real-time audit log of access attempts and policy decisions.

---

## 📄 License & Citations

Sunborn is licensed under the Apache 2.0 License. If you use Sunborn in academic research, please cite:

```bibtex
@inproceedings{sunborn2026semantic,
  title={Sunborn: Multi-Tenant Semantic Caching and Safe Execution Reuse for Autonomous AI Agents},
  author={Sunborn Engineering Team},
  booktitle={IEEE/ACM International Symposium on Computer Architecture (ISCA)},
  year={2026}
}
```
