-- P18.12 -- Migration 0023 up: the edge FLEET device-state journal (the
--          0018 registry extension for phase 18: firmware state, cohort
--          membership, attestation state, last-seen health, eSIM profile
--          state). Report/phase trace: phase-18 plan P18.12; decision
--          record: docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/
--          records/25_p18_12_fleet_registry_hardening.md
--
-- WHY A NEW JOURNAL TABLE AND NOT COLUMNS ON edge_device_registry
-- (recorded here, not implicit):
-- - `edge_device_registry` (0018) is APPEND-ONLY per identity term: its
--   UPDATE/DELETE trigger refuses unconditionally, flexcore_app holds
--   INSERT + SELECT only, and a lifecycle change appends a NEW term row.
--   Firmware state, attestation state, and last-seen health are
--   HIGH-FREQUENCY observations — putting them on the registry would
--   demand either an UPDATE grant (forbidden by the 0018 design) or a new
--   registry term per telemetry-like observation (which would churn the
--   one-ACTIVE-term invariant and the identity evidence). The expansion is
--   therefore a COMPANION STATE JOURNAL keyed to the same (device_id,
--   tenant_id) identity scope: a device's CURRENT fleet state is its
--   latest row per (device_id, state_kind), exactly the event-sourced
--   shape 0018 uses for the identity itself.
-- - Cohort membership is a state_kind here, so the phase-18 rollout
--   orchestrator (P18.06) consumes a device's current cohort as the latest
--   COHORT_MEMBERSHIP row — the read pattern is documented below and in
--   the decision record; the journal schema is the contract, the
--   orchestrator never redefines it.
--
-- EXPAND-ONLY (the P11.18 policy, crates/adapters/postgres/
-- MIGRATION_POLICY.md): every statement here is a CREATE on a NEW table —
-- no ALTER touches an existing table, no constraint scans a populated
-- table, no index is built on any lifecycle table. The 0018 registry and
-- journal stay byte-identical. Stage 2/3/4 (dual-write/backfill, switch,
-- contract) do not apply: nothing is replaced, nothing is removed — the
-- down migration drops the new table whole.
--
-- What this migration does NOT contain: NO key material, NO certificate
-- columns, NO secrets, NO carrier-specific eSIM fields (no SM-DP+ address,
-- no activation code — P18.11 scope: carrier-agnostic data only [S-167]).
-- Value domains for `state_value` are CLOSED vocabularies enforced in the
-- adapter types (crates/edge-agent/src/fleet_registry.rs and esim.rs) and
-- recorded in the decision record; the SQL layer enforces the kind set and
-- non-emptiness, matching the 0018 journal's enforcement depth (its
-- event_type CHECK plus the REJECTED-shape rule; here every row is a
-- state observation, so no per-kind shape rule applies at this depth).

CREATE TABLE edge_fleet_device_state (
    event_id                uuid        NOT NULL,
    tenant_id               uuid        NOT NULL REFERENCES tenants (tenant_id),
    device_id               text        NOT NULL CHECK (length(btrim(device_id)) > 0),
    state_kind              text        NOT NULL CHECK (
                                state_kind = ANY (ARRAY[
                                    'FIRMWARE_STATE'::text,
                                    'COHORT_MEMBERSHIP'::text,
                                    'ATTESTATION_STATE'::text,
                                    'LAST_SEEN_HEALTH'::text,
                                    'ESIM_PROFILE_STATE'::text])),
    state_value             text        NOT NULL CHECK (length(btrim(state_value)) > 0),
    detail                  text        NULL,
    correlation_id          text        NOT NULL CHECK (length(btrim(correlation_id)) > 0),
    observed_at_unix_millis bigint      NOT NULL CHECK (observed_at_unix_millis > 0),
    recorded_at             timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT edge_fleet_device_state_pk
        PRIMARY KEY (event_id, tenant_id)
);

-- The current-state read pattern: a device's current value for one state
-- kind is its highest-`observed_at_unix_millis` row for that kind, e.g.
--
--   SELECT DISTINCT ON (device_id) device_id, state_value
--   FROM edge_fleet_device_state
--   WHERE tenant_id = $1 AND device_id = $2 AND state_kind = 'COHORT_MEMBERSHIP'
--   ORDER BY device_id, observed_at_unix_millis DESC;
--
-- This index (created in-transaction on the brand-new, empty table — the
-- lock-risk rules bind only populated tables) serves that walk.
CREATE INDEX edge_fleet_device_state_current_idx
    ON edge_fleet_device_state (tenant_id, device_id, state_kind, observed_at_unix_millis DESC);

-- Append-only defence in depth (the 0008/0018 layer-2 pattern): UPDATE and
-- DELETE raise unconditionally, including for the table owner. A state
-- change is a NEW row; nothing is ever edited in place.
CREATE FUNCTION edge_fleet_device_state_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'edge_fleet_device_state is append-only: UPDATE and DELETE are refused (P18.12, the 0018 pattern); a state change appends a NEW row';
END $$;

CREATE TRIGGER edge_fleet_device_state_append_only
    BEFORE UPDATE OR DELETE ON edge_fleet_device_state
    FOR EACH ROW
    EXECUTE FUNCTION edge_fleet_device_state_append_only_guard();

ALTER TABLE edge_fleet_device_state OWNER TO flexcore_migrator;
ALTER FUNCTION edge_fleet_device_state_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON edge_fleet_device_state FROM PUBLIC;
GRANT SELECT, INSERT ON edge_fleet_device_state TO flexcore_app;

ALTER TABLE edge_fleet_device_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE edge_fleet_device_state FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON edge_fleet_device_state
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
