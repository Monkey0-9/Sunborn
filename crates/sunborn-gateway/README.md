# sunborn-gateway

High-throughput asynchronous Axum HTTP reverse proxy, OpenAI/Anthropic protocol translator, multi-tenant credential validator, and autonomic self-healing router for the Sunborn platform.

## Architecture

1. **Protocol Compatibility**: Full drop-in wire compatibility with OpenAI (`/v1/chat/completions`) and Anthropic (`/v1/messages`) APIs.
2. **Sub-Millisecond Pipeline**: P99 gateway overhead is $< 500\,\mu\text{s}$ for cache hits with zero runtime thread contention.
3. **Autonomic Incident Management**: Automated anomaly detection, circuit breaking, origin failover arbitrage, and self-healing telemetry.

## Testing

```bash
cargo test -p sunborn-gateway
cargo test --doc -p sunborn-gateway
```
