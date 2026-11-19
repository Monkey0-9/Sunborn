import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("header source contains dual-mode persona switcher and BYOK trigger", () => {
  const content = fs.readFileSync("frontend/src/components/layout/header.tsx", "utf-8");
  assert.ok(content.includes("Executive Cockpit"), "Missing Executive Cockpit persona trigger");
  assert.ok(content.includes("Developer Sandbox"), "Missing Developer Sandbox persona trigger");
  assert.ok(content.includes("BYOK") || content.includes("Set API Key"), "Missing BYOK trigger");
  assert.ok(content.includes("useClusterStore"), "Header must bind to cluster store");
});
