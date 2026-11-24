# sunborn-freshness

Multi-dimensional causal freshness and temporal validity engine for Sunborn.

## Key Capabilities

1. **Multi-Vector Validity Model**: Evaluates validity across five distinct vectors: document state ($D_t$), policy rules ($P_t$), application config ($A_t$), model version ($M_t$), and execution epoch ($E_t$).
2. **Temporal Confidence Decay**: Continuous half-life and linear confidence decay for time-sensitive domains (financial feeds, real-time news, inventory records).
3. **Targeted Causal Invalidation**: Granular dependency graphs enabling precision invalidation of affected cache entries without global cache flushes.

## Testing

```bash
cargo test -p sunborn-freshness
cargo test --doc -p sunborn-freshness
```
