"use client";

import React, { useState, useEffect, useCallback } from "react";
import { useRouter } from "next/navigation";
import {
  Search,
  Activity,
  Cpu,
  BarChart3,
  Layers,
  Flame,
  Lock,
  GitBranch,
  Globe,
  KeyRound,
  Zap,
  ArrowRight,
  ShieldCheck,
  RefreshCw,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";

interface CommandItem {
  id: string;
  title: string;
  subtitle: string;
  icon: React.ComponentType<{ className?: string }>;
  badge?: string;
  action: () => void;
  category: "Navigation" | "Quick Action" | "Tenant Profile";
}

export function CommandMenu() {
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState("");
  const router = useRouter();

  // Listen for Cmd+K / Ctrl+K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      } else if (e.key === "Escape") {
        setIsOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const navigateTo = useCallback(
    (href: string) => {
      setIsOpen(false);
      router.push(href);
    },
    [router]
  );

  const items: CommandItem[] = [
    // Navigation
    {
      id: "nav-overview",
      title: "Platform Overview & Telemetry",
      subtitle: "Live cluster metrics, cache hit rates & request stream",
      icon: Activity,
      category: "Navigation",
      action: () => navigateTo("/"),
    },
    {
      id: "nav-explorer",
      title: "Cache Explorer & Prompt Playground",
      subtitle: "Execute live inference with multi-tier latency trace",
      icon: Search,
      badge: "Interactive",
      category: "Navigation",
      action: () => navigateTo("/explorer"),
    },
    {
      id: "nav-finops",
      title: "FinOps ROI & Spend Velocity Controller",
      subtitle: "Token economics, spend ceilings & atomic reservation",
      icon: BarChart3,
      badge: "K-007 Verified",
      category: "Navigation",
      action: () => navigateTo("/finops"),
    },
    {
      id: "nav-execution",
      title: "Execution DAG & Merkle Verifier",
      subtitle: "Agent workflow subgraphs & tamper-evident inclusion proofs",
      icon: Cpu,
      category: "Navigation",
      action: () => navigateTo("/execution"),
    },
    {
      id: "nav-governance",
      title: "Governance & GDPR Cryptographic Shredding",
      subtitle: "TDEK vault management & O(1) multi-store erasure",
      icon: Flame,
      badge: "GDPR Art. 17",
      category: "Navigation",
      action: () => navigateTo("/governance"),
    },
    {
      id: "nav-security",
      title: "Security Fortress & Zero-Trust Boundary",
      subtitle: "Adversarial prompt injection firewall & audit logs",
      icon: Lock,
      category: "Navigation",
      action: () => navigateTo("/security"),
    },
    {
      id: "nav-knowledge",
      title: "Semantic Studio & Knowledge Invalidation",
      subtitle: "Conformal risk thresholds & cascade invalidation",
      icon: Layers,
      category: "Navigation",
      action: () => navigateTo("/knowledge"),
    },
    {
      id: "nav-learning",
      title: "Adaptive Learning Controller",
      subtitle: "Canary rollouts & autonomous policy recalibration",
      icon: GitBranch,
      category: "Navigation",
      action: () => navigateTo("/learning"),
    },
    {
      id: "nav-ceremony",
      title: "Cryptographic Ceremony Studio",
      subtitle: "Sovereign proof attestation & simulation verification",
      icon: Globe,
      category: "Navigation",
      action: () => navigateTo("/ceremony"),
    },
    // Quick Actions
    {
      id: "action-rebuild",
      title: "Trigger PostgreSQL Cache Rebuild",
      subtitle: "Reconstruct L1/L2 cache entries and vector indices from cold storage",
      icon: RefreshCw,
      badge: "Durable",
      category: "Quick Action",
      action: () => {
        setIsOpen(false);
        router.push("/governance");
      },
    },
    {
      id: "action-firewall",
      title: "Run Adversarial Security Test",
      subtitle: "Simulate OWASP-LLM-01 prompt extraction attempt",
      icon: ShieldCheck,
      badge: "Defense",
      category: "Quick Action",
      action: () => {
        setIsOpen(false);
        router.push("/security");
      },
    },
  ];

  const filtered = query.trim()
    ? items.filter(
        (i) =>
          i.title.toLowerCase().includes(query.toLowerCase()) ||
          i.subtitle.toLowerCase().includes(query.toLowerCase()) ||
          i.category.toLowerCase().includes(query.toLowerCase())
      )
    : items;

  if (!isOpen) {
    return (
      <button
        onClick={() => setIsOpen(true)}
        className="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-xs font-mono text-slate-400 hover:text-slate-200 transition-all duration-200"
        aria-label="Open command palette"
      >
        <Search className="w-3.5 h-3.5 text-cyan-400" />
        <span>Command Menu...</span>
        <kbd className="px-1.5 py-0.5 rounded bg-white/[0.08] text-[10px] text-slate-400 font-sans border border-white/[0.08]">
          ⌘K
        </kbd>
      </button>
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-slate-950/80 backdrop-blur-md animate-in fade-in duration-200"
        onClick={() => setIsOpen(false)}
      />

      {/* Modal Dialog */}
      <div className="relative w-full max-w-2xl bg-slate-900 border border-white/[0.12] rounded-2xl shadow-2xl overflow-hidden z-10 animate-in zoom-in-95 duration-200">
        {/* Search Input Bar */}
        <div className="flex items-center gap-3 px-4 py-3.5 border-b border-white/[0.08] bg-slate-950/60">
          <Search className="w-5 h-5 text-cyan-400 shrink-0" />
          <input
            autoFocus
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Type a command or search platform systems..."
            className="w-full bg-transparent border-0 text-white placeholder-slate-500 focus:outline-none text-sm font-sans"
          />
          <kbd className="px-2 py-0.5 rounded bg-white/[0.06] text-xs text-slate-400 font-mono border border-white/[0.08]">
            ESC
          </kbd>
        </div>

        {/* Results List */}
        <div className="max-h-[60vh] overflow-y-auto p-2 divide-y divide-white/[0.04]">
          {filtered.length === 0 ? (
            <div className="p-8 text-center text-slate-500 text-sm font-mono">
              No matching platform subsystem or command found.
            </div>
          ) : (
            filtered.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  onClick={item.action}
                  className="w-full flex items-center justify-between gap-4 p-3 rounded-xl hover:bg-white/[0.06] text-left transition-colors group"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-8 h-8 rounded-lg bg-white/[0.04] border border-white/[0.08] flex items-center justify-center shrink-0 group-hover:border-cyan-500/40 group-hover:bg-cyan-500/10 transition-colors">
                      <Icon className="w-4 h-4 text-slate-400 group-hover:text-cyan-400 transition-colors" />
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-white tracking-tight group-hover:text-cyan-300 transition-colors">
                          {item.title}
                        </span>
                        {item.badge && (
                          <Badge variant="cyan" className="text-[10px] py-0 px-1.5">
                            {item.badge}
                          </Badge>
                        )}
                      </div>
                      <p className="text-xs text-slate-400 font-mono truncate">{item.subtitle}</p>
                    </div>
                  </div>
                  <ArrowRight className="w-4 h-4 text-slate-600 group-hover:text-cyan-400 group-hover:translate-x-0.5 transition-all shrink-0" />
                </button>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-2.5 border-t border-white/[0.08] bg-slate-950/80 flex items-center justify-between text-[11px] text-slate-500 font-mono">
          <span>Sunborn FAANG Enterprise Navigation</span>
          <div className="flex items-center gap-3">
            <span>
              <kbd className="px-1.5 py-0.5 rounded bg-white/[0.06] text-slate-400">↑↓</kbd> to
              navigate
            </span>
            <span>
              <kbd className="px-1.5 py-0.5 rounded bg-white/[0.06] text-slate-400">↵</kbd> to
              select
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
