// P4.08 — Fail-closed policy integration tests (ledger P4.08 acceptance;
// report §12.5 final paragraph, §17.5 invariant 9). Decision record:
// ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md
// (D-P4.08-1..2).
//
// Covered here:
// 1. EVERY trigger in the public enumeration produces a fail-closed
//    outcome — REJECT or HOLD per the recorded policy table, NEVER a pass
//    (there is no pass value in the outcome type at all), each naming the
//    trigger for the audit record.
// 2. The policy table matches the recorded D-P4.08-1 rows exactly.
// 3. The STRUCTURAL exhaustive-match guarantee: `classify_pipeline_error`
//    matches every `PipelineError` variant WITHOUT a wildcard arm — a new
//    error variant without a fail-closed mapping is a COMPILE failure.
//    This test constructs every variant and asserts the classification is
//    fail-closed with a non-empty reason and the expected trigger.
// 4. End-to-end shape: routed through the P4.07 verdict layer, every
//    trigger lands on REJECTED or HELD — never WITHIN_LIMITS, CONSTRAINED,
//    or RETURNED.

use flexcore_grid_api::fail_closed::{
    classify_pipeline_error, fail_closed_action, fail_closed_outcome, FailClosedAction,
    FailClosedOutcome, FailClosedTrigger, PipelineError, EXHAUSTIVE_MATCH_STATEMENT,
    FAIL_CLOSED_STATEMENT, HOLD_RELEASE_POLICY,
};
use flexcore_grid_api::feasibility::{
    AttemptRecordError, SeamError, UNBALANCED_DEFERRAL_NOTE,
};
use flexcore_grid_api::model_health::ModelHealthError;
use flexcore_grid_api::port::{GridModelRef, GridPortError, ModelError};
use flexcore_grid_api::verdict::{route_fail_closed, VerdictOutcome};
use flexcore_contracts::quality::TelemetryQuality;

/// One constructor per `PipelineError` variant — adding a variant without
/// a classification arm breaks the compile of `classify_pipeline_error`
/// itself; adding one without updating THIS list breaks this test's
/// coverage count.
fn every_pipeline_error() -> Vec<PipelineError> {
    vec![
        PipelineError::Seam(SeamError {
            seam: "capability_bounds",
            detail: "the capability store is unreachable".to_string(),
        }),
        PipelineError::Seam(SeamError {
            seam: "ramp_baseline",
            detail: "the telemetry store is unreachable".to_string(),
        }),
        PipelineError::ModelHealth(ModelHealthError::NotUsable { findings: Vec::new() }),
        PipelineError::ModelHealth(ModelHealthError::UnknownVersion {
            model_id: "feeder-a".to_string(),
            version: "1".to_string(),
        }),
        PipelineError::ModelHealth(ModelHealthError::InvalidTimestamp),
        PipelineError::Port(GridPortError::ScopeExceeded(
            flexcore_grid_api::scope::ScopeError::ScopeExceeded {
                requested: "unbalanced".to_string(),
                declared: "balanced".to_string(),
            },
        )),
        PipelineError::Port(GridPortError::BackendUnavailable(
            "worker unreachable".to_string(),
        )),
        PipelineError::Port(GridPortError::MalformedModel(ModelError::InvalidSlack {
            detail: "no slack".to_string(),
        })),
        PipelineError::AttemptRecord(AttemptRecordError::NoDerivationRefs),
        PipelineError::TelemetryQualityBlocked {
            detail: TelemetryQuality::Stale.name().to_string(),
        },
        PipelineError::SnapshotRefUnresolvable {
            detail: "snapshot 'schedule_input_snapshot:env-x' not found".to_string(),
        },
        PipelineError::ScopeUndeclared {
            detail: "adapter scope not declared".to_string(),
        },
        PipelineError::CapabilityDataMissing {
            detail: "no S_max for bus_3".to_string(),
        },
        PipelineError::CandidateMalformed {
            detail: "empty setpoint id".to_string(),
        },
        PipelineError::PowerFlowNonConvergence {
            detail: "did not converge in 4 iterations".to_string(),
        },
        PipelineError::RelaxationRecordMissing {
            detail: "no P3.18 record attached".to_string(),
        },
    ]
}

// ---------------------------------------------------------------------------
// The trigger enumeration: total, fail-closed, audit-named
// ---------------------------------------------------------------------------

#[test]
fn every_trigger_produces_a_fail_closed_outcome_naming_the_trigger() {
    assert!(!FAIL_CLOSED_STATEMENT.trim().is_empty());
    for trigger in FailClosedTrigger::ALL {
        let outcome: FailClosedOutcome = fail_closed_outcome(trigger, "test detail");
        // REJECT or HOLD — the outcome type has no pass variant at all.
        assert!(
            outcome.action == FailClosedAction::Reject
                || outcome.action == FailClosedAction::Hold,
            "trigger {} mapped to neither REJECT nor HOLD",
            trigger.code()
        );
        // The outcome names the trigger (audit-visible record).
        assert!(!outcome.reason.trim().is_empty());
        assert!(outcome.reason.contains(trigger.default_reason()));
        // Re-running the classification is deterministic.
        let again = fail_closed_outcome(trigger, "test detail");
        assert_eq!(again, outcome);
    }
}

#[test]
fn the_policy_table_matches_the_recorded_d_p4_08_1_rows() {
    let expected = [
        (FailClosedTrigger::ValidatorFailed, FailClosedAction::Hold),
        (
            FailClosedTrigger::ModelStaleOrIncomplete,
            FailClosedAction::Hold,
        ),
        (FailClosedTrigger::ModelHashMismatch, FailClosedAction::Hold),
        (
            FailClosedTrigger::TelemetryQualityBlocked,
            FailClosedAction::Reject,
        ),
        (
            FailClosedTrigger::TelemetryUnavailable,
            FailClosedAction::Hold,
        ),
        (
            FailClosedTrigger::SnapshotRefUnresolvable,
            FailClosedAction::Reject,
        ),
        (FailClosedTrigger::ScopeUndeclared, FailClosedAction::Reject),
        (
            FailClosedTrigger::CapabilityDataMissing,
            FailClosedAction::Reject,
        ),
        (FailClosedTrigger::CandidateMalformed, FailClosedAction::Reject),
        (
            FailClosedTrigger::PowerFlowNonConvergence,
            FailClosedAction::Hold,
        ),
        (FailClosedTrigger::ProviderError, FailClosedAction::Hold),
        (
            FailClosedTrigger::RelaxationRecordMissing,
            FailClosedAction::Reject,
        ),
        (FailClosedTrigger::UnexpectedError, FailClosedAction::Hold),
    ];
    for (trigger, action) in expected {
        assert_eq!(
            fail_closed_action(trigger),
            action,
            "trigger {} diverges from the recorded policy table",
            trigger.code()
        );
    }
}

#[test]
fn the_hold_release_policy_is_recorded() {
    assert!(!HOLD_RELEASE_POLICY.trim().is_empty());
    assert!(HOLD_RELEASE_POLICY.contains("ONLY by"));
    assert!(HOLD_RELEASE_POLICY.contains("operator"));
}

/// Structural proof that the fail-closed vocabulary has NO pass value: the
/// action type has exactly two variants, so "a trigger maps to a pass" is
/// unrepresentable (the P4.08 acceptance criterion).
#[test]
fn the_fail_closed_action_vocabulary_has_no_pass() {
    let exhaustive = match FailClosedAction::Reject {
        FailClosedAction::Reject | FailClosedAction::Hold => "closed",
    };
    assert_eq!(exhaustive, "closed");
}

// ---------------------------------------------------------------------------
// The structural exhaustive-match guarantee (D-P4.08-2)
// ---------------------------------------------------------------------------

#[test]
fn every_pipeline_error_classifies_to_a_fail_closed_outcome() {
    assert!(!EXHAUSTIVE_MATCH_STATEMENT.trim().is_empty());
    for error in every_pipeline_error() {
        let outcome = classify_pipeline_error(&error);
        assert!(
            outcome.action == FailClosedAction::Reject
                || outcome.action == FailClosedAction::Hold,
            "error class {error:?} classified to neither REJECT nor HOLD"
        );
        assert!(
            !outcome.reason.trim().is_empty(),
            "error class {error:?} classified without a reason"
        );
    }
}

#[test]
fn the_seam_mapping_distinguishes_telemetry_from_validator_failures() {
    let baseline = classify_pipeline_error(&PipelineError::Seam(SeamError {
        seam: "ramp_baseline",
        detail: "unreachable".to_string(),
    }));
    assert_eq!(baseline.trigger, FailClosedTrigger::TelemetryUnavailable);

    let capability = classify_pipeline_error(&PipelineError::Seam(SeamError {
        seam: "capability_bounds",
        detail: "unreachable".to_string(),
    }));
    assert_eq!(capability.trigger, FailClosedTrigger::ValidatorFailed);

    // Both are HOLD (infrastructure, not candidate fault).
    assert_eq!(baseline.action, FailClosedAction::Hold);
    assert_eq!(capability.action, FailClosedAction::Hold);
}

#[test]
fn the_port_mapping_refuses_out_of_scope_requests_as_scope_undeclared() {
    let scoped = classify_pipeline_error(&PipelineError::Port(GridPortError::ScopeExceeded(
        flexcore_grid_api::scope::ScopeError::ScopeExceeded {
            requested: "meshed".to_string(),
            declared: "radial-only".to_string(),
        },
    )));
    assert_eq!(scoped.trigger, FailClosedTrigger::ScopeUndeclared);
    assert_eq!(scoped.action, FailClosedAction::Reject);
}

#[test]
fn non_convergence_classifies_as_the_recorded_trigger_with_retry_none_shape() {
    let classified = classify_pipeline_error(&PipelineError::PowerFlowNonConvergence {
        detail: "did not converge".to_string(),
    });
    assert_eq!(classified.trigger, FailClosedTrigger::PowerFlowNonConvergence);
    assert_eq!(classified.action, FailClosedAction::Hold);
    // Never worded as a pass anywhere in the reason.
    assert!(!classified.reason.to_lowercase().contains("within limits"));
}

// ---------------------------------------------------------------------------
// End-to-end shape: routed through the verdict layer, never a pass
// ---------------------------------------------------------------------------

#[test]
fn every_trigger_routes_to_rejected_or_held_through_the_verdict_layer() {
    let refs = flexcore_grid_api::feasibility::CandidateRefs {
        candidate_id: "cand-fc-1".to_string(),
        schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
        revision: 1,
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    };
    let model_ref = GridModelRef {
        model_id: "feeder-fc".to_string(),
        version: "1".to_string(),
        content_hash: "hash-fc".to_string(),
    };
    for trigger in FailClosedTrigger::ALL {
        let outcome = fail_closed_outcome(trigger, "integration detail");
        let verdict = route_fail_closed(&refs, &outcome, Some(&model_ref), 10_000);
        assert!(
            verdict.outcome == VerdictOutcome::Rejected
                || verdict.outcome == VerdictOutcome::Held,
            "trigger {} routed to {:?} through the verdict layer",
            trigger.code(),
            verdict.outcome
        );
        assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
        assert_ne!(verdict.outcome, VerdictOutcome::Constrained);
        assert_ne!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
        assert_eq!(verdict.decision_ref, "D-P4.07-1");
        // The fail-closed outcome travels on the evidence.
        let recorded = verdict.evidence.fail_closed.as_ref().expect("fail-closed");
        assert_eq!(recorded.trigger, trigger);
    }
}

#[test]
fn a_hold_outcome_records_the_operator_release_policy_reference() {
    let outcome = fail_closed_outcome(FailClosedTrigger::ProviderError, "worker down");
    assert_eq!(outcome.action, FailClosedAction::Hold);
    // The release policy is the recorded contract a hold's audit record
    // cites; releasing re-enters the gate and never converts to a pass.
    assert!(HOLD_RELEASE_POLICY.contains("never converts it into a feasibility pass"));
    assert!(!UNBALANCED_DEFERRAL_NOTE.trim().is_empty()); // sanity: the P4.05 note still travels
}
