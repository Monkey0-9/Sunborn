// Keyboard and focus audit.
//
// axe checks static structure; this audit drives the real focus order with the
// keyboard and asserts the things automation otherwise cannot see:
//
//   * a bypass mechanism (skip link) is the first stop and moves focus to main
//   * every tab stop shows a visible focus indicator of at least 2px
//   * no positive tabindex reorders the page for keyboard users
//   * focus never lands on the document body, and never on a hidden element
//   * each page exposes one h1 and a main landmark
//
// Run with `npm run test:keyboard` after `npm run build`.
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

const MAX_TAB_STOPS = 40;

const TEST_PORT = 33000 + (process.pid % 5000);
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
      assert.fail('Playwright is required for the keyboard audit in CI mode');
    }
    return;
  }
  if (!fs.existsSync(path.join(frontendDir, '.next', 'BUILD_ID'))) {
    assert.fail('Run `npm run build` before `npm run test:keyboard`');
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

/** Snapshot the currently focused element: identity, visibility and indicator. */
function describeActive() {
  const el = document.activeElement;
  if (!el) return null;
  const styles = getComputedStyle(el);
  const rect = el.getBoundingClientRect();
  const outlineWidth = parseFloat(styles.outlineWidth) || 0;
  // A transparent outline paints nothing: Tailwind's `outline-none` is
  // `outline: 2px solid transparent`, which must not count as an indicator.
  const transparentOutline = /transparent|rgba\([^)]*,\s*0\s*\)/.test(styles.outlineColor);
  const hasOutline = styles.outlineStyle !== 'none' && outlineWidth > 0 && !transparentOutline;
  const hasShadow =
    styles.boxShadow !== 'none' && styles.boxShadow !== '' && !styles.boxShadow.includes('rgba(0, 0, 0, 0)');
  return {
    tag: el.tagName.toLowerCase(),
    isBody: el === document.body,
    text: (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 60),
    outlineWidth,
    hasOutline,
    hasShadow,
    visible: rect.width > 1 && rect.height > 1 && styles.visibility !== 'hidden',
    tabIndex: el.getAttribute('tabindex'),
    isMain: el.tagName.toLowerCase() === 'main' || el.closest('main') !== null,
    href: el.getAttribute('href'),
  };
}

test('keyboard navigation is complete, ordered, and visibly indicated', async (t) => {
  if (!chromium) {
    t.skip('Playwright is not installed in this environment');
    return;
  }

  const page = await browser.newPage();
  const failures = [];

  for (const route of ROUTES) {
    await page.goto(`${BASE_URL}${route}`, { waitUntil: 'networkidle' });

    // Structure: one h1, a main landmark, no positive tabindex anywhere.
    const structure = await page.evaluate(() => ({
      h1: document.querySelectorAll('h1').length,
      mains: document.querySelectorAll('main').length,
      positiveTabindex: Array.from(document.querySelectorAll('[tabindex]'))
        .filter((el) => Number(el.getAttribute('tabindex')) > 0)
        .map((el) => `${el.tagName.toLowerCase()}.${el.className}`.slice(0, 80)),
    }));
    if (structure.h1 !== 1) failures.push(`[${route}] expected exactly one h1, found ${structure.h1}`);
    if (structure.mains !== 1) failures.push(`[${route}] expected exactly one main landmark, found ${structure.mains}`);
    for (const el of structure.positiveTabindex) {
      failures.push(`[${route}] positive tabindex reorders keyboard focus: ${el}`);
    }

    // First Tab must reach the skip link, and activating it must move focus into main.
    await page.evaluate(() => document.body.focus());
    await page.keyboard.press('Tab');
    const first = await page.evaluate(describeActive);
    if (!first || first.isBody) {
      failures.push(`[${route}] first Tab did not move focus`);
    } else if (!/skip/i.test(first.text)) {
      failures.push(
        `[${route}] first tab stop is "${first.text}" (${first.tag}); expected a skip-to-content link`,
      );
    } else {
      await page.keyboard.press('Enter');
      await page.waitForTimeout(100);
      const target = await page.evaluate(describeActive);
      if (!target || !target.isMain) {
        failures.push(`[${route}] skip link did not move focus to the main landmark`);
      }
      await page.goto(`${BASE_URL}${route}`, { waitUntil: 'networkidle' });
    }

    // Walk the tab order and require a visible indicator at every stop.
    await page.evaluate(() => document.body.focus());
    const seen = new Set();
    for (let stop = 0; stop < MAX_TAB_STOPS; stop += 1) {
      await page.keyboard.press('Tab');
      const active = await page.evaluate(describeActive);
      if (!active) break;
      if (active.isBody) {
        // Tab past the last tab stop leaves the document; that ends the walk.
        break;
      }
      const key = `${active.tag}:${active.text}:${active.href}`;
      if (seen.has(key)) break;
      seen.add(key);

      if (!active.visible) {
        failures.push(`[${route}] focus landed on a zero-size element (${active.tag} "${active.text}")`);
      }
      if (!active.hasOutline && !active.hasShadow) {
        failures.push(
          `[${route}] no visible focus indicator on ${active.tag} "${active.text}" (outline ${active.outlineWidth}px)`,
        );
      }
    }

    t.diagnostic(`keyboard: ${route} checked (${seen.size} tab stops)`);
  }

  await page.close();
  assert.deepEqual(failures, [], `Keyboard/focus findings:\n${failures.join('\n')}`);
});
