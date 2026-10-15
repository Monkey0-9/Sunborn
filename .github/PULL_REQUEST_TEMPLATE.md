## 🚀 Pull Request Overview

### 1. Summary of Changes
<!-- Provide a clear, executive-level explanation of what changes this PR introduces. Reference Linear Issue ID (e.g., `MON-XXX`, `BE-XXX`). -->
- **Linear Issue**: `FIX/FEAT: `
- **Component(s) Affected**: `crates/`, `frontend/`, `deploy/`, `migrations/`
- **Engineering Level**: L5 (Senior) / L6 (Staff/Principal)

---

## 🏛️ Architectural Invariants & Compliance Checklist

- [ ] **Authoritative Origin Truth**: No accelerator (Dragonfly, Qdrant) acts as a correctness authority. All state is backed by PostgreSQL or origin provider.
- [ ] **Tenant Isolation**: Keys and retrieval partitions are strictly namespaced by `TenantId` and `ApplicationId`. Zero cross-tenant leakage.
- [ ] **Deterministic Equivalence**: Canonical hashing normalizes whitespace, serializes JSON fields deterministically, and isolates policy versions.
- [ ] **Monotonic Epoch Fencing**: Distributed cache writebacks prevent stale read-repairs during concurrent invalidations.
- [ ] **O(1) Cryptographic Shredding**: GDPR Article 17 / CCPA erasure verified via instant TDEK memory zeroization.
- [ ] **Zero Data Drift**: Knowledge graph dependency edges correctly track source updates with automated outbox eviction.

---

## 🧪 Verification & Evidence Matrix

| Verification Tier | Command / Test Name | Result / Evidence |
|---|---|---|
| **Unit & Integration** | `cargo test --all-targets --all-features` | PASS (0 failures) |
| **Strict Linting** | `cargo clippy --all-targets --all-features -- -D warnings` | PASS (0 warnings) |
| **Code Formatting** | `cargo fmt --all -- --check` | PASS (Clean) |
| **Frontend Verification** | `npm run build && npx tsc --noEmit` | PASS (Clean) |
| **Security Audit** | OWASP Top 10 / Injection / Auth check | Verified |

---

## 📉 Performance & Latency Impact
- **P99 Latency Delta**:
- **Memory RSS Footprint**:
- **Throughput (req/s)**:

---

## 📸 Screenshots / Telemetry Evidence (if applicable)
<!-- Attach screenshots of the frontend control plane or terminal benchmark output -->
