# 36 — OWASP Assessment

---

## 1. OWASP Top 10 (2021)

| Risk | Status | Evidence |
|---|---|---|
| A01 Broken Access Control | **FAIL** | dev auth bypass (F-001); fake `is_admin:true` in simulator |
| A02 Cryptographic Failures | PARTIAL | AEAD sound; shared HMAC key role (F-015); no CSP |
| A03 Injection | PASS | parameterized SQL; React escaping; no eval found |
| A04 Insecure Design | **FAIL** | self-attested safety evidence (F-004); client-supplied freshness (F-005) |
| A05 Security Misconfiguration | **FAIL** | dev compose insecure defaults; frontend readOnlyRootFilesystem false; no CSP |
| A06 Vulnerable Components | PARTIAL | audit.toml suppresses advisories; no container scan |
| A07 Auth Failures | **FAIL** | dev auth accepts any credential; plaintext secret session |
| A08 Integrity Failures | **FAIL** | no cosign/SLSA; non-deterministic SBOM |
| A09 Logging/Monitoring | PARTIAL | audit_events table; no alerting/SLO |
| A10 SSRF | PARTIAL | configurable upstream URLs; verify host allowlist |

## 2. OWASP API Security Top 10 (2023)

| Risk | Status |
|---|---|
| API1 BOLA | PARTIAL — tenant scoping in queries; verify every object |
| API2 Broken Auth | FAIL (dev bypass) |
| API3 BOPLA | PARTIAL — typed responses; verify over-exposure |
| API4 Unrestricted Resource Consumption | **FAIL** — no general rate limit/body cap |
| API5 BFLA | PARTIAL — admin gating exists |
| API6 Unrestricted Business Flows | PARTIAL — spend velocity only |
| API7 SSRF | PARTIAL |
| API8 Misconfiguration | FAIL |
| API9 Inventory Management | PARTIAL — OpenAPI drift check |
| API10 Unsafe Consumption | PARTIAL — upstream responses passed through |

## 3. OWASP ASVS 4.0 (selected)

- V2 Auth: fails dev path; production path partial.
- V3 Session: HttpOnly/SameSite set; but stores raw secret (F-001).
- V6 Cryptography: strong primitives.
- V7 Error/Logging: structured errors; no sensitive data observed.
- V8 Data Protection: AEAD + shred strong.
- V14 Configuration: CSP absent; hardening incomplete.

## 4. Priority fixes

1. Remove dev auth bypass and fake credential minting.
2. Add general per-tenant rate limits + request size caps.
3. Add CSP and security headers.
4. Remove advisory suppressions or justify them.
5. Add cosign/SLSA and deterministic SBOM.
6. Compute certificate evidence; server-derive freshness.

## 5. Verdict

OWASP posture: **FAIL** on A01/A04/A05/A07/A08. These are exactly the P0 findings; remediation in `46`.
