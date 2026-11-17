# Final Review Ledger — Phase 02

**Ledger status:** **FIX WAVE 1 APPLIED — 1 of 1 subtasks fixed (written, awaiting human build/test).**

Phase scope: see the implementation ledgers (`ledger/phase-02-*` for phases
00–10, `docs/phases-11-20/phase-02-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P02.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P02.01 | `FRV1-R5-001` | P0 | Asset deactivation lifecycle never reaches the database — the terminal state silently reverts on read-back | Bind + read lifecycle_status/deactivation_reason/deactivated_at in postgres AssetRepository (unit_of_work.rs ASSET_SELECT/decode_asset_with); see also cross-tag from FRV1-R1-003 | FIXED (written, awaiting human build/test) — fix wave 1 |
