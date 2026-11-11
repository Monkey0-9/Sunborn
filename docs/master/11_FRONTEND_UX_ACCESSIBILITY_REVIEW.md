# Sunborn Frontend Architecture, BFF Security & Accessibility Specification

**Document Version:** 2.0.0  
**Effective Commit:** `7f1f161`  
**Classification:** Frontend Architecture & UI/UX Standards

---

## 1. Next.js 15 App Router Architecture & BFF Reverse Proxy

The Sunborn dashboard is built on **Next.js 15.5.29** utilizing the modern App Router paradigm:

```
frontend/src/
├── app/
│   ├── layout.tsx         # Root layout: semantic HTML5 landmarks, viewport, high-contrast tokens
│   ├── page.tsx           # Primary Operations Control Plane & Cluster Capacity Modeler
│   ├── ceremony/          # Cryptographic proof attestation & simulation verifier
│   ├── execution/         # Agent execution DAG planner & Merkle attestation viewer
│   ├── explorer/          # Multi-tier cache inspection & key lifecycle debugger
│   ├── finops/            # FinOps token accounting, cost savings, and rate ceiling reports
│   ├── governance/        # GDPR Article 17 cryptographic shredding & dual-admin quorum
│   ├── knowledge/         # Interactive knowledge dependency graph & invalidation cascade
│   ├── learning/          # Autonomic policy controller & shadow evaluation monitor
│   ├── login/             # Fail-closed authentication & credential entry
│   ├── security/          # Inline PII redaction vault & adversarial firewall probe
│   └── api/
│       ├── auth/          # Server-side cookie auth handler (fail-closed in production)
│       ├── health/        # Dashboard liveness probe
│       └── proxy/[...path]# BFF reverse proxy with strict deny-by-default RBAC
```

---

## 2. BFF Security Invariants & Attack Mitigations

The Backend-For-Frontend (BFF) proxy ([`frontend/src/app/api/proxy/[...path]/route.ts`](file:///C:/Sunborn/frontend/src/app/api/proxy/[...path]/route.ts)) protects internal gateway RPCs from untrusted browser access:

1. **Deny-by-Default on 12 Privileged Routes**:
   - Routes managing destructive governance (`/governance/tenants/{id}/shred`), credential creation, cluster sync, or circuit breakers strictly require authenticated administrator sessions. Anonymous calls are rejected with `401 Unauthorized`.
2. **CSRF Origin Verification**:
   - All state-mutating requests (`POST`, `PUT`, `DELETE`, `PATCH`) validate the browser `Origin` and `Host` headers. Mismatches are rejected with `403 Forbidden`.
3. **Payload Clamping & Timeouts**:
   - Ingress request bodies are capped at 10 MB to prevent memory exhaustion.
   - Upstream gateway dispatches enforce a 60-second timeout.
4. **Production Fail-Closed Guarantee**:
   - Whenever `NODE_ENV === 'production'` or `STRICT_GATEWAY === 'true'`, offline simulation is forbidden. Unreachable gateways immediately return `502 Bad Gateway`.

---

## 3. WCAG 2.2 AA Accessibility & Usability Standards

Evaluated and confirmed through automated browser test suites ([`frontend/tests/a11y_wcag.test.mjs`](file:///C:/Sunborn/frontend/tests/a11y_wcag.test.mjs)):

| Accessibility Standard | Implementation Details | Verified Test Status |
|---|---|---|
| **Semantic Landmarks** | Root `<main id="main-content">`, `<nav aria-label="Main Navigation">`, `<header>`, and `<footer>` landmarks. | **Passed** |
| **Color Contrast** | Minimum 4.5:1 text-to-background contrast ratio across dark slate terminal tokens (`bg-slate-950`, `text-slate-100`). | **Passed** |
| **Focus Visibility** | Explicit `focus-visible:ring-2 focus-visible:ring-cyan-400` outlines across all interactive buttons, tabs, and links. | **Passed** |
| **Keyboard Navigation** | Complete tab sequence order; ESC key dismisses modals and command menus (`cmdk`). | **Passed** |
| **Live Announcements** | Dynamic state changes (burst load spikes, invalidation cascades) emit `aria-live="polite"` screen-reader updates. | **Passed** |
| **Zero Dead Buttons** | Every button in the dashboard triggers an active RPC, navigates, or opens an explicit operational probe modal. | **Passed** |
