// P1.17 / P1.20 — API integration tests over the Test-variant backend.
//
// Composition rule (no network, no database): the app state is built with
// the flexcore-ports fakes (in-memory repositories, outbox, fixed clock,
// counting ID generator) and a fixed identity resolver that constructs its
// Caller through the single domain attestation point
// (`AuthenticatedClaims::from_verified`). Requests go through the real
// router via `tower::ServiceExt::oneshot`.
//
// Recorded assertions covered here: 201 happy path + outbox write pairing,
// 422 duplicate/validation, 401 unauthenticated, 403 deny-by-default,
// 404 cross-tenant reference (no existence leak), 404 unknown route,
// 200 probes. The `dev-auth`-in-default-features CI gate is asserted as a
// plain unit test reading this crate's manifest.
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase2_default_policies};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::state::AppState;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakeJournalOp,
    FakePortfolioRepository, FakeSiteRepository, FakeTelemetryReader, FakeTenantRepository,
    FakeWriteJournal, FixedClock,
};
use flexcore_ports::telemetry_read::QuarantinedTelemetry;
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuantity,
};
use serde_json::{json, Value};
use tower::ServiceExt;

// The counting generator yields `from_bytes(n)` for the n-th identifier, so
// the FIRST created tenant (the only create_tenant call in each test)
// becomes `...0001` and caller A's fixed tenant id matches it.
const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(FlexUuid::from_bytes(seed.to_be_bytes()))
}

/// Fixed test identity resolver: token -> Caller. Tokens are plain test
/// tokens (`Bearer caller-a`); a missing bearer is MissingCredentials (401),
/// an unknown one is InvalidCredentials (401). The Caller is built through
/// the reviewed domain attestation point, exactly as a real resolver would.
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

/// Builds the fixed Caller for one test principal.
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

/// A readiness bundle whose single (fake) check always passes.
fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

/// Composes the full Test-variant app state + router. Returns the router,
/// the shared fake outbox (for the outbox-write assertion), the shared
/// write journal (for the ordering/pairing assertion), and the fake
/// telemetry reader (to seed the P2.12/P2.17/P2.18 read surface).
fn composed_router() -> (
    Router,
    Arc<FakeEventOutbox>,
    FakeWriteJournal,
    Arc<FakeTelemetryReader>,
) {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let telemetry = Arc::new(FakeTelemetryReader::new());
    let resolver = FixedIdentityResolver {
        callers: vec![
            (
                "caller-a".to_string(),
                caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
            ),
            (
                "caller-b".to_string(),
                caller_for(TENANT_B_SEED, "user:admin-b", Role::TenantAdmin),
            ),
            (
                "viewer".to_string(),
                caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
            ),
            (
                // PlatformAdmin bound to tenant A: the recorded P2.12 grant
                // is `tenant.delete` → PlatformAdmin only, and no role
                // (including PlatformAdmin) bypasses the tenant match — so
                // cascade test deletes tenant A with this caller
                // (coordinator fix; the security slice pins the 403 for
                // TenantAdmin).
                "platform-a".to_string(),
                caller_for(TENANT_A_SEED, "user:platform-a", Role::PlatformAdmin),
            ),
        ],
    };
    let state = AppState::for_tests(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal.clone())),
        outbox.clone(),
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        phase2_default_policies(),
        Arc::new(resolver),
        passing_readiness(),
        Some(telemetry.clone()),
        // The P2.18 current-state freshness limit for the tests: the
        // SEED_BASE records' age under the fixed clock is 100_000 ms, so
        // this value puts the STALE boundary exactly on the seed data (see
        // the boundary test below).
        100_000,
    );
    (stage1_router(state), outbox, journal, telemetry)
}

// ---------------------------------------------------------------------------
// Request helpers (oneshot, no network)
// ---------------------------------------------------------------------------

async fn call(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot is infallible for this router");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("the API always emits JSON")
    };
    (status, body)
}

fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::empty()).expect("valid GET request")
}

fn post_json(uri: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("valid POST request")
}

fn create_tenant_body() -> Value {
    json!({
        "name": "Alpha Grid Co",
        "description": "first tenant",
        "identity_provider_reference": "idp:alpha",
        "policy_profile": "profile:default"
    })
}

async fn create_tenant_a(router: &Router) {
    let (status, _) = call(
        router,
        post_json("/api/v1/tenants", Some("caller-a"), create_tenant_body()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "tenant A setup");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_tenant_happy_path_is_201_and_writes_the_outbox() {
    let (router, outbox, journal, _telemetry) = composed_router();
    let (status, body) = call(
        &router,
        post_json("/api/v1/tenants", Some("caller-a"), create_tenant_body()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let tenant = &body["data"];
    assert_eq!(tenant["name"], "Alpha Grid Co");
    // The created tenant IS the caller's own tenant (the counting generator's
    // first identifier matches the fixed resolver's tenant A).
    assert_eq!(
        tenant["id"],
        tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string()
    );

    // The transactional-outbox rule: exactly one write, and the journal
    // shows the state change + the outbox append adjacent (one unit of
    // work, the P1.08 ordering evidence).
    let writes = outbox.writes();
    assert_eq!(writes.len(), 1, "one create = one outbox write");
    let ops = journal.ops();
    assert_eq!(ops.len(), 2, "state change + outbox append, in order");
    assert!(matches!(ops[0], FakeJournalOp::StateChange { .. }));
    assert!(matches!(ops[1], FakeJournalOp::OutboxAppend { .. }));
}

#[tokio::test]
async fn duplicate_tenant_registration_is_422() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    create_tenant_a(&router).await;
    let (status, body) = call(
        &router,
        post_json("/api/v1/tenants", Some("caller-a"), create_tenant_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_rejected");
}

#[tokio::test]
async fn unauthenticated_requests_are_401() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (status, body) = call(
        &router,
        post_json("/api/v1/portfolios", None, json!({"name": "p"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[tokio::test]
async fn wrong_role_writes_are_403() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (status, body) = call(
        &router,
        post_json("/api/v1/tenants", Some("viewer"), create_tenant_body()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
}

#[tokio::test]
async fn cross_tenant_site_creation_is_404() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    create_tenant_a(&router).await;
    // Tenant A creates a portfolio.
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/portfolios",
            Some("caller-a"),
            json!({
                "name": "Alpha Portfolio",
                "operator": "ops-a",
                "market_region": "EU-DE",
                "aggregation_rules": "rules:default"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let portfolio_id = body["data"]["id"].as_str().expect("portfolio id");

    // Caller B (another tenant) references that portfolio: the parent lives
    // outside B's tenant scope, so the application layer reports it as
    // missing and the API preserves that — 404, never 403 (no existence leak).
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/sites",
            Some("caller-b"),
            json!({
                "portfolio_id": portfolio_id,
                "name": "Outside Site",
                "location": "52.52N,13.40E",
                "timezone": "Europe/Berlin",
                "point_of_common_coupling": "poc-1",
                "grid_model_reference": "grid-model:ref",
                "edge_identity": "edge:b-1"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn invalid_asset_type_is_422() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/assets",
            Some("caller-a"),
            json!({
                "site_id": tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string(),
                "name": "Battery 1",
                "asset_type": "flux_capacitor",
                "vendor_model": "model-x",
                "rated_power_mw": 1.5,
                "phase": "three",
                "bus_reference": "bus-1",
                "capability_profile": "profile:storage"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_rejected");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message")
            .contains("flux_capacitor"),
        "the message names the rejected vocabulary id"
    );
}

#[tokio::test]
async fn unknown_route_is_404() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (status, _) = call(
        &router,
        get("/api/v1/definitely-not-here", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn probes_report_up_and_ready() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (health_status, _) = call(&router, get("/health", None)).await;
    assert_eq!(health_status, StatusCode::OK);
    let (ready_status, body) = call(&router, get("/ready", None)).await;
    assert_eq!(ready_status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn tenant_scoped_reads_return_the_callers_own_tenant() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    create_tenant_a(&router).await;

    // List: exactly the caller's own tenant.
    let (status, body) = call(&router, get("/api/v1/tenants", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("list").len(), 1);

    // A tenant id that is not the caller's own: 404, no existence leak.
    let other = tenant_id(TENANT_B_SEED)
        .uuid()
        .to_hyphenated()
        .to_string();
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/tenants/{other}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

// ---------------------------------------------------------------------------
// CI gate (P1.27 contract, documented here as a manifest assertion)
// ---------------------------------------------------------------------------

/// The `dev-auth` feature must NEVER be a default feature: the manifest is
/// the gate. This test fails the suite the moment someone makes
/// `default = ["dev-auth"]` (or otherwise wires the dev resolver into
/// default builds) — release builds must deny every unauthenticated
/// request via `DenyAllResolver`.
#[test]
fn dev_auth_is_not_a_default_feature() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("default = [\"dev-auth\"]"),
        "dev-auth must never appear in default features (CI gate, ledger P1.27)"
    );
    assert!(
        manifest.contains("default = []"),
        "the crate keeps an explicit empty default feature set"
    );
}

// ---------------------------------------------------------------------------
// P2.12 / P2.17 / P2.18 — Phase 2 API tests
// ---------------------------------------------------------------------------

fn delete_req(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("DELETE").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::empty()).expect("valid DELETE request")
}

fn patch_json(uri: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::builder().method("PATCH").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("valid PATCH request")
}

/// Creates the full ownership chain for tenant A via the API and returns
/// the (portfolio, site, asset) ids as hyphenated UUID strings.
async fn setup_chain(router: &Router) -> (String, String, String) {
    create_tenant_a(router).await;
    let (status, body) = call(
        router,
        post_json(
            "/api/v1/portfolios",
            Some("caller-a"),
            json!({
                "name": "Alpha Portfolio",
                "operator": "ops-a",
                "market_region": "EU-DE",
                "aggregation_rules": "rules:default"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "portfolio setup");
    let portfolio = body["data"]["id"].as_str().expect("portfolio id").to_string();

    let (status, body) = call(
        router,
        post_json(
            "/api/v1/sites",
            Some("caller-a"),
            json!({
                "portfolio_id": portfolio,
                "name": "Site One",
                "location": "52.52N,13.40E",
                "timezone": "Europe/Berlin",
                "point_of_common_coupling": "poc-1",
                "grid_model_reference": "grid-model:ref",
                "edge_identity": "edge-sim-1"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "site setup");
    let site = body["data"]["id"].as_str().expect("site id").to_string();

    let (status, body) = call(
        router,
        post_json(
            "/api/v1/assets",
            Some("caller-a"),
            json!({
                "site_id": site,
                "name": "Battery 1",
                "asset_type": "bess",
                "vendor_model": "model-x",
                "rated_power_mw": 1.5,
                "rated_energy_mwh": 3.0,
                "phase": "three",
                "bus_reference": "bus-1",
                "capability_profile": "profile:storage"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "asset setup");
    let asset = body["data"]["id"].as_str().expect("asset id").to_string();

    (portfolio, site, asset)
}

/// Builds one canonical simulator record for seeding the telemetry fake.
fn sim_record(
    tenant: TenantId,
    asset: AssetId,
    quantity: TelemetryQuantity,
    value: QuantityValue,
    event_ms: i64,
    quality: TelemetryQuality,
    sequence: u64,
) -> NormalizedTelemetry {
    // A MISSING record is a platform-derived gap marker (P2.04 D-T6) — the
    // source namespace and the value presence follow the canonical rules.
    let (source_id, value) = if quality == TelemetryQuality::Missing {
        ("flexcore:derived:gap-detector".to_string(), None)
    } else {
        ("sim:lab-1".to_string(), Some(value))
    };
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id: tenant,
        asset_id: asset,
        quantity,
        value,
        event_time: EventTime::from_unix_millis(event_ms).expect("valid test event time"),
        ingest_time: IngestTime::from_unix_millis(event_ms).expect("valid test ingest time"),
        source_sequence: sequence,
        source_id,
        quality,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .expect("canonical test record")
}

fn asset_uuid(raw: &str) -> AssetId {
    AssetId::new(FlexUuid::parse(raw).expect("valid asset uuid"))
}

// Query window used by the telemetry tests: 3 days (< the recorded 7-day
// maximum), covering the seeded event times.
const QUERY_FROM: &str = "2023-11-13T00:00:00Z";
const QUERY_UNTIL: &str = "2023-11-16T00:00:00Z";
const SEED_BASE_MILLIS: i64 = 1_699_900_000_000;

#[tokio::test]
async fn restrict_rule_blocks_deleting_a_non_empty_site_or_portfolio() {
    let (router, outbox, _journal, _telemetry) = composed_router();
    let (_p, site, _a) = setup_chain(&router).await;
    let writes_before = outbox.writes().len();

    // RESTRICT (P2.02, decided once): a site with assets is 409 with the
    // stable non_empty_parent code — DECIDED: 409, not 422.
    let (status, body) = call(
        &router,
        delete_req(&format!("/api/v1/sites/{site}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "non_empty_parent");
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message")
            .contains("RESTRICT"),
        "the message names the decided rule"
    );
    assert_eq!(
        outbox.writes().len(),
        writes_before,
        "a restricted delete writes nothing"
    );
}

#[tokio::test]
async fn empty_parents_and_leaves_delete_with_the_audit_trail_preserved() {
    let (router, _outbox, journal, _telemetry) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;

    // The leaf deletes first (each an individually visible operation).
    let (status, _) = call(
        &router,
        delete_req(&format!("/api/v1/assets/{asset}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Audit trail preserved (P2.02 §6): the accepted deletion is a
    // `Delete` state change paired with exactly one outbox event, adjacent
    // in the journal (the §6.3 step-1 ordering evidence).
    let ops = journal.ops();
    let (last, second_last) = (ops.last().expect("ops"), &ops[ops.len() - 2]);
    assert!(
        matches!(
            second_last,
            FakeJournalOp::StateChange {
                operation: flexcore_ports::state_change::StateChangeOperation::Delete,
                ..
            }
        ),
        "the delete state change is journalled: {second_last:?}"
    );
    match last {
        FakeJournalOp::OutboxAppend {
            idempotency_key,
            entity,
            tenant_scope,
        } => {
            assert!(idempotency_key.starts_with("operator_action_recorded:"));
            assert_eq!(entity, "assets");
            assert_eq!(*tenant_scope, Some(tenant_id(TENANT_A_SEED)));
        }
        other => panic!("expected the paired outbox append, got {other:?}"),
    }

    // Now the emptied parents delete too: site, then portfolio, then the
    // tenant itself.
    for uri in [
        format!("/api/v1/sites/{site}"),
        format!("/api/v1/portfolios/{portfolio}"),
    ] {
        let (status, _) = call(&router, delete_req(&uri, Some("caller-a"))).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "deleting {uri}");
    }
    let (status, _) = call(
        &router,
        delete_req(
            &format!(
                "/api/v1/tenants/{}",
                tenant_id(TENANT_A_SEED).uuid().to_hyphenated()
            ),
            Some("platform-a"),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "the emptied tenant deletes (PlatformAdmin; tenant.delete is platform-only)"
    );
}

#[tokio::test]
async fn patch_updates_asset_metadata_and_deactivate_persists_the_reason() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;

    // P2.03 use case via PATCH.
    let (status, body) = call(
        &router,
        patch_json(
            &format!("/api/v1/assets/{asset}"),
            Some("caller-a"),
            json!({
                "vendor_model": "vendor-y/model-9",
                "rated_power_mw": 3.5,
                "rated_energy_mwh": 7.0,
                "capability_profile": "capability:bess-v2"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["vendor_model"], "vendor-y/model-9");
    assert_eq!(body["data"]["rated_power_mw"], 3.5);
    assert_eq!(body["data"]["capability_profile"], "capability:bess-v2");
    assert_eq!(body["data"]["lifecycle_status"], "active");

    // P2.03 deactivation: terminal, reason persisted.
    let (status, body) = call(
        &router,
        post_json(
            &format!("/api/v1/assets/{asset}/deactivate"),
            Some("caller-a"),
            json!({"reason": "planned maintenance"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["lifecycle_status"], "deactivated");
    assert_eq!(body["data"]["deactivation_reason"], "planned maintenance");

    // Terminal: a second deactivation is a validation-class rejection.
    let (status, body) = call(
        &router,
        post_json(
            &format!("/api/v1/assets/{asset}/deactivate"),
            Some("caller-a"),
            json!({"reason": "again"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_rejected");
}

#[tokio::test]
async fn telemetry_query_enforces_required_range_limits_and_six_state_quality() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;

    // Missing time range: 422 (a telemetry query always names a range).
    let (status, _) = call(
        &router,
        get(
            &format!("/api/v1/telemetry/query?asset={asset}"),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Oversized range (> the recorded 7-day maximum): 422.
    let (status, _) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from=2020-01-01T00:00:00Z&until={QUERY_UNTIL}"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // A seventh quality value: 422 (the P0.08 vocabulary is closed).
    let (status, body) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&quality=PARTIALLY_VALID"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("message")
            .contains("PARTIALLY_VALID")
    );

    // Oversized page: 422.
    let (status, _) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&limit=1001"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Malformed cursor: 422.
    let (status, _) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&cursor=not-a-cursor"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn telemetry_query_returns_seeded_records_with_keyset_pagination() {
    let (router, _outbox, _journal, telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;
    let asset_id = asset_uuid(&asset);
    let tenant = tenant_id(TENANT_A_SEED);

    telemetry.seed(vec![
        sim_record(
            tenant,
            asset_id,
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(1.0).unwrap()),
            SEED_BASE_MILLIS,
            TelemetryQuality::Good,
            1,
        ),
        sim_record(
            tenant,
            asset_id,
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(2.0).unwrap()),
            SEED_BASE_MILLIS + 60_000,
            TelemetryQuality::Good,
            2,
        ),
        sim_record(
            tenant,
            asset_id,
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(3.0).unwrap()),
            SEED_BASE_MILLIS + 120_000,
            TelemetryQuality::Suspect,
            3,
        ),
    ]);

    // Page one: limit 2 → two records + a cursor.
    let (status, body) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&limit=2"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("page one");
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["value_num"], 1.0);
    assert_eq!(data[0]["unit"], "MW");
    assert_eq!(data[0]["quality"], "GOOD");
    assert_eq!(data[0]["simulated"], true, "every Phase 2 value is simulated");
    let cursor = body["next_cursor"]
        .as_str()
        .expect("keyset cursor")
        .to_string();

    // Page two: the cursor continues after the last record of page one.
    let (status, body) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&limit=2&cursor={cursor}"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("page two");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["value_num"], 3.0);
    assert!(body["next_cursor"].is_null(), "the last page has no cursor");

    // Quality filter: only the GOOD records come back.
    let (status, body) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}&quality=GOOD"
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("filtered page");
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn asset_state_carries_quality_age_and_the_simulated_flag() {
    let (router, _outbox, _journal, telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;
    let tenant = tenant_id(TENANT_A_SEED);
    let asset_id = asset_uuid(&asset);

    telemetry.seed(vec![sim_record(
        tenant,
        asset_id,
        TelemetryQuantity::ActivePower,
        QuantityValue::Power(ActivePowerMW::new(1.5).unwrap()),
        SEED_BASE_MILLIS,
        TelemetryQuality::Good,
        1,
    )]);

    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let state = &body["data"];
    assert_eq!(state["simulated"], true, "the view marks simulator data");
    let quantities = state["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 1);
    assert_eq!(quantities[0]["quantity"], "active_power");
    assert_eq!(quantities[0]["value_num"], 1.5);
    assert_eq!(quantities[0]["unit"], "MW");
    assert_eq!(quantities[0]["quality"], "GOOD");
    assert!(quantities[0]["age_ms"].is_i64(), "age is carried");
    assert_eq!(quantities[0]["simulated"], true);

    // MISSING semantics (P2.18): a quantity whose LATEST record (by
    // event_time) is a gap marker carries MISSING and no value — never a
    // zero. A later-arriving older GOOD record must not regress the state
    // either (latest-by-event_time rule).
    telemetry.seed(vec![
        sim_record(
            tenant,
            asset_id,
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(2.5).unwrap()),
            SEED_BASE_MILLIS + 300_000,
            TelemetryQuality::Missing,
            5,
        ),
        // A late record with an OLDER event_time than the gap marker.
        sim_record(
            tenant,
            asset_id,
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(9.9).unwrap()),
            SEED_BASE_MILLIS,
            TelemetryQuality::Good,
            6,
        ),
    ]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 1, "one quantity, latest record only");
    assert_eq!(quantities[0]["quality"], "MISSING");
    assert!(quantities[0]["value_num"].is_null(), "MISSING is not a zero");

    // A later-arriving NEWER record updates the state (latest-by-event_time
    // moves forward in both directions: an older record never regresses, a
    // newer one always advances).
    telemetry.seed(vec![sim_record(
        tenant,
        asset_id,
        TelemetryQuantity::ActivePower,
        QuantityValue::Power(ActivePowerMW::new(3.5).unwrap()),
        SEED_BASE_MILLIS + 600_000,
        TelemetryQuality::Good,
        7,
    )]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 1);
    assert_eq!(quantities[0]["quality"], "GOOD", "the newer record is the state");
    assert_eq!(quantities[0]["value_num"], 3.5);
}

/// P2.18: the current-state freshness limit (configuration — the test
/// composes the state with 100_000 ms, exactly the seed records' age under
/// the fixed clock) is applied by the P2.08 staleness rule: `age == limit`
/// is NOT stale, `age > limit` shows the LAST KNOWN value with the STALE
/// flag (never as GOOD), and a fresh record keeps its stored quality
/// exactly (ESTIMATED shows ESTIMATED — quality is never re-labelled).
#[tokio::test]
async fn current_state_applies_the_configured_freshness_limit_with_the_stale_boundary() {
    let (router, _outbox, _journal, telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;
    let tenant = tenant_id(TENANT_A_SEED);
    let asset_id = asset_uuid(&asset);

    // Boundary: age == limit (100_000 ms) is NOT stale — GOOD preserved.
    telemetry.seed(vec![sim_record(
        tenant,
        asset_id,
        TelemetryQuantity::ActivePower,
        QuantityValue::Power(ActivePowerMW::new(1.5).unwrap()),
        SEED_BASE_MILLIS,
        TelemetryQuality::Good,
        1,
    )]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 1);
    assert_eq!(quantities[0]["quality"], "GOOD", "age == limit is not stale");
    assert_eq!(quantities[0]["age_ms"], 100_000);

    // Beyond the boundary (a second quantity so the latest-by-event_time
    // rule keeps both): age == limit + 1_000 ms is STALE — the last known
    // value and its timestamps are preserved WITH the STALE flag, never
    // presented as GOOD, and never dropped.
    telemetry.seed(vec![sim_record(
        tenant,
        asset_id,
        TelemetryQuantity::ReactivePower,
        QuantityValue::Reactive {
            power: flexcore_contracts::units::ReactivePowerMVAr::new(0.5).unwrap(),
            convention: flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive,
        },
        SEED_BASE_MILLIS - 1_000,
        TelemetryQuality::Good,
        2,
    )]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 2, "one latest record per quantity");
    let reactive = quantities
        .iter()
        .find(|q| q["quantity"] == "reactive_power")
        .expect("the reactive entry");
    assert_eq!(reactive["quality"], "STALE", "age > limit is stale, never GOOD");
    assert_eq!(reactive["value_num"], 0.5, "the last known value is shown");
    assert_eq!(reactive["q_sign_convention"], "CONSUMPTION_POSITIVE");
    assert!(reactive["event_time"].is_string());
    assert!(reactive["ingest_time"].is_string());
    assert_eq!(reactive["age_ms"], 101_000);
    assert_eq!(reactive["simulated"], true);
    let active = quantities
        .iter()
        .find(|q| q["quantity"] == "active_power")
        .expect("the active entry");
    assert_eq!(active["quality"], "GOOD", "the boundary quantity is untouched");

    // Quality preserved (P2.18 step 5): a fresh ESTIMATED record shows
    // ESTIMATED exactly — the view never re-labels a quality state.
    telemetry.seed(vec![sim_record(
        tenant,
        asset_id,
        TelemetryQuantity::Soc,
        QuantityValue::Fraction(flexcore_contracts::units::StateOfCharge::new(0.75).unwrap()),
        SEED_BASE_MILLIS + 90_000,
        TelemetryQuality::Estimated,
        3,
    )]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities.len(), 3);
    let soc = quantities.iter().find(|q| q["quantity"] == "soc").expect("soc");
    assert_eq!(
        soc["quality"], "ESTIMATED",
        "quality preserved, never re-labelled"
    );
    assert_eq!(soc["unit"], "1");
}

/// P2.18: an asset with NO telemetry at all has an EMPTY current state —
/// absence is the MISSING representation, never a zero value and never a
/// fabricated entry.
#[tokio::test]
async fn current_state_of_an_asset_without_telemetry_is_absence_not_zero() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (_p, _s, asset) = setup_chain(&router).await;

    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let state = &body["data"];
    assert_eq!(state["asset_id"], asset.as_str());
    assert_eq!(
        state["quantities"].as_array().expect("quantities").len(),
        0,
        "no records -> no entries (absence, never a zero)"
    );
    assert_eq!(state["simulated"], true);
}

#[tokio::test]
async fn quarantine_read_is_admin_only_and_tenant_scoped() {
    let (router, _outbox, _journal, telemetry) = composed_router();
    setup_chain(&router).await;
    telemetry.quarantine(QuarantinedTelemetry::new(
        tenant_id(TENANT_A_SEED),
        "{\"raw\":true}".to_string(),
        "schema validation failed".to_string(),
        "sim:lab-1".to_string(),
        None,
        None,
        IngestTime::from_unix_millis(SEED_BASE_MILLIS).expect("valid ingest time"),
        0,
    ));

    // The tenant admin sees the tenant's entries.
    let (status, body) = call(
        &router,
        get("/api/v1/telemetry/quarantine", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("entries").len(), 1);
    assert_eq!(body["data"][0]["failure_reason"], "schema validation failed");

    // The viewer role is not granted quarantine.read: 403.
    let (status, body) = call(
        &router,
        get("/api/v1/telemetry/quarantine", Some("viewer")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    // Tenant scoping: tenant B sees none of tenant A's quarantine.
    let (status, body) = call(
        &router,
        get("/api/v1/telemetry/quarantine", Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("entries").len(), 0);
}

#[tokio::test]
async fn phase2_views_report_honest_edge_and_simulated_state() {
    let (router, _outbox, _journal, telemetry) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;

    // Portfolio view: composition + the commitments placeholder.
    let (status, body) = call(
        &router,
        get(
            &format!("/api/v1/portfolios/{portfolio}/view"),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["composition"]["site_count"], 1);
    assert_eq!(body["data"]["composition"]["asset_count"], 1);
    assert_eq!(body["data"]["commitments"]["populated"], false);

    // Site view: the edge is honestly reported as the simulated edge,
    // never live.
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/sites/{site}/view"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["edge_connectivity"]["status"], "simulator");
    assert_eq!(body["data"]["edge_connectivity"]["live"], false);
    assert_eq!(body["data"]["overview"]["asset_count"], 1);

    // Asset view: stored limits + stored maintenance/outage status, and
    // the telemetry section marks every value simulated.
    telemetry.seed(vec![sim_record(
        tenant_id(TENANT_A_SEED),
        asset_uuid(&asset),
        TelemetryQuantity::Soc,
        QuantityValue::Fraction(
            flexcore_contracts::units::StateOfCharge::new(0.75).unwrap(),
        ),
        SEED_BASE_MILLIS,
        TelemetryQuality::Good,
        1,
    )]);
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/view"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["limits"]["rated_power_mw"], 1.5);
    assert_eq!(body["data"]["lifecycle_status"], "active");
    assert_eq!(body["data"]["maintenance_outage_status"], "operational");
    let quantities = body["data"]["current_state"]["quantities"]
        .as_array()
        .expect("current state");
    assert_eq!(quantities[0]["quantity"], "soc");
    assert_eq!(quantities[0]["unit"], "1");
    assert_eq!(quantities[0]["quality"], "GOOD");
    assert_eq!(quantities[0]["simulated"], true);
}

#[tokio::test]
async fn cross_tenant_access_is_denied_on_every_new_phase2_route() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;

    // Tenant B references tenant A's resources: 404 on every route (no
    // existence leak across the tenancy boundary, P1.17 decision).
    let (status, _) = call(
        &router,
        delete_req(&format!("/api/v1/sites/{site}"), Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &router,
        patch_json(
            &format!("/api/v1/assets/{asset}"),
            Some("caller-b"),
            json!({
                "vendor_model": "x",
                "rated_power_mw": 1.0,
                "capability_profile": "p"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &router,
        post_json(
            &format!("/api/v1/assets/{asset}/deactivate"),
            Some("caller-b"),
            json!({"reason": "nope"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/view"), Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &router,
        get(
            &format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"
            ),
            Some("caller-b"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn registry_lists_honour_the_recorded_page_limit() {
    let (router, _outbox, _journal, _telemetry) = composed_router();
    create_tenant_a(&router).await;
    for name in ["P1", "P2"] {
        let (status, _) = call(
            &router,
            post_json(
                "/api/v1/portfolios",
                Some("caller-a"),
                json!({
                    "name": name,
                    "operator": "ops",
                    "market_region": "EU-DE",
                    "aggregation_rules": "rules:default"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = call(
        &router,
        get("/api/v1/portfolios?limit=1", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("page").len(), 1);

    // Over the recorded maximum: 422.
    let (status, _) = call(
        &router,
        get("/api/v1/portfolios?limit=1001", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
