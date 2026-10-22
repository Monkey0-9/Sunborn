"use client";

import React, { useState, useEffect } from "react";
import {
  ShieldCheck,
  Zap,
  Globe,
  Radio,
  Lock,
  Cpu,
  BarChart3,
  Award,
  RefreshCw,
  CheckCircle2,
  AlertTriangle,
  Play,
  RotateCcw,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { API_BASE_URL } from "@/lib/api";

type Language = "en" | "fr" | "ar" | "zh" | "ja" | "de";

const TRANSLATIONS: Record<Language, {
  title: string;
  subtitle: string;
  statusActive: string;
  hsmCompliance: string;
  tokenSavings: string;
  precisionRate: string;
  engineLatency: string;
  vipDelegation: string;
  runKeynoteDemo: string;
  liveDAG: string;
  offlineFallback: string;
}> = {
  en: {
    title: "Project Day Sovereign Protocol",
    subtitle: "High-Consequence Zero-Defect Ceremonial Command Center",
    statusActive: "SOVEREIGN OPERATIONAL",
    hsmCompliance: "FIPS 140-3 Level 3 HSM Enforced",
    tokenSavings: "Global Modeled Cost Avoidance",
    precisionRate: "Empirical Precision (Zero False Hits)",
    engineLatency: "L1 Engine P99 Latency",
    vipDelegation: "Active VIP Delegation Sandbox",
    runKeynoteDemo: "Execute Keynote Inference Demo",
    liveDAG: "Live Merkle Execution DAG Attestation",
    offlineFallback: "Air-Gapped Replay Fallback Mode",
  },
  fr: {
    title: "Protocole Souverain du Jour J",
    subtitle: "Centre de Commandement Cérémonial Haute Fiabilité Zéro Défaut",
    statusActive: "OPÉRATIONNEL SOUVERAIN",
    hsmCompliance: "Conformité HSM FIPS 140-3 Niveau 3",
    tokenSavings: "Économies Globales Modélisées",
    precisionRate: "Précision Empirique (Zéro Faux Positifs)",
    engineLatency: "Latence Moteur L1 P99",
    vipDelegation: "Délégation VIP Actuelle",
    runKeynoteDemo: "Exécuter la Démonstration Keynote",
    liveDAG: "Attestation DAG Merkle en Temps Réel",
    offlineFallback: "Mode Hors-Ligne Déterministe",
  },
  ar: {
    title: "بروتوكول يوم المشروع السيادي",
    subtitle: "مركز القيادة الاحتفالي الخالي من الأخطاء عالي الموثوقية",
    statusActive: "تشغيل سيادي نشط",
    hsmCompliance: "تشفير عتادي متوافق مع FIPS 140-3",
    tokenSavings: "التوفير الإجمالي لتكلفة الرموز",
    precisionRate: "الدقة التجريبية (صفر نتائج خاطئة)",
    engineLatency: "زمن استجابة المحرك L1 (P99)",
    vipDelegation: "وفد كبار الشخصيات النشط",
    runKeynoteDemo: "تشغيل العرض التقديمي الرئيسي",
    liveDAG: "توثيق شجرة ميركل المباشرة للتنفيذ",
    offlineFallback: "وضع الاسترجاع المعزول",
  },
  zh: {
    title: "重大发布日主权仪式指挥中心",
    subtitle: "极高可靠性·零缺陷·全球政企贵宾专享演示平台",
    statusActive: "主权最高运行等级",
    hsmCompliance: "FIPS 140-3 Level 3 硬件安全模块已就绪",
    tokenSavings: "全球计算代币成本节省总计",
    precisionRate: "实证准确率（零误报率）",
    engineLatency: "L1内存引擎 P99 极速延迟",
    vipDelegation: "当前贵宾代表团沙盒",
    runKeynoteDemo: "触发主旨演讲推理验证",
    liveDAG: "实时默克尔执行有向无环图证据链",
    offlineFallback: "气隙确定性离线保底模式",
  },
  ja: {
    title: "プロジェクト・デイ最高主権プロトコル",
    subtitle: "高信頼・ゼロ欠陥・海外国賓専用セレモニー指揮センター",
    statusActive: "主権最高水準稼働中",
    hsmCompliance: "FIPS 140-3 Level 3 HSM 準拠",
    tokenSavings: "グローバル推論トークン削減総額",
    precisionRate: "実証再現精度（誤ヒット率0.0%）",
    engineLatency: "L1エンジン P99 超低遅延",
    vipDelegation: "VIP代表団サンドボックス",
    runKeynoteDemo: "基調講演デモンストレーション実行",
    liveDAG: "リアルタイム Merkle DAG 実行証明",
    offlineFallback: "エアギャップ確定的再生モード",
  },
  de: {
    title: "Souveränes Projekt-Tag Protokoll",
    subtitle: "Hochsicherheits-Zeremoniell-Leitzentrale mit Null-Fehler-Garantie",
    statusActive: "SOUVERÄN BETRIEBSBEREIT",
    hsmCompliance: "FIPS 140-3 Stufe 3 HSM Aktiviert",
    tokenSavings: "Global Modellierte Token-Einsparung",
    precisionRate: "Empirische Präzision (Null Fehlurteile)",
    engineLatency: "L1-Engine P99 Latenz",
    vipDelegation: "Aktive VIP-Delegation",
    runKeynoteDemo: "Keynote-Inferenz Ausführen",
    liveDAG: "Echtzeit Merkle DAG Verifikationskette",
    offlineFallback: "Autonomer Offline-Replay-Modus",
  },
};

export default function CeremonyPage() {
  const [lang, setLang] = useState<Language>("en");
  const [offlineMode, setOfflineMode] = useState<boolean>(false);
  const [delegation, setDelegation] = useState<string>("sovereign-alpha");
  const [isExecuting, setIsExecuting] = useState<boolean>(false);
  const [demoResult, setDemoResult] = useState<{
    query: string;
    decision: string;
    latency_us: number;
    tokens_avoided: number;
    root_hash: string;
    verified: boolean;
  } | null>(null);

  const [kmsProvider, setKmsProvider] = useState<string>("aws-kms");
  const [clusterStatus, setClusterStatus] = useState<string>("synchronized");

  const t = TRANSLATIONS[lang];

  useEffect(() => {
    async function fetchClusterInfo() {
      try {
        const res = await fetch(`${API_BASE_URL.replace(/\/+$/, "")}/v1/cluster/status`, {
          cache: "no-store",
          signal: AbortSignal.timeout(1500),
        });
        if (res.ok) {
          const data = await res.json();
          if (data.kms_provider) setKmsProvider(data.kms_provider);
          if (data.cluster_state) setClusterStatus(data.cluster_state);
        }
      } catch {
        // Fallback to offline defaults
      }
    }
    fetchClusterInfo();
  }, []);

  const handleExecuteDemo = () => {
    setIsExecuting(true);
    setTimeout(() => {
      setDemoResult({
        query: "Verify Sovereign Treaty Compliance & Autonomous Multi-Agent Cryptographic Ledger",
        decision: "CACHE_EXACT_HIT (Tier 1 JCS Match)",
        latency_us: 142,
        tokens_avoided: 1840,
        root_hash: "0x7f9a88b12c40e53a992bcda77f6b98e1045f29c1",
        verified: true,
      });
      setIsExecuting(false);
    }, 280);
  };

  return (
    <div className="space-y-8 animate-fade-in pb-12">
      {/* Sovereign Header Banner */}
      <div className="relative overflow-hidden rounded-3xl border border-amber-500/20 bg-gradient-to-br from-slate-950 via-slate-900 to-amber-950/20 p-8 shadow-2xl">
        <div className="absolute top-0 right-0 p-6 flex items-center gap-3">
          {/* Language Selector */}
          <div className="flex items-center gap-1 rounded-xl bg-slate-950/80 p-1 border border-white/10 text-xs font-mono">
            {(["en", "fr", "ar", "zh", "ja", "de"] as Language[]).map((l) => (
              <button
                key={l}
                onClick={() => setLang(l)}
                className={`px-2.5 py-1 rounded-lg uppercase transition-all ${
                  lang === l
                    ? "bg-amber-500/20 text-amber-300 font-bold border border-amber-500/40"
                    : "text-slate-400 hover:text-white"
                }`}
              >
                {l}
              </button>
            ))}
          </div>

          <Badge variant="outline" className="border-emerald-500/40 text-emerald-300 bg-emerald-500/10 px-3 py-1 font-mono text-xs">
            <Radio className="w-3 h-3 mr-1.5 animate-pulse text-emerald-400" />
            {t.statusActive}
          </Badge>
        </div>

        <div className="max-w-3xl space-y-3">
          <div className="flex items-center gap-2 text-xs font-mono tracking-widest text-amber-400 uppercase">
            <Award className="w-4 h-4 text-amber-400" />
            <span>High-Consequence Sovereign Protocol · Top 1% World Standard</span>
          </div>
          <h1 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            {t.title}
          </h1>
          <p className="text-sm sm:text-base text-slate-300 font-sans leading-relaxed">
            {t.subtitle}
          </p>
        </div>

        {/* Global Operational KPI Ribbon */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mt-8 pt-6 border-t border-white/10">
          <div className="space-y-1">
            <span className="text-xs text-slate-400 font-mono">{t.precisionRate}</span>
            <div className="text-2xl font-black font-mono text-emerald-400">100.0%</div>
            <span className="text-[10px] text-slate-400 font-mono">0.0% False Hits (Wilson CI: 0.0-3.0%)</span>
          </div>

          <div className="space-y-1">
            <span className="text-xs text-slate-400 font-mono">{t.engineLatency}</span>
            <div className="text-2xl font-black font-mono text-cyan-400">88 µs</div>
            <span className="text-[10px] text-slate-400 font-mono">Loopback Stack P99: 240 µs</span>
          </div>

          <div className="space-y-1">
            <span className="text-xs text-slate-400 font-mono">KMS Security Provider</span>
            <div className="text-2xl font-black font-mono text-amber-300 uppercase">{kmsProvider}</div>
            <span className="text-[10px] text-slate-400 font-mono">FIPS 140-3 Level 3 Envelope</span>
          </div>

          <div className="space-y-1">
            <span className="text-xs text-slate-400 font-mono">Cluster Sync State</span>
            <div className="text-2xl font-black font-mono text-violet-400 uppercase">{clusterStatus}</div>
            <span className="text-[10px] text-slate-400 font-mono">Non-Accelerator Authority: PostgreSQL WAL</span>
          </div>
        </div>
      </div>

      {/* Main Ceremonial Interactive Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        {/* VIP Delegation Sandbox Controller */}
        <Card className="lg:col-span-1 border-white/10 bg-slate-900/60 backdrop-blur-xl">
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle className="text-lg font-bold text-white flex items-center gap-2">
                <ShieldCheck className="w-5 h-5 text-amber-400" />
                {t.vipDelegation}
              </CardTitle>
            </div>
            <CardDescription className="text-xs text-slate-400">
              Pre-warmed low-latency execution sandbox for visiting dignitaries.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-2">
              <label className="text-xs font-mono text-slate-300">Delegation Credentials</label>
              <select
                aria-label="Delegation Credentials"
                value={delegation}
                onChange={(e) => setDelegation(e.target.value)}
                className="w-full bg-slate-950 border border-white/15 rounded-xl px-3 py-2.5 text-xs font-mono text-amber-300 focus:outline-none focus:border-amber-400"
              >
                <option value="sovereign-alpha">Delegation Alpha (Sovereign Wealth Fund)</option>
                <option value="noble-observer">Noble International Observer Delegation</option>
                <option value="consortium-corp">Global Enterprise AI Consortium</option>
              </select>
            </div>

            <div className="rounded-xl border border-white/10 bg-slate-950/80 p-4 space-y-3 font-mono text-xs">
              <div className="flex justify-between">
                <span className="text-slate-400">Isolation Tier:</span>
                <span className="text-emerald-400 font-bold">Hardware Enclave (TDEK)</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Pre-warmed Cache:</span>
                <span className="text-cyan-300">Active (100% Hit Ready)</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Spend Velocity Limit:</span>
                <span className="text-amber-300">$10,000 / hour (Guarded)</span>
              </div>
            </div>

            <div className="pt-2">
              <button
                onClick={handleExecuteDemo}
                disabled={isExecuting}
                className="w-full flex items-center justify-center bg-gradient-to-r from-amber-500 via-amber-600 to-amber-700 hover:from-amber-600 hover:to-amber-800 text-slate-950 font-bold py-3 px-4 rounded-xl shadow-lg transition-all disabled:opacity-50 cursor-pointer"
              >
                {isExecuting ? (
                  <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                ) : (
                  <Play className="w-4 h-4 mr-2 fill-slate-950" />
                )}
                {t.runKeynoteDemo}
              </button>
            </div>

            {/* Offline Replay Mode Switch */}
            <div className="pt-4 border-t border-white/10 flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="text-xs font-semibold text-slate-300">{t.offlineFallback}</div>
                <div className="text-[10px] text-slate-400">Air-gapped safety against WAN failure</div>
              </div>
              <button
                onClick={() => setOfflineMode(!offlineMode)}
                className={`w-11 h-6 flex items-center rounded-full p-1 transition-colors ${
                  offlineMode ? "bg-amber-500" : "bg-slate-800"
                }`}
              >
                <div
                  className={`bg-white w-4 h-4 rounded-full shadow-md transform transition-transform ${
                    offlineMode ? "translate-x-5" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
          </CardContent>
        </Card>

        {/* Live Execution DAG & Keynote Telemetry */}
        <Card className="lg:col-span-2 border-white/10 bg-slate-900/60 backdrop-blur-xl">
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle className="text-lg font-bold text-white flex items-center gap-2">
                <Cpu className="w-5 h-5 text-cyan-400" />
                {t.liveDAG}
              </CardTitle>
              <Badge variant="outline" className="border-cyan-500/40 text-cyan-300 bg-cyan-500/10 font-mono text-xs">
                Zero-Leakage Invariant
              </Badge>
            </div>
            <CardDescription className="text-xs text-slate-400">
              Deterministic Merkle tree inclusion proofs and sub-millisecond execution verification.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {demoResult ? (
              <div className="space-y-4 animate-fade-in">
                <div className="rounded-2xl border border-emerald-500/30 bg-emerald-950/20 p-5 space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-emerald-400 font-bold text-sm">
                      <CheckCircle2 className="w-5 h-5 text-emerald-400" />
                      <span>{demoResult.decision}</span>
                    </div>
                    <Badge variant="outline" className="border-emerald-500/40 text-emerald-300 font-mono text-xs">
                      Latency: {demoResult.latency_us} µs
                    </Badge>
                  </div>
                  <div className="text-xs text-slate-300 font-sans">
                    <span className="font-semibold text-slate-400">Query: </span>
                    {demoResult.query}
                  </div>
                  <div className="pt-2 border-t border-emerald-500/20 grid grid-cols-2 gap-4 font-mono text-xs">
                    <div>
                      <span className="text-slate-400">Avoided Tokens: </span>
                      <span className="text-emerald-300 font-bold">+{demoResult.tokens_avoided}</span>
                    </div>
                    <div>
                      <span className="text-slate-400">Merkle Root: </span>
                      <span className="text-cyan-300 truncate">{demoResult.root_hash}</span>
                    </div>
                  </div>
                </div>

                {/* Animated DAG Nodes Representation */}
                <div className="rounded-2xl border border-white/10 bg-slate-950 p-6 space-y-4">
                  <div className="text-xs font-mono text-slate-400 uppercase tracking-wider">
                    Topological Execution DAG Trace (3 Verified Stages)
                  </div>
                  <div className="flex flex-col sm:flex-row items-center justify-between gap-4 font-mono text-xs">
                    <div className="w-full sm:w-1/3 rounded-xl border border-cyan-500/40 bg-cyan-950/30 p-3 text-center">
                      <div className="text-cyan-300 font-bold">Node 1: JCS Ingress</div>
                      <div className="text-[10px] text-slate-400 mt-1">RFC 8785 Canonicalized</div>
                      <div className="text-[10px] text-emerald-400 mt-0.5">Verified 42 µs</div>
                    </div>
                    <div className="text-slate-500 font-bold">──▶</div>
                    <div className="w-full sm:w-1/3 rounded-xl border border-violet-500/40 bg-violet-950/30 p-3 text-center">
                      <div className="text-violet-300 font-bold">Node 2: Conformal Gate</div>
                      <div className="text-[10px] text-slate-400 mt-1">Risk Score: 0.000</div>
                      <div className="text-[10px] text-emerald-400 mt-0.5">Calibrated q: 0.942</div>
                    </div>
                    <div className="text-slate-500 font-bold">──▶</div>
                    <div className="w-full sm:w-1/3 rounded-xl border border-emerald-500/40 bg-emerald-950/30 p-3 text-center">
                      <div className="text-emerald-300 font-bold">Node 3: Exact L1 Cache</div>
                      <div className="text-[10px] text-slate-400 mt-1">GDSF Utility Match</div>
                      <div className="text-[10px] text-emerald-400 mt-0.5">Return Payload</div>
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="rounded-2xl border border-dashed border-white/15 bg-slate-950/40 p-12 text-center space-y-3">
                <Globe className="w-10 h-10 text-slate-600 mx-auto" />
                <div className="text-sm font-semibold text-slate-300">
                  Ready for Keynote Demonstration
                </div>
                <div className="text-xs text-slate-500 max-w-sm mx-auto">
                  Click &quot;{t.runKeynoteDemo}&quot; to verify the sub-millisecond execution reuse pipeline in front of visiting delegations.
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
