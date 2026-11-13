# 11 — Model-Level Audit

Audit of the distinct "models" (data/behavioral models) the system defines.

---

## 1. Identity model (10-D canonical request)

- `sunborn-identity/src/identity.rs` and `canonical.rs` define canonicalization over prompt, system, model, temperature, tenant, app, etc.
- **Defect:** `normalize_unicode_nfc` is a 40-pair hardcoded table (`canonical.rs:8-66`), not Unicode NFC. Non-Latin input can normalize differently than a conforming implementation.
- `canonicalize_json` (`:96`) is key-sorted, not JCS; `canonicalize_json_rfc8785` (`:119`) exists and is used by certificate/merkle/signature. Two canonicalizers with different semantics is a correctness hazard if callers mix them.

## 2. Certificate model

Fields: tenant, app, identity hash, model, token count, dependency snapshots, safety evidence, validity window, monotonic epoch, signature.

- Merkle structure: 5 domain-separated leaves, length-prefixed, HMAC-SHA-256, constant-time compare.
- **F-004:** the `SafetyEvidenceRecord` is caller-constructed. `handlers.rs:658-666` supplies:
  - `structural_hash_match: true`
  - `negation_polarity_match: true`
  - `numeric_literal_match: true`
  - `entity_drift_detected: false`
  - `non_conformity_score: 0.0`
  - `calibrated_risk_bound: 0.25`
- `validate_for_reuse` (`certificate.rs:454-462`) rejects if any mismatch/drift — but since issuance always sets them to success, the check is tautological for the write path.
- **F-005:** `dependency_snapshots` and `knowledge_epoch` are request-supplied (`handlers.rs:657,671`).

**Verdict:** cryptographic envelope is sound; the *evidence semantics* are not. The certificate proves "the gateway signed these claims", not "these claims are true".

## 3. Epoch / lease model

- `EpochLeaseManager` = `HashMap<Uuid, AtomicU64>` (`epoch.rs:76-150`).
- Advanced only by admin endpoints (`handlers.rs:1235`, `:1369`).
- No ingestion/RAG path bumps epochs → in normal operation leases never move except by manual admin action.
- Watermark fencer is real but likewise decoupled from document ingestion.

## 4. Vector / embedding model

- Default: FNV-1a LSH projection (`embedding.rs:54-140`). 384 dims. This is a hashing bag-of-ngrams — not a semantic embedding.
- HTTP provider real (`embedding.rs:174-233`), but non-production failures silently fall back to LSH (`:242-251`).
- No model-version/embedding-version binding in cache keys beyond a context digest; changing embedding models can invalidate semantic meaning while the key stays constant (model-version hazard).

## 5. Conformal / risk model

- Per-stratum α incl. `SecuritySensitive => 0.001` (`conformal.rs:50-58`).
- Thresholds seeded (0.82/0.90/0.95/0.90/0.98, `:78-82`); recalibrate only after ≥20 observations using an α-quantile of similarities clamped `[0.70,0.99]` (`:103-123`).
- The statistically correct non-exchangeable calibrator and Wilson interval are present but explicitly **unwired** (`conformal.rs:323-327`).
- `handlers.rs:2775` reports `"conformalScore": heuristic_score` — the API's "conformal score" is a keyword-match heuristic.

**Verdict:** the online risk control does not provide the claimed distribution-free guarantee.

## 6. FinOps model

- Reservation/settle/release with NaN/inf/negative sanitization (`finops.rs:204-360`).
- Real and wired.
- Pricing table is static; no live provider price feed (acceptable, but must be documented).

## 7. Execution DAG model

- Nodes have purity/effect class; effect nodes are never reused.
- `splice_dag` prunes cached prefixes; **latency saving is a flat `+=150` per node** (`merkle.rs:174`) — a cosmetic estimate, not a measurement.

## Summary

| Model | Correctness | Claimed guarantee justified? |
|---|---|---|
| Identity canonicalization | Partial (NFC fake) | No |
| Certificate crypto | Sound | Crypto yes; evidence no |
| Freshness/epoch | Partial (client-supplied, admin-only bumps) | No |
| Embeddings | Default non-semantic | No |
| Conformal risk | Seeded/partial | No |
| FinOps | Sound | Yes |
| Execution DAG | Sound | Yes (reuse logic) |
