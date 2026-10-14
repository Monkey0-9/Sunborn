import type { Metadata } from "next";
import React from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: "Sunborn | Enterprise Semantic Execution Platform",
  description: "Enterprise multi-tier semantic caching and execution reuse for AI/RAG agents.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="bg-slate-950 text-slate-100 min-h-screen flex antialiased">
        {/* Sidebar */}
        <aside className="w-64 border-r border-slate-800 bg-slate-900/60 p-6 flex flex-col justify-between shrink-0">
          <div>
            <div className="flex items-center gap-3 mb-8">
              <div className="h-8 w-8 rounded-lg bg-indigo-500 flex items-center justify-center font-bold text-white shadow-lg shadow-indigo-500/30">
                S
              </div>
              <div>
                <h1 className="font-bold text-base tracking-tight text-white">Sunborn</h1>
                <p className="text-xs text-indigo-400 font-medium">Enterprise Control Plane</p>
              </div>
            </div>

            <nav className="space-y-1">
              <a
                href="/"
                className="flex items-center gap-3 px-3 py-2 text-sm font-medium rounded-md bg-indigo-600/20 text-indigo-300 border border-indigo-500/20"
              >
                📊 Executive Overview
              </a>
              <a
                href="/explorer"
                className="flex items-center gap-3 px-3 py-2 text-sm font-medium rounded-md text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 transition-colors"
              >
                🔍 Request Explorer
              </a>
              <a
                href="/knowledge"
                className="flex items-center gap-3 px-3 py-2 text-sm font-medium rounded-md text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 transition-colors"
              >
                🕸️ Knowledge Graph
              </a>
              <a
                href="/security"
                className="flex items-center gap-3 px-3 py-2 text-sm font-medium rounded-md text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 transition-colors"
              >
                🛡️ Security & Tenants
              </a>
            </nav>
          </div>

          <div className="pt-4 border-t border-slate-800/80">
            <div className="flex items-center justify-between text-xs text-slate-500">
              <span>Gateway: v0.1.0</span>
              <span className="flex items-center gap-1.5 text-emerald-400">
                <span className="h-2 w-2 rounded-full bg-emerald-400 animate-pulse" />
                Healthy
              </span>
            </div>
          </div>
        </aside>

        {/* Main Content Area */}
        <main className="flex-1 overflow-y-auto p-8">
          {children}
        </main>
      </body>
    </html>
  );
}
