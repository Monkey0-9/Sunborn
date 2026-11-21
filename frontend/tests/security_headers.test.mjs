// Verifies the headers Next.js actually serves, without starting a server.
// `next.config.mjs` reads NODE_ENV at module load, so each case imports a fresh
// copy via a cache-busting query string.
import assert from 'node:assert/strict';
import { test } from 'node:test';

const CONFIG_URL = new URL('../next.config.mjs', import.meta.url);

async function loadConfig(env) {
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = env;
  try {
    const url = new URL(CONFIG_URL.href);
    url.searchParams.set('env', `${env}-${Math.random().toString(36).slice(2)}`);
    const module = await import(url.href);
    return module.default;
  } finally {
    if (previous === undefined) {
      delete process.env.NODE_ENV;
    } else {
      process.env.NODE_ENV = previous;
    }
  }
}

async function headersFor(env) {
  const config = await loadConfig(env);
  const [entry] = await config.headers();
  return Object.fromEntries(entry.headers.map(({ key, value }) => [key, value]));
}

test('production CSP does not permit eval', async () => {
  const headers = await headersFor('production');
  const csp = headers['Content-Security-Policy'];

  assert.ok(csp, 'production must send a Content-Security-Policy');
  assert.ok(!csp.includes("'unsafe-eval'"), `production CSP must not allow eval: ${csp}`);
  assert.ok(csp.includes("script-src 'self'"), 'scripts must be restricted to same origin');
  assert.ok(csp.includes("object-src 'none'"), 'plugins must be blocked');
  assert.ok(csp.includes("frame-ancestors 'none'"), 'framing must be blocked');
  assert.ok(csp.includes("base-uri 'self'"), 'base-uri must be restricted');
});

test('development CSP keeps eval so React Refresh works', async () => {
  const headers = await headersFor('development');
  const csp = headers['Content-Security-Policy'];

  assert.ok(csp.includes("'unsafe-eval'"), 'development CSP must allow eval for HMR');
});

test('deprecated XSS auditor is explicitly disabled', async () => {
  const headers = await headersFor('production');
  assert.equal(headers['X-XSS-Protection'], '0');
});

test('framework identity is not disclosed and core hardening headers are present', async () => {
  const config = await loadConfig('production');
  assert.equal(config.poweredByHeader, false, 'X-Powered-By must be disabled');

  const headers = await headersFor('production');
  assert.equal(headers['X-Frame-Options'], 'DENY');
  assert.equal(headers['X-Content-Type-Options'], 'nosniff');
  assert.ok(headers['Strict-Transport-Security']?.includes('max-age='));
  assert.ok(headers['Referrer-Policy']);
  assert.ok(headers['Permissions-Policy']?.includes('camera=()'));
});
