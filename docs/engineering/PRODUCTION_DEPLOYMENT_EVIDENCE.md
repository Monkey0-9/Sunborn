# Sunborn: Production Deployment Architecture & SRE Evidence

**Target Standard:** Enterprise Kubernetes & SRE Deployment Hardening  
**Document Version:** 1.0.0  
**Baseline Commit:** `dd25675c5eae200b2518557f966feff74d1556fe`  
**Primary Assets:** [`deploy/`](file:///C:/Sunborn/deploy/)  
**Pre-Flight Script:** [`deploy/verify_live_stack.sh`](file:///C:/Sunborn/deploy/verify_live_stack.sh)  

---

## 1. Service Level Objectives (SLOs), Error Budgets & Recovery Targets

| Metric | Target Objective | Measurement Window | Action on Exhaustion |
|:---|:---:|:---:|:---|
| **Availability (SLO)** | **99.95%** | Rolling 30 days | Freeze production deployments; route traffic to degraded origin fallback |
| **P99 Read Latency (L1 Cache)** | **$< 1.0\text{ ms}$** | 5-minute rolling | Trigger autonomic worker thread scale-up |
| **P99 Read Latency (Gateway)** | **$< 15.0\text{ ms}$** | 5-minute rolling | Autoscale gateway pods via HPA |
| **False Cache Reuse (Safety SLO)** | **Strictly 0.00%** | Absolute (0 occurrences) | PagerDuty Sev-0 incident; instantaneous cache bypass |
| **Recovery Time Objective (RTO)** | **$< 60\text{ seconds}$** | Per Pod / Container Failure | Kubernetes automated pod restart via liveness probes |
| **Recovery Point Objective (RPO)** | **$0\text{ seconds}$** | Storage / Write-Ahead Log | PostgreSQL synchronous WAL replication + outbox durable state |

---

## 2. Container Hardening & Pod Security Standards

Both backend gateway and frontend dashboard containers adhere to the Kubernetes **Restricted Pod Security Standard**:

### Backend Container Hardening ([`deploy/Dockerfile`](file:///C:/Sunborn/deploy/Dockerfile)):
- **Base Image:** Distroless minimal runtime (`gcr.io/distroless/cc-debian12`).
- **Unprivileged UID:** Strictly runs as `USER nonroot:65532`.
- **Read-Only Root Filesystem:** Root `/` mounted as read-only (`readOnlyRootFilesystem: true`). Ephemeral writes restricted to memory `tmpfs` at `/tmp`.
- **Linux Capabilities Dropped:** All Linux capabilities (`CAP_SYS_ADMIN`, `CAP_NET_ADMIN`, etc.) are dropped (`drop: ["ALL"]`).
- **Privilege Escalation:** Disabled (`allowPrivilegeEscalation: false`).
- **Native Health Check:** Docker `HEALTHCHECK` periodically probes `/health` endpoint.

### Frontend Container Hardening ([`deploy/Dockerfile.frontend`](file:///C:/Sunborn/deploy/Dockerfile.frontend)):
- **Base Image:** Alpine Node 22 minimal runner.
- **Unprivileged UID:** Strictly runs as `USER nextjs:1001` (group `nodejs:1001`).
- **Standalone Build:** Next.js standalone server with zero unbundled source files.
- **Native Health Check:** Periodic curl check against `/api/health`.

---

## 3. Kubernetes Deployment Architecture & Network Security

### Ingress & Network Policies ([`deploy/k8s/networkpolicy.yaml`](file:///C:/Sunborn/deploy/k8s/networkpolicy.yaml))
- **Default Deny:** All ingress and egress traffic is denied by default across the namespace.
- **Narrow Port Whitelisting:**
  - Gateway accepts ingress only from Ingress Controller on port 8080.
  - Gateway permitted egress only to PostgreSQL (5432), Dragonfly (6379), Qdrant (6333), and external LLM HTTPS (443).
  - Metrics endpoint (`/metrics`) is strictly excluded from public Ingress rules.

### High Availability & Disruption Budgets ([`deploy/helm/sunborn/values.yaml`](file:///C:/Sunborn/deploy/helm/sunborn/values.yaml))
- **Replica Count:** Minimum 2 replicas deployed across distinct availability zones using `topologySpreadConstraints`.
- **Pod Disruption Budget (PDB):** Configured with `minAvailable: 1`, guaranteeing zero-downtime rolling upgrades.
- **Horizontal Pod Autoscaling (HPA):** Scales up to 10 replicas when CPU exceeds 70% or memory exceeds 80%.

---

## 4. Operational Reproduction Commands

To verify deployment configurations in any environment:

```bash
# 1. Run Pre-flight static verification suite
bash deploy/verify_live_stack.sh

# 2. Lint Helm chart with production values
helm lint deploy/helm/sunborn --values deploy/helm/sunborn/values.yaml

# 3. Dry-run Kubernetes Kustomize installation
kubectl kustomize deploy/k8s | kubectl apply --dry-run=client -f -

# 4. Verify production fail-closed security assertions
cargo test -p sunborn-gateway --test production_startup_test
```
