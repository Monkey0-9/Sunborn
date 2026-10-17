import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendDir = path.resolve(__dirname, '..');
const srcDir = path.join(frontendDir, 'src');

test('Frontend Views: All 8 App Router views exist and define default export', () => {
  const views = [
    { name: 'Root Home', path: path.join(srcDir, 'app', 'page.tsx') },
    { name: 'Execution DAG', path: path.join(srcDir, 'app', 'execution', 'page.tsx') },
    { name: 'Cache Explorer', path: path.join(srcDir, 'app', 'explorer', 'page.tsx') },
    { name: 'FinOps Economics', path: path.join(srcDir, 'app', 'finops', 'page.tsx') },
    { name: 'Governance & Crypto-Shred', path: path.join(srcDir, 'app', 'governance', 'page.tsx') },
    { name: 'Knowledge Graph', path: path.join(srcDir, 'app', 'knowledge', 'page.tsx') },
    { name: 'Adaptive Learning', path: path.join(srcDir, 'app', 'learning', 'page.tsx') },
    { name: 'Security & Isolation', path: path.join(srcDir, 'app', 'security', 'page.tsx') },
  ];

  for (const view of views) {
    assert.ok(fs.existsSync(view.path), `View file must exist: ${view.name} (${view.path})`);
    const content = fs.readFileSync(view.path, 'utf8');
    assert.ok(
      content.includes('export default') || content.includes('export default function'),
      `View ${view.name} must export a default component`
    );
  }
});

test('Frontend API: Healthcheck endpoint exists and exports GET handler', () => {
  const healthRoute = path.join(srcDir, 'app', 'api', 'health', 'route.ts');
  assert.ok(fs.existsSync(healthRoute), 'Health route file must exist');
  const content = fs.readFileSync(healthRoute, 'utf8');
  assert.ok(content.includes('export async function GET'), 'Health route must export async GET handler');
});

test('Frontend Security: Zero sensitive production secrets in client codebase', () => {
  const forbiddenPatterns = [
    /admin-secret-key-change-me/i,
    /changeme/i,
    /dev-admin-key/i,
    /POSTGRES_PASSWORD\s*[:=]\s*["'][^"']+["']/i,
    /SUNBORN_MASTER_KEK\s*[:=]\s*["'][0-9a-fA-F]{64}["']/i,
  ];

  function scanDir(dir) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== 'node_modules' && entry.name !== '.next') {
          scanDir(fullPath);
        }
      } else if (/\.(tsx|ts|js|mjs|jsx|json)$/.test(entry.name) && !fullPath.includes('/tests/')) {
        const fileContent = fs.readFileSync(fullPath, 'utf8');
        for (const pattern of forbiddenPatterns) {
          assert.ok(
            !pattern.test(fileContent),
            `Secret pattern ${pattern} detected in client source: ${fullPath}`
          );
        }
      }
    }
  }

  scanDir(srcDir);
});

test('Frontend Layout & Navigation: Header navigation links to all views', () => {
  const headerPath = path.join(srcDir, 'components', 'layout', 'header.tsx');
  assert.ok(fs.existsSync(headerPath), 'header.tsx must exist');
  const headerContent = fs.readFileSync(headerPath, 'utf8');

  const expectedNavRoutes = [
    '/',
    '/explorer',
    '/execution',
    '/knowledge',
    '/governance',
    '/security',
    '/learning',
    '/finops',
  ];

  for (const route of expectedNavRoutes) {
    assert.ok(
      headerContent.includes(`href: "${route}"`) || headerContent.includes(`href: '${route}'`),
      `Header navigation must contain link to route: ${route}`
    );
  }
});
