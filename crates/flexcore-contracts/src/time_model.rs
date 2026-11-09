// P0.09 — Time model (Phase 0 freeze)
// P1.05 — flexcore-contracts implementation (moved from the Phase 0 seed in
//         flexcore-domain and upgraded from std-only millisecond integers to
//         the `time` crate's `OffsetDateTime` with UTC enforced at
//         construction).
//
// Implements `ledger/phase-00-product-freeze/09_time_model.md`.
// Normative source: report §10.3 (time fields), §14.3 (valid-from/valid-until
// on commands), §17.4 (clock drift); foundations §6.2 Time row governs on
// conflict.
//
// Frozen rules encoded here:
// - `event_time` (when the measurement or event occurred) and `ingest_time`
//   (when FlexCore received it) are **distinct fields and must never be
//   merged or substituted for one another** in any later schema. They are
//   distinct Rust types here, so the merge is a compile error, not a bug.
// - A validity window (`valid_from` / `valid_until`) is carried where
//   applicable; dispatch commands always carry it (report §14.3 invariant 8).
// - A source sequence or version identifier orders values from the source.
// - Clock-quality / synchronization status is a **recorded field, not an
//   assumption** (report §10.3, §17.4 clock-drift threat).
// - FlexCore uses **UTC internally** and retains the original site timezone
//   for operator display and market calendar calculations; local time is
//   never stored as the canonical timestamp. Enforced here: every timestamp
//   type requires the UTC offset (zero offset) **at construction** — a
//   non-UTC offset is rejected with an error, never silently converted. The
//   site timezone is carried separately as `SiteTimezone` metadata.
// - Foundations §6.2 additionally keeps **simulation time separate from
//   wall-clock time** — these types are wall-clock timestamps only;
//   simulation timestamps belong to a separate representation in later
//   phases.
//
// Representation note: the `time` crate (workspace `time` 0.3 with the
// `serde` feature) is used per the P1.05 decision recorded in the root
// `Cargo.toml` header; `serde` round trips are RFC 3339 strings in JSON.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Errors returned by the P0.09 time model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeModelError {
    /// A value cannot be received before it occurred: `ingest_time` must be
    /// greater than or equal to `event_time` where both are present.
    IngestBeforeEvent,
    /// A validity window is inverted: `valid_from` must be less than or equal
    /// to `valid_until`.
    InvalidValidityWindow,
    /// A timestamp was supplied with a non-UTC offset. Contracts require the
    /// UTC offset at construction and never silently convert offsets (the
    /// original site timezone travels separately as `SiteTimezone`).
    NonUtcOffset,
    /// A timestamp could not be constructed from the supplied value (for
    /// example a millisecond value outside the representable range).
    InvalidTimestamp,
    /// Site-timezone metadata was empty or contained whitespace/control
    /// characters.
    InvalidSiteTimezone,
}

impl std::fmt::Display for TimeModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeModelError::IngestBeforeEvent => {
                write!(f, "ingest_time must not precede event_time")
            }
            TimeModelError::InvalidValidityWindow => {
                write!(f, "valid_from must not be after valid_until")
            }
            TimeModelError::NonUtcOffset => {
                write!(f, "timestamp must carry the UTC offset (zero offset); non-UTC offsets are rejected, not converted")
            }
            TimeModelError::InvalidTimestamp => {
                write!(f, "value does not represent a valid timestamp")
            }
            TimeModelError::InvalidSiteTimezone => {
                write!(f, "site timezone metadata must be a non-empty name without whitespace")
            }
        }
    }
}

impl std::error::Error for TimeModelError {}

/// Rejects any timestamp that does not carry the UTC offset (zero offset).
fn require_utc(t: OffsetDateTime) -> Result<OffsetDateTime, TimeModelError> {
    if t.offset().is_utc() {
        Ok(t)
    } else {
        Err(TimeModelError::NonUtcOffset)
    }
}

/// Builds a UTC timestamp from milliseconds since the Unix epoch.
fn utc_from_unix_millis(millis: i64) -> Result<OffsetDateTime, TimeModelError> {
    let nanos = i128::from(millis) * 1_000_000;
    OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| TimeModelError::InvalidTimestamp)
}

// --- The four distinct time newtypes ---------------------------------------
//
// Each type wraps an OffsetDateTime that is guaranteed (by construction) to
// carry the UTC offset. All four share the same shape but are deliberately
// distinct types: no `From`/`Into` conversion exists between them, so
// `event_time` and `ingest_time` (and the validity-window fields) cannot be
// merged or substituted accidentally.

macro_rules! utc_timestamp_newtype {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(into = "OffsetDateTime", try_from = "OffsetDateTime")]
        pub struct $name(OffsetDateTime);

        impl $name {
            /// Constructs the timestamp from an `OffsetDateTime`, requiring
            /// the UTC offset (zero offset) at construction.
            pub fn new(t: OffsetDateTime) -> Result<Self, TimeModelError> {
                require_utc(t).map(Self)
            }

            /// Constructs the timestamp from milliseconds since the Unix
            /// epoch (always UTC by definition).
            pub fn from_unix_millis(millis: i64) -> Result<Self, TimeModelError> {
                utc_from_unix_millis(millis).and_then(Self::new)
            }

            /// The underlying UTC timestamp.
            pub fn timestamp(self) -> OffsetDateTime {
                self.0
            }

            /// Milliseconds since the Unix epoch (UTC), with sub-millisecond
            /// precision truncated downward.
            pub fn unix_millis(self) -> i128 {
                self.0.unix_timestamp_nanos() / 1_000_000
            }
        }

        impl TryFrom<OffsetDateTime> for $name {
            type Error = TimeModelError;

            fn try_from(t: OffsetDateTime) -> Result<Self, Self::Error> {
                Self::new(t)
            }
        }

        impl From<$name> for OffsetDateTime {
            fn from(t: $name) -> Self {
                t.0
            }
        }
    };
}

utc_timestamp_newtype! {
    /// `event_time` — when the measurement or event **occurred** (report
    /// §10.3).
    ///
    /// Distinct from [`IngestTime`], [`ValidFrom`], and [`ValidUntil`] by
    /// construction: the types are not interchangeable, so no later schema
    /// can merge or substitute them accidentally.
    EventTime
}

utc_timestamp_newtype! {
    /// `ingest_time` — when FlexCore **received** the value (report §10.3).
    ///
    /// Distinct from [`EventTime`], [`ValidFrom`], and [`ValidUntil`] by
    /// construction.
    IngestTime
}

utc_timestamp_newtype! {
    /// `valid_from` — start of a validity window (report §10.3; §14.3
    /// invariant 8 for dispatch commands).
    ///
    /// Distinct from [`EventTime`], [`IngestTime`], and [`ValidUntil`] by
    /// construction.
    ValidFrom
}

utc_timestamp_newtype! {
    /// `valid_until` — end of a validity window (report §10.3; §14.3
    /// invariant 8 for dispatch commands).
    ///
    /// Distinct from [`EventTime`], [`IngestTime`], and [`ValidFrom`] by
    /// construction.
    ValidUntil
}

// --- Site timezone metadata -------------------------------------------------

/// The original site timezone, retained as **metadata** (report §10.3 §2) for
/// operator display and market calendar calculations. Local time is never the
/// canonical timestamp — the canonical timestamp types above are always UTC.
///
/// The name is carried verbatim (typically an IANA time-zone name such as
/// `"Europe/Berlin"`); this module performs no IANA interpretation and no DST
/// arithmetic (the frozen DST rule — local days are not always 24 hours —
/// applies to consumers of this metadata, report §10.3 §3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct SiteTimezone(String);

impl SiteTimezone {
    /// Constructs site-timezone metadata from a non-empty name without
    /// whitespace or control characters. The name is recorded verbatim; no
    /// timezone database lookup happens here.
    pub fn new(name: String) -> Result<Self, TimeModelError> {
        if name.is_empty()
            || name
                .chars()
                .any(|c| c.is_whitespace() || c.is_ascii_control())
        {
            return Err(TimeModelError::InvalidSiteTimezone);
        }
        Ok(Self(name))
    }

    /// The recorded timezone name (verbatim).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SiteTimezone {
    type Error = TimeModelError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(name)
    }
}

impl From<SiteTimezone> for String {
    fn from(v: SiteTimezone) -> Self {
        v.0
    }
}

impl std::fmt::Display for SiteTimezone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Validity window --------------------------------------------------------

/// A validity window: `valid_from ≤ valid_until` (validated by
/// [`ValidityWindow::new`], and re-validated on deserialization).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "ValidityWindowDto")]
pub struct ValidityWindow {
    /// Start of the validity window.
    pub valid_from: ValidFrom,
    /// End of the validity window.
    pub valid_until: ValidUntil,
}

impl ValidityWindow {
    /// Constructs a validity window, enforcing `valid_from ≤ valid_until`.
    pub fn new(valid_from: ValidFrom, valid_until: ValidUntil) -> Result<Self, TimeModelError> {
        Self::validate(valid_from, valid_until)?;
        Ok(Self {
            valid_from,
            valid_until,
        })
    }

    /// Validates the window ordering rule: `valid_from ≤ valid_until`.
    pub fn validate(valid_from: ValidFrom, valid_until: ValidUntil) -> Result<(), TimeModelError> {
        if valid_from.timestamp() <= valid_until.timestamp() {
            Ok(())
        } else {
            Err(TimeModelError::InvalidValidityWindow)
        }
    }
}

impl TryFrom<ValidityWindowDto> for ValidityWindow {
    type Error = TimeModelError;

    fn try_from(dto: ValidityWindowDto) -> Result<Self, Self::Error> {
        Self::new(dto.valid_from, dto.valid_until)
    }
}

#[derive(Deserialize)]
struct ValidityWindowDto {
    valid_from: ValidFrom,
    valid_until: ValidUntil,
}

// --- Clock quality ----------------------------------------------------------

/// Clock-quality / synchronization status, **recorded where available, never
/// assumed** (report §10.3, §17.4).
///
/// Clock drift is a threat-model item (report §17.4, controlled by
/// time-quality monitoring, bounded command expiry, and synchronization
/// checks) and a data-quality input; synchronisation status is a recorded
/// field, not an assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClockQuality {
    /// The source clock is known to be synchronized.
    Synchronized,
    /// The source clock is known to be unsynchronized.
    Unsynchronized,
    /// Synchronization status is unknown (the field is recorded as unknown —
    /// it is never silently assumed to be good).
    Unknown,
}

// --- The full time record ---------------------------------------------------

/// One time-dependent record with the frozen §10.3 field vocabulary.
///
/// Fields follow the freeze exactly: `event_time` and `ingest_time` are both
/// mandatory and distinct; the validity window is optional ("where
/// applicable"); the source sequence/version is the ordering identifier from
/// the source; the original site timezone is retained as metadata (never
/// canonical); clock quality is recorded, never assumed.
///
/// Construction is free-form (public fields, frozen seed semantics) and
/// [`TimeRecord::validate`] checks the frozen ordering rules; deserialization
/// routes through the validating path, so a deserialized record always
/// satisfies `validate()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TimeRecordDto")]
pub struct TimeRecord {
    /// When the measurement or event occurred.
    pub event_time: EventTime,
    /// When FlexCore received it.
    pub ingest_time: IngestTime,
    /// Validity window where applicable.
    pub validity: Option<ValidityWindow>,
    /// Source sequence or version — the ordering/version identifier from the
    /// source.
    pub source_sequence: u64,
    /// Original site timezone metadata (e.g. "Europe/Berlin"), retained for
    /// operator display and market calendar calculations. Local time is
    /// never the canonical timestamp.
    pub site_timezone: Option<SiteTimezone>,
    /// Recorded clock-quality / synchronization status.
    pub clock_quality: ClockQuality,
}

impl TimeRecord {
    /// Validates the frozen time rules:
    ///
    /// - `ingest_time ≥ event_time` where both are present (a value cannot
    ///   be received before it occurred);
    /// - `valid_from ≤ valid_until` where a validity window is present.
    pub fn validate(&self) -> Result<(), TimeModelError> {
        if self.ingest_time.timestamp() < self.event_time.timestamp() {
            return Err(TimeModelError::IngestBeforeEvent);
        }
        if let Some(window) = &self.validity {
            ValidityWindow::validate(window.valid_from, window.valid_until)?;
        }
        Ok(())
    }
}

impl TryFrom<TimeRecordDto> for TimeRecord {
    type Error = TimeModelError;

    fn try_from(dto: TimeRecordDto) -> Result<Self, Self::Error> {
        let record = Self {
            event_time: dto.event_time,
            ingest_time: dto.ingest_time,
            validity: dto.validity,
            source_sequence: dto.source_sequence,
            site_timezone: dto.site_timezone,
            clock_quality: dto.clock_quality,
        };
        record.validate()?;
        Ok(record)
    }
}

#[derive(Deserialize)]
struct TimeRecordDto {
    event_time: EventTime,
    ingest_time: IngestTime,
    validity: Option<ValidityWindow>,
    source_sequence: u64,
    site_timezone: Option<SiteTimezone>,
    clock_quality: ClockQuality,
}

/// Validates the ingest/event ordering rule alone: `ingest_time ≥
/// event_time` (a value cannot be received before it occurred).
pub fn validate_ingest_after_event(
    event_time: EventTime,
    ingest_time: IngestTime,
) -> Result<(), TimeModelError> {
    if ingest_time.timestamp() >= event_time.timestamp() {
        Ok(())
    } else {
        Err(TimeModelError::IngestBeforeEvent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::UtcOffset;

    fn utc_millis(millis: i64) -> OffsetDateTime {
        utc_from_unix_millis(millis).unwrap()
    }

    #[test]
    fn ingest_equal_to_event_is_accepted() {
        let e = EventTime::from_unix_millis(1_700_000_000_000).unwrap();
        let i = IngestTime::from_unix_millis(1_700_000_000_000).unwrap();
        assert_eq!(validate_ingest_after_event(e, i), Ok(()));
    }

    #[test]
    fn ingest_after_event_is_accepted() {
        let e = EventTime::from_unix_millis(1_700_000_000_000).unwrap();
        let i = IngestTime::from_unix_millis(1_700_000_000_001).unwrap();
        assert_eq!(validate_ingest_after_event(e, i), Ok(()));
    }

    #[test]
    fn ingest_before_event_is_rejected() {
        // A value cannot be received before it occurred.
        let e = EventTime::from_unix_millis(1_700_000_000_005).unwrap();
        let i = IngestTime::from_unix_millis(1_700_000_000_000).unwrap();
        assert_eq!(
            validate_ingest_after_event(e, i),
            Err(TimeModelError::IngestBeforeEvent)
        );
    }

    #[test]
    fn non_utc_offsets_are_rejected_not_converted() {
        // P1.05 acceptance: UTC enforced at construction — a +02:00 offset
        // is an error, never a silent conversion.
        let non_utc = utc_millis(1_700_000_000_000)
            .replace_offset(UtcOffset::from_hms(2, 0, 0).unwrap());
        assert_eq!(EventTime::new(non_utc), Err(TimeModelError::NonUtcOffset));
        assert_eq!(IngestTime::new(non_utc), Err(TimeModelError::NonUtcOffset));
        assert_eq!(ValidFrom::new(non_utc), Err(TimeModelError::NonUtcOffset));
        assert_eq!(ValidUntil::new(non_utc), Err(TimeModelError::NonUtcOffset));
    }

    #[test]
    fn utc_construction_and_from_unix_millis_agree() {
        let t = utc_millis(1_700_000_000_123);
        let e = EventTime::new(t).unwrap();
        assert_eq!(e.unix_millis(), 1_700_000_000_123);
        assert_eq!(EventTime::from_unix_millis(1_700_000_000_123).unwrap(), e);
        assert!(e.timestamp().offset().is_utc());
    }

    #[test]
    fn from_unix_millis_rejects_out_of_range_values() {
        assert_eq!(
            EventTime::from_unix_millis(i64::MAX),
            Err(TimeModelError::InvalidTimestamp)
        );
    }

    #[test]
    fn validity_window_accepts_from_equal_to_until() {
        let from = ValidFrom::from_unix_millis(100).unwrap();
        let until = ValidUntil::from_unix_millis(100).unwrap();
        assert_eq!(
            ValidityWindow::new(from, until),
            Ok(ValidityWindow {
                valid_from: from,
                valid_until: until
            })
        );
    }

    #[test]
    fn validity_window_accepts_from_before_until() {
        let from = ValidFrom::from_unix_millis(100).unwrap();
        let until = ValidUntil::from_unix_millis(200).unwrap();
        assert!(ValidityWindow::new(from, until).is_ok());
    }

    #[test]
    fn validity_window_rejects_from_after_until() {
        let from = ValidFrom::from_unix_millis(300).unwrap();
        let until = ValidUntil::from_unix_millis(200).unwrap();
        assert_eq!(
            ValidityWindow::new(from, until),
            Err(TimeModelError::InvalidValidityWindow)
        );
    }

    #[test]
    fn time_record_validates_ordering_and_window() {
        let record = TimeRecord {
            event_time: EventTime::from_unix_millis(1_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(2_000).unwrap(),
            validity: Some(
                ValidityWindow::new(
                    ValidFrom::from_unix_millis(500).unwrap(),
                    ValidUntil::from_unix_millis(3_000).unwrap(),
                )
                .unwrap(),
            ),
            source_sequence: 7,
            site_timezone: Some(SiteTimezone::new("Europe/Berlin".to_string()).unwrap()),
            clock_quality: ClockQuality::Synchronized,
        };
        assert_eq!(record.validate(), Ok(()));
    }

    #[test]
    fn time_record_rejects_ingest_before_event() {
        let record = TimeRecord {
            event_time: EventTime::from_unix_millis(2_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(1_000).unwrap(),
            validity: None,
            source_sequence: 7,
            site_timezone: None,
            clock_quality: ClockQuality::Unknown,
        };
        assert_eq!(record.validate(), Err(TimeModelError::IngestBeforeEvent));
    }

    #[test]
    fn time_record_rejects_inverted_window() {
        let record = TimeRecord {
            event_time: EventTime::from_unix_millis(1_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(2_000).unwrap(),
            validity: Some(ValidityWindow {
                valid_from: ValidFrom::from_unix_millis(3_000).unwrap(),
                valid_until: ValidUntil::from_unix_millis(2_500).unwrap(),
            }),
            source_sequence: 7,
            site_timezone: None,
            clock_quality: ClockQuality::Unsynchronized,
        };
        assert_eq!(record.validate(), Err(TimeModelError::InvalidValidityWindow));
    }

    #[test]
    fn time_event_and_ingest_times_are_distinct_types() {
        // event_time and ingest_time must never be merged or substituted for
        // one another (report §10.3). The distinct newtypes make accidental
        // interchange a compile error; this checks they do not unify.
        let e = EventTime::from_unix_millis(5).unwrap();
        let i = IngestTime::from_unix_millis(5).unwrap();
        assert_eq!(e.unix_millis(), i.unix_millis());
        assert_ne!(
            std::any::type_name::<EventTime>(),
            std::any::type_name::<IngestTime>(),
            "event_time and ingest_time must remain distinct types"
        );
        assert_ne!(
            std::any::type_name::<ValidFrom>(),
            std::any::type_name::<ValidUntil>(),
            "valid_from and valid_until must remain distinct types"
        );
    }

    #[test]
    fn clock_quality_unknown_is_recordable() {
        // Synchronization status is a recorded field, never assumed: an
        // unknown status must be representable, not silently defaulted.
        let record = TimeRecord {
            event_time: EventTime::from_unix_millis(1).unwrap(),
            ingest_time: IngestTime::from_unix_millis(2).unwrap(),
            validity: None,
            source_sequence: 1,
            site_timezone: None,
            clock_quality: ClockQuality::Unknown,
        };
        assert_eq!(record.clock_quality, ClockQuality::Unknown);
    }

    #[test]
    fn site_timezone_is_carried_separately_and_validated() {
        let tz = SiteTimezone::new("Europe/Berlin".to_string()).unwrap();
        assert_eq!(tz.as_str(), "Europe/Berlin");
        assert_eq!(SiteTimezone::new(String::new()), Err(TimeModelError::InvalidSiteTimezone));
        assert_eq!(
            SiteTimezone::new("Europe / Berlin".to_string()),
            Err(TimeModelError::InvalidSiteTimezone)
        );
    }

    // --- serde round trips ----------------------------------------------

    #[test]
    fn clock_quality_serde_round_trip() {
        for q in [
            ClockQuality::Synchronized,
            ClockQuality::Unsynchronized,
            ClockQuality::Unknown,
        ] {
            let back: ClockQuality = serde_json::from_value(serde_json::to_value(&q).unwrap())
                .unwrap();
            assert_eq!(back, q);
        }
    }

    #[test]
    fn timestamp_serde_round_trip_is_rfc3339_utc() {
        let e = EventTime::from_unix_millis(1_700_000_000_000).unwrap();
        let json = serde_json::to_value(&e).unwrap();
        let text = json.as_str().unwrap();
        assert!(text.ends_with('Z'), "serialized UTC timestamps end with Z: {text}");
        let back: EventTime = serde_json::from_value(json).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn timestamp_serde_rejects_non_utc_offsets() {
        // A non-UTC RFC 3339 string deserializes into an OffsetDateTime with
        // a +02:00 offset; the validating try_from must reject it.
        let err = serde_json::from_value::<EventTime>(serde_json::json!(
            "2026-01-01T12:00:00+02:00"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("UTC"));
    }

    #[test]
    fn site_timezone_serde_round_trip() {
        let tz = SiteTimezone::new("Europe/Berlin".to_string()).unwrap();
        let back: SiteTimezone = serde_json::from_value(serde_json::to_value(&tz).unwrap())
            .unwrap();
        assert_eq!(back, tz);
    }

    #[test]
    fn validity_window_serde_rejects_inverted_windows() {
        let window = ValidityWindow::new(
            ValidFrom::from_unix_millis(100).unwrap(),
            ValidUntil::from_unix_millis(200).unwrap(),
        )
        .unwrap();
        let back: ValidityWindow = serde_json::from_value(serde_json::to_value(&window).unwrap())
            .unwrap();
        assert_eq!(back, window);

        let err = serde_json::from_value::<ValidityWindow>(serde_json::json!({
            "valid_from": "1970-01-01T00:05:00Z",
            "valid_until": "1970-01-01T00:03:20Z",
        }))
        .unwrap_err();
        assert!(err.to_string().contains("valid_from"));
    }

    #[test]
    fn time_record_serde_round_trip_and_validation() {
        let record = TimeRecord {
            event_time: EventTime::from_unix_millis(1_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(2_000).unwrap(),
            validity: None,
            source_sequence: 42,
            site_timezone: Some(SiteTimezone::new("Europe/Berlin".to_string()).unwrap()),
            clock_quality: ClockQuality::Synchronized,
        };
        let back: TimeRecord = serde_json::from_value(serde_json::to_value(&record).unwrap())
            .unwrap();
        assert_eq!(back, record);

        // A record whose ingest precedes its event never deserializes.
        let err = serde_json::from_value::<TimeRecord>(serde_json::json!({
            "event_time": "1970-01-01T00:00:02Z",
            "ingest_time": "1970-01-01T00:00:01Z",
            "source_sequence": 1,
            "clock_quality": "UNKNOWN",
        }))
        .unwrap_err();
        assert!(err.to_string().contains("ingest_time"));
    }
}
