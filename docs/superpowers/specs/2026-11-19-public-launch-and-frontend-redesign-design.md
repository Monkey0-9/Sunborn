# Design Document: Sunborn Public Launch, Enterprise Redesign, and Full Systems Hardening

**Document Identifier:** `2026-11-19-public-launch-and-frontend-redesign-design`  
**Date:** 2026-11-19  
**Status:** Approved by Leadership / Proceeding to Plan  
**Target Systems:** Sunborn Rust Gateway (`crates/sunborn-gateway`, `crates/sunborn-core`, `crates/sunborn-semantic`), Next.js 15 Frontend Cockpit (`frontend/`), TypeScript & Python SDKs, Infrastructure & Verification Pipelines.

---

## 1. Intent and the Why

Sunborn is an ultra-low-latency distributed semantic execution gateway deployed between client applications and frontier LLM inference providers (OpenAI, Anthropic, Gemini). Its primary mission is to eliminate redundant GPU computation and latency by caching safe completions, while mathematically refusing reuse whenever safety cannot be strictly proven.

To launch publicly to millions of developers and present convincingly before board members and Enterprise Architecture Review Boards (EARB), Sunborn must project unflinching engineering maturity. There can be zero broken routes, zero unhandled network errors when background services are warming up, zero layout jank, and complete fidelity across 21 enterprise quality attributes.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Dual-Mode Experience with Global Header Toggle (Decision 2A)**:
  - **Executive Cockpit Mode**: High-density view tailored for Board Members, CTOs, CISOs, and SREs showcasing live multi-region cluster health, vector clock synchronization, FinOps token savings, cryptographic governance (GDPR crypto-shredding, Merkle proofs), and system kill-switches.
  - **Developer Sandbox Mode**: Interactive, developer-centric studio allowing external engineers to immediately test prompt canonicalization, sub-microsecond L1 exact hits, conformal semantic retrieval, AST negation barriers, and multi-provider wire format translation.
- **Fail-Safe Public Execution & Simulation (Decision 1A)**:
  - Unauthenticated public visitors execute against an authentic, deterministic **Client-Side Simulation Engine** with zero hosting spend, zero DDoS risk, and instant mathematical evaluation.
  - Optional BYOK (Bring-Your-Own-Key) modal allows developers with an API key to switch effortlessly into live cluster execution mode.
  - Clear visual indicator badges distinguish live cluster execution from deterministic simulation.
- **21 Enterprise Quality Attributes**: Systematically verify and document every one of the 21 architectural dimensions specified by leadership.
- **Zero-Breakage Verification**: Maintain passing status across all 328+ Rust tests, 44+ frontend unit tests, 100% WCAG 2.2 AA conformance (0 axe violations), 100% keyboard focus order, and OpenAPI 3.1 schema drift checks.

### Non-Goals
- Replacing the core Rust AST parser or rewriting SIMD AVX2/NEON vector kernels.
- Adding arbitrary unrelated SaaS features (e.g., credit card payment gateway integrations).
- Creating unneeded external cloud dependencies for basic local evaluation.

### Anti-Goals
- "Mock-only" facades that claim live verification when they are purely static text.
- Over-engineered abstractions that violate the Ponytail lazy senior developer standard: no unnecessary dependencies, no boilerplate wrappers, no bloated UI kits.
- Silent error swallows: if a user issues an explicit mutation or administrative action that fails, give clear, actionable diagnostic feedback.

---

## 3. Constraints

- **Platform & Environment**: Windows / Git Bash / PowerShell host with Rust 1.80+, Node.js 22+, Python 3.11+, and Next.js 15 App Router with React 19.
- **Accessibility**: Zero critical or serious axe violations across all routes (WCAG 2.2 AA). Tab order and visible 2px focus rings on every interactive element.
- **Security Invariant**: Zero production keys or admin secrets exposed in client-side bundles (`sunbornApi` and BFF proxy enforce origin pinning and cookie/header isolation).
- **Latency Budgets**:
  - L1 exact memory cache: $<1.0\,\mu\text{s}$ P50.
  - Conformal semantic vector retrieval: $<15\,\text{ms}$ P50.
  - Next.js First Load JS: $<140\,\text{kB}$ shared.

---

## 4. Approaches Considered

### Approach 1: Dual-Mode Enterprise Cockpit & Interactive Developer Sandbox (Selected)
- **Architecture**: A cohesive, high-density Next.js cockpit featuring an Executive Mission Control mode (for board members and platform architects) and an Interactive Developer Sandbox (for public developers) with a global header persona toggle. Backed by typed BFF proxying with automatic graceful fallback to deterministic simulated telemetry when backend nodes are cold.
- **Pros**: Delivers immediate hands-on value to public developers while giving board members the governance, compliance, and financial proof they require. Zero risk of embarrassing broken states.
- **Cons**: Requires careful coordination between live API streams and offline fallback schemas.

### Approach 2: Deep SRE Observability Cockpit
- **Architecture**: Heavy focus on distributed telemetry, Prometheus scrapers, outbox replay, and chaos engineering controls.
- **Pros**: Exceptional depth for SREs and systems engineers.
- **Cons**: High barrier to entry for public developers who want to evaluate semantic caching and cost savings quickly.

### Approach 3: Minimalist Developer Portal & API Playground
- **Architecture**: Simple documentation site with embedded Swagger/OpenAPI tester.
- **Pros**: Low development surface.
- **Cons**: Lacks the authoritative, production-grade visual polish required to impress board members and establish enterprise market leadership.

**Decision**: Approach 1 was selected for its balance of executive authority and public developer adoption.

---

## 5. Architectural Alignment: The 21 Quality Attributes

| # | Attribute | Implementation in Sunborn Platform & Frontend |
|---|---|---|
| 1 | **Functional Correctness** | Formal TLA+ specifications for epoch fencing and transactional outbox. 328+ Rust tests and 44+ frontend tests validating strict API contract behavior. |
| 2 | **Architecture** | Hexagonal architecture with decoupled crates (`sunborn-core`, `sunborn-crypto`, `sunborn-semantic`, `sunborn-gateway`) and Next.js BFF proxy isolation. |
| 3 | **Scalability** | Distributed Redis/Dragonfly cluster mesh with vector clock anti-entropy and multi-worker concurrent soak testing (10,000 reqs @ 16 workers). |
| 4 | **Performance** | Sub-microsecond L1 cache (1.64M req/s P50), C SIMD vector kernels (AVX2/NEON), singleflight upstream request deduplication. |
| 5 | **Efficiency** | Token savings calculation engine, GPU compute reduction, minimal memory footprint via W-TinyLFU eviction. |
| 6 | **Reliability** | Fail-closed default in production (`SUNBORN_ENV=production`), transactional outbox pattern preventing orphan writes. |
| 7 | **Availability** | Multi-replica health/readiness endpoints (`/health`, `/ready`), dynamic cluster membership, zero-downtime reconfiguration. |
| 8 | **Resilience** | Circuit breaker endpoints (`/v1/chaos/trip`, `/v1/chaos/reset`), 5 operational kill-switches, and exponential backoff retry policies. |
| 9 | **Security** | Constant-time API key verification, strict deny-by-default auth middleware, CSRF origin pinning, AST negation injection defense. |
| 10 | **Privacy** | GDPR cryptographic tenant shredding (`/v1/governance/shred`), per-tenant encryption key derivation, zero-leak telemetry. |
| 11 | **Data Integrity** | Merkle DAG proofs, causal dependency graphs, durable PostgreSQL state reconciliation (`/v1/rebuild`). |
| 12 | **Maintainability** | Strict Clippy lints (`-D warnings`), Rustfmt, full TypeScript strict mode, zero lint errors, single source of truth (`docs/STATUS.md`). |
| 13 | **Extensibility** | Pluggable storage providers (memory, Redis, PostgreSQL, Qdrant) and modular AST inspection rules. |
| 14 | **Interoperability** | Native drop-in wire compatibility with OpenAI (`/v1/chat/completions`), Anthropic (`/v1/messages`), and Gemini formats; CycloneDX SBOM. |
| 15 | **Usability & UX** | Cybernetic glassmorphism design system, unified Command Palette (`Cmd+K`), real-time interactive DAG canvas, responsive navigation. |
| 16 | **Accessibility** | 100% WCAG 2.2 AA compliance, zero axe violations, full keyboard focus rings, screen reader announcements. |
| 17 | **Testability & Verification** | End-to-end multi-gate verification script (`scripts/verify.sh`), hold-out scientific benchmark datasets, automated a11y tests. |
| 18 | **Observability** | Prometheus `/metrics` exposition, OpenTelemetry distributed tracing headers, structured JSON logging, autonomic anomaly detection. |
| 19 | **Deployability** | Production multi-stage Dockerfiles, Docker Compose stacks, Kubernetes Helm charts, documented environment configurations. |
| 20 | **Cost Sustainability** | FinOps dashboard tracking avoided GPU expenditure, token quota management, and transactional advisory locks for spend ceilings. |
| 21 | **Evolvability & Longevity** | Explicit `/v1/` API versioning, idempotent database migrations, documented RFCs, and governance models. |

---

## 6. Frontend Redesign & Wiring Specification

### 6.1 Unified Cybernetic Infrastructure Design System
- **Theme & Palette**: Dark slate background (`#020617`, `#0f172a`), deep indigo/violet primary accents (`#6366f1`, `#8b5cf6`), emerald status indicators (`#10b981`), amber warnings (`#f59e0b`), and rose security alerts (`#f43f5e`).
- **Typography**: Clean monospace font for hashes, metrics, and vector epochs; crisp sans-serif (`Inter` or system-ui) for labels and analytical prose.
- **Motion & Polish**: Subtle GPU-accelerated shimmer on live status badges, smooth tab transitions, zero layout shifts (CLS < 0.01).
- **Header Global Persona Switcher**:
  - `[ 🏢 Executive Cockpit ]` ⟷ `[ 💻 Developer Sandbox ]`
  - In Executive Cockpit: Prioritizes cluster mesh status, vector clocks, multi-tenant quotas, compliance audit logs, and FinOps ledger.
  - In Developer Sandbox: Prioritizes interactive prompt testing, DAG visualization, AST guardrails, and token savings breakdown.

### 6.2 View Specifications & Endpoint Wiring

1. **Overview & Cockpit (`/`)**:
   - Live cluster throughput, P50/P99 latency, hit ratio, and avoided cost tickers.
   - Interactive Prompt Simulator with 4 presets: Exact Match, Conformal Semantic, Negation Guardrail, and Agent DAG Subgraph.
   - Live Cluster Multi-Region Mesh with vector clock status and anti-entropy sync trigger.
   - Chaos Injection & Circuit Breaker toggles.

2. **Execution DAG & Merkle Proofs (`/execution`)**:
   - Visual execution graph rendering tool nodes, cache checkpoints, and Merkle leaf hashes.
   - Interactive proof verification against gateway public keys.

3. **Semantic Cache Explorer (`/explorer`)**:
   - Query lookup with cosine similarity thresholding, token avoidance breakdown, and L1/L2 cache inspection.
   - Cache key invalidation and purge controls.

4. **FinOps & Cost Arbitrage (`/finops`)**:
   - Model cost comparison (GPT-4o vs Claude 3.5 Sonnet vs Gemini 1.5 Pro).
   - Real-time spend ceiling controls and tenant token quota sliders wired to `/v1/governance/spend-ceiling`.

5. **Cluster Governance & Cryptographic Shredding (`/governance`)**:
   - Multi-tenant tenant lifecycle management, vector clock epoch visualization (`extractVectorClockEpoch`), and one-click GDPR crypto-shredding (`/v1/governance/shred`) with instant audit receipt.
   - PostgreSQL durable state rehydration trigger (`/v1/rebuild`).

6. **Knowledge Graph & Causal Invalidation (`/knowledge`)**:
   - Document dependency tree visualizer showing transitive invalidation cascades.
   - Invalidation trigger wired to `/v1/invalidation`.

7. **Conformal Learning & Calibration (`/learning`)**:
   - Conformal risk abstention scores, policy canary rollout sliders (`/v1/learning/canary`), policy promotion, and one-click instant rollback.

8. **Security & AST Guardrails (`/security`)**:
   - Adversarial prompt injection defense tester, structural AST negation detector, and real-time security event audit log.

9. **Key Ceremony & Cryptographic Attestation (`/ceremony`)**:
   - Ed25519 public key distribution, hardware security module (HSM) attestation proofs, and offline verification instructions.

10. **Authentication & Session (`/login`)**:
    - Secure session handling via HttpOnly cookies and explicit bearer tokens, BYOK key entry modal, strict CSRF validation, zero client secrets.

### 6.3 Resilient Offline/Cold-Start Simulation Strategy
- Every interactive component utilizes `sunbornApi`.
- If `sunbornApi` encounters a network failure or 503 (e.g., gateway running standalone without Redis or external OpenAI keys), the API client safely activates **Deterministic Simulation Fallback**:
  - The UI displays an amber badge: `Simulation Mode · Gateway Offline or Local Standalone`.
  - All interactive controls (prompts, shredding, policy rollouts, chaos injection) return mathematically valid, authentic mock responses with true-to-spec latency calculations.
  - Shared in-memory cluster state allows cross-route coordination (e.g. tripping circuit breaker on `/` updates the health status across all routes).
  - The user never experiences an ugly red error crash, blank page, or unhandled rejection.

---

## 7. What is Left to the Builder

- Internal component file structuring and refactoring in `frontend/src/` ensuring all clean code standards and accessibility constraints are preserved.
- Fine-tuning CSS animations to ensure zero CPU throttling on low-spec client machines.
- Unifying common mock generators in a shared `frontend/src/lib/simulation-engine.ts` module to eliminate duplicated fixture code across pages.
- Adding comprehensive end-to-end integration tests verifying both live gateway and simulation fallback workflows.
