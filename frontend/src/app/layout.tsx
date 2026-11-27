import type { Metadata, Viewport } from "next";
import { headers } from "next/headers";
import React from "react";
import "./globals.css";
import { Header } from "@/components/layout/header";
import { PlainPageBand } from "@/components/layout/plain-page-band";

export const metadata: Metadata = {
  title: "Sunborn | Autonomous AI Execution & Conformal Semantic Cache",
  description:
    "Enterprise Multi-Tenant Semantic Caching and Proof-Carrying Temporal Reuse Platform for Autonomous AI Agents.",
};

// The nonce-based CSP requires request-time rendering; a cached HTML shell would
// ship script tags generated without the current request's nonce. Declared here
// so the segment config is explicit rather than inferred from `headers()` alone.
export const dynamic = "force-dynamic";

export const viewport: Viewport = {
  themeColor: "#020617",
  width: "device-width",
  initialScale: 1,
  maximumScale: 5,
};

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // Reading a request header opts every route into dynamic rendering. That is a
  // prerequisite for the nonce-based CSP in `src/middleware.ts`: Next.js only
  // stamps its framework and page bundles with the nonce during request-time
  // rendering. A statically prerendered page would ship un-nonced <script> tags,
  // which `script-src 'nonce-…' 'strict-dynamic'` blocks outright.
  //
  // The trade is explicit: per-request rendering in exchange for a policy that
  // actually stops injected inline script. The value is read by Next.js itself
  // from the CSP header, so nothing here needs to forward it.
  await headers();

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

          {/* Keyboard bypass mechanism (WCAG 2.4.1). Visible only when focused. */}
          <a
            href="#main-content"
            className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[100] focus:rounded-lg focus:bg-cyan-400 focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-slate-950 focus:shadow-glow"
          >
            Skip to main content
          </a>

          {/* Floating Flagship Navigation Header */}
          <Header />

          {/* Main Viewport Container */}
          <main
            id="main-content"
            tabIndex={-1}
            className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 relative z-10"
          >
            <PlainPageBand />
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
                    <li>• Conformal abstention (calibrated per policy)</li>
                    <li>• Roaring Bitmap SIMD Reverse Index</li>
                  </ul>
                </div>

                <div className="space-y-2">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-slate-300">Architecture Tiers</div>
                  <ul className="space-y-1 text-slate-400 text-[11px]">
                    <li>• Tier 0/1: In-Memory FinOps-GDSF + DragonflyDB</li>
                    <li>• Tier 2: Qdrant Vector Conformal Search</li>
                    <li>• Tier 3: Deterministic Agent Execution DAG</li>
                    <li>• Tier 4: PostgreSQL Outbox Write-Behind</li>
                  </ul>
                </div>

                <div className="space-y-2">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-slate-300">Deployment facts</div>
                  <div className="p-3 rounded-xl bg-white/[0.02] border border-white/[0.06] space-y-1.5 text-[11px]">
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">Live health:</span>
                      <a href="/start" className="text-cyan-300 font-bold underline underline-offset-2">Check here</a>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">Release state:</span>
                      <span className="text-white">Stage 0 controlled pilot</span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-slate-400">License:</span>
                      <span className="text-white">Apache 2.0</span>
                    </div>
                  </div>
                </div>
              </div>

              <div className="flex flex-col sm:flex-row items-center justify-between gap-4 pt-6 text-[11px] text-slate-400">
                <div className="flex items-center gap-3">
                  <span className="relative flex h-2 w-2" aria-hidden="true">
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-slate-500"></span>
                  </span>
                  <span>Status is not asserted in this footer — open “Start here” for a live check.</span>
                </div>
                <div>
                  <span>Sunborn © 2026 · Apache 2.0 · see docs/STATUS.md for the release verdict</span>
                </div>
              </div>
            </div>
          </footer>
      </body>
    </html>
  );
}
