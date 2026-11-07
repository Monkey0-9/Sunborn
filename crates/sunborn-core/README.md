# sunborn-core

Core domain models, FinOps economic utility scoring, tenancy identifiers, error taxonomies, and configuration primitives for the Sunborn platform.

## Key Capabilities

1. **Tenancy & Identification**: Strongly typed `TenantId`, `ApplicationId`, and `RequestEnvelope` structures preventing multi-tenant isolation breaches.
2. **FinOps Economic Scoring**: Net-utility cost controllers calculating cache amortized breakeven against LLM origin API tariffs (input/output tokens saved vs memory/lookup overhead).
3. **Reproducibility Certificates**: Cryptographic validation models certifying empirical benchmark reproduction with canonical SHA-256 digests.
4. **Vector Clock & Causality**: Distributed Lamport and vector clock abstractions for tracking data causality across distributed replicas.

## Testing

```bash
cargo test -p sunborn-core
cargo test --doc -p sunborn-core
```
