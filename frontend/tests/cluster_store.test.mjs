import test from "node:test";
import assert from "node:assert/strict";
import { getClusterState, setPersona, setCircuitBreaker, setByokApiKey } from "../src/lib/cluster-store.ts";

test("cluster-store manages persona, circuit breaker, and BYOK credentials reactively", () => {
  setPersona("developer");
  assert.equal(getClusterState().persona, "developer");
  setPersona("executive");
  assert.equal(getClusterState().persona, "executive");

  setCircuitBreaker("OPEN");
  assert.equal(getClusterState().circuitBreaker, "OPEN");
  setCircuitBreaker("CLOSED");
  assert.equal(getClusterState().circuitBreaker, "CLOSED");

  setByokApiKey("sb-test-key-123");
  assert.equal(getClusterState().byokApiKey, "sb-test-key-123");
  setByokApiKey(null);
  assert.equal(getClusterState().byokApiKey, null);
});
