-- ==============================================================================
-- Sunborn Enterprise FinOps, Key Management & Security Schema Migration (DB-002)
-- High-concurrency zero-lock table and index structures
-- ==============================================================================

-- 1. Tenant Data Encryption Keys (TDEK) Registry (Unified Schema Authority: tenant_wrapped_keys)
CREATE TABLE IF NOT EXISTS tenant_wrapped_keys (
    tenant_id VARCHAR(128) PRIMARY KEY,
    key_version INTEGER NOT NULL DEFAULT 1,
    wrapped_tdek BYTEA NOT NULL,
    kms_provider VARCHAR(64) NOT NULL DEFAULT 'local-aead',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tenant_wrapped_keys_provider ON tenant_wrapped_keys(kms_provider);

-- Backward-compatibility view for tenant_encryption_keys
CREATE TABLE IF NOT EXISTS tenant_encryption_keys (
    tenant_id VARCHAR(128) PRIMARY KEY,
    key_version INTEGER NOT NULL DEFAULT 1,
    wrapped_tdek BYTEA NOT NULL,
    nonce BYTEA NOT NULL DEFAULT '\x000000000000000000000000'::bytea,
    auth_tag BYTEA NOT NULL DEFAULT '\x00000000000000000000000000000000'::bytea,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shredded_at TIMESTAMPTZ
);

-- 2. FinOps Accounting & Token Avoidance Ledger
CREATE TABLE IF NOT EXISTS finops_ledger (
    id BIGSERIAL PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    application_id VARCHAR(64) NOT NULL,
    model VARCHAR(128) NOT NULL,
    cache_tier VARCHAR(32) NOT NULL,
    prompt_tokens_avoided BIGINT NOT NULL DEFAULT 0,
    completion_tokens_avoided BIGINT NOT NULL DEFAULT 0,
    cost_avoided_usd NUMERIC(12, 6) NOT NULL DEFAULT 0.0,
    storage_cost_usd NUMERIC(12, 6) NOT NULL DEFAULT 0.0,
    net_savings_usd NUMERIC(12, 6) NOT NULL DEFAULT 0.0,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_finops_tenant_time ON finops_ledger(tenant_id, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_finops_model ON finops_ledger(model, recorded_at DESC);

-- 3. Radix KV-Cache Prefix Affinity State
CREATE TABLE IF NOT EXISTS prefix_affinity_records (
    prefix_hash VARCHAR(64) PRIMARY KEY,
    prefix_text TEXT NOT NULL,
    matched_tokens INTEGER NOT NULL,
    affinity_node VARCHAR(128) NOT NULL,
    access_count BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_hit_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_prefix_affinity_node ON prefix_affinity_records(affinity_node);
CREATE INDEX IF NOT EXISTS idx_prefix_hit_count ON prefix_affinity_records(access_count DESC);
