-- P13.09 -- Migration 0020 down: drop the webhook delivery bookkeeping
--          tables in reverse order of the up migration (dead letters,
--          attempts, deliveries, endpoints; policies with each table).

DROP POLICY IF EXISTS tenant_isolation ON webhook_dead_letters;
DROP INDEX IF EXISTS webhook_dead_letters_open_idx;
DROP TABLE IF EXISTS webhook_dead_letters;

DROP POLICY IF EXISTS tenant_isolation ON webhook_delivery_attempts;
DROP INDEX IF EXISTS webhook_delivery_attempts_delivery_idx;
DROP TABLE IF EXISTS webhook_delivery_attempts;

DROP POLICY IF EXISTS tenant_isolation ON webhook_deliveries;
DROP INDEX IF EXISTS webhook_deliveries_due_idx;
DROP TABLE IF EXISTS webhook_deliveries;

DROP POLICY IF EXISTS tenant_isolation ON webhook_endpoints;
DROP INDEX IF EXISTS webhook_endpoints_tenant_idx;
DROP TABLE IF EXISTS webhook_endpoints;
