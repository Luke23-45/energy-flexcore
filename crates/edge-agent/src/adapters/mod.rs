// P6.12 — Protocol adapter interface and anti-corruption layer (report
// §9.1, §6.4 anti-corruption layer, §5.3, §2.3; ledger P6.12).
//
// Submodules:
//
// | Module | Responsibility |
// |---|---|
// | [`port`] | The protocol-agnostic `EmsAdapter` port: `discover` (channel/point inventory), `read` (one point → one native reading), `capabilities` (declared conventions — no defaults), `health`. Dispatch/command forwarding is EXPLICITLY NOT part of this port (P6.12 step 1: no direct dispatch in this phase; the read-only/test-lab default holds, P6.14 owns the gated receiver). |
// | [`acl`] | The anti-corruption layer: EMS-native shapes → canonical FlexCore types (§10.2 units/signs, §10.4 quality, §10.3 event/ingest time). Every mapping explicit; unknown/unmappable → named error; NEVER a default quality, NEVER an invented unit. |
// | [`registry`] | The adapter registry — the allowlist. Only registered adapter ids can be composed; the file/config enforcement point is P6.16 (commands) and the deployment profile (P6.20); this registry is the code-level allowlist. |
// | [`simulated`] | P6.08 (TEST/simulation-ONLY): the deterministic in-process scenario fixture behind the `simulated-test` registry id (`AdapterStatus::TestOnly`). It gives the read path an executable test source without OpenEMS; it is NEVER a real EMS and never presentable as one — the id names itself and every sample's `source_id` carries it. A production composition root must not wire it for a real site. |
// | [`openems`] | P6.10: the `openems-rest` adapter — EdgeConfig discovery, the versioned mapping profile, the unmapped/ambiguous-channel register, and read-only channel reads over a minimal HTTP/1.1 GET client (GET only; the read-only guarantee is structural, P6.11 depends on it). Every REST endpoint/JSON field is an explicit UNVERIFIED-ASSUMPTION (RESEARCH.md §1; report §27 OpenEMS references — never executed; the P6.11 lab run must verify). |
//
// Adapter selection state (recorded honestly, ENTRY_NOTE boundary 1):
//
// - `openems-rest` is the adapter id for the RECOMMENDED default (OpenEMS
//   first, per the report §2.3 priority) while **D5 stays OPEN** — the
//   concrete REST/WebSocket implementation is P6.10 (discovery/read) and
//   P6.13 (fault-injection tests). The OpenEMS REST API surface used here
//   is UNVERIFIED (never executed; RESEARCH.md §1) — the port above is
//   deliberately protocol-agnostic so the concrete adapter implements
//   whatever the verified API requires without touching this interface.
// - `generic-rest` is a RESERVED placeholder id — recorded, never
//   implemented, and never constructible (no hallucinated APIs; AGENTS.md
//   rule 1).
//
// Placement note (recorded in the decision doc): ledger P6.12 step 3
// places the concrete adapter crate at `crates/adapters/<protocol>/` per
// §8.2. No adapter crate exists yet (no empty crates, AGENTS.md §1); the
// port lives in the edge agent (the port holder) and a future
// `crates/adapters/openems` will depend on THIS crate for the port +
// ACL types — adapters depend on ports, never the reverse.

pub mod acl;
// P6.10 — the `openems-rest` adapter (EdgeConfig discovery, versioned
// channel-mapping profile, unmapped/ambiguous-channel register, read-only
// GET client; see the module header for the UNVERIFIED-ASSUMPTION register
// and the structural read-only guarantee).
pub mod openems;
pub mod port;
pub mod registry;
// P6.08 — TEST/simulation-only scenario fixture (see the module table and
// the simulated module header for the hard boundary).
pub mod simulated;

pub use port::{AdapterCapabilities, AdapterError, AdapterHealth, ChannelDescriptor, EmsAdapter, PointKey};
pub use registry::{AdapterRegistry, AdapterStatus, RegistryError};
pub use simulated::{SimControl, SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture};
// P6.10 — the concrete OpenEMS adapter surface (read-only; GET only).
pub use openems::{
    ChannelMappingEntry, ChannelMappingStatus, OpenEmsRestAdapter, OpenemsError,
    OpenemsMappingProfile, OpenemsRestTransport, RestResponse, TcpHttpGetTransport,
    OPENEMS_REST_ADAPTER_ID,
};
