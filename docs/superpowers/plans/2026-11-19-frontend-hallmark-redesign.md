# Frontend Hallmark Redesign & Enterprise Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Completely redesign and modularize the Sunborn frontend using Hallmark v2 and senior engineering UI/UX standards, wiring live gateway telemetry and resilient demo states, while keeping all 21 production pillars and 44 test suites 100% green.

**Architecture:** Decompose the 1,693-line overview page into modular, accessible cockpit components backed by `:root` design tokens in `tokens.css`. Retain the hardened server-side BFF proxy (`proxy-policy.ts`) and unify telemetry across all 10 console routes with seamless live/fallback synchronization.

**Tech Stack:** Next.js 15 (App Router), TypeScript, Tailwind CSS, Hallmark Design Tokens, Lucide Icons, Playwright Chromium, axe-core WCAG 2.2 AA.

**Spec:** [docs/ENTERPRISE_ARCHITECTURE_DOSSIER.md](file:///C:/Sunborn/docs/ENTERPRISE_ARCHITECTURE_DOSSIER.md) and [docs/STATUS.md](file:///C:/Sunborn/docs/STATUS.md)

## Global Constraints

- **Hallmark Floor Rule 1**: All color and typography values must resolve through declared tokens at `:root` or designated CSS theme blocks; no hardcoded arbitrary color magic numbers in component bodies.
- **Hallmark Floor Rule 2 & 6**: Never use `transition: all`. Every interactive element must display an instant `:focus-visible` indicator (minimum 2px).
- **Hallmark Floor Rule 8 & 11**: Zero invented metrics, zero purple-to-blue gradients, and no cards nested inside cards.
- **Accessibility & Security**: Must maintain 0 serious/critical axe violations (WCAG 2.2 Level AA) and preserve 100% route authorization and BFF origin-pinning security.

## Review Focus

1. **Next.js SSR Hydration Safety**: Ensure no browser-only APIs (`window`, `localStorage`) or unescaped object interpolations execute during server rendering.
2. **BFF Origin & CSRF Defense**: Verify that proxy route mutations continue to reject cross-origin requests and preserve server-side origin pinning.
3. **Keyboard Focus Tab Order**: Ensure that all redesigned interactive controls (buttons, presets, inputs) maintain logical, sequential tab stops with visible 2px focus indicators.
4. **Vector Clock & Epoch Telemetry**: Verify that multi-node cluster vector clocks (`{ clocks: { node: epoch } }`) parse into scalar numeric values without React child errors.
5. **Gateway Disconnection Grace**: When the local Rust gateway (`:8080`) is not running, all views must gracefully transition to rich offline simulated telemetry rather than unhandled fetch exceptions.

---

### Task 1: Hallmark Foundation & Design Tokens Setup

**Files:**
- Create: `frontend/src/styles/tokens.css`
- Modify: `frontend/src/app/globals.css`
- Modify: `frontend/tailwind.config.ts`

**Interfaces:**
- Consumes: Tailwind CSS base layer
- Produces: CSS custom properties (`--color-bg-base`, `--color-panel`, `--color-accent-cyan`, `--color-text-primary`, `--font-sans`, `--font-mono`)

- [x] **Step 1: Create `frontend/src/styles/tokens.css` with Hallmark tokens**
Declare `:root` tokens conforming to Hallmark Rule 1: obsidian background (`#090d16`), dark slate panel (`#111827`), high-contrast borders (`#1e293b`), vibrant technical cyan (`#06b6d4`), and high-contrast text (`#f8fafc`).

- [x] **Step 2: Import `tokens.css` into `frontend/src/app/globals.css`**
Add `@import "../styles/tokens.css";` at the top of `globals.css` immediately after directives. Enforce `overflow-x: clip` on `html, body`.

- [x] **Step 3: Verify Tailwind configuration and build**
Run: `npm run build --prefix frontend`  
Expected: Next.js production build exits 0 with 0 errors.

- [x] **Step 4: Commit**
```bash
git add frontend/src/styles/tokens.css frontend/src/app/globals.css frontend/tailwind.config.ts
git commit -m "feat(frontend): introduce Hallmark design tokens and root theme variables"
```

---

### Task 2: Modular Cockpit Components Decomposition

**Files:**
- Create: `frontend/src/components/cockpit/metrics-ribbon.tsx`
- Create: `frontend/src/components/cockpit/prompt-probe-playground.tsx`
- Create: `frontend/src/components/cockpit/live-dag-visualizer.tsx`
- Create: `frontend/src/components/cockpit/cluster-mesh-status.tsx`
- Test: `frontend/tests/cockpit_components.test.mjs`

**Interfaces:**
- Consumes: `sunbornApi` from `@/lib/api`, formatting utilities from `@/lib/utils`
- Produces: Clean, reusable, self-contained UI components with strict TypeScript types

- [x] **Step 1: Implement `metrics-ribbon.tsx`**
Executive summary strip displaying 4 core live metrics:
  - Cache Hit Ratio (%)
  - L1 Latency P50 (µs)
  - Cumulative Avoided Spend ($ USD)
  - Active Cluster Mesh Replicas

- [x] **Step 2: Implement `prompt-probe-playground.tsx`**
Interactive query runner supporting 4 enterprise test presets:
  - Exact Match (Sub-microsecond L1 lookup)
  - Semantic Retrieval (Qdrant HNSW conformal gate)
  - Adversarial Negation (AST guardrail polarity rejection)
  - Multi-Step Agent Tool DAG
Provides real-time latency, avoided token count, and confidence score displays.

- [x] **Step 3: Implement `live-dag-visualizer.tsx`**
Clean, accessible SVG-based execution DAG visualizer rendering node dependencies, execution latencies, and Merkle root attestations without external canvas bloat.

- [x] **Step 4: Implement `cluster-mesh-status.tsx`**
Multi-region topology status component showing node states, vector clock epochs, and consensus health.

- [x] **Step 5: Write component verification test `frontend/tests/cockpit_components.test.mjs`**
Verify that all 4 cockpit components exist and export default or named functions with valid TypeScript declarations.

- [x] **Step 6: Run component tests**
Run: `node --test frontend/tests/cockpit_components.test.mjs`  
Expected: PASS with 0 errors.

- [x] **Step 7: Commit**
```bash
git add frontend/src/components/cockpit/ frontend/tests/cockpit_components.test.mjs
git commit -m "feat(frontend): decompose cockpit into modular accessible visual components"
```

---

### Task 3: Redesign Overview Page (`page.tsx`)

**Files:**
- Modify: `frontend/src/app/page.tsx`
- Test: `frontend/tests/e2e_browser_workflows.test.mjs`

**Interfaces:**
- Consumes: Cockpit components from `@/components/cockpit/*`, `Header` from `@/components/layout/header`
- Produces: Streamlined, accessible, high-performance home dashboard (< 400 lines)

- [x] **Step 1: Re-architect `frontend/src/app/page.tsx`**
Assemble the modular cockpit components into an executive layout:
  - Hallmark-compliant hero: concise headline (< 7 words), single-sentence value prop, two primary actions ("Probe Semantic Cache", "View DAG Execution").
  - Metrics Ribbon pinned above the fold.
  - Interactive Probe Playground & Execution DAG side-by-side.
  - Live activity stream feed with instant feedback.
  - Resilient error boundary handling offline gateway fallbacks seamlessly.

- [x] **Step 2: Run build and E2E browser tests**
Run: `npm run build --prefix frontend && npm test --prefix frontend`  
Expected: All 44 tests pass.

- [x] **Step 3: Commit**
```bash
git add frontend/src/app/page.tsx
git commit -m "feat(frontend): overhaul overview page with Hallmark executive cockpit"
```

---

### Task 4: Polish & Align Secondary Console Routes

**Files:**
- Modify: `frontend/src/app/explorer/page.tsx`
- Modify: `frontend/src/app/execution/page.tsx`
- Modify: `frontend/src/app/security/page.tsx`
- Modify: `frontend/src/app/finops/page.tsx`
- Modify: `frontend/src/app/governance/page.tsx`
- Modify: `frontend/src/app/knowledge/page.tsx`
- Modify: `frontend/src/app/learning/page.tsx`
- Modify: `frontend/src/app/ceremony/page.tsx`

**Interfaces:**
- Consumes: Hallmark design tokens, shared card and button primitives
- Produces: Cohesive styling, accessible focus rings, and explicit form associations across all sub-consoles

- [x] **Step 1: Update card containers and typography in secondary views**
Ensure all views reference Hallmark tokens (`--color-panel`, `--color-border`), remove nested card containers, and apply consistent heading scales.

- [x] **Step 2: Enforce focus visibility and ARIA attributes**
Verify every form input, select, and button has explicit `aria-label` or `<label>` and 2px `:focus-visible` ring.

- [x] **Step 3: Run WCAG 2.2 AA axe sweep across all 10 routes**
Run: `npm run test:a11y --prefix frontend`  
Expected: 10/10 routes checked, 0 violations.

- [x] **Step 4: Run Keyboard navigation audit**
Run: `npm run test:keyboard --prefix frontend`  
Expected: 200+ tab stops verified with visible indicators and zero traps.

- [x] **Step 5: Commit**
```bash
git add frontend/src/app/
git commit -m "feat(frontend): polish secondary views to meet Hallmark and WCAG 2.2 standards"
```

---

### Task 5: End-to-End System Verification & Release Attestation

**Files:**
- Test: All workspace test suites and verification scripts
- Modify: `docs/STATUS.md`

- [x] **Step 1: Run comprehensive backend verification**
Run: `cargo check --workspace --all-targets && cargo clippy --workspace --all-targets --all-features -- -D warnings`  
Expected: Clean exit 0 with 0 warnings.

- [x] **Step 2: Run full Rust test suite**
Run: `cargo test --workspace --all-targets`  
Expected: 328+ tests pass, 0 fail.

- [x] **Step 3: Run public launch readiness verification**
Run: `python scripts/verify_public_launch.py`  
Expected: 7/7 audit gates pass.

- [x] **Step 4: Update `docs/STATUS.md` with verified pass status**
Record the completed frontend overhaul, test outcomes, and readiness verdict.

- [x] **Step 5: Final commit**
```bash
git add docs/STATUS.md
git commit -m "docs: certify frontend Hallmark redesign and enterprise public launch readiness"
```
