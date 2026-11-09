// P3.27 — Benchmark-harness tests: the `BenchmarkRecord` shape, the
// deterministic ×2/×4 scaling-probe copy generation, the replay-stability
// contract for the record's non-measured fields, and the fail-loud
// empty-cell contract (report §19.3, §19.4; ledger
// `ledger/phase-03-flexibility-scheduling/ledger_phase_3.md` §P3.27).
//
// WHAT IS TESTED HERE (pure, offline-deterministic):
//   1. RECORD SHAPE: every required field is present in the serialized
//      record — the scenario pin, the hardware/container block, ALL FOUR
//      §19.4 measured fields (runtime, objective value, feasible interval
//      count, schedule deviation), the completeness summary, and the
//      v0.2.0-baseline label. The replay runtime is actually measured; the
//      solver-side fields are `not_measured` with a non-empty reason
//      (VERIFY-BUILD G5) — never a zero, never a blank.
//   2. SCALING PROBE: the ×2 and ×4 copy generation is deterministic
//      (byte-identical for the same input), asset-key-disjoint from the
//      base scenario and from each other (every copy carries the
//      `--x<factor>-<k>` suffix), size-correct (×2 = twice the sites and
//      assets, ×4 = four times), and the scenario loader accepts the
//      copies (the remapped fault-injection keys validate).
//   3. STABILITY: two records of the same pinned scenario have identical
//      non-measured content (`stability_json`) — only the measured runtime
//      number and the wall-clock generation instant may differ.
//   4. FAIL-LOUD EMPTY-CELL CONTRACT: a record with any `not_measured`
//      field fails `validate_complete` naming the fields; the
//      `not_measured` serialization carries NO numeric value (there is no
//      default zero anywhere); a fully measured record passes.
//
// NO MEASURED PERFORMANCE CLAIM exists in this file: the assertions are
// structural (field presence, statuses, determinism, disjointness) — never
// a runtime value or threshold (report §19.4: no arbitrary universal
// targets). The measured figures themselves belong to the human's single
// recorded run, filled into
// `ledger/phase-03-flexibility-scheduling/27_benchmark_report.md`.
//
// NOTE: cargo was not run while authoring; the first
// `cargo test -p flexcore-sim-runner --test benchmark` run is the build
// gate.

use flexcore_sim_runner::benchmark::{
    capture_hardware, scaling_scenario_json, MeasuredField, RECORD_SCHEMA, RECORD_SUBTASK,
};
use flexcore_sim_runner::benchmark::BenchmarkRecord;
use serde_json::Value;

/// The repo-root-relative phase-3 scenario directory (the test crate lives
/// at apps/sim-runner).
const SCENARIO_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenarios-phase3"
);

fn scenario_text(name: &str) -> String {
    let path = format!("{SCENARIO_DIR}/{name}");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read '{path}': {e}"))
}

fn scenario_a() -> String {
    scenario_text("scenario-a-normal-day.json")
}

fn measure_base(name: &str) -> BenchmarkRecord {
    BenchmarkRecord::measure_base(&scenario_text(name), "benchmark-tests")
        .unwrap_or_else(|e| panic!("{name}: the base measurement failed: {e}"))
}

// --- 1. Record shape -------------------------------------------------------------

/// The record carries EVERY required field: schema tag, subtask, the
/// v0.2.0-baseline label, the scenario pin (id, version, seed, forecast,
/// solver pins), the hardware/container block, ALL FOUR §19.4 measured
/// fields plus the replay runtime, and the completeness summary.
#[test]
fn the_record_carries_every_required_field() {
    let record = measure_base("scenario-a-normal-day.json");
    let value: Value =
        serde_json::from_str(&record.to_json().unwrap()).expect("the record is valid JSON");

    assert_eq!(value["record_schema"], RECORD_SCHEMA);
    assert_eq!(value["subtask"], RECORD_SUBTASK);
    let label = value["baseline_label"].as_str().expect("baseline label");
    assert!(
        label.contains("v0.2.0-baseline") && label.to_lowercase().contains("not a target"),
        "the baseline label must state baseline-for-comparison, not target: '{label}'"
    );

    // Scenario pin.
    assert_eq!(
        value["scenario_pin"]["scenario_id"],
        "synthetic-portfolio-phase3-a-normal-day"
    );
    assert_eq!(value["scenario_pin"]["scenario_version"], "v1");
    assert_eq!(value["scenario_pin"]["master_seed"], 20260601);
    assert!(
        value["scenario_pin"]["forecast_input"]
            .as_str()
            .is_some_and(|f| f.contains("forecast-fixture-v1.json")),
        "the forecast pin travels verbatim"
    );
    assert_eq!(
        value["scenario_pin"]["solver_pins"]["tier1"]["version"],
        "1.15.1",
        "the pinned HiGHS version travels verbatim from the scenario pin block"
    );

    // Hardware/container block: present, and never blank.
    for field in [
        "os",
        "cpu_model",
        "logical_cores",
        "logical_cores_source",
        "memory_total",
        "container_flag",
        "container_detection",
        "capture_note",
    ] {
        let text = value["hardware"][field].as_str().unwrap_or_else(|| {
            panic!("hardware.{field} must be present in the record")
        });
        assert!(!text.trim().is_empty(), "hardware.{field} must never be blank");
    }

    // ALL FOUR §19.4 scheduling metrics plus the replay runtime are
    // present as fields.
    for field in [
        "replay_runtime",
        "solver_runtime",
        "objective_value",
        "feasible_interval_count",
        "schedule_deviation",
    ] {
        assert!(
            value["measured"].get(field).is_some(),
            "measured.{field} must be present (measured or not_measured — never absent)"
        );
    }

    // The replay runtime is actually measured, finite, non-negative.
    assert_eq!(value["measured"]["replay_runtime"]["status"], "measured");
    let runtime = value["measured"]["replay_runtime"]["value"]
        .as_f64()
        .expect("a measured runtime carries a number");
    assert!(runtime.is_finite() && runtime >= 0.0);

    // The solver-side fields are fail-loud not_measured with a reason.
    for field in [
        "solver_runtime",
        "objective_value",
        "feasible_interval_count",
        "schedule_deviation",
    ] {
        assert_eq!(
            value["measured"][field]["status"],
            "not_measured",
            "{field} must be explicitly not_measured (VERIFY-BUILD G5), never zero-filled"
        );
        let reason = value["measured"][field]["reason"]
            .as_str()
            .expect("a not_measured field carries its reason");
        assert!(!reason.trim().is_empty(), "{field}: the reason is mandatory");
    }

    // The completeness summary names exactly the unmeasured fields.
    assert_eq!(value["completeness"]["complete"], false);
    let unmeasured: Vec<&str> = value["completeness"]["unmeasured"]
        .as_array()
        .expect("unmeasured list")
        .iter()
        .map(|v| v.as_str().expect("string field names"))
        .collect();
    assert_eq!(
        unmeasured,
        vec![
            "solver_runtime",
            "objective_value",
            "feasible_interval_count",
            "schedule_deviation"
        ]
    );

    // The record count is a positive factual figure.
    assert!(
        value["measured"]["replay_record_count"].as_u64().unwrap() > 0,
        "the replay pipeline produces canonical records"
    );
}

/// The record names the scenario pin VERBATIM — including scenario (c)'s
/// dropout identity — so a benchmark row can never be confused with another
/// scenario's.
#[test]
fn the_record_pins_each_scenario_identity() {
    let expected = [
        (
            "scenario-a-normal-day.json",
            "synthetic-portfolio-phase3-a-normal-day",
        ),
        ("scenario-b-pv-ramp.json", "synthetic-portfolio-phase3-b-pv-ramp"),
        ("scenario-c-dropout.json", "synthetic-portfolio-phase3-c-dropout"),
    ];
    for (file, id) in expected {
        let record = measure_base(file);
        assert_eq!(record.scenario_pin.scenario_id, id, "{file}: identity");
        assert_eq!(record.scenario_pin.master_seed, 20260601, "{file}: seed");
        assert_eq!(record.scenario_pin.scenario_version, "v1", "{file}: version");
    }
}

// --- 2. Scaling probe ------------------------------------------------------------

/// The ×2/×4 copy generation is deterministic (byte-identical across calls)
/// and the base/×2/×4 asset-key sets are pairwise disjoint.
#[test]
fn scaling_copy_generation_is_deterministic_and_asset_key_disjoint() {
    let base = scenario_a();

    let x2_first = scaling_scenario_json(&base, 2).unwrap();
    let x2_second = scaling_scenario_json(&base, 2).unwrap();
    let x4_first = scaling_scenario_json(&base, 4).unwrap();
    let x4_second = scaling_scenario_json(&base, 4).unwrap();
    assert_eq!(x2_first, x2_second, "×2 generation is byte-identical");
    assert_eq!(x4_first, x4_second, "×4 generation is byte-identical");

    fn asset_keys(text: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(text).unwrap();
        value["portfolio"]["sites"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|site| site["assets"].as_array().unwrap().iter())
            .map(|asset| asset["asset_key"].as_str().unwrap().to_string())
            .collect()
    }
    let base_keys: std::collections::BTreeSet<String> =
        asset_keys(&base).into_iter().collect();
    let x2_keys: std::collections::BTreeSet<String> =
        asset_keys(&x2_first).into_iter().collect();
    let x4_keys: std::collections::BTreeSet<String> =
        asset_keys(&x4_first).into_iter().collect();

    assert!(!base_keys.is_empty(), "the base portfolio has assets");
    for (name, keys, other_name, other) in [
        ("base", &base_keys, "×2", &x2_keys),
        ("base", &base_keys, "×4", &x4_keys),
        ("×2", &x2_keys, "×4", &x4_keys),
    ] {
        assert!(
            keys.is_disjoint(other),
            "{name} and {other_name} asset-key sets must be disjoint (every copy is \
             suffixed, so no key collides)"
        );
    }
}

/// The copies are size-correct: ×2 has twice the sites and assets, ×4 four
/// times; the scaled scenario id is suffixed; and the copy spec travels in
/// the record.
#[test]
fn scaling_copies_are_size_correct_and_recorded() {
    let base = scenario_a();
    let base_value: Value = serde_json::from_str(&base).unwrap();
    let base_sites = base_value["portfolio"]["sites"].as_array().unwrap().len();
    let base_assets: usize = base_value["portfolio"]["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["assets"].as_array().unwrap().len())
        .sum();

    for factor in [2u32, 4] {
        let scaled = scaling_scenario_json(&base, factor).unwrap();
        let value: Value = serde_json::from_str(&scaled).unwrap();
        let sites = value["portfolio"]["sites"].as_array().unwrap().len();
        let assets: usize = value["portfolio"]["sites"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["assets"].as_array().unwrap().len())
            .sum();
        assert_eq!(sites, base_sites * factor as usize, "×{factor}: sites scale");
        assert_eq!(assets, base_assets * factor as usize, "×{factor}: assets scale");
        assert!(
            value["meta"]["scenario_id"]
                .as_str()
                .unwrap()
                .ends_with(&format!("--x{factor}")),
            "×{factor}: the scenario id is suffixed"
        );
        // Every asset key carries the factor suffix (disjointness by
        // construction).
        for key in value["portfolio"]["sites"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["assets"].as_array().unwrap().iter())
            .map(|a| a["asset_key"].as_str().unwrap())
        {
            assert!(
                key.contains(&format!("--x{factor}-")),
                "×{factor}: asset key '{key}' carries the copy suffix"
            );
        }
    }

    // The scaling record carries the copy spec and the suffixed pin.
    let record = BenchmarkRecord::measure_scaling(&base, 2, "benchmark-tests")
        .unwrap_or_else(|e| panic!("the ×2 measurement failed: {e}"));
    assert_eq!(record.scenario_pin.scenario_id, "synthetic-portfolio-phase3-a-normal-day--x2");
    assert_eq!(record.scaling.portfolio_factor, 2);
    assert_eq!(record.scaling.asset_key_count, base_assets * 2);
    assert_eq!(
        record.scaling.copied_site_keys.len(),
        base_sites * 2,
        "the copied site keys are part of the record"
    );
    assert!(record.scaling.method.contains("--x<factor>-<k>"));
}

/// The scaling copies LOAD through the scenario loader (the remapped
/// fault-injection keys validate) and produce a proportional record count.
/// Scenario (c) is the case that matters here: it is the only one with an
/// enabled fault plan whose injection key must remap to the copy-1 key.
#[test]
fn scaling_copies_load_through_the_replay_harness() {
    let base = scenario_text("scenario-c-dropout.json");
    let base_record = BenchmarkRecord::measure_base(&base, "benchmark-tests")
        .unwrap_or_else(|e| panic!("the scenario (c) base measurement failed: {e}"));
    let x2_record = BenchmarkRecord::measure_scaling(&base, 2, "benchmark-tests")
        .unwrap_or_else(|e| panic!("the scenario (c) ×2 measurement failed: {e}"));

    assert!(
        x2_record.measured.replay_record_count > 0,
        "the scaled copy runs through the replay pipeline"
    );
    assert_eq!(
        x2_record.measured.replay_record_count,
        2 * base_record.measured.replay_record_count,
        "×2 doubles the deterministic per-asset record count"
    );
    assert_eq!(
        x2_record.scenario_pin.master_seed, base_record.scenario_pin.master_seed,
        "the master seed is unchanged by scaling"
    );
    // The remapped injection key exists in the copy portfolio.
    let x2_json = scaling_scenario_json(&base, 2).unwrap();
    let value: Value = serde_json::from_str(&x2_json).unwrap();
    let injected = value["faults"]["injections"][0]["asset_key"]
        .as_str()
        .unwrap();
    assert!(
        injected.ends_with("--x2-1"),
        "the fault injection remaps to the copy-1 key, got '{injected}'"
    );
    let portfolio_keys: Vec<&str> = value["portfolio"]["sites"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["assets"].as_array().unwrap().iter())
        .filter_map(|a| a["asset_key"].as_str())
        .collect();
    assert!(
        portfolio_keys.contains(&injected),
        "the remapped injection key is present in the scaled portfolio"
    );
}

// --- 3. Stability ------------------------------------------------------------------

/// The record's non-measured fields are replay-stable: two measurements of
/// the same pinned scenario produce identical `stability_json` views. Only
/// the measured runtime number and the wall-clock generation instant may
/// differ between runs.
#[test]
fn the_records_non_measured_fields_are_replay_stable() {
    let scenario = scenario_a();
    let first = BenchmarkRecord::measure_base(&scenario, "benchmark-tests").unwrap();
    let second = BenchmarkRecord::measure_base(&scenario, "benchmark-tests").unwrap();

    assert_eq!(
        first.stability_json().unwrap(),
        second.stability_json().unwrap(),
        "the stability view must be identical across runs"
    );

    // The stability view really does strip the volatile fields.
    let view: Value = serde_json::from_str(&first.stability_json().unwrap()).unwrap();
    assert!(
        view["run_context"].get("generated_at_unix_ms").is_none(),
        "the wall-clock instant is volatile and excluded"
    );
    assert!(
        view["measured"]["replay_runtime"].get("value").is_none(),
        "the measured number is excluded from the stability view"
    );
    assert_eq!(
        view["measured"]["replay_runtime"]["status"], "measured",
        "the status survives"
    );
    assert_eq!(
        view["run_context"]["code_version"], "benchmark-tests",
        "the code version is stable context and survives"
    );
}

// --- 4. The fail-loud empty-cell contract -------------------------------------------

/// A record with any `not_measured` field fails `validate_complete` with
/// the fields NAMED; a fully measured record passes. There is no default
/// zero anywhere: the failure is loud or the measurement is real.
#[test]
fn the_empty_cell_contract_fails_loud_and_never_defaults_to_zero() {
    let record = measure_base("scenario-a-normal-day.json");
    let error = record
        .validate_complete()
        .expect_err("a G5-incomplete record must fail the completeness check");
    for expected in [
        "solver_runtime",
        "objective_value",
        "feasible_interval_count",
        "schedule_deviation",
    ] {
        assert!(
            error.iter().any(|name| name == expected),
            "the failure must name '{expected}', got {error:?}"
        );
    }

    // A synthetic fully-measured record passes (test fixture values only —
    // clearly labelled, never presented as a measurement of anything).
    let mut complete = record.clone();
    complete.measured.replay_runtime =
        MeasuredField::measured(0.0, "s", "TEST FIXTURE (not a measurement)").unwrap();
    complete.measured.solver_runtime =
        MeasuredField::measured(0.0, "s", "TEST FIXTURE (not a measurement)").unwrap();
    complete.measured.objective_value =
        MeasuredField::measured(0.0, "dimensionless", "TEST FIXTURE (not a measurement)").unwrap();
    complete.measured.feasible_interval_count =
        MeasuredField::measured(0.0, "count", "TEST FIXTURE (not a measurement)").unwrap();
    complete.measured.schedule_deviation =
        MeasuredField::measured(0.0, "MW", "TEST FIXTURE (not a measurement)").unwrap();
    assert!(
        complete.validate_complete().is_ok(),
        "a fully measured record passes the completeness check"
    );
}

/// The `not_measured` serialization carries NO numeric value and no zero —
/// the empty cell is a named status with a reason, so no reader (human or
/// machine) can mistake it for a measured 0.
#[test]
fn a_not_measured_field_serializes_without_any_number() {
    let record = measure_base("scenario-a-normal-day.json");
    let value: Value =
        serde_json::from_str(&record.to_json().unwrap()).expect("the record is valid JSON");
    let objective = &value["measured"]["objective_value"];
    assert_eq!(objective["status"], "not_measured");
    assert!(
        objective.get("value").is_none(),
        "a not_measured field must not serialize a value"
    );
    let rendered = objective.to_string();
    assert!(
        !rendered.contains("\"value\""),
        "no value key anywhere in {rendered}"
    );
}

/// The hardware capture describes instead of inventing: every field is
/// either observed or an explicit `unavailable:` marker; the container flag
/// is one of the honest outcomes, never a fabricated "not a container".
#[test]
fn the_hardware_capture_never_invents_a_value() {
    let hardware = capture_hardware();
    // The record's unobservables are marked, not defaulted.
    for (field, value) in [
        ("cpu_model", &hardware.cpu_model),
        ("memory_total", &hardware.memory_total),
    ] {
        if value.starts_with("unavailable") {
            assert!(
                value.contains("record") || value.contains("reader"),
                "{field}: an unavailable marker states the reason, got '{value}'"
            );
        }
    }
    assert!(
        hardware.container_flag.starts_with("explicit-env:")
            || hardware.container_flag.starts_with("indicator-detected:")
            || hardware.container_flag.starts_with("no-indicator-found"),
        "the container flag is one of the recorded honest outcomes, got '{}'",
        hardware.container_flag
    );
}
