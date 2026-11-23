# sunborn-invalidation

Transitive dependency graph invalidation, Roaring Bitmap SIMD lineage propagation, and transactional outbox event streams for the Sunborn platform.

## Architecture

1. **Transactional Outbox Event Processing**: Ingestion of data mutation events with deduplication and order preservation.
2. **Roaring Bitmap Cascades**: Bit-parallel transitive closure invalidation over lineage graphs using compressed 64-bit Roaring Treemaps.
3. **Multi-Source Watermarking**: Vector-clock causal fencing preventing stale cache writeback during distributed replication lag.

## Testing

```bash
cargo test -p sunborn-invalidation
cargo test --doc -p sunborn-invalidation
```
