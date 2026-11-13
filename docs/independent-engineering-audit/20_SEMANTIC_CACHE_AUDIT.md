# 20 — Semantic Cache Audit

---

## 1. Pipeline

`DecisionEngine::verify` → structural fingerprint → vector search → conformal threshold → entailment → causal freshness (`decision.rs:73-193`).

## 2. Components

### 2.1 Normalization / identity
- Canonical request hash; context digest (`context_digest.rs`).
- NFC implementation is a fake 40-pair table (`canonical.rs:8-66`).

### 2.2 Embeddings
- Default `LshRandomProjectionEmbeddingProvider`: FNV-1a over unigrams + char 3-grams into 384 signed buckets (`embedding.rs:54-140`). **Not semantic.**
- HTTP provider real; silent LSH fallback outside production (`:242-251`).
- No embedding-model version in cache keys → model drift hazard.

### 2.3 Vector store
- Memory: O(n·d) scan (`vector_store.rs:94-150`).
- Qdrant: real REST upsert/search/count (`:169-585`). **Delete bug** (F-006): insert uses SHA-256-hashed UUID point id; delete sends the raw string id. Real Qdrant rejects non-UUID → deletes silently fail outside strict mode (`:469-501` returns `Ok(true)` on failure).
- `all_entries()` returns only the local fallback map (`:586-588`) — partial view.

### 2.4 Structural guardrails
- Extracts negation/antonym vocabulary, sorted numerics, entities, temporal markers, quantifiers (`guardrails.rs:33-495`).
- Catches negation flip, numeric transposition, entity swap (tests `:684-879`).
- **Not an AST/parser.** No parser dependency. "AST" is a naming defect.
- No latency benchmark for the "<50 µs" claim.

### 2.5 Conformal risk
- Per-stratum thresholds seeded (`conformal.rs:78-82`); recalibrated after ≥20 obs (`:103-123`); clamp `[0.70,0.99]`.
- Rigorous non-exchangeable calibrator + Wilson interval present but explicitly unwired (`:323-327`).
- API `conformalScore` is a keyword heuristic (`handlers.rs:2775`).

### 2.6 Entailment / NLI
- `CrossEncoderVerifier` is a type alias to `StructuralEntailmentScorer` (`guardrails.rs:595`); scores via cosine→probability piecewise map (`:599-637`).
- `CrossEncoderBackend` trait exists; **zero implementations**.

### 2.7 Freshness
- Epoch lease manager in-memory; only admin endpoints bump.
- Client-supplied `dependency_snapshots`/`knowledge_epoch` (F-005).

## 3. False-hit / false-miss risk

- **False hit:** mitigated by structural guardrails for common inversions, but the conformal guarantee is not established and the certificate evidence is not measured. A crafted paraphrase that preserves tokens can still hit.
- **False miss:** high — LSH default embeddings and strict thresholds cause near-total abstention (see held-out suite: reuse 0).
- **Cache poisoning:** v1 has no authenticated ingestion binding epochs; an attacker who can call a write path with crafted dependencies can influence reuse decisions (admin-gated paths reduce exposure).

## 4. Evidence from checked-in artifacts

- `research/results/adversarial_run.json`: reused 0/39, precision defaulted 100%, recall 0.
- `heldout_generalization_run.json`: coverage 0%, abstention 100%.
- `benchmark_run.json` (124 queries): B3 15 hits, 0 false at LSH embeddings; naive B2 66 hits / 51 false.

## 5. Verdict

Semantic cache is a real research prototype with a compelling thesis and a broken proof. It is not a production semantic cache: default embeddings are non-semantic, ANN is absent, the risk guarantee is not online, and the evidence chain is self-attested. Semantic cache score: **4.5/10**.
