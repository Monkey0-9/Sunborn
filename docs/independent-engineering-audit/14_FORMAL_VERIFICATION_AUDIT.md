# 14 — Formal Verification Audit

---

## 1. What exists

- **Property/invariant tests (hand-rolled):** `crates/sunborn-gateway/tests/property_invariants_test.rs` (5 tests) — canonicalization permutations, Merkle avalanche, lineage algebra, epoch monotonicity. These are LCG loops, not a property-testing framework.
- **Deterministic fuzz loops:** `crates/sunborn-identity/tests/fuzz_canonicalize_test.rs` (4 tests) and a separate `fuzz/fuzz_targets/fuzz_canonicalize.rs` (cargo-fuzz target, 33 lines).
- **Negative verifier suite:** `research/tests/test_verifier_negative.py` (23 tests) — tamper rejection for artifacts. Genuine and valuable.
- **Design-by-contract style:** `certificate.rs` returns `Result` with explicit validation errors; `finops.rs` sanitizes inputs.

## 2. What does not exist

- No TLA+/Alloy models.
- No SMT/Z3 proofs.
- No symbolic execution.
- No mutation testing.
- No model checking of the certificate/epoch state machine.
- No mechanically verified invariant for the "no stale reuse" property.

## 3. Critical safety properties and their status

| Property | Claim | Verification |
|---|---|---|
| P1: Never serve a cached answer for an inverted query | Structural guardrails | Tested by example (`guardrails.rs:684-879`); not proven |
| P2: Certificate evidence reflects measured checks | README implies yes | **Contradicted** — evidence hardcoded (F-004) |
| P3: Freshness cannot be client-forged | Implied | **Contradicted** — client-supplied snapshots/epoch (F-005) |
| P4: Invalidation is transitive | Claimed | **Only Neo4j (unwired)**; in-memory direct edges |
| P5: Cache writes are atomic with lineage | Claimed | Implemented with rollback (`graph_cache.rs`); unit-tested |
| P6: Exactly-once invalidation | Implied by outbox | In-memory at-most-once across restart |

## 4. Test-vs-proof distinction

The repo's `docs/master/12` describes an "11-gate verification pipeline". That is an ordinary test/script pipeline, not formal verification. The `research/verify_certificate.py` is a strong *artifact-integrity verifier*, but it verifies provenance of benchmark outputs, not system correctness.

## 5. Recommendation

Prioritize formal treatment of the safety path, not the whole system:
1. Model the certificate lifecycle as a state machine (epochs, issuance, expiry, reuse).
2. Encode P1/P3 as property tests against the real guardrail functions with generated adversarial prompts.
3. Add mutation testing on `certificate.rs` and `guardrails.rs`.
4. Optionally a TLA+ spec for epoch/outbox ordering.

## Verdict

Formal verification: **not performed**. There are good example-based and artifact-integrity tests, but no mechanically verified property. Formal methods score: **2/10**.
