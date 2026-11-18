// Patch B — source authentication: the envelope wire format, the
// canonical signing bytes, and the `PgSourceAuthenticator` in Enforced
// mode, against real PostgreSQL.
//
// What is asserted here:
// 1. `canonical_signing_bytes` is byte-exact against a layout built BY
//    HAND from the spec (domain + version byte + u32-LE length-prefixed
//    key_id/source_id + 16 raw tenant UUID bytes + u64-LE sequence +
//    i64-LE event_time + 32 raw digest bytes) for two distinct envelopes.
//    One of these is the FROZEN known-answer vector: it must never change
//    without a version bump (`ENVELOPE_VERSION`).
// 2. The canonical bytes are what gets signed: an Ed25519 sign/verify
//    round-trip over them (pure, no DB).
// 3. The exact `quarantine_reason` strings of every `SourceAuthError`
//    variant (pure, no DB).
// 4. Through the real authenticator (DB-gated): a correctly signed
//    envelope authenticates; one flipped payload byte is `BadDigest`;
//    one flipped signature byte is `BadSignature`; a frame signed with a
//    different key than the registered one is `BadSignature`; an envelope
//    for tenant A does not authenticate under tenant B; a key registered
//    for source A does not authenticate source B; malformed envelopes are
//    `MalformedEnvelope`; `key_id == "dev"` is rejected in Enforced mode.
//
// Gating: the pure canonical-bytes / error-string tests need no database
// and are exempt from the gate. Every authenticator test FAILS LOUDLY
// when `FLEXCORE_TEST_DATABASE_URL` is unset or empty — a DB-gated suite
// must never silently pass (the Patch A `require_db` pattern).
//
// Keys are generated ephemerally per test (a 32-byte seed from the OS
// CSPRNG via `OsRng.try_fill_bytes`, then `SigningKey::from_bytes` —
// ed25519-dalek 3.0.0's `generate` cannot take rand 0.9's `OsRng`);
// no key material is hardcoded anywhere in this file.
//
// Harness notes (copied from the Patch A reader files): roles are granted
// LOGIN with the same test-only passwords; key rows are INSERTed through
// the `flexcore_app` role under the tenant GUC (the recorded P2.06
// pattern — the runtime role is the one migration 0026 grants
// SELECT/INSERT on `source_signing_keys` to). Fixture prefix byte 0xE1:
// distinct from the Patch A reader files (0xC1/0xD1) so all suites can
// share one test database without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey, Verifier};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::entities::Tenant;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::TenantRepository;
use flexcore_ports::source_auth::{
    canonical_signing_bytes, SourceAuthError, SourceAuthMode, SourceAuthenticator,
    DEFAULT_AUTH_WINDOW_SECS, ENVELOPE_DOMAIN, ENVELOPE_VERSION,
};
use flexcore_postgres_adapter::source_keys::PgSourceAuthenticator;
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use rand::rand_core::TryRngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;

// --- Harness helpers (the Patch A pattern) -----------------------------------

/// The DB-gated suite FAILS LOUDLY when the variable is unset or empty.
fn require_db(test: &str) -> String {
    match std::env::var("FLEXCORE_TEST_DATABASE_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
    {
        Some(url) => url,
        None => panic!(
            "FLEXCORE_TEST_DATABASE_URL is not set or empty: the DB-gated \
             source-auth envelope suite must not silently pass (test {test}); \
             set it to a PostgreSQL 18 test database URL"
        ),
    }
}

fn run_key() -> &'static u64 {
    static RUN_KEY: OnceLock<u64> = OnceLock::new();
    RUN_KEY.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock is after the Unix epoch")
            .as_nanos() as u64
    })
}

fn next_counter() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0xE1; // Patch B source-auth fixture marker
    bytes[2] = (run_key() >> 56) as u8;
    bytes[3] = (run_key() >> 48) as u8;
    bytes[4] = (run_key() >> 40) as u8;
    bytes[5] = (run_key() >> 32) as u8;
    bytes[6] = (run_key() >> 24) as u8;
    bytes[7] = (run_key() >> 16) as u8;
    bytes[8] = (run_key() >> 8) as u8;
    bytes[9] = *run_key() as u8;
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn fixture_tenant_id(prefix: u8) -> TenantId {
    TenantId::new(fixture_uuid(prefix))
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "integration-test:source-auth-envelope".to_string(),
        },
    ))
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn url_as(url: &str, user: &str, password: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((_, host_and_path)) = rest.split_once('@') {
            return format!("{scheme}://{user}:{password}@{host_and_path}");
        }
        return format!("{scheme}://{user}:{password}@{rest}");
    }
    format!("{url}?user={user}&password={password}")
}

fn ensure_test_roles(pool: &PgPool) {
    let result = test_runtime().block_on(async {
        // Concurrent ALTER ROLE on the same role fails with "tuple
        // concurrently updated" (XX000): every test in every binary runs
        // this helper against the one shared database. One advisory lock
        // (one key, shared by every copy of this helper and by the
        // partition setup) serializes the grant so it stays idempotent
        // across processes; the lock is transaction-scoped and released
        // by the commit below.
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(20261118)")
            .execute(&mut *tx)
            .await?;
        for (role, password) in [
            ("flexcore_app", "flexcore_app_test_only"),
            ("flexcore_migrator", "flexcore_migrator_test_only"),
            ("flexcore_ingest", "flexcore_ingest_test_only"),
            ("flexcore_query", "flexcore_query_test_only"),
        ] {
            // AUDITED (Batch 32+): role/password are fixed test-only literals
            // from the loop above; DDL identifiers cannot be bind parameters.
            sqlx::query(sqlx::AssertSqlSafe(
                format!("ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"),
            ))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok::<(), sqlx::Error>(())
    });
    result.expect("test roles must be grantable from the migration/admin connection");
}

struct DbHarness {
    admin_store: FlexCoreStore,
    app_store: FlexCoreStore,
}

fn setup(test: &str) -> DbHarness {
    let url = require_db(test);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    DbHarness {
        admin_store,
        app_store,
    }
}

/// Seeds the tenant row (migration 0026's `source_signing_keys` and
/// `source_sequence_events` both carry a foreign key to `tenants`, so a
/// key may only be registered for a tenant that exists). Tenants carry
/// no RLS and go through the admin connection (the P2.06 pattern).
fn seed_tenant(admin_store: &FlexCoreStore, tenant_id: TenantId) {
    let tenant_entity = Tenant::try_new(
        tenant_id,
        format!("tenant-{tenant_id}"),
        None,
        format!("idp:{tenant_id}"),
        "profile:default".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid test tenant");
    let unit = admin_store.begin(None).expect("admin unit of work");
    TenantRepository::insert(&unit, &tenant_entity).expect("tenant insert");
    unit.commit().expect("commit tenant");
}

/// A fresh fixture tenant, seeded so keys may be registered for it.
fn seeded_tenant(h: &DbHarness, prefix: u8) -> TenantId {
    let tenant = fixture_tenant_id(prefix);
    seed_tenant(&h.admin_store, tenant);
    tenant
}

// --- Small crypto / encoding helpers (no key material hardcoded) -------------

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((*byte >> 4) as u32, 16).expect("hex digit"));
        out.push(char::from_digit((*byte & 0x0f) as u32, 16).expect("hex digit"));
    }
    out
}

fn hex_decode(text: &str) -> Vec<u8> {
    let chars: Vec<char> = text.chars().collect();
    assert_eq!(chars.len() % 2, 0, "hex input must have an even length");
    chars
        .chunks(2)
        .map(|pair| {
            let high = pair[0].to_digit(16).expect("hex digit") as u8;
            let low = pair[1].to_digit(16).expect("hex digit") as u8;
            (high << 4) | low
        })
        .collect()
}

/// Wall-clock "now" in Unix milliseconds.
fn now_millis() -> i64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}

fn ephemeral_key() -> (SigningKey, [u8; 32]) {
    // ed25519-dalek 3.0.0's rand_core 0.10 has no OsRng, so `generate` cannot
    // take rand 0.9's OsRng; seed from the same OS CSPRNG directly instead.
    let mut seed = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut seed)
        .expect("the OS CSPRNG supplies the ephemeral key seed");
    let signing = SigningKey::from_bytes(&seed);
    let public = signing.verifying_key().to_bytes();
    (signing, public)
}

/// Builds the wire-format envelope JSON for a correctly signed envelope:
/// SHA-256 digest over `payload`, Ed25519 signature over the canonical
/// signing bytes. This is the test's own construction of the wire format
/// (the spec in the brief), not a call into production signing code —
/// there is no production signing helper to call.
#[allow(clippy::too_many_arguments)]
fn sign_envelope(
    signing_key: &SigningKey,
    key_id: &str,
    source_id: &str,
    tenant_id: TenantId,
    sequence: u64,
    event_time_millis: i64,
    payload: &[u8],
) -> serde_json::Value {
    let digest: [u8; 32] = Sha256::digest(payload).into();
    let signing_bytes = canonical_signing_bytes(
        ENVELOPE_VERSION,
        key_id,
        source_id,
        &tenant_id,
        sequence,
        event_time_millis,
        &digest,
    );
    let signature = signing_key.sign(&signing_bytes);
    serde_json::json!({
        "version": ENVELOPE_VERSION,
        "key_id": key_id,
        "source_id": source_id,
        "tenant_id": tenant_id.uuid().to_hyphenated(),
        "sequence": sequence,
        "event_time_unix_millis": event_time_millis,
        "payload_digest": hex_encode(&digest),
        "payload": STANDARD.encode(payload),
        "signature": hex_encode(&signature.to_bytes()),
    })
}

fn envelope_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("envelope JSON serializes")
}

/// INSERTs one key row through the `flexcore_app` role under the tenant
/// GUC (the role migration 0026 grants SELECT/INSERT to). `term_seq` 1
/// unless the test says otherwise.
#[allow(clippy::too_many_arguments)]
fn register_key(
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    source_id: &str,
    key_id: &str,
    public_key: &[u8; 32],
    status: &str,
    term_seq: i64,
    valid_from_millis: i64,
    valid_until_millis: Option<i64>,
) {
    let tenant_text = tenant_id.uuid().to_hyphenated();
    test_runtime().block_on(async {
        let mut tx = app_store
            .pool()
            .begin()
            .await
            .expect("register-key transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .expect("set_config");
        sqlx::query(
            "INSERT INTO source_signing_keys \
             (tenant_id, source_id, key_id, term_seq, public_key, status, \
              valid_from_unix_millis, valid_until_unix_millis, \
              enrolled_at_unix_millis, enrolled_by) \
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&tenant_text)
        .bind(source_id)
        .bind(key_id)
        .bind(term_seq)
        .bind(&public_key[..])
        .bind(status)
        .bind(valid_from_millis)
        .bind(valid_until_millis)
        .bind(valid_from_millis)
        .bind("integration-test")
        .execute(&mut *tx)
        .await
        .expect("source key insert");
        tx.commit().await.expect("commit key insert");
    });
}

fn authenticator(app_store: &FlexCoreStore) -> PgSourceAuthenticator {
    PgSourceAuthenticator::new(
        app_store.clone(),
        SourceAuthMode::Enforced,
        Duration::from_secs(DEFAULT_AUTH_WINDOW_SECS),
        Vec::new(),
    )
}

/// Authenticates, returning the error (panics if authentication
/// unexpectedly succeeds — the caller asserts the rejection).
fn auth_error(
    auth: &PgSourceAuthenticator,
    ctx: &TenantContext,
    payload: &[u8],
) -> SourceAuthError {
    match auth.authenticate(ctx, payload) {
        Ok(verified) => panic!(
            "expected authentication to fail, but it succeeded: {verified:?}"
        ),
        Err(error) => error,
    }
}

// --- 1. Pure: envelope constants ----------------------------------------------

#[test]
fn envelope_constants_have_the_specified_values() {
    assert_eq!(ENVELOPE_DOMAIN, b"flexcore-telemetry-v1");
    assert_eq!(ENVELOPE_VERSION, 1);
    assert_eq!(DEFAULT_AUTH_WINDOW_SECS, 300);
}

// --- 2. Pure: canonical signing bytes, hand-built ------------------------------

#[test]
fn canonical_signing_bytes_match_the_hand_built_layout() {
    // FROZEN KNOWN-ANSWER VECTOR — the expected bytes below are built BY
    // HAND from the layout spec (domain + version byte + u32-LE
    // length-prefixed key_id + u32-LE length-prefixed source_id + 16 raw
    // tenant UUID bytes + u64-LE sequence + i64-LE event_time + 32 raw
    // digest bytes). They must never change without a version bump
    // (`ENVELOPE_VERSION`): any change to this layout is a wire-format
    // break for every enrolled source.
    let tenant_a = TenantId::new(
        FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01").expect("fixed test UUID"),
    );
    let digest_a = [0xab_u8; 32];
    let mut expected_a = Vec::new();
    expected_a.extend_from_slice(b"flexcore-telemetry-v1"); // domain, 21 bytes
    expected_a.push(1); // version, 1 byte
    expected_a.extend_from_slice(&[5, 0, 0, 0]); // u32 LE length of "key-1"
    expected_a.extend_from_slice(b"key-1");
    expected_a.extend_from_slice(&[15, 0, 0, 0]); // u32 LE length of "sim://exit-gate"
    expected_a.extend_from_slice(b"sim://exit-gate");
    expected_a.extend_from_slice(&[
        0x01, 0x92, 0xa5, 0xe0, 0x00, 0x00, 0x7e, 0x2f, 0x9a, 0x1b, 0x3c, 0x4d, 0x5e, 0x6f,
        0x7a, 0x01,
    ]); // tenant UUID, 16 raw bytes
    expected_a.extend_from_slice(&[7, 0, 0, 0, 0, 0, 0, 0]); // sequence 7, u64 LE
    expected_a.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // event_time 0, i64 LE
    expected_a.extend_from_slice(&[0xab; 32]); // payload digest, 32 raw bytes
    assert_eq!(expected_a.len(), 21 + 1 + 4 + 5 + 4 + 15 + 16 + 8 + 8 + 32);
    assert_eq!(
        canonical_signing_bytes(1, "key-1", "sim://exit-gate", &tenant_a, 7, 0, &digest_a),
        expected_a,
        "the frozen known-answer vector must match byte for byte"
    );

    // A second, distinct envelope, also hand-built: different key,
    // source, tenant, sequence, event_time (1000 ms -> 0x03e8 LE) and an
    // all-zero digest.
    let tenant_b = TenantId::new(
        FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02").expect("fixed test UUID"),
    );
    let digest_b = [0x00_u8; 32];
    let mut expected_b = Vec::new();
    expected_b.extend_from_slice(b"flexcore-telemetry-v1");
    expected_b.push(1);
    expected_b.extend_from_slice(&[5, 0, 0, 0]); // u32 LE length of "key-2"
    expected_b.extend_from_slice(b"key-2");
    expected_b.extend_from_slice(&[11, 0, 0, 0]); // u32 LE length of "sim://other" (11 bytes)
    expected_b.extend_from_slice(b"sim://other");
    expected_b.extend_from_slice(&[
        0x01, 0x92, 0xa5, 0xe0, 0x00, 0x00, 0x7e, 0x2f, 0x9a, 0x1b, 0x3c, 0x4d, 0x5e, 0x6f,
        0x7a, 0x02,
    ]);
    expected_b.extend_from_slice(&[42, 0, 0, 0, 0, 0, 0, 0]); // sequence 42, u64 LE
    expected_b.extend_from_slice(&[0xe8, 0x03, 0, 0, 0, 0, 0, 0]); // event_time 1000, i64 LE
    expected_b.extend_from_slice(&[0x00; 32]);
    assert_eq!(
        canonical_signing_bytes(1, "key-2", "sim://other", &tenant_b, 42, 1000, &digest_b),
        expected_b
    );

    // The two envelopes differ, and changing one field changes the bytes.
    assert_ne!(expected_a, expected_b);
    assert_ne!(
        canonical_signing_bytes(1, "key-1", "sim://exit-gate", &tenant_a, 8, 0, &digest_a),
        expected_a,
        "sequence 8 must not collide with sequence 7"
    );
    assert_ne!(
        canonical_signing_bytes(2, "key-1", "sim://exit-gate", &tenant_a, 7, 0, &digest_a),
        expected_a,
        "version 2 must not collide with version 1"
    );
}

// --- 3. Pure: the canonical bytes are what gets signed -------------------------

#[test]
fn ed25519_sign_and_verify_round_trips_over_the_canonical_bytes() {
    let (signing, _) = ephemeral_key();
    let tenant = TenantId::new(
        FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01").expect("fixed test UUID"),
    );
    let digest = Sha256::digest(b"frame payload");
    let digest: [u8; 32] = digest.into();
    let bytes = canonical_signing_bytes(
        ENVELOPE_VERSION,
        "key-1",
        "sim://exit-gate",
        &tenant,
        1,
        1_700_000_000_000,
        &digest,
    );
    let signature = signing.sign(&bytes);
    assert!(
        signing
            .verifying_key()
            .verify(&bytes, &signature)
            .is_ok(),
        "the matching verifying key must accept the signature"
    );
    let (other_signing, _) = ephemeral_key();
    assert!(
        other_signing
            .verifying_key()
            .verify(&bytes, &signature)
            .is_err(),
        "a different key must not verify the signature"
    );
    let mut tampered = bytes.clone();
    tampered[0] ^= 0x01;
    assert!(
        signing
            .verifying_key()
            .verify(&tampered, &signature)
            .is_err(),
        "a tampered signing input must not verify"
    );
}

// --- 4. Pure: the exact quarantine reasons --------------------------------------

#[test]
fn every_source_auth_error_maps_to_its_exact_quarantine_reason() {
    assert_eq!(
        SourceAuthError::MalformedEnvelope("bad json".to_string()).quarantine_reason(),
        "1-receive: source-auth: malformed-envelope"
    );
    assert_eq!(
        SourceAuthError::UnknownSourceOrKey.quarantine_reason(),
        "1-receive: source-auth: unknown-source-or-key"
    );
    assert_eq!(
        SourceAuthError::BadSignature.quarantine_reason(),
        "1-receive: source-auth: bad-signature"
    );
    assert_eq!(
        SourceAuthError::BadDigest.quarantine_reason(),
        "1-receive: source-auth: bad-digest"
    );
    assert_eq!(
        SourceAuthError::ReplayOrStaleSequence.quarantine_reason(),
        "1-receive: source-auth: replay-or-stale-sequence"
    );
    assert_eq!(
        SourceAuthError::OutsideTimeWindow.quarantine_reason(),
        "1-receive: source-auth: outside-time-window"
    );
}

// --- 5. DB: a correctly signed envelope authenticates ---------------------------

#[test]
fn correctly_signed_envelope_authenticates_and_returns_the_verified_envelope() {
    let h = setup("correctly_signed_envelope_authenticates");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let payload = b"simulator frame payload".to_vec();
    let envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        7,
        now,
        &payload,
    );
    let auth = authenticator(&h.app_store);
    let verified = match auth.authenticate(&ctx, &envelope_bytes(&envelope)) {
        Ok(verified) => verified,
        Err(error) => panic!(
            "a correctly signed envelope must authenticate, got {}",
            error.quarantine_reason()
        ),
    };
    assert_eq!(verified.key_id, "key-1");
    assert_eq!(verified.source_id, "sim://exit-gate");
    assert_eq!(verified.tenant_id, tenant);
    assert_eq!(verified.sequence, 7);
    assert_eq!(verified.event_time_unix_millis, now);
    assert!(verified.was_enveloped, "the frame arrived enveloped");
    assert_eq!(verified.payload, payload);
}

// --- 6. DB: tampering ------------------------------------------------------------

#[test]
fn flipping_one_payload_byte_is_a_bad_digest() {
    let h = setup("flipping_one_payload_byte_is_a_bad_digest");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let payload = b"simulator frame payload".to_vec();
    let mut envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        &payload,
    );
    // Flip one byte of the inner payload AFTER signing, keeping the
    // envelope's digest and signature: the recomputed digest no longer
    // matches the envelope's digest.
    let mut inner = STANDARD
        .decode(envelope["payload"].as_str().expect("base64 payload"))
        .expect("payload decodes");
    inner[0] ^= 0x01;
    envelope["payload"] = serde_json::Value::String(STANDARD.encode(&inner));
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::BadDigest),
        "expected BadDigest, got {}",
        error.quarantine_reason()
    );
    assert_eq!(error.quarantine_reason(), "1-receive: source-auth: bad-digest");
}

#[test]
fn flipping_one_signature_byte_is_a_bad_signature() {
    let h = setup("flipping_one_signature_byte_is_a_bad_signature");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let payload = b"simulator frame payload".to_vec();
    let mut envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        &payload,
    );
    // Flip one byte of the signature, keeping the digest intact: the
    // digest still matches, the signature no longer verifies.
    let mut signature = hex_decode(envelope["signature"].as_str().expect("hex signature"));
    signature[0] ^= 0x01;
    envelope["signature"] = serde_json::Value::String(hex_encode(&signature));
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::BadSignature),
        "expected BadSignature, got {}",
        error.quarantine_reason()
    );
    assert_eq!(
        error.quarantine_reason(),
        "1-receive: source-auth: bad-signature"
    );
}

#[test]
fn a_frame_signed_with_an_unregistered_key_is_a_bad_signature() {
    let h = setup("a_frame_signed_with_an_unregistered_key_is_a_bad_signature");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    // Key A is registered; the frame is signed with key B.
    let (_signing_a, public_a) = ephemeral_key();
    let (signing_b, _public_b) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public_a,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let envelope = sign_envelope(
        &signing_b,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"simulator frame payload",
    );
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::BadSignature),
        "signing with key B against registered key A must be BadSignature, got {}",
        error.quarantine_reason()
    );
}

// --- 7. DB: identity binding ------------------------------------------------------

#[test]
fn an_envelope_for_tenant_a_does_not_authenticate_under_tenant_b() {
    let h = setup("an_envelope_for_tenant_a_does_not_authenticate_under_tenant_b");
    let tenant_a = seeded_tenant(&h, 0xE1);
    let tenant_b = seeded_tenant(&h, 0xE2);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant_a,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    // The envelope names tenant A and is correctly signed for tenant A.
    let envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant_a,
        1,
        now,
        b"simulator frame payload",
    );
    // Authenticated under tenant B's context: tenant B has no such key.
    let ctx_b = context_for(tenant_b);
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx_b, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::UnknownSourceOrKey),
        "a tenant-A envelope under tenant B must be UnknownSourceOrKey, got {}",
        error.quarantine_reason()
    );
}

#[test]
fn a_key_registered_for_source_a_does_not_authenticate_source_b() {
    let h = setup("a_key_registered_for_source_a_does_not_authenticate_source_b");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    // Source B claims the identity but presents source A's key: the
    // (source B, key-1) pair was never registered.
    let envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://other-gateway",
        tenant,
        1,
        now,
        b"simulator frame payload",
    );
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::UnknownSourceOrKey),
        "source B with source A's key must be UnknownSourceOrKey, got {}",
        error.quarantine_reason()
    );
}

#[test]
fn key_id_dev_is_rejected_in_enforced_mode() {
    let h = setup("key_id_dev_is_rejected_in_enforced_mode");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, _public) = ephemeral_key();
    let now = now_millis();
    // No registration at all: in Enforced mode `key_id == "dev"` is
    // rejected as UnknownSourceOrKey even though the signature itself is
    // well-formed (dev keys are only honoured in DevKey mode).
    let envelope = sign_envelope(
        &signing,
        "dev",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"simulator frame payload",
    );
    let auth = authenticator(&h.app_store);
    let error = auth_error(&auth, &ctx, &envelope_bytes(&envelope));
    assert!(
        matches!(error, SourceAuthError::UnknownSourceOrKey),
        "key_id \"dev\" in Enforced mode must be UnknownSourceOrKey, got {}",
        error.quarantine_reason()
    );
}

// --- 8. DB: malformed envelopes ----------------------------------------------------

#[test]
fn malformed_envelopes_are_rejected() {
    let h = setup("malformed_envelopes_are_rejected");
    let tenant = seeded_tenant(&h, 0xE1);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    let now = now_millis();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let auth = authenticator(&h.app_store);
    let expect_malformed = |what: &str, bytes: &[u8]| {
        let error = auth_error(&auth, &ctx, bytes);
        assert!(
            matches!(error, SourceAuthError::MalformedEnvelope(_)),
            "{what}: expected MalformedEnvelope, got {}",
            error.quarantine_reason()
        );
        assert_eq!(
            error.quarantine_reason(),
            "1-receive: source-auth: malformed-envelope"
        );
    };

    // Not JSON at all.
    expect_malformed("bad JSON", b"this is not json");
    // Not a JSON object.
    expect_malformed("JSON array", b"[1, 2, 3]");

    let valid = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"simulator frame payload",
    );
    // Wrong version (signed as version 1, presented as version 2).
    let mut wrong_version = valid.clone();
    wrong_version["version"] = serde_json::json!(2);
    expect_malformed("wrong version", &envelope_bytes(&wrong_version));
    // Digest hex too short to be 32 bytes.
    let mut short_digest = valid.clone();
    short_digest["payload_digest"] = serde_json::Value::String("abcd".to_string());
    expect_malformed("bad hex length", &envelope_bytes(&short_digest));
    // Signature hex too short to be 64 bytes.
    let mut short_signature = valid.clone();
    short_signature["signature"] = serde_json::Value::String("00".to_string());
    expect_malformed("bad signature hex length", &envelope_bytes(&short_signature));
    // Payload is not valid base64.
    let mut bad_base64 = valid.clone();
    bad_base64["payload"] = serde_json::Value::String("!!not-base64!!".to_string());
    expect_malformed("bad base64", &envelope_bytes(&bad_base64));
    // A required field is missing entirely.
    let mut missing_signature = valid.clone();
    missing_signature
        .as_object_mut()
        .expect("envelope is an object")
        .remove("signature");
    expect_malformed("missing signature", &envelope_bytes(&missing_signature));
}
