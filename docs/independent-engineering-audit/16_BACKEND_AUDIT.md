# BACKEND AND RUNTIME ARCHITECTURE AUDIT

**Audit Standard**: IEEE 12207 / ISO/IEC 25010 (Reliability, Maintainability & Performance)  
**Core Framework**: Rust 1.96 / Tokio 1.40 / Axum 0.8 / Tower 0.5  
**Auditor**: Independent Principal Backend & Distributed Systems Team  

---

## ⚡ Gateway Architecture & Request Ingress Pipeline

The Sunborn Gateway is implemented in `crates/sunborn-gateway/src/lib.rs` and `handlers.rs`. It runs on top of the Tokio multi-threaded asynchronous runtime.

```
Client Request -> Socket Acceptor (Tokio TCP)
  -> Tower TraceLayer (OpenTelemetry tracing & correlation IDs)
  -> Tower CorsLayer (Permissive or configured origins)
  -> Axum DefaultBodyLimit (Strict 4MB body size cap)
  -> Axum Route Dispatcher
    -> Auth Middleware (RBAC / Model Permissions / Key Expiry)
    -> Rate Limit Middleware (Token Bucket per IP/Tenant)
    -> Handlers (handlers.rs)
      -> FinOps Spend Reservation
      -> Multi-Tier Cache Lookup (L1 -> L2 -> L3)
      -> Invalidation Fencing
      -> Upstream Origin Execution (Singleflight multiplexing)
      -> Write-Behind WAL Hydration
```

---

## 🛡️ Ingress Middleware & Defenses

1. **Authentication Middleware (`middleware/auth.rs`)**:
   - Validates Bearer tokens and `x-api-key` headers against active tenant credentials.
   - Enforces key expiration timestamps: expired credentials return HTTP 401 Unauthorized immediately.
   - Model Permissions: Enforces per-key model whitelists. If a key is provisioned with `models: ["gpt-4o"]`, attempting to invoke `claude-3-5-sonnet` is rejected with HTTP 403 Forbidden.
   - Empty permissions fail closed (zero wildcard escalation).

2. **Rate Limiting Middleware (`middleware/rate_limit.rs`)**:
   - In-memory token-bucket rate limiter tracking requests per minute by client IP and tenant.
   - Verified burst handling and replenishment refill in `tests/login_test.rs`.

3. **Trace & Correlation Middleware (`middleware/trace.rs`)**:
   - Preserves client-supplied `x-request-id` or generates a cryptographically secure UUIDv4 if missing.
   - Propagates `x-request-id` through all outbound logs and origin provider headers.

4. **Singleflight Request Multiplexing (`origin/singleflight.rs`)**:
   - When 50 concurrent requests arrive for the exact same uncached prompt, Sunborn issues **exactly one** upstream socket connection to OpenAI/Anthropic.
   - A multi-consumer broadcast channel fans out the streaming tokens to all 49 awaiting follower requests in real-time, eliminating redundant origin costs and duplicate network egress.

---

## 🛑 Failure Handling & Circuit Breakers

1. **Dragonfly/Redis Circuit Breaker**:
   - Monitored via sliding window error counts. If DragonflyDB becomes unreachable or times out consecutively, the circuit trips to `Open` state.
   - In `Open` state, requests bypass the distributed cache and fall through to in-memory cache or origin execution without blocking the Tokio reactor.
   - Automatic transition to `HalfOpen` after configured cooldown allows seamless self-healing.
2. **PostgreSQL WAL Circuit Breaker**:
   - If PostgreSQL goes down, the asynchronous write-behind buffer captures events into an in-memory ring buffer (up to 10,000 records).
   - If buffer capacity is exceeded, critical writes block or fail closed according to tenant reliability policy.
