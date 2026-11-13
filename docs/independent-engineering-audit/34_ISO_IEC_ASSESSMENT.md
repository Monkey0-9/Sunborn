# 34 — ISO/IEC Assessment

Alignment assessment only; no certification claimed.

| Standard | Relevance | Alignment | Notes |
|---|---|---|---|
| ISO/IEC 25010 (product quality) | High | PARTIAL | Functional suitability partial; reliability/security weak; maintainability good |
| ISO/IEC 25012 (data quality) | Medium | PARTIAL | Schemas exist; provenance/synthetic-cost caveats |
| ISO/IEC 25023 (measurement) | Medium | WEAK | No quality measures baseline |
| ISO/IEC 25030 (requirements) | Medium | WEAK | Conflicting RTMs |
| ISO/IEC 25040 (evaluation) | Medium | WEAK | No formal evaluation process |
| ISO/IEC/IEEE 12207 / 15288 | Medium | WEAK | Lifecycle process not evidenced |
| ISO/IEC/IEEE 29148 | High | WEAK | Requirements traceability contradictory |
| ISO/IEC/IEEE 42010 | High | PARTIAL | Multiple architecture descriptions, no single authority |
| ISO/IEC 27001 / 27002 | High | WEAK | No ISMS, no control evidence |
| ISO/IEC 27005 (risk) | High | WEAK | No formal risk treatment |
| ISO/IEC 27017 / 27034 | Medium | N/A | Cloud/app security not evidenced |
| ISO/IEC 27701 (privacy) | High | PARTIAL | Crypto-shred supports erasure; no PIMS |
| ISO/IEC 20000-1 | Low | N/A | Service management not evidenced |
| ISO/IEC 42001 / 23894 (AI mgmt/risk) | Medium | WEAK | No AI risk management process |
| ISO/IEC 22989 (AI concepts) | Low | N/A | Terminology only |

## Key gaps

1. No ISMS or documented security controls → 27001 not aligned.
2. No formal risk treatment → 27005 not aligned.
3. Product-quality measures (25023) not defined.
4. Privacy: crypto-shred is a good technical control but there is no PIMS (27701).

## Recommendation

Do not claim ISO alignment. If pursuing, start with 25010 quality model (define measurable characteristics), 27005 risk register (use `44_RISK_REGISTER.md`), then 27001 controls.
