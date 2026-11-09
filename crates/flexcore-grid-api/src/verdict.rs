// P4.07 — Feasibility-gate pipeline, steps 6–8: non-convergence and
// coverage limits reported EXPLICITLY, margin diagnostics finalised, and
// the reject / constrain / return routing (report §12.5 steps 6–8; ledger
// P4.07). Decision record:
// `ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md`
// (decisions D-P4.07-1..3).
//
// ```text
// step 6  non-convergence and coverage limits reported EXPLICITLY
// step 7  margin diagnostics + input-snapshot echo finalised
// step 8  routing: REJECT / CONSTRAIN / RETURN — decided once, pure
// ```
//
// STEP 6 (D-P4.07-3, the inconclusive category): the P4.05 Inconclusive
// evidence becomes a FIRST-CLASS outcome category
// ([`VerdictOutcome::Inconclusive`]). Non-convergence is named
// inconclusive — NEVER a pass and NEVER a limit-violation — and the retry
// policy is NONE (D-P4.05-2). Model-coverage limitations (from the result)
// and the step-5 scope limit gaps travel on EVERY verdict's evidence: an
// uncovered violation cannot be mistaken for a pass, and a within-limits
// verdict over a model with unverifiable limit classes is explicitly
// qualified.
//
// STEP 7 (D-P4.07-2): the P4.06 per-violation margins are finalised into
// the candidate diagnostics on the verdict evidence (worst margin
// fraction, binding constraint, named violations), together with the
// input-snapshot echo: model version + hash, provider id + version, and
// the injection derivation references.
//
// STEP 8 (D-P4.07-1, the routing rule — DECIDED ONCE, implemented as the
// pure function [`route_candidate`]): an unacceptable candidate routes to
// EXACTLY ONE of REJECTED / CONSTRAINED / RETURNED-FOR-RE-OPTIMIZATION
// (plus the two fail-closed-shaped categories: INCONCLUSIVE and HELD).
// Never silently adjusted and passed:
// - a fail-closed trigger (P4.08) routes per the policy table — REJECT or
//   HOLD, never a pass;
// - non-convergence / incomplete evidence routes INCONCLUSIVE (retry
//   NONE);
// - a HARD violation (loading strictly above the model-declared physical
//   rating, [`limits::PHYSICAL_RATING_FRACTION`]) routes REJECTED — the
//   equipment rating is exceeded and no gate-time adjustment is trusted;
// - a SINGLE operational (non-hard) thermal violation routes CONSTRAINED,
//   with a deterministic constraining proposal recorded — the proposal is
//   EVIDENCE, not an applied mutation, and the constrained candidate MUST
//   re-enter the gate (P4.37 defines the consumption contract);
// - a single voltage violation, a single capability (S_max) violation, or
//   TWO-OR-MORE violations route RETURNED-FOR-RE-OPTIMIZATION — the gate
//   never optimizes, and a voltage fix or a multi-constraint adjustment
//   is a re-dispatch decision that belongs to the scheduler, which
//   receives every violated constraint as a named input.
//
// Every routing carries its evidence, and the decision reference
// ([`ROUTING_DECISION_REF`]) is on every verdict.
//
// The [`FeasibilityCheckCompleted`] event (§15.3) is emitted per check as
// a payload record: outcome, model version, and evidence references. The
// outbox emission itself is the composition root's (grid-api cannot name
// the event bus; VERIFY-BUILD in the decision note).
//
// NO FEASIBILITY-GUARANTEE LANGUAGE: every verdict is an evidence-backed
// outcome against the DECLARED model and limits within stated tolerances —
// not a field-safety, certification, or approval claim (§12.5; §14.1:
// authorization is a distinct gate).
//
// Control status: simulation-only 0.x line; nothing here touches a device
// or authorises field control.

use crate::fail_closed::{
    fail_closed_outcome, FailClosedAction, FailClosedOutcome, FailClosedTrigger,
};
use crate::feasibility::{
    AttemptOutput, CandidateRefs, FeasibilityAttemptRecord, GateAbort, GateEvidenceRow,
    GatePipelineOutcome,
};
use crate::limits::{
    is_hard_violation, LimitCategory, LimitCheckRecord, LimitCheckReport, LimitCheckStatus,
    HARD_LIMIT_STATEMENT,
};
use crate::port::{CoverageLimitation, GridModelRef};
use crate::socp_boundary::SocpBoundaryGate;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recorded statements
// ---------------------------------------------------------------------------

/// The recorded routing decision reference, carried on EVERY verdict.
pub const ROUTING_DECISION_REF: &str = "D-P4.07-1";

/// The routing rule (D-P4.07-1), decided once — the exact rule
/// [`route_candidate`] implements, in evaluation order:
///
/// 1. A fail-closed trigger (an explicit outcome, a fail-closed SOC
///    boundary, or a fail-closed limit report) routes per the P4.08 policy
///    table: REJECT or HOLD — never a pass.
/// 2. Non-convergence / provider error / an inconclusive limit report
///    routes INCONCLUSIVE (first-class; retry policy NONE; never a pass
///    and never a rejection-by-limit).
/// 3. No violated constraint routes WITHIN-LIMITS (evidence only; the
///    coverage limitations and scope limit gaps travel; a Tier 2 candidate
///    carries the SOCP exactness caveat).
/// 4. Any HARD violation routes REJECTED (all violated constraints named).
/// 5. A single operational thermal violation (branch or transformer)
///    routes CONSTRAINED, with the deterministic proposal recorded.
/// 6. Anything else (single voltage violation, single capability
///    violation, or two-or-more violations) routes
///    RETURNED-FOR-RE-OPTIMIZATION with every violated constraint named.
pub const ROUTING_RULE_STATEMENT: &str = "Routing rule D-P4.07-1: \
     fail-closed → REJECT/HOLD per the P4.08 table; inconclusive → \
     INCONCLUSIVE (retry NONE); no violations → WITHIN-LIMITS (evidence \
     only); any hard violation → REJECT; single operational thermal \
     violation → CONSTRAIN (proposal = evidence, re-enter the gate); \
     otherwise (voltage, capability, or multi-violation) → RETURN for \
     re-optimization with the violated constraints as named inputs. The \
     gate never optimizes and never silently adjusts a candidate and \
     passes it.";

/// The constrain-proposal rule (D-P4.07-2): what the proposal is and what
/// it is not.
pub const CONSTRAIN_PROPOSAL_STATEMENT: &str = "The constraining proposal is \
     EVIDENCE, not an applied mutation (D-P4.07-2): a uniform proportional \
     scaling of every candidate injection by limit/measured, bringing the \
     violated element to its configured operational limit to first order. \
     The constrained candidate MUST re-enter the feasibility gate from \
     step 1; P4.37 defines the consumption contract. The gate never \
     optimizes and never passes a silently adjusted candidate.";

/// The completed-event decision (D-P4.07-3): a
/// [`FeasibilityCheckCompleted`] payload is produced for EVERY verdict —
/// outcome, model version, and evidence references (§15.3). The outbox /
/// event-bus emission is the composition root's: flexcore-grid-api cannot
/// name the event infrastructure (the core-crate dependency policy).
/// VERIFY-BUILD: the exact flexcore-events schema conversion is recorded
/// in `ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md`.
pub const FEASIBILITY_CHECK_COMPLETED_DECISION: &str = "A FeasibilityCheckCompleted \
     event payload is produced per check (D-P4.07-3); the composition root \
     emits it over the outbox (§15.3). A verdict that never produced the \
     payload is a pipeline bug; an event without a verdict is impossible \
     by construction here.";

// ---------------------------------------------------------------------------
// The verdict outcome vocabulary
// ---------------------------------------------------------------------------

/// The gate verdict outcome (§12.5 steps 6 and 8). Closed vocabulary; the
/// codes are frozen for evidence consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictOutcome {
    /// Every checked limit passed (boundary-inclusive), no fail-closed
    /// trigger fired. Evidence-backed against the declared model — NOT a
    /// field-safety, certification, or approval claim.
    WithinLimits,
    /// The candidate is rejected (hard violation, candidate fault, or a
    /// REJECT-class fail-closed trigger), with evidence attached.
    Rejected,
    /// The gate proposes a constraining adjustment — EVIDENCE, not an
    /// applied mutation; the constrained candidate re-enters the gate
    /// (P4.37 consumption contract).
    Constrained,
    /// The candidate returns to scheduling for re-optimization, with the
    /// violated constraints as named inputs.
    ReturnedForReOptimization,
    /// Non-convergence / provider error / incomplete evidence: a
    /// first-class outcome category — never a pass, never a
    /// limit-violation; retry policy NONE (D-P4.05-2).
    Inconclusive,
    /// A HOLD-class fail-closed trigger: no feasibility inferred; the
    /// candidate is parked pending operator release (operator only, with
    /// a recorded reason — the P4.08 release policy).
    Held,
}

impl VerdictOutcome {
    /// The canonical outcome code.
    pub const fn as_str(self) -> &'static str {
        match self {
            VerdictOutcome::WithinLimits => "within_limits",
            VerdictOutcome::Rejected => "rejected",
            VerdictOutcome::Constrained => "constrained",
            VerdictOutcome::ReturnedForReOptimization => "returned_for_re_optimization",
            VerdictOutcome::Inconclusive => "inconclusive",
            VerdictOutcome::Held => "held",
        }
    }
}

// ---------------------------------------------------------------------------
// The proposal, the echo, the evidence, the event
// ---------------------------------------------------------------------------

/// One proposed (NOT applied) injection value of a constraining proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedInjectionSetpoint {
    /// The injection identifier (as applied in the evaluated run).
    pub injection_id: String,
    /// The bus the injection applies at.
    pub bus_id: String,
    /// The proposed active power in MW (canonical sign).
    pub proposed_p_mw: f64,
    /// The proposed reactive power in MVAr (declared-convention sign).
    pub proposed_q_mvar: f64,
}

/// The constraining proposal (§12.5 step 8 CONSTRAIN; D-P4.07-2). This is
/// EVIDENCE: the proposal is recorded, never applied, and the constrained
/// candidate MUST re-enter the gate (P4.37 defines the consumption
/// contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstrainProposal {
    /// The violated constraint this proposal addresses (the named
    /// constraint from the limit report).
    pub violated_constraint: String,
    /// The named rule that produced the adjustment.
    pub rule: String,
    /// The uniform scale factor `limit / measured` (< 1 for a violation).
    pub scale_factor: f64,
    /// The proposed injection values (every applied injection, scaled).
    pub proposed_injections: Vec<ProposedInjectionSetpoint>,
    /// The predicted measured value at the violated element (the limit;
    /// margin 0, first order).
    pub predicted_value: f64,
    /// The configured limit the prediction targets.
    pub limit: f64,
    /// The recorded proposal statement (evidence-not-applied contract).
    pub statement: String,
}

/// The input-snapshot echo (§12.6 "input snapshot and grid model version";
/// ledger P4.07 step 7): the exact inputs the verdict was computed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSnapshotEcho {
    /// The model identifier.
    pub model_id: String,
    /// The model version.
    pub model_version: String,
    /// The model content hash (evidence references THE HASH, D-P4.03-2).
    pub model_content_hash: String,
    /// The provider identifier.
    pub provider_id: String,
    /// The provider version (REQUIRED — the P4.01 pin discipline).
    pub provider_version: String,
    /// The injection derivation references (baseline instant + quality
    /// encoded, D-P4.04-3).
    pub injection_derivation_refs: Vec<String>,
}

impl InputSnapshotEcho {
    /// Builds the echo from the attempt record.
    pub fn from_attempt(attempt: &FeasibilityAttemptRecord) -> Self {
        Self {
            model_id: attempt.model_ref.model_id.clone(),
            model_version: attempt.model_ref.version.clone(),
            model_content_hash: attempt.model_ref.content_hash.clone(),
            provider_id: attempt.provider_id.clone(),
            provider_version: attempt.provider_version.clone(),
            injection_derivation_refs: attempt.injection_derivation_refs.clone(),
        }
    }
}

/// The evidence attached to every verdict (ledger P4.07 acceptance: "each
/// outcome ... with distinct status and evidence"; "constrained/returned
/// candidates carry the violated constraints"; "uncovered-network
/// limitations are stated, not hidden").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictEvidence {
    /// The attempt identifier (empty when the verdict routes before an
    /// attempt exists — pipeline-level rejections/aborts).
    pub attempt_id: String,
    /// The input-snapshot echo, when an attempt was evaluated.
    pub snapshot_echo: Option<InputSnapshotEcho>,
    /// The model-coverage limitations from the result (§12.5 step 6).
    pub coverage_limitations: Vec<CoverageLimitation>,
    /// The scope-declared limit gaps (limits the declared scope cannot
    /// verify) — stated, never hidden.
    pub scope_limit_gaps: Vec<String>,
    /// How many limit checks were recorded (0 when none ran).
    pub checks_recorded: usize,
    /// The named violated constraints.
    pub violations_named: Vec<String>,
    /// The worst margin fraction across all categories (from the P4.06
    /// diagnostics), when a limit check ran.
    pub worst_margin_fraction: Option<f64>,
    /// The binding-constraint statement (smallest margin fraction), when
    /// recorded.
    pub binding_constraint: Option<String>,
    /// The caveats on this verdict (for a Tier 2 candidate, ALWAYS
    /// contains the SOCP exactness caveat — D-P4.09-2).
    pub caveats: Vec<String>,
    /// The recorded findings (condition gaps, residual findings) — never
    /// hidden.
    pub findings: Vec<String>,
    /// The fail-closed outcome, when the verdict is fail-closed-shaped
    /// (REJECTED via a fail-closed trigger, or HELD).
    pub fail_closed: Option<FailClosedOutcome>,
    /// The distinct rule codes of the pipeline evidence rows, in first-
    /// occurrence order.
    pub evidence_row_rule_codes: Vec<String>,
}

/// The §15.3 `FeasibilityCheckCompleted` event payload: emitted per check
/// with the outcome, the model version, and the evidence references
/// (ledger P4.07 step 4). The outbox emission is the composition root's;
/// this record is the payload contract (VERIFY-BUILD: the exact
/// flexcore-events schema conversion is recorded in the decision note).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeasibilityCheckCompleted {
    /// The check identifier (the attempt id; pipeline-level verdicts use a
    /// `gate:<candidate>` id).
    pub check_id: String,
    /// The candidate identifier.
    pub candidate_id: String,
    /// The schedule chain id.
    pub schedule_id: String,
    /// The schedule revision.
    pub revision: u64,
    /// The envelope version.
    pub envelope_version: String,
    /// The canonical outcome code ([`VerdictOutcome::as_str`]).
    pub outcome_code: String,
    /// The recorded reason (non-empty).
    pub reason: String,
    /// The model identifier the check evaluated against.
    pub model_id: String,
    /// The model version.
    pub model_version: String,
    /// The model content hash.
    pub model_content_hash: String,
    /// The evidence references: the attempt id, the result-record snapshot
    /// ref, and every injection derivation reference.
    pub evidence_refs: Vec<String>,
    /// UTC millis the check completed (the caller-supplied gate clock).
    pub occurred_at_millis: i64,
}

/// The feasibility-gate verdict (§12.5 steps 6–8): ONE evidence-backed
/// outcome for the candidate, with the routing decision reference, the
/// named violated constraints, the constraining proposal when routed to
/// CONSTRAIN, the full evidence block, and the completed event payload.
///
/// NO FEASIBILITY-GUARANTEE LANGUAGE: a `WithinLimits` verdict means the
/// candidate satisfied the DECLARED model and limits within stated
/// tolerances — not that it is safe in the field, certified, or approved
/// (authorization is a distinct gate, §14.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateVerdict {
    /// The routing decision reference ([`ROUTING_DECISION_REF`]) — on
    /// EVERY verdict (ledger P4.07 test requirement).
    pub decision_ref: String,
    /// The outcome.
    pub outcome: VerdictOutcome,
    /// The canonical outcome code.
    pub outcome_code: String,
    /// The recorded reason (non-empty; audit text).
    pub reason: String,
    /// The candidate references.
    pub candidate_refs: CandidateRefs,
    /// The named violated constraints (empty when none).
    pub violated_constraints: Vec<String>,
    /// The constraining proposal, exactly when the outcome is
    /// [`VerdictOutcome::Constrained`].
    pub constrain_proposal: Option<ConstrainProposal>,
    /// The evidence block.
    pub evidence: VerdictEvidence,
    /// The `FeasibilityCheckCompleted` event payload (§15.3).
    pub completed_event: FeasibilityCheckCompleted,
}

// ---------------------------------------------------------------------------
// The routing inputs
// ---------------------------------------------------------------------------

/// Everything the step-8 routing consumes for a completed attempt: the
/// attempt record (P4.05), the step-5 limit report (P4.06), the optional
/// SOC boundary gate result (P4.09), an optional explicit fail-closed
/// outcome (P4.08), and the pipeline evidence rows.
pub struct RoutingInput<'a> {
    /// The completed attempt record (its output MUST be
    /// `AttemptOutput::PowerFlowEvaluated` for a limit-based routing; an
    /// `Inconclusive` output routes INCONCLUSIVE).
    pub attempt: &'a FeasibilityAttemptRecord,
    /// The step-5 limit-check report.
    pub limit_report: &'a LimitCheckReport,
    /// The P4.09 SOC boundary gate result, when the tier provenance was
    /// recorded.
    pub socp_gate: Option<&'a SocpBoundaryGate>,
    /// An explicit fail-closed outcome recorded upstream (P4.08), when any.
    pub fail_closed: Option<&'a FailClosedOutcome>,
    /// The pipeline evidence rows (steps 1–4).
    pub pipeline_evidence: &'a [GateEvidenceRow],
    /// The gate clock (UTC millis).
    pub now_millis: i64,
}

// ---------------------------------------------------------------------------
// The routing (pure, decided once — D-P4.07-1)
// ---------------------------------------------------------------------------

/// Routes a completed attempt to its verdict (§12.5 steps 6–8; the pure
/// function implementing [`ROUTING_RULE_STATEMENT`]). See the module
/// header for the recorded rule and its rationale.
pub fn route_candidate(input: RoutingInput) -> GateVerdict {
    let refs = input.attempt.candidate_refs.clone();

    // Rule 1 — fail-closed triggers, in precedence order: the SOC boundary
    // (P4.09), an explicit upstream outcome (P4.08), and a fail-closed
    // limit report.
    if let Some(SocpBoundaryGate::FailClosed(outcome)) = input.socp_gate {
        return fail_closed_verdict(&refs, outcome, Some(input.attempt), input.pipeline_evidence, input.now_millis);
    }
    if let Some(outcome) = input.fail_closed {
        return fail_closed_verdict(&refs, outcome, Some(input.attempt), input.pipeline_evidence, input.now_millis);
    }
    if let LimitCheckStatus::FailClosed { trigger, reason } = &input.limit_report.status {
        let outcome = fail_closed_outcome(*trigger, reason);
        return fail_closed_verdict(&refs, &outcome, Some(input.attempt), input.pipeline_evidence, input.now_millis);
    }

    // Rule 2 — INCONCLUSIVE: non-convergence, provider error, or an
    // inconclusive limit report. First-class category: never a pass,
    // never a rejection-by-limit; retry policy NONE (D-P4.05-2).
    let inconclusive_reason = match (&input.attempt.output, &input.limit_report.status) {
        (AttemptOutput::Inconclusive { reason, .. }, _) => Some(reason.clone()),
        (_, LimitCheckStatus::Inconclusive { reason }) => Some(reason.clone()),
        _ => None,
    };
    if let Some(reason) = inconclusive_reason {
        let full_reason = format!(
            "{reason} Retry policy: NONE. Reported INCONCLUSIVE per §12.5 \
             step 6 — never a pass, never a limit-violation."
        );
        return build_verdict(
            &refs,
            VerdictOutcome::Inconclusive,
            full_reason,
            Vec::new(),
            None,
            evidence_from(&input, Vec::new(), Vec::new()),
            input.now_millis,
        );
    }

    // The limit report must be conclusive here (WithinLimits or Violated).
    let violations: Vec<LimitCheckRecord> = input.limit_report.violations.clone();

    // Rule 3 — no violations: WITHIN-LIMITS, evidence only, coverage
    // qualifications attached.
    if violations.is_empty() {
        let mut reason = format!(
            "every checked limit passed within tolerances against model '{}' \
             version '{}' (boundary-inclusive convention, D-P4.06-3) — an \
             evidence-backed outcome against the DECLARED model, not a \
             field-safety, certification, or approval claim",
            input.attempt.model_ref.model_id, input.attempt.model_ref.version
        );
        let gaps = scope_gap_labels(input.limit_report);
        if !gaps.is_empty() {
            reason.push_str(&format!(
                " — QUALIFIED: the declared scope cannot verify limits in \
                 these classes: {}; a violation there cannot be seen",
                gaps.join(", ")
            ));
        }
        let (caveats, findings) = tier2_caveats_and_findings(input.socp_gate);
        return build_verdict(
            &refs,
            VerdictOutcome::WithinLimits,
            reason,
            Vec::new(),
            None,
            evidence_from(&input, caveats, findings),
            input.now_millis,
        );
    }

    let names: Vec<String> = violations
        .iter()
        .map(|check| check.constraint_name())
        .collect();

    // Rule 4 — any HARD violation: REJECTED. The physical equipment rating
    // is exceeded; no gate-time adjustment is trusted, and the evidence
    // names every violated constraint.
    if violations.iter().any(is_hard_violation) {
        let reason = format!(
            "HARD limit violation(s): {} — {}. {HARD_LIMIT_STATEMENT}",
            names.join(", "),
            "the measured loading strictly exceeds the model-declared physical \
             equipment rating"
        );
        return build_verdict(
            &refs,
            VerdictOutcome::Rejected,
            reason,
            names,
            None,
            evidence_from(&input, Vec::new(), Vec::new()),
            input.now_millis,
        );
    }

    // Rule 5 — a SINGLE operational thermal violation: CONSTRAINED, with
    // the deterministic proposal recorded as evidence.
    if violations.len() == 1 {
        let check = &violations[0];
        if matches!(
            check.category,
            LimitCategory::BranchThermal | LimitCategory::TransformerLoading
        ) {
            let proposal = constrain_proposal(check, input.attempt);
            let reason = format!(
                "single operational thermal violation {} (measured {:.6}, limit \
                 {:.6}, margin {:+.6}); the gate proposes a constraining \
                 adjustment — EVIDENCE, not an applied mutation; the constrained \
                 candidate MUST re-enter the gate (P4.37 consumption contract)",
                check.constraint_name(),
                check.measured,
                check.limit,
                check.margin
            );
            return build_verdict(
                &refs,
                VerdictOutcome::Constrained,
                reason,
                names,
                Some(proposal),
                evidence_from(&input, Vec::new(), Vec::new()),
                input.now_millis,
            );
        }
    }

    // Rule 6 — everything else: RETURNED-FOR-RE-OPTIMIZATION with every
    // violated constraint as a named input. The gate never optimizes; a
    // voltage fix or a multi-constraint adjustment is a re-dispatch
    // decision that belongs to the scheduler.
    let rationale = if violations.len() == 1 {
        match violations[0].category {
            LimitCategory::BusVoltageLower | LimitCategory::BusVoltageUpper => {
                "a voltage-band violation cannot be fixed by a deterministic \
                 gate-side adjustment without optimizing, and the gate never \
                 optimizes (§12.5)"
            }
            LimitCategory::GeneratorApparentPower => {
                "the S_max capability model is the scheduler's input; a \
                 capability-model violation returns for re-optimization within \
                 the declared capability"
            }
            _ => "the violation is not a single operational thermal violation",
        }
    } else {
        "two or more constraints are violated at once; a constraining \
         adjustment is under-determined, so the candidate returns with EVERY \
         violated constraint as a named input"
    };
    let reason = format!(
        "the candidate returns for re-optimization: violated constraint(s) {}. \
         {rationale}.",
        names.join(", ")
    );
    build_verdict(
        &refs,
        VerdictOutcome::ReturnedForReOptimization,
        reason,
        names,
        None,
        evidence_from(&input, Vec::new(), Vec::new()),
        input.now_millis,
    )
}

/// Routes a fail-closed outcome that fired BEFORE an attempt exists
/// (pipeline-level aborts — model health, hash mismatch, provider errors).
/// REJECT-class triggers route REJECTED; HOLD-class triggers route HELD;
/// nothing routes to a pass.
pub fn route_fail_closed(
    refs: &CandidateRefs,
    outcome: &FailClosedOutcome,
    model_ref: Option<&GridModelRef>,
    now_millis: i64,
) -> GateVerdict {
    let evidence = VerdictEvidence {
        attempt_id: String::new(),
        snapshot_echo: model_ref.map(|model_ref| InputSnapshotEcho {
            model_id: model_ref.model_id.clone(),
            model_version: model_ref.version.clone(),
            model_content_hash: model_ref.content_hash.clone(),
            provider_id: String::new(),
            provider_version: String::new(),
            injection_derivation_refs: Vec::new(),
        }),
        coverage_limitations: Vec::new(),
        scope_limit_gaps: Vec::new(),
        checks_recorded: 0,
        violations_named: Vec::new(),
        worst_margin_fraction: None,
        binding_constraint: None,
        caveats: Vec::new(),
        findings: Vec::new(),
        fail_closed: Some(outcome.clone()),
        evidence_row_rule_codes: Vec::new(),
    };
    build_verdict(
        refs,
        fail_closed_outcome_kind(outcome.action),
        outcome.reason.clone(),
        Vec::new(),
        None,
        evidence,
        now_millis,
    )
}

/// Routes a FULL pipeline outcome (steps 1–4, `run_feasibility_gate`) to
/// its verdict — the end-to-end entry point. A step-1/2 rejection routes
/// REJECTED (candidate fault, evidence = the rejection); a step-3/4 abort
/// maps to its fail-closed trigger (P4.08); a completed attempt requires
/// the step-5 limit report and routes through [`route_candidate`].
pub fn route_pipeline_outcome(
    outcome: &GatePipelineOutcome,
    limit_report: Option<&LimitCheckReport>,
    socp_gate: Option<&SocpBoundaryGate>,
    now_millis: i64,
) -> GateVerdict {
    match outcome {
        GatePipelineOutcome::Rejected {
            rejection, evidence, ..
        } => {
            let refs = CandidateRefs {
                candidate_id: String::new(),
                schedule_id: String::new(),
                revision: 0,
                envelope_version: String::new(),
                result_record_snapshot_ref: String::new(),
            };
            let verdict_evidence = VerdictEvidence {
                attempt_id: String::new(),
                snapshot_echo: None,
                coverage_limitations: Vec::new(),
                scope_limit_gaps: Vec::new(),
                checks_recorded: 0,
                violations_named: vec![rejection.rule_code.clone()],
                worst_margin_fraction: None,
                binding_constraint: None,
                caveats: Vec::new(),
                findings: Vec::new(),
                fail_closed: None,
                evidence_row_rule_codes: distinct_rule_codes(evidence),
            };
            build_verdict(
                &refs,
                VerdictOutcome::Rejected,
                format!(
                    "rejected at pipeline step {}: rule {}: {}",
                    rejection.step.as_str(),
                    rejection.rule_code,
                    rejection.detail
                ),
                vec![rejection.rule_code.clone()],
                None,
                verdict_evidence,
                now_millis,
            )
        }
        GatePipelineOutcome::Aborted { abort, evidence, .. } => {
            let (trigger, detail) = match abort {
                GateAbort::ModelNotHealthy { findings } => (
                    FailClosedTrigger::ModelStaleOrIncomplete,
                    format!("model health findings: {}", findings.len()),
                ),
                GateAbort::ModelHashMismatch {
                    recorded,
                    computed,
                } => (
                    FailClosedTrigger::ModelHashMismatch,
                    format!("recorded hash '{recorded}', computed '{computed}'"),
                ),
                GateAbort::ProviderError { detail } => {
                    (FailClosedTrigger::ProviderError, detail.clone())
                }
            };
            let outcome = fail_closed_outcome(trigger, &detail);
            // The candidate refs are unrecoverable from the abort shape in
            // general; an empty ref block marks a pipeline-level verdict.
            let refs = CandidateRefs {
                candidate_id: String::new(),
                schedule_id: String::new(),
                revision: 0,
                envelope_version: String::new(),
                result_record_snapshot_ref: String::new(),
            };
            let verdict_evidence = VerdictEvidence {
                attempt_id: String::new(),
                snapshot_echo: None,
                coverage_limitations: Vec::new(),
                scope_limit_gaps: Vec::new(),
                checks_recorded: 0,
                violations_named: Vec::new(),
                worst_margin_fraction: None,
                binding_constraint: None,
                caveats: Vec::new(),
                findings: Vec::new(),
                fail_closed: Some(outcome.clone()),
                evidence_row_rule_codes: distinct_rule_codes(evidence),
            };
            build_verdict(
                &refs,
                fail_closed_outcome_kind(outcome.action),
                outcome.reason.clone(),
                Vec::new(),
                None,
                verdict_evidence,
                now_millis,
            )
        }
        GatePipelineOutcome::Completed { attempt, evidence } => match limit_report {
            Some(limit_report) => route_candidate(RoutingInput {
                attempt,
                limit_report,
                socp_gate,
                fail_closed: None,
                pipeline_evidence: evidence,
                now_millis,
            }),
            None => {
                // A completed attempt without a limit report is an
                // incomplete gate run: fail-closed inconclusive, never a
                // pass.
                let reason = "the gate run completed the power flow but produced \
                              no limit report; no verdict is possible (fail-closed)"
                    .to_string();
                let refs = attempt.candidate_refs.clone();
                let verdict_evidence = VerdictEvidence {
                    attempt_id: attempt.attempt_id.clone(),
                    snapshot_echo: Some(InputSnapshotEcho::from_attempt(attempt)),
                    coverage_limitations: attempt.result().coverage_limitations.clone(),
                    scope_limit_gaps: Vec::new(),
                    checks_recorded: 0,
                    violations_named: Vec::new(),
                    worst_margin_fraction: None,
                    binding_constraint: None,
                    caveats: Vec::new(),
                    findings: Vec::new(),
                    fail_closed: None,
                    evidence_row_rule_codes: distinct_rule_codes(evidence),
                };
                build_verdict(
                    &refs,
                    VerdictOutcome::Inconclusive,
                    reason,
                    Vec::new(),
                    None,
                    verdict_evidence,
                    now_millis,
                )
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Routing internals
// ---------------------------------------------------------------------------

/// Builds the fail-closed verdict for an outcome recorded anywhere in the
/// pipeline.
fn fail_closed_verdict(
    refs: &CandidateRefs,
    outcome: &FailClosedOutcome,
    attempt: Option<&FeasibilityAttemptRecord>,
    pipeline_evidence: &[GateEvidenceRow],
    now_millis: i64,
) -> GateVerdict {
    let evidence = VerdictEvidence {
        attempt_id: attempt.map(|a| a.attempt_id.clone()).unwrap_or_default(),
        snapshot_echo: attempt.map(InputSnapshotEcho::from_attempt),
        coverage_limitations: attempt
            .map(|a| a.result().coverage_limitations.clone())
            .unwrap_or_default(),
        scope_limit_gaps: Vec::new(),
        checks_recorded: 0,
        violations_named: Vec::new(),
        worst_margin_fraction: None,
        binding_constraint: None,
        caveats: Vec::new(),
        findings: Vec::new(),
        fail_closed: Some(outcome.clone()),
        evidence_row_rule_codes: distinct_rule_codes(pipeline_evidence),
    };
    build_verdict(
        refs,
        fail_closed_outcome_kind(outcome.action),
        outcome.reason.clone(),
        Vec::new(),
        None,
        evidence,
        now_millis,
    )
}

/// Maps a fail-closed action to its verdict outcome kind: REJECT →
/// Rejected, HOLD → Held. There is no pass mapping — the match is total.
fn fail_closed_outcome_kind(action: FailClosedAction) -> VerdictOutcome {
    match action {
        FailClosedAction::Reject => VerdictOutcome::Rejected,
        FailClosedAction::Hold => VerdictOutcome::Held,
    }
}

/// Builds the constraining proposal for a single operational thermal
/// violation (D-P4.07-2): a uniform proportional scaling of every applied
/// injection by `limit / measured`, bringing the violated element to its
/// configured operational limit to first order.
fn constrain_proposal(
    check: &LimitCheckRecord,
    attempt: &FeasibilityAttemptRecord,
) -> ConstrainProposal {
    let scale_factor = check.limit / check.measured;
    let proposed_injections = attempt
        .result()
        .applied_injections
        .iter()
        .map(|injection| ProposedInjectionSetpoint {
            injection_id: injection.injection_id.clone(),
            bus_id: injection.bus_id.clone(),
            proposed_p_mw: injection.active_power_mw * scale_factor,
            proposed_q_mvar: injection.reactive_power_mvar * scale_factor,
        })
        .collect();
    ConstrainProposal {
        violated_constraint: check.constraint_name(),
        rule: "uniform proportional scaling of every candidate injection by \
               limit/measured (D-P4.07-2)"
            .to_string(),
        scale_factor,
        proposed_injections,
        predicted_value: check.limit,
        limit: check.limit,
        statement: CONSTRAIN_PROPOSAL_STATEMENT.to_string(),
    }
}

/// The Tier 2 caveats and findings from the boundary gate result
/// (D-P4.09-2): a Tier 2 candidate ALWAYS carries the SOCP exactness
/// caveat on its verdict evidence.
fn tier2_caveats_and_findings(socp_gate: Option<&SocpBoundaryGate>) -> (Vec<String>, Vec<String>) {
    match socp_gate {
        Some(SocpBoundaryGate::Tier2VerifiedForAcCheck {
            caveats, findings, ..
        }) => (caveats.clone(), findings.clone()),
        _ => (Vec::new(), Vec::new()),
    }
}

/// The scope gap labels of a limit report.
fn scope_gap_labels(report: &LimitCheckReport) -> Vec<String> {
    report
        .scope_limit_gaps
        .iter()
        .map(|gap| gap.label.clone())
        .collect()
}

/// Builds the verdict evidence from the routing input.
fn evidence_from(
    input: &RoutingInput,
    caveats: Vec<String>,
    findings: Vec<String>,
) -> VerdictEvidence {
    let diagnostics = &input.limit_report.diagnostics;
    VerdictEvidence {
        attempt_id: input.attempt.attempt_id.clone(),
        snapshot_echo: Some(InputSnapshotEcho::from_attempt(input.attempt)),
        coverage_limitations: input.attempt.result().coverage_limitations.clone(),
        scope_limit_gaps: input
            .limit_report
            .scope_limit_gaps
            .iter()
            .map(|gap| gap.statement.clone())
            .collect(),
        checks_recorded: input.limit_report.checks.len(),
        violations_named: input.limit_report.violated_constraint_names(),
        worst_margin_fraction: diagnostics
            .binding_constraint
            .as_ref()
            .map(|binding| binding.margin_fraction),
        binding_constraint: diagnostics
            .binding_constraint
            .as_ref()
            .map(|binding| binding.statement.clone()),
        caveats,
        findings,
        fail_closed: None,
        evidence_row_rule_codes: distinct_rule_codes(input.pipeline_evidence),
    }
}

/// The distinct rule codes of evidence rows, in first-occurrence order.
fn distinct_rule_codes(rows: &[GateEvidenceRow]) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for row in rows {
        if !codes.contains(&row.rule_code) {
            codes.push(row.rule_code.clone());
        }
    }
    codes
}

/// The verdict builder: assembles the verdict and its completed event,
/// enforcing the non-empty reason and the decision reference on every
/// verdict.
fn build_verdict(
    refs: &CandidateRefs,
    outcome: VerdictOutcome,
    reason: String,
    violated_constraints: Vec<String>,
    constrain_proposal: Option<ConstrainProposal>,
    evidence: VerdictEvidence,
    now_millis: i64,
) -> GateVerdict {
    debug_assert!(!reason.trim().is_empty(), "a verdict reason is never empty");
    let check_id = if evidence.attempt_id.trim().is_empty() {
        format!("gate:{}", refs.candidate_id)
    } else {
        evidence.attempt_id.clone()
    };
    let mut evidence_refs = vec![check_id.clone()];
    if !refs.result_record_snapshot_ref.trim().is_empty() {
        evidence_refs.push(refs.result_record_snapshot_ref.clone());
    }
    if let Some(echo) = &evidence.snapshot_echo {
        evidence_refs.extend(echo.injection_derivation_refs.iter().cloned());
    }
    let (model_id, model_version, model_content_hash) = evidence
        .snapshot_echo
        .as_ref()
        .map(|echo| {
            (
                echo.model_id.clone(),
                echo.model_version.clone(),
                echo.model_content_hash.clone(),
            )
        })
        .unwrap_or_default();
    let completed_event = FeasibilityCheckCompleted {
        check_id,
        candidate_id: refs.candidate_id.clone(),
        schedule_id: refs.schedule_id.clone(),
        revision: refs.revision,
        envelope_version: refs.envelope_version.clone(),
        outcome_code: outcome.as_str().to_string(),
        reason: reason.clone(),
        model_id,
        model_version,
        model_content_hash,
        evidence_refs,
        occurred_at_millis: now_millis,
    };
    GateVerdict {
        decision_ref: ROUTING_DECISION_REF.to_string(),
        outcome,
        outcome_code: outcome.as_str().to_string(),
        reason,
        candidate_refs: refs.clone(),
        violated_constraints,
        constrain_proposal,
        evidence,
        completed_event,
    }
}

// ---------------------------------------------------------------------------
// In-crate unit tests (the integration tests live in tests/verdict.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outcome_vocabulary_is_closed_and_stable() {
        let all = [
            (VerdictOutcome::WithinLimits, "within_limits"),
            (VerdictOutcome::Rejected, "rejected"),
            (VerdictOutcome::Constrained, "constrained"),
            (
                VerdictOutcome::ReturnedForReOptimization,
                "returned_for_re_optimization",
            ),
            (VerdictOutcome::Inconclusive, "inconclusive"),
            (VerdictOutcome::Held, "held"),
        ];
        for (outcome, text) in all {
            assert_eq!(outcome.as_str(), text);
        }
        // Exhaustiveness: no seventh outcome to sneak past the routing
        // rule.
        let exhaustive = match VerdictOutcome::WithinLimits {
            VerdictOutcome::WithinLimits
            | VerdictOutcome::Rejected
            | VerdictOutcome::Constrained
            | VerdictOutcome::ReturnedForReOptimization
            | VerdictOutcome::Inconclusive
            | VerdictOutcome::Held => "closed",
        };
        assert_eq!(exhaustive, "closed");
    }

    #[test]
    fn fail_closed_actions_map_without_a_pass() {
        assert_eq!(
            fail_closed_outcome_kind(FailClosedAction::Reject),
            VerdictOutcome::Rejected
        );
        assert_eq!(
            fail_closed_outcome_kind(FailClosedAction::Hold),
            VerdictOutcome::Held
        );
    }
}
