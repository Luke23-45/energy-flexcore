// P7.05 — Plugin manifest schema `flexcore-plugin-manifest-v1` (report §5.5,
// §9.3, §10.5; ledger `ledger_phase_7.md` P7.05).
//
// The §5.5 manifest example is ILLUSTRATIVE, not a final schema (§5.5 says so
// verbatim). This module is the validated, versioned schema it points at;
// every deliberate difference from the example is listed in the published
// schema document (`plugins/manifests/manifest-schema-v1.md`) and in the
// decision record (`ledger/phase-07-plugin-sdk/05_06_manifest_resolution.md`).
//
// Validation stance (recorded choice, P7.05 step 3): **reject, do not warn**.
// Unknown fields at ANY level (manifest, entry-point, resources, metadata,
// configuration schema, secret objects) are a named rejection carrying the
// field name — the rejection record IS the log; no log dependency is added.
// Every rule violation has a DISTINCT named error ([`ManifestError::name`]),
// and the validator fails closed: the first violation in the documented
// field order aborts the parse, nothing is half-accepted.
//
// Enforced subset (documented, NOT full JSON Schema — recorded limitation):
// - Versions: strict `MAJOR.MINOR.PATCH` numeric SemVer core. Prerelease and
//   build metadata are REJECTED in v1 (named error), because a plugin
//   package carrying a pre-release must also carry the signing/provenance
//   semantics that P7.14 adds — not accepted half-supported.
// - `api-version` ranges: a comma-separated comparator list from
//   `{=, >=, <=, >, <, ^, ~}` over the same SemVer core, parsed as a RANGE
//   (never string-compared; P7.41 property-tests this).
// - `configuration-schema`: an embedded declaration validated STRUCTURALLY
//   against the versioned subset below (type/properties/required/
//   additionalProperties:false). Full JSON Schema is out of scope for v1.
// - Secrets: references ONLY (`env:`, `file:`, `secret-store:` schemes), with
//   closed object keys `{name, reference}`. A manifest is STRUCTURALLY
//   INCAPABLE of carrying a secret value: any other key in a secret object
//   is an unknown-field rejection, and a reference without a scheme prefix is
//   rejected as a likely inline credential. Error messages deliberately do
//   NOT echo the rejected reference value (it may be the credential itself).
// - Permissions: closed ids from the §5.7 capability table, never free text;
//   the never-granted set (`dispatch.authorize`, `dispatch.issue`,
//   `secret.read`, `network.any`) is rejected at the manifest gate — the
//   earliest point it can be refused (P7.11/P7.12 enforce it again later).
//
// No `unwrap`/`expect` on untrusted input: every JSON access is matched, and
// integer parsing goes through checked conversions. Determinism: reported
// unknown-field names and aggregate outputs are SORTED before selection —
// nothing depends on `serde_json::Value` map iteration order (keys are
// looked up by name, never iterated for decisions without an explicit sort).

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

/// The versioned manifest envelope id (P7.05 step 2). A manifest whose
/// `manifest-schema` field is not exactly this value is rejected — never
/// parsed permissively.
pub const MANIFEST_SCHEMA_ID: &str = "flexcore-plugin-manifest-v1";

/// Maximum accepted manifest source size in bytes (fail-closed bound; a
/// manifest is metadata, not a payload — P7.41 oversized-input material).
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;

// --- The §5.7 permission vocabulary (closed; ids, never free text) ----------

/// Permissions a manifest MAY request (§5.7 "Initial policy" column, allowed
/// rows). Exactly these string ids; anything else is [`ManifestError::PermissionIdUnknown`].
pub const ALLOWED_PERMISSIONS: [&str; 6] = [
    "telemetry.read",
    "asset.read",
    "forecast.publish",
    "schedule.propose",
    "dispatch.propose",
    "dispatch.validate.request",
];

/// Permissions NEVER granted to a plugin (§5.7 never-granted rows). A
/// manifest requesting one is rejected here — before composition, lifecycle,
/// or any broker exists to mis-grant it (P7.11 re-refuses at grant time).
pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];

/// The only accepted secret-reference schemes (P7.19 resolves these through
/// the host/adapter; the plugin never sees the value — §5.7).
pub const SECRET_REFERENCE_SCHEMES: [&str; 3] = ["env:", "file:", "secret-store:"];

// --- SemVer core -------------------------------------------------------------

/// A strict SemVer core: `MAJOR.MINOR.PATCH`, numeric, no prerelease/build
/// metadata (see the enforced-subset note in the module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    pub const fn new(major: u64, minor: u64, patch: u64) -> SemVer {
        SemVer { major, minor, patch }
    }

    /// Strict parse: exactly three dot-separated, all-digit, non-empty
    /// components without leading zeros. Nothing else is a v1 version.
    pub fn parse(input: &str) -> Result<SemVer, SemVerError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(SemVerError::new("empty version string"));
        }
        let parts: Vec<&str> = text.split('.').collect();
        if parts.len() != 3 {
            return Err(SemVerError::new(
                "expected exactly three dot-separated components MAJOR.MINOR.PATCH",
            ));
        }
        let mut out = [0u64; 3];
        for (idx, part) in parts.iter().enumerate() {
            out[idx] = parse_component(part)?;
        }
        Ok(SemVer { major: out[0], minor: out[1], patch: out[2] })
    }
}

fn parse_component(part: &str) -> Result<u64, SemVerError> {
    if part.is_empty() {
        return Err(SemVerError::new("empty version component"));
    }
    if part.len() > 19 {
        return Err(SemVerError::new("version component exceeds 19 digits"));
    }
    if !part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SemVerError::new(
            "version components must be numeric (no prerelease/build metadata in schema v1)",
        ));
    }
    if part.len() > 1 && part.starts_with('0') {
        return Err(SemVerError::new("leading zeros are not allowed"));
    }
    // Validated all-digits and <= 19 chars above, so this always fits u64;
    // the error path is kept explicit anyway (no unwrap, even where provably safe).
    part.parse::<u64>()
        .map_err(|_| SemVerError::new("version component out of range"))
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// SemVer-core parse failure (its reason text is embedded into the manifest
/// error; it never echoes more than the caller already has).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVerError {
    pub reason: String,
}

impl SemVerError {
    fn new(reason: &str) -> SemVerError {
        SemVerError { reason: reason.to_string() }
    }
}

impl fmt::Display for SemVerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

// --- api-version range (parsed, never string-compared) -----------------------

/// A parsed `api-version` range: a comma-separated list of comparators over
/// the SemVer core. The original source string is retained for error
/// messages and plan records; all decisions use the parsed comparators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiVersionRange {
    source: String,
    comparators: Vec<Comparator>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparatorOp {
    Eq,
    Gt,
    GtE,
    Lt,
    LtE,
    Caret,
    Tilde,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Comparator {
    op: ComparatorOp,
    version: SemVer,
}

impl ApiVersionRange {
    /// Parse the enforced comparator subset. An empty range or an empty
    /// comparator item is a parse error (a manifest must commit to a range;
    /// "any version" is spelled `>=0.0.0` and means it).
    pub fn parse(input: &str) -> Result<ApiVersionRange, SemVerError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(SemVerError::new("empty api-version range"));
        }
        let mut comparators = Vec::new();
        for raw in text.split(',') {
            let item = raw.trim();
            if item.is_empty() {
                return Err(SemVerError::new("empty comparator in api-version range"));
            }
            let (op, rest) = if let Some(rest) = item.strip_prefix(">=") {
                (ComparatorOp::GtE, rest)
            } else if let Some(rest) = item.strip_prefix("<=") {
                (ComparatorOp::LtE, rest)
            } else if let Some(rest) = item.strip_prefix('>') {
                (ComparatorOp::Gt, rest)
            } else if let Some(rest) = item.strip_prefix('<') {
                (ComparatorOp::Lt, rest)
            } else if let Some(rest) = item.strip_prefix('=') {
                (ComparatorOp::Eq, rest)
            } else if let Some(rest) = item.strip_prefix('^') {
                (ComparatorOp::Caret, rest)
            } else if let Some(rest) = item.strip_prefix('~') {
                (ComparatorOp::Tilde, rest)
            } else {
                (ComparatorOp::Eq, item)
            };
            let version = SemVer::parse(rest)
                .map_err(|_| SemVerError::new("comparator version is not a valid MAJOR.MINOR.PATCH core"))?;
            comparators.push(Comparator { op, version });
        }
        Ok(ApiVersionRange { source: text.to_string(), comparators })
    }

    /// The original range expression (for error messages and plan records).
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether `v` satisfies every comparator in the range.
    pub fn satisfied_by(&self, v: SemVer) -> bool {
        self.comparators.iter().all(|c| comparator_matches(*c, v))
    }

    /// Whether this range and `other` share at least one version. Used by the
    /// resolver for consumer↔provider compatibility (§9.3 compatibility
    /// matrix rule: composed plugins must target overlapping plugin-API
    /// windows). Bounds are computed conservatively from the comparators.
    pub fn intersects(&self, other: &ApiVersionRange) -> bool {
        let lo = max_bound(lower_bound(&self.comparators), lower_bound(&other.comparators));
        let upper = match (upper_bound(&self.comparators), upper_bound(&other.comparators)) {
            (None, None) => return true,
            (Some(u), None) | (None, Some(u)) => u,
            (Some(ua), Some(ub)) => min_bound(ua, ub),
        };
        bound_below(lo, upper)
    }
}

impl fmt::Display for ApiVersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

fn comparator_matches(c: Comparator, v: SemVer) -> bool {
    match c.op {
        ComparatorOp::Eq => v == c.version,
        ComparatorOp::Gt => v > c.version,
        ComparatorOp::GtE => v >= c.version,
        ComparatorOp::Lt => v < c.version,
        ComparatorOp::LtE => v <= c.version,
        ComparatorOp::Caret => v >= c.version && v < caret_upper(c.version),
        ComparatorOp::Tilde => v >= c.version && v < tilde_upper(c.version),
    }
}

/// `^X.Y.Z` upper bound (saturating at the numeric ceiling — a deterministic,
/// safe choice for versions no real plugin will ever declare).
fn caret_upper(c: SemVer) -> SemVer {
    if c.major > 0 {
        SemVer::new(c.major.saturating_add(1), 0, 0)
    } else if c.minor > 0 {
        SemVer::new(0, c.minor.saturating_add(1), 0)
    } else {
        SemVer::new(0, 0, c.patch.saturating_add(1))
    }
}

/// `~X.Y.Z` upper bound: the next minor.
fn tilde_upper(c: SemVer) -> SemVer {
    SemVer::new(c.major, c.minor.saturating_add(1), 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundInclusivity {
    Inclusive,
    Exclusive,
}

type Bound = (SemVer, BoundInclusivity);

fn lower_bound(comparators: &[Comparator]) -> Bound {
    let mut lo: Bound = (SemVer::new(0, 0, 0), BoundInclusivity::Inclusive);
    for c in comparators {
        let candidate = match c.op {
            ComparatorOp::Eq | ComparatorOp::GtE | ComparatorOp::Caret | ComparatorOp::Tilde => {
                (c.version, BoundInclusivity::Inclusive)
            }
            ComparatorOp::Gt => (c.version, BoundInclusivity::Exclusive),
            ComparatorOp::Lt | ComparatorOp::LtE => continue,
        };
        lo = max_bound(lo, candidate);
    }
    lo
}

fn upper_bound(comparators: &[Comparator]) -> Option<Bound> {
    let mut hi: Option<Bound> = None;
    for c in comparators {
        let candidate = match c.op {
            ComparatorOp::Eq => (c.version, BoundInclusivity::Inclusive),
            ComparatorOp::Lt => (c.version, BoundInclusivity::Exclusive),
            ComparatorOp::LtE => (c.version, BoundInclusivity::Inclusive),
            ComparatorOp::Caret => (caret_upper(c.version), BoundInclusivity::Exclusive),
            ComparatorOp::Tilde => (tilde_upper(c.version), BoundInclusivity::Exclusive),
            ComparatorOp::Gt | ComparatorOp::GtE => continue,
        };
        hi = Some(match hi {
            None => candidate,
            Some(current) => min_bound(current, candidate),
        });
    }
    hi
}

/// Tighter-of-two LOWER bounds (at equal versions, exclusive is tighter).
fn max_bound(a: Bound, b: Bound) -> Bound {
    if a.0 > b.0 {
        a
    } else if b.0 > a.0 {
        b
    } else if a.1 == BoundInclusivity::Exclusive || b.1 == BoundInclusivity::Exclusive {
        (a.0, BoundInclusivity::Exclusive)
    } else {
        (a.0, BoundInclusivity::Inclusive)
    }
}

/// Tighter-of-two UPPER bounds (at equal versions, exclusive is tighter).
fn min_bound(a: Bound, b: Bound) -> Bound {
    if a.0 < b.0 {
        a
    } else if b.0 < a.0 {
        b
    } else if a.1 == BoundInclusivity::Exclusive || b.1 == BoundInclusivity::Exclusive {
        (a.0, BoundInclusivity::Exclusive)
    } else {
        (a.0, BoundInclusivity::Inclusive)
    }
}

/// Non-empty open interval check: lower strictly below upper, or exactly at
/// it when both ends are inclusive.
fn bound_below(lo: Bound, hi: Bound) -> bool {
    if lo.0 < hi.0 {
        true
    } else if lo.0 > hi.0 {
        false
    } else {
        lo.1 == BoundInclusivity::Inclusive && hi.1 == BoundInclusivity::Inclusive
    }
}

// --- Plugin class and entry point --------------------------------------------

/// The §5.4 plugin execution classes, under the §5.5 runtime-kind names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginClass {
    /// Class A — trusted built-in Rust module (§5.4 Class A).
    BuiltInRust,
    /// Class B — sandboxed WASM component (§5.4 Class B).
    WasmComponent,
    /// Class C — out-of-process gRPC worker (§5.4 Class C).
    GrpcWorker,
}

impl PluginClass {
    pub fn parse(input: &str) -> Result<PluginClass, ManifestError> {
        match input {
            "builtin-rust" => Ok(PluginClass::BuiltInRust),
            "wasm-component" => Ok(PluginClass::WasmComponent),
            "grpc-worker" => Ok(PluginClass::GrpcWorker),
            _ => Err(ManifestError::UnknownClass { value: input.to_string() }),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PluginClass::BuiltInRust => "builtin-rust",
            PluginClass::WasmComponent => "wasm-component",
            PluginClass::GrpcWorker => "grpc-worker",
        }
    }

    /// The entry-point key this class REQUIRES (§5.5: per-class entry point).
    pub fn entry_point_key(&self) -> &'static str {
        match self {
            PluginClass::BuiltInRust => "registered-type",
            PluginClass::WasmComponent => "symbol",
            PluginClass::GrpcWorker => "target",
        }
    }
}

impl fmt::Display for PluginClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Per-class entry point (exactly one of, matching the declared class):
/// `registered-type` for Class A, `symbol` (the WIT-exported symbol id) for
/// Class B, `target` (the gRPC target URI) for Class C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryPoint {
    RegisteredType(String),
    Symbol(String),
    Target(String),
}

// --- Declared structures ------------------------------------------------------

/// A secret REFERENCE: a name and a scheme-prefixed location. There is no
/// field anywhere in the manifest that can carry a secret VALUE — the object
/// keys are closed to `{name, reference}` and the reference must name a
/// location via one of [`SECRET_REFERENCE_SCHEMES`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReference {
    pub name: String,
    pub reference: String,
}

/// Declared resource bounds (§5.5 `resources`; §5.4 Class B limits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBounds {
    pub max_memory_mb: u64,
    pub timeout_ms: u64,
    pub max_output_mb: Option<u64>,
    pub max_calls_per_minute: Option<u64>,
}

/// Manifest metadata (§9.3: validate publisher/version before activation).
/// `license` is REQUIRED — a plugin with no declared license cannot be
/// distributed (D4 discipline applied to plugins, DEFAULT decision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestMetadata {
    pub license: String,
    pub description: String,
    pub source: Option<String>,
}

/// The configuration property types of the enforced v1 subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigPropertyType {
    Str,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
}

impl ConfigPropertyType {
    fn parse(input: &str) -> Option<ConfigPropertyType> {
        match input {
            "string" => Some(ConfigPropertyType::Str),
            "number" => Some(ConfigPropertyType::Number),
            "integer" => Some(ConfigPropertyType::Integer),
            "boolean" => Some(ConfigPropertyType::Boolean),
            "array" => Some(ConfigPropertyType::Array),
            "object" => Some(ConfigPropertyType::Object),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ConfigPropertyType::Str => "string",
            ConfigPropertyType::Number => "number",
            ConfigPropertyType::Integer => "integer",
            ConfigPropertyType::Boolean => "boolean",
            ConfigPropertyType::Array => "array",
            ConfigPropertyType::Object => "object",
        }
    }
}

/// One property of the enforced configuration-schema subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProperty {
    pub ty: ConfigPropertyType,
    pub description: Option<String>,
}

/// The structurally validated `configuration-schema` declaration (the v1
/// subset — see the module header and the published schema document). Full
/// JSON Schema is explicitly NOT implemented (recorded limitation).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigurationSchema {
    pub properties: BTreeMap<String, ConfigProperty>,
    pub required: Vec<String>,
}

/// A validated plugin manifest (`flexcore-plugin-manifest-v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub plugin_id: String,
    pub name: String,
    pub version: SemVer,
    pub api_version: ApiVersionRange,
    pub class: PluginClass,
    /// Contract ids this plugin PROVIDES (capability seams, §5.2). Sorted is
    /// NOT imposed here — duplicates are rejected, and every resolver output
    /// is sorted at emission time.
    pub provider_of: Vec<String>,
    /// Contract ids this plugin CONSUMES (§5.5 `capabilities.requires`).
    pub consumes: Vec<String>,
    /// Requested permission ids from the closed §5.7 vocabulary.
    pub permissions: Vec<String>,
    pub configuration_schema: ConfigurationSchema,
    pub secrets: Vec<SecretReference>,
    pub entry_point: EntryPoint,
    pub resources: ResourceBounds,
    pub metadata: ManifestMetadata,
}

impl Manifest {
    /// Whether this manifest's api-version range covers `v` — the version-
    /// negotiation predicate (P7.05 acceptance: ranges parsed as ranges).
    pub fn covers_api_version(&self, v: SemVer) -> bool {
        self.api_version.satisfied_by(v)
    }
}

// --- Errors -------------------------------------------------------------------
//
// Every violation class has its own variant AND its own stable machine name
// ([`ManifestError::name`]) — the named-error table lives in the published
// schema document and the decision record. Error text never echoes a secret
// reference VALUE ([`ManifestError::SecretReferenceInvalid`] carries only the
// declared name and a static reason).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The source text was not valid JSON at all.
    ManifestJsonInvalid { reason: String },
    /// The JSON document parsed but is not an object.
    ManifestNotAnObject,
    /// `manifest-schema` is present but not [`MANIFEST_SCHEMA_ID`].
    SchemaVersionUnsupported { found: String },
    /// A field appeared that the closed key set for its level does not
    /// declare. Rejected, never warned (recorded choice, P7.05 step 3).
    UnknownField { location: String, field: String },
    /// A required field was absent.
    MissingField { field: String, location: String },
    /// A field was present with the wrong JSON type.
    WrongType { location: String, field: String, expected: &'static str, found: &'static str },
    /// `plugin-id` is not a valid namespaced id (`<namespace>.<name>`).
    PluginIdMalformed { value: String, reason: String },
    /// A version string is not a valid SemVer core.
    InvalidSemVer { field: String, value: String, reason: String },
    /// `api-version` is not a parseable comparator range.
    InvalidApiVersionRange { value: String, reason: String },
    /// `class` is not one of the §5.5 runtime kinds.
    UnknownClass { value: String },
    /// `entry-point` declared no entry.
    MissingEntryPoint,
    /// `entry-point` declared more than one entry.
    EntryPointAmbiguous { keys: Vec<String> },
    /// The entry-point key does not match the declared class.
    EntryPointMismatch { class: String, expected: String, found: String },
    /// The entry-point value is unusable (empty or overlong).
    EntryPointInvalid { reason: String },
    /// A provider-of/consumes entry is not a valid namespaced contract id.
    ContractIdMalformed { field: String, value: String, reason: String },
    /// The same contract id appears twice in `provider-of`.
    DuplicateProviderDeclaration { contract: String },
    /// The same contract id appears twice in `consumes`.
    DuplicateConsumesDeclaration { contract: String },
    /// A permission id is not in the §5.7 vocabulary.
    PermissionIdUnknown { value: String },
    /// A permission id is in the §5.7 never-granted set.
    PermissionNeverGranted { value: String },
    /// The same permission id appears twice in `permissions`.
    DuplicatePermissionDeclaration { value: String },
    /// The embedded configuration schema violates the enforced v1 subset.
    ConfigurationSchemaInvalid { reason: String },
    /// A secret reference does not name a scheme-prefixed location (likely an
    /// inline credential value). Deliberately does NOT echo the value.
    SecretReferenceInvalid { name: String, reason: String },
    /// Two secret references share one name.
    DuplicateSecretName { name: String },
    /// A declared resource bound is out of its closed range.
    ResourceBoundInvalid { field: String, reason: String },
}

impl ManifestError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            ManifestError::ManifestJsonInvalid { .. } => "MANIFEST_JSON_INVALID",
            ManifestError::ManifestNotAnObject => "MANIFEST_NOT_AN_OBJECT",
            ManifestError::SchemaVersionUnsupported { .. } => "SCHEMA_VERSION_UNSUPPORTED",
            ManifestError::UnknownField { .. } => "UNKNOWN_FIELD",
            ManifestError::MissingField { .. } => "MISSING_FIELD",
            ManifestError::WrongType { .. } => "WRONG_TYPE",
            ManifestError::PluginIdMalformed { .. } => "PLUGIN_ID_MALFORMED",
            ManifestError::InvalidSemVer { .. } => "INVALID_SEMVER",
            ManifestError::InvalidApiVersionRange { .. } => "INVALID_API_VERSION_RANGE",
            ManifestError::UnknownClass { .. } => "UNKNOWN_CLASS",
            ManifestError::MissingEntryPoint => "MISSING_ENTRY_POINT",
            ManifestError::EntryPointAmbiguous { .. } => "ENTRY_POINT_AMBIGUOUS",
            ManifestError::EntryPointMismatch { .. } => "ENTRY_POINT_MISMATCH",
            ManifestError::EntryPointInvalid { .. } => "ENTRY_POINT_INVALID",
            ManifestError::ContractIdMalformed { .. } => "CONTRACT_ID_MALFORMED",
            ManifestError::DuplicateProviderDeclaration { .. } => "DUPLICATE_PROVIDER_DECLARATION",
            ManifestError::DuplicateConsumesDeclaration { .. } => "DUPLICATE_CONSUMES_DECLARATION",
            ManifestError::PermissionIdUnknown { .. } => "PERMISSION_ID_UNKNOWN",
            ManifestError::PermissionNeverGranted { .. } => "PERMISSION_NEVER_GRANTED",
            ManifestError::DuplicatePermissionDeclaration { .. } => "DUPLICATE_PERMISSION_DECLARATION",
            ManifestError::ConfigurationSchemaInvalid { .. } => "CONFIGURATION_SCHEMA_INVALID",
            ManifestError::SecretReferenceInvalid { .. } => "SECRET_REFERENCE_INVALID",
            ManifestError::DuplicateSecretName { .. } => "DUPLICATE_SECRET_NAME",
            ManifestError::ResourceBoundInvalid { .. } => "RESOURCE_BOUND_INVALID",
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::ManifestJsonInvalid { reason } => {
                write!(f, "manifest is not valid JSON: {reason}")
            }
            ManifestError::ManifestNotAnObject => {
                write!(f, "manifest must be a JSON object")
            }
            ManifestError::SchemaVersionUnsupported { found } => {
                write!(f, "unsupported manifest-schema id '{found}' (expected '{MANIFEST_SCHEMA_ID}')")
            }
            ManifestError::UnknownField { location, field } => {
                write!(f, "unknown field '{field}' at {location} (unknown fields are rejected, not warned)")
            }
            ManifestError::MissingField { field, location } => {
                write!(f, "missing required field '{field}' at {location}")
            }
            ManifestError::WrongType { location, field, expected, found } => {
                write!(f, "field '{field}' at {location} must be {expected}, found {found}")
            }
            ManifestError::PluginIdMalformed { value, reason } => {
                write!(f, "plugin id '{value}' is malformed: {reason}")
            }
            ManifestError::InvalidSemVer { field, value, reason } => {
                write!(f, "field '{field}' is not a valid version '{value}': {reason}")
            }
            ManifestError::InvalidApiVersionRange { value, reason } => {
                write!(f, "api-version '{value}' is not a valid range: {reason}")
            }
            ManifestError::UnknownClass { value } => {
                write!(f, "unknown plugin class '{value}' (expected builtin-rust | wasm-component | grpc-worker)")
            }
            ManifestError::MissingEntryPoint => {
                write!(f, "entry-point declares no entry")
            }
            ManifestError::EntryPointAmbiguous { keys } => {
                write!(f, "entry-point declares multiple entries ({})", keys.join(", "))
            }
            ManifestError::EntryPointMismatch { class, expected, found } => {
                write!(f, "class '{class}' requires entry-point key '{expected}', found '{found}'")
            }
            ManifestError::EntryPointInvalid { reason } => {
                write!(f, "entry-point value is unusable: {reason}")
            }
            ManifestError::ContractIdMalformed { field, value, reason } => {
                write!(f, "{field} entry '{value}' is not a valid contract id: {reason}")
            }
            ManifestError::DuplicateProviderDeclaration { contract } => {
                write!(f, "contract '{contract}' is declared twice in provider-of")
            }
            ManifestError::DuplicateConsumesDeclaration { contract } => {
                write!(f, "contract '{contract}' is declared twice in consumes")
            }
            ManifestError::PermissionIdUnknown { value } => {
                write!(f, "permission '{value}' is not in the declared capability vocabulary (ids, not free text)")
            }
            ManifestError::PermissionNeverGranted { value } => {
                write!(f, "permission '{value}' is in the never-granted set and cannot be requested by any plugin")
            }
            ManifestError::DuplicatePermissionDeclaration { value } => {
                write!(f, "permission '{value}' is requested twice")
            }
            ManifestError::ConfigurationSchemaInvalid { reason } => {
                write!(f, "configuration-schema violates the enforced v1 subset: {reason}")
            }
            ManifestError::SecretReferenceInvalid { name, reason } => {
                // The reference VALUE is deliberately not echoed: a rejected
                // reference may be an inline credential attempt.
                write!(f, "secret '{name}' is not a valid reference: {reason}")
            }
            ManifestError::DuplicateSecretName { name } => {
                write!(f, "secret name '{name}' is declared twice")
            }
            ManifestError::ResourceBoundInvalid { field, reason } => {
                write!(f, "resource bound '{field}' is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

// --- Parsing ------------------------------------------------------------------

/// Parse and validate a manifest from its JSON source text (fail-closed:
/// oversize input is rejected before parsing).
pub fn parse_manifest(text: &str) -> Result<Manifest, ManifestError> {
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::ManifestJsonInvalid {
            reason: format!("manifest exceeds the {} byte limit", MAX_MANIFEST_BYTES),
        });
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|err| ManifestError::ManifestJsonInvalid { reason: err.to_string() })?;
    parse_manifest_value(&value)
}

/// Parse and validate a manifest from an already-parsed JSON value (used by
/// tests and by future callers that receive JSON as structured data).
pub fn parse_manifest_value(value: &Value) -> Result<Manifest, ManifestError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => return Err(ManifestError::ManifestNotAnObject),
    };

    // 1. Envelope: the versioned schema id, before anything else.
    let schema_id = required_string(obj, "manifest-schema", "manifest")?;
    if schema_id != MANIFEST_SCHEMA_ID {
        return Err(ManifestError::SchemaVersionUnsupported { found: schema_id });
    }

    // 2. Closed top-level key set: unknown fields REJECTED (sorted selection
    //    — no dependence on map iteration order).
    const ALLOWED_TOP_LEVEL: [&str; 14] = [
        "manifest-schema", "plugin-id", "name", "version", "api-version", "class",
        "entry-point", "provider-of", "consumes", "permissions", "configuration-schema",
        "secrets", "resources", "metadata",
    ];
    reject_unknown_fields(obj, &ALLOWED_TOP_LEVEL, "manifest")?;

    const REQUIRED_TOP_LEVEL: [&str; 12] = [
        "plugin-id", "name", "version", "api-version", "class", "entry-point",
        "provider-of", "consumes", "permissions", "configuration-schema", "resources",
        "metadata",
    ];
    for field in REQUIRED_TOP_LEVEL {
        if !obj.contains_key(field) {
            return Err(ManifestError::MissingField {
                field: field.to_string(),
                location: "manifest".to_string(),
            });
        }
    }

    // 3. Identity.
    let plugin_id = required_string(obj, "plugin-id", "manifest")?;
    validate_plugin_id(&plugin_id).map_err(|reason| ManifestError::PluginIdMalformed {
        value: plugin_id.clone(),
        reason,
    })?;
    let name = required_string(obj, "name", "manifest")?;
    if name.is_empty() || name.len() > 256 {
        return Err(ManifestError::WrongType {
            location: "manifest".to_string(),
            field: "name".to_string(),
            expected: "a non-empty string of at most 256 characters",
            found: "string",
        });
    }
    let version_text = required_string(obj, "version", "manifest")?;
    let version = SemVer::parse(&version_text).map_err(|err| ManifestError::InvalidSemVer {
        field: "version".to_string(),
        value: version_text.clone(),
        reason: err.reason,
    })?;
    let api_text = required_string(obj, "api-version", "manifest")?;
    let api_version =
        ApiVersionRange::parse(&api_text).map_err(|err| ManifestError::InvalidApiVersionRange {
            value: api_text.clone(),
            reason: err.reason,
        })?;
    let class_text = required_string(obj, "class", "manifest")?;
    let class = PluginClass::parse(&class_text)?;

    // 4. Entry point: exactly one key, and the one the class requires.
    let entry_point = parse_entry_point(
        obj.get("entry-point").ok_or_else(|| ManifestError::MissingField {
            field: "entry-point".to_string(),
            location: "manifest".to_string(),
        })?,
        class,
    )?;

    // 5. Capability declarations.
    let provider_of = parse_contract_list(
        obj.get("provider-of").ok_or_else(|| ManifestError::MissingField {
            field: "provider-of".to_string(),
            location: "manifest".to_string(),
        })?,
        "provider-of",
    )?;
    let consumes = parse_contract_list(
        obj.get("consumes").ok_or_else(|| ManifestError::MissingField {
            field: "consumes".to_string(),
            location: "manifest".to_string(),
        })?,
        "consumes",
    )?;
    let permissions = parse_permissions(
        obj.get("permissions").ok_or_else(|| ManifestError::MissingField {
            field: "permissions".to_string(),
            location: "manifest".to_string(),
        })?,
    )?;

    // 6. Configuration schema (structural subset validation).
    let configuration_schema = parse_configuration_schema(
        obj.get("configuration-schema").ok_or_else(|| ManifestError::MissingField {
            field: "configuration-schema".to_string(),
            location: "manifest".to_string(),
        })?,
    )?;

    // 7. Secret references (references only — never values).
    let secrets = match obj.get("secrets") {
        None => Vec::new(),
        Some(v) => parse_secrets(v)?,
    };

    // 8. Resource bounds.
    let resources = parse_resources(
        obj.get("resources").ok_or_else(|| ManifestError::MissingField {
            field: "resources".to_string(),
            location: "manifest".to_string(),
        })?,
    )?;

    // 9. Metadata.
    let metadata = parse_metadata(
        obj.get("metadata").ok_or_else(|| ManifestError::MissingField {
            field: "metadata".to_string(),
            location: "manifest".to_string(),
        })?,
    )?;

    Ok(Manifest {
        plugin_id,
        name,
        version,
        api_version,
        class,
        provider_of,
        consumes,
        permissions,
        configuration_schema,
        secrets,
        entry_point,
        resources,
        metadata,
    })
}

// --- Field-level helpers --------------------------------------------------------

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

fn required_string(
    obj: &serde_json::Map<String, Value>,
    field: &str,
    location: &str,
) -> Result<String, ManifestError> {
    match obj.get(field) {
        None => Err(ManifestError::MissingField {
            field: field.to_string(),
            location: location.to_string(),
        }),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(ManifestError::WrongType {
            location: location.to_string(),
            field: field.to_string(),
            expected: "string",
            found: json_type_name(other),
        }),
    }
}

fn reject_unknown_fields(
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    location: &str,
) -> Result<(), ManifestError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(ManifestError::UnknownField {
            location: location.to_string(),
            field: field.clone(),
        }),
    }
}

/// `<namespace>.<name>` plugin id: lowercase ASCII, at least two dot-separated
/// segments, each starting with a letter and using `[a-z0-9-]` after.
///
/// Public since P7.14/P7.15: the signing module (provenance records bind
/// `manifest-id`) and the profile module (profile selections name
/// `manifest-id`) re-validate plugin ids handed to them directly, so an
/// id outside a manifest must satisfy the same grammar the manifest
/// validator enforces. Logic unchanged — visibility only.
pub fn validate_plugin_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("empty plugin id".to_string());
    }
    if id.len() > 128 {
        return Err("plugin id exceeds 128 characters".to_string());
    }
    if !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-') {
        return Err("plugin id must be lowercase ASCII letters, digits, dots, and hyphens".to_string());
    }
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() < 2 {
        return Err("plugin id must be namespaced: <namespace>.<name>".to_string());
    }
    for segment in segments {
        if segment.is_empty() {
            return Err("empty segment in plugin id".to_string());
        }
        if segment.len() > 63 {
            return Err("plugin id segment exceeds 63 characters".to_string());
        }
        let first = segment.as_bytes()[0];
        if !first.is_ascii_lowercase() {
            return Err("each plugin id segment must start with a lowercase letter".to_string());
        }
        if !segment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("plugin id segments may only contain lowercase letters, digits, and hyphens"
                .to_string());
        }
    }
    Ok(())
}

/// Contract id: at least two dot-separated segments, each `[a-z0-9-]` and
/// starting with a letter (e.g. `optimization.lp`, `portfolio.read`).
///
/// Public since P7.08: the built-in provider registry re-validates contract
/// ids handed to it directly (a factory registered outside a plan must
/// satisfy the same id grammar the manifest validator enforces). Logic
/// unchanged — visibility only.
pub fn validate_contract_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("empty contract id".to_string());
    }
    if id.len() > 128 {
        return Err("contract id exceeds 128 characters".to_string());
    }
    if !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-') {
        return Err("contract id must be lowercase ASCII letters, digits, dots, and hyphens"
            .to_string());
    }
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() < 2 {
        return Err("contract id must be namespaced: <domain>.<capability>".to_string());
    }
    for segment in segments {
        if segment.is_empty() {
            return Err("empty segment in contract id".to_string());
        }
        let first = segment.as_bytes()[0];
        if !first.is_ascii_lowercase() {
            return Err("each contract id segment must start with a lowercase letter".to_string());
        }
    }
    Ok(())
}

fn parse_entry_point(value: &Value, class: PluginClass) -> Result<EntryPoint, ManifestError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(ManifestError::WrongType {
                location: "entry-point".to_string(),
                field: "entry-point".to_string(),
                expected: "object",
                found: json_type_name(value),
            })
        }
    };
    const ALLOWED_ENTRY_KEYS: [&str; 3] = ["registered-type", "symbol", "target"];
    reject_unknown_fields(obj, &ALLOWED_ENTRY_KEYS, "entry-point")?;
    let present: Vec<&String> = obj
        .keys()
        .filter(|k| ALLOWED_ENTRY_KEYS.contains(&k.as_str()))
        .collect();
    if present.is_empty() {
        return Err(ManifestError::MissingEntryPoint);
    }
    if present.len() > 1 {
        let mut keys: Vec<String> = present.iter().map(|k| (*k).clone()).collect();
        keys.sort();
        return Err(ManifestError::EntryPointAmbiguous { keys });
    }
    let key = present[0].as_str();
    if key != class.entry_point_key() {
        return Err(ManifestError::EntryPointMismatch {
            class: class.name().to_string(),
            expected: class.entry_point_key().to_string(),
            found: key.to_string(),
        });
    }
    let text = match obj.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(ManifestError::WrongType {
                location: "entry-point".to_string(),
                field: key.to_string(),
                expected: "string",
                found: json_type_name(other),
            })
        }
        None => {
            return Err(ManifestError::MissingField {
                field: key.to_string(),
                location: "entry-point".to_string(),
            })
        }
    };
    if text.is_empty() || text.len() > 256 {
        return Err(ManifestError::EntryPointInvalid {
            reason: "entry-point value must be a non-empty string of at most 256 characters"
                .to_string(),
        });
    }
    Ok(match key {
        "registered-type" => EntryPoint::RegisteredType(text),
        "symbol" => EntryPoint::Symbol(text),
        "target" => EntryPoint::Target(text),
        // Unreachable: `key` is one of the three allowed keys above.
        _ => EntryPoint::RegisteredType(text),
    })
}

fn string_array<'a>(
    value: &'a Value,
    field: &str,
) -> Result<Vec<&'a str>, ManifestError> {
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(s) => out.push(s.as_str()),
                    other => {
                        return Err(ManifestError::WrongType {
                            location: field.to_string(),
                            field: field.to_string(),
                            expected: "array of strings",
                            found: json_type_name(other),
                        })
                    }
                }
            }
            Ok(out)
        }
        other => Err(ManifestError::WrongType {
            location: "manifest".to_string(),
            field: field.to_string(),
            expected: "array of strings",
            found: json_type_name(other),
        }),
    }
}

fn parse_contract_list(value: &Value, field: &str) -> Result<Vec<String>, ManifestError> {
    let items = string_array(value, field)?;
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        if item.is_empty() || item.len() > 128 {
            return Err(ManifestError::ContractIdMalformed {
                field: field.to_string(),
                value: item.to_string(),
                reason: "contract ids are non-empty strings of at most 128 characters".to_string(),
            });
        }
        validate_contract_id(item).map_err(|reason| ManifestError::ContractIdMalformed {
            field: field.to_string(),
            value: item.to_string(),
            reason,
        })?;
        if seen.iter().any(|s| s == item) {
            return Err(if field == "provider-of" {
                ManifestError::DuplicateProviderDeclaration { contract: item.to_string() }
            } else {
                ManifestError::DuplicateConsumesDeclaration { contract: item.to_string() }
            });
        }
        seen.push(item.to_string());
    }
    Ok(seen)
}

fn parse_permissions(value: &Value) -> Result<Vec<String>, ManifestError> {
    let items = string_array(value, "permissions")?;
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        if NEVER_GRANTED_PERMISSIONS.contains(&item) {
            return Err(ManifestError::PermissionNeverGranted { value: item.to_string() });
        }
        if !ALLOWED_PERMISSIONS.contains(&item) {
            return Err(ManifestError::PermissionIdUnknown { value: item.to_string() });
        }
        if seen.iter().any(|s| s == item) {
            return Err(ManifestError::DuplicatePermissionDeclaration { value: item.to_string() });
        }
        seen.push(item.to_string());
    }
    Ok(seen)
}

fn parse_configuration_schema(value: &Value) -> Result<ConfigurationSchema, ManifestError> {
    const LOCATION: &str = "configuration-schema";
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(ManifestError::ConfigurationSchemaInvalid {
                reason: "configuration-schema must be a JSON object".to_string(),
            })
        }
    };
    const ALLOWED: [&str; 4] = ["type", "properties", "required", "additionalProperties"];
    reject_unknown_fields(obj, &ALLOWED, LOCATION)?;
    if !obj.contains_key("type") {
        return Err(ManifestError::MissingField {
            field: "type".to_string(),
            location: LOCATION.to_string(),
        });
    }
    if !obj.contains_key("additionalProperties") {
        return Err(ManifestError::MissingField {
            field: "additionalProperties".to_string(),
            location: LOCATION.to_string(),
        });
    }
    match obj.get("type") {
        Some(Value::String(t)) if t == "object" => {}
        Some(_) => {
            return Err(ManifestError::ConfigurationSchemaInvalid {
                reason: "configuration-schema.type must be the string \"object\"".to_string(),
            })
        }
        None => {}
    }
    match obj.get("additionalProperties") {
        Some(Value::Bool(false)) => {}
        _ => {
            return Err(ManifestError::ConfigurationSchemaInvalid {
                reason: "configuration-schema.additionalProperties must be the literal false (the v1 subset is fail-closed)".to_string(),
            })
        }
    }

    let mut properties: BTreeMap<String, ConfigProperty> = BTreeMap::new();
    if let Some(props_value) = obj.get("properties") {
        let props = match props_value {
            Value::Object(map) => map,
            _ => {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: "configuration-schema.properties must be a JSON object".to_string(),
                })
            }
        };
        for (prop_name, prop_value) in props {
            if prop_name.is_empty() || prop_name.len() > 64 {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: "property names are non-empty strings of at most 64 characters"
                        .to_string(),
                });
            }
            if !prop_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
            {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: "property names may only contain ASCII letters, digits, '_', '.', and '-'"
                        .to_string(),
                });
            }
            let prop_obj = match prop_value {
                Value::Object(map) => map,
                _ => {
                    return Err(ManifestError::ConfigurationSchemaInvalid {
                        reason: format!("property '{prop_name}' must be a JSON object"),
                    })
                }
            };
            const ALLOWED_PROP_KEYS: [&str; 2] = ["type", "description"];
            reject_unknown_fields(
                prop_obj,
                &ALLOWED_PROP_KEYS,
                &format!("{LOCATION}.properties.{prop_name}"),
            )?;
            let type_text = match prop_obj.get("type") {
                Some(Value::String(t)) => t.clone(),
                Some(_) => {
                    return Err(ManifestError::ConfigurationSchemaInvalid {
                        reason: format!("property '{prop_name}' type must be a string"),
                    })
                }
                None => {
                    return Err(ManifestError::MissingField {
                        field: "type".to_string(),
                        location: format!("{LOCATION}.properties.{prop_name}"),
                    })
                }
            };
            let ty = ConfigPropertyType::parse(&type_text).ok_or_else(|| {
                ManifestError::ConfigurationSchemaInvalid {
                    reason: format!(
                        "property '{prop_name}' type '{type_text}' is not one of string|number|integer|boolean|array|object"
                    ),
                }
            })?;
            let description = match prop_obj.get("description") {
                None => None,
                Some(Value::String(d)) => Some(d.clone()),
                Some(_) => {
                    return Err(ManifestError::ConfigurationSchemaInvalid {
                        reason: format!("property '{prop_name}' description must be a string"),
                    })
                }
            };
            properties.insert(prop_name.clone(), ConfigProperty { ty, description });
        }
    }

    let mut required: Vec<String> = Vec::new();
    if let Some(req_value) = obj.get("required") {
        let items = match req_value {
            Value::Array(items) => items,
            _ => {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: "configuration-schema.required must be an array of property names"
                        .to_string(),
                })
            }
        };
        for item in items {
            let name = match item {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(ManifestError::ConfigurationSchemaInvalid {
                        reason: "configuration-schema.required entries must be strings".to_string(),
                    })
                }
            };
            if !properties.contains_key(&name) {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: format!("required property '{name}' is not declared in properties"),
                });
            }
            if required.iter().any(|r| r == &name) {
                return Err(ManifestError::ConfigurationSchemaInvalid {
                    reason: format!("required property '{name}' is listed twice"),
                });
            }
            required.push(name);
        }
    }

    Ok(ConfigurationSchema { properties, required })
}

fn parse_secrets(value: &Value) -> Result<Vec<SecretReference>, ManifestError> {
    let items = match value {
        Value::Array(items) => items,
        _ => {
            return Err(ManifestError::WrongType {
                location: "secrets".to_string(),
                field: "secrets".to_string(),
                expected: "array of secret-reference objects",
                found: json_type_name(value),
            })
        }
    };
    let mut out: Vec<SecretReference> = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let location = format!("secrets[{idx}]");
        let obj = match item {
            Value::Object(map) => map,
            _ => {
                return Err(ManifestError::WrongType {
                    location: location.clone(),
                    field: "secrets".to_string(),
                    expected: "object with keys {name, reference}",
                    found: json_type_name(item),
                })
            }
        };
        const ALLOWED_SECRET_KEYS: [&str; 2] = ["name", "reference"];
        reject_unknown_fields(obj, &ALLOWED_SECRET_KEYS, &location)?;
        let name = required_string(obj, "name", &location)?;
        let reference = required_string(obj, "reference", &location)?;
        if name.is_empty() || name.len() > 256 {
            return Err(ManifestError::SecretReferenceInvalid {
                name,
                reason: "secret names are non-empty strings of at most 256 characters".to_string(),
            });
        }
        // Reference, not value: must name a location through one of the
        // accepted schemes. The rejected VALUE is never echoed back.
        let scheme_ok = SECRET_REFERENCE_SCHEMES.iter().any(|scheme| {
            reference
                .strip_prefix(*scheme)
                .map(|rest| !rest.is_empty())
                .unwrap_or(false)
        });
        if !scheme_ok {
            return Err(ManifestError::SecretReferenceInvalid {
                name,
                reason: "a secret reference must use the env:, file:, or secret-store: scheme and name a location; manifest secrets are references only and can never carry values".to_string(),
            });
        }
        if out.iter().any(|s| s.name == name) {
            return Err(ManifestError::DuplicateSecretName { name });
        }
        out.push(SecretReference { name, reference });
    }
    Ok(out)
}

fn parse_resource_u64(
    obj: &serde_json::Map<String, Value>,
    field: &str,
    min: u64,
    max: u64,
    required: bool,
) -> Result<Option<u64>, ManifestError> {
    match obj.get(field) {
        None => {
            if required {
                Err(ManifestError::MissingField {
                    field: field.to_string(),
                    location: "resources".to_string(),
                })
            } else {
                Ok(None)
            }
        }
        Some(Value::Number(n)) => {
            let v = match n.as_u64() {
                Some(v) => v,
                None => {
                    return Err(ManifestError::ResourceBoundInvalid {
                        field: field.to_string(),
                        reason: "resource bounds must be non-negative integers".to_string(),
                    })
                }
            };
            if v < min || v > max {
                return Err(ManifestError::ResourceBoundInvalid {
                    field: field.to_string(),
                    reason: format!("resource bound must be between {min} and {max}"),
                });
            }
            Ok(Some(v))
        }
        Some(other) => Err(ManifestError::WrongType {
            location: "resources".to_string(),
            field: field.to_string(),
            expected: "unsigned integer",
            found: json_type_name(other),
        }),
    }
}

fn parse_resources(value: &Value) -> Result<ResourceBounds, ManifestError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(ManifestError::WrongType {
                location: "resources".to_string(),
                field: "resources".to_string(),
                expected: "object",
                found: json_type_name(value),
            })
        }
    };
    const ALLOWED_RESOURCE_KEYS: [&str; 4] = [
        "max-memory-mb", "timeout-ms", "max-output-mb", "max-calls-per-minute",
    ];
    reject_unknown_fields(obj, &ALLOWED_RESOURCE_KEYS, "resources")?;
    let max_memory_mb =
        parse_resource_u64(obj, "max-memory-mb", 1, 65_536, true)?.unwrap_or(0);
    let timeout_ms =
        parse_resource_u64(obj, "timeout-ms", 1, 3_600_000, true)?.unwrap_or(0);
    let max_output_mb = parse_resource_u64(obj, "max-output-mb", 1, 4_096, false)?;
    let max_calls_per_minute =
        parse_resource_u64(obj, "max-calls-per-minute", 1, 1_000_000, false)?;
    Ok(ResourceBounds { max_memory_mb, timeout_ms, max_output_mb, max_calls_per_minute })
}

fn parse_metadata(value: &Value) -> Result<ManifestMetadata, ManifestError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(ManifestError::WrongType {
                location: "metadata".to_string(),
                field: "metadata".to_string(),
                expected: "object",
                found: json_type_name(value),
            })
        }
    };
    const ALLOWED_METADATA_KEYS: [&str; 3] = ["license", "description", "source"];
    reject_unknown_fields(obj, &ALLOWED_METADATA_KEYS, "metadata")?;
    let license = required_string(obj, "license", "metadata")?;
    if license.is_empty() || license.len() > 128 {
        return Err(ManifestError::WrongType {
            location: "metadata".to_string(),
            field: "license".to_string(),
            expected: "a non-empty string of at most 128 characters",
            found: "string",
        });
    }
    let description = required_string(obj, "description", "metadata")?;
    if description.is_empty() || description.len() > 2000 {
        return Err(ManifestError::WrongType {
            location: "metadata".to_string(),
            field: "description".to_string(),
            expected: "a non-empty string of at most 2000 characters",
            found: "string",
        });
    }
    let source = match obj.get("source") {
        None => None,
        Some(Value::String(s)) => {
            if s.is_empty() || s.len() > 256 {
                return Err(ManifestError::WrongType {
                    location: "metadata".to_string(),
                    field: "source".to_string(),
                    expected: "a non-empty string of at most 256 characters",
                    found: "string",
                });
            }
            Some(s.clone())
        }
        Some(other) => {
            return Err(ManifestError::WrongType {
                location: "metadata".to_string(),
                field: "source".to_string(),
                expected: "string",
                found: json_type_name(other),
            })
        }
    };
    Ok(ManifestMetadata { license, description, source })
}
