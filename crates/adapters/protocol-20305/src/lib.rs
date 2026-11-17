//! Neryva FlexCore — the IEEE 2030.5 AggregatorServer adapter (P14.02).
//!
//! Decision records:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/02_p14_02_aggregator_server.md`
//! (server design + model types), `03_p14_03_security_design.md` (the
//! security + PKI operations record), `04_p14_04_conformance_test_suite.md`
//! (the written-unrun test suite). This crate implements the P14.01 common
//! port layer (`flexcore-protocol-ports`).
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`model`] | P14.02 §2 | The CSIP resource-tree model types: FunctionSetAssignments, DERProgram / DefaultDERControl / DERControlList, ResponseList, PricingList, LogEventList, EndDevice/SFDI/LFDI — serde value types with UNVERIFIED attribute spellings |
//! | [`addressing`] | P14.02 §3 | The declared `AddressPlan` (resource paths declared, never hardcoded guesses), the list query semantics, and the declared primacy-resolution rules [S-156] |
//! | [`scheduling`] | P14.02 §2.1 | DERControl timing windows: start/duration/randomization modelling and the `active_at` / overlap semantics used by the conformance tests |
//! | [`server`] | P14.02 §4 | The AggregatorServer skeleton: deterministic resource store, GET/POST request routing, bounded lists, mTLS requirements (fail-closed construction), and the `AggregatorTransport` seam (UNRUN by design — no transport ships in this repo) |
//! | [`proposal`] | P14.02 §5 | The DERControl → command-lifecycle mapping: external control objects enter at PROPOSED, typed with the frozen `flexcore-domain` `CommandState`; the adapter can produce PROPOSED or record a REJECTION at the PROPOSED gate and nothing else |
//!
//! # Honest status (read before consuming anything here)
//!
//! - **WRITTEN-UNRUN.** Every module here is design-baseline code that has
//!   never been compiled or executed; the first human build gates the whole
//!   Phase 0–10 workspace (program conventions §7.1).
//! - **No protocol connected.** No server has ever run, no device has ever
//!   been reached, no message has ever been exchanged. The mTLS/HTTP
//!   transport is a designed seam, not an implementation.
//! - **No conformance claim of any kind.** CSIP certification is a
//!   human-gated lab process (conventions §7.3); the `tests/` suite is
//!   written-unrun evidence preparation (P14.04), never a result.
//! - **UNVERIFIED standard detail.** Attribute spellings, href conventions,
//!   primacy semantics, and SFDI/LFDI details are cited from secondary
//!   sources ([S-1][S-3][S-5][S-7][S-156]); every such point carries an
//!   UNVERIFIED label naming what the implementation-time check against the
//!   IEEE 2030.5-2018 text / CSIP Implementation Guide must confirm. A
//!   mismatch is a code or record edit, never a silent reinterpretation
//!   (plan §7.6).
//!
//! # Scope and control status
//!
//! The entire 0.x line is simulation-only. FlexCore is a supervisory
//! orchestration layer: this adapter performs no primary frequency
//! response, no synthetic inertia, no relay protection, no inverter
//! inner-loop control, and no sub-cycle control. External DERControls enter
//! the command lifecycle at PROPOSED and pass the validation, feasibility,
//! and authorization gates like any other candidate.

pub mod addressing;
pub mod model;
pub mod proposal;
pub mod scheduling;
pub mod server;

/// The registered adapter id (the composition-registry allowlist key shape;
/// the P6.12 registry pattern). Registered as PLANNED in the edge-agent
/// registry only when the edge seam wave (P14.18) lands — this constant is
/// the canonical spelling.
pub const ADAPTER_ID: &str = "ieee-2030.5";

/// The declared protocol identifier.
pub const PROTOCOL_ID: &str = "ieee-2030.5";

/// The declared protocol edition string. This is the edition the design
/// targets per the gap analysis ([S-1]: IEEE 2030.5-2018); it is a declared
/// design pin, NOT a conformance claim (conventions §7.3).
pub const PROTOCOL_EDITION: &str = "IEEE 2030.5-2018";
