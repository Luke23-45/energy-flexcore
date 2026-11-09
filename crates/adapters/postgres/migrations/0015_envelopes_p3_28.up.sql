-- P3.28 -- Migration 0015 up: portfolio flexibility-envelope records
--          (report §11.6, §16.1, §16.2; ledger P3.28 step 3).
--
-- Phase 3 computed the portfolio flexibility envelope (P3.12) and froze its
-- event assembly, but had no persistence surface: this migration closes the
-- gap following the exact 0013/0014 conventions (owner flexcore_migrator,
-- runtime flexcore_app, RLS ENABLE + FORCE + the shared tenant_isolation
-- policy shape; the application must never hold owner credentials [R25]).
--
-- Design (recorded in ledger/phase-03-flexibility-scheduling/28_api_wiring.md):
--
-- - envelope_id is the PRIMARY KEY (a caller-supplied UUIDv7, so identifier
--   order is a stable time order for the keyset listing — the recorded
--   P2.12 rationale). Records are IMMUTABLE — a recomputation publishes a
--   NEW envelope version, never an overwrite — so the runtime grants are
--   INSERT/SELECT only (the P2.06 append-only grant discipline).
-- - (tenant_id, portfolio_id, envelope_version) is UNIQUE: the version
--   uniqueness rule makes a silent overwrite unrepresentable at rest.
-- - The structured columns carry the identity and horizon fields a reader
--   filters on; the FULL envelope computation result (the P3.12
--   PortfolioFlexibilityEnvelope as rendered by the composition root)
--   travels in envelope_payload jsonb — opaque to the storage crate,
--   validated as JSON on insert by the adapter.
-- - covered_site_ids is a jsonb array of canonical UUID texts (one
--   FlexibilityEnvelopeUpdated event per site, in list order); the adapter
--   validates every element as a canonical UUID text before the insert.
-- - The composite FK to portfolios(portfolio_id, tenant_id) keeps the
--   denormalized tenant provably consistent (the 0013 pattern).
-- - tenant_id is part of the record (§16.2: tenant_id on every tenant-owned
--   record).
-- - No retention value, no TTL, no scheduled delete appears here (P0.16 §4:
--   retention is configurable, never universal).

CREATE TABLE flexibility_envelopes (
    envelope_id          uuid        PRIMARY KEY,
    tenant_id            uuid        NOT NULL REFERENCES tenants (tenant_id),
    portfolio_id         uuid        NOT NULL,
    envelope_version     text        NOT NULL CHECK (length(btrim(envelope_version)) > 0),
    horizon_start_millis bigint      NOT NULL,
    horizon_end_millis   bigint      NOT NULL CHECK (horizon_end_millis > horizon_start_millis),
    interval_millis      bigint      NOT NULL CHECK (interval_millis > 0),
    interval_count       integer     NOT NULL CHECK (interval_count >= 1),
    covered_site_ids     jsonb       NOT NULL
        CHECK (jsonb_typeof(covered_site_ids) = 'array' AND jsonb_array_length(covered_site_ids) >= 1),
    envelope_payload     jsonb       NOT NULL,
    created_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT flexibility_envelopes_portfolio_tenant_fk
        FOREIGN KEY (portfolio_id, tenant_id)
        REFERENCES portfolios (portfolio_id, tenant_id),
    CONSTRAINT flexibility_envelopes_version_unique
        UNIQUE (tenant_id, portfolio_id, envelope_version)
);

ALTER TABLE flexibility_envelopes OWNER TO flexcore_migrator;

REVOKE ALL ON flexibility_envelopes FROM PUBLIC;
-- Immutable records: INSERT + SELECT only.
GRANT SELECT, INSERT ON flexibility_envelopes TO flexcore_app;

ALTER TABLE flexibility_envelopes ENABLE ROW LEVEL SECURITY;
ALTER TABLE flexibility_envelopes FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON flexibility_envelopes
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
