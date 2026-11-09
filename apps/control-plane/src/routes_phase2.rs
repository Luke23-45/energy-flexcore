// P2.12 / P2.17 / P2.18 — Phase 2 API routes: registry CRUD completion
// (DELETE with the P2.02 RESTRICT rule, asset metadata update, asset
// deactivation), the telemetry query / current-state / quarantine read
// surface, and the P2.17 registry views.
//
// Every handler follows the P1.17 pattern: resolve identity (401) ->
// authorize with an explicit action + scope (403) -> run inside ONE unit of
// work (`state.run_in_uow`) -> map errors to the stable envelope
// (404 for cross-tenant references, no existence leak; 422 validation;
// 409 for the RESTRICT deletion rule; 500 opaque infrastructure).
//
// RECORDED DECISIONS (this module is the decision record for the P2.12
// contract points the ledger asked to decide once):
//
// - **Cursor pagination**: keyset over the record sort key
//   `(event_time, asset_id, quantity)` — see the `telemetry_read` port
//   header. Opaque versioned cursor; unparsable cursor is 422. No offset
//   pagination anywhere.
// - **Limits**: maximum query range 7 days
//   ([`MAX_QUERY_RANGE_MILLIS`]), maximum page size
//   [`MAX_PAGE_SIZE`] = 1000, default page size
//   [`DEFAULT_PAGE_SIZE`] = 200. Oversized range/page -> 422 (the
//   P2.12 named risk: an unbounded telemetry query is a full-table scan
//   via the API).
// - **RESTRICT denial status**: 409 Conflict, stable code
//   `non_empty_parent` — the request is well-formed; the conflict is with
//   the resource's dependents. 422 would misreport it as a validation
//   problem with the request itself.
// - **Deletion events**: the frozen §15.3 vocabulary defines no delete
//   events (P0.14: none may be invented), so an accepted deletion pairs
//   its `Delete` state change with `OperatorActionRecorded` — the same
//   recorded assumption as the Phase 1 tenant/portfolio/site creations.
// - **Audit composition**: the direct `AuditSink` append happens at the
//   composition root, where the unit of work that implements the sink is
//   open. In the Postgres backend that is `PgUnitOfWork` (P1.13); the
//   [`UnitOfWork`](crate::state::UnitOfWork) bundle does not expose the
//   sink yet (the P2.03 recorded gap), so the audit evidence shipped here
//   is the state-change record (journal `StateChange` op) plus the outbox
//   event below — both preserved by construction, never cascaded.
// - **Simulated marking** (P2.17 step 3): `simulated: true` on every
//   value whose source id is NOT in the reserved platform namespace — in
//   Phase 2 the provider set is exactly one (the simulator), so every
//   measured-source value is simulator-derived and is marked as such.
//   Platform-derived records (`flexcore:derived:` gap markers) carry
//   `simulated: false` — they are not simulator values.
// - **Telemetry read wiring**: the `TelemetryReader` port is wired in the
//   Test backend only; the Postgres implementation is the recorded
//   P2.06-adjacent gap, and the telemetry routes fail closed (500,
//   opaque) until it lands.
// - **Historical view (P2.19, decision D-H1..D-H6 in
//   `ledger/phase-02-asset-telemetry/19_20_historical_retention.md`)**:
//   `GET /api/v1/telemetry/history` serves the historical query through the
//   PostgreSQL adapter's history methods INSIDE one unit-of-work
//   transaction per request (RLS applies; cross-tenant/unknown asset →
//   404). Pagination is a keyset cursor over the P2.05 PK's varying legs
//   `(event_time, source_sequence, quantity)` (D-H1); aggregation is SQL
//   `date_bin` over the closed width set `1m|5m|15m|1h|1d` with NO rounding
//   anywhere (D-H2, P0.07); the limits reuse the P2.12 range/page maxima
//   and add `MAX_HISTORY_BUCKETS` (D-H3). The Test backend has no SQL
//   engine (no `date_bin`, no keyset SQL) and fails closed — its coverage
//   is the DB-gated suite `tests/telemetry_history.rs` plus the pure
//   validator/cursor unit tests there.
// - **Current-state freshness limit** (P2.18 step 3, recorded as
//   CONFIGURATION): the current-state use-case limit comes from
//   `FLEXCORE_CURRENT_STATE_FRESHNESS_MS` (default 300 000 ms) via the
//   app state — never a hard-coded constant (P2.08 step 3; the historical
//   view defines its own limit separately). The staleness rule is the
//   P2.08 pure rule (`flexcore_domain::quality_rules::staleness`):
//   `age > limit` → the entry is presented with the STALE flag, the last
//   known value and its timestamps preserved — never shown as GOOD
//   (P2.18 step 1). Recorded decisions on the flag's application:
//   value-bearing records only (a MISSING gap marker has no last known
//   value to grow old — it stays MISSING); the STORED quality of a fresh
//   record is preserved exactly (SUSPECT shows SUSPECT, ESTIMATED shows
//   ESTIMATED — quality is never re-labelled or upgraded for the view).
// - **As-of queries (P2.31, decisions D-C4/D-C5 in
//   `ledger/phase-02-asset-telemetry/25_31_exit_portfolio_corrections.md`)**:
//   the P2.12 `/telemetry/query` and P2.19 `/telemetry/history` routes
//   accept an OPTIONAL `as_of` parameter (RFC 3339 UTC; malformed → 422,
//   never a silent "latest"). With `as_of`, each (asset, event_time,
//   quantity) identity returns the latest version whose correction
//   timestamp is <= the as-of instant (the original version is always
//   visible; the semantics are D-C4); without it, the latest version. The
//   as-of read is served by the Postgres adapter's as-of methods inside the
//   same unit-of-work transaction (`run_history_in_uow`), so it carries RLS
//   exactly like the historical view; bucketed aggregation with `as_of` is
//   422 (record-level versions have no bucket semantics in Phase 2 —
//   recorded in D-C5). The Test backend has no versioned storage and fails
//   closed for `as_of` (infrastructure error), consistent with the recorded
//   telemetry-port wiring status.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use flexcore_application::deactivate_asset::DeactivateAssetCommand;
use flexcore_application::payload_encoding::EventPayloadEncoder as _;
use flexcore_application::update_asset_metadata::UpdateAssetMetadataCommand;
use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention};
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{DomainEvent, OperatorActionRecordedPayload, event_name};
use flexcore_events::subject::subject_for;
use flexcore_ports::event_outbox::{EventOutbox as _, OutboxWrite};
use flexcore_ports::repository::{
    AssetRepository as _, PortfolioRepository as _, SiteRepository as _, TenantRepository as _,
};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt, entities};
use flexcore_ports::telemetry_read::{
    TelemetryCursor, TelemetryReadError, TelemetryReadQuery, TelemetryReader as _,
    parse_quantity_key, quantity_key, quantity_unit,
};
use flexcore_ports::telemetry_source::{NormalizedTelemetry, QuantityValue, TelemetryQuantity};
use flexcore_postgres_adapter::telemetry::{HistoryBucketWidth, HistoryCursor, StoredTelemetry};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::authorize_tenant_scoped;
use crate::dto::{
    AssetLimitsDto, AssetViewDto, CommitmentsPlaceholderDto, CurrentStateDto, DataEnvelope,
    EdgeConnectivityDto, PortfolioCompositionDto, PortfolioViewDto, QuarantineEntryDto,
    QuantityStateDto, SiteOverviewDto, SiteViewDto, TelemetryPageEnvelope, TelemetryRecordDto,
    parse_quality, rfc3339,
};
use crate::error::ApiError;
use crate::routes::{map_asset, parse_asset_id, parse_portfolio_id, parse_site_id};
use crate::state::AppState;

/// Recorded P2.12 limit: maximum telemetry query range = 7 days.
pub const MAX_QUERY_RANGE_MILLIS: i128 = 7 * 24 * 60 * 60 * 1000;
/// Recorded P2.12 limit: maximum telemetry page size.
pub const MAX_PAGE_SIZE: u32 = 1000;
/// Recorded P2.12 limit: default telemetry page size.
pub const DEFAULT_PAGE_SIZE: u32 = 200;
/// Recorded P2.19 limit (D-H3): maximum buckets per aggregation request.
/// Configuration in the recorded sense — one named, documented value, never
/// hard-coded inline at call sites; the full decision record is
/// `ledger/phase-02-asset-telemetry/19_20_historical_retention.md` §D-H3.
pub const MAX_HISTORY_BUCKETS: u64 = 1000;

/// Builds the Phase 2 router (merged into the stage-1 router; still a
/// subset of the P0.15 resource list — these are sub-operations of the
/// tenants/portfolios/sites/assets resources and the telemetry/quarantine
/// read surface of P2.12).
pub fn phase2_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tenants/{tenant_id}", delete(delete_tenant))
        .route("/api/v1/portfolios/{portfolio_id}", delete(delete_portfolio))
        .route("/api/v1/sites/{site_id}", delete(delete_site))
        .route("/api/v1/assets/{asset_id}", delete(delete_asset))
        .route("/api/v1/assets/{asset_id}", axum::routing::patch(update_asset))
        .route(
            "/api/v1/assets/{asset_id}/deactivate",
            post(deactivate_asset),
        )
        .route("/api/v1/telemetry/query", get(query_telemetry))
        .route("/api/v1/telemetry/history", get(query_telemetry_history))
        .route("/api/v1/telemetry/quarantine", get(list_quarantine))
        .route("/api/v1/assets/{asset_id}/state", get(get_asset_state))
        .route("/api/v1/portfolios/{portfolio_id}/view", get(portfolio_view))
        .route("/api/v1/sites/{site_id}/view", get(site_view))
        .route("/api/v1/assets/{asset_id}/view", get(asset_view))
}

// ---------------------------------------------------------------------------
// Outbox assembly (composition-root spelling of report §6.3 step 1)
// ---------------------------------------------------------------------------

/// Renders a PascalCase frozen event name as lower snake_case for the
/// idempotency key (the same helper the application crate carries; three
/// lines, duplicated to keep the application crate's assembly private).
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Assembles and appends the one transactional outbox write for a state
/// change paired with `OperatorActionRecorded` (the §15.3 audit-area event
/// used for registry operations that have no dedicated frozen event —
/// deletion is one, P0.14). The application crate's `emit` is private, so
/// the composition root carries the same assembly: deterministic key
/// (snake_case frozen event name + correlation id), P1.09 envelope,
/// conservative default subject, one `OutboxWrite`.
///
/// See the module header for the audit composition note (AuditSink gap).
fn emit_operator_action(
    receipt: StateChangeReceipt,
    correlation_id: FlexUuid,
    event_time: EventTime,
    tenant_id: flexcore_domain::ids::TenantId,
    outbox: &dyn EventOutbox,
) -> Result<(), ApiError> {
    let event = DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
        correlation_id,
        tenant_id,
        event_time,
    });
    let key = format!("{}:{}", snake_case(event_name(&event)), correlation_id);
    let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &key, event)
        .map_err(|_| ApiError::infrastructure())?;
    let payload = crate::encoding::JsonEventPayloadEncoder
        .encode(&envelope)
        .map_err(|_| ApiError::infrastructure())?;
    let subject = subject_for(&envelope.event).to_string();
    let write =
        OutboxWrite::new(receipt, key, subject, payload, event_time)
            .map_err(|_| ApiError::infrastructure())?;
    outbox.append(write).map_err(|_| ApiError::infrastructure())
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// The asset metadata update body (P2.03 `UpdateAssetMetadata`): vendor /
/// model, rated power/energy, capability-profile reference. Identity,
/// ownership chain, and lifecycle are not touchable through this route.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAssetBody {
    pub vendor_model: String,
    /// Rated active power, canonical MW (P0.07).
    pub rated_power_mw: f64,
    /// Rated energy, canonical MWh, when applicable (storage assets).
    #[serde(default)]
    pub rated_energy_mwh: Option<f64>,
    /// The capability-profile reference (contents are Phase 3, P0.10 §5).
    pub capability_profile: String,
}

/// The asset deactivation body (P2.03 `DeactivateAsset`): the reason is
/// mandatory and is persisted on the asset record.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeactivateAssetBody {
    pub reason: String,
}

/// The P2.12 telemetry query parameters. `asset` and the `from`/`until`
/// time range are REQUIRED (422 when absent); `quantities` is a
/// comma-separated list of canonical quantity ids (absent = all frozen
/// quantities); `quality` accepts ONLY the six canonical states (a seventh
/// value is 422, P2.12 step 3); `limit` is bounded by [`MAX_PAGE_SIZE`];
/// `cursor` is the opaque keyset cursor from the previous page.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TelemetryQueryParams {
    /// The asset to query (required; a foreign-tenant asset is 404).
    #[serde(default)]
    pub asset: Option<String>,
    /// Inclusive range start over `event_time`, RFC 3339 (required).
    #[serde(default)]
    pub from: Option<String>,
    /// Inclusive range end over `event_time`, RFC 3339 (required).
    #[serde(default)]
    pub until: Option<String>,
    /// Comma-separated canonical quantity ids (absent = all).
    #[serde(default)]
    pub quantities: Option<String>,
    /// One canonical quality state (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE).
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    /// The opaque keyset cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
    /// P2.31: the optional as-of instant, RFC 3339 UTC. With it, each
    /// (asset, event_time, quantity) returns the latest version whose
    /// correction timestamp is <= the instant; absent = the latest version.
    #[serde(default)]
    pub as_of: Option<String>,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// The Phase 2 provenance rule (module header "Simulated marking").
fn is_simulated(source_id: &str) -> bool {
    !source_id.starts_with("flexcore:")
}

/// Renders one record's value as the `(num, bool, text)` triple — exactly
/// one leg present, none for a MISSING gap marker (P2.04 §3 totality).
fn value_fields(
    value: Option<&QuantityValue>,
) -> (Option<f64>, Option<bool>, Option<String>) {
    match value {
        None => (None, None, None),
        Some(QuantityValue::Power(p)) => (Some(p.value()), None, None),
        Some(QuantityValue::Reactive { power, .. }) => (Some(power.value()), None, None),
        Some(QuantityValue::Fraction(f)) => (Some(f.value()), None, None),
        Some(QuantityValue::Energy(e)) => (Some(e.value()), None, None),
        Some(QuantityValue::Flag(b)) => (None, Some(*b), None),
        Some(QuantityValue::State(s)) => (None, None, Some(s.clone())),
    }
}

/// The declared reactive-sign convention of a record, set exactly when the
/// record carries a reactive-power value (P0.07 §3: never inferred from the
/// sign, always declared).
fn sign_convention(value: Option<&QuantityValue>) -> Option<String> {
    match value {
        Some(QuantityValue::Reactive { convention, .. }) => Some(
            match convention {
                ReactiveSignConvention::InjectionPositive => "INJECTION_POSITIVE",
                ReactiveSignConvention::ConsumptionPositive => "CONSUMPTION_POSITIVE",
            }
            .to_string(),
        ),
        _ => None,
    }
}

/// The value's age in milliseconds (`now - event_time`, floored at 0).
fn age_ms(now: EventTime, event_time: EventTime) -> i64 {
    let delta = (now.timestamp() - event_time.timestamp()).whole_milliseconds();
    delta.clamp(0, i64::MAX as i128) as i64
}

/// Maps one canonical record onto the query-response DTO.
fn map_telemetry_record(r: &NormalizedTelemetry) -> TelemetryRecordDto {
    let (value_num, value_bool, value_text) = value_fields(r.value());
    TelemetryRecordDto {
        asset_id: r.asset_id().uuid().to_hyphenated().to_string(),
        quantity: quantity_key(r.quantity()).to_string(),
        value_num,
        value_bool,
        value_text,
        unit: quantity_unit(r.quantity()).to_string(),
        quality: crate::dto::quality_id(r.quality()).to_string(),
        q_sign_convention: sign_convention(r.value()),
        event_time: rfc3339(r.event_time()),
        ingest_time: rfc3339_ingest(r.ingest_time()),
        source_id: r.source_id().to_string(),
        source_sequence: r.source_sequence(),
        simulated: is_simulated(r.source_id()),
    }
}

/// RFC 3339 rendering of an ingest timestamp (the ingest type is distinct
/// from the event type, P0.09; the rendering is the same wire format).
/// Maps one stored (version-selected) record onto the P2.12 query-response
/// DTO — the as-of branch's record shape (P2.31). The mapping is the same
/// field set as [`map_telemetry_record`]; only the source type differs
/// (the adapter's `StoredTelemetry` instead of the port's canonical record).
fn map_stored_record(r: &StoredTelemetry) -> Result<TelemetryRecordDto, ApiError> {
    let (value_num, value_bool, value_text) = match &r.value {
        None => (None, None, None),
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Numeric(n)) => {
            (Some(*n), None, None)
        }
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Flag(b)) => {
            (None, Some(*b), None)
        }
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Text(t)) => {
            (None, None, Some(t.clone()))
        }
    };
    Ok(TelemetryRecordDto {
        asset_id: r.asset_id.uuid().to_hyphenated().to_string(),
        quantity: r.quantity.db_text().to_string(),
        value_num,
        value_bool,
        value_text,
        unit: r.quantity.canonical_unit().to_string(),
        quality: crate::dto::quality_id(r.quality).to_string(),
        q_sign_convention: r
            .q_sign_convention
            .map(|c| match c {
                ReactiveSignConvention::InjectionPositive => "INJECTION_POSITIVE",
                ReactiveSignConvention::ConsumptionPositive => "CONSUMPTION_POSITIVE",
            })
            .map(str::to_string),
        event_time: rfc3339(r.event_time),
        ingest_time: rfc3339_ingest(r.ingest_time),
        source_id: r.source_id.clone(),
        source_sequence: u64::try_from(r.source_sequence)
            .map_err(|_| ApiError::infrastructure())?,
        simulated: is_simulated(&r.source_id),
    })
}

/// RFC 3339 rendering of an ingest timestamp (the ingest type is distinct
/// from the event type, P0.09; the rendering is the same wire format).
fn rfc3339_ingest(t: flexcore_contracts::time_model::IngestTime) -> String {
    t.timestamp()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<invalid-timestamp>".to_string())
}

/// Maps one canonical record onto the P2.18 current-state quantity entry,
/// applying the current-state freshness limit (P2.18 step 1, the P2.08
/// staleness rule): a value-bearing record whose event-time age EXCEEDS the
/// limit is presented with the STALE flag — the last known value, its
/// timestamps, and its age are preserved, never shown as GOOD. A fresh
/// record keeps its stored quality exactly (no re-labelling, no upgrade).
/// A MISSING gap marker carries no value and stays MISSING (nothing to
/// grow old). `age == limit` is NOT stale (the frozen P2.08 boundary).
fn map_quantity_state(
    r: &NormalizedTelemetry,
    now: EventTime,
    freshness_millis: i64,
) -> QuantityStateDto {
    let (value_num, value_bool, value_text) = value_fields(r.value());
    let mut quality = r.quality();
    if r.value().is_some() {
        let limit = flexcore_domain::quality_rules::FreshnessLimit::try_new(freshness_millis)
            .expect("the current-state freshness limit is validated positive at composition");
        if let Some(stale) =
            flexcore_domain::quality_rules::staleness(r.event_time(), now, limit).stale_quality()
        {
            quality = stale;
        }
    }
    QuantityStateDto {
        quantity: quantity_key(r.quantity()).to_string(),
        value_num,
        value_bool,
        value_text,
        unit: quantity_unit(r.quantity()).to_string(),
        quality: crate::dto::quality_id(quality).to_string(),
        q_sign_convention: sign_convention(r.value()),
        event_time: rfc3339(r.event_time()),
        ingest_time: rfc3339_ingest(r.ingest_time()),
        age_ms: age_ms(now, r.event_time()),
        simulated: is_simulated(r.source_id()),
    }
}

/// The reader handle or the recorded fail-closed behaviour: the Postgres
/// composition root wires `None` until the P2.06-adjacent adapter work
/// lands; the telemetry routes then return the opaque infrastructure
/// error rather than an empty (lying) result.
fn telemetry_reader(state: &AppState) -> Result<std::sync::Arc<dyn TelemetryReader>, ApiError> {
    state
        .telemetry
        .clone()
        .ok_or_else(ApiError::infrastructure)
}

/// Maps a telemetry-read error onto the API envelope. Every variant is
/// either a caller-visible validation (already enforced at the API layer)
/// or an opaque backend failure — never raw storage text.
fn map_read_error(e: TelemetryReadError) -> ApiError {
    let _ = e; // never forwarded to the client
    ApiError::infrastructure()
}

// ---------------------------------------------------------------------------
// DELETE routes — the P2.02 RESTRICT rule (decided once, cited here)
// ---------------------------------------------------------------------------

/// Deletes a tenant. The P2.02 RESTRICT rule applies: a tenant that still
/// owns portfolios cannot be deleted (409). A tenant other than the
/// caller's own is 404 (no existence leak). Audit trail preserved: the
/// accepted deletion is a `Delete` state change paired with one outbox
/// event (module header).
#[utoipa::path(
    delete,
    path = "/api/v1/tenants/{tenant_id}",
    params(("tenant_id" = String, Path, description = "Tenant UUID")),
    responses(
        (status = 204, description = "Tenant deleted"),
        (status = 401), (status = 403),
        (status = 404, description = "Unknown or cross-tenant (no existence leak)"),
        (status = 409, description = "RESTRICT rule: the tenant still owns portfolios", body = crate::error::ErrorEnvelope),
    )
)]
async fn delete_tenant(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TenantDelete)?;
    let id = crate::routes::parse_tenant_id(&tenant_id).ok_or_else(ApiError::not_found)?;
    if id != caller.ctx.tenant_id() {
        return Err(ApiError::not_found());
    }
    let ctx = caller.ctx.clone();
    state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            if uow
                .tenants
                .find_by_id(id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            // P2.02 RESTRICT: the guard is pure; the caller supplies the
            // child count through the tenant-scoped port (the domain stays
            // I/O-free — one spelling of the rule, cited by the API tests).
            let portfolio_count = uow
                .portfolios
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .len();
            flexcore_domain::registry_rules::check_tenant_deletion(portfolio_count)
                .map_err(|e| ApiError::conflict("non_empty_parent", e.to_string()))?;
            let deleted = uow.tenants.delete(id).map_err(ApiError::from)?;
            if !deleted {
                return Err(ApiError::not_found());
            }
            let receipt = StateChangeReceipt::new(
                entities::TENANTS.to_string(),
                StateChangeOperation::Delete,
                None, // the tenant registry root is tenant-less by contract
            );
            let correlation_id: FlexUuid = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            emit_operator_action(
                receipt,
                correlation_id,
                event_time,
                id,
                uow.outbox.as_ref(),
            )
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a portfolio. RESTRICT: a portfolio that still owns sites cannot
/// be deleted (409). A cross-tenant portfolio reads as missing (404).
#[utoipa::path(
    delete,
    path = "/api/v1/portfolios/{portfolio_id}",
    params(("portfolio_id" = String, Path, description = "Portfolio UUID")),
    responses(
        (status = 204, description = "Portfolio deleted"),
        (status = 401), (status = 403), (status = 404),
        (status = 409, description = "RESTRICT rule: the portfolio still owns sites", body = crate::error::ErrorEnvelope),
    )
)]
async fn delete_portfolio(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(portfolio_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::PortfolioDelete)?;
    let id = parse_portfolio_id(&portfolio_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            if uow
                .portfolios
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .is_none()
            {
                // Unknown or cross-tenant: 404 either way (no leak).
                return Err(ApiError::not_found());
            }
            let site_count = uow
                .sites
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .iter()
                .filter(|s| s.portfolio_id() == id)
                .count();
            flexcore_domain::registry_rules::check_portfolio_deletion(site_count)
                .map_err(|e| ApiError::conflict("non_empty_parent", e.to_string()))?;
            let deleted = uow.portfolios.delete(&ctx, id).map_err(ApiError::from)?;
            if !deleted {
                return Err(ApiError::not_found());
            }
            let correlation_id: FlexUuid = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            emit_operator_action(
                StateChangeReceipt::new(
                    entities::PORTFOLIOS.to_string(),
                    StateChangeOperation::Delete,
                    Some(ctx.tenant_id()),
                ),
                correlation_id,
                event_time,
                ctx.tenant_id(),
                uow.outbox.as_ref(),
            )
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a site. RESTRICT: a site that still owns assets cannot be
/// deleted (409). A cross-tenant site reads as missing (404).
#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site UUID")),
    responses(
        (status = 204, description = "Site deleted"),
        (status = 401), (status = 403), (status = 404),
        (status = 409, description = "RESTRICT rule: the site still owns assets", body = crate::error::ErrorEnvelope),
    )
)]
async fn delete_site(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(site_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::SiteDelete)?;
    let id = parse_site_id(&site_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            if uow
                .sites
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let asset_count = uow
                .assets
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .iter()
                .filter(|a| a.site_id() == id)
                .count();
            flexcore_domain::registry_rules::check_site_deletion(asset_count)
                .map_err(|e| ApiError::conflict("non_empty_parent", e.to_string()))?;
            let deleted = uow.sites.delete(&ctx, id).map_err(ApiError::from)?;
            if !deleted {
                return Err(ApiError::not_found());
            }
            let correlation_id: FlexUuid = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            emit_operator_action(
                StateChangeReceipt::new(
                    entities::SITES.to_string(),
                    StateChangeOperation::Delete,
                    Some(ctx.tenant_id()),
                ),
                correlation_id,
                event_time,
                ctx.tenant_id(),
                uow.outbox.as_ref(),
            )
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes an asset (a registry leaf: no dependents to restrict on; the
/// stored telemetry history is NOT deleted — it outlives registry rows by
/// design). A cross-tenant asset reads as missing (404).
#[utoipa::path(
    delete,
    path = "/api/v1/assets/{asset_id}",
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 204, description = "Asset deleted (telemetry history preserved)"),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn delete_asset(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetDelete)?;
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            if uow
                .assets
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let deleted = uow.assets.delete(&ctx, id).map_err(ApiError::from)?;
            if !deleted {
                return Err(ApiError::not_found());
            }
            let correlation_id: FlexUuid = uow.ids.new_uuid();
            let event_time = uow.clock.now_event_time();
            emit_operator_action(
                StateChangeReceipt::new(
                    entities::ASSETS.to_string(),
                    StateChangeOperation::Delete,
                    Some(ctx.tenant_id()),
                ),
                correlation_id,
                event_time,
                ctx.tenant_id(),
                uow.outbox.as_ref(),
            )
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Asset update / deactivate (P2.03 use cases)
// ---------------------------------------------------------------------------

/// Updates an asset's capability-bearing metadata (P2.03 use case; the
/// frozen §15.3 `AssetCapabilityUpdated` event is emitted by the use case
/// via the outbox). Cross-tenant asset: 404.
#[utoipa::path(
    patch,
    path = "/api/v1/assets/{asset_id}",
    request_body = UpdateAssetBody,
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 200, description = "Metadata updated", body = DataEnvelope<crate::dto::AssetDto>),
        (status = 401), (status = 403), (status = 404), (status = 422),
    )
)]
async fn update_asset(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
    Json(body): Json<UpdateAssetBody>,
) -> Result<Json<DataEnvelope<crate::dto::AssetDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetUpdate)?;
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    let rated_power_mw = ActivePowerMW::try_new(body.rated_power_mw)
        .map_err(|e| ApiError::validation(format!("rated_power_mw rejected: {e}")))?;
    let rated_energy_mwh = match body.rated_energy_mwh {
        None => None,
        Some(v) => Some(
            EnergyMWh::try_new(v)
                .map_err(|e| ApiError::validation(format!("rated_energy_mwh rejected: {e}")))?,
        ),
    };
    let ctx = caller.ctx.clone();
    let asset = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let use_case = flexcore_application::update_asset_metadata::UpdateAssetMetadata::new(
                uow.assets.clone(),
                uow.sites.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(
                    &ctx,
                    UpdateAssetMetadataCommand {
                        asset_id: id,
                        vendor_model: body.vendor_model,
                        rated_power_mw,
                        rated_energy_mwh,
                        capability_profile: body.capability_profile,
                    },
                )
                .map_err(ApiError::from)
        })
        .await?;
    Ok(Json(DataEnvelope { data: map_asset(&asset) }))
}

/// Deactivates an asset (P2.03 use case; terminal lifecycle transition,
/// mandatory persisted reason; the frozen §15.3 `AssetAvailabilityChanged`
/// event is emitted by the use case via the outbox). Cross-tenant: 404.
#[utoipa::path(
    post,
    path = "/api/v1/assets/{asset_id}/deactivate",
    request_body = DeactivateAssetBody,
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 200, description = "Asset deactivated", body = DataEnvelope<crate::dto::AssetDto>),
        (status = 401), (status = 403), (status = 404), (status = 422),
    )
)]
async fn deactivate_asset(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
    Json(body): Json<DeactivateAssetBody>,
) -> Result<Json<DataEnvelope<crate::dto::AssetDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetDeactivate)?;
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    let asset = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let use_case = flexcore_application::deactivate_asset::DeactivateAsset::new(
                uow.assets.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(
                    &ctx,
                    DeactivateAssetCommand {
                        asset_id: id,
                        reason: body.reason,
                    },
                )
                .map_err(ApiError::from)
        })
        .await?;
    Ok(Json(DataEnvelope { data: map_asset(&asset) }))
}

// ---------------------------------------------------------------------------
// Telemetry query (P2.12) / current state (P2.18) / quarantine (P2.07)
// ---------------------------------------------------------------------------

/// Runs the tenant-scoped telemetry time-range query (P2.12). Contract:
/// `asset` + `from`/`until` required (422); maximum range 7 days and
/// maximum page 1000 enforced (422 when exceeded); the quality filter
/// accepts ONLY the six canonical states (any seventh value is 422);
/// keyset-cursor pagination. A cross-tenant or unknown asset is 404.
/// P2.31: the optional `as_of` parameter switches the query onto the
/// versioned-corrections view (latest version per identity whose
/// correction timestamp is <= the instant; no cursor in that mode).
#[utoipa::path(
    get,
    path = "/api/v1/telemetry/query",
    params(TelemetryQueryParams),
    responses(
        (status = 200, description = "One page of records (ascending sort key) with the next cursor", body = TelemetryPageEnvelope),
        (status = 401), (status = 403), (status = 404),
        (status = 422, description = "Missing range, oversized range/page, seventh quality value, malformed cursor or as_of, as_of combined with a cursor", body = crate::error::ErrorEnvelope),
    )
)]
async fn query_telemetry(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<TelemetryQueryParams>,
) -> Result<Json<TelemetryPageEnvelope>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TelemetryRead)?;
    let reader = telemetry_reader(&state)?;

    // Required parameters (P2.12 step 3): asset + time range. A missing
    // one is a validation rejection, never an implicit unbounded query.
    let asset_raw = params
        .asset
        .ok_or_else(|| ApiError::validation("asset is required".to_string()))?;
    let asset_id = parse_asset_id(&asset_raw)
        .ok_or_else(ApiError::not_found)?;
    let from_raw = params
        .from
        .ok_or_else(|| ApiError::validation("from is required (a telemetry query always names a time range)".to_string()))?;
    let until_raw = params
        .until
        .ok_or_else(|| ApiError::validation("until is required (a telemetry query always names a time range)".to_string()))?;
    let from = parse_event_time(&from_raw)?;
    let until = parse_event_time(&until_raw)?;
    if from > until {
        return Err(ApiError::validation(
            "the query time range is inverted (from after until)".to_string(),
        ));
    }
    let range_millis = (until.timestamp() - from.timestamp()).whole_milliseconds();
    if range_millis > MAX_QUERY_RANGE_MILLIS {
        return Err(ApiError::validation(format!(
            "the query range exceeds the maximum of {MAX_QUERY_RANGE_MILLIS} ms (7 days)"
        )));
    }

    // Quantity filter: canonical ids only; absent = all frozen quantities.
    let quantities = match params.quantities {
        None => None,
        Some(raw) => {
            if raw.trim().is_empty() {
                return Err(ApiError::validation(
                    "quantities must name at least one canonical quantity id".to_string(),
                ));
            }
            let mut parsed = Vec::new();
            for part in raw.split(',') {
                let part = part.trim();
                match parse_quantity_key(part) {
                    Some(q) => parsed.push(q),
                    None => {
                        return Err(ApiError::validation(format!(
                            "unknown quantity '{part}'; the P2.04 §4 vocabulary is closed"
                        )))
                    }
                }
            }
            Some(parsed)
        }
    };

    // Quality filter: ONLY the six canonical states — a seventh value is
    // 422 (P2.12 step 3; the P0.08 vocabulary does not grow).
    let quality = match params.quality {
        None => None,
        Some(raw) => match parse_quality(raw.trim()) {
            Some(q) => Some(q),
            None => {
                return Err(ApiError::validation(format!(
                    "unknown quality state '{}'; the P0.08 vocabulary is closed (exactly six canonical states)",
                    raw.trim()
                )))
            }
        },
    };

    // Page size limits (recorded): default 200, max 1000.
    let limit = match params.limit {
        None => DEFAULT_PAGE_SIZE,
        Some(l) if l == 0 => {
            return Err(ApiError::validation(
                "limit must be at least one record".to_string(),
            ))
        }
        Some(l) if l > MAX_PAGE_SIZE => {
            return Err(ApiError::validation(format!(
                "limit exceeds the maximum page size of {MAX_PAGE_SIZE}"
            )))
        }
        Some(l) => l,
    };

    // Cursor: opaque keyset value; anything malformed is 422, never a
    // silent "first page".
    let cursor = match params.cursor {
        None => None,
        Some(raw) => match TelemetryCursor::parse(raw.trim()) {
            Some(c) => Some(c),
            None => {
                return Err(ApiError::validation(
                    "malformed cursor (an opaque keyset value from the previous page is required)"
                        .to_string(),
                ))
            }
        },
    };

    // P2.31 as-of (module header, D-C4/D-C5): parse when present. The
    // as-of read is served by the Postgres adapter's as-of method inside
    // one unit-of-work transaction; it has no cursor (a complete snapshot
    // bounded by `limit`), so cursor + as_of is a 422.
    let as_of = match params.as_of {
        None => None,
        Some(raw) => Some(parse_as_of(&raw).map_err(ApiError::validation)?),
    };
    if as_of.is_some() && cursor.is_some() {
        return Err(ApiError::validation(
            "as_of queries return a complete snapshot bounded by limit; cursor pagination \
             applies to the latest-version view only"
                .to_string(),
        ));
    }
    if let Some(as_of) = as_of {
        // Map the ports quantity set onto the adapter's frozen vocabulary
        // (1:1, the recorded P2.04 §4 keys).
        let pg_quantities: Option<Vec<flexcore_postgres_adapter::telemetry::Quantity>> =
            match &quantities {
                None => None,
                Some(set) => Some(
                    set.iter()
                        .map(|q| {
                            flexcore_postgres_adapter::telemetry::Quantity::from_db_text(
                                quantity_key(*q),
                            )
                            .map_err(|_| ApiError::infrastructure())
                        })
                        .collect::<Result<Vec<_>, ApiError>>()?,
                ),
            };
        let page_limit = limit;
        let ctx = caller.ctx.clone();
        let records = run_history_in_uow(&state, ctx.clone(), move |uow| {
            // Cross-tenant/unknown asset: 404 (no existence leak); the
            // trait-call spelling disambiguates the four repository traits.
            if flexcore_ports::repository::AssetRepository::find_by_id(uow, &ctx, asset_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            uow.query_telemetry_as_of(
                &ctx,
                &[asset_id],
                from,
                until,
                pg_quantities.as_deref(),
                Some(as_of),
                page_limit,
            )
            .map_err(map_history_error)
        })
        .await?;
        let data = records
            .iter()
            .map(map_stored_record)
            .collect::<Result<Vec<_>, ApiError>>()?;
        return Ok(Json(TelemetryPageEnvelope {
            data,
            next_cursor: None,
        }));
    }

    let ctx = caller.ctx.clone();
    let page = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            // Cross-tenant/unknown asset: 404 (no existence leak) — the
            // asset must exist in the caller's registry before telemetry
            // is read.
            if uow
                .assets
                .find_by_id(&ctx, asset_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let query = TelemetryReadQuery::new(
                asset_id,
                from,
                until,
                quantities,
                quality,
                limit,
                cursor,
            )
            .map_err(map_read_error)?;
            reader.query(&ctx, query).map_err(map_read_error)
        })
        .await?;
    Ok(Json(TelemetryPageEnvelope {
        data: page.records().iter().map(map_telemetry_record).collect(),
        next_cursor: page.next_cursor().map(|c| c.key().to_string()),
    }))
}

// ---------------------------------------------------------------------------
// Historical view (P2.19) — decisions D-H1..D-H6, module header
// ---------------------------------------------------------------------------

/// The P2.19 historical query parameters (decisions D-H1..D-H3): `asset` +
/// `from`/`until` REQUIRED over `event_time`; `quantities`/`quality` as in
/// the P2.12 query (the quality filter accepts ONLY the six canonical
/// states); raw mode paginates with the `hist-v1:` keyset cursor (D-H1);
/// `bucket` selects the aggregation mode over the closed width set
/// `1m|5m|15m|1h|1d` (D-H2) — bucket mode requires exactly one numeric
/// quantity and forbids `limit`/`cursor`.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TelemetryHistoryParams {
    /// The asset to query (required; a foreign-tenant asset is 404).
    #[serde(default)]
    pub asset: Option<String>,
    /// Inclusive range start over `event_time`, RFC 3339 (required).
    #[serde(default)]
    pub from: Option<String>,
    /// Inclusive range end over `event_time`, RFC 3339 (required).
    #[serde(default)]
    pub until: Option<String>,
    /// Comma-separated canonical quantity ids. Raw mode: any subset
    /// (absent = all). Bucket mode: exactly one numeric quantity.
    #[serde(default)]
    pub quantities: Option<String>,
    /// One canonical quality state (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE).
    #[serde(default)]
    pub quality: Option<String>,
    /// Raw mode only: page size (default 200, maximum [`MAX_PAGE_SIZE`]).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Raw mode only: the opaque keyset cursor of the previous page.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Aggregation mode: the bucket width from the closed D-H2 set
    /// (`1m|5m|15m|1h|1d`); absent = raw mode.
    #[serde(default)]
    pub bucket: Option<String>,
    /// P2.31 (raw mode only): the optional as-of instant, RFC 3339 UTC.
    /// With it, each (asset, event_time, quantity) returns the latest
    /// version whose correction timestamp is <= the instant; absent = the
    /// latest version.
    #[serde(default)]
    pub as_of: Option<String>,
}

/// Validates the raw-mode page size (D-H3; pure — unit-tested without a
/// database in `tests/telemetry_history.rs`). Default 200, maximum 1000.
/// Validates the optional `as_of` query parameter (P2.31; pure — testable
/// without a database): RFC 3339 UTC, parsed into the canonical
/// [`IngestTime`]. A malformed value is a validation rejection (422), never
/// a silent "latest".
pub fn parse_as_of(raw: &str) -> Result<flexcore_contracts::time_model::IngestTime, String> {
    let parsed = time::OffsetDateTime::parse(
        raw.trim(),
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| format!("'{raw}' is not a valid RFC 3339 UTC timestamp"))?;
    flexcore_contracts::time_model::IngestTime::new(parsed)
        .map_err(|e| format!("invalid as-of time: {e}"))
}

pub fn validate_history_limit(limit: Option<u32>) -> Result<u32, String> {
    match limit {
        None => Ok(DEFAULT_PAGE_SIZE),
        Some(l) if l == 0 => Err("limit must be at least one record".to_string()),
        Some(l) if l > MAX_PAGE_SIZE => Err(format!(
            "limit exceeds the maximum page size of {MAX_PAGE_SIZE}"
        )),
        Some(l) => Ok(l),
    }
}

/// Validates the REQUIRED event_time range against the reused P2.12 limits
/// (D-H3; pure). Inverted ranges and ranges over 7 days are rejections.
pub fn validate_history_range(from: EventTime, until: EventTime) -> Result<(), String> {
    if from > until {
        return Err("the query time range is inverted (from after until)".to_string());
    }
    let range_millis = (until.timestamp() - from.timestamp()).whole_milliseconds();
    if range_millis > MAX_QUERY_RANGE_MILLIS {
        return Err(format!(
            "the query range exceeds the maximum of {MAX_QUERY_RANGE_MILLIS} ms (7 days)"
        ));
    }
    Ok(())
}

/// The number of buckets a request touches (D-H3): the first bucket starts
/// exactly at `from` (the `date_bin` origin), so it is
/// `floor((until - from) / width) + 1`.
pub fn history_bucket_count(
    from: EventTime,
    until: EventTime,
    width: HistoryBucketWidth,
) -> u64 {
    let span_millis = (until.timestamp() - from.timestamp()).whole_milliseconds();
    (span_millis.max(0) / i128::from(width.millis())) as u64 + 1
}

/// Validates the bucket count against [`MAX_HISTORY_BUCKETS`] (D-H3; pure).
pub fn validate_history_buckets(
    from: EventTime,
    until: EventTime,
    width: HistoryBucketWidth,
) -> Result<(), String> {
    let count = history_bucket_count(from, until, width);
    if count > MAX_HISTORY_BUCKETS {
        return Err(format!(
            "the request spans {count} buckets of width '{}', exceeding the maximum of \
             {MAX_HISTORY_BUCKETS} (narrow the time range or widen the bucket)",
            width.key()
        ));
    }
    Ok(())
}

/// One stored record of a historical page (the P2.19 response shape: the
/// canonical field set with unit, quality, both timestamps, source identity
/// and sequence, and the `simulated` provenance flag — P2.17 step 3).
#[derive(Debug, Serialize, ToSchema)]
pub struct HistoricalRecordDto {
    pub asset_id: String,
    pub quantity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_num: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<String>,
    /// The canonical unit (P0.07): `MW`, `MVAr`, `MWh`, or `1`.
    pub unit: String,
    /// Exactly one of the six canonical states (P0.08).
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q_sign_convention: Option<String>,
    /// When the measurement occurred (P0.09 — the ordering/filtering key).
    pub event_time: String,
    /// When FlexCore received it (metadata; never an ordering key).
    pub ingest_time: String,
    pub source_id: String,
    pub source_sequence: i64,
    pub simulated: bool,
}

/// One page of the historical raw query, ascending by the keyset sort key
/// `(event_time, source_sequence, quantity)` (D-H1), with the cursor to
/// continue from (absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct HistoricalPageEnvelope {
    pub data: Vec<HistoricalRecordDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One occupied aggregation bucket (D-H2): count/mean/min/max of the
/// bucket's numeric values. NO ROUNDING is applied anywhere (P0.07): the
/// statistics are the stored double-precision values verbatim, in the
/// quantity's canonical unit; `P > 0` = export is never re-interpreted.
#[derive(Debug, Serialize, ToSchema)]
pub struct HistoryBucketDto {
    /// The bucket's start instant (the first bucket starts exactly at the
    /// query's `from`).
    pub bucket_start: String,
    /// The number of numeric rows in the bucket (a MISSING gap marker has
    /// no value and counts in no bucket).
    pub sample_count: i64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

/// The bucketed aggregation response (D-H2): all four statistics per
/// occupied interval, with the quantity and its canonical unit stated once.
#[derive(Debug, Serialize, ToSchema)]
pub struct HistoricalAggregateEnvelope {
    pub asset_id: String,
    pub quantity: String,
    pub unit: String,
    pub bucket_width: String,
    pub buckets: Vec<HistoryBucketDto>,
}

/// Maps one stored record onto the historical DTO (units from the frozen
/// quantity map; provenance from the source identity, P2.17 step 3).
fn map_history_record(r: &StoredTelemetry) -> HistoricalRecordDto {
    let (value_num, value_bool, value_text) = match &r.value {
        None => (None, None, None),
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Numeric(n)) => {
            (Some(*n), None, None)
        }
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Flag(b)) => {
            (None, Some(*b), None)
        }
        Some(flexcore_postgres_adapter::telemetry::TelemetryValue::Text(t)) => {
            (None, None, Some(t.clone()))
        }
    };
    HistoricalRecordDto {
        asset_id: r.asset_id.uuid().to_hyphenated().to_string(),
        quantity: r.quantity.db_text().to_string(),
        value_num,
        value_bool,
        value_text,
        unit: r.quantity.canonical_unit().to_string(),
        quality: crate::dto::quality_id(r.quality).to_string(),
        q_sign_convention: r
            .q_sign_convention
            .map(|c| match c {
                ReactiveSignConvention::InjectionPositive => "INJECTION_POSITIVE",
                ReactiveSignConvention::ConsumptionPositive => "CONSUMPTION_POSITIVE",
            })
            .map(str::to_string),
        event_time: rfc3339(r.event_time),
        ingest_time: rfc3339_ingest(r.ingest_time),
        source_id: r.source_id.clone(),
        source_sequence: r.source_sequence,
        simulated: is_simulated(&r.source_id),
    }
}

/// Runs the historical query inside ONE unit-of-work transaction on the
/// Postgres backend (module header, D-H5): the RLS tenant context is set by
/// `begin`, the operation runs synchronously on the adapter's private
/// runtime (via `spawn_blocking` — the recorded hosting rule), and the unit
/// of work commits only when the operation succeeded. The Test backend has
/// no SQL engine (no `date_bin`, no keyset SQL) and fails closed with the
/// opaque infrastructure error — the recorded coverage note is on the port
/// wiring above.
async fn run_history_in_uow<T, F>(state: &AppState, ctx: TenantContext, op: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&flexcore_postgres_adapter::PgUnitOfWork) -> Result<T, ApiError> + Send + 'static,
{
    match &state.backend {
        crate::state::Backend::Postgres { store, .. } => {
            let store = store.clone();
            tokio::task::spawn_blocking(move || {
                let uow = store.begin(Some(&ctx)).map_err(ApiError::from)?;
                match op(&uow) {
                    Ok(value) => {
                        uow.commit().map_err(ApiError::from)?;
                        Ok(value)
                    }
                    // Dropping the unfinalized unit of work spawns the
                    // native rollback (P1.10 rule 4).
                    Err(err) => {
                        drop(uow);
                        Err(err)
                    }
                }
            })
            .await
            .map_err(|_| ApiError::infrastructure())?
        }
        crate::state::Backend::Test { .. } => Err(ApiError::infrastructure()),
    }
}

/// Maps a telemetry storage error onto the API envelope. Never forwarded to
/// the client (opaque infrastructure failure) — the caller-visible
/// rejections (validation, 404) are decided in the handler before the
/// storage call.
fn map_history_error(e: flexcore_postgres_adapter::telemetry::TelemetryError) -> ApiError {
    let _ = e; // never forwarded to the client
    ApiError::infrastructure()
}

/// The P2.19 historical view (D-H1..D-H6): time-range queries over
/// `event_time` per asset and quantity, with raw keyset-paginated pages
/// (D-H1) or SQL-bucketed count/mean/min/max aggregation (D-H2, `bucket`
/// parameter). The `event_time` range is REQUIRED (422 when absent — never
/// an implicit unbounded query); the quality filter accepts ONLY the six
/// canonical states (a seventh value is 422); the limits of D-H3 (7-day
/// range, page 1000, 1000 buckets) are enforced with the stable error
/// envelope. A cross-tenant or unknown asset is 404 (no existence leak).
/// P2.31 (raw mode): the optional `as_of` parameter selects, per
/// (asset, event_time, quantity), the latest version whose correction
/// timestamp is <= the instant; absent = the latest version.
#[utoipa::path(
    get,
    path = "/api/v1/telemetry/history",
    params(TelemetryHistoryParams),
    responses(
        (status = 200, description = "One raw page (ascending keyset order) or the bucketed aggregation", body = HistoricalPageEnvelope),
        (status = 401), (status = 403), (status = 404),
        (status = 422, description = "Missing range, oversized range/page/bucket count, seventh quality value, malformed cursor or as_of, non-numeric or multi-quantity aggregation, as_of combined with bucket mode", body = crate::error::ErrorEnvelope),
    )
)]
async fn query_telemetry_history(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<TelemetryHistoryParams>,
) -> Result<axum::response::Response, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TelemetryRead)?;

    // Required parameters (D-H1/D-H3): asset + time range. A missing one is
    // a validation rejection, never an implicit unbounded query.
    let asset_raw = params
        .asset
        .ok_or_else(|| ApiError::validation("asset is required".to_string()))?;
    let asset_id = parse_asset_id(&asset_raw).ok_or_else(ApiError::not_found)?;
    let from_raw = params
        .from
        .ok_or_else(|| ApiError::validation("from is required (a historical query always names an event_time range)".to_string()))?;
    let until_raw = params
        .until
        .ok_or_else(|| ApiError::validation("until is required (a historical query always names an event_time range)".to_string()))?;
    let from = parse_event_time(&from_raw)?;
    let until = parse_event_time(&until_raw)?;
    validate_history_range(from, until).map_err(ApiError::validation)?;

    // Quality filter: ONLY the six canonical states (P0.08; the P2.12 rule).
    let quality = match params.quality {
        None => None,
        Some(raw) => match parse_quality(raw.trim()) {
            Some(q) => Some(q),
            None => {
                return Err(ApiError::validation(format!(
                    "unknown quality state '{}'; the P0.08 vocabulary is closed (exactly six canonical states)",
                    raw.trim()
                )))
            }
        },
    };

    // P2.31 as-of (raw mode only, D-C7): bucketed aggregation has no
    // record-level version semantics in Phase 2, so bucket + as_of is 422.
    let as_of = match &params.as_of {
        None => None,
        Some(raw) => Some(parse_as_of(raw).map_err(ApiError::validation)?),
    };

    // Bucket mode (D-H2): exactly one NUMERIC quantity, no pagination
    // parameters; the bucket width comes from the closed set and the
    // bucket count is bounded (D-H3).
    let bucket_width = match &params.bucket {
        None => None,
        Some(raw) => {
            if params.cursor.is_some() || params.limit.is_some() {
                return Err(ApiError::validation(
                    "bucket mode aggregates the whole range; limit and cursor are raw-mode \
                     parameters"
                        .to_string(),
                ));
            }
            if as_of.is_some() {
                return Err(ApiError::validation(
                    "as_of filters record versions; bucket mode has no record-level version \
                     semantics in Phase 2 (P2.31)"
                        .to_string(),
                ));
            }
            let width = HistoryBucketWidth::parse(raw)
                .ok_or_else(|| {
                    ApiError::validation(format!(
                        "unknown bucket width '{raw}'; the recorded set is 1m|5m|15m|1h|1d"
                    ))
                })?;
            validate_history_buckets(from, until, width).map_err(ApiError::validation)?;
            Some(width)
        }
    };

    let ctx = caller.ctx.clone();
    if let Some(width) = bucket_width {
        // Exactly one quantity, and it must be numeric-carrying (D-H2):
        // units are never mixed and state values have no numeric aggregate.
        let quantities_raw = params
            .quantities
            .ok_or_else(|| ApiError::validation(
                "bucket mode requires exactly one numeric quantity (units are never aggregated together, P0.07)"
                    .to_string(),
            ))?;
        let mut parsed: Vec<TelemetryQuantity> = Vec::new();
        for part in quantities_raw.split(',') {
            let part = part.trim();
            match parse_quantity_key(part) {
                Some(q) => parsed.push(q),
                None => {
                    return Err(ApiError::validation(format!(
                        "unknown quantity '{part}'; the P2.04 §4 vocabulary is closed"
                    )))
                }
            }
        }
        if parsed.len() != 1 {
            return Err(ApiError::validation(
                "bucket mode requires exactly one quantity".to_string(),
            ));
        }
        let quantity = parsed[0];
        let unit = quantity_unit(quantity).to_string();
        let quantity_key_text = quantity_key(quantity).to_string();
        // The adapter refuses non-numeric quantities as well (defence in
        // depth); the numeric filter here gives the caller a 422, not a 500.
        let numeric_only = matches!(
            quantity,
            TelemetryQuantity::ActivePower
                | TelemetryQuantity::ReactivePower
                | TelemetryQuantity::Soc
                | TelemetryQuantity::Energy
                | TelemetryQuantity::BaselineReference
        );
        if !numeric_only {
            return Err(ApiError::validation(
                "bucketed aggregation is defined for numeric-carrying quantities only \
                 (active_power, reactive_power, soc, energy, baseline_reference)"
                    .to_string(),
            ));
        }
        // Map the ports quantity onto the adapter's frozen vocabulary (1:1,
        // the recorded P2.04 §4 keys).
        let pg_quantity = flexcore_postgres_adapter::telemetry::Quantity::from_db_text(
            quantity_key(quantity),
        )
        .map_err(|_| ApiError::infrastructure())?;
        let buckets = run_history_in_uow(&state, ctx.clone(), move |uow| {
            // Cross-tenant/unknown asset: 404 (no existence leak). The
            // trait-call spelling disambiguates (four repository traits are
            // implemented by the unit of work).
            if flexcore_ports::repository::AssetRepository::find_by_id(uow, &ctx, asset_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            uow.aggregate_telemetry_history(
                &ctx,
                asset_id,
                pg_quantity,
                from,
                until,
                quality,
                width,
            )
            .map_err(map_history_error)
        })
        .await?;
        Ok(Json(HistoricalAggregateEnvelope {
            asset_id: asset_id.uuid().to_hyphenated().to_string(),
            quantity: quantity_key_text,
            unit,
            bucket_width: width.key().to_string(),
            buckets: buckets
                .iter()
                .map(|b| HistoryBucketDto {
                    bucket_start: rfc3339(b.bucket_start),
                    sample_count: b.sample_count,
                    mean: b.mean,
                    min: b.min,
                    max: b.max,
                })
                .collect(),
        })
        .into_response())
    } else {
        // Raw mode (D-H1): keyset-cursor pagination over the P2.05 PK's
        // varying legs; limits of D-H3.
        let limit = validate_history_limit(params.limit).map_err(ApiError::validation)?;
        let cursor = match params.cursor {
            None => None,
            Some(raw) => match HistoryCursor::decode(raw.trim()) {
                Some(c) => Some(c),
                None => {
                    return Err(ApiError::validation(
                        "malformed cursor (an opaque hist-v1 keyset value from the previous \
                         page is required)"
                            .to_string(),
                    ))
                }
            },
        };
        let quantities = match params.quantities {
            None => None,
            Some(raw) => {
                if raw.trim().is_empty() {
                    return Err(ApiError::validation(
                        "quantities must name at least one canonical quantity id".to_string(),
                    ));
                }
                let mut parsed: Vec<flexcore_postgres_adapter::telemetry::Quantity> = Vec::new();
                for part in raw.split(',') {
                    let part = part.trim();
                    match flexcore_postgres_adapter::telemetry::Quantity::from_db_text(part) {
                        Ok(q) => parsed.push(q),
                        Err(_) => {
                            return Err(ApiError::validation(format!(
                                "unknown quantity '{part}'; the P2.04 §4 vocabulary is closed"
                            )))
                        }
                    }
                }
                Some(parsed)
            }
        };
        let (records, next_cursor) = run_history_in_uow(&state, ctx.clone(), move |uow| {
            // Cross-tenant/unknown asset: 404 (no existence leak); the
            // trait-call spelling disambiguates (see the bucket branch).
            if flexcore_ports::repository::AssetRepository::find_by_id(uow, &ctx, asset_id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            // P2.31: the as-of aware variant — without `as_of` it returns
            // the latest version per identity, identical semantics to the
            // D-H1 query plus the version-visibility predicate (D-C5).
            uow.query_telemetry_history_as_of(
                &ctx,
                asset_id,
                from,
                until,
                quantities.as_deref(),
                quality,
                limit,
                cursor,
                as_of,
            )
            .map_err(map_history_error)
        })
        .await?;
        Ok(Json(HistoricalPageEnvelope {
            data: records.iter().map(map_history_record).collect(),
            next_cursor: next_cursor.map(|c| c.encode()),
        })
        .into_response())
    }
}

/// The P2.18 current state of one asset: the latest record per quantity,
/// latest by `event_time` (never latest arrival; a late older record never
/// regresses the state, a late newer one advances it). A quantity with no
/// record is absent — MISSING is derived from absence, never a zero value.
/// The current-state freshness limit (configuration,
/// `FLEXCORE_CURRENT_STATE_FRESHNESS_MS`) is applied by the P2.08
/// staleness rule: a value older than the limit is shown WITH the STALE
/// flag — the last known value preserved, never presented as GOOD. Every
/// entry carries value + unit + quality + event_time + ingest_time + age +
/// `simulated: true`.
#[utoipa::path(
    get,
    path = "/api/v1/assets/{asset_id}/state",
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 200, description = "Latest record per quantity, with quality and age", body = DataEnvelope<CurrentStateDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn get_asset_state(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
) -> Result<Json<DataEnvelope<CurrentStateDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TelemetryRead)?;
    let reader = telemetry_reader(&state)?;
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    let (now, mut latest) = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            if uow
                .assets
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::not_found());
            }
            let now = uow.clock.now_event_time();
            let latest = reader.latest_state(&ctx, id).map_err(map_read_error)?;
            Ok((now, latest))
        })
        .await?;
    latest.sort_by_key(|r| quantity_key(r.quantity()));
    Ok(Json(DataEnvelope {
        data: CurrentStateDto {
            asset_id: id.uuid().to_hyphenated().to_string(),
            generated_at: rfc3339(now),
            quantities: latest
                .iter()
                .map(|r| map_quantity_state(r, now, state.current_state_freshness_millis))
                .collect(),
            simulated: true,
        },
    }))
}

/// Lists the ingestion quarantine (P2.07 §3): the evidence trail of
/// rejected records, tenant-scoped, admin role only (`quarantine.read` is
/// granted to the admin roles by policy — a viewer or operator is 403).
#[utoipa::path(
    get,
    path = "/api/v1/telemetry/quarantine",
    responses(
        (status = 200, description = "The tenant's quarantined records", body = crate::dto::ListEnvelope<QuarantineEntryDto>),
        (status = 401), (status = 403),
    )
)]
async fn list_quarantine(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
) -> Result<Json<crate::dto::ListEnvelope<QuarantineEntryDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::QuarantineRead)?;
    let reader = telemetry_reader(&state)?;
    // A pure read; the quarantine store is read through the port with the
    // caller's context (the Postgres implementation will run inside the
    // request transaction with RLS applied — recorded on the port).
    let entries = reader.list_quarantine(&caller.ctx).map_err(map_read_error)?;
    Ok(Json(crate::dto::ListEnvelope {
        data: entries
            .iter()
            .map(|e| QuarantineEntryDto {
                raw_payload: e.raw_payload().to_string(),
                failure_reason: e.failure_reason().to_string(),
                source_id: e.source_id().to_string(),
                asset_id: e.asset_id().map(|a| a.uuid().to_hyphenated().to_string()),
                event_time: e.event_time().map(rfc3339),
                quarantined_at: rfc3339_ingest(e.quarantined_at()),
                attempt_count: e.attempt_count(),
            })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Registry views (P2.17)
// ---------------------------------------------------------------------------

/// The P2.17 portfolio view: composition, aggregation rules, and the
/// commitments placeholder (populated in a later phase — stated, not
/// fabricated).
#[utoipa::path(
    get,
    path = "/api/v1/portfolios/{portfolio_id}/view",
    params(("portfolio_id" = String, Path, description = "Portfolio UUID")),
    responses(
        (status = 200, body = DataEnvelope<PortfolioViewDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn portfolio_view(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(portfolio_id): Path<String>,
) -> Result<Json<DataEnvelope<PortfolioViewDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::PortfolioRead)?;
    let id = parse_portfolio_id(&portfolio_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    let view = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let portfolio = uow
                .portfolios
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .ok_or_else(ApiError::not_found)?;
            let sites = uow
                .sites
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .into_iter()
                .filter(|s| s.portfolio_id() == id)
                .collect::<Vec<_>>();
            let site_ids: Vec<_> = sites.iter().map(|s| s.site_id()).collect();
            let asset_count = uow
                .assets
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .iter()
                .filter(|a| site_ids.contains(&a.site_id()))
                .count();
            Ok(PortfolioViewDto {
                id: portfolio.portfolio_id().uuid().to_hyphenated().to_string(),
                tenant_id: portfolio.tenant_id().uuid().to_hyphenated().to_string(),
                name: portfolio.name().to_string(),
                description: portfolio.description().map(|s| s.to_string()),
                operator: portfolio.operator().to_string(),
                market_region: portfolio.market_region().to_string(),
                aggregation_rules: portfolio.aggregation_rules().to_string(),
                composition: PortfolioCompositionDto {
                    site_count: sites.len(),
                    asset_count,
                },
                commitments: CommitmentsPlaceholderDto {
                    populated: false,
                    note: "commitments are a later-phase feature; Phase 2 stores no commitments"
                        .to_string(),
                },
                created_at: rfc3339(portfolio.created_at()),
            })
        })
        .await?;
    Ok(Json(DataEnvelope { data: view }))
}

/// The P2.17 site view: overview, PCC, grid-model reference, and the
/// HONEST edge-connectivity report — `simulator` when the site carries an
/// edge identity (the only Phase 2 provider set is the simulator), else
/// `not_connected`; `live` is always `false` in Phase 2 (first site
/// integration is v0.4.0). The API never presents a simulated edge as live.
#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/view",
    params(("site_id" = String, Path, description = "Site UUID")),
    responses(
        (status = 200, body = DataEnvelope<SiteViewDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn site_view(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(site_id): Path<String>,
) -> Result<Json<DataEnvelope<SiteViewDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::SiteRead)?;
    let id = parse_site_id(&site_id).ok_or_else(ApiError::not_found)?;
    let ctx = caller.ctx.clone();
    let view = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let site = uow
                .sites
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .ok_or_else(ApiError::not_found)?;
            let asset_count = uow
                .assets
                .list_for_tenant(&ctx)
                .map_err(ApiError::from)?
                .iter()
                .filter(|a| a.site_id() == id)
                .count();
            let edge_identity = site.edge_identity().to_string();
            Ok(SiteViewDto {
                id: site.site_id().uuid().to_hyphenated().to_string(),
                portfolio_id: site.portfolio_id().uuid().to_hyphenated().to_string(),
                tenant_id: site.tenant_id().uuid().to_hyphenated().to_string(),
                name: site.name().to_string(),
                description: site.description().map(|d| d.to_string()),
                location: site.location().to_string(),
                timezone: site.timezone().as_str().to_string(),
                point_of_common_coupling: site.point_of_common_coupling().to_string(),
                grid_model_reference: site.grid_model_reference().to_string(),
                edge_connectivity: EdgeConnectivityDto {
                    status: if edge_identity.is_empty() {
                        "not_connected"
                    } else {
                        "simulator"
                    }
                    .to_string(),
                    edge_identity,
                    live: false,
                },
                overview: SiteOverviewDto { asset_count },
                created_at: rfc3339(site.created_at()),
            })
        })
        .await?;
    Ok(Json(DataEnvelope { data: view }))
}

/// The P2.17 asset view: capability reference, stored limits, lifecycle /
/// maintenance-outage status (stored registry fields), and — when the
/// telemetry read port is wired — the latest telemetry per quantity with
/// quality and the `simulated: true` flag on every simulator-derived value.
#[utoipa::path(
    get,
    path = "/api/v1/assets/{asset_id}/view",
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 200, body = DataEnvelope<AssetViewDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn asset_view(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
) -> Result<Json<DataEnvelope<AssetViewDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetRead)?;
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    let reader = state.telemetry.clone();
    let freshness = state.current_state_freshness_millis;
    let ctx = caller.ctx.clone();
    let view = state
        .run_in_uow(Some(ctx.clone()), move |uow| {
            let asset = uow
                .assets
                .find_by_id(&ctx, id)
                .map_err(ApiError::from)?
                .ok_or_else(ApiError::not_found)?;
            // The latest-telemetry section is attached only when the read
            // port is wired (Test backend); the Postgres wiring is the
            // recorded P2.06-adjacent gap — the view stays honest about it
            // by omitting the section, never by fabricating values.
            let current_state = match reader.as_ref() {
                None => None,
                Some(reader) => {
                    let now = uow.clock.now_event_time();
                    let mut latest = reader.latest_state(&ctx, id).map_err(map_read_error)?;
                    latest.sort_by_key(|r| quantity_key(r.quantity()));
                    Some(CurrentStateDto {
                        asset_id: id.uuid().to_hyphenated().to_string(),
                        generated_at: rfc3339(now),
                        quantities: latest
                            .iter()
                            .map(|r| map_quantity_state(r, now, freshness))
                            .collect(),
                        simulated: true,
                    })
                }
            };
            let lifecycle = match asset.lifecycle_status() {
                flexcore_domain::entities::AssetLifecycleStatus::Active => "active",
                flexcore_domain::entities::AssetLifecycleStatus::Deactivated => "deactivated",
            };
            Ok(AssetViewDto {
                id: asset.asset_id().uuid().to_hyphenated().to_string(),
                site_id: asset.site_id().uuid().to_hyphenated().to_string(),
                tenant_id: asset.tenant_id().uuid().to_hyphenated().to_string(),
                name: asset.name().to_string(),
                description: asset.description().map(|d| d.to_string()),
                asset_type: crate::dto::asset_type_id(asset.asset_type()).to_string(),
                vendor_model: asset.vendor_model().to_string(),
                capability_profile: asset.capability_profile().to_string(),
                limits: AssetLimitsDto {
                    rated_power_mw: asset.rated_power_mw().value(),
                    rated_energy_mwh: asset.rated_energy_mwh().map(|e| e.value()),
                },
                lifecycle_status: lifecycle.to_string(),
                // Stored registry field (P2.17): derived from the stored
                // lifecycle status — `operational` while active, otherwise
                // `deactivated`. Richer maintenance windows are a
                // later-phase field (recorded assumption).
                maintenance_outage_status: match asset.lifecycle_status() {
                    flexcore_domain::entities::AssetLifecycleStatus::Active => "operational",
                    flexcore_domain::entities::AssetLifecycleStatus::Deactivated => "deactivated",
                }
                .to_string(),
                current_state,
                created_at: rfc3339(asset.created_at()),
            })
        })
        .await?;
    Ok(Json(DataEnvelope { data: view }))
}

// ---------------------------------------------------------------------------
// Parsing helpers (local to the telemetry routes)
// ---------------------------------------------------------------------------

/// Parses an RFC 3339 UTC timestamp into a canonical `EventTime`. A
/// non-UTC offset or malformed value is a validation rejection (422), never
/// a converted guess (P0.09: UTC only, non-UTC offsets are rejected).
fn parse_event_time(raw: &str) -> Result<EventTime, ApiError> {
    let parsed = time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            ApiError::validation(format!(
                "'{raw}' is not a valid RFC 3339 UTC timestamp"
            ))
        })?;
    EventTime::new(parsed).map_err(|e| ApiError::validation(format!("invalid event time: {e}")))
}
