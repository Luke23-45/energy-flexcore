// P2.10/P2.24 — worker simulator-source composition wiring
//
// Tests the composition pieces that are pure (no database, no broker):
//
// 1. The simulator configuration (P2.10/P2.14/P2.25): scenario path,
//    tenant reference, seed override, horizon override — present parses,
//    absent stays absent (the worker then runs WITHOUT the ingestion task,
//    the recorded honest behaviour), and invalid values fail fast with the
//    variable named.
// 2. The `TelemetryQuery` construction the composition relies on: empty
//    scopes and an inverted range are rejected (P2.11 D-S2; the query is
//    built only through `TenantContext`, P0.16).
// 3. A composition smoke test: the source the composition builds from the
//    v2 exit-gate scenario implements the `TelemetrySource` port and
//    yields deterministic, canonical records for the full 24 h horizon.
//
// Scope note: `compose_ingestion` itself lives in the binary
// (`apps/worker/src/main.rs`, the composition root — the port-only rule
// keeps the simulator crate out of the library) and resolves the tenant
// and assets through the store, so its DB-gated behaviour is exercised by
// the P2.25/P2.26 runbook, not here. The pieces it is built from are
// tested below against the same v2 scenario.
//
// Everything here is synthetic, simulation-only telemetry (the entire 0.x
// line): no field read, no field write, no control authority, no safety
// or certification implication.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, QuantityValue, TelemetryQuery, TelemetryQueryError, TelemetryQuantity,
    TelemetrySource, TelemetrySourceHealth,
};
use flexcore_simulator::{load_scenario, SimulatorTelemetrySource};
use flexcore_worker::config::{
    ConfigError, WorkerConfig, ENV_DATABASE_URL, ENV_NATS_URL, ENV_SIMULATOR_HORIZON_TICKS,
    ENV_SIMULATOR_SCENARIO, ENV_SIMULATOR_SEED, ENV_SIMULATOR_TENANT_REFERENCE,
};

/// The exit-gate scenario the runbook drives (P2.25 §0): v2, master seed
/// 20260601, 24 h from 2026-06-01T00:00:00Z, 1-minute cadence, faults
/// disabled.
const SCENARIO_V2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v2.json"
));

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

/// The ten stable asset keys of the exit-gate scenario, in scenario order,
/// with fixed test UUIDs (the real key → ID mapping happens at
/// registration, P2.25 step 2).
const ASSET_KEYS: [&str; 10] = [
    "site-north-bess-1",
    "site-north-pv-1",
    "site-north-load-1",
    "site-north-evse-1",
    "site-north-meter-1",
    "site-south-bess-1",
    "site-south-pv-1",
    "site-south-load-1",
    "site-south-evse-1",
    "site-south-meter-1",
];

fn asset_id(key: &str) -> AssetId {
    let index = ASSET_KEYS.iter().position(|k| *k == key).unwrap();
    let uuid = format!("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a{:02x}", 0x8c + index);
    AssetId::new(FlexUuid::parse(&uuid).unwrap())
}

fn asset_mapping() -> Vec<(String, AssetId)> {
    ASSET_KEYS
        .iter()
        .map(|key| (key.to_string(), asset_id(key)))
        .collect()
}

fn tenant_context() -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap()),
        subject: "user:worker-wiring-test".to_string(),
    }))
}

fn all_quantities() -> Vec<TelemetryQuantity> {
    vec![
        TelemetryQuantity::ActivePower,
        TelemetryQuantity::ReactivePower,
        TelemetryQuantity::Soc,
        TelemetryQuantity::Energy,
        TelemetryQuantity::Availability,
        TelemetryQuantity::OperatingMode,
        TelemetryQuantity::Health,
        TelemetryQuantity::ConnectionState,
        TelemetryQuantity::BaselineReference,
    ]
}

// --- Env-var serialisation -------------------------------------------------

/// Env-var tests are inherently serial (the config parser reads the process
/// environment); this lock keeps the parsing tests of this file from racing
/// each other. Edition-2024 note: `set_var`/`remove_var` are `unsafe`.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn set_env(name: &'static str, value: &str) {
    // SAFETY: serialized by ENV_LOCK; no other thread reads the environment
    // concurrently inside this test binary.
    unsafe { std::env::set_var(name, value) };
}

fn remove_env(name: &'static str) {
    // SAFETY: as above.
    unsafe { std::env::remove_var(name) };
}

/// Clears every variable these tests manipulate (call under the lock).
fn clear_simulator_env() {
    for name in [
        ENV_DATABASE_URL,
        ENV_NATS_URL,
        ENV_SIMULATOR_SCENARIO,
        ENV_SIMULATOR_TENANT_REFERENCE,
        ENV_SIMULATOR_SEED,
        ENV_SIMULATOR_HORIZON_TICKS,
    ] {
        remove_env(name);
    }
}

// --- 1. Configuration parsing -------------------------------------------------

#[test]
fn simulator_config_absent_means_no_ingestion_configuration() {
    let _env = lock_env();
    clear_simulator_env();
    set_env(ENV_DATABASE_URL, "postgres://flexcore:secret@localhost/flexcore");
    set_env(ENV_NATS_URL, "nats://localhost:4222");
    let config = WorkerConfig::from_env().unwrap();
    // The honest default: no scenario configured → the composition returns
    // its None-equivalent and the worker starts WITHOUT the ingestion task
    // (never a silent start).
    assert!(config.simulator_scenario.is_none());
    assert!(config.simulator_tenant_reference.is_none());
    assert!(config.simulator_seed.is_none());
    assert!(config.simulator_horizon_ticks.is_none());
    // The debug rendering still redacts the URLs alongside the new fields.
    let rendered = format!("{config:?}");
    assert!(!rendered.contains("secret"), "secrets must never render: {rendered}");
}

#[test]
fn simulator_config_present_parses_the_full_composition_input() {
    let _env = lock_env();
    clear_simulator_env();
    set_env(ENV_DATABASE_URL, "postgres://flexcore:secret@localhost/flexcore");
    set_env(ENV_NATS_URL, "nats://localhost:4222");
    set_env(ENV_SIMULATOR_SCENARIO, "scenarios/synthetic-portfolio/scenario-v2.json");
    set_env(ENV_SIMULATOR_TENANT_REFERENCE, "idp:flexcore-demo");
    // Defaults: the seed and horizon default to the scenario's recorded
    // values (parsed as None here; the composition applies them).
    let config = WorkerConfig::from_env().unwrap();
    assert_eq!(
        config.simulator_scenario.as_deref(),
        Some("scenarios/synthetic-portfolio/scenario-v2.json")
    );
    assert_eq!(config.simulator_tenant_reference.as_deref(), Some("idp:flexcore-demo"));
    assert!(config.simulator_seed.is_none());
    assert!(config.simulator_horizon_ticks.is_none());

    // Overrides parse.
    set_env(ENV_SIMULATOR_SEED, "20260601");
    set_env(ENV_SIMULATOR_HORIZON_TICKS, "1440");
    let config = WorkerConfig::from_env().unwrap();
    assert_eq!(config.simulator_seed, Some(20_260_601));
    assert_eq!(config.simulator_horizon_ticks, Some(1_440));
}

#[test]
fn simulator_config_rejects_unusable_values_with_the_variable_named() {
    let _env = lock_env();
    clear_simulator_env();
    set_env(ENV_DATABASE_URL, "postgres://flexcore:secret@localhost/flexcore");
    set_env(ENV_NATS_URL, "nats://localhost:4222");
    set_env(ENV_SIMULATOR_SCENARIO, "scenarios/synthetic-portfolio/scenario-v2.json");

    // A non-numeric seed fails fast.
    set_env(ENV_SIMULATOR_SEED, "not-a-seed");
    let error = WorkerConfig::from_env().unwrap_err();
    assert!(matches!(error, ConfigError::InvalidEnv { name: ENV_SIMULATOR_SEED, .. }));
    assert!(error.to_string().contains("FLEXCORE_SIMULATOR_SEED"));

    // A zero or out-of-range horizon fails fast.
    remove_env(ENV_SIMULATOR_SEED);
    set_env(ENV_SIMULATOR_HORIZON_TICKS, "0");
    let error = WorkerConfig::from_env().unwrap_err();
    assert!(matches!(error, ConfigError::InvalidEnv { name: ENV_SIMULATOR_HORIZON_TICKS, .. }));
    set_env(ENV_SIMULATOR_HORIZON_TICKS, "10000001");
    let error = WorkerConfig::from_env().unwrap_err();
    assert!(matches!(error, ConfigError::InvalidEnv { name: ENV_SIMULATOR_HORIZON_TICKS, .. }));
}

// --- 2. The subscription query the composition builds -------------------------

#[test]
fn the_ingestion_query_rejects_empty_scopes_and_an_inverted_range() {
    let ctx = tenant_context();
    let from = EventTime::from_unix_millis(1_780_272_000_000).unwrap();
    let until = EventTime::from_unix_millis(1_780_280_640_000).unwrap();
    // The composition's query rules (P2.11 D-S2): ≥1 asset, ≥1 quantity,
    // from ≤ until.
    assert_eq!(
        TelemetryQuery::new(&ctx, Vec::new(), from, until, all_quantities()),
        Err(TelemetryQueryError::EmptyAssetSet)
    );
    assert_eq!(
        TelemetryQuery::new(&ctx, vec![asset_id("site-north-bess-1")], from, until, Vec::new()),
        Err(TelemetryQueryError::EmptyQuantitySet)
    );
    assert_eq!(
        TelemetryQuery::new(
            &ctx,
            vec![asset_id("site-north-bess-1")],
            until,
            from,
            all_quantities()
        ),
        Err(TelemetryQueryError::InvertedTimeRange)
    );
    // The composition's real shape (tenant from the context, the mapped
    // asset set, the horizon window, the full frozen quantity vocabulary)
    // is valid.
    let query = TelemetryQuery::new(
        &ctx,
        asset_mapping().iter().map(|(_, id)| *id).collect(),
        from,
        until,
        all_quantities(),
    )
    .unwrap();
    assert_eq!(query.tenant_id(), tenant_context().tenant_id());
    assert_eq!(query.assets().len(), ASSET_KEYS.len());
}

// --- 3. Composition smoke test (pure, no DB) ---------------------------------

/// The replay-comparison tuple (P2.13 §3 / P2.24): everything BUT
/// `ingest_time`.
fn replay_key(record: &NormalizedTelemetry) -> (AssetId, i128, TelemetryQuantity, QuantityValue, TelemetryQuality, u64) {
    (
        record.asset_id(),
        record.event_time().unix_millis(),
        record.quantity(),
        record.value().cloned().expect("measured records carry values"),
        record.quality(),
        record.source_sequence(),
    )
}

#[test]
fn the_composed_simulator_source_implements_the_port_and_reproduces() {
    // The exact object compose_ingestion returns (SimulatorTelemetrySource
    // over the v2 scenario + the key → ID mapping).
    let source = SimulatorTelemetrySource::from_scenario(
        load_scenario(SCENARIO_V2).unwrap(),
        asset_mapping(),
    )
    .expect("the v2 exit-gate scenario plus the test mapping must build");
    // The provider rule (P2.11 step 4), declared structurally.
    assert_eq!(source.source_id(), "sim://synthetic-portfolio-exit-gate");
    assert_eq!(source.provider_declaration().cadence(), Duration::from_secs(60));
    assert_eq!(
        source.provider_declaration().clock_quality(),
        ClockQuality::Synchronized
    );
    assert!(!source.source_id().starts_with("flexcore:"));
    // Idle before start: an honest self-report, never a silent good health.
    assert!(matches!(
        TelemetrySource::health(&source),
        TelemetrySourceHealth::Unavailable { .. }
    ));
    source.start();

    // The port object the pipeline consumes (the composition hands the
    // pipeline an `Arc<dyn TelemetrySource>`; the bridge keeps the concrete
    // pull path — mirrored here with two handles on one Arc).
    let concrete = Arc::new(source);
    let port: Arc<dyn TelemetrySource> = Arc::clone(&concrete) as Arc<dyn TelemetrySource>;
    assert_eq!(port.source_id(), "sim://synthetic-portfolio-exit-gate");

    // The query compose_ingestion builds: the mapped asset set over the
    // scenario's recorded horizon (24 h at the 1-minute cadence).
    let scenario = load_scenario(SCENARIO_V2).unwrap();
    let start_ms = scenario.start_ms;
    let horizon_millis = scenario.total_ticks as i64 * scenario.cadence_seconds * 1_000;
    let from = EventTime::from_unix_millis(start_ms).unwrap();
    let until = EventTime::from_unix_millis(start_ms + horizon_millis).unwrap();
    let query = TelemetryQuery::new(
        &tenant_context(),
        asset_mapping().iter().map(|(_, id)| *id).collect(),
        from,
        until,
        all_quantities(),
    )
    .unwrap();
    let subscription = port.subscribe(&query).unwrap();

    // Pull the whole horizon through the concrete source's pull path and
    // decode through the PORT — the same split the worker's source bridge
    // and pipeline use.
    let mut records: Vec<NormalizedTelemetry> = Vec::new();
    while let Some(frame) = concrete.pull_frame(&subscription).unwrap() {
        records.extend(port.decode(&subscription, &frame).unwrap());
    }
    // The exit-gate arithmetic (P2.25 §3): 1440 ticks × 34 records per tick
    // (BESS 6 + PV 3 + load 2 + EVSE 4 + meter 2 quantities, × 2 sites) —
    // 48,960 records, every one a complete canonical record.
    assert_eq!(records.len(), 48_960);
    let cadence_millis = i128::from(scenario.cadence_seconds * 1_000);
    for record in &records {
        assert_eq!(record.source_id(), "sim://synthetic-portfolio-exit-gate");
        assert_eq!(record.tenant_id(), tenant_context().tenant_id());
        assert_eq!(record.clock_quality(), ClockQuality::Synchronized);
        assert_eq!(record.quality(), TelemetryQuality::Good);
        assert!(record.source_quality_code().is_none());
        assert!(
            record.ingest_time().unix_millis() >= record.event_time().unix_millis(),
            "ingest_time >= event_time by construction (P0.09)"
        );
        // Every event_time sits on the scenario clock's cadence grid.
        let offset = record.event_time().unix_millis() - i128::from(start_ms);
        assert!(offset >= 0 && offset < i128::from(horizon_millis));
        assert_eq!(offset % cadence_millis, 0, "event_time off the cadence grid");
    }

    // Determinism (P2.13 §3 / P2.24): a fresh instance over the same
    // scenario + mapping reproduces the replay tuple exactly, ingest_time
    // excluded by construction. The second drain decodes through the second
    // instance's own port view (its subscription state is per-instance).
    let replay_a: Vec<_> = records.iter().map(replay_key).collect();
    let second = SimulatorTelemetrySource::from_scenario(
        load_scenario(SCENARIO_V2).unwrap(),
        asset_mapping(),
    )
    .unwrap();
    second.start();
    let second_subscription = second.subscribe(&query).unwrap();
    let second_dyn: &dyn TelemetrySource = &second;
    let mut replay_b: Vec<_> = Vec::new();
    while let Some(frame) = second.pull_frame(&second_subscription).unwrap() {
        for record in second_dyn.decode(&second_subscription, &frame).unwrap() {
            replay_b.push(replay_key(&record));
        }
    }
    assert_eq!(replay_a.len(), replay_b.len());
    for (a, b) in replay_a.iter().zip(replay_b.iter()) {
        assert_eq!(a, b, "same scenario + seed must reproduce (P2.13 §3)");
    }
}
