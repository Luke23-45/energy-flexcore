# Final Review Ledger — Phase 18

**Ledger status:** **FULLY FIXED (written) — 3 of 3 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-18-*` for phases
00–10, `docs/phases-11-20/phase-18-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P18.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P18.01 | `FRV1-R6-005` | P2 | Entire OTA/update audit surface classified EdgeAuditKind::Startup | Add an update variant and reclassify the stream | FIXED (written, awaiting human build/test) — fix wave 3 (additive UpdateLifecycle variant in ports + audit_buffer with positional code 18; four emission sites reclassified) |
| FRV1-P18.02 | `FRV1-R6-007` | P2 | AntiRollbackCounter::accept durably appends before the strictly-ahead re-check | Reorder so the durable append follows the re-check | FIXED (written, awaiting human build/test) — fix wave 3 (reordered: strictly-ahead check first, durable append only after; refused sequences never reach the register; pinned test extended) |
| FRV1-P18.03 | `FRV1-R8-007` | P2 | Esime naming drift in the public eSIM vocabulary | Fix the names | FIXED (written, awaiting human build/test) — fix wave 3 (Esime renamed to Esim across 41 occurrences; zero residual) |
