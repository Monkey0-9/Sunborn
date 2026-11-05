# Sunborn Multi-Region High-Availability & Disaster Recovery Architecture

**Document Identifier**: `SUNBORN-OPS-MR-2026.1`  
**Classification**: Enterprise Infrastructure & Disaster Recovery Specification  
**Standard Compliance**: ISO/IEC 27001:2022 A.8.14, NIST SP 800-34 Rev. 1  
**Status**: Validated & Active  

---

## 1. Multi-Region Architectural Topology

Sunborn is deployed across two geographically isolated cloud regions to guarantee business continuity, data residency compliance (GDPR), and automatic disaster failover:

```
┌────────────────────────────────────────────────────────────────────────┐
│                        GLOBAL ROUTING LAYER                            │
│           (Anycast DNS / Cloudflare / AWS Route 53 Health Checks)      │
└───────────────────┬────────────────────────────────┬───────────────────┘
                    │                                │
            (Primary / Active)             (Secondary / Standby)
                    │                                │
┌───────────────────▼──────────────┐ ┌───────────────▼───────────────────┐
│       REGION 1: US-EAST-1        │ │       REGION 2: EU-CENTRAL-1      │
│  • Ingress Gateway (3 Replicas)  │ │  • Ingress Gateway (3 Replicas)   │
│  • L1 Memory + L2 Dragonfly      │ │  • L1 Memory + L2 Dragonfly       │
│  • Qdrant Vector Index (Active)  │ │  • Qdrant Vector Index (Standby)  │
│  • AWS KMS (mrk-sunborn-master)  │ │  • AWS KMS (mrk-sunborn-replica)  │
│  • PostgreSQL Primary (Read/Write)│ │  • PostgreSQL Read-Replica (WAL) │
└───────────────────┬──────────────┘ └───────────────▲───────────────────┘
                    │                                │
                    └────── Streaming Replication ───┘
                             (WAL Streaming)
```

---

## 2. Invariants & Split-Brain Mitigations

### 2.1 Single-Writer Database Authority (RPO = 0)
* **Authoritative Leader**: PostgreSQL in `us-east-1` is the sole authoritative system of record for writes.
* **Secondary Replica**: `eu-central-1` operates as a hot standby streaming replica with synchronous replication (`synchronous_commit = on`).
* **Split-Brain Immunity**: Automatic failover relies on Patroni with an odd-numbered etcd consensus cluster (3 AZs). A partitioned secondary node cannot self-promote without quorum confirmation.

### 2.2 Multi-Region Cryptographic Envelopes (Remote KMS)
* Master KEKs use AWS KMS Multi-Region Keys (`mrk-`) or Vault replicated transit engines.
* The Tenant Data Encryption Key (TDEK) wrapped under `us-east-1` master KEK can be unwrapped by the secondary gateway using `eu-central-1` replica key (`mrk-sunborn-replica`).
* This eliminates the need to synchronize plaintext key material across cloud regions.

### 2.3 Data Residency & GDPR Regional Partitioning
* European tenants requiring strict EU data residency (`residency: "eu"`) have their durable cache entries, TDEK wrappers, and Qdrant embeddings pinned strictly to `eu-central-1`.
* Requests from EU tenants are routed directly to `eu.api.sunborn.ai`, enforcing zero cross-border replication for regulated data sets.

---

## 3. Automated Failover & Failback Runbook

### Step 1: Health Check Detection
Global traffic routers continuously poll `/ready` every 5 seconds. If all three gateway replicas in `us-east-1` report non-200 or connection timeout for 3 consecutive intervals (15 seconds):
1. Anycast router marks `us-east-1` unhealthy.
2. 100% of global traffic shifts to `eu-central-1`.

### Step 2: Standby Promotion Procedure
In an unrecoverable primary region outage:
```bash
# 1. Promote secondary PostgreSQL read-replica to primary
kubectl exec -it -n sunborn postgres-eu-0 -- patronictl failover --candidate postgres-eu-0

# 2. Update secondary gateway cluster role to primary
kubectl set env -n sunborn deployment/sunborn-gateway-eu-central \
  SUNBORN_CLUSTER_ROLE=primary \
  SUNBORN_POSTGRES_URL="postgres://app_user@postgres-eu-primary.sunborn.svc:5432/sunborn_prod"

# 3. Rollout restart gateway pods to establish write-through pipeline
kubectl rollout restart -n sunborn deployment/sunborn-gateway-eu-central
kubectl rollout status -n sunborn deployment/sunborn-gateway-eu-central
```

### Step 3: Measured Recovery Envelopes
* **Recovery Point Objective (RPO)**: **0 seconds** (synchronous WAL streaming).
* **Recovery Time Objective (RTO)**: **< 30 seconds** (15s health check detection + 10s DNS propagation + 5s role promotion).
