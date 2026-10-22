import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fetchFreshGatewayStatus } from '../src/lib/gateway-status.mjs';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const now = Date.parse('2026-10-22T12:00:00.000Z');
const response = (body, status = 200) => ({
  ok: status >= 200 && status < 300,
  json: async () => body,
});

test('ceremony accepts only fresh, complete gateway status and labels it as a report', async () => {
  const status = await fetchFreshGatewayStatus(async () => response({
    kms_provider: 'local-aead',
    cluster_state: 'single-node',
    status_observed_at: new Date(now).toISOString(),
  }), now);
  assert.deepEqual(status, { kmsProvider: 'local-aead', clusterState: 'single-node' });
});

test('ceremony rejects stale or malformed status payloads', async () => {
  const stale = await fetchFreshGatewayStatus(async () => response({
    kms_provider: 'aws-kms',
    cluster_state: 'synchronized',
    status_observed_at: new Date(now - 120_000).toISOString(),
  }), now);
  const malformed = await fetchFreshGatewayStatus(async () => response([]), now);
  const incomplete = await fetchFreshGatewayStatus(async () => response({ kms_provider: 'aws-kms' }), now);
  assert.equal(stale, null);
  assert.equal(malformed, null);
  assert.equal(incomplete, null);
});

test('ceremony maps non-OK responses and request timeouts to unavailable status', async () => {
  const unavailable = await fetchFreshGatewayStatus(async () => response({}, 503), now);
  const keepAlive = setTimeout(() => {}, 20);
  const timeout = await fetchFreshGatewayStatus((_url, init) => new Promise((_, reject) => {
    init.signal.addEventListener('abort', () => reject(new Error('timeout')), { once: true });
  }), now, 1);
  clearTimeout(keepAlive);
  assert.equal(unavailable, null);
  assert.equal(timeout, null);
});

test('ceremony page is explicitly a UI-only simulation with no hardcoded live outcome', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/app/ceremony/page.tsx'), 'utf8');
  assert.match(source, /Simulation mode/);
  assert.match(source, /No request was sent to the gateway/);
  assert.match(source, /does not produce or verify cryptographic proofs/);
  assert.doesNotMatch(source, /142\s*µs|1840|0x7f9a88b12c40e53a992bcda77f6b98e1045f29c1|CACHE_EXACT_HIT/);
});
