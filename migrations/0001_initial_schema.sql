-- ==============================================================================
-- Sunborn Authoritative PostgreSQL System-of-Record Schema Baseline (DB-001)
-- Architectural Invariant: Non-Accelerator Authority (PostgreSQL is Authoritative)
-- ==============================================================================

-- 1. Multi-Tenancy & Applications
CREATE TABLE IF NOT EXISTS tenants (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    rate_limit_rps INTEGER NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS applications (
    id VARCHAR(64) NOT NULL,
    tenant_id VARCHAR(64) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    api_key_hash VARCHAR(128) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);

-- 2. Durable Cache Metadata
CREATE TABLE IF NOT EXISTS cache_entries (
    key VARCHAR(255) PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    application_id VARCHAR(64) NOT NULL,
    model VARCHAR(128) NOT NULL,
    policy_version VARCHAR(32) NOT NULL,
    hash VARCHAR(64) NOT NULL,
    prompt_canonical TEXT NOT NULL,
    content TEXT NOT NULL,
    hit_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cache_tenant_app ON cache_entries(tenant_id, application_id);
CREATE INDEX IF NOT EXISTS idx_cache_expires_at ON cache_entries(expires_at);

-- 3. Knowledge Sources & Document Version Graph
CREATE TABLE IF NOT EXISTS knowledge_sources (
    id UUID PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    uri TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS document_versions (
    id UUID PRIMARY KEY,
    source_id UUID NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    version_tag VARCHAR(64) NOT NULL,
    content_hash VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS dependency_edges (
    source_id UUID NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    cache_key VARCHAR(255) NOT NULL REFERENCES cache_entries(key) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (source_id, cache_key)
);

-- 4. Invalidation Outbox Queue
CREATE TABLE IF NOT EXISTS invalidation_outbox (
    id BIGSERIAL PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id VARCHAR(255) NOT NULL,
    action VARCHAR(32) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_outbox_status ON invalidation_outbox(status);

-- 5. Decision & Audit Records
CREATE TABLE IF NOT EXISTS decision_records (
    request_id UUID PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    application_id VARCHAR(64) NOT NULL,
    action VARCHAR(32) NOT NULL,
    cache_path VARCHAR(32) NOT NULL,
    confidence REAL NOT NULL,
    risk_score REAL NOT NULL,
    reason_code VARCHAR(64) NOT NULL,
    latency_saved_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_decision_tenant ON decision_records(tenant_id, created_at);
