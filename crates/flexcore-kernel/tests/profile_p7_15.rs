// P7.15 — Composition profiles and bundles: load, ceiling, mode-lock, and
// plan-validation tests (ledger P7.15; test file written, NOT run — the
// loop bans hold; the human build/test pass executes these).
//
// Coverage (ledger P7.15 acceptance criteria, insofar as they are code
// properties):
//
// 1. Each of the five shipped profiles (`plugins/profiles/*.profile.json`)
//    LOADS from its definition file alone (strict closed-key parse) — the
//    ledger's "load each profile from its definition file alone".
// 2. Ceiling violations are refused with named errors; a profile can
//    never grant beyond its ceiling.
// 3. The control-authority posture is LOCKED to `simulation-shadow` — any
//    other value (including the other §21.4 modes) is
//    `CONTROL_AUTHORITY_MODE_FORBIDDEN`; the profile mechanism can never
//    select a control-capable mode (report §5.8/§21.4).
// 4. An unknown manifest reference is refused
//    (`PROFILE_SELECTION_UNRESOLVABLE`).
// 5. Profile → plan validation is DETERMINISTIC: same inputs → identical
//    plan, across repeated runs and shuffled manifest input orders.
// 6. A kernel-protected contract claim is refused THROUGH a profile
//    (the resolver's guard, wrapped, never weakened).
// 7. The research profiles grant no dispatch-side permission: a manifest
//    requesting `schedule.propose` or `dispatch.propose` is refused by
//    their ceilings (the structural "no dispatch authority" test).

use flexcore_kernel::manifest::{parse_manifest, Manifest};
use flexcore_kernel::profile::{
    parse_profile, validate_profile, ControlAuthorityPosture, Profile, ProfileError,
    SHIPPED_PROFILE_IDS,
};
use flexcore_kernel::resolve::CompositionPlan;

// The shipped profile files, loaded from their definition files alone
// (paths relative to this test file: crates/flexcore-kernel/tests → repo
// root).
const SIM_LAB: &str = include_str!("../../../plugins/profiles/sim-lab.profile.json");
const VPP_OPERATOR: &str = include_str!("../../../plugins/profiles/vpp-operator.profile.json");
const SITE_EDGE: &str = include_str!("../../../plugins/profiles/site-edge.profile.json");
const RESEARCH_GNN: &str = include_str!("../../../plugins/profiles/research-gnn.profile.json");
const RESEARCH_MARL: &str = include_str!("../../../plugins/profiles/research-marl.profile.json");

// --- Fixtures ------------------------------------------------------------------

/// A minimal valid seed-built-in manifest fixture (Class A, no consumes,
/// covering the pinned platform API version 0.5.0).
fn seed_manifest_json(plugin_id: &str, provides: &[&str], permissions: &[&str]) -> String {
    let provides = provides
        .iter()
        .map(|c| format!("    \"{c}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    let permissions = permissions
        .iter()
        .map(|p| format!("    \"{p}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        r#"{{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "{plugin_id}",
  "name": "Seed fixture for {plugin_id}",
  "version": "0.5.0",
  "api-version": ">=0.5.0",
  "class": "builtin-rust",
  "entry-point": {{
    "registered-type": "{plugin_id}/default"
  }},
  "provider-of": [
{provides}
  ],
  "consumes": [],
  "permissions": [
{permissions}
  ],
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

fn parse_fixture(plugin_id: &str, provides: &[&str], permissions: &[&str]) -> Manifest {
    parse_manifest(&seed_manifest_json(plugin_id, provides, permissions))
        .expect("fixture manifest must be valid")
}

/// The three seed built-ins, as resolvable manifests for the active
/// profile selections.
fn seed_set() -> Vec<Manifest> {
    vec![
        parse_fixture("builtin.simulator-telemetry", &["telemetry.source"], &["telemetry.read"]),
        parse_fixture("builtin.highs-optimizer", &["optimization.solver"], &["schedule.propose"]),
        parse_fixture("builtin.pandapower-grid", &["grid.model"], &[]),
    ]
}

// --- 1. Each shipped profile loads from its definition file alone ---------------

#[test]
fn each_shipped_profile_loads_from_its_definition_file_alone() {
    for (source, expected_id) in [
        (SIM_LAB, "sim-lab"),
        (VPP_OPERATOR, "vpp-operator"),
        (SITE_EDGE, "site-edge"),
        (RESEARCH_GNN, "research-gnn"),
        (RESEARCH_MARL, "research-marl"),
    ] {
        let profile = parse_profile(source)
            .unwrap_or_else(|err| panic!("{expected_id} must load: {err} ({})", err.name()));
        assert_eq!(profile.profile_id(), expected_id);
        assert_eq!(profile.version().to_string(), "0.5.0");
        // The mode lock holds on every shipped profile: the ONLY posture
        // profile data can carry.
        assert_eq!(profile.control_authority(), ControlAuthorityPosture::SimulationShadow);
        assert_eq!(profile.control_authority().name(), "simulation-shadow");
    }
    // And the shipped set is exactly the five §5.8 names.
    assert_eq!(SHIPPED_PROFILE_IDS.len(), 5);
}

#[test]
fn sim_lab_loads_with_the_seed_selections_active_and_deferred_entries_stripped() {
    let profile = parse_profile(SIM_LAB).expect("sim-lab must load");
    let selected: Vec<&str> = profile
        .selections()
        .iter()
        .map(|s| s.manifest_id.as_str())
        .collect();
    // The three active seed selections, in declared order.
    assert_eq!(
        selected,
        vec!["builtin.simulator-telemetry", "builtin.highs-optimizer", "builtin.pandapower-grid"]
    );
    // The commented-out deferred entries are NOT loaded — they are
    // documentation of intent, nothing more.
    assert!(!selected.contains(&"neryva.sim-builtin-ess"));
    assert!(!selected.contains(&"builtin.mock-market"));
    assert!(!selected.contains(&"builtin.replay-tools"));
    // The ceiling is the sim-lab ceiling (no dispatch-side permission).
    let ceiling: Vec<&str> = profile.permission_ceiling().iter().map(|p| p.as_str()).collect();
    assert_eq!(ceiling, vec!["asset.read", "schedule.propose", "telemetry.read"]);
}

#[test]
fn site_edge_loads_as_a_shell_and_composes_nothing() {
    let profile = parse_profile(SITE_EDGE).expect("site-edge must load as a shell");
    assert!(profile.selections().is_empty());
    // A shell loads but NEVER composes: named refusal at plan validation.
    let err = validate_profile(&profile, &seed_set()).expect_err("a shell must not compose");
    assert_eq!(err.name(), "PROFILE_EMPTY_SELECTIONS");
}

#[test]
fn research_profiles_load_with_no_dispatch_side_permission_in_the_ceiling() {
    for source in [RESEARCH_GNN, RESEARCH_MARL] {
        let profile = parse_profile(source).expect("the research profile must load");
        let ceiling: Vec<&str> = profile.permission_ceiling().iter().map(|p| p.as_str()).collect();
        assert_eq!(ceiling, vec!["asset.read", "telemetry.read"]);
        for forbidden in ["schedule.propose", "dispatch.propose", "dispatch.validate.request", "dispatch.authorize", "dispatch.issue"] {
            assert!(
                !profile.permits(forbidden),
                "the research ceiling must withhold {forbidden}"
            );
        }
    }
}

// --- 2. Ceiling violations are refused ------------------------------------------

#[test]
fn a_manifest_outside_the_ceiling_is_refused() {
    let profile = parse_profile(SIM_LAB).expect("sim-lab must load");
    // forecast.publish is NOT in the sim-lab ceiling.
    let manifests = vec![
        parse_fixture("builtin.simulator-telemetry", &["telemetry.source"], &["telemetry.read"]),
        parse_fixture("builtin.highs-optimizer", &["optimization.solver"], &["forecast.publish"]),
        parse_fixture("builtin.pandapower-grid", &["grid.model"], &[]),
    ];
    let err = validate_profile(&profile, &manifests)
        .expect_err("a manifest outside the ceiling must be refused");
    match err {
        ProfileError::ProfilePermissionCeilingExceeded { manifest_id, permission } => {
            assert_eq!(manifest_id, "builtin.highs-optimizer");
            assert_eq!(permission, "forecast.publish");
        }
        other => panic!("named ceiling refusal expected, got {}", other.name()),
    }
}

#[test]
fn a_research_profile_refuses_even_a_propose_permission_the_structural_no_dispatch_test() {
    let profile = parse_profile(RESEARCH_GNN).expect("research-gnn must load");
    // A manifest with a propose permission inside the research profile:
    // refused by the ceiling — this is the structural form of "grant no
    // dispatch authority" (the never-granted set is refused one layer
    // earlier at the manifest gate, so dispatch.authorize cannot even be
    // a fixture here).
    let manifests = vec![
        parse_fixture("builtin.pandapower-grid", &["grid.model"], &[]),
        parse_fixture("builtin.simulator-telemetry", &["telemetry.source"], &["telemetry.read"]),
        parse_fixture("research.gnn-fixture", &["optimization.solver"], &["schedule.propose"]),
    ];
    // The profile's selections don't name the fixture; validation only
    // checks selected manifests — so select it explicitly first.
    let selection_json = RESEARCH_GNN.replace(
        "\"manifest-id\": \"builtin.simulator-telemetry\",\n      \"version-constraint\": \">=0.5.0\"\n    }",
        "\"manifest-id\": \"research.gnn-fixture\",\n      \"version-constraint\": \">=0.5.0\"\n    }",
    );
    let profile = parse_profile(&selection_json).expect("the edited profile must load");
    let err = validate_profile(&profile, &manifests).expect_err("dispatch-side permission must be refused");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_EXCEEDED");
}

#[test]
fn the_ceiling_vocabulary_is_closed_and_never_granted_ids_are_refused() {
    let base = parse_profile(SIM_LAB).expect("sim-lab must load");
    let _ = base;
    let json = SIM_LAB.replace("\"telemetry.read\"", "\"dispatch.authorize\"");
    let err = parse_profile(&json).expect_err("a never-granted ceiling entry must be refused");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_NEVER_GRANTED");
    let json = SIM_LAB.replace("\"telemetry.read\"", "\"secret.read\"");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_PERMISSION_CEILING_NEVER_GRANTED");
    let json = SIM_LAB.replace("\"telemetry.read\"", "\"tenant.admin\"");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_PERMISSION_CEILING_UNKNOWN");
}

// --- 3. The control-authority posture is locked ----------------------------------

#[test]
fn any_control_authority_value_other_than_simulation_shadow_is_a_named_error() {
    for forbidden in [
        "operator-approved",
        "bounded-automation",
        "read-only-shadow",
        "simulation",
        "shadow",
        "operator-approved,",
        "",
    ] {
        let json = SIM_LAB.replace("\"control-authority\": \"simulation-shadow\"", &format!("\"control-authority\": \"{forbidden}\""));
        let err = parse_profile(&json)
            .expect_err("the profile mechanism can never select a control-capable mode");
        assert_eq!(err.name(), "CONTROL_AUTHORITY_MODE_FORBIDDEN");
        match &err {
            ProfileError::ControlAuthorityModeForbidden { value } => {
                assert_eq!(value, &forbidden);
            }
            other => panic!("mode-lock error expected, got {}", other.name()),
        }
    }
}

#[test]
fn the_posture_enum_has_the_four_21_4_modes_but_data_reaches_only_simulation_shadow() {
    // The closed vocabulary mirrors §21.4 (for the P7.16 defence-in-depth
    // comparison); profile DATA can only ever produce SimulationShadow.
    assert_eq!(ControlAuthorityPosture::SimulationShadow.name(), "simulation-shadow");
    assert_eq!(ControlAuthorityPosture::ReadOnlyShadow.name(), "read-only-shadow");
    assert_eq!(ControlAuthorityPosture::OperatorApproved.name(), "operator-approved");
    assert_eq!(ControlAuthorityPosture::BoundedAutomation.name(), "bounded-automation");
}

// --- 4. Unknown manifest references are refused ----------------------------------

#[test]
fn an_unknown_manifest_reference_is_refused() {
    let profile = parse_profile(SIM_LAB).expect("sim-lab must load");
    // Only two of the three selected manifests are supplied.
    let manifests = vec![
        parse_fixture("builtin.simulator-telemetry", &["telemetry.source"], &["telemetry.read"]),
        parse_fixture("builtin.highs-optimizer", &["optimization.solver"], &["schedule.propose"]),
    ];
    let err = validate_profile(&profile, &manifests)
        .expect_err("an unknown manifest reference must be refused");
    match err {
        ProfileError::ProfileSelectionUnresolvable { manifest_id } => {
            assert_eq!(manifest_id, "builtin.pandapower-grid");
        }
        other => panic!("named refusal expected, got {}", other.name()),
    }
}

#[test]
fn an_unsatisfied_version_constraint_is_refused() {
    let edited = SIM_LAB
        .replace(
            "\"manifest-id\": \"builtin.highs-optimizer\",\n      \"version-constraint\": \">=0.5.0\"",
            "\"manifest-id\": \"builtin.highs-optimizer\",\n      \"version-constraint\": \">=1.0.0\"",
        );
    let profile = parse_profile(&edited).expect("the edited profile must load");
    let err = validate_profile(&profile, &seed_set())
        .expect_err("a version below the constraint must be refused");
    assert_eq!(err.name(), "PROFILE_MANIFEST_VERSION_UNSATISFIED");
}

// --- 5. Profile → plan validation is deterministic -------------------------------

#[test]
fn profile_plan_validation_is_deterministic_across_runs_and_input_orders() {
    let profile = parse_profile(SIM_LAB).expect("sim-lab must load");
    let plan_a: CompositionPlan = validate_profile(&profile, &seed_set()).expect("sim-lab must compose");
    let plan_b: CompositionPlan = validate_profile(&profile, &seed_set()).expect("repeat run");
    assert_eq!(plan_a, plan_b, "same inputs → identical plan");

    // Shuffled manifest input order: the same plan (the resolver is a pure
    // function; the profile processes selections in declared order).
    let mut shuffled = seed_set();
    shuffled.reverse();
    let plan_c: CompositionPlan = validate_profile(&profile, &shuffled).expect("shuffled run");
    assert_eq!(plan_a, plan_c);

    // The plan is the resolver's output shape: three registrations, three
    // resolved contracts, all explicitly selected by the profile.
    assert_eq!(plan_a.registrations.len(), 3);
    assert_eq!(plan_a.contracts.len(), 3);
    for contract in &plan_a.contracts {
        assert_eq!(contract.basis.name(), "SELECTION_EXPLICIT");
    }
}

// --- 6. Protected-kernel claims are refused through a profile --------------------

#[test]
fn a_protected_kernel_claim_is_refused_through_a_profile() {
    // A manifest claiming a protected contract id cannot exist in ANY
    // plan; selecting it through a profile changes nothing.
    let hostile = parse_fixture("hostile.kernel-claim", &["kernel.command-authorizer"], &[]);
    let profile_json = SIM_LAB.replace(
        "\"manifest-id\": \"builtin.highs-optimizer\"",
        "\"manifest-id\": \"hostile.kernel-claim\"",
    );
    let profile = parse_profile(&profile_json).expect("the profile data itself parses");
    let manifests = vec![
        parse_fixture("builtin.simulator-telemetry", &["telemetry.source"], &["telemetry.read"]),
        hostile,
        parse_fixture("builtin.pandapower-grid", &["grid.model"], &[]),
    ];
    let err = validate_profile(&profile, &manifests)
        .expect_err("a protected-kernel claim must be refused");
    match err {
        ProfileError::ProfileProtectedContractRefused { manifest_id, contract_id } => {
            assert_eq!(manifest_id, "hostile.kernel-claim");
            assert_eq!(contract_id, "kernel.command-authorizer");
        }
        other => panic!("named refusal expected, got {}", other.name()),
    }
}

// --- 7. Strict parse rules (closed key sets, envelope, shape) --------------------

#[test]
fn profile_parse_rules_are_fail_closed() {
    // Wrong envelope id.
    let json = SIM_LAB.replace("flexcore-profile-v1", "flexcore-profile-v2");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_SCHEMA_UNSUPPORTED");
    // Unknown top-level field (a smuggled control knob is a rejection).
    let json = SIM_LAB.replace("\"profile-id\"", "\"enable-dispatch\": true,\n  \"profile-id\"");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_UNKNOWN_FIELD");
    // Unknown selection field.
    let json = SIM_LAB.replace(
        "\"manifest-id\": \"builtin.highs-optimizer\",",
        "\"manifest-id\": \"builtin.highs-optimizer\",\n      \"auto-start\": true,",
    );
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_UNKNOWN_FIELD");
    // Duplicate selection.
    let json = SIM_LAB.replace(
        "\"manifest-id\": \"builtin.pandapower-grid\"",
        "\"manifest-id\": \"builtin.highs-optimizer\"",
    );
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_DUPLICATE_SELECTION");
    // Malformed version constraint (not a parseable range).
    let json = SIM_LAB.replace(">=0.5.0", "latest");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_VERSION_CONSTRAINT_INVALID");
    // Malformed selection manifest id.
    let json = SIM_LAB.replace("builtin.pandapower-grid", "Builtin.Pandapower");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_SELECTION_MANIFEST_ID_MALFORMED");
    // Not an object.
    assert_eq!(parse_profile("[1]").unwrap_err().name(), "PROFILE_NOT_AN_OBJECT");
    // Oversize source.
    let big = " ".repeat(70_000);
    assert_eq!(parse_profile(&big).unwrap_err().name(), "PROFILE_JSON_INVALID");
    // Missing required field (the control-authority field removed entirely).
    let json = SIM_LAB.replace("\"control-authority\": \"simulation-shadow\",", "");
    assert_eq!(parse_profile(&json).unwrap_err().name(), "PROFILE_MISSING_FIELD");
}

#[test]
fn two_selections_pin_one_contract_is_a_named_error() {
    // Two distinct manifests both providing `grid.model`, both selected:
    // the profile must refuse — never a silent last-wins.
    let grid_a = parse_fixture("acme.grid-a", &["grid.model"], &[]);
    let grid_b = parse_fixture("acme.grid-b", &["grid.model"], &[]);
    let profile_json = SIM_LAB
        .replace(
            "\"manifest-id\": \"builtin.pandapower-grid\"",
            "\"manifest-id\": \"acme.grid-a\"",
        )
        .replace(
            "\"manifest-id\": \"builtin.simulator-telemetry\"",
            "\"manifest-id\": \"acme.grid-b\"",
        );
    let profile: Profile = parse_profile(&profile_json).expect("the edited profile must load");
    let manifests = vec![
        grid_a,
        grid_b,
        parse_fixture("builtin.highs-optimizer", &["optimization.solver"], &["schedule.propose"]),
    ];
    let err = validate_profile(&profile, &manifests).expect_err("a double pin must be refused");
    assert_eq!(err.name(), "PROFILE_DUPLICATE_CONTRACT_PIN");
}

#[test]
fn every_profile_error_name_is_distinct() {
    use flexcore_kernel::resolve::ResolveError;
    let names = [
        ProfileError::ProfileJsonInvalid { reason: "x".to_string() }.name(),
        ProfileError::ProfileNotAnObject.name(),
        ProfileError::ProfileSchemaUnsupported { found: "v2".to_string() }.name(),
        ProfileError::ProfileUnknownField { location: "l".to_string(), field: "f".to_string() }.name(),
        ProfileError::ProfileMissingField { field: "f".to_string(), location: "l".to_string() }.name(),
        ProfileError::ProfileWrongType {
            location: "l".to_string(),
            field: "f".to_string(),
            expected: "string",
            found: "number",
        }
        .name(),
        ProfileError::ProfileIdMalformed { value: "v".to_string(), reason: "r".to_string() }.name(),
        ProfileError::ProfileVersionConstraintInvalid {
            manifest_id: "m".to_string(),
            value: "v".to_string(),
            reason: "r".to_string(),
        }
        .name(),
        ProfileError::ProfileSelectionManifestIdMalformed { value: "v".to_string(), reason: "r".to_string() }.name(),
        ProfileError::ProfileDuplicateSelection { manifest_id: "m".to_string() }.name(),
        ProfileError::ProfileSelectionCountExcessive { count: 9_999 }.name(),
        ProfileError::ProfilePermissionCeilingUnknown { value: "p".to_string() }.name(),
        ProfileError::ProfilePermissionCeilingNeverGranted { value: "p".to_string() }.name(),
        ProfileError::ProfilePermissionCeilingDuplicate { value: "p".to_string() }.name(),
        ProfileError::ControlAuthorityModeForbidden { value: "operator-approved".to_string() }.name(),
        ProfileError::ProfileEmptySelections.name(),
        ProfileError::ProfileSelectionUnresolvable { manifest_id: "m".to_string() }.name(),
        ProfileError::ProfileManifestVersionUnsatisfied {
            manifest_id: "m".to_string(),
            constraint: ">=1.0.0".to_string(),
            found: "0.5.0".to_string(),
        }
        .name(),
        ProfileError::ProfilePermissionCeilingExceeded {
            manifest_id: "m".to_string(),
            permission: "p".to_string(),
        }
        .name(),
        ProfileError::ProfileProtectedContractRefused {
            manifest_id: "m".to_string(),
            contract_id: "kernel.audit-integrity".to_string(),
        }
        .name(),
        ProfileError::ProfileDuplicateContractPin { contract_id: "c".to_string() }.name(),
        ProfileError::ProfileResolutionRefused {
            source: ResolveError::EmptyManifestSet,
        }
        .name(),
    ];
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            assert_ne!(names[i], names[j], "error names must be distinct");
        }
    }
}
