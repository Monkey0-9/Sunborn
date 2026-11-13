import type { Metadata, Viewport } from "next";
import React from "react";
import "./globals.css";
import { Header } from "@/components/layout/header";

export const metadata: Metadata = {
  title: "Sunborn | Autonomous AI Execution & Conformal Semantic Cache",
  description:
    "Enterprise Multi-Tenant Semantic Caching and Proof-Carrying Temporal Reuse Platform for Autonomous AI Agents.",
};

export const viewport: Viewport = {
  themeColor: "#020617",
  width: "device-width",
  initialScale: 1,
  maximumScale: 5,
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark scroll-smooth">
      <body className="min-h-screen bg-[#020617] text-slate-100 font-sans antialiased selection:bg-cyan-500/30 selection:text-cyan-200 relative overflow-x-hidden">
        {/* Silicon Valley Quantum Mesh & Atmospheric Spotlights */}
          <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
            <div className="absolute -top-40 left-1/4 w-[750px] h-[550px] rounded-full bg-gradient-to-tr from-cyan-600/12 via-indigo-600/10 to-transparent blur-[160px]" />
            <div className="absolute top-[35%] -right-20 w-[650px] h-[650px] rounded-full bg-gradient-to-bl from-violet-600/10 via-purple-700/8 to-transparent blur-[180px]" />
            <div className="absolute -bottom-20 left-[15%] w-[800px] h-[600px] rounded-full bg-gradient-to-tr from-emerald-600/8 via-cyan-900/10 to-transparent blur-[190px]" />
            <div className="absolute inset-0 bg-aerospace-grid [mask-image:radial-gradient(ellipse_75%_65%_at_50%_0%,#000_60%,transparent_100%)] opacity-70" />
          </div>

          {/* Floating Flagship Navigation Header */}
          <Header />

          {/* Main Viewport Container */}
          <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 relative z-10">
            {children}
          </main>

          {/* Flagship Enterprise Footer */}
          <footer className="w-full border-t border-white/[0.08] bg-[#030712]/90 backdrop-blur-2xl py-10 mt-20 text-xs font-mono relative z-10">
            <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
              <div className="grid grid-cols-1 md:grid-cols-4 gap-8 pb-8 border-b border-white/[0.06]">
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <span className="w-2.5 h-2.5 rounded-full bg-cyan-400 shadow-glow" />
                    <span className="font-bold text-sm tracking-tight text-white">SUNBORN GATEWAY</span>
                  </div>
                  <p className="text-slate-400 text-[11px] leading-relaxed">
                    Proof-Carrying Temporal Reuse & Multi-Tenant Semantic Ingress Gateway. Engineered for Google, Apple, Meta, Microsoft, and NVIDIA enterprise agent scale.
                  </p>
                </div>

                <div className="space-y-2">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-slate-300">Specifications</div>
                  <ul className="space-y-1 text-slate-400 text-[11px]">
                    <li>• RFC 6962 SHA-256 Merkle Provenance</li>
                    <li>• GDPR Article 17 Cryptographic Shredding</li>
                    <li>• Conformal Risk Control (α ≤ 0.001)</li>
                    <li>• Roaring Bitmap SIMD Reverse Index</li>
                  </ul>
                </div>

                <div className="space-y-2">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-slate-300">Architecture Tiers</div>
                  <ul className="space-y-1 text-slate-400 text-[11px]">
                    <li>• Tier 0/1: In-Memory W-TinyLFU + DragonflyDB</li>
                    <li>• Tier 2: Qdrant Vector Conformal Search</li>
                    <li>• Tier 3: Deterministic Agent Execution DAG</li>
                    <li>• Tier 4: PostgreSQL Outbox Write-Behind</li>
                  </ul>
                </div>

                <div className="space-y-2">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-slate-300">System Telemetry</div>
                  <div className="p-3 rounded-xl bg-white/[0.02] border border-white/[0.06] space-y-1.5 text-[11px]">
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">Authority:</span>
                      <span className="text-emerald-400 font-bold">PostgreSQL Active</span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">Cluster Mesh:</span>
                      <span className="text-cyan-400 font-bold">3 Regions Reconciled</span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">License:</span>
                      <span className="text-white">Apache 2.0 Open Spec</span>
                    </div>
                  </div>
                </div>
              </div>

              <div className="flex flex-col sm:flex-row items-center justify-between gap-4 pt-6 text-[11px] text-slate-400">
                <div className="flex items-center gap-3">
                  <span className="relative flex h-2 w-2">
                    <span className="radar-ping-ring absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
                  </span>
                  <span>All Subsystems Operational · Multi-Tier Non-Accelerator Authority</span>
                </div>
                <div>
                  <span>Sunborn Autonomous Gateway © 2026–2028 · Independent audit: NO-GO</span>
                </div>
              </div>
            </div>
          </footer>
      </body>
    </html>
  );
}
