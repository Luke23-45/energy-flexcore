-- P6.06 -- Migration 0028 up: the one-ACTIVE-term rule moves from the
--          partial unique index to a BEFORE INSERT trigger.
--
-- WHY THIS CHANGE EXISTS (Batch 67): migration 0018's partial unique
-- index `edge_device_registry_one_active_term` counts EVERY row with
-- status = 'ACTIVE', but the registry is append-only per identity TERM
-- (D-P6.06-3: current status = highest `enrollment_seq`) and the
-- append-only trigger forbids ever changing a stored row's status. So
-- after the rotation sequence ACTIVE (seq 1) -> EXPIRED (seq 2), the
-- seq-1 row still reads ACTIVE and the index refuses the seq-3 ACTIVE
-- term with 23505 — rotation, the ledger's own recorded flow
-- ("Rotation after REVOKED/EXPIRED inserts legally", migration 0018's
-- comment; "EXPIRED → rotation seq+1 + ROTATED event",
-- `routes_edge.rs` step 6), could never insert. The index enforced the
-- rule at the wrong granularity (all history) instead of the current
-- term. Both gated rotation tests failed on it
-- (`edge_enrollment.rs`: "rotation to a new ACTIVE term inserts
-- legally"; `migration_compat_p10_13.rs`: "rotation after REVOKED
-- appends a legal new ACTIVE term").
--
-- THE FIX: drop the index; a BEFORE INSERT trigger enforces the same
-- rule at the right granularity — a new ACTIVE term is refused iff the
-- CURRENT term (highest `enrollment_seq` for the device + tenant) is
-- already ACTIVE. A non-ACTIVE append is always legal (a REVOKED /
-- EXPIRED term retires the current term by superseding it in sequence).
-- Concurrent inserts for one device serialize on a transaction-scoped
-- advisory lock keyed by the device identity (the `source_keys`
-- `advance_source_sequence` pattern: `hashtext` is stable within a
-- database major version; the key is advisory only — a collision across
-- devices would serialize, never corrupt), so the check-then-insert
-- cannot admit two ACTIVE current terms. The refusal raises SQLSTATE
-- 23505 (`unique_violation`) so the adapter's existing mapping still
-- yields the named Conflict (`EdgeEnrollmentStoreError::Conflict`) —
-- no adapter change.
--
-- EXPAND-ONLY (the P11.18 policy, MIGRATION_POLICY.md): drops one
-- index and adds one trigger + function on an existing table — no
-- column, constraint, or grant belonging to any earlier migration is
-- touched; nothing else is dropped or replaced. The down migration
-- restores the index (and drops the trigger + function).
--
-- LOCK-RISK NOTE: `DROP INDEX` / `CREATE TRIGGER` take only brief
-- locks on the table; at application time the table is EMPTY in this
-- program (the workspace is WRITTEN-UNRUN, no deployment exists), so
-- the lock window is bounded by an empty table.

DROP INDEX IF EXISTS edge_device_registry_one_active_term;

CREATE FUNCTION edge_device_registry_one_active_term_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    current_status text;
BEGIN
    IF NEW.status <> 'ACTIVE' THEN
        RETURN NEW;
    END IF;
    -- Serialize same-device inserts for the life of this transaction,
    -- so two concurrent ACTIVE enrollments cannot both pass the
    -- current-term check below.
    PERFORM pg_advisory_xact_lock(
        hashtext(NEW.tenant_id::text || '|' || NEW.device_id)::bigint
    );
    SELECT status INTO current_status
      FROM edge_device_registry
     WHERE tenant_id = NEW.tenant_id AND device_id = NEW.device_id
     ORDER BY enrollment_seq DESC LIMIT 1;
    IF current_status = 'ACTIVE' THEN
        RAISE EXCEPTION 'edge_device_registry: an ACTIVE term already exists for this device identity (P6.06 one-ACTIVE-term rule)'
            USING ERRCODE = '23505';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER edge_device_registry_one_active_term
    BEFORE INSERT ON edge_device_registry
    FOR EACH ROW EXECUTE FUNCTION edge_device_registry_one_active_term_guard();

ALTER FUNCTION edge_device_registry_one_active_term_guard() OWNER TO flexcore_migrator;
