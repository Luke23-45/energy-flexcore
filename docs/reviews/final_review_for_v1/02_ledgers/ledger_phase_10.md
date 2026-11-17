# Final Review Ledger — Phase 10

**Ledger status:** **FIX WAVES 1-4 APPLIED — 4 of 5 subtasks fixed (written, awaiting human build/test); FRV1-R8-003 (LICENSE/D4) is a recorded HUMAN GATE and stays OPEN.**

Phase scope: see the implementation ledgers (`ledger/phase-10-*` for phases
00–10, `docs/phases-11-20/phase-10-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P10.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P10.01 | `FRV1-R2-001` | P1 | /api/v1/alarms is dead in the production composition root: composed policy set predates the alarm.read grant | Compose phase5_default_policies (or add the grant) in apps/control-plane/src/main.rs | FIXED (written, awaiting human build/test) — fix wave 1 |
| FRV1-P10.02 | `FRV1-R2-002` | P1 | Edge enrollment seams never composed: /api/v1/edge/status and /decommission are permanent infrastructure errors | Register the Postgres EdgeEnrollmentStore via with_edge_enrollment in main.rs | FIXED (written, awaiting human build/test) — fix wave 1 |
| FRV1-P10.03 | `FRV1-R8-002` | P1 | Root README contradicts the repository it fronts | Rewrite README.md to describe the implemented repository (cross-tag phase-20) | FIXED (written, awaiting human build/test) — fix wave 4 (root README rewritten to the actual repository: 29 members, WRITTEN-UNRUN honesty state, real doc pointers, D4 honestly open) |
| FRV1-P10.04 | `FRV1-R8-003` | P1 | No LICENSE file anywhere; the workspace omits its own license (D4 open) | Prepare the license per the recorded D4 decision — HUMAN GATE: the D4 closer selects the license; agent writes the file only after the decision is recorded | OPEN |
| FRV1-P10.05 | `FRV1-R2-010` | P2 | Stale doc-comment fragment on map_stored_record | Fix the copy-paste drift | FIXED (written, awaiting human build/test) — fix wave 4 (stale first sentence removed from the map_stored_record doc) |
