// FIM wave 8B (Agent B) — the pinned MACHINE-IDENTITY and API-KEY route
// suite: the wiring wave's named remaining step ("NO ROUTE serves the
// service yet") is now its own surface, pinned from the composition outward
// (`tests/fim_composition.rs` conventions carried forward).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE (`cargo test -p flexcore-control-plane --test
// fim_identity_routes`). Nothing here is a live-interop result: a real
// integration partner driving these endpoints is a **HUMAN LAB GATE**.
//
// WHAT THIS FILE PROVES (the wave-8B pins):
//
// 1. GATE OFF ⇒ NO ROUTES EXIST. A state composed WITHOUT the
//    machine-identity / API-key services (exactly what the wiring wave
//    left: the `FLEXCORE_MACHINE_IDENTITY_ENABLED` /
//    `FLEXCORE_API_KEYS_ENABLED` sections absent ⇒ slots `None`)
//    contributes NO identity routes at all — the paths 404 because they
//    do not exist, never a 404-fallback of a mounted-but-denied surface.
// 2. GATE ON ⇒ THE ROUND TRIPS RUN THROUGH THE REAL SERVICES (socket-free
//    via `tower::ServiceExt::oneshot`, the fim_composition conventions):
//    machine identity — register → token issuance → validation →
//    revocation → the pre-revocation token REFUSES; API keys — issue →
//    self-authentication → rotation → revocation → both secrets refuse.
// 3. A REVOKED CLIENT IS REFUSED — the credential-epoch registry check
//    refuses the issued-before-revocation token at validation.
// 4. KEY ROTATION GRACE + LAST-USED — the superseded secret keeps
//    authenticating until the configured grace window closes (then the
//    named refusal becomes a bare 401), the new secret is unaffected, and
//    the last-used evidence (timestamp + subject path + ALLOW) lands on
//    the key row on every successful authentication.
// 5. THE NEVER-GRANTED SCOPES ARE REFUSED BY THE ROUTES — a registration /
//    issuance naming `dispatch.authorize` / `network.any` is a 422 and
//    NOTHING is written (the frozen never-granted permission set, pinned
//    at the route layer exactly as the service pins it).
// 6. THE RAW KEY APPEARS EXACTLY ONCE — in the issue response (and once
//    more at each rotation, for the NEW literal), marked
//    `secret_handling: "shown-once"` with the no-store cache posture, and
//    NEVER in any subsequent response (the self-authentication, rotation,
//    and revocation bodies carry fingerprints and public fields only).
// 7. THE SHIPPED DENY-BY-DEFAULT POSTURE IS UNCHANGED — under the shipped
//    policy composition (`phase5_default_policies`, which grants none of
//    the `apikey.*` actions) the admin routes deny for an authenticated
//    administrator (403). The round trips below compose the RECORDED
//    phase-13 grant set (`flexcore_authz::phase13_policies`) — the grant
//    table the P13.08 record defines for exactly these surfaces.
//
// AUTH NOTE (recorded): the admin endpoints authenticate through the
// standard `CallerExtractor` path, so the suite injects a fixed
// `IdentityResolver` returning a TenantAdmin caller (the tests/api.rs
// convention — the dev resolver is feature-gated and never a test
// dependency). The machine-identity token/validate endpoints need NO
// caller: the client's own credentials / the presented token are the
// credentials. The machine-identity service uses the SYSTEM clock (the
// recorded test decision in `machine_identity.rs`: jsonwebtoken validates
// `exp` against the process clock); the API-key service uses a shared
// controllable clock (the recorded injected-clock discipline in
// `api_keys.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use flexcore_authz::phase13_policies::phase13_policies;
use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::api_keys::{ApiKeyConfig, ApiKeyService, ApiKeyStore, MemoryApiKeyStore};
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver};
use flexcore_control_plane::machine_identity::{
    MachineIdentityConfig, MachineIdentityService, MachineClientStore, MemoryMachineClientStore,
};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::{fim_router, stage1_router};
use flexcore_control_plane::state::AppState;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FakeWriteJournal, FixedClock,
};
use serde_json::json;
use tower::ServiceExt;

const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const CALLER_BEARER: &str = "fim-identity-routes-caller-token";
const MACHINE_CLIENT_ID: &str = "grid-operator-1";
const MACHINE_CLIENT_SECRET: &str = "test-only-registration-secret-not-a-credential";
const REGISTER_PATH: &str = "/api/v1/machine-identity/clients";
const TOKEN_PATH: &str = "/api/v1/machine-identity/token";
const VALIDATE_PATH: &str = "/api/v1/machine-identity/validate";
const REVOKE_PATH: &str = "/api/v1/machine-identity/clients/grid-operator-1/revoke";
const KEYS_PATH: &str = "/api/v1/api-keys";
const KEY_ROTATE_PREFIX: &str = "/api/v1/api-keys";
const KEY_SELF_PATH: &str = "/api/v1/api-keys/self";

fn tenant_a() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_A_UUID).expect("valid test uuid"))
}

// --- Composition helpers (the fim_composition.rs conventions) ----------------

/// The fixed caller the admin endpoints authenticate with (the tests/api.rs
/// convention; the dev resolver is feature-gated).
struct FixedIdentityResolver {
    caller: Caller,
}

impl IdentityResolver for FixedIdentityResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        match bearer {
            Some(token) if token == format!("Bearer {CALLER_BEARER}") => Ok(self.caller.clone()),
            Some(_) => Err(AuthError::InvalidCredentials),
            None => Err(AuthError::MissingCredentials),
        }
    }
}

fn tenant_admin_caller() -> Caller {
    Caller {
        ctx: TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant_a(),
                subject: "fim-identity-routes-test-admin".to_string(),
            },
        )),
        role: Role::TenantAdmin,
    }
}

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

/// The Test-backend state with NO identity service composed — the
/// absent-configuration posture (the wiring wave's fail-closed default).
/// `policies` selects the policy set (the recorded phase-13 grant set for
/// the round-trip tests; the shipped default for the deny pin).
fn base_state_with_policies(policies: flexcore_authz::policy::PolicySet) -> AppState {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    AppState::for_tests(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal.clone())),
        outbox,
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        policies,
        Arc::new(FixedIdentityResolver {
            caller: tenant_admin_caller(),
        }),
        passing_readiness(),
        None,
        300_000,
    )
}

/// The machine-identity service over a SHARED registry handle (the repo's
/// shared-fake pattern — the test inspects the same map the service uses).
/// The clock is the SYSTEM clock (the recorded test decision in
/// `machine_identity.rs`: jsonwebtoken validates `exp` against the process
/// clock, so a fixed past clock would expire every issued token).
fn machine_service(registry: MemoryMachineClientStore) -> Arc<MachineIdentityService> {
    fn system_clock_millis() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after the epoch")
            .as_millis() as i64
    }
    Arc::new(MachineIdentityService::new(
        MachineIdentityConfig {
            issuer: "https://control-plane.test/token".to_string(),
            audience: "flexcore-api".to_string(),
            ..MachineIdentityConfig::default()
        },
        b"fim-identity-routes-test-issuer-key",
        Box::new(registry),
        Box::new(system_clock_millis),
    ))
}

/// The API-key service over a SHARED store handle and a SHARED controllable
/// clock (the recorded injected-clock discipline of `api_keys.rs`).
fn api_key_service(
    store: &MemoryApiKeyStore,
    clock: &Arc<AtomicI64>,
) -> Arc<ApiKeyService> {
    let clock_handle = clock.clone();
    Arc::new(ApiKeyService::new(
        ApiKeyConfig {
            rotation_grace_seconds: 3_600,
        },
        Box::new(store.clone()),
        Box::new(move || clock_handle.load(Ordering::SeqCst)),
    ))
}

fn router(state: &AppState) -> axum::Router {
    stage1_router(state.clone()).merge(fim_router(state))
}

fn bare_request(method: &str, uri: &str, bearer: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("valid request")
}

fn json_request(method: &str, uri: &str, bearer: Option<&str>, body: serde_json::Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("valid request")
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    serde_json::from_slice(&bytes).expect("the surface answers JSON")
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    String::from_utf8(bytes.to_vec()).expect("bodies are UTF-8")
}

// --- 1. Gate off: no identity routes exist at all -----------------------------

#[tokio::test]
async fn gates_off_mount_no_identity_routes_at_all() {
    let state = base_state_with_policies(phase13_policies());
    assert!(state.machine_identity.is_none(), "no machine-identity service is composed by default");
    assert!(state.api_keys.is_none(), "no API-key service is composed by default");
    let app = router(&state);
    for (method, uri) in [
        ("POST", REGISTER_PATH),
        ("POST", TOKEN_PATH),
        ("GET", VALIDATE_PATH),
        ("POST", REVOKE_PATH),
        ("POST", KEYS_PATH),
        ("POST", "/api/v1/api-keys/some-key-id/rotate"),
        ("POST", "/api/v1/api-keys/some-key-id/revoke"),
        ("GET", KEY_SELF_PATH),
    ] {
        let response = app
            .clone()
            .oneshot(bare_request(method, uri, None))
            .await
            .expect("oneshot is infallible");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {uri} must not exist when the gates are off — the paths do not \
             exist, they are not a mounted-but-denied fallback"
        );
    }
}

// --- 2. The machine-identity round trip through the real service --------------

#[tokio::test]
async fn the_machine_identity_round_trip_runs_through_the_real_service() {
    let registry = MemoryMachineClientStore::new();
    let state =
        base_state_with_policies(phase13_policies()).with_machine_identity(machine_service(registry.clone()));
    let app = router(&state);

    // 1. REGISTER (the admin caller; the response carries the record's
    //    public fields and the 4-character display prefix — never the
    //    secret).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            Some(CALLER_BEARER),
            json!({
                "client_id": MACHINE_CLIENT_ID,
                "allowed_scopes": ["telemetry.read"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::CREATED, "registration");
    let registered = json_body(response).await;
    assert_eq!(registered["data"]["client_id"], MACHINE_CLIENT_ID);
    assert_eq!(registered["data"]["tenant_id"], TENANT_A_UUID);
    assert_eq!(registered["data"]["allowed_scopes"], json!(["telemetry.read"]));
    assert_eq!(registered["data"]["enabled"], true);
    assert_eq!(registered["data"]["credential_epoch"], 0);
    assert_eq!(registered["data"]["secret_prefix"], "test");
    // The response never echoes the secret beyond its display prefix.
    assert!(
        !registered.to_string().contains(MACHINE_CLIENT_SECRET),
        "the registration response never echoes the secret: {registered}"
    );

    // 2. ISSUE a token through the CLIENT's own credentials (record 07 §2).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            TOKEN_PATH,
            None,
            json!({
                "grant_type": "client_credentials",
                "client_id": MACHINE_CLIENT_ID,
                "client_secret": MACHINE_CLIENT_SECRET,
                "tenant_id": TENANT_A_UUID,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "token issuance");
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .expect("the token response carries the no-store posture"),
        "no-store"
    );
    let token_body = json_body(response).await;
    assert_eq!(token_body["token_type"], "Bearer");
    assert_eq!(token_body["expires_in"], 900, "the registered TTL");
    let token = token_body["access_token"]
        .as_str()
        .expect("the access token is present")
        .to_string();
    assert!(!token.is_empty());

    // 3. VALIDATE the token (the token IS the credential): the principal is
    //    the REGISTRY's tenant and scope subset.
    let response = app
        .clone()
        .oneshot(bare_request("GET", VALIDATE_PATH, Some(token.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "validation");
    let principal = json_body(response).await;
    assert_eq!(principal["data"]["client_id"], MACHINE_CLIENT_ID);
    assert_eq!(principal["data"]["subject"], "integration:grid-operator-1");
    assert_eq!(principal["data"]["tenant_id"], TENANT_A_UUID);
    assert_eq!(principal["data"]["scopes"], json!(["telemetry.read"]));

    // 4. A WRONG SECRET at the token endpoint is ONE bare 401 (no oracle).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            TOKEN_PATH,
            None,
            json!({
                "grant_type": "client_credentials",
                "client_id": MACHINE_CLIENT_ID,
                "client_secret": "a-different-secret",
                "tenant_id": TENANT_A_UUID,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 5. REVOKE (the admin caller; the emergency kill-switch).
    let response = app
        .clone()
        .oneshot(bare_request("POST", REVOKE_PATH, Some(CALLER_BEARER)))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "revocation");
    let revoked = json_body(response).await;
    assert_eq!(revoked["data"]["client_id"], MACHINE_CLIENT_ID);
    assert_eq!(revoked["data"]["state"], "disabled");

    // 6. THE PRE-REVOCATION TOKEN REFUSES at validation (the registry's
    //    credential-epoch check runs before any scope grant is consulted).
    let response = app
        .clone()
        .oneshot(bare_request("GET", VALIDATE_PATH, Some(token.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "the revoked client's token refuses immediately"
    );
    // And issuance refuses too (the disabled state).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            TOKEN_PATH,
            None,
            json!({
                "grant_type": "client_credentials",
                "client_id": MACHINE_CLIENT_ID,
                "client_secret": MACHINE_CLIENT_SECRET,
                "tenant_id": TENANT_A_UUID,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// --- 3. The API-key round trip: issue once, grace, last-used, revoke ----------

#[tokio::test]
async fn the_api_key_round_trip_runs_through_the_real_service() {
    let store = MemoryApiKeyStore::new();
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let state = base_state_with_policies(phase13_policies())
        .with_api_keys(api_key_service(&store, &clock));
    let app = router(&state);

    // 1. ISSUE: the raw literal appears EXACTLY ONCE, marked shown-once,
    //    with the no-store posture.
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            KEYS_PATH,
            Some(CALLER_BEARER),
            json!({ "scopes": ["telemetry.read"] }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::CREATED, "issuance");
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .expect("the issue response carries the no-store posture"),
        "no-store"
    );
    let issue_text = body_text(response).await;
    let issued: serde_json::Value = serde_json::from_str(&issue_text).expect("issue JSON");
    let key_id = issued["data"]["key_id"].as_str().expect("key id").to_string();
    let literal = issued["data"]["secret_literal"]
        .as_str()
        .expect("the raw literal is present")
        .to_string();
    assert_eq!(issued["data"]["secret_handling"], "shown-once");
    assert_eq!(issued["data"]["scopes"], json!(["telemetry.read"]));
    assert_eq!(
        issue_text.matches(&literal).count(),
        1,
        "the raw key appears EXACTLY ONCE in the issue response"
    );
    assert!(literal.starts_with("fk_"), "the record 08 §2 literal shape");

    // 2. SELF-AUTHENTICATE with the literal: the principal resolves, the
    //    response carries NO literal, and the last-used evidence lands on
    //    the key row.
    let response = app
        .clone()
        .oneshot(bare_request("GET", KEY_SELF_PATH, Some(literal.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "self-authentication");
    let self_text = body_text(response).await;
    assert!(
        !self_text.contains(&literal),
        "no subsequent response ever carries the raw literal: {self_text}"
    );
    let principal: serde_json::Value = serde_json::from_str(&self_text).expect("principal JSON");
    assert_eq!(principal["data"]["key_id"], key_id);
    assert_eq!(principal["data"]["tenant_id"], TENANT_A_UUID);
    assert_eq!(principal["data"]["scopes"], json!(["telemetry.read"]));
    let row = store
        .get(&tenant_a(), &key_id)
        .expect("the store reads")
        .expect("the key row exists");
    let evidence = row.last_used.expect("last-used evidence recorded on success");
    assert_eq!(evidence.last_used_subject_path, KEY_SELF_PATH);
    assert_eq!(evidence.last_used_result, "ALLOW");
    assert_eq!(row.use_count, 1);

    // 3. ROTATE: the NEW literal appears exactly once; the OLD literal is
    //    absent from the rotate response.
    let rotate_uri = format!("{KEY_ROTATE_PREFIX}/{key_id}/rotate");
    let response = app
        .clone()
        .oneshot(bare_request("POST", &rotate_uri, Some(CALLER_BEARER)))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "rotation");
    let rotate_text = body_text(response).await;
    let rotated: serde_json::Value = serde_json::from_str(&rotate_text).expect("rotate JSON");
    let new_literal = rotated["data"]["secret_literal"]
        .as_str()
        .expect("the rotated literal is present")
        .to_string();
    assert_eq!(rotated["data"]["secret_handling"], "shown-once");
    assert_ne!(new_literal, literal, "rotation mints a NEW secret");
    assert_eq!(
        rotate_text.matches(&new_literal).count(),
        1,
        "the rotated raw key appears EXACTLY ONCE"
    );
    assert!(
        !rotate_text.contains(&literal),
        "the OLD literal never appears in a subsequent response"
    );

    // 4. ROTATION GRACE: inside the window BOTH secrets authenticate.
    let response = app
        .clone()
        .oneshot(bare_request("GET", KEY_SELF_PATH, Some(new_literal.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "the new secret");
    let response = app
        .clone()
        .oneshot(bare_request("GET", KEY_SELF_PATH, Some(literal.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the superseded secret still authenticates inside the grace window"
    );

    // 5. PAST THE GRACE the superseded secret refuses (ONE bare 401) and
    //    the new secret is unaffected.
    clock.fetch_add(3_600 * 1_000 * 2, Ordering::SeqCst);
    let response = app
        .clone()
        .oneshot(bare_request("GET", KEY_SELF_PATH, Some(literal.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "the superseded secret refuses past the grace window"
    );
    let response = app
        .clone()
        .oneshot(bare_request("GET", KEY_SELF_PATH, Some(new_literal.as_str())))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "the new secret unaffected");

    // 6. REVOKE: BOTH secrets refuse immediately (record 08 §2: no
    //    works-until-refresh window for a revoked key).
    let revoke_uri = format!("{KEY_ROTATE_PREFIX}/{key_id}/revoke");
    let response = app
        .clone()
        .oneshot(bare_request("POST", &revoke_uri, Some(CALLER_BEARER)))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK, "revocation");
    let revoked = json_body(response).await;
    assert_eq!(revoked["data"]["state"], "REVOKED");
    assert!(
        !revoked.to_string().contains(&literal) && !revoked.to_string().contains(&new_literal),
        "the revocation response carries no credential material"
    );
    for presented in [literal.as_str(), new_literal.as_str()] {
        let response = app
            .clone()
            .oneshot(bare_request("GET", KEY_SELF_PATH, Some(presented)))
            .await
            .expect("oneshot is infallible");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "a revoked key refuses immediately for both secrets"
        );
    }
}

// --- 4. The never-granted scopes are refused by the routes --------------------

#[tokio::test]
async fn the_never_granted_scope_names_are_refused_by_the_routes() {
    let registry = MemoryMachineClientStore::new();
    let store = MemoryApiKeyStore::new();
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let state = base_state_with_policies(phase13_policies())
        .with_machine_identity(machine_service(registry.clone()))
        .with_api_keys(api_key_service(&store, &clock));
    let app = router(&state);

    // Machine-client registration naming a never-granted scope: 422, and
    // NOTHING was written (the frozen never-granted set stays refused at
    // the route layer exactly as the service refuses it).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            Some(CALLER_BEARER),
            json!({
                "client_id": "overreaching-1",
                "allowed_scopes": ["dispatch.authorize"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        registry
            .get(&tenant_a(), "overreaching-1")
            .expect("the registry reads")
            .is_none(),
        "nothing was written for a refused registration"
    );
    // A mixed list refuses too (the service refuses at the bad index).
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            Some(CALLER_BEARER),
            json!({
                "client_id": "overreaching-2",
                "allowed_scopes": ["telemetry.read", "secret.read"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(registry
        .get(&tenant_a(), "overreaching-2")
        .expect("the registry reads")
        .is_none());

    // API-key issuance naming a never-granted scope: 422, nothing written.
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            KEYS_PATH,
            Some(CALLER_BEARER),
            json!({ "scopes": ["network.any"] }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Nothing was written: the only lookup key a refused issuance could
    // have left is a verifier digest, and the store's verifier lookup
    // finds nothing (the api_keys.rs unit-suite's established pin).
    assert!(store
        .get_by_verifier("")
        .expect("the store reads")
        .is_none());
}

// --- 5. The shipped deny-by-default posture is unchanged ----------------------

#[tokio::test]
async fn the_shipped_policy_composition_denies_the_admin_surfaces() {
    // Under the SHIPPED policy set (phase5_default_policies grants none of
    // the apikey.* actions — FRV1-R2-009 composes phase13_policies
    // deliberately NOT) the admin routes deny for an authenticated
    // administrator: the surface exists, authorization stays
    // deny-by-default. The round trips above compose the RECORDED phase-13
    // grant set.
    let registry = MemoryMachineClientStore::new();
    let store = MemoryApiKeyStore::new();
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let state = base_state_with_policies(flexcore_control_plane::auth::phase5_default_policies())
        .with_machine_identity(machine_service(registry.clone()))
        .with_api_keys(api_key_service(&store, &clock));
    let app = router(&state);
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            KEYS_PATH,
            Some(CALLER_BEARER),
            json!({ "scopes": ["telemetry.read"] }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "deny-by-default preserved under the shipped policy composition"
    );
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            Some(CALLER_BEARER),
            json!({
                "client_id": MACHINE_CLIENT_ID,
                "allowed_scopes": ["telemetry.read"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // Nothing was written through the denied surfaces.
    assert!(registry
        .get(&tenant_a(), MACHINE_CLIENT_ID)
        .expect("the registry reads")
        .is_none());
    assert!(store
        .get(&tenant_a(), "any-key")
        .expect("the store reads")
        .is_none());
}

// --- 6. The unauthenticated caller is refused (the 401 rule) ------------------

#[tokio::test]
async fn the_admin_surfaces_require_a_resolved_caller() {
    // The resolver refuses unknown credentials (the fixed test resolver):
    // a present-but-invalid bearer is a 401, never a fabricated context
    // (P0.16 rule 5).
    let registry = MemoryMachineClientStore::new();
    let state = base_state_with_policies(phase13_policies())
        .with_machine_identity(machine_service(registry.clone()));
    let app = router(&state);
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            Some("not-the-caller-token"),
            json!({
                "client_id": MACHINE_CLIENT_ID,
                "allowed_scopes": ["telemetry.read"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            REGISTER_PATH,
            None,
            json!({
                "client_id": MACHINE_CLIENT_ID,
                "allowed_scopes": ["telemetry.read"],
                "secret": MACHINE_CLIENT_SECRET,
            }),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // Nothing was written for either refusal.
    assert!(registry
        .get(&tenant_a(), MACHINE_CLIENT_ID)
        .expect("the registry reads")
        .is_none());
}
