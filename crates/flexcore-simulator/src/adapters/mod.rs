// P14.17 — The simulated protocol counterparts (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 81_p14_17_simulated_counterparts.md`).
//
// PURPOSE, stated plainly (the P6.08 `simulated` fixture boundary,
// carried to the protocol tier): every adapter is testable against
// simulated peers with NO hardware. Each module here lands ON the named
// transport seam its real counterpart designed — and on NOTHING else:
//
// | Simulated peer | Module | Seam it exercises (owning record) |
// |---|---|---|
// | 2030.5 device client + in-process server transport | [`sim_20305`] | `protocol_20305::server::{AggregatorTransport, ResourceRequestHandler}` (record 02 §5) |
// | OpenADR VEN | [`sim_openadr`] | `protocol_openadr::endpoints::VtnEndpointDesign` + `vtn::{VtnRegistry, DeliveryPlanner}` (record 21 §3) |
// | SunSpec Modbus device | [`sim_modbus`] | `protocol_modbus::transport::ModbusTransport` (record 23 §4) |
// | DNP3 master | [`sim_dnp3`] | `protocol_dnp3::transport::Dnp3Transport` (record 24 §4) |
// | MQTT broker sink | [`sim_mqtt`] | `protocol_mqtt::transport::MqttUplinkTransport` + `birth_death::BirthDeathTracker` (record 43 §2) |
// | CIM profile fixture source | [`sim_cim`] | `protocol_cim::rdf_profile::RdfProfileSource` (record 44 §2) |
// | OCPP charge point (P17.15, phase-17 record 44) | [`sim_ocpp`] | `flexcore_ocpp_adapter::csms::CsmsTransport` (the CSMS transport seam, phase-17 record 01 §7) — the in-process OCPP-J loopback the P14.17 promise named |
//
// HARD BOUNDARY (the same wording discipline as the P6.08 fixture):
//
// - **Code, never execution.** These are simulated peers the platform's
//   tests will compose; NOTHING has ever run: no protocol was connected,
//   no device was reached, no message was exchanged, no server was
//   bound, no broker was contacted, no profile file was read. Every test
//   that composes them is WRITTEN-UNRUN and its execution is a human
//   gate.
// - **No hardware, no wire.** There is no socket, no serial port, no
//   TLS, no HTTP, no broker protocol bytes anywhere in this module tree:
//   the seams carry design-surface values (the adapters' own model
//   types), and the simulated peers produce and consume exactly those.
//   No external dependency is introduced by this tree — every crate used
//   here is an existing workspace member.
// - **Deterministic by construction.** Like the Phase 2 profiles: a
//   simulated peer driven by the same schedule yields the same sequence
//   of design-surface values; there is no wall-clock read and no
//   randomness (the injected-instant rule — every evaluation instant is
//   a parameter).
// - **Never a real peer, never presentable as one.** Every type here
//   names itself in its identity strings (`sim-…` prefixes); a record or
//   log produced FROM a simulated peer is identifiable from its identity
//   alone, forever. Nothing here is evidence about real device or peer
//   behaviour.
// - **Same rules, same gates.** The simulated counterparts construct the
//   SAME reviewed scope declarations and capability manifests their real
//   counterparts construct (via the adapter crates' own constructors) and
//   pass the SAME kernel-side permission checks
//   ([`permissions`], record 64) — a simulated adapter cannot gain
//   authority its real counterpart lacks (the wave-2 handoff, record
//   63 §6).
//
// Placement decision (record 81 §2): the simulated counterparts live in
// THIS crate — `flexcore-simulator` is a COMPOSITION crate in
// `tools/check_dependency_rule.py`, and the dependency rule allows
// composition→adapter and composition→composition edges (only CORE→
// composition edges are forbidden, and nothing may depend ON a
// composition crate except apps). The arbiter said yes, so the
// counterparts did not have to move into the adapter crates as
// test-support modules.

pub mod permissions;
pub mod sim_20305;
pub mod sim_cim;
pub mod sim_dnp3;
pub mod sim_modbus;
pub mod sim_mqtt;
// P17.15 — the simulated OCPP charge points (phase-17 wave 2, Agent A;
// decision record 44): the in-process OCPP-J loopback wired THROUGH the
// CSMS adapter's named `CsmsTransport` seam, with the SYNTHETIC V2G
// behaviour profile attached only through the frozen five-condition
// V2xConfig gate.
pub mod sim_ocpp;
pub mod sim_openadr;
