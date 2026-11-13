# 22 — Attestation Audit

---

## 1. What the attestation layer claims

- Every reused response carries a signed `ReuseCertificate` binding tenant, app, model, identity hash, dependency snapshots, safety evidence, validity window, monotonic epoch.
- Verification rejects tamper, cross-tenant use, expired lease, epoch mismatch, duplicate/missing snapshots, structural violation, entity drift.

## 2. What is cryptographically real

- Merkle root over 5 domain-separated, length-prefixed leaves (`certificate.rs:75-252`).
- HMAC-SHA-256 signature with constant-time comparison.
- Verification enforces 10 gates (`certificate.rs:454-462` and surrounding).
- Tamper/forgery/cross-tenant/epoch tests exist (`certificate.rs:506-901`).

**This part is sound.**

## 3. What breaks the attestation

### F-004: self-attested evidence
Issuer (`handlers.rs:651-674`) sets:
```
structural_hash_match: true
negation_polarity_match: true
numeric_literal_match: true
entity_drift_detected: false
non_conformity_score: 0.0
calibrated_risk_bound: 0.25
```
Verifier (`certificate.rs:454-462`) rejects if any of these is false/true respectively. Because issuance never computes them from the semantic engine, the gate is tautological. The certificate attests *what the issuer asserted*, not *what the engine measured*.

### F-005: client-controlled freshness
`dependency_snapshots` come from the request (`handlers.rs:657`); `monotonic_epoch_lease` is `req.knowledge_epoch.unwrap_or(1)` (`:671`). The verifier compares against the epoch registry, but the registry only moves via admin endpoints. A client can therefore supply epochs consistent with a stale registry.

### F-015: key-role collapse
Signing key is `state.learning_hmac_secret` (`handlers.rs:673`), the same secret used for learning feedback. Production checks separation from KEK/admin only (`state.rs:572-587`). Compromise of the learning secret compromises attestation.

### Replay
No nonce/jti on certificates; replay within TTL is possible unless the epoch advances. No independent verifier CLI.

## 4. Verdict

The cryptographic envelope is competent; the **semantic attestation is not trustworthy** because it attests unmeasured claims and client-controlled freshness, and the signing key is shared with an unrelated subsystem. Attestation score: **4/10**.

## 5. Required fixes (see 46 P4)
1. Compute evidence from `StructuralFingerprint`/decision engine at issuance; refuse issuance if measurement fails.
2. Derive dependency snapshots/epochs server-side from ingestion; reject client-only freshness.
3. Dedicated signing key + key id + rotation; separate from learning HMAC.
4. Add nonce/expiry replay protection and a standalone `sunborn-verify` CLI.
