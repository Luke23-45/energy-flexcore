-- P3.22/P3.23 -- Migration 0014 up: schedule revisions + §14.2 SUPERSEDED
--          transitions (report §12.6, §14.2, §16.1, §16.2; ledger P3.22) and
--          schedule comparison records (ledger P3.23).
--
-- Decision record:
-- ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md
-- (D-P3.22-1 identity scheme, D-P3.22-2 supersede discipline, D-P3.23-3
-- comparison persistence).
--
-- schedule_revisions (P3.22):
-- - One row per schedule revision. The PRIMARY KEY (schedule_id, tenant_id,
--   revision) makes revisions immutable and monotonic per chain; the CHECK
--   schedule_revisions_initial_chain makes the chain GAPLESS (revision 1 has
--   no supersedes link; revision n > 1 supersedes exactly n - 1 and carries
--   the superseded candidate). A re-solve creates a NEW revision — there is
--   no content mutation path (the trigger below enforces it in the database).
-- - state is the closed vocabulary 'active'/'superseded'. 'superseded' is
--   the §14.2 SUPERSEDED state — retained for audit, distinct from
--   CANCELLED. The state-link CHECK makes the bidirectional supersede link
--   total: an active row has no forward link; a superseded row points at
--   its superseding revision + candidate (strictly greater revision).
-- - The ONE-ACTIVE invariant is enforced by the partial unique index
--   schedule_revisions_one_active: the SUPERSEDED marking of revision n - 1
--   and the insert of revision n must run in ONE transaction (the adapter's
--   unit-of-work pattern).
-- - Immutability: revision content has NO update path. The runtime grants
--   are INSERT + SELECT plus a COLUMN-LEVEL UPDATE grant on exactly the
--   three supersede-transition columns (the recorded exception anticipated
--   by migration 0013 / D-P3.21-8), and the BEFORE UPDATE trigger
--   schedule_revisions_immutability rejects (a) any change to a content
--   column and (b) any state movement other than active -> superseded.
-- - roll_trigger is a REQUIRED field (ledger P3.24 step 1): the closed
--   trigger vocabulary records WHY the revision was produced (the P3.24
--   trigger set + 'initial_solve' for the first revision of a chain).
-- - candidate_id references the P3.21 schedule_results record (composite FK
--   on (candidate_id, tenant_id), backed by the unique constraint added to
--   schedule_results below): the revision and its §12.6 candidate record are
--   two views of ONE solve. Every new revision is a CANDIDATE — never a
--   feasible, approved, or dispatchable schedule (Phase 4 gates).
-- - tenant_id on the row (§16.2); RLS in the exact P1.12 §5.1 conventions
--   (owner flexcore_migrator, runtime flexcore_app, ENABLE + FORCE + the
--   shared tenant_isolation policy shape).
-- - No retention value, no TTL, no scheduled delete (P0.16 §4).

-- Referential anchor for the composite candidate FKs below (candidate_id is
-- already the primary key of schedule_results, so the pair is unique; the
-- explicit constraint makes it referenceable).
ALTER TABLE schedule_results
    ADD CONSTRAINT schedule_results_candidate_tenant_key
    UNIQUE (candidate_id, tenant_id);

CREATE TABLE schedule_revisions (
    schedule_id               uuid        NOT NULL,
    revision                  bigint      NOT NULL CHECK (revision >= 1),
    tenant_id                 uuid        NOT NULL REFERENCES tenants (tenant_id),
    portfolio_id              uuid        NOT NULL,
    site_id                   uuid        NOT NULL,
    horizon_start_millis      bigint      NOT NULL,
    horizon_end_millis        bigint      NOT NULL CHECK (horizon_end_millis > horizon_start_millis),
    roll_trigger              text        NOT NULL CHECK (
                                  roll_trigger = ANY (ARRAY[
                                      'initial_solve'::text,
                                      'horizon_advance'::text,
                                      'envelope_invalidation'::text,
                                      'asset_opt_out_dropout'::text,
                                      'forecast_update'::text,
                                      'operator_request'::text])),
    state                     text        NOT NULL CHECK (
                                  state = ANY (ARRAY['active'::text, 'superseded'::text])),
    supersedes_revision       bigint      NULL,
    supersedes_candidate_id   uuid        NULL,
    superseded_by_revision    bigint      NULL,
    superseded_by_candidate_id uuid       NULL,
    candidate_id              uuid        NOT NULL,
    schedule_payload          jsonb       NOT NULL,
    created_at                timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT schedule_revisions_pk PRIMARY KEY (schedule_id, tenant_id, revision),
    CONSTRAINT schedule_revisions_candidate_tenant_fk
        FOREIGN KEY (candidate_id, tenant_id)
        REFERENCES schedule_results (candidate_id, tenant_id),
    CONSTRAINT schedule_revisions_supersedes_candidate_fk
        FOREIGN KEY (supersedes_candidate_id, tenant_id)
        REFERENCES schedule_results (candidate_id, tenant_id),
    CONSTRAINT schedule_revisions_superseded_by_candidate_fk
        FOREIGN KEY (superseded_by_candidate_id, tenant_id)
        REFERENCES schedule_results (candidate_id, tenant_id),
    CONSTRAINT schedule_revisions_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT schedule_revisions_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    -- The gapless chain rule (D-P3.22-1): revision 1 starts the chain;
    -- revision n > 1 supersedes exactly n - 1, naming its candidate.
    CONSTRAINT schedule_revisions_initial_chain CHECK (
        (revision = 1
         AND supersedes_revision IS NULL
         AND supersedes_candidate_id IS NULL)
        OR (revision > 1
            AND supersedes_revision = revision - 1
            AND supersedes_candidate_id IS NOT NULL)),
    -- The forward link points forward in the chain only.
    CONSTRAINT schedule_revisions_supersede_direction CHECK (
        superseded_by_revision IS NULL OR superseded_by_revision > revision),
    -- The bidirectional supersede link is total (D-P3.22-2): active has no
    -- forward link; superseded carries BOTH forward-link columns.
    CONSTRAINT schedule_revisions_state_links CHECK (
        (state = 'active'
         AND superseded_by_revision IS NULL
         AND superseded_by_candidate_id IS NULL)
        OR (state = 'superseded'
            AND superseded_by_revision IS NOT NULL
            AND superseded_by_candidate_id IS NOT NULL))
);

-- Exactly ONE active (current) revision per schedule chain. The supersede
-- marking and the new-revision insert run in ONE transaction (the adapter's
-- unit-of-work pattern), so the chain never observably lacks a current
-- revision.
CREATE UNIQUE INDEX schedule_revisions_one_active
    ON schedule_revisions (tenant_id, schedule_id)
    WHERE state = 'active';

-- Immutability guard: the ONLY permitted UPDATE is the active -> superseded
-- transition of the three state/link columns; every content column is
-- immutable (P3.22: append-only revisions — replay stays possible).
CREATE FUNCTION schedule_revisions_guard_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state <> OLD.state
       AND NOT (OLD.state = 'active' AND NEW.state = 'superseded') THEN
        RAISE EXCEPTION 'schedule_revisions: only the active -> superseded transition is permitted (P3.22, report §14.2)';
    END IF;
    IF (NEW.schedule_id, NEW.revision, NEW.tenant_id, NEW.portfolio_id,
        NEW.site_id, NEW.horizon_start_millis, NEW.horizon_end_millis,
        NEW.roll_trigger, NEW.supersedes_revision, NEW.supersedes_candidate_id,
        NEW.candidate_id, NEW.schedule_payload, NEW.created_at)
       IS DISTINCT FROM
       (OLD.schedule_id, OLD.revision, OLD.tenant_id, OLD.portfolio_id,
        OLD.site_id, OLD.horizon_start_millis, OLD.horizon_end_millis,
        OLD.roll_trigger, OLD.supersedes_revision, OLD.supersedes_candidate_id,
        OLD.candidate_id, OLD.schedule_payload, OLD.created_at) THEN
        RAISE EXCEPTION 'schedule_revisions: revision content is immutable (P3.22 append-only revisions)';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER schedule_revisions_immutability
    BEFORE UPDATE ON schedule_revisions
    FOR EACH ROW EXECUTE FUNCTION schedule_revisions_guard_update();

ALTER TABLE schedule_revisions OWNER TO flexcore_migrator;

REVOKE ALL ON schedule_revisions FROM PUBLIC;
GRANT SELECT, INSERT ON schedule_revisions TO flexcore_app;
-- The dedicated §14.2 SUPERSEDED transition is the ONLY permitted state
-- change: a COLUMN-LEVEL grant on exactly the three transition columns (the
-- recorded exception anticipated by migration 0013). Content columns stay
-- unwritable by the runtime role.
GRANT UPDATE (state, superseded_by_revision, superseded_by_candidate_id)
    ON schedule_revisions TO flexcore_app;

ALTER TABLE schedule_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON schedule_revisions
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

-- schedule_comparisons (P3.23): immutable comparison records. The shared
-- modeling inputs are structured columns; the per-candidate entries (with
-- the four §19.4 scheduling metrics per candidate) travel in
-- comparison_payload jsonb. candidate_count >= 2 mirrors the application
-- gate. INSERT + SELECT only — comparison records are never rewritten.
CREATE TABLE schedule_comparisons (
    comparison_id             uuid        PRIMARY KEY,
    tenant_id                 uuid        NOT NULL REFERENCES tenants (tenant_id),
    portfolio_id              uuid        NOT NULL,
    site_id                   uuid        NOT NULL,
    shared_input_snapshot_id  text        NOT NULL CHECK (length(btrim(shared_input_snapshot_id)) > 0),
    shared_envelope_version   text        NOT NULL CHECK (length(btrim(shared_envelope_version)) > 0),
    shared_forecast_version   text        NOT NULL CHECK (length(btrim(shared_forecast_version)) > 0),
    shared_forecast_provenance text       NOT NULL CHECK (length(btrim(shared_forecast_provenance)) > 0),
    shared_objective_name     text        NOT NULL CHECK (length(btrim(shared_objective_name)) > 0),
    candidate_count           integer     NOT NULL CHECK (candidate_count >= 2),
    comparison_payload        jsonb       NOT NULL,
    created_at                timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT schedule_comparisons_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT schedule_comparisons_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id)
);

ALTER TABLE schedule_comparisons OWNER TO flexcore_migrator;

REVOKE ALL ON schedule_comparisons FROM PUBLIC;
GRANT SELECT, INSERT ON schedule_comparisons TO flexcore_app;

ALTER TABLE schedule_comparisons ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule_comparisons FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON schedule_comparisons
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
