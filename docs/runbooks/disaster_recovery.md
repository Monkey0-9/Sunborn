# Sunborn Operational Runbook: Disaster Recovery & Accelerator Rebuild

> **Invariant Compliance**: *Rule 1 (Non-Accelerator Authority)*: DragonflyDB and Qdrant are operational accelerators, not correctness authorities. PostgreSQL is the authoritative system of record. All accelerator state is 100% reconstructible.
> **Security Baseline**: All administrative mutations and diagnostic recovery endpoints require an authenticated administrative API key (`SUNBORN_ADMIN_KEY`).

---

## 1. Overview & Recovery Objectives

Sunborn's recovery architecture is based on PostgreSQL Write-Ahead Log (WAL) durability and watermark fencing:

* **Target Recovery Point Objective (RPO)**: **0 seconds** (authoritative PostgreSQL transactions committed before client acknowledgment; all accelerator mutations backed by write-behind group commits).
* **Target Recovery Time Objective (RTO)**: **< 15 minutes** for 1,000,000 cached records under parallel rebuilding.

> [!NOTE]
> Stated RPO and RTO figures represent engineering design targets. Operational compliance must be validated through scheduled, timestamped disaster recovery drills in staging and production environments.

---

## 2. Emergency Recovery Trigger Scenarios

1. Total loss, corruption, or split-brain of DragonflyDB cluster nodes.
2. Unrecoverable vector index drift, segment corruption, or OOM crashes in Qdrant collections.
3. Unscheduled catastrophic failover or migration across Kubernetes clusters.

---

## 3. Authentication & Key Retrieval Prerequisite

Before executing recovery operations, retrieve the cluster administrative key:

```bash
export SUNBORN_ADMIN_KEY=$(kubectl get secret -n sunborn-system sunborn-gateway-secrets \
  -o jsonpath='{.data.SUNBORN_ADMIN_KEY}' | base64 -d)

if [ -z "${SUNBORN_ADMIN_KEY}" ]; then
  echo "FATAL: SUNBORN_ADMIN_KEY not found in sunborn-gateway-secrets." >&2
  exit 1
fi
```

---

## 4. Step-by-Step Restoration Procedure

### Step 1: Isolate Failing Accelerators (Trip Circuit Breaker)
If the cache tier is returning corrupted responses, failing healthchecks, or suffering node loss, trip the circuit breaker to force graceful origin fallback with zero HTTP 500s:

```bash
curl -fsS -X POST http://localhost:8080/v1/chaos/trip \
  -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq
```
*Expected Output:*
```json
{
  "circuit_status": "Open",
  "status": "tripped"
}
```

### Step 2: Validate Authoritative PostgreSQL Health
Ensure the PostgreSQL primary cluster is accessible and verify durable cache table counts:

```bash
psql -h postgresql.sunborn-system.svc.cluster.local -U sunborn -d sunborn_db \
  -c "SELECT count(*) AS active_cached_records FROM cache_entries WHERE expires_at > NOW() OR expires_at IS NULL;"
```

### Step 3: Deploy Clean Accelerator Instances
Re-deploy or recreate clean DragonflyDB and Qdrant instances:

```bash
kubectl rollout restart statefulset/dragonfly -n sunborn-system
kubectl rollout restart statefulset/qdrant -n sunborn-system

# Wait for pods to achieve Ready state
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=dragonfly -n sunborn-system --timeout=120s
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=qdrant -n sunborn-system --timeout=120s
```

### Step 4: Execute Sunborn Rebuild Engine
Run the rebuild utility to scan PostgreSQL and repopulate exact and semantic caches:

```bash
# Option A: Using dedicated CLI rebuild tool:
sunborn-rebuild --batch-size 1000

# Option B: Via authenticated Gateway administrative rebuild API:
curl -fsS -X POST http://localhost:8080/v1/rebuild \
  -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq
```

*Expected Output:*
```json
{
  "status": "completed",
  "scanned_db_records": 10542,
  "rebuilt_exact_cache": 10542,
  "rebuilt_vectors": 10542
}
```

### Step 5: Reset Circuit Breaker & Verify Cache Hits
Reset the circuit breaker to resume normal caching operations:

```bash
# 1. Reset circuit breaker
curl -fsS -X POST http://localhost:8080/v1/chaos/reset \
  -H "x-api-key: ${SUNBORN_ADMIN_KEY}" | jq

# 2. Verify an authenticated request produces a cache hit
curl -fsS -X POST http://localhost:8080/v1/requests \
  -H "Content-Type: application/json" \
  -H "x-api-key: ${SUNBORN_ADMIN_KEY}" \
  -d '{
    "tenant_id": "system-admin",
    "application_id": "admin-ops",
    "model": "gpt-4o",
    "prompt": "sample prompt"
  }' | jq '{cached: .cached, reason: .decision.reason_code}'
```
*Expected Output:*
```json
{
  "cached": true,
  "reason": "EXACT_CANONICAL_MATCH"
}
```

---

## 5. Disaster Recovery Drill Protocol

Teams must execute quarterly simulated disaster recovery drills:
1. **Timestamp Initiation**: Record `drill_start_utc=$(date -u +"%Y-%m-%dT%H:%M:%SZ")`.
2. **Inject Synthetic Loss**: Terminate DragonflyDB statefulset pods.
3. **Validate Fallback**: Run live traffic probes to confirm 100% origin execution with zero client errors.
4. **Rebuild Accelerators**: Execute `sunborn-rebuild` and verify records restored.
5. **Measure Elapsed RTO**: Record `drill_end_utc` and calculate total recovery duration.
6. **Sign-off**: Store drill logs and timestamped attestation in institutional compliance archive.
