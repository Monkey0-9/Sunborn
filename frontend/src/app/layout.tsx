import type { Metadata } from "next";
import React from "react";
import "./globals.css";
import { Header } from "@/components/layout/header";

export const metadata: Metadata = {
  title: "Sunborn | Autonomous AI Execution & Conformal Semantic Cache",
  description:
    "Enterprise Multi-Tenant Semantic Caching and Proof-Carrying Temporal Reuse Platform for Autonomous AI Agents.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark scroll-smooth">
      <body className="min-h-screen bg-slate-1000 text-slate-100 font-sans antialiased selection:bg-cyan-500/30 selection:text-cyan-200">
        {/* Ambient Luminescent Glows */}
        <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
          <div className="absolute top-[-10%] left-[20%] w-[500px] h-[500px] rounded-full bg-cyan-600/10 blur-[130px]" />
          <div className="absolute top-[20%] right-[15%] w-[450px] h-[450px] rounded-full bg-violet-600/10 blur-[140px]" />
          <div className="absolute bottom-[10%] left-[30%] w-[600px] h-[600px] rounded-full bg-emerald-600/5 blur-[160px]" />
          <div className="absolute inset-0 bg-[linear-gradient(to_right,#ffffff03_1px,transparent_1px),linear-gradient(to_bottom,#ffffff03_1px,transparent_1px)] bg-[size:4rem_4rem] [mask-image:radial-gradient(ellipse_60%_50%_at_50%_0%,#000_70%,transparent_100%)]" />
        </div>

        {/* Top Header */}
        <Header />

        {/* Main Content Viewport */}
        <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          {children}
        </main>

        {/* Global Footer */}
        <footer className="w-full border-t border-white/[0.08] bg-slate-950/90 py-8 mt-16 text-xs text-slate-500 font-mono">
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between gap-4">
            <div className="flex items-center gap-3">
              <span className="w-2 h-2 rounded-full bg-emerald-400" />
              <span>Sunborn Autonomous Platform · Adaptive Closed-Loop Verification</span>
              <span className="text-slate-600">|</span>
              <span className="text-slate-400">Automated Invariant & Empirical Verification</span>
            </div>
            <div className="flex items-center gap-4">
              <span>Non-Accelerator Authority: PostgreSQL Active</span>
              <span className="text-slate-600">|</span>
              <span className="text-cyan-400">Apache 2.0</span>
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}
