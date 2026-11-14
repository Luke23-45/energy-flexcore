-- P4.30 -- Migration 0017 up: feasibility-check evidence records (report
--          §15.2–§15.3, §16.1–§16.2, §21.3; ledger P4.30).
--
-- Decision record:
-- ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md
--
-- What this migration adds (and what it deliberately does NOT):
-- - The command records and the state-change journal (with their
--   validation/feasibility/authorization EVIDENCE REFERENCES on every
--   journal row) already persist since migration 0016 (P4.10) — this
--   migration does NOT recreate them.
-- - The one missing persistence surface of the P4.30 step-1 list is the
--   FEASIBILITY-CHECK RECORD itself: the §12.5 gate's attempt record (the
--   `FeasibilityAttemptRecord` the grid crate returns in memory) and the
--   evidence behind the frozen §15.3 `FeasibilityCheckCompleted` event and
--   the `/api/v1/feasibility-checks` read resource (§15.2). One row per
--   completed check: the command/schedule/scope identity, the model
--   id/version/hash the check evaluated against (§12.5 step 6 + §21.3
--   correlation), the verdict, the failing constraint or fail-closed
--   reason, the margins/coverage summaries, the acting identity, and the
--   correlation (trace) id.
-- - The AUTHORIZATION evidence persists without a new table: the APPROVED
--   journal row (0016) carries the decision reference, actor, and instant,
--   and the immutable §14.3 command payload carries the authorization
--   scope/limits/expiry fields (§14.3 field 14) — recorded decision
--   D-P4.30-2 (no second authorization table with no independent write
--   path).
-- - AUDIT EVENTS persist in the outbox (migration 0007) written in the
--   SAME transaction as the state change (§15.5) — no new table here.
--
-- Immutability and tenancy, in the exact repo conventions:
-- - Append-only: flexcore_app holds INSERT + SELECT only; a
--   defence-in-depth trigger refuses UPDATE and DELETE unconditionally
--   (including for the owner — the 0008 audit_records pattern). A
--   re-checked candidate is a NEW check row, never an edit.
-- - tenant_id on the row (§16.2); RLS ENABLE + FORCE with the shared
--   tenant_isolation policy (owner flexcore_migrator, runtime
--   flexcore_app, P1.12 §5.1 conventions).
-- - The PRIMARY KEY (check_id, tenant_id) scopes the check identity per
--   tenant; check_id is TEXT (the attempt id or the pipeline-level
--   `gate:<candidate>` id — the recorded `FeasibilityCheckCompleted`
--   check-id convention of the P4.37 consumption contract), not a UUID.
-- - command_id is NULLABLE: a §12.5 check may complete for a schedule
--   candidate before any command record exists; when a command IS named,
--   the composite FK pins it in the same tenant scope.
-- - No retention value, no TTL, no scheduled delete (P0.16 §4): the
--   retention of this command/audit-evidence class is the deployment's
--   `RetentionPolicy.command_audit_evidence` configuration (report §16.3),
--   never a period in code.

CREATE TABLE feasibility_check_records (
    check_id               text        NOT NULL CHECK (length(btrim(check_id)) > 0),
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    command_id             uuid        NULL,
    portfolio_id           uuid        NOT NULL,
    site_id                uuid        NOT NULL,
    asset_id               uuid        NOT NULL,
    schedule_id            uuid        NOT NULL,
    schedule_revision      bigint      NOT NULL CHECK (schedule_revision >= 1),
    model_id               text        NULL,
    model_version          text        NULL,
    model_content_hash     text        NULL,
    outcome                text        NOT NULL CHECK (
                               outcome = ANY (ARRAY[
                                   'PASSED'::text,
                                   'LIMIT_VIOLATED'::text,
                                   'FAIL_CLOSED'::text])),
    failing_constraint     text        NULL,
    reason                 text        NULL,
    margins                text        NULL,
    coverage               text        NULL,
    actor                  text        NOT NULL CHECK (length(btrim(actor)) > 0),
    correlation_id         text        NOT NULL CHECK (length(btrim(correlation_id)) > 0),
    checked_at_unix_millis bigint      NOT NULL,
    recorded_at            timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT feasibility_check_records_pk
        PRIMARY KEY (check_id, tenant_id),
    CONSTRAINT feasibility_check_records_command_fk
        FOREIGN KEY (command_id, tenant_id)
        REFERENCES dispatch_commands (command_id, tenant_id),
    CONSTRAINT feasibility_check_records_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT feasibility_check_records_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    CONSTRAINT feasibility_check_records_asset_tenant_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id),
    CONSTRAINT feasibility_check_records_schedule_revision_fk
        FOREIGN KEY (schedule_id, tenant_id, schedule_revision)
        REFERENCES schedule_revisions (schedule_id, tenant_id, revision),
    -- Verdict-shape rules (§12.5 step 6–7 + the fail-closed closing rule):
    -- a PASSED check names no constraint and no failure reason and binds
    -- THE MODEL HASH (the evidence-binding rule, §17.5 invariant 3); a
    -- limit violation names the constraint; a fail-closed outcome names
    -- the reason.
    CONSTRAINT feasibility_check_records_passed_shape CHECK (
        outcome <> 'PASSED'::text
        OR (failing_constraint IS NULL AND reason IS NULL
            AND model_content_hash IS NOT NULL)),
    CONSTRAINT feasibility_check_records_limit_shape CHECK (
        outcome <> 'LIMIT_VIOLATED'::text OR failing_constraint IS NOT NULL),
    CONSTRAINT feasibility_check_records_fail_closed_shape CHECK (
        outcome <> 'FAIL_CLOSED'::text OR reason IS NOT NULL)
);

CREATE INDEX feasibility_check_records_command_idx
    ON feasibility_check_records (tenant_id, command_id, checked_at_unix_millis);
CREATE INDEX feasibility_check_records_schedule_idx
    ON feasibility_check_records (tenant_id, schedule_id, schedule_revision);

-- Append-only defence in depth (the 0008 audit_records pattern layer 2):
-- UPDATE and DELETE raise unconditionally, including for the table owner
-- (whose privileges the grants cannot revoke).
CREATE FUNCTION feasibility_check_records_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'feasibility_check_records is append-only: UPDATE and DELETE are refused (P4.30, report s16.1)';
END $$;

CREATE TRIGGER feasibility_check_records_append_only
    BEFORE UPDATE OR DELETE ON feasibility_check_records
    FOR EACH ROW
    EXECUTE FUNCTION feasibility_check_records_append_only_guard();

ALTER TABLE feasibility_check_records OWNER TO flexcore_migrator;
ALTER FUNCTION feasibility_check_records_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON feasibility_check_records FROM PUBLIC;
GRANT SELECT, INSERT ON feasibility_check_records TO flexcore_app;

ALTER TABLE feasibility_check_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE feasibility_check_records FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON feasibility_check_records
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
