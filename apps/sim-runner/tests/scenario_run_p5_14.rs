// P5.14/P5.15 — The scenario-run composition integration suite (report
// §19.1, §24 vertical slice steps 5–8, §19.2 layer 5; ledger P5.14).
//
// WHAT IS TESTED (all in-process, deterministic, on the SCENARIO CLOCK —
// no wall clock, no network, no subprocess, no pandapower):
//   1. The full composition runs the §24 steps 5–8 dispatch slice against
//      the REAL Phase 2 simulator (telemetry ingest) and the REAL Phase 4
//      adapter chain (CommandGates → SimulatorDispatchAdapter over the
//      real transport), and the run record NAMES every adapter real vs
//      seam — the P5.14 acceptance criterion.
//   2. The run record carries every version a replay needs (P5.14 step 3,
//      the P5.19 reproducibility input): scenario version, master seed,
//      code version, simulator crate version, per-asset model identities.
//   3. Two runs with identical inputs produce byte-identical run records
//      (the determinism identity, extended from records to the whole run).
//   4. A simulator asset dropout mid-run is VISIBLE (the availability
//      change is recorded with its evidence kind) and triggers the REAL
//      P3.24 rolling-roll path (P5.14 step 4).
//   5. A delivery attempted INSIDE an enabled drop window is refused by
//      the real transport mapping (D-P4.16-4): no acceptance, the command
//      FAILED — the Phase 4 reject path reached through the real simulator
//      configuration, never a coerced pass.
//   6. An unknown dispatch asset is a NAMED error, never a silent skip.
//   7. The `worker_process` grid-backend selection names the REAL P5.15
//      leg in the inventory WITHOUT spawning anything (composition only;
//      the worker was never executed — its pandapower 3.5.6 API usage is
//      UNVERIFIED until the human build/test pass).
//
// HONEST BOUNDARIES (recorded, not tested here):
//   - The command-store/outbox/envelope/model-version/asset-capability/
//     authorization seams are SEAM-LEVEL in-memory composition support —
//     the record itself names them Seam; no test asserts otherwise.
//   - The `scripted` grid backend produces a recorded FIXTURE outcome, not
//     a computed power flow; the record names it `scripted`. The real
//     worker-process leg's payload behaviour is tested in
//     `crates/flexcore-grid-api/tests/worker_backend_p5_15.rs` with
//     SCRIPTED fake-worker payloads (still no pandapower execution).
//   - NOTHING in this file was executed: the Phase 5 loop bans builds and
//     test runs. The first `cargo test -p flexcore-sim-runner --test
//     scenario_run_p5_14` run is the human's gate.
//
// The §24 steps 9–10 (console display, replay verification) are P5.13/
// P5.18–P5.19 — NOT this suite's scope.

use std::collections::BTreeMap;
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{CommandId, ScheduleId};
use flexcore_grid_api::adapter::{PROVIDER_ID, PROVIDER_PINNED_VERSION};
use flexcore_sim_runner::scenario_run::{
    AdapterImplementation, CommandRunConfig, GridBackendSelection, PriorScheduleFixture,
    RecordedDropout, ScenarioRunError, ScenarioRunHarness, SchedulingGridConfig,
};
use flexcore_simulator::SIMULATOR_CRATE_VERSION;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/synthetic-portfolio");

/// The fixed composition fixture UUIDs (deterministic stand-ins, recorded
/// in `ledger/phase-05-operator-console-simulator/14_15_adapters.md` — NOT
/// registry-issued IDs; the same defaults the CLI uses).
const SCHEDULE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7ab1";
const ROLL_SCHEDULE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7ac1";
const ROLL_CANDIDATE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7ac2";

fn read_scenario(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURE_DIR}/{name}"))
        .expect("the pinned scenario fixture exists in the repository")
}

fn schedule_id(uuid: &str) -> ScheduleId {
    ScheduleId::new(
        FlexUuid::parse(uuid).expect("the fixture schedule UUID is canonical"),
    )
}

/// The tracking criterion is DEPLOYMENT CONFIGURATION (D-P4.16-3): these
/// are the recorded composition defaults, spelled out here so a test that
/// changes one fails visibly rather than inheriting a constant.
fn command_config(asset_key: &str) -> CommandRunConfig {
    CommandRunConfig {
        asset_key: asset_key.to_string(),
        target_p_mw: 0.0,
        target_q_mvar: 0.0,
        tracking_tolerance_mw: 0.5,
        tracking_min_satisfied_samples: 1,
        tracking_timeout_millis: 1_800_000,
        approver: "scenario-operator".to_string(),
        approver_role: "scenario-approver".to_string(),
    }
}

/// The dispatch slice instants: issue one cadence after the start, poll 15
/// minutes later (both inside the horizon; both scenario-clock instants).
fn slice_instants(harness: &ScenarioRunHarness) -> (i64, i64) {
    let cadence_millis = harness.scenario().cadence_seconds.max(1) as i64 * 1_000;
    let issue_at = harness.scenario().start_ms + cadence_millis;
    (issue_at, issue_at + 15 * 60_000)
}

/// A deterministic fixture command id (the stand-in identity discipline;
/// NOT a registry-issued id — recorded in `14_15_adapters.md`).
fn command_id(suffix: &str) -> CommandId {
    CommandId::new(
        FlexUuid::parse(&format!("0192a5e0-0000-7e2f-9a1b-{suffix}"))
            .expect("the fixture command UUID is canonical"),
    )
}

/// The first scenario asset key (deterministic; the scenario file pins the
/// order).
fn first_asset_key(harness: &ScenarioRunHarness) -> String {
    harness.scenario().sites[0].assets[0].asset_key.clone()
}

/// The recorded dropouts of a scenario, derived from its OWN fault plan
/// (configuration, P2.15 — never synthesized by the test).
fn recorded_dropouts(harness: &ScenarioRunHarness) -> Vec<RecordedDropout> {
    let scenario = harness.scenario();
    if !scenario.faults.enabled {
        return Vec::new();
    }
    scenario
        .faults
        .injections
        .iter()
        .filter(|spec| spec.kind == flexcore_simulator::FaultKind::Drop)
        .map(|spec| RecordedDropout {
            asset_key: spec.asset_key.clone(),
            from_millis: spec.from_ms,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. The full slice against the REAL simulator + REAL adapter chain, with
//    the run record naming every adapter (the P5.14 acceptance core).
// ---------------------------------------------------------------------------

#[test]
fn the_composition_runs_the_slice_against_the_real_simulator_and_names_its_adapters() {
    let scenario_json = read_scenario("scenario-v2.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the exit-gate scenario composes");

    let asset_key = first_asset_key(&harness);
    let (issue_at, poll_at) = slice_instants(&harness);
    let slice = harness
        .run_dispatch_slice(
            &command_config(&asset_key),
            command_id("3c4d5e6f7ad1"),
            &schedule_id(SCHEDULE_UUID),
            1,
            issue_at,
            poll_at,
        )
        .expect("the dispatch slice composes end to end");

    // The REAL simulator accepted the delivery at the application layer
    // (acceptance — NEVER physical delivery, §14.1; the record carries the
    // statement, not the display).
    assert_eq!(slice.command.issue_outcome, "accepted");
    let acceptance_reference = slice
        .command
        .acceptance_reference
        .as_deref()
        .expect("an accepted delivery carries its acceptance reference");
    assert!(
        acceptance_reference.starts_with("sim-accept:"),
        "the acceptance reference is the simulator's application-layer form, got \
         {acceptance_reference:?}"
    );

    // Tracking reached a terminal state through the REAL observations.
    let verdict = slice
        .command
        .tracking_verdict
        .expect("a polled tracking outcome is recorded");
    assert!(
        matches!(
            verdict.as_str(),
            "completed" | "deviation" | "timeout" | "keep_tracking" | "failed" | "expired"
        ),
        "the tracking verdict is one of the recorded adapter outcomes, got {verdict:?}"
    );
    assert!(
        slice.command.final_state == CommandState::Completed.name()
            || slice.command.final_state == CommandState::Failed.name(),
        "the final lifecycle state is terminal, got {:?}",
        slice.command.final_state
    );

    // The grid leg: the REAL pipeline verdict, over the NAMED backend.
    assert_eq!(slice.grid_check.backend, "scripted");
    assert_eq!(
        slice.grid_check.provider_id_version,
        format!("{PROVIDER_ID}@{PROVIDER_PINNED_VERSION}")
    );
    assert_eq!(slice.grid_check.verdict, "passed");
    assert!(!slice.grid_check.attempt_id.is_empty());

    // The run record: the adapter inventory is the P5.14 deliverable.
    let record = harness
        .record("scenario-run-p5-14-test", vec![slice.command], vec![slice.grid_check], Vec::new())
        .expect("the run record builds");

    assert_eq!(record.scenario_id, "synthetic-portfolio-exit-gate");
    assert_eq!(record.scenario_version, "v2");
    assert_eq!(record.simulator_crate_version, SIMULATOR_CRATE_VERSION);
    assert_eq!(record.simulator_source_id, "sim://synthetic-portfolio-exit-gate");

    let mut legs = std::collections::BTreeMap::new();
    for adapter in &record.adapters {
        legs.insert(adapter.leg.as_str(), (adapter.implementation, adapter));
    }
    assert_eq!(legs.len(), 5, "every run leg is named exactly once");
    assert_eq!(legs["telemetry_ingest"].0, AdapterImplementation::Real);
    assert_eq!(legs["dispatch"].0, AdapterImplementation::Real);
    assert_eq!(legs["command_gates"].0, AdapterImplementation::Seam);
    assert_eq!(legs["grid_feasibility"].0, AdapterImplementation::Seam);
    assert_eq!(legs["schedule_roll"].0, AdapterImplementation::Real);
    for (leg, (implementation, adapter)) in &legs {
        assert!(
            !adapter.detail.is_empty(),
            "the {leg} adapter carries its real-vs-seam explanation"
        );
        if *implementation == AdapterImplementation::Seam {
            assert!(
                adapter.adapter.contains("seam")
                    || adapter.adapter.contains("in-memory")
                    || adapter.detail.contains("in-process"),
                "a seam-level entry names what stands in, got {:?} / {:?}",
                adapter.adapter,
                adapter.detail
            );
        }
    }

    // The simulation-only boundary travels ON the record.
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("issued to the SIMULATOR")));
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("NOT an AC-OPF")));

    // The telemetry accounting: the REAL source produced the samples, with
    // exactly one quality state per value (v2 ships faults disabled ⇒ the
    // exit-gate run is all-GOOD, §24 step 2).
    assert!(record.records_ingested > 0);
    let total_quality: usize = record.quality_counts.values().sum();
    assert_eq!(
        total_quality,
        record.records_ingested,
        "every ingested value carries exactly one quality state"
    );
    assert_eq!(
        record.quality_counts.get("GOOD").copied().unwrap_or(0),
        record.records_ingested,
        "the v2 exit-gate run (faults disabled) is all-GOOD; the quality map is {:?}",
        record.quality_counts
    );

    // The per-asset model identities (P5.14 step 3).
    let bess = record
        .asset_models
        .iter()
        .find(|model| model.asset_key == "site-north-bess-1")
        .expect("the BESS model identity is recorded");
    assert!(bess.rated_power_mw > 0.0);
    assert_eq!(bess.rated_energy_mwh, Some(4.0));
}

// ---------------------------------------------------------------------------
// 2. Determinism: identical inputs ⇒ byte-identical run records (the P5.19
//    replay input discipline, extended to the whole composition).
// ---------------------------------------------------------------------------

#[test]
fn identical_inputs_produce_byte_identical_run_records() {
    let scenario_json = read_scenario("scenario-v2.json");
    let run_once = || {
        let harness = ScenarioRunHarness::new(
            &scenario_json.clone(),
            SchedulingGridConfig {
                interval_millis: 15 * 60_000,
            },
            GridBackendSelection::Scripted,
            None,
        )
        .expect("the scenario composes");
        let asset_key = first_asset_key(&harness);
        let (issue_at, poll_at) = slice_instants(&harness);
        let slice = harness
            .run_dispatch_slice(
                &command_config(&asset_key),
                command_id("3c4d5e6f7ad1"),
                &schedule_id(SCHEDULE_UUID),
                1,
                issue_at,
                poll_at,
            )
            .expect("the dispatch slice composes end to end");
        harness
            .record("determinism-test", vec![slice.command], vec![slice.grid_check], Vec::new())
            .expect("the run record builds")
    };
    let first = run_once();
    let second = run_once();
    let first_json = serde_json::to_string(&first).expect("the record serializes");
    let second_json = serde_json::to_string(&second).expect("the record serializes");
    assert_eq!(
        first_json, second_json,
        "same scenario version + seed + code version ⇒ the identical run record \
         (P2.13 §3 determinism identity, extended to the composition)"
    );
}

// ---------------------------------------------------------------------------
// 3. The dropout is VISIBLE and triggers the REAL P3.24 roll path
//    (P5.14 step 4).
// ---------------------------------------------------------------------------

#[test]
fn the_dropout_is_visible_and_triggers_the_real_rolling_roll() {
    let scenario_json = read_scenario("scenarios-phase3/scenario-c-dropout.json");
    // The prior schedule fixture: the composition supplies the P3.22
    // revision identity; the roll plan supersedes it, never rewrites it.
    let prior = PriorScheduleFixture {
        schedule_id: ROLL_SCHEDULE_UUID.to_string(),
        revision: 1,
        candidate_id: ROLL_CANDIDATE_UUID.to_string(),
        setpoints_mw: {
            // A flat 0.0 plan over the scenario's assets (BTreeMap for the
            // deterministic interval rendering the planner documents).
            let harness_probe = ScenarioRunHarness::new(
                &scenario_json.clone(),
                SchedulingGridConfig {
                    interval_millis: 15 * 60_000,
                },
                GridBackendSelection::Scripted,
                None,
            )
            .expect("the scenario composes");
            let mut setpoints = BTreeMap::new();
            for site in harness_probe.scenario().sites.iter() {
                for asset in site.assets.iter() {
                    setpoints.insert(asset.asset_key.clone(), 0.0);
                }
            }
            setpoints
        },
    };
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        Some(prior),
    )
    .expect("the dropout scenario composes");

    let asset_key = first_asset_key(&harness);
    let (issue_at, poll_at) = slice_instants(&harness);
    let slice = harness
        .run_dispatch_slice(
            &command_config(&asset_key),
            command_id("3c4d5e6f7ad3"),
            &schedule_id(SCHEDULE_UUID),
            1,
            issue_at,
            poll_at,
        )
        .expect("the dispatch slice composes (the issue instant is outside the drop window)");

    let dropouts = recorded_dropouts(&harness);
    assert_eq!(
        dropouts.len(),
        1,
        "the scenario (c) fault plan carries exactly one drop injection"
    );
    assert_eq!(dropouts[0].asset_key, "site-north-bess-1");

    // The full horizon so the availability accounting covers the drop.
    harness.run_to_horizon_end().expect("the horizon completes");

    let record = harness
        .record("dropout-visibility-test", vec![slice.command], vec![slice.grid_check], dropouts)
        .expect("the run record builds");

    // VISIBILITY: the availability change is recorded with its evidence
    // kind (the drop window is a plan entry, because a drop emits NO
    // frames — the absence itself is the platform-visible effect).
    let availability = record
        .availability_changes
        .iter()
        .find(|change| {
            change.asset_key == "site-north-bess-1"
                && change.evidence == "fault_plan_drop_window"
        })
        .expect("the dropout is visible in the run record");
    assert!(!availability.available);
    assert_eq!(
        availability.at_unix_millis,
        record.dropouts[0].from_millis,
        "the availability change is recorded at the drop-window start"
    );

    // The REAL P3.24 roll: supersede the prior revision, exclude the
    // dropped asset from the re-optimized window.
    let roll = record
        .roll_plan
        .as_ref()
        .expect("the recorded dropout triggered the rolling-roll plan");
    assert_eq!(roll.trigger, "asset_dropout");
    assert!(
        roll.trigger_detail.contains("site-north-bess-1"),
        "the trigger detail names the dropped asset"
    );
    assert_ne!(roll.new_revision, roll.superseded);
    assert!(
        !roll
            .active_asset_keys
            .iter()
            .any(|key| key == "site-north-bess-1"),
        "the dropped asset is not in the re-optimized window's active set"
    );
    assert!(
        !roll.notes.is_empty(),
        "the roll plan records its notes (frozen rule, dropped assets)"
    );
}

// ---------------------------------------------------------------------------
// 4. A delivery attempted INSIDE an enabled drop window is refused
//    fail-closed by the REAL transport mapping (D-P4.16-4) — the Phase 4
//    reject path reached through the simulator's own configuration.
// ---------------------------------------------------------------------------

#[test]
fn a_delivery_inside_a_drop_window_is_refused_fail_closed() {
    let scenario_json = read_scenario("scenarios-phase3/scenario-c-dropout.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the dropout scenario composes");

    // Issue one cadence AFTER the drop window opens (12:00–13:00 UTC on
    // the scenario clock): the delivery instant is inside the window.
    let drop_from = harness.scenario().faults.injections[0].from_ms;
    let cadence_millis = harness.scenario().cadence_seconds.max(1) as i64 * 1_000;
    let issue_at = drop_from + cadence_millis;
    let poll_at = issue_at + 15 * 60_000;

    let slice = harness
        .run_dispatch_slice(
            // The DROPPED asset is the dispatch target: the real transport
            // must refuse the delivery for exactly this asset.
            &command_config("site-north-bess-1"),
            command_id("3c4d5e6f7ad4"),
            &schedule_id(SCHEDULE_UUID),
            1,
            issue_at,
            poll_at,
        )
        .expect("the slice completes with the refusal recorded");

    assert_ne!(
        slice.command.issue_outcome, "accepted",
        "a drop-window delivery has NO acceptance (the frame would not exist either)"
    );
    assert!(slice.command.acceptance_reference.is_none());
    assert!(slice.command.tracking_verdict.is_none());
    assert_eq!(
        slice.command.final_state,
        CommandState::Failed.name(),
        "the command FAILED (the frozen lifecycle mapping for a delivery timeout)"
    );
}

// ---------------------------------------------------------------------------
// 5. An unknown dispatch asset is a NAMED error, never a silent skip.
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_dispatch_asset_is_a_named_error() {
    let scenario_json = read_scenario("scenario-v2.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the scenario composes");

    let error = harness
        .run_dispatch_slice(
            &command_config("asset-that-does-not-exist"),
            command_id("3c4d5e6f7ad5"),
            &schedule_id(SCHEDULE_UUID),
            1,
            harness.scenario().start_ms + 60_000,
            harness.scenario().start_ms + 60_000 + 15 * 60_000,
        )
        .expect_err("an unknown asset is an error");
    assert_eq!(
        error,
        ScenarioRunError::UnknownAsset("asset-that-does-not-exist".to_string())
    );
}

// ---------------------------------------------------------------------------
// 6. The `worker_process` grid-backend selection names the REAL P5.15 leg
//    in the inventory WITHOUT spawning anything (composition only).
// ---------------------------------------------------------------------------

#[test]
fn the_worker_backend_selection_names_the_real_leg_without_spawning() {
    let scenario_json = read_scenario("scenario-v2.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::WorkerProcess {
            program: "python".to_string(),
            script_path: "apps/worker/python/grid_worker.py".to_string(),
            timeout: Duration::from_secs(30),
        },
        None,
    )
    .expect("composing the worker backend selection spawns NOTHING (composition is lazy)");

    let inventory = harness.adapter_inventory();
    let grid_leg = inventory
        .iter()
        .find(|trace| trace.leg == "grid_feasibility")
        .expect("the grid leg is in the inventory");
    assert_eq!(grid_leg.implementation, AdapterImplementation::Real);
    assert!(
        grid_leg.adapter.contains("worker_process"),
        "the inventory names the worker_process backend, got {:?}",
        grid_leg.adapter
    );
    assert!(
        grid_leg.detail.contains("UNVERIFIED"),
        "the real-leg detail carries the UNVERIFIED-API honesty note verbatim"
    );
    assert!(
        grid_leg.detail.contains("never executed") || grid_leg.detail.contains("build/test"),
        "the detail states the worker was never executed in this loop"
    );

    // A record built from this harness names the same backend — a run can
    // never silently substitute the scripted seam for the real leg (and
    // vice versa).
    let record = harness
        .record("worker-selection-test", Vec::new(), Vec::new(), Vec::new())
        .expect("the run record builds");
    let record_leg = record
        .adapters
        .iter()
        .find(|trace| trace.leg == "grid_feasibility")
        .expect("the grid leg is on the record");
    assert_eq!(record_leg.implementation, AdapterImplementation::Real);
    assert!(record_leg.adapter.contains("worker_process"));
}

// ---------------------------------------------------------------------------
// 7. The scripted backend is HONESTLY seam-level in the inventory (a
//    scripted fixture outcome can never pass as a computed power flow).
// ---------------------------------------------------------------------------

#[test]
fn the_scripted_backend_is_named_seam_level_in_the_inventory() {
    let scenario_json = read_scenario("scenario-v2.json");
    let harness = ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the scenario composes");

    let inventory = harness.adapter_inventory();
    let grid_leg = inventory
        .iter()
        .find(|trace| trace.leg == "grid_feasibility")
        .expect("the grid leg is in the inventory");
    assert_eq!(grid_leg.implementation, AdapterImplementation::Seam);
    assert!(grid_leg.adapter.contains("scripted"));
    assert!(
        grid_leg.detail.contains("NOT a computed power flow"),
        "the scripted detail states it is a fixture, never a computed power flow"
    );
}
