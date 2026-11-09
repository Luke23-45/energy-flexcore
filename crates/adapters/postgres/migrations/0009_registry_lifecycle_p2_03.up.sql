-- P2.06 -- Migration 0009 up: additive registry extension for the P2.03
--          domain additions (asset lifecycle status + terminal deactivation).
--
-- The Phase 1 registry tables (tenants/portfolios/sites/assets, P1.12/P1.13)
-- are the base and are EXTENDED here, never recreated (P2.06 step 1; ledger
-- acceptance criterion "registry tables are extended, not recreated"). The
-- P2.03 domain additions (ledger 02_registry_domain_model.md §2.1/§5) are:
--
--   - `lifecycle_status`: two-state registry lifecycle, 'active' ->
--     'deactivated', terminal in Phase 2. Existing rows are 'active'
--     (NOT NULL DEFAULT 'active': a NOT NULL add with a constant default
--     backfills in place without a table rewrite of values).
--   - `deactivation_reason`: mandatory exactly when deactivated.
--   - `deactivated_at`: when the deactivation was recorded (UTC, P0.09),
--     present exactly when deactivated.
--
-- No new column carries a market/jurisdiction decision (D1 stays open) and
-- no existing column is altered, renamed, or dropped. The asset-type CHECK
-- vocabulary (seven first-class types, P0.10 §2) is unchanged — the P2.03
-- domain enum already maps onto it 1:1 (src/enum_map.rs); no migration is
-- needed for it. Capability profile and PCC/grid-model reference already
-- exist on `sites`/`assets` from Phase 1 as references (P2.02 §1/§4:
-- reference-only in Phase 2), so no columns are added for them either.

ALTER TABLE assets
    ADD COLUMN lifecycle_status text NOT NULL DEFAULT 'active',
    ADD COLUMN deactivation_reason text NULL,
    ADD COLUMN deactivated_at timestamptz NULL;

-- Consistency of the terminal state (P2.03: reason mandatory at
-- deactivation): a deactivated asset row must carry both the reason and
-- the timestamp. Named constraint; the down migration drops it explicitly.
ALTER TABLE assets
    ADD CONSTRAINT assets_lifecycle_consistency
    CHECK (lifecycle_status = 'active'
           OR (lifecycle_status = 'deactivated'
               AND deactivation_reason IS NOT NULL
               AND deactivated_at IS NOT NULL));

-- Runtime grants are unchanged: flexcore_app already holds
-- SELECT/INSERT/UPDATE on assets and the new columns ride along (P1.13
-- grant shape). RLS is unchanged (the P1.12 §5.1 policy already covers the
-- whole row).
