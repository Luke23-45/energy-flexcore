-- P1.13 -- Migration 0003 up: portfolios (P1.12 §4.2) with RLS.
--
-- Tenant-owned; ENABLE + FORCE ROW LEVEL SECURITY with the P1.12 §5.1
-- tenant_isolation policy (unset/empty flexcore.tenant_id fails closed).
-- `aggregation_rules` stays a descriptor-only jsonb (P1.12 §4.2) — no
-- jurisdiction enum and no jurisdiction-specific code while D1 is open.
--
-- RECORDED DEVIATION from P1.12 §4.2: `name` and `description` columns are
-- added so the P1.07 PortfolioRepository port can round-trip the P1.06
-- domain entity (the port's find_by_id returns the full entity).

CREATE TABLE portfolios (
    portfolio_id      uuid        PRIMARY KEY DEFAULT uuidv7(),
    tenant_id         uuid        NOT NULL REFERENCES tenants (tenant_id),
    name              text        NOT NULL CHECK (length(btrim(name)) > 0),
    description       text        NULL,
    operator          text        NOT NULL,
    market_region     text        NOT NULL,
    aggregation_rules jsonb       NOT NULL DEFAULT '{}',
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz NOT NULL DEFAULT now(),
    -- Superset of the PK; the composite-FK target for sites (P1.12 §4.2).
    CONSTRAINT portfolios_id_tenant_key UNIQUE (portfolio_id, tenant_id)
);

ALTER TABLE portfolios OWNER TO flexcore_migrator;

REVOKE ALL ON portfolios FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON portfolios TO flexcore_app;

ALTER TABLE portfolios ENABLE ROW LEVEL SECURITY;
ALTER TABLE portfolios FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON portfolios
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
