// P3.17 — Deterministic-replay skeletons over the port (P3.19 step 4 /
// P3.26 land the real-solver replay; decision note §6 item 6).
//
// These are real tests over the `Optimizer` port: the in-crate reference
// fixtures are solved through the full provider path (validation → class
// gate → formulation assembly → backend call → §12.6 mapping) with scripted
// backend outcomes carrying the hand-computed optima, and the result records
// must be identical across replays. Runtime is the one measured field, so it
// is normalised before the comparison here; P3.19's real-solver replay states
// its tolerance explicitly (ledger P3.19 step 4: byte-identical or
// tolerance-identical with the tolerance stated). VERIFY-BUILD: the identical
// assertions run against the P3.19 worker adapter with the pinned binding.

mod common;

use common::{outcome, scripted};
use flexcore_optimization_api::backend::{BackendOutcome, BackendStatus};
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::fixture::ReferenceOptimum;
use flexcore_optimization_api::highs_provider::HighsTier1Provider;
use flexcore_optimization_api::port::Optimizer;
use flexcore_optimization_api::problem::{OptimizationProblem, ProblemClass};
use flexcore_optimization_api::result::SolveStatus;
use std::time::Duration;

fn optimal_outcome(objective_value: f64, column_values: Vec<f64>) -> BackendOutcome {
    let mut base = outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(objective_value),
        Some(column_values),
    );
    base.max_primal_violation = Some(0.0);
    base.iteration_count = Some(3);
    base
}

/// The hand-computed optimum per Tier 1 reference fixture.
fn fixture_optimum(problem: &OptimizationProblem) -> ReferenceOptimum {
    match problem.declared_class {
        ProblemClass::LinearProgram => fixture::reference_lp_optimum(),
        ProblemClass::ConvexQuadraticProgram => fixture::reference_qp_optimum(),
        ProblemClass::MixedIntegerLinearProgram => fixture::reference_milp_optimum(),
        other => panic!("not a Tier 1 reference fixture: {other:?}"),
    }
}

/// Solves the fixture once through the provider path and returns the
/// serialised §12.6 record with the measured runtime normalised to zero
/// (the one field that legitimately varies between runs).
fn solve_and_serialise(problem: &OptimizationProblem) -> String {
    let optimum = fixture_optimum(problem);
    assert_eq!(
        optimum.column_values.len(),
        problem.variables.len(),
        "optimum payload must match the formulation order"
    );
    let backend = scripted(vec![Ok(optimal_outcome(
        optimum.objective_value,
        optimum.column_values,
    ))]);
    let provider = HighsTier1Provider::new(backend);
    let mut result = provider
        .solve(problem, &SolverConfig::default())
        .expect("fixture solves");
    result.runtime = Duration::ZERO;
    serde_json::to_string_pretty(&result).expect("result serialises")
}

/// Every Tier 1 reference fixture (LP, QP, MILP — the three separate
/// formulation paths of P3.17 step 3) solves through the provider path with
/// its hand-computed optimum, and each record is complete.
#[test]
fn all_tier1_fixtures_solve_with_their_hand_computed_optima() {
    for problem in [fixture::reference_lp(), fixture::reference_qp(), fixture::reference_milp()] {
        let optimum = fixture_optimum(&problem);
        let backend = scripted(vec![Ok(optimal_outcome(
            optimum.objective_value,
            optimum.column_values,
        ))]);
        let provider = HighsTier1Provider::new(backend);
        let result = provider
            .solve(&problem, &SolverConfig::default())
            .expect("fixture solves");
        assert_eq!(result.status, SolveStatus::Optimal);
        assert_eq!(result.objective_value, Some(optimum.objective_value));
        assert_eq!(result.problem_class, problem.declared_class);
        match &result.primal {
            flexcore_optimization_api::result::Diagnostics::Available(primal) => {
                for (variable, value) in problem.variables.iter().zip(primal.values.iter()) {
                    assert_eq!(value.name, variable.name);
                    assert_eq!(value.unit, variable.unit.canonical_unit());
                }
                for (expected, value) in optimum.column_values.iter().zip(primal.values.iter()) {
                    assert_eq!(value.value, *expected);
                }
            }
            other => panic!("expected primal diagnostics, got {other:?}"),
        }
        result
            .validate_complete()
            .expect("record complete");
    }
}

/// Deterministic replay at the port level: same fixture + same scripted
/// outcome + same default configuration produce identical §12.6 records.
#[test]
fn replay_produces_identical_records() {
    let first = solve_and_serialise(&fixture::reference_lp());
    let second = solve_and_serialise(&fixture::reference_lp());
    assert_eq!(first, second, "same inputs + same config must replay identically");

    // The three formulation paths replay independently of each other.
    let qp_first = solve_and_serialise(&fixture::reference_qp());
    let qp_second = solve_and_serialise(&fixture::reference_qp());
    assert_eq!(qp_first, qp_second);
    let milp_first = solve_and_serialise(&fixture::reference_milp());
    let milp_second = solve_and_serialise(&fixture::reference_milp());
    assert_eq!(milp_first, milp_second);
}

/// A changed configuration changes the recorded digest — so a replay against
/// a different config cannot be mistaken for a replay of the same one, and
/// the per-result determinism record reflects the config actually used.
#[test]
fn config_digest_discriminates_replays() {
    let default_result = {
        let backend = scripted(vec![Ok(optimal_outcome(8.0, vec![4.0, 0.0]))]);
        HighsTier1Provider::new(backend)
            .solve(&fixture::reference_lp(), &SolverConfig::default())
            .expect("solves")
    };
    let mut other_config = SolverConfig::default();
    other_config.thread_count = 4;
    let other_result = {
        let backend = scripted(vec![Ok(optimal_outcome(8.0, vec![4.0, 0.0]))]);
        HighsTier1Provider::new(backend)
            .solve(&fixture::reference_lp(), &other_config)
            .expect("solves")
    };
    let default_digest = default_result
        .determinism
        .as_ref()
        .expect("determinism record")
        .config_digest
        .clone();
    let other_digest = other_result
        .determinism
        .as_ref()
        .expect("determinism record")
        .config_digest
        .clone();
    assert_ne!(default_digest, other_digest);
    assert_eq!(
        default_result.determinism.as_ref().expect("record").thread_count,
        1,
        "recorded determinism default"
    );
    assert_eq!(other_result.determinism.as_ref().expect("record").thread_count, 4);
}

// VERIFY-BUILD gate (P3.19): the identical replay assertions above re-run
// against the real Python-worker adapter with the pinned binding (highspy
// 1.15.1 — decision note §6 item 6). The scripted replay tests are the
// active port-level coverage in the meantime; no placeholder solver code is
// shipped in this crate.
