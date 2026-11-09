// P1.06 — §10.1 domain entities for the Stage 1 workflow (report §10.1;
//         P1.26 exit-gate scope).
//
// Implements the Stage 1 subset of the fourteen §10.1 canonical entities as
// **vocabulary-faithful domain types, not database models** (P0.10 §1: "this
// document is vocabulary, not a database schema"). Only the five entities the
// Stage 1 workflow needs are modelled here — Tenant, Portfolio, Site, Asset,
// AssetState — with exactly the key fields listed in the P0.10 §10.1 table;
// no field is invented beyond it (ledger P0.10 risk note). The remaining
// §10.1 entities (CapabilityEnvelope, GridModel, ForecastBundle, Schedule,
// DispatchExecution, MarketCommitment, PluginInstance, AuditRecord) are later
// subtasks; DispatchCommand is `dispatch_command.rs` in this crate.
//
// Rules encoded here:
//
// - **Typed identifiers**: every entity ID is one of the `ids` newtypes over
//   `flexcore_contracts::FlexUuid`; a bare UUID can never stand in for a
//   tenant, portfolio, site, or asset.
// - **Tenant scoping (P0.16 rule 1)**: `tenant_id` is carried on every
//   tenant-owned entity — Tenant itself excepted. For child entities the
//   tenant is **derived from the parent entity passed to the constructor**
//   (Portfolio requires a Tenant, Site requires a Portfolio, Asset requires a
//   Site), so the ownership chain cannot be assembled inconsistently: a Site
//   cannot claim a different tenant than its Portfolio, and an Asset cannot
//   claim a different tenant than its Site. Constructing a child without its
//   parent is a compile error (the constructor takes the parent by
//   reference); a nil parent ID anywhere in the chain is a construction
//   error (see the negative-test comments in the tests module).
// - **Canonical units (P0.07)**: rated power is `ActivePowerMW` and rated
//   energy is `EnergyMWh` — never a bare `f64`. Rated quantities are
//   magnitudes and must be strictly positive.
// - **Time (P0.09)**: entity creation timestamps use the contracts UTC
//   timestamp types; AssetState carries the frozen `event_time` / `ingest_time`
//   distinction and enforces `ingest_time ≥ event_time` at construction.
// - **AssetState (P0.10 §1 row 5)**: operational state (SOC, active/reactive
//   power, availability, operating mode, health) plus data quality/freshness.
//   Quality states come from `flexcore_contracts::quality` (exactly one of
//   the six canonical states per value; the non-silencing rule is upstream,
//   this type never transforms a quality flag). A reactive-power value is
//   carried **only together with the declared `ReactiveSignConvention`**
//   (P0.07: Q sign is declared per grid model and device profile, never
//   assumed, never inferred from sign alone).
// - **No validation-skipping constructors**: no `Default` impls, no public
//   fields; construction goes through `try_new`, which validates and returns
//   `Result`. No panic path on user input.
//
// Scope and control status: registry vocabulary and invariants for the design
// baseline only. The entire 0.x line is simulation-only; nothing here
// authorises field control or device writes. FlexCore is a supervisory
// orchestration layer and performs no protection-grade functions.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{
    EventTime, IngestTime, SiteTimezone, TimeModelError, validate_ingest_after_event,
};
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};

use crate::ids::{AssetId, PortfolioId, SiteId, TenantId};

/// Errors returned by §10.1 entity construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityError {
    /// A required identifier was the nil UUID. Entity identity must be a
    /// real, generated identifier (generation is an application-layer port
    /// concern; see the contracts `ids` module).
    NilIdentifier {
        /// Which identifier was nil.
        field: &'static str,
    },
    /// A required descriptive/reference field was empty or all whitespace.
    EmptyField {
        /// Which field was empty.
        field: &'static str,
    },
    /// A rated quantity (power or energy) was zero or negative; rated
    /// quantities are magnitudes and must be strictly positive.
    NonPositiveQuantity {
        /// Which quantity was non-positive.
        field: &'static str,
    },
    /// A state timestamp violated the frozen time rules (for example
    /// `ingest_time` before `event_time`; P0.09).
    Time(TimeModelError),
    /// A reactive-power value was supplied without its declared sign
    /// convention, or a convention without a value (P0.07: the pair is
    /// inseparable).
    ReactiveConventionMismatch,
    /// A lifecycle transition was attempted on an asset that is already
    /// deactivated. Deactivation is terminal in Phase 2 (P2.02 deletion /
    /// lifecycle rule): there is no reactivation capability, so a second
    /// deactivation is a state error, not a no-op.
    AlreadyDeactivated,
}

impl std::fmt::Display for EntityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityError::NilIdentifier { field } => {
                write!(f, "identifier `{field}` must not be the nil UUID")
            }
            EntityError::EmptyField { field } => {
                write!(f, "field `{field}` must not be empty")
            }
            EntityError::NonPositiveQuantity { field } => {
                write!(f, "rated quantity `{field}` must be strictly positive")
            }
            EntityError::Time(e) => write!(f, "time rule violated: {e}"),
            EntityError::ReactiveConventionMismatch => write!(
                f,
                "a reactive-power value requires its declared sign convention (and vice versa)"
            ),
            EntityError::AlreadyDeactivated => {
                write!(f, "the asset is already deactivated (deactivation is terminal)")
            }
        }
    }
}

impl std::error::Error for EntityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EntityError::Time(e) => Some(e),
            _ => None,
        }
    }
}

fn require_non_nil(uuid: FlexUuid, field: &'static str) -> Result<(), EntityError> {
    if uuid == FlexUuid::NIL {
        Err(EntityError::NilIdentifier { field })
    } else {
        Ok(())
    }
}

fn require_non_empty(value: &str, field: &'static str) -> Result<(), EntityError> {
    if value.trim().is_empty() {
        Err(EntityError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn require_positive(value: f64, field: &'static str) -> Result<(), EntityError> {
    if value <= 0.0 {
        Err(EntityError::NonPositiveQuantity { field })
    } else {
        Ok(())
    }
}

// --- Tenant -----------------------------------------------------------------

/// **Tenant** (P0.10 §1 row 1): `tenant_id`, name, identity provider
/// reference, policy profile. The root of the tenancy chain; the one
/// §10.1 entity that carries no `tenant_id` of its own.
///
/// Note: `description` is not in the P0.10 key-field list; it is added here
/// as an optional descriptive field per the P1.06 task ("name/description
/// fields on every entity") and carries no contract semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    tenant_id: TenantId,
    name: String,
    description: Option<String>,
    identity_provider_reference: String,
    policy_profile: String,
    created_at: EventTime,
}

impl Tenant {
    /// Constructs a tenant, rejecting nil identifiers and empty required
    /// fields.
    pub fn try_new(
        tenant_id: TenantId,
        name: String,
        description: Option<String>,
        identity_provider_reference: String,
        policy_profile: String,
        created_at: EventTime,
    ) -> Result<Self, EntityError> {
        require_non_nil(tenant_id.uuid(), "tenant_id")?;
        require_non_empty(&name, "name")?;
        if let Some(d) = &description {
            require_non_empty(d, "description")?;
        }
        require_non_empty(&identity_provider_reference, "identity_provider_reference")?;
        require_non_empty(&policy_profile, "policy_profile")?;
        Ok(Self {
            tenant_id,
            name,
            description,
            identity_provider_reference,
            policy_profile,
            created_at,
        })
    }

    /// The tenant identifier.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The tenant name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The optional description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The identity provider reference (P0.10 §1 row 1).
    pub fn identity_provider_reference(&self) -> &str {
        &self.identity_provider_reference
    }

    /// The policy profile (P0.10 §1 row 1).
    pub fn policy_profile(&self) -> &str {
        &self.policy_profile
    }

    /// When the tenant record was created (UTC; P0.09).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

// --- Portfolio ---------------------------------------------------------------

/// **Portfolio** (P0.10 §1 row 2): `portfolio_id`, tenant, operator, market
/// region, aggregation rules. Requires its owning [`Tenant`] — the
/// constructor takes the parent entity and derives `tenant_id` from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portfolio {
    portfolio_id: PortfolioId,
    tenant_id: TenantId,
    name: String,
    description: Option<String>,
    operator: String,
    market_region: String,
    aggregation_rules: String,
    created_at: EventTime,
}

impl Portfolio {
    /// Constructs a portfolio under the given tenant. The `tenant_id` is
    /// derived from `tenant`, so a portfolio can never be attached to a
    /// different tenant than the one it was constructed with.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        tenant: &Tenant,
        portfolio_id: PortfolioId,
        name: String,
        description: Option<String>,
        operator: String,
        market_region: String,
        aggregation_rules: String,
        created_at: EventTime,
    ) -> Result<Self, EntityError> {
        require_non_nil(portfolio_id.uuid(), "portfolio_id")?;
        require_non_empty(&name, "name")?;
        if let Some(d) = &description {
            require_non_empty(d, "description")?;
        }
        require_non_empty(&operator, "operator")?;
        require_non_empty(&market_region, "market_region")?;
        require_non_empty(&aggregation_rules, "aggregation_rules")?;
        Ok(Self {
            portfolio_id,
            tenant_id: tenant.tenant_id(),
            name,
            description,
            operator,
            market_region,
            aggregation_rules,
            created_at,
        })
    }

    /// The portfolio identifier.
    pub fn portfolio_id(&self) -> PortfolioId {
        self.portfolio_id
    }

    /// The owning tenant (derived from the parent at construction; P0.16
    /// rule 1).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The portfolio name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The optional description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The operator (P0.10 §1 row 2).
    pub fn operator(&self) -> &str {
        &self.operator
    }

    /// The market region (P0.10 §1 row 2).
    pub fn market_region(&self) -> &str {
        &self.market_region
    }

    /// The aggregation rules reference (P0.10 §1 row 2; the constrained-
    /// aggregation rule itself is P0.10 §5.5, Phase 3 implementation).
    pub fn aggregation_rules(&self) -> &str {
        &self.aggregation_rules
    }

    /// When the portfolio record was created (UTC; P0.09).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

// --- Site --------------------------------------------------------------------

/// **Site** (P0.10 §1 row 3): `site_id`, portfolio, location, timezone, PCC,
/// grid model reference, edge identity. Requires its owning [`Portfolio`]
/// (which in turn requires its [`Tenant`]); `tenant_id` is derived from the
/// portfolio, so the whole chain carries one consistent tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    site_id: SiteId,
    portfolio_id: PortfolioId,
    tenant_id: TenantId,
    name: String,
    description: Option<String>,
    location: String,
    timezone: SiteTimezone,
    point_of_common_coupling: String,
    grid_model_reference: String,
    edge_identity: String,
    created_at: EventTime,
}

impl Site {
    /// Constructs a site under the given portfolio. `tenant_id` is derived
    /// from the portfolio (P0.16 rule 1: `tenant_id` on every tenant-owned
    /// record; P0.16 rule 7: site edge identities are tenant-scoped).
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        portfolio: &Portfolio,
        site_id: SiteId,
        name: String,
        description: Option<String>,
        location: String,
        timezone: SiteTimezone,
        point_of_common_coupling: String,
        grid_model_reference: String,
        edge_identity: String,
        created_at: EventTime,
    ) -> Result<Self, EntityError> {
        require_non_nil(site_id.uuid(), "site_id")?;
        require_non_empty(&name, "name")?;
        if let Some(d) = &description {
            require_non_empty(d, "description")?;
        }
        require_non_empty(&location, "location")?;
        require_non_empty(&point_of_common_coupling, "point_of_common_coupling")?;
        require_non_empty(&grid_model_reference, "grid_model_reference")?;
        require_non_empty(&edge_identity, "edge_identity")?;
        Ok(Self {
            site_id,
            portfolio_id: portfolio.portfolio_id(),
            tenant_id: portfolio.tenant_id(),
            name,
            description,
            location,
            timezone,
            point_of_common_coupling,
            grid_model_reference,
            edge_identity,
            created_at,
        })
    }

    /// The site identifier.
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// The owning portfolio (derived from the parent at construction).
    pub fn portfolio_id(&self) -> PortfolioId {
        self.portfolio_id
    }

    /// The owning tenant (derived through the portfolio at construction;
    /// P0.16 rule 1).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The site name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The optional description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The location (P0.10 §1 row 3).
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The site timezone (metadata; the canonical timestamps are UTC —
    /// P0.09 / contracts `time_model`).
    pub fn timezone(&self) -> &SiteTimezone {
        &self.timezone
    }

    /// The point of common coupling reference (P0.10 §1 row 3 "PCC").
    pub fn point_of_common_coupling(&self) -> &str {
        &self.point_of_common_coupling
    }

    /// The grid model reference (P0.10 §1 row 3).
    pub fn grid_model_reference(&self) -> &str {
        &self.grid_model_reference
    }

    /// The edge identity (P0.10 §1 row 3; tenant-scoped per P0.16 rule 7).
    pub fn edge_identity(&self) -> &str {
        &self.edge_identity
    }

    /// When the site record was created (UTC; P0.09).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

// --- Asset -------------------------------------------------------------------

/// The asset types of the §2.2 initial-support table (P0.10 §2). Backup
/// generation is **deliberately absent**: P0.10 §2 marks it "not a
/// first-class 0.x target" — it may only be added with emissions, fuel,
/// permit, and operating policy support, so it is not part of the frozen
/// Stage 1 vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetType {
    /// Battery energy storage system (P0.10 §2 row 1).
    Bess,
    /// PV inverter (P0.10 §2 row 2).
    PvInverter,
    /// Flexible C&I load (P0.10 §2 row 3).
    FlexibleCiLoad,
    /// EVSE / EV fleet (P0.10 §2 row 4).
    EvseEvFleet,
    /// Site meter (P0.10 §2 row 5).
    SiteMeter,
    /// Wind generation (P0.10 §2 row 6).
    WindGeneration,
    /// Thermal storage / heat pumps (P0.10 §2 row 7).
    ThermalStorageHeatPump,
}

/// The registry lifecycle status of an [`Asset`] (P2.02 deletion/lifecycle
/// rule). Phase 2 has exactly two states: an asset is registered **Active**
/// and may be **Deactivated** (with a recorded reason) by the
/// `DeactivateAsset` use case (P2.03). Deactivation is terminal in Phase 2 —
/// there is no reactivation capability and none may be invented here; a
/// decommissioned asset's history stays queryable through its telemetry, and
/// hard deletion follows the P2.02 RESTRICT deletion rule (see
/// [`crate::registry_rules`]), not this lifecycle.
///
/// This is registry lifecycle, not the operational `available` flag on
/// [`AssetState`]: `available` is telemetry-derived operational state, while
/// `AssetLifecycleStatus` is a registry fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetLifecycleStatus {
    /// Registered and in scope for coordination (simulation-only in 0.x).
    Active,
    /// Deactivated with a recorded reason; no longer in scope for
    /// coordination. Terminal in Phase 2.
    Deactivated,
}

/// **Asset** (P0.10 §1 row 4): `asset_id`, site, type, vendor/model, rated
/// power/energy, phase, bus reference, capability profile. Requires its
/// owning [`Site`]; `tenant_id` is derived from the site.
///
/// P2.03 additions (registry refinements, P2.02): the lifecycle status
/// ([`AssetLifecycleStatus`]) with its terminal deactivation transition
/// ([`Asset::deactivate`]) and the metadata/capability-reference update
/// ([`Asset::update_capability_metadata`]). Every other field is the P1.06
/// Stage 1 vocabulary, unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    asset_id: AssetId,
    site_id: SiteId,
    tenant_id: TenantId,
    name: String,
    description: Option<String>,
    asset_type: AssetType,
    vendor_model: String,
    rated_power_mw: ActivePowerMW,
    rated_energy_mwh: Option<EnergyMWh>,
    phase: String,
    bus_reference: String,
    capability_profile: String,
    lifecycle_status: AssetLifecycleStatus,
    deactivation_reason: Option<String>,
    deactivated_at: Option<EventTime>,
    created_at: EventTime,
}

impl Asset {
    /// Constructs an asset under the given site. Rated power must be a
    /// strictly positive MW magnitude; rated energy, when present, must be a
    /// strictly positive MWh magnitude (P0.07 canonical units, P0.07
    /// validation).
    ///
    /// `phase` is carried as a validated non-empty reference string: the
    /// P0.10 vocabulary lists `phase` as a key field but does not freeze a
    /// phase enumeration, so none is invented here (P0.10 §1 risk note).
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        site: &Site,
        asset_id: AssetId,
        name: String,
        description: Option<String>,
        asset_type: AssetType,
        vendor_model: String,
        rated_power_mw: ActivePowerMW,
        rated_energy_mwh: Option<EnergyMWh>,
        phase: String,
        bus_reference: String,
        capability_profile: String,
        created_at: EventTime,
    ) -> Result<Self, EntityError> {
        require_non_nil(asset_id.uuid(), "asset_id")?;
        require_non_empty(&name, "name")?;
        if let Some(d) = &description {
            require_non_empty(d, "description")?;
        }
        require_non_empty(&vendor_model, "vendor_model")?;
        require_positive(rated_power_mw.value(), "rated_power_mw")?;
        if let Some(e) = &rated_energy_mwh {
            require_positive(e.value(), "rated_energy_mwh")?;
        }
        require_non_empty(&phase, "phase")?;
        require_non_empty(&bus_reference, "bus_reference")?;
        require_non_empty(&capability_profile, "capability_profile")?;
        Ok(Self {
            asset_id,
            site_id: site.site_id(),
            tenant_id: site.tenant_id(),
            name,
            description,
            asset_type,
            vendor_model,
            rated_power_mw,
            rated_energy_mwh,
            phase,
            bus_reference,
            capability_profile,
            lifecycle_status: AssetLifecycleStatus::Active,
            deactivation_reason: None,
            deactivated_at: None,
            created_at,
        })
    }

    /// The asset identifier.
    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// The owning site (derived from the parent at construction).
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// The owning tenant (derived through the site at construction; P0.16
    /// rule 1).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The asset name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The optional description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The asset type (P0.10 §2 vocabulary).
    pub fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    /// The vendor/model string (P0.10 §1 row 4).
    pub fn vendor_model(&self) -> &str {
        &self.vendor_model
    }

    /// The rated power in MW (canonical unit; strictly positive by
    /// construction).
    pub fn rated_power_mw(&self) -> ActivePowerMW {
        self.rated_power_mw
    }

    /// The rated energy in MWh, when applicable (storage assets; strictly
    /// positive by construction).
    pub fn rated_energy_mwh(&self) -> Option<EnergyMWh> {
        self.rated_energy_mwh
    }

    /// The phase reference (free-form until a phase vocabulary is frozen).
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// The bus reference in the grid model (P0.10 §1 row 4).
    pub fn bus_reference(&self) -> &str {
        &self.bus_reference
    }

    /// The capability profile reference (P0.10 §1 row 4; capability
    /// implementation is Phase 3, P0.10 §5).
    pub fn capability_profile(&self) -> &str {
        &self.capability_profile
    }

    /// The registry lifecycle status (P2.02). Newly registered assets are
    /// [`AssetLifecycleStatus::Active`].
    pub fn lifecycle_status(&self) -> AssetLifecycleStatus {
        self.lifecycle_status
    }

    /// The recorded deactivation reason, present exactly when the asset is
    /// [`AssetLifecycleStatus::Deactivated`].
    pub fn deactivation_reason(&self) -> Option<&str> {
        self.deactivation_reason.as_deref()
    }

    /// When the deactivation was recorded (UTC; P0.09), present exactly when
    /// the asset is [`AssetLifecycleStatus::Deactivated`].
    pub fn deactivated_at(&self) -> Option<EventTime> {
        self.deactivated_at
    }

    /// Deactivates the asset with a mandatory reason (P2.02/P2.03): returns
    /// a **new** [`Asset`] value with [`AssetLifecycleStatus::Deactivated`],
    /// the reason, and the deactivation timestamp set. The reason is a
    /// validated non-empty reference (it is the durable record of why the
    /// asset left coordination scope). Deactivation is terminal: deactivating
    /// an already-deactivated asset is an [`EntityError::AlreadyDeactivated`]
    /// error, not a no-op.
    ///
    /// The value is immutable: the caller persists the returned state through
    /// the repository update port and emits `AssetAvailabilityChanged`
    /// (§15.3) via the outbox — the domain performs no I/O.
    pub fn deactivate(&self, reason: String, at: EventTime) -> Result<Asset, EntityError> {
        if self.lifecycle_status == AssetLifecycleStatus::Deactivated {
            return Err(EntityError::AlreadyDeactivated);
        }
        require_non_empty(&reason, "deactivation_reason")?;
        let mut deactivated = self.clone();
        deactivated.lifecycle_status = AssetLifecycleStatus::Deactivated;
        deactivated.deactivation_reason = Some(reason);
        deactivated.deactivated_at = Some(at);
        Ok(deactivated)
    }

    /// Updates the capability-bearing metadata (P2.03 `UpdateAssetMetadata`):
    /// vendor/model, rated power/energy, and the capability-profile
    /// reference. Returns a **new** [`Asset`] value with the fields
    /// revalidated under the same construction rules (positive rated
    /// magnitudes in canonical MW/MWh per P0.07, non-empty references).
    ///
    /// The capability profile stays a **reference** (P2.02): its contents are
    /// Phase 3 work (P0.10 §5), so Phase 2 stores and re-points the reference
    /// only. The lifecycle status, identity, ownership chain, and creation
    /// time are deliberately not touchable here — metadata updates cannot
    /// move an asset between sites or tenants.
    #[allow(clippy::too_many_arguments)]
    pub fn update_capability_metadata(
        &self,
        vendor_model: String,
        rated_power_mw: ActivePowerMW,
        rated_energy_mwh: Option<EnergyMWh>,
        capability_profile: String,
    ) -> Result<Asset, EntityError> {
        require_non_empty(&vendor_model, "vendor_model")?;
        require_positive(rated_power_mw.value(), "rated_power_mw")?;
        if let Some(e) = &rated_energy_mwh {
            require_positive(e.value(), "rated_energy_mwh")?;
        }
        require_non_empty(&capability_profile, "capability_profile")?;
        let mut updated = self.clone();
        updated.vendor_model = vendor_model;
        updated.rated_power_mw = rated_power_mw;
        updated.rated_energy_mwh = rated_energy_mwh;
        updated.capability_profile = capability_profile;
        Ok(updated)
    }

    /// When the asset record was created (UTC; P0.09).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

// --- AssetState --------------------------------------------------------------

/// **AssetState** (P0.10 §1 row 5): SOC, active/reactive power, availability,
/// operating mode, health, timestamp, quality — the current-state type of an
/// [`Asset`].
///
/// Frozen rules encoded here:
/// - The timestamp pair is the P0.09 `event_time` (when the state occurred)
///   and `ingest_time` (when FlexCore received it); they are distinct types
///   and `ingest_time ≥ event_time` is enforced at construction.
/// - Quality is exactly one of the six canonical
///   [`TelemetryQuality`] states from the contracts crate (P0.08); this type
///   never transforms, downgrades, or omits a quality flag (non-silencing
///   rule).
/// - A reactive-power value is carried only together with its declared
///   [`ReactiveSignConvention`] (P0.07) — never a bare value, never a sign
///   interpreted without the convention.
/// - SOC, active power, reactive power, and health are optional: not every
///   asset class reports every quantity (P0.10 §2). Availability and
///   operating mode are required operational state.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetState {
    asset_id: AssetId,
    tenant_id: TenantId,
    event_time: EventTime,
    ingest_time: IngestTime,
    soc: Option<StateOfCharge>,
    active_power: Option<ActivePowerMW>,
    reactive_power: Option<ReactivePowerMVAr>,
    reactive_convention: Option<ReactiveSignConvention>,
    available: bool,
    operating_mode: String,
    health: Option<String>,
    quality: TelemetryQuality,
}

impl AssetState {
    /// Constructs an asset state under the given asset. Rejects an
    /// `ingest_time` before the `event_time` (P0.09), an empty operating
    /// mode, and a reactive-power value without its declared convention
    /// (or the inverse pairing).
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        asset: &Asset,
        event_time: EventTime,
        ingest_time: IngestTime,
        soc: Option<StateOfCharge>,
        active_power: Option<ActivePowerMW>,
        reactive_power: Option<ReactivePowerMVAr>,
        reactive_convention: Option<ReactiveSignConvention>,
        available: bool,
        operating_mode: String,
        health: Option<String>,
        quality: TelemetryQuality,
    ) -> Result<Self, EntityError> {
        validate_ingest_after_event(event_time, ingest_time).map_err(EntityError::Time)?;
        if reactive_power.is_some() != reactive_convention.is_some() {
            return Err(EntityError::ReactiveConventionMismatch);
        }
        require_non_empty(&operating_mode, "operating_mode")?;
        if let Some(h) = &health {
            require_non_empty(h, "health")?;
        }
        Ok(Self {
            asset_id: asset.asset_id(),
            tenant_id: asset.tenant_id(),
            event_time,
            ingest_time,
            soc,
            active_power,
            reactive_power,
            reactive_convention,
            available,
            operating_mode,
            health,
            quality,
        })
    }

    /// The asset this state belongs to (derived from the parent).
    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// The owning tenant (derived through the asset; P0.16 rule 1).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// When the state occurred (UTC; P0.09).
    pub fn event_time(&self) -> EventTime {
        self.event_time
    }

    /// When FlexCore received the state (UTC; P0.09).
    pub fn ingest_time(&self) -> IngestTime {
        self.ingest_time
    }

    /// The state of charge as a fraction in [0,1], when reported (P0.07).
    pub fn soc(&self) -> Option<StateOfCharge> {
        self.soc
    }

    /// The active power in MW under the frozen sign rule `P > 0 = export`
    /// (P0.07), when reported.
    pub fn active_power(&self) -> Option<ActivePowerMW> {
        self.active_power
    }

    /// The reactive power in MVAr, when reported. Interpretation requires
    /// [`AssetState::reactive_convention`] (P0.07).
    pub fn reactive_power(&self) -> Option<ReactivePowerMVAr> {
        self.reactive_power
    }

    /// The declared reactive sign convention governing
    /// [`AssetState::reactive_power`], when a reactive value is reported.
    pub fn reactive_convention(&self) -> Option<ReactiveSignConvention> {
        self.reactive_convention
    }

    /// Whether the asset is available for coordination.
    pub fn available(&self) -> bool {
        self.available
    }

    /// The operating mode (free-form reference until a mode vocabulary is
    /// frozen; P0.10 §1 row 5 lists `operating mode` as a key field).
    pub fn operating_mode(&self) -> &str {
        &self.operating_mode
    }

    /// The health reference, when reported (P0.10 §1 row 5).
    pub fn health(&self) -> Option<&str> {
        self.health.as_deref()
    }

    /// The data quality/freshness state: exactly one of the six canonical
    /// states (P0.08).
    pub fn quality(&self) -> TelemetryQuality {
        self.quality
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TenantId;

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8b";
    const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8e";

    fn uuid(text: &str) -> FlexUuid {
        FlexUuid::parse(text).unwrap()
    }

    fn tenant() -> Tenant {
        Tenant::try_new(
            TenantId::new(uuid(UUID_TENANT)),
            "Acme Energy".to_string(),
            None,
            "idp:acme".to_string(),
            "profile:default".to_string(),
            EventTime::from_unix_millis(1_000).unwrap(),
        )
        .unwrap()
    }

    fn portfolio(parent: &Tenant) -> Portfolio {
        Portfolio::try_new(
            parent,
            PortfolioId::new(uuid(UUID_PORTFOLIO)),
            "North portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "DE-AMPRION".to_string(),
            "rules:constrained-aggregation".to_string(),
            EventTime::from_unix_millis(2_000).unwrap(),
        )
        .unwrap()
    }

    fn site(parent: &Portfolio) -> Site {
        Site::try_new(
            parent,
            SiteId::new(uuid(UUID_SITE)),
            "Substation 7 site".to_string(),
            None,
            "52.52N 13.40E".to_string(),
            SiteTimezone::new("Europe/Berlin".to_string()).unwrap(),
            "PCC-7".to_string(),
            "grid-model:rev-42".to_string(),
            "edge-0007".to_string(),
            EventTime::from_unix_millis(3_000).unwrap(),
        )
        .unwrap()
    }

    fn asset(parent: &Site) -> Asset {
        Asset::try_new(
            parent,
            AssetId::new(uuid(UUID_ASSET)),
            "BESS 1".to_string(),
            None,
            AssetType::Bess,
            "vendor-x/model-1".to_string(),
            ActivePowerMW::new(2.0).unwrap(),
            Some(EnergyMWh::new(4.0).unwrap()),
            "three-phase".to_string(),
            "bus-17".to_string(),
            "capability:bess-v1".to_string(),
            EventTime::from_unix_millis(4_000).unwrap(),
        )
        .unwrap()
    }

    // --- Tenant -------------------------------------------------------------

    #[test]
    fn tenant_accepts_valid_fields() {
        let t = tenant();
        assert_eq!(t.tenant_id().uuid(), uuid(UUID_TENANT));
        assert_eq!(t.name(), "Acme Energy");
        assert_eq!(t.description(), None);
        assert_eq!(t.created_at().unix_millis(), 1_000);
    }

    #[test]
    fn tenant_rejects_nil_id_and_empty_fields() {
        let e = Tenant::try_new(
            TenantId::new(FlexUuid::NIL),
            "Acme".to_string(),
            None,
            "idp".to_string(),
            "profile".to_string(),
            EventTime::from_unix_millis(1_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(e, EntityError::NilIdentifier { field: "tenant_id" });

        let t = tenant();
        for (name, idp, policy) in [
            ("", "idp", "profile"),
            ("   ", "idp", "profile"),
            ("Acme", "", "profile"),
            ("Acme", "   ", "profile"),
            ("Acme", "idp", ""),
            ("Acme", "idp", "  "),
        ] {
            let err = Tenant::try_new(
                t.tenant_id(),
                name.to_string(),
                None,
                idp.to_string(),
                policy.to_string(),
                EventTime::from_unix_millis(1_000).unwrap(),
            )
            .unwrap_err();
            assert!(
                matches!(err, EntityError::EmptyField { .. }),
                "empty field must be rejected: {err}"
            );
        }
        // Some(description) must be non-empty too.
        let err = Tenant::try_new(
            t.tenant_id(),
            "Acme".to_string(),
            Some("  ".to_string()),
            "idp".to_string(),
            "profile".to_string(),
            EventTime::from_unix_millis(1_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, EntityError::EmptyField { field: "description" });
    }

    // --- Portfolio (requires Tenant) ----------------------------------------

    #[test]
    fn portfolio_requires_a_tenant_and_derives_its_tenant_id() {
        // "Portfolio requires tenant" is a compile-level fact: try_new takes
        // the parent Tenant by reference, so no portfolio can be built
        // without one. The runtime assertions pin the derived scoping.
        let t = tenant();
        let p = portfolio(&t);
        assert_eq!(p.tenant_id(), t.tenant_id());
        assert_eq!(p.portfolio_id().uuid(), uuid(UUID_PORTFOLIO));
    }

    #[test]
    fn portfolio_rejects_nil_id_and_empty_fields() {
        let t = tenant();
        let err = Portfolio::try_new(
            &t,
            PortfolioId::new(FlexUuid::NIL),
            "P".to_string(),
            None,
            "op".to_string(),
            "region".to_string(),
            "rules".to_string(),
            EventTime::from_unix_millis(2_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, EntityError::NilIdentifier { field: "portfolio_id" });

        for (name, operator, region, rules) in [
            ("", "op", "region", "rules"),
            ("P", "", "region", "rules"),
            ("P", "op", "", "rules"),
            ("P", "op", "region", ""),
        ] {
            let err = Portfolio::try_new(
                &t,
                PortfolioId::new(uuid(UUID_PORTFOLIO)),
                name.to_string(),
                None,
                operator.to_string(),
                region.to_string(),
                rules.to_string(),
                EventTime::from_unix_millis(2_000).unwrap(),
            )
            .unwrap_err();
            assert!(matches!(err, EntityError::EmptyField { .. }), "{err}");
        }
    }

    // --- Site (requires Portfolio) ------------------------------------------

    #[test]
    fn site_requires_a_portfolio_and_derives_the_full_chain() {
        // "Site without Portfolio rejected": try_new takes &Portfolio, so
        // the call cannot even be expressed without a constructed portfolio;
        // at runtime the derived IDs carry the whole ownership chain.
        let t = tenant();
        let p = portfolio(&t);
        let s = site(&p);
        assert_eq!(s.tenant_id(), t.tenant_id());
        assert_eq!(s.portfolio_id(), p.portfolio_id());
        assert_eq!(s.timezone().as_str(), "Europe/Berlin");
    }

    #[test]
    fn site_rejects_nil_id_and_empty_fields() {
        let t = tenant();
        let p = portfolio(&t);
        let err = Site::try_new(
            &p,
            SiteId::new(FlexUuid::NIL),
            "S".to_string(),
            None,
            "loc".to_string(),
            SiteTimezone::new("Europe/Berlin".to_string()).unwrap(),
            "pcc".to_string(),
            "grid".to_string(),
            "edge".to_string(),
            EventTime::from_unix_millis(3_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, EntityError::NilIdentifier { field: "site_id" });

        for (name, location, pcc, grid, edge) in [
            ("", "loc", "pcc", "grid", "edge"),
            ("S", "", "pcc", "grid", "edge"),
            ("S", "loc", "", "grid", "edge"),
            ("S", "loc", "pcc", "", "edge"),
            ("S", "loc", "pcc", "grid", ""),
        ] {
            let err = Site::try_new(
                &p,
                SiteId::new(uuid(UUID_SITE)),
                name.to_string(),
                None,
                location.to_string(),
                SiteTimezone::new("Europe/Berlin".to_string()).unwrap(),
                pcc.to_string(),
                grid.to_string(),
                edge.to_string(),
                EventTime::from_unix_millis(3_000).unwrap(),
            )
            .unwrap_err();
            assert!(matches!(err, EntityError::EmptyField { .. }), "{err}");
        }
    }

    // --- Asset (requires Site) ----------------------------------------------

    #[test]
    fn asset_requires_a_site_and_derives_the_full_chain() {
        // "Asset without Site rejected": try_new takes &Site; the derived
        // tenant/site scoping is asserted at runtime.
        let t = tenant();
        let p = portfolio(&t);
        let s = site(&p);
        let a = asset(&s);
        assert_eq!(a.tenant_id(), t.tenant_id());
        assert_eq!(a.site_id(), s.site_id());
        assert_eq!(a.asset_type(), AssetType::Bess);
        assert_eq!(a.rated_power_mw().value(), 2.0);
        assert_eq!(a.rated_energy_mwh().unwrap().value(), 4.0);
    }

    #[test]
    fn asset_rejects_nil_id_empty_fields_and_non_positive_rated_quantities() {
        let t = tenant();
        let p = portfolio(&t);
        let s = site(&p);

        let err = Asset::try_new(
            &s,
            AssetId::new(FlexUuid::NIL),
            "A".to_string(),
            None,
            AssetType::Bess,
            "vendor".to_string(),
            ActivePowerMW::new(1.0).unwrap(),
            None,
            "three-phase".to_string(),
            "bus".to_string(),
            "capability".to_string(),
            EventTime::from_unix_millis(4_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, EntityError::NilIdentifier { field: "asset_id" });

        // Non-positive rated power / energy.
        for power in [0.0, -1.0] {
            let err = Asset::try_new(
                &s,
                AssetId::new(uuid(UUID_ASSET)),
                "A".to_string(),
                None,
                AssetType::Bess,
                "vendor".to_string(),
                ActivePowerMW::new(power).unwrap(),
                None,
                "three-phase".to_string(),
                "bus".to_string(),
                "capability".to_string(),
                EventTime::from_unix_millis(4_000).unwrap(),
            )
            .unwrap_err();
            assert_eq!(
                err,
                EntityError::NonPositiveQuantity { field: "rated_power_mw" }
            );
        }
        let err = Asset::try_new(
            &s,
            AssetId::new(uuid(UUID_ASSET)),
            "A".to_string(),
            None,
            AssetType::Bess,
            "vendor".to_string(),
            ActivePowerMW::new(1.0).unwrap(),
            Some(EnergyMWh::new(0.0).unwrap()),
            "three-phase".to_string(),
            "bus".to_string(),
            "capability".to_string(),
            EventTime::from_unix_millis(4_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EntityError::NonPositiveQuantity { field: "rated_energy_mwh" }
        );

        // Empty descriptive/reference fields.
        for (name, vendor, phase, bus, capability) in [
            ("", "vendor", "phase", "bus", "capability"),
            ("A", "", "phase", "bus", "capability"),
            ("A", "vendor", "", "bus", "capability"),
            ("A", "vendor", "phase", "", "capability"),
            ("A", "vendor", "phase", "bus", ""),
        ] {
            let err = Asset::try_new(
                &s,
                AssetId::new(uuid(UUID_ASSET)),
                name.to_string(),
                None,
                AssetType::PvInverter,
                vendor.to_string(),
                ActivePowerMW::new(1.0).unwrap(),
                None,
                phase.to_string(),
                bus.to_string(),
                capability.to_string(),
                EventTime::from_unix_millis(4_000).unwrap(),
            )
            .unwrap_err();
            assert!(matches!(err, EntityError::EmptyField { .. }), "{err}");
        }
    }

    // --- AssetState -----------------------------------------------------------

    #[test]
    fn asset_state_accepts_the_frozen_field_set() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        let state = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(10_000).unwrap(),
            IngestTime::from_unix_millis(11_000).unwrap(),
            Some(StateOfCharge::new(0.75).unwrap()),
            Some(ActivePowerMW::new(-1.5).unwrap()), // P < 0 = import (P0.07)
            Some(ReactivePowerMVAr::new(0.3).unwrap()),
            Some(ReactiveSignConvention::InjectionPositive),
            true,
            "following-schedule".to_string(),
            Some("nominal".to_string()),
            TelemetryQuality::Good,
        )
        .unwrap();
        assert_eq!(state.asset_id(), a.asset_id());
        assert_eq!(state.tenant_id(), t.tenant_id());
        assert_eq!(state.quality(), TelemetryQuality::Good);
        assert_eq!(state.soc().unwrap().value(), 0.75);
        assert!(state.available());
    }

    #[test]
    fn asset_state_rejects_ingest_before_event() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        let err = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(11_000).unwrap(),
            IngestTime::from_unix_millis(10_000).unwrap(),
            None,
            None,
            None,
            None,
            true,
            "idle".to_string(),
            None,
            TelemetryQuality::Good,
        )
        .unwrap_err();
        assert!(matches!(err, EntityError::Time(_)), "{err}");
    }

    #[test]
    fn asset_state_rejects_reactive_value_without_convention_and_inverse() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        // Q without the declared convention.
        let err = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(10_000).unwrap(),
            IngestTime::from_unix_millis(10_000).unwrap(),
            None,
            None,
            Some(ReactivePowerMVAr::new(0.3).unwrap()),
            None,
            true,
            "idle".to_string(),
            None,
            TelemetryQuality::Good,
        )
        .unwrap_err();
        assert_eq!(err, EntityError::ReactiveConventionMismatch);
        // Convention without a Q value.
        let err = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(10_000).unwrap(),
            IngestTime::from_unix_millis(10_000).unwrap(),
            None,
            None,
            None,
            Some(ReactiveSignConvention::ConsumptionPositive),
            true,
            "idle".to_string(),
            None,
            TelemetryQuality::Good,
        )
        .unwrap_err();
        assert_eq!(err, EntityError::ReactiveConventionMismatch);
    }

    #[test]
    fn asset_state_rejects_empty_operating_mode_and_health() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        let err = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(10_000).unwrap(),
            IngestTime::from_unix_millis(10_000).unwrap(),
            None,
            None,
            None,
            None,
            true,
            "  ".to_string(),
            None,
            TelemetryQuality::Stale,
        )
        .unwrap_err();
        assert_eq!(err, EntityError::EmptyField { field: "operating_mode" });

        let err = AssetState::try_new(
            &a,
            EventTime::from_unix_millis(10_000).unwrap(),
            IngestTime::from_unix_millis(10_000).unwrap(),
            None,
            None,
            None,
            None,
            true,
            "idle".to_string(),
            Some("".to_string()),
            TelemetryQuality::Suspect,
        )
        .unwrap_err();
        assert_eq!(err, EntityError::EmptyField { field: "health" });
    }

    // --- Asset lifecycle + metadata update (P2.03 additions) -----------------

    #[test]
    fn a_new_asset_is_active_and_deactivate_records_reason_and_time() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        assert_eq!(a.lifecycle_status(), AssetLifecycleStatus::Active);
        assert_eq!(a.deactivation_reason(), None);
        assert_eq!(a.deactivated_at(), None);

        let deactivated = a
            .deactivate(
                "maintenance window".to_string(),
                EventTime::from_unix_millis(9_000).unwrap(),
            )
            .unwrap();
        assert_eq!(
            deactivated.lifecycle_status(),
            AssetLifecycleStatus::Deactivated
        );
        assert_eq!(deactivated.deactivation_reason(), Some("maintenance window"));
        assert_eq!(deactivated.deactivated_at().unwrap().unix_millis(), 9_000);
        // The original value is immutable: deactivation produced a new value.
        assert_eq!(a.lifecycle_status(), AssetLifecycleStatus::Active);
        // Identity, ownership, and vocabulary are untouched by the transition.
        assert_eq!(deactivated.asset_id(), a.asset_id());
        assert_eq!(deactivated.tenant_id(), a.tenant_id());
        assert_eq!(deactivated.capability_profile(), a.capability_profile());
    }

    #[test]
    fn deactivate_rejects_an_empty_reason_and_is_terminal() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        for reason in ["", "   "] {
            let err = a
                .deactivate(
                    reason.to_string(),
                    EventTime::from_unix_millis(9_000).unwrap(),
                )
                .unwrap_err();
            assert_eq!(
                err,
                EntityError::EmptyField { field: "deactivation_reason" }
            );
        }

        let deactivated = a
            .deactivate(
                "retired".to_string(),
                EventTime::from_unix_millis(9_000).unwrap(),
            )
            .unwrap();
        // Terminal: a second deactivation is a state error, not a no-op.
        let err = deactivated
            .deactivate(
                "again".to_string(),
                EventTime::from_unix_millis(9_500).unwrap(),
            )
            .unwrap_err();
        assert_eq!(err, EntityError::AlreadyDeactivated);
    }

    #[test]
    fn update_capability_metadata_revalidates_and_preserves_identity_and_lifecycle() {
        let t = tenant();
        let a = asset(&site(&portfolio(&t)));
        let updated = a
            .update_capability_metadata(
                "vendor-y/model-9".to_string(),
                ActivePowerMW::new(3.5).unwrap(),
                Some(EnergyMWh::new(7.0).unwrap()),
                "capability:bess-v2".to_string(),
            )
            .unwrap();
        assert_eq!(updated.vendor_model(), "vendor-y/model-9");
        assert_eq!(updated.rated_power_mw().value(), 3.5);
        assert_eq!(updated.rated_energy_mwh().unwrap().value(), 7.0);
        assert_eq!(updated.capability_profile(), "capability:bess-v2");
        // Untouchable fields: identity, ownership chain, lifecycle, creation.
        assert_eq!(updated.asset_id(), a.asset_id());
        assert_eq!(updated.site_id(), a.site_id());
        assert_eq!(updated.tenant_id(), a.tenant_id());
        assert_eq!(updated.lifecycle_status(), a.lifecycle_status());
        assert_eq!(updated.created_at(), a.created_at());

        // Revalidation: non-positive rated quantities and empty references
        // are rejected exactly as at construction.
        let err = a
            .update_capability_metadata(
                "vendor".to_string(),
                ActivePowerMW::new(0.0).unwrap(),
                None,
                "capability".to_string(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            EntityError::NonPositiveQuantity { field: "rated_power_mw" }
        );
        let err = a
            .update_capability_metadata(
                " ".to_string(),
                ActivePowerMW::new(1.0).unwrap(),
                None,
                "capability".to_string(),
            )
            .unwrap_err();
        assert_eq!(err, EntityError::EmptyField { field: "vendor_model" });
        let err = a
            .update_capability_metadata(
                "vendor".to_string(),
                ActivePowerMW::new(1.0).unwrap(),
                Some(EnergyMWh::new(-1.0).unwrap()),
                "capability".to_string(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            EntityError::NonPositiveQuantity { field: "rated_energy_mwh" }
        );
        let err = a
            .update_capability_metadata(
                "vendor".to_string(),
                ActivePowerMW::new(1.0).unwrap(),
                None,
                "".to_string(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            EntityError::EmptyField { field: "capability_profile" }
        );

        // Metadata updates are orthogonal to lifecycle: a deactivated asset
        // can carry a corrected capability reference (the reference is
        // registry metadata; the asset stays Deactivated).
        let deactivated = a
            .deactivate(
                "retired".to_string(),
                EventTime::from_unix_millis(9_000).unwrap(),
            )
            .unwrap();
        let corrected = deactivated
            .update_capability_metadata(
                "vendor-y/model-9".to_string(),
                ActivePowerMW::new(3.5).unwrap(),
                None,
                "capability:bess-v2".to_string(),
            )
            .unwrap();
        assert_eq!(
            corrected.lifecycle_status(),
            AssetLifecycleStatus::Deactivated
        );
    }
}
