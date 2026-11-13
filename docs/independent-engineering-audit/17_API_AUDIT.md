# 17 — API Audit

**Contract:** `docs/api/openapi.yaml` (OpenAPI 3.1). **Router:** `crates/sunborn-gateway/src/lib.rs:35-175`.

---

## 1. Endpoint inventory

- 40 routes registered in `create_router`.
- `docs/api/openapi.yaml` declares **48 paths / 49 operations**.
- `scripts/check_openapi_drift.py` parses `.route(...)` calls and set-diffs both directions. It is a genuine check.
- Root `openapi.yaml` is **not** a valid spec — it is a 21-byte hardlink file whose content is the text `docs/api/openapi.yaml`. `check_openapi_drift.py:80` only checks `os.path.exists`, so drift can "pass" with an unusable root file.

## 2. Endpoint groups (verified in router)

| Group | Examples |
|---|---|
| Chat / messages | `POST /v1/chat/completions`, `POST /v1/messages` |
| Cache | `GET/DELETE /v1/caches/{key}`, graph stats/lineage/invalidate |
| Invalidation | `POST /v1/invalidation` |
| Execution | `POST /v1/execution/plan`, `/results`, eviction, dependency |
| Governance | shred, rebuild, attest, verify-proof |
| FinOps | `/v1/finops/report` |
| Security | `/v1/security/audit`, `/v1/security/evaluate` |
| Learning | policies, calibrate, canary, promote, rollback, feedback |
| Auth | `/v1/auth/login`, `/v1/auth/session`, credentials |
| Ops | `/health`, `/ready`, `/metrics`, `/v1/cluster/status` |
| Chaos | `/v1/chaos/trip`, `/v1/chaos/reset` |
| Routing | radix route/inspect, speculative predict |

## 3. Contract correctness

| Check | Result |
|---|---|
| Path parity (router vs spec) | 49 ops / 48 paths; drift script genuine |
| Request body schemas | Present in spec; handlers deserialize typed structs |
| Response schemas | Present; not runtime-validated |
| Auth inheritance | Spec defines security; middleware enforces |
| Error format | JSON problem details |
| Versioning | `/v1` prefix; no deprecation policy |
| Idempotency | Not implemented on mutating endpoints (no idempotency keys) |
| Rate limiting | Login + spend velocity; not general per-endpoint |

## 4. Documented headers that do not exist

README examples assert `x-sunborn-cache: HIT_EXACT` and `x-sunborn-latency-ms`. A repository grep finds these **only in README**. The gateway sets `x-request-id` and `Retry-After` only (`middleware/trace.rs`, `handlers.rs`). This is a P0 claim defect (F-021).

## 5. Security-relevant API issues

- `POST /v1/security/evaluate` returns `conformalScore` equal to a keyword heuristic (`handlers.rs:2775`) — misleading field name.
- Chaos endpoints are admin-gated (verify) but exist in production router; must be confirm-guarded.
- `/v1/governance/shred` is the real shred endpoint; docs reference a non-existent `/v1/admin/tenants/{id}/crypto-shred`.

## 6. Verdict

The API surface is real and mostly well-specified; drift checking is one of the better scripts in the repo. The broken root spec pointer, missing documented headers, and leaky field semantics are P0/P2 defects. API score: **7/10**.
