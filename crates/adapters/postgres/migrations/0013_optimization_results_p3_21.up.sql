-- P3.21 -- Migration 0013 up: §12.6 schedule-result records
--          (report §12.6, §16.1, §16.2; ledger P3.21 step 3).
--
-- The optimization result record is persisted in PostgreSQL BEFORE any
-- downstream use (§16.1): the ScheduleProposed outbox row (007) and this
-- state row are written in ONE transaction by the adapter (§15.5), and the
-- publisher releases the event only after the commit.
--
-- Design (recorded in
-- ledger/phase-03-flexibility-scheduling/21_result_contract.md, D-P3.21-1):
--
-- - candidate_id is the PRIMARY KEY (§12.6 "output candidate ID"): records
--   are IMMUTABLE — a re-solve is a new candidate (P3.22 owns revision
--   identity), so there is no UPDATE or DELETE path and the runtime grants
--   are INSERT/SELECT only (the P2.06 append-only grant discipline).
-- - The structured columns carry every field a reader filters or audits on;
--   the FULL §12.6 record (diagnostics, warnings, assumptions, determinism)
--   travels in record_payload jsonb — opaque to the storage crate, validated
--   as JSON on insert by the adapter (the storage crate imports no
--   serialization library).
-- - status carries the CLOSED §12.6 vocabulary as a CHECK — no other string
--   can be stored (the vocabulary's single home is the optimization API's
--   SolveStatus; the stored-text form mirrors it exactly).
-- - The §12.6 objective-completeness rule is enforced in the database too:
--   optimal/feasible rows MUST carry objective_value AND non-blank
--   objective_units (the port contract's rule, mirrored at rest).
-- - grid_model_version is REQUIRED and traceable: grid_model_site_id +
--   grid_model_site_reference record WHICH Phase 2 Site grid-model reference
--   (P2.02) supplied the version — never a Phase 3 or Phase 4 model, which
--   do not exist (ledger P3.21 step 2). The composite FK to
--   sites(site_id, tenant_id) keeps the denormalized tenant provably
--   consistent (the 0004 pattern); the composite FK to portfolios keeps the
--   portfolio leg consistent likewise.
-- - schedule_proposed_key records the deterministic ScheduleProposed
--   idempotency key the record was persisted with — the pairing proof that
--   the outbox row and this state row belong to one transaction (§15.5).
-- - tenant_id is part of the record (§16.2: tenant_id on every tenant-owned
--   record); RLS is applied in the exact P1.12 §5.1 conventions (owner
--   flexcore_migrator, runtime flexcore_app, ENABLE + FORCE + the shared
--   tenant_isolation policy shape; the application must never hold owner
--   credentials [R25]).
-- - No retention value, no TTL, no scheduled delete appears here (P0.16 §4:
--   retention is configurable, never universal).

CREATE TABLE schedule_results (
    candidate_id              uuid        PRIMARY KEY,
    tenant_id                 uuid        NOT NULL REFERENCES tenants (tenant_id),
    portfolio_id              uuid        NOT NULL,
    site_id                   uuid        NOT NULL,
    status                    text        NOT NULL CHECK (
                                  status = ANY (ARRAY[
                                      'optimal'::text,
                                      'feasible'::text,
                                      'infeasible'::text,
                                      'timeout'::text,
                                      'numerical_failure'::text,
                                      'unsupported'::text])),
    problem_class             text        NOT NULL CHECK (length(btrim(problem_class)) > 0),
    solver_name               text        NOT NULL CHECK (length(btrim(solver_name)) > 0),
    solver_version            text        NOT NULL CHECK (length(btrim(solver_version)) > 0),
    provider_id               text        NOT NULL CHECK (length(btrim(provider_id)) > 0),
    provider_version          text        NOT NULL CHECK (length(btrim(provider_version)) > 0),
    objective_value           double precision NULL,
    objective_units           text        NULL,
    input_snapshot_id         text        NOT NULL CHECK (length(btrim(input_snapshot_id)) > 0),
    grid_model_version        text        NOT NULL CHECK (length(btrim(grid_model_version)) > 0),
    grid_model_site_id        uuid        NOT NULL,
    grid_model_site_reference text        NOT NULL CHECK (length(btrim(grid_model_site_reference)) > 0),
    schedule_proposed_key     text        NOT NULL CHECK (length(btrim(schedule_proposed_key)) > 0),
    record_payload            jsonb       NOT NULL,
    created_at                timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT schedule_results_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT schedule_results_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    -- The ledger P3.21 traceability requirement, enforced at rest: the
    -- grid-model version is traceable to THE record's own site reference.
    CONSTRAINT schedule_results_grid_model_site_fk
        FOREIGN KEY (grid_model_site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    -- The §12.6 objective-completeness rule for solving statuses.
    CONSTRAINT schedule_results_objective_complete CHECK (
        (status IN ('optimal', 'feasible')
         AND objective_value IS NOT NULL
         AND objective_units IS NOT NULL
         AND length(btrim(objective_units)) > 0)
        OR (status NOT IN ('optimal', 'feasible'))),
    -- The grid model the version came from is the record's own site.
    CONSTRAINT schedule_results_grid_model_site_is_record_site
        CHECK (grid_model_site_id = site_id)
);

ALTER TABLE schedule_results OWNER TO flexcore_migrator;

REVOKE ALL ON schedule_results FROM PUBLIC;
-- Immutable records: INSERT + SELECT only. (P3.22's SUPERSEDED marking is a
-- new-record/supersede-link concern, not an in-place mutation; if that
-- subtask needs an UPDATE grant it is that subtask's recorded decision.)
GRANT SELECT, INSERT ON schedule_results TO flexcore_app;

ALTER TABLE schedule_results ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule_results FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON schedule_results
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
