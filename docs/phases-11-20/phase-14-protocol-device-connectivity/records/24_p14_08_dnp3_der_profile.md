# P14.08 — DNP3 support: IEEE 1815.2-2025 DER-profile data objects, OUTSTATION role first, master-role assessment, secure-transport posture

| Field | Value |
|---|---|
| Subtask | P14.08 (DNP3 per IEEE 1815.2-2025 DER profile — data objects; OUTSTATION role first; master role assessed as a record; secure-transport posture note with no claims) |
| Wave / agent | Phase 14 wave 1, Agent B |
| Status | WRITTEN-UNRUN: model types, outstation-role model, assessment record, posture note, and the transport SEAM — written, never compiled or executed; no SCADA connection exists, no message was exchanged (conventions §1; first human build is the program gate) |
| Sources | [S-14] (DNP Users Group, IEEE 1815.2-2025 release announcement — the standard text was NOT consulted); [S-15] (MESA IEEE 1815.2 Guidelines overview); the frozen command lifecycle (`crates/flexcore-domain/src/command_state.rs`); the contracts unit rule (`crates/flexcore-contracts/src/units.rs`); the P14.01 common port layer (Agent A) |
| Files touched | `crates/adapters/protocol-dnp3/` (NEW: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/outstation.rs`, `src/master.rs`, `src/secure_transport.rs`, `src/transport.rs`, `src/scope.rs`); additive root `Cargo.toml` entries; additive `tools/check_dependency_rule.py` entries |

## 1. What landed (deliverable class: design-and-model)

| Module | Content |
|---|---|
| `model.rs` | DER-profile data-object vocabulary: `Dnp3ObjectGroup` (BinaryInput, Counter, AnalogInput, AnalogOutputStatus, AnalogOutput, ControlOutput) with group numbers pinned in ONE place (1/20/30/40/41/12 — ALL UNVERIFIED); `EventClass` (Class0 static, Classes 1–3 events — UNVERIFIED semantics); `DerPointKind` (fleet quantities: active power export, reactive power, SOC, availability, energy delivered, active-power setpoint) with the reviewed quantity→object-group mapping; `DeclaredScaling` (canonical → source factor DECLARED per profile, never assumed; a reactive sign convention required exactly for reactive points — units.rs rule) |
| `outstation.rs` | The OUTSTATION role model: `OutstationModel` over a declared point inventory (BTreeMap, utility point list authoritative — index conflicts refused; empty inventory refused); canonical → source-scaled conversions per declared scaling (MW→kW etc.); reactive direction via `interpret_reactive_power` under the point's declared convention; the event-queue design (classed events; class-0 points do not queue); `map_control_to_proposal` — the control-arrival path |
| `master.rs` | The master-role ASSESSMENT record: verdict DEFER with mandatory reasons and reopen evidence (a named utility SCADA integration, its point list, its security requirements); `assert_role_available()` refuses the master role by name — there is no master-role type anywhere in the crate to construct |
| `secure_transport.rs` | The posture note: declared posture (`Required` / `TestLabOnly`), mandatory non-empty open-questions list (three UNVERIFIED anchors: the 1815.2-2025 secure-authentication profile requirement, utility TLS expectations, key-management mapping to the Phase 13 secrets surface) and mandatory not-claimed list (no security/conformance/certification/compliance claim; no control implemented; no connection exists). A posture note without recorded unknowns is a construction error |
| `transport.rs` | The wire SEAM: `LinkConfig` (Tcp / Serial with declared addresses and parameters — the well-known DNP3 TCP port is a configuration choice, never a default); `OutstationReport` (static points + drained event queue — a record the seam would report); the sync `Dnp3Transport` port trait; `UnwiredTransport` refusing every report with `SeamNotWired` |
| `scope.rs` | CONSUMES the common port layer (P14.01): `outstation_scope()` (role SERVER — the outstation is the passive end; control scope MONITOR-ONLY; resource classes TELEMETRY + CONTROL; the master = common CLIENT role is NOT declared and is refused at the gate) and `outstation_manifest()` (the one common manifest shape, `SupervisoryRelayOnly`, capabilities Read + ExternalEventIngest) |

## 2. Dependence on Agent A's P14.01 layer (recorded)

Same reconciliation as record 21 §2: migrated onto the landed
`flexcore-protocol-ports` common scope/manifest in this batch. The role
mapping (outstation = common SERVER) is recorded in the scope notes; the
master-role refusal is enforced twice — structurally at the common scope
gate (CLIENT role not declared) and by name in `master.rs`.

## 3. The control-arrival rule (the reason the outstation model has a mapping)

**A DNP3 control arriving from the utility master is an external control
object. It enters the frozen 14-state command lifecycle at PROPOSED — never
as an issued setpoint.**

- `Dnp3ControlProposal::entry_state()` is a `const fn` returning
  `CommandState::Proposed` — structurally constant.
- The mapping refuses controls for non-control points and nil command ids;
  the commanded value converts INTO canonical MW at the adapter boundary
  through the point's declared scaling (inverse of the declared factor).
- The outstation never applies a control to the fleet and never returns a
  wire status from the mapping — the transport seam owns any response, and
  the select-before-operate arming sequence (UNVERIFIED) is an
  implementation-time design point recorded below.
- A test pins that `try_transition(PROPOSED, ISSUED)` is rejected by the
  frozen machine.

## 4. UNVERIFIED register (implementation-time checks against [S-14][S-15] and the utility's point list)

| # | Detail | Where |
|---|---|---|
| 1 | Object group numbers (1, 20, 30, 40, 41, 12) and variation selections | `model.rs::Dnp3ObjectGroup::group_number` |
| 2 | Event-class numbering and buffering/overflow behaviour | `model.rs::EventClass`; `outstation.rs` queue design (recorded as design, not behaviour) |
| 3 | Integrity-poll and unsolicited-reporting semantics | `outstation.rs` / `transport.rs` trait docs |
| 4 | Select-before-operate arming and control status codes | `outstation.rs::map_control_to_proposal` doc |
| 5 | Link-address semantics and the default TCP port | `transport.rs::LinkConfig` (declared, never defaulted) |
| 6 | What IEEE 1815.2-2025 requires for secure transport of DER-profile outstations | `secure_transport.rs` open questions |
| 7 | The utility's point list → point-index allocation | `outstation.rs` inventory (declared per deployment, authoritative) |

A mismatch found at those gates is a recorded edit to the named module —
never a silent reinterpretation (plan §7 note 6).

## 5. Honest boundaries

The transport is unwired: no DNP3 stack exists, no serial line or TCP
session was opened, no master polled this outstation, no control was
received, no message was exchanged. The master role is DEFER (assessment
record only). The posture note makes no security, conformance,
certification, or compliance claim of any kind (conventions §7.3; AGENTS.md
§3 rule 2). The DER-profile object model here is a design vocabulary for
the platform side — not a claim that any utility SCADA accepted it. The
simulated-counterpart seam is P14.17's deliverable (wave 3).

## 6. Permitted-tool outputs

Quoted verbatim in the batch close-out record
(`25_phase_14_wave1_agent_b_close_out.md`).
