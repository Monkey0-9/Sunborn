import { originCost, type SimResult } from "./sim-cache.mjs";

export interface LiveSettings {
  baseUrl: string;
  apiKey: string;
  model: string;
}

export const DEFAULT_LIVE: LiveSettings = {
  baseUrl: "http://localhost:8080",
  apiKey: "",
  model: "gemini-3.8-flash",
};

function cleanBase(url: string) {
  return url.trim().replace(/\/+$/, "");
}

function explainNetworkError(base: string, err: unknown): Error {
  const msg = err instanceof Error ? err.message : String(err);
  return new Error(
    `Could not reach the gateway at ${base} (${msg}). Check that it is running, that this page's address is in SUNBORN_CORS_ALLOWED_ORIGINS, and that your browser allowed access to your local network.`
  );
}

export async function checkHealth(settings: LiveSettings): Promise<{ ok: boolean; detail: string }> {
  const base = cleanBase(settings.baseUrl);
  try {
    const res = await fetch(`${base}/health`, { cache: "no-store", signal: AbortSignal.timeout(4000) });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) return { ok: false, detail: `HTTP ${res.status}` };
    return { ok: true, detail: `${body.service || "sunborn-gateway"} ${body.version || ""} is ${body.status || "up"}`.trim() };
  } catch (err) {
    return { ok: false, detail: explainNetworkError(base, err).message };
  }
}

/** Sends one prompt through a real Sunborn gateway's OpenAI-compatible endpoint. */
export async function liveQuery(settings: LiveSettings, prompt: string): Promise<SimResult> {
  const base = cleanBase(settings.baseUrl);
  const started = performance.now();
  let res: Response;
  try {
    res = await fetch(`${base}/v1/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(settings.apiKey ? { "x-api-key": settings.apiKey } : {}),
      },
      body: JSON.stringify({
        model: settings.model || DEFAULT_LIVE.model,
        messages: [{ role: "user", content: prompt }],
      }),
      signal: AbortSignal.timeout(60_000),
    });
  } catch (err) {
    throw explainNetworkError(base, err);
  }
  const latencyMs = Math.round((performance.now() - started) * 100) / 100;
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    const detail = body.detail || body.title || body.error?.message || body.error || res.statusText;
    if (res.status === 401 || res.status === 403) {
      throw new Error(`The gateway rejected the API key (HTTP ${res.status}). ${detail || ""}`.trim());
    }
    throw new Error(`Gateway returned HTTP ${res.status}: ${typeof detail === "string" ? detail : JSON.stringify(detail)}`);
  }

  const answer: string = body.choices?.[0]?.message?.content ?? "";
  const cached = Boolean(body.sunborn_cached);
  const rawTier: string = body.sunborn_tier || (cached ? "cached" : "origin");
  const tier = rawTier === "c1_exact" || rawTier === "c2_semantic" ? rawTier : cached ? "c2_semantic" : "origin";
  const cost = originCost(prompt, answer);

  return {
    outcome: cached ? "hit" : "miss",
    tier,
    answer,
    matchedPrompt: null,
    similarity: cached ? 1 : 0,
    latencyMs,
    costUsd: cached ? 0 : cost,
    savedUsd: cached ? cost : 0,
    guardrail: null,
    naiveWouldReuse: false,
  };
}
