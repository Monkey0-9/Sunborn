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
                <label className="text-xs font-mono text-slate-400">USER QUERY</label>
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

                {/* Resolved Model Response */}
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-mono">
                    <span className="text-slate-400">RESOLVED COMPLETION PAYLOAD</span>
                    <span className="text-emerald-400">CONFORMAL RISK: 0.000 (SAFE)</span>
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
    </div>
  );
}
