// P7.08 — Built-in Rust provider registry (Class A) tests (ledger P7.08;
// test file written, NOT run — the loop bans hold; the human build/test
// pass executes these).
//
// Coverage (ledger P7.08 acceptance criteria):
//
// 1. Register-from-plan happy path: the P7.06 resolver's `CompositionPlan`
//    drives registration; construction is on demand; iteration order is
//    deterministic (sorted contract ids).
// 2. Duplicate refusal: a second factory for one contract is a NAMED error,
//    never a silent overwrite.
// 3. Protected-kernel refusal: NO factory registers under any reserved
//    contract id — the P7.06 list, reconciled with the seam inventory
//    (P7.04 §6.1), enforced independently by the registry.
// 4. Swap blocked while the incumbent is Active, allowed after drain — the
//    P7.07 drain-before-replace rule through `swap_provider` (the P7.22
//    swap test itself is NOT implemented here; it is that wave's).
// 5. Plan-completeness: a missing factory and a stray factory are both
//    refused; a populated registry is never re-planned in place.
// 6. No global mutable registry: the registry is a composition-root-owned
//    value; nothing in this file (or the crate) touches a `static`.
//
// Test-provider honesty: the kernel crate depends on no `flexcore-*` crate
// (the P7.05/P7.06 dependency decision in `Cargo.toml`), so the REAL
// providers (`HighsTier1Provider`, `ClarabelTier2Provider`,
// `PandapowerGridProvider`, `SimulatorTelemetrySource`) cannot be
// constructed inside this test file. Per the recorded decision, the tests
// use LOCAL test factories over a trivial local trait, named after the real
// providers via the registry's seed constants — the real registration
// happens at the composition root through the same `FnFactory` adapter (see
// the decision record's seed-provider inventory).

use std::collections::BTreeMap;
use std::sync::Arc;

use flexcore_kernel::lifecycle::{
    DrainOutcome, DrainPolicy, DrainPort, FixedClock, GateOutcome, LifecycleAuditKind,
    LifecycleError, LifecycleState, MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::{parse_manifest_value, Manifest};
use flexcore_kernel::registry::{
    is_protected_kernel_contract, FnFactory, ProviderFactory, ProviderRegistry, RegistryError, seed,
};
use flexcore_kernel::resolve::{resolve, CompositionPlan, ExplicitSelections, PROTECTED_KERNEL_CONTRACT_IDS};

// --- Local test contract (trivial, no dependency on other crates) ---------------

trait Solver {
    fn solve_tag(&self) -> &'static str;
}

/// Local stand-in for the REAL `HighsTier1Provider` (LP/QP/MIP tier 1).
struct HighsLike;

impl Solver for HighsLike {
    fn solve_tag(&self) -> &'static str {
        "highs-like (test double)"
    }
}

/// Local stand-in for the REAL `ClarabelTier2Provider` (convex conic tier 2).
struct ClarabelLike;

impl Solver for ClarabelLike {
    fn solve_tag(&self) -> &'static str {
        "clarabel-like (test double)"
    }
}

fn highs_factory() -> Box<dyn ProviderFactory<dyn Solver>> {
    Box::new(FnFactory::new(seed::PROVIDER_HIGHS_TIER1, || -> Result<Arc<dyn Solver>, RegistryError> {
        Ok(Arc::new(HighsLike) as Arc<dyn Solver>)
    }))
}

fn clarabel_factory() -> Box<dyn ProviderFactory<dyn Solver>> {
    Box::new(FnFactory::new(seed::PROVIDER_CLARABEL_TIER2, || -> Result<Arc<dyn Solver>, RegistryError> {
        Ok(Arc::new(ClarabelLike) as Arc<dyn Solver>)
    }))
}

fn failing_factory() -> Box<dyn ProviderFactory<dyn Solver>> {
    Box::new(FnFactory::new(seed::PROVIDER_HIGHS_TIER1, || -> Result<Arc<dyn Solver>, RegistryError> {
        Err(RegistryError::construction(seed::CONTRACT_OPTIMIZER, "solver backend unavailable"))
    }))
}

// --- Plan fixtures (through the real P7.05 validator + P7.06 resolver) ----------

fn manifest_json(plugin_id: &str, provides: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": plugin_id,
        "name": "Registry test plugin",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "test/type" },
        "provider-of": provides,
        "consumes": [],
        "permissions": [],
        "configuration-schema": { "type": "object", "additionalProperties": false },
        "resources": { "max-memory-mb": 64, "timeout-ms": 1000 },
        "metadata": { "license": "Apache-2.0", "description": "registry test fixture" }
    })
}

fn m(plugin_id: &str, provides: &[&str]) -> Manifest {
    parse_manifest_value(&manifest_json(plugin_id, provides))
        .expect("registry test fixture must be a valid manifest")
}

/// The seed composition: the optimizer seam and the grid-model seam, each
/// with one first-party provider (the resolver's plan is the registration
/// list; contract ids are the DEFAULT-labelled seed ids).
fn seed_plan() -> CompositionPlan {
    let optimizer = m(seed::PLUGIN_OPTIMIZER_DEFAULT, &[seed::CONTRACT_OPTIMIZER]);
    let grid = m(seed::PLUGIN_GRID_MODEL_DEFAULT, &[seed::CONTRACT_GRID_MODEL]);
    resolve(&[optimizer, grid], &ExplicitSelections::new()).expect("seed plan resolves")
}

fn factories_for_plan() -> BTreeMap<String, Box<dyn ProviderFactory<dyn Solver>>> {
    let mut factories: BTreeMap<String, Box<dyn ProviderFactory<dyn Solver>>> = BTreeMap::new();
    factories.insert(seed::CONTRACT_OPTIMIZER.to_string(), highs_factory());
    factories.insert(seed::CONTRACT_GRID_MODEL.to_string(), clarabel_factory());
    factories
}

fn assert_error(err: RegistryError, expected_name: &str) -> RegistryError {
    assert_eq!(err.name(), expected_name, "unexpected error: {err}");
    err
}

/// The incumbent lifecycle for the optimizer provider, walked to Ready.
fn incumbent_to_ready<'a>(clock: &'a FixedClock, sink: &'a MemoryAuditSink) -> PluginLifecycle<'a> {
    let mut lifecycle =
        PluginLifecycle::register(seed::PLUGIN_OPTIMIZER_DEFAULT, "0.1.0", clock, sink, DrainPolicy::default());
    lifecycle
        .verify(GateOutcome::Passed, "sdk-installer", "provenance accepted")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "sdk-installer", "compatible")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "sdk-installer", "configured")
        .expect("configured");
    lifecycle
        .start(GateOutcome::Passed, "sdk-installer", "dependency graph resolved")
        .expect("started");
    lifecycle
        .mark_ready(GateOutcome::Passed, "health-check", "healthy")
        .expect("ready");
    lifecycle
}

/// A quiescing drain port for swap tests.
struct QuiesceDrain;

impl DrainPort for QuiesceDrain {
    fn outstanding_work(&self) -> usize {
        0
    }

    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        DrainOutcome::Quiesced { cancelled: 0 }
    }
}

// --- 1. Register-from-plan happy path ---------------------------------------------

#[test]
fn register_from_plan_happy_path_constructs_on_demand_and_iterates_deterministically() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry.register_from_plan(&plan, factories_for_plan()).expect("the whole plan registers");

    // Deterministic iteration: sorted contract ids (BTreeMap order).
    assert_eq!(
        registry.contract_ids(),
        vec![seed::CONTRACT_GRID_MODEL, seed::CONTRACT_OPTIMIZER]
    );
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());

    // The resolved provider plugin ids come from the plan.
    assert_eq!(
        registry.registered_plugin_id(seed::CONTRACT_OPTIMIZER),
        Some(seed::PLUGIN_OPTIMIZER_DEFAULT)
    );
    assert_eq!(
        registry.registered_plugin_id(seed::CONTRACT_GRID_MODEL),
        Some(seed::PLUGIN_GRID_MODEL_DEFAULT)
    );

    // Construction is on demand: two calls build two distinct instances.
    let first = registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructs");
    let second = registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructs");
    assert_eq!(first.solve_tag(), "highs-like (test double)");
    assert!(!Arc::ptr_eq(&first, &second), "the registry stores factories, never live providers");
    assert_eq!(
        registry.construct(seed::CONTRACT_GRID_MODEL).expect("constructs").solve_tag(),
        "clarabel-like (test double)"
    );
}

#[test]
fn the_seed_seam_constants_are_well_formed() {
    // The seed ids must satisfy the contract-id grammar and must NOT be
    // protected-kernel ids (they are the swappable Class A seams).
    for contract in [
        seed::CONTRACT_OPTIMIZER,
        seed::CONTRACT_GRID_MODEL,
        seed::CONTRACT_TELEMETRY_SOURCE,
    ] {
        assert!(!is_protected_kernel_contract(contract), "{contract} must be swappable");
    }
    for protected in PROTECTED_KERNEL_CONTRACT_IDS {
        assert!(is_protected_kernel_contract(protected), "{protected} must be protected");
    }
    // The real first-party provider names, as the seam inventory records them.
    assert_eq!(seed::PROVIDER_HIGHS_TIER1, "HighsTier1Provider");
    assert_eq!(seed::PROVIDER_CLARABEL_TIER2, "ClarabelTier2Provider");
    assert_eq!(seed::PROVIDER_PANDAPOWER_GRID, "PandapowerGridProvider");
    assert_eq!(seed::PROVIDER_SIMULATOR_TELEMETRY, "SimulatorTelemetrySource");
}

// --- 2. Duplicate refusal -----------------------------------------------------------

#[test]
fn duplicate_registration_is_refused_never_silently_overwritten() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry
        .register(seed::CONTRACT_OPTIMIZER, highs_factory())
        .expect("first registration is fine");
    let err = assert_error(
        registry.register(seed::CONTRACT_OPTIMIZER, clarabel_factory()).unwrap_err(),
        "DUPLICATE_CONTRACT_REGISTRATION",
    );
    match &err {
        RegistryError::DuplicateContractRegistration { contract_id } => {
            assert_eq!(contract_id, seed::CONTRACT_OPTIMIZER);
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The ORIGINAL factory still answers construction.
    assert_eq!(
        registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructs").solve_tag(),
        "highs-like (test double)"
    );
}

// --- 3. Protected-kernel reinforcement ----------------------------------------------

#[test]
fn no_factory_registers_under_any_protected_kernel_contract_id() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    for reserved in PROTECTED_KERNEL_CONTRACT_IDS {
        // Checked BEFORE the id grammar, the plan table, and duplicates —
        // the protected kernel is refused unconditionally.
        let err = assert_error(
            registry.register(reserved, highs_factory()).unwrap_err(),
            "PROTECTED_KERNEL_CONTRACT_REFUSED",
        );
        match &err {
            RegistryError::ProtectedKernelContract { contract_id } => {
                assert_eq!(contract_id, reserved);
            }
            other => panic!("unexpected error {other:?}"),
        }
        assert_eq!(registry.len(), 0, "nothing was registered");
    }
    // Even for a plan that somehow contained a protected id (the resolver
    // refuses one structurally, so this is defense in depth), the batch
    // path refuses too.
    let hostile = m("gamma.hostile", &["kernel.dispatch-gateway"]);
    let hostile_plan = resolve(&[hostile], &ExplicitSelections::new());
    assert!(hostile_plan.is_err(), "the resolver itself refuses the plan (P7.06)");
}

// --- 4. Plan-completeness and plan membership ----------------------------------------

#[test]
fn a_provider_for_a_contract_not_in_the_plan_is_refused() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let err = assert_error(
        registry.register("market.prices", highs_factory()).unwrap_err(),
        "CONTRACT_NOT_IN_PLAN",
    );
    match &err {
        RegistryError::ContractNotInPlan { contract_id } => assert_eq!(contract_id, "market.prices"),
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_malformed_contract_id_is_refused() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    assert_error(
        registry.register("NotNamespaced", highs_factory()).unwrap_err(),
        "CONTRACT_ID_MALFORMED",
    );
}

#[test]
fn a_missing_factory_for_a_plan_contract_is_refused_deterministically() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let mut partial = BTreeMap::new();
    partial.insert(seed::CONTRACT_OPTIMIZER.to_string(), highs_factory());
    let err = assert_error(
        registry.register_from_plan(&plan, partial).unwrap_err(),
        "FACTORY_MISSING_FOR_PLAN_CONTRACT",
    );
    match &err {
        RegistryError::FactoryMissingForPlanContract { contract_id } => {
            // The smallest plan contract id is reported first (deterministic).
            assert_eq!(contract_id, seed::CONTRACT_GRID_MODEL);
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_stray_factory_not_named_by_the_plan_is_refused() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let mut factories = factories_for_plan();
    factories.insert("market.prices".to_string(), highs_factory());
    let err = assert_error(
        registry.register_from_plan(&plan, factories).unwrap_err(),
        "CONTRACT_NOT_IN_PLAN",
    );
    match &err {
        RegistryError::ContractNotInPlan { contract_id } => assert_eq!(contract_id, "market.prices"),
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_populated_registry_is_never_re_planned_in_place() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry.register_from_plan(&plan, factories_for_plan()).expect("first plan registers");
    let err = assert_error(
        registry.register_from_plan(&plan, factories_for_plan()).unwrap_err(),
        "REGISTRY_ALREADY_POPULATED",
    );
    match &err {
        RegistryError::RegistryAlreadyPopulated { held_contracts } => assert_eq!(*held_contracts, 2),
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_factory_construction_failure_is_a_named_error_not_a_panic() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let mut factories = factories_for_plan();
    factories.insert(seed::CONTRACT_OPTIMIZER.to_string(), failing_factory());
    registry.register_from_plan(&plan, factories).expect("registration succeeds; construction is on demand");
    let err = assert_error(
        registry.construct(seed::CONTRACT_OPTIMIZER).unwrap_err(),
        "CONSTRUCTION_FAILED",
    );
    match &err {
        RegistryError::ConstructionFailed { contract_id, reason } => {
            assert_eq!(contract_id, seed::CONTRACT_OPTIMIZER);
            assert_eq!(reason, "solver backend unavailable");
        }
        other => panic!("unexpected error {other:?}"),
    }
}

// --- 5. Lifecycle-aware swap (drain-before-replace) -----------------------------------

#[test]
fn swap_is_blocked_while_active_and_allowed_after_drain() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry.register_from_plan(&plan, factories_for_plan()).expect("plan registers");

    let clock = FixedClock::new(1_000);
    let sink = MemoryAuditSink::new();
    let mut incumbent = incumbent_to_ready(&clock, &sink);

    // Active (Ready) without draining: REFUSED — drain-before-replace.
    let err = assert_error(
        registry
            .swap_provider(
                seed::CONTRACT_OPTIMIZER,
                clarabel_factory(),
                &mut incumbent,
                "0.2.0",
                "operator",
                "swap HiGHS-like for Clarabel-like on the same seam",
            )
            .unwrap_err(),
        "LIFECYCLE_REFUSED",
    );
    match &err {
        RegistryError::LifecycleRefused { contract_id, source } => {
            assert_eq!(contract_id, seed::CONTRACT_OPTIMIZER);
            assert_eq!(source.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The old factory still answers: nothing was replaced.
    assert_eq!(
        registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructs").solve_tag(),
        "highs-like (test double)"
    );
    // The refusal is audited on the lifecycle.
    assert_eq!(sink.records().last().expect("refusal audited").kind, LifecycleAuditKind::Refused);

    // Drain: Ready -> Draining -> Stopped (quiesced).
    let mut drain = QuiesceDrain;
    incumbent.begin_drain(&mut drain, "operator", "upgrade requested").expect("draining");
    incumbent.complete_drain("operator", "active work completed").expect("quiesced");
    assert_eq!(incumbent.state(), LifecycleState::Stopped);

    // NOW the swap proceeds, and the version change is recorded (audited).
    registry
        .swap_provider(
            seed::CONTRACT_OPTIMIZER,
            clarabel_factory(),
            &mut incumbent,
            "0.2.0",
            "operator",
            "swap HiGHS-like for Clarabel-like on the same seam",
        )
        .expect("the swap after completed drain");
    assert_eq!(
        registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructs").solve_tag(),
        "clarabel-like (test double)"
    );
    let records = sink.records();
    let version_change = records.last().expect("version-change record");
    assert_eq!(version_change.kind, LifecycleAuditKind::VersionChange);
    assert!(version_change.reason.contains("0.1.0 -> 0.2.0"), "the version change is recorded");
    assert_eq!(version_change.plugin_id, seed::PLUGIN_OPTIMIZER_DEFAULT);
}

#[test]
fn swap_is_blocked_while_draining_and_after_a_refused_deadline() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry.register_from_plan(&plan, factories_for_plan()).expect("plan registers");

    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut incumbent = incumbent_to_ready(&clock, &sink);

    // Draining but NOT quiesced: still blocked.
    let mut stuck = StuckDrain;
    incumbent.begin_drain(&mut stuck, "operator", "upgrade requested").expect("draining");
    let err = registry
        .swap_provider(
            seed::CONTRACT_OPTIMIZER,
            clarabel_factory(),
            &mut incumbent,
            "0.2.0",
            "operator",
            "mid-drain swap attempt",
        )
        .unwrap_err();
    assert_eq!(err.name(), "LIFECYCLE_REFUSED");
    match &err {
        RegistryError::LifecycleRefused { source, .. } => {
            assert_eq!(source.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
        }
        other => panic!("unexpected error {other:?}"),
    }

    // The drain deadline expires: the stop is refused, the incumbent is
    // back to Ready (active) — the swap stays blocked and the operator
    // decides (the deadline-refusal audit lives in the lifecycle).
    incumbent.complete_drain("operator", "attempted stop").unwrap_err();
    assert_eq!(incumbent.state(), LifecycleState::Ready);
    let err = registry
        .swap_provider(
            seed::CONTRACT_OPTIMIZER,
            clarabel_factory(),
            &mut incumbent,
            "0.2.0",
            "operator",
            "post-deadline swap attempt",
        )
        .unwrap_err();
    assert_eq!(err.name(), "LIFECYCLE_REFUSED");
}

/// A drain port that never quiesces (the deadline always expires).
struct StuckDrain;

impl DrainPort for StuckDrain {
    fn outstanding_work(&self) -> usize {
        7
    }

    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        DrainOutcome::DeadlineExceeded { outstanding: 7 }
    }
}

#[test]
fn swap_refuses_an_unknown_contract_and_a_mismatched_lifecycle() {
    let plan = seed_plan();
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    registry.register_from_plan(&plan, factories_for_plan()).expect("plan registers");

    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    // A lifecycle for the WRONG plugin (the grid provider's id).
    let mut wrong_plugin = PluginLifecycle::register(
        seed::PLUGIN_GRID_MODEL_DEFAULT,
        "0.1.0",
        &clock,
        &sink,
        DrainPolicy::default(),
    );

    let err = assert_error(
        registry
            .swap_provider(
                seed::CONTRACT_OPTIMIZER,
                clarabel_factory(),
                &mut wrong_plugin,
                "0.2.0",
                "operator",
                "lifecycle belongs to another plugin",
            )
            .unwrap_err(),
        "SWAP_LIFECYCLE_MISMATCH",
    );
    match &err {
        RegistryError::SwapLifecycleMismatch { contract_id, registered_plugin_id, lifecycle_plugin_id } => {
            assert_eq!(contract_id, seed::CONTRACT_OPTIMIZER);
            assert_eq!(registered_plugin_id, seed::PLUGIN_OPTIMIZER_DEFAULT);
            assert_eq!(lifecycle_plugin_id, seed::PLUGIN_GRID_MODEL_DEFAULT);
        }
        other => panic!("unexpected error {other:?}"),
    }

    // An unknown contract is refused before any lifecycle is consulted.
    let mut real_incumbent = incumbent_to_ready(&clock, &sink);
    // Drain it fully so the contract check is what fires.
    let mut drain = QuiesceDrain;
    real_incumbent.begin_drain(&mut drain, "operator", "upgrade").expect("draining");
    real_incumbent.complete_drain("operator", "quiesced").expect("stopped");
    let err = assert_error(
        registry
            .swap_provider(
                "market.prices",
                clarabel_factory(),
                &mut real_incumbent,
                "0.2.0",
                "operator",
                "unknown contract",
            )
            .unwrap_err(),
        "SWAP_CONTRACT_UNKNOWN",
    );
    match &err {
        RegistryError::SwapContractUnknown { contract_id } => assert_eq!(contract_id, "market.prices"),
        other => panic!("unexpected error {other:?}"),
    }

    // A protected contract id is refused on swap too, first of all.
    let mut drained_incumbent = incumbent_to_ready(&clock, &sink);
    let mut drain2 = QuiesceDrain;
    drained_incumbent.begin_drain(&mut drain2, "operator", "upgrade").expect("draining");
    drained_incumbent.complete_drain("operator", "quiesced").expect("stopped");
    assert_error(
        registry
            .swap_provider(
                "kernel.dispatch-gateway",
                clarabel_factory(),
                &mut drained_incumbent,
                "0.2.0",
                "operator",
                "protected contracts are never swappable",
            )
            .unwrap_err(),
        "PROTECTED_KERNEL_CONTRACT_REFUSED",
    );
}

// --- 6. Registry error names are pairwise distinct -------------------------------------

#[test]
fn registry_error_names_are_pairwise_distinct() {
    let names: Vec<&str> = vec![
        RegistryError::ProtectedKernelContract { contract_id: "c".to_string() }.name(),
        RegistryError::ContractIdMalformed { contract_id: "c".to_string(), reason: "r".to_string() }.name(),
        RegistryError::ContractNotInPlan { contract_id: "c".to_string() }.name(),
        RegistryError::FactoryMissingForPlanContract { contract_id: "c".to_string() }.name(),
        RegistryError::RegistryAlreadyPopulated { held_contracts: 1 }.name(),
        RegistryError::DuplicateContractRegistration { contract_id: "c".to_string() }.name(),
        RegistryError::ContractNotRegistered { contract_id: "c".to_string() }.name(),
        RegistryError::ConstructionFailed { contract_id: "c".to_string(), reason: "r".to_string() }.name(),
        RegistryError::SwapContractUnknown { contract_id: "c".to_string() }.name(),
        RegistryError::SwapLifecycleMismatch {
            contract_id: "c".to_string(),
            registered_plugin_id: "p".to_string(),
            lifecycle_plugin_id: "q".to_string(),
        }
        .name(),
        RegistryError::LifecycleRefused {
            contract_id: "c".to_string(),
            source: LifecycleError::DrainPortMissing,
        }
        .name(),
    ];
    let mut sorted = names.clone();
    sorted.sort();
    for pair in sorted.windows(2) {
        assert_ne!(pair[0], pair[1], "registry error names must be pairwise distinct");
    }
    assert_eq!(sorted.len(), 11);
}
