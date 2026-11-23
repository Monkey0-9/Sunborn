"use client";

import React, { useState, useEffect } from "react";
import { GitBranch, ShieldCheck, CheckCircle2, Lock, Cpu, Database, Activity, AlertCircle } from "lucide-react";
import { sunbornApi } from "@/lib/api";
import { useClusterStore } from "@/lib/cluster-store";

interface DAGNode {
  id: string;
  label: string;
  role: string;
  latencyBound: string;
  status: "active" | "verified" | "degraded";
  detail: string;
}

export function LiveDAGVisualizer() {
  const { isSimulated } = useClusterStore();
  const [selectedNodeId, setSelectedNodeId] = useState<string>("canonicalizer");
  const [readinessData, setReadinessData] = useState<any>(null);
  const [isLive, setIsLive] = useState<boolean>(false);

  useEffect(() => {
    let isMounted = true;
    async function checkLiveStatus() {
      try {
        const ready = await sunbornApi.getReadiness();
        if (isMounted && ready) {
          setReadinessData(ready);
          setIsLive(ready.status === "ready" || ready.status === "ok");
        }
      } catch {
        if (isMounted) {
          setIsLive(false);
        }
      }
    }
    checkLiveStatus();
    const interval = setInterval(checkLiveStatus, 15000);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  const cacheStatus = !isLive ? "degraded" : (readinessData?.components?.cache?.status === "connected" ? "verified" : "degraded");
  const vectorStatus = !isLive ? "degraded" : (readinessData?.components?.vector_store?.status === "connected" ? "verified" : "degraded");
  const dbStatus = !isLive ? "degraded" : (readinessData?.components?.database?.status === "connected" ? "verified" : "degraded");

  const nodes: DAGNode[] = [
    {
      id: "ingress",
      label: "HTTP Ingress",
      role: "Protocol Ingestion",
      latencyBound: "Target: < 0.05 ms",
      status: isLive ? "verified" : "degraded",
      detail: "Terminates OpenAI / Anthropic / Gemini wire protocols; extracts authorization headers and client tenant identity.",
    },
    {
      id: "canonicalizer",
      label: "RFC 8785 Canonicalizer",
      role: "Deterministic Digest",
      latencyBound: "Target: < 0.10 µs",
      status: isLive ? "active" : "degraded",
      detail: "Normalizes Unicode NFC, removes nondeterministic JSON whitespace, and executes cryptographic SHA-256 digest creation.",
    },
    {
      id: "l1_exact",
      label: "L1/L2 Tiered Cache",
      role: "Sub-Microsecond Tier",
      latencyBound: "Bench: 0.30 µs P50",
      status: cacheStatus,
      detail: "Multi-tier cache architecture combining L1 in-process memory and L2 Dragonfly/Redis store. Microbenchmark throughput: 1.64M req/sec in-process.",
    },
    {
      id: "guardrails",
      label: "AST Polarity Guardrails",
      role: "Structural Anti-Hallucination",
      latencyBound: "Target: < 0.20 ms",
      status: isLive ? "verified" : "degraded",
      detail: "Evaluates numeric shifts, antonyms, and negation inversions, guaranteeing 0 false positives across hard traps.",
    },
    {
      id: "semantic_hnsw",
      label: "Qdrant HNSW & Conformal",
      role: "Semantic Risk Control",
      latencyBound: "Target: < 1.50 ms",
      status: vectorStatus,
      detail: "C SIMD vector similarity search coupled with non-exchangeable conformal prediction bounding semantic risk.",
    },
    {
      id: "merkle_proof",
      label: "Merkle Attestation",
      role: "Cryptographic Certificate",
      latencyBound: "Target: < 0.08 ms",
      status: isLive ? "verified" : "degraded",
      detail: "Issues Ed25519-signed reuse certificates with Merkle tree inclusion proofs binding inputs, outputs, and docs.",
    },
  ];

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) || nodes[1];

  return (
    <section
      aria-label="Execution Pipeline and Merkle DAG Inspector"
      className="p-5 rounded-xl bg-slate-900/60 border border-white/10 backdrop-blur-md"
    >
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-base font-semibold text-white flex items-center gap-2">
            <GitBranch className="w-4 h-4 text-violet-400" aria-hidden="true" />
            Execution Pipeline & Merkle DAG
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            Architectural SLA design target bounds. Live request latency is measured in the Telemetry ribbon.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {isSimulated ? (
            <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/30 flex items-center gap-1 font-semibold">
              <AlertCircle className="w-3 h-3 text-amber-400" />
              SPECIFICATION TARGETS
            </span>
          ) : isLive ? (
            <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              SLA Bound Targets
            </span>
          ) : (
            <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-rose-500/15 text-rose-300 border border-rose-500/30 flex items-center gap-1 font-semibold">
              <AlertCircle className="w-3 h-3 text-rose-400" />
              OFFLINE / UNAVAILABLE
            </span>
          )}
          <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-violet-500/10 text-violet-300 border border-violet-500/20">
            Ed25519 Attested
          </span>
        </div>
      </div>

      {/* SVG Pipeline Representation */}
      <div className="p-4 rounded-lg bg-slate-950/80 border border-white/10 overflow-x-auto">
        <div className="flex items-center justify-between min-w-[580px] gap-2 py-2">
          {nodes.map((node, index) => {
            const isSelected = node.id === selectedNodeId;
            return (
              <React.Fragment key={node.id}>
                <button
                  type="button"
                  onClick={() => setSelectedNodeId(node.id)}
                  aria-pressed={isSelected}
                  className={`flex flex-col items-center p-2 rounded-lg border transition-all text-center focus:outline-none focus:ring-2 focus:ring-violet-400 ${
                    isSelected
                      ? "bg-violet-500/20 border-violet-400 text-white shadow-lg shadow-violet-500/10"
                      : "bg-slate-900/80 border-white/10 text-slate-400 hover:text-slate-200 hover:border-white/20"
                  }`}
                >
                  <span className="text-[10px] font-mono text-slate-400 uppercase tracking-wider">
                    Step 0{index + 1}
                  </span>
                  <span className="text-xs font-semibold mt-0.5">{node.label}</span>
                  <span className="text-[9px] font-mono text-cyan-400 mt-1">SLA {node.latencyBound}</span>
                </button>
                {index < nodes.length - 1 && (
                  <div className="text-slate-600 font-mono text-xs px-1 select-none" aria-hidden="true">
                    ──▶
                  </div>
                )}
              </React.Fragment>
            );
          })}
        </div>
      </div>

      {/* Node Detail Inspector */}
      <div className="mt-4 p-3.5 rounded-lg bg-slate-950/60 border border-white/10 text-xs">
        <div className="flex items-center justify-between border-b border-white/5 pb-2">
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-emerald-400" aria-hidden="true" />
            <span className="font-semibold text-white">{selectedNode.label}</span>
            <span className="text-slate-400 font-mono">({selectedNode.role})</span>
          </div>
          <span className="text-cyan-400 font-mono font-medium">Target: {selectedNode.latencyBound}</span>
        </div>
        <p className="mt-2 text-slate-300 font-sans leading-relaxed text-[12px]">
          {selectedNode.detail}
        </p>
      </div>
    </section>
  );
}
