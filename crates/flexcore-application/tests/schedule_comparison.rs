// P3.23 — Schedule comparison workflow tests (report §23 Stage 3, §19.4;
//         ledger P3.23).
//
// Covered, without any solver or I/O (the comparison is a pure application
// function):
//
// - comparisons on SHARED inputs produce all four §19.4 scheduling metrics
//   per candidate (objective value + units, schedule deviation, feasible
//   interval count, runtime);
// - the cross-input policy (D-P3.23-2): a comparison differing in ANY
//   modeling constituent (envelope version, forecast version, provenance,
//   objective name, objective config record fields, telemetry reference) is
//   REJECTED with the named `IncomparableInputs` error — never labelled
//   into acceptance; the display-side labelling path is separate;
// - a comparison is over produced schedules (non-solving status rejected)
//   and requires the metrics (missing/blank metric rejected) and at least
//   two candidates;
// - the FIRST comparison case: the P3.18 Tier 1 vs Tier 2 cross-check (the
//   recorded 1 MW linearisation gap) builds the record, its notes travel,
//   and the gap evidence is recorded with its interpretation;
// - the recorded statements name the P3.28 REST wiring (no routes, no UI
//   in this phase) and the candidate status of both sides.

use std::time::Duration;

use flexcore_application::schedule_comparison::{
    compare_candidate_schedules, incomparability_label, tier1_tier2_comparison_case,
    CandidateComparisonEntry, CandidateModelingInputs, ComparisonError, SharedComparisonInputs,
    Tier1Tier2ComparisonCaseInput, COMPARISON_API_WIRING_NOTE, COMPARISON_CANDIDATE_STATEMENT,
    COMPARISON_METRICS_STATEMENT, CROSS_INPUT_POLICY,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_optimization_api::conic_fixture::TierComparisonSide;
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::result::SolveStatus;

fn tenant() -> TenantId {
    TenantId::new(FlexUuid::nil())
}

fn portfolio() -> PortfolioId {
    PortfolioId::new(FlexUuid::parse("00000000-0000-0000-0000-00000000c001").expect("parses"))
}

fn site() -> SiteId {
    SiteId::new(FlexUuid::parse("00000000-0000-0000-0000-00000000c002").expect("parses"))
}

fn comparison_id() -> FlexUuid {
    FlexUuid::parse("00000000-0000-0000-0000-00000000c003").expect("parses")
}

fn shared_inputs() -> SharedComparisonInputs {
    SharedComparisonInputs {
        input_snapshot_id: "schedule_input_snapshot:env-v1:v1:synthetic:telemetry=t1".to_string(),
        envelope_version: "env-v1".to_string(),
        forecast_version: "v1".to_string(),
        forecast_provenance: "synthetic".to_string(),
        objective_name: "default-synthetic-v1".to_string(),
        objective_config_record: vec![
            ("objective.name".to_string(), "default-synthetic-v1".to_string()),
            ("objective.provenance".to_string(), "synthetic".to_string()),
        ],
        telemetry_snapshot_reference: "telemetry-snapshot-2026-11-07".to_string(),
    }
}

fn candidate(candidate_id: &str, problem_class: &str) -> CandidateComparisonEntry {
    CandidateComparisonEntry {
        candidate_id: candidate_id.to_string(),
        result_input_snapshot_reference: format!(
            "schedule_input_snapshot:env-v1:v1:synthetic:digest-{problem_class}"
        ),
        modeling: CandidateModelingInputs {
            envelope_version: "env-v1".to_string(),
            forecast_version: "v1".to_string(),
            forecast_provenance: "synthetic".to_string(),
            objective_name: "default-synthetic-v1".to_string(),
            objective_config_record: vec![
                ("objective.name".to_string(), "default-synthetic-v1".to_string()),
                ("objective.provenance".to_string(), "synthetic".to_string()),
            ],
            telemetry_snapshot_reference: "telemetry-snapshot-2026-11-07".to_string(),
        },
        problem_class: problem_class.to_string(),
        solver_name: "highs".to_string(),
        solver_version: "1.15.1".to_string(),
        provider_id: "tier1-python-worker".to_string(),
        provider_version: "0.1.0".to_string(),
        status: "optimal".to_string(),
        objective_value: -12.5,
        objective_units: "currency-units".to_string(),
        schedule_deviation_mw: 0.25,
        feasible_interval_count: 96,
        runtime: Duration::from_millis(12),
    }
}

#[test]
fn a_shared_input_comparison_produces_all_four_scheduling_metrics_per_candidate() {
    let record = compare_candidate_schedules(
        tenant(),
        portfolio(),
        site(),
        comparison_id(),
        shared_inputs(),
        vec![candidate("c1", "lp"), candidate("c2", "socp")],
    )
    .expect("the shared-input comparison assembles");

    assert_eq!(record.comparison_id(), comparison_id());
    assert_eq!(record.candidates().len(), 2);
    assert_eq!(record.candidate_ids(), vec!["c1", "c2"]);
    for entry in record.candidates() {
        // The four §19.4 scheduling metrics, all present per candidate.
        assert!(entry.objective_value.is_finite(), "metric 1: objective value");
        assert!(!entry.objective_units.trim().is_empty(), "metric 1: units");
        assert!(entry.schedule_deviation_mw.is_finite(), "metric 2: schedule deviation");
        assert!(entry.feasible_interval_count >= 1, "metric 3: feasible interval count");
        // Metric 4 (runtime) is a required Duration field on the entry — its
        // presence is proven by the field access compiling; no lower bound is
        // imposed (§19.4: no arbitrary targets).
        let _ = entry.runtime;
    }
    // The tier (formulation/solver class) is the varied factor and may
    // differ; the shared inputs are recorded once.
    assert_eq!(record.candidates()[0].problem_class, "lp");
    assert_eq!(record.candidates()[1].problem_class, "socp");
    assert_eq!(record.shared_inputs().envelope_version, "env-v1");
    // The per-candidate §12.6 snapshot references may differ by the
    // solver-config digest (recorded verbatim, D-P3.23-2).
    assert_ne!(
        record.candidates()[0].result_input_snapshot_reference,
        record.candidates()[1].result_input_snapshot_reference
    );
    // The recorded notes state the policy and the candidate status.
    assert!(record.notes().iter().any(|n| n == CROSS_INPUT_POLICY));
    assert!(record.notes().iter().any(|n| n == COMPARISON_CANDIDATE_STATEMENT));
}

#[test]
fn a_cross_input_comparison_is_rejected_with_the_named_error_for_every_constituent() {
    let cases: Vec<(&'static str, CandidateComparisonEntry)> = vec![
        (
            "envelope_version",
            candidate_with(|m| m.envelope_version = "env-v2".to_string()),
        ),
        (
            "forecast_version",
            candidate_with(|m| m.forecast_version = "v2".to_string()),
        ),
        (
            "forecast_provenance",
            candidate_with(|m| m.forecast_provenance = "measured".to_string()),
        ),
        (
            "objective_name",
            candidate_with(|m| m.objective_name = "other-objective".to_string()),
        ),
        (
            "objective_config_record",
            candidate_with(|m| {
                m.objective_config_record
                    .push(("objective.energy.weight".to_string(), "9.0".to_string()))
            }),
        ),
        (
            "telemetry_snapshot_reference",
            candidate_with(|m| m.telemetry_snapshot_reference = "other-telemetry".to_string()),
        ),
    ];
    for (field, differing) in cases {
        let error = compare_candidate_schedules(
            tenant(),
            portfolio(),
            site(),
            comparison_id(),
            shared_inputs(),
            vec![candidate("c1", "lp"), differing],
        )
        .expect_err("a cross-input comparison must be REJECTED (D-P3.23-2)");
        match &error {
            ComparisonError::IncomparableInputs {
                candidate_id,
                field: named,
                expected,
                found,
            } => {
                assert_eq!(named, field, "the named error identifies the constituent");
                assert_eq!(candidate_id, "cand");
                assert!(!expected.is_empty(), "the expected value is the shared input");
                assert!(!found.is_empty());
            }
            other => panic!("expected IncomparableInputs for {field}, got {other:?}"),
        }
    }
}

fn candidate_with(mutate: impl FnOnce(&mut CandidateModelingInputs)) -> CandidateComparisonEntry {
    let mut entry = candidate("cand", "socp");
    mutate(&mut entry.modeling);
    entry
}

#[test]
fn the_display_labelling_path_is_separate_from_the_rejection_gate() {
    // The label is a rendering helper for display contexts; it is NOT an
    // acceptance path (the gate above rejects; the label only describes).
    let label = incomparability_label("envelope_version", "env-v1", "env-v2");
    assert!(label.contains("INCOMPARABLE"));
    assert!(label.contains("env-v1") && label.contains("env-v2"));
}

#[test]
fn a_comparison_is_over_produced_schedules_with_all_metrics_and_two_candidates() {
    // A non-solving status is not a produced schedule.
    let mut failed = candidate("c2", "socp");
    failed.status = "infeasible".to_string();
    assert_eq!(
        compare_candidate_schedules(
            tenant(),
            portfolio(),
            site(),
            comparison_id(),
            shared_inputs(),
            vec![candidate("c1", "lp"), failed],
        ),
        Err(ComparisonError::NotAScheduleCandidate {
            candidate_id: "c2".to_string(),
            status: "infeasible".to_string(),
        })
    );

    // A blank objective-units field is a missing metric.
    let mut no_units = candidate("c2", "socp");
    no_units.objective_units = "  ".to_string();
    assert_eq!(
        compare_candidate_schedules(
            tenant(),
            portfolio(),
            site(),
            comparison_id(),
            shared_inputs(),
            vec![candidate("c1", "lp"), no_units],
        ),
        Err(ComparisonError::MissingMetric {
            candidate_id: "c2".to_string(),
            metric: "objective_units",
        })
    );

    // Fewer than two candidates is not a comparison.
    assert_eq!(
        compare_candidate_schedules(
            tenant(),
            portfolio(),
            site(),
            comparison_id(),
            shared_inputs(),
            vec![candidate("c1", "lp")],
        ),
        Err(ComparisonError::TooFewCandidates { found: 1 })
    );

    // A non-finite deviation metric is rejected by name.
    let mut bad_deviation = candidate("c2", "socp");
    bad_deviation.schedule_deviation_mw = f64::NAN;
    assert_eq!(
        compare_candidate_schedules(
            tenant(),
            portfolio(),
            site(),
            comparison_id(),
            shared_inputs(),
            vec![candidate("c1", "lp"), bad_deviation],
        ),
        Err(ComparisonError::MissingMetric {
            candidate_id: "c2".to_string(),
            metric: "schedule_deviation_mw",
        })
    );
}

// --- The first comparison case: the P3.18 Tier 1 vs Tier 2 cross-check -----

/// The P3.18 cross-check record on the shared inverter fixture (the recorded
/// 1 MW linearisation gap), constructed as plain data — the exact shape the
/// `conic_fixture::build_comparison_record` produces for the hand-computed
/// optima (Tier 1: p = 5, Tier 2: p = 4).
fn tier1_tier2_cross_check() -> flexcore_optimization_api::conic_fixture::Tier1Tier2ComparisonRecord {
    flexcore_optimization_api::conic_fixture::Tier1Tier2ComparisonRecord {
        candidate_id: Some(FlexUuid::parse("00000000-0000-0000-0000-0000001c0e1c").expect("parses")),
        tier1: TierComparisonSide {
            problem_class: ProblemClass::LinearProgram,
            status: SolveStatus::Optimal,
            objective_value: Some(5.0),
            objective_units: Some("synthetic_cost_unit".to_string()),
            binding_constraints: vec!["inverter_p_linearised_limit".to_string()],
        },
        tier2: TierComparisonSide {
            problem_class: ProblemClass::ContinuousSecondOrderConeProgram,
            status: SolveStatus::Optimal,
            objective_value: Some(4.0),
            objective_units: Some("synthetic_cost_unit".to_string()),
            binding_constraints: vec!["inverter_apparent_power_cone".to_string()],
        },
        objective_difference: Some(1.0),
        notes: vec![
            "P3.18 cross-check: the Tier 1 linearised approximation and the Tier 2 \
             conic formulation describe the SAME operating scenario"
                .to_string(),
        ],
    }
}

fn tier_case_input(
    cross_check: &flexcore_optimization_api::conic_fixture::Tier1Tier2ComparisonRecord,
) -> Tier1Tier2ComparisonCaseInput<'_> {
    Tier1Tier2ComparisonCaseInput {
        cross_check,
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        site_id: site(),
        comparison_id: comparison_id(),
        shared: shared_inputs(),
        tier1_candidate_id: "0192a5e0-0000-7000-8000-000000000101".to_string(),
        tier2_candidate_id: "0192a5e0-0000-7000-8000-000000000102".to_string(),
        tier1_result_input_snapshot_reference: "synthetic-snapshot-p3.18-0001".to_string(),
        tier2_result_input_snapshot_reference: "synthetic-snapshot-p3.18-0001".to_string(),
        tier1_solver_name: "highs".to_string(),
        tier1_solver_version: "1.15.1".to_string(),
        tier1_provider_id: "tier1-python-worker".to_string(),
        tier1_provider_version: "0.1.0".to_string(),
        tier2_solver_name: "clarabel".to_string(),
        tier2_solver_version: "0.11.1".to_string(),
        tier2_provider_id: "clarabel-tier2".to_string(),
        tier2_provider_version: "0.1.0".to_string(),
        // The fixture has a single interval and no commitment data: the
        // deviation basis and interval count are SUPPLIED and recorded.
        tier1_schedule_deviation_mw: 0.0,
        tier2_schedule_deviation_mw: 0.0,
        tier1_feasible_interval_count: 1,
        tier2_feasible_interval_count: 1,
        tier1_runtime: Duration::from_millis(3),
        tier2_runtime: Duration::from_millis(5),
    }
}

#[test]
fn the_first_comparison_case_builds_from_the_p3_18_tier_cross_check_with_the_gap_recorded() {
    let cross_check = tier1_tier2_cross_check();
    let record = tier1_tier2_comparison_case(tier_case_input(&cross_check))
        .expect("the first comparison case assembles");

    assert_eq!(record.candidate_ids(), vec![
        "0192a5e0-0000-7000-8000-000000000101",
        "0192a5e0-0000-7000-8000-000000000102",
    ]);
    assert_eq!(record.candidates()[0].problem_class, "lp");
    assert_eq!(record.candidates()[1].problem_class, "socp");

    // The P3.18 notes travel into the comparison record (never dropped) and
    // the 1 MW linearisation gap is recorded WITH its interpretation.
    assert!(record
        .notes()
        .iter()
        .any(|n| n.contains("P3.18 cross-check")));
    let gap_note = record
        .notes()
        .iter()
        .find(|n| n.contains("linearisation gap"))
        .expect("the gap evidence is recorded");
    assert!(gap_note.contains("= 1"));
    assert!(gap_note.contains("overestimates"));

    // All four metrics per side.
    for entry in record.candidates() {
        assert!(entry.objective_value.is_finite());
        assert_eq!(entry.objective_units, "synthetic_cost_unit");
        assert_eq!(entry.feasible_interval_count, 1);
        assert_eq!(entry.schedule_deviation_mw, 0.0);
    }

    // The metric values themselves: the recorded gap is 5.0 - 4.0.
    assert_eq!(
        record.candidates()[0].objective_value - record.candidates()[1].objective_value,
        1.0
    );
}

#[test]
fn the_first_comparison_case_rejects_a_non_solving_side() {
    let mut cross_check = tier1_tier2_cross_check();
    cross_check.tier2.status = SolveStatus::Infeasible;
    cross_check.tier2.objective_value = None;
    let error = tier1_tier2_comparison_case(tier_case_input(&cross_check))
        .expect_err("a non-solving side is not a produced schedule");
    assert!(matches!(
        error,
        ComparisonError::NotAScheduleCandidate { .. }
    ));
}

#[test]
fn the_recorded_statements_name_the_metrics_the_policy_and_the_p3_28_wiring() {
    for metric in [
        "objective value",
        "schedule deviation",
        "feasible interval count",
        "runtime",
    ] {
        assert!(
            COMPARISON_METRICS_STATEMENT.contains(metric),
            "the metrics statement names '{metric}'"
        );
    }
    assert!(CROSS_INPUT_POLICY.contains("REJECTED"));
    assert!(CROSS_INPUT_POLICY.contains("IncomparableInputs"));
    // The REST wiring is P3.28's — recorded, not built. No UI in Phase 3.
    assert!(COMPARISON_API_WIRING_NOTE.contains("/api/v1/schedules"));
    assert!(COMPARISON_API_WIRING_NOTE.contains("/api/v1/optimization-runs"));
    assert!(COMPARISON_API_WIRING_NOTE.contains("P3.28"));
    assert!(COMPARISON_API_WIRING_NOTE.contains("No UI"));
}
