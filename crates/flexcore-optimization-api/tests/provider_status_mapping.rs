// P3.17 — Tier 1 provider tests: the six-status mapping table, the class
// gate (SOCP/MISOCP/nonlinear rejection before any solver invocation), the
// infeasibility-evidence policy, and the §12.6 completeness of every
// produced record.
//
// Fixture-driven (scripted backend double over the `Tier1Backend` seam): the
// deterministic-replay and real-solve tests land with P3.19/P3.26 (decision
// note §6 item 6); these tests exercise everything around the solve —
// validation, assembly, gate, mapping — end to end through the port.

mod common;

use common::{outcome, scripted, ScriptedBackend};
use flexcore_optimization_api::backend::{
    BackendError, BackendOptions, BackendStatus, ConflictEvidence, Tier1Backend, Tier1Model,
};
use flexcore_optimization_api::config::{ScalingPolicy, SolverConfig};
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::port::{Optimizer, OptimizerError};
use flexcore_optimization_api::result::SolveStatus;
use flexcore_optimization_api::problem::ProblemClass;
use std::sync::{Arc, Mutex};

fn provider(backend: Arc<ScriptedBackend>) -> flexcore_optimization_api::highs_provider::HighsTier1Provider {
    flexcore_optimization_api::highs_provider::HighsTier1Provider::new(backend)
}

/// The full six-status mapping table, driven through the port: each backend
/// outcome a pinned solver can reach is mapped onto exactly the recorded
/// §12.6 status (decision note §3).
#[test]
fn six_status_mapping_table_through_the_port() {
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
    let problem = fixture::reference_lp();
    for (backend_status, expected_status) in cases {
        // An optimal/feasible outcome carries a solution payload (the
        // completeness contract requires it); the other statuses carry none,
        // which is exactly what the record must then declare as
        // not-available-with-reason.
        let solved = matches!(
            backend_status,
            BackendStatus::Optimal | BackendStatus::Feasible
        );
        let mut backend_outcome = outcome(
            backend_status,
            "fixture status text",
            if solved { Some(8.0) } else { None },
            if solved {
                Some(vec![0.0; problem.variables.len()])
            } else {
                None
            },
        );
        if solved {
            backend_outcome.max_primal_violation = Some(0.0);
        }
        let backend = scripted(vec![Ok(backend_outcome)]);
        let result = provider(backend.clone())
            .solve(&problem, &SolverConfig::default())
            .expect("backend outcome maps in-band");
        assert_eq!(
            result.status, expected_status,
            "backend status {backend_status:?} must map to {expected_status:?}"
        );
        assert_eq!(backend.solve_count(), 1);
        // Every record is complete, including the failure statuses.
        result
            .validate_complete()
            .expect("every mapped record is complete");
        // The verbatim backend status text is recorded on every mapping.
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("fixture status text")),
            "verbatim backend status must be recorded for {backend_status:?}"
        );
    }
}

/// The reference LP solved through the provider path with the hand-computed
/// optimum: proves formulation assembly, the LP path, primal diagnostics
/// with canonical units, objective units, and determinism metadata.
#[test]
fn reference_lp_solves_through_the_provider_path() {
    let optimum = fixture::reference_lp_optimum();
    let backend = scripted(vec![Ok(BackendOutcome {
        status: BackendStatus::Optimal,
        objective_value: Some(optimum.objective_value),
        column_values: Some(optimum.column_values.clone()),
        reduced_costs: Some(vec![0.0, 1.0]),
        row_duals: Some(vec![2.0]),
        max_primal_violation: Some(0.0),
        max_dual_violation: Some(0.0),
        iteration_count: Some(3),
        solver_status_text: "Optimal".to_string(),
        conflict: None,
        solver_log_excerpt: None,
        warnings: Vec::new(),
    })]);
    let backend_handle = backend.clone();
    let result = provider(backend)
        .solve(&fixture::reference_lp(), &SolverConfig::default())
        .expect("reference LP solves");

    assert_eq!(result.status, SolveStatus::Optimal);
    assert_eq!(
        result.objective_value,
        Some(optimum.objective_value),
        "hand-computed LP optimum"
    );
    assert_eq!(result.objective_units.as_deref(), Some("synthetic_cost_unit"));

    let primal = match &result.primal {
        flexcore_optimization_api::result::Diagnostics::Available(primal) => primal,
        other => panic!("optimal result must carry primal diagnostics, got {other:?}"),
    };
    for (expected, value) in optimum.column_values.iter().zip(primal.values.iter()) {
        assert_eq!(value.value, *expected);
    }
    assert_eq!(primal.values[0].unit, "MW", "canonical unit in diagnostics");
    assert!(result.dual.is_available(), "duals were provided");
    assert!(result.residuals.is_available());

    // The assembled model is the LP the fixture declares: continuous
    // columns, one row, no Hessian.
    let calls = backend_handle.recorded_calls();
    assert_eq!(calls.len(), 1);
    let (model, options) = &calls[0];
    assert_eq!(model.declared_class, ProblemClass::LinearProgram);
    assert!(model.hessian.is_empty());
    assert_eq!(model.columns.len(), 2);
    assert_eq!(model.rows.len(), 1);
    assert!(model.minimize);
    assert_eq!(options.thread_count, 1, "recorded determinism default");
    assert_eq!(options.scaling, ScalingPolicy::ProviderDefault);

    // Determinism metadata recorded per result.
    let determinism = result.determinism.as_ref().expect("determinism record");
    assert_eq!(determinism.solver_version, "1.15.1");
    assert_eq!(determinism.thread_count, 1);
    assert!(determinism.config_digest.starts_with("fnv1a64:"));
}

/// The QP tracking-term path: the Hessian is assembled and the §12.6 record
/// carries the hand-computed optimum.
#[test]
fn reference_qp_solves_through_the_provider_path() {
    let optimum = fixture::reference_qp_optimum();
    let backend = scripted(vec![Ok(outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(optimum.objective_value),
        Some(optimum.column_values.clone()),
    ))]);
    let backend_handle = backend.clone();
    let result = provider(backend)
        .solve(&fixture::reference_qp(), &SolverConfig::default())
        .expect("reference QP solves");

    assert_eq!(result.status, SolveStatus::Optimal);
    assert_eq!(result.objective_value, Some(0.5));
    assert_eq!(
        result.problem_class,
        ProblemClass::ConvexQuadraticProgram
    );

    let (model, _) = &backend_handle.recorded_calls()[0];
    assert_eq!(model.hessian.len(), 1, "QP hessian assembled");
    assert_eq!(model.hessian[0].row, 0);
    assert_eq!(model.hessian[0].column, 0);
    assert!((model.hessian[0].coefficient - 0.5).abs() < f64::EPSILON);
}

/// The MILP path: integrality is assembled into the model and a
/// gap-terminated MIP maps to `feasible` with primal diagnostics.
#[test]
fn reference_milp_solves_through_the_provider_path() {
    let optimum = fixture::reference_milp_optimum();
    let backend = scripted(vec![Ok(BackendOutcome {
        status: BackendStatus::Feasible,
        objective_value: Some(optimum.objective_value),
        column_values: Some(optimum.column_values.clone()),
        reduced_costs: None,
        row_duals: None,
        max_primal_violation: Some(0.0),
        max_dual_violation: None,
        iteration_count: Some(41),
        solver_status_text: "Feasible (within gap)".to_string(),
        conflict: None,
        solver_log_excerpt: None,
        warnings: vec!["MIP terminated within the relative gap".to_string()],
    }))]);
    let backend_handle = backend.clone();
    let result = provider(backend)
        .solve(&fixture::reference_milp(), &SolverConfig::default())
        .expect("reference MILP solves");

    assert_eq!(result.status, SolveStatus::Feasible);
    assert_eq!(result.objective_value, Some(7.0));
    assert!(result.dual.not_available_reason().is_some(), "a gap-terminated MIP need not expose duals, but records a reason");

    let (model, _) = &backend_handle.recorded_calls()[0];
    assert_eq!(model.declared_class, ProblemClass::MixedIntegerLinearProgram);
    let integer_columns = model
        .columns
        .iter()
        .filter(|c| c.kind != flexcore_optimization_api::problem::VariableKind::Continuous)
        .count();
    assert_eq!(integer_columns, 2, "the two mode binaries are integral columns");
}

/// The class gate: SOCP, MISOCP, and nonlinear declarations are rejected with
/// the in-band `unsupported` status and the backend is never invoked — the
/// solver is never handed a formulation outside LP/QP/MILP.
#[test]
fn provider_rejects_socp_misocp_nonlinear() {
    for problem in [
        fixture::reference_socp_declaration(),
        fixture::reference_misocp_declaration(),
        fixture::reference_nonlinear_declaration(),
    ] {
        let backend = scripted(Vec::new());
        let result = provider(backend.clone())
            .solve(&problem, &SolverConfig::default())
            .expect("unsupported is an in-band status, not an error");

        assert_eq!(
            result.status,
            SolveStatus::Unsupported,
            "class {}",
            problem.declared_class.as_str()
        );
        assert_eq!(result.problem_class, problem.declared_class);
        assert_eq!(backend.solve_count(), 0, "no solver invocation for a rejected class");
        assert!(result.objective_value.is_none());
        assert!(result.primal.not_available_reason().is_some());
        assert!(result.dual.not_available_reason().is_some());
        assert!(result.residuals.not_available_reason().is_some());
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("not an SOCP or nonlinear solver")),
            "the rejection names the solver-class discipline"
        );
        result
            .validate_complete()
            .expect("the unsupported record is complete");
    }
}

/// A backend-reported infeasible outcome carries the conflict evidence; no
/// relaxed schedule is fabricated.
#[test]
fn infeasible_result_carries_conflict_evidence() {
    let backend = scripted(vec![Ok(BackendOutcome {
        status: BackendStatus::Infeasible,
        objective_value: None,
        column_values: None,
        reduced_costs: None,
        row_duals: None,
        max_primal_violation: None,
        max_dual_violation: None,
        iteration_count: Some(7),
        solver_status_text: "Infeasible".to_string(),
        conflict: Some(ConflictEvidence {
            variables: vec!["bess_a_discharge".to_string()],
            rows: vec![
                "portfolio_export_floor".to_string(),
                "capacity_envelope".to_string(),
            ],
            report: "conflict refinement: portfolio_export_floor vs capacity_envelope"
                .to_string(),
        }),
        solver_log_excerpt: None,
        warnings: Vec::new(),
    }))]);
    let result = provider(backend)
        .solve(&fixture::reference_lp(), &SolverConfig::default())
        .expect("infeasible maps in-band");

    assert_eq!(result.status, SolveStatus::Infeasible);
    match &result.infeasibility_evidence {
        flexcore_optimization_api::result::Diagnostics::Available(evidence) => {
            assert_eq!(evidence.conflict_rows.len(), 2);
            assert!(evidence.backend_report.contains("conflict refinement"));
        }
        other => panic!("expected evidence, got {other:?}"),
    }
    // Never a relaxed schedule: no primal solution is fabricated.
    assert!(!result.primal.is_available());
    assert!(result.objective_value.is_none());
    result
        .validate_complete()
        .expect("infeasible record is complete");
}

/// When the backend does not expose conflict evidence, the result records an
/// explicit not-available reason (never silence, never a relaxed schedule).
#[test]
fn infeasible_without_evidence_records_an_explicit_reason() {
    let backend = scripted(vec![Ok(outcome(
        BackendStatus::Infeasible,
        "Infeasible",
        None,
        None,
    ))]);
    let result = provider(backend)
        .solve(&fixture::reference_lp(), &SolverConfig::default())
        .expect("infeasible maps in-band");
    assert_eq!(result.status, SolveStatus::Infeasible);
    let reason = result
        .infeasibility_evidence
        .not_available_reason()
        .expect("explicit reason recorded");
    assert!(reason.contains("no conflict/IIS evidence"));
    result
        .validate_complete()
        .expect("record is complete with the reason present");
}

/// A malformed formulation (class declaration not matching the body) is an
/// error, not a solver outcome — and the backend is never invoked.
#[test]
fn malformed_problem_is_an_error_before_any_solve() {
    let mut problem = fixture::reference_lp();
    problem.variables[0].kind = flexcore_optimization_api::problem::VariableKind::Integer;
    let backend = scripted(Vec::new());
    let error = provider(backend.clone())
        .solve(&problem, &SolverConfig::default())
        .expect_err("class-declaration mismatch is an error");
    match error {
        OptimizerError::MalformedProblem(_) => {}
        other => panic!("expected MalformedProblem, got {other:?}"),
    }
    assert_eq!(backend.solve_count(), 0);
}

/// An invalid configuration is an error before any solve.
#[test]
fn invalid_config_is_an_error_before_any_solve() {
    let mut config = SolverConfig::default();
    config.thread_count = 0;
    let backend = scripted(Vec::new());
    let error = provider(backend.clone())
        .solve(&fixture::reference_lp(), &config)
        .expect_err("zero threads is invalid");
    assert!(matches!(error, OptimizerError::InvalidConfig(_)));
    assert_eq!(backend.solve_count(), 0);
}

/// A backend transport failure surfaces as `BackendUnavailable`, never as a
/// fabricated §12.6 status.
#[test]
fn backend_transport_failure_is_an_error_not_a_status() {
    let backend = scripted(vec![Err(BackendError::Unavailable(
        "worker unreachable".to_string(),
    ))]);
    let error = provider(backend)
        .solve(&fixture::reference_lp(), &SolverConfig::default())
        .expect_err("transport failure is an error");
    assert!(matches!(error, OptimizerError::BackendUnavailable(_)));
}

/// A backend payload violating its vector-length contract is rejected — the
/// provider never publishes a misaligned solution vector.
#[test]
fn backend_contract_violation_is_rejected() {
    let backend = scripted(vec![Ok(outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(8.0),
        Some(vec![4.0]), // 1 value for 2 variables
    ))]);
    let error = provider(backend)
        .solve(&fixture::reference_lp(), &SolverConfig::default())
        .expect_err("wrong-length payload is a contract failure");
    assert!(matches!(error, OptimizerError::BackendUnavailable(_)));
}

/// Provider configuration flows into the backend options: time limit, gap,
/// tolerances, scaling, thread count.
#[test]
fn config_flows_into_backend_options() {
    let backend = scripted(vec![Ok(outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(8.0),
        Some(vec![4.0, 0.0]),
    ))]);
    let mut config = SolverConfig::default();
    config.time_limit = Some(std::time::Duration::from_millis(1_500));
    config.mip_relative_gap = Some(0.01);
    config.thread_count = 2;
    config.scaling = ScalingPolicy::Disabled;
    provider(backend.clone())
        .solve(&fixture::reference_lp(), &config)
        .expect("solves");

    let options: Vec<BackendOptions> = backend.recorded_options();
    assert_eq!(options.len(), 1);
    let options = &options[0];
    assert!((options.time_limit_seconds.unwrap() - 1.5).abs() < 1e-12);
    assert_eq!(options.mip_relative_gap, Some(0.01));
    assert_eq!(options.thread_count, 2);
    assert_eq!(options.scaling, ScalingPolicy::Disabled);
    assert_eq!(options.primal_feasibility_tolerance, 1e-7);
}

/// The provider declares the §7.4 capability contract: classes, constraint
/// features, warm start, licensing, determinism, diagnostics.
#[test]
fn capabilities_declare_the_full_provider_contract() {
    let backend = scripted(Vec::new());
    let capabilities = provider(backend).capabilities();
    assert_eq!(
        capabilities.problem_classes,
        vec![
            ProblemClass::LinearProgram,
            ProblemClass::ConvexQuadraticProgram,
            ProblemClass::MixedIntegerLinearProgram,
        ]
    );
    assert!(capabilities.supports_class(ProblemClass::LinearProgram));
    assert!(!capabilities.supports_class(ProblemClass::ContinuousSecondOrderConeProgram));
    assert!(!capabilities.warm_start.supported);
    assert!(capabilities.licensing.solver_license.contains("MIT"));
    assert!(!capabilities.determinism.pinned_solver_version.is_empty());
    assert!(capabilities.diagnostics.solver_log);
    assert!(capabilities.diagnostics.infeasibility_conflict_evidence);
    // Scaling and tolerances are declared (§7.4), not implicit.
    assert!(!capabilities.scaling.note.is_empty());
    assert!(capabilities.tolerances.primal_feasibility_default > 0.0);
}

/// Compile-time guard: the port is usable as a trait object and the provider
/// implements it (the composition-root shape).
#[test]
#[allow(dead_code)]
fn port_and_backend_are_object_safe() {
    fn takes_dyn_optimizer(_: &dyn Optimizer) {}
    fn takes_dyn_backend(_: &dyn Tier1Backend) {}
    // Referencing the functions compiles the trait-object requirements.
    let _ = takes_dyn_optimizer;
    let _ = takes_dyn_backend;
    let _unused: Option<Box<dyn Fn(&Tier1Model) -> usize>> = None;
    let _ = Mutex::new(0u8);
}
