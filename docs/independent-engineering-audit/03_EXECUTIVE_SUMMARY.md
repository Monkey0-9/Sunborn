# 03 — Executive Summary

**Verdict:** functional prototype / early-stage product. The execution gateway is real engineering; the semantic-safety narrative is largely unverified, and the default runtime is a simulation. **Production: NO-GO.**

---

## What Sunborn actually is

A Rust/Axum reverse proxy in front of LLM providers, with a three-tier cache story (exact, semantic, execution-DAG), an attestation/certificate layer, FinOps governance, and a Next.js control plane. The gateway — routing, OpenAI/Anthropic translation, auth/RBAC, spend reservations, graceful shutdown, ~16.4k LOC including tests — is substantive and the strongest asset in the repo.

## What is real

- **Gateway**: 40 routes, protocol translation, hashed API credentials, model RBAC, dual-admin quorum, spend-velocity reservations with settle/release on every path, production fail-closed readiness, bounded graceful shutdown.
- **PostgreSQL persistence**: real `sqlx`, real DDL (15 tables), parameter binds, circuit breaker.
- **Crypto**: real AES-256-GCM envelope encryption, per-tenant TDEKs, O(1) zeroizing key shred; genuine remote Vault Transit and AWS KMS adapters exist and are selectable.
- **Execution**: Merkle DAG hashing, RFC-6962-style inclusion proofs, Kahn topological planner with purity/effect gating.
- **Invalidation**: Roaring-bitmap lineage, 3-phase `set_with_lineage` with rollback, watermark fencer, GDSF byte-budget eviction, Redis and Qdrant adapters (behind switches).
- **Tests (executed 2026-11-13): 252 Rust, 37 frontend, 26 Python — 315 passed, 0 failed.**

## What is not real (the hard part)

- **Default runtime is simulated.** `MockOriginProvider` returns a formatted string; embeddings default to FNV-1a LSH hashes; DB/cache/vector default to in-memory; `vector_backend="memory"`, `database_backend="memory"`, graph cache disabled (`config.rs:44-50`, `state.rs:86-142`, `state.rs:298`).
- **The proof layer self-attests.** The admission certificate hardcodes `structural_hash_match=true`, `negation_polarity_match=true`, `numeric_literal_match=true`, `entity_drift_detected=false`, `calibrated_risk_bound=0.25` (`handlers.rs:658-666`); the verifier then checks exactly those flags (`certificate.rs:454-462`). Freshness epochs come from the client request body (`handlers.rs:657,671`).
- **Several headline techniques are absent or renamed:** no HNSW client (brute-force cosine; Qdrant optional), no W-TinyLFU (GDSF), no AST parser (hardcoded token vocabularies), no cross-encoder NLI implementation (cosine→probability table), no PostgreSQL WAL semantics (plain OLTP). The rigorous conformal calibrator is explicitly labelled out-of-band research.
- **A fake backend ships in the frontend.** `standalone-simulator.ts` (698 LOC) is the default for `npm run dev`; it fabricates cluster stats, FinOps savings, Merkle roots (`is_valid:true`), credentials, and decides cache hits by prompt character parity. Dev login accepts any credential and can persist the plaintext secret as the session token. `security/page.tsx` mints `Math.random()` API keys on failure.
- **Public performance/safety numbers are unsupported or fabricated.** `571,428 req/s` cites a file that does not exist; `3,257 req/s` "socket stack" uses `oneshot` with no socket; `2.15 MB / 1.8 B/edge` is the literal formula `edges*1.8`; "0.0% false hits" is vacuous because the winning baseline reused **0 of 39** traps.
- **Docs are four contradictory suites** (GA-approved vs RELEASE HOLD), with colliding claim IDs, a phantom crate, wrong endpoint/test counts, and dangling references.
- **Committed junk**: root `openapi.yaml` is a 21-byte pointer, `sbom.cyclonedx.json` is non-deterministic, `research/results/baseline_backup/` duplicates artifacts, caches are unignored.

## Numbers (with denominators)

| Metric | Value | Basis |
|---|---|---|
| Tracked files | 382 | `git ls-files` |
| Rust LOC | ~32,048 | line census, 97 `.rs` files |
| Rust tests (executed) | 252 passed | `cargo test --workspace --all-targets` |
| Frontend tests (executed) | 37 passed | `npm test` |
| Python tests (executed) | 26 passed | `unittest`, 1.534s |
| Total executed | 315 passed / 0 failed | all suites |
| Verified-working claimed capabilities | 40% (18/45) | see 09/48 methodology |
| Simulated/mocked by default | yes | mock LLM, LSH embeddings, in-memory stores |
| P0 findings (baseline) | 10 (7 security/attestation + 3 benchmark/math) | 25, 41, inventories/findings.csv |
| P0 closed post-audit | 6 (F-001/002/003/006/007 + claims) | 49_REMEDIATION_VERIFICATION.md |

## Bottom line

Sunborn has a credible gateway and a genuinely interesting thesis (safe semantic reuse with verifiable freshness). It does **not** yet have the implementation its README describes. The path forward is: **(1) make every claim true, (2) make the safety layer measure what it attests, (3) wire or delete the durable/distributed components, (4) publish real benchmarks, (5) harden the frontend and CI.** That is exactly the remediation roadmap in `46_REMEDIATION_ROADMAP.md`.
