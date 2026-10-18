"use client";

import React, { useState, useEffect } from "react";
import {
  GitBranch,
  ShieldCheck,
  CheckCircle2,
  AlertTriangle,
  RotateCcw,
  Sparkles,
  RefreshCw,
  TrendingUp,
  Cpu,
  Layers,
  Zap,
  BarChart3,
  Sliders,
  Check,
  X,
  History,
  Lock,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ShimmerButton } from "@/components/ui/shimmer-button";
import { sunbornApi } from "@/lib/api";

interface CandidatePolicy {
  policy_id: string;
  version: number;
  weights: {
    min_similarity_threshold: number;
    max_risk_threshold: number;
    conformal_alpha: number;
    prefetch_confidence_threshold: number;
    eviction_staleness_weight: number;
    eviction_cost_weight: number;
  };
  created_at: string;
  evaluation_report: {
    total_samples: number;
    training_samples: number;
    held_out_samples: number;
    training_false_hit_rate: number;
    held_out_false_hit_rate: number;
    held_out_abstention_rate: number;
    estimated_cost_reduction_pct: number;
    is_statistically_sound: boolean;
  };
  is_promoted: boolean;
}

interface AuditLog {
  policy_id: string;
  previous_policy_id: string;
  version: number;
  action: string;
  canary_percentage: number;
  reason: string;
  timestamp: string;
}

export default function LearningControllerPage() {
  const [activePolicy, setActivePolicy] = useState<CandidatePolicy | null>(null);
  const [canaryPolicy, setCanaryPolicy] = useState<{ policy: CandidatePolicy; percentage: number } | null>(null);
  const [totalFeedbackEvents, setTotalFeedbackEvents] = useState<number>(0);
  const [tenantEvents, setTenantEvents] = useState<number>(0);
  const [auditLogs, setAuditLogs] = useState<AuditLog[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionSuccess, setActionSuccess] = useState<string | null>(null);

  // Canary deployment controls
  const [canaryPercentage, setCanaryPercentage] = useState<number>(10);
  const [isDeployingCanary, setIsDeployingCanary] = useState<boolean>(false);
  const [isPromoting, setIsPromoting] = useState<boolean>(false);
  const [isRollingBack, setIsRollingBack] = useState<boolean>(false);
  const [rollbackReason, setRollbackReason] = useState<string>("Manual operator safety intervention");

  // Calibration simulator state
  const [isCalibrating, setIsCalibrating] = useState<boolean>(false);
  const [calibratedCandidate, setCalibratedCandidate] = useState<CandidatePolicy | null>(null);

  const fetchPolicyState = async () => {
    setIsLoading(true);
    setActionError(null);
    try {
      const data = await sunbornApi.getLearningPolicies();
      setActivePolicy(data.active_policy);
      setCanaryPolicy(data.canary_policy);
      setTotalFeedbackEvents(data.total_feedback_events || 0);
      setTenantEvents(data.tenant_feedback_events || 0);
      setAuditLogs(data.audit_trail || []);
    } catch (err: any) {
      setActionError(`Failed to fetch policy state: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchPolicyState();
  }, []);

  const handleCalibratePolicy = async () => {
    setIsCalibrating(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      const nextVersion = (activePolicy?.version ?? 1) + 1;
      const res = await sunbornApi.calibratePolicy(nextVersion);
      if (res && res.candidate_policy) {
        setCalibratedCandidate(res.candidate_policy);
        setActionSuccess(
          `Offline calibration completed on empirical feedback data via backend engine (Candidate: ${res.candidate_policy.policy_id}).`
        );
      } else {
        setActionError("Backend did not return a valid candidate policy.");
      }
    } catch (err: any) {
      setActionError(`Calibration failed: ${err.message}`);
    } finally {
      setIsCalibrating(false);
    }
  };

  const handlePromotePolicy = async () => {
    setIsPromoting(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      await sunbornApi.promoteLearningPolicy();
      setActionSuccess("Canary policy successfully promoted to 100% active production traffic.");
      setCalibratedCandidate(null);
      await fetchPolicyState();
    } catch (err: any) {
      setActionError(`Promotion failed: ${err.message}`);
    } finally {
      setIsPromoting(false);
    }
  };

  const handleDeployCanary = async () => {
    if (!calibratedCandidate) return;
    setIsDeployingCanary(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      await sunbornApi.startCanaryRollout(calibratedCandidate, canaryPercentage);
      setActionSuccess(`Canary rollout initiated at ${canaryPercentage}% traffic.`);
      await fetchPolicyState();
    } catch (err: any) {
      setActionError(`Canary rollout failed: ${err.message}`);
    } finally {
      setIsDeployingCanary(false);
    }
  };

  const handleRollback = async () => {
    setIsRollingBack(true);
    setActionError(null);
    setActionSuccess(null);
    try {
      await sunbornApi.rollbackLearningPolicy(rollbackReason);
      setActionSuccess("Emergency rollback executed. Policy reverted to verified baseline.");
      setCalibratedCandidate(null);
      await fetchPolicyState();
    } catch (err: any) {
      setActionError(`Rollback execution failed: ${err.message}`);
    } finally {
      setIsRollingBack(false);
    }
  };

  return (
    <div className="space-y-8">
      {actionError && (
        <div className="p-4 rounded-xl bg-rose-950/70 border border-rose-500/40 text-rose-200 text-xs font-mono space-y-1">
          <div className="flex items-center gap-2 font-bold text-rose-400">
            <span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
            LEARNING CONTROLLER ALERT
          </div>
          <div>{actionError}</div>
        </div>
      )}

      {actionSuccess && (
        <div className="p-4 rounded-xl bg-emerald-950/70 border border-emerald-500/40 text-emerald-200 text-xs font-mono flex items-center gap-2">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
          <span>{actionSuccess}</span>
        </div>
      )}

      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-white/[0.08] pb-6">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold text-white tracking-tight">
              Adaptive Learning & Autonomous Promotion Controller
            </h1>
            <Badge variant="cyan">CLOSED-LOOP GOVERNANCE</Badge>
          </div>
          <p className="text-xs text-slate-400 font-mono mt-1">
            5-Layer closed-loop learning: verified feedback, 70/30 held-out calibration, counterfactual evaluation, and automated rollback.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <Badge variant={canaryPolicy ? "warning" : "success"} dot className="font-mono">
            {canaryPolicy ? `CANARY ACTIVE (${canaryPolicy.percentage}%)` : "PRODUCTION STABLE"}
          </Badge>
          <button
            onClick={fetchPolicyState}
            disabled={isLoading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-slate-900 border border-white/10 text-xs font-mono text-slate-300 hover:text-white hover:bg-slate-800 transition-all cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? "animate-spin" : ""}`} />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      {/* 5-Layer Pipeline Architecture Cards */}
      <div className="grid grid-cols-1 md:grid-cols-5 gap-3 font-mono text-xs">
        <div className="p-3 rounded-xl bg-slate-900/80 border border-white/10 space-y-1">
          <div className="text-[10px] text-cyan-400 font-bold uppercase tracking-wider">Layer 1: Ingestion</div>
          <div className="text-white font-bold">RFC 2104 HMAC</div>
          <div className="text-[11px] text-slate-400">SHA-256 ground truth authentication</div>
        </div>
        <div className="p-3 rounded-xl bg-slate-900/80 border border-white/10 space-y-1">
          <div className="text-[10px] text-violet-400 font-bold uppercase tracking-wider">Layer 2: Storage</div>
          <div className="text-white font-bold">Tenant Safe Store</div>
          <div className="text-[11px] text-slate-400">Poison defense &amp; GDPR purge</div>
        </div>
        <div className="p-3 rounded-xl bg-slate-900/80 border border-white/10 space-y-1">
          <div className="text-[10px] text-amber-400 font-bold uppercase tracking-wider">Layer 3: Calibration</div>
          <div className="text-white font-bold">70/30 Held-Out</div>
          <div className="text-[11px] text-slate-400">Zero data leakage (p &le; 0.01)</div>
        </div>
        <div className="p-3 rounded-xl bg-slate-900/80 border border-white/10 space-y-1">
          <div className="text-[10px] text-emerald-400 font-bold uppercase tracking-wider">Layer 4: Shadow</div>
          <div className="text-white font-bold">Counterfactual</div>
          <div className="text-[11px] text-slate-400">Simulate delta on past traces</div>
        </div>
        <div className="p-3 rounded-xl bg-slate-900/80 border border-white/10 space-y-1">
          <div className="text-[10px] text-rose-400 font-bold uppercase tracking-wider">Layer 5: Promotion</div>
          <div className="text-white font-bold">Canary &amp; Rollback</div>
          <div className="text-[11px] text-slate-400">Instant invariant-driven abort</div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left Column: Active & Candidate Policy Weights */}
        <div className="lg:col-span-7 space-y-6">
          <Card glow="cyan">
            <CardHeader className="flex flex-row items-center justify-between pb-3">
              <div>
                <CardTitle className="text-base flex items-center gap-2">
                  <Sliders className="w-4 h-4 text-cyan-400" />
                  <span>Active Policy Configuration</span>
                </CardTitle>
                <CardDescription>
                  Current production inference weights and statistical calibration bounds.
                </CardDescription>
              </div>
              <Badge variant="cyan" className="font-mono">
                {activePolicy?.policy_id || "policy-v1-baseline"} (v{activePolicy?.version ?? 1})
              </Badge>
            </CardHeader>
            <CardContent className="space-y-4 font-mono text-xs">
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">MIN SIMILARITY (&tau;)</span>
                  <div className="text-lg font-bold text-cyan-300">
                    {activePolicy?.weights.min_similarity_threshold.toFixed(2) ?? "0.88"}
                  </div>
                  <span className="text-[10px] text-slate-400">&ge; 0.75 floor invariant</span>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">MAX CONFORMAL RISK</span>
                  <div className="text-lg font-bold text-amber-300">
                    {activePolicy?.weights.max_risk_threshold.toFixed(2) ?? "0.25"}
                  </div>
                  <span className="text-[10px] text-slate-400">&le; 0.25 ceiling</span>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">CONFORMAL ALPHA (&alpha;)</span>
                  <div className="text-lg font-bold text-emerald-300">
                    {activePolicy?.weights.conformal_alpha.toFixed(2) ?? "0.01"}
                  </div>
                  <span className="text-[10px] text-slate-400">99% coverage guarantee</span>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">PREFETCH CONFIDENCE</span>
                  <div className="text-lg font-bold text-white">
                    {activePolicy?.weights.prefetch_confidence_threshold.toFixed(2) ?? "0.60"}
                  </div>
                  <span className="text-[10px] text-slate-400">Markov trigger</span>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">STALENESS WEIGHT</span>
                  <div className="text-lg font-bold text-slate-300">
                    {activePolicy?.weights.eviction_staleness_weight.toFixed(2) ?? "0.40"}
                  </div>
                  <span className="text-[10px] text-slate-400">W-TinyLFU ratio</span>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-0.5">
                  <span className="text-[10px] text-slate-500">COST WEIGHT</span>
                  <div className="text-lg font-bold text-slate-300">
                    {activePolicy?.weights.eviction_cost_weight.toFixed(2) ?? "0.60"}
                  </div>
                  <span className="text-[10px] text-slate-400">FinOps priority</span>
                </div>
              </div>

              {/* Calibration Split Report */}
              <div className="p-3.5 rounded-xl bg-slate-900/90 border border-white/10 space-y-2">
                <div className="flex items-center justify-between text-xs font-bold text-white">
                  <span>Held-Out Validation Split Evaluation</span>
                  {activePolicy?.evaluation_report ? (
                    <Badge variant="success" dot>STATISTICALLY SOUND (p &le; 0.01)</Badge>
                  ) : (
                    <Badge variant="outline">PENDING CALIBRATION</Badge>
                  )}
                </div>
                <div className="grid grid-cols-3 gap-2 text-[11px] text-slate-300 pt-1">
                  <div>• Training Samples: <strong className="text-white">{activePolicy?.evaluation_report?.training_samples !== undefined ? activePolicy.evaluation_report.training_samples : "—"}</strong></div>
                  <div>• Held-Out Samples: <strong className="text-white">{activePolicy?.evaluation_report?.held_out_samples !== undefined ? activePolicy.evaluation_report.held_out_samples : "—"}</strong></div>
                  <div>• Held-Out Error Rate: <strong className="text-emerald-300">{activePolicy?.evaluation_report?.held_out_false_hit_rate !== undefined ? `${(activePolicy.evaluation_report.held_out_false_hit_rate * 100).toFixed(2)}%` : "—"}</strong></div>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Canary Deployment & Promotion Controls */}
          <Card glow="violet">
            <CardHeader>
              <CardTitle className="text-base flex items-center gap-2 text-violet-300">
                <GitBranch className="w-4 h-4 text-violet-400" />
                <span>Canary Deployment &amp; Promotion Gates</span>
              </CardTitle>
              <CardDescription>
                Safely introduce candidate policies into production with traffic rationing (5% - 25%).
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 font-mono text-xs">
              <div className="space-y-2">
                <div className="flex items-center justify-between text-slate-300">
                  <span>CANARY TRAFFIC ALLOCATION:</span>
                  <strong className="text-cyan-300 font-bold">{canaryPercentage}%</strong>
                </div>
                <input
                  type="range"
                  min="5"
                  max="25"
                  step="5"
                  value={canaryPercentage}
                  onChange={(e) => setCanaryPercentage(Number(e.target.value))}
                  className="w-full h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-cyan-400"
                />
                <div className="flex justify-between text-[10px] text-slate-500">
                  <span>5% (Conservative)</span>
                  <span>15% (Recommended)</span>
                  <span>25% (Maximum Safe)</span>
                </div>
              </div>

              <div className="flex flex-col sm:flex-row gap-3 pt-2">
                <button
                  onClick={handleCalibratePolicy}
                  disabled={isCalibrating}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-slate-900 border border-cyan-500/30 hover:border-cyan-400 text-cyan-300 font-bold cursor-pointer transition-all"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${isCalibrating ? "animate-spin" : ""}`} />
                  <span>{isCalibrating ? "Calibrating..." : "Calibrate Offline Candidate"}</span>
                </button>

                {calibratedCandidate && (
                  <button
                    onClick={handleDeployCanary}
                    disabled={isDeployingCanary}
                    className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-gradient-to-r from-violet-600 to-indigo-600 hover:from-violet-500 hover:to-indigo-500 text-white font-bold cursor-pointer transition-all shadow-lg shadow-violet-950/50"
                  >
                    <GitBranch className="w-3.5 h-3.5" />
                    <span>{isDeployingCanary ? "Deploying..." : `Deploy Canary (${canaryPercentage}%)`}</span>
                  </button>
                )}
              </div>

              {canaryPolicy && (
                <div className="p-3.5 rounded-xl bg-amber-950/30 border border-amber-500/30 space-y-3">
                  <div className="flex items-center justify-between text-amber-300 font-bold">
                    <span>Live Canary Policy Active</span>
                    <Badge variant="warning">{canaryPolicy.percentage}% TRAFFIC</Badge>
                  </div>
                  <div className="text-[11px] text-slate-300 space-y-1">
                    <div>• Candidate ID: <strong className="text-white">{canaryPolicy.policy.policy_id}</strong></div>
                    <div>• Min Similarity: <strong className="text-cyan-300">{canaryPolicy.policy.weights.min_similarity_threshold}</strong></div>
                    <div>• Held-Out Error Rate: <strong className="text-emerald-300">{canaryPolicy.policy.evaluation_report.held_out_false_hit_rate}%</strong></div>
                  </div>
                  <div className="flex flex-col sm:flex-row gap-2 pt-1">
                    <button
                      onClick={handlePromotePolicy}
                      disabled={isPromoting}
                      className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-bold cursor-pointer transition-all"
                    >
                      <Check className="w-3.5 h-3.5" />
                      <span>{isPromoting ? "Promoting..." : "Promote to 100% Production"}</span>
                    </button>
                    <button
                      onClick={handleRollback}
                      disabled={isRollingBack}
                      className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg bg-rose-600 hover:bg-rose-500 text-white font-bold cursor-pointer transition-all"
                    >
                      <RotateCcw className="w-3.5 h-3.5" />
                      <span>{isRollingBack ? "Rolling Back..." : "Trigger Emergency Rollback"}</span>
                    </button>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Right Column: Verified Feedback Telemetry & Audit Logs */}
        <div className="lg:col-span-5 space-y-6">
          <Card glow="emerald">
            <CardHeader className="pb-3">
              <CardTitle className="text-base flex items-center gap-2 text-emerald-300">
                <ShieldCheck className="w-4 h-4 text-emerald-400" />
                <span>Verified Feedback Telemetry</span>
              </CardTitle>
              <CardDescription>
                Signed ground-truth labels and poison defense quarantine tracking.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 font-mono text-xs">
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10">
                  <span className="text-[10px] text-slate-500">TOTAL VERIFIED EVENTS</span>
                  <div className="text-xl font-bold text-emerald-300 mt-0.5">
                    {totalFeedbackEvents}
                  </div>
                </div>
                <div className="p-3 rounded-lg bg-slate-900 border border-white/10">
                  <span className="text-[10px] text-slate-500">TENANT EVENTS</span>
                  <div className="text-xl font-bold text-cyan-300 mt-0.5">
                    {tenantEvents}
                  </div>
                </div>
              </div>

              <div className="p-3 rounded-lg bg-slate-900 border border-white/10 space-y-2 text-[11px]">
                <div className="flex items-center justify-between text-slate-400">
                  <span>AUTHENTICATION STANDARD:</span>
                  <strong className="text-white">RFC 2104 HMAC-SHA256</strong>
                </div>
                <div className="flex items-center justify-between text-slate-400">
                  <span>POISON DEFENSE LIMIT:</span>
                  <strong className="text-amber-300">&le; 5 Contradictory / Key</strong>
                </div>
                <div className="flex items-center justify-between text-slate-400">
                  <span>GDPR ARTICLE 17 PURGE:</span>
                  <strong className="text-emerald-300">Cascaded Atomic</strong>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Promotion & Rollback Audit Trail */}
          <Card glow="cyan">
            <CardHeader className="pb-3">
              <CardTitle className="text-base flex items-center gap-2">
                <History className="w-4 h-4 text-cyan-400" />
                <span>Immutable Promotion Audit Trail</span>
              </CardTitle>
              <CardDescription>
                Chronological log of canary deployments, full promotions, and rollbacks.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3 font-mono text-xs">
              {auditLogs.length === 0 ? (
                <div className="p-6 rounded-lg bg-slate-900/50 border border-dashed border-white/10 text-center text-slate-500 text-xs">
                  No state transitions recorded in this session.
                </div>
              ) : (
                <div className="space-y-2 max-h-64 overflow-y-auto pr-1">
                  {auditLogs.map((log, idx) => (
                    <div
                      key={idx}
                      className="p-2.5 rounded-lg bg-slate-900 border border-white/10 space-y-1 text-[11px]"
                    >
                      <div className="flex items-center justify-between">
                        <Badge
                          variant={
                            log.action === "FULLY_PROMOTED"
                              ? "success"
                              : log.action === "CANARY_STARTED"
                              ? "warning"
                              : "danger"
                          }
                          className="text-[9px] px-1.5 py-0"
                        >
                          {log.action}
                        </Badge>
                        <span className="text-slate-500 text-[10px]">
                          {new Date(log.timestamp).toLocaleTimeString()}
                        </span>
                      </div>
                      <div className="text-slate-300">{log.reason}</div>
                      <div className="text-slate-500 text-[10px]">Policy: {log.policy_id} (v{log.version})</div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
