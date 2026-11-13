# 21 — Execution Gateway Audit

---

## 1. Routing & protocol

- 40 routes (`lib.rs:35-175`).
- OpenAI `/v1/chat/completions` (`handlers.rs:904`) and Anthropic `/v1/messages` (`:1069`) both translate to the internal request and call `execute_request_inner`.
- Streaming: verify whether SSE pass-through is implemented; the origin provider supports `stream` features but the handler path should be confirmed for chunked responses (mark PARTIAL if not exercised by tests).

## 2. Execution lifecycle

```
auth → canonicalize → FinOps reserve → L1 → semantic → origin → persist → certificate → settle
```
Verified in `execute_request_inner` (`handlers.rs:980`).
- Reservation released on cache-hit and abort paths (`:364,375,387,542,553,565,598`).
- Settle with origin token cost (`:611`), ledger write (`:615-627`).

## 3. Origin providers

- `HttpLlmOriginProvider`: real `POST /chat/completions`, retries/backoff (`origin/provider.rs:74-260`).
- `MockOriginProvider`: returns `"[Origin Response from {model} for tenant {t}]: {prompt}"` (`:293-322`) — **default** (`state.rs:298`).
- Production readiness fails closed if origin is mock (`state.rs:474,540,668`).

## 4. Reliability controls

- `Singleflight` dedupe (`origin/singleflight.rs`).
- Circuit breaker on cache (`circuit_breaker.rs`).
- Timeouts on HTTP clients (verify per-call).
- No idempotency keys on mutating operations.
- Metrics process-local.

## 5. Security

- Auth/RBAC/quorum real.
- `x-sunborn-cache` headers absent despite docs.
- Chaos endpoints exist in router (admin-gated) — must verify production gating.

## 6. Verdict

The gateway is the most production-shaped component. With the P0 certificate/freshness fixes and a real default origin, it could credibly serve traffic. Gateway score: **7.5/10**.
