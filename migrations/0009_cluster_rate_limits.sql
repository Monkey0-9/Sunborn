-- Migration 0009: Cluster-wide per-tenant token buckets
--
-- The process-local bucket in the gateway multiplies by replica count. This
-- table stores one token bucket per quota-enrolled tenant so every replica
-- draws from the same RPS budget. Rows are locked during a reservation, which
-- serializes only requests for that tenant.
--
-- The table is intentionally not foreign-keyed to tenants: quota enrollment is
-- explicit and may precede tenant provisioning. Tenant deletion removes the
-- bucket explicitly in the cascade.

CREATE TABLE IF NOT EXISTS tenant_rate_limit_buckets (
    tenant_id VARCHAR(128) PRIMARY KEY,
    tokens DOUBLE PRECISION NOT NULL CHECK (tokens >= 0),
    last_refill TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tenant_rate_limit_buckets_last_refill
    ON tenant_rate_limit_buckets(last_refill);
