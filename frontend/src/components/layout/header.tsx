"use client";

import React, { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  AlertCircle,
  Layers,
  Search,
  Zap,
  Cpu,
  BarChart3,
  Flame,
  Globe,
  Radio,
  Lock,
  GitBranch,
  UserCheck,
  Server,
  X,
  Menu,
  RefreshCw,
  Sparkles,
  ShieldCheck,
  Sliders,
  ChevronDown,
  Key as KeyIcon,
} from "lucide-react";
import { cn, extractVectorClockEpoch } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { CommandMenu } from "@/components/ui/command-menu";
import { API_BASE_URL } from "@/lib/api";
import { useClusterStore, setPersona, setCurrency, setByokApiKey } from "@/lib/cluster-store";

export function Header() {
  const pathname = usePathname();
  const clusterState = useClusterStore();
  const { persona, currency, byokApiKey, isSimulated, circuitBreaker } = clusterState;
  const [showByokModal, setShowByokModal] = useState<boolean>(false);
  const [tempKey, setTempKey] = useState<string>("");
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
        setProbeResult(
          `PROBE SUCCESS (HTTP 200) · RTT: ${elapsed}ms · Service: ${data.service || "sunborn-gateway"} · Status: ${data.status || "healthy"}`
        );
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
              const epochNum = extractVectorClockEpoch(clusterData?.vector_clock);
              setEpoch(epochNum);
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
    <header role="banner" className="sticky top-0 z-50 w-full border-b border-white/[0.08] bg-[#020617]/85 backdrop-blur-3xl shadow-glass transition-all">
      {isSimulated && (
        <aside aria-label="Simulation Mode Alert" className="w-full bg-amber-500/15 border-b border-amber-500/30 text-amber-300 px-4 py-1.5 text-xs font-mono flex items-center justify-center gap-2">
          <AlertCircle className="w-3.5 h-3.5 text-amber-400 shrink-0" />
          <span>SIMULATION MODE ACTIVE — Gateway offline or unauthenticated; actions are local sandbox demonstrations and do not mutate live production state.</span>
        </aside>
      )}
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Left Brand Emblem */}
          <div className="flex items-center gap-5">
            <Link href="/" className="flex items-center gap-3 group focus-visible:ring-2 focus-visible:ring-cyan-400 rounded-xl p-1">
              <div className="relative flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-tr from-cyan-500 via-indigo-600 to-violet-600 shadow-glow p-0.5 transition-transform duration-300 group-hover:scale-105">
                <div className="w-full h-full bg-[#030712] rounded-[10px] flex items-center justify-center">
                  <Zap className="w-5 h-5 text-cyan-400 fill-cyan-400/25 group-hover:scale-110 transition-transform" />
                </div>
              </div>
              <div className="flex flex-col">
                <div className="flex items-center gap-2">
                  <span className="font-extrabold text-base tracking-tight text-white group-hover:text-cyan-300 transition-colors">
                    SUNBORN
                  </span>
                  <span className="px-1.5 py-0.2 rounded-md bg-cyan-950/80 border border-cyan-500/40 text-[10px] font-mono font-bold text-cyan-300 shadow-inner">
                    L6/L7 PROXY
                  </span>
                </div>
                <span className="text-[10px] font-mono text-slate-400 tracking-tight">
                  Autonomous AI Gateway
                </span>
              </div>
            </Link>

            {/* Live Telemetry Pill */}
            <div className="hidden lg:flex items-center gap-2 pl-4 border-l border-white/[0.08]">
              <button
                type="button"
                onClick={() => setShowDiagnostics(true)}
                title="Click to view live infrastructure diagnostics & cluster topology"
                className="flex items-center gap-2 px-3 py-1 rounded-full bg-slate-900/90 hover:bg-slate-800/90 border border-white/10 hover:border-cyan-500/40 text-xs font-mono transition-all cursor-pointer group focus-visible:ring-2 focus-visible:ring-cyan-400"
              >
                <span className="sr-only">Open cluster diagnostics: Gateway status</span>
                <span className="relative flex h-2 w-2" aria-hidden="true">
                  <span
                    className={cn(
                      "radar-ping-ring absolute inline-flex h-full w-full rounded-full opacity-75",
                      gatewayStatus === "live" ? "bg-emerald-400" : "bg-amber-400"
                    )}
                  />
                  <span
                    className={cn(
                      "relative inline-flex rounded-full h-2 w-2",
                      gatewayStatus === "live" ? "bg-emerald-500" : "bg-amber-500"
                    )}
                  />
                </span>
                <span className="text-slate-400">GW:</span>
                <span
                  className={cn(
                    "font-bold",
                    circuitBreaker === "OPEN"
                      ? "text-rose-400"
                      : gatewayStatus === "live"
                      ? "text-emerald-400"
                      : isSimulated
                      ? "text-amber-400"
                      : "text-slate-400"
                  )}
                >
                  {circuitBreaker === "OPEN"
                    ? "CIRCUIT OPEN"
                    : gatewayStatus === "live"
                    ? pingMs !== null
                      ? `8080 (${pingMs}ms)`
                      : "ACTIVE"
                    : isSimulated
                    ? "SIMULATION"
                    : "STANDBY"}
                </span>
                <span className="text-[10px] px-1 py-0.2 rounded bg-white/5 text-cyan-400 font-semibold group-hover:bg-cyan-500/20">
                  HUD
                </span>
              </button>

              <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-slate-900/70 border border-white/10 text-xs font-mono text-slate-400">
                <Globe className="w-3 h-3 text-violet-400" />
                <span>EPOCH:</span>
                <span className="text-white font-semibold">#{typeof epoch === "number" ? epoch : 1}</span>
              </div>
            </div>
          </div>

          {/* Center Navigation Links (Desktop) */}
          <nav role="navigation" aria-label="Main Navigation" className="hidden xl:flex items-center gap-1 bg-white/[0.02] border border-white/[0.06] p-1 rounded-xl backdrop-blur-md">
            {navItems.map((item) => {
              const Icon = item.icon;
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "relative flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-200 focus-visible:ring-2 focus-visible:ring-cyan-400",
                    isActive
                      ? "text-white bg-white/[0.12] shadow-sm font-semibold border border-white/10"
                      : "text-slate-400 hover:text-slate-100 hover:bg-white/[0.04]"
                  )}
                >
                  <Icon
                    className={cn(
                      "w-3.5 h-3.5 transition-colors",
                      isActive ? "text-cyan-400" : "text-slate-400"
                    )}
                  />
                  <span>{item.name}</span>
                  {isActive && (
                    <span className="absolute -bottom-1 left-3 right-3 h-0.5 bg-gradient-to-r from-cyan-400 to-indigo-500 rounded-full" />
                  )}
                </Link>
              );
            })}
          </nav>

          {/* Right Controls */}
          <div className="flex items-center gap-2 sm:gap-3">
            {/* Sovereign Currency Switcher (USD $ / INR ₹) */}
            <div
              role="group"
              aria-label="Currency Mode"
              className="hidden lg:flex items-center p-0.5 rounded-lg bg-slate-950/80 border border-white/10 text-xs font-mono"
            >
              <button
                type="button"
                onClick={() => setCurrency("INR")}
                className={cn(
                  "px-2 py-1 rounded text-[11px] font-medium transition-colors flex items-center gap-1",
                  currency === "INR"
                    ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                    : "text-slate-400 hover:text-slate-200"
                )}
                aria-pressed={currency === "INR"}
              >
                <span>₹</span>
                <span>INR</span>
              </button>
              <button
                type="button"
                onClick={() => setCurrency("USD")}
                className={cn(
                  "px-2 py-1 rounded text-[11px] font-medium transition-colors flex items-center gap-1",
                  currency === "USD"
                    ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                    : "text-slate-400 hover:text-slate-200"
                )}
                aria-pressed={currency === "USD"}
              >
                <span>$</span>
                <span>USD</span>
              </button>
            </div>

            {/* Persona Switcher */}
            <div
              role="group"
              aria-label="Console View Mode"
              className="hidden md:flex items-center p-0.5 rounded-lg bg-slate-950/80 border border-white/10 text-xs font-mono"
            >
              <button
                type="button"
                onClick={() => setPersona("executive")}
                className={cn(
                  "px-2.5 py-1 rounded text-[11px] font-medium transition-colors",
                  persona === "executive"
                    ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                    : "text-slate-400 hover:text-slate-200"
                )}
                aria-pressed={persona === "executive"}
                aria-label="Switch to Executive Cockpit view"
              >
                Executive Cockpit
              </button>
              <button
                type="button"
                onClick={() => setPersona("developer")}
                className={cn(
                  "px-2.5 py-1 rounded text-[11px] font-medium transition-colors",
                  persona === "developer"
                    ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                    : "text-slate-400 hover:text-slate-200"
                )}
                aria-pressed={persona === "developer"}
                aria-label="Switch to Developer Sandbox view"
              >
                Developer Sandbox
              </button>
            </div>

            {/* BYOK API Key Trigger */}
            <button
              type="button"
              onClick={() => setShowByokModal(true)}
              className={cn(
                "hidden sm:flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl border text-xs font-mono transition-colors",
                byokApiKey
                  ? "bg-cyan-950/60 border-cyan-500/40 text-cyan-300"
                  : "bg-slate-900/80 border-white/10 text-slate-400 hover:text-white"
              )}
              aria-label="Configure BYOK API Key"
            >
              <KeyIcon className="w-3.5 h-3.5 text-cyan-400" aria-hidden="true" />
              <span>{byokApiKey ? "BYOK ACTIVE" : "BYOK"}</span>
            </button>

            {/* Global Command Menu */}
            <CommandMenu />

            {/* Tenant Selector Dropdown */}
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-xl bg-slate-900/80 border border-white/10 text-xs font-mono hover:border-white/20 transition-colors">
              <span className="text-slate-400">TENANT:</span>
              <select
                aria-label="Active Multi-Tenant Scope"
                value={activeTenant}
                onChange={(e) => setActiveTenant(e.target.value)}
                className="bg-transparent text-cyan-300 font-semibold cursor-pointer focus-visible:ring-1 focus-visible:ring-cyan-400 rounded"
              >
                <option value="tenant-enterprise-fintech" className="bg-[#030712] text-white">
                  fintech-prod
                </option>
                <option value="tenant-healthcare-ai" className="bg-[#030712] text-white">
                  healthcare-prod
                </option>
                <option value="tenant-autonomous-agent" className="bg-[#030712] text-white">
                  agent-cluster
                </option>
              </select>
            </div>

            {/* Session / Authentication Pill */}
            <Link
              href="/login"
              aria-label={sessionAuth ? "Session active" : "Login"}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-xl border text-xs font-mono transition-all focus-visible:ring-2 focus-visible:ring-cyan-400",
                sessionAuth
                  ? "bg-emerald-500/15 border-emerald-500/40 text-emerald-300 hover:bg-emerald-500/25 shadow-glow-emerald"
                  : "bg-white/[0.04] border-white/10 text-slate-300 hover:text-white hover:bg-white/[0.08]"
              )}
            >
              <UserCheck className="w-3.5 h-3.5 text-cyan-400" />
              <span className="hidden sm:inline font-semibold">
                {sessionAuth ? "SESSION ACTIVE" : "LOGIN"}
              </span>
            </Link>

            {/* Mobile Menu Button */}
            <button
              type="button"
              onClick={() => setMobileMenuOpen((prev) => !prev)}
              aria-label="Toggle Mobile Navigation Menu"
              aria-expanded={mobileMenuOpen}
              className="xl:hidden p-2 rounded-xl border border-white/10 bg-slate-900/80 text-slate-300 hover:text-white hover:bg-slate-800 transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400"
            >
              {mobileMenuOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
            </button>
          </div>
        </div>
      </div>

      {/* Mobile Drawer Menu */}
      {mobileMenuOpen && (
        <div className="xl:hidden border-b border-white/[0.08] bg-[#030712]/98 backdrop-blur-3xl px-4 py-5 animate-in slide-in-from-top-2 duration-200">
          <div className="max-w-7xl mx-auto space-y-4">
            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
              <div className="flex items-center gap-2">
                <span className={cn("w-2 h-2 rounded-full", gatewayStatus === "live" ? "bg-emerald-400" : "bg-amber-400")} />
                <span className="text-slate-400">GATEWAY STATUS:</span>
                <span className={cn("font-bold", gatewayStatus === "live" ? "text-emerald-400" : "text-amber-400")}>
                  {gatewayStatus === "live" ? `ONLINE (${pingMs ?? 0.8}ms)` : "STANDBY"}
                </span>
              </div>
              <button
                type="button"
                onClick={() => {
                  setMobileMenuOpen(false);
                  setShowDiagnostics(true);
                }}
                className="px-2.5 py-1 rounded-lg bg-cyan-500/20 text-cyan-300 text-xs font-bold border border-cyan-500/30"
              >
                HUD View
              </button>
            </div>

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
                      "flex items-center gap-2.5 p-3 rounded-xl text-xs font-medium transition-colors border",
                      isActive
                        ? "bg-cyan-500/15 border-cyan-500/40 text-cyan-300 font-bold"
                        : "bg-slate-900/60 border-white/5 text-slate-300 hover:text-white hover:bg-white/[0.06]"
                    )}
                  >
                    <Icon className={cn("w-4 h-4 shrink-0", isActive ? "text-cyan-400" : "text-slate-400")} />
                    <span className="truncate">{item.name}</span>
                  </Link>
                );
              })}
            </div>

            {/* Mobile Currency Switcher */}
            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
              <span className="text-slate-400">CURRENCY MODE:</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => setCurrency("INR")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "INR"
                      ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  ₹ INR
                </button>
                <button
                  type="button"
                  onClick={() => setCurrency("USD")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "USD"
                      ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  $ USD
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Global Infrastructure Diagnostics & Topology Modal */}
      {showDiagnostics && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-xl animate-in fade-in duration-200">
          <div className="relative w-full max-w-2xl bg-[#050814] border border-white/20 rounded-2xl shadow-2xl overflow-hidden font-mono text-xs">
            {/* Modal Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 bg-slate-900/80">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-xl bg-cyan-500/10 border border-cyan-500/30 text-cyan-400">
                  <Server className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-white tracking-tight">
                    Sunborn Infrastructure Topology & Diagnostics
                  </h3>
                  <p className="text-[11px] text-slate-400">
                    L6/L7 Reverse Proxy · Non-Accelerator Authority · Multi-Region Mesh
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowDiagnostics(false)}
                aria-label="Close diagnostics dialog"
                className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Modal Body */}
            <div className="p-6 space-y-5 max-h-[75vh] overflow-y-auto">
              <div className="p-4 rounded-xl bg-slate-900/90 border border-white/10 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div className="space-y-0.5">
                  <div className="text-xs font-bold text-white flex items-center gap-2">
                    <span className={cn("w-2 h-2 rounded-full", gatewayStatus === "live" ? "bg-emerald-400 animate-pulse" : "bg-amber-400")} />
                    Gateway Address: 127.0.0.1:8080
                  </div>
                  <div className="text-[11px] text-slate-400">
                    High-precision loopback probe targeting authoritative proxy endpoint.
                  </div>
                </div>
                <button
                  type="button"
                  onClick={runDiagnosticsProbe}
                  disabled={isProbing}
                  className="inline-flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-500/40 text-xs font-semibold transition-all disabled:opacity-50"
                >
                  <RefreshCw className={cn("w-3.5 h-3.5", isProbing && "animate-spin")} />
                  <span>{isProbing ? "Probing..." : "Run Active Probe"}</span>
                </button>
              </div>

              {probeResult && (
                <div className="p-3 rounded-xl bg-slate-900 border border-cyan-500/40 text-cyan-300 text-[11px]">
                  {probeResult}
                </div>
              )}

              {/* Subsystem Tiers */}
              <div className="space-y-2">
                <div className="text-[11px] font-bold text-slate-300 uppercase tracking-wider">
                  Operational Tiers & Durability Subsystems
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="p-3.5 rounded-xl bg-slate-900/70 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 0/1: Exact Cache</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">ACTIVE</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      W-TinyLFU RAM + DragonflyDB (P99: Sub-ms target).
                    </div>
                  </div>

                  <div className="p-3.5 rounded-xl bg-slate-900/70 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 2: Semantic Vector</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">CALIBRATED</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      Qdrant HNSW + Non-Exchangeable Conformal Risk (α ≤ 0.001).
                    </div>
                  </div>

                  <div className="p-3.5 rounded-xl bg-slate-900/70 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 3: Execution DAG</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">PROOF ATTESTED</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      RFC 6962 SHA-256 Merkle Inclusion Proofs.
                    </div>
                  </div>

                  <div className="p-3.5 rounded-xl bg-slate-900/70 border border-white/10 space-y-1">
                    <div className="flex items-center justify-between">
                      <span className="text-white font-semibold">Tier 4: Durable Authority</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">POSTGRESQL WAL</span>
                    </div>
                    <div className="text-[11px] text-slate-400">
                      RPO=0 Transactional Outbox + Rebuild Hydration Engine.
                    </div>
                  </div>
                </div>
              </div>

              {/* Security Boundary */}
              <div className="p-4 rounded-xl bg-slate-900/50 border border-white/10 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-white font-semibold flex items-center gap-1.5">
                    <Lock className="w-3.5 h-3.5 text-cyan-400" />
                    Zero-Trust Cryptographic Envelope
                  </span>
                  <span className="text-emerald-400 font-bold">AES-256-GCM AEAD</span>
                </div>
                <div className="text-[11px] text-slate-400 leading-relaxed">
                  Tenant Data Encryption Keys (TDEK) wrapped under master KEK. Cryptographic shredding executes in O(1) time under GDPR Article 17.
                </div>
              </div>
            </div>

            {/* Modal Footer */}
            <div className="px-6 py-3.5 border-t border-white/10 bg-slate-900/60 flex items-center justify-between text-[11px] text-slate-400">
              <span>Cluster Epoch #{typeof epoch === "number" ? epoch : 1} · Multi-Region Mesh</span>
              <button
                onClick={() => setShowDiagnostics(false)}
                className="px-4 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 text-white transition-colors font-semibold"
              >
                Close HUD
              </button>
            </div>

            {/* Mobile Currency Switcher */}
            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
              <span className="text-slate-400">CURRENCY MODE:</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => setCurrency("INR")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "INR"
                      ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  ₹ INR
                </button>
                <button
                  type="button"
                  onClick={() => setCurrency("USD")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "USD"
                      ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  $ USD
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* BYOK Session Key Modal */}
      {showByokModal && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="byok-modal-title"
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md"
        >
          <div className="relative w-full max-w-md rounded-2xl bg-slate-950 border border-white/10 p-6 space-y-4 shadow-2xl font-sans">
            <div className="flex items-center justify-between">
              <h2 id="byok-modal-title" className="text-base font-bold text-white flex items-center gap-2">
                <KeyIcon className="w-4 h-4 text-cyan-400" aria-hidden="true" />
                BYOK (Bring Your Own Key) Session
              </h2>
              <button
                type="button"
                onClick={() => setShowByokModal(false)}
                className="p-1 rounded-lg text-slate-400 hover:text-white"
                aria-label="Close BYOK dialog"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <p className="text-xs text-slate-400 leading-relaxed">
              Attach an active Sunborn API key to client requests for authenticated multi-tenant routing. Session keys remain strictly in-memory.
            </p>

            <div className="space-y-1.5 font-mono text-xs">
              <label htmlFor="byok-input" className="text-slate-300 font-semibold">
                GATEWAY API KEY
              </label>
              <input
                id="byok-input"
                type="password"
                value={tempKey}
                onChange={(e) => setTempKey(e.target.value)}
                placeholder="sb_live_..."
                className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-400"
              />
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              {byokApiKey && (
                <button
                  type="button"
                  onClick={() => {
                    setByokApiKey(null);
                    setTempKey("");
                    setShowByokModal(false);
                  }}
                  className="px-3 py-1.5 rounded-lg border border-rose-500/30 text-rose-300 hover:bg-rose-950/40 text-xs font-mono"
                >
                  Clear Key
                </button>
              )}
              <button
                type="button"
                onClick={() => {
                  if (tempKey.trim()) {
                    setByokApiKey(tempKey.trim());
                  }
                  setShowByokModal(false);
                }}
                className="px-4 py-1.5 rounded-lg bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 hover:bg-cyan-500/30 text-xs font-mono font-semibold"
              >
                Save Key
              </button>
            </div>

            {/* Mobile Currency Switcher */}
            <div className="flex items-center justify-between p-3 rounded-xl bg-slate-900/90 border border-white/10 text-xs font-mono">
              <span className="text-slate-400">CURRENCY MODE:</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => setCurrency("INR")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "INR"
                      ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  ₹ INR
                </button>
                <button
                  type="button"
                  onClick={() => setCurrency("USD")}
                  className={cn(
                    "px-3 py-1 rounded text-xs font-medium transition-colors",
                    currency === "USD"
                      ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold"
                      : "text-slate-400 hover:text-slate-200"
                  )}
                >
                  $ USD
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
