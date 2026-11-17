//! P14.11 — MQTT 5 + Sparkplug B edge-transport adapter models
//! (crates/adapters/protocol-mqtt).
//!
//! Decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/43_p14_11_mqtt_sparkplug_b_uplink.md`.
//! Sources: [S-39] (Ubidots MQTT-vs-Sparkplug secondary comparison),
//! [S-40] (OpenFMB — the NATS-at-the-grid-edge evidence, NOTED without
//! productizing it). Plan: phase 14 plan §2 row P14.11.
//!
//! What this crate IS (the honest scope):
//!
//! - **Sparkplug B payload and identity model types** ([`model`]): edge
//!   node / group / device identity, the message-kind vocabulary (BIRTH /
//!   DEATH / DATA / STATE), sequence and bdSeq counters, and the metric
//!   model. Every topic spelling and payload-structure detail is
//!   UNVERIFIED against the Sparkplug B specification text (no text in the
//!   repository); the implementation-time check is named at each point.
//! - The **declared topic namespace** ([`topic`]): validated topic
//!   components and the deterministic topic-string form. The namespace
//!   prefix and segment order are UNVERIFIED secondary-source readings.
//! - The **BIRTH/DEATH session state machine** ([`birth_death`]): the
//!   consumer-side rules that make BIRTH/DEATH state management structural
//!   — no data is trusted without a live BIRTH, a DEATH ends the session,
//!   every observation is accepted or refused BY NAME, never guessed.
//! - **Per-device credential bindings** ([`credentials`]): declared
//!   client-id / username bindings with the secret material deliberately
//!   ABSENT (the never-granted `secret.read` rule — the transport seam
//!   receives a credential HANDLE from the deployment's key source; this
//!   crate holds none).
//! - The **edge-agent uplink OPTION** ([`uplink`]): how canonical uplink
//!   points map to Sparkplug topics and metrics, per-point DECLARED units,
//!   and the named-but-NOT-WIRED composition point on the edge-agent
//!   `UplinkChannel` seam (wave 3 composition; no edge-agent dependency).
//! - A **transport seam** ([`transport`]) and a **scope declaration +
//!   capability manifest** ([`scope`]) consuming the COMMON protocol port
//!   layer (P14.01, `crates/flexcore-protocol-ports`).
//!
//! What this crate is NOT:
//!
//! - Not an MQTT client. No connect/subscribe/publish I/O, no broker, no
//!   TLS stack; the wire transport is a designed, UNWIRED seam.
//! - Not a Sparkplug B conformance claim: Sparkplug B certification-style
//!   testing is a human-gated process (conventions §7.3).
//! - Not connected: no broker is reached, no message is exchanged, no
//!   topic is published or subscribed on any wire.
//! - Not a NATS-at-the-edge productization: OpenFMB's grid-edge NATS
//!   evidence [S-40] is recorded as evidence in the decision record;
//!   FlexCore's designed edge transport here is MQTT 5 + Sparkplug B, and
//!   the existing `crates/adapters/nats` crate stays the platform-internal
//!   bus. No NATS edge transport is added.
//!
//! Standard-detail honesty (plan §7 note 6): every topic segment spelling,
//! namespace prefix, message-type string, sequence-wrap semantic, and
//! payload field below is UNVERIFIED against the Sparkplug B specification
//! (implementation-time check: obtain the specification text, confirm
//! topic grammar, message-type strings, seq wrap at 256, bdSeq encoding,
//! and the STATE topic semantics). A mismatch is a recorded edit, never a
//! silent reinterpretation.

// P14.20 — the adapter-side backpressure binding over the declared
// per-edge-node uplink budgets (additive module; decision record 83).
pub mod backpressure;
pub mod birth_death;
pub mod credentials;
pub mod model;
pub mod scope;
pub mod topic;
pub mod transport;
pub mod uplink;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "mqtt-sparkplug-uplink";
