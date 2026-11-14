// P5.21/P5.22 — The scenario-level fault-injection suite (report §19.2
//         layer 6, §23 Stage 5; ledger P5.21 + P5.22).
//
// WHAT IS TESTED (all in-process, deterministic, on the SCENARIO CLOCK):
//   1. The `scenario_faults` v3 slot parses into the validated plan with
//      every kind's PRE-REGISTERED expected response and its P4.31
//      reference (the reuse-not-duplicate rule is a data fact here).
//   2. The slot validator fails CLOSED on an unknown schema, an unknown
//      kind, an unknown asset key, and an inverted window — named errors,
//      never silent interpretation.
//   3. With no slot, the plan derives 1:1 from the validated P2.15 fault
//      plan, mapping each source fault onto the §19.2 layer 6 vocabulary
//      with deterministic ids.
//   4. A disabled P2.15 plan with no slot is an INERT plan — no fault
//      invented, no spec present.
//   5. The clock-skew probe (composition-level injection) is REFUSED
//      fail-closed at an instant past the command's validity (the P4.31
//      F4 shape: expiry wins, never a silent acceptance).
//   6. The partial-fleet probe records PER-ASSET outcomes (the P4.31 F11
//      shape): the asset inside the enabled drop window fails while the
//      others accept — and each outcome is recorded individually, never
//      portfolio-inferred.
//   7. The built journal NEVER records a silent pass: every entry carries
//      a non-empty observed effect and an explicit response; unobserved
//      faults are `not_observed_in_this_run` WITH the reason; solver
//      timeout and plugin crash are CROSS-REFERENCES to the P4.31 Phase 4
//      suites (never fabricated scenario outcomes).
//   8. An explicit probe outcome wins over the mined record facts (the
//      journal renders the direct observation).
//
// HONEST BOUNDARIES:
//   - The pre-registered expectations come from the P4.31 fault matrix
//     (`ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md` §2)
//     and the P2.25 expected-quality plan — no new platform-behaviour
//     table is created here.
//   - NOTHING in this file has been executed: the Phase 5 loop bans
//     builds and test runs. The first
//     `cargo test -p flexcore-sim-runner --test fault_injection_p5_21`
//     run is the human's gate.

use flexcore_sim_runner::fault_injection::{
    build_fault_journal, run_fault_probes, ProbeOutcome, ScenarioFaultKind, ScenarioFaultPlan,
    P4_31_FAULT_TABLE_SOURCE, RESPONSE_CROSS_REFERENCED, RESPONSE_NOT_OBSERVED,
    SCENARIO_FAULTS_SLOT_SCHEMA,
};
use flexcore_sim_runner::scenario_run::{
    CommandRunConfig, GridBackendSelection, RecordedDropout, ScenarioRunHarness,
    SchedulingGridConfig,
};

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/synthetic-portfolio");
const SCHEDULE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7ab1";

fn read_scenario(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURE_DIR}/{name}"))
        .expect("the pinned scenario fixture exists in the repository")
}

/// The recorded composition defaults (the same values the CLI uses —
/// deployment configuration, D-P4.16-3, spelled out per the P5.14 suite
/// discipline).
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

fn schedule_id() -> flexcore_domain::ids::ScheduleId {
    flexcore_domain::ids::ScheduleId::new(
        flexcore_contracts::ids::FlexUuid::parse(SCHEDULE_UUID)
            .expect("the fixture schedule UUID is canonical"),
    )
}

/// The scenario JSON with the P2.15 fault plan ENABLED (the fault logic is
/// reachable only through configuration — never ad-hoc code paths, P2.15).
fn faults_enabled_scenario() -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&read_scenario("scenario-v1.json")).expect("the fixture is JSON");
    value["faults"]["enabled"] = serde_json::Value::Bool(true);
    serde_json::to_string(&value).expect("the modified scenario renders")
}

/// The scenario JSON carrying a `scenario_faults` v3 slot (faults
/// disabled). `start_ms` is the scenario's own start instant, so every
/// window is inside the horizon.
fn slot_scenario(start_ms: i64) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&read_scenario("scenario-v1.json")).expect("the fixture is JSON");
    value["faults"]["enabled"] = serde_json::Value::Bool(false);
    value["scenario_faults"] = serde_json::json!({
        "schema": SCENARIO_FAULTS_SLOT_SCHEMA,
        "enabled": true,
        "note": "P5.21 test slot — one spec per probed kind",
        "injections": [
            { "fault_id": "SF-SKEW-1", "kind": "clock_skew" },
            { "fault_id": "SF-FLEET-1", "kind": "partial_fleet_response",
              "from_ms": start_ms + 125 * 60_000, "until_ms": start_ms + 126 * 60_000 },
            { "fault_id": "SF-SOLVER-1", "kind": "solver_timeout" },
            { "fault_id": "SF-PLUGIN-1", "kind": "plugin_crash" },
            { "fault_id": "SF-DROP-MSG-1", "kind": "dropped_message",
              "asset_key": "site-north-bess-1",
              "from_ms": start_ms, "until_ms": start_ms + 60 * 60_000 },
            { "fault_id": "SF-STALE-1", "kind": "stale_telemetry",
              "from_ms": start_ms, "until_ms": start_ms + 60 * 60_000 }
        ]
    });
    serde_json::to_string(&value).expect("the slot scenario renders")
}

// ---------------------------------------------------------------------------
// 1. The v3 slot parses with every kind's pre-registered expectation.
// ---------------------------------------------------------------------------

#[test]
fn the_slot_parses_and_pre_registers_every_kind() {
    // The slot's windows need the scenario's start instant: read the
    // validated scenario once (via a harness-free plan parse on the base
    // fixture is not enough — parse the start from the artefact).
    let base = read_scenario("scenario-v1.json");
    let harness_start = {
        let harness = ScenarioRunHarness::new(
            &base,
            SchedulingGridConfig {
                interval_millis: 15 * 60_000,
            },
            GridBackendSelection::Scripted,
            None,
        )
        .expect("the base scenario composes");
        harness.scenario().start_ms
    };
    let plan = ScenarioFaultPlan::from_scenario_json(&slot_scenario(harness_start))
        .expect("a well-formed v3 slot validates");
    assert_eq!(plan.schema, SCENARIO_FAULTS_SLOT_SCHEMA);
    assert!(plan.enabled);
    assert_eq!(plan.specs.len(), 6);

    let by_id = |id: &str| {
        plan.specs
            .iter()
            .find(|spec| spec.fault_id == id)
            .unwrap_or_else(|| panic!("spec {id} missing"))
    };
    let skew = by_id("SF-SKEW-1");
    assert_eq!(skew.kind, ScenarioFaultKind::ClockSkew);
    assert_eq!(skew.expected_response, "reject");
    assert_eq!(skew.injection_leg, "composition_clock");
    assert!(skew.pre_registered_reference.contains("P4.31 F4"));

    let fleet = by_id("SF-FLEET-1");
    assert_eq!(fleet.kind, ScenarioFaultKind::PartialFleetResponse);
    assert_eq!(fleet.expected_response, "per_asset");
    assert_eq!(fleet.injection_leg, "fleet_probe");
    assert!(fleet.pre_registered_reference.contains("P4.31 F11"));

    let solver = by_id("SF-SOLVER-1");
    assert_eq!(solver.kind, ScenarioFaultKind::SolverTimeout);
    assert_eq!(solver.expected_response, "hold");
    assert!(solver.pre_registered_reference.contains("F9a"));

    let plugin = by_id("SF-PLUGIN-1");
    assert_eq!(plugin.kind, ScenarioFaultKind::PluginCrash);
    assert!(plugin.pre_registered_reference.contains("F10"));

    let drop = by_id("SF-DROP-MSG-1");
    assert_eq!(drop.kind, ScenarioFaultKind::DroppedMessage);
    assert_eq!(drop.expected_response, "failed");
    assert_eq!(drop.injection_leg, "dispatch_transport");
    assert!(drop.pre_registered_reference.contains("D-P4.16-4"));

    let stale = by_id("SF-STALE-1");
    assert_eq!(stale.kind, ScenarioFaultKind::StaleTelemetry);
    assert_eq!(stale.expected_response, "reject");
    assert!(stale.pre_registered_reference.contains("P4.31 F6"));
    // Every pre-registered reference names the reused Phase 4 source.
    for spec in &plan.specs {
        assert!(
            spec.pre_registered_reference.contains("P4.31") || spec.pre_registered_reference.contains("P2.15"),
            "spec {} must reference the reused table, got '{}'",
            spec.fault_id,
            spec.pre_registered_reference
        );
        assert!(!spec.expected_detail.trim().is_empty());
    }
    assert!(P4_31_FAULT_TABLE_SOURCE.contains("31_fault_shadow.md"));
}

// ---------------------------------------------------------------------------
// 2. The slot validator fails closed.
// ---------------------------------------------------------------------------

#[test]
fn the_slot_validator_fails_closed_on_unknown_fields_and_windows() {
    let base = read_scenario("scenario-v1.json");
    let mut value: serde_json::Value = serde_json::from_str(&base).unwrap();

    // Unknown schema → named error, never interpreted.
    value["scenario_faults"] = serde_json::json!({
        "schema": "flexcore-scenario-faults/v9-unknown", "enabled": true, "injections": []
    });
    let error = ScenarioFaultPlan::from_scenario_json(&serde_json::to_string(&value).unwrap())
        .expect_err("an unknown schema is refused");
    assert!(error.contains("schema"), "named error: {error}");

    // Unknown kind → named error (the vocabulary is closed).
    value["scenario_faults"] = serde_json::json!({
        "schema": SCENARIO_FAULTS_SLOT_SCHEMA, "enabled": true,
        "injections": [{ "fault_id": "SF-X", "kind": "kernel_panic" }]
    });
    let error = ScenarioFaultPlan::from_scenario_json(&serde_json::to_string(&value).unwrap())
        .expect_err("an unknown kind is refused");
    assert!(error.contains("unknown fault kind"), "named error: {error}");

    // Unknown asset → named error (the P2.15 validation discipline).
    value["scenario_faults"] = serde_json::json!({
        "schema": SCENARIO_FAULTS_SLOT_SCHEMA, "enabled": true,
        "injections": [{ "fault_id": "SF-X", "kind": "clock_skew", "asset_key": "ghost-asset" }]
    });
    let error = ScenarioFaultPlan::from_scenario_json(&serde_json::to_string(&value).unwrap())
        .expect_err("an unknown asset is refused");
    assert!(error.contains("not a scenario portfolio asset"), "named error: {error}");

    // Inverted window → named error.
    value["scenario_faults"] = serde_json::json!({
        "schema": SCENARIO_FAULTS_SLOT_SCHEMA, "enabled": true,
        "injections": [{ "fault_id": "SF-X", "kind": "clock_skew",
                         "from_ms": 100, "until_ms": 50 }]
    });
    let error = ScenarioFaultPlan::from_scenario_json(&serde_json::to_string(&value).unwrap())
        .expect_err("an inverted window is refused");
    assert!(error.contains("precedes"), "named error: {error}");
}

// ---------------------------------------------------------------------------
// 3. The 1:1 derivation from the validated P2.15 fault plan.
// ---------------------------------------------------------------------------

#[test]
fn the_derived_plan_maps_every_p2_15_injection_once() {
    let plan = ScenarioFaultPlan::from_scenario_json(&faults_enabled_scenario())
        .expect("the enabled P2.15 plan derives");
    assert!(plan.enabled);
    assert!(plan.schema.starts_with("derived:"));
    // The v1/v2 plan ships exactly seven injections; the derivation is 1:1
    // (no fault invented, none dropped).
    assert_eq!(plan.specs.len(), 7);
    let kinds: Vec<&str> = plan
        .specs
        .iter()
        .map(|spec| spec.kind.name())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "unavailable_asset",
            "stale_telemetry",
            "invalid_source",
            "duplicated_message",
            "reordered_message",
            "conflicting_duplicate",
            "out_of_range",
        ]
    );
    let responses: Vec<&str> = plan
        .specs
        .iter()
        .map(|spec| spec.expected_response.as_str())
        .collect();
    assert_eq!(
        responses,
        vec!["fallback", "reject", "reject", "hold", "hold", "hold", "failed"]
    );
    // Deterministic ids (never random), every target an asset key.
    for spec in &plan.specs {
        assert!(spec.fault_id.starts_with("p2-"), "id {}", spec.fault_id);
        assert!(spec.target_asset_key.is_some());
        assert!(spec.pre_registered_reference.contains("P4.31") || spec.pre_registered_reference.contains("P2.15"));
    }
}

// ---------------------------------------------------------------------------
// 4. The inert plan (no faults configured — none invented).
// ---------------------------------------------------------------------------

#[test]
fn the_inert_plan_invents_no_fault() {
    let plan = ScenarioFaultPlan::from_scenario_json(&read_scenario("scenario-v1.json"))
        .expect("the shipped-disabled plan is inert");
    assert!(!plan.enabled);
    assert!(plan.specs.is_empty());
    assert!(plan.statement.contains("inert"), "statement: {}", plan.statement);
}

// ---------------------------------------------------------------------------
// 5. The clock-skew probe: refused fail-closed past validity (P4.31 F4).
// ---------------------------------------------------------------------------

#[test]
fn the_clock_skew_probe_is_refused_fail_closed() {
    let base = read_scenario("scenario-v1.json");
    let harness = ScenarioRunHarness::new(
        &base,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the base scenario composes");
    let start = harness.scenario().start_ms;
    let plan =
        ScenarioFaultPlan::from_scenario_json(&slot_scenario(start)).expect("the slot validates");
    let config = command_config(&harness.scenario().sites[0].assets[0].asset_key);
    let mut outcomes = Vec::new();
    run_fault_probes(&harness, &plan, &config, &schedule_id(), 1, &mut outcomes);

    let skew: Vec<&ProbeOutcome> = outcomes
        .iter()
        .filter(|outcome| outcome.fault_id == "SF-SKEW-1")
        .collect();
    assert_eq!(skew.len(), 1, "exactly one clock-skew probe ran");
    let outcome = skew[0];
    // The pre-registered outcome: a fail-closed refusal (expired at issue,
    // or a named gate rejection) — NEVER an acceptance.
    assert_eq!(
        outcome.platform_response, "reject",
        "observed: {}",
        outcome.observed_effect
    );
    assert!(
        outcome.observed_effect.contains("refused")
            || outcome.observed_effect.contains("REJECTED")
            || outcome.observed_effect.contains("EXPIRED"),
        "the refusal must be named in the observation: {}",
        outcome.observed_effect
    );
    assert!(outcome.evidence_reference.starts_with("probe:clock-skew:"));
}

// ---------------------------------------------------------------------------
// 6. The partial-fleet probe: per-asset outcomes, never portfolio-inferred.
// ---------------------------------------------------------------------------

#[test]
fn the_partial_fleet_probe_records_per_asset_outcomes() {
    let harness = ScenarioRunHarness::new(
        &faults_enabled_scenario(),
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the fault-enabled scenario composes");
    let start = harness.scenario().start_ms;
    let plan =
        ScenarioFaultPlan::from_scenario_json(&slot_scenario(start)).expect("the slot validates");
    let config = command_config(&harness.scenario().sites[0].assets[0].asset_key);
    let mut outcomes = Vec::new();
    run_fault_probes(&harness, &plan, &config, &schedule_id(), 1, &mut outcomes);

    let fleet: Vec<&ProbeOutcome> = outcomes
        .iter()
        .filter(|outcome| outcome.fault_id == "SF-FLEET-1")
        .collect();
    assert_eq!(fleet.len(), 1, "exactly one fleet probe ran");
    let outcome = fleet[0];
    assert_eq!(outcome.platform_response, "per_asset");
    // One command per scenario asset (the v1 portfolio has ten).
    assert!(outcome.observed_effect.contains("(10)"), "{}", outcome.observed_effect);
    // The BESS is inside its enabled drop window at the probe instant: the
    // D-P4.16-4 mapping gives no acceptance — the command FAILED.
    assert!(
        outcome.observed_effect.contains("site-north-bess-1: state FAILED"),
        "the drop-window target must fail per the D-P4.16-4 mapping: {}",
        outcome.observed_effect
    );
    // Every asset's outcome is recorded individually (never inferred).
    assert!(outcome.observed_effect.contains("site-south-pv-1"));
    assert!(outcome.evidence_reference.contains("probe:fleet:SF-FLEET-1"));
}

// ---------------------------------------------------------------------------
// 7. The journal never records a silent pass.
// ---------------------------------------------------------------------------

#[test]
fn the_journal_never_records_a_silent_pass() {
    let harness = ScenarioRunHarness::new(
        &faults_enabled_scenario(),
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the fault-enabled scenario composes");
    let plan = ScenarioFaultPlan::from_scenario_json(&faults_enabled_scenario())
        .expect("the derived plan validates");
    // Complete the horizon so the record's quality accounting covers ALL
    // telemetry, then build the record WITHOUT dispatch commands (the
    // journal must stay factual about what THIS run observed).
    harness.run_to_horizon_end().expect("the horizon completes");
    let dropouts: Vec<RecordedDropout> = plan
        .specs
        .iter()
        .filter(|spec| spec.kind == ScenarioFaultKind::UnavailableAsset)
        .map(|spec| RecordedDropout {
            asset_key: spec.target_asset_key.clone().unwrap_or_default(),
            from_millis: spec.from_ms,
        })
        .collect();
    let record = harness
        .record("test-code", Vec::new(), Vec::new(), dropouts)
        .expect("the record builds");

    let journal = build_fault_journal(&plan, &record, &[]);
    assert_eq!(journal.entries.len(), plan.specs.len());

    // Never a silent pass: no entry claims "passed"; every unobserved
    // fault carries the not-observed marker WITH a reason.
    for entry in &journal.entries {
        assert!(
            !entry.platform_response.contains("passed"),
            "entry {} must never claim a pass",
            entry.fault_id
        );
        assert!(
            !entry.observed_effect.trim().is_empty(),
            "entry {} must never be a blank (the silent-blank-screen rule)",
            entry.fault_id
        );
        if entry.platform_response == RESPONSE_NOT_OBSERVED {
            assert!(
                entry.observed_effect.contains(RESPONSE_NOT_OBSERVED),
                "an unobserved fault names itself: {}",
                entry.fault_id
            );
        }
        assert!(
            !entry.pre_registered_reference.trim().is_empty(),
            "every entry names the reused expectation"
        );
    }

    // The unavailable-asset fault IS observed: the drop window produces the
    // recorded availability change with its evidence kind.
    let unavailable = journal
        .entries
        .iter()
        .find(|entry| entry.kind == "unavailable_asset")
        .expect("the drop spec has a journal entry");
    assert!(
        unavailable.observed_effect.contains("no frames exist"),
        "{}",
        unavailable.observed_effect
    );
    assert!(unavailable.evidence_reference.starts_with("availability_change:"));

    // The stale-telemetry fault IS observed (the delay window ages past the
    // freshness limit → STALE samples in the quality accounting).
    let stale = journal
        .entries
        .iter()
        .find(|entry| entry.kind == "stale_telemetry")
        .expect("the delay spec has a journal entry");
    assert!(
        stale.observed_effect.contains("STALE"),
        "{}",
        stale.observed_effect
    );
    assert_eq!(stale.evidence_reference, "quality_counts:STALE");

    // The source-level stored-quality faults are honest about WHERE their
    // pre-registered outcome is verified (the P2.25 coverage accounting —
    // not this run record).
    let duplicated = journal
        .entries
        .iter()
        .find(|entry| entry.kind == "duplicated_message")
        .expect("the duplicate spec has a journal entry");
    assert!(
        duplicated
            .observed_effect
            .contains("P2.25 expected_quality"),
        "{}",
        duplicated.observed_effect
    );
    assert!(journal.observed_count >= 2);
    assert!(journal.statement.contains("not_observed_in_this_run"));
}

// ---------------------------------------------------------------------------
// 8. Cross-references and probe outcomes in the journal.
// ---------------------------------------------------------------------------

#[test]
fn the_journal_cross_references_instead_of_duplicating_phase_4() {
    let base = read_scenario("scenario-v1.json");
    let harness = ScenarioRunHarness::new(
        &base,
        SchedulingGridConfig {
            interval_millis: 15 * 60_000,
        },
        GridBackendSelection::Scripted,
        None,
    )
    .expect("the base scenario composes");
    let start = harness.scenario().start_ms;
    let plan =
        ScenarioFaultPlan::from_scenario_json(&slot_scenario(start)).expect("the slot validates");
    let record = harness
        .record("test-code", Vec::new(), Vec::new(), Vec::new())
        .expect("the record builds");

    // No probes ran: the slot's clock-skew and fleet faults are honest
    // not-observed entries; solver timeout and plugin crash are
    // cross-references with NO fabricated outcome.
    let journal = build_fault_journal(&plan, &record, &[]);
    let solver = journal
        .entries
        .iter()
        .find(|entry| entry.kind == "solver_timeout")
        .expect("the solver spec has a journal entry");
    assert_eq!(solver.platform_response, RESPONSE_CROSS_REFERENCED);
    assert!(
        solver.observed_effect.contains("not injected at the scenario level"),
        "{}",
        solver.observed_effect
    );
    assert!(solver.evidence_reference.contains("F9a"));
    let plugin = journal
        .entries
        .iter()
        .find(|entry| entry.kind == "plugin_crash")
        .expect("the plugin spec has a journal entry");
    assert_eq!(plugin.platform_response, RESPONSE_CROSS_REFERENCED);
    assert!(plugin.observed_effect.contains("no plugin kernel exists"));
    assert_eq!(journal.cross_referenced_count, 2);

    // An explicit probe outcome wins over the mined facts — and it is
    // rendered verbatim, never re-interpreted into a pass.
    let probe = ProbeOutcome {
        fault_id: "SF-SKEW-1".to_string(),
        observed_effect: "the probe's issue attempt was refused fail-closed (expired)"
            .to_string(),
        platform_response: "reject".to_string(),
        evidence_reference: "probe:clock-skew:cmd-1".to_string(),
    };
    let journal = build_fault_journal(&plan, &record, &[probe]);
    let skew = journal
        .entries
        .iter()
        .find(|entry| entry.fault_id == "SF-SKEW-1")
        .expect("the skew spec has a journal entry");
    assert_eq!(skew.platform_response, "reject");
    assert!(skew.observed_effect.contains("refused fail-closed"));
    assert_eq!(skew.evidence_reference, "probe:clock-skew:cmd-1");
    // The journal counts it as observed (it is a direct observation).
    assert!(journal.observed_count >= 1);
    assert!(
        !journal
            .entries
            .iter()
            .any(|entry| entry.platform_response == RESPONSE_NOT_OBSERVED
                && entry.observed_effect.trim().is_empty()),
        "no blank entries anywhere"
    );
}
