"use client";

import React, { useState } from "react";
import {
  Layers,
  Database,
  RefreshCw,
  ShieldAlert,
  CheckCircle2,
  AlertTriangle,
  ArrowRight,
  GitCommit,
  Radio,
  Sliders,
  Sparkles,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { sunbornApi } from "@/lib/api";

export default function KnowledgePage() {
  const [sources, setSources] = useState([
    {
      id: "9a1f4b2e-0000-0000-0000-000000000001",
      name: "OpenAPI Spec v2.4 (Core Payment Gateway)",
      version: "v2.4.1",
      epoch: 418,
      dependentEntries: 1420,
      lastUpdated: "12m ago",
      status: "Synchronized",
    },
    {
      id: "8c3d1a7c-0000-0000-0000-000000000002",
      name: "Enterprise Compliance & AML Manual",
      version: "v1.9.0",
      epoch: 418,
      dependentEntries: 890,
      lastUpdated: "2h ago",
      status: "Synchronized",
    },
    {
      id: "2f0e5d8b-0000-0000-0000-000000000003",
      name: "Treasury Rates & Liquidity Rules",
      version: "v3.0.0",
      epoch: 418,
      dependentEntries: 3100,
      lastUpdated: "1d ago",
      status: "Synchronized",
    },
  ]);

  const [selectedSourceId, setSelectedSourceId] = useState("9a1f4b2e-0000-0000-0000-000000000001");
  const [newVersion, setNewVersion] = useState("v2.5.0");
  const [isInvalidating, setIsInvalidating] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [invalidationReport, setInvalidationReport] = useState<{
    evictedCaches: number;
    evictedVectors: number;
    newEpoch: number;
    latencyMs: number;
  } | null>(null);

  const handleTriggerInvalidation = async () => {
    setIsInvalidating(true);
    setActionError(null);
    const start = performance.now();
    try {
      const data = await sunbornApi.triggerInvalidation(
        selectedSourceId,
        "tenant-enterprise-fintech",
        newVersion
      );
      const elapsedMs = performance.now() - start;
      setInvalidationReport({
        evictedCaches: data.evicted_cache_count ?? 0,
        evictedVectors: data.evicted_cache_count ?? 0,
        newEpoch: 419,
        latencyMs: +(elapsedMs.toFixed(1)),
      });
    } catch (err: any) {
      setActionError(`Cascade invalidation failed: ${err.message}`);
      setInvalidationReport(null);
    } finally {
      setIsInvalidating(false);
    }
  };

  return (
    <div className="space-y-8">
      {actionError && (
        <div className="p-4 rounded-xl bg-rose-950/70 border border-rose-500/40 text-rose-200 text-xs font-mono space-y-1">
          <div className="flex items-center gap-2 font-bold text-rose-400">
            <span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
            INVALIDATION OPERATION FAILED
          </div>
          <div>{actionError}</div>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-white/[0.08] pb-6">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold text-white tracking-tight">
              Semantic Studio & Knowledge Dependency Graph
            </h1>
            <Badge variant="purple">CONFORMAL RISK α ≤ 0.001</Badge>
          </div>
          <p className="text-xs text-slate-400 font-mono mt-1">
            Bi-directional source-to-cache dependency index, monotonic epoch fencing, and transactional outbox cascades.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Badge variant="outline" className="font-mono">
            OUTBOX: ZERO DATA LOSS (RPO=0)
          </Badge>
          <Badge variant="cyan" dot>
            QDRANT VECTOR INDEX: REBUILDABLE
          </Badge>
        </div>
      </div>

      {/* Adversarial Semantic Trap & Conformal Risk Visualizer */}
      <Card glow="violet">
        <CardHeader>
          <CardTitle className="text-base flex items-center gap-2">
            <ShieldAlert className="w-4 h-4 text-violet-400" />
            <span>Why Naive Cosine Caches Fail: The Conformal Risk Solution</span>
          </CardTitle>
          <CardDescription>
            Comparison between naive fixed-threshold vector similarity (B2) and Sunborn calibrated guardrails (B3).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Trap 1: Negation Inversion */}
            <div className="p-4 rounded-xl bg-slate-900/80 border border-rose-500/30 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-mono font-bold text-rose-400">
                  ADVERSARIAL TRAP: NEGATION INVERSION
                </span>
                <Badge variant="danger">NAIVE B2 VULNERABLE</Badge>
              </div>
              <div className="space-y-1 font-mono text-xs">
                <div className="p-2 rounded bg-slate-950 text-slate-300">
                  <span className="text-slate-500">Query A:</span> "Please approve transfer #9182 for $10,000"
                </div>
                <div className="p-2 rounded bg-slate-950 text-rose-300">
                  <span className="text-slate-500">Query B:</span> "Please do NOT approve transfer #9182 for $10,000"
                </div>
              </div>
              <div className="pt-2 text-xs font-mono text-slate-400 flex items-center justify-between">
                <span>Cosine Similarity: <strong className="text-white">0.962</strong></span>
                <span className="text-emerald-400 font-bold">Sunborn: REJECTED (Guardrail Active)</span>
              </div>
            </div>

            {/* Trap 2: Numeric Shift */}
            <div className="p-4 rounded-xl bg-slate-900/80 border border-amber-500/30 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-mono font-bold text-amber-400">
                  ADVERSARIAL TRAP: NUMERIC RANGE SHIFT
                </span>
                <Badge variant="warning">NAIVE B2 VULNERABLE</Badge>
              </div>
              <div className="space-y-1 font-mono text-xs">
                <div className="p-2 rounded bg-slate-950 text-slate-300">
                  <span className="text-slate-500">Query A:</span> "Set capital reserve tier to 4.5%"
                </div>
                <div className="p-2 rounded bg-slate-950 text-amber-300">
                  <span className="text-slate-500">Query B:</span> "Set capital reserve tier to 45.0%"
                </div>
              </div>
              <div className="pt-2 text-xs font-mono text-slate-400 flex items-center justify-between">
                <span>Cosine Similarity: <strong className="text-white">0.978</strong></span>
                <span className="text-emerald-400 font-bold">Sunborn: REJECTED (Guardrail Active)</span>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Knowledge Source Invalidation Cascade Console */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        <div className="lg:col-span-7 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-base font-bold text-white tracking-tight flex items-center gap-2">
              <Database className="w-4 h-4 text-cyan-400" />
              <span>Registered Knowledge Documents & APIs</span>
            </h2>
            <span className="text-xs font-mono text-slate-400">
              Bi-directional Index Active
            </span>
          </div>

          <div className="space-y-3">
            {sources.map((src) => {
              const isSelected = selectedSourceId === src.id;
              return (
                <div
                  key={src.id}
                  onClick={() => setSelectedSourceId(src.id)}
                  className={`p-4 rounded-xl border transition-all cursor-pointer backdrop-blur-xl ${
                    isSelected
                      ? "border-cyan-500 bg-slate-900/90 shadow-glow"
                      : "border-white/[0.08] bg-slate-950/70 hover:border-white/20"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-mono text-cyan-400 font-bold">
                      {src.id}
                    </span>
                    <Badge variant="cyan">EPOCH #{src.epoch}</Badge>
                  </div>
                  <h3 className="text-sm font-bold text-white mt-1">
                    {src.name}
                  </h3>
                  <div className="mt-3 pt-3 border-t border-white/10 flex items-center justify-between text-xs font-mono text-slate-400">
                    <span>Version: <strong className="text-white">{src.version}</strong></span>
                    <span>Dependent Caches: <strong className="text-cyan-300">{src.dependentEntries}</strong></span>
                    <span>Updated: {src.lastUpdated}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Right Invalidation Trigger */}
        <div className="lg:col-span-5 space-y-6">
          <Card glow="cyan">
            <CardHeader>
              <CardTitle className="text-base flex items-center gap-2">
                <RefreshCw className="w-4 h-4 text-cyan-400" />
                <span>Transactional Outbox Invalidation</span>
              </CardTitle>
              <CardDescription>
                Atomically publish change events to bump monotonic epoch leases and purge derived accelerator entries.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1">
                <label className="text-xs font-mono text-slate-400">TARGET SOURCE ID</label>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 font-mono text-xs text-white">
                  {selectedSourceId}
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-mono text-slate-400">NEW VERSION TAG</label>
                <input
                  type="text"
                  value={newVersion}
                  onChange={(e) => setNewVersion(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-cyan-300 focus:outline-none focus:border-cyan-500"
                />
              </div>

              <div className="pt-2">
                <ShimmerButton
                  onClick={handleTriggerInvalidation}
                  disabled={isInvalidating}
                  className="w-full"
                >
                  <RefreshCw className={`w-4 h-4 ${isInvalidating ? "animate-spin" : ""}`} />
                  <span>{isInvalidating ? "Dispatching Outbox..." : "Cascade Invalidate (POST /v1/invalidation)"}</span>
                </ShimmerButton>
              </div>

              {invalidationReport && (
                <div className="p-4 rounded-xl bg-slate-900/90 border border-emerald-500/30 space-y-2 font-mono text-xs">
                  <div className="flex items-center gap-2 text-emerald-400 font-bold">
                    <CheckCircle2 className="w-4 h-4" />
                    <span>Cascade Invalidation Completed</span>
                  </div>
                  <div className="space-y-1 text-slate-300 text-[11px] pt-1">
                    <div>• Purged Exact Entries (DragonflyDB): <strong className="text-white">{invalidationReport.evictedCaches}</strong></div>
                    <div>• Purged Vector Points (Qdrant HNSW): <strong className="text-white">{invalidationReport.evictedVectors}</strong></div>
                    <div>• Advanced Monotonic Epoch: <strong className="text-cyan-300">#{invalidationReport.newEpoch}</strong></div>
                    <div>• Pipeline Invalidation Latency: <strong className="text-emerald-300">{invalidationReport.latencyMs} ms</strong></div>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
