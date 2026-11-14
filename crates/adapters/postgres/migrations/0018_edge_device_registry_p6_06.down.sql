-- P6.06 -- Migration 0018 down: drop the enrollment journal and the edge
--          device-identity registry (reverse order of the up migration).

DROP POLICY IF EXISTS tenant_isolation ON edge_enrollment_journal;
DROP TRIGGER IF EXISTS edge_enrollment_journal_append_only ON edge_enrollment_journal;
DROP FUNCTION IF EXISTS edge_enrollment_journal_append_only_guard();
DROP TABLE IF EXISTS edge_enrollment_journal;

DROP POLICY IF EXISTS tenant_isolation ON edge_device_registry;
DROP TRIGGER IF EXISTS edge_device_registry_append_only ON edge_device_registry;
DROP FUNCTION IF EXISTS edge_device_registry_append_only_guard();
DROP INDEX IF EXISTS edge_device_registry_one_active_term;
DROP INDEX IF EXISTS edge_device_registry_current_idx;
DROP TABLE IF EXISTS edge_device_registry;
