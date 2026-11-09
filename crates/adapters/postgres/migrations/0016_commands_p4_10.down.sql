-- P4.10 -- Migration 0016 down: remove the dispatch command records and the
--          state-change journal.

DROP TABLE IF EXISTS dispatch_command_transitions;
DROP TABLE IF EXISTS dispatch_commands;
DROP FUNCTION IF EXISTS dispatch_commands_guard_insert();
DROP FUNCTION IF EXISTS dispatch_commands_guard_update();
DROP FUNCTION IF EXISTS dispatch_commands_legal_edge(text, text);
