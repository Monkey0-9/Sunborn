"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Compass } from "lucide-react";

type Entry = {
  what: string;
  notProven: string;
};

/**
 * One plain-English sentence per cockpit route. Kept here rather than repeated in
 * nine pages so the explanations cannot drift apart, and so every technical screen
 * says the same thing about what a green result does and does not mean.
 */
const COPY: Record<string, Entry> = {
  "/explorer": {
    what: "Type a request and see whether Sunborn answers it fresh or hands back a stored answer. Each result shows which route it took and why.",
    notProven: "a reused answer is not automatically correct. It only means the five checks allowed reuse for this request, tenant and configuration.",
  },
  "/knowledge": {
    what: "See which sources a saved answer depends on, and what Sunborn removes when one of those sources changes.",
    notProven: "the graph is only as current as the invalidation events it actually received.",
  },
  "/execution": {
    what: "Follow the steps of a multi-step task and see whether a saved step may be reused without repeating a real-world action.",
    notProven: "this screen does not by itself guarantee that an external system performed or did not perform an action.",
  },
  "/finops": {
    what: "Estimate what reuse saves in money and tokens, and change the assumptions to match your workload.",
    notProven: "these are projections from the numbers you enter, not a bill, an audited saving, or a promise of future cost.",
  },
  "/learning": {
    what: "See how a candidate threshold change is evaluated, deployed to a small share of traffic, promoted, or rolled back.",
    notProven: "a candidate being visible does not mean it is proven. Only promotion after evaluation changes what actually runs.",
  },
  "/governance": {
    what: "Erase a tenant's cached data and rebuild derived stores, with a report of what each step touched.",
    notProven: "a success message covers the stores the gateway reports, not every backup, replica or external system outside it.",
  },
  "/security": {
    what: "Watch the request firewall block unsafe prompts before they reach the cache, and read the audit trail.",
    notProven: "one blocked test prompt shows that one rule fired. It is not a security certification.",
  },
  "/login": {
    what: "Sign in a person or service and bind the session to one tenant and application.",
    notProven: "signing in proves who is asking. The five checks still decide whether any answer may be reused.",
  },
  "/ceremony": {
    what: "Review the trust setup: who may act on whose behalf, and which key service this deployment is configured to use.",
    notProven: "a configured key service is not proof that an independent audit has been completed.",
  },
};

export function PlainPageBand() {
  const pathname = usePathname();
  const entry = pathname ? COPY[pathname] : undefined;
  if (!entry) return null;

  return (
    <section
      aria-label="In plain English"
      className="mb-6 rounded-xl border border-cyan-400/20 bg-cyan-400/[0.04] px-4 py-3"
    >
      <div className="flex items-start gap-3">
        <Compass className="mt-0.5 h-4 w-4 shrink-0 text-cyan-300" aria-hidden />
        <div className="space-y-1">
          <p className="text-[10px] font-mono uppercase tracking-[0.14em] text-cyan-300">
            In plain English
          </p>
          <p className="text-sm leading-relaxed text-slate-200">{entry.what}</p>
          <p className="text-xs leading-relaxed text-slate-400">
            <span className="text-slate-300">What this does not prove:</span> {entry.notProven}
          </p>
          <Link
            href="/start"
            className="inline-block text-xs text-cyan-300 underline underline-offset-2 hover:text-cyan-200"
          >
            See the five checks explained simply →
          </Link>
        </div>
      </div>
    </section>
  );
}
