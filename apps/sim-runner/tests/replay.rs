// P2.24 — Deterministic-replay tests (pure — no database, no broker): the
// ledger P2.24 acceptance, driven through the same library surface the CLI
// composes (`main.rs` is a thin argument shell over this).
//
// What is asserted (ledger P2.24 steps 3–5):
//
// 1. Replay of the exit-gate scenario passes with ZERO mismatches: two
//    independent runs of the versioned scenario (same version + seed +
//    code) compare equal on the frozen tuple.
// 2. A tampered scenario (changed seed) fails WITH a diff: the mismatch
//    list names the records that diverged (per-record diffs, both sides).
// 3. A generation-altering code change fails honestly: the test simulates
//    the change by mutating one record's value between the reference and
//    the fresh run — the replay BREAKS and reports it. This documents the
//    recorded P2.13 §4 rule: a generation change breaks replay, expected,
//    never hidden.
// 4. `ingest_time` is EXCLUDED from the comparison (P2.13 §3): two runs
//    whose decoded records carry DIFFERENT ingest times still match — and
//    the replay record type structurally cannot carry ingest_time at all.
// 5. The report shape: scenario version, seed, code version, record
//    counts, match count, mismatch list with diffs, verdict — all present
//    in the JSON rendering.
// 6. The exit-code contract is fail-closed: pass → 0, mismatch → 1,
//    unknown verdict → 2.
//
// The scenario is loaded from the versioned artefact itself
// (`scenarios/synthetic-portfolio/scenario-v1.json`) so the tests cannot
// drift from the exit-gate scenario. Everything here is simulation-only
// synthetic telemetry (0.x line); no safety or control-authority claim.

use flexcore_ports::telemetry_source::{NormalizedTelemetry, NormalizedTelemetryParts};
use flexcore_sim_runner::compare::{compare, DiffField};
use flexcore_sim_runner::replay::{
    collect_normalized, record_from_normalized, records_from_json, records_to_json, run_scenario,
    scenario_json_with_seed, ReplayValue,
};
use flexcore_sim_runner::report::{
    exit_code_for_verdict, ReplayReport, VERDICT_FAIL, VERDICT_PASS,
};

/// The versioned exit-gate scenario, loaded from the artefact itself.
const SCENARIO_V1: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v1.json"
));

const CODE_VERSION: &str = "replay-test-code";

// --- 1. The exit-gate replay passes with zero mismatches ----------------------

#[test]
fn replay_of_the_exit_gate_scenario_passes_with_zero_mismatches() {
    let reference = run_scenario(SCENARIO_V1).expect("the exit-gate scenario runs");
    assert_eq!(reference.scenario_id, "synthetic-portfolio-exit-gate");
    assert_eq!(reference.scenario_version, "v1");
    assert_eq!(reference.master_seed, 20260601);
    assert!(
        !reference.records.is_empty(),
        "a 24 h two-site horizon produces records"
    );

    let candidate = run_scenario(SCENARIO_V1).expect("the second run executes");
    let comparison = compare(&reference.records, &candidate.records);
    assert!(
        comparison.is_pass(),
        "two runs of the same scenario version + seed + code must replay identically; \
         first diffs: {:?}",
        comparison.mismatches.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(comparison.match_count, reference.records.len());
    assert_eq!(comparison.mismatches.len(), 0);
}

#[test]
fn the_reference_file_round_trip_preserves_replay_equality() {
    // The CLI compares against a records FILE; the file form must be
    // faithful (value round-trips exactly, no ingest_time field exists).
    let reference = run_scenario(SCENARIO_V1).expect("the scenario runs");
    let text = records_to_json(&reference.records).expect("records serialize");
    let from_file = records_from_json(&text).expect("records parse back");
    let candidate = run_scenario(SCENARIO_V1).expect("the second run executes");
    let comparison = compare(&from_file, &candidate.records);
    assert!(comparison.is_pass());
    assert!(!text.contains("ingest_time"));
}

// --- 2. A tampered seed fails with a diff --------------------------------------

#[test]
fn a_tampered_seed_fails_with_a_per_record_diff() {
    let reference = run_scenario(SCENARIO_V1).expect("the reference run executes");
    let tampered = scenario_json_with_seed(SCENARIO_V1, 20260602)
        .expect("the seed override re-renders the scenario");
    let candidate = run_scenario(&tampered).expect("the tampered run executes");
    assert_eq!(candidate.master_seed, 20260602, "the tampered seed took effect");

    let comparison = compare(&reference.records, &candidate.records);
    assert!(!comparison.is_pass(), "a changed seed must NOT replay identically");
    assert!(
        !comparison.mismatches.is_empty(),
        "the failure carries a mismatch list"
    );
    // The diffs name the records: both sides travel, the asset and the
    // event time are real, and the diverging field is identified.
    for diff in comparison.mismatches.iter().take(10) {
        assert!(!diff.asset_id.is_empty());
        assert!(diff.event_time_unix_ms > 0);
        assert!(!diff.quantity.is_empty());
        assert!(
            diff.reference.is_some() || diff.candidate.is_some(),
            "a diff carries at least the record it describes"
        );
    }
    assert!(
        comparison
            .mismatches
            .iter()
            .any(|d| d.field == DiffField::Value),
        "a seeded jitter change surfaces as value diffs: {:?}",
        comparison.mismatches.iter().take(3).collect::<Vec<_>>()
    );
}

// --- 3. A generation-altering change fails honestly ----------------------------

#[test]
fn a_generation_altering_change_breaks_replay_and_reports_it() {
    // Recorded rule (P2.13 §4): the code version pins the generation
    // arithmetic; a generation change breaks replay — expected, recorded,
    // NOT a bug to hide. The change is simulated at the record boundary:
    // one value is altered between the reference and the fresh run, which
    // is exactly what an arithmetic change produces.
    let reference = run_scenario(SCENARIO_V1).expect("the reference run executes");
    let mut candidate = run_scenario(SCENARIO_V1).expect("the fresh run executes");
    let index = candidate
        .records
        .iter()
        .position(|r| matches!(r.value, ReplayValue::Power { .. }))
        .expect("the horizon carries active-power records");
    let original = candidate.records[index].clone();
    candidate.records[index].value = match &original.value {
        ReplayValue::Power { mw } => ReplayValue::Power { mw: mw + 0.125 },
        other => other.clone(),
    };
    assert_ne!(
        candidate.records[index].value, original.value,
        "the simulated generation change altered one record"
    );

    let comparison = compare(&reference.records, &candidate.records);
    assert!(
        !comparison.is_pass(),
        "a generation change must break replay — the replay fails honestly"
    );
    let diff = comparison
        .mismatches
        .iter()
        .find(|d| d.field == DiffField::Value)
        .expect("the altered record appears as a value diff");
    assert_eq!(diff.asset_id, original.asset_id);
    assert_eq!(diff.event_time_unix_ms, original.event_time_unix_ms);
    assert_eq!(diff.quantity, original.quantity);

    // The report states the failure as a verdict (the P2.27 gate asserts
    // the verdict, not just the exit code).
    let report = ReplayReport::build(
        &reference.scenario_id,
        &reference.scenario_version,
        reference.master_seed,
        CODE_VERSION,
        comparison,
    );
    assert_eq!(report.verdict, VERDICT_FAIL);
    assert_eq!(exit_code_for_verdict(&report.verdict), 1);
}

// --- 4. ingest_time is excluded from the comparison -----------------------------

#[test]
fn two_runs_with_different_ingest_times_still_match() {
    // The dedicated P2.13 §3 proof: the decoded records of two runs are
    // rebuilt so the second run's ingest times differ — the comparison
    // must still pass. (`NormalizedTelemetry` is the full canonical record;
    // `ReplayRecord` is the projection that structurally drops ingest_time.)
    let first = collect_normalized(SCENARIO_V1).expect("the first run decodes");
    let second = collect_normalized(SCENARIO_V1).expect("the second run decodes");
    assert_eq!(first.len(), second.len());
    assert!(!first.is_empty());

    // REBUILD the second run's records with a large ingest-time shift
    // (+37 h): the runs now genuinely disagree on ingest_time.
    let shift_millis: i128 = 37 * 3_600 * 1_000;
    let mut shifted = 0usize;
    let second: Vec<NormalizedTelemetry> = second
        .into_iter()
        .map(|record| {
            let shifted_ingest = flexcore_contracts::time_model::IngestTime::from_unix_millis(
                (record.ingest_time().unix_millis() + shift_millis) as i64,
            )
            .expect("the shifted ingest time is a valid instant");
            shifted += 1;
            NormalizedTelemetry::new(NormalizedTelemetryParts {
                tenant_id: record.tenant_id(),
                asset_id: record.asset_id(),
                quantity: record.quantity(),
                value: record.value().cloned(),
                event_time: record.event_time(),
                ingest_time: shifted_ingest,
                source_sequence: record.source_sequence(),
                source_id: record.source_id().to_string(),
                quality: record.quality(),
                source_quality_code: record.source_quality_code().map(str::to_string),
                clock_quality: record.clock_quality(),
            })
            .expect("an ingest-time shift cannot invalidate a canonical record")
        })
        .collect();
    assert_eq!(shifted, first.len(), "every record was rebuilt");

    // The fault condition held: the two runs genuinely differ on ingest_time.
    for (a, b) in first.iter().zip(second.iter()) {
        assert_ne!(
            a.ingest_time(),
            b.ingest_time(),
            "the test must produce runs with different ingest times"
        );
    }

    // Yet the replay comparison passes: ingest_time is never compared.
    let reference: Vec<_> = first.iter().map(record_from_normalized).collect();
    let candidate: Vec<_> = second.iter().map(record_from_normalized).collect();
    let comparison = compare(&reference, &candidate);
    assert!(
        comparison.is_pass(),
        "ingest_time must not affect the replay comparison; diffs: {:?}",
        comparison.mismatches.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(comparison.match_count, reference.len());
}

// --- 5. The report shape --------------------------------------------------------

#[test]
fn the_replay_report_carries_every_required_field_and_verdict() {
    let run = run_scenario(SCENARIO_V1).expect("the scenario runs");
    let comparison = compare(&run.records, &run.records);
    let report = ReplayReport::build(
        &run.scenario_id,
        &run.scenario_version,
        run.master_seed,
        CODE_VERSION,
        comparison,
    );

    // Rust-side shape.
    assert_eq!(report.scenario_id, "synthetic-portfolio-exit-gate");
    assert_eq!(report.scenario_version, "v1");
    assert_eq!(report.seed, 20260601);
    assert_eq!(report.code_version, CODE_VERSION);
    assert_eq!(report.reference_count, run.records.len());
    assert_eq!(report.candidate_count, run.records.len());
    assert_eq!(report.match_count, run.records.len());
    assert_eq!(report.mismatch_count, 0);
    assert_eq!(report.verdict, VERDICT_PASS);
    assert!(report.mismatches.is_empty());

    // JSON-side shape: every required field is present with the right name.
    let text = report.to_json().expect("the report serializes");
    let value: serde_json::Value = serde_json::from_str(&text).expect("the report is valid JSON");
    for field in [
        "scenario_id",
        "scenario_version",
        "seed",
        "code_version",
        "reference_count",
        "candidate_count",
        "match_count",
        "mismatch_count",
        "verdict",
        "mismatches",
    ] {
        assert!(value.get(field).is_some(), "required field '{field}' missing");
    }
    assert_eq!(value["verdict"], "pass");
    assert_eq!(value["seed"], 20260601);

    // The human summary states the verdict, the identity triple, and counts.
    let summary = report.human_summary();
    assert!(summary.contains("PASS"));
    assert!(summary.contains("synthetic-portfolio-exit-gate:v1"));
    assert!(summary.contains("20260601"));
    assert!(summary.contains(CODE_VERSION));
    assert!(summary.contains("no mismatches"));
}

#[test]
fn a_failing_report_lists_the_mismatches_with_diffs() {
    let run = run_scenario(SCENARIO_V1).expect("the scenario runs");
    let mut candidate = run.clone();
    let last = candidate.records.len() - 1;
    candidate.records[last].quality = "SUSPECT".to_string();
    let comparison = compare(&run.records, &candidate.records);
    let report = ReplayReport::build(
        &run.scenario_id,
        &run.scenario_version,
        run.master_seed,
        CODE_VERSION,
        comparison,
    );
    assert_eq!(report.verdict, VERDICT_FAIL);
    assert_eq!(report.mismatch_count, 1);
    let text = report.to_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["mismatches"].as_array().unwrap().len(), 1);
    assert_eq!(value["mismatches"][0]["field"], "quality");
    assert!(value["mismatches"][0]["reference"].is_object());
    assert!(value["mismatches"][0]["candidate"].is_object());
}

// --- 6. The exit-code contract ---------------------------------------------------

#[test]
fn the_exit_code_contract_is_pass_zero_mismatch_one_usage_two() {
    assert_eq!(exit_code_for_verdict(VERDICT_PASS), 0);
    assert_eq!(exit_code_for_verdict(VERDICT_FAIL), 1);
    assert_eq!(exit_code_for_verdict("anything-else"), 2);
    assert_eq!(exit_code_for_verdict(""), 2, "an empty verdict fails closed");
}

// --- Canonical-value sanity on the replay output ---------------------------------

#[test]
fn the_replay_records_are_canonical_units_and_six_state_vocabulary() {
    let run = run_scenario(SCENARIO_V1).expect("the scenario runs");
    let mut qualities_seen = std::collections::BTreeSet::new();
    for record in &run.records {
        match &record.value {
            flexcore_sim_runner::replay::ReplayValue::Power { mw } => {
                assert!(mw.is_finite(), "a canonical power is finite");
            }
            flexcore_sim_runner::replay::ReplayValue::Fraction { value } => {
                assert!(
                    (0.0..=1.0).contains(value),
                    "SOC is a fraction in [0,1] (P0.07 §4), got {value}"
                );
            }
            flexcore_sim_runner::replay::ReplayValue::Reactive { mvar, convention } => {
                assert!(
                    convention == "INJECTION_POSITIVE" || convention == "CONSUMPTION_POSITIVE",
                    "the declared convention travels with the reactive value (P0.07 §3)"
                );
                assert!(mvar.is_finite());
            }
            _ => {}
        }
        assert!(!record.asset_id.is_empty());
        assert!(record.source_sequence >= 1);
        qualities_seen.insert(record.quality.clone());
    }
    // The v1 exit-gate run (fault plan disabled) is all-GOOD (P2.13 §7).
    assert_eq!(
        qualities_seen,
        std::collections::BTreeSet::from(["GOOD".to_string()]),
        "the disabled-plan v1 run emits only GOOD records"
    );
    // Quantities stay in the frozen vocabulary.
    let quantities: std::collections::BTreeSet<&str> = run
        .records
        .iter()
        .map(|r| r.quantity.as_str())
        .collect();
    for quantity in quantities {
        assert!(
            flexcore_ports::telemetry_read::parse_quantity_key(quantity).is_some(),
            "quantity '{quantity}' must be a frozen Phase 2 quantity"
        );
    }
}
