import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const pagePath = path.resolve(__dirname, "..", "src", "app", "page.tsx");

test("overview page contains persona-adaptive layout and interactive sandbox wiring", () => {
  const content = fs.readFileSync(pagePath, "utf-8");
  assert.ok(content.includes("useClusterStore"), "Overview must consume cluster store");
  assert.ok(content.includes("Executive Mission Control") || content.includes("Executive Cockpit"));
  assert.ok(content.includes("Interactive Developer Sandbox"));
  assert.ok(content.includes("handleSelectPreset"));
});
