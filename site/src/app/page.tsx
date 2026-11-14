import Link from "next/link";
import {
  ArrowRight,
  Boxes,
  GitBranch,
  Layers,
  Network,
  Repeat,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Zap,
} from "lucide-react";
import { REPO_URL, blob } from "@/lib/site";

const PIPELINE = [
  {
    icon: Zap,
    title: "Exact match",
    body: "The prompt is normalised and looked up in memory and Dragonfly. Identical questions come back in well under a millisecond.",
    tone: "text-cyan",
  },
  {
    icon: Layers,
    title: "Semantic match",
    body: "If there is no exact match, Qdrant finds earlier prompts that mean nearly the same thing, so paraphrases can reuse an answer.",
    tone: "text-violet-glow",
  },
  {
    icon: ShieldAlert,
    title: "Guardrails",
    body: "Before reusing, Sunborn checks for changes that flip the meaning: numbers, negations, entities, units and dates.",
    tone: "text-amber-glow",
  },
  {
    icon: ShieldCheck,
    title: "Risk control",
    body: "A conformal risk controller only allows reuse when the expected error rate stays under a set bound (α ≤ 0.001 by default).",
    tone: "text-emerald-glow",
  },
];

const FEATURES = [
  {
    icon: Repeat,
    title: "Drop-in API",
    body: "OpenAI-compatible /v1/chat/completions and Anthropic-compatible /v1/messages. Point your SDK at Sunborn and keep your code.",
  },
  {
    icon: Network,
    title: "Multi-model routing",
    body: "Routes between OpenAI, Anthropic Claude and Google Gemini, choosing by cost and availability.",
  },
  {
    icon: GitBranch,
    title: "Dependency-aware invalidation",
    body: "A dependency graph links cached answers to their source documents, so updating a source evicts exactly the answers built on it.",
  },
  {
    icon: Trash2,
    title: "Tenant isolation",
    body: "Each tenant's cache is separate, and per-tenant encryption keys allow cryptographic erasure on request.",
  },
  {
    icon: Boxes,
    title: "Rust core",
    body: "An async Rust gateway backed by Postgres, Dragonfly and Qdrant, with Docker Compose and Helm deployments included.",
  },
];

const BENCH = [
  { name: "Exact match (SHA-256)", reused: 0, falseHit: "0.0%", note: "Safe, but never reuses a paraphrase" },
  { name: "Naive cosine similarity ≥ 0.85", reused: 34, falseHit: "73.9%", note: "Reused 34 answers, all of them wrong", bad: true },
  { name: "Structural guardrails only", reused: 3, falseHit: "4.3%", note: "Catches most traps" },
  { name: "Complete Sunborn", reused: 0, falseHit: "0.0%", note: "Refused every trap", good: true },
];

const CURL = `curl http://localhost:8080/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "x-api-key: $SUNBORN_API_KEY" \\
  -d '{"model":"gemini-3.5-flash-lite",
       "messages":[{"role":"user","content":"What is the capital of France?"}]}'

# Response includes:  "sunborn_cached": true, "sunborn_tier": "c2_semantic"`;

export default function Home() {
  return (
    <div className="space-y-28">
      {/* Hero */}
      <section className="pt-20 text-center sm:pt-28">
        <p className="mx-auto mb-6 inline-flex items-center gap-2 rounded-full border border-cyan-500/30 bg-cyan-950/40 px-3 py-1 text-xs font-medium text-cyan-300">
          Open-source semantic cache for LLM APIs
        </p>
        <h1 className="mx-auto max-w-4xl text-4xl font-semibold leading-[1.1] tracking-tight text-white sm:text-6xl">
          Stop paying for the same answer twice.
        </h1>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-relaxed text-slate-400">
          Sunborn sits between your app and the model. It answers repeated and reworded prompts from cache, and it
          refuses to reuse an answer when a small change in wording changes the meaning.
        </p>
        <div className="mt-10 flex flex-col items-center justify-center gap-3 sm:flex-row">
          <Link
            href="/playground/"
            className="inline-flex items-center gap-2 rounded-xl bg-cyan px-5 py-3 font-semibold text-slate-950 shadow-glow transition hover:brightness-110"
          >
            Try the playground <ArrowRight className="h-4 w-4" aria-hidden />
          </Link>
          <a
            href={REPO_URL}
            className="inline-flex items-center gap-2 rounded-xl border border-white/15 px-5 py-3 font-medium text-slate-200 transition hover:border-white/30 hover:text-white"
          >
            View the source
          </a>
        </div>
      </section>

      {/* Problem */}
      <section className="grid gap-4 md:grid-cols-3">
        <div className="panel p-6">
          <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-500">The cost</h2>
          <p className="mt-3 leading-relaxed text-slate-300">
            Real traffic repeats itself. Users ask the same things in different words, and agents re-run the same
            steps. Every one of those is a full model call you pay for and wait on.
          </p>
        </div>
        <div className="panel p-6">
          <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-500">Plain caches miss</h2>
          <p className="mt-3 leading-relaxed text-slate-300">
            A key-value cache only matches identical strings. Change the punctuation or rephrase the question and it
            goes back to the model.
          </p>
        </div>
        <div className="panel p-6">
          <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-500">Similarity caches lie</h2>
          <p className="mt-3 leading-relaxed text-slate-300">
            Matching by embedding similarity alone reuses answers for prompts that look alike but mean something
            else, like <span className="text-amber-glow">5 km</span> vs <span className="text-amber-glow">10 km</span>,
            or <span className="text-amber-glow">eat</span> vs <span className="text-amber-glow">don&apos;t eat</span>.
          </p>
        </div>
      </section>

      {/* Pipeline */}
      <section>
        <h2 className="text-3xl font-semibold tracking-tight text-white">How a request flows</h2>
        <p className="mt-3 max-w-2xl text-slate-400">
          Each request walks down the tiers and stops at the first one that can answer it safely. Anything that fails a
          check goes to the model, and the new answer is cached for next time.
        </p>
        <ol className="mt-10 grid gap-4 md:grid-cols-4">
          {PIPELINE.map((step, i) => (
            <li key={step.title} className="panel relative p-6">
              <div className="flex items-center gap-3">
                <span className="flex h-7 w-7 items-center justify-center rounded-full border border-white/15 text-xs text-slate-400">
                  {i + 1}
                </span>
                <step.icon className={`h-5 w-5 ${step.tone}`} aria-hidden />
              </div>
              <h3 className="mt-4 font-semibold text-white">{step.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-slate-400">{step.body}</p>
            </li>
          ))}
        </ol>
        <p className="mt-4 text-sm text-slate-500">
          No safe match at any tier? The request goes to the model (OpenAI, Claude or Gemini) and the answer is stored.
        </p>
      </section>

      {/* Benchmark */}
      <section>
        <h2 className="text-3xl font-semibold tracking-tight text-white">Tested against adversarial traps</h2>
        <p className="mt-3 max-w-2xl text-slate-400">
          A held-out set of prompt pairs that look alike but should get different answers. A good cache should reuse
          none of them. This is a safety test, not a hit-rate test.
        </p>
        <div className="panel mt-8 overflow-x-auto">
          <table className="w-full min-w-[560px] text-left text-sm">
            <thead className="border-b border-white/[0.08] text-xs uppercase tracking-wider text-slate-500">
              <tr>
                <th className="px-6 py-4 font-medium">Strategy</th>
                <th className="px-6 py-4 font-medium">Wrong reuses</th>
                <th className="px-6 py-4 font-medium">False-hit rate</th>
                <th className="px-6 py-4 font-medium">Takeaway</th>
              </tr>
            </thead>
            <tbody>
              {BENCH.map((row) => (
                <tr key={row.name} className="border-b border-white/[0.04] last:border-0">
                  <td className={`px-6 py-4 font-medium ${row.good ? "text-emerald-glow" : "text-slate-200"}`}>{row.name}</td>
                  <td className="px-6 py-4 tabular-nums text-slate-300">{row.reused}</td>
                  <td className={`px-6 py-4 tabular-nums ${row.bad ? "font-semibold text-rose-glow" : row.good ? "font-semibold text-emerald-glow" : "text-slate-300"}`}>
                    {row.falseHit}
                  </td>
                  <td className="px-6 py-4 text-slate-400">{row.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-3 text-sm text-slate-500">
          Source:{" "}
          <a className="underline decoration-slate-600 underline-offset-2 hover:text-slate-300" href={blob("research/results/heldout_generalization_run.json")}>
            research/results/heldout_generalization_run.json
          </a>
          . Reproduce with <code className="text-slate-400">research/reproduce.sh</code>.
        </p>
      </section>

      {/* Features */}
      <section>
        <h2 className="text-3xl font-semibold tracking-tight text-white">What else is in the box</h2>
        <div className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((f) => (
            <div key={f.title} className="panel p-6">
              <f.icon className="h-5 w-5 text-cyan" aria-hidden />
              <h3 className="mt-4 font-semibold text-white">{f.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-slate-400">{f.body}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Run it */}
      <section className="grid items-start gap-8 lg:grid-cols-2">
        <div>
          <h2 className="text-3xl font-semibold tracking-tight text-white">Run it yourself</h2>
          <p className="mt-3 text-slate-400">
            The whole stack runs on one machine with Docker. Put a provider key in <code className="text-slate-300">deploy/.env</code>,
            start it, and point any OpenAI-compatible client at port 8080.
          </p>
          <pre className="panel mt-6 overflow-x-auto p-5 text-sm leading-relaxed text-slate-300">
            <code>{`echo "GEMINI_API_KEY=your-key" >> deploy/.env
docker compose -f deploy/docker-compose.yml up -d --build`}</code>
          </pre>
          <p className="mt-4 text-sm text-slate-500">
            Full steps in{" "}
            <a className="underline decoration-slate-600 underline-offset-2 hover:text-slate-300" href={blob("docs/QUICKSTART.md")}>
              docs/QUICKSTART.md
            </a>
            . The playground can then talk to your local gateway in Live mode.
          </p>
        </div>
        <pre className="panel overflow-x-auto p-5 text-[13px] leading-relaxed text-slate-300">
          <code>{CURL}</code>
        </pre>
      </section>

      {/* CTA */}
      <section className="panel flex flex-col items-center gap-6 px-6 py-14 text-center">
        <h2 className="text-2xl font-semibold text-white sm:text-3xl">See the cache decide, one prompt at a time.</h2>
        <Link
          href="/playground/"
          className="inline-flex items-center gap-2 rounded-xl bg-cyan px-5 py-3 font-semibold text-slate-950 shadow-glow transition hover:brightness-110"
        >
          Open the playground <ArrowRight className="h-4 w-4" aria-hidden />
        </Link>
      </section>
    </div>
  );
}
