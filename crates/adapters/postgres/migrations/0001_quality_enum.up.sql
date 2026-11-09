-- P1.13 -- Migration 0001 up: the P0.08 telemetry-quality enum (D-S4) and the
--          service roles (guarded DO blocks).
--
-- Implements P1.12 §3.2 exactly: a PostgreSQL enum type with exactly the six
-- P0.08 canonical states, in P0.08 §1 order. A seventh state cannot be
-- inserted; evolution requires an explicit ALTER TYPE migration plus a
-- Phase 0-deviation ADR (P1.12 §3.2).
--
-- Roles (RECORDED DEVIATION from P1.12 §3.1): the design document places
-- role creation in the deployment bootstrap. P1.13 instead creates the two
-- service roles here, inside guarded DO blocks, so the migration set is
-- self-contained for CI (P1.13 step 2 task instruction). The deviation is
-- safe: the DO block is idempotent (creates only when absent), the roles are
-- created NOLOGIN (no credentials ever live in a migration), and granting
-- LOGIN plus credentials remains a deployment/test-bootstrap step. If the
-- migration user lacks CREATEROLE (or superuser) the DO block fails by
-- design — role creation is a privileged act and is never attempted
-- silently.

CREATE TYPE telemetry_quality AS ENUM
  ('GOOD', 'SUSPECT', 'BAD', 'MISSING', 'ESTIMATED', 'STALE');

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flexcore_migrator') THEN
        CREATE ROLE flexcore_migrator NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flexcore_app') THEN
        CREATE ROLE flexcore_app NOLOGIN;
    END IF;
END
$$;

-- Schema access for the runtime and migrator roles (PG 15+ restricts the
-- public schema; tables are granted per migration that creates them).
GRANT USAGE ON SCHEMA public TO flexcore_migrator, flexcore_app;
