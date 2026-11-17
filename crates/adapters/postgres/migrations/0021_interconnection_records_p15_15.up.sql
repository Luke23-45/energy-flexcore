-- P15.15 -- Migration 0021 up: interconnection document records (UL 1741 SB
--           COCs, IEEE 1547.1-2020 test reports, certified function
--           categories, settings-profile references) per DEVICE MODEL.
--
-- Decision record:
-- docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
-- 44_p15_15_interconnection_record_ingestion.md
--
-- Sources: [S-21] (CEC: the UL 1741 SB addition to the Solar Equipment
-- Lists — certified equipment is LISTED by the certification bodies),
-- [S-169] (the CHINT IEEE 1547-2018 / UL 1741 SB bulletin), [S-170]
-- (UL 1741 SB vs IEEE 1547.1-2020 test procedures). The platform LISTS
-- certified equipment; it NEVER claims certification for itself — this
-- table stores REFERENCES to documents issued by certification bodies /
-- laboratories, never certification claims by FlexCore.
--
-- What this migration adds (and what it deliberately does NOT):
--
-- - `interconnection_records`: one row per (device_model, document_ref)
--   identity — an immutable, platform-GLOBAL equipment catalog. The
--   TENANCY DECISION (recorded in the storage trait header and the
--   decision record): certification is a property of the equipment
--   MODEL, not of any tenant's instance of it, so there is NO tenant_id
--   column and NO tenant RLS on this table (the same posture as the
--   P14.13 jurisdiction document store). Writes are operator /
--   composition-root ingest actions; reads are registry-linkage lookups
--   on the EXACT device_model string (no fuzzy match, per the domain
--   statement).
-- - Records are IMMUTABLE: a corrected, renewed, or withdrawn document
--   is a NEW row referencing the new document version; the runtime
--   grants are INSERT + SELECT only (the P2.06 append-only grant
--   discipline). The (device_model, document_ref) UNIQUE constraint
--   makes a silent overwrite unrepresentable at rest (the 0015 pattern).
-- - `device_model` is the registry's vendor_model string (P0.10 §1 row
--   4), stored as declared text — EXACT string equality is the linkage
--   contract, so no case/whitespace normalization and no FK to a device
--   model table exists (the registry carries vendor_model on assets, not
--   a device-model catalog; a catalog FK would invent one).
-- - `certified_functions` and `ride_through_categories` are jsonb arrays
--   of the P14.13 catalog names (canonical snake_case spellings, e.g.
--   "volt_var", "category_ii") — validated by the adapter on insert
--   against the closed catalog vocabularies (flexcore-domain
--   ieee1547_settings); the database check enforces array-ness only.
-- - `document_version` is the ISSUER's version label (recorded data,
--   never invented); `ingested_at_unix_ms` is the INGESTION time (the
--   P0.09 time-separation discipline), `ingestion_source` records who
--   supplied the document copy.
--
-- What this migration does NOT contain:
-- - NO tenant_id, NO tenant RLS (the recorded tenancy decision above).
-- - NO retention value, NO TTL, NO scheduled delete (P0.16 §4: retention
--   is the deployment's configuration, never universal).
-- - NO certification status OF THE PLATFORM and no claim column beyond
--   the document's own recorded data — the platform never claims
--   certification for itself [S-21].
-- - NO destructive change to any existing table: this is an expand-only
--   CREATE TABLE (the P11.18 migration policy §1/§2 — always safe).

CREATE TABLE interconnection_records (
    record_id            uuid        PRIMARY KEY,
    device_model         text        NOT NULL CHECK (length(btrim(device_model)) > 0 AND length(device_model) <= 256),
    document_kind        text        NOT NULL CHECK (document_kind IN ('ul_1741_sb_coc', 'ieee_1547_test_report')),
    document_ref         text        NOT NULL CHECK (length(btrim(document_ref)) > 0 AND length(document_ref) <= 256),
    document_version     text        NOT NULL CHECK (length(btrim(document_version)) > 0),
    issuer               text        NOT NULL CHECK (length(btrim(issuer)) > 0),
    category             text        NOT NULL CHECK (category IN ('category_a', 'category_b', 'not_classified')),
    certified_functions  jsonb       NOT NULL
        CHECK (jsonb_typeof(certified_functions) = 'array' AND jsonb_array_length(certified_functions) >= 1),
    ride_through_categories jsonb    NOT NULL
        CHECK (jsonb_typeof(ride_through_categories) = 'array'),
    settings_profile_ref text        NOT NULL CHECK (length(btrim(settings_profile_ref)) > 0),
    ingested_at_unix_ms  bigint      NOT NULL CHECK (ingested_at_unix_ms >= 0),
    ingestion_source     text        NOT NULL CHECK (length(btrim(ingestion_source)) > 0),
    created_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT interconnection_records_identity_unique
        UNIQUE (device_model, document_ref)
);

CREATE INDEX interconnection_records_device_model_idx
    ON interconnection_records (device_model);

ALTER TABLE interconnection_records OWNER TO flexcore_migrator;

REVOKE ALL ON interconnection_records FROM PUBLIC;
-- Immutable records: INSERT + SELECT only. NO tenant RLS (the recorded
-- tenancy decision: a platform-global equipment catalog).
GRANT SELECT, INSERT ON interconnection_records TO flexcore_app;
