// P8.09 — The ADMM/DMPC evaluation harness: the per-case statistics the
//         evaluation report is filled from, computed over INJECTED data
//         (run results, frozen-baseline objectives, validation outcomes,
//         recorded runtimes). Ledger `ledger/phase-08-advanced-
//         coordination/ledger_phase_8.md` §P8.09; report trace §13.2,
//         §19.3–§19.4, §23 Stage 8; decision record:
//         `ledger/phase-08-advanced-coordination/09_admm_evaluation.md`.
//
// ══════════════════════════════════════════════════════════════════════════
// HONESTY HEADER — read before trusting anything below
//
// 1. TEST ARITHMETIC IS NOT AN EXPERIMENT RESULT. Every expected value in
//    this module's test suite is HAND-COMPUTED arithmetic over toy
//    problems, derived on paper IN the comments (e.g. the toy fixed point
//    x = (2, 2) has objective 0.5·2·2² × 2 = 8.0). That is test-grade
//    arithmetic pinning the metric definitions — it is NOT a benchmark
//    measurement, NOT a seed result, and NOT evidence about any §19.3
//    case. NO benchmark figure, runtime, residual statistic, convergence
//    statistic, or objective gap from a real run exists anywhere in this
//    module or its record: every MEASURED cell is EMPTY until the
//    human/lab run fills it (ENTRY_NOTE.md boundary 4).
// 2. WHAT THIS MODULE IS: pure functions that assemble the per-case
//    evaluation statistics the P8.09 report table needs — convergence
//    statistics (iteration counts; primal/dual residuals against the Boyd
//    absolute + relative tolerances the run recorded), objective gaps
//    against the FROZEN P8.06 baseline registry (Tier 1 HiGHS LP/QP/MIP —
//    not SOCP; Tier 2 Clarabel convex conic — not MISOCP; the
//    priority-list fallback record), fallback frequency and
//    staleness-event counts (the P8.08 failure-mode matrix outcomes), and
//    the independent P8.05 validation outcome per case. The inputs are
//    INJECTED: an [`AdmmRunResult`] from the P8.08 operational contract,
//    baseline objectives from the frozen P8.06 results artifacts (which
//    do not exist until the first human/lab run — 05_06_harness_baselines.md
//    §3), the P8.05 harness outcome, and the wall-clock runtime the human
//    run records (this module NEVER measures time — no wall clock
//    anywhere, the determinism discipline).
// 3. WHAT THIS MODULE IS NOT: it is not a convergence guarantee, not an
//    optimality claim, not a threshold authority, and not a
//    success/failure verdict machine. The §19.4 PRECONDITIONS ARE STILL
//    MISSING: the numeric success thresholds (objective-gap bound,
//    tie tolerance, iteration budget as a target) are pre-registered per
//    `02_research_scope_freeze.md` §5 and X-8 — set after the model,
//    service, benchmark, and operational risk are defined, at P8.06
//    freeze time at the earliest, NEVER as universal targets, and NEVER
//    inside this module. Every comparison verdict therefore REQUIRES the
//    pre-registered tie tolerance to be injected
//    ([`compare_to_baseline`] refuses `None` with the named
//    `THRESHOLD_NOT_PRE_REGISTERED` error); the module computes
//    statistics only.
// 4. SOLVER-CLASS DISCIPLINE (VERBATIM — AGENTS.md rule 4; §7.4): ADMM is
//    NEITHER LP/QP/MIP (the HiGHS class) NOR convex conic (the Clarabel
//    class). Its convergence claim rests on the P8.07 convexity
//    assumptions (`crate::admm::ASSUMPTION_*`), carried by reference, not
//    restated here. NO global-convergence or global-optimality claim is
//    made anywhere: for the supported class the two-block ADMM theorem
//    gives residual convergence to the configured tolerances; a finite
//    test set with zero observed violations (or zero non-converged runs)
//    is a TEST OUTCOME, not a guarantee (§13.3; X-4). The objective gap
//    is computed ONLY where meaningful — never against a nonconvex or
//    mixed-integer baseline comparison (the caller declares meaningfulness
//    per baseline; the module refuses non-meaningful gaps with the named
//    `OBJECTIVE_GAP_NOT_MEANINGFUL` error).
// 5. CONTROL BOUNDARY (ADR-010): everything here is evaluation DATA.
//    Nothing authorizes, issues, approves, or dispatches anything; no
//    safety claim; no field authority. Canonical units: MW / MWh; the
//    objective is in the declared class's cost units (per MW of
//    curvature); `P > 0` = export at the point of connection (§10.2).
//
// Honest status: WRITTEN-UNCOMPILED. No cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); every type reference
// was written against source that was read (admm.rs, dmpc.rs, baselines.rs,
// validation.rs). The human build/test gate (AGENTS.md rule 7) owns the
// compile/test evidence.
// ══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;

use serde::Serialize;

use crate::admm::CoupledProblem;
use crate::baselines::{
    results_digest_for, BASELINE_ID_CLARABEL_TIER2, BASELINE_ID_HIGHS_TIER1,
    BASELINE_ID_PRIORITY_LIST,
};
use crate::dmpc::{AdmmRunResult, FinalSchedule, FallbackReason, RunStatus};

/// The evaluation-harness version (a metric-definition change is a NEW
/// version; a definition must not change after results exist — the P8.06
/// frozen-version discipline applied to the evaluation harness itself).
pub const ADMM_EVALUATION_VERSION: &str = "flexcore-research/admm-evaluation.v1";

/// The recorded evaluation statement (the repo's statement pattern —
/// pinned by tests).
pub const EVALUATION_STATEMENT: &str = "the ADMM/DMPC evaluation reports the §19.4 ADMM \
     metrics honestly — per case: primal/dual residual against the recorded Boyd \
     absolute+relative tolerances, iteration count, wall-clock runtime (recorded by the \
     human run, never measured here), communication sensitivity (the staleness/fallback \
     statistics), the objective gap ONLY where meaningful, and the independent P8.05 \
     validation outcome for every reported solution. Losses are reported alongside wins; \
     no case is silently dropped; no verdict exists without a pre-registered threshold; \
     no global-optimality or convergence claim is made beyond the P8.07 convexity \
     assumptions; a finite test set's zero violations is a test outcome, not a \
     guarantee.";

/// The solver-class discipline, VERBATIM (AGENTS.md rule 4; §7.4) — pinned
/// by tests.
pub const SOLVER_CLASS_DISCIPLINE: &str = "ADMM is NEITHER LP/QP/MIP (the HiGHS class) \
     NOR convex conic (the Clarabel class). Its convergence claim rests on the P8.07 \
     convexity assumptions (closed, proper, convex blocks; exact block solves), not on \
     a solver's certificate; no global-convergence or global-optimality claim is made \
     outside the supported class, and AC power flow is not AC-OPF.";

/// The §19.4 pre-registration status, VERBATIM (X-8;
/// `02_research_scope_freeze.md` §5) — pinned by tests.
pub const PRE_REGISTRATION_STATEMENT: &str = "the §19.4 numeric thresholds for this \
     evaluation (objective-gap bound, tie tolerance, any iteration-count target) are \
     NOT SET in this module or in its record: they are pre-registered in writing after \
     the model, service, benchmark, and operational risk are defined — at P8.06 \
     freeze time at the earliest, never as a universal target, and never after \
     research numbers are known. The module computes statistics only; a comparison \
     verdict requires the pre-registered tie tolerance to be injected, and a missing \
     tolerance is the named THRESHOLD_NOT_PRE_REGISTERED refusal.";

/// Where the evaluation cases come from (the §19.3 set + the P8.19
/// extensions). The concrete case list is recorded at the first human/lab
/// run — this module invents no case list and carries no case ids.
pub const BENCHMARK_CASE_SOURCE: &str = "the §19.3 benchmark cases (IEEE feeders, \
     synthetic portfolios, multi-feeder substation constraint, stressed scenarios) via \
     the frozen P8.06 entry point (crate::baselines::BENCHMARK_ENTRY_POINT: \
     apps/sim-runner over scenarios/library lib-01…lib-08) plus the P8.19 versioned \
     research-case extensions; the concrete case list and the benchmark-set version \
     cited by every row are recorded at the first human/lab run (P8.19 owns the \
     versioned artifact)";

/// The measured-cell status, VERBATIM (ENTRY_NOTE boundary 4) — pinned by
/// tests.
pub const MEASURED_CELLS_STATUS: &str = "every measured cell in the P8.09 evaluation \
     record is EMPTY until the human/lab run fills it: no benchmark figure, seed \
     result, runtime, residual statistic, convergence statistic, objective gap, or \
     fallback frequency from a real run exists in this module, its tests, or its \
     record. The tests' hand-computed toy arithmetic (e.g. objective 8.0 at the toy \
     fixed point) pins the metric definitions and is NOT a measurement.";

/// The relative-gap guard: below this |baseline objective| the relative gap
/// is undefined (recorded as `None`, never as a division artifact).
pub const RELATIVE_GAP_EPSILON: f64 = 1.0e-12;

// ---------------------------------------------------------------------------
// Errors (named, fail-closed — an evaluation row that cannot be assembled
// honestly is refused, never guessed)
// ---------------------------------------------------------------------------

/// The named evaluation errors. `PartialEq` only: two variants carry f64
/// values and `Eq` is not derivable over floats.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationError {
    /// The injected baseline id is not in the frozen P8.06 registry — a
    /// comparison against an unfrozen/unrecorded baseline is refused (the
    /// ledger's named risk: a baseline edited after the comparison).
    BaselineUnknown { baseline_id: String },
    /// The injected baseline record carries no version (the frozen-version
    /// discipline: a comparison without a version is not a comparison).
    BaselineVersionMissing { baseline_id: String },
    /// The caller declared this gap not meaningful (e.g. against a nonconvex
    /// comparison) — the gap is refused, never computed "for the table".
    ObjectiveGapNotMeaningful { baseline_id: String },
    /// An objective value is not finite.
    NonFiniteObjective { value: f64 },
    /// A comparison verdict was requested without the pre-registered tie
    /// tolerance (X-8: the module never invents a threshold).
    ThresholdNotPreRegistered,
    /// The tie tolerance is not finite or is negative.
    InvalidTieTolerance { value: f64 },
    /// A reported solution exists but the independent P8.05 validation
    /// outcome was not supplied — the case cannot be reported (ledger
    /// acceptance: "every reported solution passed independent validation").
    ValidationOutcomeMissing,
    /// A schedule exists but no frozen baseline objective was injected —
    /// a schedule without any baseline comparison is not a reported
    /// evaluation row.
    NoBaselineInjected,
    /// A participant of the problem has no setpoint in the adopted
    /// schedule (an assembly mismatch — refused, never silently zeroed).
    SetpointMissing { participant_id: String },
    /// A recorded runtime is not finite or is negative (the human run's
    /// wall-clock measurement; this module never measures time).
    InvalidRuntime { value: f64 },
    /// Statistics over an empty run set are refused (an empty aggregate
    /// would read as a zero frequency — a fabricated-looking figure).
    EmptyRunSet,
}

impl EvaluationError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            EvaluationError::BaselineUnknown { .. } => "BASELINE_UNKNOWN",
            EvaluationError::BaselineVersionMissing { .. } => "BASELINE_VERSION_MISSING",
            EvaluationError::ObjectiveGapNotMeaningful { .. } => {
                "OBJECTIVE_GAP_NOT_MEANINGFUL"
            }
            EvaluationError::NonFiniteObjective { .. } => "NON_FINITE_OBJECTIVE",
            EvaluationError::ThresholdNotPreRegistered => "THRESHOLD_NOT_PRE_REGISTERED",
            EvaluationError::InvalidTieTolerance { .. } => "INVALID_TIE_TOLERANCE",
            EvaluationError::ValidationOutcomeMissing => "VALIDATION_OUTCOME_MISSING",
            EvaluationError::NoBaselineInjected => "NO_BASELINE_INJECTED",
            EvaluationError::SetpointMissing { .. } => "SETPOINT_MISSING",
            EvaluationError::InvalidRuntime { .. } => "INVALID_RUNTIME",
            EvaluationError::EmptyRunSet => "EMPTY_RUN_SET",
        }
    }
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationError::BaselineUnknown { baseline_id } => write!(
                f,
                "baseline {baseline_id} is not in the frozen P8.06 registry; a \
                 comparison against an unfrozen baseline is refused"
            ),
            EvaluationError::BaselineVersionMissing { baseline_id } => write!(
                f,
                "baseline {baseline_id} carries no version; a comparison without a \
                 version is not a comparison (frozen-version discipline)"
            ),
            EvaluationError::ObjectiveGapNotMeaningful { baseline_id } => write!(
                f,
                "the objective gap against {baseline_id} was declared not meaningful \
                 (e.g. a nonconvex/mixed-integer comparison); it is refused, never \
                 computed for the table"
            ),
            EvaluationError::NonFiniteObjective { value } => {
                write!(f, "objective value {value} is not finite")
            }
            EvaluationError::ThresholdNotPreRegistered => write!(
                f,
                "no pre-registered tie tolerance was supplied; the module never \
                 invents a threshold (§19.4; X-8; 02_research_scope_freeze.md §5)"
            ),
            EvaluationError::InvalidTieTolerance { value } => {
                write!(f, "tie tolerance {value} is not finite or is negative")
            }
            EvaluationError::ValidationOutcomeMissing => write!(
                f,
                "a reported solution exists but the independent P8.05 validation \
                 outcome was not supplied; the case cannot be reported"
            ),
            EvaluationError::NoBaselineInjected => write!(
                f,
                "a schedule exists but no frozen baseline objective was injected; a \
                 schedule without any baseline comparison is not an evaluation row"
            ),
            EvaluationError::SetpointMissing { participant_id } => write!(
                f,
                "participant {participant_id} has no setpoint in the adopted schedule; \
                 the mismatch is refused, never silently zeroed"
            ),
            EvaluationError::InvalidRuntime { value } => write!(
                f,
                "recorded runtime {value} s is not finite or is negative; this module \
                 never measures time — the human run records it"
            ),
            EvaluationError::EmptyRunSet => write!(
                f,
                "statistics over an empty run set are refused; an empty aggregate \
                 would read as a zero frequency"
            ),
        }
    }
}

impl std::error::Error for EvaluationError {}

// ---------------------------------------------------------------------------
// The frozen-baseline injection and the objective gap
// ---------------------------------------------------------------------------

/// One frozen baseline's objective for ONE case, injected from the frozen
/// P8.06 results artifact. The artifact does not exist until the first
/// human/lab run (05_06_harness_baselines.md §3) — until then NO instance
/// of this type exists, and no gap is computable (the honest empty state).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BaselineObjectiveInjection {
    /// The frozen baseline id — MUST be one of the registry ids
    /// (`baseline/highs-tier1-lp`, `baseline/clarabel-tier2-conic`,
    /// `baseline/rule-based-priority-list`).
    pub baseline_id: String,
    /// The frozen baseline version the artifact records.
    pub baseline_version: String,
    /// The baseline's objective for the case, in the declared class's cost
    /// units (per MW of curvature).
    pub objective: f64,
    /// Whether the objective gap is MEANINGFUL for this comparison — the
    /// caller declares it per case (never meaningful against a nonconvex or
    /// mixed-integer comparison; the ledger's named risk).
    pub gap_meaningful: bool,
}

/// The objective gap of the adopted schedule against one frozen baseline:
/// `gap = admm_objective − baseline_objective`. For the MINIMIZATION
/// objective, `gap < 0` means the ADMM/DMPC objective is the better
/// (smaller) one; `gap > 0` means the baseline is better. The gap carries
/// NO optimality claim: for the supported convex class it compares two
/// finite computations; for anything else the caller has declared the gap
/// not meaningful and it was refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObjectiveGap {
    /// The frozen baseline id (registry-validated).
    pub baseline_id: String,
    /// The frozen baseline version.
    pub baseline_version: String,
    /// The adopted schedule's objective (cost units).
    pub admm_objective: f64,
    /// The baseline's objective (cost units).
    pub baseline_objective: f64,
    /// `admm_objective − baseline_objective` (cost units; negative = the
    /// ADMM objective is smaller).
    pub gap: f64,
    /// `gap / |baseline_objective|` when defined; `None` when the baseline
    /// objective's magnitude is below [`RELATIVE_GAP_EPSILON`] (undefined,
    /// recorded as such — never a division artifact).
    pub relative_gap: Option<f64>,
}

/// Validates the baseline injection against the FROZEN P8.06 registry
/// (exact id match; version mandatory).
fn validate_baseline_injection(
    baseline: &BaselineObjectiveInjection,
) -> Result<(), EvaluationError> {
    let registry_ids = [
        BASELINE_ID_HIGHS_TIER1,
        BASELINE_ID_CLARABEL_TIER2,
        BASELINE_ID_PRIORITY_LIST,
    ];
    if !registry_ids.iter().any(|id| *id == baseline.baseline_id) {
        return Err(EvaluationError::BaselineUnknown {
            baseline_id: baseline.baseline_id.clone(),
        });
    }
    if baseline.baseline_version.trim().is_empty() {
        return Err(EvaluationError::BaselineVersionMissing {
            baseline_id: baseline.baseline_id.clone(),
        });
    }
    Ok(())
}

/// Computes the objective gap against one injected frozen baseline.
/// Refuses: an id outside the frozen registry, a missing version, a
/// non-finite objective, and a comparison the caller declared not
/// meaningful (`OBJECTIVE_GAP_NOT_MEANINGFUL` — never computed "for the
/// table").
pub fn objective_gap(
    admm_objective: f64,
    baseline: &BaselineObjectiveInjection,
) -> Result<ObjectiveGap, EvaluationError> {
    validate_baseline_injection(baseline)?;
    if !admm_objective.is_finite() || !baseline.objective.is_finite() {
        return Err(EvaluationError::NonFiniteObjective {
            value: if admm_objective.is_finite() {
                baseline.objective
            } else {
                admm_objective
            },
        });
    }
    if !baseline.gap_meaningful {
        return Err(EvaluationError::ObjectiveGapNotMeaningful {
            baseline_id: baseline.baseline_id.clone(),
        });
    }
    let gap = admm_objective - baseline.objective;
    let relative_gap = if baseline.objective.abs() >= RELATIVE_GAP_EPSILON {
        Some(gap / baseline.objective.abs())
    } else {
        None
    };
    Ok(ObjectiveGap {
        baseline_id: baseline.baseline_id.clone(),
        baseline_version: baseline.baseline_version.clone(),
        admm_objective,
        baseline_objective: baseline.objective,
        gap,
        relative_gap,
    })
}

/// The comparison verdict against a frozen baseline. THE VERDICT EXISTS
/// ONLY WITH A PRE-REGISTERED TIE TOLERANCE ([`compare_to_baseline`]
/// refuses `None`): the module never invents a threshold (X-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// `gap < −tie_tolerance`: the ADMM/DMPC objective is smaller (better
    /// for the minimization objective) by more than the pre-registered
    /// tie tolerance.
    Win,
    /// `|gap| ≤ tie_tolerance`: inside the pre-registered tie band.
    Tie,
    /// `gap > tie_tolerance`: the frozen baseline's objective is smaller
    /// — a LOSS, reported alongside the wins (the ledger's honesty
    /// requirement).
    Loss,
}

impl Comparison {
    /// The recorded text form (report-table cell).
    pub fn as_str(self) -> &'static str {
        match self {
            Comparison::Win => "win",
            Comparison::Tie => "tie",
            Comparison::Loss => "loss",
        }
    }
}

/// Compares an objective gap against a frozen baseline using the
/// PRE-REGISTERED tie tolerance (injected from the P8.06 freeze record —
/// NOT a constant here). `None` is the named
/// [`EvaluationError::ThresholdNotPreRegistered`] refusal: no verdict
/// without a pre-registered threshold (§19.4; X-8).
pub fn compare_to_baseline(
    gap: f64,
    tie_tolerance: Option<f64>,
) -> Result<Comparison, EvaluationError> {
    let Some(tolerance) = tie_tolerance else {
        return Err(EvaluationError::ThresholdNotPreRegistered);
    };
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EvaluationError::InvalidTieTolerance { value: tolerance });
    }
    if !gap.is_finite() {
        return Err(EvaluationError::NonFiniteObjective { value: gap });
    }
    if gap < -tolerance {
        Ok(Comparison::Win)
    } else if gap > tolerance {
        Ok(Comparison::Loss)
    } else {
        Ok(Comparison::Tie)
    }
}

// ---------------------------------------------------------------------------
// The objective of an adopted schedule (the declared class's cost)
// ---------------------------------------------------------------------------

/// Computes the objective of a setpoint map over the declared problem:
/// f = Σ_k 0.5 · q_k · (x_k − c_k)², in the declared class's cost units
/// (per MW of curvature). This is the QUADRATIC LOCAL COST of the P8.07
/// formulation — the only objective the supported class declares; the
/// gap against a baseline is meaningful only when the baseline solved the
/// SAME objective (the caller declares meaningfulness per baseline).
/// Every participant must have a setpoint (a mismatch is refused, never
/// silently zeroed); every value must be finite.
pub fn objective_of_setpoints(
    problem: &CoupledProblem,
    setpoints: &BTreeMap<String, f64>,
) -> Result<f64, EvaluationError> {
    let mut total = 0.0;
    for participant in problem.participants() {
        let Some(&x) = setpoints.get(&participant.participant_id) else {
            return Err(EvaluationError::SetpointMissing {
                participant_id: participant.participant_id.clone(),
            });
        };
        if !x.is_finite() {
            return Err(EvaluationError::NonFiniteObjective { value: x });
        }
        let deviation = x - participant.preferred_output_mw;
        total += 0.5 * participant.cost_curvature * deviation * deviation;
    }
    if !total.is_finite() {
        return Err(EvaluationError::NonFiniteObjective { value: total });
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Aggregates over injected run results (the P8.08 outcomes)
// ---------------------------------------------------------------------------

/// The convergence statistics over a set of injected run results (one per
/// case/instance). Every field is a raw count or a deterministic
/// arithmetic aggregate over the INJECTED results — never a target, never
/// a pass/fail verdict (X-8).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConvergenceStatistics {
    /// The runs aggregated.
    pub runs_total: usize,
    /// Runs that terminated at `CONVERGED` (both residual tolerances met
    /// within the iteration budget).
    pub converged: usize,
    /// Runs that exhausted the iteration budget without meeting the
    /// tolerances (NON-CONVERGENCE — reported, never presented as results).
    pub non_converged_budget_exhausted: usize,
    /// Runs refused by the exact pre-iteration feasibility certificate
    /// (zero iterations — distinguishable from non-convergence by
    /// construction, P8.08).
    pub infeasible_certificate: usize,
    /// Runs abandoned on a participant (bounded staleness exceeded).
    pub abandoned_participant_unresponsive: usize,
    /// The smallest `iterations_run` over runs that ran at least one
    /// iteration (zero-iteration certificate runs are excluded from the
    /// min/max/mean — documented; the count above carries them).
    pub iterations_min: Option<u32>,
    /// The largest `iterations_run` over runs that ran at least one
    /// iteration.
    pub iterations_max: Option<u32>,
    /// The mean `iterations_run` over runs that ran at least one iteration
    /// (deterministic arithmetic over the injected values).
    pub iterations_mean: Option<f64>,
    /// Runs whose RE-CHECKED residuals satisfy BOTH recorded tolerances
    /// (`primal ≤ ε_pri` AND `dual ≤ ε_dual`) — recomputed here, not
    /// trusted from the status field, and only over runs that iterated
    /// (a zero-iteration certificate run records 0 residuals against 0
    /// tolerances, which is NOT convergence evidence).
    pub runs_meeting_both_tolerances: usize,
}

/// Aggregates the convergence statistics over the injected run results.
/// Refuses an empty set (an empty aggregate would read as a zero
/// frequency — a fabricated-looking figure).
pub fn convergence_statistics(
    runs: &[AdmmRunResult],
) -> Result<ConvergenceStatistics, EvaluationError> {
    if runs.is_empty() {
        return Err(EvaluationError::EmptyRunSet);
    }
    let mut stats = ConvergenceStatistics {
        runs_total: runs.len(),
        converged: 0,
        non_converged_budget_exhausted: 0,
        infeasible_certificate: 0,
        abandoned_participant_unresponsive: 0,
        iterations_min: None,
        iterations_max: None,
        iterations_mean: None,
        runs_meeting_both_tolerances: 0,
    };
    let mut iterated: Vec<u32> = Vec::new();
    for run in runs {
        match run.status {
            RunStatus::Converged => stats.converged += 1,
            RunStatus::NonConvergedBudgetExhausted => {
                stats.non_converged_budget_exhausted += 1
            }
            RunStatus::InfeasibleCertificate => stats.infeasible_certificate += 1,
            RunStatus::AbandonedParticipantUnresponsive => {
                stats.abandoned_participant_unresponsive += 1
            }
        }
        if run.iterations_run > 0 {
            iterated.push(run.iterations_run);
            // The honest residual re-check: the recorded pair against the
            // recorded tolerances, recomputed here rather than trusted.
            if run.primal_residual <= run.epsilon_primal
                && run.dual_residual <= run.epsilon_dual
            {
                stats.runs_meeting_both_tolerances += 1;
            }
        }
    }
    if !iterated.is_empty() {
        stats.iterations_min = Some(*iterated.iter().min().expect("non-empty"));
        stats.iterations_max = Some(*iterated.iter().max().expect("non-empty"));
        let sum: u64 = iterated.iter().map(|&it| it as u64).sum();
        stats.iterations_mean = Some(sum as f64 / iterated.len() as f64);
    }
    Ok(stats)
}

/// The fallback and staleness statistics over a set of injected run
/// results — the communication-sensitivity accounting (§19.4) assembled
/// from the P8.08 failure-mode outcomes. Raw counts only; a frequency is
/// a division the report table performs from these counts (the counts are
/// the frozen evidence; the division needs no threshold).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FallbackStalenessStatistics {
    /// The runs aggregated.
    pub runs_total: usize,
    /// Runs whose adopted schedule came from the centralized fallback
    /// (recorded fallback outcome present).
    pub runs_with_fallback: usize,
    /// Fallbacks invoked for an infeasibility certificate.
    pub fallbacks_infeasible_certificate: usize,
    /// Fallbacks invoked for a budget-exhausted non-convergence.
    pub fallbacks_non_converged_budget_exhausted: usize,
    /// Fallbacks invoked for an abandoned participant.
    pub fallbacks_participant_abandoned: usize,
    /// Runs where the fallback ITSELF failed (a recorded failure — the
    /// run adopts nothing; counted separately, never merged into the
    /// fallback count).
    pub runs_with_fallback_error: usize,
    /// The largest recorded fallback deficit, MW (`None` when no fallback
    /// recorded a deficit — never a fabricated zero).
    pub max_recorded_deficit_mw: Option<f64>,
    /// Total staleness events across the runs (every bounded-staleness
    /// reuse or abandonment event — visible, never silent, P8.08).
    pub total_stale_events: usize,
    /// Runs with at least one staleness event.
    pub runs_with_stale_values: usize,
    /// The participants named in abandonments (deduplicated, sorted — the
    /// determinism contract).
    pub abandoned_participants: Vec<String>,
}

/// Aggregates the fallback/staleness statistics over the injected run
/// results. Refuses an empty set.
pub fn fallback_staleness_statistics(
    runs: &[AdmmRunResult],
) -> Result<FallbackStalenessStatistics, EvaluationError> {
    if runs.is_empty() {
        return Err(EvaluationError::EmptyRunSet);
    }
    let mut stats = FallbackStalenessStatistics {
        runs_total: runs.len(),
        runs_with_fallback: 0,
        fallbacks_infeasible_certificate: 0,
        fallbacks_non_converged_budget_exhausted: 0,
        fallbacks_participant_abandoned: 0,
        runs_with_fallback_error: 0,
        max_recorded_deficit_mw: None,
        total_stale_events: 0,
        runs_with_stale_values: 0,
        abandoned_participants: Vec::new(),
    };
    for run in runs {
        if run.fallback.is_some() {
            stats.runs_with_fallback += 1;
        }
        if let Some(outcome) = &run.fallback {
            match outcome.reason {
                FallbackReason::InfeasibleCertificate => {
                    stats.fallbacks_infeasible_certificate += 1
                }
                FallbackReason::NonConvergedBudgetExhausted => {
                    stats.fallbacks_non_converged_budget_exhausted += 1
                }
                FallbackReason::ParticipantAbandoned => {
                    stats.fallbacks_participant_abandoned += 1
                }
            }
            if outcome.deficit_mw.is_finite() && outcome.deficit_mw >= 0.0 {
                stats.max_recorded_deficit_mw = Some(
                    stats
                        .max_recorded_deficit_mw
                        .map_or(outcome.deficit_mw, |max| max.max(outcome.deficit_mw)),
                );
            }
        }
        if run.fallback_error.is_some() {
            stats.runs_with_fallback_error += 1;
        }
        stats.total_stale_events += run.stale_events.len();
        if run.used_stale_values {
            stats.runs_with_stale_values += 1;
        }
        if let Some(participant) = &run.abandoned_participant {
            if !stats
                .abandoned_participants
                .iter()
                .any(|existing| existing == participant)
            {
                stats.abandoned_participants.push(participant.clone());
            }
        }
    }
    stats.abandoned_participants.sort();
    Ok(stats)
}

// ---------------------------------------------------------------------------
// The per-case evaluation row
// ---------------------------------------------------------------------------

/// The independent P8.05 validation outcome for ONE reported solution,
/// injected from the harness record. A reported solution WITHOUT an
/// outcome is refused ([`EvaluationError::ValidationOutcomeMissing`]) —
/// the ledger's acceptance criterion ("every reported solution passed
/// independent validation") is enforced at row assembly, not in prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOutcomeRecord {
    /// The P8.05 harness passed the proposal (harness version + the
    /// recorded feasibility evidence reference).
    Passed {
        /// The harness version that validated (P8.05 step 4).
        harness_version: String,
        /// The recorded evidence reference (the feasibility attempt binding).
        evidence_reference: String,
    },
    /// The P8.05 harness refused the proposal (the named refusal travels
    /// into the report — a refusal is a REPORTED outcome, never a silent
    /// drop and never a "win").
    Refused {
        /// The named refusal class (`validation::Refusal`, recorded form).
        refusal_name: String,
    },
    /// Not supplied — a case row carrying a schedule with this outcome is
    /// REFUSED at assembly; a case with NO adopted schedule records it
    /// honestly (nothing was proposed, so nothing was validated).
    NotRun,
}

impl ValidationOutcomeRecord {
    /// The recorded text form (report-table cell).
    pub fn as_str(&self) -> String {
        match self {
            ValidationOutcomeRecord::Passed {
                harness_version,
                evidence_reference,
            } => format!("passed ({harness_version}; {evidence_reference})"),
            ValidationOutcomeRecord::Refused { refusal_name } => {
                format!("refused ({refusal_name})")
            }
            ValidationOutcomeRecord::NotRun => "not_run".to_string(),
        }
    }
}

/// ONE case's evaluation row: the convergence evidence, the objective gaps
/// against the injected frozen baselines, the fallback/staleness record,
/// the independent-validation outcome, and the human-run wall-clock time
/// (injected — this module NEVER measures time). Everything here is
/// assembled from INJECTED data; no cell is invented, and the row is
/// refused rather than assembled dishonestly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdmmCaseEvaluation {
    /// The evaluation-harness version that assembled this row.
    pub evaluation_version: String,
    /// The case id (recorded at the human run — this module invents none).
    pub case_id: String,
    /// The run status (closed P8.08 vocabulary).
    pub run_status_name: String,
    /// Iterations actually run (0 for an infeasibility certificate).
    pub iterations_run: u32,
    /// The final primal residual r = ‖a∘x − z‖.
    pub primal_residual: f64,
    /// The final dual residual s = ρ·‖a∘(z_new − z_old)‖.
    pub dual_residual: f64,
    /// The primal tolerance the comparison used (recorded with the pair).
    pub epsilon_primal: f64,
    /// The dual tolerance the comparison used.
    pub epsilon_dual: f64,
    /// Whether any bounded-staleness reuse happened.
    pub used_stale_values: bool,
    /// The run's staleness events.
    pub stale_event_count: usize,
    /// Whether the adopted schedule came from the centralized fallback.
    pub fallback_used: bool,
    /// The fallback's frozen baseline id, when the fallback answered.
    pub fallback_baseline_id: Option<String>,
    /// The fallback's recorded deficit, MW, when it answered.
    pub fallback_deficit_mw: Option<f64>,
    /// The adopted schedule's objective (cost units), when a schedule was
    /// adopted (a recorded failure adopts nothing — honest absence).
    pub admm_objective: Option<f64>,
    /// The objective gaps against the injected frozen baselines
    /// (meaningful ones only — non-meaningful injections are REFUSED, not
    /// silently skipped).
    pub objective_gaps: Vec<ObjectiveGap>,
    /// The independent P8.05 validation outcome.
    pub validation_outcome: ValidationOutcomeRecord,
    /// The wall-clock runtime the HUMAN run recorded, seconds
    /// (`None` = not yet measured — this module never measures time).
    pub runtime_seconds_reported: Option<f64>,
}

impl AdmmCaseEvaluation {
    /// The canonical report line (fixed field order, `|` separators) — the
    /// deterministic string the results digest is computed over (the P8.06
    /// frozen-artifact discipline, via `baselines::results_digest_for`).
    /// Floats render with `{:.6e}`; `None` renders as `unrecorded`.
    pub fn report_line(&self) -> String {
        let opt_f64 = |value: Option<f64>| match value {
            Some(v) => format!("{v:.6e}"),
            None => "unrecorded".to_string(),
        };
        let gaps = if self.objective_gaps.is_empty() {
            "none".to_string()
        } else {
            self.objective_gaps
                .iter()
                .map(|gap| {
                    format!(
                        "{}@{}:{:.6e}",
                        gap.baseline_id, gap.baseline_version, gap.gap
                    )
                })
                .collect::<Vec<_>>()
                .join(";")
        };
        format!(
            "v:{}|case:{}|status:{}|iters:{}|r:{:.6e}|s:{:.6e}|eps_pri:{:.6e}|eps_dual:{:.6e}|stale:{}|events:{}|fallback:{}|fb_baseline:{}|fb_deficit:{}|obj:{}|gaps:{}|validation:{}|runtime:{}",
            self.evaluation_version,
            self.case_id,
            self.run_status_name,
            self.iterations_run,
            self.primal_residual,
            self.dual_residual,
            self.epsilon_primal,
            self.epsilon_dual,
            self.used_stale_values,
            self.stale_event_count,
            self.fallback_used,
            self.fallback_baseline_id.as_deref().unwrap_or("none"),
            opt_f64(self.fallback_deficit_mw),
            opt_f64(self.admm_objective),
            gaps,
            self.validation_outcome.as_str(),
            opt_f64(self.runtime_seconds_reported),
        )
    }
}

/// Assembles ONE case's evaluation row from injected data.
///
/// Refusals (fail-closed — a row that cannot be assembled honestly is
/// refused, never guessed):
/// - a reported solution (an adopted schedule exists) with
///   [`ValidationOutcomeRecord::NotRun`] → `VALIDATION_OUTCOME_MISSING`;
/// - a reported solution with NO injected baseline objective →
///   `NO_BASELINE_INJECTED`;
/// - a runtime that is not finite or negative → `INVALID_RUNTIME`;
/// - any baseline injection that fails registry validation or is declared
///   not meaningful → the named error (a non-meaningful gap is refused,
///   never skipped silently);
/// - a setpoint mismatch between the problem and the adopted schedule →
///   `SETPOINT_MISSING`.
pub fn evaluate_case(
    case_id: &str,
    problem: &CoupledProblem,
    run: &AdmmRunResult,
    baseline_objectives: &[BaselineObjectiveInjection],
    validation_outcome: ValidationOutcomeRecord,
    runtime_seconds_reported: Option<f64>,
) -> Result<AdmmCaseEvaluation, EvaluationError> {
    if let Some(value) = runtime_seconds_reported {
        if !value.is_finite() || value < 0.0 {
            return Err(EvaluationError::InvalidRuntime { value });
        }
    }

    // The adopted schedule's objective (when a schedule exists).
    let setpoints: Option<BTreeMap<String, f64>> = match run.final_schedule() {
        Some(FinalSchedule::ConvergedAdmm { x }) => Some(x),
        Some(FinalSchedule::CentralizedFallback { outcome }) => {
            let mut map: BTreeMap<String, f64> = problem
                .participant_ids()
                .into_iter()
                .map(|id| (id, 0.0))
                .collect();
            for dispatch in &outcome.dispatches {
                map.insert(dispatch.asset_id.clone(), dispatch.p_mw);
            }
            Some(map)
        }
        None => None,
    };
    let admm_objective = match &setpoints {
        Some(map) => Some(objective_of_setpoints(problem, map)?),
        None => None,
    };

    // The independent-validation gate: a REPORTED solution without a
    // validation outcome is refused (never "passed by omission").
    if setpoints.is_some() && validation_outcome == ValidationOutcomeRecord::NotRun {
        return Err(EvaluationError::ValidationOutcomeMissing);
    }
    // The baseline-comparison gate: a reported solution without any
    // injected frozen baseline is not an evaluation row.
    if setpoints.is_some() && baseline_objectives.is_empty() {
        return Err(EvaluationError::NoBaselineInjected);
    }

    let mut objective_gaps = Vec::new();
    if let Some(objective) = admm_objective {
        for baseline in baseline_objectives {
            objective_gaps.push(objective_gap(objective, baseline)?);
        }
    }

    Ok(AdmmCaseEvaluation {
        evaluation_version: ADMM_EVALUATION_VERSION.to_string(),
        case_id: case_id.to_string(),
        run_status_name: run.status.name().to_string(),
        iterations_run: run.iterations_run,
        primal_residual: run.primal_residual,
        dual_residual: run.dual_residual,
        epsilon_primal: run.epsilon_primal,
        epsilon_dual: run.epsilon_dual,
        used_stale_values: run.used_stale_values,
        stale_event_count: run.stale_events.len(),
        fallback_used: run.schedule_is_from_fallback(),
        fallback_baseline_id: run
            .fallback
            .as_ref()
            .map(|outcome| outcome.baseline_id.to_string()),
        fallback_deficit_mw: run.fallback.as_ref().map(|outcome| outcome.deficit_mw),
        admm_objective,
        objective_gaps,
        validation_outcome,
        runtime_seconds_reported,
    })
}

/// Assembles the results digest over the case rows' canonical report
/// lines (the P8.06 frozen-artifact discipline: the digest over the
/// recorded outputs, computed by the human run and frozen BEFORE any
/// claim cites the rows).
pub fn results_digest(case_rows: &[AdmmCaseEvaluation]) -> Result<String, EvaluationError> {
    if case_rows.is_empty() {
        return Err(EvaluationError::EmptyRunSet);
    }
    let lines: Vec<String> = case_rows.iter().map(|row| row.report_line()).collect();
    Ok(results_digest_for(&lines))
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value is HAND-COMPUTED
// arithmetic derived on paper in the comments. THIS IS TEST ARITHMETIC
// PINNING THE METRIC DEFINITIONS — it is NOT a benchmark measurement, NOT
// a seed result, and NOT evidence about any §19.3 case; no measured cell
// is filled here (ENTRY_NOTE boundary 4).)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admm::{CoupledProblem, Participant};
    use crate::baselines::PriorityListFallback;
    use crate::dmpc::{
        AdmmCoordination, ClosedFormLocalWorker, DmpcHorizonInput, TerminationCriteria,
    };

    /// Demand-floor participant: a_k = −1 (its export counts toward
    /// meeting the aggregate floor b = −d).
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

    /// The toy problem (the P8.07/P8.08 fixed-point toy): two symmetric
    /// participants, q = 2, c = 0, boxes [0, 3], demand d = 4 (b = −4).
    /// HAND-DERIVED: minimize x_a² + x_b² s.t. x_a + x_b ≥ 4 → the
    /// constraint is active, symmetry gives x_a = x_b = 2; the objective
    /// there is 0.5·2·(2−0)² + 0.5·2·(2−0)² = 4.0 + 4.0 = 8.0.
    fn toy_problem(demand_mw: f64) -> CoupledProblem {
        CoupledProblem::new(
            "eval-toy",
            -demand_mw,
            1.0,
            vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("toy problem is valid")
    }

    /// Runs the toy through the P8.08 operational contract (the
    /// hand-traced behaviour: converges EXACTLY in 2 iterations to
    /// x = (2, 2), r = s = 0 — pinned in the dmpc.rs suite).
    fn toy_run(demand_mw: f64) -> AdmmRunResult {
        let problem = toy_problem(demand_mw);
        let workers: Vec<ClosedFormLocalWorker> = problem
            .participants()
            .into_iter()
            .map(|p| ClosedFormLocalWorker {
                participant: p.clone(),
            })
            .collect();
        let mut map: BTreeMap<String, &dyn crate::dmpc::AdmmLocalWorker> = BTreeMap::new();
        for worker in &workers {
            map.insert(worker.participant.participant_id.clone(), worker);
        }
        let coordination = AdmmCoordination::new(
            &problem,
            map,
            &PriorityListFallback,
            TerminationCriteria::standard(),
        )
        .expect("toy wiring is complete");
        coordination.run(2.0).expect("toy run is valid")
    }

    /// A hand-built run result for the aggregate-statistics tests (no
    /// coordination run needed — the aggregates consume the fields).
    fn hand_run(
        status: RunStatus,
        iterations: u32,
        primal: f64,
        dual: f64,
        eps_primal: f64,
        eps_dual: f64,
        stale_events: usize,
        fallback: Option<crate::dmpc::FallbackOutcome>,
    ) -> AdmmRunResult {
        AdmmRunResult {
            operations_version: crate::dmpc::ADMM_OPERATIONS_VERSION.to_string(),
            status,
            iterations_run: iterations,
            primal_residual: primal,
            dual_residual: dual,
            epsilon_primal: eps_primal,
            epsilon_dual: eps_dual,
            distributed_x_mw: BTreeMap::new(),
            termination: TerminationCriteria::standard(),
            used_stale_values: stale_events > 0,
            stale_events: (0..stale_events)
                .map(|index| crate::dmpc::StaleEvent {
                    iteration: (index + 1) as u32,
                    participant_id: "a".to_string(),
                    reused_previous_value: true,
                    detail: "hand-built test event".to_string(),
                })
                .collect(),
            abandoned_participant: None,
            fallback,
            fallback_error: None,
        }
    }

    /// The frozen Tier-1 injection used in the toy tests (objective value
    /// HAND-CHOSEN for the gap arithmetic — NOT a measured baseline; the
    /// real frozen artifact does not exist until the first human run).
    fn tier1_injection(objective: f64) -> BaselineObjectiveInjection {
        BaselineObjectiveInjection {
            baseline_id: BASELINE_ID_HIGHS_TIER1.to_string(),
            baseline_version: "v1".to_string(),
            objective,
            gap_meaningful: true,
        }
    }

    #[test]
    fn statement_constants_carry_the_required_discipline() {
        // Solver-class discipline VERBATIM (AGENTS.md rule 4).
        assert!(SOLVER_CLASS_DISCIPLINE.contains("NEITHER LP/QP/MIP"));
        assert!(SOLVER_CLASS_DISCIPLINE.contains("NOR convex conic"));
        assert!(SOLVER_CLASS_DISCIPLINE.contains("AC power flow is not AC-OPF"));
        // Pre-registration (X-8): no threshold is set here.
        assert!(PRE_REGISTRATION_STATEMENT.contains("NOT SET"));
        assert!(PRE_REGISTRATION_STATEMENT.contains("never as a universal target"));
        assert!(PRE_REGISTRATION_STATEMENT.contains("THRESHOLD_NOT_PRE_REGISTERED"));
        // The honesty boundary (§13.3; X-4).
        assert!(EVALUATION_STATEMENT.contains("test outcome, not a guarantee."));
        assert!(EVALUATION_STATEMENT.contains("Losses are reported alongside wins"));
        // The measured-cells honesty (ENTRY_NOTE boundary 4).
        assert!(MEASURED_CELLS_STATUS.contains("EMPTY until the human/lab run"));
        assert!(MEASURED_CELLS_STATUS.contains("NOT a measurement"));
    }

    #[test]
    fn toy_converged_run_objective_and_gap_arithmetic() {
        // HAND-TRACED (dmpc.rs suite, row 1): the toy converges exactly in
        // 2 iterations to x = (2, 2), r = s = 0.
        let problem = toy_problem(4.0);
        let run = toy_run(4.0);
        assert_eq!(run.status, RunStatus::Converged);
        assert_eq!(run.iterations_run, 2);
        assert_eq!(run.distributed_x_mw.get("a"), Some(&2.0));
        assert_eq!(run.distributed_x_mw.get("b"), Some(&2.0));

        // HAND-COMPUTED objective at x = (2, 2): 0.5·2·(2−0)² + 0.5·2·(2−0)²
        // = 4.0 + 4.0 = 8.0. TEST ARITHMETIC, not a measurement.
        let setpoints = run.distributed_x_mw.clone();
        let objective = objective_of_setpoints(&problem, &setpoints)
            .expect("toy setpoints cover every participant");
        assert_eq!(objective, 8.0);

        // Gap vs an injected baseline at the SAME objective: exactly 0.0.
        let gap_zero = objective_gap(8.0, &tier1_injection(8.0)).expect("meaningful");
        assert_eq!(gap_zero.gap, 0.0);
        assert_eq!(gap_zero.relative_gap, Some(0.0));
        // Gap vs an injected baseline at 10.0: 8.0 − 10.0 = −2.0 (the
        // ADMM objective is smaller); relative = −2.0 / 10.0 = −0.2.
        let gap_better = objective_gap(8.0, &tier1_injection(10.0)).expect("meaningful");
        assert_eq!(gap_better.gap, -2.0);
        assert_eq!(gap_better.relative_gap, Some(-0.2));
        // Gap vs an injected baseline at 6.0: 8.0 − 6.0 = +2.0 (a LOSS —
        // reported, not hidden).
        let gap_worse = objective_gap(8.0, &tier1_injection(6.0)).expect("meaningful");
        assert_eq!(gap_worse.gap, 2.0);

        // Verdicts REQUIRE the pre-registered tie tolerance; with the
        // INJECTED test tolerance 0.5 (test arithmetic, NOT a registered
        // threshold): gap 0.0 → tie; −2.0 < −0.5 → win; +2.0 > 0.5 → loss.
        assert_eq!(compare_to_baseline(0.0, Some(0.5)), Ok(Comparison::Tie));
        assert_eq!(compare_to_baseline(-2.0, Some(0.5)), Ok(Comparison::Win));
        assert_eq!(compare_to_baseline(2.0, Some(0.5)), Ok(Comparison::Loss));
        assert_eq!(
            compare_to_baseline(0.0, None),
            Err(EvaluationError::ThresholdNotPreRegistered)
        );
        assert_eq!(
            compare_to_baseline(0.0, Some(-1.0)),
            Err(EvaluationError::InvalidTieTolerance { value: -1.0 })
        );
    }

    #[test]
    fn toy_infeasible_run_fallback_objective_arithmetic() {
        // HAND-TRACED (dmpc.rs suite, row 3): demand 10 over boxes [0, 3]²
        // → the certificate fails (min achievable aggregate −6 > b = −10),
        // zero iterations, the frozen priority-list fallback supplies
        // 6 MW with a recorded 4 MW deficit — committed x = (3, 3).
        let problem = toy_problem(10.0);
        let run = toy_run(10.0);
        assert_eq!(run.status, RunStatus::InfeasibleCertificate);
        assert_eq!(run.iterations_run, 0);
        let outcome = run.fallback.as_ref().expect("fallback recorded");
        assert_eq!(outcome.supplied_mw, 6.0);
        assert_eq!(outcome.deficit_mw, 4.0);

        // HAND-COMPUTED objective of the fallback schedule x = (3, 3):
        // 0.5·2·(3−0)² + 0.5·2·(3−0)² = 9.0 + 9.0 = 18.0. TEST ARITHMETIC.
        let row = evaluate_case(
            "toy-infeasible",
            &problem,
            &run,
            &[tier1_injection(18.0)],
            ValidationOutcomeRecord::Passed {
                harness_version: crate::validation::HARNESS_VERSION.to_string(),
                evidence_reference: "evidence-test".to_string(),
            },
            None,
        )
        .expect("the infeasible row assembles honestly");
        assert_eq!(row.admm_objective, Some(18.0));
        assert!(row.fallback_used);
        assert_eq!(
            row.fallback_baseline_id.as_deref(),
            Some(BASELINE_ID_PRIORITY_LIST)
        );
        assert_eq!(row.fallback_deficit_mw, Some(4.0));
        assert_eq!(row.objective_gaps.len(), 1);
        assert_eq!(row.objective_gaps[0].gap, 0.0);

        // A reported solution with NO validation outcome is REFUSED.
        assert_eq!(
            evaluate_case(
                "toy-infeasible",
                &problem,
                &run,
                &[tier1_injection(18.0)],
                ValidationOutcomeRecord::NotRun,
                None,
            ),
            Err(EvaluationError::ValidationOutcomeMissing)
        );
        // A reported solution with NO injected baseline is refused too.
        assert_eq!(
            evaluate_case(
                "toy-infeasible",
                &problem,
                &run,
                &[],
                ValidationOutcomeRecord::Passed {
                    harness_version: "h".to_string(),
                    evidence_reference: "e".to_string(),
                },
                None,
            ),
            Err(EvaluationError::NoBaselineInjected)
        );
    }

    #[test]
    fn convergence_statistics_aggregate_arithmetic() {
        // HAND-BUILT: three runs with iterations 2, 5, 3 (one of them
        // non-converged). min 2, max 5, mean 10/3; converged 2; budget-
        // exhausted 1; every run iterated so every residual pair is
        // re-checked (two of the three meet their recorded tolerances —
        // hand-chosen pairs below).
        let runs = vec![
            hand_run(RunStatus::Converged, 2, 1.0e-9, 1.0e-9, 1.0e-6, 1.0e-6, 0, None),
            hand_run(
                RunStatus::NonConvergedBudgetExhausted,
                5,
                0.5,
                0.25,
                1.0e-6,
                1.0e-6,
                0,
                None,
            ),
            hand_run(RunStatus::Converged, 3, 2.0e-7, 3.0e-7, 1.0e-6, 1.0e-6, 0, None),
        ];
        let stats = convergence_statistics(&runs).expect("non-empty");
        assert_eq!(stats.runs_total, 3);
        assert_eq!(stats.converged, 2);
        assert_eq!(stats.non_converged_budget_exhausted, 1);
        assert_eq!(stats.infeasible_certificate, 0);
        assert_eq!(stats.abandoned_participant_unresponsive, 0);
        assert_eq!(stats.iterations_min, Some(2));
        assert_eq!(stats.iterations_max, Some(5));
        assert_eq!(stats.iterations_mean, Some(10.0 / 3.0));
        assert_eq!(stats.runs_meeting_both_tolerances, 2);

        // Zero-iteration certificate runs are excluded from the min/max/
        // mean (documented) but counted in the status counts.
        let with_certificate = vec![
            hand_run(RunStatus::InfeasibleCertificate, 0, 0.0, 0.0, 0.0, 0.0, 0, None),
            hand_run(RunStatus::Converged, 4, 1.0e-9, 1.0e-9, 1.0e-6, 1.0e-6, 0, None),
        ];
        let stats = convergence_statistics(&with_certificate).expect("non-empty");
        assert_eq!(stats.infeasible_certificate, 1);
        assert_eq!(stats.iterations_min, Some(4));
        assert_eq!(stats.iterations_max, Some(4));
        assert_eq!(stats.iterations_mean, Some(4.0));
        // The certificate run's 0-vs-0 residual pair is NOT counted as
        // convergence evidence.
        assert_eq!(stats.runs_meeting_both_tolerances, 1);

        // An empty set is refused.
        assert_eq!(
            convergence_statistics(&[]),
            Err(EvaluationError::EmptyRunSet)
        );
    }

    #[test]
    fn fallback_and_staleness_aggregate_arithmetic() {
        // HAND-BUILT: four runs — one budget-exhausted fallback with a 4.0
        // MW deficit and 1 stale event, one abandoned fallback with 2
        // stale events naming participant "b", one clean converged run,
        // one fallback-ERROR run (recorded failure). Hand-summed counts:
        // fallbacks 2, per-reason (budget 1, abandoned 1, infeasible 0);
        // fallback errors 1; stale events 1 + 2 = 3; runs-with-stale 2;
        // max deficit 4.0.
        let fallback_budget_exhausted = crate::dmpc::FallbackOutcome {
            baseline_id: BASELINE_ID_PRIORITY_LIST,
            baseline_version: "priority-list.v1".to_string(),
            reason: FallbackReason::NonConvergedBudgetExhausted,
            dispatches: Vec::new(),
            supplied_mw: 4.0,
            deficit_mw: 4.0,
        };
        let fallback_abandoned = crate::dmpc::FallbackOutcome {
            baseline_id: BASELINE_ID_PRIORITY_LIST,
            baseline_version: "priority-list.v1".to_string(),
            reason: FallbackReason::ParticipantAbandoned,
            dispatches: Vec::new(),
            supplied_mw: 0.0,
            deficit_mw: 2.5,
        };
        let mut abandoned = hand_run(
            RunStatus::AbandonedParticipantUnresponsive,
            3,
            0.4,
            0.2,
            1.0e-6,
            1.0e-6,
            2,
            Some(fallback_abandoned),
        );
        abandoned.abandoned_participant = Some("b".to_string());
        let mut error_run =
            hand_run(RunStatus::Converged, 1, 0.0, 0.0, 1.0e-6, 1.0e-6, 0, None);
        error_run.status = RunStatus::NonConvergedBudgetExhausted;
        error_run.fallback_error = Some("FALLBACK_CLASS_NOT_SUPPORTED".to_string());

        let runs = vec![
            hand_run(
                RunStatus::NonConvergedBudgetExhausted,
                7,
                0.9,
                0.8,
                1.0e-6,
                1.0e-6,
                1,
                Some(fallback_budget_exhausted),
            ),
            abandoned,
            hand_run(RunStatus::Converged, 2, 1.0e-9, 1.0e-9, 1.0e-6, 1.0e-6, 0, None),
            error_run,
        ];
        let stats = fallback_staleness_statistics(&runs).expect("non-empty");
        assert_eq!(stats.runs_total, 4);
        assert_eq!(stats.runs_with_fallback, 2);
        assert_eq!(stats.fallbacks_non_converged_budget_exhausted, 1);
        assert_eq!(stats.fallbacks_participant_abandoned, 1);
        assert_eq!(stats.fallbacks_infeasible_certificate, 0);
        assert_eq!(stats.runs_with_fallback_error, 1);
        assert_eq!(stats.max_recorded_deficit_mw, Some(4.0));
        assert_eq!(stats.total_stale_events, 3);
        assert_eq!(stats.runs_with_stale_values, 2);
        assert_eq!(stats.abandoned_participants, vec!["b".to_string()]);
        assert_eq!(
            fallback_staleness_statistics(&[]),
            Err(EvaluationError::EmptyRunSet)
        );
    }

    #[test]
    fn baseline_injection_validation_refusals() {
        // Unknown id — refused.
        let unknown = BaselineObjectiveInjection {
            baseline_id: "baseline/some-new-baseline".to_string(),
            baseline_version: "v1".to_string(),
            objective: 8.0,
            gap_meaningful: true,
        };
        assert_eq!(
            objective_gap(8.0, &unknown),
            Err(EvaluationError::BaselineUnknown {
                baseline_id: "baseline/some-new-baseline".to_string()
            })
        );
        // Missing version — refused.
        let no_version = BaselineObjectiveInjection {
            baseline_id: BASELINE_ID_HIGHS_TIER1.to_string(),
            baseline_version: "  ".to_string(),
            objective: 8.0,
            gap_meaningful: true,
        };
        assert_eq!(
            objective_gap(8.0, &no_version),
            Err(EvaluationError::BaselineVersionMissing {
                baseline_id: BASELINE_ID_HIGHS_TIER1.to_string()
            })
        );
        // Declared NOT meaningful (e.g. a nonconvex comparison) — refused,
        // never computed for the table.
        let not_meaningful = BaselineObjectiveInjection {
            baseline_id: BASELINE_ID_CLARABEL_TIER2.to_string(),
            baseline_version: "v1".to_string(),
            objective: 8.0,
            gap_meaningful: false,
        };
        assert_eq!(
            objective_gap(8.0, &not_meaningful),
            Err(EvaluationError::ObjectiveGapNotMeaningful {
                baseline_id: BASELINE_ID_CLARABEL_TIER2.to_string()
            })
        );
        // Non-finite objective — refused.
        let non_finite = BaselineObjectiveInjection {
            baseline_id: BASELINE_ID_HIGHS_TIER1.to_string(),
            baseline_version: "v1".to_string(),
            objective: f64::NAN,
            gap_meaningful: true,
        };
        assert!(matches!(
            objective_gap(8.0, &non_finite),
            Err(EvaluationError::NonFiniteObjective { .. })
        ));
        // Relative gap undefined below the epsilon guard: baseline
        // objective 1.0e-15 < 1.0e-12 → None (never a division artifact).
        let tiny = tier1_injection(1.0e-15);
        let gap = objective_gap(2.0, &tiny).expect("meaningful");
        assert_eq!(gap.gap, 2.0);
        assert_eq!(gap.relative_gap, None);
    }

    #[test]
    fn setpoint_mismatch_and_runtime_refusals() {
        let problem = toy_problem(4.0);
        let run = toy_run(4.0);
        // The converged toy schedule covers both participants, so drop one
        // via a hand-built partial map to exercise the refusal path — done
        // through evaluate_case with a mismatched PROBLEM instead: build a
        // problem with an extra participant absent from the schedule.
        let bigger_problem = CoupledProblem::new(
            "eval-toy-3",
            -4.0,
            1.0,
            vec![
                demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                demand_participant("b", 2.0, 0.0, 0.0, 3.0),
                demand_participant("c", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("valid");
        // The toy run's schedule has only a and b; c is missing → refused.
        assert!(matches!(
            evaluate_case(
                "toy-mismatch",
                &bigger_problem,
                &run,
                &[tier1_injection(8.0)],
                ValidationOutcomeRecord::Passed {
                    harness_version: "h".to_string(),
                    evidence_reference: "e".to_string(),
                },
                None,
            ),
            Err(EvaluationError::SetpointMissing { .. })
        ));
        // A negative recorded runtime is refused (this module never
        // measures time; the human run's record must be sane).
        assert_eq!(
            evaluate_case(
                "toy-negative-runtime",
                &problem,
                &run,
                &[tier1_injection(8.0)],
                ValidationOutcomeRecord::Passed {
                    harness_version: "h".to_string(),
                    evidence_reference: "e".to_string(),
                },
                Some(-1.0),
            ),
            Err(EvaluationError::InvalidRuntime { value: -1.0 })
        );
    }

    #[test]
    fn report_line_and_digest_determinism() {
        // The same assembled row twice → byte-identical report lines and
        // the same digest (determinism, the P8.06 frozen-artifact
        // discipline).
        let problem = toy_problem(4.0);
        let run = toy_run(4.0);
        let build = || {
            evaluate_case(
                "toy-converged",
                &problem,
                &run,
                &[tier1_injection(8.0)],
                ValidationOutcomeRecord::Passed {
                    harness_version: crate::validation::HARNESS_VERSION.to_string(),
                    evidence_reference: "evidence-test".to_string(),
                },
                None,
            )
            .expect("row assembles")
        };
        let row_a = build();
        let row_b = build();
        assert_eq!(row_a, row_b);
        assert_eq!(row_a.report_line(), row_b.report_line());
        let digest_a = results_digest(&[row_a.clone()]).expect("non-empty");
        let digest_b = results_digest(&[row_b]).expect("non-empty");
        assert_eq!(digest_a, digest_b);
        // The objective gap travelled into the line (the frozen Tier-1
        // record id, the 0.0 gap).
        assert!(row_a.report_line().contains("baseline/highs-tier1-lp@v1:0.000000"));
        // The runtime cell is honestly unrecorded (never measured here).
        assert!(row_a.report_line().contains("|runtime:unrecorded"));
        // An empty digest is refused.
        assert_eq!(
            results_digest(&[]),
            Err(EvaluationError::EmptyRunSet)
        );
    }
}
