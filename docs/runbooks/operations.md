# Sunborn Gateway Production Operations Runbook

> **Target Audience**: Site Reliability Engineers (SRE), Platform Engineers, Cloud Architects.

---

## 1. Service Health & Readiness Probes
The Sunborn gateway provides standard RFC-compliant HTTP probes:
* **Liveness Probe**: `GET /health` -> Returns `200 OK` with version and uptime timestamp.
* **Readiness Probe**: `GET /ready` -> Validates backend cache connectivity and execution engines.
* **Prometheus Metrics**: `GET /metrics` -> Exposes OpenTelemetry/Prometheus counters:
  - `sunborn_requests_total`
  - `sunborn_cache_hits_total{tier="exact"}`
  - `sunborn_cache_hits_total{tier="semantic"}`
  - `sunborn_cache_misses_total`

---

## 2. Standard Deployment Procedures
Deploying to production Kubernetes:
```bash
# Apply ConfigMap, Deployment, Service, HPA, and NetworkPolicy
kubectl apply -f deploy/k8s/
```

Verify rollout status:
```bash
kubectl rollout status deployment/sunborn-gateway -n sunborn-system
```

---

## 3. Incident Management & Remediation

### Scenario A: High Origin Latency / Degradation
1. Check Prometheus metrics for hit rates:
   ```bash
   curl -s http://localhost:8080/metrics | grep sunborn_cache_hits_total
   ```
2. Inspect shadow evaluation drift logs:
   ```bash
   kubectl logs -l app.kubernetes.io/name=sunborn-gateway -n sunborn-system | grep "Shadow drift"
   ```

### Scenario B: Cache Cluster Connection Dropped
1. Circuit breaker automatically opens after 3 consecutive failures to safeguard client latency.
2. Monitor circuit status:
   ```bash
   curl -s http://localhost:8080/v1/chaos/status | jq
   ```
3. Once cache backend cluster recovers, reset circuit breaker:
   ```bash
   curl -s -X POST http://localhost:8080/v1/chaos/reset | jq
   ```

---

## 4. Routine Maintenance
* **Trigger Memory Sweep**:
  ```bash
  curl -s -X POST http://localhost:8080/v1/lifecycle/sweep | jq
  ```
* **Tenant Data Offboarding**:
  ```bash
  curl -s -X DELETE http://localhost:8080/v1/tenants/TENANT_ID | jq
  ```
