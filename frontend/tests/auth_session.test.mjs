import test from "node:test";
import assert from "node:assert/strict";
import { evaluateProxySecurity } from "../src/lib/proxy-policy.ts";

test("Auth & Session: CSRF Origin validation on cookie-authenticated requests", () => {
  // 1. Mutating request with cookie auth but mismatched origin must be rejected with 403
  const csrfAttack = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/requests",
    clientCredential: null,
    origin: "https://malicious-attacker.com",
    host: "console.sunborn.ai",
    isCookieAuthenticated: true,
  });

  assert.equal(csrfAttack.allowed, false);
  assert.equal(csrfAttack.status, 403);
  assert.match(csrfAttack.detail, /Cross-site request forgery/);

  // 2. Mutating request with cookie auth and valid matching origin succeeds
  const validCsrf = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/requests",
    clientCredential: "authenticated-session-key",
    origin: "https://console.sunborn.ai",
    host: "console.sunborn.ai",
    isCookieAuthenticated: true,
  });

  assert.equal(validCsrf.allowed, true);
});

test("Auth & Session: Privileged route protection fails closed without credentials", () => {
  const adminAttempt = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/governance/shred",
    clientCredential: null,
    origin: "https://console.sunborn.ai",
    host: "console.sunborn.ai",
    isCookieAuthenticated: true,
  });

  assert.equal(adminAttempt.allowed, false);
  assert.equal(adminAttempt.status, 401);
  assert.match(adminAttempt.detail, /requires authenticated administrator credentials/);
});

test("Auth & Session: Login endpoint accessibility without ambient cookie", () => {
  // Public login endpoint is reachable without prior authentication
  const loginEval = evaluateProxySecurity({
    method: "POST",
    normalizedPath: "v1/auth/login",
  });
  assert.equal(loginEval.allowed, true);
});
