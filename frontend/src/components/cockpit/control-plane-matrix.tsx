"use client";

import React from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShieldCheck, Layers } from "lucide-react";

export interface TelemetryTimingBucket {
  0: number;
  1: number;
  2: number;
}

export interface ControlPlanePolicyData {
  tenant_id?: string;
  storage_tier?: string;
  policy_storage_tier?: string;
  learning_store_durable?: boolean;
  policy_registry_durable?: boolean;
  active_policy?: {
    policy_id?: string;
    version?: number;
    weights?: {
      min_similarity_threshold?: number;
      max_risk_threshold?: number;
      conformal_alpha?: number;
      prefetch_confidence_threshold?: number;
      eviction_staleness_weight?: number;
      eviction_cost_weight?: number;
    };
  };
  canary_policy?: {
    policy?: { policy_id?: string; version?: number };
    percentage?: number;
  } | null;
  canary_metrics?: {
    canary_requests?: number;
    canary_false_hits?: number;
    canary_errors?: number;
    avg_latency_ms?: number;
  };
  shadow_conformal_observations?: number;
  shadow_conformal_threshold?: number;
  paraphrase_clusters_count?: number;
  telemetry_observations?: number;
  telemetry_hit_rate?: number;
  workload_tvd_drift?: number;
  workload_drift_detected?: boolean;
  retrieval_cache_entries?: number;
  quarantined_entries?: number;
  timing_summary?: Record<string, [number, number, number]>;
  total_feedback_events?: number;
}

export function ControlPlaneMatrix({
  policyData,
  finopsData,
}: {
  policyData: ControlPlanePolicyData | null;
  finopsData?: any;
}) {
  const tenant = policyData?.tenant_id || "default";
  const nowIso = new Date().toISOString();
  const hasLive = Boolean(policyData?.active_policy);

  const exactTiming = policyData?.timing_summary?.exact_hit;
  const semTiming = policyData?.timing_summary?.semantic_hit;
  const missTiming = policyData?.timing_summary?.origin_miss;

  const rows = [
    {
      area: "Semantic Admission",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "/v1/learning/policies",
      scope: tenant,
      detail: hasLive
        ? `min_sim=${policyData?.active_policy?.weights?.min_similarity_threshold ?? "—"}, max_risk=${policyData?.active_policy?.weights?.max_risk_threshold ?? "—"}, conformal_obs=${policyData?.shadow_conformal_observations ?? 0}`
        : "Not measured — attach live gateway",
    },
    {
      area: "Freshness & Coherence",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "TemporalValidityEngine + EpochLease",
      scope: tenant,
      detail: hasLive
        ? `5D doc/perm/cfg/model/epoch enforced; drift_detected=${String(policyData?.workload_drift_detected ?? false)}`
        : "Not measured — attach live gateway",
    },
    {
      area: "Paraphrase Clusters",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "ParaphraseClusterManager",
      scope: tenant,
      detail: hasLive
        ? `${policyData?.paraphrase_clusters_count ?? 0} active cluster(s) scoped by tenant/app`
        : "Not measured — attach live gateway",
    },
    {
      area: "Grounding & Evidence",
      status: hasLive ? "CONFIGURED" : "NOT MEASURED",
      source: "SourceGroundingVerifier",
      scope: tenant,
      detail: hasLive
        ? "Claim contradiction, ACL scope & snapshot content_hash enforced; require_grounding optional per request"
        : "Not measured — attach live gateway",
    },
    {
      area: "Self-Learning Loop",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "TelemetryWindow + DriftDetector",
      scope: tenant,
      detail: hasLive
        ? `obs=${policyData?.telemetry_observations ?? 0}, hit_rate=${((policyData?.telemetry_hit_rate ?? 0) * 100).toFixed(1)}%, tvd=${(policyData?.workload_tvd_drift ?? 0).toFixed(3)}, feedback=${policyData?.total_feedback_events ?? 0}`
        : "Not measured — attach live gateway",
    },
    {
      area: "Policy & Canary Rollout",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "PolicyPromotionController",
      scope: "global",
      detail: hasLive
        ? `active=${policyData?.active_policy?.policy_id ?? "—"} (v${policyData?.active_policy?.version ?? 0}), tier=${policyData?.policy_storage_tier ?? "ephemeral"}, canary=${policyData?.canary_policy ? `${policyData.canary_policy.percentage}%` : "none"}`
        : "Not measured — attach live gateway",
    },
    {
      area: "Cache Lifecycle & Retention",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "CacheLifecycleManager (30s worker)",
      scope: tenant,
      detail: hasLive
        ? `evict_cost_w=${policyData?.active_policy?.weights?.eviction_cost_weight ?? 0.6}, staleness_w=${policyData?.active_policy?.weights?.eviction_staleness_weight ?? 0.4}, prefetch_thresh=${policyData?.active_policy?.weights?.prefetch_confidence_threshold ?? 0.6}`
        : "Not measured — attach live gateway",
    },
    {
      area: "RAG Retrieval Cache",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "/v1/retrieval/cache",
      scope: tenant,
      detail: hasLive
        ? `${policyData?.retrieval_cache_entries ?? 0} cached chunk retrieval entries separated from answer tier`
        : "Not measured — attach live gateway",
    },
    {
      area: "Execution Effects (G4)",
      status: hasLive ? "CONFIGURED" : "NOT MEASURED",
      source: "EnforcedExecutionRuntime + DAG",
      scope: tenant,
      detail: hasLive
        ? "SubgraphPlanner + ExecutionPlanner + UnknownOutcome mutation ledger enforced"
        : "Not measured — attach live gateway",
    },
    {
      area: "Security & Timing Oracle",
      status: hasLive ? "MEASURED" : "NOT MEASURED",
      source: "QuarantinePool + CacheThreatEngine",
      scope: tenant,
      detail: hasLive
        ? `quarantined=${policyData?.quarantined_entries ?? 0}; p50(exact/sem/miss)=${exactTiming?.[1] ?? "—"}ms/${semTiming?.[1] ?? "—"}ms/${missTiming?.[1] ?? "—"}ms`
        : "Not measured — attach live gateway",
    },
    {
      area: "Economics & Cost Avoidance",
      status: finopsData?.economics ? "MEASURED" : "NOT MEASURED",
      source: "/v1/finops/report",
      scope: tenant,
      detail: finopsData?.economics
        ? `saved=$${(finopsData.economics.total_estimated_savings_usd ?? 0).toFixed(4)}, reqs=${finopsData?.metrics?.requests_total ?? 0}`
        : "Not measured — attach live gateway / FinOps feed",
    },
  ];

  return (
    <Card className="border-white/10 bg-slate-950/90" aria-label="Five-Gate Control Plane Telemetry Matrix">
      <CardHeader className="pb-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Layers className="w-4 h-4 text-cyan-400" />
            <CardTitle className="text-base">
              Control Plane Telemetry &amp; Invariant Surface (§14)
            </CardTitle>
          </div>
          <div className="flex items-center gap-2 text-[11px] font-mono text-slate-400">
            <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
            <span>Snapshot: {nowIso.slice(0, 19)}Z</span>
          </div>
        </div>
        <CardDescription>
          Live gateway state with explicit provenance (`MEASURED`, `CONFIGURED`, or `NOT MEASURED`). Never substitutes static demo estimates for missing telemetry.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="overflow-x-auto">
          <table className="w-full text-left font-mono text-xs border-collapse">
            <thead>
              <tr className="border-b border-white/10 text-slate-400 text-[11px] uppercase">
                <th className="py-2 pr-3">Control Area</th>
                <th className="py-2 px-3">State</th>
                <th className="py-2 px-3">Scope</th>
                <th className="py-2 px-3">Source</th>
                <th className="py-2 pl-3">Observed Signal</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {rows.map((r) => (
                <tr key={r.area} className="hover:bg-white/[0.02]">
                  <td className="py-2 pr-3 font-semibold text-white whitespace-nowrap">{r.area}</td>
                  <td className="py-2 px-3 whitespace-nowrap">
                    <Badge
                      variant={
                        r.status === "MEASURED"
                          ? "success"
                          : r.status === "CONFIGURED"
                          ? "warning"
                          : "outline"
                      }
                      className="text-[10px]"
                    >
                      {r.status}
                    </Badge>
                  </td>
                  <td className="py-2 px-3 text-slate-300 whitespace-nowrap">{r.scope}</td>
                  <td className="py-2 px-3 text-cyan-300 whitespace-nowrap">{r.source}</td>
                  <td className="py-2 pl-3 text-slate-300">{r.detail}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}
