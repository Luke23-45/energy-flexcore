// P3.17 — The Tier 1 (LP / convex QP / MILP) provider (report §12.3, §9.1).
//
// Provider-path decision D-P3.17-1
// (ledger/phase-03-flexibility-scheduling/17_optimizer_port.md): Tier 1 solves
// execute in the Python optimization worker through the pinned binding
// (highspy 1.15.1, verified 2026-11-07, RESEARCH.md §1); this provider talks
// to that worker only through the transport-neutral
// [`Tier1Backend`](crate::backend::Tier1Backend) seam. The adapter itself
// lands with P3.19.
//
// Solver-class discipline (AGENTS.md rule 3.4 — absolute):
// - HiGHS is an LP/QP/MIP solver. It is NOT an SOCP solver. This provider
//   supports exactly the declared classes LP, convex QP, and MILP; a
//   formulation declaring SOCP, MISOCP, or nonlinear is rejected with the
//   §12.6 status `unsupported` **before any backend invocation** (tested,
//   including a call counter proving the backend was never touched).
// - A conic formulation cannot even be assembled into a Tier 1 model: the
//   `SecondOrderCone` IR element is never consumed on this path.
//
// Infeasibility policy: an `infeasible` outcome returns the backend's
// conflict evidence where the backend exposes it, never a relaxed schedule
// (P3.19 step 3 policy, implemented here).

use crate::backend::{
    BackendError, BackendOptions, BackendOutcome, BackendStatus, ColumnSpec, RowSpec,
    Tier1Backend, Tier1Model,
};
use crate::config::SolverConfig;
use crate::port::{
    ConstraintFeature, DiagnosticsDeclaration, DeterminismDeclaration, LicenseDeclaration,
    Optimizer, OptimizerError, ScalingDeclaration, SolverCapabilities, TolerancesDeclaration,
    WarmStartSupport,
};
use crate::problem::{
    ObjectiveSense, OptimizationProblem, ProblemClass, ProblemError, VariableValue,
};
use crate::result::{
    DeterminismRecord, Diagnostics, DualDiagnostics, InfeasibilityEvidence, OptimizationResult,
    PrimalDiagnostics, ResidualReport, SolveStatus,
};
use std::sync::Arc;
use std::time::Instant;

/// The pinned Tier 1 solver version as recorded upstream: highspy 1.15.1
/// (PyPI release files read 2026-11-07; RESEARCH.md §1). This is the **Python
/// binding's** version — no Rust binding version was researched (RESEARCH.md
/// §6 task 6, closed for v0.2.0 by decision D-P3.17-1). Recorded on every
/// result via the determinism record.
pub const HIGHS_SOLVER_VERSION_PIN: &str = "1.15.1";

/// Provider identity. The `builtin.` prefix matches the §8.1 built-in
/// provider placement (`plugins/builtin/optimizer-highs` in the target tree;
/// the worker adapter lands with P3.19).
pub const PROVIDER_ID: &str = "builtin.optimizer-highs-tier1";

/// The six §12.6 statuses the Tier 1 backend statuses map onto (decision
/// note §3). Kept next to the mapping for review.
///
/// | BackendStatus            | §12.6 status       |
/// |--------------------------|--------------------|
/// | Optimal                  | optimal            |
/// | Feasible                 | feasible           |
/// | Infeasible               | infeasible         |
/// | TimeLimitReached         | timeout            |
/// | IterationLimitReached    | timeout (verbatim status text recorded) |
/// | Unbounded                | numerical_failure  |
/// | UnboundedOrInfeasible    | numerical_failure  |
/// | ModelError / SolveError / Unknown | numerical_failure |
fn map_backend_status(status: BackendStatus) -> SolveStatus {
    match status {
        BackendStatus::Optimal => SolveStatus::Optimal,
        BackendStatus::Feasible => SolveStatus::Feasible,
        BackendStatus::Infeasible => SolveStatus::Infeasible,
        BackendStatus::TimeLimitReached | BackendStatus::IterationLimitReached => {
            SolveStatus::Timeout
        }
        BackendStatus::Unbounded
        | BackendStatus::UnboundedOrInfeasible
        | BackendStatus::ModelError
        | BackendStatus::SolveError
        | BackendStatus::Unknown => SolveStatus::NumericalFailure,
    }
}

/// The Tier 1 provider: the port implementation scheduling code depends on.
pub struct HighsTier1Provider {
    backend: Arc<dyn Tier1Backend>,
    provider_version: String,
}

impl HighsTier1Provider {
    /// Builds the provider over a backend seam implementation (the Python
    /// optimization worker adapter in production, P3.19; test doubles in
    /// tests).
    pub fn new(backend: Arc<dyn Tier1Backend>) -> Self {
        Self {
            backend,
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Overrides the provider version (for tests and future versioning).
    pub fn with_provider_version(mut self, version: impl Into<String>) -> Self {
        self.provider_version = version.into();
        self
    }

    /// The supported Tier 1 classes (§12.3 Tier 1 only).
    fn supported_classes() -> Vec<ProblemClass> {
        vec![
            ProblemClass::LinearProgram,
            ProblemClass::ConvexQuadraticProgram,
            ProblemClass::MixedIntegerLinearProgram,
        ]
    }

    /// Assembles the Tier 1 model from a validated formulation. The class
    /// gate has already run; this re-check is defence in depth so a conic or
    /// nonlinear declaration can never reach model assembly.
    fn assemble_model(problem: &OptimizationProblem) -> Result<Tier1Model, OptimizerError> {
        if !Self::supported_classes().contains(&problem.declared_class) {
            // Unreachable through `solve` (the gate runs first); kept as a
            // hard stop so any future call path fails closed.
            return Err(OptimizerError::MalformedProblem(
                ProblemError::ClassMismatch {
                    declared: problem.declared_class,
                    reason: "Tier 1 model assembly supports lp/convex_qp/milp only"
                        .to_string(),
                },
            ));
        }

        let columns = problem
            .variables
            .iter()
            .map(|variable| ColumnSpec {
                name: variable.name.clone(),
                kind: variable.kind,
                // IR unbounded = None; backend infinities stand for unbounded
                // sides and are mapped to the pinned solver's infinity by the
                // adapter (VERIFY-BUILD item 4, decision note §6).
                lower_bound: variable.lower_bound.unwrap_or(f64::NEG_INFINITY),
                upper_bound: variable.upper_bound.unwrap_or(f64::INFINITY),
            })
            .collect();

        let rows = problem
            .rows
            .iter()
            .map(|row| {
                let (lower, upper) = row.bounds.as_lower_upper();
                RowSpec {
                    name: row.name.clone(),
                    coefficients: row
                        .coefficients
                        .iter()
                        .map(|c| (c.variable, c.coefficient))
                        .collect(),
                    lower,
                    upper,
                }
            })
            .collect();

        let objective_linear = problem
            .objective
            .linear_terms
            .iter()
            .map(|c| (c.variable, c.coefficient))
            .collect();

        Ok(Tier1Model {
            problem_name: problem.name.clone(),
            declared_class: problem.declared_class,
            minimize: problem.objective.sense == ObjectiveSense::Minimize,
            objective_offset: problem.objective.constant,
            objective_linear,
            hessian: problem
                .objective
                .quadratic_terms
                .iter()
                .map(|t| crate::backend::HessianTerm {
                    row: t.row,
                    column: t.column,
                    coefficient: t.coefficient,
                })
                .collect(),
            columns,
            rows,
        })
    }

    fn backend_options(config: &SolverConfig) -> BackendOptions {
        BackendOptions {
            time_limit_seconds: config.time_limit.map(|d| d.as_secs_f64()),
            thread_count: config.thread_count,
            primal_feasibility_tolerance: config.primal_feasibility_tolerance,
            dual_feasibility_tolerance: config.dual_feasibility_tolerance,
            mip_relative_gap: config.mip_relative_gap,
            scaling: config.scaling,
        }
    }

    /// Builds the `unsupported` result for a declared class outside the
    /// Tier 1 set. No solver invocation happens on this path.
    fn unsupported_result(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> OptimizationResult {
        let reason = format!(
            "declared problem class '{}' is outside the Tier 1 supported set \
             (lp/convex_qp/milp); no solve was attempted. The Tier 1 solver is an \
             LP/QP/MIP solver, not an SOCP or nonlinear solver (report §12.3; \
             decision note 17_optimizer_port.md §2).",
            problem.declared_class.as_str()
        );
        let mut warnings = vec![reason.clone()];
        if problem.declared_class == ProblemClass::ContinuousSecondOrderConeProgram {
            warnings.push(
                "a continuous SOCP formulation belongs to Tier 2 (P3.18); this \
                 rejection is the class-discipline gate, not a formulation error"
                    .to_string(),
            );
        }
        if matches!(
            problem.declared_class,
            ProblemClass::MixedIntegerSecondOrderConeProgram | ProblemClass::NonlinearProgram
        ) {
            warnings.push(
                "MISOCP (Tier 3) and nonlinear AC-OPF (Tier 4) are not built in \
                 Phase 3; see ledger P3.20 for the recorded boundary"
                    .to_string(),
            );
        }
        self.base_result(problem, config, SolveStatus::Unsupported, warnings)
            .objective_value(None)
            .primal(Diagnostics::NotAvailable { reason: reason.clone() })
            .dual(Diagnostics::NotAvailable { reason: reason.clone() })
            .residuals(Diagnostics::NotAvailable { reason: reason.clone() })
            .infeasibility(Diagnostics::NotAvailable {
                reason: "not infeasible — no solve was attempted".to_string(),
            })
            .build()
    }

    /// Skeleton of a result with provider identity and provenance filled in.
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
                solver_name: "HiGHS".to_string(),
                solver_version: HIGHS_SOLVER_VERSION_PIN.to_string(),
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
                    solver_version: HIGHS_SOLVER_VERSION_PIN.to_string(),
                    thread_count: config.thread_count,
                    time_limit_ms: config
                        .time_limit
                        .map(|limit| limit.as_millis() as u64),
                    config_digest: config.config_digest(),
                }),
            },
        }
    }

    /// Maps a backend outcome onto the §12.6 result record.
    fn map_outcome(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
        outcome: BackendOutcome,
        runtime: std::time::Duration,
    ) -> Result<OptimizationResult, OptimizerError> {
        let status = map_backend_status(outcome.status);

        // Vector-length contracts (backend.rs): a violating payload is a
        // backend contract failure, never a silently truncated result.
        let column_count = problem.variables.len();
        let row_count = problem.rows.len();
        if let Some(values) = &outcome.column_values {
            if values.len() != column_count {
                return Err(OptimizerError::BackendUnavailable(format!(
                    "backend returned {} primal values for {} variables",
                    values.len(),
                    column_count
                )));
            }
        }
        if let Some(costs) = &outcome.reduced_costs {
            if costs.len() != column_count {
                return Err(OptimizerError::BackendUnavailable(format!(
                    "backend returned {} reduced costs for {} variables",
                    costs.len(),
                    column_count
                )));
            }
        }
        if let Some(duals) = &outcome.row_duals {
            if duals.len() != row_count {
                return Err(OptimizerError::BackendUnavailable(format!(
                    "backend returned {} row duals for {} rows",
                    duals.len(),
                    row_count
                )));
            }
        }

        let mut warnings = outcome.warnings.clone();
        // The verbatim backend status is always recorded so every mapping
        // decision is auditable (especially unbounded/limit mappings,
        // decision note §3).
        warnings.push(format!(
            "backend status '{}' mapped to §12.6 status '{}'",
            outcome.solver_status_text,
            status.as_str()
        ));
        if matches!(
            outcome.status,
            BackendStatus::Unbounded | BackendStatus::UnboundedOrInfeasible
        ) {
            warnings.push(
                "the six-status §12.6 vocabulary is closed and has no 'unbounded' \
                 value; an unbounded/ambiguous objective is reported as \
                 numerical_failure with this recorded note, never as infeasible \
                 (decision note §3)"
                    .to_string(),
            );
        }
        if outcome.status == BackendStatus::IterationLimitReached {
            warnings.push(
                "resource limit was the iteration budget, not the time limit"
                    .to_string(),
            );
        }

        let objective_units = problem.objective.declared_units.clone();
        let primal = match outcome.column_values.clone() {
            Some(values) => Diagnostics::Available(PrimalDiagnostics {
                values: values
                    .into_iter()
                    .zip(problem.variables.iter())
                    .map(|(value, variable)| VariableValue {
                        name: variable.name.clone(),
                        unit: variable.unit.canonical_unit().to_string(),
                        value,
                    })
                    .collect(),
            }),
            None => Diagnostics::NotAvailable {
                reason: format!(
                    "backend status '{}' produced no primal solution",
                    outcome.solver_status_text
                ),
            },
        };

        let dual = match (outcome.reduced_costs.clone(), outcome.row_duals.clone()) {
            (Some(reduced_costs), Some(row_duals)) => {
                Diagnostics::Available(DualDiagnostics {
                    reduced_costs,
                    row_duals,
                })
            }
            (reduced_costs, row_duals) => Diagnostics::NotAvailable {
                reason: format!(
                    "backend status '{}' did not expose duals (reduced_costs: {}, \
                     row_duals: {})",
                    outcome.solver_status_text,
                    reduced_costs.is_some(),
                    row_duals.is_some()
                ),
            },
        };

        let residuals = if outcome.max_primal_violation.is_some()
            || outcome.max_dual_violation.is_some()
            || outcome.iteration_count.is_some()
        {
            Diagnostics::Available(ResidualReport {
                max_primal_violation: outcome.max_primal_violation,
                max_dual_violation: outcome.max_dual_violation,
                iteration_count: outcome.iteration_count,
            })
        } else {
            Diagnostics::NotAvailable {
                reason: format!(
                    "backend status '{}' reported no residual or effort data",
                    outcome.solver_status_text
                ),
            }
        };

        let infeasibility = if status == SolveStatus::Infeasible {
            match outcome.conflict.clone() {
                Some(conflict) => Diagnostics::Available(InfeasibilityEvidence {
                    conflict_variables: conflict.variables,
                    conflict_rows: conflict.rows,
                    backend_report: conflict.report,
                }),
                None => Diagnostics::NotAvailable {
                    reason: format!(
                        "backend status '{}' exposed no conflict/IIS evidence for \
                         this run (VERIFY-BUILD item 2, decision note §4); no \
                         relaxed schedule is returned",
                        outcome.solver_status_text
                    ),
                },
            }
        } else {
            Diagnostics::NotAvailable {
                reason: format!(
                    "not applicable to status '{}'",
                    status.as_str()
                ),
            }
        };

        let objective_value = outcome.objective_value;

        Ok(self
            .base_result(problem, config, status, warnings)
            .runtime(runtime)
            .resources(outcome.solver_log_excerpt.clone())
            .objective_value(objective_value)
            .objective_units(Some(objective_units))
            .primal(primal)
            .dual(dual)
            .residuals(residuals)
            .infeasibility(infeasibility)
            .build())
    }
}

impl Optimizer for HighsTier1Provider {
    fn capabilities(&self) -> SolverCapabilities {
        SolverCapabilities {
            provider_id: PROVIDER_ID.to_string(),
            provider_version: self.provider_version.clone(),
            solver_name: "HiGHS".to_string(),
            solver_version: HIGHS_SOLVER_VERSION_PIN.to_string(),
            problem_classes: Self::supported_classes(),
            constraint_features: vec![
                ConstraintFeature::VariableBounds,
                ConstraintFeature::LinearConstraints,
                ConstraintFeature::QuadraticObjective,
                ConstraintFeature::Integrality,
            ],
            scaling: ScalingDeclaration {
                default_approach: crate::config::ScalingPolicy::ProviderDefault,
                note: "provider default is the pinned solver's automatic scaling; \
                      disabling is an explicit recorded config choice"
                    .to_string(),
            },
            tolerances: TolerancesDeclaration {
                primal_feasibility_default: 1e-7,
                dual_feasibility_default: 1e-7,
                mip_relative_gap_default: None,
                note: "defaults are the pinned solver's documented defaults; \
                      VERIFY-BUILD at adapter build time (decision note §6 item 5)"
                    .to_string(),
            },
            // No warm-start claim in v0.2.0: nothing verified about warm-start
            // behaviour through the worker path; revisit at P3.19 with a
            // recorded test before flipping this.
            warm_start: WarmStartSupport {
                supported: false,
                note: "not supported in v0.2.0; no warm-start claim is made \
                      through the worker path until verified (P3.19)"
                    .to_string(),
            },
            licensing: LicenseDeclaration {
                solver_license: "MIT (HiGHS)".to_string(),
                note: "recorded for the D4 license/dependency audit (still OPEN; \
                      owned by legal + engineering — decision note §1)"
                    .to_string(),
            },
            determinism: DeterminismDeclaration {
                pinned_solver_version: HIGHS_SOLVER_VERSION_PIN.to_string(),
                thread_count_policy: "fixed thread count from the provider-owned \
                      SolverConfig; recorded default 1"
                    .to_string(),
                notes: vec![
                    "parallel-MIP determinism at 1.15.x is NOT assumed (RESEARCH.md \
                     §6 task 1); P3.19 pins or disables parallel MIP in determinism \
                     tests"
                        .to_string(),
                ],
            },
            diagnostics: DiagnosticsDeclaration {
                solver_log: true,
                infeasibility_conflict_evidence: true,
                iteration_counts: true,
                notes: vec![
                    "conflict/IIS evidence depends on the pinned binding's API \
                     surface (VERIFY-BUILD item 2); when absent for a run the \
                     result records an explicit not-available reason"
                        .to_string(),
                ],
            },
        }
    }

    fn solve(
        &self,
        problem: &OptimizationProblem,
        config: &SolverConfig,
    ) -> Result<OptimizationResult, OptimizerError> {
        // 1. Configuration first: an invalid config must fail before any
        //    formulation work.
        config.validate().map_err(OptimizerError::InvalidConfig)?;

        // 2. Formulation validation: structure + class-declaration
        //    consistency.
        problem.validate().map_err(OptimizerError::MalformedProblem)?;

        // 3. The class gate — before ANY backend interaction. HiGHS is an
        //    LP/QP/MIP solver and is never invoked on an SOCP, MISOCP, or
        //    nonlinear formulation.
        if !Self::supported_classes().contains(&problem.declared_class) {
            return Ok(self.unsupported_result(problem, config));
        }

        // 4. Formulation assembly → Tier 1 model.
        let model = Self::assemble_model(problem)?;

        // 5. Solve through the seam. Transport/contract failures surface as
        //    OptimizerError::BackendUnavailable (infrastructure), never as a
        //    fabricated §12.6 status.
        let options = Self::backend_options(config);
        let started = Instant::now();
        let outcome = self
            .backend
            .solve(&model, &options)
            .map_err(|error| match error {
                BackendError::Unavailable(message) => {
                    OptimizerError::BackendUnavailable(message)
                }
                BackendError::ContractViolation(message) => {
                    OptimizerError::BackendUnavailable(message)
                }
            })?;
        let runtime = started.elapsed();

        // 6. Map onto the §12.6 contract and self-check completeness (a
        //    provider that emits an incomplete record is a provider bug).
        let result = self.map_outcome(problem, config, outcome, runtime)?;
        result
            .validate_complete()
            .map_err(OptimizerError::IncompleteResult)?;
        Ok(result)
    }
}

/// Internal builder so each result-construction path stays readable and no
/// §12.6 field is forgotten.
struct OptimizationResultBuilder {
    result: OptimizationResult,
}

impl OptimizationResultBuilder {
    fn runtime(mut self, runtime: std::time::Duration) -> Self {
        self.result.runtime = runtime;
        self
    }

    fn resources(mut self, resources: Option<String>) -> Self {
        self.result.resources = resources;
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

    #[test]
    fn backend_status_mapping_table_is_the_recorded_one() {
        let cases = [
            (BackendStatus::Optimal, SolveStatus::Optimal),
            (BackendStatus::Feasible, SolveStatus::Feasible),
            (BackendStatus::Infeasible, SolveStatus::Infeasible),
            (BackendStatus::TimeLimitReached, SolveStatus::Timeout),
            (BackendStatus::IterationLimitReached, SolveStatus::Timeout),
            (BackendStatus::Unbounded, SolveStatus::NumericalFailure),
            (
                BackendStatus::UnboundedOrInfeasible,
                SolveStatus::NumericalFailure,
            ),
            (BackendStatus::ModelError, SolveStatus::NumericalFailure),
            (BackendStatus::SolveError, SolveStatus::NumericalFailure),
            (BackendStatus::Unknown, SolveStatus::NumericalFailure),
        ];
        for (backend, expected) in cases {
            assert_eq!(map_backend_status(backend), expected);
        }
        // The mapping is exhaustive over BackendStatus — this closure would
        // fail to compile if a variant were added without a recorded mapping
        // decision (decision note §3).
        let _exhaustive = |status: BackendStatus| match status {
            BackendStatus::Optimal
            | BackendStatus::Feasible
            | BackendStatus::Infeasible
            | BackendStatus::Unbounded
            | BackendStatus::UnboundedOrInfeasible
            | BackendStatus::TimeLimitReached
            | BackendStatus::IterationLimitReached
            | BackendStatus::ModelError
            | BackendStatus::SolveError
            | BackendStatus::Unknown => map_backend_status(status),
        };
    }

    #[test]
    fn capabilities_declare_only_the_tier1_classes() {
        // A backend implementation is needed to construct the provider; this
        // unit test uses a minimal inline describe-only double (the full
        // behavioural double lives in the integration tests).
        struct NoopBackend;
        impl Tier1Backend for NoopBackend {
            fn solve(
                &self,
                _model: &Tier1Model,
                _options: &BackendOptions,
            ) -> Result<BackendOutcome, BackendError> {
                Err(BackendError::Unavailable("not used in this test".to_string()))
            }
            fn describe(&self) -> crate::backend::BackendDescription {
                crate::backend::BackendDescription {
                    name: "noop".to_string(),
                    version: "0".to_string(),
                    notes: Vec::new(),
                }
            }
        }
        let provider = HighsTier1Provider::new(Arc::new(NoopBackend));
        let capabilities = provider.capabilities();
        assert_eq!(
            capabilities.problem_classes,
            vec![
                ProblemClass::LinearProgram,
                ProblemClass::ConvexQuadraticProgram,
                ProblemClass::MixedIntegerLinearProgram,
            ]
        );
        assert!(
            !capabilities
                .problem_classes
                .contains(&ProblemClass::ContinuousSecondOrderConeProgram)
        );
        assert_eq!(capabilities.solver_name, "HiGHS");
        assert_eq!(capabilities.solver_version, HIGHS_SOLVER_VERSION_PIN);
        assert!(!capabilities.warm_start.supported);
        assert!(
            capabilities
                .licensing
                .solver_license
                .starts_with("MIT (HiGHS)")
        );
        assert!(capabilities.diagnostics.infeasibility_conflict_evidence);
        // The capability surface must not mention conic support: the Tier 1
        // provider rejects SOCP/MISOCP/nonlinear declarations.
        assert!(
            !capabilities
                .constraint_features
                .contains(&ConstraintFeature::SecondOrderCones)
        );
    }
}
