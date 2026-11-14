-- P4.30 -- Migration 0017 down: remove the feasibility-check evidence
--          records.

DROP TABLE IF EXISTS feasibility_check_records;
DROP FUNCTION IF EXISTS feasibility_check_records_append_only_guard();
