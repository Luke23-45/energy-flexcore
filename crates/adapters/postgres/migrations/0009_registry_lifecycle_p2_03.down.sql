-- P2.06 -- Migration 0009 down: remove the additive P2.03 lifecycle columns.
-- Single statement (the down replay in the integration test executes one
-- statement per ';' fragment; this file is one fragment).

ALTER TABLE assets
    DROP CONSTRAINT IF EXISTS assets_lifecycle_consistency,
    DROP COLUMN IF EXISTS deactivated_at,
    DROP COLUMN IF EXISTS deactivation_reason,
    DROP COLUMN IF EXISTS lifecycle_status;
