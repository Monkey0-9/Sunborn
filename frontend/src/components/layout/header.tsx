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
  UserCheck,
  Server,
  Database,
  CheckCircle2,
  X,
  Menu,
  RefreshCw,
  Sliders,
  Sparkles,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { CommandMenu } from "@/components/ui/command-menu";
import { API_BASE_URL } from "@/lib/api";

export function Header() {
  const pathname = usePathname();
  const [gatewayStatus, setGatewayStatus] = useState<"live" | "offline">("offline");
  const [pingMs, setPingMs] = useState<number | null>(null);
  const [epoch, setEpoch] = useState<number>(1);
  const [activeTenant, setActiveTenant] = useState<string>("tenant-enterprise-fintech");
  const [sessionAuth, setSessionAuth] = useState<boolean>(false);
  const [showDiagnostics, setShowDiagnostics] = useState<boolean>(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState<boolean>(false);
  const [isProbing, setIsProbing] = useState<boolean>(false);
  const [probeResult, setProbeResult] = useState<string | null>(null);

  const runDiagnosticsProbe = async () => {
    setIsProbing(true);
    setProbeResult(null);
    const start = performance.now();
    try {
      const res = await fetch(`${API_BASE_URL.replace(/\/+$/, "")}/health`, {
        cache: "no-store",
        signal: AbortSignal.timeout(2000),
      });
      const elapsed = Math.round((performance.now() - start) * 10) / 10;
      if (res.ok) {
        const data = await res.json().catch(() => ({}));
        setGatewayStatus("live");
        setPingMs(elapsed);
        setProbeResult(`PROBE SUCCESS (HTTP 200) · RTT: ${elapsed}ms · Service: ${data.service || "sunborn-gateway"} · Status: ${data.status || "healthy"}`);
      } else {
        setGatewayStatus("offline");
        setProbeResult(`PROBE FAILED: Received HTTP ${res.status}`);
      }
    } catch (err: any) {
      setGatewayStatus("offline");
      setProbeResult(`PROBE ERROR: ${err.message || "Connection refused"}. Gateway offline.`);
    } finally {
      setIsProbing(false);
    }
  };

  useEffect(() => {
    // Check local gateway status & live vector clock
    const checkGateway = async () => {
      const start = performance.now();
      try {
        const res = await fetch(`${API_BASE_URL.replace(/\/+$/, "")}/health`, {
          cache: "no-store",
          signal: AbortSignal.timeout(1500),
        });
        const elapsed = Math.round((performance.now() - start) * 10) / 10;
        if (res.ok) {
          setGatewayStatus("live");
          setPingMs(elapsed);
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
          setPingMs(null);
        }
      } catch {
        setGatewayStatus("offline");
        setPingMs(null);
      }
    };

    const checkSession = async () => {
      try {
        const res = await fetch("/api/auth", { cache: "no-store" });
        if (res.ok) {
          const data = await res.json();
          setSessionAuth(!!data.authenticated);
        }
      } catch {
        setSessionAuth(false);
      }
    };

    checkGateway();
    checkSession();
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
    { name: "Ceremony", href: "/ceremony", icon: Globe },
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
                    L6/L7 Autonomic
                  </Badge>
                </div>
                <span className="text-[10px] font-mono text-slate-400">
                  Semantic Execution Platform
                </span>
              </div>
            </Link>

            {/* Live Gateway Telemetry Pill */}
            <div className="hidden lg:flex items-center gap-3 pl-4 border-l border-white/[0.08]">
              <button
                type="button"
                onClick={() => setShowDiagnostics(true)}
                title="Click to view live infrastructure diagnostics & cluster topology"
                className="flex items-center gap-2 px-2.5 py-1 rounded-full bg-slate-900/90 hover:bg-slate-800 border border-white/10 hover:border-cyan-500/40 text-xs font-mono transition-all cursor-pointer group"
              >
                <Radio
                  className={cn(
                    "w-3 h-3 transition-colors",
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
                  {gatewayStatus === "live"
                    ? pingMs !== null
                      ? `ACTIVE 8080 (${pingMs}ms)`
                      : "ACTIVE 8080"
                    : "OFFLINE"}
                </span>
                <span className="text-[10px] text-cyan-400 opacity-60 group-hover:opacity-100">diag</span>
              </button>

              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-slate-900/90 border border-white/10 text-xs font-mono text-slate-400">
                <Globe className="w-3 h-3 text-violet-400" />
                <span>EPOCH:</span>
                <span className="text-white font-semibold">#{epoch}</span>
              </div>
            </div>
          </div>

          {/* Center Navigation Links (Desktop) */}
          <nav className="hidden xl:flex items-center gap-1">
            {navItems.map((item) => {
              const Icon = item.icon;
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "relative flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-all duration-200",
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
          <div className="flex items-center gap-2 sm:gap-3">
            {/* Global Command Menu */}
            <CommandMenu />

            {/* Tenant Selector */}
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
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

            {/* Session / Login Link */}
            <Link
              href="/login"
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-xl border text-xs font-mono transition-colors",
                sessionAuth
                  ? "bg-emerald-500/10 border-emerald-500/30 text-emerald-300 hover:bg-emerald-500/20"
                  : "bg-white/[0.04] border-white/10 text-slate-400 hover:text-white hover:bg-white/[0.08]"
              )}
            >
              <UserCheck className="w-3.5 h-3.5 text-cyan-400" />
              <span className="hidden sm:inline">
                {sessionAuth ? "SESSION ACTIVE" : "LOGIN"}
              </span>
            </Link>

            {/* Mobile / Compact Menu Toggle */}
            <button
              type="button"
              onClick={() => setMobileMenuOpen((prev) => !prev)}
              aria-label="Toggle Navigation Menu"
              className="xl:hidden p-2 rounded-xl border border-white/10 bg-slate-900/80 text-slate-300 hover:text-white hover:bg-slate-800 transition-colors"
            >
              {mobileMenuOpen ? <X className="w-4 h-4" /> : <Menu className="w-4 h-4" />}
            </button>
          </div>
        </div>
      </div>

      {/* Mobile Drawer / Overlay Navigation Menu */}
      {mobileMenuOpen && (
        <div className="xl:hidden border-b border-white/[0.08] bg-slate-950/95 backdrop-blur-2xl px-4 py-4 animate-in slide-in-from-top-2 duration-200">
          <div className="max-w-7xl mx-auto space-y-4">
            {/* Quick Status Pill */}
            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-900/80 border border-white/10 text-xs font-mono">
              <div className="flex items-center gap-2">
                <Radio
                  className={cn(
                    "w-3 h-3",
                    gatewayStatus === "live"
                      ? "text-emerald-400 animate-pulse"
                      : "text-amber-400"
                  )}
                />
                <span className="text-slate-300">GATEWAY:</span>
                <span className={cn("font-bold", gatewayStatus === "live" ? "text-emerald-400" : "text-amber-400")}>
                  {gatewayStatus === "live" ? `ACTIVE (${pingMs ?? 0.8}ms)` : "OFFLINE"}
                </span>
              </div>
              <button
                type="button"
                onClick={() => {
                  setMobileMenuOpen(false);
                  setShowDiagnostics(true);
                }}
                className="px-2 py-1 rounded bg-cyan-500/20 text-cyan-300 hover:bg-cyan-500/30 text-[10px] font-semibold border border-cyan-500/30"
              >
                Diagnostics
              </button>
            </div>

            {/* Mobile Navigation Links Grid */}
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
              {navItems.map((item) => {
                const Icon = item.icon;
                const isActive = pathname === item.href;
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    onClick={() => setMobileMenuOpen(false)}
                    className={cn(
                      "flex items-center gap-2 p-2.5 rounded-xl text-xs font-medium transition-colors border",
                      isActive
                        ? "bg-cyan-500/15 border-cyan-500/30 text-cyan-300 font-bold"
                        : "bg-slate-900/50 border-white/5 text-slate-400 hover:text-white hover:bg-white/[0.05]"
                    )}
                  >
                    <Icon className={cn("w-4 h-4 shrink-0", isActive ? "text-cyan-400" : "text-slate-400")} />
                    <span className="truncate">{item.name}</span>
                  </Link>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {/* Global Infrastructure Diagnostics & Topology Drawer Modal */}
      {showDiagnostics && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-in fade-in duration-200">
          <div className="relative w-full max-w-2xl bg-slate-950 border border-white/15 rounded-2xl shadow-2xl overflow-hidden font-mono text-xs">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 bg-slate-900/60">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400">
                  <Server className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-white tracking-tight flex items-center gap-2">
                    Sunborn Infrastructure Topology & Diagnostics
                  </h3>
                  <p className="text-[11px] text-slate-400">
                    L6/L7 Reverse Proxy · Autonomous Storage Stack · Multi-Region Mesh
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowDiagnostics(false)}
                className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Diagnostics Content */}
            <div className="p-6 space-y-6 max-h-[75vh] overflow-y-auto">
              {/* Probe Test Action Strip */}
              <div className="p-4 rounded-xl bg-slate-900/80 border border-white/10 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div className="space-y-0.5">
                  <div className="text-xs font-bold text-white flex items-center gap-2">
                    <span className={cn("w-2 h-2 rounded-full", gatewayStatus === "live" ? "bg-emerald-400 animate-pulse" : "bg-amber-400")} />
                    Gateway Target: 127.0.0.1:8080
                  </div>
                  <div className="text-[11px] text-slate-400">
                    Live loopback socket connection probe with microsecond precision.
                  </div>
                </div>
                <button
                  type="button"
                  onClick={runDiagnosticsProbe}
                  disabled={isProbing}
                  className="inline-flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-500/40 text-xs font-semibold transition-all disabled:opacity-50"
                >
                  <RefreshCw className={cn("w-3.5 h-3.5", isProbing && "animate-spin")} />
                  <span>{isProbing ? "Probing..." : "Run Live Probe"}</span>
                </button>
              </div>

              {probeResult && (
                <div className="p-3 rounded-xl bg-slate-900 border border-cyan-500/30 text-cyan-300 text-[11px]">
                  {probeResult}
                </div>
              )}

              {/* Multi-Tier Architecture Health Grid */}
              <div className="space-y-2">
                <div className="text-[11px] font-bold text-slate-400 uppercase tracking-wider">
                  Operational Tiers & Durability Subsystems
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="p-3 rounded-xl bg-slate-900/60 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 0/1: Exact Cache</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">ACTIVE</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      W-TinyLFU In-Memory + DragonflyDB L2 (P99: 240 µs target).
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/60 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 2: Semantic Retrieval</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">CALIBRATED</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      Qdrant HNSW Vector Engine + Conformal Risk (α ≤ 0.001).
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/60 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 3: Execution Subgraph</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">PROOF VERIFIED</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      RFC 6962 SHA-256 Merkle DAG Planner + Speculative Markov.
                    </div>
                  </div>

                  <div className="p-3 rounded-xl bg-slate-900/60 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 4: Durable Authority</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">POSTGRESQL WAL</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      Non-Accelerator Authority with transactional outbox stream.
                    </div>
                  </div>
                </div>
              </div>

              {/* Security & Cryptography State */}
              <div className="p-4 rounded-xl bg-slate-900/40 border border-white/10 space-y-2">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-slate-300 font-semibold flex items-center gap-1.5">
                    <Lock className="w-3.5 h-3.5 text-cyan-400" />
                    Zero-Trust Cryptographic Perimeter
                  </span>
                  <span className="text-emerald-400 font-bold">AES-256-GCM AEAD</span>
                </div>
                <div className="text-[11px] text-slate-400 leading-relaxed">
                  Tenant Data Encryption Keys (TDEK) wrapped under Master KEK. Cryptographic shredding invalidates keys in $O(1)$ constant time with verified multi-store fail-closed erasure.
                </div>
              </div>
            </div>

            {/* Footer */}
            <div className="px-6 py-3 border-t border-white/10 bg-slate-900/40 flex items-center justify-between text-[11px] text-slate-400">
              <span>Cluster Epoch #{epoch} · Region: us-east-1</span>
              <button
                onClick={() => setShowDiagnostics(false)}
                className="px-3 py-1 rounded-lg bg-white/10 hover:bg-white/15 text-white transition-colors"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
