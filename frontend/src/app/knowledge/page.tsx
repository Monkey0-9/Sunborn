import React from "react";

export default function KnowledgePage() {
  const sources = [
    {
      id: "src_9a1f-4b2e",
      name: "API Reference v2.4 (OpenAPI Spec)",
      version: "v2.4.1",
      dependencies: 142,
      lastUpdated: "12m ago",
      status: "Synchronized",
    },
    {
      id: "src_8c3d-1a7c",
      name: "Enterprise Compliance & Security Manual",
      version: "v1.9.0",
      dependencies: 89,
      lastUpdated: "2h ago",
      status: "Synchronized",
    },
    {
      id: "src_2f0e-5d8b",
      name: "Product Pricing & SLA Terms",
      version: "v3.0.0",
      dependencies: 310,
      lastUpdated: "1d ago",
      status: "Synchronized",
    },
  ];

  return (
    <div className="space-y-8 max-w-7xl mx-auto">
      <div>
        <h2 className="text-2xl font-bold text-white tracking-tight">Knowledge Dependency & Invalidation Graph</h2>
        <p className="text-sm text-slate-400 mt-1">
          Event-driven outbox tracking ensuring zero cache drift when underlying documentation updates.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Monitored Knowledge Sources</p>
          <p className="text-2xl font-bold text-white mt-1">3 Sources</p>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Tracked Dependency Edges</p>
          <p className="text-2xl font-bold text-indigo-400 mt-1">541 Cache Entries</p>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Outbox Processing Latency</p>
          <p className="text-2xl font-bold text-emerald-400 mt-1">&lt; 4.5 ms</p>
        </div>
      </div>

      <div className="rounded-xl border border-slate-800 bg-slate-900/40 overflow-hidden shadow-sm">
        <div className="p-5 border-b border-slate-800/80 flex items-center justify-between">
          <h3 className="font-semibold text-white text-base">Knowledge Sources & Dependent Cache Graph</h3>
          <span className="text-xs text-indigo-400 font-mono">Redpanda Outbox Event Bus: Connected</span>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm text-slate-300">
            <thead className="bg-slate-900/80 text-xs font-medium text-slate-400 uppercase border-b border-slate-800">
              <tr>
                <th className="px-6 py-3">Source Name</th>
                <th className="px-6 py-3">Source ID</th>
                <th className="px-6 py-3">Version Tag</th>
                <th className="px-6 py-3">Dependent Caches</th>
                <th className="px-6 py-3">Last Sync</th>
                <th className="px-6 py-3">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60 font-mono text-xs">
              {sources.map((src) => (
                <tr key={src.id} className="hover:bg-slate-800/30 transition-colors">
                  <td className="px-6 py-3.5 font-sans font-medium text-white">{src.name}</td>
                  <td className="px-6 py-3.5 text-slate-400">{src.id}</td>
                  <td className="px-6 py-3.5 text-indigo-300">{src.version}</td>
                  <td className="px-6 py-3.5 text-emerald-400 font-semibold">{src.dependencies} entries</td>
                  <td className="px-6 py-3.5 font-sans text-slate-400">{src.lastUpdated}</td>
                  <td className="px-6 py-3.5 font-sans">
                    <span className="bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 px-2 py-0.5 rounded text-xs font-medium">
                      {src.status}
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
