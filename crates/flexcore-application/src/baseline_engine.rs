// P16.04 — The program-configurable baseline engine: CBL 10-in-10 /
//         5-in-10 / day-matching with day-type and weather filters, and
//         the capped symmetric SDA (CAISO ≤20% [S-30]).
//
// Phase 16 plan §2 row P16.04; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 04_p16_04_baseline_engine.md`.
//
// THE POLICY, DECIDED ONCE (LR-073):
//
//   **FlexCore NEVER chooses the baseline method.** The method is the
//   PROGRAM's declared configuration (carried by the domain's
//   `BaselineMethodDeclaration`, `flexcore_domain::market_program_model`).
//   This engine consumes the declared method NAME and computes ONLY the
//   closed supported set (`CBL_10_IN_10`, `CBL_5_IN_10`,
//   `DAY_MATCHING`). Any other declared method — and any required input
//   the program's method needs that was not supplied — produces a
//   RECORDED GAP ROW (`BaselineGapRecord`), never a silent
//   approximation and never a FlexCore-chosen substitute. The gap row is
//   the honest record of what could not be computed and why; persisting
//   it is the caller's write-path obligation (a financial-adjacent
//   record, P16.01 audit-chain linkage).
//
// SEPARATION FROM THE FLEXIBLE-LOAD MODEL (recorded): the flexible-load
// model's `baseline_method` field
// (`flexcore_domain::flexible_load_model`) remains restricted to the
// decided scenario-declared synthetic method — that restriction is the
// Phase 3 decision and is NOT changed by this module. The market
// baseline engine is a separate, record-level engine for program-
// declared DR baselines; it is never wired into the flexible-load model
// and never relaxes its restriction.
//
// DETERMINISM: the engine is a set of pure functions over declared
// inputs. Time enters ONLY through the injected `Clock`
// (`flexcore_ports::clock::Clock`) for the gap-record/computed-record
// timestamps; day selection is BTreeMap-ordered by day key with explicit
// tie-breaking (total descending, then day key ascending); no wall
// clock, no map iteration order, no RNG. Same inputs + same clock ⇒
// byte-identical outputs (the replay property the written-unrun tests
// pin).
//
// SOURCED BOUNDS: the symmetric SDA cap is program configuration bounded
// by the program's JURISDICTION — the bound is RECORDED DATA carried on
// the jurisdiction record pack (for CAISO the documented bound is ≤20%,
// `CAISO_SDA_MAX_PERCENT`, cited [S-30], re-verified at use). A
// jurisdiction with NO recorded bound refuses an SDA (a recorded gap) —
// never an assumed default (FRV1-R7-005). The cap is SYMMETRIC: the same
// bound applies to upward and downward adjustments, and the adjustment is
// capped, never clipped into silence — the pre-cap ratio is recorded
// alongside the applied one.
//
// Control status: pure computation for the simulation-only 0.x line. No
// I/O, no store, no dispatch authority (ADR-010: baselines are inputs
// and records, never commands).

use flexcore_ports::clock::Clock;
use std::collections::BTreeMap;

/// The baseline-engine record schema version (a change is a NEW
/// version, never a silent edit).
pub const BASELINE_ENGINE_SCHEMA_VERSION: u32 = 1;

/// The LR-073 statement (pinned by tests).
pub const FLEXCORE_NEVER_CHOOSES_STATEMENT: &str = "FlexCore NEVER chooses the baseline \
     method: the method is the program's declared configuration (LR-073). The engine \
     computes ONLY the closed supported set and produces a RECORDED GAP ROW for any other \
     declared method or any missing required input — never a silent approximation and \
     never a FlexCore-chosen substitute.";

/// The CAISO symmetric-SDA bound in percent, cited from [S-30] (the CAISO
/// BPM for Demand Response). SOURCED DATA with the re-verify-at-use rule —
/// the CAISO jurisdiction record pack's declared bound
/// (`jurisdiction_register_data`); other jurisdictions carry their OWN
/// recorded bounds, and a jurisdiction without one refuses an SDA rather
/// than assuming this constant (FRV1-R7-005).
pub const CAISO_SDA_MAX_PERCENT: f64 = 20.0;

/// The closed supported-method vocabulary (the canonical spellings).
pub const SUPPORTED_METHOD_NAMES: [&str; 3] = ["CBL_10_IN_10", "CBL_5_IN_10", "DAY_MATCHING"];

/// The CBL recency-window size for the two CBL methods: the eligible
/// non-event days ordered by day key, taking the LAST ten (most recent)
/// before the highest-N selection. Cited from the 10-in-10 / 5-in-10
/// construction [S-30] — re-verified at use.
pub const CBL_RECENCY_WINDOW_DAYS: usize = 10;

// --- Gap records ---------------------------------------------------------------

/// Why the engine recorded a gap instead of a baseline. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineGapReason {
    /// The program declared a method outside the closed supported set
    /// (LR-073: recorded, never approximated).
    UnsupportedMethod,
    /// The eligible-day set could not satisfy the method's requirement
    /// (`required`/`found` carry the numbers).
    InsufficientEligibleDays {
        required: usize,
        found: usize,
    },
    /// The eligible day profiles do not share the event day's interval
    /// count.
    IntervalCountMismatch {
        day_key: String,
        expected: usize,
        found: usize,
    },
    /// The declared configuration includes an SDA but the same-day
    /// adjustment window was not supplied (an SDA is never silently
    /// skipped).
    MissingSdaWindow,
    /// The SDA configuration exceeded the program's OWN recorded
    /// jurisdiction bound (for a CAISO pack, above
    /// `CAISO_SDA_MAX_PERCENT` [S-30]; every other jurisdiction refuses
    /// against its own recorded bound — FRV1-R7-005).
    SdaCapExceedsJurisdictionBound {
        declared_percent: f64,
        jurisdiction_bound_percent: f64,
    },
    /// The program declares an SDA but its jurisdiction carries NO
    /// recorded symmetric-SDA bound — the bound is never assumed
    /// (FRV1-R7-005), so no SDA can be applied.
    SdaJurisdictionBoundUnrecorded,
    /// The SDA's baseline (reference) window sums to zero or less, so the
    /// adjustment ratio is not representable (the ratio divides by the
    /// reference window). Recorded as a gap — never a silent no-op
    /// "applied" adjustment (FRV1-R7-004).
    SdaReferenceWindowZero,
    /// The declared SDA window fell outside the event window's interval
    /// grid (an adjustment is never computed against indices that do
    /// not exist).
    InvalidSdaWindow,
    /// The injected clock reported an instant outside the record
    /// timestamp domain (never silently clamped).
    ClockOutOfRange,
}

impl std::fmt::Display for BaselineGapReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaselineGapReason::UnsupportedMethod => write!(
                f,
                "the declared baseline method is outside the closed supported set (LR-073: \
                 recorded as a gap, never approximated)"
            ),
            BaselineGapReason::InsufficientEligibleDays { required, found } => write!(
                f,
                "the eligible-day set cannot satisfy the method: {required} required, {found} \
                 found"
            ),
            BaselineGapReason::IntervalCountMismatch {
                day_key,
                expected,
                found,
            } => write!(
                f,
                "the day profile {day_key} carries {found} intervals; the event window \
                 declares {expected}"
            ),
            BaselineGapReason::MissingSdaWindow => write!(
                f,
                "the configuration declares an SDA but the same-day adjustment window was not \
                 supplied — an SDA is never silently skipped"
            ),
            BaselineGapReason::SdaCapExceedsJurisdictionBound {
                declared_percent,
                jurisdiction_bound_percent,
            } => write!(
                f,
                "the declared SDA cap {declared_percent}% exceeds the jurisdiction bound \
                 {jurisdiction_bound_percent}%"
            ),
            BaselineGapReason::SdaJurisdictionBoundUnrecorded => write!(
                f,
                "the program declares an SDA but its jurisdiction carries no recorded \
                 symmetric-SDA bound — the bound is never assumed, so no SDA is applied"
            ),
            BaselineGapReason::SdaReferenceWindowZero => write!(
                f,
                "the SDA's baseline reference window sums to zero or less; the adjustment \
                 ratio is not representable — recorded as a gap, never a silent no-op \
                 adjustment"
            ),
            BaselineGapReason::InvalidSdaWindow => write!(
                f,
                "the declared SDA window falls outside the event window's interval grid"
            ),
            BaselineGapReason::ClockOutOfRange => write!(
                f,
                "the injected clock reported an instant outside the record timestamp domain \
                 (never silently clamped)"
            ),
        }
    }
}

/// The RECORDED GAP ROW: the honest record of a baseline that could not
/// be computed and why (LR-073). Persisting it with its P11 audit-chain
/// linkage is the caller's obligation (a financial-adjacent record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineGapRecord {
    pub tenant_id: String,
    pub program_id: String,
    /// The declared method name, as the program wrote it.
    pub method_name: String,
    pub reason: BaselineGapReason,
    /// When the gap was recorded (the INJECTED clock's instant — the
    /// P0.09 time-separation discipline; never a wall-clock read).
    pub recorded_at_unix_ms: i64,
}

impl BaselineGapRecord {
    /// Builds the gap record (non-empty tenant/program/method; the
    /// timestamp must be non-negative — the caller's clock guarantees
    /// it; a negative value is refused here too).
    pub fn new(
        tenant_id: String,
        program_id: String,
        method_name: String,
        reason: BaselineGapReason,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, BaselineEngineError> {
        for (field, value) in [
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("method_name", method_name.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(BaselineEngineError::EmptyField { field });
            }
        }
        if recorded_at_unix_ms < 0 {
            return Err(BaselineEngineError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            tenant_id,
            program_id,
            method_name,
            reason,
            recorded_at_unix_ms,
        })
    }
}

/// The engine's named errors (construction-time refusals; computation
/// outcomes are `BaselineOutcome::Gap`, not errors — a gap is a RECORD,
/// not an exception).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineEngineError {
    EmptyField {
        field: &'static str,
    },
    /// A timestamp field carried a negative value — not an empty field, a
    /// negative one (named separately: FRV1-R1-009).
    NegativeTimestamp {
        /// The offending field.
        field: &'static str,
    },
    /// A numeric field carried a NaN or infinite value — not an empty
    /// field, a non-finite one (named separately: FRV1-R7-007).
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
    },
    /// The declared SDA cap was not in (0, 100].
    InvalidSdaCap {
        declared_percent: f64,
    },
}

impl std::fmt::Display for BaselineEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaselineEngineError::EmptyField { field } => {
                write!(f, "the baseline-engine field '{field}' must be non-empty")
            }
            BaselineEngineError::NegativeTimestamp { field } => {
                write!(f, "the baseline-engine field '{field}' must be non-negative")
            }
            BaselineEngineError::NonFiniteValue { field } => {
                write!(f, "the baseline-engine field '{field}' must be finite")
            }
            BaselineEngineError::InvalidSdaCap { declared_percent } => write!(
                f,
                "the declared SDA cap {declared_percent}% is not in (0, 100]"
            ),
        }
    }
}

impl std::error::Error for BaselineEngineError {}

// --- Configuration (the program's, never FlexCore's) ----------------------------

/// The day-matching / CBL selection rule, as the program declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineSelectionRule {
    /// The N highest-usage eligible days (N ≤ the recency window for the
    /// CBL methods).
    HighestN {
        n: usize,
    },
    /// All eligible (filtered) days.
    All,
}

/// The day-type filter: candidate days must declare the SAME day type
/// (exact equality — no fuzzy day-type matching).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayTypeFilter {
    pub day_type: String,
}

/// The weather-index filter: candidate days must carry a weather index
/// within the declared symmetric tolerance of the event day's index
/// (|candidate − event| ≤ tolerance; days without an index are NOT
/// matched — a missing index never silently passes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherIndexFilter {
    pub event_day_index: f64,
    pub tolerance: f64,
}

/// The program's SDA configuration: the symmetric cap in percent. The cap
/// is refused against the program's OWN recorded jurisdiction bound
/// (`ProgramBaselineConfig::jurisdiction_sda_max_percent`; for a CAISO
/// pack that bound is `CAISO_SDA_MAX_PERCENT`, ≤20% [S-30]); exceeding
/// the recorded jurisdiction bound is a RECORDED GAP, never a widened
/// cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdaConfig {
    pub cap_percent: f64,
}

impl SdaConfig {
    /// Builds the SDA configuration (cap in (0, 100]).
    pub fn new(cap_percent: f64) -> Result<Self, BaselineEngineError> {
        if !cap_percent.is_finite() || cap_percent <= 0.0 || cap_percent > 100.0 {
            return Err(BaselineEngineError::InvalidSdaCap { declared_percent: cap_percent });
        }
        Ok(Self { cap_percent })
    }
}

/// The program's baseline configuration, as declared (LR-073: every
/// field is the program's data).
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramBaselineConfig {
    pub tenant_id: String,
    pub program_id: String,
    /// The declared method name, as the program wrote it (the engine
    /// recognizes only `SUPPORTED_METHOD_NAMES`).
    pub method_name: String,
    pub day_type_filter: Option<DayTypeFilter>,
    pub weather_filter: Option<WeatherIndexFilter>,
    pub sda: Option<SdaConfig>,
    /// The program's jurisdiction's RECORDED symmetric-SDA bound in
    /// percent, as declared data (read by the caller from the jurisdiction
    /// record pack; for a CAISO pack it is `CAISO_SDA_MAX_PERCENT` [S-30]).
    /// `None` = the jurisdiction carries NO recorded bound — a program
    /// declaring an SDA under it is a RECORDED GAP, never an assumed
    /// default (FRV1-R7-005).
    pub jurisdiction_sda_max_percent: Option<f64>,
    /// The declared selection rule (required for `DAY_MATCHING`; the
    /// CBL methods derive theirs from the method name).
    pub selection: Option<BaselineSelectionRule>,
}

impl ProgramBaselineConfig {
    /// Builds the configuration (non-empty identities and method name).
    pub fn new(
        tenant_id: String,
        program_id: String,
        method_name: String,
    ) -> Result<Self, BaselineEngineError> {
        for (field, value) in [
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("method_name", method_name.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(BaselineEngineError::EmptyField { field });
            }
        }
        Ok(Self {
            tenant_id,
            program_id,
            method_name,
            day_type_filter: None,
            weather_filter: None,
            sda: None,
            jurisdiction_sda_max_percent: None,
            selection: None,
        })
    }
}

// --- Inputs ---------------------------------------------------------------------

/// One eligible day's profile. `is_event_day` marks the day the event
/// falls on (excluded from every CBL/day-matching candidate set — a
/// baseline is built from NON-event days only [S-30]); `weather_index`
/// is the day's declared weather index (`None` = not declared — such
/// days never match a weather filter, never silently pass one).
#[derive(Debug, Clone, PartialEq)]
pub struct DayProfile {
    pub day_key: String,
    pub day_type: Option<String>,
    pub weather_index: Option<f64>,
    /// The day's interval consumption magnitudes in MW (one value per
    /// interval; P > 0 = export convention does NOT apply here — these
    /// are consumption magnitudes as the program meters them).
    pub interval_values_mw: Vec<f64>,
    pub is_event_day: bool,
}

impl DayProfile {
    /// Builds one day profile (non-empty key; finite interval values).
    pub fn new(
        day_key: String,
        day_type: Option<String>,
        weather_index: Option<f64>,
        interval_values_mw: Vec<f64>,
        is_event_day: bool,
    ) -> Result<Self, BaselineEngineError> {
        if day_key.trim().is_empty() {
            return Err(BaselineEngineError::EmptyField { field: "day_key" });
        }
        if interval_values_mw.iter().any(|v| !v.is_finite()) {
            return Err(BaselineEngineError::NonFiniteValue {
                field: "interval_values_mw",
            });
        }
        Ok(Self {
            day_key,
            day_type,
            weather_index,
            interval_values_mw,
            is_event_day,
        })
    }

    /// The day's total usage (the highest-N selection metric).
    fn total_mw(&self) -> f64 {
        self.interval_values_mw.iter().sum()
    }
}

/// The SDA same-day adjustment input: the declared morning-window
/// actuals (the same interval grid as the baseline), recorded from the
/// day-of telemetry. An SDA without this window is a RECORDED GAP
/// (`MissingSdaWindow`), never a skipped adjustment.
#[derive(Debug, Clone, PartialEq)]
pub struct SdaWindowInput {
    /// The half-open interval-index window [start, end) the adjustment
    /// is computed over (the program declares the window).
    pub window_interval_indices: (usize, usize),
    /// The same-day ACTUAL consumption magnitudes for the window
    /// (length must equal window size).
    pub actual_mw: Vec<f64>,
}

impl SdaWindowInput {
    /// Builds the SDA window input (finite values; window size matches).
    pub fn new(window_interval_indices: (usize, usize), actual_mw: Vec<f64>) -> Result<Self, BaselineEngineError> {
        let (start, end) = window_interval_indices;
        if start >= end {
            return Err(BaselineEngineError::EmptyField {
                field: "window_interval_indices",
            });
        }
        if actual_mw.len() != end - start {
            return Err(BaselineEngineError::EmptyField { field: "actual_mw" });
        }
        if actual_mw.iter().any(|v| !v.is_finite()) {
            return Err(BaselineEngineError::EmptyField { field: "actual_mw" });
        }
        Ok(Self {
            window_interval_indices,
            actual_mw,
        })
    }
}

// --- Outputs ----------------------------------------------------------------------

/// The recorded SDA application (both ratios recorded — the adjustment
/// is capped, never clipped into silence).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdaApplication {
    /// The UNCAPPED symmetric adjustment ratio (raw, as measured).
    pub raw_ratio: f64,
    /// The APPLIED ratio after the symmetric cap.
    pub applied_ratio: f64,
    /// The cap that was applied (percent).
    pub cap_percent: f64,
}

/// The computed baseline profile: the program's method name recorded
/// (LR-073 — the method stays the program's, even on the output), the
/// per-interval values, the source-day keys in deterministic order, and
/// the SDA application if one was applied.
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineProfile {
    pub tenant_id: String,
    pub program_id: String,
    /// The declared method name (as the program wrote it).
    pub method_name: String,
    /// The per-interval baseline values in MW (the event window's grid).
    pub interval_values_mw: Vec<f64>,
    /// The source-day keys, ascending (deterministic order).
    pub source_day_keys: Vec<String>,
    pub sda_applied: Option<SdaApplication>,
    /// When the profile was computed (the INJECTED clock's instant).
    pub computed_at_unix_ms: i64,
}

/// The engine's outcome: a computed profile OR a recorded gap row —
/// never a silent approximation (LR-073).
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineOutcome {
    Computed(BaselineProfile),
    Gap(BaselineGapRecord),
}

// --- The engine ---------------------------------------------------------------------

/// Filters the eligible candidate days: non-event days matching the
/// declared day-type filter (exact equality) and weather-index filter
/// (|candidate − event| ≤ tolerance; days without an index never match).
/// Deterministic: the input BTreeMap is already day-key ordered.
fn filter_candidates<'a>(
    config: &ProgramBaselineConfig,
    eligible_days: &'a BTreeMap<String, DayProfile>,
) -> Vec<&'a DayProfile> {
    eligible_days
        .values()
        .filter(|day| !day.is_event_day)
        .filter(|day| match &config.day_type_filter {
            Some(filter) => day.day_type.as_deref() == Some(filter.day_type.as_str()),
            None => true,
        })
        .filter(|day| match &config.weather_filter {
            Some(filter) => match day.weather_index {
                Some(index) if index.is_finite() && filter.tolerance >= 0.0 => {
                    (index - filter.event_day_index).abs() <= filter.tolerance
                }
                _ => false,
            },
            None => true,
        })
        .collect()
}

/// Applies the method's day selection to the filtered candidates:
/// for the CBL methods, the LAST `CBL_RECENCY_WINDOW_DAYS` candidates by
/// day key first (the most recent eligible days), then the highest-N
/// selection with explicit tie-breaking (total descending, then day key
/// ascending); for day-matching, the program's declared rule over the
/// full filtered set.
fn select_days(
    config: &ProgramBaselineConfig,
    candidates: Vec<&DayProfile>,
) -> Result<Vec<&DayProfile>, BaselineGapReason> {
    match config.method_name.as_str() {
        "CBL_10_IN_10" => highest_n_after_recency_window(candidates, 10),
        "CBL_5_IN_10" => highest_n_after_recency_window(candidates, 5),
        "DAY_MATCHING" => {
            // The program declares the selection rule; an absent rule is
            // `All` (the match set is the selection — the program named
            // the days through its filters).
            match config.selection {
                Some(BaselineSelectionRule::HighestN { n }) => {
                    if n == 0 || n > candidates.len() {
                        return Err(BaselineGapReason::InsufficientEligibleDays {
                            required: n,
                            found: candidates.len(),
                        });
                    }
                    Ok(highest_n(candidates, n))
                }
                Some(BaselineSelectionRule::All) | None => Ok(candidates),
            }
        }
        // The caller checks support first; this arm is unreachable in
        // the supported flow but stays honest (a named gap, not a panic).
        _ => Err(BaselineGapReason::UnsupportedMethod),
    }
}

fn highest_n_after_recency_window(
    mut candidates: Vec<&DayProfile>,
    n: usize,
) -> Result<Vec<&DayProfile>, BaselineGapReason> {
    // The recency window: the LAST ten eligible candidates by day key
    // (the BTreeMap iteration is day-key ascending, so the tail is the
    // most recent).
    let window_start = candidates.len().saturating_sub(CBL_RECENCY_WINDOW_DAYS);
    let windowed: Vec<&DayProfile> = candidates.drain(window_start..).collect();
    if windowed.len() < n {
        return Err(BaselineGapReason::InsufficientEligibleDays {
            required: n,
            found: windowed.len(),
        });
    }
    Ok(highest_n(windowed, n))
}

/// The highest-N selection with EXPLICIT tie-breaking: total
/// descending, then day key ascending (deterministic — equal totals
/// resolve by key order, never by input order or hash).
fn highest_n(mut days: Vec<&DayProfile>, n: usize) -> Vec<&DayProfile> {
    days.sort_by(|a, b| {
        b.total_mw()
            .partial_cmp(&a.total_mw())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.day_key.cmp(&b.day_key))
    });
    days.truncate(n);
    days
}

/// Computes the program's baseline per LR-073: the method comes from the
/// program's configuration; the engine computes only the closed
/// supported set and records a gap row for everything else. Time enters
/// only through the injected clock.
///
/// `event_interval_count` is the event window's interval count — every
/// selected day profile must match it. `sda_window` is the same-day
/// adjustment input REQUIRED when the configuration declares an SDA.
pub fn compute_baseline(
    config: &ProgramBaselineConfig,
    eligible_days: &BTreeMap<String, DayProfile>,
    event_interval_count: usize,
    sda_window: Option<&SdaWindowInput>,
    clock: &dyn Clock,
) -> BaselineOutcome {
    let event_time = clock.now_event_time();
    let event_millis = event_time.unix_millis();
    // The contracts type carries i128; the records carry i64 — a value
    // outside i64 is a RECORDED GAP (ClockOutOfRange), never a silent
    // clamp. The gap's own timestamp falls back to 0 only on this
    // refused path; the record itself is still named and reason-ed.
    if event_millis < 0 || event_millis > i64::MAX as i128 {
        return BaselineOutcome::Gap(BaselineGapRecord {
            tenant_id: config.tenant_id.clone(),
            program_id: config.program_id.clone(),
            method_name: config.method_name.clone(),
            reason: BaselineGapReason::ClockOutOfRange,
            recorded_at_unix_ms: 0,
        });
    }
    let now_unix_ms = event_millis as i64;
    let gap = |reason: BaselineGapReason| BaselineOutcome::Gap(BaselineGapRecord {
        tenant_id: config.tenant_id.clone(),
        program_id: config.program_id.clone(),
        method_name: config.method_name.clone(),
        reason,
        recorded_at_unix_ms: now_unix_ms,
    });

    // LR-073: the method is the program's. Anything outside the closed
    // supported set is a RECORDED GAP, never an approximation.
    if !SUPPORTED_METHOD_NAMES.contains(&config.method_name.as_str()) {
        return gap(BaselineGapReason::UnsupportedMethod);
    }

    // The SDA configuration is validated before anything else: the cap is
    // refused against the program's OWN recorded jurisdiction bound — the
    // bound is declared data carried on the configuration (read from the
    // jurisdiction record pack; for a CAISO pack that recorded bound is
    // `CAISO_SDA_MAX_PERCENT` ≤20% [S-30]). A jurisdiction with NO
    // recorded bound refuses an SDA outright — never an assumed default
    // (FRV1-R7-005).
    if let Some(sda) = &config.sda {
        match config.jurisdiction_sda_max_percent {
            Some(bound) => {
                if sda.cap_percent > bound {
                    return gap(BaselineGapReason::SdaCapExceedsJurisdictionBound {
                        declared_percent: sda.cap_percent,
                        jurisdiction_bound_percent: bound,
                    });
                }
            }
            None => return gap(BaselineGapReason::SdaJurisdictionBoundUnrecorded),
        }
        if sda_window.is_none() {
            return gap(BaselineGapReason::MissingSdaWindow);
        }
        // The window must fit the event window's interval grid — an
        // adjustment is never computed against indices that do not
        // exist (checked before the grid is indexed).
        if let Some(window) = sda_window {
            let (start, end) = window.window_interval_indices;
            if end > event_interval_count || start >= end {
                return gap(BaselineGapReason::InvalidSdaWindow);
            }
        }
    }

    let candidates = filter_candidates(config, eligible_days);
    let selected = match select_days(config, candidates) {
        Ok(selected) => selected,
        Err(reason) => return gap(reason),
    };
    if selected.is_empty() {
        return gap(BaselineGapReason::InsufficientEligibleDays {
            required: 1,
            found: 0,
        });
    }
    for day in &selected {
        if day.interval_values_mw.len() != event_interval_count {
            return gap(BaselineGapReason::IntervalCountMismatch {
                day_key: day.day_key.clone(),
                expected: event_interval_count,
                found: day.interval_values_mw.len(),
            });
        }
    }

    // The CBL average: per-interval mean over the selected days.
    let mut baseline = vec![0.0_f64; event_interval_count];
    for day in &selected {
        for (interval, value) in day.interval_values_mw.iter().enumerate() {
            baseline[interval] += value;
        }
    }
    let count = selected.len() as f64;
    for value in &mut baseline {
        *value /= count;
    }

    // The capped symmetric SDA (declared by the program; the recorded
    // jurisdiction bound, enforced above). Both ratios are recorded — the
    // cap is applied, never clipped into silence. A baseline reference
    // window summing to zero or less has NO representable adjustment
    // ratio — a RECORDED GAP, never a silent no-op "applied" adjustment
    // (FRV1-R7-004).
    let sda_applied = match (&config.sda, sda_window) {
        (Some(sda), Some(window)) => {
            let (start, end) = window.window_interval_indices;
            let baseline_window_sum: f64 = baseline[start..end].iter().sum();
            let actual_window_sum: f64 = window.actual_mw.iter().sum();
            if baseline_window_sum <= 0.0 {
                return gap(BaselineGapReason::SdaReferenceWindowZero);
            }
            let raw_ratio = (actual_window_sum - baseline_window_sum) / baseline_window_sum;
            let cap = sda.cap_percent / 100.0;
            let applied_ratio = raw_ratio.clamp(-cap, cap);
            for value in &mut baseline {
                *value *= 1.0 + applied_ratio;
            }
            Some(SdaApplication {
                raw_ratio,
                applied_ratio,
                cap_percent: sda.cap_percent,
            })
        }
        _ => None,
    };

    BaselineOutcome::Computed(BaselineProfile {
        tenant_id: config.tenant_id.clone(),
        program_id: config.program_id.clone(),
        method_name: config.method_name.clone(),
        interval_values_mw: baseline,
        source_day_keys: selected
            .iter()
            .map(|day| day.day_key.clone())
            .collect(),
        sda_applied,
        computed_at_unix_ms: now_unix_ms,
    })
}
