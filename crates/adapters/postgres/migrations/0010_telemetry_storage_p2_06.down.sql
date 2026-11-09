-- P2.06 -- Migration 0010 down: drop the telemetry storage surface.
-- Multiple statements, ';' separated (the integration-test down replay
-- executes each fragment individually).

DROP TABLE IF EXISTS telemetry_quarantine;
DROP TABLE IF EXISTS telemetry;
DROP TYPE IF EXISTS clock_quality;
DROP TYPE IF EXISTS reactive_sign_convention;
-- The flexcore_ingest / flexcore_query roles are cluster-level objects and
-- are deliberately NOT dropped here — the recorded 0001 down rationale
-- applies verbatim: role lifecycle belongs to the deployment bootstrap, and
-- a schema rollback must never remove security principals other databases
-- in the cluster may reference. This is the irreversible part of this
-- migration (the tables, indexes, policies, and enums are fully reversible).
