//! Neryva FlexCore — domain-event vocabulary and outbox abstraction (P1.09).
//!
//! This crate turns the frozen §15.3 event vocabulary (P0.14,
//! `ledger/phase-00-product-freeze/14_event_vocabulary.md`) into typed Rust
//! payloads with versioned serialization, and defines the transactional
//! outbox record (report §6.3, §15.5).
//!
//! | Module | Content |
//! |---|---|
//! | [`events`] | The 26 §15.3 events as a tagged `DomainEvent` enum with typed payload structs (correlation ID, tenant-scoped IDs, event time) |
//! | [`envelope`] | `EventEnvelope` owning the schema version and the **mandatory** idempotency key |
//! | [`outbox`] | The `OutboxRecord` type: serialized payload, destination subject, idempotency key, delivery state, attempt count, timestamps |
//! | [`subject`] | Subject-naming helper (`flexcore.v1.<area>.<event_slug>`; per the frozen P1.14 §3 mapping, D-J1) |
//!
//! # Frozen rules encoded here
//!
//! - **Events are facts, commands are imperatives** (P0.14 §2): an event
//!   such as `DispatchIssued` records that a command reached the ISSUED
//!   state; it is not itself the command and carries no imperative
//!   semantics. No event name may be reused as a command name.
//! - **Exactly 26 events**, spelled as in report §15.3; no `DispatchValidated`
//!   or `DispatchFeasible` exists and none may be invented.
//! - **Delivery is at-least-once** (NATS JetStream, report §15.4): consumers
//!   must be idempotent; exactly-once side effects are not promised by
//!   broker acknowledgements or deduplication alone. Duplicate side effects
//!   are prevented by application logic (inbox/idempotency keys).
//! - **Versioned serialization** (P1.09 step 4): the envelope carries a
//!   schema version; a breaking change requires a new version, never a
//!   silent field repurpose; an unknown version fails to parse.
//! - This crate defines **no transport dependency and no stream
//!   configuration**: subject naming here implements the frozen P1.14
//!   scheme (`flexcore.v1.<area>.<event_slug>`, D-J1) with the P1.14 §3
//!   area taxonomy; stream definitions themselves stay in the topology
//!   design.
//!
//! # Scope and control status
//!
//! Event types for the design baseline only. The entire 0.x line is
//! simulation-only; publishing events authorises no field control and no
//! device writes. `DispatchIssued` is a fact about the frozen command
//! lifecycle (P0.11), never an instruction to a device.

pub mod envelope;
pub mod events;
pub mod outbox;
pub mod subject;
