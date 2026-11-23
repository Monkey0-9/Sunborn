import type { Metadata, Viewport } from "next";
import React from "react";
import "./globals.css";
import { SiteHeader } from "@/components/site-header";
import { REPO_URL } from "@/lib/site";

export const metadata: Metadata = {
  title: "Sunborn: a semantic cache for LLM APIs",
  description:
    "Sunborn is a Rust gateway that answers repeated and paraphrased LLM prompts from cache, and refuses reuse when the meaning changed. Try it in the playground.",
};

export const viewport: Viewport = {
  themeColor: "#020617",
  width: "device-width",
  initialScale: 1,
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark">
      <body className="relative min-h-screen overflow-x-hidden font-sans">
        <div className="pointer-events-none fixed inset-0 -z-10 overflow-hidden" aria-hidden>
          <div className="absolute -top-48 left-1/4 h-[520px] w-[720px] rounded-full bg-gradient-to-tr from-cyan-600/15 via-indigo-600/10 to-transparent blur-[150px]" />
          <div className="absolute right-0 top-[40%] h-[560px] w-[560px] rounded-full bg-gradient-to-bl from-violet-600/10 to-transparent blur-[170px]" />
          <div className="bg-grid absolute inset-0 [mask-image:radial-gradient(ellipse_70%_55%_at_50%_0%,#000_55%,transparent_100%)]" />
        </div>
        <SiteHeader />
        <main className="mx-auto max-w-6xl px-4 pb-24 sm:px-6">{children}</main>
        <footer className="border-t border-white/[0.06] py-8 text-sm text-slate-500">
          <div className="mx-auto flex max-w-6xl flex-col gap-2 px-4 sm:flex-row sm:items-center sm:justify-between sm:px-6">
            <span>Sunborn · Apache 2.0</span>
            <a href={REPO_URL} className="hover:text-slate-300">
              Source on GitHub
            </a>
          </div>
        </footer>
      </body>
    </html>
  );
}
