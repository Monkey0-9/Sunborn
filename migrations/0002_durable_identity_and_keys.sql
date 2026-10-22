-- ==============================================================================
-- Sunborn Durable Identity & Cryptographic Key Persistence Schema (MIG-0002)
-- Invariant: Non-Accelerator Authority (PostgreSQL is Authoritative System of Record)
-- ==============================================================================

-- 6. Persistent Enveloped Tenant Data Encryption Keys (TDEK)
CREATE TABLE IF NOT EXISTS tenant_wrapped_keys (
    tenant_id VARCHAR(64) PRIMARY KEY,
    key_version INTEGER NOT NULL DEFAULT 1,
    wrapped_tdek BYTEA NOT NULL,
    kms_provider VARCHAR(64) NOT NULL DEFAULT 'local-aead',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 7. Durable API Credentials & Tenant RBAC Registry
CREATE TABLE IF NOT EXISTS api_credentials (
    key_hash VARCHAR(64) PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    application_id VARCHAR(64) NOT NULL,
    rate_limit_rps INTEGER NOT NULL DEFAULT 100,
    allowed_models JSONB NOT NULL DEFAULT '["*"]'::jsonb,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
);

-- Upgrade databases where the application-created table predates the durable schema.
ALTER TABLE api_credentials ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
ALTER TABLE api_credentials ADD COLUMN IF NOT EXISTS revoked_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_api_credentials_tenant ON api_credentials(tenant_id, status);
CREATE INDEX IF NOT EXISTS idx_api_credentials_expires_at ON api_credentials(expires_at);
