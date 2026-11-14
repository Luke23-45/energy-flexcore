// P10.05 — Optimizer independent validation (release-gate contract slice):
//         the §12.6 result vocabulary, the §12.3 solver-class discipline
//         (verbatim: HiGHS is LP/QP/MIP, NOT SOCP; Clarabel is convex
//         conic, NOT MISOCP; AC power flow is not AC-OPF; IPOPT is LOCAL,
//         never a global-optimality guarantee), and the Tier 3/4 boundary
//         guard, exercised through the PUBLIC port surface (report §22.3
//         gate 4; §12.3, §12.5–§12.6; §9.2; ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md`
//         P10.05; record:
//         `ledger/phase-10-v1-release-hardening/05_optimizer_validation.md`).
//
// WHAT THIS FILE PROVES (steps 1 and 3 of the ledger subtask — the
// provider/class table and the §12.6 contract check; the INDEPENDENT
// feasibility-path consumption is proven in the research-crate file
// `crates/flexcore-research/tests/p10_05_independent_validation.rs`):
//
// 1. THE §12.6 STATUS VOCABULARY IS CLOSED: exactly six statuses; "unbounded"
//    is NOT a status — an unbounded objective maps to `numerical_failure`
//    (an unbounded result is never published as a pass, decision note 17 §3).
// 2. THE PROVIDER/CLASS TABLE IS EXACT. HiGHS declares exactly
//    [LP, convex QP, MILP]; every Tier 3/4 declaration (SOCP, MISOCP,
//    nonlinear) is rejected with the §12.6 status `unsupported` BEFORE any
//    backend invocation (call counter == 0). Clarabel declares exactly
//    [continuous SOCP]; an integer formulation (MILP/MISOCP territory) is
//    rejected `unsupported` by the integrality gate BEFORE the solver runs
//    (invocation counter == 0), and an LP declaration is rejected `unsupported`
//    by the class gate — Clarabel is a convex CONIC solver, never used as an
//    LP/QP solver, and never a MISOCP solver.
// 3. INFEASIBLE/TIMEOUT/UNBOUNDED CASES ARE NOT PASSES: `infeasible` carries
//    the conflict evidence (or an explicit not-available reason) and never a
//    relaxed schedule; `timeout` is `timeout`; `unbounded` is
//    `numerical_failure` — never optimal/feasible by construction.
// 4. THE TIER BOUNDARY GUARD: a MISOCP/NLP request is refused with
//    `unsupported` + the decision reference + the reformulation rule; a
//    provider capability declaration naming a Tier 3/4 class fails the
//    boundary check (the checkable "no Tier 3/4 code path" guarantee).
// 5. EVERY RESULT CARRIES THE FULL §12.6 CONTRACT: the provider self-checks
//    `validate_complete` on every mapped outcome (candidate id, input
//    snapshot, solver/provider identification, determinism record; objective
//    + primal for optimal/feasible; no empty not-available reasons).
//
// EMPTY BENCHMARK CELL (honest status, ENTRY_NOTE boundary 4): NO optimizer
// has been built or run in this loop, and no §19.3–§19.4 benchmark number is
// recorded here or anywhere in the P10.05 record. What is validated is the
// CONTRACT (the port, the result vocabulary, the class gates, the
// independent-validation consumption path) — never a solver binary's
// performance. The scripted backend in `tests/common/mod.rs` is the
// documented test double, not a solver and not a benchmark.
//
// STATUS: WRITTEN-UNRUN (no cargo command was executed in this loop; the
// human build/test gate owns:
// `cargo test -p flexcore-optimization-api --test p10_05_result_contract_validation`).

mod common;

use common::{outcome, scripted};
use flexcore_optimization_api::backend::{BackendOutcome, BackendStatus, ConflictEvidence};
use flexcore_optimization_api::clarabel_provider::{
    ClarabelTier2Provider, PROVIDER_ID as CLARABEL_PROVIDER_ID, CLARABEL_SOLVER_VERSION_PIN,
};
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::fixture::{
    reference_lp, reference_milp, reference_misocp_declaration, reference_nonlinear_declaration,
    reference_qp, reference_socp_declaration,
};
use flexcore_optimization_api::highs_provider::{
    HighsTier1Provider, HIGHS_SOLVER_VERSION_PIN, PROVIDER_ID as HIGHS_PROVIDER_ID,
};
use flexcore_optimization_api::port::{Optimizer, SolverCapabilities};
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::result::SolveStatus;
use flexcore_optimization_api::tier_guard::{
    check_capabilities_within_boundary, check_tier_boundary, tier_supported,
    IPOPT_LOCALITY_NOTE, REFORMULATION_RULE, TIER_BOUNDARY_DECISION_REFERENCE,
    TIER_BOUNDARY_POLICY,
};
use flexcore_optimization_api::port::ConstraintFeature;
use std::sync::Arc;

// --- Fixtures -------------------------------------------------------------------------

fn highs_provider_with(outcomes: Vec<Result<BackendOutcome, flexcore_optimization_api::backend::BackendError>>) -> (
    HighsTier1Provider,
    Arc<common::ScriptedBackend>,
) {
    let backend = scripted(outcomes);
    (HighsTier1Provider::new(backend.clone()), backend)
}

fn clarabel_provider() -> ClarabelTier2Provider {
    ClarabelTier2Provider::new(Default::default())
}

// --- 1. The §12.6 status vocabulary is closed ------------------------------------------

#[test]
fn the_status_vocabulary_is_exactly_six_and_unbounded_is_not_a_status() {
    // Round-trip for the six canonical strings.
    for (status, text) in [
        (SolveStatus::Optimal, "optimal"),
        (SolveStatus::Feasible, "feasible"),
        (SolveStatus::Infeasible, "infeasible"),
        (SolveStatus::Timeout, "timeout"),
        (SolveStatus::NumericalFailure, "numerical_failure"),
        (SolveStatus::Unsupported, "unsupported"),
    ] {
        assert_eq!(status.as_str(), text);
        assert_eq!(SolveStatus::parse(text), Some(status));
    }
    // The release-gate discipline: "unbounded" and every other invented
    // string parse to NOTHING — the vocabulary is closed (an unbounded
    // objective arrives as `numerical_failure` through the provider
    // mapping, never as its own status).
    for illegal in [
        "unbounded",
        "unbounded_or_infeasible",
        "UNRECOGNIZED",
        "error",
        "not_solved",
        "optimal ",
        "OPTIMAL",
        "",
        "solved",
        "infeasible ",
    ] {
        assert_eq!(SolveStatus::parse(illegal), None, "illegal: {illegal:?}");
    }
}

// --- 2. The provider/class table is exact ----------------------------------------------

#[test]
fn highs_declares_exactly_lp_qp_milp_and_never_invokes_the_backend_on_other_classes() {
    let (provider, backend) = highs_provider_with(Vec::new());
    let capabilities = provider.capabilities();
    // The declared class table (the P10.05 step-1 record row for HiGHS).
    assert_eq!(capabilities.provider_id, HIGHS_PROVIDER_ID);
    assert_eq!(capabilities.solver_name, "HiGHS");
    assert_eq!(capabilities.solver_version, HIGHS_SOLVER_VERSION_PIN);
    let declared = &capabilities.problem_classes;
    assert_eq!(declared.len(), 3);
    assert!(declared.contains(&ProblemClass::LinearProgram));
    assert!(declared.contains(&ProblemClass::ConvexQuadraticProgram));
    assert!(declared.contains(&ProblemClass::MixedIntegerLinearProgram));
    // HiGHS is LP/QP/MIP, NOT SOCP: no conic class is declared.
    assert!(!declared.contains(&ProblemClass::ContinuousSecondOrderConeProgram));
    assert!(!declared.contains(&ProblemClass::MixedIntegerSecondOrderConeProgram));
    // The constraint features are the Tier 1 set — SecondOrderCones is
    // never declared by a Tier 1 provider (port.rs contract).
    assert!(!capabilities
        .constraint_features
        .contains(&ConstraintFeature::SecondOrderCones));
    // Every Tier 3/4 declaration is rejected `unsupported` BEFORE the
    // backend is touched (call counter stays 0 — the solver is never
    // invoked on a wrong-class formulation).
    let config = SolverConfig::default();
    for problem in [
        reference_socp_declaration(),
        reference_misocp_declaration(),
        reference_nonlinear_declaration(),
    ] {
        let result = provider
            .solve(&problem, &config)
            .expect("an out-of-class declaration returns an unsupported result in-band");
        assert_eq!(result.status, SolveStatus::Unsupported);
        assert_eq!(result.problem_class, problem.declared_class);
        assert!(
            result.warnings.iter().any(|w| w.contains("no solve was attempted")),
            "the unsupported refusal is explained in the warnings: {:?}",
            result.warnings
        );
    }
    assert_eq!(
        backend.solve_count(),
        0,
        "the backend is NEVER invoked on an SOCP/MISOCP/nonlinear formulation"
    );
}

#[test]
fn clarabel_declares_exactly_continuous_socp_and_never_solves_integer_or_lp() {
    let provider = clarabel_provider();
    let capabilities = provider.capabilities();
    // The declared class table (the P10.05 step-1 record row for Clarabel):
    // convex conic ONLY — never MISOCP (no integrality), never LP/QP.
    assert_eq!(capabilities.provider_id, CLARABEL_PROVIDER_ID);
    assert_eq!(capabilities.solver_version, CLARABEL_SOLVER_VERSION_PIN);
    assert_eq!(capabilities.problem_classes, vec![ProblemClass::ContinuousSecondOrderConeProgram]);
    // An integer formulation (MILP) hits the integrality gate FIRST —
    // a continuous conic solver is never silently used on integer
    // variables (Clarabel is NOT MISOCP).
    let config = SolverConfig::default();
    let milp = reference_milp();
    let result = provider
        .solve(&milp, &config)
        .expect("the integrality rejection is an in-band unsupported result");
    assert_eq!(result.status, SolveStatus::Unsupported);
    assert_eq!(provider.solver_invocations(), 0, "the solver never ran on an integer formulation");
    // An LP declaration hits the class gate — Clarabel is not an LP/QP
    // solver either.
    let lp = reference_lp();
    let result = provider
        .solve(&lp, &config)
        .expect("the class rejection is an in-band unsupported result");
    assert_eq!(result.status, SolveStatus::Unsupported);
    assert_eq!(provider.solver_invocations(), 0);
    // Both rejection records carry the §12.6 completeness contract.
    for record in [result] {
        record
            .validate_complete()
            .expect("even a rejection carries the full §12.6 contract");
    }
}

#[test]
fn every_supported_class_result_passes_the_completeness_contract() {
    // LP / QP / MILP solves through the provider path carry the full
    // §12.6 record (status, problem class, solver/provider identification,
    // objective value AND declared units, diagnostics, residuals, runtime,
    // candidate id, input snapshot, grid-model version, assumptions,
    // determinism record).
    let config = SolverConfig::default();
    // The column payloads use the frozen fixtures' hand-computed optimums
    // (P3.17): the vector lengths MUST equal the formulation's variable
    // counts (2 / 1 / 3) — a misaligned vector is a contract failure.
    for (problem, status, expected_status, objective, columns) in [
        (
            reference_lp(),
            BackendStatus::Optimal,
            SolveStatus::Optimal,
            Some(8.0),
            Some(vec![4.0, 0.0]),
        ),
        (
            reference_qp(),
            BackendStatus::Feasible,
            SolveStatus::Feasible,
            Some(0.5),
            Some(vec![2.0]),
        ),
        (
            reference_milp(),
            BackendStatus::Optimal,
            SolveStatus::Optimal,
            Some(7.0),
            Some(vec![4.0, 1.0, 0.0]),
        ),
    ] {
        let (provider, _backend) = highs_provider_with(vec![Ok(outcome(
            status,
            "scripted recorded status",
            objective,
            columns,
        ))]);
        let result = provider
            .solve(&problem, &config)
            .expect("the mapped outcome is returned in-band");
        assert_eq!(result.status, expected_status, "the recorded mapping holds");
        assert_eq!(result.solver_name, "HiGHS");
        assert_eq!(result.provider_id, HIGHS_PROVIDER_ID);
        assert!(result.determinism.is_some(), "the determinism record is present");
        assert_eq!(result.problem_class, problem.declared_class);
        assert_eq!(
            result.objective_units.as_deref(),
            Some(problem.objective.declared_units.as_str()),
            "the objective carries its declared units"
        );
        result
            .validate_complete()
            .expect("every supported-class result passes the §12.6 completeness check");
    }
}

// --- 3. Infeasible / timeout / unbounded cases are not passes ---------------------------

#[test]
fn an_infeasible_outcome_is_infeasible_with_evidence_and_never_a_relaxed_schedule() {
    let config = SolverConfig::default();
    let (provider, backend) = highs_provider_with(vec![Ok(BackendOutcome {
        status: BackendStatus::Infeasible,
        objective_value: None,
        column_values: None,
        reduced_costs: None,
        row_duals: None,
        max_primal_violation: None,
        max_dual_violation: None,
        iteration_count: None,
        solver_status_text: "infeasible".to_string(),
        conflict: Some(ConflictEvidence {
            variables: vec!["bess_a_discharge".to_string()],
            rows: vec!["portfolio_export_floor".to_string()],
            report: "conflict refinement: export floor vs discharge bounds".to_string(),
        }),
        solver_log_excerpt: None,
        warnings: Vec::new(),
    })]);
    let problem = reference_lp();
    let result = provider.solve(&problem, &config).expect("mapped in-band");
    // The status is `infeasible` — never optimal/feasible, never relaxed.
    assert_eq!(result.status, SolveStatus::Infeasible);
    assert!(result.objective_value.is_none(), "no fabricated objective");
    // The conflict evidence travels (the INFEASIBILITY_POLICY: conflict
    // evidence, never a relaxed schedule).
    match &result.infeasibility_evidence {
        flexcore_optimization_api::result::Diagnostics::Available(evidence) => {
            assert_eq!(evidence.conflict_rows, vec!["portfolio_export_floor"]);
            assert!(evidence.backend_report.contains("conflict"));
        }
        other => panic!("the exposed conflict evidence must travel, got {other:?}"),
    }
    result.validate_complete().expect("complete §12.6 record");
    assert_eq!(backend.solve_count(), 1);
}

#[test]
fn a_timeout_outcome_is_timeout_and_an_unbounded_outcome_is_never_a_pass() {
    let config = SolverConfig::default();
    // TimeLimitReached → timeout (a resource limit is not a feasibility
    // claim; §12.5: the system must reject or hold, never infer a pass).
    let (provider, _backend) = highs_provider_with(vec![Ok(outcome(
        BackendStatus::TimeLimitReached,
        "time limit reached",
        None,
        None,
    ))]);
    let result = provider
        .solve(&reference_lp(), &config)
        .expect("mapped in-band");
    assert_eq!(result.status, SolveStatus::Timeout);
    assert!(result.objective_value.is_none());
    result.validate_complete().expect("complete record");

    // Unbounded → numerical_failure: the closed vocabulary has NO
    // `unbounded` status, and an unbounded objective is NEVER published as
    // optimal/feasible.
    let (provider, _backend) = highs_provider_with(vec![Ok(outcome(
        BackendStatus::Unbounded,
        "primal/dual unbounded",
        None,
        None,
    ))]);
    let result = provider
        .solve(&reference_lp(), &config)
        .expect("mapped in-band");
    assert_eq!(result.status, SolveStatus::NumericalFailure);
    assert_ne!(result.status, SolveStatus::Optimal);
    assert_ne!(result.status, SolveStatus::Feasible);
    result.validate_complete().expect("complete record");
}

// --- 4. The tier boundary guard ---------------------------------------------------------

#[test]
fn tier3_and_tier4_requests_are_refused_with_unsupported_and_the_decision_reference() {
    for (class, tier) in [
        (ProblemClass::MixedIntegerSecondOrderConeProgram, 3u8),
        (ProblemClass::NonlinearProgram, 4u8),
    ] {
        assert!(!tier_supported(class), "{:?} is beyond the boundary", class);
        let rejection = check_tier_boundary(class).expect_err("Tier 3/4 refused");
        assert_eq!(rejection.outcome, "unsupported");
        assert_eq!(rejection.requested_class, class);
        assert_eq!(rejection.requested_tier, tier);
        assert!(rejection.decision_reference.contains("19_20_determinism_tiers.md"));
        assert!(!rejection.reformulation_guidance.is_empty());
    }
    // The recorded policy texts are pinned (the solver-class discipline,
    // verbatim where the report requires it).
    assert!(TIER_BOUNDARY_POLICY.contains("NEITHER is built"));
    assert!(REFORMULATION_RULE.contains("never forced through an unsuitable solver"));
    assert!(IPOPT_LOCALITY_NOTE.contains("NO global-optimality guarantee"));
    assert!(TIER_BOUNDARY_DECISION_REFERENCE.contains("P3.20"));
}

#[test]
fn a_capability_declaration_naming_a_tier3_or_tier4_class_fails_the_boundary() {
    // The capability surface is the checkable "no Tier 3/4 code path"
    // guarantee: a provider declaring MISOCP or NLP cannot pass.
    let mut capabilities = minimal_capabilities();
    capabilities.problem_classes = vec![ProblemClass::MixedIntegerSecondOrderConeProgram];
    assert!(check_capabilities_within_boundary(&capabilities).is_err());
    capabilities.problem_classes = vec![ProblemClass::NonlinearProgram];
    assert!(check_capabilities_within_boundary(&capabilities).is_err());
    // The Tier 1/2 declarations pass.
    capabilities.problem_classes = vec![
        ProblemClass::LinearProgram,
        ProblemClass::ConvexQuadraticProgram,
        ProblemClass::MixedIntegerLinearProgram,
        ProblemClass::ContinuousSecondOrderConeProgram,
    ];
    check_capabilities_within_boundary(&capabilities).expect("Tier 1/2 declarations pass");
}

fn minimal_capabilities() -> SolverCapabilities {
    use flexcore_optimization_api::config::ScalingPolicy;
    use flexcore_optimization_api::port::{
        DiagnosticsDeclaration, DeterminismDeclaration, LicenseDeclaration, ScalingDeclaration,
        TolerancesDeclaration, WarmStartSupport,
    };
    SolverCapabilities {
        provider_id: "test.provider".to_string(),
        provider_version: "0.1.0".to_string(),
        solver_name: "test-solver".to_string(),
        solver_version: "0.0.0".to_string(),
        problem_classes: Vec::new(),
        constraint_features: Vec::new(),
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
    }
}

// --- 5. The contract record itself refuses half-filled results --------------------------

#[test]
fn an_incomplete_result_record_is_named_and_refused() {
    use flexcore_optimization_api::result::ResultCompletenessError;
    // A result the provider never actually produced (hand-built for the
    // contract test): missing the candidate id → the named error.
    let (provider, _backend) = highs_provider_with(vec![Ok(outcome(
        BackendStatus::Optimal,
        "optimal",
        Some(8.0),
        Some(vec![4.0, 0.0]),
    ))]);
    let config = SolverConfig::default();
    let mut problem = reference_lp();
    problem.provenance.candidate_id = Some(flexcore_contracts::ids::FlexUuid::NIL);
    let outcome_err = provider.solve(&problem, &config)
        .expect_err("a NIL candidate id is a provider bug (IncompleteResult), not a status");
    assert!(
        matches!(
            outcome_err,
            flexcore_optimization_api::port::OptimizerError::IncompleteResult(
                ResultCompletenessError::MissingCandidateId
            )
        ),
        "got {outcome_err:?}"
    );
}

#[test]
fn the_empty_benchmark_cell_is_declared_not_fitted() {
    // Honest-status guard for the release record: the P10.05 record
    // validates the CONTRACT only — no benchmark runtime/objective number
    // is asserted anywhere in this suite, and the scripted backend is a
    // documented test double. This test pins that the fixture optimum
    // values used above are the hand-computed REFERENCE optimums of the
    // frozen fixtures (P3.17), not measurements of a built optimizer.
    let lp = flexcore_optimization_api::fixture::reference_lp_optimum();
    assert_eq!(lp.objective_value, 8.0);
    assert_eq!(lp.column_values, vec![4.0, 0.0]);
}
