// P1.17 — API DTOs: the response shapes for the Stage 1 registry resources.
//
// Domain entities are not serializable (the domain crate has no serde —
// §6.2); the API layer maps entities onto these explicit DTOs. Canonical
// units are preserved verbatim (MW/MWh as numbers in canonical fields);
// there are no kW/kWh fields anywhere (P0.07 — display is the console's
// concern, and there is no console yet).
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::entities::AssetType;
use serde::Serialize;
use utoipa::ToSchema;

/// The canonical snake_case id of an asset type (the frozen P0.10 §2
/// vocabulary; backup generation is deliberately absent).
pub fn asset_type_id(t: AssetType) -> &'static str {
    match t {
        AssetType::Bess => "bess",
        AssetType::PvInverter => "pv_inverter",
        AssetType::FlexibleCiLoad => "flexible_ci_load",
        AssetType::EvseEvFleet => "evse_ev_fleet",
        AssetType::SiteMeter => "site_meter",
        AssetType::WindGeneration => "wind_generation",
        AssetType::ThermalStorageHeatPump => "thermal_storage_heat_pump",
    }
}

/// Parses the canonical snake_case asset-type id. Unknown ids are a
/// validation rejection, never a default (P0.10 vocabulary is closed).
pub fn parse_asset_type(id: &str) -> Option<AssetType> {
    match id {
        "bess" => Some(AssetType::Bess),
        "pv_inverter" => Some(AssetType::PvInverter),
        "flexible_ci_load" => Some(AssetType::FlexibleCiLoad),
        "evse_ev_fleet" => Some(AssetType::EvseEvFleet),
        "site_meter" => Some(AssetType::SiteMeter),
        "wind_generation" => Some(AssetType::WindGeneration),
        "thermal_storage_heat_pump" => Some(AssetType::ThermalStorageHeatPump),
        _ => None,
    }
}

/// RFC 3339 rendering of a canonical UTC event time.
pub fn rfc3339(t: EventTime) -> String {
    t.timestamp()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<invalid-timestamp>".to_string())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TenantDto {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub identity_provider_reference: String,
    pub policy_profile: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioDto {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub operator: String,
    pub market_region: String,
    pub aggregation_rules: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SiteDto {
    pub id: String,
    pub portfolio_id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub location: String,
    pub timezone: String,
    pub point_of_common_coupling: String,
    pub grid_model_reference: String,
    pub edge_identity: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssetDto {
    pub id: String,
    pub site_id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub asset_type: String,
    pub vendor_model: String,
    /// Rated active power, canonical MW (P0.07).
    pub rated_power_mw: f64,
    /// Rated energy, canonical MWh, when applicable (storage assets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rated_energy_mwh: Option<f64>,
    pub phase: String,
    pub bus_reference: String,
    pub capability_profile: String,
    /// The stored registry lifecycle (P2.03): `active` | `deactivated`.
    pub lifecycle_status: String,
    /// The persisted deactivation reason, when the asset was deactivated
    /// (P2.03: the reason survives with the state it explains).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivation_reason: Option<String>,
    /// The deactivation timestamp (RFC 3339), when deactivated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivated_at: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Phase 2 telemetry DTOs (P2.12/P2.17/P2.18)
// ---------------------------------------------------------------------------

/// The canonical SCREAMING_SNAKE_CASE id of a quality state (the frozen
/// P0.08 vocabulary; matches the serde rendering of the contract enum).
pub fn quality_id(q: flexcore_contracts::quality::TelemetryQuality) -> &'static str {
    use flexcore_contracts::quality::TelemetryQuality as Q;
    match q {
        Q::Good => "GOOD",
        Q::Suspect => "SUSPECT",
        Q::Bad => "BAD",
        Q::Missing => "MISSING",
        Q::Estimated => "ESTIMATED",
        Q::Stale => "STALE",
    }
}

/// Parses a canonical quality id. ONLY the six canonical states parse; any
/// seventh value is `None` and the API layer rejects it with 422 (P2.12
/// step 3). Exact, case-sensitive match on the frozen vocabulary.
pub fn parse_quality(id: &str) -> Option<flexcore_contracts::quality::TelemetryQuality> {
    use flexcore_contracts::quality::TelemetryQuality as Q;
    match id {
        "GOOD" => Some(Q::Good),
        "SUSPECT" => Some(Q::Suspect),
        "BAD" => Some(Q::Bad),
        "MISSING" => Some(Q::Missing),
        "ESTIMATED" => Some(Q::Estimated),
        "STALE" => Some(Q::Stale),
        _ => None,
    }
}

/// The current value + quality of one quantity of one asset (P2.18 step 4
/// response shape: value + unit + quality + event_time + ingest_time + age
/// + `simulated` flag). Exactly one of `value_num`/`value_bool`/`value_text`
/// is present; a `MISSING` current state carries none of them (P2.18: a
/// missing value is never presented as zero).
#[derive(Debug, Serialize, ToSchema)]
pub struct QuantityStateDto {
    pub quantity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_num: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<String>,
    /// The canonical unit of the value (P2.04 §5: `MW`, `MVAr`, `MVA`,
    /// `MWh`, `1`); `1` is the dimensionless unit.
    pub unit: String,
    /// Exactly one of the six canonical states (P0.08).
    pub quality: String,
    /// The declared reactive-sign convention, set exactly when
    /// `quantity == "reactive_power"` (P0.07 §3; P2.04 D-T4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q_sign_convention: Option<String>,
    pub event_time: String,
    pub ingest_time: String,
    /// Age of the value in milliseconds: `now - event_time` (>= 0).
    pub age_ms: i64,
    /// True exactly for simulator-derived values. In Phase 2 every stored
    /// telemetry value is simulator-derived (the provider set is exactly
    /// one — P2.11), so every value here carries `simulated: true`; the
    /// flag is derived from the record's source identity (P2.04 provenance
    /// note) so the rule generalises when real providers arrive.
    pub simulated: bool,
}

/// The P2.18 current-state response: the latest record per quantity of one
/// asset, latest by `event_time` (never latest arrival).
#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentStateDto {
    pub asset_id: String,
    /// The instant the view was generated (RFC 3339).
    pub generated_at: String,
    pub quantities: Vec<QuantityStateDto>,
    /// True: every Phase 2 value is simulator-derived.
    pub simulated: bool,
}

/// One stored telemetry record as returned by the P2.12 query endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetryRecordDto {
    pub asset_id: String,
    pub quantity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_num: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_bool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<String>,
    pub unit: String,
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q_sign_convention: Option<String>,
    pub event_time: String,
    pub ingest_time: String,
    pub source_id: String,
    pub source_sequence: u64,
    /// True exactly for simulator-derived values (see [`QuantityStateDto`]).
    pub simulated: bool,
}

/// The wrapped telemetry query response: one page plus the keyset cursor
/// to continue from (opaque; absent on the last page).
#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetryPageEnvelope {
    pub data: Vec<TelemetryRecordDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One quarantined telemetry record (P2.07 §3): the evidence trail of a
/// rejected ingestion. Admin-only surface (P2.07 decision).
#[derive(Debug, Serialize, ToSchema)]
pub struct QuarantineEntryDto {
    pub raw_payload: String,
    pub failure_reason: String,
    pub source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_time: Option<String>,
    pub quarantined_at: String,
    pub attempt_count: u32,
}

// ---------------------------------------------------------------------------
// Phase 2 registry views (P2.17)
// ---------------------------------------------------------------------------

/// The commitments placeholder of the portfolio view (P2.17 step 1:
/// "commitments placeholder" — populated in a later phase, honestly stated
/// here instead of a fabricated empty list).
#[derive(Debug, Serialize, ToSchema)]
pub struct CommitmentsPlaceholderDto {
    pub populated: bool,
    pub note: String,
}

/// The composition summary of the portfolio view.
#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioCompositionDto {
    pub site_count: usize,
    pub asset_count: usize,
}

/// The P2.17 portfolio view.
#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioViewDto {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub operator: String,
    pub market_region: String,
    pub aggregation_rules: String,
    pub composition: PortfolioCompositionDto,
    pub commitments: CommitmentsPlaceholderDto,
    pub created_at: String,
}

/// The P2.17 edge-connectivity report — HONEST BY CONTRACT (P2.17
/// acceptance): Phase 2 has no site integration (first site integration is
/// v0.4.0), so the status is `simulator` when the site carries an edge
/// identity (the only Phase 2 provider set is the simulator, P2.11) and
/// `not_connected` otherwise. `live` is always `false` in Phase 2; the API
/// never presents a simulated edge as live.
#[derive(Debug, Serialize, ToSchema)]
pub struct EdgeConnectivityDto {
    /// `simulator` | `not_connected`.
    pub status: String,
    pub edge_identity: String,
    pub live: bool,
}

/// The overview summary of the site view.
#[derive(Debug, Serialize, ToSchema)]
pub struct SiteOverviewDto {
    pub asset_count: usize,
}

/// The P2.17 site view.
#[derive(Debug, Serialize, ToSchema)]
pub struct SiteViewDto {
    pub id: String,
    pub portfolio_id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub location: String,
    pub timezone: String,
    pub point_of_common_coupling: String,
    pub grid_model_reference: String,
    pub edge_connectivity: EdgeConnectivityDto,
    pub overview: SiteOverviewDto,
    pub created_at: String,
}

/// The stored limits of the asset view (the rated magnitudes the registry
/// holds — not runtime capability envelopes, which are Phase 3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AssetLimitsDto {
    pub rated_power_mw: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rated_energy_mwh: Option<f64>,
}

/// The P2.17 asset view. `maintenance_outage_status` is the stored registry
/// lifecycle field (P2.17 step 1: "a stored field, populated by the
/// registry, not by a live system"): `operational` while the asset is
/// `active`, `deactivated` once it is — richer maintenance windows are a
/// later-phase field (recorded assumption).
#[derive(Debug, Serialize, ToSchema)]
pub struct AssetViewDto {
    pub id: String,
    pub site_id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub asset_type: String,
    pub vendor_model: String,
    pub capability_profile: String,
    pub limits: AssetLimitsDto,
    /// `active` | `deactivated` (the stored lifecycle, P2.03).
    pub lifecycle_status: String,
    /// `operational` | `deactivated` (the stored maintenance/outage status).
    pub maintenance_outage_status: String,
    /// The latest telemetry per quantity, when the telemetry read port is
    /// wired (P2.18 shape; every value carries quality + `simulated: true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_state: Option<CurrentStateDto>,
    pub created_at: String,
}

/// The wrapped response envelope for single resources: `{"data": ...}`.
#[derive(Debug, Serialize, ToSchema)]
pub struct DataEnvelope<T: ToSchema> {
    pub data: T,
}

/// The wrapped response envelope for collections: `{"data": [...]}`.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListEnvelope<T: ToSchema> {
    pub data: Vec<T>,
}
