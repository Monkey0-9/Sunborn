# sunborn-threat

Adversarial cache security, threat intelligence, and poisoning defense engine for Sunborn.

## Key Capabilities

1. **Statistical & Semantic Outlier Detection**: Real-time screening for semantic clustering anomalies, embedding drift, and prompt injection signatures.
2. **Quarantine Pool Management**: Sandboxes suspicious cache candidates until corroborated by independent consensus or trusted origin validation.
3. **Cache Extraction Resistance**: Thwarts query-probing and side-channel timing attacks designed to reconstruct private tenant data from cached responses.

## Testing

```bash
cargo test -p sunborn-threat
cargo test --doc -p sunborn-threat
```
