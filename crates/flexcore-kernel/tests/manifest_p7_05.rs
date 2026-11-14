// P7.05 — Plugin manifest schema: validation tests (ledger P7.05; test file
// written, NOT run — the loop bans hold; the human build/test pass executes
// these).
//
// Coverage (ledger P7.05 acceptance criteria):
//
// 1. The published example manifests under `plugins/manifests/` all parse.
// 2. Each violation class lands on its DISTINCT named error (unknown field,
//    bad SemVer, secret VALUE instead of reference, permission id outside
//    the vocabulary / in the never-granted set, class/entry-point mismatch,
//    missing api-version, duplicate provider declaration, ...) — and the
//    error names are pairwise distinct across the whole suite.
// 3. Version negotiation: api-version ranges are parsed as RANGES (never
//    string-compared) against the current platform contract version.
// 4. Schema document ↔ implementation consistency spot checks: the published
//    `manifest-schema-v1.md` names the schema id, the fields, and every
//    validator error name this suite pins.

use std::fs;
use std::path::{Path, PathBuf};

use flexcore_kernel::manifest::{
    parse_manifest, parse_manifest_value, ApiVersionRange, EntryPoint, Manifest,
    ManifestError, PluginClass, SemVer, MANIFEST_SCHEMA_ID,
};
use flexcore_kernel::resolve::CURRENT_PLATFORM_API_VERSION;

fn manifests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/manifests")
}

// --- The reference valid Class A manifest (mirrors the published example) ----

fn valid_builtin_json() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.sim-builtin-ess",
        "name": "Built-in ESS Simulator Provider",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/sim-ess" },
        "provider-of": ["ess.simulation"],
        "consumes": ["telemetry.edge-sample"],
        "permissions": ["telemetry.read"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rated-power-mw": { "type": "number", "description": "Rated power in MW." }
            },
            "required": ["rated-power-mw"]
        },
        "secrets": [],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Deterministic built-in ESS simulator provider.",
            "source": "crates/flexcore-simulator"
        }
    })
}

/// Replace or add a top-level field on the valid manifest.
fn with_field(field: &str, value: serde_json::Value) -> serde_json::Value {
    let mut v = valid_builtin_json();
    v[field] = value;
    v
}

/// Remove a top-level field from the valid manifest.
fn without_field(field: &str) -> serde_json::Value {
    let mut v = valid_builtin_json();
    v.as_object_mut().expect("test manifest is an object").remove(field);
    v
}

fn validate(value: &serde_json::Value) -> Result<Manifest, ManifestError> {
    parse_manifest_value(value)
}

/// Assert `value` is rejected with the expected named error (both the
/// variant — via `variant_check` — and the stable machine name).
fn assert_rejected(
    value: &serde_json::Value,
    variant_check: impl Fn(&ManifestError) -> bool,
    expected_name: &str,
) -> ManifestError {
    match validate(value) {
        Ok(_) => panic!("expected rejection ({expected_name}), manifest validated"),
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

// --- 1. Published examples ------------------------------------------------------

#[test]
fn published_example_manifests_parse() {
    for file in [
        "builtin-ess-provider.manifest.json",
        "forecast-worker.manifest.json",
        "optimizer-wasm.manifest.json",
    ] {
        let path = manifests_dir().join(file);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("published example {file} must be readable: {err}"));
        let manifest = parse_manifest(&text)
            .unwrap_or_else(|err| panic!("published example {file} must validate: {err}"));
        assert!(!manifest.plugin_id.is_empty(), "{file} carries a plugin id");
        assert_eq!(manifest.version.to_string().split('.').count(), 3);
    }
}

#[test]
fn published_examples_declare_their_classes_and_entry_points() {
    let builtin = parse_manifest(
        &fs::read_to_string(manifests_dir().join("builtin-ess-provider.manifest.json")).expect("file"),
    )
    .expect("builtin example validates");
    assert_eq!(builtin.class, PluginClass::BuiltInRust);
    assert_eq!(builtin.entry_point, EntryPoint::RegisteredType("neryva.builtin/sim-ess".to_string()));
    assert_eq!(builtin.provider_of, vec!["ess.simulation".to_string()]);

    let worker = parse_manifest(
        &fs::read_to_string(manifests_dir().join("forecast-worker.manifest.json")).expect("file"),
    )
    .expect("worker example validates");
    assert_eq!(worker.class, PluginClass::GrpcWorker);
    assert_eq!(worker.entry_point, EntryPoint::Target("https://forecast-worker.internal:9443".to_string()));
    assert_eq!(worker.secrets.len(), 1);
    assert_eq!(worker.secrets[0].reference, "secret-store:forecast/worker".to_string());

    let wasm = parse_manifest(
        &fs::read_to_string(manifests_dir().join("optimizer-wasm.manifest.json")).expect("file"),
    )
    .expect("wasm example validates");
    assert_eq!(wasm.class, PluginClass::WasmComponent);
    assert_eq!(wasm.entry_point, EntryPoint::Symbol("neryva:optimizer/lp-solve@0.5.0".to_string()));
}

#[test]
fn valid_manifest_carries_declared_fields() {
    let m = validate(&valid_builtin_json()).expect("reference manifest validates");
    assert_eq!(m.plugin_id, "neryva.sim-builtin-ess");
    assert_eq!(m.version, SemVer::new(0, 1, 0));
    assert_eq!(m.class, PluginClass::BuiltInRust);
    assert_eq!(m.consumes, vec!["telemetry.edge-sample".to_string()]);
    assert_eq!(m.permissions, vec!["telemetry.read".to_string()]);
    assert!(m.secrets.is_empty());
    assert_eq!(m.resources.max_memory_mb, 64);
    assert_eq!(m.resources.timeout_ms, 30000);
    assert!(m.resources.max_output_mb.is_none());
    assert_eq!(m.metadata.license, "Apache-2.0");
    assert!(
        matches!(
            m.configuration_schema.properties.get("rated-power-mw").expect("property").ty,
            flexcore_kernel::manifest::ConfigPropertyType::Number
        ),
        "rated-power-mw is declared as a number"
    );
    assert_eq!(m.configuration_schema.required, vec!["rated-power-mw".to_string()]);
}

// --- 2. Violation classes → distinct named errors -------------------------------

#[test]
fn unknown_top_level_field_is_rejected_not_warned() {
    let err = assert_rejected(
        &with_field("extra-field", serde_json::json!(true)),
        |e| matches!(e, ManifestError::UnknownField { location, field } if location == "manifest" && field == "extra-field"),
        "UNKNOWN_FIELD",
    );
    assert!(err.to_string().contains("extra-field"));
}

#[test]
fn unknown_field_reporting_is_sorted_deterministic() {
    // Two unknown fields: the SORTED first one is named (no map-order
    // dependence — determinism discipline).
    let mut v = valid_builtin_json();
    let obj = v.as_object_mut().expect("object");
    obj.insert("zzz-late".to_string(), serde_json::json!(1));
    obj.insert("aaa-early".to_string(), serde_json::json!(2));
    assert_rejected(&v, |e| matches!(e, ManifestError::UnknownField { field } if field == "aaa-early"), "UNKNOWN_FIELD");
}

#[test]
fn unknown_nested_field_is_rejected() {
    let mut v = valid_builtin_json();
    v["resources"]["max-unicorn"] = serde_json::json!(5);
    assert_rejected(&v, |e| matches!(e, ManifestError::UnknownField { location, field } if location == "resources" && field == "max-unicorn"), "UNKNOWN_FIELD");
}

#[test]
fn bad_semver_is_rejected() {
    assert_rejected(
        &with_field("version", serde_json::json!("1.0")),
        |e| matches!(e, ManifestError::InvalidSemVer { field, .. } if field == "version"),
        "INVALID_SEMVER",
    );
    assert_rejected(
        &with_field("version", serde_json::json!("01.2.3")),
        |e| matches!(e, ManifestError::InvalidSemVer { .. }),
        "INVALID_SEMVER",
    );
    assert_rejected(
        &with_field("version", serde_json::json!("1.0.0-beta")),
        |e| matches!(e, ManifestError::InvalidSemVer { .. }),
        "INVALID_SEMVER",
    );
}

#[test]
fn bad_api_version_range_is_rejected() {
    assert_rejected(
        &with_field("api-version", serde_json::json!("0.5 or later")),
        |e| matches!(e, ManifestError::InvalidApiVersionRange { .. }),
        "INVALID_API_VERSION_RANGE",
    );
    assert_rejected(
        &with_field("api-version", serde_json::json!("")),
        |e| matches!(e, ManifestError::InvalidApiVersionRange { .. }),
        "INVALID_API_VERSION_RANGE",
    );
}

#[test]
fn secret_value_instead_of_reference_is_rejected() {
    // A secret object carrying anything beyond {name, reference} is an
    // unknown-field rejection — the manifest is structurally incapable of
    // holding a secret value.
    let mut v = valid_builtin_json();
    v["secrets"] = serde_json::json!([
        { "name": "db-credential", "reference": "env:DB_URL", "secret_value": "hunter2" }
    ]);
    assert_rejected(
        &v,
        |e| matches!(e, ManifestError::UnknownField { location, field } if location == "secrets[0]" && field == "secret_value"),
        "UNKNOWN_FIELD",
    );

    // A top-level field literally named `secret_value` is likewise unknown.
    assert_rejected(
        &with_field("secret_value", serde_json::json!("hunter2")),
        |e| matches!(e, ManifestError::UnknownField { field } if field == "secret_value"),
        "UNKNOWN_FIELD",
    );
}

#[test]
fn secret_reference_without_scheme_is_rejected_and_value_not_echoed() {
    let mut v = valid_builtin_json();
    v["secrets"] = serde_json::json!([
        { "name": "db-password", "reference": "hunter2-the-actual-password" }
    ]);
    let err = assert_rejected(&v, |e| matches!(e, ManifestError::SecretReferenceInvalid { name, .. } if name == "db-password"), "SECRET_REFERENCE_INVALID");
    // Honesty: the rejected reference VALUE may be a real credential — the
    // error must never echo it.
    assert!(!err.to_string().contains("hunter2"), "error leaked the rejected secret value");
}

#[test]
fn permission_id_malformed_or_unknown_is_rejected() {
    assert_rejected(
        &with_field("permissions", serde_json::json!(["telemetry.write"])),
        |e| matches!(e, ManifestError::PermissionIdUnknown { value } if value == "telemetry.write"),
        "PERMISSION_ID_UNKNOWN",
    );
}

#[test]
fn never_granted_permission_is_rejected_at_the_manifest_gate() {
    assert_rejected(
        &with_field("permissions", serde_json::json!(["telemetry.read", "dispatch.authorize"])),
        |e| matches!(e, ManifestError::PermissionNeverGranted { value } if value == "dispatch.authorize"),
        "PERMISSION_NEVER_GRANTED",
    );
    assert_rejected(
        &with_field("permissions", serde_json::json!(["network.any"])),
        |e| matches!(e, ManifestError::PermissionNeverGranted { .. }),
        "PERMISSION_NEVER_GRANTED",
    );
    assert_rejected(
        &with_field("permissions", serde_json::json!(["secret.read"])),
        |e| matches!(e, ManifestError::PermissionNeverGranted { .. }),
        "PERMISSION_NEVER_GRANTED",
    );
    assert_rejected(
        &with_field("permissions", serde_json::json!(["dispatch.issue"])),
        |e| matches!(e, ManifestError::PermissionNeverGranted { .. }),
        "PERMISSION_NEVER_GRANTED",
    );
}

#[test]
fn class_entry_point_mismatch_is_rejected() {
    assert_rejected(
        &with_field("class", serde_json::json!("grpc-worker")),
        |e| matches!(e, ManifestError::EntryPointMismatch { class, expected, found }
            if class == "grpc-worker" && expected == "target" && found == "registered-type"),
        "ENTRY_POINT_MISMATCH",
    );
    assert_rejected(
        &with_field("class", serde_json::json!("dylib")),
        |e| matches!(e, ManifestError::UnknownClass { value } if value == "dylib"),
        "UNKNOWN_CLASS",
    );
    let mut v = valid_builtin_json();
    v["entry-point"] = serde_json::json!({});
    assert_rejected(&v, |e| matches!(e, ManifestError::MissingEntryPoint), "MISSING_ENTRY_POINT");
    let mut v = valid_builtin_json();
    v["entry-point"] = serde_json::json!({ "registered-type": "a", "symbol": "b" });
    assert_rejected(&v, |e| matches!(e, ManifestError::EntryPointAmbiguous { keys } if keys.len() == 2), "ENTRY_POINT_AMBIGUOUS");
}

#[test]
fn missing_required_field_is_rejected() {
    assert_rejected(
        &without_field("api-version"),
        |e| matches!(e, ManifestError::MissingField { field, location } if field == "api-version" && location == "manifest"),
        "MISSING_FIELD",
    );
    for field in ["plugin-id", "name", "version", "class", "entry-point", "provider-of", "consumes", "permissions", "configuration-schema", "resources", "metadata"] {
        let v = without_field(field);
        assert!(
            validate(&v).is_err(),
            "removing required field '{field}' must be rejected"
        );
    }
}

#[test]
fn duplicate_provider_declaration_is_rejected() {
    assert_rejected(
        &with_field("provider-of", serde_json::json!(["ess.simulation", "ess.simulation"])),
        |e| matches!(e, ManifestError::DuplicateProviderDeclaration { contract } if contract == "ess.simulation"),
        "DUPLICATE_PROVIDER_DECLARATION",
    );
    assert_rejected(
        &with_field("consumes", serde_json::json!(["a.b", "a.b"])),
        |e| matches!(e, ManifestError::DuplicateConsumesDeclaration { contract } if contract == "a.b"),
        "DUPLICATE_CONSUMES_DECLARATION",
    );
}

#[test]
fn malformed_ids_are_rejected() {
    assert_rejected(
        &with_field("plugin-id", serde_json::json!("Bad Plugin")),
        |e| matches!(e, ManifestError::PluginIdMalformed { .. }),
        "PLUGIN_ID_MALFORMED",
    );
    assert_rejected(
        &with_field("plugin-id", serde_json::json!("nonnamespaced")),
        |e| matches!(e, ManifestError::PluginIdMalformed { .. }),
        "PLUGIN_ID_MALFORMED",
    );
    assert_rejected(
        &with_field("plugin-id", serde_json::json!("neryva..gap")),
        |e| matches!(e, ManifestError::PluginIdMalformed { .. }),
        "PLUGIN_ID_MALFORMED",
    );
    assert_rejected(
        &with_field("consumes", serde_json::json!(["portfolio"])),
        |e| matches!(e, ManifestError::ContractIdMalformed { field, .. } if field == "consumes"),
        "CONTRACT_ID_MALFORMED",
    );
    assert_rejected(
        &with_field("provider-of", serde_json::json!(["Portfolio.Read"])),
        |e| matches!(e, ManifestError::ContractIdMalformed { .. }),
        "CONTRACT_ID_MALFORMED",
    );
}

#[test]
fn configuration_schema_subset_violations_are_rejected() {
    // additionalProperties must be the literal false.
    let mut v = valid_builtin_json();
    v["configuration-schema"]["additionalProperties"] = serde_json::json!(true);
    assert_rejected(&v, |e| matches!(e, ManifestError::ConfigurationSchemaInvalid { .. }), "CONFIGURATION_SCHEMA_INVALID");
    // Missing additionalProperties entirely.
    let mut v = valid_builtin_json();
    v.as_object_mut().expect("object")["configuration-schema"]
        .as_object_mut()
        .expect("object")
        .remove("additionalProperties");
    assert_rejected(&v, |e| matches!(e, ManifestError::MissingField { field, location } if field == "additionalProperties" && location == "configuration-schema"), "MISSING_FIELD");
    // required naming an undeclared property.
    let mut v = valid_builtin_json();
    v["configuration-schema"]["required"] = serde_json::json!(["not-declared"]);
    assert_rejected(&v, |e| matches!(e, ManifestError::ConfigurationSchemaInvalid { .. }), "CONFIGURATION_SCHEMA_INVALID");
    // Unknown JSON-Schema keyword (not in the enforced subset).
    let mut v = valid_builtin_json();
    v["configuration-schema"]["properties"]["rated-power-mw"]["pattern"] = serde_json::json!("^x$");
    assert_rejected(&v, |e| matches!(e, ManifestError::UnknownField { location, field } if location == "configuration-schema.properties.rated-power-mw" && field == "pattern"), "UNKNOWN_FIELD");
    // Property type outside the closed set.
    let mut v = valid_builtin_json();
    v["configuration-schema"]["properties"]["rated-power-mw"]["type"] = serde_json::json!("float");
    assert_rejected(&v, |e| matches!(e, ManifestError::ConfigurationSchemaInvalid { .. }), "CONFIGURATION_SCHEMA_INVALID");
}

#[test]
fn resource_bound_violations_are_rejected() {
    let mut v = valid_builtin_json();
    v["resources"]["max-memory-mb"] = serde_json::json!(0);
    assert_rejected(&v, |e| matches!(e, ManifestError::ResourceBoundInvalid { field, .. } if field == "max-memory-mb"), "RESOURCE_BOUND_INVALID");
    let mut v = valid_builtin_json();
    v["resources"]["timeout-ms"] = serde_json::json!("soon");
    assert_rejected(&v, |e| matches!(e, ManifestError::WrongType { location, field } if location == "resources" && field == "timeout-ms"), "WRONG_TYPE");
    let mut v = valid_builtin_json();
    v["resources"]["max-output-mb"] = serde_json::json!(0);
    assert_rejected(&v, |e| matches!(e, ManifestError::ResourceBoundInvalid { .. }), "RESOURCE_BOUND_INVALID");
}

#[test]
fn metadata_and_envelope_violations_are_rejected() {
    assert_rejected(
        &without_field("metadata"),
        |e| matches!(e, ManifestError::MissingField { field } if field == "metadata"),
        "MISSING_FIELD",
    );
    let mut v = valid_builtin_json();
    v["metadata"].as_object_mut().expect("object").remove("license");
    assert_rejected(&v, |e| matches!(e, ManifestError::MissingField { field, location } if field == "license" && location == "metadata"), "MISSING_FIELD");
    assert_rejected(
        &with_field("manifest-schema", serde_json::json!("flexcore-plugin-manifest-v2")),
        |e| matches!(e, ManifestError::SchemaVersionUnsupported { .. }),
        "SCHEMA_VERSION_UNSUPPORTED",
    );
    assert_rejected(
        &with_field("manifest-schema", serde_json::json!(1)),
        |e| matches!(e, ManifestError::WrongType { .. }),
        "WRONG_TYPE",
    );
}

#[test]
fn non_json_and_non_object_inputs_are_rejected() {
    assert_rejected(
        &serde_json::json!(["not", "an", "object"]),
        |e| matches!(e, ManifestError::ManifestNotAnObject),
        "MANIFEST_NOT_AN_OBJECT",
    );
    match parse_manifest("this is not json {") {
        Err(ManifestError::ManifestJsonInvalid { .. }) => {}
        other => panic!("expected MANIFEST_JSON_INVALID, got {other:?}"),
    }
    let oversized = "{".repeat(flexcore_kernel::manifest::MAX_MANIFEST_BYTES + 1);
    match parse_manifest(&oversized) {
        Err(ManifestError::ManifestJsonInvalid { .. }) => {}
        other => panic!("expected the size cap to reject oversized input, got {other:?}"),
    }
}

// --- 3. Version negotiation (ranges, never string comparison) --------------------

#[test]
fn api_version_ranges_are_parsed_as_ranges() {
    let r = ApiVersionRange::parse(">=0.5,<1.0").expect("parses");
    assert!(r.satisfied_by(SemVer::new(0, 5, 0)));
    assert!(r.satisfied_by(SemVer::new(0, 9, 9)));
    assert!(!r.satisfied_by(SemVer::new(1, 0, 0)));
    assert!(!r.satisfied_by(SemVer::new(0, 4, 99)));

    let caret = ApiVersionRange::parse("^0.5.0").expect("parses");
    assert!(caret.satisfied_by(SemVer::new(0, 5, 3)));
    assert!(!caret.satisfied_by(SemVer::new(0, 6, 0)));
    assert!(!caret.satisfied_by(SemVer::new(0, 4, 9)));

    let tilde = ApiVersionRange::parse("~0.5.2").expect("parses");
    assert!(tilde.satisfied_by(SemVer::new(0, 5, 9)));
    assert!(!tilde.satisfied_by(SemVer::new(0, 6, 0)));

    let exact = ApiVersionRange::parse("=0.5.0").expect("parses");
    assert!(exact.satisfied_by(SemVer::new(0, 5, 0)));
    assert!(!exact.satisfied_by(SemVer::new(0, 5, 1)));

    // Boundary inclusivity: "<=0.5.0" includes its own upper bound.
    let lte = ApiVersionRange::parse("<=0.5.0").expect("parses");
    assert!(lte.satisfied_by(SemVer::new(0, 5, 0)));
    assert!(!lte.satisfied_by(SemVer::new(0, 5, 1)));
}

#[test]
fn manifest_version_negotiation_against_platform_api_version() {
    let covering = validate(&valid_builtin_json()).expect("validates");
    assert!(covering.covers_api_version(CURRENT_PLATFORM_API_VERSION));

    let mut beyond = valid_builtin_json();
    beyond["api-version"] = serde_json::json!(">=1.0");
    let m = validate(&beyond).expect("validates structurally");
    assert!(!m.covers_api_version(CURRENT_PLATFORM_API_VERSION));
}

#[test]
fn range_intersection_rule_for_composed_pairs() {
    let a = ApiVersionRange::parse("^0.5.0").expect("parses");
    let touching = ApiVersionRange::parse(">=0.6,<1.0").expect("parses");
    // [0.5.0, 0.6.0) vs [0.6.0, 1.0.0): touching at an exclusive/inclusive
    // boundary is NOT an intersection.
    assert!(!a.intersects(&touching));
    let overlapping = ApiVersionRange::parse(">=0.5.2,<0.5.9").expect("parses");
    assert!(a.intersects(&overlapping));
    let unbounded = ApiVersionRange::parse(">=0.4.0").expect("parses");
    assert!(a.intersects(&unbounded));
}

// --- 4. Schema document ↔ implementation consistency spot checks -----------------

#[test]
fn published_schema_document_names_the_implementation_surface() {
    let doc = fs::read_to_string(manifests_dir().join("manifest-schema-v1.md"))
        .expect("the published schema document is readable");
    // The schema id matches the validator constant.
    assert!(doc.contains(MANIFEST_SCHEMA_ID));
    // The documented field names match the enforced key set.
    for field in [
        "manifest-schema", "plugin-id", "name", "version", "api-version", "class",
        "entry-point", "provider-of", "consumes", "permissions", "configuration-schema",
        "secrets", "resources", "metadata",
    ] {
        assert!(doc.contains(field), "schema doc must document field '{field}'");
    }
    // The per-class entry-point keys are documented.
    for key in ["registered-type", "symbol", "target"] {
        assert!(doc.contains(key), "schema doc must document entry-point key '{key}'");
    }
    // Every validator error name the tests pin is in the named-error table.
    for name in ALL_VALIDATOR_ERROR_NAMES {
        assert!(doc.contains(name), "schema doc must list error '{name}'");
    }
    // The never-granted permission set is documented.
    for permission in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
        assert!(doc.contains(permission));
    }
    // The deliberate differences from §5.5 are recorded (P7.05 step 4).
    assert!(doc.contains("Deliberate differences"));
    assert!(doc.contains("YAML"));
}

/// The complete validator error-name set (must equal [`ManifestError::name`]
/// outputs; consistency-checked against the published document above).
const ALL_VALIDATOR_ERROR_NAMES: [&str; 24] = [
    "MANIFEST_JSON_INVALID",
    "MANIFEST_NOT_AN_OBJECT",
    "SCHEMA_VERSION_UNSUPPORTED",
    "UNKNOWN_FIELD",
    "MISSING_FIELD",
    "WRONG_TYPE",
    "PLUGIN_ID_MALFORMED",
    "INVALID_SEMVER",
    "INVALID_API_VERSION_RANGE",
    "UNKNOWN_CLASS",
    "MISSING_ENTRY_POINT",
    "ENTRY_POINT_AMBIGUOUS",
    "ENTRY_POINT_MISMATCH",
    "ENTRY_POINT_INVALID",
    "CONTRACT_ID_MALFORMED",
    "DUPLICATE_PROVIDER_DECLARATION",
    "DUPLICATE_CONSUMES_DECLARATION",
    "PERMISSION_ID_UNKNOWN",
    "PERMISSION_NEVER_GRANTED",
    "DUPLICATE_PERMISSION_DECLARATION",
    "CONFIGURATION_SCHEMA_INVALID",
    "SECRET_REFERENCE_INVALID",
    "DUPLICATE_SECRET_NAME",
    "RESOURCE_BOUND_INVALID",
];

#[test]
fn validator_error_name_table_is_complete_and_consistent() {
    // Build the name set from the actual error type via representative
    // instances and compare against the const table.
    let instances: Vec<ManifestError> = vec![
        ManifestError::ManifestJsonInvalid { reason: "x".to_string() },
        ManifestError::ManifestNotAnObject,
        ManifestError::SchemaVersionUnsupported { found: "x".to_string() },
        ManifestError::UnknownField { location: "l".to_string(), field: "f".to_string() },
        ManifestError::MissingField { field: "f".to_string(), location: "l".to_string() },
        ManifestError::WrongType { location: "l".to_string(), field: "f".to_string(), expected: "e", found: "f" },
        ManifestError::PluginIdMalformed { value: "v".to_string(), reason: "r".to_string() },
        ManifestError::InvalidSemVer { field: "f".to_string(), value: "v".to_string(), reason: "r".to_string() },
        ManifestError::InvalidApiVersionRange { value: "v".to_string(), reason: "r".to_string() },
        ManifestError::UnknownClass { value: "v".to_string() },
        ManifestError::MissingEntryPoint,
        ManifestError::EntryPointAmbiguous { keys: vec!["a".to_string()] },
        ManifestError::EntryPointMismatch { class: "c".to_string(), expected: "e".to_string(), found: "f".to_string() },
        ManifestError::EntryPointInvalid { reason: "r".to_string() },
        ManifestError::ContractIdMalformed { field: "f".to_string(), value: "v".to_string(), reason: "r".to_string() },
        ManifestError::DuplicateProviderDeclaration { contract: "c".to_string() },
        ManifestError::DuplicateConsumesDeclaration { contract: "c".to_string() },
        ManifestError::PermissionIdUnknown { value: "v".to_string() },
        ManifestError::PermissionNeverGranted { value: "v".to_string() },
        ManifestError::DuplicatePermissionDeclaration { value: "v".to_string() },
        ManifestError::ConfigurationSchemaInvalid { reason: "r".to_string() },
        ManifestError::SecretReferenceInvalid { name: "n".to_string(), reason: "r".to_string() },
        ManifestError::DuplicateSecretName { name: "n".to_string() },
        ManifestError::ResourceBoundInvalid { field: "f".to_string(), reason: "r".to_string() },
    ];
    let mut observed: Vec<&str> = instances.iter().map(|e| e.name()).collect();
    observed.sort();
    let mut expected: Vec<&str> = ALL_VALIDATOR_ERROR_NAMES.to_vec();
    expected.sort();
    assert_eq!(observed, expected, "the error-name table must cover every variant exactly once");
    // Names are pairwise distinct.
    for pair in observed.windows(2) {
        assert_ne!(pair[0], pair[1], "error names must be pairwise distinct");
    }
}

// --- 5. The whole violation suite lands on pairwise-distinct names ---------------

#[test]
fn violation_classes_land_on_pairwise_distinct_named_errors() {
    // One representative violation per error family (the entries the
    // validator can distinguish structurally).
    let mut violating = valid_builtin_json();
    violating["resources"]["max-memory-mb"] = serde_json::json!(0);

    let mut dup_perm = valid_builtin_json();
    dup_perm["permissions"] = serde_json::json!(["telemetry.read", "telemetry.read"]);

    let mut dup_secret = valid_builtin_json();
    dup_secret["secrets"] = serde_json::json!([
        { "name": "a", "reference": "env:A" },
        { "name": "a", "reference": "env:B" }
    ]);

    let mut bad_entry = valid_builtin_json();
    bad_entry["entry-point"] = serde_json::json!({ "registered-type": "" });

    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("invalid text", serde_json::json!(null)), // parsed as not-an-object below
        ("unknown field", with_field("extra-field", serde_json::json!(1))),
        ("missing field", without_field("api-version")),
        ("bad semver", with_field("version", serde_json::json!("1.0"))),
        ("bad range", with_field("api-version", serde_json::json!("nope"))),
        ("bad plugin id", with_field("plugin-id", serde_json::json!("Nope"))),
        ("unknown class", with_field("class", serde_json::json!("dylib"))),
        ("entry mismatch", with_field("class", serde_json::json!("grpc-worker"))),
        ("bad contract id", with_field("provider-of", serde_json::json!(["one-segment"]))),
        ("duplicate provider", with_field("provider-of", serde_json::json!(["a.b", "a.b"]))),
        ("duplicate consumes", with_field("consumes", serde_json::json!(["a.b", "a.b"]))),
        ("unknown permission", with_field("permissions", serde_json::json!(["made.up"]))),
        ("never-granted permission", with_field("permissions", serde_json::json!(["dispatch.authorize"]))),
        ("duplicate permission", dup_perm),
        ("config schema invalid", {
            let mut v = valid_builtin_json();
            v["configuration-schema"]["additionalProperties"] = serde_json::json!(true);
            v
        }),
        ("secret value", {
            let mut v = valid_builtin_json();
            v["secrets"] = serde_json::json!([{ "name": "a", "reference": "env:A", "value": "x" }]);
            v
        }),
        ("duplicate secret", dup_secret),
        ("resource bound", violating),
        ("wrong type", with_field("name", serde_json::json!(42))),
        ("entry invalid", bad_entry),
    ];

    let mut observed: Vec<(String, &'static str)> = Vec::new();
    for (label, value) in &cases {
        match validate(value) {
            Err(err) => observed.push((label.to_string(), err.name())),
            Ok(_) => panic!("case '{label}' unexpectedly validated"),
        }
    }
    // The text case separately (it is not valid JSON input at all).
    match parse_manifest("not json") {
        Err(err) => observed.push(("invalid text".to_string(), err.name())),
        Ok(_) => panic!("invalid text unexpectedly parsed"),
    }

    // Pairwise distinct names across ALL cases:
    let mut names: Vec<&str> = observed.iter().map(|(_, n)| *n).collect();
    names.sort();
    for w in names.windows(2) {
        assert_ne!(w[0], w[1], "two violation classes landed on the same error name: {observed:?}");
    }
    // The null case: parse_manifest_value(json!(null)) is MANIFEST_NOT_AN_OBJECT.
    assert!(names.contains(&"MANIFEST_NOT_AN_OBJECT"));
}
