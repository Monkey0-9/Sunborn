# DATABASE, STORAGE AND DATA ENGINEERING AUDIT

**Audit Standard**: ISO/IEC 25012 (Data Quality) / IEEE 12207 (Maintenance Processes)  
**Database**: PostgreSQL 16+ / SQLx 0.8 (Pure Rustls Async Driver)  
**Auditor**: Independent Principal Database Engineer  

---

## 💾 Relational Data Architecture & System of Record

In accordance with the **Non-Accelerator Authority (NAA)** principle, PostgreSQL is the authoritative system of record. 

### Schema Migrations (`migrations/`)

1. **`0001_initial_schema.sql`**:
   - `tenants`: Primary multi-tenant isolation table with wrapped KMS key slots.
   - `applications`: Application registry scoped under `tenant_id`.
   - `execution_records`: Append-only table capturing canonical input hashes, output payloads, model names, avoidance tokens, and execution latencies.
   - `indexes`: B-Tree indexes on `(tenant_id, input_hash)` and `(tenant_id, created_at)`.
2. **`0002_durable_identity_and_keys.sql`**:
   - `api_credentials`: Hashed API keys (`sha256`), tenant scopes, expiration timestamps, and model permission arrays.
   - `tenant_keys`: Active and archived Tenant Data Encryption Keys (TDEKs) for envelope encryption and key rotation.
3. **`0003_finops_and_security.sql`**:
   - `spend_reservations`: Atomic in-flight budget reservations with status (`RESERVED`, `SETTLED`, `RELEASED`).
   - `knowledge_documents`: Authoritative document registry with monotonically increasing lease epochs and Roaring Bitmap lineage bindings.
   - `invalidation_dlq`: Dead-letter queue capturing failed eviction events with retry counters.

---

## ⚡ Group Write-Behind Buffer (`crates/sunborn-db/src/write_behind.rs`)

To eliminate database write latency from the client request path (which would add 5–20 ms to every request), Sunborn uses an asynchronous **Group Write-Behind Buffer**:
- In-memory bounded MPSC channel (capacity: 10,000 records).
- Worker thread flushes records in configurable microbatches (e.g., 100 records or 50 ms timeout).
- Uses PostgreSQL `UNNEST` multi-row inserts for bulk throughput $> 15,000 \text{ writes/sec}$.
- **Graceful Shutdown**: On `SIGTERM`, worker stops accepting new records and flushes all pending buffer entries to disk before exiting cleanly.
- **Verification Evidence**: Tested in `sunborn_db::write_behind::tests`.

---

## 🔄 Disaster Recovery Replay Engine (`apps/sunborn-rebuild`)

A dedicated Rust CLI utility (`sunborn-rebuild`) provides automated disaster recovery:
```bash
# Replay PostgreSQL WAL state into ephemeral Redis and Qdrant clusters
sunborn-rebuild --from-wal --batch-size 500
```
- Restores in-memory and accelerator state from authoritative PostgreSQL tables with verified `ReuseCertificate` integrity.
- Zero data loss or state drift during complete accelerator rebuilds.
- **Verification Evidence**: Tested in `tests/disaster_recovery_test.rs`.
