//! P14.07 — SunSpec Modbus client adapter models
//! (crates/adapters/protocol-modbus).
//!
//! Decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/23_p14_07_sunspec_modbus_client.md`.
//! Source: [S-16] (SunSpec Modbus, sunspec.org/modbus). Plan: phase 14
//! plan §2 row P14.07.
//!
//! What this crate IS (the honest scope):
//!
//! - **Model types** for the SunSpec information models the client reads:
//!   the common block, model families 101/103 (inverters), 201/203
//!   (meters), and the 80x battery models ([`model`]).
//! - A **register-map registry** keyed per device model
//!   ([`registry`]): which fields live at which offsets, their data types,
//!   their scale-factor registers, and their unit kinds — reviewed data,
//!   not runtime discovery.
//! - **Unit conversion at the adapter boundary** ([`units_map`]): W → MW,
//!   VAr → MVAr with the DECLARED reactive sign convention (no default —
//!   `flexcore-contracts::units`), Wh → MWh, percent SOC → fraction.
//! - **Gateway polling budgets** ([`polling`]): registers-per-read,
//!   devices-per-gateway, and scan-interval budgets with fail-closed
//!   validation.
//! - A **transport seam** ([`transport`]): the Modbus TCP/RTU client
//!   surface as a port trait plus configuration types. NO Modbus stack
//!   library, no socket, no serial port: the wire transport is a designed
//!   seam a later wave implements.
//! - A **scope declaration + capability manifest** ([`scope`]) consuming
//!   the COMMON protocol port layer (P14.01,
//!   `crates/flexcore-protocol-ports`) — the common
//!   `ProtocolScopeDeclaration` and the one `ProtocolAdapterManifest`
//!   shape every Phase 14 protocol adapter implements, with the
//!   structurally relay-only authority declaration.
//!
//! Standard-detail honesty (plan §7 note 6): SunSpec register semantics —
//! the model ids, the common-block marker value, scale-factor rules, data
//! types, byte order, and the meter sign conventions — are cited from the
//! secondary source [S-16] and carry `UNVERIFIED` labels naming the
//! implementation-time check against the SunSpec Modbus specification and
//! a lab device. A mismatch at that gate is a recorded code or record
//! edit, never a silent reinterpretation.
//!
//! Read-only: this crate has no write path. Command-relay to Modbus
//! devices (if ever) goes through the command lifecycle and a later,
//! separately-reviewed design — absence here is structural (the EmsAdapter
//! port rule, phase 14 context §2).

// P14.20 — the adapter-side backpressure binding over the declared
// gateway polling budget (additive module; decision record 83).
pub mod backpressure;
pub mod model;
pub mod polling;
pub mod registry;
pub mod scope;
pub mod transport;
pub mod units_map;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "modbus-client";
