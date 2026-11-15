# 03 — Executive Summary

**Verdict:** functional prototype / early-stage product (~5.1/10 maturity). The execution gateway, Ed25519/RFC 8785 attestation, and SIMD Roaring bitmap graph are real engineering; the semantic-safety narrative is improving (measured evidence, ambiguity abstention, server-derived epochs) but distributed causal consistency across replicas remains unwired. **Production: NO-GO.**

> **Superseding Note (2026-11-14, baseline `e8edc4b` + working tree):** See **[`50_MASTER_EVIDENCE_REGISTER.md`](50_MASTER_EVIDENCE_REGISTER.md)** for the authoritative item-by-item verification ledger. Invalidation partial-purge (D-04) and pre-fencing (D-10) are verified closed; Neo4j traversal is unified to 10 hops (D-06); multi-provider Grok affinity is integrated; single-migration authority is landed (P0-1); and total passing automated tests stand at **351 passed, 0 failed**.

---

## What Sunborn actually is

A high-performance Rust/Axum reverse proxy in front of LLM providers, with a three-tier cache architecture (exact, semantic, execution-DAG), cryptographic attestation/certificate layer (RFC 8785 canonicalization + Ed25519 verification), FinOps spend-velocity controllers, and a Next.js control plane. The gateway — routing, OpenAI/Anthropic/Gemini/Grok translation, auth/RBAC, spend reservations, graceful shutdown, ~33.7k LOC across crates and apps — is substantive and the strongest asset in the repository.

## What is real

- **Gateway**: 49 registered routes, protocol translation, hashed API credentials, model RBAC, dual-admin quorum, spend-velocity reservations with settle/release on every path, production fail-closed readiness, bounded graceful shutdown.
- **Multi-Provider Arbitrage Router**: OpenAI, Anthropic, Gemini, and Grok endpoints with sub-millisecond failover cascade and model affinity.
- **PostgreSQL persistence**: real `sqlx`, authoritative singular migration runner (`_sunborn_migrations` tracking SHA-256 checksums across `0001` through `0004`), parameter binds, circuit breaker.
- **Crypto & Attestation**: real AES-256-GCM envelope encryption, per-tenant TDEKs, O(1) zeroizing key shred; RFC 8785 canonicalization; Ed25519 attestation signing and zero-trust verification (`sunborn-verify`); genuine remote Vault Transit and AWS KMS adapters exist.
- **Execution Engine**: Merkle DAG hashing, RFC-6962-style inclusion proofs, Kahn topological planner with purity/effect gating and transitive lineage.
- **Invalidation**: Roaring-bitmap lineage with SIMD bitwise unions (100k Roaring containers), 3-phase `set_with_lineage` with rollback, watermark write-back fencer, GDSF byte-budget eviction, graph memory cache (default on), and partial-purge error preservation (D-04).
- **Tests (executed 2026-11-14): 263 Rust, 37 frontend, 26 Python SDK, 25 research verifier — 351 passed, 0 failed.**

## What is not real (the hard part)

- **Default runtime remains in-memory unless configured.** Embeddings default to FNV-1a LSH hashes; DB/cache/vector default to in-memory unless PostgreSQL/Redis/Qdrant are provisioned (`config.rs`, `state.rs`).
- **Distributed cluster coordination is local tokio broadcast.** `DistributedGraphSyncEngine` uses `InMemoryClusterTransport` (tokio broadcast within a single process), not distributed Redis Streams or Raft across independent nodes.
- **Conformal calibration lacks production dataset.** Seeded thresholds (`alpha <= 0.001`); rigorous non-exchangeable calibrator exists in code but lacks committed production calibration artifacts.
- **Absence of neural cross-encoder NLI backend.** Currently uses structural entailment heuristics and cosine probability tables; ONNX/neural DeBERTa backend is not wired.
- **Public documentation contains legacy modeled figures.** `571,428 req/s`, `1.8 B/edge`, and earlier theoretical claims in marketing copy exceed empirical measurement.

## Numbers (with denominators)

| Metric | Value | Basis |
|---|---|---|
| Tracked files | 382 | `git ls-files` |
| Rust LOC | ~33,704 | line census, 97 `.rs` files across crates/apps |
| Rust tests (executed) | **263 passed, 0 failed** | `cargo test --workspace --all-targets` (37 binaries) |
| Frontend tests (executed) | **37 passed, 0 failed** | `npm --prefix frontend test` |
| Python SDK tests (executed) | **26 passed, 0 failed** | `python -m unittest sdk/python/...` |
| Research verifier tests | **25 passed, 0 failed** | `research/tests/test_verifier_negative.py` |
| Total executed automated tests | **351 passed / 0 failed** | **100% Green across all 4 test runtimes** |
| Verified-working claimed capabilities | ~49% (22/45) | updated component register in doc 50 |
| P0 findings (baseline) | 10 (7 security/attestation + 3 benchmark/math) | 25, 41, inventories/findings.csv |
| P0 closed post-audit | 8 (F-001/002/003/006/007, D-04, D-10, P0-1) | 49_REMEDIATION_VERIFICATION.md & doc 50 |

## Bottom line

Sunborn has elevated from an early prototype to a functional, highly robust gateway with provable cryptographic primitives and SIMD-accelerated invalidation. It does **not** yet qualify as production-ready because cross-replica cluster sync, durable epoch authority, and real-model conformal calibration datasets are pending. The systematic remediation path forward is codified in **[`52_MASTER_REMEDIATION_PLAN.md`](52_MASTER_REMEDIATION_PLAN.md)**.

