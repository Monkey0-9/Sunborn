'use client';

import React, { useState } from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ShieldCheck, Lock, Key, Building2, ArrowRight, CheckCircle2, AlertCircle } from 'lucide-react';

export default function LoginPage() {
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [tenantId, setTenantId] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [session, setSession] = useState<{
    tenant_id: string;
    application_id: string;
    is_admin: boolean;
  } | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    try {
      const res = await fetch('/api/auth', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'login',
          client_id: clientId,
          client_secret: clientSecret,
          tenant_id: tenantId || undefined,
        }),
      });

      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || 'Authentication failed');
      }

      setSession({
        tenant_id: data.tenant_id,
        application_id: data.application_id,
        is_admin: data.is_admin,
      });
    } catch (err: any) {
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    setIsLoading(true);
    try {
      await fetch('/api/auth', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'logout' }),
      });
      setSession(null);
      setClientSecret('');
    } catch (err: any) {
      setError(err.message);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="max-w-xl mx-auto space-y-6 py-8">
      <div className="flex items-center justify-between border-b border-white/[0.08] pb-4">
        <div>
          <h1 className="text-2xl font-bold text-white tracking-tight flex items-center gap-2">
            <Lock className="w-6 h-6 text-cyan-400" />
            Platform Authentication
          </h1>
          <p className="text-sm text-slate-400 mt-1 font-mono">
            HttpOnly, SameSite=Lax Session Authentication & RBAC Gate
          </p>
        </div>
        <Badge variant="cyan" dot>
          GATEWAY BFF AUTH
        </Badge>
      </div>

      {session ? (
        <Card glow="emerald" className="border-emerald-500/20">
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle className="text-base text-emerald-400 flex items-center gap-2">
                <CheckCircle2 className="w-5 h-5" />
                Active Verified Session
              </CardTitle>
              <Badge variant="emerald">HttpOnly Cookie Bound</Badge>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="bg-slate-950/60 rounded p-4 font-mono text-xs space-y-2 border border-white/[0.06]">
              <div className="flex justify-between">
                <span className="text-slate-400">Authenticated Tenant:</span>
                <span className="text-emerald-300 font-bold">{session.tenant_id}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Application Scope:</span>
                <span className="text-slate-200">{session.application_id}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Administrative Authority:</span>
                <span className={session.is_admin ? "text-cyan-400 font-bold" : "text-slate-400"}>
                  {session.is_admin ? "Administrator (Full Quorum)" : "Standard Tenant"}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Cookie Security:</span>
                <span className="text-slate-300">HttpOnly · SameSite=Lax · Anti-CSRF Protected</span>
              </div>
            </div>

            <button
              type="button"
              onClick={handleLogout}
              disabled={isLoading}
              className="w-full py-2.5 px-4 bg-rose-600/20 hover:bg-rose-600/30 text-rose-300 border border-rose-500/30 rounded-lg text-sm font-medium transition focus:outline-none focus:ring-2 focus:ring-rose-500"
            >
              Sign Out & Invalidate Session
            </button>
          </CardContent>
        </Card>
      ) : (
        <Card glow="cyan">
          <CardHeader>
            <CardTitle className="text-base text-white">Sign In to Sunborn Console</CardTitle>
          </CardHeader>
          <CardContent>
            {error && (
              <div
                role="alert"
                aria-live="polite"
                className="mb-4 p-3 rounded bg-rose-950/40 border border-rose-500/30 text-rose-300 text-xs font-mono flex items-center gap-2"
              >
                <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                <span>{error}</span>
              </div>
            )}

            <form onSubmit={handleSubmit} className="space-y-4">
              <div>
                <label
                  htmlFor="client_id"
                  className="block text-xs font-mono text-slate-300 uppercase tracking-wider mb-1"
                >
                  Client / User Identifier *
                </label>
                <div className="relative">
                  <input
                    id="client_id"
                    type="text"
                    required
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    placeholder="e.g. enterprise-admin"
                    className="w-full bg-slate-950/80 border border-white/[0.1] rounded-lg px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500 font-mono"
                  />
                </div>
              </div>

              <div>
                <label
                  htmlFor="client_secret"
                  className="block text-xs font-mono text-slate-300 uppercase tracking-wider mb-1"
                >
                  API Secret / Bearer Key *
                </label>
                <div className="relative">
                  <input
                    id="client_secret"
                    type="password"
                    required
                    value={clientSecret}
                    onChange={(e) => setClientSecret(e.target.value)}
                    placeholder="sb_live_..."
                    className="w-full bg-slate-950/80 border border-white/[0.1] rounded-lg px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500 font-mono"
                  />
                </div>
              </div>

              <div>
                <label
                  htmlFor="tenant_id"
                  className="block text-xs font-mono text-slate-300 uppercase tracking-wider mb-1"
                >
                  Tenant ID (Optional Scope Binding)
                </label>
                <div className="relative">
                  <input
                    id="tenant_id"
                    type="text"
                    value={tenantId}
                    onChange={(e) => setTenantId(e.target.value)}
                    placeholder="e.g. tenant-finance"
                    className="w-full bg-slate-950/80 border border-white/[0.1] rounded-lg px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500 font-mono"
                  />
                </div>
              </div>

              <button
                type="submit"
                disabled={isLoading}
                className="w-full py-2.5 px-4 bg-cyan-600 hover:bg-cyan-500 text-white font-medium rounded-lg text-sm transition flex items-center justify-center gap-2 focus:outline-none focus:ring-2 focus:ring-cyan-400 disabled:opacity-50"
              >
                {isLoading ? (
                  <span>Authenticating...</span>
                ) : (
                  <>
                    <span>Authenticate Session</span>
                    <ArrowRight className="w-4 h-4" />
                  </>
                )}
              </button>
            </form>
          </CardContent>
        </Card>
      )}

      <div className="text-xs text-slate-500 font-mono text-center space-y-1">
        <p>Sessions are cryptographically bound to HttpOnly, SameSite=Lax browser cookies.</p>
        <p>Never transmit raw master keys or production database credentials via client forms.</p>
      </div>
    </div>
  );
}
