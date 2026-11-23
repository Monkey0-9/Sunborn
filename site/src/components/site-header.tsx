"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Github } from "lucide-react";
import { REPO_URL } from "@/lib/site";

const NAV = [
  { href: "/", label: "Overview" },
  { href: "/playground/", label: "Playground" },
];

export function SiteHeader() {
  const pathname = usePathname() || "/";
  const norm = (p: string) => (p.endsWith("/") ? p : `${p}/`);

  return (
    <header className="sticky top-0 z-30 border-b border-white/[0.06] bg-[#020617]/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between gap-4 px-4 sm:px-6">
        <Link href="/" className="flex items-center gap-2.5 rounded-lg">
          <span className="h-2.5 w-2.5 rounded-full bg-cyan shadow-glow" aria-hidden />
          <span className="font-semibold tracking-[0.2em] text-white">SUNBORN</span>
        </Link>
        <nav aria-label="Main" className="flex items-center gap-1 text-sm">
          {NAV.map((item) => {
            const active = norm(pathname) === norm(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                aria-current={active ? "page" : undefined}
                className={
                  "rounded-lg px-3 py-1.5 transition-colors " +
                  (active ? "bg-white/[0.08] text-white" : "text-slate-400 hover:text-white")
                }
              >
                {item.label}
              </Link>
            );
          })}
          <a
            href={REPO_URL}
            className="ml-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-slate-400 transition-colors hover:text-white"
          >
            <Github className="h-4 w-4" aria-hidden />
            <span className="hidden sm:inline">GitHub</span>
          </a>
        </nav>
      </div>
    </header>
  );
}
