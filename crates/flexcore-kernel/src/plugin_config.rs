// P7.19 — Plugin configuration: schema validation and secret references —
// never secrets in config (report §5.5, §5.6 `Compatible → Configured` gate,
// §5.7 `secret.read` never granted, §17.3; ledger `ledger_phase_7.md`
// P7.19; decision record
// `ledger/phase-07-plugin-sdk/19_20_36_config_health_path.md`).
//
// WHAT THIS MODULE IS: the composition-side configuration resolver for one
// plugin instance. The manifest (P7.05) declares the configuration SCHEMA
// (the enforced JSON subset — `manifest.rs::ConfigurationSchema`); the
// operator supplies config VALUES; this module validates the values against
// the declared schema, applies defaults EXPLICITLY, resolves secret
// REFERENCES to handles, and produces the deterministic resolved view that
// the lifecycle's `Compatible → Configured` gate consumes as its
// `SCHEMA_SECRETS` [`crate::lifecycle::GateOutcome`] evidence.
//
// THE SCHEMA SUBSET AND ITS EXTENSION (documented per the task rule)
//
// BASE (reused, unchanged — `manifest.rs` §2.1 of decision record
// `05_06_manifest_resolution.md`): the manifest's `configuration-schema`
// declares the CLOSED property set (`additionalProperties: false` is
// enforced by the P7.05 validator, so an unknown config property is a
// rejection by construction), per-property types from the closed
// string|number|integer|boolean|array|object vocabulary, and the required
// list. The P7.05 validator accepts ONLY the keys `{type, description}` per
// property — `enum`, `minimum`, `maximum`, and `default` are NOT in the
// manifest subset, and this module does NOT widen the manifest validator
// (the manifest gate is a reviewed baseline; no existing logic was touched).
//
// EXTENSION (the documented P7.19 extension, a DEFAULT decision — D-P7.19-1
// in the decision record): the layered CONSTRAINTS document
// (`flexcore-plugin-config-constraints-v1`, parsed HERE by
// [`ConfigConstraints::parse`], strictly, closed key sets, named errors)
// adds, per property the manifest ALREADY declares and only as a NARROWING:
//
// | Constraint key | Applies to | Meaning |
// |---|---|---|
// | `enum` | `string` properties only | closed value set; a value outside it is [`ConfigError::EnumValueNotAllowed`] |
// | `minimum` / `maximum` | `number` and `integer` properties only | inclusive bounds; outside is [`ConfigError::BoundViolation`] |
// | `default` | any property | applied EXPLICITLY when the operator omits the property — the application is RECORDED in [`ResolvedConfig::defaults_applied`], never silent |
//
// A constraint may only name a property the manifest declares, may only
// apply to properties of the matching type, and can never widen the manifest
// schema (the closed property set is the manifest's; constraints only
// restrict further). This keeps the manifest schema and the config
// validator from drifting (the P7.05 record's explicit requirement) while
// getting the enum/bounds/defaults surface P7.19 needs.
//
// SECRET REFERENCES (the seam, recorded not implemented)
//
// A config string value that matches one of the [`SECRET_REFERENCE_SCHEMES`]
// schemes (`env:` / `file:` / `secret-store:` — the P7.05 closed scheme
// vocabulary) is a secret REFERENCE: resolution returns a
// [`SecretRef`] HANDLE carrying the scheme and the location — the secret
// VALUE is never read here. There is no read path in this module at all:
// the kernel has no secrets-manager port and must never grow one. The
// READING is a host-side capability — the broker's scoped grants (P7.11),
// where `secret.read` sits in the never-granted set for PLUGINS: the host
// resolves the reference into the adapter's environment, the plugin receives
// a handle or nothing (ledger P7.19 step 2; the seam is recorded in the
// decision record §3). Reference EXISTENCE in the store is likewise a host
// check (at `Configured` wiring time on the host side); the kernel-side
// validation here is FORM-only: a scheme prefix with an empty or
// whitespace location is [`ConfigError::SecretRefInvalid`] — rejected at
// configuration time, never at first use (ledger P7.19 step 3's
// fail-early rule, to the depth the kernel can enforce).
//
// SECRET-VALUE REFUSALS (never echo)
//
// A config value that LOOKS like a secret is refused by two closed rules:
//
// 1. **The closed secret-field vocabulary**
//    ([`SECRET_FIELD_VOCABULARY`]): a config property whose name contains a
//    vocabulary token (`password`, `secret`, `token`, `credential`,
//    `api-key`, `private-key`, `access-key`) and carries a plain (non-
//    reference) string value is [`ConfigError::SecretValueInConfig`] — an
//    inline credential attempt. The error carries the PROPERTY NAME only;
//    the VALUE is never echoed (it may be the credential itself — the
//    P7.05 `SECRET_REFERENCE_INVALID` rule, mirrored here).
// 2. **Declared-secret collision**: a config property named exactly like a
//    manifest secret (`manifest.secrets[].name`) is
//    [`ConfigError::SecretFieldCollision`] — secrets are declared in the
//    manifest's `secrets` object, never smuggled through config values.
//
// CONFIG DRIFT (re-instantiation required)
//
// [`resolve_for_instance`] takes the plugin version the INSTANCE was
// instantiated against; a resolved configuration whose manifest version
// differs is [`ConfigError::ConfigDrift`] — configuration is bound to one
// manifest version, and a changed manifest requires a NEW instance through
// drain-before-replace (`lifecycle.rs`), never a silent re-resolution
// against drifted schema.
//
// DETERMINISM (P7.42 depends on it): the resolved view is built over
// `BTreeMap`s only — property iteration is key-sorted regardless of the
// input document's key order, defaults are applied in sorted property
// order, and the recorded outputs ([`ResolvedConfig::resolved_json`],
// `defaults_applied`, `secret_refs`) are sorted structures. The same
// (schema, constraints, values) input always yields an equal resolved view.
//
// No `unwrap`/`expect` on untrusted input anywhere; every JSON access is
// matched; the config source has a fail-closed size cap.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

use crate::manifest::{ConfigurationSchema, Manifest, SECRET_REFERENCE_SCHEMES};

/// The versioned constraints-document envelope id (the documented P7.19
/// extension — see the module header). A constraints document whose
/// `constraint-schema` field is not exactly this value is rejected — never
/// parsed permissively.
pub const CONFIG_CONSTRAINTS_SCHEMA_ID: &str = "flexcore-plugin-config-constraints-v1";

/// Maximum accepted config source size in bytes (fail-closed bound; config
/// is operator metadata, not a payload).
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

/// Maximum accepted constraints-document source size in bytes.
pub const MAX_CONSTRAINTS_BYTES: usize = 64 * 1024;

/// The closed secret-field vocabulary (DEFAULT decision — D-P7.19-2 in the
/// decision record): a config property whose lowercased, separator-
/// normalised name CONTAINS one of these tokens is a secret-shaped field.
/// A plain string value there is an inline-credential attempt, refused.
/// Extending this list is a reviewed change (a closed vocabulary is the
/// point).
pub const SECRET_FIELD_VOCABULARY: [&str; 7] = [
    "password",
    "secret",
    "token",
    "credential",
    "api-key",
    "private-key",
    "access-key",
];

// --- Secret reference handles ---------------------------------------------------

/// The closed secret-reference schemes, as a typed handle vocabulary. The
/// schemes are the P7.05 manifest vocabulary
/// (`manifest.rs::SECRET_REFERENCE_SCHEMES`); the handle records WHICH
/// scheme a resolved reference uses so the host-side reader can select the
/// right backend without re-parsing the raw string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecretRefScheme {
    Env,
    File,
    SecretStore,
}

impl SecretRefScheme {
    /// The scheme prefix (with the colon) this variant stands for.
    pub fn prefix(self) -> &'static str {
        match self {
            SecretRefScheme::Env => "env:",
            SecretRefScheme::File => "file:",
            SecretRefScheme::SecretStore => "secret-store:",
        }
    }

    /// The stable machine name (audit/snapshot value).
    pub fn name(self) -> &'static str {
        match self {
            SecretRefScheme::Env => "ENV",
            SecretRefScheme::File => "FILE",
            SecretRefScheme::SecretStore => "SECRET_STORE",
        }
    }
}

impl fmt::Display for SecretRefScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The secret-REFERENCE handle a resolved config carries INSTEAD of a
/// secret value. There is no value field and no read path anywhere on this
/// type: the structurally only fields are the scheme and the LOCATION the
/// host-side reader will resolve from (the same non-secret location string
/// the manifest's `secrets` object already carries in the clear). The
/// READING is a host-side capability — the broker's scoped grants (P7.11);
/// the plugin receives a handle or nothing (§5.7: `secret.read` is in the
/// never-granted set). This seam is RECORDED, not implemented, in the
/// kernel (the kernel has no secrets-manager port, by design).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecretRef {
    /// The config property the reference was declared under (the local
    /// handle name).
    pub property: String,
    /// The resolved scheme (one of the three closed ones).
    pub scheme: SecretRefScheme,
    /// The reference location (`env:NAME`'s name, `file:PATH`'s path,
    /// `secret-store:KEY`'s key) — a location, never a value.
    pub location: String,
}

impl SecretRef {
    /// The handle's display form (scheme + location — identifiers only,
    /// exactly what the manifest already carries in the clear).
    pub fn describe(&self) -> String {
        format!("{}{}", self.scheme.prefix(), self.location)
    }
}

// --- The layered constraints document (the documented extension) ------------------

/// The constraint applied to one declared property. (No `Eq`: the numeric
/// bounds are `f64`, which has no `Eq` — equality uses `PartialEq`.)
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyConstraint {
    /// A closed value set over a `string` property.
    Enum { allowed: Vec<String> },
    /// Inclusive numeric bounds over a `number`/`integer` property.
    Bounds { minimum: Option<f64>, maximum: Option<f64> },
}

/// The `default` for one declared property, pre-validated to satisfy the
/// property's own type and constraints (a default that violates its own
/// constraints is a constraints-document rejection, never a deferred
/// surprise).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDefault {
    pub value: Value,
}

/// The parsed layered constraints document
/// (`flexcore-plugin-config-constraints-v1`): per-property narrowing
/// constraints and defaults, over properties the manifest ALREADY declares.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigConstraints {
    constraints: BTreeMap<String, PropertyConstraint>,
    defaults: BTreeMap<String, PropertyDefault>,
}

impl ConfigConstraints {
    /// Parses and validates the constraints document against the manifest's
    /// declared schema. Fail-closed: closed key sets at every level, unknown
    /// fields rejected (sorted selection), a constraint on an undeclared
    /// property or of a mismatched type is a named rejection, and every
    /// default is checked against its own property's type AND constraints.
    pub fn parse(
        document: &Value,
        schema: &ConfigurationSchema,
    ) -> Result<ConfigConstraints, ConfigError> {
        let obj = match document {
            Value::Object(map) => map,
            _ => {
                return Err(ConfigError::ConstraintsDocumentInvalid {
                    reason: "the constraints document must be a JSON object".to_string(),
                })
            }
        };
        const ALLOWED_TOP: [&str; 2] = ["constraint-schema", "constraints"];
        reject_unknown_fields(obj, &ALLOWED_TOP, "constraints-document")?;
        let schema_id = match obj.get("constraint-schema") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => {
                return Err(ConfigError::ConstraintsDocumentInvalid {
                    reason: "constraint-schema must be a string".to_string(),
                })
            }
            None => {
                return Err(ConfigError::MissingField {
                    field: "constraint-schema".to_string(),
                    location: "constraints-document".to_string(),
                })
            }
        };
        if schema_id != CONFIG_CONSTRAINTS_SCHEMA_ID {
            return Err(ConfigError::ConstraintsDocumentInvalid {
                reason: format!(
                    "unsupported constraint-schema id '{schema_id}' (expected \
                     '{CONFIG_CONSTRAINTS_SCHEMA_ID}')"
                ),
            });
        }
        let entries = match obj.get("constraints") {
            Some(Value::Object(map)) => map,
            Some(_) => {
                return Err(ConfigError::ConstraintsDocumentInvalid {
                    reason: "constraints must be a JSON object of per-property entries"
                        .to_string(),
                })
            }
            None => {
                return Err(ConfigError::MissingField {
                    field: "constraints".to_string(),
                    location: "constraints-document".to_string(),
                })
            }
        };

        let mut constraints = BTreeMap::new();
        let mut defaults = BTreeMap::new();
        // Sorted property-name iteration: the BTreeMap keys the output; the
        // input document's key order never leaks into a decision or an error
        // selection (P7.42 determinism).
        for (property, entry) in entries {
            // A constraint may only narrow a property the manifest declares.
            let declared = schema.properties.get(property).ok_or_else(|| {
                ConfigError::ConstraintUndeclared { property: property.clone() }
            })?;
            let entry_obj = match entry {
                Value::Object(map) => map,
                _ => {
                    return Err(ConfigError::ConstraintsDocumentInvalid {
                        reason: format!("constraints entry '{property}' must be a JSON object"),
                    })
                }
            };
            const ALLOWED_ENTRY: [&str; 4] = ["enum", "minimum", "maximum", "default"];
            reject_unknown_fields(
                entry_obj,
                &ALLOWED_ENTRY,
                &format!("constraints-document.constraints.{property}"),
            )?;

            // The constraint, if any — with its type-matching rule.
            let mut constraint: Option<PropertyConstraint> = None;
            if entry_obj.contains_key("enum") {
                if declared.ty != crate::manifest::ConfigPropertyType::Str {
                    return Err(ConfigError::ConstraintTypeMismatch {
                        property: property.clone(),
                        reason: format!(
                            "'enum' applies to string properties only; '{}' is declared {}",
                            property,
                            declared.ty.name()
                        ),
                    });
                }
                let allowed = match entry_obj.get("enum") {
                    Some(Value::Array(items)) => {
                        let mut allowed = Vec::with_capacity(items.len());
                        for item in items {
                            match item {
                                Value::String(s) => allowed.push(s.clone()),
                                _ => {
                                    return Err(ConfigError::ConstraintsDocumentInvalid {
                                        reason: format!(
                                            "constraints entry '{property}': 'enum' entries must be strings"
                                        ),
                                    })
                                }
                            }
                        }
                        allowed
                    }
                    _ => {
                        return Err(ConfigError::ConstraintsDocumentInvalid {
                            reason: format!(
                                "constraints entry '{property}': 'enum' must be an array of strings"
                            ),
                        })
                    }
                };
                if allowed.is_empty() {
                    return Err(ConfigError::ConstraintsDocumentInvalid {
                        reason: format!(
                            "constraints entry '{property}': 'enum' must name at least one value"
                        ),
                    });
                }
                constraint = Some(PropertyConstraint::Enum { allowed });
            }
            let mut minimum: Option<f64> = None;
            let mut maximum: Option<f64> = None;
            for (key, slot) in [("minimum", &mut minimum), ("maximum", &mut maximum)] {
                if let Some(v) = entry_obj.get(key) {
                    if !matches!(
                        declared.ty,
                        crate::manifest::ConfigPropertyType::Number
                            | crate::manifest::ConfigPropertyType::Integer
                    ) {
                        return Err(ConfigError::ConstraintTypeMismatch {
                            property: property.clone(),
                            reason: format!(
                                "'{key}' applies to number/integer properties only; '{}' is declared {}",
                                property,
                                declared.ty.name()
                            ),
                        });
                    }
                    let n = match v {
                        Value::Number(n) => n.as_f64(),
                        _ => None,
                    }
                    .ok_or_else(|| ConfigError::ConstraintsDocumentInvalid {
                        reason: format!("constraints entry '{property}': '{key}' must be a number"),
                    })?;
                    if !n.is_finite() {
                        return Err(ConfigError::ConstraintsDocumentInvalid {
                            reason: format!(
                                "constraints entry '{property}': '{key}' must be a finite number"
                            ),
                        });
                    }
                    *slot = Some(n);
                }
            }
            if minimum.is_some() || maximum.is_some() {
                if let (Some(lo), Some(hi)) = (minimum, maximum) {
                    if lo > hi {
                        return Err(ConfigError::ConstraintsDocumentInvalid {
                            reason: format!(
                                "constraints entry '{property}': minimum exceeds maximum"
                            ),
                        });
                    }
                }
                // An enum and bounds on the same property are not composed:
                // one narrowing per property keeps the validation order total
                // and the error surface unambiguous (recorded D-P7.19-1).
                if constraint.is_some() {
                    return Err(ConfigError::ConstraintsDocumentInvalid {
                        reason: format!(
                            "constraints entry '{property}': 'enum' and numeric bounds may not be combined (one narrowing per property)"
                        ),
                    });
                }
                constraint = Some(PropertyConstraint::Bounds { minimum, maximum });
            }

            // The default, if any — validated against the property's type
            // and its own constraint right here (a default that would be
            // refused as a value is refused as a default, at constraints
            // parse time).
            if let Some(default_value) = entry_obj.get("default") {
                check_value_type(property, default_value, declared.ty)?;
                if let Some(c) = &constraint {
                    // A default that violates its own constraint is refused
                    // at constraints-parse time as a DOCUMENT error (never a
                    // deferred per-value surprise).
                    check_constraint(property, default_value, c).map_err(|violation| {
                        ConfigError::ConstraintsDocumentInvalid {
                            reason: format!(
                                "the default for '{property}' violates its own constraints: \
                                 {violation}"
                            ),
                        }
                    })?;
                }
                if is_reference_value(default_value) {
                    return Err(ConfigError::ConstraintsDocumentInvalid {
                        reason: format!(
                            "constraints entry '{property}': a default may not be a secret reference (defaults are plain values applied and recorded, never resolved)"
                        ),
                    });
                }
                defaults.insert(
                    property.clone(),
                    PropertyDefault { value: default_value.clone() },
                );
            }

            if let Some(c) = constraint {
                constraints.insert(property.clone(), c);
            }
        }
        Ok(ConfigConstraints { constraints, defaults })
    }

    /// The constraint for one property, if any.
    pub fn constraint(&self, property: &str) -> Option<&PropertyConstraint> {
        self.constraints.get(property)
    }

    /// The default for one property, if any.
    pub fn default(&self, property: &str) -> Option<&PropertyDefault> {
        self.defaults.get(property)
    }

    /// The sorted property names carrying a default.
    pub fn default_properties(&self) -> Vec<&str> {
        self.defaults.keys().map(|k| k.as_str()).collect()
    }
}

// --- The resolved view -----------------------------------------------------------

/// One resolved property's value. A plain JSON value, or a secret-REFERENCE
/// handle (never a secret value — see [`SecretRef`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedValue {
    Plain(Value),
    Reference(SecretRef),
}

/// The resolved configuration for one plugin instance: every declared
/// property present exactly once (operator value or EXPLICITLY recorded
/// default), typed and constrained, with secret references resolved to
/// handles. The view is a sorted, stable structure (P7.42): `BTreeMap`
/// iteration order everywhere; equality across runs for equal inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConfig {
    plugin_id: String,
    manifest_version: String,
    /// Property → resolved value, keyed in sorted property order.
    values: BTreeMap<String, ResolvedValue>,
    /// The properties whose value came from a default (sorted — BTreeMap
    /// keys). EXPLICIT, never silent: the operator can always see which
    /// values were not supplied by hand.
    defaults_applied: BTreeMap<String, ()>,
}

impl ResolvedConfig {
    /// The plugin id the configuration belongs to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// The manifest version the configuration was resolved against (the
    /// drift anchor).
    pub fn manifest_version(&self) -> &str {
        &self.manifest_version
    }

    /// The sorted property names in the resolved view.
    pub fn properties(&self) -> Vec<&str> {
        self.values.keys().map(|k| k.as_str()).collect()
    }

    /// The resolved value for one property.
    pub fn value(&self, property: &str) -> Option<&ResolvedValue> {
        self.values.get(property)
    }

    /// The secret-reference handles, in sorted property order. HANDLES
    /// ONLY — there is no value accessor anywhere on this type.
    pub fn secret_refs(&self) -> Vec<&SecretRef> {
        self.values
            .values()
            .filter_map(|v| match v {
                ResolvedValue::Reference(r) => Some(r),
                ResolvedValue::Plain(_) => None,
            })
            .collect()
    }

    /// The properties whose value was applied from a default (sorted).
    pub fn defaults_applied(&self) -> Vec<&str> {
        self.defaults_applied.keys().map(|k| k.as_str()).collect()
    }

    /// Whether a property's value came from a default.
    pub fn is_default(&self, property: &str) -> bool {
        self.defaults_applied.contains_key(property)
    }

    /// The deterministic JSON rendering of the resolved view: keys in
    /// sorted order (serde_json object insertion order follows the sorted
    /// `BTreeMap` iteration), reference values rendered as their
    /// `{reference, scheme, location}` handle objects. Two runs over the
    /// same inputs render byte-identical output.
    pub fn resolved_json(&self) -> Value {
        let mut out = serde_json::Map::new();
        // BTreeMap iteration: sorted keys, deterministic rendering.
        for (property, value) in &self.values {
            match value {
                ResolvedValue::Plain(v) => {
                    out.insert(property.clone(), v.clone());
                }
                ResolvedValue::Reference(r) => {
                    let mut handle = serde_json::Map::new();
                    handle.insert(
                        "reference".to_string(),
                        Value::String(r.describe()),
                    );
                    handle.insert(
                        "scheme".to_string(),
                        Value::String(r.scheme.name().to_string()),
                    );
                    handle.insert("location".to_string(), Value::String(r.location.clone()));
                    out.insert(property.clone(), Value::Object(handle));
                }
            }
        }
        Value::Object(out)
    }
}

// --- Errors -----------------------------------------------------------------------

/// Every configuration refusal is a DISTINCT named error
/// ([`ConfigError::name`]) — never a warning, never a silent acceptance.
/// Error text never echoes a secret VALUE or a rejected reference VALUE
/// (it may be the credential itself — the P7.05 rule, mirrored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The config source exceeded [`MAX_CONFIG_BYTES`].
    ConfigTooLarge { size_bytes: usize, limit_bytes: usize },
    /// The config document parsed but is not a JSON object.
    ConfigNotAnObject,
    /// A config property appears that the manifest's schema does not
    /// declare (defence in depth: the manifest's `additionalProperties:
    /// false` already refuses this at the schema level; this module
    /// re-refuses over the RESOLVED surface).
    UnknownProperty { property: String },
    /// A required property was absent and no default exists for it.
    MissingRequired { property: String },
    /// A property was present with the wrong JSON type for its declared
    /// schema type.
    WrongType { property: String, expected: &'static str, found: &'static str },
    /// A string value for an enum-constrained property is outside the
    /// closed value set (the set is described; the rejected value is NOT).
    EnumValueNotAllowed { property: String },
    /// A numeric value is outside the declared inclusive bounds (the
    /// bounds are named; the rejected value is NOT echoed).
    BoundViolation { property: String, bound: &'static str },
    /// A plain string value where the closed secret-field vocabulary or a
    /// declared-secret collision says a secret belongs. The VALUE is never
    /// echoed.
    SecretValueInConfig { property: String },
    /// A config property named exactly like a manifest secret — secrets are
    /// declared in the manifest's `secrets` object, never through config.
    SecretFieldCollision { property: String },
    /// A secret reference failed its FORM check (scheme prefix with an
    /// empty location, or a scheme-prefixed empty string). The location
    /// VALUE is never echoed.
    SecretRefInvalid { property: String, reason: String },
    /// The constraints document violates the v1 subset (closed key sets,
    /// type-matched constraints, self-consistent defaults).
    ConstraintsDocumentInvalid { reason: String },
    /// The constraints document is missing a required field.
    MissingField { field: String, location: String },
    /// A field appeared that the closed key set for its level does not
    /// declare (the P7.05 reject-never-warn choice, mirrored here).
    UnknownField { location: String, field: String },
    /// A constraint names a property the manifest's schema does not
    /// declare — constraints only NARROW the manifest schema.
    ConstraintUndeclared { property: String },
    /// A constraint applies to a property of a type it does not fit.
    ConstraintTypeMismatch { property: String, reason: String },
    /// The constraints source exceeded [`MAX_CONSTRAINTS_BYTES`].
    ConstraintsTooLarge { size_bytes: usize, limit_bytes: usize },
    /// The configuration was resolved against a manifest whose version
    /// differs from the one the plugin instance was instantiated with —
    /// re-instantiation through drain-before-replace is required.
    ConfigDrift { plugin_id: String, expected_version: String, found_version: String },
    /// The plugin id the configuration names is empty.
    PluginIdMissing,
}

impl ConfigError {
    /// The stable machine-readable error name (the named-error table key;
    /// printed in the decision record).
    pub fn name(&self) -> &'static str {
        match self {
            ConfigError::ConfigTooLarge { .. } => "CONFIG_TOO_LARGE",
            ConfigError::ConfigNotAnObject => "CONFIG_NOT_AN_OBJECT",
            ConfigError::UnknownProperty { .. } => "UNKNOWN_PROPERTY",
            ConfigError::MissingRequired { .. } => "MISSING_REQUIRED",
            ConfigError::WrongType { .. } => "CONFIG_WRONG_TYPE",
            ConfigError::EnumValueNotAllowed { .. } => "ENUM_VALUE_NOT_ALLOWED",
            ConfigError::BoundViolation { .. } => "BOUND_VIOLATION",
            ConfigError::SecretValueInConfig { .. } => "SECRET_VALUE_IN_CONFIG",
            ConfigError::SecretFieldCollision { .. } => "SECRET_FIELD_COLLISION",
            ConfigError::SecretRefInvalid { .. } => "SECRET_REF_INVALID",
            ConfigError::ConstraintsDocumentInvalid { .. } => "CONSTRAINTS_DOCUMENT_INVALID",
            ConfigError::MissingField { .. } => "CONSTRAINTS_MISSING_FIELD",
            ConfigError::UnknownField { .. } => "CONSTRAINTS_UNKNOWN_FIELD",
            ConfigError::ConstraintUndeclared { .. } => "CONSTRAINT_UNDECLARED",
            ConfigError::ConstraintTypeMismatch { .. } => "CONSTRAINT_TYPE_MISMATCH",
            ConfigError::ConstraintsTooLarge { .. } => "CONSTRAINTS_TOO_LARGE",
            ConfigError::ConfigDrift { .. } => "CONFIG_DRIFT",
            ConfigError::PluginIdMissing => "PLUGIN_ID_MISSING",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ConfigTooLarge { size_bytes, limit_bytes } => {
                write!(f, "config source is {size_bytes} bytes, above the {limit_bytes}-byte limit")
            }
            ConfigError::ConfigNotAnObject => {
                write!(f, "the configuration must be a JSON object")
            }
            ConfigError::UnknownProperty { property } => {
                write!(f, "config property '{property}' is not declared in the manifest's configuration schema (unknown properties are rejected, not warned)")
            }
            ConfigError::MissingRequired { property } => {
                write!(f, "required config property '{property}' is absent and no default is declared for it")
            }
            ConfigError::WrongType { property, expected, found } => {
                write!(f, "config property '{property}' must be {expected}, found {found}")
            }
            ConfigError::EnumValueNotAllowed { property } => {
                write!(f, "config property '{property}' is outside the closed value set declared for it (the rejected value is not echoed)")
            }
            ConfigError::BoundViolation { property, bound } => {
                write!(f, "config property '{property}' is outside the declared {bound} bound (the rejected value is not echoed)")
            }
            ConfigError::SecretValueInConfig { property } => {
                write!(f, "config property '{property}' carries a plain value in a secret-shaped field; secrets are supplied as env:, file:, or secret-store: references and are never values in config (the value is not echoed)")
            }
            ConfigError::SecretFieldCollision { property } => {
                write!(f, "config property '{property}' is named like a manifest secret; secrets are declared in the manifest's secrets object, never through config values")
            }
            ConfigError::SecretRefInvalid { property, reason } => {
                // The rejected LOCATION is deliberately not echoed: it may
                // be the credential itself.
                write!(f, "config property '{property}' is not a valid secret reference: {reason}")
            }
            ConfigError::ConstraintsDocumentInvalid { reason } => {
                write!(f, "the constraints document violates the v1 subset: {reason}")
            }
            ConfigError::MissingField { field, location } => {
                write!(f, "missing required field '{field}' at {location}")
            }
            ConfigError::UnknownField { location, field } => {
                write!(f, "unknown field '{field}' at {location} (unknown fields are rejected, not warned)")
            }
            ConfigError::ConstraintUndeclared { property } => {
                write!(f, "constraint names property '{property}', which the manifest's configuration schema does not declare (constraints only narrow the declared schema)")
            }
            ConfigError::ConstraintTypeMismatch { property, reason } => {
                write!(f, "constraint on '{property}' does not fit its declared type: {reason}")
            }
            ConfigError::ConstraintsTooLarge { size_bytes, limit_bytes } => {
                write!(
                    f,
                    "constraints document is {size_bytes} bytes, above the {limit_bytes}-byte limit"
                )
            }
            ConfigError::ConfigDrift { plugin_id, expected_version, found_version } => {
                write!(
                    f,
                    "configuration drift: plugin '{plugin_id}' was instantiated against manifest \
                     version {expected_version} but the configuration resolves against \
                     {found_version}; re-instantiation through drain-before-replace is required"
                )
            }
            ConfigError::PluginIdMissing => {
                write!(f, "the configuration resolution was given an empty plugin id")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

// --- Resolution -------------------------------------------------------------------

/// Resolves one plugin instance's configuration (the version-anchored entry
/// point). `instantiated_version` is the manifest version the RUNNING
/// instance was configured against; a manifest whose version differs is
/// [`ConfigError::ConfigDrift`] — re-instantiation required.
///
/// `constraints` is the optional layered constraints document (the
/// documented extension — see the module header); `config` is the
/// operator-supplied values document. Resolution is a pure function: the
/// same inputs always yield an equal [`ResolvedConfig`] (determinism,
/// P7.42).
pub fn resolve_for_instance(
    manifest: &Manifest,
    instantiated_version: &str,
    constraints: Option<&Value>,
    config: &Value,
) -> Result<ResolvedConfig, ConfigError> {
    if manifest.plugin_id.is_empty() {
        return Err(ConfigError::PluginIdMissing);
    }
    if manifest.version.to_string() != instantiated_version {
        return Err(ConfigError::ConfigDrift {
            plugin_id: manifest.plugin_id.clone(),
            expected_version: instantiated_version.to_string(),
            found_version: manifest.version.to_string(),
        });
    }
    let parsed_constraints = match constraints {
        None => ConfigConstraints::default(),
        Some(document) => {
            let text_len = document.to_string().len();
            if text_len > MAX_CONSTRAINTS_BYTES {
                return Err(ConfigError::ConstraintsTooLarge {
                    size_bytes: text_len,
                    limit_bytes: MAX_CONSTRAINTS_BYTES,
                });
            }
            ConfigConstraints::parse(document, &manifest.configuration_schema)?
        }
    };
    resolve_checked(manifest, &parsed_constraints, config)
}

/// Resolves the configuration without the version anchor (the drift-free
/// core, shared by both entry points and by the tests).
fn resolve_checked(
    manifest: &Manifest,
    constraints: &ConfigConstraints,
    config: &Value,
) -> Result<ResolvedConfig, ConfigError> {
    let size = config.to_string().len();
    if size > MAX_CONFIG_BYTES {
        return Err(ConfigError::ConfigTooLarge {
            size_bytes: size,
            limit_bytes: MAX_CONFIG_BYTES,
        });
    }
    let obj = match config {
        Value::Object(map) => map,
        _ => return Err(ConfigError::ConfigNotAnObject),
    };
    let schema = &manifest.configuration_schema;

    // Unknown properties, sorted selection (no dependence on map iteration
    // order; the error names the FIRST unknown property in sorted order).
    let mut unknown: Vec<&String> = obj
        .keys()
        .filter(|key| !schema.properties.contains_key(*key))
        .collect();
    unknown.sort();
    if let Some(property) = unknown.first() {
        return Err(ConfigError::UnknownProperty {
            property: (*property).clone(),
        });
    }

    // Declared-secret collisions: a config property named exactly like a
    // manifest secret is a smuggling attempt (secrets live in the
    // manifest's `secrets` object), refused before any value is examined.
    for secret in &manifest.secrets {
        if obj.contains_key(&secret.name) {
            return Err(ConfigError::SecretFieldCollision {
                property: secret.name.clone(),
            });
        }
    }

    let mut values: BTreeMap<String, ResolvedValue> = BTreeMap::new();
    let mut defaults_applied: BTreeMap<String, ()> = BTreeMap::new();

    // Operator-supplied properties, in SORTED property order (deterministic
    // error selection and resolution order independent of input key order).
    for (property, declared) in &schema.properties {
        let supplied = match obj.get(property) {
            Some(v) => v,
            None => {
                // Absent: apply the EXPLICIT default, or refuse if required.
                if let Some(default) = constraints.default(property) {
                    values.insert(
                        property.clone(),
                        ResolvedValue::Plain(default.value.clone()),
                    );
                    defaults_applied.insert(property.clone(), ());
                    continue;
                }
                if schema.required.iter().any(|r| r == property) {
                    return Err(ConfigError::MissingRequired {
                        property: property.clone(),
                    });
                }
                continue;
            }
        };
        // Type check against the declared schema type.
        check_value_type(property, supplied, declared.ty)?;
        // Secret-shaped refusals (before constraint checks: an inline
        // credential must not survive further processing at all).
        if let Value::String(text) = supplied {
            if is_reference_value(supplied) {
                // A reference: validate the FORM (scheme + non-empty
                // location) and resolve to a HANDLE — no read, no store
                // contact. The handle carries scheme + location only.
                let (scheme, location) = split_reference(text).ok_or_else(|| {
                    ConfigError::SecretRefInvalid {
                        property: property.clone(),
                        reason: "a reference must name a non-empty location after its \
                                 env:/file:/secret-store: scheme prefix"
                            .to_string(),
                    }
                })?;
                values.insert(
                    property.clone(),
                    ResolvedValue::Reference(SecretRef {
                        property: property.clone(),
                        scheme,
                        location: location.to_string(),
                    }),
                );
                continue;
            }
            if matches_secret_vocabulary(property) {
                return Err(ConfigError::SecretValueInConfig {
                    property: property.clone(),
                });
            }
        }
        // Constraint check (enum membership or inclusive bounds).
        if let Some(constraint) = constraints.constraint(property) {
            check_constraint(property, supplied, constraint)?;
        }
        values.insert(property.clone(), ResolvedValue::Plain(supplied.clone()));
    }

    Ok(ResolvedConfig {
        plugin_id: manifest.plugin_id.clone(),
        manifest_version: manifest.version.to_string(),
        values,
        defaults_applied,
    })
}

/// Whether the string is a secret REFERENCE (one of the closed scheme
/// prefixes followed by a non-empty location). The schemes are the P7.05
/// closed vocabulary; any string matching one is resolved to a handle, not
/// kept as a literal (recorded D-P7.19-3 in the decision record).
fn is_reference_value(value: &Value) -> bool {
    match value {
        Value::String(text) => split_reference(text).is_some(),
        _ => false,
    }
}

/// Splits a reference string into its scheme and location. `None` when the
/// string carries no scheme prefix with a non-empty location.
fn split_reference(text: &str) -> Option<(SecretRefScheme, &str)> {
    for scheme in SECRET_REFERENCE_SCHEMES {
        if let Some(rest) = text.strip_prefix(*scheme) {
            let location = rest.trim();
            if !location.is_empty() {
                let typed = match *scheme {
                    "env:" => SecretRefScheme::Env,
                    "file:" => SecretRefScheme::File,
                    "secret-store:" => SecretRefScheme::SecretStore,
                    // Unreachable: the closed scheme array holds exactly
                    // these three prefixes.
                    _ => SecretRefScheme::SecretStore,
                };
                return Some((typed, location));
            }
        }
    }
    None
}

/// Whether a property name matches the closed secret-field vocabulary
/// (normalised: lowercased, `_`/`.`/space folded to `-`, so `api_key`,
/// `APIKey`, and `api-key` all match the `api-key` token).
fn matches_secret_vocabulary(property: &str) -> bool {
    let normalised = property.to_lowercase().replace(['_', '.', ' '], "-");
    SECRET_FIELD_VOCABULARY.iter().any(|token| normalised.contains(token))
}

/// Type check of one value against its declared schema type (the P7.05
/// closed type vocabulary).
fn check_value_type(
    property: &str,
    value: &Value,
    ty: crate::manifest::ConfigPropertyType,
) -> Result<(), ConfigError> {
    use crate::manifest::ConfigPropertyType as T;
    let ok = match ty {
        T::Str => matches!(value, Value::String(_)),
        T::Number => matches!(value, Value::Number(n) if n.as_f64().map(|f| f.is_finite()).unwrap_or(false)),
        T::Integer => matches!(value, Value::Number(n) if n.as_f64().map(|f| f.is_finite() && f.fract() == 0.0).unwrap_or(false)),
        T::Boolean => matches!(value, Value::Bool(_)),
        T::Array => matches!(value, Value::Array(_)),
        T::Object => matches!(value, Value::Object(_)),
    };
    if ok {
        return Ok(());
    }
    Err(ConfigError::WrongType {
        property: property.to_string(),
        expected: ty.name(),
        found: json_type_name(value),
    })
}

/// Constraint check of one value against its parsed constraint.
fn check_constraint(
    property: &str,
    value: &Value,
    constraint: &PropertyConstraint,
) -> Result<(), ConfigError> {
    match constraint {
        PropertyConstraint::Enum { allowed } => {
            let text = match value {
                Value::String(s) => s,
                // The type check already ran; a non-string cannot reach an
                // enum constraint.
                _ => return Ok(()),
            };
            if allowed.iter().any(|a| a == text) {
                Ok(())
            } else {
                Err(ConfigError::EnumValueNotAllowed {
                    property: property.to_string(),
                })
            }
        }
        PropertyConstraint::Bounds { minimum, maximum } => {
            let n = match value {
                Value::Number(n) => n.as_f64(),
                _ => return Ok(()),
            };
            if let Some(lo) = minimum {
                if n.map(|v| v < *lo).unwrap_or(true) {
                    return Err(ConfigError::BoundViolation {
                        property: property.to_string(),
                        bound: "minimum",
                    });
                }
            }
            if let Some(hi) = maximum {
                if n.map(|v| v > *hi).unwrap_or(true) {
                    return Err(ConfigError::BoundViolation {
                        property: property.to_string(),
                        bound: "maximum",
                    });
                }
            }
            Ok(())
        }
    }
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn reject_unknown_fields(
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    location: &str,
) -> Result<(), ConfigError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(ConfigError::UnknownField {
            location: location.to_string(),
            field: field.clone(),
        }),
    }
}
