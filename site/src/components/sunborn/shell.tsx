"use client";

import * as React from "react";
import { TesselShell, type TslNavItem } from "@/components/tessel/tessel";
import { BASE_PATH, REPO_URL, blob } from "@/lib/site";

type Page = "home" | "playground";

/** Sunborn's nav, footer and palette around a page. */
export function SunbornShell({ page, children }: { page: Page; children: React.ReactNode }) {
  const home = `${BASE_PATH}/`;
  const playground = `${BASE_PATH}/playground/`;
  // On the landing page, section links scroll in place; elsewhere they go back to it.
  const sec = (id: string) => (page === "home" ? `#${id}` : `${home}#${id}`);

  const nav: TslNavItem[] = [
    {
      label: "How it works",
      items: [
        { label: "Request pipeline", description: "Exact, semantic, then the model.", href: sec("product") },
        { label: "Guardrails", description: "Refuse reuse when the meaning changed.", href: sec("product") },
        { label: "Request trace", description: "Where the time goes, tier by tier.", href: sec("product") },
        { label: "Run it", description: "Docker Compose or Helm, one machine up.", href: sec("product") },
      ],
    },
    { label: "Evidence", href: sec("evidence") },
    { label: "Quickstart", href: sec("run") },
    { label: "Playground", href: playground, current: page === "playground" },
  ];

  return (
    <TesselShell
      brand="Sunborn"
      home={page === "home" ? "#top" : home}
      palette="ember"
      announcement={{ label: "playground can now talk to a gateway you run yourself", href: playground }}
      nav={nav}
      signIn={{ label: "GitHub", href: REPO_URL }}
      getStarted={{ label: "Try the playground", href: playground }}
      footer={{
        tagline: "A semantic cache for LLM APIs that answers repeats from cache and refuses reuse when the meaning changed.",
        columns: [
          {
            title: "Product",
            links: [
              { label: "Overview", href: home },
              { label: "Playground", href: playground },
              { label: "How it works", href: sec("product") },
              { label: "Evidence", href: sec("evidence") },
            ],
          },
          {
            title: "Run it",
            links: [
              { label: "Quickstart", href: blob("docs/QUICKSTART.md") },
              { label: "Docker Compose", href: blob("deploy/docker-compose.yml") },
              { label: "Site README", href: blob("site/README.md") },
            ],
          },
          {
            title: "Research",
            links: [
              { label: "Held-out benchmark", href: blob("research/results/heldout_generalization_run.json") },
              { label: "Reproduce script", href: blob("research/reproduce.sh") },
              { label: "Project README", href: blob("README.md") },
            ],
          },
          {
            title: "Source",
            links: [
              { label: "GitHub", href: REPO_URL },
              { label: "Issues", href: `${REPO_URL}/issues` },
              { label: "License", href: blob("LICENSE") },
            ],
          },
        ],
        note: "Sunborn · Apache 2.0 · Rust gateway with Postgres, Dragonfly and Qdrant",
        status: "Playground runs in your browser",
      }}
    >
      {children}
    </TesselShell>
  );
}
