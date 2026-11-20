# Sunborn Platform: India Sovereign & Enterprise Edition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Sunborn into an India-first sovereign semantic execution gateway and desktop AI cockpit adhering to the Digital Personal Data Protection Act (DPDP Act 2023), RBI financial data localization, CERT-In cybersecurity directives, dual INR/USD micro-unit FinOps with 18% GST, Indic multilingual AST guardrails, and native support for Indian foundation models (Sarvam, Krutrim, BharatGPT, Bhashini).

**Architecture:** Hexagonal architecture extended with an Indian Sovereign Compliance Engine (`sunborn-sovereign-in`), an Indic AST negation/polarity analyzer, an integer micro-paise FinOps ledger with GST tax engine, and automated DPDP Article 12/17 data principal rights workflows.

**Tech Stack:** Rust 1.80+ (Axum, Tokio, SQLx, Ring, AES-GCM, Unicode-NFC), Next.js 15 (Tailwind CSS, Hallmark tokens, Indian number formatting), PostgreSQL 16 (timescale partitioned outbox, advisory locks), Qdrant (dense vectors with Indic embeddings), Sarvam AI / Krutrim / OpenAI / Anthropic APIs.

**Spec:** [`docs/compliance/INDIA_SOVEREIGN_AI_SPEC.md`](file:///C:/Sunborn/docs/compliance/INDIA_SOVEREIGN_AI_SPEC.md)

---

## Global Constraints

- **DPDP Act 2023 Compliance:** Zero cross-border egress of sensitive personal data or financial reasoning context outside Indian borders when `SUNBORN_JURISDICTION=IN_SOVEREIGN`.
- **RBI Data Localization:** Payment and spend transactions must be committed exclusively to database clusters located in India (e.g. AWS `ap-south-1`/`ap-south-2`, GCP `asia-south1`/`asia-south2`, Azure Central India, or on-premise CtrlS/Yotta).
- **CERT-In Compliance:** NTP time sync strictly tied to NPL/NIC Indian Standard Time (IST, UTC+5:30); 180-day forensic log retention; 6-hour incident alerting webhook.
- **Authoritative Currency Ledger:** Integer micro-paise (`MicroPaise(i64)`, 1 INR = 100 paise = 1,000,000 micro-paise). Zero floating point rounding drift.
- **Taxation Engine:** 18% GST (CGST 9% + SGST 9% or IGST 18%) calculated on all token reservations with SAC code `998313` / `998314` and GSTIN format validation (`^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z]{1}[1-9A-Z]{1}Z[0-9A-Z]{1}$`).
- **Indic Script Normalization:** Deterministic NFC/NFKC canonicalization over 12 scheduled languages with Indic virama, matra, and ZWJ/ZWNJ preservation.

---

## Review Focus

1. **Indic Token Inflation Arbitrage:** Standard BPE tokenizers inflate Indic text by 3x–6x. Caching must correctly identify semantic equivalence across scripts without tripping on matras or conjuncts.
2. **GST Invoice Irreversibility:** Once an invoice is generated with GSTIN and IRN hash, credit notes must be issued instead of modifying original ledger entries.
3. **Data Principal Erasure (DPDP Section 12):** Cryptographic shredding must destroy per-tenant AES-256 keys in <2 ms while maintaining GST audit logs for the mandatory statutory period (7 years under GST Act).
4. **Adversarial Indic Inversion:** Negation inversion attacks in Hindi, Tamil, Telugu, and Bengali must be caught by AST structural guardrails with 0 false passes on hard negatives.
5. **Local Desktop Loopback & UPI:** Local BYOK instances must remain bound to `127.0.0.1:8080`, display timestamps in IST, and format currency in Lakhs/Crores (`₹1,50,000`).

---

## Task Decomposition

### Task 1: Indic Multilingual Normalization & AST Negation Guardrails
**Files:**
- Create: `crates/sunborn-semantic/src/indic/mod.rs`
- Create: `crates/sunborn-semantic/src/indic/normalizer.rs`
- Create: `crates/sunborn-semantic/src/indic/negation.rs`
- Modify: `crates/sunborn-semantic/src/guardrails.rs`
- Test: `crates/sunborn-semantic/tests/indic_guardrails_test.rs`

- [ ] Implement Indic Unicode canonicalizer handling Devanagari, Tamil, Telugu, Bengali, Kannada, and Malayalam scripts with ZWJ/ZWNJ preservation.
- [ ] Implement comprehensive negation dictionary and polarity inversion detector for major Indic languages (Hindi: नहीं, मत, ना; Tamil: இல்லை, வேண்டாம்; Telugu: లేదు, కాదు; Bengali: না, নয়; Kannada: ಇಲ್ಲ, ಬೇಡ).
- [ ] Connect Indic normalizer into `StructuralGuardrails::evaluate` before cosine similarity gating.
- [ ] Write integration test verifying adversarial negation rejection on 50 Indic test pairs.

### Task 2: Dual INR / USD Micro-Unit FinOps & GST Tax Engine
**Files:**
- Create: `crates/sunborn-core/src/finops/currency.rs`
- Create: `crates/sunborn-core/src/finops/gst.rs`
- Modify: `crates/sunborn-core/src/finops.rs`
- Modify: `crates/sunborn-db/src/postgres.rs`
- Test: `crates/sunborn-core/tests/gst_finops_test.rs`

- [ ] Implement `MicroPaise(i64)` alongside `MicroUnits(i64)` with 1 INR = 1,000,000 micro-paise.
- [ ] Implement Indian numbering formatter (`format_lakhs_crores(amount: f64) -> String` yielding e.g. `₹1,25,000.50`).
- [ ] Implement GST computation engine: calculate CGST (9%), SGST (9%), and IGST (18%) based on state code (e.g. 27 for Maharashtra, 29 for Karnataka, 07 for Delhi).
- [ ] Implement GSTIN regex and checksum verification.
- [ ] Add PostgreSQL schema migration `migrations/0011_india_finops_gst.sql` adding `amount_micro_paise`, `currency`, `gst_rate_basis_points`, `gstin`, and `hsn_sac_code`.

### Task 3: DPDP Act 2023 Sovereign Data Protection & CERT-In Module
**Files:**
- Create: `crates/sunborn-identity/src/dpdp.rs`
- Create: `crates/sunborn-gateway/src/compliance/cert_in.rs`
- Modify: `crates/sunborn-gateway/src/handlers.rs`
- Modify: `crates/sunborn-gateway/src/state.rs`
- Test: `crates/sunborn-gateway/tests/dpdp_compliance_test.rs`

- [ ] Implement Data Principal Consent Token (`ConsentRecord`) validating explicit consent before processing sensitive personal data under DPDP Act 2023.
- [ ] Implement DPDP Data Principal Grievance & Erasure handler (`/v1/sovereign/dpdp/erase`) performing instant cryptographic zeroization of tenant personal data while preserving statutory tax records.
- [ ] Implement CERT-In 6-hour incident report dispatcher (`send_cert_in_alert`) with automated payload serialization for unauthorized access, data breach, and DDoS events.
- [ ] Ensure all system log timestamps and audit certificates format in Indian Standard Time (IST, UTC+5:30) with NPL NTP reference metadata.

### Task 4: Sovereign Indian LLM Providers Integration
**Files:**
- Create: `crates/sunborn-gateway/src/origin/sarvam.rs`
- Create: `crates/sunborn-gateway/src/origin/krutrim.rs`
- Create: `crates/sunborn-gateway/src/origin/bhashini.rs`
- Modify: `crates/sunborn-gateway/src/origin/mod.rs`
- Test: `crates/sunborn-gateway/tests/indian_providers_test.rs`

- [ ] Implement Sarvam AI origin adapter supporting `sarvam-2b` and `bulbul:v1` translation/speech models with OpenAI-compatible wire translation.
- [ ] Implement Krutrim AI origin adapter supporting Krutrim foundation models with native INR pricing token accounting.
- [ ] Implement Bhashini Government API adapter for Indic language translation and speech recognition pipelines.
- [ ] Add provider definitions and health check probes to `crates/sunborn-gateway/src/origin/mod.rs`.

### Task 5: Frontend Enterprise Cockpit India Edition
**Files:**
- Create: `frontend/src/lib/india-formatting.ts`
- Create: `frontend/src/components/cockpit/GstBillingCard.tsx`
- Modify: `frontend/src/components/cockpit/MetricsRibbon.tsx`
- Modify: `frontend/src/components/header.tsx`
- Modify: `frontend/src/app/finops/page.tsx`
- Modify: `frontend/src/app/governance/page.tsx`
- Test: `frontend/tests/india_finops_display.test.mjs`

- [ ] Add currency toggle (INR ₹ / USD $) in header and cluster store.
- [ ] Format all financial figures in Lakhs and Crores when INR is active (`₹12.45 L`, `₹1.80 Cr`).
- [ ] Add GST breakdown card displaying Taxable Value, CGST (9%), SGST (9%), Total Tax, and SAC Code 998313.
- [ ] Add DPDP Data Principal Rights dashboard under Governance tab (Consent Management, Erasure Receipt, Grievance Officer Contact).
- [ ] Ensure 100% WCAG 2.2 AA and keyboard navigation compliance is preserved across new components.

### Task 6: Master Verification, Documentation & Verification Scripts
**Files:**
- Create: `docs/compliance/INDIA_SOVEREIGN_AI_SPEC.md`
- Create: `scripts/verify_india_sovereign.py`
- Modify: `README.md`
- Modify: `docs/STATUS.md`
- Modify: `docs/21_QUALITY_ATTRIBUTES_VERIFICATION.md`

- [ ] Run full workspace test suite `cargo test --workspace --all-targets`.
- [ ] Run clippy with zero warnings `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] Run frontend unit and accessibility tests `npm test --prefix frontend`.
- [ ] Verify launch gates `python scripts/verify_public_launch.py --strict`.
- [ ] Create sovereign verification runner `scripts/verify_india_sovereign.py` testing DPDP compliance, GST calculations, Indic AST guardrails, and Sarvam/Krutrim routing.
