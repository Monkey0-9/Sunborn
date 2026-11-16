-- Migration 0007: Distributed spend velocity ledger
--
-- SpendVelocityController was process-local, so N replicas each enforced their
-- own ceiling and a tenant could spend N x the configured budget. These tables
-- make reservation and settlement atomic across replicas.

CREATE TABLE IF NOT EXISTS spend_reservations (
    reservation_id UUID PRIMARY KEY,
    tenant_id VARCHAR(128) NOT NULL,
    amount_usd DOUBLE PRECISION NOT NULL CHECK (amount_usd >= 0),
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_spend_reservations_tenant
    ON spend_reservations(tenant_id, reserved_at);

CREATE TABLE IF NOT EXISTS spend_ledger_entries (
    id BIGSERIAL PRIMARY KEY,
    tenant_id VARCHAR(128) NOT NULL,
    reservation_id UUID NOT NULL,
    amount_usd DOUBLE PRECISION NOT NULL CHECK (amount_usd >= 0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_spend_ledger_entries_tenant
    ON spend_ledger_entries(tenant_id, recorded_at);

-- A reservation may be settled at most once.
CREATE UNIQUE INDEX IF NOT EXISTS idx_spend_ledger_entries_reservation
    ON spend_ledger_entries(reservation_id);

CREATE TABLE IF NOT EXISTS spend_tenant_ceilings (
    tenant_id VARCHAR(128) PRIMARY KEY,
    ceiling_usd DOUBLE PRECISION NOT NULL CHECK (ceiling_usd >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
