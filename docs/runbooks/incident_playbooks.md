# Mission-Critical Incident Response Playbooks

**Target Audience:** Site Reliability Engineers (SRE), Platform Operations, Incident Commanders  
**Classification:** Operational Procedure  
**Authority:** [`docs/STATUS.md`](../STATUS.md) | Part of the [Launch Architecture Package](../launch/README.md)

---

## Overview & Incident Command Principles

Sunborn enforces zero-trust fail-closed security invariants across all runtime paths. During high-severity production incidents, site reliability engineers must adhere to four non-negotiable operational principles:

1. **Safety Over Availability:** If cryptographic integrity, spend ceilings, or causal epoch invalidations cannot be guaranteed, the gateway must fail closed (`429` or `503`) rather than serving unverified or unmetered inference.
2. **Deterministic Remediation:** Every action executed by an operator must use authenticated, auditable administrative endpoints requiring `SUNBORN_ADMIN_KEY` (and `x-quorum-admin-key` where m-of-n quorum is required).
3. **Evidence-Driven Post-Mortem:** Every incident declaration must capture metric snapshots, log extracts, and PostgreSQL state before and after intervention.
4. **Zero Silent Degradation:** Client responses during degraded states must strictly adhere to RFC 7807 problem details with explicit `Retry-After` headers.

---

## Playbook 1: Free-Tier Quota & Global Pool Exhaustion

### 1.1 Detection & Symptoms
- Alert: `SunbornTenantQuotaExhaustionCritical` or `SunbornFreePoolCeilingReached`.
- Prometheus metrics:
  - `rate(sunborn_quota_rejections_total[5m]) > 0`
  - `sunborn_rate_limited_total` rising rapidly.
- Gateway HTTP response: `429 Too Many Requests` with:
  ```json
  {
    "type": "https://sunborn.ai/errors/ratelimitexceeded",
    "title": "RateLimitExceeded",
    "status": 429,
    "detail": "free_pool quota exceeded for tenant 'tenant-alpha': 50000 of 50000 requests used in the current day; retry after 1420s"
  }
  ```

### 1.2 Triaging & Assessment
1. Determine whether the exhaustion is tenant-specific or global free pool:
   ```bash
   # Check current usage in PostgreSQL
   psql "$SUNBORN_DATABASE_URL" -c "
     SELECT tenant_id, window_kind, window_start, requests 
     FROM tenant_quota_usage 
     WHERE window_start >= NOW() - INTERVAL '1 day' 
     ORDER BY requests DESC LIMIT 10;
   "
   ```
2. Verify aggregate free-pool counter status:
   ```bash
   psql "$SUNBORN_DATABASE_URL" -c "
     SELECT window_kind, window_start, requests 
     FROM free_pool_usage 
     WHERE window_start >= NOW() - INTERVAL '1 day';
   "
   ```

### 1.3 Remediation Procedures
* **Option A: Emergency Global Pool Expansion (Authorized Release Authority)**:
  If cluster capacity allows and the incident is due to legitimate traffic growth:
  ```bash
  curl -fsS -X POST http://localhost:8080/v1/governance/free-pool \
    -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "daily_requests": 100000,
      "monthly_requests": 2500000
    }' | jq
  ```
* **Option B: Isolate Abusive Tenant**:
  If a single tenant is exhausting the shared pool via burst scripting:
  ```bash
  # Step 1: Clamp abusive tenant quota to 0 immediately
  curl -fsS -X POST http://localhost:8080/v1/governance/tenant-quota \
    -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "tenant_id": "malicious-tenant-id",
      "tier": "restricted",
      "daily_requests": 0,
      "monthly_requests": 0
    }' | jq

  # Step 2: Revoke active keys for the abusive tenant
  curl -fsS -X POST http://localhost:8080/v1/auth/credentials/revoke \
    -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" \
    -H "x-quorum-admin-key: $SUNBORN_QUORUM_ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d '{"tenant_id": "malicious-tenant-id"}' | jq
  ```

---

## Playbook 2: Upstream LLM Provider Outage & Severe Throttling

### 2.1 Detection & Symptoms
- Alert: `SunbornUpstreamOriginLatencySpike` or `SunbornUpstream5xxRateHigh`.
- Gateway logs: `Origin request failed; upstream provider returned 503 Service Unavailable`.
- Fallback rate metrics: `sunborn_cache_misses_total` requests timing out or returning 502/504 to callers.

### 2.2 Triaging & Assessment
1. Inspect upstream provider status (OpenAI, Anthropic, AWS Bedrock):
   ```bash
   kubectl logs -l app.kubernetes.io/name=sunborn-gateway -n sunborn-system --tail=200 | grep -i "upstream"
   ```
2. Check local exact cache hit ratio. In steady-state, exact hits (`c0_exact`, `c1_exact`) continue to serve without upstream dependencies.

### 2.3 Remediation Procedures
* **Step 1: Shift Traffic to Backup Model Provider**:
  If the primary model (e.g., `gpt-4o`) is degraded, update tenant allowlists or reconfigure origin routing:
  ```bash
  # Reconfigure gateway environment or route headers to alternate model provider
  kubectl set env deployment/sunborn-gateway -n sunborn-system \
    SUNBORN_PRIMARY_MODEL="claude-3-5-sonnet"
  ```
* **Step 2: Enable Aggressive Cache Preservation**:
  Increase exact cache TTL and disable proactive eviction to maximize serving from cache:
  ```bash
  kubectl set env deployment/sunborn-gateway -n sunborn-system \
    SUNBORN_CACHE_TTL_SECS="86400"
  ```
* **Step 3: Monitor Upstream Health & Roll Back**:
  Once upstream latency normalizes below 500 ms P95:
  ```bash
  kubectl rollout undo deployment/sunborn-gateway -n sunborn-system
  ```

---

## Playbook 3: Distributed Cache-Store Outage (DragonflyDB / Redis)

### 2.1 Detection & Symptoms
- Alert: `SunbornCacheStoreCircuitBreakerOpen`.
- Endpoint `GET /v1/chaos/status` returns:
  ```json
  { "circuit_status": "Open" }
  ```
- Gateway logs: `Dragonfly/Redis connection pool exhausted; circuit breaker tripped OPEN`.
- System behavior: Safe automatic fail-through. Incoming requests bypass the distributed cache layer to prevent tail-latency blowouts, falling back to local memory L1 and origin dispatch.

### 3.2 Diagnostic Steps
1. Verify Dragonfly/Redis cluster pod health:
   ```bash
   kubectl get pods -l app.kubernetes.io/name=dragonfly -n sunborn-system
   kubectl logs -l app.kubernetes.io/name=dragonfly -n sunborn-system --tail=50
   ```
2. Test network latency from gateway pod to cache service:
   ```bash
   kubectl exec -it deployment/sunborn-gateway -n sunborn-system -- nc -zv dragonfly.sunborn-system.svc.cluster.local 6379
   ```

### 3.3 Recovery & Reset Procedure
1. If cache cluster was restarted or recovered, verify basic ping responsiveness:
   ```bash
   redis-cli -h dragonfly.sunborn-system.svc.cluster.local -p 6379 PING
   ```
2. Reset gateway circuit breaker via administrative endpoint:
   ```bash
   curl -fsS -X POST http://localhost:8080/v1/chaos/reset \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" | jq
   ```
3. Verify readiness probe returns healthy status:
   ```bash
   curl -fsS http://localhost:8080/ready | jq '.dragonfly'
   # Expected: "healthy"
   ```

---

## Playbook 4: Vector-Store (Qdrant) Outage & Exact-Path Fallback

### 4.1 Detection & Symptoms
- Alert: `SunbornQdrantClusterUnreachable`.
- Gateway logs: `Semantic vector search failed; abstaining to Origin due to vector store unavailability`.
- Exact cache (`c0_exact`, `c1_exact`) operates with 100% fidelity. Only semantic retrieval (Tier 2) abstains.

### 4.2 Diagnostic & Isolation Steps
1. Query Qdrant cluster health endpoint:
   ```bash
   curl -fsS http://qdrant.sunborn-system.svc.cluster.local:6333/healthz
   ```
2. Check memory and disk pressure on Qdrant statefulset:
   ```bash
   kubectl top pods -l app.kubernetes.io/name=qdrant -n sunborn-system
   ```

### 4.3 Recovery Procedure
1. If Qdrant index is corrupted or deadlocked, initiate rolling restart:
   ```bash
   kubectl rollout restart statefulset/qdrant -n sunborn-system
   kubectl rollout status statefulset/qdrant -n sunborn-system --timeout=180s
   ```
2. Verify collection consistency:
   ```bash
   curl -fsS http://qdrant.sunborn-system.svc.cluster.local:6333/collections/sunborn_semantic_v1 | jq '.result.status'
   # Expected: "green"
   ```
3. Monitor semantic hit recovery via Prometheus:
   ```bash
   curl -fsS http://localhost:8080/metrics | grep 'sunborn_cache_hits_total{tier="semantic"}'
   ```

---

## Playbook 5: Distributed Spend Ledger Outage & Advisory Lock Contention

### 5.1 Detection & Symptoms
- Alert: `SunbornPostgresConnectionPoolExhausted` or `SunbornSpendLedgerAdvisoryLockTimeout`.
- Gateway logs: `Spend ledger unavailable; failing closed with 503`.
- Invariant enforcement: System strictly refuses to process new inference requests without pre-allocated spend reservation.

### 5.2 Diagnostic Steps
1. Identify blocked queries and advisory locks in PostgreSQL:
   ```sql
   SELECT pid, age(clock_timestamp(), query_start), usename, query, state 
   FROM pg_stat_activity 
   WHERE state != 'idle' AND query LIKE '%pg_advisory_xact_lock%'
   ORDER BY age(clock_timestamp(), query_start) DESC;
   ```
2. Check for lock contention on `hashtextextended`:
   ```sql
   SELECT locktype, classid, objid, mode, granted 
   FROM pg_locks 
   WHERE locktype = 'advisory';
   ```

### 5.3 Recovery Procedure
1. Terminate orphaned backend connections holding long-running advisory locks:
   ```sql
   SELECT pg_terminate_backend(pid) 
   FROM pg_stat_activity 
   WHERE state = 'idle in transaction' 
     AND query_start < NOW() - INTERVAL '30 seconds';
   ```
2. If primary node failed over, verify PgBouncer or connection pool router updated to current read-write primary.
3. Test reservation latency:
   ```bash
   curl -fsS http://localhost:8080/ready | jq '.postgres'
   # Expected: "healthy"
   ```

---

## Playbook 6: Invalidation Outbox Stream Backlog & DLQ Accumulation

### 6.1 Detection & Symptoms
- Alert: `SunbornInvalidationDLQCountHigh` (`sunborn_invalidation_dlq > 0`).
- Outbox pending records growing monotonically in table `invalidation_outbox`.
- Risk: Invalidation lag SLI exceeds 60-second beta target; stale cached entries may be served.

### 6.2 Diagnostic Steps
1. Inspect dead-letter queue depth and failure reasons:
   ```sql
   SELECT error_reason, count(*), min(created_at), max(created_at) 
   FROM invalidation_dlq 
   GROUP BY error_reason;
   ```
2. Inspect pending outbox stream backlog:
   ```sql
   SELECT count(*) FROM invalidation_outbox WHERE status = 'pending';
   ```

### 6.3 Recovery & DLQ Drain Procedure
1. Verify downstream subscriber pods are running and consuming Redis Stream / PostgreSQL notification channels:
   ```bash
   kubectl get pods -l app.kubernetes.io/name=sunborn-sync-worker -n sunborn-system
   ```
2. Trigger automated outbox drain and dead-letter queue re-injection:
   ```bash
   curl -fsS -X POST http://localhost:8080/v1/lifecycle/sweep \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" | jq
   ```
3. Confirm DLQ is emptied and pending outbox count drops to zero:
   ```sql
   SELECT count(*) FROM invalidation_outbox WHERE status = 'pending';
   -- Expected: 0
   ```
4. Verify end-to-end freshness by publishing a canary document invalidation and querying `sunborn_invalidation_lag_seconds`.
