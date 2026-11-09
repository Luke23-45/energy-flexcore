// P1.17 — OpenAPI generation (utoipa) and the spec endpoint.
//
// The spec is GENERATED from the route annotations, never hand-written
// (ledger P1.17 acceptance: "the spec is generated, not hand-written").
use axum::response::IntoResponse;
use axum::Json;
use utoipa::OpenApi;

use crate::dto::{
    AssetDto, AssetLimitsDto, AssetViewDto, CommitmentsPlaceholderDto, CurrentStateDto,
    DataEnvelope, EdgeConnectivityDto, ListEnvelope, PortfolioCompositionDto, PortfolioDto,
    PortfolioViewDto, QuarantineEntryDto, QuantityStateDto, SiteDto, SiteOverviewDto, SiteViewDto,
    TelemetryPageEnvelope, TelemetryRecordDto, TenantDto,
};
use crate::dto_phase3::{
    CreateEnvelopeBody, CreateOptimizationRunBody, CreateScheduleBody, EnvelopePageEnvelope,
    EnvelopeRecordDto, OptimizationRunDto, OptimizationRunPageEnvelope, Phase3ListParams,
    ScheduleRevisionDto, ScheduleRevisionPageEnvelope,
};
use crate::error::ErrorEnvelope;
use crate::routes::{CreateAssetBody, CreatePortfolioBody, CreateSiteBody, CreateTenantBody};
use crate::routes_phase2::{
    DeactivateAssetBody, HistoricalAggregateEnvelope, HistoricalPageEnvelope, HistoricalRecordDto,
    HistoryBucketDto, TelemetryHistoryParams, TelemetryQueryParams, UpdateAssetBody,
};
use crate::routes_phase3::{
    create_flexibility_envelope, create_optimization_run, list_flexibility_envelopes,
    list_optimization_runs, list_schedules, propose_schedule,
};

#[derive(OpenApi)]
#[openapi(
    info(title = "FlexCore Control Plane API", version = "0.2.0",
        description = "Registry + telemetry read surface + the Phase 3 flexibility/scheduling \
                       records (subset of the P0.15 resource list). \
                       Phase 1 implemented tenants/portfolios/sites/assets create/list/get; \
                       Phase 2 (P2.12/P2.17) completes the registry CRUD (DELETE with the \
                       RESTRICT rule, asset update/deactivate), adds the telemetry query, \
                       current-state, and quarantine read routes, and the registry views. \
                       All Phase 2 telemetry is simulator-derived and marked simulated: true. \
                       Phase 3 (P3.28) exposes the flexibility-envelope, schedule, and \
                       optimization-run resources: every POST creates a CANDIDATE record \
                       with its outbox events in one transaction — there is NO execute-command \
                       endpoint, and ScheduleValidated means ASSET/TIME-COUPLED validation \
                       ONLY, never network feasibility (Phase 4)."),
    paths(
        crate::routes::create_tenant,
        crate::routes::list_tenants,
        crate::routes::get_tenant,
        crate::routes::create_portfolio,
        crate::routes::list_portfolios,
        crate::routes::get_portfolio,
        crate::routes::create_site,
        crate::routes::list_sites,
        crate::routes::get_site,
        crate::routes::create_asset,
        crate::routes::list_assets,
        crate::routes::get_asset,
        // Phase 2 (P2.12/P2.17)
        crate::routes_phase2::delete_tenant,
        crate::routes_phase2::delete_portfolio,
        crate::routes_phase2::delete_site,
        crate::routes_phase2::delete_asset,
        crate::routes_phase2::update_asset,
        crate::routes_phase2::deactivate_asset,
        crate::routes_phase2::query_telemetry,
        // Phase 2 P2.19 historical view (coordinator registration)
        crate::routes_phase2::query_telemetry_history,
        crate::routes_phase2::get_asset_state,
        crate::routes_phase2::list_quarantine,
        crate::routes_phase2::portfolio_view,
        crate::routes_phase2::site_view,
        crate::routes_phase2::asset_view,
        // Phase 3 (P3.28): the §15.2 flexibility-envelope, schedule, and
        // optimization-run resources of the Phase 3 line. NO execute-
        // command endpoint exists anywhere on the surface.
        crate::routes_phase3::create_flexibility_envelope,
        crate::routes_phase3::list_flexibility_envelopes,
        crate::routes_phase3::create_optimization_run,
        crate::routes_phase3::list_optimization_runs,
        crate::routes_phase3::propose_schedule,
        crate::routes_phase3::list_schedules,
    ),
    components(schemas(
        TenantDto, PortfolioDto, SiteDto, AssetDto,
        DataEnvelope<TenantDto>, DataEnvelope<PortfolioDto>,
        DataEnvelope<SiteDto>, DataEnvelope<AssetDto>,
        ListEnvelope<TenantDto>, ListEnvelope<PortfolioDto>,
        ListEnvelope<SiteDto>, ListEnvelope<AssetDto>,
        CreateTenantBody, CreatePortfolioBody, CreateSiteBody, CreateAssetBody,
        // Phase 2 schemas (P2.12/P2.17)
        UpdateAssetBody, DeactivateAssetBody, TelemetryQueryParams,
        TelemetryRecordDto, TelemetryPageEnvelope,
        // P2.19 historical view schemas (coordinator registration)
        TelemetryHistoryParams, HistoricalRecordDto, HistoricalPageEnvelope,
        HistoryBucketDto, HistoricalAggregateEnvelope,
        QuantityStateDto, CurrentStateDto, DataEnvelope<CurrentStateDto>,
        QuarantineEntryDto, ListEnvelope<QuarantineEntryDto>,
        PortfolioViewDto, PortfolioCompositionDto, CommitmentsPlaceholderDto,
        SiteViewDto, EdgeConnectivityDto, SiteOverviewDto,
        AssetViewDto, AssetLimitsDto,
        DataEnvelope<PortfolioViewDto>, DataEnvelope<SiteViewDto>, DataEnvelope<AssetViewDto>,
        // P3.28 schemas (flexibility envelopes, schedules, optimization
        // runs).
        CreateEnvelopeBody, EnvelopeRecordDto, EnvelopePageEnvelope,
        DataEnvelope<EnvelopeRecordDto>,
        CreateScheduleBody, ScheduleRevisionDto, ScheduleRevisionPageEnvelope,
        DataEnvelope<ScheduleRevisionDto>,
        CreateOptimizationRunBody, OptimizationRunDto, OptimizationRunPageEnvelope,
        DataEnvelope<OptimizationRunDto>,
        Phase3ListParams,
        ErrorEnvelope,
    ))
)]
pub struct Stage1ApiDoc;

/// Serves the generated OpenAPI 3.1 document at `/api/v1/openapi.json`.
pub async fn serve_openapi() -> impl IntoResponse {
    Json(Stage1ApiDoc::openapi())
}
