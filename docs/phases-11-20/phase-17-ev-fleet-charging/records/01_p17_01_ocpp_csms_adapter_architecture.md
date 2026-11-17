# P17.01 — OCPP CSMS adapter architecture: OCPP-J framing, subprotocol negotiation, session state machine, the transport seam — and P17-E-1 registered OPEN

| Field | Value |
|---|---|
| Subtask | P17.01 (OCPP CSMS adapter architecture: WebSocket server DESIGN, OCPP-J JSON framing `CALL`/`CALLRESULT`/`CALLERROR`, subprotocol negotiation `ocpp2.0.1` baseline + `ocpp2.1` additive, 1.6 = P17-E-1, the one-outstanding-CALL discipline, both-direction initiation) |
| Wave / agent | Phase 17 wave 1, Agent A |
| Status | WRITTEN-UNRUN: model types, codec, state machines, server skeleton, and written-unrun test suites exist; nothing is compiled or executed; the WebSocket transport is a designed UNRUN seam — no charge point has ever connected, no OCPP message has ever been exchanged with a real device, no conformance/certification claim is made (conventions §1, §5, §7.3) |
| Sources | [S-121] (OCA OCPP protocols page), [S-122] (OCA What is new in OCPP 2.0.1), [S-123] (OCA OCPP 2.1 announcement), [S-125] (ocpp.md OCPP 2.1) — all cited, NOT consulted page-by-page: every wire-level detail carries an UNVERIFIED label naming the implementation-time check against the OCA text (plan §7.5); OCPP version facts from `ledger/phase-06-edge-protocol/RESEARCH.md` §3 |
| Files touched | `crates/adapters/protocol-ocpp/` (NEW crate: `Cargo.toml`, `src/lib.rs`, `src/scope.rs`, `src/model.rs`, `src/framing.rs`, `src/session.rs`, `src/csms.rs`, `src/security.rs`, `src/hosting.rs`, `src/provisioning.rs`, `src/charging.rs`, `tests/framing_codec.rs`, `tests/adapter_surface.rs`); root `Cargo.toml` (additive member + workspace dependency); `tools/check_dependency_rule.py` (additive `COMPOSITION_CRATES` + `MEMBER_DIRS`) |

## 1. Why a new crate (the conventions §6.2 test, applied)

The OCPP-J frame model, the codec, the subprotocol negotiation, the session
state machine, the CSMS registry skeleton, the security-onboarding types,
the EVSE-registry mapping, and the SetChargingProfile payload mapping are
REAL code with clear ownership (a protocol adapter, the P14 adapter-crate
pattern) and a stable dependency direction (composition side; it depends on
the CORE `flexcore-protocol-ports` layer and `flexcore-domain`; nothing
depends on it except future composition roots). The dependency line is
serde/serde_json + the two workspace crates — NO WebSocket library (the
tokio-tungstenite-class transport is NOT added), NO TLS stack, NO HTTP
framework: the transport is a designed, named UNRUN seam
(`src/csms.rs::CsmsTransport` with the shipped `UnwiredCsmsTransport`
refusing every operation with `CSMS_TRANSPORT_SEAM_NOT_WIRED`), the exact
P14 adapter pattern. The checker was updated ADDITIVELY
(`flexcore-ocpp-adapter` in `COMPOSITION_CRATES` and `MEMBER_DIRS`; the
member added to the root manifest with a placement note).

## 2. The CSMS is the server role — and the roles are declared

The scope declaration (`src/scope.rs`) consumes the common
`ProtocolScopeDeclaration` (P14.01) — the phase-17 plan §7.3 rule
("mirrors `crates/flexcore-grid-api/src/scope.rs` exactly"): no `Default`,
mandatory non-empty `not_modelled` (seven reviewed entries: no OCPP 1.6, no
PKI/CA operations, no ISO 15118 PnC PKI, no bidirectional/V2G control, no
wire transport, no protection-grade control, no conformance claim),
fail-closed `assert_supports`. Role: SERVER (charge points connect TO
FlexCore — phase-17 context §3 constraint 2). Control scope:
COMMAND RELAY — NOT dispatch authority; the authority statement (validated
at construction) names the PROPOSED-entry rule and the not-a-dispatch-
authority rule. Resource classes: Telemetry, Control, Discovery, EventLog.
The manifest is the one `ProtocolAdapterManifest` with the one-variant
`SupervisoryRelayOnly` authority.

## 3. The framing codec (`src/framing.rs`)

Module-level UNVERIFIED register (8 items across model/framing; each names
the OCA-text check). The codec is pure serde_json:

| Frame | Recorded shape (UNVERIFIED) | Message-type code |
|---|---|---|
| CALL | `[2, uniqueId, action, payload]` | 2 |
| CALLRESULT | `[3, uniqueId, payload]` | 3 |
| CALLERROR | `[4, uniqueId, errorCode, errorDescription, errorDetails]` | 4 |

Refusal is WHOLE (the ACL discipline): malformed JSON, non-array, wrong
arity, non-string element, empty/oversized unique id, payload-not-object,
unknown action name, unknown error code — each a named
`FramingError`. The action vocabulary is CLOSED (BootNotification,
StatusNotification, Heartbeat, TransactionEvent,
SecurityEventNotification, SignCertificate, CertificateSigned,
InstallCertificate, SetChargingProfile, GetCompositeSchedule,
ClearChargingProfile); anything outside it is refused, never a loose
pass-through. The CALLERROR code vocabulary (FormationViolation,
NotImplemented, SecurityError, InternalError, PropertyConstraintViolation,
OccurrenceConstraintViolation, TypeConstraintViolation, RPCFrameworkError)
is recorded with UNVERIFIED spellings.

## 4. The one-outstanding-CALL discipline

`CallTracker` enforces at most ONE outstanding CALL per direction per
connection: a second `track_call` is refused (never replaces the first);
a reply that matches no outstanding CALL is refused, never applied; the
timeout path drops the CALL so a late reply reports unmatched. Each
connection carries two trackers (inbound = the station's calls we answer;
outbound = the CSMS-initiated calls we await). Module register item 5:
whether the OCA transport text permits more than one outstanding CALL per
direction in a given version must be checked at implementation time — a
relaxation would be a reviewed, recorded code change, never a silent one.

## 5. Both-direction initiation

The CSMS originates CALLs (SetChargingProfile relays, GetCompositeSchedule,
certificate flows) as well as answering station CALLs
([S-121][S-124]). `CsmsServer::queue_outbound_call` enqueues into a bounded
per-connection queue (overflow refuses with a named error — a silently
dropped CSMS-initiated CALL would masquerade as a delivered one) and tracks
the call in the outbound tracker; `drain_outbound` preserves order for the
transport seam.

## 6. Subprotocol negotiation and the session state machine (`src/session.rs`)

- Accepted set: `ocpp2.0.1` (baseline) and `ocpp2.1` (additive; 2.1 is
  backward compatible per the Phase 6 research record — a recorded finding,
  not a verified conformance fact). `ocpp1.6` is refused WITH the P17-E-1
  reason attached (1.6 is NOT backward compatible with 2.0.1; supporting it
  would be the dual-stack option, a reviewed scope change). An empty offer
  is refused (no unspecified default — fail-closed). The peer's preference
  order is honoured, never reordered.
- Session states: `AwaitingSubprotocol → Negotiated{subprotocol} →
  Provisioned{subprotocol, charge_point}` (only via an ACCEPTED boot — the
  provisioning policy's decision, never this machine's), with
  `Ended{reason}` retained for audit. No frames are processed before
  negotiation; double provisioning is refused (re-boot goes through the
  explicit policy path).

## 7. The transport seam (UNRUN by design)

`CsmsTransport::serve` binds and serves; the shipped `UnwiredCsmsTransport`
answers with `CSMS_TRANSPORT_SEAM_NOT_WIRED` and reports `ConnectionState::
Closed`. Implementing the transport (WebSocket library choice, TLS
termination, the `Sec-WebSocket-Protocol` header negotiation, close-code
semantics) is an implementation-time human gate — the same posture as the
P14 `AggregatorTransport` seam. The server skeleton itself is real model
code: deterministic `BTreeMap` connection registry keyed by validated
charge-point identity, fail-closed construction (manifest-for-this-adapter,
non-zero limits, non-empty trust-anchor labels, Profile-3-onward security
requirements), honest `ConnectionReport` mapping (session states →
port-layer `ConnectionState`), and named errors everywhere.

## 8. P17-E-1 — OCPP editions baseline (registered OPEN — agents never close E-decisions, conventions §4)

- **Question:** which OCPP editions does the CSMS adapter speak?
- **Options:** (a) `ocpp2.0.1` baseline with `ocpp2.1` additive
  compatibility (2.1 is backward compatible per the Phase 6 research);
  (b) 2.0.1 only; (c) a 1.6 + 2.0.1 dual stack.
- **DEFAULT (RECOMMENDATION):** (a) — implemented here: `ocpp2.0.1` and
  `ocpp2.1` subprotocol strings accepted, `ocpp1.6` refused with the
  recorded reason; 2.1-only features remain design records, never
  half-implementations.
- **What closes it:** D5 (human) naming the OCPP EDITION as well as the
  version — the master-ledger D5 row requires exactly that; the "Edition 2"
  publication-date conflict in `ledger/phase-06-edge-protocol/RESEARCH.md`
  §3 must resolve first (open verification task). **D5 stays OPEN and
  human-closed; nothing in this phase closes it.**
- **D5 interplay (said once):** this adapter is the CANDIDATE D5 evidence —
  the design, mappings, and written-unrun suites D5 needs. It is not a
  closure.

## 9. Honest boundaries

1. WRITTEN-UNRUN throughout; the first human build gates everything (the
   0–10 base itself has never compiled).
2. No charge point connected, no OCPP message exchanged with a real device,
   no certification/conformance claim anywhere (phase-17 context §5.4).
3. Every field name, message-type code, action spelling, subprotocol
   string, and error-code spelling is UNVERIFIED against the OCA text
   (OCPP 2.0.1 / 2.1 parts and the OCPP-J transport part); a mismatch is a
   code or record edit, never a silent reinterpretation.
4. The workspace/checker edits are ADDITIVE only;
   `python -I tools/check_dependency_rule.py` output is quoted verbatim in
   the close-out record 06.
