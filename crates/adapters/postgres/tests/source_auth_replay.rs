// Patch B — source authentication: replay protection, the sequence
// watermark, and the time-window edges, against real PostgreSQL.
//
// What is asserted here (all DB-gated, FAIL LOUDLY when
// `FLEXCORE_TEST_DATABASE_URL` is unset or empty — the Patch A
// `require_db` pattern; no test in this file silently passes):
// 1. A duplicate (same sequence, same bytes) is accepted the first time
//    and rejected as `ReplayOrStaleSequence` the second time — AFTER the
//    watermark has been advanced. NOTE (the documented Patch B contract):
//    the first `authenticate` does NOT advance the watermark; the advance
//    happens in the batch transaction (`process_batch`). So to test replay
//    you must advance the watermark via `advance_source_sequence` in a
//    unit of work first, then assert the second authenticate rejects.
//    This file tests exactly that sequence of operations.
// 2. A conflict (same sequence, different bytes) is rejected AND the
//    stored watermark is unchanged by the rejected attempt.
// 3. A sequence below the watermark is rejected; a sequence above the
//    watermark with a gap (watermark+5) is accepted.
// 4. The time-window edges: event_time exactly at now-300s and now+300s
//    is accepted; now-301s and now+301s is `OutsideTimeWindow`.
// 5. The watermark primitives themselves: `read_source_sequence` is
//    `None` before any advance; `advance_source_sequence` returns
//    `Advanced` for a new high and `Stale` for an old sequence.
//
// Keys are generated ephemerally per test (a 32-byte seed from the OS
// CSPRNG via `OsRng.try_fill_bytes`, then `SigningKey::from_bytes` —
// ed25519-dalek 3.0.0's `generate` cannot take rand 0.9's `OsRng`);
// no key material is hardcoded anywhere in this file.
//
// "Now" is `OffsetDateTime::now_utc()`, read immediately before signing
// each envelope. TOLERANCE NOTE: the authenticator reads its own clock a
// few milliseconds after the test's read, so an exactly-at-the-edge
// event_time is at most a few milliseconds past the boundary by the time
// it is checked. The specified semantics is inclusive (exactly at the
// edge accepts); if the implementation compares with strict millisecond
// precision the edge-accept test can flake by that margin, and the
// implementation — not the test — would be the thing to fix.
//
// Harness notes: same as `source_auth_envelope.rs` (roles with the same
// test-only passwords; key rows INSERTed through `flexcore_app` under the
// tenant GUC). Fixture prefix byte 0xE3: distinct from the other Patch B
// files so all suites can share one test database without colliding.
//
// FIM WAVE 8 (V-8) — section 7 below (the same-sequence race and the
// distinct-sequence interleave) was added under the Final Implementation
// Milestone program and is WRITTEN-UNRUN: never compiled, never executed
// by the implementing agent. Like every test in this file it is DB-GATED
// on `FLEXCORE_TEST_DATABASE_URL` and runs only in the human V-1 re-run
// of `docs/releases/final-implementation-before-publication.md` §3; the
// gate fails loudly when the variable is unset (no test here silently
// passes).

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
use flexcore_postgres_adapter::source_keys::{AdvanceOutcome, PgSourceAuthenticator};
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
             source-auth replay suite must not silently pass (test {test}); \
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
    bytes[1] = 0xE3; // Patch B source-auth-replay fixture marker
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
            subject: "integration-test:source-auth-replay".to_string(),
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
/// key may only be registered — and a watermark advanced — for a tenant
/// that exists). Tenants carry no RLS and go through the admin
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

// --- Small crypto / encoding helpers (no key material hardcoded) -------------

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((*byte >> 4) as u32, 16).expect("hex digit"));
        out.push(char::from_digit((*byte & 0x0f) as u32, 16).expect("hex digit"));
    }
    out
}

/// Wall-clock "now" in Unix milliseconds, read immediately before use.
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

fn register_key(
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    source_id: &str,
    key_id: &str,
    public_key: &[u8; 32],
    valid_from_millis: i64,
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
             VALUES ($1::uuid, $2, $3, 1, $4, 'ACTIVE', $5, NULL, $5, 'integration-test')",
        )
        .bind(&tenant_text)
        .bind(source_id)
        .bind(key_id)
        .bind(&public_key[..])
        .bind(valid_from_millis)
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

/// One registered source + key + authenticator, fresh per test. Returns
/// the tenant, its context, the signing key, and the authenticator.
struct Fixture {
    tenant: TenantId,
    ctx: TenantContext,
    signing: SigningKey,
    auth: PgSourceAuthenticator,
}

fn fixture(h: &DbHarness) -> Fixture {
    let tenant = fixture_tenant_id(0xE3);
    seed_tenant(&h.admin_store, tenant);
    let ctx = context_for(tenant);
    let (signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        tenant,
        "sim://exit-gate",
        "key-1",
        &public,
        now_millis() - 3_600_000,
    );
    let auth = authenticator(&h.app_store);
    Fixture {
        tenant,
        ctx,
        signing,
        auth,
    }
}

fn envelope(f: &Fixture, sequence: u64, event_time_millis: i64, payload: &[u8]) -> Vec<u8> {
    sign_envelope(
        &f.signing,
        "key-1",
        "sim://exit-gate",
        f.tenant,
        sequence,
        event_time_millis,
        payload,
    )
}

fn expect_ok(f: &Fixture, bytes: &[u8]) {
    match f.auth.authenticate(&f.ctx, bytes) {
        Ok(verified) => {
            assert!(verified.was_enveloped);
        }
        Err(error) => panic!(
            "expected authentication to succeed, got {}",
            error.quarantine_reason()
        ),
    }
}

fn expect_replay(f: &Fixture, bytes: &[u8]) {
    match f.auth.authenticate(&f.ctx, bytes) {
        Ok(_) => panic!("expected ReplayOrStaleSequence, but authentication succeeded"),
        Err(error) => {
            assert!(
                matches!(error, SourceAuthError::ReplayOrStaleSequence),
                "expected ReplayOrStaleSequence, got {}",
                error.quarantine_reason()
            );
            assert_eq!(
                error.quarantine_reason(),
                "1-receive: source-auth: replay-or-stale-sequence"
            );
        }
    }
}

/// Advances the watermark for this fixture's source in its own unit of
/// work and commits (the batch-transaction side of the contract).
fn advance(h: &DbHarness, f: &Fixture, sequence: u64) -> AdvanceOutcome {
    let unit = h
        .app_store
        .begin(Some(&f.ctx))
        .expect("advance unit of work");
    let outcome = PgUnitOfWork::advance_source_sequence(&unit, &f.ctx, "sim://exit-gate", sequence)
        .expect("advance_source_sequence");
    unit.commit().expect("commit advance");
    outcome
}

fn read_watermark(h: &DbHarness, f: &Fixture) -> Option<u64> {
    let unit = h
        .app_store
        .begin(Some(&f.ctx))
        .expect("read unit of work");
    let watermark =
        PgUnitOfWork::read_source_sequence(&unit, &f.ctx, "sim://exit-gate")
            .expect("read_source_sequence");
    unit.commit().expect("commit read");
    watermark
}

// --- 1. Duplicate: authenticate does NOT advance; the batch advance does -------

#[test]
fn duplicate_sequence_is_rejected_only_after_the_watermark_advances() {
    let h = setup("duplicate_sequence_is_rejected_after_the_watermark_advances");
    let f = fixture(&h);
    let now = now_millis();
    let bytes = envelope(&f, 5, now, b"frame payload, sequence 5");

    // The first authenticate succeeds — and does NOT advance the
    // watermark (the advance happens in the batch transaction, not here).
    expect_ok(&f, &bytes);
    assert_eq!(
        read_watermark(&h, &f),
        None,
        "authenticate alone must not advance the watermark"
    );

    // The batch transaction advances the watermark to 5.
    assert!(matches!(advance(&h, &f, 5), AdvanceOutcome::Advanced));
    assert_eq!(read_watermark(&h, &f), Some(5));

    // The same bytes a second time are now a replay.
    expect_replay(&f, &bytes);
}

// --- 2. Conflict: same sequence, different bytes --------------------------------

#[test]
fn same_sequence_different_bytes_is_rejected_and_the_watermark_is_unchanged() {
    let h = setup("same_sequence_different_bytes_is_rejected_and_the_watermark_is_unchanged");
    let f = fixture(&h);
    let now = now_millis();
    assert!(matches!(advance(&h, &f, 5), AdvanceOutcome::Advanced));

    // Sequence 5 with payload A: rejected (the watermark is already 5).
    let bytes_a = envelope(&f, 5, now, b"frame payload A, sequence 5");
    expect_replay(&f, &bytes_a);
    // Sequence 5 with DIFFERENT payload B (correctly signed, correct
    // digest — only the content differs): also rejected, and the rejected
    // attempt must not move the watermark.
    let bytes_b = envelope(&f, 5, now, b"frame payload B, sequence 5");
    assert_ne!(bytes_a, bytes_b);
    expect_replay(&f, &bytes_b);
    assert_eq!(
        read_watermark(&h, &f),
        Some(5),
        "a rejected conflicting frame must not move the watermark"
    );
}

// --- 3. Old sequence ---------------------------------------------------------------

#[test]
fn sequence_below_the_watermark_is_rejected() {
    let h = setup("sequence_below_the_watermark_is_rejected");
    let f = fixture(&h);
    let now = now_millis();
    assert!(matches!(advance(&h, &f, 10), AdvanceOutcome::Advanced));
    let bytes = envelope(&f, 9, now, b"old frame, sequence 9");
    expect_replay(&f, &bytes);
}

// --- 4. Gap acceptance ---------------------------------------------------------------

#[test]
fn sequence_above_the_watermark_with_a_gap_is_accepted() {
    let h = setup("sequence_above_the_watermark_with_a_gap_is_accepted");
    let f = fixture(&h);
    let now = now_millis();
    assert!(matches!(advance(&h, &f, 10), AdvanceOutcome::Advanced));
    // Immediately above the watermark.
    expect_ok(&f, &envelope(&f, 11, now, b"frame, sequence 11"));
    // A gap of five: accepted (the watermark is a floor, not an exact
    // next-sequence demand — gaps are the drop/out-of-order reality).
    expect_ok(&f, &envelope(&f, 15, now, b"frame, sequence 15"));
}

// --- 5. Window edges -----------------------------------------------------------------

#[test]
fn event_time_at_the_window_edges_is_accepted() {
    let h = setup("event_time_at_the_window_edges_is_accepted");
    let f = fixture(&h);
    // "Now" is read immediately before signing each envelope (see the
    // tolerance note in the file header).
    let now = now_millis();
    expect_ok(
        &f,
        &envelope(&f, 1, now - 300_000, b"frame at now minus 300s"),
    );
    let now = now_millis();
    expect_ok(
        &f,
        &envelope(&f, 2, now + 300_000, b"frame at now plus 300s"),
    );
}

#[test]
fn event_time_outside_the_window_is_rejected() {
    let h = setup("event_time_outside_the_window_is_rejected");
    let f = fixture(&h);
    let now = now_millis();
    for (what, event_time) in [
        ("now minus 301s", now - 301_000),
        ("now plus 301s", now + 301_000),
    ] {
        let bytes = envelope(&f, 3, event_time, b"frame outside the window");
        match f.auth.authenticate(&f.ctx, &bytes) {
            Ok(_) => panic!("{what}: expected OutsideTimeWindow, but authentication succeeded"),
            Err(error) => {
                assert!(
                    matches!(error, SourceAuthError::OutsideTimeWindow),
                    "{what}: expected OutsideTimeWindow, got {}",
                    error.quarantine_reason()
                );
                assert_eq!(
                    error.quarantine_reason(),
                    "1-receive: source-auth: outside-time-window"
                );
            }
        }
    }
}

// --- 6. The watermark primitives -------------------------------------------------------

#[test]
fn watermark_reads_none_before_any_advance_and_advance_reports_stale() {
    let h = setup("watermark_reads_none_before_any_advance_and_advance_reports_stale");
    let f = fixture(&h);
    assert_eq!(
        read_watermark(&h, &f),
        None,
        "no sequence event recorded yet"
    );
    assert!(matches!(advance(&h, &f, 10), AdvanceOutcome::Advanced));
    assert_eq!(read_watermark(&h, &f), Some(10));
    // Advancing to an older sequence is Stale, not an error, and the
    // watermark stands.
    assert!(matches!(advance(&h, &f, 9), AdvanceOutcome::Stale));
    assert_eq!(read_watermark(&h, &f), Some(10));
    // Advancing to the same sequence again is Stale too.
    assert!(matches!(advance(&h, &f, 10), AdvanceOutcome::Stale));
    assert_eq!(read_watermark(&h, &f), Some(10));
}

// --- 7. The same-sequence race (V-8, FIM wave 8) ---------------------------------------
//
// WRITTEN-UNRUN under the Final Implementation Milestone program; runs
// only in the human V-1 re-run (see the file header).
//
// What is pinned here: `advance_source_sequence` serializes concurrent
// advances for ONE source under `pg_advisory_xact_lock(hashtext(tenant
// || ':' || source))` and re-reads MAX(sequence) UNDER the lock, so two
// concurrent submissions of the same (tenant, source, sequence) resolve
// to exactly one `Advanced` and the rest `Stale` — the replay window the
// authenticator's early watermark filter cannot close by itself (its
// read and the advance are two transactions by design; the advance is
// the authority). The watermark moves only by INSERT, so a Stale
// submission writes nothing.
//
// No new advisory-lock key is introduced anywhere in this section: the
// race exercises the PRODUCTION `advance_source_sequence` lock (the
// hashtext key), and role setup is the file's existing `ensure_test_roles`
// copy with its shared `20261118` key — the recorded 20261118 watch item
// (a new copy must reuse the same key) is untouched because no copy is
// added.
//
// Concurrency shape: `std::thread::scope` threads, each opening its OWN
// unit of work (its own transaction and pool connection) and committing
// it — exactly the batch-transaction contract the early filter rides on.
// The store pool holds five connections (`FlexCoreStore::connect`); a
// thread holding a unit of work waits either on the advisory lock or on
// the pool, and the lock-holder always commits and releases, so every
// thread count makes progress — five keeps the whole race inside one
// pool generation.
const RACE_SUBMISSIONS: usize = 5;

#[test]
fn concurrent_same_sequence_submissions_resolve_to_exactly_one_advanced() {
    let h = setup("concurrent_same_sequence_submissions_resolve_to_exactly_one_advanced");
    let f = fixture(&h);
    let sequence = 500u64;

    // RACE_SUBMISSIONS concurrent submissions of the SAME
    // (tenant, source, sequence), released together by a barrier so the
    // submissions genuinely overlap rather than queueing by accident.
    let outcomes: Vec<AdvanceOutcome> = std::thread::scope(|scope| {
        let barrier = std::sync::Barrier::new(RACE_SUBMISSIONS);
        let handles: Vec<_> = (0..RACE_SUBMISSIONS)
            .map(|_| {
                let h = &h;
                let f = &f;
                // The barrier is shared, not moved: the spawned closure is
                // `move`, so every capture must be a Copy reference.
                let barrier = &barrier;
                scope.spawn(move || {
                    // Begin BEFORE the barrier: every thread holds its
                    // transaction (and connection) while the barrier
                    // releases, so the advisory-lock contention is real.
                    let unit = h
                        .app_store
                        .begin(Some(&f.ctx))
                        .expect("race unit of work");
                    barrier.wait();
                    let outcome = PgUnitOfWork::advance_source_sequence(
                        &unit,
                        &f.ctx,
                        "sim://exit-gate",
                        sequence,
                    )
                    .expect("race advance_source_sequence");
                    unit.commit().expect("race commit");
                    outcome
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("race thread"))
            .collect()
    });

    // Exactly one submission advanced the watermark; every other
    // submission of the same sequence is Stale (a replay or a stale
    // frame), and nothing else was written.
    assert_eq!(outcomes.len(), RACE_SUBMISSIONS);
    let advanced = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, AdvanceOutcome::Advanced))
        .count();
    let stale = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, AdvanceOutcome::Stale))
        .count();
    assert_eq!(
        advanced, 1,
        "exactly one same-sequence submission may advance"
    );
    assert_eq!(
        stale,
        RACE_SUBMISSIONS - 1,
        "every other same-sequence submission is Stale"
    );
    // The one Advanced submission moved the watermark to exactly the
    // raced sequence — and the Stale submissions moved it nowhere else.
    assert_eq!(read_watermark(&h, &f), Some(sequence));
}

#[test]
fn concurrent_distinct_sequences_advance_in_order_and_the_highest_lands() {
    let h = setup("concurrent_distinct_sequences_advance_in_order_and_the_highest_lands");
    let f = fixture(&h);
    let base = 600u64;
    let highest = base + RACE_SUBMISSIONS as u64 - 1;

    // The DIFFERENT-sequence interleave: RACE_SUBMISSIONS concurrent
    // submissions of DISTINCT increasing sequences for ONE source. Under
    // the advisory lock each submission either advances (its sequence was
    // above the watermark its turn saw) or is Stale (below it); the
    // HIGHEST submitted sequence always advances — it is above every
    // watermark that can exist before its turn — so the watermark lands
    // exactly on it in every interleaving.
    let outcomes: Vec<(u64, AdvanceOutcome)> = std::thread::scope(|scope| {
        let barrier = std::sync::Barrier::new(RACE_SUBMISSIONS);
        let handles: Vec<_> = (0..RACE_SUBMISSIONS)
            .map(|i| {
                let h = &h;
                let f = &f;
                // The barrier is shared, not moved: the spawned closure is
                // `move`, so every capture must be a Copy reference.
                let barrier = &barrier;
                scope.spawn(move || {
                    let sequence = base + i as u64;
                    let unit = h
                        .app_store
                        .begin(Some(&f.ctx))
                        .expect("interleave unit of work");
                    barrier.wait();
                    let outcome = PgUnitOfWork::advance_source_sequence(
                        &unit,
                        &f.ctx,
                        "sim://exit-gate",
                        sequence,
                    )
                    .expect("interleave advance_source_sequence");
                    unit.commit().expect("interleave commit");
                    (sequence, outcome)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("interleave thread"))
            .collect()
    });

    assert_eq!(outcomes.len(), RACE_SUBMISSIONS);
    // The highest sequence advanced and the watermark is exactly it.
    assert!(
        outcomes
            .iter()
            .any(|(sequence, outcome)| {
                *sequence == highest && matches!(outcome, AdvanceOutcome::Advanced)
            }),
        "the highest submitted sequence must advance in any interleaving"
    );
    let advanced_sequences: Vec<u64> = outcomes
        .iter()
        .filter(|(_, outcome)| matches!(outcome, AdvanceOutcome::Advanced))
        .map(|(sequence, _)| *sequence)
        .collect();
    assert!(
        !advanced_sequences.is_empty(),
        "at least the highest sequence advances"
    );
    assert_eq!(
        read_watermark(&h, &f),
        Some(highest),
        "the watermark lands on the highest submitted sequence in any interleaving"
    );

    // The "in order" pin, interleaving-independent: after the race, an
    // ascending walk over every raced sequence plus one above it advances
    // ONLY above the watermark and is Stale at or below it — the watermark
    // is a floor, never an exact next-sequence demand, and it never moves
    // backwards.
    assert!(matches!(
        advance(&h, &f, highest + 1),
        AdvanceOutcome::Advanced
    ));
    for sequence in base..=highest {
        assert!(
            matches!(advance(&h, &f, sequence), AdvanceOutcome::Stale),
            "sequence {sequence} is at or below the watermark after the race"
        );
    }
    assert_eq!(read_watermark(&h, &f), Some(highest + 1));
}
