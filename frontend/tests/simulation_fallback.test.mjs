import test from "node:test";
import assert from "node:assert/strict";

const { sunbornApi } = await import("../src/lib/api.ts");
const { getClusterState } = await import("../src/lib/cluster-store.ts");

test("sunbornApi falls back gracefully to simulationEngine when gateway is offline", async () => {
  // Offline call without running backend should fall back cleanly
  const res = await sunbornApi.chatCompletions({ messages: [{ content: "test" }] });
  assert.ok(res);
  assert.ok(res.choices && res.choices.length > 0);
  assert.equal(res._simulated, true);
  assert.equal(getClusterState().isSimulated, true);
});

test("sunbornApi cluster status returns valid data in offline simulation mode", async () => {
  const status = await sunbornApi.getClusterStatus();
  assert.ok(status);
  assert.ok(status.vector_clock);
  assert.ok(status._simulated);
});
