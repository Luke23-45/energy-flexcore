# OCPP CSMS Adapter Guide (OCPP 2.0.1 / 2.1 over OCPP-J)

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-ocpp` adapter (`crates/adapters/protocol-ocpp/`): the OCPP CSMS SERVER role — FlexCore as the charging-station-management system that charge points connect TO (the adapter never dials out to a charge point, phase-17 context §3 constraint 2) |
| Owner records | `docs/phases-11-20/phase-17-ev-fleet-charging/records/01_p17_01_ocpp_csms_adapter_architecture.md` (architecture + framing), `02_p17_02_csms_hosting_boundary.md` (hosting), `03_p17_03_security_profile_3_onboarding.md` (Profile 3), `04_p17_04_boot_status_transaction_mapping.md` (provisioning), `05_p17_05_smart_charging_composer.md` (smart charging) |
| Implementation status | **WRITTEN-UNRUN.** Framing codec, subprotocol negotiation, session state machine, connection registry, bounded queues, security onboarding model, provisioning mapping, and the smart-charging relay exist as written code; nothing has compiled, no server has run, no charge point has connected, no OCPP message has ever been exchanged (`UnwiredCsmsTransport` refuses every serve with `CSMS_TRANSPORT_SEAM_NOT_WIRED`) |
| Conformance status | **No conformance, certification, interoperability, or safety claim of any kind.** OCA conformance testing is a human-gated lab process (the written-unrun use-case matrix in `tests/conformance_suite.rs` is evidence preparation, never a result) |
| Control status | Supervisory relay only. Charging profiles are commands: they enter the frozen 14-state command lifecycle at PROPOSED and pass the validation, feasibility, and authorization gates; the adapter relays ONLY already-approved ISSUED lifecycle output and never constructs a lifecycle state from external input |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | CSMS server (common SERVER role; `CommandRelay` control scope — relay of ISSUED output only, never a dispatch authority) |
| Framing | The OCPP-J JSON codec: CALL / CALLRESULT / CALLERROR envelopes (message-type codes 2 / 3 / 4), the action-name vocabulary, named error codes, and a ONE-outstanding-CALL tracker per direction — an unmatched reply is refused WHOLE, never guessed at |
| Subprotocol negotiation | `ocpp2.0.1` baseline with `ocpp2.1` accepted additively (P17-E-1 DEFAULT); negotiation NEVER falls back to an unspecified default and `ocpp1.6` is refused with the P17-E-1 reason (not backward compatible — 1.6 is `not_modelled`) |
| Both-direction initiation | The CSMS originates CALLs (SetChargingProfile relays, certificate flows) as well as answering charge-point CALLs; each direction carries its own outstanding-CALL tracker |
| Connection registry | A deterministic `BTreeMap` keyed by validated charge-point identity; duplicate identity refused, connection cap enforced by name (never an eviction of a live connection) |
| Hosting shapes | Edge-agent-hosted (depot) vs control-plane-hosted as declared data (`crate::hosting`); the edge-agent-hosted depot shape is the P17-E-2 DEFAULT, and the composition registry decides per site |

## 2. Deployment (what a deployment would configure)

| Configuration | Honesty note |
|---|---|
| `CsmsLimits` (outbound queue cap, connection cap) | declared per deployment, zero caps are construction errors; overflow REFUSES the enqueue with a named error — a silently dropped charging profile would masquerade as a delivered one |
| `CsmsSecurityRequirements` (trust-anchor labels, revocation check) | the trust anchors are LABELS to the deployment's PKI operations — no key material exists in this repository; an empty anchor set is a construction error |
| Per-connection cadences (Heartbeat, MeterValues) | per-connection DECLARED configuration, never inferred |
| Hosting shape per site | a composition-registry decision (P17-E-2 OPEN); not chosen by code |

## 3. Security (Profile 3 posture, no claims)

| Topic | Design position |
|---|---|
| Onboarding profile | Profile 3 (TLS with client certificates) is STRUCTURAL: there is no field that could disable TLS or accept Profile 1/2 for new onboarding; the onboarding profile is a constant (`onboarding_profile() == 3`) |
| Revocation | certificate revocation is checked before a session is accepted by DEFAULT; an unverifiable revocation status is a REJECTION, not a pass (the fail-closed P14.02 pattern) |
| Identity proofing | per-device client-certificate records and registration-time identity proofing are modelled (record 03); no CA is stood up in this program |
| Security events | SecurityEventNotification ingestion is in scope (EventLog resource class) |
| Secrets | Phase 13 secret REFERENCES only; `secret.read` stays in the never-granted set |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Backpressure | The adapter's own declared `CsmsLimits` IS the cap (the recorded no-module decision, record 83 §5 — FRV1-R6-002): the outbound CALL queue and the connection registry refuse by name (`OutboundQueueFull` with tracker unwind, `ConnectionCapReached`); RECORDED GAP, deliberate for v1.0.0: these refusals are crate-local, not in the shared `RateRefusal` vocabulary, so the `flexcore_protocol_adapter_messages_refused_total` label semantics do not yet cover this surface (folded in at the first OCPP composition) |
| Smart-charging relay | `into_relay_request` maps a composed plan + an ISSUED lifecycle state onto the wire SetChargingProfile payload; EVERY other lifecycle state is refused by name (`NotIssued` with the found state) — the relay is typed to ISSUED only |
| "Accepted" semantics | a charge point's `Accepted` SetChargingProfile response is STORAGE evidence, never application evidence ("stored-not-applied"; the recorded statement travels with the status, record 05 §6); application evidence comes only from the GetCompositeSchedule reconciliation, which compares like-with-like (the unit must match the plan's) and refuses a mismatched shape instead of reconstructing it |
| Connection states | The common `ProtocolConnection` vocabulary is consumed through the adapter's own `CsmsTransport::state` / `connection_report` seam (AwaitingSubprotocol → CONNECTING, Negotiated/Provisioned → CONNECTED, ended → DEGRADED with the named reason); the common `ProtocolConnection` port itself is the recorded FRV1-R6-001 deferral — no adapter implements it in this baseline, and the shipped `UnwiredProtocolConnection` default refuses fail-closed by name |
| Provisioning boundary | connection loss is NOT inferred asset unavailability — the registry entry's SESSION ends with a named reason and is retained for audit; domain-side asset states are untouched here (P17.04) |

## 5. Honest boundary and certification pointer

No server has ever run, no charge point has ever connected, no OCPP
message has ever been exchanged with a real device. The WebSocket/TLS
transport (`CsmsTransport`) is a designed, UNRUN seam — implementing it
is an implementation-time human gate (decision record 01 §7). Every
OCPP field name, message-type code, action name, subprotocol string,
security-event name, and composite-schedule shape is cited from
secondary sources and carries an UNVERIFIED label naming the
implementation-time check against the Open Charge Alliance text
(phase-17 plan §7.5); a mismatch at that gate is a code or record edit,
never a silent reinterpretation. OCA conformance testing is the
human-gated path (P17.14; records 101 pattern). Charging control never
overrides vehicle/BMS protection limits — physical charging limits are
device-level truths the platform respects, never commands around.
