// P2.08 — Data-quality assignment and stale detection (pure rules, no I/O).
//
// Implements `ledger/phase-02-asset-telemetry/ledger_phase_2.md` P2.08, with
// `ledger/phase-00-product-freeze/08_telemetry_quality_vocabulary.md` (P0.08)
// as the test oracle and `04_telemetry_model.md` (P2.04, D-T4/D-T6) for the
// record conventions. Normative sources: report §10.4 (quality vocabulary),
// §17.4 (stale-data gates); foundations §6.4 governs on conflict.
//
// THE SUSPECT-VS-BAD BOUNDARY, DECIDED ONCE (P2.08 step 2):
//
// **BAD** — a *hard validation* failure: the record is not usable as a
// measurement at all.
//
// - schema/type-level validation failure (record shape, required fields,
//   value-kind field presence, non-finite numeric value);
// - the SOC domain rule on the raw pre-construction path: a raw `f64`
//   fraction outside [0,1] is a hard failure → BAD (note: when the typed
//   `StateOfCharge` contract type is used, this failure cannot occur —
//   construction rejects it; the rule exists for the raw validation path
//   before the type is constructed);
// - the source itself reports the value invalid.
//
// **SUSPECT** — the value was received, parsed, and is well-formed, but a
// *plausibility, range, or consistency check* failed on it:
//
// - range failure on a well-formed value (e.g. active power outside the
//   asset's rated limits);
// - consistency/plausibility failure (e.g. a BESS discharging while its SOC
//   is 0).
//
// The boundary in one sentence: **if the value cannot be interpreted as a
// measurement of the declared quantity at all, it is BAD; if it can be
// interpreted but fails a physics/rating plausibility check, it is
// SUSPECT.** BAD values are quarantined by the pipeline (P2.07/P2.10) and
// never stored as GOOD (non-silencing, P0.08 §4); SUSPECT values are stored
// with their SUSPECT flag carried, never silently downgraded or dropped.
//
// Producer separation, enforced by construction (P0.08 §1–§2, one state per
// value):
//
// - [`assign_quality`] can only ever return GOOD, SUSPECT, or BAD — the
//   three arms of its body admit no other state, and [`producer_of`] below
//   pins this in a machine-checked exhaustive match.
// - STALE is produced only by [`staleness`] (age computed from `event_time`,
//   never from ingest time — the P2.08 risk note; the two are distinct
//   contract types so the substitution is a compile error).
// - MISSING is produced only by [`expected_intervals`] (the gap detector,
//   not the ingestion of a value), as derived records under the reserved
//   `flexcore:derived:` source namespace (P2.04 D-T6).
// - ESTIMATED is produced **only** by [`derive_estimated`], which cannot be
//   called without a [`DerivationRule`] argument. Phase 2 derives nothing by
//   default: with no derivation rule configured, no code path in this module
//   returns ESTIMATED (a test pins this). This is the foundations §6.4 rule:
//   estimation is explicit, tagged, and bounded by policy.
//
// The adapter-boundary source-state mapping (`map_source_state`, P0.08 §3)
// already lives in flexcore-contracts (P1.05) and happens *before* this
// module; its outputs are surfaced upstream as check outcomes, and the
// SUBSTITUTED → ESTIMATED mapping can never leak through here — ESTIMATED
// requires an explicit [`DerivationRule`] in this module, period.
//
// Freshness limits and cadence are **parameters supplied by the caller**
// (per use case — the current-state and historical views define their own
// limits in P2.18/P2.19; there is no universal stale timeout, foundations
// §6.4). No freshness constant is hard-coded in this module.

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, StateOfCharge};

/// Reserved platform namespace for derived-record source IDs (P2.04 D-T6).
/// Measured records must never use the `flexcore:` source namespace.
pub const DERIVED_SOURCE_NAMESPACE: &str = "flexcore:";

/// The reserved prefix for platform-derived records (P2.04 D-T6), the
/// namespace plus the `derived` marker: `flexcore:derived:<detector>`.
pub const DERIVED_SOURCE_PREFIX: &str = "flexcore:derived:";

/// Errors returned by the validating constructors of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityRuleError {
    /// A freshness limit or cadence must be a positive number of
    /// milliseconds; a non-positive value would make every value stale (or
    /// every interval a gap) by construction.
    NonPositiveMillis,
    /// Rated limits are inverted or non-finite.
    InvalidRatedLimits,
    /// A derivation rule must carry a non-empty rule ID.
    EmptyRuleId,
    /// A detector name (for the `flexcore:derived:<detector>` namespace,
    /// P2.04 D-T6) must be non-empty and must not itself contain the
    /// namespace separator.
    EmptyDetectorName,
}

impl std::fmt::Display for QualityRuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityRuleError::NonPositiveMillis => {
                write!(f, "freshness limits and cadence must be positive millisecond values")
            }
            QualityRuleError::InvalidRatedLimits => {
                write!(f, "rated limits must be finite and lower <= upper")
            }
            QualityRuleError::EmptyRuleId => {
                write!(f, "a derivation rule must carry a non-empty rule id")
            }
            QualityRuleError::EmptyDetectorName => {
                write!(f, "a detector name must be non-empty")
            }
        }
    }
}

impl std::error::Error for QualityRuleError {}

// --- Reason codes and the decision record -----------------------------------

/// Machine-readable reason code carried by every [`QualityDecision`].
///
/// The reason code is the audit-facing evidence classifier: it names *which*
/// rule produced the state, so a quarantine record or a
/// `TelemetryQualityChanged` event can state its cause without re-deriving
/// it (non-silencing, P0.08 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityReason {
    /// Every configured check passed; the value is GOOD.
    PassedAllChecks,
    /// Schema/type-level hard validation failed → BAD.
    HardSchemaValidationFailure,
    /// Raw-f64 SOC fraction outside [0,1] on the pre-construction path → BAD.
    /// (The typed `StateOfCharge` makes this impossible; the rule exists for
    /// the raw validation path, P2.08 step 1.)
    HardSocDomainFailure,
    /// The source itself reported the value invalid → BAD.
    SourceReportedInvalid,
    /// Range failure on a well-formed value (e.g. P outside rated limits)
    /// → SUSPECT.
    RangeFailure,
    /// Consistency/plausibility failure (e.g. BESS discharging at SOC 0)
    /// → SUSPECT.
    ConsistencyFailure,
    /// A gap marker: no value was received for the expected interval →
    /// MISSING, produced by the gap detector as a derived record.
    DerivedGapMarker,
    /// A value produced by an explicitly configured derivation rule →
    /// ESTIMATED. Cannot arise without a [`DerivationRule`].
    DerivedEstimation,
    /// The last known value's age exceeded the use-case freshness limit →
    /// STALE, produced by the staleness detector.
    FreshnessExceeded,
}

/// The outcome of one quality-assignment decision (or of the explicit
/// derivation path): the canonical state plus its reason code and an
/// evidence note (P0.08 §4 — the flag and its cause travel together).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityDecision {
    /// Exactly one canonical quality state (P0.08 §2).
    pub quality: TelemetryQuality,
    /// Which rule produced the state.
    pub reason: QualityReason,
    /// Human-readable evidence note (what failed, and on which value).
    pub evidence: String,
}

/// Outcome of one plausibility/range/consistency check supplied to
/// [`assign_quality`]. These are *soft* checks: a failure yields SUSPECT,
/// never BAD (the SUSPECT-vs-BAD boundary above).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CheckResult {
    /// The check does not apply to this quantity/record (the default).
    #[default]
    NotApplicable,
    /// The check ran and passed.
    Pass,
    /// The check ran and failed on an otherwise well-formed value → SUSPECT.
    Fail {
        /// Short detail of what failed (e.g. `"P=+2.400 MW above rated +2.000 MW"`).
        detail: &'static str,
    },
}

/// The inputs to [`assign_quality`]: the outcomes of the hard validation
/// stage and of the soft plausibility checks, already computed by the
/// pipeline (this function is the *rule*, not the checker — it stays pure
/// and I/O-free, P2.08 step 1).
#[derive(Debug, Clone, Default)]
pub struct QualityInput {
    /// Schema/type-level validation outcome (record shape, required fields,
    /// value-kind field presence, numeric finiteness). `false` → BAD.
    pub schema_valid: bool,
    /// True when the source itself reports the value invalid → BAD.
    pub source_reported_invalid: bool,
    /// The SOC value on the raw pre-construction path (`None` when the
    /// quantity is not SOC, or when the typed `StateOfCharge` was already
    /// constructed — the type makes the domain failure impossible). A raw
    /// fraction outside [0,1] or non-finite → BAD.
    pub soc_raw_fraction: Option<f64>,
    /// Rated-limits range check on the well-formed value → SUSPECT on fail.
    pub range_check: CheckResult,
    /// Consistency/plausibility check on the well-formed value → SUSPECT on
    /// fail.
    pub consistency_check: CheckResult,
}

/// Assigns exactly one canonical quality state to one telemetry value
/// (P0.08 §2 one-state-per-value; P2.08 step 1).
///
/// Decision order (first failure wins, so a record cannot be both BAD and
/// SUSPECT):
///
/// 1. schema/type hard validation failure → BAD;
/// 2. raw SOC fraction outside [0,1] (pre-construction path) → BAD;
/// 3. source-reported invalid → BAD;
/// 4. range check failure → SUSPECT;
/// 5. consistency check failure → SUSPECT;
/// 6. otherwise → GOOD.
///
/// By construction the return is always one of GOOD/SUSPECT/BAD — never
/// MISSING, ESTIMATED, or STALE (see [`producer_of`]).
pub fn assign_quality(input: &QualityInput) -> QualityDecision {
    if !input.schema_valid {
        return QualityDecision {
            quality: TelemetryQuality::Bad,
            reason: QualityReason::HardSchemaValidationFailure,
            evidence: "schema/type-level validation failed (hard validation rule)".to_string(),
        };
    }
    if let Some(raw_soc) = input.soc_raw_fraction {
        // Raw pre-construction path: the [0,1] domain rule is a HARD failure
        // (P2.08 step 2). The typed StateOfCharge rejects the same values at
        // construction, so the two paths agree.
        if !raw_soc.is_finite() || !(0.0..=1.0).contains(&raw_soc) {
            return QualityDecision {
                quality: TelemetryQuality::Bad,
                reason: QualityReason::HardSocDomainFailure,
                evidence: format!(
                    "raw SOC fraction {raw_soc} outside the [0,1] domain (hard validation rule)"
                ),
            };
        }
    }
    if input.source_reported_invalid {
        return QualityDecision {
            quality: TelemetryQuality::Bad,
            reason: QualityReason::SourceReportedInvalid,
            evidence: "source reported the value invalid".to_string(),
        };
    }
    if let CheckResult::Fail { detail } = input.range_check {
        return QualityDecision {
            quality: TelemetryQuality::Suspect,
            reason: QualityReason::RangeFailure,
            evidence: format!("range check failed on a well-formed value: {detail}"),
        };
    }
    if let CheckResult::Fail { detail } = input.consistency_check {
        return QualityDecision {
            quality: TelemetryQuality::Suspect,
            reason: QualityReason::ConsistencyFailure,
            evidence: format!("consistency check failed on a well-formed value: {detail}"),
        };
    }
    QualityDecision {
        quality: TelemetryQuality::Good,
        reason: QualityReason::PassedAllChecks,
        evidence: "schema valid, source did not report invalid, all configured checks passed"
            .to_string(),
    }
}

// --- Soft check helpers (pure; feed QualityInput) ----------------------------

/// Rated active-power limits in MW, on the canonical sign convention
/// (P0.07: `P > 0` = export at the point of connection). A value inside
/// `[lower, upper]` is within rating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatedLimits {
    /// Lower rated limit (MW; typically the largest import magnitude, ≤ 0).
    pub lower_mw: f64,
    /// Upper rated limit (MW; typically the rated export, ≥ 0).
    pub upper_mw: f64,
}

impl RatedLimits {
    /// Symmetric limits `[-rated, +rated]` (e.g. a BESS or site meter).
    pub fn symmetric(rated_mw: f64) -> Result<Self, QualityRuleError> {
        Self::new(-rated_mw, rated_mw)
    }

    /// Validates and constructs limits: both finite, `lower <= upper`.
    pub fn new(lower_mw: f64, upper_mw: f64) -> Result<Self, QualityRuleError> {
        if !lower_mw.is_finite() || !upper_mw.is_finite() || lower_mw > upper_mw {
            return Err(QualityRuleError::InvalidRatedLimits);
        }
        Ok(Self { lower_mw, upper_mw })
    }
}

/// Range check: active power inside the rated limits on a well-formed value
/// (a failure is SUSPECT, never BAD — P2.08 step 1).
pub fn check_active_power_range(p: ActivePowerMW, limits: RatedLimits) -> CheckResult {
    let v = p.value();
    if (limits.lower_mw..=limits.upper_mw).contains(&v) {
        CheckResult::Pass
    } else if v > limits.upper_mw {
        CheckResult::Fail {
            detail: "active power above the rated upper limit",
        }
    } else {
        CheckResult::Fail {
            detail: "active power below the rated lower limit",
        }
    }
}

/// Consistency check: a BESS reporting net discharge (`P > 0` = export,
/// P0.07 §6 `P_net = P_dis − P_ch`) while its SOC is 0 is physically
/// implausible → SUSPECT (P2.08 step 1). Charging at SOC 0 and any power at
/// SOC > 0 pass.
pub fn check_bess_power_soc_consistency(p_net: ActivePowerMW, soc: StateOfCharge) -> CheckResult {
    if p_net.value() > 0.0 && soc.value() <= 0.0 {
        CheckResult::Fail {
            detail: "BESS discharging (P_net > 0) while SOC is 0",
        }
    } else {
        CheckResult::Pass
    }
}

// --- STALE detection ---------------------------------------------------------

/// A per-use-case freshness limit, supplied by the caller (configuration,
/// never a hard-coded constant — foundations §6.4, P2.08 step 3). The
/// current-state view and the historical view each pass their own limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessLimit {
    millis: i64,
}

impl FreshnessLimit {
    /// A positive limit in milliseconds.
    pub fn try_new(millis: i64) -> Result<Self, QualityRuleError> {
        if millis <= 0 {
            return Err(QualityRuleError::NonPositiveMillis);
        }
        Ok(Self { millis })
    }

    /// A positive limit in whole seconds.
    pub fn from_seconds(seconds: i64) -> Result<Self, QualityRuleError> {
        Self::try_new(seconds.checked_mul(1000).ok_or(QualityRuleError::NonPositiveMillis)?)
    }

    /// The limit in milliseconds.
    pub const fn millis(self) -> i64 {
        self.millis
    }
}

/// The staleness verdict for one value: STALE exactly when the value's age
/// **exceeds** the use-case freshness limit (`age > limit`), so
/// `age == limit` is **not** stale and `limit + 1s` is (the frozen boundary,
/// P2.08 acceptance criterion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    /// The age is within the freshness limit; the value keeps the quality
    /// state the assigner gave it (staleness does not invent GOOD).
    Fresh {
        /// Age in milliseconds, `<= limit`.
        age_millis: i64,
    },
    /// The age exceeds the freshness limit → the value is STALE.
    Stale {
        /// Age in milliseconds, `> limit`.
        age_millis: i64,
    },
}

impl Staleness {
    /// `Some(STALE)` when the verdict is stale; `None` when fresh (the
    /// value's assigned quality is unchanged — no silent re-labelling).
    pub const fn stale_quality(self) -> Option<TelemetryQuality> {
        match self {
            Staleness::Fresh { .. } => None,
            Staleness::Stale { .. } => Some(TelemetryQuality::Stale),
        }
    }

    /// Whether the verdict is stale.
    pub const fn is_stale(self) -> bool {
        matches!(self, Staleness::Stale { .. })
    }
}

/// Staleness detection with a per-use-case freshness limit (P2.08 step 3).
///
/// The age is computed **from `event_time`** (when the measurement occurred),
/// never from ingest time — both parameters are the same `EventTime` contract
/// type, so an ingest-time substitution is a compile error (P0.09 §1; the
/// P2.08 risk note). A future-dated `event_time` yields a negative age and is
/// therefore Fresh here: clock skew is a separate fault handled upstream
/// (SUSPECT per the P2.14 clock-fault path), not staleness.
pub fn staleness(
    event_time: EventTime,
    now_event_time: EventTime,
    freshness_limit: FreshnessLimit,
) -> Staleness {
    let age_millis = (now_event_time.unix_millis() - event_time.unix_millis()) as i64;
    if age_millis > freshness_limit.millis {
        Staleness::Stale { age_millis }
    } else {
        Staleness::Fresh { age_millis }
    }
}

// --- MISSING gap detection ---------------------------------------------------

/// Whether a source ID belongs to the reserved platform-derived namespace
/// (`flexcore:derived:`, P2.04 D-T6). Measured records must never use it.
pub fn is_derived_source_id(source_id: &str) -> bool {
    source_id.starts_with(DERIVED_SOURCE_PREFIX)
}

/// Builds a derived-record source ID for a detector: `flexcore:derived:<detector>`
/// (P2.04 D-T6).
pub fn derived_source_id(detector: &str) -> Result<String, QualityRuleError> {
    if detector.is_empty() {
        return Err(QualityRuleError::EmptyDetectorName);
    }
    Ok(format!("{DERIVED_SOURCE_PREFIX}{detector}"))
}

/// A MISSING marker record for one expected interval with no received value
/// (P0.08 §1 MISSING; P2.08 step 4).
///
/// Distinguishable from a measured value **by construction**:
///
/// - `source_id` is always under the reserved `flexcore:derived:` namespace
///   (P2.04 D-T6), which measured records may never use;
/// - `quality` is always MISSING — this type carries **no value field at
///   all** (the canonical-record totality rule: all value fields are NULL
///   exactly when quality is MISSING, P2.04 §3);
/// - `source_sequence` is detector-owned and monotonic, drawn from a base
///   supplied by the caller (a real source's sequence space is never reused).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingGap {
    /// `flexcore:derived:<detector>` (D-T6).
    pub source_id: String,
    /// Detector-owned monotonic sequence (`sequence_base + i`).
    pub source_sequence: u64,
    /// The `event_time` of the expected-but-missing interval.
    pub event_time: EventTime,
    /// Always MISSING for a gap marker.
    pub quality: TelemetryQuality,
}

/// The expected-interval cadence: the gap detector's grid step, supplied by
/// the caller from the asset's telemetry configuration (the simulator's
/// cadence in Phase 2 — P2.08 step 3). Never a hard-coded constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadence {
    millis: i64,
}

impl Cadence {
    /// A positive cadence in milliseconds.
    pub fn try_new(millis: i64) -> Result<Self, QualityRuleError> {
        if millis <= 0 {
            return Err(QualityRuleError::NonPositiveMillis);
        }
        Ok(Self { millis })
    }

    /// A positive cadence in whole seconds.
    pub fn from_seconds(seconds: i64) -> Result<Self, QualityRuleError> {
        Self::try_new(seconds.checked_mul(1000).ok_or(QualityRuleError::NonPositiveMillis)?)
    }

    /// The cadence in milliseconds.
    pub const fn millis(self) -> i64 {
        self.millis
    }
}

/// Gap detection (P2.08 step 4): every expected interval in
/// `[horizon_start, horizon_end]` on the cadence grid with no received value
/// produces a [`MissingGap`] MISSING marker.
///
/// Expected intervals are `horizon_start + k * cadence` while `<= horizon_end`
/// (`k >= 0`). A covered time matches exactly (same `event_time` instant).
/// An inverted horizon yields no gaps. The caller supplies the detector name
/// (D-T6 namespace) and the base of the detector-owned sequence space; this
/// function stays pure (no clock read, no I/O — the horizon is an argument).
pub fn expected_intervals(
    covered: &[EventTime],
    horizon_start: EventTime,
    horizon_end: EventTime,
    cadence: Cadence,
    detector: &str,
    sequence_base: u64,
) -> Result<Vec<MissingGap>, QualityRuleError> {
    let source_id = derived_source_id(detector)?;
    let start_millis = horizon_start.unix_millis();
    let end_millis = horizon_end.unix_millis();
    let cadence_millis = i128::from(cadence.millis);
    let covered_millis: Vec<i128> = covered.iter().map(|t| t.unix_millis()).collect();
    let mut gaps = Vec::new();
    let mut sequence = sequence_base;
    let mut k: i128 = 0;
    loop {
        let expected = start_millis + k * cadence_millis;
        if expected > end_millis {
            break;
        }
        if !covered_millis.contains(&expected) {
            let event_time = EventTime::from_unix_millis(expected as i64)
                .expect("grid point lies within the supplied horizon, hence representable");
            gaps.push(MissingGap {
                source_id: source_id.clone(),
                source_sequence: sequence,
                event_time,
                quality: TelemetryQuality::Missing,
            });
            sequence += 1;
        }
        k += 1;
    }
    Ok(gaps)
}

// --- ESTIMATED: only via an explicit derivation rule --------------------------

/// An explicitly configured derivation rule. Its existence is the *only*
/// licence to produce ESTIMATED values (P2.08 step 1: ESTIMATED appears only
/// if a derivation is explicitly configured and recorded; foundations §6.4:
/// estimation is explicit, tagged, bounded by policy). Phase 2 configures
/// none — no caller constructs this, so no ESTIMATED value arises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationRule {
    /// The recorded rule identifier (audit-facing).
    pub rule_id: &'static str,
    /// The detector name owning the derived records (D-T6 namespace).
    pub detector: &'static str,
}

impl DerivationRule {
    /// Validates and constructs a derivation rule (non-empty ID and
    /// detector name).
    pub fn new(rule_id: &'static str, detector: &'static str) -> Result<Self, QualityRuleError> {
        if rule_id.is_empty() {
            return Err(QualityRuleError::EmptyRuleId);
        }
        if detector.is_empty() {
            return Err(QualityRuleError::EmptyDetectorName);
        }
        Ok(Self { rule_id, detector })
    }
}

/// Produces an ESTIMATED decision **only** for a value derived under an
/// explicitly configured [`DerivationRule`] (P2.08 step 1). This function is
/// the single code path in this module that can yield
/// [`TelemetryQuality::Estimated`]; without it, and with no rule configured
/// in Phase 2, ESTIMATED is unreachable (a test pins this).
pub fn derive_estimated(rule: &DerivationRule, evidence: &str) -> QualityDecision {
    QualityDecision {
        quality: TelemetryQuality::Estimated,
        reason: QualityReason::DerivedEstimation,
        evidence: format!(
            "derived by explicitly configured rule '{}' (detector '{}'): {evidence}",
            rule.rule_id, rule.detector
        ),
    }
}

// --- TelemetryQualityChanged predicate ---------------------------------------

/// Pure predicate for the `TelemetryQualityChanged` outbox event trigger
/// (§15.3): true exactly when the canonical quality state of an asset's
/// value changed between the previous and the current assignment.
///
/// The event emission itself is P2.16's (via the outbox); this is only the
/// decision rule, kept here so there is one spelling of "changed". Quality
/// changes are facts, not commands — the predicate never mutates anything.
pub fn quality_state_changed(previous: TelemetryQuality, current: TelemetryQuality) -> bool {
    previous != current
}

// --- Producer separation (machine-checked) -----------------------------------

/// Which pipeline producer can emit each canonical state (P2.08 producer
/// separation). The exhaustive match below is part of the deliverable: it
/// must cover all six frozen states (P0.08 §1), and adding a seventh state
/// breaks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Producer {
    /// GOOD / SUSPECT / BAD — the per-value assigner.
    QualityAssigner,
    /// STALE — the staleness detector (event-time age vs the use-case limit).
    StalenessDetector,
    /// MISSING — the gap detector (derived records, D-T6 namespace).
    GapDetector,
    /// ESTIMATED — only an explicitly configured derivation rule.
    ExplicitDerivation,
}

/// The producer of each canonical state. Exhaustive over the six frozen
/// states by construction of [`assign_quality`], [`staleness`],
/// [`expected_intervals`], and [`derive_estimated`].
pub const fn producer_of(q: TelemetryQuality) -> Producer {
    match q {
        TelemetryQuality::Good => Producer::QualityAssigner,
        TelemetryQuality::Suspect => Producer::QualityAssigner,
        TelemetryQuality::Bad => Producer::QualityAssigner,
        TelemetryQuality::Missing => Producer::GapDetector,
        TelemetryQuality::Estimated => Producer::ExplicitDerivation,
        TelemetryQuality::Stale => Producer::StalenessDetector,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT_MS: i64 = 1_700_000_000_000;

    fn event_at(offset_millis: i64) -> EventTime {
        EventTime::from_unix_millis(EVENT_MS + offset_millis).unwrap()
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).unwrap()
    }

    fn clean_input() -> QualityInput {
        QualityInput {
            schema_valid: true,
            source_reported_invalid: false,
            soc_raw_fraction: None,
            range_check: CheckResult::Pass,
            consistency_check: CheckResult::Pass,
        }
    }

    // --- assign_quality: pass and fail case per rule -------------------------

    #[test]
    fn clean_input_is_good() {
        let d = assign_quality(&clean_input());
        assert_eq!(d.quality, TelemetryQuality::Good);
        assert_eq!(d.reason, QualityReason::PassedAllChecks);
        assert!(!d.evidence.is_empty());
    }

    #[test]
    fn source_reported_invalid_is_bad() {
        // Fail case.
        let mut input = clean_input();
        input.source_reported_invalid = true;
        let d = assign_quality(&input);
        assert_eq!(d.quality, TelemetryQuality::Bad);
        assert_eq!(d.reason, QualityReason::SourceReportedInvalid);
        // Pass case: the flag absent leaves the value GOOD.
        assert_eq!(assign_quality(&clean_input()).quality, TelemetryQuality::Good);
    }

    #[test]
    fn schema_type_hard_validation_failure_is_bad() {
        let mut input = clean_input();
        input.schema_valid = false;
        let d = assign_quality(&input);
        assert_eq!(d.quality, TelemetryQuality::Bad);
        assert_eq!(d.reason, QualityReason::HardSchemaValidationFailure);
        assert!(!d.evidence.is_empty());
    }

    #[test]
    fn raw_soc_outside_unit_interval_is_bad() {
        // Fail cases: below 0, above 1, and non-finite (NaN / infinity).
        for bad in [-0.1_f64, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut input = clean_input();
            input.soc_raw_fraction = Some(bad);
            let d = assign_quality(&input);
            assert_eq!(d.quality, TelemetryQuality::Bad, "raw SOC {bad}");
            assert_eq!(d.reason, QualityReason::HardSocDomainFailure, "raw SOC {bad}");
        }
        // Pass cases: the closed [0,1] boundaries and an interior value are
        // not hard failures (they do not make the value BAD).
        for good in [0.0_f64, 1.0, 0.5] {
            let mut input = clean_input();
            input.soc_raw_fraction = Some(good);
            let d = assign_quality(&input);
            assert_ne!(d.quality, TelemetryQuality::Bad, "raw SOC {good}");
        }
    }

    #[test]
    fn range_failure_on_well_formed_value_is_suspect() {
        // Fail case: P outside the rated limits.
        let limits = RatedLimits::symmetric(2.0).unwrap();
        let fail = check_active_power_range(ActivePowerMW::new(2.4).unwrap(), limits);
        assert_eq!(
            fail,
            CheckResult::Fail {
                detail: "active power above the rated upper limit"
            }
        );
        let mut input = clean_input();
        input.range_check = fail;
        let d = assign_quality(&input);
        assert_eq!(d.quality, TelemetryQuality::Suspect);
        assert_eq!(d.reason, QualityReason::RangeFailure);

        // Pass case: at and inside the limits.
        for v in [-2.0_f64, -1.0, 0.0, 1.0, 2.0] {
            let r = check_active_power_range(ActivePowerMW::new(v).unwrap(), limits);
            assert_eq!(r, CheckResult::Pass, "P={v}");
        }
    }

    #[test]
    fn consistency_failure_bess_discharging_at_zero_soc_is_suspect() {
        let zero_soc = StateOfCharge::new(0.0).unwrap();
        let mid_soc = StateOfCharge::new(0.5).unwrap();

        // Fail case: discharge (P > 0 = export) while SOC is 0 → SUSPECT.
        let fail = check_bess_power_soc_consistency(ActivePowerMW::new(1.2).unwrap(), zero_soc);
        assert_eq!(
            fail,
            CheckResult::Fail {
                detail: "BESS discharging (P_net > 0) while SOC is 0"
            }
        );
        let mut input = clean_input();
        input.consistency_check = fail;
        let d = assign_quality(&input);
        assert_eq!(d.quality, TelemetryQuality::Suspect);
        assert_eq!(d.reason, QualityReason::ConsistencyFailure);

        // Pass cases: charging at SOC 0; discharging at SOC > 0; idle.
        assert_eq!(
            check_bess_power_soc_consistency(ActivePowerMW::new(-1.2).unwrap(), zero_soc),
            CheckResult::Pass
        );
        assert_eq!(
            check_bess_power_soc_consistency(ActivePowerMW::new(1.2).unwrap(), mid_soc),
            CheckResult::Pass
        );
        assert_eq!(
            check_bess_power_soc_consistency(ActivePowerMW::new(0.0).unwrap(), zero_soc),
            CheckResult::Pass
        );
    }

    #[test]
    fn suspect_vs_bad_boundary_hard_failure_wins_over_soft_failure() {
        // A schema/type hard failure is BAD even when soft checks also fail.
        let mut input = clean_input();
        input.schema_valid = false;
        input.range_check = CheckResult::Fail {
            detail: "active power above the rated upper limit",
        };
        input.consistency_check = CheckResult::Fail {
            detail: "BESS discharging (P_net > 0) while SOC is 0",
        };
        input.source_reported_invalid = true;
        input.soc_raw_fraction = Some(1.5);
        let d = assign_quality(&input);
        assert_eq!(d.quality, TelemetryQuality::Bad);
        assert_eq!(d.reason, QualityReason::HardSchemaValidationFailure);

        // A soft failure never escalates to BAD.
        let mut soft_only = clean_input();
        soft_only.range_check = CheckResult::Fail {
            detail: "active power above the rated upper limit",
        };
        assert_eq!(assign_quality(&soft_only).quality, TelemetryQuality::Suspect);
        // And a hard failure never degrades to SUSPECT.
        let mut hard_only = clean_input();
        hard_only.source_reported_invalid = true;
        assert_eq!(assign_quality(&hard_only).quality, TelemetryQuality::Bad);
    }

    #[test]
    fn rated_limits_constructor_validates() {
        assert!(RatedLimits::symmetric(2.0).is_ok());
        assert_eq!(RatedLimits::new(2.0, -2.0), Err(QualityRuleError::InvalidRatedLimits));
        assert_eq!(
            RatedLimits::new(f64::NAN, 1.0),
            Err(QualityRuleError::InvalidRatedLimits)
        );
        assert_eq!(
            RatedLimits::new(f64::INFINITY, f64::INFINITY),
            Err(QualityRuleError::InvalidRatedLimits)
        );
    }

    // --- STALE boundary ------------------------------------------------------

    #[test]
    fn stale_boundary_age_equal_to_limit_is_not_stale() {
        let limit = FreshnessLimit::from_seconds(300).unwrap();
        // age == limit → NOT stale (the frozen boundary).
        let s = staleness(event_at(0), event_at(300_000), limit);
        assert_eq!(s, Staleness::Fresh { age_millis: 300_000 });
        assert!(!s.is_stale());
        assert_eq!(s.stale_quality(), None);
    }

    #[test]
    fn stale_boundary_one_second_past_limit_is_stale() {
        let limit = FreshnessLimit::from_seconds(300).unwrap();
        // limit + 1s → stale (the frozen boundary).
        let s = staleness(event_at(0), event_at(301_000), limit);
        assert_eq!(s, Staleness::Stale { age_millis: 301_000 });
        assert!(s.is_stale());
        assert_eq!(s.stale_quality(), Some(TelemetryQuality::Stale));
    }

    #[test]
    fn stale_well_inside_limit_is_fresh_and_older_is_stale() {
        let limit = FreshnessLimit::from_seconds(300).unwrap();
        assert_eq!(
            staleness(event_at(0), event_at(299_000), limit),
            Staleness::Fresh { age_millis: 299_000 }
        );
        assert!(staleness(event_at(0), event_at(600_000), limit).is_stale());
    }

    #[test]
    fn staleness_is_computed_from_event_time_not_ingest_time() {
        // Both parameters are the EventTime contract type; ingest time is a
        // different type, so this call site cannot be fed an ingest time.
        // Here: a record that arrived long after it occurred (huge ingest
        // delay) is judged on event-time age alone.
        let limit = FreshnessLimit::from_seconds(60).unwrap();
        let event = event_at(0);
        let now = event_at(61_000);
        assert!(staleness(event, now, limit).is_stale());
        // A future-dated event_time (clock skew) is not staleness.
        let future = event_at(120_000);
        let s = staleness(future, event_at(0), limit);
        assert_eq!(s, Staleness::Fresh { age_millis: -120_000 });
        assert!(!s.is_stale());
    }

    #[test]
    fn freshness_limit_is_a_per_use_case_parameter_with_no_constant() {
        // The same event pair is stale under a tight limit and fresh under a
        // loose one: the limit is the caller's configuration, never a
        // hard-coded constant.
        let event = event_at(0);
        let now = event_at(120_000);
        assert!(staleness(event, now, FreshnessLimit::from_seconds(60).unwrap()).is_stale());
        assert!(!staleness(event, now, FreshnessLimit::from_seconds(300).unwrap()).is_stale());
        assert_eq!(FreshnessLimit::try_new(0), Err(QualityRuleError::NonPositiveMillis));
        assert_eq!(FreshnessLimit::try_new(-1), Err(QualityRuleError::NonPositiveMillis));
        assert_eq!(Cadence::try_new(0), Err(QualityRuleError::NonPositiveMillis));
    }

    // --- MISSING gap detection -----------------------------------------------

    #[test]
    fn missing_gaps_are_derived_records_distinguishable_from_measured() {
        // Horizon 0s..300s at a 60s cadence → expected 0, 60, 120, 180, 300.
        let covered = [t(EVENT_MS), t(EVENT_MS + 120_000)];
        let gaps = expected_intervals(
            &covered,
            t(EVENT_MS),
            t(EVENT_MS + 300_000),
            Cadence::from_seconds(60).unwrap(),
            "gap-detector",
            7,
        )
        .unwrap();
        // Three missing intervals: 60s, 180s, and 300s offsets.
        assert_eq!(gaps.len(), 3);
        for gap in &gaps {
            // Distinguishable from measured values by construction (D-T6):
            // derived namespace, MISSING quality, and no value field exists
            // on this type at all.
            assert!(gap.source_id.starts_with(DERIVED_SOURCE_PREFIX));
            assert!(is_derived_source_id(&gap.source_id));
            assert_eq!(gap.source_id, "flexcore:derived:gap-detector");
            assert_eq!(gap.quality, TelemetryQuality::Missing);
            assert!(!is_derived_source_id("sim-1"));
        }
        // Detector-owned monotonic sequence from the supplied base.
        let seqs: Vec<u64> = gaps.iter().map(|g| g.source_sequence).collect();
        assert_eq!(seqs, vec![7, 8, 9]);
        // The gap event times are exactly the missing grid points.
        let times: Vec<i64> = gaps.iter().map(|g| g.event_time.unix_millis() as i64).collect();
        assert_eq!(
            times,
            vec![EVENT_MS + 60_000, EVENT_MS + 180_000, EVENT_MS + 300_000]
        );
    }

    #[test]
    fn full_coverage_and_inverted_horizon_produce_no_gaps() {
        let covered = [
            t(EVENT_MS),
            t(EVENT_MS + 60_000),
            t(EVENT_MS + 120_000),
            t(EVENT_MS + 180_000),
            t(EVENT_MS + 300_000),
        ];
        let gaps = expected_intervals(
            &covered,
            t(EVENT_MS),
            t(EVENT_MS + 300_000),
            Cadence::from_seconds(60).unwrap(),
            "gap-detector",
            0,
        )
        .unwrap();
        assert!(gaps.is_empty());
        // Inverted horizon: no expected intervals, no gaps.
        let inverted = expected_intervals(
            &[],
            t(EVENT_MS + 300_000),
            t(EVENT_MS),
            Cadence::from_seconds(60).unwrap(),
            "gap-detector",
            0,
        )
        .unwrap();
        assert!(inverted.is_empty());
        // Empty coverage yields a gap at every grid point.
        let all = expected_intervals(
            &[],
            t(EVENT_MS),
            t(EVENT_MS + 120_000),
            Cadence::from_seconds(60).unwrap(),
            "gap-detector",
            0,
        )
        .unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn derived_source_id_rejects_empty_detector() {
        assert_eq!(
            derived_source_id(""),
            Err(QualityRuleError::EmptyDetectorName)
        );
    }

    // --- ESTIMATED unreachable without an explicit derivation rule -----------

    #[test]
    fn estimated_never_appears_without_an_explicit_derivation_rule() {
        // Every producer except derive_estimated is exercised over its
        // pathological inputs; none can yield ESTIMATED.
        let pathological = [
            QualityInput {
                schema_valid: false,
                source_reported_invalid: true,
                soc_raw_fraction: Some(f64::NAN),
                range_check: CheckResult::Fail {
                    detail: "active power above the rated upper limit",
                },
                consistency_check: CheckResult::Fail {
                    detail: "BESS discharging (P_net > 0) while SOC is 0",
                },
            },
            clean_input(),
            QualityInput::default(),
        ];
        for input in &pathological {
            assert_ne!(assign_quality(input).quality, TelemetryQuality::Estimated);
        }
        // Staleness across the boundary: never ESTIMATED.
        for age in [-1_000_i64, 0, 300_000, 301_000] {
            let s = staleness(event_at(0), event_at(age), FreshnessLimit::from_seconds(300).unwrap());
            assert_ne!(s.stale_quality(), Some(TelemetryQuality::Estimated));
        }
        // Gap detection: markers are MISSING, never ESTIMATED.
        let gaps = expected_intervals(
            &[],
            t(EVENT_MS),
            t(EVENT_MS + 60_000),
            Cadence::from_seconds(60).unwrap(),
            "gap-detector",
            0,
        )
        .unwrap();
        assert!(gaps.iter().all(|g| g.quality == TelemetryQuality::Missing));
    }

    #[test]
    fn estimated_requires_and_records_the_derivation_rule() {
        // Without a rule there is no constructor call to make: derive_estimated
        // cannot be invoked without a &DerivationRule argument, and the rule
        // constructor validates its recorded identity.
        assert_eq!(
            DerivationRule::new("", "any-detector"),
            Err(QualityRuleError::EmptyRuleId)
        );
        assert_eq!(
            DerivationRule::new("rule-1", ""),
            Err(QualityRuleError::EmptyDetectorName)
        );
        let rule = DerivationRule::new("linear-interpolation", "estimator").unwrap();
        let d = derive_estimated(&rule, "interpolated between two GOOD neighbours");
        assert_eq!(d.quality, TelemetryQuality::Estimated);
        assert_eq!(d.reason, QualityReason::DerivedEstimation);
        // The decision carries the rule identity as evidence (explicit and
        // tagged, foundations §6.4).
        assert!(d.evidence.contains("linear-interpolation"));
        assert!(d.evidence.contains("estimator"));
    }

    // --- TelemetryQualityChanged predicate -----------------------------------

    #[test]
    fn quality_state_changed_true_exactly_on_a_change() {
        // Every distinct pair across all six states.
        for previous in TelemetryQuality::ALL {
            for current in TelemetryQuality::ALL {
                assert_eq!(
                    quality_state_changed(previous, current),
                    previous != current,
                    "{:?} -> {:?}",
                    previous,
                    current
                );
            }
        }
        // Spot checks on the event-relevant transitions.
        assert!(quality_state_changed(TelemetryQuality::Good, TelemetryQuality::Stale));
        assert!(quality_state_changed(TelemetryQuality::Good, TelemetryQuality::Suspect));
        assert!(!quality_state_changed(TelemetryQuality::Good, TelemetryQuality::Good));
    }

    // --- Exhaustiveness over the six frozen states ---------------------------

    #[test]
    fn producer_match_covers_all_six_states() {
        // Exhaustive match over the frozen six (P0.08 §1); adding a seventh
        // state breaks this test.
        for q in TelemetryQuality::ALL {
            let producer = match q {
                TelemetryQuality::Good => Producer::QualityAssigner,
                TelemetryQuality::Suspect => Producer::QualityAssigner,
                TelemetryQuality::Bad => Producer::QualityAssigner,
                TelemetryQuality::Missing => Producer::GapDetector,
                TelemetryQuality::Estimated => Producer::ExplicitDerivation,
                TelemetryQuality::Stale => Producer::StalenessDetector,
            };
            assert_eq!(producer_of(q), producer);
        }
        // The producer separation itself: the assigner owns exactly the
        // GOOD/SUSPECT/BAD trio; each other state has exactly one producer.
        assert_eq!(producer_of(TelemetryQuality::Missing), Producer::GapDetector);
        assert_eq!(producer_of(TelemetryQuality::Estimated), Producer::ExplicitDerivation);
        assert_eq!(producer_of(TelemetryQuality::Stale), Producer::StalenessDetector);
    }
}
