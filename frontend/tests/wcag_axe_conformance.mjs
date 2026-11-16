// Real-browser WCAG 2.2 AA conformance sweep.
//
// The static accessibility test checks source conventions; this sweep runs axe
// against every rendered page and fails on any critical or serious violation.
// It is intentionally not part of `npm test` because it needs a production
// build and a running server; run it with `npm run test:a11y`.
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

let chromium = null;
try {
  ({ chromium } = await import('playwright'));
} catch {
  // Handled in before().
}

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const frontendDir = path.resolve(__dirname, '..');

const ROUTES = [
  '/',
  '/login',
  '/execution',
  '/explorer',
  '/finops',
  '/governance',
  '/knowledge',
  '/learning',
  '/security',
  '/ceremony',
];

// WCAG 2.2 levels A and AA. best-practice rules are excluded because they are
// not success criteria.
const AXE_OPTIONS = {
  runOnly: {
    type: 'tag',
    values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'],
  },
  resultTypes: ['violations'],
};

const TEST_PORT = 32000 + (process.pid % 6000);
const BASE_URL = `http://127.0.0.1:${TEST_PORT}`;

let server = null;
let browser = null;

function waitForServer(timeoutMs = 45000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const attempt = () => {
      const req = http.get(`${BASE_URL}/api/health`, (res) => {
        res.resume();
        if (res.statusCode && res.statusCode < 500) {
          resolve();
        } else {
          retry();
        }
      });
      req.on('error', retry);
      req.setTimeout(2000, () => {
        req.destroy();
        retry();
      });

      function retry() {
        if (Date.now() > deadline) {
          reject(new Error(`Next.js did not become ready at ${BASE_URL}`));
          return;
        }
        setTimeout(attempt, 500);
      }
    };
    attempt();
  });
}

before(async () => {
  if (!chromium) {
    if (process.env.CI === 'true' || process.env.STRICT_E2E === 'true') {
      assert.fail('Playwright is required for the WCAG sweep in CI mode');
    }
    return;
  }
  if (!fs.existsSync(path.join(frontendDir, '.next', 'BUILD_ID'))) {
    assert.fail('Run `npm run build` before `npm run test:a11y`');
  }

  browser = await chromium.launch();
  const nextBin = path.join(frontendDir, 'node_modules', 'next', 'dist', 'bin', 'next');
  server = spawn(
    process.execPath,
    [nextBin, 'start', '-p', `${TEST_PORT}`, '-H', '127.0.0.1'],
    {
      cwd: frontendDir,
      env: { ...process.env, PORT: `${TEST_PORT}`, NODE_ENV: 'production' },
      stdio: ['ignore', 'ignore', 'pipe'],
    },
  );
  await waitForServer();
});

after(async () => {
  if (browser) await browser.close();
  if (server) {
    server.kill();
    await new Promise((resolve) => server.once('exit', resolve));
  }
});

test('every rendered page passes axe WCAG 2.2 A/AA with no serious or critical violations', async (t) => {
  if (!chromium) {
    t.skip('Playwright is not installed in this environment');
    return;
  }

  const axeSource = fs.readFileSync(
    path.join(frontendDir, 'node_modules', 'axe-core', 'axe.min.js'),
    'utf8',
  );
  const page = await browser.newPage();
  const failures = [];

  for (const route of ROUTES) {
    await page.goto(`${BASE_URL}${route}`, { waitUntil: 'networkidle' });
    await page.addScriptTag({ content: axeSource });
    const result = await page.evaluate(
      (options) => window.axe.run(document, options),
      AXE_OPTIONS,
    );
    for (const violation of result.violations) {
      if (violation.impact === 'critical' || violation.impact === 'serious') {
        const nodes = violation.nodes.map((n) => n.target.join(' ')).join(' | ');
        failures.push(`[${route}] ${violation.id} (${violation.impact}): ${nodes}`);
      }
    }
    t.diagnostic(`axe: ${route} checked (${result.violations.length} violation groups)`);
  }

  await page.close();
  assert.deepEqual(failures, [], `WCAG violations:\n${failures.join('\n')}`);
});
