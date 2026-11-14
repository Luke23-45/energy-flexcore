// P2.13/P2.14/P2.15 — The typed scenario model: serde structures for
// `scenarios/synthetic-portfolio/scenario-v1.json` (P2.13) and their
// validated runtime form.
//
// Rules encoded here:
//
// - A scenario is a **data-only, versioned artefact** (P2.13 §1). Loading
//   is all-or-nothing: a file that fails validation is rejected whole —
//   never partially loaded, never defaulted into plausibility.
// - The `synthetic: true` flag is a **load guard**: a scenario that claims
//   not to be synthetic is rejected, so nobody can silently repurpose the
//   simulator's output as measured data (the 0.x simulation-only rule).
// - The `faults` section is parsed and validated exactly like the rest of
//   the scenario (P2.15 step 2): fault injection is **scenario
//   configuration, not ad-hoc code paths**. `enabled: false` makes the
//   whole plan inert — the v1 exit-gate run stays all-`GOOD`.
// - The fault plan scope is recorded, not just implemented: these are
//   simulator faults for testing the platform's handling of missing, stale,
//   suspect, bad, duplicate, out-of-order, and conflicting values. They are
//   **not claims about real device failure modes**.
//
// Validation decisions recorded once (P2.14):
// - The engine runs the scenario default cadence for every asset. Per-asset
//   cadence overrides are **validated as positive** but rejected with a
//   clear error if non-empty: the v1 scenario sets none, and a
//   multi-cadence stream is not silently approximated by the default.
// - Window times (`"HH:MM"` UTC) may wrap midnight (end <= start), matching
//   the EVSE overnight window in v1.
// - Behaviour-profile mode spellings travel verbatim from the scenario in
//   declared order: index 0 is the charging state's name, 1 the discharging
//   state's, 2 the idle state's (the validation error states this).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use time::OffsetDateTime;

// --- The parsed (serde) form -------------------------------------------------

/// The scenario file's top level, exactly as laid out by P2.13 §1.
#[derive(Debug, Clone, Deserialize)]
struct ScenarioFile {
    meta: ScenarioMeta,
    portfolio: PortfolioSection,
    horizon: Horizon,
    cadence: Cadence,
    seeds: Seeds,
    faults: FaultPlanRaw,
}

#[derive(Debug, Clone, Deserialize)]
struct ScenarioMeta {
    scenario_id: String,
    #[allow(dead_code)]
    name: String,
    version: String,
    synthetic: bool,
    #[allow(dead_code)]
    description: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PortfolioSection {
    tenant: TenantRaw,
    #[serde(rename = "portfolio")]
    portfolio_info: PortfolioInfo,
    sites: Vec<SiteRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct TenantRaw {
    tenant_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PortfolioInfo {
    #[allow(dead_code)]
    portfolio_key: String,
    #[allow(dead_code)]
    operator: String,
    #[allow(dead_code)]
    market_region: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SiteRaw {
    site_key: String,
    #[allow(dead_code)]
    timezone: String,
    reactive_sign_convention: flexcore_contracts::units::ReactiveSignConvention,
    pcc: PccRaw,
    assets: Vec<AssetRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct PccRaw {
    #[allow(dead_code)]
    pcc_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AssetRaw {
    asset_key: String,
    asset_type: AssetType,
    rated_power_mw: Option<f64>,
    rated_energy_mwh: Option<f64>,
    rated_peak_mw: Option<f64>,
    behaviour_profile: BehaviourProfileRaw,
}

/// The scenario asset types (the P2.13 §5 vocabulary). Public since the
/// P5.14 adapter integration: the validated `AssetSpec` public field
/// already exposes the type, and the P5.14 run record names the per-asset
/// model identity — a public interface may not carry a private type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetType {
    Bess,
    Pv,
    FlexibleLoad,
    Evse,
    SiteMeter,
}

#[derive(Debug, Clone, Deserialize)]
struct BehaviourProfileRaw {
    #[allow(dead_code)]
    profile_id: String,
    synthetic: bool,
    params: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct Horizon {
    start: String,
    duration_hours: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct Cadence {
    default_seconds: i64,
    per_asset_overrides: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Seeds {
    master_seed: u64,
    #[allow(dead_code)]
    derivation: String,
}

/// The fault-injection plan (P2.15): scenario configuration, versioned with
/// the scenario. `enabled: false` renders the whole plan inert.
#[derive(Debug, Clone, Deserialize)]
struct FaultPlanRaw {
    enabled: bool,
    freshness_limit_seconds: Option<i64>,
    injections: Vec<FaultInjectionRaw>,
    #[allow(dead_code)]
    note: Option<String>,
}

/// One per-asset, per-interval fault injection (P2.15 step 1). The optional
/// parameters are required or rejected per kind at validation.
#[derive(Debug, Clone, Deserialize)]
struct FaultInjectionRaw {
    fault: FaultKind,
    asset_key: String,
    from: String,
    until: String,
    delay_seconds: Option<u64>,
    hold_ticks: Option<u32>,
    factor: Option<f64>,
}

/// The seven fault kinds (P2.15 step 1). Serialized snake_case so the
/// scenario file reads naturally. Public since the P5.14 adapter
/// integration: the validated `FaultSpec` public field already exposes
/// the kind, and the dispatch-transport composition (the D-P4.16-4
/// mapping) matches on it — a public interface may not carry a private
/// type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    /// Omit the frame for the interval (→ absence; the pipeline's gap
    /// detector turns that into `MISSING` — a provider itself can never
    /// emit `MISSING`, P2.04 D-T6).
    Drop,
    /// Release the frame late on the scenario clock (→ age past the
    /// freshness limit ⇒ `STALE` per the P2.08 rule).
    Delay,
    /// Emit an active power scaled past the rated limit (→ `SUSPECT`,
    /// source code `OUT_OF_RANGE`).
    OutOfRange,
    /// Emit the frame with the source-invalid code (→ `BAD`).
    SourceInvalid,
    /// Emit the frame twice, identical.
    Duplicate,
    /// Hold the frame `hold_ticks` ticks and release it after a later
    /// tick's frame (→ arrival order violates event order).
    OutOfOrder,
    /// Emit the frame twice with the same identity but a different value.
    ConflictingDuplicate,
}

// --- The validated runtime form ----------------------------------------------

/// Why a scenario file was rejected. The whole file is rejected — there is
/// no partial scenario (the all-or-nothing load rule).
#[derive(Debug, Clone)]
pub struct ScenarioError {
    detail: String,
}

impl ScenarioError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid scenario: {}", self.detail)
    }
}

impl std::error::Error for ScenarioError {}

/// The validated scenario: the runtime form the engine consumes. Field
/// meanings match the scenario file (P2.13 §1); everything here has been
/// checked by [`load_scenario`].
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Scenario identity and version (replay names both, P2.13 §4).
    pub meta: ValidatedMeta,
    /// The scenario's tenant key (stable scenario key; the mapping to
    /// registry IDs happens at registration, P2.13 §1 — never by direct
    /// SQL).
    pub tenant_key: String,
    /// The sites in scenario order; assets in listed order per site.
    pub sites: Vec<SiteSpec>,
    /// The horizon start on the scenario clock (UTC, ms since epoch).
    pub start_ms: i64,
    /// The horizon length in hours (24 for the exit-gate scenario).
    pub duration_hours: f64,
    /// The scenario-wide cadence in seconds (D-SC3: 60).
    pub cadence_seconds: i64,
    /// The master seed (D-SC4; per-asset derivation is `rng.rs`).
    pub master_seed: u64,
    /// The validated fault plan (inert when `enabled` is false).
    pub faults: FaultPlan,
    /// The total number of ticks the horizon spans at the default cadence.
    pub total_ticks: u64,
}

/// The validated `meta` section.
#[derive(Debug, Clone)]
pub struct ValidatedMeta {
    pub scenario_id: String,
    pub version: String,
}

/// One validated site: its key, its declared reactive convention (P0.07 §3;
/// P2.04 D-T4 — travels on every reactive record this site's assets emit),
/// and its assets.
#[derive(Debug, Clone)]
pub struct SiteSpec {
    pub site_key: String,
    pub reactive_convention: flexcore_contracts::units::ReactiveSignConvention,
    pub assets: Vec<AssetSpec>,
}

/// One validated asset.
#[derive(Debug, Clone)]
pub struct AssetSpec {
    pub asset_key: String,
    pub asset_type: AssetType,
    /// Rated active power in MW (for the flexible load: the rated peak).
    pub rated_power_mw: f64,
    /// Rated energy in MWh (BESS only; `None` for the other types).
    pub rated_energy_mwh: Option<f64>,
    pub params: BehaviourParams,
}

/// The validated per-asset behaviour parameters, selected by asset type.
#[derive(Debug, Clone)]
pub enum BehaviourParams {
    Bess(BessParams),
    Pv(PvParams),
    Load(LoadParams),
    Evse(EvseParams),
    Meter(MeterParams),
}

/// BESS parameters (synthetic rectangular-efficiency daily cycle, P2.13 §5.1).
#[derive(Debug, Clone)]
pub struct BessParams {
    /// Charge window, seconds-of-day UTC; may wrap midnight.
    pub charge_window: (u32, u32),
    /// Discharge window, seconds-of-day UTC; may wrap midnight.
    pub discharge_window: (u32, u32),
    pub charge_power_mw: f64,
    pub discharge_power_mw: f64,
    pub initial_soc: f64,
    pub soc_floor: f64,
    pub soc_ceiling: f64,
    pub round_trip_efficiency: f64,
    /// Declared operating-mode spellings, in declared order: 0 = charging,
    /// 1 = discharging, 2 = idle.
    pub operating_mode_states: [String; 3],
    pub health_state: String,
}

/// PV parameters (synthetic clear-sky cosine bell, P2.13 §5.2). Curtailment
/// is noted in the scenario as a capability reference but NOT exercised —
/// no Phase 2 mechanism reduces the output below the shape.
#[derive(Debug, Clone)]
pub struct PvParams {
    pub sunrise_sod: u32,
    pub sunset_sod: u32,
    pub peak_factor: f64,
}

/// Flexible-load parameters (declared baseline shape, P2.13 §5.3). The
/// reducible range travels as scenario data for Phase 3; the simulator
/// emits the baseline (and the baseline-reference quantity when
/// configured) plus seeded jitter.
#[derive(Debug, Clone)]
pub struct LoadParams {
    pub reducible_range_mw: f64,
    pub emit_baseline_reference: bool,
    pub jitter_fraction: f64,
}

/// EVSE parameters (declared connection windows, P2.13 §5.4). Windows may
/// wrap midnight.
#[derive(Debug, Clone)]
pub struct EvseParams {
    pub connection_windows: Vec<(u32, u32)>,
    pub charging_power_mw: f64,
    pub initial_soc: f64,
    pub soc_target: f64,
    pub emit_energy: bool,
    pub jitter_fraction: f64,
}

/// Site-meter parameters: the PCC active power is the aggregated sum of the
/// site's assets' net active power (losses = 0, non-modelled load = 0 — a
/// consistency fixture, P2.13 §5.5); reactive power is this configured
/// constant under the site's declared convention.
#[derive(Debug, Clone)]
pub struct MeterParams {
    pub q_constant_mvar: f64,
}

/// The validated fault plan. Inert unless `enabled`.
#[derive(Debug, Clone)]
pub struct FaultPlan {
    pub enabled: bool,
    /// The freshness limit (seconds) the `delay` fault is sized against —
    /// configuration, never a hard-coded constant (P2.08 step 3).
    pub freshness_limit_seconds: i64,
    pub injections: Vec<FaultSpec>,
}

/// One validated injection: a fixed kind over `[from_ms, until_ms)` on the
/// scenario clock for one asset.
#[derive(Debug, Clone)]
pub struct FaultSpec {
    pub kind: FaultKind,
    pub asset_key: String,
    pub from_ms: i64,
    pub until_ms: i64,
    /// `delay` kind: the delay in whole cadence ticks (validated > 0).
    pub delay_ticks: u64,
    /// `out_of_order` kind: the hold in whole cadence ticks (validated >= 1).
    pub hold_ticks: u64,
    /// `out_of_range` kind: the value scale versus the rated power (> 1).
    pub factor: f64,
}

// --- Parsing helpers ----------------------------------------------------------

/// Parses `"HH:MM"` into seconds-of-day.
fn parse_hh_mm(s: &str) -> Result<u32, ScenarioError> {
    if let Some((h, m)) = s.split_once(':') {
        if let (Ok(h), Ok(m)) = (h.parse::<u32>(), m.parse::<u32>()) {
            if h < 24 && m < 60 {
                return Ok(h * 3600 + m * 60);
            }
        }
    }
    Err(ScenarioError::new(format!("invalid HH:MM UTC time '{s}'")))
}

/// Parses an RFC 3339 UTC instant; a non-zero offset is rejected, never
/// converted (the P0.09/P1.05 UTC rule).
fn parse_utc_instant(s: &str, field: &str) -> Result<i64, ScenarioError> {
    let t = OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .map_err(|e| ScenarioError::new(format!("{field}: invalid RFC 3339 timestamp '{s}': {e}")))?;
    if !t.offset().is_utc() {
        return Err(ScenarioError::new(format!(
            "{field}: timestamp must carry the UTC offset; non-UTC offsets are rejected, not converted"
        )));
    }
    Ok((t.unix_timestamp_nanos() / 1_000_000) as i64)
}

fn require_finite(value: f64, field: &str) -> Result<f64, ScenarioError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ScenarioError::new(format!("{field} must be finite")))
    }
}

/// A required, finite, positive rated value.
fn required_positive(value: Option<f64>, field: &str) -> Result<f64, ScenarioError> {
    let v = value.ok_or_else(|| ScenarioError::new(format!("{field} is required for this asset type")))?;
    let v = require_finite(v, field)?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err(ScenarioError::new(format!("{field} must be > 0")))
    }
}

fn require_soc(value: f64, field: &str) -> Result<f64, ScenarioError> {
    let v = require_finite(value, field)?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err(ScenarioError::new(format!("{field} must be a fraction in [0,1]")))
    }
}

/// Parses a window pair, allowing midnight wrap (end <= start).
fn parse_window(pair: &(String, String)) -> Result<(u32, u32), ScenarioError> {
    Ok((parse_hh_mm(&pair.0)?, parse_hh_mm(&pair.1)?))
}

// --- Typed profile parameter parsing ------------------------------------------

macro_rules! parse_params {
    ($value:expr, $ty:ty, $asset_key:expr) => {
        serde_json::from_value::<$ty>($value)
            .map_err(|e| ScenarioError::new(format!("asset '{}': invalid behaviour params: {e}", $asset_key)))?
    };
}

/// Raw (serde) parameter shapes, parsed from the JSON `params` object based
/// on the asset type. `deny_unknown_fields` keeps the versioned scenario
/// schema honest: an unknown parameter is a scenario error, not a silent
/// ignore.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BessParamsRaw {
    charge_window_utc: (String, String),
    discharge_window_utc: (String, String),
    charge_power_mw: f64,
    discharge_power_mw: f64,
    initial_soc: f64,
    soc_floor: f64,
    soc_ceiling: f64,
    round_trip_efficiency: f64,
    operating_mode_states: Vec<String>,
    health_state: String,
    #[allow(dead_code)]
    availability: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PvParamsRaw {
    sunrise_utc: String,
    sunset_utc: String,
    peak_factor: f64,
    #[allow(dead_code)]
    curtailment_exercised: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadParamsRaw {
    #[allow(dead_code)]
    baseline_shape: String,
    reducible_range_mw: f64,
    emit_baseline_reference: bool,
    jitter_fraction: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvseParamsRaw {
    connection_windows_utc: Vec<(String, String)>,
    charging_power_mw: f64,
    initial_soc: f64,
    soc_target: f64,
    emit_energy: bool,
    jitter_fraction: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeterParamsRaw {
    q_constant_mvar: f64,
}

fn validate_bess_params(raw: BessParamsRaw, key: &str) -> Result<BessParams, ScenarioError> {
    let charge_power_mw = required_positive(Some(raw.charge_power_mw), "charge_power_mw")?;
    let discharge_power_mw = required_positive(Some(raw.discharge_power_mw), "discharge_power_mw")?;
    let initial_soc = require_soc(raw.initial_soc, "initial_soc")?;
    let soc_floor = require_soc(raw.soc_floor, "soc_floor")?;
    let soc_ceiling = require_soc(raw.soc_ceiling, "soc_ceiling")?;
    if soc_floor > soc_ceiling {
        return Err(ScenarioError::new(format!(
            "asset '{key}': soc_floor must not exceed soc_ceiling"
        )));
    }
    let eff = require_finite(raw.round_trip_efficiency, "round_trip_efficiency")?;
    if !(eff > 0.0 && eff <= 1.0) {
        return Err(ScenarioError::new(format!(
            "asset '{key}': round_trip_efficiency must be in (0,1]"
        )));
    }
    if raw.operating_mode_states.len() != 3 {
        return Err(ScenarioError::new(format!(
            "asset '{key}': operating_mode_states must list exactly the three declared states, in order (0 = charging, 1 = discharging, 2 = idle)"
        )));
    }
    let mut states = raw.operating_mode_states;
    if states.iter().any(|s| s.trim().is_empty()) {
        return Err(ScenarioError::new(format!(
            "asset '{key}': operating_mode_states entries must be non-empty"
        )));
    }
    Ok(BehaviourParams::Bess(BessParams {
        charge_window: parse_window(&raw.charge_window_utc)?,
        discharge_window: parse_window(&raw.discharge_window_utc)?,
        charge_power_mw,
        discharge_power_mw,
        initial_soc,
        soc_floor,
        soc_ceiling,
        round_trip_efficiency: eff,
        operating_mode_states: [states.remove(0), states.remove(0), states.remove(0)],
        health_state: raw.health_state,
    }))
}

fn validate_pv_params(raw: PvParamsRaw, key: &str) -> Result<BehaviourParams, ScenarioError> {
    let peak = require_finite(raw.peak_factor, "peak_factor")?;
    if !(peak > 0.0 && peak <= 1.0) {
        return Err(ScenarioError::new(format!(
            "asset '{key}': peak_factor must be in (0,1]"
        )));
    }
    let sunrise = parse_hh_mm(&raw.sunrise_utc)?;
    let sunset = parse_hh_mm(&raw.sunset_utc)?;
    if sunrise >= sunset {
        return Err(ScenarioError::new(format!(
            "asset '{key}': sunrise_utc must precede sunset_utc (the clear-sky bell is same-day UTC)"
        )));
    }
    Ok(BehaviourParams::Pv(PvParams {
        sunrise_sod: sunrise,
        sunset_sod: sunset,
        peak_factor: peak,
    }))
}

fn validate_load_params(raw: LoadParamsRaw, key: &str) -> Result<BehaviourParams, ScenarioError> {
    let reducible = required_positive(Some(raw.reducible_range_mw), "reducible_range_mw")?;
    let jitter = require_finite(raw.jitter_fraction, "jitter_fraction")?;
    if !(0.0..1.0).contains(&jitter) {
        return Err(ScenarioError::new(format!(
            "asset '{key}': jitter_fraction must be in [0,1)"
        )));
    }
    Ok(BehaviourParams::Load(LoadParams {
        reducible_range_mw: reducible,
        emit_baseline_reference: raw.emit_baseline_reference,
        jitter_fraction: jitter,
    }))
}

fn validate_evse_params(raw: EvseParamsRaw, key: &str) -> Result<EvseParams, ScenarioError> {
    let charging = required_positive(Some(raw.charging_power_mw), "charging_power_mw")?;
    let initial = require_soc(raw.initial_soc, "initial_soc")?;
    let target = require_soc(raw.soc_target, "soc_target")?;
    if initial > target {
        return Err(ScenarioError::new(format!(
            "asset '{key}': initial_soc must not exceed soc_target (charging-only profile, no V2G)"
        )));
    }
    let jitter = require_finite(raw.jitter_fraction, "jitter_fraction")?;
    if !(0.0..1.0).contains(&jitter) {
        return Err(ScenarioError::new(format!(
            "asset '{key}': jitter_fraction must be in [0,1)"
        )));
    }
    if raw.connection_windows_utc.is_empty() {
        return Err(ScenarioError::new(format!(
            "asset '{key}': at least one connection window is required"
        )));
    }
    let mut windows = Vec::with_capacity(raw.connection_windows_utc.len());
    for w in &raw.connection_windows_utc {
        windows.push(parse_window(w)?);
    }
    Ok(EvseParams {
        connection_windows: windows,
        charging_power_mw: charging,
        initial_soc: initial,
        soc_target: target,
        emit_energy: raw.emit_energy,
        jitter_fraction: jitter,
    })
}

fn validate_meter_params(raw: MeterParamsRaw, key: &str) -> Result<BehaviourParams, ScenarioError> {
    let q = require_finite(raw.q_constant_mvar, "q_constant_mvar")?;
    Ok(BehaviourParams::Meter(MeterParams { q_constant_mvar: q }))
}

fn validate_asset(raw: AssetRaw) -> Result<AssetSpec, ScenarioError> {
    let key = raw.asset_key.clone();
    if key.trim().is_empty() {
        return Err(ScenarioError::new("asset_key must be non-empty"));
    }
    if !raw.behaviour_profile.synthetic {
        return Err(ScenarioError::new(format!(
            "asset '{key}': behaviour_profile.synthetic must be true — every profile is synthetic and must say so"
        )));
    }
    let (asset_type, rated, energy, params) = match raw.asset_type {
        AssetType::Bess => {
            let rated = required_positive(raw.rated_power_mw, "rated_power_mw")?;
            let energy = required_positive(raw.rated_energy_mwh, "rated_energy_mwh")?;
            let BehaviourParams::Bess(bess) = validate_bess_params(
                parse_params!(raw.behaviour_profile.params, BessParamsRaw, key),
                &key,
            )?
            else {
                // validate_bess_params always returns the Bess variant; the
                // match is structural, not a silent fallback.
                unreachable!("validate_bess_params returns BehaviourParams::Bess")
            };
            if bess.charge_power_mw > rated || bess.discharge_power_mw > rated {
                return Err(ScenarioError::new(format!(
                    "asset '{key}': charge/discharge power must respect the rated power {rated} MW"
                )));
            }
            (AssetType::Bess, rated, Some(energy), BehaviourParams::Bess(bess))
        }
        AssetType::Pv => {
            let rated = required_positive(raw.rated_power_mw, "rated_power_mw")?;
            let params = validate_pv_params(
                parse_params!(raw.behaviour_profile.params, PvParamsRaw, key),
                &key,
            )?;
            (AssetType::Pv, rated, None, params)
        }
        AssetType::FlexibleLoad => {
            let rated = required_positive(raw.rated_peak_mw, "rated_peak_mw")?;
            let params = validate_load_params(
                parse_params!(raw.behaviour_profile.params, LoadParamsRaw, key),
                &key,
            )?;
            (AssetType::FlexibleLoad, rated, None, params)
        }
        AssetType::Evse => {
            let rated = required_positive(raw.rated_power_mw, "rated_power_mw")?;
            let evse = validate_evse_params(
                parse_params!(raw.behaviour_profile.params, EvseParamsRaw, key),
                &key,
            )?;
            if evse.charging_power_mw > rated {
                return Err(ScenarioError::new(format!(
                    "asset '{key}': charging_power_mw must respect the rated power {rated} MW"
                )));
            }
            (AssetType::Evse, rated, None, BehaviourParams::Evse(evse))
        }
        AssetType::SiteMeter => {
            let rated = required_positive(raw.rated_power_mw, "rated_power_mw")?;
            let params = validate_meter_params(
                parse_params!(raw.behaviour_profile.params, MeterParamsRaw, key),
                &key,
            )?;
            (AssetType::SiteMeter, rated, None, params)
        }
    };
    Ok(AssetSpec {
        asset_key: key,
        asset_type,
        rated_power_mw: rated,
        rated_energy_mwh: energy,
        params,
    })
}

fn validate_fault_plan(
    raw: FaultPlanRaw,
    asset_keys: &BTreeMap<String, ()>,
    cadence_seconds: i64,
    start_ms: i64,
    horizon_end_ms: i64,
) -> Result<FaultPlan, ScenarioError> {
    let has_delay = raw.injections.iter().any(|i| i.fault == FaultKind::Delay);
    let freshness_limit_seconds = match raw.freshness_limit_seconds {
        Some(s) if s > 0 => s,
        Some(s) => {
            return Err(ScenarioError::new(format!(
                "faults.freshness_limit_seconds must be > 0 (got {s})"
            )))
        }
        None if has_delay => {
            return Err(ScenarioError::new(
                "faults.freshness_limit_seconds is required when a delay injection is configured (P2.08: the freshness limit is configuration, never a constant)",
            ))
        }
        None => 90,
    };
    let mut injections = Vec::with_capacity(raw.injections.len());
    for inj in raw.injections {
        let key = inj.asset_key.clone();
        if !asset_keys.contains_key(&key) {
            return Err(ScenarioError::new(format!(
                "faults: injection names unknown asset '{key}'"
            )));
        }
        let from_ms = parse_utc_instant(&inj.from, "faults.injections.from")?;
        let until_ms = parse_utc_instant(&inj.until, "faults.injections.until")?;
        if from_ms > until_ms {
            return Err(ScenarioError::new(format!(
                "faults: injection for '{key}' has an inverted interval"
            )));
        }
        if from_ms < start_ms || until_ms > horizon_end_ms {
            return Err(ScenarioError::new(format!(
                "faults: injection for '{key}' must lie within the scenario horizon"
            )));
        }
        let (delay_ticks, hold_ticks, factor) = match inj.fault {
            FaultKind::Drop
            | FaultKind::SourceInvalid
            | FaultKind::Duplicate
            | FaultKind::ConflictingDuplicate => (0, 0, 0.0),
            FaultKind::Delay => {
                let seconds = inj.delay_seconds.unwrap_or(0);
                if seconds == 0 {
                    return Err(ScenarioError::new(format!(
                        "faults: delay injection for '{key}' requires delay_seconds > 0"
                    )));
                }
                // Round up to whole cadence ticks: the scenario clock moves
                // in cadence steps.
                let ticks = seconds.div_ceil(cadence_seconds as u64);
                (ticks, 0, 0.0)
            }
            FaultKind::OutOfOrder => {
                let ticks = u64::from(inj.hold_ticks.unwrap_or(0));
                if ticks == 0 {
                    return Err(ScenarioError::new(format!(
                        "faults: out_of_order injection for '{key}' requires hold_ticks >= 1"
                    )));
                }
                (0, ticks, 0.0)
            }
            FaultKind::OutOfRange => {
                let factor = inj.factor.unwrap_or(2.0);
                if !(factor.is_finite() && factor > 1.0) {
                    return Err(ScenarioError::new(format!(
                        "faults: out_of_range injection for '{key}' requires factor > 1"
                    )));
                }
                (0, 0, factor)
            }
        };
        injections.push(FaultSpec {
            kind: inj.fault,
            asset_key: key,
            from_ms,
            until_ms,
            delay_ticks,
            hold_ticks,
            factor,
        });
    }
    Ok(FaultPlan {
        enabled: raw.enabled,
        freshness_limit_seconds,
        injections,
    })
}

/// Loads and validates a scenario file (JSON text). All-or-nothing: any
/// validation failure rejects the whole file.
pub fn load_scenario(json: &str) -> Result<Scenario, ScenarioError> {
    let file: ScenarioFile = serde_json::from_str(json).map_err(|e| {
        ScenarioError::new(format!("the scenario is not valid JSON for the schema: {e}"))
    })?;
    if !file.meta.synthetic {
        return Err(ScenarioError::new(
            "meta.synthetic must be true — every scenario is synthetic and must say so",
        ));
    }
    if file.meta.scenario_id.trim().is_empty() || file.meta.version.trim().is_empty() {
        return Err(ScenarioError::new(
            "meta.scenario_id and meta.version must be non-empty",
        ));
    }
    if file.portfolio.tenant.tenant_key.trim().is_empty() {
        return Err(ScenarioError::new(
            "portfolio.tenant.tenant_key must be non-empty",
        ));
    }
    let start_ms = parse_utc_instant(&file.horizon.start, "horizon.start")?;
    if !file.horizon.duration_hours.is_finite() || file.horizon.duration_hours <= 0.0 {
        return Err(ScenarioError::new("horizon.duration_hours must be finite and > 0"));
    }
    if file.cadence.default_seconds <= 0 {
        return Err(ScenarioError::new("cadence.default_seconds must be > 0"));
    }
    if !file.cadence.per_asset_overrides.is_empty() {
        // Validated as positive numbers, then rejected: the v1 scenario sets
        // none, and the engine does not silently approximate a multi-cadence
        // stream with the default cadence (recorded boundary).
        for (key, seconds) in &file.cadence.per_asset_overrides {
            if *seconds <= 0 {
                return Err(ScenarioError::new(format!(
                    "cadence.per_asset_overrides['{key}'] must be > 0"
                )));
            }
        }
        return Err(ScenarioError::new(
            "cadence.per_asset_overrides must be empty in v1: the engine runs the scenario default cadence for every asset, and a per-asset cadence is not silently approximated (recorded boundary)",
        ));
    }
    let horizon_end_ms = start_ms + (file.horizon.duration_hours * 3600.0 * 1000.0) as i64;
    let cadence_ms = file.cadence.default_seconds * 1000;
    let total_ticks_f = (horizon_end_ms - start_ms) as f64 / cadence_ms as f64;
    if (total_ticks_f - total_ticks_f.round()).abs() > 1e-6 {
        return Err(ScenarioError::new(
            "the horizon must span a whole number of cadence ticks",
        ));
    }
    let total_ticks = total_ticks_f.round() as u64;

    let mut asset_keys: BTreeMap<String, ()> = BTreeMap::new();
    let mut sites = Vec::with_capacity(file.portfolio.sites.len());
    for site in &file.portfolio.sites {
        if site.assets.is_empty() {
            return Err(ScenarioError::new(format!(
                "site '{}' has no assets",
                site.site_key
            )));
        }
        let mut assets = Vec::with_capacity(site.assets.len());
        for asset in &site.assets {
            if asset_keys.contains_key(&asset.asset_key) {
                return Err(ScenarioError::new(format!(
                    "duplicate asset_key '{}' (keys must be unique across the scenario)",
                    asset.asset_key
                )));
            }
            asset_keys.insert(asset.asset_key.clone(), ());
            assets.push(validate_asset(asset.clone())?);
        }
        sites.push(SiteSpec {
            site_key: site.site_key.clone(),
            reactive_convention: site.reactive_sign_convention,
            assets,
        });
    }
    if sites.is_empty() {
        return Err(ScenarioError::new("the scenario must carry at least one site"));
    }

    let faults = validate_fault_plan(
        file.faults,
        &asset_keys,
        file.cadence.default_seconds,
        start_ms,
        horizon_end_ms,
    )?;

    Ok(Scenario {
        meta: ValidatedMeta {
            scenario_id: file.meta.scenario_id,
            version: file.meta.version,
        },
        tenant_key: file.portfolio.tenant.tenant_key,
        sites,
        start_ms,
        duration_hours: file.horizon.duration_hours,
        cadence_seconds: file.cadence.default_seconds,
        master_seed: file.seeds.master_seed,
        faults,
        total_ticks,
    })
}

impl Scenario {
    /// The scenario event time of tick `tick` (ms since epoch on the
    /// scenario clock).
    pub(crate) fn tick_event_ms(&self, tick: u64) -> i64 {
        self.start_ms + (tick as i64) * self.cadence_seconds * 1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The versioned exit-gate scenario, loaded from the artefact itself so
    /// the crate and the file cannot drift apart silently.
    pub(super) const SCENARIO_V1: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/synthetic-portfolio/scenario-v1.json"
    ));

    fn load_v1() -> Scenario {
        load_scenario(SCENARIO_V1).expect("the versioned exit-gate scenario must load")
    }

    #[test]
    fn the_versioned_exit_gate_scenario_loads_and_validates() {
        let scenario = load_v1();
        assert_eq!(scenario.meta.scenario_id, "synthetic-portfolio-exit-gate");
        assert_eq!(scenario.meta.version, "v1");
        assert_eq!(scenario.tenant_key, "tenant-flexcore-demo");
        assert_eq!(scenario.sites.len(), 2);
        assert_eq!(scenario.cadence_seconds, 60);
        assert_eq!(scenario.total_ticks, 1440);
        assert_eq!(scenario.master_seed, 20260601);
        // Per-site declared conventions differ deliberately (P2.04 D-T4).
        assert_eq!(
            scenario.sites[0].reactive_convention,
            flexcore_contracts::units::ReactiveSignConvention::InjectionPositive
        );
        assert_eq!(
            scenario.sites[1].reactive_convention,
            flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive
        );
        // Five assets per site: BESS, PV, load, EVSE, meter.
        assert_eq!(scenario.sites[0].assets.len(), 5);
        assert_eq!(scenario.sites[1].assets.len(), 5);
        // The v1 fault plan is present (the P2.15 fill-in) but inert.
        assert!(!scenario.faults.enabled);
        assert_eq!(scenario.faults.injections.len(), 7);
    }

    #[test]
    fn a_non_synthetic_scenario_is_rejected() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["meta"]["synthetic"] = serde_json::Value::Bool(false);
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("synthetic"));
    }

    #[test]
    fn an_injected_fault_plan_fails_without_its_asset() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["faults"]["injections"][0]["asset_key"] =
            serde_json::Value::String("site-nowhere-1".to_string());
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("site-nowhere-1"));
    }

    #[test]
    fn a_delay_fault_requires_a_freshness_limit() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["faults"]["freshness_limit_seconds"] = serde_json::Value::Null;
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("freshness"));
    }

    #[test]
    fn per_asset_cadence_overrides_are_rejected_not_approximated() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["cadence"]["per_asset_overrides"]["site-north-bess-1"] =
            serde_json::Value::Number(serde_json::Number::from(30));
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("per_asset_overrides"));
    }

    #[test]
    fn a_non_utc_horizon_start_is_rejected_not_converted() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["horizon"]["start"] =
            serde_json::Value::String("2026-06-01T01:00:00+01:00".to_string());
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("UTC"));
    }

    #[test]
    fn malformed_json_is_rejected_whole() {
        assert!(load_scenario("{ not json").is_err());
    }

    #[test]
    fn an_unknown_behaviour_param_is_rejected_not_ignored() {
        let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V1).unwrap();
        value["portfolio"]["sites"][0]["assets"][0]["behaviour_profile"]["params"]
            ["unknown_param"] = serde_json::Value::Number(serde_json::Number::from(1));
        let err = load_scenario(&value.to_string()).unwrap_err();
        assert!(err.to_string().contains("invalid behaviour params"));
    }
}
