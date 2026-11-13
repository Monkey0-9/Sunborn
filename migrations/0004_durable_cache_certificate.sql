-- Migration: 0004_durable_cache_certificate.sql
-- Description: Adds certificate_json column to durable_cache_entries for cryptographic provenance preservation during recovery (P0-03)

ALTER TABLE durable_cache_entries 
ADD COLUMN IF NOT EXISTS certificate_json TEXT;

COMMENT ON COLUMN durable_cache_entries.certificate_json IS 'Canonical JSON representation of the cryptographically signed ReuseCertificate and its source lineage snapshots';
