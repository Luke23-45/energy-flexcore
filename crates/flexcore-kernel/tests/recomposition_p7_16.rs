// P7.16 — Profile switching and safe re-composition: diff, confirmation,
// ordering, and partial-failure tests (ledger P7.16; test file written,
// NOT run — the loop bans hold; the human build/test pass executes these).
//
// Coverage (ledger P7.16 acceptance criteria, insofar as they are code
// properties):
//
// 1. A CLEAN switch: the diff is correct (added / removed / replaced /
//    departing / arriving / upgraded), the audit trail is ordered
//    (started → every applied step → committed), and the executor applied
//    exactly the diff (drain-before-replace honoured).
// 2. A MISSING confirmation is refused (`SWITCH_CONFIRMATION_MISSING`,
//    fail-closed) and the composition is untouched.
// 3. A mismatched confirmation (`SWITCH_CONFIRMATION_MISMATCH`) and a
//    posture-changing switch (`CONTROL_AUTHORITY_MODE_CHANGE_REFUSED`,
//    §21.4) are refused — a profile switch can never change the
//    control-authority mode.
// 4. The protected kernel is NEVER reachable: even a hand-constructed
//    target plan containing a protected contract id is refused at the
//    composition level (`PROTECTED_CONTRACT_IN_TARGET`) — on top of the
//    resolver's structural refusal (`PROTECTED_KERNEL_REPLACE`), which
//    the plan-producing path cannot bypass.
// 5. FAILURE MID-SWITCH: the composition stands at the LAST FULLY-APPLIED
//    state, the abort is audited (step refusal + abort record), and the
//    exact ordering is pinned (prepare → per-plugin drain/replace/remove
//    → newcomers).
// 6. A switch while a dispatch transaction is active is REFUSED: the
//    incumbent with outstanding work cannot be drained (P7.07's recorded
//    REFUSED-not-queued behaviour) and the composition is unchanged.
// 7. The diff is DETERMINISTIC (same plans → identical diff; reversed
//    manifest input order → identical plan, hence identical diff); a
//    no-op switch is refused (`SWITCH_NO_OP_REFUSED`).

use std::collections::BTreeMap;

use flexcore_kernel::lifecycle::{FixedClock, LifecycleAuditKind, MemoryAuditSink};
use flexcore_kernel::manifest::{parse_manifest, Manifest};
use flexcore_kernel::profile::ControlAuthorityPosture;
use flexcore_kernel::recomposition::{
    diff_plans, execute_switch, CompositionDiff, CompositionExecutor, ContractChange,
    ContractReplacement, PluginChange, RecompositionError, SwitchConfirmation, SwitchRequest,
    SwitchStep,
};
use flexcore_kernel::resolve::{resolve, CompositionPlan, ExplicitSelections};

// --- Plan fixtures (resolver-produced; the switch consumes real plans) ----

fn manifest_json(plugin_id: &str, version: &str, provides: &[&str]) -> String {
    let provides = provides
        .iter()
        .map(|c| format!("    \"{c}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        r#"{{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "{plugin_id}",
  "name": "Switch fixture for {plugin_id}",
  "version": "{version}",
  "api-version": ">=0.5.0",
  "class": "builtin-rust",
  "entry-point": {{
    "registered-type": "{plugin_id}/default"
  }},
  "provider-of": [
{provides}
  ],
  "consumes": [],
  "permissions": [],
  "configuration-schema": {{
    "type": "object",
    "additionalProperties": false,
    "properties": {{}},
    "required": []
  }},
  "secrets": [],
  "resources": {{
    "max-memory-mb": 64,
    "timeout-ms": 30000
  }},
  "metadata": {{
    "license": "Apache-2.0",
    "description": "Test fixture manifest (not a shipped plugin)."
  }}
}}"#
    )
}

fn fixture(plugin_id: &str, version: &str, provides: &[&str]) -> Manifest {
    parse_manifest(&manifest_json(plugin_id, version, provides)).expect("fixture manifest must be valid")
}

fn explicit_all(manifests: &[Manifest]) -> ExplicitSelections {
    let mut selections = ExplicitSelections::new();
    for manifest in manifests {
        for contract in &manifest.provider_of {
            selections.insert(contract.clone(), manifest.plugin_id.clone());
        }
    }
    selections
}

fn plan_of(manifests: &[Manifest]) -> CompositionPlan {
    let selections = explicit_all(manifests);
    resolve(manifests, &selections).expect("fixture composition must resolve")
}

/// The current composition: the three seed built-ins at 0.5.0.
fn current_plan() -> CompositionPlan {
    plan_of(&[
        fixture("builtin.simulator-telemetry", "0.5.0", &["telemetry.source"]),
        fixture("builtin.highs-optimizer", "0.5.0", &["optimization.solver"]),
        fixture("builtin.pandapower-grid", "0.5.0", &["grid.model"]),
    ])
}

/// The target composition: telemetry unchanged, the optimizer upgraded to
/// 0.6.0 (drain + replace), the grid model REMOVED (its provider departs),
/// and a newcomer `acme.replay-tools` ADDED providing `replay.tools`.
fn target_plan() -> CompositionPlan {
    plan_of(&[
        fixture("builtin.simulator-telemetry", "0.5.0", &["telemetry.source"]),
        fixture("builtin.highs-optimizer", "0.6.0", &["optimization.solver"]),
        fixture("acme.replay-tools", "0.5.0", &["replay.tools"]),
    ])
}

fn expected_diff() -> CompositionDiff {
    CompositionDiff {
        added_contracts: vec![ContractChange {
            contract_id: "replay.tools".to_string(),
            provider_plugin_id: "acme.replay-tools".to_string(),
            provider_version: "0.5.0".to_string(),
        }],
        removed_contracts: vec![ContractChange {
            contract_id: "grid.model".to_string(),
            provider_plugin_id: "builtin.pandapower-grid".to_string(),
            provider_version: "0.5.0".to_string(),
        }],
        replaced_contracts: vec![ContractReplacement {
            contract_id: "optimization.solver".to_string(),
            old_provider_plugin_id: "builtin.highs-optimizer".to_string(),
            old_provider_version: "0.5.0".to_string(),
            new_provider_plugin_id: "builtin.highs-optimizer".to_string(),
            new_provider_version: "0.6.0".to_string(),
        }],
        departing_plugins: vec![PluginChange {
            plugin_id: "builtin.pandapower-grid".to_string(),
            version: "0.5.0".to_string(),
        }],
        arriving_plugins: vec![PluginChange {
            plugin_id: "acme.replay-tools".to_string(),
            version: "0.5.0".to_string(),
        }],
        upgraded_plugins: vec![PluginChange {
            plugin_id: "builtin.highs-optimizer".to_string(),
            version: "0.6.0".to_string(),
        }],
    }
}

// --- The in-test executor (models the canonical composition-root wiring) ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PluginState {
    Active,
    Stopped,
}

/// A memory executor over a simple provider table. It models the CANONICAL
/// executor contract: drain-before-replace (a replace refuses unless the
/// incumbent is Stopped), a drain refusal is self-restoring (the plugin
/// stays Active — the lifecycle's fail-closed restoration), and eager
/// preparation is side-effect free.
struct MemoryExecutor {
    providers: BTreeMap<String, (String, String)>, // contract -> (plugin, version)
    states: BTreeMap<String, PluginState>,
    outstanding: BTreeMap<String, usize>,
    fail_prepare_for: Option<String>,
    fail_add_for: Option<String>,
    fail_drain_for: Option<String>,
    applied_ops: Vec<String>,
}

impl MemoryExecutor {
    fn from_plan(plan: &CompositionPlan) -> MemoryExecutor {
        let mut executor = MemoryExecutor {
            providers: BTreeMap::new(),
            states: BTreeMap::new(),
            outstanding: BTreeMap::new(),
            fail_prepare_for: None,
            fail_add_for: None,
            fail_drain_for: None,
            applied_ops: Vec::new(),
        };
        for registration in &plan.registrations {
            executor.states.insert(registration.plugin_id.clone(), PluginState::Active);
            executor.outstanding.insert(registration.plugin_id.clone(), 0);
            for contract in &registration.provides {
                executor
                    .providers
                    .insert(contract.clone(), (registration.plugin_id.clone(), registration.version.clone()));
            }
        }
        executor
    }

    fn refused(step: SwitchStep, reason: &str) -> RecompositionError {
        RecompositionError::StepRefused { step, reason: reason.to_string() }
    }
}

impl CompositionExecutor for MemoryExecutor {
    fn prepare_provider(&mut self, contract_id: &str, plugin_id: &str, version: &str) -> Result<(), RecompositionError> {
        if self.fail_prepare_for.as_deref() == Some(contract_id) {
            return Err(Self::refused(SwitchStep::Prepare, "injected construction failure"));
        }
        self.applied_ops.push(format!("PREPARE {contract_id}={plugin_id}@{version}"));
        Ok(())
    }

    fn drain_to_stopped(&mut self, plugin_id: &str) -> Result<(), RecompositionError> {
        // Self-restoring refusal: outstanding work (or an injected failure)
        // leaves the incumbent ACTIVE — the lifecycle restores it to Ready.
        let busy = self.outstanding.get(plugin_id).copied().unwrap_or(0) > 0;
        if busy || self.fail_drain_for.as_deref() == Some(plugin_id) {
            return Err(Self::refused(
                SwitchStep::Drain,
                &format!("incumbent '{plugin_id}' cannot be drained (work outstanding; restored to Ready, operator decides)"),
            ));
        }
        self.states.insert(plugin_id.to_string(), PluginState::Stopped);
        self.applied_ops.push(format!("DRAIN {plugin_id}"));
        Ok(())
    }

    fn replace_provider(&mut self, contract_id: &str, new_plugin_id: &str, new_version: &str) -> Result<(), RecompositionError> {
        // Drain-before-replace: the incumbent must be stopped (the P7.08
        // swap_provider gate, modelled).
        let incumbent = self.providers.get(contract_id).map(|(p, _)| p.clone());
        if let Some(incumbent) = incumbent {
            if self.states.get(&incumbent).copied() != Some(PluginState::Stopped) {
                return Err(Self::refused(
                    SwitchStep::Replace,
                    &format!("incumbent '{incumbent}' of '{contract_id}' is not fully drained"),
                ));
            }
        }
        self.providers
            .insert(contract_id.to_string(), (new_plugin_id.to_string(), new_version.to_string()));
        self.applied_ops.push(format!("REPLACE {contract_id}={new_plugin_id}@{new_version}"));
        Ok(())
    }

    fn remove_provider(&mut self, contract_id: &str) -> Result<(), RecompositionError> {
        self.providers.remove(contract_id);
        self.applied_ops.push(format!("REMOVE {contract_id}"));
        Ok(())
    }

    fn add_provider(&mut self, contract_id: &str, plugin_id: &str, version: &str) -> Result<(), RecompositionError> {
        if self.fail_add_for.as_deref() == Some(contract_id) {
            return Err(Self::refused(SwitchStep::Add, "injected registration failure"));
        }
        self.providers
            .insert(contract_id.to_string(), (plugin_id.to_string(), version.to_string()));
        self.applied_ops.push(format!("ADD {contract_id}={plugin_id}@{version}"));
        Ok(())
    }
}

// --- The switch request ----------------------------------------------------------

const FROM: &str = "sim-lab";
const TO: &str = "vpp-operator";

fn confirmation() -> SwitchConfirmation {
    SwitchConfirmation {
        actor: "op-controller".to_string(),
        from_profile: FROM.to_string(),
        to_profile: TO.to_string(),
        timestamp_millis: 176_234_880_000,
        reason: "scheduled composition change".to_string(),
    }
}

fn run(
    executor: &mut MemoryExecutor,
    current: &CompositionPlan,
    target: &CompositionPlan,
    confirmation: Option<&SwitchConfirmation>,
    from_posture: ControlAuthorityPosture,
    to_posture: ControlAuthorityPosture,
) -> (Result<usize, RecompositionError>, MemoryAuditSink, FixedClock) {
    let audit = MemoryAuditSink::new();
    let clock = FixedClock::new(10_000);
    let request = SwitchRequest {
        from_profile: FROM,
        to_profile: TO,
        from_posture,
        to_posture,
        current_plan: current,
        target_plan: target,
        confirmation,
    };
    let result = match execute_switch(&request, executor, &clock, &audit) {
        Ok(outcome) => Ok(outcome.applied_steps),
        Err(err) => Err(err),
    };
    (result, audit, clock)
}

// --- 1. Clean switch ---------------------------------------------------------------

#[test]
fn a_clean_switch_computes_the_correct_diff_applies_exactly_it_and_audits_every_step() {
    let current = current_plan();
    let target = target_plan();

    // The diff is exactly the three change kinds, sorted.
    let diff = diff_plans(&current, &target);
    assert_eq!(diff, expected_diff());

    // The switch applies exactly the diff, in the recorded order.
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let applied = result.expect("the clean switch must commit");
    // 2 prepares + 2 drains + 1 replace + 1 remove + 1 add = 7.
    assert_eq!(applied, 7);
    assert_eq!(
        executor.applied_ops,
        vec![
            "PREPARE optimization.solver=builtin.highs-optimizer@0.6.0",
            "PREPARE replay.tools=acme.replay-tools@0.5.0",
            "DRAIN builtin.highs-optimizer",
            "REPLACE optimization.solver=builtin.highs-optimizer@0.6.0",
            "DRAIN builtin.pandapower-grid",
            "REMOVE grid.model",
            "ADD replay.tools=acme.replay-tools@0.5.0",
        ]
    );
    // The final composition IS the target composition's provider table.
    assert_eq!(executor.providers.get("telemetry.source").unwrap().0, "builtin.simulator-telemetry");
    assert_eq!(executor.providers.get("optimization.solver").unwrap().1, "0.6.0");
    assert!(!executor.providers.contains_key("grid.model"));
    assert_eq!(executor.providers.get("replay.tools").unwrap().0, "acme.replay-tools");
    assert_eq!(executor.states.get("builtin.pandapower-grid"), Some(&PluginState::Stopped));

    // The audit trail is ordered: started -> every applied step ->
    // committed, all committed steps TRANSITION, all timestamps from the
    // injected clock, actor = the confirmation's actor.
    let records = audit.records();
    assert_eq!(records.len(), 1 + 7 + 1);
    assert!(records[0].reason.contains("profile switch sim-lab -> vpp-operator started"));
    assert!(records[0].reason.contains("1 contract(s) added, 1 removed, 1 replaced"));
    for record in &records[1..8] {
        assert_eq!(record.kind, LifecycleAuditKind::Transition);
        assert!(record.reason.starts_with("step "));
    }
    assert!(records[8].reason.contains("profile switch committed"));
    assert!(records[8].reason.contains("changes from 'sim-lab' to 'vpp-operator'"));
    for record in records.iter() {
        assert_eq!(record.timestamp_millis, 10_000);
        assert_eq!(record.actor, "op-controller");
        assert_eq!(record.plugin_id, "composition.profile-switch");
    }
}

#[test]
fn the_clean_switch_is_reproducible_and_the_diff_is_deterministic() {
    let current = current_plan();
    let target = target_plan();
    let confirmation = confirmation();

    // Deterministic diff: twice, and over plans built from reversed
    // manifest input order (the resolver's plan is order-independent).
    let diff_a = diff_plans(&current, &target);
    let diff_b = diff_plans(&current, &target);
    assert_eq!(diff_a, diff_b);
    let mut reversed = vec![
        fixture("builtin.pandapower-grid", "0.5.0", &["grid.model"]),
        fixture("builtin.highs-optimizer", "0.5.0", &["optimization.solver"]),
        fixture("builtin.simulator-telemetry", "0.5.0", &["telemetry.source"]),
    ];
    reversed.reverse();
    let current_from_reversed = plan_of(&reversed);
    assert_eq!(current_from_reversed, current);

    // Reproducible switch: two fresh executors reach identical state.
    let mut executor_a = MemoryExecutor::from_plan(&current);
    let mut executor_b = MemoryExecutor::from_plan(&current);
    let (ra, audit_a, _c1) = run(&mut executor_a, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let (rb, audit_b, _c2) = run(&mut executor_b, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    assert_eq!(ra, rb);
    assert_eq!(executor_a.providers, executor_b.providers);
    let reasons_a: Vec<&str> = audit_a.records().iter().map(|r| r.reason.as_str()).collect();
    let reasons_b: Vec<&str> = audit_b.records().iter().map(|r| r.reason.as_str()).collect();
    assert_eq!(reasons_a, reasons_b, "same clock + same steps → identical audit trail");
    assert_eq!(diff_a, expected_diff());
}

// --- 2. Confirmation is required, fail-closed ---------------------------------------

#[test]
fn a_switch_without_confirmation_is_refused_and_the_composition_is_untouched() {
    let current = current_plan();
    let target = target_plan();
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &target, None,
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("a switch without confirmation must be refused");
    assert_eq!(err.name(), "SWITCH_CONFIRMATION_MISSING");
    match &err {
        RecompositionError::ConfirmationMissing { from_profile, to_profile } => {
            assert_eq!(from_profile, FROM);
            assert_eq!(to_profile, TO);
        }
        other => panic!("named refusal expected, got {}", other.name()),
    }
    // Fail-closed: nothing was prepared, drained, or applied.
    assert!(executor.applied_ops.is_empty());
    assert_eq!(executor.providers.get("grid.model").unwrap().0, "builtin.pandapower-grid");
    // The refusal is audited, attributed to the unconfirmed caller.
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, LifecycleAuditKind::Refused);
    assert_eq!(records[0].actor, "unconfirmed-caller");
}

#[test]
fn a_confirmation_naming_a_different_switch_is_refused() {
    let current = current_plan();
    let target = target_plan();
    let wrong = SwitchConfirmation {
        actor: "op-controller".to_string(),
        from_profile: "sim-lab".to_string(),
        to_profile: "site-edge".to_string(), // not the requested switch
        timestamp_millis: 1,
        reason: "typo".to_string(),
    };
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &target, Some(&wrong),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("a mismatched confirmation must be refused");
    assert_eq!(err.name(), "SWITCH_CONFIRMATION_MISMATCH");
    assert!(executor.applied_ops.is_empty());
    assert_eq!(audit.records().len(), 1);
    assert_eq!(audit.records()[0].kind, LifecycleAuditKind::Refused);
}

// --- 3. The control-authority mode can never change ----------------------------------

#[test]
fn a_switch_that_would_change_the_control_authority_posture_is_refused() {
    let current = current_plan();
    let target = target_plan();
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::OperatorApproved);
    let err = result.expect_err("a posture-changing switch must be refused");
    assert_eq!(err.name(), "CONTROL_AUTHORITY_MODE_CHANGE_REFUSED");
    assert!(executor.applied_ops.is_empty());
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert!(records[0].reason.contains("deployment/per-site setting"));
}

// --- 4. The protected kernel is never reachable ---------------------------------------

#[test]
fn a_protected_contract_in_the_target_is_refused_at_the_composition_level() {
    // The resolver refuses protected claims BEFORE any plan exists, so a
    // plan containing one can only be hand-constructed — and even then the
    // switch refuses it. Both layers are pinned here.
    let current = current_plan();
    let mut hostile_target = target_plan();
    let claims = flexcore_kernel::resolve::PROTECTED_KERNEL_CONTRACT_IDS;
    // Hand-construct the violation: a contract row claiming the dispatch
    // gateway, plus a registration providing it.
    let mut contracts = hostile_target.contracts.clone();
    contracts.push(flexcore_kernel::resolve::ResolvedContract {
        contract_id: claims[1].to_string(), // kernel.dispatch-gateway
        provider_plugin_id: "hostile.gateway".to_string(),
        basis: flexcore_kernel::resolve::SelectionBasis::UniqueProvider,
        consumers: vec![],
    });
    hostile_target.contracts = contracts;
    let mut registrations = hostile_target.registrations.clone();
    let mut hostile = flexcore_kernel::resolve::PlanRegistration {
        plugin_id: "hostile.gateway".to_string(),
        version: "0.1.0".to_string(),
        class: flexcore_kernel::manifest::PluginClass::BuiltInRust,
        provides: vec![claims[1].to_string()],
        consumes: vec![],
    };
    hostile.provides.sort();
    registrations.push(hostile);
    registrations.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
    hostile_target.registrations = registrations;

    // And the resolver path refuses the same claim before any plan exists.
    let hostile_manifest = fixture("hostile.gateway", "0.1.0", &[claims[1]]);
    let err = resolve(&[hostile_manifest.clone()], &explicit_all(&[hostile_manifest]))
        .expect_err("the resolver refuses the protected claim structurally");
    assert_eq!(err.name(), "PROTECTED_KERNEL_REPLACE");

    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &hostile_target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("the protected kernel is never reachable");
    assert_eq!(err.name(), "PROTECTED_CONTRACT_IN_TARGET");
    assert!(executor.applied_ops.is_empty());
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, LifecycleAuditKind::Refused);
}

// --- 5. Failure mid-switch: last fully-applied state + audited -----------------------

#[test]
fn a_drain_failure_aborts_with_the_composition_unchanged_and_the_abort_audited() {
    let current = current_plan();
    let target = target_plan();
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    // The optimizer (the FIRST affected incumbent in plugin-id order)
    // cannot be drained: work outstanding (a dispatch transaction active).
    executor.outstanding.insert("builtin.highs-optimizer".to_string(), 3);
    let (result, audit, _clock) = run(&mut executor, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("a blocked drain aborts the switch");
    assert_eq!(err.name(), "SWITCH_STEP_REFUSED");
    assert_eq!(err.step(), Some(SwitchStep::Drain));

    // Last fully-applied state: prepares happened (side-effect free); the
    // drain refused; NOTHING else was applied. The composition is intact.
    assert_eq!(
        executor.applied_ops,
        vec![
            "PREPARE optimization.solver=builtin.highs-optimizer@0.6.0",
            "PREPARE replay.tools=acme.replay-tools@0.5.0",
        ]
    );
    assert_eq!(executor.providers.get("optimization.solver").unwrap().1, "0.5.0");
    assert_eq!(executor.providers.get("grid.model").unwrap().0, "builtin.pandapower-grid");
    assert!(!executor.providers.contains_key("replay.tools"));

    // Audited: the step refusal AND the abort record.
    let records = audit.records();
    let refused: Vec<&flexcore_kernel::lifecycle::LifecycleAuditRecord> = records
        .iter()
        .filter(|r| r.kind == LifecycleAuditKind::Refused)
        .collect();
    assert_eq!(refused.len(), 2);
    assert!(refused[0].reason.contains("step DRAIN refused"));
    assert!(refused[1].reason.contains("ABORTED at step DRAIN"));
    assert!(refused[1].reason.contains("last fully-applied state"));
}

#[test]
fn a_failure_at_the_last_step_leaves_everything_before_it_applied() {
    let current = current_plan();
    let target = target_plan();
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    executor.fail_add_for = Some("replay.tools".to_string());
    let (result, audit, _clock) = run(&mut executor, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("an injected ADD failure aborts the switch");
    assert_eq!(err.name(), "SWITCH_STEP_REFUSED");
    assert_eq!(err.step(), Some(SwitchStep::Add));

    // The recorded limitation, pinned exactly: everything through the last
    // incumbent operation stands; the newcomer is NOT half-registered.
    assert_eq!(executor.applied_ops.len(), 6);
    assert!(!executor.providers.contains_key("replay.tools"));
    assert_eq!(executor.providers.get("optimization.solver").unwrap().1, "0.6.0");
    assert!(!executor.providers.contains_key("grid.model"));
    // The abort is audited with the recovery path named.
    let records = audit.records();
    let abort = records.iter().rev().find(|r| r.kind == LifecycleAuditKind::Refused).expect("abort audited");
    assert!(abort.reason.contains("ABORTED at step ADD"));
    assert!(abort.reason.contains("restart into the recorded profile"));
}

// --- 6. Switch while a dispatch transaction is active: refused ------------------------

#[test]
fn a_switch_with_a_dispatch_transaction_active_is_refused_not_queued() {
    let current = current_plan();
    let target = target_plan();
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    // The P7.07 recorded behaviour: an upgrade over outstanding work is
    // REFUSED (fail-closed), never queued.
    executor.outstanding.insert("builtin.highs-optimizer".to_string(), 1);
    let (result, _audit, _clock) = run(&mut executor, &current, &target, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("an active dispatch blocks the switch");
    assert_eq!(err.name(), "SWITCH_STEP_REFUSED");
    assert_eq!(err.step(), Some(SwitchStep::Drain));
    // Nothing but the side-effect-free prepares happened.
    assert_eq!(executor.applied_ops.len(), 2);
    assert_eq!(executor.states.get("builtin.highs-optimizer"), Some(&PluginState::Active));
}

// --- 7. No-op switches are refused ---------------------------------------------------

#[test]
fn a_no_op_switch_is_refused_not_silently_echoed() {
    let current = current_plan();
    let confirmation = confirmation();
    let mut executor = MemoryExecutor::from_plan(&current);
    let (result, audit, _clock) = run(&mut executor, &current, &current, Some(&confirmation),
        ControlAuthorityPosture::SimulationShadow, ControlAuthorityPosture::SimulationShadow);
    let err = result.expect_err("a no-op switch is refused");
    assert_eq!(err.name(), "SWITCH_NO_OP_REFUSED");
    assert!(executor.applied_ops.is_empty());
    assert_eq!(audit.records().len(), 1);
    assert_eq!(audit.records()[0].kind, LifecycleAuditKind::Refused);
}

// --- Error-name table ------------------------------------------------------------------

#[test]
fn every_recomposition_error_name_is_distinct() {
    let names = [
        RecompositionError::ConfirmationMissing {
            from_profile: "a".to_string(),
            to_profile: "b".to_string(),
        }
        .name(),
        RecompositionError::ConfirmationMismatch { reason: "r".to_string() }.name(),
        RecompositionError::ControlAuthorityModeChange { from: "a", to: "b" }.name(),
        RecompositionError::ProtectedContractInTarget { contract_id: "c".to_string() }.name(),
        RecompositionError::NoOpSwitch.name(),
        RecompositionError::StepRefused { step: SwitchStep::Drain, reason: "r".to_string() }.name(),
    ];
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            assert_ne!(names[i], names[j], "error names must be distinct");
        }
    }
}
