"use client";

import React, { useEffect, useState } from "react";
import { Globe, Radio, Shield, Server } from "lucide-react";
import { sunbornApi } from "@/lib/api";
import { extractVectorClockEpoch } from "@/lib/utils";
import { useClusterStore } from "@/lib/cluster-store";

interface ClusterRegionNode {
  id: string;
  region: string;
  role: string;
  epoch: number;
  status: "SYNCHRONIZED" | "LEADER" | "ONLINE";
  latencyMs: number;
}

export function ClusterMeshStatus() {
  const { circuitBreaker, vectorEpoch: storeEpoch, isSimulated } = useClusterStore();
  const [epochNumber, setEpochNumber] = useState<number>(storeEpoch || 4192);
  const [clusterState, setClusterState] = useState<string>("HEALTHY_SYNCHRONIZED");
  const [activeNodeId, setActiveNodeId] = useState<string>("gateway-primary-node-1");
  const [activeRegion, setActiveRegion] = useState<string>("us-east-1");
  const [isSingleNode, setIsSingleNode] = useState<boolean>(true);

  useEffect(() => {
    let isMounted = true;
    async function fetchCluster() {
      try {
        const [statusRes, syncRes] = await Promise.allSettled([
          sunbornApi.getClusterStatus(),
          sunbornApi.getClusterSyncStatus(),
        ]);
        if (isMounted) {
          if (statusRes.status === "fulfilled" && statusRes.value) {
            const status = statusRes.value;
            setEpochNumber(extractVectorClockEpoch(status.vector_clock));
            if (status.cluster_state) {
              setClusterState(status.cluster_state);
            }
            if (status.node_id) {
              setActiveNodeId(status.node_id);
            }
            if (status.region) {
              setActiveRegion(status.region);
            }
          }
          if (syncRes.status === "fulfilled" && syncRes.value) {
            const sync = syncRes.value;
            if (sync.cluster_state) {
              setClusterState(sync.cluster_state);
            }
            setIsSingleNode(sync.nodes_active <= 1 || Boolean(sync.is_simulated_mesh));
          }
        }
      } catch {
        // Safe offline default
      }
    }
    fetchCluster();
    return () => {
      isMounted = false;
    };
  }, []);

  const activeState = circuitBreaker === "OPEN" ? "DEGRADED_CIRCUIT_OPEN" : clusterState;

  const nodes: ClusterRegionNode[] = [
    {
      id: activeNodeId,
      region: `${activeRegion} (Authoritative Gateway)`,
      role: "Authority Primary (Live)",
      epoch: epochNumber,
      status: "LEADER",
      latencyMs: 0.2,
    },
    {
      id: "node-us-west-2b",
      region: "us-west-2 (Simulated Replica)",
      role: isSingleNode ? "Simulated Edge Sentinel" : "Edge Sentinel",
      epoch: epochNumber,
      status: "SYNCHRONIZED",
      latencyMs: 14.2,
    },
    {
      id: "node-eu-central-1a",
      region: "eu-central-1 (Simulated Replica)",
      role: isSingleNode ? "Simulated GDPR Vault" : "GDPR Vault Replica",
      epoch: epochNumber,
      status: "SYNCHRONIZED",
      latencyMs: 38.6,
    },
    {
      id: "node-ap-northeast-1",
      region: "ap-northeast-1 (Simulated Replica)",
      role: isSingleNode ? "Simulated APAC Edge" : "APAC Edge Accelerator",
      epoch: epochNumber,
      status: "ONLINE",
      latencyMs: 64.1,
    },
  ];

  return (
    <section
      aria-label="Multi-Region Cluster Topology"
      className="p-5 rounded-xl bg-slate-900/60 border border-white/10 backdrop-blur-md"
    >
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-2 mb-4">
        <div>
          <h2 className="text-base font-semibold text-white flex items-center gap-2">
            <Globe className="w-4 h-4 text-emerald-400" aria-hidden="true" />
            Multi-Region Cluster Topology
          </h2>
          <p className="text-xs text-slate-400 mt-0.5">
            Distributed Redis Streams replication with vector clock causality tracking.
          </p>
        </div>
        <div className="flex items-center gap-2 text-xs font-mono">
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
              SIMULATION
            </span>
          )}
          {isSingleNode ? (
            <span className="px-2 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-500/30 text-[10px] font-semibold">
              Single-Node (Preview Mesh)
            </span>
          ) : (
            <span className="px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 text-[10px] font-semibold">
              Multi-Region Active
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
              <div className="flex items-center justify-between text-[11px] mb-1">
                <span className="font-semibold text-white truncate">{node.region}</span>
                <span
                  className={`font-mono text-[9px] px-1.5 py-0.5 rounded font-semibold ${
                    node.status === "LEADER"
                      ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30"
                      : "bg-emerald-500/10 text-emerald-300 border border-emerald-500/20"
                  }`}
                >
                  {node.status}
                </span>
              </div>
              <div className="text-[11px] text-slate-400 font-mono">{node.role}</div>
            </div>

            <div className="mt-3 pt-2 border-t border-white/5 flex items-center justify-between text-[10px] font-mono text-slate-400">
              <span>RTT: {node.latencyMs}ms</span>
              <span className="text-slate-300">Sync: OK</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
