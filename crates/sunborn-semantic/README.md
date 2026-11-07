# sunborn-semantic

Non-Exchangeable Conformal Risk Control, online semantic drift tracking, vector similarity search, and linguistic semantic guardrails for the Sunborn platform.

## Architecture & Scientific Contributions

1. **Non-Exchangeable Conformal Risk Control (CRC)**: Statistically bounds false positive rate (incorrect semantic cache hits) below a user-specified error threshold $\alpha$ (e.g., $1.0\%$) using adaptive exponential weight recalibration.
2. **Streaming Welford's OOD Drift Detection**: Real-time tracking of non-stationary query distributions with atomic `f64::to_bits` CAS updates and dynamic threshold tightening.
3. **Multi-Stage Linguistic Guardrails**: Rejects false semantic equivalence resulting from negation inversion ("not guilty" vs "guilty"), antonym flips, numeric shifts, or entity mutations.

## Testing

```bash
cargo test -p sunborn-semantic
cargo test --doc -p sunborn-semantic
```
