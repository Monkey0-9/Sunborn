import React from "react";

export default function SecurityPage() {
  const tenants = [
    {
      id: "enterprise-fintech",
      name: "Global Fintech Solutions",
      rpsQuota: "250 RPS",
      currentRps: "84 RPS",
      blockedThreats: 0,
      status: "Compliant",
    },
    {
      id: "acme-corp",
      name: "Acme Industrial Labs",
      rpsQuota: "100 RPS",
      currentRps: "42 RPS",
      blockedThreats: 2,
      status: "Compliant",
    },
    {
      id: "healthcare-systems",
      name: "National Health Records",
      rpsQuota: "500 RPS",
      currentRps: "190 RPS",
      blockedThreats: 0,
      status: "Compliant",
    },
  ];

  return (
    <div className="space-y-8 max-w-7xl mx-auto">
      <div>
        <h2 className="text-2xl font-bold text-white tracking-tight">Security & Multi-Tenant Isolation Center</h2>
        <p className="text-sm text-slate-400 mt-1">
          Zero-trust tenant isolation, token-bucket rate limiting enforcement, and prompt-injection threat mitigation.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Tenant Partitions</p>
          <p className="text-2xl font-bold text-white mt-1">3 Active Tenants</p>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Cross-Tenant Collisions</p>
          <p className="text-2xl font-bold text-emerald-400 mt-1">0 Collisions (SHA-256 Scope)</p>
        </div>
        <div className="p-5 rounded-xl border border-slate-800 bg-slate-900/40">
          <p className="text-xs font-medium text-slate-400 uppercase">Blocked Malicious Injections</p>
          <p className="text-2xl font-bold text-indigo-400 mt-1">2 Intercepted</p>
        </div>
      </div>

      <div className="rounded-xl border border-slate-800 bg-slate-900/40 overflow-hidden shadow-sm">
        <div className="p-5 border-b border-slate-800/80 flex items-center justify-between">
          <h3 className="font-semibold text-white text-base">Active Tenant Quotas & Isolation Envelopes</h3>
          <span className="text-xs text-emerald-400 font-mono">Enforcement: Active Rate Limiter</span>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm text-slate-300">
            <thead className="bg-slate-900/80 text-xs font-medium text-slate-400 uppercase border-b border-slate-800">
              <tr>
                <th className="px-6 py-3">Tenant ID</th>
                <th className="px-6 py-3">Organization</th>
                <th className="px-6 py-3">RPS Quota</th>
                <th className="px-6 py-3">Current Load</th>
                <th className="px-6 py-3">Blocked Threats</th>
                <th className="px-6 py-3">Isolation Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60 font-mono text-xs">
              {tenants.map((t) => (
                <tr key={t.id} className="hover:bg-slate-800/30 transition-colors">
                  <td className="px-6 py-3.5 text-indigo-400 font-medium">{t.id}</td>
                  <td className="px-6 py-3.5 font-sans font-medium text-white">{t.name}</td>
                  <td className="px-6 py-3.5 text-slate-300">{t.rpsQuota}</td>
                  <td className="px-6 py-3.5 text-emerald-400 font-semibold">{t.currentRps}</td>
                  <td className="px-6 py-3.5 text-slate-300">{t.blockedThreats}</td>
                  <td className="px-6 py-3.5 font-sans">
                    <span className="bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 px-2 py-0.5 rounded text-xs font-medium">
                      {t.status}
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
