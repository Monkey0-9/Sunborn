"use client";

import React, { useEffect, useState } from "react";
import { Globe, Radio, AlertCircle, Server } from "lucide-react";
import { sunbornApi } from "@/lib/api";
import { extractVectorClockEpoch } from "@/lib/utils";
import { useClusterStore } from "@/lib/cluster-store";

type TelemetryState = "LIVE" | "PARTIAL" | "UNAVAILABLE";

interface ClusterRegionNode {
  id: string;
  region: string;
  role: string;
  epoch: number;
  status: "SYNCHRONIZED" | "LEADER" | "ONLINE";
  /** Measured RTT, or null when this API does not expose it. */
  latencyMs: number | null;
  telemetry: TelemetryState;
}

export function ClusterMeshStatus() {
  const { circuitBreaker, vectorEpoch: storeEpoch, isSimulated } = useClusterStore();
  const [epochNumber, setEpochNumber] = useState<number>(storeEpoch || 0);
  const [clusterState, setClusterState] = useState<string>("UNKNOWN");
  const [activeNodeId, setActiveNodeId] = useState<string>("local");
  const [activeRegion, setActiveRegion] = useState<string>("unknown");
  const [nodesActive, setNodesActive] = useState<number>(1);
  const [telemetryState, setTelemetryState] = useState<TelemetryState>("UNAVAILABLE");

  useEffect(() => {
    let isMounted = true;
    async function fetchCluster() {
      try {
        const [statusRes, syncRes] = await Promise.allSettled([
          sunbornApi.getClusterStatus(),
          sunbornApi.getClusterSyncStatus(),
        ]);
        if (!isMounted) return;

        if (statusRes.status === "fulfilled" && statusRes.value) {
          const status = statusRes.value;
          setEpochNumber(extractVectorClockEpoch(status.vector_clock));
          if (status.cluster_state) setClusterState(status.cluster_state);
          if (status.node_id) setActiveNodeId(status.node_id);
          if (status.region) setActiveRegion(status.region);
        }

        if (syncRes.status === "fulfilled" && syncRes.value) {
          const sync = syncRes.value;
          if (sync.cluster_state) setClusterState(sync.cluster_state);
          // Only trust a numeric count the gateway attests to.
          if (typeof sync.nodes_active === "number") {
            setNodesActive(sync.nodes_active);
          }
          if (sync.region) setActiveRegion(sync.region);
          if (sync.node_id) setActiveNodeId(sync.node_id);
          // The gateway reports local-process telemetry only; there is no
          // per-peer region or RTT series to chart.
          setTelemetryState(sync.replica_telemetry === "unavailable" ? "PARTIAL" : "LIVE");
        }
      } catch {
        if (isMounted) setTelemetryState("UNAVAILABLE");
      }
    }
    fetchCluster();
    return () => {
      isMounted = false;
    };
  }, []);

  const activeState = circuitBreaker === "OPEN" ? "DEGRADED_CIRCUIT_OPEN" : clusterState;

  // Only nodes this process can actually attest to are rendered. No synthetic
  // regions, replicas, or latency numbers are ever invented here.
  const nodes: ClusterRegionNode[] = [
    {
      id: activeNodeId,
      region: `${activeRegion} (this process)`,
      role: "Authoritative Gateway",
      epoch: epochNumber,
      status: "LEADER",
      latencyMs: null,
      telemetry: telemetryState,
    },
  ];

  const peerCount = Math.max(0, nodesActive - 1);

  return (
    <section
      aria-label="Cluster Topology and Replication Status"
      className="p-5 rounded-xl bg-slate-900/60 border border-white/10 backdrop-blur-md"
    >
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-2 mb-4">
        <div>
          <h2 className="text-base font-semibold text-white flex items-center gap-2">
            <Globe className="w-4 h-4 text-emerald-400" aria-hidden="true" />
            Cluster Topology &amp; Replication
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            Values come from the gateway&apos;s own status endpoint. Per-peer region and RTT
            telemetry is not exposed and is shown as unavailable rather than estimated.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-xs font-mono">
          <span
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full border ${
              circuitBreaker === "OPEN"
                ? "bg-rose-500/10 text-rose-300 border-rose-500/30"
                : "bg-emerald-500/10 text-emerald-300 border-emerald-500/20"
            }`}
          >
            <Radio
              className={`w-3 h-3 animate-pulse ${
                circuitBreaker === "OPEN" ? "text-rose-400" : "text-emerald-400"
              }`}
              aria-hidden="true"
            />
            {activeState}
          </span>
          {isSimulated && (
            <span className="px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/30 text-[10px] font-semibold">
              SIMULATED
            </span>
          )}
          {nodesActive <= 1 ? (
            <span className="px-2 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-500/30 text-[10px] font-semibold">
              Single-Node
            </span>
          ) : (
            <span className="px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 text-[10px] font-semibold">
              {nodesActive} Nodes Active (aggregate)
            </span>
          )}
          <span className="px-2.5 py-1 rounded-full bg-slate-800 text-slate-300 border border-white/10">
            Epoch #{epochNumber}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
        {nodes.map((node) => (
          <div
            key={node.id}
            className="p-3 rounded-lg bg-slate-950/70 border border-white/10 flex flex-col justify-between"
          >
            <div>
              <div className="flex items-center justify-between text-[11px] mb-1 gap-2">
                <span className="font-semibold text-white truncate">{node.region}</span>
                <span className="font-mono text-[9px] px-1.5 py-0.5 rounded font-semibold bg-cyan-500/20 text-cyan-300 border border-cyan-500/30">
                  {node.status}
                </span>
              </div>
              <div className="text-[11px] text-slate-400 font-mono">{node.role}</div>
            </div>

            <div className="mt-3 pt-2 border-t border-white/5 flex items-center justify-between text-[10px] font-mono text-slate-400">
              <span>
                RTT: {node.latencyMs === null ? "unavailable" : `${node.latencyMs}ms`}
              </span>
              <span
                className={
                  node.telemetry === "LIVE"
                    ? "text-emerald-300"
                    : node.telemetry === "PARTIAL"
                      ? "text-amber-300"
                      : "text-slate-500"
                }
              >
                {node.telemetry}
              </span>
            </div>
          </div>
        ))}

        {peerCount > 0 && (
          <div className="p-3 rounded-lg bg-slate-950/40 border border-dashed border-white/10 flex flex-col justify-between">
            <div>
              <div className="flex items-center justify-between text-[11px] mb-1 gap-2">
                <span className="font-semibold text-slate-300 truncate flex items-center gap-1">
                  <Server className="w-3 h-3" aria-hidden="true" />
                  {peerCount} peer node{peerCount === 1 ? "" : "s"}
                </span>
                <span className="font-mono text-[9px] px-1.5 py-0.5 rounded font-semibold bg-amber-500/15 text-amber-300 border border-amber-500/30">
                  PARTIAL
                </span>
              </div>
              <div className="text-[11px] text-slate-500 font-mono">
                Per-node region and RTT are not exposed by this API.
              </div>
            </div>
            <div className="mt-3 pt-2 border-t border-white/5 flex items-center gap-1 text-[10px] font-mono text-slate-500">
              <AlertCircle className="w-3 h-3" aria-hidden="true" />
              No synthetic replica data
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
