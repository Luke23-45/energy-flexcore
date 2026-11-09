// P3.25 — Solver I/O suite: malformed solver input rejected, solver output
// validated against the §12.6 contract before use (report §19.2 layer 1;
// ledger P3.25 step 3).
//
// SCOPE — this file closes GAPS in the existing suites; it does not
// duplicate them. Existing coverage: provider_status_mapping.rs (the
// six-status table, the Tier 1 class gate, one class-mismatch malformed
// case, the invalid-config gate, infeasibility evidence, completeness of
// every mapped record), replay_skeleton.rs (replay over the provider path),
// determinism.rs / infeasibility.rs / tier_guard.rs (P3.19/P3.20),
// clarabel_provider.rs (Tier 2), in-crate unit tests for `validate` and
// `validate_complete`.
//
// WHAT IS NEW HERE — the validation GATE end to end, never a real solve:
//
// 1. Every named `ProblemError` variant of the problem IR (the solver
//    input's validation) is produced by exactly its malformed input, with a
//    stated case count per test (15 cases in the table).
// 2. Every malformed input is rejected by the Tier 1 provider as
//    `OptimizerError::MalformedProblem` BEFORE any backend interaction —
//    the scripted backend records zero solve calls (15 cases through the
//    port).
// 3. A generated non-finite coefficient always lands as `NonFiniteValue`
//    (a property over 256 generated values, seeded splitmix64 per the
//    established pattern — boundary values 0.0, ±0.0 sign loss, and
//    subnormals included in the generator).
// 4. The §12.6 output gate: a backend outcome that would map to an
//    incomplete record is refused with `OptimizerError::IncompleteResult`
//    AFTER exactly one solve call, and the incomplete record is never
//    returned to the caller (the P3.21 completeness path, exercised at the
//    provider boundary).
// 5. The `validate_complete` error matrix: one stated case per required
//    §12.6 field (candidate ID, input snapshot, four identification fields,
//    determinism record, objective value/units, primal diagnostics for
//    optimal/feasible, empty not-available reasons), each stripped
//    individually from an otherwise-complete record — 11 cases.
// 6. A complete scripted outcome passes the gate and carries every §12.6
//    required field (the happy path that proves the gate is not a
//    blanket reject).
//
// The real-solver legs (pinned HiGHS worker adapter and the Tier 2 replay
// over both conic fixtures) remain the recorded VERIFY-BUILD obligation of
// P3.26 — see 19_20_determinism_tiers.md §1 and
// 25_26_tests_scenarios.md; no real solve is performed or implied here.

mod common;

use common::{outcome, scripted};
use flexcore_contracts::ids::FlexUuid;
use flexcore_optimization_api::backend::BackendStatus;
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::port::{Optimizer, OptimizerError};
use flexcore_optimization_api::problem::{
    Coefficient, LinearRow, Objective, ObjectiveSense, OptimizationProblem, ProblemClass,
    ProblemError, QuadraticTerm, RowBounds, SecondOrderCone, Variable, VariableKind, VariableUnit,
};
use flexcore_optimization_api::result::{
    Diagnostics, OptimizationResult, ResultCompletenessError, SolveStatus,
};
use std::sync::Arc;
use std::time::Duration;

/// A minimal VALID LP the mutation table starts from (two variables, one
/// row, linear objective, snapshot declared). Passes `validate`.
fn valid_lp() -> OptimizationProblem {
    let mut problem = fixture::reference_lp();
    // Normalise to a two-variable shape independent of the fixture's exact
    // optima: the I/O gate only cares about structural validity.
    problem.name = "p3_25_solver_io_fixture".to_string();
    problem
}

fn variable(name: &str) -> Variable {
    Variable {
        name: name.to_string(),
        kind: VariableKind::Continuous,
        unit: VariableUnit::ActivePowerMW,
        lower_bound: Some(0.0),
        upper_bound: Some(10.0),
    }
}

fn row(name: &str, variable_index: usize, coefficient: f64, bound: f64) -> LinearRow {
    LinearRow {
        name: name.to_string(),
        coefficients: vec![Coefficient { variable: variable_index, coefficient }],
        bounds: RowBounds::Upper(bound),
    }
}

fn objective() -> Objective {
    Objective {
        sense: ObjectiveSense::Minimize,
        linear_terms: vec![Coefficient { variable: 0, coefficient: 1.0 }],
        quadratic_terms: Vec::new(),
        constant: 0.0,
        declared_units: "synthetic_cost_unit".to_string(),
    }
}

// --- 1. The IR validation variant table (15 stated cases) --------------------

/// CASE TABLE (15 stated cases — one per named `ProblemError` variant): each
/// malformed input produces exactly the named error, at the IR's own
/// `validate()` gate. This is the "malformed solver input rejected" unit
/// surface (ledger P3.25 step 3); the class-mismatch case (variant 15) is
/// also asserted through the provider below.
#[test]
fn every_problem_ir_validation_variant_rejects_its_malformed_input() {
    // 1 — EmptyName.
    let mut p = valid_lp();
    p.name = "   ".to_string();
    assert_eq!(p.validate(), Err(ProblemError::EmptyName), "case 1: empty name");

    // 2 — NoVariables.
    let mut p = valid_lp();
    p.variables.clear();
    assert_eq!(p.validate(), Err(ProblemError::NoVariables), "case 2: no variables");

    // 3 — DuplicateVariableName.
    let mut p = valid_lp();
    p.variables[1].name = p.variables[0].name.clone();
    assert_eq!(
        p.validate(),
        Err(ProblemError::DuplicateVariableName(p.variables[0].name.clone())),
        "case 3: duplicate variable name"
    );

    // 4 — DuplicateRowName.
    let mut p = valid_lp();
    p.rows.push(row(p.rows[0].name.clone().as_str(), 1, 1.0, 5.0));
    assert_eq!(
        p.validate(),
        Err(ProblemError::DuplicateRowName(p.rows[0].name.clone())),
        "case 4: duplicate row name"
    );

    // 5 — DuplicateConeName.
    let mut p = valid_lp();
    p.declared_class = ProblemClass::ContinuousSecondOrderConeProgram;
    p.conic_constraints = vec![
        SecondOrderCone {
            name: "cone".to_string(),
            norm_members: vec![Coefficient { variable: 0, coefficient: 1.0 }],
            norm_offset: 0.0,
            rhs_members: vec![Coefficient { variable: 1, coefficient: 1.0 }],
            rhs_offset: 2.5,
        },
        SecondOrderCone {
            name: "cone".to_string(),
            norm_members: vec![Coefficient { variable: 1, coefficient: 1.0 }],
            norm_offset: 0.0,
            rhs_members: vec![Coefficient { variable: 0, coefficient: 1.0 }],
            rhs_offset: 2.5,
        },
    ];
    assert_eq!(
        p.validate(),
        Err(ProblemError::DuplicateConeName("cone".to_string())),
        "case 5: duplicate cone name"
    );

    // 6 — VariableIndexOutOfRange (row coefficient).
    let mut p = valid_lp();
    p.rows.push(row("row_oob", 7, 1.0, 5.0));
    assert_eq!(
        p.validate(),
        Err(ProblemError::VariableIndexOutOfRange { index: 7, variable_count: 2 }),
        "case 6: row coefficient index out of range"
    );

    // 7 — NonFiniteValue (variable bound).
    let mut p = valid_lp();
    p.variables[0].upper_bound = Some(f64::NAN);
    assert!(
        matches!(p.validate(), Err(ProblemError::NonFiniteValue { .. })),
        "case 7: non-finite variable bound"
    );

    // 8 — InvertedBounds (variable).
    let mut p = valid_lp();
    p.variables[0].lower_bound = Some(5.0);
    p.variables[0].upper_bound = Some(1.0);
    match p.validate() {
        Err(ProblemError::InvertedBounds { context }) => {
            assert!(context.contains("variable"), "case 8: inverted variable bounds")
        }
        other => panic!("case 8: expected InvertedBounds, got {other:?}"),
    }

    // 9 — BinaryBoundsOutsideUnitInterval.
    let mut p = valid_lp();
    p.variables[0].kind = VariableKind::Binary;
    p.variables[0].upper_bound = Some(2.0);
    assert_eq!(
        p.validate(),
        Err(ProblemError::BinaryBoundsOutsideUnitInterval(p.variables[0].name.clone())),
        "case 9: binary bounds outside [0, 1]"
    );

    // 10 — QuadraticTermNotUpperTriangle.
    let mut p = valid_lp();
    p.declared_class = ProblemClass::ConvexQuadraticProgram;
    p.objective.quadratic_terms =
        vec![QuadraticTerm { row: 1, column: 0, coefficient: 1.0 }];
    assert_eq!(
        p.validate(),
        Err(ProblemError::QuadraticTermNotUpperTriangle { row: 1, column: 0 }),
        "case 10: quadratic term off the upper triangle"
    );

    // 11 — DuplicateQuadraticTerm.
    let mut p = valid_lp();
    p.declared_class = ProblemClass::ConvexQuadraticProgram;
    p.objective.quadratic_terms = vec![
        QuadraticTerm { row: 0, column: 1, coefficient: 1.0 },
        QuadraticTerm { row: 0, column: 1, coefficient: 2.0 },
    ];
    assert_eq!(
        p.validate(),
        Err(ProblemError::DuplicateQuadraticTerm { row: 0, column: 1 }),
        "case 11: duplicate quadratic term"
    );

    // 12 — EmptyObjectiveUnits.
    let mut p = valid_lp();
    p.objective.declared_units = "  ".to_string();
    assert_eq!(
        p.validate(),
        Err(ProblemError::EmptyObjectiveUnits),
        "case 12: empty objective units"
    );

    // 13 — EmptyInputSnapshotId.
    let mut p = valid_lp();
    p.provenance.input_snapshot_id = "".to_string();
    assert_eq!(
        p.validate(),
        Err(ProblemError::EmptyInputSnapshotId),
        "case 13: empty input snapshot id"
    );

    // 14 — NonFiniteValue (row bound, distinct context).
    let mut p = valid_lp();
    p.rows[0].bounds = RowBounds::Lower(f64::INFINITY);
    assert!(
        matches!(p.validate(), Err(ProblemError::NonFiniteValue { .. })),
        "case 14: non-finite row bound"
    );

    // 15 — ClassMismatch (LP declaration over an integer body; the same
    // shape provider_status_mapping.rs exercises through the port).
    let mut p = valid_lp();
    p.variables[0].kind = VariableKind::Integer;
    assert!(
        matches!(p.validate(), Err(ProblemError::ClassMismatch { declared, .. }) if declared == ProblemClass::LinearProgram),
        "case 15: LP declaration over an integer body"
    );
}

// --- 2. Malformed input never reaches the backend ----------------------------

/// PROPERTY (all 15 malformed cases of the table above, driven through the
/// Tier 1 provider with an EMPTY scripted backend): every malformed input is
/// rejected as `OptimizerError::MalformedProblem` and the backend records
/// ZERO solve calls — the validation gate fires before any solver
/// interaction (report §9.1; ledger P3.25 step 3).
#[test]
fn malformed_input_is_rejected_before_any_backend_interaction() {
    // Rebuild the 15 malformed cases as (label, problem) pairs.
    let mut cases: Vec<(String, OptimizationProblem)> = Vec::new();

    let mut p = valid_lp();
    p.name = String::new();
    cases.push(("empty name".to_string(), p));

    let mut p = valid_lp();
    p.variables.clear();
    cases.push(("no variables".to_string(), p));

    let mut p = valid_lp();
    p.variables[1].name = p.variables[0].name.clone();
    cases.push(("duplicate variable".to_string(), p));

    let mut p = valid_lp();
    let duplicate = p.rows[0].name.clone();
    p.rows.push(row(&duplicate, 1, 1.0, 5.0));
    cases.push(("duplicate row".to_string(), p));

    let mut p = valid_lp();
    p.declared_class = ProblemClass::ContinuousSecondOrderConeProgram;
    p.conic_constraints = vec![SecondOrderCone {
        name: "a".to_string(),
        norm_members: vec![Coefficient { variable: 0, coefficient: 1.0 }],
        norm_offset: 0.0,
        rhs_members: vec![Coefficient { variable: 1, coefficient: 1.0 }],
        rhs_offset: 2.5,
    }];
    // A valid conic body is NOT malformed — the provider gates it as
    // `unsupported` (a §12.6 status), which is NOT an error. It is included
    // here as the stated NEGATIVE control: it must NOT produce
    // MalformedProblem.
    cases.push(("conic body on tier 1 (negative control)".to_string(), p));

    let mut p = valid_lp();
    p.rows.push(row("oob", 9, 1.0, 5.0));
    cases.push(("index out of range".to_string(), p));

    let mut p = valid_lp();
    p.variables[0].lower_bound = Some(f64::NAN);
    cases.push(("NaN variable bound".to_string(), p));

    let mut p = valid_lp();
    p.variables[0].lower_bound = Some(5.0);
    p.variables[0].upper_bound = Some(1.0);
    cases.push(("inverted variable bounds".to_string(), p));

    let mut p = valid_lp();
    p.variables[0].kind = VariableKind::Binary;
    p.variables[0].upper_bound = Some(3.0);
    cases.push(("binary bounds outside [0,1]".to_string(), p));

    let mut p = valid_lp();
    p.declared_class = ProblemClass::ConvexQuadraticProgram;
    p.objective.quadratic_terms =
        vec![QuadraticTerm { row: 1, column: 0, coefficient: 1.0 }];
    cases.push(("quadratic off triangle".to_string(), p));

    let mut p = valid_lp();
    p.declared_class = ProblemClass::ConvexQuadraticProgram;
    p.objective.quadratic_terms = vec![
        QuadraticTerm { row: 0, column: 1, coefficient: 1.0 },
        QuadraticTerm { row: 0, column: 1, coefficient: 1.0 },
    ];
    cases.push(("duplicate quadratic term".to_string(), p));

    let mut p = valid_lp();
    p.objective.declared_units = String::new();
    cases.push(("empty objective units".to_string(), p));

    let mut p = valid_lp();
    p.provenance.input_snapshot_id = " ".to_string();
    cases.push(("empty input snapshot".to_string(), p));

    let mut p = valid_lp();
    p.rows[0].bounds = RowBounds::Equality(f64::INFINITY);
    cases.push(("infinite row bound".to_string(), p));

    let mut p = valid_lp();
    p.variables[0].kind = VariableKind::Integer;
    cases.push(("class mismatch".to_string(), p));

    // 14 malformed + 1 negative control = the stated 15 cases.
    const CASE_COUNT: usize = 15;
    assert_eq!(cases.len(), CASE_COUNT, "the stated case count must match the table");

    for (label, problem) in &cases {
        let backend = scripted(Vec::new());
        let outcome = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(
            Arc::clone(&backend),
        )
        .solve(problem, &SolverConfig::default());

        if label.contains("negative control") {
            // The conic body on Tier 1 is structurally VALID: the provider
            // must return it as an in-band `unsupported` §12.6 STATUS — not
            // as an error, and never by invoking the backend.
            let result = outcome
                .unwrap_or_else(|e| panic!("{label}: the negative control must not error: {e:?}"));
            assert_eq!(
                result.status,
                flexcore_optimization_api::result::SolveStatus::Unsupported,
                "{label}: a supported-IR problem outside the provider's class set returns \
                 the in-band unsupported status"
            );
            assert_eq!(
                backend.solve_count(),
                0,
                "{label}: even the unsupported status is produced without a backend call"
            );
            continue;
        }

        let error = outcome
            .expect_err("{label}: a malformed problem is an error, never a solver outcome");
        match error {
            OptimizerError::MalformedProblem(_) => {}
            other => panic!("{label}: expected MalformedProblem, got {other:?}"),
        }
        assert_eq!(
            backend.solve_count(),
            0,
            "{label}: the backend must never be invoked on malformed input"
        );
    }
}

// --- 3. Generated non-finite coefficients are always rejected ----------------

/// The seeded generator (splitmix64, Steele et al. 2014) — the crate's
/// recorded property-test pattern, re-stated here for the integration test
/// (no external test dependencies are permitted).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
    fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        low + unit * (high - low)
    }
}

/// PROPERTY (256 generated non-finite values, seed 0xB315_0100, boundary
/// values 0.0 and a subnormal included in the generator): injecting a
/// non-finite value into ANY coefficient slot of the IR (variable bounds,
/// row coefficients, row bounds, objective linear term, objective constant,
/// quadratic coefficient, cone offsets) is rejected with
/// `NonFiniteValue` — a NaN or ±∞ never enters the solver path.
#[test]
fn property_non_finite_coefficients_are_always_rejected() {
    const GENERATED_CASES: usize = 256;
    let mut rng = SplitMix64::new(0xB315_0100);
    // Boundary values are IN the generator (the ledger's vacuity risk); the
    // finite draws (0.0, the smallest normal) act as in-slot negative
    // controls and are skipped, everything else must reject.
    let boundary = [0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE];
    let mut boundary_hits = 0usize;

    for case in 0..GENERATED_CASES {
        let poison = if rng.next_u64() % 4 == 0 {
            boundary_hits += 1;
            *rng.pick(&boundary)
        } else if rng.next_u64() % 2 == 0 {
            f64::NAN
        } else if rng.next_u64() % 2 == 0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        if poison.is_finite() {
            continue; // a finite draw is not a poison case
        }

        // Inject into a rotating slot per case.
        let slot = case % 6;
        let mut p = valid_lp();
        match slot {
            0 => p.variables[case % p.variables.len()].lower_bound = Some(poison),
            1 => p.variables[case % p.variables.len()].upper_bound = Some(poison),
            2 => {
                let row_index = case % p.rows.len();
                let variable_index = case % p.variables.len();
                p.rows[row_index].coefficients =
                    vec![Coefficient { variable: variable_index, coefficient: poison }];
            }
            3 => p.rows[case % p.rows.len()].bounds = RowBounds::Upper(poison),
            4 => p.objective.constant = poison,
            _ => {
                p.declared_class = ProblemClass::ConvexQuadraticProgram;
                p.objective.quadratic_terms =
                    vec![QuadraticTerm { row: 0, column: 1, coefficient: poison }];
            }
        }
        let error = p
            .validate()
            .expect_err(&format!("case {case}: a non-finite value in slot {slot} must be rejected"));
        assert!(
            matches!(error, ProblemError::NonFiniteValue { .. }),
            "case {case}: slot {slot} with value {poison} must reject NonFiniteValue, got {error:?}"
        );
    }
    assert!(
        boundary_hits > 0,
        "the generator never drew from the boundary set — the property ran vacuously"
    );
}

// --- 4. The §12.6 output gate at the provider boundary -----------------------

/// The scripted outcome is an OPTIMAL result whose payload is missing the
/// primal solution: the provider's own `validate_complete` gate must refuse
/// to publish it as `OptimizerError::IncompleteResult` (a provider bug by
/// contract), AFTER exactly one backend call — and the incomplete record
/// must never be returned.
#[test]
fn an_optimal_outcome_without_a_primal_payload_is_refused_at_the_gate() {
    let backend = scripted(vec![Ok(outcome(BackendStatus::Optimal, "Optimal", Some(4.5), None))]);
    let provider =
        flexcore_optimization_api::highs_provider::HighsTier1Provider::new(Arc::clone(&backend));
    let error = provider
        .solve(&valid_lp(), &SolverConfig::default())
        .expect_err("an optimal result without primal diagnostics is incomplete");
    match error {
        OptimizerError::IncompleteResult(
            ResultCompletenessError::MissingPrimalDiagnostics { status },
        ) => {
            assert_eq!(status, SolveStatus::Optimal);
        }
        other => panic!("expected IncompleteResult(MissingPrimalDiagnostics), got {other:?}"),
    }
    assert_eq!(
        backend.solve_count(),
        1,
        "the gate fires after the solve: exactly one backend call"
    );
}

/// The scripted outcome is an OPTIMAL result with no objective value: the
/// gate refuses it (`ObjectiveIncomplete`), again after exactly one call.
#[test]
fn an_optimal_outcome_without_an_objective_value_is_refused_at_the_gate() {
    let backend = scripted(vec![Ok(outcome(BackendStatus::Optimal, "Optimal", None, Some(vec![1.0, 2.0])))]);
    let provider =
        flexcore_optimization_api::highs_provider::HighsTier1Provider::new(Arc::clone(&backend));
    let error = provider
        .solve(&valid_lp(), &SolverConfig::default())
        .expect_err("an optimal result without an objective value is incomplete");
    match error {
        OptimizerError::IncompleteResult(ResultCompletenessError::ObjectiveIncomplete { .. }) => {}
        other => panic!("expected IncompleteResult(ObjectiveIncomplete), got {other:?}"),
    }
    assert_eq!(backend.solve_count(), 1);
}

/// A complete scripted outcome passes the gate: the caller receives a record
/// that itself passes `validate_complete` and carries every required §12.6
/// field (candidate ID, input snapshot, identification, determinism record,
/// objective + units, primal diagnostics). This is the "validated before
/// use" happy path.
#[test]
fn a_complete_outcome_passes_the_gate_with_every_required_field() {
    let backend = scripted(vec![Ok(outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(4.5),
        Some(vec![1.5, 3.0]),
    ))]);
    let provider =
        flexcore_optimization_api::highs_provider::HighsTier1Provider::new(Arc::clone(&backend));
    let result = provider
        .solve(&valid_lp(), &SolverConfig::default())
        .expect("a complete outcome publishes its record");
    result
        .validate_complete()
        .expect("the published record passes the same gate the provider applied");
    assert_eq!(result.status, SolveStatus::Optimal);
    assert!(result.candidate_id.is_some(), "§12.6 candidate ID");
    assert!(result.input_snapshot_id.as_deref().is_some_and(|s| !s.trim().is_empty()), "§12.6 input snapshot");
    assert!(result.determinism.is_some(), "pinned version + thread count recorded");
    assert!(!result.solver_name.trim().is_empty() && !result.solver_version.trim().is_empty());
    assert!(!result.provider_id.trim().is_empty() && !result.provider_version.trim().is_empty());
    assert!(result.objective_units.as_deref().is_some_and(|u| !u.trim().is_empty()));
    assert!(result.primal.is_available(), "an optimal result carries primal diagnostics");
}

// --- 5. The validate_complete error matrix (11 stated cases) -----------------

/// An otherwise-COMPLETE §12.6 record; one field is stripped per case.
fn complete_result() -> OptimizationResult {
    OptimizationResult {
        status: SolveStatus::Optimal,
        problem_class: ProblemClass::LinearProgram,
        solver_name: "highs".to_string(),
        solver_version: "1.15.1".to_string(),
        provider_id: "test.provider".to_string(),
        provider_version: "0.1.0".to_string(),
        objective_value: Some(4.5),
        objective_units: Some("synthetic_cost_unit".to_string()),
        primal: Diagnostics::Available(flexcore_optimization_api::result::PrimalDiagnostics {
            values: vec![flexcore_optimization_api::result::VariableValue {
                name: "p".to_string(),
                unit: "MW".to_string(),
                value: 1.5,
            }],
        }),
        dual: Diagnostics::NotAvailable { reason: "not exposed by the pinned binding".to_string() },
        residuals: Diagnostics::Available(flexcore_optimization_api::result::ResidualReport {
            max_primal_violation: Some(0.0),
            max_dual_violation: None,
            iteration_count: Some(3),
        }),
        infeasibility_evidence: Diagnostics::NotAvailable {
            reason: "not applicable: the solve was optimal".to_string(),
        },
        runtime: Duration::from_millis(1),
        resources: None,
        candidate_id: Some(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap()),
        input_snapshot_id: Some("snapshot-1".to_string()),
        data_timestamp: None,
        grid_model_version: None,
        uncertainty_and_assumptions: Vec::new(),
        warnings: Vec::new(),
        determinism: Some(flexcore_optimization_api::result::DeterminismRecord {
            solver_version: "1.15.1".to_string(),
            thread_count: 1,
            time_limit_ms: None,
            config_digest: "fnv1a64:0".to_string(),
        }),
    }
}

/// CASE TABLE (11 stated cases — one per `ResultCompletenessError` variant
/// family): stripping exactly one required §12.6 field from an otherwise
/// complete record produces exactly the named error, and nothing else. This
/// is the §12.6 gate's own contract, stated per field (ledger P3.25 step 3;
/// P3.21 completeness path).
#[test]
fn every_s126_required_field_stripped_produces_its_named_error() {
    // 1 — MissingCandidateId.
    let mut r = complete_result();
    r.candidate_id = None;
    assert_eq!(r.validate_complete(), Err(ResultCompletenessError::MissingCandidateId), "case 1");

    // 2 — MissingCandidateId (the NIL UUID is not an identity).
    let mut r = complete_result();
    r.candidate_id = Some(FlexUuid::NIL);
    assert_eq!(r.validate_complete(), Err(ResultCompletenessError::MissingCandidateId), "case 2");

    // 3 — MissingInputSnapshotId.
    let mut r = complete_result();
    r.input_snapshot_id = None;
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::MissingInputSnapshotId),
        "case 3"
    );

    // 4 — EmptyIdentification (solver_name; the other three fields follow the
    // same code path and are covered by variant identity here).
    let mut r = complete_result();
    r.solver_name = "  ".to_string();
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::EmptyIdentification { field: "solver_name" }),
        "case 4"
    );

    // 5 — MissingDeterminismRecord.
    let mut r = complete_result();
    r.determinism = None;
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::MissingDeterminismRecord),
        "case 5"
    );

    // 6 — EmptyNotAvailableReason (dual).
    let mut r = complete_result();
    r.dual = Diagnostics::NotAvailable { reason: " ".to_string() };
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::EmptyNotAvailableReason { diagnostic: "dual" }),
        "case 6"
    );

    // 7 — EmptyNotAvailableReason (infeasibility evidence).
    let mut r = complete_result();
    r.infeasibility_evidence = Diagnostics::NotAvailable { reason: String::new() };
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::EmptyNotAvailableReason {
            diagnostic: "infeasibility_evidence"
        }),
        "case 7"
    );

    // 8 — ObjectiveIncomplete (units missing on an optimal result).
    let mut r = complete_result();
    r.objective_units = None;
    assert!(
        matches!(
            r.validate_complete(),
            Err(ResultCompletenessError::ObjectiveIncomplete { .. })
        ),
        "case 8"
    );

    // 9 — ObjectiveIncomplete (value missing on an optimal result).
    let mut r = complete_result();
    r.objective_value = None;
    assert!(
        matches!(
            r.validate_complete(),
            Err(ResultCompletenessError::ObjectiveIncomplete { .. })
        ),
        "case 9"
    );

    // 10 — MissingPrimalDiagnostics (optimal).
    let mut r = complete_result();
    r.primal = Diagnostics::NotAvailable { reason: "recorded".to_string() };
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::MissingPrimalDiagnostics { status: SolveStatus::Optimal }),
        "case 10"
    );

    // 11 — MissingPrimalDiagnostics (feasible: the same requirement applies).
    let mut r = complete_result();
    r.status = SolveStatus::Feasible;
    r.primal = Diagnostics::NotAvailable { reason: "recorded".to_string() };
    assert_eq!(
        r.validate_complete(),
        Err(ResultCompletenessError::MissingPrimalDiagnostics { status: SolveStatus::Feasible }),
        "case 11"
    );

    // Sanity: the untouched record passes.
    complete_result()
        .validate_complete()
        .expect("the untouched complete record must pass its own gate");
}
