"use client";

import React from "react";
import { cn } from "@/lib/utils";

interface TextShimmerProps {
  children: string;
  as?: React.ElementType;
  className?: string;
  duration?: number;
  spread?: number;
}

export function TextShimmer({
  children,
  as: Component = "p",
  className,
  duration = 2.5,
}: TextShimmerProps) {
  return (
    <Component
      className={cn(
        "relative inline-block bg-[linear-gradient(110deg,#94a3b8,45%,#38bdf8,55%,#94a3b8)] bg-[length:200%_100%] bg-clip-text text-transparent animate-shimmer",
        className
      )}
      style={{
        animationDuration: `${duration}s`,
      }}
    >
      {children}
    </Component>
  );
}
