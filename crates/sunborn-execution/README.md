# sunborn-execution

Deterministic Merkle DAG execution planner and multi-step agentic sub-graph reuse engine for the Sunborn platform.

## Architecture

1. **RFC 6962 Merkle Trees**: Cryptographic hashing across tool inputs, parameters, and parent lineage.
2. **Intermediate Sub-Graph Splicing**: Reuses verified prefix execution nodes in multi-step LLM workflows without re-executing pure deterministic steps.
3. **Side-Effect Mutator Contamination**: Pure read-only steps are safely cached; mutating steps (`write_file`, `send_email`) taint all downstream dependents to guarantee side-effect integrity.

## Testing

```bash
cargo test -p sunborn-execution
cargo test --doc -p sunborn-execution
```
