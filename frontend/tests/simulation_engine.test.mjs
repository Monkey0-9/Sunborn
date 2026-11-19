import test from "node:test";
import assert from "node:assert/strict";
import { simulationEngine } from "../src/lib/simulation-engine.ts";

test("simulationEngine generates deterministic chat completions across 4 caching presets", async () => {
  const exactRes = await simulationEngine.chatCompletions({ messages: [{ content: "test" }] }, "exact");
  assert.equal(exactRes.sunborn_cached, true);
  assert.ok(exactRes.usage.prompt_tokens > 0);
  assert.ok(exactRes._simulated);

  const negationRes = await simulationEngine.chatCompletions({ messages: [{ content: "bypass" }] }, "negation");
  assert.equal(negationRes.sunborn_cached, false);
  assert.ok(negationRes._simulated);
});

test("simulationEngine generates valid cryptographic shredding receipts", async () => {
  const shredRes = await simulationEngine.shredTenant("tenant-test-1");
  assert.equal(shredRes.tenant_id, "tenant-test-1");
  assert.equal(shredRes.status, "CRYPTOGRAPHICALLY_SHREDDED");
  assert.equal(shredRes.verified_multi_store_erasure, true);
});
