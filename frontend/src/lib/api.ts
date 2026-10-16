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

  // FinOps & Metrics
  getFinOpsReport: () =>
    request<any>("v1/finops/report", { method: "GET" }),
};
