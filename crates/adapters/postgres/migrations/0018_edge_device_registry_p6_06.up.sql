-- P6.06 -- Migration 0018 up: edge device-identity registry and the
--          append-only enrollment journal (report §18.1, §18.3, §17.3,
--          §16.2; ledger P6.06).
--
-- Decision record:
-- ledger/phase-06-edge-protocol/06_07_identity_channel.md
--
-- What this migration adds (and what it deliberately does NOT):
-- - The EDGE DEVICE-IDENTITY REGISTRY (`edge_device_registry`): one row
--   per ISSUED identity term. The registry is APPEND-ONLY per term —
--   rotation and expiry append NEW rows at the next `enrollment_seq`, and
--   a device's CURRENT status is its highest-sequence row. This keeps the
--   whole identity lifecycle expressible with INSERT + SELECT grants only
--   (no UPDATE grant on the runtime role), matching the 0008/0017
--   append-only pattern and the ledger P6.06 lifecycle (enroll, rotate,
--   revoke, expire) as an event-sourced identity.
-- - The APPEND-ONLY ENROLLMENT JOURNAL (`edge_enrollment_journal`): every
--   §17.3 identity audit event — ENROLLED / ROTATED / REVOKED / EXPIRED /
--   REJECTED — with actor, reason, and correlation id. REJECTED events may
--   name a device that was never enrolled, so the journal carries NO
--   registry foreign key (the choice is documented here, not implicit);
--   its only foreign key is the tenant.
-- - What this migration does NOT contain: NO key material, NO certificate
--   PEM columns, NO secrets. The device identity is bookkeeping (ids,
--   binding, status, validity window). Certificate issuance and the CA,
--   and OCSP/CRL revocation checking, are DEPLOYMENT REQUIREMENTS — the
--   decision record §3 records them honestly; nothing here implements a
--   PKI.
--
-- Tenancy and immutability, in the exact repo conventions:
-- - tenant_id on every row (§16.2); RLS ENABLE + FORCE with the shared
--   tenant_isolation policy (owner flexcore_migrator, runtime
--   flexcore_app, P1.12 §5.1 conventions).
-- - flexcore_app holds INSERT + SELECT only on both tables.
-- - Defence-in-depth UPDATE/DELETE triggers refuse unconditionally
--   (including for the owner — the 0008 audit_records pattern), because
--   BOTH tables are append-only: an identity term is never edited; a new
--   lifecycle state is a new row / new journal event.
-- - The registry PRIMARY KEY (device_id, tenant_id, enrollment_seq)
--   scopes each identity term per tenant; the UNIQUE constraint on
--   (device_id, tenant_id) FILTERED to status = 'ACTIVE' enforces the
--   ledger acceptance criterion "no two edges share an identity" at the
--   database level (a second ACTIVE term for the same device identity is
--   impossible; a REVOKED/EXPIRED latest term may be rotated by a new
--   ACTIVE row).
-- - site_id binds the identity to exactly one approved site (§16.2) via
--   the composite tenant-scoped FK to `sites`.
-- - No retention value, no TTL, no scheduled delete (P0.16 §4): identity
--   evidence retention is the deployment's `RetentionPolicy` configuration
--   (report §16.3), never a period in code.

CREATE TABLE edge_device_registry (
    device_id              text        NOT NULL CHECK (length(btrim(device_id)) > 0),
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    site_id                uuid        NOT NULL,
    enrollment_seq         bigint      NOT NULL CHECK (enrollment_seq >= 1),
    status                 text        NOT NULL CHECK (
                               status = ANY (ARRAY[
                                   'ACTIVE'::text,
                                   'REVOKED'::text,
                                   'EXPIRED'::text])),
    approved_asset_scope   text        NULL,
    enrolled_at_unix_millis bigint     NOT NULL CHECK (enrolled_at_unix_millis > 0),
    expires_at_unix_millis  bigint     NULL,
    recorded_at            timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT edge_device_registry_pk
        PRIMARY KEY (device_id, tenant_id, enrollment_seq),
    CONSTRAINT edge_device_registry_site_fk
        FOREIGN KEY (site_id, tenant_id)
        REFERENCES sites (site_id, tenant_id),
    -- Validity-window shape: an expiry, when present, must be after the
    -- issuance instant.
    CONSTRAINT edge_device_registry_expiry_shape CHECK (
        expires_at_unix_millis IS NULL
        OR expires_at_unix_millis > enrolled_at_unix_millis),
    -- THE identity-uniqueness rule (ledger P6.06 acceptance: no two edges
    -- share an identity): at most ONE ACTIVE identity term per
    -- (device_id, tenant_id), enforced by the PARTIAL UNIQUE INDEX created
    -- below (a unique constraint cannot carry a WHERE clause; rotation
    -- after REVOKED/EXPIRED inserts legally under the partial index).
);

CREATE INDEX edge_device_registry_current_idx
    ON edge_device_registry (tenant_id, device_id, enrollment_seq DESC);

-- THE identity-uniqueness rule (ledger P6.06 acceptance: no two edges
-- share an identity): at most ONE ACTIVE identity term per
-- (device_id, tenant_id). Rotation after REVOKED/EXPIRED inserts legally.
CREATE UNIQUE INDEX edge_device_registry_one_active_term
    ON edge_device_registry (device_id, tenant_id)
    WHERE status = 'ACTIVE'::text;

-- Append-only defence in depth (the 0008 audit_records pattern layer 2):
-- UPDATE and DELETE raise unconditionally, including for the table owner.
CREATE FUNCTION edge_device_registry_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'edge_device_registry is append-only: UPDATE and DELETE are refused (P6.06, report s16.1); lifecycle changes append a NEW term row';
END $$;

CREATE TRIGGER edge_device_registry_append_only
    BEFORE UPDATE OR DELETE ON edge_device_registry
    FOR EACH ROW
    EXECUTE FUNCTION edge_device_registry_append_only_guard();

ALTER TABLE edge_device_registry OWNER TO flexcore_migrator;
ALTER FUNCTION edge_device_registry_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON edge_device_registry FROM PUBLIC;
GRANT SELECT, INSERT ON edge_device_registry TO flexcore_app;

ALTER TABLE edge_device_registry ENABLE ROW LEVEL SECURITY;
ALTER TABLE edge_device_registry FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON edge_device_registry
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

CREATE TABLE edge_enrollment_journal (
    event_id               uuid        NOT NULL,
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    device_id              text        NOT NULL CHECK (length(btrim(device_id)) > 0),
    event_type             text        NOT NULL CHECK (
                               event_type = ANY (ARRAY[
                                   'ENROLLED'::text,
                                   'ROTATED'::text,
                                   'REVOKED'::text,
                                   'EXPIRED'::text,
                                   'REJECTED'::text])),
    actor                  text        NOT NULL CHECK (length(btrim(actor)) > 0),
    reason                 text        NULL,
    correlation_id         text        NOT NULL CHECK (length(btrim(correlation_id)) > 0),
    occurred_at_unix_millis bigint     NOT NULL CHECK (occurred_at_unix_millis > 0),
    recorded_at            timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT edge_enrollment_journal_pk
        PRIMARY KEY (event_id, tenant_id),
    -- NO registry foreign key, deliberately: a REJECTED event names a
    -- device that may never have enrolled; the journal is the §17.3
    -- identity audit trail, tenant-scoped, not a registry join table.
    -- Rejection-shape rule: a REJECTED event carries a reason; the other
    -- event types may leave it NULL.
    CONSTRAINT edge_enrollment_journal_rejected_shape CHECK (
        event_type <> 'REJECTED'::text OR reason IS NOT NULL)
);

CREATE INDEX edge_enrollment_journal_device_idx
    ON edge_enrollment_journal (tenant_id, device_id, occurred_at_unix_millis);

CREATE FUNCTION edge_enrollment_journal_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'edge_enrollment_journal is append-only: UPDATE and DELETE are refused (P6.06, report s16.1; s17.3 audit trail)';
END $$;

CREATE TRIGGER edge_enrollment_journal_append_only
    BEFORE UPDATE OR DELETE ON edge_enrollment_journal
    FOR EACH ROW
    EXECUTE FUNCTION edge_enrollment_journal_append_only_guard();

ALTER TABLE edge_enrollment_journal OWNER TO flexcore_migrator;
ALTER FUNCTION edge_enrollment_journal_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON edge_enrollment_journal FROM PUBLIC;
GRANT SELECT, INSERT ON edge_enrollment_journal TO flexcore_app;

ALTER TABLE edge_enrollment_journal ENABLE ROW LEVEL SECURITY;
ALTER TABLE edge_enrollment_journal FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON edge_enrollment_journal
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
