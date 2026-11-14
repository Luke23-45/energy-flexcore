// P5.16 — The HELICS co-simulation orchestration suite (ledger P5.16).
//
// ══════════════════════════════════════════════════════════════════════════
// PRE-REGISTERED EXPECTATIONS, UNRUN. The Phase 5 loop bans build/run:
// these tests were NEVER compiled or executed. They use the scripted
// federate launcher (a composition double over the `FederateLauncher`
// seam) — NO helics is installed, no broker is ever started, no federate
// process is ever spawned. Every assertion below is the recorded
// expectation the human's `cargo test --workspace` pass must satisfy,
// not evidence that anything passed.
//
// What the suite pins (the fail-closed contract):
// 1. config validation — protocol tag, pinned time step, stop time,
//    broker address shape, the three-federate minimum, unique names,
//    unique publication topics, every subscription matched to exactly
//    one publication, canonical units, `double` kind, per-role options;
// 2. the launch plan — broker FIRST, then every federate, each with the
//    config path + federate name (the config file is the single source
//    of federation truth);
// 3. exit-report parsing — protocol tag, kind, process identity, ok,
//    finiteness (fail-closed on every violation);
// 4. execution composition over the scripted launcher — the all-ok
//    federation verifies time synchronisation ONLY when every federate's
//    final granted time equals the stop time with equal step counts; any
//    failure (timeout, missing output, contract violation) is recorded
//    fail-closed and can NEVER verify the sync (no path from a federate
//    failure to a pass — the P4.08 policy);
// 5. the co-sim record — `composed` vs `executed` honesty, the recorded
//    statements (role, validation boundary, UNVERIFIED-API, opt-in, sign
//    convention), and the topology summary arrows.
// ══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;
use std::time::Duration;

use flexcore_sim_runner::helics_orchestration::{
    compose_record, execute_federation, execution_record, launch_plan, parse_exit_report,
    parse_federation_config, validate_config, FederateLauncher, FederateProcessOutput,
    HelicsFederationConfig, HelicsFederateRole, HelicsFederateSpec, HelicsOrchestrationError,
    ProcessKind,
};

// --- Fixtures ------------------------------------------------------------------

fn endpoint(topic: &str, unit: &str) -> flexcore_sim_runner::helics_orchestration::HelicsEndpointSpec {
    flexcore_sim_runner::helics_orchestration::HelicsEndpointSpec {
        topic: topic.to_string(),
        unit: unit.to_string(),
        kind: "double".to_string(),
    }
}

fn asset_federate() -> HelicsFederateSpec {
    HelicsFederateSpec {
        name: "flexcore_asset".to_string(),
        role: HelicsFederateRole::Asset,
        script_path: "apps/worker/python/helics_federates/asset_federate.py".to_string(),
        interpreter: "python".to_string(),
        options: BTreeMap::from([
            (
                "input_records_path".to_string(),
                serde_json::Value::String("scenarios/synthetic-portfolio/replay-records.json".to_string()),
            ),
            (
                "asset_key_map".to_string(),
                serde_json::json!({"site-north-bess-1": "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01"}),
            ),
        ]),
        publications: vec![
            endpoint("flexcore/asset/site-north-bess-1/p_mw", "MW"),
            endpoint("flexcore/asset/site-north-bess-1/q_mvar", "MVAr"),
        ],
        subscriptions: vec![endpoint(
            "flexcore/control/site-north-bess-1/setpoint_p_mw",
            "MW",
        )],
    }
}

fn grid_federate() -> HelicsFederateSpec {
    HelicsFederateSpec {
        name: "flexcore_grid".to_string(),
        role: HelicsFederateRole::Grid,
        script_path: "apps/worker/python/helics_federates/grid_federate.py".to_string(),
        interpreter: "python".to_string(),
        options: BTreeMap::from([
            (
                "grid_worker_script".to_string(),
                serde_json::Value::String("apps/worker/python/grid_worker.py".to_string()),
            ),
            (
                "model_ir_path".to_string(),
                serde_json::Value::String("scenarios/synthetic-portfolio/grid-model.json".to_string()),
            ),
            (
                "bus_map".to_string(),
                serde_json::json!({"site-north-bess-1": "bus_2"}),
            ),
        ]),
        publications: vec![endpoint("flexcore/grid/bus/bus_2/vm_pu", "pu")],
        subscriptions: vec![
            endpoint("flexcore/asset/site-north-bess-1/p_mw", "MW"),
            endpoint("flexcore/asset/site-north-bess-1/q_mvar", "MVAr"),
        ],
    }
}

fn control_federate() -> HelicsFederateSpec {
    HelicsFederateSpec {
        name: "flexcore_control".to_string(),
        role: HelicsFederateRole::Control,
        script_path: "apps/worker/python/helics_federates/control_federate.py".to_string(),
        interpreter: "python".to_string(),
        options: BTreeMap::from([(
            "setpoints_path".to_string(),
            serde_json::Value::String("scenarios/synthetic-portfolio/setpoints-fixture.json".to_string()),
        )]),
        publications: vec![endpoint(
            "flexcore/control/site-north-bess-1/setpoint_p_mw",
            "MW",
        )],
        subscriptions: vec![
            endpoint("flexcore/asset/site-north-bess-1/p_mw", "MW"),
            endpoint("flexcore/grid/bus/bus_2/vm_pu", "pu"),
        ],
    }
}

fn base_config() -> HelicsFederationConfig {
    HelicsFederationConfig {
        protocol: "flexcore-helics-federation/1".to_string(),
        federation_name: "flexcore-phase5-cosim".to_string(),
        broker: flexcore_sim_runner::helics_orchestration::HelicsBrokerConfig {
            address: "tcp://127.0.0.1:23404".to_string(),
            expected_federates: 3,
            startup_timeout_seconds: 60.0,
            script_path: "apps/worker/python/helics_federates/broker.py".to_string(),
            program: "python".to_string(),
        },
        time_step_seconds: 15.0,
        stop_time_seconds: 3600.0,
        federates: vec![asset_federate(), grid_federate(), control_federate()],
    }
}

fn base_config_json() -> String {
    serde_json::to_string(&base_config()).expect("the base config serializes")
}

/// The scripted launcher: the composition double over the
/// `FederateLauncher` seam — pre-scripted outputs per label, NO process
/// spawned, NO helics anywhere.
struct ScriptedFederateLauncher {
    scripted: BTreeMap<String, FederateProcessOutput>,
}

impl ScriptedFederateLauncher {
    fn all_ok() -> Self {
        let mut scripted = BTreeMap::new();
        scripted.insert(
            "broker".to_string(),
            FederateProcessOutput {
                label: "broker".to_string(),
                stdout: format!(
                    r#"{{"protocol":"flexcore-helics-federate/1","kind":"broker","federation":"flexcore-phase5-cosim","federate":"broker","role":null,"granted_steps":null,"final_granted_time_seconds":null,"published_values":null,"received_values":null,"skipped_publications":null,"ok":true,"status_text":"coordination only","helics_version_reported":"3.7.0"}}"#
                ),
                stderr: String::new(),
                exit_ok: true,
                timed_out: false,
            },
        );
        for (name, role) in [
            ("flexcore_asset", "asset"),
            ("flexcore_grid", "grid"),
            ("flexcore_control", "control"),
        ] {
            scripted.insert(
                name.to_string(),
                FederateProcessOutput {
                    label: name.to_string(),
                    stdout: report_json(name, role, 240, 3600.0),
                    stderr: String::new(),
                    exit_ok: true,
                    timed_out: false,
                },
            );
        }
        Self { scripted }
    }

    fn with_failure(mut self, label: &str, output: FederateProcessOutput) -> Self {
        self.scripted.insert(label.to_string(), output);
        self
    }
}

impl FederateLauncher for ScriptedFederateLauncher {
    fn run_all(&mut self, specs: &[flexcore_sim_runner::helics_orchestration::FederateProcessSpec], _timeout: Duration) -> Vec<FederateProcessOutput> {
        specs
            .iter()
            .map(|spec| {
                self.scripted.get(&spec.label).cloned().unwrap_or_else(|| {
                    // Not scripted ⇒ emulate a process that produced
                    // NOTHING (the missing-output fail-closed branch).
                    FederateProcessOutput {
                        label: spec.label.clone(),
                        stdout: String::new(),
                        stderr: "scripted: no output for this process (missing-output emulation)"
                            .to_string(),
                        exit_ok: true,
                        timed_out: false,
                    }
                })
            })
            .collect()
    }
}

fn report_json(federate: &str, role: &str, steps: u64, final_time: f64) -> String {
    format!(
        r#"{{"protocol":"flexcore-helics-federate/1","kind":"federate","federation":"flexcore-phase5-cosim","federate":"{federate}","role":"{role}","granted_steps":{steps},"final_granted_time_seconds":{final_time},"published_values":4,"received_values":3,"skipped_publications":0,"ok":true,"status_text":"HELICS coordinates; it does not replace physics","helics_version_reported":"3.7.0"}}"#
    )
}

// 1. The well-formed base config validates.
#[test]
fn a_well_formed_three_federate_config_validates() {
    let config = base_config();
    validate_config(&config).expect("the base config is well-formed");
    // The round trip through the wire document validates identically.
    let parsed = parse_federation_config(&base_config_json()).expect("the JSON form parses");
    assert_eq!(parsed, config);
}

// 2. The protocol tag is fail-closed.
#[test]
fn a_wrong_protocol_tag_is_a_config_error() {
    let mut config = base_config();
    config.protocol = "other/1".to_string();
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
    let json = base_config_json().replace("flexcore-helics-federation/1", "other/1");
    assert!(parse_federation_config(&json).is_err());
}

// 3. The pinned time step is fail-closed (zero / negative / non-finite),
//    and the stop time must reach at least one step.
#[test]
fn a_non_positive_or_non_finite_time_step_is_a_config_error() {
    let mut config = base_config();
    config.time_step_seconds = 0.0;
    assert!(validate_config(&config).is_err());
    config.time_step_seconds = -15.0;
    assert!(validate_config(&config).is_err());
    config.time_step_seconds = f64::NAN;
    assert!(validate_config(&config).is_err());
    let mut config = base_config();
    config.stop_time_seconds = 14.9;
    assert!(validate_config(&config).is_err());
}

// 4. The three-federate minimum: dropping the control federate is an
//    error (report P5.16 step 1: grid + asset/end-use + FlexCore/control).
#[test]
fn a_missing_role_violates_the_three_federate_minimum() {
    let mut config = base_config();
    config.federates.retain(|spec| spec.role != HelicsFederateRole::Control);
    config.broker.expected_federates = config.federates.len() as u32;
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
}

// 5. Federate names are unique; publication topics are globally unique.
#[test]
fn duplicate_names_and_duplicate_publication_topics_are_errors() {
    let mut config = base_config();
    config.federates[2].name = "flexcore_asset".to_string();
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
    let mut config = base_config();
    // The control federate publishes a topic the asset federate already
    // publishes.
    config.federates[2].publications.push(endpoint(
        "flexcore/asset/site-north-bess-1/p_mw",
        "MW",
    ));
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
}

// 6. Every subscription must resolve to exactly one publication.
#[test]
fn a_subscription_without_a_matching_publication_is_an_error() {
    let mut config = base_config();
    config.federates[0].subscriptions.push(endpoint(
        "flexcore/nowhere/nothing",
        "MW",
    ));
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
}

// 7. Units are canonical (MW, MVAr, MVA, MWh, kV, A, pu, percent) and
//    the kind is `double`.
#[test]
fn a_non_canonical_unit_or_kind_is_an_error() {
    let mut config = base_config();
    config.federates[0].publications[0].unit = "horsepower".to_string();
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
    let mut config = base_config();
    config.federates[0].publications[0].kind = "string".to_string();
    let error = validate_config(&config).unwrap_err();
    assert!(matches!(error, HelicsOrchestrationError::Config(_)));
}

// 8. The broker address is deployment configuration and is validated
//    (shape, scheme, port); the expected-federate count must match.
#[test]
fn broker_address_and_count_are_validated() {
    for address in ["", "127.0.0.1:23404", "smtp://127.0.0.1", "tcp://:23404", "tcp://127.0.0.1:notaport"] {
        let mut config = base_config();
        config.broker.address = address.to_string();
        let error = validate_config(&config).unwrap_err();
        assert!(matches!(error, HelicsOrchestrationError::Config(_)), "address {address:?}");
    }
    let mut config = base_config();
    config.broker.expected_federates = 5;
    assert!(validate_config(&config).is_err());
}

// 9. The per-role options: each federate must point at its REAL data
//    input (records feed / worker+IR+bus map / setpoint fixture) — never
//    at invented data.
#[test]
fn role_options_are_required() {
    let mut config = base_config();
    config.federates[0].options.remove("input_records_path");
    assert!(validate_config(&config).is_err());
    let mut config = base_config();
    config.federates[1].options.remove("bus_map");
    assert!(validate_config(&config).is_err());
    let mut config = base_config();
    config.federates[1].options.remove("grid_worker_script");
    assert!(validate_config(&config).is_err());
    let mut config = base_config();
    config.federates[1].options.remove("model_ir_path");
    assert!(validate_config(&config).is_err());
    let mut config = base_config();
    config.federates[2].options.remove("setpoints_path");
    assert!(validate_config(&config).is_err());
}

// 10. The launch plan: broker FIRST, then every federate in config
//     order; every arg list carries the config path + the federate
//     name; nothing spawns here.
#[test]
fn the_launch_plan_composes_broker_first_then_federates() {
    let config = base_config();
    let plan = launch_plan(&config, "federation.json");
    assert_eq!(plan.len(), 4);
    assert_eq!(plan[0].label, "broker");
    assert_eq!(plan[0].kind, ProcessKind::Broker);
    assert_eq!(plan[0].program, "python");
    assert!(plan[0].args.contains(&"federation.json".to_string()));
    for (spec, expected_role) in plan[1..].iter().zip([
        (HelicsFederateRole::Asset, "flexcore_asset"),
        (HelicsFederateRole::Grid, "flexcore_grid"),
        (HelicsFederateRole::Control, "flexcore_control"),
    ]) {
        assert_eq!(spec.kind, ProcessKind::Federate(expected_role.0));
        assert_eq!(spec.label, expected_role.1);
        assert!(spec.args.contains(&"federation.json".to_string()));
        assert!(spec.args.contains(&"--federate".to_string()));
        assert!(spec.args.contains(&spec.label.clone()));
    }
}

// 11. Exit-report parsing: the contract-shaped report parses; every
//     violation (tag, kind, identity, ok:false, non-finite) is
//     fail-closed.
#[test]
fn exit_reports_are_validated_fail_closed() {
    let report = parse_exit_report(
        &report_json("flexcore_asset", "asset", 240, 3600.0),
        "flexcore_asset",
        "federate",
    )
    .expect("the contract-shaped report parses");
    assert_eq!(report.granted_steps, Some(240));
    assert_eq!(report.final_granted_time_seconds, Some(3600.0));

    // Empty output, not JSON, wrong tag, wrong kind, wrong identity.
    assert!(parse_exit_report("", "flexcore_asset", "federate").is_err());
    assert!(parse_exit_report("not json", "flexcore_asset", "federate").is_err());
    let wrong_tag = report_json("flexcore_asset", "asset", 240, 3600.0)
        .replace("flexcore-helics-federate/1", "other/1");
    assert!(parse_exit_report(&wrong_tag, "flexcore_asset", "federate").is_err());
    let wrong_kind = report_json("flexcore_asset", "asset", 240, 3600.0)
        .replace(r#""kind":"federate""#, r#""kind":"broker""#);
    assert!(parse_exit_report(&wrong_kind, "flexcore_asset", "federate").is_err());
    let wrong_name = report_json("someone_else", "asset", 240, 3600.0);
    assert!(parse_exit_report(&wrong_name, "flexcore_asset", "federate").is_err());
    let ok_false = report_json("flexcore_asset", "asset", 240, 3600.0).replace(r#""ok":true"#, r#""ok":false"#);
    assert!(parse_exit_report(&ok_false, "flexcore_asset", "federate").is_err());
    let non_finite_time = format!(
        r#"{{"protocol":"flexcore-helics-federate/1","kind":"federate","federation":"f","federate":"flexcore_asset","role":"asset","granted_steps":1,"final_granted_time_seconds":1e999,"published_values":0,"received_values":0,"skipped_publications":0,"ok":true,"status_text":"s","helics_version_reported":null}}"#
    );
    assert!(parse_exit_report(&non_finite_time, "flexcore_asset", "federate").is_err());
}

// 12. Execution composition over the scripted launcher: the all-ok
//     federation verifies the time synchronisation (final granted times
//     == stop time, equal step counts) and the record says `executed`.
#[test]
fn an_all_ok_federation_verifies_time_sync_and_records_executed() {
    let config = base_config();
    let mut launcher = ScriptedFederateLauncher::all_ok();
    let execution = execute_federation(
        &config,
        "federation.json",
        &mut launcher as &mut dyn FederateLauncher,
        Duration::from_secs(60),
    );
    assert!(execution.execution_ok);
    assert!(execution.time_sync_verified);
    assert!(execution.broker.report.is_some());
    assert_eq!(execution.federates.len(), 3);
    assert!(execution.federates.iter().all(|outcome| outcome.report.is_some()));

    let record = execution_record(&config, &execution);
    assert_eq!(record.execution_mode, "executed");
    assert_eq!(record.execution_ok, Some(true));
    assert_eq!(record.time_sync_verified, Some(true));
    // The honesty statements travel with the record (role, validation
    // boundary, UNVERIFIED API, opt-in, sign convention).
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("does NOT replace")));
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("UNVERIFIED")));
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("WITHOUT HELICS")));
    assert!(record
        .statements
        .iter()
        .any(|statement| statement.contains("P > 0 means export")));
}

// 13. A federate that hits the bounded wait is fail-closed and can
//     NEVER verify the sync (no path from a federate failure to a pass).
#[test]
fn a_timed_out_federate_is_fail_closed_and_never_verifies() {
    let config = base_config();
    let mut launcher = ScriptedFederateLauncher::all_ok().with_failure(
        "flexcore_grid",
        FederateProcessOutput {
            label: "flexcore_grid".to_string(),
            stdout: String::new(),
            stderr: "the process did not answer within 60s; killed — fail-closed".to_string(),
            exit_ok: false,
            timed_out: true,
        },
    );
    let execution = execute_federation(
        &config,
        "federation.json",
        &mut launcher as &mut dyn FederateLauncher,
        Duration::from_secs(60),
    );
    assert!(!execution.execution_ok);
    assert!(!execution.time_sync_verified);
    let grid = execution
        .federates
        .iter()
        .find(|outcome| outcome.label == "flexcore_grid")
        .expect("the grid outcome is present");
    assert!(grid.report.is_none());
    assert!(grid.timed_out);
    assert!(grid.fail_closed_reason.is_some());
    let record = execution_record(&config, &execution);
    assert_eq!(record.execution_ok, Some(false));
    assert_eq!(record.time_sync_verified, Some(false));
    // The fail-closed reason travels into the record verbatim.
    let grid_trace = record
        .federates
        .iter()
        .find(|trace| trace.name == "flexcore_grid")
        .expect("the grid trace is present");
    let outcome = grid_trace.execution.as_ref().expect("the outcome is embedded");
    assert!(outcome.fail_closed_reason.as_deref().unwrap_or("").contains("fail-closed"));
}

// 14. A missing output (unscripted process = spawn-failure/missing-output
//     emulation) is fail-closed too.
#[test]
fn a_process_that_produces_no_output_is_fail_closed() {
    let config = base_config();
    let mut launcher = ScriptedFederateLauncher::all_ok();
    launcher.scripted.remove("flexcore_control");
    let execution = execute_federation(
        &config,
        "federation.json",
        &mut launcher as &mut dyn FederateLauncher,
        Duration::from_secs(60),
    );
    assert!(!execution.execution_ok);
    assert!(!execution.time_sync_verified);
    let control = execution
        .federates
        .iter()
        .find(|outcome| outcome.label == "flexcore_control")
        .expect("the control outcome is present");
    assert!(control.fail_closed_reason.is_some());
}

// 15. A final granted time that stops short of the stop time, or
//     diverging step counts, are recorded as NOT time-synchronised —
//     never coerced into a pass.
#[test]
fn short_final_times_and_diverging_step_counts_fail_time_sync() {
    let config = base_config();
    let mut launcher = ScriptedFederateLauncher::all_ok();
    // The grid federate stops one step short.
    launcher.scripted.insert(
        "flexcore_grid".to_string(),
        FederateProcessOutput {
            label: "flexcore_grid".to_string(),
            stdout: report_json("flexcore_grid", "grid", 239, 3585.0),
            stderr: String::new(),
            exit_ok: true,
            timed_out: false,
        },
    );
    let execution = execute_federation(
        &config,
        "federation.json",
        &mut launcher as &mut dyn FederateLauncher,
        Duration::from_secs(60),
    );
    assert!(!execution.execution_ok);
    assert!(!execution.time_sync_verified);

    // Equal short stop time but diverging step counts.
    let mut launcher = ScriptedFederateLauncher::all_ok();
    launcher.scripted.insert(
        "flexcore_grid".to_string(),
        FederateProcessOutput {
            label: "flexcore_grid".to_string(),
            stdout: report_json("flexcore_grid", "grid", 241, 3600.0),
            stderr: String::new(),
            exit_ok: true,
            timed_out: false,
        },
    );
    let execution = execute_federation(
        &config,
        "federation.json",
        &mut launcher as &mut dyn FederateLauncher,
        Duration::from_secs(60),
    );
    assert!(!execution.time_sync_verified);
}

// 16. The composed record (configured but NOT executed) is honest about
//     it: `composed`, no execution booleans, topology arrows verbatim.
#[test]
fn the_composed_record_names_the_topology_and_never_claims_a_run() {
    let config = base_config();
    let record = compose_record(&config);
    assert_eq!(record.execution_mode, "composed");
    assert_eq!(record.execution_ok, None);
    assert_eq!(record.time_sync_verified, None);
    assert_eq!(record.time_step_seconds, 15.0);
    assert_eq!(record.broker_address, "tcp://127.0.0.1:23404");
    // The topology summary names the publication → subscriber arrows.
    assert!(record.topology_summary.contains(
        "flexcore/control/site-north-bess-1/setpoint_p_mw (MW)--> flexcore_asset"
    ));
    assert!(record.topology_summary.contains(
        "flexcore/asset/site-north-bess-1/p_mw (MW)--> flexcore_grid, flexcore_control"
    ));
    // The version pin carries its provenance, never a bare number.
    assert!(record.helics_version_pin.contains("3.7.0"));
    assert!(record.helics_version_pin.contains("NEVER installed"));
    assert!(record.helics_version_pin.contains("RESEARCH.md open task 3"));
}
