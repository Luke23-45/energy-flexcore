// FIM Wave 6.5 (Agent A) — the COMPOSITION-PINNING suite: the
// configuration-conditional wiring of the Wave 4-6 services at the
// composition root, pinned from the LIB side (the additive
// `AppState::with_*` builders and `routes.rs::fim_router`).
//
// WHAT THIS FILE PROVES:
//
// 1. ABSENT CONFIGURATION ⇒ EVERY FAIL-CLOSED DEFAULT. A state composed
//    without any FIM service carries exactly the pre-FIM posture: the
//    webhook transport REFUSES every transmission with the named
//    `E_TRANSPORT_UNWIRED` error (`UnwiredOutboundHttpPost` — the FRV1-R8-005
//    seam), and the SCIM / step-up / machine-identity / API-key / OTA slots
//    are all `None` (no service is constructed).
// 2. SCIM OFF ⇒ NO SCIM ROUTES EXIST AT ALL. `fim_router` over a state
//    without a composed SCIM service contributes NO routes (the paths 404
//    because they do not exist — never a 404-fallback of a mounted-but-
//    denied surface).
// 3. SCIM ON ⇒ MOUNTED BEHIND THE BEARER GUARD. A state composed with
//    `with_scim(service, digest)` serves the SCIM 2.0 endpoints, and EVERY
//    request without the exact bearer token is a bare 401 (missing,
//    malformed, and wrong credentials are indistinguishable). The SCIM
//    surface itself stays tenant-pinned (the service is composed for ONE
//    tenant).
// 4. PRESENT CONFIGURATION ⇒ THE LIVE SERVICES CONSTRUCT. The production
//    constructors the composition root uses — `ReqwestOutboundHttpPost`,
//    `OidcResolver::compose` (+ `HttpJwksSource` + `OidcDeploymentConfig` +
//    `StaticTenantMapper`), `OidcStepUpVerifier` inside `StepUpService`,
//    `ScimAwareIdentityResolver`, `MachineIdentityService`,
//    `ApiKeyService`, and the OTA `OtaRolloutCoordinator` over `build_plan`
//    — all construct WITHOUT ANY NETWORK ACTIVITY (the JWKS fetch is lazy
//    on the first token; the webhook client transmits only on `post`), and
//    the DENY defaults are not weakened by the wrapping: a
//    `ScimAwareIdentityResolver` over `DenyAllResolver` denies BEFORE any
//    SCIM state is consulted.
// 5. THE SHIPPED DENY DEFAULTS ARE UNCHANGED as types: `DenyAllResolver`
//    refuses every request with `MissingCredentials` (the p10_04 suite
//    pins the full matrix; this file pins the composition-context cell),
//    and `DenyAllStepUpVerifier` denies every verification.
//
// SOCKET-FREE / DETERMINISTIC: every construction below is proven (by the
// module headers of `oidc.rs` / `webhook_http.rs` / `stepup.rs`) to perform
// NO network activity at construction — the only HTTP calls this suite
// makes are the in-process `tower::ServiceExt::oneshot` invocations. No
// listener, no socket, no IdP. WRITTEN-UNRUN (the ENTRY_NOTE boundary; the
// human build/test gate owns:
// `cargo test -p flexcore-control-plane --test fim_composition`).
//
// NAMED REMAINING WIRING (recorded honestly; NOT testable here because no
// route exists): the machine-identity and API-key services are composed
// into the state WITHOUT a serving route; the OTA report receiver is a
// TYPED port, not HTTP — no route serves device reports. Both are the
// composition-root notes in `main.rs::fim_compose` and `state.rs`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::api_keys::{ApiKeyConfig, ApiKeyService, MemoryApiKeyStore};
use flexcore_control_plane::auth::{
    parse_role, phase5_default_policies, AuthError, DenyAllResolver, IdentityResolver,
};
use flexcore_control_plane::machine_identity::{
    MachineIdentityConfig, MachineIdentityService, MemoryMachineClientStore,
};
use flexcore_control_plane::oidc::{
    HttpJwksSource, OidcDeploymentConfig, OidcResolver, StaticTenantMapper,
    DEFAULT_JWKS_FETCH_TIMEOUT,
};
use flexcore_control_plane::ota_rollout::{
    build_plan, MemoryOtaRolloutAuditSink, OtaCohortDefinition, OtaRolloutCoordinator,
};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::{fim_router, stage1_router};
use flexcore_control_plane::scim::{ScimAwareIdentityResolver, ScimService};
use flexcore_control_plane::scim_store::InMemoryScimStore;
use flexcore_control_plane::state::AppState;
use flexcore_control_plane::stepup::{
    DenyAllStepUpVerifier, OidcStepUpVerifier, StepUpPurpose, StepUpService,
    DEFAULT_STEP_UP_EVIDENCE_MAX_AGE, DEFAULT_STEP_UP_TTL,
};
use flexcore_control_plane::webhook_http::{ReqwestOutboundHttpPost, WebhookHttpConfig};
use flexcore_control_plane::webhook_service::{OutboundHttpPost as _, WebhookDeliveryError};
use flexcore_application::rollout_orchestrator::{AbortThresholds, MemoryRolloutAuditSink};
use flexcore_audit::chain::ChainKey;
use flexcore_domain::ids::TenantId;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTelemetryReader, FakeTenantRepository, FakeWriteJournal,
    FixedClock,
};
use tower::ServiceExt;

const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const SCIM_BEARER_TOKEN: &str = "fim-composition-scim-secret";

fn tenant_a() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_A_UUID).expect("valid test uuid"))
}

// --- Composition helpers -----------------------------------------------------

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

/// The Test-backend state with NO FIM composition — the absent-
/// configuration posture (the same shape the composition root builds when
/// every FIM section is absent).
fn default_state() -> AppState {
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
        phase5_default_policies(),
        Arc::new(DenyAllResolver),
        passing_readiness(),
        None,
        300_000,
    )
}

fn scim_service() -> Arc<ScimService> {
    let ids_counter = Arc::new(AtomicI64::new(0));
    let clock = Arc::new(AtomicI64::new(1_000));
    Arc::new(ScimService::new(
        Arc::new(InMemoryScimStore::default()),
        tenant_a(),
        Box::new(move || {
            format!(
                "scim-{}",
                ids_counter.fetch_add(1, Ordering::SeqCst)
            )
        }),
        Box::new(move || clock.load(Ordering::SeqCst)),
    ))
}

fn request(method: &str, uri: &str, bearer: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("valid request")
}

// --- 1. Absent configuration: every fail-closed default -----------------------

#[test]
fn an_uncomposed_state_carries_the_unwired_webhook_transport() {
    let state = default_state();
    let refused = state
        .webhook_transport
        .post("https://hooks.example/endpoint", &[], b"{}".as_slice())
        .expect_err("the unwired seam refuses every transmission");
    match refused {
        WebhookDeliveryError::Backend { code, message } => {
            assert_eq!(code, "E_TRANSPORT_UNWIRED");
            assert!(message.contains("unwired transport seam"));
        }
        other => panic!("expected the unwired backend refusal, got {other:?}"),
    }
}

#[test]
fn an_uncomposed_state_has_every_fim_service_slot_none() {
    let state = default_state();
    assert!(state.scim.is_none(), "no SCIM service is composed by default");
    assert!(
        state.scim_bearer_digest.is_none(),
        "no SCIM guard material exists without a connection"
    );
    assert!(
        state.step_up.is_none(),
        "no step-up service is composed by default (every step-up request fails closed)"
    );
    assert!(
        state.machine_identity.is_none(),
        "no machine-identity service is composed by default"
    );
    assert!(state.api_keys.is_none(), "no API-key service is composed by default");
    assert!(
        state.ota_rollout.is_none(),
        "no OTA rollout exists by default"
    );
}

#[tokio::test]
async fn scim_off_mounts_no_scim_routes_at_all() {
    let state = default_state();
    // The stage-1 router alone never had SCIM routes ...
    let stage1 = stage1_router(state.clone());
    let response = stage1
        .oneshot(request("GET", "/scim/v2/ServiceProviderConfig", None))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    // ... and the FIM router contributes NOTHING when the composition is
    // absent — a 404 because the path does not exist, never a mounted-but-
    // denied fallback.
    let router = stage1_router(state.clone()).merge(fim_router(&state));
    for (method, uri) in [
        ("GET", "/scim/v2/ServiceProviderConfig"),
        ("GET", "/scim/v2/Users"),
        ("POST", "/scim/v2/Users"),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, uri, None))
            .await
            .expect("oneshot is infallible");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {uri} must not exist when SCIM is off"
        );
    }
}

// --- 2. SCIM on: mounted behind the bearer guard ------------------------------

#[tokio::test]
async fn scim_on_serves_the_config_endpoint_through_the_guard() {
    let digest = flexcore_control_plane::webhook_signing::sha256(SCIM_BEARER_TOKEN.as_bytes());
    let state = default_state().with_scim(scim_service(), digest);
    let router = stage1_router(state.clone()).merge(fim_router(&state));

    // Missing credential: bare 401.
    let response = router
        .clone()
        .oneshot(request("GET", "/scim/v2/ServiceProviderConfig", None))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // WRONG credential: the same bare 401 (no oracle distinguishing the
    // refusal shapes).
    let response = router
        .clone()
        .oneshot(request(
            "GET",
            "/scim/v2/ServiceProviderConfig",
            Some("not-the-secret"),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // The exact token: the surface serves.
    let response = router
        .clone()
        .oneshot(request(
            "GET",
            "/scim/v2/ServiceProviderConfig",
            Some(SCIM_BEARER_TOKEN),
        ))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK);

    // The list endpoint over the EMPTY store: a SCIM ListResponse with zero
    // resources (the tenant-pinned service; deterministic).
    let response = router
        .clone()
        .oneshot(request("GET", "/scim/v2/Users", Some(SCIM_BEARER_TOKEN)))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    let body: serde_json::Value =
        serde_json::from_slice(&bytes).expect("SCIM answers JSON");
    assert_eq!(body["totalResults"], 0);
    assert_eq!(body["Resources"], serde_json::json!([]));
}

#[tokio::test]
async fn scim_miscomposition_mounts_nothing() {
    // The guard refuses to serve a surface whose digest half is missing —
    // fail-closed on miscomposition (the `with_scim` builder makes the
    // pair atomic, so this state can only exist through direct field
    // manipulation; the pin documents the router's defensive posture).
    let mut state = default_state();
    state.scim = Some(scim_service());
    state.scim_bearer_digest = None;
    let router = fim_router(&state);
    let response = router
        .oneshot(request("GET", "/scim/v2/ServiceProviderConfig", Some(SCIM_BEARER_TOKEN)))
        .await
        .expect("oneshot is infallible");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- 3. Present configuration: the live services construct --------------------

#[test]
fn the_live_webhook_transport_constructs_without_network() {
    // Construction builds the reqwest client and contacts NOTHING (the
    // first transmission happens only on `post` — the live receiver
    // interop is the HUMAN LAB GATE, FIM-F08.03).
    let transport =
        ReqwestOutboundHttpPost::new(WebhookHttpConfig::default())
            .expect("the live transport constructs with the registered budgets");
    let _ = transport;
}

#[test]
fn the_oidc_identity_path_composes_and_denies_before_any_fetch() {
    let deployment = OidcDeploymentConfig {
        issuer: "https://idp.example/realms/flexcore".to_string(),
        jwks_url: "https://idp.example/realms/flexcore/protocol/openid-connect/certs"
            .to_string(),
        audience: "flexcore-control-plane".to_string(),
        expected_client_id: None,
        jwks_fetch_timeout: DEFAULT_JWKS_FETCH_TIMEOUT,
    };
    // Empty bindings = the mapper that maps nothing (deny-by-default at the
    // mapper — never a default tenant).
    let resolver = OidcResolver::compose(
        &deployment,
        Box::new(StaticTenantMapper::new(HashMap::new())),
        Box::new(|| 1_700_000_000_000),
    )
    .expect("composition performs NO network activity");
    // A missing bearer refuses BEFORE any JWKS fetch (socket-free).
    assert!(matches!(
        IdentityResolver::resolve(&resolver, None),
        Err(AuthError::MissingCredentials)
    ));
}

#[test]
fn the_scim_aware_wrap_never_widens_the_deny_all_default() {
    let store: Arc<dyn flexcore_control_plane::scim_store::ScimStore> =
        Arc::new(InMemoryScimStore::default());
    let resolver = ScimAwareIdentityResolver::new(
        Box::new(DenyAllResolver),
        store,
        tenant_a(),
    );
    // The inner path denies FIRST (before any SCIM state is consulted) —
    // the wrap adds enforcement, never a bypass.
    assert!(matches!(
        resolver.resolve(Some("Bearer anything")),
        Err(AuthError::MissingCredentials)
    ));
    assert!(matches!(
        resolver.resolve(None),
        Err(AuthError::MissingCredentials)
    ));
}

#[test]
fn the_step_up_service_composes_with_the_oidc_verifier_and_fails_closed() {
    let source = HttpJwksSource::new(
        "https://idp.example/realms/flexcore",
        "https://idp.example/realms/flexcore/protocol/openid-connect/certs",
        DEFAULT_JWKS_FETCH_TIMEOUT,
    )
    .expect("the JWKS source constructs with NO fetch");
    let verifier = OidcStepUpVerifier::new(
        flexcore_control_plane::oidc::OidcConfig {
            issuer: "https://idp.example/realms/flexcore".to_string(),
            audience: "flexcore-control-plane".to_string(),
            ..flexcore_control_plane::oidc::OidcConfig::default()
        },
        Box::new(source),
        DEFAULT_STEP_UP_EVIDENCE_MAX_AGE,
        // The P13-E-3 DEFAULT: trust the IdP's factor policy wholesale.
        Vec::new(),
        Box::new(|| 1_700_000_000_000),
    );
    let ids_counter = Arc::new(AtomicI64::new(0));
    let service = StepUpService::new(
        Box::new(flexcore_control_plane::stepup::InMemoryStepUpStore::default()),
        Box::new(verifier),
        Box::new(move || {
            flexcore_control_plane::stepup::id_from_uuid(
                FlexUuid::from_bytes(
                    (ids_counter.fetch_add(1, Ordering::SeqCst) as u128).to_be_bytes(),
                ),
            )
        }),
        Box::new(|| 1_000),
        DEFAULT_STEP_UP_TTL,
    );
    let purpose = StepUpPurpose::CommandApproval {
        command_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b03".to_string(),
    };
    // The composed service still FAILS CLOSED: a challenge id that was
    // never issued is a recorded NoChallenge denial — the APPROVED-gate
    // semantics are the service's, unchanged by the composition.
    assert!(
        service
            .verify(
                "never-issued-challenge",
                "user-1",
                tenant_a(),
                "not-a-jwt",
                &purpose,
            )
            .is_err(),
        "no challenge ⇒ no verification pass (the shipped gate semantics)"
    );
    // The IdP-delegated verifier rejects non-JWT evidence at header decode
    // — BEFORE any JWKS fetch (socket-free).
    let challenge = service
        .issue("user-1", tenant_a(), purpose)
        .expect("a challenge issues deterministically");
    assert!(
        service
            .verify(
                &challenge.challenge_id,
                "user-1",
                tenant_a(),
                "not-a-jwt",
                &StepUpPurpose::CommandApproval {
                    command_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b03".to_string()
                },
            )
            .is_err(),
        "non-JWT evidence is refused before any network"
    );
}

#[test]
fn the_deny_all_step_up_verifier_still_denies_everything() {
    // The shipped DEFAULT verifier composed behind the SAME service shape:
    // a challenge issues, and every verification through the deny-all
    // verifier is a recorded VerifierNotComposed denial.
    let service = StepUpService::new(
        Box::new(flexcore_control_plane::stepup::InMemoryStepUpStore::default()),
        Box::new(DenyAllStepUpVerifier),
        Box::new(|| flexcore_control_plane::stepup::id_from_uuid(FlexUuid::nil())),
        Box::new(|| 1_000),
        DEFAULT_STEP_UP_TTL,
    );
    let challenge = service
        .issue(
            "user-1",
            tenant_a(),
            StepUpPurpose::ConsoleAction {
                action_id: "apikey.rotate".to_string(),
            },
        )
        .expect("a challenge issues deterministically");
    let verdict = service.verify(
        &challenge.challenge_id,
        "user-1",
        tenant_a(),
        "any-evidence",
        &StepUpPurpose::ConsoleAction {
            action_id: "apikey.rotate".to_string(),
        },
    );
    assert!(verdict.is_err(), "the shipped default denies every verification");
}

#[test]
fn the_machine_identity_and_api_key_services_construct() {
    let machine = MachineIdentityService::new(
        MachineIdentityConfig {
            issuer: "https://machines.example".to_string(),
            audience: "flexcore-control-plane".to_string(),
            ..MachineIdentityConfig::default()
        },
        b"composition-test-issuer-key-material",
        Box::new(MemoryMachineClientStore::new()),
        Box::new(|| 1_000),
    );
    let api_keys = ApiKeyService::new(
        ApiKeyConfig::default(),
        Box::new(MemoryApiKeyStore::new()),
        Box::new(|| 1_000),
    );
    // Construction is the pin (sealed types; NO route serves either —
    // the named remaining wiring step).
    let _ = (machine, api_keys);
}

#[test]
fn the_ota_rollout_coordinator_constructs_over_a_valid_plan() {
    let (plan, definitions) = build_plan(
        "edge-2.0.0",
        vec![
            OtaCohortDefinition {
                cohort_id: "canary".to_string(),
                canary: true,
                device_ids: vec!["d1".to_string(), "d2".to_string()],
                max_failing_devices: None,
            },
            OtaCohortDefinition {
                cohort_id: "block-1".to_string(),
                canary: false,
                device_ids: vec!["d3".to_string()],
                max_failing_devices: None,
            },
        ],
        AbortThresholds {
            max_failure_rate_permille: 200,
            min_observations: 2,
        },
        500,
    )
    .expect("a valid plan builds");
    let clock: Arc<dyn Clock + Send + Sync> =
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000));
    // The chain key is DEPLOYMENT-SUPPLIED material (consumed, never
    // generated; the P18.03/P11.15 ceremonies remain HUMAN GATES) — this
    // test material is test-only.
    let chain_key = ChainKey::from_bytes(&[0x11u8; 32]).expect("32 bytes is the minimum");
    let coordinator = OtaRolloutCoordinator::new(
        plan,
        definitions,
        Box::new(Arc::new(MemoryRolloutAuditSink::new())),
        Arc::new(MemoryOtaRolloutAuditSink::new()),
        chain_key,
        clock,
    );
    assert_eq!(coordinator.target_version(), "edge-2.0.0");
}

#[test]
fn the_state_slots_compose_the_live_services() {
    // The same composition shape the root uses behind its configuration
    // gates, exercised against the Test backend.
    let digest = flexcore_control_plane::webhook_signing::sha256(SCIM_BEARER_TOKEN.as_bytes());
    let state = default_state()
        .with_scim(scim_service(), digest)
        .with_machine_identity(Arc::new(MachineIdentityService::new(
            MachineIdentityConfig::default(),
            b"composition-test-issuer-key-material",
            Box::new(MemoryMachineClientStore::new()),
            Box::new(|| 1_000),
        )))
        .with_api_keys(Arc::new(ApiKeyService::new(
            ApiKeyConfig::default(),
            Box::new(MemoryApiKeyStore::new()),
            Box::new(|| 1_000),
        )));
    assert!(state.scim.is_some());
    assert!(state.scim_bearer_digest.is_some());
    assert!(state.machine_identity.is_some());
    assert!(state.api_keys.is_some());
    // Untouched slots stay at their defaults.
    assert!(state.step_up.is_none());
    assert!(state.ota_rollout.is_none());
}

#[test]
fn the_role_vocabulary_closes_the_composition() {
    // The SCIM group→role mapping consumes the SAME closed vocabulary as
    // the login path; the composition must never widen it (a forged role
    // name parses to nothing).
    assert!(
        parse_role("superadmin").is_none(),
        "a role outside the closed vocabulary never resolves"
    );
    assert!(parse_role("viewer").is_some());
    assert!(parse_role("site-approver").is_some());
}
