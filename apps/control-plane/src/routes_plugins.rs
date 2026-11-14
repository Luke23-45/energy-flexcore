// P7.25 — The plugin-inventory read (`GET /api/v1/plugins`): the operator
//         console's plugin seam (report §20.1 item 10, §20.2; ledger
//         `ledger_phase_7.md` §P7.25). ADDITIVE — no existing route is
//         modified; the path is not one of the 16 frozen §15.2 candidate
//         resources and touches no dispatch state (the recorded
//         P4.36/P5.31/P6.06 additive-path rule).
//
// AUTH (the recorded-drift pattern, the `authorize_live_stream` P5.25 and
// `get_edge_status` P6.25 precedent): the ledger's action id for this
// surface would be a new closed-enum variant (`plugin.inventory.read`),
// but the `Action` vocabulary is a CLOSED enum owned by
// `crates/flexcore-authz` (outside this wave's file ownership). The route
// therefore evaluates `telemetry.read` — already tenant-scoped and granted
// to ALL roles by the phase policy sets — as the interim deny-by-default
// gate; the single swap point is this call site. The caller's tenant is
// derived from the verified claims (§16.2); the request carries NO tenant
// field.
//
// DATA SOURCE (the honest seam, exactly the P6.25 `EdgeStatusSource`
// pattern): the control plane holds NO running plugin host — the kernel
// is a library, not a service, and no host reporter is composed in 0.x.
// The surface is therefore served by the [`PluginInventorySource`] seam:
// the shipped default [`RecordedShippedInventory`] serves the RECORDED
// inventory of the shipped manifest/profile files
// (`plugins/manifests/*.manifest.json`, `plugins/profiles/*.profile.json`,
// recorded verbatim 2026-11-13), and every RUNTIME fact (lifecycle state,
// health) renders as the explicit gap `NOT_REPORTED` / `None` — never a
// plausible "ready"/"healthy". A deployment composes a real source (its
// host/health forwarder) via [`AppState::with_plugins_inventory`].
//
// READ-ONLY SURFACE: there is NO endpoint here (or anywhere on the
// plugin surface) that activates, deactivates, upgrades, or switches
// anything. The console's profile-switch affordance records a REQUEST
// locally (the P6.25 display-only command-path rule); composition changes
// take effect ONLY through the reviewed profile-file edit + re-composition
// procedure (P7.16), which starts OFF this console.

use std::fmt;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use flexcore_authz::action::Action;
use flexcore_domain::tenant_context::TenantContext;

use crate::auth::{authorize_tenant_scoped, CallerExtractor};
use crate::dto::DataEnvelope;
use crate::dto_plugins::{
    ConfigPropertyDto, PluginHealthDto, PluginInventoryDto, PluginRecordDto, ProfileRecordDto,
};
use crate::error::ApiError;
use crate::state::AppState;

// --- The closed gap/state vocabulary ---------------------------------------------

/// The explicit gap rendered for a runtime state no composed source
/// reports (the P6.25 `NOT_REPORTED` pattern). Never silently mapped onto
/// a plausible lifecycle state.
pub const PLUGIN_STATE_NOT_REPORTED: &str = "NOT_REPORTED";

/// Composition state of a recorded profile whose active selections
/// resolve against their manifests.
pub const COMPOSITION_STATE_COMPOSES: &str = "COMPOSES";

/// Composition state of a recorded profile that loads and is reviewable
/// but composes nothing (`PROFILE_EMPTY_SELECTIONS` at plan validation).
pub const COMPOSITION_STATE_DEFERRED_SHELL: &str = "DEFERRED_SHELL";

/// The honest surface note (verbatim in every response).
pub const PLUGINS_INVENTORY_NOTE: &str = "the RECORDED inventory of the shipped plugin \
    manifests and composition profiles (plugins/manifests, plugins/profiles — recorded \
    2026-11-13), NOT a live kernel/host view: the control plane holds no running plugin \
    host, so every lifecycle state renders NOT_REPORTED and health renders as an explicit \
    gap unless a deployment composes a PluginInventorySource. The three seed built-in \
    providers the profiles select (builtin.simulator-telemetry, builtin.highs-optimizer, \
    builtin.pandapower-grid) are registry seed ids with no shipped manifest files and are \
    therefore not plugin records here. This surface is READ-ONLY: no activate, deactivate, \
    upgrade, or profile-switch endpoint exists; composition changes take effect ONLY \
    through the reviewed profile-file edit + re-composition procedure (P7.16). Plugin \
    outputs are candidates, never authorizations (ADR-010).";

// --- The source-level records ------------------------------------------------------

/// The lifecycle/health facts of one plugin as a composed source reports
/// them (runtime observations; every field optional — absence is the gap).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginRuntimeReport {
    /// The lifecycle state as reported (a `lifecycle.rs::LifecycleState`
    /// machine name). `None` = not reported.
    pub lifecycle_state: Option<String>,
    /// The health state (`HEALTHY` / `DEGRADED_<reason>` / `FAILING_<reason>`
    /// — the P7.20 closed vocabulary). `None` = not reported.
    pub health_state: Option<String>,
    /// The readiness (`UNOBSERVED` / `PROBE_FAILED` / `READY`).
    pub readiness: Option<String>,
    /// The verbatim basis note of the health report.
    pub health_note: Option<String>,
}

/// One declared configuration property of a plugin's embedded schema
/// (name + type + description — a schema record, never a value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProperty {
    pub name: String,
    pub property_type: String,
    pub description: String,
}

/// One plugin of the inventory (the metadata record + optional runtime
/// report).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRecord {
    pub plugin_id: String,
    pub version: String,
    pub class: String,
    pub provider_of: Vec<String>,
    pub consumes: Vec<String>,
    pub permissions: Vec<String>,
    pub license: String,
    pub description: String,
    /// The declared configuration properties (schema summary — no values).
    pub config_properties: Vec<ConfigProperty>,
    /// The required property names of the embedded schema.
    pub config_required: Vec<String>,
    /// The provenance of this record (the shipped file path or the
    /// composed source's own note).
    pub record_note: String,
    /// The runtime report, when a source supplies one (`None` = gap).
    pub runtime: Option<PluginRuntimeReport>,
}

/// One composition profile of the inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRecord {
    pub profile_id: String,
    pub version: String,
    pub control_authority: String,
    pub permission_ceiling: Vec<String>,
    pub active_selections: Vec<String>,
    pub composition_state: String,
    pub state_note: String,
}

/// The inventory snapshot a composed source serves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginInventorySnapshot {
    pub plugins: Vec<PluginRecord>,
    pub profiles: Vec<ProfileRecord>,
}

/// Errors of the inventory-source seam (named; mapped to infrastructure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginInventoryError {
    /// The composed source failed (reason opaque/human-safe).
    Backend { reason: String },
}

impl fmt::Display for PluginInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginInventoryError::Backend { reason } => {
                write!(f, "the plugin-inventory source failed: {reason}")
            }
        }
    }
}

impl std::error::Error for PluginInventoryError {}

/// The plugin-inventory source seam (P7.25): the deployment composes its
/// host/health forwarder (whatever carries the host's plugin facts); the
/// shipped default serves the RECORDED shipped inventory with every
/// runtime fact an explicit gap (fail-closed honesty, never a fake
/// "all ready and healthy").
pub trait PluginInventorySource: Send + Sync {
    /// The inventory snapshot. The `ctx` is the caller's server-derived
    /// tenant context (for sources that partition their records); the
    /// shipped default serves the deployment-recorded set regardless of
    /// tenant (plugin composition is per deployment, report §5.8 — the
    /// honest scoping statement is in the surface note).
    fn inventory(&self, ctx: &TenantContext) -> Result<PluginInventorySnapshot, PluginInventoryError>;
}

/// The shipped default: the RECORDED inventory of the shipped
/// manifest/profile files, recorded verbatim 2026-11-13, with NO runtime
/// report composed — lifecycle `NOT_REPORTED`, health `None`, for every
/// plugin. This is an honest record of what SHIPS, never a claim about
/// what RUNS.
#[derive(Debug, Default, Clone, Copy)]
pub struct RecordedShippedInventory;

/// Builds a recorded plugin record from a shipped manifest file (no
/// runtime report — the gap). `config_properties` entries are
/// `(name, type, description)` triples recorded verbatim from the
/// manifest's embedded schema; `config_required` is the schema's required
/// list. Values are NOT recorded anywhere — a manifest schema carries
/// declarations, and the inventory serves no configuration VALUES.
fn recorded_plugin(
    plugin_id: &str,
    version: &str,
    class: &str,
    provider_of: &[&str],
    consumes: &[&str],
    permissions: &[&str],
    license: &str,
    description: &str,
    config_properties: &[(&str, &str, &str)],
    config_required: &[&str],
    manifest_file: &str,
) -> PluginRecord {
    PluginRecord {
        plugin_id: plugin_id.to_string(),
        version: version.to_string(),
        class: class.to_string(),
        provider_of: provider_of.iter().map(|s| (*s).to_string()).collect(),
        consumes: consumes.iter().map(|s| (*s).to_string()).collect(),
        permissions: permissions.iter().map(|s| (*s).to_string()).collect(),
        license: license.to_string(),
        description: description.to_string(),
        config_properties: config_properties
            .iter()
            .map(|(name, ty, desc)| ConfigProperty {
                name: (*name).to_string(),
                property_type: (*ty).to_string(),
                description: (*desc).to_string(),
            })
            .collect(),
        config_required: config_required.iter().map(|s| (*s).to_string()).collect(),
        record_note: format!("recorded from {manifest_file} (shipped manifest file)"),
        runtime: None,
    }
}

/// Builds a recorded profile record from a shipped profile file.
fn recorded_profile(
    profile_id: &str,
    version: &str,
    control_authority: &str,
    permission_ceiling: &[&str],
    active_selections: &[&str],
    composition_state: &str,
    state_note: &str,
) -> ProfileRecord {
    ProfileRecord {
        profile_id: profile_id.to_string(),
        version: version.to_string(),
        control_authority: control_authority.to_string(),
        permission_ceiling: permission_ceiling.iter().map(|s| (*s).to_string()).collect(),
        active_selections: active_selections.iter().map(|s| (*s).to_string()).collect(),
        composition_state: composition_state.to_string(),
        state_note: state_note.to_string(),
    }
}

impl PluginInventorySource for RecordedShippedInventory {
    fn inventory(
        &self,
        _ctx: &TenantContext,
    ) -> Result<PluginInventorySnapshot, PluginInventoryError> {
        // The records below are the shipped files' contents, recorded
        // verbatim (read 2026-11-13). A drifted file and a drifted record
        // are a review failure, not a runtime hazard: the kernel
        // validates the files at composition time.
        let plugins = vec![
            recorded_plugin(
                "neryva.sim-builtin-ess",
                "0.1.0",
                "builtin-rust",
                &["ess.simulation"],
                &["telemetry.edge-sample"],
                &["telemetry.read"],
                "Apache-2.0",
                "Deterministic built-in battery energy-storage simulator provider \
                 (Class A; simulation only).",
                &[
                    (
                        "rated-power-mw",
                        "number",
                        "Rated active power of the simulated ESS in MW (canonical \
                         units, P > 0 = export).",
                    ),
                    (
                        "soc-init",
                        "number",
                        "Initial state of charge as a fraction in [0,1] (API-boundary \
                         SOC convention).",
                    ),
                ],
                &["rated-power-mw"],
                "plugins/manifests/builtin-ess-provider.manifest.json",
            ),
            recorded_plugin(
                "acme.forecast-worker",
                "0.3.1",
                "grpc-worker",
                &["forecast.day-ahead"],
                &["telemetry.history"],
                &["telemetry.read", "forecast.publish"],
                "Apache-2.0",
                "Out-of-process Class C forecast worker (illustrative third-party \
                 plugin for the SDK examples).",
                &[
                    ("horizon-hours", "integer", "Forecast horizon in hours."),
                    (
                        "region",
                        "string",
                        "Market region id the worker forecasts for.",
                    ),
                ],
                &["horizon-hours"],
                "plugins/manifests/forecast-worker.manifest.json",
            ),
            recorded_plugin(
                "neryva.optimizer-wasm-lp",
                "0.2.0",
                "wasm-component",
                &["optimization.lp"],
                &["portfolio.read"],
                &["schedule.propose"],
                "Apache-2.0",
                "Class B WASM component providing LP optimization (candidate \
                 schedules only; never dispatch authority).",
                &[(
                    "gap-tolerance",
                    "number",
                    "Relative optimality gap tolerance for LP solves.",
                )],
                &[],
                "plugins/manifests/optimizer-wasm.manifest.json",
            ),
        ];
        let profiles = vec![
            recorded_profile(
                "research-gnn",
                "0.5.0",
                "simulation-shadow",
                &["telemetry.read", "asset.read"],
                &["builtin.pandapower-grid", "builtin.simulator-telemetry"],
                COMPOSITION_STATE_COMPOSES,
                "composes the grid-model and telemetry seed built-ins; the research \
                 worker itself is Phase 8; ceiling withholds the proposal permissions \
                 (no dispatch authority — recorded DEFAULT, ADR-010)",
            ),
            recorded_profile(
                "research-marl",
                "0.5.0",
                "simulation-shadow",
                &["telemetry.read", "asset.read"],
                &["builtin.pandapower-grid", "builtin.simulator-telemetry"],
                COMPOSITION_STATE_COMPOSES,
                "same shape as research-gnn for the MARL line; ceiling withholds the \
                 proposal permissions (no dispatch authority — recorded DEFAULT, ADR-010)",
            ),
            recorded_profile(
                "sim-lab",
                "0.5.0",
                "simulation-shadow",
                &["telemetry.read", "asset.read", "schedule.propose"],
                &[
                    "builtin.simulator-telemetry",
                    "builtin.highs-optimizer",
                    "builtin.pandapower-grid",
                    "neryva.sim-builtin-ess",
                ],
                COMPOSITION_STATE_COMPOSES,
                "composes the three seed built-ins plus the shipped ESS manifest; \
                 mock-market / replay entries deferred (commented out — the documented \
                 pattern)",
            ),
            recorded_profile(
                "site-edge",
                "0.5.0",
                "simulation-shadow",
                &["telemetry.read", "asset.read"],
                &[],
                COMPOSITION_STATE_DEFERRED_SHELL,
                "a DEFERRED SHELL: loads and is reviewable, composes NOTHING \
                 (PROFILE_EMPTY_SELECTIONS at plan validation); every entry is \
                 commented out because none of these roles has a plugin manifest yet, \
                 and the command receiver is protected kernel that can never be a \
                 plugin (P6.29)",
            ),
            recorded_profile(
                "vpp-operator",
                "0.5.0",
                "simulation-shadow",
                &[
                    "telemetry.read",
                    "asset.read",
                    "forecast.publish",
                    "schedule.propose",
                    "dispatch.propose",
                    "dispatch.validate.request",
                ],
                &[
                    "builtin.simulator-telemetry",
                    "builtin.highs-optimizer",
                    "builtin.pandapower-grid",
                ],
                COMPOSITION_STATE_COMPOSES,
                "composes the three seed built-ins; forecast/WASM examples and \
                 console/scheduler/connector entries deferred (commented out — the \
                 documented pattern); dispatch.authorize and dispatch.issue are never \
                 grantable and the control-authority mode stays a deployment setting",
            ),
        ];
        Ok(PluginInventorySnapshot { plugins, profiles })
    }
}

// --- The route -----------------------------------------------------------------

/// Builds the plugin-inventory router (merged into the stage-1 router).
/// GET only — a narrow READ surface; no mutation endpoint exists and none
/// may be added here (the composition change path is the reviewed
/// profile-file edit + re-composition procedure, P7.16).
pub fn plugins_router() -> Router<AppState> {
    Router::new().route("/api/v1/plugins", get(get_plugin_inventory))
}

/// Composes the DTO from the snapshot: metadata flows verbatim; an absent
/// runtime report renders the named gaps (never plausible states).
fn compose_plugin_dto(record: &PluginRecord) -> PluginRecordDto {
    let (lifecycle_state, health) = match record.runtime.as_ref() {
        None => (PLUGIN_STATE_NOT_REPORTED.to_string(), None),
        Some(runtime) => {
            let health = match (
                runtime.health_state.as_ref(),
                runtime.readiness.as_ref(),
            ) {
                (Some(health_state), Some(readiness)) => Some(PluginHealthDto {
                    health_state: health_state.clone(),
                    readiness: readiness.clone(),
                    note: runtime.health_note.clone().unwrap_or_else(|| {
                        "no basis note was supplied by the composed health source".to_string()
                    }),
                }),
                // A partial runtime report is still the gap on the health
                // object (never a half-filled plausible snapshot).
                _ => None,
            };
            (
                runtime
                    .lifecycle_state
                    .clone()
                    .unwrap_or_else(|| PLUGIN_STATE_NOT_REPORTED.to_string()),
                health,
            )
        }
    };
    PluginRecordDto {
        plugin_id: record.plugin_id.clone(),
        version: record.version.clone(),
        class: record.class.clone(),
        provider_of: record.provider_of.clone(),
        consumes: record.consumes.clone(),
        permissions: record.permissions.clone(),
        license: record.license.clone(),
        description: record.description.clone(),
        config_properties: record
            .config_properties
            .iter()
            .map(|p| ConfigPropertyDto {
                name: p.name.clone(),
                property_type: p.property_type.clone(),
                description: p.description.clone(),
            })
            .collect(),
        config_required: record.config_required.clone(),
        record_note: record.record_note.clone(),
        lifecycle_state,
        health,
    }
}

/// The plugin-inventory read (P7.25). Auth: every authenticated role,
/// tenant-scoped (the recorded-drift interim gate above; the swap point
/// for the closed-enum `plugin.inventory.read` action). The inventory is
/// the deployment's recorded plugin set — plugin composition is per
/// deployment (report §5.8), not tenant-partitioned business data; the
/// statement is in the verbatim surface note.
#[utoipa::path(
    get,
    path = "/api/v1/plugins",
    responses(
        (status = 200, description = "The RECORDED plugin inventory (shipped manifests/profiles) plus whatever the composed source reports; absent runtime facts render as explicit gaps (NOT_REPORTED / null health)", body = DataEnvelope<PluginInventoryDto>),
        (status = 401, description = "Unauthenticated caller"),
        (status = 403, description = "The role is not granted the interim inventory-read action"),
        (status = 500, description = "The composed inventory source failed (infrastructure — honest, never a fake success)"),
    )
)]
async fn get_plugin_inventory(
    State(state): State<AppState>,
    caller: CallerExtractor,
) -> Result<Json<DataEnvelope<PluginInventoryDto>>, ApiError> {
    // AUTH: the recorded-drift interim gate (deny-by-default; the swap
    // point for the closed-enum `plugin.inventory.read` action).
    authorize_tenant_scoped(&state.policies, &caller.0, Action::TelemetryRead)?;
    let ctx = caller.0.ctx.clone();
    let snapshot = state
        .plugins_inventory
        .inventory(&ctx)
        .map_err(|error| {
            tracing::error!(
                %error,
                "the plugin-inventory source FAILED — the inventory is not served as a fake success"
            );
            ApiError::infrastructure()
        })?;
    let mut plugins: Vec<PluginRecordDto> =
        snapshot.plugins.iter().map(compose_plugin_dto).collect();
    // Sorted by plugin id (deterministic output; the source may already
    // sort, the route does not rely on it).
    plugins.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
    let mut profiles: Vec<ProfileRecordDto> = snapshot
        .profiles
        .iter()
        .map(|p| ProfileRecordDto {
            profile_id: p.profile_id.clone(),
            version: p.version.clone(),
            control_authority: p.control_authority.clone(),
            permission_ceiling: p.permission_ceiling.clone(),
            active_selections: p.active_selections.clone(),
            composition_state: p.composition_state.clone(),
            state_note: p.state_note.clone(),
        })
        .collect();
    profiles.sort_by(|a, b| a.profile_id.cmp(&b.profile_id));
    Ok(Json(DataEnvelope {
        data: PluginInventoryDto {
            plugins,
            profiles,
            inventory_note: PLUGINS_INVENTORY_NOTE.to_string(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};

    fn tenant_ctx() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes())),
                subject: "p7.25-unit-test".to_string(),
            },
        ))
    }

    #[test]
    fn the_recorded_config_summary_is_schema_only_never_values() {
        let snapshot = RecordedShippedInventory
            .inventory(&tenant_ctx())
            .expect("the recorded default never fails");
        let ess = snapshot
            .plugins
            .iter()
            .find(|p| p.plugin_id == "neryva.sim-builtin-ess")
            .expect("the ESS record");
        let names: Vec<&str> = ess
            .config_properties
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["rated-power-mw", "soc-init"]);
        assert_eq!(ess.config_required, vec!["rated-power-mw"]);
        let wasm = snapshot
            .plugins
            .iter()
            .find(|p| p.plugin_id == "neryva.optimizer-wasm-lp")
            .expect("the WASM record");
        assert_eq!(
            wasm.config_required,
            Vec::<String>::new(),
            "the WASM manifest declares an empty required list"
        );
    }

    #[test]
    fn the_recorded_inventory_serves_the_shipped_set_with_runtime_gaps() {
        let snapshot = RecordedShippedInventory
            .inventory(&tenant_ctx())
            .expect("the recorded default never fails");
        // The three shipped manifests, verbatim ids.
        let ids: Vec<&str> = snapshot
            .plugins
            .iter()
            .map(|p| p.plugin_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["acme.forecast-worker", "neryva.optimizer-wasm-lp", "neryva.sim-builtin-ess"]
        );
        // Every plugin carries NO runtime report: the gap, not a state.
        for plugin in &snapshot.plugins {
            assert!(plugin.runtime.is_none(), "{} must not fabricate a runtime report", plugin.plugin_id);
        }
        // The five shipped profiles, all locked to simulation-shadow.
        assert_eq!(snapshot.profiles.len(), 5);
        for profile in &snapshot.profiles {
            assert_eq!(profile.control_authority, "simulation-shadow");
        }
        // site-edge is the recorded deferred shell: no active selections.
        let site_edge = snapshot
            .profiles
            .iter()
            .find(|p| p.profile_id == "site-edge")
            .expect("site-edge is a shipped profile");
        assert_eq!(site_edge.composition_state, COMPOSITION_STATE_DEFERRED_SHELL);
        assert!(site_edge.active_selections.is_empty());
        // The research ceilings withhold the proposal permissions
        // (no dispatch authority — the recorded DEFAULT).
        for research in ["research-gnn", "research-marl"] {
            let profile = snapshot
                .profiles
                .iter()
                .find(|p| p.profile_id == research)
                .expect("the research profile ships");
            assert_eq!(
                profile.permission_ceiling,
                vec!["telemetry.read", "asset.read"]
            );
        }
    }

    #[test]
    fn absent_runtime_reports_render_the_named_gaps_never_plausible_states() {
        let record = recorded_plugin(
            "x.y",
            "1.0.0",
            "wasm-component",
            &[],
            &[],
            &[],
            "Apache-2.0",
            "gap test",
            &[],
            &[],
            "test",
        );
        let dto = compose_plugin_dto(&record);
        assert_eq!(dto.lifecycle_state, "NOT_REPORTED");
        assert!(dto.health.is_none());
        // A partial runtime report (lifecycle only) is still the health
        // gap — never a half-filled plausible snapshot.
        let partial = PluginRecord {
            runtime: Some(PluginRuntimeReport {
                lifecycle_state: Some("READY".to_string()),
                health_state: None,
                readiness: None,
                health_note: None,
            }),
            ..record.clone()
        };
        let dto = compose_plugin_dto(&partial);
        assert_eq!(dto.lifecycle_state, "READY");
        assert!(dto.health.is_none());
        // A full runtime report passes through VERBATIM.
        let full = PluginRecord {
            runtime: Some(PluginRuntimeReport {
                lifecycle_state: Some("READY".to_string()),
                health_state: Some("DEGRADED_ERROR_RATE".to_string()),
                readiness: Some("READY".to_string()),
                health_note: Some("basis note".to_string()),
            }),
            ..record
        };
        let dto = compose_plugin_dto(&full);
        assert_eq!(dto.lifecycle_state, "READY");
        let health = dto.health.expect("a full report composes the health object");
        assert_eq!(health.health_state, "DEGRADED_ERROR_RATE");
        assert_eq!(health.readiness, "READY");
        assert_eq!(health.note, "basis note");
    }

    #[test]
    fn the_surface_note_names_the_read_only_and_gap_rules() {
        assert!(PLUGINS_INVENTORY_NOTE.contains("NOT_REPORTED"));
        assert!(PLUGINS_INVENTORY_NOTE.contains("READ-ONLY"));
        assert!(PLUGINS_INVENTORY_NOTE.contains("no activate, deactivate, upgrade, or profile-switch endpoint"));
        assert!(PLUGINS_INVENTORY_NOTE.contains("candidates, never authorizations"));
    }
}
