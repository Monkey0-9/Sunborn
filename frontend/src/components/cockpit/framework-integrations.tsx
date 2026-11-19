"use client";

import React, { useState } from "react";
import { Code2, Copy, Check, Terminal, Layers, Sparkles } from "lucide-react";

type FrameworkTab = "html" | "react" | "vue" | "angular" | "typescript" | "curl";

interface FrameworkSnippet {
  id: FrameworkTab;
  name: string;
  badge: string;
  badgeColor: string;
  filename: string;
  code: string;
  description: string;
}

const SNIPPETS: Record<FrameworkTab, FrameworkSnippet> = {
  html: {
    id: "html",
    name: "HTML / Vanilla JS",
    badge: "Native Fetch",
    badgeColor: "bg-amber-500/10 text-amber-300 border-amber-500/20",
    filename: "app.js",
    description: "Zero external dependencies. Works directly in modern browsers via native fetch.",
    code: `// Drop-in OpenAI wire-compatible ingress
async function askSunborn(prompt) {
  const response = await fetch("http://localhost:8080/v1/chat/completions", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": "sb_live_production_key"
    },
    body: JSON.stringify({
      model: "gpt-4o",
      messages: [{ role: "user", content: prompt }],
      temperature: 0.0
    })
  });

  const data = await response.json();
  console.log("Response:", data.choices[0].message.content);
  console.log("Cache Tier:", response.headers.get("x-sunborn-tier")); // "exact" | "semantic"
  console.log("Avoided Cost:", response.headers.get("x-sunborn-avoided-usd"));
  return data;
}

// Sub-microsecond exact repeat
askSunborn("Summarize Q3 operating margins.");`,
  },
  react: {
    id: "react",
    name: "React / Next.js",
    badge: "React 19 / Hooks",
    badgeColor: "bg-cyan-500/10 text-cyan-300 border-cyan-500/20",
    filename: "useSunbornQuery.ts",
    description: "Type-safe React custom hook with automatic deduplication and cache telemetry.",
    code: `import { useState, useCallback } from "react";

export function useSunbornQuery() {
  const [data, setData] = useState<string | null>(null);
  const [isCached, setIsCached] = useState<boolean>(false);
  const [latencyUs, setLatencyUs] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);

  const query = useCallback(async (prompt: string) => {
    setLoading(true);
    const t0 = performance.now();
    try {
      const res = await fetch("/api/proxy/v1/chat/completions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          model: "gpt-4o",
          messages: [{ role: "user", content: prompt }],
          temperature: 0.0
        })
      });
      const json = await res.json();
      setData(json.choices[0]?.message?.content ?? "");
      setIsCached(json.sunborn_cached ?? false);
      setLatencyUs(Math.round((performance.now() - t0) * 1000));
    } finally {
      setLoading(false);
    }
  }, []);

  return { query, data, isCached, latencyUs, loading };
}`,
  },
  vue: {
    id: "vue",
    name: "Vue 3",
    badge: "Composition API",
    badgeColor: "bg-emerald-500/10 text-emerald-300 border-emerald-500/20",
    filename: "useSunborn.ts",
    description: "Reactive composable with Vue 3 Signals / Ref reactivity and zero runtime overhead.",
    code: `<script setup lang="ts">
import { ref } from 'vue';

const response = ref<string>('');
const isCached = ref<boolean>(false);
const latencyUs = ref<number>(0);
const loading = ref<boolean>(false);

async function executeQuery(prompt: string) {
  loading.value = true;
  const start = performance.now();
  try {
    const res = await fetch('http://localhost:8080/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': 'sb_live_production_key'
      },
      body: JSON.stringify({
        model: 'gpt-4o',
        messages: [{ role: 'user', content: prompt }]
      })
    });
    const result = await res.json();
    response.value = result.choices[0].message.content;
    isCached.value = result.sunborn_cached ?? true;
    latencyUs.value = Math.round((performance.now() - start) * 1000);
  } finally {
    loading.value = false;
  }
}
</script>`,
  },
  angular: {
    id: "angular",
    name: "Angular",
    badge: "v18+ Signals",
    badgeColor: "bg-rose-500/10 text-rose-300 border-rose-500/20",
    filename: "sunborn.service.ts",
    description: "Injectable enterprise service with reactive Signals and HttpClient integration.",
    code: `import { Injectable, signal, inject } from '@angular/core';
import { HttpClient, HttpHeaders } from '@angular/common/http';

export interface SunbornResponse {
  choices: Array<{ message: { content: string } }>;
  sunborn_cached: boolean;
  sunborn_tier: string;
}

@Injectable({ providedIn: 'root' })
export class SunbornService {
  private http = inject(HttpClient);
  
  // Angular 18+ Reactive Signals
  readonly latestCompletion = signal<string>('');
  readonly isCached = signal<boolean>(false);
  readonly currentTier = signal<string>('none');

  query(prompt: string) {
    const headers = new HttpHeaders({
      'Content-Type': 'application/json',
      'x-api-key': 'sb_live_production_key'
    });

    return this.http.post<SunbornResponse>('http://localhost:8080/v1/chat/completions', {
      model: 'gpt-4o',
      messages: [{ role: 'user', content: prompt }],
      temperature: 0.0
    }, { headers }).subscribe(res => {
      this.latestCompletion.set(res.choices[0]?.message.content ?? '');
      this.isCached.set(res.sunborn_cached);
      this.currentTier.set(res.sunborn_tier);
    });
  }
}`,
  },
  typescript: {
    id: "typescript",
    name: "TypeScript SDK",
    badge: "Official SDK",
    badgeColor: "bg-violet-500/10 text-violet-300 border-violet-500/20",
    filename: "client.ts",
    description: "Fully typed enterprise SDK with connection pooling and cryptographic attestation.",
    code: `import { SunbornClient } from "@sunborn/sdk";

const client = new SunbornClient({
  baseUrl: "http://localhost:8080",
  apiKey: process.env.SUNBORN_API_KEY!,
  timeoutMs: 15000,
  maxRetries: 3
});

// Deterministic semantic cache query
const response = await client.chatCompletion({
  model: "gpt-4o",
  messages: [{ role: "user", content: "Explain Byzantine fault tolerance." }],
  temperature: 0.0,
  maxTokens: 500
});

console.log("Output:", response.choices[0].message.content);
console.log("Cached:", response.cached); // true
console.log("Tier:", response.tier);     // "exact" (<0.9µs)
console.log("Attestation Root:", response.merkle_proof?.root_hash);`,
  },
  curl: {
    id: "curl",
    name: "cURL / Raw HTTP",
    badge: "Wire Protocol",
    badgeColor: "bg-slate-500/10 text-slate-300 border-slate-500/20",
    filename: "request.sh",
    description: "Universal wire protocol compatible with OpenAI, Anthropic, and Gemini SDKs.",
    code: `curl -X POST http://localhost:8080/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "x-api-key: sb_live_production_key" \\
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "user", "content": "What is deterministic semantic caching?"}
    ],
    "temperature": 0.0
  }'`,
  },
};

export function FrameworkIntegrations() {
  const [activeTab, setActiveTab] = useState<FrameworkTab>("html");
  const [copied, setCopied] = useState<boolean>(false);

  const current = SNIPPETS[activeTab];

  const handleCopy = () => {
    navigator.clipboard.writeText(current.code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const tabKeys = Object.keys(SNIPPETS) as FrameworkTab[];

  const handleKeyDown = (e: React.KeyboardEvent, index: number) => {
    if (e.key === "ArrowRight") {
      e.preventDefault();
      const nextIndex = (index + 1) % tabKeys.length;
      setActiveTab(tabKeys[nextIndex]);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      const prevIndex = (index - 1 + tabKeys.length) % tabKeys.length;
      setActiveTab(tabKeys[prevIndex]);
    }
  };

  return (
    <section
      aria-label="Multi-Framework Client Integration Suite"
      className="p-6 rounded-2xl bg-slate-900/60 border border-white/10 backdrop-blur-md space-y-6"
    >
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <Code2 className="w-5 h-5 text-cyan-400" aria-hidden="true" />
            <h2 className="text-lg font-bold text-white tracking-tight">
              Multi-Framework Client Integration Suite
            </h2>
          </div>
          <p className="text-xs text-slate-400 mt-1">
            Universal drop-in wire compatibility across every frontend and backend ecosystem.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-cyan-950/60 border border-cyan-500/30 text-xs font-mono text-cyan-300">
            <Sparkles className="w-3.5 h-3.5 text-cyan-400" aria-hidden="true" />
            Zero-Config Wire Parity
          </span>
        </div>
      </div>

      {/* Accessible Framework Tabs with WAI-ARIA Roving Tabindex */}
      <div
        role="tablist"
        aria-label="Select frontend or backend framework"
        className="flex flex-wrap items-center gap-2 p-1.5 rounded-xl bg-slate-950/80 border border-white/10"
      >
        {tabKeys.map((tabKey, idx) => {
          const tab = SNIPPETS[tabKey];
          const isSelected = activeTab === tabKey;
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={isSelected}
              aria-controls={`panel-${tab.id}`}
              id={`tab-${tab.id}`}
              tabIndex={isSelected ? 0 : -1}
              onClick={() => setActiveTab(tabKey)}
              onKeyDown={(e) => handleKeyDown(e, idx)}
              className={`flex items-center gap-2 px-3.5 py-2 rounded-lg text-xs font-medium transition-colors ${
                isSelected
                  ? "bg-cyan-500/20 text-white border border-cyan-500/30 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span>{tab.name}</span>
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded border font-mono ${tab.badgeColor}`}
              >
                {tab.badge}
              </span>
            </button>
          );
        })}
      </div>

      {/* Code Editor Preview & Telemetry Output */}
      <div
        role="tabpanel"
        id={`panel-${current.id}`}
        aria-labelledby={`tab-${current.id}`}
        className="rounded-xl border border-white/10 bg-slate-950/90 overflow-hidden font-mono"
      >
        {/* Editor Window Bar */}
        <div className="flex items-center justify-between px-4 py-2.5 bg-slate-900/80 border-b border-white/10 text-xs text-slate-400">
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5" aria-hidden="true">
              <span className="w-2.5 h-2.5 rounded-full bg-rose-500/80" />
              <span className="w-2.5 h-2.5 rounded-full bg-amber-500/80" />
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500/80" />
            </div>
            <span className="text-slate-300 font-semibold">{current.filename}</span>
            <span className="text-slate-500">·</span>
            <span className="text-slate-400 text-[11px] font-sans">{current.description}</span>
          </div>

          <button
            onClick={handleCopy}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-slate-800/80 hover:bg-slate-700/80 text-slate-300 hover:text-white border border-white/10 text-xs transition-colors"
            aria-label={copied ? "Code copied to clipboard" : "Copy Code"}
          >
            {copied ? (
              <>
                <Check className="w-3.5 h-3.5 text-emerald-400" aria-hidden="true" />
                <span className="text-emerald-400 font-semibold">Copied!</span>
              </>
            ) : (
              <>
                <Copy className="w-3.5 h-3.5" aria-hidden="true" />
                <span>Copy Code</span>
              </>
            )}
          </button>
        </div>

        {/* Code Content */}
        <pre className="p-4 text-xs text-slate-300 overflow-x-auto leading-relaxed select-all">
          <code>{current.code}</code>
        </pre>
      </div>

      {/* Wire Invariant Badges */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-2">
        <div className="p-3 rounded-lg bg-slate-950/40 border border-white/5 flex items-start gap-2.5">
          <Terminal className="w-4 h-4 text-cyan-400 mt-0.5 shrink-0" aria-hidden="true" />
          <div className="text-xs">
            <div className="font-semibold text-white">Zero Client Changes</div>
            <div className="text-slate-400 text-[11px] mt-0.5">
              Simply point existing SDK `baseURL` to Sunborn port 8080.
            </div>
          </div>
        </div>

        <div className="p-3 rounded-lg bg-slate-950/40 border border-white/5 flex items-start gap-2.5">
          <Layers className="w-4 h-4 text-emerald-400 mt-0.5 shrink-0" aria-hidden="true" />
          <div className="text-xs">
            <div className="font-semibold text-white">Automatic L1 Caching</div>
            <div className="text-slate-400 text-[11px] mt-0.5">
              Sub-microsecond exact repeats served without touching GPU models.
            </div>
          </div>
        </div>

        <div className="p-3 rounded-lg bg-slate-950/40 border border-white/5 flex items-start gap-2.5">
          <Sparkles className="w-4 h-4 text-violet-400 mt-0.5 shrink-0" aria-hidden="true" />
          <div className="text-xs">
            <div className="font-semibold text-white">Conformal Safety Gates</div>
            <div className="text-slate-400 text-[11px] mt-0.5">
              AST guardrails reject semantic polarity shifts and negated queries.
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
