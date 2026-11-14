// P6.21 — Multi-site and multi-tenant isolation at the edge: SERVER-SIDE
//         tests (report §16.2, §17.4, §17.5 rule 8; ledger P6.21). Written,
//         NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human
//         build/test pass runs
//         `cargo test -p flexcore-control-plane --test api_edge_isolation_p6_21`.
//
// DEFENCE IN DEPTH RULE (§16.2): the server scopes every edge-session
// request by the edge's ENROLLED identity — tenant + site from the
// enrollment store (the authoritative binding), never from any
// request-carried field. NO test here relies on edge goodwill: every case
// drives the REAL router (`stage1_router`) directly, exactly as a malicious
// client would — forged bindings, foreign sites, duplicated identities,
// revoked identities — and asserts the HTTP refusal AND the audited denial
// (actor, object, timestamp in the append-only enrollment journal,
// migration 0018).
//
// The server-side enforcement that exists in v0.4.0 (recorded honestly in
// the decision document): the ONLY server-facing edge endpoint is
// `POST /api/v1/edge/enroll`, and its scope discipline is exactly the
// isolation rule — the binding is checked against the provisioning token's
// SERVER-side approved scope, the site against the tenant's approved sites,
// and the identity against the tenant-scoped registry; a request-carried
// tenant/site that disagrees with the server-side scope is refused (403)
// and, where the refusal is a state decision, journaled. Future edge
// session endpoints MUST resolve their scope through
// `routes_edge::resolve_edge_session_scope` (the additive hardening this
// wave adds: tenant from the authenticated context, site from the store
// row, no request parameter) — the tests below prove it, including that a
// cross-tenant identity is INVISIBLE to the resolver.
//
// The RLS note (§16.2/[R25], recorded in the decision document): migration
// 0018 puts RLS ENABLE + FORCE with the shared tenant_isolation policy on
// `edge_device_registry` and `edge_enrollment_journal`, and grants
// flexcore_app INSERT+SELECT only; the app-role ownership review (the
// application role must not own tenant tables or bypass RLS) is a HUMAN
// GATE INPUT and is recorded as UNRUN here — these Test-backend tests
// cannot exercise RLS.
//
// The modified-edge-client acceptance case (a maliciously MODIFIED edge
// binary) CANNOT run in this loop — it is recorded as a human gate input
// with an EMPTY result. These tests cover the same attack surface from the
// protocol side: a malicious client is just a client that sends hostile
// requests, which is what the router receives below.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_edge::{
    resolve_edge_session_scope, ApprovedEnrollmentScope, EdgeSessionScopeError,
    FakeEdgeEnrollmentStore, StaticEnrollmentTokens,
};
use flexcore_control_plane::state::AppState;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FakeWriteJournal, FixedClock,
};
use flexcore_storage::edge_enrollment::{EdgeDeviceIdentityRow, EdgeEnrollmentStore as _};
use serde_json::{json, Value};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;

fn tenant_uuid(seed: u128) -> FlexUuid {
    FlexUuid::from_bytes(seed.to_be_bytes())
}

fn site_uuid(seed: u128) -> FlexUuid {
    FlexUuid::from_bytes(seed.to_be_bytes())
}

fn tenant_context(seed: u128) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(tenant_uuid(seed)),
            subject: "p6.21-api-isolation-test".to_string(),
        },
    ))
}

// ---------------------------------------------------------------------------
// Composition (the api_edge.rs pattern — real router, no network, no DB)
// ---------------------------------------------------------------------------

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

fn base_state() -> AppState {
    let journal = FakeWriteJournal::new();
    AppState::for_tests(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal)),
        Arc::new(FakeEventOutbox::new(journal.clone())),
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        flexcore_control_plane::auth::phase5_default_policies(),
        Arc::new(flexcore_control_plane::auth::DenyAllResolver),
        passing_readiness(),
        None,
        300_000,
    )
}

/// The edge-A composition: tenant A's site A1 pre-approved, a single-use
/// token bound to exactly that scope (the server-side binding — the
/// request can never widen it).
fn composed_router_for_edge_a(
    tokens: Vec<(String, ApprovedEnrollmentScope)>,
) -> (Router, Arc<FakeEdgeEnrollmentStore>) {
    let store = Arc::new(FakeEdgeEnrollmentStore::new());
    store.approve_site(
        TenantId::new(tenant_uuid(TENANT_A_SEED)),
        site_uuid(0xA1).to_hyphenated().to_string(),
    );
    let state = base_state()
        .with_edge_enrollment(store.clone())
        .with_enrollment_tokens(Arc::new(StaticEnrollmentTokens::new(tokens)));
    (stage1_router(state), store)
}

fn edge_a_scope() -> ApprovedEnrollmentScope {
    ApprovedEnrollmentScope {
        tenant_id: tenant_uuid(TENANT_A_SEED),
        site_id: site_uuid(0xA1),
    }
}

async fn post_enroll(router: Router, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/edge/enroll")
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .oneshot(
            builder
                .body(Body::from(body.to_string()))
                .expect("request builds"),
        )
        .await
        .expect("the router serves");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("body reads");
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn enroll_body(tenant: FlexUuid, site: FlexUuid, device: &str) -> Value {
    json!({
        "device_id": device,
        "tenant_id": tenant.to_hyphenated().to_string(),
        "site_id": site.to_hyphenated().to_string(),
    })
}

// ---------------------------------------------------------------------------
// 1. Cross-tenant/site enrollment attempts (driven as a malicious client)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_cross_tenant_binding_is_403_and_never_journaled_as_an_enrollment() {
    // Edge A's token (tenant A scope); the request claims TENANT B. The
    // server scopes against the token's approved scope, not the request.
    let (router, store) = composed_router_for_edge_a(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_B_SEED), site_uuid(0xA1), "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        store.events().is_empty(),
        "a refused cross-tenant binding produces NO enrollment journal event"
    );
}

#[tokio::test]
async fn a_foreign_site_within_the_token_tenant_is_403_and_audited() {
    // Tenant A token; the request claims a site that is NOT approved in
    // tenant A (an invented/foreign site id). 403 + journaled REJECTED.
    let (router, store) = composed_router_for_edge_a(vec![("t-1".to_string(), edge_a_scope())]);
    let foreign_site = site_uuid(0xDEAD);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), foreign_site, "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let events = store.events();
    let rejection = events
        .iter()
        .find(|e| e.event_type == "REJECTED" && e.device_id == "edge-hostile")
        .expect("the refusal is journaled");
    // The audited denial carries actor, object, and timestamp (§17.3).
    assert_eq!(rejection.actor, "edge:enrollment:edge-hostile");
    assert_eq!(rejection.tenant_id, TenantId::new(tenant_uuid(TENANT_A_SEED)));
    assert!(rejection.occurred_at_unix_millis > 0, "the denial is timestamped");
    assert!(!rejection.correlation_id.is_empty());
    assert!(rejection.reason.as_deref().unwrap_or("").contains("not an approved site"));
}

// ---------------------------------------------------------------------------
// 2. Identity-state attacks (duplicate, revoked) — 403/409 + audited denial
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_duplicate_active_identity_is_409_and_the_attempt_is_audited() {
    let (router, store) = composed_router_for_edge_a(vec![
        ("t-1".to_string(), edge_a_scope()),
        ("t-2".to_string(), edge_a_scope()),
    ]);
    let (first, _) = post_enroll(
        router.clone(),
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    // A second, DIFFERENT credential attempting the SAME identity: no two
    // edges share an identity (409) and the attempt is journaled.
    let (second, body) = post_enroll(
        router,
        Some("t-2"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "EDGE_IDENTITY_EXISTS");
    let events = store.events();
    let rejection = events
        .iter()
        .find(|e| e.event_type == "REJECTED" && e.device_id == "edge-01")
        .expect("the duplicate attempt is journaled");
    assert_eq!(rejection.actor, "edge:enrollment:edge-01");
    assert!(rejection.occurred_at_unix_millis > 0);
}

#[tokio::test]
async fn a_revoked_identity_refuses_the_session_scope_and_the_re_enrollment() {
    let (router, store) = composed_router_for_edge_a(vec![("t-1".to_string(), edge_a_scope())]);
    // Seed a REVOKED identity term directly (the server-side state).
    let revoked = EdgeDeviceIdentityRow::new(
        TenantId::new(tenant_uuid(TENANT_A_SEED)),
        "edge-revoked".to_string(),
        site_uuid(0xA1).to_hyphenated().to_string(),
        1,
        "REVOKED".to_string(),
        None,
        1_700_000_000_000,
        Some(1_700_000_000_000 + 86_400_000),
    )
    .expect("the row builds");
    store
        .enroll_device(&tenant_context(TENANT_A_SEED), &revoked)
        .expect("seeded");

    // Re-enrollment of the revoked identity: 403 + journaled.
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-revoked"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(store
        .events()
        .iter()
        .any(|e| e.event_type == "REJECTED" && e.device_id == "edge-revoked"));

    // AND the session-scope resolver refuses the same identity: a revoked
    // edge has NO scope (fail-closed, defence in depth).
    let error = resolve_edge_session_scope(&*store, &tenant_context(TENANT_A_SEED), "edge-revoked")
        .expect_err("a revoked identity has no session scope");
    assert!(matches!(
        error,
        EdgeSessionScopeError::IdentityNotActive { status, .. } if status == "REVOKED"
    ));
}

// ---------------------------------------------------------------------------
// 3. The session scope comes from the STORE, never from the request
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_server_derives_the_session_scope_from_the_enrollment_store() {
    let (router, store) = composed_router_for_edge_a(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // The scope resolver takes NO request-carried scope field: the tenant
    // comes from the authenticated context and the site from the store row.
    let scope = resolve_edge_session_scope(&*store, &tenant_context(TENANT_A_SEED), "edge-01")
        .expect("an ACTIVE identity resolves");
    assert_eq!(scope.site_id, site_uuid(0xA1).to_hyphenated().to_string());
    assert_eq!(scope.tenant_id, TenantId::new(tenant_uuid(TENANT_A_SEED)));
    assert_eq!(scope.device_id, "edge-01");
}

#[tokio::test]
async fn a_cross_tenant_identity_is_invisible_to_the_server_side_scope() {
    // Edge B is enrolled under TENANT B. A caller holding a TENANT A
    // context (a compromised/malicious client) CANNOT resolve it: the
    // tenant-scoped store returns nothing — existence never leaks (P0.16
    // rule 5), and the edge's own goodwill plays no part.
    let store = Arc::new(FakeEdgeEnrollmentStore::new());
    store.approve_site(
        TenantId::new(tenant_uuid(TENANT_B_SEED)),
        site_uuid(0xB1).to_hyphenated().to_string(),
    );
    let state = base_state()
        .with_edge_enrollment(store.clone())
        .with_enrollment_tokens(Arc::new(StaticEnrollmentTokens::new(vec![(
            "t-b".to_string(),
            ApprovedEnrollmentScope {
                tenant_id: tenant_uuid(TENANT_B_SEED),
                site_id: site_uuid(0xB1),
            },
        )])));
    let router = stage1_router(state);
    let (status, _) = post_enroll(
        router,
        Some("t-b"),
        enroll_body(tenant_uuid(TENANT_B_SEED), site_uuid(0xB1), "edge-b"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Tenant A's context: edge B is INVISIBLE.
    let error = resolve_edge_session_scope(&*store, &tenant_context(TENANT_A_SEED), "edge-b")
        .expect_err("a cross-tenant identity never resolves");
    assert!(matches!(error, EdgeSessionScopeError::UnknownIdentity { .. }));
    // Tenant B's own context resolves it normally.
    let scope = resolve_edge_session_scope(&*store, &tenant_context(TENANT_B_SEED), "edge-b")
        .expect("the owning tenant resolves its own edge");
    assert_eq!(scope.site_id, site_uuid(0xB1).to_hyphenated().to_string());
}

#[tokio::test]
async fn an_unenrolled_identity_has_no_session_scope() {
    // An identity that was never enrolled (or a store that cannot resolve
    // it) yields NO scope: the resolver maps the failed lookup to the
    // fail-closed UnknownIdentity refusal — never a guessed, defaulted, or
    // widened scope (the fail-closed default).
    let store = Arc::new(FakeEdgeEnrollmentStore::new());
    let error = resolve_edge_session_scope(&*store, &tenant_context(TENANT_A_SEED), "ghost")
        .expect_err("an unenrolled identity has no scope");
    assert!(matches!(error, EdgeSessionScopeError::UnknownIdentity { .. }));
}
