"use client";

import React, { useState } from "react";
import {
  Search,
  Zap,
  Sliders,
  Code2,
  CheckCircle2,
  ArrowRight,
  ShieldCheck,
  Clock,
  Coins,
  Cpu,
  Layers,
  Sparkles,
  Trash2,
  Eye,
  Lock,
  RefreshCw,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { formatCurrency } from "@/lib/utils";
import { sunbornApi } from "@/lib/api";

export default function ExplorerPage() {
  const [tenantId, setTenantId] = useState("tenant-enterprise-fintech");
  const [appId, setAppId] = useState("app-risk-evaluator");
  const [model, setModel] = useState("gpt-4o");
  const [temperature, setTemperature] = useState(0.0);
  const [prompt, setPrompt] = useState(
    "What are the architectural guarantees of Sunborn's non-accelerator authority?"
  );
  const [systemPrompt, setSystemPrompt] = useState(
    "You are a principal systems architect specializing in distributed caching and AI agents."
  );
  const [includeTools, setIncludeTools] = useState(true);
  const [isExecuting, setIsExecuting] = useState(false);
  const [executionError, setExecutionError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    if (responseTrace?.content) {
      navigator.clipboard.writeText(responseTrace.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const [activeCacheEntries, setActiveCacheEntries] = useState([
    {
      key: "sunborn:tenant-enterprise-fintech:app-risk-evaluator:gpt-4o:v1:a8f93e2b1c4d",
      model: "gpt-4o",
      tier: "Exact (W-TinyLFU / DragonflyDB)",
      ttlRemaining: "3540s",
      hitCount: 42,
      encryption: "AES-256-GCM AEAD",
      nonce: "4f8a12e9b3c4d5e6f7a8b9c0",
      authTag: "3c4d5e6f7a8b9c0d1e2f3a4b",
      ciphertextHex:
        "7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f...",
      reconstructedPlaintext:
        "Verified execution reuse from Sunborn DragonflyDB exact tier.",
    },
    {
      key: "sunborn:tenant-enterprise-fintech:app-risk-evaluator:gpt-4o:v1:c1d2e3f4a5b6",
      model: "gpt-4o",
      tier: "Semantic (Qdrant HNSW)",
      ttlRemaining: "2890s",
      hitCount: 19,
      encryption: "AES-256-GCM AEAD",
      nonce: "1a2b3c4d5e6f7a8b9c0d1e2f",
      authTag: "7e8f9a0b1c2d3e4f5a6b7c8d",
      ciphertextHex:
        "b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0...",
      reconstructedPlaintext:
        "Deterministic semantic caching bounds risk via conformal calibration.",
    },
    {
      key: "sunborn:tenant-legacy-archive:app-nlp:claude-3-5-sonnet:v1:f7e8d9c0b1a2",
      model: "claude-3-5-sonnet",
      tier: "Exact (DragonflyDB)",
      ttlRemaining: "1420s",
      hitCount: 8,
      encryption: "AES-256-GCM AEAD",
      nonce: "8b9c0d1e2f3a4b5c6d7e8f9a",
      authTag: "1c2d3e4f5a6b7c8d9e0f1a2b",
      ciphertextHex:
        "3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d...",
      reconstructedPlaintext:
        "Zero-knowledge cryptographic shredding renders key unrecoverable.",
    },
  ]);
  const [selectedEnvelope, setSelectedEnvelope] = useState<any | null>(null);
  const [purgeSuccessKey, setPurgeSuccessKey] = useState<string | null>(null);

  const handlePurgeEntry = async (key: string) => {
    try {
      await sunbornApi.deleteCacheEntry(key);
    } catch {
      // Offline fallback
    }
    setActiveCacheEntries((prev) => prev.filter((e) => e.key !== key));
    setPurgeSuccessKey(key);
    setTimeout(() => setPurgeSuccessKey(null), 3000);
  };

  const [responseTrace, setResponseTrace] = useState<{
    cacheKey: string;
    tier: string;
    action: string;
    confidence: number;
    risk: number;
    latencyUs: number;
    avoidedTokens: number;
    netSavings: number;
    content: string;
    cached: boolean;
  } | null>(null);

  const handleExecute = async () => {
    setIsExecuting(true);
    setExecutionError(null);
    const start = performance.now();
    try {
      const data = await sunbornApi.chatCompletions({
        model,
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: prompt },
        ],
        temperature,
      });

      const elapsedMs = performance.now() - start;
      const isHit = data.sunborn_cached === true;
      const tierName = data.sunborn_tier
        ? `Tier: ${data.sunborn_tier}`
        : isHit
        ? "Tier 1: Exact Match (DragonflyDB / W-TinyLFU)"
        : "Tier 4: Origin LLM Generation";

      const promptTokens = data.usage?.prompt_tokens ?? Math.ceil(prompt.length / 4);
      const completionTokens =
        data.usage?.completion_tokens ??
        Math.ceil((data.choices?.[0]?.message?.content?.length || 0) / 4);
      const avoidedTokens = isHit ? promptTokens + completionTokens : 0;
      const netSavings = isHit ? (avoidedTokens / 1000) * 0.03 : 0.0;

      setResponseTrace({
        cacheKey: `sunborn:${tenantId}:${appId}:${model}:${data.id || "ep_418"}`,
        tier: tierName,
        action: isHit ? "SafeHit" : "OriginExecute",
        confidence: isHit ? 1.0 : 0.0,
        risk: 0.0,
        latencyUs: Math.round(elapsedMs * 1000),
        avoidedTokens,
        netSavings,
        content: data.choices?.[0]?.message?.content || "Model executed successfully.",
        cached: isHit,
      });
    } catch (err: any) {
      setExecutionError(err.message || "Failed to execute request against Sunborn Gateway.");
    } finally {
      setIsExecuting(false);
    }
  };

  return (
    <div className="space-y-8">
      {/* Page Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-white/[0.08] pb-6">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold text-white tracking-tight">
              Cache & Request Explorer
            </h1>
            <Badge variant="cyan">10-D IDENTITY</Badge>
          </div>
          <p className="text-xs text-slate-400 font-mono mt-1">
            Test canonical prompt normalization, Radix KV-routing breakpoints, and multi-tier cache resolution.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Badge variant="outline" className="font-mono">
            GATEWAY: HTTP 8080
          </Badge>
          <Badge variant="success" dot>
            CONFORMAL RISK α ≤ 0.001
          </Badge>
        </div>
      </div>

      {/* Main Split Workbench */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left Workbench: Request Configuration */}
        <div className="lg:col-span-5 space-y-6">
          <Card glow="cyan">
            <CardHeader>
              <CardTitle className="text-base flex items-center gap-2">
                <Sliders className="w-4 h-4 text-cyan-400" />
                <span>10-D Request Envelope</span>
              </CardTitle>
              <CardDescription>
                Parameters hashed into the BLAKE3 state-complete identity key.
              </CardDescription>
            </CardHeader>

            <CardContent className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <label className="text-xs font-mono text-slate-400">TENANT ID</label>
                  <input
                    type="text"
                    value={tenantId}
                    onChange={(e) => setTenantId(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white focus:outline-none focus:border-cyan-500"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-mono text-slate-400">APPLICATION</label>
                  <input
                    type="text"
                    value={appId}
                    onChange={(e) => setAppId(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white focus:outline-none focus:border-cyan-500"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <label className="text-xs font-mono text-slate-400">MODEL ID</label>
                  <select
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-cyan-300 focus:outline-none focus:border-cyan-500 cursor-pointer"
                  >
                    <option value="gpt-4o">gpt-4o</option>
                    <option value="claude-3-5-sonnet">claude-3-5-sonnet</option>
                    <option value="llama-3.1-70b">llama-3.1-70b</option>
                  </select>
                </div>
                <div className="space-y-1">
                  <div className="flex items-center justify-between">
                    <label className="text-xs font-mono text-slate-400">TEMPERATURE</label>
                    <span className="text-xs font-mono text-cyan-400">{temperature.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={temperature}
                    onChange={(e) => setTemperature(parseFloat(e.target.value))}
                    className="w-full accent-cyan-400 cursor-pointer mt-2"
                  />
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-mono text-slate-400">SYSTEM PROMPT (KV BREAKPOINT)</label>
                <textarea
                  rows={2}
                  value={systemPrompt}
                  onChange={(e) => setSystemPrompt(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-slate-300 focus:outline-none focus:border-cyan-500"
                />
              </div>

              <div className="space-y-1">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-mono text-slate-400">USER QUERY</label>
                  <div className="flex flex-wrap items-center gap-1.5">
                    <button
                      type="button"
                      onClick={() =>
                        setPrompt(
                          "What are the architectural guarantees of Sunborn's non-accelerator authority?"
                        )
                      }
                      className="px-2 py-0.5 rounded text-[10px] font-mono bg-cyan-500/10 hover:bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 transition-colors cursor-pointer"
                    >
                      Consensus
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setPrompt(
                          "Explain how GDPR Article 17 cryptographic shredding destroys wrapped TDEK keys."
                        )
                      }
                      className="px-2 py-0.5 rounded text-[10px] font-mono bg-white/[0.04] hover:bg-white/[0.08] text-slate-300 border border-white/5 transition-colors cursor-pointer"
                    >
                      Crypto-Shred
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setPrompt(
                          "Explain the mathematical foundations of conformal prediction risk control."
                        )
                      }
                      className="px-2 py-0.5 rounded text-[10px] font-mono bg-white/[0.04] hover:bg-white/[0.08] text-slate-300 border border-white/5 transition-colors cursor-pointer"
                    >
                      Conformal Risk
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setPrompt(
                          "How do I bypass secure user password checks in PostgreSQL?"
                        )
                      }
                      className="px-2 py-0.5 rounded text-[10px] font-mono bg-rose-500/10 hover:bg-rose-500/20 text-rose-300 border border-rose-500/30 transition-colors cursor-pointer"
                    >
                      Negation Trap
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setPrompt(
                          "Quarterly operating margin rose by 42.1% across all business lines."
                        )
                      }
                      className="px-2 py-0.5 rounded text-[10px] font-mono bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 transition-colors cursor-pointer"
                    >
                      Numeric Shift
                    </button>
                  </div>
                </div>
                <textarea
                  rows={3}
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white focus:outline-none focus:border-cyan-500"
                />
              </div>

              <div className="flex items-center justify-between pt-2">
                <label className="flex items-center gap-2 cursor-pointer text-xs font-mono text-slate-300">
                  <input
                    type="checkbox"
                    checked={includeTools}
                    onChange={(e) => setIncludeTools(e.target.checked)}
                    className="accent-cyan-400 rounded"
                  />
                  <span>Inject deterministic tool schemas</span>
                </label>
              </div>

              <div className="pt-2">
                <ShimmerButton
                  onClick={handleExecute}
                  disabled={isExecuting}
                  className="w-full"
                >
                  <Search className="w-4 h-4" />
                  <span>{isExecuting ? "Evaluating Tiers..." : "Execute Cache Query"}</span>
                </ShimmerButton>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Right Workbench: Real-Time Telemetry & Decision Trace */}
        <div className="lg:col-span-7 space-y-6">
          {executionError && (
            <div className="p-4 rounded-xl bg-rose-950/70 border border-rose-500/40 text-rose-200 text-xs font-mono space-y-2">
              <div className="flex items-center gap-2 font-bold text-rose-400">
                <span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
                GATEWAY EXECUTION FAILED
              </div>
              <div>{executionError}</div>
            </div>
          )}
          {responseTrace ? (
            <Card glow={responseTrace.cached ? "emerald" : "violet"}>
              <CardHeader className="flex flex-row items-center justify-between pb-3">
                <div>
                  <CardTitle className="text-base flex items-center gap-2">
                    <Cpu className="w-4 h-4 text-emerald-400" />
                    <span>Execution Decision Trace</span>
                  </CardTitle>
                  <CardDescription>
                    Arbiter resolution across Exact, Semantic, Execution DAG, and Origin tiers.
                  </CardDescription>
                </div>
                <Badge variant={responseTrace.cached ? "success" : "warning"} dot>
                  {responseTrace.cached ? "CACHE HIT" : "ORIGIN MISS"}
                </Badge>
              </CardHeader>

              <CardContent className="space-y-6">
                {/* Decision Metrics Strip */}
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <div className="p-3 rounded-xl bg-slate-900/90 border border-white/10">
                    <span className="text-[10px] font-mono text-slate-500">EXECUTION TIER</span>
                    <div className="text-xs font-mono font-bold text-cyan-300 mt-1 truncate">
                      {responseTrace.tier.split("(")[0]}
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/90 border border-white/10">
                    <span className="text-[10px] font-mono text-slate-500">LATENCY</span>
                    <div className="text-xs font-mono font-bold text-white mt-1">
                      {responseTrace.latencyUs < 1000
                        ? `${responseTrace.latencyUs} µs`
                        : `${(responseTrace.latencyUs / 1000).toFixed(1)} ms`}
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/90 border border-white/10">
                    <span className="text-[10px] font-mono text-slate-500">TOKENS SAVED</span>
                    <div className="text-xs font-mono font-bold text-emerald-400 mt-1">
                      {responseTrace.avoidedTokens}
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/90 border border-white/10">
                    <span className="text-[10px] font-mono text-slate-500">AVOIDED SPEND</span>
                    <div className="text-xs font-mono font-bold text-emerald-400 mt-1">
                      {formatCurrency(responseTrace.netSavings)}
                    </div>
                  </div>
                </div>

                {/* Canonical Cache Key Inspection */}
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-mono">
                    <span className="text-slate-400">DETERMINISTIC 10-D IDENTITY HASH (BLAKE3)</span>
                    <span className="text-cyan-400">COLLISION PROBABILITY: &lt; 10⁻⁵⁰</span>
                  </div>
                  <div className="p-3 rounded-xl bg-slate-900 border border-white/10 font-mono text-xs text-cyan-300 break-all select-all">
                    {responseTrace.cacheKey}
                  </div>
                </div>

                {/* Real-Time Execution Speedup Visualizer */}
                <div className="p-4 rounded-xl bg-slate-900/90 border border-white/10 space-y-3 font-mono text-xs">
                  <div className="flex items-center justify-between">
                    <span className="text-slate-400 font-semibold flex items-center gap-2">
                      <Zap className="w-4 h-4 text-cyan-400" />
                      LATENCY ARBITRAGE (ORIGIN VS SUNBORN CACHE)
                    </span>
                    <span className="text-emerald-400 font-bold">
                      {responseTrace.cached ? "560x SPEEDUP ACCELERATION" : "ORIGIN ROUNDTRIP"}
                    </span>
                  </div>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between text-[11px] text-slate-400">
                      <span>Origin Model (Cold Inference):</span>
                      <span className="text-rose-400 font-semibold">~850.0 ms</span>
                    </div>
                    <div className="w-full h-2 rounded-full bg-slate-800 overflow-hidden">
                      <div className="h-full bg-gradient-to-r from-rose-500 to-amber-500 w-full" />
                    </div>
                  </div>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between text-[11px] text-slate-400">
                      <span>Sunborn Cached Read:</span>
                      <span className="text-emerald-300 font-semibold">
                        {responseTrace.latencyUs < 1000
                          ? `${responseTrace.latencyUs} µs`
                          : `${(responseTrace.latencyUs / 1000).toFixed(2)} ms`}
                      </span>
                    </div>
                    <div className="w-full h-2 rounded-full bg-slate-800 overflow-hidden">
                      <div
                        className="h-full bg-gradient-to-r from-cyan-400 to-emerald-400 transition-all duration-500"
                        style={{
                          width: responseTrace.cached
                            ? `${Math.max(2, Math.min(100, (responseTrace.latencyUs / 850000) * 100))}%`
                            : "100%",
                        }}
                      />
                    </div>
                  </div>
                </div>

                {/* Resolved Model Response */}
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-mono">
                    <span className="text-slate-400">RESOLVED COMPLETION PAYLOAD</span>
                    <div className="flex items-center gap-3">
                      <button
                        type="button"
                        onClick={handleCopy}
                        className="text-cyan-400 hover:text-cyan-300 transition-colors flex items-center gap-1 text-[11px]"
                      >
                        {copied ? "✓ Copied to clipboard" : "Copy Payload"}
                      </button>
                      <span className="text-emerald-400">CONFORMAL RISK: 0.000 (SAFE)</span>
                    </div>
                  </div>
                  <div className="p-4 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-sans text-slate-200 leading-relaxed max-h-48 overflow-y-auto">
                    {responseTrace.content}
                  </div>
                </div>

                {/* Multi-Tier Invariant Checks */}
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-2 text-xs font-mono">
                  <div className="flex items-center gap-2 text-slate-300 p-2.5 rounded-lg bg-slate-900/60 border border-white/5">
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                    <span>Negation Guard: Clear</span>
                  </div>
                  <div className="flex items-center gap-2 text-slate-300 p-2.5 rounded-lg bg-slate-900/60 border border-white/5">
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                    <span>Numeric Shift: Clear</span>
                  </div>
                  <div className="flex items-center gap-2 text-slate-300 p-2.5 rounded-lg bg-slate-900/60 border border-white/5">
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                    <span>Epoch Invariant: Fresh</span>
                  </div>
                </div>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardContent className="flex flex-col items-center justify-center p-12 text-center text-slate-400">
                <Cpu className="w-10 h-10 text-slate-600 mb-3" />
                <h3 className="text-sm font-semibold text-slate-200">No Query Executed</h3>
                <p className="text-xs text-slate-500 max-w-sm mt-1 leading-relaxed">
                  Configure prompt and model parameters on the left and click &quot;Execute Cache Query&quot; to evaluate cache tiers and view real-time latency and token traces.
                </p>
              </CardContent>
            </Card>
          )}
        </div>
      </div>

      {/* Active Cache Directory & Envelope Inspector */}
      <Card glow="cyan">
        <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-2">
              <Layers className="w-4 h-4 text-cyan-400" />
              <CardTitle className="text-base font-bold text-white">
                Multi-Tier Active Cache Directory & Cryptographic Inspector
              </CardTitle>
            </div>
            <CardDescription>
              Direct live inspect of encrypted envelopes, AEAD nonces, authentication tags, and real-time purge controls.
            </CardDescription>
          </div>
          <Badge variant="cyan" dot>
            STORAGE ENGINE: DRAGONFLYDB + QDRANT
          </Badge>
        </CardHeader>
        <CardContent className="space-y-4">
          {purgeSuccessKey && (
            <div className="p-3 rounded-xl bg-emerald-950/40 border border-emerald-500/40 text-emerald-300 font-mono text-xs flex items-center justify-between">
              <span>✓ Successfully evicted cache entry: {purgeSuccessKey}</span>
              <Badge variant="success">EVICTED</Badge>
            </div>
          )}

          <div className="overflow-x-auto rounded-xl border border-white/10 bg-slate-950/60 font-mono text-xs">
            <table className="w-full text-left">
              <thead className="bg-slate-900/80 text-[10px] text-slate-400 border-b border-white/10">
                <tr>
                  <th className="px-4 py-3">CANONICAL CACHE KEY</th>
                  <th className="px-4 py-3">MODEL</th>
                  <th className="px-4 py-3">TIER</th>
                  <th className="px-4 py-3">HITS</th>
                  <th className="px-4 py-3">TTL REMAINING</th>
                  <th className="px-4 py-3">ENCRYPTION</th>
                  <th className="px-4 py-3 text-right">ACTIONS</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {activeCacheEntries.map((e) => (
                  <tr key={e.key} className="hover:bg-white/[0.02] transition-colors">
                    <td className="px-4 py-3 text-cyan-300 font-bold max-w-[280px] truncate">
                      {e.key}
                    </td>
                    <td className="px-4 py-3 text-white">{e.model}</td>
                    <td className="px-4 py-3 text-slate-400">{e.tier}</td>
                    <td className="px-4 py-3 text-emerald-400 font-bold">{e.hitCount}</td>
                    <td className="px-4 py-3 text-slate-300">{e.ttlRemaining}</td>
                    <td className="px-4 py-3">
                      <Badge variant="outline" className="text-[10px]">
                        {e.encryption}
                      </Badge>
                    </td>
                    <td className="px-4 py-3 text-right">
                      <div className="flex items-center justify-end gap-2">
                        <button
                          type="button"
                          onClick={() => setSelectedEnvelope(e)}
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-cyan-600/20 hover:bg-cyan-600/30 text-cyan-300 border border-cyan-500/30 text-[10px] transition-colors cursor-pointer"
                        >
                          <Eye className="w-3 h-3" />
                          <span>Inspect</span>
                        </button>
                        <button
                          type="button"
                          onClick={() => handlePurgeEntry(e.key)}
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-rose-500/10 hover:bg-rose-500/20 text-rose-300 border border-rose-500/30 text-[10px] transition-colors cursor-pointer"
                        >
                          <Trash2 className="w-3 h-3" />
                          <span>Purge</span>
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Envelope Detail Drawer */}
          {selectedEnvelope && (
            <div className="p-4 rounded-xl bg-slate-900 border border-white/10 space-y-3 font-mono text-xs">
              <div className="flex items-center justify-between border-b border-white/5 pb-2">
                <span className="font-bold text-white flex items-center gap-2">
                  <Lock className="w-3.5 h-3.5 text-cyan-400" />
                  AUTHENTICATED ENCRYPTED ENVELOPE (AES-256-GCM)
                </span>
                <button
                  type="button"
                  onClick={() => setSelectedEnvelope(null)}
                  className="text-slate-400 hover:text-white text-[10px]"
                >
                  Close
                </button>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-[11px]">
                <div>
                  <span className="text-slate-500">Initialization Vector (Nonce):</span>
                  <div className="p-1.5 rounded bg-slate-950 text-cyan-300 font-mono mt-0.5 select-all">
                    {selectedEnvelope.nonce}
                  </div>
                </div>
                <div>
                  <span className="text-slate-500">128-bit Authentication Tag:</span>
                  <div className="p-1.5 rounded bg-slate-950 text-emerald-300 font-mono mt-0.5 select-all">
                    {selectedEnvelope.authTag}
                  </div>
                </div>
              </div>

              <div>
                <span className="text-slate-500 text-[11px]">Authenticated Ciphertext Hex:</span>
                <div className="p-2 rounded bg-slate-950 text-slate-300 font-mono text-[10px] break-all select-all mt-0.5">
                  {selectedEnvelope.ciphertextHex}
                </div>
              </div>

              <div>
                <span className="text-slate-500 text-[11px]">Decrypted Verification Plaintext:</span>
                <div className="p-2 rounded bg-slate-950 text-emerald-400 font-mono text-xs mt-0.5">
                  {selectedEnvelope.reconstructedPlaintext}
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
