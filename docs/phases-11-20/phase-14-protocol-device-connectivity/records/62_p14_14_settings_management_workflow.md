# P14.14 — Inverter settings-management workflow

| Field | Value |
|---|---|
| Subtask | P14.14 — certified baseline (utility-owned) vs aggregator schedules (DERControl-shaped); primacy/precedence rules; utility-override hook (1547-2018 Clause 10); a settings change is a lifecycle command with the approval gate — never a direct adapter write [S-20][S-156] |
| Wave / agent | Phase 14 wave 2, Agent B (P14.13–P14.16) |
| Status | WRITTEN-UNRUN: the workflow module and its primacy/override/lifecycle tests are written, never compiled, never executed. No settings change was issued, no device was touched, no baseline exists outside in-process test data. |
| Sources | [S-20] IEEE 1547-2018 (the Clause 10 utility-rights statement is an UNVERIFIED transcription from secondary material; the standard text was NOT consulted), [S-156] SunSpec IEEE 2030.5 in production (the CSIP primacy practice); the wave-1 PROPOSED-entry pattern (records 02 §6 and 21 §3) and the frozen 14-state/27-edge lifecycle (`crates/flexcore-domain/src/command_state.rs`); the P14.13 repository (record 61) |
| Files touched | `crates/flexcore-domain/src/settings_management.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). The frozen lifecycle (`command_state.rs`) is UNTOUCHED — byte-identical. |

## 1. What landed (deliverable class: domain workflow vocabulary)

| Piece | Where | Honest status |
|---|---|---|
| `SettingsAuthority` (Utility / Aggregator) with the declared precedence ranks (`UTILITY_RANK` = 0, `AGGREGATOR_RANK` = 1) and the mapping to the P14.13 record sources | `settings_management.rs` | Implemented; UNRUN |
| `SettingsChangeCandidate` — authority, target asset, command id, the new `FunctionSetting`, DECLARED primacy, declared creation instant, provenance note; checked constructor | same | Implemented; UNRUN |
| `SettingsChangeProposal` — the lifecycle-shaped proposal: CONST `ENTRY_STATE = CommandState::Proposed`, `CLIENT_CONSTRUCTIBLE_STATES = {PROPOSED, REJECTED}`, `reject_at_gate` and `client_may_transition_to` running the FROZEN `try_transition`; the device-declared-function-support gate runs BEFORE the lifecycle opens | same | Implemented; UNRUN — the wave-1 PROPOSED-entry pattern, reused verbatim |
| `resolve_competing_settings` — the precedence rules (a) authority rank first, (b) declared primacy (lower wins; undeclared never beats declared within a rank), (c) earlier creation, then deterministic command id; losers stay recorded (losing precedence is not rejection) | same | Implemented; UNRUN |
| `CertifiedBaseline::from_candidate` — the utility-ownership gate (a non-utility candidate is a named refusal) | same | Implemented; UNRUN |
| Tests: primacy (utility beats aggregator regardless of primacy numbers; primacy→creation→command-id tie-breaks with order-independence; undeclared primacy never defaulted in), override (enters at PROPOSED, never skips a gate, wins precedence only), lifecycle refusals (every non-frozen PROPOSED edge refused by the frozen table), declared-support refusal, baseline-ownership refusal, identifier validation | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. A settings change is a lifecycle command — never a direct adapter write

The rule, and its three enforcement layers:

1. **Structural**: `SettingsChangeProposal::ENTRY_STATE` is a `const`
   equal to `CommandState::Proposed`; no constructor names any other
   entry state. The only other state the workflow's callers construct is
   `CommandState::Rejected` (frozen transition 2), produced only through
   `reject_at_gate`, which calls the frozen
   `command_state::try_transition` — the frozen table, not this module,
   is the authority. The frozen 14-state/27-edge set is UNTOUCHED and
   byte-identical (`command_state.rs` not modified by this batch; the
   frozen-table tests in that file remain the oracle).
2. **Gates stay distinct**: the approval gate (PROPOSED → VALIDATED →
   FEASIBLE → APPROVED) is the platform's; this module runs none of the
   gates and submits nowhere. The utility-override hook changes
   PRECEDENCE ONLY — a test (`utility_override_enters_at_proposed_and_
   never_skips_the_gates`) pins that a utility override is refused by the
   frozen table for PROPOSED → ISSUED / APPROVED / FEASIBLE exactly like
   any aggregator change. Clause 10 gives the utility precedence among
   settings candidates; it gives no one authorization to skip the
   platform's approval gate.
3. **Device state is written only from lifecycle outcomes**: the P14.13
   state records (`AggregatorScheduleRecord`, `UtilityOverrideRecord`)
   each carry the `command_id` of the lifecycle command that produced
   them, and duplicate command ids are refused — one command, one state
   write, one audit anchor.

## 3. The primacy/precedence rules (declared, ordered, deterministic)

| Order | Rule | Source label |
|---|---|---|
| (a) | Authority rank first: `Utility` (rank 0) outranks `Aggregator` (rank 1) at ALL times, regardless of any declared primacy numbers | the 1547-2018 Clause 10 override right as DECLARED data — UNVERIFIED transcription [S-20] |
| (b) | Within a rank: declared primacy, lower wins; a candidate with NO declared primacy never beats one that declares (declared-never-inferred — the L4 rule of record 04) | CSIP production practice [S-156] — UNVERIFIED |
| (c) | Equal rank + equal primacy state: earlier `created_unix_ms`, then the deterministic command-id order (order-independence pinned by test) | the wave-1 `resolve_primacy` pattern (records 02/04) |

Losing a precedence comparison is not a rejection: losers stay recorded
in the resolved output and remain lifecycle candidates; SUPERSEDED
invalidation is the lifecycle's own edge, decided by the platform — never
by this module.

## 4. The certified baseline (utility-owned)

`CertifiedBaseline::from_candidate` refuses any non-utility authority BY
NAME (`BaselineNotUtilityOwned`) regardless of the candidate's lifecycle
state: an aggregator cannot rewrite the baseline, full stop. Its
schedules are separate, lower-precedence records (the P14.13
`AggregatorScheduleRecord`). Ownership decides WHO may propose a baseline
change; the change still passes all three gates like any other. A
baseline replacement is a new utility lifecycle command — the P14.13
state's `record_baseline` refuses a second silent baseline overwrite.

## 5. UNVERIFIED / ASSUMPTION register

| # | Item | Where |
|---|---|---|
| 1 | 1547-2018 Clause 10 content: that the utility holds settings-override rights and how they bind the aggregator — transcribed from [S-20] secondary material | `SettingsAuthority::UTILITY_RANK`, record 01 §3 (P3/P14.01) |
| 2 | The CSIP primacy convention (lower value wins; the utility's primacy enrollment) [S-156] | `resolve_competing_settings` rule (b) |
| 3 | That aggregator settings schedules are DERControl-shaped in production | the record-source mapping (`SettingsAuthority::setting_source`); the 2030.5 adapter's own mapping stays in its crate (record 02) |
| 4 | The declared creation instant is caller-supplied platform data; the workflow reads no clock (the injected-Clock rule belongs to the lifecycle engine) | `SettingsChangeCandidate::created_unix_ms` |

## 6. Honest boundaries

No settings change was issued; no command entered any lifecycle (no
lifecycle engine instance exists here — this module constructs proposal
VALUES, the engine consumes them); no utility, aggregator, or device
participated. The 0.x line is simulation-only (conventions §6.7); no
conformance claim of any kind.

## 7. Handoff notes

1. **Wave-3 (P14.17)**: the simulated 2030.5 client can deliver
   DERControl-shaped settings schedules that map into
   `SettingsChangeCandidate` — the mapping at the adapter boundary, not
   here (the domain never imports protocol types).
2. **The approval gate**: when the application layer wires the lifecycle
   engine to settings proposals, the APPROVED state is the operator/policy
   authorization event — the console's candidate-vs-executed distinction
   (report §20.2) applies to settings exactly as to dispatch.
3. **Human gates**: first build; the standard-text verification pass (§5).
