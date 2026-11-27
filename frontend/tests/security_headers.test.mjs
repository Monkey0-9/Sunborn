// Verifies the security headers Next.js actually serves, without starting a
// server. The CSP moved from `next.config.mjs` to the middleware so it can carry
// a per-request nonce; these tests exercise the same builder the middleware
// calls and assert the config no longer emits a competing policy.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  CSP_NONCE_HEADER,
  applyContentSecurityPolicy,
  buildContentSecurityPolicy,
  generateNonce,
} from '../src/lib/csp.mjs';

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

/** Splits a CSP into `{ directive: [sources] }`. */
function directives(policy) {
  return Object.fromEntries(
    policy
      .split(';')
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => {
        const [name, ...sources] = part.split(/\s+/);
        return [name, sources];
      }),
  );
}

test('production CSP uses a nonce instead of unsafe-inline for scripts', () => {
  const policy = buildContentSecurityPolicy({ nonce: 'test-nonce-value' });
  const parsed = directives(policy);
  const scriptSrc = parsed['script-src'];

  assert.ok(scriptSrc, 'script-src must be present');
  assert.ok(
    scriptSrc.includes("'nonce-test-nonce-value'"),
    `the nonce must be in script-src: ${policy}`,
  );
  assert.ok(
    scriptSrc.includes("'strict-dynamic'"),
    'strict-dynamic lets the trusted bootstrap load its own chunks without a host list',
  );
  assert.ok(
    !scriptSrc.includes("'unsafe-inline'"),
    `production script-src must not allow inline injection: ${policy}`,
  );
  assert.ok(
    !scriptSrc.includes("'unsafe-eval'"),
    `production CSP must not allow eval: ${policy}`,
  );

  assert.deepEqual(parsed['default-src'], ["'self'"]);
  assert.deepEqual(parsed['object-src'], ["'none'"]);
  assert.deepEqual(parsed['frame-ancestors'], ["'none'"]);
  assert.deepEqual(parsed['base-uri'], ["'self'"]);
  assert.deepEqual(parsed['form-action'], ["'self'"]);
});

test('development CSP keeps eval so React Refresh works', () => {
  const policy = buildContentSecurityPolicy({
    nonce: 'dev-nonce',
    isDevelopment: true,
  });
  assert.ok(
    directives(policy)['script-src'].includes("'unsafe-eval'"),
    'development CSP must allow eval for HMR',
  );
});

test('a nonce is required and is not caller-supplied empty', () => {
  assert.throws(
    () => buildContentSecurityPolicy({ nonce: '' }),
    /non-empty nonce/,
  );
  assert.throws(() => buildContentSecurityPolicy({}), /non-empty nonce/);
});

test('each nonce is 128 bits of base64 and differs per call', () => {
  const first = generateNonce();
  const second = generateNonce();

  assert.notEqual(first, second, 'nonces must not repeat between responses');
  assert.match(first, /^[A-Za-z0-9+/]+={0,2}$/, 'nonce must be base64');
  assert.equal(
    Buffer.from(first, 'base64').length,
    16,
    'nonce must decode to 16 random bytes',
  );
});

test('the middleware header helper writes the nonce and policy', () => {
  const headers = new Headers();
  const policy = applyContentSecurityPolicy(headers, { nonce: 'abc123' });

  assert.equal(headers.get(CSP_NONCE_HEADER), 'abc123');
  assert.equal(headers.get('Content-Security-Policy'), policy);
  assert.ok(policy.includes("'nonce-abc123'"));
});

test('the static config does not emit a second, nonce-less CSP', async () => {
  const headers = await headersFor('production');
  assert.equal(
    headers['Content-Security-Policy'],
    undefined,
    'CSP must come only from the middleware; a second policy would intersect and block the nonce scripts',
  );
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
