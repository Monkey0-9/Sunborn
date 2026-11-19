import test from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const cockpitDir = path.join(__dirname, '..', 'src', 'components', 'cockpit');

test('Cockpit Modular Components: All 5 components exist and define clean exports', () => {
  const components = [
    'metrics-ribbon.tsx',
    'prompt-probe-playground.tsx',
    'live-dag-visualizer.tsx',
    'cluster-mesh-status.tsx',
    'framework-integrations.tsx',
  ];

  for (const comp of components) {
    const fullPath = path.join(cockpitDir, comp);
    assert.ok(fs.existsSync(fullPath), `Cockpit component missing: ${comp}`);
    const content = fs.readFileSync(fullPath, 'utf8');

    // Ensure Client directive and component exports
    assert.ok(content.includes('"use client"'), `${comp} must have 'use client' directive`);
    assert.ok(
      content.includes('export function') || content.includes('export default'),
      `${comp} must export a component function`
    );

    // Ensure zero dangerous transition: all
    assert.ok(
      !content.includes('transition: all') && !content.includes('transition-all duration'),
      `${comp} must avoid unrestricted transition: all`
    );
  }
});
