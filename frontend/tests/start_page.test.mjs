import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");

const pagePath = path.join(root, "src", "app", "start", "page.tsx");
const headerPath = path.join(root, "src", "components", "layout", "header.tsx");
const layoutPath = path.join(root, "src", "app", "layout.tsx");

test("the start page calls the real gateway and keeps the key out of storage", () => {
  const page = fs.readFileSync(pagePath, "utf-8");
  assert.match(page, /\/api\/proxy\/v1\/requests/, "must call the real request endpoint");
  assert.match(page, /\/api\/proxy\/health/, "must probe the real health endpoint");
  assert.match(page, /x-api-key/, "must forward an entered key through the BFF");
  assert.match(page, /type="password"/, "the key field must be masked");
  assert.doesNotMatch(page, /localStorage|sessionStorage/, "the key must not be persisted");
  assert.doesNotMatch(page, /simulationEngine/, "the plain page must not use simulated data");
});

test("the start page announces results and explains every gate", () => {
  const page = fs.readFileSync(pagePath, "utf-8");
  assert.match(page, /aria-live="polite"/, "results must be announced");
  assert.match(page, /explainDecision/, "must render the tested explanation");
  assert.match(page, /five checks/i);
  assert.match(
    page,
    /does\s+not\s+prove\s+the\s+answer\s+is\s+factually\s+correct/i,
    "must state that integrity is not factual correctness"
  );
});

test("navigation exposes the plain-English guide first", () => {
  const header = fs.readFileSync(headerPath, "utf-8");
  const startIndex = header.indexOf('name: "Start here"');
  const overviewIndex = header.indexOf('name: "Overview"');
  assert.ok(startIndex > -1, "Start here must appear in navigation");
  assert.ok(startIndex < overviewIndex, "Start here must come before Overview");
  assert.match(header, /href: "\/start"/);
});

test("the footer no longer claims live health or an unestablished risk bound", () => {
  const layout = fs.readFileSync(layoutPath, "utf-8");
  assert.doesNotMatch(layout, /All Subsystems Operational/);
  assert.doesNotMatch(layout, /α ≤ 0\.001/);
  assert.doesNotMatch(layout, /Independent audit: NO-GO/);
  assert.match(layout, /Status is not asserted in this footer/);
});
