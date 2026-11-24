# Sunborn Admin Cockpit (`frontend/`)

Modern, high-performance administrative cockpit, live observability dashboard, and interactive developer sandbox for Sunborn.

## Core Capabilities

- **Dual-Mode Persona Switching**: Instantly toggle between **Executive Cockpit** (dense cluster metrics, cross-region vector clocks, GDPR cryptographic shredding, and FinOps spend ledgers) and **Developer Sandbox** (interactive multi-tier cache probe, AST negation guardrail tester, and sub-microsecond latency inspector).
- **10 Core Operating Views**:
  - `/` — Operations Dashboard, Real-Time 3D Manifold Canvas, Live Throughput Ribbon
  - `/explorer` — Semantic Cache Explorer & Cosine Distance Inspector
  - `/execution` — Agent Subgraph DAG & Merkle Proof Attestation
  - `/finops` — Token Economics, GPU Cost Arbitrage & Transactional Spend Ceilings
  - `/governance` — Multi-Tenant Isolation, Monotonic Vector Clocks & GDPR/DPDP Crypto-Shredder
  - `/knowledge` — Causal Knowledge Graph & Blast-Radius Invalidation Cascades
  - `/learning` — Conformal Risk Abstention, Calibration & Canary Policy Promotion
  - `/security` — Adversarial Prompt Injection Defense & Structural AST Negation Guardrails
  - `/ceremony` — Ed25519 Public Key Ceremony & Hardware Security Module Attestation
  - `/login` — Secure Session Management & BYOK Session Credentials
- **Backend-for-Frontend (BFF) Security**: The Next.js server acts as a secure reverse proxy (`/api/proxy/[...path]`) enforcing CSRF origin pinning, session cookie verification, and credential stripping. Zero raw database credentials or master keys are ever exposed to the client.
- **Deterministic Simulation Fallback**: When disconnected from a live gateway, the cockpit seamlessly transitions to an in-browser mathematical simulation engine (`simulation-engine.ts`) with a persistent status notification banner.
- **Accessibility & Compliance**: 100% WCAG 2.2 AA conformant (0 axe violations across all 10 routes, 271 verified tab stops, visible 2px focus rings, skip-to-content links).

## Development & Testing

```bash
# Install dependencies
npm install --prefix frontend

# Run local development server (http://localhost:3000)
npm run dev --prefix frontend

# Run full test suite (67 unit, BFF security, and E2E browser tests)
npm test --prefix frontend

# Run accessibility audits
npm run test:a11y --prefix frontend      # axe-core WCAG 2.2 AA sweep
npm run test:keyboard --prefix frontend  # keyboard focus and tab stop verification

# Build production bundle
npm run build --prefix frontend
```
