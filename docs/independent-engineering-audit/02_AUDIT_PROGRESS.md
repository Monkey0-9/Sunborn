# 02 — Audit Progress & Coverage Ledger

**Baseline:** `0e412f8` · **Branch:** `audit/independent-2026-11` · **Date:** 2026-11-13

This ledger tracks what was actually inspected. It distinguishes direct source reading from automated search and from unattempted work. It is updated as the audit proceeds.

---

## 1. Coverage summary

| Area | Files (tracked) | Directly read | Automated census | Not inspected | Coverage |
|---|---:|---:|---:|---:|---:|
| Rust crates (`crates/**`) | 95 | 30+ key files | all | remaining helpers | ~90% of public paths |
| App (`apps/sunborn-rebuild`) | 1 | 1 | 1 | — | 100% |
| Frontend (`frontend/src`) | 34 | 20+ | all | minor components | ~85% |
| Frontend tests | 7 | 7 | 7 | — | 100% |
| Docs (`docs/**`) | 103 | inventory + key docs | all | prose-only docs | ~95% inventory |
| Scripts | 11 | 10 | 11 | — | ~95% |
| Deploy | 34 | 20+ | all | some manifests | ~80% |
| Research/benchmarks | ~20 | 12 | all | some datasets | ~85% |
| SDK | 12 | 8 | all | — | ~90% |
| Migrations | 3 | 3 | 3 | — | 100% |
| `.github` | 9 | 9 | 9 | — | 100% |

**Tracked files at baseline:** 382.

---

## 2. Command ledger (executed)

| Command | Result | Recorded in |
|---|---|---|
| `git rev-parse --abbrev-ref HEAD` | `main` (baseline), then `audit/independent-2026-11` | 05 |
| `git ls-files` | 382 tracked files | 04 |
| `git log --oneline -8` | HEAD `0e412f8` | 05 |
| `python -m unittest sdk/python/test_sunborn_agent.py sdk/python/tests/test_sdk_package.py` | **26 tests, OK, 1.534s** | 28 |
| `cargo test --workspace --all-targets` | **252 passed, 0 failed**, 35 binaries | 28 |
| `npm test` (frontend) | **37 passed, 0 failed** | 28 |

Commands **not** run and why:

| Command | Status | Reason |
|---|---|---|
| `cargo clippy -D warnings` | NOT RUN in this pass | deferred to remediation gate (P6) |
| `cargo build --release --locked` | NOT RUN | deferred |
| Live Postgres/Redis/Qdrant/Neo4j/KMS integration | BLOCKED | infra not provisioned; audit-only |
| `docker compose up` smoke | BLOCKED | audit is non-deploy |
| Load tests | NOT RUN | prohibited in audit phase |

---

## 3. Findings seeded and independently re-verified

Each pre-seeded finding from planning was re-read at the cited line before inclusion.

| ID | Status |
|---|---|
| F-001 dev auth bypass | VERIFIED (code read) |
| F-002 fake backend default | VERIFIED (code read) |
| F-003 fabricated credentials | VERIFIED (code read) |
| F-004 self-attesting certificate | VERIFIED (code read) |
| F-005 client-controlled freshness | VERIFIED (code read) |
| F-006 Qdrant delete id mismatch | VERIFIED (insert vs delete read) |
| F-007 false public claims | VERIFIED (artifact + script read) |
| F-010 silent degradation | VERIFIED (4 call sites read) |
| F-016 `KEYS` vs `SCAN` | VERIFIED (`dragonfly.rs:218,250`) |
| F-017 lineage dropped | VERIFIED (`bitmap_lineage.rs:46-65`) |
| F-018 no in-memory transitive closure | VERIFIED |
| F-022 shutdown claim mismatch | VERIFIED (`main.rs:109-112`) |
| F-029 broken root openapi | VERIFIED (`Get-Item` + content) |

---

## 4. Exclusions & limitations

- No external services were contacted; all external-adapter verdicts derive from source inspection.
- No production deployment, no destructive migration, no push.
- Two working-tree files were already modified at audit start (`research/results/graph_benchmark_results.json`, `sbom.cyclonedx.json`); they were preserved, not reverted.
- Percentages in the final verdict are computed only where a denominator is enumerable (see 48 §Methodology).

---

## 5. Phase status

| Phase | Status |
|---|---|
| P0 Safety & snapshot | COMPLETE (tag `pre-audit-0e412f8` + branch `audit/independent-2026-11`) |
| P1 Independent audit | COMPLETE (this suite) |
| P2 Cleanup | COMPLETE (102 tracked artifacts/docs removed; `.gitignore` fixed; caches/screenshots deleted) |
| P3 Claims reconciliation | COMPLETE (README rewritten; release record replaced; integrations header claim fixed; UI footer corrected) |
| P4 Remediation | PARTIAL→ADVANCED: frontend P0s fixed; Qdrant delete, key separation, SCAN, body limit, CSP, deploy hardening landed (see `49_REMEDIATION_VERIFICATION.md`) |
| P5 Daredevil upgrade | PENDING (roadmap in 46 §P4) |
| P6 Verification gates | PARTIAL: cargo 252 / frontend 37 / python 26 pass; tsc clean; Next build OK; OpenAPI 49/49; k8s clean; SBOM deterministic |

## 6. Post-cleanup verification (2026-11-13)

| Check | Result |
|---|---|
| `cargo test --workspace --all-targets` | 252 passed, 0 failed (no `error[` / FAILED) |
| `npm test` (frontend) | 37 passed, 0 failed |
| `npx tsc --noEmit` | clean (exit 0) |
| `npm run build` | "✓ Compiled successfully" |
| `python scripts/check_openapi_drift.py` | 49/49 operations match |
| `python -m unittest` (SDK) | 26 passed |
| `git status` | 117 entries: 102 deleted, 13 modified, 2 untracked |

No commit or push was performed; changes remain staged/unstaged on the audit branch for review.
