// P4.37 — the consumption-contract test suite (report §12.5 step 8; ledger
// P4.37). Decision record: this module's contract is D-P4.37-1/D-P4.37-2
// (see `crates/flexcore-application/src/feasibility_return.rs` header).
//
// WHAT IS PROVEN HERE (the Phase 4 side, ledger P4.37 step 2):
// - a returned candidate re-enters scheduling with its violated
//   constraints ATTACHED (stub-consumer shape: this suite IS the
//   consumer-side proof; the real consumer is Phase 3's P3.24, which is
//   NOT modified here);
// - CONSTRAIN and RETURN keep distinct statuses and distinct evidence
//   (the ledger risk note: "RETURN treated as REJECTED everywhere");
// - the unconsumable case REJECTS with a recorded reason — never
//   silently dropped, never passed (D-P4.37-2);
// - REJECT/HELD/INCONCLUSIVE/WITHIN-LIMITS route per the frozen rule
//   D-P4.07-1 with NO scheduling-side consumption;
// - the mirrored outcome codes and the constraint-name grammar match the
//   frozen grid-gate vocabulary (drift fails visibly).
//
// WHAT IS NOT PROVEN HERE (recorded, ledger P4.37 step 4): the full
// closed loop (feasibility return → re-optimized schedule → re-validation)
// is the P5.18–P5.19 scenario test. Nothing in this suite runs a solve,
// touches a device, or claims a feasibility outcome — simulation-only
// 0.x line.

use flexcore_application::feasibility_return::{
    parse_constraint_name, plan_consumption, validate_returned, ConstrainEvidence,
    ConstraintCategory, ConsumptionAction, FeasibilityOutcome, FeasibilityReturnError,
    NoConsumptionRoute, ReOptimizationInput, ReturnedCandidate,
    CONSUMPTION_CONTRACT_STATEMENT, CONSTRAINT_CATEGORY_CODES, FAILURE_MODE_STATEMENT,
    GATE_OUTCOME_CODES, MIRROR_STATEMENT, OPEN_ITEM_STATEMENT,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A well-formed returned candidate with the given outcome and constraint
/// names (the composition-root conversion of a grid `GateVerdict`).
fn returned_candidate(outcome: FeasibilityOutcome, constraints: &[&str]) -> ReturnedCandidate {
    ReturnedCandidate {
        candidate_id: "cand-1".to_string(),
        schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
        revision: 2,
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
        outcome,
        outcome_reason: "the gate verdict reason (audit text)".to_string(),
        check_id: "attempt-1".to_string(),
        violated_constraints: constraints.iter().map(|s| s.to_string()).collect(),
        constrain_proposal: if outcome == FeasibilityOutcome::Constrained {
            Some(ConstrainEvidence {
                violated_constraint: constraints[0].to_string(),
                rule: "uniform proportional scaling of every candidate injection by \
                       limit/measured (D-P4.07-2)"
                    .to_string(),
                scale_factor: 0.8,
                statement: "EVIDENCE, not an applied mutation; the constrained \
                            candidate re-enters the gate"
                    .to_string(),
            })
        } else {
            None
        },
        model_id: "feeder-a".to_string(),
        model_version: "1".to_string(),
        model_content_hash: "hash-feeder-a-v1".to_string(),
    }
}

/// Extracts the `ReOptimizationInput` from a consuming action (the stub
/// consumer shape: whatever the action, scheduling receives exactly this).
fn rerun_input(action: &ConsumptionAction) -> Option<&ReOptimizationInput> {
    match action {
        ConsumptionAction::ConstrainRerun { rerun, .. } => Some(rerun),
        ConsumptionAction::ReOptimize(input) => Some(input),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The mirror pins (drift between the crates fails visibly)
// ---------------------------------------------------------------------------

#[test]
fn the_mirrored_outcome_codes_are_exactly_the_frozen_vocabulary() {
    assert_eq!(
        GATE_OUTCOME_CODES,
        [
            "within_limits",
            "rejected",
            "constrained",
            "returned_for_re_optimization",
            "inconclusive",
            "held",
        ]
    );
    for code in GATE_OUTCOME_CODES {
        let outcome = FeasibilityOutcome::from_outcome_code(code)
            .unwrap_or_else(|| panic!("frozen code '{code}' must parse"));
        assert_eq!(outcome.outcome_code(), code);
    }
}

#[test]
fn the_mirrored_category_codes_are_exactly_the_frozen_grammar() {
    assert_eq!(
        CONSTRAINT_CATEGORY_CODES,
        [
            "bus_voltage_lower",
            "bus_voltage_upper",
            "branch_thermal",
            "transformer_loading",
            "generator_apparent_power",
        ]
    );
    for code in CONSTRAINT_CATEGORY_CODES {
        let category = ConstraintCategory::from_code(code)
            .unwrap_or_else(|| panic!("frozen code '{code}' must parse"));
        assert_eq!(category.code(), code);
    }
}

#[test]
fn the_recorded_statements_are_non_empty_and_name_their_decisions() {
    for (statement, needle) in [
        (CONSUMPTION_CONTRACT_STATEMENT, "D-P4.37-1"),
        (FAILURE_MODE_STATEMENT, "D-P4.37-2"),
        (OPEN_ITEM_STATEMENT, "P5.18–P5.19"),
        (MIRROR_STATEMENT, "dependency rule"),
    ] {
        assert!(!statement.trim().is_empty());
        assert!(statement.contains(needle), "'{needle}' missing: {statement}");
    }
}

// ---------------------------------------------------------------------------
// CONSTRAIN — attach the violated constraint as a problem input; the
// constrained candidate re-enters the gate
// ---------------------------------------------------------------------------

#[test]
fn constrain_outcome_attaches_the_violated_constraint_as_a_problem_input() {
    let candidate = returned_candidate(
        FeasibilityOutcome::Constrained,
        &["branch_thermal:line_2_3"],
    );
    let action = plan_consumption(&candidate).expect("well-formed candidate plans");

    let ConsumptionAction::ConstrainRerun { rerun, proposal } = &action else {
        panic!("a CONSTRAIN outcome must plan a ConstrainRerun, got {action:?}");
    };
    // Exactly the one violated constraint, annotated, as the re-run input.
    assert_eq!(rerun.violated_constraints.len(), 1);
    let annotation = &rerun.violated_constraints[0];
    assert_eq!(annotation.constraint_name, "branch_thermal:line_2_3");
    assert_eq!(annotation.category, ConstraintCategory::BranchThermal);
    assert_eq!(annotation.element_id, "line_2_3");
    assert!(annotation.problem_input.contains("line_2_3"));
    // The proposal evidence travels (not an applied mutation) and names
    // the violated constraint.
    assert_eq!(proposal.violated_constraint, "branch_thermal:line_2_3");
    assert!((proposal.scale_factor - 0.8).abs() < 1.0e-12);
    // The re-run is bound to the SAME candidate identity and THE MODEL
    // HASH (evidence discipline).
    assert_eq!(rerun.candidate_id, "cand-1");
    assert_eq!(rerun.schedule_id, "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b");
    assert_eq!(rerun.revision, 2);
    assert_eq!(rerun.model_content_hash, "hash-feeder-a-v1");
    assert_eq!(rerun.check_id, "attempt-1");
    assert_eq!(rerun.outcome_code, "constrained");
    // The action is NOT a rejection and NOT a pass.
    assert!(!matches!(action, ConsumptionAction::RejectUnconsumable { .. }));
    assert!(!matches!(action, ConsumptionAction::NoConsumption { .. }));
}

#[test]
fn the_constrained_rerun_carries_the_re_entry_obligation_note() {
    let candidate = returned_candidate(
        FeasibilityOutcome::Constrained,
        &["transformer_loading:trafo_1_2"],
    );
    let action = plan_consumption(&candidate).expect("plans");
    let input = rerun_input(&action).expect("consuming action");
    // The trigger note travels with the input (the open item is recorded,
    // not hidden).
    assert!(input.roll_trigger_note.contains("P5.18–P5.19"));
    assert!(input.roll_trigger_note.contains("roll-trigger set"));
}

// ---------------------------------------------------------------------------
// RETURN — re-enqueue into rolling-horizon re-optimization with EVERY
// violated constraint as a named input
// ---------------------------------------------------------------------------

#[test]
fn return_outcome_reenqueues_with_every_violated_constraint_annotated() {
    let candidate = returned_candidate(
        FeasibilityOutcome::ReturnedForReOptimization,
        &[
            "bus_voltage_lower:bus_3",
            "bus_voltage_upper:bus_5",
            "generator_apparent_power:bus_5",
        ],
    );
    let action = plan_consumption(&candidate).expect("well-formed candidate plans");

    let ConsumptionAction::ReOptimize(input) = &action else {
        panic!("a RETURN outcome must plan a ReOptimize, got {action:?}");
    };
    assert_eq!(input.violated_constraints.len(), 3);
    assert_eq!(input.violated_constraints[0].constraint_name, "bus_voltage_lower:bus_3");
    assert_eq!(input.violated_constraints[1].constraint_name, "bus_voltage_upper:bus_5");
    assert_eq!(
        input.violated_constraints[2].constraint_name,
        "generator_apparent_power:bus_5"
    );
    // Every annotation carries its element and a problem-input mapping.
    for annotation in &input.violated_constraints {
        assert!(!annotation.element_id.trim().is_empty());
        assert!(!annotation.problem_input.trim().is_empty());
    }
    assert_eq!(input.outcome_code, "returned_for_re_optimization");
    assert_eq!(input.model_content_hash, "hash-feeder-a-v1");
    assert!(!matches!(action, ConsumptionAction::RejectUnconsumable { .. }));
}

#[test]
fn return_and_constrain_keep_distinct_statuses_and_distinct_evidence() {
    let constrain = returned_candidate(
        FeasibilityOutcome::Constrained,
        &["branch_thermal:line_2_3"],
    );
    let returned = returned_candidate(
        FeasibilityOutcome::ReturnedForReOptimization,
        &["branch_thermal:line_2_3"],
    );
    let constrain_action = plan_consumption(&constrain).expect("plans");
    let return_action = plan_consumption(&returned).expect("plans");
    // Distinct statuses: the outcome codes differ.
    let constrain_input = rerun_input(&constrain_action).expect("consuming");
    let return_input = rerun_input(&return_action).expect("consuming");
    assert_ne!(constrain_input.outcome_code, return_input.outcome_code);
    // Distinct evidence: only the CONSTRAIN action carries the proposal.
    match (&constrain_action, &return_action) {
        (
            ConsumptionAction::ConstrainRerun { proposal, .. },
            ConsumptionAction::ReOptimize(_),
        ) => {
            assert!(proposal.scale_factor < 1.0);
        }
        _ => panic!("expected ConstrainRerun and ReOptimize"),
    }
    // Neither is a rejection: the re-optimization path is NOT degraded to
    // a rejection (the ledger risk note).
    assert!(!matches!(constrain_action, ConsumptionAction::RejectUnconsumable { .. }));
    assert!(!matches!(return_action, ConsumptionAction::RejectUnconsumable { .. }));
}

#[test]
fn the_voltage_and_capability_annotations_name_the_redispatch_decision() {
    // The D-P4.07-1 rationale restated per category: a voltage fix and the
    // S_max capability model belong to the scheduler.
    for (name, needle) in [
        ("bus_voltage_upper:bus_3", "re-dispatch"),
        ("bus_voltage_lower:bus_3", "re-dispatch"),
        ("generator_apparent_power:bus_5", "capability"),
        ("branch_thermal:line_2_3", "bounded inputs"),
        ("transformer_loading:trafo_1_2", "bounded inputs"),
    ] {
        let annotation =
            parse_constraint_name(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            annotation.problem_input.contains(needle),
            "'{needle}' missing for {name}: {}",
            annotation.problem_input
        );
    }
}

// ---------------------------------------------------------------------------
// The failure mode (D-P4.37-2): unconsumable → REJECTED with the reason
// recorded — never silently dropped, never passed
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_constraint_category_rejects_with_a_recorded_reason() {
    let candidate = returned_candidate(
        FeasibilityOutcome::ReturnedForReOptimization,
        &["bus_voltage_lower:bus_3", "socp_slack:bus_9"],
    );
    let action = plan_consumption(&candidate).expect("a semantic failure is an action, not an Err");
    let ConsumptionAction::RejectUnconsumable { reason } = &action else {
        panic!("an unknown category must reject unconsumable, got {action:?}");
    };
    assert!(reason.contains("socp_slack"), "the reason names the offense: {reason}");
    assert!(reason.contains("D-P4.37-2"));
    // The whole candidate is rejected — a partially-consumed candidate
    // would be a silent partial drop.
    assert!(rerun_input(&action).is_none());
}

#[test]
fn a_malformed_constraint_name_rejects_with_a_recorded_reason() {
    for bad in ["no-separator-at-all", "bus_voltage_lower:", ":bus_3", ""] {
        let candidate = returned_candidate(
            FeasibilityOutcome::ReturnedForReOptimization,
            &[bad],
        );
        let action = plan_consumption(&candidate).expect("a semantic failure is an action");
        let ConsumptionAction::RejectUnconsumable { reason } = &action else {
            panic!("'{bad}' must reject unconsumable, got {action:?}");
        };
        assert!(!reason.trim().is_empty());
        assert!(rerun_input(&action).is_none());
    }
}

#[test]
fn a_structurally_incomplete_returned_candidate_is_a_caller_error_not_a_rejection() {
    // A CONSTRAIN outcome without the proposal evidence: the composition
    // root mis-converted the verdict — fail LOUDLY (Err), never a silent
    // rejection.
    let mut candidate = returned_candidate(FeasibilityOutcome::Constrained, &["branch_thermal:line_2_3"]);
    candidate.constrain_proposal = None;
    assert_eq!(
        validate_returned(&candidate),
        Err(FeasibilityReturnError::IncompleteEvidence {
            detail: "a CONSTRAIN outcome must carry the constraining-proposal \
                     evidence (D-P4.07-2)"
                .to_string()
        })
    );
    // A consuming outcome with no named constraint.
    let candidate = returned_candidate(FeasibilityOutcome::ReturnedForReOptimization, &[]);
    assert!(matches!(
        validate_returned(&candidate),
        Err(FeasibilityReturnError::IncompleteEvidence { .. })
    ));
    // Revision zero and empty identification fields.
    let mut candidate = returned_candidate(FeasibilityOutcome::ReturnedForReOptimization, &["bus_voltage_lower:bus_3"]);
    candidate.revision = 0;
    assert_eq!(validate_returned(&candidate), Err(FeasibilityReturnError::RevisionZero));
    let mut candidate = returned_candidate(FeasibilityOutcome::ReturnedForReOptimization, &["bus_voltage_lower:bus_3"]);
    candidate.model_content_hash = "  ".to_string();
    assert_eq!(
        validate_returned(&candidate),
        Err(FeasibilityReturnError::EmptyField { field: "model_content_hash" })
    );
    // plan_consumption propagates the structural error (no silent path).
    let mut candidate = returned_candidate(FeasibilityOutcome::Constrained, &["branch_thermal:line_2_3"]);
    candidate.constrain_proposal = None;
    assert!(plan_consumption(&candidate).is_err());
}

// ---------------------------------------------------------------------------
// The frozen-rule routing: REJECT/HELD/INCONCLUSIVE/WITHIN-LIMITS have NO
// scheduling-side consumption
// ---------------------------------------------------------------------------

#[test]
fn non_consuming_outcomes_route_per_the_frozen_rule_d_p4_07_1() {
    let cases = [
        (
            FeasibilityOutcome::WithinLimits,
            NoConsumptionRoute::CommandLifecycle,
        ),
        (FeasibilityOutcome::Rejected, NoConsumptionRoute::Rejected),
        (FeasibilityOutcome::Held, NoConsumptionRoute::OperatorRelease),
        (
            FeasibilityOutcome::Inconclusive,
            NoConsumptionRoute::RetryNone,
        ),
    ];
    for (outcome, route) in cases {
        // Non-consuming outcomes may carry violation names or not; the
        // contract consumes neither.
        let candidate = returned_candidate(outcome, &["branch_thermal:line_2_3"]);
        let action = plan_consumption(&candidate).expect("plans");
        let ConsumptionAction::NoConsumption {
            outcome: got_outcome,
            route: got_route,
        } = &action
        else {
            panic!("{outcome:?} must not consume, got {action:?}");
        };
        assert_eq!(*got_outcome, outcome);
        assert_eq!(*got_route, route);
        assert!(rerun_input(&action).is_none());
    }
}

#[test]
fn inconclusive_never_implies_a_re_optimization_and_rejected_stays_a_rejection() {
    // The ledger risk note, tested from the consumer side: RETURN must be
    // reachable as re-optimization, INCONCLUSIVE must never be.
    let inconclusive =
        returned_candidate(FeasibilityOutcome::Inconclusive, &[]);
    let returned = returned_candidate(
        FeasibilityOutcome::ReturnedForReOptimization,
        &["bus_voltage_upper:bus_3"],
    );
    assert!(matches!(
        plan_consumption(&inconclusive).expect("plans"),
        ConsumptionAction::NoConsumption { route: NoConsumptionRoute::RetryNone, .. }
    ));
    assert!(matches!(
        plan_consumption(&returned).expect("plans"),
        ConsumptionAction::ReOptimize(_)
    ));
}

// ---------------------------------------------------------------------------
// Contract-level properties
// ---------------------------------------------------------------------------

#[test]
fn consumption_is_deterministic_and_pure() {
    let candidate = returned_candidate(
        FeasibilityOutcome::ReturnedForReOptimization,
        &["bus_voltage_lower:bus_3", "branch_thermal:line_2_3"],
    );
    let first = plan_consumption(&candidate).expect("plans");
    let second = plan_consumption(&candidate).expect("plans");
    assert_eq!(first, second);
}

#[test]
fn every_reoptimization_input_binds_the_model_hash_and_the_check_evidence() {
    // The §12.6 evidence discipline, consumed side: no re-optimization
    // input without the model hash and the check reference.
    for outcome in [FeasibilityOutcome::Constrained, FeasibilityOutcome::ReturnedForReOptimization] {
        let candidate = returned_candidate(outcome, &["bus_voltage_lower:bus_3"]);
        let input = rerun_input(&plan_consumption(&candidate).expect("plans"))
            .expect("consuming action");
        assert!(!input.model_content_hash.trim().is_empty());
        assert!(!input.model_id.trim().is_empty());
        assert!(!input.model_version.trim().is_empty());
        assert!(!input.check_id.trim().is_empty());
        assert!(!input.result_record_snapshot_ref.trim().is_empty());
    }
}

#[test]
fn an_unknown_outcome_code_cannot_enter_the_contract() {
    // The vocabulary is closed at the seam: the composition root parses
    // the frozen code, and an unknown code has no mirror — it cannot be
    // silently mapped to a default outcome.
    assert_eq!(FeasibilityOutcome::from_outcome_code("passed"), None);
    assert_eq!(FeasibilityOutcome::from_outcome_code("return"), None);
    assert_eq!(FeasibilityOutcome::from_outcome_code(""), None);
}
