// P13.21 — the notification/audit interplay vocabulary (Phase 13 wave 2,
// Agent A; WRITTEN-UNRUN).
//
// The closed, registered, VERSIONED vocabulary of which lifecycle
// transitions and alarm-class events EMIT NOTIFICATIONS versus stay
// AUDIT-ONLY — the plan's own sentence, implemented as its own words:
// registered DATA, not scattered logic. One table, one version, one home.
//
// RELATION TO THE P13.11 PIPELINE (record 23, consumed by citation —
// rows, never logic): the pipeline's `EVENT_NOTIFICATION_MAP` (four
// conservative alarm rows; absence = audit-only) is the ROUTING data the
// `NotificationRouter` consumes. THIS module is the REGISTRY layer above
// it: the closed disposition of EVERY frozen event — including the 22
// events whose absence from the map this module now makes EXPLICIT
// instead of implicit. The two must agree where they overlap, and the
// tests pin that agreement row-by-row. Amending the interplay is a
// recorded amendment to this table (a new version constant), never a
// logic change in the router.
//
// THE NEVER-A-DISPATCH-PATH RULE (context §3 item 5): a `Notify`
// disposition routes a MESSAGE; it moves no command, mutates no state,
// and reaches no lifecycle transition. The dispositions here carry
// (event, channels, severity, template) — no handler, no target, no
// state machine. The frozen 14-state / 27-edge command lifecycle and the
// frozen 26-event vocabulary are consumed AS-IS (FROZEN_EVENT_NAMES is
// imported, never re-listed) and stay byte-identical.
//
// THE NEVER-GRANTED PERMISSION SET stays byte-identical (conventions
// §6.4): `dispatch.authorize`, `dispatch.issue`, `secret.read`,
// `network.any` — this module grants nothing to anyone.
//
// Decision record: phase-13 `records/47_p13_21_notification_audit_interplay.md`.

use crate::notifications::{EVENT_NOTIFICATION_MAP, NOTIFICATION_CHANNELS, NOTIFICATION_SEVERITIES};
use flexcore_events::FROZEN_EVENT_NAMES;

/// The registry version. Semantic-versioned DATA: amending a disposition
/// is a recorded amendment that bumps this constant (a patch bump for a
/// rationale-only edit, a minor bump for a disposition change — the
/// recorded convention). The router consumes EVENT_NOTIFICATION_MAP as
/// before; this version tracks the REGISTRY layer.
pub const INTERPLAY_REGISTRY_VERSION: &str = "1.0.0";

/// The closed disposition vocabulary. Exactly two values; extension is a
/// recorded amendment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationDisposition {
    /// The event emits a NOTIFICATION through the mapped channels (the
    /// row must agree with `EVENT_NOTIFICATION_MAP`, pinned by test).
    Notify,
    /// The event is OBSERVABLE THROUGH THE AUDIT TRAIL ONLY — it notifies
    /// nobody. The audit trail remains the evidence of record for every
    /// event regardless of this disposition (audit-only is about the
    /// NOTIFICATION surface, never about deleting or demoting evidence).
    AuditOnly,
}

/// One registry row: the closed disposition of one frozen event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDisposition {
    /// The event name — one of `FROZEN_EVENT_NAMES` (imported, never
    /// re-listed: one source of truth, byte-identical).
    pub event_name: &'static str,
    /// The registered disposition.
    pub disposition: NotificationDisposition,
    /// The channels (empty for AuditOnly rows; for Notify rows the exact
    /// channels `EVENT_NOTIFICATION_MAP` carries).
    pub channels: &'static [&'static str],
    /// The severity (empty for AuditOnly rows; for Notify rows the exact
    /// severity the map carries).
    pub severity: &'static str,
    /// The template id (empty for AuditOnly rows; for Notify rows the
    /// exact template the map carries).
    pub template_id: &'static str,
    /// The registered rationale (rendered in the record; the reason a
    /// human reviewed when the disposition was registered).
    pub rationale: &'static str,
}

/// Convenience constructor for an audit-only row.
const fn audit_only(event_name: &'static str, rationale: &'static str) -> EventDisposition {
    EventDisposition {
        event_name,
        disposition: NotificationDisposition::AuditOnly,
        channels: &[],
        severity: "",
        template_id: "",
        rationale,
    }
}

/// THE REGISTRY (version [`INTERPLAY_REGISTRY_VERSION`]): the disposition
/// of every one of the 26 frozen events, exactly once each (pinned by
/// test). The four Notify rows are the record 23 initial registration
/// verbatim (failure/quality events that already landed in the audit
/// trail and the derived alarm surface); everything else is EXPLICITLY
/// audit-only — the undecided defaults made visible, each with its
/// registered rationale. Nothing here registers a dispatch-adjacent
/// notification: `DispatchFailed` notifies because the failure already
/// happened and is already in the audit trail; no other lifecycle
/// transition notifies (a transition notification would be registered as
/// a deliberate, reviewed row in a recorded amendment — none exists).
pub const EVENT_DISPOSITIONS: [EventDisposition; 26] = [
    // --- Registry/telemetry identity events: the registry IS the record ---
    audit_only(
        "AssetRegistered",
        "the registry record itself is the durable evidence; a registration is not operator-attention-worthy",
    ),
    audit_only(
        "AssetCapabilityUpdated",
        "capability updates are registry bookkeeping; the audit trail is the evidence of record",
    ),
    audit_only(
        "TelemetryReceived",
        "high-frequency ingestion — a notification per receipt would be spam by construction; telemetry QUALITY is what notifies (see TelemetryQualityChanged)",
    ),
    audit_only(
        "MarketSignalReceived",
        "signal arrival is program bookkeeping; consumption decisions surface through their own records",
    ),
    // --- Quality/availability alarm-class events: the Notify rows (the record 23
    //     initial registration, verbatim agreement pinned by test) ---
    EventDisposition {
        event_name: "TelemetryQualityChanged",
        disposition: NotificationDisposition::Notify,
        channels: &["EMAIL"],
        severity: "WARNING",
        template_id: "alarm.quality_changed",
        rationale: "a quality state change is operator-attention evidence (the record 23 initial row)",
    },
    EventDisposition {
        event_name: "AssetAvailabilityChanged",
        disposition: NotificationDisposition::Notify,
        channels: &["EMAIL"],
        severity: "WARNING",
        template_id: "alarm.availability_changed",
        rationale: "an availability change is operator-attention evidence (the record 23 initial row)",
    },
    // --- Planning-phase records: proposals and validations are workflow state ---
    audit_only(
        "FlexibilityEnvelopeUpdated",
        "envelope registration is scheduling-pipeline state; the audit trail is the evidence of record",
    ),
    audit_only(
        "ForecastPublished",
        "a forecast publication is a PROPOSAL input (ADR-010: proposals only) — never operator-attention-worthy by itself",
    ),
    audit_only(
        "ScheduleProposed",
        "a schedule proposal is candidate workflow state; notification would duplicate the console's own surface",
    ),
    audit_only(
        "ScheduleValidated",
        "validation is workflow state; the audit trail is the evidence of record",
    ),
    audit_only(
        "FeasibilityCheckCompleted",
        "check completion is workflow state; the constraint outcome lives in the feasibility record",
    ),
    // --- The dispatch lifecycle transitions: the frozen set, made explicit.
    //     Exactly ONE notifies (the failure that already happened); every
    //     other transition is audit-only — a transition notification is a
    //     deliberate, reviewed row in a recorded amendment, never a default. ---
    audit_only(
        "DispatchProposed",
        "a proposal is workflow state; the approval flow surfaces it in the console (notifications are never a dispatch path)",
    ),
    audit_only(
        "DispatchApproved",
        "an approval is recorded authorization evidence; the audit trail is the proof, a notification would add nothing",
    ),
    audit_only(
        "DispatchRejected",
        "a rejection is recorded evidence with its failing constraint; the console surfaces it; no message leaves the platform",
    ),
    audit_only(
        "DispatchIssued",
        "issue is a lifecycle transition of the frozen set — issuing is the lifecycle's own act, never a notification's",
    ),
    audit_only(
        "DispatchAcknowledged",
        "acknowledgement is edge-side receipt evidence (acknowledgement is not physical delivery, report §14.1); audit-only",
    ),
    audit_only(
        "DispatchTrackingUpdated",
        "tracking deltas are high-frequency operational state — notification would be spam by construction",
    ),
    audit_only(
        "DispatchCompleted",
        "completion is recorded evidence; the audit trail is the proof of record",
    ),
    EventDisposition {
        event_name: "DispatchFailed",
        disposition: NotificationDisposition::Notify,
        channels: &["EMAIL"],
        severity: "CRITICAL",
        template_id: "alarm.dispatch_failed",
        rationale: "a FAILED dispatch is operator-attention evidence of a fact that already happened (the record 23 initial row; the failure is already in the audit trail — the notification neither causes nor alters it)",
    },
    audit_only(
        "DispatchExpired",
        "expiry is a recorded lifecycle outcome; the audit trail is the evidence of record",
    ),
    audit_only(
        "DispatchCancelled",
        "cancellation is a recorded lifecycle outcome; the audit trail is the evidence of record",
    ),
    audit_only(
        "DispatchSuperseded",
        "supersession is a recorded lifecycle outcome; the audit trail is the evidence of record",
    ),
    audit_only(
        "DispatchFallbackActivated",
        "fallback activation is recorded safety evidence; the audit trail is the proof, and the recorded denial/alarm surface already carries it",
    ),
    // --- Plugin events ---
    EventDisposition {
        event_name: "PluginFailed",
        disposition: NotificationDisposition::Notify,
        channels: &["EMAIL"],
        severity: "CRITICAL",
        template_id: "alarm.plugin_failed",
        rationale: "a plugin failure is platform-health operator-attention evidence (the record 23 initial row)",
    },
    audit_only(
        "PluginActivated",
        "activation is registry bookkeeping; the audit trail is the evidence of record",
    ),
    // --- Operator events ---
    audit_only(
        "OperatorActionRecorded",
        "the operator action record IS the evidence; notifying about the record of an action would double-document it",
    ),
];

// --- The registry integrity invariants (checked in-module; the tests pin them) ------

/// Validates the registry invariants over the imported frozen names and
/// the P13.11 map: every frozen event appears exactly once; no name
/// outside the frozen set; every Notify row agrees with its
/// `EVENT_NOTIFICATION_MAP` counterpart exactly; every AuditOnly row
/// carries empty routing fields. Returns the first violated invariant as
/// a readable string (empty = all hold).
pub fn validate_registry() -> Result<(), String> {
    if EVENT_DISPOSITIONS.len() != FROZEN_EVENT_NAMES.len() {
        return Err(format!(
            "the registry covers {} events; the frozen vocabulary has {}",
            EVENT_DISPOSITIONS.len(),
            FROZEN_EVENT_NAMES.len()
        ));
    }
    for (index, row) in EVENT_DISPOSITIONS.iter().enumerate() {
        if !FROZEN_EVENT_NAMES.contains(&row.event_name) {
            return Err(format!(
                "row {index} names '{}' which is not in the frozen 26-event vocabulary",
                row.event_name
            ));
        }
        match row.disposition {
            NotificationDisposition::Notify => {
                let mapped = EVENT_NOTIFICATION_MAP
                    .iter()
                    .find(|rule| rule.event_name == row.event_name);
                let mapped = match mapped {
                    Some(rule) => rule,
                    None => {
                        return Err(format!(
                            "'{}' is Notify but has no EVENT_NOTIFICATION_MAP row",
                            row.event_name
                        ))
                    }
                };
                let mapped_channels: Vec<&str> =
                    mapped.channels.iter().map(String::as_str).collect();
                if row.channels != mapped_channels.as_slice()
                    || row.severity != mapped.severity
                    || row.template_id != mapped.template_id
                {
                    return Err(format!(
                        "'{}' disagrees with its EVENT_NOTIFICATION_MAP row",
                        row.event_name
                    ));
                }
                if row.channels.is_empty()
                    || !row
                        .channels
                        .iter()
                        .all(|channel| NOTIFICATION_CHANNELS.contains(channel))
                {
                    return Err(format!("'{}' carries an unknown channel", row.event_name));
                }
                if !NOTIFICATION_SEVERITIES.contains(&row.severity) {
                    return Err(format!("'{}' carries an unknown severity", row.event_name));
                }
            }
            NotificationDisposition::AuditOnly => {
                if !row.channels.is_empty() || !row.severity.is_empty() || !row.template_id.is_empty() {
                    return Err(format!(
                        "'{}' is AuditOnly but carries routing fields",
                        row.event_name
                    ));
                }
                if EVENT_NOTIFICATION_MAP
                    .iter()
                    .any(|rule| rule.event_name == row.event_name)
                {
                    return Err(format!(
                        "'{}' is AuditOnly but EVENT_NOTIFICATION_MAP routes it",
                        row.event_name
                    ));
                }
            }
        }
    }
    // No duplicates: every frozen name exactly once.
    for name in FROZEN_EVENT_NAMES {
        let count = EVENT_DISPOSITIONS
            .iter()
            .filter(|row| row.event_name == *name)
            .count();
        if count != 1 {
            return Err(format!("'{name}' appears {count} times (exactly once required)"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_covers_every_frozen_event_exactly_once() {
        validate_registry().expect("the registry invariants hold");
        assert_eq!(EVENT_DISPOSITIONS.len(), 26);
        for name in FROZEN_EVENT_NAMES {
            assert_eq!(
                EVENT_DISPOSITIONS
                    .iter()
                    .filter(|row| row.event_name == *name)
                    .count(),
                1,
                "'{name}' appears exactly once"
            );
        }
    }

    #[test]
    fn exactly_four_events_notify_and_they_are_the_record_23_rows() {
        let notify: Vec<&str> = EVENT_DISPOSITIONS
            .iter()
            .filter(|row| row.disposition == NotificationDisposition::Notify)
            .map(|row| row.event_name)
            .collect();
        assert_eq!(
            notify,
            vec!["TelemetryQualityChanged", "AssetAvailabilityChanged", "PluginFailed", "DispatchFailed"],
            "the Notify set is exactly the record 23 initial registration"
        );
        assert_eq!(
            EVENT_NOTIFICATION_MAP.len(),
            notify.len(),
            "the map and the registry agree in size"
        );
    }

    #[test]
    fn every_dispatch_transition_except_the_failure_is_audit_only() {
        // The frozen lifecycle's transitions, made explicit: only
        // DispatchFailed notifies. Notifications are never a dispatch
        // path, and a transition notification would be a deliberate
        // reviewed amendment — none exists.
        for name in [
            "DispatchProposed",
            "DispatchApproved",
            "DispatchRejected",
            "DispatchIssued",
            "DispatchAcknowledged",
            "DispatchTrackingUpdated",
            "DispatchCompleted",
            "DispatchExpired",
            "DispatchCancelled",
            "DispatchSuperseded",
            "DispatchFallbackActivated",
        ] {
            let row = EVENT_DISPOSITIONS
                .iter()
                .find(|row| row.event_name == name)
                .expect("the transition is in the registry");
            assert_eq!(
                row.disposition,
                NotificationDisposition::AuditOnly,
                "'{name}' is audit-only"
            );
        }
        let failed = EVENT_DISPOSITIONS
            .iter()
            .find(|row| row.event_name == "DispatchFailed")
            .expect("the failure is in the registry");
        assert_eq!(failed.disposition, NotificationDisposition::Notify);
        assert!(
            failed.rationale.contains("already happened"),
            "the failure notifies as an OBSERVED fact, never as a dispatch act"
        );
    }

    #[test]
    fn the_registry_version_is_a_recorded_constant() {
        assert_eq!(INTERPLAY_REGISTRY_VERSION, "1.0.0");
        // The version is DATA (bumped by recorded amendments), not logic.
        assert!(INTERPLAY_REGISTRY_VERSION.split('.').count() == 3);
    }

    #[test]
    fn the_frozen_vocabulary_is_consumed_not_re_listed() {
        // One source of truth: the registry length is the imported
        // constant's length — if the frozen set were ever amended (it may
        // not be), this module could not silently diverge.
        assert_eq!(EVENT_DISPOSITIONS.len(), FROZEN_EVENT_NAMES.len());
    }
}
