// P1.19 — Worker health/readiness probes (liveness vs readiness,
// distinguished; ledger P1.19 acceptance: they behave differently under a
// stopped broker).
//
// - `/health` — LIVENESS. Always 200 while the process runs, even when
//   PostgreSQL or NATS are unreachable. Restarting the process cannot fix
//   a stopped broker, so broker outages must not trigger pod restarts.
// - `/ready` — READINESS. Non-2xx (503) when either dependency is
//   unreachable (a probe query on the Postgres pool, the NATS client's
//   connection state) or when the worker is draining for shutdown.
//   Removing the pod from load-balancing/routing during an outage or a
//   shutdown is exactly what readiness is for.
//
// The probe server is deliberately tiny: no business routes, no
// middleware stack — the worker is not a second API server (P1.19 risk
// note). Shutdown is graceful: the server stops accepting new requests
// when the shared shutdown watch fires, then drains open connections.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_nats::client::Client;
use async_nats::connection_state::ConnectionState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tokio::sync::watch;

/// State shared with the probe handlers.
pub struct ProbeState {
    pool: PgPool,
    nats: Client,
    /// Set when the worker has started draining for shutdown; readiness
    /// fails from that moment on.
    draining: AtomicBool,
}

impl ProbeState {
    pub fn new(pool: PgPool, nats: Client) -> Arc<Self> {
        Arc::new(Self {
            pool,
            nats,
            draining: AtomicBool::new(false),
        })
    }

    /// Marks the worker as draining (readiness goes non-2xx).
    pub fn begin_draining(&self) {
        self.draining.store(true, Ordering::SeqCst);
    }
}

/// Binds the probe server and serves until the shutdown watch fires.
/// Resolves once the server has drained its open connections.
pub async fn serve_probes(
    addr: std::net::SocketAddr,
    state: Arc<ProbeState>,
    mut shutdown: watch::Receiver<bool>,
) -> std::io::Result<()> {
    let app = Router::new()
        .route("/health", get(liveness))
        .route("/ready", get(readiness))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "worker probe server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // Fires on the shutdown flag or on sender drop (either means
            // the composition root is going away).
            let _ = shutdown.changed().await;
        })
        .await
}

/// Liveness: the process is up. Always 200 — dependency outages are a
/// readiness concern (module header).
async fn liveness() -> StatusCode {
    StatusCode::OK
}

/// Readiness: dependencies reachable AND not draining. Non-2xx otherwise.
/// The Postgres check is a one-round-trip query on the shared pool; the
/// NATS check is the client's own connection state (no network round
/// trip — the async-nats client reconnects on its own, and a disconnected
/// state is precisely what readiness must report).
async fn readiness(State(state): State<Arc<ProbeState>>) -> StatusCode {
    if state.draining.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    if state.nats.connection_state() != ConnectionState::Connected {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => StatusCode::OK,
        // The failure class is enough for a probe body; details live in
        // the logs, never in a response (and nothing secret is logged).
        Err(error) => {
            tracing::warn!(error = %error, "readiness probe: postgres unreachable");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
