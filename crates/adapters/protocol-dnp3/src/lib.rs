//! P14.08 — DNP3 (IEEE 1815.2-2025 DER profile) adapter models
//! (crates/adapters/protocol-dnp3).
//!
//! Decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/24_p14_08_dnp3_der_profile.md`.
//! Sources: [S-14] (DNP Users Group, IEEE 1815.2-2025 release),
//! [S-15] (MESA IEEE 1815.2 Guidelines overview). Plan: phase 14 plan §2
//! row P14.08.
//!
//! What this crate IS (the honest scope):
//!
//! - **DER-profile data-object model types** ([`model`]): the object
//!   groups/variations the DER profile exchanges for fleet telemetry and
//!   control — group/variation NUMBERS are UNVERIFIED secondary-source
//!   readings pending the standard text.
//! - The **OUTSTATION role model** ([`outstation`]): the role FlexCore
//!   would use to represent the DER fleet to utility SCADA — point
//!   inventory, static snapshots, event-class assignment, and the
//!   control-arrival path that enters the command lifecycle at PROPOSED.
//! - The **master-role assessment** ([`master`]): a record-level verdict
//!   (DEFER) with a named refusal — outstation first, per the phase plan.
//! - A **secure-transport posture note** ([`secure_transport`]): the
//!   declared posture toward DNP3 secure authentication/TLS, with no
//!   conformance or security claims of any kind.
//! - A **transport seam** ([`transport`]) and a **scope declaration +
//!   capability manifest** ([`scope`]) consuming the COMMON protocol port
//!   layer (P14.01, `crates/flexcore-protocol-ports`) — the common
//!   `ProtocolScopeDeclaration` (outstation = SERVER role; the master =
//!   CLIENT role is NOT declared and is refused at the gate) and the one
//!   `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
//!   implements, with the structurally relay-only authority declaration.
//!
//! What this crate is NOT:
//!
//! - Not a DNP3 stack. No framing, no link layer, no transport
//!   implementation; the wire transport is a designed seam.
//! - Not a DER-profile conformance claim: IEEE 1815.2 certification-style
//!   testing is a human-gated lab process (conventions §7.3).
//! - Not connected: no device is reached, no message is exchanged.
//!
//! Standard-detail honesty (plan §7 note 6): every object group /
//! variation number, class assignment, and select-before-operate semantic
//! below is UNVERIFIED against the IEEE 1815.2-2025 text [S-14] and the
//! MESA guidelines [S-15]; the implementation-time check is named at each
//! point. A mismatch is a recorded edit, never a silent reinterpretation.

// P14.20 — the adapter-side backpressure binding over the outstation
// event queue and the declared report cadence (additive module; decision
// record 83).
pub mod backpressure;
pub mod master;
pub mod model;
pub mod outstation;
pub mod scope;
pub mod secure_transport;
pub mod transport;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "dnp3-outstation";
