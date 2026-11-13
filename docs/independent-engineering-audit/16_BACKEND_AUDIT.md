# 16 — Backend Audit

**Scope:** `crates/sunborn-gateway` + domain crates.

---

## 1. Strengths

- **Routing & protocol translation:** 40 routes (`lib.rs:35-175`); OpenAI `/v1/chat/completions` (`handlers.rs:904`) and Anthropic `/v1/messages` (`handlers.rs:1069`) both translate and funnel into `execute_request_inner` (`:980`,`:1151`).
- **Auth:** SHA-256 key hashes in `RwLock<HashMap>`, model allowlist incl. `*`, `require_admin`, dual-admin quorum with distinct-key enforcement, expiring credentials, hydrate-from-Postgres, dev fixtures gated by `!prod && (allow_fixtures || cfg!(test) || debug_assertions)` (`middleware/auth.rs:38-450`).
- **FinOps:** pre-authorization reservation (UUID) before cache lookup; release on every early-exit path; settle with origin cost; ledger write (`handlers.rs:249-627`).
- **Graceful shutdown:** SIGINT/SIGTERM, `axum::serve(...).with_graceful_shutdown`, drain deadline env (default 15s), exit 75 on overrun, then `state.graceful_drain()` (`main.rs:40-143`).
- **Input validation:** request bodies deserialize into typed structs; model/tenant checks.

## 2. Defects

| ID | Severity | Defect |
|---|---|---|
| F-004 | P0 | Certificate evidence hardcoded at issuance (`handlers.rs:658-666`) |
| F-005 | P0 | Freshness/epochs from request body (`handlers.rs:657,671`) |
| F-010 | P1 | Silent fallback to memory/LSH on backend failure across 4 crates |
| F-015 | P1 | Certificate HMAC key == learning feedback key (`handlers.rs:673`, `state.rs:307-319`) |
| F-020 | P2 | Metrics are process-local atomics presented as Prometheus counters |
| F-022 | P2 | README claims 30s drain + outbox flush; code is 15s default + in-memory drain |
| F-031 | P1 | Cache-hit responses do not set the documented `x-sunborn-cache`/`x-sunborn-latency-ms` headers (grep: not present) |
| F-032 | P2 | `/v1/security/evaluate` returns `conformalScore = heuristic_score` (keyword match) |

## 3. Error handling

- Most handlers return typed `SunbornError` → JSON problem details.
- **Silent `catch {}` equivalents** exist in the frontend and in some backend branches (non-strict fallback). In strict+production mode, failure is closed (good).
- `production_startup_test.rs` genuinely exercises a fail-closed matrix (mock origin, weak secrets, missing KMS).

## 4. Concurrency

- Async Tokio; `DashMap` and `RwLock` used correctly in reviewed paths.
- `run_blocking_async` bridges sync KMS via `block_in_place`/spin (`crypto_shred.rs:530-575`) — potential runtime starvation under KMS latency.
- No deadlock observed in reviewed lock ordering; `WriteBehindFlusher::drop` aborts worker without drain (`write_behind.rs:211-217`) (unwired, still a latent bug).

## 5. Resource management

- Pool: 20 connections, 2s acquire (`postgres.rs`).
- `KEYS` in Dragonfly prefix ops (`dragonfly.rs:218,250`) is O(N) and blocks the server.
- No request body size limits found at the router layer (rely on Axum defaults) — verify.

## 6. Observability

- `/metrics` exposes process-local counters; no OTel exporter wired despite `docs/engineering/OBSERVABILITY_SPECIFICATION.md`.
- Request tracing middleware sets `x-request-id` (`middleware/trace.rs`).

## 7. Verdict

Backend is the strongest layer: real, well-structured, tested. The P0 certificate/freshness issues and the silent-fallback posture keep it from production readiness. Backend score: **7/10**.
