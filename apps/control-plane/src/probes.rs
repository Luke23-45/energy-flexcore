// P1.22 — Health and readiness probes.
//
// Liveness (`/health`): the process is up. Readiness (`/ready`): the
// dependencies are usable (database reachable, broker reachable). They are
// deliberately distinct: a dependency blip must fail readiness while
// liveness stays up, or the orchestrator would restart-loop on a healthy
// process (ledger P1.22 acceptance criteria).
//
// P11.21 — health-check semantics PER COMPOSE SERVICE (the semantics are a
// documented contract, not an accident of the handlers below):
//
// | Service | `/health` (liveness) | `/ready` (readiness) | `/metrics` |
// |---|---|---|---|
// | `flexcore-control-plane` | always 200 while the process runs — restarting cannot fix a dependency | 503 when a named dependency check fails (`database`, `broker`) or the process is shutting down; the body names EACH check's result | the P1.20/P2.21/P11.01 registry plus `flexcore_commands_stuck` (P11.20) |
// | `flexcore-worker` | always 200 while the process runs (its own module, `apps/worker/src/probes.rs`) | 503 when Postgres is unreachable, the NATS client is disconnected, or the worker is draining; the body names each dependency | the `flexcore_command_consumer_*` families (P11.12, exposed by P11.21) |
//
// The infrastructure services in the Compose shape (PostgreSQL, NATS, the
// P11.06 OTel Collector) carry their own Compose healthchecks — they are
// dependencies here, not probe hosts; this module documents only the two
// FlexCore application services (the P11.21 decision record carries the
// full per-service table).
//
// The P1.22 semantics are EXTENDED, never regressed: the status codes, the
// distinct liveness/readiness split, and the failure list are unchanged;
// the `/ready` body additionally names each check's result (additive).
use std::collections::BTreeMap;
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

    /// Runs every check; returns the failures (empty = ready). Built on
    /// [`Readiness::evaluate_detailed`] (P11.21) — same results.
    pub async fn evaluate(&self) -> Vec<String> {
        self.evaluate_detailed()
            .await
            .into_iter()
            .filter_map(|(name, result)| match result {
                Err(e) => Some(format!("{name}: {e}")),
                Ok(()) => None,
            })
            .collect()
    }

    /// P11.21 — runs every check and returns EACH named check's result
    /// (plus the shutdown gate as a synthetic first entry), so the `/ready`
    /// response can name per-dependency status. The process-level shutdown
    /// entry keeps the P1.22 wording ("process shutting down").
    pub async fn evaluate_detailed(&self) -> Vec<(String, Result<(), String>)> {
        let mut results: Vec<(String, Result<(), String>)> = Vec::new();
        if self.is_shutting_down() {
            results.push((
                "process".to_string(),
                Err("process shutting down".to_string()),
            ));
        }
        for (name, check) in &self.checks {
            let result = check().await;
            results.push((name.clone(), result));
        }
        results
    }

    /// P11.21 — the named checks this bundle evaluates (the names only,
    /// never a run): the per-service semantics documentation and the
    /// `/ready` body's check list both derive from the same registration.
    pub fn check_names(&self) -> Vec<String> {
        self.checks.iter().map(|(name, _)| name.clone()).collect()
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
///
/// P11.21 (additive): the body now names EVERY check's result —
/// `checks: {<name>: "ok" | "failed: <error>"}` — so an operator sees the
/// per-dependency status without correlating logs. The status codes and
/// the `failures` list are exactly the P1.22 ones (never regressed).
pub async fn ready(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let detailed = state.readiness.evaluate_detailed().await;
    let mut checks: BTreeMap<String, String> = BTreeMap::new();
    let mut failures = Vec::new();
    for (name, result) in detailed {
        match result {
            Ok(()) => {
                checks.insert(name, "ok".to_string());
            }
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                checks.insert(name, format!("failed: {error}"));
            }
        }
    }
    if failures.is_empty() {
        (
            StatusCode::OK,
            Json(json!({ "status": "ready", "checks": checks })),
        )
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "failures": failures, "checks": checks })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_with(result: Result<(), String>) -> HealthCheck {
        Arc::new(move || {
            let result = result.clone();
            Box::pin(async move { result }) as Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        })
    }

    /// P11.21 — `evaluate_detailed` names EVERY check (passing included),
    /// and the P1.22 `evaluate` failure list is exactly its failing subset
    /// (the extension never regressed the original semantics).
    #[tokio::test]
    async fn detailed_evaluation_names_every_check_and_matches_evaluate() {
        let readiness = Readiness::new(vec![
            ("database".to_string(), check_with(Ok(()))),
            ("broker".to_string(), check_with(Err("down".to_string()))),
        ]);
        let detailed = readiness.evaluate_detailed().await;
        let names: Vec<&str> = detailed.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["database", "broker"]);
        assert_eq!(detailed[0].1, Ok(()));
        assert_eq!(detailed[1].1, Err("down".to_string()));

        let failures = readiness.evaluate().await;
        assert_eq!(failures, vec!["broker: down".to_string()]);

        // All-pass: detailed reports every check ok; evaluate is empty.
        let ok = Readiness::new(vec![("database".to_string(), check_with(Ok(())))]);
        let detailed = ok.evaluate_detailed().await;
        assert_eq!(detailed[0].1, Ok(()));
        assert!(ok.evaluate().await.is_empty());
    }

    /// P1.22 (restated through the P11.21 seam) — the shutdown gate is a
    /// synthetic first entry in the detailed results and a failure in
    /// `evaluate`; the check names registration matches the bundle.
    #[tokio::test]
    async fn shutdown_gate_appears_in_the_detailed_results() {
        let readiness = Readiness::new(vec![("database".to_string(), check_with(Ok(())))]);
        assert_eq!(
            readiness.check_names(),
            vec!["database".to_string()]
        );
        readiness.begin_shutdown();
        let detailed = readiness.evaluate_detailed().await;
        assert_eq!(detailed[0].0, "process");
        assert!(detailed[0].1.is_err());
        let failures = readiness.evaluate().await;
        assert_eq!(failures, vec!["process: process shutting down".to_string()]);
    }
}
