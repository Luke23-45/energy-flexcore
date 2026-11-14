// P7.25 — The `/api/v1/plugins` plugin-inventory read DTOs (report §20.1
//         item 10; ledger `ledger_phase_7.md` §P7.25). ADDITIVE — no
//         existing DTO is modified.
//
// HONESTY CONTRACT (the P6.25 EdgeStatusDto pattern, decision record
// `25_31_32_console_clock_decommission.md` §1, applied here): every
// runtime fact this surface does not have a composed source for is an
// HONEST GAP — `lifecycle_state: NOT_REPORTED` and `health: null` mean
// "not reported", never "ready", "healthy", or "stopped". The console
// renders the gaps and the verbatim notes; it never upgrades a gap into
// a plausible value. The lifecycle-state and health vocabularies rendered
// through these DTOs are the BUILT system's closed vocabularies
// (`crates/flexcore-kernel/src/lifecycle.rs::LifecycleState::name` —
// DISCOVERED..FAILED — and `apps/plugin-host/src/health.rs::HealthState` —
// HEALTHY / DEGRADED_<reason> / FAILING_<reason>, readiness
// UNOBSERVED / PROBE_FAILED / READY), rendered VERBATIM, never prettified.
//
// NO SECRET VALUE (and no secret LOCATION beyond the recorded manifest's
// own reference strings) exists anywhere on this surface: the inventory
// is plugin metadata only. There is also NO mutation endpoint on this
// surface — no activate, deactivate, upgrade, or profile-switch route
// exists (the console's switch affordance records a REQUEST only, the
// P6.25 display-only command-path rule).

use serde::Serialize;
use utoipa::ToSchema;

/// One plugin of the recorded inventory (P7.25). Every field is plugin
/// METADATA from the recorded manifest/profile sources the composed
/// [`crate::routes_plugins::PluginInventorySource`] serves — never a
/// runtime observation unless a health source is composed.
#[derive(Debug, Serialize, ToSchema)]
pub struct PluginRecordDto {
    /// The plugin id (`<namespace>.<name>`, the manifest's `plugin-id`).
    pub plugin_id: String,
    /// The manifest version (strict SemVer core, verbatim).
    pub version: String,
    /// The §5.4 class, verbatim: `builtin-rust` | `wasm-component` | `grpc-worker`.
    pub class: String,
    /// The contract ids the manifest declares in `provider-of` (sorted).
    pub provider_of: Vec<String>,
    /// The contract ids the manifest declares in `consumes` (sorted).
    pub consumes: Vec<String>,
    /// The requested §5.7 permission ids (sorted; ids, never free text).
    pub permissions: Vec<String>,
    /// The manifest's REQUIRED license declaration (a plugin with no
    /// declared license cannot be distributed).
    pub license: String,
    /// The manifest's description (verbatim).
    pub description: String,
    /// The declared configuration properties (names + types + descriptions
    /// — the config SUMMARY; no value exists to display: the inventory
    /// serves no configuration VALUES at all, only the schema surface).
    pub config_properties: Vec<ConfigPropertyDto>,
    /// The required property names of the embedded schema (verbatim).
    pub config_required: Vec<String>,
    /// WHERE this record is recorded from (the shipped file path or the
    /// composed source's own record note — the provenance of the record,
    /// not a claim about a running instance).
    pub record_note: String,
    /// The lifecycle state (the §5.6 closed vocabulary, rendered VERBATIM:
    /// DISCOVERED / VERIFIED / COMPATIBLE / CONFIGURED / STARTED / READY /
    /// DRAINING / STOPPED / REJECTED / FAILED). With no host reporter
    /// composed this is `NOT_REPORTED` — the gap, named, never read as a
    /// serving state.
    pub lifecycle_state: String,
    /// The health snapshot, when a health source is composed. `None` =
    /// not reported — the gap, never "healthy".
    pub health: Option<PluginHealthDto>,
}

/// The health snapshot of one plugin (the P7.20 closed vocabularies,
/// rendered VERBATIM — the console never prettifies them into
/// healthy-looking synonyms).
#[derive(Debug, Serialize, ToSchema)]
pub struct PluginHealthDto {
    /// The P7.20 health state: `HEALTHY` | `DEGRADED_<reason>` |
    /// `FAILING_<reason>` (reasons: UNVERIFIED, UNRESPONSIVE,
    /// MEMORY_LIMIT, FUEL_EXHAUSTED, DEADLINE, OUTPUT_LIMIT, CALL_RATE,
    /// ERROR_RATE, BINDINGS — the closed vocabulary verbatim).
    pub health_state: String,
    /// The P7.20 readiness: `UNOBSERVED` | `PROBE_FAILED` | `READY`.
    pub readiness: String,
    /// The verbatim note from the composed health source (its basis and
    /// any limitation statement).
    pub note: String,
}

/// One declared configuration property of the manifest's embedded schema
/// (the config SUMMARY: names and types only — never a VALUE; the v1
/// subset carries no default-value field to leak and no secret VALUE is
/// served anywhere on this surface, §5.7).
#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigPropertyDto {
    /// The property name (the manifest's declared key).
    pub name: String,
    /// The declared type, verbatim: `string` | `number` | `integer` |
    /// `boolean` | `array` | `object`.
    pub property_type: String,
    /// The declared description (verbatim; may carry units and sign
    /// conventions — it is manifest text).
    pub description: String,
}

/// One composition profile of the recorded inventory (`flexcore-profile-v1`).
#[derive(Debug, Serialize, ToSchema)]
pub struct ProfileRecordDto {
    /// The profile id (the shipped five: sim-lab, vpp-operator, site-edge,
    /// research-gnn, research-marl — the set is open as data vocabulary).
    pub profile_id: String,
    /// The profile version (verbatim).
    pub version: String,
    /// The control-authority posture — with the shipped loader this can
    /// ONLY be `simulation-shadow` (the mode lock: a profile can never
    /// select or raise the control-authority mode, report §5.8/§21.4).
    pub control_authority: String,
    /// The permission ceiling (sorted §5.7 ids; a maximum, never a grant).
    pub permission_ceiling: Vec<String>,
    /// The manifest ids the profile actively selects (sorted; the deferred
    /// commented-out entries are NOT selections and never appear here).
    pub active_selections: Vec<String>,
    /// The composition state, verbatim from the recorded review:
    /// `COMPOSES` (the active selections resolve against their manifests)
    /// or `DEFERRED_SHELL` (loads and is reviewable, composes nothing —
    /// `PROFILE_EMPTY_SELECTIONS` at plan validation).
    pub composition_state: String,
    /// The verbatim state note from the recorded review (what the profile
    /// composes and why anything is deferred).
    pub state_note: String,
}

/// The plugin-inventory read (P7.25). The shape mirrors the composed
/// [`crate::routes_plugins::PluginInventorySnapshot`] — the RECORDED
/// inventory of the shipped manifests/profiles plus whatever the
/// deployment's composed sources report; absent runtime sources render as
/// the explicit gaps named above.
#[derive(Debug, Serialize, ToSchema)]
pub struct PluginInventoryDto {
    /// The recorded plugin records (sorted by plugin id).
    pub plugins: Vec<PluginRecordDto>,
    /// The recorded profile records (sorted by profile id).
    pub profiles: Vec<ProfileRecordDto>,
    /// The honest surface note (what this inventory is and is not —
    /// verbatim in every response).
    pub inventory_note: String,
}
