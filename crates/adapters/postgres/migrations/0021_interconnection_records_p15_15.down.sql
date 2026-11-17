-- P15.15 -- Migration 0021 down: drop the interconnection-records catalog
--           (index and table). Expand-only reverse of the up migration;
--           no other table is touched.

DROP INDEX IF EXISTS interconnection_records_device_model_idx;
DROP TABLE IF EXISTS interconnection_records;
