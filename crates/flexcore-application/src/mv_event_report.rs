// P16.05 — M&V event reports: event performance against the program's
//         baseline, the same-day feedback pattern [S-43], and evidence
//         assembly from interval data carrying the §10.4 quality states.
//
// Phase 16 plan §2 row P16.05; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 05_p16_05_mv_event_reports.md`.
//
// THE POLICY, DECIDED ONCE:
//
//   **Every number in an M&V report is a settlement-adjacent financial
//   record** (context constraint 6; [S-32][S-59]). Concretely: the
//   report cannot exist without its P11 audit-chain LINK (the
//   `audit_record_id` of the audit record appended for the write); the
//   interval measurements it consumes travel WITH their §10.4 quality
//   states; a quality state outside the program's accepted set is
//   EXCLUDED and the exclusion is RECORDED (a quality flag is never
//   silently converted — §10.4); and a FINALIZED report refuses unless
//   every event-window interval carries an accepted-quality
//   measurement. No settlement number, revenue figure, invoice, or
//   payment exists anywhere in this module — performance is measured in
//   MW/MWh, never in money.
//
// THE SAME-DAY FEEDBACK PATTERN ([S-43], the Uplight pattern): a
// `SameDayPreliminary` report may be produced while the event window is
// still open — the operator sees the day's performance the same day.
// A `Finalized` report requires the event window to have CLOSED (now >
// event end, via the injected clock) AND zero exclusions. The two
// classes are structurally distinct values; a preliminary report is
// never re-labeled finalized — the finalized report is re-assembled
// from the same deterministic inputs.
//
// PERSISTENCE (recorded): the interval NUMBERS are persisted as the
// P16.01 `market_deliveries` rows (each quality-tagged and
// chain-linked); the aggregated report is DETERMINISTICALLY
// RE-DERIVABLE from those persisted inputs plus the program's baseline
// configuration and the P16.04 engine. No separate M&V-report table
// exists in migration 0022; the `MV_REPORT` retention class
// (P16.01) names the record class for the additive migration that lands
// a report row (wave-2 scope, recorded — never invented here).
//
// LR-073 CONSISTENCY: the report records the baseline method name AS
// THE PROGRAM DECLARED IT (carried on the P16.04 `BaselineProfile`);
// the assembler refuses a baseline whose method name is empty — the
// method stays the program's, end to end.
//
// Control status: pure assembly for the simulation-only 0.x line. No
// I/O, no store, no dispatch authority (ADR-010: an M&V report is a
// record, never a command).

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_ports::clock::Clock;
use std::collections::BTreeMap;

/// The M&V report schema version (a change is a NEW version, never a
/// silent edit).
pub const MV_REPORT_SCHEMA_VERSION: u32 = 1;

/// The financial-record statement (pinned by tests).
pub const EVERY_NUMBER_IS_FINANCIAL_RECORD_STATEMENT: &str = "Every number in an M&V \
     report is a settlement-adjacent financial record: the report cannot exist without \
     its P11 audit-chain LINK, the interval measurements travel WITH their §10.4 quality \
     states, exclusions are RECORDED (never silent), and a FINALIZED report requires \
     zero exclusions. No settlement number, revenue figure, invoice, or payment exists \
     anywhere in this module — performance is measured in MW/MWh, never in money.";

/// The same-day feedback statement (pinned by tests).
pub const SAME_DAY_FEEDBACK_STATEMENT: &str = "A SameDayPreliminary report may be produced \
     while the event window is still open (the same-day feedback pattern [S-43]); a \
     Finalized report requires the window CLOSED and zero exclusions. The classes are \
     structurally distinct; a preliminary report is never re-labeled finalized — the \
     finalized report is re-assembled from the same deterministic inputs.";

// --- Errors -------------------------------------------------------------------

/// Why an M&V report was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MvReportError {
    /// A required text field was empty (`field` names it).
    EmptyField {
        field: &'static str,
    },
    /// The program's accepted-quality set was empty (a report whose
    /// accepted set accepts nothing can never assemble).
    EmptyAcceptedQualitySet,
    /// The event window was inverted or unbounded.
    InvalidEventWindow,
    /// The interval duration was zero.
    ZeroIntervalDuration,
    /// An interval measurement carried a non-finite magnitude.
    NonFiniteMeasurement {
        interval_start_millis: i64,
    },
    /// An interval measurement lies OUTSIDE the event window (evidence
    /// from outside the window is never silently folded in).
    MeasurementOutsideEventWindow {
        interval_start_millis: i64,
    },
    /// A FINALIZED report was requested while the event window is still
    /// open (per the injected clock).
    EventWindowStillOpen {
        now_unix_ms: i64,
        event_end_millis: i64,
    },
    /// A FINALIZED report was requested with recorded exclusions — a
    /// finalized financial record cannot rest on excluded data.
    FinalizedWithExclusions {
        excluded_intervals: usize,
    },
    /// The measurements do not align with the baseline grid (count or
    /// ordering mismatch).
    IntervalGridMismatch {
        expected_intervals: usize,
        found_intervals: usize,
    },
    /// The audit-chain link was absent — every number is a financial
    /// record; a report without its P11 linkage cannot exist.
    MissingAuditChainLink,
}

impl std::fmt::Display for MvReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MvReportError::EmptyField { field } => {
                write!(f, "the M&V-report field '{field}' must be non-empty")
            }
            MvReportError::EmptyAcceptedQualitySet => write!(
                f,
                "the program's accepted-quality set is empty (the accepted set is the \
                 program's declaration — P9.11 Q2 stays EMPTY until the program declares)"
            ),
            MvReportError::InvalidEventWindow => write!(
                f,
                "the event window is invalid (end must be strictly after start, both \
                 non-negative)"
            ),
            MvReportError::ZeroIntervalDuration => {
                write!(f, "the interval duration must be positive")
            }
            MvReportError::NonFiniteMeasurement {
                interval_start_millis,
            } => write!(
                f,
                "the interval measurement at {interval_start_millis} is not finite"
            ),
            MvReportError::MeasurementOutsideEventWindow {
                interval_start_millis,
            } => write!(
                f,
                "the interval measurement at {interval_start_millis} lies outside the event \
                 window — evidence from outside the window is never silently folded in"
            ),
            MvReportError::EventWindowStillOpen {
                now_unix_ms,
                event_end_millis,
            } => write!(
                f,
                "the event window is still open (now {now_unix_ms} <= end {event_end_millis}); \
                 a finalized report requires the window CLOSED"
            ),
            MvReportError::FinalizedWithExclusions { excluded_intervals } => write!(
                f,
                "the finalized report would carry {excluded_intervals} excluded intervals — a \
                 finalized financial record cannot rest on excluded data"
            ),
            MvReportError::IntervalGridMismatch {
                expected_intervals,
                found_intervals,
            } => write!(
                f,
                "the measurements do not align with the baseline grid: {expected_intervals} \
                 expected, {found_intervals} found"
            ),
            MvReportError::MissingAuditChainLink => write!(
                f,
                "the audit-chain link is absent — every number is a financial record and a \
                 report without its P11 linkage cannot exist"
            ),
        }
    }
}

impl std::error::Error for MvReportError {}

// --- Report classes ----------------------------------------------------------------

/// The two structurally distinct report classes (the same-day feedback
/// pattern [S-43]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MvReportClass {
    /// Produced while the event window may still be open; exclusions are
    /// RECORDED, not fatal.
    SameDayPreliminary,
    /// Requires the window CLOSED and zero exclusions.
    Finalized,
}

impl MvReportClass {
    pub fn name(&self) -> &'static str {
        match self {
            MvReportClass::SameDayPreliminary => "SAME_DAY_PRELIMINARY",
            MvReportClass::Finalized => "FINALIZED",
        }
    }
}

/// The program's accepted-quality set: WHICH §10.4 states the program
/// accepts for M&V computation (the program's declaration — P9.11 Q2
/// stays EMPTY until a program declares; the set is never defaulted).
/// A fixed flag per frozen vocabulary state — deterministic membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedQualitySet {
    accepted: [bool; 6],
}

impl AcceptedQualitySet {
    /// Builds the set from the accepted states (at least one; a state
    /// outside the frozen six cannot exist — `TelemetryQuality` is the
    /// closed vocabulary).
    pub fn new(accepted: Vec<TelemetryQuality>) -> Result<Self, MvReportError> {
        if accepted.is_empty() {
            return Err(MvReportError::EmptyAcceptedQualitySet);
        }
        let mut flags = [false; 6];
        for quality in accepted {
            flags[TelemetryQuality::ALL.iter().position(|q| *q == quality).expect(
                "a TelemetryQuality is always one of the frozen six",
            )] = true;
        }
        Ok(Self { accepted: flags })
    }

    /// Membership (deterministic).
    pub fn contains(&self, quality: TelemetryQuality) -> bool {
        self.accepted[TelemetryQuality::ALL
            .iter()
            .position(|q| *q == quality)
            .expect("a TelemetryQuality is always one of the frozen six")]
    }
}

// --- Inputs -----------------------------------------------------------------------

/// One interval measurement (the evidence-assembly input): the measured
/// delivery magnitude FOR THE INTERVAL with its §10.4 quality state —
/// the quality travels WITH the number, always.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MvIntervalMeasurement {
    pub interval_start_millis: i64,
    /// END-EXCLUSIVE (§10.3), strictly after the start.
    pub interval_end_millis: i64,
    /// The measured delivery magnitude in MWh (finite; sign convention
    /// P > 0 = export at the point of connection, recorded AS MEASURED).
    pub delivered_energy_mwh: f64,
    pub quality: TelemetryQuality,
}

/// The program's M&V configuration for one event report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MvReportConfig {
    pub tenant_id: String,
    pub program_id: String,
    /// The event's external reference (the program's event identifier —
    /// recorded data).
    pub event_ref: String,
    /// The event window start (unix millis).
    pub event_start_millis: i64,
    /// The event window end, END-EXCLUSIVE (unix millis).
    pub event_end_millis: i64,
    /// The baseline interval duration in minutes (the grid the P16.04
    /// baseline was computed on).
    pub interval_duration_minutes: u32,
}

// --- Outputs -----------------------------------------------------------------------

/// One interval's recorded performance: the baseline value, the measured
/// value (average MW over the interval, derived from the declared
/// duration), the deviation (measured − baseline), and the quality state
/// the number carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MvIntervalPerformance {
    pub interval_start_millis: i64,
    pub interval_end_millis: i64,
    /// The program baseline value for the interval (MW).
    pub baseline_mw: f64,
    /// The measured average power over the interval (MW), derived from
    /// the delivered energy and the declared duration — a MEASUREMENT,
    /// recorded as measured.
    pub measured_mw: f64,
    /// The deviation (measured − baseline, MW) — a MEASUREMENT-derived
    /// figure, never a settlement value.
    pub deviation_mw: f64,
    pub quality: TelemetryQuality,
}

/// One RECORDED exclusion (a quality state outside the program's
/// accepted set, or a MISSING interval): the exclusion is part of the
/// report — never silent (§10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MvExcludedInterval {
    pub interval_start_millis: i64,
    pub interval_end_millis: i64,
    /// The reason the interval was excluded: the quality state outside
    /// the accepted set (`None` quality = the interval carried no
    /// measurement at all — recorded as MISSING semantics).
    pub carried_quality: Option<TelemetryQuality>,
}

/// The assembled M&V event report. Every number is a settlement-adjacent
/// financial record: the report carries its P11 audit-chain LINK, the
/// quality composition, and the recorded exclusions.
#[derive(Debug, Clone, PartialEq)]
pub struct MvEventReport {
    pub tenant_id: String,
    pub program_id: String,
    pub event_ref: String,
    pub report_class: MvReportClass,
    /// The baseline method name AS THE PROGRAM DECLARED IT (LR-073 —
    /// carried from the P16.04 `BaselineProfile`).
    pub baseline_method_name: String,
    /// The baseline source-day keys (carried from the profile; the
    /// report's evidence of what the baseline was built from).
    pub baseline_source_day_keys: Vec<String>,
    /// Whether the baseline carried an SDA application (and its applied
    /// ratio — recorded, never recomputed here).
    pub baseline_sda_applied_ratio: Option<f64>,
    pub intervals: Vec<MvIntervalPerformance>,
    /// The recorded exclusions (never silent).
    pub exclusions: Vec<MvExcludedInterval>,
    /// The quality composition over the event-window intervals (counts
    /// per frozen §10.4 state; BTreeMap by canonical name —
    /// deterministic).
    pub quality_composition: BTreeMap<String, u64>,
    /// The event window (recorded on the report).
    pub event_start_millis: i64,
    pub event_end_millis: i64,
    /// When the report was assembled (the INJECTED clock's instant).
    pub assembled_at_unix_ms: i64,
    /// The P11 audit-chain LINK — every number is a financial record.
    pub audit_record_id: String,
}

// --- The assembler -------------------------------------------------------------------

/// Assembles the M&V event report against the program's baseline (the
/// P16.04 `BaselineProfile`). Time enters ONLY through the injected
/// clock. The measurements must align with the baseline grid (same
/// interval count, sorted ascending, inside the event window); the
/// measured average power per interval is derived from the delivered
/// energy and the declared duration (MW = MWh / (duration/60) — units
/// are declared data, never converted silently).
pub fn assemble_mv_report(
    config: &MvReportConfig,
    report_class: MvReportClass,
    baseline: &crate::baseline_engine::BaselineProfile,
    measurements: &[MvIntervalMeasurement],
    accepted_qualities: &AcceptedQualitySet,
    audit_record_id: String,
    clock: &dyn Clock,
) -> Result<MvEventReport, MvReportError> {
    // The financial-record precondition FIRST: no chain link, no report.
    if audit_record_id.trim().is_empty() {
        return Err(MvReportError::MissingAuditChainLink);
    }
    for (field, value) in [
        ("tenant_id", config.tenant_id.as_str()),
        ("program_id", config.program_id.as_str()),
        ("event_ref", config.event_ref.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(MvReportError::EmptyField { field });
        }
    }
    if baseline.method_name.trim().is_empty() {
        return Err(MvReportError::EmptyField {
            field: "baseline_method_name",
        });
    }
    if config.event_start_millis < 0 || config.event_end_millis <= config.event_start_millis {
        return Err(MvReportError::InvalidEventWindow);
    }
    if config.interval_duration_minutes == 0 {
        return Err(MvReportError::ZeroIntervalDuration);
    }

    // The event-window rule: FINALIZED requires the window CLOSED (per
    // the injected clock); SAME_DAY_PRELIMINARY may assemble inside the
    // window (the same-day feedback pattern [S-43]).
    let event_time = clock.now_event_time();
    let event_millis = event_time.unix_millis();
    if event_millis < 0 || event_millis > i64::MAX as i128 {
        return Err(MvReportError::InvalidEventWindow);
    }
    let now_unix_ms = event_millis as i64;
    if report_class == MvReportClass::Finalized && now_unix_ms <= config.event_end_millis {
        return Err(MvReportError::EventWindowStillOpen {
            now_unix_ms,
            event_end_millis: config.event_end_millis,
        });
    }

    // Grid alignment: one measurement per baseline interval, sorted
    // ascending, inside the event window. A MISSING interval is recorded
    // as an exclusion (never invented).
    let interval_count = baseline.interval_values_mw.len();
    if measurements.len() != interval_count {
        return Err(MvReportError::IntervalGridMismatch {
            expected_intervals: interval_count,
            found_intervals: measurements.len(),
        });
    }
    let mut sorted: Vec<&MvIntervalMeasurement> = measurements.iter().collect();
    sorted.sort_by_key(|m| m.interval_start_millis);
    for measurement in &sorted {
        if !measurement.delivered_energy_mwh.is_finite() {
            return Err(MvReportError::NonFiniteMeasurement {
                interval_start_millis: measurement.interval_start_millis,
            });
        }
        if measurement.interval_end_millis <= measurement.interval_start_millis
            || measurement.interval_start_millis < config.event_start_millis
            || measurement.interval_end_millis > config.event_end_millis
        {
            return Err(MvReportError::MeasurementOutsideEventWindow {
                interval_start_millis: measurement.interval_start_millis,
            });
        }
    }

    // Evidence assembly with the §10.4 quality states: a state outside
    // the program's accepted set is EXCLUDED and the exclusion is
    // RECORDED (never silently converted or accepted).
    let hours_per_interval = config.interval_duration_minutes as f64 / 60.0;
    let mut intervals = Vec::with_capacity(interval_count);
    let mut exclusions = Vec::new();
    let mut composition: BTreeMap<String, u64> = BTreeMap::new();
    for (index, measurement) in sorted.iter().enumerate() {
        let entry = composition
            .entry(measurement.quality.name().to_string())
            .or_insert(0);
        *entry += 1;
        if !accepted_qualities.contains(measurement.quality) {
            exclusions.push(MvExcludedInterval {
                interval_start_millis: measurement.interval_start_millis,
                interval_end_millis: measurement.interval_end_millis,
                carried_quality: Some(measurement.quality),
            });
            continue;
        }
        let measured_mw = measurement.delivered_energy_mwh / hours_per_interval;
        let baseline_mw = baseline.interval_values_mw[index];
        intervals.push(MvIntervalPerformance {
            interval_start_millis: measurement.interval_start_millis,
            interval_end_millis: measurement.interval_end_millis,
            baseline_mw,
            measured_mw,
            deviation_mw: measured_mw - baseline_mw,
            quality: measurement.quality,
        });
    }

    // The FINALIZED gate: zero exclusions (a finalized financial record
    // cannot rest on excluded data).
    if report_class == MvReportClass::Finalized && !exclusions.is_empty() {
        return Err(MvReportError::FinalizedWithExclusions {
            excluded_intervals: exclusions.len(),
        });
    }

    Ok(MvEventReport {
        tenant_id: config.tenant_id.clone(),
        program_id: config.program_id.clone(),
        event_ref: config.event_ref.clone(),
        report_class,
        baseline_method_name: baseline.method_name.clone(),
        baseline_source_day_keys: baseline.source_day_keys.clone(),
        baseline_sda_applied_ratio: baseline.sda_applied.map(|sda| sda.applied_ratio),
        intervals,
        exclusions,
        quality_composition: composition,
        event_start_millis: config.event_start_millis,
        event_end_millis: config.event_end_millis,
        assembled_at_unix_ms: now_unix_ms,
        audit_record_id,
    })
}
