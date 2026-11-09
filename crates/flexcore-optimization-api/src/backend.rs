// P3.17 — The Tier 1 backend seam: the transport-neutral, solver-neutral
// contract between the Rust provider and the Python optimization worker
// (decision note `ledger/phase-03-flexibility-scheduling/17_optimizer_port.md`
// D-P3.17-1).
//
// Decision summary (full rationale in the decision note):
// - Tier 1 (LP / convex QP / MILP) solves execute in the Python optimization
//   worker via the pinned Python binding (highspy 1.15.1, verified 2026-11-07,
//   RESEARCH.md §1). This module deliberately contains **no** binding, no
//   transport, and no solver-specific type.
// - The worker adapter implementing [`Tier1Backend`] lands with P3.19: an
//   in-process bridge for tests, and the §15.1 gRPC/Protobuf path for
//   deployment once the Phase 7 protobuf contracts exist. The trait is
//   transport-neutral so the adapter choice does not ripple into the port.
// - Python is the numerical worker only; it is never the command authority
//   (§7.2). The provider (Rust side) owns status mapping and the §12.6
//   result record.
//
// VERIFY-BUILD (all items confirmed at adapter build time against the pinned
// binding; see decision note §6):
// 1. Native model-status values → [`BackendStatus`] mapping.
// 2. IIS / conflict-refinement API surface → [`ConflictEvidence`].
// 3. Hessian off-diagonal scaling convention for [`HessianTerm`].
// 4. Infinity constant: this IR uses `f64::INFINITY`; the adapter maps it to
//    the pinned solver's infinity (conventionally 1e30).
// 5. Option names for time limit, thread count, tolerances, MIP gap, scaling.
//
// This module never names the solver; the source-scan dependency-direction
// test enforces it.

use crate::config::ScalingPolicy;
use crate::problem::{ProblemClass, VariableKind};
use serde::{Deserialize, Serialize};

/// One solver column (a formulation variable in solver order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    /// Column name.
    pub name: String,
    /// Integrality of the column.
    pub kind: VariableKind,
    /// Lower bound; `f64::NEG_INFINITY` = unbounded (VERIFY-BUILD item 4).
    pub lower_bound: f64,
    /// Upper bound; `f64::INFINITY` = unbounded (VERIFY-BUILD item 4).
    pub upper_bound: f64,
}

/// One solver row (a linear constraint in solver order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowSpec {
    /// Row name.
    pub name: String,
    /// Sparse `(column index, coefficient)` pairs.
    pub coefficients: Vec<(usize, f64)>,
    /// Row lower bound; `f64::NEG_INFINITY` = unbounded below.
    pub lower: f64,
    /// Row upper bound; `f64::INFINITY` = unbounded above.
    pub upper: f64,
}

/// One Hessian entry contributing `coefficient · x_row · x_column`
/// (upper-triangle; VERIFY-BUILD item 3 for the adapter's off-diagonal
/// handling).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HessianTerm {
    /// First column index (<= `column`).
    pub row: usize,
    /// Second column index (>= `row`).
    pub column: usize,
    /// Coefficient of the `x_row · x_column` product.
    pub coefficient: f64,
}

/// Options for one solve, translated 1:1 from the provider-owned
/// [`SolverConfig`](crate::config::SolverConfig). The adapter maps each field
/// onto the pinned solver's options (VERIFY-BUILD item 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendOptions {
    /// Time limit in seconds; `None` = no explicit limit.
    pub time_limit_seconds: Option<f64>,
    /// Fixed thread count (determinism).
    pub thread_count: u32,
    /// Primal feasibility tolerance.
    pub primal_feasibility_tolerance: f64,
    /// Dual feasibility / optimality tolerance.
    pub dual_feasibility_tolerance: f64,
    /// Relative MIP gap; `None` = solver default.
    pub mip_relative_gap: Option<f64>,
    /// Scaling policy.
    pub scaling: ScalingPolicy,
}

/// The fully assembled Tier 1 model handed to the backend: columns, rows,
/// objective, and integrality — exactly what a LP/QP/MILP solver consumes.
/// A conic formulation can never be assembled into this type (the provider
/// gates the class first), which is the structural guarantee behind "the
/// solver is never invoked on an SOCP formulation".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tier1Model {
    /// Problem name (for solver logs and audit).
    pub problem_name: String,
    /// The declared class; always LP, convex QP, or MILP for a Tier 1 model.
    pub declared_class: ProblemClass,
    /// `true` = minimize, `false` = maximize.
    pub minimize: bool,
    /// Objective constant offset.
    pub objective_offset: f64,
    /// Objective linear terms `(column index, coefficient)`.
    pub objective_linear: Vec<(usize, f64)>,
    /// Objective quadratic terms (upper triangle; empty for LP/MILP).
    pub hessian: Vec<HessianTerm>,
    /// Columns (variables) in formulation order.
    pub columns: Vec<ColumnSpec>,
    /// Rows (linear constraints) in formulation order.
    pub rows: Vec<RowSpec>,
}

/// Backend solve status — the neutral vocabulary the worker maps the pinned
/// solver's native model status onto (VERIFY-BUILD item 1). The provider maps
/// these onto the six §12.6 statuses (decision note §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendStatus {
    /// Proven optimal.
    Optimal,
    /// Feasible solution returned without a proven-optimality claim.
    Feasible,
    /// No feasible solution exists.
    Infeasible,
    /// Feasible region exists but the objective is unbounded.
    Unbounded,
    /// The solver could not separate unbounded from infeasible.
    UnboundedOrInfeasible,
    /// Time limit reached.
    TimeLimitReached,
    /// Iteration budget reached.
    IterationLimitReached,
    /// The backend rejected the model (formulation error at the solver).
    ModelError,
    /// The solver errored during the solve.
    SolveError,
    /// Anything the adapter cannot classify.
    Unknown,
}

/// Infeasibility evidence as reported by the backend (mapped from the pinned
/// solver's conflict/IIS output; VERIFY-BUILD item 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictEvidence {
    /// Names of columns the conflict refinement implicated.
    pub variables: Vec<String>,
    /// Names of rows the conflict refinement implicated.
    pub rows: Vec<String>,
    /// Verbatim backend report, for audit.
    pub report: String,
}

/// The raw backend outcome for one solve, mapped by the provider onto the
/// §12.6 result record. Vector lengths are contracts: `column_values`,
/// `reduced_costs` must equal `columns.len()`; `row_duals` must equal
/// `rows.len()` — a violating payload is a contract failure
/// ([`BackendError::ContractViolation`] or an
/// [`crate::port::OptimizerError::BackendUnavailable`] mapping).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendOutcome {
    /// Backend status.
    pub status: BackendStatus,
    /// Objective value, when a solution was produced.
    pub objective_value: Option<f64>,
    /// Primal column values, when produced (length = columns).
    pub column_values: Option<Vec<f64>>,
    /// Reduced costs, when produced (length = columns).
    pub reduced_costs: Option<Vec<f64>>,
    /// Row duals, when produced (length = rows).
    pub row_duals: Option<Vec<f64>>,
    /// Maximum primal violation, when reported.
    pub max_primal_violation: Option<f64>,
    /// Maximum dual violation, when reported.
    pub max_dual_violation: Option<f64>,
    /// Iteration count, when reported.
    pub iteration_count: Option<u64>,
    /// Verbatim backend status text (audit trail for every mapping decision,
    /// especially the unbounded/limit mappings in decision note §3).
    pub solver_status_text: String,
    /// Infeasibility conflict evidence, when the backend exposed it.
    pub conflict: Option<ConflictEvidence>,
    /// Solver-log excerpt for the diagnostics channel, when captured.
    pub solver_log_excerpt: Option<String>,
    /// Warnings the backend itself raised.
    pub warnings: Vec<String>,
}

/// Backend identity for records and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendDescription {
    /// Backend name (e.g. the worker adapter's name).
    pub name: String,
    /// Backend version.
    pub version: String,
    /// Recorded notes (transport, pinned solver identity as recorded
    /// upstream, etc.).
    pub notes: Vec<String>,
}

/// Backend seam failure (transport or contract level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The backend is unreachable or failed before a solve could be attempted.
    Unavailable(String),
    /// The backend returned a payload violating its contract (e.g. wrong
    /// vector length).
    ContractViolation(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Unavailable(message) => write!(f, "backend unavailable: {message}"),
            BackendError::ContractViolation(message) => {
                write!(f, "backend contract violation: {message}")
            }
        }
    }
}

impl std::error::Error for BackendError {}

/// The Tier 1 backend seam (decision note D-P3.17-1): the transport-neutral
/// contract the Python optimization worker adapter implements (P3.19).
///
/// Sync by the same rule as the `Optimizer` port: no async runtime named
/// here; the adapter decides how to bridge any real transport.
pub trait Tier1Backend: Send + Sync {
    /// Solves the assembled Tier 1 model under the given options.
    fn solve(&self, model: &Tier1Model, options: &BackendOptions) -> Result<BackendOutcome, BackendError>;

    /// Backend identity for records and diagnostics.
    fn describe(&self) -> BackendDescription;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_trait_is_object_safe() {
        fn takes_dyn(_: &dyn Tier1Backend) {}
        let _ = takes_dyn;
    }

    #[test]
    fn backend_status_serializes_with_stable_names() {
        let text = serde_json::to_string(&BackendStatus::TimeLimitReached).expect("serializes");
        assert_eq!(text, "\"time_limit_reached\"");
        let back: BackendStatus = serde_json::from_str(&text).expect("deserializes");
        assert_eq!(back, BackendStatus::TimeLimitReached);
    }
}
