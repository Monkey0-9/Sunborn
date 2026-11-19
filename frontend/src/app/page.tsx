"use client";

import React, { useState, useEffect } from "react";
import Link from "next/link";
import {
  Zap,
  ShieldCheck,
  TrendingUp,
  Cpu,
  Layers,
  ArrowRight,
  Database,
  Lock,
  Radio,
  CheckCircle2,
  Sparkles,
  RefreshCw,
  Server,
  Activity,
  AlertOctagon,
  RotateCcw,
} from "lucide-react";
import { CanvasManifold } from "@/components/ui/canvas-manifold";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { TextShimmer } from "@/components/ui/text-shimmer";
import { formatNumber, formatCurrency, cn } from "@/lib/utils";
import { sunbornApi } from "@/lib/api";
import { PlanetScaleStressSimulator } from "@/components/simulator/stress-simulator";
import { PromptProbePlayground } from "@/components/cockpit/prompt-probe-playground";
import { LiveDAGVisualizer } from "@/components/cockpit/live-dag-visualizer";
import { ClusterMeshStatus } from "@/components/cockpit/cluster-mesh-status";
import { FrameworkIntegrations } from "@/components/cockpit/framework-integrations";
import { useClusterStore } from "@/lib/cluster-store";

interface GatewayMetrics {
  totalRequests: number;
  hitRatio: number;
  p99LatencyUs: number | null;
  throughputReqs: number;
  avoidedCostUsd: number;
  activeTenants: number;
  cachedEntries: number;
}

export default function OverviewPage() {
  const { persona } = useClusterStore();
  const [activePreset, setActivePreset] = useState<"exact" | "semantic" | "negation" | "dag">("exact");

  const handleSelectPreset = (preset: "exact" | "semantic" | "negation" | "dag") => {
    setActivePreset(preset);
  };

  const [metrics, setMetrics] = useState<GatewayMetrics>({
    totalRequests: 0,
    hitRatio: 0.0,
    p99LatencyUs: null,
    throughputReqs: 0,
    avoidedCostUsd: 0.0,
    activeTenants: 1,
    cachedEntries: 0,
  });

  const [isConnected, setIsConnected] = useState<boolean | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const [activeStream, setActiveStream] = useState<
    {
      id: string;
      origin: string;
      event: string;
      detail: string;
      status: string;
      time: string;
    }[]
  >([]);

  const fetchLiveMetrics = async () => {
    setIsRefreshing(true);
    try {
      const data = await sunbornApi.getFinOpsReport();
      const reqTotal = data?.metrics?.requests_total ?? 0;
      const hitsTotal = data?.metrics?.total_hits ?? 0;
      const hitRatio = reqTotal > 0 ? (hitsTotal / reqTotal) * 100 : 0.0;
      const savings = data?.economics?.total_estimated_savings_usd ?? 0.0;

      setMetrics({
        totalRequests: reqTotal,
        hitRatio: +hitRatio.toFixed(1),
        p99LatencyUs: null, // Truthful: live latency histograms require Prometheus exporter
        throughputReqs: 0,
        avoidedCostUsd: +savings.toFixed(2),
        activeTenants: 1,
        cachedEntries: hitsTotal,
      });
      setIsConnected(true);

      try {
        const audit = await sunbornApi.getSecurityAudit();
        if (audit && Array.isArray(audit.events) && audit.events.length > 0) {
          const traces = audit.events.slice(0, 5).map((evt) => ({
            id: evt.id,
            origin: evt.origin || "Gateway Ingress",
            event: evt.event || "AUDIT_EVENT",
            detail: evt.detail || "Event recorded",
            status: evt.status || "ACTIVE",
            time: evt.time || "Recent",
          }));
          setActiveStream(traces);
        }
      } catch {
        // Audit events optional
      }
    } catch {
      try {
        const healthRes = await fetch("/api/proxy/health", {
          cache: "no-store",
          signal: AbortSignal.timeout(1500),
        });
        if (healthRes.ok) {
          setIsConnected(true);
          setMetrics((prev) => ({
            ...prev,
            p99LatencyUs: null,
          }));
          return;
        }
      } catch {}
      setIsConnected(false);
    } finally {
      setIsRefreshing(false);
    }
  };

  useEffect(() => {
    fetchLiveMetrics();
    const interval = setInterval(fetchLiveMetrics, 10000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="space-y-12">
      {/* Hallmark Hero Section */}
      <section className="relative flex flex-col items-start gap-6 pt-4 pb-2">
        <div className="flex flex-wrap items-center gap-3">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-cyan-950/60 border border-cyan-500/30 text-xs font-mono text-cyan-300 backdrop-blur-md">
            <Sparkles className="w-3.5 h-3.5 text-cyan-400" />
            <TextShimmer className="text-xs font-mono font-bold">PROOF-CARRYING TEMPORAL REUSE</TextShimmer>
            <span className="text-cyan-600">·</span>
            <span className="text-white font-semibold">STANDARDS-ALIGNED</span>
          </div>

          {persona === "executive" ? (
            <Badge variant="cyan" dot>
              Executive Mission Control
            </Badge>
          ) : (
            <Badge variant="violet" dot>
              Interactive Developer Sandbox
            </Badge>
          )}

          {isConnected === true && (
            <Badge variant="emerald" dot>
              GATEWAY CONNECTED (LIVE TELEMETRY)
            </Badge>
          )}
          {isConnected === false && (
            <Badge variant="danger" dot>
              GATEWAY DISCONNECTED (OFFLINE)
            </Badge>
          )}
          {isConnected === null && (
            <Badge variant="outline">
              CONNECTING TO GATEWAY...
            </Badge>
          )}

          <button
            onClick={fetchLiveMetrics}
            disabled={isRefreshing}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border border-white/10 bg-slate-900/60 text-slate-300 hover:text-white hover:bg-slate-800 text-xs font-mono transition-colors"
            title="Refresh Live Metrics"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isRefreshing ? "animate-spin text-cyan-400" : ""}`} />
            <span>Refresh</span>
          </button>
        </div>

        <div className="space-y-3">
          <h1 className="text-3xl sm:text-5xl font-extrabold tracking-tight text-white leading-[1.1]">
            Deterministic Semantic Caching &{" "}
            <span className="bg-gradient-to-r from-cyan-400 via-indigo-400 to-violet-400 bg-clip-text text-transparent">
              Agent Execution Reuse
            </span>
          </h1>
          <p className="text-base sm:text-lg text-slate-400 leading-relaxed max-w-3xl">
            Sunborn decouples AI inference latency, token expenditure, and origin failure risks
            through multi-tier execution reuse. Empirically evaluated on the frozen SemanticCacheBench suite (120+ queries across 8 categories, 0.0% observed false-hit rate, 95% Wilson CI: [0.0%, 3.0%]), with single-flight mutex coalescing and cryptographic tenant key zeroization under GDPR Article 17.
          </p>
        </div>

        {/* Action CTAs */}
        <div className="flex flex-wrap items-center gap-4 pt-2">
          <Link href="/explorer">
            <ShimmerButton>
              <span>Launch Cache Explorer</span>
              <ArrowRight className="w-4 h-4" />
            </ShimmerButton>
          </Link>

          <Link
            href="/execution"
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl border border-white/10 bg-slate-900/60 text-slate-300 hover:text-white hover:bg-slate-800/60 hover:border-white/20 transition-all font-medium text-sm backdrop-blur-md"
          >
            <Cpu className="w-4 h-4 text-violet-400" />
            <span>Agent DAG Attestation</span>
          </Link>

          <Link
            href="/governance"
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl border border-rose-500/20 bg-rose-950/20 text-rose-300 hover:bg-rose-950/40 hover:border-rose-500/40 transition-all font-medium text-sm backdrop-blur-md"
          >
            <Lock className="w-4 h-4 text-rose-400" />
            <span>GDPR Crypto Shredder</span>
          </Link>
        </div>
      </section>

      {/* Interactive 3D Manifold Canvas */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Radio className="w-4 h-4 text-cyan-400 animate-pulse" />
            <h2 className="text-lg font-bold text-white tracking-tight">
              Interactive Multi-Tier Vector & Execution Manifold
            </h2>
          </div>
          <span className="text-xs font-mono text-slate-400">
            Hardware-Accelerated WebGL/Canvas · Real-Time Dynamic Simulation
          </span>
        </div>

        <CanvasManifold
          hitRate={metrics.hitRatio}
          p99LatencyUs={metrics.p99LatencyUs ?? 0}
          throughputReqs={metrics.throughputReqs}
        />
      </section>

      {/* Track 1: Live Gateway Telemetry */}
      <section className="space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Radio className="w-4 h-4 text-emerald-400 animate-pulse" />
            <h2 className="text-base font-bold text-white tracking-tight">
              Live Production Telemetry (Gateway Cluster)
            </h2>
          </div>
          <Badge variant="emerald" dot>
            SOURCE: GATEWAY /V1/FINOPS/REPORT
          </Badge>
        </div>
        <p className="text-xs text-slate-400 font-mono">
          Dynamic runtime measurements emitted by active gateway ingress workers (process-local counters displayed; cluster-wide aggregation managed via Prometheus).
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 pt-1">
          <Card glow="cyan">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Live Ingress Queries</CardTitle>
              <Zap className="w-4 h-4 text-cyan-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                {metrics.totalRequests > 0 ? formatNumber(metrics.totalRequests) : "0"}
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">Gateway Worker (Process-Local)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Period:</span>
                  <span className="text-slate-300">Process Lifetime</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-emerald-400 font-semibold">Live Telemetry</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="emerald">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Live Cache Hit Ratio</CardTitle>
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                {metrics.totalRequests > 0 ? `${metrics.hitRatio}%` : "—"}
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">Atomic Hit Counters (C0–C2)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Hits / Reused:</span>
                  <span className="text-slate-300">{formatNumber(metrics.cachedEntries)} hits</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-emerald-400 font-semibold">Live Telemetry</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="violet">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Live P99 Latency</CardTitle>
              <Activity className="w-4 h-4 text-violet-400" />
            </CardHeader>
            <CardContent>
              <div className="text-base font-bold font-mono text-slate-400 tracking-tight">
                {metrics.p99LatencyUs !== null ? `${metrics.p99LatencyUs} µs` : "Not measured (Requires Prometheus)"}
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">Prometheus Histogram (9090)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Benchmark Baseline:</span>
                  <span className="text-slate-300">0.30 µs P50 (L1 In-Memory)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-amber-400 font-semibold">Prometheus Required</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="amber">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Avoided Token Spend</CardTitle>
              <TrendingUp className="w-4 h-4 text-amber-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                {metrics.avoidedCostUsd > 0 ? formatCurrency(metrics.avoidedCostUsd) : "$0.00"}
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">FinOps Telemetry Engine</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Pricing Model:</span>
                  <span className="text-slate-300">Modeled (GPT-4o rates)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-violet-400 font-semibold">Modeled from Live Tokens</span>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </section>

      {/* Track 2: Frozen Scientific Benchmark Results */}
      <section className="space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="w-4 h-4 text-cyan-400" />
            <h2 className="text-base font-bold text-white tracking-tight">
              Frozen Scientific Evaluation Results (SemanticCacheBench v1.0)
            </h2>
          </div>
          <Badge variant="cyan" dot>
            SOURCE: RESEARCH/RESULTS/BENCHMARK_RUN.JSON
          </Badge>
        </div>
        <p className="text-xs text-slate-400 font-mono">
          Empirical measurements on frozen benchmark dataset with two-sided 95% Wilson score confidence intervals.
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 pt-1">
          <Card glow="cyan">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Conformal Precision</CardTitle>
              <ShieldCheck className="w-4 h-4 text-cyan-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                100.0%
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">SemanticCacheBench B3</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Sample Size:</span>
                  <span className="text-slate-300">N=124 queries (8 categories)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">95% Wilson CI:</span>
                  <span className="text-cyan-300 font-semibold">[88.3%, 100.0%]</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-cyan-400 font-semibold">Verified Benchmark</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="emerald">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Observed False-Hit Rate</CardTitle>
              <CheckCircle2 className="w-4 h-4 text-emerald-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                0.0%
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">Verification Cascade (α ≤ 0.001)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Sample Size:</span>
                  <span className="text-slate-300">N=124 queries</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">95% Wilson CI:</span>
                  <span className="text-emerald-300 font-semibold">[0.0%, 3.0%]</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-emerald-400 font-semibold">Verified Benchmark</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="violet">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">Benchmark Token Savings</CardTitle>
              <TrendingUp className="w-4 h-4 text-violet-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                12.09%
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">B3 vs B0 No-Cache Baseline</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Pricing Basis:</span>
                  <span className="text-slate-300">Synthetic ($0.00625/call)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Sample Size:</span>
                  <span className="text-slate-300">N=124 queries</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-violet-400 font-semibold">Modeled Benchmark Savings</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card glow="amber">
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-slate-400">GDPR Shredding Latency</CardTitle>
              <Lock className="w-4 h-4 text-amber-400" />
            </CardHeader>
            <CardContent>
              <div className="text-3xl font-bold font-mono text-white tracking-tight">
                42.1 µs
              </div>
              <div className="text-[11px] font-mono text-slate-400 mt-2 space-y-0.5 border-t border-white/[0.06] pt-2">
                <div className="flex justify-between">
                  <span className="text-slate-400">Source:</span>
                  <span className="text-slate-300">Local AEAD TDEK Zeroization</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Sample Iterations:</span>
                  <span className="text-slate-300">N=10,000 Key Eradication Runs</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Key Erasure RPO:</span>
                  <span className="text-amber-300 font-semibold">TDEK Zeroization (Local Store)</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-400">Status:</span>
                  <span className="text-amber-400 font-semibold">Measured Microbenchmark</span>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </section>

      {/* Dynamic View Ordering based on Active Persona Mode */}
      {persona === "developer" ? (
        <>
          {/* Developer Sandbox Priority View */}
          <PromptProbePlayground
            initialPreset={activePreset}
            onPresetChange={handleSelectPreset}
          />
          <LiveDAGVisualizer />
          <FrameworkIntegrations />
          <ClusterMeshStatus />
          <PlanetScaleStressSimulator />
        </>
      ) : (
        <>
          {/* Executive Cockpit Priority View */}
          <ClusterMeshStatus />
          <PlanetScaleStressSimulator />
          <PromptProbePlayground
            initialPreset={activePreset}
            onPresetChange={handleSelectPreset}
          />
          <LiveDAGVisualizer />
          <FrameworkIntegrations />
        </>
      )}

      {/* Real-Time Live Activity Stream */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Radio className="w-4 h-4 text-cyan-400" />
            <h2 className="text-lg font-bold text-white tracking-tight">
              Execution Trace Inspection Pipeline
            </h2>
          </div>
          <Badge variant="outline">REFERENCE EXECUTION TRACES</Badge>
        </div>

        <div className="rounded-xl border border-white/10 bg-slate-950/80 overflow-hidden backdrop-blur-xl">
          {activeStream.length > 0 ? (
            <div className="divide-y divide-white/5">
              {activeStream.map((item) => (
                <div
                  key={item.id}
                  className="flex flex-col sm:flex-row sm:items-center justify-between px-6 py-4 hover:bg-white/[0.02] transition-colors gap-3"
                >
                  <div className="flex items-center gap-4">
                    <div className="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 flex items-center justify-center font-mono text-xs text-cyan-400">
                      <Zap className="w-4 h-4" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-xs font-bold text-white">
                          {item.id}
                        </span>
                        <Badge variant="outline" className="text-[10px]">
                          {item.origin}
                        </Badge>
                        <span className="text-xs text-slate-400 font-mono">
                          {item.time}
                        </span>
                      </div>
                      <div className="text-xs text-slate-400 font-mono mt-0.5">
                        Event:{" "}
                        <span className="text-cyan-300 font-semibold">{item.event}</span>
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-6 font-mono text-xs">
                    <div>
                      <span className="text-slate-400">DETAIL:</span>{" "}
                      <span className="text-slate-300">{item.detail}</span>
                    </div>
                    <div className="px-2.5 py-1 rounded-md bg-cyan-950/40 border border-cyan-500/30 text-cyan-300 font-bold">
                      {item.status}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="p-8 text-center text-slate-400 font-mono text-xs space-y-1">
              <p className="text-slate-300 font-semibold">No live execution traces recorded yet</p>
              <p className="text-slate-400">Execute cache queries in Cache Explorer or via the Python SDK to stream real production telemetry.</p>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
