// P7.25 — API contract tests for the `GET /api/v1/plugins` plugin-inventory
//         read (report §20.1 item 10, §20.2; ledger
//         `ledger_phase_7.md` §P7.25) on the Test-variant backend.
//
// Composition rule (no network, no database — the P1.17 test pattern; the
// api_alarms.rs pattern): the app state is built with the flexcore-ports
// fakes, a fixed identity resolver, and the cumulative Phase 5 policy set.
// The plugin-inventory seam stays on its shipped default
// (`RecordedShippedInventory`) except in the custom-source test, and
// requests go through the REAL router via `tower::ServiceExt::oneshot`.
//
// Recorded assertions covered here:
// - **Fail-closed auth**: unauthenticated is 401; an empty policy set is
//   403 (deny-by-default — the recorded-drift interim gate).
// - **The recorded inventory**: the three shipped manifests and the five
//   shipped profiles, sorted, with the verbatim metadata (ids, versions,
//   classes, permissions, licenses) — and the honest surface note.
// - **Runtime honesty**: with the shipped default source, EVERY plugin's
//   lifecycle state is `NOT_REPORTED` and health is `null` — the gap, never
//   a plausible "ready"/"healthy" (the P6.25 rule applied to plugins).
// - **The mode lock**: every profile's control-authority is
//   `simulation-shadow` — never a control-capable mode.
// - **The deferred shell**: `site-edge` is `DEFERRED_SHELL` with no active
//   selections.
// - **A composed source**: a test source supplying runtime facts renders
//   them VERBATIM (lifecycle READY, health DEGRADED_ERROR_RATE); a partial
//   report leaves the health object as the gap.
// - **READ-ONLY surface**: no plugin mutation path exists in the OpenAPI
//   document (GET /api/v1/plugins only) and a live POST probe is refused.
//
// UNRUN — written, never executed in this loop (the ENTRY_NOTE bans); the
// human build pass runs `cargo test -p flexcore-control-plane --test
// api_plugins`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase5_default_policies};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_plugins::{
    PluginInventoryError, PluginInventorySource, PluginRecord, PluginRuntimeReport,
    PluginInventorySnapshot, ProfileRecord,
};
use flexcore_control_plane::state::AppState;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FixedClock,
};
use serde_json::{json, Value};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;

fn tenant_uuid(seed: u128) -> FlexUuid {
    FlexUuid::from_bytes(seed.to_be_bytes())
}

fn tenant_context(seed: u128) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(tenant_uuid(seed)),
            subject: "p7.25-api-plugins-test".to_string(),
        },
    ))
}

// ---------------------------------------------------------------------------
// Identity + composition (the api_alarms.rs pattern)
// ---------------------------------------------------------------------------

struct FixedIdentityResolver {
    callers: Vec<(String, Caller)>,
}

impl IdentityResolver for FixedIdentityResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        let token = raw
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidCredentials)?;
        self.callers
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, c)| c.clone())
            .ok_or(AuthError::InvalidCredentials)
    }
}

fn caller_for(seed: u128, subject: &str, role: Role) -> Caller {
    let claims = AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant_id(seed),
        subject: subject.to_string(),
    });
    Caller {
        ctx: TenantContext::from_authenticated_claims(claims),
        role,
    }
}

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(tenant_uuid(seed))
}

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

fn base_state_with(policies: PolicySet, resolver: Arc<dyn IdentityResolver>) -> AppState {
    let journal = flexcore_ports::fakes::FakeWriteJournal::new();
    AppState::for_tests(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal)),
        Arc::new(FakeEventOutbox::new(journal.clone())),
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        policies,
        resolver,
        passing_readiness(),
        None,
        300_000,
    )
}

/// The standard router: Phase 5 policy set, viewer + admin for tenant A.
fn composed_router() -> Router {
    let resolver = FixedIdentityResolver {
        callers: vec![
            (
                "viewer-a".to_string(),
                caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
            ),
            (
                "admin-a".to_string(),
                caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
            ),
        ],
    };
    stage1_router(base_state_with(phase5_default_policies(), Arc::new(resolver)))
}

async fn get_inventory(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder().uri("/api/v1/plugins");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::empty()).expect("the request builds"))
        .await
        .expect("infallible");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body reads");
    let value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, value)
}

// ---------------------------------------------------------------------------
// The fail-closed auth matrix
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unauthenticated_is_401_never_an_anonymous_inventory() {
    let router = composed_router();
    let (status, _) = get_inventory(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_empty_policy_set_denies_by_default_403() {
    let resolver = FixedIdentityResolver {
        callers: vec![(
            "viewer-a".to_string(),
            caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
        )],
    };
    let router = stage1_router(base_state_with(PolicySet::new(), Arc::new(resolver)));
    let (status, body) = get_inventory(&router, Some("viewer-a")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], json!("forbidden"));
}

#[tokio::test]
async fn every_authenticated_role_may_read_the_inventory() {
    let router = composed_router();
    for token in ["viewer-a", "admin-a"] {
        let (status, _) = get_inventory(&router, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "token {token} must read");
    }
}

// ---------------------------------------------------------------------------
// The recorded inventory (the shipped default source)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_recorded_inventory_serves_the_shipped_set_sorted() {
    let router = composed_router();
    let (status, body) = get_inventory(&router, Some("viewer-a")).await;
    assert_eq!(status, StatusCode::OK);
    let plugins = body["data"]["plugins"].as_array().expect("plugins array");
    let profile_ids: Vec<&str> = body["data"]["profiles"]
        .as_array()
        .expect("profiles array")
        .iter()
        .map(|p| p["profile_id"].as_str().expect("profile id"))
        .collect();
    let plugin_ids: Vec<&str> = plugins
        .iter()
        .map(|p| p["plugin_id"].as_str().expect("plugin id"))
        .collect();
    assert_eq!(
        plugin_ids,
        vec!["acme.forecast-worker", "neryva.optimizer-wasm-lp", "neryva.sim-builtin-ess"]
    );
    assert_eq!(
        profile_ids,
        vec!["research-gnn", "research-marl", "sim-lab", "site-edge", "vpp-operator"]
    );
}

#[tokio::test]
async fn the_ess_manifest_record_is_verbatim_metadata() {
    let router = composed_router();
    let (_, body) = get_inventory(&router, Some("viewer-a")).await;
    let ess = body["data"]["plugins"]
        .as_array()
        .expect("plugins array")
        .iter()
        .find(|p| p["plugin_id"] == json!("neryva.sim-builtin-ess"))
        .expect("the shipped ESS manifest is recorded");
    assert_eq!(ess["version"], json!("0.1.0"));
    assert_eq!(ess["class"], json!("builtin-rust"));
    assert_eq!(ess["provider_of"], json!(["ess.simulation"]));
    assert_eq!(ess["consumes"], json!(["telemetry.edge-sample"]));
    assert_eq!(ess["permissions"], json!(["telemetry.read"]));
    assert_eq!(ess["license"], json!("Apache-2.0"));
    // The record provenance names the shipped file — a record, not a run.
    assert!(
        ess["record_note"]
            .as_str()
            .expect("record note")
            .contains("builtin-ess-provider.manifest.json")
    );
}

#[tokio::test]
async fn runtime_facts_are_explicit_gaps_with_the_shipped_default_source() {
    let router = composed_router();
    let (status, body) = get_inventory(&router, Some("viewer-a")).await;
    assert_eq!(status, StatusCode::OK);
    for plugin in body["data"]["plugins"].as_array().expect("plugins") {
        assert_eq!(
            plugin["lifecycle_state"],
            json!("NOT_REPORTED"),
            "{} must not fabricate a lifecycle state",
            plugin["plugin_id"]
        );
        assert!(
            plugin["health"].is_null(),
            "{} must not fabricate health",
            plugin["plugin_id"]
        );
    }
}

#[tokio::test]
async fn the_surface_note_names_the_read_only_and_gap_rules() {
    let router = composed_router();
    let (_, body) = get_inventory(&router, Some("viewer-a")).await;
    let note = body["data"]["inventory_note"].as_str().expect("note");
    assert!(note.contains("RECORDED inventory"));
    assert!(note.contains("NOT_REPORTED"));
    assert!(note.contains("READ-ONLY"));
    assert!(note.contains("no activate, deactivate, upgrade, or profile-switch endpoint"));
}

#[tokio::test]
async fn every_profile_is_mode_locked_to_simulation_shadow() {
    let router = composed_router();
    let (_, body) = get_inventory(&router, Some("viewer-a")).await;
    for profile in body["data"]["profiles"].as_array().expect("profiles") {
        assert_eq!(
            profile["control_authority"],
            json!("simulation-shadow"),
            "profile {} must never carry a control-capable mode",
            profile["profile_id"]
        );
    }
    // The research ceilings withhold the proposal permissions.
    let research: Vec<Vec<&str>> = body["data"]["profiles"]
        .as_array()
        .expect("profiles")
        .iter()
        .filter(|p| {
            let id = p["profile_id"].as_str().expect("id");
            id == "research-gnn" || id == "research-marl"
        })
        .map(|p| {
            p["permission_ceiling"]
                .as_array()
                .expect("ceiling")
                .iter()
                .map(|v| v.as_str().expect("permission"))
                .collect()
        })
        .collect();
    assert_eq!(research.len(), 2);
    for ceiling in research {
        assert_eq!(ceiling, vec!["telemetry.read", "asset.read"]);
    }
}

#[tokio::test]
async fn site_edge_is_the_recorded_deferred_shell() {
    let router = composed_router();
    let (_, body) = get_inventory(&router, Some("viewer-a")).await;
    let site_edge = body["data"]["profiles"]
        .as_array()
        .expect("profiles")
        .iter()
        .find(|p| p["profile_id"] == json!("site-edge"))
        .expect("site-edge ships");
    assert_eq!(site_edge["composition_state"], json!("DEFERRED_SHELL"));
    assert_eq!(site_edge["active_selections"], json!([]));
    assert!(
        site_edge["state_note"]
            .as_str()
            .expect("state note")
            .contains("PROFILE_EMPTY_SELECTIONS")
    );
}

// ---------------------------------------------------------------------------
// A composed source: runtime facts render VERBATIM, partial reports stay gaps
// ---------------------------------------------------------------------------

/// A test source: the ESS manifest with a full runtime report; the other
/// two with none.
struct TestSourceWithRuntime;

impl PluginInventorySource for TestSourceWithRuntime {
    fn inventory(
        &self,
        _ctx: &TenantContext,
    ) -> Result<PluginInventorySnapshot, PluginInventoryError> {
        let mut snapshot = flexcore_control_plane::routes_plugins::RecordedShippedInventory
            .inventory(_ctx)
            .expect("the recorded base builds");
        for plugin in &mut snapshot.plugins {
            if plugin.plugin_id == "neryva.sim-builtin-ess" {
                plugin.runtime = Some(PluginRuntimeReport {
                    lifecycle_state: Some("READY".to_string()),
                    health_state: Some("DEGRADED_ERROR_RATE".to_string()),
                    readiness: Some("READY".to_string()),
                    health_note: Some("composed test health source".to_string()),
                });
            }
        }
        Ok(snapshot)
    }
}

/// A source that fails — mapped to infrastructure, never a fake success.
struct FailingSource;

impl PluginInventorySource for FailingSource {
    fn inventory(
        &self,
        _ctx: &TenantContext,
    ) -> Result<PluginInventorySnapshot, PluginInventoryError> {
        Err(PluginInventoryError::Backend {
            reason: "test forwarder down".to_string(),
        })
    }
}

fn router_with_source(source: Arc<dyn PluginInventorySource>) -> Router {
    let resolver = FixedIdentityResolver {
        callers: vec![(
            "viewer-a".to_string(),
            caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
        )],
    };
    let state = base_state_with(phase5_default_policies(), Arc::new(resolver))
        .with_plugins_inventory(source);
    stage1_router(state)
}

#[tokio::test]
async fn a_composed_source_renders_runtime_facts_verbatim() {
    let router = router_with_source(Arc::new(TestSourceWithRuntime));
    let (status, body) = get_inventory(&router, Some("viewer-a")).await;
    assert_eq!(status, StatusCode::OK);
    let ess = body["data"]["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["plugin_id"] == json!("neryva.sim-builtin-ess"))
        .expect("the ESS record");
    assert_eq!(ess["lifecycle_state"], json!("READY"));
    assert_eq!(ess["health"]["health_state"], json!("DEGRADED_ERROR_RATE"));
    assert_eq!(ess["health"]["readiness"], json!("READY"));
    assert_eq!(ess["health"]["note"], json!("composed test health source"));
    // The other plugins stay gaps.
    let wasm = body["data"]["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["plugin_id"] == json!("neryva.optimizer-wasm-lp"))
        .expect("the WASM record");
    assert_eq!(wasm["lifecycle_state"], json!("NOT_REPORTED"));
    assert!(wasm["health"].is_null());
}

#[tokio::test]
async fn a_failing_source_is_infrastructure_never_a_fake_success() {
    let router = router_with_source(Arc::new(FailingSource));
    let (status, body) = get_inventory(&router, Some("viewer-a")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], json!("infrastructure_failure"));
}

// ---------------------------------------------------------------------------
// The READ-ONLY surface (no plugin mutation path exists anywhere)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_surface_has_no_mutation_path_probe_and_openapi_check() {
    let router = composed_router();
    // A live POST probe is refused (405 — the route carries GET only).
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/plugins")
                .header("authorization", "Bearer admin-a")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("the probe builds"),
        )
        .await
        .expect("infallible");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    // The OpenAPI document lists GET /api/v1/plugins and NO plugin
    // mutation path.
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/openapi.json")
                .body(Body::empty())
                .expect("the request builds"),
        )
        .await
        .expect("infallible");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body reads");
    let doc: Value = serde_json::from_slice(&body).expect("the openapi doc parses");
    let paths = doc["paths"].as_object().expect("paths");
    let plugin_paths: Vec<(&String, &Value)> = paths
        .iter()
        .filter(|(path, _)| path.contains("plugin"))
        .collect();
    assert_eq!(plugin_paths.len(), 1, "exactly one plugin path exists");
    assert_eq!(plugin_paths[0].0, "/api/v1/plugins");
    assert!(
        plugin_paths[0].1.get("get").is_some(),
        "the plugin path is a GET"
    );
    assert!(
        plugin_paths[0].1.get("post").is_none()
            && plugin_paths[0].1.get("put").is_none()
            && plugin_paths[0].1.get("patch").is_none()
            && plugin_paths[0].1.get("delete").is_none(),
        "no mutation method exists on the plugin path"
    );
}

// The unused-record guard: the PluginRecord/ProfileRecord source types are
// exercised through the fakes above (TestSourceWithRuntime mutates the
// recorded snapshot); a direct construction would duplicate the in-module
// unit tests in routes_plugins.rs. The import below keeps the source-level
// types named in this contract file.
#[allow(dead_code)]
fn source_types_are_named(_p: Option<PluginRecord>, _pr: Option<ProfileRecord>) {}
