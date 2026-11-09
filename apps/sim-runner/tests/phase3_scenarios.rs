// P3.26 — Deterministic Phase 3 scenario suite: the three pinned synthetic
// scenarios executed through the sim-runner replay harness (determinism
// identity) plus the asset/time-coupled constraint assertions (report
// §19.2 layer 5, §19.3, §23 Stage 3; ledger P3.26).
//
// THE SCENARIOS (scenarios/synthetic-portfolio/scenarios-phase3/, all
// SYNTHETIC — see that directory's README):
//   (a) scenario-a-normal-day.json — all assets available, central forecast;
//   (b) scenario-b-pv-ramp.json     — same inputs, forecast input is the
//       P3.31 error variant (PV down 80 % of clear-sky, 12:00–13:00 UTC);
//   (c) scenario-c-dropout.json     — one mid-horizon telemetry drop on
//       site-north-bess-1, 12:00–13:00 UTC (the P3.24 dropout-trigger
//       analogue at the telemetry layer).
//
// WHAT IS TESTED HERE (offline-deterministic parts — fully tested):
//   1. Every scenario loads all-or-nothing and carries the pinned identity
//      (synthetic flag, version, master seed, the phase3 pin block with the
//      solver pins and the forecast reference).
//   2. DETERMINISM IDENTITY: each scenario run twice through the replay
//      harness (`run_scenario`) yields identical `ScenarioRun` values and a
//      zero-mismatch record comparison — same scenario version + same seed
//      ⇒ byte-identical canonical records (P2.13 §3; `ingest_time`
//      excluded by the record shape).
//   3. Scenario (c) dropout semantics at the pipeline layer: no frames for
//      the dropped asset inside the window, frames outside it, and the
//      pre-registered expected-quality plan naming MISSING for the window.
//   4. CONSTRAINT ASSERTIONS (the §23 Stage 3 gate shape, per scenario):
//      the deterministic candidate schedules derived from each scenario's
//      declared dispatch are evaluated through the flexcore-domain
//      capability models and must pass with ZERO violations — BESS: SOC
//      stays inside [E_min, E_max] within the model tolerance; EV: the hard
//      departure-energy constraint is met and charging stays inside the
//      connection windows; load: the baseline-following schedule emits
//      consumption-negative net export (the §10.2 sign convention) with no
//      violations. For scenario (c) the realised SOC at the dropout
//      boundary is carried forward as the initial condition of the
//      post-dropout schedule, and that schedule also passes with zero
//      violations (P3.24: never re-solve the past as if it had not
//      happened).
//
// VERIFY-BUILD (recorded honestly — NOT tested here): the real-solve legs
// of P3.26 — the pinned Tier 1 HiGHS worker adapter and the Tier 2 replay
// through `flexcore-optimization-api`'s `ReplayHarness::verify_replay` over
// each scenario's assembled formulation — require the worker adapter and a
// dev-dependency edge from this crate to flexcore-optimization-api that
// this batch deliberately did not add (manifests are coordinator-owned).
// The obligation is recorded in
// ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md together
// with the expected-outcome tables (pre-registered, UNRUN) and the coverage
// command. No coverage figure is claimed anywhere in this file.
//
// NOTE: cargo was not run while authoring; the first `cargo test -p
// flexcore-sim-runner --test phase3_scenarios` run is the build gate.

use flexcore_contracts::capability::Provenance;
use flexcore_domain::bess_model::{
    AuxLoadPoint, BessModel, BessModelConfig, BessSchedule, BessSetpoint, ProvenancedValue,
};
use flexcore_domain::ev_fleet_model::{
    EvFleetConfig, EvFleetModel, EvFleetSchedule, EvSetpoint, EvseConfig, EvVehicleConfig,
    V2xConfig,
};
use flexcore_domain::flexible_load_model::{
    FlexibleLoadConfig, FlexibleLoadModel, LoadSchedule, LoadSetpoint, ReboundEnergy,
};
use flexcore_sim_runner::compare::compare;
use flexcore_sim_runner::replay::{run_scenario, stand_in_asset_id, ScenarioRun};
use serde_json::Value;

/// The repo-root-relative scenario directory (the test crate lives at
/// apps/sim-runner).
const SCENARIO_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenarios-phase3"
);
/// The committed forecast fixture directory (same distance from the crate).
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/synthetic-portfolio");

/// The scenario horizon: 2026-06-01T00:00:00Z on the scenario clock.
const HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
/// The scheduling interval (the phase3 pin block: 24 h / 15 min).
const INTERVAL_SECONDS: i64 = 900;
const INTERVAL_MILLIS: i64 = INTERVAL_SECONDS * 1000;
const INTERVALS_PER_DAY: usize = 96;

/// The recorded fixture extension (README "Named capacities"): the battery
/// capacity of each aggregate EV, MWh. Labelled ASSUMPTION — synthetic
/// value, no measured basis.
const EV_BATTERY_CAPACITY_MWH: f64 = 0.06;

fn read_json(path: &str) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read '{path}': {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("'{path}' is not valid JSON: {e}"))
}

fn scenario_path(name: &str) -> String {
    format!("{SCENARIO_DIR}/{name}")
}

// --- 1 + 2. Identity and determinism identity per scenario -------------------

/// Each scenario loads, carries the pinned identity (synthetic, v1, the
/// master seed, the phase3 pin block with the solver pins), and its pin
/// block names the pinned solvers with their exact versions and the
/// declared problem-class discipline (solver-class honesty is part of the
/// pinned record).
#[test]
fn every_scenario_loads_with_its_pinned_identity() {
    for (file, expected_id) in [
        ("scenario-a-normal-day.json", "synthetic-portfolio-phase3-a-normal-day"),
        ("scenario-b-pv-ramp.json", "synthetic-portfolio-phase3-b-pv-ramp"),
        ("scenario-c-dropout.json", "synthetic-portfolio-phase3-c-dropout"),
    ] {
        let path = scenario_path(file);
        let scenario = read_json(&path);
        assert_eq!(scenario["meta"]["scenario_id"], expected_id, "{file}: identity");
        assert_eq!(scenario["meta"]["synthetic"], true, "{file}: the synthetic flag is mandatory");
        assert_eq!(scenario["meta"]["version"], "v1", "{file}: version");
        assert_eq!(scenario["seeds"]["master_seed"], 20260601, "{file}: pinned seed");
        assert_eq!(scenario["horizon"]["duration_hours"], 24.0, "{file}: 24 h horizon");

        // The phase3 pin block: grid, seed, tolerance, and the solver pins.
        let pins = &scenario["phase3"];
        assert_eq!(pins["scheduling_grid"]["interval_seconds"], 900, "{file}: 15-min grid");
        assert_eq!(pins["master_seed"], 20260601, "{file}: pin-block seed");
        assert_eq!(pins["solver_pins"]["thread_count"], 1, "{file}: fixed thread count");
        assert_eq!(
            pins["solver_pins"]["tier1"]["version"], "1.15.1",
            "{file}: pinned HiGHS version"
        );
        assert_eq!(
            pins["solver_pins"]["tier2"]["version"], "0.11.1",
            "{file}: pinned Clarabel.rs version"
        );
        assert!(
            pins["solver_pins"]["tier1"]["class"]
                .as_str()
                .unwrap_or_default()
                .contains("never SOCP"),
            "{file}: the Tier 1 pin must carry the solver-class discipline"
        );
        assert!(
            pins["forecast_input"].as_str().is_some_and(|f| !f.is_empty()),
            "{file}: the forecast input is pinned"
        );
    }
}

/// Scenario (b) pins the P3.31 error variant as its forecast input, and the
/// committed error-variant artefact exists beside the central one.
#[test]
fn scenario_b_pins_the_error_forecast_variant() {
    let scenario = read_json(&scenario_path("scenario-b-pv-ramp.json"));
    let forecast = scenario["phase3"]["forecast_input"].as_str().expect("pinned forecast");
    assert!(
        forecast.contains("forecast-fixture-v1-error-pv-ramp.json"),
        "scenario (b) must pin the error variant, got '{forecast}'"
    );
    let path = format!("{FIXTURE_DIR}/forecast-fixture-v1-error-pv-ramp.json");
    let fixture = read_json(&path);
    assert_eq!(fixture["meta"]["variant"], "error-pv-ramp", "the variant artefact is the error variant");

    // And scenarios (a)/(c) pin the central fixture.
    for file in ["scenario-a-normal-day.json", "scenario-c-dropout.json"] {
        let scenario = read_json(&scenario_path(file));
        let forecast = scenario["phase3"]["forecast_input"].as_str().expect("pinned forecast");
        assert!(
            forecast.contains("forecast-fixture-v1.json")
                && !forecast.contains("error"),
            "{file} must pin the central fixture, got '{forecast}'"
        );
    }
}

/// DETERMINISM IDENTITY (each scenario): two runs through the replay
/// harness produce identical `ScenarioRun` values (scenario identity, seed,
/// canonical records) and the record comparison reports ZERO mismatches.
#[test]
fn each_scenario_replays_identically_through_the_replay_harness() {
    for file in [
        "scenario-a-normal-day.json",
        "scenario-b-pv-ramp.json",
        "scenario-c-dropout.json",
    ] {
        let path = scenario_path(file);
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read '{path}': {e}"));
        let first: ScenarioRun = run_scenario(&json)
            .unwrap_or_else(|e| panic!("{file}: the run failed: {e}"));
        let second: ScenarioRun = run_scenario(&json)
            .unwrap_or_else(|e| panic!("{file}: the replay run failed: {e}"));

        assert_eq!(first, second, "{file}: the determinism identity must hold exactly");
        assert_eq!(first.master_seed, 20260601, "{file}: the seed in effect");
        assert!(
            !first.records.is_empty(),
            "{file}: a run produces canonical records"
        );
        let comparison = compare(&first.records, &second.records);
        assert!(
            comparison.is_pass(),
            "{file}: replay comparison must pass with zero mismatches (got {:?})",
            comparison.mismatches.first()
        );
        assert_eq!(comparison.match_count, first.records.len(), "{file}: every record pairs");
    }
}

// --- 3. Scenario (c): the mid-horizon dropout at the pipeline layer ----------

/// Scenario (c)'s drop injection removes EVERY frame of
/// `site-north-bess-1` inside 12:00–13:00 UTC, leaves frames outside the
/// window, and the scenario's expected-quality plan pre-registers MISSING
/// for exactly that window (an EXPECTED gap, never an unexplained one).
#[test]
fn scenario_c_dropout_removes_frames_only_inside_the_window() {
    let path = scenario_path("scenario-c-dropout.json");
    let json = std::fs::read_to_string(&path).expect("the scenario file reads");
    let scenario = read_json(&path);

    // The fault plan is enabled with exactly one drop injection.
    assert_eq!(scenario["faults"]["enabled"], true, "the fault plan is enabled");
    let injections = scenario["faults"]["injections"].as_array().expect("injections");
    assert_eq!(injections.len(), 1, "exactly one injection");
    assert_eq!(injections[0]["fault"], "drop");
    assert_eq!(injections[0]["asset_key"], "site-north-bess-1");

    // The pre-registered expected-quality plan names the window.
    let plan = &scenario["expected_quality"]["per_asset"];
    let entry = plan
        .as_array()
        .and_then(|entries| entries.first())
        .expect("one expected-quality entry");
    assert_eq!(entry["expected_quality_in_window"], "MISSING");
    assert_eq!(entry["coverage_in_window"], "expected_gap");

    // The run: no frames inside the window, frames outside it.
    let run = run_scenario(&json).expect("the dropout scenario runs");
    let asset_id = stand_in_asset_id("site-north-bess-1").uuid().to_hyphenated().to_string();
    let window_start = HORIZON_START_MILLIS + 12 * 3600 * 1000;
    let window_end = HORIZON_START_MILLIS + 13 * 3600 * 1000;

    let mut inside = 0usize;
    let mut outside = 0usize;
    for record in &run.records {
        if record.asset_id != asset_id {
            continue;
        }
        if record.event_time_unix_ms >= window_start && record.event_time_unix_ms < window_end {
            inside += 1;
        } else {
            outside += 1;
        }
    }
    assert_eq!(inside, 0, "a drop injection emits NO frames inside the window");
    assert!(
        outside > 0,
        "the dropped asset still reports outside the window (got {outside} records)"
    );
}

// --- 4. Constraint assertions: the deterministic candidate schedules ---------

/// Parses `"HH:MM"` into seconds-of-day.
fn parse_hh_mm(s: &str) -> i64 {
    let (h, m) = s.split_once(':').unwrap_or_else(|| panic!("invalid HH:MM '{s}'"));
    let h: i64 = h.parse().unwrap_or_else(|e| panic!("invalid hour in '{s}': {e}"));
    let m: i64 = m.parse().unwrap_or_else(|e| panic!("invalid minute in '{s}': {e}"));
    h * 3600 + m * 60
}

/// Whether a seconds-of-day instant lies in a window that may wrap midnight.
fn in_window(sod: i64, window: (i64, i64)) -> bool {
    let (start, end) = window;
    if start <= end {
        sod >= start && sod < end
    } else {
        // Wraps midnight: [start, 24h) ∪ [0, end).
        sod >= start || sod < end
    }
}

/// The candidate BESS schedule derived deterministically from a scenario
/// asset's declared dispatch: charge during the charge window, discharge
/// during the discharge window, idle otherwise — with each interval's
/// magnitude clipped EXACTLY to keep the energy trajectory inside
/// [E_min, E_max] (the declared candidate; the model then verifies it
/// independently, so a wrong clamp shows up as a violation).
fn bess_candidate(asset: &Value) -> (BessModel, BessSchedule) {
    let params = &asset["behaviour_profile"]["params"];
    let rated_power = asset["rated_power_mw"].as_f64().expect("rated_power_mw");
    let rated_energy = asset["rated_energy_mwh"].as_f64().expect("rated_energy_mwh");
    let charge_power = params["charge_power_mw"].as_f64().expect("charge_power_mw");
    let discharge_power = params["discharge_power_mw"].as_f64().expect("discharge_power_mw");
    let initial_soc = params["initial_soc"].as_f64().expect("initial_soc");
    let soc_floor = params["soc_floor"].as_f64().expect("soc_floor");
    let soc_ceiling = params["soc_ceiling"].as_f64().expect("soc_ceiling");
    let rte = params["round_trip_efficiency"].as_f64().expect("round_trip_efficiency");

    // Scenario SOC is a fraction of the RATED energy; the model's SOC
    // convention is over the effective window [E_min, E_max]. Convert once,
    // at the boundary, and let the model own its own convention after that.
    let e_min = rated_energy * soc_floor;
    let e_max = rated_energy * soc_ceiling;
    let e_initial = (initial_soc * rated_energy).clamp(e_min, e_max);

    let eta = rte.sqrt(); // symmetric split of the round-trip efficiency
    let model = BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(e_min),
        e_max_mwh: ProvenancedValue::configured(e_max),
        p_ch_max_mw: ProvenancedValue::configured(rated_power.min(charge_power)),
        p_dis_max_mw: ProvenancedValue::configured(rated_power.min(discharge_power)),
        eta_ch: eta,
        eta_dis: eta,
        interval_seconds: INTERVAL_SECONDS as u32,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: None,
        degradation: None,
    })
    .expect("the scenario BESS model is structurally valid");

    let charge_window = {
        let w = params["charge_window_utc"].as_array().expect("charge window");
        (parse_hh_mm(w[0].as_str().unwrap()), parse_hh_mm(w[1].as_str().unwrap()))
    };
    let discharge_window = {
        let w = params["discharge_window_utc"].as_array().expect("discharge window");
        (parse_hh_mm(w[0].as_str().unwrap()), parse_hh_mm(w[1].as_str().unwrap()))
    };

    let dt_h = INTERVAL_SECONDS as f64 / 3600.0;
    let mut e = e_initial;
    let mut setpoints = Vec::with_capacity(INTERVALS_PER_DAY);
    for interval in 0..INTERVALS_PER_DAY {
        let sod = (interval as i64 * INTERVAL_SECONDS) % 86_400;
        let setpoint = if in_window(sod, charge_window) {
            // Charge, never past the ceiling (a float-rounded headroom at
            // the ceiling clamps to zero, never negative).
            let headroom = ((e_max - e) / (eta * dt_h)).max(0.0);
            let p = charge_power.min(headroom);
            e += eta * p * dt_h;
            BessSetpoint::charge(p)
        } else if in_window(sod, discharge_window) {
            // Discharge, never below the floor.
            let available = (e - e_min) * eta / dt_h;
            let p = discharge_power.min(available.max(0.0));
            e -= p / eta * dt_h;
            BessSetpoint::discharge(p)
        } else {
            BessSetpoint::idle()
        };
        setpoints.push(setpoint);
    }

    let schedule = BessSchedule {
        initial_soc: model.energy_to_soc(e_initial),
        setpoints,
        auxiliary_load: vec![
            AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured };
            INTERVALS_PER_DAY
        ],
    };
    (model, schedule)
}

/// CONSTRAINT ASSERTION (per scenario): every declared BESS candidate
/// schedule passes its capability model with ZERO violations — SOC stays
/// inside [E_min, E_max], powers inside the limits, no simultaneity, no
/// reserve breach (asset/time-coupled constraints, §23 Stage 3).
#[test]
fn every_bess_candidate_passes_its_capability_model_with_zero_violations() {
    for file in [
        "scenario-a-normal-day.json",
        "scenario-b-pv-ramp.json",
        "scenario-c-dropout.json",
    ] {
        let scenario = read_json(&scenario_path(file));
        for site in scenario["portfolio"]["sites"].as_array().expect("sites") {
            for asset in site["assets"].as_array().expect("assets") {
                if asset["asset_type"] != "BESS" {
                    continue;
                }
                let key = asset["asset_key"].as_str().expect("asset_key");
                let (model, schedule) = bess_candidate(asset);
                let evaluation = model
                    .evaluate_schedule(&schedule)
                    .unwrap_or_else(|e| panic!("{file}/{key}: evaluation failed: {e}"));
                assert!(
                    evaluation.violations.is_empty(),
                    "{file}/{key}: the declared candidate must pass with ZERO violations \
                     (got {:?})",
                    evaluation.violations.first()
                );
                // SOC bounds within the model tolerance, every boundary.
                for (boundary, &soc) in evaluation.interval_socs.iter().enumerate() {
                    assert!(
                        (0.0..=1.0).contains(&soc),
                        "{file}/{key}: boundary {boundary} SOC {soc} left [0, 1]"
                    );
                }
            }
        }
    }
}

/// CONSTRAINT ASSERTION (per scenario): every declared EVSE candidate
/// charges only inside its connection windows and MEETS the hard
/// departure-energy constraint (README fixture extension: departure at the
/// end of the morning window, required SOC = the profile's soc_target).
#[test]
fn every_ev_candidate_meets_the_hard_departure_constraint() {
    for file in [
        "scenario-a-normal-day.json",
        "scenario-b-pv-ramp.json",
        "scenario-c-dropout.json",
    ] {
        let scenario = read_json(&scenario_path(file));
        for site in scenario["portfolio"]["sites"].as_array().expect("sites") {
            for asset in site["assets"].as_array().expect("assets") {
                if asset["asset_type"] != "EVSE" {
                    continue;
                }
                let key = asset["asset_key"].as_str().expect("asset_key");
                let params = &asset["behaviour_profile"]["params"];
                let rated_power = asset["rated_power_mw"].as_f64().expect("rated_power_mw");
                let charging_power = params["charging_power_mw"].as_f64().expect("charging_power_mw");
                let initial_soc = params["initial_soc"].as_f64().expect("initial_soc");
                let soc_target = params["soc_target"].as_f64().expect("soc_target");

                // Connection windows in millis over the 24 h horizon,
                // unwrapped into non-wrapping segments.
                let mut segments: Vec<(i64, i64)> = Vec::new();
                for window in params["connection_windows_utc"].as_array().expect("windows") {
                    let start = parse_hh_mm(window[0].as_str().unwrap());
                    let end = parse_hh_mm(window[1].as_str().unwrap());
                    if start <= end {
                        segments.push((HORIZON_START_MILLIS + start * 1000, HORIZON_START_MILLIS + end * 1000));
                    } else {
                        // Wraps midnight: [start, 24h) ∪ [0, end).
                        segments.push((HORIZON_START_MILLIS + start * 1000, HORIZON_START_MILLIS + 86_400_000));
                        segments.push((HORIZON_START_MILLIS, HORIZON_START_MILLIS + end * 1000));
                    }
                }
                // The departure instant: the end of the segment that starts
                // at or before 09:00 and ends latest among morning segments
                // (the README's recorded interpretation). Deterministic and
                // simple: the LATEST segment end within the horizon that is
                // ≤ 12:00 UTC, i.e. the end of the morning connection.
                let departure = segments
                    .iter()
                    .map(|&(_, end)| end)
                    .filter(|&end| end <= HORIZON_START_MILLIS + 12 * 3600 * 1000)
                    .max()
                    .expect("a morning connection window exists for the pinned scenarios");
                assert!(
                    (departure - HORIZON_START_MILLIS) % INTERVAL_MILLIS == 0,
                    "{file}/{key}: the departure must fall on an interval boundary"
                );

                let vehicle = EvVehicleConfig {
                    vehicle_key: key.to_string(),
                    battery_capacity_mwh: EV_BATTERY_CAPACITY_MWH,
                    initial_soc,
                    required_departure_soc: soc_target,
                    departure_millis: departure,
                    connection_windows_millis: segments.clone(),
                    max_charging_power_mw: charging_power,
                    min_charging_power_mw: 0.0,
                    ramp_limit_mw_per_minute: None,
                    charging_priority: 1,
                    opt_out_from_millis: None,
                };
                let model = EvFleetModel::new(EvFleetConfig {
                    evse: EvseConfig { rated_power_mw: rated_power, phase: "L1".to_string() },
                    vehicles: vec![vehicle],
                    site_transformer_limit_mw: 1.0,
                    v2x: V2xConfig::disabled(),
                    interval_seconds: INTERVAL_SECONDS as u32,
                    horizon_start_millis: HORIZON_START_MILLIS,
                })
                .unwrap_or_else(|e| panic!("{file}/{key}: the fleet model must construct: {e}"));

                // The candidate: charge at the declared power while
                // connected and below the target, idle otherwise.
                let interval_count = ((departure - HORIZON_START_MILLIS) / INTERVAL_MILLIS) as usize;
                let dt_h = INTERVAL_SECONDS as f64 / 3600.0;
                let capacity = EV_BATTERY_CAPACITY_MWH;
                let target_energy = soc_target * capacity;
                let mut stored = initial_soc * capacity;
                let mut setpoints = Vec::with_capacity(interval_count);
                for interval in 0..interval_count {
                    let start = HORIZON_START_MILLIS + interval as i64 * INTERVAL_MILLIS;
                    let connected = segments.iter().any(|&(s, e)| start >= s && start < e);
                    let setpoint = if connected && stored < target_energy - 1e-12 {
                        let needed = (target_energy - stored) / dt_h;
                        let p = charging_power.min(needed);
                        stored += p * dt_h;
                        EvSetpoint::charge(p)
                    } else {
                        EvSetpoint::idle()
                    };
                    setpoints.push(setpoint);
                }
                let schedule = EvFleetSchedule {
                    per_vehicle: vec![(key.to_string(), setpoints)],
                };
                let evaluation = model
                    .evaluate_schedule(&schedule)
                    .unwrap_or_else(|e| panic!("{file}/{key}: evaluation failed: {e}"));
                assert!(
                    evaluation.violations.is_empty(),
                    "{file}/{key}: the EV candidate must pass with ZERO violations (got {:?})",
                    evaluation.violations.first()
                );
                let record = &evaluation.per_vehicle[0];
                assert!(
                    record.departure_energy_met,
                    "{file}/{key}: the hard departure-energy constraint must be met \
                     (soc {} vs required {})",
                    record.soc_at_departure, record.required_departure_soc
                );
            }
        }
    }
}

/// CONSTRAINT ASSERTION (per scenario): the declared flexible-load
/// candidate follows the pinned fixture baseline with ZERO violations, and
/// its net export is consumption-NEGATIVE at every interval (the §10.2
/// sign convention: baseline demand is consumption, i.e. negative export).
/// The baseline comes from the committed central forecast fixture (the
/// single labelled forecast source, P3.31), never invented here.
#[test]
fn every_load_candidate_follows_the_baseline_with_consumption_negative_export() {
    let fixture = read_json(&format!("{FIXTURE_DIR}/forecast-fixture-v1.json"));
    for file in [
        "scenario-a-normal-day.json",
        "scenario-b-pv-ramp.json",
        "scenario-c-dropout.json",
    ] {
        let scenario = read_json(&scenario_path(file));
        for site in scenario["portfolio"]["sites"].as_array().expect("sites") {
            for asset in site["assets"].as_array().expect("assets") {
                if asset["asset_type"] != "FLEXIBLE_LOAD" {
                    continue;
                }
                let key = asset["asset_key"].as_str().expect("asset_key");

                // The labelled fixture's baseline series for this asset
                // (consumption negative, MW); the model's baseline is the
                // non-negative consumption magnitude.
                let values: Vec<f64> = fixture["series"]
                    .as_array()
                    .expect("fixture series")
                    .iter()
                    .find(|s| s["asset_key"] == *key && s["quantity"] == "load_mw")
                    .unwrap_or_else(|| panic!("the fixture carries the baseline for {key}"))
                    ["values"]
                    .as_array()
                    .expect("baseline values")
                    .iter()
                    .map(|v| -v.as_f64().expect("a finite MW value"))
                    .collect();
                assert!(!values.is_empty(), "{file}/{key}: the baseline is non-empty");

                let horizon_millis = values.len() as i64 * 60_000;
                let model = FlexibleLoadModel::new(FlexibleLoadConfig {
                    baseline_method:
                        flexcore_domain::flexible_load_model::BaselineMethod::scenario_declared(),
                    baseline_demand_mw: values.clone(),
                    reducible_range_mw: ProvenancedValue::configured(
                        asset["behaviour_profile"]["params"]["reducible_range_mw"]
                            .as_f64()
                            .expect("reducible_range_mw"),
                    ),
                    increasable_range_mw: ProvenancedValue::configured(0.0),
                    response_time_seconds: 60.0,
                    max_event_duration_seconds: 3600.0,
                    min_recovery_seconds: 900.0,
                    rebound: ReboundEnergy::explicit_zero(
                        "SYNTHETIC FIXTURE: no rebound process model is on record; the \
                         zero is recorded with this reason, never defaulted (ledger P3.10)"
                            .to_string(),
                    )
                    .expect("a recorded zero reason is valid"),
                    comfort_constraints: vec![],
                    availability_windows_millis: vec![
                        (HORIZON_START_MILLIS, HORIZON_START_MILLIS + horizon_millis)
                    ],
                    interval_seconds: 60,
                    horizon_start_millis: HORIZON_START_MILLIS,
                })
                .unwrap_or_else(|e| panic!("{file}/{key}: the load model must construct: {e}"));

                // The candidate: follow the baseline (idle everywhere).
                let schedule =
                    LoadSchedule { setpoints: vec![LoadSetpoint::idle(); values.len()] };
                let evaluation = model
                    .evaluate_schedule(&schedule)
                    .unwrap_or_else(|e| panic!("{file}/{key}: evaluation failed: {e}"));
                assert!(
                    evaluation.violations.is_empty(),
                    "{file}/{key}: the baseline-following candidate must pass with ZERO \
                     violations (got {:?})",
                    evaluation.violations.first()
                );
                for (interval, &net) in evaluation.net_export_mw.iter().enumerate() {
                    assert!(
                        net <= 1e-9,
                        "{file}/{key}: interval {interval}: baseline-following net export \
                         {net} MW must be consumption-negative (P > 0 = export, §10.2)"
                    );
                }
            }
        }
    }
}

/// SCENARIO (c) P3.24 semantics at the scheduling layer: the realised SOC
/// at the dropout boundary (12:00 UTC, interval 48) is carried forward as
/// the initial condition of the post-dropout schedule for
/// `site-north-bess-1`, and that post-dropout schedule — covering ONLY the
/// remaining intervals — passes with ZERO violations. The past is frozen
/// (the pre-dropout trajectory is not re-evaluated), never re-solved as if
/// the dropout had not happened.
#[test]
fn scenario_c_realised_soc_is_carried_forward_and_the_post_dropout_schedule_passes() {
    const DROPOUT_INTERVAL: usize = 48; // 12:00 UTC on the 15-min grid

    let scenario = read_json(&scenario_path("scenario-c-dropout.json"));
    let asset = scenario["portfolio"]["sites"]
        .as_array()
        .expect("sites")
        .iter()
        .flat_map(|site| site["assets"].as_array().expect("assets").iter())
        .find(|a| a["asset_key"] == "site-north-bess-1")
        .expect("the dropped BESS asset is in the scenario")
        .clone();

    // The pre-dropout candidate and its realised SOC at the boundary.
    let (model, schedule) = bess_candidate(&asset);
    let pre = model
        .evaluate_schedule(&schedule)
        .expect("the pre-dropout candidate evaluates");
    assert!(
        pre.violations.is_empty(),
        "the pre-dropout trajectory must be violation-free (got {:?})",
        pre.violations.first()
    );
    let realised_soc = pre.interval_socs[DROPOUT_INTERVAL];

    // The post-dropout schedule: the realised SOC as the initial condition,
    // the REMAINING intervals only (the past is frozen, not re-solved).
    let remaining = INTERVALS_PER_DAY - DROPOUT_INTERVAL;
    let (post_model, _) = bess_candidate(&asset);
    let post_schedule = BessSchedule {
        initial_soc: realised_soc,
        setpoints: schedule.setpoints[DROPOUT_INTERVAL..].to_vec(),
        auxiliary_load: vec![
            AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured };
            remaining
        ],
    };
    let post = post_model
        .evaluate_schedule(&post_schedule)
        .expect("the post-dropout candidate evaluates");
    assert!(
        post.violations.is_empty(),
        "the post-dropout schedule must respect the realised SOC with ZERO violations \
         (got {:?})",
        post.violations.first()
    );
    // Continuity: the post-dropout trajectory starts exactly at the
    // realised boundary state (the carry-forward is exact, within the
    // model tolerance).
    assert!(
        (post.interval_energies_mwh[0] - pre.interval_energies_mwh[DROPOUT_INTERVAL]).abs()
            <= 1e-9,
        "the post-dropout initial energy must equal the realised boundary energy"
    );
}
