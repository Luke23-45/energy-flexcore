// P6.06 — API contract tests for the `POST /api/v1/edge/enroll` enrollment
//         seam (report §18.1, §17.3, §16.2; ledger P6.06; decision record
//         `06_07_identity_channel.md`) on the Test-variant backend.
//
// Composition rule (no network, no database — the P1.17 test pattern):
// the app state is built with the flexcore-ports fakes, the enrollment
// seam's in-crate fake store + static token table, and requests go through
// the REAL router via `tower::ServiceExt::oneshot`.
//
// Covered (the ledger P6.06 negative set + the honesty record):
// - **Fail-closed by default**: with NO token validator composed (the
//   shipped deny-all default) every enrollment is 401; a missing
//   Authorization header is 401; a garbage token is 401.
// - **The positive path**: a valid single-use token + approved binding →
//   201 with the identity RECORD — `certificate_issued: false` and the
//   DEPLOYMENT-REQUIREMENTS note verbatim (no certificate is fabricated).
// - **Binding discipline (§16.2)**: a body binding that does not match the
//   token's approved (tenant, site) scope is 403; a site that is not
//   approved in the tenant is 403 AND journaled as REJECTED.
// - **Identity uniqueness**: a second enrollment of an ACTIVE identity is
//   409 (`EDGE_IDENTITY_EXISTS`) and journaled as REJECTED.
// - **Revocation**: re-enrollment of a REVOKED identity is 403 and
//   journaled as REJECTED.
// - **Rotation**: re-enrollment after an EXPIRED term is allowed as a NEW
//   term (seq+1) with a ROTATED journal event.
// - **Token single-use**: the same token never enrolls twice (the seam
//   contract the deployment's real validator must implement).
// - **No store composed → infrastructure error** (fail closed, never a
//   fake success).
//
// UNRUN — written, never executed in this loop (the ENTRY_NOTE bans); the
// human build pass runs `cargo test -p flexcore-control-plane --test api_edge`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_edge::{
    ApprovedEnrollmentScope, FakeEdgeEnrollmentStore, StaticEnrollmentTokens,
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

fn tenant_context(seed: u128) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(tenant_uuid(seed)),
            subject: "p6.06-api-edge-test".to_string(),
        },
    ))
}

fn site_uuid(seed: u128) -> FlexUuid {
    FlexUuid::from_bytes(seed.to_be_bytes())
}

// ---------------------------------------------------------------------------
// Composition (the api_alarms.rs pattern, plus the enrollment seam)
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

/// Composes the router with the enrollment seam: a static single-use token
/// table and the fake store, with `site_a` pre-approved for tenant A.
fn composed_router(
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

fn approved_scope_a() -> ApprovedEnrollmentScope {
    ApprovedEnrollmentScope {
        tenant_id: tenant_uuid(TENANT_A_SEED),
        site_id: site_uuid(0xA1),
    }
}

fn approved_scope_b() -> ApprovedEnrollmentScope {
    ApprovedEnrollmentScope {
        tenant_id: tenant_uuid(TENANT_B_SEED),
        site_id: site_uuid(0xB1),
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
// Fail-closed by default (the shipped deny-all validator)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enrollment_is_denied_without_a_composed_token_validator() {
    let store = Arc::new(FakeEdgeEnrollmentStore::new());
    let state = base_state().with_edge_enrollment(store);
    let router = stage1_router(state);
    let (status, body) = post_enroll(
        router,
        Some("any-token"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[tokio::test]
async fn a_missing_authorization_header_is_401() {
    let (router, _store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (status, body) = post_enroll(
        router,
        None,
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[tokio::test]
async fn an_unknown_token_is_401() {
    let (router, _store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (status, _body) = post_enroll(
        router,
        Some("not-the-token"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// The positive path + the honesty record
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_valid_token_and_approved_binding_enrolls_with_an_honest_record() {
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (status, body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["device_id"], "edge-01");
    assert_eq!(body["data"]["enrollment_seq"], 1);
    assert_eq!(body["data"]["status"], "ACTIVE");
    // The honesty record: NO certificate is issued by this seam.
    assert_eq!(body["data"]["certificate_issued"], false);
    let note = body["data"]["record_note"].as_str().expect("note present");
    assert!(
        note.contains("DEPLOYMENT REQUIREMENTS"),
        "the record note names the deployment requirements verbatim"
    );

    // The journal recorded the ENROLLED event.
    let events = store.events();
    assert!(events
        .iter()
        .any(|e| e.event_type == "ENROLLED" && e.device_id == "edge-01"));
}

#[tokio::test]
async fn the_provisioning_token_is_single_use() {
    let (router, _store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (first, _) = post_enroll(
        router.clone(),
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    // The same token, a DIFFERENT device: the token is consumed — 401.
    let (second, body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-02"),
    )
    .await;
    assert_eq!(second, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

// ---------------------------------------------------------------------------
// The negative paths (the ledger P6.06 fail-closed matrix)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_binding_outside_the_token_scope_is_403() {
    // The token is bound to tenant A's scope; the body claims tenant B.
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_B_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(store.events().is_empty(), "no journal event for a scope mismatch");
}

#[tokio::test]
async fn a_site_not_approved_in_the_tenant_is_403_and_journaled() {
    // Tenant B's token + a site never approved for tenant B.
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_b())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_B_SEED), site_uuid(0xB1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let events = store.events();
    assert!(events
        .iter()
        .any(|e| e.event_type == "REJECTED" && e.device_id == "edge-01"),
        "the refusal is journaled (§17.3)");
}

#[tokio::test]
async fn a_duplicate_active_identity_is_409_and_journaled() {
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (first, _) = post_enroll(
        router.clone(),
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    // Second token, same device: 409 with the stable code.
    let (second, body) = post_enroll(
        router,
        Some("t-2"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "EDGE_IDENTITY_EXISTS");
    assert!(store
        .events()
        .iter()
        .any(|e| e.event_type == "REJECTED"),
        "the duplicate attempt is journaled");
}

#[tokio::test]
async fn a_revoked_identity_refuses_re_enrollment_and_is_journaled() {
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    // Seed a REVOKED identity term directly (the control-plane state).
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
    store.enroll_device(&tenant_context(TENANT_A_SEED), &revoked).expect("seeded");

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
        .any(|e| e.event_type == "REJECTED" && e.device_id == "edge-revoked"),
        "the revoked re-enrollment attempt is journaled");
}

#[tokio::test]
async fn re_enrollment_after_expiry_rotates_to_a_new_term() {
    let (router, store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let expired = EdgeDeviceIdentityRow::new(
        TenantId::new(tenant_uuid(TENANT_A_SEED)),
        "edge-expired".to_string(),
        site_uuid(0xA1).to_hyphenated().to_string(),
        1,
        "EXPIRED".to_string(),
        None,
        1_700_000_000_000,
        Some(1_700_000_000_000 + 86_400_000),
    )
    .expect("the row builds");
    store.enroll_device(&tenant_context(TENANT_A_SEED), &expired).expect("seeded");

    let (status, body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-expired"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["enrollment_seq"], 2, "rotation is a NEW term");
    assert!(store
        .events()
        .iter()
        .any(|e| e.event_type == "ROTATED" && e.device_id == "edge-expired"),
        "the rotation is journaled");
}

#[tokio::test]
async fn a_malformed_body_is_422() {
    let (router, _store) = composed_router(vec![("t-1".to_string(), approved_scope_a())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        json!({
            "device_id": "   ",
            "tenant_id": tenant_uuid(TENANT_A_SEED).to_hyphenated().to_string(),
            "site_id": site_uuid(0xA1).to_hyphenated().to_string(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn without_a_composed_store_the_route_fails_closed() {
    // Validator composed, store NOT composed: infrastructure error — never
    // a fake success.
    let state = base_state()
        .with_enrollment_tokens(Arc::new(StaticEnrollmentTokens::new(vec![(
            "t-1".to_string(),
            approved_scope_a(),
        )])));
    let router = stage1_router(state);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
