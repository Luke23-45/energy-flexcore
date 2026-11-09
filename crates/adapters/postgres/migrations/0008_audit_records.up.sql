-- P1.13 -- Migration 0008 up: audit_records (P1.12 §4.7) — append-only.
--
-- Append-only enforcement, three layers (P1.12 §4.7):
--   1. Grants: flexcore_app receives INSERT, SELECT only; REVOKE of
--      UPDATE/DELETE/TRUNCATE from PUBLIC; no UPDATE/DELETE grant to ANY
--      role.
--   2. Defence-in-depth trigger (below): BEFORE UPDATE OR DELETE raises an
--      exception unconditionally — including for the table owner, whose
--      owner privileges grants cannot revoke.
--   3. Domain rule: the flexcore-audit AuditSink port exposes append only.
--
-- tenant_id NULL marks platform-level actions with no tenant context; those
-- rows are invisible to flexcore_app under the policy (P1.12 §5.2) and are
-- reachable only through the owner/ops path. Corrections are NEW rows
-- referencing the original via corrected_audit_id — never in-place edits.
-- event_time / ingest_time are distinct columns (P0.09 §1).

CREATE TABLE audit_records (
    audit_id           uuid        PRIMARY KEY DEFAULT uuidv7(),
    tenant_id          uuid        NULL REFERENCES tenants (tenant_id),
    actor              text        NOT NULL,
    action             text        NOT NULL,
    object_type        text        NOT NULL,
    object_id          text        NOT NULL,
    before_ref         jsonb       NULL, -- before *reference*, not a full payload copy
    after_ref          jsonb       NULL,
    decision           jsonb       NULL, -- decision value + reason reference, never a bare allow/deny
    evidence           jsonb       NULL,
    event_time         timestamptz NOT NULL,
    ingest_time        timestamptz NOT NULL DEFAULT now(),
    correlation_id     text        NOT NULL,
    corrected_audit_id uuid        NULL REFERENCES audit_records (audit_id)
);

CREATE INDEX audit_records_tenant_event_time_idx ON audit_records (tenant_id, event_time);
CREATE INDEX audit_records_object_idx ON audit_records (object_type, object_id);

ALTER TABLE audit_records OWNER TO flexcore_migrator;

REVOKE ALL ON audit_records FROM PUBLIC;
GRANT SELECT, INSERT ON audit_records TO flexcore_app;

ALTER TABLE audit_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_records FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON audit_records
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

CREATE OR REPLACE FUNCTION audit_records_append_only_guard()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION
        'audit_records is append-only: UPDATE and DELETE are refused (P1.12 s4.7 layer 2)';
END
$$;

CREATE TRIGGER audit_records_append_only
    BEFORE UPDATE OR DELETE ON audit_records
    FOR EACH ROW
    EXECUTE FUNCTION audit_records_append_only_guard();

ALTER FUNCTION audit_records_append_only_guard() OWNER TO flexcore_migrator;
