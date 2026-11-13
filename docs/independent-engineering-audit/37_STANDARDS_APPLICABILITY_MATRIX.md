# 37 — Standards Applicability Matrix

Applicability (A) and current alignment (S) — `HIGH/MED/WEAK/NA`.

| Standard | Domain | A | S | Owner doc |
|---|---|---|---|---|
| IEEE 1012 | V&V | HIGH | WEAK | 32 |
| IEEE 29119 | Testing | HIGH | WEAK | 32,28 |
| IEEE 730 | SQA | HIGH | WEAK | 32 |
| IEEE 42010 | Architecture | HIGH | PARTIAL | 32,06 |
| IEEE 12207/15288 | Lifecycle | MED | WEAK | 32 |
| IEEE 29148 | Requirements | HIGH | WEAK | 32,08 |
| IEEE 1016 | Design | MED | PARTIAL | 32 |
| IEEE 1028 | Audits | MED | PARTIAL | 32 |
| IEEE 754 | Floating point | MED | PASS | 13 |
| ISO/IEC 25010 | Product quality | HIGH | PARTIAL | 34 |
| ISO/IEC 25012 | Data quality | MED | PARTIAL | 34,24 |
| ISO/IEC 25023 | Measurement | MED | WEAK | 34 |
| ISO/IEC 25030/25040 | Req/eval | MED | WEAK | 34 |
| ISO/IEC/IEEE 42010 | Arch description | HIGH | PARTIAL | 34 |
| ISO/IEC 27001/27002 | Security | HIGH | WEAK | 34,25 |
| ISO/IEC 27005 | Risk | HIGH | WEAK | 34,44 |
| ISO/IEC 27701 | Privacy | HIGH | PARTIAL | 34,26 |
| ISO/IEC 42001/23894 | AI mgmt/risk | MED | WEAK | 34,23 |
| NIST CSF 2.0 | Security | HIGH | WEAK | 35 |
| NIST 800-218 | Secure SDLC | HIGH | WEAK | 35 |
| NIST 800-53 | Controls | HIGH | WEAK | 35 |
| NIST 800-57 | Key mgmt | HIGH | PARTIAL | 35,26 |
| NIST 800-90 | RNG | HIGH | PASS | 35,26 |
| NIST 800-207 | Zero trust | MED | PARTIAL | 35 |
| NIST AI RMF | AI risk | MED | WEAK | 35,23 |
| OWASP Top 10 | Web | HIGH | FAIL | 36 |
| OWASP API Top 10 | API | HIGH | FAIL | 36 |
| OWASP ASVS | Verification | HIGH | PARTIAL | 36 |
| OWASP SAMM | Maturity | MED | WEAK | 36 |
| SLSA | Supply chain | HIGH | WEAK | 39 |
| SPDX / CycloneDX | SBOM | MED | PARTIAL | 39 |
| Sigstore | Signing | HIGH | NOT IMPLEMENTED | 39 |
| OpenTelemetry | Observability | MED | WEAK | 30 |
| WCAG 2.2 | Accessibility | HIGH | FAIL | 38 |
| WAI-ARIA | Accessibility | HIGH | PARTIAL | 38 |
| RFC 6962 | Merkle | MED | PARTIAL | 26 |
| RFC 8785 | JCS | MED | PARTIAL | 11 |
| OAuth 2.0 / OIDC | Auth | LOW | NA | — |

## Notes

- "Alignment" here means the repo contains artifacts consistent with the standard, not certification.
- No standard is fully satisfied.
- The standards most worth adopting now: IEEE 29148 (one RTM), NIST 800-57 (key mgmt), OWASP ASVS (verification), WCAG 2.2 (frontend).
