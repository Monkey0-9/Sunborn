# Sunborn Gateway Production Operations Runbook

> **Target Audience**: Site Reliability Engineers (SRE), Platform Engineers, Cloud Architects, Security Operations.
> **Security Baseline**: All administrative mutations and diagnostic endpoints require valid administrative credentials (`SUNBORN_ADMIN_KEY`).

---

## 1. Authentication & Security Prerequisite

Sunborn strictly implements zero-trust fail-closed authorization. All administrative, chaos, lifecycle, and tenant management endpoints require an administrative API key.

Before executing any administrative operations, securely retrieve the cluster admin key:

```bash
# Retrieve provisioned administrative key from Kubernetes Secrets
export SUNBORN_ADMIN_KEY=$(kubectl get secret -n sunborn-system sunborn-gateway-secrets \
  -o jsonpath='{.data.SUNBORN_ADMIN_KEY}' | base64 -d)

# Verify key is non-empty
if [ -z "${SUNBORN_ADMIN_KEY}" ]; then
  echo "ERROR: SUNBORN_ADMIN_KEY secret is not provisioned or empty!" >&2
  exit 1
fi
```

---

## 2. Service Health, Readiness & Internal Metrics

The Sunborn gateway provides standard RFC-compliant HTTP probes:

* **Liveness Probe**: `GET /health` -> Returns `200 OK` with version and uptime timestamp.
  ```bash
  curl -fsS http://localhost:8080/health | jq
  ```
  *Expected Output:*
  ```json
  {"service":"sunborn-gateway","status":"healthy","version":"0.1.0"}
  ```

* **Readiness Probe**: `GET /ready` -> Validates backend cache connectivity and execution engines.
  ```bash
  curl -fsS http://localhost:8080/ready | jq
  ```
  *Expected Output:*
  ```json
  {"ready":true,"dragonfly":"healthy","postgres":"healthy","qdrant":"healthy"}
  ```

* **Prometheus Operational Metrics**: `GET /metrics` -> Exposes OpenTelemetry/Prometheus counters.
  > [!IMPORTANT]
  > `/metrics` is an internal telemetry endpoint. In accordance with P0.2 security governance, it is **excluded from public Ingress** and accessible only internally within the Kubernetes cluster by authorized Prometheus scraping or via secure port-forwarding:
  ```bash
  # Cluster-internal scrape or port-forwarded access
  curl -fsS http://localhost:8080/metrics | grep -E "sunborn_requests_total|sunborn_cache_hits_total"
  ```
  *Key Metrics:*
  - `sunborn_requests_total`: Total processed AI inference requests.
  - `sunborn_cache_hits_total{tier="exact"}`: Tier 0/1 deterministic hash hits.
  - `sunborn_cache_hits_total{tier="semantic"}`: Tier 2 conformal semantic retrieval hits.
  - `sunborn_cache_misses_total`: Fallthrough cache misses dispatched to origin LLMs.

---

## 3. Standard Deployment & Rollout Procedures

Deploying to production Kubernetes via Helm or raw manifests:

```bash
# 1. Option A: Deploy via Helm with external secret reference
helm upgrade --install sunborn deploy/helm/sunborn \
  --namespace sunborn-system \
  --create-namespace \
  --set secrets.existingSecret="sunborn-gateway-secrets"

# 2. Option B: Apply raw K8s manifests
kubectl apply -f deploy/k8s/

# 3. Monitor rollout status to completion
kubectl rollout status deployment/sunborn-gateway -n sunborn-system --timeout=120s
```

Rollback procedure if healthchecks fail:
```bash
helm rollback sunborn -n sunborn-system
# Or with kubectl:
kubectl rollout undo deployment/sunborn-gateway -n sunborn-system
```

---

## 4. Incident Management & Remediation

### Scenario A: High Origin Latency or Fallback Spikes
1. Check Prometheus metrics for cache hit and miss rates:
   ```bash
   curl -fsS http://localhost:8080/metrics | grep sunborn_
   ```
2. Inspect shadow evaluation drift logs:
   ```bash
   kubectl logs -l app.kubernetes.io/name=sunborn-gateway -n sunborn-system --tail=100 | grep "Shadow drift"
   ```

### Scenario B: Cache Cluster Connection Dropped
1. Circuit breaker automatically opens after 3 consecutive failures to safeguard client latency without raising HTTP 500 errors.
2. Monitor circuit breaker status (requires authentication):
   ```bash
   curl -fsS http://localhost:8080/v1/chaos/status \
     -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq
   ```
   *Expected Output:*
   ```json
   {
     "circuit_status": "Open"
   }
   ```
3. Once the cache backend cluster recovers, manually reset the circuit breaker:
   ```bash
   curl -fsS -X POST http://localhost:8080/v1/chaos/reset \
     -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq
   ```
   *Expected Output:*
   ```json
   {
     "circuit_status": "Closed",
     "status": "reset"
   }
   ```

---

## 5. Routine Maintenance & Tenant Offboarding

* **Trigger FinOps Cache Memory Sweep**:
  ```bash
  curl -fsS -X POST http://localhost:8080/v1/lifecycle/sweep \
    -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq
  ```
  *Expected Output:*
  ```json
  {
    "status": "completed",
    "swept_entries": 0
  }
  ```

* **Live GDPR Article 17 Tenant Cryptographic Shredding & Data Offboarding**:
  Performs atomic zeroization of the tenant data encryption key (TDEK) in memory and purges records across PostgreSQL, DragonflyDB, and Qdrant.
  ```bash
  export TARGET_TENANT="acme-corp"

  curl -fsS -X POST http://localhost:8080/v1/governance/shred \
    -H "Content-Type: application/json" \
    -H "x-api-key: ${SUNBORN_ADMIN_KEY}" \
    -d "{\"tenant_id\": \"${TARGET_TENANT}\"}" | jq
  ```
  *Expected Output:*
  ```json
  {
    "status": "shredded",
    "tenant_id": "acme-corp",
    "shred_latency_nanos": 4200,
    "verified_multi_store_erasure": true,
    "residual_records": {
      "db": 0,
      "exact_cache": 0,
      "execution_store": 0,
      "vector_store": 0
    }
  }
  ```
