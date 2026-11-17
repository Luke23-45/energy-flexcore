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
// - `/metrics` — P11.21 (Phase 11 wave 2, Agent B): the worker's FIRST
//   metrics surface, rendering [`CommandConsumerMetrics::render`] (the
//   P11.12 handoff seam — the record named this exact function as the
//   wiring point). Same Prometheus text exposition as the control plane's
//   `/metrics`; zero values always render (the always-render discipline).
//
//   FRV1-R2-003 (final review): the route now renders BOTH metric sets the
//   worker collects — the P11.12 command-consumer families AND the P2.21
//   telemetry families ([`crate::ingestion::IngestionMetrics::
//   render_prometheus`], whose doc named this surface as its wiring point
//   but which nothing called). The SAME `Arc<IngestionMetrics>` the
//   ingestion task drives is shared with the probe state (the composition
//   root wires both), so the counters are live, not a snapshot. When no
//   telemetry source is composed (`FLEXCORE_SIMULATOR_SCENARIO` unset) the
//   telemetry families render at zero — the always-render discipline, with
//   the not-composed status recorded by the composition root's startup
//   log line (the same honesty posture the control-plane registry's
//   not-composed families take, FRV1-R2-003).
//
// P11.21 — health-check semantics PER COMPOSE SERVICE (the semantics are
// a documented contract, not an accident of the handlers below):
//
// | Service            | `/health` (liveness)                | `/ready` (readiness)                                                   | `/metrics` |
// |---|---|---|---|
// | `flexcore-worker`  | always 200 while the process runs   | 503 when Postgres is unreachable, the NATS client is disconnected, or the worker is draining; 200 with per-dependency detail otherwise | the `flexcore_command_consumer_*` families (P11.12) + the `flexcore_telemetry_*` families (P2.21; FRV1-R2-003) |
// | `flexcore-control-plane` | always 200 while running (its own module, `apps/control-plane/src/probes.rs`) | 503 with the failing named checks (`database`, `broker`) or during shutdown | the P1.20/P2.21/P11.01 registry incl. `flexcore_commands_stuck` (P11.20) |
//
// The infrastructure services in the Compose shape (PostgreSQL, NATS, the
// P11.06 OTel Collector) carry their OWN Compose healthchecks — they are
// dependencies here, not probe hosts; this module documents only the two
// FlexCore application services (the P11.21 decision record carries the
// full per-service table).
//
// The probe server is deliberately tiny: no business routes, no
// middleware stack — the worker is not a second API server (P1.19 risk
// note); `/metrics` is an exposition endpoint, not a business route.
// Shutdown is graceful: the server stops accepting new requests when the
// shared shutdown watch fires, then drains open connections.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_nats::client::Client;
use async_nats::connection_state::ConnectionState;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::watch;

use crate::command_consumer::CommandConsumerMetrics;
use crate::ingestion::IngestionMetrics;

/// State shared with the probe handlers.
pub struct ProbeState {
    pool: PgPool,
    nats: Client,
    /// Set when the worker has started draining for shutdown; readiness
    /// fails from that moment on.
    draining: AtomicBool,
    /// P11.21 — the command-consumer metrics the `/metrics` route renders.
    /// The SAME Arc the consumer task drives (composition root wires both),
    /// so the counters are live, not a snapshot.
    command_metrics: Arc<CommandConsumerMetrics>,
    /// FRV1-R2-003 — the P2.21 telemetry ingestion metrics the `/metrics`
    /// route renders alongside the command families. The SAME Arc the
    /// ingestion task drives (the composition root wires both); when no
    /// telemetry source is composed the Arc is shared with nothing and the
    /// families render at zero (always-render discipline).
    ingestion_metrics: Arc<IngestionMetrics>,
}

impl ProbeState {
    /// Builds the probe state. `command_metrics` is the consumer's metric
    /// block and `ingestion_metrics` the ingestion pipeline's metric block
    /// (P11.21 seam — the composition root passes the SAME Arcs to
    /// `run_command_consumer` and the `IngestionTask`).
    pub fn new(
        pool: PgPool,
        nats: Client,
        command_metrics: Arc<CommandConsumerMetrics>,
        ingestion_metrics: Arc<IngestionMetrics>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            nats,
            draining: AtomicBool::new(false),
            command_metrics,
            ingestion_metrics,
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
        // P11.21 — the worker's first metrics surface (see module header).
        .route("/metrics", get(metrics))
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

/// P11.21 — the worker's Prometheus exposition endpoint: the P11.12
/// command-consumer families (`flexcore_command_consumer_*`), rendered by
/// [`CommandConsumerMetrics::render`] (the recorded handoff seam), AND —
/// FRV1-R2-003 — the P2.21 telemetry families rendered by
/// [`IngestionMetrics::render_prometheus`], composed into one body (the
/// same composition discipline the control plane's registry uses: one
/// metrics surface, no second exposition). The same
/// `text/plain; version=0.0.4; charset=utf-8` content type the control
/// plane's `/metrics` serves. Zero values always render so a scrape
/// exposes the full inventory on an idle consumer.
async fn metrics(State(state): State<Arc<ProbeState>>) -> impl IntoResponse {
    let mut body = state.command_metrics.render();
    body.push_str(&state.ingestion_metrics.render_prometheus());
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    (StatusCode::OK, headers, body)
}

/// Readiness: dependencies reachable AND not draining. Non-2xx otherwise.
/// The Postgres check is a one-round-trip query on the shared pool; the
/// NATS check is the client's own connection state (no network round
/// trip — the async-nats client reconnects on its own, and a disconnected
/// state is precisely what readiness must report).
///
/// P11.21 — the response body now names EACH dependency's status
/// (additive: the status codes and the failure semantics are exactly the
/// P1.19 ones; the body only stops being empty). The detail names the
/// failing dependency so an operator sees WHICH of Postgres/NATS/drain is
/// down without reading the logs; the log line keeps the underlying error
/// (never a response body — and nothing secret is logged either way).
async fn readiness(State(state): State<Arc<ProbeState>>) -> impl IntoResponse {
    let draining = state.draining.load(Ordering::SeqCst);
    let nats_connected = state.nats.connection_state() == ConnectionState::Connected;
    let postgres: Result<(), String> = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => Ok(()),
        // The failure class is enough for a probe body; details live in
        // the logs, never in a response (and nothing secret is logged).
        Err(error) => {
            tracing::warn!(error = %error, "readiness probe: postgres unreachable");
            Err("unreachable".to_string())
        }
    };
    let ready = !draining && nats_connected && postgres.is_ok();
    let body = json!({
        "status": if ready { "ready" } else { "not_ready" },
        "draining": draining,
        "dependencies": {
            "postgres": if postgres.is_ok() { "up" } else { "down" },
            "nats": if nats_connected { "up" } else { "down" },
        },
    });
    if ready {
        (StatusCode::OK, Json(body)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}
