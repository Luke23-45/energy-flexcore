// P5.18 — The P4.37 closed loop, wired at the SCENARIO level (report
//         §12.5 step 8, §24 step 10; ledger P5.18; the P4.37 open item
//         "closed-loop verification is Phase 5").
//
// The feasibility gate inside a scenario run (`scenario_run.rs`) captures
// the pipeline outcome (steps 1–4) and the step-5 limit report. THIS
// module routes that SAME evidence through the frozen D-P4.07-1 routing
// (`flexcore_grid_api::verdict::route_pipeline_outcome` — the REAL grid
// crate's pure function, not a reimplementation) and feeds the result to
// the P4.37 consumption contract
// (`flexcore_application::feasibility_return::plan_consumption` — the
// frozen contract, imported, not mirrored). The decision is RECORDED on
// the run record (`ScenarioRunRecord.consumption_decisions`).
//
// ## What is wired and what is NOT (the honest boundary)
//
// - **Wired here (the scenario level):** gate outcome → frozen routing →
//   `ReturnedCandidate` conversion → `plan_consumption` → the recorded
//   consumption decision. A RETURN outcome re-enqueues the candidate into
//   re-optimization WITH every violated constraint as a named input; a
//   CONSTRAIN outcome carries the proposal evidence; an unconsumable
//   candidate is REJECTED with the recorded reason (D-P4.37-2); a
//   pipeline-level verdict without candidate identity is recorded
//   `not_planned` with the contract's structural error (never silently
//   dropped).
// - **NOT modified:** the re-scheduling CONSUMER stays the existing P3.24
//   rolling-horizon trigger path. The frozen trigger set
//   (`SCHEDULE_ROLL_TRIGGER_STRINGS` — initial_solve, horizon_advance,
//   envelope_invalidation, asset_opt_out_dropout, forecast_update,
//   operator_request) has NO feasibility-return trigger, and this module
//   does NOT add one: the `ReOptimizationInput` produced here is the
//   CONTRACT the next roll consumes, and the trigger gap TRAVELS with it
//   (the P4.37 `OPEN_ITEM_STATEMENT`, restated in every summary). Wiring
//   it as a new stored trigger string is a recorded decision owned
//   jointly with Phase 3 (flexcore-storage vocabulary is frozen).
// - **Where RETURN/CONSTRAIN actually arises:** the scripted grid backend
//   produces a converged fixture that never violates a limit, so a
//   scripted-backend scenario run routes `within_limits` — the contract
//   chain still executes and is recorded, but a RETURN/CONSTRAIN outcome
//   only arises on a leg whose power flow actually violates a limit (the
//   worker_process backend, or a stressed library case). The library's
//   stress records (P5.20) PRE-REGISTER those expectations; their
//   execution is UNRUN (the human build/test gate).
//
// Control status: composition-root conversion over the frozen contract;
// no solver call, no I/O, no dispatch path; a consumed candidate is a
// CANDIDATE again — never feasible, approved, or dispatchable without the
// full §14.1 gate chain.

use flexcore_application::feasibility_return::{
    plan_consumption, ConsumptionAction, ConstrainEvidence, FeasibilityOutcome,
    NoConsumptionRoute, OPEN_ITEM_STATEMENT, ReturnedCandidate,
};
use flexcore_grid_api::verdict::{route_pipeline_outcome, GateVerdict, ROUTING_DECISION_REF};
use serde::Serialize;

use crate::scenario_run::{ClosedLoopObservation, ConsumptionDecisionSummary};

/// The closed-loop wiring statement, recorded on every summary.
pub const SCENARIO_CLOSED_LOOP_STATEMENT: &str = "The P4.37 closed loop is \
     wired at the SCENARIO level (P5.18): the slice's captured pipeline \
     outcome and step-5 limit report are routed through the frozen \
     D-P4.07-1 rule (flexcore_grid_api::verdict::route_pipeline_outcome) \
     and the result feeds the P4.37 consumption contract \
     (plan_consumption); the decision is recorded on the run record. No \
     grid-api or application code was modified.";

/// The consumer-boundary statement, recorded on every summary.
pub const SCENARIO_CONSUMER_BOUNDARY_STATEMENT: &str = "The re-scheduling \
     consumer stays the EXISTING P3.24 rolling-horizon trigger path; the \
     frozen trigger set is NOT modified and has NO feasibility-return \
     trigger — the ReOptimizationInput produced here is the CONTRACT the \
     next roll consumes, and the trigger gap travels with it (the P4.37 \
     open item, owned jointly with Phase 3).";

/// The summary action for a verdict the contract could not structurally
/// consume (e.g. a pipeline-level verdict without candidate identity).
pub const ACTION_NOT_PLANNED: &str = "not_planned";

// ---------------------------------------------------------------------------
// The frozen-routing step (grid-api's pure function, imported)
// ---------------------------------------------------------------------------

/// Routes the captured observation through the frozen D-P4.07-1 rule.
/// Pure: the same evidence yields the same verdict.
pub fn route_observation(observation: &ClosedLoopObservation) -> GateVerdict {
    route_pipeline_outcome(
        &observation.pipeline_outcome,
        observation.limit_report.as_ref(),
        None,
        observation.now_millis,
    )
}

// ---------------------------------------------------------------------------
// The composition-root conversion (GateVerdict → ReturnedCandidate)
// ---------------------------------------------------------------------------

/// Converts the frozen gate verdict into the P4.37 contract's
/// `ReturnedCandidate` (the composition-root conversion the contract
/// names; the verdict types live in flexcore-grid-api, which is NOT an
/// application-layer dependency edge — the sim-runner is the composition
/// root and may hold both ends). Every field is the frozen evidence.
pub fn returned_candidate_from_verdict(verdict: &GateVerdict) -> ReturnedCandidate {
    let outcome = FeasibilityOutcome::from_outcome_code(&verdict.outcome_code)
        .unwrap_or(FeasibilityOutcome::Inconclusive);
    // The candidate identity is the attempt's CandidateRefs (the §12.5
    // step-1 inputs the routing echoes); a pipeline-level verdict that
    // routed before an attempt exists carries EMPTY refs — the contract's
    // structural validation then refuses the conversion, and the summary
    // records `not_planned` (never silently dropped).
    ReturnedCandidate {
        candidate_id: verdict.candidate_refs.candidate_id.clone(),
        schedule_id: verdict.candidate_refs.schedule_id.clone(),
        revision: verdict.candidate_refs.revision,
        envelope_version: verdict.candidate_refs.envelope_version.clone(),
        result_record_snapshot_ref: verdict
            .candidate_refs
            .result_record_snapshot_ref
            .clone(),
        outcome,
        outcome_reason: verdict.reason.clone(),
        check_id: verdict.completed_event.check_id.clone(),
        violated_constraints: verdict.violated_constraints.clone(),
        constrain_proposal: verdict.constrain_proposal.as_ref().map(|proposal| {
            ConstrainEvidence {
                violated_constraint: proposal.violated_constraint.clone(),
                rule: proposal.rule.clone(),
                scale_factor: proposal.scale_factor,
                statement: proposal.statement.clone(),
            }
        }),
        model_id: verdict.completed_event.model_id.clone(),
        model_version: verdict.completed_event.model_version.clone(),
        model_content_hash: verdict.completed_event.model_content_hash.clone(),
    }
}

// ---------------------------------------------------------------------------
// The consumption planning + the run-record summary
// ---------------------------------------------------------------------------

/// Plans and records the consumption decision for one closed-loop
/// observation (the P5.18 step: a RETURN/CONSTRAIN outcome feeds
/// `plan_consumption` and the decision is recorded). Never panics, never
/// silently drops: a contract-structural error is recorded as
/// [`ACTION_NOT_PLANNED`] with the error text.
pub fn closed_loop_summary(
    observation: &ClosedLoopObservation,
    command_id: &str,
) -> ConsumptionDecisionSummary {
    let verdict = route_observation(observation);
    let candidate = returned_candidate_from_verdict(&verdict);
    let planning = plan_consumption(&candidate);

    let (action, route, reason, problem_inputs) = match &planning {
        Ok(ConsumptionAction::NoConsumption { outcome, route }) => (
            "no_consumption".to_string(),
            Some(route_name(*route).to_string()),
            format!(
                "the outcome '{}' has no scheduling-side consumption; it routes per the \
                 frozen rule D-P4.07-1",
                outcome.outcome_code()
            ),
            Vec::new(),
        ),
        Ok(ConsumptionAction::ReOptimize(input)) => (
            "re_optimize".to_string(),
            None,
            "RETURN: the candidate is re-enqueued into rolling-horizon re-optimization \
             with every violated constraint as a named input; the gate never optimizes \
             and the re-dispatch decision belongs to the scheduler (D-P4.37-1)"
                .to_string(),
            input
                .violated_constraints
                .iter()
                .map(|annotation| {
                    format!(
                        "{}: {}",
                        annotation.constraint_name, annotation.problem_input
                    )
                })
                .collect(),
        ),
        Ok(ConsumptionAction::ConstrainRerun { rerun, proposal }) => (
            "constrain_rerun".to_string(),
            None,
            format!(
                "CONSTRAIN: the violated constraint '{}' and the constraining proposal \
                 (rule '{}', scale factor {}) are attached as problem inputs of the \
                 constrained candidate, which MUST re-enter the feasibility gate from \
                 step 1; the proposal is EVIDENCE, not an applied mutation (D-P4.07-2)",
                proposal.violated_constraint, proposal.rule, proposal.scale_factor
            ),
            rerun
                .violated_constraints
                .iter()
                .map(|annotation| {
                    format!(
                        "{}: {}",
                        annotation.constraint_name, annotation.problem_input
                    )
                })
                .collect(),
        ),
        Ok(ConsumptionAction::RejectUnconsumable { reason }) => (
            "reject_unconsumable".to_string(),
            None,
            reason.clone(),
            Vec::new(),
        ),
        Err(error) => (
            ACTION_NOT_PLANNED.to_string(),
            None,
            format!(
                "the consumption contract refused the converted candidate structurally \
                 ({error}); the decision is recorded as not planned — never silently \
                 dropped, never passed",
            ),
            Vec::new(),
        ),
    };

    ConsumptionDecisionSummary {
        command_id: command_id.to_string(),
        outcome_code: verdict.outcome_code.clone(),
        decision_ref: ROUTING_DECISION_REF.to_string(),
        action,
        route,
        reason,
        violated_constraints: verdict.violated_constraints.clone(),
        problem_inputs,
        check_id: verdict.completed_event.check_id.clone(),
        model_id_version: format!(
            "{}@{}",
            verdict.completed_event.model_id, verdict.completed_event.model_version
        ),
        roll_trigger_note: OPEN_ITEM_STATEMENT.to_string(),
        statements: vec![
            SCENARIO_CLOSED_LOOP_STATEMENT.to_string(),
            SCENARIO_CONSUMER_BOUNDARY_STATEMENT.to_string(),
        ],
    }
}

/// The stable name of a no-consumption route (the summary's route field).
fn route_name(route: NoConsumptionRoute) -> &'static str {
    match route {
        NoConsumptionRoute::CommandLifecycle => "command_lifecycle",
        NoConsumptionRoute::Rejected => "rejected",
        NoConsumptionRoute::OperatorRelease => "operator_release",
        NoConsumptionRoute::RetryNone => "retry_none",
    }
}

/// The summary for a slice that ended BEFORE the feasibility pipeline ran
/// (e.g. the validation gate refused the candidate): the outcome code is
/// honestly `not_routed` — no consumption decision is invented.
pub fn not_routed_summary(command_id: &str) -> ConsumptionDecisionSummary {
    ConsumptionDecisionSummary {
        command_id: command_id.to_string(),
        outcome_code: "not_routed".to_string(),
        decision_ref: ROUTING_DECISION_REF.to_string(),
        action: "no_consumption".to_string(),
        route: None,
        reason: "the slice ended before the feasibility pipeline ran; no consumption \
                 decision exists to plan (none is invented)"
            .to_string(),
        violated_constraints: Vec::new(),
        problem_inputs: Vec::new(),
        check_id: String::new(),
        model_id_version: String::new(),
        roll_trigger_note: OPEN_ITEM_STATEMENT.to_string(),
        statements: vec![
            SCENARIO_CLOSED_LOOP_STATEMENT.to_string(),
            SCENARIO_CONSUMER_BOUNDARY_STATEMENT.to_string(),
        ],
    }
}

// ---------------------------------------------------------------------------
// Unit tests (the pure contract mapping; the scenario-level wiring is
// exercised in tests/scenario_replay_p5_18.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_application::feasibility_return::{
        ConstraintAnnotation, ConstraintCategory, ReOptimizationInput,
    };

    /// A RETURN-shaped returned candidate with one parseable thermal
    /// constraint (the frozen grammar `<category_code>:<element_id>`).
    fn returned_candidate() -> ReturnedCandidate {
        ReturnedCandidate {
            candidate_id: "cand-1".to_string(),
            schedule_id: "sched-1".to_string(),
            revision: 2,
            envelope_version: "env-1".to_string(),
            result_record_snapshot_ref: "snap-1".to_string(),
            outcome: FeasibilityOutcome::ReturnedForReOptimization,
            outcome_reason: "single voltage violation".to_string(),
            check_id: "check-1".to_string(),
            violated_constraints: vec!["bus_voltage_upper:bus_3".to_string()],
            constrain_proposal: None,
            model_id: "feeder-a".to_string(),
            model_version: "1".to_string(),
            model_content_hash: "hash-1".to_string(),
        }
    }

    #[test]
    fn a_return_outcome_plans_re_optimization_with_named_inputs() {
        let action = plan_consumption(&returned_candidate()).expect("a valid candidate");
        match action {
            ConsumptionAction::ReOptimize(input) => {
                assert_eq!(input.candidate_id, "cand-1");
                assert_eq!(input.outcome_code, "returned_for_re_optimization");
                assert_eq!(input.violated_constraints.len(), 1);
                let annotation: &ConstraintAnnotation = &input.violated_constraints[0];
                assert_eq!(
                    annotation.category,
                    ConstraintCategory::BusVoltageUpper
                );
                assert!(annotation.problem_input.contains("bus_3"));
                // The trigger gap travels with the input (the open item).
                assert!(input.roll_trigger_note.contains("roll-trigger"));
            }
            other => panic!("expected ReOptimize, got {other:?}"),
        }
    }

    #[test]
    fn an_unconsumable_constraint_rejects_the_whole_candidate_with_reason() {
        let mut candidate = returned_candidate();
        candidate.violated_constraints = vec!["not_a_frozen_name".to_string()];
        match plan_consumption(&candidate).expect("semantic failure is not an Err") {
            ConsumptionAction::RejectUnconsumable { reason } => {
                assert!(reason.contains("not_a_frozen_name"));
                assert!(reason.contains("never silently dropped"));
            }
            other => panic!("expected RejectUnconsumable, got {other:?}"),
        }
    }

    #[test]
    fn a_structurally_broken_candidate_is_an_err_not_a_silent_drop() {
        let mut candidate = returned_candidate();
        candidate.candidate_id = String::new();
        assert!(plan_consumption(&candidate).is_err());
    }

    #[test]
    fn the_summary_records_the_frozen_route_for_a_within_limits_outcome() {
        let mut candidate = returned_candidate();
        candidate.outcome = FeasibilityOutcome::WithinLimits;
        candidate.violated_constraints = Vec::new();
        match plan_consumption(&candidate).expect("valid") {
            ConsumptionAction::NoConsumption { outcome, route } => {
                assert_eq!(outcome.outcome_code(), "within_limits");
                assert!(matches!(route, NoConsumptionRoute::CommandLifecycle));
            }
            other => panic!("expected NoConsumption, got {other:?}"),
        }
    }

    #[test]
    fn the_re_optimization_input_is_the_contract_the_next_roll_consumes() {
        // The input the contract produces is a CANDIDATE identity plus
        // named constraint inputs — never a schedule, never a dispatch.
        let action = plan_consumption(&returned_candidate()).expect("valid");
        if let ConsumptionAction::ReOptimize(input) = action {
            let ReOptimizationInput {
                candidate_id,
                schedule_id,
                revision,
                model_content_hash,
                ..
            } = input;
            assert_eq!(candidate_id, "cand-1");
            assert_eq!(schedule_id, "sched-1");
            assert_eq!(revision, 2);
            assert_eq!(model_content_hash, "hash-1");
        } else {
            panic!("expected ReOptimize");
        }
    }
}
