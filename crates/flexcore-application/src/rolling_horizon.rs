// P3.24 — Rolling-horizon re-scheduling (report §12.1, §24 steps 3–4; ledger
//         P3.24). Decision record:
//         `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`.
//
// Rolling-horizon scheduling: re-solve on new information, carrying forward
// the REALISED state, without rewriting history (§12.1; ledger P3.24 step 2:
// "never re-solve the past as if it had not happened").
//
// ## The roll-trigger set, decided once (D-P3.24-1)
//
//   `horizon_advance`, `envelope_invalidation` (the P3.13 trigger
//   evaluation), `asset_opt_out_dropout` (the P3.11 opt-out semantics), and
//   `forecast_update` are the report-traceable re-solve triggers;
//   `operator_request` is the §14.1 PROPOSED-by-operator source. The set is
//   RECORDED in each new schedule revision as a REQUIRED field (the
//   `roll_trigger` column, migration 0014); `initial_solve` names the first
//   revision of a chain (a first solve has no re-solve trigger, and the
//   required column stays total). The stored strings live in
//   `flexcore-storage::schedule_revision::SCHEDULE_ROLL_TRIGGERS`; this
//   module carries the typed form with the SAME strings (both sides pinned
//   by tests; see [`SCHEDULE_ROLL_TRIGGER_STRINGS`]).
//
// ## Realised-state carry-forward and the frozen past (D-P3.24-3)
//
//   - Actual SOC and actual load response enter the new solve as INITIAL
//     CONDITIONS — the past is an input, never a variable.
//   - An interval is FROZEN iff it started before the execution boundary
//     (`interval_start < executed_until`): intervals already executed (and
//     the one in progress) keep their REALISED values. The boundary
//     convention is start-inclusive on purpose — re-solving a partially
//     executed interval would contradict measured reality; the remaining
//     part of the in-progress interval is covered by the next roll.
//   - Only FUTURE intervals (start >= executed_until) are re-optimized.
//     [`project_rolling_schedule`] pastes frozen + future into the new
//     schedule projection and ENFORCES by construction that frozen
//     intervals carry the realised values, that the grid is continuous, and
//     that an opted-out asset is never dispatched at or after its opt-out
//     instant (the P3.11 `DispatchedAfterOptOut` discipline).
//
// ## Overlap and the supersede chain (D-P3.24-4)
//
//   Every roll produces a NEW revision of the SAME schedule chain:
//   revision = prior + 1, the prior revision marked SUPERSEDED (§14.2) with
//   the bidirectional link (P3.22, migration 0014). The actual store calls
//   are the composition root's
//   (`ScheduleRevisionChainStore::mark_revision_superseded` +
//   `insert_schedule_revision` in ONE unit of work); the plan carries the
//   identities both sides of the link.
//
// ## `ScheduleProposed` per new revision (D-P3.24-5)
//
//   [`rolling_horizon_schedule_proposed_publication`] assembles the
//   `ScheduleProposed` outbox entry for the new revision through the P3.21
//   machinery (reused, never restated). The FROZEN payload (P0.14/P1.09)
//   cannot carry the trigger or the supersede link — the gap is recorded in
//   [`SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE`]; the trigger and the link travel
//   in the persisted revision row (migration 0014: `roll_trigger`,
//   `supersedes_revision`, `supersedes_candidate_id`) and the revision's
//   idempotency key pairs the event with the record.
//
// Control status: pure roll planning for the simulation-only 0.x line. No
// solver call (the re-optimized window goes through the `Optimizer` port at
// the composition root), no I/O, no dispatch path. Every projection is a
// CANDIDATE — never feasible, approved, or dispatchable (Phase 4 gates).

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;

use crate::optimization_result_record::{
    schedule_proposed_publication, PersistedResultRecord, ScheduleProposedPublication,
    ScheduleResultError, ScheduleResultRecord,
};

// --- Recorded statements ------------------------------------------------------

/// The trigger set, stated once (D-P3.24-1).
pub const ROLL_TRIGGER_STATEMENT: &str = "The roll trigger set is: \
     horizon_advance, envelope_invalidation (P3.13 trigger evaluation), \
     asset_opt_out_dropout (P3.11 opt-out semantics), forecast_update, \
     operator_request — plus initial_solve for the first revision of a \
     chain. The trigger is a REQUIRED field of every new schedule revision \
     (recorded in migration 0014).";

/// The frozen-past rule, stated once (D-P3.24-3).
pub const FROZEN_INTERVAL_STATEMENT: &str = "Intervals already executed keep \
     their REALISED values: an interval is frozen iff it started before the \
     execution boundary (start-inclusive, so the in-progress interval is \
     frozen too). The frozen intervals are immutable INPUTS of the new \
     solve — only future intervals (start >= executed_until) are \
     re-optimized. Re-solving the past as if it had not happened is a \
     violation, enforced by construction and by test.";

/// The carry-forward rule, stated once (D-P3.24-3).
pub const CARRY_FORWARD_STATEMENT: &str = "Actual SOC (a fraction in [0,1]) \
     and actual load response enter the new solve as initial conditions — \
     the realised state is the starting point of the new solve, never a \
     variable re-derived from forecasts.";

/// The `ScheduleProposed` payload-fit gap, recorded honestly (the D-FX-9 /
/// D-P3.12-7 / D-P3.21-4 precedent): the frozen `ScheduleProposedPayload`
/// cannot carry the roll trigger or the supersede link.
pub const SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE: &str = "The frozen \
     ScheduleProposedPayload carries correlation/tenant/portfolio/site/asset \
     + event_time only: no roll trigger and no supersede link. Both travel \
     in the persisted revision row (migration 0014: roll_trigger, \
     supersedes_revision, supersedes_candidate_id) and in the revision's \
     deterministic ScheduleProposed idempotency key \
     schedule_proposed:<portfolio>:<site>:<candidate_id> (the P3.21 key, \
     which pairs the event with the record). Needs extension: additive \
     serde(default) payload fields (trigger, superseded_revision) per the \
     P2.16 precedent, for a future subtask that owns flexcore-events.";

/// The stored trigger strings this module's typed set must match EXACTLY
/// (the persistence side lives in
/// `flexcore-storage::schedule_revision::SCHEDULE_ROLL_TRIGGERS`; both sets
/// are pinned by tests so any drift fails visibly).
pub const SCHEDULE_ROLL_TRIGGER_STRINGS: [&str; 6] = [
    "initial_solve",
    "horizon_advance",
    "envelope_invalidation",
    "asset_opt_out_dropout",
    "forecast_update",
    "operator_request",
];

// --- The trigger vocabulary ----------------------------------------------------

/// The roll trigger of one schedule revision (ledger P3.24 step 1). The
/// stored text is pinned against the storage crate's closed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollTrigger {
    /// The first revision of a schedule chain (no re-solve trigger).
    InitialSolve,
    /// The horizon advanced: a new horizon begins (the P3.03 rollover rule —
    /// the prior revision is superseded, never silently overwritten).
    HorizonAdvance,
    /// The envelope was invalidated (a P3.13 staleness verdict); a schedule
    /// never silently uses a stale envelope.
    EnvelopeInvalidation,
    /// An asset opted out / dropped out mid-horizon (P3.11 semantics);
    /// capacity is never silently retained.
    AssetOptOutDropout,
    /// A new forecast version was published (P3.31 / `ForecastPublished`).
    ForecastUpdate,
    /// An operator requested a re-solve.
    OperatorRequest,
}

impl RollTrigger {
    /// The stored text — EXACTLY the string in
    /// `flexcore-storage::schedule_revision::SCHEDULE_ROLL_TRIGGERS`.
    pub fn as_str(self) -> &'static str {
        match self {
            RollTrigger::InitialSolve => "initial_solve",
            RollTrigger::HorizonAdvance => "horizon_advance",
            RollTrigger::EnvelopeInvalidation => "envelope_invalidation",
            RollTrigger::AssetOptOutDropout => "asset_opt_out_dropout",
            RollTrigger::ForecastUpdate => "forecast_update",
            RollTrigger::OperatorRequest => "operator_request",
        }
    }

    /// Every trigger, in the order of the stored vocabulary.
    pub fn all() -> [RollTrigger; 6] {
        [
            RollTrigger::InitialSolve,
            RollTrigger::HorizonAdvance,
            RollTrigger::EnvelopeInvalidation,
            RollTrigger::AssetOptOutDropout,
            RollTrigger::ForecastUpdate,
            RollTrigger::OperatorRequest,
        ]
    }
}

// --- Schedule-shape types --------------------------------------------------------

/// One asset's setpoint in one interval (MW; `P > 0` means export at the
/// point of connection, §10.2).
#[derive(Debug, Clone, PartialEq)]
pub struct AssetSetpoint {
    pub asset_key: String,
    pub p_mw: f64,
}

/// One interval of a schedule (prior plan, realised record, or projection):
/// start-inclusive, end-exclusive (§10.3), aligned to the scheduling grid.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalPlan {
    pub interval_start_millis: i64,
    pub interval_end_millis: i64,
    /// One setpoint per asset (at most; unique per asset).
    pub setpoints: Vec<AssetSetpoint>,
}

/// One realised (measured) asset value in one interval — the measured
/// reality the new solve must respect, never re-derived.
#[derive(Debug, Clone, PartialEq)]
pub struct RealisedAssetValue {
    pub asset_key: String,
    pub realised_p_mw: f64,
}

/// One realised interval (the measured record of an executed interval).
#[derive(Debug, Clone, PartialEq)]
pub struct RealisedInterval {
    pub interval_start_millis: i64,
    pub interval_end_millis: i64,
    pub values: Vec<RealisedAssetValue>,
}

/// One asset's recorded opt-out/dropout instant (P3.11 semantics): the asset
/// leaves the feasible set from `from_millis` onward (start-inclusive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDropout {
    pub asset_key: String,
    pub from_millis: i64,
}

/// The carried-forward initial condition of one asset. `realised_soc` is the
/// ACTUAL state of charge as a fraction in [0,1] (the canonical API-boundary
/// unit, §10.2) — `None` for assets without a storage state (e.g. a
/// flexible load), never for a BESS/EV whose SOC is on record.
#[derive(Debug, Clone, PartialEq)]
pub struct CarriedAssetState {
    pub asset_key: String,
    pub realised_soc: Option<f64>,
}

/// The realised state carried into the new solve (ledger P3.24 step 2):
/// actual SOC per asset, actual load response per executed interval, and the
/// recorded dropouts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RealisedState {
    /// The execution boundary, UTC millis: intervals that started before it
    /// are frozen; only intervals starting at or after it are re-optimized.
    pub executed_until_millis: i64,
    /// The per-asset initial conditions for the new solve.
    pub carried_assets: Vec<CarriedAssetState>,
    /// The measured records of executed intervals (the frozen values).
    pub realised_intervals: Vec<RealisedInterval>,
    /// The recorded opt-outs/dropouts (P3.11).
    pub dropouts: Vec<AssetDropout>,
}

/// The prior schedule revision a roll re-solves from (the revision being
/// superseded). Its intervals are the plan the realised values are checked
/// against — the past is never rewritten.
#[derive(Debug, Clone, PartialEq)]
pub struct PriorRevision {
    /// The schedule-chain identity (canonical UUID text; D-P3.22-1).
    pub schedule_id: String,
    /// The prior revision number (>= 1).
    pub revision: u64,
    /// The prior revision's §12.6 candidate ID (canonical UUID text).
    pub candidate_id: String,
    /// The start-inclusive prior horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// The end-exclusive prior horizon end, UTC millis.
    pub horizon_end_millis: i64,
    /// The interval length, millis (must divide the horizon exactly — the
    /// P3.03 grid rule).
    pub interval_millis: i64,
    /// The assets the prior schedule covers (non-empty, unique).
    pub asset_keys: Vec<String>,
    /// The prior plan's intervals (grid-aligned, sorted, unique).
    pub intervals: Vec<IntervalPlan>,
}

impl PriorRevision {
    /// Validates the prior-revision shape: horizon, grid, asset set, and
    /// interval alignment/coverage of the plan.
    pub fn validate(&self) -> Result<(), RollingHorizonError> {
        if self.schedule_id.trim().is_empty() {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "schedule_id" });
        }
        if self.candidate_id.trim().is_empty() {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "candidate_id" });
        }
        if self.revision == 0 {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "revision" });
        }
        if self.horizon_end_millis <= self.horizon_start_millis {
            return Err(RollingHorizonError::InvalidPriorRevision {
                field: "horizon_end_millis",
            });
        }
        if self.interval_millis <= 0
            || (self.horizon_end_millis - self.horizon_start_millis) % self.interval_millis != 0
        {
            return Err(RollingHorizonError::InvalidPriorRevision {
                field: "interval_millis",
            });
        }
        if self.asset_keys.is_empty() {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "asset_keys" });
        }
        let mut sorted_keys = self.asset_keys.clone();
        sorted_keys.sort();
        sorted_keys.dedup();
        if sorted_keys.len() != self.asset_keys.len() {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "asset_keys" });
        }
        let horizon_len = self.horizon_end_millis - self.horizon_start_millis;
        let expected_intervals = (horizon_len / self.interval_millis) as usize;
        if self.intervals.len() != expected_intervals {
            return Err(RollingHorizonError::InvalidPriorRevision { field: "intervals" });
        }
        let mut previous_start: Option<i64> = None;
        for interval in &self.intervals {
            if interval.interval_end_millis != interval.interval_start_millis + self.interval_millis
            {
                return Err(RollingHorizonError::InvalidPriorRevision {
                    field: "intervals[].interval_end_millis",
                });
            }
            if (interval.interval_start_millis - self.horizon_start_millis)
                % self.interval_millis
                != 0
                || interval.interval_start_millis < self.horizon_start_millis
                || interval.interval_start_millis >= self.horizon_end_millis
            {
                return Err(RollingHorizonError::MisalignedInterval {
                    interval_start_millis: interval.interval_start_millis,
                });
            }
            if let Some(previous) = previous_start {
                if interval.interval_start_millis <= previous {
                    return Err(RollingHorizonError::InvalidPriorRevision {
                        field: "intervals[]",
                    });
                }
            }
            previous_start = Some(interval.interval_start_millis);
            let mut seen: Vec<&str> = Vec::new();
            for setpoint in &interval.setpoints {
                if !self.asset_keys.contains(&setpoint.asset_key) {
                    return Err(RollingHorizonError::InvalidPriorRevision {
                        field: "intervals[].setpoints[].asset_key",
                    });
                }
                if !setpoint.p_mw.is_finite() {
                    return Err(RollingHorizonError::InvalidPriorRevision {
                        field: "intervals[].setpoints[].p_mw",
                    });
                }
                if seen.contains(&setpoint.asset_key.as_str()) {
                    return Err(RollingHorizonError::DuplicateAssetSetpoint {
                        interval_start_millis: interval.interval_start_millis,
                        asset_key: setpoint.asset_key.clone(),
                    });
                }
                seen.push(&setpoint.asset_key);
            }
        }
        Ok(())
    }
}

/// The roll request (ledger P3.24 step 1): the trigger (REQUIRED, recorded
/// on the new revision) plus the realised state carried forward.
#[derive(Debug, Clone, PartialEq)]
pub struct RollRequest {
    pub trigger: RollTrigger,
    /// The human-readable trigger detail (which event, which asset, which
    /// forecast version) recorded with the revision's audit trail.
    pub trigger_detail: String,
    pub realised: RealisedState,
}

/// One side of the supersede link (D-P3.24-4): the prior revision being
/// superseded and the new revision superseding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionIdentity {
    pub schedule_id: String,
    pub revision: u64,
}

// --- Errors --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RollingHorizonError {
    /// A carried SOC is outside [0,1] (canonical unit, §10.2) or not finite.
    SocOutOfRange {
        asset_key: String,
        value: f64,
    },
    /// The execution boundary lies outside the prior horizon.
    ExecutedUntilOutsideHorizon {
        executed_until_millis: i64,
        horizon_start_millis: i64,
        horizon_end_millis: i64,
    },
    /// The trigger does not match the execution boundary: `horizon_advance`
    /// requires the horizon consumed; every other trigger requires a
    /// mid-horizon boundary (decided once, D-P3.24-2).
    TriggerHorizonMismatch {
        trigger: &'static str,
        executed_until_millis: i64,
        horizon_end_millis: i64,
    },
    /// An interval is not aligned to the scheduling grid (§10.3).
    MisalignedInterval {
        interval_start_millis: i64,
    },
    /// An executed (frozen) interval has no realised record — the past
    /// cannot be frozen without its measured reality.
    MissingRealisedInterval {
        interval_start_millis: i64,
    },
    /// A frozen (interval, asset) pair has no realised value and no dropout
    /// covering it.
    MissingRealisedValue {
        interval_start_millis: i64,
        asset_key: String,
    },
    /// The revision counter overflowed.
    RevisionOverflow {
        schedule_id: String,
    },
    /// The prior-revision shape is invalid (`field` names the problem).
    InvalidPriorRevision {
        field: &'static str,
    },
    /// The projection dispatches an asset at or after its recorded opt-out
    /// (the P3.11 discipline; capacity is never silently retained).
    DispatchedAfterOptOut {
        asset_key: String,
        interval_start_millis: i64,
    },
    /// The projection is not a continuous grid-aligned schedule.
    ProjectionGap {
        detail: String,
    },
    /// Two setpoints for the same asset in one interval.
    DuplicateAssetSetpoint {
        interval_start_millis: i64,
        asset_key: String,
    },
    /// The P3.21 publication machinery rejected the assembly (wrapped, never
    /// restated).
    Publication {
        source: ScheduleResultError,
    },
    /// The publication's candidate does not match the plan's new revision.
    PublicationMismatch {
        expected: String,
        found: String,
    },
}

impl std::fmt::Display for RollingHorizonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RollingHorizonError::SocOutOfRange { asset_key, value } => write!(
                f,
                "asset '{asset_key}': realised SOC {value} is outside [0,1] (canonical \
                 fraction unit, §10.2)"
            ),
            RollingHorizonError::ExecutedUntilOutsideHorizon {
                executed_until_millis,
                horizon_start_millis,
                horizon_end_millis,
            } => write!(
                f,
                "the execution boundary {executed_until_millis} lies outside the prior \
                 horizon [{horizon_start_millis}, {horizon_end_millis})"
            ),
            RollingHorizonError::TriggerHorizonMismatch {
                trigger,
                executed_until_millis,
                horizon_end_millis,
            } => write!(
                f,
                "trigger '{trigger}' does not match the execution boundary \
                 {executed_until_millis} against the horizon end {horizon_end_millis} \
                 (D-P3.24-2: horizon_advance requires the horizon consumed; every other \
                 trigger requires a mid-horizon boundary)"
            ),
            RollingHorizonError::MisalignedInterval { interval_start_millis } => write!(
                f,
                "interval starting at {interval_start_millis} is not aligned to the \
                 scheduling grid (§10.3)"
            ),
            RollingHorizonError::MissingRealisedInterval { interval_start_millis } => write!(
                f,
                "the executed interval starting at {interval_start_millis} has no realised \
                 record; the past cannot be frozen without its measured reality"
            ),
            RollingHorizonError::MissingRealisedValue { interval_start_millis, asset_key } => {
                write!(
                    f,
                    "asset '{asset_key}' has no realised value for the frozen interval \
                     starting at {interval_start_millis} and no dropout covering it"
                )
            }
            RollingHorizonError::RevisionOverflow { schedule_id } => write!(
                f,
                "the revision counter of schedule {schedule_id} overflowed"
            ),
            RollingHorizonError::InvalidPriorRevision { field } => {
                write!(f, "the prior revision is invalid: '{field}' is missing or malformed")
            }
            RollingHorizonError::DispatchedAfterOptOut { asset_key, interval_start_millis } => {
                write!(
                    f,
                    "asset '{asset_key}' is dispatched for the interval starting at \
                     {interval_start_millis} at or after its opt-out instant (P3.11: \
                     capacity is never silently retained)"
                )
            }
            RollingHorizonError::ProjectionGap { detail } => {
                write!(f, "the rolling schedule projection is not continuous: {detail}")
            }
            RollingHorizonError::DuplicateAssetSetpoint { interval_start_millis, asset_key } => {
                write!(
                    f,
                    "asset '{asset_key}' has two setpoints in the interval starting at \
                     {interval_start_millis}"
                )
            }
            RollingHorizonError::Publication { source } => {
                write!(f, "the ScheduleProposed assembly failed: {source}")
            }
            RollingHorizonError::PublicationMismatch { expected, found } => write!(
                f,
                "the publication's candidate '{found}' does not match the roll plan's new \
                 revision candidate '{expected}'"
            ),
        }
    }
}

impl std::error::Error for RollingHorizonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RollingHorizonError::Publication { source } => Some(source),
            _ => None,
        }
    }
}

// --- The roll plan -------------------------------------------------------------

/// The plan for one rolling-horizon roll (ledger P3.24 steps 1–3): the
/// trigger, the supersede pair, the carried-forward initial conditions, the
/// FROZEN realised intervals, and the re-optimization window. The actual
/// re-solve of the window happens at the composition root through the
/// `Optimizer` port; the plan is the contract the solve must respect.
#[derive(Debug, Clone, PartialEq)]
pub struct RollPlan {
    /// The recorded trigger (REQUIRED per revision, D-P3.24-1).
    pub trigger: RollTrigger,
    /// The human-readable trigger detail.
    pub trigger_detail: String,
    /// The prior revision — superseded by the new one (§14.2; both sides of
    /// the bidirectional P3.22 link).
    pub superseded: RevisionIdentity,
    /// The new revision (prior + 1; the chain has no gaps).
    pub new_revision: RevisionIdentity,
    /// The new horizon (start-inclusive, end-exclusive). For a mid-horizon
    /// trigger this is the prior horizon unchanged; for `horizon_advance` it
    /// is the shifted window of the same length (D-P3.24-2).
    pub new_horizon_start_millis: i64,
    pub new_horizon_end_millis: i64,
    /// The interval length, millis (the prior grid configuration — the grid
    /// never changes mid-chain).
    pub interval_millis: i64,
    /// The execution boundary the split is taken at.
    pub executed_until_millis: i64,
    /// The carried-forward initial conditions (actual SOC per asset).
    pub initial_conditions: Vec<CarriedAssetState>,
    /// The FROZEN intervals with their REALISED values — immutable inputs
    /// of the new solve (D-P3.24-3). Never the prior plan's values.
    pub frozen_intervals: Vec<IntervalPlan>,
    /// The re-optimization window: [start, end) — only future intervals.
    pub reoptimize_start_millis: i64,
    pub reoptimize_end_millis: i64,
    /// The assets available to the re-optimized window (prior assets minus
    /// those dropped out at or before the window start).
    pub active_asset_keys: Vec<String>,
    /// The recorded dropouts carried into the solve (P3.11): an asset here
    /// leaves the feasible set from `from_millis` onward — the re-solve must
    /// honour the from-instant even when it lies inside the window.
    pub dropouts: Vec<AssetDropout>,
    /// The recorded notes (candidate status, frozen rule, dropped assets).
    pub notes: Vec<String>,
}

impl RollPlan {
    /// The new revision's roll trigger as the stored string (the value the
    /// composition root writes into the `roll_trigger` column; must equal
    /// one of `flexcore-storage::schedule_revision::SCHEDULE_ROLL_TRIGGERS`).
    pub fn trigger_storage_string(&self) -> &'static str {
        self.trigger.as_str()
    }
}

/// Plans one rolling-horizon roll (ledger P3.24 steps 1–3; pure).
///
/// Validation, in order: the prior shape; the execution boundary inside the
/// prior horizon; the trigger/boundary consistency (D-P3.24-2); SOC ranges;
/// grid alignment of every realised interval; total realised coverage of the
/// frozen set (every frozen (interval, asset) pair carries a realised value
/// unless the asset dropped out at or before the interval start).
pub fn plan_rolling_horizon_roll(
    prior: &PriorRevision,
    request: &RollRequest,
) -> Result<RollPlan, RollingHorizonError> {
    prior.validate()?;

    let executed_until = request.realised.executed_until_millis;
    if executed_until < prior.horizon_start_millis || executed_until > prior.horizon_end_millis {
        return Err(RollingHorizonError::ExecutedUntilOutsideHorizon {
            executed_until_millis: executed_until,
            horizon_start_millis: prior.horizon_start_millis,
            horizon_end_millis: prior.horizon_end_millis,
        });
    }

    // Trigger/boundary consistency (D-P3.24-2, decided once).
    match request.trigger {
        RollTrigger::HorizonAdvance => {
            if executed_until < prior.horizon_end_millis {
                return Err(RollingHorizonError::TriggerHorizonMismatch {
                    trigger: request.trigger.as_str(),
                    executed_until_millis: executed_until,
                    horizon_end_millis: prior.horizon_end_millis,
                });
            }
        }
        _ => {
            if executed_until >= prior.horizon_end_millis {
                return Err(RollingHorizonError::TriggerHorizonMismatch {
                    trigger: request.trigger.as_str(),
                    executed_until_millis: executed_until,
                    horizon_end_millis: prior.horizon_end_millis,
                });
            }
        }
    }

    // Carried SOC: a fraction in [0,1], finite (canonical unit, §10.2).
    for carried in &request.realised.carried_assets {
        if let Some(soc) = carried.realised_soc {
            if !soc.is_finite() || !(0.0..=1.0).contains(&soc) {
                return Err(RollingHorizonError::SocOutOfRange {
                    asset_key: carried.asset_key.clone(),
                    value: soc,
                });
            }
        }
    }

    // Realised intervals must sit on the prior grid.
    for realised in &request.realised.realised_intervals {
        if realised.interval_end_millis != realised.interval_start_millis + prior.interval_millis
            || (realised.interval_start_millis - prior.horizon_start_millis)
                % prior.interval_millis
                != 0
            || realised.interval_start_millis < prior.horizon_start_millis
            || realised.interval_start_millis >= prior.horizon_end_millis
        {
            return Err(RollingHorizonError::MisalignedInterval {
                interval_start_millis: realised.interval_start_millis,
            });
        }
    }

    // The split (D-P3.24-3): frozen = started before the boundary. On a
    // horizon advance there is NO overlap — the new solve covers the NEW
    // horizon, and the fully executed prior horizon is realised history
    // retained in the superseded revision rows (the P3.03 rollover rule);
    // the frozen set is empty and the whole new horizon is re-optimized.
    let frozen_starts: Vec<i64> = match request.trigger {
        RollTrigger::HorizonAdvance => Vec::new(),
        _ => prior
            .intervals
            .iter()
            .filter(|i| i.interval_start_millis < executed_until)
            .map(|i| i.interval_start_millis)
            .collect(),
    };

    // Realised coverage of the frozen set.
    let mut frozen_intervals = Vec::with_capacity(frozen_starts.len());
    for start in &frozen_starts {
        let realised = request
            .realised
            .realised_intervals
            .iter()
            .find(|r| r.interval_start_millis == *start)
            .ok_or(RollingHorizonError::MissingRealisedInterval {
                interval_start_millis: *start,
            })?;
        let mut setpoints = Vec::with_capacity(prior.asset_keys.len());
        for asset_key in &prior.asset_keys {
            let dropped_at_or_before = request.realised.dropouts.iter().any(|d| {
                &d.asset_key == asset_key && d.from_millis <= *start
            });
            if dropped_at_or_before {
                continue;
            }
            let value = realised
                .values
                .iter()
                .find(|v| &v.asset_key == asset_key)
                .ok_or(RollingHorizonError::MissingRealisedValue {
                    interval_start_millis: *start,
                    asset_key: asset_key.clone(),
                })?;
            setpoints.push(AssetSetpoint {
                asset_key: asset_key.clone(),
                p_mw: value.realised_p_mw,
            });
        }
        frozen_intervals.push(IntervalPlan {
            interval_start_millis: realised.interval_start_millis,
            interval_end_millis: realised.interval_end_millis,
            setpoints,
        });
    }

    // The new horizon (D-P3.24-2): mid-horizon triggers keep the prior
    // horizon; horizon_advance shifts the same-length window to the
    // boundary (which must be grid-aligned).
    let (new_horizon_start, new_horizon_end) = match request.trigger {
        RollTrigger::HorizonAdvance => {
            if (executed_until - prior.horizon_start_millis) % prior.interval_millis != 0 {
                return Err(RollingHorizonError::MisalignedInterval {
                    interval_start_millis: executed_until,
                });
            }
            (
                executed_until,
                executed_until + (prior.horizon_end_millis - prior.horizon_start_millis),
            )
        }
        _ => (prior.horizon_start_millis, prior.horizon_end_millis),
    };

    // The re-optimization window: strictly future (D-P3.24-3).
    let reoptimize_start = new_horizon_start.max(executed_until);
    let reoptimize_end = new_horizon_end;

    // Available assets: prior assets minus those dropped out at or before
    // the window start (P3.11 — capacity is never silently retained). A
    // dropout INSIDE the window stays recorded: the re-solve must exclude
    // the asset from its from-instant onward.
    let active_asset_keys: Vec<String> = prior
        .asset_keys
        .iter()
        .filter(|key| {
            !request
                .realised
                .dropouts
                .iter()
                .any(|d| &d.asset_key == *key && d.from_millis <= reoptimize_start)
        })
        .cloned()
        .collect();

    let new_revision_number = prior
        .revision
        .checked_add(1)
        .ok_or_else(|| RollingHorizonError::RevisionOverflow {
            schedule_id: prior.schedule_id.clone(),
        })?;

    let mut notes = vec![
        "the new revision is a CANDIDATE — never feasible, approved, or dispatchable \
         (Phase 4 gates)"
            .to_string(),
        FROZEN_INTERVAL_STATEMENT.to_string(),
        CARRY_FORWARD_STATEMENT.to_string(),
        format!(
            "trigger '{}': {}",
            request.trigger.as_str(),
            request.trigger_detail
        ),
    ];
    for dropout in &request.realised.dropouts {
        notes.push(format!(
            "dropout: asset '{}' leaves the feasible set from {} onward (P3.11)",
            dropout.asset_key, dropout.from_millis
        ));
    }
    if matches!(request.trigger, RollTrigger::HorizonAdvance) {
        notes.push(
            "horizon advance: the fully executed prior horizon is realised history \
             retained in the superseded revision rows (the P3.03 rollover rule — never \
             a silent overwrite); the whole new horizon is re-optimized"
                .to_string(),
        );
    }

    Ok(RollPlan {
        trigger: request.trigger,
        trigger_detail: request.trigger_detail.clone(),
        superseded: RevisionIdentity {
            schedule_id: prior.schedule_id.clone(),
            revision: prior.revision,
        },
        new_revision: RevisionIdentity {
            schedule_id: prior.schedule_id.clone(),
            revision: new_revision_number,
        },
        new_horizon_start_millis: new_horizon_start,
        new_horizon_end_millis: new_horizon_end,
        interval_millis: prior.interval_millis,
        executed_until_millis: executed_until,
        initial_conditions: request.realised.carried_assets.clone(),
        frozen_intervals,
        reoptimize_start_millis: reoptimize_start,
        reoptimize_end_millis: reoptimize_end,
        active_asset_keys,
        dropouts: request.realised.dropouts.clone(),
        notes,
    })
}

// --- The schedule projection ---------------------------------------------------

/// The rolling schedule projection (ledger P3.24 step 3): the frozen
/// realised intervals plus the re-optimized future intervals, pasted into
/// ONE grid-aligned schedule. This is a CANDIDATE schedule — the input to
/// the P3.21 record assembly and the P3.22 revision insert, never a
/// feasibility verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct RollingScheduleProjection {
    /// The new revision the projection belongs to.
    pub revision: RevisionIdentity,
    /// The projection intervals (sorted, continuous, grid-aligned; the
    /// frozen ones carry their realised values verbatim).
    pub intervals: Vec<IntervalPlan>,
    /// The carried-forward initial conditions the projection respects.
    pub initial_conditions: Vec<CarriedAssetState>,
}

/// Pastes the frozen realised intervals and the re-optimized future intervals
/// into the projection (ledger P3.24 step 3), enforcing:
///
/// - every future interval is aligned to the new grid and inside the
///   re-optimization window (the past is never re-solved — a future interval
///   starting before the boundary is a `ProjectionGap`);
/// - the projection is continuous over the window (no gaps, no overlaps);
/// - no asset is dispatched at or after its recorded opt-out instant
///   (`DispatchedAfterOptOut`, the P3.11 discipline);
/// - the frozen intervals carry their realised values verbatim (the test
///   gate: the past is unchanged).
pub fn project_rolling_schedule(
    plan: &RollPlan,
    mut future_intervals: Vec<IntervalPlan>,
) -> Result<RollingScheduleProjection, RollingHorizonError> {
    // Grid alignment of the future intervals against the new horizon.
    for interval in &future_intervals {
        if interval.interval_end_millis != interval.interval_start_millis + plan.interval_millis
        {
            return Err(RollingHorizonError::ProjectionGap {
                detail: format!(
                    "interval starting at {} has a non-grid length",
                    interval.interval_start_millis
                ),
            });
        }
        if interval.interval_start_millis < plan.reoptimize_start_millis {
            return Err(RollingHorizonError::ProjectionGap {
                detail: format!(
                    "interval starting at {} lies before the re-optimization boundary {}; \
                     the past is never re-solved",
                    interval.interval_start_millis, plan.reoptimize_start_millis
                ),
            });
        }
        if interval.interval_start_millis >= plan.reoptimize_end_millis {
            return Err(RollingHorizonError::ProjectionGap {
                detail: format!(
                    "interval starting at {} lies beyond the new horizon end {}",
                    interval.interval_start_millis, plan.reoptimize_end_millis
                ),
            });
        }
        if (interval.interval_start_millis - plan.new_horizon_start_millis)
            % plan.interval_millis
            != 0
        {
            return Err(RollingHorizonError::MisalignedInterval {
                interval_start_millis: interval.interval_start_millis,
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for setpoint in &interval.setpoints {
            if seen.contains(&setpoint.asset_key.as_str()) {
                return Err(RollingHorizonError::DuplicateAssetSetpoint {
                    interval_start_millis: interval.interval_start_millis,
                    asset_key: setpoint.asset_key.clone(),
                });
            }
            seen.push(&setpoint.asset_key);
            // The opt-out discipline (P3.11): no dispatch at or after the
            // opt-out instant.
            if let Some(dropout) = plan
                .dropouts
                .iter()
                .find(|d| d.asset_key == setpoint.asset_key)
            {
                if interval.interval_start_millis >= dropout.from_millis {
                    return Err(RollingHorizonError::DispatchedAfterOptOut {
                        asset_key: setpoint.asset_key.clone(),
                        interval_start_millis: interval.interval_start_millis,
                    });
                }
            }
        }
    }
    future_intervals.sort_by_key(|i| i.interval_start_millis);

    // Continuity: the future window is covered exactly once, starting at the
    // re-optimization boundary.
    let mut cursor = plan.reoptimize_start_millis;
    for interval in &future_intervals {
        if interval.interval_start_millis != cursor {
            return Err(RollingHorizonError::ProjectionGap {
                detail: format!(
                    "expected the next future interval at {cursor}, found {}",
                    interval.interval_start_millis
                ),
            });
        }
        cursor = interval.interval_end_millis;
    }
    if cursor != plan.reoptimize_end_millis {
        return Err(RollingHorizonError::ProjectionGap {
            detail: format!(
                "the future intervals end at {cursor} but the re-optimization window ends \
                 at {}",
                plan.reoptimize_end_millis
            ),
        });
    }

    let mut intervals = plan.frozen_intervals.clone();
    intervals.extend(future_intervals);
    intervals.sort_by_key(|i| i.interval_start_millis);

    Ok(RollingScheduleProjection {
        revision: plan.new_revision.clone(),
        intervals,
        initial_conditions: plan.initial_conditions.clone(),
    })
}

// --- ScheduleProposed per new revision (D-P3.24-5) ------------------------------

/// The inputs for the per-revision `ScheduleProposed` assembly (ledger
/// P3.24 step 4). The P3.21 record and persistence proof are REUSED — the
/// new revision's solve went through the P3.21 record assembly and was
/// persisted before any downstream use (§16.1).
pub struct RollingHorizonPublicationInput<'a> {
    /// The new revision's assembled §12.6 schedule-result record (P3.21).
    pub record: &'a ScheduleResultRecord,
    /// The persistence proof for that record (P3.21).
    pub persisted: &'a PersistedResultRecord,
    /// The roll plan that produced the new revision (its trigger and
    /// supersede pair are recorded on the revision row, not in the event —
    /// see [`SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE`]).
    pub plan: &'a RollPlan,
    /// The candidate id the new revision was persisted with — verified
    /// against both the record and the plan's new revision.
    pub new_candidate_id: String,
    pub correlation_id: FlexUuid,
    pub event_time: EventTime,
}

/// Assembles the `ScheduleProposed` outbox entry for one rolled revision
/// (ledger P3.24 step 4; pure assembly over the FROZEN payload — the outbox
/// wiring is P3.28's, through `event_assembly`). Delegates to the P3.21
/// `schedule_proposed_publication` (reused, never restated) after verifying
/// the candidate identity agrees across the record, the persistence proof
/// (verified there), and the roll plan (whose supersede pair must be a
/// gapless one-step link — the shape the P3.22 store enforces at rest).
pub fn rolling_horizon_schedule_proposed_publication(
    input: RollingHorizonPublicationInput<'_>,
) -> Result<ScheduleProposedPublication, RollingHorizonError> {
    if input.new_candidate_id.trim().is_empty() {
        return Err(RollingHorizonError::PublicationMismatch {
            expected: "<non-empty candidate id>".to_string(),
            found: String::new(),
        });
    }
    let record_reference = input.record.record_reference();
    if input.new_candidate_id != record_reference {
        return Err(RollingHorizonError::PublicationMismatch {
            expected: record_reference,
            found: input.new_candidate_id,
        });
    }
    // The plan's supersede pair must be the gapless one-step link the P3.22
    // store will persist (the composition root inserts the revision row with
    // this candidate id and the plan's trigger in the SAME unit of work as
    // the event — §15.5).
    let expected_new = input
        .plan
        .superseded
        .revision
        .checked_add(1)
        .ok_or(RollingHorizonError::RevisionOverflow {
            schedule_id: input.plan.superseded.schedule_id.clone(),
        })?;
    if input.plan.new_revision.revision != expected_new {
        return Err(RollingHorizonError::PublicationMismatch {
            expected: format!("revision {expected_new}"),
            found: format!("revision {}", input.plan.new_revision.revision),
        });
    }
    schedule_proposed_publication(
        input.record,
        input.persisted,
        input.correlation_id,
        input.event_time,
    )
    .map_err(|source| RollingHorizonError::Publication { source })
}
