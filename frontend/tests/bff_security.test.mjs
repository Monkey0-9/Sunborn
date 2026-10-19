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

test("BFF Security: All 12 privileged routes strictly reject anonymous access with 401", () => {
  const adminRoutes = [
    "v1/tenants",
    "v1/tenants/victim-123",
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
    const decision = evaluateProxySecurity({
      method: "POST",
      normalizedPath: route,
      clientCredential: null,
    });
    assert.equal(decision.allowed, false, `Route '${route}' must be rejected when unauthenticated`);
    assert.equal(decision.status, 401);
  }
});

test("BFF Security: SB-P0-05 Deny-by-default unauthenticated GET requests to protected endpoints return 401", () => {
  const protectedEndpoints = [
    "v1/models",
    "v1/finops/report",
    "v1/security/audit",
    "v1/cluster/status",
    "v1/learning/policies",
    "v1/chaos/status",
  ];

  for (const route of protectedEndpoints) {
    const decision = evaluateProxySecurity({
      method: "GET",
      normalizedPath: route,
      clientCredential: null,
    });
    assert.equal(
      decision.allowed,
      false,
      `Protected GET '${route}' must require authentication`
    );
    assert.equal(decision.status, 401);
    assert.equal(decision.error, "AUTHENTICATION_REQUIRED");
  }
});

test("BFF Security: Authenticated requests to protected endpoints are granted", () => {
  const decision = evaluateProxySecurity({
    method: "GET",
    normalizedPath: "v1/finops/report",
    clientCredential: "Bearer valid-user-token",
  });
  assert.equal(decision.allowed, true);
});

test("BFF Security: Malformed or non-HTTP origins are strictly rejected", () => {
  // Malformed URL
  const malformed = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/chat/completions",
    clientCredential: "Bearer user-token",
    origin: "not a valid url",
    host: "localhost:3000",
  });
  assert.equal(malformed.allowed, false);
  assert.equal(malformed.status, 400);
  assert.equal(malformed.error, "INVALID_ORIGIN_HEADER");

  // Unsafe javascript: protocol
  const jsOrigin = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/chat/completions",
    clientCredential: "Bearer user-token",
    origin: "javascript:void(0)",
    host: "localhost:3000",
  });
  assert.equal(jsOrigin.allowed, false);
  assert.equal(jsOrigin.status, 403);
  assert.equal(jsOrigin.error, "UNSAFE_ORIGIN_PROTOCOL");
});

test("BFF Security: Substring-based hostname bypass attempts are strictly rejected", () => {
  const evilHosts = [
    "http://evil-localhost.com:3000",
    "http://localhost.attacker.com",
    "http://127.0.0.1.attacker.io",
    "https://fake-localhost.net",
  ];

  for (const evilOrigin of evilHosts) {
    const decision = evaluateProxySecurity({
      method: "POST",
      normalizedPath: "v1/chat/completions",
      clientCredential: "Bearer user-token",
      origin: evilOrigin,
      host: "sunborn.production.internal",
    });
    assert.equal(
      decision.allowed,
      false,
      `Origin '${evilOrigin}' containing 'localhost' substring must be rejected`
    );
    assert.equal(decision.status, 403);
    assert.equal(decision.error, "CSRF_ORIGIN_MISMATCH");
  }
});

test("BFF Security: Missing Origin or Host on cookie-authenticated mutations is strictly rejected", () => {
  // Missing Origin on cookie-authenticated mutation
  const missingOrigin = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/chat/completions",
    clientCredential: "Bearer session-token",
    isCookieAuthenticated: true,
    origin: null,
    host: "localhost:3000",
  });
  assert.equal(missingOrigin.allowed, false);
  assert.equal(missingOrigin.status, 403);
  assert.equal(missingOrigin.error, "CSRF_MISSING_ORIGIN");

  // Missing Host on cookie-authenticated mutation
  const missingHost = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/chat/completions",
    clientCredential: "Bearer session-token",
    isCookieAuthenticated: true,
    origin: "http://localhost:3000",
    host: null,
  });
  assert.equal(missingHost.allowed, false);
  assert.equal(missingHost.status, 400);
  assert.equal(missingHost.error, "MISSING_HOST_HEADER");
});

test("BFF Security: Path normalization prevents traversal bypasses of privileged routes", () => {
  const traversalAttempts = [
    "v1//tenants",
    "v1/caches/../tenants",
    "v1/./governance/shred",
    "v1/nonexistent/../governance/shred",
    "v1/tenants/..//tenants",
  ];

  for (const rawPath of traversalAttempts) {
    const decision = evaluateProxySecurity({
      method: "POST",
      normalizedPath: rawPath,
      clientCredential: null,
    });
    assert.equal(
      decision.allowed,
      false,
      `Normalized path from '${rawPath}' must detect privileged target and reject anonymous access`
    );
    assert.equal(decision.status, 401);
    assert.equal(decision.error, "UNAUTHORIZED");
  }
});
