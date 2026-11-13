# 15 — Frontend Audit

**Scope:** `frontend/src` (34 files, ~11,055 LOC) + `frontend/tests` (7 files, ~1,142 LOC).

---

## 1. What is real

- **BFF security policy** (`src/lib/proxy-policy.ts`): CSRF origin matching, path-traversal normalization, exact-host/localhost-hostname matching, privileged-route fail-closed, deny-by-default. Genuinely unit-tested (`bff_security.test.mjs` 14 + `auth_session.test.mjs` 3).
- **API client** (`src/lib/api.ts`): ~45 typed methods, structured `SunbornApiError`, actionable network errors.
- **Gateway status reader** (`src/lib/gateway-status.mjs`): rejects stale (>60s)/malformed/non-OK payloads. The most honest file in the repo.
- **Ceremony page**: explicitly labelled demo; a test forbids fabricated numbers in it.
- **Stress simulator**: carries an honest "No network traffic is sent" banner.

## 2. What is fake (P0)

### 2.1 A fake backend ships in the app
`src/lib/standalone-simulator.ts` (698 LOC) intercepts every non-privileged route when `NODE_ENV !== "production"` and `STRICT_GATEWAY !== "true"` (`src/app/api/proxy/[...path]/route.ts:163-190`). Since `npm run dev` sets NODE_ENV=development, **fabrication is the default local experience.** It fabricates:
- cluster status/nodes/epochs (`:68-83`)
- FinOps savings (`:90-110`)
- Merkle root + `is_valid: true` (`:196-222`)
- credentials `sb_live_${Math.random()}` (`:492-520`)
- session `authenticated:true, is_admin:true` (`:600`)
- cache hit decided by `lastMsg.length % 2 === 0` (`:167-190`)

### 2.2 Auth bypass
`src/app/api/auth/route.ts`:
- `:132` — if upstream omits `access_token`, stores the **plaintext client_secret** as the session token.
- `:153-176` — when gateway is unreachable and non-strict, accepts **any** client_id/client_secret and sets `sunborn_session` to the raw secret.

### 2.3 Fabricated credentials on failure
`src/app/security/page.tsx:145-172` — on API error, mints `sb_live_<random>` and a random 64-hex "hash", then adds it to the "Active Tenant Credentials" directory as ACTIVE.

## 3. Hardcoded metrics / dead interactions

| Location | Fabrication |
|---|---|
| `app/page.tsx` | cluster ping 0.8/18.2/34.1ms, hit rates 98.4/96.1/94.8%, "0.12 µs" invalidation, 100.0% precision, 42.1 µs shred, 12.09% savings, adversarial table |
| `app/knowledge/page.tsx` | dummy sources with invented UUIDs; guardrail "evaluate" = client Jaccard |
| `app/finops/page.tsx` | CSV/JSON export hardcoded; `infraOverhead=280.0`; reservation workbench is `useState` only |
| `app/execution/page.tsx` | DAG nodes invented; `proofVerified` initialized `true`; "812 ms saved" literal |
| `app/security/page.tsx` | pre-seeded `threatDetected:true`, fake keys, "TENANT LEAKAGE 0.0% PROVEN" |
| `app/governance/page.tsx` | TDEK registry hardcoded tenants + invented KMS backends |
| `app/explorer/page.tsx` | hardcoded cache entries; purge optimistically succeeds on failure; tenant/app fields never sent |
| `components/simulator/stress-simulator.tsx` | "63 Passing Tests" literals; magic latency formulas |
| `components/ui/canvas-manifold.tsx` | random particles; P99 renders `0 µs` |

Dead code: `components/ui/aurora-background.tsx` (zero importers), `components/ui/toast.tsx` (zero call sites). ~19 of ~45 API client methods are unwired.

## 4. Accessibility (WCAG 2.2)

Good: `lang="en"`, banner/nav roles, global focus-visible, labelled login inputs, aria-live toasts.

Failures:
- 11 unlabelled range sliders (8 unlabelled): `stress-simulator.tsx:330,352,376,398`, `finops/page.tsx:410,426`, `learning/page.tsx:353`, `explorer/page.tsx:253`, `security/page.tsx:509`.
- Clickable `<div>`s with no keyboard/role: `knowledge/page.tsx:628-641`, `execution/page.tsx:211-218` (WCAG 2.1.1/4.1.2).
- Canvas with click handler, no role/label/tabIndex (`canvas-manifold.tsx:330`).
- Colour-only status in several tables.
- No `Content-Security-Policy` header (only HSTS/nosniff/X-Frame-Options, `next.config.mjs:13-26`).
- Zero ARIA in 7 of 10 page files.

## 5. Tests

- `bff_security.test.mjs` (14) and `auth_session.test.mjs` (3): genuine pure-function tests.
- `ceremony_status.test.mjs` (4): genuine.
- `views_and_navigation.test.mjs` (4) + `accessibility_wcag.test.mjs` (5): **source-text grep**, assert nothing about rendered behavior. The a11y tests pass despite real violations.
- `e2e_browser_workflows.test.mjs` (7): Playwright with **every gateway call mocked**, then asserts its own mocks. Proves rendering, not integration.
- `test_inspect.mjs`: screenshot script, not a test.

## 6. Verdict

Frontend authenticity: **~40% real code, ~60% set dressing.** BFF and API client are well-engineered; the default dev experience is fabricated, auth has a bypass, credentials are minted on failure, and headline numbers are literals. Accessibility is not conformance-level. Frontend score: **4/10**.
