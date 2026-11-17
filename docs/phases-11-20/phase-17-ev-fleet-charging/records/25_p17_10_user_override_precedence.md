# P17.10 — User-override precedence policy

| Field | Value |
|---|---|
| Subtask | P17.10 — Driver manual starts > VPP curtailment, recorded as dispatch policy + negative tests (written-unrun); precedence resolves BEFORE issuance — an adapter never adjudicates and never refuses a lifecycle-issued command on its own authority [S-140] |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the policy module and its tests (including the negative tests) are written, never compiled, never executed. No driver interaction was observed, no vehicle is connected, no command was ever issued by this platform. |
| Sources | [S-140] DOE FEMP smart charge management (the driver-override-over-curtailment framing). The lifecycle and adapter-authority semantics come from the frozen lifecycle (P0.11) and ADR-010-EXT-01, which govern — no external claim is made. |
| Files touched | `crates/flexcore-domain/src/ev_override_policy.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive registration only). |

## 1. What landed (deliverable class: dispatch policy module + negative tests)

| Piece | Where | Honest status |
|---|---|---|
| `OverrideKind` (ManualStart / ManualStop) and `OverrideEvent` — the closed driver-override vocabulary, recorded data (the platform learns of an override through a reporting path, never by observing a device) | `ev_override_policy.rs` | Implemented; UNRUN |
| `PrecedenceOutcome` (NoOverrideApplies / CandidateYieldsToDriver / OverrideRecordedNoYield) — a RECORD the protected gates consume; no variant issues, authorizes, or mutates a command | same | Implemented; UNRUN |
| `resolve_override_precedence` — the pure policy function: an overlapping driver ManualStart over a candidate that would curtail (reduce or stop charging below the manual rate) yields the candidate back to the proposal path; a candidate that matches or raises the manual rate does not yield; a ManualStop is recorded without forcing a yield | same | Implemented; UNRUN |
| `OverridePolicyError::AdjudicationAfterIssuance` — the before-issuance rule in code: every stage from ISSUED onward is refused for adjudication; the frozen lifecycle (FAILED → FALLBACK, SUPERSEDED by a newer revision) governs issued commands, and no adapter may refuse them on its own authority either | same | Implemented; UNRUN |
| `USER_OVERRIDE_PRECEDENCE_STATEMENT` — the recorded dispatch-policy statement | same | Implemented; UNRUN |

## 2. The structural negative tests (written-unrun; execution is a human gate)

| Negative test | What it pins |
|---|---|
| `negative_adjudication_is_refused_at_and_after_issuance` | No call shape (adapter, consumer, or operator surface) can obtain a precedence decision for an ISSUED-or-later command from this policy — the before-issuance rule is structural |
| `negative_the_frozen_lifecycle_governs_issued_commands_not_the_policy` | The policy's output type carries no lifecycle state to mutate; the lifecycle engine's own frozen-edge tests remain the only transition authority |
| `negative_inverted_windows_and_empty_keys_are_refused` | The policy refuses to guess: inverted candidate/override windows and empty keys are named errors |
| `negative_the_policy_produces_no_command_and_holds_no_authority` | The output vocabulary is three record variants; the module adds no permission, no command constructor, no transport — the compile-time shape of "adapters never adjudicate" (the never-granted set is untouched by construction) |

## 3. Opt-out remains the P3.11 hard gate (referenced, not re-implemented)

The driver opt-out (start-inclusive, removing the vehicle from the
feasible set from the opt-out instant onward) lives in `ev_fleet_model`
(`DispatchedAfterOptOut`); this policy references it and governs only the
manual-START-vs-VPP-candidate precedence. No opt-out semantics are
duplicated here.

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The FEMP framing of driver override (app-initiated vs vehicle-initiated manual starts; whether both outrank SMQ curtailment in the reference deployments) [S-140] | `OverrideKind` (both shapes arrive as ManualStart; the distinction is upstream data) |
| 2 | Whether a driver manual STOP should ever block a VPP charge candidate (this module records it without forcing a yield — the conservative default) | `PrecedenceOutcome::OverrideRecordedNoYield` |

## 5. Honest scope notes

1. The policy resolves precedence for ONE candidate window against the
   vehicle's override events; scheduling across a fleet with many
   overrides is the composer's/scheduler's concern downstream.
2. The policy never sees an adapter context: its inputs are the lifecycle
   stage (a read-only parameter), the candidate window, the manual rate,
   and the override events. There is no transport, no device handle, and
   no refusal path for issued commands other than the refusal to
   adjudicate.
