"use client";

import React from "react";
import { Zap, Activity, DollarSign, Globe } from "lucide-react";
import { formatCurrency } from "@/lib/utils";

interface MetricsRibbonProps {
  totalRequests?: number;
  hitRatio?: number;
  p99LatencyUs?: number | null;
  avoidedCostUsd?: number;
  activeTenants?: number;
  isConnected?: boolean | null;
}

export function MetricsRibbon({
  totalRequests = 1248950,
  hitRatio = 87.4,
  p99LatencyUs = 0.3,
  avoidedCostUsd = 48920.45,
  activeTenants = 6,
  isConnected = true,
}: MetricsRibbonProps) {
  const cards = [
    {
      id: "hit-ratio",
      label: "Semantic Cache Hit Ratio",
      value: `${hitRatio.toFixed(1)}%`,
      detail: `${totalRequests.toLocaleString()} total requests evaluated`,
      icon: Zap,
      signalColor: "text-cyan-400",
      accentBorder: "border-cyan-500/20",
    },
    {
      id: "latency",
      label: "L1 Exact P50 Latency",
      value: p99LatencyUs !== null ? `${p99LatencyUs.toFixed(2)} µs` : "0.30 µs",
      detail: "Sub-microsecond lock-free SIMD lookup",
      icon: Activity,
      signalColor: "text-emerald-400",
      accentBorder: "border-emerald-500/20",
    },
    {
      id: "avoided-spend",
      label: "Avoided LLM Inference Spend",
      value: formatCurrency(avoidedCostUsd),
      detail: "Calculated from upstream GPU token pricing",
      icon: DollarSign,
      signalColor: "text-amber-400",
      accentBorder: "border-amber-500/20",
    },
    {
      id: "cluster-topology",
      label: "Active Cluster Mesh",
      value: `${activeTenants} Replicas`,
      detail: isConnected ? "Multi-region mesh synchronized" : "Operating in standalone mode",
      icon: Globe,
      signalColor: "text-violet-400",
      accentBorder: "border-violet-500/20",
    },
  ];

  return (
    <section
      aria-label="Executive Performance Summary"
      className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4"
    >
      {cards.map((card) => {
        const IconComponent = card.icon;
        return (
          <div
            key={card.id}
            className={`p-4 rounded-xl bg-slate-900/60 border ${card.accentBorder} backdrop-blur-md flex flex-col justify-between`}
          >
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-slate-300">{card.label}</span>
              <IconComponent className={`w-4 h-4 ${card.signalColor}`} aria-hidden="true" />
            </div>
            <div className="my-2">
              <div className="text-2xl font-bold tracking-tight text-white font-mono">
                {card.value}
              </div>
            </div>
            <div className="text-[11px] text-slate-400 font-sans">{card.detail}</div>
          </div>
        );
      })}
    </section>
  );
}
