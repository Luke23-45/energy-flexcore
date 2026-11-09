// P3.18 — Tier 2 (continuous SOCP) provider tests: the integrality and class
// gates before any solver invocation, cone assembly in the problem IR's
// terms, the foundations §7.8 per-branch relaxation residual against
// hand-computed values, tolerance configuration, the §12.5 exactness caveat,
// and the Tier 1 / Tier 2 cross-check record (P3.23 input).
//
// Offline-deterministic discipline (mirrors the P3.17 test note): everything
// around the solve — gates, assembly, residual math, mapping table, caveats —
// is fully tested WITHOUT the solver, proved by the provider's invocation
// counter staying at zero on every gate path. The real-solve tests are
// marked VERIFY-BUILD (18_clarabel_tier2.md §7): they exercise the pinned
// crate once the coordinator's manifest lines land.

use flexcore_optimization_api::clarabel_provider::{
    apply_branch_specs, assemble_conic_model, binding_cones, binding_linear_rows, cone_slack,
    distflow_branch_residual, relaxation_residual_report, ClarabelTier2Provider,
    DEFAULT_RELAXATION_RESIDUAL_TOLERANCE, SOCP_EXACTNESS_CAVEAT, Tier2ProviderConfig,
};
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::conic_fixture::{
    build_comparison_record, distflow_branch_specs, distflow_branch_socp,
    distflow_branch_socp_optimum, inverter_cone_socp, inverter_cone_socp_optimum,
    inverter_linearised_lp, inverter_linearised_lp_optimum, DISTFLOW_CONE_NAME,
    INVERTER_CONE_NAME, INVERTER_LINEARISED_P_LIMIT,
};
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::port::{Optimizer, OptimizerError};
use flexcore_optimization_api::problem::{
    Coefficient, OptimizationProblem, ProblemClass, Variable, VariableKind, VariableUnit,
};
use flexcore_optimization_api::result::{
    DeterminismRecord, Diagnostics, OptimizationResult, PrimalDiagnostics, SolveStatus,
    VariableValue,
};
use std::time::Duration;

// The residual tolerance used by the default provider configuration is the
// imported DEFAULT_RELAXATION_RESIDUAL_TOLERANCE const (the recorded default,
// not a copy).

fn provider(config: Tier2ProviderConfig) -> ClarabelTier2Provider {
    ClarabelTier2Provider::new(config)
}

fn default_provider() -> ClarabelTier2Provider {
    provider(Tier2ProviderConfig::default())
}

/// Builds a complete, hand-computed §12.6 result for a fixture (used to feed
/// the cross-check record without invoking either provider).
fn hand_result(
    problem: &OptimizationProblem,
    status: SolveStatus,
    objective_value: f64,
    column_values: Vec<f64>,
) -> OptimizationResult {
    let result = OptimizationResult {
        status,
        problem_class: problem.declared_class,
        solver_name: if problem.declared_class == ProblemClass::LinearProgram {
            "HiGHS"
        } else {
            "Clarabel"
        }
        .to_string(),
        solver_version: if problem.declared_class == ProblemClass::LinearProgram {
            "1.15.1"
        } else {
            "0.11.1"
        }
        .to_string(),
        provider_id: if problem.declared_class == ProblemClass::LinearProgram {
            "builtin.optimizer-highs-tier1"
        } else {
            "builtin.optimizer-clarabel-tier2"
        }
        .to_string(),
        provider_version: "0.1.0".to_string(),
        objective_value: Some(objective_value),
        objective_units: Some(problem.objective.declared_units.clone()),
        primal: Diagnostics::Available(PrimalDiagnostics {
            values: column_values
                .into_iter()
                .zip(problem.variables.iter())
                .map(|(value, variable)| VariableValue {
                    name: variable.name.clone(),
                    unit: variable.unit.canonical_unit().to_string(),
                    value,
                })
                .collect(),
        }),
        dual: Diagnostics::NotAvailable {
            reason: "not needed for the cross-check record".to_string(),
        },
        residuals: Diagnostics::NotAvailable {
            reason: "not needed for the cross-check record".to_string(),
        },
        infeasibility_evidence: Diagnostics::NotAvailable {
            reason: format!("not applicable to status '{}'", status.as_str()),
        },
        runtime: Duration::ZERO,
        resources: None,
        candidate_id: Some(problem.provenance.candidate_id),
        input_snapshot_id: Some(problem.provenance.input_snapshot_id.clone()),
        data_timestamp: problem.provenance.data_timestamp,
        grid_model_version: None,
        uncertainty_and_assumptions: problem.declared_assumptions.clone(),
        warnings: Vec::new(),
        determinism: Some(DeterminismRecord {
            solver_version: "0.0.0".to_string(),
            thread_count: 1,
            time_limit_ms: None,
            config_digest: "fnv1a64:test".to_string(),
        }),
    };
    // The hand-built record must itself be complete before it is consumed.
    result.validate_complete().expect("hand result is complete");
    result
}

/// A continuous SOCP fixture with one binary variable appended — an
/// integer-carrying formulation that must be rejected by the Tier 2 provider
/// with `unsupported` BEFORE any solver invocation (Clarabel is not MISOCP).
fn socp_with_a_binary_variable() -> OptimizationProblem {
    let mut problem = inverter_cone_socp();
    problem.name = "conic-fixture-socp-with-binary".to_string();
    problem.variables.push(Variable {
        name: "mode_switch_binary".to_string(),
        kind: VariableKind::Binary,
        unit: VariableUnit::Dimensionless,
        lower_bound: Some(0.0),
        upper_bound: Some(1.0),
    });
    problem
}

/// The integrality gate: an integer-carrying formulation returns `unsupported`
/// in-band, the provider never invokes the solver, and the record is complete.
#[test]
fn integer_formulation_returns_unsupported_before_any_solver_invocation() {
    let tier2 = default_provider();
    let problem = socp_with_a_binary_variable();

    let result = tier2
        .solve(&problem, &SolverConfig::default())
        .expect("unsupported is an in-band status, not an error");

    assert_eq!(result.status, SolveStatus::Unsupported);
    assert_eq!(
        result.problem_class,
        ProblemClass::ContinuousSecondOrderConeProgram
    );
    assert_eq!(tier2.solver_invocations(), 0, "no solver call on the gate path");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("integer") && w.contains("not a MISOCP")),
        "the rejection must state that the pinned conic solver is not MISOCP; \
         warnings: {:?}",
        result.warnings
    );
    result
        .validate_complete()
        .expect("the unsupported record is complete");
}

/// The integrality gate fires for ANY integer-carrying formulation, whatever
/// its declared class — a MILP handed to Tier 2 never reaches the solver.
#[test]
fn any_integer_declaration_is_rejected_by_the_tier2_provider() {
    let tier2 = default_provider();
    let result = tier2
        .solve(&fixture::reference_milp(), &SolverConfig::default())
        .expect("unsupported is an in-band status, not an error");
    assert_eq!(result.status, SolveStatus::Unsupported);
    assert_eq!(tier2.solver_invocations(), 0);

    let misocp = fixture::reference_misocp_declaration();
    let result = tier2
        .solve(&misocp, &SolverConfig::default())
        .expect("unsupported is an in-band status, not an error");
    assert_eq!(result.status, SolveStatus::Unsupported);
    assert_eq!(tier2.solver_invocations(), 0);
}

/// The class gate: LP, QP, MILP, and nonlinear declarations are outside the
/// Tier 2 set and are rejected with `unsupported` before any solver call.
#[test]
fn non_socp_classes_are_rejected_before_any_solver_invocation() {
    let tier2 = default_provider();
    for problem in [
        fixture::reference_lp(),
        fixture::reference_qp(),
        fixture::reference_nonlinear_declaration(),
    ] {
        let result = tier2
            .solve(&problem, &SolverConfig::default())
            .expect("unsupported is an in-band status, not an error");
        assert_eq!(result.status, SolveStatus::Unsupported);
        assert!(tier2.solver_invocations() == 0);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("outside the Tier 2 supported set")),
            "the class rejection must name the Tier 2 supported set"
        );
        result
            .validate_complete()
            .expect("the unsupported record is complete");
    }
}

/// A structurally inconsistent declaration (socp without a cone) is a
/// malformed problem, not a solver outcome.
#[test]
fn socp_declaration_without_a_cone_is_malformed() {
    let mut problem = inverter_cone_socp();
    problem.conic_constraints.clear();
    let error = default_provider()
        .solve(&problem, &SolverConfig::default())
        .expect_err("class mismatch is an error, not a solver outcome");
    assert!(matches!(
        error,
        OptimizerError::MalformedProblem(
            flexcore_optimization_api::problem::ProblemError::ClassMismatch { .. }
        )
    ));
    assert_eq!(
        default_provider().solver_invocations(),
        0,
        "the provider never invokes the solver on a malformed formulation"
    );
}

/// Cone assembly in the problem IR's terms: columns, rows, and cones land in
/// the model in formulation order with their coefficients intact.
#[test]
fn conic_assembly_carries_the_inverter_cone_into_the_model() {
    let problem = inverter_cone_socp();
    let model = assemble_conic_model(&problem).expect("socp assembles");

    assert_eq!(model.declared_class, ProblemClass::ContinuousSecondOrderConeProgram);
    assert!(!model.minimize, "the fixture maximises active export");
    assert_eq!(model.columns.len(), 2);
    assert!(model.rows.is_empty());
    assert_eq!(model.cones.len(), 1);
    let cone = &model.cones[0];
    assert_eq!(cone.name, INVERTER_CONE_NAME);
    assert_eq!(
        cone.norm_members,
        vec![(0usize, 1.0_f64), (1, 1.0)],
        "‖(p, q)‖₂"
    );
    assert!((cone.rhs_offset - 5.0).abs() <= 1e-12, "S_max = 5 MVA");
    assert!(cone.rhs_members.is_empty());
}

/// The DistFlow fixture assembles with its equality rows and the canonical
/// rotated branch cone.
#[test]
fn conic_assembly_carries_the_distflow_branch_cone_into_the_model() {
    let problem = distflow_branch_socp();
    let model = assemble_conic_model(&problem).expect("socp assembles");
    assert_eq!(model.columns.len(), 4);
    assert_eq!(model.rows.len(), 3, "v/P/Q setpoints as equality rows");
    assert_eq!(model.cones.len(), 1);
    let cone = &model.cones[0];
    assert_eq!(cone.name, DISTFLOW_CONE_NAME);
    // ‖(2·P, 2·Q, v − l)‖₂ ≤ v + l  (canonical foundations §7.8 encoding).
    assert_eq!(
        cone.norm_members,
        vec![(2usize, 2.0_f64), (3, 2.0), (0, 1.0), (1, -1.0)]
    );
    assert_eq!(cone.rhs_members, vec![(0usize, 1.0_f64), (1, 1.0)]);
}

/// The residual formula matches hand-computed values and the canonical
/// encoding equivalence holds (foundations §7.8).
#[test]
fn branch_residual_matches_hand_computed_values() {
    // Loose point: g = 1.0·4.5 − 4 − 0.25 = 0.25 (relaxation held, not tight).
    assert!((distflow_branch_residual(1.0, 4.5, 2.0, 0.5) - 0.25).abs() <= 1e-12);
    // Binding point: g = 1.0·4.25 − 4 − 0.25 = 0.
    assert!(distflow_branch_residual(1.0, 4.25, 2.0, 0.5).abs() <= 1e-12);
    // Violated point: g = 1.0·4.0 − 4 − 0.25 = −0.25.
    assert!((distflow_branch_residual(1.0, 4.0, 2.0, 0.5) - (-0.25)).abs() <= 1e-12);

    // Canonical cone encoding equivalence: slack = 4·g_ij.
    let problem = distflow_branch_socp();
    let cone = &problem.conic_constraints[0];
    let primal = vec![1.0, 4.5, 2.0, 0.5];
    let slack = cone_slack(cone, &primal).expect("primal aligned");
    assert!((slack / 4.0 - distflow_branch_residual(1.0, 4.5, 2.0, 0.5)).abs() <= 1e-12);

    // The branch specs apply the formula per branch in spec order.
    let residuals = apply_branch_specs(&distflow_branch_specs(), &primal)
        .expect("spec indices in range");
    assert_eq!(residuals, vec![("12".to_string(), 0.25)]);
}

/// The inverter cone slack matches hand-computed values: binding at the
/// fixture optimum ‖(4, 3)‖ = 5, positive slack elsewhere.
#[test]
fn inverter_cone_slack_matches_hand_computed_values() {
    let problem = inverter_cone_socp();
    let cone = &problem.conic_constraints[0];
    let binding = cone_slack(cone, &inverter_cone_socp_optimum().column_values)
        .expect("primal aligned");
    assert!(binding.abs() <= 1e-12);

    let loose = cone_slack(cone, &[3.0, 3.0]).expect("primal aligned");
    assert!((loose - 7.0).abs() <= 1e-12, "25 − 18 = 7");

    let violated = cone_slack(cone, &[5.0, 5.0]).expect("primal aligned");
    assert!((violated - (-25.0)).abs() <= 1e-12, "25 − 50 = −25");
}

/// The tolerance configuration is respected: the same primal point is
/// reported as loose at a tight tolerance and binding at a loose one, and
/// violations are flagged regardless of tolerance. (The provider validates
/// and carries the tolerance; the report evaluation is exercised directly
/// here against hand-computed slacks.)
#[test]
fn residual_tolerance_configuration_is_respected() {
    let problem = distflow_branch_socp();
    let primal = vec![1.0, 4.5, 2.0, 0.5]; // slack = 4·0.25 = 1.0

    let report = relaxation_residual_report(
        &problem,
        &primal,
        DEFAULT_RELAXATION_RESIDUAL_TOLERANCE,
    )
    .expect("primal aligned");
    assert!(!report.entries[0].violated);
    assert!(report.entries[0].exceeds_tolerance, "1.0 > 1e-4: loose");
    assert_eq!(report.loose_cones, vec![DISTFLOW_CONE_NAME.to_string()]);
    assert!(
        (report.tolerance - DEFAULT_RELAXATION_RESIDUAL_TOLERANCE).abs() <= 1e-15
    );

    let report = relaxation_residual_report(&problem, &primal, 2.0).expect("primal aligned");
    assert!(!report.entries[0].exceeds_tolerance, "1.0 ≤ 2.0: within");
    assert!((report.tolerance - 2.0).abs() <= 1e-15);

    // A violated point is flagged at any tolerance.
    let violated_primal = vec![1.0, 4.0, 2.0, 0.5]; // g = −0.25, slack = −1.0
    let report = relaxation_residual_report(&problem, &violated_primal, 2.0)
        .expect("primal aligned");
    assert!(report.entries[0].violated);
    assert_eq!(report.violated_cones, vec![DISTFLOW_CONE_NAME.to_string()]);
    assert!(
        report.summary().contains("does not replace the Phase 4 AC power-flow"),
        "the summary must state that the residual does not replace Phase 4"
    );
}

/// An invalid provider-level residual tolerance is rejected before any
/// formulation work.
#[test]
fn invalid_residual_tolerance_is_rejected_before_any_solver_invocation() {
    let tier2 = provider(Tier2ProviderConfig {
        relaxation_residual_tolerance: 0.0,
    });
    let error = tier2
        .solve(&inverter_cone_socp(), &SolverConfig::default())
        .expect_err("zero tolerance is an invalid config");
    assert!(matches!(
        error,
        OptimizerError::InvalidConfig(
            flexcore_optimization_api::config::ConfigError::NonPositiveTolerance { .. }
        )
    ));
    assert_eq!(tier2.solver_invocations(), 0);
}

/// The §12.5 exactness caveat is part of the recorded provider surface.
#[test]
fn exactness_caveat_is_present_in_the_provider_surface() {
    assert!(
        SOCP_EXACTNESS_CAVEAT.contains("conditional"),
        "the caveat must state that exactness is conditional"
    );
    assert!(
        SOCP_EXACTNESS_CAVEAT.contains("Phase 4"),
        "the caveat must point at the Phase 4 validation gate"
    );
    assert!(
        SOCP_EXACTNESS_CAVEAT.contains("never a feasibility verdict"),
        "the caveat must forbid treating the result as a feasibility verdict"
    );
    let capabilities = default_provider().capabilities();
    assert!(
        capabilities
            .determinism
            .notes
            .iter()
            .chain(capabilities.diagnostics.notes.iter())
            .any(|note| note == SOCP_EXACTNESS_CAVEAT),
        "the caveat must be recorded on the capability declaration"
    );
    assert_eq!(
        capabilities.problem_classes,
        vec![ProblemClass::ContinuousSecondOrderConeProgram]
    );
    assert!(
        !capabilities
            .constraint_features
            .contains(&flexcore_optimization_api::port::ConstraintFeature::Integrality),
        "the capability surface must never declare integrality"
    );
}

/// Binding sets for the cross-check record, computed at hand-computed optima.
#[test]
fn binding_sets_are_computed_from_the_hand_computed_optima() {
    let socp = inverter_cone_socp();
    let socp_primal = inverter_cone_socp_optimum().column_values;
    assert_eq!(
        binding_cones(&socp, &socp_primal, 1e-6).expect("primal aligned"),
        vec![INVERTER_CONE_NAME.to_string()],
        "the cone binds at the Tier 2 optimum"
    );

    let lp = inverter_linearised_lp();
    let lp_primal = inverter_linearised_lp_optimum().column_values;
    assert_eq!(
        binding_linear_rows(&lp, &lp_primal, 1e-6).expect("primal aligned"),
        vec![INVERTER_LINEARISED_P_LIMIT.to_string()],
        "the linearised export limit binds at the Tier 1 optimum"
    );

    let distflow = distflow_branch_socp();
    let distflow_primal = distflow_branch_socp_optimum().column_values;
    assert_eq!(
        binding_cones(&distflow, &distflow_primal, 1e-6).expect("primal aligned"),
        vec![DISTFLOW_CONE_NAME.to_string()],
        "the branch cone binds at the fixture optimum"
    );
}

/// The cross-check record (P3.23 input) built from hand-computed results:
/// per-candidate problem class, objective, binding sets, and the
/// linearisation gap of exactly 1 MW on the shared inverter fixture.
#[test]
fn comparison_record_compares_class_objective_and_binding_sets() {
    let lp = inverter_linearised_lp();
    let socp = inverter_cone_socp();
    let tier1_result = hand_result(
        &lp,
        SolveStatus::Optimal,
        inverter_linearised_lp_optimum().objective_value,
        inverter_linearised_lp_optimum().column_values,
    );
    let tier2_result = hand_result(
        &socp,
        SolveStatus::Optimal,
        inverter_cone_socp_optimum().objective_value,
        inverter_cone_socp_optimum().column_values,
    );

    let record = build_comparison_record(
        (&lp, &tier1_result),
        (&socp, &tier2_result),
        1e-6,
    )
    .expect("both records carry primal diagnostics");

    // Per-candidate problem class: the classes MUST differ across the check.
    assert_eq!(record.tier1.problem_class, ProblemClass::LinearProgram);
    assert_eq!(
        record.tier2.problem_class,
        ProblemClass::ContinuousSecondOrderConeProgram
    );

    // Objectives and the linearisation gap.
    assert_eq!(record.tier1.objective_value, Some(5.0));
    assert_eq!(record.tier2.objective_value, Some(4.0));
    assert_eq!(record.objective_difference, Some(1.0));

    // Binding sets: the linearised row on Tier 1, the cone on Tier 2.
    assert_eq!(
        record.tier1.binding_constraints,
        vec![INVERTER_LINEARISED_P_LIMIT.to_string()]
    );
    assert_eq!(
        record.tier2.binding_constraints,
        vec![INVERTER_CONE_NAME.to_string()]
    );

    // Identity and caveats recorded.
    assert_eq!(record.candidate_id, tier2_result.candidate_id);
    assert!(
        record
            .notes
            .iter()
            .any(|note| note.contains("neither side is a feasibility verdict")),
        "the record must carry the Phase 4 caveat"
    );
}

/// VERIFY-BUILD (18_clarabel_tier2.md §7 item 1): the continuous SOCP fixture
/// solves through the port path with the hand-computed optimum. This test
/// exercises the pinned Clarabel 0.11.1 crate and runs once the coordinator's
/// manifest lines (18_clarabel_tier2.md §3) are applied.
#[test]
fn verify_build_continuous_socp_fixture_solves_through_the_port() {
    let tier2 = default_provider();
    let problem = inverter_cone_socp();
    let result = tier2
        .solve(&problem, &SolverConfig::default())
        .expect("the fixture is valid and supported");

    assert_eq!(result.status, SolveStatus::Optimal);
    assert_eq!(result.solver_name, "Clarabel");
    assert_eq!(result.solver_version, "0.11.1");
    let objective = result
        .objective_value
        .expect("an optimal result carries the objective value");
    assert!(
        (objective - inverter_cone_socp_optimum().objective_value).abs() <= 1e-6,
        "hand-computed optimum is 4.0, got {objective}"
    );
    if let Diagnostics::Available(primal) = &result.primal {
        let p = primal.values[0].value;
        let q = primal.values[1].value;
        assert!((p - 4.0).abs() <= 1e-6, "p ≈ 4 MW, got {p}");
        assert!((q - 3.0).abs() <= 1e-9, "q pinned at 3 MVAr, got {q}");
    } else {
        panic!("an optimal result must carry primal diagnostics");
    }
    // The §12.5 caveat is recorded verbatim on every solved result.
    assert!(
        result.warnings.iter().any(|w| w == SOCP_EXACTNESS_CAVEAT),
        "the exactness caveat must appear on solved results"
    );
    // The per-cone residual report is recorded per run.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("per-cone relaxation residual report")),
        "the residual report must be recorded per run"
    );
    assert_eq!(tier2.solver_invocations(), 1);
    result
        .validate_complete()
        .expect("the solved record is complete");
}

/// VERIFY-BUILD (18_clarabel_tier2.md §7 item 1): the DistFlow branch fixture
/// solves with the hand-computed optimum and a binding branch cone
/// (g_ij = 0 at the optimum).
#[test]
fn verify_build_distflow_branch_fixture_solves_through_the_port() {
    let tier2 = default_provider();
    let problem = distflow_branch_socp();
    let result = tier2
        .solve(&problem, &SolverConfig::default())
        .expect("the fixture is valid and supported");

    assert_eq!(result.status, SolveStatus::Optimal);
    let objective = result.objective_value.expect("optimal carries objective");
    assert!(
        (objective - distflow_branch_socp_optimum().objective_value).abs() <= 1e-6,
        "hand-computed optimum is 4.25, got {objective}"
    );
    if let Diagnostics::Available(primal) = &result.primal {
        let l = primal.values[1].value;
        assert!((l - 4.25).abs() <= 1e-6, "l ≈ 4.25, got {l}");
    } else {
        panic!("an optimal result must carry primal diagnostics");
    }
    // The branch cone binds: its residual entry is within tolerance, not
    // flagged as loose or violated.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("per-cone relaxation residual report")
                && w.contains("violated = []; slack beyond tolerance = []")),
        "the binding branch cone must be reported within tolerance; warnings: {:?}",
        result.warnings
    );
    result
        .validate_complete()
        .expect("the solved record is complete");
}

/// The tier 2 provider is object-safe and usable behind the port trait object
/// (the same composition-root shape the Tier 1 provider guarantees).
#[test]
fn tier2_provider_is_usable_behind_the_port_object() {
    fn takes_dyn(_: &dyn Optimizer) {}
    takes_dyn(&default_provider());
}

/// A `Range` row formulation is structurally legal and reaches the solver
/// path; its dual recombination is a VERIFY-BUILD item (duals recorded as
/// not-available rather than partially recomputed). The gate paths are
/// unaffected.
#[test]
fn verify_build_range_row_duals_are_recorded_as_not_available() {
    let mut problem = inverter_cone_socp();
    problem.name = "conic-fixture-socp-with-range-row".to_string();
    problem.rows.push(flexcore_optimization_api::problem::LinearRow {
        name: "export_window".to_string(),
        coefficients: vec![Coefficient {
            variable: 0,
            coefficient: 1.0,
        }],
        bounds: flexcore_optimization_api::problem::RowBounds::Range {
            lower: 0.0,
            upper: 4.0,
        },
    });
    problem.validate().expect("range rows are structurally legal");

    let result = default_provider()
        .solve(&problem, &SolverConfig::default())
        .expect("the formulation is valid and supported");
    assert_eq!(result.status, SolveStatus::Optimal);
    // The objective must respect the added range constraint: p = 4 is the
    // cone optimum and also within [0, 4], so the optimum is unchanged.
    let objective = result.objective_value.expect("optimal carries objective");
    assert!((objective - 4.0).abs() <= 1e-6);
    assert!(
        result
            .dual
            .not_available_reason()
            .is_some_and(|reason| reason.contains("VERIFY-BUILD")),
        "range-row duals are recorded as not available with the VERIFY-BUILD \
         reason, not partially recomputed"
    );
}
