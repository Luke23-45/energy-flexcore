// P15.09 — BMS-reported SoH ingestion vocabulary: provenance + staleness
// (pure domain rules, no I/O).
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 23_p15_09_soh_ingestion.md`.
//
// THE RULE, DECIDED ONCE:
//
//   **SoH is REPORTED data. FlexCore ingests BMS-reported SoH with
//   provenance and staleness bounds; it never estimates SoH itself in this
//   phase.**
//
// - **Platform-side SoH estimation is DEFERRED and recorded as such**
//   ([`SOH_ESTIMATION_DEFERRAL_STATEMENT`]). The estimation-method survey
//   ([`SURVEYED_ESTIMATION_METHODS`]) records the known method families with
//   every one marked NOT-IMPLEMENTED — a survey, never an implementation
//   [S-145][S-146][S-147].
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
/// comment): platform-side SoH estimation is DEFERRED, not implemented.
pub const SOH_ESTIMATION_DEFERRAL_STATEMENT: &str =
    "Platform-side SoH estimation is DEFERRED (P15.09): FlexCore ingests \
BMS-reported SoH only. No estimation method is implemented in this phase; \
the survey below is a record of candidate method families, every one \
NOT-IMPLEMENTED. No SoH value produced or consumed by FlexCore is a \
platform-computed estimate.";

/// The surveyed SoH-estimation method families (P15.09: "estimation methods
/// surveyed, none implemented"). Each entry is `(method family, status)`;
/// every status is the same recorded NOT-IMPLEMENTED string. The survey is a
/// RECORD — the presence of a method here implies no capability claim.
/// Sources: the calendar/cycle aging literature [S-145], the BLAST /
/// BLAST-Lite semi-empirical parameterized models [S-146][S-147].
pub const SURVEYED_ESTIMATION_METHODS: [(&str, &str); 5] = [
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

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("DEFERRED"));
        assert!(STALE_SOH_POLICY.contains("never credited"));
        for (method, status) in SURVEYED_ESTIMATION_METHODS {
            assert!(
                status.contains("NOT-IMPLEMENTED"),
                "surveyed method '{method}' must carry the NOT-IMPLEMENTED status"
            );
        }
    }
}
