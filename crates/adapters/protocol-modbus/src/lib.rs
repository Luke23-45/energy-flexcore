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
//!   surface as a port trait plus configuration types, with the
//!   [`transport::UnwiredTransport`] refusal as the not-wired default.
//! - An **MBAP + PDU frame codec** ([`codec`]) — the hand-written
//!   minimal Modbus TCP framing (FIM standing decision 1: no Modbus
//!   stack library exists in this program), pinned by byte-level test
//!   vectors.
//! - A **live Modbus TCP client** ([`tcp_transport`]) implementing the
//!   EXISTING port trait over tokio: connect/response timeouts,
//!   transaction-id matching, budget-honouring reconnect backoff, and a
//!   graceful close (FIM-F03).
//! - The **SunSpec device read flow** ([`sunspec`]): marker check →
//!   budget-chunked model-block walk → registry decode → adapter-boundary
//!   canonical conversion → telemetry rows carrying the frozen
//!   quality/provenance vocabulary (FIM-F03).
//! - A **scope declaration + capability manifest** ([`scope`]) consuming
//!   the COMMON protocol port layer (P14.01,
//!   `crates/flexcore-protocol-ports`) — the common
//!   `ProtocolScopeDeclaration` and the one `ProtocolAdapterManifest`
//!   shape every Phase 14 protocol adapter implements, with the
//!   structurally relay-only authority declaration.
//!
//! # Honest status (read before consuming anything here)
//!
//! - **WRITTEN-UNRUN.** Every module here has never been compiled or
//!   executed; the first human build gates the workspace (program
//!   conventions §1).
//! - **Live transport implemented (written-unrun); device interop
//!   UNPROVEN — HUMAN LAB GATE.** Under FIM-F03 (live Modbus TCP client;
//!   `docs/releases/implementation_ledger/ledger_F03.md`) the crate now
//!   HAS a TCP client ([`tcp_transport::LiveTcpTransport`]), a wire
//!   codec ([`codec`]), and the SunSpec read flow over them
//!   ([`sunspec`]) — none of which has ever connected, exchanged a frame,
//!   or answered a read from a real device. Conformance against a real
//!   Modbus TCP device is a HUMAN LAB GATE; the gated socket suite
//!   (`tests/live_transport_gated.rs`) skips cleanly without its
//!   `FLEXCORE_TEST_MODBUS_HOST` gate and is evidence preparation, not a
//!   result. The RTU serial path REMAINS unwired
//!   ([`transport::SeamNotWired`]) — absence is structural, not claimed.
//! - **Read-only: this crate has no write path.** Command-relay to
//!   Modbus devices (if ever) goes through the command lifecycle and a
//!   later, separately-reviewed design — absence here is structural (the
//!   EmsAdapter port rule, phase 14 context §2). The codec's write
//!   function codes exist for WIRE COMPLETENESS only; no transport
//!   method exposes them.
//! - **UNVERIFIED standard detail** (plan §7 note 6): SunSpec register
//!   semantics — the model ids, the common-block marker value,
//!   scale-factor rules, data types, byte order, and the meter sign
//!   conventions — are cited from the secondary source [S-16] and carry
//!   `UNVERIFIED` labels naming the implementation-time check against
//!   the SunSpec Modbus specification and a lab device. A mismatch at
//!   that gate is a recorded code or record edit, never a silent
//!   reinterpretation.

// P14.20 — the adapter-side backpressure binding over the declared
// gateway polling budget (additive module; decision record 83).
pub mod backpressure;
// FIM-F03.01 — the hand-written MBAP + PDU frame codec.
pub mod codec;
pub mod model;
pub mod polling;
pub mod registry;
pub mod scope;
// FIM-F03.03 — the SunSpec model read through the live client, plus the
// deterministic register fixture.
pub mod sunspec;
pub mod transport;
// FIM-F03.02 / FIM-F03.04 — the live tokio TCP client and the ONE total
// refusal mapping onto the transport taxonomy.
pub mod tcp_transport;
pub mod units_map;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "modbus-client";
