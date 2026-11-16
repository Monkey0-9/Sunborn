# 54 — P0 Remediation Status (2026-11-15)

> **HISTORICAL (superseded 2026-11-16).** Distributed FinOps, crash-boundary
> fault injection, and formal verification have since been implemented and
> model-checked. Current status and evidence live in
> [`../STATUS.md`](../STATUS.md).

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
- **Live proof (exact cache):** `DIST_MODE=exact bash
  scripts/test_distributed_invalidation.sh` runs Redis 7, PostgreSQL 16, Qdrant
  and two independent gateway processes. It seeds the same exact-cache key on
  both replicas, records the dependency only on A, mutates on A, and asserts B's
  local cache is purged by the stream event. The run passes with
  `cluster_broadcast_sent=true` and no failed keys.
- **Live proof (shared Qdrant semantic cache):** `DIST_MODE=semantic` writes a
  semantic candidate on A, asserts B serves that shared candidate (one Qdrant
  point), records the dependency only on A, mutates on A, asserts the shared
  Qdrant count drops to zero, and asserts B falls through to origin instead of
  serving stale semantic content.
- **Live proof (disconnect/reconnect replay):** `DIST_MODE=replay` routes B
  through a test TCP proxy, severs the proxy, mutates A while B is disconnected,
  asserts the event remains durable (`XLEN=1`) and B still serves its stale local
  entry, then restarts the proxy and asserts B reconnects, drains the stream, and
  purges the entry.
- That replay gate exposed a real broadcast defect: every replica joined the
  same Redis consumer group, so the stream load-balanced each event to one peer
  instead of delivering it to all peers. A disconnected peer could therefore
  miss an event even though the stream retained it. Consumer groups are now
  namespaced per pod (`sunborn-gateway:<pod_id>`), restoring broadcast semantics
  and per-pod pending replay. New groups start at the stream tail (`$`), so a
  fresh replica does not replay retained history; a restarted replica keeps its
  existing group and replays only its own unacknowledged entries.
- The live run also exposed and fixed two startup defects: concurrent replicas
  raced the migration bookkeeping table (now serialized with a PostgreSQL
  advisory lock), and a fresh Qdrant deployment returned 404 for a missing
  collection during readiness (now treated as zero points, not unavailable).
- Tenant provisioning is now idempotent: an existing persisted TDEK is unwrapped
  instead of rotated, so one replica cannot invalidate another replica's
  encrypted cache entries.
- The live harness is enforced in CI as the `distributed-invalidation-verification`
  job (`.github/workflows/ci.yml`), which runs exact, semantic, replay, and
  outbox modes, so these proofs cannot silently regress.

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
- **Live proof:** `DIST_MODE=outbox` inserts a PENDING event, an expired
  PROCESSING claim owned by a dead worker, and a poison event against PostgreSQL.
  The live gateway worker processes the first two, and the poison event reaches
  `FAILED` with `retry_count=2` and exactly one durable `outbox_dlq` row while
  retaining the failure reason.

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
| In-flight process-kill fault injection | Disconnect/reconnect and expired-lease reclaim are proven, but no test kills a process at the exact ACK/commit boundary | Inject a crash between purge and ACK/commit; assert no loss or duplicate side effects |
| Independent semantic dataset | Still 12 pairs in-repo; no external annotation | Pre-registered held-out set, n ≥ 300, lower confidence bound |
| Distributed FinOps | `SpendVelocityController` remains process-local | Atomic shared reservation/settlement with concurrent replicas |
| Formal verification | No TLA+/Alloy model | Model-check epoch monotonicity, outbox at-least-once, trust monotonicity |
| Full WCAG conformance | Static tests only | axe/Playwright conformance run |
| Documentation reconciliation | 53 documents plus this one still coexist | One SSOT with commit-linked evidence |

---

## 3. Verification

Run on 2026-11-16 (Windows/Git Bash):

| Check | Command | Result |
|---|---|---|
| Workspace compile | `cargo check --workspace --all-targets` | **PASS**, exit 0 |
| Rust tests | `cargo test --workspace --all-targets` | **275 passed, 0 failed**, exit 0 |
| Release gateway latency | `cargo test --release -p sunborn-gateway --test performance_test -- --nocapture` | **14,202.9 req/s, P99 0.144 ms**, exit 0 |
| Live two-replica exact-cache invalidation | `DIST_MODE=exact bash scripts/test_distributed_invalidation.sh` | **PASS** — replica B local cache purged by replica A's Redis Stream event; broadcast clean; provisioning idempotent |
| Live shared-Qdrant semantic invalidation | `DIST_MODE=semantic bash scripts/test_distributed_invalidation.sh` | **PASS** — B served A's shared candidate; after A's mutation Qdrant count reached 0 and B fell through to origin |
| Live Redis disconnect/reconnect replay | `DIST_MODE=replay bash scripts/test_distributed_invalidation.sh` | **PASS** — event durable while B disconnected; B reconnected, replayed the stream, and purged its stale local entry |
| Live PostgreSQL outbox crash/retry/DLQ | `DIST_MODE=outbox bash scripts/test_distributed_invalidation.sh` | **PASS** — pending and expired-claim events processed; poison event retried twice, reached `FAILED`, and produced one DLQ row |
| Frontend | `npm test --prefix frontend` | 37/37 (run before the Rust-only changes) |
| Python SDK | `python -m unittest ...` | 26/26 (run before the Rust-only changes) |
| Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **exit 0, no warnings** |
| Format | `cargo fmt --all -- --check` | **exit 0** |
| Audit | `cargo audit --json` | **0 vulnerabilities** |

**No commit or push was performed. All changes are local for manual review.**
