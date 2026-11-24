# sunborn-economics

Real FinOps economic accounting, net utility optimization, and multi-provider pricing engine for Sunborn.

## Key Capabilities

1. **Exact Micro-Unit Amortization**: Evaluates cache lookup and infrastructure amortization against token generation costs across OpenAI, Anthropic, Gemini, and open-source models.
2. **Net Utility Maximization**: Implements utility function $U(q) = \text{SavedCost}(q) - \text{InfraCost} - \text{RiskPenalty}$ to dynamically decide whether reuse is economically accretive.
3. **Breakeven Horizon Analysis**: Computes empirical reuse thresholds to guarantee positive ROI before admitting expensive embeddings to storage.

## Testing

```bash
cargo test -p sunborn-economics
cargo test --doc -p sunborn-economics
```
