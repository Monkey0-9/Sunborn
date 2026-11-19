/**
 * Sunborn Shared Simulation & Demonstration Engine
 * Provides deterministic simulation fixtures and offline mock generators
 * ensuring zero unhandled runtime exceptions during standalone demonstrations.
 */

export interface SimulatedTraceEvent {
  id: string;
  origin: string;
  event: string;
  detail: string;
  status: string;
  time: string;
}

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
