import React from "react";

export default function ExecutionDAGPage() {
  const dagNodes = [
    {
      id: "step_01",
      name: "Extract Parameters & Intent",
      type: "Deterministic LLM Step",
      status: "REUSED (Cached)",
      latency: "1.2 ms (Saved 320 ms)",
      signature: "0a89d71c3...",
      cached: true,
    },
    {
      id: "step_02",
      name: "SQL Tool: query_financial_metrics",
      type: "Deterministic Tool Call",
      status: "REUSED (Cached)",
      latency: "0.8 ms (Saved 180 ms)",
      signature: "f4b819a0e...",
      cached: true,
    },
    {
      id: "step_03",
      name: "Vector Tool: search_recent_filings",
      type: "Deterministic Tool Call",
      status: "REUSED (Cached)",
      latency: "2.1 ms (Saved 250 ms)",
      signature: "6e1a0b32f...",
      cached: true,
    },
    {
      id: "step_04",
      name: "Agent Synthesis: Generate Executive Brief",
      type: "Non-Deterministic LLM Step",
      status: "EXECUTED (Origin)",
      latency: "410 ms",
      signature: "N/A (Dynamic)",
      cached: false,
    },
  ];

  const shadowEvaluations = [
    {
      id: "sh_01",
      tenant: "acme-corp",
      key: "sunborn:exact:acme:q4_outlook",
      similarity: "98.4%",
      driftStatus: "PASSED",
      action: "VERIFIED_CONSISTENT",
      timestamp: "3m ago",
    },
    {
      id: "sh_02",
      tenant: "enterprise-fintech",
      key: "sunborn:exact:fintech:treasury_yield",
      similarity: "71.2%",
      driftStatus: "DRIFT_DETECTED",
      action: "AUTO_EVICTED_STALE",
      timestamp: "12m ago",
    },
  ];

  return (
    <div className="space-y-8 max-w-7xl mx-auto">
      <div>
        <h2 className="text-2xl font-bold text-white tracking-tight">Agent Execution DAG & Tool Reuse</h2>
        <p className="text-sm text-slate-400 mt-1">
          Deterministic execution subgraph memoization, topological re-planning, and background shadow drift validation.
        </p>
      </div>

      {/* High-level Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-5">
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">DAG Subgraph Hit Rate</p>
          <p className="text-2xl font-bold text-white mt-2">75.0%</p>
          <span className="text-xs text-emerald-400 font-medium">3 of 4 pipeline nodes memoized</span>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Pipeline Latency Avoidance</p>
          <p className="text-2xl font-bold text-white mt-2">750 ms</p>
          <span className="text-xs text-indigo-400 font-medium">64.6% total latency reduction</span>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Shadow Verification Rate</p>
          <p className="text-2xl font-bold text-white mt-2">99.2%</p>
          <span className="text-xs text-emerald-400 font-medium">Zero false hits served to clients</span>
        </div>
      </div>

      {/* Sample Execution DAG */}
      <div className="rounded-xl border border-slate-800 bg-slate-900/30 overflow-hidden">
        <div className="p-5 border-b border-slate-800 bg-slate-900/50 flex justify-between items-center">
          <div>
            <h3 className="text-base font-semibold text-white">Active Agent Workflow: Quarterly Earnings Multi-Agent Analyst</h3>
            <p className="text-xs text-slate-400 mt-0.5">Execution Plan DAG with topological reuse evaluation</p>
          </div>
          <span className="text-xs px-2.5 py-1 rounded-full font-mono font-medium bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
            Topological Sort: Valid
          </span>
        </div>

        <div className="divide-y divide-slate-800/60 font-mono text-xs">
          {dagNodes.map((node) => (
            <div key={node.id} className="p-4 flex flex-col md:flex-row md:items-center justify-between gap-3 hover:bg-slate-800/20 transition-colors">
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <span className="text-slate-500 font-semibold">{node.id}</span>
                  <span className="text-sm font-sans font-medium text-slate-200">{node.name}</span>
                  <span className="text-slate-400 text-[11px] font-sans">({node.type})</span>
                </div>
                <div className="text-slate-500 text-[11px]">Signature: {node.signature}</div>
              </div>
              <div className="flex items-center gap-4">
                <span className="text-slate-300">{node.latency}</span>
                <span
                  className={`px-2.5 py-1 rounded-full font-sans font-semibold text-[11px] ${
                    node.cached
                      ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"
                      : "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                  }`}
                >
                  {node.status}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Shadow Evaluation Feed */}
      <div className="rounded-xl border border-slate-800 bg-slate-900/30 overflow-hidden">
        <div className="p-5 border-b border-slate-800 bg-slate-900/50">
          <h3 className="text-base font-semibold text-white">Asynchronous Shadow Evaluation & Drift Log</h3>
          <p className="text-xs text-slate-400 mt-0.5">
            Continuous background validation comparing cached responses against origin execution
          </p>
        </div>

        <table className="w-full text-left text-xs">
          <thead className="bg-slate-900/80 text-slate-400 font-medium border-b border-slate-800">
            <tr>
              <th className="py-3 px-4">Evaluation ID</th>
              <th className="py-3 px-4">Tenant</th>
              <th className="py-3 px-4">Cache Key</th>
              <th className="py-3 px-4">Cosine Similarity</th>
              <th className="py-3 px-4">Drift Result</th>
              <th className="py-3 px-4">Automated Action</th>
              <th className="py-3 px-4">Time</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60 font-mono">
            {shadowEvaluations.map((sh) => (
              <tr key={sh.id} className="hover:bg-slate-800/20">
                <td className="py-3 px-4 text-slate-300">{sh.id}</td>
                <td className="py-3 px-4 text-slate-400 font-sans">{sh.tenant}</td>
                <td className="py-3 px-4 text-slate-400 truncate max-w-xs">{sh.key}</td>
                <td className="py-3 px-4 font-semibold text-slate-200">{sh.similarity}</td>
                <td className="py-3 px-4 font-sans">
                  <span
                    className={`px-2 py-0.5 rounded text-[10px] font-semibold ${
                      sh.driftStatus === "PASSED"
                        ? "bg-emerald-500/10 text-emerald-400"
                        : "bg-amber-500/10 text-amber-400"
                    }`}
                  >
                    {sh.driftStatus}
                  </span>
                </td>
                <td className="py-3 px-4 font-sans text-slate-300">{sh.action}</td>
                <td className="py-3 px-4 font-sans text-slate-500">{sh.timestamp}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
