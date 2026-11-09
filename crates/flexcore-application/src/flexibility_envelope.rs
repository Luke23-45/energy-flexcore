// P3.12 — Portfolio flexibility envelope (constrained aggregation, §11.6).
//
// Implements ledger P3.12 ("Portfolio flexibility envelope", report §11.6,
// §24 step 3). Decision record:
// `ledger/phase-03-flexibility-scheduling/12_13_envelope.md`.
//
// THE AGGREGATION RULE (report §11.6), DECIDED ONCE:
//
//   P_portfolio(t) = Σ_i P_net(i,t) − P_loss/adjustment(t)
//
// computed as a **constrained aggregation over per-interval feasible sets**,
// never a nameplate sum. Per interval t, in order:
//
//   1. Each asset's declared feasible P_net range (MW, export-positive,
//      §10.2) is narrowed by the frozen P3.02 unknown-narrows rule
//      (`flexcore_domain::capability_provenance`, reused — never
//      reimplemented): an UNKNOWN-provenance declaration collapses to zero
//      credit; an empty intersection makes the interval INFEASIBLE (never
//      repaired by widening).
//   2. Forecast-bounded assets are capped per interval by their per-asset
//      forecast from the single labelled fixture (P3.31), accessed ONLY
//      through `LabeledForecastSeries::values` — the label, version, and
//      quantity travel with every read (four-place labelling, places 1–2)
//      and the synthetic label is copied into the envelope's input snapshot
//      (the envelope-input place of the four-place rule).
//   3. Availability (a declared per-interval fraction in [0, 1] with a
//      recorded source) scales the flexibility magnitudes toward zero. An
//      asset WITHOUT evidenced availability credits NOTHING (recorded
//      narrowing, P3.02-consistent: unknown never widens).
//   4. Time-coupled energy constraints enter as a conservative
//      horizon-start energy cap for BESS-class assets: the usable energy
//      above the floor (and the headroom below the ceiling) converted to a
//      sustained-power bound over one interval, with NO intra-horizon
//      recharge/discharge credit (a recorded conservatism). The values come
//      from the P3.05 BESS model quantities (`e_min`, `e_max`, `eta_ch`,
//      `eta_dis`, the reserve-narrowed usable window); the cap is REQUIRED
//      configuration for a BESS-class dispatchable asset.
//   5. Shared constraints bind the aggregate: per site, the members' ranges
//      are summed and then capped by the site's declared PCC export/import
//      limits (the P2.02 site entity carries the PCC as a reference string;
//      the numeric limits are required configuration — the registry holds
//      no transformer limit, gap G17). A portfolio-level coupling limit
//      does not exist in the data and is a recorded unknown dimension.
//   6. The loss/adjustment term is subtracted from the aggregate. With no
//      model on record the term is 0.0 and the omission is a recorded
//      unknown dimension (omitting losses may overstate export capability —
//      stated, never hidden).
//
// THE CORRELATION TREATMENT, DECIDED ONCE (recorded per ledger P3.12
// step 2; no correlation coefficient is ever invented):
//
//   - Forecast-driven assets are bounded per interval by their per-asset
//     forecast values from the single fixture. A shared weather driver
//     therefore enters through the forecast values themselves, not through
//     an assumed statistical correlation; no additional derating is applied
//     to the forecast-capped contribution.
//   - Assets declared into a `SharedExposureGroup` (shared site limit,
//     shared weather exposure beyond the forecast, common operating
//     restriction) carry a CONFIGURED conservative derate fraction in
//     [0, 1] with a MANDATORY recorded basis string (labelled ASSUMPTION).
//     Both bounds of every member's per-interval range are scaled by
//     (1 − f) toward zero before the site aggregation. The derate value is
//     configuration with a recorded basis — never generated here.
//   - Assets in NO group are recorded in
//     `CorrelationTreatmentRecord::unmodelled_assets`: no correlation model
//     exists on record for them, and none is assumed.
//
// CONFIDENCE (§11.6 "where the model supports it"): none of the Phase 3
// capability models provides a verified confidence or availability
// distribution. The envelope's availability descriptor is the explicit
// statement `AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED` — no confidence number
// is invented anywhere in this module.
//
// EVENT EMISSION (§15.3 `FlexibilityEnvelopeUpdated`): a pure outbox
// assembly (`envelope_updated_publications`), one event per covered site.
// PAYLOAD-FIT GAP (recorded honestly, the D-FX-9 precedent): the frozen
// `FlexibilityEnvelopeUpdatedPayload` carries correlation/tenant/portfolio/
// site/asset + event_time only — NO envelope version, NO horizon, and NO
// input-snapshot references, and it is asset-scoped while an envelope
// update is site/portfolio-scoped. Until flexcore-events extends the
// payload additively (the P2.16 `serde(default)` precedent, no
// schema-version bump), (a) the nil asset id denotes "the envelope scope,
// not a single asset" (a recorded convention), and (b) the envelope version
// travels in the deterministic idempotency key
// `flexibility_envelope_updated:<portfolio>:<site>:<envelope_version>`
// (republication of one envelope version for one site dedupes durably).
// The full version/horizon/snapshot record is the returned
// `PortfolioFlexibilityEnvelope` itself.
//
// THE GATE (ledger P3.12 acceptance): the envelope is strictly less than
// the nameplate sum on the documented test case where a shared site limit
// binds (`tests/flexibility_envelope.rs`); this is a gate, not a
// nice-to-have.
//
// Control status: pure computation and pure outbox assembly for the
// simulation-only 0.x line. No I/O, no solver coupling, no field-write
// path. A computed envelope is an ESTIMATE for scheduling, never a
// feasibility, authorization, or dispatch statement (Phase 4 gates).

use std::collections::HashMap;

use flexcore_contracts::capability::{ConstraintKind, Provenance as CapabilityProvenance};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::capability_provenance::{
    narrow_interval, AssetClass, BoundInterval, NarrowedSet,
};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::scheduling_time::SchedulingGrid;
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{DomainEvent, FlexibilityEnvelopeUpdatedPayload};
use flexcore_events::subject::{subject_for, Subject};

use crate::forecast_fixture::{
    ForecastFixture, ForecastQuantity, ForecastVersion, LabeledForecastSeries,
    Provenance as ForecastProvenance, FIXTURE_ID,
};

/// Numerical tolerance of the envelope arithmetic, in MW (the domain
/// models' recorded 1e-9 convention, P3.07-2).
pub const ENVELOPE_TOLERANCE: f64 = 1e-9;

/// The availability/confidence descriptor carried on every envelope (§11.6:
/// return a confidence/availability descriptor **where the model supports
/// it** — none does, so support is explicitly denied rather than a number
/// invented).
pub const AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED: &str = "NOT SUPPORTED: no Phase 3 capability model (P3.05-P3.11) provides a verified confidence or availability distribution; no confidence number is invented. Availability enters the envelope only as the declared per-interval fractions in [0, 1] recorded on each asset input.";

/// The recorded correlation-treatment statement carried on every envelope.
pub const CORRELATION_TREATMENT_STATEMENT: &str = "Forecast-driven assets are bounded per interval by their per-asset forecast from the single labelled fixture (P3.31), so a shared weather driver enters through the forecast values rather than an invented correlation coefficient. Assets declared into a shared-exposure group carry a configured conservative derate fraction with a recorded ASSUMPTION basis (no correlation coefficient is generated). Assets in no group have NO correlation model on record and none is assumed; they are listed in the envelope's unmodelled-assets record.";

/// The recorded payload-fit gap note (see the module header).
pub const ENVELOPE_PAYLOAD_GAP_NOTE: &str = "The frozen FlexibilityEnvelopeUpdatedPayload (P0.14/P1.09) carries correlation/tenant/portfolio/site/asset + event_time only: no envelope version, no horizon, no input-snapshot reference, and an asset scope that an envelope update does not have. The nil asset id denotes the envelope scope (recorded convention); the envelope version travels in the idempotency key flexibility_envelope_updated:<portfolio>:<site>:<envelope_version>. Needs extension: additive serde(default) payload fields (envelope_version, horizon_start_millis, input_snapshot_ref) per the P2.16 precedent, for a future subtask that owns flexcore-events.";

// --- Envelope version --------------------------------------------------------

/// The envelope version: a validated semantic string. Non-empty,
/// whitespace-free — the staleness checks (P3.13), the schedule references
/// (P3.21), and the idempotency key carry it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvelopeVersion(String);

impl EnvelopeVersion {
    /// Validates and wraps an envelope version.
    pub fn new(version: &str) -> Result<Self, EnvelopeError> {
        if version.is_empty() {
            return Err(EnvelopeError::EmptyEnvelopeVersion);
        }
        if version.chars().any(|c| c.is_whitespace()) {
            return Err(EnvelopeError::EnvelopeVersionHasWhitespace);
        }
        Ok(Self(version.to_string()))
    }

    /// The version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EnvelopeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Errors ------------------------------------------------------------------

/// Errors returned by the P3.12 envelope calculator and publication.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvelopeError {
    /// The envelope version string was empty.
    EmptyEnvelopeVersion,
    /// The envelope version contained whitespace.
    EnvelopeVersionHasWhitespace,
    /// No asset was supplied; an envelope over an empty portfolio is not a
    /// computation result, it is a caller error.
    NoAssets,
    /// No site coupling was supplied although assets were.
    NoSiteCouplings,
    /// An asset referenced a site with no declared coupling. Every asset's
    /// site must be a coupled site; the site PCC coupling is how shared
    /// constraints bind (§11.6).
    MissingSiteCoupling { site_id: SiteId },
    /// The same asset was supplied twice.
    DuplicateAsset { asset_id: AssetId },
    /// The same site coupling was supplied twice.
    DuplicateSiteCoupling { site_id: SiteId },
    /// An asset's capability-profile reference was empty (the P2.02
    /// registry field is a required reference string).
    EmptyCapabilityProfileReference { asset_id: AssetId },
    /// A declared P_net bound was not finite.
    NonFiniteDeclaredBound { asset_id: AssetId },
    /// A declared P_net range was inverted (lower > upper).
    InvertedDeclaredRange { asset_id: AssetId },
    /// A declared P_net range side was unbounded. The envelope composes
    /// finite per-interval bounds; an unbounded side would propagate an
    /// unbounded aggregate and is rejected rather than silently clipped.
    UnboundedDeclaredRange { asset_id: AssetId },
    /// An availability fraction was not finite or outside [0, 1].
    AvailabilityOutOfRange { asset_id: AssetId, index: usize, value: f64 },
    /// The availability series length did not match the grid interval count.
    AvailabilityCountMismatch { asset_id: AssetId, expected: usize, found: usize },
    /// The declared availability source string was empty (the source must
    /// be recorded, never silent).
    EmptyAvailabilitySource { asset_id: AssetId },
    /// A declared flexibility offset (reducible/increasable MW) was not
    /// finite or negative.
    NonPositiveOffset { asset_id: AssetId, value: f64 },
    /// The forecast series length did not match the grid interval count.
    ForecastSeriesIntervalCountMismatch { asset_key: String, expected: usize, found: usize },
    /// The forecast series quantity did not match the asset's contribution
    /// kind (a PV cap needs a PV-output series, a load baseline needs a
    /// load series).
    ForecastQuantityMismatch { asset_key: String, expected: &'static str, found: &'static str },
    /// The fixture grid does not match the envelope grid (start or
    /// configuration). The envelope consumes the fixture on the fixture's
    /// own grid (D-FX-1); a mismatched grid is a caller error, never an
    /// implicit aggregation.
    ForecastGridMismatch {
        envelope_start_millis: i128,
        fixture_start_millis: i64,
    },
    /// An energy-coupling value was invalid (non-finite, inverted window,
    /// efficiency outside (0, 1]).
    EnergyCouplingInvalid { asset_id: AssetId, reason: &'static str },
    /// A BESS-class dispatchable asset was supplied without the required
    /// conservative energy cap (the P3.05 model provides it; omitting it
    /// would overstate sustained capability).
    EnergyCouplingRequiredForBess { asset_id: AssetId },
    /// A correlation-group derate fraction was outside [0, 1].
    DerateOutOfRange { group_id: String, value: f64 },
    /// A correlation group had an empty id, an empty basis, or no members.
    InvalidSharedExposureGroup { group_id: String, reason: &'static str },
    /// The same asset appeared in two groups or twice in one group; an
    /// asset may belong to at most one shared-exposure group.
    DuplicateGroupAsset { group_id: String, asset_id: AssetId },
    /// A group member was not among the supplied assets.
    UnknownGroupAsset { group_id: String, asset_id: AssetId },
    /// The loss/adjustment value was not finite.
    NonFiniteLossAdjustment,
    /// The nameplate reference sum was not finite or negative.
    NonFiniteNameplateSum,
    /// A site coupling had an empty PCC or grid-model reference (the P2.02
    /// site entity requires both reference strings).
    EmptySiteReference { site_id: SiteId },
    /// A site limit was not finite or negative (limits are magnitudes).
    SiteLimitInvalid { site_id: SiteId, value: f64 },
    /// The publication was requested with no covered site.
    PublicationWithoutSite,
    /// The publication listed the same site twice.
    DuplicatePublicationSite { site_id: SiteId },
    /// The event envelope construction failed (the idempotency key was
    /// rejected). Cannot arise from validated inputs; carried for
    /// exhaustiveness.
    EventEnvelopeRejected,
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeError::EmptyEnvelopeVersion => {
                write!(f, "the envelope version must be non-empty")
            }
            EnvelopeError::EnvelopeVersionHasWhitespace => {
                write!(f, "the envelope version must be whitespace-free")
            }
            EnvelopeError::NoAssets => write!(f, "an envelope requires at least one asset"),
            EnvelopeError::NoSiteCouplings => {
                write!(f, "an envelope requires at least one site coupling")
            }
            EnvelopeError::MissingSiteCoupling { site_id } => {
                write!(f, "asset site {site_id} has no declared site coupling")
            }
            EnvelopeError::DuplicateAsset { asset_id } => {
                write!(f, "asset {asset_id} was supplied twice")
            }
            EnvelopeError::DuplicateSiteCoupling { site_id } => {
                write!(f, "site coupling {site_id} was supplied twice")
            }
            EnvelopeError::EmptyCapabilityProfileReference { asset_id } => {
                write!(f, "asset {asset_id} has an empty capability-profile reference")
            }
            EnvelopeError::NonFiniteDeclaredBound { asset_id } => {
                write!(f, "asset {asset_id} has a non-finite declared P_net bound")
            }
            EnvelopeError::InvertedDeclaredRange { asset_id } => {
                write!(f, "asset {asset_id} has an inverted declared P_net range")
            }
            EnvelopeError::UnboundedDeclaredRange { asset_id } => write!(
                f,
                "asset {asset_id} has an unbounded declared P_net side; the envelope composes finite bounds"
            ),
            EnvelopeError::AvailabilityOutOfRange { asset_id, index, value } => write!(
                f,
                "asset {asset_id} availability at interval {index} is {value}; must be in [0, 1]"
            ),
            EnvelopeError::AvailabilityCountMismatch { asset_id, expected, found } => write!(
                f,
                "asset {asset_id} availability has {found} values; the grid has {expected} intervals"
            ),
            EnvelopeError::EmptyAvailabilitySource { asset_id } => {
                write!(f, "asset {asset_id} availability source must be recorded")
            }
            EnvelopeError::NonPositiveOffset { asset_id, value } => write!(
                f,
                "asset {asset_id} declared flexibility offset {value} must be finite and >= 0"
            ),
            EnvelopeError::ForecastSeriesIntervalCountMismatch { asset_key, expected, found } => {
                write!(
                    f,
                    "forecast series {asset_key} has {found} values; the grid has {expected} intervals"
                )
            }
            EnvelopeError::ForecastQuantityMismatch { asset_key, expected, found } => write!(
                f,
                "forecast series {asset_key} has quantity {found}; the contribution kind requires {expected}"
            ),
            EnvelopeError::ForecastGridMismatch {
                envelope_start_millis,
                fixture_start_millis,
            } => write!(
                f,
                "the fixture grid start {fixture_start_millis} does not match the envelope grid start {envelope_start_millis} (the envelope consumes the fixture on the fixture's own grid)"
            ),
            EnvelopeError::EnergyCouplingInvalid { asset_id, reason } => {
                write!(f, "asset {asset_id} energy coupling is invalid: {reason}")
            }
            EnvelopeError::EnergyCouplingRequiredForBess { asset_id } => write!(
                f,
                "BESS-class asset {asset_id} requires the conservative energy cap (the P3.05 model provides it); omitting it would overstate sustained capability"
            ),
            EnvelopeError::DerateOutOfRange { group_id, value } => write!(
                f,
                "shared-exposure group {group_id} derate fraction {value} must be in [0, 1]"
            ),
            EnvelopeError::InvalidSharedExposureGroup { group_id, reason } => {
                write!(f, "shared-exposure group {group_id} is invalid: {reason}")
            }
            EnvelopeError::DuplicateGroupAsset { group_id, asset_id } => write!(
                f,
                "asset {asset_id} appears twice (or in two groups) via group {group_id}"
            ),
            EnvelopeError::UnknownGroupAsset { group_id, asset_id } => {
                write!(f, "group {group_id} references unknown asset {asset_id}")
            }
            EnvelopeError::NonFiniteLossAdjustment => {
                write!(f, "the loss/adjustment term must be finite")
            }
            EnvelopeError::NonFiniteNameplateSum => {
                write!(f, "the nameplate reference sum must be finite and >= 0")
            }
            EnvelopeError::EmptySiteReference { site_id } => {
                write!(f, "site {site_id} has an empty PCC or grid-model reference")
            }
            EnvelopeError::SiteLimitInvalid { site_id, value } => {
                write!(f, "site {site_id} limit {value} must be finite and >= 0")
            }
            EnvelopeError::PublicationWithoutSite => {
                write!(f, "an envelope publication requires at least one covered site")
            }
            EnvelopeError::DuplicatePublicationSite { site_id } => {
                write!(f, "publication site {site_id} was listed twice")
            }
            EnvelopeError::EventEnvelopeRejected => {
                write!(f, "the event envelope rejected the idempotency key")
            }
        }
    }
}

impl std::error::Error for EnvelopeError {}

// --- Inputs ------------------------------------------------------------------

/// A declared scalar with the provenance the frozen P3.02 rule applies to
/// (an UNKNOWN-provenance scalar credits nothing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProvenancedScalar {
    /// The declared value (a non-negative MW magnitude).
    pub value: f64,
    /// The declared provenance.
    pub provenance: CapabilityProvenance,
}

/// The per-interval availability input of one asset: a declared fraction in
/// [0, 1] per interval with a RECORDED source string. Availability with no
/// recorded source is [`AssetAvailabilityInput::Unknown`], which credits
/// nothing (the P3.02 unknown-narrows discipline applied to operational
/// state: unknown never widens).
#[derive(Debug, Clone, PartialEq)]
pub enum AssetAvailabilityInput<'a> {
    /// Per-interval availability fractions with the recorded source (for
    /// the synthetic portfolio: the scenario's declared constant
    /// availability, D-FX-2).
    Declared {
        /// One fraction in [0, 1] per grid interval.
        fractions: &'a [f64],
        /// The recorded source (non-empty; e.g. the scenario declaration).
        source: String,
    },
    /// No evidenced availability: the asset credits NO flexibility in this
    /// computation, and the narrowing is recorded as an unknown dimension.
    Unknown,
}

/// The conservative time-coupled energy input of a dispatchable asset, in
/// the units of the P3.05 BESS model: the caller takes the values from the
/// model configuration (`e_min_mwh`, `e_max_mwh`, `eta_ch`, `eta_dis`,
/// optionally the reserve-narrowed usable window). The calculator converts
/// the usable window into a per-interval sustained-power cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyCouplingInput {
    /// The (post-narrowing) energy window floor, MWh.
    pub e_min_mwh: f64,
    /// The (post-narrowing) energy window ceiling, MWh.
    pub e_max_mwh: f64,
    /// The energy at the horizon start, MWh (clamped into the window).
    pub e_initial_mwh: f64,
    /// Charge efficiency in (0, 1].
    pub eta_ch: f64,
    /// Discharge efficiency in (0, 1].
    pub eta_dis: f64,
}

/// One asset's contribution kind to the portfolio envelope.
#[derive(Debug, Clone, PartialEq)]
pub enum AssetContribution<'a> {
    /// Forecast-capped export capability (PV, §11.3/§11.6): the per-interval
    /// feasible range is `[narrowed_lower, min(narrowed_upper, forecast(t) ×
    /// availability(t))]`. The series must be a `PvOutputMw` series.
    ForecastCappedPv {
        /// The labelled forecast series (values accessed only through the
        /// labelled accessor).
        series: &'a LabeledForecastSeries,
    },
    /// Forecast baseline plus declared offsets (flexible load, §11.4):
    /// `P_net(t) = baseline(t) + offset range`, where the baseline is the
    /// labelled `LoadMw` series (consumption negative, §10.2) and the
    /// offsets are the declared reduction/increase magnitudes scaled by
    /// availability. `declared_p_net_mw` for this variant is the declared
    /// OFFSET range around the baseline.
    ForecastBaselineLoad {
        /// The labelled load-baseline series.
        series: &'a LabeledForecastSeries,
        /// The declared reducible magnitude (>= 0 MW) with provenance; the
        /// credited offset is the intersection of this magnitude and the
        /// narrowed declared range's negative side.
        reducible_mw: ProvenancedScalar,
        /// The declared increasable magnitude (>= 0 MW) with provenance;
        /// the credited offset is the intersection of this magnitude and
        /// the narrowed declared range's positive side.
        increasable_mw: ProvenancedScalar,
    },
    /// Dispatchable (BESS / EV fleet): the narrowed declared range scaled
    /// by availability, optionally intersected with the conservative
    /// energy cap. EV-fleet assets carry the recorded limitation that
    /// per-vehicle connection windows and the hard departure constraint
    /// (D-P3.11-2) are enforced by the schedule formulation (P3.17), not
    /// by this per-interval range.
    Dispatchable,
}

/// One asset's input to the portfolio envelope computation.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetEnvelopeInput<'a> {
    /// The asset identifier (registry identity, P2.02).
    pub asset_id: AssetId,
    /// The asset's site (the coupling domain).
    pub site_id: SiteId,
    /// The asset class (drives the P3.02 conservative-default rows).
    pub class: AssetClass,
    /// The registry capability-profile reference string (P2.02; recorded
    /// in the input snapshot — Phase 2 carries the reference only).
    pub capability_profile_reference: String,
    /// The declared feasible P_net range, MW, export-positive (§10.2),
    /// finite on both sides. For `ForecastBaselineLoad` this is the
    /// declared OFFSET range around the forecast baseline. Narrowed by the
    /// frozen P3.02 rule.
    pub declared_p_net_mw: BoundInterval,
    /// The provenance of the declared range.
    pub p_net_provenance: CapabilityProvenance,
    /// The availability input (recorded source or unknown-narrows-to-zero).
    pub availability: AssetAvailabilityInput<'a>,
    /// The contribution kind.
    pub contribution: AssetContribution<'a>,
    /// The conservative time-coupled energy cap (required for BESS-class
    /// dispatchable assets; recorded as an unknown dimension when absent
    /// for an EV fleet).
    pub energy_coupling: Option<EnergyCouplingInput>,
}

/// One site's coupling input: the shared constraint that binds the
/// aggregate at the point of common coupling (§11.6 "site/network coupling
/// constraints"). The PCC and grid-model references come from the P2.02
/// site entity; the numeric limits are required configuration (gap G17:
/// the registry holds no transformer limit).
#[derive(Debug, Clone, PartialEq)]
pub struct SiteCouplingInput {
    /// The site identifier.
    pub site_id: SiteId,
    /// The point-of-common-coupling reference (from `Site`).
    pub point_of_common_coupling: String,
    /// The grid-model reference (from `Site`; recorded — the numeric grid
    /// model is a Phase 4 feasibility concern, §12.4–§12.5).
    pub grid_model_reference: String,
    /// The declared maximum aggregate export at the PCC, MW (>= 0), with
    /// provenance. `None` = no declared export limit (recorded).
    pub export_limit_mw: Option<ProvenancedScalar>,
    /// The declared maximum aggregate import magnitude at the PCC, MW
    /// (>= 0), with provenance. `None` = no declared import limit
    /// (recorded).
    pub import_limit_mw: Option<ProvenancedScalar>,
}

/// A declared shared-exposure group (§11.6: shared weather exposure, site
/// limits, communication paths, common operating restrictions). The derate
/// fraction is CONFIGURED with a MANDATORY recorded basis (labelled
/// ASSUMPTION); no correlation coefficient is ever generated here.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedExposureGroup {
    /// The group identifier (recorded).
    pub group_id: String,
    /// The member assets (each must exist and belong to exactly one group).
    pub asset_ids: Vec<AssetId>,
    /// The conservative coincident derate fraction in [0, 1]: both bounds
    /// of every member's per-interval range are scaled by (1 − f) toward
    /// zero before the site aggregation.
    pub derate_fraction: f64,
    /// The recorded basis of the derate (must state the assumption, e.g.
    /// "ASSUMPTION: shared clear-sky exposure; conservative coincident
    /// derate; no correlation coefficient on record").
    pub basis: String,
}

/// The loss/adjustment term P_loss/adjustment(t) (§11.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LossAdjustment {
    /// No loss/adjustment model on record: the term is 0.0 and the omission
    /// is a recorded unknown dimension (omitting losses may overstate
    /// export capability — stated, never hidden; no value invented).
    Unmodelled,
    /// A declared constant adjustment subtracted from the aggregate
    /// (positive MW = additional consumption/loss; `P > 0` export). An
    /// UNKNOWN-provenance constant is NOT credited (treated as
    /// `Unmodelled`, recorded) — unknown never widens.
    Constant {
        /// The constant adjustment, MW.
        mw: f64,
        /// The provenance of the declared value.
        provenance: CapabilityProvenance,
    },
}

/// The complete input of one envelope computation.
pub struct EnvelopeComputationInput<'a> {
    /// The envelope version string (validated here into [`EnvelopeVersion`]).
    pub envelope_version: &'a str,
    /// The computation timestamp (UTC, P0.09).
    pub computed_at: EventTime,
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    /// The scheduling grid (P3.03 semantics). The fixture must match this
    /// grid exactly (start and configuration) — D-FX-1.
    pub grid: SchedulingGrid,
    /// The validated forecast fixture (the single forecast source, P3.31).
    /// The caller obtained it through `validate_forecast_input` (four-place
    /// labelling, place 3); every value read here again carries the label.
    pub fixture: &'a ForecastFixture,
    /// The asset inputs.
    pub assets: Vec<AssetEnvelopeInput<'a>>,
    /// The site coupling inputs (one per site holding assets).
    pub site_couplings: Vec<SiteCouplingInput>,
    /// The declared shared-exposure groups (possibly empty; members of no
    /// group are recorded as correlation-unmodelled).
    pub correlation_groups: Vec<SharedExposureGroup>,
    /// The loss/adjustment treatment.
    pub loss_adjustment: LossAdjustment,
    /// The reference nameplate sum (Σ rated power of the assets, MW) for
    /// the gate comparison against this envelope. Recorded on the output;
    /// the envelope never uses it in the computation.
    pub nameplate_sum_mw: f64,
}

// --- Outputs -----------------------------------------------------------------

/// Why one interval's envelope is infeasible (never repaired by widening).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfeasibilityCause {
    /// An asset's narrowed feasible set was empty (the frozen P3.02 rule:
    /// empty intersection = infeasible, never widened back).
    AssetDimensionEmpty { asset_id: AssetId },
    /// A site's export/import caps crossed the members' aggregate range.
    SiteRangeEmpty { site_id: SiteId },
}

/// The status of one envelope interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeIntervalStatus {
    /// The interval has a non-empty feasible aggregate range.
    Feasible,
    /// The interval is infeasible. The recorded bounds are [0, 0] (the
    /// conservative reading: no capability is credited); the cause names
    /// the infeasibility.
    Infeasible { cause: InfeasibilityCause },
}

/// One interval of the portfolio envelope, MW, export-positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeInterval {
    /// The interval index on the scheduling grid.
    pub index: usize,
    /// The start-inclusive interval start, UTC millis.
    pub start_millis: i64,
    /// The inclusive lower bound of the aggregate P_portfolio, MW (most
    /// negative = maximum import).
    pub lower_mw: f64,
    /// The inclusive upper bound of the aggregate P_portfolio, MW (largest
    /// export).
    pub upper_mw: f64,
    /// The central estimate: the midpoint of the feasible range (the
    /// recorded central-estimate convention; no confidence number is
    /// attached — see `AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED`).
    pub central_estimate_mw: f64,
    /// The interval status.
    pub status: EnvelopeIntervalStatus,
}

/// The recorded correlation treatment of one computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationTreatmentRecord {
    /// The recorded statement (`CORRELATION_TREATMENT_STATEMENT`).
    pub statement: &'static str,
    /// The applied groups (ids, in input order).
    pub applied_groups: Vec<String>,
    /// The assets of NO group: no correlation model on record, none assumed.
    pub unmodelled_assets: Vec<AssetId>,
}

/// A recorded unknown dimension of the envelope (never silently ignored,
/// never filled with an invented value — the P3.02/P3.05 discipline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeUnknownKind {
    /// The loss/adjustment term is unmodelled (term 0.0, may overstate
    /// export capability).
    LossAdjustmentUnmodelled,
    /// No portfolio-level coupling limit exists in the data; only per-site
    /// PCC coupling is applied.
    PortfolioLevelLimitUnknown,
    /// Shared communication-path constraints are not modelled (no data).
    CommunicationPathUnknown,
    /// The portfolio envelope is ACTIVE-POWER only (the §11.6 formula); no
    /// reactive envelope is composed. Per-asset reactive capability stays
    /// in the P3.08/P3.09 models, where unknown volt-var provenance
    /// narrows the Q credit to zero (D-P3.09-1).
    ReactiveEnvelopeNotComposed,
    /// An asset had no evidenced availability and credited nothing.
    AvailabilityUnknownNarrowedToZero,
    /// An EV-fleet asset was supplied without an energy cap; its
    /// per-interval range ignores connection windows and the hard
    /// departure constraint (D-P3.11-2), which the P3.17 formulation must
    /// enforce.
    EvEnergyCouplingAbsent,
    /// The declared loss/adjustment constant carried UNKNOWN provenance
    /// and was not credited (unknown never widens).
    LossAdjustmentUnknownNotCredited,
}

/// A recorded unknown dimension with its affected asset (when per-asset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeUnknownDimension {
    /// The dimension kind.
    pub kind: EnvelopeUnknownKind,
    /// The affected asset, when the dimension is per-asset.
    pub asset_id: Option<AssetId>,
}

/// The input snapshot of one envelope: the recorded references to every
/// input family. The forecast provenance label is carried here — the
/// envelope-input place of the four-place synthetic-labelling rule
/// (D-FX-8): no envelope exists without its forecast label and version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeInputSnapshot {
    /// The fixture identity (`FIXTURE_ID`).
    pub fixture_id: String,
    /// The forecast version the envelope consumed (never bare values).
    pub forecast_version: ForecastVersion,
    /// The forecast provenance label (always `Synthetic` in v0.2.0).
    pub forecast_provenance: ForecastProvenance,
    /// The per-asset capability-profile references (P2.02 reference
    /// strings, input order).
    pub capability_profile_references: Vec<String>,
    /// The site PCC references the coupling was declared against.
    pub site_pcc_references: Vec<String>,
}

impl EnvelopeInputSnapshot {
    /// The deterministic input-snapshot reference string carried into the
    /// envelope metadata (P3.13) and the event idempotency context.
    pub fn reference(&self, portfolio_id: PortfolioId, asset_count: usize) -> String {
        format!(
            "envelope_input_snapshot:{}:{}:{}:assets={}",
            self.fixture_id,
            self.forecast_version,
            portfolio_id,
            asset_count
        )
    }
}

/// The metadata every envelope carries for the staleness discipline
/// (P3.13): version, computation timestamp, input-snapshot reference,
/// forecast version, horizon, and the scoped assets. Schedules reference
/// the envelope version they used (P3.21 consumes this type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeMetadata {
    /// The envelope version.
    pub envelope_version: EnvelopeVersion,
    /// The computation timestamp.
    pub computed_at: EventTime,
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    /// The deterministic input-snapshot reference.
    pub input_snapshot_ref: String,
    /// The forecast version the envelope consumed.
    pub forecast_version: String,
    /// The start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// The end-exclusive horizon end, UTC millis.
    pub horizon_end_millis: i64,
    /// The assets in scope (sorted, deduplicated).
    pub scoped_asset_ids: Vec<AssetId>,
}

/// The computed portfolio flexibility envelope (the P3.12 deliverable).
#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioFlexibilityEnvelope {
    envelope_version: EnvelopeVersion,
    computed_at: EventTime,
    tenant_id: TenantId,
    portfolio_id: PortfolioId,
    horizon_start_millis: i64,
    horizon_end_millis: i64,
    interval_millis: i64,
    intervals: Vec<EnvelopeInterval>,
    input_snapshot: EnvelopeInputSnapshot,
    correlation: CorrelationTreatmentRecord,
    unknown_dimensions: Vec<EnvelopeUnknownDimension>,
    availability_descriptor: &'static str,
    nameplate_sum_mw: f64,
    scoped_asset_ids: Vec<AssetId>,
}

impl PortfolioFlexibilityEnvelope {
    /// The envelope version.
    pub fn envelope_version(&self) -> &EnvelopeVersion {
        &self.envelope_version
    }

    /// The computation timestamp.
    pub const fn computed_at(&self) -> EventTime {
        self.computed_at
    }

    pub const fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub const fn portfolio_id(&self) -> PortfolioId {
        self.portfolio_id
    }

    /// The start-inclusive horizon start, UTC millis.
    pub const fn horizon_start_millis(&self) -> i64 {
        self.horizon_start_millis
    }

    /// The end-exclusive horizon end, UTC millis.
    pub const fn horizon_end_millis(&self) -> i64 {
        self.horizon_end_millis
    }

    /// The interval length, millis.
    pub const fn interval_millis(&self) -> i64 {
        self.interval_millis
    }

    /// The per-interval envelope, in grid order.
    pub fn intervals(&self) -> &[EnvelopeInterval] {
        &self.intervals
    }

    /// The input snapshot (forecast version + synthetic label + references).
    pub const fn input_snapshot(&self) -> &EnvelopeInputSnapshot {
        &self.input_snapshot
    }

    /// The recorded correlation treatment.
    pub const fn correlation_treatment(&self) -> &CorrelationTreatmentRecord {
        &self.correlation
    }

    /// The recorded unknown dimensions.
    pub fn unknown_dimensions(&self) -> &[EnvelopeUnknownDimension] {
        &self.unknown_dimensions
    }

    /// The availability/confidence descriptor (explicit "not supported").
    pub const fn availability_descriptor(&self) -> &'static str {
        self.availability_descriptor
    }

    /// The reference nameplate sum the envelope is compared against (the
    /// gate: a binding shared constraint must make the envelope strictly
    /// less than this sum — see the tests).
    pub const fn nameplate_sum_mw(&self) -> f64 {
        self.nameplate_sum_mw
    }

    /// The assets in scope (sorted).
    pub fn scoped_asset_ids(&self) -> Vec<AssetId> {
        let mut ids = self.scoped_asset_ids.clone();
        ids.sort();
        ids
    }

    /// The P3.13 metadata of this envelope (version, timestamp,
    /// input-snapshot reference, horizon, scoped assets).
    pub fn metadata(&self) -> EnvelopeMetadata {
        EnvelopeMetadata {
            envelope_version: self.envelope_version.clone(),
            computed_at: self.computed_at,
            tenant_id: self.tenant_id,
            portfolio_id: self.portfolio_id,
            input_snapshot_ref: self.input_snapshot.reference(
                self.portfolio_id,
                self.input_snapshot.capability_profile_references.len(),
            ),
            forecast_version: self.input_snapshot.forecast_version.as_str().to_string(),
            horizon_start_millis: self.horizon_start_millis,
            horizon_end_millis: self.horizon_end_millis,
            scoped_asset_ids: self.scoped_asset_ids(),
        }
    }
}

// --- Helpers -----------------------------------------------------------------

/// Applies the frozen P3.02 rule to a declared non-negative scalar: the
/// effective credited value is the upper end of the narrowed interval
/// (0 for UNKNOWN provenance via the collapse-to-zero default, and 0 for
/// an empty intersection — never widened).
fn effective_scalar(class: AssetClass, scalar: &ProvenancedScalar) -> f64 {
    let declared = BoundInterval { lower: Some(0.0), upper: Some(scalar.value) };
    match narrow_interval(class, ConstraintKind::ActivePower, scalar.provenance, declared) {
        NarrowedSet::Empty => 0.0,
        NarrowedSet::Interval { upper, .. } => upper.unwrap_or(scalar.value),
    }
}

/// Applies the frozen P3.02 rule to a declared site limit. The recorded
/// conservative-default table has no explicit `SITE_NETWORK` row, so the
/// collapse-to-zero fallback applies identically for every asset class
/// (`conservative_default` is class-divergence-ready but identical today);
/// the class argument is therefore inert and recorded as such.
fn effective_site_limit(limit: &ProvenancedScalar) -> f64 {
    let declared = BoundInterval { lower: Some(0.0), upper: Some(limit.value) };
    match narrow_interval(
        AssetClass::Bess,
        ConstraintKind::SiteNetwork,
        limit.provenance,
        declared,
    ) {
        NarrowedSet::Empty => 0.0,
        NarrowedSet::Interval { upper, .. } => upper.unwrap_or(limit.value),
    }
}

/// Folds a computed raw range into the recorded interval range: empty
/// (beyond tolerance) becomes `None`; a range collapsed to a point within
/// tolerance is clamped to that point.
fn finalize_range(lower: f64, upper: f64) -> Option<(f64, f64)> {
    if upper < lower - ENVELOPE_TOLERANCE {
        None
    } else if upper < lower {
        Some((lower, lower))
    } else {
        Some((lower, upper))
    }
}

/// Validates a labelled forecast series against the envelope grid.
fn validate_series(
    series: &LabeledForecastSeries,
    expected: ForecastQuantity,
    interval_count: usize,
) -> Result<(), EnvelopeError> {
    let values = series.values();
    if values.quantity != expected {
        return Err(EnvelopeError::ForecastQuantityMismatch {
            asset_key: series.asset_key().to_string(),
            expected: expected.wire_name(),
            found: values.quantity.wire_name(),
        });
    }
    if series.len() != interval_count {
        return Err(EnvelopeError::ForecastSeriesIntervalCountMismatch {
            asset_key: series.asset_key().to_string(),
            expected: interval_count,
            found: series.len(),
        });
    }
    Ok(())
}

/// Validates the conservative energy-coupling input.
fn validate_energy_coupling(
    asset_id: AssetId,
    coupling: EnergyCouplingInput,
) -> Result<(), EnvelopeError> {
    let invalid = |reason: &'static str| EnvelopeError::EnergyCouplingInvalid {
        asset_id,
        reason,
    };
    if !coupling.e_min_mwh.is_finite() || !coupling.e_max_mwh.is_finite() {
        return Err(invalid("energy window bounds must be finite"));
    }
    if coupling.e_min_mwh > coupling.e_max_mwh {
        return Err(invalid("the energy window must satisfy e_min <= e_max"));
    }
    if !coupling.e_initial_mwh.is_finite() {
        return Err(invalid("the initial energy must be finite"));
    }
    if !coupling.eta_ch.is_finite() || !(0.0 < coupling.eta_ch && coupling.eta_ch <= 1.0) {
        return Err(invalid("eta_ch must be in (0, 1]"));
    }
    if !coupling.eta_dis.is_finite() || !(0.0 < coupling.eta_dis && coupling.eta_dis <= 1.0) {
        return Err(invalid("eta_dis must be in (0, 1]"));
    }
    Ok(())
}

/// Validates one asset input (static parts; the per-interval loop reads the
/// series values, whose lengths are validated here).
fn validate_asset_input(
    asset: &AssetEnvelopeInput<'_>,
    interval_count: usize,
) -> Result<(), EnvelopeError> {
    if asset.capability_profile_reference.trim().is_empty() {
        return Err(EnvelopeError::EmptyCapabilityProfileReference {
            asset_id: asset.asset_id,
        });
    }
    let (lo, hi) = match (asset.declared_p_net_mw.lower, asset.declared_p_net_mw.upper) {
        (Some(l), Some(u)) => (l, u),
        _ => return Err(EnvelopeError::UnboundedDeclaredRange { asset_id: asset.asset_id }),
    };
    if !lo.is_finite() || !hi.is_finite() {
        return Err(EnvelopeError::NonFiniteDeclaredBound { asset_id: asset.asset_id });
    }
    if lo > hi {
        return Err(EnvelopeError::InvertedDeclaredRange { asset_id: asset.asset_id });
    }
    match &asset.availability {
        AssetAvailabilityInput::Declared { fractions, source } => {
            if source.trim().is_empty() {
                return Err(EnvelopeError::EmptyAvailabilitySource { asset_id: asset.asset_id });
            }
            if fractions.len() != interval_count {
                return Err(EnvelopeError::AvailabilityCountMismatch {
                    asset_id: asset.asset_id,
                    expected: interval_count,
                    found: fractions.len(),
                });
            }
            for (index, value) in fractions.iter().enumerate() {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(EnvelopeError::AvailabilityOutOfRange {
                        asset_id: asset.asset_id,
                        index,
                        value: *value,
                    });
                }
            }
        }
        AssetAvailabilityInput::Unknown => {}
    }
    match &asset.contribution {
        AssetContribution::ForecastCappedPv { series } => {
            validate_series(series, ForecastQuantity::PvOutputMw, interval_count)?;
        }
        AssetContribution::ForecastBaselineLoad { series, reducible_mw, increasable_mw } => {
            validate_series(series, ForecastQuantity::LoadMw, interval_count)?;
            for scalar in [reducible_mw, increasable_mw] {
                if !scalar.value.is_finite() || scalar.value < 0.0 {
                    return Err(EnvelopeError::NonPositiveOffset {
                        asset_id: asset.asset_id,
                        value: scalar.value,
                    });
                }
            }
        }
        AssetContribution::Dispatchable => {
            if let Some(coupling) = &asset.energy_coupling {
                validate_energy_coupling(asset.asset_id, *coupling)?;
            } else if asset.class == AssetClass::Bess {
                return Err(EnvelopeError::EnergyCouplingRequiredForBess {
                    asset_id: asset.asset_id,
                });
            }
        }
    }
    Ok(())
}

/// Validates the site couplings.
fn validate_site_couplings(sites: &[SiteCouplingInput]) -> Result<(), EnvelopeError> {
    let mut seen: Vec<SiteId> = Vec::new();
    for site in sites {
        if seen.contains(&site.site_id) {
            return Err(EnvelopeError::DuplicateSiteCoupling { site_id: site.site_id });
        }
        seen.push(site.site_id);
        if site.point_of_common_coupling.trim().is_empty()
            || site.grid_model_reference.trim().is_empty()
        {
            return Err(EnvelopeError::EmptySiteReference { site_id: site.site_id });
        }
        for limit in [&site.export_limit_mw, &site.import_limit_mw].into_iter().flatten() {
            if !limit.value.is_finite() || limit.value < 0.0 {
                return Err(EnvelopeError::SiteLimitInvalid {
                    site_id: site.site_id,
                    value: limit.value,
                });
            }
        }
    }
    Ok(())
}

/// Validates the shared-exposure groups and returns the per-asset derate
/// factor map (1 − f per member) and the applied group ids.
fn validate_groups(
    groups: &[SharedExposureGroup],
    assets: &[AssetEnvelopeInput<'_>],
) -> Result<(HashMap<AssetId, f64>, Vec<String>), EnvelopeError> {
    let mut factors: HashMap<AssetId, f64> = HashMap::new();
    let mut applied = Vec::with_capacity(groups.len());
    for group in groups {
        if group.group_id.trim().is_empty() {
            return Err(EnvelopeError::InvalidSharedExposureGroup {
                group_id: group.group_id.clone(),
                reason: "the group id must be non-empty",
            });
        }
        if group.basis.trim().is_empty() {
            return Err(EnvelopeError::InvalidSharedExposureGroup {
                group_id: group.group_id.clone(),
                reason: "the derate basis must be recorded (labelled ASSUMPTION; no correlation coefficient is invented)",
            });
        }
        if group.asset_ids.is_empty() {
            return Err(EnvelopeError::InvalidSharedExposureGroup {
                group_id: group.group_id.clone(),
                reason: "the group must have at least one member",
            });
        }
        if !group.derate_fraction.is_finite() || !(0.0..=1.0).contains(&group.derate_fraction) {
            return Err(EnvelopeError::DerateOutOfRange {
                group_id: group.group_id.clone(),
                value: group.derate_fraction,
            });
        }
        for asset_id in &group.asset_ids {
            if factors.contains_key(asset_id) {
                return Err(EnvelopeError::DuplicateGroupAsset {
                    group_id: group.group_id.clone(),
                    asset_id: *asset_id,
                });
            }
            if !assets.iter().any(|a| &a.asset_id == asset_id) {
                return Err(EnvelopeError::UnknownGroupAsset {
                    group_id: group.group_id.clone(),
                    asset_id: *asset_id,
                });
            }
            factors.insert(*asset_id, 1.0 - group.derate_fraction);
        }
        applied.push(group.group_id.clone());
    }
    Ok((factors, applied))
}

/// Computes one asset's feasible P_net range at interval `index`, narrowed
/// per the frozen P3.02 rule, capped by the forecast and availability, and
/// scaled by the group derate. `None` = the asset's dimension is empty at
/// this interval (infeasible, never widened).
fn asset_range_at(
    asset: &AssetEnvelopeInput<'_>,
    index: usize,
    derate_factor: Option<f64>,
    interval_millis: i64,
) -> Option<(f64, f64)> {
    // 1. The frozen P3.02 narrowing of the declared range. The declared
    //    sides were validated finite and bounded, so the narrowed interval
    //    keeps finite sides.
    let narrowed = narrow_interval(
        asset.class,
        ConstraintKind::ActivePower,
        asset.p_net_provenance,
        asset.declared_p_net_mw,
    );
    let (mut lower, mut upper) = match narrowed {
        NarrowedSet::Empty => return None,
        NarrowedSet::Interval { lower, upper } => (lower, upper),
    };

    // The availability factor (0.0 when unknown — the recorded narrowing:
    // no evidenced availability credits nothing).
    let availability = match &asset.availability {
        AssetAvailabilityInput::Declared { fractions, .. } => fractions[index],
        AssetAvailabilityInput::Unknown => 0.0,
    };

    match &asset.contribution {
        AssetContribution::ForecastCappedPv { series } => {
            let forecast = series.values().values[index];
            // The export side is capped by the forecast output scaled by
            // availability; the narrowed lower bound stays.
            let cap = forecast * availability;
            upper = Some(upper.unwrap_or(f64::INFINITY).min(cap));
            lower = Some(lower.unwrap_or(0.0));
        }
        AssetContribution::ForecastBaselineLoad { series, reducible_mw, increasable_mw } => {
            let baseline = series.values().values[index];
            let reduce = effective_scalar(asset.class, reducible_mw);
            let increase = effective_scalar(asset.class, increasable_mw);
            // The declared range for this variant IS the declared OFFSET
            // range around the baseline. The effective offset interval is
            // the narrowed declared offsets intersected with the credited
            // magnitudes, then scaled toward zero by availability.
            let off_lower = (lower.unwrap_or(0.0).max(-reduce)) * availability;
            let off_upper = (upper.unwrap_or(0.0).min(increase)) * availability;
            lower = Some(baseline + off_lower);
            upper = Some(baseline + off_upper);
        }
        AssetContribution::Dispatchable => {
            // Scale toward zero by availability.
            lower = Some(lower.unwrap_or(0.0) * availability);
            upper = Some(upper.unwrap_or(0.0) * availability);
            if let Some(coupling) = asset.energy_coupling {
                let dt_hours = interval_millis as f64 / 3_600_000.0;
                let e0 = coupling
                    .e_initial_mwh
                    .clamp(coupling.e_min_mwh, coupling.e_max_mwh);
                // Conservative horizon-start caps: deliverable energy above
                // the floor (NO intra-horizon recharge credit) and
                // absorbable headroom below the ceiling (NO intra-horizon
                // discharge credit).
                let export_cap =
                    ((e0 - coupling.e_min_mwh).max(0.0) * coupling.eta_dis) / dt_hours;
                let import_cap =
                    ((coupling.e_max_mwh - e0).max(0.0) / coupling.eta_ch) / dt_hours;
                upper = Some(upper.unwrap_or(0.0).min(export_cap));
                lower = Some(lower.unwrap_or(0.0).max(-import_cap));
            }
        }
    }

    // 2. The declared shared-exposure derate: both bounds toward zero.
    if let Some(factor) = derate_factor {
        lower = lower.map(|l| l * factor);
        upper = upper.map(|u| u * factor);
    }

    finalize_range(lower.unwrap_or(0.0), upper.unwrap_or(0.0))
}

// --- The computation ---------------------------------------------------------

/// Computes the portfolio flexibility envelope (the P3.12 deliverable).
pub fn calculate_portfolio_envelope(
    input: EnvelopeComputationInput<'_>,
) -> Result<PortfolioFlexibilityEnvelope, EnvelopeError> {
    let envelope_version = EnvelopeVersion::new(input.envelope_version)?;
    if input.assets.is_empty() {
        return Err(EnvelopeError::NoAssets);
    }
    if input.site_couplings.is_empty() {
        return Err(EnvelopeError::NoSiteCouplings);
    }
    if !input.nameplate_sum_mw.is_finite() || input.nameplate_sum_mw < 0.0 {
        return Err(EnvelopeError::NonFiniteNameplateSum);
    }
    let interval_count = input.grid.interval_count();
    let interval_millis = input.grid.config().interval_millis();

    // The fixture must be the same grid (D-FX-1): identical start and
    // configuration, hence identical boundaries.
    let fixture_start = input.fixture.grid().horizon_start_millis();
    if input.grid.start().unix_millis() != i128::from(fixture_start)
        || !input.fixture.grid().matches_config(input.grid.config())
    {
        return Err(EnvelopeError::ForecastGridMismatch {
            envelope_start_millis: input.grid.start().unix_millis(),
            fixture_start_millis: fixture_start,
        });
    }

    // Static validation.
    let mut seen_assets: Vec<AssetId> = Vec::new();
    for asset in &input.assets {
        if seen_assets.contains(&asset.asset_id) {
            return Err(EnvelopeError::DuplicateAsset { asset_id: asset.asset_id });
        }
        seen_assets.push(asset.asset_id);
        validate_asset_input(asset, interval_count)?;
    }
    validate_site_couplings(&input.site_couplings)?;
    for asset in &input.assets {
        if !input.site_couplings.iter().any(|s| s.site_id == asset.site_id) {
            return Err(EnvelopeError::MissingSiteCoupling { site_id: asset.site_id });
        }
    }
    let (derate_factors, applied_groups) =
        validate_groups(&input.correlation_groups, &input.assets)?;

    // The loss/adjustment term and its recorded state.
    let mut unknown_dimensions: Vec<EnvelopeUnknownDimension> = vec![
        EnvelopeUnknownDimension {
            kind: EnvelopeUnknownKind::PortfolioLevelLimitUnknown,
            asset_id: None,
        },
        EnvelopeUnknownDimension {
            kind: EnvelopeUnknownKind::CommunicationPathUnknown,
            asset_id: None,
        },
        EnvelopeUnknownDimension {
            kind: EnvelopeUnknownKind::ReactiveEnvelopeNotComposed,
            asset_id: None,
        },
    ];
    let loss_mw = match input.loss_adjustment {
        LossAdjustment::Unmodelled => {
            unknown_dimensions.push(EnvelopeUnknownDimension {
                kind: EnvelopeUnknownKind::LossAdjustmentUnmodelled,
                asset_id: None,
            });
            0.0
        }
        LossAdjustment::Constant { mw, provenance } => {
            if !mw.is_finite() {
                return Err(EnvelopeError::NonFiniteLossAdjustment);
            }
            if provenance == CapabilityProvenance::Unknown {
                unknown_dimensions.push(EnvelopeUnknownDimension {
                    kind: EnvelopeUnknownKind::LossAdjustmentUnknownNotCredited,
                    asset_id: None,
                });
                0.0
            } else {
                mw
            }
        }
    };
    for asset in &input.assets {
        if matches!(asset.availability, AssetAvailabilityInput::Unknown) {
            unknown_dimensions.push(EnvelopeUnknownDimension {
                kind: EnvelopeUnknownKind::AvailabilityUnknownNarrowedToZero,
                asset_id: Some(asset.asset_id),
            });
        }
        if asset.class == AssetClass::EvFleet
            && matches!(asset.contribution, AssetContribution::Dispatchable)
            && asset.energy_coupling.is_none()
        {
            unknown_dimensions.push(EnvelopeUnknownDimension {
                kind: EnvelopeUnknownKind::EvEnergyCouplingAbsent,
                asset_id: Some(asset.asset_id),
            });
        }
    }

    // Assets of no group: recorded as correlation-unmodelled.
    let unmodelled_assets: Vec<AssetId> = input
        .assets
        .iter()
        .filter(|a| !derate_factors.contains_key(&a.asset_id))
        .map(|a| a.asset_id)
        .collect();

    // Per interval: per-site constrained aggregation, then the portfolio sum.
    let mut intervals = Vec::with_capacity(interval_count);
    for index in 0..interval_count {
        let start_millis = input
            .grid
            .interval_bounds(index)
            .map(|(s, _)| i64::try_from(s.unix_millis()).expect("grid bounds stay in range"))
            .expect("in-range index");
        let mut portfolio_lower = 0.0f64;
        let mut portfolio_upper = 0.0f64;
        let mut cause: Option<InfeasibilityCause> = None;

        'sites: for site in &input.site_couplings {
            let mut site_lower = 0.0f64;
            let mut site_upper = 0.0f64;
            for asset in input.assets.iter().filter(|a| a.site_id == site.site_id) {
                match asset_range_at(
                    asset,
                    index,
                    derate_factors.get(&asset.asset_id).copied(),
                    interval_millis,
                ) {
                    Some((lo, hi)) => {
                        site_lower += lo;
                        site_upper += hi;
                    }
                    None => {
                        cause = Some(InfeasibilityCause::AssetDimensionEmpty {
                            asset_id: asset.asset_id,
                        });
                        break 'sites;
                    }
                }
            }
            // The shared site coupling binds the aggregate here (§11.6):
            // the PCC export/import caps apply to the site SUM, not to the
            // individual assets.
            if let Some(export) = &site.export_limit_mw {
                site_upper = site_upper.min(effective_site_limit(export));
            }
            if let Some(import) = &site.import_limit_mw {
                site_lower = site_lower.max(-effective_site_limit(import));
            }
            match finalize_range(site_lower, site_upper) {
                Some((lo, hi)) => {
                    portfolio_lower += lo;
                    portfolio_upper += hi;
                }
                None => {
                    cause = Some(InfeasibilityCause::SiteRangeEmpty { site_id: site.site_id });
                    break 'sites;
                }
            }
        }

        let status = match cause {
            Some(cause) => EnvelopeIntervalStatus::Infeasible { cause },
            None => EnvelopeIntervalStatus::Feasible,
        };
        // An infeasible interval records the conservative [0, 0]; the
        // status field governs the reading.
        let (lower, upper) = match status {
            EnvelopeIntervalStatus::Feasible => {
                (portfolio_lower - loss_mw, portfolio_upper - loss_mw)
            }
            EnvelopeIntervalStatus::Infeasible { .. } => (0.0, 0.0),
        };
        let (lower, upper) = finalize_range(lower, upper).unwrap_or((0.0, 0.0));
        let central_estimate = (lower + upper) / 2.0;
        intervals.push(EnvelopeInterval {
            index,
            start_millis,
            lower_mw: lower,
            upper_mw: upper,
            central_estimate_mw: central_estimate,
            status,
        });
    }

    // The input snapshot: the forecast version and the SYNTHETIC label are
    // recorded here — the envelope-input place of the four-place rule.
    let input_snapshot = EnvelopeInputSnapshot {
        fixture_id: FIXTURE_ID.to_string(),
        forecast_version: input.fixture.forecast_version().clone(),
        forecast_provenance: input.fixture.meta().provenance(),
        capability_profile_references: input
            .assets
            .iter()
            .map(|a| a.capability_profile_reference.clone())
            .collect(),
        site_pcc_references: input
            .site_couplings
            .iter()
            .map(|s| s.point_of_common_coupling.clone())
            .collect(),
    };

    let horizon_start_millis =
        i64::try_from(input.grid.start().unix_millis()).expect("grid start in range");
    let horizon_end_millis = horizon_start_millis + input.grid.config().horizon_millis();

    Ok(PortfolioFlexibilityEnvelope {
        scoped_asset_ids: input.assets.iter().map(|a| a.asset_id).collect(),
        envelope_version,
        computed_at: input.computed_at,
        tenant_id: input.tenant_id,
        portfolio_id: input.portfolio_id,
        horizon_start_millis,
        horizon_end_millis,
        interval_millis,
        intervals,
        input_snapshot,
        correlation: CorrelationTreatmentRecord {
            statement: CORRELATION_TREATMENT_STATEMENT,
            applied_groups,
            unmodelled_assets,
        },
        unknown_dimensions,
        availability_descriptor: AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED,
        nameplate_sum_mw: input.nameplate_sum_mw,
    })
}

// --- FlexibilityEnvelopeUpdated assembly (outbox pattern, pure) ---------------

/// The inputs of one `FlexibilityEnvelopeUpdated` publication batch: one
/// event per covered site (the frozen payload is site-scoped; the
/// portfolio envelope covers the listed sites).
pub struct EnvelopePublicationInput<'a> {
    /// The correlation identifier (per-use-case invocation).
    pub correlation_id: FlexUuid,
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    /// The covered sites (one event each, in list order).
    pub site_ids: &'a [SiteId],
    /// The envelope version being published (validated).
    pub envelope_version: &'a EnvelopeVersion,
    /// The event time (the envelope computation time).
    pub event_time: EventTime,
}

/// One assembled `FlexibilityEnvelopeUpdated` publication: the versioned
/// envelope, its destination subject (the frozen P1.14 scheme), and the
/// idempotency key. Pure assembly only — the wiring into the transactional
/// outbox (state-change receipt, encoder, `EventOutbox` port) lands with
/// P3.28 through `event_assembly`.
pub struct EnvelopePublication {
    /// The versioned envelope carrying the frozen
    /// `FlexibilityEnvelopeUpdated` event.
    pub envelope: EventEnvelope,
    /// The destination subject:
    /// `flexcore.v1.flexibility.flexibility_envelope_updated`.
    pub destination_subject: Subject,
    /// The idempotency key, deterministic from the portfolio, the site, and
    /// the envelope version:
    /// `flexibility_envelope_updated:<portfolio>:<site>:<envelope_version>`.
    /// Republication of one envelope version for one site dedupes durably.
    pub idempotency_key: String,
}

/// Assembles the `FlexibilityEnvelopeUpdated` publications for one envelope
/// version across the covered sites (§15.3; outbox pattern only — pure
/// assembly, no I/O). See the module header for the recorded payload-fit
/// gap (`ENVELOPE_PAYLOAD_GAP_NOTE`).
pub fn envelope_updated_publications(
    input: EnvelopePublicationInput<'_>,
) -> Result<Vec<EnvelopePublication>, EnvelopeError> {
    if input.site_ids.is_empty() {
        return Err(EnvelopeError::PublicationWithoutSite);
    }
    let mut seen: Vec<SiteId> = Vec::new();
    let mut publications = Vec::with_capacity(input.site_ids.len());
    for site_id in input.site_ids {
        if seen.contains(site_id) {
            return Err(EnvelopeError::DuplicatePublicationSite { site_id: *site_id });
        }
        seen.push(*site_id);
        // The frozen payload is asset-scoped; an envelope update is not
        // attributable to one asset. The nil asset id denotes the envelope
        // scope (recorded convention; see ENVELOPE_PAYLOAD_GAP_NOTE).
        let payload = FlexibilityEnvelopeUpdatedPayload {
            correlation_id: input.correlation_id,
            tenant_id: input.tenant_id,
            portfolio_id: input.portfolio_id,
            site_id: *site_id,
            asset_id: AssetId::new(FlexUuid::nil()),
            event_time: input.event_time,
        };
        let key = format!(
            "flexibility_envelope_updated:{}:{}:{}",
            input.portfolio_id,
            site_id,
            input.envelope_version
        );
        let event = DomainEvent::FlexibilityEnvelopeUpdated(payload);
        let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &key, event)
            .map_err(|_| EnvelopeError::EventEnvelopeRejected)?;
        let destination_subject = subject_for(&envelope.event);
        publications.push(EnvelopePublication {
            idempotency_key: envelope.idempotency_key.clone(),
            destination_subject,
            envelope,
        });
    }
    Ok(publications)
}
