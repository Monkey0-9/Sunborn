import test from "node:test";
import assert from "node:assert/strict";

import {
  isPrivilegedRoute,
  isPublicRoute,
  evaluateProxySecurity,
} from "../src/lib/proxy-policy.ts";

test("BFF Security: Privileged administrative routes detection", () => {
  const adminRoutes = [
    "v1/tenants",
    "v1/tenants/tenant-123",
    "v1/governance/shred",
    "v1/learning/canary",
    "v1/learning/promote",
    "v1/learning/rollback",
    "v1/rebuild",
    "v1/invalidation",
    "v1/execution/evict",
    "v1/chaos/trip",
    "v1/chaos/reset",
    "v1/lifecycle/sweep",
  ];

  for (const route of adminRoutes) {
    assert.equal(
      isPrivilegedRoute(route),
      true,
      `Route '${route}' must be categorized as privileged admin route`
    );
  }
});

test("BFF Security: Public unauthenticated routes detection", () => {
  const publicRoutes = ["health", "ready", "metrics", "v1/auth/login"];

  for (const route of publicRoutes) {
    assert.equal(
      isPublicRoute(route),
      true,
      `Route '${route}' must be categorized as public unauthenticated route`
    );
  }
});

test("BFF Security: Non-privileged, non-public routes", () => {
  const normalRoutes = ["v1/chat/completions", "v1/models", "v1/finops/report"];

  for (const route of normalRoutes) {
    assert.equal(isPrivilegedRoute(route), false);
    assert.equal(isPublicRoute(route), false);
  }
});

test("BFF Security: P0-1 Anonymous caller on privileged admin endpoint is strictly rejected with 401", () => {
  // 1. Anonymous tenant deletion attempt
  const deleteDecision = evaluateProxySecurity({
    method: "DELETE",
    normalizedPath: "v1/tenants/victim-corp",
    clientCredential: null,
  });
  assert.equal(deleteDecision.allowed, false);
  assert.equal(deleteDecision.status, 401);
  assert.equal(deleteDecision.error, "UNAUTHORIZED");

  // 2. Anonymous cryptographic shredding attempt
  const shredDecision = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/governance/shred",
    clientCredential: null,
  });
  assert.equal(shredDecision.allowed, false);
  assert.equal(shredDecision.status, 401);
  assert.equal(shredDecision.error, "UNAUTHORIZED");

  // 3. Anonymous policy promotion attempt
  const promoteDecision = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/learning/promote",
    clientCredential: null,
  });
  assert.equal(promoteDecision.allowed, false);
  assert.equal(promoteDecision.status, 401);
  assert.equal(promoteDecision.error, "UNAUTHORIZED");
});

test("BFF Security: Authenticated administrator can access privileged endpoints", () => {
  const adminDecision = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/governance/shred",
    clientCredential: "Bearer verified-admin-key-token",
  });
  assert.equal(adminDecision.allowed, true);
});

test("BFF Security: Public endpoints require no credentials", () => {
  const readyDecision = evaluateProxySecurity({
    method: "GET",
    normalizedPath: "ready",
    clientCredential: null,
  });
  assert.equal(readyDecision.allowed, true);

  const healthDecision = evaluateProxySecurity({
    method: "GET",
    normalizedPath: "health",
    clientCredential: null,
  });
  assert.equal(healthDecision.allowed, true);
});

test("BFF Security: Cross-Site Request Forgery (CSRF) origin mismatch is rejected with 403", () => {
  const csrfDecision = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/chat/completions",
    clientCredential: "Bearer user-key",
    origin: "https://evil-attacker-site.com",
    host: "sunborn.production.internal",
  });
  assert.equal(csrfDecision.allowed, false);
  assert.equal(csrfDecision.status, 403);
  assert.equal(csrfDecision.error, "CSRF_ORIGIN_MISMATCH");
});
