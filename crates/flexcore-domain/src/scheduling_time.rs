// P3.03 — Scheduling time model and horizon (pure rules, no I/O).
//
// Implements ledger P3.03 ("Time model and horizon definition", report §10.3,
// §11.1, §12.1). Decision record:
// `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.
//
// THE TIME MODEL, DECIDED ONCE:
//
// - **Horizon**: 24 hours is the DEFAULT per report §24 step 3 ("calculate
//   each resource's available flexibility over a defined 24-hour horizon").
//   It is a configuration value, not a hard requirement.
// - **Interval length**: a configuration value. The default of 15 minutes is
//   the phase ledger's PROPOSAL (ledger open question 1: "proposed 15-minute
//   intervals over 24 hours — confirm with product; it is configuration, not
//   a report requirement"). It is recorded as a default here, never baked
//   into the grid semantics.
// - **UTC only internally** (§10.3): the grid is built on the contracts UTC
//   timestamp types; local time is display-only. A local calendar day is not
//   assumed to equal 24 hours (DST rule, §10.3) — and is irrelevant here,
//   because the grid never does local-time arithmetic.
// - **Interval semantics: start-inclusive, end-exclusive.** An instant
//   exactly on an interval boundary belongs to the LATER interval. A
//   timestamp exactly at the horizon end belongs to no interval of the
//   horizon. All interval arithmetic is pure integer-millisecond arithmetic
//   over the contract timestamp types (the domain crate takes no `time`
//   dependency; timestamps are rebuilt through the validating contract
//   constructors, so UTC and range invariants are re-checked on every
//   construction).
// - The horizon must be an exact integer multiple of the interval length, so
//   that every scheduling grid over the same configuration has identical
//   interval boundaries (acceptance criterion: two independently built
//   fixtures on the same horizon produce identical interval boundaries).
//
// THE AGGREGATION RULE, DECIDED ONCE (quality survival, §10.4 / P0.08):
//
//   Sub-interval telemetry is aggregated onto the scheduling grid per
//   scheduling interval. Each value carries exactly one of the six canonical
//   quality states. The bucket's quality is decided by the recorded
//   precedence (highest wins):
//
//     MISSING > BAD > STALE > SUSPECT > ESTIMATED > GOOD
//
//   | Bucket contains | Result | Reading |
//   |---|---|---|
//   | any MISSING | MISSING | the bucket is incomplete; it cannot be certified at all (recorded choice: incompleteness outranks known-invalid data) |
//   | else any BAD | BAD | a hard-invalid value poisons the aggregate (a BAD value is never silently treated as GOOD, §10.4) |
//   | else any STALE | STALE | freshness failed for part of the bucket |
//   | else any SUSPECT | SUSPECT | a plausibility/range/consistency check failed on part of the bucket |
//   | else any ESTIMATED | ESTIMATED | derived values stay derived; ESTIMATED never upgrades to GOOD |
//   | else (all GOOD) | GOOD | GOOD survives ONLY if every input is GOOD |
//
//   An empty bucket is MISSING (no value was received for the expected
//   interval, §10.4). Non-silencing by construction: the aggregation can
//   only ever return a non-GOOD state unless every input is GOOD — the same
//   discipline as the P0.08 source-state mapping. Values aggregate as an
//   arithmetic mean (power-like quantities over a scheduling interval) or a
//   sum (energy-like quantities), with the assumption recorded that
//   sub-interval inputs are uniform-duration; a caller with non-uniform
//   sub-intervals passes duration-weighted values.
//
// THE HORIZON-ROLLOVER RULE, DECIDED ONCE:
//
//   A new horizon SUPERSEDES the previous schedule revision — it never
//   silently overwrites or deletes it (cf. §14.2 SUPERSEDED, retained for
//   audit). [`plan_horizon_rollover`] records the outcome as a pure value:
//   the prior revision reference travels unchanged into
//   [`RolloverOutcome::Supersedes`], and the new revision number is
//   `prior + 1`. The schedule store that persists both revisions lands in
//   P3.22; this module fixes the rule and the record shape only.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no persistence, no device-write path.

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil};

use crate::ids::ScheduleId;

/// Default horizon length: 24 hours, per report §24 step 3. A configuration
/// default, not a hard requirement.
pub const DEFAULT_HORIZON_MILLIS: i64 = 24 * 60 * 60 * 1000;

/// Default interval length: 15 minutes. The phase ledger's proposal (open
/// question 1) — a configuration default recorded for the synthetic baseline,
/// explicitly not a report requirement.
pub const DEFAULT_INTERVAL_MILLIS: i64 = 15 * 60 * 1000;

/// Errors returned by the P3.03 scheduling time model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingTimeError {
    /// The interval length must be a positive number of milliseconds.
    NonPositiveInterval,
    /// The horizon must be a positive number of milliseconds.
    NonPositiveHorizon,
    /// The horizon must be an exact integer multiple of the interval length,
    /// so that every grid over the same configuration has identical interval
    /// boundaries.
    HorizonNotMultipleOfInterval,
    /// The interval index was outside `[0, interval_count)`.
    IntervalIndexOutOfRange {
        /// The offending index.
        index: usize,
        /// The number of intervals in the horizon.
        interval_count: usize,
    },
    /// A timestamp could not be rebuilt through the validating contract
    /// constructor (millisecond value outside the representable range).
    TimestampOutOfRange,
    /// A telemetry value was NaN or infinite; aggregated quantities must be
    /// finite.
    NonFiniteTelemetryValue,
    /// A telemetry bucket contained no values; a caller must supply at least
    /// the gap markers (P2.04 derived records) rather than an empty bucket.
    EmptyTelemetryBucket,
    /// The rollover was requested for a schedule ID different from the prior
    /// revision's schedule ID. Rollover applies per schedule.
    ForeignScheduleId,
    /// The revision number would overflow; no next revision exists.
    RevisionOverflow,
}

impl std::fmt::Display for SchedulingTimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchedulingTimeError::NonPositiveInterval => {
                write!(f, "the scheduling interval length must be positive")
            }
            SchedulingTimeError::NonPositiveHorizon => {
                write!(f, "the scheduling horizon must be positive")
            }
            SchedulingTimeError::HorizonNotMultipleOfInterval => write!(
                f,
                "the scheduling horizon must be an exact integer multiple of the interval length"
            ),
            SchedulingTimeError::IntervalIndexOutOfRange { index, interval_count } => write!(
                f,
                "interval index {index} is outside the horizon (interval_count = {interval_count})"
            ),
            SchedulingTimeError::TimestampOutOfRange => {
                write!(f, "timestamp value is outside the representable range")
            }
            SchedulingTimeError::NonFiniteTelemetryValue => {
                write!(f, "telemetry values must be finite (no NaN or infinity)")
            }
            SchedulingTimeError::EmptyTelemetryBucket => {
                write!(f, "a telemetry bucket must contain at least one value or gap marker")
            }
            SchedulingTimeError::ForeignScheduleId => {
                write!(f, "rollover applies per schedule: the prior revision belongs to a different schedule ID")
            }
            SchedulingTimeError::RevisionOverflow => {
                write!(f, "schedule revision number overflow; no next revision exists")
            }
        }
    }
}

impl std::error::Error for SchedulingTimeError {}

// --- Configuration ----------------------------------------------------------

/// The scheduling grid configuration: horizon and interval length. Both are
/// configuration values (P3.03 step 1); the defaults are the recorded
/// 24-hour horizon over 15-minute intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingGridConfig {
    horizon_millis: i64,
    interval_millis: i64,
}

impl SchedulingGridConfig {
    /// Constructs and validates the configuration: positive horizon and
    /// interval, horizon an exact integer multiple of the interval.
    pub fn new(horizon_millis: i64, interval_millis: i64) -> Result<Self, SchedulingTimeError> {
        if interval_millis <= 0 {
            return Err(SchedulingTimeError::NonPositiveInterval);
        }
        if horizon_millis <= 0 {
            return Err(SchedulingTimeError::NonPositiveHorizon);
        }
        if horizon_millis % interval_millis != 0 {
            return Err(SchedulingTimeError::HorizonNotMultipleOfInterval);
        }
        Ok(Self {
            horizon_millis,
            interval_millis,
        })
    }

    /// The configured horizon length in milliseconds.
    pub const fn horizon_millis(&self) -> i64 {
        self.horizon_millis
    }

    /// The configured interval length in milliseconds.
    pub const fn interval_millis(&self) -> i64 {
        self.interval_millis
    }

    /// The number of intervals the horizon covers.
    pub const fn interval_count(&self) -> usize {
        (self.horizon_millis / self.interval_millis) as usize
    }
}

impl Default for SchedulingGridConfig {
    /// The recorded defaults: 24-hour horizon (§24 step 3) over 15-minute
    /// intervals (phase ledger proposal, open question 1) — configuration
    /// values, not hard requirements.
    fn default() -> Self {
        Self {
            horizon_millis: DEFAULT_HORIZON_MILLIS,
            interval_millis: DEFAULT_INTERVAL_MILLIS,
        }
    }
}

// --- The grid ---------------------------------------------------------------

/// A scheduling grid: a UTC horizon start plus a validated configuration.
/// Every grid over the same start and configuration has identical interval
/// boundaries (acceptance criterion; the boundaries are pure arithmetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingGrid {
    start: ValidFrom,
    config: SchedulingGridConfig,
}

impl SchedulingGrid {
    /// Constructs the grid from a UTC horizon start and a configuration.
    pub fn new(start: ValidFrom, config: SchedulingGridConfig) -> Self {
        Self { start, config }
    }

    /// The start-inclusive start of the horizon (interval 0 start).
    pub const fn start(&self) -> ValidFrom {
        self.start
    }

    /// The grid configuration.
    pub const fn config(&self) -> &SchedulingGridConfig {
        &self.config
    }

    /// The number of intervals in the horizon.
    pub const fn interval_count(&self) -> usize {
        self.config.interval_count()
    }

    /// The start-inclusive / end-exclusive bounds of interval `index`.
    ///
    /// The end of interval `index` equals the start of interval `index + 1`;
    /// an instant on that boundary belongs to interval `index + 1`
    /// (see [`SchedulingGrid::interval_index_at`]).
    pub fn interval_bounds(
        &self,
        index: usize,
    ) -> Result<(ValidFrom, ValidUntil), SchedulingTimeError> {
        if index >= self.interval_count() {
            return Err(SchedulingTimeError::IntervalIndexOutOfRange {
                index,
                interval_count: self.interval_count(),
            });
        }
        let start_millis = self.start.unix_millis() + i128::from(self.config.interval_millis)
            * i128::from(index as u64);
        let end_millis = start_millis + i128::from(self.config.interval_millis);
        let start_millis =
            i64::try_from(start_millis).map_err(|_| SchedulingTimeError::TimestampOutOfRange)?;
        let end_millis =
            i64::try_from(end_millis).map_err(|_| SchedulingTimeError::TimestampOutOfRange)?;
        // The contract constructors re-validate UTC and range; a range
        // failure maps to the named scheduling-time error.
        let start = ValidFrom::from_unix_millis(start_millis)
            .map_err(|_| SchedulingTimeError::TimestampOutOfRange)?;
        let end = ValidUntil::from_unix_millis(end_millis)
            .map_err(|_| SchedulingTimeError::TimestampOutOfRange)?;
        Ok((start, end))
    }

    /// The interval index an instant belongs to, under start-inclusive /
    /// end-exclusive semantics: `t ∈ [start_i, start_i + interval)`.
    ///
    /// An instant exactly on a boundary belongs to the LATER interval; an
    /// instant exactly at the horizon end belongs to no interval of this
    /// horizon; an instant before the horizon start belongs to no interval.
    pub fn interval_index_at(&self, t: EventTime) -> Option<usize> {
        let start_millis = self.start.unix_millis();
        let t_millis = t.unix_millis();
        if t_millis < start_millis {
            return None;
        }
        let horizon_end = start_millis + i128::from(self.config.horizon_millis);
        if t_millis >= horizon_end {
            return None;
        }
        let offset = t_millis - start_millis;
        Some((offset / i128::from(self.config.interval_millis)) as usize)
    }

    /// Whether the instant falls inside this horizon (start-inclusive,
    /// horizon-end-exclusive).
    pub fn contains(&self, t: EventTime) -> bool {
        self.interval_index_at(t).is_some()
    }

    /// The start-inclusive start of every interval, in order. Two grids built
    /// independently over the same start and configuration produce identical
    /// vectors (acceptance criterion).
    pub fn interval_starts(&self) -> Vec<ValidFrom> {
        (0..self.interval_count())
            .map(|i| self.interval_bounds(i).map(|(s, _)| s).expect("in-range index"))
            .collect()
    }

    /// The exclusive end of the horizon: the first instant that belongs to no
    /// interval of this grid.
    pub fn horizon_end(&self) -> ValidUntil {
        let end_millis =
            self.start.unix_millis() + i128::from(self.config.horizon_millis);
        ValidUntil::from_unix_millis(i64::try_from(end_millis).expect("grid timestamps stay in range when the start and configuration are in range"))
            .expect("grid timestamps stay in range when the start and configuration are in range")
    }
}

// --- Quality survival on aggregation -----------------------------------------

/// Aggregates the quality states of the sub-interval values inside one
/// scheduling-interval bucket, per the recorded precedence:
///
/// ```text
/// MISSING > BAD > STALE > SUSPECT > ESTIMATED > GOOD
/// ```
///
/// GOOD survives only if every input is GOOD; an empty bucket is MISSING.
/// Non-silencing by construction: no path returns GOOD unless all inputs are
/// GOOD, and no path drops a quality flag (§10.4, P0.08).
pub fn aggregate_bucket_quality(qualities: &[TelemetryQuality]) -> TelemetryQuality {
    if qualities.is_empty() {
        return TelemetryQuality::Missing;
    }
    let contains = |target: TelemetryQuality| {
        qualities.iter().any(|q| *q == target)
    };
    if contains(TelemetryQuality::Missing) {
        TelemetryQuality::Missing
    } else if contains(TelemetryQuality::Bad) {
        TelemetryQuality::Bad
    } else if contains(TelemetryQuality::Stale) {
        TelemetryQuality::Stale
    } else if contains(TelemetryQuality::Suspect) {
        TelemetryQuality::Suspect
    } else if contains(TelemetryQuality::Estimated) {
        TelemetryQuality::Estimated
    } else {
        TelemetryQuality::Good
    }
}

/// How numeric sub-interval values aggregate onto the scheduling grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueAggregation {
    /// Arithmetic mean over the bucket (power-like quantities: the average
    /// power of the scheduling interval).
    Mean,
    /// Sum over the bucket (energy-like quantities: the energy accumulated
    /// within the scheduling interval).
    Sum,
}

/// One aggregated scheduling-grid value with its surviving quality state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AggregatedValue {
    /// The aggregated value in the canonical unit of the quantity.
    pub value: f64,
    /// The bucket quality decided by the recorded precedence (see
    /// [`aggregate_bucket_quality`]).
    pub quality: TelemetryQuality,
}

/// Aggregates sub-interval telemetry values onto one scheduling-grid bucket:
/// the numeric values aggregate per [`ValueAggregation`], the quality states
/// per the recorded precedence.
///
/// Recorded assumption: sub-interval inputs are uniform-duration; a caller
/// with non-uniform sub-intervals passes duration-weighted values.
pub fn aggregate_subinterval_values(
    points: &[(f64, TelemetryQuality)],
    rule: ValueAggregation,
) -> Result<AggregatedValue, SchedulingTimeError> {
    if points.is_empty() {
        return Err(SchedulingTimeError::EmptyTelemetryBucket);
    }
    let mut qualities = Vec::with_capacity(points.len());
    for (value, quality) in points {
        if !value.is_finite() {
            return Err(SchedulingTimeError::NonFiniteTelemetryValue);
        }
        qualities.push(*quality);
    }
    let value = match rule {
        ValueAggregation::Mean => {
            let total: f64 = points.iter().map(|(v, _)| v).sum();
            total / points.len() as f64
        }
        ValueAggregation::Sum => points.iter().map(|(v, _)| v).sum(),
    };
    Ok(AggregatedValue {
        value,
        quality: aggregate_bucket_quality(&qualities),
    })
}

// --- Horizon rollover -------------------------------------------------------

/// A reference to one schedule revision (the schedule store lands in P3.22;
/// this record shape is fixed here by the rollover rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleRevisionRef {
    /// The schedule the revision belongs to.
    pub schedule_id: ScheduleId,
    /// The revision number (monotonically increasing per schedule; the first
    /// revision is 1).
    pub revision: u64,
}

/// The outcome of a horizon rollover (P3.03 step 3, §14.2 supersede
/// discipline): a new horizon supersedes the previous schedule revision and
/// never silently overwrites or deletes it. There is deliberately **no
/// overwrite variant**: the prior revision reference travels unchanged into
/// the outcome, retained for audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloverOutcome {
    /// The first horizon for this schedule: no prior revision exists to
    /// supersede.
    Initial {
        /// The reference of the new (first) revision.
        new_revision: ScheduleRevisionRef,
    },
    /// A prior revision exists and is superseded by the new revision. The
    /// superseded reference is carried unchanged and retained for audit;
    /// nothing is mutated or deleted (§14.2: SUPERSEDED, distinct from
    /// CANCELLED).
    Supersedes {
        /// The superseded prior revision, unchanged.
        superseded: ScheduleRevisionRef,
        /// The reference of the new revision (`prior.revision + 1`).
        new_revision: ScheduleRevisionRef,
    },
}

/// Plans a horizon rollover for a schedule: the pure rule behind P3.03 step
/// 3. The store (P3.22) persists BOTH the prior and the new revision.
///
/// - No prior revision → [`RolloverOutcome::Initial`] with revision 1.
/// - Prior revision on the same schedule →
///   [`RolloverOutcome::Supersedes`] carrying the prior reference unchanged
///   and revision `prior + 1`.
/// - Prior revision on a different schedule → error: rollover applies per
///   schedule.
pub fn plan_horizon_rollover(
    prior: Option<ScheduleRevisionRef>,
    schedule_id: ScheduleId,
) -> Result<RolloverOutcome, SchedulingTimeError> {
    match prior {
        None => Ok(RolloverOutcome::Initial {
            new_revision: ScheduleRevisionRef {
                schedule_id,
                revision: 1,
            },
        }),
        Some(prior) => {
            if prior.schedule_id != schedule_id {
                return Err(SchedulingTimeError::ForeignScheduleId);
            }
            let next = prior
                .revision
                .checked_add(1)
                .ok_or(SchedulingTimeError::RevisionOverflow)?;
            Ok(RolloverOutcome::Supersedes {
                superseded: prior,
                new_revision: ScheduleRevisionRef {
                    schedule_id,
                    revision: next,
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const START_MILLIS: i64 = 1_700_000_000_000;

    fn start() -> ValidFrom {
        ValidFrom::from_unix_millis(START_MILLIS).unwrap()
    }

    fn instant(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).unwrap()
    }

    fn default_grid() -> SchedulingGrid {
        SchedulingGrid::new(start(), SchedulingGridConfig::default())
    }

    // --- configuration --------------------------------------------------

    #[test]
    fn default_config_is_24h_over_15min_96_intervals() {
        // Recorded defaults: configuration values, not hard-coded semantics
        // (the ledger proposal, open question 1).
        let config = SchedulingGridConfig::default();
        assert_eq!(config.horizon_millis(), 24 * 60 * 60 * 1000);
        assert_eq!(config.interval_millis(), 15 * 60 * 1000);
        assert_eq!(config.interval_count(), 96);
    }

    #[test]
    fn config_validates_horizon_and_interval() {
        assert_eq!(
            SchedulingGridConfig::new(0, 900_000),
            Err(SchedulingTimeError::NonPositiveHorizon)
        );
        assert_eq!(
            SchedulingGridConfig::new(3_600_000, 0),
            Err(SchedulingTimeError::NonPositiveInterval)
        );
        assert_eq!(
            SchedulingGridConfig::new(3_600_000, -1),
            Err(SchedulingTimeError::NonPositiveInterval)
        );
        assert_eq!(
            SchedulingGridConfig::new(3_600_000, 900_001),
            Err(SchedulingTimeError::HorizonNotMultipleOfInterval)
        );
        // A non-default valid configuration is accepted (interval length is
        // configuration, P3.03 step 1).
        assert_eq!(
            SchedulingGridConfig::new(3_600_000, 600_000).map(|c| c.interval_count()),
            Ok(6)
        );
    }

    // --- identical boundaries from independent fixtures -------------------

    #[test]
    fn two_independently_built_fixtures_produce_identical_boundaries() {
        // Fixture A: built through the default configuration.
        let a = default_grid();
        // Fixture B: built with an explicitly constructed (equal)
        // configuration, independent construction path.
        let b = SchedulingGrid::new(
            ValidFrom::from_unix_millis(START_MILLIS).unwrap(),
            SchedulingGridConfig::new(24 * 60 * 60 * 1000, 15 * 60 * 1000).unwrap(),
        );
        assert_eq!(a.interval_starts(), b.interval_starts());
        assert_eq!(a.interval_count(), b.interval_count());
        for i in 0..a.interval_count() {
            assert_eq!(a.interval_bounds(i), b.interval_bounds(i));
        }
    }

    #[test]
    fn interval_starts_are_consecutive_and_tile_the_horizon() {
        let grid = default_grid();
        let starts = grid.interval_starts();
        assert_eq!(starts.len(), 96);
        assert_eq!(starts[0].unix_millis(), i128::from(START_MILLIS));
        for i in 1..starts.len() {
            assert_eq!(
                starts[i].unix_millis() - starts[i - 1].unix_millis(),
                i128::from(15 * 60 * 1000)
            );
        }
        // The last interval ends exactly at the horizon end.
        let (_, last_end) = grid.interval_bounds(95).unwrap();
        assert_eq!(last_end.unix_millis(), grid.horizon_end().unix_millis());
    }

    // --- boundary timestamps ----------------------------------------------

    #[test]
    fn interval_bounds_are_start_inclusive_end_exclusive() {
        let grid = SchedulingGrid::new(start(), SchedulingGridConfig::new(3_600_000, 900_000).unwrap());
        let (s0, e0) = grid.interval_bounds(0).unwrap();
        assert_eq!(s0.unix_millis(), i128::from(START_MILLIS));
        assert_eq!(e0.unix_millis(), i128::from(START_MILLIS + 900_000));
        // End-exclusive: interval 1 starts exactly where interval 0 ends.
        let (s1, _) = grid.interval_bounds(1).unwrap();
        assert_eq!(e0.unix_millis(), s1.unix_millis());
        // The last valid index is interval_count - 1; anything beyond is an
        // out-of-range error.
        assert!(grid.interval_bounds(3).is_ok());
        assert_eq!(
            grid.interval_bounds(4),
            Err(SchedulingTimeError::IntervalIndexOutOfRange { index: 4, interval_count: 4 })
        );
        assert_eq!(
            grid.interval_bounds(5),
            Err(SchedulingTimeError::IntervalIndexOutOfRange { index: 5, interval_count: 4 })
        );
    }

    #[test]
    fn boundary_instants_belong_to_the_later_interval() {
        let grid = SchedulingGrid::new(start(), SchedulingGridConfig::new(3_600_000, 900_000).unwrap());
        // Start-inclusive: the horizon start is interval 0.
        assert_eq!(grid.interval_index_at(instant(START_MILLIS)), Some(0));
        // End-exclusive: an instant exactly on the 0/1 boundary is interval 1.
        assert_eq!(
            grid.interval_index_at(instant(START_MILLIS + 900_000)),
            Some(1)
        );
        // One millisecond before the boundary is still interval 0.
        assert_eq!(
            grid.interval_index_at(instant(START_MILLIS + 900_000 - 1)),
            Some(0)
        );
        // Exactly at the horizon end: no interval of this horizon.
        assert_eq!(
            grid.interval_index_at(instant(START_MILLIS + 3_600_000)),
            None
        );
        // One millisecond before the horizon end: the last interval.
        assert_eq!(
            grid.interval_index_at(instant(START_MILLIS + 3_600_000 - 1)),
            Some(3)
        );
        // Before the horizon start: no interval.
        assert_eq!(grid.interval_index_at(instant(START_MILLIS - 1)), None);
        assert!(!grid.contains(instant(START_MILLIS - 1)));
        assert!(grid.contains(instant(START_MILLIS)));
        assert!(!grid.contains(instant(START_MILLIS + 3_600_000)));
    }

    // --- quality survival on aggregation ---------------------------------

    #[test]
    fn empty_bucket_is_missing() {
        assert_eq!(aggregate_bucket_quality(&[]), TelemetryQuality::Missing);
    }

    #[test]
    fn all_good_survives_as_good() {
        assert_eq!(
            aggregate_bucket_quality(&[TelemetryQuality::Good, TelemetryQuality::Good]),
            TelemetryQuality::Good
        );
    }

    #[test]
    fn good_survives_only_if_every_input_is_good() {
        for q in TelemetryQuality::ALL {
            if q == TelemetryQuality::Good {
                continue;
            }
            assert_ne!(
                aggregate_bucket_quality(&[TelemetryQuality::Good, q, TelemetryQuality::Good]),
                TelemetryQuality::Good,
                "a single non-GOOD input must prevent a GOOD bucket"
            );
            assert_eq!(
                aggregate_bucket_quality(&[TelemetryQuality::Good, q, TelemetryQuality::Good]),
                q,
                "the non-GOOD input's state must decide the bucket"
            );
        }
    }

    #[test]
    fn estimated_stays_estimated() {
        assert_eq!(
            aggregate_bucket_quality(&[TelemetryQuality::Estimated, TelemetryQuality::Good]),
            TelemetryQuality::Estimated
        );
        assert_eq!(
            aggregate_bucket_quality(&[TelemetryQuality::Estimated, TelemetryQuality::Estimated]),
            TelemetryQuality::Estimated
        );
    }

    #[test]
    fn precedence_holds_for_every_pair_of_states() {
        // The recorded precedence order, highest wins.
        let precedence = [
            TelemetryQuality::Missing,
            TelemetryQuality::Bad,
            TelemetryQuality::Stale,
            TelemetryQuality::Suspect,
            TelemetryQuality::Estimated,
            TelemetryQuality::Good,
        ];
        let rank = |q: TelemetryQuality| {
            precedence
                .iter()
                .position(|p| *p == q)
                .expect("all six canonical states are ranked")
        };
        for a in TelemetryQuality::ALL {
            for b in TelemetryQuality::ALL {
                let expected = if rank(a) <= rank(b) { a } else { b };
                assert_eq!(
                    aggregate_bucket_quality(&[a, b]),
                    expected,
                    "pair ({a:?}, {b:?}) must resolve to the higher-precedence state"
                );
            }
        }
        // And a triple mixing the extremes.
        assert_eq!(
            aggregate_bucket_quality(&[
                TelemetryQuality::Good,
                TelemetryQuality::Stale,
                TelemetryQuality::Suspect,
            ]),
            TelemetryQuality::Stale
        );
    }

    #[test]
    fn value_aggregation_mean_sum_and_rejections() {
        let points = vec![
            (2.0, TelemetryQuality::Good),
            (4.0, TelemetryQuality::Good),
        ];
        let mean = aggregate_subinterval_values(&points, ValueAggregation::Mean).unwrap();
        assert_eq!(mean.value, 3.0);
        assert_eq!(mean.quality, TelemetryQuality::Good);
        let sum = aggregate_subinterval_values(&points, ValueAggregation::Sum).unwrap();
        assert_eq!(sum.value, 6.0);

        // A poisoned bucket keeps the value rule but not the quality.
        let poisoned = vec![
            (2.0, TelemetryQuality::Good),
            (4.0, TelemetryQuality::Missing),
        ];
        let agg = aggregate_subinterval_values(&poisoned, ValueAggregation::Mean).unwrap();
        assert_eq!(agg.value, 3.0);
        assert_eq!(agg.quality, TelemetryQuality::Missing);

        // Empty bucket: the caller must supply at least the gap markers.
        assert_eq!(
            aggregate_subinterval_values(&[], ValueAggregation::Mean),
            Err(SchedulingTimeError::EmptyTelemetryBucket)
        );
        // Non-finite values never aggregate.
        let nan = vec![(f64::NAN, TelemetryQuality::Good)];
        assert_eq!(
            aggregate_subinterval_values(&nan, ValueAggregation::Mean),
            Err(SchedulingTimeError::NonFiniteTelemetryValue)
        );
    }

    // --- horizon rollover -------------------------------------------------

    #[test]
    fn first_horizon_is_an_initial_revision_one() {
        let schedule_id = ScheduleId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap());
        let outcome = plan_horizon_rollover(None, schedule_id).unwrap();
        assert_eq!(
            outcome,
            RolloverOutcome::Initial {
                new_revision: ScheduleRevisionRef { schedule_id, revision: 1 }
            }
        );
    }

    #[test]
    fn rollover_supersedes_and_preserves_the_prior_revision() {
        // The rule: a new horizon supersedes the previous revision — never
        // silently overwrites it. The outcome carries the prior reference
        // UNCHANGED (retained for audit, §14.2) and bumps the revision.
        let schedule_id = ScheduleId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap());
        let prior = ScheduleRevisionRef { schedule_id, revision: 3 };
        let outcome = plan_horizon_rollover(Some(prior), schedule_id).unwrap();
        match outcome {
            RolloverOutcome::Supersedes { superseded, new_revision } => {
                assert_eq!(superseded, prior, "the prior revision is preserved unchanged");
                assert_eq!(new_revision.revision, 4);
                assert_eq!(new_revision.schedule_id, schedule_id);
            }
            other => panic!("expected Supersedes, got {other:?}"),
        }
        // A chain of rollovers strictly increases the revision number; every
        // prior stays recoverable from the outcome it was superseded in.
        let mut current = prior;
        for _ in 0..5 {
            match plan_horizon_rollover(Some(current), schedule_id).unwrap() {
                RolloverOutcome::Supersedes { superseded, new_revision } => {
                    assert_eq!(superseded, current);
                    assert!(new_revision.revision > superseded.revision);
                    current = new_revision;
                }
                other => panic!("expected Supersedes, got {other:?}"),
            }
        }
    }

    #[test]
    fn rollover_rejects_a_foreign_schedule_id() {
        let a = ScheduleId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap());
        let b = ScheduleId::new(FlexUuid::parse("00000000-0000-0000-0000-000000000001").unwrap());
        let prior = ScheduleRevisionRef { schedule_id: a, revision: 2 };
        assert_eq!(
            plan_horizon_rollover(Some(prior), b),
            Err(SchedulingTimeError::ForeignScheduleId)
        );
    }

    #[test]
    fn rollover_rejects_revision_overflow() {
        let schedule_id = ScheduleId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap());
        let prior = ScheduleRevisionRef { schedule_id, revision: u64::MAX };
        assert_eq!(
            plan_horizon_rollover(Some(prior), schedule_id),
            Err(SchedulingTimeError::RevisionOverflow)
        );
    }
}
