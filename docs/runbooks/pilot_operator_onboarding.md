# Sunborn Pilot Operator Onboarding Runbook (Gate N3)

**Document Type:** Standard Operating Procedure (SOP) & Operator Manual  
**Target Audience:** SREs, Platform Engineers, and Designated Pilot Operators  
**Authority:** [docs/STATUS.md](file:///C:/Sunborn/docs/STATUS.md) | [ROADMAP.md](file:///C:/Sunborn/ROADMAP.md) (Gate N3)  
**Status:** **ACTIVE — PILOT READY**

---

## 1. Overview & Operating Boundaries

Sunborn is an enterprise AI inference caching and semantic routing gateway designed for mission-critical, regulated environments.

During **Stage 0 (Controlled Pilot)**:
- **Authorized Scope:** Named enterprise design partners, non-public endpoints, operator-supervised workloads.
- **Governing Principle:** *Correctness before reach*. When safe reuse cannot be mathematically proven, Sunborn abstains and routes the request upstream to the LLM origin.
- **Service Level:** Non-production SLA with active human-in-the-loop oversight.

---

## 2. Infrastructure & Environment Setup

### 2.1 Minimum Hardware Requirements
- **CPU:** 4+ Cores (x86_64 with AVX2/FMA support, or aarch64 with NEON).
- **RAM:** 8 GB minimum (16 GB recommended for high-volume in-memory exact caching).
- **Network:** Low-latency connectivity (< 5 ms) to PostgreSQL, Dragonfly/Redis, and upstream LLM providers.

### 2.2 Core Environment Variables

Create a secure `.env` file or provision Kubernetes secrets containing:

```bash
# --- Gateway Network & Mode ---
SUNBORN_ENV=production
SUNBORN_HTTP_HOST=0.0.0.0
SUNBORN_HTTP_PORT=8080
SUNBORN_LOG_LEVEL=info

# --- Ingress Request Caps ---
SUNBORN_MAX_PROMPT_CHARS=32000
SUNBORN_MAX_COMPLETION_TOKENS=1000

# --- Database & Invalidation Authority ---
SUNBORN_DATABASE_URL=postgres://sunborn_app:SECURE_PASS@postgres.internal:5432/sunborn_prod
SUNBORN_REDIS_URL=redis://dragonfly.internal:6379

# --- Key Management Service (Choose AWS KMS or HashiCorp Vault) ---
# Option A: AWS KMS
SUNBORN_KMS_PROVIDER=aws-kms
AWS_REGION=us-east-1
SUNBORN_AWS_KMS_KEY_ARN=arn:aws:kms:us-east-1:123456789012:key/pilot-hsm-key

# Option B: HashiCorp Vault
# SUNBORN_KMS_PROVIDER=vault-transit
# SUNBORN_VAULT_ADDR=https://vault.internal:8200
# SUNBORN_VAULT_TRANSIT_KEY=sunborn-pilot-transit
# SUNBORN_VAULT_TOKEN=s.YOUR_ROTATING_TOKEN

# --- FinOps Global Limits ---
SUNBORN_MAX_SPEND_PER_WINDOW_USD=100.0
```

---

## 3. Starting and Verifying the Gateway

### 3.1 Pre-Flight Check
Before starting the gateway process, verify all database migrations and configurations:

```bash
# Verify system integrity and OpenAPI specifications
python scripts/verify_public_launch.py --strict
```

### 3.2 Starting the Gateway Binary

```bash
# Direct execution (production profile)
cargo run --release -p sunborn-gateway
```

### 3.3 Verifying Health and Component Readiness

Inspect the `/ready` diagnostic endpoint to ensure all external adapters are connected:

```bash
curl -s http://localhost:8080/ready | jq .
```

**Expected Response**:
```json
{
  "status": "ready",
  "components": {
    "cache": { "status": "connected", "error": null },
    "database": { "status": "connected", "error": null },
    "vector_store": { "status": "connected", "error": null },
    "llm_origin": { "status": "connected", "is_mock": false, "error": null },
    "embedding_provider": { "status": "connected", "is_deterministic": false, "error": null },
    "execution_cache": { "status": "ready", "storage_tier": "process-local-ephemeral" },
    "learning_store": { "status": "ready", "storage_tier": "process-local-ephemeral" }
  }
}
```

---

## 4. Tenant Provisioning & Credential Lifecycle

### 4.1 Provisioning a Pilot Tenant
Create a tenant record with an initial rate limit:

```bash
curl -X POST http://localhost:8080/v1/governance/provision \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -d '{
    "tenant_id": "tenant-enterprise-pilot-01",
    "name": "Enterprise Pilot Partner",
    "rate_limit_rps": 50
  }'
```

### 4.2 Issuing API Credentials
Generate an authenticated, scoped API credential:

```bash
curl -X POST http://localhost:8080/v1/auth/credentials \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -H "x-quorum-admin-key: $SUNBORN_QUORUM_ADMIN_KEY" \
  -d '{
    "tenant_id": "tenant-enterprise-pilot-01",
    "application_id": "app-pilot-rag",
    "rate_limit_rps": 50,
    "allowed_models": ["gpt-4o", "claude-3-5-sonnet"],
    "is_admin": false,
    "expires_in_secs": 31536000
  }'
```

`x-quorum-admin-key` is mandatory in production mode: credential issuance
requires two-person control, and the second key must belong to a different
administrator than the primary caller (`$SUNBORN_QUORUM_ADMIN_KEY` is separate
from `$SUNBORN_ADMIN_KEY`). Setting `SUNBORN_ADMIN_QUORUM_REQUIRED=false`
disables the second key for local testing only. The response contains the raw
key exactly once; only its hash is persisted.

`expires_in_secs` is a relative lifetime, not a timestamp. Omit it for a
non-expiring key.

---

## 5. FinOps & Spend Velocity Enforcement

### 5.1 Setting a Tenant Spend Ceiling
Set a hard spend ceiling (enforced across all cluster replicas with advisory locking):

```bash
curl -X POST http://localhost:8080/v1/governance/spend-ceiling \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -d '{
    "tenant_id": "tenant-enterprise-pilot-01",
    "ceiling_usd": 25.0
  }'
```

### 5.2 Setting Daily & Monthly Request Quotas

```bash
curl -X POST http://localhost:8080/v1/governance/tenant-quota \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -d '{
    "tenant_id": "tenant-enterprise-pilot-01",
    "tier": "enterprise-pilot",
    "daily_requests": 10000,
    "monthly_requests": 250000
  }'
```

---

## 6. Runtime Monitoring & Observability

### 6.1 Prometheus Metrics Scrape
Scrape the gateway Prometheus metrics at `/metrics`:

```bash
curl -s http://localhost:8080/metrics | grep sunborn_
```

Key golden signals:
- `sunborn_requests_total`: Total processed LLM requests.
- `sunborn_cache_hits_total{tier="exact"}`: Exact hit rate.
- `sunborn_cache_hits_total{tier="semantic"}`: Conformal semantic hit rate.
- `sunborn_spend_velocity_exceeded_total`: Requests rejected due to spend ceilings.
- `sunborn_rate_limit_exceeded_total`: Rate-limited requests (HTTP 429).

### 6.2 Grafana Dashboard Setup
Import the pre-configured Grafana telemetry dashboard from:
[`deploy/grafana/sunborn_gateway_dashboard.json`](../../deploy/grafana/sunborn_gateway_dashboard.json).

---

## 7. Emergency Response & Kill Switches

### 7.1 Emergency Cryptographic Shredding (GDPR Article 17)
Crypto-shredding destroys the tenant's data-encryption key, which makes every
ciphertext written under it permanently unreadable. The gateway then cascades
the deletion across the exact cache, vector store, execution store, database,
learning store, API credentials and quota ledger. The key destruction itself is
sub-millisecond; the cascade scales with how much data the tenant has stored.

```bash
curl -X POST http://localhost:8080/v1/governance/shred \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -H "x-quorum-admin-key: $SUNBORN_QUORUM_ADMIN_KEY" \
  -d '{
    "tenant_id": "tenant-enterprise-pilot-01"
  }'
```

The response reports per-store deletion counts and the key-shred duration, so
the cascade is auditable rather than assumed.

### 7.2 Immediate Credential Revocation
Revoke a compromised credential instantly across all cluster replicas:

```bash
curl -X POST http://localhost:8080/v1/auth/credentials/revoke \
  -H "Content-Type: application/json" \
  -H "x-api-key: $SUNBORN_ADMIN_KEY" \
  -H "x-quorum-admin-key: $SUNBORN_QUORUM_ADMIN_KEY" \
  -d '{
    "key_hash": "a1b2c3d4e5..."
  }'
```

### 7.3 Circuit Breaker Manual Trip
To bypass cache lookup and route 100% of traffic directly to upstream LLM providers:

```bash
curl -X POST http://localhost:8080/v1/chaos/trip \
  -H "x-api-key: $SUNBORN_ADMIN_KEY"
```

To reset the circuit breaker back to normal caching:

```bash
curl -X POST http://localhost:8080/v1/chaos/reset \
  -H "x-api-key: $SUNBORN_ADMIN_KEY"
```

---

## 8. Operator Verification Sign-Off Template (Gate N3)

To be completed by the designated pilot lead before activating pilot partner traffic:

```
=============================================================================
             SUNBORN PILOT OPERATOR ATTESTATION (GATE N3)
=============================================================================
Pilot Name        : __________________________________________________
Designated Lead   : __________________________________________________
Gateway Version   : 1.0.0 (commit cf29c03)
Environment URL   : __________________________________________________
Execution Date    : 2026-11-19

Checklist Items:
[ ] 1. All environment variables configured and validated via `/ready`.
[ ] 2. KMS/Vault remote provider verified with canary roundtrip probe.
[ ] 3. Named tenant provisioned with explicit allowed models and spend ceiling.
[ ] 4. Test request successfully executed and cached under tenant TDEK.
[ ] 5. Golden signals verified in Prometheus/Grafana dashboard.
[ ] 6. Emergency kill switch and shredding procedures verified on test tenant.

Operator Signature: ____________________________ Date: ______________
=============================================================================
```
