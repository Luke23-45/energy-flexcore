# Ledger — V-8/V-9/V-10 Pre-publication items V-8/V-9/V-10

**Owning code:** `crates/adapters/postgres + crates/flexcore-ports`
**Phase record:** §3 items
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature V-8/V-9/V-10); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-V08.01 | Same-sequence race regression test (gated): two concurrent submissions of the same (source_id, sequence) produce exactly one Advanced, the rest Stale (source_keys.rs advance_source_sequence) fixed (written, awaiting human build/test) — wave 8 |
| FIM-V09.01 | Postgres audit_records chain upgraded from test-grade FNV-1a to the audit-crate HMAC-SHA-256 construction, with a migration note for pre-upgrade chain verification (old chains keep their recorded digest algorithm - no silent re-hash) fixed (written, awaiting human build/test) — wave 8 |
| FIM-V10.01 | Remove the .expect() fail-fast at crates/flexcore-ports/src/source_auth.rs push_length_prefixed; typed error already returned - ensure callers refuse fixed (written, awaiting human build/test) — wave 8 |
| FIM-VXX.02 | Honesty header: gated tests run only in the human V-1/V-2 re-run fixed (written, awaiting human build/test) — wave 8 |

---

**Status:** 4/4 fixed (written, awaiting human build/test) — wave 8. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.