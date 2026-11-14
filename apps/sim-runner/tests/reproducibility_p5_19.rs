// P5.19 — The replay reproducibility verification suite (report §24 step
// 10, §19.2 layer 5; ledger P5.19).
//
// WHAT IS TESTED (pure comparison logic + the record pins; NO run is
// executed here — the double-run PROOF is the human's pre-registered run,
// never executed by the authoring loop):
//   1. The pre-registered tolerance table exists, is stated per output
//      type, and was registered BEFORE any run (the anti-fitting rule):
//      byte-identical for the scripted deterministic legs; relative 1e-9
//      (floor 1e-12, DEFAULT) for floating-point legs; runtimes/conditions
//      NEVER compared as correctness.
//   2. The checker verdicts: identical ⇒ pass; a deviation within the
//      tolerance ⇒ `within_tolerance` (recorded, never silent); a
//      deviation outside the tolerance (the negative control's shape) ⇒
//      `mismatch` with a NAMED diff (asset, time, quantity, values).
//   3. The nondeterminism-source inventory: wall clock, HashMap iteration
//      order, thread count (the P3.26 solver pin thread_count = 1,
//      CITED), ambient entropy, the floating-point backend, and the
//      opt-in HELICS leg — each listed WITH its neutralization.
//   4. The library records carry the pins: every record pins
//      thread_count = 1, the tolerances, and the nondeterminism
//      inventory — the pre-registration is per record, not tribal
//      knowledge.
//
// HONEST BOUNDARY: the actual double-run execution (the proof) and the
// negative control's execution are UNRUN. The commands are pre-registered
// in `ledger/phase-05-operator-console-simulator/18_20_replay_library.md`
// and run by `flexcore-sim-runner replay-run --record <path> --double-run`
// / `--baseline <path>` at the human build/test pass. Nothing here claims
// a measured verdict.

use flexcore_sim_runner::compare::DiffField;
use flexcore_sim_runner::replay::{ReplayRecord, ReplayValue};
use flexcore_sim_runner::reproducibility::{
    check_reproducibility, compare_records_within_tolerance, values_within_tolerance,
    DEFAULT_TOLERANCE_ENTRIES, DEFAULT_VALUE_ABSOLUTE_FLOOR, DEFAULT_VALUE_RELATIVE_TOLERANCE,
    NONDETERMINISM_CONTROLS, ToleranceTable, VERDICT_IDENTICAL, VERDICT_MISMATCH,
    VERDICT_WITHIN_TOLERANCE,
};

const LIBRARY_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/library");

fn telemetry_record(seq: u64, mw: f64) -> ReplayRecord {
    ReplayRecord {
        asset_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c".to_string(),
        event_time_unix_ms: 1_780_272_000_000,
        quantity: "active_power".to_string(),
        value: ReplayValue::Power { mw },
        quality: "GOOD".to_string(),
        source_sequence: seq,
    }
}

// --- 1. The pre-registered tolerance table ---------------------------------------

#[test]
fn the_tolerance_table_is_stated_per_output_type_before_the_runs() {
    let table = ToleranceTable::default_table();
    assert!(table.entries.len() >= 5, "the table covers every output class");
    // The scripted deterministic legs are byte-identical — exact f64
    // equality, no tolerance (P2.13 §3: anything else IS a replay break).
    assert_eq!(table.entries[0].rule, "byte_identical");
    assert!(table.entries[0].relative.is_none());
    assert_eq!(table.entries[1].rule, "byte_identical");
    // The floating-point legs carry the DEFAULT relative 1e-9 with the
    // absolute floor.
    assert_eq!(
        table.entries[2].relative,
        Some(DEFAULT_VALUE_RELATIVE_TOLERANCE)
    );
    assert_eq!(
        table.entries[2].absolute_floor,
        Some(DEFAULT_VALUE_ABSOLUTE_FLOOR)
    );
    assert_eq!(DEFAULT_VALUE_RELATIVE_TOLERANCE, 1e-9);
    assert_eq!(DEFAULT_VALUE_ABSOLUTE_FLOOR, 1e-12);
    // Conditions are never compared as correctness (P2.13 §3 ingest_time;
    // P4.35 conditions-travel rule).
    assert!(DEFAULT_TOLERANCE_ENTRIES
        .iter()
        .any(|entry| entry.rule.starts_with("never_compared")));
}

// --- 2. The checker verdicts -------------------------------------------------------

#[test]
fn identical_record_sets_verdict_identical() {
    let reference = vec![telemetry_record(1, 1.0), telemetry_record(2, -0.5)];
    let candidate = reference.clone();
    let comparison = compare_records_within_tolerance(
        &reference,
        &candidate,
        DEFAULT_VALUE_RELATIVE_TOLERANCE,
        DEFAULT_VALUE_ABSOLUTE_FLOOR,
    );
    assert!(comparison.is_pass());
    assert_eq!(comparison.exact_matches, 2);
    assert_eq!(comparison.within_tolerance_matches, 0);
    let verdict = check_reproducibility(
        &[("telemetry", 0, 0, Vec::new())],
        "byte_identical (scripted)",
    );
    assert_eq!(verdict.verdict, VERDICT_IDENTICAL);
}

#[test]
fn a_deviation_within_the_tolerance_is_recorded_not_silent() {
    // 1e-12 relative deviation: inside the DEFAULT 1e-9 bound.
    let reference = vec![telemetry_record(1, 100.0)];
    let candidate = vec![telemetry_record(1, 100.0000000001)];
    let comparison = compare_records_within_tolerance(
        &reference,
        &candidate,
        DEFAULT_VALUE_RELATIVE_TOLERANCE,
        DEFAULT_VALUE_ABSOLUTE_FLOOR,
    );
    assert!(comparison.is_pass());
    assert_eq!(comparison.exact_matches, 0);
    assert_eq!(comparison.within_tolerance_matches, 1);
    // The deviation is NAMED even though it passes (never silent).
    let verdict = check_reproducibility(
        &[(
            "telemetry",
            0,
            1,
            vec![format!(
                "{} seq 1: reference 100 vs candidate 100.0000000001",
                comparison.mismatches[0].asset_id
            )],
        )],
        "relative 1e-9",
    );
    assert_eq!(verdict.verdict, VERDICT_WITHIN_TOLERANCE);
    assert!(verdict.diffs[0].contains(&reference[0].asset_id));
}

#[test]
fn a_deviation_outside_the_tolerance_is_a_named_mismatch_the_negative_control_shape() {
    // 1e-6 relative deviation: OUTSIDE the DEFAULT 1e-9 bound — the
    // negative-control territory (a changed input must never silently
    // reproduce the old result).
    let reference = vec![telemetry_record(1, 100.0), telemetry_record(2, 2.0)];
    let candidate = vec![telemetry_record(1, 100.001), telemetry_record(2, 2.0)];
    let comparison = compare_records_within_tolerance(
        &reference,
        &candidate,
        DEFAULT_VALUE_RELATIVE_TOLERANCE,
        DEFAULT_VALUE_ABSOLUTE_FLOOR,
    );
    assert!(!comparison.is_pass());
    assert_eq!(comparison.mismatches.len(), 1);
    let diff = &comparison.mismatches[0];
    assert!(diff.outside_tolerance);
    assert_eq!(diff.source_sequence, 1);
    assert_eq!(diff.quantity, "active_power");
    let verdict = check_reproducibility(
        &[(
            "telemetry",
            1,
            0,
            vec![format!("{} seq 1: value outside tolerance", diff.asset_id)],
        )],
        "relative 1e-9",
    );
    assert_eq!(verdict.verdict, VERDICT_MISMATCH);
    assert!(verdict.statement.contains("NOT reproducible"));
}

#[test]
fn a_quality_difference_is_never_tolerance_bounded() {
    let mut candidate = vec![telemetry_record(1, 1.0)];
    candidate[0].quality = "SUSPECT".to_string();
    let reference = vec![telemetry_record(1, 1.0)];
    let comparison = compare_records_within_tolerance(
        &reference,
        &candidate,
        DEFAULT_VALUE_RELATIVE_TOLERANCE,
        DEFAULT_VALUE_ABSOLUTE_FLOOR,
    );
    assert!(!comparison.is_pass());
    assert!(comparison.mismatches[0]
        .deviation
        .contains("never tolerance-bounded"));
}

#[test]
fn a_one_sided_record_is_a_mismatch_never_absorbed() {
    let reference = vec![telemetry_record(1, 1.0), telemetry_record(2, 2.0)];
    let candidate = vec![telemetry_record(1, 1.0)];
    let comparison = compare_records_within_tolerance(
        &reference,
        &candidate,
        DEFAULT_VALUE_RELATIVE_TOLERANCE,
        DEFAULT_VALUE_ABSOLUTE_FLOOR,
    );
    assert!(!comparison.is_pass());
    assert!(comparison.mismatches[0].deviation.contains("one side only"));
}

#[test]
fn the_tolerance_arithmetic_is_the_registered_rule() {
    // Exact equality passes.
    assert!(values_within_tolerance(1.0, 1.0, 1e-9, 1e-12));
    // Floor-scale differences pass at any magnitude.
    assert!(values_within_tolerance(100.0, 100.0 + 1e-13, 1e-9, 1e-12));
    // Inside 1e-9 relative passes.
    assert!(values_within_tolerance(1.0, 1.0 + 5e-10, 1e-9, 1e-12));
    // Outside 1e-9 relative fails — the negative control's bound.
    assert!(!values_within_tolerance(1.0, 1.0 + 2e-9, 1e-9, 1e-12));
    // Near-zero: the floor decides, and it is small enough to catch real
    // deviations.
    assert!(values_within_tolerance(0.0, 1e-13, 1e-9, 1e-12));
    assert!(!values_within_tolerance(0.0, 1e-9, 1e-9, 1e-12));
    // Non-finite values never compare equal unless bit-identical.
    assert!(!values_within_tolerance(f64::NAN, f64::NAN, 1e-9, 1e-12));
    assert!(!values_within_tolerance(f64::INFINITY, 1e9, 1e-9, 1e-12));
}

// --- 3. The nondeterminism inventory ----------------------------------------------

#[test]
fn every_named_nondeterminism_source_is_listed_and_neutralized() {
    let sources: Vec<&str> = NONDETERMINISM_CONTROLS
        .iter()
        .map(|control| control.source)
        .collect();
    // The ledger names wall-clock, HashMap iteration order, and thread
    // count explicitly.
    for required in ["wall_clock", "hash_map_iteration_order", "thread_count"] {
        assert!(
            sources.contains(&required),
            "the inventory must name '{required}'"
        );
    }
    // Every entry carries a real neutralization statement.
    for control in NONDETERMINISM_CONTROLS {
        assert!(
            control.neutralization.len() > 40,
            "source '{}' needs a real neutralization, not a token phrase",
            control.source
        );
    }
}

#[test]
fn the_thread_count_control_cites_the_existing_solver_pin() {
    let thread = NONDETERMINISM_CONTROLS
        .iter()
        .find(|control| control.source == "thread_count")
        .expect("thread_count is listed");
    // The pin ALREADY exists (P3.26, scenarios-phase3); this module cites
    // it — it does not re-pin or re-derive it.
    assert!(thread.neutralization.contains("thread_count = 1"));
    assert!(thread.neutralization.contains("scenarios-phase3"));
}

// --- 4. The library records carry the pre-registration ------------------------------

#[test]
fn every_library_record_pins_thread_count_one_and_the_tolerance_table() {
    let entries = std::fs::read_dir(LIBRARY_DIR)
        .expect("the library directory exists")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()) == Some("json")
        })
        .collect::<Vec<_>>();
    assert!(entries.len() >= 8, "the library carries the eight cases");
    for entry in entries {
        let text = std::fs::read_to_string(entry.path()).expect("readable");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let name = entry.file_name().to_string_lossy().to_string();
        assert_eq!(
            value["nondeterminism_controls"]["thread_count"], 1,
            "{name} must pin thread_count = 1"
        );
        assert!(
            value["nondeterminism_controls"]["controls"].as_array().expect("controls").len() >= 4,
            "{name} must carry the nondeterminism inventory"
        );
        assert_eq!(
            value["tolerances"]["value_relative_tolerance"], 1e-9,
            "{name} must pin the DEFAULT relative tolerance"
        );
        assert_eq!(
            value["tolerances"]["value_absolute_floor"], 1e-12,
            "{name} must pin the absolute floor"
        );
        assert!(
            !value["tolerances"]["table_reference"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "{name} must reference the pre-registered tolerance table"
        );
    }
}

// --- The DiffField import guard (the comparison fields are the frozen
// --- tuple's diff vocabulary; a drift there is a P5.19-visible break) ---

#[test]
fn the_comparison_diff_vocabulary_is_the_frozen_one() {
    // The four diff fields the P2.24 comparison freezes are the same four
    // the P5.19 tolerance comparison reuses — a new field would be a
    // comparison-vocabulary change, not a silent addition.
    let fields = [
        DiffField::Value,
        DiffField::Quality,
        DiffField::MissingInCandidate,
        DiffField::MissingInReference,
    ];
    let rendered: Vec<&str> = fields
        .iter()
        .map(|field| match field {
            DiffField::Value => "value",
            DiffField::Quality => "quality",
            DiffField::MissingInCandidate => "missing_in_candidate",
            DiffField::MissingInReference => "missing_in_reference",
        })
        .collect();
    assert_eq!(rendered.len(), 4);
    assert!(rendered.contains(&"value"));
}
