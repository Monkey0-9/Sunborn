# 27 — Distributed Systems Audit

---

## 1. Claims vs reality

| Claim | Reality |
|---|---|
| "Guaranteed causal consistency across distributed pods" | `DistributedGraphSyncEngine` uses `tokio::sync::broadcast` in-process; **not referenced by the gateway**; no Redis pub/sub |
| "Durable transactional outbox" | in-process `mpsc` + `Mutex<Vec<DLQ>>`; Postgres outbox methods dead |
| "Monotonic epoch fencing" | in-memory `HashMap<Uuid,AtomicU64>`; advanced only by admin HTTP endpoints |
| "Exactly-once invalidation" | at-most-once in-process; lost on restart |
| "Multi-region failover" | design sketches only |
| "Linearizable freshness" | client-supplied epochs, admin-only bumps |

## 2. Concurrency

- `DashMap` lineage, `RwLock` auth/execution, atomics for metrics.
- No deadlock found in reviewed lock ordering.
- `run_blocking_async` can starve the runtime under KMS latency (`crypto_shred.rs:530-575`).
- `WriteBehindFlusher::drop` aborts worker without drain (`write_behind.rs:211-217`) — latent.

## 3. Ordering & delivery

- Outbox retries with exponential backoff; DLQ retention.
- No dedup/idempotency token on outbox events → replay can double-apply purges (purges are idempotent-ish, but not proven).
- No vector clock propagation across pods (the `VectorClock` type exists but is not used by the sync engine's transport).

## 4. Failure behavior

- Backend failure → silent in-memory fallback (F-010), so the system "succeeds" while partitioned from durable state.
- No network-partition tests; no fault injection beyond chaos endpoint flags.

## 5. Clock assumptions

- `chrono::Utc::now()` for validity windows; no monotonic clock for expiry comparisons. Clock skew between pods can produce inconsistent TTL decisions.

## 6. Verdict

Distributed claims are **not implemented**. The code contains a plausible skeleton but not a distributed system. Distributed score: **3/10**.
