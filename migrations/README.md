# Sunborn PostgreSQL Migrations (`migrations/`)

Authoritative PostgreSQL database schema definitions and versioned migrations forming the system-of-record storage layer for Sunborn.

## Migration Sequence

| Migration | Description |
|---|---|
| `0001_initial_schema.sql` | Core schema: tenants, applications, credentials, policies, cache entries, and metrics. |
| `0002_durable_identity_and_keys.sql` | SHA-256 hashed API key lifecycle, rotation metadata, and tenant-scoped access policies. |
| `0003_finops_and_security.sql` | FinOps spend allocations, velocity ceilings, and security incident audit events. |
| `0004_durable_cache_certificate.sql` | Cryptographic attestation certificates and Merkle proof JSON persistence. |
| `0005_persistent_epochs_and_dlq.sql` | Monotonic vector clock epochs and invalidation dead-letter queue (DLQ) tables. |
| `0006_durable_outbox_queue.sql` | Transactional outbox pattern for guaranteed at-least-once invalidation broadcasting. |
| `0007_distributed_spend_ledger.sql` | Advisory-locked transactional spend reservations and settlements. |
| `0008_free_tier_quotas.sql` | Per-tenant daily/monthly request quotas and aggregate free-pool ceilings. |
| `0009_cluster_rate_limits.sql` | Cluster-wide token bucket rate limiting tables for multi-replica coordination. |
| `0010_financial_precision_micro_units.sql` | 64-bit integer micro-unit ($10^{-6}$ USD) and MicroPaise ($10^{-4}$ INR) accounting. |
| `0011_micro_units_non_negative_checks.sql` | Check constraints guaranteeing non-negative spend ledgers and token bounds. |
| `0012_durable_erasure_and_learning.sql` | Cryptographic shredding logs (DPDP/GDPR Art. 17) and dual-shadow policy tables. |

## Execution & Rollout

Migrations are applied automatically by `crates/sunborn-db` using SQLx embedded migrations upon gateway startup when PostgreSQL is configured.

```bash
# Manual migration verification using sqlx-cli
sqlx migrate run --source migrations
```
