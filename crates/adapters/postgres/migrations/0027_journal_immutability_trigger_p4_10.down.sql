-- P4.10 -- Migration 0027 down: remove the journal immutability
--          trigger whole. EXPAND-ONLY reversal (the P11.18 policy):
--          the up migration only CREATED one trigger + function, so the
--          down migration only DROPS them — no column, table,
--          constraint, or grant belonging to any earlier migration is
--          touched. Dropping the trigger restores the pre-0027 behaviour
--          (the journal is then protected only by the flexcore_app
--          SELECT/INSERT grant from migration 0016).

DROP TRIGGER IF EXISTS dispatch_command_transitions_immutability
    ON dispatch_command_transitions;
DROP FUNCTION IF EXISTS dispatch_command_transitions_guard_immutable();
