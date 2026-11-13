# 25 — Security Audit

Defensive review only. No external systems attacked. Findings classified P0–P4.

---

## 1. Threat model (STRIDE summary)

| Threat | Surface | Control | Status |
|---|---|---|---|
| Spoofing | API key/session | hashed keys, HttpOnly cookie | PARTIAL (dev bypass F-001) |
| Tampering | request/certificate | HMAC cert | PARTIAL (self-attested F-004) |
| Repudiation | audit | audit_events table | PARTIAL (not transactionally bound) |
| Information disclosure | tenant data | tenant-scoped queries, AEAD | PARTIAL (no RLS; test coverage thin) |
| DoS | ingress | login rate limit, spend velocity | PARTIAL (no general rate limit/body cap) |
| Elevation | admin quorum | dual-admin distinct keys | PARTIAL (not cryptographic) |

## 2. P0 findings

### F-001 — Dev auth bypass
`frontend/src/app/api/auth/route.ts:152-176`: when the gateway is unreachable and not strict, any `client_id`/`client_secret` yields a successful session; `:168` stores the raw secret as `sunborn_session`. `:132` also persists `client_secret` when upstream omits `access_token`.
Impact: authentication is decorative in dev; a leaked/known dev secret becomes a session token. In production `NODE_ENV=production` disables it, but the posture is unsafe by default.

### F-002 — Fake backend default
`proxy/route.ts:163-190` serves `standalone-simulator.ts` in dev. It returns `authenticated:true, is_admin:true` for sessions and fabricates proofs/keys. Impact: operators/developers can be misled; a demo could be mistaken for real security behavior.

### F-003 — Fabricated credentials
`security/page.tsx:145-172`: on API failure, mints `sb_live_<random>` and a random "hash", labels it ACTIVE. Impact: an operator could distribute a credential that authenticates nothing, or believe a revoked key still works.

### F-004 — Self-attesting certificate
`handlers.rs:658-666` hardcodes safety evidence; `certificate.rs:454-462` validates it. Impact: the "verified reuse" guarantee is not backed by measurement.

### F-005 — Client-controlled freshness
`handlers.rs:657,671`: dependency snapshots/epoch from request body. Impact: stale reuse can be forced/forged within registry constraints.

### F-006 — Qdrant delete mismatch
`vector_store.rs:469-501`: delete uses raw id, insert hashed UUID. Impact: invalidated vectors may persist in Qdrant → stale semantic hits.

### F-007 — False public claims
Performance/safety claims in README (see 29/41). Impact: consumer/regulatory misrepresentation.

## 3. P1 findings

| ID | Finding |
|---|---|
| F-010 | Silent fallback to in-memory/LSH on backend failure |
| F-015 | Certificate HMAC key reused as learning-feedback secret |
| F-016 | `KEYS` O(N) blocking scan |
| F-019 | `run_blocking_async` spin/block_in_place |
| F-031 | Missing documented cache headers (integrity of representation) |
| F-032 | `conformalScore` is a keyword heuristic |
| F-033 | Chaos endpoints in production router (verify admin gating) |
| F-034 | No idempotency keys on mutating endpoints |
| F-035 | No general rate limiting / request size cap |
| F-036 | Multi-region manifests contain fake KMS ARNs and placeholder images |

## 4. Injection / web

- SQL: parameterized, no injection found.
- XSS: React escapes by default; no `dangerouslySetInnerHTML` found.
- CSRF: BFF origin checks real (`proxy-policy.ts`).
- SSRF: BFF forwards to configured gateway URL only; verify host allowlist.
- Path traversal: normalized in BFF policy.
- Deserialization: serde typed; no unsafe deserialization found.
- CSP: absent (`next.config.mjs` lacks `Content-Security-Policy`).

## 5. Secrets

- No production secrets in tracked source.
- Demo credentials in `login/page.tsx` (placeholders) escape the secret-scan test.
- Helm secret template genuinely fails closed on placeholders/reuse (good).
- `deploy/docker-compose.yml` uses `dev_insecure_*` values (dev only).

## 6. Dependency / supply chain

- `audit.toml` still ignores RUSTSEC-2026-0459 and RUSTSEC-2023-0071 while docs claim "0 vulnerabilities".
- No container scanning, no cosign/SLSA despite claims.
- `npm audit --omit=dev` skips dev CVEs.

## 7. Verdict

Security is **NOT production-ready**. The gateway auth is solid in production mode, but the frontend dev path, self-attested proofs, client-controlled freshness, and silent fallbacks are material. Security score: **4/10**.
