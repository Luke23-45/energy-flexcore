-- P11.15 -- Migration 0019 down: remove the keyed audit-chain storage.
--
-- Contract-migration discipline (P11.18 policy): the down path is a
-- dev/CI rollback concern (P1.12 §7) — it drops the chain columns and the
-- head-state table. On a database where chain rows exist, dropping the
-- columns DISCARDS the stored chain evidence (the exported anchors remain
-- the retained evidence — the P11.15 anchor-export design exists precisely
-- so the retained anchors outlive any local storage state).

DROP TABLE IF EXISTS audit_chain_state;

ALTER TABLE audit_records
    DROP COLUMN IF EXISTS chain_sequence,
    DROP COLUMN IF EXISTS chain_prev_head,
    DROP COLUMN IF EXISTS chain_digest,
    DROP COLUMN IF EXISTS chain_algorithm;
