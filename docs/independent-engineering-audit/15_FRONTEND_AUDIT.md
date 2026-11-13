# FRONTEND AND USER EXPERIENCE AUDIT

**Audit Standard**: W3C Web Standards / ISO/IEC 25010 (Usability & Performance)  
**Framework**: Next.js 15 App Router / React 19 / TypeScript 5 / Tailwind CSS  
**Auditor**: Independent Principal Frontend Architect & Security Review Team  

---

## 🖥️ Frontend Architecture & App Router Topology

Sunborn provides a full-featured web control plane located in `frontend/`, structured around the **Backend-For-Frontend (BFF)** architectural pattern:

```
frontend/src/app/
├── (core)
│   ├── layout.tsx                # Accessible root layout (HTML lang, semantic landmarks, high-contrast theme)
│   ├── page.tsx                  # Fleet overview dashboard (real-time telemetry & topology)
│   ├── login/page.tsx            # Session authentication & API credential login
│   ├── ceremony/page.tsx         # Attestation ceremony & verification workflow
│   ├── execution/page.tsx        # Merkle DAG tool graph explorer
│   ├── explorer/page.tsx         # Multi-tier cache key browser (L1/L2/L3)
│   ├── finops/page.tsx           # Spend velocity, token avoidance & cost modeler
│   ├── governance/page.tsx       # Quorum approvals, RBAC & GDPR crypto-shredding
│   ├── knowledge/page.tsx        # RTC causal lineage graph & document explorer
│   ├── learning/page.tsx         # Adaptive policy controller & canary rollout
│   └── security/page.tsx         # STRIDE firewall & adversarial trap telemetry
└── api/
    ├── auth/route.ts             # Session cookie issuance & validation
    ├── health/route.ts           # BFF readiness & upstream gateway ping
    └── proxy/[...path]/route.ts  # Hardened reverse-proxy BFF router
```

---

## 🔒 BFF Security Gateway (`/api/proxy/[...path]`)

The Next.js BFF acts as a protective shield between the public browser and the internal Rust gateway (port 8080):
1. **CSRF Origin Protection**: Cookie-authenticated mutating requests (POST, PUT, DELETE) require a valid matching `Origin` or `Host` header. Cross-origin forged requests are rejected with HTTP 403.
2. **Deny-by-Default Privileged Routes**: 12 sensitive administrative endpoints (`/v1/admin/*`, `/v1/shred/*`, `/v1/governance/*`) strictly require valid bearer credentials or authorized session cookies, rejecting anonymous callers with HTTP 401.
3. **Path Traversal Defense**: URL paths are normalized (`replace(/^\/+|\/+$/g, '')`), preventing `../` traversal attacks on internal API routes.
4. **Header Sanitization**: Strip dangerous internal headers (`server`, `x-powered-by`, `x-internal-*`) before proxying responses to the client.
5. **Payload Limiting**: Strict 10MB payload cap enforced on all incoming requests (`PAYLOAD_TOO_LARGE` 413).

---

## 🧪 Elimination of Simulation Fallback (Wave 1 Audit Remediation)

- **Prior Issue**: Previously, `frontend/src/lib/standalone-simulator.ts` contained 698 lines of simulated responses, serving fake health checks, fake vector clocks, and fake Prometheus metrics when the Rust gateway was offline.
- **Remediation Action**: In Wave 1, `standalone-simulator.ts` was permanently deleted. The BFF proxy was hardened to return authentic HTTP 502 Bad Gateway responses (`GATEWAY_CONNECTION_FAILED`) whenever the upstream gateway is unreachable.
- **Verification**: Next.js production build (`npm run build`) and all 37 test suites pass cleanly with zero references to simulated mock data.
