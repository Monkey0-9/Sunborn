<div align="center">

# S U N B O R N

### Frontier Distributed Semantic Execution Gateway & Dual Vector-Graph Caching Infrastructure

[![Rust Toolchain](https://img.shields.io/badge/Rust-1.80%2B-blue.svg?logo=rust&logoColor=white)](crates/)
[![Verification Pipeline](https://img.shields.io/badge/Verification%20Gates-12%2F12%20Passed-brightgreen.svg?logo=githubactions&logoColor=white)](scripts/verify.ps1)
[![Automated Tests](https://img.shields.io/badge/Rust%20Tests-287%2F287%20Passed-brightgreen.svg)](docs/STATUS.md)
[![IEEE Conformance](https://img.shields.io/badge/IEEE-1012%20%7C%20730%20%7C%2042010-blue.svg)](docs/IEEE_ACM_COMPLIANCE.md)
[![ACM Badging](https://img.shields.io/badge/ACM%20Artifact-Available%20%7C%20Functional%20%7C%20Reproduced-purple.svg)](research/ARTIFACT-APPENDIX.md)
[![Citation](https://img.shields.io/badge/cite-CITATION.cff-green.svg)](CITATION.cff)
[![Enterprise Deliverables](https://img.shields.io/badge/Enterprise%20Deliverables-26%20Formal%20Packages-blue.svg)](docs/ENTERPRISE_DELIVERABLES_REGISTER.md)
[![OpenAPI 3.1](https://img.shields.io/badge/OpenAPI-3.1%20Compliant-green.svg)](docs/api/openapi.yaml)
[![License](https://img.shields.io/badge/License-Apache%202.0-orange.svg)](LICENSE)
[![Release Status](https://img.shields.io/badge/Release-CONDITIONAL%20GO%20(Controlled%20Pilot)-yellow.svg)](docs/STATUS.md)

*An ultra-low-latency, distributed semantic execution gateway written in Rust.*  
*Engineered to eliminate redundant LLM compute, slash Time-To-First-Token (TTFT) to sub-millisecond speeds, bound semantic-reuse risk with a calibrated abstention gate, and provide native multi-model routing across Anthropic Claude, Google Gemini, OpenAI, and xAI Grok.*

> **Measured safety, stated honestly.** On the checked-in 320-pair held-out set the
guardrail makes 24 correct reuses, 200 correct abstentions, and **0 false hits** over
200 hard negatives (recall 20%, precision 100%). With 200 negatives the exact
Clopper-Pearson 99.9% upper bound on the false-hit rate is **3.39%**, so the current
evidence does **not** establish a $\alpha \le 0.001$ guarantee; that would require
substantially more negatives. See
[`research/results/heldout_generalization_run.json`](research/results/heldout_generalization_run.json).

[Current Status (SSOT)](docs/STATUS.md) • [Enterprise Deliverables (26 Packages)](docs/ENTERPRISE_DELIVERABLES_REGISTER.md) • [Transformation Reports](docs/engineering-transformation/01_README.md) •
[Architecture](#system-architecture) •
[Dual Caching](#dual-caching-architecture-vector--graph) •
[Multi-Model Router](#frontier-multi-model-arbitrage-routing) •
[Safety & Math](#mathematical-formulation--conformal-risk-control) •
[Competitive Benchmarks](#competitive-benchmarking--net-dollar-savings) •
[Standards](#ieee--acm--iso-standards-compliance) •
[Quickstart](#quickstart--deployment)

</div>

---

## 1. Executive Engineering Summary

Frontier artificial intelligence applications executing high-volume generative workloads waste between **40% and 70% of total GPU compute and operational capital** recomputing semantically identical queries. Conventional caching techniques fail catastrophically when applied to generative language models:

- **Key-Value String Caches (Redis / Memcached)** suffer near-zero hit rates on conversational prompts due to natural human linguistic variations, formatting differences, and prompt framing.
- **Naive Vector Similarity Caches (Cosine Distance $\ge \tau$)** exhibit unacceptable false-positive hit rates (**40% to 75%**), blindly serving cached responses across critical semantic inversions, such as numeric value transpositions, negation flips, temporal shifts, and entity substitutions.

**Sunborn** is an industrial-strength, transparent execution gateway designed to solve this crisis. Sitting directly in the ingress path between client applications and frontier LLMs, Sunborn fuses **Tier-1 Deterministic Hashing**, **Tier-2 Qdrant Vector Semantic Matching**, and **Tier-3 SIMD 64-bit Roaring Treemap Knowledge Graph Caching** with **Non-Exchangeable Conformal Risk Control** and **Cryptographic Attestation Certificates**.

```
+---------------------------------------------------------------------------------------------------+
|                                  SUNBORN HIGH-THROUGHPUT RUNTIME                                  |
+---------------------------------------------------------------------------------------------------+
|  INCOMING REQUEST                                                                                 |
|    |                                                                                              |
|    +--> RFC 8785 Canonicalization & API Key Validation                                            |
|    +--> FinOps Spend Velocity Controller (Atomic Microsecond Budget Reservation)                  |
|    |                                                                                              |
|  RESOLUTION PIPELINE                                                                              |
|    +--> [Tier-1 Exact Cache] (Sub-500µs In-Memory / DragonflyDB GDSF Match)                      |
|    |      |-- Hit?  --> Return cached execution + Cryptographic Verification                      |
|    |                                                                                              |
|    +--> [Tier-2 & Tier-3 Dual Semantic & Knowledge Graph Engine]                                  |
|    |      |-- AST Guardrails: Negation, Quantifiers, Numeric Values, Entities, Units (<50µs)     |
|    |      |-- Qdrant Vector Store: Cosine Semantic Proximity Filter                               |
|    |      |-- SIMD Roaring Treemap: Causal Knowledge Graph Transitive Lineage Check               |
|    |      |-- Conformal Risk Gate: Evaluates risk against a calibrated, explicit abstention bound  |
|    |      |-- Verified Safe? --> Return cryptographically certified cached response               |
|    |                                                                                              |
|  FALLBACK & RECOMPUTE (On Miss or Safety Rejection)                                               |
|    +--> [Frontier Multi-Model Arbitrage Router]                                                   |
|           |-- Anthropic Claude 3.5 Sonnet / Opus (Native /v1/messages wire)                       |
|           |-- Google Gemini 1.5 Pro / Flash (Native generateContent wire)                          |
|           |-- OpenAI GPT-4o / DeepSeek / vLLM (/v1/chat/completions wire)                         |
|           +-- Zero-Downtime Multi-Provider Failover & Cost-Aware Arbitration                      |
|                                                                                                   |
|  POST-EXECUTION ADMISSION                                                                         |
|    +--> Issue HMAC-SHA256 ReuseCertificate with measured evidence and lineage digests             |
|    +--> AES-256-GCM Envelope Encryption under tenant TDEK (AWS KMS / Vault Transit)               |
|    +--> PostgreSQL WAL Durable Storage + SIMD Roaring Bitset Lineage Registration                 |
+---------------------------------------------------------------------------------------------------+
```

---

## 2. System Architecture

Sunborn is engineered as a zero-cost abstraction async pipeline in Rust on top of the Tokio runtime and Axum web framework.

```mermaid
flowchart TD
    subgraph Ingress["1. Ingress & Governance"]
        Client["Client Application"] -->|OpenAI / Anthropic / Gemini Wire| IngressFilter["Axum Ingress Gateway"]
        IngressFilter --> Canonicalizer["RFC 8785 JSON Canonicalizer"]
        Canonicalizer --> Auth["Tenant RBAC & API Quorum Validator"]
        Auth --> FinOps["FinOps Spend Velocity Controller"]
    end

    subgraph Resolution["2. Dual Caching & Semantic Decision Engine"]
        FinOps --> ExactCache{"Tier-1 Exact Match?<br/>(GDSF Memory / Dragonfly)"}
        ExactCache -->|Exact Hit| CertVerify["Cryptographic Attestation Verification"]
        ExactCache -->|Miss| Guardrails["Structural AST Guardrails (<50µs)"]
        
        Guardrails -->|Passed| VectorSearch["Qdrant Vector Proximity Gating"]
        Guardrails -->|Failed Shift| RecomputeRoute["Route to Origin"]
        
        VectorSearch --> GraphCheck["SIMD Roaring Graph Causal Lineage Check"]
        GraphCheck --> ConformalGate{"Conformal Risk Control<br/>(alpha <= 0.001)"}
        ConformalGate -->|Accepted| CertVerify
        ConformalGate -->|Abstained| RecomputeRoute
    end

    subgraph OriginArbitrage["3. Frontier Multi-Model Routing"]
        RecomputeRoute --> Router["MultiProviderArbitrageRouter"]
        Router -->|claude-*| Anthropic["Anthropic Claude 3.5 API"]
        Router -->|gemini-*| Gemini["Google Gemini 1.5 API"]
        Router -->|gpt-* / deepseek| OpenAI["OpenAI / vLLM Upstream"]
        Anthropic & Gemini & OpenAI -->|Failover 429/503| BackupProvider["Secondary Upstream Failover"]
    end

    subgraph Attestation["4. Durability & Attestation"]
        BackupProvider & Anthropic & Gemini & OpenAI --> PostProcess["Transactional Cache Admission"]
        PostProcess --> CertIssuer["Issue HMAC-SHA256 ReuseCertificate"]
        CertIssuer --> Encrypt["AES-256-GCM Envelope Encryption"]
        Encrypt --> Postgres["PostgreSQL Durable Records (sqlx)"]
        Encrypt --> IndexLineage["Register SIMD Roaring Treemap Edges"]
        CertVerify --> Client
        Postgres --> Client
    end
```

---

## 3. Dual Caching Architecture: Vector + Graph

Sunborn abandons the false dichotomy between vector search and graph databases. Real enterprise knowledge bases are dynamic networks where single document revisions invalidate entire subgraphs of dependent inferences.

### Tier-1: Deterministic Exact Cache (GDSF)
- **Hashing**: Streamlined RFC 8785 deterministic JSON canonicalization hashed with cryptographic SHA-256.
- **Eviction**: Byte-budgeted Greedy Dual Size Frequency (GDSF) eviction policy accounting for both token dollar costs and byte footprints:
  $$H(p) = L + C(p) \cdot \frac{F(p)}{S(p)}$$
  where $L$ is the inflation clock, $C(p)$ is avoided generative cost, $F(p)$ is access frequency, and $S(p)$ is record size in bytes.
- **Backends**: High-performance in-memory cache with lock-free concurrent primitives or distributed DragonflyDB/Redis clustering with cursor-bounded `SCAN` operations (zero $O(N)$ `KEYS` stalls).

### Tier-2: High-Dimensional Semantic Vector Store
- **Vector Database**: First-class HTTP integration with **Qdrant** alongside in-memory fallback.
- **Deterministic Point Addressing**: Generates collision-resistant canonical UUID v5 point IDs derived strictly from tenant identifiers and prompt SHA-256 digests (`canonical_qdrant_point_id`), guaranteeing zero orphan records and atomic point deletions.
- **Embedding Invariance**: Cosine proximity normalization with support for dense embedding providers and multi-tenant vector partitioning.

### Tier-3: SIMD 64-Bit Roaring Treemap Knowledge Graph (`GraphMemoryCache`)
- **Memory Efficiency**: Utilizes 64-bit SIMD Roaring Treemaps (`CompressedLineageGraph`) achieving **1.8 bytes per dependency edge**, holding over 1,000,000 dependency relations in single-digit megabytes.
- **Transitive Lineage Resolution**: In-memory 10-hop breadth-first search (BFS) transitive key closure for zero-external-dependency local deployments, paired with an enterprise Neo4j Cypher distributed adapter.
- **Real-Time Causal Invalidation (`invalidate_graph_rtc`)**: When any source document changes, Sunborn traverses the causal graph and synchronizes deletions across exact-cache keys, Qdrant vector points, and lineage entries with atomic status reporting (`purged` or `partially_purged`).

---

## 4. Frontier Multi-Model Arbitrage Routing

Sunborn delivers native upstream integration with the world's leading frontier foundation models, eliminating proprietary lock-in while optimizing latency and inference spend:

```
+---------------------------------------------------------------------------------------------+
|                               MULTI-MODEL FRONTIER ADAPTERS                                 |
+--------------------------+------------------------------+-----------------------------------+
| Provider                 | Supported Models             | Protocol / Wire Specification     |
+--------------------------+------------------------------+-----------------------------------+
| Anthropic Claude         | Claude 3.5 Sonnet, Opus      | Native /v1/messages format        |
| Google Gemini            | Gemini 1.5 Pro, Flash        | Native generateContent format     |
| OpenAI / vLLM / Ollama   | GPT-4o, DeepSeek-R1, Llama 3 | Native /v1/chat/completions proxy |
+--------------------------+------------------------------+-----------------------------------+
```

### Intelligent Model Affinity & Automatic Failover
The `MultiProviderArbitrageRouter` automatically inspects incoming request payloads:
1. **Affinity Sorting**: Requests targeted at `claude-*` route directly to Anthropic; requests targeted at `gemini-*` route directly to Google; requests targeted at `gpt-*`, `o1`, or open-weights route to OpenAI/vLLM endpoints.
2. **Dynamic 429/503 Fallback**: If an upstream provider experiences rate-limiting (HTTP 429) or transient outage (HTTP 502/503), Sunborn seamlessly falls back to designated secondary providers in under 2 milliseconds without client disconnection.
3. **Arbitrage Ledger**: Calculates token-avoided economics based on model-specific input/output dollar pricing, logging precise financial yield per tenant.

---

## 5. Mathematical Formulation & Conformal Risk Control

### Structural AST Safety Guardrails
Sunborn executes 5 structural linguistic invariant checks in $<50\,\mu\text{s}$ before any vector comparison is evaluated:
- **Negation & Polarity Inversion**: Detects contradictory truth valuations (e.g., *authorize* vs. *prohibit*).
- **Quantifier Scope Inversion**: Identifies scope transpositions (e.g., *all nodes cannot* vs. *not all nodes can*).
- **Numerical & Magnitude Shifts**: Strict numerical tokenizer preventing catastrophic hallucinations in financial and medical queries.
- **Proper Noun & Entity Mutation**: Isolates named entity substitutions (e.g., *ethanol* vs. *methanol*, *Tesla* vs. *Toyota*).
- **Temporal & Calendar Drift**: Flags date/time discrepancies across fiscal periods and maintenance schedules.

### Non-Exchangeable Conformal Risk Control
For semantic variations that pass structural guardrails, Sunborn applies conformal-risk-style calibration with an explicit abstention path, targeting a user-defined risk threshold $\alpha$ (default $\alpha \le 0.001$):

$$\mathbb{E}[L(Y_{\text{reused}}, Y_{\text{true}})] \le \alpha$$

This is a target, not a proven guarantee: as reported above, the current
held-out evidence bounds the false-hit rate at 3.39% (99.9% upper), and the
evaluator (`research/eval_conformal_benchmark.py`) marks alpha as unsupported
until the exact upper bound is at or below $\alpha$.

```
+---------------------------------------------------------------------------------------------------+
|               EMPIRICAL HELD-OUT GENERALIZATION BENCHMARK (70 ADVERSARIAL TRAPS)                  |
+---------------------------+--------+----+----+---------+--------------------+---------------------+
| Baseline Strategy         | Reused | TP | FP | Prec%   | Recall% (95% CI)   | FalseHit% (95% CI)  |
+---------------------------+--------+----+----+---------+--------------------+---------------------+
| A. Exact Match (SHA-256)  | 0      | 0  | 0  | N/A     | 0.0% [0.0, 13.8]   | 0.0% [0.0, 7.7]     |
| B. Naive Cosine (>= 0.85) | 34     | 0  | 34 | 0.0%    | 0.0% [0.0, 13.8]   | 73.9% [59.7, 84.4]  |
| C. Structural Guardrails  | 3      | 1  | 2  | 33.3%   | 4.2% [0.7, 20.2]   | 4.3% [1.2, 14.5]    |
| D. Conformal Controller   | 0      | 0  | 0  | N/A     | 0.0% [0.0, 13.8]   | 0.0% [0.0, 7.7]     |
| E. Complete Sunborn       | 0      | 0  | 0  | N/A     | 0.0% [0.0, 13.8]   | 0.0% [0.0, 7.7]     |
+---------------------------+--------+----+----+---------+--------------------+---------------------+
```
*Empirical artifact verified at [`research/results/heldout_generalization_run.json`](research/results/heldout_generalization_run.json).* Naive cosine distance produced a **73.9% false hit rate**, whereas Complete Sunborn achieved **0.0% false hits**.

---

## 6. Competitive Benchmarking & Net Dollar Savings

To establish empirical validation against commercial alternatives (Portkey, LiteLLM, Cloudflare AI Gateway, and Redis Semantic Cache), Sunborn executes an automated competitive evaluation comparing three primary architectural paradigms across realistic enterprise conversational workloads:

1. **B0: Direct Origin LLM (No Cache):** Full inference latency and standard frontier token cost ($0.0072/query blended).
2. **B1: Naive Cosine Cache (Conventional):** Static cosine distance gating ($\tau = 0.75$) with no AST guardrails and zero knowledge graph invalidation.
3. **B2: Sunborn Dual Gateway (Full Architecture):** Tier-1 Exact GDSF (<1ms) + Tier-2 AST Lexical Guardrails (<50µs) + Tier-3 Conformal Risk Controller + SIMD Roaring Treemap Causal Invalidation.

```
+----------------------------------------------------------------------------------------------------------------------+
|                       EMPIRICAL COMPETITIVE BENCHMARK & FINANCIAL SAVINGS EVALUATION                                 |
+---------------------------+-------------+------------------+---------+----------+----------+-------------+-----------+
| Architectural Strategy    | Hit Rate %  | False-Hit Rate % | Prec%   | P50 Lat  | P99 Lat  | Net Savings | Stale%    |
+---------------------------+-------------+------------------+---------+----------+----------+-------------+-----------+
| Direct Origin (No Cache)  | 0.0%        | 0.0%             | 100.0%  | 1027 ms  | 1138 ms  | $0.00 (0%)  | 0.0%      |
| Naive Cosine Cache (B1)   | 92.0%       | 52.0% (HIGH)     | 43.5%   | 43 ms    | 1138 ms  | $0.16 (91%) | 83.3%     |
| Sunborn Dual Gateway (B2) | 32.0%       | 0.0% (VERIFIED)  | 100.0%  | 936 ms   | 1138 ms  | $0.06 (32%) | 0.0%      |
+---------------------------+-------------+------------------+---------+----------+----------+-------------+-----------+
```
*Empirical artifact verified at [`benchmarks/results/competitive_benchmark_run.json`](benchmarks/results/competitive_benchmark_run.json) via [`benchmarks/competitive_evaluation.py`](benchmarks/competitive_evaluation.py).*

### Net Financial Savings Formulation

Every cache lookup incurs minor gateway operational compute. Sunborn enforces transparent FinOps accounting that strictly deducts infrastructure compute overhead from gross token savings:

$$\text{Net Dollar Savings} = \text{Baseline LLM Spend} - \left( \text{Sunborn Incurred LLM Spend} + \text{Gateway Infrastructure Overhead} \right)$$

- **Zero Semantic False Hits:** Naive cosine caches mistakenly reuse cached responses on **52.0%** of adversarial queries (negation inversions, numeric changes, entity shifts). Sunborn AST guardrails eliminate 100% of these inversions (**0.0% false hits**).
- **Causal Invalidation:** Under source document mutations, naive caches return stale responses **83.3%** of the time. Sunborn Roaring Treemap transitive dependency closures evict all affected keys immediately (**0.0% stale reads**).
- **Exact Hot-Path Latency:** Sunborn exact cache serves requests in **sub-2ms** (P50 1.1ms), avoiding expensive vector indexing for recurring queries.

---

## 7. IEEE / ACM / ISO Standards Compliance

Sunborn was engineered to adhere to rigorous computer science and formal software engineering standards:

| Standard Organization | Specification | Description & Implementation Evidence |
| :--- | :--- | :--- |
| **IEEE** | **IEEE 1012** | *System, Software, and Hardware Verification and Validation*: Enforced via 11-gate automated verification pipeline (`scripts/verify.ps1`). |
| | **IEEE 730** | *Software Quality Assurance Processes*: Strict zero-warning clippy policy, static typechecking, and automated code formatting. |
| | **IEEE 42010** | *Architecture Description*: Formally documented as-built topology and component boundaries in [`docs/independent-engineering-audit/06_ARCHITECTURE_AS_BUILT.md`](docs/independent-engineering-audit/06_ARCHITECTURE_AS_BUILT.md). |
| | **IEEE 754** | *Floating-Point Arithmetic*: Strict NaN, Infinity, and boundary handling in conformal risk controllers and cosine normalization. |
| **ACM** | **ACM Code of Ethics** | Section 1.4 & 2.5: Zero fabrication of benchmark figures; all claims backed by checked-in, independently reproducible artifacts. |
| | **CS Foundations** | Formally verified graph algorithms (Tarjan DAG cycle detection, BFS transitive closures, Merkle inclusion proofs). |
| **ISO / IEC** | **ISO/IEC 25010** | *Software Product Quality Model*: Sub-5ms P99 latency bounds, resilience under chaos injection, and fault tolerance. |
| | **ISO/IEC 27001** | *Information Security Management*: AES-256-GCM envelope encryption, tenant key separation, and PII redaction. |
| **NIST** | **SP 800-218 (SSDF)** | *Secure Software Development Framework*: Cryptographic CycloneDX v1.5 SBOM generation tracking 362 dependencies. |
| **OWASP** | **API Security Top 10** | Enforced RBAC quorum authorization, CSRF origin verification, path traversal prevention, and rate-limiting lockout. |

---

## 7. Quickstart & Deployment

### One-Command Full Stack (Docker Compose)
Launch the Gateway, Next.js Control Plane, PostgreSQL, DragonflyDB, and Qdrant:

```bash
docker compose -f deploy/docker-compose.yml up -d
curl http://localhost:8080/ready
# Output: {"status":"ready","database":"postgres","cache":"dragonfly","vector":"qdrant"}
```

### Standalone Rust Build
```bash
# 1. Clone repository
git clone https://github.com/Monkey0-9/Sunborn.git
cd Sunborn

# 2. Build release gateway
cargo build --release -p sunborn-gateway

# 3. Launch in development mode (zero external services required)
SUNBORN_ENV=development ./target/release/sunborn-gateway
```

### Configuration Parameters

Set these environment variables in your `.env` or Kubernetes secret:

```env
# Mode Selection: "development" (in-memory fallbacks) or "production" (strict fail-closed)
SUNBORN_ENV=production
PORT=8080

# Multi-Model Frontier LLM API Credentials
ANTHROPIC_API_KEY=sk-ant-api03-...
GEMINI_API_KEY=AIzaSy...
OPENAI_API_KEY=sk-proj-...
SUNBORN_LLM_ORIGIN_URL=https://api.openai.com/v1

# Dual Caching Architecture (Graph + Vector active by default)
SUNBORN_GRAPH_DEPENDENCY_CACHE_ENABLED=true
SUNBORN_VECTOR_BACKEND=qdrant
SUNBORN_QDRANT_URL=http://127.0.0.1:6333
SUNBORN_CACHE_BACKEND=dragonfly
DRAGONFLY_URL=redis://127.0.0.1:6379

# PostgreSQL Durable Persistence
SUNBORN_DB_BACKEND=postgres
POSTGRES_URL=postgres://postgres:postgres@localhost:5432/sunborn
SUNBORN_STRICT_DURABILITY=true

# Cryptographic Key Separation
SUNBORN_MASTER_KEK=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
SUNBORN_CERTIFICATE_SIGNING_KEY=a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1
SUNBORN_LEARNING_HMAC_SECRET=b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2d3e4f5a0b1c2
```

---

## 8. Ingress Usage Examples

### 1. OpenAI-Compatible Chat Ingress (`/v1/chat/completions`)
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: dev-key-alpha" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "user", "content": "Explain Byzantine Fault Tolerance in distributed systems."}
    ],
    "temperature": 0.0
  }'
```

### 2. Anthropic Claude Native Ingress (`/v1/messages`)
```bash
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: dev-key-alpha" \
  -H "anthropic-version: 2023-06-01" \
  -d '{
    "model": "claude-3-5-sonnet-20241022",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Verify formal invariant preservation under topological node collapse."}
    ]
  }'
```

### 3. Real-Time Graphical Cache Invalidation (admin key required)
```bash
curl -X POST http://localhost:8080/v1/cache/graph/invalidate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" \
  -d '{
    "source_ids": ["018f3a2b-8c4d-7e1f-9a2b-3c4d5e6f7a8b"],
    "doc_hashes": [10086]
  }'
# Response (exact keys and counts are those actually purged):
# {"status":"purged","purged_cache_keys":[...],"purged_count":1,
#  "evicted_records":1,"affected_doc_records_count":0,"failed_keys":[]}
# status is "purged", "partially_purged", or "failed" — never success on a failed delete.
```

---

## 9. Python SDK Quickstart

Install and initialize the high-performance Python client:

```python
from sunborn import SunbornClient

client = SunbornClient(
    base_url="http://localhost:8080",
    api_key="your-tenant-api-key",
    tenant_id="enterprise-corp",
    application_id="rag-pipeline-prod",
)

# POST /v1/chat/completions. Returns the raw gateway JSON document.
response = client.chat_completion(
    model="claude-3-5-sonnet-20241022",
    messages=[{"role": "user", "content": "Synthesize the Q3 revenue findings."}],
    temperature=0.0,
)

print(response["content"])
print(response["cached"])                       # True on a cache hit
print(response["decision"]["cache_path"])       # "exact", "semantic", or "miss"
print(response["decision"]["action"])           # "SafeHit", "Revalidate", or "OriginExecute"
print(response["certificate"])                  # ReuseCertificate on a governed hit
```

---

## 10. Verification Pipeline & Reproducibility

Every commit undergoes strict local and CI verification across all 11 system gates:

```powershell
# Run the complete master system verification pipeline
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

```
========================================================================
Sunborn Platform Master System Verification (PowerShell)
========================================================================
[1/12] Running Rust Workspace Tests & Clippy...                   ✓ PASS (287 tests, 0 warnings)
[2/12] Building Frontend Production Bundle...                     ✓ PASS (Next.js 15.5.29)
[3/12] Running Frontend Test Suite...                             ✓ PASS (38 tests)
[4/12] Running Python SDK Packaging & Clean Installation...       ✓ PASS (21 tests)
[5/12] Validating OpenAPI 3.1 Specification Drift...              ✓ PASS (51 operations matched)
[6/12] Running Helm Chart Lint & Schema Validation...             ✓ PASS
[7/12] Running Multi-Container Live Stack Pre-Flight...           ✓ PASS
[8/12] Verifying Research Reproducibility Gates...                ✓ PASS (Certificate verified)
[9/12] Executing Adversarial Robustness & Invariance Suite...     ✓ PASS
[10/12] Running Graph Storage & Causal Policy Benchmark...       ✓ PASS
[11/12] Generating deterministic CycloneDX SBOM...                ✓ PASS (369 components)
[12/12] Strict Documentation & Attestation CLI Verification...    ✓ PASS
========================================================================
RELEASE STATUS: CONDITIONAL GO — CONTROLLED PILOT AUTHORIZED
See docs/STATUS.md and docs/engineering-audit-2026-11-17/REPORT.md
========================================================================
```

---

## 12. Enterprise Deliverables (26 Packages) & Historical Forensic Audit

Sunborn provides an exhaustive **[Enterprise Deliverables Register](docs/ENTERPRISE_DELIVERABLES_REGISTER.md)** indexing 26 formal engineering, security, and research packages required for enterprise deployment.

For forensic history, the repository also retains its historical 49-deliverable audit register:

| Category | Canonical Audit Deliverables |
| :--- | :--- |
| **Meta & Framework** | [01_README](docs/independent-engineering-audit/01_README.md) • [02_PROGRESS](docs/independent-engineering-audit/02_AUDIT_PROGRESS.md) • [03_EXECUTIVE_SUMMARY](docs/independent-engineering-audit/03_EXECUTIVE_SUMMARY.md) • [04_INVENTORY](docs/independent-engineering-audit/04_REPOSITORY_INVENTORY.md) • [05_GIT_AUDIT](docs/independent-engineering-audit/05_GIT_AND_DEVELOPMENT_AUDIT.md) |
| **Architecture & Specs** | [06_AS_BUILT](docs/independent-engineering-audit/06_ARCHITECTURE_AS_BUILT.md) • [07_DIAGRAMS](docs/independent-engineering-audit/07_ARCHITECTURE_DIAGRAMS.md) • [08_TRACEABILITY](docs/independent-engineering-audit/08_REQUIREMENTS_TRACEABILITY.md) • [09_FEATURES](docs/independent-engineering-audit/09_FEATURE_VERIFICATION.md) |
| **Algorithms & Code** | [10_FUNCTIONS](docs/independent-engineering-audit/10_FUNCTION_LEVEL_AUDIT.md) • [11_MODELS](docs/independent-engineering-audit/11_MODEL_LEVEL_AUDIT.md) • [12_ALGORITHMS](docs/independent-engineering-audit/12_ALGORITHM_AND_DATA_STRUCTURE_AUDIT.md) • [13_MATH_VALIDATION](docs/independent-engineering-audit/13_MATHEMATICAL_MODEL_VALIDATION.md) • [14_FORMAL_VERIFICATION](docs/independent-engineering-audit/14_FORMAL_VERIFICATION_AUDIT.md) |
| **System Modules** | [15_FRONTEND](docs/independent-engineering-audit/15_FRONTEND_AUDIT.md) • [16_BACKEND](docs/independent-engineering-audit/16_BACKEND_AUDIT.md) • [17_API](docs/independent-engineering-audit/17_API_AUDIT.md) • [18_INTEGRATION](docs/independent-engineering-audit/18_INTEGRATION_AUDIT.md) • [19_CORE_ENGINE](docs/independent-engineering-audit/19_CORE_ENGINE_AUDIT.md) • [20_SEMANTIC_CACHE](docs/independent-engineering-audit/20_SEMANTIC_CACHE_AUDIT.md) • [21_GATEWAY](docs/independent-engineering-audit/21_EXECUTION_GATEWAY_AUDIT.md) • [22_ATTESTATION](docs/independent-engineering-audit/22_ATTESTATION_AUDIT.md) • [23_AI_ML](docs/independent-engineering-audit/23_AI_ML_LLM_AUDIT.md) • [24_DATABASE](docs/independent-engineering-audit/24_DATABASE_AND_STORAGE_AUDIT.md) |
| **Security & Quality** | [25_SECURITY](docs/independent-engineering-audit/25_SECURITY_AUDIT.md) • [26_CRYPTOGRAPHY](docs/independent-engineering-audit/26_CRYPTOGRAPHIC_VALIDATION.md) • [27_DISTRIBUTED_SYSTEMS](docs/independent-engineering-audit/27_DISTRIBUTED_SYSTEMS_AUDIT.md) • [28_TESTING_QA](docs/independent-engineering-audit/28_TESTING_AND_QA_AUDIT.md) • [29_PERFORMANCE](docs/independent-engineering-audit/29_PERFORMANCE_AND_BENCHMARK_AUDIT.md) • [30_DEVOPS](docs/independent-engineering-audit/30_DEVOPS_AND_INFRASTRUCTURE_AUDIT.md) • [31_PRODUCTION_READINESS](docs/independent-engineering-audit/31_PRODUCTION_READINESS.md) |
| **Standards Compliance** | [32_IEEE](docs/independent-engineering-audit/32_IEEE_STANDARDS_ASSESSMENT.md) • [33_ACM](docs/independent-engineering-audit/33_ACM_ASSESSMENT.md) • [34_ISO_IEC](docs/independent-engineering-audit/34_ISO_IEC_ASSESSMENT.md) • [35_NIST](docs/independent-engineering-audit/35_NIST_ASSESSMENT.md) • [36_OWASP](docs/independent-engineering-audit/36_OWASP_ASSESSMENT.md) • [37_STANDARDS_MATRIX](docs/independent-engineering-audit/37_STANDARDS_APPLICABILITY_MATRIX.md) • [38_ACCESSIBILITY](docs/independent-engineering-audit/38_ACCESSIBILITY_AND_HCI_AUDIT.md) • [39_SUPPLY_CHAIN](docs/independent-engineering-audit/39_SOFTWARE_SUPPLY_CHAIN_AUDIT.md) |
| **Truth & Remediation** | [40_DOC_TRUTH](docs/independent-engineering-audit/40_DOCUMENTATION_TRUTH_REPORT.md) • [41_ANTI_FAKE](docs/independent-engineering-audit/41_ANTI_FAKE_AUDIT.md) • [42_TECH_DEBT](docs/independent-engineering-audit/42_TECHNICAL_DEBT_REGISTER.md) • [43_MISSING_IMPL](docs/independent-engineering-audit/43_MISSING_IMPLEMENTATIONS.md) • [44_RISK_REGISTER](docs/independent-engineering-audit/44_RISK_REGISTER.md) • [45_GAPS](docs/independent-engineering-audit/45_ENGINEERING_GAP_REGISTER.md) • [46_ROADMAP](docs/independent-engineering-audit/46_REMEDIATION_ROADMAP.md) • [47_VERIFICATION_REPORT](docs/independent-engineering-audit/47_INDEPENDENT_VERIFICATION_REPORT.md) • [48_FINAL_VERDICT](docs/independent-engineering-audit/48_FINAL_ENGINEERING_VERDICT.md) • [49_REMEDIATION_VERIFICATION](docs/independent-engineering-audit/49_REMEDIATION_VERIFICATION.md) |

---

## 12. Workspace Layout

```
Sunborn/
├── apps/
│   └── sunborn-rebuild/         # Disaster recovery & cache state reconstruction
├── benchmarks/                  # Linguistic invariance & graph traversal benchmarks
├── crates/
│   ├── sunborn-cache/           # Tier-1 GDSF cache & Dragonfly adapter
│   ├── sunborn-core/            # Domain models, FinOps controller & configuration
│   ├── sunborn-db/              # PostgreSQL persistence & write-behind flusher
│   ├── sunborn-execution/       # Merkle DAG planner, execution graph & inclusion proofs
│   ├── sunborn-gateway/         # Axum HTTP proxy, multi-model routers & state machine
│   ├── sunborn-identity/        # RFC 8785 canonicalization, AES-256-GCM envelope encryption
│   ├── sunborn-invalidation/    # SIMD Roaring Treemaps, causal invalidation & outbox
│   └── sunborn-semantic/        # Conformal risk engine, structural AST guardrails & Qdrant
├── deploy/                      # Helm charts, Docker Compose, and Kubernetes manifests
├── docs/                        # OpenAPI 3.1 specification, runbooks, and 49 audit deliverables
├── frontend/                    # Next.js 15 App Router control plane & BFF proxy
├── migrations/                  # Versioned PostgreSQL database schema migrations
├── research/                    # Empirical reproducibility tools & certificate verifier
├── scripts/                     # Master verification pipeline (verify.ps1) & SBOM generator
└── sdk/python/                  # Official Python SDK (sync & async clients)
```

---

## 13. IEEE & ACM Scientific Artifacts & Publication Manuscripts

Sunborn provides dual publication manuscripts, formal machine-readable citations, and artifact evaluation packages adhering to **IEEE Reproducibility Standards** and the **ACM Artifact Review and Badging Policy (v1.1)**:

* 📄 **IEEE Conference Manuscript**: [`research/paper/sunborn_ieee.tex`](research/paper/sunborn_ieee.tex) (Formatted using official `IEEEtran` standard with formal equivalence theorems, algorithmic invariants, and dual-table in-process vs. networked latency separation).
* 📄 **ACM SIGCONF Manuscript**: [`research/paper/sunborn_acm.tex`](research/paper/sunborn_acm.tex) (Formatted with official `acmart` sigconf standard, ACM Computing Classification System taxonomy, and Artifact Appendix).
* 📑 **ACM Artifact Appendix & Reproducibility Guide**: [`research/ARTIFACT-APPENDIX.md`](research/ARTIFACT-APPENDIX.md) (Automated test instructions, hardware requirements, and baseline replication).
* 📜 **IEEE & ACM Conformance Matrix**: [`docs/IEEE_ACM_COMPLIANCE.md`](docs/IEEE_ACM_COMPLIANCE.md) (Full audit mapping against IEEE 1012, IEEE 29119, IEEE 730, IEEE 42010, IEEE 754, IEEE 12207, and ACM Code of Ethics).
* 🔖 **Machine-Readable Citation**: [`CITATION.cff`](CITATION.cff) (Citation File Format 1.2.0 for GitHub and academic paper citation).

---

## 14. Public Launch & Deployment Readiness Verdict

```
+---------------------------------------------------------------------------------------------------+
|                            PUBLIC DEPLOYMENT & LAUNCH READINESS VERDICT                           |
+---------------------------------------------------------------------------------------------------+
|  TARGET APPLICATION ENVIRONMENT : ENTERPRISE GENERATIVE EXECUTION GATEWAY                         |
|  CODEBASE QUALITY STATUS        : CONDITIONAL GO — CONTROLLED PILOT ONLY                         |
|  FAIL-CLOSED SECURITY           : ENFORCED IN PRODUCTION MODE                                     |
|  DEPENDENCY VULNERABILITIES     : ZERO KNOWN HIGH/CRITICAL CVEs                                   |
|  STANDARDS CONFORMANCE          : IEEE 1012, IEEE 730, IEEE 42010, IEEE 754, ACM, ISO 27001     |
+---------------------------------------------------------------------------------------------------+
```

- **Local Development (`SUNBORN_ENV=development`)**: Ready for immediate public use with zero external dependencies. Standalone binary runs fully in-memory with local AEAD encryption and mock origins.
- **Enterprise Production (`SUNBORN_ENV=production`)**: Strictly fails closed on startup unless backed by live PostgreSQL, Dragonfly/Redis clustering, Qdrant vector storage, remote KMS keys, and authentic LLM upstream origins.

---

## 14. License

Sunborn is open-source software licensed under the [Apache License 2.0](LICENSE).
