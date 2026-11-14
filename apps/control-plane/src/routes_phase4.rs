// P4.30 — Phase 4 API routes: the dispatch-command and feasibility-check
//         READ resources (report §15.2, §16.1, §21.3; ledger P4.30 step 3;
//         decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md`),
//         on the exact P1.17/P2.12/P3.28 route pattern.
//         P4.36 appended additively: the `/api/v1/audit-events` READ
//         resource (report §15.2, §16.2; decision record
//         `36_audit_read_api.md`) — same pattern, the `audit.event.read`
//         action, NO update/delete/export endpoint (the trail is
//         append-only).
//
// Every handler: resolve identity (CallerExtractor; 401 when absent or
// invalid) -> authorize (deny-by-default, explicit Phase 4 read action;
// 403 on deny) -> read inside ONE unit of work (state.run_in_uow) -> map
// errors to the stable envelope (401/403/404/422/500; cross-tenant
// references are 404, no existence leak).
//
// RECORDED DECISIONS (full rationale in the decision record):
//
// - **READ-ONLY — NO execute-command endpoint exists (§15.2).** Both
//   resources serve persisted evidence. There is no POST/PUT/PATCH/DELETE
//   and no generic "execute arbitrary command" operation anywhere on this
//   surface. The §14.1 transitions happen exclusively through the
//   application-layer gates (`flexcore_application::command_gates`),
//   which are protected-kernel surfaces with their own distinct
//   authorization actions (`command.create` / `command.validate` /
//   `command.approve` — flexcore-authz; `dispatch.authorize` /
//   `dispatch.issue` do not exist on the action surface at all, §5.7).
//   Exposing gate operations over REST is NOT part of P4.30 and is not
//   added here (the gates are invoked by the composition root's use
//   cases; a REST write path would be a new subtask decision). The
//   structural test in `tests/api_phase4.rs` asserts the route set stays
//   free of any non-read method on these resources.
// - **Tenant scoping**: listings filter by the caller's tenant in the
//   store (and under RLS on the Postgres backend); a by-id read naming a
//   foreign-tenant record is 404 — existence never leaks (P0.16 rule 5).
// - **Listing limits (recorded, the P2.12/P2.19/P3.28 convention)**:
//   default page 200, maximum page 1000 (over the maximum or below 1 is
//   422), keyset cursors only (opaque; malformed = 422, never a silent
//   "first page"). NO offset pagination, NO unbounded query.
// - **Correlation (§21.3)**: every served record carries the tenant/
//   portfolio/site/asset scope, the schedule + command identities, the
//   model id/version/hash where applicable, the acting identity, and the
//   correlation (trace) id. No secrets and no unnecessary personal or
//   customer data appear in any field (the actor is an identity
//   reference).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_storage::audit_read::{AuditEventFilter, AuditEventReader as _};
use flexcore_storage::command_store::{CommandListFilter, CommandRecordStore as _};
use flexcore_storage::feasibility_check::{FeasibilityCheckFilter, FeasibilityCheckStore as _};

use crate::auth::authorize_tenant_scoped;
use crate::dto::DataEnvelope;
use crate::dto_phase4::{
    AuditEventDto, AuditEventListParams, AuditEventPageEnvelope, CommandTransitionDto,
    DispatchCommandDetailDto, DispatchCommandDto, DispatchCommandPageEnvelope,
    FeasibilityCheckDto, FeasibilityCheckPageEnvelope, Phase4ListParams,
    audit_event_dto, command_transition_dto, dispatch_command_dto, feasibility_check_dto,
};
use crate::error::ApiError;
use crate::routes::apply_list_limit;
use crate::state::AppState;

/// Builds the Phase 4 router (merged into the stage-1 router; the §15.2
/// dispatch-command, feasibility-check, and — P4.36 — audit-events
/// resources of the Phase 4 line, READ-ONLY). There is NO execute-command
/// endpoint, NO update/delete on the audit trail, and no route outside
/// the resource list.
pub fn phase4_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/dispatch-commands",
            get(list_dispatch_commands),
        )
        .route(
            "/api/v1/dispatch-commands/{command_id}",
            get(get_dispatch_command),
        )
        .route(
            "/api/v1/feasibility-checks",
            get(list_feasibility_checks),
        )
        .route(
            "/api/v1/feasibility-checks/{check_id}",
            get(get_feasibility_check),
        )
        .route(
            "/api/v1/audit-events",
            get(list_audit_events),
        )
        .route(
            "/api/v1/audit-events/{event_id}",
            get(get_audit_event),
        )
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parses one canonical UUID reference (a cross-tenant/unknown reference
/// reads as missing: 404, no existence leak — the P1.17 rule).
fn parse_uuid_reference(raw: &str) -> Result<FlexUuid, ApiError> {
    FlexUuid::parse(raw).map_err(|_| ApiError::not_found())
}

/// Parses one canonical UUID filter value (a malformed LIST FILTER is a
/// validation rejection, not a resource reference: 422 — the recorded
/// P3.28 convention).
fn parse_uuid_filter(raw: &str, field: &str) -> Result<FlexUuid, ApiError> {
    FlexUuid::parse(raw).map_err(|_| {
        ApiError::validation(format!("'{field}' is not a valid UUID: '{raw}'"))
    })
}

/// The keyset cursor of the dispatch-command listing: the last
/// `command_id` of the previous page.
fn parse_command_cursor(raw: &str) -> Result<String, ApiError> {
    FlexUuid::parse(raw.trim())
        .map(|_| raw.trim().to_string())
        .map_err(|_| {
            ApiError::validation(
                "malformed cursor (an opaque keyset value from the previous page is required)"
                    .to_string(),
            )
        })
}

// ---------------------------------------------------------------------------
// Handlers — dispatch commands (§15.2 /api/v1/dispatch-commands)
// ---------------------------------------------------------------------------

/// Lists the caller's tenant's dispatch commands, newest first (keyset
/// over `command_id`). READ-ONLY: no execute-command operation exists
/// (§15.2).
#[utoipa::path(
    get,
    path = "/api/v1/dispatch-commands",
    params(Phase4ListParams),
    responses(
        (status = 200, description = "One page of the tenant's dispatch commands (newest first; the records carry the simulation-only and no-execute statements)", body = DispatchCommandPageEnvelope),
        (status = 401), (status = 403),
        (status = 422, description = "Malformed filter/cursor or out-of-range limit", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_dispatch_commands(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<Phase4ListParams>,
) -> Result<Json<DispatchCommandPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::CommandRead)?;
    let limit = apply_list_limit(params.limit)?;
    let state_filter = match params.state.as_deref() {
        None => None,
        Some(value) => {
            if !flexcore_storage::command_store::COMMAND_STATES.contains(&value) {
                return Err(ApiError::validation(format!(
                    "'state' is not a valid command state: '{value}'"
                )));
            }
            Some(value.to_string())
        }
    };
    let after_command_id = match params.cursor.as_deref() {
        None => None,
        Some(raw) => Some(parse_command_cursor(raw)?),
    };
    let ctx = caller.ctx.clone();
    let page = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.command_records
                .list_commands(
                    &ctx,
                    &CommandListFilter {
                        state: state_filter,
                        after_command_id,
                    },
                    limit as u32,
                )
                .map_err(map_command_store_error)
        })
        .await?;
    let next_cursor = if page.len() == limit {
        page.last().map(|record| record.row.command_id.clone())
    } else {
        None
    };
    Ok(Json(DispatchCommandPageEnvelope {
        data: page.iter().map(dispatch_command_dto).collect(),
        next_cursor,
    }))
}

/// Reads one dispatch command with its FULL transition journal (the audit
/// view). A foreign-tenant command is 404 — no existence leak.
#[utoipa::path(
    get,
    path = "/api/v1/dispatch-commands/{command_id}",
    params(("command_id" = String, Path, description = "The dispatch command id (canonical UUID)")),
    responses(
        (status = 200, description = "The command record with its full transition journal (never rewritten — the reconstruction contract)", body = DataEnvelope<DispatchCommandDetailDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant command (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Malformed command id", body = crate::error::ErrorEnvelope),
    )
)]
async fn get_dispatch_command(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(command_id): Path<String>,
) -> Result<Json<DataEnvelope<DispatchCommandDetailDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::CommandRead)?;
    parse_uuid_reference(&command_id)?;
    let ctx = caller.ctx.clone();
    let detail = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let record = uow
                .command_records
                .find_command(&ctx, &command_id)
                .map_err(map_command_store_error)?
                .ok_or_else(ApiError::not_found)?;
            let journal = uow
                .command_records
                .transition_journal(&ctx, &command_id)
                .map_err(map_command_store_error)?;
            let transitions: Vec<CommandTransitionDto> = journal
                .iter()
                .enumerate()
                .map(|(index, entry)| command_transition_dto(index as i64 + 1, entry))
                .collect();
            Ok(DispatchCommandDetailDto {
                command: dispatch_command_dto(&record),
                journal: transitions,
            })
        })
        .await?;
    Ok(Json(DataEnvelope { data: detail }))
}

// ---------------------------------------------------------------------------
// Handlers — feasibility checks (§15.2 /api/v1/feasibility-checks)
// ---------------------------------------------------------------------------

/// Lists the caller's tenant's persisted §12.5 feasibility-check evidence
/// records (P4.30, migration 0017). READ-ONLY.
#[utoipa::path(
    get,
    path = "/api/v1/feasibility-checks",
    params(Phase4ListParams),
    responses(
        (status = 200, description = "One page of the tenant's feasibility-check evidence records (ordered by completion time then check id)", body = FeasibilityCheckPageEnvelope),
        (status = 401), (status = 403),
        (status = 422, description = "Malformed filter/cursor or out-of-range limit", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_feasibility_checks(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<Phase4ListParams>,
) -> Result<Json<FeasibilityCheckPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::FeasibilityRead)?;
    let limit = apply_list_limit(params.limit)?;
    let command_id = match params.command_id.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "command_id")?.to_hyphenated().to_string()),
    };
    let schedule_id = match params.schedule_id.as_deref() {
        None => None,
        Some(raw) => Some(parse_uuid_filter(raw, "schedule_id")?.to_hyphenated().to_string()),
    };
    let after_check_id = params.cursor.clone();
    let ctx = caller.ctx.clone();
    let page = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.feasibility_checks
                .list_feasibility_checks(
                    &ctx,
                    &FeasibilityCheckFilter {
                        command_id,
                        schedule_id,
                        after_check_id,
                    },
                    limit as u32,
                )
                .map_err(map_feasibility_check_error)
        })
        .await?;
    let next_cursor = if page.len() == limit {
        page.last().map(|row| row.check_id.clone())
    } else {
        None
    };
    Ok(Json(FeasibilityCheckPageEnvelope {
        data: page.iter().map(feasibility_check_dto).collect(),
        next_cursor,
    }))
}

/// Reads one feasibility-check evidence record. A foreign-tenant check is
/// 404 — no existence leak.
#[utoipa::path(
    get,
    path = "/api/v1/feasibility-checks/{check_id}",
    params(("check_id" = String, Path, description = "The feasibility-check id (the §12.5 attempt id or the pipeline-level gate:<candidate> id)")),
    responses(
        (status = 200, description = "The check record (verdict, model binding, margins/coverage, actor, correlation id)", body = DataEnvelope<FeasibilityCheckDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant check (no existence leak)", body = crate::error::ErrorEnvelope),
    )
)]
async fn get_feasibility_check(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(check_id): Path<String>,
) -> Result<Json<DataEnvelope<FeasibilityCheckDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::FeasibilityRead)?;
    if check_id.trim().is_empty() {
        return Err(ApiError::not_found());
    }
    let ctx = caller.ctx.clone();
    let row = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.feasibility_checks
                .find_feasibility_check(&ctx, check_id.trim())
                .map_err(map_feasibility_check_error)?
                .ok_or_else(ApiError::not_found)
        })
        .await?;
    Ok(Json(DataEnvelope {
        data: feasibility_check_dto(&row),
    }))
}

// ---------------------------------------------------------------------------
// Handlers — audit events (§15.2 /api/v1/audit-events; P4.36)
// ---------------------------------------------------------------------------

/// Parses one non-empty exact-match filter value (an empty/whitespace
/// LIST FILTER is a validation rejection, not a resource reference: 422 —
/// the recorded convention).
fn require_nonempty_filter(raw: &str, field: &str) -> Result<String, ApiError> {
    if raw.trim().is_empty() {
        Err(ApiError::validation(format!(
            "'{field}' must not be empty"
        )))
    } else {
        Ok(raw.to_string())
    }
}

/// Lists the caller's tenant's audit evidence records (P4.36, migration
/// 0008), ordered by `(event_time, audit_id)`. READ-ONLY: the trail is
/// append-only — there is no update, delete, or export/dump endpoint
/// anywhere on this resource (§15.2, §16.2); corrections are NEW records
/// referenced by `corrected_audit_id`.
#[utoipa::path(
    get,
    path = "/api/v1/audit-events",
    params(AuditEventListParams),
    responses(
        (status = 200, description = "One page of the tenant's audit evidence records (ordered by event time then record id; every record carries its decision, evidence, and before/after references end to end)", body = AuditEventPageEnvelope),
        (status = 401), (status = 403),
        (status = 422, description = "Malformed filter/cursor or out-of-range limit", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_audit_events(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<AuditEventListParams>,
) -> Result<Json<AuditEventPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AuditEventRead)?;
    let limit = apply_list_limit(params.limit)?;
    let action = match params.action.as_deref() {
        None => None,
        Some(raw) => Some(require_nonempty_filter(raw, "action")?),
    };
    let actor = match params.actor.as_deref() {
        None => None,
        Some(raw) => Some(require_nonempty_filter(raw, "actor")?),
    };
    let object_type = match params.object_type.as_deref() {
        None => None,
        Some(raw) => Some(require_nonempty_filter(raw, "object_type")?),
    };
    let object_id = params.object_id.clone();
    if let (Some(from), Some(to)) = (params.from_event_unix_millis, params.to_event_unix_millis) {
        if from > to {
            return Err(ApiError::validation(
                "'from_event_unix_millis' must not be after 'to_event_unix_millis'".to_string(),
            ));
        }
    }
    let after_audit_id = match params.cursor.as_deref() {
        None => None,
        // The recorded keyset convention (the P4.30 dispatch-command
        // cursor rule): a malformed cursor is a validation rejection —
        // never a silent "first page".
        Some(raw) => Some(
            FlexUuid::parse(raw.trim())
                .map(|_| raw.trim().to_string())
                .map_err(|_| {
                    ApiError::validation(
                        "malformed cursor (an opaque keyset value from the previous page is required)"
                            .to_string(),
                    )
                })?,
        ),
    };
    let ctx = caller.ctx.clone();
    let page = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.audit_events
                .list_audit_events(
                    &ctx,
                    &AuditEventFilter {
                        action,
                        actor,
                        object_type,
                        object_id,
                        from_event_unix_millis: params.from_event_unix_millis,
                        to_event_unix_millis: params.to_event_unix_millis,
                        after_audit_id,
                    },
                    limit as u32,
                )
                .map_err(map_audit_read_error)
        })
        .await?;
    let next_cursor = if page.len() == limit {
        page.last().map(|row| row.audit_id.clone())
    } else {
        None
    };
    Ok(Json(AuditEventPageEnvelope {
        data: page.iter().map(audit_event_dto).collect(),
        next_cursor,
    }))
}

/// Reads one audit evidence record with every §10.1 reference preserved
/// (P5.09's consumption contract). A foreign-tenant record is 404 — no
/// existence leak (§16.2, P0.16 rule 5).
#[utoipa::path(
    get,
    path = "/api/v1/audit-events/{event_id}",
    params(("event_id" = String, Path, description = "The audit record id (canonical UUID)")),
    responses(
        (status = 200, description = "The audit record (actor, action, object, decision + reason reference, evidence reference, before/after references, event/ingest times, correlation id, correction linkage)", body = DataEnvelope<AuditEventDto>),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant record (no existence leak)", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Malformed record id", body = crate::error::ErrorEnvelope),
    )
)]
async fn get_audit_event(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(event_id): Path<String>,
) -> Result<Json<DataEnvelope<AuditEventDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AuditEventRead)?;
    parse_uuid_reference(&event_id)?;
    let ctx = caller.ctx.clone();
    let row = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            uow.audit_events
                .find_audit_event(&ctx, event_id.trim())
                .map_err(map_audit_read_error)?
                .ok_or_else(ApiError::not_found)
        })
        .await?;
    Ok(Json(DataEnvelope {
        data: audit_event_dto(&row),
    }))
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Maps a command-store error onto the API envelope: InvalidRecord is a
/// caller-visible 422, Conflict/TransitionConflict cannot arise from a
/// pure read (still mapped to the opaque 500, never raw storage text),
/// Backend is the opaque 500.
fn map_command_store_error(e: flexcore_storage::command_store::CommandStoreError) -> ApiError {
    use flexcore_storage::command_store::CommandStoreError;
    match e {
        CommandStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the command query was rejected: '{field}' is missing or malformed"
        )),
        CommandStoreError::Backend { .. }
        | CommandStoreError::Conflict { .. }
        | CommandStoreError::TransitionConflict { .. }
        | CommandStoreError::IllegalTransition { .. }
        | CommandStoreError::ScheduleRevisionSuperseded { .. }
        | CommandStoreError::ScheduleRevisionUnknown { .. } => ApiError::infrastructure(),
        // A read error names no command; the NotFound class cannot arise
        // from these read methods (they return Options) — mapped to the
        // opaque class to keep the match total without leaking anything.
        CommandStoreError::NotFound { .. } => ApiError::not_found(),
    }
}

/// Maps a feasibility-check-store error onto the API envelope (the same
/// discipline).
fn map_feasibility_check_error(
    e: flexcore_storage::feasibility_check::FeasibilityCheckStoreError,
) -> ApiError {
    use flexcore_storage::feasibility_check::FeasibilityCheckStoreError;
    match e {
        FeasibilityCheckStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the feasibility-check query was rejected: '{field}' is missing or malformed"
        )),
        FeasibilityCheckStoreError::NotFound { .. } => ApiError::not_found(),
        FeasibilityCheckStoreError::Backend { .. } | FeasibilityCheckStoreError::Conflict { .. } => {
            ApiError::infrastructure()
        }
    }
}

/// Maps an audit-read error onto the API envelope (the same discipline):
/// InvalidFilter is a caller-visible 422, Backend the opaque 500 (never
/// raw storage text). A read error names no record; the read port has no
/// conflict/transition classes at all (SELECT-only — P4.36).
fn map_audit_read_error(e: flexcore_storage::audit_read::AuditEventReadError) -> ApiError {
    use flexcore_storage::audit_read::AuditEventReadError;
    match e {
        AuditEventReadError::InvalidFilter { field } => ApiError::validation(format!(
            "the audit-events query was rejected: '{field}' is missing or malformed"
        )),
        AuditEventReadError::Backend { .. } => ApiError::infrastructure(),
    }
}
