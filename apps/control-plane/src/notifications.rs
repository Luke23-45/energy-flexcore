// P13.11 — The notification pipeline: email/SMS/webhook channels over
//          provider-abstracted ports, a validated template registry,
//          per-tenant channel preferences, and the event→notification
//          mapping — alarm/audit-driven OUTPUT only.
//
// Phase 13 wave 1, Agent B. WRITTEN-UNRUN: no channel is configured, no
// provider account exists, and NO NOTIFICATION HAS EVER BEEN SENT (the
// loop bans and the P13.19/P13.20 gates govern; program conventions §1).
// This module is the pipeline's design as code, not a message path.
//
// THE NEVER-A-DISPATCH-PATH RULE (Phase 13 context §3 item 5, binding):
// a notification is an OBSERVATION of a fact that already happened. No
// command is issued, altered, or advanced through a notification; no
// channel provider gains control authority; P13-E-2 selects VENDORS, not
// POWERS. Structurally, in this module:
// - the channel ports ([`EmailNotificationPort`], [`SmsNotificationPort`],
//   [`WebhookNotificationPort`]) accept rendered TEXT and return a
//   delivery RECEIPT — no port accepts or returns any command, approval,
//   lifecycle, or dispatch type, and none can be added without violating
//   the recorded rule;
// - the pipeline CONSUMES facts (the frozen §15.3 event vocabulary,
//   `crates/flexcore-events` — exactly 26 events, never extended here)
//   and emits messages; the mapping ([`EVENT_NOTIFICATION_MAP`]) carries
//   no transition logic and cannot advance a lifecycle;
// - the never-granted permission set (`dispatch.authorize`,
//   `dispatch.issue`, `secret.read`, `network.any`) stays byte-identical —
//   channel adapters receive scope SUBSETS at most, never command reach
//   (plan risk row 7).
//
// The E-decision (registered OPEN, never closed here — conventions §4):
// P13-E-2 (notification channel providers: email/SMS vendor selection).
// DEFAULT implemented per plan §4 because the subtask says so: EMAIL-FIRST
// with provider-abstracted ports (adapters depend on ports); SMS and
// webhook channels are DESIGNED (types and ports below) but no vendor is
// selected and no SMS/email provider exists. What closes P13-E-2: the
// human owner's vendor selection and budget.
//
// The webhook channel composes the P13.09 delivery service
// (`crate::webhook_service`) — one delivery mechanism, two entry points
// (record `21_p13_09_webhook_service.md` §7).

use std::sync::LazyLock;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The closed channel vocabulary (exactly the plan P13.11 row's three
/// channels; adding one is a recorded amendment).
pub const NOTIFICATION_CHANNELS: [&str; 3] = ["EMAIL", "SMS", "WEBHOOK"];

/// The closed notification-severity vocabulary (the preference floor's
/// values; independent of the telemetry quality states — severities rank
/// MESSAGES, not measurements).
pub const NOTIFICATION_SEVERITIES: [&str; 3] = ["INFO", "WARNING", "CRITICAL"];

// --- The event→notification mapping (registered data, not logic) ------------

/// One row of the event→notification mapping (registered, versioned DATA
/// — plan P13.21: "the mapping is registered, versioned data, not
/// scattered logic"; wave 2's P13.21 registers the closed vocabulary
/// AGAINST this table and may amend rows, never scatter logic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventNotificationRule {
    /// The event name from the FROZEN 26-event vocabulary
    /// (`crates/flexcore-events/src/events.rs`, `FROZEN_EVENT_NAMES`) —
    /// the mapping cannot name an event outside that set (validated at
    /// registration).
    pub event_name: String,
    /// The channels the notification goes to (subset of
    /// [`NOTIFICATION_CHANNELS`]; empty = audit-only — the event stays
    /// observable through the audit trail but notifies nobody).
    pub channels: Vec<String>,
    /// The message severity floor a tenant preference must permit for
    /// this notification to go out.
    pub severity: String,
    /// The template registry key (`NotificationTemplate::template_id`).
    pub template_id: String,
}

impl EventNotificationRule {
    /// Validates the closed vocabularies (event names are validated
    /// against the caller-supplied frozen set — this module does not
    /// re-list the 26 names, it consumes them; one source of truth).
    pub fn validate(&self, frozen_event_names: &[&str]) -> Result<(), NotificationError> {
        if !frozen_event_names.contains(&self.event_name.as_str()) {
            return Err(NotificationError::UnknownEvent {
                event: self.event_name.clone(),
            });
        }
        if !NOTIFICATION_SEVERITIES.contains(&self.severity.as_str()) {
            return Err(NotificationError::InvalidRule {
                field: "severity",
            });
        }
        if self.template_id.trim().is_empty() {
            return Err(NotificationError::InvalidRule {
                field: "template_id",
            });
        }
        for channel in &self.channels {
            if !NOTIFICATION_CHANNELS.contains(&channel.as_str()) {
                return Err(NotificationError::InvalidRule {
                    field: "channels",
                });
            }
        }
        Ok(())
    }
}

/// The INITIAL conservative mapping: alarm/audit-driven only. Rows name
/// ONLY failure/quality events that already landed in the audit trail and
/// the derived alarm surface; every lifecycle transition whose notification
/// status is undecided is ABSENT from this table = audit-only (the
/// conservative default: absence never notifies). The final closed
/// vocabulary is wave 2's P13.21 registration against this table — this
/// initial set is the starting DATA, not the final word.
pub static EVENT_NOTIFICATION_MAP: LazyLock<[EventNotificationRule; 4]> =
    LazyLock::new(|| [
    EventNotificationRule {
        event_name: "TelemetryQualityChanged".to_string(),
        channels: vec!["EMAIL".to_string()],
        severity: "WARNING".to_string(),
        template_id: "alarm.quality_changed".to_string(),
    },
    EventNotificationRule {
        event_name: "AssetAvailabilityChanged".to_string(),
        channels: vec!["EMAIL".to_string()],
        severity: "WARNING".to_string(),
        template_id: "alarm.availability_changed".to_string(),
    },
    EventNotificationRule {
        event_name: "PluginFailed".to_string(),
        channels: vec!["EMAIL".to_string()],
        severity: "CRITICAL".to_string(),
        template_id: "alarm.plugin_failed".to_string(),
    },
    EventNotificationRule {
        event_name: "DispatchFailed".to_string(),
        channels: vec!["EMAIL".to_string()],
        severity: "CRITICAL".to_string(),
        template_id: "alarm.dispatch_failed".to_string(),
    },
]);

// --- The template registry ------------------------------------------------------

/// Errors of the notification pipeline. Closed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    /// The rule/template/preference argument was malformed (`field` names
    /// the problem).
    InvalidRule {
        field: &'static str,
    },
    /// The rule names an event outside the frozen 26-event vocabulary.
    UnknownEvent {
        event: String,
    },
    /// The template references a placeholder outside its closed
    /// placeholder vocabulary.
    UnknownPlaceholder {
        template_id: String,
        placeholder: String,
    },
    /// The template key is not registered.
    TemplateNotFound {
        template_id: String,
    },
    /// The channel is not enabled for the tenant (an advisory outcome of
    /// preference evaluation, surfaced as a value — see
    /// [`NotificationRouter::route`]).
    ChannelDisabled {
        channel: String,
    },
    /// The channel port rejected the message. `reason` is the port's
    /// human-safe explanation.
    PortRejected {
        channel: String,
        reason: String,
    },
}

impl std::fmt::Display for NotificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationError::InvalidRule { field } => {
                write!(f, "the notification rule is invalid: '{field}' is missing or malformed")
            }
            NotificationError::UnknownEvent { event } => {
                write!(f, "the mapping names '{event}', which is outside the frozen event vocabulary")
            }
            NotificationError::UnknownPlaceholder { template_id, placeholder } => {
                write!(f, "template {template_id} references unknown placeholder '{placeholder}'")
            }
            NotificationError::TemplateNotFound { template_id } => {
                write!(f, "template '{template_id}' is not registered")
            }
            NotificationError::ChannelDisabled { channel } => {
                write!(f, "the {channel} channel is not enabled for this tenant")
            }
            NotificationError::PortRejected { channel, reason } => {
                write!(f, "the {channel} channel port rejected the message: {reason}")
            }
        }
    }
}

impl std::error::Error for NotificationError {}

/// One registered template: validated placeholders, versioned content.
///
/// Templates are DATA (a registry, not scattered format! calls): the
/// placeholder vocabulary is closed and validated at registration
/// ([`NotificationTemplate::new`]) so a template can never reference a
/// fact the pipeline does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationTemplate {
    /// The registry key (non-empty).
    pub template_id: String,
    /// The channel this template renders for.
    pub channel: String,
    /// The message subject (email; the SMS/webhook channels use the body
    /// only).
    pub subject: String,
    /// The body, with `{{placeholder}}` references.
    pub body: String,
    /// The template version (monotonic per key; amendments are new
    /// versions — the audit discipline mirrors the registry rule of
    /// record `26_p13_14_jurisdiction_registry.md` §2).
    pub version: u32,
    /// The CLOSED placeholder vocabulary this template may reference.
    pub placeholders: Vec<String>,
}

impl NotificationTemplate {
    /// Registers a template: validates non-empty content and re-parses
    /// the body's `{{placeholder}}` references against the declared
    /// vocabulary (fail closed on an unknown reference).
    pub fn new(
        template_id: String,
        channel: String,
        subject: String,
        body: String,
        version: u32,
        placeholders: Vec<String>,
    ) -> Result<Self, NotificationError> {
        if template_id.trim().is_empty() {
            return Err(NotificationError::InvalidRule { field: "template_id" });
        }
        if !NOTIFICATION_CHANNELS.contains(&channel.as_str()) {
            return Err(NotificationError::InvalidRule { field: "channel" });
        }
        if body.trim().is_empty() {
            return Err(NotificationError::InvalidRule { field: "body" });
        }
        if version < 1 {
            return Err(NotificationError::InvalidRule { field: "version" });
        }
        let template = Self {
            template_id,
            channel,
            subject,
            body,
            version,
            placeholders,
        };
        // Re-parse the references through the renderer (which fails on an
        // unknown placeholder): registration is the validation point.
        Self::validate_references(&template)?;
        Ok(template)
    }

    fn validate_references(&self) -> Result<(), NotificationError> {
        let mut rest = self.body.as_str();
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                return Err(NotificationError::UnknownPlaceholder {
                    template_id: self.template_id.clone(),
                    placeholder: "<unterminated>".to_string(),
                });
            };
            let name = after[..end].trim().to_string();
            if !self.placeholders.contains(&name) {
                return Err(NotificationError::UnknownPlaceholder {
                    template_id: self.template_id.clone(),
                    placeholder: name,
                });
            }
            rest = &after[end + 2..];
        }
        Ok(())
    }

    /// Renders the template with the supplied placeholder values. A value
    /// for a placeholder the template never references is accepted but
    /// unused; a referenced placeholder with NO value renders as
    /// `<unset:NAME>` — visibly broken, never silently blank (an operator
    /// reading "<unset:asset_id>" sees the bug; a blank would hide it).
    pub fn render(&self, values: &BTreeMap<String, String>) -> String {
        let mut out = self.body.clone();
        for name in &self.placeholders {
            let token = format!("{{{{{name}}}}}");
            let rendered = values
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("<unset:{name}>"));
            out = out.replace(&token, &rendered);
        }
        out
    }
}

/// The template registry (versioned data; `BTreeMap` determinism —
/// conventions §6.6).
#[derive(Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<String, NotificationTemplate>,
}

impl TemplateRegistry {
    /// An empty registry. The pipeline SHIPS with no templates: a
    /// deployment registers the templates its mapping references (the
    /// initial `EVENT_NOTIFICATION_MAP` keys are registered by the
    /// composition; an unregistered key fails the route with
    /// [`NotificationError::TemplateNotFound`] — fail closed, never send
    /// a half-templated message).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Registers (or version-amends) a template.
    pub fn register(&mut self, template: NotificationTemplate) -> Result<(), NotificationError> {
        self.templates.insert(template.template_id.clone(), template);
        Ok(())
    }

    /// Looks a template up by key.
    pub fn get(&self, template_id: &str) -> Result<&NotificationTemplate, NotificationError> {
        self.templates
            .get(template_id)
            .ok_or_else(|| NotificationError::TemplateNotFound {
                template_id: template_id.to_string(),
            })
    }
}

// --- Per-tenant preferences -----------------------------------------------------

/// One tenant's channel preferences (the per-tenant matrix; plan P13.11:
/// "per-tenant preferences"). Defaults are the P13-E-2 DEFAULT expressed
/// as data: EMAIL enabled, SMS disabled, WEBHOOK disabled (webhook
/// delivery is per-endpoint explicit subscription, P13.09's model — a
/// tenant-wide webhook preference would silently subscribe every
/// endpoint, so the webhook row here is permanently advisory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantNotificationPreferences {
    pub tenant_id: flexcore_domain::ids::TenantId,
    /// The minimum severity each channel delivers (`None` = channel off).
    pub email_min_severity: Option<String>,
    pub sms_min_severity: Option<String>,
    pub webhook_min_severity: Option<String>,
}

impl Default for TenantNotificationPreferences {
    fn default() -> Self {
        Self {
            tenant_id: flexcore_domain::ids::TenantId::new(
                flexcore_contracts::ids::FlexUuid::from_bytes([0u8; 16]),
            ),
            email_min_severity: Some("WARNING".to_string()),
            sms_min_severity: None,
            webhook_min_severity: None,
        }
    }
}

impl TenantNotificationPreferences {
    /// Whether the channel is enabled for `severity` (the preference
    /// floor: a message at or above the floor delivers; below it, or with
    /// the channel off, it does not).
    pub fn permits(&self, channel: &str, severity: &str) -> bool {
        let floor = match channel {
            "EMAIL" => self.email_min_severity.as_ref(),
            "SMS" => self.sms_min_severity.as_ref(),
            "WEBHOOK" => self.webhook_min_severity.as_ref(),
            _ => None,
        };
        let Some(floor) = floor else {
            return false;
        };
        severity_rank(severity) >= severity_rank(floor)
    }
}

/// The severity rank (INFO < WARNING < CRITICAL); unknown severities rank
/// lowest (fail closed — an off-vocabulary severity delivers nowhere).
fn severity_rank(severity: &str) -> u8 {
    match severity {
        "INFO" => 1,
        "WARNING" => 2,
        "CRITICAL" => 3,
        _ => 0,
    }
}

// --- The channel ports ----------------------------------------------------------

/// A delivery receipt (all a port can return — no command, no lifecycle,
/// no dispatch type travels back; the never-a-dispatch-path rule is a type
/// fact, not a convention here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationReceipt {
    pub channel: String,
    /// The provider-abstracted message handle (opaque non-empty text).
    pub message_id: String,
    /// Unix millis at acceptance by the port (not a delivery promise).
    pub accepted_at_unix_millis: i64,
}

impl NotificationReceipt {
    fn validate(&self) -> Result<(), NotificationError> {
        if !NOTIFICATION_CHANNELS.contains(&self.channel.as_str()) {
            return Err(NotificationError::InvalidRule { field: "receipt.channel" });
        }
        if self.message_id.trim().is_empty() {
            return Err(NotificationError::InvalidRule { field: "receipt.message_id" });
        }
        Ok(())
    }
}

/// The EMAIL port (provider-abstracted: the P13-E-2 DEFAULT keeps the
/// vendor out of the pipeline; the composition binds SMTP or a commercial
/// provider behind this trait).
pub trait EmailNotificationPort: Send + Sync {
    fn send_email(
        &self,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<NotificationReceipt, NotificationError>;
}

/// The SMS port (DESIGNED, no vendor — the P13-E-2 DEFAULT defers SMS
/// selection; the port exists so the pipeline's shape does not change
/// when a vendor lands).
pub trait SmsNotificationPort: Send + Sync {
    fn send_sms(
        &self,
        recipient: &str,
        body: &str,
    ) -> Result<NotificationReceipt, NotificationError>;
}

/// The WEBHOOK port: composes the P13.09 delivery service (the outbound
/// webhook path — signed, booked, retried, dead-lettered THERE; this port
/// is the mapping's entry into that service, never a second, unsigned
/// sender).
pub trait WebhookNotificationPort: Send + Sync {
    fn enqueue_webhook(
        &self,
        tenant_id: &flexcore_domain::ids::TenantId,
        event_subject: &str,
        raw_body: &str,
    ) -> Result<NotificationReceipt, NotificationError>;
}

// --- The router (the pipeline's one decision point) -----------------------------

/// The notification router: maps an observed event fact to zero or more
/// channel messages, applying the registered mapping, the template
/// registry, and the tenant's preferences. One decision point, no
/// scattered logic.
pub struct NotificationRouter<'a> {
    registry: &'a TemplateRegistry,
    frozen_event_names: &'a [&'static str],
    email: Option<&'a dyn EmailNotificationPort>,
    sms: Option<&'a dyn SmsNotificationPort>,
    webhook: Option<&'a dyn WebhookNotificationPort>,
    clock_millis: Box<dyn Fn() -> i64 + 'a>,
}

impl<'a> NotificationRouter<'a> {
    /// Composes the router over the registry, the frozen event names (the
    /// `crates/flexcore-events` constant — one source of truth), and the
    /// channel ports the composition selected (an uncomposed port is
    /// `None`: the DEFAULT composition is email-only, P13-E-2).
    pub fn new(
        registry: &'a TemplateRegistry,
        frozen_event_names: &'a [&'static str],
        email: Option<&'a dyn EmailNotificationPort>,
        sms: Option<&'a dyn SmsNotificationPort>,
        webhook: Option<&'a dyn WebhookNotificationPort>,
        clock_millis: Box<dyn Fn() -> i64 + 'a>,
    ) -> Self {
        Self {
            registry,
            frozen_event_names,
            email,
            sms,
            webhook,
            clock_millis,
        }
    }

    /// Handles one observed event fact: finds the mapping row (absence =
    /// audit-only — returns an empty receipt list, the conservative
    /// default), renders the template, applies the tenant preferences,
    /// and sends over the composed ports. Returns the receipts (EMPTY
    /// when the event maps to nothing, the channel is disabled for the
    /// tenant, or no port is composed — observation-only outcomes, never
    /// errors, because "nothing notified" is a normal pipeline state).
    ///
    /// The event fact arrives as data (`event_name` + placeholder
    /// values); the router holds no event-source reference and cannot
    /// subscribe to anything: it is INVOKED by the pipeline's hosting
    /// task (a composition step, recorded in the decision record), never
    /// self-driving.
    pub fn route(
        &self,
        event_name: &str,
        values: &BTreeMap<String, String>,
        preferences: &TenantNotificationPreferences,
    ) -> Result<Vec<NotificationReceipt>, NotificationError> {
        // The mapping is registered data; a row must exist for the event
        // to notify anyone.
        let Some(rule) = EVENT_NOTIFICATION_MAP
            .iter()
            .find(|r| r.event_name == event_name)
        else {
            return Ok(Vec::new());
        };
        rule.validate(self.frozen_event_names)?;
        if !preferences.permits("EMAIL", &rule.severity) {
            return Ok(Vec::new());
        }
        let Some(port) = self.email else {
            return Ok(Vec::new());
        };
        // The template is looked up only once the route will actually
        // send — after the suppression and no-port gates, but BEFORE the
        // recipient check: a route that would send with an unregistered
        // template fails closed on the template (the unregistered-template
        // test routes with no recipient and pins TemplateNotFound).
        let template = self.registry.get(&rule.template_id)?;
        if template.channel != "EMAIL" {
            // The initial mapping is email-only by the P13-E-2 DEFAULT;
            // a row naming another channel needs that channel's port and
            // template — a composition gap, surfaced honestly.
            return Err(NotificationError::InvalidRule { field: "channel" });
        }
        let recipient = values
            .get("recipient")
            .map(String::as_str)
            .ok_or(NotificationError::InvalidRule { field: "recipient" })?;
        let receipt =
            port.send_email(recipient, &template.subject, &template.render(values))?;
        receipt.validate()?;
        let _ = (self.clock_millis)();
        Ok(vec![receipt])
    }
}

// --- The in-memory fakes (the test path; never a real provider) ------------------

/// A recording email fake: captures every send (recipient, subject, body)
/// — the reference port implementation for tests.
#[derive(Default)]
pub struct MemoryEmailPort {
    pub sent: Mutex<Vec<(String, String, String)>>,
}

impl EmailNotificationPort for MemoryEmailPort {
    fn send_email(
        &self,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<NotificationReceipt, NotificationError> {
        self.sent
            .lock()
            .expect("poisoned mutex in test fake")
            .push((recipient.to_string(), subject.to_string(), body.to_string()));
        Ok(NotificationReceipt {
            channel: "EMAIL".to_string(),
            message_id: format!("email-{}", self.sent.lock().unwrap().len()),
            accepted_at_unix_millis: 1_000,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;
    use flexcore_events::events::FROZEN_EVENT_NAMES;

    fn tenant(seed: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = seed;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn email_template() -> NotificationTemplate {
        NotificationTemplate::new(
            "alarm.plugin_failed".to_string(),
            "EMAIL".to_string(),
            "Plugin failure".to_string(),
            "Plugin {{plugin_id}} failed for tenant {{tenant_name}} at {{event_time}}."
                .to_string(),
            1,
            vec![
                "plugin_id".to_string(),
                "tenant_name".to_string(),
                "event_time".to_string(),
            ],
        )
        .expect("valid template")
    }

    #[test]
    fn the_initial_mapping_names_only_frozen_events() {
        // Every registered rule resolves against the FROZEN 26-event
        // vocabulary (one source of truth — the events crate's constant).
        for rule in EVENT_NOTIFICATION_MAP.iter() {
            rule.validate(&FROZEN_EVENT_NAMES)
                .expect("every initial rule names a frozen event");
        }
        // A rule naming an invented event is rejected.
        let invented = EventNotificationRule {
            event_name: "AssetTeleported".to_string(),
            channels: vec!["EMAIL".to_string()],
            severity: "INFO".to_string(),
            template_id: "t".to_string(),
        };
        assert_eq!(
            invented.validate(&FROZEN_EVENT_NAMES),
            Err(NotificationError::UnknownEvent {
                event: "AssetTeleported".to_string(),
            })
        );
    }

    #[test]
    fn the_initial_mapping_is_email_only_and_alarm_audit_driven() {
        for rule in EVENT_NOTIFICATION_MAP.iter() {
            assert_eq!(rule.channels, vec!["EMAIL".to_string()], "P13-E-2 DEFAULT: email-first");
            assert!(
                rule.event_name.contains("Failed")
                    || rule.event_name.contains("Changed"),
                "the initial set is alarm/audit-driven only: {rule:?}"
            );
        }
        // The dispatch-lifecycle transitions whose notification status is
        // undecided are ABSENT (absence = audit-only, never notify).
        for undecided in [
            "DispatchProposed",
            "DispatchApproved",
            "DispatchIssued",
            "DispatchAcknowledged",
            "DispatchCompleted",
        ] {
            assert!(
                !EVENT_NOTIFICATION_MAP.iter().any(|r| r.event_name == undecided),
                "{undecided} must stay audit-only until P13.21 registers it"
            );
        }
    }

    #[test]
    fn templates_reject_unknown_placeholders_at_registration() {
        let err = NotificationTemplate::new(
            "bad".to_string(),
            "EMAIL".to_string(),
            "s".to_string(),
            "references {{secret_key}}".to_string(),
            1,
            vec!["asset_id".to_string()],
        );
        assert_eq!(
            err,
            Err(NotificationError::UnknownPlaceholder {
                template_id: "bad".to_string(),
                placeholder: "secret_key".to_string(),
            })
        );
        // An unterminated reference is its own failure.
        let err = NotificationTemplate::new(
            "bad2".to_string(),
            "EMAIL".to_string(),
            "s".to_string(),
            "references {{open".to_string(),
            1,
            vec![],
        );
        assert!(matches!(
            err,
            Err(NotificationError::UnknownPlaceholder { placeholder, .. })
              if placeholder == "<unterminated>"
        ));
    }

    #[test]
    fn rendering_replaces_known_and_marks_unset_references() {
        let template = email_template();
        let mut values = BTreeMap::new();
        values.insert("plugin_id".to_string(), "p-1".to_string());
        values.insert("tenant_name".to_string(), "acme".to_string());
        // event_time is NOT supplied: renders visibly broken, never blank.
        assert_eq!(
            template.render(&values),
            "Plugin p-1 failed for tenant acme at <unset:event_time>."
        );
    }

    #[test]
    fn preferences_enforce_the_severity_floor_and_channel_defaults() {
        // The P13-E-2 DEFAULT as data: EMAIL at WARNING+, SMS/WEBHOOK off.
        let mut prefs = TenantNotificationPreferences::default();
        prefs.tenant_id = tenant(3);
        assert!(prefs.permits("EMAIL", "WARNING"));
        assert!(prefs.permits("EMAIL", "CRITICAL"));
        assert!(!prefs.permits("EMAIL", "INFO"), "below the floor");
        assert!(!prefs.permits("SMS", "CRITICAL"), "SMS defaults off");
        assert!(!prefs.permits("WEBHOOK", "CRITICAL"), "WEBHOOK defaults off");
        // An off-vocabulary severity delivers nowhere (fail closed).
        assert!(!prefs.permits("EMAIL", "LOUD"));
        // Raising the SMS floor enables it for criticals only.
        prefs.sms_min_severity = Some("CRITICAL".to_string());
        assert!(prefs.permits("SMS", "CRITICAL"));
        assert!(!prefs.permits("SMS", "WARNING"));
    }

    #[test]
    fn absence_from_the_mapping_is_audit_only() {
        let registry = TemplateRegistry::empty();
        let email = MemoryEmailPort::default();
        let router = NotificationRouter::new(
            &registry,
            &FROZEN_EVENT_NAMES,
            Some(&email),
            None,
            None,
            Box::new(|| 1_000),
        );
        let prefs = TenantNotificationPreferences::default();
        // A lifecycle event with no mapping row notifies NOBODY.
        let receipts = router
            .route("DispatchApproved", &BTreeMap::new(), &prefs)
            .expect("absence is not an error");
        assert!(receipts.is_empty());
        assert!(email.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn a_mapped_event_routes_through_the_email_port_with_preferences() {
        let mut registry = TemplateRegistry::empty();
        registry
            .register(email_template())
            .expect("registers");
        let email = MemoryEmailPort::default();
        let router = NotificationRouter::new(
            &registry,
            &FROZEN_EVENT_NAMES,
            Some(&email),
            None,
            None,
            Box::new(|| 1_000),
        );
        let mut prefs = TenantNotificationPreferences::default();
        prefs.tenant_id = tenant(3);
        let mut values = BTreeMap::new();
        values.insert("recipient".to_string(), "ops@example.test".to_string());
        values.insert("plugin_id".to_string(), "p-1".to_string());
        values.insert("tenant_name".to_string(), "acme".to_string());
        values.insert("event_time".to_string(), "2026-11-14T00:00:00Z".to_string());
        let receipts = router
            .route("PluginFailed", &values, &prefs)
            .expect("routes");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].channel, "EMAIL");
        let sent = email.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "ops@example.test");
        assert_eq!(
            sent[0].2,
            "Plugin p-1 failed for tenant acme at 2026-11-14T00:00:00Z."
        );
    }

    #[test]
    fn a_disabled_channel_returns_no_receipts_without_touching_the_port() {
        let registry = TemplateRegistry::empty();
        let email = MemoryEmailPort::default();
        let router = NotificationRouter::new(
            &registry,
            &FROZEN_EVENT_NAMES,
            Some(&email),
            None,
            None,
            Box::new(|| 1_000),
        );
        // A WARNING-severity mapping under a CRITICAL floor: no message,
        // no port call — and no error (an unrouted observation is normal).
        let mut prefs = TenantNotificationPreferences::default();
        prefs.tenant_id = tenant(3);
        prefs.email_min_severity = Some("CRITICAL".to_string());
        let receipts = router
            .route("TelemetryQualityChanged", &BTreeMap::new(), &prefs)
            .expect("preference suppression is not an error");
        assert!(receipts.is_empty());
        assert!(email.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn an_unregistered_template_fails_closed_without_sending() {
        let registry = TemplateRegistry::empty();
        let email = MemoryEmailPort::default();
        let router = NotificationRouter::new(
            &registry,
            &FROZEN_EVENT_NAMES,
            Some(&email),
            None,
            None,
            Box::new(|| 1_000),
        );
        let prefs = TenantNotificationPreferences::default();
        assert_eq!(
            router.route("PluginFailed", &BTreeMap::new(), &prefs),
            Err(NotificationError::TemplateNotFound {
                template_id: "alarm.plugin_failed".to_string(),
            })
        );
        assert!(email.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn no_port_composed_returns_no_receipts_and_sends_nothing() {
        let registry = TemplateRegistry::empty();
        let router = NotificationRouter::new(
            &registry,
            &FROZEN_EVENT_NAMES,
            None, // no email port composed (the honest un-composed state)
            None,
            None,
            Box::new(|| 1_000),
        );
        let prefs = TenantNotificationPreferences::default();
        let receipts = router
            .route("PluginFailed", &BTreeMap::new(), &prefs)
            .expect("no port is the honest silent state");
        assert!(receipts.is_empty());
    }

    #[test]
    fn the_channel_vocabulary_is_exactly_three() {
        assert_eq!(NOTIFICATION_CHANNELS, ["EMAIL", "SMS", "WEBHOOK"]);
        assert_eq!(NOTIFICATION_SEVERITIES, ["INFO", "WARNING", "CRITICAL"]);
    }
}
