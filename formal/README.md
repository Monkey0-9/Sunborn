# Sunborn Formal Verification (`formal/`)

TLA+ specifications and TLC model checker configurations formally verifying the distributed safety invariants of Sunborn.

## Verified Specifications

1. **`PCSR_Safety.tla`**: Verifies that no invalid or tampered execution reuse certificate can pass verification under arbitrary adversarial model perturbations.
2. **`TemporalValidity.tla`**: Verifies that no stale dependency can be served across cluster invalidation epochs under arbitrary event arrival orderings.
3. **`EffectAwareExecution.tla`**: Verifies that external mutating actions (such as transactions or payments) are never skipped, duplicated, or memoized.

## Verification Instructions

```bash
# Verify using TLC (TLA+ Tools)
java -cp /path/to/tla2tools.jar tlc2.TLC PCSR_Safety.tla
```
