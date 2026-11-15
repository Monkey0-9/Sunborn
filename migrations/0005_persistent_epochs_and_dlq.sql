-- Migration 0005: Persistent Epoch Authority & Database Outbox DLQ
-- Enforces distributed cache monotonic ordering and durable outbox dead-letter queue.

-- 1. Persistent Source Monotonic Epochs
CREATE TABLE IF NOT EXISTS tenant_source_epochs (
    source_id UUID PRIMARY KEY,
    current_epoch BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Transactional Outbox Dead-Letter Queue (DLQ)
CREATE TABLE IF NOT EXISTS outbox_dlq (
    id BIGSERIAL PRIMARY KEY,
    outbox_id BIGINT,
    tenant_id VARCHAR(128) NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id VARCHAR(255) NOT NULL,
    payload JSONB NOT NULL,
    error_message TEXT NOT NULL,
    retry_count INTEGER NOT NULL,
    failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_outbox_dlq_failed_at ON outbox_dlq(failed_at DESC);
