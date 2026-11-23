import { normalize, type SimResult } from "./sim-cache.mjs";
import type { TslRunPlan, TslRunStep } from "@/components/tessel/tessel";

export const usd = (n: number) => (n === 0 ? "$0" : n < 0.01 ? `$${n.toFixed(6)}` : `$${n.toFixed(4)}`);

export const ms = (n: number) => (n < 10 ? `${n.toFixed(2)} ms` : `${Math.round(n).toLocaleString("en-US")} ms`);

export function verdict(r: SimResult): { label: string; tone: "hit" | "warn" | "miss" } {
  if (r.tier === "c1_exact") return { label: "Exact hit", tone: "hit" };
  if (r.tier === "c2_semantic") return { label: "Semantic hit", tone: "hit" };
  if (r.guardrail) return { label: "Reuse refused", tone: "warn" };
  return { label: "Miss, sent to model", tone: "miss" };
}

const clip = (s: string, n = 46) => (s.length > n ? s.slice(0, n - 1) + "…" : s);

/** The decision path of one simulated request, as steps the prompt card can play back. */
export function simTrace(prompt: string, r: SimResult): TslRunPlan {
  const steps: TslRunStep[] = [{ tool: "normalize", label: `“${clip(normalize(prompt))}”`, ms: 160, time: "<0.01 ms" }];
  if (r.tier === "c1_exact") {
    steps.push({ tool: "c1.exact", label: "Exact match in memory", ms: 200, time: ms(r.latencyMs), tone: "hit" });
  } else {
    steps.push({ tool: "c1.exact", label: "No exact match", ms: 200, time: "0.3 ms" });
    steps.push({
      tool: "c2.semantic",
      label: r.matchedPrompt ? `Closest: “${clip(r.matchedPrompt, 34)}” · ${r.similarity.toFixed(2)}` : "Nothing similar cached yet",
      ms: 300,
      time: "1.9 ms",
      tone: r.tier === "c2_semantic" ? "hit" : undefined,
    });
    if (r.tier === "c2_semantic") {
      steps.push({ tool: "guardrails", label: "Numbers, negation and direction unchanged", ms: 260, time: ms(r.latencyMs), tone: "ok" });
    } else if (r.guardrail) {
      steps.push({ tool: "guardrails", label: `Refused: ${r.guardrail}`, ms: 300, time: "0.1 ms", tone: "warn" });
    }
    if (r.tier === "origin") {
      steps.push({ tool: "origin.model", label: "Asked the model and cached the answer", ms: Math.min(r.latencyMs, 1100), time: ms(r.latencyMs) });
    }
  }
  const v = verdict(r);
  const tail = r.outcome === "hit" ? `${v.label} · ${ms(r.latencyMs)}` : v.label === "Reuse refused" ? "Refused · sent to model" : "Miss · sent to model";
  return { kind: r.tier, title: "Decision path", steps, answer: r.answer, verdict: tail, tone: v.tone };
}

/** What a live gateway response tells us, as steps. */
export function liveTrace(r: SimResult, model: string): TslRunPlan {
  const v = verdict(r);
  const steps: TslRunStep[] = [
    { tool: "POST /v1/chat", label: `Sent to your gateway · ${model}`, ms: 120, time: ms(r.latencyMs) },
    {
      tool: "sunborn_tier",
      label: r.tier === "origin" ? "origin: answered by the model, now cached" : `${r.tier}: answered from cache`,
      ms: 200,
      time: r.outcome === "hit" ? "cached" : "",
      tone: r.outcome === "hit" ? "hit" : undefined,
    },
  ];
  return { kind: r.tier, title: "Gateway response", steps, answer: r.answer || "(empty response)", verdict: `${v.label} · ${ms(r.latencyMs)}`, tone: v.tone };
}
