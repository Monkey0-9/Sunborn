# sunborn-cache

Tiered L1 in-memory and L2 distributed cache layer with FinOps Greedy Dual-Size Frequency (GDSF) cost-aware eviction policies for Sunborn.

## Key Capabilities

1. **Tiered L1 / L2 Cache**: Sub-microsecond local in-memory store backed by asynchronous read-through and write-through L2 DragonflyDB (Redis 7 protocol compatible).
2. **FinOps GDSF Eviction**: Cost-aware cache eviction weighting frequency, byte footprint, and token recomputation savings against memory pressure.
3. **Resilience & Circuit Breaking**: Circuit breaker wrapping remote cache instances with automatic fallback to origin execution during network partitions.

## Testing

```bash
cargo test -p sunborn-cache
cargo test --doc -p sunborn-cache
```
