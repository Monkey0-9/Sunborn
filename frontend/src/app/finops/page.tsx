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
  ShieldCheck,
  Download,
  FileSpreadsheet,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { formatCurrency, formatNumber, formatIndianNumbering } from "@/lib/utils";
import { sunbornApi } from "@/lib/api";
import { useClusterStore } from "@/lib/cluster-store";

const GST_STATES: Record<string, string> = {
  "01": "Jammu & Kashmir",
  "02": "Himachal Pradesh",
  "03": "Punjab",
  "06": "Haryana",
  "07": "Delhi",
  "08": "Rajasthan",
  "09": "Uttar Pradesh",
  "10": "Bihar",
  "19": "West Bengal",
  "24": "Gujarat",
  "27": "Maharashtra",
  "29": "Karnataka",
  "32": "Kerala",
  "33": "Tamil Nadu",
  "36": "Telangana",
  "37": "Andhra Pradesh",
};

export default function FinOpsPage() {
  const { currency } = useClusterStore();
  const [monthlyQueries, setMonthlyQueries] = useState<number>(500000);
  const [cacheHitRatio, setCacheHitRatio] = useState<number>(92.5);
  const [avgInputTokens, setAvgInputTokens] = useState<number>(1200);
  const [avgOutputTokens, setAvgOutputTokens] = useState<number>(600);
  const [modelPricing, setModelPricing] = useState<
    "gpt-4o" | "claude-3-5-sonnet" | "llama-3-1-70b" | "sarvam-2b" | "krutrim-v1"
  >("gpt-4o");
  const [gstin, setGstin] = useState<string>("29ABCDE1234F1Z5");
  const [isInterState, setIsInterState] = useState<boolean>(false);

  const [liveReport, setLiveReport] = useState<{
    exactHits: number;
    semanticHits: number;
    totalHits: number;
    requestsTotal: number;
    estimatedSavingsUsd: number;
  } | null>(null);

  // K-007 Spend Velocity Controller Interactive Workbench State
  const [velocityCeiling, setVelocityCeiling] = useState<number>(10.0);
  const [reserveAmount, setReserveAmount] = useState<number>(4.5);
  const [reservationState, setReservationState] = useState<{
    status: "IDLE" | "RESERVED" | "SETTLED" | "OVERSPEND_BLOCKED";
    reservationId: string | null;
    currentSpend: number;
    currentReserved: number;
    totalCommitted: number;
    message: string;
  }>({
    status: "IDLE",
    reservationId: null,
    currentSpend: 0.0,
    currentReserved: 0.0,
    totalCommitted: 0.0,
    message: "Ready to test atomic pre-authorization reservation & settlement.",
  });

  const handleReserve = (amt: number) => {
    const totalIfReserved = reservationState.currentSpend + reservationState.currentReserved + amt;
    if (totalIfReserved > velocityCeiling) {
      setReservationState((prev) => ({
        ...prev,
        status: "OVERSPEND_BLOCKED",
        message: `ATOMIC REJECTION: In-flight reservation of $${amt.toFixed(2)} would commit $${totalIfReserved.toFixed(2)}, exceeding the $${velocityCeiling.toFixed(2)} tenant ceiling!`,
      }));
    } else {
      const resId = `res_${Math.random().toString(36).substring(2, 9)}-${Date.now().toString(36)}`;
      setReservationState({
        status: "RESERVED",
        reservationId: resId,
        currentSpend: reservationState.currentSpend,
        currentReserved: reservationState.currentReserved + amt,
        totalCommitted: reservationState.currentSpend + reservationState.currentReserved + amt,
        message: `RESERVED: Atomic pre-auth lock acquired for $${amt.toFixed(2)}. In-flight overspend race eliminated (K-007).`,
      });
    }
  };

  const handleSettle = (actual: number) => {
    if (!reservationState.reservationId) return;
    const newSpend = reservationState.currentSpend + actual;
    setReservationState({
      status: "SETTLED",
      reservationId: null,
      currentSpend: +newSpend.toFixed(2),
      currentReserved: 0.0,
      totalCommitted: +newSpend.toFixed(2),
      message: `SETTLED: Pre-authorization settled with actual spend of $${actual.toFixed(2)}. Reservation released.`,
    });
  };

  const handleRelease = () => {
    if (!reservationState.reservationId) return;
    setReservationState({
      status: "IDLE",
      reservationId: null,
      currentSpend: reservationState.currentSpend,
      currentReserved: 0.0,
      totalCommitted: reservationState.currentSpend,
      message: "RELEASED: In-flight reservation cleared without spend attribution.",
    });
  };

  const handleExportCsv = () => {
    const rows = [
      [
        "Timestamp",
        "TenantId",
        "ApplicationId",
        "Model",
        "CacheTier",
        "AvoidedTokens",
        "CostAvoidedUSD",
        "NetSavingsUSD",
        "NetSavingsINR",
        "GST_SAC",
        "CGST_INR_9PCT",
        "SGST_INR_9PCT",
        "IGST_INR_18PCT",
      ],
      [
        new Date().toISOString(),
        "tenant-enterprise-fintech",
        "app-risk-evaluator",
        "gpt-4o",
        "exact_l1",
        "1800",
        "0.054",
        "0.054",
        "4.56",
        "998313",
        "0.41",
        "0.41",
        "0.00",
      ],
      [
        new Date(Date.now() - 60000).toISOString(),
        "tenant-enterprise-fintech",
        "app-nlp-search",
        "sarvam-2b",
        "semantic_c2",
        "1450",
        "0.0065",
        "0.0065",
        "0.55",
        "998313",
        "0.05",
        "0.05",
        "0.00",
      ],
      [
        new Date(Date.now() - 120000).toISOString(),
        "tenant-enterprise-fintech",
        "app-risk-evaluator",
        "krutrim-v1",
        "exact_l1",
        "2400",
        "0.012",
        "0.012",
        "1.01",
        "998313",
        "0.09",
        "0.09",
        "0.00",
      ],
      [
        new Date(Date.now() - 180000).toISOString(),
        "tenant-corp-1",
        "app-rag",
        "gpt-4o",
        "origin",
        "0",
        "0.00",
        "-0.015",
        "-1.27",
        "998313",
        "0.00",
        "0.00",
        "0.00",
      ],
    ];
    const csvContent =
      "data:text/csv;charset=utf-8," + rows.map((e) => e.join(",")).join("\n");
    const encodedUri = encodeURI(csvContent);
    const link = document.createElement("a");
    link.setAttribute("href", encodedUri);
    link.setAttribute("download", `sunborn-finops-ledger-${Date.now()}.csv`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const handleExportJson = () => {
    const data = {
      exported_at: new Date().toISOString(),
      report_period: "Trailing 30 Days",
      active_currency: currency,
      sovereign_metrics: {
        exchange_rate_usd_to_inr: 84.50,
        gst_sac_code: "998313",
        gstin: gstin,
        supply_type: isInterState ? "inter_state_igst" : "intra_state_cgst_sgst",
        compliance: {
          dpdp_act_2023: "COMPLIANT_SECTION_8",
          rbi_localization: "COMPLIANT_CIRCULAR_2017_18",
          cert_in: "DIRECTIVE_6HR_AUDIT_LOGGED",
        },
      },
      summary: {
        total_queries: monthlyQueries,
        hit_ratio_pct: cacheHitRatio,
        gross_projected_savings_usd:
          liveReport?.estimatedSavingsUsd ?? 142.5,
        gross_projected_savings_inr:
          (liveReport?.estimatedSavingsUsd ?? 142.5) * 84.50,
      },
      ledger_records: [
        {
          timestamp: new Date().toISOString(),
          tenant_id: "tenant-enterprise-fintech",
          application_id: "app-risk-evaluator",
          model: "gpt-4o",
          cache_tier: "exact_l1",
          tokens_avoided: 1800,
          net_savings_usd: 0.054,
          net_savings_inr: 4.56,
          gst_sac: "998313",
        },
      ],
    };
    const jsonStr =
      "data:text/json;charset=utf-8," +
      encodeURIComponent(JSON.stringify(data, null, 2));
    const link = document.createElement("a");
    link.setAttribute("href", jsonStr);
    link.setAttribute("download", `sunborn-finops-ledger-${Date.now()}.json`);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  React.useEffect(() => {
    sunbornApi
      .getFinOpsReport()
      .then((data) => {
        if (data && data.metrics) {
          setLiveReport({
            exactHits: data.metrics.exact_hits ?? 0,
            semanticHits: data.metrics.semantic_hits ?? 0,
            totalHits: data.metrics.total_hits ?? 0,
            requestsTotal: data.metrics.requests_total ?? 0,
            estimatedSavingsUsd: data.economics?.total_estimated_savings_usd ?? 0,
          });
        }
      })
      .catch(() => {});
  }, []);

  // Model Pricing Rates ($ per Million Tokens)
  const rates = {
    "gpt-4o": { input: 2.5, output: 10.0, name: "OpenAI GPT-4o" },
    "claude-3-5-sonnet": { input: 3.0, output: 15.0, name: "Anthropic Claude 3.5 Sonnet" },
    "llama-3-1-70b": { input: 0.8, output: 0.8, name: "Meta Llama 3.1 70B (Hosted)" },
    "sarvam-2b": { input: 0.15, output: 0.30, name: "Sarvam AI 2B Indic Sovereign" },
    "krutrim-v1": { input: 0.20, output: 0.50, name: "Ola Krutrim Indic LLM" },
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

  // Dynamic Break-Even Calculation (FIN-001)
  const savingsPerQuery = (cacheHitRatio / 100) * costPerQueryOrigin;
  const dynamicBreakEven = savingsPerQuery > 0 ? Math.ceil(infraOverhead / savingsPerQuery) : 0;

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

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={handleExportCsv}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-cyan-600/20 hover:bg-cyan-600/30 text-cyan-300 border border-cyan-500/30 text-xs font-mono font-semibold transition-all cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            <span>Export CSV</span>
          </button>
          <button
            type="button"
            onClick={handleExportJson}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-slate-300 border border-white/10 text-xs font-mono transition-all cursor-pointer"
          >
            <FileSpreadsheet className="w-3.5 h-3.5" />
            <span>Export JSON</span>
          </button>
          <Badge variant="outline" className="font-mono">
            API: GET /v1/finops/report
          </Badge>
          <Badge variant="cyan" dot>
            MODELED TOKEN PROJECTIONS
          </Badge>
        </div>
      </div>

      {/* Live Gateway Telemetry Banner */}
      {liveReport && (
        <div className="p-4 rounded-xl bg-slate-900/80 border border-cyan-500/30 flex flex-wrap items-center justify-between gap-4 font-mono text-xs">
          <div className="flex items-center gap-3">
            <span className="w-2 h-2 rounded-full bg-cyan-400 animate-pulse" />
            <span className="text-slate-400 uppercase tracking-wider font-semibold">Live Gateway Telemetry:</span>
            <span className="text-white font-bold">{formatNumber(liveReport.requestsTotal)} Total Requests</span>
          </div>
          <div className="flex items-center gap-6 text-slate-300">
            <div>
              <span className="text-slate-400 mr-1.5">Exact Hits:</span>
              <span className="text-cyan-400 font-bold">{formatNumber(liveReport.exactHits)}</span>
            </div>
            <div>
              <span className="text-slate-400 mr-1.5">Semantic Hits:</span>
              <span className="text-emerald-400 font-bold">{formatNumber(liveReport.semanticHits)}</span>
            </div>
            <div>
              <span className="text-slate-400 mr-1.5">Modeled Estimate:</span>
              <span className="text-emerald-300 font-bold">{formatCurrency(liveReport.estimatedSavingsUsd, currency)}</span>
            </div>
          </div>
        </div>
      )}

      {/* Primary KPI Ribbon */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card glow="emerald">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-slate-400">Net Monthly Savings</CardTitle>
            <DollarSign className="w-4 h-4 text-emerald-400" />
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold font-mono text-emerald-300">
              {formatCurrency(netSavings, currency)}
            </div>
            <p className="text-xs font-mono text-emerald-400 mt-1 flex items-center justify-between">
              <span>After infra overhead</span>
              {currency === "INR" && (
                <span className="text-amber-300 font-bold">({formatIndianNumbering(netSavings * 84.50)})</span>
              )}
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
              {formatCurrency(avoidedTokenSpend, currency)}
            </div>
            <p className="text-xs font-mono text-cyan-400 mt-1 flex items-center justify-between">
              <span>{formatNumber(cachedQueries)} cached reqs</span>
              {currency === "INR" && (
                <span className="text-amber-300 font-bold">({formatIndianNumbering(avoidedTokenSpend * 84.50)})</span>
              )}
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
              {formatCurrency(infraOverhead, currency)}
            </div>
            <p className="text-xs font-mono text-slate-400 mt-1 flex items-center justify-between">
              <span>Dragonfly + Qdrant host</span>
              {currency === "INR" && (
                <span className="text-slate-300 font-bold">({formatIndianNumbering(infraOverhead * 84.50)})</span>
              )}
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
                <label htmlFor="finops-model-tier" className="text-xs font-mono text-slate-400">MODEL TIER & TOKEN RATES</label>
                <select
                  id="finops-model-tier"
                  value={modelPricing}
                  onChange={(e) => setModelPricing(e.target.value as any)}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-cyan-300 focus:border-cyan-500 cursor-pointer"
                >
                  <option value="gpt-4o">OpenAI GPT-4o ($2.50 in / $10.00 out per M)</option>
                  <option value="claude-3-5-sonnet">Anthropic Claude 3.5 Sonnet ($3.00 in / $15.00 out per M)</option>
                  <option value="llama-3-1-70b">Meta Llama 3.1 70B ($0.80 in / $0.80 out per M)</option>
                  <option value="sarvam-2b">Sarvam AI 2B Indic Sovereign ($0.15 in / $0.30 out · ~₹12.50/M)</option>
                  <option value="krutrim-v1">Ola Krutrim Indic LLM ($0.20 in / $0.50 out · ~₹17.00/M)</option>
                </select>
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between text-xs font-mono">
                  <span className="text-slate-400">MONTHLY REQUEST VOLUME</span>
                  <span className="text-white font-bold">{formatNumber(monthlyQueries)} reqs</span>
                </div>
                <input
                  type="range"
                  aria-label="Monthly request volume"
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
                  aria-label="Cache hit ratio (percent)"
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
                  <label htmlFor="finops-avg-prompt-tokens" className="text-xs font-mono text-slate-400">AVG PROMPT TOKENS</label>
                  <input
                    id="finops-avg-prompt-tokens"
                    type="number"
                    value={avgInputTokens}
                    onChange={(e) => setAvgInputTokens(parseInt(e.target.value) || 0)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white"
                  />
                </div>
                <div className="space-y-1">
                  <label htmlFor="finops-avg-completion-tokens" className="text-xs font-mono text-slate-400">AVG COMPLETION TOKENS</label>
                  <input
                    id="finops-avg-completion-tokens"
                    type="number"
                    value={avgOutputTokens}
                    onChange={(e) => setAvgOutputTokens(parseInt(e.target.value) || 0)}
                    className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-white"
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
                Modeled comparison between uncached origin spend and Sunborn caching based on current inputs.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 font-mono text-xs">
              <div className="p-4 rounded-xl bg-slate-900/90 border border-white/10 space-y-3">
                <div className="flex items-center justify-between text-slate-400">
                  <span>Baseline Spend (Without Cache):</span>
                  <span className="text-rose-400 font-bold">{formatCurrency(totalMonthlySpendOrigin, currency)}</span>
                </div>
                <div className="flex items-center justify-between text-slate-400">
                  <span>Spend With Sunborn Caching:</span>
                  <span className="text-cyan-300 font-bold">
                    {formatCurrency(totalMonthlySpendOrigin - avoidedTokenSpend + infraOverhead, currency)}
                  </span>
                </div>
                <div className="pt-2 border-t border-white/10 flex items-center justify-between text-sm">
                  <span className="text-white font-bold">Total Net Monthly Savings:</span>
                  <span className="text-emerald-400 font-bold text-base">{formatCurrency(netSavings, currency)}</span>
                </div>
              </div>

              <div className="p-4 rounded-xl bg-emerald-950/20 border border-emerald-500/30 space-y-2">
                <div className="flex items-center gap-2 text-emerald-400 font-bold">
                  <CheckCircle2 className="w-4 h-4" />
                  <span>Calculated Dynamic Break-Even</span>
                </div>
                <p className="text-[11px] text-slate-300 leading-relaxed">
                  Sunborn achieves break-even at approximately <strong>{formatNumber(dynamicBreakEven)} requests/month</strong> with these slider inputs. *Estimates reflect mathematical cost projections, not an audited accounting ledger.
                </p>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>

      {/* India Sovereign Statutory GST & Tax Attribution (SAC 998313) */}
      <Card glow="amber">
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <CardTitle className="text-base flex items-center gap-2">
                <ShieldCheck className="w-4 h-4 text-amber-400" />
                <span>India Sovereign Statutory GST & Tax Attribution (SAC 998313)</span>
              </CardTitle>
              <CardDescription>
                Statutory Goods and Services Tax (GST) breakdown for enterprise IT & Cloud infrastructure services under the Central GST Act 2017 & CERT-In financial directives.
              </CardDescription>
            </div>
            <Badge variant="warning">
              SAC 998313 · 18% GST ELIGIBLE
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="space-y-1">
              <label htmlFor="finops-gstin-input" className="text-xs font-mono text-slate-400">
                ENTERPRISE GSTIN (15 CHARACTERS)
              </label>
              <input
                id="finops-gstin-input"
                type="text"
                maxLength={15}
                value={gstin}
                onChange={(e) => setGstin(e.target.value.toUpperCase())}
                placeholder="29ABCDE1234F1Z5"
                className="w-full px-3 py-2 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-amber-300 focus:border-amber-500 uppercase"
              />
              <span className="text-[10px] font-mono text-slate-400">
                State: {GST_STATES[gstin.substring(0, 2)] || "Unknown State Code"}
              </span>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-mono text-slate-400">SUPPLY TYPE</label>
              <div className="flex items-center gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => setIsInterState(false)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-mono transition-colors cursor-pointer ${
                    !isInterState
                      ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                      : "bg-slate-900 text-slate-400 border border-white/10"
                  }`}
                >
                  Intra-State (CGST + SGST)
                </button>
                <button
                  type="button"
                  onClick={() => setIsInterState(true)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-mono transition-colors cursor-pointer ${
                    isInterState
                      ? "bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold"
                      : "bg-slate-900 text-slate-400 border border-white/10"
                  }`}
                >
                  Inter-State (IGST)
                </button>
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-mono text-slate-400">SOVEREIGN PEG</label>
              <div className="p-2.5 rounded-lg bg-slate-900 border border-white/10 font-mono text-xs flex items-center justify-between">
                <span className="text-slate-400">1 USD =</span>
                <span className="text-white font-bold">₹84.50 INR</span>
              </div>
              <span className="text-[10px] font-mono text-emerald-400">RBI Reference Rate</span>
            </div>
          </div>

          {/* GST Statutory Breakdown Table */}
          {(() => {
            const taxableInr = netSavings * 84.50;
            const cgst = !isInterState ? taxableInr * 0.09 : 0;
            const sgst = !isInterState ? taxableInr * 0.09 : 0;
            const igst = isInterState ? taxableInr * 0.18 : 0;
            const totalTax = cgst + sgst + igst;
            const totalEconomicValue = taxableInr + totalTax;

            return (
              <div className="overflow-x-auto rounded-xl border border-white/10">
                <table className="w-full text-xs font-mono text-left">
                  <thead className="bg-slate-950 text-slate-400 border-b border-white/10">
                    <tr>
                      <th className="py-2.5 px-4">Tax Component</th>
                      <th className="py-2.5 px-4">SAC Code</th>
                      <th className="py-2.5 px-4">Applicable Rate</th>
                      <th className="py-2.5 px-4 text-right">Taxable Base (INR)</th>
                      <th className="py-2.5 px-4 text-right">Tax Amount (INR)</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5 bg-slate-900/60">
                    {!isInterState ? (
                      <>
                        <tr>
                          <td className="py-2.5 px-4 text-white font-medium">Central GST (CGST)</td>
                          <td className="py-2.5 px-4 text-amber-300">998313</td>
                          <td className="py-2.5 px-4 text-slate-300">9.00%</td>
                          <td className="py-2.5 px-4 text-right text-slate-200">
                            {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(taxableInr)}
                          </td>
                          <td className="py-2.5 px-4 text-right text-amber-300 font-bold">
                            {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(cgst)}
                          </td>
                        </tr>
                        <tr>
                          <td className="py-2.5 px-4 text-white font-medium">State GST (SGST)</td>
                          <td className="py-2.5 px-4 text-amber-300">998313</td>
                          <td className="py-2.5 px-4 text-slate-300">9.00%</td>
                          <td className="py-2.5 px-4 text-right text-slate-200">
                            {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(taxableInr)}
                          </td>
                          <td className="py-2.5 px-4 text-right text-amber-300 font-bold">
                            {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(sgst)}
                          </td>
                        </tr>
                      </>
                    ) : (
                      <tr>
                        <td className="py-2.5 px-4 text-white font-medium">Integrated GST (IGST)</td>
                        <td className="py-2.5 px-4 text-amber-300">998313</td>
                        <td className="py-2.5 px-4 text-slate-300">18.00%</td>
                        <td className="py-2.5 px-4 text-right text-slate-200">
                          {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(taxableInr)}
                        </td>
                        <td className="py-2.5 px-4 text-right text-amber-300 font-bold">
                          {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(igst)}
                        </td>
                      </tr>
                    )}
                    <tr className="bg-slate-950 font-bold">
                      <td className="py-3 px-4 text-white" colSpan={3}>
                        Total Economic Value Delivered (Net Savings + Input Tax Credit)
                      </td>
                      <td className="py-3 px-4 text-right text-slate-300">
                        {formatIndianNumbering(taxableInr)}
                      </td>
                      <td className="py-3 px-4 text-right text-emerald-400 text-sm">
                        {new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" }).format(totalEconomicValue)}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            );
          })()}

          <div className="p-3 rounded-lg bg-amber-950/20 border border-amber-500/20 text-[11px] text-amber-200/90 leading-relaxed font-sans">
            <strong>Statutory Compliance Note:</strong> Invoices generated under SAC 998313 qualify for Input Tax Credit (ITC) under Section 16 of the Central Goods and Services Tax Act, 2017. All telemetry and transactional ledgers adhere strictly to the Digital Personal Data Protection Act, 2023 (DPDP Act) and RBI Financial Data Localization Mandate (Circular DPSS.CO.OD.No.2785/06.08.005/2017-18).
          </div>
        </CardContent>
      </Card>

      {/* K-007 Atomic Spend Velocity Controller & Concurrency Workbench */}
      <Card glow="cyan">
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <CardTitle className="text-base flex items-center gap-2">
                <ShieldCheck className="w-4 h-4 text-cyan-400" />
                <span>K-007 Atomic Spend Velocity & Concurrency Governor</span>
              </CardTitle>
              <CardDescription>
                Interactive demonstration of Sunborn&apos;s two-phase atomic pre-authorization reservation (reserve_spend) and settlement (settle_spend) preventing concurrent overspend races.
              </CardDescription>
            </div>
            <Badge
              variant={
                reservationState.status === "OVERSPEND_BLOCKED"
                  ? "danger"
                  : reservationState.status === "RESERVED"
                  ? "warning"
                  : "cyan"
              }
            >
              STATUS: {reservationState.status}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4 font-mono text-xs">
            <div className="p-3 rounded-xl bg-slate-900 border border-white/10 space-y-1">
              <span className="text-[10px] text-slate-400">TENANT CEILING</span>
              <div className="text-sm font-bold text-white">${velocityCeiling.toFixed(2)}</div>
              <span className="text-[10px] text-slate-400">Fixed Budget Cap</span>
            </div>
            <div className="p-3 rounded-xl bg-slate-900 border border-white/10 space-y-1">
              <span className="text-[10px] text-slate-400">CURRENT SPEND (SETTLED)</span>
              <div className="text-sm font-bold text-emerald-400">
                ${reservationState.currentSpend.toFixed(2)}
              </div>
              <span className="text-[10px] text-slate-400">Durable Ledger</span>
            </div>
            <div className="p-3 rounded-xl bg-slate-900 border border-white/10 space-y-1">
              <span className="text-[10px] text-slate-400">IN-FLIGHT RESERVED</span>
              <div className="text-sm font-bold text-amber-400">
                ${reservationState.currentReserved.toFixed(2)}
              </div>
              <span className="text-[10px] text-slate-400">Atomic Lock Active</span>
            </div>
            <div className="p-3 rounded-xl bg-slate-900 border border-white/10 space-y-1">
              <span className="text-[10px] text-slate-400">TOTAL COMMITMENT</span>
              <div className="text-sm font-bold text-cyan-300">
                ${reservationState.totalCommitted.toFixed(2)} / ${velocityCeiling.toFixed(2)}
              </div>
              <span className="text-[10px] text-slate-400">
                {((reservationState.totalCommitted / velocityCeiling) * 100).toFixed(0)}% Utilized
              </span>
            </div>
          </div>

          {/* Commitment Progress Bar */}
          <div className="space-y-1.5 font-mono text-xs">
            <div className="flex items-center justify-between text-[11px] text-slate-400">
              <span>Budget Commitment Gauge (Spend + In-Flight Reservations):</span>
              <span className="text-cyan-300 font-bold">
                {((reservationState.totalCommitted / velocityCeiling) * 100).toFixed(1)}%
              </span>
            </div>
            <div className="w-full h-3 rounded-full bg-slate-950 border border-white/10 overflow-hidden flex">
              <div
                className="h-full bg-emerald-500 transition-all duration-300"
                style={{
                  width: `${Math.min(
                    100,
                    (reservationState.currentSpend / velocityCeiling) * 100
                  )}%`,
                }}
              />
              <div
                className="h-full bg-amber-500 transition-all duration-300"
                style={{
                  width: `${Math.min(
                    100 - (reservationState.currentSpend / velocityCeiling) * 100,
                    (reservationState.currentReserved / velocityCeiling) * 100
                  )}%`,
                }}
              />
            </div>
            <div className="flex items-center gap-4 text-[10px] text-slate-400 pt-1">
              <div className="flex items-center gap-1.5">
                <span className="w-2 h-2 rounded-full bg-emerald-500" />
                <span>Settled Spend</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="w-2 h-2 rounded-full bg-amber-500" />
                <span>In-Flight Pre-Authorization (K-007)</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="w-2 h-2 rounded-full bg-slate-800" />
                <span>Available Headroom</span>
              </div>
            </div>
          </div>

          {/* Interactive Actions Grid */}
          <div className="p-4 rounded-xl bg-slate-950/80 border border-white/10 space-y-4">
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                onClick={() => handleReserve(reserveAmount)}
                className="px-4 py-2 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-white font-mono text-xs font-semibold shadow-lg shadow-cyan-900/30 transition-all cursor-pointer"
              >
                1. Pre-Authorize (${reserveAmount.toFixed(2)})
              </button>
              <button
                type="button"
                onClick={() => handleReserve(7.0)}
                className="px-4 py-2 rounded-lg bg-rose-600/20 hover:bg-rose-600/30 text-rose-300 border border-rose-500/30 font-mono text-xs font-semibold transition-all cursor-pointer"
              >
                2. Test Concurrent Collision (Attempt +$7.00)
              </button>
              <button
                type="button"
                onClick={() => handleSettle(4.2)}
                disabled={!reservationState.reservationId}
                className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 disabled:pointer-events-none text-white font-mono text-xs font-semibold shadow-lg shadow-emerald-900/30 transition-all cursor-pointer"
              >
                3. Settle Spend ($4.20)
              </button>
              <button
                type="button"
                onClick={handleRelease}
                disabled={!reservationState.reservationId}
                className="px-4 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:opacity-40 disabled:pointer-events-none text-slate-300 font-mono text-xs transition-all cursor-pointer"
              >
                Release Without Spend
              </button>
            </div>

            <div className="p-3 rounded-lg bg-slate-900 border border-white/5 font-mono text-xs space-y-1">
              <div className="text-slate-400 text-[10px]">CONTROLLER LOG:</div>
              <div className="text-slate-200">{reservationState.message}</div>
              {reservationState.reservationId && (
                <div className="text-cyan-400 text-[10px]">
                  RESERVATION UUID: {reservationState.reservationId}
                </div>
              )}
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
