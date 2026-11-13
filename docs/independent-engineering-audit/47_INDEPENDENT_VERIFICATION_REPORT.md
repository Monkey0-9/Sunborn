# 47 — Independent Verification Report

Purpose: re-derive the strongest conclusions from primary evidence, state confidence, and define what would falsify them.

---

## 1. Verification method

For each critical claim, the auditor (a) read the cited source, (b) looked for a contrary implementation, (c) checked whether a test exercises it, and (d) assigned a confidence level.

| Finding | Primary evidence | Contrary evidence | Test? | Confidence |
|---|---|---|---|---|
| F-001 dev auth bypass | `api/auth/route.ts:152-176` | `NODE_ENV=production` disables it | none | **HIGH** |
| F-002 fake backend default | `proxy/route.ts:163-190`; simulator 698 LOC | privileged routes exempt; production strict | Playwright mocks only | **HIGH** |
| F-003 fake credentials | `security/page.tsx:145-172` | — | none | **HIGH** |
| F-004 self-attesting cert | `handlers.rs:658-666` vs `certificate.rs:454-462` | none | cert tamper tests don't test evidence truth | **HIGH** |
| F-005 client freshness | `handlers.rs:657,671` | registry validation | none | **HIGH** |
| F-006 Qdrant delete id | insert `vector_store.rs:281-353` vs delete `:469-478` | — | none | **HIGH** |
| F-007 false claims | README vs artifacts/scripts | — | — | **HIGH** |
| F-010 silent fallback | `embedding.rs:242-251`, `dragonfly.rs`, `vector_store.rs`, `postgres.rs` | production strict path | startup tests | **HIGH** |
| F-015 shared key | `handlers.rs:673`, `state.rs:307-319` | prod separation check only KEK/admin | production_startup_test | **MED-HIGH** |
| F-017 lineage dropped | `bitmap_lineage.rs:46-65` | — | no repeat-mutation test | **HIGH** |
| F-022 shutdown mismatch | `main.rs:109-112` (15s) vs README 30s | — | shutdown tests | **HIGH** |

## 2. Claims re-derived from artifacts

- **Precision 100% / 0 false hits:** re-derived from `adversarial_run.json` (`reused=0, tp=0`) and the code's `else 100.0` default. Conclusion: vacuous. Confidence HIGH.
- **2.15 MB / 1.8 B/edge:** re-derived by reading the formula at `graph_invalidation_deep_benchmark.py:298`. Conclusion: constant, not measured. Confidence HIGH.
- **571,428 req/s:** searched whole repo for `throughput_test` → zero files. Conclusion: citation invalid. Confidence HIGH.
- **252 Rust tests:** executed `cargo test` → 252 passed. Confidence HIGH.
- **37 frontend / 26 Python:** executed → pass. Confidence HIGH.

## 3. Cross-checks that could overturn findings

| Finding | What would overturn it | Result |
|---|---|---|
| Silent fallback is harmful | If production always sets strict + real backends | Docs/README imply real by default → **not overturned** |
| Certificate evidence unmeasured | If `SafetyEvidenceRecord` were computed elsewhere | Grep: only construction site is `handlers.rs:658` → **not overturned** |
| Qdrant delete bug | If Qdrant accepted raw ids | Qdrant API requires UUID point ids → **not overturned** |
| Fake backend default | If `npm run dev` set `STRICT_GATEWAY=true` | `package.json` dev script does not → **not overturned** |
| Cluster sync unwired | If gateway referenced `DistributedGraphSyncEngine` | Zero references in `sunborn-gateway/src` → **not overturned** |

## 4. Limitations of this verification

- No live Postgres/Redis/Qdrant/KMS was provisioned; external-adapter verdicts are source-level.
- No `cargo clippy`/`--locked` build was run in the audit phase (deferred to gates).
- Some helper functions were reviewed by census rather than line-by-line.
- Percentages are computed from the enumerated 45-feature matrix, not from an exhaustive function count.

## 5. Reproducibility instructions

```
git checkout audit/independent-2026-11
cargo test --workspace --all-targets      # expect 252 passed
cd frontend && npm test                   # expect 37 passed
python -m unittest sdk/python/test_sunborn_agent.py sdk/python/tests/test_sdk_package.py  # 26
python scripts/check_openapi_drift.py
```
Read `41_ANTI_FAKE_AUDIT.md` and verify each `file:line`.

## 6. Verification verdict

The critical conclusions are **independently reproducible from source** and are not artifacts of tooling. The audit's confidence is HIGH for P0/P1 findings and MEDIUM for performance magnitude claims (which are unverifiable rather than disproven).
