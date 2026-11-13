# 46 — Remediation Roadmap

Phased, dependency-ordered. Each task has acceptance criteria and a verification method. No implementation is performed in the audit phase.

---

## P0 — Critical blockers (must fix before any production claim)

| ID | Task | Files | Acceptance | Verify |
|---|---|---|---|---|
| P0-1 | Remove frontend fake backend from default path; fail closed when gateway down | `frontend/src/lib/standalone-simulator.ts`, `app/api/proxy/[...path]/route.ts` | No simulated response unless explicit `SUNBORN_DEMO=1`; UI shows offline | Playwright offline test |
| P0-2 | Remove dev auth bypass; never store plaintext secret | `frontend/src/app/api/auth/route.ts:132,152-176` | Gateway-down login returns 502; token only from `access_token` | Unit + e2e |
| P0-3 | Remove fake credential minting | `frontend/src/app/security/page.tsx:145-172` | Failure shows error, no key created | Unit test |
| P0-4 | Compute certificate evidence from engine; refuse if unmeasured | `handlers.rs:651-674`, `certificate.rs` | Evidence equals measured fingerprint results | New integration test |
| P0-5 | Server-derive freshness/epochs; reject client-only | `handlers.rs:657,671`; ingestion endpoint | Epochs from registry, not body | Fencing + replay tests |
| P0-6 | Fix Qdrant delete point id | `vector_store.rs:469-501` | Insert/delete use same hashed UUID | Contract test |
| P0-7 | Rewrite all false public claims (perf, headers, SDK example, counts) | `README.md`, docs | Every number cites a reproducible artifact | `verify_doc_paths.py` + review |

## P1 — High priority (correctness, security, durability)

| ID | Task | Acceptance |
|---|---|---|
| P1-1 | Fail closed or explicit degraded mode instead of silent fallback | No silent in-memory substitution in production; metric + log when degraded |
| P1-2 | Wire Postgres transactional outbox + persisted DLQ; idempotent replay | Invalidation survives restart; replay safe |
| P1-3 | Wire `DistributedGraphSyncEngine` via Redis/Dragonfly pub/sub or mark experimental out of path | Multi-pod purge test |
| P1-4 | Implement in-memory transitive closure or require Neo4j | Transitive invalidation test in-memory |
| P1-5 | Wire or delete `WriteBehindFlusher` | No dead durable path |
| P1-6 | Separate certificate signing key + key id + rotation | Keys distinct; rotation test |
| P1-7 | Replace `KEYS` with `SCAN` | No O(N) blocking |
| P1-8 | Fix lineage retention | Repeat-mutation test |
| P1-9 | Async KMS trait (remove `run_blocking_async`) | No runtime spin |
| P1-10 | Add general rate limiting + request size caps | Load test |
| P1-11 | Remove advisory suppressions or document exceptions | `cargo audit` clean or justified |
| P1-12 | Add live-infra integration tests (compose/testcontainers) | CI touches Postgres/Redis/Qdrant |
| P1-13 | Fix CI branch protection names; wire `verify_doc_paths`; `--locked` | Protection satisfiable |
| P1-14 | Deploy: real secret provisioning; frontend read-only; anti-affinity; unify tags | `kustomize build` + smoke |
| P1-15 | Remove or fix dead components (conformal controller, micro-batch, PKCS11) | No exported-unused API |

## P2 — Important engineering improvements

| ID | Task |
|---|---|
| P2-1 | Deterministic SBOM; wire into release |
| P2-2 | Fix broken root `openapi.yaml`; update drift script/links |
| P2-3 | Rewrite identity canonicalization with real NFC |
| P2-4 | Unify canonical JSON (single RFC 8785 path) |
| P2-5 | Real OTel metrics/tracing; dashboards/alerts |
| P2-6 | Consolidate docs into one SSOT suite |
| P2-7 | Consolidate SDK to one package; fix README example |
| P2-8 | Add idempotency keys to mutations |
| P2-9 | `.gitignore` fixes; remove junk/artifacts |
| P2-10 | Fix accessibility (labels, keyboard, CSP, status text) |
| P2-11 | Replace source-grep a11y tests with axe/Playwright |

## P3 — Performance / maintainability / optimization

| ID | Task |
|---|---|
| P3-1 | Criterion benches + wrk/oha HTTP harness; delete hardcoded metrics |
| P3-2 | Publish p50/p90/p99/p999 with hardware manifest + signed artifacts |
| P3-3 | Add coverage thresholds |
| P3-4 | Remove dead code; reduce API client surface |
| P3-5 | Optimize L1 path; measure not assert |

## P4 — Advanced capabilities (Daredevil upgrade)

| ID | Task |
|---|---|
| P4-1 | Real embedding providers; no silent fallback |
| P4-2 | Real ANN (Qdrant HNSW prod + embedded `hnsw_rs`/`usearch`) |
| P4-3 | Real cross-encoder NLI backend (ONNX/HTTP) |
| P4-4 | Rigorous non-exchangeable conformal calibrator on production path |
| P4-5 | Asymmetric, third-party-verifiable certificates + `sunborn-verify` CLI |
| P4-6 | Held-out evaluation with real embeddings, non-zero reuse |
| P4-7 | Multi-region implementation with partition tests |
| P4-8 | OIDC/MFA operator auth |
| P4-9 | Cosign + SLSA + Trivy |
| P4-10 | Generated API reference from OpenAPI; machine-checked claims |

## Sequencing

`P0 → P1-1..P1-8 → P1-9..P1-15 → P2 → P3 → P4`.

P0-7 (claims) can land immediately and should, because it stops misleading users while code is fixed.

## Definition of done

- All P0/P1 closed with regression tests.
- `31_PRODUCTION_READINESS.md` gates re-run with transcripts.
- `29` updated with reproducible numbers.
- No hardcoded metrics remain (CI guard).
