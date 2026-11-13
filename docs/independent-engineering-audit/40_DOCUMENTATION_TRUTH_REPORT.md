# 40 — Documentation Truth Report

The repo carries **four competing documentation suites** with mutually exclusive verdicts and numbers.

---

## 1. Contradictory release verdicts

| Verdict | Document |
|---|---|
| "APPROVED FOR PRODUCTION GA", gates A–G PASSED | `docs/master/18_RELEASE_READINESS_REPORT.md` |
| "ALL GATES PASS / pilot cleared", G1–G10 VERIFIED CLOSED | `docs/architecture/release_acceptance_record.md` |
| "CONDITIONAL PRODUCTION PILOT CLEARED" | `docs/release/RELEASE_READINESS.md` |
| "RELEASE HOLD / NO-GO" | `docs/audit/BASELINE.md` |
| "RELEASE HOLD", gates open | `docs/architecture/project_completion_assessment_2026-10-22.md`, `master_completion_audit...md` |

Four of five claim to be authoritative. They cannot all be true.

## 2. Contradictory numbers

| Metric | Values asserted | Truth |
|---|---|---|
| Endpoints/ops | 42 / 45 / 46 / 49 | 48 paths / 49 ops |
| Tests | 238 / 250 / 256 / 312+ / 338+ / 342 / 358+ | **315 executed** (252+37+26) |
| Throughput | 571,428 / 14,166 / 15,428 / 12,450 / 3,257 | no single reproducible artifact |
| Temporal recall | 0% / 33.33% / 15.79% | contradictory "verified" results |
| Vulnerabilities | "0 (remediated)" | audit.toml still ignores 2 advisories |
| KMS | "real remote adapters" / "simulated test client" / "no real provider in this candidate" | all three in repo |

## 3. Structural duplication

- 4 traceability matrices.
- 4 claim registers (`CLM-0x` reused for different claims).
- 3 defect/finding ledgers with no crosswalk.
- 5 roadmaps.
- 7 security/architecture topic clusters.

## 4. Concrete false/dangling statements

| Statement | Reality |
|---|---|
| `docs/audit/REQUIREMENT_TRACEABILITY.md:60` cites `src/router.rs` | does not exist |
| `docs/audit/EVIDENCE_REGISTER.md:55` counts `sunborn-governance` tests | crate does not exist |
| `docs/master/10:11` "executed using Criterion" | no criterion dep, no `[[bench]]` |
| `docs/master/20:56` `POST /v1/admin/tenants/{id}/crypto-shred` | route is `/v1/governance/shred` |
| `docs/QUICKSTART.md:87,92` `scripts/run_fullstack.*` | does not exist |
| `docs/master/15:66-81` `verify_doc_paths.py` is a CI gate | not invoked by any workflow |
| `docs/verification/SEMANTIC_GENERALIZATION.md:40` "46 traps" | script has 39 |
| `deploy/k8s/multi-region/README.md` claims placeholders removed | manifests still have fake KMS ARNs |

## 5. What is honest and should survive

- `docs/audit/BASELINE.md` — NO-GO, accurate about mock/simulated state.
- `docs/verification/POST_HARDENING_AUDIT.md` — 7.4/10, debunks 0.12 µs claim.
- `research/README.md` — declares non-reproduction and synthetic cost model.
- `docs/master/20_KNOWN_LIMITATIONS_AND_RISKS.md` — discloses limitations (with one bad route ref).

## 6. Verdict

Documentation truth: **LOW**. The README and optimistic suites misrepresent state; the honest audit docs contradict them. Cleanup must collapse to one authoritative set (see `46` P2/P3).
