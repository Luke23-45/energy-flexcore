-- P13.09 -- Migration 0020 up: outbound webhook delivery bookkeeping
--          (endpoints, deliveries, booked attempts, dead letters).
--
-- Decision record:
-- docs/phases-11-20/phase-13-enterprise-identity-integration/records/
-- 21_p13_09_webhook_service.md
--
-- Contract source: phase-13 plan s2.3 (the webhook delivery contract) and
-- ledger P13.09. [S-83] Stripe webhook practice is the DESIGN reference
-- (HMAC over timestamp.rawbody, tolerance window, bounded retries,
-- dead-lettering, secret rolling); no FlexCore delivery has ever been made
-- and none is claimed — the schema is bookkeeping storage for the
-- WRITTEN-UNRUN service in apps/control-plane/src/webhook_service.rs.
--
-- What this migration adds (and what it deliberately does NOT):
-- - `webhook_endpoints`: the registered delivery target per tenant, with
--   the CURRENT signing secret and, while a rolling overlap is open, the
--   PREVIOUS one (plan s2.3 "secret rolling" row: both verify during the
--   overlap; the old secret is revoked by clearing the column — the
--   overlap DURATION is deployment configuration, never a column default).
--   Endpoints are mutable CONFIGURATION rows (not evidence): rotation and
--   deactivation are UPDATEs, so flexcore_app holds UPDATE here — the
--   append-only evidence class is untouched (audit evidence remains
--   `audit_records`, migration 0008; the P11.18 protected set is not
--   modified by this migration).
-- - `webhook_deliveries`: one row per delivery IDEMPOTENCY KEY (UNIQUE
--   per tenant). Every booking under an existing key collapses to the
--   existing row; a same-key different-content booking is rejected by the
--   service layer (the UNIQUE constraint is the database-level backstop).
-- - `webhook_delivery_attempts`: the BOOK-BEFORE-ATTEMPT record (plan
--   s2.3 "Retries" row): an attempt row with outcome BOOKED is INSERTed
--   and durable BEFORE the HTTP call is made; the row is then UPDATEd to
--   its outcome. UNIQUE (delivery_id, attempt_seq) keys each attempt.
-- - `webhook_dead_letters`: one row per delivery that exhausted its
--   bounded retries; the depth of this table is the
--   `flexcore_webhook_dead_letters` gauge (rendered by the single P1.17
--   metrics registry) — nothing is silently dropped (plan s2.3).
--
-- What this migration does NOT contain:
-- - NO retention value, NO TTL, NO scheduled-delete rule anywhere (P0.16
--   s4 / report s16.3: no universal retention period in code; delivery
--   bookkeeping retention is the deployment's RetentionPolicy
--   configuration, per jurisdiction via the P13.14 jurisdiction registry
--   — see the phase-13 records 26_p13_14 and 25_p13_13).
-- - NO SECRET MATERIAL and NO secret FORMAT: the `whsec_<43 unpadded
--   base64url>` literal form is defined ONCE by the P13.08 record §4
--   (records/08_p13_08_api_keys_webhook_secret_format.md — the P13.08 ->
--   P13.09 seam, phase-13 plan s2.1) and consumed by citation. The
--   endpoint row carries a PROVIDER REFERENCE + FINGERPRINT per that
--   record's storage rule ("the secret lands in the P11.16 posture ...
--   The database row carries a fingerprint and a state, never the
--   material"); the composition resolves the reference into 32 bytes of
--   signing material through an injected resolver port at attempt time.
-- - NO dispatch authority of any kind: webhook delivery is outbound
--   notification plumbing; nothing here can issue, authorize, or advance
--   a command (the never-granted set dispatch.authorize / dispatch.issue /
--   secret.read / network.any stays byte-identical).
-- - NO destructive ALTER, NO constraint on a populated table, NO
--   CONCURRENTLY index: every statement is additive (four new tables +
--   their indexes — the P11.18 expand-only policy,
--   crates/adapters/postgres/MIGRATION_POLICY.md; CREATE TABLE and
--   CREATE INDEX on new tables take no locks on existing ones).

CREATE TABLE webhook_endpoints (
    endpoint_id          uuid        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id            uuid        NOT NULL REFERENCES tenants (tenant_id),
    url                  text        NOT NULL CHECK (length(btrim(url)) > 0),
    -- The CURRENT signing secret is NOT stored here. Per the P13.08
    -- secret-format record §4 (records/08_p13_08_*.md — the format and
    -- its storage rule are defined ONCE there and consumed here): the
    -- material lands in the P11.16 secrets posture (Vault DEFAULT,
    -- Compose file-per-host floor), and the database row carries an
    -- opaque PROVIDER REFERENCE plus a FINGERPRINT, never the material.
    -- The `whsec_<43 unpadded base64url>` literal form is record 08 §4's
    -- definition; this schema stores only the reference into whatever
    -- store the deployment composed.
    secret_current_ref   text        NOT NULL CHECK (length(btrim(secret_current_ref)) > 0),
    -- The secret fingerprint (the first 8 characters after the `whsec_`
    -- prefix, per record 08 §4) — safe to render in admin surfaces
    -- (P13.18), never the secret.
    secret_current_fingerprint text NOT NULL CHECK (length(btrim(secret_current_fingerprint)) > 0),
    -- The PREVIOUS secret's reference while a rolling overlap is open;
    -- NULL once the overlap closes (the revocation act). Same
    -- reference-not-material rule; the overlap is VALUE state on the row.
    secret_previous_ref  text        CHECK (length(btrim(secret_previous_ref)) > 0),
    secret_previous_fingerprint text CHECK (length(btrim(secret_previous_fingerprint)) > 0),
    active               boolean     NOT NULL DEFAULT true,
    created_at           timestamptz NOT NULL DEFAULT now(),
    updated_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT webhook_endpoints_pk PRIMARY KEY (endpoint_id, tenant_id)
);

CREATE INDEX webhook_endpoints_tenant_idx
    ON webhook_endpoints (tenant_id, active);

CREATE TABLE webhook_deliveries (
    delivery_id          uuid        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id            uuid        NOT NULL REFERENCES tenants (tenant_id),
    endpoint_id          uuid        NOT NULL,
    -- The delivery IDEMPOTENCY KEY (plan s2.3): collapses duplicate
    -- bookings; unique per tenant.
    delivery_key         text        NOT NULL CHECK (length(btrim(delivery_key)) > 0),
    -- The event subject (the frozen flexcore.v1.* subject namespace, P1.14
    -- D-J1) the payload carries.
    event_subject        text        NOT NULL CHECK (length(btrim(event_subject)) > 0),
    -- The RAW request body exactly as transmitted (the signature is
    -- computed over these bytes — never a re-serialization, plan s2.3).
    raw_body             text        NOT NULL,
    -- The closed delivery-status vocabulary (the service state machine).
    status               text        NOT NULL CHECK (
                             status = ANY (ARRAY[
                                 'PENDING'::text,
                                 'IN_FLIGHT'::text,
                                 'DELIVERED'::text,
                                 'DEAD_LETTERED'::text])),
    next_attempt_seq     bigint      NOT NULL CHECK (next_attempt_seq >= 1),
    next_attempt_not_before_unix_millis bigint,
    last_response_code   integer,
    created_at           timestamptz NOT NULL DEFAULT now(),
    updated_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT webhook_deliveries_pk PRIMARY KEY (delivery_id, tenant_id),
    CONSTRAINT webhook_deliveries_key_unique UNIQUE (tenant_id, delivery_key),
    -- The endpoint belongs to the SAME tenant (tenant-scoped FK, the 0018
    -- composite-FK pattern): a delivery can never target a foreign
    -- tenant's endpoint.
    CONSTRAINT webhook_deliveries_endpoint_fk FOREIGN KEY (endpoint_id, tenant_id)
        REFERENCES webhook_endpoints (endpoint_id, tenant_id)
);

CREATE INDEX webhook_deliveries_due_idx
    ON webhook_deliveries (status, next_attempt_not_before_unix_millis);

CREATE TABLE webhook_delivery_attempts (
    attempt_id           uuid        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id            uuid        NOT NULL REFERENCES tenants (tenant_id),
    delivery_id          uuid        NOT NULL,
    attempt_seq          bigint      NOT NULL CHECK (attempt_seq >= 1),
    -- The closed attempt-outcome vocabulary. BOOKED is written BEFORE the
    -- HTTP call (the book-before-attempt rule); the row is then UPDATEd
    -- to its observed outcome.
    outcome              text        NOT NULL CHECK (
                             outcome = ANY (ARRAY[
                                 'BOOKED'::text,
                                 'SUCCESS'::text,
                                 'RETRYABLE_FAILURE'::text,
                                 'FATAL_FAILURE'::text])),
    booked_at_unix_millis     bigint      NOT NULL CHECK (booked_at_unix_millis > 0),
    concluded_at_unix_millis  bigint,
    response_code        integer,
    -- The closed failure-class vocabulary (classes, never payloads —
    -- operators alert on classes; free-text receiver output never becomes
    -- a stored value).
    failure_class        text        CHECK (
                             failure_class IS NULL OR failure_class = ANY (ARRAY[
                                 'TIMEOUT'::text,
                                 'CONNECT_ERROR'::text,
                                 'NON_2XX'::text,
                                 'TLS'::text,
                                 'UNKNOWN'::text])),
    CONSTRAINT webhook_delivery_attempts_pk PRIMARY KEY (attempt_id, tenant_id),
    CONSTRAINT webhook_delivery_attempts_unique_seq UNIQUE (delivery_id, attempt_seq),
    CONSTRAINT webhook_delivery_attempts_delivery_fk FOREIGN KEY (delivery_id, tenant_id)
        REFERENCES webhook_deliveries (delivery_id, tenant_id)
);

CREATE INDEX webhook_delivery_attempts_delivery_idx
    ON webhook_delivery_attempts (delivery_id, attempt_seq);

CREATE TABLE webhook_dead_letters (
    dead_letter_id       uuid        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id            uuid        NOT NULL REFERENCES tenants (tenant_id),
    delivery_id          uuid        NOT NULL,
    endpoint_id          uuid        NOT NULL,
    -- Why the delivery exhausted (the closed failure classes plus the
    -- exhaustion case itself).
    reason               text        NOT NULL CHECK (
                             reason = ANY (ARRAY[
                                 'ATTEMPTS_EXHAUSTED'::text,
                                 'TIMEOUT'::text,
                                 'CONNECT_ERROR'::text,
                                 'NON_2XX'::text,
                                 'TLS'::text,
                                 'UNKNOWN'::text])),
    dead_lettered_at_unix_millis bigint NOT NULL CHECK (dead_lettered_at_unix_millis > 0),
    -- Operator resolution (re-drive/dismissal) marks WHEN, never deletes:
    -- a resolved dead letter is still a row (nothing silently dropped).
    resolved_at_unix_millis      bigint,
    CONSTRAINT webhook_dead_letters_pk PRIMARY KEY (dead_letter_id, tenant_id),
    CONSTRAINT webhook_dead_letters_one_per_delivery UNIQUE (delivery_id),
    CONSTRAINT webhook_dead_letters_delivery_fk FOREIGN KEY (delivery_id, tenant_id)
        REFERENCES webhook_deliveries (delivery_id, tenant_id)
);

CREATE INDEX webhook_dead_letters_open_idx
    ON webhook_dead_letters (resolved_at_unix_millis)
    WHERE resolved_at_unix_millis IS NULL;

-- Ownership, grants, RLS — the exact repo conventions (migrations 0008 /
-- 0010 / 0018): owner flexcore_migrator; runtime flexcore_app holds
-- INSERT + SELECT (+ UPDATE where the state machine requires it) on
-- tenant-scoped bookkeeping tables; RLS ENABLE + FORCE with the shared
-- tenant_isolation policy. These four tables are NOT the append-only
-- evidence class (that is `audit_records`, 0008 + 0019 chain): they carry
-- UPDATE by design and no defence-in-depth append-only trigger is added.

ALTER TABLE webhook_endpoints OWNER TO flexcore_migrator;
REVOKE ALL ON webhook_endpoints FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON webhook_endpoints TO flexcore_app;
ALTER TABLE webhook_endpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_endpoints FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON webhook_endpoints
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE webhook_deliveries OWNER TO flexcore_migrator;
REVOKE ALL ON webhook_deliveries FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON webhook_deliveries TO flexcore_app;
ALTER TABLE webhook_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_deliveries FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON webhook_deliveries
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE webhook_delivery_attempts OWNER TO flexcore_migrator;
REVOKE ALL ON webhook_delivery_attempts FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON webhook_delivery_attempts TO flexcore_app;
ALTER TABLE webhook_delivery_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_delivery_attempts FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON webhook_delivery_attempts
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE webhook_dead_letters OWNER TO flexcore_migrator;
REVOKE ALL ON webhook_dead_letters FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON webhook_dead_letters TO flexcore_app;
ALTER TABLE webhook_dead_letters ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_dead_letters FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON webhook_dead_letters
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
