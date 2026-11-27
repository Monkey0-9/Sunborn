-- Migration 0013: tier defaults applied at provisioning
--
-- Migration 0008 added `tenants.tier` and the quota/pool tables, but a tier was
-- only a label: the model allowlist, retention window, and support level stayed
-- manual, and nothing recorded who invited a tenant. These columns let
-- provisioning apply one policy per tier and let the invitation cap be enforced
-- from durable state instead of an operator's memory.
--
-- Nullable/defaulted columns only, so this is safe on a live table: no rewrite,
-- no backfill, and every existing tenant keeps its current behaviour (`*`
-- models, 30-day retention) until an operator re-applies its tier.

ALTER TABLE tenants ADD COLUMN IF NOT EXISTS retention_days INTEGER NOT NULL DEFAULT 30;
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS support_level VARCHAR(32) NOT NULL DEFAULT 'community';
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS identity_verified BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS invited_by VARCHAR(128);
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS default_allowed_models JSONB NOT NULL DEFAULT '["*"]'::jsonb;

-- Backs the per-inviter cap lookup; partial so it stays small (most tenants are
-- not invitation-provisioned).
CREATE INDEX IF NOT EXISTS idx_tenants_invited_by_created
    ON tenants(invited_by, created_at)
    WHERE invited_by IS NOT NULL;

COMMENT ON COLUMN tenants.retention_days IS
    'Data retention window applied from the tenant tier policy at provisioning.';
COMMENT ON COLUMN tenants.support_level IS
    'Support commitment from the tenant tier policy; not a contractual SLA by itself.';
COMMENT ON COLUMN tenants.identity_verified IS
    'True once identity verification completed; required by identity-gated tiers.';
COMMENT ON COLUMN tenants.invited_by IS
    'Tenant or operator that provisioned this tenant; used for the invitation cap.';
COMMENT ON COLUMN tenants.default_allowed_models IS
    'Tier model allowlist applied to new credentials that do not name their own.';

-- Back path (see migrations/down/0013_tenant_tier_defaults.down.sql):
--   DROP INDEX IF EXISTS idx_tenants_invited_by_created;
--   ALTER TABLE tenants
--       DROP COLUMN IF EXISTS default_allowed_models,
--       DROP COLUMN IF EXISTS invited_by,
--       DROP COLUMN IF EXISTS identity_verified,
--       DROP COLUMN IF EXISTS support_level,
--       DROP COLUMN IF EXISTS retention_days;
