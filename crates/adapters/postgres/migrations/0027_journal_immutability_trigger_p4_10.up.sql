-- P4.10 -- Migration 0027 up: the immutability trigger on the command
--          journal (`dispatch_command_transitions`).
--
-- WHY THIS TRIGGER EXISTS (Batch 67): migration 0016 made the journal
-- INSERT-only for the runtime role (GRANT SELECT, INSERT to
-- `flexcore_app`) and the ledger D-P4.10-2 design says "the journal is
-- INSERT-only: one immutable row per transition" — but the GRANT binds
-- only the runtime role. An owner session (the migrator, or a superuser
-- test connection) bypasses grants, and no trigger existed on the
-- journal at all: a raw `UPDATE dispatch_command_transitions ...` or
-- `DELETE FROM dispatch_command_transitions ...` SUCCEEDED against a
-- table whose whole point is that a row, once recorded, is never
-- rewritten. The sibling parent table got its backstop trigger in 0016
-- (`dispatch_commands_immutability`); the journal never got one. The
-- gated test (`crates/adapters/postgres/tests/command_store.rs`,
-- "a journal UPDATE must be rejected") asserted the backstop and failed.
--
-- THE FIX: a BEFORE UPDATE OR DELETE trigger that raises on ANY
-- attempt — the journal has no legitimate update path at all (the
-- adapter only INSERTs journal rows; corrections are NEW rows, never
-- rewrites). This is the trigger the 0016-era test comment already
-- describes as "the backstop for owner sessions".
--
-- EXPAND-ONLY (the P11.18 policy, MIGRATION_POLICY.md): the statement
-- creates ONE new trigger + function on an existing table — no column,
-- constraint, or grant belonging to any earlier migration is touched;
-- nothing is dropped or replaced. The down migration drops only what
-- this migration created.
--
-- LOCK-RISK NOTE: `CREATE TRIGGER` takes only a brief lock on the
-- table; at application time the table is EMPTY in this program (the
-- workspace is WRITTEN-UNRUN, no deployment exists), so the lock
-- window is bounded by an empty table.

CREATE FUNCTION dispatch_command_transitions_guard_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'dispatch_command_transitions: the command journal is append-only (P4.10 — one immutable row per transition; corrections are NEW rows, never rewrites)';
END $$;

CREATE TRIGGER dispatch_command_transitions_immutability
    BEFORE UPDATE OR DELETE ON dispatch_command_transitions
    FOR EACH ROW EXECUTE FUNCTION dispatch_command_transitions_guard_immutable();
