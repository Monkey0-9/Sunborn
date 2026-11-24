/**
 * Types and interfaces for the Sunborn TypeScript SDK.
 * Fully aligned with Sunborn OpenAPI 3.1 specification contracts.
 */

export type CachePath =
  | "exact"
  | "semantic"
  | "revalidate"
  | "origin"
  | "none";

export type DecisionAction =
  | "safe_hit"
  | "abstain_low_confidence"
  | "abstain_drift_detected"
  | "abstain_structural_inversion"
  | "abstain_causal_invalidation"
  | "origin_execute";

export interface DecisionRecord {
  action: DecisionAction;
  cache_path: CachePath;
  confidence: number;
  risk_score: number;
  reason_code: string;
  latency_saved_ms?: number;
  policy_version?: number;
}

export interface AttestationRecord {
  request_id: string;
  merkle_root: string;
  signature: string;
  key_id: string;
  timestamp: string;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  name?: string;
}

export interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  temperature?: number;
  top_p?: number;
  max_tokens?: number;
  stop?: string[];
  response_format?: { type: string };
  tenant_id?: string;
  application_id?: string;
}

export interface ChatCompletionChoice {
  index: number;
  message: ChatMessage;
  finish_reason: string;
}

export interface UsageMetrics {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  estimated_cost_avoided_usd?: number;
}

export interface ChatCompletionResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: ChatCompletionChoice[];
  usage: UsageMetrics;
  decision: DecisionRecord;
  cached: boolean;
  tier: string;
  attestation?: AttestationRecord;
}

export interface SunbornConfig {
  baseUrl?: string;
  apiKey?: string;
  timeoutMs?: number;
  maxRetries?: number;
  backoffFactor?: number;
}

export interface ChatCompletionChunkChoice {
  index: number;
  delta: {
    role?: string;
    content?: string;
  };
  finish_reason: string | null;
}

export interface ChatCompletionChunk {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: ChatCompletionChunkChoice[];
}

export interface GatewayHealth {
  service: string;
  status: string;
  version: string;
}

export interface GatewayReadiness {
  ready: boolean;
  dragonfly?: string;
  postgres?: string;
  qdrant?: string;
}
