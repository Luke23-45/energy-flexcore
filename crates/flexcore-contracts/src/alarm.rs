// P5.31 — The minimal alarm model (ADDITIVE; report §20.1 item 1 "key
//         alarms", §15.3, §20.2; ledger phase-5 P5.31).
//
// This module resolves the alarm-contract gap P5.02 recorded (no alarm
// model existed in any prior phase; the frozen §15.3 event vocabulary
// carries no alarm event) by DEFINING the minimal model the Overview
// workspace displays — the ledger's preferred resolution over escalation.
//
// The rules this module encodes, exactly:
//
// - **Alarms are operator-attention records, NOT protection signals.**
//   An alarm record never triggers, inhibits, or supervises any physical
//   action; it carries no protection-grade semantics and no safety-case
//   weight of any kind (AGENTS.md rules 2–3; report §21.4: the 0.x line is
//   simulation-only). [`ALARM_MODEL_STATEMENT`] is the verbatim statement
//   every consumer (API response, console panel) renders.
// - **The frozen §15.3 event vocabulary is NOT extended.** The 26-event
//   freeze (P0.14, `flexcore-events::EVENT_COUNT == 26`) stays exactly as
//   frozen: no `AlarmRaised`/`AlarmCleared` event exists and none is added.
//   Alarms are DERIVED records — derived deterministically from existing
//   platform facts (telemetry quality states, dispatch outcomes,
//   feasibility verdicts; `flexcore-domain::alarm_rules`), never invented
//   by the console, never platform events of their own (ledger P5.31 step
//   2: "Alarms are derived from events (§15.3), never invented by the
//   console").
// - **Derived on read in 0.x (recorded decision, DEFAULT):** no migration,
//   no persistence, no alarm table. A derived-on-read contract cannot know
//   that a condition ended (no cleared-at observation) or that an operator
//   acknowledged anything — so the served records are exactly the alarms
//   derivable from the CURRENT platform facts at read time, and the
//   [`AlarmState`] vocabulary keeps `Acknowledged`/`Cleared` as defined
//   values for the persistence/acknowledgement path a later phase owns.
//   Recorded honestly in the P5.31 decision record
//   (`ledger/phase-05-operator-console-simulator/07_31_market_plugins_admin_alarms.md`).
// - **Closed vocabularies (labelled DEFAULT — this wave's choice, reviewable
//   like every Phase 5 DEFAULT):** [`AlarmSeverity`] (3 values) and
//   [`AlarmCode`] (7 values, one per alarm source that EXISTS by Phase 5 —
//   the ledger P5.31 step-1 inventory). Edge/adapter health (P6.17) does
//   NOT exist yet and has NO code: no invented source.
// - **event_time vs ingest_time stay distinct (P0.09):** `raised_at` is the
//   `event_time` of the source observation; the derivation instant is
//   `derived_at` and is never merged with either.
// - **Deterministic identity:** a derived alarm has no persisted UUID; its
//   `alarm_id` is the deterministic composite key built by the derivation
//   rules (`flexcore:alarm:<CODE>:<scope>`), so two derivations from the
//   same facts produce the SAME record (the P5.33-adjacent dedup property
//   at zero cost).

use crate::ids::FlexUuid;
use crate::time_model::EventTime;
use serde::{Deserialize, Serialize};

/// The alarm-model version identifier (rendered by every consumer so a
/// display can never silently present a different model than the one that
/// was reviewed).
pub const ALARM_MODEL_VERSION: &str = "ALARM-MODEL-0.1.0 (P5.31)";

/// The verbatim alarm-model statement. Rendered, never paraphrased, by the
/// API response and the console panels.
pub const ALARM_MODEL_STATEMENT: &str = "Alarms are OPERATOR-ATTENTION records derived deterministically from existing platform facts (telemetry quality states, dispatch outcomes, feasibility verdicts). They are NOT protection signals and carry no protection-grade or safety semantics: an alarm never triggers, inhibits, or supervises any physical action. The frozen §15.3 event vocabulary is not extended — no alarm event exists; alarms are derived records (derived on read in the 0.x simulation line), never invented by the console.";

// --- Severity (closed; DEFAULT) -------------------------------------------------

/// The closed alarm-severity vocabulary (DEFAULT, P5.31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlarmSeverity {
    /// `INFO` — attention-worthy but not urgent (e.g. a stale telemetry value).
    Info,
    /// `WARNING` — a data or evidence condition an operator should act on.
    Warning,
    /// `CRITICAL` — an operator-attention emergency IN THE SIMULATION LINE
    /// ONLY: a dispatch or fallback condition in the simulated lifecycle.
    /// This is a display-priority label, never a protection or safety claim.
    Critical,
}

impl AlarmSeverity {
    /// All three severities, in display order (lowest first).
    pub const ALL: [AlarmSeverity; 3] = [
        AlarmSeverity::Info,
        AlarmSeverity::Warning,
        AlarmSeverity::Critical,
    ];

    /// The canonical SCREAMING_SNAKE name.
    pub const fn name(self) -> &'static str {
        match self {
            AlarmSeverity::Info => "INFO",
            AlarmSeverity::Warning => "WARNING",
            AlarmSeverity::Critical => "CRITICAL",
        }
    }

    /// The display rank (higher = more urgent); used for deterministic
    /// ordering of derived lists.
    pub const fn rank(self) -> u8 {
        match self {
            AlarmSeverity::Info => 0,
            AlarmSeverity::Warning => 1,
            AlarmSeverity::Critical => 2,
        }
    }
}

// --- Code (closed, one per EXISTING source; DEFAULT) -----------------------------

/// The closed alarm-code vocabulary (DEFAULT, P5.31): exactly one code per
/// alarm source that EXISTS by Phase 5 (the ledger P5.31 step-1 inventory).
/// Sources that do not exist (market signals with no adapter, edge/adapter
/// health — P6.17, plugin health — no plugin kernel) have NO code here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlarmCode {
    /// `TELEMETRY_LOSS` — a current telemetry observation is BAD or MISSING
    /// (the §10.4 quality states; source: the P2.18 current state).
    TelemetryLoss,
    /// `TELEMETRY_STALENESS` — a current telemetry observation is STALE
    /// (its event-time age exceeded the use-case freshness limit, P2.08).
    TelemetryStaleness,
    /// `FEASIBILITY_LIMIT_VIOLATED` — a persisted §12.5 feasibility-check
    /// evidence record with the `LIMIT_VIOLATED` outcome (P4.30).
    FeasibilityLimitViolated,
    /// `FEASIBILITY_FAIL_CLOSED` — a persisted §12.5 feasibility-check
    /// evidence record with the `FAIL_CLOSED` outcome (the closing rule:
    /// a validator/model/infrastructure failure is never a pass).
    FeasibilityFailClosed,
    /// `DISPATCH_FAILURE` — a dispatch command record currently in the
    /// FAILED state (§14.2; the simulated lifecycle).
    DispatchFailure,
    /// `DISPATCH_EXPIRY` — a dispatch command record currently in the
    /// EXPIRED state (§14.2; the valid-until bound elapsed).
    DispatchExpiry,
    /// `DISPATCH_FALLBACK` — a dispatch command record currently in the
    /// FALLBACK state (§14.2; a fallback activation in the simulated
    /// lifecycle).
    DispatchFallback,
}

impl AlarmCode {
    /// All seven codes (the closed vocabulary, inventory order).
    pub const ALL: [AlarmCode; 7] = [
        AlarmCode::TelemetryLoss,
        AlarmCode::TelemetryStaleness,
        AlarmCode::FeasibilityLimitViolated,
        AlarmCode::FeasibilityFailClosed,
        AlarmCode::DispatchFailure,
        AlarmCode::DispatchExpiry,
        AlarmCode::DispatchFallback,
    ];

    /// The canonical SCREAMING_SNAKE name (the wire and display spelling).
    pub const fn name(self) -> &'static str {
        match self {
            AlarmCode::TelemetryLoss => "TELEMETRY_LOSS",
            AlarmCode::TelemetryStaleness => "TELEMETRY_STALENESS",
            AlarmCode::FeasibilityLimitViolated => "FEASIBILITY_LIMIT_VIOLATED",
            AlarmCode::FeasibilityFailClosed => "FEASIBILITY_FAIL_CLOSED",
            AlarmCode::DispatchFailure => "DISPATCH_FAILURE",
            AlarmCode::DispatchExpiry => "DISPATCH_EXPIRY",
            AlarmCode::DispatchFallback => "DISPATCH_FALLBACK",
        }
    }

    /// The deterministic DEFAULT severity per code (the mapping is part of
    /// the reviewed model, never chosen per display).
    pub const fn default_severity(self) -> AlarmSeverity {
        match self {
            AlarmCode::TelemetryLoss => AlarmSeverity::Warning,
            AlarmCode::TelemetryStaleness => AlarmSeverity::Info,
            AlarmCode::FeasibilityLimitViolated => AlarmSeverity::Info,
            AlarmCode::FeasibilityFailClosed => AlarmSeverity::Warning,
            AlarmCode::DispatchFailure => AlarmSeverity::Critical,
            AlarmCode::DispatchExpiry => AlarmSeverity::Warning,
            AlarmCode::DispatchFallback => AlarmSeverity::Critical,
        }
    }
}

// --- Source, state, record -------------------------------------------------------

/// What kind of platform fact an alarm was derived from (closed; the
/// derivation is always traceable to ONE source record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlarmSourceKind {
    /// The P2.18 current-state observation of one asset quantity.
    TelemetryCurrentState,
    /// One persisted dispatch-command record (P4.10 store; §14.2 state).
    DispatchCommand,
    /// One persisted §12.5 feasibility-check evidence record (P4.30).
    FeasibilityCheck,
}

impl AlarmSourceKind {
    /// All three source kinds.
    pub const ALL: [AlarmSourceKind; 3] = [
        AlarmSourceKind::TelemetryCurrentState,
        AlarmSourceKind::DispatchCommand,
        AlarmSourceKind::FeasibilityCheck,
    ];

    /// The canonical SCREAMING_SNAKE name.
    pub const fn name(self) -> &'static str {
        match self {
            AlarmSourceKind::TelemetryCurrentState => "TELEMETRY_CURRENT_STATE",
            AlarmSourceKind::DispatchCommand => "DISPATCH_COMMAND",
            AlarmSourceKind::FeasibilityCheck => "FEASIBILITY_CHECK",
        }
    }
}

/// The alarm lifecycle state. The full ledger vocabulary is
/// `active/acknowledged/cleared`; the derived-on-read contract can only
/// ever PRODUCE `ACTIVE` (it observes current facts, not history), and the
/// other two values exist in the vocabulary for the persistence +
/// acknowledgement path a later phase owns (recorded, not reachable here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlarmState {
    /// `ACTIVE` — currently derivable from the platform facts.
    Active,
    /// `ACKNOWLEDGED` — defined for the later persistence path; the
    /// derived-on-read contract never produces it.
    Acknowledged,
    /// `CLEARED` — defined for the later persistence path; the
    /// derived-on-read contract never produces it (a condition that ended
    /// is simply no longer derivable).
    Cleared,
}

impl AlarmState {
    /// All three states.
    pub const ALL: [AlarmState; 3] = [AlarmState::Active, AlarmState::Acknowledged, AlarmState::Cleared];

    /// The canonical SCREAMING_SNAKE name.
    pub const fn name(self) -> &'static str {
        match self {
            AlarmState::Active => "ACTIVE",
            AlarmState::Acknowledged => "ACKNOWLEDGED",
            AlarmState::Cleared => "CLEARED",
        }
    }
}

/// The reference to the ONE platform record the alarm was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmSource {
    /// The kind of source record.
    pub kind: AlarmSourceKind,
    /// A stable reference to the exact record (an asset id + quantity, a
    /// command id, or a check id) — the evidence link the ledger P5.31
    /// contract requires, pointing at the record the reader can fetch from
    /// the §15.2 read APIs.
    pub reference: String,
}

impl AlarmSource {
    /// Builds a source reference.
    pub fn new(kind: AlarmSourceKind, reference: String) -> Self {
        Self { kind, reference }
    }
}

/// One derived alarm record — the minimal contract the Overview workspace
/// displays (ledger P5.31 step 2: alarm ID, source, severity, state,
/// tenant/site/asset scope, timestamps, evidence link; every field present
/// here is that list, nothing more).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmRecord {
    /// The DETERMINISTIC alarm id (no persisted UUID exists for a
    /// derived-on-read record; two derivations from the same facts yield
    /// the same id).
    pub alarm_id: String,
    /// The owning tenant (P0.16 rule 1 — alarms are tenant-scoped like
    /// every record they derive from).
    pub tenant_id: FlexUuid,
    /// The site scope, where the source record carries one.
    pub site_id: Option<FlexUuid>,
    /// The asset scope, where the source record carries one.
    pub asset_id: Option<FlexUuid>,
    /// The closed alarm code.
    pub code: AlarmCode,
    /// The severity (the code's deterministic default).
    pub severity: AlarmSeverity,
    /// The lifecycle state (`ACTIVE` only on the derived-on-read path).
    pub state: AlarmState,
    /// The deterministic operator-facing message (built by the derivation
    /// rule, never authored per display).
    pub message: String,
    /// The `event_time` of the source observation (P0.09: event time, NOT
    /// ingest time — the two stay distinct types and distinct fields).
    pub raised_at: EventTime,
    /// When the condition was observed to end. ALWAYS `None` on the
    /// derived-on-read path (an ended condition is simply not derivable);
    /// defined for the later persistence path.
    pub cleared_at: Option<EventTime>,
    /// The instant of the derivation (the read). Distinct from `raised_at`
    /// and never merged with it.
    pub derived_at: EventTime,
    /// The source record the alarm derives from (the evidence link).
    pub source: AlarmSource,
    /// The recorded derivation-rule identifier (which rule produced this
    /// record; audit-facing, like the P2.08 `QualityReason` discipline).
    pub derivation_rule: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_vocabulary_is_closed_and_serde_round_trips() {
        assert_eq!(AlarmSeverity::ALL.len(), 3);
        for severity in AlarmSeverity::ALL {
            let json = serde_json::to_value(&severity).unwrap();
            assert_eq!(json, serde_json::json!(severity.name()));
            let back: AlarmSeverity = serde_json::from_value(json).unwrap();
            assert_eq!(back, severity);
        }
        // No invented severity parses.
        assert!(serde_json::from_value::<AlarmSeverity>(serde_json::json!("EMERGENCY")).is_err());
        assert!(serde_json::from_value::<AlarmSeverity>(serde_json::json!("info")).is_err());
    }

    #[test]
    fn severity_ranks_are_ordered_and_distinct() {
        assert!(AlarmSeverity::Info.rank() < AlarmSeverity::Warning.rank());
        assert!(AlarmSeverity::Warning.rank() < AlarmSeverity::Critical.rank());
    }

    #[test]
    fn code_vocabulary_is_closed_one_per_existing_source() {
        assert_eq!(AlarmCode::ALL.len(), 7);
        let names: Vec<&str> = AlarmCode::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(
            names,
            [
                "TELEMETRY_LOSS",
                "TELEMETRY_STALENESS",
                "FEASIBILITY_LIMIT_VIOLATED",
                "FEASIBILITY_FAIL_CLOSED",
                "DISPATCH_FAILURE",
                "DISPATCH_EXPIRY",
                "DISPATCH_FALLBACK",
            ]
        );
        // Round-trip through the wire form.
        for code in AlarmCode::ALL {
            let json = serde_json::to_value(&code).unwrap();
            assert_eq!(json, serde_json::json!(code.name()));
            let back: AlarmCode = serde_json::from_value(json).unwrap();
            assert_eq!(back, code);
        }
        // No invented code parses — the vocabulary does not grow by typo.
        assert!(serde_json::from_value::<AlarmCode>(serde_json::json!("MARKET_SIGNAL_LOST")).is_err());
        assert!(serde_json::from_value::<AlarmCode>(serde_json::json!("EDGE_ADAPTER_DOWN")).is_err());
        assert!(serde_json::from_value::<AlarmCode>(serde_json::json!("telemetry_loss")).is_err());
    }

    #[test]
    fn every_code_has_a_deterministic_default_severity() {
        for code in AlarmCode::ALL {
            let severity = code.default_severity();
            assert!(AlarmSeverity::ALL.contains(&severity));
        }
    }

    #[test]
    fn state_vocabulary_carries_the_ledger_words_and_the_derived_path_is_active_only() {
        assert_eq!(AlarmState::ALL.len(), 3);
        assert_eq!(
            AlarmState::ALL
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>(),
            ["ACTIVE", "ACKNOWLEDGED", "CLEARED"]
        );
    }

    #[test]
    fn alarm_record_round_trips_through_serde() {
        let record = AlarmRecord {
            alarm_id: "flexcore:alarm:DISPATCH_FAILURE:cmd-1".to_string(),
            tenant_id: FlexUuid::nil(),
            site_id: None,
            asset_id: Some(FlexUuid::nil()),
            code: AlarmCode::DispatchFailure,
            severity: AlarmSeverity::Critical,
            state: AlarmState::Active,
            message: "a deterministic message".to_string(),
            raised_at: EventTime::from_unix_millis(1_700_000_000_000).unwrap(),
            cleared_at: None,
            derived_at: EventTime::from_unix_millis(1_700_000_005_000).unwrap(),
            source: AlarmSource::new(AlarmSourceKind::DispatchCommand, "cmd-1".to_string()),
            derivation_rule: "dispatch_outcome_alarm".to_string(),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["code"], serde_json::json!("DISPATCH_FAILURE"));
        assert_eq!(json["severity"], serde_json::json!("CRITICAL"));
        assert_eq!(json["state"], serde_json::json!("ACTIVE"));
        assert_eq!(json["source"]["kind"], serde_json::json!("DISPATCH_COMMAND"));
        assert!(json.get("cleared_at").is_some());
        let back: AlarmRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn model_statement_and_version_are_nonempty_and_honest() {
        assert!(!ALARM_MODEL_VERSION.is_empty());
        assert!(ALARM_MODEL_STATEMENT.contains("NOT protection signals"));
        assert!(ALARM_MODEL_STATEMENT.contains("never invented by the console"));
    }
}
