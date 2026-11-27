-- Back path for 0013_tenant_tier_defaults.sql.
--
-- Dropping these columns discards the applied tier defaults, so run this only
-- when rolling back to a binary that predates tier-aware provisioning. The
-- tenant's `tier` label and quota rows (migration 0008) are intentionally left in
-- place: they are readable by the older binary and removing them would change
-- admission behaviour rather than just the schema shape.

BEGIN;

DROP INDEX IF EXISTS idx_tenants_invited_by_created;

ALTER TABLE tenants
    DROP COLUMN IF EXISTS default_allowed_models,
    DROP COLUMN IF EXISTS invited_by,
    DROP COLUMN IF EXISTS identity_verified,
    DROP COLUMN IF EXISTS support_level,
    DROP COLUMN IF EXISTS retention_days;

COMMIT;
