# P14.05 — OpenADR 3.0 VTN adapter: endpoint design, VEN management, and the event → PROPOSED mapping

| Field | Value |
|---|---|
| Subtask | P14.05 (OpenADR 3.0 VTN — REST/JSON event/report/registration endpoint design, VEN management, polling + push delivery, event-to-PROPOSED mapping) |
| Wave / agent | Phase 14 wave 1, Agent B |
| Status | WRITTEN-UNRUN: model types, endpoint design data, registry/planner, and mapping — written, never compiled or executed (conventions §1; the first human build is the program gate) |
| Sources | [S-10] (OpenADR Alliance, openadr.org — the specification-download page; the 3.0 text itself was NOT consulted and every protocol detail below is UNVERIFIED against it); the frozen 14-state lifecycle (`crates/flexcore-domain/src/command_state.rs`); the contracts unit rule (`crates/flexcore-contracts/src/units.rs`) |
| Files touched | `crates/adapters/protocol-openadr/` (NEW: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/scope.rs`, `src/endpoints.rs`, `src/vtn.rs`, `src/event_mapping.rs`, `tests/event_mapping.rs`); additive root `Cargo.toml` entries; additive `tools/check_dependency_rule.py` entries |

## 1. What landed (deliverable class: design-and-model)

| Module | Content |
|---|---|
| `model.rs` | OpenADR 3.0 model types the VTN role exchanges: `EventDescriptor` (event id, modification number ≥ 1, market context, priority, test-event flag), `EventSignal` + `SignalUnits` (kW/MW/A/percent/other), `Interval` (start/duration/payload), `OpenAdrEvent` (descriptor + targets + signals + program id), `Ven`, `ReportSpec` (structurally `telemetry_only`). JSON serde shapes for the REST/JSON surface. All field sets UNVERIFIED against [S-10] |
| `scope.rs` | CONSUMES the common protocol port layer (P14.01, `flexcore-protocol-ports`): `vtn_scope()` builds the common `ProtocolScopeDeclaration` (role SERVER, control scope MONITOR-ONLY, resource classes Control/Telemetry/Discovery, non-empty `not_modelled`, authority statement naming the PROPOSED entry and not-a-dispatch-authority — enforced by the common constructor); `vtn_manifest()` builds the one common `ProtocolAdapterManifest` with `SupervisoryRelayOnly` authority. Protocol-specific `DeliveryMode` (Polling/Push) declared here |
| `endpoints.rs` | The REST/JSON endpoint surface AS DATA: `RouteSpec` (method, path pattern, payload kinds, purpose) validated into `VtnEndpointDesign` — `/registration`, `/poll/{ven_id}`, `/push/{ven_id}`, `/report/{ven_id}`, `/vens` (FlexCore-specific). Every path literal is UNVERIFIED. NO HTTP framework: serving is a designed seam |
| `vtn.rs` | `VtnRegistry` (BTreeMap VEN inventory; registration/update separation; resolve_targets refuses unknown and not-accepted VENs — enrolment acceptance is an operator decision) and `DeliveryPlanner` (declared preference order over polling/push; per-VEN plan; named refusals for unservable modes). A plan is a RECORD — execution belongs to the serving seam |
| `event_mapping.rs` | `OpenAdrEventMapper::map_to_proposal` — THE load-bearing rule (§3 below). kW/MW payloads → canonical `ActivePowerMW` at the adapter boundary; validity window from the intervals; named refusals for test events, ambiguous power signals, non-power units, nil command ids, out-of-scope mappings |
| `tests/event_mapping.rs` | WRITTEN-UNRUN mapping tests: PROPOSED entry with no lifecycle shortcut; kW→MW with sign pass-through; named refusals; JSON round trips; registry+planner+endpoint-design composition; relay-free common manifest |

## 2. Dependence on Agent A's P14.01 layer (recorded)

Agent A landed `crates/flexcore-protocol-ports` + records 01–03 mid-wave (it was
absent at this agent's wait-check). The crate was reconciled onto the landed
layer in the same batch: the scope gate is the COMMON
`ProtocolScopeDeclaration::assert_supports` (mirroring A's `protocol-20305`
`proposal.rs` pattern), the manifest is the COMMON `ProtocolAdapterManifest`
(one shape, record 01), and the crate-local scope/manifest vocabulary that had
mirrored the grid-api pattern directly was removed in favor of the common one.
No dependence remains on anything not yet landed.

## 3. The mapping rule (the reason this subtask exists)

**An OpenADR event is an external control object. It enters the frozen
14-state command lifecycle at PROPOSED — never as an issued setpoint.**

Enforced structurally, in three layers:

1. `ExternalCommandProposal::entry_state()` is a `const fn` returning
   `CommandState::Proposed` — not a field, so no code path (here or in a
   caller) can construct a later entry state from external input.
2. `ADAPTER_CONSTRUCTIBLE_STATES` is exactly `{PROPOSED, REJECTED}` (frozen
   transition 2 is the only other state an adapter can produce evidence for);
   every forward state is produced by the protected gates.
3. The mapping tests pin that `try_transition(PROPOSED, ISSUED)` and every
   other gate-skipping shortcut is rejected by the frozen machine itself.

The adapter never opens the lifecycle record — that is the application/kernel
side. The proposal carries the §14.3-style reason text, the verbatim event
id / modification number / program id for audit, and the validity window;
SUPERSEDED invalidation of a superseded candidate is the lifecycle's own
edge, decided by the platform — never by the adapter.

## 4. Unit conversion at the adapter

Signal payloads arrive in the event's declared unit and leave as canonical
`ActivePowerMW` (frozen P > 0 = export rule). kW → MW is /1000
(UNVERIFIED that 3.0 payloads carry kW — the `SignalUnits` enum names the
check). Reactive power is deliberately NOT mapped: an OpenADR event carries
no reactive-sign convention, and the platform never assumes one
(`units.rs`) — the proposal type has no reactive field at all (a test pins
this). Negative payload values pass through unchanged; sign interpretation
is the platform's validation gate.

## 5. UNVERIFIED register (implementation-time checks against the [S-10] text)

| # | Detail | Where |
|---|---|---|
| 1 | Endpoint paths and verbs (`/registration`, `/poll/{ven_id}`, `/push/{ven_id}`, `/report/{ven_id}`) and path-parameter syntax | `endpoints.rs::canonical_vtn_design` |
| 2 | JSON payload field names and the units vocabulary | `model.rs` |
| 3 | Poll-frequency agreement semantics; push notification shape | `vtn.rs` |
| 4 | Report request/submission resource shapes | `model.rs::ReportSpec` |
| 5 | Whether VEN management is protocol-specified or platform-specific (recorded as platform-specific) | `endpoints.rs` |

A mismatch found at that gate is a recorded edit to the named module — never
a silent reinterpretation (plan §7 note 6).

## 6. Honest boundaries

No HTTP serving exists; no OpenADR connection exists; no VEN was reached; no
message was exchanged. Certification is a human-gated lab process
(conventions §7.3); this crate claims no conformance. 2.0b is out of scope
(record 22, P14-E-2).

## 7. Permitted-tool outputs

Quoted verbatim in the batch close-out record
(`25_phase_14_wave1_agent_b_close_out.md`).
