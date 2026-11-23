# sunborn-gateway

High-throughput asynchronous Axum HTTP reverse proxy, OpenAI/Anthropic protocol translator, multi-tenant credential validator, and autonomic self-healing router for the Sunborn platform.

## Architecture

1. **Protocol Compatibility**: Full drop-in wire compatibility with OpenAI (`/v1/chat/completions`) and Anthropic (`/v1/messages`) APIs.
2. **Low-Overhead Pipeline**: Gateway-internal overhead targets P99 $< 500\,\mu\text{s}$ for cache hits, excluding client, network, and origin. The published end-to-end HTTP benchmark measured ~6 ms p50 on a 20-core laptop.
3. **Autonomic Incident Management**: Automated anomaly detection, circuit breaking, origin failover arbitrage, and self-healing telemetry.

## Testing

```bash
cargo test -p sunborn-gateway
cargo test --doc -p sunborn-gateway
```
