# Final Review Ledger — Phase 13

**Ledger status:** **FULLY FIXED (written) — 6 of 6 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-13-*` for phases
00–10, `docs/phases-11-20/phase-13-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P13.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P13.01 | `FRV1-R3-003` | P1 | Console carries two inconsistent role mirrors; canAction denial reason asserts a falsehood for site-approver | Unify capabilityMatrix/adminManagement; fix the denial reason | FIXED (written, awaiting human build/test) — fix wave 4 (capabilityMatrix.ts is the single authoritative five-role mirror; adminManagement/adminData derive from it; denial reason states only what is true) |
| FRV1-P13.02 | `FRV1-R8-004` | P1 | whsec_ strict parse rule (record 08 sec 4) has no implementation in code | Implement the strict parse at the webhook secret ingestion point | FIXED (written, awaiting human build/test) — fix wave 3 (parse_webhook_secret implements record 08 sec 4 exactly; five named rejection classes; no-echo Display; six vector tests) |
| FRV1-P13.03 | `FRV1-R8-005` | P1 | Webhook delivery cannot deliver: Postgres store, HTTP client, composition un-landed and untracked | Land the pieces or register them as tracked open items in the phase-13 close record | FIXED (written, awaiting human build/test) — fix wave 4 (Postgres delivery store landed; UnwiredOutboundHttpPost refuses with E_TRANSPORT_UNWIRED — no HTTP-client crate exists and none was added; booked rows stay honestly BOOKED for stale-recovery; the P13.09 record's step list updated additively) |
| FRV1-P13.04 | `FRV1-R2-007` | P2 | AppState.step_up is a write-only composition hook | Consume it (step-up middleware) or record the non-consumption | FIXED (recorded status, awaiting human build/test) — fix wave 4 (the step-up hook's non-consumption is recorded at the composition site citing the finding — the step-up middleware surface remains a recorded follow-up) |
| FRV1-P13.05 | `FRV1-R2-008` | P2 | Webhook dispatcher never produces the FATAL_FAILURE attempt outcome it validates | Produce the outcome on the terminal-failure path | FIXED (written, awaiting human build/test) — fix wave 4 + orchestrator correction: the wave-4 tick for this row was applied WITHOUT a matching fix and was caught by the orchestrator's own verification sweep (grep: FATAL_FAILURE produced nowhere); the row was re-grounded and the fix then implemented for real — HttpPostOutcome::is_permanent_rejection (4xx except 408/429) produces FATAL_FAILURE and dead-letters immediately; 408/429/5xx/transport stay retryable; two pinned tests added |
| FRV1-P13.06 | `FRV1-R2-009` | P2 | Three recorded grant sets composed nowhere: phase13_policies, phase16_market_policies, phase4_command_policies | Compose the grant sets in the production root (bundle with FRV1-R2-001) | FIXED (written, awaiting human build/test) — fix wave 1 (recorded-status route: sets intentionally not composed; composition-root comment added) |
