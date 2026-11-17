// P1.19 — `flexcore-worker`: the background worker binary (composition
// root; report §8.1/§8.3, §6.3; ledger P1.19).
//
// Wiring: config → tracing → PostgreSQL store (outbox port + inbox store)
// → NATS/JetStream (topology provisioning) → spawned background loops
// (publisher, consumers, and — when FLEXCORE_SIMULATOR_SCENARIO is
// configured — the telemetry ingestion task over the composed simulator
// source, P2.10/P2.14/P2.25) + probe server → signal-driven graceful
// shutdown.
//
// # Shutdown sequence (recorded, ledger P1.19 step 3)
//
// On SIGTERM (or SIGINT):
//
// 1. **Stop accepting new work.** The shutdown watch channel fires; the
//    probe server enters graceful shutdown (readiness already went 503 —
//    see `probes`) and drains open connections.
// 2. **Finish or cancel in-flight work with a bounded timeout**
//    (`FLEXCORE_SHUTDOWN_TIMEOUT_MS`, default 10 s). Each loop finishes
//    its current step — the publisher its drain cycle (an entry is marked
//    delivered only after its broker ack), each consumer its single
//    in-flight message (ack only after the handler's transaction
//    committed) — then exits at the next shutdown check. If the timeout
//    elapses, remaining tasks are aborted; that is SAFE by design: an
//    aborted drain leaves the outbox entry pending (re-published later,
//    deduplicated by key within the duplicate window), and an aborted
//    message delivery stays unacknowledged (JetStream redelivers; the
//    consumer inbox deduplicates). At-least-once, never exactly-once.
// 3. **Close connections.** The NATS connection is closed by dropping the
//    client; the PostgreSQL pool is closed explicitly. Tasks that already
//    exited are joined first so nothing writes through a closing pool.
// 4. **Exit** 0 on clean drain, 1 if the bounded timeout was hit (the
//    operator sees that work was cancelled, not silently dropped).
//
// Startup is fail-fast: configuration errors abort before any connection;
// store/broker provisioning failures abort before any loop starts. URLs
// are never logged (no secret logging).

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;

use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::AssetId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_nats_adapter::consumer::ConsumerConfig;
use flexcore_nats_adapter::topology::{
    provision_durable_pull_consumer, provision_dlq_streams, provision_phase_1_streams,
    provision_stream, ConsumerPolicy, DeploymentLimits, StreamDefinition, STREAM_DISPATCH,
    STREAM_REGISTRY,
};
use flexcore_ports::repository::{AssetRepository, TenantRepository};
use flexcore_ports::telemetry_source::{TelemetryQuantity, TelemetryQuery, TelemetrySource};
use flexcore_postgres_adapter::FlexCoreStore;
use flexcore_simulator::{load_scenario, SimulatorTelemetrySource};
use flexcore_worker::command_consumer::{
    dispatch_consumer_policy, run_command_consumer, CommandConsumerMetrics,
    CommandStateGateway, PgCommandStateGateway,
};
use flexcore_worker::config::WorkerConfig;
use flexcore_worker::inbox::PgInboxStore;
use flexcore_worker::probes::{serve_probes, ProbeState};
use flexcore_worker::runner::{
    publisher_config, run_idempotent_consumer, run_outbox_publisher,
};
use tokio::sync::watch;

/// The Phase 1 durable consumer the worker runs (P1.14 §5.1
/// `<area>-<purpose>-v1` naming). The subject filter covers the registry
/// area; the inbox-recording handler is the Phase 1 side effect.
const REGISTRY_CONSUMER_DURABLE: &str = "registry-worker-v1";

/// Deployment limits for the provisioned streams. All four values are
/// deployment configuration per P1.14 §4.1/D-J4 — these are the worker's
/// defaults for a single-node simulation deployment (D-J5: replication 1;
/// clustering is a per-deployment decision). The duplicate window is the
/// broker-side courtesy only; the durable guarantees are the outbox and
/// the inbox, never the window.
///
// P11.13 — bounded retention (Phase 11 wave 1, Agent B): the worker's
// stream limits switch from the server-default `None` ceilings to the
// registered bounded defaults ([`DeploymentLimits::bounded_simulation_defaults`]
// — the values are REGISTERED RECOMMENDATIONS written down in the P11.13
// record before any run that could measure against them; production values
// remain a per-deployment decision). The dedicated per-family DLQ streams
// take their own tighter budget
// ([`DeploymentLimits::bounded_dlq_defaults`], provisioned below).
fn deployment_limits() -> DeploymentLimits {
    DeploymentLimits::bounded_simulation_defaults()
}

/// The Phase 1 consumer policy: AckExplicit durable pull consumer on the
/// registry stream (provisioned by [`provision_durable_pull_consumer`]).
fn registry_consumer_policy() -> ConsumerPolicy {
    ConsumerPolicy {
        durable_name: REGISTRY_CONSUMER_DURABLE.to_string(),
        stream: STREAM_REGISTRY.to_string(),
        filter_subject: "flexcore.v1.registry.>".to_string(),
        ack_wait: Duration::from_secs(30),
        max_deliver: 5,
        max_ack_pending: 100,
    }
}

fn main() {
    // Fail fast: parse the configuration before anything else.
    let config = match WorkerConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("flexcore-worker: configuration error: {error}");
            std::process::exit(2);
        }
    };

    // Structured tracing at the configured level. EnvFilter gets the plain
    // level (parsed, not free-form), so no filter expression can
    // accidentally capture logged secrets.
    //
    // P11.05 — OTel telemetry wiring (Phase 11 wave 1, Agent A): the
    // subscriber composition and the optional OTLP trace provider move
    // into `otel::init_from_env`. With
    // `FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT` unset the behavior is the
    // recorded plain-`fmt` subscriber (unchanged); with it set, worker
    // spans additionally export OTLP to the Collector gateway (P11.06
    // Compose shape). The global W3C propagator is set either way — the
    // publish-side header seam (`flexcore_worker::otel::
    // trace_header_pairs`, wired in `runner.rs`) injects whatever the
    // current context holds and nothing when there is none. WRITTEN-UNRUN:
    // pinned SDK versions are an ASSUMPTION that compiles — verified only
    // at the first human build (records/05_p11_05_otel_python_and_trace_headers.md).
    let otel_config = match flexcore_worker::otel::TelemetryConfig::from_env() {
        Ok(otel_config) => otel_config,
        Err(error) => {
            eprintln!("flexcore-worker: configuration error: otel telemetry ({error})");
            std::process::exit(2);
        }
    };
    let otel_guard =
        flexcore_worker::otel::init_from_env(&otel_config, config.log_level.as_str());
    tracing::info!(?config, "flexcore-worker starting (simulation-only, 0.x)");

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("flexcore-worker: could not start the async runtime: {error}");
            std::process::exit(1);
        }
    };
    let exit_code = runtime.block_on(run(config));
    // P11.05 — flush and shut the OTel provider down explicitly BEFORE
    // the recorded `std::process::exit` (which skips `Drop`, so a
    // drop-only guard would never run). Best-effort batched-export
    // flush; an abrupt exit loses the unflushed batch (recorded
    // limitation, records/05_p11_05_otel_python_and_trace_headers.md).
    otel_guard.shutdown();
    std::process::exit(exit_code);
}

/// The composed worker lifetime. Returns the process exit code.
async fn run(config: WorkerConfig) -> i32 {
    // --- Dependencies (fail fast, named errors, no secret logging) ------

    let store = match FlexCoreStore::connect(&config.database_url) {
        Ok(store) => store,
        Err(error) => {
            tracing::error!(
                error = %error,
                "the configured database could not be reached; aborting startup"
            );
            return 1;
        }
    };
    tracing::info!("postgres pool connected");

    let inbox = match PgInboxStore::new(store.pool().clone()) {
        Ok(inbox) => inbox,
        Err(error) => {
            tracing::error!(error = %error, "the inbox store could not start");
            return 1;
        }
    };
    // FRV1-R5-004 (final review): the inbox table is owned by the
    // adapter's embedded migration `0024_worker_inbox_p1_16` — the P1.19
    // runtime `ensure_table` was removed with the promotion. A database
    // provisioned by the embedded migration set (the recorded image, MIGRATION_POLICY.md)
    // carries the table; a worker pointed at an unmigrated database fails
    // on its first inbox statement (fail-fast, the same posture as every
    // other migrated table).

    let client = match async_nats::connect(&config.nats_url).await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(
                error = %error,
                "the configured NATS server could not be reached; aborting startup"
            );
            return 1;
        }
    };
    let jetstream = async_nats::jetstream::new(client.clone());

    // Idempotent topology provisioning (P1.14 §3/§4/§5; safe on every
    // worker start).
    let limits = deployment_limits();
    if let Err(error) = provision_phase_1_streams(&jetstream, &limits).await {
        tracing::error!(error = %error, "stream provisioning failed; aborting startup");
        return 1;
    }
    let policy = registry_consumer_policy();
    if let Err(error) = provision_durable_pull_consumer(&jetstream, &policy).await {
        tracing::error!(error = %error, "durable consumer provisioning failed; aborting startup");
        return 1;
    }
    tracing::info!(
        durable = %policy.durable_name,
        stream = %policy.stream,
        "jetstream topology provisioned"
    );

    // P11.13 — per-family DLQ streams + bounded retention (Phase 11 wave 1,
    // Agent B): the eleven dedicated per-area DLQ streams provision with
    // their own tighter budget, additive on the Phase 1 catch-all
    // (`FLEXCORE_DLQ` on `flexcore.dlq.v1.unrouted`). Idempotent; safe on
    // every start. Fail-fast like the rest of provisioning.
    if let Err(error) = provision_dlq_streams(&jetstream, &DeploymentLimits::bounded_dlq_defaults())
        .await
    {
        tracing::error!(error = %error, "DLQ family stream provisioning failed; aborting startup");
        return 1;
    }

    // P11.12/P11.13 — the dispatch-command-track topology (Phase 11 wave 1,
    // Agent B): the `FLEXCORE_DISPATCH` stream provisions here (a recorded
    // deviation from the D-J7 "created by the phase that first publishes"
    // rule: the P11.12 consumer requires its stream to exist, and no
    // dispatch publisher exists in 0.x — the consumer idles at zero
    // counters), then the `dispatch-command-track-v1` durable pull
    // consumer. The helper config is built FROM the provisioned policy and
    // the MaxDeliver alignment is checked (P11.13): a mismatch is a named
    // fail-fast error, never silently corrected.
    let dispatch_stream = StreamDefinition {
        name: STREAM_DISPATCH,
        subject_filter: "flexcore.v1.dispatch.>",
        provisioned_in_phase_1: false,
    };
    if let Err(error) = provision_stream(&jetstream, &dispatch_stream, &limits).await {
        tracing::error!(error = %error, "dispatch stream provisioning failed; aborting startup");
        return 1;
    }
    let command_policy = dispatch_consumer_policy();
    if let Err(error) = provision_durable_pull_consumer(&jetstream, &command_policy).await {
        tracing::error!(
            error = %error,
            "dispatch durable consumer provisioning failed; aborting startup"
        );
        return 1;
    }
    let command_consumer_config = ConsumerConfig::from_policy(&command_policy);
    if let Err(mismatch) = command_consumer_config.validate_against_policy(&command_policy) {
        tracing::error!(error = %mismatch, "MaxDeliver alignment check failed; aborting startup");
        return 1;
    }
    tracing::info!(
        durable = %command_policy.durable_name,
        stream = %command_policy.stream,
        "dispatch-command-track topology provisioned (P11.12/P11.13)"
    );

    // --- Background work -------------------------------------------------

    // Shutdown: `true` = drain now; the sender dropping means the same.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // P11.21 — the worker /metrics surface (Phase 11 wave 2, Agent B): the
    // command-consumer metric block is created HERE, before the probe
    // state, and the SAME Arc is shared with the consumer task below — the
    // probe server's new `/metrics` route renders exactly
    // `CommandConsumerMetrics::render()` (the recorded P11.12 handoff
    // seam). WRITTEN-UNRUN; records/52_p11_21_health_checks.md.
    //
    // FRV1-R2-003 (final review): the P2.21 telemetry metric block is
    // created HERE too and the SAME Arc is shared three ways — the probe
    // state (whose `/metrics` route now renders BOTH the command-consumer
    // and the telemetry families), and, when a telemetry source is
    // composed, the ingestion task. When no scenario is configured the Arc
    // is shared with nothing and the telemetry families render at zero on
    // `/metrics` (the always-render discipline; the startup log records
    // that no ingestion task was composed).
    let command_metrics = Arc::new(CommandConsumerMetrics::default());
    let ingestion_metrics = Arc::new(flexcore_worker::ingestion::IngestionMetrics::default());
    let probe_state = ProbeState::new(
        store.pool().clone(),
        client.clone(),
        command_metrics.clone(),
        ingestion_metrics.clone(),
    );
    let probe_addr = config.probe_addr;
    let probe_task = tokio::spawn({
        let state = Arc::clone(&probe_state);
        let shutdown = shutdown_rx.clone();
        async move { serve_probes(probe_addr, state, shutdown).await }
    });

    let outbox: Arc<dyn flexcore_ports::event_outbox::EventOutbox> = Arc::new(store.clone());
    let publisher_task = tokio::spawn(run_outbox_publisher(
        jetstream.clone(),
        outbox,
        publisher_config(config.outbox_poll_interval),
        shutdown_rx.clone(),
    ));

    let side_effects = Arc::new(AtomicUsize::new(0));
    let mut consumer_tasks = Vec::new();
    for _ in 0..config.consumer_concurrency {
        consumer_tasks.push(tokio::spawn(run_idempotent_consumer(
            jetstream.clone(),
            Arc::new(inbox.clone()) as Arc<dyn flexcore_nats_adapter::inbox::InboxStore>,
            // P11.13: the helper config is built from the provisioned
            // policy (alignment by construction; the field-by-field shape
            // could drift).
            ConsumerConfig::from_policy(&policy),
            Arc::clone(&side_effects),
            shutdown_rx.clone(),
        )));
    }

    // P11.12 — the command consumer task (Phase 11 wave 1, Agent B): the
    // `dispatch-command-track-v1` loop with the frozen state machine as
    // the dedupe oracle (invalid transition on a redelivered command =
    // no-op + duplicate-delivery metric). The lifecycle-state gateway is
    // the real Postgres store; the metrics counter block (`command_metrics`,
    // created above for the P11.21 probe-server `/metrics` route) is the
    // worker-side surface — the P11.12 record's handoff note is closed by
    // the wave-2 `/metrics` route on the probe server.
    let command_gateway: Arc<dyn CommandStateGateway> =
        Arc::new(PgCommandStateGateway::new(store.clone()));
    let command_consumer_task = tokio::spawn(run_command_consumer(
        jetstream.clone(),
        Arc::new(inbox.clone()) as Arc<dyn flexcore_nats_adapter::inbox::InboxStore>,
        command_gateway,
        command_metrics,
        command_consumer_config,
        shutdown_rx.clone(),
    ));
    tracing::info!(
        consumers = config.consumer_concurrency,
        "background loops started (registry consumers + P11.12 command consumer)"
    );

    // The telemetry ingestion task (P2.10): the P2.07 pipeline as a worker
    // task, consuming a `TelemetrySource` port. The simulator source (P2.14)
    // is composed when the scenario configuration is present — see
    // [`compose_ingestion`]. The composition resolves the tenant and the
    // registered assets through the store's sync repository ports, which
    // block on the adapter's private runtime, so it runs under
    // `block_in_place` (the recorded sync-port hosting rule; the runtime is
    // multi-thread).
    let ingestion_composition =
        match tokio::task::block_in_place(|| compose_ingestion(&config, &store)) {
            Ok(composition) => composition,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "the telemetry ingestion composition failed; aborting startup"
                );
                return 1;
            }
        };
    let mut ingestion_handle: Option<
        tokio::task::JoinHandle<flexcore_worker::ingestion::IngestionRunReport>,
    > = None;
    let mut bridge_handle: Option<tokio::task::JoinHandle<()>> = None;
    match ingestion_composition {
        Some((source, query)) => {
            // FRV1-R2-003: the SAME `ingestion_metrics` Arc the probe
            // state renders on `/metrics` (created above) drives the task.
            let metrics = Arc::clone(&ingestion_metrics);
            let runner = flexcore_worker::ingestion::IngestionTask::new(
                Arc::clone(&source) as Arc<dyn flexcore_ports::telemetry_source::TelemetrySource>,
                store.clone(),
                flexcore_worker::ingestion::PgIngestionInbox::new(inbox.clone()),
                Arc::new(flexcore_worker::ingestion::JsonEventPayloadEncoder),
                Arc::new(flexcore_postgres_adapter::SystemClock),
                Arc::new(flexcore_worker::ingestion::UuidV7Generator::new(store.pool().clone())),
                flexcore_worker::ingestion::IngestionConfig {
                    channel_capacity: config.ingest_channel_capacity,
                    trusted_source_id: config.ingest_trusted_source_id.clone(),
                    stale_freshness_limit_millis: ingest_stale_freshness_limit_ms(),
                    ..Default::default()
                },
                metrics,
            );
            // The source bridge (P2.14) holds `runner.queue()` and offers
            // each pulled frame; the offer side sheds at the bounded-channel
            // limit (retryable error + shed metric). The pipeline's own
            // subscription happens inside `IngestionTask::run`.
            //
            // FRV1-R2-004 (final review; RECORDED SEAM): the EV ingest
            // bridge (`flexcore_worker::ev_ingest::route_ev_inbound`) is
            // composed with NOTHING here — its frames come from the OCPP
            // CSMS adapter's inbound path, which constructs no
            // `EvInboundFrame` yet and which no binary hosts. The queue
            // handle below is the SAME `TelemetryOffer` seam the bridge
            // will receive when that integration lands; until then the
            // bridge stays the honestly recorded seam the finding
            // documents (see the `ev_ingest` module header for the named
            // follow-up). Nothing here fakes a caller.
            let queue = runner.queue();
            ingestion_handle = Some(tokio::spawn(async move {
                let report = runner.run(query.clone(), shutdown_rx.clone()).await;
                tracing::info!(?report, "telemetry ingestion task ended");
                report
            }));
            bridge_handle = Some(tokio::spawn(run_source_bridge(
                source,
                query,
                queue,
                shutdown_rx.clone(),
            )));
            tracing::info!(
                scenario = %config.simulator_scenario.as_deref().unwrap_or_default(),
                "telemetry ingestion task started (P2.10; simulator source composed)"
            );
        }
        None => {
            tracing::info!(
                "telemetry ingestion task not started: no simulator scenario is configured \
                 (set FLEXCORE_SIMULATOR_SCENARIO to compose the simulator source); the \
                 ingestion pipeline itself is source-agnostic (flexcore_worker::ingestion)"
            );
        }
    }

    // The partition-maintenance task (P2.20, D-R1): scheduled monthly
    // pre-creation ahead of the ingest frontier + the approved-operation
    // retention path. Started ONLY when the maintenance database URL is
    // configured — the passes need DDL, which the runtime role must not
    // hold (D-P6); no maintenance without the privileged path (fail-safe).
    let mut maintenance_handle: Option<tokio::task::JoinHandle<()>> = None;
    if let Some(maintenance_url) = config.maintenance_database_url.clone() {
        match FlexCoreStore::connect(&maintenance_url) {
            Ok(maintenance_store) => {
                let settings = flexcore_worker::maintenance::MaintenanceSettings::from_config(
                    config.maintenance_months_ahead,
                    config.retention_normalized_telemetry_months,
                    config.retention_approved,
                );
                maintenance_handle = Some(tokio::spawn(
                    flexcore_worker::maintenance::run_maintenance_loop(
                        maintenance_store,
                        settings,
                        config.maintenance_interval,
                        shutdown_rx.clone(),
                    ),
                ));
                tracing::info!(
                    months_ahead = config.maintenance_months_ahead,
                    "partition-maintenance task started (P2.20)"
                );
            }
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "the maintenance database could not be reached; the partition-maintenance \
                     task is NOT started (pre-creation and retention are inactive — configure \
                     the maintenance URL to enable them)"
                );
            }
        }
    } else {
        tracing::info!(
            "partition-maintenance task not started: no maintenance database URL is configured \
             (the passes need the migrator/maintenance role; D-R1)"
        );
    }

    // --- Wait for the signal ---------------------------------------------

    wait_for_signal().await;
    tracing::info!("shutdown signal received; beginning graceful drain");
    probe_state.begin_draining();
    let _ = shutdown_tx.send(true);

    // --- Bounded drain ----------------------------------------------------

    let mut aborted = false;
    for task in consumer_tasks {
        if finish_or_abort(task, config.shutdown_timeout).await.is_err() {
            aborted = true;
        }
    }
    // P11.12 — the command consumer drains in the same bounded window
    // (an aborted delivery stays unacknowledged and JetStream redelivers;
    // the state-machine oracle deduplicates the redelivery).
    if finish_or_abort(command_consumer_task, config.shutdown_timeout)
        .await
        .is_err()
    {
        aborted = true;
    }
    if finish_or_abort(publisher_task, config.shutdown_timeout)
        .await
        .is_err()
    {
        aborted = true;
    }
    // The source bridge stops pulling at the shutdown flag first (its queue
    // handle drops, which tells the ingestion loop the receive side is done).
    if let Some(bridge) = bridge_handle {
        if finish_or_abort(bridge, config.shutdown_timeout).await.is_err() {
            aborted = true;
        }
    }
    // The ingestion task drains in the same bounded window: stop accepting
    // (its queue sheds), process the buffered frames, exit.
    if let Some(ingestion) = ingestion_handle {
        if finish_or_abort(ingestion, config.shutdown_timeout).await.is_err() {
            aborted = true;
        }
    }
    // The maintenance task stops at the next tick boundary (its passes are
    // idempotent; an aborted pass retries on the next start).
    if let Some(maintenance) = maintenance_handle {
        if finish_or_abort(maintenance, config.shutdown_timeout).await.is_err() {
            aborted = true;
        }
    }

    // --- Close connections ------------------------------------------------

    drop(client); // closes the NATS connection
    drop(jetstream);
    store.pool().close();
    drop(store);
    drop(inbox);

    // The probe server is the last thing to go: it drains with the
    // process, then we stop waiting on it.
    let _ = finish_or_abort(probe_task, config.shutdown_timeout).await;

    if aborted {
        tracing::warn!(
            "graceful drain hit the bounded timeout; in-flight work was cancelled \
             safely (pending outbox entries and unacknowledged messages are retried)"
        );
        return 1;
    }
    tracing::info!("graceful shutdown complete");
    0
}

/// The P2.21 staleness-signal freshness limit (milliseconds), read directly
/// from the environment: `FLEXCORE_STALE_FRESHNESS_LIMIT_MS`. The staleness
/// threshold is DEPLOYMENT CONFIGURATION — never a hard-coded constant
/// (P0.16 rule; P2.08: per-use-case limits). Unset = the pipeline performs
/// no staleness evaluation. Fail-fast on an unusable value (the config
/// convention: named error, never the value).
fn ingest_stale_freshness_limit_ms() -> Option<i64> {
    let raw = match std::env::var("FLEXCORE_STALE_FRESHNESS_LIMIT_MS") {
        Ok(raw) => raw,
        Err(_) => return None,
    };
    match raw.trim().parse::<i64>() {
        Ok(millis) if millis > 0 => Some(millis),
        _ => {
            eprintln!(
                "flexcore-worker: configuration error: \
                 FLEXCORE_STALE_FRESHNESS_LIMIT_MS must be a positive number of milliseconds"
            );
            std::process::exit(2);
        }
    }
}

/// One composed ingestion dependency pair: the simulator source as the
/// concrete port object (the bridge needs its pull path) and the
/// tenant-scoped subscription query.
type IngestionComposition = (Arc<SimulatorTelemetrySource>, TelemetryQuery);

/// Composes the telemetry ingestion task's dependencies (P2.10 step 5; the
/// P2.25 runbook step-0 wiring): the simulator `TelemetrySource` port
/// object and its subscription query. Closes the recorded blocker in
/// `ledger/phase-02-asset-telemetry/25_exit_gate_runbook.md` §0.
///
/// Configuration (`FLEXCORE_SIMULATOR_SCENARIO`): when the scenario path is
/// set, the composition
///
/// 1. loads the scenario all-or-nothing (P2.13), honouring the optional
///    seed override (`FLEXCORE_SIMULATOR_SEED`; default = the scenario's
///    recorded seed) and the optional horizon-ticks override
///    (`FLEXCORE_SIMULATOR_HORIZON_TICKS`; default = the recorded horizon);
/// 2. resolves the tenant and the registered assets from the store's
///    repository ports — the scenario speaks stable asset keys, and the
///    key → registry `AssetId` mapping exists ONLY at registration
///    (P2.25 step 2), so every scenario asset key is matched against the
///    tenant's registered asset names and a missing key fails composition
///    (never a silently reduced portfolio);
/// 3. builds the `TelemetryQuery` over the registered tenant context, the
///    mapped asset IDs, the configured horizon window, and the full frozen
///    quantity vocabulary (the exit-gate run covers every registered
///    asset's full quantity set, P2.25 §3).
///
/// When the scenario configuration is absent, the composition returns
/// `Ok(None)` and the worker runs without the ingestion task — recorded
/// honestly in the startup log, never silently started. Every failure is a
/// named error; the worker aborts startup (fail-fast).
///
/// The port-only rule holds: this function (the composition root) is the
/// only place the simulator crate is named in library-visible code; the
/// ingestion pipeline ([`flexcore_worker::ingestion::IngestionTask`])
/// compiles against the port only. Decision note:
/// `ledger/phase-02-asset-telemetry/24_worker_source_wiring.md`.
///
/// Phase 2 source authentication is a recorded STUB — see
/// `flexcore_worker::ingestion::SOURCE_AUTH_STATUS`; this wiring does not
/// change what is asserted.
fn compose_ingestion(config: &WorkerConfig, store: &FlexCoreStore) -> Result<Option<IngestionComposition>, String> {
    let Some(scenario_path) = config.simulator_scenario.as_deref() else {
        return Ok(None);
    };
    let Some(tenant_reference) = config.simulator_tenant_reference.as_deref() else {
        return Err(
            "FLEXCORE_SIMULATOR_SCENARIO is set but FLEXCORE_SIMULATOR_TENANT_REFERENCE is \
             not: the ingestion query is tenant-scoped and needs the registered tenant's \
             identity-provider reference (P2.25 step 2)"
                .to_string(),
        );
    };

    // 1. The scenario (all-or-nothing; P2.13).
    let scenario_text = std::fs::read_to_string(scenario_path).map_err(|error| {
        format!("could not read the configured scenario file '{scenario_path}': {error}")
    })?;
    let scenario_text = apply_seed_override(&scenario_text, config.simulator_seed)?;
    let scenario = load_scenario(&scenario_text).map_err(|error| {
        format!("the configured scenario file '{scenario_path}' was rejected: {error}")
    })?;

    // The query window: the scenario start over the configured horizon
    // (default: the scenario's recorded tick count). Overflow is a
    // configuration error, never a silently truncated window.
    let ticks = config.simulator_horizon_ticks.unwrap_or(scenario.total_ticks);
    let cadence_millis = scenario.cadence_seconds.checked_mul(1_000).ok_or_else(|| {
        format!(
            "the scenario cadence {} seconds is not usable in milliseconds",
            scenario.cadence_seconds
        )
    })?;
    let horizon_millis = i64::try_from(ticks)
        .ok()
        .and_then(|ticks| ticks.checked_mul(cadence_millis))
        .ok_or_else(|| format!("the configured horizon of {ticks} ticks overflows the scenario clock"))?;
    let until_millis = scenario.start_ms.checked_add(horizon_millis).ok_or_else(|| {
        format!("the configured horizon of {ticks} ticks overflows the scenario clock")
    })?;
    let from = EventTime::from_unix_millis(scenario.start_ms)
        .map_err(|error| format!("the scenario start is not a canonical event time: {error}"))?;
    let until = EventTime::from_unix_millis(until_millis).map_err(|error| {
        format!("the configured horizon end is not a canonical event time: {error}")
    })?;

    // 2. Tenant and asset resolution from the registry (the P2.25 step 2
    //    registration record). Read-only unit of work: dropped without
    //    commit, which rolls the native transaction back (the adapter
    //    contract, P1.10 rule 4).
    let unit = store.begin(None).map_err(|error| {
        format!("the registry unit of work for the ingestion composition failed: {error}")
    })?;
    let tenant = TenantRepository::find_by_identity_provider_reference(&unit, tenant_reference)
        .map_err(|error| {
            format!(
                "the configured tenant reference '{tenant_reference}' could not be resolved \
                 from the registry: {error}"
            )
        })?
        .ok_or_else(|| {
            format!(
                "no tenant is registered with identity-provider reference \
                 '{tenant_reference}'; register the simulator portfolio first (P2.25 step 2)"
            )
        })?;
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant.tenant_id(),
            subject: "worker:simulator-ingestion".to_string(),
        },
    ));
    let registered = AssetRepository::list_for_tenant(&unit, &ctx).map_err(|error| {
        format!("the tenant's registered assets could not be listed: {error}")
    })?;
    drop(unit);

    // The scenario speaks asset keys; the registry speaks IDs, and the
    // registration recorded the key as the asset's name (P2.25 §2.4). Every
    // scenario key must resolve.
    let mut mapping: Vec<(String, AssetId)> = Vec::new();
    for site in &scenario.sites {
        for asset in &site.assets {
            let registered_asset = registered
                .iter()
                .find(|candidate| candidate.name() == asset.asset_key)
                .ok_or_else(|| {
                    format!(
                        "scenario asset '{}' has no registered asset of that name under tenant \
                         reference '{tenant_reference}' (the key → ID mapping exists only at \
                         registration, P2.25 step 2)",
                        asset.asset_key
                    )
                })?;
            mapping.push((asset.asset_key.clone(), registered_asset.asset_id()));
        }
    }

    // 3. The port object (P2.14) and the subscription query.
    let query = TelemetryQuery::new(
        &ctx,
        mapping.iter().map(|(_, id)| *id).collect(),
        from,
        until,
        vec![
            TelemetryQuantity::ActivePower,
            TelemetryQuantity::ReactivePower,
            TelemetryQuantity::Soc,
            TelemetryQuantity::Energy,
            TelemetryQuantity::Availability,
            TelemetryQuantity::OperatingMode,
            TelemetryQuantity::Health,
            TelemetryQuantity::ConnectionState,
            TelemetryQuantity::BaselineReference,
        ],
    )
    .map_err(|error| format!("the ingestion subscription query is invalid: {error}"))?;
    let source = SimulatorTelemetrySource::from_scenario(scenario, mapping).map_err(|error| {
        format!(
            "the simulator telemetry source could not be built from '{scenario_path}': {error}"
        )
    })?;
    Ok(Some((Arc::new(source), query)))
}

/// Returns the scenario JSON with the master seed replaced by `seed` when an
/// override is configured; otherwise the text unchanged. The seed lives in
/// the versioned scenario artefact (D-SC4) — overriding it is a
/// replay-relevant change (P2.24), so it is an explicit operator action,
/// never a default.
fn apply_seed_override(scenario_text: &str, seed: Option<u64>) -> Result<String, String> {
    let Some(seed) = seed else {
        return Ok(scenario_text.to_string());
    };
    let mut value: serde_json::Value = serde_json::from_str(scenario_text)
        .map_err(|error| format!("the configured scenario is not valid JSON: {error}"))?;
    let seeds = value
        .get_mut("seeds")
        .and_then(|seeds| seeds.as_object_mut())
        .ok_or_else(|| "the scenario has no seeds section to override".to_string())?;
    seeds.insert("master_seed".to_string(), serde_json::Value::Number(seed.into()));
    serde_json::to_string(&value)
        .map_err(|error| format!("the seed-overridden scenario could not be re-rendered: {error}"))
}

/// The source bridge (P2.14/P2.10): the pull-side adapter between the
/// composed simulator source and the ingestion pipeline's bounded channel.
/// The port is sync and pull-based — nothing is generated unless pulled,
/// and each pull yields at most one frame (one asset, one tick) — so the
/// bridge drives the emission: pull one frame, offer it, repeat, until the
/// horizon is exhausted, the shutdown flag fires, or the source fails.
///
/// A shed offer is retryable by contract (P2.07 §5): the pull has already
/// consumed the frame bytes from the deterministic stream, so the SAME
/// bytes are re-offered with a short backoff until the bounded channel
/// accepts them — never a silent drop, never unbounded buffering. If the
/// pipeline's channel is closed (the ingestion task ended), the bridge
/// stops and reports; re-offering into a closed channel would be a silent
/// loss.
async fn run_source_bridge(
    source: Arc<SimulatorTelemetrySource>,
    query: TelemetryQuery,
    queue: flexcore_worker::ingestion::IngestionQueue,
    shutdown: watch::Receiver<bool>,
) {
    let subscription = match TelemetrySource::subscribe(source.as_ref(), &query) {
        Ok(subscription) => subscription,
        Err(error) => {
            tracing::warn!(error = %error, "the simulator source rejected the bridge subscription");
            return;
        }
    };
    source.start();
    let mut frames_pulled: u64 = 0;
    loop {
        if *shutdown.borrow() {
            tracing::info!(frames_pulled, "the source bridge stops at the shutdown flag");
            return;
        }
        let frame = match source.pull_frame(&subscription) {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                tracing::info!(
                    frames_pulled,
                    "the simulator source is exhausted (horizon reached or not running); \
                     the source bridge stops"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    frames_pulled,
                    "the simulator source failed; the source bridge stops"
                );
                return;
            }
        };
        let offered = loop {
            if *shutdown.borrow() {
                break false;
            }
            if queue.is_closed() {
                tracing::warn!(
                    frames_pulled,
                    "the ingestion channel is closed (the ingestion task ended); \
                     the source bridge stops"
                );
                break false;
            }
            match queue.offer(frame.clone()) {
                Ok(()) => break true,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        };
        if !offered {
            tracing::info!(
                frames_pulled,
                "the source bridge stopped before the horizon was complete"
            );
            return;
        }
        frames_pulled += 1;
    }
}

/// Waits for SIGTERM or SIGINT.
async fn wait_for_signal() {
    let sigterm = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => {
                    tracing::error!(error = %error, "SIGTERM handler unavailable");
                    std::future::pending::<()>().await;
                }
            }
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
    };
    let sigint = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(error = %error, "SIGINT handler unavailable");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        () = sigterm => {}
        () = sigint => {}
    }
}

/// Awaits a task for the bounded timeout: `Ok(())` = finished or already
/// gone; `Err(())` = timed out and was aborted (safe by design — see the
/// module-header shutdown sequence).
async fn finish_or_abort<T>(
    task: tokio::task::JoinHandle<T>,
    timeout: Duration,
) -> Result<(), ()> {
    match tokio::time::timeout(timeout, task).await {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}
