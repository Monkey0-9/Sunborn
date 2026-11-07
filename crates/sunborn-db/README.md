# sunborn-db

Authoritative PostgreSQL durability engine, write-behind buffers, and audit log persistence for the Sunborn platform.

## Architecture

1. **Non-Accelerator Authority**: PostgreSQL Write-Ahead Log (WAL) is the single source of truth across all tiers.
2. **Write-Behind Group Commits**: High-throughput memory buffer that batches asynchronous database writes to prevent blocking real-time inference pathways while guaranteeing durability.
3. **Audit & Incident Log**: Tamper-evident recording of security events, administrative policy mutations, and autonomic remediation triggers.

## Testing

```bash
cargo test -p sunborn-db
cargo test --doc -p sunborn-db
```
