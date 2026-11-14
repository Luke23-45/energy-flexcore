// P4.37 — Constrain/return consumption contract (report §12.5 step 8,
// §12.6; ledger P4.37).
//
// CLOSES THE OPEN LOOP: the feasibility gate (flexcore-grid-api, P4.07)
// routes an unacceptable candidate to EXACTLY ONE of REJECTED / CONSTRAINED
// / RETURNED-FOR-RE-OPTIMIZATION (plus INCONCLUSIVE and HELD) by the frozen
// routing rule D-P4.07-1. Until this module, nothing defined what the
// scheduling side DOES with a CONSTRAIN or RETURN outcome — the risk the
// ledger names is "RETURN treated as REJECTED everywhere, hiding the
// re-optimization path". This module is the consumption contract: it maps
// every gate outcome to the consumer action, so a returned candidate is
// re-optimized with its violated constraints as named INPUTS instead of
// silently degrading to a rejection.
//
// ## Why the outcome vocabulary is MIRRORED here, not imported
//
// The verdict types (`GateVerdict`, `VerdictOutcome`) and the limit
// categories (`LimitCategory`) live in flexcore-grid-api — which is NOT an
// allowed dependency edge for flexcore-application
// (tools/check_dependency_rule.py: the application layer depends on
// contracts/domain/ports/events + the optimization-api contract crates
// only). The composition root converts the grid verdict into this module's
// `ReturnedCandidate` — the SAME pattern the P4.12 command gate already
// uses (`command_gates::FeasibilityGateVerdict` is a local shape the
// composition root converts; the "application-side mirror" precedent).
// What travels across that seam are the FROZEN string codes
// (`VerdictOutcome::as_str`, "frozen for evidence consumers" per the grid
// crate) and the frozen constraint-name grammar
// `<category_code>:<element_id>` (`LimitCheckRecord::constraint_name`).
// Both are mirrored here with the identical strings, pinned by tests, so
// drift fails visibly — the P3.24 trigger-string precedent.
//
// ## The contract (decided once, D-P4.37-1)
//
// - **CONSTRAIN** → the violated constraint and the constraining proposal
//   (EVIDENCE, not an applied mutation — D-P4.07-2) are attached as
//   problem inputs of the constrained candidate, which MUST re-enter the
//   feasibility gate from step 1. This is a re-run of the SAME candidate
//   with the recorded proposal, not a scheduler re-optimization.
// - **RETURN** → the candidate is re-enqueued into rolling-horizon
//   re-optimization (P3.24) with EVERY violated constraint as a named
//   input annotation. The gate never optimizes; the re-dispatch decision
//   belongs to the scheduler.
// - **REJECTED** → no consumption: the D-P4.07-1 routing is a rejection;
//   the command lifecycle owns it (P4.12: VALIDATED → REJECTED with the
//   named constraint).
// - **HELD** → no consumption: parked pending operator release, operator
//   only, with a recorded reason (the P4.08 release policy).
// - **INCONCLUSIVE** → no consumption: retry policy NONE (D-P4.05-2).
// - **WITHIN-LIMITS** → no consumption: the candidate proceeds through
//   the command lifecycle (validation, feasibility, authorization are
//   distinct gates, §14.1).
//
// ## The failure mode (decided once, D-P4.37-2)
//
// If a returned candidate cannot be consumed — its constraint names do
// not parse into the frozen grammar, or name a constraint category this
// contract has no problem-input mapping for — the candidate is REJECTED
// with the reason RECORDED. Never silently dropped, never passed
// (§12.5 step 8; the P4.07 "never silently adjust a candidate and pass
// it" rule, consumed side).
//
// ## Open items, recorded honestly
//
// 1. **Closed-loop verification is Phase 5.** The full loop (feasibility
//    return → re-optimized schedule → re-validation through the gate) is
//    the P5.18–P5.19 scenario test, not a Phase 4 unit test. The Phase 4
//    contract test proves the consumption SIDE (a stub consumer shape in
//    tests); the real consumer (P3.24 rolling horizon) is NOT modified
//    here.
// 2. **The roll-trigger gap.** The frozen P3.24 roll-trigger set
//    (`SCHEDULE_ROLL_TRIGGER_STRINGS`: initial_solve, horizon_advance,
//    envelope_invalidation, asset_opt_out_dropout, forecast_update,
//    operator_request) has NO feasibility-return trigger. A
//    `ReOptimizationInput` produced here is the CONTRACT the next roll
//    consumes; wiring it as a new stored trigger string is a recorded
//    decision owned jointly with Phase 3 (the flexcore-storage
//    vocabulary is frozen and NOT modified by this subtask). Until that
//    decision, the consumer carries the annotation set and the trigger
//    gap travels with it ([`ReOptimizationInput::roll_trigger_note`]).
//
// Control status: pure application-layer contract for the simulation-only
// 0.x line. No solver call, no I/O, no dispatch path; a consumed
// candidate is a CANDIDATE again — never feasible, approved, or
// dispatchable without the full §14.1 gate chain.

// ---------------------------------------------------------------------------
// Recorded statements
// ---------------------------------------------------------------------------

/// The consumption contract, stated once (D-P4.37-1).
pub const CONSUMPTION_CONTRACT_STATEMENT: &str = "Consumption contract \
     (D-P4.37-1): CONSTRAIN attaches the violated constraint and the \
     constraining proposal (evidence, not an applied mutation) as problem \
     inputs and the constrained candidate re-enters the feasibility gate \
     from step 1; RETURN re-enqueues the candidate into rolling-horizon \
     re-optimization with EVERY violated constraint as a named input; \
     REJECTED/HELD/INCONCLUSIVE/WITHIN-LIMITS have no scheduling-side \
     consumption and route per the frozen rule D-P4.07-1. The returned \
     candidate is a CANDIDATE again — the full gate chain applies.";

/// The failure mode, stated once (D-P4.37-2).
pub const FAILURE_MODE_STATEMENT: &str = "Failure mode (D-P4.37-2): a \
     returned candidate whose constraints cannot be consumed (an \
     unparseable constraint name, or a constraint category this contract \
     has no problem-input mapping for) is REJECTED with the reason \
     RECORDED — never silently dropped, never passed.";

/// The open-item statement (ledger P4.37 step 4).
pub const OPEN_ITEM_STATEMENT: &str = "Open items (ledger P4.37 step 4): \
     (1) full closed-loop verification (feasibility return → re-optimized \
     schedule → re-validation) is the P5.18–P5.19 scenario test, not a \
     Phase 4 unit test; (2) the frozen P3.24 roll-trigger set has no \
     feasibility-return trigger — wiring the ReOptimizationInput as a new \
     stored trigger string is a recorded decision owned jointly with \
     Phase 3 (flexcore-storage SCHEDULE_ROLL_TRIGGERS is frozen, not \
     modified here).";

/// Why the outcome codes are mirrored (the dependency-rule statement).
pub const MIRROR_STATEMENT: &str = "The verdict types live in \
     flexcore-grid-api, which is NOT an allowed dependency edge for \
     flexcore-application (the dependency rule; the composition root \
     converts). This module mirrors the FROZEN outcome codes and the \
     frozen constraint-name grammar as local constants, pinned by tests \
     so drift fails visibly (the P3.24 trigger-string precedent).";

// ---------------------------------------------------------------------------
// The mirrored outcome vocabulary (frozen codes — pinned by tests)
// ---------------------------------------------------------------------------

/// The frozen gate outcome codes (`flexcore_grid_api::verdict::
/// VerdictOutcome::as_str`), mirrored verbatim.
pub const GATE_OUTCOME_CODES: [&str; 6] = [
    "within_limits",
    "rejected",
    "constrained",
    "returned_for_re_optimization",
    "inconclusive",
    "held",
];

/// The gate outcome, as the scheduling side consumes it (the local mirror;
/// see [`MIRROR_STATEMENT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeasibilityOutcome {
    /// Every checked limit passed (evidence-backed against the DECLARED
    /// model — never a field-safety or approval claim).
    WithinLimits,
    /// The candidate is rejected (hard violation, candidate fault, or a
    /// REJECT-class fail-closed trigger).
    Rejected,
    /// The gate proposes a constraining adjustment — evidence, not an
    /// applied mutation; the constrained candidate re-enters the gate.
    Constrained,
    /// The candidate returns to scheduling for re-optimization with the
    /// violated constraints as named inputs.
    ReturnedForReOptimization,
    /// Non-convergence / provider error / incomplete evidence; retry
    /// policy NONE.
    Inconclusive,
    /// A HOLD-class fail-closed trigger: parked pending operator release.
    Held,
}

impl FeasibilityOutcome {
    /// The frozen outcome code (must equal the grid crate's `as_str`).
    pub const fn outcome_code(self) -> &'static str {
        match self {
            FeasibilityOutcome::WithinLimits => "within_limits",
            FeasibilityOutcome::Rejected => "rejected",
            FeasibilityOutcome::Constrained => "constrained",
            FeasibilityOutcome::ReturnedForReOptimization => "returned_for_re_optimization",
            FeasibilityOutcome::Inconclusive => "inconclusive",
            FeasibilityOutcome::Held => "held",
        }
    }

    /// Parses the frozen outcome code. `None` for an unknown code — the
    /// vocabulary is closed, and an unknown code is never silently mapped
    /// to a default.
    pub fn from_outcome_code(code: &str) -> Option<Self> {
        match code {
            "within_limits" => Some(FeasibilityOutcome::WithinLimits),
            "rejected" => Some(FeasibilityOutcome::Rejected),
            "constrained" => Some(FeasibilityOutcome::Constrained),
            "returned_for_re_optimization" => Some(FeasibilityOutcome::ReturnedForReOptimization),
            "inconclusive" => Some(FeasibilityOutcome::Inconclusive),
            "held" => Some(FeasibilityOutcome::Held),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// The mirrored constraint-name grammar (frozen — pinned by tests)
// ---------------------------------------------------------------------------

/// The frozen constraint-category codes
/// (`flexcore_grid_api::limits::LimitCategory::code`), mirrored verbatim.
pub const CONSTRAINT_CATEGORY_CODES: [&str; 5] = [
    "bus_voltage_lower",
    "bus_voltage_upper",
    "branch_thermal",
    "transformer_loading",
    "generator_apparent_power",
];

/// The constraint category of a violated constraint (the local mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintCategory {
    /// Bus voltage below the configured lower band (per-unit).
    BusVoltageLower,
    /// Bus voltage above the configured upper band (per-unit).
    BusVoltageUpper,
    /// Branch current/thermal loading above the operational limit.
    BranchThermal,
    /// Transformer loading above the operational limit.
    TransformerLoading,
    /// Generator/inverter apparent power above the declared S_max.
    GeneratorApparentPower,
}

impl ConstraintCategory {
    /// The frozen category code (must equal the grid crate's `code()`).
    pub const fn code(self) -> &'static str {
        match self {
            ConstraintCategory::BusVoltageLower => "bus_voltage_lower",
            ConstraintCategory::BusVoltageUpper => "bus_voltage_upper",
            ConstraintCategory::BranchThermal => "branch_thermal",
            ConstraintCategory::TransformerLoading => "transformer_loading",
            ConstraintCategory::GeneratorApparentPower => "generator_apparent_power",
        }
    }

    /// Parses a frozen category code. `None` for an unknown code — an
    /// unknown category is the D-P4.37-2 failure mode, never a default.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "bus_voltage_lower" => Some(ConstraintCategory::BusVoltageLower),
            "bus_voltage_upper" => Some(ConstraintCategory::BusVoltageUpper),
            "branch_thermal" => Some(ConstraintCategory::BranchThermal),
            "transformer_loading" => Some(ConstraintCategory::TransformerLoading),
            "generator_apparent_power" => Some(ConstraintCategory::GeneratorApparentPower),
            _ => None,
        }
    }
}

/// The errors of the consumption contract (structural — a malformed
/// `ReturnedCandidate` record is a CALLER BUG and fails loudly; a
/// semantically unconsumable constraint routes
/// [`ConsumptionAction::RejectUnconsumable`] instead, per D-P4.37-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeasibilityReturnError {
    /// A required reference or field is empty (`field` names it).
    EmptyField { field: &'static str },
    /// The schedule revision is zero (the first revision is 1).
    RevisionZero,
    /// The record is structurally incomplete for its outcome
    /// (`detail` says what is missing).
    IncompleteEvidence { detail: String },
}

impl std::fmt::Display for FeasibilityReturnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeasibilityReturnError::EmptyField { field } => {
                write!(f, "returned-candidate field '{field}' is empty")
            }
            FeasibilityReturnError::RevisionZero => {
                write!(f, "the schedule revision must be >= 1")
            }
            FeasibilityReturnError::IncompleteEvidence { detail } => {
                write!(f, "the returned candidate is incomplete: {detail}")
            }
        }
    }
}

impl std::error::Error for FeasibilityReturnError {}

// ---------------------------------------------------------------------------
// The contract types
// ---------------------------------------------------------------------------

/// The constraining proposal, as the scheduling side receives it (the
/// mirrored evidence subset of the grid crate's `ConstrainProposal`): the
/// proposal is EVIDENCE, not an applied mutation (D-P4.07-2), and the
/// constrained candidate MUST re-enter the feasibility gate from step 1.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstrainEvidence {
    /// The violated constraint this proposal addresses (the frozen
    /// constraint name from the limit report).
    pub violated_constraint: String,
    /// The named rule that produced the adjustment (the recorded
    /// deterministic proposal rule).
    pub rule: String,
    /// The uniform scale factor `limit / measured` (< 1 for a violation).
    pub scale_factor: f64,
    /// The recorded proposal statement (evidence-not-applied contract).
    pub statement: String,
}

/// One violated constraint rendered as a re-optimization INPUT
/// (§12.5 step 8: "RETURN for re-optimization … with the violated
/// constraints as new inputs"). The annotation names the constraint and
/// states the problem-input mapping — it does NOT restate physics and
/// does NOT claim the re-solve will succeed: the re-optimized schedule is
/// a CANDIDATE that re-enters the full gate chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintAnnotation {
    /// The constraint category (the frozen code's mirror).
    pub category: ConstraintCategory,
    /// The checked element (bus / branch / transformer id), verbatim from
    /// the frozen constraint name.
    pub element_id: String,
    /// The frozen constraint name `<category_code>:<element_id>`,
    /// verbatim — the name every evidence consumer cites.
    pub constraint_name: String,
    /// The problem-input mapping statement: how the constraint enters the
    /// re-solve as a named input (per category, decided once below).
    pub problem_input: String,
}

/// The per-category problem-input mapping (decided once; the wording
/// follows the D-P4.07-1 routing rationale — a voltage fix or a
/// multi-constraint adjustment is a re-dispatch decision that belongs to
/// the scheduler, and the S_max capability model is the scheduler's
/// input). Each statement records what the constraint IS as an input;
/// none claims an outcome.
fn problem_input_statement(category: ConstraintCategory, element_id: &str) -> String {
    match category {
        ConstraintCategory::BusVoltageLower => format!(
            "the re-solve receives the lower voltage-band violation at bus \
             '{element_id}' as a named input: the injection pattern must be \
             re-dispatched so the DECLARED model's band is respected; the \
             gate re-checks the new candidate with an AC power flow (§12.5) \
             and the re-dispatch decision belongs to the scheduler, not the \
             gate"
        ),
        ConstraintCategory::BusVoltageUpper => format!(
            "the re-solve receives the upper voltage-band violation at bus \
             '{element_id}' as a named input: the injection pattern must be \
             re-dispatched so the DECLARED model's band is respected; the \
             gate re-checks the new candidate with an AC power flow (§12.5) \
             and the re-dispatch decision belongs to the scheduler, not the \
             gate"
        ),
        ConstraintCategory::BranchThermal => format!(
            "the re-solve receives the branch thermal violation at \
             '{element_id}' as a named input: the injections feeding that \
             branch are bounded inputs of the re-dispatch; the gate \
             re-checks the new candidate with an AC power flow (§12.5)"
        ),
        ConstraintCategory::TransformerLoading => format!(
            "the re-solve receives the transformer-loading violation at \
             '{element_id}' as a named input: the injections feeding that \
             transformer are bounded inputs of the re-dispatch; the gate \
             re-checks the new candidate with an AC power flow (§12.5)"
        ),
        ConstraintCategory::GeneratorApparentPower => format!(
            "the re-solve receives the apparent-power (S_max) violation at \
             bus '{element_id}' as a named input: the declared capability \
             model (§11) is the scheduler's input, and the re-dispatch must \
             stay within it; the gate re-checks the new candidate with an \
             AC power flow (§12.5)"
        ),
    }
}

/// The re-optimization input the scheduling side consumes (the P4.37
/// contract type; ledger P4.37 step 1). Produced for CONSTRAIN (the
/// constrained candidate's re-run inputs) and RETURN (the re-enqueued
/// candidate's inputs). The candidate it names is a CANDIDATE again —
/// never feasible, approved, or dispatchable without the full §14.1 gate
/// chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReOptimizationInput {
    /// The candidate identifier being returned / re-run.
    pub candidate_id: String,
    /// The schedule chain id.
    pub schedule_id: String,
    /// The schedule revision the candidate derives from (>= 1).
    pub revision: u64,
    /// The envelope version the schedule was computed against (P3.12/13).
    pub envelope_version: String,
    /// The §12.6 result-record snapshot reference the candidate derives
    /// from (P3.21).
    pub result_record_snapshot_ref: String,
    /// EVERY violated constraint, as named inputs, in gate order.
    pub violated_constraints: Vec<ConstraintAnnotation>,
    /// The model identifier the check evaluated against (§12.6
    /// traceability).
    pub model_id: String,
    /// The model version.
    pub model_version: String,
    /// THE MODEL CONTENT HASH the evidence binds to (D-P4.03-2: a model
    /// change invalidates the evidence).
    pub model_content_hash: String,
    /// The feasibility-check evidence reference (the
    /// `FeasibilityCheckCompleted.check_id` — the attempt id or the
    /// pipeline-level `gate:<candidate>` id).
    pub check_id: String,
    /// The outcome that produced this input (`constrained` or
    /// `returned_for_re_optimization` — the frozen code).
    pub outcome_code: &'static str,
    /// The recorded roll-trigger note (the open item: the frozen P3.24
    /// trigger set has no feasibility-return trigger —
    /// [`OPEN_ITEM_STATEMENT`]).
    pub roll_trigger_note: String,
}

/// The consumption action for one gate verdict (the contract, D-P4.37-1).
#[derive(Debug, Clone, PartialEq)]
pub enum ConsumptionAction {
    /// **CONSTRAIN** — the violated constraint and the proposal are
    /// attached as problem inputs of the constrained candidate
    /// ([`ReOptimizationInput`]), with the proposal evidence carried
    /// ([`ConstrainEvidence`]). The constrained candidate MUST re-enter
    /// the feasibility gate from step 1 (D-P4.07-2) — a re-run of the
    /// proposal, not a scheduler re-optimization.
    ConstrainRerun {
        /// The re-run inputs (exactly the one violated constraint,
        /// annotated).
        rerun: ReOptimizationInput,
        /// The constraining proposal (evidence, not an applied mutation).
        proposal: ConstrainEvidence,
    },
    /// **RETURN** — the candidate is re-enqueued into rolling-horizon
    /// re-optimization (P3.24) with EVERY violated constraint as a named
    /// input. The gate never optimizes.
    ReOptimize(ReOptimizationInput),
    /// **No consumption** — the outcome routes per the frozen rule
    /// D-P4.07-1 and has NO scheduling-side action. The route records WHY.
    NoConsumption {
        /// The outcome.
        outcome: FeasibilityOutcome,
        /// The frozen-rule route.
        route: NoConsumptionRoute,
    },
    /// **Unconsumable** (D-P4.37-2) — the returned candidate cannot be
    /// consumed (an unparseable constraint name, or a constraint category
    /// with no problem-input mapping): the candidate is REJECTED with the
    /// reason RECORDED — never silently dropped, never passed.
    RejectUnconsumable {
        /// The recorded reason (names the offending constraint; audit
        /// text).
        reason: String,
    },
}

/// The no-consumption routes (why an outcome has no scheduling-side
/// action), one per non-consuming outcome — the D-P4.07-1 routing restated
/// from the consumer side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoConsumptionRoute {
    /// WITHIN-LIMITS: the candidate proceeds through the command
    /// lifecycle (validation, feasibility, authorization are distinct
    /// gates, §14.1; P4.11–P4.13).
    CommandLifecycle,
    /// REJECTED: the routing IS a rejection; the command lifecycle owns
    /// it (P4.12: VALIDATED → REJECTED with the named constraint).
    Rejected,
    /// HELD: parked pending operator release — operator only, with a
    /// recorded reason (the P4.08 release policy).
    OperatorRelease,
    /// INCONCLUSIVE: retry policy NONE (D-P4.05-2); no re-optimization is
    /// implied by an inconclusive check.
    RetryNone,
}

/// The returned candidate, as the composition root converts it from the
/// grid gate's verdict (`flexcore_grid_api::verdict::GateVerdict` — see
/// [`MIRROR_STATEMENT`]). Every field is the frozen evidence the
/// consumption contract consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnedCandidate {
    /// The candidate identifier.
    pub candidate_id: String,
    /// The schedule chain id.
    pub schedule_id: String,
    /// The schedule revision (>= 1).
    pub revision: u64,
    /// The envelope version.
    pub envelope_version: String,
    /// The §12.6 result-record snapshot reference.
    pub result_record_snapshot_ref: String,
    /// The gate outcome (parsed from the frozen code — the composition
    /// root builds this via [`FeasibilityOutcome::from_outcome_code`]; an
    /// unknown code has no mirror and cannot be converted).
    pub outcome: FeasibilityOutcome,
    /// The recorded verdict reason (non-empty; audit text).
    pub outcome_reason: String,
    /// The feasibility-check evidence reference (the
    /// `FeasibilityCheckCompleted.check_id`).
    pub check_id: String,
    /// The named violated constraints (frozen names, gate order; may be
    /// empty for non-consuming outcomes).
    pub violated_constraints: Vec<String>,
    /// The constraining proposal — present exactly when the outcome is
    /// [`FeasibilityOutcome::Constrained`] (mirrored evidence subset).
    pub constrain_proposal: Option<ConstrainEvidence>,
    /// The model identifier the check evaluated against.
    pub model_id: String,
    /// The model version.
    pub model_version: String,
    /// THE MODEL CONTENT HASH the evidence binds to (D-P4.03-2).
    pub model_content_hash: String,
}

// ---------------------------------------------------------------------------
// The contract functions (pure, decided once)
// ---------------------------------------------------------------------------

/// Validates the structural completeness of a returned candidate. A
/// malformed record is a CALLER BUG (the composition root mis-converted
/// the verdict) and fails loudly — this is distinct from the D-P4.37-2
/// semantic failure mode, which routes
/// [`ConsumptionAction::RejectUnconsumable`].
///
/// Rules: every identification/reference field non-empty; revision >= 1;
/// the outcome reason and the check id non-empty; the model content hash
/// non-empty (evidence binds to THE HASH); a `Constrained` outcome MUST
/// carry the proposal evidence; a consuming outcome (`Constrained`,
/// `ReturnedForReOptimization`) MUST name at least one violated
/// constraint.
pub fn validate_returned(candidate: &ReturnedCandidate) -> Result<(), FeasibilityReturnError> {
    for (field, value) in [
        ("candidate_id", candidate.candidate_id.as_str()),
        ("schedule_id", candidate.schedule_id.as_str()),
        ("envelope_version", candidate.envelope_version.as_str()),
        (
            "result_record_snapshot_ref",
            candidate.result_record_snapshot_ref.as_str(),
        ),
        ("outcome_reason", candidate.outcome_reason.as_str()),
        ("check_id", candidate.check_id.as_str()),
        ("model_id", candidate.model_id.as_str()),
        ("model_version", candidate.model_version.as_str()),
        ("model_content_hash", candidate.model_content_hash.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(FeasibilityReturnError::EmptyField { field });
        }
    }
    if candidate.revision == 0 {
        return Err(FeasibilityReturnError::RevisionZero);
    }
    let consuming = matches!(
        candidate.outcome,
        FeasibilityOutcome::Constrained | FeasibilityOutcome::ReturnedForReOptimization
    );
    if candidate.outcome == FeasibilityOutcome::Constrained
        && candidate.constrain_proposal.is_none()
    {
        return Err(FeasibilityReturnError::IncompleteEvidence {
            detail: "a CONSTRAIN outcome must carry the constraining-proposal \
                     evidence (D-P4.07-2)"
                .to_string(),
        });
    }
    if consuming && candidate.violated_constraints.is_empty() {
        return Err(FeasibilityReturnError::IncompleteEvidence {
            detail: "a consuming outcome (constrained / \
                     returned_for_re_optimization) must name at least one \
                     violated constraint"
                .to_string(),
        });
    }
    Ok(())
}

/// Parses ONE frozen constraint name (`<category_code>:<element_id>`)
/// into its annotation. `Err` names the offense — the caller decides
/// whether a parse failure is a record bug or the D-P4.37-2 failure mode
/// ([`plan_consumption`] maps it to [`ConsumptionAction::RejectUnconsumable`]).
pub fn parse_constraint_name(name: &str) -> Result<ConstraintAnnotation, String> {
    let (category_code, element_id) = match name.split_once(':') {
        Some(parts) => parts,
        None => {
            return Err(format!(
                "constraint name '{name}' violates the frozen grammar \
                 '<category_code>:<element_id>'"
            ))
        }
    };
    let category = ConstraintCategory::from_code(category_code).ok_or_else(|| {
        format!(
            "constraint name '{name}' names unknown category '{category_code}' — \
             no problem-input mapping exists (the frozen category set is: {})",
            CONSTRAINT_CATEGORY_CODES.join(", ")
        )
    })?;
    if element_id.trim().is_empty() {
        return Err(format!(
            "constraint name '{name}' has an empty element id"
        ));
    }
    Ok(ConstraintAnnotation {
        category,
        element_id: element_id.to_string(),
        constraint_name: name.to_string(),
        problem_input: problem_input_statement(category, element_id),
    })
}

/// Plans the consumption of one returned candidate (§12.5 step 8, the
/// consumer side; the pure function implementing D-P4.37-1 + D-P4.37-2).
/// Deterministic: the same candidate yields the same action.
///
/// Order: structural validation ([`validate_returned`], a caller bug is
/// an `Err`), then the outcome mapping. A semantically unconsumable
/// constraint (unparseable name, unknown category) routes
/// [`ConsumptionAction::RejectUnconsumable`] with the recorded reason —
/// never an `Err`, never a pass, never a silent drop (D-P4.37-2).
pub fn plan_consumption(
    candidate: &ReturnedCandidate,
) -> Result<ConsumptionAction, FeasibilityReturnError> {
    validate_returned(candidate)?;

    match candidate.outcome {
        FeasibilityOutcome::WithinLimits => Ok(ConsumptionAction::NoConsumption {
            outcome: candidate.outcome,
            route: NoConsumptionRoute::CommandLifecycle,
        }),
        FeasibilityOutcome::Rejected => Ok(ConsumptionAction::NoConsumption {
            outcome: candidate.outcome,
            route: NoConsumptionRoute::Rejected,
        }),
        FeasibilityOutcome::Held => Ok(ConsumptionAction::NoConsumption {
            outcome: candidate.outcome,
            route: NoConsumptionRoute::OperatorRelease,
        }),
        FeasibilityOutcome::Inconclusive => Ok(ConsumptionAction::NoConsumption {
            outcome: candidate.outcome,
            route: NoConsumptionRoute::RetryNone,
        }),
        FeasibilityOutcome::Constrained | FeasibilityOutcome::ReturnedForReOptimization => {
            // Annotate EVERY violated constraint; any unconsumable one
            // rejects the WHOLE candidate with the recorded reason (a
            // partially-consumed candidate is a silent partial drop —
            // forbidden by D-P4.37-2).
            let mut annotations = Vec::with_capacity(candidate.violated_constraints.len());
            for name in &candidate.violated_constraints {
                match parse_constraint_name(name) {
                    Ok(annotation) => annotations.push(annotation),
                    Err(reason) => {
                        return Ok(ConsumptionAction::RejectUnconsumable {
                            reason: format!(
                                "the returned candidate cannot be consumed: {reason}. \
                                 {FAILURE_MODE_STATEMENT}"
                            ),
                        });
                    }
                }
            }
            let input = ReOptimizationInput {
                candidate_id: candidate.candidate_id.clone(),
                schedule_id: candidate.schedule_id.clone(),
                revision: candidate.revision,
                envelope_version: candidate.envelope_version.clone(),
                result_record_snapshot_ref: candidate.result_record_snapshot_ref.clone(),
                violated_constraints: annotations,
                model_id: candidate.model_id.clone(),
                model_version: candidate.model_version.clone(),
                model_content_hash: candidate.model_content_hash.clone(),
                check_id: candidate.check_id.clone(),
                outcome_code: candidate.outcome.outcome_code(),
                roll_trigger_note: OPEN_ITEM_STATEMENT.to_string(),
            };
            if candidate.outcome == FeasibilityOutcome::Constrained {
                let proposal = candidate
                    .constrain_proposal
                    .clone()
                    .expect("validated: a CONSTRAIN outcome carries the proposal");
                Ok(ConsumptionAction::ConstrainRerun { rerun: input, proposal })
            } else {
                Ok(ConsumptionAction::ReOptimize(input))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// In-crate unit tests (the full suite lives in tests/feasibility_return.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mirrored_outcome_codes_are_the_frozen_vocabulary() {
        // Every mirror code must parse to its variant and back, and the
        // const array must list exactly the six frozen codes in order.
        let all = [
            (FeasibilityOutcome::WithinLimits, "within_limits"),
            (FeasibilityOutcome::Rejected, "rejected"),
            (FeasibilityOutcome::Constrained, "constrained"),
            (
                FeasibilityOutcome::ReturnedForReOptimization,
                "returned_for_re_optimization",
            ),
            (FeasibilityOutcome::Inconclusive, "inconclusive"),
            (FeasibilityOutcome::Held, "held"),
        ];
        for (outcome, code) in all {
            assert_eq!(outcome.outcome_code(), code);
            assert_eq!(FeasibilityOutcome::from_outcome_code(code), Some(outcome));
        }
        for (index, code) in GATE_OUTCOME_CODES.iter().enumerate() {
            assert_eq!(*code, all[index].1);
        }
        assert_eq!(GATE_OUTCOME_CODES.len(), all.len());
        // Unknown codes never map to a default.
        assert_eq!(FeasibilityOutcome::from_outcome_code("passed"), None);
        assert_eq!(FeasibilityOutcome::from_outcome_code(""), None);
    }

    #[test]
    fn the_mirrored_category_codes_match_the_frozen_grammar() {
        let all = [
            (ConstraintCategory::BusVoltageLower, "bus_voltage_lower"),
            (ConstraintCategory::BusVoltageUpper, "bus_voltage_upper"),
            (ConstraintCategory::BranchThermal, "branch_thermal"),
            (ConstraintCategory::TransformerLoading, "transformer_loading"),
            (
                ConstraintCategory::GeneratorApparentPower,
                "generator_apparent_power",
            ),
        ];
        for (category, code) in all {
            assert_eq!(category.code(), code);
            assert_eq!(ConstraintCategory::from_code(code), Some(category));
        }
        for (index, code) in CONSTRAINT_CATEGORY_CODES.iter().enumerate() {
            assert_eq!(*code, all[index].1);
        }
        assert_eq!(CONSTRAINT_CATEGORY_CODES.len(), all.len());
        assert_eq!(ConstraintCategory::from_code("socp_slack"), None);
    }

    #[test]
    fn every_category_has_a_problem_input_statement_naming_its_element() {
        for code in CONSTRAINT_CATEGORY_CODES {
            let category = ConstraintCategory::from_code(code).expect("listed code parses");
            let statement = problem_input_statement(category, "element-7");
            assert!(statement.contains("element-7"));
            // Every mapping re-checks through the gate; none claims an
            // outcome.
            assert!(statement.contains("re-checks"), "{statement}");
        }
    }
}
