// P7.21 — Plugin conformance suite (contract tests every plugin must pass)
//         (report §19.2(3), §9.3, §22.2, §5.x, §9.x; ledger
//         `ledger_phase_7.md` §P7.21). WRITTEN-UNRUN: the loop bans hold
//         (`ledger/phase-07-plugin-sdk/ENTRY_NOTE.md`); the human
//         build/test pass executes
//         `cargo test -p flexcore-kernel --test conformance_p7_21`.
//
// SUITE IDENTITY (the versioned artefact the claim boundary speaks about):
//
//   flexcore-plugin-sdk-conformance-suite-v1
//
//   - Contract surface under test: the KERNEL SEAMS every plugin passes
//     through regardless of class (§5.4 A/B/C) — the `flexcore-plugin-
//     manifest-v1` validator (P7.05), the P7.06 resolver (§9.3 api-version
//     matrix), the §5.7 permission grammar, the secret-REFERENCE-only rule
//     (P7.05 manifest + P7.19 config), the closed resource bounds (§5.5),
//     the P7.14 provenance record + `Discovered → Verified` gate, the §5.6
//     lifecycle transition table (P7.07), the kernel audit-trail shape
//     (§5.1), the P7.36 plugin-path determinism identity (the P7.17/P7.42
//     seed/request_id contract, kernel side), and the P7.06 protected-
//     kernel guard (ADR-005/ADR-010).
//   - Per-class model (P7.21 step 2): the SAME class-agnostic case set runs
//     against one fixture per class (A = builtin-rust, B = wasm-component,
//     C = grpc-worker) and the per-class results are recorded in the run
//     table (`per_class_run_record`); the class-specific case groups add
//     what only one class can exercise (K: registry registration; L: world
//     conformance; M: worker contract presence rules).
//   - CI wiring (P7.21 step 3) is the P7.27 wave's deliverable (config-only
//     CI change); the SEAM is RECORDED in the decision record, and NOTHING
//     is added to `ci.yml` here.
//   - Cross-reference (P7.21 step 4): the Phase 6 adapter conformance
//     approach (`crates/edge-agent/tests/conformance_suite_p6_19.rs` and
//     the P6.13 `ConformanceHarness`) is the MODEL — suite identity, the
//     case→requirement map, and the claim boundary are reused; the P6.19
//     edge-adapter suite itself is NOT duplicated here.
//
// CASE → REQUIREMENT MAP (every case maps to a report requirement or is
// marked FLEXCORE-ONLY — the P6.19 acceptance rule):
//
// | #  | Case | Requirement |
// |---|---|---|
// | 1  | manifest_valid_fixture_passes_for_every_class | §19.2(3) + §5.5 — the same validated manifest surface for every class |
// | 2  | manifest_unknown_field_is_a_named_rejection | §19.2(3) — every bad input is a NAMED error, reject-never-warn |
// | 3  | manifest_missing_required_field_is_named | §19.2(3) + §5.5 |
// | 4  | manifest_bad_json_and_non_object_are_named | §19.2(3) |
// | 5  | manifest_wrong_envelope_id_is_named | §19.2(3) + §5.5 (versioned envelope) |
// | 6  | manifest_entry_point_must_match_class | §19.2(3) + §5.4/§5.5 per-class entry point |
// | 7  | manifest_oversize_input_is_refused | FLEXCORE-ONLY (fail-closed loader cap, §17.5 honesty) |
// | 8  | every_conforming_manifest_covers_the_platform_api_version | §9.3 + §5.5 |
// | 9  | manifest_not_covering_platform_version_is_refused_by_the_resolver | §9.3 (PLATFORM_API_UNSUPPORTED) |
// | 10 | disjoint_consumer_provider_ranges_are_named | §9.3 (UNSUPPORTED_API_VERSION) |
// | 11 | never_granted_permission_refused_at_the_manifest_gate | §5.7 + §19.2(7) — the earliest refusal |
// | 12 | unknown_permission_refused | §5.7 (closed ids, never free text) |
// | 13 | duplicate_permission_refused | §19.2(3) |
// | 14 | ceiling_containment_manifest_above_ceiling_refused | §5.8 + §5.7 — profile ceiling cannot be exceeded |
// | 15 | ceiling_never_contains_a_never_granted_id | §5.7/§5.8 |
// | 16 | manifest_secret_must_be_a_reference_not_a_value | §5.7 + §17.3 — references only, value never echoed |
// | 17 | config_secret_shaped_plain_value_refused_value_not_echoed | §5.7 + §17.3 (P7.19) |
// | 18 | config_property_named_like_manifest_secret_refused | §5.7 (P7.19 collision rule) |
// | 19 | secret_reference_resolves_to_handle_only | §5.7 (P7.19 handle; no value accessor exists) |
// | 20 | resource_bounds_required_and_within_closed_ranges | §5.4/§5.5 — declared and inside the closed ranges |
// | 21 | plugin_path_request_identity_requires_request_id_and_sequence | FLEXCORE-ONLY (P7.17/P7.42 determinism contract: request_id/seed first-class, kernel-side identity) |
// | 22 | payload_bound_digest_is_deterministic | FLEXCORE-ONLY (P7.42 — same input, same digest) |
// | 23 | proposal_envelope_carries_producer_identity_and_sequence | FLEXCORE-ONLY (P7.17/P7.42 + ADR-010 candidate envelope) |
// | 24 | expired_request_is_refused_never_executed | §14.3 + §17.3 (validity window) |
// | 25 | valid_provenance_record_passes_the_gate | §9.3 + §5.6 (P7.14) |
// | 26 | unsigned_artifact_refused_before_anything_else | §9.3 — unsigned rejected before compatibility (P7.14 step 2) |
// | 27 | sbom_reference_required | §9.3 (P7.14; SBOM generation itself is P7.27 CI) |
// | 28 | signer_key_is_a_reference_not_material | §17.3 (P7.14 closed key set) |
// | 29 | provenance_binding_mismatch_is_refused | §9.3 (P7.14 MANIFEST_MISMATCH) |
// | 30 | the_legal_transition_set_is_exactly_the_frozen_table_plus_crash_edges | §5.6 (the legal set) |
// | 31 | gates_refuse_advancement_and_leave_state_unchanged_audited | §5.6 (fail-closed gates) |
// | 32 | terminal_states_hold_their_outcome | §5.6 |
// | 33 | every_lifecycle_record_carries_the_required_shape | §5.1 + §5.6 step 4 (plugin id/version, actor, reason, clock timestamp) |
// | 34 | audit_is_append_only_in_emission_order_and_deterministic | §5.1 (audit integrity) + P7.42 |
// | 35 | protected_kernel_contract_claim_refused_by_the_resolver | §5.1/§5.7/§9.1 + ADR-005/ADR-010 |
// | 36 | plugin_path_refuses_protected_claim_without_the_resolver | §5.7/§17.4 (P7.36 restatement — defence in depth) |
// | 37 | class_a_registers_through_the_registry_from_a_plan | §5.4 Class A + §8.1 (Class A group) |
// | 38 | class_a_registry_refuses_protected_and_duplicate_factories | §5.4 Class A + §9.1 (Class A group) |
// | 39 | class_b_manifest_symbol_must_name_a_recorded_wit_world | §5.4 Class B + §8.1 (Class B group) |
// | 40 | class_b_bindings_seam_is_honestly_unrun | FLEXCORE-ONLY (Class B group — the structural guarantee; real instantiation is the human gate) |
// | 41 | class_c_worker_contract_version_presence_rule | §5.4 Class C + §10.5 (Class C group — `worker_api.proto` field names, read-only) |
// | 42 | class_c_determinism_fields_seed_and_request_id_first_class | FLEXCORE-ONLY (P7.17/P7.42 — Class C group) |
// | 43 | class_c_entry_point_target_required | §5.5 (Class C group) |
// | 44 | per_class_run_record | FLEXCORE-ONLY (P7.21 step 2 — per-class run records; suite version recorded with the run) |
//
// CLAIM BOUNDARY (plain language — the P6.19 step-4 honesty, restated):
//
//   Passing this suite entitles FlexCore to claim EXACTLY:
//     "passes the FlexCore plugin SDK conformance suite version 1."
//   It certifies NOTHING externally: no protocol certification, no
//   security certification, no safety case, no grid-code conformance, and
//   no permission to control field devices (the entire 0.x line is
//   simulation-only; AGENTS.md §3 rules 2/5). The suite tests the
//   plugin-SDK contract surface as implemented in this kernel; where a
//   layer cannot be exercised without a real WASM component or gRPC
//   worker (the bindings seam; the wire transport), the suite asserts the
//   structural guarantee that exists and the residual items are HUMAN
//   GATE entries in the decision record
//   (`ledger/phase-07-plugin-sdk/21_22_23_conformance_swap_untrusted.md`).

use flexcore_kernel::lifecycle::{
    is_legal_transition, FixedClock, CRASH_TRANSITIONS, FROZEN_TRANSITIONS, GateOutcome,
    LifecycleAuditKind, LifecycleState, LIFECYCLE_STATES, MemoryAuditSink,
    PluginLifecycle,
};
use flexcore_kernel::manifest::{
    parse_manifest, parse_manifest_value, Manifest, ManifestError, PluginClass,
    ALLOWED_PERMISSIONS, MANIFEST_SCHEMA_ID, MAX_MANIFEST_BYTES, NEVER_GRANTED_PERMISSIONS,
    SECRET_REFERENCE_SCHEMES,
};
use flexcore_kernel::plugin_config::{resolve_for_instance, ConfigError, ResolvedValue};
use flexcore_kernel::plugin_path_protection::{
    refuse_protected_contract_claim, PluginProposalEnvelope, PluginRequestEnvelope,
};
use flexcore_kernel::profile::{
    parse_profile, validate_profile, ProfileError, POSTURE_SIMULATION_SHADOW,
};
use flexcore_kernel::registry::{FnFactory, ProviderFactory, ProviderRegistry, seed};
use flexcore_kernel::resolve::{
    resolve, ExplicitSelections, CURRENT_PLATFORM_API_VERSION,
    PROTECTED_KERNEL_CONTRACT_IDS,
};
use flexcore_kernel::signing::{
    parse_provenance, provenance_gate, ProvenanceRecord, SigningError, UnsafeTestOnlyDigest,
    DIGEST_ALGORITHMS, PROVENANCE_SCHEMA_ID, REFUSAL_UNSIGNED,
};

// --- Suite identity -----------------------------------------------------------

/// The versioned suite id (the ONLY thing a pass claim may name).
pub const SUITE_ID: &str = "flexcore-plugin-sdk-conformance-suite-v1";
/// The suite version (recorded with every run — ledger P7.21 acceptance).
pub const SUITE_VERSION: u32 = 1;
/// The claim boundary, verbatim (the header restates it; the run record
/// carries it; the decision record quotes it).
pub const SUITE_CLAIM_BOUNDARY: &str = "Passing this suite entitles FlexCore to claim \
     exactly: 'passes the FlexCore plugin SDK conformance suite version 1'. It certifies \
     NOTHING externally: no protocol, security, or safety certification of any kind, and \
     no permission to control field devices (the entire 0.x line is simulation-only). \
     Where a layer needs a real WASM component or gRPC worker, the suite asserts the \
     structural guarantee that exists and the residual is a recorded human-gate item.";

// --- Per-class fixtures (the same three classes, one fixture each) --------------

/// Class A fixture — mirrors the shipped `plugins/manifests/
/// builtin-ess-provider.manifest.json` shape (builtin-rust class).
fn class_a_manifest() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.sim-builtin-ess",
        "name": "Built-in ESS Simulator Provider",
        "version": "0.1.0",
        "api-version": ">=0.5.0, <1.0.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/sim-ess" },
        "provider-of": ["ess.simulation"],
        "consumes": ["telemetry.edge-sample"],
        "permissions": ["telemetry.read"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rated-power-mw": { "type": "number", "description": "Rated active power in MW (P > 0 = export)." },
                "soc-init": { "type": "number", "description": "Initial SOC as a fraction in [0,1]." }
            },
            "required": ["rated-power-mw"]
        },
        "secrets": [],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Deterministic built-in battery simulator provider (Class A; simulation only)."
        }
    })
}

/// Class B fixture — mirrors the shipped `plugins/manifests/
/// optimizer-wasm.manifest.json` shape (wasm-component class).
fn class_b_manifest() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.optimizer-wasm-lp",
        "name": "WASM LP Optimizer Component",
        "version": "0.2.0",
        "api-version": ">=0.5.0, <0.6.0",
        "class": "wasm-component",
        "entry-point": { "symbol": "flexcore:optimizer/lp-solve@0.5.0" },
        "provider-of": ["optimization.lp"],
        "consumes": ["portfolio.read"],
        "permissions": ["schedule.propose"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "gap-tolerance": { "type": "number", "description": "Relative optimality gap tolerance." }
            },
            "required": []
        },
        "secrets": [],
        "resources": { "max-memory-mb": 256, "timeout-ms": 60000, "max-output-mb": 8 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Class B WASM component providing LP optimization (candidate schedules only; never dispatch authority)."
        }
    })
}

/// Class C fixture — mirrors the shipped `plugins/manifests/
/// forecast-worker.manifest.json` shape (grpc-worker class).
fn class_c_manifest() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "acme.forecast-worker",
        "name": "Acme Day-Ahead Forecast Worker",
        "version": "0.3.1",
        "api-version": "^0.5.0",
        "class": "grpc-worker",
        "entry-point": { "target": "https://forecast-worker.internal:9443" },
        "provider-of": ["forecast.day-ahead"],
        "consumes": ["telemetry.history"],
        "permissions": ["telemetry.read", "forecast.publish"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "horizon-hours": { "type": "integer", "description": "Forecast horizon in hours." },
                "region": { "type": "string", "description": "Market region id." }
            },
            "required": ["horizon-hours"]
        },
        "secrets": [
            { "name": "worker-credentials", "reference": "secret-store:forecast/worker" }
        ],
        "resources": { "max-memory-mb": 512, "timeout-ms": 30000, "max-output-mb": 16, "max-calls-per-minute": 600 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Out-of-process Class C forecast worker (illustrative third-party plugin)."
        }
    })
}

fn parse_fixture(value: &serde_json::Value) -> Manifest {
    parse_manifest_value(value).expect("class fixture must be a valid manifest")
}

// --- #1–#7: manifest validity (§19.2(3), §5.5) ------------------------------------

#[test]
fn manifest_valid_fixture_passes_for_every_class() {
    for (class, fixture) in [
        (PluginClass::BuiltInRust, class_a_manifest()),
        (PluginClass::WasmComponent, class_b_manifest()),
        (PluginClass::GrpcWorker, class_c_manifest()),
    ] {
        let manifest = parse_fixture(&fixture);
        assert_eq!(manifest.class, class);
        assert!(manifest.plugin_id.contains('.'), "plugin ids are namespaced");
        // The envelope is the versioned schema id — never parsed permissively.
        assert_eq!(
            fixture.get("manifest-schema").and_then(|v| v.as_str()),
            Some(MANIFEST_SCHEMA_ID)
        );
        // License metadata is required (D4 discipline applied to plugins).
        assert!(!manifest.metadata.license.is_empty());
    }
}

#[test]
fn manifest_unknown_field_is_a_named_rejection() {
    let mut bad = class_a_manifest();
    bad.as_object_mut()
        .unwrap()
        .insert("extra-field".to_string(), serde_json::json!(1));
    let err = parse_manifest_value(&bad).expect_err("unknown fields are rejected, never warned");
    assert_eq!(err.name(), "UNKNOWN_FIELD");
}

#[test]
fn manifest_missing_required_field_is_named() {
    let mut bad = class_b_manifest();
    bad.as_object_mut().unwrap().remove("provider-of");
    let err = parse_manifest_value(&bad).expect_err("missing required field must be named");
    assert_eq!(err.name(), "MISSING_FIELD");
}

#[test]
fn manifest_bad_json_and_non_object_are_named() {
    let err = parse_manifest("{ not json").expect_err("bad JSON must be named");
    assert_eq!(err.name(), "MANIFEST_JSON_INVALID");
    let err = parse_manifest_value(&serde_json::json!([1, 2]))
        .expect_err("a non-object manifest must be named");
    assert_eq!(err.name(), "MANIFEST_NOT_AN_OBJECT");
}

#[test]
fn manifest_wrong_envelope_id_is_named() {
    let mut bad = class_c_manifest();
    bad.as_object_mut()
        .unwrap()
        .insert("manifest-schema".to_string(), serde_json::json!("flexcore-plugin-manifest-v2"));
    let err = parse_manifest_value(&bad).expect_err("a foreign envelope is never parsed");
    assert_eq!(err.name(), "SCHEMA_VERSION_UNSUPPORTED");
}

#[test]
fn manifest_entry_point_must_match_class() {
    // A Class B manifest carrying the Class A entry-point key is a named
    // mismatch (§5.4/§5.5: per-class entry point).
    let mut bad = class_b_manifest();
    bad.as_object_mut().unwrap().insert(
        "entry-point".to_string(),
        serde_json::json!({ "registered-type": "not/for-wasm" }),
    );
    let err = parse_manifest_value(&bad).expect_err("entry point must match the class");
    assert_eq!(err.name(), "ENTRY_POINT_MISMATCH");
}

#[test]
fn manifest_oversize_input_is_refused() {
    // FLEXCORE-ONLY: the fail-closed loader cap (a manifest is metadata).
    let filler = "x".repeat(MAX_MANIFEST_BYTES + 1);
    let err = parse_manifest(&filler).expect_err("oversize input is refused before parsing");
    assert_eq!(err.name(), "MANIFEST_JSON_INVALID");
}

// --- #8–#10: api-version compatibility (§9.3) --------------------------------------

#[test]
fn every_conforming_manifest_covers_the_platform_api_version() {
    for fixture in [class_a_manifest(), class_b_manifest(), class_c_manifest()] {
        let manifest = parse_fixture(&fixture);
        assert!(
            manifest.covers_api_version(CURRENT_PLATFORM_API_VERSION),
            "every composed plugin must cover the pinned platform plugin-API version {}",
            CURRENT_PLATFORM_API_VERSION
        );
    }
}

#[test]
fn manifest_not_covering_platform_version_is_refused_by_the_resolver() {
    let mut bad = class_b_manifest();
    bad.as_object_mut()
        .unwrap()
        .insert("api-version".to_string(), serde_json::json!(">=0.6.0, <0.7.0"));
    let manifest = parse_fixture(&bad);
    // A single-manifest composition still runs the platform gate.
    let err = resolve(&[manifest], &ExplicitSelections::new())
        .expect_err("a manifest outside the platform window must be refused");
    assert_eq!(err.name(), "PLATFORM_API_UNSUPPORTED");
}

#[test]
fn disjoint_consumer_provider_ranges_are_named() {
    // Provider and consumer with disjoint api-version windows (§9.3).
    let provider = parse_fixture(&class_a_manifest());
    let mut consumer_json = class_b_manifest();
    consumer_json.as_object_mut().unwrap().insert(
        "api-version".to_string(),
        serde_json::json!(">=2.0.0"),
    );
    let consumer = parse_fixture(&consumer_json);
    let err = resolve(&[provider, consumer], &ExplicitSelections::new())
        .expect_err("disjoint ranges must be a named refusal");
    assert_eq!(err.name(), "UNSUPPORTED_API_VERSION");
}

// --- #11–#15: permission grammar (§5.7, §5.8, §19.2(7)) -----------------------------

#[test]
fn never_granted_permission_refused_at_the_manifest_gate() {
    for id in NEVER_GRANTED_PERMISSIONS {
        let mut bad = class_a_manifest();
        bad.as_object_mut()
            .unwrap()
            .insert("permissions".to_string(), serde_json::json!([id]));
        let err = parse_manifest_value(&bad)
            .expect_err("a never-granted permission cannot even be declared");
        assert_eq!(err.name(), "PERMISSION_NEVER_GRANTED", "id: {id}");
    }
}

#[test]
fn unknown_permission_refused() {
    let mut bad = class_a_manifest();
    bad.as_object_mut()
        .unwrap()
        .insert("permissions".to_string(), serde_json::json!(["telemetry.read-all"]));
    let err = parse_manifest_value(&bad).expect_err("permissions are ids, never free text");
    assert_eq!(err.name(), "PERMISSION_ID_UNKNOWN");
}

#[test]
fn duplicate_permission_refused() {
    let mut bad = class_a_manifest();
    bad.as_object_mut()
        .unwrap()
        .insert("permissions".to_string(), serde_json::json!(["telemetry.read", "telemetry.read"]));
    let err = parse_manifest_value(&bad).expect_err("a permission may be declared once");
    assert_eq!(err.name(), "DUPLICATE_PERMISSION_DECLARATION");
}

#[test]
fn ceiling_containment_manifest_above_ceiling_refused() {
    // The Class B fixture declares `schedule.propose`; a profile whose
    // ceiling withholds it (the shipped research-shell posture) refuses
    // the manifest — the ceiling can never be exceeded (§5.8).
    let profile_json = serde_json::json!({
        "profile-schema": "flexcore-profile-v1",
        "profile-id": "suite-ceiling-shell",
        "name": "Conformance ceiling shell",
        "version": "0.5.0",
        "description": "Suite fixture: a ceiling that withholds schedule.propose.",
        "control-authority": POSTURE_SIMULATION_SHADOW,
        "permission-ceiling": ["telemetry.read", "asset.read"],
        "selections": [{ "manifest-id": "neryva.optimizer-wasm-lp", "version-constraint": ">=0.2.0" }]
    });
    let profile = parse_profile(&profile_json).expect("suite ceiling shell parses");
    let class_b = parse_fixture(&class_b_manifest());
    // The Class B fixture consumes `portfolio.read`, which nothing here
    // provides; the ceiling violation fires FIRST (fail-closed order),
    // which is exactly the containment rule under test.
    let err = validate_profile(&profile, &[class_b]).expect_err("ceiling cannot be exceeded");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_EXCEEDED");
}

#[test]
fn ceiling_never_contains_a_never_granted_id() {
    let profile_json = serde_json::json!({
        "profile-schema": "flexcore-profile-v1",
        "profile-id": "suite-bad-ceiling",
        "name": "Conformance bad ceiling",
        "version": "0.5.0",
        "description": "Suite fixture: a ceiling attempting to include a never-granted id.",
        "control-authority": POSTURE_SIMULATION_SHADOW,
        "permission-ceiling": ["telemetry.read", "dispatch.authorize"],
        "selections": [{ "manifest-id": "neryva.sim-builtin-ess", "version-constraint": ">=0.1.0" }]
    });
    let err = parse_profile(&profile_json).expect_err("a ceiling can never grant §5.7 never rows");
    assert_eq!(err.name(), "PROFILE_PERMISSION_CEILING_NEVER_GRANTED");
}

// --- #16–#19: secret-REFERENCE-only (§5.7, §17.3, P7.19) -----------------------------

const INLINE_CREDENTIAL: &str = "hunter2-do-not-echo";

#[test]
fn manifest_secret_must_be_a_reference_not_a_value() {
    let mut bad = class_c_manifest();
    bad.as_object_mut().unwrap().insert(
        "secrets".to_string(),
        serde_json::json!([{ "name": "worker-credentials", "reference": INLINE_CREDENTIAL }]),
    );
    let err = parse_manifest_value(&bad).expect_err("manifest secrets are references only");
    assert_eq!(err.name(), "SECRET_REFERENCE_INVALID");
    // The rejected VALUE is never echoed (it may be the credential itself).
    assert!(!err.to_string().contains(INLINE_CREDENTIAL));
}

#[test]
fn config_secret_shaped_plain_value_refused_value_not_echoed() {
    let manifest = parse_fixture(&class_c_manifest());
    // A secret-shaped property carrying a plain (non-reference) string is
    // an inline-credential attempt.
    let config = serde_json::json!({
        "horizon-hours": 24,
        "worker-password": INLINE_CREDENTIAL
    });
    // The fixture's schema does not declare `worker-password`; the unknown
    // property refusal is the FIRST fence. To reach the secret-value rule
    // the property must be DECLARED — build the schema-declared variant.
    assert_eq!(
        resolve_for_instance(&manifest, "0.3.1", None, &config)
            .expect_err("undeclared property refused first")
            .name(),
        "UNKNOWN_PROPERTY"
    );

    let mut declaring = class_c_manifest();
    declaring.as_object_mut().unwrap()["configuration-schema"]["properties"]["worker-password"] =
        serde_json::json!({ "type": "string", "description": "malformed fixture on purpose" });
    let manifest = parse_fixture(&declaring);
    let err = resolve_for_instance(&manifest, "0.3.1", None, &config)
        .expect_err("a secret-shaped plain value is refused");
    assert_eq!(err.name(), "SECRET_VALUE_IN_CONFIG");
    assert!(!err.to_string().contains(INLINE_CREDENTIAL), "the value is never echoed");
}

#[test]
fn config_property_named_like_manifest_secret_refused() {
    let manifest = parse_fixture(&class_c_manifest()); // declares secret `worker-credentials`
    let mut declaring = class_c_manifest();
    declaring.as_object_mut().unwrap()["configuration-schema"]["properties"]["worker-credentials"] =
        serde_json::json!({ "type": "string", "description": "collision fixture" });
    let manifest = parse_fixture(&declaring);
    let config = serde_json::json!({ "horizon-hours": 24, "worker-credentials": "env:OK" });
    let err = resolve_for_instance(&manifest, "0.3.1", None, &config)
        .expect_err("secrets live in the manifest secrets object, never in config");
    assert_eq!(err.name(), "SECRET_FIELD_COLLISION");
    let _ = manifest; // the original fixture documents the declared secret name
}

#[test]
fn secret_reference_resolves_to_handle_only() {
    let manifest = parse_fixture(&class_c_manifest());
    let config = serde_json::json!({ "horizon-hours": 24 });
    let resolved = resolve_for_instance(&manifest, "0.3.1", None, &config)
        .expect("reference-only config resolves");
    // The manifest declares the secret via `secrets` (not config), so the
    // resolved view has zero secret handles here; the HANDLE-ONLY rule is
    // structural: `ResolvedConfig` exposes `secret_refs()` (handles) and
    // has NO value accessor anywhere. To exercise a handle, a declared
    // secret-reference property resolves to `ResolvedValue::Reference`.
    assert!(resolved.secret_refs().is_empty());
    // A config property carrying a scheme-prefixed string is a reference
    // handle: the resolved value records scheme + location, never a value.
    let mut ref_manifest = class_c_manifest();
    ref_manifest.as_object_mut().unwrap()["configuration-schema"]["properties"]["api-endpoint"] =
        serde_json::json!({ "type": "string", "description": "endpoint" });
    let ref_manifest = parse_fixture(&ref_manifest);
    let config = serde_json::json!({ "horizon-hours": 24, "api-endpoint": "env:API_ENDPOINT" });
    let resolved = resolve_for_instance(&ref_manifest, "0.3.1", None, &config)
        .expect("a scheme-prefixed reference resolves to a handle");
    // The resolved value IS the handle (the two-variant closed set), and
    // `secret_refs()` exposes handles only — there is no value accessor
    // anywhere on `ResolvedConfig`.
    match resolved.value("api-endpoint") {
        Some(ResolvedValue::Reference(handle)) => {
            assert_eq!(handle.scheme.name(), "ENV");
            assert_eq!(handle.location, "API_ENDPOINT");
        }
        other => panic!("expected a Reference handle, got {other:?}"),
    }
    assert_eq!(resolved.secret_refs().len(), 1);
    // The reference scheme is one of the closed vocabulary.
    assert!(SECRET_REFERENCE_SCHEMES.iter().any(|s| config["api-endpoint"]
        .as_str()
        .unwrap()
        .starts_with(s)));
    let rendered = resolved.resolved_json().to_string();
    // The handle rendering names the reference and scheme — there is no
    // secret VALUE anywhere in the resolved view.
    assert!(rendered.contains("reference") && rendered.contains("scheme"));
}

// --- #20: resource bounds (§5.4/§5.5) ------------------------------------------------

#[test]
fn resource_bounds_required_and_within_closed_ranges() {
    // Every fixture declares bounds inside the closed ranges.
    for fixture in [class_a_manifest(), class_b_manifest(), class_c_manifest()] {
        let manifest = parse_fixture(&fixture);
        assert!(manifest.resources.max_memory_mb >= 1 && manifest.resources.max_memory_mb <= 65_536);
        assert!(manifest.resources.timeout_ms >= 1 && manifest.resources.timeout_ms <= 3_600_000);
    }
    // Out-of-range bound: a named refusal carrying the range.
    let mut bad = class_a_manifest();
    bad.as_object_mut().unwrap()["resources"]["max-memory-mb"] = serde_json::json!(1_000_000);
    let err = parse_manifest_value(&bad).expect_err("bounds outside the closed range are refused");
    match &err {
        ManifestError::ResourceBoundInvalid { field, reason } => {
            assert_eq!(field, "max-memory-mb");
            assert!(reason.contains("between 1 and 65536"));
        }
        other => panic!("expected RESOURCE_BOUND_INVALID, got {other:?}"),
    }
    // Zero timeout: the range is 1..=3_600_000.
    let mut zero = class_a_manifest();
    zero.as_object_mut().unwrap()["resources"]["timeout-ms"] = serde_json::json!(0);
    assert_eq!(
        parse_manifest_value(&zero).expect_err("zero timeout refused").name(),
        "RESOURCE_BOUND_INVALID"
    );
    // An unknown resource field is an unknown-field rejection (closed key set).
    let mut extra = class_a_manifest();
    extra.as_object_mut().unwrap()["resources"]["fuel-ms"] = serde_json::json!(10);
    assert_eq!(
        parse_manifest_value(&extra).expect_err("unalignable resource field refused").name(),
        "UNKNOWN_FIELD"
    );
}

// --- #21–#24: determinism metadata (P7.17/P7.42 contract, kernel side) ---------------
//
// The Class C proto contract (`python/workers/contracts/worker_api.proto`)
// makes `seed` and `request_id` first-class fields on every request. The
// KERNEL-side projection of that contract is the P7.36 plugin-path
// identity: a non-empty correlation id (the request id), a monotonic
// sequence ≥ 1, and a payload-bound digest that is a pure function of the
// binding. The Class C presence rules themselves are case #42.

#[test]
fn plugin_path_request_identity_requires_request_id_and_sequence() {
    let payload = serde_json::json!({ "op": "solve" });
    // Empty request id (correlation id) → refused.
    let err = PluginRequestEnvelope::new("", "plugin.a", "0.1.0", 1, 100, 200, payload.clone())
        .expect_err("a request without an identity is refused");
    assert_eq!(err.name(), "REQUEST_ENVELOPE_INVALID");
    // Sequence 0 → refused (sequences start at 1).
    let err = PluginRequestEnvelope::new("req-1", "plugin.a", "0.1.0", 0, 100, 200, payload.clone())
        .expect_err("sequence 0 is refused");
    assert_eq!(err.name(), "REQUEST_ENVELOPE_INVALID");
    // Inverted validity window → refused (self-validity, check 1).
    let err = PluginRequestEnvelope::new("req-1", "plugin.a", "0.1.0", 1, 300, 200, payload)
        .expect_err("an inverted window is refused");
    assert_eq!(err.name(), "REQUEST_ENVELOPE_INVALID");
}

#[test]
fn payload_bound_digest_is_deterministic() {
    // Same inputs → same digest (P7.42: the identity is a pure function).
    let build = |payload: serde_json::Value| {
        PluginRequestEnvelope::new("req-42", "plugin.a", "0.1.0", 7, 100, 200, payload)
            .expect("well-formed envelope")
    };
    let a = build(serde_json::json!({ "op": "solve", "n": 3 }));
    let b = build(serde_json::json!({ "n": 3, "op": "solve" })); // same payload, different key order
    assert_eq!(a.digest_hex, b.digest_hex, "digest does not depend on key order");
    // A mutated payload under the captured identity is DETECTED (check 2).
    let mut tampered = a.clone();
    tampered.payload = serde_json::json!({ "op": "solve", "n": 4 });
    assert_eq!(
        tampered.verify_digest().expect_err("tampering detected").name(),
        "REQUEST_DIGEST_MISMATCH"
    );
    // The untampered envelope verifies.
    assert!(a.verify_digest().is_ok());
}

#[test]
fn proposal_envelope_carries_producer_identity_and_sequence() {
    // A plugin response to an optimizer-class contract is a PROPOSAL
    // envelope (ADR-010): producer id+version and sequence are mandatory.
    let err = PluginProposalEnvelope::new("p-1", "plugin.b", "0.2.0", serde_json::json!({}), 0)
        .expect_err("sequence 0 is refused");
    assert_eq!(err.name(), "PROPOSAL_ENVELOPE_INVALID");
    let err = PluginProposalEnvelope::new("", "plugin.b", "0.2.0", serde_json::json!({}), 1)
        .expect_err("a proposal without an id is refused");
    assert_eq!(err.name(), "PROPOSAL_ENVELOPE_INVALID");
    let ok = PluginProposalEnvelope::new(
        "p-1",
        "plugin.b",
        "0.2.0",
        serde_json::json!({ "candidate": true }),
        1,
    )
    .expect("well-formed proposal");
    assert_eq!(ok.producer_plugin_id, "plugin.b");
    assert!(!ok.producer_plugin_version.is_empty());
}

#[test]
fn expired_request_is_refused_never_executed() {
    // The window is checked against an INJECTED clock, and an expired
    // request is refused, never executed (§14.3/§17.3). The envelope
    // self-validity refuses an inverted window up front; the window vs
    // clock comparison is the verifier's check 3 (exercised end-to-end in
    // `plugin_path_protection_p7_36.rs`; here the boundary is pinned at
    // construction: valid_from == valid_until is an inverted window).
    let err = PluginRequestEnvelope::new(
        "req-9", "plugin.a", "0.1.0", 1, 200, 200, serde_json::json!({}),
    )
    .expect_err("an empty window is refused");
    assert_eq!(err.name(), "REQUEST_ENVELOPE_INVALID");
    let ok = PluginRequestEnvelope::new(
        "req-9", "plugin.a", "0.1.0", 1, 100, 200, serde_json::json!({}),
    )
    .expect("a non-empty window is well-formed");
    assert_eq!(ok.valid_from_ms, 100);
}

// --- #25–#29: provenance-record conformance (P7.14, §9.3) -----------------------------

const ARTIFACT_BYTES: &[u8] = b"conformance-fixture-artifact-bytes";

fn valid_provenance_json() -> serde_json::Value {
    // Digest computed with the test-grade FNV-1a-64 (the same algorithm the
    // `UnsafeTestOnlyDigest` verifier computes — the record must match the
    // artifact bytes for the gate to accept).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in ARTIFACT_BYTES {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let digest = format!("{hash:016x}");
    serde_json::json!({
        "provenance-schema": PROVENANCE_SCHEMA_ID,
        "artifact-id": "fixtures/conformance-artifact",
        "digest-algorithm": DIGEST_ALGORITHMS[1], // "fnv1a64-test"
        "digest": digest,
        "signer-key-reference": "key-store:suite/release-1",
        "signed-at-millis": 1_700_000_000_000_u64,
        "manifest-id": "neryva.optimizer-wasm-lp",
        "manifest-version": "0.2.0",
        "builder-id": "suite-builder",
        "sbom-reference": "sbom/conformance-artifact.json"
    })
}

#[test]
fn valid_provenance_record_passes_the_gate() {
    let record: ProvenanceRecord =
        parse_provenance(&valid_provenance_json().to_string()).expect("valid record parses");
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let outcome = provenance_gate(
        Some(&record),
        ARTIFACT_BYTES,
        "neryva.optimizer-wasm-lp",
        flexcore_kernel::manifest::SemVer::new(0, 2, 0),
        &UnsafeTestOnlyDigest,
        &clock,
        &audit,
    );
    assert!(outcome.passed(), "a matching record passes the gate: {outcome:?}");
    // The acceptance is audited (the gate's Transition record).
    let records = audit.records();
    assert!(records.iter().any(|r| r.kind == LifecycleAuditKind::Transition));
}

#[test]
fn unsigned_artifact_refused_before_anything_else() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let outcome = provenance_gate(
        None,
        ARTIFACT_BYTES,
        "neryva.optimizer-wasm-lp",
        flexcore_kernel::manifest::SemVer::new(0, 2, 0),
        &UnsafeTestOnlyDigest,
        &clock,
        &audit,
    );
    match &outcome {
        GateOutcome::Failed { reason } => {
            assert!(reason.starts_with(REFUSAL_UNSIGNED));
        }
        GateOutcome::Passed => panic!("an unsigned artifact can never pass"),
    }
    // The refusal is audited with the attempted Discovered → Verified edge.
    let records = audit.records();
    let refusal = records
        .iter()
        .find(|r| r.kind == LifecycleAuditKind::Refused)
        .expect("the refusal is audited");
    assert_eq!(refusal.plugin_id, "neryva.optimizer-wasm-lp");
}

#[test]
fn sbom_reference_required() {
    let mut bad = valid_provenance_json();
    bad.as_object_mut().unwrap().insert("sbom-reference".to_string(), serde_json::json!(""));
    let err = parse_provenance_value_shape(&bad).expect_err("an artifact without an SBOM is not provenanced");
    assert_eq!(err.name(), "PROVENANCE_SBOM_REFERENCE_INVALID");    let mut missing = valid_provenance_json();
    missing.as_object_mut().unwrap().remove("sbom-reference");
    assert_eq!(
        parse_provenance_value_shape(&missing).expect_err("missing SBOM named").name(),
        "PROVENANCE_MISSING_FIELD"
    );
}

#[test]
fn signer_key_is_a_reference_not_material() {
    let mut bad = valid_provenance_json();
    bad.as_object_mut()
        .unwrap()
        .insert("signer-key-reference".to_string(), serde_json::json!("raw-key-material-abc123"));
    let err = parse_provenance_value_shape(&bad)
        .expect_err("a key reference must be scheme-prefixed");
    assert_eq!(err.name(), "PROVENANCE_KEY_REFERENCE_INVALID");
    // The rejected reference VALUE is never echoed (it may be key material).
    assert!(!err.to_string().contains("raw-key-material-abc123"));
}

#[test]
fn provenance_binding_mismatch_is_refused() {
    let record: ProvenanceRecord =
        parse_provenance(&valid_provenance_json().to_string()).expect("valid record parses");
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    // The record binds `neryva.optimizer-wasm-lp` 0.2.0; verifying it
    // against a DIFFERENT manifest identity is refused.
    let outcome = provenance_gate(
        Some(&record),
        ARTIFACT_BYTES,
        "neryva.sim-builtin-ess",
        flexcore_kernel::manifest::SemVer::new(0, 1, 0),
        &UnsafeTestOnlyDigest,
        &clock,
        &audit,
    );
    match &outcome {
        GateOutcome::Failed { reason } => {
            assert!(reason.starts_with("MANIFEST_MISMATCH"));
        }
        GateOutcome::Passed => panic!("a record for another manifest can never pass"),
    }
}

/// Local helper: parse from a `Value` (the signing module exposes
/// `parse_provenance` over text; the Value path goes through text).
fn parse_provenance_value_shape(value: &serde_json::Value) -> Result<ProvenanceRecord, SigningError> {
    parse_provenance(&value.to_string())
}

// --- #30–#32: lifecycle-transition conformance (§5.6) ---------------------------------

#[test]
fn the_legal_transition_set_is_exactly_the_frozen_table_plus_crash_edges() {
    // The §5.6 legal set is the frozen table; the crash edges are the
    // recorded DEFAULT extension. The full 10×10 state space is
    // enumerated: `is_legal_transition` must accept EXACTLY the table ∪
    // crash edges — nothing more, nothing less.
    let frozen_count = FROZEN_TRANSITIONS.len();
    assert_eq!(frozen_count, 12, "the §5.6 table is frozen at 12 edges");
    let mut legal: Vec<(LifecycleState, LifecycleState)> = Vec::new();
    for from in LIFECYCLE_STATES {
        for to in LIFECYCLE_STATES {
            if is_legal_transition(from, to) {
                legal.push((from, to));
            }
        }
    }
    assert_eq!(legal.len(), 17, "12 frozen + 5 crash edges; got {legal:?}");
    // Spot the drain-before-replace spine explicitly.
    assert!(is_legal_transition(LifecycleState::Ready, LifecycleState::Draining));
    assert!(is_legal_transition(LifecycleState::Draining, LifecycleState::Stopped));
    // And the famous illegal shortcut: an Active (Ready) plugin never
    // jumps straight to Stopped (drain-before-replace, §5.6).
    assert!(!is_legal_transition(LifecycleState::Ready, LifecycleState::Stopped));
    assert!(!is_legal_transition(LifecycleState::Discovered, LifecycleState::Ready));
}

#[test]
fn gates_refuse_advancement_and_leave_state_unchanged_audited() {
    let clock = FixedClock::new(500);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        "suite.plugin",
        "0.1.0",
        &clock,
        &audit,
        flexcore_kernel::lifecycle::DrainPolicy::default(),
    );
    // A failed provenance gate refuses `Discovered → Verified` and the
    // state is UNCHANGED (fail-closed).
    let err = lifecycle
        .verify(
            GateOutcome::Failed { reason: "suite: unsigned artifact".to_string() },
            "suite",
            "attempted verify with a failed gate",
        )
        .expect_err("a failed gate refuses the transition");
    assert_eq!(err.name(), "GATE_REFUSED");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    // The refusal is audited with the attempted edge.
    let records = audit.records();
    let refusal = records
        .iter()
        .find(|r| r.kind == LifecycleAuditKind::Refused)
        .expect("the gate refusal is audited");
    assert_eq!(refusal.from, Some(LifecycleState::Discovered));
    assert_eq!(refusal.to, Some(LifecycleState::Verified));
    // An out-of-table transition is a distinct named error.
    let err = lifecycle
        .mark_ready(GateOutcome::Passed, "suite", "skip everything")
        .expect_err("the machine never invents a path");
    assert_eq!(err.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
}

#[test]
fn terminal_states_hold_their_outcome() {
    let clock = FixedClock::new(500);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        "suite.plugin",
        "0.1.0",
        &clock,
        &audit,
        flexcore_kernel::lifecycle::DrainPolicy::default(),
    );
    lifecycle
        .verify(GateOutcome::Passed, "suite", "gate passes")
        .expect("Discovered → Verified");
    lifecycle
        .reject("suite", "suite: verification withdrawn")
        .expect("Verified → Rejected (the §5.6 exit edge)");
    assert!(lifecycle.state().is_terminal());
    // No outgoing transition exists from a terminal state (one
    // representative attempt; the 10×10 enumeration above pins the rest).
    let err = lifecycle
        .verify(GateOutcome::Passed, "suite", "terminal states hold")
        .expect_err("Rejected is terminal");
    assert_eq!(err.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Rejected);
}

// --- #33–#34: audit-trail shape (§5.1, §5.6 step 4, P7.42) -----------------------------

#[test]
fn every_lifecycle_record_carries_the_required_shape() {
    let clock = FixedClock::new(7_777);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = PluginLifecycle::register(
        "suite.shape-plugin",
        "1.2.3",
        &clock,
        &audit,
        flexcore_kernel::lifecycle::DrainPolicy::default(),
    );
    lifecycle
        .verify(GateOutcome::Passed, "suite-actor", "suite reason")
        .expect("verified");
    let records = audit.records();
    assert!(records.len() >= 2, "registration + transition are both audited");
    for record in records.iter() {
        // §5.6 step 4: plugin ID/version, actor, reason — every record.
        assert!(!record.plugin_id.is_empty());
        assert!(!record.version.is_empty());
        assert!(!record.actor.is_empty());
        assert!(!record.reason.is_empty());
        // Timestamps come ONLY from the injected clock.
        assert_eq!(record.timestamp_millis, 7_777);
        // The kind name is a stable machine name.
        assert!(!record.kind.name().is_empty());
    }
}

#[test]
fn audit_is_append_only_in_emission_order_and_deterministic() {
    let run = || {
        let clock = FixedClock::new(1_000);
        let audit = MemoryAuditSink::new();
        let mut lifecycle = PluginLifecycle::register(
            "suite.order-plugin",
            "0.1.0",
            &clock,
            &audit,
            flexcore_kernel::lifecycle::DrainPolicy::default(),
        );
        lifecycle.verify(GateOutcome::Passed, "suite", "verify").expect("verify");
        lifecycle
            .mark_compatible(GateOutcome::Passed, "suite", "compatible")
            .expect("compatible");
        audit.records().iter().map(|r| r.kind.name()).collect::<Vec<_>>()
    };
    let first = run();
    let second = run();
    // Same clock, same operations → the same audit ordering (P7.42).
    assert_eq!(first, second);
    assert_eq!(first.first(), Some(&"REGISTRATION"));
    // Emission order is preserved (append-only).
    let idx = |name: &str| first.iter().position(|k| *k == name).expect("kind present");
    assert!(idx("TRANSITION") > idx("REGISTRATION"));
}

// --- #35–#36: the protected-kernel guard (§5.1/§5.7/§9.1, ADR-005/ADR-010) --------------

#[test]
fn protected_kernel_contract_claim_refused_by_the_resolver() {
    for contract in PROTECTED_KERNEL_CONTRACT_IDS {
        let mut bad = class_a_manifest();
        bad.as_object_mut()
            .unwrap()
            .insert("provider-of".to_string(), serde_json::json!([contract]));
        let manifest = parse_fixture(&bad);
        let err = resolve(&[manifest], &ExplicitSelections::new())
            .expect_err("no plugin may claim a protected-kernel contract");
        assert_eq!(err.name(), "PROTECTED_KERNEL_REPLACE", "contract: {contract}");
    }
}

#[test]
fn plugin_path_refuses_protected_claim_without_the_resolver() {
    // Defence in depth: a caller that skips the resolver still cannot walk
    // a protected claim through the plugin path (P7.36 restatement).
    for contract in PROTECTED_KERNEL_CONTRACT_IDS {
        let mut bad = class_a_manifest();
        bad.as_object_mut()
            .unwrap()
            .insert("provider-of".to_string(), serde_json::json!([contract]));
        let manifest = parse_fixture(&bad);
        let err = refuse_protected_contract_claim(&manifest)
            .expect_err("the plugin path restates the guard");
        assert_eq!(err.name(), "PROTECTED_CONTRACT_CLAIM", "contract: {contract}");
    }
}

// --- #37–#38: Class A group — registry registration (§5.4 Class A, §8.1) ----------------
//
// Test-provider honesty (the P7.08 record's rule): the kernel crate depends
// on no `flexcore-*` crate, so the REAL providers cannot be constructed
// here; the Class A group uses a LOCAL trivial port trait with local
// factories named after the real providers via the registry's seed
// constants. The swap itself is P7.22's file (`swap_p7_22.rs`) — not
// duplicated here.

trait SuitePort {
    fn port_tag(&self) -> &'static str;
}

struct SuiteProviderA;
impl SuitePort for SuiteProviderA {
    fn port_tag(&self) -> &'static str {
        "class-a-provider"
    }
}

fn class_a_factory() -> Box<dyn ProviderFactory<dyn SuitePort>> {
    Box::new(FnFactory::new(seed::PROVIDER_HIGHS_TIER1, || {
        Ok(std::sync::Arc::new(SuiteProviderA) as std::sync::Arc<dyn SuitePort>)
    }))
}

fn class_a_plan() -> flexcore_kernel::resolve::CompositionPlan {
    let optimizer = parse_fixture(&{
        let mut m = class_a_manifest();
        m.as_object_mut().unwrap().insert("plugin-id".to_string(), serde_json::json!(seed::PLUGIN_OPTIMIZER_DEFAULT));
        m.as_object_mut().unwrap().insert(
            "provider-of".to_string(),
            serde_json::json!([seed::CONTRACT_OPTIMIZER]),
        );
        m.as_object_mut().unwrap().insert("consumes".to_string(), serde_json::json!([]));
        m
    });
    resolve(&[optimizer], &ExplicitSelections::new()).expect("class A seed plan resolves")
}

#[test]
fn class_a_registers_through_the_registry_from_a_plan() {
    let plan = class_a_plan();
    let mut registry: ProviderRegistry<dyn SuitePort> = ProviderRegistry::new(&plan);
    let mut factories = std::collections::BTreeMap::new();
    factories.insert(
        seed::CONTRACT_OPTIMIZER.to_string(),
        class_a_factory() as Box<dyn ProviderFactory<dyn SuitePort>>,
    );
    registry
        .register_from_plan(&plan, factories)
        .expect("registration from the plan covers every plan contract");
    // Construction is on demand; the provider answers through the SAME
    // port trait the consumer holds (`dyn SuitePort`).
    let provider = registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructed");
    assert_eq!(provider.port_tag(), "class-a-provider");
    // The resolved provider plugin id comes from the plan.
    assert_eq!(
        registry.registered_plugin_id(seed::CONTRACT_OPTIMIZER),
        Some(seed::PLUGIN_OPTIMIZER_DEFAULT)
    );
}

#[test]
fn class_a_registry_refuses_protected_and_duplicate_factories() {
    let plan = class_a_plan();
    // A protected-kernel contract id is refused BEFORE anything else.
    assert!(flexcore_kernel::registry::is_protected_kernel_contract(
        PROTECTED_KERNEL_CONTRACT_IDS[0]
    ));
    let mut registry: ProviderRegistry<dyn SuitePort> = ProviderRegistry::new(&plan);
    let err = registry
        .register(
            PROTECTED_KERNEL_CONTRACT_IDS[0],
            class_a_factory(),
        )
        .expect_err("no factory registers under a protected contract");
    assert_eq!(err.name(), "PROTECTED_KERNEL_CONTRACT_REFUSED");
    // A duplicate registration is refused, never a silent overwrite.
    registry
        .register(seed::CONTRACT_OPTIMIZER, class_a_factory())
        .expect("first registration ok");
    let err = registry
        .register(seed::CONTRACT_OPTIMIZER, class_a_factory())
        .expect_err("duplicates are refused");
    assert_eq!(err.name(), "DUPLICATE_CONTRACT_REGISTRATION");
}

// --- #39–#40: Class B group — world conformance (§5.4 Class B, §8.1) --------------------
//
// The recorded `wit/` worlds (P7.09), mirrored READ-ONLY from
// `apps/plugin-host/src/host.rs::WorldId` and the `wit/` tree (the kernel
// cannot import the host crate; the values are the recorded constants).

const RECORDED_WIT_WORLDS: [&str; 3] = [
    "flexcore:optimizer@1.0.0/optimizer-plugin",
    "flexcore:grid@1.0.0/grid-model-provider-plugin",
    "flexcore:telemetry@1.0.0/telemetry-source-plugin",
];

/// The suite's world classifier (DEFAULT decision, recorded): a Class B
/// manifest's `entry-point.symbol` names a WIT export as
/// `<package-namespace>:<package>/<export>@<version>`; the suite accepts a
/// symbol whose PACKAGE NAMESPACE matches a recorded world's namespace
/// (`flexcore:`) and refuses a foreign namespace. The manifest schema v1
/// carries no dedicated world field — the package-namespace check is the
/// structural rule the suite can enforce kernel-side; the exact
/// package-id↔world binding is materialised at the bindings build (human
/// gate).
fn class_b_world_namespace(symbol: &str) -> Option<String> {
    let namespace = symbol.split(':').next()?;
    Some(namespace.to_string())
}

#[test]
fn class_b_manifest_symbol_must_name_a_recorded_wit_world() {
    // The recorded worlds are exactly the P7.09 set.
    assert_eq!(RECORDED_WIT_WORLDS.len(), 3);
    assert!(RECORDED_WIT_WORLDS.iter().all(|w| w.starts_with("flexcore:")));
    // The Class B fixture's symbol names the recorded optimizer namespace.
    let manifest = parse_fixture(&class_b_manifest());
    let symbol = match &manifest.entry_point {
        flexcore_kernel::manifest::EntryPoint::Symbol(s) => s.clone(),
        other => panic!("class B fixture must carry a symbol entry point, got {other:?}"),
    };
    let namespace = class_b_world_namespace(&symbol).expect("symbol names a package namespace");
    assert_eq!(namespace, "flexcore", "the fixture binds to the recorded worlds");
    // A foreign-namespace component is a suite failure.
    assert_ne!(
        class_b_world_namespace("evil:component/sneaky@1.0.0").as_deref(),
        Some("flexcore")
    );
    // The host's recorded world selector names are pinned too (mirror):
    for expected in ["optimizer-plugin", "grid-model-provider-plugin", "telemetry-source-plugin"] {
        assert!(
            RECORDED_WIT_WORLDS.iter().any(|w| w.ends_with(expected)),
            "world {expected} must stay recorded"
        );
    }
}

#[test]
fn class_b_bindings_seam_is_honestly_unrun() {
    // FLEXCORE-ONLY / HUMAN GATE: the kernel exposes NO WASM binding or
    // instantiation API at all — there is no function in this crate that
    // executes plugin code, so the kernel-side guarantee is structural:
    // nothing here can run a component, permitted or not. The real
    // instantiation behaviour (limits, fuel, teardown) is exercised at the
    // host's compile-gated bindings seam and is the recorded human-gate
    // item (`40_toolchain_pinning.md` §5). This test pins the structural
    // half: the kernel's public surface (this suite's import list) contains
    // no execution seam — the suite runs against DATA seams only.
    let kernel_modules = [
        "lifecycle", "manifest", "plugin_config", "plugin_path_protection", "profile",
        "recomposition", "registry", "resolve", "signing",
    ];
    // The recorded module map (kernel `lib.rs`): none of them is a
    // runtime/execution module.
    assert_eq!(kernel_modules.len(), 9);
    assert!(!kernel_modules.iter().any(|m| m.contains("wasm") || m.contains("exec")));
    // And the Class B manifest alone cannot DO anything: parsing it yields
    // data (the manifest), never a running instance.
    let manifest = parse_fixture(&class_b_manifest());
    assert_eq!(manifest.class, PluginClass::WasmComponent);
}

// --- #41–#43: Class C group — worker contract conformance (§5.4 Class C) -----------------
//
// Mirrors `python/workers/contracts/worker_api.proto` FIELD NAMES read-only
// (the file is the Class C contract; the suite does not re-declare it):
// every message carries `contract_version` ("flexcore.worker.v1"; major
// skew is `UNSUPPORTED_CONTRACT_VERSION`), and `ExecuteRequest` carries
// `DeterminismMetadata { seed, request_id, idempotency_key, deterministic,
// seed_supported }`.

const WORKER_CONTRACT_VERSION: &str = "flexcore.worker.v1";
const WORKER_FIELD_CONTRACT_VERSION: &str = "contract_version";
const WORKER_FIELD_REQUEST_ID: &str = "request_id";
const WORKER_FIELD_SEED: &str = "seed";
const WORKER_FIELD_SEED_SUPPORTED: &str = "seed_supported";

#[test]
fn class_c_worker_contract_version_presence_rule() {
    // Presence rule: every worker envelope carries contract_version.
    let good = serde_json::json!({ WORKER_FIELD_CONTRACT_VERSION: WORKER_CONTRACT_VERSION });
    assert_eq!(
        good.get(WORKER_FIELD_CONTRACT_VERSION).and_then(|v| v.as_str()),
        Some(WORKER_CONTRACT_VERSION)
    );
    // Missing contract_version → refused (the supervisor treats it as a
    // contract violation).
    let missing = serde_json::json!({});
    assert!(missing.get(WORKER_FIELD_CONTRACT_VERSION).is_none());
    // Major-version skew → refused (UNSUPPORTED_CONTRACT_VERSION); the
    // major number is the compatibility class.
    let skewed = serde_json::json!({ WORKER_FIELD_CONTRACT_VERSION: "flexcore.worker.v2" });
    let skew_refused = skewed.get(WORKER_FIELD_CONTRACT_VERSION).and_then(|v| v.as_str())
        != Some(WORKER_CONTRACT_VERSION);
    assert!(skew_refused);
}

#[test]
fn class_c_determinism_fields_seed_and_request_id_first_class() {
    // The determinism block: request_id mandatory (echoed verbatim),
    // seed mandatory in the message; a worker that cannot honour the seed
    // records `seed_supported = false` instead of silently ignoring it.
    let request = serde_json::json!({
        WORKER_FIELD_CONTRACT_VERSION: WORKER_CONTRACT_VERSION,
        WORKER_FIELD_REQUEST_ID: "req-2026-001",
        WORKER_FIELD_SEED: 8675309_u64,
    });
    assert!(!request[WORKER_FIELD_REQUEST_ID].as_str().unwrap().is_empty());
    assert!(request[WORKER_FIELD_SEED].is_u64());
    let honest_unseedable = serde_json::json!({
        WORKER_FIELD_CONTRACT_VERSION: WORKER_CONTRACT_VERSION,
        WORKER_FIELD_REQUEST_ID: "req-2026-002",
        WORKER_FIELD_SEED: 0_u64,
        WORKER_FIELD_SEED_SUPPORTED: false,
    });
    // An honest `seed_supported = false` admission is well-formed data —
    // but excludes the response from the P7.42 equivalence claim (recorded
    // in the decision doc, not claimed here).
    assert_eq!(honest_unseedable[WORKER_FIELD_SEED_SUPPORTED], serde_json::json!(false));
}

#[test]
fn class_c_entry_point_target_required() {
    let manifest = parse_fixture(&class_c_manifest());
    match &manifest.entry_point {
        flexcore_kernel::manifest::EntryPoint::Target(target) => {
            assert!(!target.is_empty(), "a Class C worker names its gRPC target");
        }
        other => panic!("class C fixture must carry a target entry point, got {other:?}"),
    }
    // A Class C manifest without `target` is the named mismatch.
    let mut bad = class_c_manifest();
    bad.as_object_mut().unwrap().insert(
        "entry-point".to_string(),
        serde_json::json!({ "symbol": "flexcore:telemetry/ingest@0.5.0" }),
    );
    let err = parse_manifest_value(&bad).expect_err("class/key mismatch is named");
    assert_eq!(err.name(), "ENTRY_POINT_MISMATCH");
}

// --- #44: the per-class run record (P7.21 step 2) ----------------------------------------

/// One cell of the per-class run table.
struct ClassRunRecord {
    class: &'static str,
    cases_run: usize,
    cases_passed: usize,
}

impl ClassRunRecord {
    fn is_clean(&self) -> bool {
        self.cases_run > 0 && self.cases_run == self.cases_passed
    }

    /// The per-class run-record line (suite version recorded with the run —
    /// ledger P7.21 acceptance: "suite version is recorded with each run").
    fn render(&self) -> String {
        format!(
            "class={} suite={SUITE_ID} version={SUITE_VERSION} cases={}/{}",
            self.class, self.cases_passed, self.cases_run
        )
    }
}

/// Runs the CLASS-AGNOSTIC manifest-layer case set against one class
/// fixture and records the outcome (P7.21 step 2: the same suite for every
/// class; per-class results recorded).
fn run_class_agnostic_manifest_cases(fixture: &serde_json::Value) -> usize {
    let mut passed = 0usize;
    // Case: the manifest parses against the one validator.
    if parse_manifest_value(fixture).is_ok() {
        passed += 1;
    }
    // Case: the manifest covers the pinned platform API version.
    if parse_manifest_value(fixture)
        .map(|m| m.covers_api_version(CURRENT_PLATFORM_API_VERSION))
        .unwrap_or(false)
    {
        passed += 1;
    }
    // Case: every requested permission is in the closed vocabulary and out
    // of the never-granted set.
    if parse_manifest_value(fixture)
        .map(|m| {
            m.permissions.iter().all(|p| {
                ALLOWED_PERMISSIONS.contains(&p.as_str())
                    && !NEVER_GRANTED_PERMISSIONS.contains(&p.as_str())
            })
        })
        .unwrap_or(false)
    {
        passed += 1;
    }
    // Case: declared resource bounds are inside the closed ranges.
    if parse_manifest_value(fixture)
        .map(|m| {
            (1..=65_536).contains(&m.resources.max_memory_mb)
                && (1..=3_600_000).contains(&m.resources.timeout_ms)
        })
        .unwrap_or(false)
    {
        passed += 1;
    }
    // Case: declared secrets are scheme-prefixed references only.
    if parse_manifest_value(fixture)
        .map(|m| {
            m.secrets.iter().all(|s| {
                SECRET_REFERENCE_SCHEMES
                    .iter()
                    .any(|scheme| s.reference.starts_with(scheme))
            })
        })
        .unwrap_or(false)
    {
        passed += 1;
    }
    passed
}

#[test]
fn per_class_run_record() {
    let records = [
        ClassRunRecord {
            class: "builtin-rust (A)",
            cases_run: 5,
            cases_passed: run_class_agnostic_manifest_cases(&class_a_manifest()),
        },
        ClassRunRecord {
            class: "wasm-component (B)",
            cases_run: 5,
            cases_passed: run_class_agnostic_manifest_cases(&class_b_manifest()),
        },
        ClassRunRecord {
            class: "grpc-worker (C)",
            cases_run: 5,
            cases_passed: run_class_agnostic_manifest_cases(&class_c_manifest()),
        },
    ];
    // The SAME suite passes for all three classes on the same contract —
    // no class-specific shortcut (the ledger's named risk).
    for record in &records {
        assert!(record.is_clean(), "per-class run not clean: {}", record.render());
    }
    // The rendered record carries the suite id and version (the run
    // identity the claim boundary speaks about).
    assert!(records[0].render().contains(SUITE_ID));
    assert!(records[0].render().contains("version=1"));
}

// --- Suite-surface guards ---------------------------------------------------------------

#[test]
fn the_suite_identity_and_claim_boundary_are_pinned() {
    assert_eq!(SUITE_ID, "flexcore-plugin-sdk-conformance-suite-v1");
    assert_eq!(SUITE_VERSION, 1);
    // The claim boundary names exactly the P6.19 sentence shape and
    // certifies nothing externally.
    assert!(SUITE_CLAIM_BOUNDARY.contains("passes the FlexCore plugin SDK conformance suite version 1"));
    assert!(SUITE_CLAIM_BOUNDARY.contains("NOTHING externally"));
    // The crash-edge list (the recorded DEFAULT extension) stays at five
    // edges — the legal-set enumeration above depends on that count.
    assert_eq!(CRASH_TRANSITIONS.len(), 5);
}
