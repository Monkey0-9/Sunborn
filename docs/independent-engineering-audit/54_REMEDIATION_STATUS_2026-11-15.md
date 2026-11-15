# 54 — P0 Remediation Status (2026-11-15)

**Status: remediation in progress. Release decision remains NO-GO until the
acceptance evidence below is produced on live infrastructure.**

This document tracks the code changes made after audit 53. It is deliberately
narrow: it does not re-score the project or claim that the remaining P0/P1 items
are closed without evidence.

---

## 1. Implemented in this pass

### P0-01 — Epoch authority is fail-closed and monotonic
- `EpochPersistenceBackend::advance_epoch` replaces the best-effort
  `persist_epoch`; it returns the authoritative epoch.
- `EpochLeaseManager::bump_epoch` now returns `Result<u64>` and propagates
  persistence errors. It refuses to advance before hydration succeeds.
- PostgreSQL uses `GREATEST(current_epoch + 1, proposed) ... RETURNING` so
  concurrent replicas cannot overwrite a newer epoch with a stale value.
- `AppState::verify_production_readiness` hydrates epochs synchronously and
  fails startup if the authoritative store is unavailable. The old
  fire-and-forget background hydration is removed.
- `observe_epoch` applies peer epochs monotonically to the local fence.

### P0-02/P0-03 — Durable cluster stream and cross-replica vector purge
- Redis **Pub/Sub** was replaced with Redis **Streams** + consumer group
  (`XGROUP`/`XREADGROUP`/`XACK`), including pending-message replay on reconnect.
- Events are acknowledged only after the local purge succeeds; failed purges stay
  pending.
- Messages carry `mutated_cache_keys`, so a peer with a process-local dependency
  graph can delete the same exact-cache keys and Qdrant vector points.
- The receiving replica applies the origin's `monotonic_epoch` to its local
  epoch manager before accepting future writes.
- Production readiness rejects an in-memory cluster transport
  (`cluster_transport_durable == false`).
- **Live proof (exact cache):** `scripts/test_distributed_invalidation.sh` runs
  Redis 7, PostgreSQL 16, Qdrant and two independent gateway processes. It seeds
  the same exact-cache key on both replicas, records the dependency only on A,
  mutates on A, and asserts B's local cache is purged by the stream event. The
  run passes with `cluster_broadcast_sent=true` and no failed keys.
- The live run also exposed and fixed two startup defects: concurrent replicas
  raced the migration bookkeeping table (now serialized with a PostgreSQL
  advisory lock), and a fresh Qdrant deployment returned 404 for a missing
  collection during readiness (now treated as zero points, not unavailable).
- Tenant provisioning is now idempotent: an existing persisted TDEK is unwrapped
  instead of rotated, so one replica cannot invalidate another replica's
  encrypted cache entries.
- The live harness is enforced in CI as the `distributed-invalidation-verification`
  job (`.github/workflows/ci.yml`), so this proof cannot silently regress.

### P0-04 — Durable outbox
- Migration `0006_durable_outbox_queue.sql` creates the missing `outbox_events`
  table and claim/retry/DLQ columns.
- `claim_pending_outbox_events` uses `FOR UPDATE SKIP LOCKED`; leases expire after
  five minutes so a crashed worker does not strand events.
- `mark_outbox_failed` increments a durable retry count, returns the event to
  PENDING, and inserts into `outbox_dlq` after the maximum attempts.
- The admin invalidation path writes a PENDING event **before** purging, marks it
  PROCESSED only after a fully successful purge, and returns an error if the
  durable acknowledgement fails.
- The worker no longer ignores purge errors or marks failed events processed.

### P0-05 — Rebuild trust
- A malformed `SUNBORN_ATTESTATION_PUBLIC_KEY` now fails immediately; missing
  verification is never treated as success.
- Production rebuild requires an explicit public key and never derives a key from
  a public default seed.
- Rebuild verifies signature, Merkle root, tenant/application/model binding, and
  source snapshots against authoritative stored epochs before admission.
- Records without a certificate or with stale snapshots are skipped (not
  admitted with reduced trust).

### P0-06 — Purge reporting
- Purge failures now carry the structured `PurgeReport` in the error details.
- Cluster broadcast failure is recorded as a failed operation and makes the
  result unsuccessful.
- Dependency edges are removed per-key only when that key's cache and vector
  deletions both succeeded.

### P1 items addressed
- `HttpCrossEncoderBackend` fails closed on HTTP errors, parse errors, missing
  `model_hash`, and hash mismatch. The active `DecisionEngine` accepts an
  optional backend and abstains with `CROSS_ENCODER_UNAVAILABLE` /
  `CROSS_ENCODER_CONTRADICTION`.
- Helm no longer defaults to `local-aead`; it requires an AWS KMS ARN (or Vault
  configuration) and production readiness rejects local software keys.
- The reproducibility latency gate runs `cargo test --release`, so the 5 ms
  production SLO is not compared against a debug-profile measurement.
- README release badges/claims were replaced with the audit NO-GO status.

---

## 2. Still open (do not claim production readiness)

| Item | Why it is still open | Required evidence |
|---|---|---|
| Live multi-process cluster proof (exact cache) | **PASS** — exact-cache peer purge is now proven by `scripts/test_distributed_invalidation.sh`; disconnect/reconnect and crash replay are still untested | Forced Redis disconnect/reconnect; kill A mid-broadcast; assert replay |
| Live Qdrant cross-replica purge | Vector deletion still exercised only with `MemoryVectorStore`; the live gate disables semantic hits so it proves the exact-cache path | Shared Qdrant, mutate on A, assert B's semantic search cannot return the stale point |
| Live PostgreSQL outbox crash test | Memory implementation tested; SQL schema/claim not run against Postgres | Kill worker between claim and ack; assert retry/DLQ |
| Independent semantic dataset | Still 12 pairs in-repo; no external annotation | Pre-registered held-out set, n ≥ 300, lower confidence bound |
| Distributed FinOps | `SpendVelocityController` remains process-local | Atomic shared reservation/settlement with concurrent replicas |
| Formal verification | No TLA+/Alloy model | Model-check epoch monotonicity, outbox at-least-once, trust monotonicity |
| Full WCAG conformance | Static tests only | axe/Playwright conformance run |
| Documentation reconciliation | 53 documents plus this one still coexist | One SSOT with commit-linked evidence |

---

## 3. Verification

Run on 2026-11-15 (Windows/Git Bash):

| Check | Command | Result |
|---|---|---|
| Workspace compile | `cargo check --workspace --all-targets` | **PASS**, exit 0 |
| Rust tests | `cargo test --workspace --all-targets` | **275 passed, 0 failed**, exit 0 |
| Release gateway latency | `cargo test --release -p sunborn-gateway --test performance_test -- --nocapture` | **14,202.9 req/s, P99 0.144 ms**, exit 0 |
| Live two-replica invalidation | `bash scripts/test_distributed_invalidation.sh` | **PASS** — replica B local cache purged by replica A's Redis Stream event; broadcast clean; tenant provisioning idempotent |
| Frontend | `npm test --prefix frontend` | 37/37 (run before the Rust-only changes) |
| Python SDK | `python -m unittest ...` | 26/26 (run before the Rust-only changes) |
| Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **exit 0, no warnings** |
| Format | `cargo fmt --all -- --check` | **exit 0** |
| Audit | `cargo audit --json` | **0 vulnerabilities** |

**No commit or push was performed. All changes are local for manual review.**
