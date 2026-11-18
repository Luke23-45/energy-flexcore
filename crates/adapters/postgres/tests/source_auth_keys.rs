// Patch B — source authentication: key registration, rotation,
// expiry, and revocation, against real PostgreSQL.
//
// What is asserted here (all DB-gated, FAIL LOUDLY when
// `FLEXCORE_TEST_DATABASE_URL` is unset or empty — the Patch A
// `require_db` pattern; no test in this file silently passes):
// 1. Registration: an INSERTed key row is readable through
//    `lookup_source_key` with the right fields.
// 2. Rotation overlap: two ACTIVE key_ids for the same source both
//    authenticate.
// 3. Expiry: a key whose `valid_until` is in the past is
//    `UnknownSourceOrKey`; a key whose `valid_until` is in the future
//    authenticates.
// 4. Revocation: appending a REVOKED term with a higher `term_seq`
//    rejects subsequent frames — including a frame signed BEFORE the
//    revocation but delivered after it.
// 5. Unknown key/source: a never-registered key or source is
//    `UnknownSourceOrKey`.
// 6. `lookup_source_key` returns `Ok(None)` — not an error — for an
//    unregistered key.
//
// Keys are generated ephemerally per test (a 32-byte seed from the OS
// CSPRNG via `OsRng.try_fill_bytes`, then `SigningKey::from_bytes` —
// ed25519-dalek 3.0.0's `generate` cannot take rand 0.9's `OsRng`);
// no key material is hardcoded anywhere in this file.
//
// Rotation/revocation model (the brief's term model): the row with the
// highest `term_seq` for (tenant, source, key_id) is the current term.
// The revocation test asserts the observable behaviour (frames rejected
// after the REVOKED term lands) and that `lookup_source_key` then reads
// the key as NO usable key — the implementation hides unacceptable terms
// behind `None` by design (revoked, expired, and unknown are
// indistinguishable), so the REVOKED row exists in the table but never
// reads back as a row.
//
// Harness notes: same as `source_auth_envelope.rs` (roles with the same
// test-only passwords; key rows INSERTed through `flexcore_app` under the
// tenant GUC). Fixture prefix byte 0xE5: distinct from the other Patch B
// files so all suites can share one test database without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::entities::Tenant;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::TenantRepository;
use flexcore_ports::source_auth::{
    canonical_signing_bytes, SourceAuthError, SourceAuthMode, SourceAuthenticator,
    DEFAULT_AUTH_WINDOW_SECS, ENVELOPE_VERSION,
};
use flexcore_postgres_adapter::source_keys::PgSourceAuthenticator;
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
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
             source-auth keys suite must not silently pass (test {test}); \
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
    bytes[1] = 0xE5; // Patch B source-auth-keys fixture marker
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
            subject: "integration-test:source-auth-keys".to_string(),
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

/// Seeds the tenant row (migration 0026's `source_signing_keys` carries
/// a foreign key to `tenants`, so a key may only be registered for a
/// tenant that exists). Tenants carry no RLS and go through the admin
/// connection (the P2.06 pattern).
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

/// Builds the wire-format envelope JSON for a correctly signed envelope
/// (the test's own construction of the wire format, per the brief).
#[allow(clippy::too_many_arguments)]
fn sign_envelope(
    signing_key: &SigningKey,
    key_id: &str,
    source_id: &str,
    tenant_id: TenantId,
    sequence: u64,
    event_time_millis: i64,
    payload: &[u8],
) -> Vec<u8> {
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
    let value = serde_json::json!({
        "version": ENVELOPE_VERSION,
        "key_id": key_id,
        "source_id": source_id,
        "tenant_id": tenant_id.uuid().to_hyphenated(),
        "sequence": sequence,
        "event_time_unix_millis": event_time_millis,
        "payload_digest": hex_encode(&digest),
        "payload": STANDARD.encode(payload),
        "signature": hex_encode(&signature.to_bytes()),
    });
    serde_json::to_vec(&value).expect("envelope JSON serializes")
}

/// INSERTs one key term through the `flexcore_app` role under the tenant
/// GUC (the role migration 0026 grants SELECT/INSERT to).
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

fn expect_unknown(auth: &PgSourceAuthenticator, ctx: &TenantContext, bytes: &[u8]) {
    match auth.authenticate(ctx, bytes) {
        Ok(_) => panic!("expected UnknownSourceOrKey, but authentication succeeded"),
        Err(error) => {
            assert!(
                matches!(error, SourceAuthError::UnknownSourceOrKey),
                "expected UnknownSourceOrKey, got {}",
                error.quarantine_reason()
            );
            assert_eq!(
                error.quarantine_reason(),
                "1-receive: source-auth: unknown-source-or-key"
            );
        }
    }
}

fn expect_ok(auth: &PgSourceAuthenticator, ctx: &TenantContext, bytes: &[u8]) {
    match auth.authenticate(ctx, bytes) {
        Ok(verified) => {
            assert!(verified.was_enveloped);
        }
        Err(error) => panic!(
            "expected authentication to succeed, got {}",
            error.quarantine_reason()
        ),
    }
}

// --- 1. Registration --------------------------------------------------------------

#[test]
fn registration_is_readable_with_the_right_fields() {
    let h = setup("registration_is_readable_with_the_right_fields");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let (_signing, public) = ephemeral_key();
    let now = now_millis();
    let valid_from = now - 3_600_000;
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "ACTIVE",
        1,
        valid_from,
        None,
    );
    let unit = h
        .app_store
        .begin(Some(&ctx))
        .expect("lookup unit of work");
    let row = PgUnitOfWork::lookup_source_key(&unit, &ctx, "sim://exit-gate", "key-1")
        .expect("lookup_source_key");
    unit.commit().expect("commit read");
    let row = row.expect("the registered key must be found");
    assert_eq!(row.key_id, "key-1");
    assert_eq!(row.source_id, "sim://exit-gate");
    assert_eq!(row.tenant_id, tenant);
    assert_eq!(row.public_key, public.to_vec());
    assert_eq!(row.status, "ACTIVE");
    assert_eq!(row.valid_from_unix_millis, valid_from);
    assert_eq!(row.valid_until_unix_millis, None);
}

// --- 2. Rotation overlap --------------------------------------------------------------

#[test]
fn two_active_keys_for_one_source_both_authenticate() {
    let h = setup("two_active_keys_for_one_source_both_authenticate");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let now = now_millis();
    // Rotation overlap: key-1 (old) and key-2 (new) are both ACTIVE for
    // the same source — frames signed under either must authenticate
    // during the overlap window.
    let (signing_1, public_1) = ephemeral_key();
    let (signing_2, public_2) = ephemeral_key();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public_1,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-2",
        &public_2,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let auth = authenticator(&h.app_store);
    let envelope_1 = sign_envelope(
        &signing_1,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"frame signed with the old key",
    );
    let envelope_2 = sign_envelope(
        &signing_2,
        "key-2",
        "sim://exit-gate",
        tenant,
        2,
        now,
        b"frame signed with the new key",
    );
    expect_ok(&auth, &ctx, &envelope_1);
    expect_ok(&auth, &ctx, &envelope_2);
}

// --- 3. Expiry ---------------------------------------------------------------------------

#[test]
fn an_expired_key_is_unknown_and_a_future_expiry_accepts() {
    let h = setup("an_expired_key_is_unknown_and_a_future_expiry_accepts");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let now = now_millis();
    let (signing_expired, public_expired) = ephemeral_key();
    let (signing_live, public_live) = ephemeral_key();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-expired",
        &public_expired,
        "ACTIVE",
        1,
        now - 7_200_000,
        Some(now - 1_000),
    );
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-live",
        &public_live,
        "ACTIVE",
        1,
        now - 3_600_000,
        Some(now + 3_600_000),
    );
    let auth = authenticator(&h.app_store);
    // Expired (`valid_until` one second in the past): unknown.
    let expired = sign_envelope(
        &signing_expired,
        "key-expired",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"frame signed with the expired key",
    );
    expect_unknown(&auth, &ctx, &expired);
    // Still valid (`valid_until` one hour in the future): accepted.
    let live = sign_envelope(
        &signing_live,
        "key-live",
        "sim://exit-gate",
        tenant,
        2,
        now,
        b"frame signed with the live key",
    );
    expect_ok(&auth, &ctx, &live);
}

// --- 4. Revocation -------------------------------------------------------------------------

#[test]
fn revocation_by_a_higher_term_rejects_frames_signed_before_it() {
    let h = setup("revocation_by_a_higher_term_rejects_frames_signed_before_it");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let now = now_millis();
    let (signing, public) = ephemeral_key();
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
    // A frame signed while the key was ACTIVE, delivered after the
    // revocation lands: the signature is still cryptographically valid,
    // but the key is revoked, so the frame must be rejected. The envelope
    // is built (signed) BEFORE the REVOKED term is appended.
    let envelope = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"frame signed before revocation",
    );
    expect_ok(&auth, &ctx, &envelope);

    // Revocation: a REVOKED term with a higher term_seq for the same key.
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        "REVOKED",
        2,
        now - 3_600_000,
        None,
    );
    // The same pre-revocation frame, delivered after revocation.
    expect_unknown(&auth, &ctx, &envelope);
    // And a freshly signed frame under the revoked key is rejected too.
    let fresh = sign_envelope(
        &signing,
        "key-1",
        "sim://exit-gate",
        tenant,
        2,
        now,
        b"frame signed after revocation",
    );
    expect_unknown(&auth, &ctx, &fresh);

    // The lookup reports the current term as NOT FOUND: `lookup_source_key`
    // returns `None` when the current term is not acceptable (status not
    // ACTIVE, expired, or not yet valid) — revoked, expired, and unknown
    // are deliberately indistinguishable by design, so the REVOKED row
    // still exists in the table but reads back as no usable key.
    let unit = h
        .app_store
        .begin(Some(&ctx))
        .expect("lookup unit of work");
    let row = PgUnitOfWork::lookup_source_key(&unit, &ctx, "sim://exit-gate", "key-1")
        .expect("lookup_source_key");
    unit.commit().expect("commit read");
    assert!(
        row.is_none(),
        "after revocation the key must read back as no usable key, not as a REVOKED row"
    );
}

// --- 5. Unknown key / source -------------------------------------------------------------------

#[test]
fn an_unregistered_key_or_source_is_unknown() {
    let h = setup("an_unregistered_key_or_source_is_unknown");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let now = now_millis();
    // Only key-1 for sim://exit-gate is registered.
    let (signing_registered, public_registered) = ephemeral_key();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public_registered,
        "ACTIVE",
        1,
        now - 3_600_000,
        None,
    );
    let auth = authenticator(&h.app_store);
    // An unregistered key_id for a registered source.
    let (signing_other, _public_other) = ephemeral_key();
    let unknown_key = sign_envelope(
        &signing_other,
        "key-never-registered",
        "sim://exit-gate",
        tenant,
        1,
        now,
        b"frame with an unregistered key",
    );
    expect_unknown(&auth, &ctx, &unknown_key);
    // A registered key_id claimed for a source it was never registered
    // under (the source itself was never registered).
    let unknown_source = sign_envelope(
        &signing_registered,
        "key-1",
        "sim://never-registered",
        tenant,
        2,
        now,
        b"frame from an unregistered source",
    );
    expect_unknown(&auth, &ctx, &unknown_source);
}

// --- 6. Lookup returns None, not an error ---------------------------------------------------------

#[test]
fn lookup_returns_none_not_an_error_for_an_unregistered_key() {
    let h = setup("lookup_returns_none_not_an_error_for_an_unregistered_key");
    let tenant = seeded_tenant(&h, 0xE5);
    let ctx = context_for(tenant);
    let unit = h
        .app_store
        .begin(Some(&ctx))
        .expect("lookup unit of work");
    let row = PgUnitOfWork::lookup_source_key(&unit, &ctx, "sim://exit-gate", "key-1")
        .expect("lookup_source_key must not fail for an unregistered key");
    unit.commit().expect("commit read");
    assert!(
        row.is_none(),
        "an unregistered key must read back as None, not an error"
    );
}
