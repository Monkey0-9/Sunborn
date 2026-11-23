"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon, Kicker, Mosaic, PromptCard, useTsl, type TslPlanner } from "@/components/tessel/tessel";
import { SunbornShell } from "@/components/sunborn/shell";
import { createSimulatedCache, type SimResult } from "@/lib/sim-cache.mjs";
import { DEFAULT_LIVE, checkHealth, liveQuery, type LiveSettings } from "@/lib/live";
import { SCENARIOS, simulatedAnswer } from "@/lib/scenarios";
import { liveTrace, ms, simTrace, usd, verdict } from "@/lib/trace";
import { BASE_PATH, blob } from "@/lib/site";

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

export function Playground() {
  return (
    <SunbornShell page="playground">
      <PlaygroundBody />
    </SunbornShell>
  );
}

function PlaygroundBody() {
  const { ramp, reduced } = useTsl();
  const stageState = useRef({ cell: 16, shift: 0.8 });
  const [mode, setMode] = useState<Mode>("sim");
  const [live, setLive] = useState<LiveSettings>(DEFAULT_LIVE);
  const [health, setHealth] = useState<{ state: "idle" | "checking" | "ok" | "fail"; detail?: string }>({ state: "idle" });
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [request, setRequest] = useState<{ text: string; id: number } | null>(null);
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

  const record = useCallback((prompt: string, result: SimResult, m: Mode) => {
    setHistory((h) => [{ ...result, id: nextId.current++, prompt, mode: m }, ...h]);
  }, []);

  const planner: TslPlanner = useCallback(
    async (prompt: string) => {
      if (mode === "sim") {
        const r = sim.query(prompt);
        record(prompt, r, "sim");
        return simTrace(prompt, r);
      }
      const r = await liveQuery(live, prompt);
      record(prompt, r, "live");
      return liveTrace(r, live.model || DEFAULT_LIVE.model);
    },
    [mode, sim, live, record]
  );

  const send = (text: string) => setRequest({ text, id: Date.now() });

  const reset = () => {
    sim.clear();
    setHistory([]);
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
    <>
      <section className="tsl-sec sb-pg-head" data-sec="top">
        <span className="tsl-cross" data-side="l" />
        <span className="tsl-cross" data-side="r" />
        <Kicker text="Playground" />
        <h1 className="tsl-h1">See the Cache Decide, One Prompt at a Time</h1>
        <p className="tsl-lead">
          Send a prompt, then send it again in different words. Watch where the answer comes from, how long it takes and what
          it costs, and see Sunborn refuse to reuse an answer when the meaning changed.
        </p>
      </section>

      <section className="tsl-sec sb-pg" aria-label="Playground">
        <span className="tsl-cross" data-side="l" />
        <span className="tsl-cross" data-side="r" />
        <div className="sb-pg-grid">
          <div className="sb-pg-main">
            <div className="sb-pg-bar">
              <div className="tsl-seg sb-mode" role="tablist" aria-label="Playground mode">
                {(["sim", "live"] as const).map((m) => (
                  <button key={m} type="button" role="tab" aria-selected={mode === m} aria-pressed={mode === m} onClick={() => {
                      setRequest(null);
                      setMode(m);
                    }}>
                    {m === "sim" ? "Simulation" : "Live gateway"}
                  </button>
                ))}
              </div>
              <span className="sb-pg-mode-note">{mode === "sim" ? "In your browser · no key" : "Your gateway · your key stays in this tab"}</span>
            </div>

            {mode === "sim" ? (
              <p className="sb-note">
                <b>Simulation mode.</b> Runs entirely in your browser with no gateway and no API key. The decision steps match
                the real gateway, but matching is simplified and latency and cost figures are illustrative.
              </p>
            ) : (
              <div className="sb-card sb-live">
                <p className="sb-live-intro">
                  Connect to a Sunborn gateway you are running yourself. Requests go straight from your browser to it, and your
                  key is kept only in this tab. Setup steps are in the{" "}
                  <a className="sb-a" href={blob("site/README.md")}>
                    site README
                  </a>
                  .
                </p>
                <div className="sb-live-fields">
                  <label>
                    <span>Gateway URL</span>
                    <input value={live.baseUrl} onChange={(e) => updateLive({ baseUrl: e.target.value })} spellCheck={false} />
                  </label>
                  <label>
                    <span>Sunborn API key</span>
                    <input
                      type="password"
                      value={live.apiKey}
                      onChange={(e) => updateLive({ apiKey: e.target.value })}
                      placeholder="x-api-key"
                      autoComplete="off"
                    />
                  </label>
                  <label>
                    <span>Model</span>
                    <input value={live.model} onChange={(e) => updateLive({ model: e.target.value })} spellCheck={false} />
                  </label>
                </div>
                <div className="sb-live-test">
                  <button type="button" className="tsl-mini" onClick={test} disabled={health.state === "checking"}>
                    <Icon name="replay" size={12} />
                    {health.state === "checking" ? "Checking…" : "Test connection"}
                  </button>
                  {health.state === "ok" ? (
                    <span className="sb-ok" role="status">
                      <Icon name="check" size={12} stroke={2} /> {health.detail}
                    </span>
                  ) : null}
                  {health.state === "fail" ? (
                    <span className="sb-bad" role="status">
                      {health.detail}
                    </span>
                  ) : null}
                </div>
              </div>
            )}

            <div className="tsl-hero-panel sb-pg-stage">
              <Mosaic scene="silk" ramp={ramp} state={stageState.current} reduced={reduced} interactive />
              <PromptCard
                key={mode}
                label={mode === "sim" ? "Sunborn gateway, simulated" : "Your Sunborn gateway"}
                placeholder="Ask anything, then ask it again in different words"
                suggestions={[]}
                models={[mode === "sim" ? "Simulated" : live.model || DEFAULT_LIVE.model]}
                modelHints={[mode === "sim" ? "Runs in this tab, no key" : "Sent to " + (live.baseUrl || DEFAULT_LIVE.baseUrl)]}
                reduced={reduced}
                planner={planner}
                request={request}
              />
              <div className="tsl-hero-ruler" aria-hidden="true">
                <span>{mode === "sim" ? "SIMULATION" : "LIVE GATEWAY"}</span>
                <span>{history.length} REQUESTS</span>
              </div>
            </div>

            <div aria-live="polite">{latest ? <ResultCard item={latest} /> : <EmptyState />}</div>

            {history.length > 1 ? (
              <div className="sb-history">
                <h2 className="tsl-kicker">Earlier requests</h2>
                <ul>
                  {history.slice(1).map((h) => {
                    const v = verdict(h);
                    return (
                      <li key={h.id}>
                        <span className="sb-history-q">{h.prompt}</span>
                        <span className="sb-history-ms">{ms(h.latencyMs)}</span>
                        <span className="sb-tag" data-tone={v.tone}>
                          {v.label}
                        </span>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ) : null}
          </div>

          <aside className="sb-pg-side">
            <div className="sb-card">
              <div className="tsl-card-head">
                <b>Guided tour</b>
                <span className="tsl-pill">
                  <i />
                  {SCENARIOS.filter((s) => doneIds.has(s.prompt)).length}/{SCENARIOS.length}
                </span>
              </div>
              <p className="sb-muted">Click in order. Each step uses what the cache learned before it.</p>
              <ol className="sb-tour">
                {SCENARIOS.map((s, i) => {
                  const done = doneIds.has(s.prompt);
                  const isNext = nextScenario?.id === s.id;
                  return (
                    <li key={s.id}>
                      <button type="button" onClick={() => send(s.prompt)} data-next={isNext ? "" : undefined} data-done={done ? "" : undefined}>
                        <span className="sb-tour-n" aria-hidden="true">
                          {done ? <Icon name="check" size={11} stroke={2} /> : i + 1}
                        </span>
                        <span className="sb-tour-body">
                          <b>{s.label}</b>
                          <small>“{s.prompt}”</small>
                          <em>Expect: {s.expect}</em>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ol>
            </div>

            <div className="sb-card">
              <div className="tsl-card-head">
                <b>This session</b>
                <button type="button" className="tsl-mini" onClick={reset}>
                  <Icon name="replay" size={12} />
                  Reset
                </button>
              </div>
              <dl className="tsl-metrics sb-metrics">
                <Stat label="Requests" value={String(stats.total)} />
                <Stat label="Hit rate" value={`${stats.hitRate}%`} />
                <Stat label="Saved" value={usd(stats.saved)} tone="ok" />
                <Stat label="Spent" value={usd(stats.spent)} />
                <Stat label="Avg hit" value={stats.avgHit === null ? "–" : ms(stats.avgHit)} />
                <Stat label="Avg miss" value={stats.avgMiss === null ? "–" : ms(stats.avgMiss)} />
                <Stat label="Unsafe reuses blocked" value={String(stats.refused)} tone="warn" wide />
              </dl>
              {mode === "sim" ? <p className="sb-muted sb-small">Costs use the Gemini list prices configured in the gateway.</p> : null}
            </div>

            <a className="tsl-btn tsl-btn-line sb-back" href={`${BASE_PATH}/#product`}>
              How the gateway decides
              <Icon name="arrow" />
            </a>
          </aside>
        </div>
      </section>
    </>
  );
}

function Stat({ label, value, tone, wide }: { label: string; value: string; tone?: "ok" | "warn"; wide?: boolean }) {
  return (
    <div className="tsl-metric" data-tone={tone} data-wide={wide ? "" : undefined}>
      <dt>
        <small>{label}</small>
      </dt>
      <dd>
        <b>{value}</b>
      </dd>
    </div>
  );
}

function EmptyState() {
  return <div className="sb-empty">Send a prompt or start the guided tour. The cache starts empty, so the first request always goes to the model.</div>;
}

function ResultCard({ item }: { item: HistoryItem }) {
  const v = verdict(item);
  return (
    <article className="sb-card sb-result">
      <div className="tsl-card-head">
        <span className="sb-tag" data-tone={v.tone}>
          {v.label}
        </span>
        <b>{item.mode === "sim" ? "Simulated" : "Live gateway"}</b>
      </div>
      <p className="sb-asked">
        <span>You asked</span> “{item.prompt}”
      </p>
      <dl className="tsl-metrics sb-metrics sb-metrics-4">
        <Stat label="Latency" value={ms(item.latencyMs)} />
        <Stat label="Cost" value={usd(item.costUsd)} />
        <Stat label="Saved" value={usd(item.savedUsd)} tone={item.savedUsd ? "ok" : undefined} />
        <Stat label="Tier" value={item.tier} />
      </dl>
      {item.mode === "sim" && item.matchedPrompt && item.tier !== "c1_exact" ? (
        <p className="sb-muted">
          Closest cached prompt: “{item.matchedPrompt}” (similarity {item.similarity.toFixed(2)})
        </p>
      ) : null}
      {item.guardrail ? (
        <div className="sb-guard">
          <b>Guardrail: {item.guardrail}</b>
          <p>
            The wording is close enough that a plain similarity cache would have returned the cached answer, which would have
            been wrong here. Sunborn sent it to the model instead.
          </p>
        </div>
      ) : null}
    </article>
  );
}
