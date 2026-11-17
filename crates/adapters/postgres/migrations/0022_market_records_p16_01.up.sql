-- P16.01 -- Migration 0022 up: the market-records store (programs,
--           contracts, enrollments, market events, deliveries, settlements,
--           and the market-record retention configuration rows).
--
-- Decision record:
-- docs/phases-11-20/phase-16-market-integration-settlement/records/
-- 01_p16_01_market_records_store.md
--
-- What this closes (at DESIGN level, honestly stated): the LR-074/LR-190
-- gap — the §16.3 market-records class was recorded DOES NOT EXIST
-- (`ledger/phase-09-pilot-qualification/22_market_operational_procedures.md`
-- §5; `10_data_residency_retention.md` §5.C). This migration adds the
-- SCHEMA only; no market is connected, no registration is filed, no
-- settlement statement is received, and no qualification is claimed
-- anywhere in this program (ADR-011; program conventions §7.3). Every
-- table starts EMPTY and stays empty until a recorded, jurisdiction-named
-- (D1-class, OPEN) integration writes to it.
--
-- ADR-011 boundary: these are jurisdiction-specific RECORDS, not a
-- generic engine. Market-structure knowledge (a two-ID CAISO load-shift,
-- a single PJM aggregation node, a 15-minute ISP) lives in record packs
-- and declared data, never in claimed-universal schema semantics; the
-- declared-text columns (jurisdiction, event_kind, line_item_kind,
-- source_format additions) are the additive, per-jurisdiction surface.
--
-- Audit-chain linkage (the P11 chain is CONSUMED, never redefined):
-- every record row carries `audit_record_id uuid NOT NULL` — the P11
-- `audit_records` row (migration 0008 + the P11.15 chain columns 0019)
-- that the caller appends THROUGH THE P11 SINK in the SAME unit of work.
-- The chain treatment (keyed HMAC-SHA-256 linear chain, per-tenant
-- namespace, NULL-while-unkeyed semantics, LR-203 production-crypto
-- requirement) is audit_records' own; these tables store the LINK to it
-- and no chain columns of their own. The write-path obligation is stated
-- on the storage trait (`flexcore_storage::market_records_store`) and in
-- the adapter; no FK is created to audit_records (the linkage is the
-- recorded reference; the audit table is append-only and tenant-scoped,
-- and a cross-migration FK would couple migration units).
--
-- Immutability model: every record table is append-only. A lifecycle
-- transition or a corrected statement is a NEW row carrying a
-- `supersedes_*_id` link to the row it replaces (the P4.10 D-P3.21-8 /
-- D-P4.10-3 supersede discipline, mirrored for financial records); the
-- runtime grants are INSERT + SELECT only, so a silent overwrite of a
-- financial record is unrepresentable at rest. Readers derive currency
-- ("which row is current") from the supersede links, never from an
-- UPDATE.
--
-- Money discipline (plan §4 non-goals; hard ban 9): settlement amounts
-- are explicit DECIMAL TEXT (`amount_text`) plus a `currency_code` that
-- is DATA (a three-letter code; ISO 4217 as recorded data, re-verified at
-- use). NO money column is numeric, no float money arithmetic is
-- representable, and no settlement total, revenue figure, invoice, or
-- payment is computed anywhere in this migration.
--
-- Retention rows (P0.16 §4): retention is the deployment's configuration,
-- never universal. The `market_record_retention_config` table stores the
-- per-tenant, per-record-class configuration rows, every row starting its
-- life UNSET where the deployment has not decided; a CONFIGURED row must
-- carry its period AND its recorded basis. No default period exists
-- anywhere; enforcement (partition drops, deletion jobs) is infrastructur
-- e work this migration does not contain. Note: the MV_REPORT and
-- REGISTRATION classes name the P16.05 M&V reports and the P16.03
-- registration records — financial-record-adjacent classes that inherit
-- the same retention treatment.
--
-- Tenancy: these are tenant-owned commercial records. Every table is
-- tenant-scoped with the exact P1.12 §5.1 RLS conventions (owner
-- flexcore_migrator, runtime flexcore_app, ENABLE + FORCE + the shared
-- tenant_isolation policy shape — the 0016 pattern).
--
-- Expand-only (the P11.18 policy, `crates/adapters/postgres/
-- MIGRATION_POLICY.md`): every statement here is additive CREATE TABLE /
-- CREATE INDEX on NEW, EMPTY tables (always safe per policy §2) + grants.
-- No ALTER touches an existing table, no constraint scans a populated
-- table, nothing destructive.

CREATE TABLE market_programs (
    program_id            uuid        NOT NULL,
    tenant_id             uuid        NOT NULL REFERENCES tenants (tenant_id),
    program_name          text        NOT NULL CHECK (length(btrim(program_name)) > 0 AND length(program_name) <= 256),
    jurisdiction          text        NOT NULL CHECK (length(btrim(jurisdiction)) > 0 AND length(jurisdiction) <= 64),
    state                 text        NOT NULL CHECK (
                              state = ANY (ARRAY[
                                  'DECLARED'::text,
                                  'ACTIVE'::text,
                                  'SUSPENDED'::text,
                                  'RETIRED'::text])),
    program_config        jsonb       NOT NULL,
    supersedes_program_id uuid        NULL,
    audit_record_id       uuid        NOT NULL,
    recorded_at_unix_ms   bigint      NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by           text        NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at            timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_programs_pk PRIMARY KEY (program_id, tenant_id),
    -- The supersede link (the immutability model): a NEW row names the row
    -- it replaces; the current program state is the latest non-superseded
    -- row. Never self-referential.
    CONSTRAINT market_programs_supersedes_fk
        FOREIGN KEY (supersedes_program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    CONSTRAINT market_programs_no_self_supersede CHECK (
        supersedes_program_id IS NULL
        OR supersedes_program_id <> program_id)
);

-- The program's declared rules (its OWN baseline-method declaration —
-- LR-073: the method is the program's, never FlexCore's) travel in
-- program_config as recorded JSON. The database check enforces JSON-ness
-- only; the adapter validates it parses and carries a non-empty object.

CREATE INDEX market_programs_tenant_idx
    ON market_programs (tenant_id);

CREATE TABLE market_contracts (
    contract_id            uuid        NOT NULL,
    tenant_id              uuid        NOT NULL REFERENCES tenants (tenant_id),
    program_id             uuid        NOT NULL,
    contract_ref           text        NOT NULL CHECK (length(btrim(contract_ref)) > 0 AND length(contract_ref) <= 256),
    state                  text        NOT NULL CHECK (
                               state = ANY (ARRAY[
                                   'RECORDED'::text,
                                   'ACTIVE'::text,
                                   'TERMINATED'::text])),
    terms                  jsonb       NOT NULL,
    supersedes_contract_id uuid        NULL,
    audit_record_id        uuid        NOT NULL,
    recorded_at_unix_ms    bigint      NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by            text        NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at             timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_contracts_pk PRIMARY KEY (contract_id, tenant_id),
    -- A contract exists only under its program (same tenant scope).
    CONSTRAINT market_contracts_program_fk
        FOREIGN KEY (program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    CONSTRAINT market_contracts_supersedes_fk
        FOREIGN KEY (supersedes_contract_id, tenant_id)
        REFERENCES market_contracts (contract_id, tenant_id),
    CONSTRAINT market_contracts_no_self_supersede CHECK (
        supersedes_contract_id IS NULL
        OR supersedes_contract_id <> contract_id)
);

CREATE INDEX market_contracts_program_idx
    ON market_contracts (tenant_id, program_id);

CREATE TABLE market_enrollments (
    enrollment_id            uuid        NOT NULL,
    tenant_id                uuid        NOT NULL REFERENCES tenants (tenant_id),
    program_id               uuid        NOT NULL,
    asset_id                 uuid        NOT NULL,
    state                    text        NOT NULL CHECK (
                                 state = ANY (ARRAY[
                                     'REQUESTED'::text,
                                     'ACCEPTED'::text,
                                     'PARTICIPATING'::text,
                                     'SUSPENDED'::text,
                                     'WITHDRAWN'::text,
                                     'REFUSED'::text])),
    -- The per-asset participation window (the domain P16.02 types carry
    -- the closed semantics; NULL = no window declared yet). End-exclusive
    -- (the §10.3 convention).
    window_start_millis      bigint      NULL CHECK (window_start_millis IS NULL OR window_start_millis >= 0),
    window_end_millis        bigint      NULL CHECK (window_end_millis IS NULL OR window_end_millis >= 0),
    qualification_evidence   jsonb       NOT NULL
                                 CHECK (jsonb_typeof(qualification_evidence) = 'array'),
    supersedes_enrollment_id uuid        NULL,
    audit_record_id          uuid        NOT NULL,
    recorded_at_unix_ms      bigint      NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by              text        NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at               timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_enrollments_pk PRIMARY KEY (enrollment_id, tenant_id),
    CONSTRAINT market_enrollments_program_fk
        FOREIGN KEY (program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    CONSTRAINT market_enrollments_asset_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id),
    -- A declared window is bounded on both ends and never inverted.
    CONSTRAINT market_enrollments_window_pair CHECK (
        (window_start_millis IS NULL AND window_end_millis IS NULL)
        OR (window_start_millis IS NOT NULL AND window_end_millis IS NOT NULL
            AND window_end_millis > window_start_millis)),
    CONSTRAINT market_enrollments_supersedes_fk
        FOREIGN KEY (supersedes_enrollment_id, tenant_id)
        REFERENCES market_enrollments (enrollment_id, tenant_id),
    CONSTRAINT market_enrollments_no_self_supersede CHECK (
        supersedes_enrollment_id IS NULL
        OR supersedes_enrollment_id <> enrollment_id)
);

CREATE INDEX market_enrollments_program_idx
    ON market_enrollments (tenant_id, program_id);

CREATE INDEX market_enrollments_asset_idx
    ON market_enrollments (tenant_id, asset_id);

CREATE TABLE market_events (
    event_id           uuid        NOT NULL,
    tenant_id          uuid        NOT NULL REFERENCES tenants (tenant_id),
    program_id         uuid        NOT NULL,
    external_ref       text        NOT NULL CHECK (length(btrim(external_ref)) > 0 AND length(external_ref) <= 256),
    -- The event kind is DECLARED by the jurisdiction record pack / program
    -- (ADR-011: jurisdiction-specific records — no universal event
    -- taxonomy is invented here).
    event_kind         text        NOT NULL CHECK (length(btrim(event_kind)) > 0 AND length(event_kind) <= 64),
    start_millis       bigint      NOT NULL CHECK (start_millis >= 0),
    end_millis         bigint      NOT NULL CHECK (end_millis >= 0),
    event_payload      jsonb       NOT NULL,
    audit_record_id    uuid        NOT NULL,
    recorded_at_unix_ms bigint     NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by        text        NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at         timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_events_pk PRIMARY KEY (event_id, tenant_id),
    CONSTRAINT market_events_program_fk
        FOREIGN KEY (program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    CONSTRAINT market_events_window CHECK (end_millis > start_millis)
);

CREATE INDEX market_events_program_idx
    ON market_events (tenant_id, program_id);

CREATE TABLE market_deliveries (
    delivery_id         uuid         NOT NULL,
    tenant_id           uuid         NOT NULL REFERENCES tenants (tenant_id),
    program_id          uuid         NOT NULL,
    event_id            uuid         NOT NULL,
    asset_id            uuid         NOT NULL,
    interval_start_millis bigint     NOT NULL CHECK (interval_start_millis >= 0),
    interval_end_millis   bigint     NOT NULL CHECK (interval_end_millis >= 0),
    -- The measured delivery magnitude for the interval, in MWh with the
    -- platform's P > 0 = export-at-PCC sign convention (canonical units;
    -- a measurement is recorded AS MEASURED, including a negative value
    -- where the measurement says so — no positivity CHECK).
    delivered_energy_mwh double precision NOT NULL CHECK (delivered_energy_mwh <> 'NaN'::float8 AND delivered_energy_mwh <> 'Infinity'::float8 AND delivered_energy_mwh <> '-Infinity'::float8),
    -- The §10.4 telemetry quality state of the value the delivery record
    -- carries (the frozen quality vocabulary is the single source of
    -- truth; a quality flag is never silently converted — the state
    -- travels WITH the number).
    measured_quality    text         NOT NULL CHECK (
                            measured_quality = ANY (ARRAY[
                                'GOOD'::text,
                                'SUSPECT'::text,
                                'BAD'::text,
                                'MISSING'::text,
                                'ESTIMATED'::text,
                                'STALE'::text])),
    audit_record_id     uuid         NOT NULL,
    recorded_at_unix_ms bigint       NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by         text         NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at          timestamptz  NOT NULL DEFAULT now(),
    CONSTRAINT market_deliveries_pk PRIMARY KEY (delivery_id, tenant_id),
    CONSTRAINT market_deliveries_program_fk
        FOREIGN KEY (program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    CONSTRAINT market_deliveries_event_fk
        FOREIGN KEY (event_id, tenant_id)
        REFERENCES market_events (event_id, tenant_id),
    CONSTRAINT market_deliveries_asset_fk
        FOREIGN KEY (asset_id, tenant_id)
        REFERENCES assets (asset_id, tenant_id),
    CONSTRAINT market_deliveries_interval CHECK (interval_end_millis > interval_start_millis)
);

CREATE INDEX market_deliveries_event_idx
    ON market_deliveries (tenant_id, event_id);

CREATE TABLE market_settlements (
    settlement_id           uuid        NOT NULL,
    tenant_id               uuid        NOT NULL REFERENCES tenants (tenant_id),
    program_id              uuid        NOT NULL,
    statement_ref           text        NOT NULL CHECK (length(btrim(statement_ref)) > 0 AND length(statement_ref) <= 256),
    -- The statement format the record was transcribed from (closed at
    -- design level to the formats the plan names; additions are additive
    -- migration decisions, never silent): PJM Manual 29 CSV/XML + MSRS/BLI
    -- [S-93], CAISO 5-minute SQMD [S-97], EBGL 15-minute ISP imbalance
    -- [S-107][S-108][S-109], or a program-declared format.
    source_format           text        NOT NULL CHECK (
                                source_format = ANY (ARRAY[
                                    'PJM_MANUAL_29'::text,
                                    'CAISO_SQMD'::text,
                                    'EBGL_ISP'::text,
                                    'PROGRAM_DECLARED'::text])),
    period_start_millis     bigint      NOT NULL CHECK (period_start_millis >= 0),
    period_end_millis       bigint      NOT NULL CHECK (period_end_millis >= 0),
    -- The statement's own line-item kind (an MSRS/BLI name, an SQMD
    -- charge type, an ISP component — RECORDED DATA, never interpreted
    -- into a universal taxonomy by this schema).
    line_item_kind          text        NOT NULL CHECK (length(btrim(line_item_kind)) > 0 AND length(line_item_kind) <= 128),
    -- Money discipline: the amount is EXPLICIT DECIMAL TEXT exactly as
    -- transcribed (no float conversion anywhere), and the currency is
    -- DATA — a three-letter code whose ISO 4217 meaning is recorded data,
    -- re-verified at use, NOT a typed unit (the contracts crate has no
    -- currency unit; plan §4).
    amount_text             text        NOT NULL CHECK (length(btrim(amount_text)) > 0 AND length(amount_text) <= 64),
    currency_code           text        NOT NULL CHECK (length(currency_code) = 3),
    statement_payload       jsonb       NOT NULL,
    supersedes_settlement_id uuid       NULL,
    audit_record_id         uuid        NOT NULL,
    recorded_at_unix_ms     bigint      NOT NULL CHECK (recorded_at_unix_ms >= 0),
    recorded_by             text        NOT NULL CHECK (length(btrim(recorded_by)) > 0),
    created_at              timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_settlements_pk PRIMARY KEY (settlement_id, tenant_id),
    CONSTRAINT market_settlements_program_fk
        FOREIGN KEY (program_id, tenant_id)
        REFERENCES market_programs (program_id, tenant_id),
    -- A corrected statement is a NEW row superseding the old one; the
    -- superseded row is retained for audit. Never self-referential.
    CONSTRAINT market_settlements_supersedes_fk
        FOREIGN KEY (supersedes_settlement_id, tenant_id)
        REFERENCES market_settlements (settlement_id, tenant_id),
    CONSTRAINT market_settlements_no_self_supersede CHECK (
        supersedes_settlement_id IS NULL
        OR supersedes_settlement_id <> settlement_id),
    CONSTRAINT market_settlements_period CHECK (period_end_millis > period_start_millis)
);

CREATE INDEX market_settlements_program_idx
    ON market_settlements (tenant_id, program_id);

-- The retention configuration rows (P0.16 §4): per tenant, per record
-- class, every deployment-undecided class UNSET. This is the only table
-- in this migration whose rows are mutable bookkeeping (UPDATE granted);
-- it carries configuration, not evidence — the evidence tables above are
-- INSERT + SELECT only.
CREATE TABLE market_record_retention_config (
    tenant_id       uuid        NOT NULL REFERENCES tenants (tenant_id),
    record_class    text        NOT NULL CHECK (
                        record_class = ANY (ARRAY[
                            'PROGRAM'::text,
                            'CONTRACT'::text,
                            'ENROLLMENT'::text,
                            'MARKET_EVENT'::text,
                            'DELIVERY'::text,
                            'SETTLEMENT'::text,
                            'MV_REPORT'::text,
                            'REGISTRATION'::text])),
    retention_state text        NOT NULL CHECK (
                        retention_state = ANY (ARRAY[
                            'UNSET'::text,
                            'CONFIGURED'::text])),
    period_days     integer     NULL CHECK (period_days IS NULL OR period_days > 0),
    basis           text        NULL CHECK (basis IS NULL OR length(btrim(basis)) > 0),
    set_by          text        NOT NULL CHECK (length(btrim(set_by)) > 0),
    set_at_unix_ms  bigint      NOT NULL CHECK (set_at_unix_ms >= 0),
    created_at      timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT market_record_retention_config_pk PRIMARY KEY (tenant_id, record_class),
    -- UNSET carries NO period and NO basis (a missing value can never
    -- silently become a built-in period, the P1.10 RetentionPolicy rule
    -- mirrored at rest); CONFIGURED carries BOTH — a period without a
    -- recorded basis is not a configurable retention row.
    CONSTRAINT market_record_retention_unset_shape CHECK (
        (retention_state = 'UNSET'::text) = (period_days IS NULL AND basis IS NULL))
);

ALTER TABLE market_programs OWNER TO flexcore_migrator;
ALTER TABLE market_contracts OWNER TO flexcore_migrator;
ALTER TABLE market_enrollments OWNER TO flexcore_migrator;
ALTER TABLE market_events OWNER TO flexcore_migrator;
ALTER TABLE market_deliveries OWNER TO flexcore_migrator;
ALTER TABLE market_settlements OWNER TO flexcore_migrator;
ALTER TABLE market_record_retention_config OWNER TO flexcore_migrator;

REVOKE ALL ON market_programs FROM PUBLIC;
REVOKE ALL ON market_contracts FROM PUBLIC;
REVOKE ALL ON market_enrollments FROM PUBLIC;
REVOKE ALL ON market_events FROM PUBLIC;
REVOKE ALL ON market_deliveries FROM PUBLIC;
REVOKE ALL ON market_settlements FROM PUBLIC;
REVOKE ALL ON market_record_retention_config FROM PUBLIC;

-- Financial and delivery records: INSERT + SELECT only — append-only
-- records whose supersede model is a NEW row, so a silent overwrite is
-- unrepresentable at rest (the 0016 content-immutability posture,
-- simplified: these records have NO state-transition column to grant).
GRANT SELECT, INSERT ON market_programs TO flexcore_app;
GRANT SELECT, INSERT ON market_contracts TO flexcore_app;
GRANT SELECT, INSERT ON market_enrollments TO flexcore_app;
GRANT SELECT, INSERT ON market_events TO flexcore_app;
GRANT SELECT, INSERT ON market_deliveries TO flexcore_app;
GRANT SELECT, INSERT ON market_settlements TO flexcore_app;
-- The retention configuration is mutable bookkeeping, not evidence:
-- SELECT + INSERT + UPDATE on THIS table only.
GRANT SELECT, INSERT, UPDATE ON market_record_retention_config TO flexcore_app;

-- Tenant scoping: the exact P1.12 §5.1 conventions (ENABLE + FORCE +
-- the shared tenant_isolation policy shape, the 0016 pattern).
ALTER TABLE market_programs ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_programs FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_programs
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_contracts ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_contracts FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_contracts
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_enrollments ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_enrollments FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_enrollments
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_events FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_events
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_deliveries FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_deliveries
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_settlements ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_settlements FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_settlements
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);

ALTER TABLE market_record_retention_config ENABLE ROW LEVEL SECURITY;
ALTER TABLE market_record_retention_config FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON market_record_retention_config
    USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
    WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
