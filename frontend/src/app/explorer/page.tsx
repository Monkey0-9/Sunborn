import React from "react";

export default function ExplorerPage() {
  return (
    <div className="space-y-8 max-w-7xl mx-auto">
      <div>
        <h2 className="text-2xl font-bold text-white tracking-tight">Request & Cache Trace Explorer</h2>
        <p className="text-sm text-slate-400 mt-1">
          Deep inspection of canonical request hashing, exact identity matches, and vector decision reasoning.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Interactive Query Simulator */}
        <div className="p-6 rounded-xl border border-slate-800 bg-slate-900/40 space-y-4">
          <h3 className="font-semibold text-white text-base">Request Payload Inspector</h3>
          <div className="space-y-3 text-sm">
            <div>
              <label className="text-xs text-slate-400 font-medium">Tenant ID</label>
              <input
                type="text"
                readOnly
                value="enterprise-alpha"
                className="w-full mt-1 bg-slate-950 border border-slate-800 rounded-lg px-3 py-2 text-slate-200 font-mono text-xs"
              />
            </div>
            <div>
              <label className="text-xs text-slate-400 font-medium">Model</label>
              <input
                type="text"
                readOnly
                value="gpt-4o"
                className="w-full mt-1 bg-slate-950 border border-slate-800 rounded-lg px-3 py-2 text-slate-200 font-mono text-xs"
              />
            </div>
            <div>
              <label className="text-xs text-slate-400 font-medium">Input Prompt</label>
              <textarea
                rows={4}
                readOnly
                value="What are the architectural guarantees of Sunborn's non-accelerator authority?"
                className="w-full mt-1 bg-slate-950 border border-slate-800 rounded-lg p-3 text-slate-200 font-mono text-xs resize-none"
              />
            </div>
          </div>
        </div>

        {/* Pipeline Decision Decomposition */}
        <div className="p-6 rounded-xl border border-slate-800 bg-slate-900/40 space-y-4">
          <h3 className="font-semibold text-white text-base">Pipeline Decision Decomposition</h3>
          <div className="space-y-3 font-mono text-xs">
            <div className="p-3 bg-slate-950 border border-slate-800 rounded-lg">
              <span className="text-slate-400 block text-[10px] uppercase">1. Deterministic Identity Key</span>
              <span className="text-indigo-400 break-all">
                sunborn:enterprise-alpha:app1:gpt-4o:v1:7d94f2e1a8b9c034...
              </span>
            </div>
            <div className="p-3 bg-slate-950 border border-slate-800 rounded-lg">
              <span className="text-slate-400 block text-[10px] uppercase">2. Multi-Tier Cache State</span>
              <span className="text-emerald-400 font-semibold">C0 Exact Cache: HIT (0.8ms)</span>
            </div>
            <div className="p-3 bg-slate-950 border border-slate-800 rounded-lg">
              <span className="text-slate-400 block text-[10px] uppercase">3. Calibrated Confidence Score</span>
              <span className="text-slate-200 font-semibold">1.00 (Exact Canonical Identity)</span>
            </div>
            <div className="p-3 bg-slate-950 border border-slate-800 rounded-lg">
              <span className="text-slate-400 block text-[10px] uppercase">4. Domain Risk Assessment</span>
              <span className="text-emerald-400 font-semibold">0.00 (Safe Informational Query)</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
