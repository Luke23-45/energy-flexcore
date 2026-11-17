//! Neryva FlexCore — the common port layer for protocol adapters (P14.01).
//!
//! Every Phase 14 protocol adapter (`crates/adapters/protocol-*`: ieee-2030.5,
//! openadr, modbus, dnp3, 104, 61850, mqtt, cim) implements THIS layer — the
//! same discipline the edge-agent `EmsAdapter` port (P6.12) and the grid-api
//! `ScopeDeclaration` (P4.02) already enforce, lifted to the protocol-adapter
//! tier (decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/01_p14_01_adapter_architecture.md`).
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`scope`] | P14.01 (mirrors P4.02) | The mandatory `ProtocolScopeDeclaration`: declared never inferred, mandatory non-empty `not_modelled`, fail-closed `assert_supports` — no `Default`, no partially-declared scope |
//! | [`manifest`] | P14.01; conventions §6.4 | The `ProtocolAdapterManifest` capability manifest every protocol adapter constructs exactly once — with the one-variant `AuthorityDeclaration` (`SupervisoryRelayOnly`), so "dispatch authority" is not expressible |
//! | [`connection`] | P14.01 (mirrors P6.12) | The `ProtocolConnection` port: connect/disconnect lifecycle and honest connection state. Deliberately NO write path and NO data path — absence is structural (the P6.12 rule carried forward) |
//!
//! # Rules this crate encodes
//!
//! - **A protocol adapter is a transport, never an authorizer** (ADR-010
//!   extended; phase-14 context §3 constraint 1). External control objects
//!   (2030.5 DERControls, OpenADR events, 1547 settings changes) enter the
//!   protected command lifecycle at PROPOSED and pass the validation,
//!   feasibility, and authorization gates like any other candidate. This
//!   crate has no command type and no dispatch type — a port cannot carry
//!   what the layer does not name.
//! - **The never-granted set is untouched** (conventions §6.4):
//!   `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`
//!   stay byte-identical in `crates/flexcore-kernel/src/manifest.rs`; the
//!   manifest here cannot even express them.
//! - **Sync traits.** No port takes or returns a future or names an async
//!   runtime (the `flexcore-ports` house rule). A concrete adapter owns its
//!   I/O shape and bridges to this sync surface.
//! - **No vendor types.** No module names a wire library, HTTP framework, or
//!   TLS implementation; transports are adapter-owned.
//! - **Every adapter declares its scope** — there is no "universal protocol
//!   adapter". An undeclared-scope adapter is a construction error, exactly
//!   as for grid-model adapters (P4.02).
//!
//! # Scope and control status
//!
//! Capability contracts for the design baseline only. The entire 0.x line is
//! simulation-only; nothing in this crate connects to a device, exchanges a
//! message, or authorises field control. FlexCore is a supervisory
//! orchestration layer: no primary frequency response, no synthetic inertia,
//! no relay protection, no inverter inner-loop control, no sub-cycle
//! control.

pub mod connection;
pub mod manifest;
// P14.20/P14.21 — the shared protocol-adapter observability and
// backpressure vocabulary (registered metric family names, label rules,
// budget types). ADDITIVE module — decision records 83/84 under the
// phase-14 records directory; no existing module is changed.
pub mod observability;
pub mod scope;
