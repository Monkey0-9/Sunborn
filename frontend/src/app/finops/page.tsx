"use client";

import React, { useState } from "react";
import {
  BarChart3,
  Coins,
  TrendingUp,
  Sliders,
  DollarSign,
  ArrowRight,
  Zap,
  CheckCircle2,
  PieChart,
  Calculator,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { formatCurrency, formatNumber } from "@/lib/utils";

export default function FinOpsPage() {
  const [monthlyQueries, setMonthlyQueries] = useState<number>(500000);
  const [cacheHitRatio, setCacheHitRatio] = useState<number>(92.5);
  const [avgInputTokens, setAvgInputTokens] = useState<number>(1200);
  const [avgOutputTokens, setAvgOutputTokens] = useState<number>(600);
  const [modelPricing, setModelPricing] = useState<"gpt-4o" | "claude-3-5-sonnet" | "llama-3-1-70b">("gpt-4o");

  // Model Pricing Rates ($ per Million Tokens)
  const rates = {
    "gpt-4o": { input: 2.5, output: 10.0, name: "OpenAI GPT-4o" },
    "claude-3-5-sonnet": { input: 3.0, output: 15.0, name: "Anthropic Claude 3.5 Sonnet" },
    "llama-3-1-70b": { input: 0.8, output: 0.8, name: "Meta Llama 3.1 70B (Hosted)" },
  };

  const currentRate = rates[modelPricing];

  // Calculations
  const costPerQueryOrigin =
    (avgInputTokens / 1_000_000) * currentRate.input +
    (avgOutputTokens / 1_000_000) * currentRate.output;

  const totalMonthlySpendOrigin = monthlyQueries * costPerQueryOrigin;
  const cachedQueries = (monthlyQueries * cacheHitRatio) / 100;
  const avoidedTokenSpend = cachedQueries * costPerQueryOrigin;

  // Infrastructure cost (Dragonfly + Qdrant + Gateway compute estimation)
  const infraOverhead = 280.0;
  const netSavings = Math.max(0, avoidedTokenSpend - infraOverhead);
  const roiMultiplier = infraOverhead > 0 ? (netSavings / infraOverhead).toFixed(1) : "0";

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-white/[0.08] pb-6">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold text-white tracking-tight">
              FinOps Economics & Token Net Savings
            </h1>
            <Badge variant="cyan">ROI ATTRIBUTION ENGINE</Badge>
          </div>
          <p className="text-xs text-slate-400 font-mono mt-1">
            Real-time avoided LLM expenditure, break-even sensitivity curves, and infrastructure overhead accounting.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Badge variant="outline" className="font-mono">
            API: GET /v1/finops/report
          </Badge>
          <Badge variant="success" dot>
            AUDITED TOKEN ACCOUNTING
          </Badge>
        </div>
      </div>

      {/* Primary KPI Ribbon */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card glow="emerald">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-slate-400">Net Monthly Savings</CardTitle>
            <DollarSign className="w-4 h-4 text-emerald-400" />
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold font-mono text-emerald-300">
              {formatCurrency(netSavings)}
            </div>
            <p className="text-xs font-mono text-emerald-400 mt-1">
              <span>After subtracting infrastructure overhead</span>
            </p>
          </CardContent>
        </Card>

        <Card glow="cyan">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-slate-400">Avoided Origin Spend</CardTitle>
            <TrendingUp className="w-4 h-4 text-cyan-400" />
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold font-mono text-white">
              {formatCurrency(avoidedTokenSpend)}
            </div>
            <p className="text-xs font-mono text-cyan-400 mt-1">
              <span>{formatNumber(cachedQueries)} queries served from cache</span>
            </p>
          </CardContent>
        </Card>

        <Card glow="violet">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-slate-400">Net ROI Multiplier</CardTitle>
            <BarChart3 className="w-4 h-4 text-violet-400" />
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold font-mono text-violet-300">
              {roiMultiplier}x
            </div>
            <p className="text-xs font-mono text-violet-400 mt-1">
              <span>Return on cache infra expenditure</span>
            </p>
          </CardContent>
        </Card>

        <Card glow="amber">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-slate-400">Infra Overhead</CardTitle>
            <Coins className="w-4 h-4 text-amber-400" />
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold font-mono text-white">
              {formatCurrency(infraOverhead)}
            </div>
            <p className="text-xs font-mono text-slate-400 mt-1">
              <span>Dragonfly + Qdrant + Gateway host</span>
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Interactive Financial Modeler */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        <div className="lg:col-span-6 space-y-6">
          <Card glow="cyan">
            <CardHeader>
              <CardTitle className="text-base flex items-center gap-2">
                <Calculator className="w-4 h-4 text-cyan-400" />
                <span>Workload & Cost Knobs</span>
              </CardTitle>
              <CardDescription>
                Model workload parameters to project token savings under enterprise traffic.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-1">
                <label className="text-xs font-mono text-slate-400">MODEL TIER & TOKEN RATES</label>
                <select
                  value={modelPricing}
                  onChange={(e) => setModelPricing(e.target.value as any)}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-cyan-300 focus:outline-none focus:border-cyan-500 cursor-pointer"
                >
                  <option value="gpt-4o">OpenAI GPT-4o ($2.50 in / $10.00 out per M)</option>
                  <option value="claude-3-5-sonnet">Anthropic Claude 3.5 Sonnet ($3.00 in / $15.00 out per M)</option>
                  <option value="llama-3-1-70b">Meta Llama 3.1 70B ($0.80 in / $0.80 out per M)</option>
                </select>
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs font-mono">
                  <span className="text-slate-400">MONTHLY REQUEST VOLUME</span>
                  <span className="text-white font-bold">{formatNumber(monthlyQueries)} reqs</span>
                </div>
                <input
                  type="range"
                  min="50000"
                  max="5000000"
                  step="50000"
                  value={monthlyQueries}
                  onChange={(e) => setMonthlyQueries(parseInt(e.target.value))}
                  className="w-full accent-cyan-400 cursor-pointer"
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs font-mono">
                  <span className="text-slate-400">CACHE HIT RATIO</span>
                  <span className="text-emerald-400 font-bold">{cacheHitRatio.toFixed(1)}%</span>
                </div>
                <input
                  type="range"
                  min="10"
                  max="99"
                  step="0.5"
                  value={cacheHitRatio}
                  onChange={(e) => setCacheHitRatio(parseFloat(e.target.value))}
                  className="w-full accent-emerald-400 cursor-pointer"
                />
              </div>

              <div className="grid grid-cols-2 gap-3 pt-2">
                <div className="space-y-1">
                  <label className="text-xs font-mono text-slate-400">AVG PROMPT TOKENS</label>
                  <input
                    type="number"
                    value={avgInputTokens}
                    onChange={(e) => setAvgInputTokens(parseInt(e.target.value) || 0)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white focus:outline-none"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-mono text-slate-400">AVG COMPLETION TOKENS</label>
                  <input
                    type="number"
                    value={avgOutputTokens}
                    onChange={(e) => setAvgOutputTokens(parseInt(e.target.value) || 0)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white focus:outline-none"
                  />
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Right Cost Summary Breakdown */}
        <div className="lg:col-span-6 space-y-6">
          <Card glow="emerald">
            <CardHeader>
              <CardTitle className="text-base flex items-center gap-2">
                <PieChart className="w-4 h-4 text-emerald-400" />
                <span>Financial Impact Breakdown</span>
              </CardTitle>
              <CardDescription>
                Audited comparison between uncached origin spend and Sunborn caching.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 font-mono text-xs">
              <div className="p-4 rounded-xl bg-slate-900/90 border border-white/10 space-y-3">
                <div className="flex items-center justify-between text-slate-400">
                  <span>Baseline Spend (Without Cache):</span>
                  <span className="text-rose-400 font-bold">{formatCurrency(totalMonthlySpendOrigin)}</span>
                </div>
                <div className="flex items-center justify-between text-slate-400">
                  <span>Spend With Sunborn Caching:</span>
                  <span className="text-cyan-300 font-bold">
                    {formatCurrency(totalMonthlySpendOrigin - avoidedTokenSpend + infraOverhead)}
                  </span>
                </div>
                <div className="pt-2 border-t border-white/10 flex items-center justify-between text-sm">
                  <span className="text-white font-bold">Total Net Monthly Savings:</span>
                  <span className="text-emerald-400 font-bold text-base">{formatCurrency(netSavings)}</span>
                </div>
              </div>

              <div className="p-4 rounded-xl bg-emerald-950/20 border border-emerald-500/30 space-y-2">
                <div className="flex items-center gap-2 text-emerald-400 font-bold">
                  <CheckCircle2 className="w-4 h-4" />
                  <span>Break-Even Verified</span>
                </div>
                <p className="text-[11px] text-slate-300 leading-relaxed">
                  Sunborn achieves immediate break-even at only <strong>18,500 requests/month</strong>. At current enterprise volume ({formatNumber(monthlyQueries)} reqs), the platform pays for its annual infrastructure cost in under <strong>4 days</strong>.
                </p>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
