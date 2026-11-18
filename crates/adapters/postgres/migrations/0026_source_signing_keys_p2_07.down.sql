-- Patch B (P2.07) -- Migration 0026 down: drop the sequence-watermark
--          events and the source-signing-key registry (reverse order of
--          the up migration).

DROP POLICY IF EXISTS tenant_isolation ON source_sequence_events;
DROP TRIGGER IF EXISTS source_sequence_events_append_only ON source_sequence_events;
DROP FUNCTION IF EXISTS source_sequence_events_append_only_guard();
DROP TABLE IF EXISTS source_sequence_events;

DROP POLICY IF EXISTS tenant_isolation ON source_signing_keys;
DROP TRIGGER IF EXISTS source_signing_keys_append_only ON source_signing_keys;
DROP FUNCTION IF EXISTS source_signing_keys_append_only_guard();
DROP INDEX IF EXISTS source_signing_keys_one_active_term;
DROP INDEX IF EXISTS source_signing_keys_current_idx;
DROP TABLE IF EXISTS source_signing_keys;
