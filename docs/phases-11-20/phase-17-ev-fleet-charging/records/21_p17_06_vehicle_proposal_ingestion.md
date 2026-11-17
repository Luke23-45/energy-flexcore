# P17.06 — Vehicle-proposal ingestion (proposals are records, never auto-commands)

| Field | Value |
|---|---|
| Subtask | P17.06 — NotifyEVChargingNeeds / NotifyEVChargingSchedule (ISO 15118 proposals relayed by the station) and NotifyChargingLimit / ClearedChargingLimit (locally imposed limits) → constraint records; proposals are inputs and records, NEVER auto-commands (ADR-010 extension) [S-124][S-125] |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the domain vocabulary, the application assembly, and the worker routing are written with their tests, never compiled, never executed. No charge point is connected, no OCPP message was ever exchanged with a real device, no proposal was ever ingested, no ISO 15118 session exists. |
| Sources | [S-124] ocpp.md OCPP 2.0.1 smart charging; [S-125] ocpp.md OCPP 2.1. **Every message name, block reference, and payload-field behavior below is SECONDARY-SOURCED and UNVERIFIED against the OCA specification text** — the implementation-time check is §4. |
| A-dependence | The OCPP CSMS adapter is Agent A's P17.01 `crates/adapters/protocol-ocpp/` inbound path. A's P17.01 layout record had NOT landed when this batch was written (checked: `crates/adapters/` contains no `protocol-ocpp` at batch time). The adapter types are consumed **BY CITATION** — the application and worker modules define the adapter-neutral `InboundProposalMessage` / `EvInboundFrame` shapes A's inbound path constructs at its boundary. A layout mismatch when A's record lands is a recorded edit, never a silent reinterpretation. This is the reconciliation recorded for the task instruction "inbound-path modules consuming A's adapter types by citation". |
| Files touched | `crates/flexcore-domain/src/ev_proposal_records.rs` (NEW), `crates/flexcore-application/src/ev_proposal_ingest.rs` (NEW), `apps/worker/src/ev_ingest.rs` (NEW, shared with P17.07), `crates/flexcore-domain/src/lib.rs` (additive module registration only), `crates/flexcore-application/src/lib.rs` (additive module registration only), `apps/worker/src/lib.rs` (additive module registration only). |

## 1. What landed (deliverable class: ingestion + constraint records)

| Piece | Where | Honest status |
|---|---|---|
| The closed proposal-kind vocabulary `EvProposalKind` (ChargingNeeds / ChargingScheduleProposal / ChargingLimit / ClearedChargingLimit) with the OCPP message-name spellings carried as UNVERIFIED constants | `ev_proposal_records.rs` | Implemented; UNRUN |
| `EvProposalSource` (StationRelayed15118 / StationLocalLimit) — the declared origin, never inferred from payload shape | same | Implemented; UNRUN |
| `EvProposalPayload` — the declared fields (charging limit MW, min charging rate MW, target SOC, energy request MWh, windows) with validation; zero limit is a meaningful full curtailment, never a default | same | Implemented; UNRUN |
| `EvProposalRecord` — provenance-carried constraint record; a ClearedChargingLimit record must name the limit transaction id it clears (an audit record, never a silent deletion) | same | Implemented; UNRUN |
| `EvProposalRecord::lifecycle_entry_state()` — **structurally pinned to `CommandState::Proposed`**: a constant, not a parameter; no field, constructor, accessor, or conversion yields any other lifecycle state | same | Implemented; UNRUN |
| `InboundProposalMessageName` (application) — the closed four-family vocabulary with the total family→(kind, source) mapping; the cleared-id shape may not leak across families (refused, named error) | `ev_proposal_ingest.rs` | Implemented; UNRUN |
| `assemble_proposal_record` — the one assembly path message→record; the output type is the RECORD (the negative test pins that neither the output nor the error type can carry a command) | same | Implemented; UNRUN |
| `EvInboundFrame` routing (worker) — proposal frames route to the assembly; telemetry-shaped frames route to the EXISTING P2.07 pipeline's bounded channel; signed meter values route to the opaque evidence hooks | `apps/worker/src/ev_ingest.rs` | Implemented; UNRUN (shared module with P17.07) |

## 2. The ADR-010 extension (recorded here, referencing the ADR-010-EXT-01 pattern)

**Decision (the ADR-010-EXT-01 pattern applied to the EV proposal surfaces):
vehicle-proposal messages are inputs and constraint RECORDS, never
auto-commands.** Concretely:

1. A `NotifyEVChargingNeeds`, `NotifyEVChargingSchedule`,
   `NotifyChargingLimit`, or `ClearedChargingLimit` message enters the
   command lifecycle ONLY at `PROPOSED` and reaches a device only through
   the frozen 14-state lifecycle's three distinct gates (validation,
   feasibility, authorization) — the same rule ADR-010-EXT-01 fixes for
   external control objects ("an adapter that constructs an APPROVED or
   ISSUED state from external input is a defect, not a shortcut").
2. The entry state is a structural constant
   (`EvProposalRecord::lifecycle_entry_state()`), not a field — the type
   surface cannot express an auto-command.
3. Relay is not authority: the CSMS adapter relays the proposal and
   reports the transport outcome; authorization, issuance, mode
   selection, and sequencing stay in the platform.
4. The never-granted permission set (`dispatch.authorize`,
   `dispatch.issue`, `secret.read`, `network.any`) is untouched — no
   permission, transport, or command constructor was added anywhere in
   this batch (reviewed; the batch adds modules only).

**Where the extension is recorded.** The recorded statement lives in the
domain constant `PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT`
(`ev_proposal_records.rs`) and in this record. A formal
`ADR-010-EXT-02` file under `docs/architecture/adr/` was deliberately NOT
created: that directory is outside this agent's wave-1 file ownership
(plan §3), and creating an ADR-family record there without the
coordinator's edit scope would itself violate the ownership agreement.
The extension statement here follows the ADR-010-EXT-01 record form; if
the human owner wants it as an ADR file, this record is the source text.
ADR-010 itself and the EXT-01 file are UNTOUCHED (byte-identical).

## 3. What this module is NOT

1. **Not a dispatch path** — no charging-profile payload is produced
   anywhere; the P17.05 composer (Agent A) consumes constraint records by
   citation, downstream of the lifecycle gates.
2. **Not a device interface** — no charge point connected, no message
   exchanged, no station named.
3. **Not a validator of vehicle intent** — the payload is DECLARED data
   relayed by the station; nothing is validated against a vehicle.

## 4. UNVERIFIED register (implementation-time checks against the OCA text)

| # | Detail | Where |
|---|---|---|
| 1 | The four message-family spellings (`NotifyEVChargingNeeds`, `NotifyEVChargingSchedule`, `NotifyChargingLimit`, `ClearedChargingLimit`) and their OCPP 2.0.1 vs 2.1 block placement [S-124][S-125] | `EvProposalKind::message_name()`, `InboundProposalMessageName::as_str()`, `EV_PROPOSAL_FAMILIES` |
| 2 | The exact payload fields each message carries (which fields are optional; whether a charging-needs notification carries a limit at all) [S-124][S-125] | `EvProposalPayload` (all fields optional; a mismatch is a recorded edit) |
| 3 | Whether `ClearedChargingLimit` carries a transaction id in the OCA text (this module requires one — an unnamed clearance is not an audit record) [S-125] | `EvProposalError::ClearedWithoutLimitTransactionId` |
| 4 | Whether the station's locally imposed limits arrive as `NotifyChargingLimit` (vs being readable via `GetCompositeSchedule` only) [S-124] | the `ChargingLimit` kind |

## 5. Tests (written-unrun; execution is a human gate)

Closed vocabulary, the structural-PROPOSED pin, the statement constants,
valid-record round-trip, construction refusals (empty ids, empty notes,
negative/non-finite limits, out-of-range SOC target, inverted windows,
out-of-range timestamps), zero-limit-is-meaningful, cleared-limit audit
record shape, no-id-leak-across-families (application), the total
family→(kind, source) mapping, domain-refusal verbatim carry, and the
structural negative test that the module surface has no command or
dispatch path.
