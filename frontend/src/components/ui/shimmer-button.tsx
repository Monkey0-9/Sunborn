"use client";

import React, { CSSProperties } from "react";
import { cn } from "@/lib/utils";

export interface ShimmerButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  shimmerColor?: string;
  shimmerSize?: string;
  borderRadius?: string;
  shimmerDuration?: string;
  background?: string;
  className?: string;
  children?: React.ReactNode;
}

export const ShimmerButton = React.forwardRef<HTMLButtonElement, ShimmerButtonProps>(
  (
    {
      shimmerColor = "#22d3ee",
      shimmerSize = "0.08em",
      shimmerDuration = "2.5s",
      borderRadius = "0.75rem",
      background = "rgba(15, 23, 42, 0.95)",
      className,
      children,
      ...props
    },
    ref,
  ) => {
    return (
      <button
        style={
          {
            "--spread": "90deg",
            "--shimmer-color": shimmerColor,
            "--radius": borderRadius,
            "--speed": shimmerDuration,
            "--cut": shimmerSize,
            "--bg": background,
          } as CSSProperties
        }
        className={cn(
          "group relative z-0 flex cursor-pointer items-center justify-center overflow-hidden whitespace-nowrap border border-cyan-500/30 px-5 py-2.5 font-medium text-white shadow-glow [background:var(--bg)] [border-radius:var(--radius)]",
          "transform-gpu transition-all duration-300 ease-in-out hover:border-cyan-400/60 hover:shadow-cyan-500/20 active:translate-y-px",
          className,
        )}
        ref={ref}
        {...props}
      >
        <div className="-z-30 absolute inset-0 overflow-visible [container-type:size]">
          <div className="absolute inset-0 h-[100cqh] animate-shimmer [aspect-ratio:1] opacity-70">
            <div className="absolute -inset-full w-auto rotate-0 [background:conic-gradient(from_0deg,transparent_0,var(--shimmer-color)_90deg,transparent_90deg)] animate-spin-slow" />
          </div>
        </div>
        <div className="-z-20 absolute inset-0 [background:var(--bg)] [border-radius:var(--radius)] opacity-95" />
        <span className="relative z-10 flex items-center gap-2">{children}</span>
      </button>
    );
  },
);

ShimmerButton.displayName = "ShimmerButton";
