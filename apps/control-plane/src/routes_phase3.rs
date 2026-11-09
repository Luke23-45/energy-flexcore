// P3.28 — Phase 3 API routes: the flexibility-envelope, schedule, and
//         optimization-run resources (report §15.1–§15.3, §15.5; ledger
//         P3.28 steps 1–2), on the exact P1.17/P2.12 route pattern.
//
// Every handler: resolve identity (CallerExtractor; 401 when absent or
// invalid) -> authorize (deny-by-default, explicit Phase 3 action; 403 on
// deny) -> run the use case inside ONE unit of work (state.run_in_uow —
// state + outbox atomic, §15.5) -> map errors to the stable envelope
// (401/403/404/409/422/500; cross-tenant references are 404, no existence
// leak).
//
// RECORDED DECISIONS (this module is the decision record for the P3.28
// contract points the ledger asked to decide once; full rationale in
// `ledger/phase-03-flexibility-scheduling/28_api_wiring.md`):
//
// - **NO execute-command endpoint exists.** POST creates a CANDIDATE
//   record only: an envelope record + its events, a schedule revision
//   (CANDIDATE), or a §12.6 optimization-run record (CANDIDATE). There is
//   no approval, dispatch, or execution operation anywhere on this surface
//   (§15.2: "Do not expose a generic execute arbitrary command endpoint";
//   validation, feasibility, and authorization are distinct Phase 4
//   gates). The structural test in `tests/api_phase3.rs` asserts the route
//   set stays free of such an endpoint.
// - **ScheduleValidated scope (the ledger requirement)**: "validated" in
//   Phase 3 means asset/time-coupled constraint validation ONLY (the
//   P3.01–P3.11 capability models + the P3.03 time model — the
//   P3.25/P3.26 assertion surface), explicitly NOT network feasibility
//   (Phase 4, §12.5). The frozen `ScheduleValidatedPayload` cannot carry
//   the caveat (no validation-scope field — recorded gap), so the caveat
//   travels in the record fields (`validation_scope` +
//   `validation_scope_statement` on every schedule response) and in the
//   docs; the event is emitted ONLY for a revision whose recorded
//   asset/time-coupled validation outcome is PASS (a failed outcome is a
//   422 with NOTHING stored and NO event — fail-before-write).
// - **Events via the transactional outbox (§15.5)**: each POST writes the
//   state row(s) and the paired outbox row(s) inside ONE unit-of-work
//   transaction. The event assemblies reuse the existing application
//   surfaces where they fit: the envelope route calls the P3.12
//   `envelope_updated_publications` (one `FlexibilityEnvelopeUpdated` per
//   covered site, deterministic key
//   `flexibility_envelope_updated:<portfolio>:<site>:<version>`); the
//   schedule/run routes emit `ScheduleProposed` with the P3.21
//   deterministic key (`schedule_proposed_idempotency_key`) and the frozen
//   payload. The P3.21 proof-verified `schedule_proposed_publication`
//   requires the typed `ScheduleResultRecord` assembly (the full §12.6
//   result object), which the REST boundary does not receive — so the
//   composition root emits the IDENTICAL frozen payload and key here and
//   records that deviation honestly (same gap convention as the P3.12
//   payload note). The `ScheduleValidated` key mirrors the rule:
//   `schedule_validated:<portfolio>:<site>:<candidate_id>`.
// - **State-change receipts**: the P1.12 §3 canonical entity names cover
//   the registry tables; the Phase 3 record tables are named by their
//   table names (`flexibility_envelopes`, `schedule_results`,
//   `schedule_revisions`). A revision insert pairs with TWO events
//   (`ScheduleProposed` + `ScheduleValidated`), each `OutboxWrite`
//   carrying a receipt descriptor of the SAME state change — the one-
//   receipt-one-event rule is read as one-receipt-per-event here
//   (recorded).
// - **Candidate identity discipline**: a schedule revision references the
//   §12.6 optimization-run record of the SAME candidate (the migration
//   0014 composite FK); the route verifies the record exists in the same
//   transaction (a missing candidate is 422 — POST /api/v1/
//   optimization-runs first). The grid-model traceability of a run is NOT
//   caller-supplied: the route records the named site's own Phase 2
//   grid-model reference verbatim (the P3.21 rule that a grid-model
//   version is never detached from its site).
// - **Listing limits (recorded, the P2.12/P2.19 convention)**: default
//   page 200, maximum page 1000 (over the maximum or below 1 is 422),
//   keyset cursors only (opaque; malformed = 422, never a silent "first
//   page"). NO offset pagination, NO unbounded query, and no time-range
//   parameter (the record primary keys are UUIDv7s whose embedded
//   timestamps make keyset order a stable time order; the
//   portfolio/site/status filters scope each query — recorded).
// - **Cross-tenant reads**: listings are tenant-scoped by construction
//   (the stores filter by the caller's tenant; RLS is the outer gate on
//   the Postgres backend) — another tenant's records are invisible, and a
//   by-id chain query naming a foreign chain is 404.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use flexcore_application::flexibility_envelope::{
    envelope_updated_publications, EnvelopeError, EnvelopePublicationInput, EnvelopeVersion,
};
use flexcore_application::optimization_result_record::schedule_proposed_idempotency_key;
// The trait method `encode` is called on the unit-struct encoder in
// `emit_phase3_event` (the routes_phase2 emission pattern).
use flexcore_application::payload_encoding::EventPayloadEncoder as _;
use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{
    DomainEvent, ScheduleProposedPayload, ScheduleValidatedPayload, event_name,
};
use flexcore_events::subject::subject_for;
use flexcore_ports::event_outbox::{EventOutbox as _, OutboxWrite};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_storage::envelope_store::{EnvelopeListFilter, EnvelopeRecordRow};
use flexcore_storage::schedule_revision::{
    ScheduleRevisionError, ScheduleRevisionRow, ScheduleSupersedeTarget, SCHEDULE_ROLL_TRIGGERS,
};
use flexcore_storage::schedule_store::{ScheduleRecordRow, ScheduleStoreError, SCHEDULE_RESULT_STATUSES};

use crate::auth::authorize_tenant_scoped;
use crate::dto::DataEnvelope;
use crate::dto_phase3::{
    CreateEnvelopeBody, CreateOptimizationRunBody, CreateScheduleBody, EnvelopePageEnvelope,
    EnvelopeRecordDto, OptimizationRunDto, OptimizationRunPageEnvelope, Phase3ListParams,
    ScheduleRevisionDto, ScheduleRevisionPageEnvelope, SCHEDULE_VALIDATED_IDEMPOTENCY_PREFIX,
    SCHEDULE_VALIDATED_SCOPE_STATEMENT, envelope_record_dto, optimization_run_dto,
    schedule_revision_dto,
};
use crate::error::ApiError;
use crate::routes::apply_list_limit;
use crate::state::AppState;

/// The state-change entity names of the Phase 3 record tables (module
/// header note: the P1.12 §3 canonical names cover the registry tables;
/// these are the Phase 3 tables' own names).
const ENTITY_FLEXIBILITY_ENVELOPES: &str = "flexibility_envelopes";
const ENTITY_SCHEDULE_RESULTS: &str = "schedule_results";
const ENTITY_SCHEDULE_REVISIONS: &str = "schedule_revisions";

/// Builds the Phase 3 router (merged into the stage-1 router; a subset of
/// the P0.15 resource list — the §15.2 flexibility-envelope, schedule, and
/// optimization-run resources of the Phase 3 line). There is NO
/// execute-command endpoint and no route outside the resource list.
pub fn phase3_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/flexibility/envelopes",
            post(create_flexibility_envelope).get(list_flexibility_envelopes),
        )
        .route(
            "/api/v1/schedules",
            post(propose_schedule).get(list_schedules),
        )
        .route(
            "/api/v1/optimization-runs",
            post(create_optimization_run).get(list_optimization_runs),
        )
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// The Phase 3 state-change receipt for one tenant-owned record insert.
fn insert_receipt(
    entity: &str,
    tenant_id: TenantId,
) -> StateChangeReceipt {
    StateChangeReceipt::new(entity.to_string(), StateChangeOperation::Insert, Some(tenant_id))
}

/// Assembles and appends ONE transactional outbox write for a frozen §15.3
/// event (the composition-root spelling the P2.12 routes use for
/// `OperatorActionRecorded`; the P3.12/P3.21 proof-verified assemblies are
/// reused where their input shapes fit — module header). Deterministic
/// key, P1.09 envelope, frozen subject, one `OutboxWrite` bound to the
/// receipt of the state change it is paired with.
fn emit_phase3_event(
    outbox: &dyn flexcore_ports::event_outbox::EventOutbox,
    receipt: StateChangeReceipt,
    idempotency_key: String,
    event: DomainEvent,
    event_time: flexcore_contracts::time_model::EventTime,
) -> Result<(), ApiError> {
    let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &idempotency_key, event)
        .map_err(|_| ApiError::infrastructure())?;
    let payload = crate::encoding::JsonEventPayloadEncoder
        .encode(&envelope)
        .map_err(|_| ApiError::infrastructure())?;
    let subject = subject_for(&envelope.event).to_string();
    let write = OutboxWrite::new(receipt, idempotency_key, subject, payload, event_time)
        .map_err(|_| ApiError::infrastructure())?;
    outbox.append(write).map_err(|_| ApiError::infrastructure())
}

/// Parses one canonical UUID reference (a cross-tenant/unknown reference
/// reads as missing: 404, no existence leak — the P1.17 rule).
fn parse_uuid_reference(raw: &str) -> Result<FlexUuid, ApiError> {
    FlexUuid::parse(raw).map_err(|_| ApiError::not_found())
}

/// Parses one canonical UUID filter value (a malformed LIST FILTER is a
/// validation rejection, not a resource reference: 422 — recorded).
fn parse_uuid_filter(raw: &str, field: &str) -> Result<FlexUuid, ApiError> {
    FlexUuid::parse(raw).map_err(|_| {
        ApiError::validation(format!("'{field}' is not a valid UUID: '{raw}'"))
    })
}

/// Maps an envelope-store error onto the API envelope: InvalidRecord is a
/// caller-visible 422, Conflict is the immutability 409, Backend is the
/// opaque 500 (never raw storage text).
fn map_envelope_store_error(e: flexcore_storage::envelope_store::EnvelopeStoreError) -> ApiError {
    use flexcore_storage::envelope_store::EnvelopeStoreError;
    match e {
        EnvelopeStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the envelope record was rejected: '{field}' is missing or malformed"
        )),
        EnvelopeStoreError::Conflict {
            portfolio_id,
            envelope_version,
        } => ApiError::conflict(
            "duplicate_envelope_version",
            format!(
                "an envelope record for portfolio {portfolio_id} version {envelope_version} \
                 already exists; envelope versions are immutable and a recomputation is \
                 a NEW version"
            ),
        ),
        EnvelopeStoreError::Backend { .. } => ApiError::infrastructure(),
    }
}

/// Maps a schedule-record-store error onto the API envelope.
fn map_schedule_store_error(e: ScheduleStoreError) -> ApiError {
    match e {
        ScheduleStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the optimization-run record was rejected: '{field}' is missing or malformed"
        )),
        ScheduleStoreError::Conflict { candidate_id } => ApiError::conflict(
            "candidate_exists",
            format!(
                "an optimization-run record for candidate {candidate_id} already exists; \
                 records are immutable and a re-solve is a NEW candidate"
            ),
        ),
        ScheduleStoreError::Backend { .. } => ApiError::infrastructure(),
    }
}

/// Maps a schedule-revision-store error onto the API envelope.
fn map_revision_error(e: ScheduleRevisionError) -> ApiError {
    match e {
        ScheduleRevisionError::InvalidRevision { field } => ApiError::validation(format!(
            "the schedule revision was rejected: '{field}' is missing or malformed"
        )),
        ScheduleRevisionError::Conflict {
            schedule_id,
            revision,
        } => ApiError::conflict(
            "schedule_revision_exists",
            format!("schedule {schedule_id} revision {revision} already exists; revisions are immutable"),
        ),
        ScheduleRevisionError::ChainBroken {
            schedule_id,
            revision,
            detail,
        } => ApiError::conflict(
            "schedule_chain_conflict",
            format!("schedule {schedule_id} revision {revision} breaks the chain discipline: {detail}"),
        ),
        ScheduleRevisionError::NotFound {
            schedule_id,
            revision,
        } => ApiError::not_found(),
        ScheduleRevisionError::AlreadySuperseded {
            schedule_id,
            revision,
        } => ApiError::conflict(
            "schedule_already_superseded",
            format!(
                "schedule {schedule_id} revision {revision} is already superseded \
                 (§14.2: the transition is one-way)"
            ),
        ),
        ScheduleRevisionError::Backend { .. } => ApiError::infrastructure(),
    }
}

/// Maps the P3.12 envelope-event assembly errors: both are caller-visible
/// validation failures (an empty or duplicated site list).
fn map_envelope_error(e: EnvelopeError) -> ApiError {
    ApiError::validation(e.to_string())
}

/// The keyset cursor of the schedule listing: `<schedule_uuid>:<revision>`.
fn parse_schedule_cursor(raw: &str) -> Result<(FlexUuid, u64), ApiError> {
    let (id_part, revision_part) = raw.rsplit_once(':').ok_or_else(|| {
        ApiError::validation(
            "malformed cursor (an opaque keyset value from the previous page is required)"
                .to_string(),
        )
    })?;
    let schedule_id = parse_uuid_filter(id_part.trim(), "cursor")?;
    let revision = revision_part.trim().parse::<u64>().map_err(|_| {
        ApiError::validation(
            "malformed cursor (an opaque keyset value from the previous page is required)"
                .to_string(),
        )
    })?;
    Ok((schedule_id, revision))
}

// ---------------------------------------------------------------------------
// Handlers — flexibility envelopes (§15.2 /api/v1/flexibility/envelopes)
// ---------------------------------------------------------------------------

/// Registers one computed portfolio flexibility-envelope record (P3.28):
/// the record insert and the one `FlexibilityEnvelopeUpdated` event per
/// covered site (the P3.12 assembly) commit in ONE unit-of-work
/// transaction (§15.5). Cross-tenant portfolio/site references are 404.
/// NO command or execution semantics exist on this route.
#[utoipa::path(
    post,
    path = "/api/v1/flexibility/envelopes",
    request_body = CreateEnvelopeBody,
    responses(
        (status = 201, description = "Envelope record registered (with one FlexibilityEnvelopeUpdated event per covered site, same transaction)", body = DataEnvelope<EnvelopeRecordDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant portfolio/site reference (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Duplicate (portfolio, envelope_version) — versions are immutable", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation rejected", body = crate::error::ErrorEnvelope),
    )
)]
async fn create_flexibility_envelope(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateEnvelopeBody>,
) -> Result<(StatusCode, Json<DataEnvelope<EnvelopeRecordDto>>), ApiError> {
    authorize_tenant_scoped(
        &state.policies,
        &caller,
        Action::FlexibilityEnvelopeCreate,
    )?;
    let portfolio_uuid = parse_uuid_reference(&body.portfolio_id)?;
    let portfolio_id = PortfolioId::new(portfolio_uuid);
    if body.covered_site_ids.is_empty() {
        return Err(ApiError::validation(
            "covered_site_ids must name at least one site (one FlexibilityEnvelopeUpdated \
             event is emitted per site)"
                .to_string(),
        ));
    }
    let mut site_ids = Vec::with_capacity(body.covered_site_ids.len());
    for raw in &body.covered_site_ids {
        let uuid = parse_uuid_reference(raw)?;
        site_ids.push(SiteId::new(uuid));
    }
    let envelope_version = EnvelopeVersion::new(&body.envelope_version)
        .map_err(map_envelope_error)?;
    if body.horizon_end_millis <= body.horizon_start_millis {
        return Err(ApiError::validation(
            "horizon_end_millis must be strictly after horizon_start_millis (§10.3: \
             start-inclusive, end-exclusive)"
                .to_string(),
        ));
    }
    if body.interval_millis <= 0 {
        return Err(ApiError::validation(
            "interval_millis must be strictly positive".to_string(),
        ));
    }
    if body.interval_count == 0 {
        return Err(ApiError::validation(
            "interval_count must be at least 1".to_string(),
        ));
    }
    if serde_json::from_str::<serde_json::Value>(&body.envelope_payload_json).is_err() {
        return Err(ApiError::validation(
            "envelope_payload_json must be valid JSON".to_string(),
        ));
    }
    let ctx = caller.ctx.clone();
    let dto_portfolio = body.portfolio_id.clone();
    let payload_json = body.envelope_payload_json.clone();
    let dto = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            // The portfolio must exist in the caller's tenant (404 — no
            // existence leak).
            if uow
                .portfolios
                .find_by_id(&ctx, portfolio_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            // Every covered site must exist in the caller's tenant (404).
            for site_id in &site_ids {
                if uow
                    .sites
                    .find_by_id(&ctx, *site_id)
                    .map_err(ApiError::from)?
                    .is_none()
                {
                    return Err(ApiError::not_found());
                }
            }
            let envelope_id = uow.ids.new_uuid().to_hyphenated();
            let row = EnvelopeRecordRow::new(
                ctx.tenant_id(),
                dto_portfolio.clone(),
                envelope_id.clone(),
                envelope_version.as_str().to_string(),
                body.horizon_start_millis,
                body.horizon_end_millis,
                body.interval_millis,
                body.interval_count,
                body.covered_site_ids.clone(),
                payload_json.clone(),
            )
            .map_err(map_envelope_store_error)?;
            uow.envelope_records
                .insert_envelope_record(&ctx, &row)
                .map_err(map_envelope_store_error)?;
            // One FlexibilityEnvelopeUpdated per covered site, SAME
            // transaction (§15.5): the P3.12 assembly produces the frozen
            // payload + deterministic key + subject; this composition root
            // serializes and appends.
            let correlation_id = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            let publications = envelope_updated_publications(EnvelopePublicationInput {
                correlation_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id,
                site_ids: &site_ids,
                envelope_version: &envelope_version,
                event_time,
            })
            .map_err(map_envelope_error)?;
            for publication in publications {
                emit_phase3_event(
                    uow.outbox.as_ref(),
                    insert_receipt(ENTITY_FLEXIBILITY_ENVELOPES, ctx.tenant_id()),
                    publication.idempotency_key,
                    publication.envelope.event,
                    event_time,
                )?;
            }
            Ok(envelope_record_dto(&row))
        })
        .await?;
    Ok((StatusCode::CREATED, Json(DataEnvelope { data: dto })))
}

/// Lists the caller's tenant-scoped envelope records: keyset pagination
/// over `envelope_id` (a UUIDv7 — identifier order is a stable time
/// order), the recorded limits (default 200, max 1000), the optional
/// portfolio filter. Records of other tenants never appear; there is NO
/// offset pagination and NO unbounded query.
#[utoipa::path(
    get,
    path = "/api/v1/flexibility/envelopes",
    params(Phase3ListParams),
    responses(
        (status = 200, body = EnvelopePageEnvelope),
        (status = 401), (status = 403),
        (status = 422, description = "Malformed cursor/filter or oversized page", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_flexibility_envelopes(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<Phase3ListParams>,
) -> Result<Json<EnvelopePageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::FlexibilityEnvelopeRead)?;
    let limit = apply_list_limit(params.limit)?;
    let portfolio_filter = match params.portfolio_id.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "portfolio_id")?.to_hyphenated()),
    };
    let after_envelope_id = match params.cursor.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "cursor")?.to_hyphenated()),
    };
    let ctx = caller.ctx.clone();
    let rows = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let filter = EnvelopeListFilter {
                portfolio_id: portfolio_filter,
                after_envelope_id,
            };
            uow.envelope_records
                .list_envelope_records(&ctx, &filter, limit)
                .map_err(map_envelope_store_error)
        })
        .await?;
    let next_cursor = if rows.len() == limit {
        rows.last().map(|r| r.envelope_id.clone())
    } else {
        None
    };
    Ok(Json(EnvelopePageEnvelope {
        data: rows.iter().map(envelope_record_dto).collect(),
        next_cursor,
    }))
}

// ---------------------------------------------------------------------------
// Handlers — optimization runs (§15.2 /api/v1/optimization-runs)
// ---------------------------------------------------------------------------

/// Registers one §12.6 optimization-run (schedule-result) record —
/// persisted BEFORE any downstream use (§16.1) — and emits `ScheduleProposed`
/// with the P3.21 deterministic key, in ONE unit-of-work transaction
/// (§15.5). The record is a CANDIDATE: `status` is the SOLVER outcome,
/// never a network-feasibility, authorization, or dispatch verdict. The
/// grid-model traceability is taken from the named site's Phase 2
/// reference, not the caller.
#[utoipa::path(
    post,
    path = "/api/v1/optimization-runs",
    request_body = CreateOptimizationRunBody,
    responses(
        (status = 201, description = "Optimization-run record registered (a CANDIDATE; ScheduleProposed emitted in the same transaction)", body = DataEnvelope<OptimizationRunDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant portfolio/site reference (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Duplicate candidate — records are immutable, a re-solve is a NEW candidate", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation rejected", body = crate::error::ErrorEnvelope),
    )
)]
async fn create_optimization_run(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateOptimizationRunBody>,
) -> Result<(StatusCode, Json<DataEnvelope<OptimizationRunDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::OptimizationRunCreate)?;
    let portfolio_uuid = parse_uuid_reference(&body.portfolio_id)?;
    let site_uuid = parse_uuid_reference(&body.site_id)?;
    let candidate_uuid = parse_uuid_reference(&body.candidate_id)?;
    let portfolio_id = PortfolioId::new(portfolio_uuid);
    let site_id = SiteId::new(site_uuid);
    let ctx = caller.ctx.clone();
    let dto = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            // The ownership chain must exist in the caller's tenant (404 —
            // no existence leak).
            if uow
                .portfolios
                .find_by_id(&ctx, portfolio_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let site = uow
                .sites
                .find_by_id(&ctx, site_id)
                .map_err(ApiError::from)?
                .ok_or_else(ApiError::not_found)?;
            // The deterministic ScheduleProposed idempotency key (P3.21
            // rule) — recorded ON the row as the state+outbox pairing
            // proof.
            let proposed_key = schedule_proposed_idempotency_key(
                portfolio_id,
                site_id,
                candidate_uuid,
            );
            // The grid-model traceability is NOT caller-supplied: the
            // version is traceable to the site's own Phase 2
            // grid-model reference (P3.21 GRID_MODEL_PROVENANCE rule).
            let row = ScheduleRecordRow::new(
                ctx.tenant_id(),
                body.portfolio_id.clone(),
                body.site_id.clone(),
                body.candidate_id.clone(),
                body.status.clone(),
                body.problem_class.clone(),
                body.solver_name.clone(),
                body.solver_version.clone(),
                body.provider_id.clone(),
                body.provider_version.clone(),
                body.objective_value,
                body.objective_units.clone(),
                body.input_snapshot_id.clone(),
                body.grid_model_version.clone(),
                body.site_id.clone(),
                site.grid_model_reference().to_string(),
                proposed_key.clone(),
                body.record_payload_json.clone(),
            )
            .map_err(map_schedule_store_error)?;
            uow.schedule_records
                .insert_schedule_record(&ctx, &row)
                .map_err(map_schedule_store_error)?;
            // ScheduleProposed, SAME transaction (§15.5): the frozen
            // payload (the nil asset id denotes the schedule scope — the
            // recorded P3.21 convention) with the deterministic key.
            let correlation_id = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            let payload = ScheduleProposedPayload {
                correlation_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id,
                site_id,
                asset_id: AssetId::new(FlexUuid::nil()),
                event_time,
            };
            emit_phase3_event(
                uow.outbox.as_ref(),
                insert_receipt(ENTITY_SCHEDULE_RESULTS, ctx.tenant_id()),
                proposed_key,
                DomainEvent::ScheduleProposed(payload),
                event_time,
            )?;
            Ok(optimization_run_dto(&row))
        })
        .await?;
    Ok((StatusCode::CREATED, Json(DataEnvelope { data: dto })))
}

/// Lists the caller's tenant-scoped optimization-run records: keyset
/// pagination over `candidate_id`, the recorded limits, the optional
/// portfolio/site/status filters (the status filter accepts ONLY the
/// closed §12.6 vocabulary — any seventh value is 422).
#[utoipa::path(
    get,
    path = "/api/v1/optimization-runs",
    params(Phase3ListParams),
    responses(
        (status = 200, body = OptimizationRunPageEnvelope),
        (status = 401), (status = 403),
        (status = 422, description = "Malformed cursor/filter, oversized page, or seventh status value", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_optimization_runs(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<Phase3ListParams>,
) -> Result<Json<OptimizationRunPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::OptimizationRunRead)?;
    let limit = apply_list_limit(params.limit)?;
    let portfolio_filter = match params.portfolio_id.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "portfolio_id")?.to_hyphenated()),
    };
    let site_filter = match params.site_id.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "site_id")?.to_hyphenated()),
    };
    let status_filter = match params.status.as_deref() {
        None => None,
        Some(raw) => {
            if !SCHEDULE_RESULT_STATUSES.contains(&raw) {
                return Err(ApiError::validation(format!(
                    "unknown status '{raw}'; the §12.6 vocabulary is closed \
                     (optimal|feasible|infeasible|timeout|numerical_failure|unsupported)"
                )));
            }
            Some(raw.to_string())
        }
    };
    let after_candidate_id = match params.cursor.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "cursor")?.to_hyphenated()),
    };
    let ctx = caller.ctx.clone();
    let rows = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.schedule_records
                .list_schedule_records(
                    &ctx,
                    portfolio_filter.as_deref(),
                    site_filter.as_deref(),
                    status_filter.as_deref(),
                    after_candidate_id.as_deref(),
                    limit,
                )
                .map_err(map_schedule_store_error)
        })
        .await?;
    let next_cursor = if rows.len() == limit {
        rows.last().map(|r| r.candidate_id.clone())
    } else {
        None
    };
    Ok(Json(OptimizationRunPageEnvelope {
        data: rows.iter().map(optimization_run_dto).collect(),
        next_cursor,
    }))
}

// ---------------------------------------------------------------------------
// Handlers — schedules (§15.2 /api/v1/schedules)
// ---------------------------------------------------------------------------

/// Proposes one CANDIDATE schedule revision (P3.28): the revision insert
/// (with the P3.22 chain discipline — revision n > 1 supersedes exactly
/// n − 1 in the SAME transaction), plus `ScheduleProposed` and — for a
/// revision whose recorded asset/time-coupled validation outcome is PASS —
/// `ScheduleValidated`, in ONE unit-of-work transaction (§15.5).
///
/// **Scope of "validated" (the ledger requirement):** asset/time-coupled
/// constraint validation ONLY (capability models + time model; the
/// P3.25/P3.26 assertion surface). It is NOT network feasibility
/// (Phase 4, §12.5). A revision whose recorded validation outcome is FAIL
/// is rejected (422) with NOTHING stored and NO event. Every response
/// carries `candidate_status: "candidate"` and
/// `validation_scope: "asset_time_coupled"` with the recorded statement.
/// There is NO approval, dispatch, or execution operation on this surface.
#[utoipa::path(
    post,
    path = "/api/v1/schedules",
    request_body = CreateScheduleBody,
    responses(
        (status = 201, description = "Candidate schedule revision registered (ScheduleProposed and — for a passing asset/time-coupled validation — ScheduleValidated emitted in the same transaction; NOT a feasibility, authorization, or dispatch verdict)", body = DataEnvelope<ScheduleRevisionDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant portfolio/site/candidate reference (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Revision exists, chain conflict, or predecessor already superseded", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Validation rejected (including a FAILED asset/time-coupled validation outcome — nothing is stored)", body = crate::error::ErrorEnvelope),
    )
)]
async fn propose_schedule(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateScheduleBody>,
) -> Result<(StatusCode, Json<DataEnvelope<ScheduleRevisionDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::SchedulePropose)?;
    let portfolio_uuid = parse_uuid_reference(&body.portfolio_id)?;
    let site_uuid = parse_uuid_reference(&body.site_id)?;
    let candidate_uuid = parse_uuid_reference(&body.candidate_id)?;
    let portfolio_id = PortfolioId::new(portfolio_uuid);
    let site_id = SiteId::new(site_uuid);
    // The ledger requirement, enforced at the boundary: only a revision
    // whose recorded asset/time-coupled validation PASSED becomes a stored
    // candidate, and only it gets a ScheduleValidated event.
    if !body.asset_time_coupled_validation_passed {
        return Err(ApiError::validation(
            "the recorded asset/time-coupled validation outcome is FAIL: a schedule that \
             fails the Phase 3 validation never becomes a stored candidate revision and no \
             ScheduleValidated event is emitted for it"
                .to_string(),
        ));
    }
    // Chain discipline pre-checks (the store enforces them again inside
    // the transaction): a continuation must name its chain; the supersede
    // link must be the one-step gapless pair.
    let supersedes_revision = match body.revision {
        0 => {
            return Err(ApiError::validation(
                "revision must be at least 1 (1 = initial solve)".to_string(),
            ))
        }
        1 => None,
        n => {
            if body.schedule_id.is_none() {
                return Err(ApiError::validation(
                    "revision > 1 continues an existing chain: schedule_id is required"
                        .to_string(),
                ));
            }
            let superseded_candidate = body
                .supersedes_candidate_id
                .as_deref()
                .ok_or_else(|| {
                    ApiError::validation(
                        "revision > 1 supersedes the prior revision: supersedes_candidate_id \
                         is required"
                            .to_string(),
                    )
                })?
                .to_string();
            parse_uuid_reference(&superseded_candidate)?;
            Some(n - 1)
        }
    };
    if !SCHEDULE_ROLL_TRIGGERS.contains(&body.roll_trigger.as_str()) {
        return Err(ApiError::validation(format!(
            "unknown roll_trigger '{}'; the recorded vocabulary is closed \
             (initial_solve|horizon_advance|envelope_invalidation|asset_opt_out_dropout|\
             forecast_update|operator_request)",
            body.roll_trigger
        )));
    }
    if body.horizon_end_millis <= body.horizon_start_millis {
        return Err(ApiError::validation(
            "horizon_end_millis must be strictly after horizon_start_millis (§10.3: \
             start-inclusive, end-exclusive)"
                .to_string(),
        ));
    }
    if serde_json::from_str::<serde_json::Value>(&body.schedule_payload_json).is_err() {
        return Err(ApiError::validation(
            "schedule_payload_json must be valid JSON".to_string(),
        ));
    }
    let ctx = caller.ctx.clone();
    let dto = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            // The referenced §12.6 candidate record must already be
            // registered (the migration 0014 composite FK discipline; the
            // revision and the run are two views of ONE solve).
            if uow
                .schedule_records
                .find_schedule_record(&ctx, &body.candidate_id)
                .map_err(map_schedule_store_error)?
                .is_none()
            {
                return Err(ApiError::validation(format!(
                    "the schedule revision references candidate {}, whose optimization-run \
                     record is not registered (POST /api/v1/optimization-runs first)",
                    body.candidate_id
                )));
            }
            // The ownership chain must exist in the caller's tenant (404).
            if uow
                .portfolios
                .find_by_id(&ctx, portfolio_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            if uow
                .sites
                .find_by_id(&ctx, site_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let schedule_id = match body.schedule_id.as_deref() {
                Some(existing) => existing.to_string(),
                None => uow.ids.new_uuid().to_hyphenated(),
            };
            let row = ScheduleRevisionRow::new(
                ctx.tenant_id(),
                body.portfolio_id.clone(),
                body.site_id.clone(),
                schedule_id.clone(),
                body.revision,
                body.horizon_start_millis,
                body.horizon_end_millis,
                body.roll_trigger.clone(),
                supersedes_revision,
                body.supersedes_candidate_id.clone(),
                body.candidate_id.clone(),
                body.schedule_payload_json.clone(),
            )
            .map_err(map_revision_error)?;
            // Revision n > 1: mark the predecessor SUPERSEDED in THIS
            // transaction first (the store verifies the gapless pair).
            if body.revision > 1 {
                let target = ScheduleSupersedeTarget::new(
                    row.revision - 1,
                    body.supersedes_candidate_id
                        .clone()
                        .expect("checked above"),
                )
                .map_err(map_revision_error)?;
                uow.schedule_revisions
                    .mark_revision_superseded(&ctx, &row.schedule_id, row.revision - 1, &target)
                    .map_err(map_revision_error)?;
            }
            uow.schedule_revisions
                .insert_schedule_revision(&ctx, &row)
                .map_err(map_revision_error)?;
            // Two events, SAME transaction (§15.5): ScheduleProposed (the
            // P3.21 deterministic key + frozen payload) and
            // ScheduleValidated — which, in Phase 3, records the
            // asset/time-coupled validation ONLY (module header; the
            // caveat travels in the record fields, the frozen payload has
            // no scope field — recorded gap).
            let correlation_id = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            let proposed_payload = ScheduleProposedPayload {
                correlation_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id,
                site_id,
                asset_id: AssetId::new(FlexUuid::nil()),
                event_time,
            };
            let proposed_key = schedule_proposed_idempotency_key(
                portfolio_id,
                site_id,
                candidate_uuid,
            );
            emit_phase3_event(
                uow.outbox.as_ref(),
                insert_receipt(ENTITY_SCHEDULE_REVISIONS, ctx.tenant_id()),
                proposed_key,
                DomainEvent::ScheduleProposed(proposed_payload),
                event_time,
            )?;
            let validated_key = format!(
                "{}:{}:{}:{}",
                SCHEDULE_VALIDATED_IDEMPOTENCY_PREFIX,
                body.portfolio_id,
                body.site_id,
                body.candidate_id
            );
            let validated_payload = ScheduleValidatedPayload {
                correlation_id,
                tenant_id: ctx.tenant_id(),
                portfolio_id,
                site_id,
                asset_id: AssetId::new(FlexUuid::nil()),
                event_time,
            };
            emit_phase3_event(
                uow.outbox.as_ref(),
                insert_receipt(ENTITY_SCHEDULE_REVISIONS, ctx.tenant_id()),
                validated_key,
                DomainEvent::ScheduleValidated(validated_payload),
                event_time,
            )?;
            // The response record: active state, no forward link (the
            // insert built it that way; the fields carry the recorded
            // candidate/validation statements).
            let record = flexcore_storage::schedule_revision::ScheduleRevisionRecord::new(
                row,
                flexcore_storage::schedule_revision::SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
                None,
                None,
            )
            .map_err(|_| ApiError::infrastructure())?;
            Ok(schedule_revision_dto(&record))
        })
        .await?;
    Ok((StatusCode::CREATED, Json(DataEnvelope { data: dto })))
}

/// Lists the caller's tenant-scoped schedule revisions: with `schedule_id`
/// the FULL revision chain is returned (every superseded revision
/// retained for audit, §14.2); without it a keyset-paginated page over
/// `(schedule_id, revision)` with the recorded limits and the optional
/// portfolio filter. A foreign-tenant chain is 404 (no existence leak).
#[utoipa::path(
    get,
    path = "/api/v1/schedules",
    params(Phase3ListParams),
    responses(
        (status = 200, body = ScheduleRevisionPageEnvelope),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant schedule chain (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Malformed cursor/filter or oversized page", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_schedules(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<Phase3ListParams>,
) -> Result<Json<ScheduleRevisionPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::ScheduleRead)?;
    let ctx = caller.ctx.clone();
    match params.schedule_id.as_deref() {
        Some(schedule_id_raw) => {
            let schedule_uuid = parse_uuid_reference(schedule_id_raw)?;
            let schedule_id = schedule_uuid.to_hyphenated();
            let records = state
                .run_in_uow(Some(ctx.clone()), move |uow| {
                    // A foreign-tenant chain reads as missing (404 — no
                    // existence leak); the store scopes by tenant.
                    let chain = uow
                        .schedule_revisions
                        .revision_chain(&ctx, &schedule_id)
                        .map_err(map_revision_error)?;
                    if chain.is_empty() {
                        return Err(ApiError::not_found());
                    }
                    Ok(chain)
                })
                .await?;
            Ok(Json(ScheduleRevisionPageEnvelope {
                data: records.iter().map(schedule_revision_dto).collect(),
                next_cursor: None,
            }))
        }
        None => {
            let limit = apply_list_limit(params.limit)?;
            let portfolio_filter = match params.portfolio_id.as_deref() {
                None => None,
                Some(raw) => Some(parse_uuid_filter(raw, "portfolio_id")?.to_hyphenated()),
            };
            let (after_schedule_id, after_revision) = match params.cursor.as_deref() {
                None => (None, None),
                Some(raw) => {
                    let (schedule, revision) = parse_schedule_cursor(raw)?;
                    (
                        Some(schedule.to_hyphenated()),
                        Some(revision),
                    )
                }
            };
            let records = state
                .run_in_uow(Some(ctx.clone()), move |uow| {
                    uow.schedule_revisions
                        .list_revisions(
                            &ctx,
                            portfolio_filter.as_deref(),
                            after_schedule_id.as_deref(),
                            after_revision,
                            limit,
                        )
                        .map_err(map_revision_error)
                })
                .await?;
            let next_cursor = if records.len() == limit {
                records.last().map(|r| {
                    format!("{}:{}", r.row.schedule_id, r.row.revision)
                })
            } else {
                None
            };
            Ok(Json(ScheduleRevisionPageEnvelope {
                data: records.iter().map(schedule_revision_dto).collect(),
                next_cursor,
            }))
        }
    }
}

/// The recorded ScheduleValidated scope statement, re-exported for the
/// contract tests (the response field and the docs carry the same text).
pub fn schedule_validated_scope_statement() -> &'static str {
    SCHEDULE_VALIDATED_SCOPE_STATEMENT
}

/// The frozen event names this surface emits, re-exported for the
/// structural test (rendered through the event crate's own `event_name`
/// over the frozen payloads, so the names can never drift from
/// flexcore-events).
pub fn emitted_event_names() -> [&'static str; 3] {
    fn nil_event_time() -> flexcore_contracts::time_model::EventTime {
        flexcore_contracts::time_model::EventTime::from_unix_millis(0)
            .expect("the Unix epoch is a valid event time")
    }
    [
        event_name(&DomainEvent::FlexibilityEnvelopeUpdated(
            flexcore_events::events::FlexibilityEnvelopeUpdatedPayload {
                correlation_id: FlexUuid::nil(),
                tenant_id: TenantId::new(FlexUuid::nil()),
                portfolio_id: PortfolioId::new(FlexUuid::nil()),
                site_id: SiteId::new(FlexUuid::nil()),
                asset_id: AssetId::new(FlexUuid::nil()),
                event_time: nil_event_time(),
            },
        )),
        event_name(&DomainEvent::ScheduleProposed(ScheduleProposedPayload {
            correlation_id: FlexUuid::nil(),
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            asset_id: AssetId::new(FlexUuid::nil()),
            event_time: nil_event_time(),
        })),
        event_name(&DomainEvent::ScheduleValidated(ScheduleValidatedPayload {
            correlation_id: FlexUuid::nil(),
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            asset_id: AssetId::new(FlexUuid::nil()),
            event_time: nil_event_time(),
        })),
    ]
}
