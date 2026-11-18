// P15.13 — Telemetry-drift detection: reporting-rate expectations and
//         value plausibility bounds per asset class (twin-lite).
//
// Phase 15 plan §2 row P15.13; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 42_p15_13_telemetry_drift_detection.md`.
// Sources: [S-168] (the MDPI DER state-estimation twin — cited for the
// digital-twin SPECTRUM this feature deliberately stays at the lite end
// of; no state-estimation capability is claimed), [S-53] (the
// asset-performance-management context: reporting-quality supervision as
// a fleet-operations function).
//
// WHAT THIS IS (and is not), DECIDED ONCE:
//
// - **Twin-lite, explicitly NOT grid state estimation.** This module
//   checks that per-asset telemetry keeps its DECLARED reporting-rate
//   expectations and its DECLARED value plausibility bounds. It solves
//   no network model, produces no state estimate, estimates no voltage,
//   and performs no observability analysis. A drift record is an
//   operator-attention record about TELEMETRY HEALTH, never a grid
//   state.
// - **The integration point is the Phase-2 telemetry source contract —
//   NO parallel ingestion path.** Every input is a
//   `flexcore_ports::telemetry_source::NormalizedTelemetry` record
//   delivered by the EXISTING `TelemetrySource` port (the P2.11
//   provider contract, `telemetry_source_contract.rs`). This module
//   never receives a raw frame, never decodes, never deduplicates, and
//   never stores: those are the P2.10 pipeline's stages, unchanged.
// - **Quality non-interference.** The six-state quality vocabulary
//   (P0.08) is owned by the ingestion pipeline's quality assignment
//   (P2.08). A drift verdict NEVER rewrites, upgrades, or downgrades a
//   record's quality state: it produces a separate drift record for
//   operator views. A SUSPECT value flagged by plausibility here stays
//   SUSPECT-or-as-assigned in the store — the drift record adds a
//   second, independent lens, never a mutation.
// - **Declared bounds, never learned.** Every reporting-rate
//   expectation and plausibility bound is a DECLARED input per asset
//   class with provenance and a recorded source. No bound is learned,
//   fitted, or inferred here (the platform has never run — no data
//   exists to learn from; the declared-parameters discipline is the
//   P15.14 posture mirrored).
// - **Scores/records, never dispatch inputs** (ADR-010 extension,
//   program conventions §6.3): a drift record feeds operator views and
//   telemetry-quality supervision; it never authorizes, produces, or
//   influences a dispatch command.
//
// Control status: pure evaluation over declared inputs for the
// simulation-only 0.x line. No I/O, no store, no scheduler wiring, no
// field-write path. No safety, certification, or skill claim is made.

use std::collections::BTreeMap;

use flexcore_contracts::time_model::EventTime;
use flexcore_domain::capability_provenance::AssetClass;
use flexcore_domain::ids::AssetId;
use flexcore_ports::telemetry_source::{NormalizedTelemetry, QuantityValue, TelemetryQuantity};

/// The telemetry-drift record schema version (a change is a NEW version,
/// never a silent edit).
pub const TELEMETRY_DRIFT_SCHEMA_VERSION: u32 = 1;

/// The twin-lite boundary statement (pinned by tests).
pub const TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT: &str = "Telemetry-drift detection is \
     TWIN-LITE: it checks declared reporting-rate expectations and declared value \
     plausibility bounds per asset class. It is explicitly NOT grid state estimation — no \
     network model is solved, no state is estimated, no voltage or power-flow quantity is \
     computed, and no observability analysis is performed [S-168 names the spectrum this \
     feature deliberately stays at the lite end of].";

/// The single-ingestion-path statement (pinned by tests).
pub const NO_PARALLEL_INGESTION_STATEMENT: &str = "Every drift evaluation consumes \
     NormalizedTelemetry records delivered by the EXISTING Phase-2 TelemetrySource port \
     (the P2.11 provider contract). No parallel ingestion path exists: no raw frames, no \
     second decoder, no side store, no separate broker subscription. The P2.10 pipeline \
     stages (receive, validate, normalize, quality-assign, dedup, store, publish) are \
     unchanged and remain the only way telemetry enters the platform.";

/// The quality non-interference statement (pinned by tests).
pub const QUALITY_NON_INTERFERENCE_STATEMENT: &str = "A drift verdict NEVER rewrites, \
     upgrades, or downgrades a telemetry record's quality state (the P0.08 six-state \
     vocabulary is owned by the P2.08 quality assignment in the ingestion pipeline). A \
     drift record is a separate, additive operator-view record; the non-silencing rule \
     (a quality flag is never silently converted) applies to drift exactly as it applies \
     inside the pipeline.";

/// The ADR-010 statement (pinned by tests).
pub const DRIFT_RECORD_IS_NOT_A_COMMAND_STATEMENT: &str = "A drift record is an \
     operator-attention record about telemetry health. It never authorizes, produces, or \
     influences a dispatch command (ADR-010 extension, program conventions 6.3).";

// --- Errors -------------------------------------------------------------------

/// Why a drift evaluation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq)]
pub enum DriftError {
    /// A declared bound was non-finite.
    NonFiniteBound { quantity: String, bound: &'static str },
    /// A plausibility band was inverted (min > max).
    InvertedBand { quantity: String, min: f64, max: f64 },
    /// A reporting-rate expectation was malformed (min > max, or a
    /// non-positive minimum).
    InvalidRateExpectation { reason: String },
    /// The declared expectation did not carry a recorded source.
    EmptySource,
    /// The record set was empty — an empty run is a refusal, never a
    /// clean bill of health (fail-closed, the P4.30 closing-rule
    /// discipline mirrored).
    EmptyRecordSet,
}

impl std::fmt::Display for DriftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftError::NonFiniteBound { quantity, bound } => write!(
                f,
                "the declared {bound} plausibility bound for quantity {quantity} is not finite"
            ),
            DriftError::InvertedBand { quantity, min, max } => write!(
                f,
                "the plausibility band for quantity {quantity} is inverted: min {min} > max {max}"
            ),
            DriftError::InvalidRateExpectation { reason } => write!(
                f,
                "the declared reporting-rate expectation is invalid: {reason}"
            ),
            DriftError::EmptySource => write!(
                f,
                "the declared expectation must carry a recorded source (never silent)"
            ),
            DriftError::EmptyRecordSet => write!(
                f,
                "the record set is empty; an empty run is a refusal, never a clean bill of health"
            ),
        }
    }
}

impl std::error::Error for DriftError {}

// --- 1. Reporting-rate expectations ---------------------------------------------

/// The declared reporting-rate expectation for one asset class (the
/// cadence the platform EXPECTS, declared per asset class — never
/// inferred from the observed data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingRateExpectation {
    /// The asset class the expectation is declared for.
    pub asset_class: AssetClass,
    /// The declared expected cadence in milliseconds (the provider
    /// declaration's cadence, mirrored per asset class for drift
    /// supervision).
    pub expected_interval_millis: i64,
    /// The declared tolerance band around the expected cadence, in
    /// milliseconds. An observed inter-arrival gap g drifts when
    /// |g − expected| > tolerance. `0` means exact (equality holds — the
    /// P2.08 convention: a boundary value is NOT a violation).
    pub tolerance_millis: i64,
    /// The recorded source of the declaration (non-empty).
    pub source: String,
}

impl ReportingRateExpectation {
    /// Validates and constructs one expectation.
    pub fn new(
        asset_class: AssetClass,
        expected_interval_millis: i64,
        tolerance_millis: i64,
        source: String,
    ) -> Result<Self, DriftError> {
        if expected_interval_millis <= 0 {
            return Err(DriftError::InvalidRateExpectation {
                reason: format!(
                    "the expected interval must be positive; got {expected_interval_millis}"
                ),
            });
        }
        if tolerance_millis < 0 {
            return Err(DriftError::InvalidRateExpectation {
                reason: format!("the tolerance must be >= 0; got {tolerance_millis}"),
            });
        }
        if source.trim().is_empty() {
            return Err(DriftError::EmptySource);
        }
        Ok(Self {
            asset_class,
            expected_interval_millis,
            tolerance_millis,
            source,
        })
    }
}

/// Why a reporting rate drifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDriftKind {
    /// Observed inter-arrival gaps fall below the declared band more
    /// often than the declared allowance (reporting too fast — a
    /// configuration or duplicate-source symptom).
    TooFrequent,
    /// Observed inter-arrival gaps exceed the declared band (reporting
    /// too sparse — an asset or channel degrading).
    TooSparse,
}

/// One asset's reporting-rate drift verdict over one record run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportingRateDriftRecord {
    /// The asset the verdict describes.
    pub asset_id: AssetId,
    /// The asset class the expectation was declared for.
    pub asset_class: AssetClass,
    /// The verdict: no drift, or the drift kind with its evidence.
    pub verdict: RateDriftVerdict,
    /// The number of inter-arrival gaps evaluated.
    pub gaps_evaluated: usize,
    /// The recorded source of the expectation (carried verbatim).
    pub source: String,
}

/// The per-run rate verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDriftVerdict {
    /// Every evaluated gap lies within the declared band (boundary values
    /// included — the P2.08 equality convention).
    WithinExpectation,
    /// The declared violation count was exceeded in one direction.
    Drifted {
        /// Which side of the band.
        kind: RateDriftKind,
        /// How many gaps violated (a violation is |gap − expected| >
        /// tolerance; equality is NOT a violation).
        violating_gaps: usize,
    },
}

/// Evaluates the reporting rate of one asset's ordered record run (the
/// records must be of the SAME asset and the same quantity; the caller
/// groups — the port's per-(asset, source) sequence rule, P0.09 §1, is
/// the ordering authority). The gaps are evaluated in record order; the
/// verdict fires when the declared violation count is exceeded.
pub fn evaluate_reporting_rate(
    records: &[NormalizedTelemetry],
    expectation: &ReportingRateExpectation,
    violation_threshold: usize,
) -> Result<ReportingRateDriftRecord, DriftError> {
    if records.is_empty() {
        return Err(DriftError::EmptyRecordSet);
    }
    if violation_threshold == 0 {
        return Err(DriftError::InvalidRateExpectation {
            reason: "the violation threshold must be at least 1 (a single boundary \
                     gap is not a drift)"
                .to_string(),
        });
    }
    let asset_id = records[0].asset_id();
    let mut too_frequent = 0usize;
    let mut too_sparse = 0usize;
    let mut gaps = 0usize;
    for pair in records.windows(2) {
        let (earlier, later) = (&pair[0], &pair[1]);
        let gap = later.event_time().unix_millis() - earlier.event_time().unix_millis();
        gaps += 1;
        let expected = i128::from(expectation.expected_interval_millis);
        let deviation = if gap > expected {
            gap - expected
        } else {
            expected - gap
        };
        // Boundary values are NOT violations (the P2.08 equality
        // convention; i128 comparison — the EventTime millis are i128).
        if deviation > i128::from(expectation.tolerance_millis) {
            if gap > expected {
                too_sparse += 1;
            } else {
                too_frequent += 1;
            }
        }
    }
    let verdict = if too_sparse > violation_threshold.saturating_sub(1) {
        RateDriftVerdict::Drifted {
            kind: RateDriftKind::TooSparse,
            violating_gaps: too_sparse,
        }
    } else if too_frequent > violation_threshold.saturating_sub(1) {
        RateDriftVerdict::Drifted {
            kind: RateDriftKind::TooFrequent,
            violating_gaps: too_frequent,
        }
    } else {
        RateDriftVerdict::WithinExpectation
    };
    Ok(ReportingRateDriftRecord {
        asset_id,
        asset_class: expectation.asset_class,
        verdict,
        gaps_evaluated: gaps,
        source: expectation.source.clone(),
    })
}

// --- 2. Value plausibility bounds ------------------------------------------------

/// The declared plausibility band for one quantity, in the quantity's
/// canonical unit (MW for active power, [0, 1] fractions for SOC, MVAr
/// magnitude for reactive power). Declared per asset class with
/// provenance — never learned.
#[derive(Debug, Clone, PartialEq)]
pub struct PlausibilityBounds {
    /// The quantity the band governs.
    pub quantity: TelemetryQuantity,
    /// The inclusive lower bound (canonical unit, caller's declared sign
    /// convention — the band never flips signs).
    pub min_value: f64,
    /// The inclusive upper bound.
    pub max_value: f64,
    /// The provenance of the declaration.
    pub provenance: flexcore_contracts::capability::Provenance,
    /// The recorded source of the declaration (non-empty).
    pub source: String,
}

impl PlausibilityBounds {
    /// Validates and constructs one band.
    pub fn new(
        quantity: TelemetryQuantity,
        min_value: f64,
        max_value: f64,
        provenance: flexcore_contracts::capability::Provenance,
        source: String,
    ) -> Result<Self, DriftError> {
        let name = quantity_name(quantity);
        if !min_value.is_finite() {
            return Err(DriftError::NonFiniteBound {
                quantity: name.to_string(),
                bound: "min",
            });
        }
        if !max_value.is_finite() {
            return Err(DriftError::NonFiniteBound {
                quantity: name.to_string(),
                bound: "max",
            });
        }
        if min_value > max_value {
            return Err(DriftError::InvertedBand {
                quantity: name.to_string(),
                min: min_value,
                max: max_value,
            });
        }
        if source.trim().is_empty() {
            return Err(DriftError::EmptySource);
        }
        Ok(Self {
            quantity,
            min_value,
            max_value,
            provenance,
            source,
        })
    }
}

/// The canonical quantity spelling (the P2.04 §4 vocabulary names).
fn quantity_name(quantity: TelemetryQuantity) -> &'static str {
    match quantity {
        TelemetryQuantity::ActivePower => "active_power",
        TelemetryQuantity::ReactivePower => "reactive_power",
        TelemetryQuantity::Soc => "soc",
        TelemetryQuantity::Energy => "energy",
        TelemetryQuantity::Availability => "availability",
        TelemetryQuantity::OperatingMode => "operating_mode",
        TelemetryQuantity::Health => "health",
        TelemetryQuantity::ConnectionState => "connection_state",
        TelemetryQuantity::BaselineReference => "baseline_reference",
    }
}

/// The numeric value of a record's quantity, if it carries one and the
/// quantity/value pairing makes it numeric. A `MISSING` gap marker
/// (`value == None`) has no value and is skipped by the plausibility
/// lens (the MISSING state is the pipeline's own supervision).
fn numeric_value(record: &NormalizedTelemetry) -> Option<f64> {
    let value = record.value()?;
    match (record.quantity(), value) {
        (TelemetryQuantity::ActivePower, QuantityValue::Power(p)) => Some(p.value()),
        (TelemetryQuantity::BaselineReference, QuantityValue::Power(p)) => Some(p.value()),
        (
            TelemetryQuantity::ReactivePower,
            QuantityValue::Reactive { power, .. },
        ) => Some(power.value()),
        (TelemetryQuantity::Soc, QuantityValue::Fraction(f)) => Some(f.value()),
        (TelemetryQuantity::Energy, QuantityValue::Energy(e)) => Some(e.value()),
        _ => None,
    }
}

/// One plausibility violation (a drift record; never a quality rewrite).
#[derive(Debug, Clone, PartialEq)]
pub struct PlausibilityViolation {
    /// The asset the violation concerns.
    pub asset_id: AssetId,
    /// The quantity.
    pub quantity: TelemetryQuantity,
    /// The record's event time (the observation instant, carried
    /// verbatim).
    pub event_time: EventTime,
    /// The observed value (canonical unit, caller's declared sign).
    pub observed_value: f64,
    /// The declared band (carried for the operator view).
    pub declared_min: f64,
    pub declared_max: f64,
    /// The recorded source of the band.
    pub source: String,
}

/// Evaluates one record against one declared band. A record of a
/// different quantity than the band governs is skipped (`None` — the
/// caller declares per-quantity bands); a MISSING gap marker is skipped;
/// a numeric value outside the INCLUSIVE band is one violation.
pub fn evaluate_plausibility(
    record: &NormalizedTelemetry,
    bounds: &PlausibilityBounds,
) -> Option<PlausibilityViolation> {
    if record.quantity() != bounds.quantity {
        return None;
    }
    let value = numeric_value(record)?;
    // Inclusive bounds: a boundary value is NOT a violation.
    if value >= bounds.min_value && value <= bounds.max_value {
        return None;
    }
    Some(PlausibilityViolation {
        asset_id: record.asset_id(),
        quantity: record.quantity(),
        event_time: record.event_time(),
        observed_value: value,
        declared_min: bounds.min_value,
        declared_max: bounds.max_value,
        source: bounds.source.clone(),
    })
}

/// Evaluates a record run against a per-quantity band table (BTreeMap —
/// deterministic iteration order), returning the violations in record
/// order. The run must be non-empty (fail-closed).
pub fn evaluate_plausibility_run(
    records: &[NormalizedTelemetry],
    bounds: &BTreeMap<TelemetryQuantity, PlausibilityBounds>,
) -> Result<Vec<PlausibilityViolation>, DriftError> {
    if records.is_empty() {
        return Err(DriftError::EmptyRecordSet);
    }
    let mut violations = Vec::new();
    for record in records {
        if let Some(band) = bounds.get(&record.quantity()) {
            if let Some(violation) = evaluate_plausibility(record, band) {
                violations.push(violation);
            }
        }
    }
    Ok(violations)
}

// --- Tests (WRITTEN-UNRUN — no cargo command was executed in this loop;
// --- the human build/test gate owns the run. Every expected value below
// --- is hand-computed from the rule sets above over CONSTRUCTED inputs.
// --- Nothing is fabricated.) -----------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
    use flexcore_contracts::units::{ActivePowerMW, StateOfCharge};
    use flexcore_domain::ids::TenantId;
    use flexcore_ports::telemetry_source::NormalizedTelemetryParts;

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn tenant() -> TenantId {
        TenantId::new(uid(0xA100))
    }

    fn asset(tag: u128) -> AssetId {
        AssetId::new(uid(tag))
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("valid test time")
    }

    fn it(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).expect("valid test ingest time")
    }

    /// One constructed active-power record (MW, the caller's declared
    /// sign convention).
    fn power_record(tag: u128, millis: i64, mw: f64) -> NormalizedTelemetry {
        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: tenant(),
            asset_id: asset(tag),
            quantity: TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(
                ActivePowerMW::new(mw).expect("finite test power"),
            )),
            event_time: t(millis),
            ingest_time: it(millis),
            source_sequence: 1,
            source_id: "sim://test".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid constructed record")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT.contains("TWIN-LITE"));
        assert!(TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT.contains("NOT grid state estimation"));
        assert!(TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT.contains("[S-168 names the spectrum"));
        assert!(NO_PARALLEL_INGESTION_STATEMENT.contains("EXISTING Phase-2 TelemetrySource port"));
        assert!(NO_PARALLEL_INGESTION_STATEMENT.contains("No parallel ingestion path"));
        assert!(QUALITY_NON_INTERFERENCE_STATEMENT.contains("NEVER rewrites"));
        assert!(DRIFT_RECORD_IS_NOT_A_COMMAND_STATEMENT.contains("never authorizes"));
    }

    #[test]
    fn reporting_rate_boundary_gaps_are_not_violations() {
        // Expected 60_000 ms, tolerance 10_000: gaps of 50_000, 70_000,
        // and 60_000 are ALL within (boundary == tolerance is not a
        // violation — the P2.08 convention).
        let expectation =
            ReportingRateExpectation::new(AssetClass::Bess, 60_000, 10_000, "declared".into())
                .expect("valid expectation");
        let records = vec![
            power_record(0xB001, 0, 1.0),
            power_record(0xB001, 50_000, 1.0),
            power_record(0xB001, 120_000, 1.0),
            power_record(0xB001, 180_000, 1.0),
        ];
        let record = evaluate_reporting_rate(&records, &expectation, 1)
            .expect("valid run");
        assert_eq!(record.gaps_evaluated, 3);
        assert_eq!(record.verdict, RateDriftVerdict::WithinExpectation);
    }

    #[test]
    fn reporting_rate_flags_too_sparse_and_too_frequent() {
        let expectation =
            ReportingRateExpectation::new(AssetClass::Bess, 60_000, 10_000, "declared".into())
                .expect("valid expectation");
        // Gaps: 90_000, 90_000, 60_000 -> two sparse violations; with a
        // threshold of 2 (>1 violation needed), the verdict fires
        // TooSparse.
        let records = vec![
            power_record(0xB001, 0, 1.0),
            power_record(0xB001, 90_000, 1.0),
            power_record(0xB001, 180_000, 1.0),
            power_record(0xB001, 240_000, 1.0),
        ];
        let record = evaluate_reporting_rate(&records, &expectation, 2)
            .expect("valid run");
        assert_eq!(
            record.verdict,
            RateDriftVerdict::Drifted {
                kind: RateDriftKind::TooSparse,
                violating_gaps: 2,
            }
        );
        // One violating gap with threshold 2: within expectation.
        let records = vec![
            power_record(0xB001, 0, 1.0),
            power_record(0xB001, 150_000, 1.0),
        ];
        let record = evaluate_reporting_rate(&records, &expectation, 2)
            .expect("valid run");
        assert_eq!(record.verdict, RateDriftVerdict::WithinExpectation);
        // Too-frequent direction: gaps of 10_000 (deviation 50_000 > 10_000).
        let records = vec![
            power_record(0xB001, 0, 1.0),
            power_record(0xB001, 10_000, 1.0),
            power_record(0xB001, 20_000, 1.0),
        ];
        let record = evaluate_reporting_rate(&records, &expectation, 1)
            .expect("valid run");
        assert_eq!(
            record.verdict,
            RateDriftVerdict::Drifted {
                kind: RateDriftKind::TooFrequent,
                violating_gaps: 2,
            }
        );
    }

    #[test]
    fn reporting_rate_refuses_empty_runs_bad_expectations_and_zero_threshold() {
        let expectation =
            ReportingRateExpectation::new(AssetClass::Bess, 60_000, 10_000, "declared".into())
                .expect("valid expectation");
        let err = evaluate_reporting_rate(&[], &expectation, 1).expect_err("empty refused");
        assert_eq!(err, DriftError::EmptyRecordSet);
        let err = ReportingRateExpectation::new(AssetClass::Bess, 0, 0, "s".into())
            .expect_err("zero interval refused");
        assert!(matches!(err, DriftError::InvalidRateExpectation { .. }));
        let err = ReportingRateExpectation::new(AssetClass::Bess, 60_000, -1, "s".into())
            .expect_err("negative tolerance refused");
        assert!(matches!(err, DriftError::InvalidRateExpectation { .. }));
        let err = ReportingRateExpectation::new(AssetClass::Bess, 60_000, 0, "  ".into())
            .expect_err("empty source refused");
        assert_eq!(err, DriftError::EmptySource);
        let records = vec![power_record(0xB001, 0, 1.0)];
        let err = evaluate_reporting_rate(&records, &expectation, 0)
            .expect_err("zero threshold refused");
        assert!(matches!(err, DriftError::InvalidRateExpectation { .. }));
    }

    #[test]
    fn plausibility_bounds_are_inclusive_and_sign_preserving() {
        let bounds = PlausibilityBounds::new(
            TelemetryQuantity::ActivePower,
            -2.0,
            2.0,
            flexcore_contracts::capability::Provenance::Configured,
            "declared".into(),
        )
        .expect("valid band");
        // In-band (boundary inclusive) -> no violation.
        assert!(evaluate_plausibility(&power_record(0xB001, 0, -2.0), &bounds).is_none());
        assert!(evaluate_plausibility(&power_record(0xB001, 0, 2.0), &bounds).is_none());
        // Out-of-band in both directions -> violations with the value
        // carried verbatim (the sign is never flipped).
        let violation = evaluate_plausibility(&power_record(0xB001, 0, -2.5), &bounds)
            .expect("import-side violation");
        assert_eq!(violation.observed_value, -2.5);
        assert_eq!(violation.declared_min, -2.0);
        let violation = evaluate_plausibility(&power_record(0xB001, 0, 2.1), &bounds)
            .expect("export-side violation");
        assert_eq!(violation.observed_value, 2.1);
    }

    #[test]
    fn plausibility_skips_other_quantities_missing_markers_and_non_numeric_values() {
        let bounds = PlausibilityBounds::new(
            TelemetryQuantity::ActivePower,
            -2.0,
            2.0,
            flexcore_contracts::capability::Provenance::Configured,
            "declared".into(),
        )
        .expect("valid band");
        // A different quantity is skipped by the lens.
        let soc_record = NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: tenant(),
            asset_id: asset(0xB001),
            quantity: TelemetryQuantity::Soc,
            value: Some(QuantityValue::Fraction(
                StateOfCharge::new(0.5).expect("valid fraction"),
            )),
            event_time: t(0),
            ingest_time: it(0),
            source_sequence: 1,
            source_id: "sim://test".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid constructed record");
        assert!(evaluate_plausibility(&soc_record, &bounds).is_none());
        // A MISSING gap marker (value None) is skipped — the MISSING
        // state is the pipeline's own supervision, not the drift lens'.
        let missing = NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: tenant(),
            asset_id: asset(0xB001),
            quantity: TelemetryQuantity::ActivePower,
            value: None,
            event_time: t(0),
            ingest_time: it(0),
            source_sequence: 1,
            source_id: "flexcore:derived:test".to_string(),
            quality: TelemetryQuality::Missing,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid constructed gap marker");
        assert!(evaluate_plausibility(&missing, &bounds).is_none());
    }

    #[test]
    fn plausibility_run_is_deterministic_and_fail_closed() {
        let mut bands = BTreeMap::new();
        bands.insert(
            TelemetryQuantity::ActivePower,
            PlausibilityBounds::new(
                TelemetryQuantity::ActivePower,
                -2.0,
                2.0,
                flexcore_contracts::capability::Provenance::Configured,
                "declared".into(),
            )
            .expect("valid band"),
        );
        let records = vec![
            power_record(0xB001, 0, 1.0),
            power_record(0xB001, 60_000, 9.0),
            power_record(0xB001, 120_000, -0.5),
        ];
        let violations = evaluate_plausibility_run(&records, &bands).expect("valid run");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].observed_value, 9.0);
        assert_eq!(violations[0].event_time, t(60_000));
        let err = evaluate_plausibility_run(&[], &bands).expect_err("empty refused");
        assert_eq!(err, DriftError::EmptyRecordSet);
    }

    #[test]
    fn every_drift_error_renders_non_empty_prose() {
        let errors = [
            DriftError::NonFiniteBound {
                quantity: "q".to_string(),
                bound: "min",
            },
            DriftError::InvertedBand {
                quantity: "q".to_string(),
                min: 2.0,
                max: 1.0,
            },
            DriftError::InvalidRateExpectation {
                reason: "r".to_string(),
            },
            DriftError::EmptySource,
            DriftError::EmptyRecordSet,
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
