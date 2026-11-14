"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Loader2,
  PlugZap,
  RotateCcw,
  Send,
  ShieldAlert,
  Sparkles,
  Zap,
} from "lucide-react";
import { createSimulatedCache, type SimResult } from "@/lib/sim-cache.mjs";
import { DEFAULT_LIVE, checkHealth, liveQuery, type LiveSettings } from "@/lib/live";
import { SCENARIOS, simulatedAnswer } from "@/lib/scenarios";
import { blob } from "@/lib/site";

type Mode = "sim" | "live";

interface HistoryItem extends SimResult {
  id: number;
  prompt: string;
  mode: Mode;
}

const STORAGE_KEY = "sunborn-playground-live";

function loadLive(): LiveSettings {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_LIVE, ...JSON.parse(raw) };
  } catch {
    // storage unavailable: fall back to defaults
  }
  return DEFAULT_LIVE;
}

function saveLive(s: LiveSettings) {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {
    // ignore
  }
}

const usd = (n: number) =>
  n === 0 ? "$0" : n < 0.01 ? `$${n.toFixed(6)}` : `$${n.toFixed(4)}`;

const ms = (n: number) => (n < 10 ? `${n.toFixed(2)} ms` : `${Math.round(n).toLocaleString()} ms`);

function verdict(r: SimResult) {
  if (r.tier === "c1_exact") return { label: "Exact hit", tone: "cyan", Icon: Zap };
  if (r.tier === "c2_semantic") return { label: "Semantic hit", tone: "violet", Icon: Sparkles };
  if (r.guardrail) return { label: "Reuse refused", tone: "amber", Icon: ShieldAlert };
  return { label: "Miss, sent to model", tone: "slate", Icon: CircleDashed };
}

const TONE: Record<string, string> = {
  cyan: "border-cyan-500/40 bg-cyan-950/40 text-cyan-300",
  violet: "border-violet-500/40 bg-violet-950/40 text-violet-300",
  amber: "border-amber-500/40 bg-amber-950/40 text-amber-300",
  slate: "border-white/15 bg-white/[0.04] text-slate-300",
};

export function Playground() {
  const [mode, setMode] = useState<Mode>("sim");
  const [live, setLive] = useState<LiveSettings>(DEFAULT_LIVE);
  const [health, setHealth] = useState<{ state: "idle" | "checking" | "ok" | "fail"; detail?: string }>({ state: "idle" });
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const nextId = useRef(1);
  const sim = useMemo(() => createSimulatedCache({ answerFor: simulatedAnswer }), []);

  useEffect(() => setLive(loadLive()), []);

  const updateLive = (patch: Partial<LiveSettings>) => {
    setLive((prev) => {
      const next = { ...prev, ...patch };
      saveLive(next);
      return next;
    });
    setHealth({ state: "idle" });
  };

  const send = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || busy) return;
      setBusy(true);
      setError(null);
      try {
        let result: SimResult;
        if (mode === "sim") {
          // A short pause on misses so the difference is felt, not just read.
          result = sim.query(trimmed);
          if (result.outcome === "miss") await new Promise((r) => setTimeout(r, Math.min(result.latencyMs, 900)));
        } else {
          result = await liveQuery(live, trimmed);
        }
        setHistory((h) => [{ ...result, id: nextId.current++, prompt: trimmed, mode }, ...h]);
        setPrompt("");
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusy(false);
      }
    },
    [busy, live, mode, sim]
  );

  const reset = () => {
    sim.clear();
    setHistory([]);
    setError(null);
  };

  const test = async () => {
    setHealth({ state: "checking" });
    const r = await checkHealth(live);
    setHealth({ state: r.ok ? "ok" : "fail", detail: r.detail });
  };

  const stats = useMemo(() => {
    const total = history.length;
    const hits = history.filter((h) => h.outcome === "hit");
    const misses = history.filter((h) => h.outcome === "miss");
    const avg = (xs: HistoryItem[]) => (xs.length ? xs.reduce((a, b) => a + b.latencyMs, 0) / xs.length : null);
    return {
      total,
      hitRate: total ? Math.round((hits.length / total) * 100) : 0,
      saved: history.reduce((a, b) => a + b.savedUsd, 0),
      spent: history.reduce((a, b) => a + b.costUsd, 0),
      avgHit: avg(hits),
      avgMiss: avg(misses),
      refused: history.filter((h) => h.guardrail).length,
    };
  }, [history]);

  const latest = history[0];
  const doneIds = new Set(history.filter((h) => h.mode === mode).map((h) => h.prompt));
  const nextScenario = SCENARIOS.find((s) => !doneIds.has(s.prompt));

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_340px]">
      <div className="min-w-0 space-y-6">
        {/* Mode switch */}
        <div className="panel p-2">
          <div role="tablist" aria-label="Playground mode" className="grid grid-cols-2 gap-2">
            {(["sim", "live"] as const).map((m) => (
              <button
                key={m}
                role="tab"
                aria-selected={mode === m}
                onClick={() => {
                  setMode(m);
                  setError(null);
                }}
                className={
                  "rounded-xl px-4 py-2.5 text-sm font-medium transition " +
                  (mode === m ? "bg-white/[0.09] text-white" : "text-slate-400 hover:text-white")
                }
              >
                {m === "sim" ? "Simulation" : "Live gateway"}
              </button>
            ))}
          </div>
        </div>

        {mode === "sim" ? (
          <p className="rounded-xl border border-white/[0.08] bg-white/[0.02] px-4 py-3 text-sm text-slate-400">
            <strong className="text-slate-200">Simulation mode.</strong> Runs entirely in your browser with no gateway and
            no API key. The decision steps match the real gateway, but matching is simplified and latency and cost figures
            are illustrative.
          </p>
        ) : (
          <div className="panel space-y-4 p-5">
            <div className="flex items-start gap-3">
              <PlugZap className="mt-0.5 h-5 w-5 shrink-0 text-cyan" aria-hidden />
              <p className="text-sm text-slate-400">
                Connect to a Sunborn gateway you are running yourself. Requests go straight from your browser to it, and
                your key is kept only in this tab. Setup steps are in the{" "}
                <a className="underline underline-offset-2 hover:text-slate-200" href={blob("site/README.md")}>
                  site README
                </a>
                .
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-3">
              <label className="block text-xs text-slate-500 sm:col-span-1">
                Gateway URL
                <input
                  value={live.baseUrl}
                  onChange={(e) => updateLive({ baseUrl: e.target.value })}
                  className="mt-1 w-full rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none focus:border-cyan-500/50"
                  spellCheck={false}
                />
              </label>
              <label className="block text-xs text-slate-500">
                Sunborn API key
                <input
                  type="password"
                  value={live.apiKey}
                  onChange={(e) => updateLive({ apiKey: e.target.value })}
                  placeholder="x-api-key"
                  autoComplete="off"
                  className="mt-1 w-full rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none focus:border-cyan-500/50"
                />
              </label>
              <label className="block text-xs text-slate-500">
                Model
                <input
                  value={live.model}
                  onChange={(e) => updateLive({ model: e.target.value })}
                  className="mt-1 w-full rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-slate-100 outline-none focus:border-cyan-500/50"
                  spellCheck={false}
                />
              </label>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <button
                onClick={test}
                className="inline-flex items-center gap-2 rounded-lg border border-white/15 px-3 py-1.5 text-sm text-slate-200 hover:border-white/30"
              >
                {health.state === "checking" ? <Loader2 className="h-4 w-4 animate-spin" aria-hidden /> : null}
                Test connection
              </button>
              {health.state === "ok" && (
                <span className="inline-flex items-center gap-1.5 text-sm text-emerald-glow">
                  <CheckCircle2 className="h-4 w-4" aria-hidden /> {health.detail}
                </span>
              )}
              {health.state === "fail" && <span className="text-sm text-rose-glow">{health.detail}</span>}
            </div>
          </div>
        )}

        {/* Composer */}
        <form
          className="panel p-4"
          onSubmit={(e) => {
            e.preventDefault();
            send(prompt);
          }}
        >
          <label htmlFor="prompt" className="sr-only">
            Prompt
          </label>
          <textarea
            id="prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send(prompt);
              }
            }}
            rows={3}
            placeholder="Ask anything, then ask it again in different words…"
            className="w-full resize-none bg-transparent text-base text-slate-100 outline-none placeholder:text-slate-600"
          />
          <div className="mt-3 flex items-center justify-between gap-3">
            <span className="text-xs text-slate-600">Enter to send · Shift+Enter for a new line</span>
            <button
              type="submit"
              disabled={busy || !prompt.trim()}
              className="inline-flex items-center gap-2 rounded-xl bg-cyan px-4 py-2 text-sm font-semibold text-slate-950 transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {busy ? <Loader2 className="h-4 w-4 animate-spin" aria-hidden /> : <Send className="h-4 w-4" aria-hidden />}
              Send
            </button>
          </div>
        </form>

        {error && (
          <div role="alert" className="flex gap-3 rounded-xl border border-rose-500/40 bg-rose-950/40 px-4 py-3 text-sm text-rose-200">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden />
            <span>{error}</span>
          </div>
        )}

        {/* Latest result */}
        <div aria-live="polite">
          {latest ? <ResultCard item={latest} /> : <EmptyState />}
        </div>

        {/* History */}
        {history.length > 1 && (
          <section>
            <h2 className="mb-3 text-sm font-semibold uppercase tracking-wider text-slate-500">Earlier requests</h2>
            <ul className="space-y-2">
              {history.slice(1).map((h) => {
                const v = verdict(h);
                return (
                  <li key={h.id} className="flex items-center justify-between gap-4 rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 text-sm">
                    <span className="min-w-0 truncate text-slate-300">{h.prompt}</span>
                    <span className="flex shrink-0 items-center gap-3">
                      <span className="tabular-nums text-slate-500">{ms(h.latencyMs)}</span>
                      <span className={`rounded-full border px-2 py-0.5 text-xs ${TONE[v.tone]}`}>{v.label}</span>
                    </span>
                  </li>
                );
              })}
            </ul>
          </section>
        )}
      </div>

      {/* Sidebar */}
      <aside className="space-y-6">
        <div className="panel p-5">
          <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-500">Guided tour</h2>
          <p className="mt-1 text-sm text-slate-500">Click in order. Each step uses what the cache learned before it.</p>
          <ol className="mt-4 space-y-2">
            {SCENARIOS.map((s, i) => {
              const done = doneIds.has(s.prompt);
              const isNext = nextScenario?.id === s.id;
              return (
                <li key={s.id}>
                  <button
                    onClick={() => send(s.prompt)}
                    disabled={busy}
                    className={
                      "w-full rounded-xl border px-3 py-2.5 text-left transition disabled:opacity-60 " +
                      (isNext
                        ? "border-cyan-500/40 bg-cyan-950/30 hover:bg-cyan-950/50"
                        : "border-white/[0.06] bg-white/[0.02] hover:border-white/15")
                    }
                  >
                    <span className="flex items-center gap-2 text-sm font-medium text-slate-200">
                      <span className={`text-xs tabular-nums ${done ? "text-emerald-glow" : "text-slate-500"}`}>
                        {done ? "✓" : i + 1}
                      </span>
                      {s.label}
                    </span>
                    <span className="mt-0.5 block truncate pl-5 text-xs text-slate-500">“{s.prompt}”</span>
                    <span className="mt-0.5 block pl-5 text-xs text-slate-600">Expect: {s.expect}</span>
                  </button>
                </li>
              );
            })}
          </ol>
        </div>

        <div className="panel p-5">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-500">This session</h2>
            <button
              onClick={reset}
              className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs text-slate-500 hover:text-slate-200"
            >
              <RotateCcw className="h-3.5 w-3.5" aria-hidden /> Reset
            </button>
          </div>
          <dl className="mt-4 grid grid-cols-2 gap-4">
            <Stat label="Requests" value={String(stats.total)} />
            <Stat label="Hit rate" value={`${stats.hitRate}%`} />
            <Stat label="Saved" value={usd(stats.saved)} tone="text-emerald-glow" />
            <Stat label="Spent" value={usd(stats.spent)} />
            <Stat label="Avg hit" value={stats.avgHit === null ? "–" : ms(stats.avgHit)} tone="text-cyan" />
            <Stat label="Avg miss" value={stats.avgMiss === null ? "–" : ms(stats.avgMiss)} />
            <Stat label="Unsafe reuses blocked" value={String(stats.refused)} tone="text-amber-glow" wide />
          </dl>
          {mode === "sim" && (
            <p className="mt-4 text-xs text-slate-600">Costs use Gemini list prices configured in the gateway.</p>
          )}
        </div>
      </aside>
    </div>
  );
}

function Stat({ label, value, tone, wide }: { label: string; value: string; tone?: string; wide?: boolean }) {
  return (
    <div className={wide ? "col-span-2" : undefined}>
      <dt className="text-xs text-slate-500">{label}</dt>
      <dd className={`mt-0.5 text-lg font-semibold tabular-nums ${tone || "text-slate-100"}`}>{value}</dd>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="rounded-2xl border border-dashed border-white/10 px-6 py-12 text-center text-sm text-slate-500">
      Send a prompt or start the guided tour. The cache starts empty, so the first request always goes to the model.
    </div>
  );
}

function ResultCard({ item }: { item: HistoryItem }) {
  const v = verdict(item);
  return (
    <article className="panel space-y-5 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm font-medium ${TONE[v.tone]}`}>
          <v.Icon className="h-4 w-4" aria-hidden /> {v.label}
        </span>
        <span className="text-xs uppercase tracking-wider text-slate-600">
          {item.mode === "sim" ? "Simulated" : "Live gateway"}
        </span>
      </div>

      <p className="text-slate-300">
        <span className="text-slate-500">You asked: </span>“{item.prompt}”
      </p>

      <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <Stat label="Latency" value={ms(item.latencyMs)} tone={item.outcome === "hit" ? "text-cyan" : undefined} />
        <Stat label="Cost" value={usd(item.costUsd)} />
        <Stat label="Saved" value={usd(item.savedUsd)} tone={item.savedUsd ? "text-emerald-glow" : undefined} />
        <Stat label="Tier" value={item.tier} />
      </dl>

      {item.mode === "sim" && item.matchedPrompt && item.tier !== "c1_exact" && (
        <p className="text-sm text-slate-400">
          Closest cached prompt: “{item.matchedPrompt}” (similarity {item.similarity.toFixed(2)})
        </p>
      )}

      {item.guardrail && (
        <div className="rounded-xl border border-amber-500/30 bg-amber-950/30 px-4 py-3 text-sm">
          <p className="font-medium text-amber-200">Guardrail: {item.guardrail}</p>
          <p className="mt-1 text-amber-100/70">
            The wording is close enough that a plain similarity cache would have returned the cached answer, which would
            have been wrong here. Sunborn sent it to the model instead.
          </p>
        </div>
      )}

      <div className="rounded-xl border border-white/[0.06] bg-black/20 px-4 py-3">
        <p className="text-xs uppercase tracking-wider text-slate-600">
          {item.outcome === "hit" ? "Answer (from cache)" : "Answer (from model)"}
        </p>
        <p className="mt-1.5 whitespace-pre-wrap leading-relaxed text-slate-200">{item.answer || "(empty response)"}</p>
      </div>
    </article>
  );
}
