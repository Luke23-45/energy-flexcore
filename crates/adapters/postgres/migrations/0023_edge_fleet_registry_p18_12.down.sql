-- P18.12 -- Migration 0023 down: remove the edge fleet device-state
--          journal whole. EXPAND-ONLY reversal (the P11.18 policy): the
--          up migration only CREATED objects, so the down migration only
--          DROPS them — no column, table, constraint, or grant belonging
--          to any earlier migration is touched. Dropping the journal
--          discloses fleet-state OBSERVATIONS (firmware/cohort/attestation/
--          health/eSIM) but never identity evidence: the 0018 registry and
--          enrollment journal remain untouched. (No fleet exists in this
--          program — the table has never held a row; the drop is recorded,
--          never exercised. WRITTEN-UNRUN.)

DROP POLICY tenant_isolation ON edge_fleet_device_state;
DROP TABLE edge_fleet_device_state;
DROP FUNCTION edge_fleet_device_state_append_only_guard();
