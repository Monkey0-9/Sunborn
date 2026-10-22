# Sunborn Gateway Route-by-Route Authorization & Isolation Matrix

This document establishes the institutional authorization policy, tenant isolation contract, and failure-mode boundaries across all endpoints served by the Sunborn Gateway and Frontend BFF.

> **Policy inventory, not security certification.** The current route source contains 41 route declarations and 42 method-path operations. This matrix is not generated from the router and does not prove complete handler coverage. Gate G2 remains open pending generated parity checks and independent adversarial review. The project completion assessment records known gaps.

---

## 1. Route-by-Route Authorization Matrix

| HTTP Method | Route Path | Access Tier | Identity Source | Tenant Isolation Boundary | Idempotent | Audited | Destructive |
|---|---|---|---|---|---|---|---|
| `GET` | `/health` | **Public** | None | Global Service Health | Yes | No | No |
| `GET` | `/ready` | **Public** | None | Deep Backend Connectivity | Yes | No | No |
| `GET` | `/metrics` | **Public at gateway middleware; restrict at network layer** | None in auth middleware | Global Prometheus Metrics | Yes | No | No |
| `POST` | `/v1/auth/login` | **Public (Rate Limited)** | Request Body | Rate Limited by IP (10/min) | Yes | Yes | No |
| `POST` | `/v1/requests` | **Authenticated Tenant** | Verified `x-api-key` | Strict: authenticated tenant only | Yes (read) | Yes | No |
| `POST` | `/v1/chat/completions` | **Authenticated Tenant** | Verified `x-api-key` | Strict: authenticated tenant only | Yes (read) | Yes | No |
| `POST` | `/v1/messages` | **Authenticated Tenant** | Verified `x-api-key` | Strict: authenticated tenant only | Yes (read) | Yes | No |
| `POST` | `/v1/invalidation` | **Admin / Tenant** | Verified `x-api-key` | Tenant-scoped or global (admin) | Yes | Yes | Yes (eviction) |
| `POST` | `/v1/dependencies` | **Cluster Admin** | Verified Admin Key | Cross-source dependency graph | Yes | Yes | No |
| `GET` | `/v1/caches/{key}` | **Cluster Admin** | Verified Admin Key | Raw key inspection | Yes | Yes | No |
| `DELETE` | `/v1/caches/{key}` | **Cluster Admin** | Verified Admin Key | Cache key purging | Yes | Yes | Yes (purge) |
| `POST` | `/v1/execution/plan` | **Authenticated Tenant** | Verified `x-api-key` | Tenant enforced from credential | Yes | Yes | No |
| `POST` | `/v1/execution/results`| **Authenticated Tenant** | Verified `x-api-key` | Tenant enforced from credential | Yes | Yes | No |
| `POST` | `/v1/execution/evict` | **Authenticated Tenant** | Verified `x-api-key` | Tenant enforced from credential | Yes | Yes | Yes (eviction) |
| `POST` | `/v1/shadow/evaluate` | **Tenant / Admin** | Verified `x-api-key` | Tenant-scoped; admin can eval any | Yes | Yes | Yes (drift evict) |
| `GET` | `/v1/chaos/status` | **Authenticated Tenant** | Verified `x-api-key` | Global circuit breaker status | Yes | No | No |
| `POST` | `/v1/chaos/trip` | **Cluster Admin** | Verified Admin Key | Fault injection circuit breaker | Yes | Yes | Yes (disruption) |
| `POST` | `/v1/chaos/reset` | **Cluster Admin** | Verified Admin Key | Restore circuit breaker | Yes | Yes | No |
| `POST` | `/v1/rebuild` | **Cluster Admin** | Verified Admin Key | Global background index rebuild | Yes | Yes | No |
| `DELETE` | `/v1/tenants/{id}` | **Cluster Admin** | Verified Admin Key | Tenant metadata & state purge | Yes | Yes | **Yes (permanent)** |
| `POST` | `/v1/governance/provision` | **Cluster Admin** | Verified Admin Key | Master KEK-wrapped TDEK creation | Yes | Yes | No |
| `POST` | `/v1/governance/shred` | **Cluster Admin** | Verified Admin Key | GDPR Art 17 cryptographic shred | Yes | Yes | **Yes (irreversible)** |
| `POST` | `/v1/governance/attest` | **Authenticated Tenant** | Verified `x-api-key` | RFC 6962 inclusion proof | Yes | Yes | No |
| `POST` | `/v1/governance/verify-proof` | **Authenticated Tenant** | Verified `x-api-key` (gateway middleware) | Proof Payload | Yes | No | No |
| `POST` | `/v1/speculative/predict` | **Authenticated Tenant** | Verified `x-api-key` | Tenant KV cache affinity | Yes | No | No |
| `GET` | `/v1/cluster/status` | **Authenticated Tenant** | Verified `x-api-key` | Node & cluster operational state | Yes | No | No |
| `GET` | `/v1/cluster/sync/status` | **Authenticated Tenant** | Verified `x-api-key` | Vector clocks & outbox lag | Yes | No | No |
| `GET` | `/v1/autonomic/incidents` | **Authenticated Tenant** | Verified `x-api-key` | Incident log & self-healing events | Yes | No | No |
| `GET` | `/v1/autonomic/status` | **Authenticated Tenant** | Verified `x-api-key` | Autonomic controller health | Yes | No | No |
| `GET` | `/v1/finops/report` | **Cluster Admin** | Verified Admin Key | Cross-tenant economic telemetry | Yes | Yes | No |
| `GET` | `/v1/routing/radix/inspect` | **Cluster Admin** | Verified Admin Key | Prefix tree & worker load | Yes | No | No |
| `POST` | `/v1/routing/radix/route` | **Authenticated Tenant** | Verified `x-api-key` | Affinity calculation | Yes | No | No |
| `POST` | `/v1/lifecycle/sweep` | **Cluster Admin** | Verified Admin Key | TTL sweep & tombstone compaction | Yes | Yes | Yes (pruning) |
| `POST` | `/v1/security/evaluate` | **Authenticated Tenant** | Verified `x-api-key` | Injection & PII scanner | Yes | Yes | No |
| `GET` | `/v1/security/audit` | **Cluster Admin** | Verified Admin Key | Tamper-evident audit log | Yes | Yes | No |
| `GET` | `/v1/learning/policies` | **Authenticated Tenant** | Verified `x-api-key` | Read registered policies | Yes | No | No |
| `POST` | `/v1/learning/evaluate` | **Authenticated Tenant** | Verified `x-api-key` | Compute conformal threshold | Yes | No | No |
| `POST` | `/v1/learning/feedback` | **Authenticated Tenant** | Verified `x-api-key` | Ingest labeled cache outcomes | Yes | Yes | No |
| `POST` | `/v1/learning/calibrate` | **Cluster Admin** | Verified Admin Key | Offline conformal risk fitting | Yes | Yes | No |
| `POST` | `/v1/learning/canary` | **Cluster Admin** | Verified Admin Key | Start progressive traffic canary | Yes | Yes | No |
| `POST` | `/v1/learning/promote` | **Cluster Admin** | Verified Admin Key | Promote canary to production | Yes | Yes | No |
| `POST` | `/v1/learning/rollback` | **Cluster Admin** | Verified Admin Key | Emergency revert of active policy | Yes | Yes | Yes (revert) |

---

## 2. Source Review and Evidence Status

### 2.1 Administrative authorization
Handlers contain `require_admin()` checks, and the repository includes non-admin denial tests. This is local source/test evidence, not independent proof that every route/method is covered. The handler-to-route inventory must be generated and checked before G2 closes.

### 2.2 Tenant isolation
The code includes tenant-aware cache identities, request checks, and tests. This document does not establish zero cross-tenant leakage across every storage adapter, error path, cache hit, race, or replica. Independent adversarial testing and multi-tenant staging evidence remain required.

### 2.3 BFF authorization
Both BFF policy and gateway authentication code exist. Review and tests must verify each privileged route and ensure no gateway credential reaches browser-visible code. This matrix does not substitute for that review.

### 2.4 Tenant and application scope
The request handler now binds empty tenant/application fields to `SecurityContext` and rejects supplied values that differ from the credential. Regression coverage is included for tenant and application spoofing. Re-run these tests on the release candidate and include the result in G2 evidence.

### 2.5 Credential lifecycle and cross-replica revocation
**Not established.** The PostgreSQL schema and startup hydration exist, but the gateway has no runtime call sites for credential persistence or revocation. A database revocation does not evict the credential from already-running process registries, and this code does not implement the claimed <100 ms replica propagation. Do not claim revocation or rate-limit consistency across replicas until the lifecycle and distributed invalidation are implemented and measured.

### 2.6 Audit, idempotency, and destructive effects
Source and tests are present, but per-operation audit completeness, retry idempotency, durable event delivery, and end-to-end erasure have not been independently verified. Treat irreversible operations as release blockers until their staging evidence is reviewed.
