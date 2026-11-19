import test from "node:test";
import assert from "node:assert/strict";

// Dynamic import to test ESM or compiled file
const {
  getClusterState,
  setPersona,
  setCircuitBreaker,
  setByokApiKey,
  subscribeClusterState,
  resetClusterState,
  extractVectorClockEpoch,
} = await import("../src/lib/cluster-store.ts");

test("cluster-store manages persona, circuit breaker, and BYOK credentials reactively", () => {
  resetClusterState();
  const initial = getClusterState();
  assert.equal(initial.persona, "executive");
  assert.equal(initial.circuitBreaker, "CLOSED");
  assert.equal(initial.byokApiKey, null);

  let notifiedCount = 0;
  const unsubscribe = subscribeClusterState(() => {
    notifiedCount++;
  });

  setPersona("developer");
  assert.equal(getClusterState().persona, "developer");
  assert.equal(notifiedCount, 1);

  setPersona("executive");
  assert.equal(getClusterState().persona, "executive");
  assert.equal(notifiedCount, 2);

  setCircuitBreaker("OPEN");
  assert.equal(getClusterState().circuitBreaker, "OPEN");
  assert.equal(notifiedCount, 3);

  setCircuitBreaker("CLOSED");
  assert.equal(getClusterState().circuitBreaker, "CLOSED");
  assert.equal(notifiedCount, 4);

  setByokApiKey("sb-test-key-123");
  assert.equal(getClusterState().byokApiKey, "sb-test-key-123");
  assert.equal(notifiedCount, 5);

  setByokApiKey(null);
  assert.equal(getClusterState().byokApiKey, null);
  assert.equal(notifiedCount, 6);

  unsubscribe();
  setPersona("developer");
  assert.equal(notifiedCount, 6, "Listener must not be called after unsubscribe");
});

test("cluster-store extractVectorClockEpoch utility behaves correctly", () => {
  assert.equal(extractVectorClockEpoch(null), 1);
  assert.equal(extractVectorClockEpoch(5), 5);
  assert.equal(extractVectorClockEpoch({ epoch: 42 }), 42);
  assert.equal(extractVectorClockEpoch({ clocks: { "node-1": 10, "node-2": 25 } }), 25);
});
