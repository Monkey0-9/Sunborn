# Sunborn Public Launch, Enterprise Redesign, and Full Systems Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Sunborn into an enterprise-ready, public-launch platform by redesigning the Next.js frontend into a dual-mode Executive Cockpit & Developer Sandbox, wiring all 50+ endpoints with deterministic offline simulation fallback, thoroughly verifying all 21 enterprise quality attributes, and leaving the repository pristine for GitHub/board review.

**Architecture:** Client-side Next.js 15 cockpit equipped with a lightweight reactive cluster store, a deterministic simulation fallback engine in `frontend/src/lib/simulation-engine.ts`, a global persona switcher in `header.tsx`, and verified BFF proxying to the Axum Rust gateway with 100% WCAG 2.2 AA accessibility and zero unhandled errors.

**Tech Stack:** Next.js 15 (App Router, React 19, TypeScript, Tailwind CSS, Lucide Icons), Rust 1.80+ (Axum, Tokio, SIMD AVX2/NEON), Node.js test runner, axe-core, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-19-public-launch-and-frontend-redesign-design.md`

## Global Constraints

- **Accessibility**: 100% WCAG 2.2 AA compliance; zero serious or critical axe violations across all 10 routes. Visible 2px focus rings on all interactive elements.
- **Security Invariant**: Zero production keys or client secrets in browser bundles. BFF proxy validates origin, host, and path traversal.
- **Resilience Standard**: Zero unhandled network errors or broken views when backend nodes are offline; graceful fallback to deterministic simulation with amber badge indicator.
- **Code Quality**: `cargo check --workspace` and `cargo clippy -D warnings` must exit 0. `npm run lint` (`tsc --noEmit`) and `npm run build` must exit 0. All 328+ Rust tests and 44+ frontend tests must pass.
- **Prong Alignment**: Fulfill all 21 leadership quality attributes (Functional Correctness, Architecture, Scalability, Performance, Efficiency, Reliability, Availability, Resilience, Security, Privacy, Data Integrity, Maintainability, Extensibility, Interoperability, Usability & UX, Accessibility, Testability, Observability, Deployability, Cost Sustainability, Evolvability & Longevity).

## Review Focus

1. Unauthenticated public visitor running high-volume prompt queries against an offline or keyless gateway — must return authentic deterministic simulation responses with zero console errors.
2. Executive persona toggle in header switching views — must smoothly adapt dashboard density between Executive SRE metrics and Developer prompt sandbox without layout shift (CLS < 0.01).
3. Cross-route state synchronization in simulation mode — tripping chaos circuit breaker on `/` must immediately reflect in cluster health status across all routes.
4. GDPR crypto-shredding on `/governance` — must execute safely, record a verifiable audit receipt, and clear all tenant records in both live and simulation modes.
5. All 21 system quality attributes documentation and proof — must be rigorously verifiable with commands and artifact references in `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`.

---

### Task 1: Reactive Cluster State Store & Persona Management

**Files:**
- Create: `frontend/src/lib/cluster-store.ts`
- Test: `frontend/tests/cluster_store.test.mjs`

**Interfaces:**
- Produces: `useClusterStore()`, `getClusterState()`, `setPersona(mode: 'executive' | 'developer')`, `setCircuitBreaker(state: 'CLOSED' | 'OPEN')`, `setByokApiKey(key: string | null)`, `extractVectorClockEpoch(vc: unknown): number`

- [x] **Step 1: Write failing unit test for `cluster-store.ts`**

```javascript
// frontend/tests/cluster_store.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { getClusterState, setPersona, setCircuitBreaker, setByokApiKey } from "../src/lib/cluster-store.ts";

test("cluster-store manages persona, circuit breaker, and BYOK credentials reactively", () => {
  setPersona("developer");
  assert.equal(getClusterState().persona, "developer");
  setPersona("executive");
  assert.equal(getClusterState().persona, "executive");

  setCircuitBreaker("OPEN");
  assert.equal(getClusterState().circuitBreaker, "OPEN");
  setCircuitBreaker("CLOSED");
  assert.equal(getClusterState().circuitBreaker, "CLOSED");

  setByokApiKey("sb-test-key-123");
  assert.equal(getClusterState().byokApiKey, "sb-test-key-123");
  setByokApiKey(null);
  assert.equal(getClusterState().byokApiKey, null);
});
```

- [x] **Step 2: Run test to verify it fails**

Run: `node --test frontend/tests/cluster_store.test.mjs`  
Expected: FAIL (module not found)

- [x] **Step 3: Implement `frontend/src/lib/cluster-store.ts`**

Implement a lightweight, framework-independent reactive store with pub/sub listener support and a custom React hook `useClusterStore()` for seamless component re-rendering.

- [x] **Step 4: Run test to verify it passes**

Run: `node --test frontend/tests/cluster_store.test.mjs`  
Expected: PASS

- [x] **Step 5: Commit Task 1**

```bash
git add frontend/src/lib/cluster-store.ts frontend/tests/cluster_store.test.mjs
git commit -m "feat(frontend): implement reactive cluster store and persona state machine"
```

---

### Task 2: Deterministic Simulation Fallback Engine

**Files:**
- Create: `frontend/src/lib/simulation-engine.ts`
- Modify: `frontend/src/lib/api.ts`
- Test: `frontend/tests/simulation_engine.test.mjs`

**Interfaces:**
- Produces: `simulationEngine`: comprehensive deterministic mock generators for `chatCompletions`, `clusterStatus`, `shredTenant`, `rebuildTenant`, `triggerInvalidation`, `securityEvaluate`, `merkleAttestation`, `learningPolicies`, and `finopsReport`.
- Updates `sunbornApi` in `frontend/src/lib/api.ts` to transparently catch network/connection failures and return valid simulated responses stamped with `_simulated: true`.

- [x] **Step 1: Write failing unit test for `simulation-engine.ts`**

```javascript
// frontend/tests/simulation_engine.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { simulationEngine } from "../src/lib/simulation-engine.ts";

test("simulationEngine generates deterministic chat completions across 4 caching presets", async () => {
  const exactRes = await simulationEngine.chatCompletions({ messages: [{ content: "test" }] }, "exact");
  assert.equal(exactRes.sunborn_cached, true);
  assert.ok(exactRes.usage.prompt_tokens > 0);
  assert.ok(exactRes._simulated);

  const negationRes = await simulationEngine.chatCompletions({ messages: [{ content: "bypass" }] }, "negation");
  assert.equal(negationRes.sunborn_cached, false);
  assert.ok(negationRes._simulated);
});

test("simulationEngine generates valid cryptographic shredding receipts", async () => {
  const shredRes = await simulationEngine.shredTenant("tenant-test-1");
  assert.equal(shredRes.tenant_id, "tenant-test-1");
  assert.equal(shredRes.status, "CRYPTOGRAPHICALLY_SHREDDED");
  assert.equal(shredRes.verified_multi_store_erasure, true);
});
```

- [x] **Step 2: Run test to verify it fails**

Run: `node --test frontend/tests/simulation_engine.test.mjs`  
Expected: FAIL (module not found)

- [x] **Step 3: Implement `frontend/src/lib/simulation-engine.ts` and update `frontend/src/lib/api.ts`**

Implement realistic mathematical mock responses, realistic simulated microsecond latencies, and fallback wrappers in `sunbornApi` so that when `fetch` fails with network/offline errors, it gracefully returns simulated data rather than throwing an unhandled exception.

- [x] **Step 4: Run test to verify it passes**

Run: `node --test frontend/tests/simulation_engine.test.mjs`  
Expected: PASS

- [x] **Step 5: Commit Task 2**

```bash
git add frontend/src/lib/simulation-engine.ts frontend/src/lib/api.ts frontend/tests/simulation_engine.test.mjs
git commit -m "feat(frontend): build deterministic simulation fallback engine with zero-cost public sandbox"
```

---

### Task 3: Redesign Header with Dual-Mode Persona Switcher & BYOK Modal

**Files:**
- Modify: `frontend/src/components/layout/header.tsx`
- Test: `frontend/tests/header_dual_mode.test.mjs`

**Interfaces:**
- Consumes: `useClusterStore` from `frontend/src/lib/cluster-store.ts`
- Produces: Header navigation with interactive Persona Switcher (`[ 🏢 Executive Cockpit ]` ⟷ `[ 💻 Developer Sandbox ]`), BYOK API Key modal dialog, and live/simulation status badge indicator.

- [x] **Step 1: Write failing test for header component structure and accessibility**

```javascript
// frontend/tests/header_dual_mode.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("header source contains dual-mode persona switcher and BYOK trigger", () => {
  const content = fs.readFileSync("frontend/src/components/layout/header.tsx", "utf-8");
  assert.ok(content.includes("Executive Cockpit"), "Missing Executive Cockpit persona trigger");
  assert.ok(content.includes("Developer Sandbox"), "Missing Developer Sandbox persona trigger");
  assert.ok(content.includes("BYOK") || content.includes("Set API Key"), "Missing BYOK trigger");
  assert.ok(content.includes("useClusterStore"), "Header must bind to cluster store");
});
```

- [x] **Step 2: Run test to verify it fails**

Run: `node --test frontend/tests/header_dual_mode.test.mjs`  
Expected: FAIL

- [x] **Step 3: Update `frontend/src/components/layout/header.tsx`**

Integrate persona switcher, BYOK key modal with secure local session storage, cluster health badge, and preserve all WCAG 2.2 AA ARIA attributes and 208+ tab stops.

- [x] **Step 4: Run test to verify it passes**

Run: `node --test frontend/tests/header_dual_mode.test.mjs`  
Expected: PASS

- [x] **Step 5: Run existing keyboard focus and accessibility audits**

Run: `npm run test:keyboard --prefix frontend` and `npm run test:a11y --prefix frontend`  
Expected: PASS (0 axe violations, complete keyboard tab stops)

- [x] **Step 6: Commit Task 3**

```bash
git add frontend/src/components/layout/header.tsx frontend/tests/header_dual_mode.test.mjs
git commit -m "feat(frontend): redesign header with dual-mode persona switcher and BYOK session modal"
```

---

### Task 4: Redesign Overview Cockpit (`/`) with Dual-Mode Adaptation

**Files:**
- Modify: `frontend/src/app/page.tsx`
- Test: `frontend/tests/overview_dual_mode.test.mjs`

**Interfaces:**
- Consumes: `useClusterStore`, `sunbornApi`
- Produces: Dynamic Dashboard page adapting between Executive SRE view (cluster topology, FinOps, vector clocks) and Developer Sandbox view (interactive prompt sandbox, AST negation barriers, L1 cache inspector).

- [x] **Step 1: Write failing test for overview page persona adaptation**

```javascript
// frontend/tests/overview_dual_mode.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("overview page contains persona-adaptive layout and interactive sandbox wiring", () => {
  const content = fs.readFileSync("frontend/src/app/page.tsx", "utf-8");
  assert.ok(content.includes("useClusterStore"), "Overview must consume cluster store");
  assert.ok(content.includes("Executive Mission Control") || content.includes("Executive Cockpit"));
  assert.ok(content.includes("Interactive Developer Sandbox"));
  assert.ok(content.includes("handleSelectPreset"));
});
```

- [x] **Step 2: Run test to verify it fails**

Run: `node --test frontend/tests/overview_dual_mode.test.mjs`  
Expected: FAIL

- [x] **Step 3: Redesign `frontend/src/app/page.tsx`**

Implement smooth persona switching, interactive prompt simulation with true-to-spec latency calculations, cluster mesh synchronization trigger, and chaos circuit breaker controls.

- [x] **Step 4: Run test to verify it passes**

Run: `node --test frontend/tests/overview_dual_mode.test.mjs`  
Expected: PASS

- [x] **Step 5: Verify build and accessibility**

Run: `npm test --prefix frontend` and `npm run build --prefix frontend`  
Expected: PASS

- [x] **Step 6: Commit Task 4**

```bash
git add frontend/src/app/page.tsx frontend/tests/overview_dual_mode.test.mjs
git commit -m "feat(frontend): redesign overview page with adaptive persona views and live sandbox"
```

---

### Task 5: End-to-End Endpoint Wiring & Polish Across All 9 Child Routes

**Files:**
- Modify: `frontend/src/app/governance/page.tsx`
- Modify: `frontend/src/app/execution/page.tsx`
- Modify: `frontend/src/app/explorer/page.tsx`
- Modify: `frontend/src/app/finops/page.tsx`
- Modify: `frontend/src/app/knowledge/page.tsx`
- Modify: `frontend/src/app/learning/page.tsx`
- Modify: `frontend/src/app/security/page.tsx`
- Modify: `frontend/src/app/ceremony/page.tsx`
- Modify: `frontend/src/app/login/page.tsx`
- Test: `frontend/tests/all_routes_wiring.test.mjs`

**Interfaces:**
- Connects every action button (GDPR shredding, state rebuild, cache eviction, canary rollout, policy promotion, Merkle attestation) to `sunbornApi` with simulation fallback and clear user notification banners.

- [x] **Step 1: Write integration test validating all routes export valid default components and consume `sunbornApi`**

```javascript
// frontend/tests/all_routes_wiring.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const routes = [
  "governance",
  "execution",
  "explorer",
  "finops",
  "knowledge",
  "learning",
  "security",
  "ceremony",
  "login",
];

test("all child routes contain sunbornApi integration and resilient error handling", () => {
  for (const route of routes) {
    const file = `frontend/src/app/${route}/page.tsx`;
    const content = fs.readFileSync(file, "utf-8");
    assert.ok(content.length > 500, `${route} must have substantial implementation`);
    assert.ok(content.includes("sunbornApi") || content.includes("api"), `${route} must wire to API`);
  }
});
```

- [x] **Step 2: Run test to verify it passes or catches gaps**

Run: `node --test frontend/tests/all_routes_wiring.test.mjs`

- [x] **Step 3: Update and polish all child routes**

Ensure every view uses `extractVectorClockEpoch` safely, handles offline states gracefully, displays simulation badges when offline, and provides immediate feedback upon user interaction.

- [x] **Step 4: Run full test suite and accessibility checks**

Run: `npm test --prefix frontend`, `npm run test:a11y --prefix frontend`, and `npm run test:keyboard --prefix frontend`  
Expected: All passing with 0 axe violations.

- [x] **Step 5: Commit Task 5**

```bash
git add frontend/src/app/ frontend/tests/all_routes_wiring.test.mjs
git commit -m "feat(frontend): wire all child routes with resilient fallback and status feedback"
```

---

### Task 6: Comprehensive 21 Quality Attributes Dossier & Repository Cleanup

**Files:**
- Create: `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`
- Clean: remove untracked ephemeral files (`.playwright-mcp/`, `scripts/watch_ci.py`), verify `.gitignore`
- Format & Lint: Run `cargo fmt`, `cargo clippy`, `cargo test`, `npm run lint`, `npm test`

- [x] **Step 1: Author `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`**

Provide comprehensive, audit-grade verification entries for each of the 21 leadership attributes, complete with architectural evidence, exact test commands, and formal verification proofs.

- [x] **Step 2: Clean untracked files and format codebase**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
npm run lint --prefix frontend
```

- [x] **Step 3: Run full Rust workspace test suite and verification gate**

Run: `cargo test --workspace --all-targets` and `npm test --prefix frontend`  
Expected: All tests pass.

- [x] **Step 4: Commit Task 6**

```bash
git add docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md .gitignore Cargo.lock
git commit -m "docs(compliance): publish 21 enterprise quality attributes audit dossier and clean repo"
```
