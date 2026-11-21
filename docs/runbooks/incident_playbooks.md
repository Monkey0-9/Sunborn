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
   curl -fsS http://localhost:8080/ready | jq '.components.cache.status'
   # Expected: "connected" (cache status is "unavailable" during an outage)
   ```

---

## Playbook 4: Vector-Store (Qdrant) Outage & Exact-Path Fallback

### 4.1 Detection & Symptoms
- Alert: `SunbornSemanticTierDegraded` (`rate(sunborn_semantic_eval_failures_total[5m]) > 0.1`).
- Gateway logs: `Semantic evaluation failed; degrading to exact-cache miss plus origin`, with the underlying error on the same event.
- Exact cache (`c0_exact`, `c1_exact`) operates with 100% fidelity. Only semantic retrieval (Tier 2) abstains, so requests keep succeeding while the hit rate and origin cost silently degrade.
- The same alert also fires when the embedding provider is unreachable, not only Qdrant — read the logged `error` field to tell them apart.

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
- Alert: `SunbornPostgresConnectionPoolExhausted` (`rate(sunborn_spend_ledger_errors_total[5m]) > 0.05`).
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

## Playbook 6: Invalidation Outbox DLQ Accumulation

### 6.1 Detection & Symptoms
- Alert: `SunbornInvalidationDLQCountHigh` (`sunborn_invalidation_dlq > 0`).
- The gauge counts outbox events in `FAILED` state — events that exhausted
  `max_retries` (default 5). Each retry is recorded in `outbox_events.last_error`.
- Risk: the invalidation never completed, so a cached entry or dependency edge
  is still live. Unlike a partial failure this one is durable: it survives
  restarts until replayed.

### 6.2 Diagnostic Steps
1. Inspect the failed events and their last error:
   ```sql
   SELECT id, tenant_id, entity_type, entity_id, retry_count, last_error
   FROM outbox_events
   WHERE status = 'FAILED'
   ORDER BY id
   LIMIT 20;
   ```
2. The immutability audit trail of every dead-lettered attempt:
   ```sql
   SELECT outbox_id, tenant_id, entity_id, error_message, retry_count, failed_at
   FROM outbox_dlq
   ORDER BY failed_at DESC
   LIMIT 20;
   ```
   `outbox_dlq` is append-only history: a row stays after a successful replay,
   which is why the alert uses the `FAILED` count and not `count(*)` on this table.
3. Classify the error in `last_error`: a downstream store outage needs
   Playbooks 3/4; `Unsupported outbox event` means the worker rejected the
   event type and a retry will fail the same way.

### 6.3 Recovery
1. Fix the underlying store if the error is an outage.
2. Requeue the failed events. The endpoint is bounded to 100 events per call and
   is admin-quorum protected; the background worker picks them up on its next
   cycle:
   ```bash
   curl -fsS -X POST http://localhost:8080/v1/lifecycle/outbox/replay \
     -H "x-api-key: $SUNBORN_ADMIN_KEY" \
     -H "x-quorum-admin-key: $SUNBORN_QUORUM_ADMIN_KEY" | jq
   # {"status":"drained","replayed":N,"remaining_failed":0,"batch_size":100}
   ```
3. Repeat while `status` is `partial`; `remaining_failed` reports what is left.
4. Confirm `sunborn_invalidation_dlq` returns to 0 and the queued events reach
   `PROCESSED`:
   ```sql
   SELECT status, count(*) FROM outbox_events GROUP BY status;
   ```

---

## Playbook 7: Partial Invalidation (Stale Keys Left Behind)

### 7.1 Detection & Symptoms
- Alert: `SunbornInvalidationPartialFailure` (`increase(sunborn_invalidation_partial_failures_total[5m]) > 0`).
- Unlike Playbook 6, the invalidation ran but at least one store refused the
  delete; the outbox event may still be marked processed.
- Gateway logs: `Invalidation completed with un-evicted keys; stale entries may be served`, with `failed` and `purged` counts.
- Risk: a cache entry or dependency edge survives an invalidation, so a later request can be served content that should already be gone.

### 7.2 Diagnostic Steps
Invalidation is synchronous: the caller receives the per-store outcome, so the
first place to look is the response body of the failing invalidation, not a
table.

1. Re-run the invalidation and read `failed_keys`; each entry is prefixed with
the store that rejected the delete (`exact_cache:`, `vector_store:`,
`dependency_graph:`, `graph_memory:`, `doc_hashes:`).
   ```bash
   curl -fsS -X POST http://localhost:8080/v1/invalidation \
     -H "Content-Type: application/json" \
     -H "x-api-key: $SUNBORN_ADMIN_KEY" \
     -d '{"tenant_id":"tenant-enterprise-pilot-01","source_id":"<uuid>","version_tag":"v2"}' \
     | jq '{status, purged_count, failed_keys}'
   ```
   `status` is `purged`, `partially_purged` or `failed`; anything other than
   `purged` means at least one store did not confirm the delete.
2. Match the prefix to the failing dependency:
   - `exact_cache:` — cache store unreachable; check the circuit-breaker alert
     and `curl -fsS http://localhost:8080/ready`.
   - `vector_store:` — Qdrant unavailable; see Playbook 4.
   - `dependency_graph:` / `graph_memory:` — graph edge cleanup failed; the
     cached entry is gone but lineage may now be stale.
   - `doc_hashes:` — document-hash eviction failed; the next retrieval can
     re-materialise the document.

### 7.3 Recovery
1. Restore the failing store (Playbooks 3 and 4).
2. Re-issue the same invalidation once the store is healthy. The operation is
   idempotent: re-deleting an absent key succeeds.
3. Confirm `status` is `purged` and `failed_keys` is empty, and that
   `sunborn_invalidation_partial_failures_total` stops increasing.

### 7.4 Relationship to the outbox
A partial failure is reported to the caller and counted, but it does not by
itself park an event in the DLQ: the outbox event is acknowledged once the purge
call returns. If you need guaranteed retry of a store that was down, treat the
non-`purged` status as a failure at your orchestration layer, or rely on the
DLQ path in Playbook 6 (events that fail inside the worker).
