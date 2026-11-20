-- Migration 0011: Non-negativity invariants for the micro-unit ledger
--
-- 0010 introduced BIGINT micro-unit columns but left them unconstrained. Every
-- application path treats a spend amount as a magnitude, so a negative value
-- can only arise from a bug: a sign flip, a double release, or a manual insert.
-- Such a row silently *credits* the tenant, which is exactly the class of error
-- that reconciliation cannot detect after the fact.
--
-- The constraints are added NOT VALID and then validated in the same
-- transaction-safe sequence:
--   * NOT VALID takes only a brief ACCESS EXCLUSIVE lock to install the
--     constraint metadata and does not scan the table, so a large production
--     ledger is not blocked.
--   * VALIDATE CONSTRAINT scans with SHARE UPDATE EXCLUSIVE, which does not
--     block reads or writes, and promotes the constraint to fully enforced.
--
-- Fully idempotent: checks pg_constraint before adding.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'spend_reservations_amount_micro_units_non_negative') THEN
        ALTER TABLE spend_reservations
            ADD CONSTRAINT spend_reservations_amount_micro_units_non_negative
            CHECK (amount_micro_units >= 0) NOT VALID;
        ALTER TABLE spend_reservations
            VALIDATE CONSTRAINT spend_reservations_amount_micro_units_non_negative;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'spend_ledger_entries_amount_micro_units_non_negative') THEN
        ALTER TABLE spend_ledger_entries
            ADD CONSTRAINT spend_ledger_entries_amount_micro_units_non_negative
            CHECK (amount_micro_units >= 0) NOT VALID;
        ALTER TABLE spend_ledger_entries
            VALIDATE CONSTRAINT spend_ledger_entries_amount_micro_units_non_negative;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'spend_tenant_ceilings_ceiling_micro_units_non_negative') THEN
        ALTER TABLE spend_tenant_ceilings
            ADD CONSTRAINT spend_tenant_ceilings_ceiling_micro_units_non_negative
            CHECK (ceiling_micro_units >= 0) NOT VALID;
        ALTER TABLE spend_tenant_ceilings
            VALIDATE CONSTRAINT spend_tenant_ceilings_ceiling_micro_units_non_negative;
    END IF;
END $$;
