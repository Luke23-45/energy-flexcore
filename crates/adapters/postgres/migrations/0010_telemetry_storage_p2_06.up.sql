-- P2.06 -- Migration 0010 up: the partitioned telemetry table, indexes, RLS
--          and roles (P2.05 D-P1..D-P7), and the telemetry quarantine table
--          (P2.07 §3). Implements the P2.05 design exactly.
--
-- The Phase 1 registry tables, the telemetry_quality enum (P1.12 §3.2) and
-- the outbox are the base and are NOT recreated (P2.06 step 1). Everything
-- below is new Phase 2 surface.
--
-- D-P7: TimescaleDB is NOT used (revisit trigger: measured volume; P2.05
-- §10). The partitioned shape is the designed form for the volume
-- trajectory, not a measured Phase 2 win (P2.05 §4.2 honesty note).
--
-- D-P5: NO retention value, no TTL trigger, and no scheduled-delete rule
-- appear anywhere in this DDL. Retention is deployment configuration per
-- data class (§16.3); execution is detach-then-drop (or archive-first) of
-- whole monthly partitions, the P2.20 worker's decision, under the
-- migrator/maintenance role with an audit record per action.
--
-- Partition management: PostgreSQL does not auto-create partitions. The
-- P2.20 maintenance job pre-creates monthly partitions ahead of the ingest
-- frontier (P2.05 §7 item 1, the recorded default). No partition is created
-- here: a time-dependent partition in a migration would be a hidden
-- wall-clock behaviour, and an empty partitioned table keeps the
-- out-of-range error path (P2.05 §7 item 2) testable from the first insert.

CREATE TYPE reactive_sign_convention AS ENUM ('INJECTION_POSITIVE', 'CONSUMPTION_POSITIVE');

CREATE TYPE clock_quality AS ENUM ('SYNCHRONIZED', 'UNSYNCHRONIZED', 'UNKNOWN');

-- D-P6: the two distinct runtime roles for telemetry. Guarded DO block,
-- NOLOGIN, no credentials in a migration (the 0001 recorded deviation
-- pattern); granting LOGIN + credentials is the deployment/test-bootstrap
-- step. Neither runtime role owns any table; both are subject to the RLS
-- policy via ENABLE + FORCE.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flexcore_ingest') THEN
        CREATE ROLE flexcore_ingest NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flexcore_query') THEN
        CREATE ROLE flexcore_query NOLOGIN;
    END IF;
END
$$;

-- Schema access for the telemetry runtime roles (PG 15+ restricts the
-- public schema; the migrator and flexcore_app were granted in 0001; the
-- table grants below are useless without schema usage). The migrator role
-- additionally executes the partition-management DDL (P2.20 monthly
-- pre-creation ahead of the ingest frontier; retention detach/drop), so it
-- needs schema CREATE, which PG 15+ no longer grants to PUBLIC.
GRANT USAGE ON SCHEMA public TO flexcore_ingest, flexcore_query;
GRANT CREATE ON SCHEMA public TO flexcore_migrator;

-- D-P1: one partitioned table for all quantities, with the quantity column
-- (P2.04 D-T2: one row per (asset_id, event_time, quantity) within a
-- source sample). D-P2: PARTITION BY RANGE (event_time), monthly partitions
-- named telemetry_yYYYYmMM with bounds [first day of month, first day of
-- next month). D-P3: the PK includes the partition key — PostgreSQL rejects
-- a PK/unique constraint on a partitioned table that omits it.
--
-- site_id/portfolio_id are deliberately absent (P2.04 D-T1: resolved by
-- JOIN through assets -> sites -> portfolios; only tenant_id is
-- denormalized, proven consistent by the composite FK to
-- assets(asset_id, tenant_id)). tenant_id is NOT in the PK (P2.05 §5
-- item 5: asset_id is globally unique; the composite FK proves tenant
-- consistency; reads are scoped by RLS).
CREATE TABLE telemetry (
    telemetry_id        uuid        NOT NULL DEFAULT uuidv7(),
    tenant_id           uuid        NOT NULL,
    asset_id            uuid        NOT NULL,
    -- Frozen Phase 2 quantity set (P2.04 §4); extension is an explicit
    -- migration, never a silent widening.
    quantity            text        NOT NULL
                        CHECK (quantity IN
                            ('active_power', 'reactive_power', 'soc', 'energy',
                             'availability', 'operating_mode', 'health',
                             'connection_state', 'baseline_reference')),
    -- Partition key; UTC internal (P0.09 §2); when the measurement occurred.
    event_time          timestamptz NOT NULL,
    -- When FlexCore received it; distinct column, never merged with
    -- event_time (P0.09 §1). ingest_time >= event_time is re-checked at
    -- the repository boundary (P1.05 TimeRecord::validate semantics).
    ingest_time         timestamptz NOT NULL,
    source_sequence     bigint      NOT NULL CHECK (source_sequence >= 0),
    source_id           text        NOT NULL CHECK (length(btrim(source_id)) > 0),
    -- Exactly one of the six P0.08 states; a seventh value is rejected by
    -- the enum (D-S4). The Phase 1 enum is reused, not recreated.
    quality             telemetry_quality NOT NULL,
    -- Original richer source state as diagnostic metadata (P0.08 §3);
    -- never a seventh canonical state.
    source_quality_code text        NULL,
    value_num           double precision NULL
                        CHECK (value_num IS NULL OR
                               (value_num <> 'NaN'::float8 AND
                                value_num <> 'Infinity'::float8 AND
                                value_num <> '-Infinity'::float8)),
    value_text          text        NULL
                        CHECK (value_text IS NULL OR length(btrim(value_text)) > 0),
    value_bool          boolean     NULL,
    -- Canonical unit vocabulary (P0.07 §1); '1' = dimensionless (SOC
    -- fraction, boolean and state values). No kW/kWh or other scaled unit
    -- exists anywhere (P0.07 §5).
    unit                text        NOT NULL
                        CHECK (unit IN ('MW', 'MVAr', 'MVA', 'MWh', '1')),
    -- D-T4: the declared convention travels on the record; NOT NULL exactly
    -- when quantity = 'reactive_power'; no default exists (P0.07 §3).
    q_sign_convention   reactive_sign_convention NULL,
    -- D-T5: recorded synchronisation status with explicit UNKNOWN; never
    -- assumed (P0.09 §4).
    clock_quality       clock_quality NOT NULL,

    -- D-P3: PK includes the partition key; also the storage-level form of
    -- the P2.09 dedup identity (asset_id, event_time, source_sequence) +
    -- the per-quantity row grain (P2.05 §5).
    CONSTRAINT telemetry_pkey
        PRIMARY KEY (asset_id, event_time, source_sequence, quantity),
    -- Composite FK proves the denormalized tenant matches the asset's
    -- tenant (P1.12 §4.5 pattern; P2.04 D-T1).
    CONSTRAINT telemetry_asset_tenant_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id),
    CONSTRAINT telemetry_q_sign_convention_conditional
        CHECK ((quantity = 'reactive_power') = (q_sign_convention IS NOT NULL)),
    -- Value-field totality (P2.04 §3): exactly one value field non-null,
    -- except all three NULL exactly when quality = 'MISSING' (a gap marker
    -- has no value).
    CONSTRAINT telemetry_value_totality
        CHECK (
            (quality = 'MISSING'
             AND value_num IS NULL AND value_text IS NULL AND value_bool IS NULL)
            OR
            (quality <> 'MISSING'
             AND ((value_num IS NOT NULL)::int
                  + (value_text IS NOT NULL)::int
                  + (value_bool IS NOT NULL)::int) = 1)
        ),
    -- The (quantity, value kind, unit) triples per the P2.04 §4 mapping
    -- (that table is the specification of this constraint's contents).
    CONSTRAINT telemetry_quantity_value_unit
        CHECK (
            (quantity = 'active_power' AND value_num IS NOT NULL
                AND value_text IS NULL AND value_bool IS NULL AND unit = 'MW')
            OR (quantity = 'reactive_power' AND value_num IS NOT NULL
                AND value_text IS NULL AND value_bool IS NULL AND unit = 'MVAr')
            OR (quantity = 'soc' AND value_num IS NOT NULL
                AND value_text IS NULL AND value_bool IS NULL AND unit = '1')
            OR (quantity = 'energy' AND value_num IS NOT NULL
                AND value_text IS NULL AND value_bool IS NULL AND unit = 'MWh')
            OR (quantity = 'availability' AND value_bool IS NOT NULL
                AND value_num IS NULL AND value_text IS NULL AND unit = '1')
            OR (quantity = 'operating_mode' AND value_text IS NOT NULL
                AND value_num IS NULL AND value_bool IS NULL AND unit = '1')
            OR (quantity = 'health' AND value_text IS NOT NULL
                AND value_num IS NULL AND value_bool IS NULL AND unit = '1')
            OR (quantity = 'connection_state' AND value_bool IS NOT NULL
                AND value_num IS NULL AND value_text IS NULL AND unit = '1')
            OR (quantity = 'baseline_reference' AND value_num IS NOT NULL
                AND value_text IS NULL AND value_bool IS NULL AND unit = 'MW')
            OR (quality = 'MISSING' AND value_num IS NULL
                AND value_text IS NULL AND value_bool IS NULL)
        ),
    -- SOC is a fraction in [0,1] (P0.07 §4; D-S5 forward constraint).
    CONSTRAINT telemetry_soc_fraction_range
        CHECK (quantity <> 'soc' OR (value_num >= 0 AND value_num <= 1))
) PARTITION BY RANGE (event_time);

-- Indexes (P2.05 §6), created on the partitioned parent (PostgreSQL
-- propagates them to existing and future partitions). D-P4: no
-- latest-value partial index and no materialized current-state table in
-- Phase 2 — these composite DESC indexes serve the P2.18/P2.19 query
-- patterns with partition pruning.
CREATE INDEX telemetry_asset_time_idx
    ON telemetry (asset_id, event_time DESC);
CREATE INDEX telemetry_tenant_asset_time_idx
    ON telemetry (tenant_id, asset_id, event_time DESC);
CREATE INDEX telemetry_asset_quantity_time_idx
    ON telemetry (asset_id, quantity, event_time DESC);

-- Owner + grants (D-P6). flexcore_migrator owns; the runtime roles hold
-- exactly the privileges of their path; neither holds DELETE, TRUNCATE, or
-- DDL (retention executes via the P2.20 approved-operation path). RLS,
-- grants, and the table are created in the same migration, so the table is
-- never momentarily without its policy (P1.12 §7 ordering guarantee).
ALTER TABLE telemetry OWNER TO flexcore_migrator;

REVOKE ALL ON telemetry FROM PUBLIC;
GRANT INSERT, SELECT ON telemetry TO flexcore_ingest;
GRANT SELECT ON telemetry TO flexcore_query;

ALTER TABLE telemetry ENABLE ROW LEVEL SECURITY;
ALTER TABLE telemetry FORCE ROW LEVEL SECURITY;

-- One policy, exactly the P1.12 §5.1 text (P2.05 §9 item 2). An
-- unset/empty tenant setting yields NULL -> zero rows and no writes:
-- fails closed, never open.
CREATE POLICY tenant_isolation ON telemetry
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

-- The telemetry quarantine (P2.07 §3): the evidence trail for every
-- rejected record. A failed record is quarantined with a reason — never
-- silently dropped, never silently stored as GOOD (P2.07 §2). Records are
-- written under the authenticated tenant context of the source (the source
-- identity is tenant-scoped, P2.22), so tenant_id is NOT NULL and the RLS
-- policy scopes every row. asset_id/event_time/source_sequence are nullable
-- because the failing record may be unresolvable at the failed stage
-- (e.g. a schema-validation failure before the asset was resolved); there
-- is deliberately NO FK on asset_id — an unresolvable asset id is itself a
-- quarantine reason. raw_payload is the stage evidence (JSONB). No
-- retention value appears here (D-P5: retention of the quarantine class is
-- configuration, P0.16 §4).
CREATE TABLE telemetry_quarantine (
    quarantine_id   uuid        PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid        NOT NULL REFERENCES tenants (tenant_id),
    source_id       text        NULL,
    source_sequence bigint      NULL,
    asset_id        uuid        NULL,
    event_time      timestamptz NULL,
    -- Pipeline stage + the rule that rejected the record (P2.07 §3:
    -- "failure reason (stage + rule)").
    stage           text        NOT NULL CHECK (length(btrim(stage)) > 0),
    reason          text        NOT NULL CHECK (length(btrim(reason)) > 0),
    raw_payload     jsonb       NOT NULL,
    quarantined_at  timestamptz NOT NULL DEFAULT now(),
    attempt_count   integer     NOT NULL DEFAULT 0 CHECK (attempt_count >= 0)
);

CREATE INDEX telemetry_quarantine_tenant_idx
    ON telemetry_quarantine (tenant_id, quarantined_at DESC);

ALTER TABLE telemetry_quarantine OWNER TO flexcore_migrator;

REVOKE ALL ON telemetry_quarantine FROM PUBLIC;
GRANT INSERT, SELECT ON telemetry_quarantine TO flexcore_ingest;
GRANT SELECT ON telemetry_quarantine TO flexcore_query;

ALTER TABLE telemetry_quarantine ENABLE ROW LEVEL SECURITY;
ALTER TABLE telemetry_quarantine FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON telemetry_quarantine
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
