// P7.06 — Dependency resolution: missing, incompatible, cyclic, conflicting
// providers (ledger P7.06; test file written, NOT run — the loop bans hold;
// the human build/test pass executes these).
//
// Coverage (ledger P7.06 acceptance criteria):
//
// 1. Happy path: a valid multi-provider graph resolves to a plan whose
//    registration order is topological with plugin-id tie-breaking — and the
//    SAME input yields the IDENTICAL plan across runs (determinism, P7.42).
// 2. Each failure mode lands on its DISTINCT named error: MISSING_PROVIDER,
//    UNSUPPORTED_API_VERSION, CYCLIC_DEPENDENCY (with the cycle named),
//    CONFLICTING_PROVIDERS, SELECTION_UNRESOLVED, PROTECTED_KERNEL_REPLACE,
//    DUPLICATE_PLUGIN_ID, EMPTY_MANIFEST_SET, PLATFORM_API_UNSUPPORTED —
//    pairwise distinct across the suite.
// 3. SELECTION_EXPLICIT: an explicit selection deterministically resolves a
//    would-be conflict (never silent first-wins, report §5.5).
// 4. Protected-kernel guard: a manifest claiming any reserved contract id
//    (report §5.1/§5.7/§9.1) is refused — ADR-005/ADR-010 made structural.
// 5. Empty / missing manifest sets are refused, never planned as empty.
// 6. The resolver is pure: a failing resolution returns an error and leaves
//    NO plan behind (no activation exists in this crate at all).

use flexcore_kernel::manifest::{parse_manifest_value, Manifest, SemVer};
use flexcore_kernel::resolve::{
    resolve, ExplicitSelections, ResolveError, SelectionBasis, CURRENT_PLATFORM_API_VERSION,
    PROTECTED_KERNEL_CONTRACT_IDS,
};

// --- Test-manifest builder (goes through the real P7.05 validator) --------------

fn manifest_json(plugin_id: &str, provides: &[&str], consumes: &[&str], api_version: &str) -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": plugin_id,
        "name": "Test plugin",
        "version": "0.1.0",
        "api-version": api_version,
        "class": "builtin-rust",
        "entry-point": { "registered-type": "test/type" },
        "provider-of": provides,
        "consumes": consumes,
        "permissions": [],
        "configuration-schema": { "type": "object", "additionalProperties": false },
        "resources": { "max-memory-mb": 64, "timeout-ms": 1000 },
        "metadata": { "license": "Apache-2.0", "description": "resolver test fixture" }
    })
}

fn m(plugin_id: &str, provides: &[&str], consumes: &[&str]) -> Manifest {
    manifest_with_range(plugin_id, provides, consumes, ">=0.5,<1.0")
}

fn manifest_with_range(plugin_id: &str, provides: &[&str], consumes: &[&str], api_version: &str) -> Manifest {
    let value = manifest_json(plugin_id, provides, consumes, api_version);
    parse_manifest_value(&value).expect("resolver test fixture must be a valid manifest")
}

fn ids(registrations: &[flexcore_kernel::resolve::PlanRegistration]) -> Vec<String> {
    registrations.iter().map(|r| r.plugin_id.clone()).collect()
}

fn assert_rejected(
    manifests: &[Manifest],
    selections: &ExplicitSelections,
    variant_check: impl Fn(&ResolveError) -> bool,
    expected_name: &str,
) -> ResolveError {
    match resolve(manifests, selections) {
        Ok(plan) => panic!("expected rejection ({expected_name}), got a plan: {plan:?}"),
        Err(err) => {
            assert!(
                variant_check(&err),
                "expected {expected_name}, got {}: {err}",
                err.name()
            );
            assert_eq!(err.name(), expected_name);
            err
        }
    }
}

// --- 1. Happy path ---------------------------------------------------------------

#[test]
fn multi_provider_graph_resolves_in_topological_order() {
    let provider = m("alpha.ess-provider", &["ess.simulation"], &[]);
    let middle = m("beta.forecast", &["forecast.day-ahead"], &["ess.simulation"]);
    let consumer = m("gamma.scheduler", &[], &["ess.simulation", "forecast.day-ahead"]);
    // Deliberately shuffled input order: the plan must not depend on it.
    let manifests = vec![consumer, provider, middle];
    let plan = resolve(&manifests, &ExplicitSelections::new()).expect("resolves");

    assert_eq!(
        ids(&plan.registrations),
        vec![
            "alpha.ess-provider".to_string(),
            "beta.forecast".to_string(),
            "gamma.scheduler".to_string(),
        ]
    );
    // Contracts sorted; the multi-consumer contract lists its consumers sorted.
    let ess = plan
        .contracts
        .iter()
        .find(|c| c.contract_id == "ess.simulation")
        .expect("contract present");
    assert_eq!(ess.provider_plugin_id, "alpha.ess-provider");
    assert_eq!(ess.basis, SelectionBasis::UniqueProvider);
    assert_eq!(ess.consumers, vec!["beta.forecast".to_string(), "gamma.scheduler".to_string()]);
    // Every manifest lands exactly once.
    assert_eq!(plan.registrations.len(), 3);
    // Registration entries carry what the P7.08 registry needs.
    assert_eq!(plan.registrations[0].provides, vec!["ess.simulation".to_string()]);
    assert_eq!(plan.registrations[2].consumes.len(), 2);
}

#[test]
fn independent_plugins_are_ordered_by_plugin_id() {
    let zeta = m("zeta.market-mock", &["market.prices"], &[]);
    let kappa = m("kappa.telemetry", &["telemetry.history"], &[]);
    let manifests = vec![zeta, kappa];
    let plan = resolve(&manifests, &ExplicitSelections::new()).expect("resolves");
    assert_eq!(
        ids(&plan.registrations),
        vec!["kappa.telemetry".to_string(), "zeta.market-mock".to_string()]
    );
}

#[test]
fn same_input_yields_identical_plan_across_runs() {
    let a = m("alpha.ess-provider", &["ess.simulation"], &[]);
    let b = m("beta.forecast", &["forecast.day-ahead"], &["ess.simulation"]);
    let c = m("gamma.scheduler", &[], &["forecast.day-ahead"]);
    let run1 = resolve(&[c.clone(), a.clone(), b.clone()], &ExplicitSelections::new()).expect("resolves");
    let run2 = resolve(&[b, a, c], &ExplicitSelections::new()).expect("resolves");
    assert_eq!(run1, run2, "the plan must be a pure function of its input");
}

// --- 2. Failure modes, each a distinct named error --------------------------------

#[test]
fn missing_provider_is_named_with_its_consumers() {
    let consumer = m("gamma.scheduler", &[], &["market.prices"]);
    let err = assert_rejected(
        &[consumer],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::MissingProvider { contract_id, consumers }
            if contract_id == "market.prices" && consumers == &vec!["gamma.scheduler".to_string()]),
        "MISSING_PROVIDER",
    );
    assert!(err.to_string().contains("market.prices"));
}

#[test]
fn incompatible_api_version_is_named() {
    let provider = manifest_with_range("alpha.ess-provider", &["ess.simulation"], &[], "^0.5.0");
    let consumer = manifest_with_range(
        "gamma.scheduler",
        &[],
        &["ess.simulation"],
        ">=0.6,<1.0",
    );
    // [0.5.0, 0.6.0) vs [0.6.0, 1.0.0): touching but disjoint.
    assert_rejected(
        &[provider, consumer],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::UnsupportedApiVersion { contract_id, consumer_plugin_id, provider_plugin_id, .. }
            if contract_id == "ess.simulation"
                && consumer_plugin_id == "gamma.scheduler"
                && provider_plugin_id == "alpha.ess-provider"),
        "UNSUPPORTED_API_VERSION",
    );
}

#[test]
fn compatible_api_version_ranges_compose() {
    let provider = manifest_with_range("alpha.ess-provider", &["ess.simulation"], &[], "^0.5.0");
    let consumer = manifest_with_range("gamma.scheduler", &[], &["ess.simulation"], ">=0.5.2,<1.0");
    let plan = resolve(&[provider, consumer], &ExplicitSelections::new()).expect("overlapping ranges compose");
    assert_eq!(plan.registrations.len(), 2);
}

#[test]
fn cyclic_dependency_is_detected_and_named() {
    let p1 = m("alpha.first", &["graph.a"], &["graph.b"]);
    let p2 = m("beta.second", &["graph.b"], &["graph.a"]);
    let err = assert_rejected(
        &[p1, p2],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::CyclicDependency { cycle }
            if cycle.contains(&"alpha.first".to_string()) && cycle.contains(&"beta.second".to_string())),
        "CYCLIC_DEPENDENCY",
    );
    // The cycle is NAMED: first node repeated at the end, both members present.
    let cycle = match &err {
        ResolveError::CyclicDependency { cycle } => cycle.clone(),
        other => panic!("unexpected error {other:?}"),
    };
    assert_eq!(cycle.first(), cycle.last(), "the named cycle is closed");
    assert_eq!(cycle.len(), 3, "two plugins plus the repeated closing node");
}

#[test]
fn self_cycle_is_detected() {
    let p = m("alpha.selfish", &["graph.a"], &["graph.a"]);
    assert_rejected(
        &[p],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::CyclicDependency { cycle } if cycle == &vec!["alpha.selfish".to_string(), "alpha.selfish".to_string()]),
        "CYCLIC_DEPENDENCY",
    );
}

#[test]
fn conflicting_providers_without_selection_is_an_error_never_a_silent_choice() {
    let p1 = m("alpha.optimizer", &["optimization.lp"], &[]);
    let p2 = m("beta.optimizer", &["optimization.lp"], &[]);
    assert_rejected(
        &[p1, p2],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::ConflictingProviders { contract_id, providers }
            if contract_id == "optimization.lp"
                && providers == &vec!["alpha.optimizer".to_string(), "beta.optimizer".to_string()]),
        "CONFLICTING_PROVIDERS",
    );
}

#[test]
fn explicit_selection_resolves_the_conflict_deterministically() {
    let p1 = m("alpha.optimizer", &["optimization.lp"], &[]);
    let p2 = m("beta.optimizer", &["optimization.lp"], &[]);
    let mut selections = ExplicitSelections::new();
    selections.insert("optimization.lp".to_string(), "beta.optimizer".to_string());
    let run1 = resolve(&[p1.clone(), p2.clone()], &selections).expect("selection resolves the conflict");
    let run2 = resolve(&[p2, p1], &selections).expect("selection resolves the conflict");
    assert_eq!(run1, run2, "explicit selection is deterministic regardless of input order");
    // SELECTION_EXPLICIT: the resolved contract records the explicit basis.
    let contract = run1
        .contracts
        .iter()
        .find(|c| c.contract_id == "optimization.lp")
        .expect("contract present");
    assert_eq!(contract.provider_plugin_id, "beta.optimizer");
    assert_eq!(contract.basis, SelectionBasis::ExplicitSelection);
    assert_eq!(contract.basis.name(), "SELECTION_EXPLICIT");
}

#[test]
fn explicit_selection_pins_even_a_unique_provider() {
    let p = m("alpha.optimizer", &["optimization.lp"], &[]);
    let mut selections = ExplicitSelections::new();
    selections.insert("optimization.lp".to_string(), "alpha.optimizer".to_string());
    let plan = resolve(&[p], &selections).expect("pinning a unique provider is allowed");
    let contract = &plan.contracts[0];
    assert_eq!(contract.basis, SelectionBasis::ExplicitSelection);
}

#[test]
fn selections_referencing_missing_things_are_named() {
    let p = m("alpha.optimizer", &["optimization.lp"], &[]);
    // Selection names a plugin that does not provide the contract.
    let mut bad_plugin = ExplicitSelections::new();
    bad_plugin.insert("optimization.lp".to_string(), "delta.nobody".to_string());
    assert_rejected(
        &[p.clone()],
        &bad_plugin,
        |e| matches!(e, ResolveError::SelectionUnresolved { contract_id, plugin_id, .. }
            if contract_id == "optimization.lp" && plugin_id == "delta.nobody"),
        "SELECTION_UNRESOLVED",
    );
    // Selection names a contract nobody provides at all.
    let mut bad_contract = ExplicitSelections::new();
    bad_contract.insert("market.prices".to_string(), "alpha.optimizer".to_string());
    assert_rejected(
        &[p],
        &bad_contract,
        |e| matches!(e, ResolveError::SelectionUnresolved { contract_id, .. } if contract_id == "market.prices"),
        "SELECTION_UNRESOLVED",
    );
}

#[test]
fn protected_kernel_claim_is_refused_for_every_reserved_id() {
    for reserved in PROTECTED_KERNEL_CONTRACT_IDS {
        let claim = m("gamma.hostile", &[reserved], &[]);
        assert_rejected(
            &[claim],
            &ExplicitSelections::new(),
            |e| matches!(e, ResolveError::ProtectedKernelReplace { plugin_id, contract_id }
                if plugin_id == "gamma.hostile" && contract_id == reserved),
            "PROTECTED_KERNEL_REPLACE",
        );
    }
}

#[test]
fn duplicate_plugin_id_is_an_identity_conflict() {
    let p1 = m("alpha.dup", &["graph.a"], &[]);
    let p2 = m("alpha.dup", &["graph.b"], &[]);
    assert_rejected(
        &[p1, p2],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::DuplicatePluginId { plugin_id } if plugin_id == "alpha.dup"),
        "DUPLICATE_PLUGIN_ID",
    );
}

#[test]
fn empty_manifest_set_is_refused() {
    assert_rejected(
        &[],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::EmptyManifestSet),
        "EMPTY_MANIFEST_SET",
    );
}

#[test]
fn manifest_not_covering_platform_api_version_is_refused() {
    assert_eq!(CURRENT_PLATFORM_API_VERSION, SemVer::new(0, 5, 0));
    let p = manifest_with_range("alpha.future", &["graph.a"], &[], ">=1.0");
    assert_rejected(
        &[p],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::PlatformApiUnsupported { plugin_id, current_version, .. }
            if plugin_id == "alpha.future" && current_version == "0.5.0"),
        "PLATFORM_API_UNSUPPORTED",
    );
}

#[test]
fn resolution_failure_names_are_pairwise_distinct() {
    let names: Vec<&str> = vec![
        ResolveError::EmptyManifestSet.name(),
        ResolveError::ManifestSetTooLarge { count: 1 }.name(),
        ResolveError::DuplicatePluginId { plugin_id: "p".to_string() }.name(),
        ResolveError::ProtectedKernelReplace { plugin_id: "p".to_string(), contract_id: "c".to_string() }.name(),
        ResolveError::MissingProvider { contract_id: "c".to_string(), consumers: vec![] }.name(),
        ResolveError::UnsupportedApiVersion {
            contract_id: "c".to_string(),
            consumer_plugin_id: "a".to_string(),
            provider_plugin_id: "b".to_string(),
            consumer_range: "r".to_string(),
            provider_range: "r".to_string(),
        }
        .name(),
        ResolveError::PlatformApiUnsupported {
            plugin_id: "p".to_string(),
            declared_range: "r".to_string(),
            current_version: "v".to_string(),
        }
        .name(),
        ResolveError::CyclicDependency { cycle: vec![] }.name(),
        ResolveError::ConflictingProviders { contract_id: "c".to_string(), providers: vec![] }.name(),
        ResolveError::SelectionUnresolved { contract_id: "c".to_string(), plugin_id: "p".to_string(), reason: "r".to_string() }.name(),
    ];
    let mut sorted = names.clone();
    sorted.sort();
    for pair in sorted.windows(2) {
        assert_ne!(pair[0], pair[1], "resolver error names must be pairwise distinct");
    }
    assert_eq!(sorted.len(), 10);
}

// --- 3. Deeper composition shapes --------------------------------------------------

#[test]
fn deeper_chain_registers_dependencies_first() {
    // d -> c -> b -> a (d consumes from c, c from b, b from a).
    let a = m("alpha.base", &["chain.a"], &[]);
    let b = m("beta.mid", &["chain.b"], &["chain.a"]);
    let c = m("gamma.upper", &["chain.c"], &["chain.b"]);
    let d = m("delta.top", &[], &["chain.c"]);
    let plan = resolve(&[d, c, b, a], &ExplicitSelections::new()).expect("resolves");
    assert_eq!(
        ids(&plan.registrations),
        vec![
            "alpha.base".to_string(),
            "beta.mid".to_string(),
            "gamma.upper".to_string(),
            "delta.top".to_string(),
        ]
    );
}

#[test]
fn a_three_way_conflict_names_all_providers() {
    let p1 = m("alpha.optimizer", &["optimization.lp"], &[]);
    let p2 = m("beta.optimizer", &["optimization.lp"], &[]);
    let p3 = m("gamma.optimizer", &["optimization.lp"], &[]);
    let err = assert_rejected(
        &[p3, p1, p2],
        &ExplicitSelections::new(),
        |e| matches!(e, ResolveError::ConflictingProviders { providers } if providers.len() == 3),
        "CONFLICTING_PROVIDERS",
    );
    match &err {
        ResolveError::ConflictingProviders { providers, .. } => {
            assert_eq!(
                providers,
                &vec![
                    "alpha.optimizer".to_string(),
                    "beta.optimizer".to_string(),
                    "gamma.optimizer".to_string(),
                ]
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn protected_kernel_guard_fires_before_missing_provider_errors() {
    // A manifest that BOTH claims a protected contract and would trigger a
    // missing-provider error: the protected-kernel refusal is the structural
    // gate and comes first (fail-closed ordering, documented in resolve.rs).
    let hostile = m("gamma.hostile", &["kernel.dispatch-gateway"], &["market.prices"]);
    let err = resolve(&[hostile], &ExplicitSelections::new()).expect_err("refused");
    assert_eq!(err.name(), "PROTECTED_KERNEL_REPLACE");
}
