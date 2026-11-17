-- P11.15 -- Migration 0019 up: the keyed audit-chain columns and the
--          per-tenant chain head-state table (the P11-E-1 DEFAULT:
--          keyed HMAC-SHA-256 linear chain, per-tenant chain namespace,
--          periodic anchor-export design).
--
-- DEPLOYMENT REQUIREMENT (program conventions s6.6): the chain is keyed
-- with deployment-supplied key material — this schema carries the STORAGE
-- for chain state only; until an operator provisions reviewed keys and
-- wires them into the store (`FlexCoreStore::with_audit_chain_key`), the
-- chain columns stay NULL and rows are verifiable only through the
-- TEST-GRADE FNV-1a chain (LR-203), which is never a release signature.
--
-- EXPAND-ONLY (the P11.18 policy, `crates/adapters/postgres/
-- MIGRATION_POLICY.md`): every statement here is additive — nullable
-- columns and a new table. No ALTER touches an existing column, no
-- constraint scans a populated table, no index is built here (the
-- verification walk orders in SQL; a `(tenant_id, chain_sequence)` index
-- is the recorded CONCURRENTLY remediation, scheduled — see the P11.15
-- decision record). audit_records keeps its three append-only layers
-- unchanged (grants, trigger, port).
--
-- audit_records chain columns (all NULL-able: rows written before this
-- migration, and rows written while no key is configured, have no chain
-- position — the adapter and the verifier treat NULL as "unchained"):
-- - chain_sequence  bigint  — the per-tenant chain position (1-based,
--   assigned inside the append transaction under the head-state row lock).
-- - chain_prev_head text    — the previous head digest in the tenant's
--   chain (64 hex chars), so each row links backwards and the chain is
--   verifiable from the stored rows alone without trusting the mutable
--   head-state table.
-- - chain_digest    text    — HMAC-SHA-256 over (previous head || the
--   record's canonical encoding) under the TENANT-NAMESPACE chain key
--   (64 hex chars).
-- - chain_algorithm text    — the versioned algorithm label
--   (`hmac-sha256-chain-v1`); a construction change is a detectable
--   format change, never a silent one.
--
-- Table-level grants ride along automatically: migration 0008 granted
-- flexcore_app table-level INSERT, SELECT on audit_records, so the new
-- columns need no new grant (and no UPDATE grant appears — the row is
-- written once, at INSERT, by the same statement that appends the record).

ALTER TABLE audit_records
    ADD COLUMN chain_sequence bigint NULL,
    ADD COLUMN chain_prev_head text NULL,
    ADD COLUMN chain_digest text NULL,
    ADD COLUMN chain_algorithm text NULL;

-- The per-tenant chain head-state table: ONE row per chain namespace, the
-- mutable half of the design (the head pointer advances per append; the
-- evidence rows on audit_records are immutable). `tenant_key` is the
-- namespace key: the tenant's uuid, or the all-zero uuid for the
-- platform-level namespace (PLATFORM_CHAIN_TENANT_KEY in the adapter — a
-- uuid PRIMARY KEY cannot hold SQL NULL, so the platform namespace uses
-- the recorded sentinel).
--
-- The MUTABLE grant is deliberate and recorded: flexcore_app receives
-- SELECT, INSERT, UPDATE on THIS bookkeeping table only — audit_records
-- keeps its INSERT, SELECT-only grant set (0008 layer 1), and the
-- before-update/delete trigger (0008 layer 2) still refuses any audit-
-- records row mutation. The state table holds only the advancing head
-- pointer (sequence number + current digest); a tampered head state
-- cannot rewrite history — the stored rows' backwards links
-- (chain_prev_head) and the exported anchors carry the evidence.
CREATE TABLE audit_chain_state (
    tenant_key      uuid        PRIMARY KEY,
    chain_sequence  bigint      NOT NULL DEFAULT 0 CHECK (chain_sequence >= 0),
    chain_head      text        NULL CHECK (chain_head IS NULL OR length(btrim(chain_head)) = 64),
    chain_algorithm text        NOT NULL CHECK (length(btrim(chain_algorithm)) > 0),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE audit_chain_state OWNER TO flexcore_migrator;

REVOKE ALL ON audit_chain_state FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON audit_chain_state TO flexcore_app;
