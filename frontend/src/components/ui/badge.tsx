import React from "react";
import { cn } from "@/lib/utils";

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?:
    | "default"
    | "success"
    | "emerald"
    | "warning"
    | "danger"
    | "purple"
    | "violet"
    | "cyan"
    | "outline";
  dot?: boolean;
}

export function Badge({
  className,
  variant = "default",
  dot = false,
  children,
  ...props
}: BadgeProps) {
  const variantStyles = {
    default: "bg-slate-800/80 text-slate-300 border-slate-700/60",
    success: "bg-emerald-950/50 text-emerald-300 border-emerald-500/30",
    emerald: "bg-emerald-950/50 text-emerald-300 border-emerald-500/30",
    warning: "bg-amber-950/50 text-amber-300 border-amber-500/30",
    danger: "bg-rose-950/50 text-rose-300 border-rose-500/30",
    purple: "bg-violet-950/50 text-violet-300 border-violet-500/30",
    violet: "bg-violet-950/50 text-violet-300 border-violet-500/30",
    cyan: "bg-cyan-950/50 text-cyan-300 border-cyan-500/30",
    outline: "bg-transparent text-slate-400 border-white/10",
  };

  const dotColors = {
    default: "bg-slate-400",
    success: "bg-emerald-400 animate-pulse",
    emerald: "bg-emerald-400 animate-pulse",
    warning: "bg-amber-400 animate-pulse",
    danger: "bg-rose-400 animate-pulse",
    purple: "bg-violet-400 animate-pulse",
    violet: "bg-violet-400 animate-pulse",
    cyan: "bg-cyan-400 animate-pulse",
    outline: "bg-slate-500",
  };

  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[11px] font-mono border backdrop-blur-md transition-colors",
        variantStyles[variant],
        className
      )}
      {...props}
    >
      {dot && <span className={cn("w-1.5 h-1.5 rounded-full", dotColors[variant])} />}
      <span>{children}</span>
    </div>
  );
}
