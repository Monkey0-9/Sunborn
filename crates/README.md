# Sunborn Workspace Crates (`crates/`)

Modular Rust crates forming the core architecture of the Sunborn Trustworthy Execution-Reuse Infrastructure.

## Architecture & Crates

| Crate | Purpose |
|---|---|
| [`sunborn-core`](./sunborn-core/) | Core domain models, FinOps utility scoring, tenancy identifiers, and errors. |
| [`sunborn-identity`](./sunborn-identity/) | Multi-tenant cryptographic identity, RFC 8785 canonicalization, inline PII tokenization, and O(1) AEAD tenant crypto-shredding. |
| [`sunborn-cache`](./sunborn-cache/) | Tiered L1 in-memory and L2 distributed cache with FinOps GDSF eviction policies. |
| [`sunborn-gateway`](./sunborn-gateway/) | High-throughput asynchronous Axum HTTP reverse proxy and multi-tenant router. |
| [`sunborn-semantic`](./sunborn-semantic/) | Non-Exchangeable Conformal Risk Control, drift tracking, and linguistic guardrails. |
| [`sunborn-invalidation`](./sunborn-invalidation/) | Transitive dependency graph invalidation and transactional outbox event engine. |
| [`sunborn-db`](./sunborn-db/) | Authoritative PostgreSQL durability engine, write-behind buffers, and audit log persistence. |
| [`sunborn-execution`](./sunborn-execution/) | Deterministic Merkle DAG execution planner and sub-graph step reuse engine. |
| [`sunborn-proof`](./sunborn-proof/) | Proof-Carrying Semantic Reuse (PCSR) certificate and zero-trust independent verification. |
| [`sunborn-freshness`](./sunborn-freshness/) | Multi-dimensional causal freshness and temporal validity engine. |
| [`sunborn-effects`](./sunborn-effects/) | Effect-aware agentic partial execution reuse and sub-DAG memoization. |
| [`sunborn-threat`](./sunborn-threat/) | Adversarial cache security, threat intelligence, and poisoning defense. |
| [`sunborn-economics`](./sunborn-economics/) | Real FinOps economic accounting, net utility optimization, and multi-provider pricing. |
| [`sunborn-learning`](./sunborn-learning/) | Safe continuous calibration, dual-shadow feedback, and drift monitoring. |

## Build & Test

```bash
cargo check --workspace
cargo test --workspace
```
