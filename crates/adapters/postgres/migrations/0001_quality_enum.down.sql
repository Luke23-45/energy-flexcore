-- P1.13 -- Migration 0001 down: drop the quality enum.
--
-- The service roles are cluster-level objects and are deliberately NOT
-- dropped here: role lifecycle belongs to the deployment bootstrap, and a
-- schema rollback must never remove security principals that other
-- databases in the cluster may reference. Recorded as the irreversible part
-- of this migration (roles only; the enum itself is fully reversible).

DROP TYPE IF EXISTS telemetry_quality;
