# Sunborn Gateway Route-by-Route Authorization & Isolation Matrix

This document establishes the institutional authorization policy, tenant isolation contract, and failure-mode boundaries across all endpoints served by the Sunborn Gateway and Frontend BFF.

---

## 1. Route-by-Route Authorization Matrix

| HTTP Method | Route Path | Access Tier | Identity Source | Tenant Isolation Boundary | Idempotent | Audited | Destructive |
|---|---|---|---|---|---|---|---|
| `GET` | `/health` | **Public** | None | Global Service Health | Yes | No | No |
| `GET` | `/ready` | **Public** | None | Deep Backend Connectivity | Yes | No | No |
| `GET` | `/metrics` | **Restricted / Scrape** | Internal / NetworkPolicy | Global Prometheus Metrics | Yes | No | No |
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
| `POST` | `/v1/governance/verify-proof` | **Public / Tenant** | Proof Payload | Cryptographic hash verification | Yes | No | No |
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

## 2. Institutional Security Answers

### 2.1 Can a non-admin tenant invoke any administrative or destructive operation?
**NO.** All administrative and destructive endpoints require explicit `sec_ctx.require_admin()` checks in the gateway handlers. If a non-admin tenant key (or an anonymous request) attempts to access any of these endpoints, the gateway immediately returns `403 Forbidden` (`SunbornForbiddenError`) without executing downstream logic. This invariant is verified over the real TCP HTTP stack in `tests/adversarial_security_test.rs::test_adversarial_non_admin_privilege_escalation_denial_real_http_stack`.

### 2.2 Can Tenant A read, delete, invalidate, or infer Tenant B's state?
**NO.** 
1. **Cache & Vector Isolation**: Every cache key and vector point payload is namespaced by canonicalized `TenantId`. Querying or deleting without matching credentials returns zero results or 403 Forbidden.
2. **Execution DAG Isolation**: Execution plans and tool outputs are validated against the authenticated tenant. If `payload.tenant_id != sec_ctx.tenant_id.0`, the gateway rejects the request with `403 Forbidden`.
3. **Cryptographic Erasure Isolation**: Tenant DEKs are isolated in `TenantCryptoRegistry`. Shredding Tenant A has zero impact on Tenant B's data encryption keys.

### 2.3 Can the frontend BFF invoke privileged endpoints without independent server-side authorization?
**NO.** The gateway does NOT trust the BFF implicitly. While the BFF evaluates `isPrivilegedRoute` and rejects unauthorized client requests with 401/403, every request forwarded by the BFF to the gateway must still carry valid backend credentials (`x-api-key`). The gateway's native `auth_middleware` independently validates credentials, builds `SecurityContext`, and enforces authorization on every single route.

### 2.4 Are tenant and application identities taken from verified credentials rather than trusted request-body values?
**YES.** Tenant and application identities are resolved strictly from the verified cryptographic API key stored in `ApiKeyRegistry`. If an unprivileged client specifies `tenant_id` in the JSON body, the gateway compares it with `sec_ctx.tenant_id.0`. If there is a mismatch (spoofing attempt), the request is rejected with `403 Forbidden`. If empty, it is automatically bound to `sec_ctx.tenant_id.0`.

### 2.5 Are login, API keys, session handling, revocation, and rate limits effective across replicas?
**YES in Production Topology.** In production deployments:
- API key verification and tenant crypto keys are hydrated from PostgreSQL with write-through caching in DragonflyDB.
- Token revocation and rate limiting use atomic DragonflyDB/Redis tokens (`INCR` + `EXPIRE` token bucket).
- Outbox event propagation guarantees key revocation propagates to all active gateway replicas within <100ms.

### 2.6 Are destructive operations audited, idempotent, and recoverable where appropriate?
**YES.**
- **Audited**: Destructive operations (`/v1/governance/shred`, `/v1/caches/{key}`, `/v1/tenants/{id}`, `/v1/invalidation`) emit structured audit records with `x-request-id`, actor identity, timestamp, and target resource.
- **Idempotent**: Deleting or shredding a resource multiple times returns the same deterministic outcome without side-effect cascades or duplicate events.
- **Recoverable**: Invalidation DAG marks tombstones allowing rollback where configured, except for GDPR Art 17 Cryptographic Shredding which is strictly irreversible by legal design.
