-- P1.13 -- Migration 0004 up: sites (P1.12 §4.3) with RLS.
--
-- Composite FK to portfolios(portfolio_id, tenant_id) keeps the
-- denormalized tenant_id provably consistent (P1.12 §4.3). `timezone` is
-- the IANA site timezone; DST 23/25-hour days are a consumer rule, never
-- normalised away (P0.09 §3). No geometry in Phase 1 (location is a
-- descriptive jsonb). grid_model_reference and edge_identity are NULLable
-- (grid models are Phase 4; no edge component exists in Phase 1).
--
-- RECORDED DEVIATION from P1.12 §4.3: `name` and `description` columns are
-- added for the P1.07 SiteRepository port round-trip of the P1.06 entity.

CREATE TABLE sites (
    site_id             uuid        PRIMARY KEY DEFAULT uuidv7(),
    portfolio_id        uuid        NOT NULL,
    tenant_id           uuid        NOT NULL REFERENCES tenants (tenant_id),
    name                text        NOT NULL CHECK (length(btrim(name)) > 0),
    description         text        NULL,
    location            jsonb       NOT NULL DEFAULT '{}',
    timezone            text        NOT NULL CHECK (length(btrim(timezone)) > 0),
    pcc                 jsonb       NOT NULL DEFAULT '{}',
    grid_model_reference text       NULL,
    edge_identity       text        NULL,
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT sites_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    -- Superset of the PK; the composite-FK target for assets and asset_state.
    CONSTRAINT sites_id_tenant_key UNIQUE (site_id, tenant_id)
);

ALTER TABLE sites OWNER TO flexcore_migrator;

REVOKE ALL ON sites FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON sites TO flexcore_app;

ALTER TABLE sites ENABLE ROW LEVEL SECURITY;
ALTER TABLE sites FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON sites
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
