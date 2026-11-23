"use client";

import * as React from "react";
import { Icon } from "@/components/tessel/tessel";

// The four cards on the "How it works" stage. Each is small, interactive and
// only claims what the gateway does; timings marked "~" are illustrative.

function Seg<T extends string>({ label, options, value, onChange }: { label: string; options: readonly T[]; value: T; onChange: (v: T) => void }) {
  return (
    <div className="tsl-seg" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o} type="button" role="radio" aria-checked={value === o} onClick={() => onChange(o)}>
          {o}
        </button>
      ))}
    </div>
  );
}

type TierState = "hit" | "miss" | "pass" | "refused" | "called" | "skipped";

const PIPELINE = {
  Repeat: {
    cached: "What is the capital of France?",
    asked: "what is the capital of france",
    tiers: ["hit", "skipped", "skipped", "skipped"] as TierState[],
    result: "Exact hit · served in well under a millisecond",
  },
  Paraphrase: {
    cached: "What is the capital of France?",
    asked: "Tell me France's capital city",
    tiers: ["miss", "hit", "pass", "skipped"] as TierState[],
    result: "Semantic hit · the paraphrase reuses the answer",
  },
  Trap: {
    cached: "Convert a 5 km run into miles",
    asked: "Convert a 10 km run into miles",
    tiers: ["miss", "hit", "refused", "called"] as TierState[],
    result: "Refused · the number changed, so the model answers",
  },
} as const;

const TIERS = [
  ["Exact match", "memory + Dragonfly"],
  ["Semantic match", "Qdrant nearest prompt"],
  ["Guardrails + risk", "numbers, negation, entities"],
  ["Origin model", "OpenAI, Claude or Gemini"],
];

const STATE_LABEL: Record<TierState, string> = {
  hit: "match",
  miss: "no match",
  pass: "passed",
  refused: "refused",
  called: "called",
  skipped: "skipped",
};

export function PipelineCard() {
  const [ex, setEx] = React.useState<keyof typeof PIPELINE>("Paraphrase");
  const d = PIPELINE[ex];
  return (
    <div>
      <div className="tsl-card-head">
        <b>Request path</b>
        <Seg label="Example request" options={["Repeat", "Paraphrase", "Trap"] as const} value={ex} onChange={setEx} />
      </div>
      <div className="sb-pair">
        <span>
          <small>Cached</small>“{d.cached}”
        </span>
        <span>
          <small>Asked</small>“{d.asked}”
        </span>
      </div>
      <ol className="sb-tiers">
        {TIERS.map(([name, sub], i) => (
          <li key={name} data-state={d.tiers[i]}>
            <span className="sb-tier-n">{i + 1}</span>
            <span>
              {name}
              <small>{sub}</small>
            </span>
            <em>{STATE_LABEL[d.tiers[i]]}</em>
          </li>
        ))}
      </ol>
      <div className="tsl-card-foot">
        <span aria-live="polite">{d.result}</span>
      </div>
    </div>
  );
}

const TRAPS = {
  Numbers: { cached: ["Convert a ", "5", " km run into miles"], asked: ["Convert a ", "10", " km run into miles"], check: 0 },
  Negation: { cached: ["Which foods should I", "", " eat before a long run?"], asked: ["Which foods should I", " not", " eat before a long run?"], check: 1 },
  Direction: { cached: ["Stretches to do ", "before", " running"], asked: ["Stretches to do ", "after", " running"], check: 2 },
} as const;

const CHECKS = [
  ["Numbers and units", "5 km is not 10 km"],
  ["Negation", "eat vs. don't eat"],
  ["Direction words", "before vs. after, buy vs. sell"],
  ["Entities and dates", "names and years must match"],
];

export function GuardrailCard() {
  const [ex, setEx] = React.useState<keyof typeof TRAPS>("Numbers");
  const t = TRAPS[ex];
  return (
    <div>
      <div className="tsl-card-head">
        <b>Guardrails</b>
        <Seg label="Trap" options={["Numbers", "Negation", "Direction"] as const} value={ex} onChange={setEx} />
      </div>
      <div className="tsl-field">
        <span>Cached prompt</span>
        <div className="tsl-field-box sb-diff">
          {t.cached[0]}
          {t.cached[1] ? <mark data-side="old">{t.cached[1]}</mark> : null}
          {t.cached[2]}
        </div>
      </div>
      <div className="tsl-field">
        <span>New prompt, nearly the same words</span>
        <div className="tsl-field-box sb-diff">
          {t.asked[0]}
          <mark data-side="new">{t.asked[1]}</mark>
          {t.asked[2]}
        </div>
      </div>
      <ul className="sb-checks">
        {CHECKS.map(([name, hint], i) => (
          <li key={name} data-fail={i === t.check ? "" : undefined}>
            <span aria-hidden="true">{i === t.check ? <Icon name="x" size={12} stroke={2} /> : <Icon name="check" size={12} stroke={2} />}</span>
            {name}
            <small>{hint}</small>
          </li>
        ))}
      </ul>
      <div className="tsl-card-foot">
        <span aria-live="polite">A similarity-only cache would reuse this. Sunborn asks the model.</span>
      </div>
    </div>
  );
}

const SPANS = {
  Hit: [
    { name: "normalize", from: 0, to: 0.08, kind: "tool", t: "~0.01 ms" },
    { name: "c1.exact", from: 0.06, to: 0.22, kind: "tool", t: "~0.3 ms" },
    { name: "c2.qdrant", from: 0.2, to: 0.72, kind: "llm", t: "~1.5 ms" },
    { name: "guardrails", from: 0.7, to: 0.9, kind: "tool", t: "~0.2 ms" },
    { name: "serve", from: 0.88, to: 1, kind: "out", t: "~0.1 ms" },
  ],
  Miss: [
    { name: "normalize", from: 0, to: 0.01, kind: "tool", t: "~0.01 ms" },
    { name: "c1.exact", from: 0.005, to: 0.02, kind: "tool", t: "~0.3 ms" },
    { name: "c2.qdrant", from: 0.015, to: 0.04, kind: "llm", t: "~1.5 ms" },
    { name: "origin.call", from: 0.04, to: 0.97, kind: "llm", t: "~900 ms" },
    { name: "cache.store", from: 0.96, to: 1, kind: "out", t: "~2 ms" },
  ],
} as const;

export function TraceCard({ active }: { active: boolean }) {
  const [kind, setKind] = React.useState<keyof typeof SPANS>("Hit");
  const [play, setPlay] = React.useState(false);
  const [focus, setFocus] = React.useState(-1);
  React.useEffect(() => {
    if (!active) return setPlay(false);
    setPlay(false);
    const id = window.setTimeout(() => setPlay(true), 60);
    return () => clearTimeout(id);
  }, [active, kind]);
  const spans = SPANS[kind];
  const s = focus > -1 ? spans[focus] : null;
  return (
    <div>
      <div className="tsl-card-head">
        <b>Request trace</b>
        <Seg label="Outcome" options={["Hit", "Miss"] as const} value={kind} onChange={setKind} />
      </div>
      <div className="tsl-wf" data-play={play ? "" : undefined}>
        {spans.map((sp, i) => (
          <div
            key={sp.name}
            className="tsl-wf-row"
            tabIndex={0}
            onPointerEnter={() => setFocus(i)}
            onPointerLeave={() => setFocus(-1)}
            onFocus={() => setFocus(i)}
            onBlur={() => setFocus(-1)}
          >
            <span>{sp.name}</span>
            <span className="tsl-wf-track">
              <span className="tsl-wf-bar" data-kind={sp.kind} style={{ left: sp.from * 100 + "%", width: (sp.to - sp.from) * 100 + "%", transitionDelay: i * 0.12 + "s" }} />
            </span>
          </div>
        ))}
      </div>
      <div className="tsl-wf-axis" aria-hidden="true">
        <span>0</span>
        <span>{kind === "Hit" ? "~1 ms" : "~450 ms"}</span>
        <span>{kind === "Hit" ? "~2 ms" : "~900 ms"}</span>
      </div>
      <div className="tsl-card-foot">
        <span aria-live="polite">{s ? `${s.name} · ${s.t}` : "Hover a span · timings illustrative"}</span>
        <button
          type="button"
          className="tsl-mini"
          onClick={() => {
            setPlay(false);
            window.setTimeout(() => setPlay(true), 80);
          }}
        >
          <Icon name="replay" size={12} />
          Replay
        </button>
      </div>
    </div>
  );
}

const RUN = {
  Compose: {
    cmd: ['echo "GEMINI_API_KEY=your-key" >> deploy/.env', "docker compose -f deploy/docker-compose.yml up -d --build"],
    note: "One machine. The gateway listens on localhost:8080.",
  },
  Helm: {
    cmd: ["helm install sunborn deploy/helm/sunborn"],
    note: "A cluster. Values for replicas, secrets and storage are in the chart.",
  },
} as const;

const SERVICES = [
  ["gateway", "Rust · :8080"],
  ["postgres", "metadata · :5432"],
  ["dragonfly", "exact tier · :6379"],
  ["qdrant", "semantic tier · :6333"],
];

export function RunCard() {
  const [how, setHow] = React.useState<keyof typeof RUN>("Compose");
  const r = RUN[how];
  return (
    <div>
      <div className="tsl-card-head">
        <b>Run it</b>
        <Seg label="Deploy with" options={["Compose", "Helm"] as const} value={how} onChange={setHow} />
      </div>
      <pre className="sb-term">
        {r.cmd.map((c) => (
          <code key={c}>
            <i aria-hidden="true">$ </i>
            {c}
          </code>
        ))}
      </pre>
      <ul className="sb-services">
        {SERVICES.map(([n, d]) => (
          <li key={n}>
            <i aria-hidden="true" />
            <b>{n}</b>
            <small>{d}</small>
          </li>
        ))}
      </ul>
      <div className="tsl-card-foot">
        <span aria-live="polite">{r.note}</span>
      </div>
    </div>
  );
}

export function StageCard({ visual, active }: { visual: string; active: boolean }) {
  if (visual === "pipeline") return <PipelineCard />;
  if (visual === "guardrails") return <GuardrailCard />;
  if (visual === "trace") return <TraceCard active={active} />;
  return <RunCard />;
}
