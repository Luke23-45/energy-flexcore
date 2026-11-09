// P3.10 — Flexible load capability model (pure rules, no I/O).
//
// Implements ledger P3.10 ("Flexible load model"), report §11.4 (with §11.1).
// Decision record:
// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.
//
// THE MODEL, EXACTLY AS SPECIFIED (report §11.4): a flexible load is more
// than a negative power injection. It carries:
//
//   baseline demand · reducible/increasable range · response time ·
//   maximum event duration · minimum recovery time · rebound energy ·
//   comfort/process constraints · availability windows
//
// - Units are canonical (§10.2): MW for power, MWh for energy. `P > 0` means
//   EXPORT at the point of connection, so the baseline demand enters the net
//   injection as a NEGATIVE term. Reduction and increase are non-negative
//   demand-side magnitudes (like the BESS P_ch/P_dis convention, §11.2).
// - **THE BASELINE RULE, DECIDED ONCE**: the baseline method of this module
//   is [`SCENARIO_DECLARED_BASELINE_METHOD`] — "scenario-declared baseline
//   profile": the per-interval baseline demand is the value the scenario
//   fixture itself declares (scenario-v2.json behaviour profiles). It is
//   recorded as SYNTHETIC ([`SYNTHETIC_BASELINE_DISCLAIMER`]) and is NOT a
//   market/DR baseline and NOT a measurement claim: region/product-specific
//   DR baseline and measurement-and-verification rules are explicitly out of
//   scope for v0.2.0 (§11.4; D1/D2 open). The constructor REJECTS any other
//   baseline-method name — a market baseline cannot enter through
//   configuration (the decision is enforced, not just documented).
// - **THE DURATION/RECOVERY POLICY, DECIDED ONCE — EXCLUDE-AND-FLAG.** An
//   event that exceeds the maximum event duration, or that starts before
//   the minimum recovery time of the previous event has completed, is (a)
//   FLAGGED with a named, structured violation (never clipped, never
//   silently repaired — the same honesty discipline as the BESS REJECT
//   policy) and (b) EXCLUDED from credited flexibility: its declared energy
//   is subtracted from the credited reduction and it owes no rebound. The
//   pure enforcement function is [`enforce_event`] — the single
//   implementation the P3.12 envelope and the P3.17 scheduler reuse rather
//   than reimplement.
// - **THE REBOUND RULE, DECIDED ONCE**: rebound energy is a REQUIRED field
//   ([`ReboundEnergy`]) — zero only if explicitly configured as zero WITH a
//   recorded reason; it is never defaulted. The payback owed by a credited
//   event is prorated linearly by event duration against the full,
//   maximum-duration event: `owed = rebound_energy_mwh ×
//   (event_duration / max_event_duration)`. The proration is a DECLARED
//   CONVENTION for the synthetic portfolio, not a measured load law. Net
//   flexibility never overstates: `net_flexibility_mwh = credited_reduction
//   − max(rebound_owed, scheduled_increase)` — every scheduled increase is
//   treated as rebound payback (a conservative recorded assumption), and any
//   unpaid rebound is subtracted even though it will occur beyond the
//   horizon. A shortfall beyond the tolerance raises a named
//   `ReboundPaybackIncomplete` violation.
// - **Response time** is required, validated configuration, recorded on
//   every evaluation. Its LAG is not enforced at interval resolution here
//   (that cross-interval formulation is a P3.17 concern); the omission is
//   recorded as an unknown dimension on every output — never silent.
// - **Comfort/process constraints** are declared descriptors (§11.4 lists
//   them as model data; no numeric comfort law exists in the scenario, and
//   none is invented). Undeclared constraints are recorded as an unknown
//   dimension.
// - **Availability windows** are required configuration; a setpoint outside
//   every window is a named violation. No load is dispatched outside its
//   availability window (ledger acceptance).
// - **Unknown provenance narrows (frozen P3.02 rule, reused, not
//   reimplemented)**: a reducible/increasable range declared with UNKNOWN
//   provenance narrows via [`crate::capability_provenance::narrow_interval`]
//   for [`AssetClass::FlexibleLoad`] — the collapse-to-zero fallback gives
//   zero flexibility credit, never "unconstrained".
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no device-write path; a violation-free evaluation makes
// a schedule a CANDIDATE with respect to asset constraints — feasibility,
// authorization, and dispatch remain Phase 4 gates (never implied here).

use flexcore_contracts::capability::Provenance;

use crate::bess_model::ProvenancedValue;
use crate::capability_provenance::{narrow_interval, AssetClass, BoundInterval};

/// Evaluation tolerance in each value's own unit (MW, MWh, seconds):
/// comparisons are boundary-inclusive within this epsilon — an event of
/// exactly the maximum duration and a recovery gap of exactly the minimum
/// are feasible. Part of the module's API contract; the P3.12 envelope and
/// the P3.21 residual check reuse this constant rather than inventing their
/// own.
pub const LOAD_EVALUATION_TOLERANCE: f64 = 1e-9;

/// The named synthetic baseline method, DECIDED ONCE (ledger P3.10 step 2):
/// the per-interval baseline demand is the value the scenario fixture itself
/// declares. The constructor rejects any other name.
pub const SCENARIO_DECLARED_BASELINE_METHOD: &str = "scenario-declared-baseline-profile";

/// The recorded disclaimer carried on every evaluation output: the baseline
/// is SYNTHETIC and is not a market/DR baseline or measurement claim.
pub const SYNTHETIC_BASELINE_DISCLAIMER: &str =
    "SYNTHETIC baseline: the value the scenario fixture itself declares \
(scenario-v2.json behaviour profiles). NOT a market/DR baseline and NOT a \
measurement claim; region/product-specific DR baseline and \
measurement-and-verification rules are out of scope for v0.2.0 (report \
§11.4; decisions D1/D2 open).";

/// The recorded duration/recovery policy statement (decided once):
/// EXCLUDE-AND-FLAG.
pub const DURATION_RECOVERY_POLICY_STATEMENT: &str =
    "EXCLUDE-AND-FLAG (decided once): an event exceeding the maximum event \
duration, or starting before the minimum recovery time completes, is flagged \
with a named violation AND excluded from credited flexibility (its energy is \
never credited and it owes no rebound); setpoints are never clipped or \
silently repaired.";

/// The recorded rebound convention statement (decided once).
pub const REBOUND_CONVENTION_STATEMENT: &str =
    "Rebound is a REQUIRED configuration: zero only if explicitly configured \
as zero with a recorded reason, never defaulted. Payback owed by a credited \
event is prorated linearly by event duration against the full \
maximum-duration event (a declared synthetic convention, not a measured load \
law). Net flexibility never overstates: credited reduction minus the greater \
of rebound owed and scheduled increase (all scheduled increase is treated as \
rebound payback — a conservative recorded assumption).";

// --- Errors -----------------------------------------------------------------

/// Errors returned by flexible-load model construction and evaluation. Each
/// variant names the offending field/value — never a bare "invalid input"
/// (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadModelError {
    /// A numeric parameter was NaN or infinite; model parameters must be
    /// finite.
    NonFiniteParameter {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The baseline profile was empty; the scenario-declared per-interval
    /// baseline must have at least one interval.
    EmptyBaselineProfile,
    /// A baseline demand value was negative or non-finite (demand is a
    /// non-negative MW magnitude).
    InvalidBaselineDemand {
        /// The offending interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
    /// A declared power range was negative.
    NegativeRange {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The maximum event duration was zero or negative (a non-positive
    /// maximum is a configuration error, not "no events allowed").
    NonPositiveMaxEventDuration {
        /// The offending value.
        value: f64,
    },
    /// The rebound energy was negative or non-finite.
    InvalidReboundEnergy {
        /// The offending value.
        value: f64,
    },
    /// Zero rebound energy was declared without a recorded reason (rebound
    /// is never silently defaulted to zero).
    ReboundZeroWithoutReason,
    /// A rebound zero-reason (or any reason field) was empty.
    EmptyReasonField {
        /// The offending field.
        field: &'static str,
    },
    /// A baseline-method name other than the decided synthetic method was
    /// supplied (the baseline rule is decided once — see the module header).
    UnknownBaselineMethod {
        /// The rejected name.
        name: String,
        /// The only accepted method.
        accepted: &'static str,
    },
    /// A comfort/process constraint descriptor was empty.
    EmptyComfortConstraint,
    /// An availability window was inverted (start ≥ end).
    InvertedAvailabilityWindow {
        /// The offending window index.
        index: usize,
        /// The window start in UTC milliseconds.
        start_millis: i64,
        /// The window end in UTC milliseconds.
        end_millis: i64,
    },
    /// No availability window was declared; availability windows are
    /// required configuration (never "always available" by omission).
    NoAvailabilityWindowDeclared,
    /// The interval length was zero (Δt must be positive).
    NonPositiveIntervalSeconds,
    /// The schedule's setpoint count differed from the model's interval
    /// count.
    ScheduleLengthMismatch {
        /// Number of setpoints supplied.
        setpoints: usize,
        /// Number of intervals in the model's horizon.
        intervals: usize,
    },
    /// A setpoint value was NaN or infinite.
    NonFiniteSetpoint {
        /// The interval index.
        interval: u32,
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
}

impl std::fmt::Display for LoadModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadModelError::NonFiniteParameter { field, value } => {
                write!(f, "parameter '{field}' must be finite (found {value})")
            }
            LoadModelError::EmptyBaselineProfile => {
                write!(f, "the scenario-declared baseline profile must contain at least one interval")
            }
            LoadModelError::InvalidBaselineDemand { interval, value } => {
                write!(f, "interval {interval}: baseline demand {value} MW must be a finite non-negative magnitude")
            }
            LoadModelError::NegativeRange { field, value } => {
                write!(f, "parameter '{field}' must be non-negative (found {value} MW)")
            }
            LoadModelError::NonPositiveMaxEventDuration { value } => {
                write!(f, "the maximum event duration must be positive (found {value} s)")
            }
            LoadModelError::InvalidReboundEnergy { value } => {
                write!(f, "rebound energy {value} MWh must be finite and non-negative")
            }
            LoadModelError::ReboundZeroWithoutReason => write!(
                f,
                "zero rebound energy requires an explicitly recorded reason (rebound is never defaulted to zero, ledger P3.10)"
            ),
            LoadModelError::EmptyReasonField { field } => {
                write!(f, "recorded reason '{field}' must be non-empty")
            }
            LoadModelError::UnknownBaselineMethod { name, accepted } => write!(
                f,
                "baseline method '{name}' is not supported (the decided synthetic method is '{accepted}'; market/DR baselines are out of scope, §11.4)"
            ),
            LoadModelError::EmptyComfortConstraint => {
                write!(f, "comfort/process constraint descriptors must be non-empty")
            }
            LoadModelError::InvertedAvailabilityWindow { index, start_millis, end_millis } => {
                write!(
                    f,
                    "availability window {index} is inverted (start {start_millis} >= end {end_millis} UTC ms)"
                )
            }
            LoadModelError::NoAvailabilityWindowDeclared => {
                write!(f, "at least one availability window must be declared (availability is never 'always' by omission)")
            }
            LoadModelError::NonPositiveIntervalSeconds => {
                write!(f, "the scheduling interval length must be positive")
            }
            LoadModelError::ScheduleLengthMismatch { setpoints, intervals } => {
                write!(f, "the schedule has {setpoints} setpoints but the horizon has {intervals} intervals")
            }
            LoadModelError::NonFiniteSetpoint { interval, field, value } => {
                write!(f, "interval {interval}: setpoint '{field}' must be finite (found {value})")
            }
        }
    }
}

impl std::error::Error for LoadModelError {}

// --- Configuration types -----------------------------------------------------

/// The declared baseline method. Construction is restricted to the decided
/// synthetic method ([`SCENARIO_DECLARED_BASELINE_METHOD`]) — any other name
/// is rejected, so a market/DR baseline cannot enter through configuration
/// (report §11.4: baseline and measurement rules are region/product-
/// specific; D1/D2 open).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineMethod(String);

impl BaselineMethod {
    /// Constructs a baseline method, rejecting any name other than the
    /// decided synthetic method.
    pub fn try_new(name: String) -> Result<Self, LoadModelError> {
        if name != SCENARIO_DECLARED_BASELINE_METHOD {
            return Err(LoadModelError::UnknownBaselineMethod {
                name,
                accepted: SCENARIO_DECLARED_BASELINE_METHOD,
            });
        }
        Ok(Self(name))
    }

    /// The decided synthetic baseline method (the only supported one).
    pub fn scenario_declared() -> Self {
        Self(SCENARIO_DECLARED_BASELINE_METHOD.to_string())
    }

    /// The method name.
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// The REQUIRED rebound-energy field (ledger P3.10 risk note): zero only if
/// explicitly configured as zero WITH a recorded reason — never defaulted.
#[derive(Debug, Clone, PartialEq)]
pub struct ReboundEnergy {
    energy_mwh: f64,
    explicit_zero_reason: Option<String>,
}

impl ReboundEnergy {
    /// A non-zero rebound energy in MWh (finite, > 0). No reason is
    /// recorded (there is nothing to justify).
    pub fn nonzero(energy_mwh: f64) -> Result<Self, LoadModelError> {
        if !energy_mwh.is_finite() || energy_mwh <= 0.0 {
            return Err(LoadModelError::InvalidReboundEnergy { value: energy_mwh });
        }
        Ok(Self { energy_mwh, explicit_zero_reason: None })
    }

    /// An EXPLICIT zero rebound energy with a mandatory recorded reason.
    /// The reason is the durable record of why the load does not rebound.
    pub fn explicit_zero(reason: String) -> Result<Self, LoadModelError> {
        if reason.trim().is_empty() {
            return Err(LoadModelError::EmptyReasonField { field: "rebound_zero_reason" });
        }
        Ok(Self { energy_mwh: 0.0, explicit_zero_reason: Some(reason) })
    }

    /// The rebound energy in MWh (0.0 only when explicitly declared zero).
    pub fn energy_mwh(&self) -> f64 {
        self.energy_mwh
    }

    /// The recorded zero-reason, present exactly when the energy is an
    /// explicit zero.
    pub fn zero_reason(&self) -> Option<&str> {
        self.explicit_zero_reason.as_deref()
    }
}

/// The full flexible-load model configuration (report §11.4). All fields are
/// required configuration; nothing is defaulted and nothing is invented.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexibleLoadConfig {
    /// The declared baseline method — only the decided synthetic method is
    /// accepted (see the module header).
    pub baseline_method: BaselineMethod,
    /// The scenario-declared baseline demand per interval, as a non-negative
    /// consumption magnitude in MW (one value per interval; length must
    /// match the interval count implied by the horizon and interval length —
    /// see [`FlexibleLoadModel::new`]).
    pub baseline_demand_mw: Vec<f64>,
    /// Reducible power range in MW with provenance (UNKNOWN narrows per the
    /// frozen P3.02 rule — zero credit).
    pub reducible_range_mw: ProvenancedValue,
    /// Increasable power range in MW with provenance (same narrowing rule).
    pub increasable_range_mw: ProvenancedValue,
    /// Response time in seconds (≥ 0). Recorded on every evaluation; the
    /// response LAG is not enforced at interval resolution (a P3.17
    /// formulation concern) — the omission is recorded as an unknown
    /// dimension.
    pub response_time_seconds: f64,
    /// Maximum event duration in seconds (> 0).
    pub max_event_duration_seconds: f64,
    /// Minimum recovery time between events in seconds (≥ 0).
    pub min_recovery_seconds: f64,
    /// The REQUIRED rebound energy (zero only with a recorded reason).
    pub rebound: ReboundEnergy,
    /// Declared comfort/process constraint descriptors (§11.4). Descriptors
    /// only — no numeric comfort law exists in the scenario and none is
    /// invented; undeclared constraints are recorded as an unknown
    /// dimension.
    pub comfort_constraints: Vec<String>,
    /// Availability windows in UTC milliseconds, start-inclusive and
    /// end-exclusive (§10.3 interval semantics). Required: at least one
    /// window; dispatch outside every window is a named violation.
    pub availability_windows_millis: Vec<(i64, i64)>,
    /// Interval length in seconds (§10.3 per-model configuration).
    pub interval_seconds: u32,
    /// The start-inclusive horizon start in UTC milliseconds (§10.3).
    pub horizon_start_millis: i64,
}

// --- Schedule types ------------------------------------------------------------

/// One interval's demand-side setpoint: non-negative magnitudes for the
/// reduction (demand below baseline) and increase (demand above baseline).
/// Under the frozen `P > 0 = export` rule, a reduction RAISES the net export
/// relative to the baseline and an increase LOWERS it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSetpoint {
    /// Demand reduction in MW (non-negative magnitude, ≤ the effective
    /// reducible range).
    pub reduction_mw: f64,
    /// Demand increase in MW (non-negative magnitude, ≤ the effective
    /// increasable range).
    pub increase_mw: f64,
}

impl LoadSetpoint {
    /// An idle (follow-the-baseline) setpoint.
    pub fn idle() -> Self {
        Self { reduction_mw: 0.0, increase_mw: 0.0 }
    }

    /// A reduction setpoint.
    pub fn reduce(reduction_mw: f64) -> Self {
        Self { reduction_mw, increase_mw: 0.0 }
    }

    /// An increase setpoint.
    pub fn increase(increase_mw: f64) -> Self {
        Self { reduction_mw: 0.0, increase_mw }
    }
}

/// A candidate flexible-load schedule over the model's interval grid: one
/// setpoint per interval.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadSchedule {
    /// One setpoint per interval of the horizon.
    pub setpoints: Vec<LoadSetpoint>,
}

// --- Violations -----------------------------------------------------------------

/// A named, structured flexible-load violation (EXCLUDE-AND-FLAG policy:
/// nothing is clipped or silently credited). Every variant names the
/// interval or event, the offending value, and the limit.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadViolation {
    /// A negative setpoint magnitude (magnitudes are non-negative; a
    /// negative value is a violation, never reinterpreted as the opposite
    /// direction).
    NegativeMagnitude {
        /// The interval index.
        interval: u32,
        /// The offending field.
        field: &'static str,
        /// The offending value in MW.
        value_mw: f64,
    },
    /// Reduction above the effective reducible range for that interval
    /// (declared range ∩ provenance narrowing).
    ReductionAboveRange {
        /// The interval index.
        interval: u32,
        /// The declared reduction in MW.
        reduction_mw: f64,
        /// The effective range limit in MW.
        limit_mw: f64,
    },
    /// Increase above the effective increasable range for that interval.
    IncreaseAboveRange {
        /// The interval index.
        interval: u32,
        /// The declared increase in MW.
        increase_mw: f64,
        /// The effective range limit in MW.
        limit_mw: f64,
    },
    /// A setpoint outside every declared availability window (ledger
    /// acceptance: no load is dispatched outside its availability window).
    DispatchOutsideAvailabilityWindow {
        /// The interval index.
        interval: u32,
        /// The interval start in UTC milliseconds.
        interval_start_millis: i64,
    },
    /// A reduction event exceeded the maximum event duration. The event is
    /// flagged and EXCLUDED from credited flexibility (the decided
    /// EXCLUDE-AND-FLAG policy).
    EventDurationExceeded {
        /// The index of the event's first interval.
        event_start_interval: u32,
        /// The event duration in seconds.
        duration_seconds: f64,
        /// The configured maximum in seconds.
        max_event_duration_seconds: f64,
    },
    /// A reduction event started before the previous event's minimum
    /// recovery time completed. The event is flagged and EXCLUDED from
    /// credited flexibility.
    RecoveryIncomplete {
        /// The index of the (too-early) event's first interval.
        event_start_interval: u32,
        /// The actual recovery gap in seconds.
        gap_seconds: f64,
        /// The configured minimum in seconds.
        min_recovery_seconds: f64,
    },
    /// The scheduled increase did not cover the rebound owed by the credited
    /// events; the unpaid remainder is subtracted from the net flexibility
    /// (it will occur beyond the horizon — the net never overstates).
    ReboundPaybackIncomplete {
        /// The rebound owed by credited events, in MWh.
        rebound_owed_mwh: f64,
        /// The scheduled increase over the horizon, in MWh.
        scheduled_increase_mwh: f64,
        /// The unpaid remainder in MWh.
        unpaid_mwh: f64,
    },
}

impl std::fmt::Display for LoadViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadViolation::NegativeMagnitude { interval, field, value_mw } => {
                write!(f, "interval {interval}: setpoint '{field}' is {value_mw} MW; magnitudes must be non-negative")
            }
            LoadViolation::ReductionAboveRange { interval, reduction_mw, limit_mw } => {
                write!(f, "interval {interval}: reduction {reduction_mw} MW exceeds the effective reducible range {limit_mw} MW")
            }
            LoadViolation::IncreaseAboveRange { interval, increase_mw, limit_mw } => {
                write!(f, "interval {interval}: increase {increase_mw} MW exceeds the effective increasable range {limit_mw} MW")
            }
            LoadViolation::DispatchOutsideAvailabilityWindow { interval, interval_start_millis } => {
                write!(
                    f,
                    "interval {interval} (start {interval_start_millis} UTC ms) lies outside every declared availability window"
                )
            }
            LoadViolation::EventDurationExceeded { event_start_interval, duration_seconds, max_event_duration_seconds } => {
                write!(
                    f,
                    "event starting at interval {event_start_interval} holds {duration_seconds} s, above the maximum event duration {max_event_duration_seconds} s (flagged and excluded, EXCLUDE-AND-FLAG policy)"
                )
            }
            LoadViolation::RecoveryIncomplete { event_start_interval, gap_seconds, min_recovery_seconds } => {
                write!(
                    f,
                    "event starting at interval {event_start_interval} begins after a {gap_seconds} s gap, below the minimum recovery time {min_recovery_seconds} s (flagged and excluded, EXCLUDE-AND-FLAG policy)"
                )
            }
            LoadViolation::ReboundPaybackIncomplete { rebound_owed_mwh, scheduled_increase_mwh, unpaid_mwh } => {
                write!(
                    f,
                    "rebound payback incomplete: {rebound_owed_mwh} MWh owed, {scheduled_increase_mwh} MWh scheduled, {unpaid_mwh} MWh unpaid (subtracted from the net flexibility — it never overstates)"
                )
            }
        }
    }
}

/// One dimension recorded as unknown on an evaluation (P3.02: unknown is
/// visible, never silently treated as unconstrained or filled with invented
/// values).
#[derive(Debug, Clone, PartialEq)]
pub struct LoadUnknownDimension {
    /// The dimension name (frozen vocabulary string).
    pub dimension: &'static str,
    /// The recorded explanation (audit-safe).
    pub note: String,
}

// --- The pure event-enforcement function (P3.12 / P3.17 reuse) ------------------

/// A declared reduction event over the scenario clock: a contiguous run of
/// intervals with a declared reduction power. Start-inclusive, end-exclusive
/// (§10.3 interval semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredEvent {
    /// The event start in UTC milliseconds.
    pub start_millis: i64,
    /// The event end (exclusive) in UTC milliseconds.
    pub end_millis: i64,
    /// The declared reduction power in MW (constant over the event).
    pub power_mw: f64,
}

/// A violation returned by the pure event-enforcement rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventViolation {
    /// The event duration exceeds the configured maximum.
    DurationExceeded {
        /// The event duration in seconds.
        duration_seconds: f64,
        /// The configured maximum in seconds.
        max_event_duration_seconds: f64,
    },
    /// The recovery gap since the previous event is below the configured
    /// minimum.
    RecoveryIncomplete {
        /// The actual gap in seconds.
        gap_seconds: f64,
        /// The configured minimum in seconds.
        min_recovery_seconds: f64,
    },
}

/// THE pure duration/recovery enforcement function, DECIDED ONCE — the
/// single implementation the P3.12 envelope and the P3.17 scheduler reuse
/// rather than reimplement (ledger P3.10 step 3; EXCLUDE-AND-FLAG policy:
/// the caller flags and excludes on any returned violation; this function
/// never clips or repairs).
///
/// - Duration: `end − start` must not exceed `max_event_duration_seconds`
///   (boundary-inclusive within [`LOAD_EVALUATION_TOLERANCE`]).
/// - Recovery: the gap from the previous event's end (exclusive) to this
///   event's start must be at least `min_recovery_seconds` (boundary-
///   inclusive). The first event of the horizon has no predecessor and no
///   recovery obligation.
pub fn enforce_event(
    event: &DeclaredEvent,
    previous_event: Option<&DeclaredEvent>,
    max_event_duration_seconds: f64,
    min_recovery_seconds: f64,
) -> Vec<EventViolation> {
    let mut violations = Vec::new();
    let duration_seconds = (event.end_millis - event.start_millis) as f64 / 1000.0;
    if duration_seconds > max_event_duration_seconds + LOAD_EVALUATION_TOLERANCE {
        violations.push(EventViolation::DurationExceeded {
            duration_seconds,
            max_event_duration_seconds,
        });
    }
    if let Some(previous) = previous_event {
        let gap_seconds = (event.start_millis - previous.end_millis) as f64 / 1000.0;
        if gap_seconds + LOAD_EVALUATION_TOLERANCE < min_recovery_seconds {
            violations.push(EventViolation::RecoveryIncomplete {
                gap_seconds,
                min_recovery_seconds,
            });
        }
    }
    violations
}

// --- Evaluation output ---------------------------------------------------------

/// One derived reduction event with its credit status.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadEventRecord {
    /// The index of the event's first interval.
    pub start_interval: u32,
    /// The number of intervals the event spans.
    pub intervals: u32,
    /// The event duration in seconds.
    pub duration_seconds: f64,
    /// The declared event energy in MWh (Σ reduction·Δt over the event's
    /// intervals).
    pub declared_energy_mwh: f64,
    /// Whether the event is credited (false = flagged by the
    /// EXCLUDE-AND-FLAG policy and excluded from credit and rebound).
    pub credited: bool,
    /// The rebound payback owed by this event in MWh (0.0 for excluded
    /// events).
    pub rebound_owed_mwh: f64,
}

/// The evaluation of a candidate flexible-load schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexibleLoadEvaluation {
    /// The declared reduction energy over the horizon in MWh (Σ
    /// reduction·Δt, including flagged events).
    pub declared_reduction_mwh: f64,
    /// The credited reduction energy in MWh (declared minus the energy of
    /// flagged, excluded events).
    pub credited_reduction_mwh: f64,
    /// The scheduled increase energy over the horizon in MWh (Σ increase·Δt;
    /// treated as rebound payback under the recorded conservative
    /// assumption).
    pub scheduled_increase_mwh: f64,
    /// The rebound payback owed by the credited events in MWh.
    pub rebound_owed_mwh: f64,
    /// The unpaid rebound remainder in MWh (`max(0, owed − scheduled
    /// increase)`).
    pub unpaid_rebound_mwh: f64,
    /// The net flexibility in MWh: `credited − max(owed, scheduled
    /// increase)` — never overstates the deliverable flexibility (the
    /// recorded rebound convention).
    pub net_flexibility_mwh: f64,
    /// The net active power per interval in MW under the frozen
    /// `P > 0 = export` rule: `reduction − increase − baseline` (the
    /// baseline demand is consumption, hence a negative export term).
    pub net_export_mw: Vec<f64>,
    /// The derived reduction events with their credit status.
    pub events: Vec<LoadEventRecord>,
    /// All named violations; empty = the schedule carries no flagged events
    /// and no boundary violations.
    pub violations: Vec<LoadViolation>,
    /// The baseline method name (the decided synthetic method).
    pub baseline_method_name: String,
    /// The recorded synthetic-baseline disclaimer
    /// ([`SYNTHETIC_BASELINE_DISCLAIMER`]).
    pub baseline_disclaimer: &'static str,
    /// The recorded duration/recovery policy statement
    /// ([`DURATION_RECOVERY_POLICY_STATEMENT`]).
    pub duration_recovery_policy: &'static str,
    /// The recorded rebound convention statement
    /// ([`REBOUND_CONVENTION_STATEMENT`]).
    pub rebound_convention: &'static str,
    /// Dimensions recorded unknown for this evaluation (P3.02: visible,
    /// never silently unconstrained).
    pub unknown_dimensions: Vec<LoadUnknownDimension>,
}

// --- The model -----------------------------------------------------------------

/// The flexible-load capability model (report §11.4): baseline, ranges,
/// duration/recovery/rebound, comfort/process constraints, availability
/// windows. Pure feasibility/limits layer — no control, no baseline-method
/// claims.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexibleLoadModel {
    baseline_method: BaselineMethod,
    baseline_demand_mw: Vec<f64>,
    declared_reducible_mw: f64,
    declared_increasable_mw: f64,
    effective_reducible_mw: f64,
    effective_increasable_mw: f64,
    reducible_provenance: Provenance,
    increasable_provenance: Provenance,
    response_time_seconds: f64,
    max_event_duration_seconds: f64,
    min_recovery_seconds: f64,
    rebound: ReboundEnergy,
    comfort_constraints: Vec<String>,
    availability_windows_millis: Vec<(i64, i64)>,
    interval_seconds: u32,
    horizon_start_millis: i64,
}

fn require_finite(field: &'static str, value: f64) -> Result<(), LoadModelError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(LoadModelError::NonFiniteParameter { field, value })
    }
}

impl FlexibleLoadModel {
    /// Constructs and validates the model. The interval count is derived
    /// from the horizon the caller declares through the length of
    /// `baseline_demand_mw` (the scenario-declared per-interval profile);
    /// the interval length must be positive. Unknown-provenance ranges
    /// narrow per the frozen P3.02 rule (zero credit — never widened).
    pub fn new(config: FlexibleLoadConfig) -> Result<Self, LoadModelError> {
        if config.interval_seconds == 0 {
            return Err(LoadModelError::NonPositiveIntervalSeconds);
        }
        if config.baseline_demand_mw.is_empty() {
            return Err(LoadModelError::EmptyBaselineProfile);
        }
        for (index, value) in config.baseline_demand_mw.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(LoadModelError::InvalidBaselineDemand {
                    interval: index as u32,
                    value: *value,
                });
            }
        }
        // Range fields carry provenance: an UNKNOWN-provenance value carries
        // no declared number (the BESS `ProvenancedValue::unknown` marker),
        // so finiteness/ordering validation applies only to known
        // provenance. The unknown case narrows below (zero credit).
        if config.reducible_range_mw.provenance != Provenance::Unknown {
            require_finite("reducible_range_mw", config.reducible_range_mw.value)?;
            if config.reducible_range_mw.value < 0.0 {
                return Err(LoadModelError::NegativeRange {
                    field: "reducible_range_mw",
                    value: config.reducible_range_mw.value,
                });
            }
        }
        if config.increasable_range_mw.provenance != Provenance::Unknown {
            require_finite("increasable_range_mw", config.increasable_range_mw.value)?;
            if config.increasable_range_mw.value < 0.0 {
                return Err(LoadModelError::NegativeRange {
                    field: "increasable_range_mw",
                    value: config.increasable_range_mw.value,
                });
            }
        }
        for (field, value) in [
            ("response_time_seconds", config.response_time_seconds),
            ("min_recovery_seconds", config.min_recovery_seconds),
        ] {
            require_finite(field, value)?;
            if value < 0.0 {
                return Err(LoadModelError::NegativeRange { field, value });
            }
        }
        if config.max_event_duration_seconds <= 0.0 || !config.max_event_duration_seconds.is_finite()
        {
            return Err(LoadModelError::NonPositiveMaxEventDuration {
                value: config.max_event_duration_seconds,
            });
        }
        for constraint in &config.comfort_constraints {
            if constraint.trim().is_empty() {
                return Err(LoadModelError::EmptyComfortConstraint);
            }
        }
        if config.availability_windows_millis.is_empty() {
            return Err(LoadModelError::NoAvailabilityWindowDeclared);
        }
        for (index, (start, end)) in config.availability_windows_millis.iter().enumerate() {
            if start >= end {
                return Err(LoadModelError::InvertedAvailabilityWindow {
                    index,
                    start_millis: *start,
                    end_millis: *end,
                });
            }
        }

        // Unknown provenance narrows (frozen P3.02 rule, reused): the
        // effective range is the declared [0, range] window intersected with
        // the conservative default for FLEXIBLE_LOAD / ACTIVE_POWER. For
        // UNKNOWN provenance the collapse-to-zero default gives zero
        // credit; for known provenance the declared range is kept.
        let effective_reducible = if config.reducible_range_mw.provenance == Provenance::Unknown {
            // Unknown narrows to zero credit directly (the collapse-to-zero
            // fallback for FLEXIBLE_LOAD / ACTIVE_POWER); no declared number
            // exists to intersect with.
            0.0
        } else {
            let declared_reducible = BoundInterval::new(
                Some(0.0),
                Some(config.reducible_range_mw.value),
            )
            .map_err(|_| LoadModelError::NonFiniteParameter {
                field: "reducible_range_mw",
                value: config.reducible_range_mw.value,
            })?;
            let narrowed = narrow_interval(
                AssetClass::FlexibleLoad,
                flexcore_contracts::capability::ConstraintKind::ActivePower,
                config.reducible_range_mw.provenance,
                declared_reducible,
            );
            match narrowed {
                crate::capability_provenance::NarrowedSet::Interval { upper: Some(u), .. } => u,
                crate::capability_provenance::NarrowedSet::Interval { upper: None, .. } => {
                    config.reducible_range_mw.value
                }
                crate::capability_provenance::NarrowedSet::Empty => 0.0,
            }
        };
        let effective_increasable = if config.increasable_range_mw.provenance == Provenance::Unknown {
            0.0
        } else {
            let declared_increasable = BoundInterval::new(
                Some(0.0),
                Some(config.increasable_range_mw.value),
            )
            .map_err(|_| LoadModelError::NonFiniteParameter {
                field: "increasable_range_mw",
                value: config.increasable_range_mw.value,
            })?;
            let narrowed = narrow_interval(
                AssetClass::FlexibleLoad,
                flexcore_contracts::capability::ConstraintKind::ActivePower,
                config.increasable_range_mw.provenance,
                declared_increasable,
            );
            match narrowed {
                crate::capability_provenance::NarrowedSet::Interval { upper: Some(u), .. } => u,
                crate::capability_provenance::NarrowedSet::Interval { upper: None, .. } => {
                    config.increasable_range_mw.value
                }
                crate::capability_provenance::NarrowedSet::Empty => 0.0,
            }
        };

        Ok(Self {
            baseline_method: config.baseline_method,
            baseline_demand_mw: config.baseline_demand_mw,
            declared_reducible_mw: config.reducible_range_mw.value,
            declared_increasable_mw: config.increasable_range_mw.value,
            effective_reducible_mw: effective_reducible,
            effective_increasable_mw: effective_increasable,
            reducible_provenance: config.reducible_range_mw.provenance,
            increasable_provenance: config.increasable_range_mw.provenance,
            response_time_seconds: config.response_time_seconds,
            max_event_duration_seconds: config.max_event_duration_seconds,
            min_recovery_seconds: config.min_recovery_seconds,
            rebound: config.rebound,
            comfort_constraints: config.comfort_constraints,
            availability_windows_millis: config.availability_windows_millis,
            interval_seconds: config.interval_seconds,
            horizon_start_millis: config.horizon_start_millis,
        })
    }

    /// The declared baseline method.
    pub fn baseline_method(&self) -> &BaselineMethod {
        &self.baseline_method
    }

    /// The declared reducible range in MW.
    pub fn declared_reducible_mw(&self) -> f64 {
        self.declared_reducible_mw
    }

    /// The declared increasable range in MW.
    pub fn declared_increasable_mw(&self) -> f64 {
        self.declared_increasable_mw
    }

    /// The effective reducible range in MW (after provenance narrowing).
    pub fn effective_reducible_mw(&self) -> f64 {
        self.effective_reducible_mw
    }

    /// The effective increasable range in MW (after provenance narrowing).
    pub fn effective_increasable_mw(&self) -> f64 {
        self.effective_increasable_mw
    }

    /// The declared provenance of the reducible range.
    pub fn reducible_provenance(&self) -> Provenance {
        self.reducible_provenance
    }

    /// The declared provenance of the increasable range.
    pub fn increasable_provenance(&self) -> Provenance {
        self.increasable_provenance
    }

    /// The configured response time in seconds.
    pub fn response_time_seconds(&self) -> f64 {
        self.response_time_seconds
    }

    /// The configured maximum event duration in seconds.
    pub fn max_event_duration_seconds(&self) -> f64 {
        self.max_event_duration_seconds
    }

    /// The configured minimum recovery time in seconds.
    pub fn min_recovery_seconds(&self) -> f64 {
        self.min_recovery_seconds
    }

    /// The configured rebound energy.
    pub fn rebound(&self) -> &ReboundEnergy {
        &self.rebound
    }

    /// The declared comfort/process constraint descriptors.
    pub fn comfort_constraints(&self) -> &[String] {
        &self.comfort_constraints
    }

    /// The declared availability windows (UTC ms, start-inclusive,
    /// end-exclusive).
    pub fn availability_windows_millis(&self) -> &[(i64, i64)] {
        &self.availability_windows_millis
    }

    /// The interval length in seconds.
    pub const fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    /// The number of intervals in the horizon (the baseline profile length).
    pub fn interval_count(&self) -> usize {
        self.baseline_demand_mw.len()
    }

    /// The start-inclusive start of interval `index` in UTC milliseconds.
    pub fn interval_start_millis(&self, index: usize) -> i64 {
        self.horizon_start_millis
            + i64::from(self.interval_seconds) * 1000 * index as i64
    }

    /// Whether an instant (an interval start) lies inside any availability
    /// window (start-inclusive, end-exclusive).
    pub fn available_at(&self, instant_millis: i64) -> bool {
        self.availability_windows_millis
            .iter()
            .any(|(start, end)| instant_millis >= *start && instant_millis < *end)
    }

    /// Evaluates a candidate schedule against the model. All applicable
    /// constraints are checked and every violation is reported; flagged
    /// events are excluded from credited flexibility (EXCLUDE-AND-FLAG).
    pub fn evaluate_schedule(
        &self,
        schedule: &LoadSchedule,
    ) -> Result<FlexibleLoadEvaluation, LoadModelError> {
        if schedule.setpoints.len() != self.interval_count() {
            return Err(LoadModelError::ScheduleLengthMismatch {
                setpoints: schedule.setpoints.len(),
                intervals: self.interval_count(),
            });
        }
        let interval_millis = i64::from(self.interval_seconds) * 1000;
        let delta_hours = i64::from(self.interval_seconds) as f64 / 3600.0;

        let mut violations: Vec<LoadViolation> = Vec::new();
        let mut net_export_mw = Vec::with_capacity(self.interval_count());
        let mut declared_reduction_mwh = 0.0;
        let mut scheduled_increase_mwh = 0.0;

        for (index, setpoint) in schedule.setpoints.iter().enumerate() {
            if !setpoint.reduction_mw.is_finite() {
                return Err(LoadModelError::NonFiniteSetpoint {
                    interval: index as u32,
                    field: "reduction_mw",
                    value: setpoint.reduction_mw,
                });
            }
            if !setpoint.increase_mw.is_finite() {
                return Err(LoadModelError::NonFiniteSetpoint {
                    interval: index as u32,
                    field: "increase_mw",
                    value: setpoint.increase_mw,
                });
            }
            if setpoint.reduction_mw < 0.0 {
                violations.push(LoadViolation::NegativeMagnitude {
                    interval: index as u32,
                    field: "reduction_mw",
                    value_mw: setpoint.reduction_mw,
                });
            }
            if setpoint.increase_mw < 0.0 {
                violations.push(LoadViolation::NegativeMagnitude {
                    interval: index as u32,
                    field: "increase_mw",
                    value_mw: setpoint.increase_mw,
                });
            }
            if setpoint.reduction_mw > self.effective_reducible_mw + LOAD_EVALUATION_TOLERANCE {
                violations.push(LoadViolation::ReductionAboveRange {
                    interval: index as u32,
                    reduction_mw: setpoint.reduction_mw,
                    limit_mw: self.effective_reducible_mw,
                });
            }
            if setpoint.increase_mw > self.effective_increasable_mw + LOAD_EVALUATION_TOLERANCE {
                violations.push(LoadViolation::IncreaseAboveRange {
                    interval: index as u32,
                    increase_mw: setpoint.increase_mw,
                    limit_mw: self.effective_increasable_mw,
                });
            }
            let start = self.interval_start_millis(index);
            let dispatching =
                setpoint.reduction_mw > LOAD_EVALUATION_TOLERANCE
                    || setpoint.increase_mw > LOAD_EVALUATION_TOLERANCE;
            if dispatching && !self.available_at(start) {
                violations.push(LoadViolation::DispatchOutsideAvailabilityWindow {
                    interval: index as u32,
                    interval_start_millis: start,
                });
            }
            declared_reduction_mwh += setpoint.reduction_mw * delta_hours;
            scheduled_increase_mwh += setpoint.increase_mw * delta_hours;
            // P > 0 = export (§10.2): the baseline demand is consumption
            // (negative export); a reduction raises the net export.
            net_export_mw
                .push(setpoint.reduction_mw - setpoint.increase_mw - self.baseline_demand_mw[index]);
        }

        // --- events: contiguous runs of reduction above tolerance ---------
        let mut events: Vec<LoadEventRecord> = Vec::new();
        let mut previous_event: Option<DeclaredEvent> = None;
        let mut index = 0usize;
        while index < schedule.setpoints.len() {
            if schedule.setpoints[index].reduction_mw > LOAD_EVALUATION_TOLERANCE {
                let start_interval = index;
                let mut run = 1usize;
                while index + run < schedule.setpoints.len()
                    && schedule.setpoints[index + run].reduction_mw
                        > LOAD_EVALUATION_TOLERANCE
                {
                    run += 1;
                }
                let start_millis = self.interval_start_millis(start_interval);
                let end_millis = start_millis + interval_millis * run as i64;
                let event = DeclaredEvent {
                    start_millis,
                    end_millis,
                    power_mw: schedule.setpoints[start_interval].reduction_mw,
                };
                let event_violations = enforce_event(
                    &event,
                    previous_event.as_ref(),
                    self.max_event_duration_seconds,
                    self.min_recovery_seconds,
                );
                let duration_seconds = (end_millis - start_millis) as f64 / 1000.0;
                // The declared event energy sums each interval's reduction —
                // no constant-power assumption across the run.
                let declared_energy_mwh: f64 = schedule.setpoints
                    [start_interval..start_interval + run]
                    .iter()
                    .map(|setpoint| setpoint.reduction_mw * delta_hours)
                    .sum();
                let mut credited = true;
                for violation in event_violations {
                    credited = false;
                    match violation {
                        EventViolation::DurationExceeded {
                            duration_seconds,
                            max_event_duration_seconds,
                        } => violations.push(LoadViolation::EventDurationExceeded {
                            event_start_interval: start_interval as u32,
                            duration_seconds,
                            max_event_duration_seconds,
                        }),
                        EventViolation::RecoveryIncomplete {
                            gap_seconds,
                            min_recovery_seconds,
                        } => violations.push(LoadViolation::RecoveryIncomplete {
                            event_start_interval: start_interval as u32,
                            gap_seconds,
                            min_recovery_seconds,
                        }),
                    }
                }
                let rebound_owed_mwh = if credited {
                    self.rebound.energy_mwh()
                        * (duration_seconds / self.max_event_duration_seconds)
                } else {
                    0.0
                };
                events.push(LoadEventRecord {
                    start_interval: start_interval as u32,
                    intervals: run as u32,
                    duration_seconds,
                    declared_energy_mwh,
                    credited,
                    rebound_owed_mwh,
                });
                previous_event = Some(event);
                index += run;
            } else {
                index += 1;
            }
        }

        // --- rebound accounting (net flexibility never overstates) --------
        let credited_reduction_mwh = declared_reduction_mwh
            - events
                .iter()
                .filter(|event| !event.credited)
                .map(|event| event.declared_energy_mwh)
                .sum::<f64>();
        let rebound_owed_mwh: f64 =
            events.iter().map(|event| event.rebound_owed_mwh).sum();
        let unpaid_rebound_mwh =
            (rebound_owed_mwh - scheduled_increase_mwh).max(0.0);
        if unpaid_rebound_mwh > LOAD_EVALUATION_TOLERANCE {
            violations.push(LoadViolation::ReboundPaybackIncomplete {
                rebound_owed_mwh,
                scheduled_increase_mwh,
                unpaid_mwh: unpaid_rebound_mwh,
            });
        }
        let net_flexibility_mwh =
            credited_reduction_mwh - rebound_owed_mwh.max(scheduled_increase_mwh);

        // --- recorded unknown dimensions ----------------------------------
        let mut unknown_dimensions = vec![LoadUnknownDimension {
            dimension: "RESPONSE_TIME_LAG",
            note: "the configured response time is recorded; its response LAG \
is not enforced at interval resolution (a cross-interval formulation concern \
of the P3.17 scheduler) — the omission is recorded, never silent"
                .to_string(),
        }];
        if self.comfort_constraints.is_empty() {
            unknown_dimensions.push(LoadUnknownDimension {
                dimension: "COMFORT_PROCESS_CONSTRAINTS",
                note: "no comfort/process constraint is declared; none is \
invented (§11.4 lists them as model data) — the load model enforces only the \
declared duration/recovery/rebound/availability rules"
                    .to_string(),
            });
        }

        Ok(FlexibleLoadEvaluation {
            declared_reduction_mwh,
            credited_reduction_mwh,
            scheduled_increase_mwh,
            rebound_owed_mwh,
            unpaid_rebound_mwh,
            net_flexibility_mwh,
            net_export_mw,
            events,
            violations,
            baseline_method_name: self.baseline_method.name().to_string(),
            baseline_disclaimer: SYNTHETIC_BASELINE_DISCLAIMER,
            duration_recovery_policy: DURATION_RECOVERY_POLICY_STATEMENT,
            rebound_convention: REBOUND_CONVENTION_STATEMENT,
            unknown_dimensions,
        })
    }
}
