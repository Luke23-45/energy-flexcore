// P8.07 — The ADMM/DMPC provider: formulation with explicit convexity
//         assumptions. Ledger `ledger/phase-08-advanced-coordination/
//         ledger_phase_8.md` §P8.07; report trace §13.2, §12.3, §7.4;
//         decision record:
//         `ledger/phase-08-advanced-coordination/07_admm_dmpc.md`.
//
// WHAT THIS IS: the coupling formulation of the ADMM/DMPC provider AS CODE
// (ledger P8.07 step 1) plus the explicit convexity assumptions the
// convergence claim depends on (step 2), the closed problem-class
// vocabulary (supported and — equally important — NOT supported), and the
// provider skeleton behind the mirrored `Optimizer` contract (step 3).
//
// THE FORMULATION (report §13.2 form, verbatim shape): minimize
// Σ_k f_k(x_k) subject to Σ_k a_k x_k ≤ b and x_k ∈ X_k, where
//
// - x_k is participant k's dispatched power for ONE coordination interval
//   (MW; `P > 0` = export at the point of connection, §10.2);
// - f_k(x) = 0.5 · q_k · (x − c_k)² is participant k's local convex cost
//   (q_k ≥ 0 — the curvature gate is the convexity assumption enforced at
//   construction; q_k = 0 is a flat cost);
// - X_k = [p_min_k, p_max_k] is the participant's capability box (MW);
// - Σ_k a_k x_k ≤ b is the ONE shared linear coupling (e.g. the multi-feeder
//   substation constraint of §19.3: a_k = +1 export cap; or a flexibility
//   request the fleet must jointly meet: a_k = −1, b = −d).
//
// THE ALGORITHM (exact two-block ADMM, Boyd et al. 2011 §3.2, on the
// consensus-on-copies reformulation): introduce per-participant copies z_k
// of the coupling contribution with z_k = a_k x_k and g(z) = I(Σ z_k ≤ b).
// With scaled duals u, the two blocks update in closed form:
//
// - x_k-update (EXACT, separable per participant — parallelizable, but this
//   crate evaluates it in fixed sorted-id order for determinism):
//       x_k ← clamp( (q_k c_k + ρ a_k (z_k + u_k)) / (q_k + ρ a_k²) ,
//                    p_min_k, p_max_k )
// - z-update (EXACT block minimizer of (ρ/2)Σ(z_k − v_k)² s.t. Σ z ≤ b,
//   v_k = a_k x_k − u_k): if Σ v ≤ b then z = v, else z_k = v_k − ν with
//   ν = (Σ v − b)/n > 0 (the KKT solution: identical quadratic weight per
//   copy ⇒ one uniform shift; μ = ρν > 0 satisfies complementarity);
// - u-update: u ← u + z − a∘x (the scaled dual of z − a∘x = 0).
//
// Convergence of the primal residual r = ‖a∘x − z‖ and the dual residual
// s = ρ‖a∘(z_new − z_old)‖ to configured tolerances is guaranteed by the
// two-block ADMM theorem UNDER THE STATED CONVEXITY ASSUMPTIONS ONLY
// ([`ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS`]). Residual COMPUTATION and
// the operational contract (termination, infeasibility, staleness,
// fallback) live in `dmpc.rs` (P8.08); this module owns the formulation.
//
// SOLVER-CLASS DISCIPLINE (AGENTS.md rule 4; §7.4, VERBATIM): ADMM is
// NEITHER LP/QP/MIP (the HiGHS class) NOR convex conic (the Clarabel class).
// Its convergence claim rests on the stated convexity assumptions, not on a
// solver's certificate. Nonconvex AC-OPF and mixed-integer local problems
// are NOT supported (the closed vocabulary below); AC power flow evaluates
// a state and is not AC-OPF (§12.5); ADMM on mixed-integer local problems
// is a heuristic (§13.2) and would be reported as such, never as convergent.
//
// §13.2 RISK, ENFORCED IN THE FORMULATION: the copies z_k are per-
// participant CONTRIBUTION copies of the shared constraint — never a
// consensus setpoint. No step of the algorithm forces unrelated sites to
// identical power values; only the dual price is shared
// ([`ASSUMPTION_NO_IDENTICAL_SETPOINTS`], pinned by tests).
//
// DEPENDENCY DECISION (recorded, `07_admm_dmpc.md` §4): NO solver crate is
// added. Every supported-class update is a closed-form expression (the
// local QP has one variable; the copy block has the exact uniform-shift
// solution), so the formulation needs no external solver; the CENTRALIZED
// comparison/fallback path is the frozen P8.06 baseline registry
// (`crate::baselines`) consumed through ports, keeping this crate at
// serde/serde_json only (see `Cargo.toml`).
//
// COMPOSITION SEAMS (recorded, not silently drifted):
// - the `Optimizer` port of `crates/flexcore-optimization-api/src/port.rs`
//   is mirrored by citation (the P7.10/11 mirror discipline) — the
//   composition root implements the real provider wiring;
// - the Phase 7 Class C gRPC worker shell (PLAN.md §3 places the provider
//   in `python/workers/` behind the `Optimizer` contract) is NOT written in
//   this loop (loop bans + wave file ownership); the algorithm core below
//   is what that worker executes. Recorded as a follow-up seam, never
//   silently assumed.
//
// Control status: research proposals are DATA (ADR-010). Nothing here
// authorizes, issues, approves, or dispatches anything. No field control,
// no safety claim, no certification claim. Determinism: participants are
// held in a `BTreeMap` (sorted-id iteration everywhere), fixed update
// order, no wall clock, no randomness — `SystemTime` appears nowhere in
// this module. Canonical units: MW / MWh; SOC a fraction in [0, 1] (§10.2).
//
// Honest status: WRITTEN-UNCOMPILED. No cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); every type reference
// was written against source that was read, and every test expectation is
// hand-computed arithmetic stated in the test comments. The human
// build/test gate (AGENTS.md rule 7) owns the compile/test evidence.

use serde::Serialize;

/// The formulation version (a formulation change is a NEW version — the
/// P8.06 frozen-version discipline applied to the provider).
pub const ADMM_FORMULATION_VERSION: &str = "flexcore-research/admm-formulation.v1";

/// The recorded formulation statement (the repo's statement pattern —
/// pinned by tests so what IS and IS NOT claimed stays reviewable).
pub const ADMM_FORMULATION_STATEMENT: &str = "the ADMM/DMPC formulation minimizes \
     convex quadratic local costs over box capability sets subject to ONE linear coupling \
     constraint (report §13.2 form), solved by exact two-block ADMM on per-participant \
     contribution copies. ADMM is NEITHER LP/QP/MIP (HiGHS class) NOR convex conic \
     (Clarabel class): its convergence claim is the two-block ADMM theorem UNDER THE \
     STATED CONVEXITY ASSUMPTIONS ONLY — residual convergence to the configured \
     tolerances, no iteration-count guarantee, no uniqueness claim without strong \
     convexity, and NO global-optimality claim outside the supported class. Nonconvex \
     AC-OPF and mixed-integer local problems are not supported; on such problems ADMM \
     is a heuristic and is reported as one. The copies are per-participant contribution \
     copies, never a consensus setpoint: unrelated sites are not forced to identical \
     power values. Proposals only (ADR-010); no dispatch authority; no safety claim.";

// --- The closed problem-class vocabulary (ledger P8.07 step 2) --------------

/// The SUPPORTED problem class (the only class the provider accepts; the
/// class every convergence statement in this crate is scoped to).
pub const SUPPORTED_PROBLEM_CLASS: &str =
    "convex quadratic local costs (f_k = 0.5*q_k*(x_k - c_k)^2, q_k >= 0) over closed \
     box local sets X_k = [p_min_k, p_max_k], coupled by exactly ONE linear constraint \
     sum_k a_k x_k <= b with finite non-zero a_k — solved by exact two-block ADMM \
     (Boyd et al. 2011 §3.2) with closed-form local and copy updates";

/// The classes the provider does NOT support (each names the discipline that
/// forbids it — pinned by tests; the list is part of the frozen formulation).
pub const UNSUPPORTED_PROBLEM_CLASSES: [&str; 4] = [
    "nonconvex AC-OPF — not solved here; AC power flow evaluates a state and is not \
     AC-OPF (§12.5; AGENTS.md rule 4); no optimality claim is possible without a \
     specific theorem AND experiment (§13.2)",
    "mixed-integer local problems — ADMM on them is a heuristic (§13.2); such a run \
     would be reported as heuristic behaviour with residuals and fallback outcomes, \
     never as convergent or optimally solved",
    "multiple or nonlinear coupling constraints — the supported class has exactly one \
     scalar linear coupling; extending the class is a formulation change and a NEW \
     formulation version",
    "sub-cycle or protection-grade control — the provider is a supervisory \
     coordination layer (AGENTS.md rule 3); it does not perform inverter inner-loop, \
     synthetic inertia, or relay-protection functions",
];

// --- The convexity assumptions (ledger P8.07 step 2 — each a named,
// --- test-pinned constant; the assumptions ARE part of the deliverable) ----

/// Assumption 1 — local costs convex (the construction-time curvature gate).
pub const ASSUMPTION_LOCAL_COSTS_CONVEX: &str = "each local cost \
     f_k(x_k) = 0.5 * q_k * (x_k - c_k)^2 is convex: q_k >= 0 is enforced at problem \
     construction and a negative curvature is a NAMED validation error (the convexity \
     gate — the convergence claim does not apply without it)";
/// Assumption 2 — local sets convex (closed boxes; clip = projection).
pub const ASSUMPTION_LOCAL_SETS_CONVEX: &str = "each local feasible set \
     X_k = [p_min_k, p_max_k] is a closed convex box; the closed-form clip in the \
     local update is exactly the Euclidean projection onto X_k";
/// Assumption 3 — exactly one scalar linear coupling (the §13.2 form).
pub const ASSUMPTION_COUPLING_LINEAR_SCALAR: &str = "exactly ONE linear coupling \
     constraint sum_k a_k x_k <= b (report §13.2 form) couples the participants; \
     a_k is finite and non-zero per participant; no second coupling and no nonlinear \
     coupling exists in the supported class";
/// Assumption 4 — the ONLY convergence claim made (scoped to the theorem).
pub const ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS: &str = "the convergence claim is \
     EXACTLY the two-block ADMM theorem (Boyd, Parikh, Chu, Peleato, Eckstein 2011, \
     'Distributed Optimization and Statistical Learning via the Alternating Direction \
     Method of Multipliers', §3.2): for closed, proper, convex block objectives both \
     blocks here are solved EXACTLY (closed-form local update; exact copy-block \
     minimizer), so the primal and dual residuals converge to the configured \
     tolerances. NO iteration-count guarantee is claimed (X-8: no universal \
     'converges in K iterations' target), no uniqueness claim is made without strong \
     convexity, and NO global-optimality claim is made outside the supported class";
/// Assumption 5 — the §13.2 anti-risk: copies are NOT consensus setpoints.
pub const ASSUMPTION_NO_IDENTICAL_SETPOINTS: &str = "the coordination copies z_k are \
     per-participant CONTRIBUTION copies of the shared constraint (z_k = a_k x_k), \
     never a consensus setpoint: no step of the algorithm forces unrelated sites to \
     identical power values (§13.2 risk: 'consensus variables that force unrelated \
     sites to identical setpoints'); only the dual price is shared across participants";

/// The composition seam to the platform `Optimizer` port (mirrored by
/// citation — the same discipline as `baselines.rs::OPTIMIZER_PORT_ALIGNMENT_SEAM`).
pub const OPTIMIZER_PORT_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-optimization-api/src/port.rs::Optimizer \
     (capabilities() -> SolverCapabilities; solve(problem, config) -> OptimizationResult, \
     closed six-status §12.6 vocabulary); the ADMM provider declares its problem class \
     per §7.4 through this seam and the composition root implements the real wiring";

/// The recorded seam for the Phase 7 Class C gRPC worker shell (PLAN.md §3
/// places the provider in `python/workers/` behind the `Optimizer`
/// contract). NOT written in this loop — recorded honestly, never assumed.
pub const P7_GRPC_WORKER_SEAM: &str =
    "the Phase 7 Class C gRPC worker shell (python/workers/, reusing the Phase 7 \
     worker contracts per ENTRY_NOTE boundary 3) is NOT written in this loop (loop \
     bans + wave file ownership); the formulation and operational contract in \
     flexcore-research are the algorithm core that worker executes; the composition \
     wiring is a recorded follow-up in 07_admm_dmpc.md";

// ---------------------------------------------------------------------------
// Participants and the coupled problem
// ---------------------------------------------------------------------------

/// One participant of the coupled coordination problem (one DER site / asset
/// group for ONE coordination interval). Units: MW / MWh; `P > 0` = export
/// at the point of connection (§10.2); SOC a fraction in [0, 1].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Participant {
    /// The participant id (unique within the problem; iteration order is the
    /// sorted-id order — determinism).
    pub participant_id: String,
    /// The local cost curvature q_k ≥ 0 (f_k = 0.5·q_k·(x − c_k)²).
    /// q_k = 0 is a flat cost. A NEGATIVE value is refused (the convexity
    /// gate — [`ASSUMPTION_LOCAL_COSTS_CONVEX`]).
    pub cost_curvature: f64,
    /// The preferred output c_k, MW (the cost minimum; may lie outside the
    /// box — the update clips).
    pub preferred_output_mw: f64,
    /// Minimum output when committed, MW (≥ 0; export convention).
    pub p_min_mw: f64,
    /// Maximum output, MW (≥ p_min).
    pub p_max_mw: f64,
    /// The storable energy capacity, MWh, when energy-limited
    /// (`None` = not energy-limited; the DMPC driver turns this into a
    /// per-interval box cap — see `dmpc.rs`).
    pub energy_capacity_mwh: Option<f64>,
    /// The state of charge as a fraction in [0, 1] (§10.2), when
    /// energy-limited. `None` with a capacity means fully charged (1.0).
    pub soc_fraction: Option<f64>,
    /// The coupling coefficient a_k: finite and NON-zero
    /// ([`ASSUMPTION_COUPLING_LINEAR_SCALAR`]). +1 = the participant's
    /// export counts toward an aggregate cap; −1 = the participant's export
    /// counts toward meeting an aggregate floor (b = −d).
    pub coupling_coefficient: f64,
}

/// The named validation errors of the coupled problem (fail-closed; every
/// rejection names its field — the §14.2 rejection discipline, the same
/// pattern as `baselines.rs::DispatchInputError`). PartialEq only: the
/// errors carry f64 values, and `Eq` is not derivable over floats.
#[derive(Debug, Clone, PartialEq)]
pub enum ProblemValidationError {
    /// The problem id is empty.
    EmptyProblemId,
    /// A coordination problem with no participants is a formulation error.
    EmptyParticipantSet,
    /// A participant id is empty.
    EmptyParticipantId { index: usize },
    /// A numeric field is not finite (`field` names which).
    NonFiniteField { participant_id: String, field: &'static str },
    /// A power field is negative (export convention: dispatch is ≥ 0 here;
    /// one-directional dispatch is a recorded limitation, see the record).
    NegativePower { participant_id: String, field: &'static str, value: f64 },
    /// p_min exceeds p_max for a participant.
    PMinAbovePMax { participant_id: String, p_min_mw: f64, p_max_mw: f64 },
    /// NEGATIVE cost curvature — the convexity gate
    /// ([`ASSUMPTION_LOCAL_COSTS_CONVEX`]): the convergence claim does not
    /// apply, so the problem is refused, never accepted "with a warning".
    NegativeCostCurvature { participant_id: String, value: f64 },
    /// The energy capacity is not finite or negative.
    InvalidEnergyCapacity { participant_id: String, value: f64 },
    /// The SOC fraction is outside [0, 1] (§10.2: out-of-range is a
    /// formulation error, never clipped).
    SocOutOfRange { participant_id: String, value: f64 },
    /// A zero coupling coefficient (a participant coupled by nothing does
    /// not belong in the coupled problem).
    ZeroCouplingCoefficient { participant_id: String },
    /// Two participants share an id (ids are the determinism keys).
    DuplicateParticipantId { participant_id: String },
    /// The coupling bound is not finite.
    NonFiniteCouplingBound,
    /// The interval duration is not a finite positive number of hours.
    NonPositiveIntervalHours { value: f64 },
}

impl ProblemValidationError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            ProblemValidationError::EmptyProblemId => "EMPTY_PROBLEM_ID",
            ProblemValidationError::EmptyParticipantSet => "EMPTY_PARTICIPANT_SET",
            ProblemValidationError::EmptyParticipantId { .. } => "EMPTY_PARTICIPANT_ID",
            ProblemValidationError::NonFiniteField { .. } => "NON_FINITE_FIELD",
            ProblemValidationError::NegativePower { .. } => "NEGATIVE_POWER",
            ProblemValidationError::PMinAbovePMax { .. } => "P_MIN_ABOVE_P_MAX",
            ProblemValidationError::NegativeCostCurvature { .. } => "NEGATIVE_COST_CURVATURE",
            ProblemValidationError::InvalidEnergyCapacity { .. } => "INVALID_ENERGY_CAPACITY",
            ProblemValidationError::SocOutOfRange { .. } => "SOC_OUT_OF_RANGE",
            ProblemValidationError::ZeroCouplingCoefficient { .. } => "ZERO_COUPLING_COEFFICIENT",
            ProblemValidationError::DuplicateParticipantId { .. } => "DUPLICATE_PARTICIPANT_ID",
            ProblemValidationError::NonFiniteCouplingBound => "NON_FINITE_COUPLING_BOUND",
            ProblemValidationError::NonPositiveIntervalHours { .. } => "NON_POSITIVE_INTERVAL_HOURS",
        }
    }
}

impl std::fmt::Display for ProblemValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProblemValidationError::EmptyProblemId => write!(f, "the problem id is empty"),
            ProblemValidationError::EmptyParticipantSet => {
                write!(f, "the coupled problem has no participants; a coordination \
                           problem with nothing to coordinate is a formulation error")
            }
            ProblemValidationError::EmptyParticipantId { index } => {
                write!(f, "participant {index}: participant_id is empty")
            }
            ProblemValidationError::NonFiniteField { participant_id, field } => {
                write!(f, "participant {participant_id}: field '{field}' is not finite")
            }
            ProblemValidationError::NegativePower { participant_id, field, value } => {
                write!(f, "participant {participant_id}: field '{field}' is {value} MW; \
                           negative power is a formulation error (P > 0 is export; \
                           one-directional dispatch is a recorded limitation)")
            }
            ProblemValidationError::PMinAbovePMax { participant_id, p_min_mw, p_max_mw } => {
                write!(f, "participant {participant_id}: p_min {p_min_mw} MW exceeds \
                           p_max {p_max_mw} MW")
            }
            ProblemValidationError::NegativeCostCurvature { participant_id, value } => {
                write!(f, "participant {participant_id}: cost curvature {value} is \
                           negative — the local cost is not convex and the convergence \
                           claim does not apply; refused, never accepted with a warning")
            }
            ProblemValidationError::InvalidEnergyCapacity { participant_id, value } => {
                write!(f, "participant {participant_id}: energy_capacity_mwh {value} is \
                           not a finite non-negative quantity")
            }
            ProblemValidationError::SocOutOfRange { participant_id, value } => {
                write!(f, "participant {participant_id}: soc_fraction {value} is outside \
                           [0,1] (§10.2); out-of-range is a formulation error, never clipped")
            }
            ProblemValidationError::ZeroCouplingCoefficient { participant_id } => {
                write!(f, "participant {participant_id}: coupling coefficient is zero; \
                           a participant coupled by nothing is a formulation error")
            }
            ProblemValidationError::DuplicateParticipantId { participant_id } => {
                write!(f, "participant id {participant_id} appears twice; ids are the \
                           determinism keys and must be unique")
            }
            ProblemValidationError::NonFiniteCouplingBound => {
                write!(f, "the coupling bound b is not finite")
            }
            ProblemValidationError::NonPositiveIntervalHours { value } => {
                write!(f, "interval_hours must be a finite positive number of hours; got {value}")
            }
        }
    }
}

impl std::error::Error for ProblemValidationError {}

/// The exact feasibility certificate of the supported class (P8.08 step 2
/// consumes it — infeasibility is a CERTIFICATE, never a timeout guess):
/// for one linear coupling over closed boxes, the constraint set is
/// non-empty if and only if the minimum achievable aggregate
/// min_{x_k ∈ X_k} Σ a_k x_k satisfies the bound.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeasibilityCertificate {
    /// Whether the coupled constraint set is non-empty (exact for this class).
    pub feasible: bool,
    /// min over the boxes of Σ_k a_k x_k (the best achievable aggregate).
    pub min_achievable_aggregate: f64,
    /// The coupling bound b the aggregate was checked against.
    pub coupling_bound: f64,
}

/// The coupled coordination problem: ONE interval, participants held in a
/// `BTreeMap` (sorted-id iteration everywhere — determinism), constructed
/// fail-closed. Private map field: the sorted order is the contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoupledProblem {
    /// The problem id (provenance; unique within the producer's stream).
    pub problem_id: String,
    /// The coupling bound b of Σ_k a_k x_k ≤ b.
    pub coupling_bound: f64,
    /// The interval duration, hours (> 0; MWh = MW × hours).
    pub interval_hours: f64,
    /// The participants, keyed by id (serialized sorted — the determinism
    /// contract).
    participants: BTreeMap<String, Participant>,
}

impl CoupledProblem {
    /// Constructs the problem, validating EVERY participant fail-closed
    /// (first failure wins, in sorted-id order for the per-participant
    /// checks; the per-participant field checks run in recorded field order).
    pub fn new(
        problem_id: &str,
        coupling_bound: f64,
        interval_hours: f64,
        participants: Vec<Participant>,
    ) -> Result<CoupledProblem, ProblemValidationError> {
        if problem_id.trim().is_empty() {
            return Err(ProblemValidationError::EmptyProblemId);
        }
        if !coupling_bound.is_finite() {
            return Err(ProblemValidationError::NonFiniteCouplingBound);
        }
        if !interval_hours.is_finite() || interval_hours <= 0.0 {
            return Err(ProblemValidationError::NonPositiveIntervalHours { value: interval_hours });
        }
        if participants.is_empty() {
            return Err(ProblemValidationError::EmptyParticipantSet);
        }
        let mut map: BTreeMap<String, Participant> = BTreeMap::new();
        for (index, participant) in participants.into_iter().enumerate() {
            validate_participant(index, &participant)?;
            if map.contains_key(&participant.participant_id) {
                return Err(ProblemValidationError::DuplicateParticipantId {
                    participant_id: participant.participant_id,
                });
            }
            map.insert(participant.participant_id.clone(), participant);
        }
        Ok(CoupledProblem {
            problem_id: problem_id.to_string(),
            coupling_bound,
            interval_hours,
            participants: map,
        })
    }

    /// The problem id.
    pub fn problem_id(&self) -> &str {
        &self.problem_id
    }

    /// The coupling bound b.
    pub fn coupling_bound(&self) -> f64 {
        self.coupling_bound
    }

    /// The interval duration, hours.
    pub fn interval_hours(&self) -> f64 {
        self.interval_hours
    }

    /// The participant ids in SORTED order (the fixed iteration order —
    /// the determinism contract).
    pub fn participant_ids(&self) -> Vec<String> {
        self.participants.keys().cloned().collect()
    }

    /// The participants in sorted-id order.
    pub fn participants(&self) -> Vec<&Participant> {
        self.participants.values().collect()
    }

    /// One participant by id.
    pub fn participant(&self, participant_id: &str) -> Option<&Participant> {
        self.participants.get(participant_id)
    }

    /// The coupling coefficients a_k in sorted-id order.
    pub fn coupling_coefficients(&self) -> Vec<f64> {
        self.participants.values().map(|p| p.coupling_coefficient).collect()
    }

    /// The exact feasibility certificate of the supported class: the
    /// constraint set is non-empty iff min over the boxes of Σ a_k x_k
    /// satisfies Σ a_k x_k ≤ b. (For each participant the per-coefficient
    /// minimum is a_k·p_min when a_k > 0 and a_k·p_max when a_k < 0.)
    pub fn feasibility_certificate(&self) -> FeasibilityCertificate {
        let mut min_aggregate = 0.0;
        for participant in self.participants.values() {
            let contribution = if participant.coupling_coefficient > 0.0 {
                participant.coupling_coefficient * participant.p_min_mw
            } else {
                participant.coupling_coefficient * participant.p_max_mw
            };
            min_aggregate += contribution;
        }
        FeasibilityCertificate {
            feasible: min_aggregate <= self.coupling_bound,
            min_achievable_aggregate: min_aggregate,
            coupling_bound: self.coupling_bound,
        }
    }
}

/// Per-participant field validation (fail-closed, named, first failure wins
/// in the recorded field order).
fn validate_participant(
    index: usize,
    p: &Participant,
) -> Result<(), ProblemValidationError> {
    if p.participant_id.trim().is_empty() {
        return Err(ProblemValidationError::EmptyParticipantId { index });
    }
    if !p.cost_curvature.is_finite() {
        return Err(ProblemValidationError::NonFiniteField {
            participant_id: p.participant_id.clone(),
            field: "cost_curvature",
        });
    }
    if p.cost_curvature < 0.0 {
        return Err(ProblemValidationError::NegativeCostCurvature {
            participant_id: p.participant_id.clone(),
            value: p.cost_curvature,
        });
    }
    if !p.preferred_output_mw.is_finite() {
        return Err(ProblemValidationError::NonFiniteField {
            participant_id: p.participant_id.clone(),
            field: "preferred_output_mw",
        });
    }
    for (field, value) in [("p_min_mw", p.p_min_mw), ("p_max_mw", p.p_max_mw)] {
        if !value.is_finite() {
            return Err(ProblemValidationError::NonFiniteField {
                participant_id: p.participant_id.clone(),
                field,
            });
        }
        if value < 0.0 {
            return Err(ProblemValidationError::NegativePower {
                participant_id: p.participant_id.clone(),
                field,
                value,
            });
        }
    }
    if p.p_min_mw > p.p_max_mw {
        return Err(ProblemValidationError::PMinAbovePMax {
            participant_id: p.participant_id.clone(),
            p_min_mw: p.p_min_mw,
            p_max_mw: p.p_max_mw,
        });
    }
    if let Some(capacity) = p.energy_capacity_mwh {
        if !capacity.is_finite() || capacity < 0.0 {
            return Err(ProblemValidationError::InvalidEnergyCapacity {
                participant_id: p.participant_id.clone(),
                value: capacity,
            });
        }
    }
    if let Some(soc) = p.soc_fraction {
        if !soc.is_finite() || !(0.0..=1.0).contains(&soc) {
            return Err(ProblemValidationError::SocOutOfRange {
                participant_id: p.participant_id.clone(),
                value: soc,
            });
        }
    }
    if !p.coupling_coefficient.is_finite() {
        return Err(ProblemValidationError::NonFiniteField {
            participant_id: p.participant_id.clone(),
            field: "coupling_coefficient",
        });
    }
    if p.coupling_coefficient == 0.0 {
        return Err(ProblemValidationError::ZeroCouplingCoefficient {
            participant_id: p.participant_id.clone(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The formulation as code — the closed-form updates (exact block solves)
// ---------------------------------------------------------------------------

/// The LOCAL x-update, closed form and EXACT (the x-block of the two-block
/// ADMM is separable per participant, so this is the exact block minimizer):
///
/// minimize over x ∈ [p_min, p_max]:
///     0.5·q·(x − c)² + (ρ/2)·(a·x − (z_k + u_k))²
///
/// Setting the derivative to zero: q(x − c) + ρ·a·(a·x − s) = 0 with
/// s = z_k + u_k gives x = (q·c + ρ·a·s)/(q + ρ·a²); the box clip is
/// exactly the Euclidean projection onto X_k
/// ([`ASSUMPTION_LOCAL_SETS_CONVEX`]).
pub fn local_update_closed_form(
    participant: &Participant,
    rho: f64,
    z_plus_u: f64,
) -> f64 {
    let q = participant.cost_curvature;
    let a = participant.coupling_coefficient;
    let unconstrained = (q * participant.preferred_output_mw + rho * a * z_plus_u)
        / (q + rho * a * a);
    unconstrained.clamp(participant.p_min_mw, participant.p_max_mw)
}

/// The COPY (z) update, the EXACT minimizer of (ρ/2)·Σ_k (z_k − v_k)²
/// subject to Σ_k z_k ≤ b, where v_k = a_k·x_k − u_k:
///
/// - if Σ v ≤ b, the unconstrained minimizer z = v is feasible ⇒ optimal;
/// - otherwise the solution lies on Σ z = b and the KKT conditions give
///   z_k = v_k − ν with ONE uniform shift ν = (Σ v − b)/n > 0 (identical
///   quadratic weight per copy ⇒ one shared multiplier μ = ρ·ν > 0, which
///   satisfies complementary slackness on the single coupling).
///
/// This is the aggregation step the COORDINATOR performs; it never equalizes
/// the participants' setpoints — only the shift (the dual price) is shared
/// ([`ASSUMPTION_NO_IDENTICAL_SETPOINTS`]).
pub fn copy_update(v: &[f64], bound: f64) -> Vec<f64> {
    let total: f64 = v.iter().sum();
    if total <= bound {
        return v.to_vec();
    }
    let n = v.len() as f64;
    let nu = (total - bound) / n;
    v.iter().map(|&v_k| v_k - nu).collect()
}

/// The DUAL (scaled u) update of the two-block ADMM for the constraint
/// z_k − a_k·x_k = 0: u ← u + z − a∘x (Boyd et al. 2011 §3.3 scaled form).
pub fn dual_update(u: &[f64], z: &[f64], a: &[f64], x: &[f64]) -> Vec<f64> {
    (0..u.len())
        .map(|k| u[k] + z[k] - a[k] * x[k])
        .collect()
}

/// The deterministic zero-init state of the local variable: the box-clamped
/// zero (documented — the initial "last accepted value" the bounded-
/// staleness policy of P8.08 reuses, and the x state every run starts from).
pub fn clamped_zero_init(participant: &Participant) -> f64 {
    0.0_f64.clamp(participant.p_min_mw, participant.p_max_mw)
}

// ---------------------------------------------------------------------------
// The provider skeleton behind the mirrored Optimizer contract (P8.07 step 3)
// ---------------------------------------------------------------------------

/// The §7.4 problem-class declaration seam (mirrors the `Optimizer`
/// capabilities discipline — see [`OPTIMIZER_PORT_ALIGNMENT_SEAM`]).
pub trait OptimizerProviderSeam {
    /// The problem class this provider declares (per §7.4: the provider
    /// DECLARES its class; consumers gate on the declaration).
    fn problem_class_declaration(&self) -> &'static str;
    /// The classes this provider does NOT support (the closed vocabulary).
    fn unsupported_problem_classes(&self) -> &'static [&'static str; 4];
}

/// The ADMM provider skeleton. The formulation IS the provider: every
/// supported-class update is closed-form (see the dependency decision in
/// the module header), and the operational run lives in `dmpc.rs` (P8.08).
pub struct AdmmProvider;

impl OptimizerProviderSeam for AdmmProvider {
    fn problem_class_declaration(&self) -> &'static str {
        SUPPORTED_PROBLEM_CLASS
    }

    fn unsupported_problem_classes(&self) -> &'static [&'static str; 4] {
        &UNSUPPORTED_PROBLEM_CLASSES
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is either
// hand-computed arithmetic stated in the comments, a literal copied from
// this module's constants, or an exact fixed point derived on paper —
// nothing is fabricated).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    // --- constants and the closed class vocabulary (known answers) ---------

    #[test]
    fn formulation_constants_are_the_recorded_strings() {
        assert_eq!(ADMM_FORMULATION_VERSION, "flexcore-research/admm-formulation.v1");
        // Solver-class discipline VERBATIM (AGENTS.md rule 4): ADMM is
        // neither the HiGHS class nor the Clarabel class — the statement
        // carries both negations so no summary can drop them.
        assert!(ADMM_FORMULATION_STATEMENT.contains("NEITHER LP/QP/MIP"));
        assert!(ADMM_FORMULATION_STATEMENT.contains("NOR convex conic"));
        assert!(ADMM_FORMULATION_STATEMENT.contains("UNDER THE STATED CONVEXITY ASSUMPTIONS"));
        assert!(ADMM_FORMULATION_STATEMENT.contains("NO global-optimality claim"));
        assert!(ADMM_FORMULATION_STATEMENT.contains("never a consensus setpoint"));
        assert!(ADMM_FORMULATION_STATEMENT.contains("Proposals only (ADR-010)"));
    }

    #[test]
    fn unsupported_problem_classes_name_the_discipline() {
        assert_eq!(UNSUPPORTED_PROBLEM_CLASSES.len(), 4);
        assert!(UNSUPPORTED_PROBLEM_CLASSES[0].contains("AC-OPF"));
        assert!(UNSUPPORTED_PROBLEM_CLASSES[0].contains("not AC-OPF"));
        assert!(UNSUPPORTED_PROBLEM_CLASSES[1].contains("mixed-integer"));
        assert!(UNSUPPORTED_PROBLEM_CLASSES[1].contains("heuristic"));
        assert!(UNSUPPORTED_PROBLEM_CLASSES[2].contains("exactly one"));
        assert!(UNSUPPORTED_PROBLEM_CLASSES[3].contains("protection-grade"));
        // The supported class names its solver discipline too.
        assert!(SUPPORTED_PROBLEM_CLASS.contains("two-block ADMM"));
        assert!(SUPPORTED_PROBLEM_CLASS.contains("ONE linear constraint"));
    }

    #[test]
    fn the_five_convexity_assumptions_are_recorded_and_scoped() {
        assert!(ASSUMPTION_LOCAL_COSTS_CONVEX.contains("q_k >= 0 is enforced"));
        assert!(ASSUMPTION_LOCAL_COSTS_CONVEX.contains("NAMED validation error"));
        assert!(ASSUMPTION_LOCAL_SETS_CONVEX.contains("closed convex box"));
        assert!(ASSUMPTION_COUPLING_LINEAR_SCALAR.contains("exactly ONE linear coupling"));
        // The convergence claim is SCOPED: the theorem is named, the
        // non-claims are explicit (X-8 discipline — no universal target).
        assert!(ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS.contains("EXACTLY the two-block ADMM theorem"));
        assert!(ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS.contains("Boyd"));
        assert!(ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS.contains("NO iteration-count guarantee"));
        assert!(ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS.contains("NO global-optimality claim"));
        // The §13.2 anti-risk assumption (no identical setpoints).
        assert!(ASSUMPTION_NO_IDENTICAL_SETPOINTS.contains("never a consensus setpoint"));
        assert!(ASSUMPTION_NO_IDENTICAL_SETPOINTS.contains("identical power values"));
    }

    #[test]
    fn provider_declares_its_class_per_the_seam() {
        let provider = AdmmProvider;
        assert_eq!(provider.problem_class_declaration(), SUPPORTED_PROBLEM_CLASS);
        assert_eq!(provider.unsupported_problem_classes().len(), 4);
        assert!(OPTIMIZER_PORT_ALIGNMENT_SEAM.contains("flexcore-optimization-api/src/port.rs"));
        assert!(OPTIMIZER_PORT_ALIGNMENT_SEAM.contains("Optimizer"));
        assert!(P7_GRPC_WORKER_SEAM.contains("NOT written in this loop"));
        assert!(P7_GRPC_WORKER_SEAM.contains("python/workers/"));
    }

    // --- problem construction: fail-closed, named errors -------------------

    #[test]
    fn problem_validation_refuses_each_named_error() {
        // Empty problem id.
        let err = CoupledProblem::new("  ", -3.0, 1.0, vec![demand_participant("a", 2.0, 0.0, 0.0, 3.0)])
            .expect_err("empty problem id refused");
        assert_eq!(err.name(), "EMPTY_PROBLEM_ID");
        // Empty participant set.
        let err = CoupledProblem::new("p", -3.0, 1.0, Vec::new())
            .expect_err("empty participant set refused");
        assert_eq!(err.name(), "EMPTY_PARTICIPANT_SET");
        // Empty participant id.
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![demand_participant("", 2.0, 0.0, 0.0, 3.0)])
            .expect_err("empty participant id refused");
        assert_eq!(err.name(), "EMPTY_PARTICIPANT_ID");
        // Duplicate id (ids are the determinism keys).
        let err = CoupledProblem::new(
            "p",
            -3.0,
            1.0,
            vec![
                demand_participant("dup", 2.0, 0.0, 0.0, 3.0),
                demand_participant("dup", 4.0, 1.0, 0.0, 3.0),
            ],
        )
        .expect_err("duplicate id refused");
        assert_eq!(err.name(), "DUPLICATE_PARTICIPANT_ID");
        // p_min above p_max.
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![demand_participant("a", 2.0, 0.0, 3.0, 2.0)])
            .expect_err("p_min above p_max refused");
        assert_eq!(err.name(), "P_MIN_ABOVE_P_MAX");
        // SOC outside [0, 1] (§10.2: never clipped).
        let mut p = demand_participant("a", 2.0, 0.0, 0.0, 3.0);
        p.energy_capacity_mwh = Some(4.0);
        p.soc_fraction = Some(1.5);
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![p])
            .expect_err("soc out of range refused");
        assert_eq!(err.name(), "SOC_OUT_OF_RANGE");
        // Zero coupling coefficient.
        let mut p = demand_participant("a", 2.0, 0.0, 0.0, 3.0);
        p.coupling_coefficient = 0.0;
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![p])
            .expect_err("zero coupling refused");
        assert_eq!(err.name(), "ZERO_COUPLING_COEFFICIENT");
        // Non-positive interval hours.
        let err = CoupledProblem::new("p", -3.0, 0.0, vec![demand_participant("a", 2.0, 0.0, 0.0, 3.0)])
            .expect_err("zero interval hours refused");
        assert_eq!(err.name(), "NON_POSITIVE_INTERVAL_HOURS");
        // Non-finite coupling bound.
        let err = CoupledProblem::new("p", f64::NAN, 1.0, vec![demand_participant("a", 2.0, 0.0, 0.0, 3.0)])
            .expect_err("non-finite bound refused");
        assert_eq!(err.name(), "NON_FINITE_COUPLING_BOUND");
    }

    #[test]
    fn the_convexity_gate_refuses_negative_curvature() {
        // THE convexity gate (ASSUMPTION_LOCAL_COSTS_CONVEX): a negative
        // curvature makes the local cost non-convex — the convergence claim
        // would not apply, so the problem is REFUSED, never accepted with a
        // warning.
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![demand_participant("a", -2.0, 0.0, 0.0, 3.0)])
            .expect_err("negative curvature refused (convexity gate)");
        assert_eq!(err.name(), "NEGATIVE_COST_CURVATURE");
        // Non-finite curvature is the named non-finite error.
        let err = CoupledProblem::new("p", -3.0, 1.0, vec![demand_participant("a", f64::INFINITY, 0.0, 0.0, 3.0)])
            .expect_err("non-finite curvature refused");
        assert_eq!(err.name(), "NON_FINITE_FIELD");
    }

    #[test]
    fn every_validation_error_renders_non_empty_prose() {
        let errors = [
            ProblemValidationError::EmptyProblemId,
            ProblemValidationError::EmptyParticipantSet,
            ProblemValidationError::EmptyParticipantId { index: 0 },
            ProblemValidationError::NonFiniteField {
                participant_id: "a".to_string(),
                field: "cost_curvature",
            },
            ProblemValidationError::NegativePower {
                participant_id: "a".to_string(),
                field: "p_min_mw",
                value: -1.0,
            },
            ProblemValidationError::PMinAbovePMax {
                participant_id: "a".to_string(),
                p_min_mw: 3.0,
                p_max_mw: 2.0,
            },
            ProblemValidationError::NegativeCostCurvature {
                participant_id: "a".to_string(),
                value: -1.0,
            },
            ProblemValidationError::InvalidEnergyCapacity {
                participant_id: "a".to_string(),
                value: -1.0,
            },
            ProblemValidationError::SocOutOfRange {
                participant_id: "a".to_string(),
                value: 1.5,
            },
            ProblemValidationError::ZeroCouplingCoefficient {
                participant_id: "a".to_string(),
            },
            ProblemValidationError::DuplicateParticipantId {
                participant_id: "a".to_string(),
            },
            ProblemValidationError::NonFiniteCouplingBound,
            ProblemValidationError::NonPositiveIntervalHours { value: 0.0 },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
            assert!(!error.name().is_empty());
        }
    }

    // --- the exact feasibility certificate (known answers) -----------------

    #[test]
    fn feasibility_certificate_known_answers() {
        // Two demand-floor participants with boxes [0, 3]: the minimum
        // achievable aggregate is (−3) + (−3) = −6.
        // b = −4: −6 ≤ −4 → FEASIBLE. b = −10: −6 > −10 → INFEASIBLE.
        let build = |b: f64| {
            CoupledProblem::new(
                "toy",
                b,
                1.0,
                vec![
                    demand_participant("a", 2.0, 0.0, 0.0, 3.0),
                    demand_participant("b", 2.0, 0.0, 0.0, 3.0),
                ],
            )
            .expect("valid problem")
        };
        let feasible = build(-4.0).feasibility_certificate();
        assert!(feasible.feasible);
        assert_eq!(feasible.min_achievable_aggregate, -6.0);
        assert_eq!(feasible.coupling_bound, -4.0);
        let infeasible = build(-10.0).feasibility_certificate();
        assert!(!infeasible.feasible);
        assert_eq!(infeasible.min_achievable_aggregate, -6.0);
        assert_eq!(infeasible.coupling_bound, -10.0);

        // MIXED signs: a = +1 box [1, 2] (its aggregate minimum is +1·1 = 1),
        // a = −1 box [0, 4] (its aggregate minimum is (−1)·4 = −4):
        // min aggregate = 1 + (−4) = −3. b = −1 → feasible; b = −4 → infeasible.
        let mut cap_participant = demand_participant("cap", 2.0, 0.0, 1.0, 2.0);
        cap_participant.coupling_coefficient = 1.0;
        let mixed = CoupledProblem::new(
            "mixed",
            -1.0,
            1.0,
            vec![cap_participant, demand_participant("dem", 2.0, 0.0, 0.0, 4.0)],
        )
        .expect("valid mixed problem");
        let cert = mixed.feasibility_certificate();
        assert!(cert.feasible);
        assert_eq!(cert.min_achievable_aggregate, -3.0);
        let mixed_tight = CoupledProblem::new(
            "mixed-tight",
            -4.0,
            1.0,
            vec![
                Participant {
                    participant_id: "cap".to_string(),
                    cost_curvature: 2.0,
                    preferred_output_mw: 0.0,
                    p_min_mw: 1.0,
                    p_max_mw: 2.0,
                    energy_capacity_mwh: None,
                    soc_fraction: None,
                    coupling_coefficient: 1.0,
                },
                demand_participant("dem", 2.0, 0.0, 0.0, 4.0),
            ],
        )
        .expect("valid mixed problem");
        let cert = mixed_tight.feasibility_certificate();
        assert!(!cert.feasible);
        assert_eq!(cert.min_achievable_aggregate, -3.0);
    }

    // --- the closed-form updates (hand-computed known answers) -------------

    #[test]
    fn local_update_closed_form_known_answers() {
        // Toy-2 participant: q = 2, c = 0, box [0, 3], a = −1. With ρ = 2 and
        // s = z_k + u_k = (−2) + (−2) = −4:
        // x = (q·c + ρ·a·s)/(q + ρ·a²) = (0 + 2·(−1)·(−4))/(2 + 2) = 8/4 = 2.0.
        let p = demand_participant("a", 2.0, 0.0, 0.0, 3.0);
        assert_eq!(local_update_closed_form(&p, 2.0, -4.0), 2.0);
        // Same participant with s = +10:
        // x = (0 + 2·(−1)·10)/4 = −5 → clipped to the box → 0.0 (the clip is
        // exactly the projection onto X_k).
        assert_eq!(local_update_closed_form(&p, 2.0, 10.0), 0.0);
        // And with s = −20: x = (0 + 2·(−1)·(−20))/4 = 10 → clipped to 3.0.
        assert_eq!(local_update_closed_form(&p, 2.0, -20.0), 3.0);
        // Flat cost (q = 0): x = s/a = (−1)/(−1) = 1.0 (then clipped).
        let flat = demand_participant("f", 0.0, 7.0, 0.0, 3.0);
        assert_eq!(local_update_closed_form(&flat, 2.0, -1.0), 1.0);
        // Preferred output inside the box with an inactive coupling term
        // (s = a·c): the update stays at c. c = 1, a = −1 → s = −1:
        // x = (2·1 + 2·(−1)·(−1))/4 = 4/4 = 1.0.
        let pref = demand_participant("c", 2.0, 1.0, 0.0, 3.0);
        assert_eq!(local_update_closed_form(&pref, 2.0, -1.0), 1.0);
        // p_min above zero clamps the zero-init: box [2, 3] → init 2.0.
        let floored = demand_participant("m", 2.0, 0.0, 2.0, 3.0);
        assert_eq!(clamped_zero_init(&floored), 2.0);
        assert_eq!(clamped_zero_init(&p), 0.0);
    }

    #[test]
    fn copy_update_exact_minimizer_known_answers() {
        // INACTIVE coupling: v = [−5, −5], b = −4: Σ v = −10 ≤ −4 → z = v.
        assert_eq!(copy_update(&[-5.0, -5.0], -4.0), vec![-5.0, -5.0]);
        // ACTIVE coupling: v = [0, 0], b = −4: Σ v = 0 > −4 →
        // ν = (0 − (−4))/2 = 2 → z = [−2, −2] (Σ z = −4 = b ✓).
        assert_eq!(copy_update(&[0.0, 0.0], -4.0), vec![-2.0, -2.0]);
        // Mixed v: v = [0, −1], b = −4: Σ v = −1 > −4 → ν = (−1 + 4)/2 = 1.5
        // → z = [−1.5, −2.5]; Σ z = −4 = b ✓ and each z_k = v_k − ν ✓ (the
        // KKT uniform shift).
        assert_eq!(copy_update(&[0.0, -1.0], -4.0), vec![-1.5, -2.5]);
        // EXACT boundary: Σ v = b → inactive branch (≤) → z = v.
        assert_eq!(copy_update(&[-2.0, -2.0], -4.0), vec![-2.0, -2.0]);
    }

    #[test]
    fn dual_update_known_answer() {
        // u ← u + z − a∘x: u = [0, 0], z = [−2, −2], a = [−1, −1], x = [0, 0]
        // → [−2 + 0, −2 + 0]... hand-computed: u_k + z_k − a_k·x_k
        // = 0 + (−2) − (−1)·0 = −2 per participant.
        assert_eq!(
            dual_update(&[0.0, 0.0], &[-2.0, -2.0], &[-1.0, -1.0], &[0.0, 0.0]),
            vec![-2.0, -2.0]
        );
        // At the toy-2 fixed point the update is the identity:
        // u = [−2, −2], z = [−2, −2], a∘x = [−2, −2] → u unchanged.
        assert_eq!(
            dual_update(&[-2.0, -2.0], &[-2.0, -2.0], &[-1.0, -1.0], &[2.0, 2.0]),
            vec![-2.0, -2.0]
        );
    }

    // --- THE FIXED POINT (hand-derived on paper, stated arithmetic) --------

    #[test]
    fn toy_two_fixed_point_is_unchanged_by_one_full_iteration() {
        // Toy 2 (demand class): two symmetric participants, q = 2, c = 0,
        // boxes [0, 3], a = −1, demand d = 4 (b = −4), ρ = 2.
        //
        // CENTRALIZED OPTIMUM (hand-derived): minimize x_a² + x_b² s.t.
        // x_a + x_b ≥ 4, boxes: the constraint is active, symmetry gives
        // x_a = x_b = 2 (marginal costs 2·x equal at λ = 4 > 0 ✓).
        //
        // THE ADMM FIXED POINT (hand-derived): x = (2, 2), z = a∘x = (−2, −2),
        // u = (−ν, −ν) with ν = 2·q/ρ = 2 (the x-update first-order condition
        // ρ·ν = 2q forces ν = 2; the z-update active-branch shift
        // ν = (Σv − b)/n with v_k = a_k x_k − u_k = −2 + 2 = 0 gives
        // ν = (0 + 4)/2 = 2 ✓ self-consistent; the u-update is the identity
        // because z = a∘x).
        //
        // One full iteration FROM this state must leave it EXACTLY unchanged
        // and both residuals are exactly 0.
        let rho = 2.0;
        let pa = demand_participant("a", 2.0, 0.0, 0.0, 3.0);
        let pb = demand_participant("b", 2.0, 0.0, 0.0, 3.0);
        let a = [pa.coupling_coefficient, pb.coupling_coefficient];
        let mut x = [2.0_f64, 2.0];
        let z_prev: Vec<f64> = vec![-2.0, -2.0];
        let mut z: Vec<f64> = z_prev.clone();
        let mut u: Vec<f64> = vec![-2.0, -2.0];

        // x-updates (each hand-checked: minimize x² + (x − 4)² → x = 2).
        x[0] = local_update_closed_form(&pa, rho, z[0] + u[0]);
        x[1] = local_update_closed_form(&pb, rho, z[1] + u[1]);
        assert_eq!(x, [2.0, 2.0], "x-update keeps the fixed point");
        // z-update: v_k = a_k x_k − u_k = −2 − (−2) = 0; Σ v = 0 > −4 →
        // active branch, ν = (0 + 4)/2 = 2 → z = [−2, −2] (unchanged).
        let v: Vec<f64> = (0..2).map(|k| a[k] * x[k] - u[k]).collect();
        z = copy_update(&v, -4.0);
        assert_eq!(z, vec![-2.0, -2.0], "z-update keeps the fixed point");
        // u-update: u + z − a∘x = −2 + (−2) − (−2) = −2 (identity).
        u = dual_update(&u, &z, &a, &x);
        assert_eq!(u, vec![-2.0, -2.0], "u-update keeps the fixed point");
        // Residuals at the fixed point: primal r_k = a_k x_k − z_k = 0;
        // dual s = ρ·‖a∘(z_new − z_old)‖ = 0 (z did not move).
        let primal: f64 = (0..2)
            .map(|k| (a[k] * x[k] - z[k]).powi(2))
            .sum::<f64>()
            .sqrt();
        let delta: f64 = (0..2)
            .map(|k| (a[k] * (z[k] - z_prev[k])).powi(2))
            .sum::<f64>()
            .sqrt();
        let dual = rho * delta;
        assert_eq!(primal, 0.0, "primal residual is exactly 0 at the fixed point");
        assert_eq!(dual, 0.0, "dual residual is exactly 0 at the fixed point");
    }

    // --- determinism and serialization -------------------------------------

    #[test]
    fn participant_iteration_order_is_sorted_by_id() {
        // Given in scrambled order, the problem holds them sorted: the
        // iteration order IS the determinism contract.
        let problem = CoupledProblem::new(
            "order",
            -4.0,
            1.0,
            vec![
                demand_participant("zeta", 2.0, 0.0, 0.0, 3.0),
                demand_participant("alpha", 2.0, 0.0, 0.0, 3.0),
                demand_participant("mid", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("valid problem");
        assert_eq!(problem.participant_ids(), vec!["alpha", "mid", "zeta"]);
        let ids: Vec<&str> = problem.participants().iter().map(|p| p.participant_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "mid", "zeta"]);
        assert_eq!(problem.coupling_coefficients(), vec![-1.0, -1.0, -1.0]);
        assert_eq!(problem.coupling_bound(), -4.0);
        assert_eq!(problem.interval_hours(), 1.0);
        assert_eq!(problem.problem_id(), "order");
        assert!(problem.participant("mid").is_some());
        assert!(problem.participant("nope").is_none());
    }

    #[test]
    fn problem_serializes_with_sorted_participant_keys() {
        // The BTreeMap serializes SORTED (serde_json's default map is
        // ordered) — the wire form is deterministic by construction.
        let problem = CoupledProblem::new(
            "ser",
            -4.0,
            0.5,
            vec![
                demand_participant("zeta", 2.0, 0.0, 0.0, 3.0),
                demand_participant("alpha", 2.0, 0.0, 0.0, 3.0),
            ],
        )
        .expect("valid problem");
        let value = serde_json::to_value(&problem).expect("serializes");
        let keys: Vec<&str> = value["participants"].as_object().expect("object").keys().collect();
        assert_eq!(keys, vec!["alpha", "zeta"]);
        assert_eq!(value["coupling_bound"], serde_json::json!(-4.0));
        assert_eq!(value["interval_hours"], serde_json::json!(0.5));
    }
}
