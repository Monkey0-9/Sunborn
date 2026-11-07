/**
 * Sunborn Unified Typed API Client
 * Enforces server-side BFF authentication, structured error propagation,
 * and zero browser secret exposure.
 */

export const API_BASE_URL =
  typeof window !== "undefined"
    ? "/api/proxy"
    : process.env.SUNBORN_GATEWAY_URL ||
      process.env.API_URL ||
      "http://127.0.0.1:8080";

export interface ApiErrorResponse {
  type?: string;
  title?: string;
  status?: number;
  detail?: string;
  error?: any;
}

export class SunbornApiError extends Error {
  public status: number;
  public details?: any;

  constructor(message: string, status: number, details?: any) {
    super(message);
    this.name = "SunbornApiError";
    this.status = status;
    this.details = details;
  }
}

/**
 * Resolves an explicit client session key if provided.
 * Does NOT read from NEXT_PUBLIC variables or localStorage admin fallbacks,
 * delegating credential management to the secure server-side BFF.
 */
export function getActiveApiKey(explicitKey?: string): string | undefined {
  if (explicitKey && explicitKey.trim().length > 0) {
    return explicitKey.trim();
  }
  return undefined;
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  apiKey?: string
): Promise<T> {
  const cleanBase = API_BASE_URL.replace(/\/+$/, "");
  const cleanPath = path.replace(/^\/+/, "");
  const url = `${cleanBase}/${cleanPath}`;
  const headers = new Headers(options.headers || {});

  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }
  const resolvedKey = getActiveApiKey(apiKey);
  if (!headers.has("x-api-key") && resolvedKey) {
    headers.set("x-api-key", resolvedKey);
  }

  try {
    const res = await fetch(url, {
      ...options,
      headers,
    });

    if (!res.ok) {
      let errorDetail = `HTTP ${res.status}: ${res.statusText}`;
      let errorJson: any = null;
      try {
        errorJson = await res.json();
        if (errorJson.detail) {
          errorDetail = errorJson.detail;
        } else if (errorJson.message) {
          errorDetail = errorJson.message;
        }
      } catch {
        // Non-JSON error body
      }
      throw new SunbornApiError(errorDetail, res.status, errorJson);
    }

    return (await res.json()) as T;
  } catch (err: any) {
    if (err instanceof SunbornApiError) {
      throw err;
    }
    throw new SunbornApiError(
      `Gateway connection failed: ${err.message || "Network unreachable"}. Ensure Sunborn Gateway is running at ${API_BASE_URL}.`,
      0,
      err
    );
  }
}

export const sunbornApi = {
  // Chat completions
  chatCompletions: (body: any, apiKey?: string) =>
    request<any>(
      "v1/chat/completions",
      {
        method: "POST",
        body: JSON.stringify(body),
      },
      apiKey
    ),

  // Governance: Cryptographic shredding
  shredTenant: (tenantId: string) =>
    request<{
      tenant_id: string;
      status: string;
      shred_latency_nanos?: number;
      deleted_db_entries?: number;
      deleted_exact_cache?: number;
      deleted_vectors?: number;
      deleted_execution_tools?: number;
      deleted_learning_events?: number;
      verified_multi_store_erasure?: boolean;
    }>("v1/governance/shred", {
      method: "POST",
      body: JSON.stringify({ tenant_id: tenantId }),
    }),

  // Governance: Durable state rebuild from PostgreSQL
  triggerRebuild: (tenantId: string) =>
    request<{
      status: string;
      rehydrated_caches?: number;
      rehydrated_vectors?: number;
      elapsed_ms?: number;
    }>("v1/rebuild", {
      method: "POST",
      body: JSON.stringify({ tenant_id: tenantId }),
    }),

  // Invalidation: Transactional outbox invalidation cascade
  triggerInvalidation: (sourceId: string, tenantId = "tenant-enterprise-fintech", versionTag = "v2.5.0") =>
    request<{
      status: string;
      source_id: string;
      evicted_cache_count?: number;
    }>("v1/invalidation", {
      method: "POST",
      body: JSON.stringify({
        source_id: sourceId,
        tenant_id: tenantId,
        version_tag: versionTag,
      }),
    }),

  // Chaos & Circuit Breaker
  tripCircuit: () =>
    request<any>("v1/chaos/trip", { method: "POST" }),
  resetCircuit: () =>
    request<any>("v1/chaos/reset", { method: "POST" }),
  getChaosStatus: () =>
    request<any>("v1/chaos/status", { method: "GET" }),

  // Multi-Region Cluster Status
  getClusterStatus: () =>
    request<{
      node_id: string;
      region: string;
      cluster_state: string;
      vector_clock: Record<string, number>;
      anti_entropy_protocol: string;
      consensus_mode: string;
    }>("v1/cluster/status", { method: "GET" }),

  // Learning Controller
  getLearningPolicies: () =>
    request<{
      tenant_id: string;
      active_policy: any;
      canary_policy: any;
      tenant_feedback_events: number;
      total_feedback_events: number;
      audit_trail: any[];
    }>("v1/learning/policies", { method: "GET" }),

  submitLearningFeedback: (event: any) =>
    request<{ status: string; timestamp: string }>("v1/learning/feedback", {
      method: "POST",
      body: JSON.stringify(event),
    }),

  calibratePolicy: (version: number) =>
    request<{
      status: string;
      candidate_policy: any;
    }>("v1/learning/calibrate", {
      method: "POST",
      body: JSON.stringify({ version }),
    }),

  startCanaryRollout: (candidatePolicy: any, canaryPercentage: number) =>
    request<{ status: string; canary_percentage: number; timestamp: string }>(
      "v1/learning/canary",
      {
        method: "POST",
        body: JSON.stringify({
          candidate_policy: candidatePolicy,
          canary_percentage: canaryPercentage,
        }),
      }
    ),

  promoteLearningPolicy: () =>
    request<{
      status: string;
      active_policy: any;
      timestamp: string;
    }>("v1/learning/promote", {
      method: "POST",
    }),

  rollbackLearningPolicy: (reason: string) =>
    request<{ status: string; reason: string; timestamp: string }>(
      "v1/learning/rollback",
      {
        method: "POST",
        body: JSON.stringify({ reason }),
      }
    ),

  // Security
  evaluateSecurity: (prompt: string, model?: string) =>
    request<{
      threatDetected: boolean;
      ruleMatched: string;
      action: string;
      conformalScore: number;
      tenantId: string;
      timestamp: string;
    }>("v1/security/evaluate", {
      method: "POST",
      body: JSON.stringify({ prompt, model }),
    }),

  getSecurityAudit: () =>
    request<{
      tenant_id: string;
      events: Array<{
        id: string;
        event: string;
        origin: string;
        detail: string;
        status: string;
        time: string;
      }>;
    }>("v1/security/audit", { method: "GET" }),

  // Governance: Cryptographic Merkle DAG Attestation & Verification
  generateMerkleAttestation: (leafHashes: string[]) =>
    request<{
      status: string;
      root_hash: string;
      leaf_count: number;
      proofs: Array<{
        leaf_hash: string;
        root_hash: string;
        path: Array<{ sibling_hash: string; is_right: boolean }>;
      }>;
    }>("v1/governance/attest", {
      method: "POST",
      body: JSON.stringify({ leaf_hashes: leafHashes }),
    }),

  verifyMerkleProof: (payload: {
    proof?: {
      leaf_hash: string;
      root_hash: string;
      path: Array<{ sibling_hash: string; is_right: boolean }>;
    };
    merkle_root?: string;
    root_hash?: string;
    leaf_hash?: string;
    path?: Array<{ sibling_hash: string; is_right: boolean }>;
  }) =>
    request<{
      is_valid: boolean;
      valid: boolean;
      root_hash: string;
      leaf_hash: string;
    }>("v1/governance/verify-proof", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  // FinOps & Metrics
  getFinOpsReport: () =>
    request<any>("v1/finops/report", { method: "GET" }),

  // Predictive Routing & Speculative Prefetching
  predictSpeculative: (currentAction: string, recordTransitionTo?: string) =>
    request<{
      current_action: string;
      predictions: Array<{
        predicted_action: string;
        confidence_score: number;
        estimated_savings_ms: number;
      }>;
      speculative_cache_warmed: boolean;
    }>("v1/speculative/predict", {
      method: "POST",
      body: JSON.stringify({
        current_action: currentAction,
        record_transition_to: recordTransitionTo,
      }),
    }),

  // Autonomic Health & Incident History
  getAutonomicStatus: () =>
    request<{
      autonomic_health: string;
      remediation_mode: string;
      circuit_breaker_status: string;
      total_incidents_resolved: number;
      recent_incidents: Array<{
        incident_id: string;
        component: string;
        trigger_reason: string;
        recovery_action: string;
        resolved_at: string;
      }>;
    }>("v1/autonomic/status", { method: "GET" }),

  // Prefix KV-Cache Affinity & Cost Arbitrage Routing
  radixRoute: (prompt: string, warmPrefix?: string, warmNode?: string) =>
    request<{
      selected_node: string;
      is_kv_affinity_hit: boolean;
      matched_tokens: number;
      estimated_prefill_savings_pct: number;
    }>("v1/routing/radix/route", {
      method: "POST",
      body: JSON.stringify({
        prompt,
        warm_prefix: warmPrefix,
        warm_node: warmNode,
      }),
    }),

  radixInspect: () =>
    request<{
      status: string;
      backend_nodes: string[];
      min_affinity_tokens: number;
      max_load_factor: number;
      strategy: string;
    }>("v1/routing/radix/inspect", { method: "GET" }),

  // Auth & Credentials Management
  createApiCredential: (payload: {
    tenant_id: string;
    application_id: string;
    rate_limit_rps?: number;
    allowed_models?: string[];
    is_admin?: boolean;
    expires_in_secs?: number;
    raw_key?: string;
  }) =>
    request<{
      status: string;
      raw_key: string;
      key_hash: string;
      tenant_id: string;
      application_id: string;
      is_admin: boolean;
      expires_at?: string;
    }>("v1/auth/credentials", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  revokeApiCredential: (payload: {
    key_hash?: string;
    raw_key?: string;
    tenant_id?: string;
  }) =>
    request<{
      status: string;
      revoked_count?: number;
      durable_revoked_count?: number;
      tenant_id?: string;
      key_hash?: string;
    }>("v1/auth/credentials/revoke", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  rotateApiCredential: (payload: {
    old_raw_key?: string;
    old_key_hash?: string;
    expires_in_secs?: number;
  }) =>
    request<{
      status: string;
      new_raw_key: string;
      new_key_hash: string;
    }>("v1/auth/credentials/rotate", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  verifySession: () =>
    request<{
      authenticated: boolean;
      tenant_id: string;
      application_id: string;
      is_admin: boolean;
    }>("v1/auth/session", { method: "GET" }),

  // Governance: Tenant Provisioning
  provisionTenant: (tenantId: string) =>
    request<{
      status: string;
      tenant_id: string;
      kms_provider: string;
      durable_persistence: boolean;
    }>("v1/governance/provision", {
      method: "POST",
      body: JSON.stringify({ tenant_id: tenantId }),
    }),

  // Exact & Multi-Tier Cache CRUD
  getCacheEntry: (key: string) =>
    request<any>(`v1/caches/${encodeURIComponent(key)}`, { method: "GET" }),

  deleteCacheEntry: (key: string) =>
    request<{ key: string; deleted: boolean }>(
      `v1/caches/${encodeURIComponent(key)}`,
      { method: "DELETE" }
    ),

  // Causal Execution DAG
  planExecution: (payload: {
    tenant_id: string;
    application_id: string;
    plan: any;
  }) =>
    request<any>("v1/execution/plan", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  recordToolResult: (payload: {
    tenant_id: string;
    application_id: string;
    node_id: string;
    tool_name: string;
    result: any;
  }) =>
    request<any>("v1/execution/results", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  evictExecution: (tenantId: string) =>
    request<{ tenant_id: string; evicted_nodes: number }>(
      "v1/execution/evict",
      {
        method: "POST",
        body: JSON.stringify({ tenant_id: tenantId }),
      }
    ),

  // Lineage & Dependencies
  recordDependency: (sourceId: string, cacheKey: string) =>
    request<{ status: string; source_id: string; cache_key: string }>(
      "v1/dependencies",
      {
        method: "POST",
        body: JSON.stringify({ source_id: sourceId, cache_key: cacheKey }),
      }
    ),

  sweepLifecycle: () =>
    request<{ status: string; swept_entries: number }>(
      "v1/lifecycle/sweep",
      { method: "POST" }
    ),

  shadowEvaluate: (payload: {
    tenant_id: string;
    application_id: string;
    prompt: string;
    model: string;
    origin_content: string;
  }) =>
    request<{
      drift_detected: boolean;
      cosine_similarity: number;
      action: string;
      evicted: boolean;
    }>("v1/shadow/evaluate", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  getClusterSyncStatus: () =>
    request<any>("v1/cluster/sync/status", { method: "GET" }),

  getAutonomicIncidents: () =>
    request<any>("v1/autonomic/incidents", { method: "GET" }),

  getHealth: () =>
    request<{ status: string; service: string; version: string; timestamp: string }>(
      "health",
      { method: "GET" }
    ),

  getReadiness: () =>
    request<any>("ready", { method: "GET" }),

  getMetrics: () =>
    request<string>("metrics", { method: "GET" }),
};

