// P6.23 — Security testing, auth-bypass category, at the SERVER (report
//         §19.2(7) auth bypass; §17.4; ledger P6.23 step 3). Written, NOT
//         run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human
//         build/test pass runs
//         `cargo test -p flexcore-control-plane --test api_security_p6_23`.
//
// THE ATTACK SURFACE: the ONLY server-facing edge endpoint in v0.4.0 is
// `POST /api/v1/edge/enroll` (routes_edge.rs — the recorded fail-closed
// matrix). Every case below drives the REAL router (`stage1_router`)
// directly, exactly as a malicious client would — no edge goodwill, no
// authenticated helper: a hostile HTTP request in, the HTTP refusal plus
// the audited denial out.
//
// Cases (the P6.23 step-3 negative set):
//
// | # | Attack | Expected |
// |---|---|---|
// | 1 | unauthenticated edge-session request (no Authorization header) | 401, nothing journaled as an enrollment |
// | 2 | invalid/garbage bearer token | 401 |
// | 3 | CONSUMED (single-use) token replay | 401 — the replayed credential is dead at the seam |
// | 4 | EXPIRED token (the deployment validator refuses) | 401 |
// | 5 | wrong-TENANT token: token scope tenant A, request claims tenant B | 403, nothing journaled as an enrollment |
// | 6 | right tenant, foreign SITE | 403 + REJECTED journal event (actor, object, timestamp, correlation id) |
// | 7 | revoked identity re-enrollment | 403 + REJECTED journal event |
// | 8 | malformed device identity | 422, fail-closed before any store write |
//
// The token EXPIRY/REVOCATION enforcement lives in the
// [`ProvisioningTokenValidator`] seam (the deployment composes its real
// single-use token service there — the shipped default is DenyAll). The
// expired-token case is therefore modelled honestly: a test-local validator
// that refuses with `InvalidCredentials`, proving the ROUTE fails closed on
// the refusal — the route never second-guesses or retries a refused
// credential.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_edge::{
    ApprovedEnrollmentScope, EnrollmentAuthError, FakeEdgeEnrollmentStore,
    ProvisioningTokenValidator, StaticEnrollmentTokens,
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
            subject: "p6.23-api-security-test".to_string(),
        },
    ))
}

// ---------------------------------------------------------------------------
// Composition (the api_edge_isolation_p6_21.rs pattern — real router, no
// network, no DB)
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

fn edge_a_scope() -> ApprovedEnrollmentScope {
    ApprovedEnrollmentScope {
        tenant_id: tenant_uuid(TENANT_A_SEED),
        site_id: site_uuid(0xA1),
    }
}

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
// 1–2. Unauthenticated and invalid credentials fail closed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_unauthenticated_edge_session_request_is_401_and_never_journaled_as_an_enrollment() {
    // THE ATTACK: a hostile client hits the edge enrollment surface with
    // NO credential at all.
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _body) = post_enroll(
        router,
        None,
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        store.events().is_empty(),
        "an unauthenticated request produces NO enrollment journal event: {:?}",
        store.events()
    );
}

#[tokio::test]
async fn a_garbage_bearer_token_is_401() {
    // THE ATTACK: a fabricated bearer value (not a provisioning token).
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _body) = post_enroll(
        router,
        Some("not-a-real-provisioning-token"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(store.events().is_empty());
}

// ---------------------------------------------------------------------------
// 3–4. Consumed and expired credentials fail closed at the seam
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_consumed_single_use_token_replay_is_401() {
    // THE ATTACK: the token was captured after legitimate use and replayed
    // (the replay attack at the credential layer). Single-use semantics at
    // the seam kill it; the route fail-closes on the refusal.
    let (router, store) =
        composed_router(vec![("t-1".to_string(), edge_a_scope()), ("t-2".to_string(), edge_a_scope())]);
    let (first, _) = post_enroll(
        router.clone(),
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    // The REPLAY of the SAME token string (even for a DIFFERENT device):
    let (replay, _) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-02"),
    )
    .await;
    assert_eq!(replay, StatusCode::UNAUTHORIZED, "a consumed token is dead");
    // The successful first enrollment is journaled; the replay adds
    // nothing.
    let events = store.events();
    assert!(events.iter().any(|e| e.event_type == "ENROLLED" && e.device_id == "edge-01"));
    assert!(!events.iter().any(|e| e.device_id == "edge-02"));
}

/// A test-local validator modelling an EXPIRED credential: the deployment's
/// real token service would check validity against its own clock and
/// refuse with `InvalidCredentials`. The route must fail closed on that
/// refusal — 401, no scope assigned, nothing journaled as an enrollment.
struct ExpiredTokenValidator;

impl ProvisioningTokenValidator for ExpiredTokenValidator {
    fn validate(
        &self,
        bearer: Option<&str>,
    ) -> Result<ApprovedEnrollmentScope, EnrollmentAuthError> {
        match bearer {
            Some(_) => Err(EnrollmentAuthError::InvalidCredentials),
            None => Err(EnrollmentAuthError::MissingCredentials),
        }
    }
}

#[tokio::test]
async fn an_expired_token_is_401_and_assigns_no_scope() {
    let store = Arc::new(FakeEdgeEnrollmentStore::new());
    store.approve_site(
        TenantId::new(tenant_uuid(TENANT_A_SEED)),
        site_uuid(0xA1).to_hyphenated().to_string(),
    );
    let state = base_state()
        .with_edge_enrollment(store.clone())
        .with_enrollment_tokens(Arc::new(ExpiredTokenValidator));
    let router = stage1_router(state);
    let (status, _body) = post_enroll(
        router,
        Some("expired-but-well-formed-token"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-01"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(store.events().is_empty(), "an expired credential enrolls nothing");
}

// ---------------------------------------------------------------------------
// 5–6. Wrong-tenant and foreign-site bindings fail closed (and audit)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_wrong_tenant_token_is_403_and_never_journaled_as_an_enrollment() {
    // THE ATTACK: a TENANT A token presented with a TENANT B binding —
    // the request tries to widen the credential's approved scope.
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_B_SEED), site_uuid(0xA1), "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(store.events().is_empty());
}

#[tokio::test]
async fn a_foreign_site_within_the_tenant_is_403_and_the_denial_is_audited() {
    // THE ATTACK: right tenant, invented site — the audited denial carries
    // actor, object, timestamp, and correlation id (§17.3).
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let foreign_site = site_uuid(0xDEAD);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), foreign_site, "edge-hostile"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let rejection = store
        .events()
        .iter()
        .find(|e| e.event_type == "REJECTED" && e.device_id == "edge-hostile")
        .expect("the foreign-site denial is journaled");
    assert_eq!(rejection.actor, "edge:enrollment:edge-hostile");
    assert_eq!(rejection.tenant_id, TenantId::new(tenant_uuid(TENANT_A_SEED)));
    assert!(rejection.occurred_at_unix_millis > 0, "the denial is timestamped");
    assert!(!rejection.correlation_id.is_empty());
    assert!(
        rejection.reason.as_deref().unwrap_or("").contains("not an approved site"),
        "the denial names its cause: {:?}",
        rejection.reason
    );
}

// ---------------------------------------------------------------------------
// 7. Revoked identity re-enrollment fails closed (and audits)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_revoked_identity_cannot_re_enroll_and_the_attempt_is_audited() {
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    // Seed a REVOKED identity term (the server-side state a revocation
    // decision produced).
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
    // THE ATTACK: the revoked edge re-enrolls with a fresh valid token.
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-revoked"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let rejection = store
        .events()
        .iter()
        .find(|e| e.event_type == "REJECTED" && e.device_id == "edge-revoked")
        .expect("the revoked re-enrollment attempt is journaled");
    assert_eq!(rejection.actor, "edge:enrollment:edge-revoked");
    assert!(rejection.occurred_at_unix_millis > 0);
    assert!(
        rejection
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("REVOKED"),
        "the denial names the revocation: {:?}",
        rejection.reason
    );
}

// ---------------------------------------------------------------------------
// 8. Malformed identity fails closed before any store write
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_malformed_device_identity_is_422_and_writes_nothing() {
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let (status, _body) = post_enroll(
        router,
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "   "),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // No registry row, no journal event: the malformed attempt never
    // reaches persistence.
    assert!(store.events().is_empty());
}

#[tokio::test]
async fn a_malformed_tenant_or_site_uuid_is_422() {
    let (router, store) = composed_router(vec![("t-1".to_string(), edge_a_scope())]);
    let bad_tenant = json!({
        "device_id": "edge-01",
        "tenant_id": "not-a-uuid",
        "site_id": site_uuid(0xA1).to_hyphenated().to_string(),
    });
    let (status, _body) = post_enroll(router, Some("t-1"), bad_tenant).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let bad_site = json!({
        "device_id": "edge-01",
        "tenant_id": tenant_uuid(TENANT_A_SEED).to_hyphenated().to_string(),
        "site_id": "../../etc/passwd",
    });
    let (status, _body) = post_enroll(router, Some("t-1"), bad_site).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(store.events().is_empty());
}

// ---------------------------------------------------------------------------
// The audited-denial ledger: every 403 refusal on a state decision is
// journaled with actor + object + timestamp (the §17.3 rule, proven
// collectively).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn every_state_decision_refusal_carries_actor_object_and_timestamp() {
    let (router, store) = composed_router(vec![
        ("t-1".to_string(), edge_a_scope()),
        ("t-2".to_string(), edge_a_scope()),
    ]);
    // Duplicate ACTIVE identity (409): an audited REJECTED decision.
    let (first, _) = post_enroll(
        router.clone(),
        Some("t-1"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-dup"),
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    let (second, _) = post_enroll(
        router,
        Some("t-2"),
        enroll_body(tenant_uuid(TENANT_A_SEED), site_uuid(0xA1), "edge-dup"),
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);
    let rejections: Vec<_> = store
        .events()
        .iter()
        .filter(|e| e.event_type == "REJECTED")
        .collect();
    assert!(
        !rejections.is_empty(),
        "the duplicate-identity refusal is journaled"
    );
    for rejection in rejections {
        assert!(!rejection.actor.is_empty(), "actor present: {rejection:?}");
        assert!(!rejection.device_id.is_empty(), "object present: {rejection:?}");
        assert!(rejection.occurred_at_unix_millis > 0, "timestamp present");
        assert!(!rejection.correlation_id.is_empty(), "correlation id present");
        assert_eq!(
            rejection.tenant_id,
            TenantId::new(tenant_uuid(TENANT_A_SEED)),
            "the journal is tenant-scoped"
        );
    }
}
