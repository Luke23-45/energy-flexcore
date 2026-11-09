// P4.08 — The fail-closed policy: validator failure, stale/incomplete model,
// missing telemetry, unresolvable snapshot refs, scope-undeclared adapters,
// hash mismatches — NONE of these may infer feasibility (report §12.5 final
// paragraph, §17.5 invariant 9; ledger P4.08). Decision record:
// `ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md`
// (decisions D-P4.08-1 and D-P4.08-2).
//
// THE RULE, in one place: every trigger in the public [`FailClosedTrigger`]
// enumeration maps — through the recorded policy table (D-P4.08-1) — to a
// recorded fail-closed outcome of REJECT or HOLD. There is no code path,
// type, or function in this module that can produce a pass: the outcome
// type has no pass variant and the classification functions return
// [`FailClosedOutcome`] unconditionally.
//
// THE DEFAULT POSTURE (D-P4.08-2): any UNEXPECTED error class in the gate
// pipeline is also fail-closed. This is enforced structurally: the error
// classes of the gate pipeline are enumerated in [`PipelineError`] and
// classified in [`classify_pipeline_error`] by a match WITHOUT a wildcard
// arm — a new error variant without a fail-closed mapping is a COMPILE
// failure, and the in-crate plus integration tests assert that every
// variant classifies to a fail-closed outcome.
//
// Relationship to earlier subtasks (the pre-wiring this module completes):
// - P4.04 (D-P4.04-3): BAD/MISSING/STALE ramp baselines block the ramp
//   check — the telemetry half of the trigger enumeration.
// - P4.05 (D-P4.05-2): non-convergence is an in-band INCONCLUSIVE with
//   retry policy NONE; here it is enumerated as a fail-closed trigger with
//   the HOLD action, rendered at the verdict layer (P4.07) as the
//   first-class Inconclusive outcome — the same fail-closed shape, never a
//   pass, never a rejection-by-limit.
// - P4.03 (D-P4.03-3): stale/incomplete/uncovered models refuse the run.
//
// Control status: simulation-only 0.x line. A fail-closed outcome is an
// evidence-backed record, not a safety action: FlexCore is a supervisory
// orchestration layer and nothing here touches a device.

use crate::feasibility::{AttemptRecordError, SeamError};
use crate::model_health::ModelHealthError;
use crate::port::GridPortError;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recorded statements
// ---------------------------------------------------------------------------

/// The fail-closed rule, carried wherever a fail-closed outcome travels.
pub const FAIL_CLOSED_STATEMENT: &str = "Fail-closed rule (§12.5 final \
     paragraph; §17.5 invariant 9): if the validator fails or the model is \
     stale/incomplete — or any enumerated fail-closed trigger fires — the \
     system MUST NOT infer feasibility. Each trigger maps to a recorded \
     REJECT or HOLD per the policy table (D-P4.08-1); no trigger yields a \
     pass.";

/// The hold-release policy (ledger P4.08 step 2: "who may release a hold").
/// Recorded as decision D-P4.08-1; enforced socially by the approval gate,
/// recorded here as the contract the audit evidence cites.
pub const HOLD_RELEASE_POLICY: &str = "A HOLD outcome is releasable ONLY by \
     a human operator, with a recorded reason, through the approval gate \
     (§14.1 — authorization is a distinct gate). No automated path releases \
     a hold; releasing a hold never converts it into a feasibility pass — \
     the candidate re-enters the gate from step 1.";

/// The exhaustive-match requirement (D-P4.08-2), recorded so the structural
/// guarantee is auditable.
pub const EXHAUSTIVE_MATCH_STATEMENT: &str = "Default posture: any UNEXPECTED \
     error class in the gate pipeline is fail-closed. The pipeline's error \
     classes are enumerated in PipelineError and classified by a match \
     WITHOUT a wildcard arm: a new error variant without a fail-closed \
     mapping is a compile failure, and the tests assert every variant maps \
     to REJECT or HOLD with a non-empty reason.";

// ---------------------------------------------------------------------------
// The trigger enumeration (the module's public contract)
// ---------------------------------------------------------------------------

/// The fail-closed trigger set (ledger P4.08 step 1: "Enumerate fail-closed
/// triggers"). This enumeration is the module's PUBLIC CONTRACT: every
/// condition that must never infer feasibility is named here, and the
/// policy table ([`fail_closed_action`]) maps each to REJECT or HOLD.
///
/// The telemetry half of the enumeration is the P4.04 injection-quality
/// policy (D-P4.04-3): BAD, MISSING, and STALE baseline quality states are
/// exactly the blocked set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailClosedTrigger {
    /// A validator/seam exception or timeout — the step 1/2 validation
    /// machinery itself failed (infrastructure, not a candidate fault).
    ValidatorFailed,
    /// The model is stale, incomplete, or uncovered (the P4.03 health
    /// check refused), or the model version is unresolvable.
    ModelStaleOrIncomplete,
    /// The live model content hash no longer matches the recorded hash —
    /// a mid-gate model change or an edited artefact (D-P4.03-2 hook).
    ModelHashMismatch,
    /// Telemetry the check needs is present but in the blocked quality set
    /// (BAD, MISSING, STALE — the D-P4.04-3 policy; §10.4).
    TelemetryQualityBlocked,
    /// Telemetry the check needs is not available at all (the seam failed
    /// or answered unknown — unknown is never usable, the P3.13 rule).
    TelemetryUnavailable,
    /// A required input-snapshot / result-record / envelope reference
    /// cannot be resolved against the caller's stores.
    SnapshotRefUnresolvable,
    /// An adapter without a declared scope was used, or a request was
    /// attempted outside the declared scope (ledger P4.02 step 3).
    ScopeUndeclared,
    /// Required capability / equipment-limit data is not declared for an
    /// asset or bus — unknown is never unconstrained (the P3.02 rule).
    CapabilityDataMissing,
    /// The candidate itself is malformed in a way that slipped past the
    /// structural checks (a candidate fault; fail-closed by default).
    CandidateMalformed,
    /// The AC power flow did not converge (a result, reported with
    /// diagnostics; retry policy NONE per D-P4.05-2). Rendered at the
    /// verdict layer as the first-class INCONCLUSIVE outcome — never a
    /// pass, never a rejection-by-limit.
    PowerFlowNonConvergence,
    /// The grid-model provider/backend failed (unavailable, contract
    /// violation, incomplete result) — infrastructure, not a candidate
    /// fault.
    ProviderError,
    /// A Tier 2 (SOCP) candidate arrived WITHOUT its P3.18
    /// relaxation-residual record, or the record is malformed/failed
    /// (P4.09; §12.5 branch-flow SOCP paragraph).
    RelaxationRecordMissing,
    /// Any error class not enumerated above (the default posture,
    /// D-P4.08-2).
    UnexpectedError,
}

impl FailClosedTrigger {
    /// The frozen machine-readable trigger code (audit text matches on it).
    pub const fn code(self) -> &'static str {
        match self {
            FailClosedTrigger::ValidatorFailed => "VALIDATOR_FAILED",
            FailClosedTrigger::ModelStaleOrIncomplete => "MODEL_STALE_OR_INCOMPLETE",
            FailClosedTrigger::ModelHashMismatch => "MODEL_HASH_MISMATCH",
            FailClosedTrigger::TelemetryQualityBlocked => "TELEMETRY_QUALITY_BLOCKED",
            FailClosedTrigger::TelemetryUnavailable => "TELEMETRY_UNAVAILABLE",
            FailClosedTrigger::SnapshotRefUnresolvable => "SNAPSHOT_REF_UNRESOLVABLE",
            FailClosedTrigger::ScopeUndeclared => "SCOPE_UNDECLARED",
            FailClosedTrigger::CapabilityDataMissing => "CAPABILITY_DATA_MISSING",
            FailClosedTrigger::CandidateMalformed => "CANDIDATE_MALFORMED",
            FailClosedTrigger::PowerFlowNonConvergence => "POWER_FLOW_NON_CONVERGENCE",
            FailClosedTrigger::ProviderError => "PROVIDER_ERROR",
            FailClosedTrigger::RelaxationRecordMissing => "RELAXATION_RECORD_MISSING",
            FailClosedTrigger::UnexpectedError => "UNEXPECTED_ERROR",
        }
    }

    /// Every trigger, in enumeration order (the policy-table order). Used
    /// by the tests to prove the table and the classification are total.
    pub const ALL: [FailClosedTrigger; 13] = [
        FailClosedTrigger::ValidatorFailed,
        FailClosedTrigger::ModelStaleOrIncomplete,
        FailClosedTrigger::ModelHashMismatch,
        FailClosedTrigger::TelemetryQualityBlocked,
        FailClosedTrigger::TelemetryUnavailable,
        FailClosedTrigger::SnapshotRefUnresolvable,
        FailClosedTrigger::ScopeUndeclared,
        FailClosedTrigger::CapabilityDataMissing,
        FailClosedTrigger::CandidateMalformed,
        FailClosedTrigger::PowerFlowNonConvergence,
        FailClosedTrigger::ProviderError,
        FailClosedTrigger::RelaxationRecordMissing,
        FailClosedTrigger::UnexpectedError,
    ];

    /// The recorded default reason for a trigger, used when a caller
    /// supplies no detail — a fail-closed outcome NEVER carries an empty
    /// reason (silence is never legal, the §12.6 evidence-discipline
    /// pattern).
    pub const fn default_reason(self) -> &'static str {
        match self {
            FailClosedTrigger::ValidatorFailed => {
                "the validation machinery failed (exception/timeout); no validation \
                 evidence exists, so no feasibility may be inferred"
            }
            FailClosedTrigger::ModelStaleOrIncomplete => {
                "the network model is stale, incomplete, or uncovered; a check \
                 against an unfit model proves nothing (§17.5 invariant 9)"
            }
            FailClosedTrigger::ModelHashMismatch => {
                "the live model content no longer hashes to the recorded hash; prior \
                 evidence citing that hash is invalidated (D-P4.03-2)"
            }
            FailClosedTrigger::TelemetryQualityBlocked => {
                "required telemetry is BAD/MISSING/STALE; the check cannot run on \
                 unusable data (D-P4.04-3, §10.4)"
            }
            FailClosedTrigger::TelemetryUnavailable => {
                "required telemetry is not available at all; unknown is never usable"
            }
            FailClosedTrigger::SnapshotRefUnresolvable => {
                "a required input-snapshot/result-record reference cannot be resolved; \
                 an untraceable candidate is never evaluated"
            }
            FailClosedTrigger::ScopeUndeclared => {
                "the request is outside a declared adapter scope (or the scope was \
                 never declared); out-of-scope evaluation is refused, never approximated"
            }
            FailClosedTrigger::CapabilityDataMissing => {
                "required capability/equipment-limit data is not declared; unknown is \
                 never unconstrained (the P3.02 rule)"
            }
            FailClosedTrigger::CandidateMalformed => {
                "the candidate is malformed; a structurally invalid candidate is never \
                 evaluated against the network"
            }
            FailClosedTrigger::PowerFlowNonConvergence => {
                "the AC power flow did not converge; no network verdict exists \
                 (retry policy NONE, D-P4.05-2)"
            }
            FailClosedTrigger::ProviderError => {
                "the grid-model provider/backend failed; no evaluation evidence exists"
            }
            FailClosedTrigger::RelaxationRecordMissing => {
                "the Tier 2 candidate carries no usable P3.18 relaxation-residual \
                 record; the relaxation boundary is fail-closed (P4.09)"
            }
            FailClosedTrigger::UnexpectedError => {
                "an unexpected error class occurred in the gate pipeline; the default \
                 posture is fail-closed (D-P4.08-2)"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The policy table (D-P4.08-1)
// ---------------------------------------------------------------------------

/// The action the policy table assigns to a trigger.
///
/// There is deliberately NO pass variant: the type makes "a fail-closed
/// trigger maps to a pass" unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailClosedAction {
    /// The candidate/request is rejected with the recorded reason. It may
    /// be re-proposed after the named condition is fixed (it re-enters the
    /// gate from step 1 as a new candidate).
    Reject,
    /// The candidate is HELD: no feasibility is inferred, the candidate is
    /// parked pending operator action. Releasable ONLY by an operator with
    /// a recorded reason ([`HOLD_RELEASE_POLICY`]); release re-enters the
    /// gate, it never converts into a pass.
    Hold,
}

impl FailClosedAction {
    /// The canonical action code for records.
    pub const fn code(self) -> &'static str {
        match self {
            FailClosedAction::Reject => "REJECT",
            FailClosedAction::Hold => "HOLD",
        }
    }
}

/// The recorded fail-closed outcome for one trigger: trigger + action +
/// non-empty reason. The outcome is EVIDENCE (it names the trigger for the
/// audit record, ledger P4.08 step 4) — never a feasibility inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailClosedOutcome {
    /// The trigger that fired.
    pub trigger: FailClosedTrigger,
    /// The policy action (REJECT or HOLD, per the D-P4.08-1 table).
    pub action: FailClosedAction,
    /// The recorded reason (non-empty; audit text, safe to log).
    pub reason: String,
}

/// The policy table (D-P4.08-1), decided once and recorded in the decision
/// note. Rationale per row:
///
/// | Trigger | Action | Rationale |
/// |---|---|---|
/// | ValidatorFailed | HOLD | infrastructure/implementation fault; an operator investigates before anything re-runs |
/// | ModelStaleOrIncomplete | HOLD | model-governance issue; the model must be re-verified, not the candidate re-proposed |
/// | ModelHashMismatch | HOLD | integrity anomaly; investigate before any further use of the model |
/// | TelemetryQualityBlocked | REJECT | the data exists but is unusable for this candidate window; a repaired re-proposal re-enters as a new candidate |
/// | TelemetryUnavailable | HOLD | the telemetry source failed; infrastructure |
/// | SnapshotRefUnresolvable | REJECT | the candidate cites evidence that does not exist — a candidate fault |
/// | ScopeUndeclared | REJECT | the caller/adapter configuration is wrong; refusing is the contract (P4.02) |
/// | CapabilityDataMissing | REJECT | the candidate references capability that is not declared — re-propose after registration |
/// | CandidateMalformed | REJECT | a candidate fault |
/// | PowerFlowNonConvergence | HOLD | no network verdict exists (rendered INCONCLUSIVE at the verdict layer, retry NONE — D-P4.05-2) |
/// | ProviderError | HOLD | infrastructure |
/// | RelaxationRecordMissing | REJECT | the Tier 2 submission is incomplete; re-submit with the record (P4.09) |
/// | UnexpectedError | HOLD | the default posture: unknown failure → park for investigation, never pass |
pub fn fail_closed_action(trigger: FailClosedTrigger) -> FailClosedAction {
    match trigger {
        FailClosedTrigger::ValidatorFailed => FailClosedAction::Hold,
        FailClosedTrigger::ModelStaleOrIncomplete => FailClosedAction::Hold,
        FailClosedTrigger::ModelHashMismatch => FailClosedAction::Hold,
        FailClosedTrigger::TelemetryQualityBlocked => FailClosedAction::Reject,
        FailClosedTrigger::TelemetryUnavailable => FailClosedAction::Hold,
        FailClosedTrigger::SnapshotRefUnresolvable => FailClosedAction::Reject,
        FailClosedTrigger::ScopeUndeclared => FailClosedAction::Reject,
        FailClosedTrigger::CapabilityDataMissing => FailClosedAction::Reject,
        FailClosedTrigger::CandidateMalformed => FailClosedAction::Reject,
        FailClosedTrigger::PowerFlowNonConvergence => FailClosedAction::Hold,
        FailClosedTrigger::ProviderError => FailClosedAction::Hold,
        FailClosedTrigger::RelaxationRecordMissing => FailClosedAction::Reject,
        FailClosedTrigger::UnexpectedError => FailClosedAction::Hold,
    }
}

/// Builds the recorded fail-closed outcome for a trigger. The reason is
/// never empty: an empty/whitespace `detail` falls back to the trigger's
/// recorded default reason.
pub fn fail_closed_outcome(trigger: FailClosedTrigger, detail: &str) -> FailClosedOutcome {
    let reason = if detail.trim().is_empty() {
        trigger.default_reason().to_string()
    } else {
        format!("{} (detail: {detail})", trigger.default_reason())
    };
    FailClosedOutcome {
        trigger,
        action: fail_closed_action(trigger),
        reason,
    }
}

// ---------------------------------------------------------------------------
// The pipeline error enumeration and the default posture (D-P4.08-2)
// ---------------------------------------------------------------------------

/// The error classes of the feasibility-gate pipeline, enumerated so the
/// default posture is structurally enforced: [`classify_pipeline_error`]
/// matches every variant WITHOUT a wildcard arm, so a new error class
/// without a fail-closed mapping is a compile failure (D-P4.08-2).
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineError {
    /// An injected source seam failed (capability bounds, envelope
    /// freshness, revision state, ramp baseline).
    Seam(SeamError),
    /// The model-health layer refused or errored (P4.03).
    ModelHealth(ModelHealthError),
    /// The grid-model port failed (infrastructure/contract).
    Port(GridPortError),
    /// The attempt record failed its completeness gate (a pipeline bug —
    /// an incomplete record is never published, D-P4.05-4).
    AttemptRecord(AttemptRecordError),
    /// Required telemetry is in the blocked quality set (BAD/MISSING/STALE,
    /// D-P4.04-3).
    TelemetryQualityBlocked { detail: String },
    /// A required input-snapshot / result-record reference is unresolvable.
    SnapshotRefUnresolvable { detail: String },
    /// A request outside a declared scope was attempted.
    ScopeUndeclared { detail: String },
    /// Required capability / equipment-limit data is not declared.
    CapabilityDataMissing { detail: String },
    /// The candidate is malformed.
    CandidateMalformed { detail: String },
    /// The AC power flow did not converge.
    PowerFlowNonConvergence { detail: String },
    /// A Tier 2 candidate arrived without a usable relaxation-residual
    /// record (P4.09).
    RelaxationRecordMissing { detail: String },
}

/// Classifies any pipeline error into its recorded fail-closed outcome
/// (D-P4.08-2). The match has NO wildcard arm: adding a [`PipelineError`]
/// variant without a mapping here is a compile failure — the structural
/// guarantee that no error class can fall through to a pass.
pub fn classify_pipeline_error(error: &PipelineError) -> FailClosedOutcome {
    match error {
        PipelineError::Seam(seam) => {
            // Any seam error is fail-closed. The ramp-baseline seam is a
            // telemetry failure; the others are validation-machinery
            // failures. A seam ERROR is infrastructure either way — HOLD.
            let trigger = if seam.seam == "ramp_baseline" {
                FailClosedTrigger::TelemetryUnavailable
            } else {
                FailClosedTrigger::ValidatorFailed
            };
            fail_closed_outcome(
                trigger,
                &format!("seam '{}' failed: {}", seam.seam, seam.detail),
            )
        }
        PipelineError::ModelHealth(health) => match health {
            ModelHealthError::NotUsable { findings } => fail_closed_outcome(
                FailClosedTrigger::ModelStaleOrIncomplete,
                &format!(
                    "the model health check refused with {} finding(s)",
                    findings.len()
                ),
            ),
            ModelHealthError::MalformedModel(_) | ModelHealthError::InvalidCoverage { .. } => {
                fail_closed_outcome(
                    FailClosedTrigger::ModelStaleOrIncomplete,
                    "the model artefact failed structural/coverage validation",
                )
            }
            ModelHealthError::UnknownVersion { model_id, version } => fail_closed_outcome(
                FailClosedTrigger::ModelStaleOrIncomplete,
                &format!("model '{model_id}' version '{version}' is not registered"),
            ),
            ModelHealthError::InvalidTimestamp
            | ModelHealthError::InvalidSourceChange { .. }
            | ModelHealthError::VersionAlreadyExists { .. }
            | ModelHealthError::IllegalTransition { .. } => fail_closed_outcome(
                FailClosedTrigger::UnexpectedError,
                "the model-health layer reported an unexpected bookkeeping error",
            ),
        },
        PipelineError::Port(port) => match port {
            GridPortError::ScopeExceeded(_) => fail_closed_outcome(
                FailClosedTrigger::ScopeUndeclared,
                "the request exceeds the adapter's declared scope",
            ),
            GridPortError::BackendUnavailable(_)
            | GridPortError::BackendContractViolation(_)
            | GridPortError::IncompleteResult(_) => fail_closed_outcome(
                FailClosedTrigger::ProviderError,
                "the grid-model backend failed or violated its contract",
            ),
            GridPortError::MalformedModel(_) | GridPortError::InvalidCandidate(_) => {
                fail_closed_outcome(
                    FailClosedTrigger::CandidateMalformed,
                    "the model or candidate failed the port's structural validation",
                )
            }
        },
        PipelineError::AttemptRecord(record) => {
            let _ = record; // the variant payload is the record error itself
            fail_closed_outcome(
                FailClosedTrigger::UnexpectedError,
                "the assembled attempt record failed its completeness gate \
                 (a pipeline bug — an incomplete record is never published)",
            )
        }
        PipelineError::TelemetryQualityBlocked { detail } => {
            fail_closed_outcome(FailClosedTrigger::TelemetryQualityBlocked, detail)
        }
        PipelineError::SnapshotRefUnresolvable { detail } => {
            fail_closed_outcome(FailClosedTrigger::SnapshotRefUnresolvable, detail)
        }
        PipelineError::ScopeUndeclared { detail } => {
            fail_closed_outcome(FailClosedTrigger::ScopeUndeclared, detail)
        }
        PipelineError::CapabilityDataMissing { detail } => {
            fail_closed_outcome(FailClosedTrigger::CapabilityDataMissing, detail)
        }
        PipelineError::CandidateMalformed { detail } => {
            fail_closed_outcome(FailClosedTrigger::CandidateMalformed, detail)
        }
        PipelineError::PowerFlowNonConvergence { detail } => {
            fail_closed_outcome(FailClosedTrigger::PowerFlowNonConvergence, detail)
        }
        PipelineError::RelaxationRecordMissing { detail } => {
            fail_closed_outcome(FailClosedTrigger::RelaxationRecordMissing, detail)
        }
    }
}

// ---------------------------------------------------------------------------
// In-crate tests (the integration tests live in tests/fail_closed.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Every trigger's frozen code is stable and distinct.
    #[test]
    fn trigger_codes_are_stable_and_distinct() {
        let mut codes: Vec<&str> = FailClosedTrigger::ALL.iter().map(|t| t.code()).collect();
        let total = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), total, "trigger codes must be distinct");
        assert_eq!(total, 13);
    }

    /// The policy table maps EVERY trigger to Reject or Hold — there is no
    /// third action and no pass anywhere.
    #[test]
    fn the_policy_table_is_total_and_fail_closed() {
        for trigger in FailClosedTrigger::ALL {
            let action = fail_closed_action(trigger);
            assert!(
                action == FailClosedAction::Reject || action == FailClosedAction::Hold,
                "trigger {} mapped to neither REJECT nor HOLD",
                trigger.code()
            );
            assert!(!trigger.default_reason().trim().is_empty());
        }
    }

    /// The outcome reason is never empty, even for an empty detail.
    #[test]
    fn outcomes_always_carry_a_reason() {
        for trigger in FailClosedTrigger::ALL {
            let outcome = fail_closed_outcome(trigger, "");
            assert!(!outcome.reason.trim().is_empty());
            let outcome = fail_closed_outcome(trigger, "custom detail");
            assert!(outcome.reason.contains("custom detail"));
        }
    }
}
