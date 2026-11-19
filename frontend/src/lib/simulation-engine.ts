/**
 * Sunborn Deterministic Simulation Fallback Engine
 * Provides authentic, mathematically calibrated mock fixtures and offline simulation
 * generators for zero-cost public evaluation and graceful offline failover.
 */

export interface SimulatedTraceEvent {
  id: string;
  origin: string;
  event: string;
  detail: string;
  status: string;
  time: string;
}

export interface SimulatedChatCompletion {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: { role: string; content: string };
    finish_reason: string;
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    estimated_cost_avoided_usd?: number;
  };
  sunborn_cached: boolean;
  sunborn_tier: "exact" | "semantic" | "none";
  decision: {
    action: "REUSE" | "ABSTAIN";
    reason_code: string;
    similarity_score?: number;
    latency_us: number;
  };
  _simulated: boolean;
}

export interface SimulatedShredReceipt {
  tenant_id: string;
  status: "CRYPTOGRAPHICALLY_SHREDDED";
  shredded_at: string;
  algorithm: string;
  keys_erased: number;
  verified_multi_store_erasure: boolean;
  tamper_evident_receipt_hash: string;
  _simulated: boolean;
}

export const simulationEngine = {
  async chatCompletions(
    body: any,
    preset: "exact" | "semantic" | "negation" | "dag" = "exact"
  ): Promise<SimulatedChatCompletion> {
    const isNegation =
      preset === "negation" ||
      (body?.messages &&
        JSON.stringify(body.messages).toLowerCase().includes("bypass"));

    const isSemantic = preset === "semantic";
    const isDag = preset === "dag";

    if (isNegation) {
      return {
        id: `chatcmpl_sim_${Math.random().toString(36).substring(2, 9)}`,
        object: "chat.completion",
        created: Math.floor(Date.now() / 1000),
        model: body?.model || "gpt-4o",
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
              content:
                "Conformal abstention triggered: Request failed structural polarity guardrail (adversarial negation detected). Transmitted to upstream origin safely.",
            },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: 38,
          completion_tokens: 28,
          total_tokens: 66,
          estimated_cost_avoided_usd: 0.0,
        },
        sunborn_cached: false,
        sunborn_tier: "none",
        decision: {
          action: "ABSTAIN",
          reason_code: "POLARITY_NEGATION_DETECTED",
          similarity_score: 0.965,
          latency_us: 1240,
        },
        _simulated: true,
      };
    }

    if (isSemantic) {
      return {
        id: `chatcmpl_sim_${Math.random().toString(36).substring(2, 9)}`,
        object: "chat.completion",
        created: Math.floor(Date.now() / 1000),
        model: body?.model || "gpt-4o",
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
              content:
                "Deterministic semantic reuse verified from Qdrant HNSW vector index under conformal risk gate α ≤ 0.001.",
            },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: 44,
          completion_tokens: 22,
          total_tokens: 66,
          estimated_cost_avoided_usd: 0.00045,
        },
        sunborn_cached: true,
        sunborn_tier: "semantic",
        decision: {
          action: "REUSE",
          reason_code: "CONFORMAL_GATE_PASSED",
          similarity_score: 0.941,
          latency_us: 420,
        },
        _simulated: true,
      };
    }

    if (isDag) {
      return {
        id: `chatcmpl_sim_${Math.random().toString(36).substring(2, 9)}`,
        object: "chat.completion",
        created: Math.floor(Date.now() / 1000),
        model: body?.model || "gpt-4o",
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
              content:
                "Multi-step agent workflow DAG executed: 4 cached subgraph nodes reused, Merkle attestation proof verified.",
            },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: 82,
          completion_tokens: 34,
          total_tokens: 116,
          estimated_cost_avoided_usd: 0.00115,
        },
        sunborn_cached: true,
        sunborn_tier: "exact",
        decision: {
          action: "REUSE",
          reason_code: "MERKLE_SUBGRAPH_REUSED",
          latency_us: 180,
        },
        _simulated: true,
      };
    }

    // Default: Exact L1 Hit
    return {
      id: `chatcmpl_sim_${Math.random().toString(36).substring(2, 9)}`,
      object: "chat.completion",
      created: Math.floor(Date.now() / 1000),
      model: body?.model || "gpt-4o",
      choices: [
        {
          index: 0,
          message: {
            role: "assistant",
            content:
              "Verified execution reuse from Sunborn DragonflyDB exact tier.",
          },
          finish_reason: "stop",
        },
      ],
      usage: {
        prompt_tokens: 30,
        completion_tokens: 15,
        total_tokens: 45,
        estimated_cost_avoided_usd: 0.00032,
      },
      sunborn_cached: true,
      sunborn_tier: "exact",
      decision: {
        action: "REUSE",
        reason_code: "L1_EXACT_HASH_HIT",
        latency_us: 45,
      },
      _simulated: true,
    };
  },

  async shredTenant(tenantId: string): Promise<SimulatedShredReceipt> {
    return {
      tenant_id: tenantId,
      status: "CRYPTOGRAPHICALLY_SHREDDED",
      shredded_at: new Date().toISOString(),
      algorithm: "AES-256-GCM TDEK Zeroization",
      keys_erased: 3,
      verified_multi_store_erasure: true,
      tamper_evident_receipt_hash: `sha256:${Math.random().toString(36).substring(2)}${Math.random().toString(36).substring(2)}`,
      _simulated: true,
    };
  },

  async clusterStatus() {
    return {
      status: "AUTONOMIC_ACTIVE",
      cluster_state: "AUTONOMIC_ACTIVE",
      vector_clock: {
        clocks: {
          "node-us-east-1a": 42,
          "node-us-west-2b": 42,
          "node-eu-central-1a": 41,
        },
      },
      active_replicas: 6,
      healthy_percentage: 100,
      _simulated: true,
    };
  },

  async finopsReport() {
    return {
      metrics: {
        requests_total: 1248950,
        total_hits: 1091580,
      },
      economics: {
        total_estimated_savings_usd: 48920.45,
      },
      _simulated: true,
    };
  },

  async getSecurityAudit() {
    return {
      events: [
        {
          id: "sec_sim_01",
          event: "ZERO_TRUST_VERIFIED",
          origin: "Gateway Ingress",
          detail: "Tenant tenant-enterprise-fintech authenticated",
          status: "200_OK",
          time: "Just now",
        },
        {
          id: "sec_sim_02",
          event: "NEGATION_DEFENSE_TRIGGERED",
          origin: "Structural Guardrail Engine",
          detail: "Adversarial polarity shift blocked",
          status: "200_ABSTAIN",
          time: "1m ago",
        },
      ],
      _simulated: true,
    };
  },
};

export function generateSimulatedTrace(
  action: string,
  tier: string,
  avoidedTokens: number = 184,
  latencyMs: number = 0.28
): SimulatedTraceEvent {
  return {
    id: `tr_${Math.random().toString(36).substring(2, 8)}`,
    origin: "Console Ingress",
    event: action,
    detail: `${tier} · avoided: ${avoidedTokens} tokens · latency: ${latencyMs.toFixed(2)}ms`,
    status: avoidedTokens > 0 ? "200_HIT" : "200_ORIGIN",
    time: "Just now",
  };
}

export function getDeterministicBenchmarkMetrics() {
  return {
    conformalPrecision: 100.0,
    observedFalseHitRate: 0.0,
    benchmarkTokenSavings: 12.09,
    gdprShreddingLatencyUs: 42.1,
  };
}
