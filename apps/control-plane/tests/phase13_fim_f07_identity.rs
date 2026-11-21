// FIM-F07.05 — the pinned identity suite for the FIM-F07 wave (Agent A;
// WRITTEN-UNRUN — never compiled or executed; execution is the human
// build/test gate, conventions §1 item 3).
//
// What this file pins (docs/releases/implementation_ledger/ledger_F07.md,
// FIM-F07.01/FIM-F07.03/FIM-F07.05):
//
// - wrong-ISSUER refusal and wrong-AUDIENCE refusal by the composed
//   production path (the P13.02 `OidcResolver` with a real signed token;
//   the refusal is the existing error vocabulary — `AuthError::InvalidCredentials`);
// - JWKS cache HIT/MISS through an in-memory `JwksSource` (a fresh cache
//   is reused without a second fetch; an unknown `kid` triggers exactly
//   ONE refresh — the recorded bounded anti-stampede posture);
// - resolver COMPOSITION: the OIDC resolver resolves a `TenantContext`
//   through the EXISTING `TenantClaimMapper`/role-mapping surfaces, and
//   `DenyAllResolver` REMAINS the shipped default (fail-closed pin);
// - step-up refusal WITHOUT fresh evidence and acceptance WITH fresh
//   evidence through the service API (the `OidcStepUpVerifier` freshness
//   policy + the `require_command_approval_evidence` APPROVED-gate helper
//   over the UNCHANGED `StepUpService::verify` semantics);
// - SCIM-independent, SOCKET-FREE, DETERMINISTIC: every test below runs
//   against in-memory doubles; the only signed tokens are test-minted
//   HMAC tokens (the algorithm allow-list is a config value — the tests
//   opt in to HS256 with an `oct` JWK so the FULL decode pipeline runs
//   without an RSA keypair; the DEFAULT allow-list stays RS256 and is
//   pinned by the in-module oidc tests). No network, no sockets, no
//   IdP.
//
// HONESTY (FIM-F07.06): the LIVE IdP interop (a real issuer, a real JWKS
// endpoint, a real IdP-minted token) is UNPROVEN — a HUMAN LAB GATE
// (P13.20; P13-E-1/P13-E-3 stay OPEN). The ONE live-gated test at the
// bottom of this file (`live_idp_jwks_document_fetches_and_parses`) is
// gated on `FLEXCORE_TEST_OIDC_ISSUER` + `FLEXCORE_TEST_OIDC_JWKS_URL`
// (the recorded gate variables) and SKIPS CLEANLY when they are unset; it
// drives the real `HttpJwksSource` fetch path and nothing else.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::auth::{AuthError, DenyAllResolver, IdentityResolver};
use flexcore_control_plane::oidc::{
    JwksSource, JwksSourceError, OidcConfig, OidcDeploymentConfig, OidcResolver,
    StaticTenantMapper, DEFAULT_JWKS_FETCH_TIMEOUT,
};
// The JWKS document type comes from the JWT crate directly (the oidc
// module's own import is private; the type is the crate's public API).
use jsonwebtoken::jwk::JwkSet;
use flexcore_control_plane::stepup::{
    id_from_uuid, require_command_approval_evidence, DenyAllStepUpVerifier,
    InMemoryStepUpStore, OidcStepUpVerifier, StepUpDenial, StepUpPurpose, StepUpService,
    StepUpVerifier, DEFAULT_STEP_UP_EVIDENCE_MAX_AGE, DEFAULT_STEP_UP_TTL,
};
use flexcore_domain::ids::TenantId;

const ISSUER: &str = "https://idp.example/realms/flexcore";
const AUDIENCE: &str = "flexcore-control-plane";
const COMMAND_ID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b03";
const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const KID: &str = "test-key";
const SECRET: &[u8] = b"fim-f07-test-signing-secret";

fn tenant() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_A_UUID).expect("valid test uuid"))
}

// --- Test-only signing (HS256 + an `oct` JWK; the DEFAULT allow-list is
// --- RS256 and is NOT weakened: the config below opts in per test) -------

/// base64url (RFC 4648 §5, no padding) — hand-rolled so the test needs no
/// extra dependency (FIM standing decision 1).
fn base64url_no_pad(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(n >> 18) as usize & 63] as char);
        out.push(CHARS[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[n as usize & 63] as char);
        }
    }
    out
}

/// The `oct`-JWK JWKS document carrying the test signing secret under
/// `kid = "test-key"`.
fn oct_jwks_json(kid: &str) -> String {
    format!(
        r#"{{"keys":[{{"kty":"oct","kid":"{kid}","use":"sig","alg":"HS256","k":"{}"}}]}}"#,
        base64url_no_pad(SECRET)
    )
}

fn parse_jwks(json_text: &str) -> JwkSet {
    serde_json::from_str(json_text).expect("the test JWKS document parses")
}

/// Signs the claims JSON with HS256 under the named key (the test stand-in
/// for an IdP-minted token).
fn sign_hs256(claims: serde_json::Value, kid: &str) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.kid = Some(kid.to_string());
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET),
    )
    .expect("the test token signs")
}

/// A deterministic "now" for the tokens: the seconds-since-epoch anchor
/// the token instants are computed from (real wall-clock, so `exp` in the
/// future and `auth_time` in the past are stable under the 60 s leeway).
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the test clock is after the epoch")
        .as_secs() as i64
}

/// A login-path token: subject, issuer, audience, groups, and a 10-minute
/// expiry; the `auth_time`/`iat` anchors are shared with the step-up
/// verifier's freshness window (`auth_time_offset_secs` is relative to
/// now — negative = in the past).
fn token(
    iss: &str,
    aud: serde_json::Value,
    sub: &str,
    groups: &[&str],
    auth_time_offset_secs: i64,
    kid: &str,
) -> String {
    let now = now_secs();
    sign_hs256(
        serde_json::json!({
            "iss": iss,
            "aud": aud,
            "sub": sub,
            "exp": now + 600,
            "iat": now + auth_time_offset_secs,
            "auth_time": now + auth_time_offset_secs,
            "groups": groups,
        }),
        kid,
    )
}

/// The `Bearer <token>` header value as the `IdentityResolver` trait
/// expects it (a temporary `String` kept alive for the call).
fn bearer(token_text: &str) -> Option<String> {
    Some(format!("Bearer {token_text}"))
}

// --- OIDC fixtures --------------------------------------------------------

/// A static in-memory `JwksSource` (the same shape the in-module oidc
/// tests use — the cache hit/miss logic under test never touches a socket).
struct StaticSource(JwkSet);

impl JwksSource for StaticSource {
    fn fetch(&self, _issuer: &str) -> Result<JwkSet, JwksSourceError> {
        Ok(self.0.clone())
    }
}

/// A counting source that serves ONE document (the cache-HIT half).
struct CountingSource {
    document: JwkSet,
    fetches: Arc<AtomicI64>,
}

impl JwksSource for CountingSource {
    fn fetch(&self, _issuer: &str) -> Result<JwkSet, JwksSourceError> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(self.document.clone())
    }
}

/// A counting source that serves a ROTATED document after the first fetch
/// (the key-rotation half of the cache hit/miss pin).
struct RotatingSource {
    first: JwkSet,
    second: JwkSet,
    fetches: Arc<AtomicI64>,
}

impl JwksSource for RotatingSource {
    fn fetch(&self, _issuer: &str) -> Result<JwkSet, JwksSourceError> {
        let n = self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(if n == 0 {
            self.first.clone()
        } else {
            self.second.clone()
        })
    }
}

fn hs256_config() -> OidcConfig {
    OidcConfig {
        issuer: ISSUER.to_string(),
        audience: AUDIENCE.to_string(),
        algorithms: vec![jsonwebtoken::Algorithm::HS256],
        ..OidcConfig::default()
    }
}

fn tenant_mapper() -> StaticTenantMapper {
    let mut bindings = HashMap::new();
    bindings.insert(ISSUER.to_string(), TENANT_A_UUID.to_string());
    StaticTenantMapper::new(bindings)
}

fn fixed_clock() -> Box<dyn Fn() -> i64 + Send + Sync> {
    Box::new(|| 1_000)
}

// --- FIM-F07.01: the composed production path -----------------------------

#[test]
fn composed_oidc_resolver_resolves_tenant_context_through_the_existing_surfaces() {
    let resolver = OidcResolver::new(
        hs256_config(),
        Box::new(StaticSource(parse_jwks(&oct_jwks_json(KID)))),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    let tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        KID,
    );
    let caller = resolver
        .resolve(bearer(&tok).as_deref())
        .expect("a correct token from the configured issuer resolves");
    // The TenantContext came through the EXISTING surfaces: the
    // StaticTenantMapper binding → TenantId, the closed role vocabulary →
    // the role, and the reviewed domain attestation path in between.
    assert_eq!(caller.ctx.tenant_id(), tenant());
    assert_eq!(caller.ctx.subject(), "user:alice");
    assert_eq!(caller.role, Role::TenantAdmin);
}

#[test]
fn a_token_from_the_wrong_issuer_is_refused() {
    let resolver = OidcResolver::new(
        hs256_config(),
        Box::new(StaticSource(parse_jwks(&oct_jwks_json(KID)))),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    // The token is VALIDLY SIGNED by the same key the JWKS publishes and
    // carries the right audience — only the issuer is wrong. The refusal
    // is the named InvalidCredentials (the existing error vocabulary).
    let tok = token(
        "https://evil.example/realms/other",
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        KID,
    );
    assert_eq!(
        resolver
            .resolve(bearer(&tok).as_deref())
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "a wrong-issuer token never resolves, however valid its signature"
    );
}

#[test]
fn a_token_for_the_wrong_audience_is_refused() {
    let resolver = OidcResolver::new(
        hs256_config(),
        Box::new(StaticSource(parse_jwks(&oct_jwks_json(KID)))),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    let tok = token(
        ISSUER,
        serde_json::json!("another-rp"),
        "user:alice",
        &["tenant-admin"],
        0,
        KID,
    );
    assert_eq!(
        resolver
            .resolve(bearer(&tok).as_deref())
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "a token minted for another relying party never resolves"
    );
}

#[test]
fn a_token_missing_the_expected_client_id_in_a_multi_audience_is_refused() {
    // The optional expected-client-id check (FIM-F07.01): with
    // `required_client_id` configured (a DISTINCT id from the required
    // audience), a token whose audience lists the audience but not the
    // client id is refused.
    let config = OidcConfig {
        audience: AUDIENCE.to_string(),
        required_client_id: Some("the-real-client-id".to_string()),
        ..hs256_config()
    };
    let resolver = OidcResolver::new(
        config,
        Box::new(StaticSource(parse_jwks(&oct_jwks_json(KID)))),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    let partial_aud = serde_json::json!(["another-rp", AUDIENCE]);
    let tok = token(ISSUER, partial_aud, "user:alice", &["tenant-admin"], 0, KID);
    assert_eq!(
        resolver
            .resolve(bearer(&tok).as_deref())
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "the audience restriction requires the client id IN ADDITION to the audience"
    );
    // The same token WITH the client id in the audience resolves.
    let full_aud = serde_json::json!(["another-rp", AUDIENCE, "the-real-client-id"]);
    let tok = token(ISSUER, full_aud, "user:alice", &["tenant-admin"], 0, KID);
    let caller = resolver
        .resolve(bearer(&tok).as_deref())
        .expect("the token naming both audiences resolves");
    assert_eq!(caller.ctx.tenant_id(), tenant());
}

#[test]
fn compose_builds_the_production_path_without_any_network() {
    // The composed path from deployment configuration: NO hardcoded URLs
    // (every value comes from the config), and NO network at construction
    // or at a bearer-less resolve (the first fetch is LAZY, driven by the
    // first token needing a key).
    let deployment = OidcDeploymentConfig {
        issuer: ISSUER.to_string(),
        jwks_url: "https://idp.example/realms/flexcore/protocol/openid-connect/certs".to_string(),
        audience: AUDIENCE.to_string(),
        expected_client_id: Some("the-real-client-id".to_string()),
        jwks_fetch_timeout: DEFAULT_JWKS_FETCH_TIMEOUT,
    };
    let resolver = OidcResolver::compose(&deployment, Box::new(tenant_mapper()), fixed_clock())
        .expect("the composed path builds");
    // No bearer: refused BEFORE any JWKS fetch could happen (a fetch here
    // would be a socket — the test stays socket-free by construction).
    assert_eq!(
        resolver
            .resolve(None)
            .expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
}

#[test]
fn jwks_cache_hit_reuses_the_fresh_entry_without_a_second_fetch() {
    let fetches = Arc::new(AtomicI64::new(0));
    let source = CountingSource {
        document: parse_jwks(&oct_jwks_json(KID)),
        fetches: Arc::clone(&fetches),
    };
    let resolver = OidcResolver::new(
        hs256_config(),
        Box::new(source),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    let tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        KID,
    );
    resolver
        .resolve(bearer(&tok).as_deref())
        .expect("the first resolve fetches the document");
    resolver
        .resolve(bearer(&tok).as_deref())
        .expect("the second resolve reuses the fresh cache entry");
    assert_eq!(
        fetches.load(Ordering::SeqCst),
        1,
        "a fresh cache entry is reused — the second token caused NO second fetch"
    );
}

#[test]
fn jwks_cache_miss_on_an_unknown_kid_triggers_exactly_one_refresh() {
    let fetches = Arc::new(AtomicI64::new(0));
    let source = RotatingSource {
        first: parse_jwks(&oct_jwks_json("old-key")),
        second: parse_jwks(&oct_jwks_json("new-key")),
        fetches: Arc::clone(&fetches),
    };
    let resolver = OidcResolver::new(
        hs256_config(),
        Box::new(source),
        Box::new(tenant_mapper()),
        fixed_clock(),
    );
    // First token signed under the OLD key: fetch #1, resolve succeeds.
    let pre = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        "old-key",
    );
    resolver
        .resolve(bearer(&pre).as_deref())
        .expect("the pre-rotation token resolves");
    // Second token under the NEW key: the fresh cache misses the kid →
    // exactly ONE refresh (the recorded anti-stampede posture) serves the
    // rotated document → resolve succeeds.
    let post = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        "new-key",
    );
    resolver
        .resolve(bearer(&post).as_deref())
        .expect("the post-rotation token triggers the bounded refresh and resolves");
    assert_eq!(
        fetches.load(Ordering::SeqCst),
        2,
        "the unknown-kid refresh is bounded to ONE attempt per miss"
    );
    // A third token under a key in NEITHER document: the refresh attempt
    // happens, fails, and denies fail-closed (the named error).
    let unknown = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:alice",
        &["tenant-admin"],
        0,
        "never-published-key",
    );
    assert_eq!(
        resolver
            .resolve(bearer(&unknown).as_deref())
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );
    assert_eq!(
        fetches.load(Ordering::SeqCst),
        3,
        "the failed refresh is also exactly one attempt (fail closed, no stampede)"
    );
}

#[test]
fn deny_all_resolver_remains_the_shipped_default_fail_closed() {
    // The composition pin: NOTHING in this wave changed the default. The
    // default resolver denies everything — a deployment opts into the OIDC
    // path by CONSTRUCTION ORDERING, not by code change.
    assert_eq!(
        DenyAllResolver
            .resolve(None)
            .expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
    assert_eq!(
        DenyAllResolver
            .resolve(Some("Bearer anything-at-all"))
            .expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
}

// --- FIM-F07.03: step-up enforcement ----------------------------------------

fn stepup_clock() -> Arc<AtomicI64> {
    Arc::new(AtomicI64::new(1_700_000_000_000))
}

fn stepup_verifier(clock: &Arc<AtomicI64>, required_amr: Vec<String>) -> OidcStepUpVerifier {
    let clock_for_verifier = Arc::clone(clock);
    OidcStepUpVerifier::new(
        hs256_config(),
        Box::new(StaticSource(parse_jwks(&oct_jwks_json(KID)))),
        DEFAULT_STEP_UP_EVIDENCE_MAX_AGE,
        required_amr,
        Box::new(move || clock_for_verifier.load(Ordering::SeqCst)),
    )
}

fn stepup_service(clock: &Arc<AtomicI64>, verifier: Box<dyn StepUpVerifier>) -> StepUpService {
    let clock_for_service = Arc::clone(clock);
    StepUpService::new(
        Box::new(InMemoryStepUpStore::default()),
        verifier,
        Box::new(|| id_from_uuid(FlexUuid::nil())),
        Box::new(move || clock_for_service.load(Ordering::SeqCst)),
        DEFAULT_STEP_UP_TTL,
    )
}

fn approval_purpose() -> StepUpPurpose {
    StepUpPurpose::CommandApproval {
        command_id: COMMAND_ID.to_string(),
    }
}

#[test]
fn step_up_accepts_fresh_evidence_through_a_fake_verifier_via_the_gate_helper() {
    // The acceptance half of the pin through a FAKE verifier (the real
    // IdP-backed verifier is the lab gate): the gate helper runs the
    // UNCHANGED service API and yields the single-use evidence bound to
    // the (subject, tenant, purpose) triple.
    let clock = stepup_clock();
    struct AcceptingFakeVerifier;
    impl StepUpVerifier for AcceptingFakeVerifier {
        fn verify(
            &self,
            challenge: &flexcore_control_plane::stepup::StepUpChallenge,
            presented: &str,
        ) -> Result<(), StepUpDenial> {
            if presented == "fresh-idp-evidence" && challenge.subject == "approver-1" {
                Ok(())
            } else {
                Err(StepUpDenial::EvidenceRejected {
                    reason: "test: the presented evidence does not match".to_string(),
                })
            }
        }
    }
    let service = stepup_service(&clock, Box::new(AcceptingFakeVerifier));
    let challenge = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the challenge issues");
    let evidence = require_command_approval_evidence(
        &service,
        &challenge.challenge_id,
        "approver-1",
        tenant(),
        "fresh-idp-evidence",
        COMMAND_ID,
    )
    .expect("the fake-verifier acceptance yields evidence");
    assert_eq!(evidence.subject, "approver-1");
    assert_eq!(evidence.challenge_id, challenge.challenge_id);
    assert_eq!(evidence.purpose, approval_purpose());
    // The evidence never outlives its challenge window.
    assert_eq!(evidence.expires_at_unix_millis, challenge.expires_at_unix_millis);
    // Wrong evidence through the SAME fake verifier: named refusal.
    assert!(matches!(
        require_command_approval_evidence(
            &service,
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            "stale-or-wrong",
            COMMAND_ID,
        ),
        Err(StepUpDenial::AlreadyUsed)
    ), "the accepted challenge was consumed; a replay is AlreadyUsed (single-use)");
}

#[test]
fn step_up_accepts_fresh_idp_evidence_through_the_service_api() {
    let clock = stepup_clock();
    let verifier = stepup_verifier(&clock, vec![]);
    let service = stepup_service(&clock, Box::new(verifier));
    let challenge = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the challenge issues");
    // The principal reauthenticated at the IdP 10 seconds ago: FRESH
    // evidence (the P13-E-3 DEFAULT: the IdP owns the authenticator; the
    // verifier consumes the authentication result).
    let evidence_tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "approver-1",
        &[],
        -10,
        KID,
    );
    let evidence = require_command_approval_evidence(
        &service,
        &challenge.challenge_id,
        "approver-1",
        tenant(),
        &evidence_tok,
        COMMAND_ID,
    )
    .expect("fresh IdP evidence verifies");
    assert_eq!(evidence.subject, "approver-1");
    assert_eq!(evidence.tenant, tenant());
    assert_eq!(evidence.purpose, approval_purpose());
}

#[test]
fn step_up_refuses_stale_evidence_with_a_named_denial() {
    let clock = stepup_clock();
    let verifier = stepup_verifier(&clock, vec![]);
    let service = stepup_service(&clock, Box::new(verifier));
    let challenge = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the challenge issues");
    // The authentication happened 400 seconds ago — beyond the 300 s
    // registered max age: the approval refuses server-side.
    let stale_tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "approver-1",
        &[],
        -400,
        KID,
    );
    let verdict = require_command_approval_evidence(
        &service,
        &challenge.challenge_id,
        "approver-1",
        tenant(),
        &stale_tok,
        COMMAND_ID,
    );
    match verdict {
        Err(StepUpDenial::EvidenceRejected { reason }) => {
            assert!(
                reason.contains("stale"),
                "the recorded reason names the staleness: {reason}"
            );
        }
        other => panic!(
            "the stale evidence must be a named EvidenceRejected denial, got {other:?}"
        ),
    }
    // The consumed challenge denies a retry as AlreadyUsed (single-use).
    assert_eq!(
        service.verify(
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            "anything",
            &approval_purpose(),
        ),
        Err(StepUpDenial::AlreadyUsed)
    );
}

#[test]
fn step_up_refuses_evidence_of_another_subject_and_without_the_required_factor() {
    let clock = stepup_clock();
    // The deployment requires an MFA-bearing method in `amr`.
    let verifier = stepup_verifier(&clock, vec!["mfa".to_string()]);
    let service = stepup_service(&clock, Box::new(verifier));
    let challenge = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the challenge issues");
    // Another principal's token: a SUBJECT binding failure (named), and
    // the challenge is consumed (a fresh challenge is the retry path).
    let mallory_tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "user:mallory",
        &[],
        -10,
        KID,
    );
    assert_eq!(
        service.verify(
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            &mallory_tok,
            &approval_purpose(),
        ),
        Err(StepUpDenial::SubjectMismatch)
    );
    // Fresh evidence from the right subject but WITHOUT the required
    // factor: named refusal (the evidence token carries amr ["pwd"]).
    let challenge2 = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the second challenge issues");
    let pwd_token = {
        let now = now_secs();
        sign_hs256(
            serde_json::json!({
                "iss": ISSUER,
                "aud": AUDIENCE,
                "sub": "approver-1",
                "exp": now + 600,
                "iat": now - 10,
                "auth_time": now - 10,
                "amr": ["pwd"],
            }),
            KID,
        )
    };
    match service.verify(
        &challenge2.challenge_id,
        "approver-1",
        tenant(),
        &pwd_token,
        &approval_purpose(),
    ) {
        Err(StepUpDenial::EvidenceRejected { reason }) => {
            assert!(
                reason.contains("required"),
                "the recorded reason names the factor requirement: {reason}"
            );
        }
        other => panic!("the missing-factor evidence must be a named denial, got {other:?}"),
    }
}

#[test]
fn the_default_step_up_verifier_refuses_every_high_impact_approval() {
    // The APPROVED-gate pin (FIM-F07.03): with the SHIPPED DEFAULT
    // verifier composed, EVERY high-impact approval refuses THROUGH THE
    // EXISTING service API — wiring the gate without an IdP-backed
    // verifier fails closed.
    let clock = stepup_clock();
    let service = stepup_service(&clock, Box::new(DenyAllStepUpVerifier));
    let challenge = service
        .issue("approver-1", tenant(), approval_purpose())
        .expect("the challenge issues");
    let evidence_tok = token(
        ISSUER,
        serde_json::json!(AUDIENCE),
        "approver-1",
        &[],
        -10,
        KID,
    );
    assert_eq!(
        require_command_approval_evidence(
            &service,
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            &evidence_tok,
            COMMAND_ID,
        ),
        Err(StepUpDenial::VerifierNotComposed)
    );
}

// --- The gated live-IdP test (the ONLY networked test in this file) ---------

/// Live JWKS fetch: gated on `FLEXCORE_TEST_OIDC_ISSUER` +
/// `FLEXCORE_TEST_OIDC_JWKS_URL` (the recorded gate variables). SKIPS
/// CLEANLY when both are unset or empty; a MIS-SET gate (one variable set,
/// the other empty) FAILS loudly instead of silently skipping (the F01
/// gated-suite convention). This drives the real `HttpJwksSource` HTTPS
/// fetch path — the exact surface the human lab gate (P13.20) exercises
/// with a real IdP.
#[test]
fn live_idp_jwks_document_fetches_and_parses() {
    let issuer = std::env::var("FLEXCORE_TEST_OIDC_ISSUER").unwrap_or_default();
    let jwks_url = std::env::var("FLEXCORE_TEST_OIDC_JWKS_URL").unwrap_or_default();
    let issuer_set = !issuer.trim().is_empty();
    let url_set = !jwks_url.trim().is_empty();
    if issuer_set && !url_set {
        panic!(
            "FLEXCORE_TEST_OIDC_ISSUER is set but FLEXCORE_TEST_OIDC_JWKS_URL is empty — \
             set both or neither"
        );
    }
    if !issuer_set && url_set {
        panic!(
            "FLEXCORE_TEST_OIDC_JWKS_URL is set but FLEXCORE_TEST_OIDC_ISSUER is empty — \
             set both or neither"
        );
    }
    if !issuer_set {
        eprintln!(
            "skipping: FLEXCORE_TEST_OIDC_ISSUER and FLEXCORE_TEST_OIDC_JWKS_URL are not set \
             (point them at the lab IdP — e.g. the P13.20 Keycloak realm — and re-run)"
        );
        return;
    }
    let source = flexcore_control_plane::oidc::HttpJwksSource::new(
        issuer.clone(),
        jwks_url,
        DEFAULT_JWKS_FETCH_TIMEOUT,
    )
    .expect("the live source builds without contacting the IdP");
    let document = source.fetch(&issuer).expect("the live JWKS fetch succeeds");
    assert!(
        !document.keys.is_empty(),
        "the live IdP publishes at least one signing key"
    );
}
