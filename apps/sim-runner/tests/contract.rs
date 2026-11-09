// P2.27/P2.11 — contract-suite instantiation against the simulator
//
// The recorded follow-up from
// `ledger/phase-02-asset-telemetry/27_ci_gate_inventory.md`: the reusable
// `TelemetrySource` provider contract suite
// (`flexcore_ports::telemetry_source_contract`, generic over the port) is
// instantiated HERE against `flexcore-simulator`'s real
// `SimulatorTelemetrySource` — this crate legally depends on both sides
// (the suite is in flexcore-ports, the provider in flexcore-simulator),
// which the ports crate itself cannot do (the dependency rule forbids the
// reverse edge; P2.27 step 3). The suite itself is unchanged — that is the
// point of making it generic.
//
// What is driven (the exit-gate scenario v2, master seed 20260601, 24 h at
// the 1-minute cadence, P2.25):
//
// - the full suite on the clean scenario (all-GOOD): provider declaration,
//   canonical record schema, six-state closure, per-asset sequence
//   monotonicity, event_time on the scenario clock, ingest_time at
//   emission, and same-seed reproduction across three independent
//   instances (P2.13 §3; P2.24);
// - the suite on scenario configurations that enable the fault kinds whose
//   QUALITY mapping is what the closure check needs to see exercised
//   (`out_of_range` → SUSPECT with `OUT_OF_RANGE`, `source_invalid` → BAD
//   with `SOURCE_INVALID`). The duplicate / out-of-order / conflicting
//   kinds are deliberately NOT part of the suite runs: they violate the
//   arrival-order contract BY DESIGN (a duplicated or held frame repeats
//   or regresses `source_sequence` in decode order) — that is exactly what
//   the pipeline's dedup (P2.09) handles, and the suite holds providers to
//   the arrival contract the P2.14 source respects outside those faults.
//   Fault injection is scenario configuration, never a code path (P2.15
//   step 2); these runs flip only the `faults` section.
//
// Each suite run subscribes ONE asset and pulls its full horizon, so the
// frame list is one frame per tick in tick order and the provided clock's
// expected event time per frame index is the scenario grid
// `start + tick * cadence` — derived from the scenario's recorded
// constants, never from the frames themselves (the check would be
// circular otherwise).
//
// CI note: no workflow change accompanies this file — the stand-in job
// (`telemetry-contract`, ports) stays as recorded; this real-provider
// instantiation runs as `cargo test -p flexcore-sim-runner --test
// contract` (one-line CI addition is a recorded follow-up for the workflow
// owner).
//
// Everything here is synthetic, simulation-only telemetry (the entire 0.x
// line): no field read, no field write, no control authority, no safety
// or certification implication.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::telemetry_source::{TelemetryQuery, TelemetryQuantity, TelemetrySource};
use flexcore_ports::telemetry_source_contract::{run_contract_suite, ContractSpec};
use flexcore_simulator::{load_scenario, SimulatorTelemetrySource};

/// The exit-gate scenario v2 (P2.25): `synthetic-portfolio-exit-gate`,
/// master seed 20260601, 24 h from 2026-06-01T00:00:00Z, 1-minute cadence,
/// faults shipped disabled.
const SCENARIO_V2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v2.json"
));

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

/// The ten stable asset keys of the exit-gate scenario, in scenario order,
/// with fixed test UUIDs (the real key → ID mapping happens at
/// registration, P2.25 step 2; stand-in IDs are deterministic test
/// fixtures, P2.24 §2.5 analogue).
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
        subject: "user:sim-runner-contract".to_string(),
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

/// A started, ready-to-pull source over `scenario_text` with the fixed
/// key → ID mapping (the provider must be cheap to construct and
/// stateless between instances — the property the suite's reproduction
/// check needs, P2.13).
fn build_source(scenario_text: &str) -> SimulatorTelemetrySource {
    let source = SimulatorTelemetrySource::from_scenario(
        load_scenario(scenario_text).unwrap(),
        asset_mapping(),
    )
    .unwrap_or_else(|error| panic!("the scenario must build a source: {error}"));
    source.start();
    source
}

/// The single-asset subscription query over the full recorded horizon.
fn query_for(asset_key: &str) -> TelemetryQuery {
    let scenario = load_scenario(SCENARIO_V2).unwrap();
    let horizon_millis = scenario.total_ticks as i64 * scenario.cadence_seconds * 1_000;
    TelemetryQuery::new(
        &tenant_context(),
        vec![asset_id(asset_key)],
        EventTime::from_unix_millis(scenario.start_ms).unwrap(),
        EventTime::from_unix_millis(scenario.start_ms + horizon_millis).unwrap(),
        all_quantities(),
    )
    .unwrap()
}

/// Pulls the full horizon of one asset, in tick order (the opaque frame
/// bytes the suite feeds to `decode` — the same bytes for every instance
/// it drives, P2.27 step 3).
fn pull_frames(scenario_text: &str, asset_key: &str) -> Vec<Vec<u8>> {
    let source = build_source(scenario_text);
    let query = query_for(asset_key);
    let subscription = source.subscribe(&query).unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = source.pull_frame(&subscription).unwrap() {
        frames.push(frame);
    }
    frames
}

/// The provided scenario clock's event time per tick, derived from the
/// scenario's recorded constants (start, cadence, total ticks) — never
/// from the frames (that would make the clock check circular).
fn expected_event_times() -> Vec<EventTime> {
    let scenario = load_scenario(SCENARIO_V2).unwrap();
    let cadence_millis = scenario.cadence_seconds * 1_000;
    (0..scenario.total_ticks)
        .map(|tick| {
            EventTime::from_unix_millis(scenario.start_ms + tick as i64 * cadence_millis).unwrap()
        })
        .collect()
}

/// Returns the scenario JSON with the fault plan enabled and the
/// injections reduced to exactly one kind — fault injection is scenario
/// configuration (P2.15 step 2); the suite run declares which faults it
/// exercises.
fn scenario_with_fault_kind(kind: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(SCENARIO_V2).unwrap();
    value["faults"]["enabled"] = serde_json::Value::Bool(true);
    let injections: Vec<serde_json::Value> = value["faults"]["injections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|injection| injection["fault"].as_str() == Some(kind))
        .cloned()
        .collect();
    assert!(
        !injections.is_empty(),
        "the scenario declares the '{kind}' injection"
    );
    value["faults"]["injections"] = serde_json::Value::Array(injections);
    value.to_string()
}

#[test]
fn the_simulator_passes_the_full_contract_suite_on_the_exit_gate_scenario() {
    let frames = pull_frames(SCENARIO_V2, "site-north-bess-1");
    let expected = expected_event_times();
    assert_eq!(frames.len(), 1440, "the full 24 h horizon at 1-minute cadence");
    assert_eq!(expected.len(), frames.len());
    let query = query_for("site-north-bess-1");
    let make = || -> Box<dyn TelemetrySource> { Box::new(build_source(SCENARIO_V2)) };
    let spec = ContractSpec {
        make_source: &make,
        query,
        frames: &frames,
        expected_event_times: &expected,
    };
    let outcome = run_contract_suite(&spec);
    assert!(
        outcome.passed,
        "the simulator must pass the provider contract suite; violations: {outcome:?}"
    );
    assert!(outcome.violations.is_empty());
}

/// Runs the suite for one asset under a one-kind fault configuration and
/// additionally proves the run was not vacuous: the fault window really
/// emitted records carrying the mapped canonical state and its declared
/// source code (P0.08 §3).
fn assert_suite_passes_with_fault_kind(kind: &str, asset_key: &str, expected_quality: TelemetryQuality, expected_code: &str) {
    let text = scenario_with_fault_kind(kind);
    let frames = pull_frames(&text, asset_key);
    let expected_times = expected_event_times();
    assert_eq!(frames.len(), 1440, "the fault kind must not change coverage");
    let query = query_for(asset_key);
    let make = || -> Box<dyn TelemetrySource> { Box::new(build_source(&text)) };
    let spec = ContractSpec {
        make_source: &make,
        query,
        frames: &frames,
        expected_event_times: &expected_times,
    };
    let outcome = run_contract_suite(&spec);
    assert!(
        outcome.passed,
        "the simulator must pass the suite with the '{kind}' fault declared; violations: {outcome:?}"
    );

    // Non-vacuity proof: an independent run over the same configuration
    // really emits the mapped state with the declared source code.
    let source = build_source(&text);
    let subscription = source.subscribe(&query_for(asset_key)).unwrap();
    let mut exercised = 0;
    while let Some(frame) = source.pull_frame(&subscription).unwrap() {
        for record in source.decode(&subscription, &frame).unwrap() {
            if record.quality() == expected_quality {
                assert_eq!(
                    record.source_quality_code(),
                    Some(expected_code),
                    "the mapped source code must travel on the record (P0.08 §3)"
                );
                exercised += 1;
            }
        }
    }
    assert!(
        exercised > 0,
        "the '{kind}' window must emit {expected_quality:?} records (the closure check must not pass vacuously)"
    );
}

#[test]
fn the_out_of_range_fault_stays_inside_the_declared_quality_closure() {
    // out_of_range (site-north-pv-1, 10:00–10:10): SUSPECT with
    // `OUT_OF_RANGE`.
    assert_suite_passes_with_fault_kind(
        "out_of_range",
        "site-north-pv-1",
        TelemetryQuality::Suspect,
        "OUT_OF_RANGE",
    );
}

#[test]
fn the_source_invalid_fault_stays_inside_the_declared_quality_closure() {
    // source_invalid (site-north-load-1, 05:00–05:10): BAD with
    // `SOURCE_INVALID`.
    assert_suite_passes_with_fault_kind(
        "source_invalid",
        "site-north-load-1",
        TelemetryQuality::Bad,
        "SOURCE_INVALID",
    );
}
