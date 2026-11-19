import test from "node:test";
import assert from "node:assert/strict";

const { sunbornApi, SunbornApiError } = await import("../src/lib/api.ts");
const { getClusterState, setCircuitBreaker, setPersona } = await import("../src/lib/cluster-store.ts");
const { simulationEngine } = await import("../src/lib/simulation-engine.ts");

test("simulationEngine auto-infers preset from prompt text content", async () => {
  const negationRes = await simulationEngine.chatCompletions({
    messages: [{ content: "What are the mechanisms to bypass Sunborn non-accelerator authority?" }],
  });
  assert.equal(negationRes.sunborn_cached, false);
  assert.equal(negationRes.sunborn_action, "BypassVectorCache");

  const dagRes = await simulationEngine.chatCompletions({
    messages: [{ content: "Execute risk assessment DAG workflow for multi-currency portfolio rebalance" }],
  });
  assert.equal(dagRes.sunborn_cached, true);
  assert.equal(dagRes.sunborn_action, "PureToolDAGReused");
});

test("sunbornApi shredTenant gracefully returns simulated receipt when unauthenticated", async () => {
  const shredRes = await sunbornApi.shredTenant("tenant-public-demo");
  assert.ok(shredRes);
  assert.equal(shredRes.tenant_id, "tenant-public-demo");
  assert.equal(shredRes.status, "CRYPTOGRAPHICALLY_SHREDDED");
  assert.equal(shredRes.verified_multi_store_erasure, true);
  assert.equal(shredRes._simulated, true);
  assert.equal(getClusterState().isSimulated, true);
});

test("cluster-store cross-route circuit breaker reactive state synchronization", () => {
  setCircuitBreaker("CLOSED");
  assert.equal(getClusterState().circuitBreaker, "CLOSED");

  setCircuitBreaker("OPEN");
  assert.equal(getClusterState().circuitBreaker, "OPEN");

  setCircuitBreaker("CLOSED");
  assert.equal(getClusterState().circuitBreaker, "CLOSED");
});

test("cluster-store persona mode switching updates store correctly", () => {
  setPersona("executive");
  assert.equal(getClusterState().persona, "executive");

  setPersona("developer");
  assert.equal(getClusterState().persona, "developer");
});
