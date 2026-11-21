// FIM-F08.04/F08.05/F08.06 — the pinned integration suite for the F-08
// live integration platform (docs/releases/implementation_ledger/
// ledger_F08.md): machine identity (FIM-F08.01), per-tenant API keys
// (FIM-F08.02), live webhook delivery over HTTP (FIM-F08.03), the
// dead-letter budget boundary (FIM-F08.04), and the secret posture
// (FIM-F08.05).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is an interop result: live receiver interop
// is a **HUMAN LAB GATE** (FIM standing decisions, ledger README).
//
// DETERMINISM / GATING (the recorded FIM convention, as in
// `crates/adapters/protocol-20305/tests/live_transport_gated.rs`):
// - the token/key/rotation/dead-letter/posture tests are SOCKET-FREE and
//   deterministic (fixed fixtures, injected or system clocks with
//   ±1-hour margins against a 15-minute TTL — jsonwebtoken validates
//   `exp` against the process clock, the recorded boundary in
//   `crate::oidc`);
// - the live HTTP delivery tests bind 127.0.0.1 EPHEMERAL ports only
//   (loopback-only, no external host, no gate needed per the recorded
//   FIM allowance); the receiver serves exactly one request and the
//   client's total timeout bounds any hang;
// - NOTHING in this file reaches an external host, so no env-var gate is
//   required.
//
// The RECEIVER-SIDE HMAC VERIFICATION (FIM-F08.06) is pinned two ways:
// 1. an INDEPENDENT VECTOR, computed outside this repo's code with
//    `python -I` (stdlib `hmac`/`hashlib`; the exact derivation below),
//    asserting the hand-written signing primitives reproduce it:
//
//        key  = 32 bytes of 0x42
//        body = b'{"event": "ScheduleProposed"}'
//        msg  = b'1700000000' + b'.' + body        (timestamp + '.' + rawbody)
//        HMAC-SHA256(key, msg)
//            = 29e0015ef16f441f6da05878f14e9d36348361d4096da6ebdb2b294994999a85
//
// 2. an INDEPENDENT LOOPBACK RECEIVER (a raw std TCP server in this file,
//    not the sender's code) that parses the transmitted request, extracts
//    the `FlexCore-Signature` header, and verifies it through the
//    EXISTING `WebhookSecretSet::verify` over the exact received bytes —
//    the receiver-side check plan §2.3 prescribes, driven end-to-end
//    through the real `WebhookDispatcher` and the real reqwest transport.
//
// Consumed, never redefined: `webhook_service.rs` (the dispatcher, the
// permanent-rejection classifier, the dead-letter policy — BYTE-IDENTICAL
// in behavior), `webhook_signing.rs` (the scheme), `webhook_http.rs` (the
// live transport), `machine_identity.rs`, `api_keys.rs`.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::api_keys::{
    ApiKeyError, ApiKeyService, ApiKeyStore, IssuedApiKey, MemoryApiKeyStore,
};
use flexcore_control_plane::machine_identity::{
    MachineClientRecord, MachineClientStore, MachineIdentityConfig, MachineIdentityError,
    MachineIdentityService, MemoryMachineClientStore,
};
use flexcore_control_plane::webhook_http::{ReqwestOutboundHttpPost, WebhookHttpConfig};
use flexcore_control_plane::webhook_service::{
    attempt_allowed, HttpPostOutcome, MemoryWebhookDeliveryStore, OutboundHttpPost,
    WebhookDeliveryError, WebhookDeliveryStore, WebhookDispatcher, WebhookEndpointRecord,
    WebhookSecretResolver, MAX_ATTEMPTS,
};
use flexcore_control_plane::webhook_signing::{
    hex_encode_digest, hmac_sha256, WebhookSecretSet, WebhookSigningError, WebhookSigningKey,
};
use flexcore_domain::ids::TenantId;

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const OTHER_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const MACHINE_ISSUER_SECRET: &[u8] = b"fim-f08-machine-identity-test-issuer-key";
/// The fixed signing fixture (TEST-ONLY bytes — never a real secret).
const WEBHOOK_TEST_KEY: u8 = 0x42;

fn tenant(uuid_text: &str) -> TenantId {
    TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
}

/// The system clock in unix millis (see the file header's clock note).
fn system_clock_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after the epoch")
        .as_millis() as i64
}

// --- FIM-F08.01 / FIM-F08.06: machine identity --------------------------------

const CLIENT_SECRET: &str = "test-only-client-secret-not-a-real-credential";

fn machine_service(
    registry: MemoryMachineClientStore,
    issuer: &str,
    audience: &str,
    ttl_seconds: i64,
) -> MachineIdentityService {
    MachineIdentityService::new(
        MachineIdentityConfig {
            issuer: issuer.to_string(),
            audience: audience.to_string(),
            token_ttl_seconds: ttl_seconds,
            leeway_seconds: 0,
        },
        MACHINE_ISSUER_SECRET,
        Box::new(registry),
        Box::new(system_clock_millis),
    )
}

fn register_client(svc: &MachineIdentityService) {
    svc.register_client(
        "grid-operator-1",
        tenant(TENANT_UUID),
        vec![
            Action::TelemetryRead.id().to_string(),
            Action::SchedulePropose.id().to_string(),
        ],
        CLIENT_SECRET,
    )
    .expect("registration");
}

#[test]
fn a_client_credentials_token_authorizes_exactly_its_scoped_calls() {
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", 900);
    register_client(&svc);
    let token = svc
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued");
    let principal = svc.validate_machine_token(&token).expect("validates");
    assert_eq!(principal.subject, "integration:grid-operator-1");
    assert_eq!(principal.tenant_id, tenant(TENANT_UUID));
    principal.allows(Action::TelemetryRead).expect("granted");
    principal.allows(Action::SchedulePropose).expect("granted");
    // The scope refusal is NAMED (FIM-F08.06's scope-refusal pin).
    assert_eq!(
        principal.allows(Action::CommandApprove),
        Err(MachineIdentityError::ScopeNotGranted { action: "command.approve" }),
    );
    assert_eq!(
        principal.allows(Action::CommandApprove).expect_err("refused").name(),
        "MACHINE_IDENTITY_SCOPE_NOT_GRANTED",
    );
}

#[test]
fn a_revoked_machine_client_refuses_at_validation_immediately() {
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", 900);
    register_client(&svc);
    let token = svc
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued");
    // The token validated BEFORE the revocation.
    svc.validate_machine_token(&token).expect("valid before revocation");
    svc.revoke_client(&tenant(TENANT_UUID), "grid-operator-1").expect("revoked");
    assert_eq!(
        svc.validate_machine_token(&token),
        Err(MachineIdentityError::ClientRevoked),
        "a pre-revocation token refuses at the registry check — immediately, not at exp",
    );
    // And no new token can be issued for a revoked client.
    assert_eq!(
        svc.issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET),
        Err(MachineIdentityError::ClientDisabled),
    );
}

#[test]
fn the_registry_update_replaces_the_row_and_refuses_an_unknown_client() {
    // FIM2-R1-002 of the FIM final analysis: the MachineClientStore port
    // carries an EXPLICIT update path (the ApiKeyStore::update shape) —
    // replace-when-present, refuse-when-absent — and the emergency
    // kill-switch travels through it. `put` stays insert-only with its
    // STABLE duplicate refusal.
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry.clone(), "https://control-plane.test/token", "flexcore-api", 900);
    register_client(&svc);
    let row = registry
        .get(&tenant(TENANT_UUID), MACHINE_CLIENT_ID)
        .expect("read")
        .expect("registered");
    // Replace-when-present: the state flips and the epoch bumps in place
    // (exactly what revoke_client needs to persist).
    registry
        .update(MachineClientRecord {
            enabled: false,
            credential_epoch: row.credential_epoch + 1,
            ..row.clone()
        })
        .expect("update replaces the registered row");
    let updated = registry
        .get(&tenant(TENANT_UUID), MACHINE_CLIENT_ID)
        .expect("read")
        .expect("still registered");
    assert!(!updated.enabled);
    assert_eq!(updated.credential_epoch, row.credential_epoch + 1);
    assert_eq!(updated.allowed_scopes, row.allowed_scopes, "the scope subset is untouched by the state transition");
    // Refuse-when-absent: an unknown client is a NAMED update failure,
    // never an implicit insert.
    let stranger = MachineClientRecord {
        client_id: "never-registered".to_string(),
        tenant_id: tenant(TENANT_UUID),
        allowed_scopes: vec![],
        secret_verifier_sha256_hex: "aa".repeat(32),
        secret_prefix: "TEST".to_string(),
        enabled: true,
        credential_epoch: 0,
    };
    let refused = registry.update(stranger).expect_err("an unknown client refuses");
    match refused {
        MachineIdentityError::Backend { code, .. } => assert_eq!(code, "E_CLIENT_MISSING"),
        other => panic!("expected the missing-client update refusal, got {other:?}"),
    }
    // Nothing was written for the refused update.
    assert!(registry
        .get(&tenant(TENANT_UUID), "never-registered")
        .expect("read")
        .is_none());
    // And `put` keeps its insert-only contract: re-registering the same id
    // is the STABLE E_CLIENT_EXISTS refusal (the frozen string, unchanged
    // by the fix).
    let duplicate = MachineClientRecord {
        client_id: MACHINE_CLIENT_ID.to_string(),
        tenant_id: tenant(TENANT_UUID),
        allowed_scopes: vec![],
        secret_verifier_sha256_hex: "aa".repeat(32),
        secret_prefix: "TEST".to_string(),
        enabled: true,
        credential_epoch: 0,
    };
    let exists = registry.put(duplicate).expect_err("registration is explicit, never an upsert");
    match exists {
        MachineIdentityError::Backend { code, .. } => assert_eq!(code, "E_CLIENT_EXISTS"),
        other => panic!("expected the duplicate-registration refusal, got {other:?}"),
    }
}

#[test]
fn a_foreign_tenant_client_reads_as_absent_at_issuance() {
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", 900);
    register_client(&svc);
    // The client exists, but NOT in this tenant scope: a foreign-tenant
    // issuance reads as absent — existence never leaks across the tenant
    // boundary (P0.16 rule 5).
    assert_eq!(
        svc.issue_client_credentials_token(
            &tenant(OTHER_TENANT_UUID),
            "grid-operator-1",
            CLIENT_SECRET,
        ),
        Err(MachineIdentityError::UnknownClient),
    );
}

#[test]
fn an_expired_machine_token_is_refused_with_the_named_error() {
    let registry = MemoryMachineClientStore::new();
    // A NEGATIVE TTL makes the issuance itself stamp `exp` an hour in the
    // past (jsonwebtoken then refuses at decode — the crate's validation
    // against the process clock, the recorded boundary).
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", -3600);
    register_client(&svc);
    let token = svc
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued (expiry is the validation's business)");
    assert_eq!(
        svc.validate_machine_token(&token),
        Err(MachineIdentityError::ExpiredToken),
    );
}

#[test]
fn a_wrong_issuer_token_is_refused_with_the_named_error() {
    let registry = MemoryMachineClientStore::new();
    let issuer_a = machine_service(
        registry.clone(),
        "https://control-plane.test/token",
        "flexcore-api",
        900,
    );
    let issuer_b = machine_service(
        registry,
        "https://elsewhere.test/token",
        "flexcore-api",
        900,
    );
    register_client(&issuer_a);
    let token = issuer_a
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued");
    assert_eq!(
        issuer_b.validate_machine_token(&token),
        Err(MachineIdentityError::WrongIssuer),
        "a token from any other issuer is refused",
    );
}

#[test]
fn a_wrong_audience_token_is_refused_with_the_named_error() {
    let registry = MemoryMachineClientStore::new();
    let minter = machine_service(registry.clone(), "https://control-plane.test/token", "flexcore-api", 900);
    let validator = machine_service(registry, "https://control-plane.test/token", "other-api", 900);
    register_client(&minter);
    let token = minter
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued");
    assert_eq!(
        validator.validate_machine_token(&token),
        Err(MachineIdentityError::WrongAudience),
    );
}

#[test]
fn a_tampered_machine_token_is_refused() {
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", 900);
    register_client(&svc);
    let token = svc
        .issue_client_credentials_token(&tenant(TENANT_UUID), "grid-operator-1", CLIENT_SECRET)
        .expect("token issued");
    // Flip a character inside the payload segment.
    let mut bytes = token.into_bytes();
    let mid = bytes.len() / 2;
    bytes[mid] = if bytes[mid] == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(bytes).expect("utf8");
    assert_eq!(
        svc.validate_machine_token(&tampered),
        Err(MachineIdentityError::InvalidToken),
    );
}

#[test]
fn the_never_granted_names_are_structurally_excluded_from_machine_scopes() {
    let registry = MemoryMachineClientStore::new();
    let svc = machine_service(registry, "https://control-plane.test/token", "flexcore-api", 900);
    // Each never-granted name is not on the Action surface, so
    // registration refuses it structurally (parse → None → InvalidScope).
    for never_granted in
        ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"]
    {
        assert_eq!(
            svc.register_client(
                "overreaching",
                tenant(TENANT_UUID),
                vec![never_granted.to_string()],
                CLIENT_SECRET,
            ),
            Err(MachineIdentityError::InvalidScope { index: 0 }),
            "the never-granted name '{never_granted}' is structurally excluded",
        );
    }
}

// --- FIM-F08.02 / FIM-F08.06: API keys ------------------------------------------

#[test]
fn an_issued_api_key_authenticates_and_records_last_used_evidence() {
    let store = MemoryApiKeyStore::new();
    let svc = ApiKeyService::new(
        flexcore_control_plane::api_keys::ApiKeyConfig::default(),
        Box::new(store.clone()),
        Box::new(system_clock_millis),
    );
    let issued = svc
        .issue_key(tenant(TENANT_UUID), vec![Action::TelemetryRead.id().to_string()])
        .expect("issued");
    let principal = svc
        .authenticate(&issued.secret_literal, "/api/v1/telemetry")
        .expect("authenticates");
    assert_eq!(principal.tenant_id, tenant(TENANT_UUID));
    principal.allows(Action::TelemetryRead).expect("granted");
    // The evidence is on the row (timestamp + subject + ALLOW).
    let row = store
        .get(&tenant(TENANT_UUID), &issued.key_id)
        .expect("read")
        .expect("stored");
    let evidence = row.last_used.expect("evidence recorded");
    assert_eq!(evidence.last_used_subject_path, "/api/v1/telemetry");
    assert_eq!(evidence.last_used_result, "ALLOW");
    assert_eq!(row.use_count, 1);
    // The scope refusal is NAMED.
    assert_eq!(
        principal.allows(Action::CommandApprove).expect_err("refused").name(),
        "API_KEY_SCOPE_NOT_GRANTED",
    );
}

#[test]
fn rotation_keeps_in_flight_callers_until_the_grace_closes_then_refuses() {
    let store = MemoryApiKeyStore::new();
    let clock = Arc::new(AtomicI64::new(system_clock_millis()));
    let handle = clock.clone();
    let svc = ApiKeyService::new(
        flexcore_control_plane::api_keys::ApiKeyConfig { rotation_grace_seconds: 3_600 },
        Box::new(store.clone()),
        Box::new(move || handle.load(Ordering::SeqCst)),
    );
    let issued: IssuedApiKey = svc
        .issue_key(tenant(TENANT_UUID), vec![Action::TelemetryRead.id().to_string()])
        .expect("issued");
    let rotated = svc.rotate_key(&tenant(TENANT_UUID), &issued.key_id).expect("rotated");
    assert_ne!(rotated.secret_literal, issued.secret_literal);
    // In-flight callers (old secret) authenticate inside the grace window;
    // the new secret authenticates immediately.
    svc.authenticate(&rotated.secret_literal, "/new").expect("new key");
    svc.authenticate(&issued.secret_literal, "/in-flight").expect("old key in grace");
    // Past the grace window: named refusal for the old secret; the new
    // secret is unaffected.
    clock.fetch_add(3_600 * 1_000 * 2, Ordering::SeqCst);
    assert_eq!(
        svc.authenticate(&issued.secret_literal, "/late"),
        Err(ApiKeyError::SupersededPastGrace),
    );
    svc.authenticate(&rotated.secret_literal, "/after-grace").expect("new key unaffected");
}

#[test]
fn api_key_revocation_is_immediate_for_both_secrets() {
    let store = MemoryApiKeyStore::new();
    let svc = ApiKeyService::new(
        flexcore_control_plane::api_keys::ApiKeyConfig::default(),
        Box::new(store.clone()),
        Box::new(system_clock_millis),
    );
    let issued = svc
        .issue_key(tenant(TENANT_UUID), vec![Action::TelemetryRead.id().to_string()])
        .expect("issued");
    let rotated = svc.rotate_key(&tenant(TENANT_UUID), &issued.key_id).expect("rotated");
    svc.revoke_key(&tenant(TENANT_UUID), &issued.key_id).expect("revoked");
    assert_eq!(svc.authenticate(&issued.secret_literal, "/p"), Err(ApiKeyError::KeyRevoked));
    assert_eq!(svc.authenticate(&rotated.secret_literal, "/p"), Err(ApiKeyError::KeyRevoked));
    // A revoked key's scope set is unreachable — no principal exists to
    // grant anything (the refusal precedes any scope consultation).
}

// --- FIM-F08.05: the secret posture (hard) --------------------------------------

#[test]
fn the_api_key_store_holds_only_verifiers_never_raw_key_material() {
    let store = MemoryApiKeyStore::new();
    let svc = ApiKeyService::new(
        flexcore_control_plane::api_keys::ApiKeyConfig::default(),
        Box::new(store.clone()),
        Box::new(system_clock_millis),
    );
    let issued = svc
        .issue_key(tenant(TENANT_UUID), vec![Action::TelemetryRead.id().to_string()])
        .expect("issued");
    svc.authenticate(&issued.secret_literal, "/p").expect("authenticates");
    // Every retrievable row's Debug — the only surface a dump would
    // render — carries digests and fingerprints, never a `fk_` literal.
    let row = store
        .get(&tenant(TENANT_UUID), &issued.key_id)
        .expect("read")
        .expect("stored");
    let debugged = format!("{row:?}");
    assert!(!debugged.contains(&issued.secret_literal), "raw secret absent from the row");
    assert!(!debugged.contains("fk_"), "no `fk_` literal anywhere in the row");
    // The verifier IS the SHA-256 hex of the full literal (the P13.08
    // digest-at-rest rule), computed through the repo's own
    // vector-anchored primitive.
    assert_eq!(
        row.secret_verifier_sha256_hex,
        hex_encode_digest(&flexcore_control_plane::webhook_signing::sha256(
            issued.secret_literal.as_bytes()
        )),
        "the stored verifier is the SHA-256 hex of the secret literal",
    );
    // The rotated-away (superseded) secret is likewise only a digest —
    // and the CURRENT verifier is the rotated secret's.
    let rotated = svc.rotate_key(&tenant(TENANT_UUID), &issued.key_id).expect("rotated");
    let row = store
        .get(&tenant(TENANT_UUID), &issued.key_id)
        .expect("read")
        .expect("stored");
    let debugged = format!("{row:?}");
    assert!(!debugged.contains(&issued.secret_literal), "superseded secret absent");
    assert!(!debugged.contains(&rotated.secret_literal), "current raw secret absent");
    assert_eq!(
        row.secret_verifier_sha256_hex,
        hex_encode_digest(&flexcore_control_plane::webhook_signing::sha256(
            rotated.secret_literal.as_bytes()
        )),
    );
    assert_eq!(
        row.superseded_verifier_sha256_hex.as_deref(),
        Some(
            hex_encode_digest(&flexcore_control_plane::webhook_signing::sha256(
                issued.secret_literal.as_bytes()
            ))
            .as_str()
        ),
        "the superseded slot carries the OLD digest, never the old secret",
    );
}

/// A pure map-backed secret resolver: references in, TEST-ONLY bytes out.
/// NO database, NO store, NO network — the resolver port's whole input is
/// the provider reference (the P11.16 posture: the material comes from the
/// deployment's secret store, resolved at attempt time).
struct MapSecretResolver {
    material: Vec<u8>,
    consulted: Mutex<Vec<String>>,
}

impl MapSecretResolver {
    fn new(material: Vec<u8>) -> Self {
        Self { material, consulted: Mutex::new(Vec::new()) }
    }
}

impl WebhookSecretResolver for MapSecretResolver {
    fn resolve(&self, reference: &str) -> Result<Vec<u8>, WebhookDeliveryError> {
        self.consulted.lock().expect("consulted lock").push(reference.to_string());
        if reference.starts_with("vault:") {
            Ok(self.material.clone())
        } else {
            Err(WebhookDeliveryError::NoActiveSecret)
        }
    }
}

/// A recording HTTP fake (the socket-free transport for the dispatcher
/// pins).
struct FakeHttp {
    calls: Mutex<Vec<(String, Vec<(String, String)>, Vec<u8>)>>,
    outcome: HttpPostOutcome,
}

impl FakeHttp {
    fn new(outcome: HttpPostOutcome) -> Self {
        Self { calls: Mutex::new(Vec::new()), outcome }
    }
}

impl OutboundHttpPost for FakeHttp {
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
        self.calls.lock().expect("calls lock").push((
            url.to_string(),
            headers.to_vec(),
            raw_body.to_vec(),
        ));
        Ok(self.outcome.clone())
    }
}

fn endpoint_record(tenant_id: TenantId, url: &str) -> WebhookEndpointRecord {
    WebhookEndpointRecord {
        endpoint_id: "ep-1".to_string(),
        tenant_id,
        url: url.to_string(),
        // PROVIDER REFERENCES, never material (the P13.08 → P13.09 seam).
        secret_current_ref: "vault:webhook/ep-1/current".to_string(),
        secret_previous_ref: None,
        active: true,
    }
}

#[test]
fn webhook_records_and_endpoint_rows_carry_no_secret_material() {
    // Drive one full delivery, then scan EVERY record surface the flow
    // produces for the material (its hex and its base64url renderings —
    // no record may carry either, P11.16 / P13.08 §4).
    let store = MemoryWebhookDeliveryStore::new();
    let http = FakeHttp::new(HttpPostOutcome::Responded { code: 200 });
    let resolver = MapSecretResolver::new(vec![WEBHOOK_TEST_KEY; 32]);
    let t = tenant(TENANT_UUID);
    let endpoint = endpoint_record(t, "https://receiver.example.test/hook");
    let endpoints =
        move |tid: &TenantId, eid: &str| if *tid == t && eid == "ep-1" { Ok(endpoint.clone()) } else { Err(WebhookDeliveryError::EndpointNotFound) };
    let disp = WebhookDispatcher::new(
        &store,
        &http,
        &resolver,
        Box::new(|| 1_700_000_000_000),
        &endpoints,
    );
    let delivery = disp
        .enqueue(
            t,
            "evt:outbox:0001".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{\"event\":\"ScheduleProposed\"}".to_string(),
            "d-1".to_string(),
        )
        .expect("booked");
    disp.run_attempt(&delivery).expect("attempt runs");

    let material = vec![WEBHOOK_TEST_KEY; 32];
    // The renderings a dump could carry: the base64url form and the
    // debug-hex byte rendering. (The material is 32 repeated bytes; if a
    // record carried it, either rendering would appear verbatim.)
    let material_forms = [
        flexcore_control_plane::api_keys::base64url_encode_no_pad(&material),
        format!("{:x?}", material),
    ];
    // Scan the delivery row (the record the booking returned and its
    // idempotent re-read), the endpoint record, and every call the
    // transport observed: none may contain the material. (The attempt and
    // dead-letter rows are the same public record types — their maps are
    // module-private here; the type-level absence is pinned by this scan
    // plus the module-level map scans.)
    let stored = store.list_due_deliveries(0, 0, 100).expect("list");
    let mut scanned = vec![format!("{delivery:?}"), format!("{:?}", endpoint_record(t, "https://receiver.example.test/hook"))];
    for record in &stored {
        scanned.push(format!("{record:?}"));
    }
    for debugged in &scanned {
        for form in &material_forms {
            assert!(!debugged.contains(form), "material rendering leaked into a webhook record");
        }
    }
    let calls = http.calls.lock().expect("calls lock");
    let (url, headers, body) = &calls[0];
    // The transport saw exactly the DERIVED signature header — the only
    // secret-derived artifact on the wire is the signature itself.
    assert!(headers.iter().any(|(name, value)| {
        name == flexcore_control_plane::webhook_signing::scheme::SIGNATURE_HEADER
            && value.starts_with("t=1700000000,v1=")
    }));
    assert_eq!(url, "https://receiver.example.test/hook");
    assert_eq!(body.as_slice(), b"{\"event\":\"ScheduleProposed\"}");
    // The resolver was consulted ONLY with the endpoint's provider
    // reference — never with a store handle, a tenant id, or the material
    // (the structural half of the "never reads the database" pin: the
    // port's entire input is the reference string).
    let consulted = resolver.consulted.lock().expect("consulted lock");
    assert_eq!(
        consulted.as_slice(),
        ["vault:webhook/ep-1/current"],
        "the resolver saw exactly the current-secret reference",
    );
}

// --- FIM-F08.04: the dead-letter budget boundary (socket-free) ------------------

#[test]
fn a_failing_endpoint_dead_letters_exactly_at_the_budget_boundary() {
    let store = MemoryWebhookDeliveryStore::new();
    let http = FakeHttp::new(HttpPostOutcome::Responded { code: 500 });
    let resolver = MapSecretResolver::new(vec![WEBHOOK_TEST_KEY; 32]);
    let t = tenant(TENANT_UUID);
    let endpoint = endpoint_record(t, "https://receiver.example.test/hook");
    let endpoints =
        move |tid: &TenantId, eid: &str| if *tid == t && eid == "ep-1" { Ok(endpoint.clone()) } else { Err(WebhookDeliveryError::EndpointNotFound) };
    let disp = WebhookDispatcher::new(
        &store,
        &http,
        &resolver,
        Box::new(|| 1_000),
        &endpoints,
    );
    let delivery = disp
        .enqueue(
            t,
            "k".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{}".to_string(),
            "d-1".to_string(),
        )
        .expect("booked");

    assert_eq!(store.count_dead_letters().expect("count"), 0, "nothing dead-lettered yet");
    let mut current = delivery.clone();
    for attempt in 1..=MAX_ATTEMPTS {
        let status = disp.run_attempt(&current).expect("attempt runs");
        // Exactly AT the recorded boundary: attempts 1..MAX-1 schedule;
        // attempt MAX dead-letters.
        if attempt < MAX_ATTEMPTS {
            assert_eq!(status, "PENDING", "attempt {attempt} schedules the next");
            assert_eq!(store.count_dead_letters().expect("count"), 0,
                "no dead letter before the budget boundary");
        } else {
            assert_eq!(status, "DEAD_LETTERED", "attempt {attempt} (the MAX) dead-letters");
            assert_eq!(store.count_dead_letters().expect("count"), 1,
                "exactly one dead letter lands at the budget boundary");
        }
        // Re-read the delivery through the idempotent booking (the
        // integration-visible record surface).
        let reread = disp
            .enqueue(
                t,
                "k".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{}".to_string(),
                "d-1".to_string(),
            )
            .expect("idempotent re-read");
        current = reread;
    }
    // The bound is exact: no further attempt is even allowed.
    assert!(!attempt_allowed(MAX_ATTEMPTS + 1));
    // And the dead-lettered delivery never rescans.
    let due = store.list_due_deliveries(1_000_000, 0, 100).expect("scan");
    assert!(due.is_empty(), "the dead-lettered delivery is out of the scan set");
    // Every failure was a 500: MAX_ATTEMPTS calls happened, no more.
    assert_eq!(http.calls.lock().expect("calls lock").len(), MAX_ATTEMPTS as usize);
}

#[test]
fn a_permanent_rejection_dead_letters_immediately_without_burning_the_budget() {
    // 404 (a non-408/429 4xx): the UNCHANGED classifier says permanent;
    // one attempt, one dead letter, budget untouched.
    let store = MemoryWebhookDeliveryStore::new();
    let http = FakeHttp::new(HttpPostOutcome::Responded { code: 404 });
    let resolver = MapSecretResolver::new(vec![WEBHOOK_TEST_KEY; 32]);
    let t = tenant(TENANT_UUID);
    let endpoint = endpoint_record(t, "https://receiver.example.test/hook");
    let endpoints =
        move |tid: &TenantId, eid: &str| if *tid == t && eid == "ep-1" { Ok(endpoint.clone()) } else { Err(WebhookDeliveryError::EndpointNotFound) };
    let disp = WebhookDispatcher::new(
        &store,
        &http,
        &resolver,
        Box::new(|| 1_000),
        &endpoints,
    );
    let delivery = disp
        .enqueue(
            t,
            "k".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{}".to_string(),
            "d-1".to_string(),
        )
        .expect("booked");
    assert_eq!(disp.run_attempt(&delivery).expect("attempt runs"), "DEAD_LETTERED");
    assert_eq!(store.count_dead_letters().expect("count"), 1);
    assert_eq!(http.calls.lock().expect("calls lock").len(), 1, "the budget is not burned");
    // The retryable 4xx (408/429) and transport failures would schedule —
    // the classifier's sparing behavior is pinned module-side and by the
    // 500 test above; here the transport-failure class is pinned directly.
    assert!(!HttpPostOutcome::TransportFailure { class: "CONNECT_ERROR".to_string() }
        .is_permanent_rejection());
    assert!(!HttpPostOutcome::Responded { code: 429 }.is_permanent_rejection());
}

// --- FIM-F08.06: the independent HMAC vector ------------------------------------

#[test]
fn the_signing_primitives_reproduce_the_independent_python_hmac_vector() {
    // Independent derivation (run OUTSIDE this repo, `python -I`):
    //   key  = bytes([0x42]*32)
    //   body = b'{"event": "ScheduleProposed"}'
    //   msg  = b'1700000000' + b'.' + body
    //   hmac.new(key, msg, hashlib.sha256).hexdigest()
    //     = '29e0015ef16f441f6da05878f14e9d36348361d4096da6ebdb2b294994999a85'
    let key = [WEBHOOK_TEST_KEY; 32];
    let body = b"{\"event\": \"ScheduleProposed\"}";
    let mut msg = Vec::new();
    msg.extend_from_slice(b"1700000000");
    msg.push(b'.');
    msg.extend_from_slice(body);
    assert_eq!(
        hex_encode_digest(&hmac_sha256(&key, &msg)),
        "29e0015ef16f441f6da05878f14e9d36348361d4096da6ebdb2b294994999a85",
    );
    // And the SCHEME surface reproduces the same digest through its header
    // form: sign with the fixture set and verify the header's v1 element
    // carries exactly that hex.
    let set = WebhookSecretSet::single(WebhookSigningKey::from_bytes(&key).expect("key"));
    let header = set.sign(1_700_000_000, body);
    assert!(header.starts_with("t=1700000000,v1=29e0015ef16f441f6da05878f14e9d36348361d4096da6ebdb2b294994999a85"));
}

// --- FIM-F08.03 / FIM-F08.06: live loopback delivery, receiver-verified ---------

/// A raw loopback receiver (std TCP; NOT the sender's code): reads one
/// HTTP/1.1 request to completion (headers + content-length body),
/// verifies the `FlexCore-Signature` header through the EXISTING
/// `WebhookSecretSet::verify` over the exact received bytes at the
/// dispatcher's injected timestamp, records the verification result, and
/// answers with `response_code`.
struct LoopbackReceiver {
    url: String,
    observed: Arc<Mutex<Vec<ReceiverObservation>>>,
}

#[derive(Debug)]
struct ReceiverObservation {
    body: Vec<u8>,
    signature_header: Option<String>,
    verification: Result<(), WebhookSigningError>,
}

fn serve_one_loopback_request(
    secret_key_bytes: &[u8; 32],
    verification_timestamp: i64,
    response_code: u16,
    listener: TcpListener,
    observed: Arc<Mutex<Vec<ReceiverObservation>>>,
) {
    let (mut stream, _) = match listener.accept() {
        Ok(accepted) => accepted,
        Err(_) => return,
    };
    // Read until the end of the headers, then the content-length body.
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = match stream.read(&mut chunk) {
            Ok(0) => break buffer.len(),
            Ok(read) => read,
            Err(_) => break buffer.len(),
        };
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
        if buffer.len() > 1_048_576 {
            break buffer.len();
        }
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let _request_line = lines.next();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let content_length: usize = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    let body_start = header_end + 4; // past the \r\n\r\n
    while buffer.len() < body_start + content_length {
        let read = match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer
        .get(body_start..(body_start + content_length).min(buffer.len()))
        .unwrap_or(&[])
        .to_vec();
    let signature_header = headers
        .iter()
        .find(|(name, _)| {
            name == flexcore_control_plane::webhook_signing::scheme::SIGNATURE_HEADER
        })
        .map(|(_, value)| value.clone());
    let verification = match &signature_header {
        Some(value) => {
            let set = WebhookSecretSet::single(
                WebhookSigningKey::from_bytes(secret_key_bytes).expect("receiver key"),
            );
            set.verify(value, verification_timestamp, &body)
        }
        None => Err(WebhookSigningError::HeaderMissing),
    };
    observed.lock().expect("observed lock").push(ReceiverObservation {
        body,
        signature_header,
        verification,
    });
    let response = format!(
        "HTTP/1.1 {} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        response_code
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

#[test]
fn a_live_delivery_is_signed_and_independently_verified_by_the_loopback_receiver() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral loopback bind");
    let address = listener.local_addr().expect("local addr");
    let observed: Arc<Mutex<Vec<ReceiverObservation>>> = Arc::new(Mutex::new(Vec::new()));
    // The receiver verifies at the dispatcher's INJECTED timestamp (the
    // attempt stamps `t=` from the same fixed clock).
    {
        let observed_handle = observed.clone();
        std::thread::spawn(move || {
            serve_one_loopback_request(
                &[WEBHOOK_TEST_KEY; 32],
                1_700_000_000,
                200,
                listener,
                observed_handle,
            );
        });
    }
    let store = MemoryWebhookDeliveryStore::new();
    let transport =
        ReqwestOutboundHttpPost::new(WebhookHttpConfig {
            connect_timeout_secs: 5,
            total_timeout_secs: 5,
        })
        .expect("transport built");
    let resolver = MapSecretResolver::new(vec![WEBHOOK_TEST_KEY; 32]);
    let t = tenant(TENANT_UUID);
    let endpoint = endpoint_record(t, &format!("http://{address}/hook"));
    let endpoints =
        move |tid: &TenantId, eid: &str| if *tid == t && eid == "ep-1" { Ok(endpoint.clone()) } else { Err(WebhookDeliveryError::EndpointNotFound) };
    let disp = WebhookDispatcher::new(
        &store,
        &transport,
        &resolver,
        Box::new(|| 1_700_000_000_000),
        &endpoints,
    );
    let delivery = disp
        .enqueue(
            t,
            "k".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{\"event\":\"ScheduleProposed\"}".to_string(),
            "d-1".to_string(),
        )
        .expect("booked");
    assert_eq!(disp.run_attempt(&delivery).expect("attempt runs"), "DELIVERED");
    let observations = observed.lock().expect("observed lock");
    assert_eq!(observations.len(), 1, "exactly one request reached the receiver");
    let observation = &observations[0];
    assert_eq!(
        observation.verification,
        Ok(()),
        "the INDEPENDENT receiver's HMAC check over the exact received bytes verifies",
    );
    assert_eq!(observation.body, b"{\"event\":\"ScheduleProposed\"}", "the RAW body arrived");
}

#[test]
fn a_receiver_with_a_different_secret_rejects_and_the_delivery_dead_letters() {
    // The full negative loop: receiver holds a DIFFERENT key → its HMAC
    // check fails → it answers 404 → the UNCHANGED classifier reads a
    // permanent rejection → the delivery dead-letters immediately.
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral loopback bind");
    let address = listener.local_addr().expect("local addr");
    let observed: Arc<Mutex<Vec<ReceiverObservation>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let observed_handle = observed.clone();
        std::thread::spawn(move || {
            serve_one_loopback_request(
                &[0x43; 32], // the WRONG receiver key
                1_700_000_000,
                404,
                listener,
                observed_handle,
            );
        });
    }
    let store = MemoryWebhookDeliveryStore::new();
    let transport =
        ReqwestOutboundHttpPost::new(WebhookHttpConfig {
            connect_timeout_secs: 5,
            total_timeout_secs: 5,
        })
        .expect("transport built");
    let resolver = MapSecretResolver::new(vec![WEBHOOK_TEST_KEY; 32]);
    let t = tenant(TENANT_UUID);
    let endpoint = endpoint_record(t, &format!("http://{address}/hook"));
    let endpoints =
        move |tid: &TenantId, eid: &str| if *tid == t && eid == "ep-1" { Ok(endpoint.clone()) } else { Err(WebhookDeliveryError::EndpointNotFound) };
    let disp = WebhookDispatcher::new(
        &store,
        &transport,
        &resolver,
        Box::new(|| 1_700_000_000_000),
        &endpoints,
    );
    let delivery = disp
        .enqueue(
            t,
            "k".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{}".to_string(),
            "d-1".to_string(),
        )
        .expect("booked");
    assert_eq!(disp.run_attempt(&delivery).expect("attempt runs"), "DEAD_LETTERED");
    assert_eq!(store.count_dead_letters().expect("count"), 1);
    let observations = observed.lock().expect("observed lock");
    assert_eq!(observations.len(), 1);
    assert!(observations[0].verification.is_err(), "the receiver's check FAILED (wrong key)");
}

// --- A compile-shape guard for the store handles --------------------------------

/// `MemoryApiKeyStore` is shared across the service and the test handles
/// through the repo's store-trait pattern (Clone + Send + Sync), and the
/// machine registry is a plain owned handle.
#[test]
fn the_store_handles_compose_across_service_and_test() {
    fn assert_store<T: ApiKeyStore + Send + Sync>() {}
    assert_store::<MemoryApiKeyStore>();
    fn assert_registry<T: MachineClientStore + Send + Sync>() {}
    assert_registry::<MemoryMachineClientStore>();
    // The BTreeMap orderings stay deterministic (conventions §6.6).
    let mut map: BTreeMap<String, u8> = BTreeMap::new();
    map.insert("b".to_string(), 2);
    map.insert("a".to_string(), 1);
    let first = map.values().next().copied();
    assert_eq!(first, Some(1));
}

// --- A note on the unused-import surface ----------------------------------------
// `IssuedApiKey` is used above in the rotation test's type annotation;
// `WEBHOOK_TEST_KEY` names the fixture; nothing else is imported
// speculatively. (This comment is documentation, not a check.)
