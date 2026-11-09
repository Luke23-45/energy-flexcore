// P3.17 — Shared test double for the integration tests: a scripted
// `Tier1Backend` that returns pre-recorded outcomes and records every model
// it was handed (so tests can assert the assembled formulation and that the
// backend was — or was never — invoked).
//
// This is a documented test double, not a solver and not a stub in the
// provider: it exists so the provider's formulation assembly, class gate,
// and §12.6 mapping are fully tested without a built worker (the real-solver
// replay lands with P3.19/P3.26 — decision note §6 item 6).

use flexcore_optimization_api::backend::{
    BackendDescription, BackendError, BackendOutcome, BackendStatus, Tier1Backend, Tier1Model,
};
use std::sync::{Arc, Mutex};

/// A scripted backend: pops outcomes from a queue, records the models it
/// receives.
pub struct ScriptedBackend {
    outcomes: Mutex<Vec<Result<BackendOutcome, BackendError>>>,
    calls: Mutex<Vec<(Tier1Model, BackendOptions)>>,
    options: Mutex<Vec<BackendOptions>>,
}

impl ScriptedBackend {
    /// Creates the backend with the given outcome queue (consumed front to
    /// back, one per solve call).
    pub fn new(outcomes: Vec<Result<BackendOutcome, BackendError>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes),
            calls: Mutex::new(Vec::new()),
            options: Mutex::new(Vec::new()),
        }
    }

    /// Number of solve calls received.
    pub fn solve_count(&self) -> usize {
        self.calls.lock().expect("test mutex").len()
    }

    /// The recorded (model, options) pairs, most recent last.
    pub fn recorded_calls(&self) -> Vec<(Tier1Model, BackendOptions)> {
        self.calls.lock().expect("test mutex").clone()
    }

    /// All recorded option sets.
    pub fn recorded_options(&self) -> Vec<BackendOptions> {
        self.options.lock().expect("test mutex").clone()
    }
}

impl Tier1Backend for ScriptedBackend {
    fn solve(
        &self,
        model: &Tier1Model,
        options: &BackendOptions,
    ) -> Result<BackendOutcome, BackendError> {
        self.calls
            .lock()
            .expect("test mutex")
            .push((model.clone(), options.clone()));
        self.options
            .lock()
            .expect("test mutex")
            .push(options.clone());
        let mut queue = self.outcomes.lock().expect("test mutex");
        if queue.is_empty() {
            return Err(BackendError::Unavailable(
                "scripted outcome queue exhausted".to_string(),
            ));
        }
        queue.remove(0)
    }

    fn describe(&self) -> BackendDescription {
        BackendDescription {
            name: "scripted-backend (test double)".to_string(),
            version: "0".to_string(),
            notes: vec![
                "test double over the Tier1Backend seam; the real worker adapter \
                 lands with P3.19 (decision note D-P3.17-1)"
                    .to_string(),
            ],
        }
    }
}

/// Shorthand for an owned scripted backend behind the trait object.
pub fn scripted(outcomes: Vec<Result<BackendOutcome, BackendError>>) -> Arc<ScriptedBackend> {
    Arc::new(ScriptedBackend::new(outcomes))
}

/// Builds a `BackendOutcome` with the given status and optional payload.
pub fn outcome(
    status: BackendStatus,
    solver_status_text: &str,
    objective_value: Option<f64>,
    column_values: Option<Vec<f64>>,
) -> BackendOutcome {
    BackendOutcome {
        status,
        objective_value,
        column_values,
        reduced_costs: None,
        row_duals: None,
        max_primal_violation: None,
        max_dual_violation: None,
        iteration_count: None,
        solver_status_text: solver_status_text.to_string(),
        conflict: None,
        solver_log_excerpt: None,
        warnings: Vec::new(),
    }
}
