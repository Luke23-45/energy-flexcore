// P1.09 — Subject-naming helper for the outbox destination subject.
//
// Part of the flexcore-events P1.09 deliverable. P0.14 §5 explicitly defines
// **no subject naming**: report §15.4 leaves the partition/subject strategy
// to the P1.14 JetStream topology design.
//
// RECONCILED (2026-11-07, coordinator consistency pass): the scheme is no
// longer an assumption. The governing authority is the frozen P1.14 design
// (`ledger/phase-01-rust-foundation/14_jetstream_topology.md`), decisions
// D-J1..D-J3, whose §3 table fixes the complete 26-event mapping:
//
//     flexcore.v1.<area>.<event_slug>
//
// where `<area>` is one of the eleven P1.14 stream/area tokens registry |
// state | telemetry | flexibility | forecast | market | scheduling |
// validation | dispatch | plugins | audit (one area per bounded-context
// stream, D-J3) and `<event_slug>` is the lower snake_case rendering of the
// frozen PascalCase §15.3 event name (D-J1; for example
// `flexcore.v1.registry.asset_registered`,
// `flexcore.v1.dispatch.dispatch_issued`). The P1.14 areas deliberately
// differ from the frozen P0.14 *domain* area labels: registry events
// (AssetRegistered, AssetCapabilityUpdated) leave `asset`, asset state
// (AssetAvailabilityChanged) moves to `state`, schedules map to
// `scheduling`, feasibility checks to `validation`, plugin events to
// `plugins`, and operator actions to `audit` — matching the Phase 1 stream
// filters (FLEXCORE_REGISTRY / FLEXCORE_STATE / FLEXCORE_AUDIT / FLEXCORE_DLQ).
//
// Properties that hold per D-J1/D-J2: every event maps to exactly one
// subject, the 26 subjects are distinct, no tenant identifier appears in
// the subject (no tenant enumeration leak; tenant scoping happens at
// consumption, D-J2), and subjects are concrete (no `*`/`>` wildcards — a
// published subject must never be a wildcard).

use crate::events::{event_name, DomainEvent};
use serde::{Deserialize, Serialize};

/// The versioned prefix of the subject scheme (P1.14 D-J1).
pub const SUBJECT_PREFIX: &str = "flexcore.v1";

/// Errors returned when validating a subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectError {
    /// The subject string is empty.
    Empty,
    /// A dot-separated segment is empty (leading/trailing/doubled dot).
    InvalidSegment {
        /// The index of the offending segment.
        index: usize,
    },
    /// A segment contains a character outside the allowed set
    /// (ASCII alphanumeric, `_`, `-`). This also rejects the NATS wildcard
    /// characters `*` and `>`: the subjects this crate produces are concrete
    /// publish targets, never wildcard patterns.
    InvalidCharacter(char),
}

impl std::fmt::Display for SubjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubjectError::Empty => write!(f, "subject must not be empty"),
            SubjectError::InvalidSegment { index } => {
                write!(f, "subject segment {index} must not be empty")
            }
            SubjectError::InvalidCharacter(c) => {
                write!(
                    f,
                    "subject character '{c}' is not allowed (ASCII alphanumeric, '_', '-'; \
                     wildcard characters are rejected for publish targets)"
                )
            }
        }
    }
}

impl std::error::Error for SubjectError {}

/// A validated NATS subject as an opaque dot-separated token string.
///
/// Validation: non-empty, non-empty segments, ASCII alphanumeric plus `_`
/// and `-` per segment, no wildcard characters. Representation only — this
/// crate performs no broker interaction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Subject(String);

impl Subject {
    /// Validates and wraps a subject string.
    pub fn new(subject: String) -> Result<Self, SubjectError> {
        if subject.is_empty() {
            return Err(SubjectError::Empty);
        }
        for (index, segment) in subject.split('.').enumerate() {
            if segment.is_empty() {
                return Err(SubjectError::InvalidSegment { index });
            }
            for c in segment.chars() {
                if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    return Err(SubjectError::InvalidCharacter(c));
                }
            }
        }
        Ok(Self(subject))
    }

    /// The subject as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Subject {
    type Error = SubjectError;

    fn try_from(subject: String) -> Result<Self, Self::Error> {
        Self::new(subject)
    }
}

impl From<Subject> for String {
    fn from(v: Subject) -> Self {
        v.0
    }
}

impl std::fmt::Display for Subject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The subject area of an event, per the frozen P1.14 stream/area taxonomy
/// (P1.14 §3, D-J1/D-J3): eleven bounded-context areas, one JetStream stream
/// each. These are the P1.14 subject-area tokens, deliberately not the P0.14
/// domain-area labels — e.g. `AssetRegistered` (P0.14 area "Asset") maps to
/// [`EventArea::Registry`], `AssetAvailabilityChanged` (P0.14 area "Asset
/// state") maps to [`EventArea::State`], and `OperatorActionRecorded` maps
/// to [`EventArea::Audit`]. See the module header for the authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventArea {
    /// Asset registry events (`FLEXCORE_REGISTRY`).
    Registry,
    /// Asset state events (`FLEXCORE_STATE`).
    State,
    /// Telemetry events (`FLEXCORE_TELEMETRY`).
    Telemetry,
    /// Flexibility-envelope events (`FLEXCORE_FLEXIBILITY`).
    Flexibility,
    /// Forecasting events (`FLEXCORE_FORECAST`).
    Forecast,
    /// Market events (`FLEXCORE_MARKET`).
    Market,
    /// Scheduling events (`FLEXCORE_SCHEDULING`).
    Scheduling,
    /// Feasibility/validation events (`FLEXCORE_VALIDATION`).
    Validation,
    /// Dispatch-lifecycle events (`FLEXCORE_DISPATCH`).
    Dispatch,
    /// Plugin events (`FLEXCORE_PLUGINS`).
    Plugins,
    /// Audit / operator-action events (`FLEXCORE_AUDIT`).
    Audit,
}

impl EventArea {
    /// The subject token for this area (P1.14 §3).
    pub const fn token(self) -> &'static str {
        match self {
            EventArea::Registry => "registry",
            EventArea::State => "state",
            EventArea::Telemetry => "telemetry",
            EventArea::Flexibility => "flexibility",
            EventArea::Forecast => "forecast",
            EventArea::Market => "market",
            EventArea::Scheduling => "scheduling",
            EventArea::Validation => "validation",
            EventArea::Dispatch => "dispatch",
            EventArea::Plugins => "plugins",
            EventArea::Audit => "audit",
        }
    }
}

/// The subject area of an event, exactly per the frozen P1.14 §3 mapping
/// table (exhaustive over the 26 frozen events).
pub fn event_area(event: &DomainEvent) -> EventArea {
    match event {
        DomainEvent::AssetRegistered(_) => EventArea::Registry,
        DomainEvent::AssetCapabilityUpdated(_) => EventArea::Registry,
        DomainEvent::TelemetryReceived(_) => EventArea::Telemetry,
        DomainEvent::TelemetryQualityChanged(_) => EventArea::Telemetry,
        DomainEvent::AssetAvailabilityChanged(_) => EventArea::State,
        DomainEvent::FlexibilityEnvelopeUpdated(_) => EventArea::Flexibility,
        DomainEvent::ForecastPublished(_) => EventArea::Forecast,
        DomainEvent::MarketSignalReceived(_) => EventArea::Market,
        DomainEvent::ScheduleProposed(_) => EventArea::Scheduling,
        DomainEvent::ScheduleValidated(_) => EventArea::Scheduling,
        DomainEvent::FeasibilityCheckCompleted(_) => EventArea::Validation,
        DomainEvent::DispatchProposed(_)
        | DomainEvent::DispatchApproved(_)
        | DomainEvent::DispatchRejected(_)
        | DomainEvent::DispatchIssued(_)
        | DomainEvent::DispatchAcknowledged(_)
        | DomainEvent::DispatchTrackingUpdated(_)
        | DomainEvent::DispatchCompleted(_)
        | DomainEvent::DispatchFailed(_)
        | DomainEvent::DispatchExpired(_)
        | DomainEvent::DispatchCancelled(_)
        | DomainEvent::DispatchSuperseded(_)
        | DomainEvent::DispatchFallbackActivated(_) => EventArea::Dispatch,
        DomainEvent::PluginActivated(_) | DomainEvent::PluginFailed(_) => EventArea::Plugins,
        DomainEvent::OperatorActionRecorded(_) => EventArea::Audit,
    }
}

/// The destination subject for an event, per the frozen P1.14 scheme
/// `flexcore.v1.<area>.<event_slug>` (P1.14 §3, D-J1 — see the module
/// header).
pub fn subject_for(event: &DomainEvent) -> Subject {
    let subject = format!(
        "{}.{}.{}",
        SUBJECT_PREFIX,
        event_area(event).token(),
        snake_case(event_name(event))
    );
    Subject::new(subject)
        .expect("subjects built from the frozen event names and area tokens are always valid")
}

/// Renders a PascalCase frozen event name as lower snake_case
/// (`AssetRegistered` → `asset_registered`).
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_maps_to_a_valid_prefixed_subject() {
        // P1.14 acceptance rehearsal: every §15.3 event maps to exactly one
        // subject (uniqueness below) and every subject is valid.
        for event in DomainEvent::all_variants().iter() {
            let subject = subject_for(event);
            assert!(
                subject.as_str().starts_with(SUBJECT_PREFIX),
                "subject {} must carry the versioned prefix",
                subject
            );
            let segments: Vec<&str> = subject.as_str().split('.').collect();
            assert_eq!(segments.len(), 4, "scheme is flexcore.v1.<area>.<event>: {subject}");
            assert!(!segments[2].is_empty());
            assert!(!segments[3].is_empty());
        }
    }

    #[test]
    fn all_26_subjects_are_distinct() {
        let subjects: Vec<String> = DomainEvent::all_variants()
            .iter()
            .map(|e| subject_for(e).to_string())
            .collect();
        for (i, a) in subjects.iter().enumerate() {
            for b in subjects.iter().skip(i + 1) {
                assert_ne!(a, b, "every frozen event must map to a distinct subject");
            }
        }
    }

    #[test]
    fn area_mapping_matches_the_frozen_domain_areas() {
        for event in DomainEvent::all_variants().iter() {
            let area = event_area(event).token();
            let name = event_name(event);
            // P1.14 §3 area rules by event-name prefix.
            if name.starts_with("Dispatch") {
                assert_eq!(area, "dispatch");
            }
            if name.starts_with("Telemetry") {
                assert_eq!(area, "telemetry");
            }
            if name.starts_with("AssetRegistered") || name.starts_with("AssetCapabilityUpdated") {
                assert_eq!(area, "registry");
            }
            if name.starts_with("AssetAvailabilityChanged") {
                assert_eq!(area, "state");
            }
            if name.starts_with("Schedule") {
                assert_eq!(area, "scheduling");
            }
            if name.starts_with("Feasibility") {
                assert_eq!(area, "validation");
            }
            if name.starts_with("Plugin") {
                assert_eq!(area, "plugins");
            }
            if name.starts_with("Operator") {
                assert_eq!(area, "audit");
            }
        }
    }

    #[test]
    fn all_26_subjects_match_the_frozen_p1_14_table() {
        // Exact expected subjects, row by row, from the frozen P1.14 §3
        // mapping table (D-J1/D-J3), in the §15.3 event order.
        let expected: [(&str, &str); 26] = [
            ("AssetRegistered", "flexcore.v1.registry.asset_registered"),
            (
                "AssetCapabilityUpdated",
                "flexcore.v1.registry.asset_capability_updated",
            ),
            ("TelemetryReceived", "flexcore.v1.telemetry.telemetry_received"),
            (
                "TelemetryQualityChanged",
                "flexcore.v1.telemetry.telemetry_quality_changed",
            ),
            (
                "AssetAvailabilityChanged",
                "flexcore.v1.state.asset_availability_changed",
            ),
            (
                "FlexibilityEnvelopeUpdated",
                "flexcore.v1.flexibility.flexibility_envelope_updated",
            ),
            ("ForecastPublished", "flexcore.v1.forecast.forecast_published"),
            (
                "MarketSignalReceived",
                "flexcore.v1.market.market_signal_received",
            ),
            ("ScheduleProposed", "flexcore.v1.scheduling.schedule_proposed"),
            (
                "ScheduleValidated",
                "flexcore.v1.scheduling.schedule_validated",
            ),
            (
                "FeasibilityCheckCompleted",
                "flexcore.v1.validation.feasibility_check_completed",
            ),
            ("DispatchProposed", "flexcore.v1.dispatch.dispatch_proposed"),
            ("DispatchApproved", "flexcore.v1.dispatch.dispatch_approved"),
            ("DispatchRejected", "flexcore.v1.dispatch.dispatch_rejected"),
            ("DispatchIssued", "flexcore.v1.dispatch.dispatch_issued"),
            (
                "DispatchAcknowledged",
                "flexcore.v1.dispatch.dispatch_acknowledged",
            ),
            (
                "DispatchTrackingUpdated",
                "flexcore.v1.dispatch.dispatch_tracking_updated",
            ),
            ("DispatchCompleted", "flexcore.v1.dispatch.dispatch_completed"),
            ("DispatchFailed", "flexcore.v1.dispatch.dispatch_failed"),
            ("DispatchExpired", "flexcore.v1.dispatch.dispatch_expired"),
            ("DispatchCancelled", "flexcore.v1.dispatch.dispatch_cancelled"),
            ("DispatchSuperseded", "flexcore.v1.dispatch.dispatch_superseded"),
            (
                "DispatchFallbackActivated",
                "flexcore.v1.dispatch.dispatch_fallback_activated",
            ),
            ("PluginActivated", "flexcore.v1.plugins.plugin_activated"),
            ("PluginFailed", "flexcore.v1.plugins.plugin_failed"),
            (
                "OperatorActionRecorded",
                "flexcore.v1.audit.operator_action_recorded",
            ),
        ];
        let events = DomainEvent::all_variants();
        for (event, (name, subject)) in events.iter().zip(expected.iter()) {
            assert_eq!(event_name(event), *name);
            assert_eq!(subject_for(event).as_str(), *subject);
        }
    }

    #[test]
    fn subjects_never_leak_tenant_identifiers() {
        // The scheme carries no tenant token (no tenant enumeration leak —
        // frozen decision D-J2: tenant scoping happens at consumption).
        for event in DomainEvent::all_variants().iter() {
            let subject = subject_for(event);
            assert!(
                !subject.as_str().contains("tenant"),
                "the subject scheme must not carry tenant tokens: {subject}"
            );
        }
    }

    #[test]
    fn subject_validation_rejects_bad_input() {
        assert_eq!(Subject::new(String::new()), Err(SubjectError::Empty));
        assert_eq!(
            Subject::new("flexcore..v1".to_string()),
            Err(SubjectError::InvalidSegment { index: 1 })
        );
        assert_eq!(
            Subject::new(".flexcore.v1".to_string()),
            Err(SubjectError::InvalidSegment { index: 0 })
        );
        assert_eq!(
            Subject::new("flexcore.v1.registry.asset_*".to_string()),
            Err(SubjectError::InvalidCharacter('*'))
        );
        assert_eq!(
            Subject::new("flexcore.v1.registry.asset_>".to_string()),
            Err(SubjectError::InvalidCharacter('>'))
        );
        assert_eq!(
            Subject::new("flexcore.v1.registry.asset registered".to_string()),
            Err(SubjectError::InvalidCharacter(' '))
        );
    }

    #[test]
    fn subject_serde_round_trips() {
        let subject = subject_for(&DomainEvent::all_variants()[0]);
        let back: Subject = serde_json::from_value(serde_json::to_value(&subject).unwrap())
            .unwrap();
        assert_eq!(back, subject);
    }
}
