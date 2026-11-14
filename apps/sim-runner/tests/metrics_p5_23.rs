// P5.23/P5.24 — The metrics (timing-surface) and load-record suites
//         (report §19.3–§19.4, §19.2 layer 8; ledger P5.23 + P5.24).
//
// WHAT IS TESTED (all in-process, deterministic, no network, no wall-clock
// assertions on VALUES — only on structure and honesty contracts):
//   1. The per-leg timings block records measured legs only; an untimed
//      leg is ABSENT (never zero, never a placeholder); a duplicate leg is
//      a named error; a non-finite figure is refused.
//   2. The scenario-run record JSON carries the additive `fault_journal`
//      and `timings` fields exactly when they were attached (skip-if-empty
//      serialisation — old consumers unaffected).
//   3. The load-record schema carries the honesty contracts: the scope
//      notes (console browser-side NOT measured; single sequential
//      client), the baseline label, the hardware block, and every
//      not-measured field WITH its mandatory reason and NO numeric value.
//   4. The load spec validator fails loud on an empty load, zero
//      iterations, and an https URL (named errors, exit-2 territory).
//   5. The nearest-rank percentile is reproducible from the samples
//      (documented method; no interpolation, no fitting).
//   6. A produced load record (ingest leg only — the deterministic replay
//      pipeline, no network) carries measured ingest fields with methods
//      and not_measured query/fan-out/recovery fields with reasons.
//
// HONEST BOUNDARIES:
//   - No measured figure is asserted anywhere (the tests pin STRUCTURE and
//     honesty rules, never values — no fabricated numbers in this repo).
//   - NOTHING in this file has been executed: the Phase 5 loop bans
//     builds and test runs. The first
//     `cargo test -p flexcore-sim-runner --test metrics_p5_23` run is the
//     human's gate.

use flexcore_sim_runner::fault_injection::ScenarioFaultPlan;
use flexcore_sim_runner::load_run::{
    percentile_nearest_rank, run_load, LoadMeasurementRecord, LoadResults, LoadRunError, LoadSpec,
    CONSOLE_SCOPE_NOTE, LOAD_BASELINE_LABEL, LOAD_RECORD_SCHEMA, LOAD_RECORD_SUBTASK,
};
use flexcore_sim_runner::metrics::{
    ScenarioRunTimings, METRICS_SCHEMA, METRICS_SUBTASK, TIMINGS_STATEMENT,
};
use flexcore_sim_runner::scenario_run::{
    GridBackendSelection, ScenarioRunHarness, SchedulingGridConfig,
};

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/synthetic-portfolio");

fn read_scenario(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURE_DIR}/{name}"))
        .expect("the pinned scenario fixture exists in the repository")
}

// ---------------------------------------------------------------------------
// 1. The timings block honesty contracts.
// ---------------------------------------------------------------------------

#[test]
fn the_timings_block_records_measured_legs_only_never_zeroes() {
    let mut timings = ScenarioRunTimings::new("test-code", "synthetic-portfolio-exit-gate", "v1", 1);
    // An untimed leg is absent — not a zero, not a placeholder.
    assert!(!timings.has_leg("dispatch_slice"));
    assert_eq!(timings.leg_duration("dispatch_slice"), None);
    let json = timings.to_json().unwrap();
    assert!(!json.contains("dispatch_slice"));

    // A recorded leg appears with its method; a duplicate is a named
    // error; a non-finite figure is refused and leaves nothing behind.
    timings.record("dispatch_slice", 0.125).unwrap();
    assert!(timings
        .record("dispatch_slice", 0.5)
        .is_err());
    assert!(timings.record("horizon_sweep", f64::NAN).is_err());
    assert!(!timings.has_leg("horizon_sweep"));
    assert_eq!(timings.leg_duration("dispatch_slice"), Some(0.125));
    let value: serde_json::Value = serde_json::from_str(&timings.to_json().unwrap()).unwrap();
    assert_eq!(value["record_schema"], METRICS_SCHEMA);
    assert_eq!(value["subtask"], METRICS_SUBTASK);
    assert_eq!(value["legs"].as_array().unwrap().len(), 1);
    assert!(TIMINGS_STATEMENT.contains("never a target"));
    assert!(TIMINGS_STATEMENT.contains("MEASUREMENTS"));
}

#[test]
fn the_run_record_serializes_the_additive_fields_only_when_attached() {
    let scenario_json = read_scenario("scenario-v1.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the scenario composes");
    harness.run_to_horizon_end().expect("the horizon completes");
    let bare = harness
        .record("test-code", Vec::new(), Vec::new(), Vec::new())
        .expect("the record builds");

    // Without attachments the additive fields are OMITTED (old consumers
    // unaffected; no empty-structure noise).
    let bare_json = serde_json::to_string(&bare).unwrap();
    assert!(!bare_json.contains("fault_journal"));
    assert!(!bare_json.contains("timings"));

    // With attachments both fields render.
    let plan = ScenarioFaultPlan::from_scenario_json(&scenario_json).unwrap();
    let journal = flexcore_sim_runner::fault_injection::build_fault_journal(&plan, &bare, &[]);
    let mut timings = ScenarioRunTimings::new("test-code", "s", "v1", 1);
    timings.record("record_build", 0.001).unwrap();
    let full = bare
        .with_fault_journal(journal)
        .with_timings(timings);
    let value: serde_json::Value = serde_json::from_str(&serde_json::to_string(&full).unwrap()).unwrap();
    let journal_json = value.get("fault_journal").expect("the journal renders");
    assert!(journal_json.get("entries").is_some());
    assert!(journal_json.get("statement").is_some());
    assert!(journal_json.get("observed_count").is_some());
    let timings_json = value.get("timings").expect("the timings render");
    assert_eq!(timings_json["record_schema"], METRICS_SCHEMA);
    assert_eq!(timings_json["legs"][0]["leg"], "record_build");
    // No fabricated measured figures anywhere in the timings.
    for leg in timings_json["legs"].as_array().unwrap() {
        let duration = leg["duration_seconds"].as_f64().unwrap();
        assert!(duration.is_finite() && duration >= 0.0);
    }
}

// ---------------------------------------------------------------------------
// 2. The load-record honesty contracts (schema, scope notes, reasons).
// ---------------------------------------------------------------------------

#[test]
fn the_load_record_schema_carries_the_honesty_contracts() {
    // The constants the record is built from — the template doc renders
    // these, so a drift breaks the test.
    assert_eq!(LOAD_RECORD_SCHEMA, "flexcore-load-measurement-record/v1");
    assert_eq!(LOAD_RECORD_SUBTASK, "P5.24");
    assert!(LOAD_BASELINE_LABEL.contains("not a target"));
    assert!(LOAD_BASELINE_LABEL.contains("report §19.4"));
    assert!(CONSOLE_SCOPE_NOTE.contains("NOT measured"));
    assert!(CONSOLE_SCOPE_NOTE.contains("browser"));
    assert!(CONSOLE_SCOPE_NOTE.contains("24_load_measurement.md"));

    // The struct's field inventory (the record schema is additive-only;
    // removing a field breaks this test on purpose).
    let results_json = serde_json::to_string(&LoadResults {
        telemetry_ingest_rate:
            flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        telemetry_ingest_iteration_p95_ms:
            flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        telemetry_ingest_iteration_max_ms:
            flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        query_latency_p50_ms: flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        query_latency_p95_ms: flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        query_latency_p99_ms: flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        command_fan_out_time: flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
        recovery_time_after_restart:
            flexcore_sim_runner::benchmark::MeasuredField::not_measured("r"),
    })
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&results_json).unwrap();
    for field in [
        "telemetry_ingest_rate",
        "telemetry_ingest_iteration_p95_ms",
        "telemetry_ingest_iteration_max_ms",
        "query_latency_p50_ms",
        "query_latency_p95_ms",
        "query_latency_p99_ms",
        "command_fan_out_time",
        "recovery_time_after_restart",
    ] {
        assert!(value.get(field).is_some(), "required field {field} missing");
        let field_value = value.get(field).unwrap();
        assert_eq!(field_value["status"], "not_measured");
        // A not-measured field NEVER carries a numeric value.
        assert!(
            field_value.get("value").is_none(),
            "{field} must not carry a value"
        );
        assert!(!field_value["reason"].as_str().unwrap().is_empty());
    }
}

// ---------------------------------------------------------------------------
// 3. The load spec validator fails loud.
// ---------------------------------------------------------------------------

#[test]
fn the_load_spec_validator_fails_loud() {
    let nothing = LoadSpec {
        ingest_scenario: None,
        ingest_iterations: 0,
        query_url: None,
        query_count: 0,
    };
    let error = nothing.validate().expect_err("an empty load is an error");
    assert!(matches!(error, LoadRunError::Config(_)));
    assert!(error.to_string().contains("measures nothing"));

    let zero_iterations = LoadSpec {
        ingest_scenario: Some("scenario-v1.json".to_string()),
        ingest_iterations: 0,
        query_url: None,
        query_count: 0,
    };
    assert!(zero_iterations.validate().is_err());

    let https = LoadSpec {
        ingest_scenario: None,
        ingest_iterations: 0,
        query_url: Some("https://control-plane/api/v1/tenants".to_string()),
        query_count: 5,
    };
    let error = https.validate().expect_err("https is a named error");
    assert!(error.to_string().contains("http://"));

    let ok = LoadSpec {
        ingest_scenario: Some("scenario-v1.json".to_string()),
        ingest_iterations: 2,
        query_url: Some("http://127.0.0.1:9/api/v1/tenants".to_string()),
        query_count: 1,
    };
    assert!(ok.validate().is_ok());
}

// ---------------------------------------------------------------------------
// 4. The nearest-rank percentile method.
// ---------------------------------------------------------------------------

#[test]
fn the_nearest_rank_percentile_is_reproducible() {
    let samples = vec![4.0, 1.0, 3.0, 2.0, 5.0];
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(percentile_nearest_rank(&sorted, 0.5).unwrap(), 3.0);
    assert_eq!(percentile_nearest_rank(&sorted, 0.95).unwrap(), 5.0);
    assert_eq!(percentile_nearest_rank(&sorted, 0.99).unwrap(), 5.0);
    assert_eq!(percentile_nearest_rank(&sorted, 0.2).unwrap(), 1.0);
    assert!(percentile_nearest_rank(&[], 0.5).is_err());
    assert!(percentile_nearest_rank(&sorted, 0.0).is_err());
    assert!(percentile_nearest_rank(&sorted, 1.01).is_err());
}

// ---------------------------------------------------------------------------
// 5. A produced record (ingest leg only; no network in this suite).
// ---------------------------------------------------------------------------

#[test]
fn a_produced_ingest_only_record_carries_measured_and_not_measured_honestly() {
    let spec = LoadSpec {
        ingest_scenario: Some(format!("{FIXTURE_DIR}/scenario-v1.json")),
        ingest_iterations: 2,
        query_url: None,
        query_count: 0,
    };
    let record: LoadMeasurementRecord = run_load(&spec, "test-code")
        .expect("the ingest-only load runs offline against the pinned scenario");

    // The conditions travel with every figure (ledger P5.24 step 3).
    assert_eq!(record.conditions.ingest_scenario_id.as_deref(), Some("synthetic-portfolio-exit-gate"));
    assert_eq!(record.conditions.ingest_iterations, 2);
    assert!(record.conditions.ingest_records_total > 0);
    assert!(record.conditions.concurrency.contains("single sequential client"));
    assert_eq!(record.conditions.query_url, None);
    assert_eq!(record.conditions.query_count, 0);

    // The measured ingest figures carry their methods; the query leg is
    // honestly not measured (it was not configured).
    let results_json = serde_json::to_string(&record.results).unwrap();
    let value: serde_json::Value = serde_json::from_str(&results_json).unwrap();
    assert_eq!(value["telemetry_ingest_rate"]["status"], "measured");
    assert!(value["telemetry_ingest_rate"]["method"]
        .as_str()
        .unwrap()
        .contains("total canonical records"));
    for field in [
        "query_latency_p50_ms",
        "query_latency_p95_ms",
        "query_latency_p99_ms",
    ] {
        assert_eq!(value[field]["status"], "not_measured");
    }
    // The two 0.3.0 gaps are named, never zero-filled.
    assert_eq!(value["command_fan_out_time"]["status"], "not_measured");
    assert!(value["command_fan_out_time"]["reason"]
        .as_str()
        .unwrap()
        .contains("24_load_measurement.md"));
    assert_eq!(value["recovery_time_after_restart"]["status"], "not_measured");

    // The scope notes and manual procedures travel on the record.
    assert!(record.scope_notes.iter().any(|note| note.contains("NOT measured")));
    assert!(record
        .manual_procedures
        .iter()
        .any(|procedure| procedure.contains("browser")));
    assert!(record
        .manual_procedures
        .iter()
        .any(|procedure| procedure.contains("recovery")));

    // The hardware block is the P3.27 capture (comparable conditions).
    assert!(!record.hardware.os.is_empty());
    assert!(record.hardware.capture_note.contains("unavailable"));

    // The record identifies itself.
    let full_json = record.to_json().unwrap();
    assert!(full_json.contains(LOAD_RECORD_SCHEMA));
    assert!(full_json.contains(LOAD_BASELINE_LABEL));
    assert!(full_json.contains("P5.24"));
}
