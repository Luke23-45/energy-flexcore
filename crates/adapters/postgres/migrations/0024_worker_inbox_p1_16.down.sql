-- P1.16/P1.19 -- Migration 0024 down: remove the worker consumer inbox
--          whole. EXPAND-ONLY reversal (the P11.18 policy): the up
--          migration only CREATED objects, so the down migration only
--          DROPS them — no column, table, constraint, or grant belonging
--          to any earlier migration is touched. Dropping the inbox
--          discloses consumed-at bookkeeping only (which consumer
--          processed which idempotency key when); no domain evidence and
--          no tenant data lives here. (The inbox has never held a row in
--          this program — WRITTEN-UNRUN; the drop is recorded, never
--          exercised.)

DROP TABLE flexcore_worker_inbox;
