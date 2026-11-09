// P2.31 — As-of replay tests (pure — no database, no broker): the ledger
// P2.31 step 6 replay acceptances, driven through the same library surface
// the CLI composes (`main.rs` is a thin argument shell over this).
//
// What is asserted (ledger P2.31 acceptance rows):
//
// 1. As-of replay reproduces a past decision's input snapshot from the
//    as-of view: a decision that ran at T1 (before the correction) is
//    REPRODUCED exactly when the snapshot is derived at T1, and BREAKS with
//    a per-record value diff when the snapshot is derived at T2 (after the
//    correction) — the diff names both sides, P2.24 comparison semantics.
// 2. A decision that ran after the correction (its input carried the
//    corrected value) is reproduced at T2 and under the default (no
//    as-of = latest version), and breaks at T1.
// 3. The superseded version never reaches the as-of snapshot at T2, and
//    the corrected version never reaches it at T1 (one row per identity,
//    never both).
// 4. The original version is always visible: an as-of before any
//    correction still shows the original record (the view decides WHICH
//    version was current, never whether a record existed).
// 5. The report shape (as-of instant, counts, verdict, mismatches) and the
//    fail-closed exit-code contract.
//
// Everything here is simulation-only synthetic telemetry (0.x line); no
// safety or control-authority claim.

use flexcore_sim_runner::as_of::{
    derive_as_of_snapshot, exit_code, run_as_of, AsOfReplayReport, VersionedRecord,
};
use flexcore_sim_runner::compare::DiffField;
use flexcore_sim_runner::replay::{records_to_json, ReplayRecord, ReplayValue};
use flexcore_sim_runner::report::{EXIT_MISMATCH, EXIT_PASS, VERDICT_FAIL, VERDICT_PASS};

const ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
const EVENT_MS: i64 = 1_780_272_000_000;
/// T1: the moment the original decision ran (before the correction).
const T1: i64 = 1_000;
/// T2: the correction timestamp (and the later decision's moment).
const T2: i64 = 2_000;

fn record(seq: u64, mw: f64) -> ReplayRecord {
    ReplayRecord {
        asset_id: ASSET.to_string(),
        event_time_unix_ms: EVENT_MS,
        quantity: "active_power".to_string(),
        value: ReplayValue::Power { mw },
        quality: "GOOD".to_string(),
        source_sequence: seq,
    }
}

fn versioned(record: ReplayRecord, corrected_at: Option<i64>) -> VersionedRecord {
    VersionedRecord {
        record,
        corrected_at_unix_ms: corrected_at,
        corrected_by: corrected_at.map(|_| "operator:test".to_string()),
        correction_reason: corrected_at.map(|_| "sensor recalibration".to_string()),
    }
}

/// The versioned history: original 1.0 MW at seq 1, corrected to 2.0 MW at
/// T2. Nothing else.
fn history() -> Vec<VersionedRecord> {
    vec![
        versioned(record(1, 1.0), None),
        versioned(record(1, 2.0), Some(T2)),
    ]
}

// --- 1. The pre-correction decision is reproduced at T1, breaks at T2 -----

#[test]
fn the_pre_correction_decision_is_reproduced_at_t1_and_diffs_at_t2() {
    // What the decision at T1 actually used (exported records file form).
    let decision_input = vec![record(1, 1.0)];
    // The as-of view at T1: the pre-correction value — reproduction.
    let snapshot_t1 = derive_as_of_snapshot(&history(), Some(T1));
    let comparison = flexcore_sim_runner::compare::compare(&decision_input, &snapshot_t1);
    assert!(comparison.is_pass(), "T1 replay must reproduce the decision inputs");
    assert_eq!(comparison.match_count, 1);
    assert_eq!(comparison.mismatches.len(), 0);

    // The same decision re-derived at T2: MISMATCH with a value diff that
    // names both sides (the ledger's "diff on mismatch").
    let snapshot_t2 = derive_as_of_snapshot(&history(), Some(T2));
    let comparison = flexcore_sim_runner::compare::compare(&decision_input, &snapshot_t2);
    assert!(!comparison.is_pass());
    assert_eq!(comparison.mismatches.len(), 1);
    assert_eq!(comparison.mismatches[0].field, DiffField::Value);
    assert_eq!(
        comparison.mismatches[0].reference.as_ref().expect("decision side").value,
        ReplayValue::Power { mw: 1.0 }
    );
    assert_eq!(
        comparison.mismatches[0].candidate.as_ref().expect("snapshot side").value,
        ReplayValue::Power { mw: 2.0 }
    );
}

// --- 2. The post-correction decision is reproduced at T2 / default ---------

#[test]
fn the_post_correction_decision_is_reproduced_at_t2_and_by_default() {
    let decision_input = vec![record(1, 2.0)];
    let snapshot_t2 = derive_as_of_snapshot(&history(), Some(T2));
    assert!(flexcore_sim_runner::compare::compare(&decision_input, &snapshot_t2).is_pass());
    // Default (no as-of) = the latest version view.
    let snapshot_latest = derive_as_of_snapshot(&history(), None);
    assert!(flexcore_sim_runner::compare::compare(&decision_input, &snapshot_latest).is_pass());
    // At T1 the same decision does NOT reproduce (it saw data that did not
    // exist yet).
    let snapshot_t1 = derive_as_of_snapshot(&history(), Some(T1));
    let comparison = flexcore_sim_runner::compare::compare(&decision_input, &snapshot_t1);
    assert!(!comparison.is_pass());
    assert_eq!(comparison.mismatches[0].field, DiffField::Value);
}

// --- 3. One row per identity — never both versions ---------------------------

#[test]
fn the_snapshot_carries_exactly_one_version_per_identity() {
    for as_of in [None, Some(0), Some(T1), Some(T2), Some(i64::MAX)] {
        let snapshot = derive_as_of_snapshot(&history(), as_of);
        assert_eq!(
            snapshot.len(),
            1,
            "as-of {as_of:?}: exactly one version per identity"
        );
    }
    // The superseded value never appears at T2; the corrected value never
    // appears at T1.
    assert_eq!(derive_as_of_snapshot(&history(), Some(T2))[0].value, ReplayValue::Power { mw: 2.0 });
    assert_eq!(derive_as_of_snapshot(&history(), Some(T1))[0].value, ReplayValue::Power { mw: 1.0 });
}

// --- 4. The original is always visible ---------------------------------------

#[test]
fn the_original_is_visible_even_before_any_correction() {
    let snapshot = derive_as_of_snapshot(&history(), Some(0));
    assert_eq!(snapshot, vec![record(1, 1.0)]);
}

// --- 5. Report shape, verdict, exit codes, and the file entrypoint -----------

#[test]
fn the_report_carries_the_shape_and_the_exit_code_contract_is_fail_closed() {
    // Pass: the T1 decision against the T1 view.
    let pass = AsOfReplayReport::build(
        Some(T1),
        history().len(),
        flexcore_sim_runner::compare::compare(
            &[record(1, 1.0)],
            &derive_as_of_snapshot(&history(), Some(T1)),
        ),
    );
    assert_eq!(pass.verdict, VERDICT_PASS);
    assert_eq!(pass.as_of_unix_ms, Some(T1));
    assert_eq!(pass.versions_count, 2);
    assert_eq!(pass.snapshot_count, 1);
    assert_eq!(pass.decision_input_count, 1);
    assert_eq!(pass.mismatch_count, 0);
    assert_eq!(exit_code(&pass), EXIT_PASS);
    assert_eq!(exit_code(&pass), 0);

    // Fail: the same decision against the T2 view.
    let fail = AsOfReplayReport::build(
        Some(T2),
        history().len(),
        flexcore_sim_runner::compare::compare(
            &[record(1, 1.0)],
            &derive_as_of_snapshot(&history(), Some(T2)),
        ),
    );
    assert_eq!(fail.verdict, VERDICT_FAIL);
    assert_eq!(fail.mismatch_count, 1);
    assert_eq!(exit_code(&fail), EXIT_MISMATCH);
    assert_eq!(exit_code(&fail), 1);
    // The JSON holds the required field names.
    let value: serde_json::Value = serde_json::from_str(&fail.to_json().expect("serialize")).expect("json");
    for field in [
        "as_of_unix_ms",
        "versions_count",
        "snapshot_count",
        "decision_input_count",
        "match_count",
        "mismatch_count",
        "verdict",
        "mismatches",
    ] {
        assert!(value.get(field).is_some(), "required field {field} missing");
    }
}

#[test]
fn the_file_entrypoint_reproduces_a_decision_from_written_files() {
    // Write the versioned history and the decision-input file to a temp
    // directory, then run the same code path the CLI subcommand runs.
    let dir = std::env::temp_dir().join(format!("flexcore-as-of-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let versions_path = dir.join("versions.json");
    let input_path = dir.join("decision-input.json");
    let versions: Vec<VersionedRecord> = history();
    std::fs::write(&versions_path, serde_json::to_string_pretty(&versions).expect("serialize"))
        .expect("write versions");
    std::fs::write(&input_path, records_to_json(&[record(1, 1.0)]).expect("serialize"))
        .expect("write decision input");

    // T1: reproduces.
    let report = run_as_of(
        versions_path.to_str().expect("utf8 path"),
        Some(T1),
        input_path.to_str().expect("utf8 path"),
    )
    .expect("as-of run");
    assert_eq!(report.verdict, VERDICT_PASS);
    assert_eq!(exit_code(&report), EXIT_PASS);

    // Latest (no as-of): the decision that used the pre-correction value
    // breaks against the latest view.
    let report = run_as_of(versions_path.to_str().expect("utf8 path"), None, input_path.to_str().expect("utf8 path"))
        .expect("as-of run");
    assert_eq!(report.verdict, VERDICT_FAIL);
    assert_eq!(exit_code(&report), EXIT_MISMATCH);

    // A malformed versions file is a config error, never a silent pass.
    let broken = dir.join("broken.json");
    std::fs::write(&broken, "not json").expect("write broken");
    let error = run_as_of(broken.to_str().expect("utf8 path"), Some(T1), input_path.to_str().expect("utf8 path"))
        .expect_err("malformed versions file must fail");
    assert!(matches!(error, flexcore_sim_runner::replay::ReplayError::Config(_)));

    let _ = std::fs::remove_dir_all(&dir);
}
