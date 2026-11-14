// P5.31 — Alarm derivation rules (pure functions, no I/O; ADDITIVE).
//
// The derivation half of the minimal alarm model
// (`flexcore-contracts::alarm`): every alarm the platform serves is derived
// deterministically from ONE existing platform fact, by ONE rule recorded
// here, with the source data CITED in the record (the ledger P5.31
// requirement: "every rule must cite its source data and be
// deterministic").
//
// The sources that EXIST by Phase 5 (the ledger P5.31 step-1 inventory) and
// their rules:
//
// | Source fact (verbatim source)                | Rule                       | Alarm code                    |
// |----------------------------------------------|----------------------------|-------------------------------|
// | Current telemetry quality BAD or MISSING (§10.4 quality, P2.18 current state) | [`alarm_for_quality`] | `TELEMETRY_LOSS` |
// | Current telemetry quality STALE (P2.08 staleness — event-time age beyond the use-case freshness limit) | [`alarm_for_quality`] | `TELEMETRY_STALENESS` |
// | Dispatch command currently FAILED (§14.2 stored state, P4.10 store) | [`alarm_for_dispatch_state`] | `DISPATCH_FAILURE` |
// | Dispatch command currently EXPIRED (§14.2)   | [`alarm_for_dispatch_state`] | `DISPATCH_EXPIRY`             |
// | Dispatch command currently FALLBACK (§14.2)  | [`alarm_for_dispatch_state`] | `DISPATCH_FALLBACK`           |
// | Feasibility check record LIMIT_VIOLATED (§12.5 persisted outcome, P4.30) | [`alarm_for_feasibility_outcome`] | `FEASIBILITY_LIMIT_VIOLATED` |
// | Feasibility check record FAIL_CLOSED (§12.5) | [`alarm_for_feasibility_outcome`] | `FEASIBILITY_FAIL_CLOSED` |
//
// Sources that do NOT exist get NO rule and NO code (no invented alarms):
// market signals (no adapter, D1/D2 OPEN), edge/adapter health (P6.17 —
// Stage 6), plugin health (no plugin kernel — Stage 7). Feasibility
// rejections at the GATE (a REJECTED command) surface through the same
// dispatch-state rule when the command sits in REJECTED — the persisted
// §12.5 check rules cover the evidence records.
//
// What "sustained" means here (recorded honestly): the derived-on-read
// contract observes CURRENT facts only. A telemetry-loss alarm therefore
// exists exactly while the current observation carries the qualifying
// state — re-derivation without the condition simply yields no record (the
// alarm self-clears by absence). A minimum-duration debounce (a true
// "sustained N seconds" threshold) needs event-history processing that the
// 0.x derived-on-read contract does not perform; that limitation is
// recorded in the P5.31 decision record, not hidden.
//
// NO protection-grade semantics: these rules produce operator-attention
// records and nothing else (see `flexcore-contracts::alarm::
// ALARM_MODEL_STATEMENT`, rendered verbatim by every consumer).

use flexcore_contracts::alarm::{
    AlarmCode, AlarmRecord, AlarmSource, AlarmSourceKind, AlarmState,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;

/// The deterministic alarm-id prefix. Alarms are derived records; their id
/// namespace is `flexcore:alarm:` — deliberately outside the measured-
/// telemetry namespace (`flexcore:` measured sources never use it, P2.04
/// D-T6 discipline applied to the alarm model's own records).
pub const ALARM_ID_PREFIX: &str = "flexcore:alarm:";

/// The recorded rule identifiers (audit-facing; each record names the rule
/// that produced it).
pub const RULE_TELEMETRY_QUALITY_ALARM: &str = "p5_31/telemetry_quality_alarm";
pub const RULE_DISPATCH_OUTCOME_ALARM: &str = "p5_31/dispatch_outcome_alarm";
pub const RULE_FEASIBILITY_OUTCOME_ALARM: &str = "p5_31/feasibility_outcome_alarm";

/// Builds the deterministic alarm id for a derived record: the prefix, the
/// closed code, and the scope key (the source record's identity — an asset
/// id + quantity, a command id, or a check id). Same facts in, same id out.
pub fn derived_alarm_id(code: AlarmCode, scope_key: &str) -> String {
    format!("{ALARM_ID_PREFIX}{}:{scope_key}", code.name())
}

/// The telemetry-quality rule: BAD or MISSING → `TELEMETRY_LOSS`, STALE →
/// `TELEMETRY_STALENESS`, every other state → no alarm (GOOD/SUSPECT values
/// are usable; ESTIMATED values are explicitly tagged derivations, not
/// faults). Source data: the §10.4 quality state of the P2.18 current-state
/// observation.
pub fn alarm_for_quality(quality: TelemetryQuality) -> Option<AlarmCode> {
    match quality {
        TelemetryQuality::Bad | TelemetryQuality::Missing => Some(AlarmCode::TelemetryLoss),
        TelemetryQuality::Stale => Some(AlarmCode::TelemetryStaleness),
        TelemetryQuality::Good
        | TelemetryQuality::Suspect
        | TelemetryQuality::Estimated => None,
    }
}

/// The dispatch-outcome rule over the stored §14.2 state text (the frozen
/// `command_state` vocabulary as stored by the P4.10 record store):
/// FAILED → `DISPATCH_FAILURE`, EXPIRED → `DISPATCH_EXPIRY`, FALLBACK →
/// `DISPATCH_FALLBACK`; every other state (including the in-flight
/// PROPOSED..TRACKING chain and the terminal COMPLETED/REJECTED/
/// CANCELLED/SUPERSEDED outcomes) → no alarm. A REJECTED command is the
/// gate's decision evidence (P5.06 renders it), not an operator alarm.
pub fn alarm_for_dispatch_state(state: &str) -> Option<AlarmCode> {
    match state {
        "FAILED" => Some(AlarmCode::DispatchFailure),
        "EXPIRED" => Some(AlarmCode::DispatchExpiry),
        "FALLBACK" => Some(AlarmCode::DispatchFallback),
        _ => None,
    }
}

/// The feasibility-outcome rule over the persisted §12.5 outcome text (the
/// closed three-outcome vocabulary of the P4.30 check store):
/// LIMIT_VIOLATED → `FEASIBILITY_LIMIT_VIOLATED`, FAIL_CLOSED →
/// `FEASIBILITY_FAIL_CLOSED`; PASSED → no alarm.
pub fn alarm_for_feasibility_outcome(outcome: &str) -> Option<AlarmCode> {
    match outcome {
        "LIMIT_VIOLATED" => Some(AlarmCode::FeasibilityLimitViolated),
        "FAIL_CLOSED" => Some(AlarmCode::FeasibilityFailClosed),
        _ => None,
    }
}

/// The shared record constructor: assembles the minimal [`AlarmRecord`]
/// from one rule's inputs. `severity` is always the code's deterministic
/// default (never chosen per call); `state` is always
/// [`AlarmState::Active`] on the derived-on-read path; `raised_at` is the
/// caller-supplied EVENT time of the source observation (never an ingest
/// time — the types are distinct, P0.09).
#[allow(clippy::too_many_arguments)]
pub fn build_alarm_record(
    code: AlarmCode,
    tenant_id: FlexUuid,
    site_id: Option<FlexUuid>,
    asset_id: Option<FlexUuid>,
    scope_key: &str,
    message: String,
    raised_at: EventTime,
    derived_at: EventTime,
    source: AlarmSource,
    derivation_rule: &'static str,
) -> AlarmRecord {
    AlarmRecord {
        alarm_id: derived_alarm_id(code, scope_key),
        tenant_id,
        site_id,
        asset_id,
        code,
        severity: code.default_severity(),
        state: AlarmState::Active,
        message,
        raised_at,
        cleared_at: None,
        derived_at,
        source,
        derivation_rule: derivation_rule.to_string(),
    }
}

/// Builds the telemetry-quality alarm record for one current-state
/// observation. The scope key is `<asset uuid>:<quantity>` so a BAD
/// `active_power` and a STALE `soc` on the same asset are two distinct
/// alarms; the source reference names the exact observation the reader can
/// re-fetch (`GET /api/v1/assets/{asset_id}/state`).
#[allow(clippy::too_many_arguments)]
pub fn telemetry_alarm_record(
    tenant_id: FlexUuid,
    site_id: Option<FlexUuid>,
    asset_id: FlexUuid,
    quantity: &str,
    quality: TelemetryQuality,
    event_time: EventTime,
    derived_at: EventTime,
) -> Option<AlarmRecord> {
    let code = alarm_for_quality(quality)?;
    Some(build_alarm_record(
        code,
        tenant_id,
        site_id,
        Some(asset_id),
        &format!("{}:{quantity}", asset_id.to_hyphenated()),
        format!(
            "telemetry quantity '{quantity}' is {} on asset {}: the current observation \
             carries no usable value (quality state per §10.4; the alarm exists exactly \
             while this observation holds)",
            quality.name(),
            asset_id.to_hyphenated()
        ),
        event_time,
        derived_at,
        AlarmSource::new(
            AlarmSourceKind::TelemetryCurrentState,
            format!("{}:{quantity}", asset_id.to_hyphenated()),
        ),
        RULE_TELEMETRY_QUALITY_ALARM,
    ))
}

/// Builds the dispatch-outcome alarm record for one command currently in a
/// qualifying stored state. `raised_at` is the event time of the transition
/// INTO that state (the journal entry's timestamp — the caller passes it;
/// it is the source observation's event time, not the derivation time).
#[allow(clippy::too_many_arguments)]
pub fn dispatch_alarm_record(
    tenant_id: FlexUuid,
    site_id: Option<FlexUuid>,
    asset_id: Option<FlexUuid>,
    command_id: &str,
    state: &str,
    outcome_event_time: EventTime,
    evidence_reference: Option<&str>,
    derived_at: EventTime,
) -> Option<AlarmRecord> {
    let code = alarm_for_dispatch_state(state)?;
    Some(build_alarm_record(
        code,
        tenant_id,
        site_id,
        asset_id,
        command_id,
        format!(
            "dispatch command {command_id} is in the {state} state (the simulated \
             lifecycle, §14.2): an operator-attention record — acknowledgement is \
             application-layer acceptance and nothing here is field delivery"
        ),
        outcome_event_time,
        derived_at,
        AlarmSource::new(
            AlarmSourceKind::DispatchCommand,
            match evidence_reference {
                Some(reference) => format!("{command_id}:{reference}"),
                None => command_id.to_string(),
            },
        ),
        RULE_DISPATCH_OUTCOME_ALARM,
    ))
}

/// Builds the feasibility-outcome alarm record for one persisted §12.5
/// check evidence record. The source reference is the check id — the same
/// id the `/api/v1/feasibility-checks/{check_id}` read resource serves.
#[allow(clippy::too_many_arguments)]
pub fn feasibility_alarm_record(
    tenant_id: FlexUuid,
    site_id: Option<FlexUuid>,
    asset_id: Option<FlexUuid>,
    check_id: &str,
    outcome: &str,
    checked_at: EventTime,
    derived_at: EventTime,
) -> Option<AlarmRecord> {
    let code = alarm_for_feasibility_outcome(outcome)?;
    Some(build_alarm_record(
        code,
        tenant_id,
        site_id,
        asset_id,
        check_id,
        format!(
            "feasibility check {check_id} completed with outcome {outcome} (the persisted \
             §12.5 verdict): an operator-attention record on the simulated scheduling \
             pipeline, never a grid-protection signal"
        ),
        checked_at,
        derived_at,
        AlarmSource::new(AlarmSourceKind::FeasibilityCheck, check_id.to_string()),
        RULE_FEASIBILITY_OUTCOME_ALARM,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::alarm::AlarmSeverity;

    const EVENT_MS: i64 = 1_700_000_000_000;

    fn event_at(offset_millis: i64) -> EventTime {
        EventTime::from_unix_millis(EVENT_MS + offset_millis).unwrap()
    }

    fn asset() -> FlexUuid {
        FlexUuid::from_u128(9)
    }

    fn tenant() -> FlexUuid {
        FlexUuid::from_u128(1)
    }

    // --- The quality rule ----------------------------------------------------

    #[test]
    fn bad_missing_and_stale_qualify_every_other_state_does_not() {
        assert_eq!(
            alarm_for_quality(TelemetryQuality::Bad),
            Some(AlarmCode::TelemetryLoss)
        );
        assert_eq!(
            alarm_for_quality(TelemetryQuality::Missing),
            Some(AlarmCode::TelemetryLoss)
        );
        assert_eq!(
            alarm_for_quality(TelemetryQuality::Stale),
            Some(AlarmCode::TelemetryStaleness)
        );
        // GOOD/SUSPECT are usable; ESTIMATED is an explicitly tagged
        // derivation, not a fault — no alarm from any of them.
        assert_eq!(alarm_for_quality(TelemetryQuality::Good), None);
        assert_eq!(alarm_for_quality(TelemetryQuality::Suspect), None);
        assert_eq!(alarm_for_quality(TelemetryQuality::Estimated), None);
    }

    #[test]
    fn telemetry_alarm_record_cites_its_source_and_is_deterministic() {
        let first = telemetry_alarm_record(
            tenant(),
            Some(FlexUuid::from_u128(7)),
            asset(),
            "active_power",
            TelemetryQuality::Bad,
            event_at(0),
            event_at(5_000),
        )
        .expect("BAD qualifies");
        let second = telemetry_alarm_record(
            tenant(),
            Some(FlexUuid::from_u128(7)),
            asset(),
            "active_power",
            TelemetryQuality::Bad,
            event_at(0),
            event_at(5_000),
        )
        .expect("BAD qualifies");
        // Same facts in, same record out (the derived-on-read dedup property).
        assert_eq!(first, second);
        assert_eq!(
            first.alarm_id,
            format!("flexcore:alarm:TELEMETRY_LOSS:{}:active_power", asset().to_hyphenated())
        );
        assert_eq!(first.code, AlarmCode::TelemetryLoss);
        assert_eq!(first.severity, AlarmSeverity::Warning);
        assert_eq!(first.state, AlarmState::Active);
        assert_eq!(first.raised_at, event_at(0));
        assert_eq!(first.derived_at, event_at(5_000));
        assert!(first.cleared_at.is_none());
        assert_eq!(first.source.kind, AlarmSourceKind::TelemetryCurrentState);
        assert!(first.source.reference.contains("active_power"));
        assert_eq!(first.derivation_rule, RULE_TELEMETRY_QUALITY_ALARM);
        assert!(first.message.contains("active_power"));
    }

    #[test]
    fn staleness_is_a_distinct_code_from_loss() {
        let stale = telemetry_alarm_record(
            tenant(),
            None,
            asset(),
            "soc",
            TelemetryQuality::Stale,
            event_at(0),
            event_at(0),
        )
        .expect("STALE qualifies");
        assert_eq!(stale.code, AlarmCode::TelemetryStaleness);
        assert_eq!(stale.severity, AlarmSeverity::Info);
        assert_ne!(stale.alarm_id, {
            let loss = telemetry_alarm_record(
                tenant(),
                None,
                asset(),
                "soc",
                TelemetryQuality::Missing,
                event_at(0),
                event_at(0),
            )
            .expect("MISSING qualifies");
            loss.alarm_id
        });
    }

    #[test]
    fn usable_quality_yields_no_telemetry_record() {
        for quality in [TelemetryQuality::Good, TelemetryQuality::Suspect, TelemetryQuality::Estimated] {
            assert!(telemetry_alarm_record(
                tenant(),
                None,
                asset(),
                "active_power",
                quality,
                event_at(0),
                event_at(0),
            )
            .is_none());
        }
    }

    // --- The dispatch rule ----------------------------------------------------

    #[test]
    fn failed_expired_and_fallback_qualify() {
        assert_eq!(
            alarm_for_dispatch_state("FAILED"),
            Some(AlarmCode::DispatchFailure)
        );
        assert_eq!(
            alarm_for_dispatch_state("EXPIRED"),
            Some(AlarmCode::DispatchExpiry)
        );
        assert_eq!(
            alarm_for_dispatch_state("FALLBACK"),
            Some(AlarmCode::DispatchFallback)
        );
        // The in-flight chain, the pass outcomes, and the other terminal
        // states yield no alarm (a REJECTED command is gate evidence, P5.06).
        for state in [
            "PROPOSED",
            "VALIDATED",
            "FEASIBLE",
            "APPROVED",
            "ISSUED",
            "ACKNOWLEDGED",
            "TRACKING",
            "COMPLETED",
            "REJECTED",
            "CANCELLED",
            "SUPERSEDED",
        ] {
            assert_eq!(alarm_for_dispatch_state(state), None, "{state}");
        }
        // An unknown state text is not an alarm either (no invented mapping).
        assert_eq!(alarm_for_dispatch_state("MAGIC"), None);
    }

    #[test]
    fn dispatch_alarm_record_carries_the_command_scope_and_severity() {
        let record = dispatch_alarm_record(
            tenant(),
            Some(FlexUuid::from_u128(7)),
            Some(asset()),
            "cmd-1",
            "FAILED",
            event_at(2_000),
            Some("evidence-1"),
            event_at(5_000),
        )
        .expect("FAILED qualifies");
        assert_eq!(record.alarm_id, "flexcore:alarm:DISPATCH_FAILURE:cmd-1");
        assert_eq!(record.severity, AlarmSeverity::Critical);
        assert_eq!(record.raised_at, event_at(2_000));
        assert_eq!(record.derived_at, event_at(5_000));
        assert_eq!(record.source.kind, AlarmSourceKind::DispatchCommand);
        assert_eq!(record.source.reference, "cmd-1:evidence-1");
        // The simulation-only qualification travels in the message.
        assert!(record.message.contains("simulated"));
        assert!(record.message.contains("NOT field delivery"));
        // The event/ingest distinction is structural: raised_at carries the
        // caller-supplied EVENT time; derived_at carries the derivation
        // instant; neither is ever the other.
        assert_ne!(record.raised_at, record.derived_at);
    }

    // --- The feasibility rule -------------------------------------------------

    #[test]
    fn limit_violated_and_fail_closed_qualify_passed_does_not() {
        assert_eq!(
            alarm_for_feasibility_outcome("LIMIT_VIOLATED"),
            Some(AlarmCode::FeasibilityLimitViolated)
        );
        assert_eq!(
            alarm_for_feasibility_outcome("FAIL_CLOSED"),
            Some(AlarmCode::FeasibilityFailClosed)
        );
        assert_eq!(alarm_for_feasibility_outcome("PASSED"), None);
        assert_eq!(alarm_for_feasibility_outcome(""), None);
    }

    #[test]
    fn feasibility_alarm_record_uses_the_check_id_as_the_evidence_link() {
        let record = feasibility_alarm_record(
            tenant(),
            Some(FlexUuid::from_u128(7)),
            Some(asset()),
            "check-1",
            "FAIL_CLOSED",
            event_at(1_000),
            event_at(5_000),
        )
        .expect("FAIL_CLOSED qualifies");
        assert_eq!(record.alarm_id, "flexcore:alarm:FEASIBILITY_FAIL_CLOSED:check-1");
        assert_eq!(record.severity, AlarmSeverity::Warning);
        assert_eq!(record.source.kind, AlarmSourceKind::FeasibilityCheck);
        assert_eq!(record.source.reference, "check-1");
        assert!(record.message.contains("never a grid-protection signal"));
        assert_eq!(record.derivation_rule, RULE_FEASIBILITY_OUTCOME_ALARM);
    }

    // --- The id rule -----------------------------------------------------------

    #[test]
    fn derived_alarm_ids_are_deterministic_and_namespaced() {
        let id = derived_alarm_id(AlarmCode::DispatchExpiry, "cmd-9");
        assert_eq!(id, "flexcore:alarm:DISPATCH_EXPIRY:cmd-9");
        assert_eq!(derived_alarm_id(AlarmCode::DispatchExpiry, "cmd-9"), id);
        assert!(id.starts_with(ALARM_ID_PREFIX));
    }

    // --- No invented sources ----------------------------------------------------

    #[test]
    fn the_record_builder_never_escalates_severity_per_call() {
        // The severity is always the code's default — there is no input that
        // can raise or lower it (no per-display choice exists).
        for code in AlarmCode::ALL {
            let record = build_alarm_record(
                code,
                tenant(),
                None,
                None,
                "scope",
                "message".to_string(),
                event_at(0),
                event_at(0),
                AlarmSource::new(AlarmSourceKind::DispatchCommand, "ref".to_string()),
                RULE_DISPATCH_OUTCOME_ALARM,
            );
            assert_eq!(record.severity, code.default_severity());
            assert_eq!(record.state, AlarmState::Active);
        }
    }
}
