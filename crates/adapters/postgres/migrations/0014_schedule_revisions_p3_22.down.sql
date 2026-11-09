-- P3.22/P3.23 -- Migration 0014 down: remove the schedule-revision chain
--          (P3.22), the comparison records (P3.23), and the unique anchor
--          constraint added to schedule_results.

DROP TABLE IF EXISTS schedule_comparisons;
DROP TABLE IF EXISTS schedule_revisions;
DROP FUNCTION IF EXISTS schedule_revisions_guard_update();

ALTER TABLE schedule_results
    DROP CONSTRAINT IF EXISTS schedule_results_candidate_tenant_key;
