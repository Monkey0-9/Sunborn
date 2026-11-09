"use client";

import React, { useEffect } from "react";
import { AlertCircle, RefreshCw, Home } from "lucide-react";
import Link from "next/link";

interface ErrorPageProps {
  error: Error & { digest?: string };
  reset: () => void;
}

export default function ErrorPage({ error, reset }: ErrorPageProps) {
  useEffect(() => {
    // Log unexpected client exceptions for observability
    console.error("Next.js Client Error Boundary intercepted exception:", error);
  }, [error]);

  return (
    <div className="min-h-[70vh] flex items-center justify-center p-6">
      <div className="max-w-md w-full rounded-xl border border-red-900/50 bg-slate-950/90 p-8 shadow-2xl backdrop-blur-md">
        <div className="flex items-center gap-3 text-red-500 mb-4">
          <div className="p-3 rounded-lg bg-red-950/80 border border-red-800/60">
            <AlertCircle className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-xl font-bold tracking-tight text-slate-100">
              System Boundary Fault
            </h2>
            <p className="text-xs font-mono text-red-400">
              {error.digest ? `Fault Digest: ${error.digest}` : "Uncaught Exception"}
            </p>
          </div>
        </div>

        <p className="text-sm text-slate-400 mb-6 leading-relaxed">
          The Sunborn control plane intercepted an unhandled rendering fault. The fail-safe
          isolation boundary prevented cascading failure across the application workspace.
        </p>

        {error.message && (
          <div className="mb-6 p-3 rounded-lg bg-slate-900/90 border border-slate-800 font-mono text-xs text-slate-300 break-words max-h-32 overflow-y-auto">
            {error.message}
          </div>
        )}

        <div className="flex items-center gap-3">
          <button
            onClick={() => reset()}
            className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-red-600 hover:bg-red-500 text-white font-medium text-sm transition-colors shadow-lg shadow-red-950/50"
          >
            <RefreshCw className="w-4 h-4" />
            Retry View
          </button>
          <Link
            href="/"
            className="flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-slate-900 hover:bg-slate-800 text-slate-300 font-medium text-sm transition-colors border border-slate-800"
          >
            <Home className="w-4 h-4" />
            Home
          </Link>
        </div>
      </div>
    </div>
  );
}
