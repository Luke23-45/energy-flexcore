// P2.22 — Phase 2 security test slice: tenant isolation on every new
// Phase 2 route (registry CRUD, telemetry query, current state, views,
// quarantine), registry ownership security, and the P2.21 metric-inventory
// check over `/metrics`.
//
// Composition rule (no network, no database — the P1.17/P2.12 harness
// pattern): the app state is built with the flexcore-ports fakes and a
// fixed identity resolver that constructs its Caller through the single
// domain attestation point (`AuthenticatedClaims::from_verified`).
// Requests go through the real router via `tower::ServiceExt::oneshot`.
//
// Recorded assertions covered here (ledger P2.22 acceptance):
// 1. Tenant isolation: tenant B's verified claims cannot read or write any
//    tenant A resource on ANY Phase 2 route — 404 (the cross-tenant rule:
//    no existence leak, no A-side data in the error body), except where the
//    action is not granted to B's role at all (the platform-only
//    `tenant.delete` denies with 403 before any existence check).
// 2. Registry security: an asset cannot be registered under another
//    tenant's site; a portfolio cannot be moved across tenants (no
//    re-parent route exists; the only update route cannot change
//    ownership; the cross-tenant delete-and-recreate path is closed).
// 3. Telemetry security: telemetry query and current-state reads cannot
//    cross tenants; the quarantine list is tenant-scoped (B sees none of
//    A's quarantined payloads).
// 4. Deny-by-default holds: no role bypasses the tenant match — the admin
//    role denied on the platform-only `tenant.delete` grant, the viewer
//    role denied on `quarantine.read`; unauthenticated callers are 401.
// 5. P2.21 (observability acceptance): the `/metrics` scrape exposes all
//    six telemetry metric families with all six canonical quality states.
//
// Everything here asserts AUTHORIZATION behaviour only. FlexCore is a
// supervisory orchestration layer; the entire 0.x line is simulation-only
// — nothing in this file is a safety, certification, or protection claim.
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
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTelemetryReader, FakeTenantRepository, FixedClock,
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
/// the shared fake telemetry reader (to seed the quarantine/telemetry read
/// surface), and the fake outbox.
fn composed_router() -> (Router, Arc<FakeTelemetryReader>, Arc<FakeEventOutbox>) {
    let journal = flexcore_ports::fakes::FakeWriteJournal::new();
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
                "viewer-b".to_string(),
                caller_for(TENANT_B_SEED, "user:viewer-b", Role::Viewer),
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
    );
    (stage1_router(state), telemetry, outbox)
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

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

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
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id: tenant,
        asset_id: asset,
        quantity,
        value: Some(value),
        event_time: EventTime::from_unix_millis(event_ms).expect("valid test event time"),
        ingest_time: IngestTime::from_unix_millis(event_ms).expect("valid test ingest time"),
        source_sequence: sequence,
        source_id: "sim:lab-1".to_string(),
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

/// Asserts a cross-tenant response is a 404 whose body leaks NOTHING about
/// the referenced resource: the stable envelope carries only the generic
/// error, never the ids or any data field.
fn assert_not_found_without_leak(status: StatusCode, body: &Value, secrets: &[&str]) {
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-tenant access reads as 404");
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body.get("data").is_none(), "no data field on an error envelope");
    let rendered = body.to_string();
    for secret in secrets {
        assert!(
            !rendered.contains(secret),
            "the error body must not carry the referenced resource value {secret}"
        );
    }
}

// ---------------------------------------------------------------------------
// 1. Tenant isolation on every new Phase 2 route (P2.22 step 1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_tenant_registry_crud_is_404_with_no_data_leak() {
    let (router, _telemetry, _outbox) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;
    let tenant_a = tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string();

    // Tenant B references tenant A's resources: 404 on EVERY registry
    // route whose action B's role holds, with no existence leak in the
    // body (the cross-tenant rule). Tenant deletion is PlatformAdmin-only,
    // so B's admin is denied earlier (403) — authorization precedes the
    // existence check, and the platform-only action is unreachable for a
    // foreign tenant admin at all (asserted separately below).
    let cases: Vec<Request<Body>> = vec![
        delete_req(&format!("/api/v1/portfolios/{portfolio}"), Some("caller-b")),
        delete_req(&format!("/api/v1/sites/{site}"), Some("caller-b")),
        delete_req(&format!("/api/v1/assets/{asset}"), Some("caller-b")),
        patch_json(
            &format!("/api/v1/assets/{asset}"),
            Some("caller-b"),
            json!({
                "vendor_model": "tenant-b-write",
                "rated_power_mw": 9.9,
                "capability_profile": "capability:hostile"
            }),
        ),
        post_json(
            &format!("/api/v1/assets/{asset}/deactivate"),
            Some("caller-b"),
            json!({"reason": "tenant B touching tenant A"}),
        ),
    ];
    for request in cases {
        let (status, body) = call(&router, request).await;
        assert_not_found_without_leak(
            status,
            &body,
            &[&portfolio, &site, &asset, &tenant_a, "tenant-b-write", "capability:hostile"],
        );
    }

    // The platform-only tenant.delete action: B's TenantAdmin is denied
    // outright (403, the recorded grant design) — the cross-tenant delete
    // never even reaches the existence check.
    let (status, body) = call(
        &router,
        delete_req(&format!("/api/v1/tenants/{tenant_a}"), Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "tenant.delete is PlatformAdmin-only");
    assert_eq!(body["error"]["code"], "forbidden");
    assert!(!body.to_string().contains(&tenant_a), "no existence leak on the deny");

    // The failed writes wrote NOTHING: tenant A's asset is unchanged.
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["vendor_model"], "model-x", "B's write never landed");
    assert_eq!(body["data"]["lifecycle_status"], "active");
}

#[tokio::test]
async fn cross_tenant_telemetry_state_and_views_are_404_with_no_data_leak() {
    let (router, telemetry, _outbox) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;
    let tenant_a = tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string();

    // Seed tenant A's telemetry and quarantine with recognizable values.
    telemetry.seed(vec![sim_record(
        tenant_id(TENANT_A_SEED),
        asset_uuid(&asset),
        TelemetryQuantity::ActivePower,
        QuantityValue::Power(ActivePowerMW::new(4.2).unwrap()),
        SEED_BASE_MILLIS,
        TelemetryQuality::Good,
        1,
    )]);
    telemetry.quarantine(QuarantinedTelemetry::new(
        tenant_id(TENANT_A_SEED),
        "{\"secret\":\"tenant-a-payload\"}".to_string(),
        "schema validation failed".to_string(),
        "sim:lab-1".to_string(),
        None,
        None,
        IngestTime::from_unix_millis(SEED_BASE_MILLIS).expect("valid ingest time"),
        0,
    ));

    let cases: Vec<(String, String)> = vec![
        (
            "telemetry query".to_string(),
            format!(
                "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"
            ),
        ),
        ("current state".to_string(), format!("/api/v1/assets/{asset}/state")),
        ("asset view".to_string(), format!("/api/v1/assets/{asset}/view")),
        ("site view".to_string(), format!("/api/v1/sites/{site}/view")),
        (
            "portfolio view".to_string(),
            format!("/api/v1/portfolios/{portfolio}/view"),
        ),
    ];
    for (name, uri) in cases {
        let (status, body) = call(&router, get(&uri, Some("caller-b"))).await;
        assert_not_found_without_leak(status, &body, &[&portfolio, &site, &asset, &tenant_a]);
        let rendered = body.to_string();
        assert!(
            !rendered.contains("tenant-a-payload"),
            "{name}: the error body leaked quarantined content"
        );
        assert!(
            !rendered.contains("4.2"),
            "{name}: the error body leaked telemetry values"
        );
    }

    // The quarantine LIST route is tenant-scoped, not 404: B gets an empty
    // list and none of A's quarantined payloads.
    let (status, body) = call(&router, get("/api/v1/telemetry/quarantine", Some("viewer-b"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewer lacks quarantine.read");
    let (status, body) = call(&router, get("/api/v1/telemetry/quarantine", Some("caller-b"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"].as_array().expect("entries").len(),
        0,
        "tenant B sees none of tenant A's quarantine"
    );
    assert!(
        !body.to_string().contains("tenant-a-payload"),
        "no A-side quarantine content crossed the tenancy boundary"
    );

    // B's telemetry query over ITS OWN (nonexistent) asset scope cannot
    // surface A's records either: an unknown asset in B's scope is 404.
    let (status, body) = call(
        &router,
        get(
            &format!("/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"),
            Some("caller-b"),
        ),
    )
    .await;
    assert_not_found_without_leak(status, &body, &[&asset]);
}

// ---------------------------------------------------------------------------
// 2. Registry security (P2.22 step 2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_asset_cannot_be_registered_under_another_tenants_site() {
    let (router, _telemetry, _outbox) = composed_router();
    let (_portfolio, site, _asset) = setup_chain(&router).await;

    // Tenant B registers an asset under tenant A's site: the parent lives
    // outside B's tenant scope → 404 (no existence leak), and no asset is
    // created under A's site.
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/assets",
            Some("caller-b"),
            json!({
                "site_id": site,
                "name": "Hostile Asset",
                "asset_type": "bess",
                "vendor_model": "model-b",
                "rated_power_mw": 1.0,
                "phase": "three",
                "bus_reference": "bus-9",
                "capability_profile": "profile:storage"
            }),
        ),
    )
    .await;
    assert_not_found_without_leak(status, &body, &[&site]);

    // Tenant A still sees exactly its own single asset.
    let (status, body) = call(&router, get("/api/v1/assets", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("assets").len(), 1);
}

#[tokio::test]
async fn a_portfolio_cannot_be_moved_across_tenants() {
    let (router, _telemetry, _outbox) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;
    let tenant_a = tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string();

    // (a) The API exposes NO re-parent route at all: POST/PATCH on the
    // portfolio resource are method-not-allowed — a portfolio cannot be
    // moved, only created inside the caller's own tenant.
    let (status, _) = call(
        &router,
        post_json(
            &format!("/api/v1/portfolios/{portfolio}"),
            Some("caller-b"),
            json!({"tenant_id": tenant_id(TENANT_B_SEED).uuid().to_hyphenated().to_string()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "no move route exists");
    let (status, _) = call(
        &router,
        patch_json(
            &format!("/api/v1/portfolios/{portfolio}"),
            Some("caller-b"),
            json!({"name": "renamed-by-b"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "no rename/move route exists");

    // (b) The only resource update route (asset metadata) cannot change
    // ownership: a `site_id` in the body is not part of the update contract
    // and the asset's ownership chain is immutable.
    let hostile_site = tenant_id(TENANT_B_SEED).uuid().to_hyphenated().to_string();
    let (status, body) = call(
        &router,
        patch_json(
            &format!("/api/v1/assets/{asset}"),
            Some("caller-a"),
            json!({
                "vendor_model": "model-x2",
                "rated_power_mw": 2.5,
                "rated_energy_mwh": 5.0,
                "capability_profile": "profile:storage",
                "site_id": hostile_site,
                "tenant_id": tenant_id(TENANT_B_SEED).uuid().to_hyphenated().to_string()
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "A's own metadata update succeeds");
    assert_eq!(body["data"]["site_id"], site, "the site binding is immutable");
    assert_eq!(body["data"]["tenant_id"], tenant_a, "the tenant binding is immutable");

    // (c) The delete-and-recreate path is closed too: B cannot delete A's
    // portfolio (404), so it can never be recreated under B.
    let (status, body) = call(
        &router,
        delete_req(&format!("/api/v1/portfolios/{portfolio}"), Some("caller-b")),
    )
    .await;
    assert_not_found_without_leak(status, &body, &[&portfolio]);
    let (status, _) = call(
        &router,
        get(&format!("/api/v1/portfolios/{portfolio}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "A's portfolio still exists");
}

// ---------------------------------------------------------------------------
// 3. Telemetry security: the read surface is tenant-scoped (P2.22 step 3)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn telemetry_reads_return_only_the_callers_own_tenant_data() {
    let (router, telemetry, _outbox) = composed_router();
    let (_portfolio, _site, asset) = setup_chain(&router).await;
    let tenant = tenant_id(TENANT_A_SEED);
    telemetry.seed(vec![sim_record(
        tenant,
        asset_uuid(&asset),
        TelemetryQuantity::Soc,
        QuantityValue::Fraction(flexcore_contracts::units::StateOfCharge::new(0.6).unwrap()),
        SEED_BASE_MILLIS,
        TelemetryQuality::Good,
        1,
    )]);

    // A reads its own asset's state: the seeded value comes back.
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/assets/{asset}/state"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let quantities = body["data"]["quantities"].as_array().expect("quantities");
    assert_eq!(quantities[0]["value_num"], 0.6);

    // B (and B's viewer) can never see it — the state read is 404.
    for token in ["caller-b", "viewer-b"] {
        let (status, body) = call(
            &router,
            get(&format!("/api/v1/assets/{asset}/state"), Some(token)),
        )
        .await;
        assert_not_found_without_leak(status, &body, &[&asset]);
    }

    // A's own telemetry query returns the record; the same query by B is
    // 404 (the asset must exist in the CALLER's registry before telemetry
    // is read — the recorded P2.12 rule).
    let uri = format!(
        "/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"
    );
    let (status, body) = call(&router, get(&uri, Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("records").len(), 1);
    let (status, body) = call(&router, get(&uri, Some("caller-b"))).await;
    assert_not_found_without_leak(status, &body, &[&asset]);
}

// ---------------------------------------------------------------------------
// 4. Deny-by-default holds (P2.22; no test relies on a role bypass)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deny_by_default_holds_across_the_phase2_surface() {
    let (router, _telemetry, _outbox) = composed_router();
    let (portfolio, site, asset) = setup_chain(&router).await;

    // Unauthenticated callers are 401 on every Phase 2 route (no bearer, no
    // anonymous fallback).
    for (uri, method) in [
        (format!("/api/v1/assets/{asset}"), "DELETE"),
        (format!("/api/v1/assets/{asset}/state"), "GET"),
        ("/api/v1/telemetry/quarantine".to_string(), "GET"),
        (
            format!("/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"),
            "GET",
        ),
        (format!("/api/v1/portfolios/{portfolio}/view"), "GET"),
    ] {
        let request = match method {
            "DELETE" => delete_req(&uri, None),
            _ => get(&uri, None),
        };
        let (status, body) = call(&router, request).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(body["error"]["code"], "unauthenticated");
    }

    // Role grants are explicit, not hierarchical: TenantAdmin (admin on its
    // own tenant) is STILL denied the platform-only tenant.delete action.
    let (status, body) = call(
        &router,
        delete_req(
            &format!(
                "/api/v1/tenants/{}",
                tenant_id(TENANT_A_SEED).uuid().to_hyphenated()
            ),
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "tenant.delete is PlatformAdmin-only");
    assert_eq!(body["error"]["code"], "forbidden");

    // The viewer role is denied every write and the admin-only quarantine
    // read — even on its OWN tenant's resources.
    for request in [
        delete_req(&format!("/api/v1/assets/{asset}"), Some("viewer")),
        patch_json(
            &format!("/api/v1/assets/{asset}"),
            Some("viewer"),
            json!({"vendor_model": "v", "rated_power_mw": 1.0, "capability_profile": "p"}),
        ),
        post_json(
            &format!("/api/v1/assets/{asset}/deactivate"),
            Some("viewer"),
            json!({"reason": "nope"}),
        ),
        get("/api/v1/telemetry/quarantine", Some("viewer")),
    ] {
        let (status, body) = call(&router, request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "viewer writes/quarantine deny");
        assert_eq!(body["error"]["code"], "forbidden");
    }

    // Least privilege still reads: the viewer CAN read its own tenant's
    // telemetry (telemetry.read is granted to every authenticated role —
    // the recorded grant design), which is not a bypass.
    let (status, _) = call(
        &router,
        get(
            &format!("/api/v1/telemetry/query?asset={asset}&from={QUERY_FROM}&until={QUERY_UNTIL}"),
            Some("viewer"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the viewer's own-tenant telemetry read is granted");
}

// ---------------------------------------------------------------------------
// 5. P2.21 acceptance: /metrics exposes all six telemetry metric families
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_metrics_scrape_exposes_all_six_telemetry_metric_families() {
    let (router, _telemetry, _outbox) = composed_router();
    let response = router
        .clone()
        .oneshot(get("/metrics", None))
        .await
        .expect("oneshot is infallible for this router");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    let body = String::from_utf8(bytes.to_vec()).expect("the metrics body is text");

    // The six P2.21 metric families (ledger acceptance: a metric inventory
    // where each family is asserted present).
    for family in [
        "flexcore_telemetry_ingestion_delay_milliseconds",
        "flexcore_telemetry_stale_assets",
        "flexcore_telemetry_queue_depth",
        "flexcore_telemetry_events_total",
        "flexcore_telemetry_quarantine_total",
        "flexcore_telemetry_quality_states_total",
    ] {
        assert!(body.contains(family), "missing metric family {family}");
    }
    // All six canonical quality states are observable (the P2.21 risk note:
    // quality counts missing STALE is the named failure mode).
    for state in ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"] {
        assert!(
            body.contains(&format!("quality=\"{state}\"")),
            "quality state {state} must be observable"
        );
    }
    // The frozen telemetry event types are the family's closed label set.
    for event in ["TelemetryReceived", "TelemetryQualityChanged", "AssetAvailabilityChanged"] {
        assert!(body.contains(&format!("event=\"{event}\"")), "event {event} observable");
    }
    // The Phase 1 families are still there (extended, not replaced).
    assert!(body.contains("flexcore_http_requests_total"));
    assert!(body.contains("flexcore_http_errors_total"));
}
