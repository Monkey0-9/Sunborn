"use client";

import React, { createContext, useContext, useState, useCallback } from "react";
import { CheckCircle2, AlertTriangle, XCircle, Info, X } from "lucide-react";
import { cn } from "@/lib/utils";

export type ToastType = "success" | "error" | "warning" | "info";

export interface Toast {
  id: string;
  title: string;
  description?: string;
  type: ToastType;
  duration?: number;
}

interface ToastContextType {
  toasts: Toast[];
  toast: (options: Omit<Toast, "id">) => void;
  removeToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextType | undefined>(undefined);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    ({ title, description, type = "info", duration = 4000 }: Omit<Toast, "id">) => {
      const id = `${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
      setToasts((prev) => [...prev, { id, title, description, type, duration }]);

      if (duration > 0) {
        setTimeout(() => {
          removeToast(id);
        }, duration);
      }
    },
    [removeToast]
  );

  return (
    <ToastContext.Provider value={{ toasts, toast, removeToast }}>
      {children}
      {/* Toast Viewport */}
      <div
        aria-live="polite"
        className="fixed bottom-6 right-6 z-50 flex flex-col gap-2 max-w-md w-full pointer-events-none"
      >
        {toasts.map((t) => {
          const icons = {
            success: <CheckCircle2 className="w-5 h-5 text-emerald-400 shrink-0" />,
            error: <XCircle className="w-5 h-5 text-rose-400 shrink-0" />,
            warning: <AlertTriangle className="w-5 h-5 text-amber-400 shrink-0" />,
            info: <Info className="w-5 h-5 text-cyan-400 shrink-0" />,
          };

          const borders = {
            success: "border-emerald-500/30 bg-slate-950/95 shadow-emerald-950/40",
            error: "border-rose-500/30 bg-slate-950/95 shadow-rose-950/40",
            warning: "border-amber-500/30 bg-slate-950/95 shadow-amber-950/40",
            info: "border-cyan-500/30 bg-slate-950/95 shadow-cyan-950/40",
          };

          return (
            <div
              key={t.id}
              className={cn(
                "pointer-events-auto flex items-start gap-3 p-4 rounded-xl border shadow-xl backdrop-blur-xl transition-all duration-300 animate-in fade-in slide-in-from-bottom-2",
                borders[t.type]
              )}
            >
              {icons[t.type]}
              <div className="flex-1 min-w-0">
                <p className="text-sm font-semibold text-white tracking-tight">{t.title}</p>
                {t.description && (
                  <p className="text-xs text-slate-400 mt-0.5 leading-relaxed font-mono">
                    {t.description}
                  </p>
                )}
              </div>
              <button
                onClick={() => removeToast(t.id)}
                className="text-slate-500 hover:text-white transition-colors p-1 rounded-md"
                aria-label="Close notification"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    return {
      toasts: [],
      toast: () => {},
      removeToast: () => {},
    };
  }
  return context;
}
