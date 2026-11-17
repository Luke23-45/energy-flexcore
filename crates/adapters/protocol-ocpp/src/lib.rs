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
//! | [`csms`] | P17.01/P17.02 | The CSMS server skeleton: deterministic connection registry, both-direction initiation, bounded queues, and the `CsmsTransport` seam (UNRUN by design — no transport ships in this repo) |
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
//! - **No charge point connected.** No server has ever run, no charge point
//!   has ever connected, no OCPP message has ever been exchanged with a real
//!   device. The WebSocket transport is a designed seam, not an
//!   implementation. There is no certification, conformance, metrology, or
//!   safety claim of any kind (phase-17 context §5.4).
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
pub mod framing;
pub mod hosting;
pub mod model;
pub mod provisioning;
pub mod scope;
pub mod security;
pub mod session;

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
