"use client";

import React, { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Layers,
  Search,
  Zap,
  ShieldAlert,
  Cpu,
  BarChart3,
  Flame,
  Globe,
  Radio,
  Lock,
  GitBranch,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { API_BASE_URL } from "@/lib/api";

export function Header() {
  const pathname = usePathname();
  const [gatewayStatus, setGatewayStatus] = useState<"live" | "offline">("offline");
  const [epoch, setEpoch] = useState<number>(1);
  const [activeTenant, setActiveTenant] = useState<string>("tenant-enterprise-fintech");

  useEffect(() => {
    // Check local gateway status & live vector clock
    const checkGateway = async () => {
      try {
        const res = await fetch(`${API_BASE_URL.replace(/\/+$/, "")}/health`, {
          cache: "no-store",
          signal: AbortSignal.timeout(1500),
        });
        if (res.ok) {
          setGatewayStatus("live");
          try {
            const clusterRes = await fetch(`${API_BASE_URL.replace(/\/+$/, "")}/v1/cluster/status`, {
              cache: "no-store",
            });
            if (clusterRes.ok) {
              const clusterData = await clusterRes.json();
              const epochs = Object.values(clusterData.vector_clock || {}) as number[];
              if (epochs.length > 0) {
                setEpoch(epochs[0]);
              }
            }
          } catch {}
        } else {
          setGatewayStatus("offline");
        }
      } catch {
        setGatewayStatus("offline");
      }
    };
    checkGateway();
    const interval = setInterval(checkGateway, 10000);
    return () => clearInterval(interval);
  }, []);

  const navItems = [
    { name: "Overview", href: "/", icon: Activity },
    { name: "Cache Explorer", href: "/explorer", icon: Search },
    { name: "Semantic Studio", href: "/knowledge", icon: Layers },
    { name: "Execution DAG", href: "/execution", icon: Cpu },
    { name: "FinOps ROI", href: "/finops", icon: BarChart3 },
    { name: "Learning Control", href: "/learning", icon: GitBranch },
    { name: "Governance", href: "/governance", icon: Flame },
    { name: "Security", href: "/security", icon: Lock },
  ];

  return (
    <header className="sticky top-0 z-50 w-full border-b border-white/[0.08] bg-slate-950/80 backdrop-blur-2xl">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Left Brand Identity */}
          <div className="flex items-center gap-6">
            <Link href="/" className="flex items-center gap-3 group">
              <div className="relative flex items-center justify-center w-9 h-9 rounded-xl bg-gradient-to-tr from-cyan-500 via-indigo-600 to-violet-600 shadow-glow p-0.5 transition-transform duration-300 group-hover:scale-105">
                <div className="w-full h-full bg-slate-950 rounded-[10px] flex items-center justify-center">
                  <Zap className="w-4 h-4 text-cyan-400 fill-cyan-400/20" />
                </div>
              </div>
              <div className="flex flex-col">
                <div className="flex items-center gap-2">
                  <span className="font-bold text-base tracking-tight text-white group-hover:text-cyan-300 transition-colors">
                    SUNBORN
                  </span>
                  <Badge variant="cyan" className="px-1.5 py-0 text-[10px]">
                    L7 FAANG
                  </Badge>
                </div>
                <span className="text-[10px] font-mono text-slate-400">
                  Semantic Execution Platform
                </span>
              </div>
            </Link>

            {/* Live Gateway Telemetry Pill */}
            <div className="hidden lg:flex items-center gap-3 pl-4 border-l border-white/[0.08]">
              <div className="flex items-center gap-2 px-2.5 py-1 rounded-full bg-slate-900/90 border border-white/10 text-xs font-mono">
                <Radio
                  className={cn(
                    "w-3 h-3",
                    gatewayStatus === "live"
                      ? "text-emerald-400 animate-pulse"
                      : "text-amber-400"
                  )}
                />
                <span className="text-slate-300">GATEWAY:</span>
                <span
                  className={cn(
                    "font-bold",
                    gatewayStatus === "live" ? "text-emerald-400" : "text-amber-400"
                  )}
                >
                  {gatewayStatus === "live" ? "ACTIVE 8080" : "OFFLINE"}
                </span>
              </div>

              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-slate-900/90 border border-white/10 text-xs font-mono text-slate-400">
                <Globe className="w-3 h-3 text-violet-400" />
                <span>EPOCH:</span>
                <span className="text-white font-semibold">#{epoch}</span>
              </div>
            </div>
          </div>

          {/* Center Navigation Links */}
          <nav className="hidden md:flex items-center gap-1">
            {navItems.map((item) => {
              const Icon = item.icon;
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "relative flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-200",
                    isActive
                      ? "text-white bg-white/[0.08] shadow-sm border border-white/10"
                      : "text-slate-400 hover:text-slate-200 hover:bg-white/[0.03]"
                  )}
                >
                  <Icon
                    className={cn(
                      "w-3.5 h-3.5",
                      isActive ? "text-cyan-400" : "text-slate-400"
                    )}
                  />
                  <span>{item.name}</span>
                  {isActive && (
                    <span className="absolute -bottom-[17px] left-0 right-0 h-0.5 bg-gradient-to-r from-transparent via-cyan-400 to-transparent" />
                  )}
                </Link>
              );
            })}
          </nav>

          {/* Right Controls */}
          <div className="flex items-center gap-3">
            {/* Tenant Selector */}
            <div className="hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
              <span className="text-slate-400">TENANT:</span>
              <select
                aria-label="Active Tenant Scope"
                value={activeTenant}
                onChange={(e) => setActiveTenant(e.target.value)}
                className="bg-transparent text-cyan-300 font-semibold focus:outline-none cursor-pointer"
              >
                <option value="tenant-enterprise-fintech" className="bg-slate-950 text-white">
                  fintech-prod
                </option>
                <option value="tenant-healthcare-ai" className="bg-slate-950 text-white">
                  healthcare-prod
                </option>
                <option value="tenant-autonomous-agent" className="bg-slate-950 text-white">
                  agent-cluster
                </option>
              </select>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}
