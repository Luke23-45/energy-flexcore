-- P4.10 -- Migration 0016 up: dispatch command records + the state-change
--          journal (report §14.1–§14.2, §16.1, §16.2; ledger P4.10).
--
-- Decision record:
-- ledger/phase-04-grid-feasibility-dispatch/10_command_state_machine.md
-- (D-P4.10-1 frozen-edge conformance, D-P4.10-2 write model, D-P4.10-3
-- supersede discipline, D-P4.10-4 gate separation, D-P4.10-5 P4.12
-- pre-wiring, D-P4.10-6 expiry state support).
--
-- dispatch_commands (P4.10):
-- - One row per dispatch command. The PRIMARY KEY (command_id, tenant_id)
--   makes command records immutable per tenant; content columns have NO
--   update path (the trigger below enforces it in the database). A
--   re-proposal is a NEW command.
-- - state is the closed 14-state §14.2 vocabulary (the P0.11 frozen table
--   is the single source of truth; the edge CHECK in the update trigger is
--   database DEFENCE IN DEPTH only). The BEFORE INSERT trigger enforces
--   the freeze's initial marker: every command enters as PROPOSED.
-- - The ONE-ACTIVE-TRANSITION invariant per command is enforced in SQL:
--   the runtime transition is a compare-and-swap (`UPDATE ... WHERE
--   state = <from>`, the adapter's statement shape) and the BEFORE UPDATE
--   trigger admits exactly ONE frozen-table state change per UPDATE —
--   a no-op state rewrite and a double movement are both rejected.
-- - SUPERSEDED keeps the command record for audit (§14.2), with the
--   backward link superseded_by_command_id required exactly in that state
--   and never self-referential; the forward link supersedes_command_id is
--   the §14.3 field-16 declaration of the replacing command. SUPERSEDED
--   and CANCELLED are different states and never merged (P0.11 §3.1).
-- - Commands reference schedule ID + revision (P3.22 identity scheme):
--   the composite FK into schedule_revisions makes the referenced revision
--   exist; the SUPERSEDED/unknown revision rejection is the adapter's
--   ensure_schedule_revision_usable check (P4.12 pre-wiring; the §12.5
--   feasibility pipeline itself is P4.12's).
-- - valid_until_millis is the §14.3 valid-until bound the EXPIRED state
--   support compares against (end-exclusive §10.3 semantics); the expiry
--   sweep/job is P4.15's — this schema carries only the state support.
-- - tenant_id on the row (§16.2); RLS in the exact P1.12 §5.1 conventions
--   (owner flexcore_migrator, runtime flexcore_app, ENABLE + FORCE + the
--   shared tenant_isolation policy shape).
-- - Immutability: content has NO update path. The runtime grants are
--   INSERT + SELECT plus a COLUMN-LEVEL UPDATE grant on exactly the two
--   transition columns (state, superseded_by_command_id — the recorded
--   exception anticipated by migration 0013 / D-P3.21-8, same shape as
--   migration 0014's supersede grant).
-- - No retention value, no TTL, no scheduled delete (P0.16 §4).

CREATE TABLE dispatch_commands (
    command_id               uuid        NOT NULL,
    tenant_id                uuid        NOT NULL REFERENCES tenants (tenant_id),
    portfolio_id             uuid        NOT NULL,
    site_id                  uuid        NOT NULL,
    asset_id                 uuid        NOT NULL,
    schedule_id              uuid        NOT NULL,
    schedule_revision        bigint      NOT NULL CHECK (schedule_revision >= 1),
    valid_until_millis       bigint      NOT NULL,
    supersedes_command_id    uuid        NULL,
    superseded_by_command_id uuid        NULL,
    state                    text        NOT NULL CHECK (
                                 state = ANY (ARRAY[
                                     'PROPOSED'::text,
                                     'VALIDATED'::text,
                                     'FEASIBLE'::text,
                                     'APPROVED'::text,
                                     'ISSUED'::text,
                                     'ACKNOWLEDGED'::text,
                                     'TRACKING'::text,
                                     'COMPLETED'::text,
                                     'REJECTED'::text,
                                     'FAILED'::text,
                                     'EXPIRED'::text,
                                     'CANCELLED'::text,
                                     'SUPERSEDED'::text,
                                     'FALLBACK'::text])),
    command_payload          jsonb       NOT NULL,
    created_at               timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT dispatch_commands_pk PRIMARY KEY (command_id, tenant_id),
    CONSTRAINT dispatch_commands_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT dispatch_commands_site_tenant_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    CONSTRAINT dispatch_commands_asset_tenant_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id),
    -- Commands reference schedule ID + revision (P3.22 D-P3.22-1); the
    -- referenced revision must exist in the same tenant scope.
    CONSTRAINT dispatch_commands_schedule_revision_fk
        FOREIGN KEY (schedule_id, tenant_id, schedule_revision)
        REFERENCES schedule_revisions (schedule_id, tenant_id, revision),
    -- The bidirectional supersede pair (D-P4.10-3): the forward link names
    -- the command THIS one replaces; the backward link is set by the
    -- SUPERSEDED transition. Both are tenant-scoped self-references.
    CONSTRAINT dispatch_commands_supersedes_fk
        FOREIGN KEY (supersedes_command_id, tenant_id)
        REFERENCES dispatch_commands (command_id, tenant_id),
    CONSTRAINT dispatch_commands_superseded_by_fk
        FOREIGN KEY (superseded_by_command_id, tenant_id)
        REFERENCES dispatch_commands (command_id, tenant_id),
    -- No command replaces itself.
    CONSTRAINT dispatch_commands_no_self_supersede CHECK (
        supersedes_command_id IS NULL
        OR supersedes_command_id <> command_id),
    CONSTRAINT dispatch_commands_no_self_superseded_by CHECK (
        superseded_by_command_id IS NULL
        OR superseded_by_command_id <> command_id),
    -- The backward link is present EXACTLY in the SUPERSEDED state
    -- (§14.2: retained for audit, distinct from CANCELLED).
    CONSTRAINT dispatch_commands_supersede_state_link CHECK (
        (state = 'SUPERSEDED'::text) = (superseded_by_command_id IS NOT NULL))
);

-- The freeze's initial marker (P0.11 §2 row 28): every command enters as
-- PROPOSED — no other initial state can be inserted.
CREATE FUNCTION dispatch_commands_guard_insert() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state <> 'PROPOSED' THEN
        RAISE EXCEPTION 'dispatch_commands: every command enters as PROPOSED (P0.11 initial marker, report §14.1)';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER dispatch_commands_initial_state
    BEFORE INSERT ON dispatch_commands
    FOR EACH ROW EXECUTE FUNCTION dispatch_commands_guard_insert();

-- The frozen §14.1 transition table (27 edges), as database defence in
-- depth. The single source of truth is the domain's P0.11 table
-- (flexcore_domain::command_state); this encoding MUST mirror it exactly —
-- a divergence is a bug against the oracle, never an extension.
CREATE FUNCTION dispatch_commands_legal_edge(from_state text, to_state text)
RETURNS boolean
LANGUAGE sql IMMUTABLE AS $$
    SELECT (from_state, to_state) IN (
        ('PROPOSED',    'VALIDATED'),        -- 1
        ('PROPOSED',    'REJECTED'),         -- 2
        ('VALIDATED',   'FEASIBLE'),         -- 3
        ('VALIDATED',   'REJECTED'),         -- 4
        ('VALIDATED',   'SUPERSEDED'),       -- 5
        ('FEASIBLE',    'APPROVED'),         -- 6
        ('FEASIBLE',    'REJECTED'),         -- 7
        ('FEASIBLE',    'SUPERSEDED'),       -- 8
        ('APPROVED',    'ISSUED'),           -- 9
        ('APPROVED',    'EXPIRED'),          -- 10
        ('APPROVED',    'SUPERSEDED'),       -- 11
        ('APPROVED',    'CANCELLED'),        -- 12
        ('ISSUED',      'ACKNOWLEDGED'),     -- 13
        ('ISSUED',      'FAILED'),           -- 14
        ('ISSUED',      'EXPIRED'),          -- 15
        ('ISSUED',      'SUPERSEDED'),       -- 16
        ('ISSUED',      'CANCELLED'),        -- 17
        ('ACKNOWLEDGED','TRACKING'),         -- 18
        ('ACKNOWLEDGED','FAILED'),           -- 19
        ('ACKNOWLEDGED','EXPIRED'),          -- 20
        ('ACKNOWLEDGED','SUPERSEDED'),       -- 21
        ('TRACKING',    'COMPLETED'),        -- 22
        ('TRACKING',    'FAILED'),           -- 23
        ('TRACKING',    'EXPIRED'),          -- 24
        ('TRACKING',    'SUPERSEDED'),       -- 25
        ('FAILED',      'FALLBACK'),         -- 26
        ('EXPIRED',     'FALLBACK')          -- 27
    );
$$;

-- Immutability guard: the ONLY permitted UPDATE is exactly ONE frozen-table
-- state transition (the compare-and-swap in the adapter's statement is the
-- one-active-transition invariant's first half; this trigger is the second —
-- one state change per UPDATE, no content change, and the SUPERSEDED
-- backward link set exactly by that transition).
CREATE FUNCTION dispatch_commands_guard_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.command_id, NEW.tenant_id, NEW.portfolio_id, NEW.site_id,
        NEW.asset_id, NEW.schedule_id, NEW.schedule_revision,
        NEW.valid_until_millis, NEW.supersedes_command_id,
        NEW.command_payload, NEW.created_at)
       IS DISTINCT FROM
       (OLD.command_id, OLD.tenant_id, OLD.portfolio_id, OLD.site_id,
        OLD.asset_id, OLD.schedule_id, OLD.schedule_revision,
        OLD.valid_until_millis, OLD.supersedes_command_id,
        OLD.command_payload, OLD.created_at) THEN
        RAISE EXCEPTION 'dispatch_commands: command content is immutable (P4.10 append-only records)';
    END IF;
    IF NEW.state = OLD.state THEN
        RAISE EXCEPTION 'dispatch_commands: a command row only ever moves through ONE state transition (one-active-transition invariant, ledger P4.10)';
    END IF;
    IF NOT dispatch_commands_legal_edge(OLD.state, NEW.state) THEN
        RAISE EXCEPTION 'dispatch_commands: transition is not in the frozen command state machine (P0.11, report §14.1)';
    END IF;
    IF NEW.state = 'SUPERSEDED' THEN
        IF NEW.superseded_by_command_id IS NULL THEN
            RAISE EXCEPTION 'dispatch_commands: the SUPERSEDED transition requires the superseded_by link (§14.2)';
        END IF;
    ELSIF NEW.superseded_by_command_id IS NOT NULL THEN
        RAISE EXCEPTION 'dispatch_commands: the superseded_by link may only be set by the SUPERSEDED transition';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER dispatch_commands_immutability
    BEFORE UPDATE ON dispatch_commands
    FOR EACH ROW EXECUTE FUNCTION dispatch_commands_guard_update();

ALTER TABLE dispatch_commands OWNER TO flexcore_migrator;

REVOKE ALL ON dispatch_commands FROM PUBLIC;
GRANT SELECT, INSERT ON dispatch_commands TO flexcore_app;
-- The state transition is the ONLY permitted update: a COLUMN-LEVEL grant
-- on exactly the two transition columns (the recorded exception anticipated
-- by migration 0013, same shape as migration 0014). Content columns stay
-- unwritable by the runtime role.
GRANT UPDATE (state, superseded_by_command_id) ON dispatch_commands TO flexcore_app;

ALTER TABLE dispatch_commands ENABLE ROW LEVEL SECURITY;
ALTER TABLE dispatch_commands FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON dispatch_commands
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

-- dispatch_command_transitions (P4.10 step 6): the append-only state-change
-- journal. One immutable row per journalled transition: from, to, actor,
-- timestamp, reason, evidence reference, and the gate that produced gate
-- transitions/rejections (§14.1: validation, feasibility, and authorization
-- are distinct — the audit record names WHICH gate). INSERT + SELECT only —
-- journal rows are never updated or deleted, so the SUPERSEDED and
-- CANCELLED audit records stay structurally distinct and the full history
-- is reconstructable.
CREATE TABLE dispatch_command_transitions (
    command_id        uuid        NOT NULL,
    tenant_id         uuid        NOT NULL REFERENCES tenants (tenant_id),
    seq               bigint      GENERATED ALWAYS AS IDENTITY,
    from_state        text        NOT NULL CHECK (
                          from_state = ANY (ARRAY[
                              'PROPOSED'::text, 'VALIDATED'::text,
                              'FEASIBLE'::text, 'APPROVED'::text,
                              'ISSUED'::text, 'ACKNOWLEDGED'::text,
                              'TRACKING'::text, 'COMPLETED'::text,
                              'REJECTED'::text, 'FAILED'::text,
                              'EXPIRED'::text, 'CANCELLED'::text,
                              'SUPERSEDED'::text, 'FALLBACK'::text])),
    to_state          text        NOT NULL CHECK (
                          to_state = ANY (ARRAY[
                              'PROPOSED'::text, 'VALIDATED'::text,
                              'FEASIBLE'::text, 'APPROVED'::text,
                              'ISSUED'::text, 'ACKNOWLEDGED'::text,
                              'TRACKING'::text, 'COMPLETED'::text,
                              'REJECTED'::text, 'FAILED'::text,
                              'EXPIRED'::text, 'CANCELLED'::text,
                              'SUPERSEDED'::text, 'FALLBACK'::text])),
    actor             text        NOT NULL CHECK (length(btrim(actor)) > 0),
    at_unix_millis    bigint      NOT NULL,
    gate              text        NULL CHECK (
                          gate = ANY (ARRAY[
                              'validation'::text,
                              'feasibility'::text,
                              'authorization'::text])),
    reason            text        NULL,
    evidence_reference text       NULL,
    recorded_at       timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT dispatch_command_transitions_pk
        PRIMARY KEY (command_id, tenant_id, seq),
    CONSTRAINT dispatch_command_transitions_command_fk
        FOREIGN KEY (command_id, tenant_id)
        REFERENCES dispatch_commands (command_id, tenant_id),
    -- No self-transition; the frozen edge itself is re-checked by the
    -- storage constructor through the domain table (the journal row is
    -- written in the same transaction as the command-row transition).
    CONSTRAINT dispatch_command_transitions_moves CHECK (from_state <> to_state),
    -- The gate is named EXACTLY for the three gate passes and the gate
    -- rejections (§14.1: the three gates are distinct).
    CONSTRAINT dispatch_command_transitions_gate_rule CHECK (
        ((to_state = ANY (ARRAY['VALIDATED'::text, 'FEASIBLE'::text, 'APPROVED'::text]))
         OR to_state = 'REJECTED'::text)
        = (gate IS NOT NULL)),
    -- Evidence rule (§6.4: each stage records evidence; §14.3 field 16 for
    -- SUPERSEDED; §14.4 for FALLBACK).
    CONSTRAINT dispatch_command_transitions_evidence_rule CHECK (
        NOT (to_state = ANY (ARRAY[
                'VALIDATED'::text, 'FEASIBLE'::text, 'APPROVED'::text,
                'ISSUED'::text, 'ACKNOWLEDGED'::text, 'TRACKING'::text,
                'COMPLETED'::text, 'SUPERSEDED'::text, 'FALLBACK'::text]))
        OR evidence_reference IS NOT NULL),
    -- Reason rule (§14.3 failing check; the FAILED trigger; the P4.15
    -- milestone-not-reached; the §14.2 cancellation reason; the §14.4
    -- fallback policy).
    CONSTRAINT dispatch_command_transitions_reason_rule CHECK (
        NOT (to_state = ANY (ARRAY[
                'REJECTED'::text, 'FAILED'::text, 'EXPIRED'::text,
                'CANCELLED'::text, 'FALLBACK'::text]))
        OR reason IS NOT NULL)
);

CREATE INDEX dispatch_command_transitions_command_idx
    ON dispatch_command_transitions (tenant_id, command_id, seq);

ALTER TABLE dispatch_command_transitions OWNER TO flexcore_migrator;

REVOKE ALL ON dispatch_command_transitions FROM PUBLIC;
GRANT SELECT, INSERT ON dispatch_command_transitions TO flexcore_app;

ALTER TABLE dispatch_command_transitions ENABLE ROW LEVEL SECURITY;
ALTER TABLE dispatch_command_transitions FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON dispatch_command_transitions
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
