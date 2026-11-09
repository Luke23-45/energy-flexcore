// P2.24 — Replay execution: running a scenario version + seed through the
// deterministic simulator and rendering the canonical replay records.
//
// Rules this module encodes (P2.13 §3–§4; P2.24):
//
// - **Replay always names the scenario version + seed + code version.** The
//   run's output (`ScenarioRun`) carries the scenario identity and the
//   master seed straight from the validated scenario; the code version is a
//   CLI-level input (`main.rs`: `--code-version` or the
//   `FLEXCORE_CODE_VERSION` environment, recorded in the report). A replay
//   is only reproducible against the generator code that produced the
//   original run — a generation change breaks replay honestly (P2.13 §4).
// - **The comparison tuple excludes `ingest_time` by construction**
//   (P2.13 §3): [`ReplayRecord`] structurally CANNOT hold it. The record is
//   the canonical P2.04 tuple `(asset_id, event_time, quantity, value,
//   quality, source_sequence)` — exactly the comparison fields the ledger
//   P2.24 step 3 freezes. `ingest_time` is read from the decoded records
//   here only to be dropped.
// - **Stand-in identity, recorded:** the scenario speaks stable asset keys;
//   the mapping key → registry `AssetId` happens at registration through
//   the API (P2.25 step 2). The sim-runner does not register anything, so
//   it derives a DETERMINISTIC stand-in UUID per asset key
//   ([`stand_in_asset_id`]) — stable across runs (which is what the
//   comparison needs), clearly not a registry-issued ID. The stand-in
//   tenant scope is likewise a fixed, documented stand-in ([`REPLAY_TENANT_UUID`]);
//   tenant is not a comparison field.
// - **Deterministic identity derivation:** the stand-in UUID is the
//   concatenation of two FNV-1a-64 digests — one over the raw asset key,
//   one over a domain-separated string (`flexcore-sim-runner/stand-in/` +
//   key) — so the bytes carry no wall-clock, no ambient entropy, and no
//   dependence on iteration order. FNV-1a-64 is the standard public-domain
//   reference implementation (the same constants the simulator's recorded
//   derivation rule uses, `flexcore-simulator` `rng.rs`).
// - **The port is the only integration point:** records are produced by
//   pulling frames from [`SimulatorTelemetrySource`] and decoding them —
//   the same path the ingestion pipeline consumes. No engine internals are
//   reached around the port.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactivePowerMVAr, StateOfCharge};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuery,
    TelemetryQuantity, TelemetrySource,
};
use flexcore_simulator::{load_scenario, SimulatorTelemetrySource};
use serde::{Deserialize, Serialize};

// --- The canonical replay record ---------------------------------------------

/// The value of one replay record, in a JSON-stable tagged form. The
/// variant set mirrors the canonical [`QuantityValue`] kinds 1:1 (P2.04 §3:
/// the unit travels with the value; a bare number never appears). f64
/// fields round-trip exactly through `serde_json` (shortest-roundtrip
/// rendering), so byte-identical runs produce byte-identical files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayValue {
    /// Active power (or a baseline reference) in MW, P > 0 = export (P0.07 §2).
    Power { mw: f64 },
    /// Reactive power in MVAr TOGETHER with its declared sign convention
    /// (P0.07 §3; P2.04 D-T4) — the value is uninterpretable without it.
    Reactive { mvar: f64, convention: String },
    /// A fraction in [0,1] (SOC; P0.07 §4).
    Fraction { value: f64 },
    /// Interval energy in MWh.
    Energy { mwh: f64 },
    /// A boolean state (availability, connection state).
    Flag { value: bool },
    /// A source-declared text state (operating mode, health).
    State { value: String },
}

impl ReplayValue {
    /// Converts the canonical value into its replay form.
    pub fn from_quantity_value(value: &QuantityValue) -> Self {
        match value {
            QuantityValue::Power(p) => ReplayValue::Power { mw: p.value() },
            QuantityValue::Reactive { power, convention } => ReplayValue::Reactive {
                mvar: power.value(),
                convention: match convention {
                    flexcore_contracts::units::ReactiveSignConvention::InjectionPositive => {
                        "INJECTION_POSITIVE".to_string()
                    }
                    flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive => {
                        "CONSUMPTION_POSITIVE".to_string()
                    }
                },
            },
            QuantityValue::Fraction(f) => ReplayValue::Fraction { value: f.value() },
            QuantityValue::Energy(e) => ReplayValue::Energy { mwh: e.value() },
            QuantityValue::Flag(b) => ReplayValue::Flag { value: *b },
            QuantityValue::State(text) => ReplayValue::State {
                value: text.clone(),
            },
        }
    }

    /// Converts the replay form back into the canonical value.
    pub fn to_quantity_value(&self) -> Result<QuantityValue, ReplayError> {
        let unit_error = |e: flexcore_contracts::units::UnitsError| {
            ReplayError::Config(format!("the replay file carried a non-canonical value: {e}"))
        };
        Ok(match self {
            ReplayValue::Power { mw } => {
                QuantityValue::Power(ActivePowerMW::new(*mw).map_err(unit_error)?)
            }
            ReplayValue::Reactive { mvar, convention } => QuantityValue::Reactive {
                power: ReactivePowerMVAr::new(*mvar).map_err(unit_error)?,
                convention: match convention.as_str() {
                    "INJECTION_POSITIVE" => {
                        flexcore_contracts::units::ReactiveSignConvention::InjectionPositive
                    }
                    "CONSUMPTION_POSITIVE" => {
                        flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive
                    }
                    other => {
                        return Err(ReplayError::Config(format!(
                            "unknown reactive sign convention '{other}' in the reference file"
                        )))
                    }
                },
            },
            ReplayValue::Fraction { value } => {
                QuantityValue::Fraction(StateOfCharge::new(*value).map_err(unit_error)?)
            }
            ReplayValue::Energy { mwh } => {
                QuantityValue::Energy(EnergyMWh::new(*mwh).map_err(unit_error)?)
            }
            ReplayValue::Flag { value } => QuantityValue::Flag(*value),
            ReplayValue::State { value } => QuantityValue::State(value.clone()),
        })
    }
}

/// One canonical replay record: exactly the P2.24 comparison tuple
/// `(asset_id, event_time, quantity, value, quality, source_sequence)`.
/// There is no `ingest_time` field and there must never be one — a replay
/// comparison that touches `ingest_time` is wrong by definition (P2.13 §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// The asset's UUID (hyphenated canonical form). In a sim-runner run
    /// this is the deterministic stand-in identity ([`stand_in_asset_id`]).
    pub asset_id: String,
    /// When the measurement occurred, on the scenario clock (ms since epoch).
    pub event_time_unix_ms: i64,
    /// The canonical quantity id (`quantity_key`, P2.04 §4).
    pub quantity: String,
    /// The canonical value with its unit/convention.
    pub value: ReplayValue,
    /// Exactly one of the six canonical quality states (P0.08).
    pub quality: String,
    /// The source-assigned ordering identifier (monotonic per asset/source).
    pub source_sequence: u64,
}

/// The canonical quality-state spelling (SCREAMING_SNAKE_CASE, the frozen
/// P0.08 vocabulary and the serde form of `TelemetryQuality`).
pub fn quality_name(quality: TelemetryQuality) -> &'static str {
    match quality {
        TelemetryQuality::Good => "GOOD",
        TelemetryQuality::Suspect => "SUSPECT",
        TelemetryQuality::Bad => "BAD",
        TelemetryQuality::Missing => "MISSING",
        TelemetryQuality::Estimated => "ESTIMATED",
        TelemetryQuality::Stale => "STALE",
    }
}

/// Parses the canonical quality-state spelling; an unknown spelling is an
/// error, never a silent mapping (the six-state vocabulary is frozen).
pub fn parse_quality(name: &str) -> Result<TelemetryQuality, ReplayError> {
    match name {
        "GOOD" => Ok(TelemetryQuality::Good),
        "SUSPECT" => Ok(TelemetryQuality::Suspect),
        "BAD" => Ok(TelemetryQuality::Bad),
        "MISSING" => Ok(TelemetryQuality::Missing),
        "ESTIMATED" => Ok(TelemetryQuality::Estimated),
        "STALE" => Ok(TelemetryQuality::Stale),
        other => Err(ReplayError::Config(format!(
            "unknown canonical quality state '{other}' (the vocabulary is the frozen six, P0.08)"
        ))),
    }
}

/// Converts one decoded canonical record into a replay record. This is the
/// ONLY place `ingest_time` could enter the comparison — and it is dropped
/// here, on the record boundary, so nothing downstream can compare it.
pub fn record_from_normalized(record: &NormalizedTelemetry) -> ReplayRecord {
    ReplayRecord {
        asset_id: record.asset_id().uuid().to_hyphenated(),
        event_time_unix_ms: record.event_time().unix_millis() as i64,
        quantity: flexcore_ports::telemetry_read::quantity_key(record.quantity()).to_string(),
        value: ReplayValue::from_quantity_value(
            record
                .value()
                .expect("a decoded measured record always carries a value (P2.11 step 3)"),
        ),
        quality: quality_name(record.quality()).to_string(),
        source_sequence: record.source_sequence(),
    }
}

// --- Stand-in identity ---------------------------------------------------------

/// The fixed stand-in tenant scope of a sim-runner run. Replay comparison
/// never reads the tenant (it is not a comparison field); this only has to
/// be a valid UUID so the canonical record constructors hold. The real
/// tenant mapping happens at registration (P2.25 step 2).
pub const REPLAY_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7aa0";

/// FNV-1a-64 over `bytes` (the standard offset basis and prime — the same
/// public-domain reference implementation recorded in the simulator's
/// `rng.rs`; re-implemented here so the sim-runner does not reach into the
/// simulator's private modules).
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

/// The deterministic stand-in asset identity for a scenario asset key:
/// two FNV-1a-64 digests (the raw key, and the key under a
/// domain-separated prefix) concatenated into the 16 UUID bytes. No
/// wall-clock, no ambient entropy, no iteration-order dependence — the same
/// key always yields the same bytes, which is what makes runs and reference
/// files comparable. NOT a registry-issued ID (P2.25 step 2 does the real
/// mapping); recorded as a stand-in, never presented as registration.
pub fn stand_in_asset_id(asset_key: &str) -> AssetId {
    let h1 = fnv1a64(asset_key.as_bytes());
    let h2 = fnv1a64(format!("flexcore-sim-runner/stand-in/{asset_key}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&h1.to_be_bytes());
    bytes[8..].copy_from_slice(&h2.to_be_bytes());
    AssetId::new(FlexUuid::from_bytes(bytes))
}

fn replay_tenant_context() -> TenantContext {
    let tenant_id = TenantId::new(
        FlexUuid::parse(REPLAY_TENANT_UUID).expect("the replay tenant UUID is a valid constant"),
    );
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id,
        subject: "sim-runner:deterministic-replay".to_string(),
    }))
}

// --- Errors -------------------------------------------------------------------

/// Everything the sim-runner reports as a usage/config or execution error.
/// Every variant carries a human-safe message (never raw backend text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The scenario file was rejected (all-or-nothing validation, P2.13).
    Scenario(String),
    /// The telemetry source rejected a subscription or a frame.
    Source(String),
    /// The CLI invocation or a file's content is not usable.
    Config(String),
    /// Reading or writing a records/report file failed.
    Io(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Scenario(detail) => write!(f, "scenario rejected: {detail}"),
            ReplayError::Source(detail) => write!(f, "telemetry source failed: {detail}"),
            ReplayError::Config(detail) => write!(f, "usage/config error: {detail}"),
            ReplayError::Io(detail) => write!(f, "file error: {detail}"),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<flexcore_simulator::ScenarioError> for ReplayError {
    fn from(error: flexcore_simulator::ScenarioError) -> Self {
        ReplayError::Scenario(error.to_string())
    }
}

// --- Scenario manipulation ------------------------------------------------------

/// Returns the scenario JSON with the master seed replaced by `seed`. The
/// seed lives in the versioned scenario artefact (D-SC4); overriding it is
/// a scenario change for replay purposes — the tampered-seed replay MUST
/// fail with a diff (P2.24 acceptance), which is exactly what this produces.
pub fn scenario_json_with_seed(scenario_json: &str, seed: u64) -> Result<String, ReplayError> {
    let mut value: serde_json::Value = serde_json::from_str(scenario_json)
        .map_err(|e| ReplayError::Config(format!("the scenario is not valid JSON: {e}")))?;
    let seeds = value
        .get_mut("seeds")
        .and_then(|s| s.as_object_mut())
        .ok_or_else(|| ReplayError::Config("the scenario has no seeds section".to_string()))?;
    seeds.insert(
        "master_seed".to_string(),
        serde_json::Value::Number(seed.into()),
    );
    serde_json::to_string(&value)
        .map_err(|e| ReplayError::Config(format!("the scenario could not be re-rendered: {e}")))
}

/// Reads and validates the scenario file text (all-or-nothing).
pub fn read_scenario_text(path: &str) -> Result<String, ReplayError> {
    std::fs::read_to_string(path).map_err(|e| {
        ReplayError::Io(format!("could not read the scenario file '{path}': {e}"))
    })
}

// --- The run -------------------------------------------------------------------

/// One complete deterministic run: the scenario identity it names, the seed
/// that was in effect, and the canonical replay records it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioRun {
    /// The scenario id (`meta.scenario_id`, verbatim from the artefact).
    pub scenario_id: String,
    /// The scenario version (`meta.version`) — replay names it (P2.13 §4).
    pub scenario_version: String,
    /// The master seed in effect for this run (D-SC4).
    pub master_seed: u64,
    /// The decoded canonical records, in arrival order.
    pub records: Vec<ReplayRecord>,
}

/// Decodes the whole deterministic horizon into canonical records WITHOUT
/// the replay projection — used by [`run_scenario`] and by tests that need
/// the full `NormalizedTelemetry` (e.g. the ingest_time-exclusion proof,
/// which must REBUILD records with different ingest times to show the
/// comparison ignores them).
pub fn collect_normalized(scenario_json: &str) -> Result<Vec<NormalizedTelemetry>, ReplayError> {
    let scenario = load_scenario(scenario_json)?;
    let mut mapping: Vec<(String, AssetId)> = Vec::new();
    for site in &scenario.sites {
        for asset in &site.assets {
            mapping.push((asset.asset_key.clone(), stand_in_asset_id(&asset.asset_key)));
        }
    }
    let asset_ids: Vec<AssetId> = mapping.iter().map(|(_, id)| *id).collect();
    let source = SimulatorTelemetrySource::from_scenario(scenario, mapping).map_err(|e| {
        ReplayError::Scenario(format!("the scenario could not build a source: {e}"))
    })?;

    let ctx = replay_tenant_context();
    let all_quantities: Vec<TelemetryQuantity> = vec![
        TelemetryQuantity::ActivePower,
        TelemetryQuantity::ReactivePower,
        TelemetryQuantity::Soc,
        TelemetryQuantity::Energy,
        TelemetryQuantity::Availability,
        TelemetryQuantity::OperatingMode,
        TelemetryQuantity::Health,
        TelemetryQuantity::ConnectionState,
        TelemetryQuantity::BaselineReference,
    ];
    let query = TelemetryQuery::new(
        &ctx,
        asset_ids,
        EventTime::from_unix_millis(0).map_err(|e| ReplayError::Config(e.to_string()))?,
        EventTime::from_unix_millis(i64::MAX / 2)
            .map_err(|e| ReplayError::Config(e.to_string()))?,
        all_quantities,
    )
    .map_err(|e| ReplayError::Config(format!("the replay subscription query is invalid: {e}")))?;

    source.start();
    let subscription = source
        .subscribe(&query)
        .map_err(|e| ReplayError::Source(e.to_string()))?;
    let mut records = Vec::new();
    while let Some(frame) = source
        .pull_frame(&subscription)
        .map_err(|e| ReplayError::Source(e.to_string()))?
    {
        records.extend(
            source
                .decode(&subscription, &frame)
                .map_err(|e| ReplayError::Source(e.to_string()))?,
        );
    }
    Ok(records)
}

/// Runs the scenario (version + seed as configured in the artefact) through
/// the deterministic simulator and renders the canonical replay records.
/// Same scenario version + same seed + same code version ⇒ identical
/// `ScenarioRun` (P2.13 §3; `ingest_time` excluded by the record shape).
pub fn run_scenario(scenario_json: &str) -> Result<ScenarioRun, ReplayError> {
    let scenario = load_scenario(scenario_json)?;
    let run = ScenarioRun {
        scenario_id: scenario.meta.scenario_id.clone(),
        scenario_version: scenario.meta.version.clone(),
        master_seed: scenario.master_seed,
        records: collect_normalized(scenario_json)?
            .iter()
            .map(record_from_normalized)
            .collect(),
    };
    Ok(run)
}

// --- Reference files ------------------------------------------------------------

/// Serializes records to the JSON text the reference file carries (a pretty
/// array; stable field order for reviewable diffs).
pub fn records_to_json(records: &[ReplayRecord]) -> Result<String, ReplayError> {
    serde_json::to_string_pretty(records)
        .map_err(|e| ReplayError::Io(format!("the records could not be serialized: {e}")))
}

/// Parses a reference records file (the `run --out` form, or an export of
/// stored telemetry in the same canonical shape).
pub fn records_from_json(text: &str) -> Result<Vec<ReplayRecord>, ReplayError> {
    serde_json::from_str(text).map_err(|e| {
        ReplayError::Config(format!(
            "the reference file is not a canonical records array: {e}"
        ))
    })
}

/// Reads a reference records file.
pub fn read_records_file(path: &str) -> Result<Vec<ReplayRecord>, ReplayError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ReplayError::Io(format!("could not read the reference file '{path}': {e}")))?;
    records_from_json(&text)
}

/// Writes a file (records or report), creating it fresh.
pub fn write_file(path: &str, contents: &str) -> Result<(), ReplayError> {
    std::fs::write(path, contents)
        .map_err(|e| ReplayError::Io(format!("could not write '{path}': {e}")))
}

// --- A small self-check ----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(mw: f64) -> ReplayRecord {
        ReplayRecord {
            asset_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c".to_string(),
            event_time_unix_ms: 1_780_272_000_000,
            quantity: "active_power".to_string(),
            value: ReplayValue::Power { mw },
            quality: "GOOD".to_string(),
            source_sequence: 1,
        }
    }

    #[test]
    fn replay_records_round_trip_through_json_without_ingest_time() {
        let records = vec![sample_record(1.5), sample_record(-0.25)];
        let text = records_to_json(&records).unwrap();
        let parsed = records_from_json(&text).unwrap();
        assert_eq!(parsed, records);
        // The struct cannot carry ingest_time: the serialized form has no
        // such field. The comparison tuple is what this record freezes
        // (P2.13 §3); a reference file stays forward-compatible for
        // non-comparison additions because serde ignores unknown fields.
        assert!(!text.contains("ingest_time"));
    }

    #[test]
    fn the_stand_in_identity_is_deterministic_and_key_separated() {
        let a1 = stand_in_asset_id("site-north-bess-1");
        let a2 = stand_in_asset_id("site-north-bess-1");
        let b = stand_in_asset_id("site-south-bess-1");
        assert_eq!(a1, a2, "the same key must derive the same stand-in id");
        assert_ne!(a1, b, "different keys must derive different stand-in ids");
    }

    #[test]
    fn the_seed_override_only_touches_the_master_seed() {
        let json = r#"{"meta":{"scenario_id":"s","version":"v1","synthetic":true},
            "seeds":{"master_seed":1},"cadence":{"default_seconds":60}}"#;
        let overridden = scenario_json_with_seed(json, 42).unwrap();
        let value: serde_json::Value = serde_json::from_str(&overridden).unwrap();
        assert_eq!(value["seeds"]["master_seed"], 42);
        assert_eq!(value["meta"]["version"], "v1");
    }

    #[test]
    fn unknown_quality_spellings_are_rejected_not_mapped() {
        assert_eq!(parse_quality("GOOD").unwrap(), TelemetryQuality::Good);
        assert!(parse_quality("FINE").is_err());
    }

    #[test]
    fn quality_and_clock_names_round_trip() {
        for q in [
            TelemetryQuality::Good,
            TelemetryQuality::Suspect,
            TelemetryQuality::Bad,
            TelemetryQuality::Missing,
            TelemetryQuality::Estimated,
            TelemetryQuality::Stale,
        ] {
            assert_eq!(parse_quality(quality_name(q)).unwrap(), q);
        }
    }
}
