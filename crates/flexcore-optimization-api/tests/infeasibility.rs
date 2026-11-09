// P3.19 — Infeasibility policy and failure-handling tests (ledger P3.19
// steps 2–3; report §12.6).
//
// Acceptance coverage here:
// - ALL SIX §12.6 statuses are reachable through the Tier 1 port path
//   (optimal / feasible / infeasible / timeout / numerical_failure /
//   unsupported) — the ledger's acceptance criterion.
// - The timeout status construction, including the time-limit-hit outcome
//   WITH an incumbent payload (VERIFY-BUILD on the exact Tier 1 binding
//   signal for the time-limit-hit model status — decision note
//   17_optimizer_port.md §6 item 1).
// - The infeasibility-evidence policy: an infeasible run carries conflict
//   evidence or an explicit not-available reason — NEVER a silently relaxed
//   schedule — and any relaxation requires a named, reviewed decision
//   ([`RelaxationDecision`] / [`RelaxedScheduleRecord`]).

mod common;

use common::{outcome, scripted};
use flexcore_optimization_api::backend::{BackendOutcome, BackendStatus};
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::highs_provider::HighsTier1Provider;
use flexcore_optimization_api::infeasibility::{
    evidence_from_result, InfeasibilityPolicyError, RelaxationDecision, RelaxedScheduleRecord,
    INFEASIBILITY_POLICY,
};
use flexcore_optimization_api::port::Optimizer;
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::result::{Diagnostics, SolveStatus};
use flexcore_contracts::ids::FlexUuid;

fn provider(backend: std::sync::Arc<common::ScriptedBackend>) -> HighsTier1Provider {
    HighsTier1Provider::new(backend)
}

/// Acceptance criterion (ledger P3.19): all six §12.6 statuses are reachable
/// in tests through the Tier 1 port path — five via scripted solver outcomes,
/// `unsupported` via the class gate (no solver invocation).
#[test]
fn all_six_statuses_are_reachable_through_the_tier1_port_path() {
    let problem = fixture::reference_lp();
    let column_count = problem.variables.len();

    // optimal / feasible / infeasible / timeout / numerical_failure — each
    // from a backend outcome the pinned solver can reach (decision note §3).
    let cases = [
        (BackendStatus::Optimal, SolveStatus::Optimal, true),
        (BackendStatus::Feasible, SolveStatus::Feasible, true),
        (BackendStatus::Infeasible, SolveStatus::Infeasible, false),
        (BackendStatus::TimeLimitReached, SolveStatus::Timeout, false),
        (
            BackendStatus::IterationLimitReached,
            SolveStatus::Timeout,
            false,
        ),
        (BackendStatus::Unbounded, SolveStatus::NumericalFailure, false),
        (BackendStatus::Unknown, SolveStatus::NumericalFailure, false),
    ];
    for (backend_status, expected, solved) in cases {
        let backend = outcome(
            backend_status,
            "fixture status",
            if solved { Some(8.0) } else { None },
            if solved {
                Some(vec![0.0; column_count])
            } else {
                None
            },
        );
        let result = provider(scripted(vec![Ok(backend)]))
            .solve(&problem, &SolverConfig::default())
            .expect("in-band outcome");
        assert_eq!(result.status, expected, "backend {backend_status:?}");
        result
            .validate_complete()
            .expect("every reachable record is complete");
    }

    // unsupported — via the class gate, before any solver invocation.
    let backend = scripted(Vec::new());
    let result = provider(backend)
        .solve(
            &fixture::reference_misocp_declaration(),
            &SolverConfig::default(),
        )
        .expect("unsupported is in-band");
    assert_eq!(result.status, SolveStatus::Unsupported);

    // The closed vocabulary: exactly these six strings parse.
    for text in [
        "optimal",
        "feasible",
        "infeasible",
        "timeout",
        "numerical_failure",
        "unsupported",
    ] {
        assert_eq!(
            SolveStatus::parse(text).map(SolveStatus::as_str),
            Some(text)
        );
    }
}

/// The timeout status construction from the solver's time-limit-hit outcome,
/// WITH an incumbent payload (the pinned Tier 1 solver reports its
/// time-limit model status and may still carry a feasible incumbent):
/// the recorded table maps the outcome to `timeout`, and the incumbent
/// payload is recorded without any optimality claim (VERIFY-BUILD on the
/// exact binding signal for the time-limit-hit model status — decision note
/// 17_optimizer_port.md §6 item 1).
#[test]
fn timeout_with_incumbent_maps_to_timeout_and_keeps_the_verbatim_status() {
    let problem = fixture::reference_lp();
    let mut limited = outcome(
        BackendStatus::TimeLimitReached,
        "Time limit reached",
        Some(8.0),
        Some(vec![4.0, 0.0]),
    );
    limited.iteration_count = Some(120);
    let result = provider(scripted(vec![Ok(limited)]))
        .solve(&problem, &SolverConfig::default())
        .expect("limited solve maps in-band");

    assert_eq!(result.status, SolveStatus::Timeout);
    // No optimality claim: the objective is an incumbent value, and the
    // verbatim backend status is recorded so the limit kind is auditable.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Time limit reached") && w.contains("timeout")),
        "verbatim status recorded: {:?}",
        result.warnings
    );
    assert!(result.primal.is_available(), "the incumbent is recorded");
    // The determinism record carries the configured time limit.
    let determinism = result.determinism.as_ref().expect("determinism record");
    assert_eq!(determinism.thread_count, 1);
    result
        .validate_complete()
        .expect("the timeout record is complete");
}

/// The infeasibility policy: an infeasible result carries conflict evidence
/// bridged into the policy-level type, and NEVER a primal solution (no
/// silently relaxed schedule).
#[test]
fn infeasible_result_yields_conflict_evidence_and_never_a_schedule() {
    let problem = fixture::reference_lp();
    let mut infeasible = outcome(BackendStatus::Infeasible, "Infeasible", None, None);
    infeasible.conflict =
        Some(flexcore_optimization_api::backend::ConflictEvidence {
            variables: vec!["bess_a_discharge".to_string()],
            rows: vec![
                "portfolio_export_floor".to_string(),
                "capacity_envelope".to_string(),
            ],
            report: "conflict refinement: export floor vs capacity".to_string(),
        });
    let result = provider(scripted(vec![Ok(infeasible)]))
        .solve(&problem, &SolverConfig::default())
        .expect("infeasible maps in-band");

    assert_eq!(result.status, SolveStatus::Infeasible);
    assert!(!result.primal.is_available(), "no relaxed schedule");
    assert!(result.objective_value.is_none());

    let evidence = evidence_from_result(&result).expect("evidence bridges");
    evidence.validate().expect("evidence is complete");
    assert!(evidence.is_available());
    assert_eq!(
        evidence.conflicting_constraints,
        vec![
            "portfolio_export_floor".to_string(),
            "capacity_envelope".to_string(),
            "bess_a_discharge".to_string(),
        ]
    );
    assert!(evidence.solver_excerpt.contains("conflict refinement"));
}

/// Where the solver cannot provide evidence, the record — and the bridged
/// policy evidence — carries an explicit not-available reason (never
/// silence).
#[test]
fn infeasible_without_solver_evidence_carries_an_explicit_reason() {
    let problem = fixture::reference_lp();
    let result = provider(scripted(vec![Ok(outcome(
        BackendStatus::Infeasible,
        "Infeasible",
        None,
        None,
    ))]))
    .solve(&problem, &SolverConfig::default())
    .expect("infeasible maps in-band");

    assert_eq!(result.status, SolveStatus::Infeasible);
    let evidence = evidence_from_result(&result).expect("bridges");
    assert!(!evidence.is_available());
    let reason = evidence
        .not_available_reason
        .as_deref()
        .expect("explicit reason recorded");
    assert!(reason.contains("no conflict/IIS evidence"));
    evidence.validate().expect("the reason is complete");
}

/// Any relaxation requires a NAMED, REVIEWED decision recorded in the
/// schedule: the full policy flow — infeasible run, named decision, recorded
/// relaxed schedule — and the rejections that make a silent relaxation
/// unrepresentable.
#[test]
fn relaxation_requires_a_named_reviewed_decision_recorded_in_the_schedule() {
    assert!(
        INFEASIBILITY_POLICY.contains("NEVER a silently relaxed schedule"),
        "the policy statement is present"
    );
    assert!(
        INFEASIBILITY_POLICY.contains("DepartureEnergyPolicy::Hard"),
        "the policy names the declared-hard departure-energy policy \
         (flexcore-domain, the ONLY variant of that policy)"
    );

    // The named, reviewed decision (e.g. softening the EV departure-energy
    // hard policy for one documented scenario).
    let decision = RelaxationDecision {
        decision_name: "departure-energy-softening-2026-11".to_string(),
        review_reference: "review/2026-11-07-departure-energy".to_string(),
        relaxed_constraint_refs: vec!["ev_departure_energy_floor".to_string()],
        rationale: "fleet energy shortfall documented and accepted for this \
                    scenario; recorded per P3.19 step 3"
            .to_string(),
    };
    decision.validate().expect("the decision is complete");

    // The schedule record (P3.21/P3.22 embeds this) can only be created
    // WITH the decision.
    let candidate = FlexUuid::parse("00000000-0000-0000-0000-0000000c4d1d")
        .expect("candidate id parses");
    let record = RelaxedScheduleRecord::new(
        candidate,
        decision.clone(),
        vec!["ev_departure_energy_floor".to_string()],
    )
    .expect("a reviewed relaxation records");
    assert_eq!(record.decision.review_reference, decision.review_reference);

    // A relaxation outside the reviewed scope is unrepresentable.
    assert_eq!(
        RelaxedScheduleRecord::new(
            candidate,
            decision.clone(),
            vec!["unrelated_constraint".to_string()],
        ),
        Err(InfeasibilityPolicyError::RelaxationBeyondReviewedScope {
            constraint: "unrelated_constraint".to_string()
        })
    );

    // An unnamed or unreviewed "decision" backs nothing.
    let mut unnamed = decision.clone();
    unnamed.decision_name = String::new();
    assert!(matches!(
        RelaxedScheduleRecord::new(
            candidate,
            unnamed,
            vec!["ev_departure_energy_floor".to_string()],
        ),
        Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
            field: "decision_name"
        })
    ));
    let mut unreviewed = decision;
    unreviewed.review_reference = String::new();
    assert!(matches!(
        RelaxedScheduleRecord::new(
            candidate,
            unreviewed,
            vec!["ev_departure_energy_floor".to_string()],
        ),
        Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
            field: "review_reference"
        })
    ));
}

/// The bridge rejects a result-record shape the providers cannot produce
/// (a not-available diagnostic with an empty reason) — defence in depth for
/// the policy's "never silence" rule.
#[test]
fn bridge_rejects_silence() {
    let mut result = provider(scripted(Vec::new()))
        .solve(
            &fixture::reference_nonlinear_declaration(),
            &SolverConfig::default(),
        )
        .expect("unsupported is in-band");
    // Simulate a provider bug: silence where a reason is required.
    result.infeasibility_evidence = Diagnostics::NotAvailable {
        reason: String::new(),
    };
    let evidence = evidence_from_result(&result).expect("bridges structurally");
    assert_eq!(
        evidence.validate(),
        Err(InfeasibilityPolicyError::EmptyNotAvailableReason),
        "the policy layer rejects silence even if a provider emitted it"
    );
    // The declared class of this record is outside Tier 1 (the rejection
    // path used above); the P3.20 guard covers the class-level boundary.
    assert_eq!(result.problem_class, ProblemClass::NonlinearProgram);
}
