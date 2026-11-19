import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const headerPath = path.resolve(__dirname, "..", "src", "components", "layout", "header.tsx");

test("header source contains dual-mode persona switcher and BYOK trigger", () => {
  const content = fs.readFileSync(headerPath, "utf-8");
  assert.ok(content.includes("Executive Cockpit"), "Missing Executive Cockpit persona trigger");
  assert.ok(content.includes("Developer Sandbox"), "Missing Developer Sandbox persona trigger");
  assert.ok(content.includes("BYOK") || content.includes("Set API Key"), "Missing BYOK trigger");
  assert.ok(content.includes("useClusterStore"), "Header must bind to cluster store");
});
