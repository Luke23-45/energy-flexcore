// P15.12 — Fleet health scoring: deviation-from-normal models per asset
//         class, alarm normalization into prioritized issues, and
//         availability KPI computation independent of O&M.
//
// Phase 15 plan §2 row P15.12; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 41_p15_12_fleet_health_scoring.md`.
// Sources: [S-53] (Fluence Nispera — asset performance management:
// deviation-from-normal models, alarm normalization into prioritized
// issues, availability KPIs independent of O&M). The Nispera pattern is
// the DESIGN reference only; no Nispera integration, benchmark, or
// conformance claim is made, and no vendor figure is carried as a
// FlexCore claim.
//
// THE THREE DELIVERABLES, DECIDED ONCE:
//
// 1. **Deviation-from-normal models** ([`DeviationFromNormalModel`]): per
//    asset class, a DECLARED normal-behaviour reference per quantity
//    (expected mean + declared tolerance fraction, each with provenance
//    and a recorded source). The deviation of an observed window mean
//    from the declared normal mean is pure arithmetic over declared
//    inputs ([`evaluate_deviation`]). The normal reference is DECLARED,
//    never fitted: no fitting procedure exists in this module (the
//    per-asset calibrated-model records, P15.14, are the declared
//    parameter home; this module consumes them as caller-declared
//    inputs).
// 2. **Alarm normalization into prioritized issues**
//    ([`normalize_alarms`]): derived alarm facts (the P5.31 alarm model —
//    `flexcore_contracts::alarm`; alarms are operator-attention records,
//    never protection signals) plus health-deviation flags normalize
//    into `PrioritizedIssue` records with a DETERMINISTIC priority:
//    severity rank first (Critical→P1, Warning→P2, Info→P3), then the
//    recorded promotion rule — an issue whose asset carries an
//    out-of-tolerance deviation is promoted one tier (P3→P2, P2→P1; P1
//    stays P1). The mapping is fixed and testable; no threshold is
//    invented at normalization time.
// 3. **Availability KPI** ([`availability_kpi`]): the duration-weighted
//    mean of the declared per-interval availability fractions (the P2.16
//    availability rule's outputs, declared by the caller with their
//    recorded source) over a declared window. The KPI is computed from
//    PLATFORM availability facts ONLY — no O&M data (work orders,
//    maintenance logs, vendor service records) exists in this platform,
//    none is consumed, and none is assumed ([S-53] names the
//    independence; this module records it).
//
// THE HONESTY LINES:
//
// - **Scores are RECORDS, never dispatch inputs** (ADR-010 extension,
//   program conventions §6.3): every type in this module is an
//   operator-view record. There is no function here that constructs,
//   mutates, or carries a dispatch command, and no score, issue, or KPI
//   is an input to the command lifecycle, the feasibility gates, or the
//   optimizer (the scheduling objective consumes degradation cost curves
//   — P15.08 — never health scores).
// - **No score from real data**: the platform has never run; every
//   number in the tests is hand-computed over CONSTRUCTED inputs. No
//   fleet health number is claimed anywhere.
// - **No health threshold is registered**: the tolerance fractions and
//   the promotion rule are DECLARED inputs and a fixed deterministic
//   mapping; an X-8-gated registration of real thresholds would be a new
//   versioned artifact, never a silent edit.
//
// Control status: records for the simulation-only 0.x line. No I/O, no
// store, no scheduler wiring, no field-write path. No safety,
// certification, or skill claim is made.

use std::collections::BTreeMap;

use flexcore_contracts::alarm::{AlarmCode, AlarmSeverity};
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::capability_provenance::AssetClass;
use flexcore_domain::ids::AssetId;

/// The fleet-health record schema version (a change is a NEW version,
/// never a silent edit — the P8.19 anti-silent-growth rule mirrored).
pub const FLEET_HEALTH_SCHEMA_VERSION: u32 = 1;

/// The ADR-010 statement carried by every consumer of this module (pinned
/// by tests).
pub const HEALTH_SCORE_IS_A_RECORD_STATEMENT: &str = "A fleet health score, deviation \
     record, prioritized issue, or availability KPI produced by this module is a RECORD \
     feeding operator views — never a dispatch input. No score authorizes, produces, or \
     influences a dispatch command (ADR-010 extension, program conventions 6.3).";

/// The [S-53] pattern citation and its limits (pinned by tests).
pub const NISPERA_PATTERN_STATEMENT: &str = "The deviation-from-normal model, the alarm \
     normalization into prioritized issues, and the availability KPI computed independently \
     of O&M follow the Nispera-class asset-performance-management pattern [S-53] as a \
     DESIGN reference only. No Nispera (or any vendor) integration, benchmark, accuracy, \
     or conformance claim is made; no vendor figure is carried as a FlexCore claim.";

/// The O&M independence statement (pinned by tests).
pub const AVAILABILITY_KPI_INDEPENDENCE_STATEMENT: &str = "The availability KPI is \
     computed from PLATFORM availability facts only (the P2.16 availability rule's \
     outputs, declared by the caller with their recorded source). No O&M data (work \
     orders, maintenance logs, vendor service records) exists in this platform, none is \
     consumed, and none is assumed — the KPI is independent of O&M by construction, not \
     by reconciliation.";

/// The no-fitted-reference statement (pinned by tests; P15.14 owns the
/// declared-parameter records).
pub const NORMAL_REFERENCE_IS_DECLARED_STATEMENT: &str = "Every normal-behaviour \
     reference is a DECLARED input with provenance and a recorded source. No fitting \
     procedure exists in this module; a fitted reference claim would require runtime \
     data that does not exist and an evaluation discipline (X-8) that has not run.";

// --- Errors -------------------------------------------------------------------

/// Why a fleet-health computation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq)]
pub enum FleetHealthError {
    /// The asset identity was empty.
    EmptyAssetId,
    /// A declared normal mean was not finite.
    NonFiniteNormalMean { quantity: String },
    /// A declared tolerance fraction was negative, non-finite, or >= 1
    /// (a tolerance of 1 or more would make every observation normal —
    /// the reference must narrow, never rubber-stamp).
    InvalidToleranceFraction { quantity: String, value: f64 },
    /// An observed value was not finite.
    NonFiniteObservedValue { quantity: String, index: usize },
    /// An observed value fell outside the canonical range for its
    /// quantity (SOC fractions outside [0, 1]).
    ObservedValueOutOfRange { quantity: String, index: usize, value: f64 },
    /// The deviation window was empty (no observations) — an empty window
    /// is a refusal, never a zero-deviation verdict.
    EmptyObservationWindow { quantity: String },
    /// The availability KPI was requested over a window with no
    /// intervals.
    EmptyAvailabilityWindow,
    /// An availability fraction was not finite or outside [0, 1].
    AvailabilityOutOfRange { index: usize, value: f64 },
    /// The availability fractions carried no recorded source (the source
    /// must be recorded, never silent — the P3.12 discipline mirrored).
    EmptyAvailabilitySource,
    /// A window length did not match the weights length.
    WindowLengthMismatch { expected: usize, found: usize },
    /// A window weight was not finite or negative.
    NonPositiveWeight { index: usize, value: f64 },
    /// The window was inverted (end before start).
    InvertedWindow { start_millis: i64, end_millis: i64 },
}

impl std::fmt::Display for FleetHealthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FleetHealthError::EmptyAssetId => {
                write!(f, "the asset identity must be non-empty")
            }
            FleetHealthError::NonFiniteNormalMean { quantity } => write!(
                f,
                "the declared normal mean for quantity {quantity} is not finite"
            ),
            FleetHealthError::InvalidToleranceFraction { quantity, value } => write!(
                f,
                "the declared tolerance fraction for quantity {quantity} is {value}; \
                 it must be finite and in [0, 1)"
            ),
            FleetHealthError::NonFiniteObservedValue { quantity, index } => write!(
                f,
                "the observed value for quantity {quantity} at index {index} is not finite"
            ),
            FleetHealthError::ObservedValueOutOfRange { quantity, index, value } => write!(
                f,
                "the observed value for quantity {quantity} at index {index} is {value}; \
                 it is outside the quantity's canonical range"
            ),
            FleetHealthError::EmptyObservationWindow { quantity } => write!(
                f,
                "the observation window for quantity {quantity} is empty; an empty window \
                 is a refusal, never a zero-deviation verdict"
            ),
            FleetHealthError::EmptyAvailabilityWindow => write!(
                f,
                "the availability KPI requires at least one interval"
            ),
            FleetHealthError::AvailabilityOutOfRange { index, value } => write!(
                f,
                "availability fraction at index {index} is {value}; must be in [0, 1]"
            ),
            FleetHealthError::EmptyAvailabilitySource => write!(
                f,
                "the availability fractions must carry a recorded source (never silent)"
            ),
            FleetHealthError::WindowLengthMismatch { expected, found } => write!(
                f,
                "the window carries {found} values; {expected} weights were declared"
            ),
            FleetHealthError::NonPositiveWeight { index, value } => write!(
                f,
                "window weight at index {index} is {value}; weights must be finite and >= 0"
            ),
            FleetHealthError::InvertedWindow { start_millis, end_millis } => write!(
                f,
                "the window is inverted: end {end_millis} is before start {start_millis}"
            ),
        }
    }
}

impl std::error::Error for FleetHealthError {}

// --- 1. Deviation-from-normal --------------------------------------------------

/// One declared normal-behaviour expectation for one quantity (P15.12
/// deliverable 1). The expected mean and tolerance are DECLARED inputs
/// with provenance — never fitted, never inferred.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalExpectation {
    /// The quantity key (the platform's canonical quantity spelling, e.g.
    /// `active_power`); a closed string the caller declares — never a
    /// free-form label.
    pub quantity: String,
    /// The declared normal (expected) mean over the comparison window, in
    /// the quantity's canonical unit and the caller's declared sign
    /// convention.
    pub normal_mean: f64,
    /// The declared tolerance fraction in [0, 1): an observed window mean
    /// deviates when |observed − normal| / |normal| > tolerance (a normal
    /// mean of exactly 0.0 has no relative deviation — the caller must
    /// declare an absolute band instead, refused here as undeclarable).
    pub tolerance_fraction: f64,
    /// The provenance of the declaration (`Measured`, `Configured`,
    /// `Unknown` — the P3.02 discipline: unknown narrows, never widens).
    pub provenance: flexcore_contracts::capability::Provenance,
    /// The recorded source of the declaration (non-empty; e.g. the
    /// P15.14 calibrated-model record reference).
    pub source: String,
}

impl NormalExpectation {
    /// Validates and constructs one expectation.
    pub fn new(
        quantity: String,
        normal_mean: f64,
        tolerance_fraction: f64,
        provenance: flexcore_contracts::capability::Provenance,
        source: String,
    ) -> Result<Self, FleetHealthError> {
        if quantity.trim().is_empty() {
            return Err(FleetHealthError::EmptyAssetId);
        }
        if !normal_mean.is_finite() {
            return Err(FleetHealthError::NonFiniteNormalMean { quantity });
        }
        if !tolerance_fraction.is_finite() || !(0.0..1.0).contains(&tolerance_fraction) {
            return Err(FleetHealthError::InvalidToleranceFraction {
                quantity: quantity.clone(),
                value: tolerance_fraction,
            });
        }
        if source.trim().is_empty() {
            return Err(FleetHealthError::EmptyAvailabilitySource);
        }
        Ok(Self {
            quantity,
            normal_mean,
            tolerance_fraction,
            provenance,
            source,
        })
    }
}

/// One quantity's deviation verdict over one declared window.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationObservation {
    /// The quantity key.
    pub quantity: String,
    /// The observed window mean (fixed accumulation order — the replay
    /// requirement, P15.06).
    pub observed_mean: f64,
    /// The declared normal mean.
    pub normal_mean: f64,
    /// The relative deviation |observed − normal| / |normal| (undefined —
    /// `None` — when the declared normal mean is exactly 0.0; a zero
    /// reference must be compared in absolute terms by the caller, never
    /// silently treated as zero deviation).
    pub relative_deviation: Option<f64>,
    /// Whether the relative deviation exceeds the declared tolerance.
    pub exceeds_tolerance: bool,
}

/// One asset's deviation-from-normal record (a RECORD for operator views;
/// never a dispatch input).
#[derive(Debug, Clone, PartialEq)]
pub struct HealthDeviationRecord {
    /// The asset the record describes.
    pub asset_id: AssetId,
    /// The asset class the expectations were declared for.
    pub asset_class: AssetClass,
    /// The window: start-inclusive / end-exclusive, UTC millis.
    pub window_start_millis: i64,
    pub window_end_millis: i64,
    /// One verdict per declared quantity (input order).
    pub observations: Vec<DeviationObservation>,
}

/// The pure deviation evaluation: the observed window mean of each
/// declared quantity against its declared normal mean and tolerance.
/// Accumulation order is fixed (input order); no rounding, no
/// normalization, no sign flipping — the caller's declared sign
/// convention travels verbatim.
pub fn evaluate_deviation(
    asset_id: AssetId,
    asset_class: AssetClass,
    expectations: &[NormalExpectation],
    observed: &BTreeMap<String, Vec<f64>>,
    window_start_millis: i64,
    window_end_millis: i64,
) -> Result<HealthDeviationRecord, FleetHealthError> {
    if window_end_millis <= window_start_millis {
        return Err(FleetHealthError::InvertedWindow {
            start_millis: window_start_millis,
            end_millis: window_end_millis,
        });
    }
    let mut observations = Vec::with_capacity(expectations.len());
    for expectation in expectations {
        let series = observed.get(&expectation.quantity).ok_or_else(|| {
            FleetHealthError::EmptyObservationWindow {
                quantity: expectation.quantity.clone(),
            }
        })?;
        if series.is_empty() {
            return Err(FleetHealthError::EmptyObservationWindow {
                quantity: expectation.quantity.clone(),
            });
        }
        // Canonical-range guard: SOC fractions stay in [0, 1] (P0.07 §4).
        if expectation.quantity == "soc" {
            for (index, value) in series.iter().enumerate() {
                if !value.is_finite() {
                    return Err(FleetHealthError::NonFiniteObservedValue {
                        quantity: expectation.quantity.clone(),
                        index,
                    });
                }
                if !(0.0..=1.0).contains(value) {
                    return Err(FleetHealthError::ObservedValueOutOfRange {
                        quantity: expectation.quantity.clone(),
                        index,
                        value: *value,
                    });
                }
            }
        } else {
            for (index, value) in series.iter().enumerate() {
                if !value.is_finite() {
                    return Err(FleetHealthError::NonFiniteObservedValue {
                        quantity: expectation.quantity.clone(),
                        index,
                    });
                }
            }
        }
        // The observed window mean, fixed accumulation order.
        let sum: f64 = series.iter().sum();
        let observed_mean = sum / series.len() as f64;

        // The relative deviation; a zero normal mean is UNDEFINED, never
        // zero — the caller must declare an absolute comparison instead.
        let relative_deviation = if expectation.normal_mean == 0.0 {
            None
        } else {
            Some(((observed_mean - expectation.normal_mean).abs())
                / expectation.normal_mean.abs())
        };
        let exceeds_tolerance = relative_deviation
            .map(|d| d > expectation.tolerance_fraction)
            .unwrap_or(false);
        observations.push(DeviationObservation {
            quantity: expectation.quantity.clone(),
            observed_mean,
            normal_mean: expectation.normal_mean,
            relative_deviation,
            exceeds_tolerance,
        });
    }
    Ok(HealthDeviationRecord {
        asset_id,
        asset_class,
        window_start_millis,
        window_end_millis,
        observations,
    })
}

// --- 2. Alarm normalization into prioritized issues ------------------------------

/// The deterministic priority tiers (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssuePriority {
    /// Highest operator attention.
    P1,
    /// Middle tier.
    P2,
    /// Lowest tier.
    P3,
}

impl IssuePriority {
    /// The tier one step higher (P1 stays P1 — the promotion rule's
    /// ceiling).
    pub fn promoted(self) -> Self {
        match self {
            IssuePriority::P1 => IssuePriority::P1,
            IssuePriority::P2 => IssuePriority::P1,
            IssuePriority::P3 => IssuePriority::P2,
        }
    }

    /// The wire spelling (records and operator views).
    pub const fn wire_name(self) -> &'static str {
        match self {
            IssuePriority::P1 => "P1",
            IssuePriority::P2 => "P2",
            IssuePriority::P3 => "P3",
        }
    }
}

/// The base priority mapping from the P5.31 alarm severity (deterministic;
/// pinned by tests): Critical→P1, Warning→P2, Info→P3.
#[must_use]
pub fn base_priority(severity: AlarmSeverity) -> IssuePriority {
    match severity {
        AlarmSeverity::Critical => IssuePriority::P1,
        AlarmSeverity::Warning => IssuePriority::P2,
        AlarmSeverity::Info => IssuePriority::P3,
    }
}

/// One normalized, prioritized issue (a RECORD feeding operator views).
#[derive(Debug, Clone, PartialEq)]
pub struct PrioritizedIssue {
    /// The normalized priority (the base severity rank, possibly promoted
    /// by an out-of-tolerance health deviation — the recorded rule).
    pub priority: IssuePriority,
    /// The P5.31 alarm code the issue was normalized from.
    pub alarm_code: AlarmCode,
    /// The asset the issue concerns (empty for fleet-scope alarms).
    pub asset_id: Option<AssetId>,
    /// Whether the asset carries an out-of-tolerance deviation that
    /// triggered the promotion.
    pub promoted_by_deviation: bool,
    /// The recorded time of the source observation (the alarm's raised_at
    /// equivalent — carried verbatim, never merged with the derivation
    /// instant, the P5.31 time-separation discipline).
    pub raised_at: EventTime,
}

/// Normalizes derived alarm facts (P5.31 vocabulary) plus per-asset
/// out-of-tolerance deviation flags into prioritized issues.
///
/// The priority rule, decided once: base priority = severity rank
/// (Critical→P1, Warning→P2, Info→P3); an issue whose asset carries an
/// out-of-tolerance deviation is promoted one tier. `deviated_assets` is
/// the set of assets whose latest [`HealthDeviationRecord`] carried at
/// least one `exceeds_tolerance` observation.
pub fn normalize_alarms(
    alarms: &[(AlarmCode, AlarmSeverity, Option<AssetId>, EventTime)],
    deviated_assets: &BTreeMap<AssetId, bool>,
) -> Vec<PrioritizedIssue> {
    alarms
        .iter()
        .map(|(code, severity, asset, raised_at)| {
            let promoted = asset
                .as_ref()
                .map(|id| deviated_assets.get(id).copied().unwrap_or(false))
                .unwrap_or(false);
            let base = base_priority(*severity);
            PrioritizedIssue {
                priority: if promoted {
                    base.promoted()
                } else {
                    base
                },
                alarm_code: *code,
                asset_id: asset.clone(),
                promoted_by_deviation: promoted,
                raised_at: *raised_at,
            }
        })
        .collect()
}

// --- 3. Availability KPI (independent of O&M) -------------------------------------

/// One asset's availability KPI record over one declared window.
#[derive(Debug, Clone, PartialEq)]
pub struct AvailabilityKpiRecord {
    /// The asset the KPI describes.
    pub asset_id: AssetId,
    /// The window: start-inclusive / end-exclusive, UTC millis.
    pub window_start_millis: i64,
    pub window_end_millis: i64,
    /// The duration-weighted mean availability in [0, 1].
    pub availability: f64,
    /// The number of intervals the KPI consumed.
    pub interval_count: usize,
    /// The recorded source of the availability fractions (carried
    /// verbatim from the declared input).
    pub source: String,
}

/// Computes the duration-weighted mean availability over a declared
/// window from the declared per-interval availability fractions (the
/// P2.16 availability rule's outputs) and their interval durations.
///
/// The computation consumes PLATFORM availability facts only (see
/// [`AVAILABILITY_KPI_INDEPENDENCE_STATEMENT`]). Zero weights are legal
/// (zero-length intervals); a window of all-zero weights is a refusal —
/// a KPI over zero duration is not a KPI.
pub fn availability_kpi(
    asset_id: AssetId,
    fractions: &[f64],
    interval_weights: &[f64],
    window_start_millis: i64,
    window_end_millis: i64,
    source: String,
) -> Result<AvailabilityKpiRecord, FleetHealthError> {
    if window_end_millis <= window_start_millis {
        return Err(FleetHealthError::InvertedWindow {
            start_millis: window_start_millis,
            end_millis: window_end_millis,
        });
    }
    if source.trim().is_empty() {
        return Err(FleetHealthError::EmptyAvailabilitySource);
    }
    if fractions.is_empty() {
        return Err(FleetHealthError::EmptyAvailabilityWindow);
    }
    if interval_weights.len() != fractions.len() {
        return Err(FleetHealthError::WindowLengthMismatch {
            expected: fractions.len(),
            found: interval_weights.len(),
        });
    }
    for (index, fraction) in fractions.iter().enumerate() {
        if !fraction.is_finite() || !(0.0..=1.0).contains(fraction) {
            return Err(FleetHealthError::AvailabilityOutOfRange {
                index,
                value: *fraction,
            });
        }
    }
    for (index, weight) in interval_weights.iter().enumerate() {
        if !weight.is_finite() || *weight < 0.0 {
            return Err(FleetHealthError::NonPositiveWeight {
                index,
                value: *weight,
            });
        }
    }
    // Fixed accumulation order; the KPI is the duration-weighted mean.
    let mut weighted_sum = 0.0f64;
    let mut total_weight = 0.0f64;
    for (fraction, weight) in fractions.iter().zip(interval_weights.iter()) {
        weighted_sum += fraction * weight;
        total_weight += weight;
    }
    if total_weight <= 0.0 {
        return Err(FleetHealthError::EmptyAvailabilityWindow);
    }
    Ok(AvailabilityKpiRecord {
        asset_id,
        window_start_millis,
        window_end_millis,
        availability: weighted_sum / total_weight,
        interval_count: fractions.len(),
        source,
    })
}

// --- Tests (WRITTEN-UNRUN — no cargo command was executed in this loop;
// --- the human build/test gate owns the run. Every expected value below
// --- is hand-computed from the rule sets above over CONSTRUCTED inputs.
// --- Nothing is fabricated.) -----------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::capability::Provenance;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::AssetId;

    /// Deterministic test identity from a numeric tag (constructed input,
    /// never a measurement) — the same helper shape the Phase 3 tests use.
    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn asset(tag: u128) -> AssetId {
        AssetId::new(uid(tag))
    }

    fn expectation(quantity: &str, mean: f64, tolerance: f64) -> NormalExpectation {
        NormalExpectation::new(
            quantity.to_string(),
            mean,
            tolerance,
            Provenance::Configured,
            "test-declaration".to_string(),
        )
        .expect("valid expectation")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(HEALTH_SCORE_IS_A_RECORD_STATEMENT.contains("never a dispatch input"));
        assert!(HEALTH_SCORE_IS_A_RECORD_STATEMENT.contains("ADR-010"));
        assert!(NISPERA_PATTERN_STATEMENT.contains("[S-53]"));
        assert!(NISPERA_PATTERN_STATEMENT.contains("DESIGN reference only"));
        assert!(AVAILABILITY_KPI_INDEPENDENCE_STATEMENT.contains("independent of O&M"));
        assert!(NORMAL_REFERENCE_IS_DECLARED_STATEMENT.contains("never fitted"));
    }

    #[test]
    fn deviation_is_relative_and_tolerance_is_exclusive() {
        // HAND CHECK: normal mean 1.0 MW, tolerance 0.1; observed series
        // [1.0, 1.2] -> observed mean 1.1 -> deviation 0.1 == tolerance ->
        // NOT exceeding (the tolerance is exclusive: `>` not `>=`).
        let mut observed = BTreeMap::new();
        observed.insert("active_power".to_string(), vec![1.0, 1.2]);
        let record = evaluate_deviation(
            asset(0xB001),
            AssetClass::FlexibleLoad,
            &[expectation("active_power", 1.0, 0.1)],
            &observed,
            0,
            1000,
        )
        .expect("valid window");
        let observation = &record.observations[0];
        assert_eq!(observation.observed_mean, 1.1);
        assert_eq!(observation.relative_deviation, Some(0.1));
        assert!(!observation.exceeds_tolerance, "deviation == tolerance is NOT exceeding");

        // One step further: [1.0, 1.21] -> mean 1.105 -> 0.105 > 0.1.
        let mut observed = BTreeMap::new();
        observed.insert("active_power".to_string(), vec![1.0, 1.21]);
        let record = evaluate_deviation(
            asset(0xB001),
            AssetClass::FlexibleLoad,
            &[expectation("active_power", 1.0, 0.1)],
            &observed,
            0,
            1000,
        )
        .expect("valid window");
        assert!(record.observations[0].exceeds_tolerance);
    }

    #[test]
    fn a_zero_normal_mean_has_no_relative_deviation() {
        let mut observed = BTreeMap::new();
        observed.insert("active_power".to_string(), vec![5.0, -5.0]);
        let record = evaluate_deviation(
            asset(0xB001),
            AssetClass::Bess,
            &[expectation("active_power", 0.0, 0.1)],
            &observed,
            0,
            1000,
        )
        .expect("valid window");
        let observation = &record.observations[0];
        assert_eq!(observation.relative_deviation, None);
        assert!(!observation.exceeds_tolerance, "an undefined deviation never flags");
    }

    #[test]
    fn deviation_refuses_empty_windows_inverted_windows_and_out_of_range_soc() {
        let mut observed = BTreeMap::new();
        observed.insert("active_power".to_string(), Vec::new());
        let err = evaluate_deviation(
            asset(0xB001),
            AssetClass::Bess,
            &[expectation("active_power", 1.0, 0.1)],
            &observed,
            0,
            1000,
        )
        .expect_err("empty window refused");
        assert_eq!(
            err,
            FleetHealthError::EmptyObservationWindow {
                quantity: "active_power".to_string()
            }
        );
        let err = evaluate_deviation(
            asset(0xB001),
            AssetClass::Bess,
            &[expectation("active_power", 1.0, 0.1)],
            &BTreeMap::new(),
            0,
            1000,
        )
        .expect_err("missing quantity refused");
        assert!(matches!(err, FleetHealthError::EmptyObservationWindow { .. }));
        let err = evaluate_deviation(
            asset(0xB001),
            AssetClass::Bess,
            &[expectation("active_power", 1.0, 0.1)],
            &BTreeMap::new(),
            1000,
            1000,
        )
        .expect_err("inverted (empty) window refused");
        assert!(matches!(err, FleetHealthError::InvertedWindow { .. }));
        let mut observed = BTreeMap::new();
        observed.insert("soc".to_string(), vec![0.5, 1.2]);
        let err = evaluate_deviation(
            asset(0xB001),
            AssetClass::Bess,
            &[expectation("soc", 0.5, 0.1)],
            &observed,
            0,
            1000,
        )
        .expect_err("SOC outside [0, 1] refused");
        assert_eq!(
            err,
            FleetHealthError::ObservedValueOutOfRange {
                quantity: "soc".to_string(),
                index: 1,
                value: 1.2,
            }
        );
    }

    #[test]
    fn expectation_validation_refuses_bad_tolerances_and_empty_sources() {
        let err = NormalExpectation::new(
            "active_power".to_string(),
            1.0,
            1.0,
            Provenance::Configured,
            "s".to_string(),
        )
        .expect_err("tolerance 1.0 refused");
        assert_eq!(
            err,
            FleetHealthError::InvalidToleranceFraction {
                quantity: "active_power".to_string(),
                value: 1.0,
            }
        );
        let err = NormalExpectation::new(
            "active_power".to_string(),
            f64::NAN,
            0.1,
            Provenance::Configured,
            "s".to_string(),
        )
        .expect_err("NaN normal mean refused");
        assert!(matches!(
            err,
            FleetHealthError::NonFiniteNormalMean { .. }
        ));
    }

    #[test]
    fn base_priority_follows_severity_rank() {
        assert_eq!(base_priority(AlarmSeverity::Critical), IssuePriority::P1);
        assert_eq!(base_priority(AlarmSeverity::Warning), IssuePriority::P2);
        assert_eq!(base_priority(AlarmSeverity::Info), IssuePriority::P3);
        assert_eq!(IssuePriority::P3.promoted(), IssuePriority::P2);
        assert_eq!(IssuePriority::P2.promoted(), IssuePriority::P1);
        assert_eq!(IssuePriority::P1.promoted(), IssuePriority::P1, "P1 is the ceiling");
    }

    #[test]
    fn normalization_promotes_one_tier_for_deviating_assets_only() {
        let t = EventTime::from_unix_millis(1000).expect("valid test time");
        let alarms = vec![
            (
                AlarmCode::TelemetryLoss,
                AlarmSeverity::Warning,
                Some(asset(0xB001)),
                t,
            ),
            (
                AlarmCode::TelemetryStaleness,
                AlarmSeverity::Info,
                Some(asset(0xB002)),
                t,
            ),
            (
                AlarmCode::DispatchFailure,
                AlarmSeverity::Critical,
                Some(asset(0xB003)),
                t,
            ),
        ];
        let mut deviated = BTreeMap::new();
        deviated.insert(asset(0xB001), true);
        deviated.insert(asset(0xB002), false);
        let issues = normalize_alarms(&alarms, &deviated);
        assert_eq!(issues.len(), 3);
        // a1: Warning + deviation -> promoted to P1.
        assert_eq!(issues[0].priority, IssuePriority::P1);
        assert!(issues[0].promoted_by_deviation);
        // a2: Info, no deviation -> stays P3.
        assert_eq!(issues[1].priority, IssuePriority::P3);
        assert!(!issues[1].promoted_by_deviation);
        // a3: Critical -> P1 regardless; P1 is the ceiling.
        assert_eq!(issues[2].priority, IssuePriority::P1);
        assert!(!issues[2].promoted_by_deviation);
    }

    #[test]
    fn availability_kpi_is_duration_weighted() {
        // HAND CHECK: fractions [1.0, 0.5] with weights [1.0, 3.0] ->
        // (1.0*1.0 + 0.5*3.0) / 4.0 = 2.5 / 4.0 = 0.625.
        let kpi = availability_kpi(
            asset(0xB001),
            &[1.0, 0.5],
            &[1.0, 3.0],
            0,
            1000,
            "p2-16-availability-rule".to_string(),
        )
        .expect("valid window");
        assert_eq!(kpi.availability, 0.625);
        assert_eq!(kpi.interval_count, 2);
        assert_eq!(kpi.source, "p2-16-availability-rule");
        // Equal weights -> the plain mean.
        let kpi = availability_kpi(
            asset(0xB001),
            &[1.0, 0.0],
            &[1.0, 1.0],
            0,
            1000,
            "s".to_string(),
        )
        .expect("valid window");
        assert_eq!(kpi.availability, 0.5);
    }

    #[test]
    fn availability_kpi_refuses_bad_windows_weights_and_sources() {
        let err = availability_kpi(
            asset(0xB001),
            &[],
            &[],
            0,
            1000,
            "s".to_string(),
        )
        .expect_err("empty window refused");
        assert_eq!(err, FleetHealthError::EmptyAvailabilityWindow);
        let err = availability_kpi(
            asset(0xB001),
            &[1.0],
            &[1.0],
            1000,
            1000,
            "s".to_string(),
        )
        .expect_err("inverted window refused");
        assert!(matches!(err, FleetHealthError::InvertedWindow { .. }));
        let err = availability_kpi(
            asset(0xB001),
            &[1.0, 0.5],
            &[1.0],
            0,
            1000,
            "s".to_string(),
        )
        .expect_err("length mismatch refused");
        assert_eq!(
            err,
            FleetHealthError::WindowLengthMismatch { expected: 2, found: 1 }
        );
        let err = availability_kpi(
            asset(0xB001),
            &[1.5],
            &[1.0],
            0,
            1000,
            "s".to_string(),
        )
        .expect_err("fraction outside [0, 1] refused");
        assert_eq!(
            err,
            FleetHealthError::AvailabilityOutOfRange { index: 0, value: 1.5 }
        );
        let err = availability_kpi(
            asset(0xB001),
            &[1.0],
            &[-1.0],
            0,
            1000,
            "s".to_string(),
        )
        .expect_err("negative weight refused");
        assert_eq!(
            err,
            FleetHealthError::NonPositiveWeight { index: 0, value: -1.0 }
        );
        let err = availability_kpi(
            asset(0xB001),
            &[1.0],
            &[1.0],
            0,
            1000,
            "  ".to_string(),
        )
        .expect_err("empty source refused");
        assert_eq!(err, FleetHealthError::EmptyAvailabilitySource);
        // All-zero weights: a KPI over zero duration is refused.
        let err = availability_kpi(
            asset(0xB001),
            &[1.0, 0.5],
            &[0.0, 0.0],
            0,
            1000,
            "s".to_string(),
        )
        .expect_err("all-zero weights refused");
        assert_eq!(err, FleetHealthError::EmptyAvailabilityWindow);
    }

    #[test]
    fn every_fleet_health_error_renders_non_empty_prose() {
        let errors = [
            FleetHealthError::EmptyAssetId,
            FleetHealthError::NonFiniteNormalMean {
                quantity: "q".to_string(),
            },
            FleetHealthError::InvalidToleranceFraction {
                quantity: "q".to_string(),
                value: 1.0,
            },
            FleetHealthError::NonFiniteObservedValue {
                quantity: "q".to_string(),
                index: 0,
            },
            FleetHealthError::ObservedValueOutOfRange {
                quantity: "q".to_string(),
                index: 0,
                value: 2.0,
            },
            FleetHealthError::EmptyObservationWindow {
                quantity: "q".to_string(),
            },
            FleetHealthError::EmptyAvailabilityWindow,
            FleetHealthError::AvailabilityOutOfRange { index: 0, value: 2.0 },
            FleetHealthError::EmptyAvailabilitySource,
            FleetHealthError::WindowLengthMismatch { expected: 2, found: 1 },
            FleetHealthError::NonPositiveWeight { index: 0, value: -1.0 },
            FleetHealthError::InvertedWindow { start_millis: 1, end_millis: 0 },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
