// P3.31 — Synthetic forecast fixture: labeled schema, deterministic
//         generator, envelope input validation, `ForecastPublished`
//         assembly (pure).
//
// Implements ledger P3.31 ("Synthetic forecast fixture — owned by Phase 3;
// no Phase 2 forecast exists", report §11.6, §12.1, §15.3 `ForecastPublished`;
// P2.28 forecast non-delivery). Decision record:
// `ledger/phase-03-flexibility-scheduling/31_forecast_fixture.md`.
//
// THE FIXTURE, DECIDED ONCE:
//
// - The fixture is the **single forecast source for v0.2.0** (ledger P3.31):
//   per-asset per-interval forecast values (PV output MW, load MW, availability
//   fraction) over the P3.03 scheduling grid, generated deterministically from
//   the P2.13 exit-gate scenario (`scenario-v2.json`) — same horizon, same
//   cadence (60 s), same master seed. Two versions exist, versioned SEPARATELY:
//   the central forecast `v1` and the P3.26 error variant `v1-error-pv-ramp`
//   (stated magnitude, see [`ForecastErrorVariant`]).
// - **Every value is labelled synthetic, and the label cannot be dropped**:
//   forecast values are only accessible through [`LabeledForecastSeries::values`],
//   which returns [`LabeledSeriesValues`] — the provenance label, the forecast
//   version, and the quantity travel WITH the values on every access. There is
//   no accessor that returns bare values. This is place 1 and 2 of the
//   four-place labelling rule (schema + type); place 3 is the envelope input
//   validation ([`validate_forecast_input`], which P3.12 calls and which
//   REJECTS an unlabelled or unversioned forecast with a named error); place 4
//   is the P3.21 result record (downstream obligation, recorded in the decision
//   doc).
// - **Canonical units and sign** (§10.2): MW, `P > 0` = export at the point of
//   connection. PV output is positive (export); load is negative
//   (consumption). Availability is a fraction in [0, 1]. Forecasts are never
//   presented as measurements or market prices, and the fixture carries no
//   price data (D1/D2 OPEN).
// - The forecast grid uses the P3.03 semantics (start-inclusive, end-exclusive,
//   UTC-only, horizon an exact integer multiple of the interval) with the
//   scenario's cadence (60 s) as the interval length — a valid
//   [`SchedulingGridConfig`]; the P3.03 default (15 min) aggregation consumes
//   this fixture through the recorded sub-interval aggregation rules.
//
// DETERMINISM AND THE COMMITTED ARTEFACTS:
//
// - Same master seed → byte-identical fixture: the generator is pure (no wall
//   clock, no ambient entropy), the per-series streams derive as
//   `seed(stream_key) = master_seed XOR splitmix64(FNV-1a-64(stream_key))`
//   with `stream_key = <asset_key>/forecast/<forecast_version>` — the P2.14
//   rule applied to a namespaced stream key, so adding an asset or a forecast
//   version never perturbs another series' stream. Each power series draws
//   exactly one splitmix64 value per interval, in interval order.
// - Values are quantized to whole milli units (round half away from zero) and
//   the JSON writer renders them with exactly three decimals, so the artefact
//   is byte-reproducible **without any float formatting** in the file.
// - The committed artefacts
//   (`scenarios/synthetic-portfolio/forecast-fixture-v1.json` and
//   `forecast-fixture-v1-error-pv-ramp.json`) are **RECORDED-OUTPUT**: they
//   were produced by replicating this module's algorithm outside cargo (the
//   generator was not built in the authoring session). VERIFY-BUILD: the
//   integration test `committed_artefact_matches_generator` (and the error
//   variant twin) asserts byte equality between [`to_json`] output and the
//   committed file; it must pass on the first build. The one platform-dependent
//   piece is the trigonometric shape evaluation (see [`pv_shape`]); any libm
//   1-ulp difference large enough to cross a milli-quantization boundary
//   surfaces there as a test failure, not as silent drift.
//
// SCOPE (recorded): the fixture carries forecast series for the scenario's PV
// and flexible-load assets only. BESS and EVSE setpoints are schedule
// decisions (P3.21/P3.22), not forecast inputs; site meters are aggregates.
// The scenarios' behaviour profiles declare availability constant true, so the
// availability forecast is the constant fraction 1.0. No BESS/EVSE/meter
// forecast series are invented.
//
// GAP (recorded honestly): the frozen `ForecastPublishedPayload` (P0.14 /
// P1.09) carries identity and time only — no forecast version and no
// provenance field. flexcore-events is frozen for this subtask, so the
// version + provenance travel (a) in the envelope's idempotency key
// (deterministic from the forecast version, see [`forecast_publication`]) and
// (b) in the fixture store/artefact itself. The P2.16 precedent (additive
// `serde(default)` payload fields without a schema-version bump) is the
// recorded extension path for a future subtask that owns the events crate.
//
// This module duplicates the P2.14 RNG primitives ([`fnv1a64`],
// [`splitmix64_mix`], [`SplitMix64`]) and the scenario shape formulas
// ([`pv_shape`], [`cosine_bump`], [`load_shape`]) because
// flexcore-application may depend on nothing beyond
// contracts/domain/ports/events (manifest policy), and flexcore-simulator is
// not on that list. The duplication is pinned by test vectors so the two
// implementations cannot drift silently.
//
// Control status: pure assembly for the simulation-only 0.x line. No I/O, no
// persistence, no device-write path; the wiring into a use case lands with
// P3.12/P3.28.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::scheduling_time::{SchedulingGridConfig, SchedulingTimeError};
use flexcore_events::envelope::{
    EnvelopeError, EventEnvelope, IdempotencyKey, SUPPORTED_SCHEMA_VERSION,
};
use flexcore_events::events::{DomainEvent, ForecastPublishedPayload};
use flexcore_events::subject::{subject_for, Subject};

/// The mandatory provenance label: the only provenance this crate produces.
/// Forecast data is synthetic — always, by construction (D1/D2 OPEN; 0.x
/// simulation-only line).
pub const PROVENANCE_SYNTHETIC: &str = "synthetic";

/// The fixture identity rendered in the committed artefacts.
pub const FIXTURE_ID: &str = "synthetic-forecast-fixture";

/// The central-forecast variant name (version `v1`).
pub const CENTRAL_VARIANT: &str = "central";

/// The P3.26 PV-ramp / forecast-error variant name (version
/// `v1-error-pv-ramp`).
pub const ERROR_PV_RAMP_VARIANT: &str = "error-pv-ramp";

// --- Recorded prose carried verbatim into the committed artefacts ------------
//
// These strings are part of the byte-identity contract with the committed
// JSON: change one and the artefact match test fails (which is the intended
// drift alarm — a changed method description means a new fixture version).

const PROVENANCE_NOTE: &str = "SYNTHETIC forecast data generated from the versioned exit-gate scenario; not measured data, not a market price, and never presented as either.";
const GRID_SEMANTICS_DOC: &str =
    "start-inclusive, end-exclusive (P3.03 scheduling grid semantics)";
const SEED_DERIVATION_DOC: &str = "seed(stream_key) = master_seed XOR splitmix64(FNV-1a-64(stream_key)), stream_key = <asset_key>/forecast/<forecast_version>";
const STREAM_RULE_DOC: &str = "one splitmix64 draw per interval per power series, intervals in order";
const PV_SHAPE_DOC: &str = "clear-sky cosine bell at the interval start instant, per the scenario pv_clear_sky profile: value = rated_power_mw * peak_factor * 0.5 * (1 - cos(2*pi*x)), x = (sod - sunrise) / (sunset - sunrise) on [0, 1), capped at rated_power_mw; 0 outside [sunrise, sunset); sunrise 05:30 UTC, sunset 21:00 UTC";
const LOAD_SHAPE_DOC: &str = "scenario load_baseline_reducible baseline at the interval start instant: night 0.25 * rated_peak_mw + morning 0.45 * rated_peak_mw * cosine_bump(sod, 07:30 UTC, half-width 2.5 h) + evening 0.50 * rated_peak_mw * cosine_bump(sod, 19:00 UTC, half-width 3.0 h), cosine_bump = 0.5 * (1 + cos(pi * delta / half_width)) for delta < half_width else 0, capped at rated_peak_mw; emitted in the canonical net sign (consumption negative, P > 0 export)";
const AVAILABILITY_DOC: &str = "constant 1.000 per interval (fraction in [0, 1]); the scenario declares availability constant true; no noise";
const FORECAST_NOISE_DOC: &str = "value_raw = shape * (1 + u) with u drawn once per interval per power series from the stream keyed <asset_key>/forecast/<forecast_version>; u = low + unit * (high - low), unit = (draw >> 11) / 2^53";
const QUANTIZATION_DOC: &str = "every value is quantized to whole milli units (round half away from zero) and rendered with exactly three decimals, so the artefact is byte-reproducible without float formatting";
const SHAPE_NOTE: &str = "ASSUMPTION: these are the scenario synthetic profiles evaluated on the P3.03 grid semantics, not a weather model, not measured data, and not a validated physical model; the central forecast is not a market price and carries no price data";
const ERROR_VARIANT_NOTE: &str = "Inside the window the PV forecast asserts a downward ramp: value = shape * (1 - error_magnitude) * (1 + u). Against the clear-sky truth the stated forecast-error magnitude is 80.0 percent of the clear-sky value. P3.26 scenario (b) consumes this variant.";

// --- The provenance label and the forecast version ---------------------------

/// The provenance label of a forecast value: an enum with exactly one
/// variant. An unlabelled forecast is represented by the ABSENCE of this
/// type (`Option<Provenance>` in [`UnvalidatedForecastInput`]) — never by a
/// default, an empty string, or a silent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Generated from a versioned synthetic scenario. The only kind of
    /// forecast data that exists in FlexCore v0.2.0.
    Synthetic,
}

impl Provenance {
    /// The canonical label spelling (rendered in the artefact and in every
    /// downstream record).
    pub const fn as_str(self) -> &'static str {
        PROVENANCE_SYNTHETIC
    }
}

/// The forecast version: a validated semantic string (`v1`,
/// `v1-error-pv-ramp`, ...). Non-empty, whitespace-free — envelopes,
/// fixture records, and the idempotency key carry it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForecastVersion(String);

impl ForecastVersion {
    /// Validates and wraps a forecast version.
    pub fn new(version: &str) -> Result<Self, ForecastFixtureError> {
        if version.is_empty() {
            return Err(ForecastFixtureError::EmptyForecastVersion);
        }
        if version.chars().any(|c| c.is_whitespace()) {
            return Err(ForecastFixtureError::ForecastVersionHasWhitespace);
        }
        Ok(Self(version.to_string()))
    }

    /// The version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ForecastVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Quantities ---------------------------------------------------------------

/// The forecast quantity of a series, with its canonical unit and sign rule
/// (§10.2: MW, `P > 0` export; availability is a fraction in [0, 1]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastQuantity {
    /// Forecast PV active-power output at the asset's point of connection:
    /// MW, positive (export).
    PvOutputMw,
    /// Forecast load active power at the asset's point of connection: MW,
    /// negative (consumption; `P > 0` export, so consumption is negative).
    LoadMw,
    /// Forecast availability of the asset: a fraction in [0, 1] (the same
    /// [0, 1] boundary rule as SOC; not a telemetry quality state).
    AvailabilityFraction,
}

impl ForecastQuantity {
    /// The wire spelling of the quantity (rendered in the artefact).
    pub const fn wire_name(self) -> &'static str {
        match self {
            ForecastQuantity::PvOutputMw => "pv_output_mw",
            ForecastQuantity::LoadMw => "load_mw",
            ForecastQuantity::AvailabilityFraction => "availability_fraction",
        }
    }

    /// The canonical unit of the quantity (§10.2).
    pub const fn unit(self) -> &'static str {
        match self {
            ForecastQuantity::PvOutputMw => "MW",
            ForecastQuantity::LoadMw => "MW",
            ForecastQuantity::AvailabilityFraction => "fraction",
        }
    }
}

// --- The grid -----------------------------------------------------------------

/// The fixture's interval grid: the P3.03 scheduling-grid semantics with the
/// generating scenario's cadence as the interval length. Every grid over the
/// same start and configuration has identical interval boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureGrid {
    horizon_start_millis: i64,
    horizon_start_iso: String,
    horizon_millis: i64,
    interval_millis: i64,
    interval_count: usize,
    scenario_cadence_seconds: i64,
}

impl FixtureGrid {
    /// Validates and constructs the grid. The horizon must be a positive,
    /// exact integer multiple of the interval (P3.03), the interval must echo
    /// the generating scenario's cadence, and the start ISO string must be
    /// present (the canonical timestamp is the millisecond value; the ISO
    /// string is artefact metadata validated by the composition root's
    /// parser, P0.09).
    pub fn new(
        horizon_start_millis: i64,
        horizon_start_iso: String,
        horizon_millis: i64,
        interval_millis: i64,
        scenario_cadence_seconds: i64,
    ) -> Result<Self, ForecastFixtureError> {
        if horizon_start_iso.trim().is_empty() {
            return Err(ForecastFixtureError::EmptyHorizonStartIso);
        }
        if scenario_cadence_seconds <= 0 {
            return Err(ForecastFixtureError::ScenarioCadenceNonPositive);
        }
        if interval_millis != scenario_cadence_seconds * 1000 {
            return Err(ForecastFixtureError::IntervalNotScenarioCadence {
                interval_millis,
                scenario_cadence_seconds,
            });
        }
        // The P3.03 validation: positive horizon/interval, horizon an exact
        // integer multiple of the interval.
        SchedulingGridConfig::new(horizon_millis, interval_millis)
            .map_err(ForecastFixtureError::SchedulingTime)?;
        Ok(Self {
            horizon_start_millis,
            horizon_start_iso,
            horizon_millis,
            interval_millis,
            interval_count: (horizon_millis / interval_millis) as usize,
            scenario_cadence_seconds,
        })
    }

    /// The start-inclusive horizon start (interval 0 start), UTC millis.
    pub const fn horizon_start_millis(&self) -> i64 {
        self.horizon_start_millis
    }

    /// The horizon start as an ISO string (artefact metadata).
    pub fn horizon_start_iso(&self) -> &str {
        &self.horizon_start_iso
    }

    /// The horizon length in milliseconds.
    pub const fn horizon_millis(&self) -> i64 {
        self.horizon_millis
    }

    /// The interval length in milliseconds (the scenario cadence).
    pub const fn interval_millis(&self) -> i64 {
        self.interval_millis
    }

    /// The number of intervals in the horizon.
    pub const fn interval_count(&self) -> usize {
        self.interval_count
    }

    /// The generating scenario's cadence in seconds (echoed from the
    /// scenario file).
    pub const fn scenario_cadence_seconds(&self) -> i64 {
        self.scenario_cadence_seconds
    }

    /// The start-inclusive start of interval `index`, UTC millis. Intervals
    /// are start-inclusive / end-exclusive (P3.03).
    pub fn interval_start_millis(&self, index: usize) -> Option<i64> {
        if index >= self.interval_count {
            return None;
        }
        Some(self.horizon_start_millis + (index as i64) * self.interval_millis)
    }

    /// Whether this grid is identical to the given P3.03 configuration
    /// (same horizon and interval length — hence identical boundaries).
    pub const fn matches_config(&self, config: &SchedulingGridConfig) -> bool {
        self.horizon_millis == config.horizon_millis()
            && self.interval_millis == config.interval_millis()
    }
}

// --- The recorded method block ------------------------------------------------

/// The recorded generation method, carried verbatim into the artefact. Only
/// the recorded constants ([`FixtureMethod::recorded`]) pass validation — a
/// fixture rendered with this module's writer always describes exactly the
/// method that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureMethod {
    jitter_fraction_milli: i64,
}

impl FixtureMethod {
    /// The recorded method of the committed v1 fixtures: ±2 percent seeded
    /// forecast noise on the scenario shapes (2 percent = 20 milli as an
    /// integer, so no float formatting enters the artefact).
    pub const fn recorded() -> Self {
        Self {
            jitter_fraction_milli: 20,
        }
    }

    /// The forecast noise half-width as integer milli (20 = 0.02).
    pub const fn jitter_fraction_milli(&self) -> i64 {
        self.jitter_fraction_milli
    }
}

// --- The typed, labeled fixture ------------------------------------------------

/// The fixture metadata: version, variant, provenance, generating-scenario
/// reference, master seed, and the recorded method block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureMeta {
    forecast_version: ForecastVersion,
    variant: String,
    provenance: Provenance,
    scenario_id: String,
    scenario_version: String,
    master_seed: u64,
    method: FixtureMethod,
}

impl FixtureMeta {
    /// The forecast version (referenced by the envelope P3.12 and the result
    /// records P3.21 — never bare values).
    pub fn forecast_version(&self) -> &ForecastVersion {
        &self.forecast_version
    }

    /// The variant name (`central` or `error-pv-ramp`).
    pub fn variant(&self) -> &str {
        &self.variant
    }

    /// The mandatory provenance label (fixture level — place 1 of the
    /// four-place labelling rule).
    pub const fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The generating scenario's id.
    pub fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    /// The generating scenario's version.
    pub fn scenario_version(&self) -> &str {
        &self.scenario_version
    }

    /// The master seed the fixture was generated from.
    pub const fn master_seed(&self) -> u64 {
        self.master_seed
    }

    /// The recorded method block.
    pub const fn method(&self) -> &FixtureMethod {
        &self.method
    }
}

/// One forecast-error variant (ledger P3.31 step 3): a stated magnitude, a
/// stated direction, the quantity it applies to, and the window it applies
/// to — all recorded, versioned separately from the central forecast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForecastErrorVariant {
    variant_id: String,
    applies_to: ForecastQuantity,
    direction_downward: bool,
    magnitude_milli: i64,
    window_start_iso: String,
    window_end_iso: String,
    window_first_interval: usize,
    window_last_exclusive: usize,
    note: String,
}

impl ForecastErrorVariant {
    /// The stated error magnitude as a fraction (0.8 = 80 percent).
    pub const fn magnitude(&self) -> f64 {
        // milli / 1000 is exact in binary floating point (1000 is not a
        // power of two, so the division rounds — but the magnitude is
        // authoritative in milli units; the f64 form is the human-readable
        // statement of the same recorded integer).
        self.magnitude_milli as f64 / 1000.0
    }

    /// The stated error magnitude as integer milli (the authoritative form).
    pub const fn magnitude_milli(&self) -> i64 {
        self.magnitude_milli
    }

    /// The variant id (`pv-ramp`).
    pub fn variant_id(&self) -> &str {
        &self.variant_id
    }

    /// The quantity the error applies to.
    pub const fn applies_to(&self) -> ForecastQuantity {
        self.applies_to
    }

    /// Whether the variant asserts a downward ramp (the only direction this
    /// crate generates; recorded boundary).
    pub const fn direction_downward(&self) -> bool {
        self.direction_downward
    }

    /// The window start (ISO, artefact metadata).
    pub fn window_start_iso(&self) -> &str {
        &self.window_start_iso
    }

    /// The window end, exclusive (ISO, artefact metadata).
    pub fn window_end_iso(&self) -> &str {
        &self.window_end_iso
    }

    /// The first interval index inside the window.
    pub const fn window_first_interval(&self) -> usize {
        self.window_first_interval
    }

    /// One past the last interval index inside the window (end-exclusive).
    pub const fn window_last_exclusive(&self) -> usize {
        self.window_last_exclusive
    }

    /// The recorded note.
    pub fn note(&self) -> &str {
        &self.note
    }
}

/// One labeled forecast series: per-interval values for one asset and one
/// quantity. The values are **only accessible through [`values`]**, which
/// returns the provenance label, the forecast version, and the quantity
/// alongside the values — the label cannot be dropped on any access path
/// (four-place labelling rule, places 1–2).
///
/// [`values`]: LabeledForecastSeries::values
#[derive(Debug, Clone, PartialEq)]
pub struct LabeledForecastSeries {
    asset_key: String,
    site_key: String,
    quantity: ForecastQuantity,
    label: Provenance,
    version: ForecastVersion,
    /// The authoritative quantized values (milli units).
    values_milli: Vec<i64>,
    /// The same values as MW / fraction at the API boundary
    /// (`values_milli[i] as f64 / 1000.0`).
    values_mw: Vec<f64>,
}

/// What [`LabeledForecastSeries::values`] returns: the values never travel
/// without their label, version, and quantity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabeledSeriesValues<'a> {
    /// The mandatory provenance label (per-value place of the four-place
    /// labelling rule).
    pub provenance: Provenance,
    /// The forecast version this series belongs to.
    pub forecast_version: &'a ForecastVersion,
    /// The quantity and its canonical unit.
    pub quantity: ForecastQuantity,
    /// The per-interval values (MW for power quantities, fraction for
    /// availability), quantized to milli units.
    pub values: &'a [f64],
}

impl LabeledForecastSeries {
    /// The asset key (scenario key; the registry ID mapping is a
    /// registration concern, P2.13 §1).
    pub fn asset_key(&self) -> &str {
        &self.asset_key
    }

    /// The site key.
    pub fn site_key(&self) -> &str {
        &self.site_key
    }

    /// The ONLY access path to the values: always returns the provenance
    /// label, the forecast version, and the quantity with the values.
    pub fn values(&self) -> LabeledSeriesValues<'_> {
        LabeledSeriesValues {
            provenance: self.label,
            forecast_version: &self.version,
            quantity: self.quantity,
            values: &self.values_mw,
        }
    }

    /// The number of intervals in the series.
    pub fn len(&self) -> usize {
        self.values_mw.len()
    }

    /// Whether the series is empty (a valid series never is; validation
    /// rejects an interval-count mismatch).
    pub fn is_empty(&self) -> bool {
        self.values_mw.is_empty()
    }
}

/// The typed fixture: the labeled schema of the synthetic forecast, the
/// single forecast source for v0.2.0.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastFixture {
    meta: FixtureMeta,
    grid: FixtureGrid,
    error_variant: Option<ForecastErrorVariant>,
    series: Vec<LabeledForecastSeries>,
}

impl ForecastFixture {
    /// The fixture metadata.
    pub const fn meta(&self) -> &FixtureMeta {
        &self.meta
    }

    /// The interval grid (P3.03 semantics, scenario cadence).
    pub const fn grid(&self) -> &FixtureGrid {
        &self.grid
    }

    /// The error variant, if this fixture version is one.
    pub const fn error_variant(&self) -> Option<&ForecastErrorVariant> {
        self.error_variant.as_ref()
    }

    /// The labeled series, in fixture order.
    pub fn series(&self) -> &[LabeledForecastSeries] {
        &self.series
    }

    /// The forecast version (convenience for envelope/result references).
    pub fn forecast_version(&self) -> &ForecastVersion {
        &self.meta.forecast_version
    }

    /// Renders the fixture as the byte-deterministic JSON artefact. This is
    /// the regeneration path for the committed versioned artefacts; the
    /// output is byte-identical for the same fixture (the integration test
    /// asserts byte equality against the committed files).
    pub fn to_json(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("{".to_string());
        // meta
        lines.push("  \"meta\": {".to_string());
        lines.push(format!("    \"fixture_id\": {},", jstr(FIXTURE_ID)));
        lines.push(format!(
            "    \"forecast_version\": {},",
            jstr(self.meta.forecast_version.as_str())
        ));
        lines.push(format!("    \"variant\": {},", jstr(&self.meta.variant)));
        lines.push(format!(
            "    \"provenance\": {},",
            jstr(self.meta.provenance.as_str())
        ));
        lines.push(format!("    \"provenance_note\": {},", jstr(PROVENANCE_NOTE)));
        lines.push("    \"generating_scenario\": {".to_string());
        lines.push(format!(
            "      \"scenario_id\": {},",
            jstr(&self.meta.scenario_id)
        ));
        lines.push(format!(
            "      \"scenario_version\": {}",
            jstr(&self.meta.scenario_version)
        ));
        lines.push("    },".to_string());
        lines.push("    \"seed\": {".to_string());
        lines.push(format!("      \"master_seed\": {},", self.meta.master_seed));
        lines.push(format!("      \"derivation\": {},", jstr(SEED_DERIVATION_DOC)));
        lines.push(format!("      \"stream_rule\": {}", jstr(STREAM_RULE_DOC)));
        lines.push("    }".to_string());
        lines.push("  },".to_string());
        // grid
        lines.push("  \"grid\": {".to_string());
        lines.push(format!(
            "    \"horizon_start\": {},",
            jstr(&self.grid.horizon_start_iso)
        ));
        lines.push(format!(
            "    \"horizon_millis\": {},",
            self.grid.horizon_millis
        ));
        lines.push(format!("    \"interval_millis\": {},", self.grid.interval_millis));
        lines.push(format!("    \"interval_count\": {},", self.grid.interval_count));
        lines.push(format!("    \"interval_semantics\": {},", jstr(GRID_SEMANTICS_DOC)));
        lines.push(format!(
            "    \"scenario_cadence_seconds\": {}",
            self.grid.scenario_cadence_seconds
        ));
        lines.push("  },".to_string());
        // method
        lines.push("  \"method\": {".to_string());
        lines.push(format!("    \"pv_shape\": {},", jstr(PV_SHAPE_DOC)));
        lines.push(format!("    \"load_shape\": {},", jstr(LOAD_SHAPE_DOC)));
        lines.push(format!("    \"availability\": {},", jstr(AVAILABILITY_DOC)));
        lines.push(format!("    \"forecast_noise\": {},", jstr(FORECAST_NOISE_DOC)));
        lines.push(format!(
            "    \"jitter_fraction_milli\": {},",
            self.meta.method.jitter_fraction_milli()
        ));
        lines.push(format!("    \"quantization\": {},", jstr(QUANTIZATION_DOC)));
        lines.push(format!("    \"shape_note\": {}", jstr(SHAPE_NOTE)));
        lines.push("  },".to_string());
        // error variant
        match &self.error_variant {
            None => lines.push("  \"error_variant\": null,".to_string()),
            Some(ev) => {
                lines.push("  \"error_variant\": {".to_string());
                lines.push(format!("    \"variant_id\": {},", jstr(&ev.variant_id)));
                lines.push(format!(
                    "    \"applies_to_quantity\": {},",
                    jstr(ev.applies_to.wire_name())
                ));
                let direction = if ev.direction_downward { "downward" } else { "upward" };
                lines.push(format!("    \"direction\": {},", jstr(direction)));
                lines.push(format!(
                    "    \"error_magnitude_milli\": {},",
                    ev.magnitude_milli
                ));
                lines.push(format!(
                    "    \"window_start\": {},",
                    jstr(&ev.window_start_iso)
                ));
                lines.push(format!("    \"window_end\": {},", jstr(&ev.window_end_iso)));
                lines.push(format!(
                    "    \"window_first_interval\": {},",
                    ev.window_first_interval
                ));
                lines.push(format!(
                    "    \"window_last_exclusive\": {}",
                    ev.window_last_exclusive
                ));
                lines.push(format!("    \"note\": {}", jstr(&ev.note)));
                lines.push("  },".to_string());
            }
        }
        // series
        lines.push("  \"series\": [".to_string());
        for (idx, series) in self.series.iter().enumerate() {
            lines.push("    {".to_string());
            lines.push(format!("      \"asset_key\": {},", jstr(&series.asset_key)));
            lines.push(format!("      \"site_key\": {},", jstr(&series.site_key)));
            lines.push(format!(
                "      \"quantity\": {},",
                jstr(series.quantity.wire_name())
            ));
            lines.push(format!("      \"unit\": {},", jstr(series.quantity.unit())));
            lines.push(format!(
                "      \"provenance\": {},",
                jstr(series.label.as_str())
            ));
            lines.push(format!(
                "      \"forecast_version\": {},",
                jstr(series.version.as_str())
            ));
            let values: Vec<String> = series.values_milli.iter().map(|m| fmt_milli(*m)).collect();
            lines.push(format!("      \"values\": [{}]", values.join(", ")));
            lines.push(format!(
                "    {}",
                if idx + 1 < self.series.len() { "}," } else { "}" }
            ));
        }
        lines.push("  ]".to_string());
        lines.push("}".to_string());
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

// --- JSON writer helpers -------------------------------------------------------

/// Renders a JSON string literal with the minimal escaping (`"`, `\`,
/// control characters). All recorded strings are plain ASCII without
/// escapes, so the output is stable across toolchains.
fn jstr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Renders one quantized value as a JSON number with exactly three decimals
/// (`-417` → `-0.417`, `1000` → `1.000`). Integer-only formatting: no float
/// formatting ever enters the artefact.
fn fmt_milli(m: i64) -> String {
    let sign = if m < 0 { "-" } else { "" };
    let a = m.unsigned_abs();
    format!("{}{}.{:03}", sign, a / 1000, a % 1000)
}

// --- The P2.14 RNG primitives (duplicated from flexcore-simulator) -------------
//
// flexcore-application may not depend on flexcore-simulator (manifest
// policy), so the recorded primitives are reproduced here verbatim and
// pinned by the recorded test vectors (see the tests below). The derivation
// rule is the scenario's recorded rule applied to a namespaced stream key:
// `seed(stream_key) = master_seed XOR splitmix64(FNV-1a-64(stream_key))`.

/// FNV-1a-64 over `bytes` (the standard offset basis and prime).
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The splitmix64 finalizer applied to an arbitrary value.
fn splitmix64_mix(x: u64) -> u64 {
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derives a stream seed from the master seed and the stream key (the
/// recorded rule, P2.14 / D-SC4).
fn derive_asset_seed(master_seed: u64, stream_key: &str) -> u64 {
    master_seed ^ splitmix64_mix(fnv1a64(stream_key.as_bytes()))
}

/// A splitmix64 stream generator (Steele et al., 2014): golden-ratio
/// increment on the state, then the finalizer, per step.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64_mix(self.state)
    }

    /// A finite f64 uniformly spread over `[low, high)`, with the same
    /// 53-bit-draw convention as the simulator's RNG.
    fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        debug_assert!(low <= high && low.is_finite() && high.is_finite());
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
        low + unit * (high - low)
    }
}

// --- The recorded scenario inputs (transcribed from scenario-v2.json) ----------
//
// The application crate cannot parse JSON (no serialization dependency), so
// the versioned scenario's forecast-relevant constants are transcribed here
// and PINNED by the integration test, which loads `scenario-v2.json` itself
// and asserts the recorded values appear verbatim in the file — a silent
// drift between artefact and generator is a test failure.

/// The recorded exit-gate scenario inputs (scenario v2, P2.13/P2.25).
mod exit_gate_scenario_v2 {
    pub const SCENARIO_ID: &str = "synthetic-portfolio-exit-gate";
    pub const SCENARIO_VERSION: &str = "v2";
    pub const MASTER_SEED: u64 = 20260601;
    /// 2026-06-01T00:00:00Z.
    pub const HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
    pub const HORIZON_START_ISO: &str = "2026-06-01T00:00:00Z";
    pub const HORIZON_MILLIS: i64 = 86_400_000;
    pub const INTERVAL_MILLIS: i64 = 60_000;
    pub const INTERVAL_COUNT: usize = 1440;
    pub const CADENCE_SECONDS: i64 = 60;
    /// The pv_clear_sky profile window, seconds of day UTC: 05:30 – 21:00.
    pub const SUNRISE_SOD: u32 = 19_800;
    pub const SUNSET_SOD: u32 = 75_600;
}

/// One forecasted asset from the scenario (transcribed; pinned by test).
struct SeriesSpec {
    site_key: &'static str,
    asset_key: &'static str,
    quantity: ForecastQuantity,
    /// Rated power in MW (for the load: the rated peak).
    rated_mw: f64,
    /// The pv_clear_sky peak factor (unused for the load).
    peak_factor: f64,
}

/// The four forecasted assets, in fixture order (scenario order: site-north
/// then site-south; PV then load within a site). BESS/EVSE/meter assets get
/// NO forecast series (recorded scope, see the module header).
const SERIES_SPECS: [SeriesSpec; 4] = [
    SeriesSpec {
        site_key: "site-north",
        asset_key: "site-north-pv-1",
        quantity: ForecastQuantity::PvOutputMw,
        rated_mw: 1.5,
        peak_factor: 0.85,
    },
    SeriesSpec {
        site_key: "site-north",
        asset_key: "site-north-load-1",
        quantity: ForecastQuantity::LoadMw,
        rated_mw: 0.8,
        peak_factor: 0.0,
    },
    SeriesSpec {
        site_key: "site-south",
        asset_key: "site-south-pv-1",
        quantity: ForecastQuantity::PvOutputMw,
        rated_mw: 2.5,
        peak_factor: 0.9,
    },
    SeriesSpec {
        site_key: "site-south",
        asset_key: "site-south-load-1",
        quantity: ForecastQuantity::LoadMw,
        rated_mw: 1.2,
        peak_factor: 0.0,
    },
];

/// The forecast noise half-width (±2 percent, the same order as the load
/// profile's jitter fraction).
const JITTER_FRACTION: f64 = 0.02;

// --- The shape functions (duplicated from flexcore-simulator profiles.rs) ------
//
// The central forecast reproduces the scenario's declared synthetic profiles
// so that it is the near-perfect forecast of the simulator's truth (plus the
// seeded ±2 percent forecast noise). These are scenario profiles, NOT a
// weather model and NOT validated physics (ASSUMPTION, recorded).

/// The pv_clear_sky cosine bell at seconds-of-day `sod` (simulator
/// `pv_tick`): zero outside `[sunrise, sunset)`, otherwise
/// `rated * peak_factor * 0.5 * (1 - cos(2*pi*x))` with `x` over `[0, 1)`,
/// capped at the rated power.
fn pv_shape(sod: u32, rated_mw: f64, peak_factor: f64) -> f64 {
    if sod >= exit_gate_scenario_v2::SUNRISE_SOD && sod < exit_gate_scenario_v2::SUNSET_SOD {
        let span = f64::from(exit_gate_scenario_v2::SUNSET_SOD - exit_gate_scenario_v2::SUNRISE_SOD);
        let x = (f64::from(sod) - f64::from(exit_gate_scenario_v2::SUNRISE_SOD)) / span; // [0, 1)
        let bell = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * x).cos());
        (rated_mw * peak_factor * bell).min(rated_mw)
    } else {
        0.0
    }
}

/// A symmetric cosine bump of height 1 centred on `center` with half-width
/// `half_width_s` seconds; 0 outside (simulator `cosine_bump`).
fn cosine_bump(sod: u32, center: f64, half_width_s: f64) -> f64 {
    let delta = (f64::from(sod) - center).abs();
    if delta < half_width_s {
        0.5 * (1.0 + (std::f64::consts::PI * delta / half_width_s).cos())
    } else {
        0.0
    }
}

/// The load_baseline_reducible baseline shape at seconds-of-day `sod`
/// (simulator `load_tick` baseline, WITHOUT the simulator's measurement
/// jitter — the forecast carries its own seeded forecast noise instead):
/// night 0.25 + morning 0.45 + evening 0.50, all times the rated peak,
/// capped at the rated peak. The return value is the consumption magnitude;
/// the caller applies the canonical net sign (negative).
fn load_shape(sod: u32, rated_peak_mw: f64) -> f64 {
    let night = 0.25 * rated_peak_mw;
    let morning = 0.45 * rated_peak_mw * cosine_bump(sod, 7.5 * 3600.0, 2.5 * 3600.0);
    let evening = 0.50 * rated_peak_mw * cosine_bump(sod, 19.0 * 3600.0, 3.0 * 3600.0);
    (night + morning + evening).min(rated_peak_mw)
}

// --- Quantization ----------------------------------------------------------------

/// Quantizes a raw MW/fraction value to whole milli units, round half away
/// from zero. The quantized milli value is authoritative for the artefact;
/// the API-boundary f64 is `milli as f64 / 1000.0`.
fn quantize(raw: f64) -> i64 {
    let scaled = raw * 1000.0;
    if scaled >= 0.0 {
        (scaled + 0.5).floor() as i64
    } else {
        (scaled - 0.5).ceil() as i64
    }
}

// --- The generator -----------------------------------------------------------------

/// The generator-internal error-variant parameters.
struct ErrorSpec {
    applies_to: ForecastQuantity,
    magnitude: f64,
    window_first_interval: usize,
    window_last_exclusive: usize,
}

/// Generates one power series' quantized values. Draws exactly one splitmix64
/// value per interval from the stream keyed
/// `<asset_key>/forecast/<forecast_version>`, in interval order (the recorded
/// stream rule).
fn generate_power_series(spec: &SeriesSpec, forecast_version: &str, error: Option<&ErrorSpec>) -> Vec<i64> {
    let stream_key = format!("{}/forecast/{}", spec.asset_key, forecast_version);
    let mut rng = SplitMix64::new(derive_asset_seed(exit_gate_scenario_v2::MASTER_SEED, &stream_key));
    let mut values = Vec::with_capacity(exit_gate_scenario_v2::INTERVAL_COUNT);
    for i in 0..exit_gate_scenario_v2::INTERVAL_COUNT {
        let sod = (i as u32) * exit_gate_scenario_v2::CADENCE_SECONDS as u32;
        // The scenario shapes are the PV and load profiles; the specs table
        // carries no availability series (enforced by a test), so the
        // availability arm below is never taken in generation.
        let shape = match spec.quantity {
            ForecastQuantity::PvOutputMw => pv_shape(sod, spec.rated_mw, spec.peak_factor),
            ForecastQuantity::LoadMw => -load_shape(sod, spec.rated_mw),
            ForecastQuantity::AvailabilityFraction => 0.0, // never generated here (recorded scope)
        };
        let u = rng.f64_in(-JITTER_FRACTION, JITTER_FRACTION);
        let in_window = error.is_some_and(|e| {
            e.applies_to == spec.quantity && (e.window_first_interval..e.window_last_exclusive).contains(&i)
        });
        let raw = match error {
            Some(e) if in_window => shape * (1.0 - e.magnitude) * (1.0 + u),
            _ => shape * (1.0 + u),
        };
        values.push(quantize(raw));
    }
    values
}

fn labeled_series_from_milli(
    spec: &SeriesSpec,
    forecast_version: &str,
    values_milli: Vec<i64>,
) -> LabeledForecastSeries {
    let values_mw: Vec<f64> = values_milli.iter().map(|m| *m as f64 / 1000.0).collect();
    LabeledForecastSeries {
        asset_key: spec.asset_key.to_string(),
        site_key: spec.site_key.to_string(),
        quantity: spec.quantity,
        label: Provenance::Synthetic,
        version: ForecastVersion::new(forecast_version)
            .expect("generator version strings are validated constants"),
        values_milli,
        values_mw,
    }
}

fn availability_series(spec: &SeriesSpec, forecast_version: &str) -> LabeledForecastSeries {
    // The scenario declares availability constant true: the availability
    // forecast is the constant fraction 1.0 (no noise, no stream draws).
    let values_milli = vec![1000i64; exit_gate_scenario_v2::INTERVAL_COUNT];
    let values_mw = vec![1.0f64; exit_gate_scenario_v2::INTERVAL_COUNT];
    LabeledForecastSeries {
        asset_key: spec.asset_key.to_string(),
        site_key: spec.site_key.to_string(),
        quantity: ForecastQuantity::AvailabilityFraction,
        label: Provenance::Synthetic,
        version: ForecastVersion::new(forecast_version)
            .expect("generator version strings are validated constants"),
        values_milli,
        values_mw,
    }
}

fn build_fixture(
    forecast_version: &str,
    variant: &str,
    error: Option<(ErrorSpec, UnvalidatedErrorVariant)>,
) -> ForecastFixture {
    let mut series = Vec::with_capacity(SERIES_SPECS.len() * 2);
    for spec in SERIES_SPECS.iter() {
        let values_milli = generate_power_series(spec, forecast_version, error.as_ref().map(|(e, _)| e));
        series.push(labeled_series_from_milli(spec, forecast_version, values_milli));
    }
    for spec in SERIES_SPECS.iter() {
        series.push(availability_series(spec, forecast_version));
    }
    ForecastFixture {
        meta: FixtureMeta {
            forecast_version: ForecastVersion::new(forecast_version)
                .expect("generator version strings are validated constants"),
            variant: variant.to_string(),
            provenance: Provenance::Synthetic,
            scenario_id: exit_gate_scenario_v2::SCENARIO_ID.to_string(),
            scenario_version: exit_gate_scenario_v2::SCENARIO_VERSION.to_string(),
            master_seed: exit_gate_scenario_v2::MASTER_SEED,
            method: FixtureMethod::recorded(),
        },
        grid: FixtureGrid::new(
            exit_gate_scenario_v2::HORIZON_START_MILLIS,
            exit_gate_scenario_v2::HORIZON_START_ISO.to_string(),
            exit_gate_scenario_v2::HORIZON_MILLIS,
            exit_gate_scenario_v2::INTERVAL_MILLIS,
            exit_gate_scenario_v2::CADENCE_SECONDS,
        )
        .expect("the recorded scenario grid satisfies the P3.03 grid rules"),
        error_variant: error.map(|(_, record)| ForecastErrorVariant {
            variant_id: record.variant_id,
            applies_to: record.applies_to,
            direction_downward: record.direction_downward,
            magnitude_milli: record.magnitude_milli,
            window_start_iso: record.window_start_iso,
            window_end_iso: record.window_end_iso,
            window_first_interval: record.window_first_interval,
            window_last_exclusive: record.window_last_exclusive,
            note: record.note,
        }),
        series,
    }
}

/// Generates the central synthetic forecast (version `v1`): the scenario's
/// declared shapes plus seeded ±2 percent forecast noise. Deterministic from
/// the recorded master seed — same seed, byte-identical artefact.
pub fn generate_central_fixture() -> ForecastFixture {
    build_fixture("v1", CENTRAL_VARIANT, None)
}

/// Generates the P3.26 PV-ramp / forecast-error variant (version
/// `v1-error-pv-ramp`): inside the 12:00–13:00 UTC window the PV forecast
/// asserts a downward ramp of the recorded magnitude (80 percent of the
/// clear-sky value). Versioned SEPARATELY from the central forecast; its
/// noise streams are keyed by its own version, so both fixtures are
/// independently reproducible.
pub fn generate_error_pv_ramp_fixture() -> ForecastFixture {
    build_fixture(
        "v1-error-pv-ramp",
        ERROR_PV_RAMP_VARIANT,
        Some((
            ErrorSpec {
                applies_to: ForecastQuantity::PvOutputMw,
                magnitude: 0.8,
                window_first_interval: 720,
                window_last_exclusive: 780,
            },
            UnvalidatedErrorVariant {
                variant_id: "pv-ramp".to_string(),
                applies_to: ForecastQuantity::PvOutputMw,
                direction_downward: true,
                magnitude_milli: 800,
                window_start_iso: "2026-06-01T12:00:00Z".to_string(),
                window_end_iso: "2026-06-01T13:00:00Z".to_string(),
                window_first_interval: 720,
                window_last_exclusive: 780,
                note: ERROR_VARIANT_NOTE.to_string(),
            },
        )),
    )
}

// --- The envelope input validation (P3.12 calls this) ----------------------------

/// One raw series from outside the trust boundary (the fixture store's
/// parsed artefact, P3.12's input). UNVALIDATED by type: no provenance, no
/// version guarantee, values unchecked.
#[derive(Debug, Clone, PartialEq)]
pub struct UnvalidatedSeries {
    /// The asset key (scenario key).
    pub asset_key: String,
    /// The site key.
    pub site_key: String,
    /// The quantity.
    pub quantity: ForecastQuantity,
    /// The raw per-interval values (MW for power quantities, fraction for
    /// availability). Validated for finiteness, sign, and range; quantized
    /// to milli units on acceptance.
    pub values: Vec<f64>,
}

/// One raw error-variant record from outside the trust boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct UnvalidatedErrorVariant {
    pub variant_id: String,
    pub applies_to: ForecastQuantity,
    pub direction_downward: bool,
    pub magnitude_milli: i64,
    pub window_start_iso: String,
    pub window_end_iso: String,
    pub window_first_interval: usize,
    pub window_last_exclusive: usize,
    pub note: String,
}

/// A raw forecast from outside the trust boundary: exactly what P3.12 must
/// never trust. The provenance label and the forecast version are OPTIONAL
/// here — and [`validate_forecast_input`] rejects their absence with a named
/// error. There is no unvalidated path into [`ForecastFixture`].
#[derive(Debug, Clone, PartialEq)]
pub struct UnvalidatedForecastInput {
    /// The provenance label. `None` (or any unlabelled form) is REJECTED.
    pub provenance: Option<Provenance>,
    /// The forecast version. `None` (or an empty/whitespace string) is
    /// REJECTED.
    pub forecast_version: Option<String>,
    /// The variant name (`central`, `error-pv-ramp`, ...).
    pub variant: String,
    /// The generating scenario's id.
    pub scenario_id: String,
    /// The generating scenario's version.
    pub scenario_version: String,
    /// The master seed the fixture was generated from.
    pub master_seed: u64,
    /// The recorded method block. Must be the recorded constants; anything
    /// else is rejected (a fixture's writer output must describe the method
    /// that actually produced it).
    pub method: FixtureMethod,
    /// Grid: start (UTC millis), start ISO metadata, horizon, interval,
    /// scenario cadence. Validated against the P3.03 grid rules.
    pub horizon_start_millis: i64,
    pub horizon_start_iso: String,
    pub horizon_millis: i64,
    pub interval_millis: i64,
    pub scenario_cadence_seconds: i64,
    /// The error variant, if this fixture version is one.
    pub error_variant: Option<UnvalidatedErrorVariant>,
    /// The raw series (per-asset per-interval values).
    pub series: Vec<UnvalidatedSeries>,
}

/// Why a forecast input was rejected. Every variant is a named, matchable
/// error — an unlabelled or unversioned forecast never enters the pipeline
/// with a generic failure (ledger P3.31 acceptance: no Phase 3 code path
/// consumes an unlabelled or unversioned forecast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastFixtureError {
    /// The forecast version string was empty.
    EmptyForecastVersion,
    /// The forecast version contained whitespace.
    ForecastVersionHasWhitespace,
    /// The forecast carried NO provenance label. This is the named rejection
    /// of an unlabelled forecast (place 3 of the four-place labelling rule).
    MissingProvenanceLabel,
    /// The forecast carried NO version. This is the named rejection of an
    /// unversioned forecast.
    MissingForecastVersion,
    /// The horizon-start ISO metadata was empty.
    EmptyHorizonStartIso,
    /// The scenario cadence was not positive.
    ScenarioCadenceNonPositive,
    /// The fixture interval does not echo the generating scenario's cadence.
    IntervalNotScenarioCadence {
        /// The supplied interval length (millis).
        interval_millis: i64,
        /// The declared scenario cadence (seconds).
        scenario_cadence_seconds: i64,
    },
    /// The grid violated a P3.03 rule (non-positive horizon/interval, or the
    /// horizon is not an integer multiple of the interval).
    SchedulingTime(SchedulingTimeError),
    /// The recorded method block did not match the recorded constants.
    MethodMismatch,
    /// The variant id was empty.
    EmptyVariantId,
    /// The error magnitude is not in (0, 1000) milli: a downward variant
    /// with magnitude 0 changes nothing and one at or above 1000 would
    /// invert the sign of the forecast.
    InvalidErrorVariantMagnitude {
        /// The supplied magnitude (milli).
        magnitude_milli: i64,
    },
    /// An upward error variant was supplied. Only downward variants are
    /// generated in v0.2.0 (recorded boundary).
    UnsupportedErrorVariantDirection,
    /// The error window is inverted or reaches outside the horizon.
    InvalidErrorVariantWindow {
        /// The first window interval.
        first: usize,
        /// The exclusive last window interval.
        last_exclusive: usize,
        /// The horizon's interval count.
        interval_count: usize,
    },
    /// The input carried no series at all.
    NoSeries,
    /// A series carried an empty asset key.
    EmptyAssetKey,
    /// A series' interval count does not match the grid.
    SeriesIntervalCountMismatch {
        /// The offending asset key.
        asset_key: String,
        /// The grid's interval count.
        expected: usize,
        /// The series' value count.
        found: usize,
    },
    /// A series value was NaN or infinite.
    NonFiniteForecastValue {
        /// The offending asset key.
        asset_key: String,
        /// The offending interval index.
        index: usize,
    },
    /// A power value violated the canonical sign rule (§10.2): PV output
    /// must be >= 0 (export), load must be <= 0 (consumption negative).
    PowerSignViolation {
        /// The offending asset key.
        asset_key: String,
        /// The offending quantity.
        quantity: ForecastQuantity,
        /// The offending interval index.
        index: usize,
    },
    /// An availability value was outside [0, 1].
    AvailabilityOutOfRange {
        /// The offending asset key.
        asset_key: String,
        /// The offending interval index.
        index: usize,
    },
    /// The `ForecastPublished` envelope failed construction.
    Envelope(EnvelopeError),
}

impl std::fmt::Display for ForecastFixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastFixtureError::EmptyForecastVersion => {
                write!(f, "the forecast version must be non-empty")
            }
            ForecastFixtureError::ForecastVersionHasWhitespace => {
                write!(f, "the forecast version must not contain whitespace")
            }
            ForecastFixtureError::MissingProvenanceLabel => {
                write!(
                    f,
                    "the forecast carries no provenance label: an unlabelled forecast is rejected \
                     (every forecast value must be labelled synthetic)"
                )
            }
            ForecastFixtureError::MissingForecastVersion => {
                write!(
                    f,
                    "the forecast carries no version: an unversioned forecast is rejected \
                     (envelope and result records reference the forecast version, never bare values)"
                )
            }
            ForecastFixtureError::EmptyHorizonStartIso => {
                write!(f, "the horizon-start ISO metadata must be non-empty")
            }
            ForecastFixtureError::ScenarioCadenceNonPositive => {
                write!(f, "the scenario cadence must be positive")
            }
            ForecastFixtureError::IntervalNotScenarioCadence {
                interval_millis,
                scenario_cadence_seconds,
            } => write!(
                f,
                "the fixture interval ({interval_millis} ms) must echo the generating scenario's \
                 cadence ({scenario_cadence_seconds} s)"
            ),
            ForecastFixtureError::SchedulingTime(e) => write!(f, "grid rule violated: {e}"),
            ForecastFixtureError::MethodMismatch => {
                write!(f, "the method block does not match the recorded method")
            }
            ForecastFixtureError::EmptyVariantId => {
                write!(f, "the error variant id must be non-empty")
            }
            ForecastFixtureError::InvalidErrorVariantMagnitude { magnitude_milli } => write!(
                f,
                "the error magnitude ({magnitude_milli} milli) must be in (0, 1000) milli"
            ),
            ForecastFixtureError::UnsupportedErrorVariantDirection => write!(
                f,
                "only downward error variants are generated in v0.2.0 (recorded boundary)"
            ),
            ForecastFixtureError::InvalidErrorVariantWindow {
                first,
                last_exclusive,
                interval_count,
            } => write!(
                f,
                "the error window [{first}, {last_exclusive}) is inverted or reaches outside the \
                 horizon ({interval_count} intervals)"
            ),
            ForecastFixtureError::NoSeries => write!(f, "the forecast carries no series"),
            ForecastFixtureError::EmptyAssetKey => {
                write!(f, "a series asset key must be non-empty")
            }
            ForecastFixtureError::SeriesIntervalCountMismatch {
                asset_key,
                expected,
                found,
            } => write!(
                f,
                "series '{asset_key}' carries {found} values but the grid has {expected} intervals"
            ),
            ForecastFixtureError::NonFiniteForecastValue { asset_key, index } => write!(
                f,
                "series '{asset_key}' interval {index}: values must be finite (no NaN or infinity)"
            ),
            ForecastFixtureError::PowerSignViolation {
                asset_key,
                quantity,
                index,
            } => write!(
                f,
                "series '{asset_key}' interval {index}: {quantity:?} violates the canonical sign \
                 rule (P > 0 export: PV output >= 0, load <= 0)"
            ),
            ForecastFixtureError::AvailabilityOutOfRange { asset_key, index } => write!(
                f,
                "series '{asset_key}' interval {index}: availability must be a fraction in [0, 1]"
            ),
            ForecastFixtureError::Envelope(e) => write!(f, "event envelope invalid: {e}"),
        }
    }
}

impl std::error::Error for ForecastFixtureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ForecastFixtureError::SchedulingTime(e) => Some(e),
            ForecastFixtureError::Envelope(e) => Some(e),
            _ => None,
        }
    }
}

impl From<EnvelopeError> for ForecastFixtureError {
    fn from(e: EnvelopeError) -> Self {
        Self::Envelope(e)
    }
}

fn validate_error_variant(
    raw: UnvalidatedErrorVariant,
    interval_count: usize,
) -> Result<ForecastErrorVariant, ForecastFixtureError> {
    if raw.variant_id.trim().is_empty() {
        return Err(ForecastFixtureError::EmptyVariantId);
    }
    if !raw.direction_downward {
        return Err(ForecastFixtureError::UnsupportedErrorVariantDirection);
    }
    if raw.magnitude_milli <= 0 || raw.magnitude_milli >= 1000 {
        return Err(ForecastFixtureError::InvalidErrorVariantMagnitude {
            magnitude_milli: raw.magnitude_milli,
        });
    }
    if raw.window_first_interval >= raw.window_last_exclusive
        || raw.window_last_exclusive > interval_count
    {
        return Err(ForecastFixtureError::InvalidErrorVariantWindow {
            first: raw.window_first_interval,
            last_exclusive: raw.window_last_exclusive,
            interval_count,
        });
    }
    Ok(ForecastErrorVariant {
        variant_id: raw.variant_id,
        applies_to: raw.applies_to,
        direction_downward: raw.direction_downward,
        magnitude_milli: raw.magnitude_milli,
        window_start_iso: raw.window_start_iso,
        window_end_iso: raw.window_end_iso,
        window_first_interval: raw.window_first_interval,
        window_last_exclusive: raw.window_last_exclusive,
        note: raw.note,
    })
}

/// The envelope input validation helper (ledger P3.31 step 5; P3.12 calls
/// this): REJECTS an unlabelled forecast ([`ForecastFixtureError::MissingProvenanceLabel`]),
/// an unversioned forecast ([`ForecastFixtureError::MissingForecastVersion`]),
/// and every other frozen rule violation — grid semantics, scenario-cadence
/// echo, method block, series coverage, finiteness, sign, and range. On
/// success it returns the typed, labeled [`ForecastFixture`]: there is no
/// path by which an unlabelled or unversioned forecast reaches the envelope.
pub fn validate_forecast_input(
    input: UnvalidatedForecastInput,
) -> Result<ForecastFixture, ForecastFixtureError> {
    // Place 3 of the four-place labelling rule: no label, no entry.
    let provenance = input.provenance.ok_or(ForecastFixtureError::MissingProvenanceLabel)?;
    let version_raw = input.forecast_version.ok_or(ForecastFixtureError::MissingForecastVersion)?;
    let forecast_version = ForecastVersion::new(&version_raw)?;
    if input.method != FixtureMethod::recorded() {
        return Err(ForecastFixtureError::MethodMismatch);
    }
    let grid = FixtureGrid::new(
        input.horizon_start_millis,
        input.horizon_start_iso,
        input.horizon_millis,
        input.interval_millis,
        input.scenario_cadence_seconds,
    )?;
    let error_variant = match input.error_variant {
        None => None,
        Some(raw) => Some(validate_error_variant(raw, grid.interval_count())?),
    };
    if input.series.is_empty() {
        return Err(ForecastFixtureError::NoSeries);
    }
    let mut series = Vec::with_capacity(input.series.len());
    for raw_series in input.series {
        if raw_series.asset_key.trim().is_empty() {
            return Err(ForecastFixtureError::EmptyAssetKey);
        }
        if raw_series.values.len() != grid.interval_count() {
            return Err(ForecastFixtureError::SeriesIntervalCountMismatch {
                asset_key: raw_series.asset_key,
                expected: grid.interval_count(),
                found: raw_series.values.len(),
            });
        }
        let mut values_milli = Vec::with_capacity(raw_series.values.len());
        for (index, value) in raw_series.values.iter().enumerate() {
            if !value.is_finite() {
                return Err(ForecastFixtureError::NonFiniteForecastValue {
                    asset_key: raw_series.asset_key.clone(),
                    index,
                });
            }
            match raw_series.quantity {
                ForecastQuantity::PvOutputMw => {
                    if *value < 0.0 {
                        return Err(ForecastFixtureError::PowerSignViolation {
                            asset_key: raw_series.asset_key.clone(),
                            quantity: raw_series.quantity,
                            index,
                        });
                    }
                }
                ForecastQuantity::LoadMw => {
                    if *value > 0.0 {
                        return Err(ForecastFixtureError::PowerSignViolation {
                            asset_key: raw_series.asset_key.clone(),
                            quantity: raw_series.quantity,
                            index,
                        });
                    }
                }
                ForecastQuantity::AvailabilityFraction => {
                    if !(0.0..=1.0).contains(value) {
                        return Err(ForecastFixtureError::AvailabilityOutOfRange {
                            asset_key: raw_series.asset_key.clone(),
                            index,
                        });
                    }
                }
            }
            values_milli.push(quantize(*value));
        }
        series.push(LabeledForecastSeries {
            asset_key: raw_series.asset_key,
            site_key: raw_series.site_key,
            quantity: raw_series.quantity,
            label: provenance,
            version: forecast_version.clone(),
            values_mw: values_milli.iter().map(|m| *m as f64 / 1000.0).collect(),
            values_milli,
        });
    }
    Ok(ForecastFixture {
        meta: FixtureMeta {
            forecast_version,
            variant: input.variant,
            provenance,
            scenario_id: input.scenario_id,
            scenario_version: input.scenario_version,
            master_seed: input.master_seed,
            method: input.method,
        },
        grid,
        error_variant,
        series,
    })
}

// --- ForecastPublished assembly (outbox pattern, pure) ----------------------------

/// The inputs to one `ForecastPublished` publication. The provenance label
/// and the version are REQUIRED types here — there is no field a caller
/// could leave empty.
pub struct ForecastPublicationInput<'a> {
    /// The correlation identifier (per-use-case invocation).
    pub correlation_id: FlexUuid,
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    pub site_id: SiteId,
    pub asset_id: AssetId,
    /// The forecast version being published (validated).
    pub forecast_version: &'a ForecastVersion,
    /// The provenance label (always [`Provenance::Synthetic`]).
    pub provenance: Provenance,
    /// The event time (contracts type, P0.09).
    pub event_time: EventTime,
}

/// One assembled `ForecastPublished` publication: the envelope, its
/// destination subject (the frozen P1.14 scheme), and the idempotency key.
/// This is the pure assembly only — the wiring into the transactional outbox
/// (state-change receipt + encoder + `EventOutbox` port) lands with
/// P3.12/P3.28 through `event_assembly`.
pub struct ForecastPublication {
    /// The versioned envelope carrying the frozen `ForecastPublished` event.
    pub envelope: EventEnvelope,
    /// The destination subject per the frozen P1.14 table:
    /// `flexcore.v1.forecast.forecast_published`.
    pub destination_subject: Subject,
    /// The idempotency key, deterministic from the forecast version and the
    /// provenance label: `forecast_published:<version>:<provenance>`.
    /// Republication of the same forecast version dedupes durably.
    pub idempotency_key: IdempotencyKey,
}

/// Assembles the `ForecastPublished` publication for one fixture version
/// (§15.3; outbox pattern only — pure assembly, no I/O).
///
/// GAP (recorded, see the module header): the frozen payload carries
/// identity and time only; the version + provenance travel in the
/// idempotency key and the fixture store until the events crate extends the
/// payload additively (the P2.16 serde-default precedent).
pub fn forecast_publication(
    input: ForecastPublicationInput<'_>,
) -> Result<ForecastPublication, ForecastFixtureError> {
    let payload = ForecastPublishedPayload {
        correlation_id: input.correlation_id,
        tenant_id: input.tenant_id,
        portfolio_id: input.portfolio_id,
        site_id: input.site_id,
        asset_id: input.asset_id,
        event_time: input.event_time,
    };
    let event = DomainEvent::ForecastPublished(payload);
    let key = format!(
        "forecast_published:{}:{}",
        input.forecast_version.as_str(),
        input.provenance.as_str()
    );
    let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &key, event)?;
    let destination_subject = subject_for(&envelope.event);
    Ok(ForecastPublication {
        idempotency_key: envelope.idempotency_key.clone(),
        destination_subject,
        envelope,
    })
}

// --- Unit tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_matches_the_recorded_p2_14_vectors() {
        // Recorded vectors (computed once, pinned so the duplicated P2.14
        // primitives cannot drift from the simulator's):
        assert_eq!(fnv1a64(b"site-north-pv-1/forecast/v1"), 4655564613477277291);
        assert_eq!(
            derive_asset_seed(20260601, "site-north-pv-1/forecast/v1"),
            13097627805479815825
        );
        assert_eq!(
            derive_asset_seed(20260601, "site-north-load-1/forecast/v1"),
            11322007199298442809
        );
        assert_eq!(
            derive_asset_seed(20260601, "site-south-pv-1/forecast/v1"),
            8960531524195384644
        );
        assert_eq!(
            derive_asset_seed(20260601, "site-south-load-1/forecast/v1"),
            5806753243203157154
        );
        assert_eq!(
            derive_asset_seed(20260601, "site-north-pv-1/forecast/v1-error-pv-ramp"),
            1926656629803522461
        );
        let seed = derive_asset_seed(20260601, "site-north-pv-1/forecast/v1");
        let mut rng = SplitMix64::new(seed);
        assert_eq!(rng.next_u64(), 7410279095070433649);
        assert_eq!(rng.next_u64(), 10115515937822676253);
        assert_eq!(rng.next_u64(), 5966303833650957850);
        let mut rng = SplitMix64::new(seed);
        assert_eq!(rng.f64_in(-0.02, 0.02), -0.0039315186128014365);
        assert_eq!(rng.f64_in(-0.02, 0.02), 0.001934528711198178);
        assert_eq!(rng.f64_in(-0.02, 0.02), -0.007062640843694078);
    }

    #[test]
    fn the_derivation_rule_is_deterministic_and_stream_separated() {
        let a1 = derive_asset_seed(20260601, "site-north-pv-1/forecast/v1");
        let a2 = derive_asset_seed(20260601, "site-north-pv-1/forecast/v1");
        let b = derive_asset_seed(20260601, "site-north-pv-1/forecast/v1-error-pv-ramp");
        let c = derive_asset_seed(20260602, "site-north-pv-1/forecast/v1");
        assert_eq!(a1, a2);
        assert_ne!(a1, b, "a different forecast version must get its own stream");
        assert_ne!(a1, c, "a different master seed must change the stream");
    }

    #[test]
    fn pv_shape_matches_the_scenario_profile_bounds() {
        let p = pv_shape(12 * 3600, 1.5, 0.85);
        // The bell peaks between sunrise and sunset; noon sits at x ≈ 0.419.
        assert!(p > 1.0 && p < 1.28, "noon PV ≈ 1.195 MW, got {p}");
        assert_eq!(pv_shape(0, 1.5, 0.85), 0.0, "night: exactly zero");
        assert_eq!(pv_shape(22 * 3600, 1.5, 0.85), 0.0, "after sunset: zero");
        // The bell is exactly zero at sunrise and capped at rated.
        assert_eq!(pv_shape(19_800, 1.5, 0.85), 0.0, "sunrise: zero");
        let peak = pv_shape((19_800 + 75_600) / 2, 1.5, 0.85);
        assert!((peak - 1.275).abs() < 1e-9, "bell peak = rated * peak_factor");
    }

    #[test]
    fn load_shape_matches_the_scenario_profile_bounds() {
        // Night base 0.25 * rated peak; peaks never overlap and stay below
        // the rated peak (simulator note).
        assert!((load_shape(0, 0.8) - 0.2).abs() < 1e-12);
        let morning_peak = load_shape(7 * 3600 + 1800, 0.8); // 07:30 center
        assert!(morning_peak > 0.2 && morning_peak <= 0.8);
        let evening_peak = load_shape(19 * 3600, 0.8); // 19:00 center
        assert!(evening_peak > 0.4 && evening_peak <= 0.8);
        assert!(load_shape(12 * 3600, 0.8) <= 0.8);
    }

    #[test]
    fn quantize_is_round_half_away_from_zero_and_fmt_is_three_decimals() {
        assert_eq!(quantize(0.4175), 418);
        assert_eq!(quantize(-0.4175), -418);
        assert_eq!(quantize(1.0), 1000);
        assert_eq!(quantize(0.0), 0);
        assert_eq!(fmt_milli(418), "0.418");
        assert_eq!(fmt_milli(-418), "-0.418");
        assert_eq!(fmt_milli(1000), "1.000");
        assert_eq!(fmt_milli(0), "0.000");
        assert_eq!(fmt_milli(-1), "-0.001");
    }

    #[test]
    fn generated_fixture_is_deterministic_from_the_seed() {
        let a = generate_central_fixture();
        let b = generate_central_fixture();
        assert_eq!(a, b, "same seed must regenerate the identical fixture");
        let c = generate_error_pv_ramp_fixture();
        let d = generate_error_pv_ramp_fixture();
        assert_eq!(c, d);
        assert_ne!(a, c, "the error variant is a different fixture version");
    }

    #[test]
    fn the_specs_table_has_no_availability_series() {
        // The availability arm of generate_power_series is never taken:
        // every spec is a power quantity.
        for spec in SERIES_SPECS.iter() {
            assert_ne!(spec.quantity, ForecastQuantity::AvailabilityFraction);
        }
    }

    #[test]
    fn forecast_version_validation() {
        assert!(ForecastVersion::new("v1").is_ok());
        assert_eq!(
            ForecastVersion::new(""),
            Err(ForecastFixtureError::EmptyForecastVersion)
        );
        assert_eq!(
            ForecastVersion::new("v1 beta"),
            Err(ForecastFixtureError::ForecastVersionHasWhitespace)
        );
    }

    #[test]
    fn publication_is_deterministic_and_uses_the_frozen_subject() {
        let version = ForecastVersion::new("v1").unwrap();
        let correlation = FlexUuid::nil();
        let publication = forecast_publication(ForecastPublicationInput {
            correlation_id: correlation,
            tenant_id: TenantId::new(correlation),
            portfolio_id: PortfolioId::new(correlation),
            site_id: SiteId::new(correlation),
            asset_id: AssetId::new(correlation),
            forecast_version: &version,
            provenance: Provenance::Synthetic,
            event_time: EventTime::from_unix_millis(1_000).expect("valid timestamp"),
        })
        .unwrap();
        assert_eq!(
            publication.destination_subject.as_str(),
            "flexcore.v1.forecast.forecast_published"
        );
        assert_eq!(
            publication.idempotency_key.as_str(),
            "forecast_published:v1:synthetic"
        );
        // Same version + provenance → same key (deterministic from the
        // forecast version; republication dedupes).
        let again = forecast_publication(ForecastPublicationInput {
            correlation_id: FlexUuid::nil(),
            tenant_id: TenantId::new(correlation),
            portfolio_id: PortfolioId::new(correlation),
            site_id: SiteId::new(correlation),
            asset_id: AssetId::new(correlation),
            forecast_version: &version,
            provenance: Provenance::Synthetic,
            event_time: EventTime::from_unix_millis(2_000).expect("valid timestamp"),
        })
        .unwrap();
        assert_eq!(again.idempotency_key, publication.idempotency_key);
        // A different version gets a different key.
        let v2 = ForecastVersion::new("v1-error-pv-ramp").unwrap();
        let other = forecast_publication(ForecastPublicationInput {
            correlation_id: FlexUuid::nil(),
            tenant_id: TenantId::new(correlation),
            portfolio_id: PortfolioId::new(correlation),
            site_id: SiteId::new(correlation),
            asset_id: AssetId::new(correlation),
            forecast_version: &v2,
            provenance: Provenance::Synthetic,
            event_time: EventTime::from_unix_millis(2_000).expect("valid timestamp"),
        })
        .unwrap();
        assert_ne!(other.idempotency_key, publication.idempotency_key);
    }

    #[test]
    fn the_error_variant_record_is_complete() {
        let fixture = generate_error_pv_ramp_fixture();
        let variant = fixture.error_variant().expect("the error variant is set");
        assert_eq!(variant.variant_id(), "pv-ramp");
        assert_eq!(variant.applies_to(), ForecastQuantity::PvOutputMw);
        assert!(variant.direction_downward());
        assert_eq!(variant.magnitude_milli(), 800);
        assert!((variant.magnitude() - 0.8).abs() < 1e-12);
        assert_eq!(variant.window_first_interval(), 720);
        assert_eq!(variant.window_last_exclusive(), 780);
        let central = generate_central_fixture();
        assert!(central.error_variant().is_none());
    }

    #[test]
    fn validate_rejects_unlabelled_and_unversioned_forecasts() {
        let good = generate_central_fixture();
        let mut raw = UnvalidatedForecastInput {
            provenance: Some(Provenance::Synthetic),
            forecast_version: Some("v1".to_string()),
            variant: good.meta.variant().to_string(),
            scenario_id: good.meta.scenario_id().to_string(),
            scenario_version: good.meta.scenario_version().to_string(),
            master_seed: good.meta.master_seed(),
            method: FixtureMethod::recorded(),
            horizon_start_millis: good.grid.horizon_start_millis(),
            horizon_start_iso: good.grid.horizon_start_iso().to_string(),
            horizon_millis: good.grid.horizon_millis(),
            interval_millis: good.grid.interval_millis(),
            scenario_cadence_seconds: good.grid.scenario_cadence_seconds(),
            error_variant: None,
            series: Vec::new(),
        };
        // Rebuild the raw series from the generated fixture's data.
        raw.series = good
            .series()
            .iter()
            .map(|s| UnvalidatedSeries {
                asset_key: s.asset_key().to_string(),
                site_key: s.site_key().to_string(),
                quantity: s.values().quantity,
                values: s.values().values.to_vec(),
            })
            .collect();
        assert!(validate_forecast_input(raw.clone()).is_ok());

        // Unlabelled → the named rejection.
        let mut unlabelled = raw.clone();
        unlabelled.provenance = None;
        assert_eq!(
            validate_forecast_input(unlabelled),
            Err(ForecastFixtureError::MissingProvenanceLabel)
        );

        // Unversioned → the named rejection.
        let mut unversioned = raw;
        unversioned.forecast_version = None;
        assert_eq!(
            validate_forecast_input(unversioned),
            Err(ForecastFixtureError::MissingForecastVersion)
        );
    }

    #[test]
    fn validate_rejects_sign_finiteness_and_count_violations() {
        let good = generate_central_fixture();
        let base = |series: Vec<UnvalidatedSeries>| UnvalidatedForecastInput {
            provenance: Some(Provenance::Synthetic),
            forecast_version: Some("v1".to_string()),
            variant: good.meta.variant().to_string(),
            scenario_id: good.meta.scenario_id().to_string(),
            scenario_version: good.meta.scenario_version().to_string(),
            master_seed: good.meta.master_seed(),
            method: FixtureMethod::recorded(),
            horizon_start_millis: good.grid.horizon_start_millis(),
            horizon_start_iso: good.grid.horizon_start_iso().to_string(),
            horizon_millis: good.grid.horizon_millis(),
            interval_millis: good.grid.interval_millis(),
            scenario_cadence_seconds: good.grid.scenario_cadence_seconds(),
            error_variant: None,
            series,
        };
        let mut series: Vec<UnvalidatedSeries> = good
            .series()
            .iter()
            .map(|s| UnvalidatedSeries {
                asset_key: s.asset_key().to_string(),
                site_key: s.site_key().to_string(),
                quantity: s.values().quantity,
                values: s.values().values.to_vec(),
            })
            .collect();

        // A negative PV output violates the canonical sign rule.
        let mut negative_pv = series.clone();
        negative_pv[0].values[500] = -0.001;
        assert!(matches!(
            validate_forecast_input(base(negative_pv)),
            Err(ForecastFixtureError::PowerSignViolation { .. })
        ));

        // A positive load violates it too (consumption is negative).
        let mut positive_load = series.clone();
        positive_load[1].values[500] = 0.001;
        assert!(matches!(
            validate_forecast_input(base(positive_load)),
            Err(ForecastFixtureError::PowerSignViolation { .. })
        ));

        // NaN is rejected.
        let mut nan = series.clone();
        nan[0].values[0] = f64::NAN;
        assert!(matches!(
            validate_forecast_input(base(nan)),
            Err(ForecastFixtureError::NonFiniteForecastValue { .. })
        ));

        // Availability outside [0, 1] is rejected.
        let mut availability = series.clone();
        availability[4].values[0] = 1.5;
        assert!(matches!(
            validate_forecast_input(base(availability)),
            Err(ForecastFixtureError::AvailabilityOutOfRange { .. })
        ));

        // A truncated series is a count mismatch.
        let mut short = series.clone();
        short[0].values.truncate(1439);
        assert!(matches!(
            validate_forecast_input(base(short)),
            Err(ForecastFixtureError::SeriesIntervalCountMismatch { .. })
        ));

        // An empty series list is rejected.
        assert_eq!(
            validate_forecast_input(base(Vec::new())),
            Err(ForecastFixtureError::NoSeries)
        );

        // A grid that does not echo the scenario cadence is rejected.
        let mut cadence = base(series.clone());
        cadence.scenario_cadence_seconds = 900;
        assert!(matches!(
            validate_forecast_input(cadence),
            Err(ForecastFixtureError::IntervalNotScenarioCadence { .. })
        ));

        // A method block that is not the recorded one is rejected.
        let mut method = base(series);
        method.method = FixtureMethod { jitter_fraction_milli: 50 };
        assert_eq!(
            validate_forecast_input(method),
            Err(ForecastFixtureError::MethodMismatch)
        );
    }

    #[test]
    fn validate_rejects_bad_error_variants() {
        let ok_variant = UnvalidatedErrorVariant {
            variant_id: "pv-ramp".to_string(),
            applies_to: ForecastQuantity::PvOutputMw,
            direction_downward: true,
            magnitude_milli: 800,
            window_start_iso: "2026-06-01T12:00:00Z".to_string(),
            window_end_iso: "2026-06-01T13:00:00Z".to_string(),
            window_first_interval: 720,
            window_last_exclusive: 780,
            note: "n".to_string(),
        };
        let good = generate_central_fixture();
        let base = |variant: Option<UnvalidatedErrorVariant>| UnvalidatedForecastInput {
            provenance: Some(Provenance::Synthetic),
            forecast_version: Some("v1".to_string()),
            variant: good.meta.variant().to_string(),
            scenario_id: good.meta.scenario_id().to_string(),
            scenario_version: good.meta.scenario_version().to_string(),
            master_seed: good.meta.master_seed(),
            method: FixtureMethod::recorded(),
            horizon_start_millis: good.grid.horizon_start_millis(),
            horizon_start_iso: good.grid.horizon_start_iso().to_string(),
            horizon_millis: good.grid.horizon_millis(),
            interval_millis: good.grid.interval_millis(),
            scenario_cadence_seconds: good.grid.scenario_cadence_seconds(),
            error_variant: variant,
            series: Vec::new(),
        };
        // An empty series list fails only AFTER the variant validates, so
        // check the variant rejections first via their distinct errors.
        let mut bad_magnitude = ok_variant.clone();
        bad_magnitude.magnitude_milli = 1000;
        assert!(matches!(
            validate_forecast_input(base(Some(bad_magnitude))),
            Err(ForecastFixtureError::InvalidErrorVariantMagnitude { .. })
        ));
        let mut zero_magnitude = ok_variant.clone();
        zero_magnitude.magnitude_milli = 0;
        assert!(matches!(
            validate_forecast_input(base(Some(zero_magnitude))),
            Err(ForecastFixtureError::InvalidErrorVariantMagnitude { .. })
        ));
        let mut upward = ok_variant.clone();
        upward.direction_downward = false;
        assert_eq!(
            validate_forecast_input(base(Some(upward))),
            Err(ForecastFixtureError::UnsupportedErrorVariantDirection)
        );
        let mut inverted = ok_variant.clone();
        inverted.window_first_interval = 780;
        inverted.window_last_exclusive = 720;
        assert!(matches!(
            validate_forecast_input(base(Some(inverted))),
            Err(ForecastFixtureError::InvalidErrorVariantWindow { .. })
        ));
        let mut beyond = ok_variant;
        beyond.window_last_exclusive = 1441;
        assert!(matches!(
            validate_forecast_input(base(Some(beyond))),
            Err(ForecastFixtureError::InvalidErrorVariantWindow { .. })
        ));
    }

    #[test]
    fn the_grid_matches_the_p3_03_configuration() {
        let fixture = generate_central_fixture();
        let config = SchedulingGridConfig::new(86_400_000, 60_000)
            .expect("24 h over 60 s is a valid P3.03 grid");
        assert!(fixture.grid().matches_config(&config));
        assert_eq!(fixture.grid().interval_count(), 1440);
        // The fixture's interval starts match the P3.03 grid arithmetic
        // exactly (start-inclusive, end-exclusive).
        for i in [0usize, 1, 719, 720, 1439] {
            assert_eq!(
                fixture.grid().interval_start_millis(i),
                Some(1_780_272_000_000 + (i as i64) * 60_000)
            );
        }
        assert_eq!(fixture.grid().interval_start_millis(1440), None);
        // Interval 0 starts at the horizon start; the horizon end belongs to
        // no interval.
        assert_eq!(
            fixture.grid().interval_start_millis(0),
            Some(1_780_272_000_000)
        );
    }
}
