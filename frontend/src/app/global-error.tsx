"use client";

import React from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <html lang="en" className="dark">
      <body className="bg-slate-950 text-slate-100 min-h-screen flex items-center justify-center p-6 antialiased font-sans">
        <div className="max-w-md w-full rounded-2xl border border-red-900/60 bg-slate-900/95 p-8 shadow-2xl text-center">
          <div className="w-14 h-14 mx-auto mb-4 rounded-xl bg-red-950/90 border border-red-800 flex items-center justify-center text-red-500">
            <AlertTriangle className="w-8 h-8" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight mb-2">
            Critical Root Failure
          </h1>
          <p className="text-sm text-slate-400 mb-6">
            A fatal exception occurred in the root document layout.
            {error.digest && (
              <span className="block font-mono text-xs text-red-400 mt-2">
                ID: {error.digest}
              </span>
            )}
          </p>
          <button
            onClick={() => reset()}
            className="w-full inline-flex items-center justify-center gap-2 px-5 py-3 rounded-xl bg-red-600 hover:bg-red-500 text-white font-semibold text-sm transition-colors shadow-lg"
          >
            <RefreshCw className="w-4 h-4" />
            Reload Control Plane
          </button>
        </div>
      </body>
    </html>
  );
}
