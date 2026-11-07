# sunborn-rebuild

Disaster recovery utility to rebuild discardable accelerators (L2 DragonflyDB cache and Qdrant vector indices) directly from authoritative PostgreSQL Write-Ahead Log (WAL) records.

## Usage

```bash
cargo run --bin sunborn-rebuild -- --dry-run
cargo run --bin sunborn-rebuild -- --batch-size 1000
```
