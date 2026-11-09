// P3.18 — The Tier 2 (continuous SOCP) provider via Clarabel (report §12.3,
// §12.5, §11.3, §7.4; foundations §7.8).
//
// Solver-class discipline (AGENTS.md rule 3.4 — ABSOLUTE):
// - Clarabel is a **convex conic** solver (LP/QP/SOCP/SDP, exponential and
//   power cones). It has **no integer variables**. It is NOT a MISOCP solver.
//   A formulation carrying any integer/binary variable is rejected by this
//   provider with the §12.6 status `unsupported` **before any solver
//   invocation** (an invocation counter, `ClarabelTier2Provider::
//   solver_invocations`, proves the gate ordering in tests) — never silently
//   solved as if it were continuous, and never routed to a MISOCP path that
//   does not exist.
// - This provider supports exactly one declared class:
//   `ContinuousSecondOrderConeProgram` (§12.3 Tier 2). LP/QP/MILP belong to
//   Tier 1 ([`crate::highs_provider::HighsTier1Provider`]); MISOCP (Tier 3)
//   and nonlinear AC-OPF (Tier 4) are not built in Phase 3 (P3.20).
//
// SOCP relaxation exactness caveat (report §12.5 — recorded as
// [`SOCP_EXACTNESS_CAVEAT`] and repeated on every solved result):
// exactness of a branch-flow SOCP relaxation is CONDITIONAL and must be
// checked, never assumed. A Tier 2 result is a **candidate** for the Phase 4
// validation gate, never a feasibility verdict, and must be verified with an
// independent AC power-flow engine. An `optimal` status proves optimality for
// the declared conic formulation only — never global optimality of, or
// feasibility for, the underlying nonlinear AC problem.
//
// Per-cone relaxation residual (foundations §7.8): for every second-order
// cone the provider computes the slack
//
//     slack = (rhs(x))² − ‖diag(a)·x + b‖₂          ([`cone_slack`])
//
// and for a DistFlow branch cone encoded canonically (see
// [`distflow_branch_residual`]) this equals `4·g_ij` with the foundations
// §7.8 residual `g_ij = v_i·l_ij − (P_ij² + Q_ij²)`. The residual is
// diagnostic evidence reported per run against a CONFIGURED tolerance
// ([`Tier2ProviderConfig::relaxation_residual_tolerance`]); it is reported,
// never hidden, and it does NOT replace the Phase 4 AC power-flow
// verification.
//
// Provider path (recorded as D-P3.18-1 in
// ledger/phase-03-flexibility-scheduling/18_clarabel_tier2.md): unlike Tier 1
// (D-P3.17-1, Python worker), Clarabel.rs is a **native Rust crate** invoked
// in-process — there is no transport boundary, so no `Tier2Backend` seam
// exists; the provider owns assembly, translation, status mapping, and the
// §12.6 result record directly. Version pin: clarabel 0.11.1 (RESEARCH.md §2;
// the Rust crate — NOT the R package 0.11.3).
//
// VERIFY-BUILD (established repo pattern; items confirmed against the pinned
// crate at build time — the pinned-version API facts below were taken from
// docs.rs `clarabel` 0.11.1 and the crate's own examples `example_socp.rs` /
// `example_qp.rs` on 2026-11-07, but no build has run in this batch):
// 1. `DefaultSolver::new(&P, &q, &A, &b, &cones, settings)` returns
//    `Result<Self, SolverError>`; `SolverError` must implement `Display` for
//    the error text recorded here.
// 2. Second-order cone element order: `SecondOrderConeT(dim)` blocks are
//    `(t, u₁, …, u_{dim−1})` with the scalar `t` **first**
//    (`‖(u₁,…,u_{dim−1})‖₂ ≤ t`) — as demonstrated by the crate's own
//    `examples/rust/example_socp.rs`. Confirm at build time.
// 3. Objective convention `minimize ½·xᵀPx + qᵀx` (crate README) and the
//    constant offset handling: Clarabel's `solution.obj_val` excludes the
//    IR objective constant; the provider re-adds it (and re-applies the
//    maximisation sign flip) when reporting `objective_value`.
// 4. Quadratic-objective translation: the IR stores upper-triangle terms
//    contributing `c·xᵢ·xⱼ`; the P-matrix entries built in
//    [`build_clarabel_data`] (`P_ii = 2c_ii`, symmetric `P_ij = P_ji = c_ij`,
//    sign-flipped for maximisation) follow from the ½xᵀPx convention —
//    confirm the off-diagonal convention at build time (mirrors decision note
//    17 §6 item 3).
// 5. Settings fields used: `max_threads` (usize; 0 = solver chooses),
//    `time_limit` (f64 seconds; default no limit), `verbose`, `tol_feas`,
//    `tol_gap_abs`, `tol_gap_rel`, `equilibrate_enable`. Confirm names/types
//    and the default tolerance values at build time.
// 6. Dual sign convention for rows encoded with negated coefficients (upper
//    bounds) and for `Range` rows (expanded to two conic rows): the raw
//    Clarabel `z` multipliers are recorded; the sign convention relative to
//    the original IR row is a build-time confirmation item.
// 7. `SolverStatus` debug text recorded verbatim per solve.

use crate::config::SolverConfig;
use crate::port::{
    ConstraintFeature, DiagnosticsDeclaration, DeterminismDeclaration, LicenseDeclaration,
    Optimizer, OptimizerError, ScalingDeclaration, SolverCapabilities, TolerancesDeclaration,
    WarmStartSupport,
};
use crate::problem::{
    ObjectiveSense, OptimizationProblem, ProblemClass, ProblemError, RowBounds, SecondOrderCone,
    VariableKind, VariableValue,
};
use crate::result::{
    DeterminismRecord, Diagnostics, DualDiagnostics, InfeasibilityEvidence, OptimizationResult,
    PrimalDiagnostics, ResidualReport, SolveStatus,
};
use clarabel::algebra::CscMatrix;
use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus, SupportedConeT};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The pinned Tier 2 solver version: **Clarabel.rs 0.11.1** (docs.rs, crate
/// `clarabel` 0.11.1, CHANGELOG entry dated 2025-11-06; read 2026-11-07,
/// RESEARCH.md §2). This is the **Rust crate** — the R package `clarabel`
/// 0.11.3 tracks it and must never be confused with it. Recorded on every
/// result via the determinism record.
pub const CLARABEL_SOLVER_VERSION_PIN: &str = "0.11.1";

/// Provider identity (the `builtin.` prefix matches the §8.1 built-in
/// provider placement, mirroring the Tier 1 provider).
pub const PROVIDER_ID: &str = "builtin.optimizer-clarabel-tier2";

/// The §12.5 SOCP relaxation exactness caveat, recorded verbatim on every
/// solved Tier 2 result and in the capability declaration. Exactness is
/// conditional and must be checked, never assumed; the Tier 2 result is a
/// CANDIDATE for Phase 4 validation, never a feasibility verdict.
pub const SOCP_EXACTNESS_CAVEAT: &str =
    "SOCP relaxation exactness is conditional and must be checked, never \
     assumed (report §12.5): this Tier 2 result is a CANDIDATE for Phase 4 \
     validation, never a feasibility verdict; verify with an independent AC \
     power-flow engine before any operational use.";

/// The foundations §7.8 per-branch relaxation residual formula, recorded so
/// the reported diagnostic is traceable to its source.
pub const DISTFLOW_RESIDUAL_FORMULA: &str =
    "g_ij = v_i·l_ij − (P_ij² + Q_ij²) (foundations §7.8)";

/// Default per-cone relaxation-residual tolerance. This is a **provider-chosen
/// diagnostic threshold** (not a solver tolerance, and not a certified
/// feasibility margin); it is recorded here so the default is reviewable.
/// Rationale for `1e-4`: the pinned solver's feasibility/optimality
/// tolerances are at the 1e-7..1e-8 level on unit-scaled data, and a cone
/// slack scales quadratically in the primal variables, so a threshold at the
/// 1e-6 level would flag numerically binding cones as "violated"; `1e-4`
/// leaves that margin while still catching meaningful violations and loose
/// (non-tight) relaxations.
pub const DEFAULT_RELAXATION_RESIDUAL_TOLERANCE: f64 = 1e-4;

/// Provider-owned Tier 2 configuration. The residual tolerance is a
/// **provider configuration** item rather than a field of the per-run
/// [`SolverConfig`] because it is diagnostic policy of this provider, not a
/// solver option (the per-run port configuration remains the P3.17 contract
/// unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tier2ProviderConfig {
    /// The tolerance the per-cone relaxation residual is evaluated against
    /// (must be finite and > 0). A cone slack `s` with `s < −tolerance` is
    /// reported as violated; `s > tolerance` is reported as loose (relaxation
    /// not tight — exactness not demonstrated); `|s| ≤ tolerance` is reported
    /// as binding/tight. Every entry is reported, never hidden.
    pub relaxation_residual_tolerance: f64,
}

impl Default for Tier2ProviderConfig {
    fn default() -> Self {
        Self {
            relaxation_residual_tolerance: DEFAULT_RELAXATION_RESIDUAL_TOLERANCE,
        }
    }
}

impl Tier2ProviderConfig {
    /// Validates the provider configuration (mirrors
    /// [`crate::config::SolverConfig::validate`]).
    pub fn validate(&self) -> Result<(), crate::config::ConfigError> {
        if !self.relaxation_residual_tolerance.is_finite() {
            return Err(crate::config::ConfigError::NonFiniteValue {
                field: "relaxation_residual_tolerance",
            });
        }
        if self.relaxation_residual_tolerance <= 0.0 {
            return Err(crate::config::ConfigError::NonPositiveTolerance {
                field: "relaxation_residual_tolerance",
            });
        }
        Ok(())
    }
}

/// The supported Tier 2 classes (§12.3 Tier 2 only).
fn supported_classes() -> Vec<ProblemClass> {
    vec![ProblemClass::ContinuousSecondOrderConeProgram]
}

/// The recorded mapping from the pinned solver's native statuses onto the
/// closed six-status §12.6 vocabulary (mirrors decision note 17 §3; recorded
/// in 18_clarabel_tier2.md §4). Kept next to the mapping for review.
///
/// | SolverStatus (clarabel 0.11.1) | §12.6 status | Notes |
/// |---|---|---|
/// | Solved | optimal | proven optimal for the declared conic formulation |
/// | AlmostSolved | feasible | reduced-accuracy solution; no proven-optimality claim (the §12.6 `feasible` definition) |
/// | PrimalInfeasible | infeasible | carries an explicit not-available reason for conflict evidence (Clarabel exposes no IIS) |
/// | AlmostPrimalInfeasible | numerical_failure | a reduced-accuracy infeasibility certificate is never published as an infeasible verdict (Tier 1 discipline: ambiguity/uncertainty is never mapped to `infeasible`) |
/// | DualInfeasible | numerical_failure | dual infeasibility implies an unbounded primal objective; the closed vocabulary has no `unbounded` value — mirrors the Tier 1 Unbounded mapping |
/// | AlmostDualInfeasible | numerical_failure | same reasoning, reduced accuracy |
/// | MaxIterations | timeout | resource limit was the iteration budget (recorded) |
/// | MaxTime | timeout | |
/// | NumericalError | numerical_failure | |
/// | InsufficientProgress | numerical_failure | |
/// | CallbackTerminated | numerical_failure | no callbacks are used by this provider; fail closed |
/// | Unsolved | numerical_failure | the solver did not run to any conclusion; fail closed |
fn map_clarabel_status(status: SolverStatus) -> SolveStatus {
    match status {
        SolverStatus::Solved => SolveStatus::Optimal,
        SolverStatus::AlmostSolved => SolveStatus::Feasible,
        SolverStatus::PrimalInfeasible => SolveStatus::Infeasible,
        SolverStatus::MaxIterations | SolverStatus::MaxTime => SolveStatus::Timeout,
        SolverStatus::Unsolved
        | SolverStatus::DualInfeasible
        | SolverStatus::AlmostPrimalInfeasible
        | SolverStatus::AlmostDualInfeasible
        | SolverStatus::NumericalError
        | SolverStatus::InsufficientProgress
        | SolverStatus::CallbackTerminated => SolveStatus::NumericalFailure,
    }
}

/// One column of the assembled conic model (solver-neutral; the conic analogue
/// of the Tier 1 `ColumnSpec`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConicColumn {
    /// Column name.
    pub name: String,
    /// Lower bound; `None` = unbounded below.
    pub lower_bound: Option<f64>,
    /// Upper bound; `None` = unbounded above.
    pub upper_bound: Option<f64>,
}

/// One linear row of the assembled conic model, carried with its original
/// declared bounds; the translation to conic standard form (equality → zero
/// cone; one-sided → nonnegative cone with sign-fixed coefficients; range →
/// two rows) happens in [`build_clarabel_data`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConicRow {
    /// Row name.
    pub name: String,
    /// Sparse `(variable index, coefficient)` pairs.
    pub coefficients: Vec<(usize, f64)>,
    /// Declared bounds.
    pub bounds: RowBounds,
}

/// One second-order cone of the assembled conic model, in the problem IR's
/// `‖diag(a)·x + b‖₂ ≤ cᵀx + d` form (the §11.3 inverter cone
/// `‖(P, Q)‖₂ ≤ S` is the canonical example).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConicCone {
    /// Cone name.
    pub name: String,
    /// Norm-side `(variable index, coefficient)` pairs.
    pub norm_members: Vec<(usize, f64)>,
    /// Norm-side offset.
    pub norm_offset: f64,
    /// Right-hand-side `(variable index, coefficient)` pairs.
    pub rhs_members: Vec<(usize, f64)>,
    /// Right-hand-side offset.
    pub rhs_offset: f64,
}

/// The fully assembled Tier 2 conic model: the solver-neutral, fully testable
/// intermediate between the validated problem IR and the pinned solver's
/// standard-form data. No solver-specific type appears here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConicModel {
    /// Problem name (for solver logs and audit).
    pub problem_name: String,
    /// The declared class; always `ContinuousSecondOrderConeProgram` for a
    /// Tier 2 model (defence in depth, mirroring the Tier 1 assembly gate).
    pub declared_class: ProblemClass,
    /// `true` = minimize, `false` = maximize.
    pub minimize: bool,
    /// Objective constant offset (re-applied when reporting the objective;
    /// VERIFY-BUILD item 3).
    pub objective_offset: f64,
    /// Objective linear terms `(variable index, coefficient)` in the
    /// formulation's declared sense (translation applies the sign flip).
    pub objective_linear: Vec<(usize, f64)>,
    /// Objective quadratic terms (upper triangle, declared sense;
    /// `(row, column, coefficient)`).
    pub hessian: Vec<(usize, usize, f64)>,
    /// Columns (variables) in formulation order.
    pub columns: Vec<ConicColumn>,
    /// Linear rows in formulation order.
    pub rows: Vec<ConicRow>,
    /// Second-order cones in formulation order.
    pub cones: Vec<ConicCone>,
}

/// Assembles the Tier 2 conic model from a validated formulation. The class
/// gate has already run when called from [`ClarabelTier2Provider::solve`];
/// this re-check is defence in depth so a non-conic declaration can never
/// reach model assembly (mirrors the Tier 1 pattern).
pub fn assemble_conic_model(problem: &OptimizationProblem) -> Result<ConicModel, OptimizerError> {
    if !supported_classes().contains(&problem.declared_class) {
        return Err(OptimizerError::MalformedProblem(
            ProblemError::ClassMismatch {
                declared: problem.declared_class,
                reason: "Tier 2 conic model assembly supports continuous socp only".to_string(),
            },
        ));
    }

    let columns = problem
        .variables
        .iter()
        .map(|variable| ConicColumn {
            name: variable.name.clone(),
            lower_bound: variable.lower_bound,
            upper_bound: variable.upper_bound,
        })
        .collect();

    let rows = problem
        .rows
        .iter()
        .map(|row| ConicRow {
            name: row.name.clone(),
            coefficients: row
                .coefficients
                .iter()
                .map(|c| (c.variable, c.coefficient))
                .collect(),
            bounds: row.bounds,
        })
        .collect();

    let cones = problem
        .conic_constraints
        .iter()
        .map(|cone| ConicCone {
            name: cone.name.clone(),
            norm_members: cone
                .norm_members
                .iter()
                .map(|c| (c.variable, c.coefficient))
                .collect(),
            norm_offset: cone.norm_offset,
            rhs_members: cone
                .rhs_members
                .iter()
                .map(|c| (c.variable, c.coefficient))
                .collect(),
            rhs_offset: cone.rhs_offset,
        })
        .collect();

    Ok(ConicModel {
        problem_name: problem.name.clone(),
        declared_class: problem.declared_class,
        minimize: problem.objective.sense == ObjectiveSense::Minimize,
        objective_offset: problem.objective.constant,
        objective_linear: problem
            .objective
            .linear_terms
            .iter()
            .map(|c| (c.variable, c.coefficient))
            .collect(),
        hessian: problem
            .objective
            .quadratic_terms
            .iter()
            .map(|t| (t.row, t.column, t.coefficient))
            .collect(),
        columns,
        rows,
        cones,
    })
}

/// Computes the slack of one second-order cone at a primal point:
/// `slack = (rhs(x))² − ‖diag(a)·x + b‖₂`. For a satisfied cone the slack is
/// ≥ 0; a binding cone has slack 0; a negative slack means the cone is
/// violated at the given point. For a DistFlow branch cone encoded
/// canonically (see [`distflow_branch_residual`]) this equals `4·g_ij`.
///
/// The primal vector must have one entry per formulation variable in
/// formulation order; a shorter/longer vector is a caller bug and is
/// rejected.
pub fn cone_slack(
    cone: &SecondOrderCone,
    primal: &[f64],
) -> Result<f64, OptimizerError> {
    let rhs = cone
        .rhs_members
        .iter()
        .try_fold(cone.rhs_offset, |acc, member| {
            let value = primal.get(member.variable).ok_or_else(|| {
                OptimizerError::BackendUnavailable(format!(
                    "cone '{}' rhs member references variable {} beyond the primal \
                     vector of length {}",
                    cone.name,
                    member.variable,
                    primal.len()
                ))
            })?;
            Result::<f64, OptimizerError>::Ok(acc + member.coefficient * value)
        })?;
    let norm_squared = cone
        .norm_members
        .iter()
        .try_fold(0.0_f64, |acc, member| {
            let value = primal.get(member.variable).ok_or_else(|| {
                OptimizerError::BackendUnavailable(format!(
                    "cone '{}' norm member references variable {} beyond the primal \
                     vector of length {}",
                    cone.name,
                    member.variable,
                    primal.len()
                ))
            })?;
            let term = member.coefficient * value + cone.norm_offset;
            Result::<f64, OptimizerError>::Ok(acc + term * term)
        })?;
    Ok(rhs * rhs - norm_squared)
}

/// One DistFlow branch relaxation residual specification (foundations §7.8):
/// the variable indices carrying the squared send voltage `v_i`, the squared
/// branch current `l_ij`, and the branch flows `P_ij` / `Q_ij` in the
/// formulation's variable order. The roles live with the formulation (e.g.
/// the shared fixtures in [`crate::conic_fixture`]); the frozen problem IR
/// carries no role annotation (recorded as a schema note in
/// 18_clarabel_tier2.md §6 for the frozen-IR review).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchResidualSpec {
    /// Branch identifier (e.g. `"12"` for branch i→j).
    pub branch: String,
    /// Name of the cone encoding this branch's relaxation.
    pub cone_name: String,
    /// Variable index of the squared send voltage `v_i` (per-unit squared).
    pub send_voltage_squared: usize,
    /// Variable index of the squared branch current `l_ij` (per-unit squared).
    pub branch_current_squared: usize,
    /// Variable index of the send active flow `P_ij` (MW).
    pub send_active_flow: usize,
    /// Variable index of the send reactive flow `Q_ij` (MVAr).
    pub send_reactive_flow: usize,
}

/// The foundations §7.8 per-branch SOCP relaxation residual:
/// `g_ij = v_i·l_ij − (P_ij² + Q_ij²)`. `g_ij ≥ 0` means the relaxation held
/// at the given point; `g_ij = 0` means the branch constraint is tight
/// (relaxation exact at this branch); `g_ij < 0` means it is violated. The
/// residual is diagnostic evidence only and does NOT replace the Phase 4 AC
/// power-flow verification.
pub fn distflow_branch_residual(
    send_voltage_squared: f64,
    branch_current_squared: f64,
    send_active_flow_mw: f64,
    send_reactive_flow_mvar: f64,
) -> f64 {
    send_voltage_squared * branch_current_squared
        - send_active_flow_mw * send_active_flow_mw
        - send_reactive_flow_mvar * send_reactive_flow_mvar
}

/// Applies branch specs to a primal solution: one `g_ij` per spec, in spec
/// order. Index errors are caller bugs and are rejected.
pub fn apply_branch_specs(
    specs: &[BranchResidualSpec],
    primal: &[f64],
) -> Result<Vec<(String, f64)>, OptimizerError> {
    specs
        .iter()
        .map(|spec| {
            let pick = |what: &str, index: usize| -> Result<f64, OptimizerError> {
                primal.get(index).copied().ok_or_else(|| {
                    OptimizerError::BackendUnavailable(format!(
                        "branch '{}' residual spec field {what} (index {index}) is \
                         beyond the primal vector of length {}",
                        spec.branch,
                        primal.len()
                    ))
                })
            };
            let v = pick("send_voltage_squared", spec.send_voltage_squared)?;
            let l = pick("branch_current_squared", spec.branch_current_squared)?;
            let p = pick("send_active_flow", spec.send_active_flow)?;
            let q = pick("send_reactive_flow", spec.send_reactive_flow)?;
            Ok((spec.branch.clone(), distflow_branch_residual(v, l, p, q)))
        })
        .collect()
}

/// One per-cone residual entry of a [`RelaxationResidualReport`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConeResidual {
    /// Cone name.
    pub cone_name: String,
    /// The cone slack at the reported primal point
    /// (`(rhs(x))² − ‖diag(a)·x + b‖₂`); for a canonically encoded DistFlow
    /// branch cone this equals `4·g_ij` (foundations §7.8).
    pub slack: f64,
    /// `true` when `slack < −tolerance` — the cone is violated at this point.
    /// Violations are always reported, never hidden.
    pub violated: bool,
    /// `true` when `slack > tolerance` — the cone is satisfied but not tight,
    /// so relaxation exactness is NOT demonstrated on this cone.
    pub exceeds_tolerance: bool,
}

/// The per-cone relaxation residual report computed for every solved Tier 2
/// run against the configured tolerance. Diagnostic evidence only: it does
/// not replace the Phase 4 AC power-flow verification, and a large residual
/// (or a violation) is reported, never hidden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelaxationResidualReport {
    /// The configured tolerance the entries were evaluated against.
    pub tolerance: f64,
    /// One entry per declared cone, in formulation order.
    pub entries: Vec<ConeResidual>,
    /// Names of violated cones (empty when none).
    pub violated_cones: Vec<String>,
    /// Names of cones whose slack exceeds the tolerance (empty when none).
    pub loose_cones: Vec<String>,
}

impl RelaxationResidualReport {
    /// A compact one-line summary for the result warnings channel.
    pub fn summary(&self) -> String {
        format!(
            "per-cone relaxation residual report (tolerance = {:e}): {} cone(s); \
             violated = [{}]; slack beyond tolerance = [{}]; diagnostic evidence \
             only — does not replace the Phase 4 AC power-flow verification",
            self.tolerance,
            self.entries.len(),
            self.violated_cones.join(", "),
            self.loose_cones.join(", "),
        )
    }
}

/// Computes the per-cone relaxation residual report for a solved problem
/// against the configured tolerance. The primal vector must have one entry
/// per formulation variable in formulation order.
pub fn relaxation_residual_report(
    problem: &OptimizationProblem,
    primal: &[f64],
    tolerance: f64,
) -> Result<RelaxationResidualReport, OptimizerError> {
    if primal.len() != problem.variables.len() {
        return Err(OptimizerError::BackendUnavailable(format!(
            "residual computation received {} primal values for {} variables",
            primal.len(),
            problem.variables.len()
        )));
    }
    let mut entries = Vec::with_capacity(problem.conic_constraints.len());
    let mut violated_cones = Vec::new();
    let mut loose_cones = Vec::new();
    for cone in &problem.conic_constraints {
        let slack = cone_slack(cone, primal)?;
        let violated = slack < -tolerance;
        let exceeds_tolerance = slack > tolerance;
        if violated {
            violated_cones.push(cone.name.clone());
        }
        if exceeds_tolerance {
            loose_cones.push(cone.name.clone());
        }
        entries.push(ConeResidual {
            cone_name: cone.name.clone(),
            slack,
            violated,
            exceeds_tolerance,
        });
    }
    Ok(RelaxationResidualReport {
        tolerance,
        entries,
        violated_cones,
        loose_cones,
    })
}

/// Names of linear rows binding at a primal point: rows whose activity lies
/// within `tolerance` of one of their declared bounds. Used by the shared
/// Tier 1 / Tier 2 cross-check record (P3.23 input).
pub fn binding_linear_rows(
    problem: &OptimizationProblem,
    primal: &[f64],
    tolerance: f64,
) -> Result<Vec<String>, OptimizerError> {
    if primal.len() != problem.variables.len() {
        return Err(OptimizerError::BackendUnavailable(format!(
            "binding-row computation received {} primal values for {} variables",
            primal.len(),
            problem.variables.len()
        )));
    }
    let mut binding = Vec::new();
    for row in &problem.rows {
        let activity = row.coefficients.iter().try_fold(0.0_f64, |acc, c| {
            let value = primal.get(c.variable).ok_or_else(|| {
                OptimizerError::BackendUnavailable(format!(
                    "row '{}' references variable {} beyond the primal vector",
                    row.name, c.variable
                ))
            })?;
            Result::<f64, OptimizerError>::Ok(acc + c.coefficient * value)
        })?;
        let near = |target: f64| (activity - target).abs() <= tolerance;
        let is_binding = match row.bounds {
            RowBounds::Lower(v) => near(v),
            RowBounds::Upper(v) => near(v),
            RowBounds::Equality(v) => near(v),
            RowBounds::Range { lower, upper } => near(lower) || near(upper),
        };
        if is_binding {
            binding.push(row.name.clone());
        }
    }
    Ok(binding)
}

/// Names of second-order cones binding at a primal point: cones whose slack
/// is within `tolerance` of zero. Used by the shared Tier 1 / Tier 2
/// cross-check record (P3.23 input).
pub fn binding_cones(
    problem: &OptimizationProblem,
    primal: &[f64],
    tolerance: f64,
) -> Result<Vec<String>, OptimizerError> {
    if primal.len() != problem.variables.len() {
        return Err(OptimizerError::BackendUnavailable(format!(
            "binding-cone computation received {} primal values for {} variables",
            primal.len(),
            problem.variables.len()
        )));
    }
    let mut binding = Vec::new();
    for cone in &problem.conic_constraints {
        if cone_slack(cone, primal)?.abs() <= tolerance {
            binding.push(cone.name.clone());
        }
    }
    Ok(binding)
}

/// The Tier 2 provider: the port implementation scheduling code depends on
/// for continuous SOCP formulations.
pub struct ClarabelTier2Provider {
    config: Tier2ProviderConfig,
    provider_version: String,
    /// Count of solver invocations. Production diagnostics AND the test
    /// proof that the class/integrality gates fire before any solver
    /// invocation (the Tier 1 provider proves the same through its backend
    /// call counter; here the solver is in-process, so the provider counts
    /// its own invocations).
    solver_invocations: AtomicU64,
}

impl ClarabelTier2Provider {
    /// Builds the provider over a provider-owned configuration (the
    /// relaxation-residual tolerance; validated per solve, mirroring the
    /// per-run [`SolverConfig`]).
    pub fn new(config: Tier2ProviderConfig) -> Self {
        Self {
            config,
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
            solver_invocations: AtomicU64::new(0),
        }
    }

    /// Overrides the provider version (for tests and future versioning).
    pub fn with_provider_version(mut self, version: impl Into<String>) -> Self {
        self.provider_version = version.into();
        self
    }

    /// The number of times this provider has invoked the pinned solver.
    /// Gate-rejection paths must leave this at zero (tested).
    pub fn solver_invocations(&self) -> u64 {
        self.solver_invocations.load(Ordering::SeqCst)
    }

    /// Builds the `unsupported` result for a formulation the Tier 2 provider
    /// must never solve: an integer-carrying formulation (Clarabel is a
    /// continuous conic solver — NOT MISOCP), or a declared class outside the
    /// Tier 2 set. No solver invocation happens on this path.
    fn unsupported_result(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
        reason: String,
        mut warnings: Vec<String>,
    ) -> OptimizationResult {
        warnings.push(reason.clone());
        warnings.push(SOCP_EXACTNESS_CAVEAT.to_string());
        self.base_result(problem, config, SolveStatus::Unsupported, warnings)
            .objective_value(None)
            .objective_units(None)
            .primal(Diagnostics::NotAvailable {
                reason: reason.clone(),
            })
            .dual(Diagnostics::NotAvailable {
                reason: reason.clone(),
            })
            .residuals(Diagnostics::NotAvailable {
                reason: reason.clone(),
            })
            .infeasibility(Diagnostics::NotAvailable {
                reason: "not infeasible — no solve was attempted".to_string(),
            })
            .build()
    }

    /// The `unsupported` result for an integer-carrying formulation
    /// (ledger P3.18 step 2: rejected with `unsupported` before any solver
    /// invocation — Clarabel is not MISOCP).
    fn integer_rejection_result(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> OptimizationResult {
        let reason = format!(
            "formulation carries integer/binary variables; the Tier 2 solver is a \
             continuous conic solver with NO integer variables — it is not a MISOCP \
             solver, and an integer formulation is never silently solved as if it \
             were continuous (report §12.3; ledger P3.18). No solve was attempted. \
             Mixed-integer decisions belong to Tier 1 (MILP) or are not built \
             (MISOCP, Tier 3 — P3.20)."
        );
        self.unsupported_result(
            problem,
            config,
            reason,
            vec![
                "Clarabel is a convex conic solver: LP/QP/SOCP/SDP and exponential \
                 or power cones, no integer variables (RESEARCH.md §2; AGENTS.md \
                 solver-class discipline)"
                    .to_string(),
            ],
        )
    }

    /// The `unsupported` result for a declared class outside the Tier 2 set.
    fn class_rejection_result(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> OptimizationResult {
        let reason = format!(
            "declared problem class '{}' is outside the Tier 2 supported set \
             (continuous socp); no solve was attempted. LP/QP/MILP belongs to \
             Tier 1; MISOCP (Tier 3) and nonlinear AC-OPF (Tier 4) are not built \
             in Phase 3 (report §12.3; ledger P3.20).",
            problem.declared_class.as_str()
        );
        self.unsupported_result(problem, config, reason, Vec::new())
    }

    /// Skeleton of a result with provider identity and provenance filled in
    /// (mirrors the Tier 1 builder).
    fn base_result(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
        status: SolveStatus,
        warnings: Vec<String>,
    ) -> OptimizationResultBuilder {
        OptimizationResultBuilder {
            result: OptimizationResult {
                status,
                problem_class: problem.declared_class,
                solver_name: "Clarabel".to_string(),
                solver_version: CLARABEL_SOLVER_VERSION_PIN.to_string(),
                provider_id: PROVIDER_ID.to_string(),
                provider_version: self.provider_version.clone(),
                objective_value: None,
                objective_units: None,
                primal: Diagnostics::NotAvailable {
                    reason: "no solve output".to_string(),
                },
                dual: Diagnostics::NotAvailable {
                    reason: "no solve output".to_string(),
                },
                residuals: Diagnostics::NotAvailable {
                    reason: "no solve output".to_string(),
                },
                infeasibility_evidence: Diagnostics::NotAvailable {
                    reason: "no solve output".to_string(),
                },
                runtime: std::time::Duration::ZERO,
                resources: None,
                candidate_id: Some(problem.provenance.candidate_id),
                input_snapshot_id: Some(problem.provenance.input_snapshot_id.clone()),
                data_timestamp: problem.provenance.data_timestamp,
                grid_model_version: problem.provenance.grid_model_version.clone(),
                uncertainty_and_assumptions: problem.declared_assumptions.clone(),
                warnings,
                determinism: Some(DeterminismRecord {
                    solver_version: CLARABEL_SOLVER_VERSION_PIN.to_string(),
                    thread_count: config.thread_count,
                    time_limit_ms: config.time_limit.map(|limit| limit.as_millis() as u64),
                    config_digest: config.config_digest(),
                }),
            },
        }
    }
}

impl Optimizer for ClarabelTier2Provider {
    fn capabilities(&self) -> SolverCapabilities {
        SolverCapabilities {
            provider_id: PROVIDER_ID.to_string(),
            provider_version: self.provider_version.clone(),
            solver_name: "Clarabel".to_string(),
            solver_version: CLARABEL_SOLVER_VERSION_PIN.to_string(),
            problem_classes: supported_classes(),
            // Integrality is deliberately absent: Clarabel has no integer
            // variables and is not a MISOCP solver (AGENTS.md rule 3.4).
            constraint_features: vec![
                ConstraintFeature::VariableBounds,
                ConstraintFeature::LinearConstraints,
                ConstraintFeature::QuadraticObjective,
                ConstraintFeature::SecondOrderCones,
            ],
            scaling: ScalingDeclaration {
                default_approach: crate::config::ScalingPolicy::ProviderDefault,
                note: "provider default is the pinned solver's data equilibration \
                      (settings field equilibrate_enable); disabling is an explicit \
                      recorded config choice"
                    .to_string(),
            },
            tolerances: TolerancesDeclaration {
                primal_feasibility_default: 1e-8,
                dual_feasibility_default: 1e-8,
                mip_relative_gap_default: None,
                note: "values are the pinned solver's documented defaults \
                      (tol_feas, tol_gap_abs/rel); VERIFY-BUILD at build time. The \
                      pinned solver has no separate dual-feasibility knob; the \
                      optimality-gap tolerances carry it (18_clarabel_tier2.md §5)"
                    .to_string(),
            },
            // No warm-start claim in v0.2.0: the persistent solver API exists
            // in the 0.11.x line (RESEARCH.md §2) but nothing is verified;
            // revisit with a recorded test before flipping this.
            warm_start: WarmStartSupport {
                supported: false,
                note: "not supported in v0.2.0; the persistent solver API exists in \
                      the 0.11.x line (RESEARCH.md §2) but no warm-start claim is \
                      made until verified"
                    .to_string(),
            },
            licensing: LicenseDeclaration {
                solver_license: "Apache-2.0 (Clarabel)".to_string(),
                note: "recorded for the D4 license/dependency audit (still OPEN; \
                      owned by legal + engineering — RESEARCH.md §6 task 5)"
                    .to_string(),
            },
            determinism: DeterminismDeclaration {
                pinned_solver_version: CLARABEL_SOLVER_VERSION_PIN.to_string(),
                thread_count_policy: "max_threads pinned from the provider-owned \
                      SolverConfig; recorded default 1 (multithreaded-KKT \
                      determinism is NOT assumed)"
                    .to_string(),
                notes: vec![
                    SOCP_EXACTNESS_CAVEAT.to_string(),
                    "a solved schedule is a candidate; feasibility is the Phase 4 \
                     gate (report §12.5)"
                        .to_string(),
                ],
            },
            diagnostics: DiagnosticsDeclaration {
                // Verbose printing is disabled and no log capture is built in
                // this batch; the verbatim status text is recorded instead.
                solver_log: false,
                // Clarabel exposes no conflict/IIS refinement.
                infeasibility_conflict_evidence: false,
                // solution.iterations is available per solve.
                iteration_counts: true,
                notes: vec![
                    "no solver-log capture in v0.2.0; the verbatim native status \
                     text is recorded on every result"
                        .to_string(),
                    "no conflict/IIS evidence exists for this solver; an infeasible \
                     result records an explicit not-available reason"
                        .to_string(),
                    SOCP_EXACTNESS_CAVEAT.to_string(),
                ],
            },
        }
    }

    fn solve(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> Result<OptimizationResult, OptimizerError> {
        // 1. Configuration first: both configuration layers must be valid
        //    before any formulation work.
        config.validate().map_err(OptimizerError::InvalidConfig)?;
        self.config.validate().map_err(OptimizerError::InvalidConfig)?;

        // 2. The integrality gate — before ANY solver invocation, and before
        //    structural validation. Clarabel is a continuous conic solver with
        //    no integer variables; an integer formulation is never silently
        //    solved as if it were continuous (ledger P3.18 step 2).
        let has_integer = problem
            .variables
            .iter()
            .any(|v| v.kind == VariableKind::Integer || v.kind == VariableKind::Binary);
        if has_integer {
            return Ok(self.integer_rejection_result(problem, config));
        }

        // 3. Formulation validation: structure + class-declaration
        //    consistency.
        problem.validate().map_err(OptimizerError::MalformedProblem)?;

        // 4. The class gate — before ANY solver invocation. This provider
        //    solves exactly the continuous SOCP class.
        if !supported_classes().contains(&problem.declared_class) {
            return Ok(self.class_rejection_result(problem, config));
        }

        // 5. Formulation assembly (defence in depth) → conic standard form.
        let model = assemble_conic_model(problem)?;
        let data = build_clarabel_data(&model)?;

        // 6. Settings from the provider-owned configuration.
        let settings = build_settings(config);

        // 7. Solve in-process (native Rust crate; no transport seam —
        //    decision D-P3.18-1). The invocation counter is the in-process
        //    analogue of the Tier 1 backend call counter.
        self.solver_invocations.fetch_add(1, Ordering::SeqCst);
        let mut solver = DefaultSolver::new(&data.p, &data.q, &data.a, &data.b, &data.cones, settings)
            .map_err(|error| {
                OptimizerError::BackendUnavailable(format!(
                    "Clarabel model construction failed: {error}"
                ))
            })?;
        let started = Instant::now();
        solver.solve();
        let runtime = started.elapsed();

        // 8. Map onto the §12.6 contract and self-check completeness (a
        //    provider that emits an incomplete record is a provider bug).
        let result = self.map_outcome(problem, config, &data, &solver.solution, runtime)?;
        result
            .validate_complete()
            .map_err(OptimizerError::IncompleteResult)?;
        Ok(result)
    }
}

/// Solver settings translated from the provider-owned configuration
/// (VERIFY-BUILD item 5). `max_threads` is pinned from the configured thread
/// count (default 1) — multithreaded-KKT determinism is not assumed.
fn build_settings(config: &SolverConfig) -> DefaultSettings<f64> {
    let mut settings = DefaultSettings::<f64>::default();
    settings.verbose = false;
    settings.max_threads = config.thread_count as usize;
    if let Some(limit) = config.time_limit {
        settings.time_limit = limit.as_secs_f64();
    }
    settings.tol_feas = config.primal_feasibility_tolerance;
    // The pinned solver has no separate dual-feasibility tolerance; the
    // optimality-gap tolerances carry it (recorded mapping,
    // 18_clarabel_tier2.md §5).
    settings.tol_gap_abs = config.dual_feasibility_tolerance;
    settings.tol_gap_rel = config.dual_feasibility_tolerance;
    if config.scaling == crate::config::ScalingPolicy::Disabled {
        settings.equilibrate_enable = false;
    }
    settings
}

/// The pinned solver's standard-form data plus the bookkeeping needed to map
/// the solution back onto the §12.6 contract.
struct ClarabelData {
    p: CscMatrix<f64>,
    q: Vec<f64>,
    a: CscMatrix<f64>,
    b: Vec<f64>,
    cones: Vec<SupportedConeT<f64>>,
    /// Per IR row: the index into the solver's `z` vector carrying that
    /// row's dual, or `None` for a `Range` row (expanded to two conic rows;
    /// dual recombination is a VERIFY-BUILD item, so duals are reported as
    /// not-available when any range row exists).
    row_z_index: Vec<Option<usize>>,
    /// Total number of conic rows (the `z`/`b` length).
    conic_row_count: usize,
}

/// Translates the assembled conic model into the pinned solver's standard
/// form `minimize ½·xᵀPx + qᵀx` subject to `Ax + s = b`, `s ∈ K`
/// (VERIFY-BUILD items 3 and 4). Translation conventions:
/// - maximisation is solved as the negated minimisation; the sign (and the
///   constant offset) is re-applied when reporting the objective;
/// - `Equality` rows → zero cone; `Lower` rows → nonnegative cone with the
///   coefficients as-is; `Upper` rows → nonnegative cone with negated
///   coefficients; `Range` rows → two nonnegative rows;
/// - each second-order cone `‖diag(a)·x + b‖₂ ≤ cᵀx + d` becomes one zero-cone
///   row for the scalar `t` (`t = cᵀx + d`) followed by a
///   `SecondOrderConeT(1 + k)` block over `(t, norm member 1, …, norm member
///   k)` — the scalar is the FIRST element of the block (VERIFY-BUILD item 2,
///   per the crate's `example_socp.rs`);
/// - column bounds become anonymous nonnegative rows (the solver has no
///   variable-bound concept).
/// Named IR rows are emitted first, then bound rows, then cones, so the
/// linear-row duals occupy the first block of `z` and `row_z_index` stays
/// exact.
fn build_clarabel_data(model: &ConicModel) -> Result<ClarabelData, OptimizerError> {
    let n = model.columns.len();
    if n == 0 {
        return Err(OptimizerError::MalformedProblem(ProblemError::NoVariables));
    }
    let sign = if model.minimize { 1.0 } else { -1.0 };

    // Objective: q and P (VERIFY-BUILD item 4 for the P convention).
    let q: Vec<f64> = {
        let mut q = vec![0.0; n];
        for (variable, coefficient) in &model.objective_linear {
            q[*variable] += sign * coefficient;
        }
        q
    };
    let p: CscMatrix<f64> = if model.hessian.is_empty() {
        CscMatrix::zeros((n, n))
    } else {
        let mut i = Vec::new();
        let mut j = Vec::new();
        let mut v = Vec::new();
        for (row, column, coefficient) in &model.hessian {
            let value = sign * coefficient;
            if row == column {
                // ½·P_ii·x_i² = c_ii·x_i²  ⇒  P_ii = 2·c_ii.
                i.push(*row);
                j.push(*column);
                v.push(2.0 * value);
            } else {
                // ½·xᵀPx contributes P_ij·x_i·x_j for symmetric P ⇒
                // P_ij = P_ji = c_ij.
                i.push(*row);
                j.push(*column);
                v.push(value);
                i.push(*column);
                j.push(*row);
                v.push(value);
            }
        }
        CscMatrix::new_from_triplets(n, n, i, j, v)
    };

    // Constraints: linear rows first, then cones.
    let mut triplets: Vec<(usize, usize, f64)> = Vec::new();
    let mut b: Vec<f64> = Vec::new();
    let mut cones: Vec<SupportedConeT<f64>> = Vec::new();
    let mut row_z_index: Vec<Option<usize>> = Vec::with_capacity(model.rows.len());

    for row in &model.rows {
        match row.bounds {
            RowBounds::Equality(value) => {
                let index = b.len();
                for (variable, coefficient) in &row.coefficients {
                    triplets.push((index, *variable, *coefficient));
                }
                b.push(value);
                cones.push(SupportedConeT::ZeroConeT(1));
                row_z_index.push(Some(index));
            }
            RowBounds::Lower(value) => {
                let index = b.len();
                for (variable, coefficient) in &row.coefficients {
                    triplets.push((index, *variable, *coefficient));
                }
                b.push(value);
                cones.push(SupportedConeT::NonnegativeConeT(1));
                row_z_index.push(Some(index));
            }
            RowBounds::Upper(value) => {
                let index = b.len();
                for (variable, coefficient) in &row.coefficients {
                    triplets.push((index, *variable, -coefficient));
                }
                b.push(value);
                cones.push(SupportedConeT::NonnegativeConeT(1));
                row_z_index.push(Some(index));
            }
            RowBounds::Range { lower, upper } => {
                // Two conic rows; the per-row dual recombination is a
                // VERIFY-BUILD item, so this row is marked None.
                let lower_index = b.len();
                for (variable, coefficient) in &row.coefficients {
                    triplets.push((lower_index, *variable, *coefficient));
                }
                b.push(lower);
                cones.push(SupportedConeT::NonnegativeConeT(1));

                let upper_index = b.len();
                for (variable, coefficient) in &row.coefficients {
                    triplets.push((upper_index, *variable, -coefficient));
                }
                b.push(upper);
                cones.push(SupportedConeT::NonnegativeConeT(1));

                row_z_index.push(None);
            }
        }
    }

    // Variable bounds: the pinned solver has no variable-bound concept, so
    // every declared column bound becomes an anonymous nonnegative row (x ≥ L
    // as `+1·x ≥ L`; x ≤ U as `−1·x ≥ −U`). They are appended after the named
    // IR rows and before the cones, so `row_z_index` (recorded above) is
    // unaffected; a bound that is unbounded on a side contributes no row.
    for (variable, column) in model.columns.iter().enumerate() {
        if let Some(lower) = column.lower_bound {
            let index = b.len();
            triplets.push((index, variable, 1.0));
            b.push(lower);
            cones.push(SupportedConeT::NonnegativeConeT(1));
        }
        if let Some(upper) = column.upper_bound {
            let index = b.len();
            triplets.push((index, variable, -1.0));
            b.push(-upper);
            cones.push(SupportedConeT::NonnegativeConeT(1));
        }
    }

    for cone in &model.cones {
        // The scalar t: equality row `t − cᵀx = d`, so s_t = cᵀx + d = rhs(x).
        let t_index = b.len();
        for (variable, coefficient) in &cone.rhs_members {
            triplets.push((t_index, *variable, -coefficient));
        }
        b.push(cone.rhs_offset);
        cones.push(SupportedConeT::ZeroConeT(1));

        // The norm members: row `−aᵢ·xᵢ + sᵢ = b_offset`, so
        // sᵢ = aᵢ·xᵢ + b_offset (the norm argument).
        for (variable, coefficient) in &cone.norm_members {
            let index = b.len();
            triplets.push((index, *variable, -coefficient));
            b.push(cone.norm_offset);
        }
        let dim = 1 + cone.norm_members.len();
        if dim < 2 {
            return Err(OptimizerError::MalformedProblem(
                ProblemError::ClassMismatch {
                    declared: model.declared_class,
                    reason: format!(
                        "cone '{}' has no norm members; a second-order cone needs at \
                         least one",
                        cone.name
                    ),
                },
            ));
        }
        cones.push(SupportedConeT::SecondOrderConeT(dim));
    }

    let conic_row_count = b.len();
    let a = CscMatrix::new_from_triplets(
        conic_row_count,
        n,
        triplets.iter().map(|(r, _, _)| *r).collect(),
        triplets.iter().map(|(_, c, _)| *c).collect(),
        triplets.iter().map(|(_, _, v)| *v).collect(),
    );

    Ok(ClarabelData {
        p,
        q,
        a,
        b,
        cones,
        row_z_index,
        conic_row_count,
    })
}

impl ClarabelTier2Provider {
    /// Maps the pinned solver's solution onto the §12.6 result record.
    fn map_outcome(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
        data: &ClarabelData,
        solution: &clarabel::solver::DefaultSolution<f64>,
        runtime: std::time::Duration,
    ) -> Result<OptimizationResult, OptimizerError> {
        let status = map_clarabel_status(solution.status);
        let column_count = problem.variables.len();

        // Vector-length contracts: a violating payload is a solver-contract
        // failure, never a silently truncated result (mirrors Tier 1).
        if solution.x.len() != column_count {
            return Err(OptimizerError::BackendUnavailable(format!(
                "solver returned {} primal values for {} variables",
                solution.x.len(),
                column_count
            )));
        }
        if solution.z.len() != data.conic_row_count {
            return Err(OptimizerError::BackendUnavailable(format!(
                "solver returned {} duals for {} conic rows",
                solution.z.len(),
                data.conic_row_count
            )));
        }

        let mut warnings: Vec<String> = Vec::new();
        // The verbatim native status is always recorded so every mapping
        // decision is auditable (mirrors the Tier 1 pattern).
        warnings.push(format!(
            "Clarabel status '{:?}' mapped to §12.6 status '{}'",
            solution.status,
            status.as_str()
        ));

        let solved = matches!(status, SolveStatus::Optimal | SolveStatus::Feasible);
        if status == SolveStatus::Feasible {
            warnings.push(
                "the solver terminated with a reduced-accuracy solution \
                 ('AlmostSolved'); the result carries no proven-optimality claim"
                    .to_string(),
            );
        }
        if status == SolveStatus::NumericalFailure {
            warnings.push(
                "the six-status §12.6 vocabulary is closed and has no 'unbounded' \
                 value: a dual-infeasibility certificate implies an unbounded \
                 primal objective, and reduced-accuracy infeasibility \
                 certificates are not published as verdicts — the outcome is \
                 reported as numerical_failure with this recorded note, never as \
                 infeasible (18_clarabel_tier2.md §4)"
                    .to_string(),
            );
        }
        if status == SolveStatus::Timeout
            && matches!(solution.status, SolverStatus::MaxIterations)
        {
            warnings.push(
                "resource limit was the iteration budget, not the time limit"
                    .to_string(),
            );
        }

        // Objective value: the IR constant is re-applied and the
        // maximisation sign flip reversed (VERIFY-BUILD item 3).
        let objective_value = if solved {
            Some(if model_sense_is_minimize(problem) {
                solution.obj_val + problem.objective.constant
            } else {
                -solution.obj_val + problem.objective.constant
            })
        } else {
            None
        };

        let primal = if solved {
            Diagnostics::Available(PrimalDiagnostics {
                values: solution
                    .x
                    .iter()
                    .zip(problem.variables.iter())
                    .map(|(value, variable)| VariableValue {
                        name: variable.name.clone(),
                        unit: variable.unit.canonical_unit().to_string(),
                        value: *value,
                    })
                    .collect(),
            })
        } else {
            Diagnostics::NotAvailable {
                reason: format!(
                    "native status '{:?}' produced no publishable primal solution",
                    solution.status
                ),
            }
        };

        // Duals: linear-row multipliers are extracted from the first block of
        // z (rows are emitted before cones). Range rows expand to two conic
        // rows and their dual recombination is a VERIFY-BUILD item, so any
        // range row forces duals to not-available rather than a partially
        // recomputed report. Reduced costs are not exposed by the pinned
        // solver; the vector is left empty and the omission recorded.
        let has_range_row = problem
            .rows
            .iter()
            .any(|row| matches!(row.bounds, RowBounds::Range { .. }));
        let dual = if solved && !has_range_row {
            let row_duals: Vec<f64> = data
                .row_z_index
                .iter()
                .map(|index| solution.z[index.expect("no range rows checked above")])
                .collect();
            Diagnostics::Available(DualDiagnostics {
                reduced_costs: Vec::new(),
                row_duals,
            })
        } else if solved && has_range_row {
            Diagnostics::NotAvailable {
                reason: "the formulation declares range rows, which expand to two \
                         conic rows; the per-row dual recombination is a \
                         VERIFY-BUILD item, so duals are recorded as not available \
                         rather than partially recomputed"
                    .to_string(),
            }
        } else {
            Diagnostics::NotAvailable {
                reason: format!(
                    "native status '{:?}' produced no publishable duals",
                    solution.status
                ),
            }
        };

        let residuals = Diagnostics::Available(ResidualReport {
            max_primal_violation: Some(solution.r_prim),
            max_dual_violation: Some(solution.r_dual),
            iteration_count: Some(u64::from(solution.iterations)),
        });

        let infeasibility = if status == SolveStatus::Infeasible {
            Diagnostics::NotAvailable {
                reason: "the pinned conic solver exposes no conflict/IIS \
                         refinement, so no conflict evidence can be recorded; \
                         no relaxed schedule is returned (ledger P3.18; decision \
                         note 18 §4)"
                    .to_string(),
            }
        } else {
            Diagnostics::NotAvailable {
                reason: format!("not applicable to status '{}'", status.as_str()),
            }
        };

        // Per-cone relaxation residual report (foundations §7.8): diagnostic
        // evidence, computed for every solved run against the configured
        // tolerance and reported in the warnings channel, never hidden.
        if solved {
            let report = relaxation_residual_report(
                problem,
                &solution.x,
                self.config.relaxation_residual_tolerance,
            )?;
            warnings.push(report.summary());
            for entry in &report.entries {
                if entry.violated {
                    warnings.push(format!(
                        "cone '{}' is VIOLATED at the reported solution (slack = \
                         {:e}); this is diagnostic evidence and must be \
                         investigated — the result must not be treated as \
                         acceptable",
                        entry.cone_name, entry.slack
                    ));
                } else if entry.exceeds_tolerance {
                    warnings.push(format!(
                        "cone '{}' has slack {:e} beyond the configured tolerance \
                         {:e}: the relaxation is not tight on this cone and \
                         exactness is NOT demonstrated",
                        entry.cone_name,
                        entry.slack,
                        self.config.relaxation_residual_tolerance
                    ));
                }
            }
            // The §12.5 caveat is recorded verbatim on every solved result.
            warnings.push(SOCP_EXACTNESS_CAVEAT.to_string());
            warnings.push(
                "an optimal/feasible status proves optimality for the declared \
                 conic formulation only, never for the underlying nonlinear AC \
                 problem"
                    .to_string(),
            );
        }

        if config.mip_relative_gap.is_some() {
            warnings.push(
                "a MIP relative gap was configured but the Tier 2 solve is a \
                 continuous conic problem; the gap is not applicable and was \
                 ignored (recorded)"
                    .to_string(),
            );
        }

        Ok(self
            .base_result(problem, config, status, warnings)
            .runtime(runtime)
            .objective_value(objective_value)
            .objective_units(Some(problem.objective.declared_units.clone()))
            .primal(primal)
            .dual(dual)
            .residuals(residuals)
            .infeasibility(infeasibility)
            .build())
    }
}

/// Whether the formulation's objective is declared as a minimisation (used
/// for the sign/constant re-application of the reported objective value).
fn model_sense_is_minimize(problem: &OptimizationProblem) -> bool {
    problem.objective.sense == ObjectiveSense::Minimize
}

/// Internal builder so each result-construction path stays readable and no
/// §12.6 field is forgotten (mirrors the Tier 1 provider's builder).
struct OptimizationResultBuilder {
    result: OptimizationResult,
}

impl OptimizationResultBuilder {
    fn runtime(mut self, runtime: std::time::Duration) -> Self {
        self.result.runtime = runtime;
        self
    }

    fn objective_value(mut self, value: Option<f64>) -> Self {
        self.result.objective_value = value;
        self
    }

    fn objective_units(mut self, units: Option<String>) -> Self {
        self.result.objective_units = units;
        self
    }

    fn primal(mut self, primal: Diagnostics<PrimalDiagnostics>) -> Self {
        self.result.primal = primal;
        self
    }

    fn dual(mut self, dual: Diagnostics<DualDiagnostics>) -> Self {
        self.result.dual = dual;
        self
    }

    fn residuals(mut self, residuals: Diagnostics<ResidualReport>) -> Self {
        self.result.residuals = residuals;
        self
    }

    fn infeasibility(
        mut self,
        infeasibility: Diagnostics<InfeasibilityEvidence>,
    ) -> Self {
        self.result.infeasibility_evidence = infeasibility;
        self
    }

    fn build(self) -> OptimizationResult {
        self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    #[test]
    fn clarabel_status_mapping_table_is_the_recorded_one() {
        let cases = [
            (SolverStatus::Solved, SolveStatus::Optimal),
            (SolverStatus::AlmostSolved, SolveStatus::Feasible),
            (SolverStatus::PrimalInfeasible, SolveStatus::Infeasible),
            (SolverStatus::AlmostPrimalInfeasible, SolveStatus::NumericalFailure),
            (SolverStatus::DualInfeasible, SolveStatus::NumericalFailure),
            (SolverStatus::AlmostDualInfeasible, SolveStatus::NumericalFailure),
            (SolverStatus::MaxIterations, SolveStatus::Timeout),
            (SolverStatus::MaxTime, SolveStatus::Timeout),
            (SolverStatus::NumericalError, SolveStatus::NumericalFailure),
            (SolverStatus::InsufficientProgress, SolveStatus::NumericalFailure),
            (SolverStatus::CallbackTerminated, SolveStatus::NumericalFailure),
            (SolverStatus::Unsolved, SolveStatus::NumericalFailure),
        ];
        for (native, expected) in cases {
            assert_eq!(map_clarabel_status(native), expected);
        }
        // The mapping is exhaustive over SolverStatus — this closure would
        // fail to compile if the pinned version added a variant without a
        // recorded mapping decision (mirrors the Tier 1 guard).
        let _exhaustive = |status: SolverStatus| match status {
            SolverStatus::Solved
            | SolverStatus::AlmostSolved
            | SolverStatus::PrimalInfeasible
            | SolverStatus::AlmostPrimalInfeasible
            | SolverStatus::DualInfeasible
            | SolverStatus::AlmostDualInfeasible
            | SolverStatus::MaxIterations
            | SolverStatus::MaxTime
            | SolverStatus::NumericalError
            | SolverStatus::InsufficientProgress
            | SolverStatus::CallbackTerminated
            | SolverStatus::Unsolved => map_clarabel_status(status),
        };
    }

    #[test]
    fn default_provider_config_carries_the_recorded_tolerance() {
        let config = Tier2ProviderConfig::default();
        assert_eq!(
            config.relaxation_residual_tolerance,
            DEFAULT_RELAXATION_RESIDUAL_TOLERANCE
        );
        config.validate().expect("default config validates");
    }

    #[test]
    fn invalid_provider_configs_are_rejected() {
        let mut config = Tier2ProviderConfig::default();
        config.relaxation_residual_tolerance = 0.0;
        assert_eq!(
            config.validate(),
            Err(crate::config::ConfigError::NonPositiveTolerance {
                field: "relaxation_residual_tolerance"
            })
        );
        config.relaxation_residual_tolerance = f64::NAN;
        assert_eq!(
            config.validate(),
            Err(crate::config::ConfigError::NonFiniteValue {
                field: "relaxation_residual_tolerance"
            })
        );
    }

    #[test]
    fn assembly_rejects_non_conic_declarations_as_defence_in_depth() {
        // Unreachable through `solve` (the gates run first); the hard stop
        // fails closed if any future call path skips the gates.
        let error = assemble_conic_model(&fixture::reference_lp()).unwrap_err();
        assert!(matches!(
            error,
            OptimizerError::MalformedProblem(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn capabilities_declare_only_the_tier2_class_and_never_integrality() {
        let provider = ClarabelTier2Provider::new(Tier2ProviderConfig::default());
        let capabilities = provider.capabilities();
        assert_eq!(
            capabilities.problem_classes,
            vec![ProblemClass::ContinuousSecondOrderConeProgram]
        );
        assert!(!capabilities.solver_name.is_empty());
        assert_eq!(capabilities.solver_version, CLARABEL_SOLVER_VERSION_PIN);
        assert!(!capabilities.warm_start.supported);
        assert!(
            capabilities
                .licensing
                .solver_license
                .starts_with("Apache-2.0 (Clarabel)")
        );
        assert!(
            !capabilities
                .constraint_features
                .contains(&ConstraintFeature::Integrality),
            "the capability surface must never declare integrality: Clarabel has \
             no integer variables and is not a MISOCP solver"
        );
        assert!(capabilities.diagnostics.iteration_counts);
        assert!(!capabilities.diagnostics.solver_log);
        assert!(!capabilities.diagnostics.infeasibility_conflict_evidence);
        // The §12.5 exactness caveat is part of the recorded capability
        // surface.
        assert!(capabilities
            .determinism
            .notes
            .iter()
            .any(|note| note == SOCP_EXACTNESS_CAVEAT));
    }

    #[test]
    fn distflow_formula_and_canonical_encoding_agree() {
        // Canonical encoding: ‖(2P, 2Q, v − l)‖₂ ≤ v + l ⇒
        // slack = 4·(v·l − P² − Q²) = 4·g_ij (foundations §7.8).
        let (v, l, p, q) = (1.0_f64, 4.5_f64, 2.0_f64, 0.5_f64);
        let cone = SecondOrderCone {
            name: "branch_cone".to_string(),
            norm_members: vec![
                crate::problem::Coefficient { variable: 2, coefficient: 2.0 },
                crate::problem::Coefficient { variable: 3, coefficient: 2.0 },
                crate::problem::Coefficient { variable: 0, coefficient: 1.0 },
                crate::problem::Coefficient { variable: 1, coefficient: -1.0 },
            ],
            norm_offset: 0.0,
            rhs_members: vec![
                crate::problem::Coefficient { variable: 0, coefficient: 1.0 },
                crate::problem::Coefficient { variable: 1, coefficient: 1.0 },
            ],
            rhs_offset: 0.0,
        };
        let primal = vec![v, l, p, q];
        let slack = cone_slack(&cone, &primal).expect("primal aligned");
        let g = distflow_branch_residual(v, l, p, q);
        assert!((slack / 4.0 - g).abs() < 1e-12);
        assert!((g - 0.25).abs() < 1e-12, "hand-computed g_ij = 0.25");
    }

    #[test]
    fn branch_specs_apply_the_formula_per_branch() {
        let specs = vec![
            BranchResidualSpec {
                branch: "12".to_string(),
                cone_name: "branch_cone".to_string(),
                send_voltage_squared: 0,
                branch_current_squared: 1,
                send_active_flow: 2,
                send_reactive_flow: 3,
            },
        ];
        let primal = vec![1.0, 4.0, 2.0, 0.5];
        let residuals = apply_branch_specs(&specs, &primal).expect("indices in range");
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].0, "12");
        // g = 1.0·4.0 − 4 − 0.25 = −0.25: violated — reported, never hidden.
        assert!((residuals[0].1 - (-0.25)).abs() < 1e-12);
    }

    #[test]
    fn residual_report_flags_violated_and_loose_cones_per_tolerance() {
        let problem = crate::conic_fixture::distflow_branch_socp();
        // Primal at l = 4.5: slack = 4·0.25 = 1.0 — satisfied but loose at a
        // tight tolerance, binding at a loose one.
        let primal = vec![1.0, 4.5, 2.0, 0.5];

        let report = relaxation_residual_report(&problem, &primal, 1e-6)
            .expect("primal aligned");
        assert_eq!(report.entries.len(), 1);
        assert!((report.entries[0].slack - 1.0).abs() < 1e-12);
        assert!(!report.entries[0].violated);
        assert!(report.entries[0].exceeds_tolerance);
        assert_eq!(report.loose_cones, vec!["branch_relaxation_cone".to_string()]);
        assert!(report.violated_cones.is_empty());

        let report = relaxation_residual_report(&problem, &primal, 2.0)
            .expect("primal aligned");
        assert!(!report.entries[0].exceeds_tolerance);
        assert!(report.loose_cones.is_empty());

        // Violated point (l = 4.0): flagged regardless of tolerance.
        let primal = vec![1.0, 4.0, 2.0, 0.5];
        let report = relaxation_residual_report(&problem, &primal, 1e-6)
            .expect("primal aligned");
        assert!((report.entries[0].slack - (-1.0)).abs() < 1e-12);
        assert!(report.entries[0].violated);
        assert_eq!(report.violated_cones, vec!["branch_relaxation_cone".to_string()]);
        // The summary names the diagnostic caveat.
        assert!(report.summary().contains("Phase 4 AC power-flow"));
    }

    #[test]
    fn misaligned_primal_vectors_are_rejected_not_guessed() {
        let problem = crate::conic_fixture::distflow_branch_socp();
        let error = relaxation_residual_report(&problem, &[1.0, 4.5], 1e-6).unwrap_err();
        assert!(matches!(error, OptimizerError::BackendUnavailable(_)));
    }
}
