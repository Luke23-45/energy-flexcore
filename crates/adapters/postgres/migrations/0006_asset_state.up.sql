-- P1.13 -- Migration 0006 up: asset_state (P1.12 §4.5, D-S2) with RLS.
--
-- Current state, one row per asset, updated in place (§6.5); state history
-- is Phase 2 and is NOT created here. SOC is a fraction in [0,1] (D-S5
-- forward constraint; no percentage representation anywhere, P0.07).
-- event_time / ingest_time are distinct columns and are never merged or
-- substituted (P0.09 §1). quality is exactly one of the six P0.08 states
-- (D-S4); source-state mapping happens at the adapter, never as a new enum
-- value (P1.12 §3.2).
--
-- Phase 1 write convention (P1.12 §4.5): registration inserts the initial
-- row with event_time = ingest_time = registration time, quality = 'MISSING',
-- and NULL measurements.

CREATE TABLE asset_state (
    asset_id            uuid        PRIMARY KEY,
    tenant_id           uuid        NOT NULL REFERENCES tenants (tenant_id),
    soc                 double precision NULL
                        CHECK (soc IS NULL OR (soc >= 0 AND soc <= 1)),
    active_power_mw     double precision NULL, -- P > 0 = export at the PCC (P0.07)
    reactive_power_mvar double precision NULL, -- sign declared per grid model/device profile, never assumed
    availability        boolean     NULL,
    operating_mode      text        NULL,
    health              text        NULL,
    event_time          timestamptz NOT NULL,
    ingest_time         timestamptz NOT NULL DEFAULT now(),
    source_sequence     bigint      NULL,  -- P0.09 §1 "source sequence or version" (Phase 2)
    clock_quality       text        NULL,  -- P0.09 §1 clock-quality status (Phase 2 vocabulary)
    quality             telemetry_quality NOT NULL,
    CONSTRAINT asset_state_asset_tenant_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id)
);

ALTER TABLE asset_state OWNER TO flexcore_migrator;

REVOKE ALL ON asset_state FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON asset_state TO flexcore_app;

ALTER TABLE asset_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE asset_state FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON asset_state
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
