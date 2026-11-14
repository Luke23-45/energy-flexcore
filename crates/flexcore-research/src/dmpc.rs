// P8.08 — The ADMM operational contract: residuals, termination,
//         infeasibility handling, iteration budget, communication failure,
//         stale participants, and the centralized fallback — plus the DMPC
//         rolling-horizon driver. Ledger `ledger/phase-08-advanced-
//         coordination/ledger_phase_8.md` §P8.08; report trace §13.2,
//         §19.4 (ADMM metrics), §19.2(6); decision record:
//         `ledger/phase-08-advanced-coordination/08_admm_operations.md`.
//
// WHAT THIS IS: the operational layer over the P8.07 formulation
// (`crate::admm`) — the part that makes a distributed run trustworthy:
//
// 1. RESIDUALS (step 1): the primal residual r = ‖a∘x − z‖ and the dual
//    residual s = ρ·‖a∘(z_new − z_old)‖ are computed on EVERY iteration and
//    the recorded pair travels on the result with the tolerances that were
//    used (§9.2 diagnostics — a run that does not name its residuals and
//    termination evidence is not a result).
// 2. TERMINATION (step 1): residual tolerances (the Boyd et al. 2011 §3.3.1
//    absolute + relative form, scalar-block specialization:
//    ε_pri = √n·ε_abs + ε_rel·max(‖a∘x‖, ‖z‖),
//    ε_dual = √n·ε_abs + ε_rel·ρ‖u‖) AND the iteration budget. Exceeding
//    the budget without meeting the tolerances is NON-CONVERGENCE, never a
//    result. No universal iteration-count target exists anywhere (X-8):
//    the budget is a configuration input, and the human evaluation (P8.09)
//    owns the threshold pre-registration.
// 3. INFEASIBILITY (step 2): the coupled problem's feasibility is checked
//    BEFORE any iteration through the EXACT certificate of the supported
//    class (`CoupledProblem::feasibility_certificate` — for one linear
//    coupling over closed boxes the certificate is if-and-only-if). An
//    infeasible problem returns INFEASIBLE_CERTIFICATE with zero iterations
//    — distinguishable, by construction, from non-convergence within budget.
// 4. COMMUNICATION FAILURE / STALE PARTICIPANTS (step 3): every participant
//    answers through an injected worker port; a worker that flags its reply
//    stale or errors is handled by BOUNDED STALENESS — the last accepted
//    value is reused and every reuse is RECORDED, for at most
//    `max_stale_iterations` consecutive iterations; beyond the bound (or
//    with no value to reuse) the run is ABANDONED with the participant and
//    iteration named. A dropped participant produces a recorded failure or
//    a fallback — never a silently partial result.
// 5. CENTRALIZED FALLBACK (step 4): on ANY non-converged outcome the run
//    falls back to the frozen P8.06 baseline registry — the in-crate
//    default wraps `priority_list_dispatch` (baseline/rule-based-priority-
//    list, the frozen `priority-list.v1` rules; NOT an unstudied heuristic),
//    and the composition root implements the fallback port over the Tier 1
//    HiGHS seam for the full supported class. The fallback and its reason
//    are RECORDED on the result; a half-converged distributed result is
//    never presented as final (`final_schedule` returns the fallback or
//    nothing — never the partial ADMM state). The failed ADMM output is
//    never used as an unvalidated warm start for the fallback.
// 6. DMPC ROLLING HORIZON: `plan_horizon` drives the operational run across
//    a demand horizon in fixed interval order, carrying the energy state
//    forward (each energy-limited participant's interval box is capped by
//    remaining energy / interval hours — the recorded inter-temporal
//    coupling of this formulation; the multi-interval objective is NOT
//    jointly optimized, a recorded limitation). The receding-horizon
//    property composes with the platform's EXISTING rolling-horizon seam
//    (`crates/flexcore-application/src/rolling_horizon.rs`, P3.24, §12.1)
//    BY CITATION — the composition root re-invokes `plan_horizon` each
//    scheduling cycle with refreshed SOC state.
// 7. PROPOSALS ONLY (ADR-010): the output type carries NO authorization
//    field and no dispatch path exists; `proposal_for_first_step` packages
//    the first step as a `ResearchProposal` (§13.4 vocabulary,
//    `proposed_schedule`) for the P8.05 independent validation harness —
//    the same protected path as every other candidate.
//
// §19.4 METRICS HONESTY: primal/dual residual, iteration count, and
// communication sensitivity (the staleness/fault machinery above) are
// computed here; RUNTIME and OBJECTIVE GAP are NOT — no wall clock appears
// anywhere in this module (determinism), and the objective gap against the
// frozen baselines is an evaluation-time metric owned by P8.09, computed
// from the frozen artifacts, never asserted in-run.
//
// Control status: research proposals are DATA (ADR-010). Nothing here
// authorizes, issues, approves, or dispatches anything; no safety claim; no
// field authority. Determinism: BTreeMap participant maps (sorted-id
// iteration), fixed update order, no wall clock, no randomness — the same
// input always produces the same result. Canonical units: MW / MWh; SOC a
// fraction in [0, 1] (§10.2).
//
// Honest status: WRITTEN-UNCOMPILED. No cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); every type reference
// was written against source that was read, and every test expectation is
// hand-computed arithmetic stated in the test comments (the toys' optima
// and fixed points are derived on paper, in the comments). The human
// build/test gate (AGENTS.md rule 7) owns the compile/test evidence.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::admm::{
    clamped_zero_init, copy_update, dual_update, local_update_closed_form, CoupledProblem,
    Participant,
};
use crate::baselines::{
    priority_list_dispatch, AssetIntervalDispatch, BASELINE_ID_PRIORITY_LIST, DispatchAsset,
    IntervalDemand, PriorityListInput,
};
use crate::validation::{
    Refusal, ResearchProposal, ResearchProvenance, ResearchResultKind, ResearchValidityWindow,
};

/// The operational-contract version (a change to termination, staleness, or
/// fallback semantics is a NEW version — the P8.06 frozen-version
/// discipline applied to the operational contract).
pub const ADMM_OPERATIONS_VERSION: &str = "flexcore-research/admm-operations.v1";

/// The DMPC rolling-horizon driver version.
pub const DMPC_DRIVER_VERSION: &str = "flexcore-research/dmpc.v1";

/// The decision record that carries the reviewer-signed formulation and its
/// convexity assumptions (the `model_artifact_ref` the proposals cite — the
/// formulation document IS the artifact governing this provider).
pub const ADMM_FORMULATION_RECORD: &str =
    "ledger/phase-08-advanced-coordination/07_admm_dmpc.md";

/// The recorded operational statement (the repo's statement pattern —
/// pinned by tests).
pub const OPERATIONS_STATEMENT: &str = "the ADMM operational contract: primal and \
     dual residuals are computed and recorded on every run together with the tolerances \
     used; termination is residual tolerances (absolute + relative) AND the iteration \
     budget — exceeding the budget is non-convergence, never a result; infeasibility \
     is an exact certificate checked before any iteration, distinguishable from \
     non-convergence by construction; a participant that cannot answer is bounded-\
     stale (last accepted value reused, every reuse recorded) and abandoned beyond the \
     bound; on ANY non-converged outcome the run falls back to the frozen P8.06 \
     centralized baseline and the fallback is recorded — a half-converged distributed \
     result is never presented as final, and the failed ADMM output is never an \
     unvalidated warm start. Proposals only (ADR-010); no dispatch authority; no \
     safety claim.";

/// The recorded rolling-horizon composition seam (BY CITATION — this crate
/// does not depend on flexcore-application).
pub const ROLLING_HORIZON_SEAM: &str =
    "the receding-horizon driver composes with the platform's EXISTING rolling-horizon \
     scheduling seam — crates/flexcore-application/src/rolling_horizon.rs (P3.24, \
     report §12.1: plan_rolling_horizon_roll / project_rolling_schedule / \
     rolling_horizon_schedule_proposed_publication) — BY CITATION: this crate does not \
     depend on flexcore-application; the composition root re-invokes plan_horizon each \
     scheduling cycle with refreshed SOC/energy state and routes every proposal through \
     the protected path; the RETURN/re-enqueue discipline is the platform seam's, not \
     duplicated here";

/// The recorded fallback seam (the frozen P8.06 registry, never an unstudied
/// heuristic).
pub const FALLBACK_SEAM: &str =
    "centralized fallback = the frozen P8.06 baseline registry (crate::baselines): the \
     in-crate default PriorityListFallback wraps priority_list_dispatch \
     (baseline/rule-based-priority-list, the frozen priority-list.v1 rules) for \
     demand-floor problems (every a_k = -1); the composition root implements \
     CentralizedFallbackSolver over the Tier 1 HiGHS seam (baseline/highs-tier1-lp) \
     for the full supported class. NEVER an unstudied heuristic; never the failed \
     ADMM output as an unvalidated warm start; a refused fallback is a recorded \
     failure, never a silently partial schedule";

// ---------------------------------------------------------------------------
// Termination criteria (P8.08 step 1)
// ---------------------------------------------------------------------------

/// The termination criteria: residual tolerances AND the iteration budget.
/// NOT a universal target (X-8): these are configuration inputs recorded on
/// every result; the human evaluation (P8.09) owns threshold
/// pre-registration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TerminationCriteria {
    /// The absolute residual tolerance ε_abs.
    pub epsilon_abs: f64,
    /// The relative residual tolerance ε_rel.
    pub epsilon_rel: f64,
    /// The iteration budget (≥ 1). Exhausting it without meeting the
    /// tolerances is NON-CONVERGENCE.
    pub max_iterations: u32,
    /// The bounded-staleness limit: how many CONSECUTIVE iterations a
    /// participant may be stale (flagged-stale or unresponsive) while its
    /// last accepted value is reused. 0 = no staleness tolerated: the first
    /// unanswered iteration abandons the run.
    pub max_stale_iterations: u32,
}

impl TerminationCriteria {
    /// The recorded standard configuration.
    pub fn standard() -> TerminationCriteria {
        TerminationCriteria {
            epsilon_abs: 1.0e-6,
            epsilon_rel: 1.0e-6,
            max_iterations: 1000,
            max_stale_iterations: 2,
        }
    }

    /// Fail-closed validation (named errors; a run with unvalid criteria is
    /// never started).
    pub fn validate(&self) -> Result<(), CriteriaError> {
        for (field, value) in [("epsilon_abs", self.epsilon_abs), ("epsilon_rel", self.epsilon_rel)]
        {
            if !value.is_finite() || value <= 0.0 {
                return Err(CriteriaError::NonPositiveEpsilon { field, value });
            }
        }
        if self.max_iterations == 0 {
            return Err(CriteriaError::ZeroIterationBudget);
        }
        Ok(())
    }
}

/// The named criteria errors (PartialEq only: the errors carry f64 values,
/// and `Eq` is not derivable over floats).
#[derive(Debug, Clone, PartialEq)]
pub enum CriteriaError {
    /// A tolerance is not finite or not positive.
    NonPositiveEpsilon { field: &'static str, value: f64 },
    /// The iteration budget is zero (a run that can never iterate).
    ZeroIterationBudget,
}

impl CriteriaError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            CriteriaError::NonPositiveEpsilon { .. } => "NON_POSITIVE_EPSILON",
            CriteriaError::ZeroIterationBudget => "ZERO_ITERATION_BUDGET",
        }
    }
}

impl std::fmt::Display for CriteriaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CriteriaError::NonPositiveEpsilon { field, value } => {
                write!(f, "termination criterion '{field}' is {value}; tolerances must be \
                           finite and positive")
            }
            CriteriaError::ZeroIterationBudget => {
                write!(f, "the iteration budget is zero; a run that can never iterate is \
                           a configuration error")
            }
        }
    }
}

impl std::error::Error for CriteriaError {}

// ---------------------------------------------------------------------------
// The participant port (communication failure happens HERE, at the seam)
// ---------------------------------------------------------------------------

/// The coordinator's request to one participant for one iteration's local
/// update: the price context s = z_k + u_k (the participant solves
/// min f_k(x) + (ρ/2)(a_k x − s)² over its own box — it holds its OWN
/// capability; the coordinator never needs it).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocalSolveRequest {
    /// The participant this request addresses.
    pub participant_id: String,
    /// The iteration the request belongs to (1-based).
    pub iteration: u32,
    /// The ADMM penalty ρ (> 0).
    pub rho: f64,
    /// The price shift s = z_k + u_k for this iteration.
    pub price_shift: f64,
}

/// One participant's local reply. `stale: true` is the HONEST flag of a
/// worker that could not recompute and is re-serving its previous state —
/// the coordinator discards a flagged reply and applies the bounded-
/// staleness policy (never silently accepts a flagged value as fresh).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocalSolveReply {
    /// The participant's local x_k, MW.
    pub value_mw: f64,
    /// Whether this reply is a stale re-serve (honest flag; a flagged reply
    /// is discarded, the last accepted value is reused, and the reuse is
    /// recorded on the run result).
    pub stale: bool,
}

/// A local-solve failure (the participant did not answer at all).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalSolveError {
    /// The participant that failed to answer.
    pub participant_id: String,
    /// The recorded failure detail.
    pub detail: String,
}

impl std::fmt::Display for LocalSolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "participant {participant_id} did not answer its local solve: {detail}",
            participant_id = self.participant_id,
            detail = self.detail
        )
    }
}

impl std::error::Error for LocalSolveError {}

/// The participant port (the composition seam where communication failure
/// lives): the composition root implements this over the real
/// participants/worker processes; the in-crate reference implementation is
/// the closed-form local solve of the P8.07 formulation.
pub trait AdmmLocalWorker {
    fn solve_local(&self, request: &LocalSolveRequest) -> Result<LocalSolveReply, LocalSolveError>;
}

/// The in-crate reference worker: the exact closed-form local update
/// (`admm::local_update_closed_form`) over its own participant profile.
/// Never stale by construction.
pub struct ClosedFormLocalWorker {
    /// The participant profile this worker holds (its OWN capability).
    pub participant: Participant,
}

impl AdmmLocalWorker for ClosedFormLocalWorker {
    fn solve_local(
        &self,
        request: &LocalSolveRequest,
    ) -> Result<LocalSolveReply, LocalSolveError> {
        let value_mw = local_update_closed_form(&self.participant, request.rho, request.price_shift);
        Ok(LocalSolveReply {
            value_mw,
            stale: false,
        })
    }
}

// ---------------------------------------------------------------------------
// The centralized fallback (P8.08 step 4 — the frozen P8.06 registry)
// ---------------------------------------------------------------------------

/// WHY the fallback was invoked (recorded on the outcome).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    /// The coupled problem's feasibility certificate failed (before any
    /// iteration).
    InfeasibleCertificate,
    /// The iteration budget was exhausted without meeting the tolerances.
    NonConvergedBudgetExhausted,
    /// A participant was abandoned (bounded staleness exceeded).
    ParticipantAbandoned,
}

impl FallbackReason {
    /// The recorded text form (provenance/diagnostics field).
    pub fn as_str(self) -> &'static str {
        match self {
            FallbackReason::InfeasibleCertificate => "infeasible_certificate",
            FallbackReason::NonConvergedBudgetExhausted => "non_converged_budget_exhausted",
            FallbackReason::ParticipantAbandoned => "participant_abandoned",
        }
    }
}

/// The recorded fallback outcome: WHICH frozen baseline answered, why, and
/// the honest schedule it produced (deficits included, never silently
/// filled — the frozen rule set's own honesty).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FallbackOutcome {
    /// The baseline id of the frozen P8.06 registry record that answered.
    pub baseline_id: &'static str,
    /// The baseline version (the frozen-version discipline).
    pub baseline_version: String,
    /// Why the fallback was invoked.
    pub reason: FallbackReason,
    /// The fallback's dispatch, per committed asset, in the baseline's fixed
    /// order (uncommitted participants are 0 — never silently filled).
    pub dispatches: Vec<AssetIntervalDispatch>,
    /// The total supplied, MW.
    pub supplied_mw: f64,
    /// The deficit (demand − supplied, ≥ 0), MW — an honest recorded
    /// outcome, never silently filled.
    pub deficit_mw: f64,
}

/// The named fallback errors: a REFUSED fallback is a recorded failure on
/// the run result (`fallback_error`), never a silently partial schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackError {
    /// The frozen in-crate fallback serves demand-floor couplings only; the
    /// composition root implements the port over the Tier 1 seam for the
    /// full supported class.
    ClassNotSupportedByFallback { detail: String },
    /// The frozen baseline refused its input (named by the baseline's own
    /// validation, propagated verbatim).
    BaselineInputInvalid { detail: String },
}

impl FallbackError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            FallbackError::ClassNotSupportedByFallback { .. } => "FALLBACK_CLASS_NOT_SUPPORTED",
            FallbackError::BaselineInputInvalid { .. } => "FALLBACK_BASELINE_INPUT_INVALID",
        }
    }
}

impl std::fmt::Display for FallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackError::ClassNotSupportedByFallback { detail } => {
                write!(f, "the frozen fallback does not support this coupling class: {detail}")
            }
            FallbackError::BaselineInputInvalid { detail } => {
                write!(f, "the frozen baseline refused its input: {detail}")
            }
        }
    }
}

impl std::error::Error for FallbackError {}

/// The centralized fallback port. The composition root implements it over
/// the frozen P8.06 baseline registry (see [`FALLBACK_SEAM`]).
pub trait CentralizedFallbackSolver {
    fn fallback_for(
        &self,
        problem: &CoupledProblem,
        reason: &FallbackReason,
    ) -> Result<FallbackOutcome, FallbackError>;
}

/// The in-crate frozen fallback: wraps `baselines::priority_list_dispatch`
/// (baseline/rule-based-priority-list — the P8.06 frozen registry's rule
/// set, NOT an unstudied heuristic). Demand-floor couplings only (every
/// a_k = −1, b = −d): the rule set serves the demand d, records deficits
/// honestly, and orders assets by (priority, asset_id) — here priority 0
/// for all participants, i.e. pure lexicographic id order (deterministic,
/// recorded).
pub struct PriorityListFallback;

impl CentralizedFallbackSolver for PriorityListFallback {
    fn fallback_for(
        &self,
        problem: &CoupledProblem,
        reason: &FallbackReason,
    ) -> Result<FallbackOutcome, FallbackError> {
        for participant in problem.participants() {
            if participant.coupling_coefficient != -1.0 {
                return Err(FallbackError::ClassNotSupportedByFallback {
                    detail: format!(
                        "participant {} has coupling coefficient {}; the in-crate frozen \
                         fallback (baseline/rule-based-priority-list) serves demand-floor \
                         couplings (a_k = -1) only — the composition root implements the \
                         fallback over the Tier 1 HiGHS seam for the full supported class",
                        participant.participant_id, participant.coupling_coefficient
                    ),
                });
            }
        }
        let demand_mw = -problem.coupling_bound();
        let input = PriorityListInput {
            interval_hours: problem.interval_hours(),
            intervals: vec![IntervalDemand {
                net_demand_mw: demand_mw,
            }],
            assets: problem
                .participants()
                .iter()
                .map(|p| DispatchAsset {
                    asset_id: p.participant_id.clone(),
                    priority: 0,
                    p_min_mw: p.p_min_mw,
                    p_max_mw: p.p_max_mw,
                    // The interval problem's box ALREADY carries the DMPC
                    // energy cap (plan_horizon caps p_max by remaining
                    // energy), so the frozen rule set's own energy logic is
                    // not double-applied here.
                    energy_capacity_mwh: None,
                    soc_fraction: None,
                })
                .collect(),
        };
        let dispatch = priority_list_dispatch(&input).map_err(|error| {
            FallbackError::BaselineInputInvalid {
                detail: error.to_string(),
            }
        })?;
        let interval = &dispatch.intervals[0];
        Ok(FallbackOutcome {
            baseline_id: BASELINE_ID_PRIORITY_LIST,
            baseline_version: dispatch.baseline_version.clone(),
            reason: *reason,
            dispatches: interval.dispatched.clone(),
            supplied_mw: interval.supplied_mw,
            deficit_mw: interval.deficit_mw,
        })
    }
}

// ---------------------------------------------------------------------------
// The run result (residuals, termination, staleness, fallback — recorded)
// ---------------------------------------------------------------------------

/// The outcome of ONE operational run. The status vocabulary is CLOSED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Both residual tolerances met within the iteration budget.
    Converged,
    /// The budget was exhausted without meeting the tolerances
    /// (NON-CONVERGENCE — never a result; the fallback is recorded).
    NonConvergedBudgetExhausted,
    /// The exact feasibility certificate failed before any iteration
    /// (distinguishable from non-convergence BY CONSTRUCTION — no
    /// iterations ran).
    InfeasibleCertificate,
    /// A participant was abandoned: bounded staleness exceeded mid-run
    /// (the participant and iteration are named on the result).
    AbandonedParticipantUnresponsive,
}

impl RunStatus {
    /// The stable machine-readable status name (the §9.2 termination_status
    /// source — never the bare "ok").
    pub fn name(self) -> &'static str {
        match self {
            RunStatus::Converged => "CONVERGED",
            RunStatus::NonConvergedBudgetExhausted => "NON_CONVERGED_BUDGET_EXHAUSTED",
            RunStatus::InfeasibleCertificate => "INFEASIBLE_CERTIFICATE",
            RunStatus::AbandonedParticipantUnresponsive => "ABANDONED_PARTICIPANT_UNRESPONSIVE",
        }
    }

    /// The fallback reason this status triggers (None for Converged — a
    /// converged run needs no fallback).
    pub fn fallback_reason(self) -> Option<FallbackReason> {
        match self {
            RunStatus::Converged => None,
            RunStatus::NonConvergedBudgetExhausted => Some(FallbackReason::NonConvergedBudgetExhausted),
            RunStatus::InfeasibleCertificate => Some(FallbackReason::InfeasibleCertificate),
            RunStatus::AbandonedParticipantUnresponsive => Some(FallbackReason::ParticipantAbandoned),
        }
    }
}

/// One recorded staleness event (bounded-staleness reuse or the abandoning
/// event — every reuse is visible, never silent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleEvent {
    /// The iteration the event happened in (1-based).
    pub iteration: u32,
    /// The participant involved.
    pub participant_id: String,
    /// Whether the participant's last accepted value was reused.
    pub reused_previous_value: bool,
    /// The recorded detail (flag text or error prose).
    pub detail: String,
}

/// THE schedule a consumer may act on — the bypass proof for "never a
/// half-converged distributed result presented as final": a converged run
/// yields its distributed solution; any other outcome yields the fallback's
/// schedule or NOTHING (a recorded failure). There is no constructor for a
/// schedule from a non-converged ADMM state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalSchedule {
    /// The converged distributed solution (the run's status is Converged).
    ConvergedAdmm {
        /// Per-participant setpoints, MW (sorted keys).
        x: BTreeMap<String, f64>,
    },
    /// The centralized fallback's schedule (the frozen baseline's, with its
    /// own honest deficit accounting).
    CentralizedFallback { outcome: FallbackOutcome },
}

/// The result of one operational run (P8.08 step 1: residuals and
/// termination evidence travel on the result — §9.2 diagnostics).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdmmRunResult {
    /// The operational-contract version that produced this result.
    pub operations_version: String,
    /// The closed-vocabulary status.
    pub status: RunStatus,
    /// Iterations actually run (0 for an infeasibility certificate).
    pub iterations_run: u32,
    /// The final primal residual r = ‖a∘x − z‖ (0.0 when no iteration ran).
    pub primal_residual: f64,
    /// The final dual residual s = ρ·‖a∘(z_new − z_old)‖ (0.0 when no
    /// iteration ran).
    pub dual_residual: f64,
    /// The primal tolerance the comparison used.
    pub epsilon_primal: f64,
    /// The dual tolerance the comparison used.
    pub epsilon_dual: f64,
    /// The final distributed x per participant (DIAGNOSTICS: for a
    /// non-converged/abandoned run this is the partial state and is NEVER
    /// presented as a schedule — [`AdmmRunResult::final_schedule`] is the
    /// only schedule path).
    pub distributed_x_mw: BTreeMap<String, f64>,
    /// The criteria this run used (recorded, §9.2).
    pub termination: TerminationCriteria,
    /// Whether any bounded-staleness reuse happened (visible, never silent).
    pub used_stale_values: bool,
    /// Every staleness event, in order.
    pub stale_events: Vec<StaleEvent>,
    /// The participant that caused an abandonment, when abandoned.
    pub abandoned_participant: Option<String>,
    /// The recorded fallback outcome (present whenever the fallback was
    /// invoked and succeeded).
    pub fallback: Option<FallbackOutcome>,
    /// The recorded fallback FAILURE (present when the fallback itself was
    /// refused/failed — a named recorded failure, never a silent one).
    pub fallback_error: Option<String>,
}

impl AdmmRunResult {
    /// THE schedule path (the bypass proof): a converged run yields its
    /// distributed solution; any non-converged outcome yields the fallback's
    /// schedule or NOTHING. There is no path from a partial ADMM state to a
    /// schedule.
    pub fn final_schedule(&self) -> Option<FinalSchedule> {
        if let Some(outcome) = &self.fallback {
            return Some(FinalSchedule::CentralizedFallback {
                outcome: outcome.clone(),
            });
        }
        if self.fallback_error.is_some() {
            // The fallback itself failed: a recorded failure, never a
            // schedule.
            return None;
        }
        if self.status == RunStatus::Converged {
            return Some(FinalSchedule::ConvergedAdmm {
                x: self.distributed_x_mw.clone(),
            });
        }
        None
    }

    /// Whether the final schedule (if any) came from the centralized
    /// fallback (recorded so the consumer cannot mistake the source).
    pub fn schedule_is_from_fallback(&self) -> bool {
        self.fallback.is_some()
    }
}

/// The named coordination errors (fail-closed construction — a run with
/// broken wiring is never started). PartialEq only: the errors carry f64
/// values, and `Eq` is not derivable over floats.
#[derive(Debug, Clone, PartialEq)]
pub enum CoordinationError {
    /// No worker is wired for a participant (coverage is validated at
    /// construction; a silent gap would look like a mid-run drop).
    WorkerCoverageMissing { participant_id: String },
    /// The ADMM penalty ρ is not finite or not positive.
    InvalidPenalty { value: f64 },
    /// The termination criteria are invalid.
    InvalidCriteria(CriteriaError),
    /// The horizon id is empty.
    EmptyHorizonId,
    /// The horizon's interval duration is not finite positive hours.
    NonPositiveIntervalHours { value: f64 },
    /// A horizon demand is not finite.
    NonFiniteDemand { index: usize },
    /// A horizon demand is negative (the demand-floor class serves demands,
    /// never absorbs them).
    NegativeDemand { index: usize, value: f64 },
    /// The horizon has no participants.
    EmptyParticipantSet,
    /// The coupled problem refused its participants (the named error is
    /// propagated as detail).
    ProblemInvalid { detail: String },
}

impl CoordinationError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            CoordinationError::WorkerCoverageMissing { .. } => "WORKER_COVERAGE_MISSING",
            CoordinationError::InvalidPenalty { .. } => "INVALID_PENALTY",
            CoordinationError::InvalidCriteria(_) => "INVALID_CRITERIA",
            CoordinationError::EmptyHorizonId => "EMPTY_HORIZON_ID",
            CoordinationError::NonPositiveIntervalHours { .. } => "NON_POSITIVE_INTERVAL_HOURS",
            CoordinationError::NonFiniteDemand { .. } => "NON_FINITE_DEMAND",
            CoordinationError::NegativeDemand { .. } => "NEGATIVE_DEMAND",
            CoordinationError::EmptyParticipantSet => "EMPTY_PARTICIPANT_SET",
            CoordinationError::ProblemInvalid { .. } => "PROBLEM_INVALID",
        }
    }
}

impl std::fmt::Display for CoordinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinationError::WorkerCoverageMissing { participant_id } => {
                write!(f, "no worker is wired for participant {participant_id}; a coverage \
                           gap is refused at construction, never discovered mid-run")
            }
            CoordinationError::InvalidPenalty { value } => {
                write!(f, "the ADMM penalty rho is {value}; it must be finite and positive")
            }
            CoordinationError::InvalidCriteria(error) => write!(f, "{error}"),
            CoordinationError::EmptyHorizonId => write!(f, "the horizon id is empty"),
            CoordinationError::NonPositiveIntervalHours { value } => {
                write!(f, "interval_hours must be finite positive hours; got {value}")
            }
            CoordinationError::NonFiniteDemand { index } => {
                write!(f, "interval {index}: demand is not finite")
            }
            CoordinationError::NegativeDemand { index, value } => {
                write!(f, "interval {index}: demand {value} MW is negative; the demand-floor \
                           class serves demands, never absorbs them")
            }
            CoordinationError::EmptyParticipantSet => {
                write!(f, "the horizon has no participants")
            }
            CoordinationError::ProblemInvalid { detail } => {
                write!(f, "the coupled problem refused its participants: {detail}")
            }
        }
    }
}

impl std::error::Error for CoordinationError {}

// ---------------------------------------------------------------------------
// The operational run (P8.08 steps 1–4)
// ---------------------------------------------------------------------------

/// The operational ADMM coordination: the problem, the injected worker port
/// per participant, the injected centralized fallback, and the criteria.
/// Construction is fail-closed (coverage + criteria); `run` records every
/// outcome on the result.
pub struct AdmmCoordination<'a> {
    /// The coupled problem (P8.07 formulation).
    pub problem: &'a CoupledProblem,
    /// The injected worker per participant (sorted-id keys; coverage
    /// validated at construction).
    pub workers: BTreeMap<String, &'a dyn AdmmLocalWorker>,
    /// The injected centralized fallback (the frozen P8.06 registry seam).
    pub fallback: &'a dyn CentralizedFallbackSolver,
    /// The termination + staleness criteria.
    pub criteria: TerminationCriteria,
}

impl<'a> AdmmCoordination<'a> {
    /// Constructs the coordination, validating worker coverage and criteria
    /// fail-closed BEFORE any run (a coverage gap is refused here, never
    /// discovered mid-run as a fake "participant drop").
    pub fn new(
        problem: &'a CoupledProblem,
        workers: BTreeMap<String, &'a dyn AdmmLocalWorker>,
        fallback: &'a dyn CentralizedFallbackSolver,
        criteria: TerminationCriteria,
    ) -> Result<AdmmCoordination<'a>, CoordinationError> {
        criteria.validate()
            .map_err(CoordinationError::InvalidCriteria)?;
        for participant_id in problem.participant_ids() {
            if !workers.contains_key(&participant_id) {
                return Err(CoordinationError::WorkerCoverageMissing { participant_id });
            }
        }
        Ok(AdmmCoordination {
            problem,
            workers,
            fallback,
            criteria,
        })
    }

    /// Runs the operational contract (P8.08 steps 1–4, in the recorded
    /// order): feasibility certificate FIRST (infeasibility is a
    /// certificate, zero iterations), then the exact two-block ADMM
    /// iterations with per-iteration residuals and the bounded-staleness
    /// policy, then — on ANY non-converged outcome — the recorded
    /// centralized fallback. Deterministic: sorted-id iteration, fixed
    /// update order, no wall clock, no randomness.
    pub fn run(&self, rho: f64) -> Result<AdmmRunResult, CoordinationError> {
        if !rho.is_finite() || rho <= 0.0 {
            return Err(CoordinationError::InvalidPenalty { value: rho });
        }
        let criteria = &self.criteria;
        let ids = self.problem.participant_ids();
        let n = ids.len();
        let a = self.problem.coupling_coefficients();
        let participants: Vec<&Participant> = self.problem.participants();

        // Deterministic zero init: x_k is the box-clamped zero (the "last
        // accepted value" the staleness policy reuses), z = 0, u = 0.
        let mut x: Vec<f64> = participants.iter().map(|p| clamped_zero_init(p)).collect();
        let mut status = RunStatus::NonConvergedBudgetExhausted;
        let mut iterations_run: u32 = 0;
        let mut primal_residual = 0.0;
        let mut dual_residual = 0.0;
        let mut epsilon_primal = 0.0;
        let mut epsilon_dual = 0.0;
        let mut stale_events: Vec<StaleEvent> = Vec::new();
        let mut used_stale_values = false;
        let mut abandoned_participant: Option<String> = None;

        // STEP 2 FIRST — the exact feasibility certificate: an infeasible
        // problem is INFEASIBLE_CERTIFICATE with zero iterations, never a
        // timeout guess and never confused with non-convergence.
        let certificate = self.problem.feasibility_certificate();
        if !certificate.feasible {
            status = RunStatus::InfeasibleCertificate;
        } else {
            let mut z: Vec<f64> = vec![0.0; n];
            let mut u: Vec<f64> = vec![0.0; n];
            let mut consecutive_stale = vec![0u32; n];
            let mut last_accepted = x.clone();

            for iteration in 1..=criteria.max_iterations {
                iterations_run = iteration;

                // STEP 3 — the x-update through the injected workers, sorted
                // order, with the bounded-staleness policy: a flagged-stale
                // or unresponsive worker costs one staleness unit; the last
                // accepted value is reused and the reuse is RECORDED; beyond
                // the bound the run is ABANDONED (participant + iteration
                // named) — never a silently partial result.
                let mut abandoned = false;
                for (k, participant_id) in ids.iter().enumerate() {
                    let worker = self
                        .workers
                        .get(participant_id)
                        .copied()
                        .expect("worker coverage validated at construction");
                    let request = LocalSolveRequest {
                        participant_id: participant_id.clone(),
                        iteration,
                        rho,
                        price_shift: z[k] + u[k],
                    };
                    match worker.solve_local(&request) {
                        Ok(reply) if !reply.stale => {
                            x[k] = reply.value_mw;
                            last_accepted[k] = reply.value_mw;
                            consecutive_stale[k] = 0;
                        }
                        Ok(_flagged) => {
                            if consecutive_stale[k] + 1 > criteria.max_stale_iterations {
                                status = RunStatus::AbandonedParticipantUnresponsive;
                                abandoned_participant = Some(participant_id.clone());
                                stale_events.push(StaleEvent {
                                    iteration,
                                    participant_id: participant_id.clone(),
                                    reused_previous_value: true,
                                    detail: "stale beyond the bounded-staleness limit; \
                                             run abandoned"
                                        .to_string(),
                                });
                                abandoned = true;
                                break;
                            }
                            consecutive_stale[k] += 1;
                            used_stale_values = true;
                            x[k] = last_accepted[k];
                            stale_events.push(StaleEvent {
                                iteration,
                                participant_id: participant_id.clone(),
                                reused_previous_value: true,
                                detail: "worker flagged its reply stale; last accepted value \
                                         reused"
                                    .to_string(),
                            });
                        }
                        Err(error) => {
                            if consecutive_stale[k] + 1 > criteria.max_stale_iterations {
                                status = RunStatus::AbandonedParticipantUnresponsive;
                                abandoned_participant = Some(participant_id.clone());
                                stale_events.push(StaleEvent {
                                    iteration,
                                    participant_id: participant_id.clone(),
                                    reused_previous_value: true,
                                    detail: format!("worker unresponsive: {error}; run \
                                                     abandoned"),
                                });
                                abandoned = true;
                                break;
                            }
                            consecutive_stale[k] += 1;
                            used_stale_values = true;
                            x[k] = last_accepted[k];
                            stale_events.push(StaleEvent {
                                iteration,
                                participant_id: participant_id.clone(),
                                reused_previous_value: true,
                                detail: format!("worker error within the staleness bound: \
                                                 {error}"),
                            });
                        }
                    }
                }
                if abandoned {
                    break;
                }

                // The z-update (exact block minimizer) and the u-update
                // (scaled dual) — the coordinator's aggregation math, from
                // the P8.07 formulation.
                let v: Vec<f64> = (0..n).map(|k| a[k] * x[k] - u[k]).collect();
                let z_prev = z.clone();
                z = copy_update(&v, self.problem.coupling_bound());
                u = dual_update(&u, &z, &a, &x);

                // STEP 1 — residuals on EVERY iteration; the recorded pair
                // is the final iteration's, with the tolerances used.
                let n_f = n as f64;
                let ax_norm = (0..n)
                    .map(|k| (a[k] * x[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                let z_norm = (0..n).map(|k| z[k].powi(2)).sum::<f64>().sqrt();
                let u_norm = (0..n).map(|k| u[k].powi(2)).sum::<f64>().sqrt();
                primal_residual = (0..n)
                    .map(|k| (a[k] * x[k] - z[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                dual_residual = rho
                    * (0..n)
                        .map(|k| (a[k] * (z[k] - z_prev[k])).powi(2))
                        .sum::<f64>()
                        .sqrt();
                epsilon_primal = n_f.sqrt() * criteria.epsilon_abs
                    + criteria.epsilon_rel * ax_norm.max(z_norm);
                epsilon_dual = n_f.sqrt() * criteria.epsilon_abs
                    + criteria.epsilon_rel * (rho * u_norm);
                if primal_residual <= epsilon_primal && dual_residual <= epsilon_dual {
                    status = RunStatus::Converged;
                    break;
                }
            }
        }

        let distributed_x_mw: BTreeMap<String, f64> = ids
            .iter()
            .cloned()
            .zip(x.iter().cloned())
            .collect();

        let mut result = AdmmRunResult {
            operations_version: ADMM_OPERATIONS_VERSION.to_string(),
            status,
            iterations_run,
            primal_residual,
            dual_residual,
            epsilon_primal,
            epsilon_dual,
            distributed_x_mw,
            termination: criteria.clone(),
            used_stale_values,
            stale_events,
            abandoned_participant,
            fallback: None,
            fallback_error: None,
        };

        // STEP 4 — the centralized fallback: on ANY non-converged outcome the
        // run falls back to the frozen P8.06 baseline and RECORDS it. The
        // failed ADMM output is not passed to the fallback at all (no
        // unvalidated warm start — the recorded failure mode of P8.08).
        if let Some(reason) = status.fallback_reason() {
            match self.fallback.fallback_for(self.problem, &reason) {
                Ok(outcome) => result.fallback = Some(outcome),
                Err(error) => result.fallback_error = Some(error.to_string()),
            }
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// The DMPC rolling-horizon driver
// ---------------------------------------------------------------------------

/// The DMPC horizon input: a demand horizon over the demand-floor coupling
/// class (every a_k = −1; the interval demand d_t becomes b_t = −d_t), the
/// participants (energy-limited ones carry their capacity + SOC), and the
/// operational configuration. The receding-horizon property is the
/// composition root's: re-invoke `plan_horizon` each scheduling cycle with
/// refreshed SOC state (see [`ROLLING_HORIZON_SEAM`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DmpcHorizonInput {
    /// The horizon id (provenance; unique within the producer's stream).
    pub horizon_id: String,
    /// The interval duration, hours (> 0).
    pub interval_hours: f64,
    /// The per-interval aggregate demand to serve, MW (≥ 0, finite), in
    /// horizon order. An empty horizon is legal and plans nothing.
    pub interval_demand_mw: Vec<f64>,
    /// The participants (validated fail-closed; energy-limited ones carry
    /// capacity + SOC).
    pub participants: Vec<Participant>,
    /// The ADMM penalty ρ (> 0).
    pub rho: f64,
    /// The termination + staleness criteria (recorded on every run).
    pub criteria: TerminationCriteria,
}

impl DmpcHorizonInput {
    /// Fail-closed validation (named errors) BEFORE any interval runs; the
    /// participants are validated eagerly through the same constructor the
    /// per-interval problems use.
    pub fn validate(&self) -> Result<(), CoordinationError> {
        if self.horizon_id.trim().is_empty() {
            return Err(CoordinationError::EmptyHorizonId);
        }
        if !self.interval_hours.is_finite() || self.interval_hours <= 0.0 {
            return Err(CoordinationError::NonPositiveIntervalHours {
                value: self.interval_hours,
            });
        }
        if !self.rho.is_finite() || self.rho <= 0.0 {
            return Err(CoordinationError::InvalidPenalty { value: self.rho });
        }
        for (index, demand) in self.interval_demand_mw.iter().enumerate() {
            if !demand.is_finite() {
                return Err(CoordinationError::NonFiniteDemand { index });
            }
            if *demand < 0.0 {
                return Err(CoordinationError::NegativeDemand {
                    index,
                    value: *demand,
                });
            }
        }
        if self.participants.is_empty() {
            return Err(CoordinationError::EmptyParticipantSet);
        }
        CoupledProblem::new(
            &format!("{}.validation", self.horizon_id),
            0.0,
            self.interval_hours,
            self.participants.clone(),
        )
        .map_err(|error| CoordinationError::ProblemInvalid {
            detail: error.to_string(),
        })?;
        self.criteria
            .validate()
            .map_err(CoordinationError::InvalidCriteria)?;
        Ok(())
    }
}

/// One interval's outcome: the operational run result plus the energy the
/// FINAL schedule adopted (the rolling energy state's update).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntervalOutcome {
    /// The interval index (horizon order).
    pub index: usize,
    /// The interval's demand, MW.
    pub demand_mw: f64,
    /// The operational run result (status, residuals, staleness, fallback).
    pub run: AdmmRunResult,
    /// The energy adopted from the FINAL schedule this interval, MWh, per
    /// participant (0.0 for a participant the schedule did not commit; all
    /// zeros when no schedule was adopted — a recorded failure adopts
    /// nothing).
    pub energy_used_mwh: BTreeMap<String, f64>,
}

/// The DMPC plan: per-interval outcomes, the FIRST STEP schedule (the
/// receding-horizon committed step — the only step the platform consumes per
/// cycle), and the fallback flag. Proposals only (ADR-010): nothing here is
/// a dispatch decision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DmpcPlan {
    /// The DMPC driver version that produced this plan.
    pub dmpc_version: String,
    /// The horizon id (echoed).
    pub horizon_id: String,
    /// The interval duration, hours.
    pub interval_hours: f64,
    /// The per-interval outcomes, in horizon order.
    pub intervals: Vec<IntervalOutcome>,
    /// The first interval's final schedule (the committed receding-horizon
    /// step; None when the first interval produced no schedule — a recorded
    /// failure).
    pub first_step_schedule: Option<FinalSchedule>,
    /// Whether ANY interval fell back to the centralized baseline (visible
    /// at plan level too).
    pub any_fallback_used: bool,
}

/// Plans the horizon: fixed interval order; each interval's participants get
/// their box upper bound capped by remaining energy / interval hours (the
/// recorded inter-temporal coupling; when the remaining energy cannot
/// sustain a participant's declared minimum, its box collapses to [0, 0] —
/// the analogue of the frozen baseline's skip rule); the FINAL schedule of
/// each interval (converged ADMM or recorded fallback) updates the rolling
/// energy state; a recorded failure adopts nothing. Deterministic:
/// sorted-id iteration everywhere, no wall clock, no randomness.
pub fn plan_horizon(
    input: &DmpcHorizonInput,
    fallback: &dyn CentralizedFallbackSolver,
) -> Result<DmpcPlan, CoordinationError> {
    input.validate()?;

    // The rolling energy state: remaining energy per energy-limited
    // participant (MWh), starting at capacity × SOC (§10.2).
    let mut remaining_mwh: BTreeMap<String, f64> = input
        .participants
        .iter()
        .filter(|p| p.energy_capacity_mwh.is_some())
        .map(|p| {
            (
                p.participant_id.clone(),
                p.energy_capacity_mwh.unwrap_or(0.0) * p.soc_fraction.unwrap_or(1.0),
            )
        })
        .collect();

    let mut interval_outcomes: Vec<IntervalOutcome> =
        Vec::with_capacity(input.interval_demand_mw.len());

    for (index, &demand_mw) in input.interval_demand_mw.iter().enumerate() {
        // Per-interval participants: the energy cap enters through the box.
        let mut interval_participants: Vec<Participant> = input.participants.clone();
        for p in &mut interval_participants {
            if p.energy_capacity_mwh.is_some() {
                let remaining = remaining_mwh
                    .get(&p.participant_id)
                    .copied()
                    .unwrap_or(0.0);
                let energy_cap_mw = (remaining / input.interval_hours).max(0.0);
                if energy_cap_mw < p.p_min_mw {
                    // The remaining energy cannot sustain the declared
                    // minimum: the participant cannot commit this interval.
                    p.p_min_mw = 0.0;
                    p.p_max_mw = 0.0;
                } else {
                    p.p_max_mw = p.p_max_mw.min(energy_cap_mw);
                }
            }
        }
        let problem = CoupledProblem::new(
            &format!("{}.t{}", input.horizon_id, index),
            -demand_mw,
            input.interval_hours,
            interval_participants.clone(),
        )
        .map_err(|error| CoordinationError::ProblemInvalid {
            detail: error.to_string(),
        })?;

        // The interval's workers hold the CAPPED profiles (the worker owns
        // its participant's capability; the coordinator never re-derives it).
        let interval_workers: Vec<ClosedFormLocalWorker> = interval_participants
            .iter()
            .map(|p| ClosedFormLocalWorker {
                participant: p.clone(),
            })
            .collect();
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        for w in &interval_workers {
            workers.insert(w.participant.participant_id.clone(), w);
        }

        let coordination =
            AdmmCoordination::new(&problem, workers, fallback, input.criteria.clone())?;
        let run = coordination.run(input.rho)?;

        // The energy the FINAL schedule adopted (recorded failure adopts
        // nothing — the rolling state is honest about what actually happened).
        let adopted: BTreeMap<String, f64> = match run.final_schedule() {
            Some(FinalSchedule::ConvergedAdmm { x }) => x,
            Some(FinalSchedule::CentralizedFallback { outcome }) => {
                let mut map: BTreeMap<String, f64> = problem
                    .participant_ids()
                    .into_iter()
                    .map(|id| (id, 0.0))
                    .collect();
                for d in &outcome.dispatches {
                    map.insert(d.asset_id.clone(), d.p_mw);
                }
                map
            }
            None => problem
                .participant_ids()
                .into_iter()
                .map(|id| (id, 0.0))
                .collect(),
        };
        let mut energy_used_mwh: BTreeMap<String, f64> = BTreeMap::new();
        for (participant_id, p_mw) in &adopted {
            let used = p_mw * input.interval_hours;
            energy_used_mwh.insert(participant_id.clone(), used);
            if let Some(remaining) = remaining_mwh.get_mut(participant_id) {
                *remaining = (*remaining - used).max(0.0);
            }
        }

        interval_outcomes.push(IntervalOutcome {
            index,
            demand_mw,
            run,
            energy_used_mwh,
        });
    }

    let first_step_schedule = interval_outcomes.first().and_then(|o| o.run.final_schedule());
    let any_fallback_used = interval_outcomes
        .iter()
        .any(|o| o.run.schedule_is_from_fallback());

    Ok(DmpcPlan {
        dmpc_version: DMPC_DRIVER_VERSION.to_string(),
        horizon_id: input.horizon_id.clone(),
        interval_hours: input.interval_hours,
        intervals: interval_outcomes,
        first_step_schedule,
        any_fallback_used,
    })
}

// ---------------------------------------------------------------------------
// Proposals only (ADR-010): the first step as a ResearchProposal input to
// the P8.05 independent validation harness
// ---------------------------------------------------------------------------

/// Packages the plan's FIRST STEP as a [`ResearchProposal`] — the §13.4
/// `proposed_schedule` vocabulary — for the P8.05 independent validation
/// harness. `Ok(None)` when the plan adopted no schedule (a recorded
/// failure proposes nothing; absence is honest). The provenance is honest
/// by construction: the ADMM is deterministic with NO randomness
/// (`seed_supported = false`, no seed — the P7.42 admission), the
/// termination status is the named run status (never the bare "ok"), and
/// the `model_artifact_ref` is the reviewer-signed formulation record (the
/// artifact governing this provider).
pub fn proposal_for_first_step(
    plan: &DmpcPlan,
    producer_plugin_id: &str,
    producer_plugin_version: &str,
    sequence: u64,
    model_version: &str,
    input_snapshot_id: &str,
    validity: ResearchValidityWindow,
) -> Result<Option<ResearchProposal>, Refusal> {
    let Some(schedule) = &plan.first_step_schedule else {
        // A recorded failure adopted no schedule — there is nothing to
        // propose (honest absence, never a fabricated payload).
        return Ok(None);
    };
    let first = &plan.intervals[0];

    // Setpoints: every participant id present (0.0 when the schedule did not
    // commit the participant), sorted keys (BTreeMap serialization).
    let mut setpoints: BTreeMap<String, f64> = first
        .run
        .distributed_x_mw
        .keys()
        .map(|id| (id.clone(), 0.0))
        .collect();
    let mut payload = serde_json::Map::new();
    payload.insert("horizon_id".to_string(), serde_json::json!(plan.horizon_id));
    payload.insert(
        "interval_hours".to_string(),
        serde_json::json!(plan.interval_hours),
    );
    payload.insert("dmpc_version".to_string(), serde_json::json!(DMPC_DRIVER_VERSION));
    match schedule {
        FinalSchedule::ConvergedAdmm { x } => {
            for (id, value) in x {
                setpoints.insert(id.clone(), *value);
            }
            payload.insert(
                "schedule_source".to_string(),
                serde_json::json!("converged_admm"),
            );
        }
        FinalSchedule::CentralizedFallback { outcome } => {
            for d in &outcome.dispatches {
                setpoints.insert(d.asset_id.clone(), d.p_mw);
            }
            payload.insert(
                "schedule_source".to_string(),
                serde_json::json!("centralized_fallback"),
            );
            payload.insert(
                "fallback_baseline_id".to_string(),
                serde_json::json!(outcome.baseline_id),
            );
            payload.insert(
                "fallback_baseline_version".to_string(),
                serde_json::json!(outcome.baseline_version),
            );
            payload.insert(
                "fallback_reason".to_string(),
                serde_json::json!(outcome.reason.as_str()),
            );
            payload.insert(
                "fallback_supplied_mw".to_string(),
                serde_json::json!(outcome.supplied_mw),
            );
            payload.insert(
                "fallback_deficit_mw".to_string(),
                serde_json::json!(outcome.deficit_mw),
            );
        }
    }
    payload.insert("setpoints_mw".to_string(), serde_json::json!(setpoints));

    // Diagnostics (§9.2): staleness, fallback, and the boundary statement —
    // human-facing prose the harness never interprets as a verdict.
    let mut diagnostics: Vec<String> = vec![
        "proposals only (ADR-010): this payload is a candidate schedule for the P8.05 \
         independent validation harness, never a dispatch decision"
            .to_string(),
    ];
    for event in &first.run.stale_events {
        diagnostics.push(format!(
            "iteration {}: participant {} stale; last accepted value reused ({})",
            event.iteration, event.participant_id, event.detail
        ));
    }
    if let Some(outcome) = &first.run.fallback {
        diagnostics.push(format!(
            "centralized fallback used: {} {} (reason: {}); supplied {} MW, deficit {} MW",
            outcome.baseline_id,
            outcome.baseline_version,
            outcome.reason.as_str(),
            outcome.supplied_mw,
            outcome.deficit_mw
        ));
    }
    if let Some(error) = &first.run.fallback_error {
        diagnostics.push(format!("centralized fallback FAILED: {error}"));
    }

    let provenance = ResearchProvenance {
        worker_version: DMPC_DRIVER_VERSION.to_string(),
        model_version: model_version.to_string(),
        solver_version: ADMM_OPERATIONS_VERSION.to_string(),
        input_snapshot_id: input_snapshot_id.to_string(),
        termination_status: first.run.status.name().to_string(),
        diagnostics,
        seed: None,
        seed_supported: false,
        deterministic: true,
        model_artifact_ref: ADMM_FORMULATION_RECORD.to_string(),
    };
    let proposal = ResearchProposal::new(
        &format!("{}-t0", plan.horizon_id),
        producer_plugin_id,
        producer_plugin_version,
        sequence,
        ResearchResultKind::ProposedSchedule,
        serde_json::Value::Object(payload),
        provenance,
        validity,
    )?;
    Ok(Some(proposal))
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value is hand-computed
// arithmetic stated in the comments — the toys' optima and fixed points are
// derived on paper IN the comments; nothing is fabricated and no benchmark
// figure exists).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admm::FeasibilityCertificate;
    use crate::validation::{
        GateSeamError, HarnessClock, IndependentFeasibilityGate, IndependentVerdict,
        SnapshotFreshness, SnapshotFreshnessSource, ValidationHarness,
    };

    /// Demand-floor participant: a_k = −1 (its export counts toward meeting
    /// the aggregate floor b = −d).
    fn demand_participant(id: &str, q: f64, c: f64, p_min: f64, p_max: f64) -> Participant {
        Participant {
            participant_id: id.to_string(),
            cost_curvature: q,
            preferred_output_mw: c,
            p_min_mw: p_min,
            p_max_mw: p_max,
            energy_capacity_mwh: None,
            soc_fraction: None,
            coupling_coefficient: -1.0,
        }
    }

    /// Battery participant (energy-limited, demand-floor coupling).
    fn battery_participant(
        id: &str,
        q: f64,
        c: f64,
        p_min: f64,
        p_max: f64,
        capacity_mwh: f64,
        soc: f64,
    ) -> Participant {
        Participant {
            participant_id: id.to_string(),
            cost_curvature: q,
            preferred_output_mw: c,
            p_min_mw: p_min,
            p_max_mw: p_max,
            energy_capacity_mwh: Some(capacity_mwh),
            soc_fraction: Some(soc),
            coupling_coefficient: -1.0,
        }
    }

    /// Toy 2 (the P8.07 fixed-point toy): two symmetric participants, q = 2,
    /// c = 0, boxes [0, 3], demand d = 4 (b = −4). Hand-derived fixed point:
    /// x = (2, 2), z = (−2, −2), u = (−2, −2) with ρ = 2 (see the admm.rs
    /// test for the derivation); the run converges EXACTLY in 2 iterations.
    fn toy_two_problem() -> CoupledProblem {
        CoupledProblem::new(
            "toy-two",
            -4.0,
            1.0,
            vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("toy two is a valid problem")
    }

    /// Toy 3 (asymmetric, active coupling): q_a = 2 (f_a = (x_a − 1)²),
    /// q_b = 4 (f_b = 2(x_b − 3)²), boxes [0, 4], demand d = 5 (b = −5).
    /// Hand-derived optimum: 2(x_a − 1) = λ, 4(x_b − 3) = λ, x_a + x_b = 5
    /// → λ = 4/3 → (x_a, x_b) = (5/3, 10/3).
    fn toy_three_problem() -> CoupledProblem {
        CoupledProblem::new(
            "toy-three",
            -5.0,
            1.0,
            vec![
                demand_participant("a", 2.0, 1.0, 0.0, 4.0),
                demand_participant("b", 4.0, 3.0, 0.0, 4.0),
            ],
        )
        .expect("toy three is a valid problem")
    }

    /// A worker that NEVER answers (the communication-failure injection).
    struct AlwaysErrors {
        detail: String,
    }

    impl AdmmLocalWorker for AlwaysErrors {
        fn solve_local(
            &self,
            request: &LocalSolveRequest,
        ) -> Result<LocalSolveReply, LocalSolveError> {
            Err(LocalSolveError {
                participant_id: request.participant_id.clone(),
                detail: self.detail.clone(),
            })
        }
    }

    /// A worker that flags EVERY reply stale (the stale-participant
    /// injection). A flagged reply is discarded by the coordinator, so the
    /// value here is irrelevant to the run.
    struct AlwaysStale;

    impl AdmmLocalWorker for AlwaysStale {
        fn solve_local(
            &self,
            _request: &LocalSolveRequest,
        ) -> Result<LocalSolveReply, LocalSolveError> {
            Ok(LocalSolveReply {
                value_mw: 0.0,
                stale: true,
            })
        }
    }

    /// A healthy closed-form worker that flags its replies stale through the
    /// given iteration (inclusive), then answers exactly.
    struct StaleThenHealthy {
        participant: Participant,
        stale_through: u32,
    }

    impl AdmmLocalWorker for StaleThenHealthy {
        fn solve_local(
            &self,
            request: &LocalSolveRequest,
        ) -> Result<LocalSolveReply, LocalSolveError> {
            if request.iteration <= self.stale_through {
                return Ok(LocalSolveReply {
                    value_mw: 0.0,
                    stale: true,
                });
            }
            ClosedFormLocalWorker {
                participant: self.participant.clone(),
            }
            .solve_local(request)
        }
    }

    // --- constants (known answers) -----------------------------------------

    #[test]
    fn operations_constants_are_the_recorded_strings() {
        assert_eq!(ADMM_OPERATIONS_VERSION, "flexcore-research/admm-operations.v1");
        assert_eq!(DMPC_DRIVER_VERSION, "flexcore-research/dmpc.v1");
        assert_eq!(
            ADMM_FORMULATION_RECORD,
            "ledger/phase-08-advanced-coordination/07_admm_dmpc.md"
        );
        assert!(OPERATIONS_STATEMENT.contains("recorded on every run"));
        assert!(OPERATIONS_STATEMENT.contains("non-convergence, never a result"));
        assert!(OPERATIONS_STATEMENT.contains("exact certificate"));
        assert!(OPERATIONS_STATEMENT.contains("bounded-"));
        assert!(OPERATIONS_STATEMENT.contains("never presented as final"));
        assert!(OPERATIONS_STATEMENT.contains("never an unvalidated warm start"));
        assert!(OPERATIONS_STATEMENT.contains("Proposals only (ADR-010)"));
        // The rolling-horizon seam is BY CITATION (no dependency on
        // flexcore-application) and names the P3.24 surface.
        assert!(ROLLING_HORIZON_SEAM.contains("rolling_horizon.rs"));
        assert!(ROLLING_HORIZON_SEAM.contains("P3.24"));
        assert!(ROLLING_HORIZON_SEAM.contains("BY CITATION"));
        assert!(ROLLING_HORIZON_SEAM.contains("plan_rolling_horizon_roll"));
        // The fallback seam names the frozen registry records.
        assert!(FALLBACK_SEAM.contains("baseline/rule-based-priority-list"));
        assert!(FALLBACK_SEAM.contains("priority-list.v1"));
        assert!(FALLBACK_SEAM.contains("baseline/highs-tier1-lp"));
        assert!(FALLBACK_SEAM.contains("NEVER an unstudied heuristic"));
        assert!(FALLBACK_SEAM.contains("never an unvalidated warm start"));
    }

    #[test]
    fn run_status_vocabulary_is_closed_and_named() {
        let cases: [(RunStatus, &str, Option<FallbackReason>); 4] = [
            (RunStatus::Converged, "CONVERGED", None),
            (
                RunStatus::NonConvergedBudgetExhausted,
                "NON_CONVERGED_BUDGET_EXHAUSTED",
                Some(FallbackReason::NonConvergedBudgetExhausted),
            ),
            (
                RunStatus::InfeasibleCertificate,
                "INFEASIBLE_CERTIFICATE",
                Some(FallbackReason::InfeasibleCertificate),
            ),
            (
                RunStatus::AbandonedParticipantUnresponsive,
                "ABANDONED_PARTICIPANT_UNRESPONSIVE",
                Some(FallbackReason::ParticipantAbandoned),
            ),
        ];
        for (status, name, reason) in cases {
            assert_eq!(status.name(), name);
            assert_eq!(status.fallback_reason(), reason);
        }
        assert_ne!(RunStatus::Converged.name(), "ok", "never the bare \"ok\"");
        assert_eq!(FallbackReason::InfeasibleCertificate.as_str(), "infeasible_certificate");
        assert_eq!(
            FallbackReason::NonConvergedBudgetExhausted.as_str(),
            "non_converged_budget_exhausted"
        );
        assert_eq!(FallbackReason::ParticipantAbandoned.as_str(), "participant_abandoned");
    }

    // --- criteria validation (named, fail-closed) ---------------------------

    #[test]
    fn termination_criteria_validate_with_named_errors() {
        let standard = TerminationCriteria::standard();
        assert_eq!(standard.epsilon_abs, 1.0e-6);
        assert_eq!(standard.epsilon_rel, 1.0e-6);
        assert_eq!(standard.max_iterations, 1000);
        assert_eq!(standard.max_stale_iterations, 2);
        standard.validate().expect("the standard criteria are valid");
        // Zero tolerance → named error.
        let zero = TerminationCriteria {
            epsilon_abs: 0.0,
            ..TerminationCriteria::standard()
        };
        assert_eq!(
            zero.validate().expect_err("zero tolerance refused"),
            CriteriaError::NonPositiveEpsilon {
                field: "epsilon_abs",
                value: 0.0
            }
        );
        // Negative relative tolerance → named error.
        let negative = TerminationCriteria {
            epsilon_rel: -1.0,
            ..TerminationCriteria::standard()
        };
        assert_eq!(
            negative.validate().expect_err("negative tolerance refused").name(),
            "NON_POSITIVE_EPSILON"
        );
        // Zero budget → named error.
        let zero_budget = TerminationCriteria {
            max_iterations: 0,
            ..TerminationCriteria::standard()
        };
        assert_eq!(
            zero_budget.validate().expect_err("zero budget refused"),
            CriteriaError::ZeroIterationBudget
        );
        // max_stale_iterations = 0 is LEGAL (no staleness tolerated).
        let strict = TerminationCriteria {
            max_stale_iterations: 0,
            ..TerminationCriteria::standard()
        };
        strict.validate().expect("zero staleness tolerance is legal");
        assert!(!zero.to_string().is_empty());
        assert!(!CriteriaError::ZeroIterationBudget.to_string().is_empty());
    }

    // --- construction: fail-closed coverage and penalty ----------------------

    #[test]
    fn construction_refuses_incomplete_worker_coverage_and_bad_criteria() {
        let problem = toy_two_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        // Participant "b" has no worker → refused at construction (a coverage
        // gap must never look like a mid-run participant drop).
        let error = AdmmCoordination::new(
            &problem,
            workers,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect_err("missing coverage refused");
        assert_eq!(
            error,
            CoordinationError::WorkerCoverageMissing {
                participant_id: "b".to_string()
            }
        );
        // Zero iteration budget → refused at construction.
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let bad_criteria = TerminationCriteria {
            max_iterations: 0,
            ..TerminationCriteria::standard()
        };
        let error = AdmmCoordination::new(&problem, workers, &PriorityListFallback, bad_criteria)
            .expect_err("bad criteria refused");
        assert_eq!(
            error,
            CoordinationError::InvalidCriteria(CriteriaError::ZeroIterationBudget)
        );
    }

    #[test]
    fn run_refuses_a_non_positive_penalty() {
        let problem = toy_two_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let coordination = AdmmCoordination::new(
            &problem,
            workers,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect("coverage ok");
        let error = coordination.run(0.0).expect_err("zero penalty refused");
        assert_eq!(error, CoordinationError::InvalidPenalty { value: 0.0 });
        let error = coordination.run(f64::NAN).expect_err("nan penalty refused");
        assert_eq!(error.name(), "INVALID_PENALTY");
    }

    // --- THE CONVERGENCE KNOWN ANSWERS (hand-traced arithmetic) --------------

    #[test]
    fn toy_two_converges_exactly_in_two_iterations() {
        // Hand-traced with ρ = 2, ε = 1e-6 (absolute + relative):
        // iteration 1: s = z + u = 0 → x = (q·c + ρ·a·s)/(q + ρa²) = 0 both;
        //   z: v = a∘x − u = 0, Σv = 0 > −4 → ν = 2 → z = (−2, −2);
        //   u = z − a∘x = (−2, −2). Residuals: r = ‖a∘x − z‖ = 2√2 > ε,
        //   s = ρ‖a∘Δz‖ = 2·2√2 > ε → continue.
        // iteration 2: s = −4 → x_k = (0 + 2·(−1)·(−4))/4 = 2.0 exactly both;
        //   z: v_k = −2 + 2 = 0 → ν = 2 → z unchanged; u unchanged
        //   → r = 0 exactly, s = 0 exactly → CONVERGED at iteration 2.
        let problem = toy_two_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let coordination = AdmmCoordination::new(
            &problem,
            workers,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::Converged);
        assert_eq!(result.iterations_run, 2);
        assert_eq!(result.primal_residual, 0.0);
        assert_eq!(result.dual_residual, 0.0);
        assert_eq!(result.distributed_x_mw["a"], 2.0);
        assert_eq!(result.distributed_x_mw["b"], 2.0);
        assert!(result.fallback.is_none());
        assert!(result.fallback_error.is_none());
        assert!(!result.used_stale_values);
        assert!(result.stale_events.is_empty());
        // The schedule path returns the CONVERGED distributed solution.
        let schedule = result.final_schedule().expect("converged run has a schedule");
        match schedule {
            FinalSchedule::ConvergedAdmm { x } => {
                assert_eq!(x["a"], 2.0);
                assert_eq!(x["b"], 2.0);
            }
            other => panic!("a converged run must not yield a fallback schedule: {other:?}"),
        }
        assert!(!result.schedule_is_from_fallback());
    }

    #[test]
    fn toy_three_converges_to_the_hand_derived_optimum() {
        // Asymmetric toy (see toy_three_problem): hand-derived optimum
        // (5/3, 10/3) with the coupling active (λ = 4/3 > 0). The assertion
        // is against the ANALYTIC optimum (derived above from the KKT
        // conditions), within a tolerance consistent with the residual
        // tolerances — NOT a fabricated benchmark figure.
        let problem = toy_three_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 1.0, 0.0, 4.0),
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 4.0, 3.0, 0.0, 4.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let criteria = TerminationCriteria {
            epsilon_abs: 1.0e-10,
            epsilon_rel: 1.0e-10,
            max_iterations: 5000,
            max_stale_iterations: 2,
        };
        let coordination =
            AdmmCoordination::new(&problem, workers, &PriorityListFallback, criteria)
                .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::Converged);
        assert!(result.iterations_run <= 5000);
        // The hand-derived optimum:
        let x = &result.distributed_x_mw;
        assert!(
            (x["a"] - 5.0 / 3.0).abs() <= 1.0e-6,
            "x_a = {} must approach the analytic 5/3",
            x["a"]
        );
        assert!(
            (x["b"] - 10.0 / 3.0).abs() <= 1.0e-6,
            "x_b = {} must approach the analytic 10/3",
            x["b"]
        );
        // The coupling is met: x_a + x_b ≥ 5 (within the analytic tolerance).
        assert!(x["a"] + x["b"] >= 5.0 - 1.0e-5);
        // The recorded residuals are within the recorded tolerances (the
        // termination evidence travels with the result).
        assert!(result.primal_residual <= result.epsilon_primal);
        assert!(result.dual_residual <= result.epsilon_dual);
    }

    // --- infeasibility (step 2: certificate, never a timeout guess) ---------

    #[test]
    fn infeasible_demand_is_a_certificate_with_an_honest_fallback_deficit() {
        // Demand 10 MW over boxes [0, 3] × 2: min achievable aggregate
        // = −6 > b = −10 → INFEASIBLE, certified BEFORE any iteration.
        // The frozen priority-list fallback serves what it can —
        // a: min(10, 3, ∞) = 3; b: min(7, 3, ∞) = 3 — supplied 6, deficit 4,
        // recorded, never silently filled.
        let problem = CoupledProblem::new(
            "infeasible",
            -10.0,
            1.0,
            vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("valid problem");
        assert!(!problem.feasibility_certificate().feasible);
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let coordination = AdmmCoordination::new(
            &problem,
            workers,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::InfeasibleCertificate);
        assert_eq!(result.iterations_run, 0, "no iteration ran for a certificate");
        let outcome = result.fallback.as_ref().expect("the fallback is recorded");
        assert_eq!(outcome.baseline_id, BASELINE_ID_PRIORITY_LIST);
        assert_eq!(outcome.baseline_version, "priority-list.v1");
        assert_eq!(outcome.reason, FallbackReason::InfeasibleCertificate);
        assert_eq!(outcome.supplied_mw, 6.0);
        assert_eq!(outcome.deficit_mw, 4.0);
        // The frozen rule set's order: (priority, asset_id) with priority 0
        // → lexicographic: a then b, each committed at its 3 MW cap.
        assert_eq!(outcome.dispatches.len(), 2);
        assert_eq!(outcome.dispatches[0].asset_id, "a");
        assert_eq!(outcome.dispatches[0].p_mw, 3.0);
        assert_eq!(outcome.dispatches[1].asset_id, "b");
        assert_eq!(outcome.dispatches[1].p_mw, 3.0);
        // The schedule path returns the FALLBACK schedule (never a partial
        // ADMM state).
        match result.final_schedule().expect("fallback produced a schedule") {
            FinalSchedule::CentralizedFallback { outcome } => {
                assert_eq!(outcome.deficit_mw, 4.0);
            }
            other => panic!("expected the fallback schedule, got {other:?}"),
        }
        assert!(result.schedule_is_from_fallback());
    }

    // --- non-convergence within budget (step 1) + fallback (step 4) ---------

    #[test]
    fn budget_exhaustion_falls_back_and_never_presents_the_partial_result() {
        // Toy 3 needs more than one iteration (hand-traced: after iteration 1
        // the primal residual is ‖(1.25, 1.25)‖ = 1.25·√2 > 0 — x moved to
        // (0.5, 2.0), z to (−1.75, −3.25), u to (−1.25, −1.25)), so a budget
        // of 1 exhausts it. The run records NON_CONVERGED + the fallback
        // schedule; the partial distributed x stays diagnostics-only.
        // Fallback (demand 5, boxes [0,4]×2, rule set in id order):
        // a: min(5, 4, ∞) = 4; b: min(1, 4, ∞) = 1 → supplied 5, deficit 0.
        let problem = toy_three_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 1.0, 0.0, 4.0),
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 4.0, 3.0, 0.0, 4.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let criteria = TerminationCriteria {
            max_iterations: 1,
            ..TerminationCriteria::standard()
        };
        let coordination =
            AdmmCoordination::new(&problem, workers, &PriorityListFallback, criteria)
                .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::NonConvergedBudgetExhausted);
        assert_eq!(result.iterations_run, 1);
        assert!(result.primal_residual > 0.0, "the residual is recorded, never hidden");
        let outcome = result.fallback.as_ref().expect("the fallback is recorded");
        assert_eq!(outcome.reason, FallbackReason::NonConvergedBudgetExhausted);
        assert_eq!(outcome.supplied_mw, 5.0);
        assert_eq!(outcome.deficit_mw, 0.0);
        assert_eq!(outcome.dispatches.len(), 2);
        assert_eq!(outcome.dispatches[0].asset_id, "a");
        assert_eq!(outcome.dispatches[0].p_mw, 4.0);
        assert_eq!(outcome.dispatches[1].asset_id, "b");
        assert_eq!(outcome.dispatches[1].p_mw, 1.0);
        match result.final_schedule().expect("fallback produced a schedule") {
            FinalSchedule::CentralizedFallback { .. } => {}
            other => panic!("a non-converged run must not present its partial state: {other:?}"),
        }
    }

    // --- communication failure and stale participants (step 3) ---------------

    #[test]
    fn an_unresponsive_participant_abandons_the_run_and_falls_back() {
        // Worker "b" never answers; max_stale_iterations = 0 → the FIRST
        // unanswered call abandons the run (no staleness tolerated).
        // The result names the participant and the iteration, records the
        // event, and the fallback (demand 4: a commits 3, b commits 1 →
        // supplied 4, deficit 0) — never a silently partial result.
        let problem = toy_two_problem();
        let wa = ClosedFormLocalWorker {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
        };
        let dead = AlwaysErrors {
            detail: "worker process died".to_string(),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &dead);
        let criteria = TerminationCriteria {
            max_stale_iterations: 0,
            ..TerminationCriteria::standard()
        };
        let coordination =
            AdmmCoordination::new(&problem, workers, &PriorityListFallback, criteria)
                .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::AbandonedParticipantUnresponsive);
        assert_eq!(result.abandoned_participant.as_deref(), Some("b"));
        assert_eq!(result.iterations_run, 1);
        assert_eq!(result.stale_events.len(), 1);
        assert!(result.stale_events[0].detail.contains("worker process died"));
        let outcome = result.fallback.as_ref().expect("the fallback is recorded");
        assert_eq!(outcome.reason, FallbackReason::ParticipantAbandoned);
        assert_eq!(outcome.supplied_mw, 4.0);
        assert_eq!(outcome.deficit_mw, 0.0);
    }

    #[test]
    fn bounded_staleness_reuses_within_the_bound_and_abandons_beyond_it() {
        // Worker "a" flags EVERY reply stale; max_stale_iterations = 1:
        // iteration 1: consecutive 0 + 1 = 1 ≤ 1 → reuse the last accepted
        //   value (event recorded, run continues);
        // iteration 2: consecutive 1 + 1 = 2 > 1 → ABANDONED (second event).
        let problem = toy_two_problem();
        let stale = AlwaysStale;
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &stale);
        workers.insert("b".to_string(), &wb);
        let criteria = TerminationCriteria {
            max_iterations: 10,
            max_stale_iterations: 1,
            ..TerminationCriteria::standard()
        };
        let coordination =
            AdmmCoordination::new(&problem, workers, &PriorityListFallback, criteria)
                .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::AbandonedParticipantUnresponsive);
        assert_eq!(result.abandoned_participant.as_deref(), Some("a"));
        assert_eq!(result.iterations_run, 2);
        assert_eq!(result.stale_events.len(), 2);
        assert!(result.used_stale_values);
        assert!(result.fallback.is_some());
    }

    #[test]
    fn a_converged_run_records_its_staleness_honestly() {
        // Worker "a" flags its reply stale at iteration 1 only, healthy after.
        // Toy 2 converges at iteration 2 EXACTLY, and the reused value at
        // iteration 1 equals the computed one (both are the box-clamped
        // zero), so the run converges with used_stale_values = true and ONE
        // recorded event — the staleness is visible, never silent.
        let problem = toy_two_problem();
        let flaky = StaleThenHealthy {
            participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
            stale_through: 1,
        };
        let wb = ClosedFormLocalWorker {
            participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &flaky);
        workers.insert("b".to_string(), &wb);
        let coordination = AdmmCoordination::new(
            &problem,
            workers,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::Converged);
        assert_eq!(result.iterations_run, 2);
        assert_eq!(result.distributed_x_mw["a"], 2.0);
        assert_eq!(result.distributed_x_mw["b"], 2.0);
        assert!(result.used_stale_values);
        assert_eq!(result.stale_events.len(), 1);
        assert_eq!(result.stale_events[0].iteration, 1);
        assert_eq!(result.stale_events[0].participant_id, "a");
        assert!(result.fallback.is_none());
    }

    // --- fallback refusal (a recorded failure, never a schedule) -------------

    #[test]
    fn a_fallback_refusal_is_a_recorded_failure_never_a_schedule() {
        // EXPORT-CAP class (a = +1, aggregate export ≤ 4; participants prefer
        // 5 MW each, boxes [0, 5]): the in-crate frozen fallback serves
        // demand-floor couplings only → it REFUSES with a named error. The
        // result records the refusal; final_schedule() is NONE — the failed
        // run produces NO schedule at all rather than a silently partial one.
        // (Hand-trace for the non-convergence: iteration 1 with ρ = 2 gives
        // x = (2.5, 2.5) — s = 0 → x = q·c/(q + ρa²) = 10/4 — and the primal
        // residual ‖(0.5, 0.5)‖ > 0, so a budget of 1 exhausts the run.)
        let problem = CoupledProblem::new(
            "export-cap",
            4.0,
            1.0,
            vec![
                Participant {
                    participant_id: "a".to_string(),
                    cost_curvature: 2.0,
                    preferred_output_mw: 5.0,
                    p_min_mw: 0.0,
                    p_max_mw: 5.0,
                    energy_capacity_mwh: None,
                    soc_fraction: None,
                    coupling_coefficient: 1.0,
                },
                Participant {
                    participant_id: "b".to_string(),
                    cost_curvature: 2.0,
                    preferred_output_mw: 5.0,
                    p_min_mw: 0.0,
                    p_max_mw: 5.0,
                    energy_capacity_mwh: None,
                    soc_fraction: None,
                    coupling_coefficient: 1.0,
                },
            ],
        )
        .expect("valid problem");
        let wa = ClosedFormLocalWorker {
            participant: Participant {
                participant_id: "a".to_string(),
                cost_curvature: 2.0,
                preferred_output_mw: 5.0,
                p_min_mw: 0.0,
                p_max_mw: 5.0,
                energy_capacity_mwh: None,
                soc_fraction: None,
                coupling_coefficient: 1.0,
            },
        };
        let wb = ClosedFormLocalWorker {
            participant: Participant {
                participant_id: "b".to_string(),
                cost_curvature: 2.0,
                preferred_output_mw: 5.0,
                p_min_mw: 0.0,
                p_max_mw: 5.0,
                energy_capacity_mwh: None,
                soc_fraction: None,
                coupling_coefficient: 1.0,
            },
        };
        let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
        workers.insert("a".to_string(), &wa);
        workers.insert("b".to_string(), &wb);
        let criteria = TerminationCriteria {
            max_iterations: 1,
            ..TerminationCriteria::standard()
        };
        let coordination =
            AdmmCoordination::new(&problem, workers, &PriorityListFallback, criteria)
                .expect("coverage ok");
        let result = coordination.run(2.0).expect("penalty valid");
        assert_eq!(result.status, RunStatus::NonConvergedBudgetExhausted);
        let error = result.fallback_error.as_ref().expect("the refusal is recorded");
        assert!(error.contains("does not support this coupling class"), "{error}");
        assert!(result.fallback.is_none());
        assert!(
            result.final_schedule().is_none(),
            "a refused fallback leaves NO schedule — never a silently partial result"
        );
    }

    // --- the DMPC rolling horizon (hand-computed known answers) --------------

    #[test]
    fn dmpc_horizon_two_known_answer_with_energy_depletion() {
        // Participant "a" is a 4 MWh battery (SOC 1.0, p_max 3 MW); "b" is
        // unlimited (p_max 3 MW). Δt = 1 h; demands [4, 4]; ρ = 2.
        // Interval 0: a's energy cap = 4/1 = 4 MW → box stays [0, 3] → the
        //   toy-2 problem exactly → converges exactly at x = (2, 2) in 2
        //   iterations. Energy adopted: a 2.0 MWh → remaining 2.0.
        // Interval 1: a's cap = 2/1 = 2 MW → box [0, 2]; demand 4 is feasible
        //   (2 + 3 ≥ 4). Hand-traced: iteration 1 x = 0; iteration 2:
        //   a: s = −4 → (0 + 2·(−1)·(−4))/4 = 2 → clip [0, 2] = 2; b → 2 →
        //   converged exactly at (2, 2). Energy adopted: a 2.0 MWh (empty).
        let input = DmpcHorizonInput {
            horizon_id: "horizon-1".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: vec![4.0, 4.0],
            participants: vec![
                battery_participant("a", 2.0, 0.0, 0.0, 3.0, 4.0, 1.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
        assert_eq!(plan.dmpc_version, DMPC_DRIVER_VERSION);
        assert_eq!(plan.intervals.len(), 2);
        let i0 = &plan.intervals[0];
        assert_eq!(i0.index, 0);
        assert_eq!(i0.demand_mw, 4.0);
        assert_eq!(i0.run.status, RunStatus::Converged);
        assert_eq!(i0.run.distributed_x_mw["a"], 2.0);
        assert_eq!(i0.run.distributed_x_mw["b"], 2.0);
        assert_eq!(i0.energy_used_mwh["a"], 2.0);
        let i1 = &plan.intervals[1];
        assert_eq!(i1.run.status, RunStatus::Converged);
        assert_eq!(i1.run.distributed_x_mw["a"], 2.0, "the depleted battery still meets its 2 MW cap");
        assert_eq!(i1.energy_used_mwh["a"], 2.0);
        assert!(!plan.any_fallback_used);
        // The first step is the committed receding-horizon schedule.
        match plan.first_step_schedule.expect("the first step exists") {
            FinalSchedule::ConvergedAdmm { x } => {
                assert_eq!(x["a"], 2.0);
                assert_eq!(x["b"], 2.0);
            }
            other => panic!("expected the converged first step, got {other:?}"),
        }
    }

    #[test]
    fn dmpc_records_an_infeasible_interval_with_its_fallback_deficit() {
        // Demands [4, 10]: interval 1 is infeasible (boxes [0, 3] × 2 → min
        // aggregate −6 > −10) → INFEASIBLE_CERTIFICATE + the frozen
        // priority-list fallback (supplied 6, deficit 4). Interval 0 remains
        // the converged first step; the plan flags any_fallback_used.
        let input = DmpcHorizonInput {
            horizon_id: "horizon-2".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: vec![4.0, 10.0],
            participants: vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
        assert!(plan.any_fallback_used);
        assert_eq!(plan.intervals[0].run.status, RunStatus::Converged);
        assert_eq!(plan.intervals[1].run.status, RunStatus::InfeasibleCertificate);
        assert_eq!(plan.intervals[1].run.iterations_run, 0);
        let outcome = plan.intervals[1]
            .run
            .fallback
            .as_ref()
            .expect("the fallback is recorded");
        assert_eq!(outcome.supplied_mw, 6.0);
        assert_eq!(outcome.deficit_mw, 4.0);
        // The first step is interval 0's converged schedule (the receding
        // horizon commits the FIRST step, not the failed later interval).
        match plan.first_step_schedule.expect("the first step exists") {
            FinalSchedule::ConvergedAdmm { x } => {
                assert_eq!(x["a"], 2.0);
                assert_eq!(x["b"], 2.0);
            }
            other => panic!("expected the converged first step, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_horizon_plan_plans_nothing_and_proposes_nothing() {
        let input = DmpcHorizonInput {
            horizon_id: "horizon-empty".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: Vec::new(),
            participants: vec![demand_participant("a", 2.0, 0.0, 0.0, 3.0)],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("an empty horizon is legal");
        assert!(plan.intervals.is_empty());
        assert!(plan.first_step_schedule.is_none());
        let proposal = proposal_for_first_step(
            &plan,
            "plugin/research-admm",
            "0.1.0",
            1,
            "grid-model/2026.11",
            "snapshot-a",
            ResearchValidityWindow {
                valid_from_ms: 1_000,
                valid_until_ms: 2_000,
            },
        )
        .expect("the builder answers");
        assert!(
            proposal.is_none(),
            "a plan with no schedule proposes nothing — honest absence"
        );
    }

    #[test]
    fn horizon_input_validation_refuses_named_errors() {
        let valid = |mut input: DmpcHorizonInput| {
            input.participants = vec![demand_participant("a", 2.0, 0.0, 0.0, 3.0)];
            input
        };
        let mut input = valid(DmpcHorizonInput {
            horizon_id: String::new(),
            interval_hours: 1.0,
            interval_demand_mw: vec![1.0],
            participants: Vec::new(),
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        });
        input.horizon_id = String::new();
        assert_eq!(
            input.validate().expect_err("empty horizon id refused").name(),
            "EMPTY_HORIZON_ID"
        );
        input.horizon_id = "h".to_string();
        input.interval_hours = 0.0;
        assert_eq!(
            input.validate().expect_err("zero hours refused").name(),
            "NON_POSITIVE_INTERVAL_HOURS"
        );
        input.interval_hours = 1.0;
        input.rho = 0.0;
        assert_eq!(input.validate().expect_err("zero rho refused").name(), "INVALID_PENALTY");
        input.rho = 2.0;
        input.interval_demand_mw = vec![1.0, -2.0];
        assert_eq!(
            input.validate().expect_err("negative demand refused"),
            CoordinationError::NegativeDemand {
                index: 1,
                value: -2.0
            }
        );
        input.interval_demand_mw = vec![1.0];
        input.participants = Vec::new();
        assert_eq!(
            input.validate().expect_err("no participants refused").name(),
            "EMPTY_PARTICIPANT_SET"
        );
        // The eager participant validation propagates the named problem error
        // (here: a negative curvature — the convexity gate).
        input.participants = vec![demand_participant("a", -2.0, 0.0, 0.0, 3.0)];
        let error = input.validate().expect_err("non-convex participant refused");
        assert_eq!(error.name(), "PROBLEM_INVALID");
        assert!(error.to_string().contains("NEGATIVE_COST_CURVATURE") || error.to_string().contains("negative"));
    }

    // --- proposals only: into the P8.05 independent validation harness ------

    /// A passing injected feasibility gate (the harness supplies the physics;
    /// these fakes mirror the validation.rs test fixtures).
    struct PassingGate;

    impl IndependentFeasibilityGate for PassingGate {
        fn revalidate(
            &self,
            _proposal: &ResearchProposal,
        ) -> Result<IndependentVerdict, GateSeamError> {
            Ok(IndependentVerdict::Passed {
                attempt_id: "AT-ADMM".to_string(),
                model_id: "toy".to_string(),
                model_content_hash: "hash-admm".to_string(),
                margins: "no violations".to_string(),
                coverage: "declared scope only".to_string(),
            })
        }
    }

    struct CurrentSnapshots;

    impl SnapshotFreshnessSource for CurrentSnapshots {
        fn freshness(&self, _snapshot_id: &str) -> SnapshotFreshness {
            SnapshotFreshness::Current
        }
    }

    struct FixedClock {
        now: i64,
    }

    impl HarnessClock for FixedClock {
        fn now_millis(&self) -> i64 {
            self.now
        }
    }

    #[test]
    fn the_first_step_proposal_passes_the_independent_harness() {
        let input = DmpcHorizonInput {
            horizon_id: "horizon-3".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: vec![4.0],
            participants: vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
        let proposal = proposal_for_first_step(
            &plan,
            "plugin/research-admm",
            "0.1.0",
            1,
            "grid-model/2026.11",
            "snapshot-2026-11-13-a",
            ResearchValidityWindow {
                valid_from_ms: 1_000,
                valid_until_ms: 2_000,
            },
        )
        .expect("the builder answers")
        .expect("the converged plan proposes");
        // The §13.4 vocabulary and the honest provenance.
        assert_eq!(proposal.result_kind.as_str(), "proposed_schedule");
        assert_eq!(proposal.provenance.termination_status, "CONVERGED");
        assert_ne!(proposal.provenance.termination_status, "ok");
        assert_eq!(proposal.provenance.seed, None);
        assert!(!proposal.provenance.seed_supported, "the ADMM has no randomness: the honest admission");
        assert!(proposal.provenance.deterministic);
        assert_eq!(proposal.provenance.worker_version, DMPC_DRIVER_VERSION);
        assert_eq!(proposal.provenance.solver_version, ADMM_OPERATIONS_VERSION);
        assert_eq!(
            proposal.provenance.model_artifact_ref,
            "ledger/phase-08-advanced-coordination/07_admm_dmpc.md"
        );
        // The payload is deterministic: sorted object keys, sorted setpoints.
        let keys: Vec<&str> = proposal.payload.as_object().expect("object").keys().collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "payload keys are sorted");
        let setpoints = proposal.payload["setpoints_mw"]
            .as_object()
            .expect("setpoints object");
        assert_eq!(setpoints["a"], serde_json::json!(2.0));
        assert_eq!(setpoints["b"], serde_json::json!(2.0));
        assert_eq!(proposal.payload["schedule_source"], serde_json::json!("converged_admm"));
        // AND it passes the FULL independent harness (proposals enter the
        // protected path like any other candidate).
        let gate = PassingGate;
        let snapshots = CurrentSnapshots;
        let clock = FixedClock { now: 1_500 };
        let harness = ValidationHarness::new(&gate, &snapshots, &clock);
        let report = harness.validate(&proposal).expect("full pass");
        assert!(report.is_validated());
        assert_eq!(report.harness_version, crate::validation::HARNESS_VERSION);
    }

    #[test]
    fn an_expired_first_step_proposal_is_refused_by_the_harness() {
        // The P8.05 discipline applies to THIS crate's proposals too — no
        // self-trust: an expired validity window is refused by the harness.
        let input = DmpcHorizonInput {
            horizon_id: "horizon-4".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: vec![4.0],
            participants: vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
        let proposal = proposal_for_first_step(
            &plan,
            "plugin/research-admm",
            "0.1.0",
            1,
            "grid-model/2026.11",
            "snapshot-2026-11-13-a",
            ResearchValidityWindow {
                valid_from_ms: 1_000,
                valid_until_ms: 2_000,
            },
        )
        .expect("the builder answers")
        .expect("the converged plan proposes");
        let gate = PassingGate;
        let snapshots = CurrentSnapshots;
        let clock = FixedClock { now: 2_500 }; // past the window
        let harness = ValidationHarness::new(&gate, &snapshots, &clock);
        let refusal = harness
            .validate(&proposal)
            .expect_err("an expired proposal is refused");
        assert_eq!(refusal.name(), "PROPOSAL_EXPIRED");
    }

    #[test]
    fn a_fallback_schedule_is_proposed_with_its_provenance_recorded() {
        // Demand 10 over boxes [0, 3] × 2: infeasible → the first step IS the
        // fallback schedule. The proposal records the fallback fields, the
        // INFEASIBLE_CERTIFICATE termination (never the bare "ok"), and the
        // deficit honestly — then passes the harness like any candidate.
        let input = DmpcHorizonInput {
            horizon_id: "horizon-5".to_string(),
            interval_hours: 1.0,
            interval_demand_mw: vec![10.0],
            participants: vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
            rho: 2.0,
            criteria: TerminationCriteria::standard(),
        };
        let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
        let proposal = proposal_for_first_step(
            &plan,
            "plugin/research-admm",
            "0.1.0",
            1,
            "grid-model/2026.11",
            "snapshot-2026-11-13-a",
            ResearchValidityWindow {
                valid_from_ms: 1_000,
                valid_until_ms: 2_000,
            },
        )
        .expect("the builder answers")
        .expect("the fallback schedule is proposed");
        assert_eq!(proposal.provenance.termination_status, "INFEASIBLE_CERTIFICATE");
        assert_eq!(
            proposal.payload["schedule_source"],
            serde_json::json!("centralized_fallback")
        );
        assert_eq!(proposal.payload["fallback_deficit_mw"], serde_json::json!(4.0));
        let setpoints = proposal.payload["setpoints_mw"].as_object().expect("object");
        assert_eq!(setpoints["a"], serde_json::json!(3.0));
        assert_eq!(setpoints["b"], serde_json::json!(3.0));
        let diagnostics = proposal.provenance.diagnostics.join("\n");
        assert!(diagnostics.contains("centralized fallback used"));
        assert!(diagnostics.contains("deficit 4"));
        let gate = PassingGate;
        let snapshots = CurrentSnapshots;
        let clock = FixedClock { now: 1_500 };
        let harness = ValidationHarness::new(&gate, &snapshots, &clock);
        assert!(harness.validate(&proposal).expect("full pass").is_validated());
    }

    // --- determinism ---------------------------------------------------------

    #[test]
    fn two_runs_of_the_same_input_are_identical() {
        let problem = toy_two_problem();
        let run_once = |stamp: u32| {
            // Fresh workers per run (the closed-form worker is stateless, but
            // the fixture must not share state across runs either way).
            let wa = ClosedFormLocalWorker {
                participant: demand_participant("a", 2.0, 0.0, 0.0, 3.0),
            };
            let wb = ClosedFormLocalWorker {
                participant: demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            };
            let mut workers: BTreeMap<String, &dyn AdmmLocalWorker> = BTreeMap::new();
            workers.insert("a".to_string(), &wa);
            workers.insert("b".to_string(), &wb);
            let _ = stamp;
            AdmmCoordination::new(
                &problem,
                workers,
                &PriorityListFallback,
                TerminationCriteria::standard(),
            )
            .expect("coverage ok")
            .run(2.0)
            .expect("penalty valid")
        };
        let first = run_once(1);
        let second = run_once(2);
        assert_eq!(first, second, "same input → same result (determinism)");
    }

    // --- unused-import guards for the mirror imports -------------------------

    #[test]
    fn the_feasibility_certificate_type_travels_from_the_formulation() {
        // The certificate type is the P8.07 formulation's; the operational
        // layer consumes it as the infeasibility evidence.
        let problem = toy_two_problem();
        let certificate: FeasibilityCertificate = problem.feasibility_certificate();
        assert!(certificate.feasible);
        assert_eq!(certificate.min_achievable_aggregate, -6.0);
    }
}
