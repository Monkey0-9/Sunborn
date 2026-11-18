-- Migration 0008: Cluster-wide tenant quotas and free-pool admission
--
-- The process-local rate limiter bounds velocity, not monthly volume, and each
-- replica owns its own counters. These tables make daily/monthly request
-- admission and the aggregate free-pool ceiling atomic across replicas.
--
-- Enrollment is explicit: a tenant is quota-enforced only when it has a row in
-- tenant_quota_limits. The free pool is enforced only when free_pool_limits has
-- its singleton row. Both default to absent, so existing deployments are
-- unchanged until an operator enrolls a tenant.

ALTER TABLE tenants ADD COLUMN IF NOT EXISTS tier VARCHAR(32) NOT NULL DEFAULT 'standard';

CREATE TABLE IF NOT EXISTS tenant_quota_limits (
    tenant_id VARCHAR(128) PRIMARY KEY,
    tier VARCHAR(32) NOT NULL DEFAULT 'free',
    daily_requests BIGINT NOT NULL CHECK (daily_requests >= 0),
    monthly_requests BIGINT NOT NULL CHECK (monthly_requests >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tenant_quota_usage (
    tenant_id VARCHAR(128) NOT NULL,
    window_kind VARCHAR(8) NOT NULL CHECK (window_kind IN ('day', 'month')),
    window_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0 CHECK (requests >= 0),
    PRIMARY KEY (tenant_id, window_kind, window_start)
);

CREATE INDEX IF NOT EXISTS idx_tenant_quota_usage_window
    ON tenant_quota_usage(window_start);

CREATE TABLE IF NOT EXISTS free_pool_limits (
    pool_id SMALLINT PRIMARY KEY CHECK (pool_id = 1),
    daily_requests BIGINT NOT NULL CHECK (daily_requests >= 0),
    monthly_requests BIGINT NOT NULL CHECK (monthly_requests >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS free_pool_usage (
    window_kind VARCHAR(8) NOT NULL CHECK (window_kind IN ('day', 'month')),
    window_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0 CHECK (requests >= 0),
    PRIMARY KEY (window_kind, window_start)
);
