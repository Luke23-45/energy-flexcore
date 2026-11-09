-- P2.31 -- Migration 0012 up: versioned telemetry corrections and the
--          as-of (point-in-time) visibility columns (foundations §6.4;
--          ledger P2.31 steps 1 and 4).
--
-- ADDITIVE, in the P2.06 spirit: the telemetry table is EXTENDED, never
-- recreated. Everything already stored keeps its meaning; the original row
-- of any corrected record remains queryable unchanged (the ledger's
-- "a correction never mutates the original row" acceptance).
--
-- Design (recorded in
-- ledger/phase-02-asset-telemetry/25_31_exit_portfolio_corrections.md,
-- D-C1..D-C4):
--
-- - A correction is a NEW ROW, never an in-place update: it carries the
--   same storage identity legs (asset_id, event_time, source_sequence,
--   quantity) as the record it supersedes, plus the corrected value and
--   the correction metadata (reason, actor, correction timestamp). The
--   append-only property holds at the storage level: no UPDATE or DELETE
--   path exists for telemetry rows in any runtime role (D-P6 grants are
--   INSERT/SELECT only and stay unchanged).
-- - telemetry_version distinguishes the versions of one storage identity:
--   1 = the original record (DEFAULT 1, so existing and pipeline-written
--   rows are originals without any pipeline change), >= 2 = corrections.
--   The PRIMARY KEY is EXTENDED to include telemetry_version — the P2.05
--   D-P3 semantics are kept intact (the partition key event_time remains
--   part of the PK; PostgreSQL requires exactly that) while the unique
--   constraint that "permits multiple versions of the same (asset,
--   event_time, quantity) distinguished by version" is the extended PK
--   itself. The constraint NAME telemetry_pkey is retained on purpose:
--   the P2.09 duplicate classification matches it by name, never by text.
-- - correction_of references the superseded row's telemetry_id. There is
--   deliberately NO foreign key: telemetry_id alone is not unique (the PK
--   is composite) and PostgreSQL cannot enforce an FK that omits PK legs;
--   the linkage is enforced by the correction procedure, which reads the
--   original row inside the SAME transaction before inserting the
--   correction (a stronger guarantee than an FK for this path).
-- - corrected_at is the correction's own timestamp (foundations §6.4:
--   "correction timestamp — its own ingest/event time"): the moment the
--   correction became known. It is the as-of visibility key for versions
--   >= 2. Version 1 (the original) is always visible — an as-of query
--   never un-sees the fact that SOMETHING was stored; it only decides
--   WHICH version was current.
--
-- No retention value, no TTL, no scheduled delete appears here (D-P5
-- unchanged). No new grants: corrections are written by the same
-- flexcore_ingest INSERT grant the pipeline holds, and read through the
-- existing SELECT grants (flexcore_query, flexcore_app from 0011).

ALTER TABLE telemetry
    ADD COLUMN telemetry_version integer NOT NULL DEFAULT 1,
    ADD COLUMN correction_of uuid NULL,
    ADD COLUMN correction_reason text NULL,
    ADD COLUMN corrected_by text NULL,
    ADD COLUMN corrected_at timestamptz NULL;

-- Version and correction-metadata shape: version 1 rows carry NO
-- correction metadata (they are originals); versions >= 2 carry ALL of it,
-- with non-blank reason and actor (the ledger: a correction carries the
-- reason and the actor/source — mandatory, not optional).
ALTER TABLE telemetry
    ADD CONSTRAINT telemetry_version_positive
        CHECK (telemetry_version >= 1),
    ADD CONSTRAINT telemetry_correction_metadata
        CHECK (
            (telemetry_version = 1
             AND correction_of IS NULL
             AND correction_reason IS NULL
             AND corrected_by IS NULL
             AND corrected_at IS NULL)
            OR
            (telemetry_version >= 2
             AND correction_of IS NOT NULL
             AND correction_reason IS NOT NULL
                 AND length(btrim(correction_reason)) > 0
             AND corrected_by IS NOT NULL
                 AND length(btrim(corrected_by)) > 0
             AND corrected_at IS NOT NULL)
        );

-- Extend the PK with the version leg (same name, D-P3 partition key kept).
-- This rebuilds the per-partition unique indexes; on the Phase 2 synthetic
-- volume that is an acceptable one-time cost. VERIFY-BUILD: the drop/re-add
-- of a partitioned-table PK is exercised by the migration up/down replay in
-- the adapter integration suite on PostgreSQL 18.
ALTER TABLE telemetry DROP CONSTRAINT telemetry_pkey;
ALTER TABLE telemetry
    ADD CONSTRAINT telemetry_pkey
    PRIMARY KEY (asset_id, event_time, source_sequence, quantity, telemetry_version);
