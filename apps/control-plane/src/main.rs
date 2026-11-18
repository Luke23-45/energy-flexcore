// P1.17 — The control-plane composition root.
//
// THIS is the only place that wires real adapters to ports (§6.2): the
// Postgres store (P1.13), the broker client (P1.15), the authz policy set,
// the identity resolver (dev-auth feature only — see auth.rs for the
// recorded authentication status), and the Axum router. Nothing else wires
// them.
//
// P1.21 — config from environment, fail-fast, secrets never logged.
// P1.22 — health/readiness probes and bounded graceful shutdown.
// P1.20 — JSON request-correlation logs and metrics middleware (metrics.rs).
//
// Recorded adapter constraint: FlexCoreStore is SYNCHRONOUS and owns a
// private tokio runtime — connect, migrations, and every port call must run
// from a blocking context (`tokio::task::spawn_blocking`), never inside the
// async executor. Per-request unit-of-work execution lives in
// state.rs::run_in_uow.
use std::future::{Future, IntoFuture};
use std::sync::Arc;
use std::time::Duration;

use flexcore_control_plane::auth::{DenyAllResolver, IdentityResolver, phase5_default_policies};
use flexcore_control_plane::config::Config;
use flexcore_control_plane::metrics::track_metrics;
use flexcore_control_plane::probes::{HealthCheck, Readiness};
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::state::{AppState, Backend};

#[cfg(feature = "dev-auth")]
use flexcore_control_plane::auth::DevAuthResolver;

use flexcore_ports::clock::Clock;
use flexcore_ports::id_generator::IdGenerator;

/// Timeout of the one broker health request per readiness evaluation.
const BROKER_HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// The readiness health-request subject: deliberately OUTSIDE both the
/// event namespace (`flexcore.v1.*`, P1.14 D-J1) and the DLQ namespace
/// (`flexcore.dlq.v1.*`) so the request is never captured by a stream.
const BROKER_HEALTH_SUBJECT: &str = "_flexcore.healthcheck.v1.ping";

#[tokio::main]
async fn main() {
    // Fail fast on configuration (P1.21): a missing required value is a
    // named error, never a panic with a secret in the message.
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuration error: {e}");
            std::process::exit(2);
        }
    };

    // JSON logs for every line, including the per-request correlation line
    // (P1.20). The level comes from FLEXCORE_LOG_LEVEL.
    //
    // P11.04 — OTel tracing wiring (Phase 11 wave 1, Agent A): the
    // subscriber composition and the OTLP trace/log providers move into
    // `otel_telemetry::init_from_env`. With
    // `FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT` unset the behavior is the
    // plain P1.20 JSON subscriber (unchanged); with it set, spans and log
    // records also export OTLP to the Collector gateway (P11.06 Compose
    // shape). WRITTEN-UNRUN: pinned SDK versions are an ASSUMPTION that
    // compiles — verified only at the first human build (records/
    // 04_p11_04_otel_tracing_rust.md).
    let otel_config = match flexcore_control_plane::otel_telemetry::TelemetryConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("configuration error: otel telemetry ({e})");
            std::process::exit(2);
        }
    };
    let otel_guard = flexcore_control_plane::otel_telemetry::init_from_env(
        &otel_config,
        &cfg.log_level,
    );

    // Postgres store: the adapter is SYNCHRONOUS (it owns a private runtime),
    // so connect + migrations run on a blocking thread. The URL is never
    // logged (Config's Debug impl redacts it).
    let database_url = cfg.database_url.clone();
    let store = tokio::task::spawn_blocking(move || {
        flexcore_postgres_adapter::FlexCoreStore::connect(&database_url)
    })
    .await
    .unwrap_or_else(|e| {
        eprintln!("startup error: database connect task failed ({e})");
        std::process::exit(2);
    })
    .unwrap_or_else(|e| {
        eprintln!("startup error: database unavailable ({e})");
        std::process::exit(2);
    });
    let store = Arc::new(store);
    let migrating = store.clone();
    tokio::task::spawn_blocking(move || migrating.run_migrations())
        .await
        .unwrap_or_else(|e| {
            eprintln!("startup error: migration task failed ({e})");
            std::process::exit(2);
        })
        .unwrap_or_else(|e| {
            eprintln!("startup error: migrations failed ({e})");
            std::process::exit(2);
        });

    // Broker (P1.15): connection only. The adapter crate hosts the outbox
    // publisher (apps/worker, P1.19); the control plane needs a live client
    // for the readiness check and no more.
    let nats = async_nats::connect(&cfg.nats_url)
        .await
        .unwrap_or_else(|e| {
            eprintln!("startup error: broker unavailable ({e})");
            std::process::exit(2);
        });

    // Non-transaction-bound ports: the pool-backed UUIDv7 generator (D-S1)
    // and the wall clock. Both come from the adapter crate and share the
    // store's pool/runtime.
    let ids: Arc<dyn IdGenerator> = Arc::new(store.id_generator());
    let clock: Arc<dyn Clock> = Arc::new(flexcore_postgres_adapter::SystemClock);

    // Readiness checks (P1.22).
    let readiness = Readiness::new(vec![
        ("database".to_string(), database_check(store.clone())),
        ("broker".to_string(), broker_check(nats.clone())),
    ]);

    // Identity resolver: dev-auth feature only (recorded Phase 1 status).
    let identity: Arc<dyn IdentityResolver> = build_identity_resolver();

    // Composition: backend -> state (use cases are built per unit of work,
    // see state.rs — the repository ports are transaction-bound). P3.28:
    // the policy set is the cumulative phase set (Phase 3 grants added).
    // P4.30: the Phase 4 read-surface grants are added (command.read,
    // feasibility.read). P5.31 (FRV1-R2-001): the Phase 5 cumulative set is
    // composed — it is the Phase 4 set extended with `alarm.read`, so
    // `/api/v1/alarms` is reachable in production instead of a guaranteed
    // 403. Still deny-by-default: the union of grants is exactly the
    // recorded phase sets, nothing wider.
    //
    // FRV1-R2-009: `flexcore_authz::phase13_policies` (SiteApprover
    // command.approve + apikey.*), `flexcore_authz::phase16_market_policies`
    // (market.*) and `flexcore_authz::phase4_command_policies` (command
    // gate actions) are deliberately NOT composed here — no REST surface in
    // this binary evaluates them (the command gates run via sim-runner;
    // apikey.*/market.* have no routes), so each stays recorded as a
    // design-level grant set.
    //
    // FRV1-R1-004 (final review) — the P4.11–P4.18 COMMAND CORE COMPOSITION
    // STATUS, recorded here because this file is the composition root:
    // `CommandGates`, `CommandExpiryService`, the dispatch adapter
    // (`SimulatorDispatchAdapter`), `ApprovalWorkflowEngine`, and
    // `FallbackEngine` are NOT constructed in this binary in 0.x, and v0.x
    // exposes none of them. The recorded reasons, verified by inspection
    // for this finding:
    // 1. Constructor contract: every one of the five is a per-composition
    //    struct borrowing its collaborators (`&'a dyn` seams) — there is no
    //    long-lived instance to register in `AppState`, whose Postgres ports
    //    are themselves transaction-bound per request (state.rs).
    // 2. Unimplemented ports (grep-verified): the injected seams have NO
    //    production implementation — `CommandGateStore` (which the module
    //    contract says the composition root implements over the P4.10
    //    `CommandRecordStore`), `ScheduleEnvelopeSource`,
    //    `ModelVersionSource`, `AssetCapabilitySource`,
    //    `CommandFeasibilityRunner` (the §12.5 grid pipeline — the grid
    //    crate is not even a dependency of this binary),
    //    `CommandAuthorizationEvaluator`, `FallbackAuditSink`,
    //    `ApprovalAccumulationStore`, and `SimulatorDispatchTransport` have
    //    only sim-runner in-memory implementations or test fakes.
    // 3. No call site: the §15.2 dispatch-command surface here is
    //    READ-ONLY by recorded decision (routes_phase4.rs — no
    //    execute-command endpoint), so no route handler touches any port
    //    the core consumes; constructing the core would be unused code.
    // In 0.x the protected command path is exercised exclusively by
    // `apps/sim-runner` (its scenario composition implements the seams
    // in-memory and drives the gates against the simulator). The recorded
    // composition-root plan for a later release: implement
    // `CommandGateStore` over the P4.10 `CommandRecordStore` (the module's
    // stated contract) plus the remaining five seams, and construct the
    // core per unit of work at the seam that invokes it — exactly the
    // sim-runner composition shape, over real adapters.
    let state = AppState::new(
        Backend::Postgres {
            store: store.clone(),
            ids,
            clock,
        },
        phase5_default_policies(),
        identity,
        readiness,
        // Telemetry read port (P2.12): the Postgres adapter's
        // `PgTelemetryReader`, pool-held on the store — each call opens
        // its own tenant-scoped unit of work (see
        // `flexcore_postgres_adapter::telemetry_reader`).
        Some(Arc::new(store.telemetry_reader())),
        // The current-state freshness limit (P2.18 step 3): configuration,
        // not a hard-coded constant.
        cfg.current_state_freshness.as_millis() as i64,
    )
    // P6.06 (FRV1-R2-002): the Postgres `EdgeEnrollmentStore` on the same
    // store/pool is the real persistence for `/api/v1/edge/status` and
    // `/api/v1/edge/decommission` — without it the routes fail closed
    // unconditionally. The provisioning-token validator stays the deny-all
    // default (recorded 0.x posture: no token service), so `/edge/enroll`
    // keeps failing closed as recorded.
    .with_edge_enrollment(store.clone())
    // FRV1-R8-005 (final review): the P13.09 webhook-delivery bookkeeping
    // store composes here, over the Postgres adapter's store
    // (`webhook_delivery_pg.rs` implements the port; migration 0020 is
    // the schema). COMPOSITION IS NOT DELIVERY: in 0.x there is no
    // enqueue route, no hosting scan loop, and no secret-resolver
    // composition at the P11.16 seam — the HTTP transport for any future
    // hosting loop is `UnwiredOutboundHttpPost`
    // (`webhook_service.rs`), a fail-closed seam that REFUSES every
    // transmission with the named `E_TRANSPORT_UNWIRED` error because the
    // workspace has no HTTP-client dependency (an unwired seam stays
    // recorded, never faked). The remaining recorded composition steps
    // (hosting-loop cadence, resolver composition, the P13.18 admin
    // surface) are the P13.09 decision record §5's open items.
    .with_webhook_delivery(Arc::new(
        flexcore_postgres_adapter::webhook_delivery::PgWebhookDeliveryStore::new(
            (*store).clone(),
        ),
    ));

    // FRV1-R5-002 (final review) — the P15.15/P16.01 MARKET AND
    // INTERCONNECTION RECORD STORES COMPOSITION STATUS, recorded here
    // because this file is the composition root (the FRV1-R1-004
    // precedent): `MarketRecordsStore` and `InterconnectionRecordStore`
    // are fully implemented on the Postgres adapter (migrations
    // 0021/0022, RLS + grants + indexes) and page-bounded (FRV1-R5-009),
    // but NOTHING constructs them in this binary in 0.x — no route, no
    // use case, no worker job reads or writes either trait. This is the
    // recorded DESIGN-LEVEL-ONLY posture, not a defect: no market is
    // connected and no interconnection record exists anywhere (ADR-011;
    // migration 0022's header records the same), so the tables and code
    // are unwired by design and exercised only by the adapter's
    // WRITTEN-UNRUN test surface. The recorded wiring step (the finding's
    // option (a)) is: market-record ingestion becomes a composition-root
    // action when a market is actually connected, and the
    // interconnection ingest becomes an operator-only control-plane route
    // in the release that first files registrations — until then neither
    // store is constructed here, and no release note may imply either is
    // reachable through this binary.

    let app = stage1_router(state.clone()).layer(
        tower::ServiceBuilder::new()
            // Outermost: metrics + correlation log see every response,
            // including timeout- and limit-generated ones.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                track_metrics,
            ))
            // P11.14 — per-tenant token-bucket rate limiting + in-flight
            // command-submission caps (Phase 11 wave 1, Agent B): 429 +
            // `Retry-After` contract; probes and `/metrics` are never
            // rate limited. Outer to the idempotency middleware so a 429
            // is never cached as a stored response. WRITTEN-UNRUN;
            // records/26_p11_14_rate_limiting.md.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                flexcore_control_plane::rate_limit::rate_limit_middleware,
            ))
            // P11.11 — Idempotency-Key enforcement on command-submission
            // POSTs (Phase 11 wave 1, Agent B): key required (422),
            // stored-response replay, 409 replay conflict; key
            // representation reuses the P1.09 envelope key type.
            // WRITTEN-UNRUN; records/23_p11_11_command_post_idempotency.md.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                flexcore_control_plane::idempotency::idempotency_middleware,
            ))
            .layer(tower_http::timeout::TimeoutLayer::new(Duration::from_secs(15)))
            .layer(tower_http::limit::RequestBodyLimitLayer::new(1024 * 1024)),
    );

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("startup error: cannot bind {}: {e}", cfg.bind_addr);
            std::process::exit(2);
        });
    tracing::info!(bind = %cfg.bind_addr, "control plane listening");

    // Graceful shutdown (P1.22): on SIGTERM/SIGINT readiness goes false
    // FIRST (load balancers stop routing), the drain window is bounded by
    // FLEXCORE_DRAIN_TIMEOUT_SECS, then the process exits 0.
    let (signal_tx, signal_rx) = tokio::sync::watch::channel(false);
    let readiness_on_signal = state.readiness.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("shutdown signal received; readiness now false");
        readiness_on_signal.begin_shutdown();
        let _ = signal_tx.send(true);
    });

    // P5.25 — live updates (§15.1, §21.3): an EPHEMERAL core-NATS
    // subscription on the same event subjects the durable consumers
    // process (`flexcore.v1.>`, P1.14 §3) forwards every message into the
    // in-process `LiveEventBus`; the `GET /api/v1/live` SSE handlers
    // subscribe to the bus and filter per caller tenant from the decoded
    // payload (D-J2). No JetStream consumer, no inbox record, no side
    // effect: the durable-consumer rule (P1.14 §5.1) is reserved for
    // stateful work, and this stream is a stateless view.
    tokio::spawn(flexcore_control_plane::routes_live::run_live_forwarder(
        nats.clone(),
        state.live.clone(),
        signal_rx.clone(),
    ));

    // P11.20 — stuck-state scanner (Phase 11 wave 2, Agent B): a periodic
    // scan of every tenant's commands parked in a NON-terminal frozen
    // lifecycle state beyond the registered threshold
    // (`COMMANDS_STUCK_THRESHOLD_MILLIS` — X-8 pre-registered
    // RECOMMENDATION, pending operator tuning) drives the
    // `flexcore_commands_stuck` gauge (NO labels — exactly the P11.01
    // taxonomy name, record 01 §2 family 5; the P11.03 alert rule and the
    // P11.09 dashboard panel bind that name verbatim and needed zero
    // edits). The reader's port calls are synchronous adapter calls, so
    // the scan runs under `block_in_place` on this multi-thread runtime
    // (the recorded sync-port hosting rule); on a scan error the gauge
    // keeps its previous value (never silently zeroed). WRITTEN-UNRUN;
    // records/51_p11_20_stuck_state.md.
    {
        let stuck_reader =
            flexcore_control_plane::metrics::PgCommandStateReader::new(store.clone());
        tokio::spawn(flexcore_control_plane::metrics::run_stuck_state_scanner(
            Arc::new(stuck_reader),
            state.metrics.clone(),
            flexcore_control_plane::metrics::STUCK_STATE_SCAN_INTERVAL,
            signal_rx.clone(),
        ));
    }

    let drain = cfg.drain_timeout;
    let drain_rx = signal_rx.clone();
    // `WithGracefulShutdown` implements `IntoFuture`, not `Future`, so
    // the server future is materialized explicitly for the `select!` below.
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let mut rx = signal_rx;
            while !*rx.borrow_and_update() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => {
            result.expect("server error");
        }
        _ = async move {
            // Signal -> bounded drain window; expiry forces exit.
            let mut rx = drain_rx;
            while !*rx.borrow_and_update() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
            tokio::time::sleep(drain).await;
        } => {
            tracing::warn!(drain_secs = drain.as_secs(), "drain window expired; exiting");
        }
    }
    tracing::info!("control plane stopped");
    // P11.04 — flush and shut the OTel providers down explicitly BEFORE
    // the recorded `std::process::exit(0)` (which skips `Drop`, so a
    // drop-only guard would never run). Best-effort batched-export flush;
    // an abrupt exit simply loses the unflushed batch (recorded
    // limitation, records/04_p11_04_otel_tracing_rust.md).
    otel_guard.shutdown();
    std::process::exit(0);
}

/// Waits for SIGINT (Ctrl-C) or SIGTERM (Unix) — whichever comes first.
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Builds the identity resolver for the compiled feature set. Release
/// builds (no `dev-auth`) deny every request — there is no production OIDC
/// integration in Phase 1 (recorded status, ledger P1.17).
fn build_identity_resolver() -> Arc<dyn IdentityResolver> {
    #[cfg(feature = "dev-auth")]
    {
        Arc::new(DevAuthResolver)
    }
    #[cfg(not(feature = "dev-auth"))]
    {
        Arc::new(DenyAllResolver)
    }
}

/// The database readiness check. The adapter exposes no dedicated ping; the
/// recorded check is one real database round trip: open a unit of work
/// (platform level, no tenant GUC) and commit it immediately. Both steps
/// are synchronous adapter calls -> blocking thread.
fn database_check(store: Arc<flexcore_postgres_adapter::FlexCoreStore>) -> HealthCheck {
    Arc::new(move || {
        let store = store.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || match store.begin(None) {
                Ok(uow) => uow
                    .commit()
                    .map(|_| ())
                    .map_err(|e| format!("database check failed: {e}")),
                Err(e) => Err(format!("database check failed: {e}")),
            })
            .await
            .map_err(|e| format!("database check task failed: {e}"))?
        }) as std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
    })
}

/// The broker readiness check: one core request to a subject outside every
/// stream filter. A broker that processed the request answers "no
/// responders" — that IS the healthy outcome (the request round trip
/// happened); a connection failure or our own timeout is not.
///
// VERIFY-BUILD (async-nats 0.50): `RequestError::kind()` and
// `RequestErrorKind::NoResponders` are the documented shapes; the same
// accessor is already relied on by crates/adapters/nats/src/consumer.rs.
fn broker_check(client: async_nats::Client) -> HealthCheck {
    Arc::new(move || {
        let client = client.clone();
        Box::pin(async move {
            let request =
                tokio::time::timeout(BROKER_HEALTH_TIMEOUT, client.request(
                    BROKER_HEALTH_SUBJECT,
                    Vec::<u8>::new().into(),
                ))
                .await;
            match request {
                Err(_) => Err(format!(
                    "broker did not answer the health request within {} ms",
                    BROKER_HEALTH_TIMEOUT.as_millis()
                )),
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => {
                    if matches!(
                        e.kind(),
                        async_nats::client::RequestErrorKind::NoResponders
                    ) {
                        Ok(())
                    } else {
                        Err(format!("broker health request failed: {e}"))
                    }
                }
            }
        }) as std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
    })
}
