/**
 * Sunborn Unified Typed API Client
 * Enforces server-side BFF authentication, structured error propagation,
 * and zero browser secret exposure.
 */

export { simulationEngine } from "./simulation-engine.ts";
import { simulationEngine } from "./simulation-engine.ts";
import { setIsSimulated } from "./cluster-store.ts";

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

async function withFallback<T>(
  action: () => Promise<T>,
  fallback: () => Promise<T>
): Promise<T> {
  try {
    const res = await action();
    setIsSimulated(false);
    return res;
  } catch (err: any) {
    if (err instanceof SunbornApiError) {
      // 1. Gateway offline / connection refused (status 0) or Next.js BFF connection error (status 502)
      const isConnectionOffline =
        err.status === 0 ||
        (err.status === 502 && err.details?.error === "GATEWAY_CONNECTION_FAILED");

      // 2. Anonymous public visitor accessing endpoint in demo mode without credentials
      const isAnonymousPublicDemo =
        err.status === 401 &&
        (!getActiveApiKey() || err.details?.error === "AUTHENTICATION_REQUIRED");

      if (isConnectionOffline || isAnonymousPublicDemo) {
        setIsSimulated(true);
        return await fallback();
      }
    }
    throw err;
  }
}

export const sunbornApi = {
  // Chat completions
  chatCompletions: (body: any, apiKey?: string, preset?: "exact" | "semantic" | "negation" | "dag") =>
    withFallback(
      () =>
        request<any>(
          "v1/chat/completions",
          {
            method: "POST",
            body: JSON.stringify(body),
          },
          apiKey
        ),
      () => simulationEngine.chatCompletions(body, preset)
    ),

  // Governance: Cryptographic shredding
  shredTenant: (tenantId: string) =>
    withFallback(
      () =>
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
      () => simulationEngine.shredTenant(tenantId)
    ),

  // Governance: Durable state rebuild from PostgreSQL
  triggerRebuild: (tenantId: string) =>
    withFallback(
      () =>
        request<{
          status: string;
          rehydrated_caches?: number;
          rehydrated_vectors?: number;
          elapsed_ms?: number;
        }>("v1/rebuild", {
          method: "POST",
          body: JSON.stringify({ tenant_id: tenantId }),
        }),
      () => simulationEngine.triggerRebuild(tenantId) as any
    ),

  // Invalidation: Transactional outbox invalidation cascade
  triggerInvalidation: (sourceId: string, tenantId = "tenant-enterprise-fintech", versionTag = "v2.5.0") =>
    withFallback(
      () =>
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
      () => simulationEngine.triggerInvalidation(sourceId, tenantId, versionTag) as any
    ),

  // Chaos & Circuit Breaker
  tripCircuit: () =>
    withFallback(
      () => request<any>("v1/chaos/trip", { method: "POST" }),
      () => simulationEngine.tripCircuit()
    ),
  resetCircuit: () =>
    withFallback(
      () => request<any>("v1/chaos/reset", { method: "POST" }),
      () => simulationEngine.resetCircuit()
    ),
  getChaosStatus: () =>
    withFallback(
      () => request<any>("v1/chaos/status", { method: "GET" }),
      () => simulationEngine.getChaosStatus()
    ),

  // Multi-Region Cluster Status
  getClusterStatus: () =>
    withFallback(
      () =>
        request<{
          node_id: string;
          region: string;
          cluster_state: string;
          vector_clock: Record<string, number>;
          anti_entropy_protocol: string;
          consensus_mode: string;
        }>("v1/cluster/status", { method: "GET" }),
      () => simulationEngine.getClusterStatus() as any
    ),

  getClusterSyncStatus: () =>
    withFallback(
      () => request<any>("v1/cluster/sync/status", { method: "GET" }),
      () => simulationEngine.getClusterSyncStatus()
    ),

  // Learning Controller
  getLearningPolicies: () =>
    withFallback(
      () =>
        request<{
          tenant_id: string;
          active_policy: any;
          canary_policy: any;
          tenant_feedback_events: number;
          total_feedback_events: number;
          audit_trail: any[];
        }>("v1/learning/policies", { method: "GET" }),
      () => simulationEngine.getLearningPolicies() as any
    ),

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
    withFallback(
      () =>
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
      () => simulationEngine.evaluateSecurity(prompt, model) as any
    ),

  getSecurityAudit: () =>
    withFallback(
      () =>
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
      () => simulationEngine.getSecurityAudit() as any
    ),

  // Governance: Cryptographic Merkle DAG Attestation & Verification
  generateMerkleAttestation: (leafHashes: string[]) =>
    withFallback(
      () =>
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
      () => simulationEngine.generateMerkleAttestation(leafHashes) as any
    ),

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
    withFallback(
      () => request<any>("v1/finops/report", { method: "GET" }),
      () => simulationEngine.getFinopsReport()
    ),

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

  // Real-Time Causal (RTC) Graph Caching & Lineage
  getGraphStats: () =>
    request<{
      node_count: number;
      edge_count: number;
      cached_entries: number;
      max_capacity: number;
      invalidated_records: number;
      hits: number;
      misses: number;
    }>("v1/cache/graph/stats", { method: "GET" }),

  getGraphLineage: (key: string) =>
    request<{
      key: string;
      is_cached: boolean;
      upstream_source_ids: string[];
    }>(`v1/cache/graph/lineage/${encodeURIComponent(key)}`, { method: "GET" }),

  invalidateGraphRtc: (payload: { source_ids?: string[]; doc_hashes?: number[] }) =>
    request<{
      status: string;
      evicted_records: number;
    }>("v1/cache/graph/invalidate", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  // Graphify Multi-Relational Knowledge Graph & Causal Invalidation
  getGraphTopology: () =>
    request<{
      status: string;
      engine: string;
      total_nodes: number;
      total_edges: number;
      node_type_counts: Record<string, number>;
      sample_nodes: any[];
    }>("v1/graph/topology", { method: "GET" }),

  addGraphNode: (payload: {
    id: string;
    label: string;
    node_type: string;
    source_file?: string;
    source_location?: string;
    content_hash?: number;
    community_id?: number;
    metadata?: any;
  }) =>
    request<any>("v1/graph/nodes", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  getGraphNode: (id: string) =>
    request<any>(`v1/graph/nodes/${encodeURIComponent(id)}`, { method: "GET" }),

  deleteGraphNode: (id: string) =>
    request<any>(`v1/graph/nodes/${encodeURIComponent(id)}`, { method: "DELETE" }),

  addGraphEdge: (payload: {
    source_id: string;
    target_id: string;
    relation: string;
    confidence?: string;
    weight?: number;
    source_file?: string;
    source_location?: string;
    metadata?: any;
  }) =>
    request<any>("v1/graph/edges", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  calculateBlastRadius: (payload: {
    mutated_roots: string[];
    max_depth?: number;
    allowed_relations?: string[];
    include_cache_entries?: boolean;
  }) =>
    request<{
      mutated_roots: string[];
      affected_nodes: Array<{
        node_id: string;
        depth: number;
        via_relation: string;
        via_file?: string;
        via_location?: string;
      }>;
      affected_cache_keys: string[];
      max_depth_reached: number;
      total_affected: number;
    }>("v1/graph/blast-radius", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  invalidateGraphCausal: (payload: {
    mutated_roots: string[];
    tenant_id?: string;
    max_depth?: number;
    allowed_relations?: string[];
  }) =>
    request<{
      status: string;
      mutated_roots: string[];
      max_depth_reached: number;
      total_affected_nodes: number;
      affected_nodes: any[];
      affected_cache_keys: string[];
      purged_keys: string[];
      evicted_cache_count: number;
      failed_keys: any[];
      latency_ms: number;
      finops: {
        avoided_tokens_estimate: number;
        avoided_usd: number;
        avoided_inr: number;
        avoided_inr_formatted: string;
        avoided_micro_paise: number;
      };
      timestamp: string;
    }>("v1/graph/invalidate", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  explainGraphNode: (id: string) =>
    request<any>(`v1/graph/explain/${encodeURIComponent(id)}`, { method: "GET" }),

  getGraphCommunities: () =>
    request<any>("v1/graph/communities", { method: "GET" }),

  getGraphGodNodes: () =>
    request<any>("v1/graph/god-nodes", { method: "GET" }),

  getGraphSubgraph: (id: string) =>
    request<any>(`v1/graph/subgraph/${encodeURIComponent(id)}`, { method: "GET" }),

  // Sovereign Indian Governance & Compliance (DPDP Act 2023, CERT-In, RBI Localization)
  recordDpdpConsent: (payload: {
    tenant_id: string;
    data_principal_id: string;
    purpose: string;
    retention_period_days: number;
    consent_notice_version: string;
  }) =>
    request<{
      status: string;
      consent_id: string;
      outbox_id: number;
      principal_hash: string;
      tenant_id: string;
      purpose: string;
      expires_at: string;
      compliance: string;
    }>("v1/governance/dpdp/consent", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  executeDpdpErasure: (payload: {
    tenant_id: string;
    data_principal_id: string;
    reason?: string;
  }) =>
    request<{
      status: string;
      statute: string;
      tenant_id: string;
      principal_hash: string;
      exact_cache_records_purged: number;
      latency_ms: number;
      timestamp: string;
    }>("v1/governance/dpdp/erasure", {
      method: "POST",
      body: JSON.stringify(payload),
    }),

  getDpdpAudit: () =>
    request<{
      status: string;
      fiduciary_jurisdiction: string;
      governing_statute: string;
      data_fiduciary_classification: string;
      consent_architecture: string;
      crypto_shredding_enabled: boolean;
      retention_audit_frequency_days: number;
      cross_border_transfer_policy: string;
      grievance_redressal_window_hours: number;
      timestamp: string;
    }>("v1/governance/dpdp/audit", { method: "GET" }),

  getCertInIncidents: () =>
    request<{
      status: string;
      cert_in_statutory_directive: string;
      mandatory_reporting_sla_hours: number;
      incidents_logged: number;
      incident_log: any[];
      designated_incident_response_officer: string;
      timestamp: string;
    }>("v1/governance/cert-in/incidents", { method: "GET" }),

  verifyRbiLocalization: () =>
    request<{
      status: string;
      regulatory_authority: string;
      circular_reference: string;
      primary_datacenter_region: string;
      disaster_recovery_datacenter_region: string;
      cross_border_financial_data_mirroring: string;
      full_end_to_end_localization_verified: boolean;
      gst_sac_code: string;
      timestamp: string;
    }>("v1/governance/rbi/localization", { method: "GET" }),
};

