//! P1.15 / P1.16 — NATS JetStream adapter for Neryva FlexCore:
//! stream/consumer provisioning, the transactional-outbox publisher, and the
//! idempotent consumer helper (report §15.4, §15.5, §6.3, §6.4; P1.14
//! topology design; ledger subtasks P1.15 and P1.16).
//!
//! # Delivery rule (P1.14 D-J6 — binding on everything in this crate)
//!
//! **At-least-once delivery, never exactly-once.** JetStream redelivers
//! unacknowledged messages; broker-side `Nats-Msg-Id` deduplication covers
//! only the configured duplicate window (a deployment courtesy, not a
//! guarantee). The durable guarantees are the §6.3 four-step outbox
//! discipline (state + outbox row in one transaction → publish → mark
//! delivered only after broker acknowledgement → idempotent consumers) and
//! the consumer inbox (P1.16). No code, log line, or document in this crate
//! claims exactly-once or zero-loss. The Jepsen finding on JetStream
//! durability (P1.14 §7, RESEARCH.md §5) is the reason the claims stay
//! modest: the broker is transport (report §16.1 row 5), never the
//! authoritative record.
//!
//! # Modules
//!
//! | Module | Subtask | Content |
//! |---|---|---|
//! | [`topology`] | P1.15 | P1.14 §3/§4 stream definitions, the `flexcore.v1.<area>.<event_slug>` → stream mapping, and the provisioning functions (Phase 1 streams `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ`; durable pull consumers) |
//! | [`publisher`] | P1.15 | [`publisher::OutboxPublisher`] — drains the outbox through the sync [`flexcore_ports::event_outbox::EventOutbox`] port, publishes to JetStream with `Nats-Msg-Id` = idempotency key, marks delivered **only after broker ack**, retries with backoff |
//! | [`inbox`] | P1.16 | The [`inbox::InboxStore`] consumer-side port and an in-memory test/dev implementation |
//! | [`consumer`] | P1.16 | The idempotent consumer helper: check inbox → skip; process → record inbox + state change in the handler's one transaction; ack only after commit; poison path → `FLEXCORE_DLQ` + alert, never a silent drop |
//!
//! # Recorded decisions (P1.15/P1.16)
//!
//! 1. **Sync port / async adapter resolution.** The `EventOutbox` port is
//!    deliberately sync (flexcore-ports rule: no port names a runtime), and
//!    the NATS adapter is inherently async. Resolution: the publisher keeps
//!    its own async API and calls the sync port directly — the port calls
//!    are short database round-trips issued between async broker
//!    operations, never held across an await of long duration. The
//!    composition root (apps/worker, P1.19) drives the publisher on a real
//!    tokio runtime and may, if its outbox adapter blocks long enough to
//!    matter, host the drain loop on a blocking thread with a channel
//!    bridge; the publisher does not require it. No ports trait is
//!    implemented here, so no `blocking_on` wrapper is needed: the adapter
//!    *consumes* the sync port; it does not *implement* a port. This is the
//!    mechanism recorded for the P1.30 boundary audit.
//! 2. **The `InboxStore` trait lives in this adapter crate, not in
//!    flexcore-ports.** The inbox is consumer-side infrastructure (a
//!    PostgreSQL table per P1.12/P1.13 and P1.16 step 1); the consumer
//!    helper defines the contract it needs, the same way an adapter-defined
//!    trait works, because flexcore-ports is out of this subtask's edit
//!    scope and the inbox is not a domain capability port — it is a
//!    delivery-deduplication table owned by the messaging infrastructure.
//!    The durable implementation (SQLX + the P1.12/P1.16 migration with the
//!    unique constraint on `(consumer_name, idempotency_key)`, consumed-at
//!    timestamp, and consumer identity) lands in
//!    `crates/adapters/postgres` (flexcore-postgres-adapter), which owns
//!    that schema. If a later phase promotes the trait to flexcore-ports,
//!    this crate's definition moves verbatim (single-home rule: one
//!    definition; the promotion must delete this one).
//! 3. **Outbox publisher hosting (P1.15 step 2, decided once).** The
//!    publisher is a library type ([`publisher::OutboxPublisher`]), not a
//!    process. apps/worker (P1.19) hosts it as a background task with the
//!    shutdown watch channel it already needs; this crate ships the type
//!    and the loop, the worker ships the runtime wiring.
//! 4. **No `futures`/`futures-util` dependency.** The async-nats
//!    streaming consumer APIs (`consumer.messages()`, `consumer.batch()`)
//!    require a `StreamExt` import from the `futures-util` or
//!    `tokio-stream` crates, which the P1.15 dependency ban (workspace
//!    dependency list only, no new external crates) forbids. The pull
//!    consumption in [`consumer`] therefore uses the documented JetStream
//!    pull protocol over the core request/publish surface
//!    (`$JS.API.CONSUMER.MSG.NEXT.<stream>.<consumer>` with a batch of 1,
//!    acks published to the message's reply subject) — the same protocol
//!    the client library uses internally. Every protocol detail carries a
//!    `VERIFY-BUILD:` note (a recorded build-time verification instruction,
//!    not a deferred task).
//! 5. **Integration tests are gated on `FLEXCORE_TEST_NATS_URL` (CI
//!    service, not testcontainers) and skip cleanly when it is unset.**
//!    Database-gated variants (`FLEXCORE_TEST_DATABASE_URL`) cannot live in
//!    this crate yet: the durable `InboxStore` implementation is the
//!    postgres adapter's (decision 2), so those tests land with it. This is
//!    recorded here and in the README rather than simulated.
//! 6. **Subject-scheme verification (P1.14 §9 binding).** See the README:
//!    the scheme *shape* matches P1.14 D-J1 (`flexcore.v1.<area>.<slug>`,
//!    lower_snake_case slugs, no tenant token, wildcards rejected) but the
//!    flexcore-events area tokens diverge from the P1.14 §3 area tokens on
//!    8 of 26 events. The publisher is scheme-agnostic (it publishes the
//!    outbox `destination_subject` verbatim), so the reconciliation is
//!    required in flexcore-events, not masked here.
//!
//! # Scope and control status
//!
//! Simulation-only infrastructure (AGENTS.md §5). The entire 0.x line is
//! simulation-only; nothing here touches a field device and nothing here
//! authorises field control. FlexCore is a supervisory orchestration layer:
//! no primary frequency response, synthetic inertia, relay protection,
//! inverter inner-loop control, or sub-cycle control.

pub mod consumer;
pub mod inbox;
pub mod publisher;
pub mod topology;
