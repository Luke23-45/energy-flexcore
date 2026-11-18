-- Patch B (P2.07) -- Migration 0026 up: the Ed25519 source-signing-key
--          registry and the append-only sequence-watermark events
--          (the P2.07 signed-envelope source authentication).
--
-- What this migration adds (and what it deliberately does NOT):
-- - The SOURCE-SIGNING-KEY REGISTRY (`source_signing_keys`): one row per
--   ISSUED key term. The registry is APPEND-ONLY per term — rotation
--   appends a NEW row for a NEW `key_id` (the old `key_id` stays valid
--   until its `valid_until`), and revocation appends a REVOKED term with
--   a higher `term_seq` for the same (tenant, source, key_id). A source's
--   CURRENT key term is its highest-`term_seq` row. This keeps the whole
--   key lifecycle expressible with INSERT + SELECT grants only (no UPDATE
--   grant on the runtime role), matching the 0018 append-only pattern.
-- - The APPEND-ONLY SEQUENCE-WATERMARK EVENTS (`source_sequence_events`):
--   one row per accepted envelope sequence, per (tenant, source). The
--   watermark is the MAX(sequence); it moves only by INSERT, never by
--   UPDATE, so a replayed or stale sequence can never overwrite the
--   watermark — the advance re-checks MAX under a per-source advisory
--   lock inside the ingestion batch transaction.
-- - What this migration does NOT contain: NO private-key material, NO
--   secret columns of any kind. The registry holds PUBLIC (verifying)
--   keys only — 32-byte Ed25519 verifying keys as `bytea`. Private keys
--   live only in the producer that signs (the simulator's ephemeral
--   dev keypair lives in process memory and is never written to disk or
--   configuration); enrolled production keys are issued and held outside
--   this database.
--
-- Tenancy and immutability, in the exact repo conventions:
-- - tenant_id on every row (§16.2); RLS ENABLE + FORCE with the shared
--   tenant_isolation policy (owner flexcore_migrator, runtime
--   flexcore_app, P1.12 §5.1 conventions).
-- - flexcore_app holds INSERT + SELECT only on both tables.
-- - Defence-in-depth UPDATE/DELETE triggers refuse unconditionally
--   (including for the owner — the 0008 audit_records pattern), because
--   BOTH tables are append-only: a key term is never edited; a new
--   lifecycle state is a new row / a new sequence event.
-- - The registry PRIMARY KEY (tenant_id, source_id, key_id, term_seq)
--   scopes each key term per tenant; the UNIQUE partial index on
--   (tenant_id, source_id, key_id) FILTERED to status = 'ACTIVE'
--   enforces at most ONE ACTIVE term per key identity at the database
--   level (a second ACTIVE term for the same key is impossible; a
--   REVOKED/EXPIRED latest term may be replaced by a new ACTIVE row).
-- - No retention value, no TTL, no scheduled delete (P0.16 §4): signing
--   keys and sequence evidence are identity evidence; retention is the
--   deployment's `RetentionPolicy` configuration (report §16.3), never
--   a period in code.

CREATE TABLE source_signing_keys (
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    source_id              text        NOT NULL CHECK (length(btrim(source_id)) > 0),
    key_id                 text        NOT NULL CHECK (length(btrim(key_id)) > 0),
    term_seq               bigint      NOT NULL CHECK (term_seq >= 1),
    public_key             bytea       NOT NULL CHECK (length(public_key) = 32),
    status                 text        NOT NULL CHECK (
                               status = ANY (ARRAY[
                                   'ACTIVE'::text,
                                   'REVOKED'::text,
                                   'EXPIRED'::text])),
    valid_from_unix_millis bigint      NOT NULL CHECK (valid_from_unix_millis > 0),
    valid_until_unix_millis bigint     NULL,
    enrolled_at_unix_millis bigint     NOT NULL CHECK (enrolled_at_unix_millis > 0),
    enrolled_by            text        NOT NULL DEFAULT 'system',
    CONSTRAINT source_signing_keys_pk
        PRIMARY KEY (tenant_id, source_id, key_id, term_seq),
    -- Validity-window shape: an expiry, when present, must be after the
    -- issuance instant.
    CONSTRAINT source_signing_keys_validity_shape CHECK (
        valid_until_unix_millis IS NULL
        OR valid_until_unix_millis > valid_from_unix_millis)
);

CREATE INDEX source_signing_keys_current_idx
    ON source_signing_keys (tenant_id, source_id, key_id, term_seq DESC);

-- THE one-active-term rule: at most ONE ACTIVE key term per
-- (tenant, source, key_id). Rotation after REVOKED/EXPIRED inserts
-- legally under the partial index; a second ACTIVE term for the same
-- key identity is impossible.
CREATE UNIQUE INDEX source_signing_keys_one_active_term
    ON source_signing_keys (tenant_id, source_id, key_id)
    WHERE status = 'ACTIVE'::text;

-- Append-only defence in depth (the 0008 audit_records pattern layer 2):
-- UPDATE and DELETE raise unconditionally, including for the table owner.
CREATE FUNCTION source_signing_keys_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'source_signing_keys is append-only: UPDATE and DELETE are refused (Patch B, P2.07); lifecycle changes append a NEW term row';
END $$;

CREATE TRIGGER source_signing_keys_append_only
    BEFORE UPDATE OR DELETE ON source_signing_keys
    FOR EACH ROW
    EXECUTE FUNCTION source_signing_keys_append_only_guard();

COMMENT ON TABLE source_signing_keys IS
    'Ed25519 source-signing-key registry (Patch B, P2.07): PUBLIC verifying keys only, never private keys. One row per issued key term; rotation is a new key_id row, revocation is a REVOKED term at a higher term_seq.';

ALTER TABLE source_signing_keys OWNER TO flexcore_migrator;
ALTER FUNCTION source_signing_keys_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON source_signing_keys FROM PUBLIC;
GRANT SELECT, INSERT ON source_signing_keys TO flexcore_app;

ALTER TABLE source_signing_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE source_signing_keys FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON source_signing_keys
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

CREATE TABLE source_sequence_events (
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    source_id              text        NOT NULL CHECK (length(btrim(source_id)) > 0),
    sequence               bigint      NOT NULL CHECK (sequence >= 1),
    recorded_at_unix_millis bigint     NOT NULL CHECK (recorded_at_unix_millis > 0),
    CONSTRAINT source_sequence_events_pk
        PRIMARY KEY (tenant_id, source_id, sequence)
);

-- Append-only defence in depth (the 0008 audit_records pattern layer 2):
-- UPDATE and DELETE raise unconditionally, including for the table owner.
CREATE FUNCTION source_sequence_events_append_only_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'source_sequence_events is append-only: UPDATE and DELETE are refused (Patch B, P2.07); the watermark moves only by INSERT';
END $$;

CREATE TRIGGER source_sequence_events_append_only
    BEFORE UPDATE OR DELETE ON source_sequence_events
    FOR EACH ROW
    EXECUTE FUNCTION source_sequence_events_append_only_guard();

ALTER TABLE source_sequence_events OWNER TO flexcore_migrator;
ALTER FUNCTION source_sequence_events_append_only_guard() OWNER TO flexcore_migrator;

REVOKE ALL ON source_sequence_events FROM PUBLIC;
GRANT SELECT, INSERT ON source_sequence_events TO flexcore_app;

ALTER TABLE source_sequence_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE source_sequence_events FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON source_sequence_events
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
