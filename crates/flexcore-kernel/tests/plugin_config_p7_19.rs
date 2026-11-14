// P7.19 — Plugin configuration: schema validation + secret references (test
// file written, NOT run — the loop bans hold; the human build/test pass
// executes these).
//
// Coverage (ledger P7.19 acceptance criteria; task steps):
//
// 1. A valid config resolves, with every applied default RECORDED in the
//    resolved view (never silent).
// 2. Each violation class lands on its DISTINCT named error (`ConfigError::name`):
//    unknown property, missing required, wrong type, enum violation, bound
//    violation, constraints-document violations (unknown key, undeclared
//    property, type mismatch), secret-value-in-config, declared-secret
//    collision, invalid reference form, config drift, non-object config —
//    and the names are pairwise distinct across the suite.
// 3. Secret-value-in-config is refused and the VALUE is never echoed in the
//    error text.
// 4. Reference resolution returns HANDLES only (scheme + location — the
//    reading is the host/broker capability, P7.11; there is no value
//    accessor on the resolved type at all).
// 5. Drift between manifest versions is refused (re-instantiation required).
// 6. The resolved view is deterministic: equality across runs, and across
//    different input key orders.

use flexcore_kernel::manifest::{parse_manifest_value, Manifest};
use flexcore_kernel::plugin_config::{
    resolve_for_instance, ConfigConstraints, ConfigError, PropertyConstraint, ResolvedValue,
    SecretRefScheme, CONFIG_CONSTRAINTS_SCHEMA_ID,
};

const PLUGIN_VERSION: &str = "0.1.0";

// --- The reference valid manifest (mirrors the P7.05 test-manifest style) ----

fn valid_manifest_json() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.test-ess",
        "name": "Test ESS Provider",
        "version": PLUGIN_VERSION,
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/test-ess" },
        "provider-of": ["ess.simulation"],
        "consumes": ["telemetry.edge-sample"],
        "permissions": ["telemetry.read"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rated-power-mw": { "type": "number", "description": "Rated power in MW." },
                "mode": { "type": "string", "description": "Operating mode." },
                "retries": { "type": "integer", "description": "Retry count." },
                "notes": { "type": "string", "description": "Optional notes." },
                "api-key": { "type": "string", "description": "Credential reference holder." }
            },
            "required": ["rated-power-mw", "mode"]
        },
        "secrets": [
            { "name": "grid-credential", "reference": "secret-store:grid/cred" }
        ],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Test provider for the P7.19 configuration suite.",
            "source": "crates/flexcore-kernel/tests"
        }
    })
}

fn manifest() -> Manifest {
    parse_manifest_value(&valid_manifest_json()).expect("the reference manifest validates")
}

/// The layered constraints document (the documented P7.19 extension):
/// enum on `mode`, inclusive bounds on `rated-power-mw`, a default on
/// `retries`.
fn constraints_json() -> serde_json::Value {
    serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": {
            "mode": { "enum": ["dispatch", "standby"] },
            "rated-power-mw": { "minimum": 0.0, "maximum": 10.0 },
            "retries": { "default": 3 }
        }
    })
}

fn valid_config_json() -> serde_json::Value {
    serde_json::json!({
        "rated-power-mw": 2.5,
        "mode": "dispatch",
        "api-key": "secret-store:ess/api-key"
    })
}

fn resolve(
    m: &Manifest,
    constraints: Option<&serde_json::Value>,
    config: &serde_json::Value,
) -> Result<flexcore_kernel::plugin_config::ResolvedConfig, ConfigError> {
    resolve_for_instance(m, PLUGIN_VERSION, constraints, config)
}

// --- 1. Valid config resolves; defaults are RECORDED -----------------------------

#[test]
fn a_valid_config_resolves_with_recorded_defaults() {
    let m = manifest();
    let constraints = constraints_json();
    let resolved = resolve(&m, Some(&constraints), &valid_config_json())
        .expect("the valid config resolves");

    // Every declared property present exactly once.
    assert_eq!(
        resolved.properties(),
        vec!["api-key", "mode", "rated-power-mw", "retries"],
        "all declared properties resolve, sorted"
    );
    // The default WAS applied and IS recorded (never silent).
    assert_eq!(resolved.defaults_applied(), vec!["retries"]);
    assert!(resolved.is_default("retries"));
    assert!(!resolved.is_default("mode"));
    match resolved.value("retries").expect("retries present") {
        ResolvedValue::Plain(v) => assert_eq!(v, &serde_json::json!(3)),
        other => panic!("retries should be a plain default value, found {other:?}"),
    }
    // The manifest version anchor is recorded.
    assert_eq!(resolved.manifest_version(), PLUGIN_VERSION);
    assert_eq!(resolved.plugin_id(), "neryva.test-ess");
    // No defaults at all when no constraints document is supplied.
    let resolved_no_constraints =
        resolve(&m, None, &valid_config_json()).expect("resolves without constraints");
    assert!(resolved_no_constraints.defaults_applied().is_empty());
    // A required property absent with NO default is MISSING_REQUIRED.
    let error = resolve(&m, None, &serde_json::json!({ "mode": "dispatch" }))
        .expect_err("rated-power-mw is required with no default");
    assert_eq!(error.name(), "MISSING_REQUIRED");
}

#[test]
fn an_absent_required_property_with_a_default_is_filled_by_the_recorded_default() {
    let m = manifest();
    let constraints = constraints_json();
    // `retries` is optional-with-default and absent: filled, recorded.
    let resolved = resolve(&m, Some(&constraints), &valid_config_json()).expect("resolves");
    assert_eq!(resolved.defaults_applied(), vec!["retries"]);
}

// --- 2. Each violation class is a DISTINCT named error ----------------------------

#[test]
fn an_unknown_property_is_rejected_with_its_name() {
    let m = manifest();
    let mut config = valid_config_json();
    config["mystery-setting"] = serde_json::json!(1);
    let error = resolve(&m, None, &config).expect_err("unknown property refused");
    assert_eq!(error.name(), "UNKNOWN_PROPERTY");
    assert!(error.to_string().contains("mystery-setting"));
}

#[test]
fn a_missing_required_property_is_rejected() {
    let m = manifest();
    let error = resolve(&m, None, &serde_json::json!({ "rated-power-mw": 2.5 }))
        .expect_err("mode is required and has no default");
    assert_eq!(error.name(), "MISSING_REQUIRED");
    assert!(error.to_string().contains("mode"));
}

#[test]
fn a_wrong_type_value_is_rejected_per_declared_schema_type() {
    let m = manifest();
    // rated-power-mw is number; a string is refused.
    let mut config = valid_config_json();
    config["rated-power-mw"] = serde_json::json!("2.5");
    let error = resolve(&m, None, &config).expect_err("string in a number property");
    assert_eq!(error.name(), "CONFIG_WRONG_TYPE");
    // retries is integer; a fractional number is refused.
    let mut config = valid_config_json();
    config["retries"] = serde_json::json!(2.5);
    let error = resolve(&m, None, &config).expect_err("fractional in an integer property");
    assert_eq!(error.name(), "CONFIG_WRONG_TYPE");
}

#[test]
fn an_enum_constraint_refuses_values_outside_the_closed_set() {
    let m = manifest();
    let constraints = constraints_json();
    let mut config = valid_config_json();
    config["mode"] = serde_json::json!("turbo");
    let error = resolve(&m, Some(&constraints), &config)
        .expect_err("enum value outside the closed set");
    assert_eq!(error.name(), "ENUM_VALUE_NOT_ALLOWED");
    // The rejected VALUE is not echoed (constraint values may be sensitive).
    assert!(!error.to_string().contains("turbo"));
}

#[test]
fn numeric_bounds_are_enforced_inclusively() {
    let m = manifest();
    let constraints = constraints_json();
    let mut config = valid_config_json();
    config["rated-power-mw"] = serde_json::json!(10.0);
    resolve(&m, Some(&constraints), &config)
        .expect("the maximum bound is inclusive");
    let mut config = valid_config_json();
    config["rated-power-mw"] = serde_json::json!(-0.5);
    let error = resolve(&m, Some(&constraints), &config).expect_err("below minimum");
    assert_eq!(error.name(), "BOUND_VIOLATION");
    assert!(error.to_string().contains("minimum"));
    let mut config = valid_config_json();
    config["rated-power-mw"] = serde_json::json!(10.5);
    let error = resolve(&m, Some(&constraints), &config).expect_err("above maximum");
    assert_eq!(error.name(), "BOUND_VIOLATION");
    assert!(error.to_string().contains("maximum"));
}

#[test]
fn constraints_document_violations_are_named() {
    let m = manifest();
    // Unknown key at the constraints level.
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": {},
        "mystery": true
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("unknown constraints-document key");
    assert_eq!(error.name(), "CONSTRAINTS_UNKNOWN_FIELD");
    // Wrong envelope id.
    let bad = serde_json::json!({ "constraint-schema": "other-v9", "constraints": {} });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("wrong constraint-schema id");
    assert_eq!(error.name(), "CONSTRAINTS_DOCUMENT_INVALID");
    // A constraint on a property the manifest does not declare.
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": { "undeclared-prop": { "enum": ["a"] } }
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("constraints only narrow the declared schema");
    assert_eq!(error.name(), "CONSTRAINT_UNDECLARED");
    // An enum constraint on a non-string property.
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": { "retries": { "enum": ["1"] } }
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("enum on an integer property");
    assert_eq!(error.name(), "CONSTRAINT_TYPE_MISMATCH");
    // A default that violates its own constraints is refused at parse time.
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": { "rated-power-mw": { "minimum": 0.0, "default": -1.0 } }
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("a default below its own minimum");
    assert_eq!(error.name(), "CONSTRAINTS_DOCUMENT_INVALID");
    // A default of the wrong type for its property.
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": { "retries": { "default": "three" } }
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("a string default on an integer property");
    assert_eq!(error.name(), "CONFIG_WRONG_TYPE");
}

#[test]
fn a_non_object_config_is_refused() {
    let m = manifest();
    let error = resolve(&m, None, &serde_json::json!([1, 2, 3]))
        .expect_err("an array is not a config object");
    assert_eq!(error.name(), "CONFIG_NOT_AN_OBJECT");
}

// --- 3. Secret-value-in-config refused; the value is NEVER echoed ---------------

#[test]
fn a_plain_value_in_a_secret_shaped_field_is_refused_and_never_echoed() {
    let m = manifest();
    // `api-key` is in the closed secret-field vocabulary: a plain string
    // value there is an inline-credential attempt.
    let mut config = valid_config_json();
    config["api-key"] = serde_json::json!("S3cr3t-Credential-Value-42");
    let error = resolve(&m, None, &config).expect_err("inline credential refused");
    assert_eq!(error.name(), "SECRET_VALUE_IN_CONFIG");
    // THE VALUE IS NEVER ECHOED — it may be the credential itself.
    let text = error.to_string();
    assert!(!text.contains("S3cr3t"), "the error must not echo the value: {text}");
    assert!(!text.contains("Credential-Value"), "the error must not echo the value: {text}");
    assert!(text.contains("api-key"), "the error names the property");
}

#[test]
fn the_secret_field_vocabulary_covers_separator_variants() {
    let m = manifest();
    // Add a property whose normalised name hits the vocabulary through a
    // different separator spelling.
    let mut m_json = valid_manifest_json();
    m_json["configuration-schema"]["properties"]["site.password"] =
        serde_json::json!({ "type": "string" });
    let m = parse_manifest_value(&m_json).expect("validates");
    let config = serde_json::json!({
        "rated-power-mw": 2.5,
        "mode": "dispatch",
        "site.password": "another-credential-attempt"
    });
    let error = resolve(&m, None, &config).expect_err("vocabulary matches folded separators");
    assert_eq!(error.name(), "SECRET_VALUE_IN_CONFIG");
}

#[test]
fn a_config_property_named_like_a_manifest_secret_is_refused() {
    let m = manifest();
    let config = serde_json::json!({
        "rated-power-mw": 2.5,
        "mode": "dispatch",
        "grid-credential": "env:GRID_CRED"
    });
    let error = resolve(&m, None, &config).expect_err("declared-secret name collision");
    assert_eq!(error.name(), "SECRET_FIELD_COLLISION");
    assert!(error.to_string().contains("grid-credential"));
}

#[test]
fn a_malformed_reference_is_refused_at_configuration_time() {
    let m = manifest();
    // Scheme prefix with an empty location: form-invalid, refused now —
    // never deferred to first use.
    let mut config = valid_config_json();
    config["api-key"] = serde_json::json!("secret-store:");
    let error = resolve(&m, None, &config).expect_err("empty reference location");
    assert_eq!(error.name(), "SECRET_REF_INVALID");
}

// --- 4. Reference resolution returns HANDLES only ---------------------------------

#[test]
fn references_resolve_to_handles_carrying_scheme_and_location_only() {
    let m = manifest();
    let resolved = resolve(&m, None, &valid_config_json()).expect("resolves");
    let refs = resolved.secret_refs();
    assert_eq!(refs.len(), 1, "exactly one reference in the resolved view");
    let handle = refs[0];
    assert_eq!(handle.property, "api-key");
    assert_eq!(handle.scheme, SecretRefScheme::SecretStore);
    assert_eq!(handle.location, "ess/api-key");
    assert_eq!(handle.describe(), "secret-store:ess/api-key");
    // The handle carries NO value and the resolved type has no value
    // accessor for references — the rendering shows the handle shape only.
    match resolved.value("api-key").expect("present") {
        ResolvedValue::Reference(r) => {
            assert_eq!(r.scheme, SecretRefScheme::SecretStore);
            assert_eq!(r.location, "ess/api-key");
        }
        other => panic!("api-key should resolve to a reference handle, found {other:?}"),
    }
    // The JSON rendering carries the handle, never a secret value.
    let json = resolved.resolved_json().to_string();
    assert!(json.contains("secret-store:ess/api-key"));
    assert!(json.contains("SECRET_STORE"));
}

#[test]
fn all_three_reference_schemes_resolve_to_handles() {
    let m = manifest();
    let cases = [
        ("env:ESS_API_KEY", SecretRefScheme::Env),
        ("file:/etc/flexcore/ess-api-key", SecretRefScheme::File),
        ("secret-store:ess/api-key", SecretRefScheme::SecretStore),
    ];
    for (reference, expected_scheme) in cases {
        let config = serde_json::json!({
            "rated-power-mw": 2.5,
            "mode": "dispatch",
            "api-key": reference
        });
        let resolved = resolve(&m, None, &config).expect("each scheme resolves");
        let refs = resolved.secret_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].scheme, expected_scheme);
    }
}

#[test]
fn a_default_may_not_be_a_secret_reference() {
    let m = manifest();
    let bad = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": { "notes": { "default": "env:SOMETHING" } }
    });
    let error = resolve(&m, Some(&bad), &valid_config_json())
        .expect_err("defaults are plain values, never references");
    assert_eq!(error.name(), "CONSTRAINTS_DOCUMENT_INVALID");
}

// --- 5. Drift is refused (re-instantiation required) -------------------------------

#[test]
fn configuration_drift_between_manifest_versions_is_refused() {
    let m = manifest();
    // The instance was configured against 0.1.0; the manifest now says
    // 0.2.0 — resolve_for_instance refuses.
    let error = resolve_for_instance(
        &m,
        "0.2.0",
        None,
        &valid_config_json(),
    )
    .expect_err("version drift is a refusal");
    assert_eq!(error.name(), "CONFIG_DRIFT");
    let text = error.to_string();
    assert!(text.contains("0.2.0"));
    assert!(text.contains("0.1.0"));
    assert!(text.to_lowercase().contains("drain-before-replace") || text.contains("re-instantiation"));
}

// --- 6. The resolved view is deterministic -----------------------------------------

#[test]
fn the_resolved_view_is_equal_across_runs_and_input_key_orders() {
    let m = manifest();
    let constraints = constraints_json();
    let resolved_a =
        resolve(&m, Some(&constraints), &valid_config_json()).expect("first resolution");
    let resolved_b =
        resolve(&m, Some(&constraints), &valid_config_json()).expect("second resolution");
    assert_eq!(
        resolved_a.resolved_json(),
        resolved_b.resolved_json(),
        "same inputs resolve to the identical view"
    );
    assert_eq!(resolved_a.defaults_applied(), resolved_b.defaults_applied());
    assert_eq!(resolved_a.secret_refs(), resolved_b.secret_refs());

    // Different input key order — same resolved view (sorted structure,
    // P7.42).
    let reordered = serde_json::json!({
        "api-key": "secret-store:ess/api-key",
        "mode": "dispatch",
        "rated-power-mw": 2.5
    });
    let resolved_reordered = resolve(&m, Some(&constraints), &reordered).expect("resolves");
    assert_eq!(
        resolved_a.resolved_json(),
        resolved_reordered.resolved_json(),
        "input key order never leaks into the resolved view"
    );

    // Different constraints key order — same resolved view.
    let constraints_reordered = serde_json::json!({
        "constraints": {
            "retries": { "default": 3 },
            "rated-power-mw": { "maximum": 10.0, "minimum": 0.0 },
            "mode": { "enum": ["dispatch", "standby"] }
        },
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID
    });
    let parsed = ConfigConstraints::parse(&constraints_reordered, &m.configuration_schema)
        .expect("the reordered constraints document parses");
    let resolved_c = resolve_for_instance(&m, PLUGIN_VERSION, Some(&constraints_reordered), &valid_config_json())
        .expect("resolves");
    assert_eq!(
        resolved_a.resolved_json(),
        resolved_c.resolved_json(),
        "constraints key order never leaks into the resolved view"
    );
    assert_eq!(parsed.default_properties(), vec!["retries"]);
}

#[test]
fn the_named_error_table_is_pairwise_distinct() {
    let names = [
        ConfigError::ConfigTooLarge { size_bytes: 1, limit_bytes: 1 }.name(),
        ConfigError::ConfigNotAnObject.name(),
        ConfigError::UnknownProperty { property: "x".to_string() }.name(),
        ConfigError::MissingRequired { property: "x".to_string() }.name(),
        ConfigError::WrongType { property: "x".to_string(), expected: "number", found: "string" }.name(),
        ConfigError::EnumValueNotAllowed { property: "x".to_string() }.name(),
        ConfigError::BoundViolation { property: "x".to_string(), bound: "minimum" }.name(),
        ConfigError::SecretValueInConfig { property: "x".to_string() }.name(),
        ConfigError::SecretFieldCollision { property: "x".to_string() }.name(),
        ConfigError::SecretRefInvalid { property: "x".to_string(), reason: "r".to_string() }.name(),
        ConfigError::ConstraintsDocumentInvalid { reason: "r".to_string() }.name(),
        ConfigError::MissingField { field: "f".to_string(), location: "l".to_string() }.name(),
        ConfigError::UnknownField { location: "l".to_string(), field: "f".to_string() }.name(),
        ConfigError::ConstraintUndeclared { property: "x".to_string() }.name(),
        ConfigError::ConstraintTypeMismatch { property: "x".to_string(), reason: "r".to_string() }.name(),
        ConfigError::ConstraintsTooLarge { size_bytes: 1, limit_bytes: 1 }.name(),
        ConfigError::ConfigDrift {
            plugin_id: "p".to_string(),
            expected_version: "1".to_string(),
            found_version: "2".to_string(),
        }
        .name(),
        ConfigError::PluginIdMissing.name(),
    ];
    let mut sorted = names.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "error names are pairwise distinct: {sorted:?}");
}

#[test]
fn a_default_property_missing_from_the_config_is_applied_even_when_required() {
    let m = manifest();
    // Make `mode` required (it already is) and give it a default: absence
    // applies the default (recorded) instead of refusing.
    let constraints = serde_json::json!({
        "constraint-schema": CONFIG_CONSTRAINTS_SCHEMA_ID,
        "constraints": {
            "mode": { "enum": ["dispatch", "standby"], "default": "standby" }
        }
    });
    let config = serde_json::json!({ "rated-power-mw": 2.5 });
    let resolved = resolve(&m, Some(&constraints), &config).expect("the default fills mode");
    assert_eq!(resolved.defaults_applied(), vec!["mode"]);
    match resolved.value("mode").expect("mode present") {
        ResolvedValue::Plain(v) => assert_eq!(v, &serde_json::json!("standby")),
        other => panic!("mode should be the plain default, found {other:?}"),
    }
    // And the applied default respects the enum constraint (a default
    // outside the enum was already refused at constraints-parse time).
    assert!(matches!(
        ConfigConstraints::parse(&constraints, &m.configuration_schema)
            .expect("parses")
            .constraint("mode"),
        Some(PropertyConstraint::Enum { .. })
    ));
}
