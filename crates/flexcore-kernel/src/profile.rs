// P7.15 — Composition profiles and bundles (report §5.8, §5.1, §21.4;
// ledger `ledger_phase_7.md` P7.15; PLAN.md §3 "Profiles/bundles =
// Profile definitions (data)").
//
// A profile IS DATA, never code: a versioned JSON document
// (`flexcore-profile-v1`) naming an ORDERED list of plugin selections
// (manifest id + version constraint), a profile-level PERMISSION CEILING,
// and the profile's control-authority posture. There are no hidden
// defaults and no code path behind a profile: everything the profile
// does is visible in its file (the ledger's recorded risk — "a profile
// that is really a code path with hidden defaults" — is countered by
// construction).
//
// The shipped profiles live in `plugins/profiles/` (the five §5.8 names:
// `sim-lab`, `vpp-operator`, `site-edge`, `research-gnn`, `research-marl`)
// and are loaded by exactly this validator. Loading is STRICT and
// fail-closed, in the [`crate::manifest`] validation style: closed key
// sets at every level, unknown fields REJECTED (never warned), named
// errors for every violation, first violation aborts. One documented
// extension over the manifest parser: FULL-LINE `//` comments are
// stripped before JSON parsing, because a profile that cannot fully
// resolve in v0.5.0 ships with its unresolvable selections COMMENTED OUT
// in the JSON (the documented pattern; see `plugins/profiles/README.md`).
// Comment-stripping is deterministic (whole lines only, no trailing
// comments) and happens before any size-parsed content is interpreted.
//
// The control-authority posture is a CLOSED-VOCABULARITY field, and the
// lock is absolute (report §5.8/§21.4; 02_scope_freeze §4): the control-
// authority mode is a DEPLOYMENT/PER-SITE setting — a profile, like a
// plugin, can NEVER select or raise it. Parsing accepts EXACTLY ONE
// value, `simulation-shadow`; any other value — including the other §21.4
// modes — is the named error
// [`ProfileError::ControlAuthorityModeForbidden`]. The
// [`ControlAuthorityPosture`] enum carries the other mode names for the
// recomposition boundary's defence-in-depth comparison (P7.16), where a
// hand-constructed posture other than `SimulationShadow` is refused;
// from profile DATA they are unreachable.
//
// The permission-ceiling model: the profile declares the MAXIMUM set of
// §5.7 permission ids ANY of its selections may carry
// (`permission-ceiling`, closed vocabulary, never-granted ids refused at
// parse). At plan validation, every selected manifest's `permissions`
// must be a SUBSET of the ceiling — a profile can never grant beyond its
// ceiling, and a manifest requesting more than the ceiling allows is a
// named rejection. The ceiling is additive-downward only: it restricts;
// it never grants. The §5.7 never-granted set (`dispatch.authorize`,
// `dispatch.issue`, `secret.read`, `network.any`) is not part of the
// profile vocabulary at all (the P7.05 validator already refuses it at
// the manifest gate; it is re-refused here at the ceiling gate).
//
// Profile → plan: every selection resolves through the P7.06 resolver —
// the referenced manifests must exist, cover the pinned platform API
// version, satisfy the selection's version constraint, and resolve into a
// cycle- and conflict-free [`crate::resolve::CompositionPlan`]. A profile
// referencing an unknown manifest, an unsatisfiable version constraint,
// or a manifest claiming a kernel-protected contract is a named
// rejection (the protected-kernel guard is the RESOLVER's
// `PROTECTED_KERNEL_REPLACE`; this module wraps it, it does not weaken
// it). Each selection pins EVERY contract in the selected manifest's
// `provider-of` to that manifest as an explicit selection — a profile is
// the reviewable place where provider conflicts are resolved; nothing is
// ever resolved silently.
//
// Empty selections are legal AT PARSE (a profile shell loads — the
// ledger's "record which profiles are deferred and why") and refused AT
// PLAN VALIDATION (`PROFILE_EMPTY_SELECTIONS`): a shell can be loaded,
// reviewed, and diffed, but never composed.
//
// Determinism: validation is a pure function of (profile, manifests);
// selections are processed in declared order, aggregate maps are
// `BTreeMap`s, and the emitted plan is the resolver's deterministic
// output. No `unwrap`/`expect` on untrusted input anywhere.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

use crate::manifest::{
    validate_plugin_id, ApiVersionRange, Manifest, SemVer, ALLOWED_PERMISSIONS,
    NEVER_GRANTED_PERMISSIONS,
};
use crate::resolve::{resolve, CompositionPlan, ExplicitSelections, ResolveError,
    MAX_COMPOSITION_MANIFESTS};

/// The versioned profile envelope id (P7.15 step 1).
pub const PROFILE_SCHEMA_ID: &str = "flexcore-profile-v1";

/// Maximum accepted profile source size in bytes (fail-closed bound).
pub const MAX_PROFILE_BYTES: usize = 64 * 1024;

/// The ONLY control-authority posture profile data may carry (report
/// §5.8/§21.4: the mode is a deployment/per-site setting; v0.5.0 is
/// simulation/shadow — every shipped profile carries exactly this value).
pub const POSTURE_SIMULATION_SHADOW: &str = "simulation-shadow";

/// The five shipped §5.8 profile ids (PLAN.md §1). The set is OPEN as a
/// data vocabulary (a deployment may define more profiles as data) but
/// these five are what v0.5.0 ships.
pub const SHIPPED_PROFILE_IDS: [&str; 5] =
    ["sim-lab", "vpp-operator", "site-edge", "research-gnn", "research-marl"];

// --- Control-authority posture (the mode lock) ---------------------------------------

/// The §21.4 control-authority modes, as a closed vocabulary. Profile
/// DATA can only ever produce [`ControlAuthorityPosture::SimulationShadow`]
/// (the parser refuses everything else with
/// [`ProfileError::ControlAuthorityModeForbidden`]); the other variants
/// exist so the P7.16 recomposition boundary can compare postures and
/// refuse a mode change it is handed from non-profile code (defence in
/// depth — the composition-level restatement of "a profile switch can
/// never change the control-authority mode"). No constructor path leads
/// from profile data to any other variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAuthorityPosture {
    /// Simulation AND shadow operation (the v0.5.0 posture; §21.4).
    SimulationShadow,
    /// Read-only / shadow operation (§21.4). Never producible from
    /// profile data in this kernel version.
    ReadOnlyShadow,
    /// Operator-approved control (§21.4). Never producible from profile
    /// data in this kernel version.
    OperatorApproved,
    /// Bounded automation (§21.4). Never producible from profile data in
    /// this kernel version.
    BoundedAutomation,
}

impl ControlAuthorityPosture {
    /// The stable machine-readable name (the profile field's value).
    pub fn name(&self) -> &'static str {
        match self {
            ControlAuthorityPosture::SimulationShadow => POSTURE_SIMULATION_SHADOW,
            ControlAuthorityPosture::ReadOnlyShadow => "read-only-shadow",
            ControlAuthorityPosture::OperatorApproved => "operator-approved",
            ControlAuthorityPosture::BoundedAutomation => "bounded-automation",
        }
    }

    /// Parse the posture FROM PROFILE DATA. Accepts EXACTLY
    /// `simulation-shadow`; any other value is
    /// [`ProfileError::ControlAuthorityModeForbidden`] — the profile
    /// mechanism can never select (or be smuggled into) a control-capable
    /// mode (report §5.8: mode is a deployment/site setting).
    pub fn parse_from_profile_data(input: &str) -> Result<ControlAuthorityPosture, ProfileError> {
        if input == POSTURE_SIMULATION_SHADOW {
            return Ok(ControlAuthorityPosture::SimulationShadow);
        }
        Err(ProfileError::ControlAuthorityModeForbidden { value: input.to_string() })
    }
}

impl fmt::Display for ControlAuthorityPosture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// --- Profile data types ---------------------------------------------------------------

/// One ordered profile selection: a manifest id and the version constraint
/// the referenced manifest must satisfy. At plan validation the selection
/// pins every contract in the manifest's `provider-of` to this manifest
/// (explicit selection; never a silent resolution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSelection {
    pub manifest_id: String,
    pub version_constraint: ApiVersionRange,
}

/// A validated profile (`flexcore-profile-v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    profile_id: String,
    name: String,
    version: SemVer,
    description: String,
    control_authority: ControlAuthorityPosture,
    /// Sorted, deduplicated ceiling (BTreeSet semantics via BTreeMap).
    permission_ceiling: Vec<String>,
    /// The ordered selections (order preserved — the profile's bundle
    /// order is reviewable data).
    selections: Vec<ProfileSelection>,
}

impl Profile {
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> SemVer {
        self.version
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// The profile's control-authority posture (always
    /// [`ControlAuthorityPosture::SimulationShadow`] for anything parsed
    /// from data — the mode lock).
    pub fn control_authority(&self) -> ControlAuthorityPosture {
        self.control_authority
    }

    /// The sorted permission ceiling.
    pub fn permission_ceiling(&self) -> &[String] {
        &self.permission_ceiling
    }

    /// The ordered selections.
    pub fn selections(&self) -> &[ProfileSelection] {
        &self.selections
    }

    /// Whether `permission` may appear in ANY of this profile's plugins
    /// (the ceiling is a maximum, never a grant).
    pub fn permits(&self, permission: &str) -> bool {
        self.permission_ceiling.iter().any(|p| p == permission)
    }
}

// --- Errors -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// The source text was not valid JSON at all (also: oversize source).
    ProfileJsonInvalid { reason: String },
    /// The JSON document parsed but is not an object.
    ProfileNotAnObject,
    /// `profile-schema` is present but not [`PROFILE_SCHEMA_ID`].
    ProfileSchemaUnsupported { found: String },
    /// A field appeared that the closed key set for its level does not
    /// declare. Rejected, never warned.
    ProfileUnknownField { location: String, field: String },
    /// A required field was absent.
    ProfileMissingField { field: String, location: String },
    /// A field was present with the wrong JSON type.
    ProfileWrongType { location: String, field: String, expected: &'static str, found: &'static str },
    /// `profile-id` is not a valid profile id.
    ProfileIdMalformed { value: String, reason: String },
    /// A selection's `version-constraint` is not a parseable range.
    ProfileVersionConstraintInvalid { manifest_id: String, value: String, reason: String },
    /// A selection's `manifest-id` is not a valid plugin id.
    ProfileSelectionManifestIdMalformed { value: String, reason: String },
    /// The same manifest id appears twice in `selections`.
    ProfileDuplicateSelection { manifest_id: String },
    /// More selections than the composition cap.
    ProfileSelectionCountExcessive { count: usize },
    /// A ceiling entry is not in the §5.7 allowed vocabulary.
    ProfilePermissionCeilingUnknown { value: String },
    /// A ceiling entry is in the §5.7 never-granted set (a profile can
    /// never ceiling-grant what no manifest may request).
    ProfilePermissionCeilingNeverGranted { value: String },
    /// The same permission appears twice in `permission-ceiling`.
    ProfilePermissionCeilingDuplicate { value: String },
    /// The `control-authority` value is anything other than
    /// `simulation-shadow` — the profile mechanism can never select a
    /// control-capable mode (report §5.8/§21.4).
    ControlAuthorityModeForbidden { value: String },
    /// Plan validation on a profile with no active selections (a shell
    /// loads, but never composes).
    ProfileEmptySelections,
    /// A selection names a manifest that is not in the supplied manifest
    /// set (the ledger's "unknown manifest reference" refusal).
    ProfileSelectionUnresolvable { manifest_id: String },
    /// The referenced manifest's version does not satisfy the selection's
    /// declared constraint.
    ProfileManifestVersionUnsatisfied { manifest_id: String, constraint: String, found: String },
    /// A selected manifest requests a permission outside the profile
    /// ceiling — the ceiling cannot be exceeded.
    ProfilePermissionCeilingExceeded { manifest_id: String, permission: String },
    /// A selection's manifest claims a protected-kernel contract id
    /// (wrapped from the resolver's `PROTECTED_KERNEL_REPLACE`).
    ProfileProtectedContractRefused { manifest_id: String, contract_id: String },
    /// Two selections pin the same contract (via their manifests'
    /// `provider-of` sets) — never a silent last-wins.
    ProfileDuplicateContractPin { contract_id: String },
    /// The P7.06 resolver refused the composition; the source error's
    /// name is the specific reason (e.g. `MISSING_PROVIDER`,
    /// `CYCLIC_DEPENDENCY`, `PROTECTED_KERNEL_REPLACE`).
    ProfileResolutionRefused { source: ResolveError },
}

impl ProfileError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            ProfileError::ProfileJsonInvalid { .. } => "PROFILE_JSON_INVALID",
            ProfileError::ProfileNotAnObject => "PROFILE_NOT_AN_OBJECT",
            ProfileError::ProfileSchemaUnsupported { .. } => "PROFILE_SCHEMA_UNSUPPORTED",
            ProfileError::ProfileUnknownField { .. } => "PROFILE_UNKNOWN_FIELD",
            ProfileError::ProfileMissingField { .. } => "PROFILE_MISSING_FIELD",
            ProfileError::ProfileWrongType { .. } => "PROFILE_WRONG_TYPE",
            ProfileError::ProfileIdMalformed { .. } => "PROFILE_ID_MALFORMED",
            ProfileError::ProfileVersionConstraintInvalid { .. } => "PROFILE_VERSION_CONSTRAINT_INVALID",
            ProfileError::ProfileSelectionManifestIdMalformed { .. } => {
                "PROFILE_SELECTION_MANIFEST_ID_MALFORMED"
            }
            ProfileError::ProfileDuplicateSelection { .. } => "PROFILE_DUPLICATE_SELECTION",
            ProfileError::ProfileSelectionCountExcessive { .. } => "PROFILE_SELECTION_COUNT_EXCESSIVE",
            ProfileError::ProfilePermissionCeilingUnknown { .. } => "PROFILE_PERMISSION_CEILING_UNKNOWN",
            ProfileError::ProfilePermissionCeilingNeverGranted { .. } => {
                "PROFILE_PERMISSION_CEILING_NEVER_GRANTED"
            }
            ProfileError::ProfilePermissionCeilingDuplicate { .. } => {
                "PROFILE_PERMISSION_CEILING_DUPLICATE"
            }
            ProfileError::ControlAuthorityModeForbidden { .. } => "CONTROL_AUTHORITY_MODE_FORBIDDEN",
            ProfileError::ProfileEmptySelections => "PROFILE_EMPTY_SELECTIONS",
            ProfileError::ProfileSelectionUnresolvable { .. } => "PROFILE_SELECTION_UNRESOLVABLE",
            ProfileError::ProfileManifestVersionUnsatisfied { .. } => {
                "PROFILE_MANIFEST_VERSION_UNSATISFIED"
            }
            ProfileError::ProfilePermissionCeilingExceeded { .. } => {
                "PROFILE_PERMISSION_CEILING_EXCEEDED"
            }
            ProfileError::ProfileProtectedContractRefused { .. } => "PROFILE_PROTECTED_CONTRACT_REFUSED",
            ProfileError::ProfileDuplicateContractPin { .. } => "PROFILE_DUPLICATE_CONTRACT_PIN",
            ProfileError::ProfileResolutionRefused { .. } => "PROFILE_RESOLUTION_REFUSED",
        }
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::ProfileJsonInvalid { reason } => {
                write!(f, "profile is not valid JSON: {reason}")
            }
            ProfileError::ProfileNotAnObject => write!(f, "profile must be a JSON object"),
            ProfileError::ProfileSchemaUnsupported { found } => {
                write!(f, "unsupported profile-schema id '{found}' (expected '{PROFILE_SCHEMA_ID}')")
            }
            ProfileError::ProfileUnknownField { location, field } => {
                write!(f, "unknown field '{field}' at {location} (unknown fields are rejected, not warned)")
            }
            ProfileError::ProfileMissingField { field, location } => {
                write!(f, "missing required field '{field}' at {location}")
            }
            ProfileError::ProfileWrongType { location, field, expected, found } => {
                write!(f, "field '{field}' at {location} must be {expected}, found {found}")
            }
            ProfileError::ProfileIdMalformed { value, reason } => {
                write!(f, "profile id '{value}' is malformed: {reason}")
            }
            ProfileError::ProfileVersionConstraintInvalid { manifest_id, value, reason } => {
                write!(f, "selection '{manifest_id}' has an invalid version constraint '{value}': {reason}")
            }
            ProfileError::ProfileSelectionManifestIdMalformed { value, reason } => {
                write!(f, "selection manifest-id '{value}' is malformed: {reason}")
            }
            ProfileError::ProfileDuplicateSelection { manifest_id } => {
                write!(f, "manifest '{manifest_id}' is selected twice in the profile")
            }
            ProfileError::ProfileSelectionCountExcessive { count } => {
                write!(f, "profile carries {count} selections, above the composition cap of {MAX_COMPOSITION_MANIFESTS}")
            }
            ProfileError::ProfilePermissionCeilingUnknown { value } => {
                write!(f, "permission-ceiling entry '{value}' is not in the declared capability vocabulary (ids, not free text)")
            }
            ProfileError::ProfilePermissionCeilingNeverGranted { value } => {
                write!(f, "permission-ceiling entry '{value}' is in the never-granted set; a profile can never ceiling what no manifest may request")
            }
            ProfileError::ProfilePermissionCeilingDuplicate { value } => {
                write!(f, "permission '{value}' appears twice in permission-ceiling")
            }
            ProfileError::ControlAuthorityModeForbidden { value } => write!(
                f,
                "control-authority value '{value}' is forbidden: the control-authority mode is a deployment/per-site setting and a profile can never select it (report §5.8/§21.4); the only accepted profile posture is '{POSTURE_SIMULATION_SHADOW}'"
            ),
            ProfileError::ProfileEmptySelections => {
                write!(f, "profile has no active selections; a shell profile loads but never composes")
            }
            ProfileError::ProfileSelectionUnresolvable { manifest_id } => {
                write!(f, "profile selection names manifest '{manifest_id}', which is not in the supplied manifest set")
            }
            ProfileError::ProfileManifestVersionUnsatisfied { manifest_id, constraint, found } => {
                write!(
                    f,
                    "manifest '{manifest_id}' version {found} does not satisfy the profile's constraint '{constraint}'"
                )
            }
            ProfileError::ProfilePermissionCeilingExceeded { manifest_id, permission } => write!(
                f,
                "manifest '{manifest_id}' requests permission '{permission}' outside the profile's ceiling; a profile can never grant beyond its ceiling"
            ),
            ProfileError::ProfileProtectedContractRefused { manifest_id, contract_id } => write!(
                f,
                "selected manifest '{manifest_id}' claims the kernel-protected contract '{contract_id}'"
            ),
            ProfileError::ProfileDuplicateContractPin { contract_id } => write!(
                f,
                "two selections pin contract '{contract_id}'; a profile resolves providers explicitly, never by a silent last-wins"
            ),
            ProfileError::ProfileResolutionRefused { source } => {
                write!(f, "the profile's composition was refused by the resolver ({}): {source}", source.name())
            }
        }
    }
}

impl std::error::Error for ProfileError {}

// --- Parsing --------------------------------------------------------------------------

/// Parse and validate a profile from its JSON source text. Full-line `//`
/// comments are stripped first (the documented JSONC pattern for shipped
/// profiles with deferred, commented-out selections — see
/// `plugins/profiles/README.md`); oversize input is rejected before
/// parsing.
pub fn parse_profile(text: &str) -> Result<Profile, ProfileError> {
    if text.len() > MAX_PROFILE_BYTES {
        return Err(ProfileError::ProfileJsonInvalid {
            reason: format!("profile exceeds the {} byte limit", MAX_PROFILE_BYTES),
        });
    }
    let stripped = strip_full_line_comments(text);
    let value: Value = serde_json::from_str(&stripped)
        .map_err(|err| ProfileError::ProfileJsonInvalid { reason: err.to_string() })?;
    parse_profile_value(&value)
}

/// Strip FULL-LINE `//` comments only: a line whose first non-whitespace
/// characters are `//` is removed; nothing else is touched (no trailing
/// comments, no block comments) — the pattern stays deterministic and a
/// `//` inside a string value is never affected.
fn strip_full_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Parse and validate a profile from an already-parsed JSON value.
pub fn parse_profile_value(value: &Value) -> Result<Profile, ProfileError> {
    let obj = match value {
        Value::Object(map) => map,
        _ => return Err(ProfileError::ProfileNotAnObject),
    };

    // 1. Envelope first.
    let schema_id = required_string(obj, "profile-schema", "profile")?;
    if schema_id != PROFILE_SCHEMA_ID {
        return Err(ProfileError::ProfileSchemaUnsupported { found: schema_id });
    }

    // 2. Closed top-level key set.
    const ALLOWED_TOP_LEVEL: [&str; 8] = [
        "profile-schema",
        "profile-id",
        "name",
        "version",
        "description",
        "control-authority",
        "permission-ceiling",
        "selections",
    ];
    reject_unknown_fields(obj, &ALLOWED_TOP_LEVEL, "profile")?;
    for field in ALLOWED_TOP_LEVEL {
        if !obj.contains_key(field) {
            return Err(ProfileError::ProfileMissingField {
                field: field.to_string(),
                location: "profile".to_string(),
            });
        }
    }

    // 3. Identity.
    let profile_id = required_string(obj, "profile-id", "profile")?;
    validate_profile_id(&profile_id).map_err(|reason| ProfileError::ProfileIdMalformed {
        value: profile_id.clone(),
        reason,
    })?;
    let name = required_string(obj, "name", "profile")?;
    if name.is_empty() || name.len() > 256 {
        return Err(ProfileError::ProfileWrongType {
            location: "profile".to_string(),
            field: "name".to_string(),
            expected: "a non-empty string of at most 256 characters",
            found: "string",
        });
    }
    let version_text = required_string(obj, "version", "profile")?;
    let version = match SemVer::parse(&version_text) {
        Ok(version) => version,
        Err(_) => {
            return Err(ProfileError::ProfileWrongType {
                location: "profile".to_string(),
                field: "version".to_string(),
                expected: "a strict MAJOR.MINOR.PATCH version (no prerelease/build metadata in v1)",
                found: "string",
            })
        }
    };

    // 4. Description (reviewability: the profile says what it is for).
    let description = required_string(obj, "description", "profile")?;
    if description.is_empty() || description.len() > 2000 {
        return Err(ProfileError::ProfileWrongType {
            location: "profile".to_string(),
            field: "description".to_string(),
            expected: "a non-empty string of at most 2000 characters",
            found: "string",
        });
    }

    // 5. The control-authority posture — the mode lock (§5.8/§21.4).
    let posture_text = required_string(obj, "control-authority", "profile")?;
    let control_authority = ControlAuthorityPosture::parse_from_profile_data(&posture_text)?;

    // 6. The permission ceiling (closed vocabulary, sorted, deduplicated).
    let permission_ceiling = parse_ceiling(
        obj.get("permission-ceiling").ok_or_else(|| ProfileError::ProfileMissingField {
            field: "permission-ceiling".to_string(),
            location: "profile".to_string(),
        })?,
    )?;

    // 7. The ordered selections.
    let selections = parse_selections(
        obj.get("selections").ok_or_else(|| ProfileError::ProfileMissingField {
            field: "selections".to_string(),
            location: "profile".to_string(),
        })?,
    )?;
    if selections.len() > MAX_COMPOSITION_MANIFESTS {
        return Err(ProfileError::ProfileSelectionCountExcessive { count: selections.len() });
    }

    Ok(Profile {
        profile_id,
        name,
        version,
        description,
        control_authority,
        permission_ceiling,
        selections,
    })
}

/// Profile id: a single segment, lowercase ASCII `[a-z0-9-]`, starting
/// with a letter, at most 64 characters (e.g. `sim-lab`).
fn validate_profile_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("empty profile id".to_string());
    }
    if id.len() > 64 {
        return Err("profile id exceeds 64 characters".to_string());
    }
    let first = id.as_bytes()[0];
    if !first.is_ascii_lowercase() {
        return Err("profile id must start with a lowercase letter".to_string());
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("profile id may only contain lowercase letters, digits, and hyphens".to_string());
    }
    Ok(())
}

fn parse_ceiling(value: &Value) -> Result<Vec<String>, ProfileError> {
    let items = match value {
        Value::Array(items) => items,
        other => {
            return Err(ProfileError::ProfileWrongType {
                location: "permission-ceiling".to_string(),
                field: "permission-ceiling".to_string(),
                expected: "array of permission ids",
                found: json_type_name(other),
            })
        }
    };
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut out: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        let id = match item {
            Value::String(s) => s.clone(),
            other => {
                return Err(ProfileError::ProfileWrongType {
                    location: "permission-ceiling".to_string(),
                    field: "permission-ceiling".to_string(),
                    expected: "array of permission ids",
                    found: json_type_name(other),
                })
            }
        };
        if NEVER_GRANTED_PERMISSIONS.contains(&id.as_str()) {
            return Err(ProfileError::ProfilePermissionCeilingNeverGranted { value: id });
        }
        if !ALLOWED_PERMISSIONS.contains(&id.as_str()) {
            return Err(ProfileError::ProfilePermissionCeilingUnknown { value: id });
        }
        if seen.insert(id.clone(), ()).is_some() {
            return Err(ProfileError::ProfilePermissionCeilingDuplicate { value: id });
        }
        out.push(id);
    }
    out.sort();
    Ok(out)
}

fn parse_selections(value: &Value) -> Result<Vec<ProfileSelection>, ProfileError> {
    let items = match value {
        Value::Array(items) => items,
        other => {
            return Err(ProfileError::ProfileWrongType {
                location: "selections".to_string(),
                field: "selections".to_string(),
                expected: "array of selection objects",
                found: json_type_name(other),
            })
        }
    };
    let mut out: Vec<ProfileSelection> = Vec::with_capacity(items.len());
    let mut seen_ids: BTreeMap<String, ()> = BTreeMap::new();
    for (idx, item) in items.iter().enumerate() {
        let location = format!("selections[{idx}]");
        let obj = match item {
            Value::Object(map) => map,
            other => {
                return Err(ProfileError::ProfileWrongType {
                    location: location.clone(),
                    field: "selections".to_string(),
                    expected: "object with keys {manifest-id, version-constraint}",
                    found: json_type_name(other),
                })
            }
        };
        const ALLOWED_SELECTION_KEYS: [&str; 2] = ["manifest-id", "version-constraint"];
        reject_unknown_fields(obj, &ALLOWED_SELECTION_KEYS, &location)?;
        let manifest_id = required_string(obj, "manifest-id", &location)?;
        validate_plugin_id(&manifest_id).map_err(|reason| {
            ProfileError::ProfileSelectionManifestIdMalformed {
                value: manifest_id.clone(),
                reason,
            }
        })?;
        if seen_ids.insert(manifest_id.clone(), ()).is_some() {
            return Err(ProfileError::ProfileDuplicateSelection { manifest_id });
        }
        let constraint_text = required_string(obj, "version-constraint", &location)?;
        let version_constraint = ApiVersionRange::parse(&constraint_text).map_err(|err| {
            ProfileError::ProfileVersionConstraintInvalid {
                manifest_id: manifest_id.clone(),
                value: constraint_text.clone(),
                reason: err.reason,
            }
        })?;
        out.push(ProfileSelection { manifest_id, version_constraint });
    }
    Ok(out)
}

// --- Plan validation -------------------------------------------------------------------

/// Validate the profile against a supplied manifest set and produce the
/// composition plan (P7.15 step 3: "resolve its bundle graph, verify every
/// plugin's lifecycle state, and refuse to start a profile with an
/// unresolved or incompatible bundle" — resolution is implemented here;
/// lifecycle-state verification is the P7.07 machine's job once the
/// composition root registers the plan, and the P7.16 switch refuses
/// anything the lifecycle refuses).
///
/// Deterministic order: selections in declared order; ceiling checks
/// before resolution (fail-closed: a ceiling violation is reported even
/// if the composition would also fail elsewhere); explicit selections
/// built in selection order; the resolver's plan is its deterministic
/// output. Same inputs → identical plan.
pub fn validate_profile(profile: &Profile, manifests: &[Manifest]) -> Result<CompositionPlan, ProfileError> {
    if profile.selections.is_empty() {
        return Err(ProfileError::ProfileEmptySelections);
    }
    if manifests.len() > MAX_COMPOSITION_MANIFESTS {
        return Err(ProfileError::ProfileSelectionCountExcessive { count: manifests.len() });
    }

    // Index the manifest set by plugin id (BTreeMap: deterministic lookups).
    let mut by_id: BTreeMap<&str, &Manifest> = BTreeMap::new();
    for manifest in manifests {
        by_id.insert(manifest.plugin_id.as_str(), manifest);
    }

    // 1. Every selection resolves to a manifest, satisfies its version
    //    constraint, and stays under the permission ceiling (in declared
    //    selection order — deterministic first-violation reporting).
    let mut referenced: Vec<&Manifest> = Vec::with_capacity(profile.selections.len());
    for selection in &profile.selections {
        let manifest = match by_id.get(selection.manifest_id.as_str()) {
            Some(manifest) => *manifest,
            None => {
                return Err(ProfileError::ProfileSelectionUnresolvable {
                    manifest_id: selection.manifest_id.clone(),
                })
            }
        };
        if !selection.version_constraint.satisfied_by(manifest.version) {
            return Err(ProfileError::ProfileManifestVersionUnsatisfied {
                manifest_id: selection.manifest_id.clone(),
                constraint: selection.version_constraint.source().to_string(),
                found: manifest.version.to_string(),
            });
        }
        // The ceiling check: manifest permissions ⊆ profile ceiling. The
        // never-granted set cannot appear (the manifest validator already
        // refused it), so a plain membership check suffices.
        for permission in &manifest.permissions {
            if !profile.permits(permission) {
                return Err(ProfileError::ProfilePermissionCeilingExceeded {
                    manifest_id: manifest.plugin_id.clone(),
                    permission: permission.clone(),
                });
            }
        }
        referenced.push(manifest);
    }

    // 2. Build the explicit selections: each selection pins EVERY contract
    //    in its manifest's `provider-of` to that manifest. Two selections
    //    pinning one contract is a named error — never a silent last-wins.
    let mut selections_map = ExplicitSelections::new();
    for manifest in &referenced {
        let mut contracts: Vec<&String> = manifest.provider_of.iter().collect();
        contracts.sort();
        for contract in contracts {
            if selections_map.get(contract).is_some() {
                return Err(ProfileError::ProfileDuplicateContractPin {
                    contract_id: contract.clone(),
                });
            }
            selections_map.insert(contract.clone(), manifest.plugin_id.clone());
        }
    }

    // 3. Resolve through the P7.06 resolver (the protected-kernel guard,
    //    platform-API gate, missing-provider, cycle, and conflict checks
    //    all live there — wrapped, never weakened).
    let owned: Vec<Manifest> = referenced.into_iter().cloned().collect();
    match resolve(&owned, &selections_map) {
        Ok(plan) => Ok(plan),
        Err(ResolveError::ProtectedKernelReplace { plugin_id, contract_id }) => {
            Err(ProfileError::ProfileProtectedContractRefused { manifest_id: plugin_id, contract_id })
        }
        Err(source) => Err(ProfileError::ProfileResolutionRefused { source }),
    }
}

// --- Shared parsing helpers (the manifest.rs style, local copies) ----------------------

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
) -> Result<String, ProfileError> {
    match obj.get(field) {
        None => Err(ProfileError::ProfileMissingField {
            field: field.to_string(),
            location: location.to_string(),
        }),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(ProfileError::ProfileWrongType {
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
) -> Result<(), ProfileError> {
    let mut unknown: Vec<String> = obj
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    match unknown.first() {
        None => Ok(()),
        Some(field) => Err(ProfileError::ProfileUnknownField {
            location: location.to_string(),
            field: field.clone(),
        }),
    }
}
