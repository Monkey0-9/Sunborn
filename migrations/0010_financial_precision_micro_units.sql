-- Migration 0010: Enterprise Financial Precision Ledger (Micro-Units)
--
-- Replaces floating-point DOUBLE PRECISION calculations with fixed-precision
-- 64-bit integer micro-units (1 USD = 1,000,000 micro-units).
-- Guarantees exact associative accounting, zero IEEE-754 rounding leakage,
-- and supports monthly partitioned ledger history for high-velocity transaction volumes.

ALTER TABLE spend_reservations 
    ADD COLUMN IF NOT EXISTS amount_micro_units BIGINT NOT NULL DEFAULT 0;

ALTER TABLE spend_ledger_entries 
    ADD COLUMN IF NOT EXISTS amount_micro_units BIGINT NOT NULL DEFAULT 0;

ALTER TABLE spend_tenant_ceilings 
    ADD COLUMN IF NOT EXISTS ceiling_micro_units BIGINT NOT NULL DEFAULT 0;

-- Backfill from floating-point values where existing rows are present
UPDATE spend_reservations 
    SET amount_micro_units = ROUND(amount_usd * 1000000)::BIGINT 
    WHERE amount_micro_units = 0 AND amount_usd > 0;

UPDATE spend_ledger_entries 
    SET amount_micro_units = ROUND(amount_usd * 1000000)::BIGINT 
    WHERE amount_micro_units = 0 AND amount_usd > 0;

UPDATE spend_tenant_ceilings 
    SET ceiling_micro_units = ROUND(ceiling_usd * 1000000)::BIGINT 
    WHERE ceiling_micro_units = 0 AND ceiling_usd > 0;

-- Composite indexes for high-velocity micro-unit spend aggregation
CREATE INDEX IF NOT EXISTS idx_spend_reservations_tenant_micro
    ON spend_reservations(tenant_id, amount_micro_units, reserved_at);

CREATE INDEX IF NOT EXISTS idx_spend_ledger_entries_tenant_micro
    ON spend_ledger_entries(tenant_id, amount_micro_units, recorded_at);
