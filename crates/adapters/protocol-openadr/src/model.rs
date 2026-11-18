// P14.05 — OpenADR 3.0 model types (the VTN-side object vocabulary).
//
// These types carry the shapes the VTN role exchanges over the REST/JSON
// surface designed in [`crate::endpoints`]. The JSON serialization shapes
// below are DESIGN shapes: field names follow the secondary-source reading
// of the OpenADR 3.0 REST/JSON payloads [S-10] and every protocol-named
// field is UNVERIFIED until checked against the specification text
// (implementation-time human gate; plan §7 note 6 — the openems.rs
// UNVERIFIED-ASSUMPTION pattern). A mismatch found at that gate is a
// recorded edit to this file, never a silent reinterpretation.
//
// Scope notes (phase 14 context §3):
// - Model types only: no wire encoding beyond serde JSON derive, no HTTP,
//   no schema validation of arrival payloads beyond these constructors'
//   own rules (validation of arriving JSON against these types happens at
//   the designed serving seam).
// - The types are the MAPPING INPUT (`event_mapping`), never a dispatch
//   path: nothing here can construct a lifecycle state later than
//   PROPOSED.

use serde::{Deserialize, Serialize};

/// Errors returned by the OpenADR model constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A required identification field is empty.
    EmptyField(&'static str),
    /// A modification number is zero (modifications count from 1 —
    /// UNVERIFIED convention, checked with the 3.0 text at implementation).
    InvalidModificationNumber,
    /// An event has no intervals: an event that defines no delivery
    /// windows cannot be mapped and is refused here rather than guessed.
    EventWithoutIntervals,
    /// An interval has a non-positive or non-finite duration.
    InvalidInterval,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyField(field) => {
                write!(f, "OpenADR model field '{field}' is empty")
            }
            ModelError::InvalidModificationNumber => {
                write!(f, "modification number must be at least 1 (UNVERIFIED convention)")
            }
            ModelError::EventWithoutIntervals => {
                write!(f, "event declares no intervals — refusing rather than guessing a window")
            }
            ModelError::InvalidInterval => {
                write!(f, "interval duration must be finite and positive")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// The signal unit vocabulary the adapter understands for power-shape
/// signals. The mapping (`event_mapping`) converts these into canonical
/// MW at the adapter boundary (`crates/flexcore-contracts/src/units.rs`).
///
/// UNVERIFIED: the exact unit tokens 3.0 payloads carry (and whether
/// per-event unit declarations exist outside the enumeration) are checked
/// against the specification text [S-10] at implementation; an unknown
/// unit arriving on the wire maps to [`SignalUnits::Other`] and is
/// REFUSED by the mapping — never guessed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalUnits {
    /// Kilowatts (the historical OpenADR power unit — UNVERIFIED for 3.0).
    Kilowatts,
    /// Megawatts (canonical platform unit; rarely the wire unit).
    Megawatts,
    /// Amperes.
    Amperes,
    /// A percentage signal (e.g. simple level signals).
    Percent,
    /// Any other unit token: carried verbatim, refused by the mapping.
    Other(String),
}

impl std::fmt::Display for SignalUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalUnits::Kilowatts => write!(f, "kW"),
            SignalUnits::Megawatts => write!(f, "MW"),
            SignalUnits::Amperes => write!(f, "A"),
            SignalUnits::Percent => write!(f, "percent"),
            SignalUnits::Other(token) => write!(f, "{token}"),
        }
    }
}

/// One interval of an event signal: a start time, a duration, and the
/// payload value(s) the VEN should realize over that window.
///
/// Time representation: Unix-epoch MILLISECONDS at the model boundary.
/// The 3.0 wire format carries ISO-8601 date-time strings (UNVERIFIED);
/// parsing that wire format into these models is the designed serving
/// seam's job, and the conversion to the platform's UTC-validating time
/// types (`flexcore_contracts::time_model`) happens in `event_mapping`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// Interval id (protocol-assigned; non-zero — UNVERIFIED whether 3.0
    /// requires ids; a zero id is refused defensively).
    pub id: u64,
    /// Interval start, Unix-epoch milliseconds (UTC).
    pub start_unix_millis: i64,
    /// Interval duration in milliseconds (positive).
    pub duration_millis: i64,
    /// The payload value for this interval (e.g. a power value in the
    /// signal's declared [`SignalUnits`]).
    pub payload_value: f64,
}

impl Interval {
    /// Validates and constructs an interval.
    pub fn new(
        id: u64,
        start_unix_millis: i64,
        duration_millis: i64,
        payload_value: f64,
    ) -> Result<Self, ModelError> {
        if id == 0 {
            return Err(ModelError::InvalidInterval);
        }
        // `duration_millis` is an integer: finiteness is vacuous (every
        // i64 is finite), so the positivity check is the whole rule. The
        // f64 finiteness check below covers the payload.
        if duration_millis <= 0 {
            return Err(ModelError::InvalidInterval);
        }
        if !payload_value.is_finite() {
            return Err(ModelError::InvalidInterval);
        }
        Ok(Self {
            id,
            start_unix_millis,
            duration_millis,
            payload_value,
        })
    }
}

/// One event signal: a named, typed signal with a declared unit and its
/// intervals [S-10] (UNVERIFIED: signal-name/type vocabulary against the
/// 3.0 text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSignal {
    /// Signal name as the program declared it (e.g. a simple load-shift
    /// signal name; carried verbatim — the mapping does not interpret
    /// program-specific names).
    pub signal_name: String,
    /// The declared unit of every interval payload in this signal.
    pub units: SignalUnits,
    /// The intervals, in declaration order.
    pub intervals: Vec<Interval>,
}

impl EventSignal {
    /// Validates and constructs an event signal.
    pub fn new(
        signal_name: String,
        units: SignalUnits,
        intervals: Vec<Interval>,
    ) -> Result<Self, ModelError> {
        if signal_name.trim().is_empty() {
            return Err(ModelError::EmptyField("signal_name"));
        }
        if intervals.is_empty() {
            return Err(ModelError::EventWithoutIntervals);
        }
        Ok(Self {
            signal_name,
            units,
            intervals,
        })
    }
}

/// The event descriptor block [S-10] (UNVERIFIED field set).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventDescriptor {
    /// Program-assigned event id (non-empty).
    pub event_id: String,
    /// Modification number (starts at 1; a later modification of the same
    /// event id SUPERSEDES the earlier candidate in the lifecycle — the
    /// mapping records the pairing and the lifecycle's own SUPERSEDED edge
    /// does the invalidation; the adapter never cancels anything).
    pub modification_number: u32,
    /// Program/market context label, carried verbatim.
    pub market_context: String,
    /// Program-assigned priority (carried verbatim; the platform's own
    /// prioritization is a command-policy concern, NOT decided by the
    /// adapter).
    pub priority: u32,
    /// Whether the event is a test event. Test events are REFUSED by the
    /// mapping by default (they are lab traffic, never fleet control).
    pub test_event: bool,
}

impl EventDescriptor {
    /// Validates and constructs an event descriptor.
    pub fn new(
        event_id: String,
        modification_number: u32,
        market_context: String,
        priority: u32,
        test_event: bool,
    ) -> Result<Self, ModelError> {
        if event_id.trim().is_empty() {
            return Err(ModelError::EmptyField("event_id"));
        }
        if market_context.trim().is_empty() {
            return Err(ModelError::EmptyField("market_context"));
        }
        if modification_number == 0 {
            return Err(ModelError::InvalidModificationNumber);
        }
        Ok(Self {
            event_id,
            modification_number,
            market_context,
            priority,
            test_event,
        })
    }
}

/// An OpenADR event as the VTN holds it [S-10] (UNVERIFIED aggregate
/// shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAdrEvent {
    /// The event descriptor block.
    pub descriptor: EventDescriptor,
    /// The VEN ids the event targets (non-empty; an event with no targets
    /// is refused — there is no fleet-wide implicit target).
    pub target_ven_ids: Vec<String>,
    /// The event's signals (at least one; the mapping uses the FIRST
    /// power-shaped signal and refuses ambiguity when several power
    /// signals are present — see `event_mapping::MappingError`).
    pub signals: Vec<EventSignal>,
    /// The program id the event belongs to (non-empty).
    pub program_id: String,
}

impl OpenAdrEvent {
    /// Validates and constructs an event.
    pub fn new(
        descriptor: EventDescriptor,
        target_ven_ids: Vec<String>,
        signals: Vec<EventSignal>,
        program_id: String,
    ) -> Result<Self, ModelError> {
        if program_id.trim().is_empty() {
            return Err(ModelError::EmptyField("program_id"));
        }
        if target_ven_ids.iter().any(|v| v.trim().is_empty()) {
            return Err(ModelError::EmptyField("target_ven_ids"));
        }
        if target_ven_ids.is_empty() {
            return Err(ModelError::EmptyField("target_ven_ids"));
        }
        if signals.is_empty() {
            return Err(ModelError::EventWithoutIntervals);
        }
        Ok(Self {
            descriptor,
            target_ven_ids,
            signals,
            program_id,
        })
    }
}

/// A report request/payload spec the VTN can ask a VEN for [S-10]
/// (UNVERIFIED field set; report delivery is declared in the scope but the
/// report payload vocabulary is intentionally minimal until a named DR
/// program fixes the reporting requirements).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSpec {
    /// Report specifier id (non-empty).
    pub report_specifier_id: String,
    /// Human-readable report name.
    pub report_name: String,
    /// The reading granularity the VTN requests, in milliseconds.
    pub granularity_millis: i64,
    /// Whether the report backs a REPORT request only (report payloads are
    /// telemetry records entering the platform through the normal
    /// telemetry-ingest ports — NEVER through the command lifecycle).
    pub telemetry_only: bool,
}

impl ReportSpec {
    /// Validates and constructs a report spec.
    pub fn new(
        report_specifier_id: String,
        report_name: String,
        granularity_millis: i64,
    ) -> Result<Self, ModelError> {
        if report_specifier_id.trim().is_empty() {
            return Err(ModelError::EmptyField("report_specifier_id"));
        }
        if report_name.trim().is_empty() {
            return Err(ModelError::EmptyField("report_name"));
        }
        if granularity_millis <= 0 {
            return Err(ModelError::InvalidInterval);
        }
        Ok(Self {
            report_specifier_id,
            report_name,
            granularity_millis,
            // Report payloads are always telemetry-shaped inputs; the flag
            // is fixed true and has no setter (structural, not advisory).
            telemetry_only: true,
        })
    }
}

/// A registered VEN [S-10] (UNVERIFIED: registration payload fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ven {
    /// The VEN id (non-empty; assigned at registration).
    pub ven_id: String,
    /// The VEN name (non-empty).
    pub ven_name: String,
    /// The VEN's push-reception endpoint, when the VEN registered one
    /// (push delivery to a VEN without a declared endpoint is refused by
    /// the delivery planner).
    pub push_endpoint: Option<String>,
    /// The agreed poll frequency in milliseconds (polling mode).
    pub poll_frequency_millis: i64,
    /// Whether the VEN is accepted for event delivery. Registration is a
    /// human-reviewed enrolment record (context §3: enrolment is an
    /// operator decision); the flag is set by the operator surface, never
    /// inferred from protocol traffic.
    pub accepted: bool,
}

impl Ven {
    /// Validates and constructs a VEN record.
    pub fn new(
        ven_id: String,
        ven_name: String,
        push_endpoint: Option<String>,
        poll_frequency_millis: i64,
        accepted: bool,
    ) -> Result<Self, ModelError> {
        if ven_id.trim().is_empty() {
            return Err(ModelError::EmptyField("ven_id"));
        }
        if ven_name.trim().is_empty() {
            return Err(ModelError::EmptyField("ven_name"));
        }
        if poll_frequency_millis <= 0 {
            return Err(ModelError::InvalidInterval);
        }
        if let Some(ep) = &push_endpoint {
            if ep.trim().is_empty() {
                return Err(ModelError::EmptyField("push_endpoint"));
            }
        }
        Ok(Self {
            ven_id,
            ven_name,
            push_endpoint,
            poll_frequency_millis,
            accepted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_requires_targets_program_and_intervals() {
        let descriptor =
            EventDescriptor::new("evt-1".to_string(), 1, "test-market".to_string(), 1, false)
                .expect("valid descriptor");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 0, 900_000, 100.0).expect("valid interval")],
        )
        .expect("valid signal");
        // No targets: refused.
        assert_eq!(
            OpenAdrEvent::new(descriptor.clone(), Vec::new(), vec![signal.clone()], "p1".to_string()),
            Err(ModelError::EmptyField("target_ven_ids"))
        );
        // No signals: refused.
        assert_eq!(
            OpenAdrEvent::new(descriptor.clone(), vec!["ven-1".to_string()], Vec::new(), "p1".to_string()),
            Err(ModelError::EventWithoutIntervals)
        );
        // Valid construction.
        let event = OpenAdrEvent::new(
            descriptor,
            vec!["ven-1".to_string()],
            vec![signal],
            "p1".to_string(),
        )
        .expect("valid event");
        assert_eq!(event.target_ven_ids, vec!["ven-1".to_string()]);
    }

    #[test]
    fn modification_number_starts_at_one() {
        assert_eq!(
            EventDescriptor::new("evt-1".to_string(), 0, "m".to_string(), 1, false),
            Err(ModelError::InvalidModificationNumber)
        );
    }

    #[test]
    fn interval_rejects_non_positive_duration_and_non_finite_payload() {
        assert_eq!(
            Interval::new(1, 0, 0, 1.0),
            Err(ModelError::InvalidInterval)
        );
        assert_eq!(
            Interval::new(1, 0, -5, 1.0),
            Err(ModelError::InvalidInterval)
        );
        assert_eq!(
            Interval::new(1, 0, 1000, f64::NAN),
            Err(ModelError::InvalidInterval)
        );
        assert_eq!(
            Interval::new(0, 0, 1000, 1.0),
            Err(ModelError::InvalidInterval)
        );
    }

    #[test]
    fn signal_requires_name_and_intervals() {
        assert_eq!(
            EventSignal::new("  ".to_string(), SignalUnits::Percent, Vec::new()),
            Err(ModelError::EmptyField("signal_name"))
        );
        assert_eq!(
            EventSignal::new("simple".to_string(), SignalUnits::Percent, Vec::new()),
            Err(ModelError::EventWithoutIntervals)
        );
    }

    #[test]
    fn report_spec_is_structurally_telemetry_only() {
        let spec = ReportSpec::new("r-1".to_string(), "telemetry".to_string(), 60_000)
            .expect("valid report spec");
        assert!(spec.telemetry_only, "report payloads are telemetry records, never commands");
    }

    #[test]
    fn event_json_round_trip_carries_unit_token() {
        let descriptor =
            EventDescriptor::new("evt-1".to_string(), 1, "m".to_string(), 1, false).expect("valid");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 1_700_000_000_000, 900_000, 42.0).expect("valid")],
        )
        .expect("valid");
        let event =
            OpenAdrEvent::new(descriptor, vec!["ven-1".to_string()], vec![signal], "p1".to_string())
                .expect("valid");
        let json = serde_json::to_value(&event).expect("serializes");
        assert_eq!(json["signals"][0]["units"], serde_json::json!("kilowatts"));
        let back: OpenAdrEvent = serde_json::from_value(json).expect("round trip");
        assert_eq!(back, event);
    }
}
