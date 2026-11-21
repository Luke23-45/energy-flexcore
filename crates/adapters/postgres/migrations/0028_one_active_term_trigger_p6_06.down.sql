-- P6.06 -- Migration 0028 down: remove the one-ACTIVE-term trigger and
--          restore the partial unique index whole. EXPAND-ONLY reversal
--          (the P11.18 policy): the up migration only DROPPED one index
--          and CREATED one trigger + function, so the down migration only
--          drops the trigger + function and re-creates that index — no
--          column, table, constraint, or grant belonging to any earlier
--          migration is touched. NOTE: re-creating the index fails if the
--          table holds two ACTIVE rows for one device (rotation history
--          written while the trigger was in force) — the down replay runs
--          on an empty database in the gated suite, where the table holds
--          no rows; on a populated database, drop the later ACTIVE terms
--          first (an ops-runbook step, not an embedded-migration step).

DROP TRIGGER IF EXISTS edge_device_registry_one_active_term
    ON edge_device_registry;
DROP FUNCTION IF EXISTS edge_device_registry_one_active_term_guard();
CREATE UNIQUE INDEX edge_device_registry_one_active_term
    ON edge_device_registry (device_id, tenant_id)
    WHERE status = 'ACTIVE'::text;
