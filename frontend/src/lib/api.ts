/**
 * Sunborn Unified Typed API Client
 * Enforces server-side authentication, structured error propagation,
 * and zero simulated fallbacks.
 */

export const API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL || "http://127.0.0.1:8080";

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

async function request<T>(
  path: string,
  options: RequestInit = {},
  apiKey = "sunborn-admin-key"
): Promise<T> {
  const url = `${API_BASE_URL.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
  const headers = new Headers(options.headers || {});

  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }
  if (!headers.has("x-api-key") && apiKey) {
    headers.set("x-api-key", apiKey);
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
  chatCompletions: (body: any, apiKey = "key-enterprise-fintech") =>
    request<any>("v1/chat/completions", {
      method: "POST",
      body: JSON.stringify(body),
    }, apiKey),

  // Governance: Cryptographic shredding
  shredTenant: (tenantId: string) =>
    request<{ status: string; shredding_latency_us?: number }>(
      "v1/governance/shred",
      {
        method: "POST",
        body: JSON.stringify({ tenant_id: tenantId }),
      }
    ),

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

  // Chaos & Circuit Breaker
  tripCircuit: () =>
    request<any>("v1/chaos/trip", { method: "POST" }),
  resetCircuit: () =>
    request<any>("v1/chaos/reset", { method: "POST" }),
  getChaosStatus: () =>
    request<any>("v1/chaos/status", { method: "GET" }),

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
