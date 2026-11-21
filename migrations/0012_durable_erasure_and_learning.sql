-- Migration 0012: Durable Tenant Erasure State Machine & Persistent Learning Feedback
--
-- Provides restart-safe, at-least-once durable state tracking for tenant offboarding,
-- and distributed cross-replica persistence for verified learning feedback events.

CREATE TABLE IF NOT EXISTS tenant_erasure_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id VARCHAR(128) NOT NULL,
    current_step VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'IN_PROGRESS',
    last_error TEXT,
    shred_duration_ns BIGINT,
    deleted_db_entries INT DEFAULT 0,
    deleted_exact_cache INT DEFAULT 0,
    deleted_vectors INT DEFAULT 0,
    deleted_execution_tools INT DEFAULT 0,
    deleted_learning_events INT DEFAULT 0,
    verified_integrity BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_erasure_tasks_tenant_status 
    ON tenant_erasure_tasks(tenant_id, status);

CREATE TABLE IF NOT EXISTS learning_feedback_events (
    event_id VARCHAR(128) PRIMARY KEY,
    tenant_id VARCHAR(128) NOT NULL,
    application_id VARCHAR(128) NOT NULL,
    cache_key TEXT NOT NULL,
    decision_type VARCHAR(64) NOT NULL,
    prompt_hash VARCHAR(128) NOT NULL,
    similarity_score DOUBLE PRECISION,
    conformal_risk_score DOUBLE PRECISION,
    tokens_saved INT NOT NULL DEFAULT 0,
    cost_saved_usd DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    verified_outcome VARCHAR(64) NOT NULL,
    verification_source VARCHAR(64) NOT NULL,
    hmac_signature VARCHAR(256) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_learning_events_tenant 
    ON learning_feedback_events(tenant_id, created_at DESC);
