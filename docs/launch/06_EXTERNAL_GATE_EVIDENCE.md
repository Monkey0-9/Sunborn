# External Gate Evidence Checklist

**Purpose:** Track the gates that cannot be executed on the local development host. This checklist is
not a launch approval and does not certify compliance. A gate is complete only when the linked
evidence exists, is reproducible by a second person, and meets the stated pass criteria.

**Current position:** all in-tree gates pass. The gates below are **EXTERNAL / NOT RUN** and remain
open until evidence is attached. See [`docs/STATUS.md`](../STATUS.md) and
[`docs/audit/DEFECT_REGISTER.md`](../audit/DEFECT_REGISTER.md) for the in-tree results.

| Gate | Owner | Status | Required evidence | Pass criteria |
|---|---|---|---|---|
| G-EXT-01 Clean Windows 11 x64 acceptance (Issue #21) | Release engineering | NOT RUN | Fresh Windows 11 VM with no Rust/Node/Python developer toolchains; installer log; `sunborn --version` output; `uninstall.ps1 -Silent -PurgeData` log; screenshot or screen recording | Install, launch, upgrade, and silent uninstall complete without missing-runtime errors; no residual service, firewall rule, or data directory after purge |
| G-EXT-02 Certificate-signed Windows artifact (Issue #23) | Release engineering / security | NOT RUN | Authenticode-signed installer and binary; `signtool verify /pa /v` output; certificate chain and timestamp; release-workflow attestation URL; SHA-256 and SHA-512 sums | Signature verifies on a clean host with a trusted chain and timestamp; published hash matches the signed bytes; GitHub build provenance attestation resolves to the release commit |
| G-EXT-03 Independent penetration test | Security | NOT RUN | Signed engagement scope and rules of engagement; tester report; finding tracker; retest letter | No open Critical or High findings; Medium findings have an accepted owner and date; retest confirms fixes |
| G-EXT-04 Multi-region load and failover at 2x peak | SRE / performance | NOT RUN | Topology diagram; load script and dataset; raw result files; p50/p95/p99, error rate, saturation, and cost; failover timeline with RTO/RPO | Sustains 2x forecast peak within the error budget for the agreed duration; regional failover meets the documented RTO/RPO with no data loss |
| G-EXT-05 Full billing cycle | Finance / billing engineering | NOT RUN | Test-clock or low-value live subscription; invoice; usage export; ledger reconciliation; dunning, refund, and cancellation records | Invoice, usage, and internal ledger reconcile to the cent; dunning and cancellation behave as documented; no unbilled or double-billed usage |
| G-EXT-06 SOC 2 / ISO 27001 | Compliance | NOT RUN | Audit engagement letter; system description; evidence window; auditor report | Report issued for the stated scope and period. No certification is claimed before the report exists |
| G-EXT-07 Legal and privacy review | Legal | NOT RUN | DPA, subprocessor list, privacy notice, records-of-processing, cross-border transfer review, incident-response contact | Signed review covering the launch regions and data classes; required disclosures published |

## Evidence handling

- Store raw artifacts in the release evidence bucket with immutable retention; link them from this
  table rather than pasting screenshots into the repository.
- Record the commit SHA, workflow run URL, operator, date, and host image version for every gate.
- Re-run G-EXT-01, G-EXT-02, and G-EXT-04 for each release candidate that changes the installer,
  signing path, gateway, or deployment topology.
- A gate marked **PARTIAL** in the defect register must not be described as closed in release notes.

## Explicit non-claims

Until the rows above are complete, the project does not claim: production readiness at millions of
concurrent users, multi-region high availability, a completed billing cycle, a signed Windows
artifact, an independent penetration test, or SOC 2 / ISO 27001 certification.
