# 24 — Database & Storage Audit

---

## 1. Schema

`migrations/0001-0003` + inline DDL in `postgres.rs:176-360` create ~15 tables:
tenants, durable_cache_entries, outbox_events, audit_events, tenant_wrapped_keys, api_credentials, finops_ledger, prefix_affinity_records, decision_records, applications, knowledge_sources, document_versions, dependency_edges, invalidation_outbox.

- Primary keys/foreign keys present.
- Indexes: verify per-table; some hot paths (tenant+app) need composite indexes.

## 2. Access layer

- `sqlx` 0.8, Postgres, rustls.
- Pool: 20 connections, 2s acquire timeout.
- All CRUD uses parameterized binds (no SQL injection found).
- `DbCircuitBreaker` with open/cooldown (`postgres.rs:35-75`).
- `SELECT 1` health with URL redaction (`:13-27`).

## 3. Durability problems

| Issue | Evidence |
|---|---|
| Silent fallback to `MemoryDatabaseStore` on any Postgres error | `postgres.rs:604,692,793,884,928,1001` |
| `strict_durability` defaults **false** | `config.rs:49` |
| Only strict+production returns an error | `handlers.rs:804-809` |
| Durable outbox methods never called | `insert_outbox_event`/`fetch_pending_outbox_events`/`mark_outbox_processed` |
| Write-behind never instantiated | `write_behind.rs` |

**Consequence:** a non-strict deployment can appear healthy while persisting nothing.

## 4. "WAL engine" claim

There is no WAL tailing, logical replication, LSN, or write-ahead log consumer. It is ordinary OLTP. The `001_initial_schema.sql` comment "WAL authority" is marketing.

## 5. Persistence across restart

- If `database_backend=postgres` and strict, durable entries survive.
- In-memory mode loses everything.
- Execution store is memory-only → DAG memoization lost on restart.
- Invalidation outbox/DLQ lost on restart.

## 6. Transactions & concurrency

- Per-statement writes; no explicit multi-statement transaction around "persist + outbox insert + certificate". A crash between steps can leave inconsistent state.
- No optimistic concurrency / version checks on cache rows.
- No tenant row-level security; isolation enforced in application queries.

## 7. Recovery

- `apps/sunborn-rebuild` genuinely scans DB entries and re-populates cache/vector (`main.rs:118-160`). Good.
- No backup/restore automation; runbooks are manual.

## 8. Verdict

Real, parameterized Postgres integration with good structure, undermined by silent fallback, absent transactions, and dead durable-outbox wiring. Storage score: **5.5/10**.
