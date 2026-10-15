---
name: Security Advisory Disclosure
about: Report a potential vulnerability, prompt-injection bypass, or isolation breach
title: "[SEC]: "
labels: ["security", "p0-critical"]
assignees: ""
---

> **CONFIDENTIALITY NOTICE**: Please ensure zero leakage of private keys, tenant IDs, or production secrets in this report.

### 1. Vulnerability Overview
<!-- Summarize the threat vector, injection method, or tenant boundary condition. -->

### 2. Affected Component
- [ ] AI Gateway / AuthN (`crates/sunborn-gateway`)
- [ ] Identity & Canonicalization (`crates/sunborn-identity`)
- [ ] Multi-Tier Cache Store (`crates/sunborn-cache`)
- [ ] Semantic Vector Store (`crates/sunborn-semantic`)
- [ ] Execution Engine (`crates/sunborn-execution`)
- [ ] PostgreSQL / Database Store (`crates/sunborn-db`)

### 3. Impact Assessment
- **CVSS Estimate**:
- **Exploit Preconditions**:
- **Cross-Tenant Leakage Risk**: Yes / No

### 4. Reproduction Proof-of-Concept
<!-- Minimal reproducible script or curl command -->
