// P16.11 — Performance evidence capture: the EVIDENCE RECORD types for
//         market performance. Records only — no score is computed or
//         claimed anywhere.
//
// Phase 16 plan §2 row P16.11; sources: [S-94] (PJM DataMiner2
// reg_market_results — the publication class the precision-score inputs
// reference), [S-95] (PJM regulation precision scoring coverage — the
// post-Oct-2025 consolidated score AS REPORTED; the RegUp/RegDn
// post-Phase-2 change is reported without a verifiable date and is
// marked UNVERIFIED), [S-119] (German TSO prequalification, v1.05 Jul
// 2024 — the FCR 30 s / aFRR 5 min full-response evidence classes),
// [S-120] (ESIG MMS aggregated-DER bids — the aggregation-evidence
// context).
//
// WHAT THIS IS (decided once):
//
// - **Evidence RECORDS, never scores.** Each type below captures one
//   observed fact class with its instruction linkage and provenance.
//   Nothing in this module computes a precision score, a performance
//   metric, or a qualification verdict — scoring belongs to the market
//   operator, and any FlexCore-side aggregation would be a claim this
//   program cannot make (conventions §7.3: market qualification is
//   human-gated; no score is claimed anywhere).
// - **Dates are sourced data with the re-verify-at-use rule** (the
//   phase-16 context timeline rule): the PJM consolidated precision
//   score's post-Oct-2025 change is carried AS REPORTED [S-94][S-95];
//   the RegUp/RegDn post-Phase-2 date is UNVERIFIED and MARKED — every
//   record of that class carries the
//   [`ScoreDateStatus::RegUpRegDnDateUnverified`] status, never a
//   silently-assumed date.
// - **Every evidence record links to its instruction** (the command
//   record reference): signal-following samples, response timestamps,
//   and mileage all carry the instruction/command reference they answer.
//   An evidence record with no instruction linkage is unconstructable
//   where the class demands one.
// - **The German evidence classes carry the DECLARED full-response
//   windows** ([S-119]: FCR 30 s, aFRR 5 min) as re-verify-at-use
//   constants. A latency observation is compared against the window by
//   the CALLER; this module records observations, never verdicts.
//
// Control status: pure record types for the simulation-only 0.x line.
// No evidence has ever been captured from a market (none exists); every
// fixture value in the tests is synthetic. No score is claimed.

use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, StateOfCharge};
use flexcore_domain::ids::AssetId;

use crate::fixed_decimal::FixedDecimal;

/// The performance-evidence schema version (a change is a NEW version).
pub const PERFORMANCE_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// The no-score-claim statement (pinned by tests).
pub const NO_SCORE_CLAIMED_STATEMENT: &str = "No precision score, performance metric, \
     or qualification verdict is computed, claimed, or predicted anywhere in this \
     module: evidence records capture observed facts with their instruction linkage. \
     Scoring belongs to the market operator; the platform's post-Oct-2025 consolidated \
     score posture is 'as reported' [S-94][S-95], the RegUp/RegDn post-Phase-2 date is \
     UNVERIFIED and marked on every record of that class, and market qualification is \
     a human-gated process this program never claims (conventions 7.3).";

/// The date-status rule (pinned by tests).
pub const EVIDENCE_DATE_STATUS_RULE: &str = "Every precision-score input record carries \
     an explicit date status: ReportedAsOf (a sourced, re-verify-at-use date) or \
     RegUpRegDnDateUnverified (the post-Phase-2 RegUp/RegDn scoring change whose date \
     could NOT be verified — carried UNVERIFIED, never silently assumed [S-95]). A \
     record without an explicit date status is unconstructable.";

/// The closed evidence-record-kind vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceRecordKind {
    /// Signal-following telemetry samples linked to an instruction.
    SignalFollowing,
    /// Response timestamps measured against an instruction.
    ResponseTimestamp,
    /// Mileage observations as published by the market's reporting
    /// source (the RMRTS class per [S-94][S-95]; carried as reported,
    /// UNVERIFIED per-source).
    Mileage,
    /// State-of-charge trace points (the SOC fraction in [0, 1], the
    /// canonical-unit rule).
    SocTrace,
    /// Precision-score input references (recorded inputs; no score).
    PrecisionScoreInput,
}

impl EvidenceRecordKind {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            EvidenceRecordKind::SignalFollowing => "signal-following",
            EvidenceRecordKind::ResponseTimestamp => "response-timestamp",
            EvidenceRecordKind::Mileage => "mileage",
            EvidenceRecordKind::SocTrace => "soc-trace",
            EvidenceRecordKind::PrecisionScoreInput => "precision-score-input",
        }
    }
}

/// Why an evidence record was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// A reference field was empty.
    EmptyReference { field: &'static str },
    /// A latency was negative (a response cannot precede its
    /// instruction — the observation is refused, never clamped).
    NegativeLatency { millis: i64 },
    /// The window bounds were inverted.
    InvertedWindow,
    /// A mileage value failed decimal parsing or was negative (mileage
    /// magnitude is non-negative as published; a negative value is a
    /// named refusal, never a sign flip).
    MileageInvalid { reason: String },
    /// A precision-score input carried no input reference at all (an
    /// input record referencing nothing is unconstructable).
    EmptyInputReferenceSet,
}

impl EvidenceError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            EvidenceError::EmptyReference { .. } => "EVIDENCE_EMPTY_REFERENCE",
            EvidenceError::NegativeLatency { .. } => "EVIDENCE_NEGATIVE_LATENCY",
            EvidenceError::InvertedWindow => "EVIDENCE_INVERTED_WINDOW",
            EvidenceError::MileageInvalid { .. } => "EVIDENCE_MILEAGE_INVALID",
            EvidenceError::EmptyInputReferenceSet => "EVIDENCE_EMPTY_INPUT_REFERENCE_SET",
        }
    }
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            EvidenceError::NegativeLatency { millis } => write!(
                f,
                "the response latency is {millis} ms; a response cannot precede its \
                 instruction — the observation is refused, never clamped"
            ),
            EvidenceError::InvertedWindow => {
                write!(f, "the observation window is inverted (start must precede end)")
            }
            EvidenceError::MileageInvalid { reason } => {
                write!(f, "the mileage value is invalid: {reason}")
            }
            EvidenceError::EmptyInputReferenceSet => write!(
                f,
                "a precision-score input record must reference at least one input; \
                 an input record referencing nothing is unconstructable"
            ),
        }
    }
}

impl std::error::Error for EvidenceError {}

/// One signal-following sample: the instruction's setpoint and the
/// observed power at one instant, carried verbatim. NO follow score is
/// computed — the sample is the evidence; scoring belongs to the
/// operator ([`NO_SCORE_CLAIMED_STATEMENT`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalFollowingSample {
    /// The evidence record reference.
    pub evidence_ref: String,
    /// The instruction/command record the sample answers (the P4.14
    /// command record reference; non-empty).
    pub instruction_ref: String,
    /// The asset the sample belongs to.
    pub asset_id: AssetId,
    /// The observation instant.
    pub event_time: EventTime,
    /// The instruction's setpoint, when the instruction declared one.
    pub setpoint_mw: Option<ActivePowerMW>,
    /// The observed power, when the telemetry carried one (a MISSING
    /// observation is carried as `None` — never zero-filled: a missing
    /// sample is evidence of a gap, never evidence of agreement).
    pub observed_mw: Option<ActivePowerMW>,
}

impl SignalFollowingSample {
    /// Validates and constructs one sample (fail-closed).
    pub fn new(
        evidence_ref: String,
        instruction_ref: String,
        asset_id: AssetId,
        event_time: EventTime,
        setpoint_mw: Option<ActivePowerMW>,
        observed_mw: Option<ActivePowerMW>,
    ) -> Result<Self, EvidenceError> {
        if evidence_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "evidence_ref",
            });
        }
        if instruction_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "instruction_ref",
            });
        }
        Ok(Self {
            evidence_ref,
            instruction_ref,
            asset_id,
            event_time,
            setpoint_mw,
            observed_mw,
        })
    }
}

/// The German full-response evidence class (the DECLARED windows per
/// [S-119]; re-verify at use against regelleistung.net v1.05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GermanEvidenceClass {
    /// FCR: full response within 30 seconds ([S-119]).
    Fcr30s,
    /// aFRR: full response within 5 minutes ([S-119]).
    Afrr5min,
}

impl GermanEvidenceClass {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            GermanEvidenceClass::Fcr30s => "german-fcr-30s",
            GermanEvidenceClass::Afrr5min => "german-afrr-5min",
        }
    }

    /// The DECLARED full-response window in milliseconds ([S-119];
    /// re-verify at use). The window is the evidence class's expectation
    /// — a latency observation is compared against it by the CALLER;
    /// this module records observations, never pass/fail verdicts.
    pub const fn window_millis(self) -> i64 {
        match self {
            GermanEvidenceClass::Fcr30s => 30_000,
            GermanEvidenceClass::Afrr5min => 300_000,
        }
    }
}

/// The German FCR full-response window: 30 seconds ([S-119]; re-verify
/// at use).
pub const GERMAN_FCR_WINDOW_MILLIS: i64 = 30_000;
/// The German aFRR full-response window: 5 minutes ([S-119]; re-verify
/// at use).
pub const GERMAN_AFRR_WINDOW_MILLIS: i64 = 300_000;

/// One response-timestamp observation: the instruction time and the
/// observed full-response time, with the measured latency carried
/// verbatim. The latency is a RECORD — the class verdict is the
/// caller's (or the operator's), never this module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseTimestampObservation {
    /// The evidence record reference.
    pub evidence_ref: String,
    /// The instruction reference (non-empty).
    pub instruction_ref: String,
    /// The instruction instant.
    pub instruction_time: EventTime,
    /// The observed full-response instant.
    pub response_time: EventTime,
    /// The measured latency in milliseconds (`response − instruction`,
    /// validated non-negative at construction).
    pub latency_millis: i64,
}

impl ResponseTimestampObservation {
    /// Validates and constructs one observation (fail-closed: non-empty
    /// references, non-negative latency).
    pub fn new(
        evidence_ref: String,
        instruction_ref: String,
        instruction_time: EventTime,
        response_time: EventTime,
    ) -> Result<Self, EvidenceError> {
        if evidence_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "evidence_ref",
            });
        }
        if instruction_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "instruction_ref",
            });
        }
        let latency = response_time.unix_millis() - instruction_time.unix_millis();
        if latency < 0 {
            return Err(EvidenceError::NegativeLatency {
                millis: i64::try_from(latency).unwrap_or(i64::MIN),
            });
        }
        let latency_millis = i64::try_from(latency).unwrap_or(i64::MAX);
        Ok(Self {
            evidence_ref,
            instruction_ref,
            instruction_time,
            response_time,
            latency_millis,
        })
    }
}

/// One mileage observation, carried AS PUBLISHED by the market's
/// reporting source ([S-94][S-95]; the RMRTS reporting class — the
/// source reference is carried verbatim and is UNVERIFIED per-source).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MileageObservation {
    /// The evidence record reference.
    pub evidence_ref: String,
    /// The instruction reference the mileage accrues against (non-empty).
    pub instruction_ref: String,
    /// The observation window start.
    pub window_start: EventTime,
    /// The observation window end.
    pub window_end: EventTime,
    /// The mileage magnitude as published (exact decimal; non-negative —
    /// a negative value is a named refusal, never a sign flip).
    pub mileage: FixedDecimal,
    /// The reporting source reference as published (verbatim; UNVERIFIED
    /// per-source until the RMRTS/reporting check).
    pub reported_by: String,
}

impl MileageObservation {
    /// Validates and constructs one observation (fail-closed).
    pub fn new(
        evidence_ref: String,
        instruction_ref: String,
        window_start: EventTime,
        window_end: EventTime,
        mileage: FixedDecimal,
        reported_by: String,
    ) -> Result<Self, EvidenceError> {
        if evidence_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "evidence_ref",
            });
        }
        if instruction_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "instruction_ref",
            });
        }
        if reported_by.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "reported_by",
            });
        }
        if window_start.unix_millis() >= window_end.unix_millis() {
            return Err(EvidenceError::InvertedWindow);
        }
        if mileage.mantissa() < 0 {
            return Err(EvidenceError::MileageInvalid {
                reason: "the mileage magnitude is negative as published; a negative \
                         magnitude is refused, never sign-flipped"
                    .to_string(),
            });
        }
        Ok(Self {
            evidence_ref,
            instruction_ref,
            window_start,
            window_end,
            mileage,
            reported_by,
        })
    }
}

/// One SOC trace point (the SOC fraction in [0, 1] at the API boundary —
/// the canonical-unit rule holds in evidence records).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocTracePoint {
    /// The evidence record reference.
    pub evidence_ref: String,
    /// The asset the trace point belongs to.
    pub asset_id: AssetId,
    /// The observation instant.
    pub event_time: EventTime,
    /// The SOC fraction (validated in [0, 1] by the units type).
    pub soc: StateOfCharge,
}

impl SocTracePoint {
    /// Validates and constructs one trace point (fail-closed).
    pub fn new(
        evidence_ref: String,
        asset_id: AssetId,
        event_time: EventTime,
        soc: StateOfCharge,
    ) -> Result<Self, EvidenceError> {
        if evidence_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "evidence_ref",
            });
        }
        Ok(Self {
            evidence_ref,
            asset_id,
            event_time,
            soc,
        })
    }
}

/// The explicit date status of a precision-score input record (the
/// honesty rule: a record without an explicit status is
/// unconstructable — [`EVIDENCE_DATE_STATUS_RULE`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreDateStatus {
    /// The scoring change is dated as reported, with the source; the
    /// date is re-verified at use (the phase-16 timeline rule).
    ReportedAsOf { as_of: String, source: String },
    /// The RegUp/RegDn post-Phase-2 scoring change's date could NOT be
    /// verified — carried UNVERIFIED, never silently assumed [S-95].
    RegUpRegDnDateUnverified,
}

impl ScoreDateStatus {
    /// The canonical label spelling (by reference — the status carries
    /// sourced strings and is not `Copy`).
    pub fn as_str(&self) -> &'static str {
        match self {
            ScoreDateStatus::ReportedAsOf { .. } => "reported-as-of",
            ScoreDateStatus::RegUpRegDnDateUnverified => "regup-regdn-date-unverified",
        }
    }
}

/// One precision-score INPUT record: the recorded input references for a
/// market precision-score class (the post-Oct-2025 consolidated score
/// class as reported [S-94][S-95]). This is a RECORD of what would feed
/// the operator's own scoring review — no score is computed, no score is
/// claimed, and no qualification is implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecisionScoreInputRecord {
    /// The evidence record reference.
    pub evidence_ref: String,
    /// The scoring program's label as published (verbatim; e.g. the
    /// consolidated-score program label — UNVERIFIED per-source).
    pub program_label: String,
    /// The explicit date status ([`EVIDENCE_DATE_STATUS_RULE`]).
    pub date_status: ScoreDateStatus,
    /// The input record references (command records, evidence records,
    /// market publications — non-empty set).
    pub input_refs: Vec<String>,
}

impl PrecisionScoreInputRecord {
    /// Validates and constructs one input record (fail-closed: non-empty
    /// references, a non-empty input-reference set, an explicit date
    /// status carried by construction).
    pub fn new(
        evidence_ref: String,
        program_label: String,
        date_status: ScoreDateStatus,
        input_refs: Vec<String>,
    ) -> Result<Self, EvidenceError> {
        if evidence_ref.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "evidence_ref",
            });
        }
        if program_label.trim().is_empty() {
            return Err(EvidenceError::EmptyReference {
                field: "program_label",
            });
        }
        let non_empty: Vec<String> = input_refs
            .into_iter()
            .filter(|r| !r.trim().is_empty())
            .collect();
        if non_empty.is_empty() {
            return Err(EvidenceError::EmptyInputReferenceSet);
        }
        Ok(Self {
            evidence_ref,
            program_label,
            date_status,
            input_refs: non_empty,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture value below is
// SYNTHETIC (no market exists); every expected value is hand-derived
// from the validation rules. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn asset(tag: u128) -> AssetId {
        AssetId::new(uid(tag))
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("valid test time")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(NO_SCORE_CLAIMED_STATEMENT.contains("No precision score"));
        assert!(NO_SCORE_CLAIMED_STATEMENT.contains("UNVERIFIED and marked"));
        assert!(NO_SCORE_CLAIMED_STATEMENT.contains("[S-94][S-95]"));
        assert!(EVIDENCE_DATE_STATUS_RULE.contains("RegUpRegDnDateUnverified"));
        assert!(EVIDENCE_DATE_STATUS_RULE.contains("re-verified at use"));
    }

    #[test]
    fn german_windows_match_the_declared_classes() {
        assert_eq!(GermanEvidenceClass::Fcr30s.window_millis(), GERMAN_FCR_WINDOW_MILLIS);
        assert_eq!(GermanEvidenceClass::Fcr30s.window_millis(), 30_000);
        assert_eq!(GermanEvidenceClass::Afrr5min.window_millis(), GERMAN_AFRR_WINDOW_MILLIS);
        assert_eq!(GermanEvidenceClass::Afrr5min.window_millis(), 300_000);
        assert_eq!(GermanEvidenceClass::Fcr30s.as_str(), "german-fcr-30s");
        assert_eq!(GermanEvidenceClass::Afrr5min.as_str(), "german-afrr-5min");
    }

    #[test]
    fn signal_following_samples_carry_the_instruction_and_never_zero_fill() {
        let sample = SignalFollowingSample::new(
            "EVI-1".to_string(),
            "CMD-REF-1".to_string(),
            asset(0xE001),
            t(1_000),
            Some(ActivePowerMW::new(2.0).expect("finite setpoint")),
            Some(ActivePowerMW::new(1.9).expect("finite observation")),
        )
        .expect("valid sample");
        assert_eq!(sample.instruction_ref, "CMD-REF-1");
        assert_eq!(sample.setpoint_mw.as_ref().map(|p| p.value()), Some(2.0));
        // A MISSING observation is carried as None — never zero-filled.
        let sample = SignalFollowingSample::new(
            "EVI-2".to_string(),
            "CMD-REF-1".to_string(),
            asset(0xE001),
            t(2_000),
            Some(ActivePowerMW::new(2.0).expect("finite setpoint")),
            None,
        )
        .expect("valid sample");
        assert!(sample.observed_mw.is_none());
        // Empty references are named refusals.
        let err = SignalFollowingSample::new(
            "EVI-3".to_string(),
            "  ".to_string(),
            asset(0xE001),
            t(0),
            None,
            None,
        )
        .expect_err("empty instruction ref refused");
        assert_eq!(err.name(), "EVIDENCE_EMPTY_REFERENCE");
    }

    #[test]
    fn response_observations_measure_latency_and_refuse_reversed_time() {
        let observation = ResponseTimestampObservation::new(
            "EVI-4".to_string(),
            "CMD-REF-2".to_string(),
            t(0),
            t(12_000),
        )
        .expect("valid observation");
        assert_eq!(observation.latency_millis, 12_000);
        // A response BEFORE its instruction is refused, never clamped.
        let err = ResponseTimestampObservation::new(
            "EVI-5".to_string(),
            "CMD-REF-2".to_string(),
            t(5_000),
            t(4_000),
        )
        .expect_err("negative latency refused");
        assert_eq!(err.name(), "EVIDENCE_NEGATIVE_LATENCY");
    }

    #[test]
    fn mileage_is_carried_as_published_and_refuses_negatives() {
        let mileage = FixedDecimal::parse("3.75").expect("fixture mileage");
        let observation = MileageObservation::new(
            "EVI-6".to_string(),
            "CMD-REF-3".to_string(),
            t(0),
            t(600_000),
            mileage,
            "fixture://rmrts-report".to_string(),
        )
        .expect("valid observation");
        assert_eq!(observation.mileage.canonical_string(), "3.75");
        let err = MileageObservation::new(
            "EVI-7".to_string(),
            "CMD-REF-3".to_string(),
            t(0),
            t(600_000),
            FixedDecimal::parse("-1.0").expect("negative fixture"),
            "fixture://rmrts-report".to_string(),
        )
        .expect_err("negative mileage refused");
        assert_eq!(err.name(), "EVIDENCE_MILEAGE_INVALID");
        // An inverted window is refused.
        let err = MileageObservation::new(
            "EVI-8".to_string(),
            "CMD-REF-3".to_string(),
            t(600_000),
            t(0),
            FixedDecimal::parse("1.0").expect("fixture"),
            "fixture://rmrts-report".to_string(),
        )
        .expect_err("inverted window refused");
        assert_eq!(err.name(), "EVIDENCE_INVERTED_WINDOW");
    }

    #[test]
    fn soc_trace_points_hold_the_fraction_convention() {
        let point = SocTracePoint::new(
            "EVI-9".to_string(),
            asset(0xE002),
            t(0),
            StateOfCharge::new(0.87).expect("valid fraction"),
        )
        .expect("valid trace point");
        assert_eq!(point.soc.value(), 0.87);
        // Out-of-range SOC is refused by the units type itself.
        assert!(StateOfCharge::new(1.2).is_err());
        let err = SocTracePoint::new(
            " ".to_string(),
            asset(0xE002),
            t(0),
            StateOfCharge::new(0.5).expect("valid fraction"),
        )
        .expect_err("empty evidence ref refused");
        assert_eq!(err.name(), "EVIDENCE_EMPTY_REFERENCE");
    }

    #[test]
    fn precision_score_inputs_carry_explicit_date_status_and_never_a_score() {
        // The consolidated-score class: as reported (re-verify at use).
        let record = PrecisionScoreInputRecord::new(
            "EVI-10".to_string(),
            "SYN-CONSOLIDATED".to_string(),
            ScoreDateStatus::ReportedAsOf {
                as_of: "2025-10".to_string(),
                source: "[S-94][S-95]".to_string(),
            },
            vec!["CMD-REF-1".to_string(), "EVI-1".to_string()],
        )
        .expect("valid input record");
        assert_eq!(record.date_status.as_str(), "reported-as-of");
        assert_eq!(record.input_refs.len(), 2);
        // The RegUp/RegDn post-Phase-2 class: the date is UNVERIFIED and
        // MARKED — this is the only honest status for that class.
        let record = PrecisionScoreInputRecord::new(
            "EVI-11".to_string(),
            "SYN-REGUP-REGDN".to_string(),
            ScoreDateStatus::RegUpRegDnDateUnverified,
            vec!["CMD-REF-2".to_string()],
        )
        .expect("valid input record");
        assert_eq!(record.date_status.as_str(), "regup-regdn-date-unverified");
        // An input record referencing nothing is unconstructable.
        let err = PrecisionScoreInputRecord::new(
            "EVI-12".to_string(),
            "SYN-PROGRAM".to_string(),
            ScoreDateStatus::RegUpRegDnDateUnverified,
            vec![],
        )
        .expect_err("empty input set refused");
        assert_eq!(err.name(), "EVIDENCE_EMPTY_INPUT_REFERENCE_SET");
    }

    #[test]
    fn evidence_kind_labels_are_stable() {
        assert_eq!(EvidenceRecordKind::SignalFollowing.as_str(), "signal-following");
        assert_eq!(EvidenceRecordKind::ResponseTimestamp.as_str(), "response-timestamp");
        assert_eq!(EvidenceRecordKind::Mileage.as_str(), "mileage");
        assert_eq!(EvidenceRecordKind::SocTrace.as_str(), "soc-trace");
        assert_eq!(
            EvidenceRecordKind::PrecisionScoreInput.as_str(),
            "precision-score-input"
        );
    }
}
