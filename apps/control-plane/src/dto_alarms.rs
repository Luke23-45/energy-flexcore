// P5.31 — Alarm DTOs: the wire shapes of the `/api/v1/alarms` read
//         resource (report §20.1 item 1; ledger P5.31). ADDITIVE — no
//         existing DTO is modified.
//
// The DTO mirrors `flexcore-contracts::alarm::AlarmRecord` field-for-field
// (snake_case wire names, the house DTO discipline). The honesty fields
// (`derivation_statement`, `model_version`, `sources`) travel IN the
// response so a consumer can never render the alarm list without its
// contract statement: alarms are derived-on-read operator-attention
// records, NOT protection signals, and the frozen §15.3 event vocabulary is
// not extended.

use serde::Serialize;
use utoipa::ToSchema;

use crate::dto::rfc3339;
use flexcore_contracts::alarm::{
    ALARM_MODEL_STATEMENT, ALARM_MODEL_VERSION, AlarmRecord,
};

/// The wire rendering of one derived alarm record.
#[derive(Debug, Serialize, ToSchema)]
pub struct AlarmDto {
    /// The deterministic alarm id (`flexcore:alarm:<CODE>:<scope>`).
    pub alarm_id: String,
    pub tenant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    /// The closed code (e.g. `TELEMETRY_LOSS`, `DISPATCH_FAILURE`).
    pub code: String,
    /// The closed severity (`INFO` | `WARNING` | `CRITICAL`).
    pub severity: String,
    /// `ACTIVE` on the derived-on-read path (the acknowledgement/cleared
    /// vocabulary values are defined but unreachable without persistence).
    pub state: String,
    /// The deterministic operator-facing message.
    pub message: String,
    /// The source observation's event time, RFC 3339 UTC (P0.09: event
    /// time, never ingest time).
    pub raised_at: String,
    /// The source observation's event time, unix millis (the filterable
    /// form; the time-window filters key on this).
    pub raised_at_unix_millis: i64,
    /// Always absent on the derived-on-read path (`cleared_at` is defined
    /// for the later persistence path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleared_at: Option<String>,
    /// The derivation instant (the read), RFC 3339 UTC — distinct from
    /// `raised_at`, never merged with it.
    pub derived_at: String,
    /// The kind of source record (`TELEMETRY_CURRENT_STATE` |
    /// `DISPATCH_COMMAND` | `FEASIBILITY_CHECK`).
    pub source_kind: String,
    /// The reference to the exact source record (the evidence link —
    /// fetchable from the §15.2 read APIs).
    pub source_reference: String,
    /// The recorded derivation-rule identifier.
    pub derivation_rule: String,
}

/// Maps one contract alarm record onto the wire shape.
pub fn alarm_dto(record: &AlarmRecord) -> AlarmDto {
    AlarmDto {
        alarm_id: record.alarm_id.clone(),
        tenant_id: record.tenant_id.to_hyphenated().to_string(),
        site_id: record.site_id.map(|id| id.to_hyphenated().to_string()),
        asset_id: record.asset_id.map(|id| id.to_hyphenated().to_string()),
        code: record.code.name().to_string(),
        severity: record.severity.name().to_string(),
        state: record.state.name().to_string(),
        message: record.message.clone(),
        raised_at: rfc3339(record.raised_at),
        raised_at_unix_millis: record.raised_at.unix_millis() as i64,
        cleared_at: record.cleared_at.map(rfc3339),
        derived_at: rfc3339(record.derived_at),
        source_kind: record.source.kind.name().to_string(),
        source_reference: record.source.reference.clone(),
        derivation_rule: record.derivation_rule.clone(),
    }
}

/// Which alarm sources were consulted for this derivation, and what was
/// found. Absent sources are NAMED (no invented alarms) — the honest
/// inventory the ledger P5.31 step 1 requires.
#[derive(Debug, Serialize, ToSchema)]
pub struct AlarmSourcesDto {
    /// `"available"` when the telemetry read port is wired; otherwise the
    /// honest unavailability note (no telemetry-derived alarms are served,
    /// and none are invented in their place).
    pub telemetry_current_state: String,
    /// How many asset current states were consulted (bounded by the asset
    /// page limit; the bound is part of the derivation contract).
    pub telemetry_assets_consulted: usize,
    /// How many dispatch-command records were consulted (the three
    /// qualifying-state pages, each bounded by the page limit).
    pub dispatch_commands_consulted: usize,
    /// How many feasibility-check evidence records were consulted (bounded
    /// by the page limit).
    pub feasibility_checks_consulted: usize,
    /// How many source records were skipped because they were DEFECTIVE
    /// (e.g. a command whose journal lacks the transition into its current
    /// state — no alarm is derived with an invented timestamp). Never
    /// silently dropped; counted, per the platform's non-silencing
    /// discipline.
    pub skipped_defective_source_records: usize,
    /// The alarm sources that DO NOT exist in this phase, named explicitly
    /// (the ledger P5.31 inventory: market signals — no adapter, D1/D2
    /// OPEN; edge/adapter health — P6.17, Stage 6; plugin health — no
    /// plugin kernel, Stage 7).
    pub absent_sources: Vec<String>,
}

/// The response envelope of `GET /api/v1/alarms`. The derivation happens on
/// every read (no persistence): the response is a single bounded page (no
/// cursor), and every honesty field travels with it.
#[derive(Debug, Serialize, ToSchema)]
pub struct AlarmPageEnvelope {
    /// The currently derivable alarms, ordered by severity rank (highest
    /// first) then alarm id (deterministic).
    pub data: Vec<AlarmDto>,
    /// The consulted/absent source inventory.
    pub sources: AlarmSourcesDto,
    /// The derivation instant of THIS response (RFC 3339 UTC).
    pub derived_at: String,
    /// The alarm-model statement, verbatim (rendered by consumers, never
    /// paraphrased).
    pub derivation_statement: String,
    /// The alarm-model version identifier.
    pub model_version: String,
    /// Always `null`: the derivation is a single bounded read (no persisted
    /// alarm history exists to paginate — recorded honestly).
    pub next_cursor: Option<String>,
}

/// Builds the response envelope from the derived records and the consulted
/// counts.
pub fn alarm_page_envelope(
    mut records: Vec<AlarmRecord>,
    sources: AlarmSourcesDto,
    derived_at: flexcore_contracts::time_model::EventTime,
) -> AlarmPageEnvelope {
    records.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.alarm_id.cmp(&b.alarm_id))
    });
    AlarmPageEnvelope {
        data: records.iter().map(alarm_dto).collect(),
        sources,
        derived_at: rfc3339(derived_at),
        derivation_statement: ALARM_MODEL_STATEMENT.to_string(),
        model_version: ALARM_MODEL_VERSION.to_string(),
        // Single bounded derivation: no cursor, by contract.
        next_cursor: None,
    }
}
