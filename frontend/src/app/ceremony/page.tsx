"use client";

import React, { useState, useEffect } from "react";
import {
  ShieldCheck,
  Radio,
  Cpu,
  Award,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { API_BASE_URL } from "@/lib/api";
import { fetchFreshGatewayStatus } from "@/lib/gateway-status.mjs";

type Language = "en" | "fr" | "ar" | "zh" | "ja" | "de";

const TRANSLATIONS: Record<Language, {
  title: string;
  subtitle: string;
  statusUnverified: string;
  offlineFallback: string;
  vipDelegation: string;
  liveDAG: string;
  simulationMode: string;
}> = {
  en: {
    title: "Sunborn Demonstration",
    subtitle: "A demonstration interface. Gateway-reported status is shown with explicit verification limits.",
    statusUnverified: "STATUS UNVERIFIED",
    offlineFallback: "Offline mode (UI preview)",
    vipDelegation: "Demonstration Setup",
    liveDAG: "Execution Workflow Preview",
    simulationMode: "Simulation mode",
  },
  fr: {
    title: "Démonstration Sunborn",
    subtitle: "Interface de démonstration. L’état rapporté par la passerelle est affiché avec ses limites de vérification.",
    statusUnverified: "ÉTAT NON VÉRIFIÉ",
    offlineFallback: "Mode hors ligne (aperçu UI)",
    vipDelegation: "Configuration de démonstration",
    liveDAG: "Aperçu du flux d’exécution",
    simulationMode: "Mode simulation",
  },
  ar: {
    title: "عرض Sunborn التوضيحي",
    subtitle: "واجهة توضيحية. تُعرض حالة البوابة المُبلّغ عنها مع توضيح حدود التحقق.",
    statusUnverified: "الحالة غير متحقق منها",
    offlineFallback: "وضع عدم الاتصال (معاينة الواجهة)",
    vipDelegation: "إعداد العرض التوضيحي",
    liveDAG: "معاينة سير التنفيذ",
    simulationMode: "وضع المحاكاة",
  },
  zh: {
    title: "Sunborn 演示",
    subtitle: "演示界面。显示网关报告的状态，并明确说明验证限制。",
    statusUnverified: "状态未验证",
    offlineFallback: "离线模式（界面预览）",
    vipDelegation: "演示设置",
    liveDAG: "执行流程预览",
    simulationMode: "模拟模式",
  },
  ja: {
    title: "Sunborn デモ",
    subtitle: "デモ用インターフェースです。ゲートウェイが報告した状態を検証上の制限とともに表示します。",
    statusUnverified: "状態未確認",
    offlineFallback: "オフラインモード（UIプレビュー）",
    vipDelegation: "デモ設定",
    liveDAG: "実行フローのプレビュー",
    simulationMode: "シミュレーションモード",
  },
  de: {
    title: "Sunborn-Demonstration",
    subtitle: "Demonstrationsoberfläche. Vom Gateway gemeldeter Status wird mit klaren Prüfgrenzen angezeigt.",
    statusUnverified: "STATUS NICHT VERIFIZIERT",
    offlineFallback: "Offline-Modus (UI-Vorschau)",
    vipDelegation: "Demo-Einstellungen",
    liveDAG: "Vorschau des Ausführungsablaufs",
    simulationMode: "Simulationsmodus",
  },
};

export default function CeremonyPage() {
  const [lang, setLang] = useState<Language>("en");
  const [offlineMode, setOfflineMode] = useState<boolean>(false);
  const [delegation, setDelegation] = useState<string>("demo-profile-a");
  const simulationNotice = "Simulation only. No request was sent to the gateway, and no latency, savings, cache decision, or provenance was measured.";
  const [kmsProvider, setKmsProvider] = useState<string>("unknown");
  const [clusterStatus, setClusterStatus] = useState<string>("unknown");

  const t = TRANSLATIONS[lang];

  useEffect(() => {
    async function fetchClusterInfo() {
      const status = await fetchFreshGatewayStatus(
        (path, init) => fetch(`${API_BASE_URL.replace(/\/+$/, "")}${path}`, init),
      );
      if (status) {
        setKmsProvider(status.kmsProvider);
        setClusterStatus(status.clusterState);
      } else {
        setKmsProvider("unavailable");
        setClusterStatus("unavailable");
      }
    }
    fetchClusterInfo();
  }, []);

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

          <Badge variant="outline" className="border-amber-500/40 text-amber-300 bg-amber-500/10 px-3 py-1 font-mono text-xs">
            <Radio className="w-3 h-3 mr-1.5 text-amber-400" />
            {t.statusUnverified}
          </Badge>
        </div>

        <div className="max-w-3xl space-y-3">
          <div className="flex items-center gap-2 text-xs font-mono tracking-widest text-amber-400 uppercase">
            <Award className="w-4 h-4 text-amber-400" />
            <span>Sunborn interface demonstration</span>
          </div>
          <h1 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            {t.title}
          </h1>
          <p className="text-sm sm:text-base text-slate-300 font-sans leading-relaxed">
            {t.subtitle}
          </p>
        </div>

        <div className="mt-8 rounded-xl border border-amber-500/30 bg-amber-950/30 p-4 text-sm text-amber-100" role="status">
          Demonstration mode. Performance, security, savings, and cluster claims are not measured on this page.
        </div>
        <dl className="mt-4 grid gap-2 text-xs font-mono text-slate-300 sm:grid-cols-2" aria-label="Unverified gateway status">
          <div><dt className="inline text-slate-500">Gateway-reported provider: </dt><dd className="inline">{kmsProvider}</dd></div>
          <div><dt className="inline text-slate-500">Gateway-reported cluster state: </dt><dd className="inline">{clusterStatus}</dd></div>
        </dl>
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
              UI preview only; no dedicated or isolated delegation environment is connected.
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
                <option value="demo-profile-a">Demo profile A</option>
                <option value="demo-profile-b">Demo profile B</option>
                <option value="demo-profile-c">Demo profile C</option>
              </select>
            </div>

            <div className="rounded-xl border border-white/10 bg-slate-950/80 p-4 space-y-3 font-mono text-xs">
              <div className="flex justify-between">
                <span className="text-slate-400">Isolation Tier:</span>
                  <span className="text-amber-300 font-bold">Not verified</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Pre-warmed Cache:</span>
                <span className="text-slate-300">Not measured</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Spend Velocity Limit:</span>
                <span className="text-slate-300">Not configured</span>
              </div>
            </div>

            {/* Offline Replay Mode Switch */}
            <div className="pt-4 border-t border-white/10 flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="text-xs font-semibold text-slate-300">{t.offlineFallback}</div>
                <div className="text-[10px] text-slate-400">UI state only; no offline failover is connected</div>
              </div>
              <button
                aria-pressed={offlineMode}
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
                Not independently verified
              </Badge>
            </div>
            <CardDescription className="text-xs text-slate-400">
              Workflow preview only; this page does not produce or verify cryptographic proofs or measure execution time.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="rounded-2xl border border-amber-500/30 bg-amber-950/30 p-5 text-sm text-amber-100" role="status">
              <div className="mb-2 font-mono text-xs font-bold uppercase tracking-wider">{t.simulationMode}</div>
              {simulationNotice}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
