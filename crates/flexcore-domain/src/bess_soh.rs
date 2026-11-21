// P15.09 — BMS-reported SoH ingestion vocabulary: provenance + staleness
// (pure domain rules, no I/O). FIM-F11 (Wave 5) adds the on-platform SoH
// ESTIMATOR additively (composition of fresh, creditable observations — no
// aging law, no physics); the P15.09 ingestion vocabulary is unchanged.
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 23_p15_09_soh_ingestion.md`.
//
// THE RULE, DECIDED ONCE (P15.09), UPDATED BY FIM-F11:
//
//   **SoH is REPORTED data. FlexCore ingests BMS-reported SoH with
//   provenance and staleness bounds.** P15.09 deferred platform-side
//   estimation entirely; FIM-F11 supersedes that deferral AS CODE with one
//   on-platform estimator (`estimate_soh`, implemented written-unrun,
//   calibration a HUMAN LAB GATE — see [`SOH_ESTIMATION_DEFERRAL_STATEMENT`]
//   for the recorded supersession). The reported value remains the
//   AUTHORITY: the estimator composes with it and never overrides a
//   BMS-reported SoH without recording the disagreement as data
//   ([`SOH_AUTHORITY_RULE_STATEMENT`]).
//
// - **The estimation-method survey ([`SURVEYED_ESTIMATION_METHODS`])**
//   records the known method families with every P15.09-surveyed family
//   still marked NOT-IMPLEMENTED, plus one FIM-F11 row recording the
//   implemented composition method (written-unrun) — a growing record,
//   never a capability claim beyond it [S-145][S-146][S-147].
// - **Staleness per the envelope-staleness pattern** (`flexcore-application::
//   envelope_staleness.rs`, P3.13 — mirrored by citation, no dependency; the
//   domain crate cannot depend on the application crate): a CONFIGURED
//   maximum-age bound; `age == max_age` is NOT stale, `age > max_age` is
//   (the P2.08 staleness convention the P3.13 pattern carries). A STALE SoH
//   report is never credited for warranty-headroom or capability purposes —
//   the consumer recomputes (asks the BMS again) or fails loudly, never
//   silently proceeds.
// - **Provenance per the P3.02 discipline**: a report's provenance travels
//   with it; an UNKNOWN-provenance SoH report is not creditable (unknown
//   narrows, never widens). A BAD/MISSING telemetry quality flag is never
//   silently treated as GOOD (§10.4) — the report carries its source and the
//   adapter-side ingestion binds to the Phase-2 `TelemetrySource` port (the
//   recorded integration point; no parallel ingestion path is created — the
//   wiring is adapter/composition-root work, not domain code).
//
// UNITS: SoH is a fraction in [0, 1] at the API boundary (canonical units
// freeze, `flexcore-contracts::units` — READ-ONLY, untouched). Timestamps
// are unix millis (the `doe_envelope.rs` domain precedent).
//
// ADR-010 (conventions §6.3): a SoH value — like every degradation datum —
// never authorizes or produces a dispatch command.
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// I/O, no solver coupling, no device-write path, no dispatch reach.

use flexcore_contracts::capability::Provenance;

// --- Recorded statements -----------------------------------------------------

/// The recorded deferral statement (visible in records and exports, not a
/// comment), updated honestly by FIM-F11 (Wave 5): the P15.09 deferral is
/// recorded as history, the supersession as current state — the estimator is
/// implemented, WRITTEN-UNRUN, its calibration a HUMAN LAB GATE.
pub const SOH_ESTIMATION_DEFERRAL_STATEMENT: &str =
    "RECORDED STATEMENT (updated by FIM-F11, Wave 5): P15.09 DEFERRED \
platform-side SoH estimation — FlexCore ingested BMS-reported SoH only, \
every surveyed method NOT-IMPLEMENTED. FIM-F11 supersedes the deferral AS \
CODE: one on-platform estimator (deterministic composition of fresh, \
creditable capacity-retention observations; weakest-provenance merge; \
staleness bound; no aging law) is implemented in this module, \
WRITTEN-UNRUN — never compiled, never executed; calibration is a HUMAN LAB \
GATE (no measured fleet exists). BMS-reported SoH remains the ingestion \
path and the authority: the estimate never overrides a BMS-reported SoH \
without recording the disagreement as data.";

/// The surveyed SoH-estimation method families (P15.09: "estimation methods
/// surveyed, none implemented"). Each entry is `(method family, status)`;
/// the five P15.09-surveyed families all carry the recorded
/// NOT-IMPLEMENTED status, and the sixth row records the FIM-F11
/// composition method (implemented, written-unrun). The survey is a growing
/// RECORD — the presence of a method here implies no capability claim
/// beyond its recorded status.
/// Sources: the calendar/cycle aging literature [S-145], the BLAST /
/// BLAST-Lite semi-empirical parameterized models [S-146][S-147].
pub const SURVEYED_ESTIMATION_METHODS: [(&str, &str); 6] = [
    (
        "semi-empirical calendar+cycle aging parameterization (BLAST-Lite class)",
        "NOT-IMPLEMENTED (survey record only; [S-146][S-147])",
    ),
    (
        "coulomb counting / throughput-integrated capacity tracking",
        "NOT-IMPLEMENTED (survey record only)",
    ),
    (
        "capacity derivation from observed full cycles",
        "NOT-IMPLEMENTED (survey record only)",
    ),
    (
        "impedance / EIS-based health estimation",
        "NOT-IMPLEMENTED (survey record only)",
    ),
    (
        "data-driven (ML) SoH estimation",
        "NOT-IMPLEMENTED (survey record only; no runtime data exists to train on)",
    ),
    (
        "on-platform composition of fresh, creditable capacity-retention \
observations (FIM-F11)",
        "IMPLEMENTED-WRITTEN-UNRUN (FIM-F11 Wave 5; composition of \
observations, no aging law, no physics; calibration is a HUMAN LAB GATE)",
    ),
];

/// The recorded staleness policy for SoH reports (the P3.13 pattern applied).
pub const STALE_SOH_POLICY: &str =
    "A STALE BMS-reported SoH value is never credited for warranty headroom \
or capability narrowing: the consumer re-queries the BMS or fails loudly — \
it never silently proceeds with a stale SoH. age == max_age is NOT stale; \
age > max_age is (the P2.08 convention, mirrored from the P3.13 \
envelope-staleness pattern).";

/// The recorded DEFAULT maximum SoH-report age: 7 days. ASSUMPTION (a
/// configuration default with no measured basis — the binding value is a
/// deployment configuration; per-asset overrides are the consumer's declared
/// configuration).
pub const DEFAULT_SOH_MAX_AGE_MILLIS: i64 = 7 * 24 * 3_600_000;

// --- Errors ------------------------------------------------------------------

/// Errors of SoH report construction and freshness evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum SohIngestionError {
    /// A numeric datum was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A fraction was outside [0, 1].
    ValueOutOfRange {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A required text field was empty or whitespace.
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
        /// The recorded bound.
        max_chars: usize,
    },
    /// The staleness configuration was non-finite or negative (a max-age
    /// bound must be a positive number of milliseconds).
    InvalidMaxAge {
        /// The offending value.
        value: i64,
    },
    /// The report's ingest time preceded its event time (the P0.09
    /// `ingest_time >= event_time` ordering rule); refused, never
    /// re-stamped.
    IngestBeforeEvent {
        /// The event time (unix millis).
        event_time_unix_ms: i64,
        /// The offending ingest time (unix millis).
        ingest_time_unix_ms: i64,
    },
}

impl std::fmt::Display for SohIngestionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SohIngestionError::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite (found {value})")
            }
            SohIngestionError::ValueOutOfRange { field, value } => {
                write!(f, "{field} must be a fraction in [0,1] (found {value})")
            }
            SohIngestionError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            SohIngestionError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
            SohIngestionError::InvalidMaxAge { value } => {
                write!(f, "the maximum-age bound must be a positive number of milliseconds (found {value})")
            }
            SohIngestionError::IngestBeforeEvent {
                event_time_unix_ms,
                ingest_time_unix_ms,
            } => write!(
                f,
                "ingest time {ingest_time_unix_ms} ms precedes the event time \
                 {event_time_unix_ms} ms (P0.09 ordering rule); the report is refused, \
                 never re-stamped"
            ),
        }
    }
}

impl std::error::Error for SohIngestionError {}


// --- Configuration -------------------------------------------------------------

/// The SoH staleness configuration (the P3.13 `StalenessConfig` max-age
/// half, mirrored for SoH; the debounce half has no SoH analogue — SoH
/// reports are periodic observations, not event-triggered recomputations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SohStalenessConfig {
    /// The maximum report age in milliseconds beyond which the report is
    /// stale. `age == max_age` is not stale; `age > max_age` is.
    pub max_age_millis: i64,
}

impl Default for SohStalenessConfig {
    /// The recorded default: [`DEFAULT_SOH_MAX_AGE_MILLIS`] (ASSUMPTION — a
    /// configuration default, not a measured requirement).
    fn default() -> Self {
        Self {
            max_age_millis: DEFAULT_SOH_MAX_AGE_MILLIS,
        }
    }
}

impl SohStalenessConfig {
    /// Constructs and validates the configuration.
    pub fn new(max_age_millis: i64) -> Result<Self, SohIngestionError> {
        if max_age_millis <= 0 {
            return Err(SohIngestionError::InvalidMaxAge { value: max_age_millis });
        }
        Ok(Self { max_age_millis })
    }
}

// --- The report -----------------------------------------------------------------

/// One BMS-reported SoH value (P15.09): the ingested datum with its
/// provenance, event/ingest timestamps, and reporting source. This is the
/// type the adapter-side ingestion produces when binding to the Phase-2
/// `TelemetrySource` port (the recorded integration point — the SoH
/// quantity arrives through the existing telemetry path, never a parallel
/// ingestion path).
#[derive(Debug, Clone, PartialEq)]
pub struct BmsSohReport {
    /// The reported SoH as a fraction in [0, 1].
    pub soh_fraction: f64,
    /// When the BMS measured the value (unix millis).
    pub event_time_unix_ms: i64,
    /// When FlexCore received the value (unix millis; >= event time is the
    /// caller's responsibility at the port boundary — the P0.09
    /// `ingest_time >= event_time` ordering rule).
    pub ingest_time_unix_ms: i64,
    /// The provenance of the reported value (a BMS report is typically
    /// `Measured`; an adapter-declared value is `Configured`; UNKNOWN is
    /// legal and narrows — never widens).
    pub provenance: Provenance,
    /// The reporting source reference (e.g. the BMS identity; recorded data).
    pub source: String,
}

impl BmsSohReport {
    /// Constructs and validates a BMS SoH report.
    pub fn new(
        soh_fraction: f64,
        event_time_unix_ms: i64,
        ingest_time_unix_ms: i64,
        provenance: Provenance,
        source: String,
    ) -> Result<Self, SohIngestionError> {
        if !soh_fraction.is_finite() {
            return Err(SohIngestionError::NonFiniteValue {
                field: "soh_fraction",
                value: soh_fraction,
            });
        }
        if !(0.0..=1.0).contains(&soh_fraction) {
            return Err(SohIngestionError::ValueOutOfRange {
                field: "soh_fraction",
                value: soh_fraction,
            });
        }
        if ingest_time_unix_ms < event_time_unix_ms {
            // The P0.09 ordering rule, applied at construction: a report
            // received before it occurred is refused, never silently
            // re-stamped.
            return Err(SohIngestionError::IngestBeforeEvent {
                event_time_unix_ms,
                ingest_time_unix_ms,
            });
        }
        if source.trim().is_empty() {
            return Err(SohIngestionError::EmptyField { field: "source" });
        }
        if source.len() > 256 {
            return Err(SohIngestionError::FieldTooLong { field: "source", max_chars: 256 });
        }
        Ok(Self {
            soh_fraction,
            event_time_unix_ms,
            ingest_time_unix_ms,
            provenance,
            source,
        })
    }
}

// --- The freshness evaluation -----------------------------------------------------

/// Why a SoH report is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SohStalenessReason {
    /// The configured maximum age was exceeded (`age > max_age`; equality is
    /// NOT stale — the P2.08 convention the P3.13 pattern carries).
    MaxAgeExceeded {
        /// The report's age at evaluation time, millis.
        age_millis: i64,
        /// The configured bound.
        max_age_millis: i64,
    },
    /// The evaluation clock read before the report's event time — a
    /// caller-side clock inconsistency, treated as stale-until-resolved
    /// (never silently credited).
    ClockBeforeEventTime,
}

impl std::fmt::Display for SohStalenessReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SohStalenessReason::MaxAgeExceeded { age_millis, max_age_millis } => write!(
                f,
                "SoH report age {age_millis} ms exceeds the configured maximum {max_age_millis} ms"
            ),
            SohStalenessReason::ClockBeforeEventTime => {
                write!(f, "the evaluation clock is before the report's event time; the report is stale-until-resolved")
            }
        }
    }
}

/// Whether a SoH report is fresh or stale (the P3.13 verdict shape, applied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SohFreshness {
    /// No staleness rule fired.
    Fresh,
    /// The report is stale: it must not back warranty headroom or capability
    /// narrowing ([`STALE_SOH_POLICY`]).
    Stale {
        /// Why the report is stale.
        reason: SohStalenessReason,
    },
}

/// The pure staleness evaluation (P3.13 pattern): the max-age bound against
/// the report's EVENT time (what the BMS measured), not the ingest time.
/// A clock read before the event time is its own staleness reason — never a
/// silent fresh verdict.
pub fn evaluate_soh_freshness(
    report: &BmsSohReport,
    now_unix_millis: i64,
    config: &SohStalenessConfig,
) -> SohFreshness {
    if now_unix_millis < report.event_time_unix_ms {
        return SohFreshness::Stale {
            reason: SohStalenessReason::ClockBeforeEventTime,
        };
    }
    let age = now_unix_millis - report.event_time_unix_ms;
    if age > config.max_age_millis {
        return SohFreshness::Stale {
            reason: SohStalenessReason::MaxAgeExceeded {
                age_millis: age,
                max_age_millis: config.max_age_millis,
            },
        };
    }
    SohFreshness::Fresh
}

/// Whether a SoH report is CREDITABLE for downstream narrowing/warranty use:
/// fresh AND known provenance (P3.02: unknown narrows, never widens — an
/// UNKNOWN-provenance SoH report credits nothing). This is the single gate
/// the P15.10 headroom computation applies.
pub fn soh_report_creditable(
    report: &BmsSohReport,
    now_unix_millis: i64,
    config: &SohStalenessConfig,
) -> bool {
    report.provenance != Provenance::Unknown
        && evaluate_soh_freshness(report, now_unix_millis, config) == SohFreshness::Fresh
}

// --- The FIM-F11 on-platform estimator (additive) -----------------------------
//
// WHAT THE ESTIMATOR IS: one on-platform SoH ESTIMATE produced by a
// deterministic composition of the fresh, creditable capacity-retention
// observations it is given — the arithmetic mean over the credited inputs
// (a DECLARED convention, not physics), the provenance the WEAKEST credited
// provenance (the P3.02 merge reused from `crate::bess_degradation`), and a
// staleness bound: the estimate is as fresh as its newest credited input.
//
// WHAT THE ESTIMATOR IS NOT:
//
// - **Not an aging law.** No fade equation, no calendar/cycle-aging physics,
//   no capacity-from-throughput derivation: turning the P15.07 ledger
//   counters into SoH would require a calibrated degradation law, and none
//   exists (the P15.07 honesty line stands). The ledger counters travel in
//   the outcome as RECORDED CONTEXT only — never in the arithmetic.
// - **Not an override.** A fresh, creditable REPORTED observation
//   (typically BMS-reported SoH) is the operative value; the estimate is
//   advisory and never replaces it. A disagreement beyond the configured
//   tolerance is recorded as DATA ([`SOH_DISAGREEMENT_STATEMENT`]).
// - **Not calibrated.** The weighting scheme (equal weights), the window
//   (the configured staleness bound), and the disagreement tolerance are
//   calibration INPUTS pending the human gate: no measured fleet exists.
//
// Control status: pure domain computation for the simulation-only 0.x line
// (see the module header for the units and ADR-010 discipline).

/// The recorded authority rule (visible in records and exports, not a
/// comment).
pub const SOH_AUTHORITY_RULE_STATEMENT: &str =
    "AUTHORITY RULE: a fresh, creditable reported observation (typically \
BMS-reported SoH) is the operative SoH value; the platform estimate is \
advisory and never replaces it. Where the estimate disagrees with the \
reported value beyond the configured tolerance, a disagreement record is \
produced as DATA. A SoH value never authorizes or produces a dispatch \
command (ADR-010).";

/// The recorded disagreement policy carried on every disagreement record.
pub const SOH_DISAGREEMENT_STATEMENT: &str =
    "DISAGREEMENT RECORD: the platform SoH estimate differs from the \
reported value beyond the configured tolerance. This record is DATA: the \
reported value remains the authority and the estimate remains advisory; \
neither value authorizes or produces a dispatch command (ADR-010).";

/// The recorded method label of the implemented estimator (the sixth row of
/// [`SURVEYED_ESTIMATION_METHODS`]).
pub const SOH_ESTIMATION_METHOD_LABEL: &str = "RECENCY_BOUNDED_OBSERVATION_COMPOSITION_V1";

/// The recorded DEFAULT disagreement tolerance: 0.01 (one percentage point
/// of SoH). ASSUMPTION (a configuration default with no measured basis —
/// the binding value is per-deployment configuration; calibration is part
/// of the HUMAN LAB GATE).
pub const DEFAULT_SOH_DISAGREEMENT_TOLERANCE_FRACTION: f64 = 0.01;

/// The estimator configuration: the disagreement tolerance (the staleness
/// bound itself is the existing [`SohStalenessConfig`], reused).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SohEstimationConfig {
    /// `|estimate − reported|` beyond which a disagreement record is
    /// produced (fraction in [0, 1]).
    pub disagreement_tolerance_fraction: f64,
}

impl Default for SohEstimationConfig {
    /// The recorded default: [`DEFAULT_SOH_DISAGREEMENT_TOLERANCE_FRACTION`]
    /// (ASSUMPTION — a configuration default, not a measured requirement).
    fn default() -> Self {
        Self {
            disagreement_tolerance_fraction: DEFAULT_SOH_DISAGREEMENT_TOLERANCE_FRACTION,
        }
    }
}

impl SohEstimationConfig {
    /// Constructs and validates the configuration.
    pub fn new(disagreement_tolerance_fraction: f64) -> Result<Self, SohIngestionError> {
        if !disagreement_tolerance_fraction.is_finite() {
            return Err(SohIngestionError::NonFiniteValue {
                field: "disagreement_tolerance_fraction",
                value: disagreement_tolerance_fraction,
            });
        }
        if !(0.0..=1.0).contains(&disagreement_tolerance_fraction) {
            return Err(SohIngestionError::ValueOutOfRange {
                field: "disagreement_tolerance_fraction",
                value: disagreement_tolerance_fraction,
            });
        }
        Ok(Self {
            disagreement_tolerance_fraction,
        })
    }
}

/// The P15.07 ledger counters as RECORDED CONTEXT on an estimation outcome:
/// they travel with the outcome (with their own provenance) so a consumer
/// sees what the counters showed — they NEVER enter the estimate arithmetic
/// (no aging law exists; see the section header).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SohLedgerCounters {
    /// The ledger's cumulative full equivalent cycles (dimensionless).
    pub full_equivalent_cycles: f64,
    /// The FEC counter's provenance.
    pub full_equivalent_cycles_provenance: Provenance,
    /// The ledger's cumulative (charge + discharge) throughput in MWh.
    pub cumulative_throughput_mwh: f64,
    /// The throughput counter's provenance.
    pub cumulative_throughput_mwh_provenance: Provenance,
}

/// The platform SoH estimate: the composed value with its provenance, the
/// staleness anchor (its newest credited input), and the recorded method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SohEstimate {
    /// The composed estimate, a fraction in [0, 1].
    pub soh_fraction: f64,
    /// The merged provenance: the WEAKEST credited input provenance
    /// (unknown narrows, never widens — P3.02).
    pub provenance: Provenance,
    /// When the estimate was computed (unix millis).
    pub computed_at_unix_ms: i64,
    /// The number of credited inputs the estimate composed.
    pub input_observations: usize,
    /// The event time of the newest credited input (unix millis) — the
    /// estimate's staleness anchor ([`evaluate_soh_estimate_freshness`]).
    pub newest_input_event_time_unix_ms: i64,
    /// The recorded method label ([`SOH_ESTIMATION_METHOD_LABEL`]).
    pub method: &'static str,
}

/// The operative SoH value for one estimation outcome. A fresh, creditable
/// REPORTED observation is the authority; the estimate is advisory and
/// never replaces it (the recorded authority rule). Where no credited
/// observation exists, nothing is credited and no value is invented.
#[derive(Debug, Clone, PartialEq)]
pub enum SohAuthority {
    /// A fresh, creditable reported observation is the operative value
    /// (typically BMS-reported SoH; the reporting source is recorded data).
    Reported {
        /// The reported SoH fraction in [0, 1].
        soh_fraction: f64,
        /// The reported value's provenance.
        provenance: Provenance,
        /// The observation's event time (unix millis).
        event_time_unix_ms: i64,
        /// The reporting source reference (recorded data).
        source: String,
    },
    /// No fresh, creditable reported observation exists: nothing is
    /// credited (P3.02 — unknown narrows, stale is never silently used) and
    /// no value is invented.
    NotCreditable {
        /// The recorded reason (audit-safe).
        reason: String,
    },
}

/// One recorded disagreement between the platform estimate and the reported
/// value. DATA — never a correction: the reported value stays authoritative
/// ([`SOH_DISAGREEMENT_STATEMENT`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SohDisagreementRecord {
    /// The reported (authoritative) SoH fraction.
    pub reported_soh_fraction: f64,
    /// The reported value's provenance.
    pub reported_provenance: Provenance,
    /// The reported observation's event time (unix millis).
    pub reported_event_time_unix_ms: i64,
    /// The advisory platform estimate.
    pub estimated_soh_fraction: f64,
    /// The estimate's merged provenance.
    pub estimate_provenance: Provenance,
    /// `estimated − reported` (a signed fraction; the sign says which side
    /// is higher).
    pub delta_fraction: f64,
    /// The configured tolerance the delta exceeded.
    pub tolerance_fraction: f64,
    /// When the disagreement was recorded (unix millis).
    pub recorded_at_unix_ms: i64,
    /// The recorded disagreement policy ([`SOH_DISAGREEMENT_STATEMENT`]).
    pub note: &'static str,
}

/// The outcome of one estimation run: the operative authority, the advisory
/// estimate (when any credited input exists), the recorded disagreement
/// (when the advisory estimate diverges beyond tolerance), and the recorded
/// exclusions — every exclusion is visible, never silent.
#[derive(Debug, Clone, PartialEq)]
pub struct SohEstimationOutcome {
    /// The operative SoH authority for this outcome.
    pub authority: SohAuthority,
    /// The advisory platform estimate, when any credited input existed.
    pub estimate: Option<SohEstimate>,
    /// The recorded disagreement, when the estimate diverged from the
    /// reported authority beyond the configured tolerance.
    pub disagreement: Option<SohDisagreementRecord>,
    /// Inputs excluded as stale (beyond the configured maximum age) —
    /// recorded, never silently dropped.
    pub stale_inputs_excluded: usize,
    /// Inputs excluded as UNKNOWN-provenance (unknown data is never
    /// credited, P3.02) — recorded, never silently dropped.
    pub unknown_provenance_inputs_excluded: usize,
    /// The ledger counters as recorded context (never in the arithmetic).
    pub ledger_counters_recorded: Option<SohLedgerCounters>,
    /// The recorded authority rule ([`SOH_AUTHORITY_RULE_STATEMENT`]).
    pub statement: &'static str,
}

/// The staleness bound of an estimate: the estimate is as fresh as its
/// newest credited input (it cannot out-live the data it composed). The
/// same max-age pattern and equality convention as
/// [`evaluate_soh_freshness`] apply.
pub fn evaluate_soh_estimate_freshness(
    estimate: &SohEstimate,
    now_unix_millis: i64,
    config: &SohStalenessConfig,
) -> SohFreshness {
    if now_unix_millis < estimate.newest_input_event_time_unix_ms {
        return SohFreshness::Stale {
            reason: SohStalenessReason::ClockBeforeEventTime,
        };
    }
    let age = now_unix_millis - estimate.newest_input_event_time_unix_ms;
    if age > config.max_age_millis {
        return SohFreshness::Stale {
            reason: SohStalenessReason::MaxAgeExceeded {
                age_millis: age,
                max_age_millis: config.max_age_millis,
            },
        };
    }
    SohFreshness::Fresh
}

/// Whether an estimate is CREDITABLE for downstream use: fresh (as fresh as
/// its newest credited input) AND known provenance (P3.02).
pub fn soh_estimate_creditable(
    estimate: &SohEstimate,
    now_unix_millis: i64,
    config: &SohStalenessConfig,
) -> bool {
    estimate.provenance != Provenance::Unknown
        && evaluate_soh_estimate_freshness(estimate, now_unix_millis, config) == SohFreshness::Fresh
}

/// Produces the platform SoH estimate over the given capacity-retention
/// observations (typically the P15.07 ledger's append-ordered
/// `soh_observations`). The observations are validated (named errors);
/// stale and UNKNOWN-provenance inputs are EXCLUDED and the exclusions are
/// recorded; the estimate is the equal-weight mean over the credited
/// inputs; the authority is the freshest credited reported observation.
///
/// The ledger counters, when supplied, travel as RECORDED CONTEXT only.
pub fn estimate_soh(
    observations: &[crate::bess_degradation::SohObservation],
    ledger_counters: Option<SohLedgerCounters>,
    now_unix_millis: i64,
    staleness: &SohStalenessConfig,
    config: &SohEstimationConfig,
) -> Result<SohEstimationOutcome, SohIngestionError> {
    if !config.disagreement_tolerance_fraction.is_finite() {
        return Err(SohIngestionError::NonFiniteValue {
            field: "disagreement_tolerance_fraction",
            value: config.disagreement_tolerance_fraction,
        });
    }
    if !(0.0..=1.0).contains(&config.disagreement_tolerance_fraction) {
        return Err(SohIngestionError::ValueOutOfRange {
            field: "disagreement_tolerance_fraction",
            value: config.disagreement_tolerance_fraction,
        });
    }
    for observation in observations {
        if !observation.soh_fraction.is_finite() {
            return Err(SohIngestionError::NonFiniteValue {
                field: "soh_fraction",
                value: observation.soh_fraction,
            });
        }
        if !(0.0..=1.0).contains(&observation.soh_fraction) {
            return Err(SohIngestionError::ValueOutOfRange {
                field: "soh_fraction",
                value: observation.soh_fraction,
            });
        }
    }
    // Classify every input: credited (fresh + known provenance), stale, or
    // unknown-provenance. Every exclusion is recorded, never silent.
    let mut credited: Vec<&crate::bess_degradation::SohObservation> = Vec::new();
    let mut stale_inputs_excluded = 0usize;
    let mut unknown_provenance_inputs_excluded = 0usize;
    for observation in observations {
        if observation.provenance == Provenance::Unknown {
            unknown_provenance_inputs_excluded += 1;
            continue;
        }
        // The staleness bound evaluates the observation's EVENT time; the
        // observation carries no separate ingest stamp (the P15.10 recorded
        // convention: the ledger records observations, not receipts).
        let report = BmsSohReport {
            soh_fraction: observation.soh_fraction,
            event_time_unix_ms: observation.observed_at_unix_ms,
            ingest_time_unix_ms: observation.observed_at_unix_ms,
            provenance: observation.provenance,
            source: observation.source.clone(),
        };
        if evaluate_soh_freshness(&report, now_unix_millis, staleness) == SohFreshness::Fresh {
            credited.push(observation);
        } else {
            stale_inputs_excluded += 1;
        }
    }
    if credited.is_empty() {
        return Ok(SohEstimationOutcome {
            authority: SohAuthority::NotCreditable {
                reason: "no fresh, creditable SoH observation exists (all inputs stale, \
                         UNKNOWN-provenance, or absent); nothing is credited and no value \
                         is invented (P3.02 / P15.09)"
                    .to_string(),
            },
            estimate: None,
            disagreement: None,
            stale_inputs_excluded,
            unknown_provenance_inputs_excluded,
            ledger_counters_recorded: ledger_counters,
            statement: SOH_AUTHORITY_RULE_STATEMENT,
        });
    }
    // The estimate: the equal-weight arithmetic mean over the credited
    // inputs (declared convention — the weighting scheme is a calibration
    // input pending the human gate), with the WEAKEST credited provenance
    // (the P3.02 merge, reused).
    let mut sum = 0.0f64;
    let mut provenance = credited[0].provenance;
    let mut newest = credited[0].observed_at_unix_ms;
    for observation in &credited {
        sum += observation.soh_fraction;
        provenance = crate::bess_degradation::weaker_provenance(provenance, observation.provenance);
        if observation.observed_at_unix_ms >= newest {
            newest = observation.observed_at_unix_ms;
        }
    }
    let estimate = SohEstimate {
        soh_fraction: sum / credited.len() as f64,
        provenance,
        computed_at_unix_ms: now_unix_millis,
        input_observations: credited.len(),
        newest_input_event_time_unix_ms: newest,
        method: SOH_ESTIMATION_METHOD_LABEL,
    };
    // The authority: the freshest credited REPORTED observation (the last
    // one at the newest event time — per the append-ordered ledger this is
    // the latest observation). The reported value stays operative; the
    // estimate is advisory and never replaces it.
    let mut authority_observation = credited[0];
    for observation in &credited {
        if observation.observed_at_unix_ms >= authority_observation.observed_at_unix_ms {
            authority_observation = *observation;
        }
    }
    let authority = SohAuthority::Reported {
        soh_fraction: authority_observation.soh_fraction,
        provenance: authority_observation.provenance,
        event_time_unix_ms: authority_observation.observed_at_unix_ms,
        source: authority_observation.source.clone(),
    };
    // The disagreement record: DATA, produced only when the advisory
    // estimate diverges beyond the configured tolerance.
    let delta = estimate.soh_fraction - authority_observation.soh_fraction;
    let disagreement = if delta.abs() > config.disagreement_tolerance_fraction {
        Some(SohDisagreementRecord {
            reported_soh_fraction: authority_observation.soh_fraction,
            reported_provenance: authority_observation.provenance,
            reported_event_time_unix_ms: authority_observation.observed_at_unix_ms,
            estimated_soh_fraction: estimate.soh_fraction,
            estimate_provenance: estimate.provenance,
            delta_fraction: delta,
            tolerance_fraction: config.disagreement_tolerance_fraction,
            recorded_at_unix_ms: now_unix_millis,
            note: SOH_DISAGREEMENT_STATEMENT,
        })
    } else {
        None
    };
    Ok(SohEstimationOutcome {
        authority,
        estimate: Some(estimate),
        disagreement,
        stale_inputs_excluded,
        unknown_provenance_inputs_excluded,
        ledger_counters_recorded: ledger_counters,
        statement: SOH_AUTHORITY_RULE_STATEMENT,
    })
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bess_degradation::SohObservation;

    fn report(event_ms: i64, provenance: Provenance) -> BmsSohReport {
        BmsSohReport::new(
            0.94,
            event_ms,
            event_ms + 500,
            provenance,
            "bms-7".to_string(),
        )
        .unwrap()
    }

    // --- construction ---------------------------------------------------

    #[test]
    fn report_validates_range_ordering_and_source() {
        assert!(matches!(
            BmsSohReport::new(1.2, 0, 0, Provenance::Measured, "bms".to_string()),
            Err(SohIngestionError::ValueOutOfRange { field: "soh_fraction", .. })
        ));
        assert!(matches!(
            BmsSohReport::new(f64::NAN, 0, 0, Provenance::Measured, "bms".to_string()),
            Err(SohIngestionError::NonFiniteValue { field: "soh_fraction", .. })
        ));
        // ingest before event: refused, never re-stamped.
        assert_eq!(
            BmsSohReport::new(0.9, 1_000, 999, Provenance::Measured, "bms".to_string()),
            Err(SohIngestionError::IngestBeforeEvent {
                event_time_unix_ms: 1_000,
                ingest_time_unix_ms: 999,
            })
        );
        assert!(matches!(
            BmsSohReport::new(0.9, 0, 0, Provenance::Measured, " ".to_string()),
            Err(SohIngestionError::EmptyField { field: "source" })
        ));
    }

    // --- staleness (the P3.13 pattern) -----------------------------------

    #[test]
    fn age_equal_to_max_age_is_not_stale() {
        let config = SohStalenessConfig::new(1_000).unwrap();
        let r = report(10_000, Provenance::Measured);
        // age == max_age exactly: NOT stale (P2.08 convention).
        assert_eq!(
            evaluate_soh_freshness(&r, 11_000, &config),
            SohFreshness::Fresh
        );
    }

    #[test]
    fn age_one_millisecond_beyond_max_age_is_stale() {
        let config = SohStalenessConfig::new(1_000).unwrap();
        let r = report(10_000, Provenance::Measured);
        assert_eq!(
            evaluate_soh_freshness(&r, 11_001, &config),
            SohFreshness::Stale {
                reason: SohStalenessReason::MaxAgeExceeded {
                    age_millis: 1_001,
                    max_age_millis: 1_000,
                }
            }
        );
    }

    #[test]
    fn the_default_bound_is_the_recorded_seven_day_assumption() {
        let config = SohStalenessConfig::default();
        assert_eq!(config.max_age_millis, DEFAULT_SOH_MAX_AGE_MILLIS);
        let r = report(0, Provenance::Measured);
        assert!(matches!(
            evaluate_soh_freshness(&r, DEFAULT_SOH_MAX_AGE_MILLIS + 1, &config),
            SohFreshness::Stale { .. }
        ));
    }

    #[test]
    fn a_clock_before_the_event_time_is_stale_until_resolved() {
        let config = SohStalenessConfig::new(1_000_000).unwrap();
        let r = report(10_000, Provenance::Measured);
        assert_eq!(
            evaluate_soh_freshness(&r, 9_999, &config),
            SohFreshness::Stale {
                reason: SohStalenessReason::ClockBeforeEventTime
            }
        );
    }

    #[test]
    fn max_age_bounds_must_be_positive() {
        assert!(matches!(
            SohStalenessConfig::new(0),
            Err(SohIngestionError::InvalidMaxAge { value: 0 })
        ));
        assert!(matches!(
            SohStalenessConfig::new(-5),
            Err(SohIngestionError::InvalidMaxAge { value: -5 })
        ));
    }

    // --- creditability (P3.02 unknown-narrows) -----------------------------

    #[test]
    fn unknown_provenance_soh_is_never_creditable() {
        let config = SohStalenessConfig::new(1_000).unwrap();
        let r = report(10_000, Provenance::Unknown);
        assert!(!soh_report_creditable(&r, 10_500, &config));
    }

    #[test]
    fn stale_reports_are_never_creditable_even_when_measured() {
        let config = SohStalenessConfig::new(1_000).unwrap();
        let r = report(10_000, Provenance::Measured);
        assert!(!soh_report_creditable(&r, 12_000, &config));
        assert!(soh_report_creditable(&r, 11_000, &config));
    }

    // --- recorded statements -------------------------------------------------

    #[test]
    fn the_deferral_statement_and_survey_are_records_not_implementations() {
        // The recorded supersession: the P15.09 deferral is history, the
        // FIM-F11 estimator is implemented (written-unrun), its calibration
        // is the human gate.
        assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("DEFERRED"));
        assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("WRITTEN-UNRUN"));
        assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("HUMAN LAB GATE"));
        assert!(STALE_SOH_POLICY.contains("never credited"));
        // The five P15.09-surveyed families remain NOT-IMPLEMENTED; the
        // sixth row records the implemented FIM-F11 composition method.
        let surveyed = &SURVEYED_ESTIMATION_METHODS[..SURVEYED_ESTIMATION_METHODS.len() - 1];
        for (method, status) in surveyed {
            assert!(
                status.contains("NOT-IMPLEMENTED"),
                "surveyed method '{method}' must carry the NOT-IMPLEMENTED status"
            );
        }
        let (fim_f11_method, fim_f11_status) = SURVEYED_ESTIMATION_METHODS[SURVEYED_ESTIMATION_METHODS.len() - 1];
        assert!(fim_f11_status.contains("IMPLEMENTED-WRITTEN-UNRUN"));
        assert_eq!(SOH_ESTIMATION_METHOD_LABEL, "RECENCY_BOUNDED_OBSERVATION_COMPOSITION_V1");
        assert!(SOH_AUTHORITY_RULE_STATEMENT.contains("never replaces"));
        assert!(SOH_DISAGREEMENT_STATEMENT.contains("remains the authority"));
        let _ = fim_f11_method;
    }

    // --- FIM-F11 estimator ----------------------------------------------------

    fn observation(soh: f64, at_ms: i64, provenance: Provenance) -> SohObservation {
        SohObservation::new(soh, at_ms, provenance, "bms-7".to_string()).unwrap()
    }

    #[test]
    fn the_estimator_composes_the_mean_with_the_weakest_provenance() {
        let staleness = SohStalenessConfig::new(10_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [
            observation(0.96, 1_000, Provenance::Measured),
            observation(0.94, 2_000, Provenance::Configured),
        ];
        let outcome =
            estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
        let estimate = outcome.estimate.expect("credited inputs exist");
        // Equal-weight mean of 0.96 and 0.94.
        assert!((estimate.soh_fraction - 0.95).abs() < 1e-12);
        // Weakest credited provenance: Measured then Configured -> Configured.
        assert_eq!(estimate.provenance, Provenance::Configured);
        assert_eq!(estimate.input_observations, 2);
        assert_eq!(estimate.newest_input_event_time_unix_ms, 2_000);
        assert_eq!(estimate.method, SOH_ESTIMATION_METHOD_LABEL);
    }

    #[test]
    fn the_estimator_excludes_stale_and_unknown_inputs_and_records_the_exclusions() {
        let staleness = SohStalenessConfig::new(1_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [
            observation(0.96, 1_000, Provenance::Measured),
            observation(0.50, 1_500, Provenance::Unknown), // unknown: never credited
            observation(0.30, 10_000, Provenance::Measured), // age 0 at 10_000: fresh
        ];
        let outcome =
            estimate_soh(&observations, None, 10_000, &staleness, &config).unwrap();
        // 0.96 at 1_000 is stale (age 9_000 > 1_000); 0.50 is
        // UNKNOWN-provenance; 0.30 is fresh and credited.
        assert_eq!(outcome.stale_inputs_excluded, 1);
        assert_eq!(outcome.unknown_provenance_inputs_excluded, 1);
        let estimate = outcome.estimate.expect("one credited input");
        assert!((estimate.soh_fraction - 0.30).abs() < 1e-12);
        assert_eq!(estimate.provenance, Provenance::Measured);
    }

    #[test]
    fn with_no_credited_input_nothing_is_estimated_or_invented() {
        let staleness = SohStalenessConfig::new(1_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [observation(0.96, 1_000, Provenance::Unknown)];
        let outcome =
            estimate_soh(&observations, None, 10_000, &staleness, &config).unwrap();
        assert!(matches!(
            outcome.authority,
            SohAuthority::NotCreditable { .. }
        ));
        assert!(outcome.estimate.is_none());
        assert!(outcome.disagreement.is_none());
        assert_eq!(outcome.unknown_provenance_inputs_excluded, 1);
    }

    #[test]
    fn the_reported_observation_stays_authoritative_and_the_disagreement_is_recorded() {
        let staleness = SohStalenessConfig::new(10_000).unwrap();
        let config = SohEstimationConfig::new(0.01).unwrap();
        let observations = [
            observation(0.96, 1_000, Provenance::Measured),
            observation(0.90, 2_000, Provenance::Measured),
        ];
        let outcome =
            estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
        // Estimate = 0.93; the freshest reported observation 0.90 stays
        // authoritative; |0.93 - 0.90| = 0.03 > 0.01 -> disagreement DATA.
        match &outcome.authority {
            SohAuthority::Reported {
                soh_fraction,
                event_time_unix_ms,
                source,
                ..
            } => {
                assert!((soh_fraction - 0.90).abs() < 1e-12);
                assert_eq!(*event_time_unix_ms, 2_000);
                assert_eq!(source, "bms-7");
            }
            other => panic!("expected a reported authority, got {other:?}"),
        }
        let record = outcome.disagreement.expect("divergence beyond tolerance");
        assert!((record.estimated_soh_fraction - 0.93).abs() < 1e-12);
        assert!((record.reported_soh_fraction - 0.90).abs() < 1e-12);
        assert!((record.delta_fraction - 0.03).abs() < 1e-12);
        assert!((record.tolerance_fraction - 0.01).abs() < 1e-12);
        assert!(record.note.contains("remains the authority"));
    }

    #[test]
    fn agreement_within_tolerance_records_no_disagreement() {
        let staleness = SohStalenessConfig::new(10_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [
            observation(0.94, 1_000, Provenance::Measured),
            observation(0.945, 2_000, Provenance::Measured),
        ];
        let outcome =
            estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
        // Estimate = 0.9425; the freshest reported 0.945; delta 0.0025
        // is within the 0.01 tolerance -> no disagreement record.
        assert!(outcome.disagreement.is_none());
        assert!(matches!(outcome.authority, SohAuthority::Reported { .. }));
    }

    #[test]
    fn ledger_counters_travel_as_recorded_context_only() {
        let staleness = SohStalenessConfig::new(10_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [observation(0.96, 1_000, Provenance::Measured)];
        let counters = SohLedgerCounters {
            full_equivalent_cycles: 1_234.5,
            full_equivalent_cycles_provenance: Provenance::Measured,
            cumulative_throughput_mwh: 24_690.0,
            cumulative_throughput_mwh_provenance: Provenance::Measured,
        };
        let outcome = estimate_soh(&observations, Some(counters), 5_000, &staleness, &config)
            .unwrap();
        let recorded = outcome.ledger_counters_recorded.expect("context travels");
        assert!((recorded.full_equivalent_cycles - 1_234.5).abs() < 1e-9);
        assert!((recorded.cumulative_throughput_mwh - 24_690.0).abs() < 1e-9);
        // The context never enters the arithmetic: the estimate is the
        // observation mean, unchanged by the counters.
        let estimate = outcome.estimate.expect("credited input");
        assert!((estimate.soh_fraction - 0.96).abs() < 1e-12);
    }

    #[test]
    fn the_estimate_staleness_bound_follows_its_newest_input() {
        let staleness = SohStalenessConfig::new(1_000).unwrap();
        let config = SohEstimationConfig::default();
        let observations = [
            observation(0.96, 1_000, Provenance::Measured),
            observation(0.94, 2_000, Provenance::Measured),
        ];
        let outcome =
            estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
        let estimate = outcome.estimate.unwrap();
        // age == max_age exactly is NOT stale (the P2.08 convention).
        assert_eq!(
            evaluate_soh_estimate_freshness(&estimate, 3_000, &staleness),
            SohFreshness::Fresh
        );
        assert!(matches!(
            evaluate_soh_estimate_freshness(&estimate, 3_001, &staleness),
            SohFreshness::Stale { .. }
        ));
        assert!(soh_estimate_creditable(&estimate, 3_000, &staleness));
        assert!(!soh_estimate_creditable(&estimate, 3_001, &staleness));
    }

    #[test]
    fn the_estimator_config_and_inputs_are_validated() {
        let staleness = SohStalenessConfig::new(1_000).unwrap();
        let config = SohEstimationConfig::default();
        assert!(matches!(
            SohEstimationConfig::new(-0.01),
            Err(SohIngestionError::ValueOutOfRange {
                field: "disagreement_tolerance_fraction",
                ..
            })
        ));
        assert!(matches!(
            SohEstimationConfig::new(f64::NAN),
            Err(SohIngestionError::NonFiniteValue {
                field: "disagreement_tolerance_fraction",
                ..
            })
        ));
        let observations = [observation(1.5, 0, Provenance::Measured)];
        assert!(matches!(
            estimate_soh(&observations, None, 0, &staleness, &config),
            Err(SohIngestionError::ValueOutOfRange { field: "soh_fraction", .. })
        ));
    }
}
