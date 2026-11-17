# Final Review Ledger — Phase 16

**Ledger status:** **FULLY FIXED (written) — 7 of 7 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-16-*` for phases
00–10, `docs/phases-11-20/phase-16-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P16.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P16.01 | `FRV1-R7-001` | P0 | Unchecked i128 negation in reconciliation tolerance: panics in debug, silently accepts catastrophic mismatch in release | Add a checked-negation helper in fixed_decimal mapped to a named reconciliation refusal | FIXED (written, awaiting human build/test) — fix wave 1 (checked_neg helper + named run refusal) |
| FRV1-P16.02 | `FRV1-R7-003` | P1 | Payment lifecycle accepts duplicate transition ids; mutates chain storage before all checks pass | Enforce per-chain transition_id uniqueness; order mutation after all checks | FIXED (written, awaiting human build/test) — fix wave 2 (DuplicateTransitionId named refusal; mutation only after all checks) |
| FRV1-P16.03 | `FRV1-R7-004` | P1 | SDA with a zero baseline window sum silently records a no-op adjustment instead of a named gap | Name the gap instead of the silent no-op | FIXED (written, awaiting human build/test) — fix wave 2 (SdaReferenceWindowZero recorded gap) |
| FRV1-P16.04 | `FRV1-R7-005` | P1 | CAISO's 20% SDA bound enforced as a universal jurisdiction bound | Scope the 20% bound to CAISO; other programs get their recorded bounds | FIXED (written, awaiting human build/test) — fix wave 2 (SDA bound is recorded per-jurisdiction data; CAISO carries 20%; unrecorded jurisdictions refuse, never assume) |
| FRV1-P16.05 | `FRV1-R1-009` | P2 | BaselineEngineError::EmptyField used for a non-empty-field violation | Use the correct variant | FIXED (written, awaiting human build/test) — fix wave 2 (NegativeTimestamp/NonFiniteValue/InvalidEnumValue/InvertedValidityWindow named variants) |
| FRV1-P16.06 | `FRV1-R7-007` | P2 | EmptyField used as the error variant for non-empty semantic failures across the settlement cluster | Use correct variants | FIXED (written, awaiting human build/test) — fix wave 2 (same named-variant conversion across the settlement cluster) |
| FRV1-P16.07 | `FRV1-R7-008` | P2 | Dead SettlementParseError::CurrencyInvalid variant and duplicated render_tenths helper | Remove the dead variant; dedupe the helper | FIXED (written, awaiting human build/test) — fix wave 2 (dead CurrencyInvalid removed; render_tenths deduped to a single home via re-export) |
