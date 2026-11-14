// P7.43 — Scenario-regression suite for the SDK (ledger P7.43; report §22.2,
// §19.2(5), §24). WRITTEN, NEVER RUN — the loop bans hold; the human
// build/test pass and the `sdk-scenario-regression` CI job execute these
// (`cargo test -p flexcore-kernel --test scenario_regression_p7_43`).
//
// WHAT THIS IS: the SDK leg of the platform scenario-regression pattern
// (P5.20 `scenarios/library/` + P6.27 edge wiring — the PATTERN is reused,
// no scenario is duplicated across legs, and no scenario re-runs a unit
// suite). The scenarios are DATA: pinned-input JSON files under
// `plugins/scenarios/` (format documented in `plugins/scenarios/README.md`),
// embedded here at compile time and executed by this small in-test
// interpreter over the PUBLIC kernel APIs. Every step's expected outcome is
// a NAMED verdict — a named refusal from the recorded error tables, an exact
// structural equality (registration order, diff counts, applied-step count,
// resolved view), or an exact lifecycle/path-outcome state name. An empty or
// mutated suite cannot be "green": a scenario whose expectations no longer
// match the kernel's named outcomes fails CI and blocks the merge.
//
// SUITE VERSION: `flexcore-plugin-sdk-scenario-regression-v1` (recorded per
// run by the CI job; asserted per scenario below).
//
// DELIBERATE SCOPE (recorded; see `plugins/scenarios/README.md` §"Deliberate
// exclusions"): the scenarios exercise the CLOSED kernel APIs only. The
// P7.38/P7.39 revocation/tenant-scope extension scenarios are NOT written —
// at authoring time the concurrent agent's revocation.rs/tenant_scope.rs
// files were not on disk (checked), and scenarios must never be written
// against a PLANNED API shape. They are recorded as a follow-up for the
// coordinator in
// `ledger/phase-07-plugin-sdk/41_42_43_hardening_determinism_regression.md`
// §6. Class B/C execution legs (WASM instantiation, gRPC workers) belong to
// the P7.21/P7.22 conformance and swap suites and the P7.28 exit-gate run,
// not here.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_kernel::lifecycle::{
    DrainOutcome, DrainPolicy, DrainPort, FixedClock, GateOutcome, MemoryAuditSink,
    PluginLifecycle,
};
use flexcore_kernel::manifest::{parse_manifest_value, Manifest, ManifestError};
use flexcore_kernel::plugin_config::{resolve_for_instance, ConfigError};
use flexcore_kernel::plugin_path_protection::{
    FilePluginSequenceCheckpoint, PluginPathError, PluginPathOutcome, PluginPathVerifier,
    PluginProposalEnvelope, PluginRequestEnvelope, PluginResponseEnvelope,
};
use flexcore_kernel::profile::{parse_profile_value, validate_profile, ControlAuthorityPosture,
    Profile, ProfileError};
use flexcore_kernel::recomposition::{
    diff_plans, execute_switch, CompositionExecutor, RecompositionError, SwitchConfirmation,
    SwitchRequest,
};
use flexcore_kernel::resolve::{resolve, ExplicitSelections, ResolveError};

/// The suite id every CI run records (P7.43 step 2; mirrored by the
/// `sdk-scenario-regression` CI job's version-log step).
const SUITE_ID: &str = "flexcore-plugin-sdk-scenario-regression-v1";

/// The scenario files, embedded at compile time (the pinned bytes are part
/// of the test binary — a scenario change is a build-visible change).
const SCENARIO_SOURCES: [&str; 4] = [
    include_str!("../../../plugins/scenarios/sdk-01-manifest-to-plan.json"),
    include_str!("../../../plugins/scenarios/sdk-02-profile-switch-with-drain.json"),
    include_str!("../../../plugins/scenarios/sdk-03-config-drift-refusal.json"),
    include_str!("../../../plugins/scenarios/sdk-04-proposal-envelope-replay-refused.json"),
];

const BASE_MS: u64 = 1_700_000_000_000;

// --- The named-error plumbing (every refusal is a NAMED error) -------------------

/// Every error type a scenario step can produce reports its recorded stable
/// machine name — the property the refusal expectations pin.
trait NamedError {
    fn named(&self) -> &'static str;
}

macro_rules! impl_named_error {
    ($($error:ty),+ $(,)?) => {
        $(impl NamedError for $error {
            fn named(&self) -> &'static str {
                self.name()
            }
        })+
    };
}

impl_named_error!(
    ManifestError,
    ResolveError,
    ProfileError,
    ConfigError,
    LifecycleError,
    RecompositionError,
    PluginPathError,
);

/// Runs one step's expectation against a `Result`. Returns `Some(value)`
/// when an `ok` outcome was expected and produced; `None` when a `refused`
/// outcome was expected and matched BY NAME. Any mismatch fails the scenario.
fn expect_step<T, E: NamedError>(
    scenario_id: &str,
    step_index: usize,
    expect: &serde_json::Value,
    result: Result<T, E>,
) -> Option<T> {
    let what = format!("scenario '{scenario_id}' step {step_index}");
    match expect["outcome"].as_str() {
        Some("ok") => match result {
            Ok(value) => Some(value),
            Err(err) => panic!(
                "{what}: expected ok, was refused with '{}' ({err})",
                err.named()
            ),
        },
        Some("refused") => {
            let expected = expect["name"]
                .as_str()
                .unwrap_or_else(|| panic!("{what}: a refused expectation must carry a name"));
            match result {
                Ok(_) => panic!("{what}: expected refusal '{expected}', was ok"),
                Err(err) => {
                    assert_eq!(
                        err.named(),
                        expected,
                        "{what}: refusal name mismatch (got '{}', scenario pins '{expected}')",
                        err.named()
                    );
                    None
                }
            }
        }
        other => panic!("{what}: invalid expect.outcome {other:?}"),
    }
}

// --- The scripted ports (drain port + composition executor) ----------------------

/// What the next drain reports (driven by the scenario's `set-drain-script`
/// steps — the scripted stand-in for a real plugin's `DrainPort`).
#[derive(Debug, Clone, Default)]
struct DrainScript {
    quiesce: bool,
    outstanding: usize,
    cancelled: usize,
}

/// The scenario's drain port: reports the shared script (the Rc<RefCell>
/// lets `set-drain-script` reprogram the port while the lifecycle holds the
/// port borrow — the lifecycle keeps `&mut dyn DrainPort` for its whole life
/// once `begin_drain` runs).
struct ScriptedDrainPort {
    script: Rc<RefCell<DrainScript>>,
}

impl ScriptedDrainPort {
    fn new(script: Rc<RefCell<DrainScript>>) -> ScriptedDrainPort {
        ScriptedDrainPort { script }
    }
}

impl DrainPort for ScriptedDrainPort {
    fn outstanding_work(&self) -> usize {
        self.script.borrow().outstanding
    }

    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        let script = self.script.borrow().clone();
        if script.quiesce {
            DrainOutcome::Quiesced { cancelled: script.cancelled }
        } else {
            DrainOutcome::DeadlineExceeded { outstanding: script.outstanding }
        }
    }
}

/// The scenario's composition executor: every step succeeds (the faithful
/// implementation of prepare/drain/replace/remove/add for the composition
/// seam; the drain mechanics themselves are exercised through the lifecycle
/// steps above).
struct ScriptedExecutor;

impl CompositionExecutor for ScriptedExecutor {
    fn prepare_provider(
        &mut self,
        _contract_id: &str,
        _plugin_id: &str,
        _version: &str,
    ) -> Result<(), RecompositionError> {
        Ok(())
    }

    fn drain_to_stopped(&mut self, _plugin_id: &str) -> Result<(), RecompositionError> {
        Ok(())
    }

    fn replace_provider(
        &mut self,
        _contract_id: &str,
        _new_plugin_id: &str,
        _new_version: &str,
    ) -> Result<(), RecompositionError> {
        Ok(())
    }

    fn remove_provider(&mut self, _contract_id: &str) -> Result<(), RecompositionError> {
        Ok(())
    }

    fn add_provider(
        &mut self,
        _contract_id: &str,
        _plugin_id: &str,
        _version: &str,
    ) -> Result<(), RecompositionError> {
        Ok(())
    }
}

/// A unique scratch path per call (std-only; no tempfile dep — the kernel
/// test pattern).
fn scratch_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "flexcore-kernel-p743-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

// --- The interpreter --------------------------------------------------------------

/// Executes one embedded scenario end-to-end against the public kernel APIs.
fn run_scenario(scenario: &serde_json::Value) {
    let scenario_id = scenario["scenario-id"]
        .as_str()
        .expect("every scenario carries a scenario-id");
    let inputs = &scenario["inputs"];

    // Pinned inputs (the scenario bytes ARE the pins; the interpreter only
    // indexes them).
    let manifest_inputs = inputs["manifests"]
        .as_array()
        .expect("every scenario declares an inputs.manifests array");
    let profile_inputs = inputs["profiles"].as_array().expect("inputs.profiles array");
    let config_inputs: Vec<serde_json::Value> = inputs["configs"]
        .as_array()
        .map(|items| items.clone())
        .unwrap_or_default();

    // Runtime state.
    let mut manifests: Vec<Option<Manifest>> = vec![None; manifest_inputs.len()];
    let mut profiles: Vec<Option<Profile>> = vec![None; profile_inputs.len()];
    let mut plans: BTreeMap<String, flexcore_kernel::resolve::CompositionPlan> = BTreeMap::new();
    let mut resolved: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut envelopes: BTreeMap<String, PluginRequestEnvelope> = BTreeMap::new();

    let clock = FixedClock::new(BASE_MS);
    let audit = MemoryAuditSink::new();
    let drain_script = Rc::new(RefCell::new(DrainScript::default()));
    let mut drain_port = ScriptedDrainPort::new(drain_script.clone());
    let mut lifecycle: Option<PluginLifecycle<'_>> = None;
    let mut verifier_slot: Option<PluginPathVerifier<'_>> = None;

    let steps = scenario["steps"]
        .as_array()
        .expect("every scenario declares a steps array");

    for (step_index, step) in steps.iter().enumerate() {
        let kind = step["step"]
            .as_str()
            .unwrap_or_else(|| panic!("scenario '{scenario_id}' step {step_index} has no kind"));
        match kind {
            "parse-manifest" => {
                let index = step["index"].as_u64().expect("parse-manifest carries an index") as usize;
                let result = parse_manifest_value(&manifest_inputs[index]);
                if let Some(manifest) =
                    expect_step(scenario_id, step_index, &step["expect"], result)
                {
                    manifests[index] = Some(manifest);
                }
            }
            "resolve" => {
                let indices: Vec<usize> = step["manifests"]
                    .as_array()
                    .expect("resolve carries a manifests list")
                    .iter()
                    .map(|v| v.as_u64().expect("manifest index") as usize)
                    .collect();
                let list: Vec<Manifest> = indices
                    .iter()
                    .map(|i| {
                        manifests[*i]
                            .as_ref()
                            .unwrap_or_else(|| {
                                panic!("scenario '{scenario_id}' step {step_index}: manifest {i} was not parsed by an earlier step")
                            })
                            .clone()
                    })
                    .collect();
                let mut selections = ExplicitSelections::new();
                if let Some(pairs) = step["selections"].as_array() {
                    for pair in pairs {
                        let contract = pair[0].as_str().expect("selection contract").to_string();
                        let plugin = pair[1].as_str().expect("selection plugin").to_string();
                        selections.insert(contract, plugin);
                    }
                }
                let outcome = resolve(&list, &selections);
                if let Some(plan) = expect_step(scenario_id, step_index, &step["expect"], outcome) {
                    if let Some(expected) = step["expect"]["registrations"].as_array() {
                        let actual: Vec<&str> = plan
                            .registrations
                            .iter()
                            .map(|r| r.plugin_id.as_str())
                            .collect();
                        let expected: Vec<&str> =
                            expected.iter().map(|v| v.as_str().expect("registration id")).collect();
                        assert_eq!(
                            actual, expected,
                            "scenario '{scenario_id}' step {step_index}: registration order mismatch"
                        );
                    }
                    if let Some(name) = step["as"].as_str() {
                        plans.insert(name.to_string(), plan);
                    }
                }
            }
            "parse-profile" => {
                let index = step["index"].as_u64().expect("parse-profile carries an index") as usize;
                let result = parse_profile_value(&profile_inputs[index]);
                if let Some(profile) = expect_step(scenario_id, step_index, &step["expect"], result) {
                    profiles[index] = Some(profile);
                }
            }
            "validate-profile" => {
                let profile_index =
                    step["profile"].as_u64().expect("validate-profile carries a profile index") as usize;
                let indices: Vec<usize> = step["manifests"]
                    .as_array()
                    .expect("validate-profile carries a manifests list")
                    .iter()
                    .map(|v| v.as_u64().expect("manifest index") as usize)
                    .collect();
                let profile = profiles[profile_index]
                    .as_ref()
                    .unwrap_or_else(|| panic!("scenario '{scenario_id}' step {step_index}: profile {profile_index} was not parsed by an earlier step"));
                let list: Vec<Manifest> = indices
                    .iter()
                    .map(|i| {
                        manifests[*i]
                            .as_ref()
                            .unwrap_or_else(|| {
                                panic!("scenario '{scenario_id}' step {step_index}: manifest {i} was not parsed by an earlier step")
                            })
                            .clone()
                    })
                    .collect();
                let outcome = validate_profile(profile, &list);
                if let Some(plan) = expect_step(scenario_id, step_index, &step["expect"], outcome) {
                    if let Some(expected) = step["expect"]["registrations"].as_array() {
                        let actual: Vec<&str> = plan
                            .registrations
                            .iter()
                            .map(|r| r.plugin_id.as_str())
                            .collect();
                        let expected: Vec<&str> =
                            expected.iter().map(|v| v.as_str().expect("registration id")).collect();
                        assert_eq!(
                            actual, expected,
                            "scenario '{scenario_id}' step {step_index}: registration order mismatch"
                        );
                    }
                    if let Some(name) = step["as"].as_str() {
                        plans.insert(name.to_string(), plan);
                    }
                }
            }
            "diff-plans" => {
                let current = plans
                    .get(step["current"].as_str().expect("diff-plans carries a current plan name"))
                    .expect("the current plan was produced by an earlier step");
                let target = plans
                    .get(step["target"].as_str().expect("diff-plans carries a target plan name"))
                    .expect("the target plan was produced by an earlier step");
                let diff = diff_plans(current, target);
                if expect_step::<(), RecompositionError>(
                    scenario_id,
                    step_index,
                    &step["expect"],
                    Ok(()),
                )
                .is_some()
                {
                    if let Some(counts) = step["expect"]["counts"].as_object() {
                        let assert_count = |key: &str, actual: usize| {
                            if let Some(expected) = counts[key].as_u64() {
                                assert_eq!(
                                    actual as u64, expected,
                                    "scenario '{scenario_id}' step {step_index}: diff count '{key}' mismatch"
                                );
                            }
                        };
                        assert_count("added", diff.added_contracts.len());
                        assert_count("removed", diff.removed_contracts.len());
                        assert_count("replaced", diff.replaced_contracts.len());
                        assert_count("departing", diff.departing_plugins.len());
                        assert_count("arriving", diff.arriving_plugins.len());
                        assert_count("upgraded", diff.upgraded_plugins.len());
                    }
                }
            }
            "execute-switch" => {
                let from = step["from"].as_str().expect("execute-switch carries from");
                let to = step["to"].as_str().expect("execute-switch carries to");
                let current_plan = plans
                    .get(step["current-plan"].as_str().expect("current-plan name"))
                    .expect("the current plan was produced by an earlier step");
                let target_plan = plans
                    .get(step["target-plan"].as_str().expect("target-plan name"))
                    .expect("the target plan was produced by an earlier step");
                let to_posture = match step
                    .get("to-posture")
                    .and_then(|v| v.as_str())
                    .unwrap_or("simulation-shadow")
                {
                    "simulation-shadow" => ControlAuthorityPosture::SimulationShadow,
                    "read-only-shadow" => ControlAuthorityPosture::ReadOnlyShadow,
                    "operator-approved" => ControlAuthorityPosture::OperatorApproved,
                    "bounded-automation" => ControlAuthorityPosture::BoundedAutomation,
                    other => panic!(
                        "scenario '{scenario_id}' step {step_index}: unknown to-posture '{other}'"
                    ),
                };
                let confirmation = match step.get("confirmation") {
                    Some(c) if !c.is_null() => Some(SwitchConfirmation {
                        actor: c["actor"].as_str().expect("confirmation actor").to_string(),
                        from_profile: c["from-profile"]
                            .as_str()
                            .expect("confirmation from-profile")
                            .to_string(),
                        to_profile: c["to-profile"]
                            .as_str()
                            .expect("confirmation to-profile")
                            .to_string(),
                        timestamp_millis: c["timestamp-millis"]
                            .as_u64()
                            .expect("confirmation timestamp"),
                        reason: c["reason"].as_str().expect("confirmation reason").to_string(),
                    }),
                    _ => None,
                };
                let request = SwitchRequest {
                    from_profile: from,
                    to_profile: to,
                    from_posture: ControlAuthorityPosture::SimulationShadow,
                    to_posture,
                    current_plan,
                    target_plan,
                    confirmation: confirmation.as_ref(),
                };
                let mut executor = ScriptedExecutor;
                let outcome = execute_switch(&request, &mut executor, &clock, &audit);
                if let Some(switched) =
                    expect_step(scenario_id, step_index, &step["expect"], outcome)
                {
                    if let Some(expected_steps) = step["expect"]["applied-steps"].as_u64() {
                        assert_eq!(
                            switched.applied_steps as u64, expected_steps,
                            "scenario '{scenario_id}' step {step_index}: applied-steps mismatch"
                        );
                    }
                }
            }
            "lifecycle-register" => {
                let plugin_id = step["plugin-id"].as_str().expect("register carries plugin-id");
                let version = step["version"].as_str().expect("register carries version");
                lifecycle = Some(PluginLifecycle::register(
                    plugin_id,
                    version,
                    &clock,
                    &audit,
                    DrainPolicy::default(),
                ));
                // Registration never refuses; the expect is asserted for
                // uniformity so a scenario cannot silently skip it.
                expect_step::<(), LifecycleError>(
                    scenario_id,
                    step_index,
                    &step["expect"],
                    Ok(()),
                );
            }
            "lifecycle-gate" => {
                let gate = match step["gate"].as_str().expect("lifecycle-gate carries a gate") {
                    "passed" => GateOutcome::Passed,
                    "failed" => GateOutcome::Failed {
                        reason: "scenario scripted gate failure".to_string(),
                    },
                    other => panic!(
                        "scenario '{scenario_id}' step {step_index}: unknown gate '{other}'"
                    ),
                };
                let transition =
                    step["transition"].as_str().expect("lifecycle-gate carries a transition");
                let active = lifecycle
                    .as_mut()
                    .expect("lifecycle-gate runs after lifecycle-register");
                let result = match transition {
                    "verify" => active.verify(gate, "scenario-interpreter", "scenario step"),
                    "mark-compatible" => {
                        active.mark_compatible(gate, "scenario-interpreter", "scenario step")
                    }
                    "configure" => active.configure(gate, "scenario-interpreter", "scenario step"),
                    "start" => active.start(gate, "scenario-interpreter", "scenario step"),
                    "mark-ready" => active.mark_ready(gate, "scenario-interpreter", "scenario step"),
                    other => panic!(
                        "scenario '{scenario_id}' step {step_index}: unknown transition '{other}'"
                    ),
                };
                expect_step(scenario_id, step_index, &step["expect"], result);
            }
            "lifecycle-state" => {
                let active = lifecycle
                    .as_ref()
                    .expect("lifecycle-state runs after lifecycle-register");
                let expected =
                    step["state"].as_str().expect("lifecycle-state carries a state name");
                assert_eq!(
                    active.state().name(),
                    expected,
                    "scenario '{scenario_id}' step {step_index}: lifecycle state mismatch"
                );
            }
            "set-drain-script" => {
                let mut script = drain_script.borrow_mut();
                script.quiesce = step["quiesce"].as_bool().expect("set-drain-script carries quiesce");
                script.outstanding =
                    step["outstanding"].as_u64().unwrap_or(0) as usize;
                script.cancelled = step["cancelled"].as_u64().unwrap_or(0) as usize;
            }
            "lifecycle-begin-drain" => {
                let active = lifecycle
                    .as_mut()
                    .expect("lifecycle-begin-drain runs after lifecycle-register");
                let result = active.begin_drain(
                    &mut drain_port,
                    "scenario-interpreter",
                    "scenario scripted drain",
                );
                expect_step(scenario_id, step_index, &step["expect"], result);
            }
            "lifecycle-complete-drain" => {
                let active = lifecycle
                    .as_mut()
                    .expect("lifecycle-complete-drain runs after lifecycle-register");
                let result = active.complete_drain("scenario-interpreter", "scenario scripted drain");
                expect_step(scenario_id, step_index, &step["expect"], result);
            }
            "lifecycle-record-version-change" => {
                let new_version = step["new-version"]
                    .as_str()
                    .expect("record-version-change carries new-version");
                let active = lifecycle
                    .as_mut()
                    .expect("lifecycle-record-version-change runs after lifecycle-register");
                let result =
                    active.record_version_change(new_version, "scenario-interpreter", "scenario upgrade");
                expect_step(scenario_id, step_index, &step["expect"], result);
            }
            "resolve-config" => {
                let manifest_index =
                    step["manifest"].as_u64().expect("resolve-config carries a manifest index") as usize;
                let manifest = manifests[manifest_index]
                    .as_ref()
                    .unwrap_or_else(|| panic!("scenario '{scenario_id}' step {step_index}: manifest {manifest_index} was not parsed by an earlier step"));
                let instantiated_version = step["instantiated-version"]
                    .as_str()
                    .expect("resolve-config carries instantiated-version");
                let config_index =
                    step["config"].as_u64().expect("resolve-config carries a config index") as usize;
                let config = &config_inputs[config_index];
                let outcome = resolve_for_instance(manifest, instantiated_version, None, config);
                if let Some(resolved_config) =
                    expect_step(scenario_id, step_index, &step["expect"], outcome)
                {
                    if let Some(expected_json) = step["expect"]["resolved-json"].as_object() {
                        assert_eq!(
                            resolved_config.resolved_json(),
                            serde_json::Value::Object(expected_json.clone()),
                            "scenario '{scenario_id}' step {step_index}: resolved view mismatch"
                        );
                    }
                    if let Some(name) = step["as"].as_str() {
                        resolved.insert(name.to_string(), resolved_config.resolved_json());
                    }
                }
            }
            "build-envelope" => {
                let payload = resolve_payload(&config_inputs, &step["payload"]);
                let build = || {
                    PluginRequestEnvelope::new(
                        step["correlation-id"].as_str().expect("correlation-id"),
                        step["plugin-id"].as_str().expect("plugin-id"),
                        step["plugin-version"].as_str().expect("plugin-version"),
                        step["sequence"].as_u64().expect("sequence"),
                        step["valid-from"].as_u64().expect("valid-from"),
                        step["valid-until"].as_u64().expect("valid-until"),
                        payload.clone(),
                    )
                };
                // The P7.42 pin on EVERY scenario build: construction is a
                // pure function — building twice yields byte-identical
                // canonical strings and digests.
                if let (Ok(first), Ok(second)) = (build(), build()) {
                    assert_eq!(
                        first.canonical_string(),
                        second.canonical_string(),
                        "scenario '{scenario_id}' step {step_index}: envelope construction is not byte-stable"
                    );
                    assert_eq!(first.digest_hex, second.digest_hex);
                }
                let outcome = build();
                if let Some(envelope) =
                    expect_step(scenario_id, step_index, &step["expect"], outcome)
                {
                    if let Some(name) = step["as"].as_str() {
                        envelopes.insert(name.to_string(), envelope);
                    }
                }
            }
            "verify-envelope" => {
                if verifier_slot.is_none() {
                    let path = scratch_path(&format!("scenario-{scenario_id}"));
                    let checkpoint = Box::new(
                        FilePluginSequenceCheckpoint::open(&path)
                            .expect("the scenario checkpoint opens"),
                    );
                    verifier_slot = Some(
                        PluginPathVerifier::new(&clock, checkpoint)
                            .expect("the scenario verifier loads its history"),
                    );
                }
                let name = step["envelope"].as_str().expect("verify-envelope carries an envelope");
                let envelope = envelopes
                    .get(name)
                    .unwrap_or_else(|| panic!("scenario '{scenario_id}' step {step_index}: envelope '{name}' was not built by an earlier step"));
                let verifier = verifier_slot
                    .as_mut()
                    .expect("the scenario verifier exists after the first use");
                let outcome = verifier.verify_and_admit(envelope);
                expect_step(scenario_id, step_index, &step["expect"], outcome);
            }
            "tamper-envelope" => {
                let source = step["envelope"].as_str().expect("tamper-envelope carries an envelope");
                let target = step["as"].as_str().expect("tamper-envelope carries an as name");
                let mut envelope = envelopes
                    .get(source)
                    .expect("the tampered envelope's source was built by an earlier step")
                    .clone();
                envelope.payload = resolve_payload(&config_inputs, &step["payload"]);
                envelopes.insert(target.to_string(), envelope);
            }
            "advance-clock" => {
                let by = step["by"].as_u64().expect("advance-clock carries by");
                clock.advance_by(by);
            }
            "record-outcome" => {
                let name = step["envelope"].as_str().expect("record-outcome carries an envelope");
                let request = envelopes
                    .get(name)
                    .expect("the recorded envelope was built by an earlier step");
                let response = PluginResponseEnvelope {
                    correlation_id: step["response"]["correlation-id"]
                        .as_str()
                        .expect("response correlation-id")
                        .to_string(),
                    sequence: step["response"]["sequence"]
                        .as_u64()
                        .expect("response sequence"),
                    payload: resolve_payload(&config_inputs, &step["response"].get("payload").unwrap_or(&serde_json::Value::Null)),
                };
                let proposal = match step.get("proposal") {
                    Some(p) if !p.is_null() => Some(PluginProposalEnvelope::new(
                        p["proposal-id"].as_str().expect("proposal-id"),
                        p["producer-plugin-id"]
                            .as_str()
                            .expect("producer-plugin-id"),
                        p["producer-plugin-version"]
                            .as_str()
                            .expect("producer-plugin-version"),
                        p["payload"].clone(),
                        p["sequence"].as_u64().expect("proposal sequence"),
                    )
                    .expect("the scenario proposal envelope is valid")),
                    _ => None,
                };
                let verifier = verifier_slot
                    .as_ref()
                    .expect("record-outcome runs after a verify-envelope step");
                let outcome = verifier.record_outcome(request, &response, proposal);
                if let Some(recorded) =
                    expect_step(scenario_id, step_index, &step["expect"], outcome)
                {
                    if let Some(expected_variant) = step["expect"]["variant"].as_str() {
                        let matches = match (&recorded, expected_variant) {
                            (PluginPathOutcome::ProposalRecorded(_), "PROPOSAL_RECORDED") => true,
                            (PluginPathOutcome::ResultObserved { .. }, "RESULT_OBSERVED") => true,
                            _ => false,
                        };
                        assert!(
                            matches,
                            "scenario '{scenario_id}' step {step_index}: outcome variant mismatch \
                             (expected {expected_variant})"
                        );
                    }
                }
            }
            other => panic!(
                "scenario '{scenario_id}' step {step_index}: unknown step kind '{other}' \
                 (the interpreter vocabulary is closed; see plugins/scenarios/README.md)"
            ),
        }
    }
}

/// Resolves a scenario payload reference: an object carrying `config-index`
/// names an entry of `inputs.configs`; any other JSON value is used as-is.
fn resolve_payload(config_inputs: &[serde_json::Value], spec: &serde_json::Value) -> serde_json::Value {
    if let Some(index) = spec.get("config-index").and_then(|v| v.as_u64()) {
        config_inputs
            .get(index as usize)
            .cloned()
            .expect("the config-index names an existing inputs.configs entry")
    } else {
        spec.clone()
    }
}

// --- The suite entry points --------------------------------------------------------

#[test]
fn the_embedded_sdk_scenario_suite_executes_end_to_end() {
    // P7.43 step 2: every run records the suite version.
    println!("suite-id={SUITE_ID}");
    println!("scenario-count={}", SCENARIO_SOURCES.len());
    for (index, source) in SCENARIO_SOURCES.iter().enumerate() {
        let scenario: serde_json::Value = serde_json::from_str(source)
            .unwrap_or_else(|e| panic!("scenario file {index} is not valid JSON: {e}"));
        assert_eq!(
            scenario["suite"].as_str(),
            Some(SUITE_ID),
            "scenario {index} does not carry the recorded suite id"
        );
        run_scenario(&scenario);
        println!(
            "scenario '{}' (v{}) executed with every pinned expectation met",
            scenario["scenario-id"].as_str().expect("scenario-id"),
            scenario["scenario-version"].as_str().expect("scenario-version"),
        );
    }
}

#[test]
fn every_scenario_is_wellformed_with_pinned_inputs_seeds_and_tolerance() {
    // P7.43 step 1: every named scenario records its inputs, seeds,
    // versions, expected outcome, and tolerance — asserted structurally so
    // an empty or unpinned suite cannot be green.
    for (index, source) in SCENARIO_SOURCES.iter().enumerate() {
        let scenario: serde_json::Value = serde_json::from_str(source)
            .unwrap_or_else(|e| panic!("scenario file {index} is not valid JSON: {e}"));
        let id = scenario["scenario-id"]
            .as_str()
            .unwrap_or_else(|| panic!("scenario file {index} has no scenario-id"));
        assert!(
            scenario["scenario-version"].as_str().is_some(),
            "scenario '{id}' has no scenario-version"
        );
        assert_eq!(
            scenario["suite"].as_str(),
            Some(SUITE_ID),
            "scenario '{id}' does not carry the recorded suite id"
        );
        assert!(
            !scenario["description"].as_str().unwrap_or_default().is_empty(),
            "scenario '{id}' has no description"
        );
        assert!(
            !scenario["pins"]["seeds"].as_str().unwrap_or_default().is_empty(),
            "scenario '{id}' does not record its seeds position"
        );
        assert!(
            scenario["pins"]["versions"].as_object().is_some_and(|v| !v.is_empty()),
            "scenario '{id}' does not record its versions"
        );
        assert!(
            !scenario["pins"]["tolerance"].as_str().unwrap_or_default().is_empty(),
            "scenario '{id}' does not record its tolerance"
        );
        let manifest_inputs = scenario["inputs"]["manifests"]
            .as_array()
            .expect("inputs.manifests");
        let profile_inputs = scenario["inputs"]["profiles"].as_array().expect("inputs.profiles");
        let config_inputs = scenario["inputs"]["configs"].as_array().cloned().unwrap_or_default();
        let steps = scenario["steps"].as_array().expect("steps");
        assert!(!steps.is_empty(), "scenario '{id}' has no steps (an empty suite is not green)");
        for (step_index, step) in steps.iter().enumerate() {
            let kind = step["step"]
                .as_str()
                .unwrap_or_else(|| panic!("scenario '{id}' step {step_index} has no step kind"));
            // Structural reference checks: every referenced input index exists.
            if let Some(i) = step["index"].and_then(|v| v.as_u64()) {
                let bound = match kind {
                    "parse-manifest" => manifest_inputs.len(),
                    "parse-profile" => profile_inputs.len(),
                    other => panic!("scenario '{id}' step {step_index}: unexpected indexed kind '{other}'"),
                };
                assert!(
                    (i as usize) < bound,
                    "scenario '{id}' step {step_index}: index {i} out of bounds for {kind}"
                );
            }
            if let Some(config_index) = step["payload"].get("config-index").and_then(|v| v.as_u64())
            {
                assert!(
                    (config_index as usize) < config_inputs.len(),
                    "scenario '{id}' step {step_index}: config-index out of bounds"
                );
            }
            // Expectation presence: every outcome-producing step carries an
            // expect object; state-mutation helpers do not.
            let needs_expect = !matches!(kind, "tamper-envelope" | "set-drain-script" | "advance-clock");
            if needs_expect {
                assert!(
                    step["expect"].as_object().is_some(),
                    "scenario '{id}' step {step_index} ({kind}) carries no expectation"
                );
            }
        }
    }
}

#[test]
fn the_suite_covers_the_recorded_sdk_scenario_set() {
    // P7.43 step 1 (naming): the recorded scenario set is present by id —
    // the manifest-to-plan slice scenario, the lifecycle/profile-switch
    // scenario with drain, the configuration-drift denial scenario, and the
    // plugin-path replay/denial scenario. A removed or renamed scenario
    // fails here.
    let mut ids: Vec<String> = SCENARIO_SOURCES
        .iter()
        .map(|source| {
            serde_json::from_str::<serde_json::Value>(source)
                .expect("scenario parses")["scenario-id"]
                .as_str()
                .expect("scenario-id")
                .to_string()
        })
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        vec![
            "sdk-01-manifest-to-plan",
            "sdk-02-profile-switch-with-drain",
            "sdk-03-config-drift-refusal",
            "sdk-04-proposal-envelope-replay-refused",
        ],
        "the recorded SDK scenario set drifted"
    );
}
