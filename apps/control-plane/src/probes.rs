// P1.22 — Health and readiness probes.
//
// Liveness (`/health`): the process is up. Readiness (`/ready`): the
// dependencies are usable (database reachable, broker reachable). They are
// deliberately distinct: a dependency blip must fail readiness while
// liveness stays up, or the orchestrator would restart-loop on a healthy
// process (ledger P1.22 acceptance criteria).
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A dependency health check: returns Ok(()) when usable.
pub type HealthCheck =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

/// The readiness bundle: named dependency checks plus the shutdown gate.
#[derive(Clone)]
pub struct Readiness {
    checks: Vec<(String, HealthCheck)>,
    shutting_down: Arc<AtomicBool>,
}

impl Readiness {
    /// Builds the readiness bundle from named checks.
    pub fn new(checks: Vec<(String, HealthCheck)>) -> Self {
        Self {
            checks,
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Marks the process as shutting down (readiness goes false first, so
    /// the load balancer stops routing while we drain).
    pub fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Runs every check; returns the failures (empty = ready).
    pub async fn evaluate(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if self.is_shutting_down() {
            failures.push("process shutting down".to_string());
        }
        for (name, check) in &self.checks {
            if let Err(e) = check().await {
                failures.push(format!("{name}: {e}"));
            }
        }
        failures
    }
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readiness")
            .field("checks", &self.checks.len())
            .field("shutting_down", &self.is_shutting_down())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::state::AppState;

/// Liveness: the process is up. Always 200 while running, independent of
/// dependency state (readiness ≠ liveness, ledger P1.22).
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({ "status": "up" })),
    )
}

/// Readiness: all dependency checks pass AND the process is not shutting
/// down. Non-2xx with the failing check names otherwise.
pub async fn ready(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let failures = state.readiness.evaluate().await;
    if failures.is_empty() {
        (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "failures": failures })),
        )
            .into_response()
    }
}
