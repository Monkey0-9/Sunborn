# 23 — AI / ML / LLM Audit

---

## 1. LLM provider integrations

| Provider | Adapter | Real? | Default? |
|---|---|---|---|
| OpenAI-compatible `/chat/completions` | `HttpLlmOriginProvider` | YES (reqwest) | NO (Mock default) |
| Anthropic `/v1/messages` | same provider | YES | NO |
| vLLM (self-hosted) | OpenAI-compatible URL | YES | NO |
| Mock | `MockOriginProvider` | returns formatted string | **YES** |

No model selection/fallback policy beyond URL config; no cost tracking against live price feed; token cost derived from response usage.

## 2. Embeddings / retrieval

- Default LSH hashing (not a model).
- HTTP embeddings real.
- Vector search: brute force (default) or Qdrant (opt-in).
- No reranker model; entailment is a cosine table.
- No dataset/model versioning binding in cache keys.

## 3. Evaluation

Checked-in artifacts:
- `adversarial_run.json`: 39 scenarios, reuse 0, precision defaulted 100%, recall 0.
- `heldout_generalization_run.json`: 70 scenarios (script prints "75"), coverage 0, abstention 100.
- `benchmark_run.json`: 124 queries, B3 15 hits / 0 false, naive B2 66/51 false; savings from `SYNTHETIC_COST_ESTIMATION`.
- Provenance: `git_commit: unknown` in run_manifest; certificate commit `6bf5d40` ≠ HEAD `0e412f8`; Windows manifest bundled into a Linux-generated certificate.

**No real-model evaluation exists.** Every reported semantic number uses hash vectors.

## 4. Reproducibility

- `research/verify_certificate.py` genuinely recomputes metrics from raw traces and rejects tampering (23 negative tests).
- But the certificate is unsigned (`signing_algorithm: NONE_UNAUTHENTICATED_LOCAL_DIGEST`) and declares non-reproduction in `research/README.md`.
- Cost model is declared synthetic and must not be presented as invoices.

## 5. Hallucination / output validation

- No output validation beyond upstream response pass-through.
- No content safety filter.
- Grounding is the cache's job (reuse only entailed answers), but the entailment is heuristic.

## 6. Privacy

- `privacy.rs` provides PII placeholder tokenization for canonicalization (real, lexical).
- No model-side data-retention controls beyond sending to upstream.

## 7. Verdict

Sunborn is an **LLM infrastructure** project, not an AI model project. Its AI claims are about embedding/retrieval/entailment, and those are the weakest, least-reproducible part. AI/ML score: **4/10**.
