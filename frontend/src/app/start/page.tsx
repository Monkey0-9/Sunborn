"use client";

import React, { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  AlertCircle,
  ArrowRight,
  CheckCircle2,
  KeyRound,
  Loader2,
  RefreshCw,
  ShieldCheck,
  XCircle,
} from "lucide-react";
import {
  explainDecision,
  plainHttpError,
  type PlainOutcome,
  type RawDecision,
} from "@/lib/plain-language";

type GatewayState = "checking" | "online" | "offline";
type KeyState = "none" | "checking" | "valid" | "invalid";

interface SessionInfo {
  tenant: string;
  application: string;
}

interface RequestResult {
  request_id: string;
  content: string;
  cached: boolean;
  duration_ms: number;
  decision: RawDecision;
  certificate?: unknown;
}

const DEFAULT_QUESTION =
  "Explain in two sentences why a cache must not reuse an answer to a question it cannot prove is the same.";

export default function StartHerePage() {
  const [gateway, setGateway] = useState<GatewayState>("checking");
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [keyDraft, setKeyDraft] = useState("");
  const [apiKey, setApiKey] = useState<string | null>(null);
  const [keyState, setKeyState] = useState<KeyState>("none");
  const [question, setQuestion] = useState(DEFAULT_QUESTION);
  const [model, setModel] = useState("gpt-4o");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<RequestResult | null>(null);
  const [outcome, setOutcome] = useState<PlainOutcome | null>(null);

  const checkConnection = useCallback(async () => {
    setGateway("checking");
    try {
      const res = await fetch("/api/proxy/health", {
        cache: "no-store",
        signal: AbortSignal.timeout(2500),
      });
      setGateway(res.ok ? "online" : "offline");
    } catch {
      setGateway("offline");
    }
  }, []);

  const checkSession = useCallback(async () => {
    try {
      const res = await fetch("/api/auth", { cache: "no-store" });
      if (!res.ok) return;
      const data = await res.json();
      if (data?.authenticated && data?.session?.tenant_id) {
        setSession({
          tenant: data.session.tenant_id,
          application: data.session.application_id || "",
        });
      }
    } catch {
      // The page states its own connection status; no fallback data is used.
    }
  }, []);

  useEffect(() => {
    checkConnection();
    checkSession();
  }, [checkConnection, checkSession]);

  const verifyKey = async () => {
    const candidate = keyDraft.trim();
    if (!candidate) return;
    setKeyState("checking");
    setError(null);
    try {
      const res = await fetch("/api/proxy/v1/auth/session", {
        cache: "no-store",
        headers: { "x-api-key": candidate },
      });
      if (!res.ok) {
        setKeyState("invalid");
        return;
      }
      const data = await res.json();
      setApiKey(candidate);
      setSession({
        tenant: data?.tenant_id || "",
        application: data?.application_id || "",
      });
      setKeyState("valid");
      setKeyDraft("");
    } catch {
      setKeyState("invalid");
    }
  };

  const ask = async () => {
    const prompt = question.trim();
    if (!prompt || busy) return;

    setBusy(true);
    setError(null);
    setResult(null);
    setOutcome(null);

    try {
      const headers: Record<string, string> = {
        "Content-Type": "application/json",
      };
      if (apiKey) {
        headers["x-api-key"] = apiKey;
      }

      const res = await fetch("/api/proxy/v1/requests", {
        method: "POST",
        cache: "no-store",
        headers,
        body: JSON.stringify({
          tenant_id: session?.tenant || "",
          application_id: session?.application || "",
          model,
          prompt,
          temperature: 0,
        }),
      });

      const body = await res.json().catch(() => null);

      if (!res.ok) {
        setError(plainHttpError(res.status, body));
        if (res.status === 401) {
          setGateway((prev) => (prev === "offline" ? prev : "online"));
        }
        return;
      }

      const data = body as RequestResult;
      setResult(data);
      setOutcome(explainDecision(data?.decision, { cached: data?.cached }));
    } catch {
      setError(
        "Could not reach Sunborn. Check that the gateway is running on 127.0.0.1:8080."
      );
      setGateway("offline");
    } finally {
      setBusy(false);
    }
  };

  const authenticated = Boolean(session);
  const canAsk = gateway === "online" && authenticated && !busy;

  return (
    <div className="mx-auto max-w-4xl space-y-14 pb-10">
      {/* Hero */}
      <section className="space-y-5 pt-2">
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-cyan-400">
          Start here · plain English
        </p>
        <h1 className="text-3xl font-bold leading-tight text-white sm:text-5xl">
          Sunborn, explained simply
        </h1>
        <p className="max-w-3xl text-lg leading-relaxed text-slate-300">
          Sunborn sits between an app and an AI model. When the same work has
          already been done, it can hand back the earlier answer instead of
          paying for it twice.
        </p>
        <p className="max-w-3xl text-lg leading-relaxed text-slate-300">
          It only does that when five checks all pass. If any check fails, it
          asks the model again. That refusal is the point: an answer that looks
          similar can still be the wrong answer.
        </p>
      </section>

      {/* Three ideas */}
      <section aria-labelledby="three-ideas" className="space-y-5">
        <h2 id="three-ideas" className="text-xl font-semibold text-white">
          Three things it does
        </h2>
        <div className="grid gap-4 sm:grid-cols-3">
          <SimpleCard
            title="Saves repeat work"
            body="An identical request can be answered from memory in microseconds instead of calling the model again."
          />
          <SimpleCard
            title="Refuses look-alikes"
            body="“Authorize the payment” and “Prohibit the payment” are close in wording and opposite in meaning. Sunborn does not treat them as the same."
          />
          <SimpleCard
            title="Shows its working"
            body="Every decision lists the five checks, in plain language, so you can see why an answer was reused or refused."
          />
        </div>
      </section>

      {/* The five checks */}
      <section aria-labelledby="five-checks" className="space-y-5">
        <h2 id="five-checks" className="text-xl font-semibold text-white">
          The five checks
        </h2>
        <p className="max-w-3xl text-base leading-relaxed text-slate-400">
          All five must say yes. One “not proven” is enough to send the request
          to the model. Sunborn never trades a failed check for a higher hit
          rate.
        </p>
        <ol className="space-y-3">
          {[
            ["1", "Is it really the same question?", "Same task, same numbers, same names, and no added or removed “not”."],
            ["2", "Is it the same person, account and permission?", "Tenant, application, user, conversation and model permission must match."],
            ["3", "Is the information still up to date?", "Sources, permissions, configuration and model versions must still be valid."],
            ["4", "Is it safe to skip or repeat a real action?", "A payment, email or database write must never be silently skipped or replayed."],
            ["5", "Can Sunborn prove the saved answer was not altered?", "A signed record must tie the answer to this request and verify cleanly."],
          ].map(([number, title, detail]) => (
            <li
              key={number}
              className="flex gap-4 rounded-2xl border border-white/10 bg-white/[0.02] p-4 sm:p-5"
            >
              <span
                aria-hidden="true"
                className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-cyan-500/30 bg-cyan-500/10 font-mono text-sm font-bold text-cyan-300"
              >
                {number}
              </span>
              <div className="space-y-1">
                <p className="text-base font-semibold text-white">{title}</p>
                <p className="text-sm leading-relaxed text-slate-400">{detail}</p>
              </div>
            </li>
          ))}
        </ol>
      </section>

      {/* Live try */}
      <section
        aria-labelledby="try-it"
        className="space-y-5 rounded-2xl border border-white/10 bg-white/[0.02] p-5 sm:p-7"
      >
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <h2 id="try-it" className="text-xl font-semibold text-white">
              Try one real question
            </h2>
            <p className="mt-1 max-w-2xl text-sm leading-relaxed text-slate-400">
              This sends one real request through Sunborn to the model you name.
              Nothing on this page is simulated: if Sunborn is offline or the
              key is wrong, the page says so and shows no invented answer.
            </p>
          </div>
          <ConnectionPill state={gateway} onRetry={checkConnection} />
        </div>

        {gateway === "offline" && (
          <div className="flex gap-3 rounded-xl border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-200">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
            <div className="space-y-1">
              <p className="font-semibold">Sunborn is not reachable.</p>
              <p className="text-amber-200/80">
                Start the gateway with{" "}
                <code className="rounded bg-black/30 px-1.5 py-0.5 font-mono text-xs">
                  SUNBORN_ENV=development cargo run -p sunborn-gateway
                </code>{" "}
                and then retry. This page will not show a fake result.
              </p>
            </div>
          </div>
        )}

        {gateway === "online" && !authenticated && (
          <div className="space-y-3 rounded-xl border border-white/10 bg-slate-950/60 p-4">
            <div className="flex gap-2 text-sm text-slate-300">
              <KeyRound className="mt-0.5 h-4 w-4 shrink-0 text-cyan-400" aria-hidden="true" />
              <p>
                Sunborn is running, but it will not answer without a key. Sign in{" "}
                <Link href="/login" className="text-cyan-300 underline underline-offset-4">
                  on the login page
                </Link>{" "}
                or paste a development key below. The key stays in this tab&apos;s
                memory and is never written to browser storage.
              </p>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row">
              <label htmlFor="plain-key" className="sr-only">
                Sunborn API key
              </label>
              <input
                id="plain-key"
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={keyDraft}
                onChange={(event) => setKeyDraft(event.target.value)}
                placeholder="Paste an API key"
                className="min-w-0 flex-1 rounded-xl border border-white/15 bg-slate-900 px-4 py-2.5 font-mono text-sm text-white placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-400"
              />
              <button
                type="button"
                onClick={verifyKey}
                disabled={!keyDraft.trim() || keyState === "checking"}
                className="rounded-xl border border-cyan-500/40 bg-cyan-500/15 px-4 py-2.5 text-sm font-semibold text-cyan-200 transition-colors hover:bg-cyan-500/25 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400"
              >
                {keyState === "checking" ? "Checking…" : "Use this key"}
              </button>
            </div>
            {keyState === "invalid" && (
              <p role="alert" className="text-sm text-rose-300">
                That key was not accepted. Check it and try again.
              </p>
            )}
            {keyState === "valid" && (
              <p className="text-sm text-emerald-300">
                Key accepted for tenant {session?.tenant || "unknown"}.
              </p>
            )}
          </div>
        )}

        <div className="space-y-2">
          <label htmlFor="plain-question" className="block text-sm font-semibold text-slate-200">
            Your question
          </label>
          <textarea
            id="plain-question"
            rows={3}
            value={question}
            onChange={(event) => setQuestion(event.target.value)}
            className="w-full resize-y rounded-xl border border-white/15 bg-slate-900 px-4 py-3 text-base leading-relaxed text-white placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-400"
          />
        </div>

        <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div className="space-y-2">
            <label htmlFor="plain-model" className="block text-sm font-semibold text-slate-200">
              Model
            </label>
            <input
              id="plain-model"
              value={model}
              onChange={(event) => setModel(event.target.value)}
              spellCheck={false}
              className="w-full rounded-xl border border-white/15 bg-slate-900 px-4 py-2.5 font-mono text-sm text-white focus:outline-none focus:ring-2 focus:ring-cyan-400 sm:w-64"
            />
          </div>
          <button
            type="button"
            onClick={ask}
            disabled={!canAsk || !question.trim()}
            className="inline-flex items-center justify-center gap-2 rounded-xl bg-cyan-400 px-6 py-3 text-base font-bold text-slate-950 transition-colors hover:bg-cyan-300 disabled:cursor-not-allowed disabled:bg-slate-700 disabled:text-slate-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950"
          >
            {busy ? (
              <>
                <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
                Asking…
              </>
            ) : (
              <>
                Ask Sunborn
                <ArrowRight className="h-5 w-5" aria-hidden="true" />
              </>
            )}
          </button>
        </div>

        {gateway === "online" && !authenticated && (
          <p className="text-xs text-slate-500">
            The button stays disabled until Sunborn knows who is asking.
          </p>
        )}

        <div aria-live="polite" className="space-y-4">
          {error && (
            <div
              role="alert"
              className="flex gap-3 rounded-xl border border-rose-500/30 bg-rose-500/10 p-4 text-sm text-rose-200"
            >
              <XCircle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
              <p>{error}</p>
            </div>
          )}

          {result && outcome && (
            <ResultPanel result={result} outcome={outcome} onAskAgain={ask} />
          )}
        </div>
      </section>

      {/* Honesty */}
      <section
        aria-labelledby="limits"
        className="space-y-3 rounded-2xl border border-white/10 bg-white/[0.02] p-5 sm:p-6"
      >
        <h2 id="limits" className="flex items-center gap-2 text-lg font-semibold text-white">
          <ShieldCheck className="h-5 w-5 text-cyan-400" aria-hidden="true" />
          What a green check does not mean
        </h2>
        <ul className="space-y-2 text-sm leading-relaxed text-slate-300">
          <li>
            It does not mean the answer is factually correct. The signed record
            proves it was not altered; it does not prove the model told the
            truth.
          </li>
          <li>
            It does not mean reuse is safe for every workload. The evidence
            applies to this request, this tenant and this configuration.
          </li>
          <li>
            It does not mean the project is production-certified. The
            repository&apos;s own status document lists two external items as
            still open.
          </li>
        </ul>
        <p className="text-sm text-slate-400">
          The full technical record lives in{" "}
          <Link href="/security" className="text-cyan-300 underline underline-offset-4">
            Security
          </Link>{" "}
          and{" "}
          <Link href="/governance" className="text-cyan-300 underline underline-offset-4">
            Governance
          </Link>
          . The release state is in{" "}
          <code className="rounded bg-black/30 px-1.5 py-0.5 font-mono text-xs">
            docs/STATUS.md
          </code>
          .
        </p>
      </section>
    </div>
  );
}

function SimpleCard({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-white/10 bg-white/[0.02] p-5">
      <p className="text-base font-semibold text-white">{title}</p>
      <p className="mt-2 text-sm leading-relaxed text-slate-400">{body}</p>
    </div>
  );
}

function ConnectionPill({
  state,
  onRetry,
}: {
  state: GatewayState;
  onRetry: () => void;
}) {
  if (state === "checking") {
    return (
      <span
        role="status"
        className="inline-flex shrink-0 items-center gap-2 rounded-full border border-white/15 bg-slate-900 px-3 py-1.5 font-mono text-xs text-slate-300"
      >
        <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
        Checking connection
      </span>
    );
  }
  if (state === "online") {
    return (
      <span
        role="status"
        className="inline-flex shrink-0 items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1.5 font-mono text-xs text-emerald-300"
      >
        <span className="h-2 w-2 rounded-full bg-emerald-400" aria-hidden="true" />
        Gateway reachable
      </span>
    );
  }
  return (
    <button
      type="button"
      onClick={onRetry}
      className="inline-flex shrink-0 items-center gap-2 rounded-full border border-amber-500/30 bg-amber-500/10 px-3 py-1.5 font-mono text-xs text-amber-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400"
    >
      <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
      Offline · retry
    </button>
  );
}

function ResultPanel({
  result,
  outcome,
  onAskAgain,
}: {
  result: RequestResult;
  outcome: PlainOutcome;
  onAskAgain: () => void;
}) {
  const reused = outcome.tone === "reused";
  const panelTone = reused
    ? "border-emerald-500/40 bg-emerald-500/5"
    : "border-sky-500/30 bg-sky-500/5";

  return (
    <div className={`space-y-5 rounded-2xl border p-5 ${panelTone}`}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-1.5">
          <p
            className={`inline-flex items-center gap-2 text-lg font-bold ${
              reused ? "text-emerald-300" : "text-sky-300"
            }`}
          >
            {reused ? (
              <CheckCircle2 className="h-5 w-5" aria-hidden="true" />
            ) : (
              <RefreshCw className="h-5 w-5" aria-hidden="true" />
            )}
            {outcome.headline}
          </p>
          <p className="max-w-2xl text-sm leading-relaxed text-slate-300">
            {outcome.summary}
          </p>
        </div>
        <div className="shrink-0 rounded-xl border border-white/10 bg-black/20 px-3 py-2 font-mono text-[11px] leading-relaxed text-slate-400">
          <div>took {result.duration_ms} ms</div>
          <div>path {result.decision?.cache_path || "unknown"}</div>
          <div>
            signed record: {result.certificate ? "returned" : "not returned"}
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-sm font-semibold text-white">
          {outcome.why.length === 1 ? "Reason" : "Reasons"}
        </p>
        <ul className="space-y-1.5">
          {outcome.why.map((reason) => (
            <li key={reason} className="flex gap-2 text-sm leading-relaxed text-slate-300">
              <span aria-hidden="true" className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-slate-500" />
              <span>{reason}</span>
            </li>
          ))}
        </ul>
      </div>

      <div className="space-y-2">
        <p className="text-sm font-semibold text-white">
          The five checks for this request
        </p>
        {!outcome.evidenceReported && (
          <p className="text-sm text-amber-300">
            This gateway did not return all five check results. Do not read this
            as a clean safety record.
          </p>
        )}
        <ul className="space-y-2">
          {outcome.gates.map((gate) => (
            <li
              key={gate.id}
              className="flex gap-3 rounded-xl border border-white/10 bg-black/10 p-3"
            >
              {gate.passed ? (
                <CheckCircle2
                  className="mt-0.5 h-4 w-4 shrink-0 text-emerald-400"
                  aria-label="passed"
                />
              ) : (
                <XCircle
                  className="mt-0.5 h-4 w-4 shrink-0 text-amber-400"
                  aria-label="did not pass"
                />
              )}
              <div className="min-w-0 space-y-0.5">
                <p className="text-sm font-medium text-white">
                  <span className="font-mono text-[11px] text-slate-500">{gate.id}</span>{" "}
                  {gate.question}
                </p>
                <p className="text-xs leading-relaxed text-slate-400">
                  <span className={gate.passed ? "text-emerald-300" : "text-amber-300"}>
                    {gate.statusLabel}.
                  </span>{" "}
                  {gate.explanation}
                </p>
              </div>
            </li>
          ))}
        </ul>
      </div>

      <div className="space-y-2">
        <p className="text-sm font-semibold text-white">The answer</p>
        <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-white/10 bg-slate-950/70 p-4 font-sans text-sm leading-relaxed text-slate-200">
          {result.content}
        </pre>
      </div>

      <div className="flex flex-col gap-2 text-xs text-slate-500 sm:flex-row sm:items-center sm:justify-between">
        <span className="font-mono">
          request {result.request_id?.slice(0, 8) || "unknown"}
        </span>
        <button
          type="button"
          onClick={onAskAgain}
          className="self-start rounded-lg border border-white/15 px-3 py-1.5 text-xs font-semibold text-slate-200 hover:bg-white/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400"
        >
          Ask the same question again
        </button>
      </div>

      <p className="text-xs leading-relaxed text-slate-500">
        A signed record proves this answer was not altered in storage. It does
        not prove the answer is factually correct.
      </p>
    </div>
  );
}
