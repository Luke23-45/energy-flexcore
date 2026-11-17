-- P16.01 -- Migration 0022 down: drop the market-records tables in
--           reverse dependency order. A down migration on evidence-class
--           tables is a DESTRUCTIVE operation: in any real deployment it
--           requires operator sign-off (MIGRATION_POLICY §3 — evidence
--           removal is never a code-only decision). It exists here only
--           to keep the expand/contract pair complete.

DROP POLICY IF EXISTS tenant_isolation ON market_record_retention_config;
DROP POLICY IF EXISTS tenant_isolation ON market_settlements;
DROP POLICY IF EXISTS tenant_isolation ON market_deliveries;
DROP POLICY IF EXISTS tenant_isolation ON market_events;
DROP POLICY IF EXISTS tenant_isolation ON market_enrollments;
DROP POLICY IF EXISTS tenant_isolation ON market_contracts;
DROP POLICY IF EXISTS tenant_isolation ON market_programs;

DROP TABLE market_record_retention_config;
DROP TABLE market_settlements;
DROP TABLE market_deliveries;
DROP TABLE market_events;
DROP TABLE market_enrollments;
DROP TABLE market_contracts;
DROP TABLE market_programs;
