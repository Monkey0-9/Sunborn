# Sunborn Formal Verification (`formal/`)

TLA+ specifications and TLC model checker configurations formally verifying the distributed safety and liveness invariants of Sunborn.

## Verified Specifications

### Core Distributed Invariants
1. **`EpochFencing.tla`**: Verifies that vector-clock causal fencing prevents stale reads and writebacks across distributed gateway replicas during replication lag.
2. **`OutboxAtLeastOnce.tla`**: Verifies that transactional outbox events are delivered at-least-once to peer replicas even under process crash-recovery cycles.
3. **`OutboxLiveness.tla`**: Verifies bounded liveness, guaranteeing that pending outbox events eventually drain to zero without permanent deadlocks.

### Crate Invariant Specifications
4. **[`reuse_safety/PCSR_Safety.tla`](./reuse_safety/)**: Proves that no invalid or tampered Proof-Carrying Semantic Reuse (PCSR) certificate can pass verification under arbitrary adversarial model perturbations.
5. **[`temporal_validity/TemporalValidity.tla`](./temporal_validity/)**: Proves that no stale dependency can be served across cluster invalidation epochs under arbitrary event arrival orderings.
6. **[`execution_effects/EffectAwareExecution.tla`](./execution_effects/)**: Proves that external mutating actions (financial transactions, webhook dispatches, state mutations) are never skipped, duplicated, or memoized.

## Verification Instructions

Run all formal models using the in-tree automation script:

```bash
# On Linux / macOS / Git Bash
bash scripts/run_tlaplus.sh

# On Windows PowerShell
powershell -File scripts/run_tlaplus.ps1

# Direct execution via TLC model checker
java -cp formal/tla2tools.jar tlc2.TLC formal/EpochFencing.tla
```
