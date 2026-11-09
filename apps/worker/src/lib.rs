// P1.19 — flexcore-worker: the background worker composition root
// (report §8.1 `apps/worker/`, §8.3 service-extraction policy, §6.3/§6.4;
// ledger subtask P1.19).
//
// The worker hosts exactly two kinds of long-lived background work and
// nothing else (no API server, no business use cases — the P1.19 risk note:
// "worker as a second API server" is scope creep):
//
// 1. The **outbox publisher** ([`flexcore_nats_adapter::publisher::
//    OutboxPublisher`], P1.15): drains pending transactional-outbox entries
//    through the sync `EventOutbox` port (implemented by the PostgreSQL
//    adapter's `FlexCoreStore`), publishes to JetStream with
//    `Nats-Msg-Id` = idempotency key, marks delivered ONLY after the broker
//    acknowledgement (report §6.3 steps 2–3).
// 2. The **idempotent consumers** ([`flexcore_nats_adapter::consumer::
//    IdempotentConsumer`], P1.16): durable pull consumers with inbox
//    deduplication — check inbox → skip; process → record inbox + state
//    change in one transaction; ack only after commit (report §6.3 step 4).
// 3. The **telemetry ingestion task** ([`ingestion`], P2.10): the P2.07
//    pipeline (validate → normalize → quality-assign → dedup → store →
//    publish via outbox) with quarantine and bounded backpressure,
//    consuming a `TelemetrySource` port (the simulator wires in at
//    composition, P2.14).
//
// # Modules
//
// | Module | Content |
// |---|---|
// | [`config`] | `FLEXCORE_*` environment configuration, fail-fast named errors, no secret logging |
// | [`inbox`] | The durable PostgreSQL `InboxStore` (P1.16 step 1) + the recorded single-home decision |
// | [`runner`] | The hosted loops: publisher drain loop, consumer pull loop, and the Phase 1 inbox-recording handler |
// | [`ingestion`] | The telemetry ingestion task (P2.10): the 7-stage pipeline, quarantine, bounded backpressure |
// | [`maintenance`] | The partition-maintenance job (P2.20): scheduled monthly pre-creation ahead of the ingest frontier + the approved-operation retention path (detach; drop is a second step) |
// | [`probes`] | `/health` (liveness) and `/ready` (readiness) probe server, liveness vs readiness distinguished |
//
// `src/main.rs` (binary `flexcore-worker`) wires everything: config →
// store → broker → topology provisioning → spawned loops → signal-driven
// graceful shutdown. The shutdown sequence is documented there and in
// README.md.
//
// # Sync-port hosting (recorded resolution, nats adapter lib.rs decision 1)
//
// The ports the worker drives are sync (`EventOutbox`, `InboxStore`); the
// PostgreSQL adapter implements them on a private runtime whose
// `block_on` **panics** when called from inside another runtime's async
// context. The worker therefore runs every port-touching section of its
// loops under `tokio::task::block_in_place` on the multi-thread runtime
// (see [`runner`]) — the blocking-thread mechanism the recorded decision
// names, without a channel bridge. The consequence for tests: anything
// that drives these loops needs a multi-thread tokio runtime (the
// integration test pins `flavor = "multi_thread"`).
//
// # Single-home decision (inbox store)
//
// The `InboxStore` trait is defined once in
// `crates/adapters/nats/src/inbox.rs` (P1.16). The durable implementation
// lives HERE in Phase 1 ([`inbox::PgInboxStore`]) because the P1.19 edit
// scope is `apps/worker/**` and the postgres adapter's migration set is
// outside it. Promotion into `flexcore-postgres-adapter` (with the table
// moving into the P1.12/P1.16 migration set) must delete this module —
// one home, one definition. Full rationale in [`inbox`].
//
// # Scope and control status
//
// Simulation-only infrastructure (AGENTS.md §5). The entire 0.x line is
// simulation-only; the worker grants no control authority, touches no field
// device, and performs no protection-grade function. FlexCore is a
// supervisory orchestration layer.

pub mod config;
pub mod inbox;
pub mod ingestion;
pub mod maintenance;
pub mod probes;
pub mod runner;
