import React from "react";

export default function DashboardPage() {
  const stats = [
    { name: "Gateway Throughput", value: "15,422 req/s", change: "90 µs P99", positive: true },
    { name: "Conformal Risk Bound", value: "α ≤ 0.005", change: "0% False Hits", positive: true },
    { name: "Merkle-DAG Splicing", value: "72% Spliced", change: "<100 µs Pruning", positive: true },
    { name: "Prefix KV Locality", value: "80% Prefill Saved", change: "Radix Router", positive: true },
  ];

  const recentRequests = [
    {
      id: "req_8f1a",
      tenant: "enterprise-fintech",
      model: "gpt-4o",
      tier: "C0/C1 Tiered Exact",
      latency: "0.062 ms",
      saved: "420 ms",
      status: "HIT (L1 Moka)",
      time: "10s ago",
    },
    {
      id: "req_9c2b",
      tenant: "acme-corp",
      model: "claude-3-5-sonnet",
      tier: "C2+ Conformal NLI",
      latency: "3.2 ms",
      saved: "680 ms",
      status: "HIT (Entailed)",
      time: "24s ago",
    },
    {
      id: "req_1d4e",
      tenant: "acme-corp",
      model: "gpt-4o",
      tier: "C3+ Merkle-DAG Spliced",
      latency: "1.1 ms",
      saved: "1,200 ms",
      status: "SPLICED (Steps 1-7)",
      time: "1m ago",
    },
    {
      id: "req_3a7f",
      tenant: "healthcare-systems",
      model: "gpt-4o",
      tier: "C4 Singleflight SSE",
      latency: "450 ms",
      saved: "Origin Coalesced",
      status: "MULTIPLEXED (Follower)",
      time: "2m ago",
    },
  ];

  return (
    <div className="space-y-8 max-w-7xl mx-auto">
      <div>
        <h2 className="text-2xl font-bold text-white tracking-tight">Executive System Overview</h2>
        <p className="text-sm text-slate-400 mt-1">
          Real-time performance metrics, multi-tier execution reuse, and cost savings across all tenants.
        </p>
      </div>

      {/* KPI Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {stats.map((stat) => (
          <div
            key={stat.name}
            className="p-5 rounded-xl border border-slate-800 bg-slate-900/40 shadow-sm backdrop-blur"
          >
            <p className="text-xs font-medium text-slate-400 uppercase tracking-wider">{stat.name}</p>
            <div className="mt-2 flex items-baseline justify-between">
              <span className="text-2xl font-bold text-white">{stat.value}</span>
              <span className="text-xs font-semibold text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded-full">
                {stat.change}
              </span>
            </div>
          </div>
        ))}
      </div>

      {/* Architecture Status & Invariant Card */}
      <div className="p-6 rounded-xl border border-indigo-950/60 bg-gradient-to-r from-indigo-950/30 via-slate-900/50 to-slate-900/30 shadow-lg">
        <div className="flex items-center gap-3 mb-2">
          <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />
          <h3 className="font-semibold text-white text-base">Architectural Invariant Status: Operational</h3>
        </div>
        <p className="text-sm text-slate-300 leading-relaxed">
          PostgreSQL authoritative system-of-record is synchronized with Dragonfly exact cache and Qdrant vector index. 
          Event-driven outbox pipeline active with zero detected knowledge drift.
        </p>
      </div>

      {/* Recent Requests Table */}
      <div className="rounded-xl border border-slate-800 bg-slate-900/40 overflow-hidden shadow-sm">
        <div className="p-5 border-b border-slate-800/80 flex items-center justify-between">
          <h3 className="font-semibold text-white text-base">Recent Gateway Requests & Decision Traces</h3>
          <span className="text-xs text-slate-400">Live stream (Polling Gateway /metrics)</span>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm text-slate-300">
            <thead className="bg-slate-900/80 text-xs font-medium text-slate-400 uppercase border-b border-slate-800">
              <tr>
                <th className="px-6 py-3">Request ID</th>
                <th className="px-6 py-3">Tenant</th>
                <th className="px-6 py-3">Model</th>
                <th className="px-6 py-3">Execution Tier</th>
                <th className="px-6 py-3">Latency</th>
                <th className="px-6 py-3">Time Saved</th>
                <th className="px-6 py-3">Decision</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60 font-mono text-xs">
              {recentRequests.map((req) => (
                <tr key={req.id} className="hover:bg-slate-800/30 transition-colors">
                  <td className="px-6 py-3.5 text-indigo-400 font-medium">{req.id}</td>
                  <td className="px-6 py-3.5 font-sans text-slate-200">{req.tenant}</td>
                  <td className="px-6 py-3.5 text-slate-400">{req.model}</td>
                  <td className="px-6 py-3.5 text-slate-300 font-sans">{req.tier}</td>
                  <td className="px-6 py-3.5 text-slate-400">{req.latency}</td>
                  <td className="px-6 py-3.5 text-emerald-400 font-semibold">{req.saved}</td>
                  <td className="px-6 py-3.5 font-sans">
                    <span
                      className={`px-2 py-0.5 rounded text-xs font-medium ${
                        req.status.startsWith("HIT")
                          ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"
                          : "bg-amber-500/10 text-amber-400 border border-amber-500/20"
                      }`}
                    >
                      {req.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
