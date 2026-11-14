// P4.30 — Phase 4 API DTOs: the response shapes for the dispatch-command
//         and feasibility-check resources (report §15.2–§15.3, §14.3,
//         §21.3; ledger P4.30 step 3; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md`).
//         P4.36 appended additively: the audit-event response shapes
//         (`/api/v1/audit-events`, report §15.2; decision record
//         `36_audit_read_api.md`).
//
// Conventions carried over from dto.rs (P1.17) and dto_phase3.rs (P3.28):
// the domain/storage types are not serializable at the boundary; every
// response is an explicit DTO; timestamps are UTC unix millis where the
// record stores them as millis.
//
// ## Recorded contract statements carried IN the responses
//
// - **READ-ONLY resources.** `/api/v1/dispatch-commands` and
//   `/api/v1/feasibility-checks` serve persisted evidence. There is NO
//   POST/PUT/execute operation on either resource: the §14.1 transitions
//   happen exclusively through the application-layer gates
//   (`flexcore_application::command_gates`, each with its own distinct
//   authorization action), never through a generic "execute arbitrary
//   command" endpoint (§15.2). [`NO_EXECUTE_ENDPOINT_STATEMENT`] travels
//   in every response.
// - **Simulation-only.** Every dispatch-command response carries
//   [`DISPATCH_SIMULATION_ONLY_STATEMENT`]: the entire 0.x line is
//   simulation-only; an ACKNOWLEDGED journal entry is application-layer
//   acceptance, never physical delivery (§14.1, §14.5).
// - **Correlation (§21.3).** Every record carries the tenant/site/
//   portfolio/asset scope, the schedule + command identities, the model
//   id/version/hash where applicable, the acting identity, and the
//   correlation (trace) id — the reconstruction contract. No secrets and
//   no unnecessary personal/customer data appear in any field (the actor
//   is an identity reference such as `svc:validator` or `user:<subject>`).

use serde::Serialize;
use utoipa::ToSchema;

/// The recorded no-execute statement, carried in every Phase 4 response
/// and asserted by the contract tests (§15.2).
pub const NO_EXECUTE_ENDPOINT_STATEMENT: &str = "This resource is READ-ONLY. \
     There is no execute-command endpoint anywhere on this API surface \
     (report s15.2). Dispatch transitions happen exclusively through the \
     application-layer gates (validation, feasibility, authorization - the \
     three gates are distinct, report s14.1), each under its own explicit \
     authorization action.";

/// The recorded simulation-only boundary statement (report §14.5, §21.4),
/// carried in every dispatch-command response.
pub const DISPATCH_SIMULATION_ONLY_STATEMENT: &str = "SIMULATION ONLY (0.x line): \
     dispatch commands are executed against the simulator adapter at most \
     (report s14.16/P4.16). No device write, no field control, no primary \
     frequency response, synthetic inertia, relay protection, or inverter \
     inner-loop control. An ACKNOWLEDGED journal entry is application-layer \
     acceptance, never physical delivery (report s14.1).";

/// One journalled state transition of a dispatch command (the P4.10
/// state-change journal row — immutable, never rewritten).
#[derive(Debug, Serialize, ToSchema)]
pub struct CommandTransitionDto {
    /// The journal sequence number (per-command transition order).
    pub seq: i64,
    pub from_state: String,
    pub to_state: String,
    /// The acting identity (an identity reference, never a credential —
    /// §21.3).
    pub actor: String,
    /// The transition timestamp (UTC unix millis, controlled clock).
    pub at_unix_millis: i64,
    /// The gate that produced the transition (`validation` |
    /// `feasibility` | `authorization`), present exactly on the three
    /// gate passes and the gate rejections (§14.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    /// The reason, where the report requires one (the failing check for
    /// REJECTED, the FAILED trigger, the milestone not reached for
    /// EXPIRED, the cancellation reason, the fallback policy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The evidence reference, where the transition rests on evidence
    /// (§6.4, §14.3 field 16, §14.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_reference: Option<String>,
}

/// One persisted dispatch command (the P4.10 immutable record plus its
/// current state — migration 0016 `dispatch_commands`).
#[derive(Debug, Serialize, ToSchema)]
pub struct DispatchCommandDto {
    pub command_id: String,
    pub tenant_id: String,
    pub portfolio_id: String,
    pub site_id: String,
    pub asset_id: String,
    /// The source schedule chain (P3.22 identity scheme).
    pub schedule_id: String,
    pub schedule_revision: u64,
    /// The valid-until bound (UTC millis, end-exclusive — §10.3).
    pub valid_until_millis: i64,
    /// The command THIS command replaces (§14.3 field 16, forward link).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_command_id: Option<String>,
    /// The command that replaced THIS command (set exactly in the
    /// SUPERSEDED state).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by_command_id: Option<String>,
    /// The current state (the closed 14-state §14.2 vocabulary).
    pub state: String,
    /// The full §14.3 command record as persisted (verbatim JSON).
    pub command_payload: serde_json::Value,
    pub no_execute_endpoint_statement: String,
    pub simulation_only_statement: String,
}

/// One dispatch command with its FULL transition journal (the audit view
/// of `GET /api/v1/dispatch-commands/{command_id}` — every transition,
/// never rewritten: the reconstruction contract, §21.3).
#[derive(Debug, Serialize, ToSchema)]
pub struct DispatchCommandDetailDto {
    /// The immutable command record plus its current state.
    pub command: DispatchCommandDto,
    /// The full journal, in transition order.
    pub journal: Vec<CommandTransitionDto>,
}

/// One page of the per-tenant dispatch-command listing (keyset over
/// `command_id` — UUIDv7s, so id order is a stable time order;
/// `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct DispatchCommandPageEnvelope {
    pub data: Vec<DispatchCommandDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One persisted §12.5 feasibility-check evidence record (migration 0017
/// `feasibility_check_records`; P4.30). The verdict detail of the frozen
/// §15.3 `FeasibilityCheckCompleted` event (whose payload carries identity
/// + time only — the recorded payload-fit convention).
#[derive(Debug, Serialize, ToSchema)]
pub struct FeasibilityCheckDto {
    /// The check identity (the §12.5 attempt id or the pipeline-level
    /// `gate:<candidate>` id).
    pub check_id: String,
    pub tenant_id: String,
    /// The command the check ran for, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    pub portfolio_id: String,
    pub site_id: String,
    pub asset_id: String,
    pub schedule_id: String,
    pub schedule_revision: u64,
    /// The model the check evaluated against (§12.6 traceability,
    /// §21.3 correlation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    /// THE MODEL HASH the evidence binds to (a model change invalidates
    /// it — §17.5 invariant 3). Present exactly for a PASSED check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_content_hash: Option<String>,
    /// The closed verdict vocabulary: `PASSED` | `LIMIT_VIOLATED` |
    /// `FAIL_CLOSED`.
    pub outcome: String,
    /// The violated network constraint (present exactly for
    /// `LIMIT_VIOLATED`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failing_constraint: Option<String>,
    /// The fail-closed reason (present exactly for `FAIL_CLOSED`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The margin/diagnostics summary (§12.5 step 7), when produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margins: Option<String>,
    /// The coverage statement/limitations (§12.5 step 6), when produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<String>,
    /// The acting identity (an identity reference, never a credential).
    pub actor: String,
    /// The correlation (trace) id (§21.3).
    pub correlation_id: String,
    /// The completion timestamp (UTC unix millis, controlled clock).
    pub checked_at_unix_millis: i64,
    pub no_execute_endpoint_statement: String,
}

/// One page of the per-tenant feasibility-check listing (ordered by
/// `(checked_at, check_id)`; `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct FeasibilityCheckPageEnvelope {
    pub data: Vec<FeasibilityCheckDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The shared per-tenant listing parameters (the recorded P2.12/P3.28
/// limits apply: default page 200, maximum 1000; the cursor is the opaque
/// keyset value of the previous page — malformed = 422).
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct Phase4ListParams {
    /// Dispatch-command listing only: restrict to one stored state (a
    /// value outside the closed 14-state vocabulary is 422).
    #[serde(default)]
    pub state: Option<String>,
    /// Feasibility-check listing only: restrict to one command (canonical
    /// UUID; malformed = 422).
    #[serde(default)]
    pub command_id: Option<String>,
    /// Feasibility-check listing only: restrict to one schedule chain.
    #[serde(default)]
    pub schedule_id: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    /// The opaque keyset cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Maps one storage
/// [`flexcore_storage::command_store::CommandRecord`] onto the API DTO.
pub fn dispatch_command_dto(
    record: &flexcore_storage::command_store::CommandRecord,
) -> DispatchCommandDto {
    DispatchCommandDto {
        command_id: record.row.command_id.clone(),
        tenant_id: record.row.tenant_id.uuid().to_hyphenated().to_string(),
        portfolio_id: record.row.portfolio_id.clone(),
        site_id: record.row.site_id.clone(),
        asset_id: record.row.asset_id.clone(),
        schedule_id: record.row.schedule_id.clone(),
        schedule_revision: record.row.schedule_revision,
        valid_until_millis: record.row.valid_until_millis,
        supersedes_command_id: record.row.supersedes_command_id.clone(),
        superseded_by_command_id: record.superseded_by_command_id.clone(),
        state: record.state.clone(),
        command_payload: serde_json::from_str(&record.row.command_payload_json)
            .unwrap_or(serde_json::Value::Null),
        no_execute_endpoint_statement: NO_EXECUTE_ENDPOINT_STATEMENT.to_string(),
        simulation_only_statement: DISPATCH_SIMULATION_ONLY_STATEMENT.to_string(),
    }
}

/// Maps one storage
/// [`flexcore_storage::command_store::CommandTransitionRecord`] (journal
/// sequence included) onto the API DTO.
pub fn command_transition_dto(
    seq: i64,
    record: &flexcore_storage::command_store::CommandTransitionRecord,
) -> CommandTransitionDto {
    CommandTransitionDto {
        seq,
        from_state: record.from_state.clone(),
        to_state: record.to_state.clone(),
        actor: record.actor.clone(),
        at_unix_millis: record.at_unix_millis,
        gate: record.gate.clone(),
        reason: record.reason.clone(),
        evidence_reference: record.evidence_reference.clone(),
    }
}

/// Maps one storage
/// [`flexcore_storage::feasibility_check::FeasibilityCheckRow`] onto the
/// API DTO.
pub fn feasibility_check_dto(
    row: &flexcore_storage::feasibility_check::FeasibilityCheckRow,
) -> FeasibilityCheckDto {
    FeasibilityCheckDto {
        check_id: row.check_id.clone(),
        tenant_id: row.tenant_id.uuid().to_hyphenated().to_string(),
        command_id: row.command_id.clone(),
        portfolio_id: row.portfolio_id.clone(),
        site_id: row.site_id.clone(),
        asset_id: row.asset_id.clone(),
        schedule_id: row.schedule_id.clone(),
        schedule_revision: row.schedule_revision,
        model_id: row.model_id.clone(),
        model_version: row.model_version.clone(),
        model_content_hash: row.model_content_hash.clone(),
        outcome: row.outcome.clone(),
        failing_constraint: row.failing_constraint.clone(),
        reason: row.reason.clone(),
        margins: row.margins.clone(),
        coverage: row.coverage.clone(),
        actor: row.actor.clone(),
        correlation_id: row.correlation_id.clone(),
        checked_at_unix_millis: row.checked_at_unix_millis,
        no_execute_endpoint_statement: NO_EXECUTE_ENDPOINT_STATEMENT.to_string(),
    }
}

// ---------------------------------------------------------------------------
// P4.36 — audit events (§15.2 /api/v1/audit-events)
// ---------------------------------------------------------------------------

/// The recorded append-only statement, carried in every audit-event
/// response and asserted by the contract tests (§15.2, §16.2; the P1.10
/// port rule applied at the API layer).
pub const AUDIT_APPEND_ONLY_STATEMENT: &str = "This resource is READ-ONLY and the audit \
     trail is append-only: there is no update, delete, or export/dump endpoint anywhere \
     on this surface (report s15.2, s16.2). A correction is a NEW record whose \
     corrected_audit_id references the original (P1.10); the original is never edited \
     or removed.";

/// One persisted audit evidence record (migration 0008 `audit_records`;
/// P4.36) — the §10.1 field set served END TO END: every record carries its
/// decision (outcome + reason reference, never a bare allow/deny), its
/// evidence reference, its before/after REFERENCES (pointers, never payload
/// copies — P1.12 §4.7), the distinct P0.09 event/ingest timestamps, and
/// the §21.3 correlation id. Never a summary that drops references (P5.09
/// consumes them).
///
/// Data discipline (§21.3): the actor is an identity reference (never a
/// credential); no secret and no unnecessary personal or customer data
/// appears in any field. `tenant_id` is absent only for the
/// platform-level records the read port never serves (P1.12 §5.2).
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEventDto {
    /// The record identifier (canonical UUID text; UUIDv7, so id order is
    /// a stable time order).
    pub audit_id: String,
    /// The owning tenant (canonical UUID text); never absent on records
    /// this resource serves (platform-level rows are invisible to the
    /// tenant-scoped port, P1.12 §5.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// The acting identity (an identity reference, never a credential —
    /// §21.3).
    pub actor: String,
    /// The audited action (the event-class discriminator, e.g.
    /// `command.approve`).
    pub action: String,
    /// The audited object type (e.g. `command`, `schedule`, `asset`).
    pub object_type: String,
    /// The audited object identifier (the entity-id correlation).
    pub object_id: String,
    /// Reference to the previous state, where applicable (a pointer, never
    /// a payload copy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_ref: Option<String>,
    /// Reference to the new state, where applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_ref: Option<String>,
    /// The decision outcome (`ALLOWED` | `DENIED`), where one was recorded.
    /// Present exactly together with `decision_reason_reference` — never a
    /// bare allow/deny (P1.11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_outcome: Option<String>,
    /// The reason/evidence reference behind the recorded decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason_reference: Option<String>,
    /// The evidence reference, where applicable (preserved end to end —
    /// never dropped by a summary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// When the audited action occurred (UTC unix millis, P0.09 event
    /// time; distinct from `ingest_time_unix_millis`).
    pub event_time_unix_millis: i64,
    /// When the record was written (UTC unix millis, P0.09 ingest time;
    /// never before the event time).
    pub ingest_time_unix_millis: i64,
    /// The workflow correlation (trace) id (§21.3).
    pub correlation_id: String,
    /// The original record this record corrects, when it is a NEW
    /// correction row (P1.10 step 3 — the original is never edited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corrected_audit_id: Option<String>,
    pub append_only_statement: String,
}

/// One page of the per-tenant audit-event listing (ordered by
/// `(event_time, audit_id)`; `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEventPageEnvelope {
    pub data: Vec<AuditEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The `/api/v1/audit-events` listing parameters (the recorded
/// P2.12/P3.28/P4.30 limits apply: default page 200, maximum 1000; the
/// cursor is the opaque keyset value of the previous page — malformed =
/// 422). The time bounds are INCLUSIVE on both ends (the recorded P4.36
/// DEFAULT decision).
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct AuditEventListParams {
    /// Restrict to one exact audited action (the event class, e.g.
    /// `command.approve`); empty is 422.
    #[serde(default)]
    pub action: Option<String>,
    /// Restrict to one acting identity (exact identity reference); empty
    /// is 422.
    #[serde(default)]
    pub actor: Option<String>,
    /// Restrict to one audited object type (e.g. `command`, `schedule`);
    /// empty is 422.
    #[serde(default)]
    pub object_type: Option<String>,
    /// Restrict to one audited object identifier (the entity-id filter).
    #[serde(default)]
    pub object_id: Option<String>,
    /// Inclusive lower bound on the event time (UTC unix millis); greater
    /// than `to_event_unix_millis` is 422.
    #[serde(default)]
    pub from_event_unix_millis: Option<i64>,
    /// Inclusive upper bound on the event time (UTC unix millis).
    #[serde(default)]
    pub to_event_unix_millis: Option<i64>,
    #[serde(default)]
    pub limit: Option<u32>,
    /// The opaque keyset cursor of the previous page (the last record's
    /// `audit_id`; canonical UUID — malformed = 422).
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Maps one storage [`flexcore_storage::audit_read::AuditEventRow`] onto
/// the API DTO.
pub fn audit_event_dto(row: &flexcore_storage::audit_read::AuditEventRow) -> AuditEventDto {
    AuditEventDto {
        audit_id: row.audit_id.clone(),
        tenant_id: row
            .tenant_id
            .map(|tenant| tenant.uuid().to_hyphenated().to_string()),
        actor: row.actor.clone(),
        action: row.action.clone(),
        object_type: row.object_type.clone(),
        object_id: row.object_id.clone(),
        before_ref: row.before_ref.clone(),
        after_ref: row.after_ref.clone(),
        decision_outcome: row.decision_outcome.clone(),
        decision_reason_reference: row.decision_reason_reference.clone(),
        evidence: row.evidence.clone(),
        event_time_unix_millis: row.event_time_unix_millis,
        ingest_time_unix_millis: row.ingest_time_unix_millis,
        correlation_id: row.correlation_id.clone(),
        corrected_audit_id: row.corrected_audit_id.clone(),
        append_only_statement: AUDIT_APPEND_ONLY_STATEMENT.to_string(),
    }
}
