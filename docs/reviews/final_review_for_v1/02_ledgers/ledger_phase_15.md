# Final Review Ledger — Phase 15

**Ledger status:** **FULLY FIXED (written) — 6 of 6 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-15-*` for phases
00–10, `docs/phases-11-20/phase-15-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P15.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P15.01 | `FRV1-R4-001` | P0 | forecast/gnn/marl __main__ blocks never call main() — documented transport contract cannot run (phase-08 workers cross-noted) | Make the three __main__ blocks call main() | FIXED (written, awaiting human build/test) — fix wave 1 |
| FRV1-P15.02 | `FRV1-R4-002` | P0 | The "approved" key is not in FORBIDDEN_RESULT_KEYS; the human gate fails on first execution (phase-08 workers cross-noted) | Add "approved" to the three FORBIDDEN_RESULT_KEYS sets and keep the refusal tests | FIXED (written, awaiting human build/test) — fix wave 1 |
| FRV1-P15.03 | `FRV1-R1-003` | P1 | WeatherDataProvider/PriceSignalSource ports unwired; TelemetryIngestStore/TelemetryReader have no real adapter | Implement the ports/adapter or add named Unwired* gates + recorded deferrals (phase-02 telemetry part cross-noted) | FIXED (named Unwired defaults + recorded adapter status, awaiting human build/test) — fix wave 4 (UnwiredWeatherDataProvider / UnwiredPriceSignalSource fail closed with recorded deferrals citing the open P15-E-1 gate; telemetry port pair carries the adapter-status seam note) |
| FRV1-P15.04 | `FRV1-R1-005` | P2 | flexcore-domain lib.rs: doc comment above pub mod bess_model describes the wrong module | Fix the doc comment | FIXED (written, awaiting human build/test) — fix wave 4 (bess_model doc comment corrected) |
| FRV1-P15.05 | `FRV1-R4-007` | P2 | forecast README claims numpy/pandas guarded imports — pandas does not exist | Fix the README claim | FIXED (written, awaiting human build/test) — fix wave 3 (README states numpy is the only guarded import; pandas exists nowhere) |
| FRV1-P15.06 | `FRV1-R7-010` | P2 | Guarded expect()s in non-test code where crate style is a named refusal (15/16/17 sites) | Convert to named refusals or invariant notes per crate style | FIXED (written, awaiting human build/test) — fix waves 4 + orchestrator (payment_lifecycle/forecast_integration/revenue_share_billing converted to named refusals or hoisted documented-constant helpers; the ev_charging_composer peek site carries the invariant note per the crate's guarded-not-hidden distinction) |
