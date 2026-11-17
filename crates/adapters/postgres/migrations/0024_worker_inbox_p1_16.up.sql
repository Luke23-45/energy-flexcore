-- P1.16/P1.19 -- Migration 0024 up: the durable worker consumer inbox.
--          Report/phase trace: P1.16 (the §6.3 inbox discipline),
--          P1.19 (the worker binary); decision record: the P1.19 ledger.
--          Promotion recorded for the final review as FRV1-R5-004.
--
-- WHY THIS MIGRATION EXISTS (FRV1-R5-004): the consumer inbox — which
-- carries the durable duplicate-side-effect guarantee of the §6.3
-- discipline (P1.16) — was created at worker startup by
-- `apps/worker/src/inbox.rs::ensure_table` (`CREATE TABLE IF NOT EXISTS`)
-- because the store's edit scope at P1.19 excluded the adapter's
-- migration set. That interim measure made a database provisioned by
-- migrations alone lack the inbox table until the worker ran, outside the
-- checksum/owner/grant conventions every other runtime table follows.
-- The DDL moves here, column-for-column IDENTICAL to the runtime
-- statement it replaces (the runtime `ensure_table` is REMOVED in the
-- same change; the migration owns the table).
--
-- INFRASTRUCTURE WORKING QUEUE, NOT TENANT-OWNED (the 0007 outbox
-- precedent, P1.12 §5.2): the inbox is per-consumer bookkeeping keyed by
-- (consumer_name, idempotency_key); it carries no tenant attribute, so
-- there is NO row-level security — enabling RLS without a tenant column
-- would fail every worker access. Access is flexcore_app-only: the worker
-- reads existence and inserts with ON CONFLICT DO NOTHING (SELECT +
-- INSERT exactly).
--
-- EXPAND-ONLY (the P11.18 policy, MIGRATION_POLICY.md): every statement
-- is a CREATE on a NEW table or a grant on it — no ALTER touches an
-- existing table, nothing is dropped or replaced. Stage 2/3/4 do not
-- apply: nothing is replaced, nothing is removed — the down migration
-- drops the new table whole.
--
-- UPGRADE NOTE (recorded, not hidden): a database where the interim
-- runtime table already exists (the worker ran before this migration)
-- holds the SAME schema this migration creates — the P1.19 statement was
-- never executed in this program (the workspace is WRITTEN-UNRUN), so no
-- such database is known to exist; if one does, the migration fails
-- loudly on the name collision rather than silently diverging, and the
-- recorded remediation is dropping the interim table (it holds only
-- consumed-at bookkeeping that the JetStream redelivery window covers)
-- before re-applying.

CREATE TABLE flexcore_worker_inbox (
    consumer_name text NOT NULL,
    idempotency_key text NOT NULL,
    consumed_at timestamptz NOT NULL DEFAULT now(),
    consumer_identity text NOT NULL DEFAULT 'flexcore-worker',
    PRIMARY KEY (consumer_name, idempotency_key)
);

ALTER TABLE flexcore_worker_inbox OWNER TO flexcore_migrator;

REVOKE ALL ON flexcore_worker_inbox FROM PUBLIC;
GRANT SELECT, INSERT ON flexcore_worker_inbox TO flexcore_app;
