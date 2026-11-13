# 35 — NIST Assessment

Alignment only; no certification.

| Framework | Relevance | Alignment | Notes |
|---|---|---|---|
| Cybersecurity Framework 2.0 | High | WEAK | No Govern/Identify/Protect/Detect/Respond/Recover evidence trail |
| SP 800-218 SSDF | High | WEAK | No secure SDLC policy; no threat model in build gate |
| SP 800-53 | High | WEAK | No control catalog mapping |
| SP 800-61 (incident response) | Medium | WEAK | No IR plan |
| SP 800-63 (identity) | Medium | PARTIAL | API-key auth; no MFA/OIDC |
| SP 800-57 (key mgmt) | High | PARTIAL | Keys/KMS real; no rotation policy, shared HMAC key |
| SP 800-90 series (RNG) | High | PASS | OS CSPRNG for nonces/keys |
| SP 800-207 (Zero Trust) | Medium | PARTIAL | Tenant isolation; no device/user identity plane |
| AI RMF / Generative AI Profile | Medium | WEAK | No AI risk governance |

## Highest-priority gaps

1. **SP 800-57:** no key rotation; certificate HMAC key shared with learning feedback (F-015). Add key IDs + rotation + separation.
2. **SP 800-218:** add a threat-model gate and secure-coding checks to CI.
3. **800-63:** API keys only; document that operator consoles need OIDC/MFA.
4. **AI RMF:** document retrieval/embedding risk, dataset provenance, and abstention policy.

## Strengths

- CSPRNG usage correct.
- AES-256-GCM with unique 96-bit nonces.
- Fail-closed production readiness checks.
- Tenant isolation in queries.
