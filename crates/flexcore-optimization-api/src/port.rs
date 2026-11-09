// P3.17 — The `Optimizer` port (report §9.1, §7.4).
//
// ```text
// Optimizer
//   supported_problem_classes() -> SolverCapabilities
//   solve(problem, options) -> OptimizationResult
// ```
//
// Scheduling depends on this contract only; solvers are providers behind it
// (report §5.2). A provider **declares** what it supports — problem classes,
// constraint features, scaling, tolerances, warm-start support, licensing,
// determinism, diagnostic outputs (the §7.4 solver provider contract) — and
// **rejects** everything else with the §12.6 status `unsupported`, never by
// approximating.
//
// This module contains **no solver-specific type**. The dependency-direction
// test (`tests/dependency_direction.rs`) enforces that the port-surface
// modules, this one included, never name a concrete solver or binding.

use crate::config::{ConfigError, ScalingPolicy, SolverConfig};
use crate::problem::{OptimizationProblem, ProblemClass, ProblemError};
use crate::result::{OptimizationResult, ResultCompletenessError};

/// A constraint/feature kind a provider can declare support for (§7.4
/// "supported constraint types").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintFeature {
    /// Explicit variable bounds.
    VariableBounds,
    /// Linear constraint rows (equality / inequality / range).
    LinearConstraints,
    /// Quadratic objective terms (convex QP objective).
    QuadraticObjective,
    /// Integer / binary variables (integrality).
    Integrality,
    /// Second-order cone constraints (Tier 2 IR element; a Tier 1 provider
    /// must not declare this).
    SecondOrderCones,
}

/// Warm-start declaration (§7.4 "warm-start support").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WarmStartSupport {
    /// Whether the provider accepts a warm start.
    pub supported: bool,
    /// Recorded note (e.g. what is accepted, or why not).
    pub note: String,
}

/// Licensing declaration (§7.4 "licensing"; D4 audit input).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LicenseDeclaration {
    /// The solver's license (name of the solver included for audit clarity).
    pub solver_license: String,
    /// Recorded note feeding the D4 license/dependency audit; the formal
    /// audit itself is OPEN and owned by legal + engineering.
    pub note: String,
}

/// Determinism declaration (§7.4 "determinism"; §19.2 layer 5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeterminismDeclaration {
    /// The pinned solver version the provider runs.
    pub pinned_solver_version: String,
    /// The thread-count policy (a fixed count from the config; the recorded
    /// default is 1).
    pub thread_count_policy: String,
    /// Recorded determinism notes / open verification items (e.g.
    /// parallel-MIP determinism is not assumed until verified — RESEARCH.md
    /// §6 task 1).
    pub notes: Vec<String>,
}

/// Diagnostic-output declaration (§7.4 "diagnostic outputs" — the
/// diagnostics channel).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticsDeclaration {
    /// Whether the provider can return a solver-log excerpt.
    pub solver_log: bool,
    /// Whether the provider can return infeasibility conflict evidence.
    pub infeasibility_conflict_evidence: bool,
    /// Whether the provider reports iteration counts.
    pub iteration_counts: bool,
    /// Recorded notes about the diagnostics channel.
    pub notes: Vec<String>,
}

/// Tolerances declared by the provider (§7.4 "tolerances").
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TolerancesDeclaration {
    /// Default primal feasibility tolerance.
    pub primal_feasibility_default: f64,
    /// Default dual feasibility / optimality tolerance.
    pub dual_feasibility_default: f64,
    /// Default MIP relative gap handling (`None` = solver default).
    pub mip_relative_gap_default: Option<f64>,
    /// Recorded note (defaults trace to the pinned solver's documentation;
    /// VERIFY-BUILD at adapter build time).
    pub note: String,
}

/// Scaling declaration (§7.4 "scaling").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScalingDeclaration {
    /// The provider's default scaling approach.
    pub default_approach: ScalingPolicy,
    /// Recorded note.
    pub note: String,
}

/// The full provider capability declaration (§9.1 `SolverCapabilities`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SolverCapabilities {
    /// Provider identifier.
    pub provider_id: String,
    /// Provider version.
    pub provider_version: String,
    /// Solver name behind the provider.
    pub solver_name: String,
    /// Pinned solver version.
    pub solver_version: String,
    /// Declared supported problem classes; anything else is rejected with
    /// `unsupported`.
    pub problem_classes: Vec<ProblemClass>,
    /// Declared supported constraint features.
    pub constraint_features: Vec<ConstraintFeature>,
    /// Scaling declaration.
    pub scaling: ScalingDeclaration,
    /// Tolerances declaration.
    pub tolerances: TolerancesDeclaration,
    /// Warm-start declaration.
    pub warm_start: WarmStartSupport,
    /// Licensing declaration.
    pub licensing: LicenseDeclaration,
    /// Determinism declaration.
    pub determinism: DeterminismDeclaration,
    /// Diagnostics-channel declaration.
    pub diagnostics: DiagnosticsDeclaration,
}

impl SolverCapabilities {
    /// Whether the provider declares support for the given problem class.
    /// Providers must reject undeclared classes with `unsupported` even if
    /// this returns `true` by mistake; this helper is for callers choosing a
    /// provider, not a substitute for the provider's own gate.
    pub fn supports_class(&self, class: ProblemClass) -> bool {
        self.problem_classes.contains(&class)
    }
}

/// Errors the port itself reports (as opposed to solver outcomes, which are
/// mapped onto the six §12.6 statuses and returned in-band as an
/// [`OptimizationResult`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizerError {
    /// The formulation is structurally invalid or its class declaration does
    /// not match its body. Never a solver outcome.
    MalformedProblem(ProblemError),
    /// The solver configuration is invalid. Never a solver outcome.
    InvalidConfig(ConfigError),
    /// The backend seam failed at the transport/contract level before or
    /// during a solve (e.g. the worker is unreachable, or returned a payload
    /// violating its contract). Infrastructure failure — not one of the six
    /// §12.6 solver-outcome statuses, which describe solver results.
    BackendUnavailable(String),
    /// Provider-internal guard: a provider produced a result that fails the
    /// §12.6 completeness validation. This is a provider bug, not a solver
    /// outcome; report it as such.
    IncompleteResult(ResultCompletenessError),
}

impl std::fmt::Display for OptimizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizerError::MalformedProblem(error) => {
                write!(f, "malformed problem: {error}")
            }
            OptimizerError::InvalidConfig(error) => {
                write!(f, "invalid solver config: {error}")
            }
            OptimizerError::BackendUnavailable(message) => {
                write!(f, "solver backend unavailable: {message}")
            }
            OptimizerError::IncompleteResult(error) => {
                write!(f, "provider produced an incomplete result (provider bug): {error}")
            }
        }
    }
}

impl std::error::Error for OptimizerError {}

/// The `Optimizer` port (report §9.1): the contract the scheduling
/// application depends on and solver providers implement.
///
/// Implementations must be object-safe (`Arc<dyn Optimizer>` in composition
/// roots) and sync — no port names an async runtime (flexcore-ports rule,
/// applied here as well).
pub trait Optimizer: Send + Sync {
    /// Declares what this provider supports (§9.1 `supported_problem_classes`,
    /// §7.4 provider contract: problem classes, constraint types, scaling,
    /// tolerances, warm start, licensing, determinism, diagnostics).
    fn capabilities(&self) -> SolverCapabilities;

    /// Solves the given formulation under the given provider-owned
    /// configuration (§9.1 `solve(problem, options)`).
    ///
    /// Solver outcomes — including "cannot solve this class" — are returned
    /// in-band as an [`OptimizationResult`] with one of the six §12.6
    /// statuses; [`OptimizerError`] is reserved for malformed input,
    /// invalid configuration, and backend infrastructure/contract failures.
    ///
    /// A provider must never invoke its solver on a declared problem class it
    /// does not support: the class gate fires first and returns `unsupported`
    /// (tested for the Tier 1 provider).
    fn solve(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> Result<OptimizationResult, OptimizerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port is held as a trait object in composition roots; object safety
    /// is part of the contract.
    #[test]
    fn optimizer_trait_is_object_safe() {
        fn takes_dyn(_: &dyn Optimizer) {}
        let _ = takes_dyn;
    }

    #[test]
    fn supports_class_reflects_the_declared_list() {
        let capabilities = SolverCapabilities {
            provider_id: "test.provider".to_string(),
            provider_version: "0.1.0".to_string(),
            solver_name: "test-solver".to_string(),
            solver_version: "0.0.0".to_string(),
            problem_classes: vec![ProblemClass::LinearProgram],
            constraint_features: vec![ConstraintFeature::VariableBounds],
            scaling: ScalingDeclaration {
                default_approach: ScalingPolicy::ProviderDefault,
                note: String::new(),
            },
            tolerances: TolerancesDeclaration {
                primal_feasibility_default: 1e-7,
                dual_feasibility_default: 1e-7,
                mip_relative_gap_default: None,
                note: String::new(),
            },
            warm_start: WarmStartSupport {
                supported: false,
                note: String::new(),
            },
            licensing: LicenseDeclaration {
                solver_license: "test-license".to_string(),
                note: String::new(),
            },
            determinism: DeterminismDeclaration {
                pinned_solver_version: "0.0.0".to_string(),
                thread_count_policy: "fixed".to_string(),
                notes: Vec::new(),
            },
            diagnostics: DiagnosticsDeclaration {
                solver_log: false,
                infeasibility_conflict_evidence: false,
                iteration_counts: false,
                notes: Vec::new(),
            },
        };
        assert!(capabilities.supports_class(ProblemClass::LinearProgram));
        assert!(!capabilities.supports_class(ProblemClass::NonlinearProgram));
    }
}
