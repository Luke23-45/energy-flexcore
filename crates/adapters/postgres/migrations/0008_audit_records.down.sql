-- P1.13 -- Migration 0008 down.

DROP TRIGGER IF EXISTS audit_records_append_only ON audit_records;
DROP FUNCTION IF EXISTS audit_records_append_only_guard();
DROP TABLE IF EXISTS audit_records;
