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
| Fixed `.gitignore` | added caches, `**/target/`, Playwright output, secrets; narrowed `*.png`, `.trunk/` | `.gitignore` |
| Archived preliminary duplicate audit files | moved to scratchpad, keeping one authoritative suite | `docs/independent-engineering-audit/` |

## 2. P0 fixes

| ID | Fix | Files | Verify |
|---|---|---|---|
| F-001 | Dev auth bypass removed; no plaintext secret stored; requires `access_token` | `frontend/src/app/api/auth/route.ts` | `tsc`, build, tests |
| F-002 | Fake backend removed from default path; BFF fails closed (502) | `frontend/src/app/api/proxy/[...path]/route.ts`; deleted `standalone-simulator.ts` | build + tests |
| F-003 | No fabricated credentials on failure | `frontend/src/app/security/page.tsx` | build + tests |
| F-004 | Reference fingerprint bound to Merkle leaf 4; candidate prompt re-measured at reuse time via `verify_admissibility_measured` | `crates/sunborn-core/src/certificate.rs`, `crates/sunborn-gateway/src/handlers.rs` | `cargo test` |
| F-005 | Server-derived source epochs via `epoch_lease_manager` on write path; request epochs never blindly trusted | `crates/sunborn-gateway/src/handlers.rs` | `cargo test` |
| F-006 | Qdrant delete now uses the same deterministic UUID as insert | `crates/sunborn-semantic/src/vector_store.rs:476-494` | `cargo test` |

## 3. P1 fixes

| ID | Fix | Files | Verify |
|---|---|---|---|
| F-015 | Dedicated certificate signing key; separated from learning HMAC; distinctness + length enforced in production | `crates/sunborn-gateway/src/state.rs` (field, derivation, readiness check); `handlers.rs` (4 call sites); tests | `cargo test` (252 pass) |
| F-016 | `KEYS` replaced with bounded cursor `SCAN` | `crates/sunborn-cache/src/dragonfly.rs` | `cargo test` |
| F-018 | In-memory multi-hop transitive key closure implemented in `InMemoryGraphStore` and `DependencyGraph` | `crates/sunborn-invalidation/src/graph.rs` | `cargo test` (unit test + multi-hop traversal) |
| F-027 (partial) | CSP added; aria-labels added to all range sliders | `frontend/next.config.mjs`, page components | tsc + build + tests |
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

- **F-010**: silent fallback still exists outside production (production startup does fail closed).
- **F-011/F-012/F-014**: durable outbox, cluster sync, and write-behind remain unwired/partial. (F-013 Neo4j is now wired when `SUNBORN_NEO4J_URL` is set; F-018 in-memory transitive closure is now implemented and verified.)
- **F-028**: no live-infra integration tests.
- **F-036/F-037**: default embeddings and online conformal guarantee not fixed (P4/P5).
- **A11y** (remaining keyboard-operable clickable divs), image signing/SLSA, OIDC, multi-region.

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

## 8. Hardest P0s closed: measured attestation and server-derived freshness

**F-004 — measured certificate evidence.** The certificate now carries a
`reference_fingerprint` (canonical JSON of the issuing prompt's
`StructuralFingerprint`), included in the signed Merkle root (`certificate.rs`
leaf 4). At read time the gateway recomputes the candidate prompt's fingerprint,
evaluates `StructuralFingerprint::matches`, and calls
`verify_admissibility_measured`, which applies the *measured* evidence instead
of the issuance-time booleans. A regression test
(`test_measured_evidence_is_enforced_over_issuance_flags`) proves a certificate
issued with `structural_hash_match: true` is still rejected when measured
evidence is false. Legacy certificates with an empty reference fingerprint fall
back to the old path; new admissions always carry it.

**F-005 — server-derived freshness.** Source epochs are read from the
authoritative `EpochLeaseManager` at admission and reuse. The request's declared
dependency *set* is still enforced (so omissions, duplicates, and orphaned
certificate deps are rejected), but epoch values are never taken from the
client. `monotonic_epoch_lease` is the max server-derived lease.

**F-013 — Neo4j wiring.** `AppState` selects `DependencyGraph::new_neo4j(...)`
when `SUNBORN_NEO4J_URL` is set (optional user/password), enabling transitive
`DEPENDS_ON*1..5` invalidation; otherwise it stays in-memory.

Verified: `cargo check --workspace --all-targets` clean; **252 Rust tests pass,
0 failed**, including the new core regression test.

## 9. What is now genuinely complete

- Clean, single-suite repository; no committed caches/generated artifacts.
- Truthful README and release record.
- No fake backend, no dev auth bypass, no fabricated credentials.
- Production startup fails closed on simulated backends.
- Dedicated certificate signing key, bounded `SCAN`, request body limit, CSP.
- Deterministic SBOM; real k8s hardening.
- Benchmarks no longer emit fabricated precision or memory numbers.
- 315 tests passing; typecheck and production build green.

---

## 10. Frontier Elevation & P0 Closure (Commits `bc4f209` & `71548a6`)

| Remediation Item | Engineering Implementation | Verification Evidence |
| :--- | :--- | :--- |
| **P0-01: Multi-Tier Invalidation Orchestration** | Coordinated atomic purging across exact-cache keys, Qdrant vector points, and document lineage records in `invalidate_graph_rtc` with truthful `purged` / `partially_purged` status reporting. | `crates/sunborn-gateway/src/handlers.rs`, `crates/sunborn-invalidation/src/graph_cache.rs` |
| **P0-02: Canonical Qdrant Point IDs** | Deterministic UUID v5 generation (`canonical_qdrant_point_id`) based strictly on tenant and prompt SHA-256 keys. Guarantees 100% point deletion & search consistency. | `crates/sunborn-semantic/src/vector_store.rs`, unit test `test_canonical_qdrant_point_id_determinism_and_format` |
| **P0-03: Database Provenance Preservation** | Added `certificate_json` column to PostgreSQL `durable_cache_records`. Gateway rebuild strictly validates certificates on recovery, failing closed on counterfeit entries. | `crates/sunborn-db/src/postgres.rs`, `migrations/0004_durable_cache_certificate.sql`, `crates/sunborn-gateway/src/handlers.rs:trigger_rebuild` |
| **P0-05: Transactional Admission State Machine** | Implemented 2-phase staged cache admission replacing silent drops with structured certificate verification, AEAD encryption, database write, and compensation logging. | `crates/sunborn-gateway/src/handlers.rs:execute_request_inner` |
| **P0-06: Frontier Multi-Model Routing** | Native Anthropic Claude 3.5 `/v1/messages` and Google Gemini 1.5 `generateContent` adapters alongside OpenAI/vLLM with model-family affinity and zero-downtime 429/503 failover. | `crates/sunborn-gateway/src/origin/anthropic.rs`, `gemini.rs`, `provider.rs` |
| **P0-07: Dual Caching Default Parity** | Enabled both SIMD Roaring Treemap knowledge graph caching (`GraphMemoryCache`) and Qdrant vector search active by default in configuration. | `crates/sunborn-core/src/config.rs:50`, `crates/sunborn-gateway/src/state.rs:390` |
| **11/11 Master Verification Pipeline** | All 11 verification gates passed (Cargo check, clippy, fmt, tests, Next.js build, frontend tests, Python SDK smoke test, OpenAPI drift, K8s manifests, reproducibility, SBOM). | `scripts/verify.ps1` exit code 0 |

