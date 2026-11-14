// P5.31 — The `/api/v1/alarms` READ resource (report §20.1 item 1 "key
//         alarms", §15.2, §16.2; ledger P5.31) on the exact P4.30 route
//         pattern. ADDITIVE — no existing route is modified.
//
// WHAT THIS RESOURCE IS:
//
// Alarms are DERIVED ON READ (the recorded P5.31 decision, DEFAULT — the
// ledger's derived-record contract requires no migration and no
// persistence): every request re-derives the currently derivable alarm
// records from EXISTING platform facts, through the deterministic rules in
// `flexcore-domain::alarm_rules` over the minimal model in
// `flexcore-contracts::alarm`:
//
// - telemetry current states (P2.18): BAD/MISSING → `TELEMETRY_LOSS`,
//   STALE → `TELEMETRY_STALENESS` (only when the telemetry read port is
//   wired — its absence is reported honestly in `sources`, never filled
//   with invented alarms);
// - dispatch-command records (P4.10 store): FAILED → `DISPATCH_FAILURE`,
//   EXPIRED → `DISPATCH_EXPIRY`, FALLBACK → `DISPATCH_FALLBACK`;
// - feasibility-check evidence records (P4.30 store): LIMIT_VIOLATED →
//   `FEASIBILITY_LIMIT_VIOLATED`, FAIL_CLOSED → `FEASIBILITY_FAIL_CLOSED`.
//
// Sources that do not exist get no code and no alarms (market signals —
// no adapter, D1/D2 OPEN; edge/adapter health — P6.17; plugin health — no
// plugin kernel): `sources.absent_sources` names them.
//
// WHAT THIS RESOURCE IS NOT:
//
// - READ-ONLY: there is no POST/PUT/PATCH/DELETE and no acknowledge
//   endpoint — the derived-on-read contract has no alarm mutation path at
//   all (`alarm.write`/`alarm.acknowledge` do not exist on the closed
//   `Action` enum). Alarms are operator-attention records, never
//   protection signals; the model statement travels verbatim in every
//   response and is rendered, never paraphrased, by the console.
// - NOT an alarm history: no persistence exists, so `cleared_at` is never
//   served, `active_only=false` is refused with an honest 422, and the
//   derivation consults BOUNDED pages of each source listing (the page
//   limit; recorded limitation — a full-fleet derivation is later-phase
//   work when a fleet endpoint exists).
// - NOT a new §15.3 event surface: the frozen 26-event vocabulary is not
//   extended (no AlarmRaised/AlarmCleared event exists or is emitted).
//
// Every handler: resolve identity (401) -> authorize `alarm.read`
// (deny-by-default; 403) -> derive inside ONE unit of work (the source
// reads are tenant-scoped, RLS on the Postgres backend) -> map errors to
// the stable envelope. Cross-tenant derivation is impossible by
// construction: every source read is tenant-scoped (§16.2).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use flexcore_authz::action::Action;
use flexcore_contracts::alarm::AlarmRecord;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::alarm_rules::{
    dispatch_alarm_record, feasibility_alarm_record, telemetry_alarm_record,
};
use flexcore_ports::telemetry_read::TelemetryReader;
use flexcore_storage::command_store::{CommandListFilter, CommandRecordStore as _};
use flexcore_storage::feasibility_check::{FeasibilityCheckFilter, FeasibilityCheckStore as _};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::auth::{CallerExtractor, authorize_tenant_scoped};
use crate::dto_alarms::{AlarmPageEnvelope, AlarmSourcesDto, alarm_page_envelope};
use crate::error::ApiError;
use crate::routes::apply_list_limit;
use crate::state::AppState;

/// The qualifying dispatch states (the closed §14.2 vocabulary subset the
/// dispatch rule maps — the store validates the filter value against the
/// frozen vocabulary).
const ALARM_DISPATCH_STATES: [&str; 3] = ["FAILED", "EXPIRED", "FALLBACK"];

/// The closed severity vocabulary (mirror of `flexcore-contracts::alarm::
/// AlarmSeverity`; a filter value outside it is a 422).
const ALARM_SEVERITIES: [&str; 3] = ["INFO", "WARNING", "CRITICAL"];

/// The closed code vocabulary (mirror of `AlarmCode::name()`; a filter
/// value outside it is a 422).
const ALARM_CODES: [&str; 7] = [
    "TELEMETRY_LOSS",
    "TELEMETRY_STALENESS",
    "FEASIBILITY_LIMIT_VIOLATED",
    "FEASIBILITY_FAIL_CLOSED",
    "DISPATCH_FAILURE",
    "DISPATCH_EXPIRY",
    "DISPATCH_FALLBACK",
];

/// The list filters of `GET /api/v1/alarms`. All are optional; every
/// malformed value is a 422 (never a silent "no filter").
#[derive(Debug, Deserialize, ToSchema)]
pub struct AlarmListParams {
    /// Filter by closed severity (`INFO` | `WARNING` | `CRITICAL`).
    #[serde(default)]
    pub severity: Option<String>,
    /// Filter by closed code (e.g. `DISPATCH_FAILURE`).
    #[serde(default)]
    pub code: Option<String>,
    /// Filter to one asset (canonical UUID; a malformed value is 422).
    #[serde(default)]
    pub asset_id: Option<String>,
    /// Filter to one site (canonical UUID; a malformed value is 422).
    #[serde(default)]
    pub site_id: Option<String>,
    /// Inclusive lower bound on `raised_at` (unix millis).
    #[serde(default)]
    pub from_event_unix_millis: Option<i64>,
    /// Inclusive upper bound on `raised_at` (unix millis).
    #[serde(default)]
    pub to_event_unix_millis: Option<i64>,
    /// Only active alarms. Defaults to `true`; an explicit `false` is a 422:
    /// the derived-on-read contract has no persistence, so no cleared or
    /// acknowledged alarm can ever be served — the refusal says so instead
    /// of silently returning the active set.
    #[serde(default)]
    pub active_only: Option<bool>,
    /// Page bound per consulted source listing (default 200, max 1000; the
    /// recorded P2.12/P3.28/P4.30 convention).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Builds the alarms router (merged into the stage-1 router). GET only —
/// the derivation is a READ surface with no write path of any kind.
pub fn alarms_router() -> Router<AppState> {
    Router::new().route("/api/v1/alarms", get(list_alarms))
}

/// The derivation outcome of one request (before the response filters).
struct Derivation {
    derived_at: EventTime,
    records: Vec<AlarmRecord>,
    dispatch_consulted: usize,
    feasibility_consulted: usize,
    telemetry_consulted: usize,
    skipped_defective: usize,
    telemetry_available: bool,
}

/// Derives and serves the caller's tenant's currently derivable alarms
/// (READ-only; §20.1 item 1).
#[utoipa::path(
    get,
    path = "/api/v1/alarms",
    params(AlarmListParams),
    responses(
        (status = 200, description = "The currently derivable alarm records for the caller's tenant, with the consulted/absent source inventory, the derivation instant, and the alarm-model statement verbatim (derived on read — a single bounded page, no cursor)", body = AlarmPageEnvelope),
        (status = 401, description = "Missing or invalid authentication"),
        (status = 403, description = "Denied by policy"),
        (status = 422, description = "Malformed filter value, inverted time window, or active_only=false (no persistence exists to serve cleared alarms)", body = crate::error::ErrorEnvelope),
    )
)]
async fn list_alarms(
    State(state): State<AppState>,
    CallerExtractor(caller): CallerExtractor,
    Query(params): Query<AlarmListParams>,
) -> Result<Json<AlarmPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AlarmRead)?;
    let limit = apply_list_limit(params.limit)?;

    // The severity/code filters must be IN the closed vocabularies (a
    // near-miss is a 422, never a silent match-all).
    let severity_filter = match params.severity.as_deref() {
        None => None,
        Some(raw) => {
            if !ALARM_SEVERITIES.contains(&raw) {
                return Err(ApiError::validation(format!(
                    "'severity' is not a valid alarm severity: '{raw}'"
                )));
            }
            Some(raw.to_string())
        }
    };
    let code_filter = match params.code.as_deref() {
        None => None,
        Some(raw) => {
            if !ALARM_CODES.contains(&raw) {
                return Err(ApiError::validation(format!(
                    "'code' is not a valid alarm code: '{raw}'"
                )));
            }
            Some(raw.to_string())
        }
    };
    let asset_filter = match params.asset_id.as_deref() {
        None => None,
        Some(raw) => Some(
            FlexUuid::parse(raw)
                .map_err(|_| {
                    ApiError::validation(format!("'asset_id' is not a valid UUID: '{raw}'"))
                })?
                .to_hyphenated()
                .to_string(),
        ),
    };
    let site_filter = match params.site_id.as_deref() {
        None => None,
        Some(raw) => Some(
            FlexUuid::parse(raw)
                .map_err(|_| {
                    ApiError::validation(format!("'site_id' is not a valid UUID: '{raw}'"))
                })?
                .to_hyphenated()
                .to_string(),
        ),
    };
    if let (Some(from), Some(to)) = (params.from_event_unix_millis, params.to_event_unix_millis)
    {
        if from > to {
            return Err(ApiError::validation(
                "'from_event_unix_millis' must not be after 'to_event_unix_millis'",
            ));
        }
    }
    // active_only: the honest refusal, by contract.
    if params.active_only == Some(false) {
        return Err(ApiError::validation(
            "active_only=false cannot be served: alarms are derived on read from current \
             platform facts (no alarm persistence exists in this phase), so only ACTIVE \
             alarms are ever derivable — there is no alarm history to query"
                .to_string(),
        ));
    }

    let telemetry: Option<Arc<dyn TelemetryReader>> = state.telemetry.clone();
    let ctx = caller.ctx.clone();
    let tenant = caller.ctx.tenant_id().uuid();
    let derivation = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let now = uow.clock.now_event_time();
            let mut records: Vec<AlarmRecord> = Vec::new();
            let mut dispatch_consulted = 0usize;
            let mut feasibility_consulted = 0usize;
            let mut telemetry_consulted = 0usize;
            let mut skipped_defective = 0usize;

            // --- Dispatch-outcome alarms (P4.10 store; §14.2 states) ------
            for state_name in ALARM_DISPATCH_STATES {
                let page = uow
                    .command_records
                    .list_commands(
                        &ctx,
                        &CommandListFilter {
                            state: Some(state_name.to_string()),
                            after_command_id: None,
                        },
                        limit as u32,
                    )
                    .map_err(map_command_store_error)?;
                dispatch_consulted += page.len();
                for record in page {
                    // raised_at = the event time of the transition INTO the
                    // qualifying state (the journal is the source of that
                    // fact). A record whose journal lacks the transition is
                    // DEFECTIVE: skipped AND counted, never derived with an
                    // invented timestamp.
                    let journal = uow
                        .command_records
                        .transition_journal(&ctx, &record.row.command_id)
                        .map_err(map_command_store_error)?;
                    let Some(outcome) = journal.iter().rev().find(|t| t.to_state == record.state)
                    else {
                        skipped_defective += 1;
                        continue;
                    };
                    let Ok(raised) = EventTime::from_unix_millis(outcome.at_unix_millis) else {
                        skipped_defective += 1;
                        continue;
                    };
                    if let Some(alarm) = dispatch_alarm_record(
                        tenant,
                        FlexUuid::parse(&record.row.site_id).ok(),
                        FlexUuid::parse(&record.row.asset_id).ok(),
                        &record.row.command_id,
                        state_name,
                        raised,
                        outcome.evidence_reference.as_deref(),
                        now,
                    ) {
                        records.push(alarm);
                    }
                }
            }

            // --- Feasibility-verdict alarms (P4.30 evidence store) --------
            let checks = uow
                .feasibility_checks
                .list_feasibility_checks(&ctx, &FeasibilityCheckFilter::default(), limit as u32)
                .map_err(map_feasibility_check_error)?;
            feasibility_consulted = checks.len();
            for row in checks {
                let Ok(checked_at) = EventTime::from_unix_millis(row.checked_at_unix_millis)
                else {
                    skipped_defective += 1;
                    continue;
                };
                if let Some(alarm) = feasibility_alarm_record(
                    tenant,
                    FlexUuid::parse(&row.site_id).ok(),
                    FlexUuid::parse(&row.asset_id).ok(),
                    &row.check_id,
                    &row.outcome,
                    checked_at,
                    now,
                ) {
                    records.push(alarm);
                }
            }

            // --- Telemetry-quality alarms (P2.18 current states) ----------
            let assets = uow
                .assets
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?;
            let assets: Vec<_> = assets.into_iter().take(limit).collect();
            let telemetry_available = telemetry.is_some();
            if let Some(reader) = telemetry.as_ref() {
                telemetry_consulted = assets.len();
                for asset in assets {
                    // The telemetry read port is deliberately NOT
                    // transaction-bound (state.rs); its calls stay
                    // tenant-scoped by the port contract (P0.16 rule 1).
                    let latest = reader
                        .latest_state(&ctx, asset.asset_id())
                        .map_err(map_telemetry_read_error)?;
                    for observation in latest {
                        if let Some(alarm) = telemetry_alarm_record(
                            tenant,
                            Some(asset.site_id().uuid()),
                            asset.asset_id().uuid(),
                            flexcore_ports::telemetry_read::quantity_key(observation.quantity()),
                            observation.quality(),
                            observation.event_time(),
                            now,
                        ) {
                            records.push(alarm);
                        }
                    }
                }
            }

            Ok(Derivation {
                derived_at: now,
                records,
                dispatch_consulted,
                feasibility_consulted,
                telemetry_consulted,
                skipped_defective,
                telemetry_available,
            })
        })
        .await?;

    // --- Response filters (post-derivation; all bounded and validated) -----
    let mut records = derivation.records;
    records.retain(|alarm| {
        let alarm_asset = alarm.asset_id.map(|id| id.to_hyphenated().to_string());
        let alarm_site = alarm.site_id.map(|id| id.to_hyphenated().to_string());
        severity_filter
            .as_deref()
            .map_or(true, |s| s == alarm.severity.name())
            && code_filter
                .as_deref()
                .map_or(true, |c| c == alarm.code.name())
            && asset_filter
                .as_deref()
                .map_or(true, |a| alarm_asset.as_deref() == Some(a))
            && site_filter
                .as_deref()
                .map_or(true, |s| alarm_site.as_deref() == Some(s))
            && params
                .from_event_unix_millis
                .map_or(true, |from| alarm.raised_at.unix_millis() >= i128::from(from))
            && params
                .to_event_unix_millis
                .map_or(true, |to| alarm.raised_at.unix_millis() <= i128::from(to))
    });

    let absent_sources = vec![
        "market signals and market adapters: no market integration exists in 0.x (D1/D2 OPEN) \
         — no signal can be lost and no market alarm can be derived"
            .to_string(),
        "edge/adapter health: P6.17 (Stage 6) — the first site integration is v0.4.0; no edge \
         adapter exists to be healthy or unhealthy"
            .to_string(),
        "plugin health: no plugin kernel exists (the plugin SDK is Stage 7, v0.5.0) — no plugin \
         can fail"
            .to_string(),
    ];
    let sources = AlarmSourcesDto {
        telemetry_current_state: if derivation.telemetry_available {
            "available".to_string()
        } else {
            "unavailable: the telemetry read port is not wired in this deployment (the \
             recorded P2.06-adjacent gap) — NO telemetry-derived alarms are served and none \
             are invented in their place"
                .to_string()
        },
        telemetry_assets_consulted: derivation.telemetry_consulted,
        dispatch_commands_consulted: derivation.dispatch_consulted,
        feasibility_checks_consulted: derivation.feasibility_consulted,
        skipped_defective_source_records: derivation.skipped_defective,
        absent_sources,
    };
    Ok(Json(alarm_page_envelope(records, sources, derivation.derived_at)))
}

fn map_command_store_error(e: flexcore_storage::command_store::CommandStoreError) -> ApiError {
    use flexcore_storage::command_store::CommandStoreError;
    match e {
        CommandStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the alarm derivation's command query was rejected: '{field}' is missing or malformed"
        )),
        CommandStoreError::NotFound { .. } => ApiError::not_found(),
        _ => ApiError::infrastructure(),
    }
}

fn map_feasibility_check_error(
    e: flexcore_storage::feasibility_check::FeasibilityCheckStoreError,
) -> ApiError {
    use flexcore_storage::feasibility_check::FeasibilityCheckStoreError;
    match e {
        FeasibilityCheckStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the alarm derivation's feasibility-check query was rejected: '{field}' is missing or malformed"
        )),
        _ => ApiError::infrastructure(),
    }
}

fn map_telemetry_read_error(e: flexcore_ports::telemetry_read::TelemetryReadError) -> ApiError {
    let _ = e; // never forwarded to the client
    ApiError::infrastructure()
}
