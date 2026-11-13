# 19 — Core Engine Audit

The "engine" = semantic cache + execution gateway + attestation + invalidation. Detailed deep-dives are in 20–22; this is the integrated verdict.

---

## 1. Capability reality table

| Capability | Real? | On default runtime path? | Notes |
|---|---|---|---|
| Exact cache | YES (GDSF) | YES (memory) | not W-TinyLFU |
| Distributed exact cache | YES (redis) | NO | opt-in |
| Semantic vector search | YES | YES (memory brute force) | no ANN |
| Real embeddings | YES (HTTP) | NO (LSH default) | silent fallback |
| Structural guardrails | PARTIAL | YES | token heuristics, not AST |
| Conformal risk | PARTIAL | YES (seeded) | rigorous calibrator unwired |
| Cross-encoder NLI | NO | — | cosine→prob table |
| ReuseCertificate | YES crypto | YES | self-attested evidence |
| Epoch/watermark fencing | YES | YES (admin-only bumps) | client-supplied epochs |
| Roaring lineage | YES | YES | lineage dropped after purge |
| Transitive invalidation | PARTIAL | NO (in-memory direct only) | Neo4j unwired |
| 2-phase admission | YES | YES (when graph cache on) | graph cache default OFF |
| Merkle DAG execution | YES | YES | in-memory store |
| Durable outbox | NO | NO | in-memory; Postgres dead |
| Distributed cluster sync | YES | NO | broadcast, unwired |
| Crypto shredding | YES | YES | |
| FinOps reservations | YES | YES | |

## 2. The single biggest engineering problem

**The default composition is a simulation and the safety proof is self-referential.**
- `config.rs:44-50` selects memory/memory/memory and disables graph cache.
- `state.rs:298` selects `MockOriginProvider`.
- `state.rs:114` selects LSH embeddings.
- `handlers.rs:658-666` hardcodes the very evidence the verifier checks.

Therefore "Sunborn works" in a fresh checkout means "the gateway returns a mocked completion and caches it with hash vectors", and "certificates are verified" means "the gateway verified flags it set itself".

**Important nuance (verified):** in `SUNBORN_ENV=production`, `verify_production_readiness` **refuses to start** unless cache=Dragonfly, db=Postgres, vector=Qdrant, origin is real, embeddings are real, and secrets are strong and separated (`state.rs:474-624`). The simulation therefore exists in development only; production fails closed. That is a genuine strength and is why the remaining blockers are correctness/attestation, not accidental mock deployment.

## 3. What is genuinely differentiated (if completed)

- The **certificate + epoch + lineage** design is a coherent thesis for provable cache reuse.
- **Crypto-shredding with per-tenant TDEKs** is real and elegant.
- **FinOps atomic reservations** are real and uncommon.
- **Merkle DAG execution reuse with purity gating** is real.

These are worth building on — which is exactly the Daredevil upgrade (46 §P5).

## 4. Verdict

Core engine: strong skeleton, real primitives, simulated defaults, unverified safety guarantee. Core score: **5.5/10**.
