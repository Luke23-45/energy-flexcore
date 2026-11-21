//! Neryva FlexCore — the OCPP CSMS adapter (P17.01-P17.05).
//!
//! Decision records:
//! `docs/phases-11-20/phase-17-ev-fleet-charging/records/01_p17_01_ocpp_csms_adapter_architecture.md`
//! (CSMS architecture + framing + P17-E-1 registration),
//! `02_p17_02_csms_hosting_boundary.md` (hosting boundary + P17-E-2),
//! `03_p17_03_security_profile_3_onboarding.md` (Profile 3 onboarding),
//! `04_p17_04_boot_status_transaction_mapping.md` (EVSE registry mapping),
//! `05_p17_05_smart_charging_composer.md` (the composer + payload mapping).
//! This crate implements the P14.01 common port layer
//! (`flexcore-protocol-ports`) following the `crates/adapters/protocol-*`
//! adapter-crate pattern established in Phase 14.
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`scope`] | P17.01 | The reviewed CSMS scope declaration + capability manifest (common `ProtocolScopeDeclaration`/`ProtocolAdapterManifest`, `SupervisoryRelayOnly` authority) |
//! | [`model`] | P17.01/P17.03/P17.04/P17.05 | The OCPP message payload model types: BootNotification, StatusNotification, Heartbeat, TransactionEvent, the security block (SignCertificate / CertificateSigned / InstallCertificate / SecurityEventNotification), and the smart-charging block (SetChargingProfile payloads) — serde value types with UNVERIFIED spellings |
//! | [`framing`] | P17.01 | The OCPP-J JSON framing codec: CALL / CALLRESULT / CALLERROR envelopes, the action-name vocabulary, error codes, and the one-outstanding-CALL tracker (per direction) |
//! | [`session`] | P17.01 | Subprotocol negotiation (`ocpp2.0.1` baseline, `ocpp2.1` additive; 1.6 out of scope per P17-E-1 DEFAULT) and the session state machine |
//! | [`csms`] | P17.01/P17.02 | The CSMS server skeleton: deterministic connection registry, both-direction initiation, bounded queues, and the `CsmsTransport` seam (live transport implemented under FIM-F02, written-unrun — RFC 6455 conformance and real charge-point interoperability are HUMAN LAB GATES) |
//! | [`websocket`] | FIM-F02.01 | The hand-written RFC 6455 SERVER-side codec: HTTP/1.1 Upgrade handshake, accept-key (hand-written pinned SHA-1 in [`sha1`]), frame codec with masking enforcement, ping/pong, close codes, bounded fragmentation reassembly; NO permessage-deflate (refused by omitting the extensions header). Written-unrun; RFC 6455 conformance is a HUMAN LAB GATE |
//! | [`sha1`] | FIM-F02 | The hand-written pinned SHA-1 (FIPS 180-4 shape) used ONLY for the RFC 6455 accept-key hash, which the RFC mandates exactly; NOT a cryptographic-strength claim. Pinned by FIPS/RFC test vectors |
//! | [`dispatch`] | FIM-F02.02/FIM-F02.02b | The OCPP-J session layer over the codec: CALL/CALLRESULT/CALLERROR dispatch with id correlation (via the EXISTING trackers and bounded queues), boot/heartbeat/status/transaction/security-event answering through the EXISTING surfaces, the ISSUED-only charging-profile relay, and the refusal path — a charge point offering only `ocpp1.6` is refused with the EXACT P17-E-1 reason string, no new vocabulary |
//! | [`transport`] | FIM-F02.03 | The live `CsmsTransport` implementation over tokio: the listener loop, the RFC 6455 handshake, and the frame loop wired to the EXISTING server skeleton, session state machine, and dispatcher. PLAINTEXT only — no TLS termination exists anywhere (Profile 3 mTLS is a HUMAN LAB GATE); construction requires a recorded lab-posture reason. Written-unrun; real charge-point interop is a HUMAN LAB GATE |
//! | [`security`] | P17.03 | Security Profile 3 onboarding: per-device client-certificate records, the certificate flows, identity proofing at registration, and SecurityEventNotification ingestion (no CA is stood up) |
//! | [`hosting`] | P17.02 | The two CSMS hosting shapes (edge-agent-hosted vs control-plane-hosted) as declared data; the edge-agent-hosted depot shape is the P17-E-2 DEFAULT implemented per conventions §4 |
//! | [`provisioning`] | P17.04 | BootNotification / StatusNotification / Heartbeat / TransactionEvent mapping into the domain EVSE registry extension (`flexcore_domain::evse_registry`); connection loss is NOT inferred asset unavailability |
//! | [`charging`] | P17.05 | The SetChargingProfile payload mapping from the domain composer plan (`flexcore_domain::ev_charging_composer`); relay is typed to ISSUED lifecycle states only; "Accepted" = stored-not-applied reconciliation |
//!
//! # Honest status (read before consuming anything here)
//!
//! - **WRITTEN-UNRUN.** Every module here is design-baseline code that has
//!   never been compiled or executed; the first human build gates the whole
//!   Phase 0-10 workspace (program conventions §7.1).
//! - **Live transport, written-unrun (FIM-F02).** The WebSocket transport
//!   is now IMPLEMENTED (`websocket` + `dispatch` + `transport`) under the
//!   Final Implementation Milestone feature F-02 — as hand-written minimal
//!   codecs per the FIM standing decision 1
//!   (docs/releases/implementation_ledger/README.md) — and remains
//!   written-unrun: RFC 6455 conformance and real charge-point
//!   interoperability are HUMAN LAB GATES. No server has ever run, no
//!   charge point has ever connected, no OCPP message has ever been
//!   exchanged with a real device. There is no certification, conformance,
//!   metrology, or safety claim of any kind (phase-17 context §5.4).
//! - **UNVERIFIED protocol detail.** Every OCPP field name, message-type
//!   code, action name, subprotocol string, security-event name, profile
//!   purpose spelling, and composition-precedence rule below is cited from
//!   secondary sources ([S-121]-[S-125][S-126][S-127][S-133][S-141]) and
//!   carries an UNVERIFIED label naming the implementation-time check
//!   against the Open Charge Alliance specification text (OCPP 2.0.1 / 2.1
//!   parts, and the OCPP-J / "OCPP over WebSockets" transport part). A
//!   mismatch at that gate is a code or record edit, never a silent
//!   reinterpretation (phase-17 plan §7.5).
//! - **Charging profiles are commands.** They enter the command lifecycle
//!   at PROPOSED and pass the validation, feasibility, and authorization
//!   gates like any other candidate; the adapter relays only ISSUED
//!   lifecycle output and never constructs a lifecycle state from external
//!   input (phase-17 context §3 constraint 1; plan §7.4).
//!
//! # Scope and control status
//!
//! The entire 0.x line is simulation-only. FlexCore is a supervisory
//! orchestration layer: this adapter performs no primary frequency
//! response, no synthetic inertia, no relay protection, no inverter
//! inner-loop control, and no sub-cycle control. Charging control never
//! overrides vehicle/BMS protection limits — physical charging limits
//! (IEC 61851 charging modes [S-142]) are device-level truths the platform
//! respects, never commands around. External OCPP control objects (e.g.
//! NotifyChargingLimit-class inputs, Agent B's ingestion surface) enter the
//! command lifecycle at PROPOSED.

pub mod charging;
// P17.14 (wave 2 Agent A; ADDITIVE) — the OCA conformance use-case matrix
// SUBSET as declared data: the keyed use cases (provisioning, transactions,
// smart charging, security events, firmware status, negative) the
// written-unrun suite `tests/conformance_suite.rs` pins, plus the shared
// negative-case vectors (the tests/framing_codec.rs negatives restated with
// citation, the subprotocol-mismatch rows, and the certificate-failure
// rows). WRITTEN-UNRUN: execution and certification are HUMAN LAB GATES;
// zero conformance/certification/interoperability claims are made
// anywhere; no action, subprotocol, or state was added for the suite's
// sake. Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/43_p17_14_ocpp_conformance_suite.md`.
pub mod conformance;
pub mod csms;
// FIM-F02 (ADDITIVE) — the live transport stack: the hand-written RFC 6455
// server codec (`websocket`), the hand-written pinned SHA-1 the RFC 6455
// accept-key hash requires (`sha1`), the OCPP-J dispatch layer over the
// EXISTING skeleton surfaces (`dispatch`), and the live `CsmsTransport`
// implementation (`transport`). WRITTEN-UNRUN throughout; every module
// carries the FIM honesty header — RFC 6455 conformance and real
// charge-point interoperability are HUMAN LAB GATES, and no TLS
// termination exists anywhere (Profile 3 mTLS is a HUMAN LAB GATE). The
// EXISTING framing/session/csms surfaces are untouched except for the two
// ADDITIVE boot-outcome recording methods the dispatch layer calls
// (`CsmsServer::mark_boot_accepted` / `mark_boot_rejected`); no frozen
// vocabulary, state, or edge changed.
pub mod dispatch;
pub mod framing;
pub mod hosting;
pub mod model;
// FIM-F13 (Wave 6, final implementation milestone; ADDITIVE, WRITTEN-UNRUN)
// — the ISO 15118 Plug & Charge certificate-management CODE SURFACE
// (feature F-13; owner authorization: CODE SURFACE ONLY — ledger_F13.md,
// FIM standing decision 2): the V2G Root CA trust store consuming INJECTED
// anchors, never bundled ones (`pnc_trust_store`), the record-41
// certificate-flow handlers Get15118EVCertificate / CertificateSigned /
// InstallCertificate (`pnc_certificate_flows`), and the hand-written
// minimal DER reader with the fail-closed OCSP validation path
// (`pnc_ocsp`, the sha1.rs hand-codec precedent). THE FIVE RECORD-41 §3
// UNGATE CONDITIONS (a named PKI operator; trust-anchor distribution
// agreed; an OCSP service at fail-closed reliability; the OCA-text
// verification pass; D5 closed naming OCPP and the edition) ARE NOT MET
// AND ARE NOT CLAIMED MET: each is a fail-closed runtime/configuration
// gate that defaults to UNMET and every PnC handler refuses while any is
// UNMET, naming the unmet condition. Each condition is a HUMAN GATE. No CA
// is stood up; no certificate, key material, or trust anchor exists
// anywhere; the closed framing action vocabulary is NOT widened
// (Get15118EVCertificate is never a `framing::Action` variant — record 41
// §5.1); OCSP/DER conformance against a real responder and real PKI
// interop are HUMAN LAB GATES.
pub mod pnc_certificate_flows;
pub mod pnc_ocsp;
pub mod pnc_trust_store;
pub mod provisioning;
pub mod scope;
pub mod security;
pub mod session;
pub mod sha1;
pub mod transport;
pub mod websocket;

/// The registered adapter id (the composition-registry allowlist key shape;
/// the P6.12 registry pattern). Registered in the edge-agent /
/// control-plane composition registry only when a hosting shape (P17-E-2,
/// OPEN) is chosen by the human owner; this constant is the canonical
/// spelling.
pub const ADAPTER_ID: &str = "ocpp-csms";

/// The declared protocol identifier.
pub const PROTOCOL_ID: &str = "ocpp";

/// The declared protocol edition string. This is the edition the design
/// targets per the phase-17 gap analysis ([S-121][S-123]); it is a declared
/// design pin, NOT a conformance claim (conventions §7.3). The exact EDITION
/// (which published part set of 2.0.1/2.1 governs) is P17-E-1, OPEN — the
/// Edition 2 publication-date conflict in
/// `ledger/phase-06-edge-protocol/RESEARCH.md` §3 must resolve before D5
/// closes, and D5 stays human-closed.
pub const PROTOCOL_EDITION: &str = "OCPP 2.0.1 (P17-E-1 OPEN; 2.1 additive)";
