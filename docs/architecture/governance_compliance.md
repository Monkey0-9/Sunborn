# Sunborn Enterprise Governance, Compliance & Data Lifecycle

> **Standard Compliance**: GDPR Article 17 (Right to Erasure), CCPA § 1798.105, ISO/IEC 27001, SOC 2 Type II.

---

## 1. Compliance Architecture Overview
Sunborn is designed from first principles to guarantee verifiable tenant isolation and deterministic data lifecycle controls across all caching and storage tiers.

```
[ Administrative Offboarding Event: DELETE /v1/tenants/:id ]
                             │
            ┌────────────────┴────────────────┬────────────────┬────────────────┐
            ▼                                 ▼                ▼                ▼
   PostgreSQL RDBMS                DragonflyDB / RAM       Qdrant Vector      Execution DAG Store
  ├── Tenant Metadata Purged      └── Prefix Eviction    └── Namespace Purge  └── Signature Purge
  ├── Durable Cache Purged            ("tenant:id:*")      (Tenant Vectors)      (Tool Memoization)
  ├── Outbox Queue Purged
  └── Audit Trail Recorded (Cryptographic)
```

---

## 2. Invariant: Zero Lingering Records
When a tenant invokes offboarding or data erasure:
1. **Cascade Erasure**: All partitions across PostgreSQL, exact cache, vector collections, and execution DAG tool memoization are purged atomically.
2. **Audit Immutability**: A structured audit record (`TENANT_OFFBOARDING_ERASURE`) is persisted with timestamp, deleted counts, and actor identity.
3. **Verification**: Automated test `test_tenant_cascade_offboarding_zero_lingering_data` cryptographically verifies 0 residual records across all 4 system tiers.

---

## 3. Automated Lifecycle & TTL Expiration Sweeper
Entries admitted into Sunborn have a bounded TTL (Time-To-Live). The lifecycle sweep API (`POST /v1/lifecycle/sweep`) purges expired entries deterministically to maintain optimal memory and disk footprint.
