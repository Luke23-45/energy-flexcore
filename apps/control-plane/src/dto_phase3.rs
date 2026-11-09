// P3.28 — Phase 3 API DTOs: the response shapes for the flexibility
//         envelope, schedule, and optimization-run resources (report
//         §15.1–§15.2, §12.6; ledger P3.28 step 1).
//
// Conventions carried over from dto.rs (P1.17): the domain/storage types
// are not serializable at the boundary; every response is an explicit DTO;
// canonical units (MW/MWh) stay verbatim as numbers in canonical fields;
// timestamps are RFC 3339 UTC strings.
//
// ## Recorded contract statements carried IN the responses
//
// Phase 3's boundaries must never imply feasibility, approval, or dispatch
// (§12.5; ledger Phase 3 discipline "a solved schedule is a CANDIDATE").
// Every schedule-bearing response therefore carries:
//
// - `candidate_status` — always the literal `"candidate"` for a Phase 3
//   record (the §12.6 `status` field is the SOLVER outcome and is exposed
//   verbatim as `status`; `candidate_status` is the lifecycle statement);
// - `candidate_status_statement` — the frozen P3.21
//   [`CANDIDATE_STATUS_STATEMENT`];
// - `validation_scope` + `validation_scope_statement` — the recorded
//   ScheduleValidated caveat (see [`SCHEDULE_VALIDATED_SCOPE_STATEMENT`]):
//   "validated" in Phase 3 means asset/time-coupled constraint validation
//   ONLY, never network feasibility (Phase 4, §12.5).
//
// ## Recorded payload gaps carried IN the responses
//
// The frozen §15.3 payloads (flexcore-events, frozen for this subtask)
// cannot carry the full record references:
// - `FlexibilityEnvelopeUpdatedPayload` carries no envelope version, no
//   horizon, no input-snapshot reference (the P3.12
//   [`ENVELOPE_PAYLOAD_GAP_NOTE`]); the version travels in the
//   deterministic idempotency key.
// - `ScheduleProposedPayload` carries no candidate id and no result-record
//   reference (the P3.21 [`SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE`]); the
//   reference travels in the deterministic idempotency key.
// - `ScheduleValidatedPayload` carries no validation-scope field at all —
//   it CANNOT carry the asset/time-coupled-only caveat. The caveat is
//   therefore carried in the persisted/served record fields
//   (`validation_scope` + statement above) and in the docs; the gap is
//   recorded here and in the decision doc (the recorded P3.12/P3.21
//   gap-note precedent: needs an additive serde(default) payload field for
//   a future subtask that owns flexcore-events).
use serde::Serialize;
use utoipa::ToSchema;

use flexcore_application::flexibility_envelope::ENVELOPE_PAYLOAD_GAP_NOTE;
use flexcore_application::optimization_result_record::{
    CANDIDATE_STATUS_STATEMENT, SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE,
};

/// The recorded ScheduleValidated caveat, stated once and carried in every
/// schedule response, in the docs, and in the decision record: "validated"
/// in Phase 3 means asset/time-coupled constraint validation ONLY (the
/// P3.01–P3.11 capability models plus the P3.03 scheduling time model —
/// the assertion surface the P3.25/P3.26 tests exercise). It is NOT a
/// network-feasibility verdict: the feasibility gate is Phase 4 (§12.5).
/// No Phase 4 reader may mistake a Phase 3 `ScheduleValidated` for a
/// feasibility result.
pub const SCHEDULE_VALIDATED_SCOPE_STATEMENT: &str = "Phase 3 ScheduleValidated \
     records ASSET/TIME-COUPLED constraint validation only (capability models \
     P3.01-P3.11 + scheduling time model P3.03; the P3.25/P3.26 assertion \
     surface). It is NOT network feasibility, NOT an authorization, and NOT a \
     dispatch verdict - the network feasibility gate is Phase 4 (report \
     section 12.5). A Phase 3 validated schedule remains a CANDIDATE.";

/// The recorded idempotency-key scheme for the `ScheduleValidated` outbox
/// row (mirrors the P3.21 `schedule_proposed` key rule):
/// `schedule_validated:<portfolio>:<site>:<candidate_id>`. The frozen
/// payload carries no validation-scope field (recorded gap — see the
/// module header), so the event identity is the key plus the record-side
/// caveat fields.
pub const SCHEDULE_VALIDATED_IDEMPOTENCY_PREFIX: &str = "schedule_validated";

/// One persisted portfolio flexibility-envelope record (P3.28). The full
/// envelope computation result travels in `envelope_payload` (the P3.12
/// rendering); the structured fields are the identity/horizon summary a
/// reader filters on.
#[derive(Debug, Serialize, ToSchema)]
pub struct EnvelopeRecordDto {
    pub envelope_id: String,
    pub tenant_id: String,
    pub portfolio_id: String,
    /// The envelope version (P3.12 `EnvelopeVersion`).
    pub envelope_version: String,
    /// Start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// End-exclusive horizon end, UTC millis.
    pub horizon_end_millis: i64,
    /// The interval length, UTC millis.
    pub interval_millis: i64,
    pub interval_count: u32,
    /// The covered sites (canonical UUID texts; one
    /// `FlexibilityEnvelopeUpdated` event per site was emitted with the
    /// record, in list order).
    pub covered_site_ids: Vec<String>,
    /// The full envelope computation result (the P3.12 rendering, verbatim).
    pub envelope_payload: serde_json::Value,
    /// The recorded frozen-payload gap note (P3.12): the event payload
    /// carries correlation/tenant/portfolio/site/asset + event_time only.
    pub event_payload_gap_note: String,
}

/// One persisted schedule revision (P3.28). Every revision is a CANDIDATE;
/// the `ScheduleValidated` caveat is carried in the record fields (the
/// frozen event payload cannot carry it — recorded gap, module header).
#[derive(Debug, Serialize, ToSchema)]
pub struct ScheduleRevisionDto {
    pub schedule_id: String,
    pub revision: u64,
    pub tenant_id: String,
    pub portfolio_id: String,
    pub site_id: String,
    /// Start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// End-exclusive horizon end, UTC millis.
    pub horizon_end_millis: i64,
    /// The roll trigger that produced this revision (closed P3.24
    /// vocabulary + `initial_solve`).
    pub roll_trigger: String,
    /// The stored revision state: `active` | `superseded` (§14.2; a
    /// superseded revision is retained for audit, never deleted).
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_candidate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by_candidate_id: Option<String>,
    /// The §12.6 candidate identity this revision realizes.
    pub candidate_id: String,
    /// Always `"candidate"` in Phase 3 (module header).
    pub candidate_status: String,
    /// The frozen P3.21 candidate-status statement.
    pub candidate_status_statement: String,
    /// Always `"asset_time_coupled"` in Phase 3.
    pub validation_scope: String,
    /// The recorded ScheduleValidated caveat (asset/time-coupled ONLY;
    /// NOT network feasibility — Phase 4).
    pub validation_scope_statement: String,
    /// The schedule content (per-asset per-interval setpoint plan), as
    /// persisted verbatim.
    pub schedule_payload: serde_json::Value,
}

/// One persisted §12.6 optimization-run (schedule-result) record (P3.28).
/// `status` is the SOLVER outcome (the closed §12.6 vocabulary) — never a
/// feasibility/approval verdict.
#[derive(Debug, Serialize, ToSchema)]
pub struct OptimizationRunDto {
    /// The candidate identity (§12.6 "output candidate ID").
    pub candidate_id: String,
    pub tenant_id: String,
    pub portfolio_id: String,
    pub site_id: String,
    /// The closed §12.6 solver-outcome vocabulary: `optimal` | `feasible` |
    /// `infeasible` | `timeout` | `numerical_failure` | `unsupported`.
    pub status: String,
    pub problem_class: String,
    pub solver_name: String,
    pub solver_version: String,
    pub provider_id: String,
    pub provider_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective_units: Option<String>,
    /// The input-snapshot reference the result cites (§12.6).
    pub input_snapshot_id: String,
    /// The grid-model version (traceable to the site's Phase 2 reference).
    pub grid_model_version: String,
    /// The site whose Phase 2 grid-model reference supplied the version.
    pub grid_model_site_id: String,
    /// The verbatim site grid-model reference (P2.02).
    pub grid_model_site_reference: String,
    /// The deterministic `ScheduleProposed` idempotency key the record was
    /// persisted with (the state+outbox pairing proof, §15.5).
    pub schedule_proposed_key: String,
    /// Always `"candidate"` in Phase 3 (module header).
    pub candidate_status: String,
    /// The frozen P3.21 candidate-status statement.
    pub candidate_status_statement: String,
    /// The full §12.6 record (diagnostics, warnings, assumptions,
    /// determinism fields), as persisted verbatim.
    pub record: serde_json::Value,
}

/// One page of the per-tenant envelope listing (keyset over `envelope_id`;
/// `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct EnvelopePageEnvelope {
    pub data: Vec<EnvelopeRecordDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One page of the per-tenant schedule-revision listing (keyset over
/// `(schedule_id, revision)`; `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct ScheduleRevisionPageEnvelope {
    pub data: Vec<ScheduleRevisionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One page of the per-tenant optimization-run listing (keyset over
/// `candidate_id`; `next_cursor` is absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct OptimizationRunPageEnvelope {
    pub data: Vec<OptimizationRunDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// POST `/api/v1/flexibility/envelopes`: registers one computed portfolio
/// flexibility-envelope record and emits one `FlexibilityEnvelopeUpdated`
/// event per covered site, atomically (§15.5). The computation itself is
/// the P3.12 application function — the route is the persistence + event
/// wiring, not the solver.
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct CreateEnvelopeBody {
    pub portfolio_id: String,
    /// The envelope version (P3.12 `EnvelopeVersion` rules; a duplicate
    /// (portfolio, version) is 409 — versions are immutable).
    pub envelope_version: String,
    /// Start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// End-exclusive horizon end, UTC millis (strictly after the start).
    pub horizon_end_millis: i64,
    /// The interval length, UTC millis (strictly positive).
    pub interval_millis: i64,
    /// The number of intervals (at least 1).
    pub interval_count: u32,
    /// The covered sites (canonical UUIDs; all must exist in the caller's
    /// tenant — one `FlexibilityEnvelopeUpdated` event per site).
    pub covered_site_ids: Vec<String>,
    /// The full envelope computation result as JSON (persisted verbatim).
    pub envelope_payload_json: String,
}

/// POST `/api/v1/schedules`: registers one CANDIDATE schedule revision
/// (P3.22 chain discipline: revision 1 starts a chain; revision n > 1
/// supersedes exactly revision n − 1 in the same transaction) and emits
/// `ScheduleProposed` + `ScheduleValidated` (asset/time-coupled validation
/// ONLY — see the module header), atomically (§15.5).
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct CreateScheduleBody {
    pub portfolio_id: String,
    pub site_id: String,
    /// The schedule-chain identity. Absent = a NEW chain (a UUIDv7 is
    /// generated; `revision` must be 1). Present = continue that chain.
    #[serde(default)]
    pub schedule_id: Option<String>,
    /// The revision number (1 = initial solve; n > 1 supersedes n − 1).
    pub revision: u64,
    /// Start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// End-exclusive horizon end, UTC millis (strictly after the start).
    pub horizon_end_millis: i64,
    /// The roll trigger (closed vocabulary: `initial_solve`,
    /// `horizon_advance`, `envelope_invalidation`, `asset_opt_out_dropout`,
    /// `forecast_update`, `operator_request`).
    pub roll_trigger: String,
    /// The candidate id of the superseded prior revision — required
    /// exactly when `revision > 1`.
    #[serde(default)]
    pub supersedes_candidate_id: Option<String>,
    /// The §12.6 candidate identity this revision realizes — its
    /// optimization-run record must already be registered
    /// (`POST /api/v1/optimization-runs` first).
    pub candidate_id: String,
    /// The recorded asset/time-coupled validation outcome. `false` is
    /// rejected (422): a schedule that fails the Phase 3 validation never
    /// becomes a stored candidate revision, and no `ScheduleValidated` is
    /// emitted for it.
    pub asset_time_coupled_validation_passed: bool,
    /// The schedule content (per-asset per-interval setpoint plan) as JSON
    /// (persisted verbatim).
    pub schedule_payload_json: String,
}

/// POST `/api/v1/optimization-runs`: registers one §12.6 optimization-run
/// (schedule-result) record — persisted BEFORE any downstream use — and
/// emits `ScheduleProposed`, atomically (§15.5; the recorded P3.21
/// persist-before-use statement). The record is a CANDIDATE.
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct CreateOptimizationRunBody {
    pub portfolio_id: String,
    pub site_id: String,
    /// The §12.6 candidate identity (the record's primary identity; a
    /// re-solve is a NEW candidate, never an overwrite — duplicates are
    /// 409).
    pub candidate_id: String,
    /// The closed §12.6 solver-outcome vocabulary: `optimal` | `feasible` |
    /// `infeasible` | `timeout` | `numerical_failure` | `unsupported`.
    pub status: String,
    pub problem_class: String,
    pub solver_name: String,
    pub solver_version: String,
    pub provider_id: String,
    pub provider_version: String,
    /// Objective value, when a solution was produced (canonical units in
    /// `objective_units`; required for `optimal`/`feasible`).
    #[serde(default)]
    pub objective_value: Option<f64>,
    #[serde(default)]
    pub objective_units: Option<String>,
    /// The input-snapshot reference the result cites (non-empty).
    pub input_snapshot_id: String,
    /// The grid-model version. Its traceability is NOT caller-supplied:
    /// the route records the named site's own Phase 2 grid-model reference
    /// verbatim (`grid_model_site_reference`) — the recorded P3.21 rule
    /// that a grid-model version is never detached from its site.
    pub grid_model_version: String,
    /// The full §12.6 record as JSON (persisted verbatim).
    pub record_payload_json: String,
}

/// The shared per-tenant listing parameters (P3.28, recorded limits):
/// `limit` default 200, maximum 1000 (the recorded P2.12 page-size limit;
/// a value over the maximum or below 1 is 422); `cursor` is the opaque
/// keyset value from the previous page (malformed = 422 — never a silent
/// "first page"). There is NO offset pagination and no unbounded query.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct Phase3ListParams {
    /// Restrict to one portfolio (canonical UUID; malformed = 422).
    #[serde(default)]
    pub portfolio_id: Option<String>,
    /// Optimization-run listing only: restrict to one site.
    #[serde(default)]
    pub site_id: Option<String>,
    /// Optimization-run listing only: one closed §12.6 status value.
    #[serde(default)]
    pub status: Option<String>,
    /// Schedule listing only: restrict to one schedule chain (canonical
    /// UUID); when present the FULL chain (all revisions, superseded
    /// included) is returned without pagination.
    #[serde(default)]
    pub schedule_id: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    /// The opaque keyset cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// The recorded P3.28 listing limits, stated once (the P2.12/P2.19
/// convention): default page size and maximum page size. There is no time
/// range on these listings (the records' primary keys are UUIDv7s whose
/// embedded timestamps make the keyset order a stable time order; the
/// portfolio/site/status filters scope the query — recorded in the
/// decision doc). No query spans without a bound: the page size is always
/// applied.
pub const PHASE3_DEFAULT_PAGE_SIZE: u32 = 200;
/// The recorded P3.28 maximum page size (the P2.12 value, reused).
pub const PHASE3_MAX_PAGE_SIZE: u32 = 1000;

/// Maps one storage [`flexcore_storage::schedule_store::ScheduleRecordRow`]
/// onto the API DTO.
pub fn optimization_run_dto(row: &flexcore_storage::schedule_store::ScheduleRecordRow) -> OptimizationRunDto {
    OptimizationRunDto {
        candidate_id: row.candidate_id.clone(),
        tenant_id: row.tenant_id.uuid().to_hyphenated().to_string(),
        portfolio_id: row.portfolio_id.clone(),
        site_id: row.site_id.clone(),
        status: row.status.clone(),
        problem_class: row.problem_class.clone(),
        solver_name: row.solver_name.clone(),
        solver_version: row.solver_version.clone(),
        provider_id: row.provider_id.clone(),
        provider_version: row.provider_version.clone(),
        objective_value: row.objective_value,
        objective_units: row.objective_units.clone(),
        input_snapshot_id: row.input_snapshot_id.clone(),
        grid_model_version: row.grid_model_version.clone(),
        grid_model_site_id: row.grid_model_site_id.clone(),
        grid_model_site_reference: row.grid_model_site_reference.clone(),
        schedule_proposed_key: row.schedule_proposed_key.clone(),
        candidate_status: "candidate".to_string(),
        candidate_status_statement: CANDIDATE_STATUS_STATEMENT.to_string(),
        record: serde_json::from_str(&row.record_payload_json).unwrap_or(serde_json::Value::Null),
    }
}

/// Maps one storage [`flexcore_storage::envelope_store::EnvelopeRecordRow`]
/// onto the API DTO.
pub fn envelope_record_dto(
    row: &flexcore_storage::envelope_store::EnvelopeRecordRow,
) -> EnvelopeRecordDto {
    EnvelopeRecordDto {
        envelope_id: row.envelope_id.clone(),
        tenant_id: row.tenant_id.uuid().to_hyphenated().to_string(),
        portfolio_id: row.portfolio_id.clone(),
        envelope_version: row.envelope_version.clone(),
        horizon_start_millis: row.horizon_start_millis,
        horizon_end_millis: row.horizon_end_millis,
        interval_millis: row.interval_millis,
        interval_count: row.interval_count,
        covered_site_ids: row.covered_site_ids.clone(),
        envelope_payload: serde_json::from_str(&row.envelope_payload_json)
            .unwrap_or(serde_json::Value::Null),
        event_payload_gap_note: ENVELOPE_PAYLOAD_GAP_NOTE.to_string(),
    }
}

/// Maps one storage
/// [`flexcore_storage::schedule_revision::ScheduleRevisionRecord`] onto the
/// API DTO (with the candidate/validation statements).
pub fn schedule_revision_dto(
    record: &flexcore_storage::schedule_revision::ScheduleRevisionRecord,
) -> ScheduleRevisionDto {
    ScheduleRevisionDto {
        schedule_id: record.row.schedule_id.clone(),
        revision: record.row.revision,
        tenant_id: record.row.tenant_id.uuid().to_hyphenated().to_string(),
        portfolio_id: record.row.portfolio_id.clone(),
        site_id: record.row.site_id.clone(),
        horizon_start_millis: record.row.horizon_start_millis,
        horizon_end_millis: record.row.horizon_end_millis,
        roll_trigger: record.row.roll_trigger.clone(),
        state: record.state.clone(),
        supersedes_revision: record.row.supersedes_revision,
        supersedes_candidate_id: record.row.supersedes_candidate_id.clone(),
        superseded_by_revision: record.superseded_by_revision,
        superseded_by_candidate_id: record.superseded_by_candidate_id.clone(),
        candidate_id: record.row.candidate_id.clone(),
        candidate_status: "candidate".to_string(),
        candidate_status_statement: CANDIDATE_STATUS_STATEMENT.to_string(),
        validation_scope: "asset_time_coupled".to_string(),
        validation_scope_statement: SCHEDULE_VALIDATED_SCOPE_STATEMENT.to_string(),
        schedule_payload: serde_json::from_str(&record.row.schedule_payload_json)
            .unwrap_or(serde_json::Value::Null),
    }
}

/// The recorded payload-gap notes the API responses carry (P3.12/P3.21),
/// exposed as one constant for the docs and tests.
pub const PAYLOAD_GAP_NOTES: (&str, &str) = (ENVELOPE_PAYLOAD_GAP_NOTE, SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE);
