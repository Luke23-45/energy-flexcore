//! P14.05 — OpenADR 3.0 VTN adapter models (crates/adapters/protocol-openadr).
//!
//! Decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/21_p14_05_openadr_vtn.md`.
//! Source: [S-10] (OpenADR Alliance, openadr.org). Plan: phase 14 plan §2
//! row P14.05; conventions: `../01_PROGRAM_CONVENTIONS.md` (binding).
//!
//! What this crate IS (the honest scope):
//!
//! - **Model types** for the OpenADR 3.0 objects the VTN role needs:
//!   events (`model`), VEN registrations (`model`, `vtn`), and reports.
//! - An **endpoint design** — the REST/JSON surface as DATA
//!   (`endpoints::RouteSpec`), so the HTTP serving layer is a *designed
//!   seam*: the serving layer wires onto [`endpoints::VtnEndpointDesign`]
//!   without re-deciding the surface. Under FIM-F04 the live serving IS
//!   NOW IMPLEMENTED in this crate (`serving`, `transport`) — WRITTEN-UNRUN:
//!   it has never been compiled or executed, no socket was ever opened, and
//!   no server loop has ever run. VEN interoperability is UNPROVEN and
//!   remains a HUMAN LAB GATE (crate conventions §7.3); the endpoint paths
//!   and payload shapes stay UNVERIFIED against the 3.0 specification text
//!   [S-10] (record 21 §5).
//! - The **event → command-lifecycle mapping** (`event_mapping`): an
//!   OpenADR event arriving from a DR program is an EXTERNAL CONTROL
//!   OBJECT and enters the frozen 14-state lifecycle at **PROPOSED** —
//!   never as an issued setpoint (ADR-010 extended to protocol adapters;
//!   phase 14 context §3 constraint 1; plan §7 note 4).
//! - A **scope declaration + capability manifest** (`scope`) consuming
//!   the COMMON protocol port layer (P14.01,
//!   `crates/flexcore-protocol-ports`): the common
//!   `ProtocolScopeDeclaration` (declared never inferred, mandatory
//!   non-empty `not_modelled`, fail-closed `assert_supports`) and the one
//!   `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
//!   implements — with the structurally relay-only authority declaration,
//!   so the never-granted permissions are not expressible at all.
//!
//! What this crate is NOT:
//!
//! - Not a conformant OpenADR 3.0 VTN. Conformance/certification is a
//!   human-gated lab process (conventions §7.3); nothing here claims it.
//! - Not connected. No protocol is connected, no device is reached, no
//!   message is exchanged (plan §6 acceptance criterion 2). The FIM-F04
//!   live endpoints are WRITTEN-UNRUN code, not an exchange record: no VEN
//!   has ever registered, polled, or reported against them, and the
//!   outbound VTN → VEN push call is not implemented at all
//!   (`transport.rs` module header).
//! - Not a VEN client. The VTN (server) role is the declared scope; the
//!   VEN (client) role is out of scope (the common scope declares the
//!   SERVER role only).
//!
//! Standard-detail honesty (plan §7 note 6, the openems.rs
//! UNVERIFIED-ASSUMPTION pattern carried forward): every protocol detail
//! below cited from a secondary source — endpoint paths, JSON field names,
//! signal units, poll semantics — carries an `UNVERIFIED` label naming the
//! implementation-time check against the OpenADR 3.0 specification text
//! [S-10]. A mismatch at that check is a recorded code or record edit,
//! never a silent reinterpretation.

// P14.20 — the adapter-side backpressure binding over the agreed VEN
// poll frequencies and the fleet connection budget (additive module;
// decision record 83).
pub mod backpressure;
pub mod endpoints;
pub mod event_mapping;
pub mod model;
pub mod scope;
// FIM-F04 — the LIVE VTN endpoint serving over the designed seam (both
// WRITTEN-UNRUN; VEN interop UNPROVEN — a HUMAN LAB GATE):
// - `serving`: the VTN serving state (registry + planner + pending events
//   + report specs + the delivery log recording the event version in
//   force) behind the designed routes.
// - `transport`: the hyper 1.x HTTP/1.1 wire mapping (`VtnHttpService`)
//   and the live listener (`LiveVtnTransport`) with the fail-closed,
//   optional-by-config TLS posture.
pub mod serving;
pub mod transport;
pub mod vtn;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "openadr-vtn";
