-- P4.10 -- Migration 0025 down: remove the tenant-listing index on
--          `dispatch_commands` whole. EXPAND-ONLY reversal (the P11.18
--          policy): the up migration only CREATED one index, so the down
--          migration only DROPS it — no column, table, constraint, or
--          grant belonging to any earlier migration is touched. Dropping
--          the index discloses no data (it is a derived access path, not
--          a record); the documented tenant-listing query pattern falls
--          back to the pre-0025 scan behavior it always had.

DROP INDEX dispatch_commands_tenant_idx;
