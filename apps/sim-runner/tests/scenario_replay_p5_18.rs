// P5.18 — The scenario definition and replay integration suite (report
// §19.2 layer 5, §24 step 10, §19.3; ledger P5.18).
//
// WHAT IS TESTED (all in-process, deterministic, on the SCENARIO CLOCK —
// no wall clock, no network, no subprocess, no pandapower):
//   1. Pin/record round-trip: a library record loads, validates, and
//      round-trips through JSON unchanged; the input hash is stable and
//      covers exactly the run inputs.
//   2. Replay identity on the scripted backend (deterministic): the same
//      record executed twice produces byte-identical run records AND
//      byte-identical telemetry; the comparison verdict is `identical`.
//   3. Diff detection on a perturbed baseline: a doctored stored run
//      record produces a NAMED structural diff (never a silent pass), and
//      a perturbed telemetry record produces a NAMED comparison diff.
//   4. Missing-input replay errors LOUDLY (the P5.18 acceptance): a
//      missing scenario file, a wrong scenario id/version, a tampered
//      seed, a wrong clock pin, a missing worker pin, and an unpinned
//      thread count each fail with a NAMED error — replay never
//      substitutes an input, never silently defaults one.
//   5. The P4.37 closed loop, wired at the scenario level: a real run
//      records the consumption decision (`within_limits` →
//      `no_consumption` / route `command_lifecycle` on the scripted
//      backend); the pure mapping covers RETURN (→ re-optimization with
//      named constraint inputs), CONSTRAIN (→ the proposal evidence),
//      the D-P4.37-2 unconsumable rejection, and the honest
//      `not_routed` / `not_planned` cases. The re-scheduling consumer
//      stays the P3.24 trigger path (the frozen trigger set is NOT
//      modified).
//
// HONEST BOUNDARIES (recorded, not tested here):
//   - NOTHING in this file was executed: the Phase 5 loop bans builds and
//     test runs. The first `cargo test -p flexcore-sim-runner --test
//     scenario_replay_p5_18` run is the human's gate.
//   - The scripted grid backend produces a recorded FIXTURE outcome, not
//     a computed power flow; the record names it `scripted`.
//   - The worker_process legs are NEVER spawned here (UNVERIFIED
//     pandapower API); the library's stress records pre-register their
//     expectations for the human build/test pass.

use flexcore_application::feasibility_return::{
    plan_consumption, ConsumptionAction, FeasibilityOutcome, ReturnedCandidate,
};
use flexcore_sim_runner::replay_record::{
    compare_against_baseline, compare_double_run, execute_run, input_hash, load_scenario_record,
    ScenarioRecordV3,
};
use flexcore_sim_runner::reproducibility::VERDICT_IDENTICAL;

const LIBRARY_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/library");

fn library_record(file: &str) -> String {
    std::fs::read_to_string(format!("{LIBRARY_DIR}/{file}"))
        .expect("the library record exists in the repository")
}

fn load_lib(file: &str) -> (ScenarioRecordV3, String) {
    let path = format!("{LIBRARY_DIR}/{file}");
    load_scenario_record(&path).expect("the library record loads and validates")
}

fn execute_lib(file: &str, code_version: &str) -> (ScenarioRecordV3, flexcore_sim_runner::replay_record::ExecutedRun) {
    let (record, record_dir) = load_lib(file);
    let executed =
        execute_run(&record, &record_dir, code_version).expect("the record executes from its pins");
    (record, executed)
}

// --- 1. Pin/record round-trip -------------------------------------------------

#[test]
fn a_library_record_round_trips_through_json_unchanged() {
    let (record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    record.validate().expect("a loaded library record is valid");
    let text = serde_json::to_string_pretty(&record).expect("serialize");
    let parsed: ScenarioRecordV3 = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(parsed, record);
}

#[test]
fn the_input_hash_is_stable_and_separates_the_run_inputs_from_the_pre_registration() {
    let (record, _dir) = load_lib("lib-02-small-portfolio.json");
    let hash_a = input_hash(&record).expect("hash");
    let hash_b = input_hash(&record).expect("hash");
    assert_eq!(hash_a, hash_b, "the same record hashes the same");
    // The pre-registration text is NOT a run input: changing the expected
    // outcomes does not change the input hash (it is a documentation
    // change, never a scenario change).
    let mut documentation_change = record.clone();
    documentation_change.expected_outcomes.statements.push(
        "a recorded status update (never an expectation edit)".to_string(),
    );
    let hash_c = input_hash(&documentation_change).expect("hash");
    assert_eq!(hash_a, hash_c, "pre-registration text is not hashed");
    // A run-input change DOES change the hash.
    let mut scenario_change = record.clone();
    scenario_change.inputs.dispatch.as_mut().expect("dispatch pin").target_p_mw = 0.25;
    let hash_d = input_hash(&scenario_change).expect("hash");
    assert_ne!(hash_a, hash_d, "a dispatch-pin change changes the hash");
}

// --- 2. Replay identity on the scripted backend --------------------------------

#[test]
fn the_same_record_executed_twice_is_byte_identical_on_the_scripted_backend() {
    let (record, run_a) = execute_lib("lib-01-minimal-single-asset.json", "test-code");
    let (record_dir) = {
        // Re-derive the record dir the same way load does.
        let path = format!("{LIBRARY_DIR}/lib-01-minimal-single-asset.json");
        std::path::Path::new(&path)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    };
    let run_b =
        execute_run(&record, &record_dir, "test-code").expect("the second execution");
    assert_eq!(run_a.run_record, run_b.run_record, "the run record is identical");
    assert_eq!(run_a.telemetry, run_b.telemetry, "the telemetry is identical");

    let comparison = compare_double_run(&record, &run_a, &run_b, "test-code")
        .expect("the comparison builds");
    assert_eq!(comparison.comparison_kind, "double_run");
    assert_eq!(comparison.verdict, VERDICT_IDENTICAL, "{}", comparison.verdict_statement);
    assert!(comparison.input_hash_match);
    for leg in &comparison.legs {
        assert!(
            leg.verdict == "pass" || leg.verdict == VERDICT_IDENTICAL,
            "leg {} reported {} ({})",
            leg.leg,
            leg.verdict,
            leg.detail
        );
    }
}

#[test]
fn the_small_portfolio_record_executes_and_records_the_full_slice_accounting() {
    let (_record, run) = execute_lib("lib-02-small-portfolio.json", "test-code");
    assert_eq!(run.run_record.scenario_id, "synthetic-portfolio-exit-gate");
    assert_eq!(run.run_record.scenario_version, "v2");
    assert_eq!(run.run_record.master_seed, 20260601);
    // The telemetry leg covers the full horizon (1440 ticks × 10 assets ×
    // the emitted quantities — the exact count is the simulator's; the
    // identity is what replay requires).
    assert!(!run.telemetry.is_empty());
    // The adapter inventory names every leg real-vs-seam (P5.14 evidence,
    // carried into the replay path).
    assert!(run
        .run_record
        .adapters
        .iter()
        .any(|adapter| adapter.leg == "grid_feasibility"));
}

// --- 3. Diff detection ----------------------------------------------------------

#[test]
fn a_perturbed_baseline_produces_a_named_structural_diff() {
    let (_record, run) = execute_lib("lib-01-minimal-single-asset.json", "test-code");
    let (record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    // The baseline: the run record with ONE field doctored.
    let mut baseline: serde_json::Value =
        serde_json::to_value(&run.run_record).expect("render");
    baseline["commands"][0]["final_state"] = serde_json::Value::String("FAILED".to_string());
    let baseline_json = serde_json::to_string_pretty(&baseline).expect("render");
    let comparison = compare_against_baseline(&record, &run, &baseline_json, "test-code")
        .expect("the comparison builds");
    assert_eq!(comparison.verdict, "mismatch");
    let run_leg = comparison
        .legs
        .iter()
        .find(|leg| leg.leg == "run_record")
        .expect("the run-record leg");
    assert!(run_leg.verdict == "fail" || run_leg.verdict == "mismatch");
    assert!(
        run_leg
            .diffs
            .iter()
            .any(|diff| diff.contains("commands[0].final_state")),
        "the diff names the path: {:?}",
        run_leg.diffs
    );
}

#[test]
fn a_perturbed_telemetry_record_produces_a_named_comparison_diff() {
    let (record, run_a) = execute_lib("lib-01-minimal-single-asset.json", "test-code");
    let record_dir = format!("{LIBRARY_DIR}");
    // Perturb ONE telemetry value (the negative control's shape: a changed
    // input must NOT silently reproduce the old result).
    let mut perturbed = run_a.telemetry.clone();
    let first = perturbed
        .iter_mut()
        .find(|record| matches!(record.value, flexcore_sim_runner::replay::ReplayValue::Power { mw } if mw != 0.0))
        .expect("a power record exists to perturb");
    if let flexcore_sim_runner::replay::ReplayValue::Power { mw } = &mut first.value {
        *mw += 1.0;
    }
    let run_b = flexcore_sim_runner::replay_record::ExecutedRun {
        run_record: run_a.run_record.clone(),
        telemetry: perturbed,
    };
    let comparison = compare_double_run(&record, &run_a, &run_b, "test-code")
        .expect("the comparison builds");
    assert_eq!(comparison.verdict, "mismatch");
    let telemetry_leg = comparison
        .legs
        .iter()
        .find(|leg| leg.leg == "telemetry")
        .expect("the telemetry leg");
    assert!(telemetry_leg.diff_count >= 1);
    assert!(
        telemetry_leg.diffs.iter().any(|diff| diff.contains("value differs")),
        "the diff names the field: {:?}",
        telemetry_leg.diffs
    );
    let _ = record_dir;
}

#[test]
fn a_recorded_input_hash_mismatch_is_a_tamper_finding_not_a_silent_rehash() {
    let (mut record, run) = load_lib("lib-01-minimal-single-asset.json");
    let recomputed = input_hash(&record).expect("hash");
    record.input_hash = Some("00000000000000000000000000000000".to_string());
    let record_dir = LIBRARY_DIR.to_string();
    let baseline_json =
        serde_json::to_string_pretty(&run.run_record).expect("render the run record");
    let comparison = compare_against_baseline(&record, &run, &baseline_json, "test-code")
        .expect("the comparison builds even with a tampered hash");
    assert!(!comparison.input_hash_match);
    let input_leg = comparison
        .legs
        .iter()
        .find(|leg| leg.leg == "input_hash")
        .expect("the input-hash leg");
    assert_eq!(input_leg.verdict, "fail");
    assert!(input_leg.diffs[0].contains("tamper finding"));
    assert_ne!(recomputed, "00000000000000000000000000000000");
    let _ = record_dir;
}

// --- 4. Missing-input replay errors LOUDLY ---------------------------------------

#[test]
fn a_missing_scenario_file_is_a_named_error() {
    let (mut record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    record.inputs.scenario.path = "fixtures/does-not-exist.json".to_string();
    let error = execute_run(&record, LIBRARY_DIR, "test-code").unwrap_err();
    assert!(error.to_string().contains("does-not-exist"), "{error}");
}

#[test]
fn a_wrong_scenario_version_pin_is_a_named_error_never_a_substitution() {
    let (mut record, _dir) = load_lib("lib-02-small-portfolio.json");
    record.inputs.scenario.version = "v1".to_string();
    let error = execute_run(&record, LIBRARY_DIR, "test-code").unwrap_err();
    assert!(
        error.to_string().contains("never substitutes"),
        "the error names the rule: {error}"
    );
}

#[test]
fn a_tampered_seed_pin_is_a_named_error_never_a_reseed() {
    let (mut record, _dir) = load_lib("lib-02-small-portfolio.json");
    record.seeds.master_seed = 999999;
    let error = execute_run(&record, LIBRARY_DIR, "test-code").unwrap_err();
    assert!(
        error.to_string().contains("never re-seeds"),
        "the error names the rule: {error}"
    );
}

#[test]
fn a_wrong_clock_pin_is_a_named_error() {
    let (mut record, _dir) = load_lib("lib-02-small-portfolio.json");
    record.inputs.clock.cadence_seconds = 900;
    let error = execute_run(&record, LIBRARY_DIR, "test-code").unwrap_err();
    assert!(error.to_string().contains("cadence"), "{error}");
}

#[test]
fn an_unpinned_worker_selection_is_rejected_before_anything_spawns() {
    let (mut record, _dir) = load_lib("lib-02-small-portfolio.json");
    record.backend.selection = "worker_process".to_string();
    record.backend.worker = None;
    // Validation rejects the record before any execution path exists.
    let error = record.validate().unwrap_err();
    assert!(error.to_string().contains("worker pin"), "{error}");
}

#[test]
fn an_unpinned_thread_count_is_rejected_before_anything_executes() {
    let (mut record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    record.nondeterminism_controls.thread_count = 8;
    let error = record.validate().unwrap_err();
    assert!(error.to_string().contains("thread_count = 1"), "{error}");
}

#[test]
fn an_unknown_dispatch_asset_is_a_named_error() {
    let (mut record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    record.inputs.dispatch.as_mut().expect("dispatch pin").asset_key = "no-such-asset".to_string();
    let error = execute_run(&record, LIBRARY_DIR, "test-code").unwrap_err();
    assert!(error.to_string().contains("no-such-asset"), "{error}");
}

// --- 5. The P4.37 closed loop at the scenario level -------------------------------

#[test]
fn the_run_record_carries_the_closed_loop_consumption_decision() {
    let (_record, run) = execute_lib("lib-01-minimal-single-asset.json", "test-code");
    assert_eq!(run.run_record.consumption_decisions.len(), 1);
    let decision = &run.run_record.consumption_decisions[0];
    // The scripted fixture never violates a limit: the frozen routing is
    // `within_limits`, and the contract plans NO scheduling-side
    // consumption — the command lifecycle owns the candidate.
    assert_eq!(decision.outcome_code, "within_limits");
    assert_eq!(decision.action, "no_consumption");
    assert_eq!(decision.route.as_deref(), Some("command_lifecycle"));
    assert_eq!(decision.decision_ref, "D-P4.07-1");
    assert!(decision.violated_constraints.is_empty());
    // The consumer boundary travels ON the summary: the frozen P3.24
    // trigger set is NOT modified.
    assert!(decision
        .statements
        .iter()
        .any(|statement| statement.contains("P3.24 rolling-horizon trigger path")));
    assert!(decision.roll_trigger_note.contains("roll-trigger"));
}

#[test]
fn the_not_routed_case_is_recorded_honestly() {
    // A dispatch pin whose validation fails BEFORE the feasibility gate:
    // the P4.37 contract is not invoked, and the summary says so — no
    // consumption decision is invented.
    let (record, _dir) = load_lib("lib-01-minimal-single-asset.json");
    let summary = flexcore_sim_runner::feasibility_consumption::not_routed_summary("cmd-x");
    assert_eq!(summary.outcome_code, "not_routed");
    assert_eq!(summary.action, "no_consumption");
    assert!(summary.route.is_none());
    assert!(summary.reason.contains("no consumption decision exists to plan"));
    let _ = (record, _dir);
}

/// A RETURN-shaped candidate (the frozen grammar
/// `<category_code>:<element_id>`), the closed loop's consuming path.
fn returned_candidate() -> ReturnedCandidate {
    ReturnedCandidate {
        candidate_id: "cand-1".to_string(),
        schedule_id: "sched-1".to_string(),
        revision: 2,
        envelope_version: "env-1".to_string(),
        result_record_snapshot_ref: "snap-1".to_string(),
        outcome: FeasibilityOutcome::ReturnedForReOptimization,
        outcome_reason: "single voltage violation".to_string(),
        check_id: "check-1".to_string(),
        violated_constraints: vec!["bus_voltage_upper:bus_3".to_string()],
        constrain_proposal: None,
        model_id: "feeder-a".to_string(),
        model_version: "1".to_string(),
        model_content_hash: "hash-1".to_string(),
    }
}

#[test]
fn a_return_outcome_feeds_plan_consumption_with_named_constraint_inputs() {
    let action = plan_consumption(&returned_candidate()).expect("a valid candidate");
    match action {
        ConsumptionAction::ReOptimize(input) => {
            assert_eq!(input.outcome_code, "returned_for_re_optimization");
            assert_eq!(input.violated_constraints.len(), 1);
            let annotation = &input.violated_constraints[0];
            assert_eq!(annotation.constraint_name, "bus_voltage_upper:bus_3");
            assert!(annotation.problem_input.contains("bus_3"));
            assert!(annotation.problem_input.contains("re-checks"));
            // The trigger gap travels with the input (the P4.37 open
            // item): the frozen trigger set is NOT extended here.
            assert!(input.roll_trigger_note.contains("roll-trigger"));
        }
        other => panic!("expected ReOptimize, got {other:?}"),
    }
}

#[test]
fn an_unconsumable_constraint_rejects_the_candidate_with_the_recorded_reason() {
    let mut candidate = returned_candidate();
    candidate.violated_constraints = vec!["made_up_category:bus_3".to_string()];
    match plan_consumption(&candidate).expect("semantic failure is not an Err") {
        ConsumptionAction::RejectUnconsumable { reason } => {
            assert!(reason.contains("made_up_category"));
        }
        other => panic!("expected RejectUnconsumable, got {other:?}"),
    }
}

#[test]
fn a_pipeline_level_verdict_without_candidate_identity_is_not_planned_never_silent() {
    // The composition-root conversion of a pipeline-level verdict carries
    // EMPTY identity refs; the contract's structural validation refuses
    // it, and the recorded summary says `not_planned` with the reason —
    // never silently dropped, never passed.
    let mut candidate = returned_candidate();
    candidate.candidate_id = String::new();
    candidate.schedule_id = String::new();
    candidate.envelope_version = String::new();
    candidate.result_record_snapshot_ref = String::new();
    candidate.check_id = String::new();
    candidate.model_id = String::new();
    candidate.model_version = String::new();
    candidate.model_content_hash = String::new();
    let error = plan_consumption(&candidate).expect_err("structural refusal");
    assert!(error.to_string().contains("candidate_id"));
}
