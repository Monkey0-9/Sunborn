# sunborn-effects

Effect-aware agentic partial execution reuse and sub-DAG memoization engine for Sunborn.

## Key Capabilities

1. **4-Tier Effect Classification**: Categorizes agent execution nodes into `Pure`, `ReadOnlyStateful`, `ExternalMutableRead`, and `MutatingAction`.
2. **Sub-DAG Planning & Pruning**: Identifies deterministic intermediate steps within complex multi-step agent workflows and enables safe sub-graph memoization.
3. **Mutation Non-Replay Guarantees**: Enforces strict invariants ensuring external mutating actions (payments, state mutations, webhook dispatches) are never bypassed or falsely replayed.

## Testing

```bash
cargo test -p sunborn-effects
cargo test --doc -p sunborn-effects
```
