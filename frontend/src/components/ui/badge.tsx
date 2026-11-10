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
    default: "bg-slate-800/80 text-slate-300 border-white/10",
    success: "bg-emerald-950/60 text-emerald-300 border-emerald-500/40 shadow-glow-emerald/30",
    emerald: "bg-emerald-950/60 text-emerald-300 border-emerald-500/40 shadow-glow-emerald/30",
    warning: "bg-amber-950/60 text-amber-300 border-amber-500/40 shadow-glow-amber/30",
    danger: "bg-rose-950/60 text-rose-300 border-rose-500/40 shadow-glow-rose/30",
    purple: "bg-violet-950/60 text-violet-300 border-violet-500/40 shadow-glow-violet/30",
    violet: "bg-violet-950/60 text-violet-300 border-violet-500/40 shadow-glow-violet/30",
    cyan: "bg-cyan-950/60 text-cyan-300 border-cyan-500/40 shadow-glow/30",
    outline: "bg-white/[0.03] text-slate-300 border-white/15",
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
    outline: "bg-slate-400",
  };

  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 px-3 py-0.5 rounded-full text-[11px] font-mono font-medium border backdrop-blur-xl transition-all duration-200 select-none",
        variantStyles[variant],
        className
      )}
      {...props}
    >
      {dot && (
        <span className="relative flex h-1.5 w-1.5">
          <span className={cn("animate-ping absolute inline-flex h-full w-full rounded-full opacity-75", dotColors[variant])} />
          <span className={cn("relative inline-flex rounded-full h-1.5 w-1.5", dotColors[variant])} />
        </span>
      )}
      <span>{children}</span>
    </div>
  );
}
