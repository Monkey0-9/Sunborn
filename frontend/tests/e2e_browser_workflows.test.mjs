import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawn, execSync } from 'node:child_process';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendDir = path.resolve(__dirname, '..');

const TEST_PORT = 38472;
const BASE_URL = `http://127.0.0.1:${TEST_PORT}`;

let nextProcess = null;
let browser = null;

before(async () => {
  // Ensure Next.js production build exists
  const buildIdPath = path.join(frontendDir, '.next', 'BUILD_ID');
  if (!fs.existsSync(buildIdPath)) {
    execSync('npx next build', { cwd: frontendDir, stdio: 'ignore' });
  }

  // Start Next.js in production mode
  const nextBin = path.join(frontendDir, 'node_modules', '.bin', 'next');
  nextProcess = spawn(nextBin, ['start', '-p', `${TEST_PORT}`], {
    cwd: frontendDir,
    env: { ...process.env, PORT: `${TEST_PORT}`, NODE_ENV: 'production' },
    stdio: 'ignore',
    detached: true,
  });

  // Wait for health check
  const startTime = Date.now();
  let isReady = false;
  while (Date.now() - startTime < 15000) {
    try {
      await new Promise((resolve, reject) => {
        const req = http.get(`${BASE_URL}/api/health`, (res) => {
          if (res.statusCode === 200) {
            isReady = true;
            resolve();
          } else {
            reject(new Error(`Status ${res.statusCode}`));
          }
        });
        req.on('error', reject);
        req.setTimeout(1000, () => {
          req.destroy();
          reject(new Error('Timeout'));
        });
      });
      if (isReady) break;
    } catch {
      await new Promise((r) => setTimeout(r, 250));
    }
  }

  assert.ok(isReady, `Next.js failed to start on ${BASE_URL} within 15 seconds`);

  // Launch headless browser
  try {
    browser = await chromium.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox'],
    });
  } catch (err) {
    if (process.env.CI === 'true' || process.env.STRICT_E2E === 'true' || process.env.STRICT_E2E === '1') {
      assert.fail(`Fatal E2E error: Chromium failed to launch in CI / STRICT_E2E mode: ${err.message}`);
    } else {
      console.warn('Chromium launch failed or browser not installed:', err.message);
    }
  }
});

function requireBrowser(t) {
  if (!browser) {
    if (process.env.CI === 'true' || process.env.STRICT_E2E === 'true' || process.env.STRICT_E2E === '1') {
      assert.fail('Fatal E2E error: Chromium browser is strictly required in CI / STRICT_E2E mode but was not available');
    }
    t.skip('Skipping: Chromium browser is not installed in this environment');
    return false;
  }
  return true;
}

after(async () => {
  if (browser) {
    await browser.close().catch(() => {});
  }
  if (nextProcess && nextProcess.pid) {
    try {
      process.kill(-nextProcess.pid, 'SIGKILL');
    } catch {
      try {
        nextProcess.kill('SIGKILL');
      } catch {}
    }
  }
});

test('E2E Workflow 1: Onboarding & Gateway Connectivity Lifecycle', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    await page.goto(`${BASE_URL}/`, { waitUntil: 'domcontentloaded' });

    // Verify hero presentation and headings
    const heroHeading = page.locator('h1');
    await heroHeading.waitFor({ timeout: 5000 });
    const headingText = await heroHeading.textContent();
    assert.match(headingText, /Deterministic Semantic Caching/);
    assert.match(headingText, /Agent Execution Reuse/);

    // Verify separation of Live Telemetry vs Frozen Scientific Benchmark Results
    const liveTelemetryHeading = page.locator('text=Live Production Telemetry (Gateway Cluster)');
    await liveTelemetryHeading.waitFor({ timeout: 3000 });
    assert.ok(await liveTelemetryHeading.isVisible(), 'Live Production Telemetry header must be visible');

    const benchmarkHeading = page.locator('text=Frozen Scientific Evaluation Results (SemanticCacheBench v1.0)');
    await benchmarkHeading.waitFor({ timeout: 3000 });
    assert.ok(await benchmarkHeading.isVisible(), 'Frozen Benchmark Results header must be visible');

    // Verify truthful metric labeling: "Not measured" badge when latency histogram is not provided
    const notMeasuredBadge = page.locator('text=Not measured (Requires Prometheus)');
    assert.ok(await notMeasuredBadge.isVisible(), 'Not measured badge must be displayed truthfully for latency');

    // Test Live Connection: Mock the proxy endpoint and click refresh
    await page.route('**/api/proxy/v1/finops/report', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          metrics: {
            requests_total: 12450,
            total_hits: 9840,
          },
          economics: {
            total_estimated_savings_usd: 142.5,
          },
        }),
      });
    });

    await page.route('**/api/proxy/v1/security/audit', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          events: [
            {
              id: 'sec_test_01',
              event: 'ZERO_TRUST_VERIFIED',
              origin: 'Gateway Ingress',
              detail: 'Tenant tenant-enterprise-fintech authenticated',
              status: '200_OK',
              time: 'Just now',
            },
          ],
        }),
      });
    });

    // Click Refresh button
    const refreshButton = page.locator('button:has-text("Refresh")');
    await refreshButton.click();

    // Verify Gateway Connected status badge appears
    const connectedBadge = page.locator('text=GATEWAY CONNECTED (LIVE TELEMETRY)');
    await connectedBadge.waitFor({ timeout: 5000 });
    assert.ok(await connectedBadge.isVisible(), 'Gateway connected badge must be visible after successful telemetry fetch');

    // Verify metric cards updated with live data
    const reqTotalLocator = page.locator('text=12,450');
    await reqTotalLocator.waitFor({ timeout: 3000 });
    assert.ok(await reqTotalLocator.isVisible(), 'Live request counter must reflect fetched telemetry');
  } finally {
    await context.close();
  }
});

test('E2E Workflow 2: Cache Exploration & Multi-Tier Resolution', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Intercept chat completions proxy
    await page.route('**/api/proxy/v1/chat/completions', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'chatcmpl_e2e_reused_42',
          choices: [
            {
              message: {
                role: 'assistant',
                content: 'Verified execution reuse from Sunborn DragonflyDB exact tier.',
              },
            },
          ],
          usage: {
            prompt_tokens: 30,
            completion_tokens: 15,
          },
          sunborn_cached: true,
          sunborn_tier: 'exact',
        }),
      });
    });

    await page.goto(`${BASE_URL}/explorer`, { waitUntil: 'domcontentloaded' });

    // Verify 10-D envelope form inputs
    const tenantInput = page.locator('label:has-text("TENANT ID") + input');
    await tenantInput.waitFor({ timeout: 3000 });
    assert.strictEqual(await tenantInput.inputValue(), 'tenant-enterprise-fintech');

    // Click Execute Cache Query button
    const executeButton = page.locator('button:has-text("Execute Cache Query")');
    await executeButton.waitFor({ timeout: 3000 });
    await executeButton.click();

    // Verify Execution Decision Trace card renders
    const traceCard = page.locator('text=Execution Decision Trace');
    await traceCard.waitFor({ timeout: 5000 });
    assert.ok(await traceCard.isVisible(), 'Execution Decision Trace card must be displayed');

    // Verify CACHE HIT badge and exact tier are displayed
    const cacheHitBadge = page.locator('text=CACHE HIT');
    await cacheHitBadge.waitFor({ timeout: 5000 });
    assert.ok(await cacheHitBadge.isVisible(), 'CACHE HIT badge must be visible');

    const tierBadge = page.locator('text=Tier: exact');
    assert.ok(await tierBadge.isVisible(), 'Tier: exact badge must be visible');

    const reusedContent = page.locator('text=Verified execution reuse from Sunborn DragonflyDB exact tier.');
    assert.ok(await reusedContent.isVisible(), 'Reused completion content must be displayed');
  } finally {
    await context.close();
  }
});

test('E2E Workflow 3: Execution DAG Inspection & Merkle Proof Attestation', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Mock Merkle attestation and verification endpoints
    await page.route('**/api/proxy/v1/governance/attest', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          root_hash: '7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0',
          proofs: [
            {
              leaf_hash: 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2',
              root_hash: '7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0',
              path: [],
            },
          ],
        }),
      });
    });

    await page.route('**/api/proxy/v1/governance/verify-proof', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          is_valid: true,
          valid: true,
          root_hash: '7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0',
          leaf_hash: 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2',
        }),
      });
    });

    // Mock speculative prefetching endpoint
    await page.route('**/api/proxy/v1/speculative/predict', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          predictions: [
            {
              predicted_action: 'fetch_document_content',
              confidence_score: 0.85,
              estimated_savings_ms: 380,
            },
          ],
          speculative_cache_warmed: true,
        }),
      });
    });

    await page.goto(`${BASE_URL}/execution`, { waitUntil: 'domcontentloaded' });

    // Verify DAG visualizer loaded
    const dagHeader = page.locator('text=Agent Workflow Subgraph (5 Nodes)');
    await dagHeader.waitFor({ timeout: 5000 });
    assert.ok(await dagHeader.isVisible(), 'Agent Workflow Subgraph must be displayed');

    // Verify Tool Purity Contract card is rendered
    const purityCard = page.locator('text=Tool Purity Contract');
    assert.ok(await purityCard.isVisible(), 'Tool Purity Contract must be displayed');

    // Click "Verify Proof" button
    const verifyButton = page.locator('button:has-text("Verify Proof")');
    await verifyButton.waitFor({ timeout: 3000 });
    await verifyButton.click();

    // Verify Proof Validated banner appears
    const proofBanner = page.locator('text=Proof Validated: Tamper-Evident Inclusion Guaranteed');
    await proofBanner.waitFor({ timeout: 5000 });
    assert.ok(await proofBanner.isVisible(), 'Proof Validated banner must be displayed');

    // Test Speculative Markov Prefetching
    const predictButton = page.locator('button:has-text("Predict Next")');
    await predictButton.waitFor({ timeout: 3000 });
    await predictButton.click();

    // Verify Markov predictions updated (scope to prediction results container)
    const confidenceBadge = page.locator('text=85%');
    await confidenceBadge.waitFor({ timeout: 5000 });
    assert.ok(await confidenceBadge.isVisible(), 'Prediction confidence must be displayed');
  } finally {
    await context.close();
  }
});

test('E2E Workflow 4: Security Firewall & Adversarial Injection Defense', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Mock security evaluation
    await page.route('**/api/proxy/v1/security/evaluate', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          threatDetected: true,
          ruleMatched: 'OWASP-LLM-01: System Prompt Extraction & Privilege Hijack',
          action: 'DROPPED_BEFORE_CACHE_EVALUATION',
          conformalScore: 0.999,
        }),
      });
    });

    await page.goto(`${BASE_URL}/security`, { waitUntil: 'domcontentloaded' });

    // Verify firewall simulator card
    const firewallCard = page.locator('text=Adversarial Prompt Firewall Simulator');
    await firewallCard.waitFor({ timeout: 5000 });
    assert.ok(await firewallCard.isVisible(), 'Firewall simulator card must be displayed');

    // Click "Evaluate Firewall" button
    const evalButton = page.locator('button:has-text("Evaluate Firewall")');
    await evalButton.waitFor({ timeout: 3000 });
    await evalButton.click();

    // Verify threat detection action badge
    const droppedBadge = page.locator('text=DROPPED_BEFORE_CACHE_EVALUATION');
    await droppedBadge.waitFor({ timeout: 5000 });
    assert.ok(await droppedBadge.isVisible(), 'DROPPED_BEFORE_CACHE_EVALUATION badge must be visible');

    // Verify Zero-Trust Invariants
    const invariantsCard = page.locator('text=Zero-Trust Architecture Invariants');
    assert.ok(await invariantsCard.isVisible(), 'Zero-Trust Architecture Invariants must be visible');
  } finally {
    await context.close();
  }
});

test('E2E Workflow 5: Governance Operations & GDPR Cryptographic Shredding', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Mock shred endpoint
    await page.route('**/api/proxy/v1/governance/shred', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          status: 'shredded',
          shred_latency_nanos: 39500, // 39.5 µs
          deleted_db_entries: 65,
          deleted_exact_cache: 92,
          deleted_vectors: 92,
          deleted_execution_tools: 18,
          deleted_learning_events: 12,
          verified_multi_store_erasure: true,
        }),
      });
    });

    await page.goto(`${BASE_URL}/governance`, { waitUntil: 'domcontentloaded' });

    // Verify GDPR Cryptographic Shredding card
    const shredCard = page.locator('text=GDPR Article 17: O(1) Cryptographic Shredding');
    await shredCard.waitFor({ timeout: 5000 });
    assert.ok(await shredCard.isVisible(), 'GDPR shredding card must be displayed');

    // Enter target tenant identifier
    const tenantInput = page.locator('label:has-text("TARGET TENANT IDENTIFIER") + input');
    await tenantInput.waitFor({ timeout: 3000 });
    await tenantInput.fill('tenant-e2e-audit-target');

    // Click execute cryptographic shred
    const shredButton = page.locator('button:has-text("Execute O(1) Cryptographic Shred")');
    await shredButton.waitFor({ timeout: 3000 });
    await shredButton.click();

    // Verify shred report card
    const shredSuccess = page.locator('text=Multi-Store Erasure & Shredding Completed');
    await shredSuccess.waitFor({ timeout: 5000 });
    assert.ok(await shredSuccess.isVisible(), 'Shredding completed banner must be displayed');

    // Verify latency and erasure confirmation
    const latencyMetric = page.locator('text=39.5 µs');
    assert.ok(await latencyMetric.isVisible(), 'Shred latency in microseconds must be displayed');

    const confirmedErasure = page.locator('text=CONFIRMED');
    assert.ok(await confirmedErasure.isVisible(), 'Verified multi-store erasure must show CONFIRMED');
  } finally {
    await context.close();
  }
});

test('E2E Workflow 6: Resilient Error Handling & Gateway Offline Recovery', async (t) => {
  if (!requireBrowser(t)) return;
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // 1. Test Gateway failure handling on Explorer page
    await page.route('**/api/proxy/v1/chat/completions', async (route) => {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({
          detail: 'Gateway connection pool exhausted (Simulated 503)',
        }),
      });
    });

    await page.goto(`${BASE_URL}/explorer`, { waitUntil: 'domcontentloaded' });

    const executeButton = page.locator('button:has-text("Execute Cache Query")');
    await executeButton.waitFor({ timeout: 3000 });
    await executeButton.click();

    // Verify error banner is displayed without crashing the UI
    const errorBanner = page.locator('text=GATEWAY EXECUTION FAILED');
    await errorBanner.waitFor({ timeout: 5000 });
    assert.ok(await errorBanner.isVisible(), 'Error banner must be rendered upon 503 failure');

    const errorDetail = page.locator('text=Gateway connection pool exhausted');
    assert.ok(await errorDetail.isVisible(), 'Error detail must be displayed');

    // 2. Test Governance failure handling
    await page.route('**/api/proxy/v1/governance/shred', async (route) => {
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({
          message: 'KMS master key decryption rejected by HSM policy',
        }),
      });
    });

    await page.goto(`${BASE_URL}/governance`, { waitUntil: 'domcontentloaded' });

    const shredButton = page.locator('button:has-text("Execute O(1) Cryptographic Shred")');
    await shredButton.waitFor({ timeout: 3000 });
    await shredButton.click();

    const govError = page.locator('text=GOVERNANCE OPERATION FAILED');
    await govError.waitFor({ timeout: 5000 });
    assert.ok(await govError.isVisible(), 'Governance failure banner must be displayed upon 500');

    // 3. Verify application navigation remains completely operational after errors
    const navLink = page.locator('a[href="/security"]');
    await navLink.first().click();
    await page.waitForURL('**/security');
    const secTitle = page.locator('h1');
    assert.match(await secTitle.textContent(), /Security Fortress/);
  } finally {
    await context.close();
  }
});
