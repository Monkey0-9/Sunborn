# 28 — Testing & QA Audit

**Executed on 2026-11-13 on the audit branch, baseline code unchanged.**

---

## 1. Executed results (real)

| Suite | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace --all-targets` | **252 passed, 0 failed**, 35 test binaries |
| Frontend | `npm test` | **37 passed, 0 failed** |
| Python SDK | `python -m unittest sdk/python/test_sunborn_agent.py sdk/python/tests/test_sdk_package.py` | **26 passed, OK, 1.534s** |
| **Total executed** | | **315 passed, 0 failed** |

Rust per-binary sample: 9,13,5,15,21,0,18,1,1,7,1,1,1,3,2,2,13,10,6,1,15,5,1,8,3,8,12,1,1,23,4,17,4,0,20.

## 2. Claimed vs actual counts

| Claim | Actual | Verdict |
|---|---|---|
| README badge "358+" tests | 315 runnable (252+37+26) | **FALSE/overstated** |
| README "250 cargo tests" | 252 | VERIFIED (off-by-2) |
| README "37 frontend" | 37 | VERIFIED |
| README "26 python" | 26 | VERIFIED |
| README "39 adversarial scenarios" then "46" | 39 scripts / 70 held-out | inconsistent; 46 FALSE |
| docs say "238 / 250 / 256 / 312+ / 338+ / 342 / 358+" | — | no single source; contradictory |
| "11 CI gates" | `verify.sh` has 11 stages; CI runs 5 unrelated jobs | PARTIAL |

## 3. Test quality

**Genuine:**
- Gateway integration tests via `oneshot` (in-process HTTP).
- Two real TCP+reqwest adversarial tests.
- Certificate tamper/forgery/cross-tenant tests.
- FinOps reservation semantics.
- `research/tests/test_verifier_negative.py` (23) — artifact tamper rejection.
- Frontend `bff_security.test.mjs` (14) pure-function policy tests.

**Weak / non-evidential:**
- Frontend `accessibility_wcag.test.mjs` (5) and `views_and_navigation.test.mjs` (4) are **source-text greps**, not rendered behavior. They pass despite real a11y violations.
- Playwright E2E (7) mocks every gateway call then asserts its own mocks — proves rendering, not integration.
- "Property-based" tests are hand-rolled LCG loops, not a property-testing framework.
- No test touches live Postgres, Redis, Qdrant, Neo4j, Vault, or AWS KMS.
- No mutation testing, no coverage gate, no `--locked`.

## 4. Test gaps by crate

No `tests/` directory at all in: `sunborn-core`, `sunborn-cache`, `sunborn-semantic`, `sunborn-execution`, `sunborn-db`. External-adapter coverage is mock-server or "must fail when unreachable".

## 5. CI vs local

- `.github/workflows/ci.yml` is real (`fmt`, `clippy -D warnings`, `test`, frontend build/test, SDK, openapi drift, Helm).
- It **does not call `scripts/verify.sh`**, so the "11/11 gates" badge is not produced by CI.
- `release.yml` has no signing/SLSA despite claims.

## 6. Verdict

Test volume is real (315 passing) and some tests are genuinely good. But the suite **cannot detect the P0 defects** (self-attesting certificate, fake backend, client-supplied freshness) and contains non-evidential "tests". QA score: **5/10**.
