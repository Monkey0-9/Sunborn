"use client";

import * as React from "react";
import { Cta, Hero, LogoStrip, Problem, Section, Solution, Stats, type TslPlanner } from "@/components/tessel/tessel";
import { SunbornShell } from "@/components/sunborn/shell";
import { StageCard } from "@/components/sunborn/stage-cards";
import { createSimulatedCache } from "@/lib/sim-cache.mjs";
import { simulatedAnswer } from "@/lib/scenarios";
import { simTrace } from "@/lib/trace";
import { BASE_PATH, REPO_URL, blob } from "@/lib/site";

const BENCH = [
  { name: "Exact match (SHA-256)", reused: 0, falseHit: "0.0%", note: "Safe, but never reuses a paraphrase" },
  { name: "Naive cosine similarity ≥ 0.85", reused: 34, falseHit: "73.9%", note: "Reused 34 answers, all of them wrong", tone: "bad" },
  { name: "Structural guardrails only", reused: 3, falseHit: "4.3%", note: "Catches most traps" },
  { name: "Complete Sunborn", reused: 0, falseHit: "0.0%", note: "Refused every trap", tone: "good" },
];

const CURL = `curl http://localhost:8080/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "x-api-key: $SUNBORN_API_KEY" \\
  -d '{"model":"gemini-3.5-flash-lite",
       "messages":[{"role":"user","content":"What is the capital of France?"}]}'

# The response carries:
#   "sunborn_cached": true, "sunborn_tier": "c2_semantic"`;

export function Landing() {
  const playground = `${BASE_PATH}/playground/`;
  // One in-browser cache for the hero, so a second question can hit the first.
  const sim = React.useMemo(() => createSimulatedCache({ answerFor: simulatedAnswer }), []);
  const planner: TslPlanner = React.useCallback((prompt: string) => simTrace(prompt, sim.query(prompt)), [sim]);

  return (
    <SunbornShell page="home">
      <Hero
        planner={planner}
        data={{
          badge: { mark: "S", label: "Open-source semantic cache for LLM APIs" },
          title: "Stop Paying for the\nSame Answer Twice",
          text: "Sunborn sits between your app and the model. It answers repeated and reworded prompts from cache, and refuses to reuse an answer when a small change in wording changes the meaning.",
          primary: { label: "Open the playground", href: playground },
          secondary: { label: "View the source", href: REPO_URL },
          prompt: {
            label: "Sunborn gateway, simulated in your browser",
            placeholder: "Ask something, then ask it again in other words",
            suggestions: [
              "What is the capital of France?",
              "Tell me France's capital city",
              "What was the capital of France in 1900?",
              "Convert a 5 km run into miles",
              "Convert a 10 km run into miles",
            ],
            models: ["Simulated"],
            modelHints: ["Runs in this tab, no key"],
            footer: (
              <a className="tsl-mini" href={playground}>
                Full playground
              </a>
            ),
          },
          ruler: ["SUNBORN / GATEWAY", "EXACT · SEMANTIC · GUARDED"],
        }}
      />

      <LogoStrip
        label="Speaks these APIs and runs on"
        logos={[
          { name: "OpenAI API", mark: "orbit", style: "bold" },
          { name: "Anthropic API", mark: "spark", style: "light" },
          { name: "Gemini", mark: "leaf", style: "italic" },
          { name: "Rust", mark: "bolt", style: "bold" },
          { name: "POSTGRES", mark: "bars", style: "caps" },
          { name: "Dragonfly", mark: "chevron", style: "light" },
          { name: "Qdrant", mark: "knot", style: "bold" },
          { name: "HELM", mark: "orbit", style: "caps" },
        ]}
      />

      <Problem
        data={{
          kicker: "The problem",
          title: "Real Traffic Repeats Itself, *and Most Caches Either Miss It or Get It Wrong.*",
          items: [
            {
              icon: "steps",
              title: "Every Repeat Costs Full Price",
              text: "Users ask the same things in different words, and agents re-run the same steps. Each one is a full model call you pay for and wait on.",
            },
            {
              icon: "layers",
              title: "Plain Caches Miss",
              text: "A key-value cache only matches identical strings. Change the punctuation or rephrase the question and it goes back to the model.",
            },
            {
              icon: "eye",
              title: "Similarity Caches Lie",
              text: "Matching on embedding similarity alone reuses answers for prompts that look alike but mean something else, like 5 km vs 10 km, or eat vs don't eat.",
            },
          ],
        }}
      />

      <Solution
        data={{
          kicker: "How it works",
          title: "Similarity Proposes a Candidate. *Independent Checks Decide Whether It May Be Served.*",
          features: [
            {
              kicker: "Request pipeline",
              visual: "pipeline",
              title: "Each request stops at the first tier that can answer it safely.",
              text: "The prompt is normalised and looked up in memory and Dragonfly. If there is no exact match, Qdrant finds earlier prompts that mean nearly the same thing. Anything that fails a check goes to the model, and the new answer is cached.",
              points: ["Exact tier answers in well under a millisecond", "Semantic tier reuses paraphrases", "OpenAI-compatible and Anthropic-compatible endpoints"],
            },
            {
              kicker: "Guardrails",
              visual: "guardrails",
              title: "Refuse reuse when a small change flips the meaning.",
              text: "Before reusing, Sunborn checks for changes that matter: numbers, negations, direction words, entities, units and dates. A conformal risk controller only allows reuse while the expected error rate stays under a set bound.",
              points: ["Numbers, units and dates must match", "Negation and direction flips are refused", "Risk bound set per deployment"],
            },
            {
              kicker: "Request trace",
              visual: "trace",
              title: "See where every millisecond goes.",
              text: "A hit is answered by the gateway in about a millisecond. A miss pays the model's latency once, then the answer is stored so the next paraphrase does not.",
              points: ["Tier and cache status on every response", "Per-tenant caches with separate keys", "Dependency-aware invalidation when a source changes"],
            },
            {
              kicker: "Run it",
              visual: "deploy",
              title: "One machine with Docker, or a cluster with Helm.",
              text: "An async Rust gateway backed by Postgres, Dragonfly and Qdrant. Put a provider key in deploy/.env, start the stack and point any OpenAI-compatible client at port 8080.",
              points: ["Docker Compose for local runs", "Helm chart for Kubernetes", "Routes across OpenAI, Claude and Gemini"],
            },
          ],
        }}
        renderStage={(visual, active) => <StageCard visual={visual} active={active} />}
      />

      <Stats
        label="Measured results"
        stats={[
          { value: 0, label: "False hits on 200 adversarial hard negatives" },
          { value: 52, suffix: "%", label: "Wrong reuses by a naive cosine cache on the same workload" },
          { value: 0.07, suffix: " ms", decimals: 2, label: "Gateway P50, release build, 5,000 sequential requests" },
          { value: 13241, suffix: "/s", label: "Requests per second through the gateway stack, sequential" },
        ]}
      />

      <Section
        id="evidence"
        kicker="Evidence"
        title="Tested Against Prompts Built to Fool a Cache. *A Good Cache Reuses None of Them.*"
        lead="A held-out set of prompt pairs that look alike but should get different answers. This is a safety test, not a hit-rate test."
      >
        <div className="sb-table-wrap" data-reveal>
          <table className="sb-table">
            <thead>
              <tr>
                <th scope="col">Strategy</th>
                <th scope="col">Wrong reuses</th>
                <th scope="col">False-hit rate</th>
                <th scope="col">Takeaway</th>
              </tr>
            </thead>
            <tbody>
              {BENCH.map((row) => (
                <tr key={row.name} data-tone={row.tone}>
                  <th scope="row">{row.name}</th>
                  <td>{row.reused}</td>
                  <td className="sb-rate">{row.falseHit}</td>
                  <td>{row.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="sb-source" data-reveal>
          Source:{" "}
          <a className="tsl-link" href={blob("research/results/heldout_generalization_run.json")}>
            research/results/heldout_generalization_run.json
          </a>
          . Reproduce with <code>research/reproduce.sh</code>. Results and limits are written up in the{" "}
          <a className="tsl-link" href={blob("README.md")}>
            README
          </a>
          .
        </p>
      </Section>

      <Section
        id="run"
        kicker="Quickstart"
        title="Run It Yourself in *Two Commands.*"
        lead={
          <>
            Put a provider key in <code>deploy/.env</code>, start the stack, and point any OpenAI-compatible client at port
            8080. The playground can then talk to your local gateway in Live mode.
          </>
        }
      >
        <div className="sb-code-grid">
          <figure className="sb-code" data-reveal>
            <figcaption>1 · Start the stack</figcaption>
            <pre>
              <code>{`echo "GEMINI_API_KEY=your-key" >> deploy/.env
docker compose -f deploy/docker-compose.yml up -d --build`}</code>
            </pre>
          </figure>
          <figure className="sb-code" data-reveal>
            <figcaption>2 · Send a request</figcaption>
            <pre>
              <code>{CURL}</code>
            </pre>
          </figure>
        </div>
        <p className="sb-source" data-reveal>
          Full steps in{" "}
          <a className="tsl-link" href={blob("docs/QUICKSTART.md")}>
            docs/QUICKSTART.md
          </a>
          .
        </p>
      </Section>

      <Cta
        title="See the cache decide, one prompt at a time."
        text="The playground runs in your browser with no key. Switch to Live mode to send the same prompts through a gateway you run."
        actions={[
          { label: "Open the playground", href: playground },
          { label: "Read the quickstart", href: blob("docs/QUICKSTART.md") },
        ]}
      />
    </SunbornShell>
  );
}
