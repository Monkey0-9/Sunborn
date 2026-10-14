# Sunborn Operational Runbook: Disaster Recovery & Accelerator Rebuild

> **Invariant Compliance**: *Rule 1 (Non-Accelerator Authority)*: DragonflyDB and Qdrant are operational accelerators, not correctness authorities. PostgreSQL is the authoritative system of record. All accelerator state is 100% reconstructible.

---

## 1. Overview & Recovery Objectives
* **Recovery Point Objective (RPO)**: 0 seconds (authoritative PostgreSQL transactions committed before acknowledgment).
* **Recovery Time Objective (RTO)**: < 15 minutes for 1,000,000 cached records.

---

## 2. Emergency Recovery Trigger Scenarios
1. Total loss or corruption of Dragonfly cluster nodes.
2. Unrecoverable vector index drift or corruption in Qdrant collections.
3. Unscheduled catastrophic failover across Kubernetes clusters.

---

## 3. Step-by-Step Restoration Procedure

### Step 1: Isolate Failing Accelerators
If the cache tier is returning corrupted responses or failing healthchecks:
```bash
# Trip the gateway circuit breaker to force graceful origin fallback with zero HTTP 500s
curl -s -X POST http://localhost:8080/v1/chaos/trip | jq
```

### Step 2: Validate Authoritative PostgreSQL Health
Ensure the PostgreSQL primary cluster is accessible and verify durable cache table counts:
```bash
psql -h postgresql.sunborn-system.svc.cluster.local -U sunborn -d sunborn_db \
  -c "SELECT count(*) FROM cache_entries WHERE expires_at > NOW() OR expires_at IS NULL;"
```

### Step 3: Deploy Fresh Accelerator Instances
Re-deploy clean Dragonfly and Qdrant instances:
```bash
kubectl rollout restart statefulset/dragonfly -n sunborn-system
kubectl rollout restart statefulset/qdrant -n sunborn-system
```

### Step 4: Execute Sunborn Rebuild Engine
Run the rebuild utility to scan PostgreSQL and repopulate exact and semantic caches:
```bash
# Using CLI tool:
sunborn-rebuild --batch-size 1000

# Or via Gateway administrative rebuild API:
curl -s -X POST http://localhost:8080/v1/rebuild | jq
```

Expected Output:
```json
{
  "status": "completed",
  "scanned_db_records": 10542,
  "rebuilt_exact_cache": 10542,
  "rebuilt_vectors": 10542
}
```

### Step 5: Reset Circuit Breaker & Verify Cache Hits
```bash
# Reset circuit breaker
curl -s -X POST http://localhost:8080/v1/chaos/reset | jq

# Verify request produces cache hit with 0ms origin overhead
curl -s -X POST http://localhost:8080/v1/requests \
  -H "Content-Type: application/json" \
  -d '{"tenant_id":"tenant-alpha","application_id":"app-rag","model":"gpt-4o","prompt":"sample prompt"}' | jq .cached
# Expect: true
```
