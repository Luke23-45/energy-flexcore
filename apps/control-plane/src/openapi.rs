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
// P5.31 — the alarm DTOs (the /api/v1/alarms derived-on-read resource).
use crate::dto_alarms::{AlarmDto, AlarmListParams, AlarmPageEnvelope, AlarmSourcesDto};
use crate::dto_phase3::{
    CreateEnvelopeBody, CreateOptimizationRunBody, CreateScheduleBody, EnvelopePageEnvelope,
    EnvelopeRecordDto, OptimizationRunDto, OptimizationRunPageEnvelope, Phase3ListParams,
    ScheduleRevisionDto, ScheduleRevisionPageEnvelope,
};
use crate::dto_phase4::{
    AuditEventDto, AuditEventListParams, AuditEventPageEnvelope, CommandTransitionDto,
    DispatchCommandDetailDto, DispatchCommandDto, DispatchCommandPageEnvelope,
    FeasibilityCheckDto, FeasibilityCheckPageEnvelope, Phase4ListParams,
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
use crate::routes_phase4::{
    get_audit_event, get_dispatch_command, get_feasibility_check, list_audit_events,
    list_dispatch_commands, list_feasibility_checks,
};
// P5.25 — the live-update SSE stream.
use crate::routes_live::{live_events, LiveEventEnvelope, LiveResyncNotice};
// P5.31 — the alarms read resource.
use crate::routes_alarms::list_alarms;
// P6.06 — the edge enrollment seam. P6.25/P6.32 appended the edge-status
// read and the operator decommission additively (same additive-path rule).
use crate::dto_edge::{
    CommandPathStateDto, DataFreshnessDto, DecommissionRequestBody, DecommissionResponseDto,
    EdgeConnectivityReportDto, EdgeStatusDto, EdgeStatusParams, EnrollRequestBody,
    EnrollResponseDto,
};
use crate::routes_edge::{decommission_edge_device, enroll_edge_device, get_edge_status};
// P7.25 — the plugin-inventory read resource (the operator console's
// plugin seam, §20.1 item 10; READ-only).
use crate::dto_plugins::{
    ConfigPropertyDto, PluginHealthDto, PluginInventoryDto, PluginRecordDto, ProfileRecordDto,
};
use crate::routes_plugins::get_plugin_inventory;
// P8.27 — the research-status read resource (the operator console's
// research surfacing seam, §20.1 item 9; READ-only).
use crate::dto_research::{
    ResearchModuleDto, ResearchProposalDto, ResearchStatusDto, ShadowComparisonDto,
};
use crate::routes_research::get_research_status;

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
                       ONLY, never network feasibility (Phase 4). \
                       Phase 4 (P4.30) exposes the dispatch-command and feasibility-check \
                       READ resources: persisted §14.1/§12.5 evidence with tenant-scoped \
                       authorization. Both resources are READ-ONLY — there is NO \
                       execute-command endpoint (§15.2); the lifecycle transitions happen \
                       exclusively through the application-layer gates. \
                       P4.36 (Phase 4) exposes the /api/v1/audit-events READ resource: the \
                       §10.1 audit trail (migration 0008) served tenant-scoped with every \
                       decision/evidence/before-after reference preserved end to end. The \
                       trail is append-only — there is NO update, delete, or export/dump \
                       endpoint (§15.2, §16.2); corrections are new records referencing \
                       the original (P1.10). \
                       P5.25 (Phase 5) exposes GET /api/v1/live: a tenant-scoped \
                       text/event-stream of the frozen §15.3 events, correlated with the \
                       audit records by the events' correlation ids (§21.3). Delivery is \
                       at-least-once and UNORDERED (§15.4): events may be duplicated or \
                       reordered; a lagged stream emits RESYNC_REQUIRED instead of \
                       silently gap-filling, and the client re-syncs from the REST read \
                       APIs. The stream is READ-only — there is no write or execute path \
                       anywhere on it. \
                       P5.31 (Phase 5) exposes GET /api/v1/alarms: the minimal alarm model \
                       (ALARM-MODEL-0.1.0) derived ON READ from existing platform facts — \
                       telemetry quality states, dispatch outcomes, feasibility verdicts. \
                       Alarms are operator-attention records, NOT protection signals; there \
                       is NO acknowledge/create/delete endpoint (no alarm mutation path in \
                       this phase), no alarm persistence (cleared alarms cannot be served, \
                       active_only=false is refused), and the frozen §15.3 event vocabulary \
                       is not extended (no alarm event exists)."),
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
        // Phase 4 (P4.30): the §15.2 dispatch-command and
        // feasibility-check resources of the Phase 4 line. READ-ONLY —
        // no execute-command endpoint exists anywhere on the surface.
        crate::routes_phase4::list_dispatch_commands,
        crate::routes_phase4::get_dispatch_command,
        crate::routes_phase4::list_feasibility_checks,
        crate::routes_phase4::get_feasibility_check,
        // Phase 4 (P4.36): the §15.2 audit-events resource of the Phase 4
        // line. READ-ONLY — the append-only trail has no update, delete,
        // or export/dump endpoint anywhere on the surface.
        crate::routes_phase4::list_audit_events,
        crate::routes_phase4::get_audit_event,
        // Phase 5 (P5.25): the live-update SSE stream. READ-only.
        crate::routes_live::live_events,
        // Phase 5 (P5.31): the derived alarm read resource. READ-only —
        // alarms are derived on read from existing platform facts
        // (telemetry quality states, dispatch outcomes, feasibility
        // verdicts); there is NO acknowledge/create/delete endpoint (no
        // alarm mutation path exists in this phase), and the frozen §15.3
        // event vocabulary is not extended (no alarm event exists).
        crate::routes_alarms::list_alarms,
        // Phase 6 (P6.06): the edge enrollment seam. An ADDITIVE path per
        // the recorded P4.36/P5.31 rule (not one of the 16 frozen §15.2
        // candidates; no dispatch-state mutation). Provisioning-token
        // auth — NO certificate is issued (issuance and OCSP/CRL are
        // deployment requirements; decision record
        // 06_07_identity_channel.md).
        crate::routes_edge::enroll_edge_device,
        // Phase 6 (P6.25): the per-edge status read. An ADDITIVE path per
        // the recorded P4.36/P5.31 rule; a POLLING read (no live SSE),
        // tenant-scoped, all operational fields honest gaps when no
        // edge-status reporter is composed. READ-only — there is NO
        // endpoint anywhere on this surface that could enable the
        // command path (decision record
        // 25_31_32_console_clock_decommission.md).
        crate::routes_edge::get_edge_status,
        // Phase 6 (P6.32): the operator-initiated edge decommission
        // (RBAC + step-up confirmation binding; append-only REVOKED term
        // + the confirmation record in the enrollment journal).
        crate::routes_edge::decommission_edge_device,
        // Phase 7 (P7.25): the plugin-inventory read. An ADDITIVE path per
        // the recorded P4.36/P5.31/P6.06 rule; a POLLING read, tenant-
        // scoped via the recorded-drift interim gate (the swap point for
        // `plugin.inventory.read`). READ-only — there is NO endpoint
        // anywhere on this surface that could activate, deactivate,
        // upgrade, or switch anything (composition changes take effect
        // ONLY through the reviewed profile-file edit + re-composition
        // procedure, P7.16). Absent runtime facts render as explicit gaps.
        crate::routes_plugins::get_plugin_inventory,
        // Phase 8 (P8.27): the research-status read. An ADDITIVE path per
        // the recorded P4.36/P5.31/P6.06/P7.25 rule; a POLLING read,
        // tenant-scoped via the recorded-drift interim gate (the swap
        // point for `research.status.read`). READ-only — there is NO
        // endpoint anywhere on this surface that could promote, issue,
        // authorize, fast-track, or approve a research proposal (a
        // promotion requires the protected validation → feasibility →
        // authorization gates and, for any field use, per-feature
        // qualification — ADR-010/P8.29). Absent run facts render as
        // explicit gaps (TEMPLATE_EMPTY / NO_RUN / zero proposals).
        crate::routes_research::get_research_status,
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
        // P4.30 schemas (dispatch commands, feasibility checks).
        DispatchCommandDto, CommandTransitionDto, DispatchCommandDetailDto,
        DispatchCommandPageEnvelope,
        DataEnvelope<DispatchCommandDetailDto>,
        FeasibilityCheckDto, FeasibilityCheckPageEnvelope,
        DataEnvelope<FeasibilityCheckDto>,
        Phase4ListParams,
        // P4.36 schemas (audit events).
        AuditEventDto, AuditEventPageEnvelope,
        DataEnvelope<AuditEventDto>,
        AuditEventListParams,
        // P5.25 schemas (the SSE live stream: the per-event data envelope
        // and the lag/resync notice).
        LiveEventEnvelope, LiveResyncNotice,
        // P5.31 schemas (the derived alarm read resource).
        AlarmDto, AlarmSourcesDto, AlarmPageEnvelope, AlarmListParams,
        // P6.06 schemas (the edge enrollment seam).
        EnrollRequestBody, EnrollResponseDto, DataEnvelope<EnrollResponseDto>,
        // P6.25/P6.32 schemas (the edge-status read and the decommission
        // surface).
        EdgeStatusParams, EdgeConnectivityReportDto, DataFreshnessDto, CommandPathStateDto,
        EdgeStatusDto, DataEnvelope<EdgeStatusDto>,
        DecommissionRequestBody, DecommissionResponseDto, DataEnvelope<DecommissionResponseDto>,
        // P7.25 schemas (the plugin-inventory read resource).
        ConfigPropertyDto, PluginRecordDto, PluginHealthDto, ProfileRecordDto,
        PluginInventoryDto, DataEnvelope<PluginInventoryDto>,
        // P8.27 schemas (the research-status read resource).
        ResearchModuleDto, ResearchProposalDto, ShadowComparisonDto,
        ResearchStatusDto, DataEnvelope<ResearchStatusDto>,
        ErrorEnvelope,
    ))
)]
pub struct Stage1ApiDoc;

/// Serves the generated OpenAPI 3.1 document at `/api/v1/openapi.json`.
pub async fn serve_openapi() -> impl IntoResponse {
    Json(Stage1ApiDoc::openapi())
}
