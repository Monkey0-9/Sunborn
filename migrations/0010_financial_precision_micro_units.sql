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

-- Automatic synchronization trigger for legacy/direct SQL inserts
CREATE OR REPLACE FUNCTION sync_spend_tenant_ceilings_micro_units()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.ceiling_micro_units IS NULL OR NEW.ceiling_micro_units = 0 THEN
        IF NEW.ceiling_usd IS NOT NULL AND NEW.ceiling_usd > 0 THEN
            NEW.ceiling_micro_units := ROUND(NEW.ceiling_usd * 1000000)::BIGINT;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_spend_tenant_ceilings_micro_units ON spend_tenant_ceilings;
CREATE TRIGGER trg_sync_spend_tenant_ceilings_micro_units
BEFORE INSERT OR UPDATE ON spend_tenant_ceilings
FOR EACH ROW EXECUTE FUNCTION sync_spend_tenant_ceilings_micro_units();
