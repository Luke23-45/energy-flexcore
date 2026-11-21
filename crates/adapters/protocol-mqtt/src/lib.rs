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
//!   and the composition point on the edge-agent `UplinkChannel` seam
//!   (composed in `crates/edge-agent/src/adapters/edge_to_cloud.rs`; no
//!   edge-agent dependency).
//! - A **transport seam** ([`transport`]) and a **scope declaration +
//!   capability manifest** ([`scope`]) consuming the COMMON protocol port
//!   layer (P14.01, `crates/flexcore-protocol-ports`).
//! - **FIM-F05 (written-unrun): the live wire stack.** A hand-written
//!   MQTT 5 client codec ([`mqtt_wire`]) and a hand-written minimal
//!   Sparkplug B protobuf encoder/decoder ([`sparkplug_wire`]) — no MQTT
//!   client crate, no prost, no new dependency (FIM standing decision 1)
//!   — plus the live transport ([`live`]) implementing the UNCHANGED
//!   `MqttUplinkTransport` seam: CONNECT with the NDEATH LWT, QoS 1
//!   publish with PUBACK-wait receipts, NBIRTH/DBIRTH on connect with a
//!   `BirthDeathTracker` self-check, the command-topic delivery surface,
//!   TLS via tokio-rustls. **Every byte of it has never been compiled or
//!   executed; broker interop and Sparkplug B conformity are UNPROVEN and
//!   remain a HUMAN LAB GATE** (conventions §7.3). The Sparkplug wire
//!   FIELD TABLE is no longer a declared pin: on 2026-11-21 it was
//!   verified against the primary text (the Eclipse Tahu
//!   `sparkplug_b.proto` and the Sparkplug specification's payload and
//!   operational-behavior chapters — sources and retrieval date recorded
//!   in `sparkplug_wire.rs`'s module header) and the pinned fixtures were
//!   recomputed from it; bdSeq travels as a METRIC named `bdSeq`
//!   (datatype Int64) in NBIRTH/NDEATH only, not as a payload field.
//!
//! What this crate is NOT:
//!
//! - Not a Sparkplug B conformance claim: Sparkplug B certification-style
//!   testing is a human-gated process (conventions §7.3); the pinned
//!   byte fixtures pin the field mapping VERIFIED against the primary
//!   `.proto` text (2026-11-21), and conformity against a real consumer
//!   stays the human lab gate.
//! - Not connected: no broker has ever been reached, no message has ever
//!   been exchanged, no topic has ever been published or subscribed on
//!   any wire (the entire FIM-F05 surface is WRITTEN-UNRUN).
//! - Not a dispatch authority: the command path DELIVERS received
//!   commands into the edge command lifecycle at PROPOSED and nowhere
//!   else — no authorization, no dispatch, no relay (the frozen
//!   lifecycle and never-granted set are untouched).
//! - Not a secret holder: no password field exists anywhere in this
//!   crate (the CONNECT password flag is never set; authentication
//!   travels the TLS client-certificate identity — absence is structural).
//! - Not a NATS-at-the-edge productization: OpenFMB's grid-edge NATS
//!   evidence [S-40] is recorded as evidence in the decision record;
//!   FlexCore's designed edge transport here is MQTT 5 + Sparkplug B, and
//!   the existing `crates/adapters/nats` crate stays the platform-internal
//!   bus. No NATS edge transport is added.
//!
//! Standard-detail honesty (plan §7 note 6): every topic segment spelling,
//! namespace prefix, message-type string, and sequence-wrap semantic below
//! is UNVERIFIED against the Sparkplug B specification (implementation-time
//! check: obtain the specification text, confirm topic grammar,
//! message-type strings, the seq wrap at 256, and the STATE topic
//! semantics). The payload FIELD TABLE and the bdSeq mechanism are the one
//! recorded exception: verified 2026-11-21 against the primary text (see
//! [`sparkplug_wire`]'s module header) — bdSeq travels as a metric named
//! `bdSeq` (datatype Int64) in NBIRTH/NDEATH, the specification wraps it
//! after 255, and the model's declared saturating `bd_seq` counter remains
//! the model-level design pin (its own UNVERIFIED label stands). A
//! mismatch is a recorded edit, never a silent reinterpretation.

// P14.20 — the adapter-side backpressure binding over the declared
// per-edge-node uplink budgets (additive module; decision record 83).
pub mod backpressure;
pub mod birth_death;
pub mod credentials;
// FIM-F05 (written-unrun): the live MQTT 5 + Sparkplug B wire stack —
// the hand-written codecs and the live transport. Interop UNPROVEN;
// HUMAN LAB GATE (module headers carry the full honesty registers).
pub mod live;
pub mod model;
pub mod mqtt_wire;
pub mod scope;
pub mod sparkplug_wire;
pub mod topic;
pub mod transport;
pub mod uplink;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope/module tests construct with.
pub const ADAPTER_ID: &str = "mqtt-sparkplug-uplink";
