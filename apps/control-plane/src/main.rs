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

use flexcore_control_plane::auth::IdentityResolver;
use flexcore_control_plane::config::Config;
use flexcore_control_plane::metrics::track_metrics;
use flexcore_control_plane::probes::{HealthCheck, Readiness};
use flexcore_control_plane::routes::{fim_router, stage1_router};
use flexcore_control_plane::state::{AppState, Backend};

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

    // FIM Wave 6.5 — the FIM COMPOSITION CONFIGURATION (`fim_compose`
    // below). Every live Wave 4-6 service is OFF unless its configuration
    // section is present: absent sections construct exactly the
    // fail-closed defaults recorded before this wave (DenyAllResolver, no
    // step-up service, the unwired webhook transport, no SCIM / OIDC /
    // machine-identity / API-key / OTA composition). Parsed with the same
    // fail-fast discipline as `Config` (named errors; secrets are never
    // logged). WHY AN OWN ENV SURFACE: `config.rs` is a closed struct
    // loaded by `Config::from_env` and was frozen for this wave's agent,
    // so the composition reads its own `FLEXCORE_*` variables through an
    // INJECTED lookup (unit-testable without process-global env mutation),
    // reusing `ConfigError` for the named-error contract.
    let fim = match fim_compose::FimCompose::from_env() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("configuration error: fim composition ({e})");
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

    // FIM2-R1-004 — the ONE SCIM store of this composition: constructed
    // ONCE here and handed to BOTH the identity resolver's
    // deprovisioning-enforcement wrap and the SCIM HTTP surface, so a
    // subject provisioned or deactivated through the SCIM connection is
    // visible to identity resolution (the recorded F-07 composition
    // semantics; two independent stores would deny every token on the
    // composed path — the resolver's store would stay empty forever).
    // `None` when no SCIM connection is configured (every consumer keeps
    // its recorded absent-configuration default).
    let scim_store = fim_compose::build_scim_store(&fim);

    // Identity resolver (FIM Wave 6.5): the configuration-gated
    // construction — the production OIDC path when configured (FIM-F07.01,
    // composed over `HttpJwksSource`; NO network happens at construction —
    // the first token drives the first lazy JWKS fetch), else the Phase 1
    // feature-gated posture; wrapped by the SCIM deprovisioning enforcement
    // (`ScimAwareIdentityResolver`) when a SCIM connection is configured.
    // Absent configuration ⇒ DenyAllResolver exactly as recorded (Phase 1
    // status; `tests/p10_04_deny_all_and_default_policies.rs` pins the
    // deny-all behavior, `tests/fim_composition.rs` pins the gating).
    let identity: Arc<dyn IdentityResolver> =
        match fim_compose::build_identity_resolver(&fim, &clock, scim_store.clone()) {
            Ok(resolver) => resolver,
            Err(e) => {
                eprintln!("configuration error: fim identity composition ({e})");
                std::process::exit(2);
            }
        };

    // FIM Wave 6.5 — the remaining configuration-gated service
    // constructions (each fail-fast with a named error; each returns its
    // recorded default when the section is absent). Built BEFORE
    // `AppState::new` because the machine-identity/API-key/SCIM/step-up
    // closures borrow clones of the id generator and clock ports, and the
    // `ids` Arc moves into the backend below.
    let webhook_transport = match fim_compose::build_webhook_transport(&fim) {
        Ok(transport) => transport,
        Err(e) => {
            eprintln!("configuration error: fim webhook transport ({e})");
            std::process::exit(2);
        }
    };
    let step_up = match fim_compose::build_step_up_service(&fim, &ids, &clock) {
        Ok(service) => service,
        Err(e) => {
            eprintln!("configuration error: fim step-up composition ({e})");
            std::process::exit(2);
        }
    };
    let scim = match fim_compose::build_scim_service(&fim, &ids, &clock, scim_store) {
        Ok(composed) => composed,
        Err(e) => {
            eprintln!("configuration error: fim scim composition ({e})");
            std::process::exit(2);
        }
    };
    let machine_identity = match fim_compose::build_machine_identity_service(&fim, &clock) {
        Ok(service) => service,
        Err(e) => {
            eprintln!("configuration error: fim machine-identity composition ({e})");
            std::process::exit(2);
        }
    };
    let api_keys = match fim_compose::build_api_keys_service(&fim, &clock) {
        Ok(service) => service,
        Err(e) => {
            eprintln!("configuration error: fim api-keys composition ({e})");
            std::process::exit(2);
        }
    };
    let ota_rollout = match fim_compose::build_ota_rollout(&fim) {
        Ok(coordinator) => coordinator,
        Err(e) => {
            eprintln!("configuration error: fim ota-rollout composition ({e})");
            std::process::exit(2);
        }
    };

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
    // FRV1-R2-009, corrected to the HONEST POSTURE (FIM2-R1-005 of the FIM
    // final analysis — the prior wording "apikey.*/market.* have no routes"
    // is factually stale since Wave 8B): the machine-identity and API-key
    // REST surfaces DO exist in this binary and DO evaluate the `apikey.*`
    // actions against `state.policies` (`machine_identity_routes.rs:359,
    // :489`; `api_key_routes.rs:328, :357, :384`). The DEFAULT composition
    // stays `phase5_default_policies()` — zero `apikey.*` grants — so those
    // admin surfaces deny for every caller (deny-by-default preserved; the
    // shipped 403 posture is pinned by `tests/fim_identity_routes.rs`).
    // A deployment that sets `FLEXCORE_APIKEY_POLICIES_ENABLED=true`
    // composes the phase-13 `apikey.*` credential grants
    // (`fim_compose::build_policy_set`, equivalence-pinned against the
    // authoritative `flexcore_authz::phase13_policies()`). Still
    // deliberately NOT composed here: the phase-13 SiteApprover
    // `command.approve` slice (no REST surface in this binary evaluates
    // command.approve — the command gates run via sim-runner; FRV1-R2-009
    // stands for it), `flexcore_authz::phase16_market_policies` (market.*
    // has no routes), and `flexcore_authz::phase4_command_policies`
    // (command gate actions are evaluated at the GATES, not through the
    // REST policy surface).
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
        fim_compose::build_policy_set(&fim),
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
    // FRV1-R8-005 (final review; FIM Wave 6.5 correction): the P13.09
    // webhook-delivery bookkeeping store composes here, over the Postgres
    // adapter's store (`webhook_delivery_pg.rs` implements the port;
    // migration 0020 is the schema). COMPOSITION IS NOT DELIVERY: in 0.x
    // there is no enqueue route, no hosting scan loop, and no
    // secret-resolver composition at the P11.16 seam — the hosting loop +
    // resolver remain the P13.09 decision record §5's open items.
    //
    // TRANSPORT POSTURE (correcting this comment's STALE review-time claim
    // that "the workspace has no HTTP-client dependency"): the LIVE
    // transport EXISTS since FIM-F08.03 (`webhook_http.rs` —
    // `ReqwestOutboundHttpPost` over the lock-resolved reqwest 0.13.5) and
    // is CONSTRUCTED ONLY WHEN the `FLEXCORE_WEBHOOK_HTTP_ENABLED`
    // configuration section is present (WRITTEN-UNRUN; live receiver
    // interop is a HUMAN LAB GATE). Absent that configuration the
    // transport seam composes `UnwiredOutboundHttpPost`
    // (`webhook_service.rs`), the fail-closed seam that REFUSES every
    // transmission with the named `E_TRANSPORT_UNWIRED` error — the
    // unwired default stays the DEFAULT (`fim_compose::
    // build_webhook_transport`, pinned by the fim_compose unit tests and
    // `tests/fim_composition.rs`).
    .with_webhook_delivery(Arc::new(
        flexcore_postgres_adapter::webhook_delivery::PgWebhookDeliveryStore::new(
            (*store).clone(),
        ),
    ))
    // FIM Wave 6.5: the transport seam is an explicit state field
    // (`AppState::webhook_transport`) so the live/unwired choice above is
    // configuration-conditional; the default constructed inside
    // `AppState::new` is the same unwired seam, byte-identical to the
    // pre-FIM posture.
    .with_webhook_transport(webhook_transport);

    // FIM Wave 6.5 — the CONDITIONAL compositions. Each is applied only
    // when its configuration section is present; absent ⇒ the recorded
    // default stays exactly as before (no step-up service, no SCIM
    // connection, no machine-identity or API-key service, no OTA
    // rollout).
    let state = match step_up {
        Some(service) => state.with_step_up(service),
        None => state,
    };
    let state = match scim {
        Some((service, bearer_digest)) => state.with_scim(service, bearer_digest),
        None => state,
    };
    let state = match machine_identity {
        Some(service) => state.with_machine_identity(service),
        None => state,
    };
    let state = match api_keys {
        Some(service) => state.with_api_keys(service),
        None => state,
    };
    let state = match ota_rollout {
        Some(coordinator) => state.with_ota_rollout(coordinator),
        None => state,
    };

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

    let app = stage1_router(state.clone())
        // FIM Wave 6.5: the configuration-gated FIM surfaces
        // (`routes.rs::fim_router` — the SCIM 2.0 surface mounts ONLY when
        // the SCIM service is composed WITH its bearer digest; absent
        // configuration mounts NO routes at all, not a 404 fallback).
        // Merged before the middleware stack so every mounted surface
        // rides the same metrics/rate-limit/timeout/body-limit layers as
        // the stage-1 routes.
        .merge(fim_router(&state))
        .layer(
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

/// FIM Wave 6.5 (Agent A) — the FIM composition configuration and the
/// construction helpers for the Wave 4-6 services (F-07 enterprise
/// identity, F-08 integration platform, F-11 degradation input data, F-12
/// OTA rollout).
///
/// WHY THIS MODULE LIVES IN THE BINARY: `main.rs` is the composition root
/// (P1.17 — the only place that wires adapters to ports), and the
/// control-plane package is lib+bin with `lib.rs` FROZEN for this wave
/// (no new library module may be registered). The helpers therefore live
/// in this binary-crate module; the reusable LIB-side seams they fill are
/// the additive `AppState::with_*` builders (`state.rs`) and the additive
/// `routes.rs::fim_router` — both pinned by
/// `tests/fim_composition.rs`. The binary-side behavior (the env gating
/// and the constructions below) is pinned by the unit tests at the bottom
/// of this module, run by the human build gate (`cargo test -p
/// flexcore-control-plane --bin flexcore-control-plane`).
///
/// CONFIGURATION MECHANISM (recorded decision): `config.rs` is a closed
/// struct loaded by `Config::from_env` and was frozen for this wave, so
/// the FIM sections read their OWN environment variables through an
/// injected lookup closure — deterministic unit tests pass a table, no
/// process-global env mutation. Every section is gated by an explicit
/// `FLEXCORE_*_ENABLED=true` variable: absent (or `false`) constructs
/// EXACTLY the pre-FIM fail-closed default; a present-but-invalid value is
/// a named `ConfigError` (the config.rs vocabulary — reused, not
/// restated), which the composition root turns into the standard fail-fast
/// exit. Secrets are never logged: the SCIM bearer token is held only as
/// its SHA-256 digest from parse time on; the machine-identity issuer
/// secret and the OTA evidence-chain key are carried in non-Debug structs
/// and never rendered.
///
/// THE FAIL-CLOSED DEFAULTS THIS MODULE PRESERVES (pinned by tests):
/// - `DenyAllResolver` — absent OIDC and SCIM configuration (and no
///   `dev-auth` feature) builds exactly the pre-FIM resolver;
/// - no step-up service — absent OIDC configuration, `state.step_up` stays
///   `None` (every step-up request fails closed, exactly as before);
/// - `UnwiredOutboundHttpPost` — absent webhook-HTTP configuration, the
///   transport seam refuses every transmission with `E_TRANSPORT_UNWIRED`;
/// - no SCIM routes — the router is never mounted (no 404 fallback);
/// - no machine-identity/API-key service, no OTA rollout — the state slots
///   stay `None`;
/// - the shipped phase-5 policy set (FIM2-R1-005) — the
///   `FLEXCORE_APIKEY_POLICIES_ENABLED` gate absent or `false` composes
///   EXACTLY `phase5_default_policies()` (zero `apikey.*` grants: the
///   Wave 8B admin surfaces deny for every caller), and a SCIM section is
///   served only over the ONE composition-wide store
///   (`build_scim_store` — FIM2-R1-004; a section without it is a named
///   fail-closed miscomposition).
mod fim_compose {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use flexcore_authz::action::Action;
    use flexcore_authz::decision::EvidenceRef;
    use flexcore_authz::phase13_policies::PHASE13_POLICY_PREFIX;
    use flexcore_authz::policy::{PolicySet, Role};
    use flexcore_authz::scope::ScopeKind;
    use flexcore_control_plane::auth::{IdentityResolver, phase5_default_policies};
    use flexcore_control_plane::config::ConfigError;
    use flexcore_control_plane::ota_rollout::{
        build_plan, MemoryOtaRolloutAuditSink, OtaCohortDefinition, OtaRolloutCoordinator,
    };
    use flexcore_control_plane::scim::{ScimAwareIdentityResolver, ScimService};
    use flexcore_control_plane::scim_store::{InMemoryScimStore, ScimStore};
    use flexcore_control_plane::stepup::{
        id_from_uuid, InMemoryStepUpStore, OidcStepUpVerifier, StepUpService,
        DEFAULT_STEP_UP_EVIDENCE_MAX_AGE, DEFAULT_STEP_UP_TTL,
    };
    use flexcore_control_plane::webhook_http::{
        ReqwestOutboundHttpPost, WebhookHttpConfig, CONNECT_TIMEOUT_SECONDS_DEFAULT,
        TOTAL_TIMEOUT_SECONDS_DEFAULT,
    };
    use flexcore_control_plane::webhook_service::{OutboundHttpPost, UnwiredOutboundHttpPost};
    use flexcore_application::rollout_orchestrator::{AbortThresholds, MemoryRolloutAuditSink};
    use flexcore_audit::chain::ChainKey;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::bess_cost_curve::{BessDegradationCostInput, BessDegradationCurveShape};
    use flexcore_domain::bess_degradation::DegradationChemistry;
    use flexcore_domain::ids::{AssetId, TenantId};
    use flexcore_ports::clock::Clock;
    use flexcore_ports::id_generator::IdGenerator;

    /// The injected variable lookup: `from_env` passes `std::env::var`;
    /// tests pass a deterministic table (no process-global env mutation).
    type Lookup = dyn Fn(&str) -> Option<String>;

    // --- The parsed configuration sections -----------------------------------

    /// FIM-F08.03 (webhook transport): the transport budgets. Gated by
    /// `FLEXCORE_WEBHOOK_HTTP_ENABLED=true`; optional
    /// `FLEXCORE_WEBHOOK_HTTP_CONNECT_TIMEOUT_SECS` /
    /// `FLEXCORE_WEBHOOK_HTTP_TOTAL_TIMEOUT_SECS` (defaults: the registered
    /// recommendations in `webhook_http.rs` — 10 s / 30 s).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WebhookHttpSection {
        pub connect_timeout_secs: u64,
        pub total_timeout_secs: u64,
    }

    /// FIM-F07.01 (OIDC): the one production identity path. Gated by
    /// `FLEXCORE_OIDC_ENABLED=true`; required
    /// `FLEXCORE_OIDC_ISSUER` / `FLEXCORE_OIDC_JWKS_URL` (must be https) /
    /// `FLEXCORE_OIDC_AUDIENCE`; optional
    /// `FLEXCORE_OIDC_EXPECTED_CLIENT_ID`,
    /// `FLEXCORE_OIDC_JWKS_FETCH_TIMEOUT_MS` (default: the registered
    /// recommendation `DEFAULT_JWKS_FETCH_TIMEOUT`, 5 s), and
    /// `FLEXCORE_OIDC_TENANT_BINDINGS` — `issuer=tenant-uuid` pairs
    /// separated by `;` (the closed in-config tenant mapper; ABSENT
    /// bindings compose the mapper that maps nothing, so every token is
    /// DENIED at the mapper — the documented fail-closed empty-mapper
    /// behavior, never a default tenant).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OidcSection {
        pub issuer: String,
        pub jwks_url: String,
        pub audience: String,
        pub expected_client_id: Option<String>,
        pub jwks_fetch_timeout: Duration,
        pub tenant_bindings: HashMap<String, String>,
    }

    /// FIM-F07.02 (SCIM): the ONE connection this process serves. Gated by
    /// `FLEXCORE_SCIM_ENABLED=true`; required `FLEXCORE_SCIM_TENANT_ID`
    /// (the pinned tenant) and `FLEXCORE_SCIM_BEARER_TOKEN` (the shared
    /// credential of the IdP's SCIM push client — carried ONLY as its
    /// SHA-256 digest from parse time on, never logged, never stored).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScimSection {
        pub tenant: TenantId,
        pub bearer_digest: [u8; 32],
    }

    /// FIM-F08.01 (machine identity). Gated by
    /// `FLEXCORE_MACHINE_IDENTITY_ENABLED=true`; required
    /// `FLEXCORE_MACHINE_IDENTITY_ISSUER` /
    /// `FLEXCORE_MACHINE_IDENTITY_AUDIENCE` /
    /// `FLEXCORE_MACHINE_IDENTITY_ISSUER_SECRET` (the symmetric issuer key
    /// material — never logged; NOT carried in a Debug-rendering struct).
    /// TTL/leeway keep the registered service defaults
    /// (`MachineIdentityConfig::default()`).
    #[derive(Clone)]
    pub struct MachineIdentitySection {
        pub issuer: String,
        pub audience: String,
        pub issuer_secret: String,
    }

    /// FIM-F08.02 (API keys). Gated by `FLEXCORE_API_KEYS_ENABLED=true`;
    /// optional `FLEXCORE_API_KEYS_ROTATION_GRACE_SECONDS` (default: the
    /// registered 24 h recommendation `ROTATION_GRACE_SECONDS_DEFAULT`).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ApiKeysSection {
        pub rotation_grace_seconds: i64,
    }

    /// FIM-F12.05 (OTA rollout). Gated by
    /// `FLEXCORE_OTA_ROLLOUT_ENABLED=true`; required
    /// `FLEXCORE_OTA_TARGET_VERSION`,
    /// `FLEXCORE_OTA_COHORTS` (format
    /// `cohort_id:device1,device2;cohort_id2:device3` — the FIRST cohort is
    /// the canary), the abort thresholds
    /// `FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE` /
    /// `FLEXCORE_OTA_MIN_OBSERVATIONS` /
    /// `FLEXCORE_OTA_CANARY_BASIS_POINTS` (REQUIRED — safety thresholds are
    /// an explicit operator choice, never an invented default), and
    /// `FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX` (the P11.15
    /// deployment-supplied key material, ≥ 32 bytes, hex-encoded — never
    /// logged; the P18.03/P11.15 key ceremonies remain HUMAN GATES). The
    /// per-cohort `max_failing_devices` tripwire is not reachable through
    /// this surface (the definition carries `None` = the plan's failure-rate
    /// threshold governs — the struct's documented safe reading).
    #[derive(Clone)]
    pub struct OtaSection {
        pub target_version: String,
        pub cohorts: Vec<OtaCohortDefinition>,
        pub max_failure_rate_permille: u32,
        pub min_observations: u32,
        pub canary_basis_points: u16,
        pub evidence_chain_key: Vec<u8>,
    }

    /// The parsed FIM composition configuration: `None` for every section
    /// whose gate is absent — the constructor maps each `None` onto EXACTLY
    /// the pre-FIM default (see the builders below). `apikey_policies` is
    /// the pure-flag section (FIM2-R1-005): `false` composes exactly the
    /// shipped phase-5 policy set.
    pub struct FimCompose {
        pub webhook_http: Option<WebhookHttpSection>,
        pub oidc: Option<OidcSection>,
        pub scim: Option<ScimSection>,
        pub machine_identity: Option<MachineIdentitySection>,
        pub api_keys: Option<ApiKeysSection>,
        pub apikey_policies: bool,
        pub ota: Option<OtaSection>,
    }

    impl FimCompose {
        /// The production lookup: the process environment.
        pub fn from_env() -> Result<Self, ConfigError> {
            Self::from_lookup(&|name| std::env::var(name).ok())
        }

        /// The injected lookup (unit-testable, deterministic).
        pub fn from_lookup(lookup: &Lookup) -> Result<Self, ConfigError> {
            Ok(Self {
                webhook_http: parse_webhook_http(lookup)?,
                oidc: parse_oidc(lookup)?,
                scim: parse_scim(lookup)?,
                machine_identity: parse_machine_identity(lookup)?,
                api_keys: parse_api_keys(lookup)?,
                apikey_policies: parse_apikey_policies(lookup)?,
                ota: parse_ota(lookup)?,
            })
        }
    }

    // --- Parsing helpers -----------------------------------------------------

    /// The section gate: absent = off (the default), `true`/`false` (case
    /// insensitive) = explicit; anything else is a named invalid error.
    fn enabled(lookup: &Lookup, name: &'static str) -> Result<bool, ConfigError> {
        match lookup(name) {
            None => Ok(false),
            Some(raw) if raw.eq_ignore_ascii_case("true") => Ok(true),
            Some(raw) if raw.eq_ignore_ascii_case("false") => Ok(false),
            Some(raw) => Err(ConfigError::Invalid {
                name,
                reason: format!("'{raw}' is not true|false"),
            }),
        }
    }

    /// A required non-empty string (trimmed); a named error carries no
    /// value (P1.21).
    fn required(lookup: &Lookup, name: &'static str) -> Result<String, ConfigError> {
        match lookup(name) {
            Some(raw) if !raw.trim().is_empty() => Ok(raw.trim().to_string()),
            Some(_) => Err(ConfigError::Invalid {
                name,
                reason: "empty".to_string(),
            }),
            None => Err(ConfigError::Missing { name }),
        }
    }

    /// An optional non-negative integer with a registered default.
    fn optional_u64(
        lookup: &Lookup,
        name: &'static str,
        default: u64,
    ) -> Result<u64, ConfigError> {
        match lookup(name) {
            None => Ok(default),
            Some(raw) => raw.trim().parse::<u64>().map_err(|_| ConfigError::Invalid {
                name,
                reason: "not a non-negative integer".to_string(),
            }),
        }
    }

    /// An optional non-negative integer without a default (required-when-
    /// enabled).
    fn required_u(lookup: &Lookup, name: &'static str) -> Result<u64, ConfigError> {
        let raw = required(lookup, name)?;
        raw.parse::<u64>().map_err(|_| ConfigError::Invalid {
            name,
            reason: "not a non-negative integer".to_string(),
        })
    }

    /// Hand-rolled hex decoding (no hex crate — the FIM dependency rule;
    /// the encoding direction lives in `webhook_signing::hex_encode_digest`,
    /// this is its strict parse counterpart, lower/upper case accepted).
    fn decode_hex(raw: &str) -> Result<Vec<u8>, ConfigError> {
        fn nibble(c: u8) -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        }
        let bytes = raw.as_bytes();
        if bytes.len() % 2 != 0 {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX",
                reason: "odd hex length".to_string(),
            });
        }
        let mut out = Vec::with_capacity(bytes.len() / 2);
        for pair in bytes.chunks_exact(2) {
            let high = nibble(pair[0]).ok_or(ConfigError::Invalid {
                name: "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX",
                reason: "not a hex character".to_string(),
            })?;
            let low = nibble(pair[1]).ok_or(ConfigError::Invalid {
                name: "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX",
                reason: "not a hex character".to_string(),
            })?;
            out.push((high << 4) | low);
        }
        Ok(out)
    }

    // --- The section parsers -------------------------------------------------

    fn parse_webhook_http(lookup: &Lookup) -> Result<Option<WebhookHttpSection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_WEBHOOK_HTTP_ENABLED")? {
            return Ok(None);
        }
        Ok(Some(WebhookHttpSection {
            connect_timeout_secs: optional_u64(
                lookup,
                "FLEXCORE_WEBHOOK_HTTP_CONNECT_TIMEOUT_SECS",
                CONNECT_TIMEOUT_SECONDS_DEFAULT,
            )?,
            total_timeout_secs: optional_u64(
                lookup,
                "FLEXCORE_WEBHOOK_HTTP_TOTAL_TIMEOUT_SECS",
                TOTAL_TIMEOUT_SECONDS_DEFAULT,
            )?,
        }))
    }

    fn parse_oidc(lookup: &Lookup) -> Result<Option<OidcSection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_OIDC_ENABLED")? {
            return Ok(None);
        }
        let issuer = required(lookup, "FLEXCORE_OIDC_ISSUER")?;
        let jwks_url = required(lookup, "FLEXCORE_OIDC_JWKS_URL")?;
        // Composition-level gate: the JWKS fetch is HTTPS by contract
        // (`HttpJwksSource`: "HTTPS GET of the CONFIGURED URL"). A non-https
        // URL is a misconfiguration, refused at load (fail fast, never a
        // silently-downgraded fetch).
        if !jwks_url.starts_with("https://") {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OIDC_JWKS_URL",
                reason: "the JWKS URL must be an https:// URL".to_string(),
            });
        }
        let audience = required(lookup, "FLEXCORE_OIDC_AUDIENCE")?;
        let expected_client_id = match lookup("FLEXCORE_OIDC_EXPECTED_CLIENT_ID") {
            None => None,
            Some(raw) if raw.trim().is_empty() => None,
            Some(raw) => Some(raw.trim().to_string()),
        };
        let fetch_ms = optional_u64(
            lookup,
            "FLEXCORE_OIDC_JWKS_FETCH_TIMEOUT_MS",
            flexcore_control_plane::oidc::DEFAULT_JWKS_FETCH_TIMEOUT.as_millis() as u64,
        )?;
        if fetch_ms == 0 {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OIDC_JWKS_FETCH_TIMEOUT_MS",
                reason: "the fetch timeout must be a positive millisecond value".to_string(),
            });
        }
        let tenant_bindings = parse_tenant_bindings(lookup)?;
        Ok(Some(OidcSection {
            issuer,
            jwks_url,
            audience,
            expected_client_id,
            jwks_fetch_timeout: Duration::from_millis(fetch_ms),
            tenant_bindings,
        }))
    }

    /// `issuer=tenant-uuid` pairs separated by `;` (empty/absent = the
    /// empty mapper that maps nothing and denies everything).
    fn parse_tenant_bindings(lookup: &Lookup) -> Result<HashMap<String, String>, ConfigError> {
        let raw = match lookup("FLEXCORE_OIDC_TENANT_BINDINGS") {
            None => return Ok(HashMap::new()),
            Some(raw) => raw,
        };
        let mut bindings = HashMap::new();
        for entry in raw.split(';').filter(|e| !e.trim().is_empty()) {
            let (issuer, tenant) = entry.split_once('=').ok_or(ConfigError::Invalid {
                name: "FLEXCORE_OIDC_TENANT_BINDINGS",
                reason: format!("binding '{entry}' is not issuer=tenant-uuid"),
            })?;
            let issuer = issuer.trim();
            let tenant = tenant.trim();
            if issuer.is_empty() {
                return Err(ConfigError::Invalid {
                    name: "FLEXCORE_OIDC_TENANT_BINDINGS",
                    reason: format!("binding '{entry}' carries an empty issuer"),
                });
            }
            let uuid = FlexUuid::parse(tenant).map_err(|_| ConfigError::Invalid {
                name: "FLEXCORE_OIDC_TENANT_BINDINGS",
                reason: format!("binding '{entry}' carries a non-canonical tenant uuid"),
            })?;
            bindings.insert(issuer.to_string(), uuid.to_hyphenated());
        }
        Ok(bindings)
    }

    fn parse_scim(lookup: &Lookup) -> Result<Option<ScimSection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_SCIM_ENABLED")? {
            return Ok(None);
        }
        let tenant_raw = required(lookup, "FLEXCORE_SCIM_TENANT_ID")?;
        let uuid = FlexUuid::parse(&tenant_raw).map_err(|_| ConfigError::Invalid {
            name: "FLEXCORE_SCIM_TENANT_ID",
            reason: "not a canonical tenant uuid".to_string(),
        })?;
        let bearer = required(lookup, "FLEXCORE_SCIM_BEARER_TOKEN")?;
        // ONLY the digest is carried forward (the guard compares digests;
        // the token itself is never stored, never logged).
        let bearer_digest = flexcore_control_plane::webhook_signing::sha256(bearer.as_bytes());
        Ok(Some(ScimSection {
            tenant: TenantId::new(uuid),
            bearer_digest,
        }))
    }

    fn parse_machine_identity(
        lookup: &Lookup,
    ) -> Result<Option<MachineIdentitySection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_MACHINE_IDENTITY_ENABLED")? {
            return Ok(None);
        }
        Ok(Some(MachineIdentitySection {
            issuer: required(lookup, "FLEXCORE_MACHINE_IDENTITY_ISSUER")?,
            audience: required(lookup, "FLEXCORE_MACHINE_IDENTITY_AUDIENCE")?,
            issuer_secret: required(lookup, "FLEXCORE_MACHINE_IDENTITY_ISSUER_SECRET")?,
        }))
    }

    fn parse_api_keys(lookup: &Lookup) -> Result<Option<ApiKeysSection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_API_KEYS_ENABLED")? {
            return Ok(None);
        }
        Ok(Some(ApiKeysSection {
            rotation_grace_seconds: optional_u64(
                lookup,
                "FLEXCORE_API_KEYS_ROTATION_GRACE_SECONDS",
                flexcore_control_plane::api_keys::ROTATION_GRACE_SECONDS_DEFAULT as u64,
            )? as i64,
        }))
    }

    /// FIM2-R1-005 — the apikey-policies gate (a pure flag, no section
    /// struct): `FLEXCORE_APIKEY_POLICIES_ENABLED=true` composes the
    /// phase-13 `apikey.*` credential grants into the shipped policy set
    /// (`build_policy_set`); absent or `false` keeps EXACTLY the shipped
    /// `phase5_default_policies()` (zero apikey.* grants — deny-by-default
    /// preserved for the Wave 8B admin surfaces). Same gate discipline as
    /// every other section: absent = off, strict `true`/`false`, anything
    /// else a named `ConfigError`.
    fn parse_apikey_policies(lookup: &Lookup) -> Result<bool, ConfigError> {
        enabled(lookup, "FLEXCORE_APIKEY_POLICIES_ENABLED")
    }

    fn parse_ota(lookup: &Lookup) -> Result<Option<OtaSection>, ConfigError> {
        if !enabled(lookup, "FLEXCORE_OTA_ROLLOUT_ENABLED")? {
            return Ok(None);
        }
        let target_version = required(lookup, "FLEXCORE_OTA_TARGET_VERSION")?;
        let cohorts_raw = required(lookup, "FLEXCORE_OTA_COHORTS")?;
        let max_failure_rate_permille = required_u(lookup, "FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE")?;
        if max_failure_rate_permille > 1000 {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE",
                reason: "a permille failure rate cannot exceed 1000".to_string(),
            });
        }
        let min_observations = required_u(lookup, "FLEXCORE_OTA_MIN_OBSERVATIONS")?;
        if min_observations == 0 {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OTA_MIN_OBSERVATIONS",
                reason: "the minimum observation count must be at least 1".to_string(),
            });
        }
        let canary_basis_points = required_u(lookup, "FLEXCORE_OTA_CANARY_BASIS_POINTS")?;
        if !(500..=1000).contains(&canary_basis_points) {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OTA_CANARY_BASIS_POINTS",
                reason: "the canary basis points must be within 500..=1000".to_string(),
            });
        }
        let key_hex = required(lookup, "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX")?;
        let evidence_chain_key = decode_hex(&key_hex)?;
        Ok(Some(OtaSection {
            target_version,
            cohorts: parse_cohorts(&cohorts_raw)?,
            max_failure_rate_permille: max_failure_rate_permille as u32,
            min_observations: min_observations as u32,
            canary_basis_points: canary_basis_points as u16,
            evidence_chain_key,
        }))
    }

    /// `cohort_id:device1,device2;cohort_id2:device3` — the FIRST cohort is
    /// the canary (`build_plan` enforces the canary-first rule and every
    /// other plan invariant; a malformed entry is a named error here).
    fn parse_cohorts(raw: &str) -> Result<Vec<OtaCohortDefinition>, ConfigError> {
        let mut cohorts = Vec::new();
        for (index, entry) in raw
            .split(';')
            .filter(|e| !e.trim().is_empty())
            .enumerate()
        {
            let (cohort_id, devices) = entry.split_once(':').ok_or(ConfigError::Invalid {
                name: "FLEXCORE_OTA_COHORTS",
                reason: format!("cohort entry '{entry}' is not cohort_id:device_ids"),
            })?;
            let device_ids: Vec<String> = devices
                .split(',')
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string)
                .collect();
            if device_ids.is_empty() {
                return Err(ConfigError::Invalid {
                    name: "FLEXCORE_OTA_COHORTS",
                    reason: format!("cohort entry '{entry}' declares no device ids"),
                });
            }
            cohorts.push(OtaCohortDefinition {
                cohort_id: cohort_id.trim().to_string(),
                // Exactly the first cohort is the canary (build_plan
                // refuses any other arrangement).
                canary: index == 0,
                device_ids,
                // The per-cohort tripwire is not reachable through this
                // surface (None = the plan's failure-rate threshold
                // governs — the struct's documented safe reading).
                max_failing_devices: None,
            });
        }
        if cohorts.is_empty() {
            return Err(ConfigError::Invalid {
                name: "FLEXCORE_OTA_COHORTS",
                reason: "no cohort entries".to_string(),
            });
        }
        Ok(cohorts)
    }

    // --- The injected-port closures ------------------------------------------

    /// The millis closure every Wave 4-6 service takes
    /// (`Box<dyn Fn() -> i64 + Send + Sync>`), derived from the composed
    /// `Clock` port (never a wall-clock read inside a service).
    fn clock_millis_closure(clock: &Arc<dyn Clock>) -> Box<dyn Fn() -> i64 + Send + Sync> {
        let clock = clock.clone();
        Box::new(move || clock.now_event_time().unix_millis() as i64)
    }

    /// The id closure (`Box<dyn Fn() -> String + Send + Sync>`): the
    /// composed UUIDv7 generator's identifiers in their canonical
    /// hyphenated string form.
    fn ids_string_closure(ids: &Arc<dyn IdGenerator>) -> Box<dyn Fn() -> String + Send + Sync> {
        let ids = ids.clone();
        Box::new(move || ids.new_uuid().to_hyphenated())
    }

    // --- The builders (each absent section ⇒ the pre-FIM default) ------------

    /// The webhook transport seam: the LIVE `ReqwestOutboundHttpPost` when
    /// the section is present, else `UnwiredOutboundHttpPost` — the exact
    /// pre-FIM default (fail-closed, `E_TRANSPORT_UNWIRED` on every post).
    /// Client construction does NOT touch the network (the first post is
    /// the first transmission — WRITTEN-UNRUN, HUMAN LAB GATE).
    pub fn build_webhook_transport(
        fim: &FimCompose,
    ) -> Result<Arc<dyn OutboundHttpPost>, ConfigError> {
        match &fim.webhook_http {
            None => Ok(Arc::new(UnwiredOutboundHttpPost)),
            Some(section) => {
                let config = WebhookHttpConfig {
                    connect_timeout_secs: section.connect_timeout_secs,
                    total_timeout_secs: section.total_timeout_secs,
                };
                let transport: Arc<dyn OutboundHttpPost> =
                    Arc::new(ReqwestOutboundHttpPost::new(config).map_err(|e| {
                        ConfigError::Invalid {
                            name: "FLEXCORE_WEBHOOK_HTTP_ENABLED",
                            reason: format!(
                                "the webhook HTTP transport could not be constructed: {e}"
                            ),
                        }
                    })?);
                Ok(transport)
            }
        }
    }

    /// The identity resolver: the production OIDC path when configured
    /// (FIM-F07.01 — overrides the dev-only resolver; a production
    /// configuration is the stronger intent), else the Phase 1
    /// feature-gated posture; wrapped by the SCIM deprovisioning
    /// enforcement when a SCIM connection is configured. NO network
    /// happens in any construction path here (the JWKS fetch is lazy, on
    /// the first token).
    ///
    /// The SCIM wrap consumes the ONE composition-wide store
    /// (`build_scim_store` — FIM2-R1-004 of the FIM final analysis: the
    /// resolver and the SCIM HTTP surface MUST share one store, or a
    /// provisioned subject is invisible to the resolver and a SCIM
    /// deactivation never propagates). A SCIM section without its shared
    /// store is a MISCOMPOSITION: a named, fail-closed refusal — never a
    /// second store the SCIM surface cannot reach.
    pub fn build_identity_resolver(
        fim: &FimCompose,
        clock: &Arc<dyn Clock>,
        scim_store: Option<Arc<InMemoryScimStore>>,
    ) -> Result<Arc<dyn IdentityResolver>, ConfigError> {
        // The INNER identity path.
        let inner: Box<dyn IdentityResolver> = match &fim.oidc {
            Some(section) => {
                let deployment = flexcore_control_plane::oidc::OidcDeploymentConfig {
                    issuer: section.issuer.clone(),
                    jwks_url: section.jwks_url.clone(),
                    audience: section.audience.clone(),
                    expected_client_id: section.expected_client_id.clone(),
                    jwks_fetch_timeout: section.jwks_fetch_timeout,
                };
                let mapper = flexcore_control_plane::oidc::StaticTenantMapper::new(
                    section.tenant_bindings.clone(),
                );
                Box::new(flexcore_control_plane::oidc::OidcResolver::compose(
                    &deployment,
                    Box::new(mapper),
                    clock_millis_closure(clock),
                )
                .map_err(|e| ConfigError::Invalid {
                    name: "FLEXCORE_OIDC_ENABLED",
                    reason: format!("the OIDC identity path could not be composed: {e}"),
                })?)
            }
            None => {
                // The Phase 1 feature-gated posture, byte-identical to the
                // pre-FIM `build_identity_resolver` (dev-auth feature only;
                // release builds deny every request).
                #[cfg(feature = "dev-auth")]
                {
                    Box::new(flexcore_control_plane::auth::DevAuthResolver)
                }
                #[cfg(not(feature = "dev-auth"))]
                {
                    Box::new(flexcore_control_plane::auth::DenyAllResolver)
                }
            }
        };
        // The SCIM wrapper (FIM-F07.02): a provisioned-and-ACTIVE user of
        // the connection's tenant, role from the SCIM group→role mapping.
        match &fim.scim {
            Some(section) => {
                // FIM2-R1-004: the ONE shared store the composition root
                // constructed (build_scim_store) — the same instance
                // build_scim_service receives. A `None` here is a named
                // fail-closed miscomposition, never a silently split store.
                let store: Arc<dyn ScimStore> =
                    scim_store.ok_or_else(|| ConfigError::Invalid {
                        name: "FLEXCORE_SCIM_ENABLED",
                        reason: "the SCIM section is enabled but the shared SCIM store was \
                                 not constructed (the composition root must pass \
                                 fim_compose::build_scim_store's instance to BOTH the \
                                 identity resolver and the SCIM service)"
                            .to_string(),
                    })?;
                Ok(Arc::new(ScimAwareIdentityResolver::new(
                    inner,
                    store,
                    section.tenant,
                )))
            }
            None => Ok(Arc::from(inner)),
        }
    }

    /// The ONE SCIM store of the composition (FIM2-R1-004 of the FIM
    /// final analysis): constructed ONCE here and handed to BOTH the
    /// identity resolver's deprovisioning-enforcement wrap
    /// (`build_identity_resolver`) and the SCIM HTTP surface
    /// (`build_scim_service`), so a subject provisioned or deactivated
    /// through the SCIM connection is visible to identity resolution —
    /// the recorded F-07 composition semantics. The in-memory
    /// implementation is the single-process choice (the Postgres-backed
    /// store is the recorded `UnwiredScimStoreBackend` deployment seam —
    /// `scim_store.rs` honesty header). `None` when no SCIM connection is
    /// configured.
    pub fn build_scim_store(fim: &FimCompose) -> Option<Arc<InMemoryScimStore>> {
        fim.scim.as_ref()?;
        Some(Arc::new(InMemoryScimStore::default()))
    }

    /// The SCIM service + its bearer digest pair (the router mounts ONLY
    /// when both are composed — `routes.rs::fim_router`). The store is the
    /// ONE composition-wide instance (`build_scim_store` — FIM2-R1-004:
    /// the resolver's wrap consumes the SAME store, so deprovisioning
    /// propagates); a SCIM section without it is a named fail-closed
    /// miscomposition.
    pub fn build_scim_service(
        fim: &FimCompose,
        ids: &Arc<dyn IdGenerator>,
        clock: &Arc<dyn Clock>,
        scim_store: Option<Arc<InMemoryScimStore>>,
    ) -> Result<Option<(Arc<ScimService>, [u8; 32])>, ConfigError> {
        let Some(section) = &fim.scim else {
            return Ok(None);
        };
        let store: Arc<dyn ScimStore> = scim_store.ok_or_else(|| ConfigError::Invalid {
            name: "FLEXCORE_SCIM_ENABLED",
            reason: "the SCIM section is enabled but the shared SCIM store was not \
                     constructed (the composition root must pass \
                     fim_compose::build_scim_store's instance to BOTH the identity \
                     resolver and the SCIM service)"
                .to_string(),
        })?;
        let service = Arc::new(ScimService::new(
            store,
            section.tenant,
            ids_string_closure(ids),
            clock_millis_closure(clock),
        ));
        Ok(Some((service, section.bearer_digest)))
    }

    /// The authz policy surface of the shipped binary (FIM2-R1-005 of the
    /// FIM final analysis). The DEFAULT (gate absent or `false`) is EXACTLY
    /// `phase5_default_policies()` — the cumulative phase grant set with
    /// ZERO `apikey.*` grants, so the Wave 8B machine-identity and API-key
    /// admin surfaces (which evaluate `apikey.*` against `state.policies`)
    /// deny for every caller: deny-by-default preserved byte-for-byte
    /// (pinned by evaluation tests below and by the shipped-403 pin in
    /// `tests/fim_identity_routes.rs`).
    ///
    /// With `FLEXCORE_APIKEY_POLICIES_ENABLED=true` the composition grants
    /// the phase-13 `apikey.*` credential slice — the EXACT
    /// (role, action, scope-kind, rule-reference) tuples
    /// `flexcore_authz::phase13_policies()` records for those actions
    /// (PlatformAdmin + TenantAdmin, tenant scope only, the
    /// `policy:phase13:<role>:<action>:tenant` references). The tuples are
    /// re-granted here — `PolicySet` exposes no merge API and this binary
    /// crate may not extend `flexcore-authz` — and the equivalence is
    /// PINNED by evaluation against the authoritative accessor, so drift
    /// between the two surfaces fails at the human gate.
    ///
    /// Deliberately NOT composed: the phase-13 SiteApprover
    /// `command.approve` slice stays unrouted in this binary (no REST
    /// surface evaluates command.approve — the command gates run via
    /// sim-runner; FRV1-R2-009 stands for it), and the market/command-gate
    /// sets are untouched. The never-granted permission set is unaffected:
    /// no apikey grant widens any other action, and the four never-granted
    /// names remain structurally ungrantable.
    pub fn build_policy_set(fim: &FimCompose) -> PolicySet {
        let mut set = phase5_default_policies();
        if fim.apikey_policies {
            // The phase-13 apikey.* slice, grant-for-grant as
            // `flexcore_authz::phase13_policies()` records it (same action
            // order, same roles, same scope kind, same rule-reference
            // format — the equivalence pin asserts the identical evidence).
            for action in [
                Action::ApiKeyCreate,
                Action::ApiKeyRotate,
                Action::ApiKeyRevoke,
                Action::ApiKeyRead,
            ] {
                for role in [Role::PlatformAdmin, Role::TenantAdmin] {
                    set.grant(
                        role,
                        action,
                        ScopeKind::Tenant,
                        EvidenceRef::new(format!(
                            "{PHASE13_POLICY_PREFIX}:{}:{}:tenant",
                            role.id(),
                            action.id()
                        )),
                    );
                }
            }
        }
        set
    }

    /// The step-up service: composed ONLY when the OIDC section is present
    /// (the same IdP backs the fresh-evidence verifier — one IdP, one
    /// configuration). Absent ⇒ `None`, which is exactly the pre-FIM
    /// default (`state.step_up == None`: every step-up request fails
    /// closed). The verifier is `OidcStepUpVerifier` with the registered
    /// defaults (evidence max-age 300 s; empty `required_amr` = the
    /// P13-E-3 DEFAULT's wholesale delegation to the IdP's factor policy;
    /// challenge TTL 5 min). NO APPROVED-gate call site changes: the gates
    /// are not constructed in this binary at all (FRV1-R1-004) — this
    /// composes the SERVICE behind the existing `with_step_up` seam.
    pub fn build_step_up_service(
        fim: &FimCompose,
        ids: &Arc<dyn IdGenerator>,
        clock: &Arc<dyn Clock>,
    ) -> Result<Option<Arc<StepUpService>>, ConfigError> {
        let Some(section) = &fim.oidc else {
            return Ok(None);
        };
        let source = flexcore_control_plane::oidc::HttpJwksSource::new(
            section.issuer.clone(),
            section.jwks_url.clone(),
            section.jwks_fetch_timeout,
        )
        .map_err(|e| ConfigError::Invalid {
            name: "FLEXCORE_OIDC_ENABLED",
            reason: format!("the step-up verifier's JWKS source could not be composed: {e}"),
        })?;
        let verifier = OidcStepUpVerifier::new(
            flexcore_control_plane::oidc::OidcConfig {
                issuer: section.issuer.clone(),
                audience: section.audience.clone(),
                required_client_id: section.expected_client_id.clone(),
                ..flexcore_control_plane::oidc::OidcConfig::default()
            },
            Box::new(source),
            DEFAULT_STEP_UP_EVIDENCE_MAX_AGE,
            // Empty = trust the IdP's factor policy wholesale (the P13-E-3
            // DEFAULT; a deployment that requires named factors composes
            // the list explicitly — never a code default).
            Vec::new(),
            clock_millis_closure(clock),
        );
        let service = StepUpService::new(
            Box::new(InMemoryStepUpStore::default()),
            Box::new(verifier),
            // The challenge ids ride the composed UUIDv7 generator through
            // the stepup module's canonical string form.
            {
                let ids = ids.clone();
                Box::new(move || id_from_uuid(ids.new_uuid()))
            },
            clock_millis_closure(clock),
            DEFAULT_STEP_UP_TTL,
        );
        Ok(Some(Arc::new(service)))
    }

    /// The machine-identity service (FIM-F08.01): composed when the
    /// section is present. The registry is the in-memory implementation
    /// (the Postgres-backed registry is the recorded fail-closed
    /// deployment seam). FIM WAVE 8B: the route surface now serves this
    /// service (`machine_identity_routes.rs`, mounted by
    /// `routes.rs::fim_router` when the state slot is composed) — the
    /// wiring wave's named remaining step; live interop stays the HUMAN
    /// LAB GATE.
    pub fn build_machine_identity_service(
        fim: &FimCompose,
        clock: &Arc<dyn Clock>,
    ) -> Result<Option<Arc<flexcore_control_plane::machine_identity::MachineIdentityService>>, ConfigError>
    {
        let Some(section) = &fim.machine_identity else {
            return Ok(None);
        };
        let config = flexcore_control_plane::machine_identity::MachineIdentityConfig {
            issuer: section.issuer.clone(),
            audience: section.audience.clone(),
            ..flexcore_control_plane::machine_identity::MachineIdentityConfig::default()
        };
        Ok(Some(Arc::new(
            flexcore_control_plane::machine_identity::MachineIdentityService::new(
                config,
                section.issuer_secret.as_bytes(),
                Box::new(flexcore_control_plane::machine_identity::MemoryMachineClientStore::new()),
                clock_millis_closure(clock),
            ),
        )))
    }

    /// The API-key service (FIM-F08.02): composed when the section is
    /// present, over the in-memory store (the Postgres-backed store is the
    /// recorded fail-closed deployment seam). FIM WAVE 8B: the route
    /// surface now serves this service (`api_key_routes.rs`, mounted by
    /// `routes.rs::fim_router` when the state slot is composed) — same
    /// named remaining step as the machine-identity service; live interop
    /// stays the HUMAN LAB GATE.
    pub fn build_api_keys_service(
        fim: &FimCompose,
        clock: &Arc<dyn Clock>,
    ) -> Result<Option<Arc<flexcore_control_plane::api_keys::ApiKeyService>>, ConfigError> {
        let Some(section) = &fim.api_keys else {
            return Ok(None);
        };
        Ok(Some(Arc::new(
            flexcore_control_plane::api_keys::ApiKeyService::new(
                flexcore_control_plane::api_keys::ApiKeyConfig {
                    rotation_grace_seconds: section.rotation_grace_seconds,
                },
                Box::new(flexcore_control_plane::api_keys::MemoryApiKeyStore::new()),
                clock_millis_closure(clock),
            ),
        )))
    }

    /// The OTA rollout coordinator (FIM-F12.05): composed when the section
    /// is present, over the plan built from the parsed definitions (the
    /// orchestrator's fail-closed plan validation applies unchanged). The
    /// audit sinks are the IN-MEMORY implementations (the orchestrator's
    /// `MemoryRolloutAuditSink` + the coordinator's
    /// `MemoryOtaRolloutAuditSink`) — the durable Postgres audit-sink
    /// wiring is a named remaining step, so the audit records live
    /// in-process only. The clock is the wall clock (the coordinator's
    /// injected-clock rule is about determinism of DECISIONS, which read
    /// the clock through the port, never their own wall read).
    pub fn build_ota_rollout(
        fim: &FimCompose,
    ) -> Result<Option<Arc<std::sync::Mutex<OtaRolloutCoordinator>>>, ConfigError> {
        let Some(section) = &fim.ota else {
            return Ok(None);
        };
        let chain_key = ChainKey::from_bytes(&section.evidence_chain_key).map_err(|e| {
            ConfigError::Invalid {
                name: "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX",
                reason: format!("the evidence-chain key was refused: {e}"),
            }
        })?;
        let (plan, definitions) = build_plan(
            &section.target_version,
            section.cohorts.clone(),
            AbortThresholds {
                max_failure_rate_permille: section.max_failure_rate_permille,
                min_observations: section.min_observations,
            },
            section.canary_basis_points,
        )
        .map_err(|e| ConfigError::Invalid {
            name: "FLEXCORE_OTA_ROLLOUT_ENABLED",
            reason: format!("the OTA rollout plan was refused: {e}"),
        })?;
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(flexcore_postgres_adapter::SystemClock);
        let coordinator = OtaRolloutCoordinator::new(
            plan,
            definitions,
            Box::new(Arc::new(MemoryRolloutAuditSink::new())),
            Arc::new(MemoryOtaRolloutAuditSink::new()),
            chain_key,
            clock,
        );
        // The receiver port takes `&mut self` (a state machine over
        // recorded events), so the composition holds it behind a mutex.
        Ok(Some(Arc::new(std::sync::Mutex::new(coordinator))))
    }

    // --- The C_degradation composition (FIM-F11) ------------------------------

    // RECORDED POSTURE (verified by inspection at authoring time, not
    // guessed): THIS BINARY HAS NO OPTIMIZER CALL PATH. No construction of
    // `flexcore_application::scheduling_objective::ObjectiveInputs` exists
    // anywhere under apps/control-plane (grep-verified); the §12.6
    // optimization runs are SUBMITTED as candidates through
    // /api/v1/optimization-runs and evaluated by the sim-runner program,
    // not here. The mapping below therefore composes the DATA HALF only:
    // the domain-side P15.08 input (`BessDegradationCostInput`) mapped onto
    // the exact field set of the optimizer declaration
    // (`flexcore-optimization-api::degradation_cost::
    // DegradationCostDeclaration`) that
    // `ObjectiveInputs.degradation_cost_curves` consumes.
    //
    // THE ONE REMAINING TYPE-LEVEL STEP (constructing
    // `DegradationCostDeclaration` and assigning the vec) composes at the
    // site that assembles `ObjectiveInputs` — the future optimization
    // assembly — because:
    // 1. `flexcore-optimization-api` is not a direct dependency of this
    //    binary (apps/control-plane's manifest was outside this wave's
    //    ownership) and `flexcore-application` does not re-export the type
    //    (its `use` is private), so the type is not nameable here;
    // 2. the assembly site does not exist here anyway (point 0).
    // That step is mechanical and pinned by the mirror contract:
    // `DegradationChemistry` → `DegradationChemistryLabel` and
    // `BessDegradationCurveShape` → `DegradationCostCurve` are
    // FIELD-FOR-FIELD mirrors by citation (the bess_cost_curve.rs header),
    // and `DegradationCostDeclaration::new(asset_id, curve_version,
    // chemistry, curve, provenance_note)` validates the rest.
    //
    // THE ASSET-CONFIG SOURCE is the recorded remaining step beyond the
    // type step: per-asset curve declarations arrive from the asset
    // registry's capability-profile data, whose reader this binary does
    // not compose yet (the projection builder
    // `bess_cost_curve::bess_degradation_cost_input` produces the domain
    // input; where its per-asset configuration comes from is the
    // deployment's recorded decision).

    /// The mapped field set of one optimizer degradation-cost declaration.
    /// The two structural fields carry the DOMAIN mirror values (field-for-
    /// field mirrors by citation — see the recorded posture above); the
    /// future assembly site converts them into the optimizer's types and
    /// calls `DegradationCostDeclaration::new`.
    // dead_code: the optimizer assembly site does not exist in this binary
    // yet (the recorded posture above); the mapping is composed NOW, pinned
    // by the unit tests below, so the data half lands at the composition
    // root instead of being invented later at a call site.
    #[allow(dead_code)]
    pub struct MappedDegradationCostDeclaration {
        /// The asset the curve applies to — the REGISTRY REFERENCE STRING
        /// FORM (the canonical hyphenated UUID, the same rendering the
        /// registry DTO layer uses; `routes.rs::map_asset`).
        pub asset_id: String,
        pub curve_version: String,
        /// The declared chemistry scope (the domain mirror value; the
        /// one-step conversion is the recorded future assembly step).
        pub chemistry: DegradationChemistry,
        /// The declared curve (the domain mirror value; same note).
        pub curve: BessDegradationCurveShape,
        pub provenance_note: String,
        /// The HUMAN-LAB-GATE flag (passes through unchanged; the assembly
        /// site must carry it into its own provenance record).
        pub parameters_unverified_secondary: bool,
    }

    /// The registry reference string form of an asset id (the canonical
    /// hyphenated UUID — the form every registry DTO and the optimization
    /// records render).
    // dead_code: see the recorded posture above (composed ahead of the
    // optimizer assembly site; pinned by the unit tests).
    #[allow(dead_code)]
    pub fn asset_registry_reference(asset: AssetId) -> String {
        asset.uuid().to_hyphenated()
    }

    /// Maps one domain-side P15.08 degradation-cost input onto the field
    /// set of the optimizer declaration the `C_degradation` term consumes
    /// (`ObjectiveInputs.degradation_cost_curves`). Pure, total, no
    /// validation DUPLICATED: the domain builder
    /// (`bess_degradation_cost_input`) already enforces the version/note
    /// bounds, and `DegradationCostDeclaration::new` (at the future
    /// assembly site) re-validates against the optimizer's own contract —
    /// single-home discipline, never restated here.
    // dead_code: see the recorded posture above (composed ahead of the
    // optimizer assembly site; pinned by the unit tests).
    #[allow(dead_code)]
    pub fn map_bess_degradation_cost_input(
        input: &BessDegradationCostInput,
    ) -> MappedDegradationCostDeclaration {
        MappedDegradationCostDeclaration {
            asset_id: asset_registry_reference(input.asset_id),
            curve_version: input.curve_version.clone(),
            chemistry: input.chemistry.clone(),
            curve: input.curve.clone(),
            provenance_note: input.provenance_note.clone(),
            parameters_unverified_secondary: input.parameters_unverified_secondary,
        }
    }

    // --- Unit tests (deterministic, socket-free; run at the human gate) ------

    #[cfg(test)]
    mod tests {
        use super::*;
        use flexcore_authz::scope::{AssetScope, Scope, SiteScope, TenantScope};
        use flexcore_control_plane::auth::{AuthError, Caller};
        use flexcore_control_plane::scim_store::{NewScimGroup, NewScimUser};
        use flexcore_domain::ids::SiteId;
        use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
        use flexcore_ports::fakes::{CountingIdGenerator, FixedClock};

        const TEST_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
        const TEST_ASSET_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8b02";
        const TEST_SITE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8c03";

        fn table(entries: Vec<(&'static str, &'static str)>) -> impl Fn(&str) -> Option<String> {
            let map: HashMap<String, String> = entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            move |name: &str| map.get(name).cloned()
        }

        fn test_clock() -> Arc<dyn Clock> {
            Arc::new(FixedClock::at_unix_millis(1_700_000_000_000))
        }

        fn test_ids() -> Arc<dyn IdGenerator> {
            Arc::new(CountingIdGenerator::new())
        }

        // --- Absent configuration: every default -------------------------------

        #[test]
        fn absent_configuration_yields_every_default_section_none() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            assert!(fim.webhook_http.is_none());
            assert!(fim.oidc.is_none());
            assert!(fim.scim.is_none());
            assert!(fim.machine_identity.is_none());
            assert!(fim.api_keys.is_none());
            assert!(!fim.apikey_policies, "the apikey-policies gate defaults OFF");
            assert!(fim.ota.is_none());
        }

        #[cfg(not(feature = "dev-auth"))]
        #[test]
        fn absent_configuration_builds_the_deny_all_identity() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            let resolver =
                build_identity_resolver(&fim, &test_clock(), None).expect("the default composes");
            // DenyAllResolver's exact recorded behavior: EVERY request is
            // refused with MissingCredentials, with or without a bearer.
            assert!(matches!(
                resolver.resolve(None),
                Err(AuthError::MissingCredentials)
            ));
            assert!(matches!(
                resolver.resolve(Some("Bearer anything")),
                Err(AuthError::MissingCredentials)
            ));
        }

        #[test]
        fn absent_configuration_keeps_the_unwired_webhook_transport() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            let transport =
                build_webhook_transport(&fim).expect("the default composes");
            let refused = transport
                .post("https://hooks.example/endpoint", &[], b"{}".as_slice())
                .expect_err("the unwired seam refuses every transmission");
            match refused {
                flexcore_control_plane::webhook_service::WebhookDeliveryError::Backend {
                    code,
                    ..
                } => assert_eq!(code, "E_TRANSPORT_UNWIRED"),
                other => panic!("expected the unwired backend refusal, got {other:?}"),
            }
        }

        #[test]
        fn absent_configuration_keeps_no_step_up_service() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            let service = build_step_up_service(&fim, &test_ids(), &test_clock())
                .expect("the default composes");
            assert!(service.is_none(), "absent OIDC ⇒ no step-up service (None = the fail-closed default)");
        }

        #[test]
        fn absent_configuration_builds_no_scim_machine_identity_api_keys_or_ota() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            let ids = test_ids();
            let clock = test_clock();
            assert!(build_scim_service(&fim, &ids, &clock, None)
                .expect("the default composes")
                .is_none());
            assert!(build_machine_identity_service(&fim, &clock)
                .expect("the default composes")
                .is_none());
            assert!(build_api_keys_service(&fim, &clock)
                .expect("the default composes")
                .is_none());
            assert!(build_ota_rollout(&fim).expect("the default composes").is_none());
        }

        // --- The gate flag ------------------------------------------------------

        #[test]
        fn the_enable_flag_is_strict() {
            let on = table(vec![("FLEXCORE_WEBHOOK_HTTP_ENABLED", "true")]);
            assert!(parse_webhook_http(&on).expect("parses").is_some());
            let on_upper = table(vec![("FLEXCORE_WEBHOOK_HTTP_ENABLED", "TRUE")]);
            assert!(parse_webhook_http(&on_upper).expect("parses").is_some());
            let off = table(vec![("FLEXCORE_WEBHOOK_HTTP_ENABLED", "false")]);
            assert!(parse_webhook_http(&off).expect("parses").is_none());
            let junk = table(vec![("FLEXCORE_WEBHOOK_HTTP_ENABLED", "maybe")]);
            assert!(parse_webhook_http(&junk).is_err(), "a non-bool gate is a named error");
        }

        // --- Webhook HTTP section ----------------------------------------------

        #[test]
        fn webhook_http_enabled_uses_the_registered_timeout_defaults() {
            let lookup = table(vec![("FLEXCORE_WEBHOOK_HTTP_ENABLED", "true")]);
            let section = parse_webhook_http(&lookup)
                .expect("parses")
                .expect("enabled");
            assert_eq!(
                section,
                WebhookHttpSection {
                    connect_timeout_secs: CONNECT_TIMEOUT_SECONDS_DEFAULT,
                    total_timeout_secs: TOTAL_TIMEOUT_SECONDS_DEFAULT,
                }
            );
        }

        #[test]
        fn webhook_http_enabled_constructs_the_live_transport() {
            let fim = FimCompose {
                webhook_http: Some(WebhookHttpSection {
                    connect_timeout_secs: CONNECT_TIMEOUT_SECONDS_DEFAULT,
                    total_timeout_secs: TOTAL_TIMEOUT_SECONDS_DEFAULT,
                }),
                oidc: None,
                scim: None,
                machine_identity: None,
                api_keys: None,
                apikey_policies: false,
                ota: None,
            };
            // Construction builds the reqwest client and contacts NOTHING
            // (the first post is the first transmission — the live fetch
            // path is the HUMAN LAB GATE).
            let transport =
                build_webhook_transport(&fim).expect("the live transport constructs");
            // The Arc holds the LIVE type (construction is the pin; no
            // post is attempted — a socket-free test).
            let _ = transport;
        }

        // --- OIDC section -------------------------------------------------------

        fn oidc_entries() -> Vec<(&'static str, &'static str)> {
            vec![
                ("FLEXCORE_OIDC_ENABLED", "true"),
                ("FLEXCORE_OIDC_ISSUER", "https://idp.example/realms/flexcore"),
                ("FLEXCORE_OIDC_JWKS_URL", "https://idp.example/realms/flexcore/protocol/openid-connect/certs"),
                ("FLEXCORE_OIDC_AUDIENCE", "flexcore-control-plane"),
                (
                    "FLEXCORE_OIDC_TENANT_BINDINGS",
                    "https://idp.example/realms/flexcore=0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01",
                ),
            ]
        }

        #[test]
        fn oidc_section_parses_with_the_registered_fetch_default() {
            let lookup = table(oidc_entries());
            let section = parse_oidc(&lookup).expect("parses").expect("enabled");
            assert_eq!(section.issuer, "https://idp.example/realms/flexcore");
            assert_eq!(section.audience, "flexcore-control-plane");
            assert_eq!(section.expected_client_id, None);
            assert_eq!(
                section.jwks_fetch_timeout,
                flexcore_control_plane::oidc::DEFAULT_JWKS_FETCH_TIMEOUT
            );
            assert_eq!(section.tenant_bindings.len(), 1);
        }

        #[test]
        fn oidc_requires_issuer_jwks_url_and_audience() {
            for (name, entries) in [
                ("issuer", vec![("FLEXCORE_OIDC_ENABLED", "true"), ("FLEXCORE_OIDC_JWKS_URL", "https://idp.example/certs"), ("FLEXCORE_OIDC_AUDIENCE", "aud")]),
                ("jwks", vec![("FLEXCORE_OIDC_ENABLED", "true"), ("FLEXCORE_OIDC_ISSUER", "https://idp.example"), ("FLEXCORE_OIDC_AUDIENCE", "aud")]),
                ("audience", vec![("FLEXCORE_OIDC_ENABLED", "true"), ("FLEXCORE_OIDC_ISSUER", "https://idp.example"), ("FLEXCORE_OIDC_JWKS_URL", "https://idp.example/certs")]),
            ] {
                let lookup = table(entries);
                let err = parse_oidc(&lookup).expect_err("a required value is missing");
                assert!(
                    matches!(err, ConfigError::Missing { .. }),
                    "{name} absence is a named missing error"
                );
            }
        }

        #[test]
        fn oidc_jwks_url_must_be_https() {
            let mut entries = oidc_entries();
            entries[2] = ("FLEXCORE_OIDC_JWKS_URL", "http://idp.example/certs");
            let lookup = table(entries);
            let err = parse_oidc(&lookup).expect_err("a plain-http JWKS URL is refused");
            assert!(matches!(err, ConfigError::Invalid { name: "FLEXCORE_OIDC_JWKS_URL", .. }));
        }

        #[test]
        fn oidc_tenant_bindings_parse_canonical_uuids_and_refuse_junk() {
            let mut entries = oidc_entries();
            entries[4] = ("FLEXCORE_OIDC_TENANT_BINDINGS", "issuer=nope");
            let lookup = table(entries);
            assert!(parse_oidc(&lookup).is_err(), "a non-uuid tenant is refused");
        }

        #[cfg(not(feature = "dev-auth"))]
        #[test]
        fn oidc_composition_rejects_malformed_tokens_before_any_fetch() {
            let lookup = table(oidc_entries());
            let section = parse_oidc(&lookup).expect("parses").expect("enabled");
            let fim = FimCompose {
                oidc: Some(section),
                webhook_http: None,
                scim: None,
                machine_identity: None,
                api_keys: None,
                apikey_policies: false,
                ota: None,
            };
            let resolver =
                build_identity_resolver(&fim, &test_clock(), None).expect("the OIDC path composes");
            // A bearer-shaped but non-JWT credential is refused at header
            // decode — BEFORE any JWKS fetch (socket-free).
            assert!(matches!(
                resolver.resolve(Some("Bearer not-a-jwt")),
                Err(AuthError::InvalidCredentials)
            ));
        }

        // --- SCIM section -------------------------------------------------------

        #[test]
        fn scim_section_requires_tenant_and_bearer_and_carries_only_the_digest() {
            let lookup = table(vec![
                ("FLEXCORE_SCIM_ENABLED", "true"),
                ("FLEXCORE_SCIM_TENANT_ID", TEST_TENANT_UUID),
                ("FLEXCORE_SCIM_BEARER_TOKEN", "scim-connection-secret"),
            ]);
            let section = parse_scim(&lookup).expect("parses").expect("enabled");
            assert_eq!(
                section.tenant,
                TenantId::new(FlexUuid::parse(TEST_TENANT_UUID).expect("valid"))
            );
            // ONLY the digest travels (never the token).
            assert_eq!(
                section.bearer_digest,
                flexcore_control_plane::webhook_signing::sha256(b"scim-connection-secret")
            );
        }

        #[cfg(not(feature = "dev-auth"))]
        #[test]
        fn scim_wrap_denies_when_the_inner_resolver_denies() {
            let lookup = table(vec![
                ("FLEXCORE_SCIM_ENABLED", "true"),
                ("FLEXCORE_SCIM_TENANT_ID", TEST_TENANT_UUID),
                ("FLEXCORE_SCIM_BEARER_TOKEN", "scim-connection-secret"),
            ]);
            let section = parse_scim(&lookup).expect("parses").expect("enabled");
            let fim = FimCompose {
                scim: Some(section),
                webhook_http: None,
                oidc: None,
                machine_identity: None,
                api_keys: None,
                apikey_policies: false,
                ota: None,
            };
            // The composition-wide store (FIM2-R1-004): the wrap consumes
            // the SAME instance the SCIM service receives.
            let store = build_scim_store(&fim).expect("the SCIM section builds the shared store");
            let resolver = build_identity_resolver(&fim, &test_clock(), Some(store))
                .expect("the SCIM wrap composes");
            // The inner DenyAllResolver denies BEFORE any SCIM state is
            // consulted — the wrap never widens the shipped default.
            assert!(matches!(
                resolver.resolve(Some("Bearer anything")),
                Err(AuthError::MissingCredentials)
            ));
        }

        // --- The shared SCIM store (FIM2-R1-004) --------------------------------

        #[test]
        fn the_scim_store_is_constructed_only_when_the_section_is_present() {
            let empty = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            assert!(
                build_scim_store(&empty).is_none(),
                "no SCIM section ⇒ no store (the absent-configuration default)"
            );
            let lookup = table(vec![
                ("FLEXCORE_SCIM_ENABLED", "true"),
                ("FLEXCORE_SCIM_TENANT_ID", TEST_TENANT_UUID),
                ("FLEXCORE_SCIM_BEARER_TOKEN", "scim-connection-secret"),
            ]);
            let fim = FimCompose::from_lookup(&lookup).expect("parses");
            assert!(
                build_scim_store(&fim).is_some(),
                "a SCIM section builds the ONE composition-wide store"
            );
        }

        #[test]
        fn a_scim_section_without_the_shared_store_is_a_named_miscomposition() {
            let lookup = table(vec![
                ("FLEXCORE_SCIM_ENABLED", "true"),
                ("FLEXCORE_SCIM_TENANT_ID", TEST_TENANT_UUID),
                ("FLEXCORE_SCIM_BEARER_TOKEN", "scim-connection-secret"),
            ]);
            let fim = FimCompose::from_lookup(&lookup).expect("parses");
            // BOTH consumers refuse: the FIM2-R1-004 defect shape (each
            // builder constructing its own store) is structurally
            // unreachable — a composition can no longer split the store
            // silently.
            let resolver_err =
                build_identity_resolver(&fim, &test_clock(), None).expect_err("miscomposed");
            assert!(matches!(
                resolver_err,
                ConfigError::Invalid { name: "FLEXCORE_SCIM_ENABLED", .. }
            ));
            let service_err = build_scim_service(&fim, &test_ids(), &test_clock(), None)
                .expect_err("miscomposed");
            assert!(matches!(
                service_err,
                ConfigError::Invalid { name: "FLEXCORE_SCIM_ENABLED", .. }
            ));
        }

        /// The inner identity path stood in by an ALLOWING fake: the
        /// production inner (the OIDC path) refuses unknown tokens before
        /// any SCIM state is consulted, which would hide the store from
        /// this pin; the fake lets the deprovisioning check run (the real
        /// inner path against a live IdP is the recorded HUMAN LAB GATE).
        struct AllowingResolver {
            tenant: TenantId,
        }

        impl IdentityResolver for AllowingResolver {
            fn resolve(&self, _bearer: Option<&str>) -> Result<Caller, AuthError> {
                let ctx = TenantContext::from_authenticated_claims(
                    AuthenticatedClaims::from_verified(VerifiedClaims {
                        tenant_id: self.tenant,
                        subject: "alice".to_string(),
                    }),
                );
                Ok(Caller {
                    ctx,
                    role: Role::Viewer,
                })
            }
        }

        #[test]
        fn the_identity_wrap_and_the_scim_surface_share_one_store() {
            let lookup = table(vec![
                ("FLEXCORE_SCIM_ENABLED", "true"),
                ("FLEXCORE_SCIM_TENANT_ID", TEST_TENANT_UUID),
                ("FLEXCORE_SCIM_BEARER_TOKEN", "scim-connection-secret"),
            ]);
            let fim = FimCompose::from_lookup(&lookup).expect("parses");
            let tenant = fim.scim.as_ref().expect("the section").tenant;
            let store = build_scim_store(&fim).expect("the section builds the shared store");
            // The SCIM-surface half: the REAL composition builder over the
            // shared handle.
            let (service, _digest) =
                build_scim_service(&fim, &test_ids(), &test_clock(), Some(store.clone()))
                    .expect("composes")
                    .expect("enabled ⇒ Some");
            assert_eq!(service.tenant(), tenant);
            // The resolver half: EXACTLY the construction
            // build_identity_resolver performs over the shared store (the
            // inner identity path stood in by the allowing fake).
            let resolver = ScimAwareIdentityResolver::new(
                Box::new(AllowingResolver { tenant }),
                store.clone(),
                tenant,
            );
            // Provision through the SHARED store (the same instance the
            // service was built over): a mapped group first — the user's
            // membership must reference an existing group — then the active
            // user.
            store
                .create_group(
                    &tenant,
                    NewScimGroup {
                        id: "g-1".to_string(),
                        display_name: "Operators".to_string(),
                        external_id: None,
                        member_user_ids: Vec::new(),
                        mapped_role_id: Some("operator".to_string()),
                    },
                    1_000,
                )
                .expect("group created");
            store
                .create_user(
                    &tenant,
                    NewScimUser {
                        id: "u-1".to_string(),
                        user_name: "alice".to_string(),
                        display_name: None,
                        external_id: None,
                        active: true,
                        group_ids: vec!["g-1".to_string()],
                    },
                    1_000,
                )
                .expect("user created");
            // The provisioned subject RESOLVES through the shared store —
            // and the role is the SCIM mapping's (the inner's role is
            // REPLACED), proving the store lookup ran on this instance.
            let caller = resolver
                .resolve(Some("Bearer test"))
                .expect("the provisioned subject resolves through the SHARED store");
            assert!(
                matches!(caller.role, Role::Operator),
                "the role is the SCIM group→role mapping's, not the inner's"
            );
            // Deactivation through the SAME store propagates to resolution:
            // the recorded F-07 deprovisioning semantics, now actually
            // composed (FIM2-R1-004).
            store
                .deactivate_user(&tenant, "u-1", 2_000)
                .expect("deactivated");
            assert!(
                matches!(
                    resolver.resolve(Some("Bearer test")),
                    Err(AuthError::InvalidCredentials)
                ),
                "a deactivated subject denies at the resolver (fail closed)",
            );
        }

        // --- Machine identity / API keys sections -------------------------------

        #[test]
        fn machine_identity_section_requires_issuer_audience_and_secret() {
            let lookup = table(vec![
                ("FLEXCORE_MACHINE_IDENTITY_ENABLED", "true"),
                ("FLEXCORE_MACHINE_IDENTITY_ISSUER", "https://machines.example"),
                ("FLEXCORE_MACHINE_IDENTITY_AUDIENCE", "flexcore-control-plane"),
                ("FLEXCORE_MACHINE_IDENTITY_ISSUER_SECRET", "issuer-key-material"),
            ]);
            let section = parse_machine_identity(&lookup)
                .expect("parses")
                .expect("enabled");
            assert_eq!(section.issuer, "https://machines.example");
            assert_eq!(section.audience, "flexcore-control-plane");
            assert_eq!(section.issuer_secret, "issuer-key-material");

            let missing = table(vec![
                ("FLEXCORE_MACHINE_IDENTITY_ENABLED", "true"),
                ("FLEXCORE_MACHINE_IDENTITY_ISSUER", "https://machines.example"),
                ("FLEXCORE_MACHINE_IDENTITY_AUDIENCE", "flexcore-control-plane"),
            ]);
            let err = parse_machine_identity(&missing).expect_err("the secret is required");
            assert!(matches!(err, ConfigError::Missing { .. }));
        }

        #[test]
        fn machine_identity_and_api_keys_services_construct_when_enabled() {
            let machine = MachineIdentitySection {
                issuer: "https://machines.example".to_string(),
                audience: "flexcore-control-plane".to_string(),
                issuer_secret: "issuer-key-material".to_string(),
            };
            let api_keys = ApiKeysSection {
                rotation_grace_seconds: 3_600,
            };
            let fim = FimCompose {
                machine_identity: Some(machine),
                api_keys: Some(api_keys),
                webhook_http: None,
                oidc: None,
                scim: None,
                apikey_policies: false,
                ota: None,
            };
            let clock = test_clock();
            let machine_service = build_machine_identity_service(&fim, &clock)
                .expect("composes")
                .expect("enabled ⇒ Some");
            let api_service = build_api_keys_service(&fim, &clock)
                .expect("composes")
                .expect("enabled ⇒ Some");
            // Construction is the pin (both services are sealed types; no
            // route serves them — the named remaining wiring step).
            let _ = (machine_service, api_service);
        }

        #[test]
        fn api_keys_section_defaults_to_the_registered_grace_window() {
            let lookup = table(vec![("FLEXCORE_API_KEYS_ENABLED", "true")]);
            let section = parse_api_keys(&lookup).expect("parses").expect("enabled");
            assert_eq!(
                section.rotation_grace_seconds,
                flexcore_control_plane::api_keys::ROTATION_GRACE_SECONDS_DEFAULT
            );
        }

        // --- The apikey-policies gate (FIM2-R1-005) ------------------------------

        #[test]
        fn the_apikey_policies_gate_defaults_off_and_is_strict() {
            assert!(
                !FimCompose::from_lookup(&|_| None)
                    .expect("an empty table always parses")
                    .apikey_policies,
                "absent gate ⇒ OFF (deny-by-default preserved)"
            );
            let on = table(vec![("FLEXCORE_APIKEY_POLICIES_ENABLED", "true")]);
            assert!(parse_apikey_policies(&on).expect("parses"));
            let on_upper = table(vec![("FLEXCORE_APIKEY_POLICIES_ENABLED", "TRUE")]);
            assert!(parse_apikey_policies(&on_upper).expect("parses"));
            let off = table(vec![("FLEXCORE_APIKEY_POLICIES_ENABLED", "false")]);
            assert!(!parse_apikey_policies(&off).expect("parses"));
            let junk = table(vec![("FLEXCORE_APIKEY_POLICIES_ENABLED", "maybe")]);
            assert!(
                parse_apikey_policies(&junk).is_err(),
                "a non-bool gate is a named error"
            );
        }

        fn test_tenant_id() -> TenantId {
            TenantId::new(FlexUuid::parse(TEST_TENANT_UUID).expect("valid test uuid"))
        }

        /// The claims-derived tenant context (the ONE public construction
        /// path — P0.16 rule 5; a fixed test identity, never a principal).
        fn test_context() -> TenantContext {
            TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
                VerifiedClaims {
                    tenant_id: test_tenant_id(),
                    subject: "fim-compose-policy-test".to_string(),
                },
            ))
        }

        #[test]
        fn the_absent_gate_composes_exactly_the_phase5_policy_set() {
            let fim = FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            // EXACTLY the shipped set (PolicySet equality).
            assert_eq!(build_policy_set(&fim), phase5_default_policies());
            // And the shipped deny for the Wave 8B admin surfaces' action:
            // NO role carries an apikey.create grant at tenant scope.
            let context = test_context();
            let tenant_scope = Scope::Tenant(TenantScope::new(test_tenant_id()));
            for role in [
                Role::PlatformAdmin,
                Role::TenantAdmin,
                Role::Operator,
                Role::Viewer,
            ] {
                assert!(
                    !build_policy_set(&fim)
                        .evaluate(role, Action::ApiKeyCreate, Some(&tenant_scope), Some(&context))
                        .is_allowed(),
                    "{role:?} carries no apikey.create grant under the shipped default",
                );
            }
        }

        #[test]
        fn the_apikey_gate_composes_exactly_the_phase13_credential_grants() {
            let mut fim =
                FimCompose::from_lookup(&|_| None).expect("an empty table always parses");
            fim.apikey_policies = true;
            let composed = build_policy_set(&fim);
            // The AUTHORITATIVE phase-13 grant set: every composed apikey.*
            // decision must be identical — the same allow/deny AND the same
            // rule-reference evidence (the equivalence pin that makes the
            // re-granted slice drift-proof).
            let recorded = flexcore_authz::phase13_policies::phase13_policies();
            let context = test_context();
            let tenant_scope = Scope::Tenant(TenantScope::new(test_tenant_id()));
            for role in [
                Role::PlatformAdmin,
                Role::TenantAdmin,
                Role::Operator,
                Role::Viewer,
                Role::SiteApprover,
            ] {
                for action in [
                    Action::ApiKeyCreate,
                    Action::ApiKeyRotate,
                    Action::ApiKeyRevoke,
                    Action::ApiKeyRead,
                ] {
                    let mine =
                        composed.evaluate(role, action, Some(&tenant_scope), Some(&context));
                    let theirs =
                        recorded.evaluate(role, action, Some(&tenant_scope), Some(&context));
                    assert_eq!(
                        mine.is_allowed(),
                        theirs.is_allowed(),
                        "the composed apikey grant for {role:?}/{:?} disagrees with phase13",
                        action.id()
                    );
                    assert_eq!(
                        mine.evidence().as_str(),
                        theirs.evidence().as_str(),
                        "the composed apikey grant for {role:?}/{:?} must carry the recorded \
                         rule reference",
                        action.id()
                    );
                }
            }
            // The deliberate exclusion (FRV1-R2-009): the phase-13
            // SiteApprover command.approve slice is NOT composed — the
            // authoritative set allows it at site scope, the composed
            // surface denies exactly as the shipped default did.
            let site_scope = Scope::Site(SiteScope::new(
                test_tenant_id(),
                SiteId::new(FlexUuid::parse(TEST_SITE_UUID).expect("valid test uuid")),
            ));
            assert!(recorded
                .evaluate(
                    Role::SiteApprover,
                    Action::CommandApprove,
                    Some(&site_scope),
                    Some(&context)
                )
                .is_allowed());
            assert!(
                !composed
                    .evaluate(
                        Role::SiteApprover,
                        Action::CommandApprove,
                        Some(&site_scope),
                        Some(&context)
                    )
                    .is_allowed(),
                "the SiteApprover command.approve slice stays unrouted (FRV1-R2-009)",
            );
            // The gate widens NOTHING else: a phase-1 grant is unaffected…
            let asset_scope = Scope::Asset(AssetScope::new(
                test_tenant_id(),
                AssetId::new(FlexUuid::parse(TEST_ASSET_UUID).expect("valid test uuid")),
            ));
            assert!(composed
                .evaluate(
                    Role::TenantAdmin,
                    Action::AssetRead,
                    Some(&asset_scope),
                    Some(&context)
                )
                .is_allowed());
            // …and the never-granted names stay structurally ungrantable
            // (the frozen invariant, re-asserted at the composition root).
            assert_eq!(Action::parse("dispatch.authorize"), None);
        }

        // --- OTA section --------------------------------------------------------

        const CHAIN_KEY_HEX: &str =
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

        fn ota_entries() -> Vec<(&'static str, &'static str)> {
            vec![
                ("FLEXCORE_OTA_ROLLOUT_ENABLED", "true"),
                ("FLEXCORE_OTA_TARGET_VERSION", "edge-2.0.0"),
                ("FLEXCORE_OTA_COHORTS", "canary:d1,d2;block-1:d3"),
                ("FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE", "200"),
                ("FLEXCORE_OTA_MIN_OBSERVATIONS", "2"),
                ("FLEXCORE_OTA_CANARY_BASIS_POINTS", "500"),
                ("FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX", CHAIN_KEY_HEX),
            ]
        }

        #[test]
        fn ota_section_parses_cohorts_thresholds_and_chain_key() {
            let lookup = table(ota_entries());
            let section = parse_ota(&lookup).expect("parses").expect("enabled");
            assert_eq!(section.target_version, "edge-2.0.0");
            assert_eq!(section.cohorts.len(), 2);
            // The FIRST cohort is the canary; nobody else is.
            assert!(section.cohorts[0].canary);
            assert!(!section.cohorts[1].canary);
            assert_eq!(section.cohorts[0].device_ids, vec!["d1", "d2"]);
            assert_eq!(section.cohorts[1].device_ids, vec!["d3"]);
            assert_eq!(section.max_failure_rate_permille, 200);
            assert_eq!(section.min_observations, 2);
            assert_eq!(section.canary_basis_points, 500);
            assert_eq!(section.evidence_chain_key.len(), 32);
        }

        #[test]
        fn ota_section_requires_the_thresholds_and_a_long_enough_chain_key() {
            for missing in [
                "FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE",
                "FLEXCORE_OTA_MIN_OBSERVATIONS",
                "FLEXCORE_OTA_CANARY_BASIS_POINTS",
                "FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX",
            ] {
                let entries: Vec<(&'static str, &'static str)> = ota_entries()
                    .into_iter()
                    .filter(|(name, _)| *name != missing)
                    .collect();
                let lookup = table(entries);
                let err = parse_ota(&lookup).expect_err("a required value is missing");
                assert!(matches!(err, ConfigError::Missing { .. }), "{missing} is required");
            }
            let short = table(vec![
                ("FLEXCORE_OTA_ROLLOUT_ENABLED", "true"),
                ("FLEXCORE_OTA_TARGET_VERSION", "edge-2.0.0"),
                ("FLEXCORE_OTA_COHORTS", "canary:d1"),
                ("FLEXCORE_OTA_MAX_FAILURE_RATE_PERMILLE", "200"),
                ("FLEXCORE_OTA_MIN_OBSERVATIONS", "2"),
                ("FLEXCORE_OTA_CANARY_BASIS_POINTS", "500"),
                ("FLEXCORE_OTA_EVIDENCE_CHAIN_KEY_HEX", "00ff"),
            ]);
            let err = parse_ota(&short).expect_err("a too-short chain key is refused");
            assert!(matches!(err, ConfigError::Invalid { .. }));
        }

        #[test]
        fn ota_coordinator_constructs_when_enabled() {
            let lookup = table(ota_entries());
            let section = parse_ota(&lookup).expect("parses").expect("enabled");
            let fim = FimCompose {
                ota: Some(section),
                webhook_http: None,
                oidc: None,
                scim: None,
                machine_identity: None,
                api_keys: None,
                apikey_policies: false,
            };
            let coordinator = build_ota_rollout(&fim)
                .expect("composes")
                .expect("enabled ⇒ Some");
            let guard = coordinator.lock().expect("unpoisoned test mutex");
            assert_eq!(guard.target_version(), "edge-2.0.0");
        }

        // --- The C_degradation mapping ------------------------------------------

        fn linear_input() -> BessDegradationCostInput {
            BessDegradationCostInput {
                asset_id: AssetId::new(FlexUuid::parse(TEST_ASSET_UUID).expect("valid")),
                curve_version: "measured-curve-v1".to_string(),
                chemistry: DegradationChemistry::Lfp,
                curve: BessDegradationCurveShape::Linear {
                    cost_per_mwh_throughput: 12.5,
                },
                provenance: flexcore_contracts::capability::Provenance::Measured,
                provenance_note: "measured histogram, planning aid".to_string(),
                parameters_unverified_secondary: false,
            }
        }

        #[test]
        fn asset_registry_reference_is_the_canonical_hyphenated_form() {
            let asset = AssetId::new(FlexUuid::parse(TEST_ASSET_UUID).expect("valid"));
            assert_eq!(asset_registry_reference(asset), TEST_ASSET_UUID);
        }

        #[test]
        fn the_degradation_mapping_is_field_exact() {
            let mapped = map_bess_degradation_cost_input(&linear_input());
            assert_eq!(mapped.asset_id, TEST_ASSET_UUID);
            assert_eq!(mapped.curve_version, "measured-curve-v1");
            assert!(matches!(mapped.chemistry, DegradationChemistry::Lfp));
            assert_eq!(
                mapped.curve,
                BessDegradationCurveShape::Linear {
                    cost_per_mwh_throughput: 12.5
                }
            );
            assert_eq!(mapped.provenance_note, "measured histogram, planning aid");
            assert!(!mapped.parameters_unverified_secondary);
        }

        #[test]
        fn the_degradation_mapping_preserves_out_of_scope_chemistry_and_pwl_shape() {
            let input = BessDegradationCostInput {
                asset_id: AssetId::new(FlexUuid::parse(TEST_ASSET_UUID).expect("valid")),
                curve_version: "unverified-v2".to_string(),
                chemistry: DegradationChemistry::OutOfScope {
                    name: "sodium-ion".to_string(),
                },
                curve: BessDegradationCurveShape::PiecewiseLinear {
                    segments: vec![
                        flexcore_domain::bess_cost_curve::BessPwlSegment {
                            cumulative_throughput_limit_mwh: 10.0,
                            marginal_cost_per_mwh: 4.0,
                        },
                        flexcore_domain::bess_cost_curve::BessPwlSegment {
                            cumulative_throughput_limit_mwh: 25.0,
                            marginal_cost_per_mwh: 9.0,
                        },
                    ],
                },
                provenance: flexcore_contracts::capability::Provenance::Unknown,
                provenance_note: "secondary-source parameters".to_string(),
                parameters_unverified_secondary: true,
            };
            let mapped = map_bess_degradation_cost_input(&input);
            assert!(matches!(
                mapped.chemistry,
                DegradationChemistry::OutOfScope { name } if name == "sodium-ion"
            ));
            match mapped.curve {
                BessDegradationCurveShape::PiecewiseLinear { segments } => {
                    assert_eq!(segments.len(), 2);
                    assert_eq!(segments[0].cumulative_throughput_limit_mwh, 10.0);
                    assert_eq!(segments[1].marginal_cost_per_mwh, 9.0);
                }
                other => panic!("expected the piecewise shape to survive, got {other:?}"),
            }
            assert!(mapped.parameters_unverified_secondary);
        }
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
