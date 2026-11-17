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
// | [`supervision`] | The Class C worker supervisor (P7.18): per-call timeouts (injected clock), bounded deterministic retries, crash isolation, backpressure, and the in-module audit port — over the P7.17 `flexcore.worker.v1` worker contract |
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
// P11.12 (Phase 11 wave 1, Agent B) — the command consumer: consumer
// idempotency keyed on command_id with the frozen lifecycle state machine
// as the dedupe oracle (no-op + duplicate-delivery metric on invalid
// transitions); see the module header and the P11.12 decision record.
pub mod command_consumer;
// P11.05 (Phase 11 wave 1, Agent A) — OTel wiring for the worker: the
// W3C trace-context header carrier for the JetStream publish seam and
// the optional OTLP trace export. WRITTEN-UNRUN; see the phase-11
// record `records/05_p11_05_otel_python_and_trace_headers.md`.
pub mod otel;
pub mod inbox;
pub mod ingestion;
// P17.06/P17.07 (Phase 17 wave 1, Agent B) — the EV ingest bridge: the
// inbound-path extension routing the OCPP CSMS adapter's frames onto their
// seams — MeterValues frames into the EXISTING P2.07 pipeline's bounded
// channel (no parallel ingestion path), proposal messages onto the
// constraint-record path (NEVER auto-commands, ADR-010-EXT-01 pattern),
// signed meter values onto the opaque evidence-record hooks. The
// `protocol-ocpp` adapter is Agent A's P17.01 inbound path — consumed BY
// CITATION; see the module header and the phase-17 records
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 21_p17_06_vehicle_proposal_ingestion.md` and
// `22_p17_07_meter_values_ingestion.md`. WRITTEN-UNRUN.
pub mod ev_ingest;
pub mod maintenance;
pub mod probes;
pub mod runner;
// P7.18 — the Class C worker supervisor (timeouts, bounded deterministic
// retries, crash isolation, backpressure) over the P7.17 worker
// contract; see the module header for the placement and lifecycle-seam
// records.
pub mod supervision;
