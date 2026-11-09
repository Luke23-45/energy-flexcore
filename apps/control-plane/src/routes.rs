// P1.17 — Stage 1 API routes (the implemented subset of the P0.15 surface).
//
// Implemented here (recorded subset; the full 16-resource list belongs to
// later phases): tenants, portfolios, sites, assets — create, list,
// get-by-id. There is NO "execute arbitrary command" endpoint and no route
// outside the P0.15 resource list (P0.15 prohibitions).
//
// Every handler: resolve identity (CallerExtractor; 401 when absent or
// invalid) -> authorize (deny-by-default, explicit scope; 403 on deny) ->
// run the use case or repository read inside ONE unit of work
// (state.run_in_uow — the real backend is transaction-bound) -> map errors
// to the stable envelope (401/403/404/422/500; cross-tenant references are
// 404, no existence leak).
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use flexcore_application::create_portfolio::CreatePortfolioCommand;
use flexcore_application::create_site::CreateSiteCommand;
use flexcore_application::register_asset::RegisterAssetCommand;
use flexcore_application::register_tenant::RegisterTenantCommand;
use flexcore_authz::action::Action;
use flexcore_authz::scope::{AssetScope, Scope, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::entities::{Asset, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
// Trait-object method calls on the UnitOfWork's ports.
use flexcore_ports::repository::{
    AssetRepository as _, PortfolioRepository as _, SiteRepository as _, TenantRepository as _,
};
use serde::Deserialize;
use axum::extract::Query;
use utoipa::ToSchema;

use crate::auth::{authorize_tenant_scoped, evaluate};
use crate::dto::{
    asset_type_id, parse_asset_type, rfc3339, AssetDto, DataEnvelope, ListEnvelope, PortfolioDto,
    SiteDto, TenantDto,
};
use crate::error::ApiError;
use crate::state::AppState;

/// Builds the Stage 1 router (subset of `/api/v1` per P0.15) plus the
/// probe/metrics/OpenAPI endpoints, merged with the Phase 2 routes
/// (P2.12/P2.17: registry DELETE/update/deactivate, telemetry query,
/// current state, quarantine, registry views) and the Phase 3 routes
/// (P3.28: flexibility envelopes, schedules, optimization runs — the
/// §15.2 resources of the Phase 3 line; still NO execute-command
/// endpoint).
pub fn stage1_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/tenants", post(create_tenant).get(list_tenants))
        .route("/api/v1/tenants/{tenant_id}", get(get_tenant))
        .route(
            "/api/v1/portfolios",
            post(create_portfolio).get(list_portfolios),
        )
        .route("/api/v1/portfolios/{portfolio_id}", get(get_portfolio))
        .route("/api/v1/sites", post(create_site).get(list_sites))
        .route("/api/v1/sites/{site_id}", get(get_site))
        .route("/api/v1/assets", post(create_asset).get(list_assets))
        .route("/api/v1/assets/{asset_id}", get(get_asset))
        .route("/api/v1/openapi.json", get(crate::openapi::serve_openapi))
        .route("/health", get(crate::probes::health))
        .route("/ready", get(crate::probes::ready))
        .route("/metrics", get(crate::metrics_route::metrics))
        .merge(crate::routes_phase2::phase2_router())
        .merge(crate::routes_phase3::phase3_router())
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// The P2.17 registry list paging parameters (recorded decision): registry
/// lists accept a `limit` only — the registry tables are small current-state
/// tables, and the keyset-cursor contract (P2.12) belongs to the
/// time-ordered telemetry query where offsets would be unstable. Default
/// 200, maximum 1000 (the P2.12 page-size limit; over the maximum is a 422).
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListParams {
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Applies the recorded registry list paging: default 200, max 1000, a
/// value over the maximum is a validation rejection (422).
pub(crate) fn apply_list_limit(
    limit: Option<u32>,
) -> Result<usize, crate::error::ApiError> {
    const DEFAULT: u32 = 200;
    const MAX: u32 = 1000;
    match limit {
        Some(l) if l > MAX => Err(crate::error::ApiError::validation(format!(
            "limit exceeds the maximum page size of {MAX}"
        ))),
        Some(l) if l == 0 => Err(crate::error::ApiError::validation(
            "limit must be at least one record".to_string(),
        )),
        Some(l) => Ok(l as usize),
        None => Ok(DEFAULT as usize),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTenantBody {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub identity_provider_reference: String,
    pub policy_profile: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePortfolioBody {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub operator: String,
    pub market_region: String,
    pub aggregation_rules: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSiteBody {
    pub portfolio_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub location: String,
    pub timezone: String,
    pub point_of_common_coupling: String,
    pub grid_model_reference: String,
    pub edge_identity: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAssetBody {
    pub site_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// The canonical asset-type id (P0.10 §2 vocabulary; closed).
    pub asset_type: String,
    pub vendor_model: String,
    /// Rated active power, canonical MW (P0.07).
    pub rated_power_mw: f64,
    /// Rated energy, canonical MWh, when applicable (storage assets).
    #[serde(default)]
    pub rated_energy_mwh: Option<f64>,
    pub phase: String,
    pub bus_reference: String,
    pub capability_profile: String,
}

// ---------------------------------------------------------------------------
// DTO mapping
// ---------------------------------------------------------------------------

fn map_tenant(t: &Tenant) -> TenantDto {
    TenantDto {
        id: t.tenant_id().uuid().to_hyphenated(),
        name: t.name().to_string(),
        description: t.description().map(|s| s.to_string()),
        identity_provider_reference: t.identity_provider_reference().to_string(),
        policy_profile: t.policy_profile().to_string(),
        created_at: rfc3339(t.created_at()),
    }
}

fn map_portfolio(p: &Portfolio) -> PortfolioDto {
    PortfolioDto {
        id: p.portfolio_id().uuid().to_hyphenated(),
        tenant_id: p.tenant_id().uuid().to_hyphenated(),
        name: p.name().to_string(),
        description: p.description().map(|s| s.to_string()),
        operator: p.operator().to_string(),
        market_region: p.market_region().to_string(),
        aggregation_rules: p.aggregation_rules().to_string(),
        created_at: rfc3339(p.created_at()),
    }
}

fn map_site(s: &Site) -> SiteDto {
    SiteDto {
        id: s.site_id().uuid().to_hyphenated(),
        portfolio_id: s.portfolio_id().uuid().to_hyphenated(),
        tenant_id: s.tenant_id().uuid().to_hyphenated(),
        name: s.name().to_string(),
        description: s.description().map(|d| d.to_string()),
        location: s.location().to_string(),
        timezone: s.timezone().as_str().to_string(),
        point_of_common_coupling: s.point_of_common_coupling().to_string(),
        grid_model_reference: s.grid_model_reference().to_string(),
        edge_identity: s.edge_identity().to_string(),
        created_at: rfc3339(s.created_at()),
    }
}

pub(crate) fn map_asset(a: &Asset) -> AssetDto {
    AssetDto {
        id: a.asset_id().uuid().to_hyphenated(),
        site_id: a.site_id().uuid().to_hyphenated(),
        tenant_id: a.tenant_id().uuid().to_hyphenated(),
        name: a.name().to_string(),
        description: a.description().map(|d| d.to_string()),
        asset_type: asset_type_id(a.asset_type()).to_string(),
        vendor_model: a.vendor_model().to_string(),
        rated_power_mw: a.rated_power_mw().value(),
        rated_energy_mwh: a.rated_energy_mwh().map(|e| e.value()),
        phase: a.phase().to_string(),
        bus_reference: a.bus_reference().to_string(),
        capability_profile: a.capability_profile().to_string(),
        lifecycle_status: match a.lifecycle_status() {
            flexcore_domain::entities::AssetLifecycleStatus::Active => "active",
            flexcore_domain::entities::AssetLifecycleStatus::Deactivated => "deactivated",
        }
        .to_string(),
        deactivation_reason: a.deactivation_reason().map(|s| s.to_string()),
        deactivated_at: a.deactivated_at().map(rfc3339),
        created_at: rfc3339(a.created_at()),
    }
}

// ---------------------------------------------------------------------------
// ID parsing helpers (unparsable ids are 404 — no existence leak, no 500)
// ---------------------------------------------------------------------------

pub(crate) fn parse_tenant_id(raw: &str) -> Option<TenantId> {
    FlexUuid::parse(raw).ok().map(TenantId::new)
}

pub(crate) fn parse_portfolio_id(raw: &str) -> Option<PortfolioId> {
    FlexUuid::parse(raw).ok().map(PortfolioId::new)
}

pub(crate) fn parse_site_id(raw: &str) -> Option<SiteId> {
    FlexUuid::parse(raw).ok().map(SiteId::new)
}

pub(crate) fn parse_asset_id(raw: &str) -> Option<AssetId> {
    FlexUuid::parse(raw).ok().map(AssetId::new)
}

// ---------------------------------------------------------------------------
// Handlers — tenants
// ---------------------------------------------------------------------------

/// Creates a tenant (platform-level registry action). The acting
/// principal's own tenant scopes the evaluation; the new tenant's
/// identity comes from the request, never a context default.
#[utoipa::path(
    post,
    path = "/api/v1/tenants",
    request_body = CreateTenantBody,
    responses(
        (status = 201, description = "Tenant created", body = DataEnvelope<TenantDto>),
        (status = 401, description = "Missing or invalid authentication"),
        (status = 403, description = "Denied by policy"),
        (status = 422, description = "Validation rejected", body = crate::error::ErrorEnvelope),
    )
)]
async fn create_tenant(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateTenantBody>,
) -> Result<(StatusCode, Json<DataEnvelope<TenantDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TenantCreate)?;
    let tenant = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            let use_case = flexcore_application::register_tenant::RegisterTenant::new(
                uow.tenants.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(RegisterTenantCommand {
                    name: body.name,
                    description: body.description,
                    identity_provider_reference: body.identity_provider_reference,
                    policy_profile: body.policy_profile,
                })
                .map_err(ApiError::from)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope { data: map_tenant(&tenant) }),
    ))
}

/// Lists tenants visible to the caller: the caller's own tenant only
/// (tenant-scoped; no platform-wide tenant enumeration in Phase 1).
#[utoipa::path(
    get,
    path = "/api/v1/tenants",
    responses(
        (status = 200, body = ListEnvelope<TenantDto>),
        (status = 401), (status = 403),
    )
)]
async fn list_tenants(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
) -> Result<Json<ListEnvelope<TenantDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TenantRead)?;
    let own = caller.ctx.tenant_id();
    let out = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            let own_tenant = uow.tenants.find_by_id(own)?;
            Ok(own_tenant
                .map(|t| vec![map_tenant(&t)])
                .unwrap_or_default())
        })
        .await?;
    Ok(Json(ListEnvelope { data: out }))
}

/// Gets a tenant by id. A tenant other than the caller's own is 404 (no
/// existence leak across the tenancy boundary).
#[utoipa::path(
    get,
    path = "/api/v1/tenants/{tenant_id}",
    params(("tenant_id" = String, Path, description = "Tenant UUID")),
    responses(
        (status = 200, body = DataEnvelope<TenantDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn get_tenant(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(tenant_id): Path<String>,
) -> Result<Json<DataEnvelope<TenantDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::TenantRead)?;
    let id = parse_tenant_id(&tenant_id).ok_or_else(ApiError::not_found)?;
    // Cross-tenant read: 404, never 403 (no existence leak).
    if id != caller.ctx.tenant_id() {
        return Err(ApiError::not_found());
    }
    let tenant = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.tenants.find_by_id(id).map_err(ApiError::from)
        })
        .await?;
    match tenant {
        Some(t) => Ok(Json(DataEnvelope { data: map_tenant(&t) })),
        None => Err(ApiError::not_found()),
    }
}

// ---------------------------------------------------------------------------
// Handlers — portfolios
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/api/v1/portfolios",
    request_body = CreatePortfolioBody,
    responses(
        (status = 201, body = DataEnvelope<PortfolioDto>),
        (status = 401), (status = 403), (status = 422),
    )
)]
async fn create_portfolio(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreatePortfolioBody>,
) -> Result<(StatusCode, Json<DataEnvelope<PortfolioDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::PortfolioCreate)?;
    let portfolio = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            let use_case = flexcore_application::create_portfolio::CreatePortfolio::new(
                uow.tenants.clone(),
                uow.portfolios.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(
                    &caller.ctx,
                    CreatePortfolioCommand {
                        name: body.name,
                        description: body.description,
                        operator: body.operator,
                        market_region: body.market_region,
                        aggregation_rules: body.aggregation_rules,
                    },
                )
                .map_err(ApiError::from)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope { data: map_portfolio(&portfolio) }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/portfolios",
    responses((status = 200, body = ListEnvelope<PortfolioDto>), (status = 401), (status = 403))
)]
async fn list_portfolios(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<ListParams>,
) -> Result<Json<ListEnvelope<PortfolioDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::PortfolioRead)?;
    let limit = apply_list_limit(params.limit)?;
    let items = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.portfolios
                .list_for_tenant(&caller.ctx)
                .map(|rows| {
                    rows.iter()
                        .take(limit)
                        .map(map_portfolio)
                        .collect()
                })
                .map_err(ApiError::from)
        })
        .await?;
    Ok(Json(ListEnvelope { data: items }))
}

#[utoipa::path(
    get,
    path = "/api/v1/portfolios/{portfolio_id}",
    params(("portfolio_id" = String, Path, description = "Portfolio UUID")),
    responses(
        (status = 200, body = DataEnvelope<PortfolioDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn get_portfolio(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(portfolio_id): Path<String>,
) -> Result<Json<DataEnvelope<PortfolioDto>>, ApiError> {
    let id = parse_portfolio_id(&portfolio_id).ok_or_else(ApiError::not_found)?;
    // Asset/portfolio reads evaluate against the resource's own scope; the
    // tenant match inside the policy denies cross-tenant scopes.
    // Portfolio reads evaluate at tenant scope (the Scope enum has no
    // portfolio kind; the policy's tenant match enforces isolation).
    evaluate(
        &state.policies,
        &caller,
        Action::PortfolioRead,
        Some(Scope::Tenant(TenantScope::new(caller.ctx.tenant_id()))),
    )?;
    let portfolio = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.portfolios
                .find_by_id(&caller.ctx, id)
                .map_err(ApiError::from)
        })
        .await?;
    match portfolio {
        // Not found within the caller's tenant scope — 404 either way.
        Some(p) => Ok(Json(DataEnvelope { data: map_portfolio(&p) })),
        None => Err(ApiError::not_found()),
    }
}

// ---------------------------------------------------------------------------
// Handlers — sites
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/api/v1/sites",
    request_body = CreateSiteBody,
    responses(
        (status = 201, body = DataEnvelope<SiteDto>),
        (status = 401), (status = 403), (status = 404), (status = 422),
    )
)]
async fn create_site(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateSiteBody>,
) -> Result<(StatusCode, Json<DataEnvelope<SiteDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::SiteCreate)?;
    let portfolio_id = parse_portfolio_id(&body.portfolio_id).ok_or_else(ApiError::not_found)?;
    let site = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            let use_case = flexcore_application::create_site::CreateSite::new(
                uow.portfolios.clone(),
                uow.sites.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(
                    &caller.ctx,
                    CreateSiteCommand {
                        portfolio_id,
                        name: body.name,
                        description: body.description,
                        location: body.location,
                        timezone: body.timezone,
                        point_of_common_coupling: body.point_of_common_coupling,
                        grid_model_reference: body.grid_model_reference,
                        edge_identity: body.edge_identity,
                    },
                )
                .map_err(ApiError::from)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope { data: map_site(&site) }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/sites",
    responses((status = 200, body = ListEnvelope<SiteDto>), (status = 401), (status = 403))
)]
async fn list_sites(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<ListParams>,
) -> Result<Json<ListEnvelope<SiteDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::SiteRead)?;
    let limit = apply_list_limit(params.limit)?;
    let items = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.sites
                .list_for_tenant(&caller.ctx)
                .map(|rows| {
                    rows.iter()
                        .take(limit)
                        .map(map_site)
                        .collect()
                })
                .map_err(ApiError::from)
        })
        .await?;
    Ok(Json(ListEnvelope { data: items }))
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site UUID")),
    responses(
        (status = 200, body = DataEnvelope<SiteDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn get_site(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(site_id): Path<String>,
) -> Result<Json<DataEnvelope<SiteDto>>, ApiError> {
    let id = parse_site_id(&site_id).ok_or_else(ApiError::not_found)?;
    evaluate(
        &state.policies,
        &caller,
        Action::SiteRead,
        Some(Scope::Site(SiteScope::new(caller.ctx.tenant_id(), id))),
    )?;
    let site = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.sites.find_by_id(&caller.ctx, id).map_err(ApiError::from)
        })
        .await?;
    match site {
        Some(s) => Ok(Json(DataEnvelope { data: map_site(&s) })),
        None => Err(ApiError::not_found()),
    }
}

// ---------------------------------------------------------------------------
// Handlers — assets
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/api/v1/assets",
    request_body = CreateAssetBody,
    responses(
        (status = 201, body = DataEnvelope<AssetDto>),
        (status = 401), (status = 403), (status = 404), (status = 422),
    )
)]
async fn create_asset(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Json(body): Json<CreateAssetBody>,
) -> Result<(StatusCode, Json<DataEnvelope<AssetDto>>), ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetCreate)?;
    let site_id = parse_site_id(&body.site_id).ok_or_else(ApiError::not_found)?;
    let asset_type = parse_asset_type(&body.asset_type).ok_or_else(|| {
        ApiError::validation(format!(
            "unknown asset_type '{}'; the P0.10 §2 vocabulary is closed",
            body.asset_type
        ))
    })?;
    let rated_power_mw = ActivePowerMW::try_new(body.rated_power_mw).map_err(|e| {
        ApiError::validation(format!("rated_power_mw rejected: {e}"))
    })?;
    let rated_energy_mwh = match body.rated_energy_mwh {
        None => None,
        Some(v) => Some(
            flexcore_contracts::units::EnergyMWh::try_new(v)
                .map_err(|e| ApiError::validation(format!("rated_energy_mwh rejected: {e}")))?,
        ),
    };
    let asset = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            let use_case = flexcore_application::register_asset::RegisterAsset::new(
                uow.sites.clone(),
                uow.assets.clone(),
                uow.outbox.clone(),
                uow.ids.clone(),
                uow.clock.clone(),
                uow.event_encoder(),
            );
            use_case
                .execute(
                    &caller.ctx,
                    RegisterAssetCommand {
                        site_id,
                        name: body.name,
                        description: body.description,
                        asset_type,
                        vendor_model: body.vendor_model,
                        rated_power_mw,
                        rated_energy_mwh,
                        phase: body.phase,
                        bus_reference: body.bus_reference,
                        capability_profile: body.capability_profile,
                    },
                )
                .map_err(ApiError::from)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope { data: map_asset(&asset) }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/assets",
    responses((status = 200, body = ListEnvelope<AssetDto>), (status = 401), (status = 403))
)]
async fn list_assets(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Query(params): Query<ListParams>,
) -> Result<Json<ListEnvelope<AssetDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::AssetRead)?;
    let limit = apply_list_limit(params.limit)?;
    let items = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.assets
                .list_for_tenant(&caller.ctx)
                .map(|rows| {
                    rows.iter()
                        .take(limit)
                        .map(map_asset)
                        .collect()
                })
                .map_err(ApiError::from)
        })
        .await?;
    Ok(Json(ListEnvelope { data: items }))
}

#[utoipa::path(
    get,
    path = "/api/v1/assets/{asset_id}",
    params(("asset_id" = String, Path, description = "Asset UUID")),
    responses(
        (status = 200, body = DataEnvelope<AssetDto>),
        (status = 401), (status = 403), (status = 404),
    )
)]
async fn get_asset(
    State(state): State<AppState>,
    crate::auth::CallerExtractor(caller): crate::auth::CallerExtractor,
    Path(asset_id): Path<String>,
) -> Result<Json<DataEnvelope<AssetDto>>, ApiError> {
    let id = parse_asset_id(&asset_id).ok_or_else(ApiError::not_found)?;
    evaluate(
        &state.policies,
        &caller,
        Action::AssetRead,
        Some(Scope::Asset(AssetScope::new(caller.ctx.tenant_id(), id))),
    )?;
    let asset = state
        .run_in_uow(Some(caller.ctx.clone()), move |uow| {
            uow.assets.find_by_id(&caller.ctx, id).map_err(ApiError::from)
        })
        .await?;
    match asset {
        Some(a) => Ok(Json(DataEnvelope { data: map_asset(&a) })),
        None => Err(ApiError::not_found()),
    }
}
