# P17.05 — The smart-charging schedule composer: dispatch envelopes → charging-profile plans → SetChargingProfile relay, with the stored-not-applied honesty rule

| Field | Value |
|---|---|
| Subtask | P17.05 (dispatch targets/envelopes → SetChargingProfile payloads: purpose selection — ChargingStationExternalConstraints at evseId 0 with isGridCritical, TxDefaultProfile, TxProfile; stack-level/purpose precedence; minChargingRate handling — 0 elsewhere when headroom < min rate; W/A conversion — AC A·V·phases vs DC A·V; GetCompositeSchedule reconciliation; "Accepted" = stored-not-applied honesty [S-124][S-141]. Charging profiles enter the lifecycle at PROPOSED and pass all three gates) |
| Wave / agent | Phase 17 wave 1, Agent A (composer: `crates/flexcore-domain` per the dependency rule; payload types: the adapter crate) |
| Status | WRITTEN-UNRUN: the composer, the plan type, the reconciliation, the lifecycle-typed relay, and the test suites are written model code, never compiled or executed; no charging profile has ever been sent to a charger; no conformance claim |
| Sources | [S-124] (ocpp.md OCPP 2.0.1 smart charging), [S-141] (Ampcontrol OCPP smart-charging guide); `crates/flexcore-domain/src/ev_fleet_model.rs` (the P3.11 model — the HARD departure policy and the site transformer limit the composer feeds), `crates/flexcore-domain/src/command_state.rs` (the frozen 14-state lifecycle) |
| Files touched | `crates/flexcore-domain/src/ev_charging_composer.rs` (NEW); `crates/flexcore-domain/src/lib.rs` (additive module entry); `crates/flexcore-domain/tests/ev_charging_composer.rs` (NEW); `crates/adapters/protocol-ocpp/src/charging.rs` (NEW); `crates/adapters/protocol-ocpp/src/model.rs` (the SetChargingProfile payload types, from record 01's batch); `tests/adapter_surface.rs` |

## 1. The split, per the dependency rule

The composer LIVES IN THE DOMAIN (`flexcore_domain::ev_charging_composer`)
— pure rules, no infrastructure; the WIRE PAYLOAD TYPES live in the
adapter (`crates/adapters/protocol-ocpp::model` + `charging`). The adapter
depends on the domain; the domain never sees the wire. The bridge between
them is lifecycle-typed (§4).

## 2. The composer (domain, pure)

| Rule | Implementation |
|---|---|
| Purpose selection | `compose_external_constraints_profile` → station scope (evseId 0) with `is_grid_critical` (the site/depot-limit path; the P14.15 DOE envelope and the site transformer limit surface upstream as constraint data — the composer converts, never decides); `compose_tx_default_profile` → per-EVSE default (station scope REFUSED); `compose_tx_profile` → per-transaction (transaction id REQUIRED, station scope REFUSED). `ChargingStationMaxProfile` is carried in the precedence record and the vocabulary but has no composer entry point in this batch |
| Precedence | The recorded order — external constraints, then station max, then Tx default, then Tx profile; higher stack levels override lower ones WITHIN a purpose — travels as `STACK_PRECEDENCE_STATEMENT` on EVERY plan, labeled UNVERIFIED against the OCA composition section [S-124]. The composer does not see the station's installed profiles; the reconciliation (§5) is where the ACTUAL composition is observed |
| minChargingRate | The unfulfillable-small-rate rule [S-124]: a positive limit below the declared minimum is NOT a valid alternative — it becomes 0 (off), and EVERY to-zero decision is a named record (`ComposerDecision::MinRateToZero` with the requested value, the minimum, and the unit). An explicit 0 never trips the rule. The minimum normalizes across units with the declared formula (`ComposerDecision::MinRateConversion`) |
| W/A conversion | Canonical MW → W exact (×1e6). W → A, AC: `A = (MW·1e6)/(V·phases)` (phases 1 or 3, declared per device profile — never inferred); DC: `A = (MW·1e6)/V` (no phase factor). Every conversion decision records its formula VERBATIM; missing/non-positive voltage or non-1/3 AC phases is a named refusal — no conversion is invented |
| Decision log | The plan carries `Vec<ComposerDecision>` — every conversion, every min-rate normalization, every to-zero — so every consumer sees what the composer did |

The plan (`ChargingProfilePlan`) carries NO lifecycle state and has NO
transition method — it is a payload CANDIDATE, structurally incapable of
being a command state.

## 3. Charging profiles are commands (the PROPOSED-entry rule)

The plan enters the command lifecycle at PROPOSED and passes the
validation, feasibility, and authorization gates like any other candidate
(phase-17 context §3 constraint 1; plan §7.4). The composer produces
candidate shapes only; nothing here constructs, mutates, or implies a
lifecycle state. Agent B's constraint-record surfaces (P17.06) and the
depot load-management model (P17.11) feed envelopes INTO this composer;
user-override precedence (P17.10) resolves BEFORE issuance — never inside
the adapter.

## 4. The lifecycle-typed relay (adapter)

`into_relay_request(plan, lifecycle_state, profile_id)` builds the wire
`SetChargingProfileRequest` ONLY from `CommandState::Issued` — every other
state of the frozen 14 is refused with the found state named
(`ChargingError::NotIssued`). An adapter that constructs APPROVED or ISSUED
from external input is a DEFECT, not a shortcut; this surface makes the
defect unrepresentable: the state arrives as the frozen enum from the
lifecycle engine, and no path exists from a protocol payload to a state.
The payload is re-validated against the model rules (TxProfile ⇒
transaction id, TxDefault ⇒ per-EVSE, ascending periods) before encoding.

## 5. "Accepted" = stored-not-applied (the honesty rule)

The charge point's `Accepted` response is STORAGE evidence, never
application evidence. `record_accepted_status` pairs the status with
`SetChargingProfileStatus::STORED_NOT_APPLIED_STATEMENT` (which names
ACKNOWLEDGED ≠ physical delivery and the reconciliation as the application
surface). Application evidence comes ONLY from the GetCompositeSchedule
reconciliation: `parse_composite_schedule_response` decodes the observed
composite (like-with-like units only — a unit mismatch is a named refusal,
never a silent conversion), and
`reconcile_composite_schedule` names EVERY deviation — `MissingPeriod`,
`ExtraPeriod` (another profile composed on top — the precedence
statement's business), `LimitDeviation` — as a
`ReconciliationOutcome::Mismatch`. The adapter NEVER retries or re-issues
on its own authority: a mismatch surfaces to the operator; the lifecycle
owns any replacement (SUPERSEDED is a lifecycle edge, not an adapter
behavior). No lifecycle transition is performed by any recording function
here — the lifecycle engine owns its edges.

## 6. UNVERIFIED register (implementation-time checks against the OCA 2.0.1 Part 2 smart-charging text [S-124][S-141])

| # | Detail | Where |
|---|---|---|
| 1 | The four purpose spellings and their composition precedence ORDER | `STACK_PRECEDENCE_STATEMENT`; the model register |
| 2 | evseId 0 = whole station | `PlanPurpose` scope rules; the model register |
| 3 | The minChargingRate semantics (positive-below-minimum ⇒ invalid, must be 0) | The composer's min-rate rule |
| 4 | The W/A rate-unit semantics and the A↔W formula OCPP expects (whether `numberPhases` scales the amperes) | The declared formula in every conversion decision |
| 5 | `isGridCritical` applicability to 2.0.1 (a 2.1-era field per secondary sources) | `SetChargingProfileRequest.is_grid_critical` |
| 6 | The GetCompositeSchedule response shape | `parse_composite_schedule_response` |
| 7 | The `ChargingProfileKind` values and Absolute anchoring | The model types |

A mismatch is a code or record edit, never a silent reinterpretation.

## 7. Honest boundaries

1. No charging profile has ever been sent to a charger; no composite
   schedule has ever been observed from a real device; the reconciliation
   is exercised only by written-unrun tests.
2. The composer feeds FROM the P3.11 model's constraints (site transformer
   limit, HARD departure energy) and the P14.15 DOE envelope — both
   upstream constraint data; the composer adds no new constraint authority
   and relaxes nothing (the departure policy is never touched here).
3. The entire 0.x line is simulation-only; no field write authority
   changes.
4. Domain and adapter test suites are WRITTEN-UNRUN (conventions §1.3);
   execution is a human gate.
