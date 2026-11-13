# 49 — Remediation Verification (Post-Audit Pass)

> Note: `inventories/findings.csv` preserves the baseline audit snapshot (all
> statuses `PENDING`). This appendix records post-baseline closure; it supersedes
> the status column for the items listed below.

This appendix records what was actually changed after the baseline audit, with verification. It does not change the baseline finding IDs (see `inventories/findings.csv`); it tracks closure.

---

## 1. Repository cleanup

| Change | Detail | Verify |
|---|---|---|
| Removed redundant doc suites | `docs/master`, `architecture`, `verification`, `engineering`, `audit`, `operations`, `research` (102 tracked artifacts) | `git status` shows 102 deletions |
| Removed generated/junk | root `openapi.yaml` pointer, `sbom.cyclonedx.json`, `research/results/baseline_backup/`, screenshots, caches | files absent |
| Fixed `.gitignore` | added caches, `.commandcode/`, `**/target/`, Playwright output, secrets; narrowed `*.png`, `.trunk/` | `.gitignore` |
| Archived preliminary duplicate audit files | moved to scratchpad, keeping one authoritative suite | `docs/independent-engineering-audit/` |

## 2. P0 fixes

| ID | Fix | Files | Verify |
|---|---|---|---|
| F-001 | Dev auth bypass removed; no plaintext secret stored; requires `access_token` | `frontend/src/app/api/auth/route.ts` | `tsc`, build, tests |
| F-002 | Fake backend removed from default path; BFF fails closed (502) | `frontend/src/app/api/proxy/[...path]/route.ts`; deleted `standalone-simulator.ts` | build + tests |
| F-003 | No fabricated credentials on failure | `frontend/src/app/security/page.tsx` | build + tests |
| F-006 | Qdrant delete now uses the same deterministic UUID as insert | `crates/sunborn-semantic/src/vector_store.rs:476-494` | `cargo test` |

## 3. P1 fixes

| ID | Fix | Files | Verify |
|---|---|---|---|
| F-015 | Dedicated certificate signing key; separated from learning HMAC; distinctness + length enforced in production | `crates/sunborn-gateway/src/state.rs` (field, derivation, readiness check); `handlers.rs` (4 call sites); tests | `cargo test` (252 pass) |
| F-016 | `KEYS` replaced with bounded cursor `SCAN` | `crates/sunborn-cache/src/dragonfly.rs` | `cargo test` |
| F-027 (partial) | Content-Security-Policy added | `frontend/next.config.mjs` | build |
| — | 4 MiB request body limit | `crates/sunborn-gateway/src/lib.rs` | `cargo test` |
| — | Deploy hardening: frontend read-only rootfs + emptyDir volumes + drop ALL; topology spread; image tags unified to `0.1.0` | `deploy/k8s/*.yaml`, `deploy/helm/sunborn/*` | `check_k8s_manifests.py` |

## 4. Claims and docs

| Change | Detail |
|---|---|
| README rewritten | removed 571k/3.2k req/s, 2.15 MB, 0.0% false hits, HNSW/W-TinyLFU/AST/NLI/WAL language, missing headers, broken SDK example; added NO-GO banner and real config matrix |
| Release record replaced | `docs/release/RELEASE_READINESS.md` now NO-GO and points to this audit |
| Integrations guide corrected | header claim replaced with body-field reality |
| UI footer corrected | "Production Ready" → "Independent audit: NO-GO" |
| Branch protection aligned | real job ids; SLSA claim corrected |
| SBOM generator | deterministic serial (content digest) + sorted components; timestamp only from `SOURCE_DATE_EPOCH` |

## 5. Verification results (executed)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | clean (exit 0) |
| `cargo test --workspace --all-targets` | **252 passed, 0 failed** (35 binaries) |
| `npx tsc --noEmit` | clean |
| `npm test` | **37 passed, 0 failed** |
| `npm run build` | compiled successfully |
| `python -m unittest` (SDK) | **26 passed** |
| `python scripts/check_openapi_drift.py` | 49/49 operations match |
| `python scripts/check_k8s_manifests.py` | clean |
| `python scripts/generate_sbom.py` | deterministic (362 components) |

## 6. Still open (the reason production remains NO-GO)

- **F-004**: certificate safety evidence is still asserted at issuance, not measured. Requires the certificate to carry the reference fingerprint and the verifier to recompute it against the incoming request.
- **F-005**: freshness/epochs still originate from the request body; needs an ingestion-driven epoch authority.
- **F-010**: silent fallback still exists outside production (production startup does fail closed).
- **F-011/F-012/F-013/F-014/F-018**: durable outbox, cluster sync, Neo4j, write-behind, and in-memory transitive closure remain unwired/partial.
- **F-028**: no live-infra integration tests.
- **F-036/F-037**: default embeddings and online conformal guarantee not fixed (P4/P5).
- **A11y** (sliders, keyboard divs), image signing/SLSA, OIDC, multi-region.

These remain tracked in `46_REMEDIATION_ROADMAP.md`. The production verdict is unchanged.

---

## 7. Benchmark and gate honesty fixes

| ID | Fix | Files |
|---|---|---|
| F-038 | Precision is now `null`/`N/A` when nothing was reused (no more 100% from an empty denominator) | `benchmarks/adversarial_evaluation_suite.py`, `benchmarks/heldout_semantic_generalization.py` |
| F-039 | Graph memory is measured recursively (`deep_size_mb`) instead of `edges * 1.8` | `benchmarks/graph_invalidation_deep_benchmark.py` |
| F-041 | Held-out summary prints the real scenario count and false-hit rate, not a hardcoded "75 / 0.0%" | `benchmarks/heldout_semantic_generalization.py` |
| F-042 | Capacity soak prints PASS/FAIL from measured error counts, not an unconditional PASS | `benchmarks/capacity_soak.py` |
| F-024 | Removed cosmetic `audit_licenses.py` and no-op `ci_load_test.py`; verify scripts no longer claim "FAANG / TIER-1 COMPLIANCE" | `scripts/verify.sh`, `scripts/verify.ps1` |

Python syntax check: `python -m py_compile` on all modified scripts — exit 0.

## 8. What is now genuinely complete

- Clean, single-suite repository; no committed caches/generated artifacts.
- Truthful README and release record.
- No fake backend, no dev auth bypass, no fabricated credentials.
- Production startup fails closed on simulated backends.
- Dedicated certificate signing key, bounded `SCAN`, request body limit, CSP.
- Deterministic SBOM; real k8s hardening.
- Benchmarks no longer emit fabricated precision or memory numbers.
- 315 tests passing; typecheck and production build green.
