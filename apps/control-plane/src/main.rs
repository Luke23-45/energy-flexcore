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
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use flexcore_control_plane::auth::{DenyAllResolver, IdentityResolver, phase3_default_policies};
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
    let filter = tracing_subscriber::EnvFilter::try_new(&cfg.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();

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
    let state = AppState::new(
        Backend::Postgres {
            store: store.clone(),
            ids,
            clock,
        },
        phase3_default_policies(),
        identity,
        readiness,
        // Telemetry read port (P2.12): the Postgres adapter does not
        // implement `TelemetryReader` yet (recorded P2.06-adjacent gap);
        // the telemetry routes fail closed until it lands.
        None,
        // The current-state freshness limit (P2.18 step 3): configuration,
        // not a hard-coded constant.
        cfg.current_state_freshness.as_millis() as i64,
    );

    let app = stage1_router(state.clone()).layer(
        tower::ServiceBuilder::new()
            // Outermost: metrics + correlation log see every response,
            // including timeout- and limit-generated ones.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                track_metrics,
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

    let drain = cfg.drain_timeout;
    let drain_rx = signal_rx.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let mut rx = signal_rx;
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
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
                    Vec::<u8>::new(),
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
