// P2.27 — TelemetrySource contract suite, driven against the in-memory
// simulator stand-in harness (report §19.2 test layer 3; ledger P2.27
// step 3; P2.11 step 6).
//
// THE ARRANGEMENT (recorded; the module header of
// `flexcore_ports::telemetry_source_contract` states the same decision):
// the ports crate cannot depend on flexcore-simulator — the dependency rule
// (tools/check_dependency_rule.py) allows flexcore-ports to depend only on
// flexcore-contracts and flexcore-domain, and flexcore-simulator already
// depends on flexcore-ports, so the reverse edge would be a cycle. The
// contract suite is therefore GENERIC over the `TelemetrySource` port, and
// this file is its CI instantiation for Phase 2: it drives the suite
// against an in-memory deterministic stand-in provider that mirrors the
// simulator's declared provider contract (P2.14/P2.15):
//
// - a `sim://`-namespaced source_id (never the reserved `flexcore:`
//   namespace, P2.04 D-T6);
// - the declared quality-state mapping with the original code travelling
//   as `source_quality_code` (P0.08 §3);
// - `ClockQuality::Synchronized` (the scenario clock is the simulation's
//   time reference — recorded, never assumed, P2.04 D-T5);
// - event times on the provided scenario clock at the declared cadence;
// - per-asset streams derived from the recorded seed rule (P2.13 D-SC4:
//   `seed(asset) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`);
// - `ingest_time` stamped from the same clock at the frame's release tick
//   (so `ingest_time >= event_time` by construction; excluded from the
//   replay comparison, P2.13 §3);
// - one frame per (asset, tick) via the pull path, decoded through the
//   port's `decode` — no engine internals, exactly what the port sees.
//
// The suite itself is unchanged by this stand-in — that is the point of
// making it generic. The FOLLOW-UP wiring (owned by the sim-runner /
// simulator owners, recorded in
// `ledger/phase-02-asset-telemetry/27_ci_gate_inventory.md`) is to run the
// SAME suite against `flexcore-simulator`'s `TelemetrySource` from a crate
// that legally depends on both (a `flexcore-sim-runner contract` subcommand
// or a dev-dependency test there). Future real adapters plug in the same
// way: instantiate `ContractSpec` against the adapter and run
// `run_contract_suite`.
//
// Everything here is synthetic and simulation-only (the entire 0.x line):
// no field read, no field write, no control authority, no safety or
// certification implication.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, StateOfCharge};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    QuantityValue, SourceStateMapping, TelemetryQuery, TelemetrySource, TelemetrySourceError,
    TelemetrySourceHealth, TelemetrySubscription, TelemetryQuantity,
};
use flexcore_ports::telemetry_source_contract::{
    run_contract_suite, run_subscription, ContractSpec, ContractViolation,
};

// --- Fixed identities (deterministic stand-ins, P2.24 §2.5 analogue) -------

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
/// Stand-in registry ID for the scenario asset key `site-north-bess-1`.
const ASSET_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
/// Stand-in registry ID for the scenario asset key `site-north-evse-1`.
const ASSET_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";

/// The scenario clock base of the exit-gate scenario: 2026-06-01T00:00:00Z.
const BASE_EVENT_MILLIS: i64 = 1_780_272_000_000;

/// The recorded master seed of the exit-gate scenario (P2.13).
const MASTER_SEED: u64 = 20_260_601;

fn tenant_id() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap())
}

fn asset_a() -> AssetId {
    AssetId::new(FlexUuid::parse(ASSET_A_UUID).unwrap())
}

fn asset_b() -> AssetId {
    AssetId::new(FlexUuid::parse(ASSET_B_UUID).unwrap())
}

fn tenant_context() -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant_id(),
        subject: "user:contract-harness".to_string(),
    }))
}

fn event(millis: i64) -> EventTime {
    EventTime::from_unix_millis(millis).unwrap()
}

// --- The deterministic frame factory (the seeded "scenario" side) ----------

/// FNV-1a-64 over the asset key — the first half of the recorded derivation
/// rule (P2.13 D-SC4).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// splitmix64 — the second half of the recorded derivation rule.
fn splitmix64(x: u64) -> u64 {
    let mixed = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = mixed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// xorshift64* — the recorded simulator RNG family (P2.14 step 2). Test
/// stand-in only; the engine's own RNG is not re-implemented here.
struct Xorshift64Star(u64);

impl Xorshift64Star {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_f64_unit(&mut self) -> f64 {
        f64::from((self.next_u64() >> 11) as u32) / f64::from(1u32 << 24)
    }
}

/// The per-asset frame shape: which quantities the asset's frame carries,
/// and the value kind of each line.
struct AssetFrameSpec {
    key: &'static str,
    /// `(quantity token, value kind)` per frame line.
    lines: &'static [(&'static str, &'static str)],
}

const EXIT_GATE_ASSETS: [AssetFrameSpec; 2] = [
    AssetFrameSpec {
        key: "site-north-bess-1",
        lines: &[("ACTIVE_POWER", "MW"), ("SOC", "FRAC")],
    },
    AssetFrameSpec {
        key: "site-north-evse-1",
        lines: &[("ACTIVE_POWER", "MW"), ("AVAILABILITY", "FLAG")],
    },
];

const ALT_PROVIDER_ASSETS: [AssetFrameSpec; 1] = [AssetFrameSpec {
    key: "site-north-bess-1",
    lines: &[("ACTIVE_POWER", "MW"), ("OPERATING_MODE", "STATE")],
}];

/// Harness configuration: the declared provider contract plus the deliberate
/// deviation drivers the negative tests flip (each one breaks exactly one
/// contract rule so the suite's named violation can be asserted).
#[derive(Clone)]
struct HarnessConfig {
    provider_id: &'static str,
    cadence_seconds: u64,
    clock_quality: ClockQuality,
    /// The declared quality mapping (verbatim code -> canonical state).
    quality_codes: &'static [(&'static str, TelemetryQuality)],
    assets: &'static [AssetFrameSpec],
    /// Deviation: the factory skews this tick's frame event_time by this
    /// many milliseconds (the provider leaves the provided clock).
    skew_tick: Option<(usize, i64)>,
    /// Deviation: the provider attaches an UNDECLARED source quality code
    /// to every record of this tick (P0.08 §3 broken).
    undeclared_code_tick: Option<usize>,
    /// Deviation: the factory emits an empty payload on this tick (an
    /// `Ok` with an empty record set — never allowed, P2.11 step 3).
    empty_frame_tick: Option<usize>,
    /// Deviation: the factory emits a frame naming an unknown quantity on
    /// this tick (a hard decode failure).
    corrupt_token_tick: Option<usize>,
}

fn exit_gate_codes() -> &'static [(&'static str, TelemetryQuality)] {
    &[
        ("GOOD", TelemetryQuality::Good),
        ("OUT_OF_RANGE", TelemetryQuality::Suspect),
        ("SOURCE_INVALID", TelemetryQuality::Bad),
    ]
}

/// The exit-gate stand-in: the simulator's declared contract shape.
fn exit_gate_config() -> HarnessConfig {
    HarnessConfig {
        provider_id: "sim://contract-harness",
        cadence_seconds: 60,
        clock_quality: ClockQuality::Synchronized,
        quality_codes: exit_gate_codes(),
        assets: &EXIT_GATE_ASSETS,
        skew_tick: None,
        undeclared_code_tick: None,
        empty_frame_tick: None,
        corrupt_token_tick: None,
    }
}

/// A differently declared provider (for the genericity test): different
/// source ID, cadence, clock behaviour, mapping, and quantity set.
fn alt_provider_config() -> HarnessConfig {
    HarnessConfig {
        provider_id: "sim://alt-provider",
        cadence_seconds: 300,
        clock_quality: ClockQuality::Unknown,
        quality_codes: &[("GOOD", TelemetryQuality::Good)],
        assets: &ALT_PROVIDER_ASSETS,
        skew_tick: None,
        undeclared_code_tick: None,
        empty_frame_tick: None,
        corrupt_token_tick: None,
    }
}

/// Builds the deterministic frame list and the provided clock's expected
/// event times: one frame per (tick, asset), each frame carrying the asset's
/// declared quantity lines. Frame bodies are a pure function of
/// (master seed, asset key, tick) — the same property replay determinism
/// needs (P2.13).
fn harness_frames(
    config: &HarnessConfig,
    ticks: usize,
) -> (Vec<Vec<u8>>, Vec<EventTime>) {
    let mut frames: Vec<Vec<u8>> = Vec::new();
    let mut expected: Vec<EventTime> = Vec::new();
    for tick in 0..ticks {
        let clock_millis =
            BASE_EVENT_MILLIS + tick as i64 * (config.cadence_seconds as i64 * 1_000);
        // The provided clock's time is what the suite holds the provider to;
        // the skew deviation moves the FRAME away from it, never the clock.
        // The clock list has one entry PER FRAME (the suite indexes it by
        // frame position), so every frame of one tick repeats the tick's
        // clock time.
        let frame_event_millis = clock_millis
            + config.skew_tick.filter(|(skew_tick, _)| *skew_tick == tick).map_or(0, |(_, ms)| ms);
        for asset in config.assets {
            expected.push(event(clock_millis));
            let asset_seed = MASTER_SEED ^ splitmix64(fnv1a64(asset.key.as_bytes()));
            let mut rng = Xorshift64Star::new(
                asset_seed ^ (tick as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );
            let mut lines: Vec<String> = Vec::new();
            for (quantity_token, value_kind) in asset.lines {
                let value_token = match *value_kind {
                    "MW" => format!("MW:{}", (rng.next_f64_unit() * 2.0 - 1.0) * 1.5),
                    "FRAC" => format!("FRAC:{}", 0.05 + 0.9 * rng.next_f64_unit()),
                    "FLAG" => "FLAG:true".to_string(),
                    "STATE" => "STATE:IDLE".to_string(),
                    other => panic!("frame factory misuse: unknown value kind {other}"),
                };
                let quantity_token = if config.corrupt_token_tick == Some(tick)
                    && lines.is_empty()
                {
                    "NOT_A_QUANTITY"
                } else {
                    quantity_token
                };
                lines.push(format!(
                    "SIMH|1|{tick}|{frame_event_millis}|{}|{quantity_token}|{value_token}|GOOD|{}",
                    asset.key,
                    tick as u64 + 1
                ));
            }
            if config.empty_frame_tick == Some(tick) {
                // An empty payload: the provider decodes it to an empty
                // record set — the port forbids a successful empty decode.
                frames.push(Vec::new());
            } else {
                frames.push(lines.join("\n").into_bytes());
            }
        }
    }
    (frames, expected)
}

// --- The in-memory stand-in provider (the port's view of the simulator) ----

struct SubscriptionInfo {
    tenant_id: TenantId,
    assets: HashSet<AssetId>,
    quantities: HashSet<TelemetryQuantity>,
}

/// The in-memory `TelemetrySource` stand-in. It is a faithful mirror of the
/// simulator's provider CONTRACT (see the file header), not of its engine:
/// `decode` parses the opaque frame bytes into complete canonical records —
/// exactly what the port requires — with no engine internals.
struct HarnessProvider {
    config: HarnessConfig,
    declaration: ProviderDeclaration,
    asset_ids: HashMap<&'static str, AssetId>,
    subscriptions: Mutex<HashMap<u64, SubscriptionInfo>>,
    next_handle: AtomicU64,
    /// Instance-level active-power offset: zero for a conforming instance;
    /// non-zero only in the reproduction-failure test (a provider whose
    /// records depend on instance state instead of its inputs — the
    /// generation-change analogue of the P2.24 tampered-seed case).
    mw_offset: f64,
}

impl HarnessProvider {
    fn new(config: HarnessConfig) -> Self {
        Self::with_mw_offset(config, 0.0)
    }

    fn with_mw_offset(config: HarnessConfig, mw_offset: f64) -> Self {
        let declaration = ProviderDeclaration::new(
            config.provider_id.to_string(),
            Duration::from_secs(config.cadence_seconds),
            QualityMappingDeclaration::new(
                config
                    .quality_codes
                    .iter()
                    .map(|(code, canonical)| SourceStateMapping {
                        source_code: (*code).to_string(),
                        canonical: *canonical,
                    })
                    .collect(),
            )
            .unwrap(),
            config.clock_quality,
        )
        .unwrap();
        let asset_ids = config
            .assets
            .iter()
            .map(|asset| (asset.key, asset_id_for_key(asset.key)))
            .collect();
        Self {
            config,
            declaration,
            asset_ids,
            subscriptions: Mutex::new(HashMap::new()),
            next_handle: AtomicU64::new(0),
            mw_offset,
        }
    }

    fn decode_line(
        &self,
        line_index: usize,
        fields: [&str; 9],
        subscription: &SubscriptionInfo,
    ) -> Result<NormalizedTelemetry, TelemetrySourceError> {
        let fail = |reason: String| TelemetrySourceError::DecodeFailed { reason };

        let tick: usize = fields[2]
            .parse()
            .map_err(|_| fail("the frame tick field is not an index".to_string()))?;
        let event_millis: i64 = fields[3].parse().map_err(|_| {
            fail("the frame event-time field is not a millisecond value".to_string())
        })?;
        let asset_id = self
            .asset_ids
            .get(fields[4])
            .copied()
            .ok_or_else(|| {
                fail("the frame names an asset this source does not serve".to_string())
            })?;
        if !subscription.assets.contains(&asset_id) {
            return Err(fail(
                "the frame names an asset outside this subscription".to_string(),
            ));
        }
        let sequence: u64 = fields[8]
            .parse()
            .map_err(|_| fail("the frame sequence field is not a number".to_string()))?;


        // Quantity + canonical value (the P2.04 §4 pairing is decided by the
        // quantity token; the unit travels with the value kind).
        let parse_number = |token: &str, prefix: &str| -> Result<f64, TelemetrySourceError> {
            let Some(raw) = token.strip_prefix(prefix) else {
                return Err(fail(
                    "the frame value carries the wrong unit prefix for its quantity".to_string(),
                ));
            };
            raw.parse::<f64>()
                .map_err(|_| fail("the frame value is not a finite number".to_string()))
        };
        let (quantity, value) = match (fields[5], fields[6]) {
            ("ACTIVE_POWER", number) => {
                let mw = parse_number(number, "MW:")? + self.mw_offset;
                let power = ActivePowerMW::new(mw).map_err(|_| {
                    fail("the frame value is outside the canonical MW range".to_string())
                })?;
                (TelemetryQuantity::ActivePower, QuantityValue::Power(power))
            }
            ("SOC", number) => {
                let fraction = StateOfCharge::new(parse_number(number, "FRAC:")?)
                    .map_err(|_| {
                        fail("the frame value is outside the canonical [0,1] range".to_string())
                    })?;
                (TelemetryQuantity::Soc, QuantityValue::Fraction(fraction))
            }
            ("AVAILABILITY", "FLAG:true") => {
                (TelemetryQuantity::Availability, QuantityValue::Flag(true))
            }
            ("AVAILABILITY", "FLAG:false") => {
                (TelemetryQuantity::Availability, QuantityValue::Flag(false))
            }
            ("OPERATING_MODE", state) => {
                let Some(text) = state.strip_prefix("STATE:") else {
                    return Err(fail(
                        "the operating-mode value is not a declared text state".to_string(),
                    ));
                };
                (TelemetryQuantity::OperatingMode, QuantityValue::State(text.to_string()))
            }
            _ => {
                return Err(fail(
                    "the frame names an unknown quantity or value kind".to_string(),
                ))
            }
        };
        if !subscription.quantities.contains(&quantity) {
            return Err(fail(
                "the frame carries a quantity outside this subscription".to_string(),
            ));
        }

        // The declared quality mapping: an undeclared code fails decoding
        // (the provider never invents a canonical state outside its
        // declaration, P0.08 §3).
        let quality = self
            .config
            .quality_codes
            .iter()
            .find(|(code, _)| *code == fields[7])
            .map(|(_, canonical)| *canonical)
            .ok_or_else(|| {
                fail("the frame carries a quality code the provider has not declared".to_string())
            })?;

        // Deviation driver: an undeclared diagnostic code attached on one
        // tick (the closure check must catch it).
        let source_quality_code = if self.config.undeclared_code_tick == Some(tick) {
            Some("UNDECLARED_CODE".to_string())
        } else {
            None
        };

        // ingest_time is stamped from the same clock at the frame's release
        // tick (the simulator's recorded stamping rule) — never a wall clock.
        let ingest_time = IngestTime::from_unix_millis(event_millis)
            .map_err(|_| fail("the frame event time is not a valid ingest time".to_string()))?;

        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: subscription.tenant_id,
            asset_id,
            quantity,
            value: Some(value),
            event_time: event(event_millis),
            ingest_time,
            source_sequence: sequence,
            source_id: self.config.provider_id.to_string(),
            quality,
            source_quality_code,
            clock_quality: self.config.clock_quality,
        })
        .map_err(|error| {
            fail(format!("the frame did not decode into a canonical record: {error}"))
        })
    }
}

fn asset_id_for_key(key: &str) -> AssetId {
    match key {
        "site-north-bess-1" => asset_a(),
        "site-north-evse-1" => asset_b(),
        other => panic!("unknown harness asset key: {other}"),
    }
}

impl TelemetrySource for HarnessProvider {
    fn provider_declaration(&self) -> &ProviderDeclaration {
        &self.declaration
    }

    fn subscribe(
        &self,
        query: &TelemetryQuery,
    ) -> Result<TelemetrySubscription, TelemetrySourceError> {
        for asset in query.assets() {
            if !self.asset_ids.values().any(|served| served == asset) {
                return Err(TelemetrySourceError::InvalidQuery {
                    reason: "the source does not serve every asset in the query".to_string(),
                });
            }
        }
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst) + 1;
        self.subscriptions
            .lock()
            .unwrap()
            .insert(
                handle,
                SubscriptionInfo {
                    tenant_id: query.tenant_id(),
                    assets: query.assets().iter().copied().collect(),
                    quantities: query.quantities().iter().copied().collect(),
                },
            );
        Ok(TelemetrySubscription::from_handle(handle))
    }

    fn decode(
        &self,
        subscription: &TelemetrySubscription,
        payload: &[u8],
    ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
        let subscriptions = self.subscriptions.lock().unwrap();
        let Some(info) = subscriptions.get(&subscription.handle()) else {
            return Err(TelemetrySourceError::NotSubscribed);
        };
        let text = std::str::from_utf8(payload).map_err(|_| TelemetrySourceError::DecodeFailed {
            reason: "the frame payload is not a valid text frame".to_string(),
        })?;
        let mut records: Vec<NormalizedTelemetry> = Vec::new();
        for (line_index, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split('|').collect();
            let fields: [&str; 9] = fields.try_into().map_err(|_| {
                TelemetrySourceError::DecodeFailed {
                    reason: format!("frame line {line_index} does not have 9 fields"),
                }
            })?;
            if fields[0] != "SIMH" {
                return Err(TelemetrySourceError::DecodeFailed {
                    reason: format!("frame line {line_index} has an unknown frame marker"),
                });
            }
            records.push(self.decode_line(line_index, fields, info)?);
        }
        Ok(records)
    }

    fn health(&self) -> TelemetrySourceHealth {
        TelemetrySourceHealth::Healthy
    }
}

// --- Suite wiring helpers ---------------------------------------------------

fn exit_gate_query() -> TelemetryQuery {
    TelemetryQuery::new(
        &tenant_context(),
        vec![asset_a(), asset_b()],
        event(BASE_EVENT_MILLIS),
        event(BASE_EVENT_MILLIS + 7 * 60_000),
        vec![
            TelemetryQuantity::ActivePower,
            TelemetryQuantity::Soc,
            TelemetryQuantity::Availability,
        ],
    )
    .unwrap()
}

fn alt_query() -> TelemetryQuery {
    TelemetryQuery::new(
        &tenant_context(),
        vec![asset_a()],
        event(BASE_EVENT_MILLIS),
        event(BASE_EVENT_MILLIS + 3 * 300_000),
        vec![TelemetryQuantity::ActivePower, TelemetryQuantity::OperatingMode],
    )
    .unwrap()
}

/// Builds a spec over locally owned frames/clock (the spec borrows them).
fn suite_spec<'a>(
    make_source: &'a dyn Fn() -> Box<dyn TelemetrySource>,
    query: &TelemetryQuery,
    frames: &'a [Vec<u8>],
    clock: &'a [EventTime],
) -> ContractSpec<'a> {
    ContractSpec {
        make_source,
        query: query.clone(),
        frames,
        expected_event_times: clock,
    }
}

/// The frozen replay comparison tuple, assembled here (outside the suite)
/// for the explicit determinism assertion. Same field set as the suite's
/// internal tuple — no `ingest_time` (P2.24 §2.2).
fn comparison_tuple(
    record: &NormalizedTelemetry,
) -> (AssetId, EventTime, TelemetryQuantity, Option<QuantityValue>, TelemetryQuality, u64) {
    (
        record.asset_id(),
        record.event_time(),
        record.quantity(),
        record.value().cloned(),
        record.quality(),
        record.source_sequence(),
    )
}

// --- Tests ------------------------------------------------------------------

const EXIT_GATE_TICKS: usize = 8;

#[test]
fn the_suite_passes_the_simulator_stand_in_harness_with_zero_violations() {
    let config = exit_gate_config();
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(
        outcome.passed,
        "the conforming stand-in must pass the whole suite; violations: {:?}",
        outcome.violations
    );

    // The suite actually DROVE the provider: 8 ticks x 2 assets x 2
    // quantities, all decoded through the port's decode.
    let source = (spec.make_source)();
    let run = run_subscription(source.as_ref(), &spec).unwrap();
    let record_count: usize = run.per_tick.iter().map(Vec::len).sum();
    assert_eq!(record_count, 8 * 2 * 2);
    assert_eq!(run.source_id, "sim://contract-harness");
}

#[test]
fn two_independent_runs_reproduce_the_frozen_comparison_tuple_element_wise() {
    let config = exit_gate_config();
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let run_a = run_subscription((spec.make_source)().as_ref(), &spec).unwrap();
    let run_b = run_subscription((spec.make_source)().as_ref(), &spec).unwrap();
    let tuples_a: Vec<_> = run_a
        .per_tick
        .iter()
        .flatten()
        .map(comparison_tuple)
        .collect();
    let tuples_b: Vec<_> = run_b
        .per_tick
        .iter()
        .flatten()
        .map(comparison_tuple)
        .collect();
    assert_eq!(tuples_a.len(), 32);
    assert_eq!(tuples_a, tuples_b, "same seed + same frames must reproduce exactly");
}

#[test]
fn an_instance_state_drift_breaking_reproduction_is_flagged_as_such() {
    // The third provider instance (the suite's second reproduction run)
    // carries an internal active-power offset: its records depend on
    // instance state, not on its inputs — the generation-change analogue
    // of the P2.24 tampered-seed case. The suite must fail with the named
    // violation and BOTH diverging tuples rendered.
    let config = exit_gate_config();
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let make = || -> Box<dyn TelemetrySource> {
        let call = calls.fetch_add(1, Ordering::SeqCst);
        if call == 2 {
            Box::new(HarnessProvider::with_mw_offset(config.clone(), 0.25))
        } else {
            Box::new(HarnessProvider::new(config.clone()))
        }
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    let Some(ContractViolation::SameSeedReproduction(detail)) = outcome
        .violations
        .iter()
        .find(|v| matches!(v, ContractViolation::SameSeedReproduction(_)))
    else {
        panic!("expected a SameSeedReproduction violation, got: {outcome:?}");
    };
    assert!(
        detail.contains("value"),
        "the rendered divergence must name the differing tuple side: {detail}"
    );
}

#[test]
fn a_frame_off_the_provided_clock_is_flagged_as_an_event_time_violation() {
    let mut config = exit_gate_config();
    config.skew_tick = Some((3, 500));
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::EventTimeOnClock(_))),
        "expected an EventTimeOnClock violation, got: {outcome:?}"
    );
    // The clock itself was never moved: the expected times stay the
    // provided scenario clock (the violation is the provider's, not the
    // clock's).
    assert_eq!(clock[3], event(BASE_EVENT_MILLIS + 3 * 60_000));
}

#[test]
fn an_undeclared_source_quality_code_is_flagged_by_the_closure_check() {
    let mut config = exit_gate_config();
    config.undeclared_code_tick = Some(5);
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::QualityClosure(_))),
        "expected a QualityClosure violation, got: {outcome:?}"
    );
}

#[test]
fn a_corrupt_frame_is_a_decode_violation_never_a_silent_skip() {
    let mut config = exit_gate_config();
    config.corrupt_token_tick = Some(2);
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::Decode(_))),
        "expected a Decode violation for the corrupt frame, got: {outcome:?}"
    );
}

#[test]
fn an_empty_record_set_is_a_decode_violation() {
    // A successful decode that carries no records is forbidden by the port
    // (P2.11 step 3: success is the complete non-empty set or an error) —
    // the suite must catch the provider doing it anyway.
    let mut config = exit_gate_config();
    config.empty_frame_tick = Some(4);
    let (frames, clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    let decode_violations: Vec<&String> = outcome
        .violations
        .iter()
        .filter_map(|v| match v {
            ContractViolation::Decode(detail) => Some(detail),
            _ => None,
        })
        .collect();
    assert!(
        decode_violations
            .iter()
            .any(|detail| detail.contains("empty record set")),
        "expected the empty-record-set rule to fire, got: {decode_violations:?}"
    );
}

#[test]
fn a_differently_declared_provider_also_passes_the_same_suite() {
    // Genericity: a second provider with a different source ID, cadence,
    // clock behaviour, mapping, and quantity set plugs into the SAME suite
    // unchanged — this is how future real adapters (later phases) run it.
    let config = alt_provider_config();
    let (frames, clock) = harness_frames(&config, 4);
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = alt_query();
    let spec = suite_spec(&make, &query, &frames, &clock);
    let outcome = run_contract_suite(&spec);
    assert!(
        outcome.passed,
        "the differently declared provider must pass the same suite; violations: {:?}",
        outcome.violations
    );
    assert_eq!((spec.make_source)().source_id(), "sim://alt-provider");
}

#[test]
fn the_suite_rejects_a_spec_whose_clock_list_does_not_match_the_frames() {
    let config = exit_gate_config();
    let (frames, _clock) = harness_frames(&config, EXIT_GATE_TICKS);
    let short_clock = vec![event(BASE_EVENT_MILLIS)];
    let make = || -> Box<dyn TelemetrySource> {
        Box::new(HarnessProvider::new(config.clone()))
    };
    let query = exit_gate_query();
    let spec = suite_spec(&make, &query, &frames, &short_clock);
    let outcome = run_contract_suite(&spec);
    assert!(!outcome.passed);
    assert!(
        outcome
            .violations
            .iter()
            .any(|v| matches!(v, ContractViolation::Spec(_))),
        "a malformed spec must be rejected, not silently checked, got: {outcome:?}"
    );
}
