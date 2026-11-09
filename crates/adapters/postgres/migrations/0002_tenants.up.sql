-- P1.13 -- Migration 0002 up: tenants (P1.12 §4.1).
--
-- The tenant registry root. No tenant_id column (the table IS the tenant
-- root) and no RLS policy (P1.12 §5.2). PK default uuidv7() per D-S1.
--
-- RECORDED DEVIATION from P1.12 §4.1: `description` (a P1.06 domain
-- descriptive field on every entity) and a UNIQUE constraint on
-- `idp_reference` are added. The P1.07 repository port documents the
-- identity-provider-reference uniqueness rule ("one tenant per
-- identity-provider reference — recorded assumption") and must return the
-- full domain entity on read, so the descriptive field travels with the
-- row. Both additions are additive and do not touch D-S1..D-S5.

CREATE TABLE tenants (
    tenant_id     uuid        PRIMARY KEY DEFAULT uuidv7(),
    name          text        NOT NULL UNIQUE CHECK (length(btrim(name)) > 0),
    description   text        NULL,
    idp_reference text        NOT NULL,
    policy_profile text       NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tenants_idp_reference_key UNIQUE (idp_reference)
);

ALTER TABLE tenants OWNER TO flexcore_migrator;

REVOKE ALL ON tenants FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON tenants TO flexcore_app;
-- No DELETE grant to any role (P1.12 §6: registry tables have no Phase 1
-- delete path; a future delete needs an explicit, reviewed migration).
