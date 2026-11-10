import React from "react";
import { cn } from "@/lib/utils";

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  glow?: "cyan" | "violet" | "emerald" | "amber" | "rose" | "none";
}

export const Card = React.forwardRef<HTMLDivElement, CardProps>(
  ({ className, glow = "none", children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "relative rounded-2xl sm:rounded-3xl border border-white/[0.08] bg-gradient-to-br from-[#0c1328]/80 via-[#070b18]/90 to-[#030612]/95 p-6 backdrop-blur-2xl shadow-glass transition-all duration-300 overflow-hidden",
          "before:absolute before:inset-0 before:rounded-2xl sm:before:rounded-3xl before:bg-gradient-to-b before:from-white/[0.06] before:to-transparent before:pointer-events-none",
          glow === "cyan" && "hover:border-cyan-500/40 hover:shadow-glow",
          glow === "violet" && "hover:border-violet-500/40 hover:shadow-glow-violet",
          glow === "emerald" && "hover:border-emerald-500/40 hover:shadow-glow-emerald",
          glow === "amber" && "hover:border-amber-500/40 hover:shadow-glow-amber",
          glow === "rose" && "hover:border-rose-500/40 hover:shadow-glow-rose",
          glow === "none" && "hover:border-white/[0.18]",
          className
        )}
        {...props}
      >
        {children}
      </div>
    );
  }
);
Card.displayName = "Card";

export const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex flex-col space-y-1.5 pb-4", className)} {...props} />
  )
);
CardHeader.displayName = "CardHeader";

export const CardTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3
      ref={ref}
      className={cn("text-base sm:text-lg font-bold leading-none tracking-tight text-white font-sans", className)}
      {...props}
    />
  )
);
CardTitle.displayName = "CardTitle";

export const CardDescription = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLParagraphElement>>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn("text-xs text-slate-400 font-mono tracking-tight", className)} {...props} />
  )
);
CardDescription.displayName = "CardDescription";

export const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("pt-0", className)} {...props} />
  )
);
CardContent.displayName = "CardContent";
