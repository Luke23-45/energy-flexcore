-- P1.13 -- Migration 0005 up: assets (P1.12 §4.4) with RLS.
--
-- asset_type CHECK carries the §2.2 first-class set exactly; backup
-- generation is deliberately absent (P0.10 §2). Canonical units at the
-- boundary: rated_power_mw / rated_energy_mwh as double precision (P0.07).
-- NO SOC column here (D-S5): SOC is per-asset state, not an attribute.
-- NO reactive-power sign column: the Q sign convention is declared per grid
-- model / device profile, never defaulted (P0.07).
--
-- RECORDED DEVIATION from P1.12 §4.4: `name` and `description` columns are
-- added for the P1.07 AssetRepository port round-trip of the P1.06 entity.

CREATE TABLE assets (
    asset_id           uuid        PRIMARY KEY DEFAULT uuidv7(),
    site_id            uuid        NOT NULL,
    tenant_id          uuid        NOT NULL REFERENCES tenants (tenant_id),
    name               text        NOT NULL CHECK (length(btrim(name)) > 0),
    description        text        NULL,
    asset_type         text        NOT NULL CHECK (asset_type IN
                           ('bess', 'pv_inverter', 'flexible_load',
                            'evse_ev_fleet', 'site_meter', 'wind',
                            'thermal_storage_heat_pump')),
    vendor_model       text        NULL,
    rated_power_mw     double precision NOT NULL CHECK (rated_power_mw >= 0),
    rated_energy_mwh   double precision NULL
                           CHECK (rated_energy_mwh IS NULL OR rated_energy_mwh >= 0),
    phase              text        NOT NULL,
    bus_reference      text        NULL,
    capability_profile jsonb       NOT NULL DEFAULT '{}',
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT assets_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    -- Superset of the PK; the composite-FK target for asset_state.
    CONSTRAINT assets_id_tenant_key UNIQUE (asset_id, tenant_id)
);

ALTER TABLE assets OWNER TO flexcore_migrator;

REVOKE ALL ON assets FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON assets TO flexcore_app;

ALTER TABLE assets ENABLE ROW LEVEL SECURITY;
ALTER TABLE assets FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON assets
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
