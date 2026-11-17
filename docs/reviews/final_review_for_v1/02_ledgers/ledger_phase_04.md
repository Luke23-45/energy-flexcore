# Final Review Ledger — Phase 04

**Ledger status:** **FULLY FIXED (written) — 8 of 8 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-04-*` for phases
00–10, `docs/phases-11-20/phase-04-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P04.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P04.01 | `FRV1-R1-002` | P1 | CommandExpiryService::run_sweep stamps every swept command's events with ONE caller-supplied EventScope | Derive per-command EventScope inside run_sweep | FIXED (written, awaiting human build/test) — fix wave 2 (per-command scope travels on the ExpiryCandidate; caller scope is the named fallback only) |
| FRV1-P04.02 | `FRV1-R1-004` | P1 | The P4.11-P4.18 command core is constructed by no composition root — test-only wiring | Construct the command core at a production composition root (coordinate with FRV1-R2-001/002 composition-root work) | FIXED (recorded status/plan, awaiting human build/test) — fix wave 2: the core cannot honestly be constructed in the control-plane root (borrowing constructors, six unimplemented ports, read-only dispatch surface); the composition plan and future seam are recorded at the composition root citing the finding |
| FRV1-P04.03 | `FRV1-R5-002` | P1 | Two implemented store families (market records, interconnection records) have no composition-root consumer | Compose MarketRecordsStore + InterconnectionRecordStore (migrations 0021/0022) at a consumer, or record the interim design-only status at the seam | FIXED (recorded seam status, awaiting human build/test) — fix wave 4 (no natural consumer exists; design-level-only status recorded at the composition root citing the finding, per the FRV1-R1-004 precedent) |
| FRV1-P04.04 | `FRV1-R1-007` | P2 | CommandGateStoreError documented as 1:1 of CommandStoreError, but the mapping cannot be 1:1 | Fix doc/impl mismatch or the mapping | FIXED (written, awaiting human build/test) — fix wave 4 (per-variant mapping table replaces the false 1:1 claim; fail-closed Backend mappings recorded) |
| FRV1-P04.05 | `FRV1-R1-008` | P2 | CommandRecordFields::validate_common: always-empty if block in the supersedes group | Repair or remove the dead if block | FIXED (written, awaiting human build/test) — fix wave 4 (dead if removed; domain already enforces the supersede rule via InvalidSupersession, intent not recoverable in the flat record — recorded) |
| FRV1-P04.06 | `FRV1-R5-005` | P2 | dispatch_commands has no index for its documented tenant-listing query pattern | Add the tenant-listing index to a new expand-only migration | FIXED (written, awaiting human build/test) — fix wave 4 (additive migration 0025: dispatch_commands_tenant_idx matching the documented listing pattern; compat suite pins 25) |
| FRV1-P04.07 | `FRV1-R5-008` | P2 | Migration 0020 defaults PKs to gen_random_uuid(), off the recorded D-S1 uuidv7() convention | Reconcile with the D-S1 convention; record the decision (expand-only rules apply) | FIXED (recorded reconciliation, awaiting human build/test) — fix wave 4 (0020 untouched per expand-only; the P13.09 record gains the reconciliation addendum: DB defaults are fallback-only, adapter binds D-S1 uuidv7 ids) |
| FRV1-P04.08 | `FRV1-R5-009` | P2 | MarketRecordsStore read methods are unbounded | Page-bound the market-record listing reads | FIXED (written, awaiting human build/test) — fix wave 4 (mandatory limit on all seven market-record reads, trait + adapter, LIMIT $n bound) |
