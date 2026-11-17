# Final Review Ledger — Phase 06

**Ledger status:** **FULLY FIXED (written) — 2 of 2 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-06-*` for phases
00–10, `docs/phases-11-20/phase-06-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P06.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P06.01 | `FRV1-R3-004` | P1 | P6.25 edge-status panel unreachable: no route renders SitesEdgeStatusSection | Compose the panel into the Sites workspace route (server-side counterpart is FRV1-R2-002) | FIXED (written, awaiting human build/test) — fix wave 2 (SitesWithEdgeStatus composed on the sites route; loader dedupes shared edge identities) |
| FRV1-P06.02 | `FRV1-R1-006` | P2 | flexcore-storage lib.rs module table missing three modules (P6.06/P15.15/P16.01) | Complete the module table | FIXED (written, awaiting human build/test) — fix wave 4 (flexcore-storage lib.rs module table completed) |
