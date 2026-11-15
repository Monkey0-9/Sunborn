# Database, Storage & Persistence Audit

**Primary Store:** PostgreSQL 16 (Deadpool Connection Pooling)  
**Migration Authority:** Sequential SQL Runner (`_sunborn_migrations`)  
**Outbox Engine:** Transactional Outbox with Exponential Backoff & DLQ  
**L2 Cache Engine:** Dragonfly / Redis 7+ Multi-Node  

---

## 1. Singular Database Migration Authority

In the 5.2 baseline, database schema authority was fractured between an inline raw SQL string in `postgres.rs` and loose files in `migrations/`. This created schema drift and broken rollbacks.

### 1.1 Consolidated Migration Architecture
Schema migrations are now executed exclusively via the authoritative sequential runner:
- `migrations/0001_initial_schema.sql`: Core tenant tables, applications, API credentials.
- `migrations/0002_durable_identity_and_keys.sql`: Tenant encryption keys and DEK metadata.
- `migrations/0003_finops_and_security.sql`: Spend velocity limits and audit logs.
- `migrations/0004_durable_cache_certificate.sql`: Ed25519 attestation certificates.
- `migrations/0005_persistent_epochs_and_dlq.sql`: Monotonic source epochs and dead-letter queues.

Each migration computes a SHA-256 checksum recorded in `_sunborn_migrations`. Any tampering or missing sequence numbers halts server initialization immediately.

---

## 2. Transactional Outbox Background Worker

To prevent invalidation event loss under database crash scenarios:
```rust
// Background outbox flusher loop in state.rs
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        interval.tick().await;
        if let Ok(pending) = db.fetch_pending_outbox_events(50).await {
            for record in pending {
                // Purge across exact cache, Qdrant, and Roaring graph
                let _ = orchestrator.purge_sources(tenant_id, vec![source_id]).await;
                let _ = db.mark_outbox_event_processed(record.event_id).await;
            }
        }
    }
});
```
