export type CacheTier = "c1_exact" | "c2_semantic" | "origin";

export interface SimResult {
  outcome: "hit" | "miss";
  tier: CacheTier;
  answer: string;
  matchedPrompt: string | null;
  similarity: number;
  latencyMs: number;
  costUsd: number;
  savedUsd: number;
  guardrail: string | null;
  naiveWouldReuse: boolean;
}

export interface SimulatedCache {
  query(prompt: string): SimResult;
  size(): number;
  clear(): void;
}

export const SEMANTIC_THRESHOLD: number;
export function normalize(prompt: string): string;
export function tokenize(prompt: string): string[];
export function contentWords(prompt: string): Set<string>;
export function numbersIn(prompt: string): string[];
export function similarity(a: string, b: string): number;
export function guardrailViolation(cachedPrompt: string, newPrompt: string): string | null;
export function estimateTokens(text: string): number;
export function originCost(prompt: string, answer: string): number;
export function createSimulatedCache(opts?: {
  answerFor?: (prompt: string) => string;
  rng?: () => number;
}): SimulatedCache;
