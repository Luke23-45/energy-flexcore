-- P2.31 -- Migration 0012 down: remove the correction-version surface
--          (reverses 0012 up exactly). Multiple statements, ';' separated
--          (the integration-test down replay executes each fragment
--          individually).
--
-- DATA LOSS, recorded honestly: versioned corrections (telemetry_version
-- >= 2) are IRREVERSIBLY removed by this down migration — a correction
-- that is dropped cannot be recovered, and the original row is all that
-- remains. The original rows themselves are untouched. Rolling back to the
-- pre-P2.31 PK shape requires their removal (the 4-leg PK cannot hold two
-- rows of one identity).
--
-- RLS note: the table was created with ENABLE + FORCE ROW LEVEL SECURITY
-- (0010), so even the owning role is subject to the tenant policy and the
-- DELETE below would silently match zero rows from a migration connection
-- (no flexcore.tenant_id set — the policy fails closed). The FORCE flag is
-- lifted for the duration of this migration and restored immediately
-- after; the migration connection is the migrator/admin path, which is
-- exactly the recorded privileged path for maintenance DDL (P2.20).

ALTER TABLE telemetry NO FORCE ROW LEVEL SECURITY;
DELETE FROM telemetry WHERE telemetry_version > 1;
ALTER TABLE telemetry FORCE ROW LEVEL SECURITY;

ALTER TABLE telemetry DROP CONSTRAINT telemetry_pkey;
ALTER TABLE telemetry
    ADD CONSTRAINT telemetry_pkey
    PRIMARY KEY (asset_id, event_time, source_sequence, quantity);

ALTER TABLE telemetry DROP CONSTRAINT telemetry_correction_metadata;
ALTER TABLE telemetry DROP CONSTRAINT telemetry_version_positive;

ALTER TABLE telemetry
    DROP COLUMN corrected_at,
    DROP COLUMN corrected_by,
    DROP COLUMN correction_reason,
    DROP COLUMN correction_of,
    DROP COLUMN telemetry_version;
