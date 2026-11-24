import os
import subprocess
import sys
import json

ISSUES = [
    {
        "title": "[P1][Security/Networking] Pre-Auth Rate Limiting Collapses Behind Ingress Proxies Without Trusted Proxy Header Extraction",
        "labels": ["security", "bug", "P1"],
        "body": """### Summary
In `crates/sunborn-gateway/src/middleware/rate_limit.rs` (lines 264-275), `pre_auth_rate_limit_middleware` extracts the caller IP exclusively via:
```rust
let Some(peer_ip) = req
    .extensions()
    .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    .map(|info| info.0.ip())
```
In standard Kubernetes multi-tier deployments behind ingress controllers (e.g., NGINX Ingress, AWS ALB Ingress Controller, Envoy Gateway, Cloudflare Tunnel), `ConnectInfo` resolves to the ingress controller pod's internal cluster IP (`10.244.x.x`), rather than the actual client IP.

### Blast Radius & Impact
1. **Cluster-Wide Denial of Service**: If a single unauthenticated client or scanner attacks any gateway endpoint and trips the pre-auth rate limit (default 20 requests per second / 100 burst), the gateway throttles the ingress pod IP itself. This denies access to ALL incoming legitimate traffic across the entire cluster.
2. **Fail-Open Bypass**: If the Axum service is hosted behind reverse proxies where socket connect info is not propagated, `ConnectInfo` is `None`, causing the limiter to fail completely open, leaving the gateway vulnerable to unmitigated authentication credential stuffing and brute-force attacks.

### Proposed Architectural Remediation
1. Introduce a configurable trusted proxy CIDR whitelist via `SUNBORN_TRUSTED_PROXIES` (e.g. `10.0.0.0/8,172.16.0.0/12,192.168.0.0/16`).
2. When the direct socket peer matches a trusted proxy CIDR, inspect standard client IP headers (`X-Forwarded-For`, `CF-Connecting-IP`, `X-Real-IP`), validating IP formatting and extracting the leftmost non-trusted hop.
3. If no trusted proxy is configured, log an explicit startup warning and only rely on direct socket peer addresses.
"""
    },
    {
        "title": "[P1][Performance/Concurrency] Algorithmic O(N^2) Byte Recalculation in Singleflight Stream History Lock",
        "labels": ["bug", "P1"],
        "body": """### Summary
In `crates/sunborn-gateway/src/origin/singleflight.rs` (lines 95-107), `SingleflightSender::send` bounds the in-memory catch-up stream history buffer against memory exhaustion:
```rust
let mut hist = self.history.write().await;
let current_bytes: usize = hist.iter().map(|b| b.len()).sum();
if current_bytes.saturating_add(bytes.len()) <= MAX_SINGLEFLIGHT_HISTORY_BYTES
    && hist.len() < MAX_SINGLEFLIGHT_HISTORY_CHUNKS
{
    hist.push(bytes.clone());
}
```

### Blast Radius & Impact
1. **$O(N^2)$ Computational Complexity**: For every new token chunk emitted by an origin LLM stream, `send` iterates through all existing chunks in `hist` to compute `.sum()`. For a 2,000-chunk generation (e.g., long code blocks or analytical reasoning), this loop runs $2,000 \\times 1,000 = 2,000,000$ iterations per stream.
2. **Tokio Runtime Worker Thread Contention**: Because this calculation is performed while holding an exclusive async `write()` lock on `self.history`, concurrent followers attempting to read history chunks are blocked. Under high concurrency with 100+ active streaming requests, this induces CPU spikes, memory cache churn, and token delivery jitter.

### Proposed Architectural Remediation
1. Replace `Arc<RwLock<Vec<Bytes>>>` with a dedicated struct `StreamHistoryLog`:
   ```rust
   struct StreamHistoryLog {
       chunks: Vec<Bytes>,
       total_bytes: usize,
   }
   ```
2. Update `total_bytes` in $O(1)$ time on each chunk admission (`self.total_bytes += bytes.len()`), completely removing the linear scan inside the critical section.
"""
    },
    {
        "title": "[P1][Distributed Systems] Gateway Pods Lack Cluster Invalidation Subscriber for Multi-Pod L1 Memory Cache Synchronization",
        "labels": ["bug", "P1"],
        "body": """### Summary
In multi-node deployments (`deploy/k8s/deployment.yaml` specifies 3 replicas of `sunborn-gateway`), each gateway pod instantiates its own in-process L1 cache (`MemoryExactCache`) wrapped in `TieredExactCache`.
When an invalidation occurs via `/v1/graph/invalidate` or `/v1/cache/invalidate`, `crates/sunborn-gateway/src/handlers/graph.rs` publishes an invalidation event to Redis on `CLUSTER_INVALIDATION_CHANNEL`:
```rust
transport.publish(CLUSTER_INVALIDATION_CHANNEL, &msg).await;
```
However, `sunborn-gateway` never instantiates or spawns a background subscriber task to listen on `CLUSTER_INVALIDATION_CHANNEL`.

### Blast Radius & Impact
1. **Multi-Node State Inconsistency**: Pod A processes a graph mutation or cache invalidation, invalidates its local L1 memory, updates L2 (Redis), and publishes the broadcast event.
2. Pod B and Pod C never consume the event. If Pod B or Pod C already has the mutated entry in its local L1 memory, it continues serving stale, invalidated data directly from memory without contacting L2, violating cache consistency and temporal safety until L1 TTL expiration.

### Proposed Architectural Remediation
1. In `crates/sunborn-gateway/src/state.rs`, upon initializing `AppState`, spawn an asynchronous background subscriber task:
   ```rust
   let subscriber = transport.subscribe(CLUSTER_INVALIDATION_CHANNEL).await?;
   tokio::spawn(async move {
       while let Ok(msg) = subscriber.recv().await {
           tiered_cache.l1().delete(&msg.key).await;
           tiered_cache.advance_cluster_epoch();
       }
   });
   ```
2. Add an end-to-end integration test verifying that an invalidation issued to Pod A triggers immediate eviction from Pod B's in-process L1 cache.
"""
    },
    {
        "title": "[P1][Scalability/Database] Disaster Recovery & Rebuild Engine Uses O(N^2) OFFSET-Based Pagination for Large-Scale Database Scans",
        "labels": ["bug", "P1"],
        "body": """### Summary
In `apps/sunborn-rebuild/src/main.rs` (lines 148-155) and `crates/sunborn-db/src/postgres.rs` (lines 1982-1988), the disaster recovery rebuild engine paginates through durable cache entries using SQL `OFFSET`:
```sql
SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json
FROM durable_cache_entries ORDER BY created_at DESC LIMIT $1 OFFSET $2
```

### Blast Radius & Impact
1. **$O(N^2)$ PostgreSQL Disk I/O & Query Latency**: In PostgreSQL, `OFFSET M` requires the query engine to traverse and discard $M$ rows before returning $N$ rows. At 10 million rows, fetching page 10,000 (`OFFSET 5,000,000 LIMIT 500`) scans 5,000,500 index tuples on every single batch.
2. **Disaster Recovery Failure Under High Scale**: A cluster rebuild on a production database with millions of records will experience exponential slowdowns, connection pool timeouts (`acquire_timeout(2s)`), and heavy database CPU saturation, making recovery unviable within RTO SLAs.

### Proposed Architectural Remediation
1. Migrate from `OFFSET` pagination to cursor-based / keyset pagination:
   ```sql
   SELECT ... FROM durable_cache_entries
   WHERE (created_at, key) < ($cursor_created_at, $cursor_key)
   ORDER BY created_at DESC, key DESC
   LIMIT $1
   ```
2. Ensure a composite index `CREATE INDEX idx_durable_cache_rebuild_cursor ON durable_cache_entries (created_at DESC, key DESC)` exists in database migrations to provide $O(1)$ index seek performance across arbitrarily large tables.
"""
    },
    {
        "title": "[P2][Async Concurrency] CPU-Intensive Multi-Hop Graph Invalidation & Blast Radius Blocks Tokio Worker Thread",
        "labels": ["bug"],
        "body": """### Summary
In `crates/sunborn-gateway/src/handlers/graph.rs` (lines 225 & 250), `calculate_graph_blast_radius` and `invalidate_graph_causal` execute:
```rust
let report = compute_affected(&state.graphify_graph, &roots, Some(cfg));
```
This is executed directly inside the asynchronous Axum handler on the Tokio runtime worker thread.

### Blast Radius & Impact
1. **Tokio Executor Starvation**: `compute_affected` performs multi-hop breadth-first/depth-first graph traversal, relationship filtering, cycle detection, and community grouping over `GraphifyKnowledgeGraph`. In a large enterprise knowledge graph with 50,000+ nodes and dense relational edges, this traversal consumes tens of milliseconds of pure CPU compute.
2. **Head-of-Line Blocking**: Running synchronous CPU loops on the Tokio worker thread starves other tasks scheduled on the same thread, including streaming token delivery, HTTP keep-alive pings, and socket I/O.

### Proposed Architectural Remediation
1. Wrap graph traversal operations in `tokio::task::spawn_blocking`:
   ```rust
   let graph = state.graphify_graph.clone();
   let roots_owned: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
   let report = tokio::task::spawn_blocking(move || {
       let roots_ref: Vec<&str> = roots_owned.iter().map(|s| s.as_str()).collect();
       compute_affected(&graph, &roots_ref, Some(cfg))
   }).await.map_err(|e| SunbornError::internal(format!("Graph traversal task panicked: {e}")))?;
   ```
"""
    },
    {
        "title": "[P2][Container/Infra] Frontend Dockerfile Copies Full node_modules & Omits Next.js Standalone Build Optimization",
        "labels": ["release", "enhancement"],
        "body": """### Summary
In `frontend/next.config.mjs`, `output: 'standalone'` is not specified. In `deploy/Dockerfile.frontend` (lines 33-36):
```dockerfile
COPY --from=builder /app/.next ./.next
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./package.json
...
CMD ["npm", "run", "start"]
```

### Blast Radius & Impact
1. **Container Image Bloat**: Copying the full `node_modules` directory into the production runner image bundles development and test dependencies (`playwright`, `axe-core`, `@types/*`, `typescript`, `eslint`), swelling the production Docker image by ~400MB. This increases image pull times and Kubernetes horizontal pod autoscaling cold-start latency.
2. **Improper Signal Handling (PID 1)**: Executing `CMD ["npm", "run", "start"]` sets `npm` as the entrypoint process, which fails to forward POSIX `SIGTERM` signals to the underlying Node.js process during Kubernetes rolling updates, leading to aborted client connections instead of graceful drains.

### Proposed Architectural Remediation
1. Add `output: 'standalone'` to `frontend/next.config.mjs`.
2. Update `deploy/Dockerfile.frontend` to copy only `.next/standalone`, `.next/static`, and `public`:
   ```dockerfile
   COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
   COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
   COPY --from=builder /app/public ./public
   CMD ["node", "server.js"]
   ```
"""
    },
    {
        "title": "[P2][Observability] Distributed Tracing Missing W3C Trace Context (traceparent) Propagation to Upstream Origin Providers",
        "labels": ["enhancement"],
        "body": """### Summary
In `crates/sunborn-gateway/src/middleware/trace.rs`, the gateway validates and injects proprietary `x-request-id` headers into request extensions and response headers.
However, it does not parse, extract, or propagate standard W3C Trace Context headers (`traceparent`, `tracestate`, W3C Recommendation / RFC 3704). Furthermore, outgoing client requests dispatched to origin LLMs in `crates/sunborn-gateway/src/origin/provider.rs` do not propagate trace contexts.

### Blast Radius & Impact
1. **Severed Observability Tracing**: In enterprise distributed environments monitored by OpenTelemetry, Datadog, or Dynatrace, trace contexts originating from client web/mobile applications are broken at the Sunborn gateway boundary.
2. Upstream LLM provider calls and backend database queries appear as disconnected, orphaned spans rather than unified distributed traces.

### Proposed Architectural Remediation
1. In `trace.rs`, inspect incoming `traceparent` headers. If present and valid, parse the 4-part W3C header (`version-trace_id-parent_id-trace_flags`) and link it to the Axum tracing span.
2. Inject outgoing `traceparent` headers into upstream HTTP client requests within `crates/sunborn-gateway/src/origin/provider.rs`.
"""
    },
    {
        "title": "[P2][SDK Parity] TypeScript SDK Lacks Streaming SSE Generator & Covers Only 3 of 77 OpenAPI Operations",
        "labels": ["enhancement"],
        "body": """### Summary
In `sdk/typescript/src/client.ts`, the TypeScript client currently implements only three methods:
- `chatCompletion(request: ChatCompletionRequest): Promise<ChatCompletionResponse>`
- `getHealth(): Promise<GatewayHealth>`
- `getReady(): Promise<GatewayReadiness>`

### Blast Radius & Impact
1. **No Streaming Support**: The SDK cannot consume Server-Sent Events (SSE) streaming chunks. Web frontend applications using TypeScript are forced to write raw `fetch` streaming parsers rather than using a first-class SDK method `chatCompletionStream()`.
2. **Contract Disparity**: The authoritative Sunborn OpenAPI specification documents 77 operations (including tenant management, graph causal invalidation, blast radius simulation, spend ceilings, audit logs, and cryptographic verification). The TypeScript SDK implements less than 5% of the API surface.

### Proposed Architectural Remediation
1. Implement `chatCompletionStream` returning `AsyncIterable<ChatCompletionChunk>` with SSE parsing and backoff retry logic.
2. Generate comprehensive, strongly-typed client modules covering all 77 OpenAPI operations across tenant, cache, graph, and governance domains.
"""
    },
    {
        "title": "[P2][Database Resilience] PostgreSQL Connection Pool Lacks Configurable Pool Bounds, Max Lifetime, and Idle Timeouts",
        "labels": ["bug"],
        "body": """### Summary
In `crates/sunborn-db/src/postgres.rs` (lines 277-282), `PgPoolOptions` is configured with hardcoded values:
```rust
PgPoolOptions::new()
    .max_connections(20)
    .acquire_timeout(Duration::from_secs(2))
    .connect(&self.connection_url)
    .await
```
It omits `min_connections`, `max_lifetime`, and `idle_timeout`.

### Blast Radius & Impact
1. **Connection Starvation or Exhaustion Under HPA**: When Kubernetes scales the gateway from 3 pods to 30 pods, 30 pods $\\times$ 20 connections = 600 backend connections. Without PgBouncer or connection bounds tuning, this can exhaust PostgreSQL `max_connections`. Conversely, for high-throughput pods, 20 connections can become a bottleneck.
2. **Zombie Socket Resets (ECONNRESET)**: Without `max_lifetime` (e.g. 30 minutes) and `idle_timeout` (e.g. 10 minutes), persistent TCP connections held open through cloud NAT gateways or AWS Aurora failover transitions are silently dropped by intermediate firewalls, leading to transient query failures on stale pooled sockets.

### Proposed Architectural Remediation
1. Expose pool options via environment variables:
   - `DATABASE_MAX_CONNECTIONS` (default: 20)
   - `DATABASE_MIN_CONNECTIONS` (default: 5)
   - `DATABASE_IDLE_TIMEOUT_SECS` (default: 600)
   - `DATABASE_MAX_LIFETIME_SECS` (default: 1800)
2. Pass these settings into `PgPoolOptions` during database initialization.
"""
    },
    {
        "title": "[P2][Infrastructure/State] Kubernetes Gateway Deployment Mounts Ephemeral emptyDir Volume for Write-Ahead Log",
        "labels": ["bug"],
        "body": """### Summary
In `deploy/k8s/deployment.yaml` (lines 93-100), the WAL volume is declared as:
```yaml
volumeMounts:
  - name: sunborn-wal
    mountPath: /var/lib/sunborn
volumes:
  - name: sunborn-wal
    emptyDir: {}
```

### Blast Radius & Impact
1. **Loss of Crash-Recovery Durability**: `emptyDir` volumes are ephemeral; they survive container restarts within a pod, but are completely wiped if the pod is deleted, evicted, or rescheduled to another Kubernetes node.
2. When a node undergoes maintenance or fails, any uncommitted write-behind records in the WAL are permanently lost, breaking the zero-data-loss durability contract of the write-behind storage subsystem.

### Proposed Architectural Remediation
1. Update `deploy/k8s/` and Helm templates to support persistent volume claims:
   - Provide a `StatefulSet` variant using `volumeClaimTemplates` with storage classes (e.g., AWS gp3, GCE pd-ssd).
   - Add a configuration toggle in `values.yaml` for Helm deployments (`persistence.enabled: true`, `persistence.size: 10Gi`).
"""
    }
]

def main():
    token = os.environ.get("GH_TOKEN")
    if not token:
        print("ERROR: GH_TOKEN not found in environment", file=sys.stderr)
        sys.exit(1)

    created_issues = []
    for issue in ISSUES:
        title = issue["title"]
        labels = ",".join(issue["labels"])
        body = issue["body"].strip()

        print(f"Creating issue: {title} ...")
        cmd = [
            "gh", "issue", "create",
            "--repo", "Monkey0-9/Sunborn",
            "--title", title,
            "--body", body,
            "--label", labels
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            print(f"FAILED: {res.stderr}", file=sys.stderr)
            sys.exit(res.returncode)
        
        issue_url = res.stdout.strip()
        print(f"SUCCESS: {issue_url}")
        created_issues.append({"title": title, "url": issue_url})

    with open("docs/audit/NEW_GITHUB_ISSUES.json", "w", encoding="utf-8") as f:
        json.dump(created_issues, f, indent=2)

    print(f"\nAll {len(created_issues)} issues created successfully!")

if __name__ == "__main__":
    main()
