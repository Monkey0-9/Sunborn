# sunborn-learning

Safe continuous calibration, dual-shadow feedback, and drift monitoring engine for Sunborn.

## Key Capabilities

1. **Dual-Shadow Policy Evaluation**: Runs candidate cache thresholds and policy updates in shadow mode against live production traffic to verify safety before promotion.
2. **Semantic Drift Tracking**: Continuously monitors population drift and token distribution shifts to alert on calibration degradation.
3. **Fail-Closed Safety Gates**: Automatically reverts to conservative baseline policies if shadow evaluation detects precision drops or elevated false-positive reuse.

## Testing

```bash
cargo test -p sunborn-learning
cargo test --doc -p sunborn-learning
```
