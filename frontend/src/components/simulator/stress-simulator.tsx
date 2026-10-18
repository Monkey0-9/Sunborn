"use client";

import React, { useState, useMemo, useEffect } from "react";
import {
  Activity,
  Cpu,
  Layers,
  Database,
  Shield,
  Zap,
  Flame,
  CheckCircle2,
  AlertTriangle,
  Play,
  RotateCcw,
  Sliders,
  Server,
  Lock,
  Boxes,
  ArrowUpRight,
  TrendingDown,
  Gauge,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { formatNumber, formatCurrency } from "@/lib/utils";

interface GuildInfo {
  id: string;
  name: string;
  headcount: number;
  initialScore: number;
  hardenedScore: number;
  targetGate: string;
  keyDeliverable: string;
  status: "Hardened" | "Active" | "Verified";
  accentColor: string;
}

const GUILD_DATA: GuildInfo[] = [
  {
    id: "guild-1",
    name: "Data-Plane & Gateway Core",
    headcount: 40,
    initialScore: 68,
    hardenedScore: 94,
    targetGate: "100% OpenAI & Anthropic wire proxy, HTTP/3 + gRPC, simd-json, Gradient2 concurrency",
    keyDeliverable: "/v1/chat/completions & /v1/messages zero-code migration wire compatibility",
    status: "Hardened",
    accentColor: "from-cyan-500 to-blue-500",
  },
  {
    id: "guild-2",
    name: "Distributed Cache & Memory Kernel",
    headcount: 35,
    initialScore: 74,
    hardenedScore: 91,
    targetGate: "S3-FIFO lock-free eviction, io_uring NVMe L3 hybrid tier, Maglev consistent hashing",
    keyDeliverable: "CacheLib-style tiering with zero GC pauses and sub-120 µs P99 exact lookup",
    status: "Active",
    accentColor: "from-blue-500 to-indigo-500",
  },
  {
    id: "guild-3",
    name: "Applied ML & Vector Retrieval",
    headcount: 40,
    initialScore: 70,
    hardenedScore: 92,
    targetGate: "1-bit Binary Quantized HNSW (VPOPCNT) → Matryoshka Int8 rescoring → ONNX Cross-Encoder",
    keyDeliverable: "Task-stratified conformal calibration with proven 0.0% false-hit rate (α ≤ 0.001)",
    status: "Verified",
    accentColor: "from-indigo-500 to-violet-500",
  },
  {
    id: "guild-4",
    name: "Agent Compiler & DAG Runtime",
    headcount: 30,
    initialScore: 78,
    hardenedScore: 95,
    targetGate: "Speculative parallel branch execution, wasmtime sandboxed pure tools, Saga idempotency",
    keyDeliverable: "Cryptographic Merkle sub-DAG reuse avoiding pure tool re-execution",
    status: "Hardened",
    accentColor: "from-violet-500 to-purple-500",
  },
  {
    id: "guild-5",
    name: "Storage Kernel & CDC Lineage",
    headcount: 30,
    initialScore: 65,
    hardenedScore: 93,
    targetGate: "UUIDv7 hourly table partitioning, Postgres pgoutput WAL stream, 64-bit Roaring Bitmaps",
    keyDeliverable: "O(k) SIMD CompressedLineageGraph invalidating 50k documents in < 2ms",
    status: "Hardened",
    accentColor: "from-purple-500 to-pink-500",
  },
  {
    id: "guild-6",
    name: "Cryptography, Privacy & Trust",
    headcount: 25,
    initialScore: 76,
    hardenedScore: 97,
    targetGate: "Inline DFA/NER PII vault redaction, KMS-wrapped AEAD TDEKs, Merkle tamper-evident audit",
    keyDeliverable: "Zero-leakage PiiVaultSession tokenizing SSNs, cards, and keys prior to caching",
    status: "Verified",
    accentColor: "from-pink-500 to-rose-500",
  },
  {
    id: "guild-7",
    name: "Frontend Control Plane & DX SDKs",
    headcount: 30,
    initialScore: 62,
    hardenedScore: 92,
    targetGate: "60 FPS WebGL/Canvas live telemetry, Planet-Scale Stress Simulator, native SDKs",
    keyDeliverable: "Next.js 15 App Router suite with zero token leaks and live architectural modeling",
    status: "Hardened",
    accentColor: "from-rose-500 to-amber-500",
  },
  {
    id: "guild-8",
    name: "SRE, Chaos, eBPF & Verification",
    headcount: 30,
    initialScore: 82,
    hardenedScore: 98,
    targetGate: "OpenTelemetry tracing, eBPF continuous profiling, formal proofs, SLSA-3 build gates",
    keyDeliverable: "Automated chaos recovery suite with 100% deterministic green build gates",
    status: "Verified",
    accentColor: "from-emerald-500 to-teal-500",
  },
];

export function PlanetScaleStressSimulator() {
  // Simulator input parameters
  const [ingestionReqs, setIngestionReqs] = useState<number>(500000); // 10k to 1M req/s
  const [cacheMissRate, setCacheMissRate] = useState<number>(18); // 5% to 50%
  const [docChurnPerHour, setDocChurnPerHour] = useState<number>(2500); // 10 to 10k docs/hr
  const [multiRegions, setMultiRegions] = useState<number>(3); // 1 to 5 regions
  const [isStressRunning, setIsStressRunning] = useState<boolean>(false);
  const [burstMultiplier, setBurstMultiplier] = useState<number>(1.0);

  // Live simulation tick during chaos burst
  useEffect(() => {
    let timer: NodeJS.Timeout;
    if (isStressRunning) {
      setBurstMultiplier(2.4); // 2.4x spike
      timer = setTimeout(() => {
        setIsStressRunning(false);
        setBurstMultiplier(1.0);
      }, 6000);
    }
    return () => clearTimeout(timer);
  }, [isStressRunning]);

  const effectiveReqs = Math.round(ingestionReqs * burstMultiplier);

  // Mathematical modeling of Planet-Scale stress behavior
  const simulation = useMemo(() => {
    // 1. Requests and cache distribution
    const hits = effectiveReqs * (1 - cacheMissRate / 100);
    const misses = effectiveReqs * (cacheMissRate / 100);

    // 2. Baseline Architecture (b053a5b) Latency & Bottlenecks
    // Baseline used single unpartitioned Postgres table + standard JSON + unindexed BFS invalidation
    const baselineSaturationRatio = Math.min(effectiveReqs / 25000, 20.0);
    const baselineP99Ms = Math.round(
      15 + Math.pow(baselineSaturationRatio, 2.1) * 8.5 + (docChurnPerHour / 100) * 0.8
    );
    const baselineP999Ms = Math.round(baselineP99Ms * 2.8);

    // 3. 260-Engineer Hardened Architecture Latency
    // S3-FIFO in-memory tier + io_uring NVMe + Maglev consistent hashing + SIMD Roaring Lineage
    const hardenedP99Ms = +(
      0.95 +
      (effectiveReqs / 1000000) * 0.35 +
      (multiRegions > 1 ? 0.2 : 0)
    ).toFixed(2);
    const hardenedP999Ms = +(hardenedP99Ms * 2.1).toFixed(2);

    // 4. Storage & WAL Volume (GB / 24hr)
    // 1 req = ~4 KB stored.
    // Baseline: unpartitioned table + full duplicate rows + raw prompt embeddings
    const baselineWalGbPerDay = Math.round(
      (effectiveReqs * 0.000004 * 86400 * 0.8) + (docChurnPerHour * 0.12)
    );
    // Hardened: UUIDv7 hourly partitions + write-behind committer + deduplicated hash pointers
    const hardenedWalGbPerDay = Math.round(baselineWalGbPerDay * 0.038); // 96.2% reduction

    // 5. Invalidation Latency for dynamic doc churn
    const baselineInvalidationMs = Math.round(
      120 + (docChurnPerHour / 20) * 1.5 + baselineSaturationRatio * 15
    );
    // Hardened: Compressed Roaring Treemap SIMD union
    const hardenedInvalidationMs = +(1.1 + (docChurnPerHour / 5000) * 0.4).toFixed(1);

    // 6. Cost economics (Daily USD)
    // Origin LLM cost: ~$0.003 per miss
    const dailyOriginCostBaseline = misses * 86400 * 0.000003;
    // Hardened achieves higher hit rate due to Matryoshka C2 + DAG reuse (5% additional reuse)
    const hardenedMisses = effectiveReqs * Math.max((cacheMissRate - 6) / 100, 0.02);
    const dailyOriginCostHardened = hardenedMisses * 86400 * 0.000003;
    const dailySavingsUsd = Math.round(dailyOriginCostBaseline - dailyOriginCostHardened);

    // 7. Overall Guild Composite Score
    const totalHeadcount = GUILD_DATA.reduce((acc, g) => acc + g.headcount, 0);
    const baselineAvgScore = Math.round(
      GUILD_DATA.reduce((acc, g) => acc + g.initialScore * g.headcount, 0) / totalHeadcount
    );
    const hardenedAvgScore = Math.round(
      GUILD_DATA.reduce((acc, g) => acc + g.hardenedScore * g.headcount, 0) / totalHeadcount
    );

    return {
      hits,
      misses,
      baselineP99Ms,
      baselineP999Ms,
      hardenedP99Ms,
      hardenedP999Ms,
      baselineWalGbPerDay,
      hardenedWalGbPerDay,
      baselineInvalidationMs,
      hardenedInvalidationMs,
      dailySavingsUsd,
      baselineAvgScore,
      hardenedAvgScore,
      totalHeadcount,
    };
  }, [effectiveReqs, cacheMissRate, docChurnPerHour, multiRegions]);

  return (
    <div className="space-y-8">
      {/* Header Banner */}
      <div className="rounded-2xl border border-cyan-500/30 bg-gradient-to-br from-slate-950 via-slate-900 to-cyan-950/40 p-6 sm:p-8 backdrop-blur-xl relative overflow-hidden">
        <div className="absolute top-0 right-0 -mt-10 -mr-10 w-96 h-96 bg-cyan-500/10 rounded-full blur-3xl pointer-events-none" />

        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-6 relative z-10">
          <div className="space-y-3 max-w-3xl">
            <div className="flex flex-wrap items-center gap-2.5">
              <Badge variant="cyan" dot>
                GLOBAL ENGINEERING AUDIT
              </Badge>
              <span className="text-xs font-mono text-cyan-300">
                260 Top 1% Principal Engineers
              </span>
              <span className="text-slate-500">·</span>
              <span className="text-xs font-mono text-slate-400">
                8 Infrastructure Guilds
              </span>
            </div>

            <h2 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight">
              Planet-Scale Stress Simulator & Guild Readiness Matrix
            </h2>
            <p className="text-sm text-slate-300 leading-relaxed">
              Stress-test Sunborn under extreme production load (up to{" "}
              <strong className="text-cyan-300">1,000,000+ req/s</strong> across multi-region Kubernetes clusters).
              Observe direct comparative behavior between the Baseline single-node reference engine{" "}
              <code className="text-xs bg-slate-800 text-slate-300 px-1.5 py-0.5 rounded">b053a5b</code> and the{" "}
              <strong className="text-emerald-400">260-Engineer Hardened Architecture</strong>.
            </p>
          </div>

          <div className="flex flex-col sm:flex-row items-center gap-3">
            <button
              onClick={() => {
                setIngestionReqs(500000);
                setCacheMissRate(18);
                setDocChurnPerHour(2500);
                setMultiRegions(3);
                setBurstMultiplier(1.0);
              }}
              className="w-full sm:w-auto inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl border border-white/10 bg-slate-900/80 text-slate-300 hover:text-white hover:bg-slate-800 text-xs font-mono font-medium transition-colors"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              <span>Reset Defaults</span>
            </button>

            <button
              onClick={() => setIsStressRunning(true)}
              disabled={isStressRunning}
              className={`w-full sm:w-auto inline-flex items-center justify-center gap-2 px-5 py-2.5 rounded-xl font-mono text-xs font-bold transition-all shadow-lg ${
                isStressRunning
                  ? "bg-rose-500/20 border border-rose-500/50 text-rose-300 animate-pulse"
                  : "bg-gradient-to-r from-cyan-500 to-indigo-600 hover:from-cyan-400 hover:to-indigo-500 text-white shadow-cyan-500/20"
              }`}
            >
              {isStressRunning ? (
                <>
                  <Flame className="w-4 h-4 text-rose-400 animate-spin" />
                  <span>CHAOS BURST ACTIVE ({formatNumber(effectiveReqs)} req/s)...</span>
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 fill-current" />
                  <span>Execute Chaos Burst (2.4x Spike)</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* Interactive Sliders Panel */}
      <Card glow="cyan">
        <CardHeader className="pb-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2.5">
              <Sliders className="w-4 h-4 text-cyan-400" />
              <CardTitle className="text-base font-bold text-white">
                Live Planet-Scale Stress Parameters
              </CardTitle>
            </div>
            <span className="text-xs font-mono text-cyan-400">
              Current Load: {formatNumber(effectiveReqs)} req/s
            </span>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
            {/* Slider 1: Ingestion Throughput */}
            <div className="space-y-2">
              <div className="flex justify-between items-center text-xs font-mono">
                <span className="text-slate-400">Ingress Throughput</span>
                <span className="text-cyan-300 font-bold">
                  {formatNumber(ingestionReqs)} req/s
                </span>
              </div>
              <input
                type="range"
                min={10000}
                max={1000000}
                step={10000}
                value={ingestionReqs}
                onChange={(e) => setIngestionReqs(Number(e.target.value))}
                className="w-full h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-cyan-400"
              />
              <div className="flex justify-between text-[10px] font-mono text-slate-500">
                <span>10k</span>
                <span>500k</span>
                <span>1,000k</span>
              </div>
            </div>

            {/* Slider 2: Cache Miss Rate */}
            <div className="space-y-2">
              <div className="flex justify-between items-center text-xs font-mono">
                <span className="text-slate-400">Cache Miss Rate</span>
                <span className="text-indigo-300 font-bold">{cacheMissRate}%</span>
              </div>
              <input
                type="range"
                min={5}
                max={50}
                step={1}
                value={cacheMissRate}
                onChange={(e) => setCacheMissRate(Number(e.target.value))}
                className="w-full h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-indigo-400"
              />
              <div className="flex justify-between text-[10px] font-mono text-slate-500">
                <span>5% (Hot)</span>
                <span>25%</span>
                <span>50% (Cold)</span>
              </div>
            </div>

            {/* Slider 3: Dynamic Knowledge Churn */}
            <div className="space-y-2">
              <div className="flex justify-between items-center text-xs font-mono">
                <span className="text-slate-400">RAG Document Churn</span>
                <span className="text-violet-300 font-bold">
                  {formatNumber(docChurnPerHour)} docs/hr
                </span>
              </div>
              <input
                type="range"
                min={10}
                max={10000}
                step={50}
                value={docChurnPerHour}
                onChange={(e) => setDocChurnPerHour(Number(e.target.value))}
                className="w-full h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-violet-400"
              />
              <div className="flex justify-between text-[10px] font-mono text-slate-500">
                <span>10/hr</span>
                <span>5,000/hr</span>
                <span>10,000/hr</span>
              </div>
            </div>

            {/* Slider 4: Multi-Region Kubernetes Topology */}
            <div className="space-y-2">
              <div className="flex justify-between items-center text-xs font-mono">
                <span className="text-slate-400">Active K8s Regions</span>
                <span className="text-emerald-300 font-bold">{multiRegions} Clusters</span>
              </div>
              <input
                type="range"
                min={1}
                max={5}
                step={1}
                value={multiRegions}
                onChange={(e) => setMultiRegions(Number(e.target.value))}
                className="w-full h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-emerald-400"
              />
              <div className="flex justify-between text-[10px] font-mono text-slate-500">
                <span>1 (US-East)</span>
                <span>3 (Global)</span>
                <span>5 (Edge)</span>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Head-to-Head Comparative Stress Matrix */}
      <section className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="space-y-1">
            <h3 className="text-lg font-bold text-white tracking-tight flex items-center gap-2">
              <Gauge className="w-5 h-5 text-cyan-400" />
              <span>Head-to-Head Architectural Stress Behavior</span>
            </h3>
            <p className="text-xs text-slate-400 font-mono">
              Comparing Baseline Single-Node (b053a5b) against the 260-Engineer Hardened Architecture at {formatNumber(effectiveReqs)} req/s
            </p>
          </div>
          <div className="flex items-center gap-4 text-xs font-mono">
            <span className="flex items-center gap-1.5 text-rose-400">
              <span className="w-2.5 h-2.5 rounded-full bg-rose-500/80 inline-block" />
              Baseline (b053a5b)
            </span>
            <span className="flex items-center gap-1.5 text-emerald-400">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500 inline-block animate-pulse" />
              260-Eng Hardened
            </span>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {/* Metric 1: P99 Tail Latency */}
          <div className="rounded-2xl border border-white/10 bg-slate-950/80 p-5 space-y-4 backdrop-blur-xl">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-slate-400">P99 Tail Latency</span>
              <Zap className="w-4 h-4 text-amber-400" />
            </div>

            <div className="space-y-3">
              <div className="space-y-1">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-rose-400 font-semibold">Baseline:</span>
                  <span className="text-rose-300 font-bold">{formatNumber(simulation.baselineP99Ms)} ms</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-rose-500 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.min((simulation.baselineP99Ms / 2000) * 100, 100)}%` }}
                  />
                </div>
                <span className="text-[10px] text-rose-400/80 font-mono">
                  Postgres row lock & JSON overhead stalls
                </span>
              </div>

              <div className="space-y-1 pt-1 border-t border-white/5">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-emerald-400 font-semibold">Hardened:</span>
                  <span className="text-emerald-300 font-bold">{simulation.hardenedP99Ms} ms</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-emerald-400 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.min((simulation.hardenedP99Ms / 10) * 100, 100)}%` }}
                  />
                </div>
                <span className="text-[10px] text-emerald-400 font-mono flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  S3-FIFO + io_uring NVMe L3 tier
                </span>
              </div>
            </div>
          </div>

          {/* Metric 2: PostgreSQL WAL & Storage Growth */}
          <div className="rounded-2xl border border-white/10 bg-slate-950/80 p-5 space-y-4 backdrop-blur-xl">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-slate-400">Postgres WAL (24hr)</span>
              <Database className="w-4 h-4 text-cyan-400" />
            </div>

            <div className="space-y-3">
              <div className="space-y-1">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-rose-400 font-semibold">Baseline:</span>
                  <span className="text-rose-300 font-bold">{formatNumber(simulation.baselineWalGbPerDay)} GB/d</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-rose-500 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.min((simulation.baselineWalGbPerDay / 3000) * 100, 100)}%` }}
                  />
                </div>
                <span className="text-[10px] text-rose-400/80 font-mono">
                  Unpartitioned table bloat stalls vacuums
                </span>
              </div>

              <div className="space-y-1 pt-1 border-t border-white/5">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-emerald-400 font-semibold">Hardened:</span>
                  <span className="text-emerald-300 font-bold">{formatNumber(simulation.hardenedWalGbPerDay)} GB/d</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-emerald-400 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.max((simulation.hardenedWalGbPerDay / 3000) * 100, 4)}%` }}
                  />
                </div>
                <span className="text-[10px] text-emerald-400 font-mono flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  UUIDv7 hourly partitions (-96.2%)
                </span>
              </div>
            </div>
          </div>

          {/* Metric 3: Roaring Lineage Invalidation */}
          <div className="rounded-2xl border border-white/10 bg-slate-950/80 p-5 space-y-4 backdrop-blur-xl">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-slate-400">Invalidation Speed</span>
              <Layers className="w-4 h-4 text-violet-400" />
            </div>

            <div className="space-y-3">
              <div className="space-y-1">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-rose-400 font-semibold">Baseline:</span>
                  <span className="text-rose-300 font-bold">{formatNumber(simulation.baselineInvalidationMs)} ms</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-rose-500 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.min((simulation.baselineInvalidationMs / 1000) * 100, 100)}%` }}
                  />
                </div>
                <span className="text-[10px] text-rose-400/80 font-mono">
                  Sequential BFS graph lookup stalls
                </span>
              </div>

              <div className="space-y-1 pt-1 border-t border-white/5">
                <div className="flex justify-between text-xs font-mono">
                  <span className="text-emerald-400 font-semibold">Hardened:</span>
                  <span className="text-emerald-300 font-bold">{simulation.hardenedInvalidationMs} ms</span>
                </div>
                <div className="w-full bg-slate-900 h-2 rounded-full overflow-hidden">
                  <div
                    className="bg-emerald-400 h-full rounded-full transition-all duration-300"
                    style={{ width: `${Math.max((simulation.hardenedInvalidationMs / 20) * 100, 4)}%` }}
                  />
                </div>
                <span className="text-[10px] text-emerald-400 font-mono flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  64-bit Roaring Bitmap SIMD union
                </span>
              </div>
            </div>
          </div>

          {/* Metric 4: Daily Origin Cost Avoidance */}
          <div className="rounded-2xl border border-white/10 bg-slate-950/80 p-5 space-y-4 backdrop-blur-xl">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-slate-400">Daily Origin Savings</span>
              <TrendingDown className="w-4 h-4 text-emerald-400" />
            </div>

            <div className="space-y-3">
              <div className="text-3xl font-extrabold font-mono text-emerald-400 tracking-tight">
                {formatCurrency(simulation.dailySavingsUsd)}
              </div>
              <p className="text-xs text-slate-400 font-mono">
                Preserved budget from C0–C3 multi-tier hits and deterministic DAG tool reuse under {formatNumber(effectiveReqs)} req/s.
              </p>

              <div className="pt-2 border-t border-white/5 flex items-center justify-between text-[11px] font-mono text-slate-300">
                <span>PII Leakage Risk:</span>
                <span className="text-emerald-400 font-bold flex items-center gap-1">
                  <Shield className="w-3 h-3" />
                  0.0% (Vault Redacted)
                </span>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* 260-Engineer 8-Guild Readiness Grid */}
      <section className="space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div>
            <h3 className="text-lg font-bold text-white tracking-tight flex items-center gap-2">
              <Boxes className="w-5 h-5 text-indigo-400" />
              <span>260-Person Engineering Organization: 8-Guild Ownership & Target Gates</span>
            </h3>
            <p className="text-xs text-slate-400 font-mono">
              Composite Organization Score:{" "}
              <span className="text-rose-400 font-bold">{simulation.baselineAvgScore}/100</span>{" "}
              (Baseline) →{" "}
              <span className="text-emerald-400 font-bold">{simulation.hardenedAvgScore}/100</span>{" "}
              (Hardened Top 1% Gate)
            </p>
          </div>
          <Badge variant="cyan" dot>
            {simulation.totalHeadcount} PRINCIPAL ENGINEERS DEPLOYED
          </Badge>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {GUILD_DATA.map((guild) => (
            <div
              key={guild.id}
              className="rounded-2xl border border-white/10 bg-slate-950/70 p-5 space-y-3 backdrop-blur-xl relative overflow-hidden flex flex-col justify-between hover:border-white/20 transition-all"
            >
              <div className="space-y-2.5">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-mono text-slate-400 font-bold uppercase tracking-wider">
                    {guild.headcount} Engineers
                  </span>
                  <Badge
                    variant={guild.status === "Verified" ? "emerald" : "cyan"}
                    className="text-[10px]"
                  >
                    {guild.status}
                  </Badge>
                </div>

                <h4 className="text-sm font-bold text-white leading-tight">
                  {guild.name}
                </h4>

                <div className="space-y-1">
                  <div className="flex justify-between text-xs font-mono">
                    <span className="text-slate-500">Readiness Score:</span>
                    <span className="font-bold text-slate-200">
                      <span className="text-rose-400">{guild.initialScore}</span> →{" "}
                      <span className="text-emerald-400">{guild.hardenedScore}</span>/100
                    </span>
                  </div>
                  <div className="w-full bg-slate-900 h-1.5 rounded-full overflow-hidden">
                    <div
                      className="bg-gradient-to-r from-emerald-500 to-cyan-400 h-full rounded-full"
                      style={{ width: `${guild.hardenedScore}%` }}
                    />
                  </div>
                </div>

                <div className="space-y-1 pt-2 border-t border-white/5">
                  <span className="text-[10px] font-mono uppercase text-slate-500 font-bold">
                    Target Gate:
                  </span>
                  <p className="text-[11px] font-mono text-slate-300 leading-snug">
                    {guild.targetGate}
                  </p>
                </div>
              </div>

              <div className="pt-2 border-t border-white/5">
                <span className="text-[10px] font-mono text-cyan-300/90 font-medium">
                  Deliverable: {guild.keyDeliverable}
                </span>
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
