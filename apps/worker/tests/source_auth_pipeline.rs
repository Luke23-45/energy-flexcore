// Patch B — source authentication through the real worker pipeline:
// `IngestionTask` constructed with the real `PgSourceAuthenticator`,
// frames offered through the bounded queue, the SAME run-loop path the
// worker hosts.
//
// What is asserted here (all DB-gated, FAIL LOUDLY when
// `FLEXCORE_TEST_DATABASE_URL` is unset or empty — the Patch A pattern,
// a deliberate deviation from the older worker-test skip convention:
// a DB-gated suite must never silently pass):
// 1. A signed frame (Enforced mode, key registered in the DB) is stored
//    as GOOD, with a `TelemetryReceived` outbox event.
// 2. An unsigned frame is rejected and quarantined with
//    `1-receive: source-auth: malformed-envelope`, counted, never stored
//    as GOOD.
// 3. A tampered frame (one inner payload byte flipped) is quarantined
//    with `1-receive: source-auth: bad-digest`.
// 4. A frame with a flipped signature byte is quarantined with
//    `1-receive: source-auth: bad-signature`.
// 5. A replayed frame (the same signed bytes offered twice) is
//    quarantined as replay on the second delivery and never re-stored —
//    the watermark advanced in the first batch.
// 6. THE OLD BEHAVIOUR IS GONE: a frame with the correct
//    `trusted_source_id` but NO valid signature does NOT authenticate —
//    the allowlist alone no longer passes a frame.
// 7. The real simulator source with a signing key emits a well-formed
//    envelope (key_id "dev", per-frame sequence starting at 1, SHA-256
//    digest, Ed25519 signature over the canonical bytes) that verifies
//    against its dev key at the authenticator.
// 8. That simulator frame stores as GOOD through the real pipeline in
//    DevKey mode.
//
// NATS is NOT needed: the pipeline publishes through the OUTBOX port
// only (report §6.3) — the outbox → broker delivery is the P1.15
// publisher's tested path. There is therefore no NATS gate in this file.
//
// Two composition facts the tests encode (see the report for the full
// discussion):
// - The simulator signs with `key_id == "dev"`, which Enforced mode
//   REJECTS as `UnknownSourceOrKey`. So the Enforced end-to-end tests
//   (1–6) use hand-built envelopes under a DB-registered key_id, and the
//   simulator end-to-end tests (7–8) use DevKey mode. The brief's
//   "Enforced mode + simulator source + registered public key"
//   combination cannot store as written, because the simulator's key_id
//   is "dev".
// - The simulator runs on the scenario clock (June 2026), not the wall
//   clock, so its frames are outside the default 300 s window at any
//   later wall-clock time. Tests 7–8 therefore use a wide window
//   (400 days) on the DevKey authenticator, and test 8 creates the
//   2026-06 telemetry partition. A production deployment that ingests
//   simulator frames through the window check needs the window (or the
//   simulator's event_time) reconciled with the scenario clock — that
//   decision is outside this test file.
// - Test 8's decode path is CONFIRMED against the sibling
//   implementation (`apps/worker/src/ingestion.rs`): `process_frame`
//   decodes the VERIFIED inner payload (`VerifiedEnvelope.payload`),
//   not the raw envelope bytes, so the simulator's format check passes
//   on the inner frame. (An earlier draft of this file carried this as
//   a VERIFY-BUILD assumption; the sibling code as landed resolves it.)
//
// Keys are generated ephemerally per test (32 OS-RNG bytes via
// `OsRng::try_fill_bytes` into `SigningKey::from_bytes` — dalek 3 needs
// rand_core 0.10, which rand 0.9's `OsRng` cannot satisfy, so
// `SigningKey::generate` is unusable here); no key material is hardcoded
// anywhere in this file.
//
// Harness notes: the P2.06/P2.10 pattern (fixed 2030 test months,
// partitions created as the migrator role, tenant-owned rows through the
// runtime role with the tenant context). Fixture prefix byte 0xE7:
// distinct from the other Patch B files so all suites can share one test
// database without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey, Verifier};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime, SiteTimezone};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::source_auth::{SourceAuthMode, SourceAuthenticator};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    QuantityValue, SourceStateMapping, TelemetryQuery, TelemetryQuantity, TelemetrySource,
};
use flexcore_postgres_adapter::source_keys::PgSourceAuthenticator;
use flexcore_postgres_adapter::telemetry::Quantity as PgQuantity;
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
use flexcore_simulator::SimulatorTelemetrySource;
use flexcore_worker::inbox::PgInboxStore;
use flexcore_worker::ingestion::{
    IngestionConfig, IngestionMetrics, IngestionRunOutcome, IngestionRunReport, IngestionTask,
    JsonEventPayloadEncoder, PgIngestionInbox, UuidV7Generator,
};
use rand::rand_core::TryRngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

// --- Fixed test months ----------------------------------------------------------

const JAN_2030_MS: i64 = 1_893_456_000_000;
const FEB_2030_MS: i64 = 1_896_134_400_000;
/// 2026-06-01T00:00:00Z — the exit-gate scenario's first tick (the
/// simulator runs on the scenario clock, June 2026).
const JUN_2026_MS: i64 = 1_780_272_000_000;

const PARTITION_MONTHS: [(&str, &str, &str); 6] = [
    ("telemetry_y2026m06", "2026-06-01 00:00:00+00", "2026-07-01 00:00:00+00"),
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
    ("telemetry_y2030m03", "2030-03-01 00:00:00+00", "2030-04-01 00:00:00+00"),
    ("telemetry_y2030m04", "2030-04-01 00:00:00+00", "2030-05-01 00:00:00+00"),
    ("telemetry_y2030m05", "2030-05-01 00:00:00+00", "2030-06-01 00:00:00+00"),
];

const SOURCE_ID: &str = "sim://exit-gate";
const KEY_ID: &str = "key-1";

const SCENARIO_V1: &str = include_str!("../../../scenarios/synthetic-portfolio/scenario-v1.json");

// --- Harness helpers (the P2.10 pattern, fail-loud DB gate) ---------------------

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
             source-auth pipeline suite must not silently pass (test {test}); \
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
    bytes[1] = 0xE7; // Patch B pipeline fixture marker
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "integration-test:source-auth-pipeline".to_string(),
        },
    ))
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

fn blocking_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn ensure_test_roles(pool: &PgPool) {
    blocking_runtime()
        .block_on(async {
            for (role, password) in [
                ("flexcore_app", "flexcore_app_test_only"),
                ("flexcore_migrator", "flexcore_migrator_test_only"),
            ] {
                // SAFETY (sqlx 0.9 SqlSafeStr): `role` and `password` are
                // fixed test-only literals from the list above — no user
                // input reaches this DDL, and role names/passwords cannot
                // be bind parameters in ALTER ROLE.
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"
                )))
                .execute(pool)
                .await?;
            }
            Ok::<(), sqlx::Error>(())
        })
        .expect("test roles must be grantable from the migration/admin connection");
}

fn ensure_partitions(url: &str) {
    let migrator_pool = blocking_runtime()
        .block_on(PgPool::connect(&url_as(
            url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("connect as flexcore_migrator");
    blocking_runtime().block_on(async {
        for (name, from, to) in PARTITION_MONTHS {
            // SAFETY (sqlx 0.9 SqlSafeStr): `name`/`from`/`to` are fixed
            // literals from the PARTITION_MONTHS const above — no user
            // input reaches this DDL, and partition names/bounds cannot
            // be bind parameters in CREATE TABLE.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
                 FOR VALUES FROM ('{from}') TO ('{to}')"
            )))
            .execute(&migrator_pool)
            .await
            .expect("partition DDL as the table owner");
        }
    });
}

fn seed_registry(
    admin_store: &FlexCoreStore,
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    asset_id: AssetId,
) -> TenantContext {
    let tenant_entity = Tenant::try_new(
        tenant_id,
        format!("tenant-{tenant_id}"),
        None,
        format!("idp:{tenant_id}"),
        "profile:default".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid test tenant");
    {
        let unit = admin_store.begin(None).expect("admin unit of work");
        TenantRepository::insert(&unit, &tenant_entity).expect("tenant insert");
        unit.commit().expect("commit tenant");
    }
    let ctx = context_for(tenant_id);
    let portfolio_id = PortfolioId::new(fixture_uuid(0x0F));
    let site_id = SiteId::new(fixture_uuid(0x0C));
    {
        let unit = app_store.begin(Some(&ctx)).expect("app unit of work");
        let portfolio = Portfolio::try_new(
            &tenant_entity,
            portfolio_id,
            "Source-auth pipeline portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "REGION-PLACEHOLDER".to_string(),
            "rules:constrained-aggregation".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid portfolio");
        PortfolioRepository::insert(&unit, &ctx, &portfolio).expect("portfolio insert");
        let site = Site::try_new(
            &portfolio,
            site_id,
            "Source-auth pipeline site".to_string(),
            None,
            "{}".to_string(),
            SiteTimezone::new("Europe/Berlin".to_string()).expect("valid timezone"),
            "pcc:test".to_string(),
            "grid-model:rev-1".to_string(),
            "edge:test".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid site");
        SiteRepository::insert(&unit, &ctx, &site).expect("site insert");
        let asset = Asset::try_new(
            &site,
            asset_id,
            format!("asset-{asset_id}"),
            None,
            AssetType::Bess,
            "vendor:test".to_string(),
            ActivePowerMW::new(2.0).expect("positive rating"),
            Some(EnergyMWh::new(4.0).expect("positive rating")),
            "phase:abc".to_string(),
            "bus:1".to_string(),
            "profile:cap".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid asset");
        AssetRepository::insert(&unit, &ctx, &asset).expect("asset insert");
        unit.commit().expect("commit registry seed");
    }
    ctx
}

struct Harness {
    store: FlexCoreStore,
    app_store: FlexCoreStore,
    ctx: TenantContext,
    asset_id: AssetId,
    inbox: PgInboxStore,
}

fn harness(test: &str, prefix: u8) -> Harness {
    let url = require_db(test);
    ensure_test_roles(
        &blocking_runtime()
            .block_on(PgPool::connect(&url))
            .expect("admin pool"),
    );
    ensure_partitions(&url);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    let store = FlexCoreStore::connect(&url).expect("app store");
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("flexcore_app store");
    let tenant_id = TenantId::new(fixture_uuid(prefix));
    let asset_id = AssetId::new(fixture_uuid(prefix + 1));
    let ctx = seed_registry(&admin_store, &store, tenant_id, asset_id);
    let inbox = PgInboxStore::new(store.pool().clone()).expect("inbox store");
    Harness {
        store,
        app_store,
        ctx,
        asset_id,
        inbox,
    }
}

// --- Envelope / key helpers (no key material hardcoded) ---------------------------

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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is after the Unix epoch")
        .as_millis() as i64
}

fn ephemeral_key() -> (SigningKey, [u8; 32]) {
    let mut seed = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut seed)
        .expect("the OS RNG must be available to generate the ephemeral test keypair");
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
) -> serde_json::Value {
    use flexcore_ports::source_auth::{canonical_signing_bytes, ENVELOPE_VERSION};
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

fn register_key(
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    source_id: &str,
    key_id: &str,
    public_key: &[u8; 32],
    valid_from_millis: i64,
) {
    let tenant_text = tenant_id.uuid().to_hyphenated();
    blocking_runtime().block_on(async {
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

fn enforced_authenticator(app_store: &FlexCoreStore) -> Arc<dyn SourceAuthenticator> {
    Arc::new(PgSourceAuthenticator::new(
        app_store.clone(),
        SourceAuthMode::Enforced,
        Duration::from_secs(300),
        Vec::new(),
    ))
}

// --- Pipeline construction / driving ---------------------------------------------

fn provider_declaration() -> ProviderDeclaration {
    ProviderDeclaration::new(
        SOURCE_ID.to_string(),
        Duration::from_secs(60),
        QualityMappingDeclaration::new(vec![SourceStateMapping {
            source_code: "GOOD".to_string(),
            canonical: TelemetryQuality::Good,
        }])
        .unwrap(),
        ClockQuality::Synchronized,
    )
    .unwrap()
}

fn record(
    tenant_id: TenantId,
    asset_id: AssetId,
    event_millis: i64,
    sequence: u64,
    value_mw: f64,
) -> NormalizedTelemetry {
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id,
        asset_id,
        quantity: TelemetryQuantity::ActivePower,
        value: Some(QuantityValue::Power(ActivePowerMW::new(value_mw).unwrap())),
        event_time: EventTime::from_unix_millis(event_millis).unwrap(),
        ingest_time: IngestTime::from_unix_millis(event_millis + 1_000).unwrap(),
        source_sequence: sequence,
        source_id: SOURCE_ID.to_string(),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .unwrap()
}

fn build_task(
    store: FlexCoreStore,
    records: Vec<NormalizedTelemetry>,
    trusted_source_id: Option<String>,
    inbox: PgInboxStore,
    authenticator: Arc<dyn SourceAuthenticator>,
) -> (
    Arc<IngestionTask>,
    Arc<flexcore_ports::fakes::FakeTelemetrySource>,
    Arc<IngestionMetrics>,
) {
    let source = Arc::new(flexcore_ports::fakes::FakeTelemetrySource::new(
        provider_declaration(),
        records,
    ));
    let metrics = Arc::new(IngestionMetrics::default());
    let task = Arc::new(IngestionTask::new(
        Arc::clone(&source) as Arc<dyn TelemetrySource>,
        store.clone(),
        PgIngestionInbox::new(inbox),
        Arc::new(JsonEventPayloadEncoder),
        Arc::new(SystemClock),
        Arc::new(UuidV7Generator::new(store.pool().clone())),
        IngestionConfig {
            channel_capacity: 100,
            trusted_source_id,
            ..Default::default()
        },
        Arc::clone(&metrics),
        authenticator,
    ));
    (task, source, metrics)
}

fn subscription_query(ctx: &TenantContext, asset_id: AssetId) -> TelemetryQuery {
    TelemetryQuery::new(
        ctx,
        vec![asset_id],
        EventTime::from_unix_millis(JAN_2030_MS).unwrap(),
        EventTime::from_unix_millis(FEB_2030_MS).unwrap(),
        vec![TelemetryQuantity::ActivePower],
    )
    .expect("valid subscription query")
}

/// Drives the run loop: spawn, let it subscribe, offer each frame, let
/// the loop drain, shut down gracefully, and return the run report.
async fn drive(
    task: Arc<IngestionTask>,
    query: TelemetryQuery,
    frames: Vec<Vec<u8>>,
) -> IngestionRunReport {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let run_task = Arc::clone(&task);
    let run_handle = tokio::spawn(async move { run_task.run(query, shutdown_rx).await });
    let queue = task.queue();
    tokio::time::sleep(Duration::from_millis(300)).await; // the loop subscribes
    for frame in frames {
        queue.offer(frame).expect("offered");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(600)).await;
    let _ = shutdown_tx.send(true);
    let report = tokio::time::timeout(Duration::from_secs(10), run_handle)
        .await
        .expect("the task drains within the timeout")
        .expect("the run task does not panic");
    assert_eq!(
        report.outcome,
        Some(IngestionRunOutcome::ShutdownDrained),
        "the run loop must drain cleanly on shutdown"
    );
    report
}

fn count_telemetry(store: &FlexCoreStore, ctx: &TenantContext) -> i64 {
    blocking_runtime().block_on(async {
        let mut tx = store.pool().begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(ctx.tenant_id().uuid().to_hyphenated().to_string())
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM telemetry")
            .fetch_one(&mut *tx)
            .await
            .expect("count query");
        tx.commit().await.expect("commit");
        count
    })
}

fn pending_outbox_keys(store: &FlexCoreStore) -> Vec<String> {
    store
        .pending(1000)
        .expect("outbox pending read")
        .into_iter()
        .map(|entry| entry.idempotency_key().to_string())
        .collect()
}

/// The (stage, reason) pairs of the tenant's quarantine entries.
fn quarantine_stage_reasons(store: &FlexCoreStore, ctx: &TenantContext) -> Vec<(String, String)> {
    let unit = store.begin(Some(ctx)).expect("unit of work");
    let listed = PgUnitOfWork::list_telemetry_quarantine(&unit, ctx, 100)
        .expect("quarantine list");
    unit.commit().expect("commit read");
    listed
        .into_iter()
        .map(|item| (item.entry.stage, item.entry.reason))
        .collect()
}

/// One signed envelope for this harness's tenant/source/key, with the
/// given sequence and inner payload. The event_time is wall-clock now,
/// inside the 300 s window.
fn signed_frame(
    signing: &SigningKey,
    tenant_id: TenantId,
    sequence: u64,
    payload: &[u8],
) -> Vec<u8> {
    envelope_bytes(&sign_envelope(
        signing,
        KEY_ID,
        SOURCE_ID,
        tenant_id,
        sequence,
        now_millis(),
        payload,
    ))
}

// --- 1. Signed frame stores GOOD ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signed_frame_is_stored_as_good() {
    let h = harness("signed_frame_is_stored_as_good", 0x10);
    let (signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        None,
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    let frame = signed_frame(&signing, h.ctx.tenant_id(), 1, b"frame-1");
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![frame],
    )
    .await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 1, "one record stored: {snapshot:?}");
    assert_eq!(snapshot.records_quarantined, 0);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
    let keys = pending_outbox_keys(&h.store);
    assert!(
        keys.iter().any(|k| k.starts_with("telemetry_received:")),
        "a telemetry_received outbox entry must exist: {keys:?}"
    );
}

// --- 2. Unsigned frame ------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsigned_frame_is_rejected_and_quarantined() {
    let h = harness("unsigned_frame_is_rejected_and_quarantined", 0x12);
    let (_signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        None,
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    // Raw bytes, no envelope at all. The fake source would decode these
    // into its preset record if it were ever reached — authentication
    // runs BEFORE decode, so it never is.
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![b"this is not an envelope".to_vec()],
    )
    .await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 0, "nothing unsigned is stored");
    assert_eq!(snapshot.records_quarantined, 1);
    assert_eq!(snapshot.frames_rejected_auth, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);
    let quarantined = quarantine_stage_reasons(&h.store, &h.ctx);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].0, "1-receive");
    assert_eq!(
        quarantined[0].1, "1-receive: source-auth: malformed-envelope",
        "the quarantine reason names the auth failure exactly"
    );
}

// --- 3. Tampered frame (digest) ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tampered_frame_is_quarantined_with_bad_digest() {
    let h = harness("tampered_frame_is_quarantined_with_bad_digest", 0x14);
    let (signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        None,
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    let mut envelope = sign_envelope(
        &signing,
        KEY_ID,
        SOURCE_ID,
        h.ctx.tenant_id(),
        1,
        now_millis(),
        b"frame-1",
    );
    // Flip one byte of the inner payload after signing.
    let mut inner = STANDARD
        .decode(envelope["payload"].as_str().expect("base64 payload"))
        .expect("payload decodes");
    inner[0] ^= 0x01;
    envelope["payload"] = serde_json::Value::String(STANDARD.encode(&inner));
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![envelope_bytes(&envelope)],
    )
    .await;

    assert_eq!(metrics.snapshot().records_stored, 0);
    assert_eq!(metrics.snapshot().records_quarantined, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);
    let quarantined = quarantine_stage_reasons(&h.store, &h.ctx);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].0, "1-receive");
    assert_eq!(quarantined[0].1, "1-receive: source-auth: bad-digest");
}

// --- 4. Tampered frame (signature) ----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_signature_frame_is_quarantined_with_bad_signature() {
    let h = harness("bad_signature_frame_is_quarantined_with_bad_signature", 0x16);
    let (signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        None,
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    let mut envelope = sign_envelope(
        &signing,
        KEY_ID,
        SOURCE_ID,
        h.ctx.tenant_id(),
        1,
        now_millis(),
        b"frame-1",
    );
    // Flip one byte of the signature, keeping the digest intact.
    let mut signature = hex_decode(envelope["signature"].as_str().expect("hex signature"));
    signature[0] ^= 0x01;
    envelope["signature"] = serde_json::Value::String(hex_encode(&signature));
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![envelope_bytes(&envelope)],
    )
    .await;

    assert_eq!(metrics.snapshot().records_stored, 0);
    assert_eq!(metrics.snapshot().records_quarantined, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);
    let quarantined = quarantine_stage_reasons(&h.store, &h.ctx);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].0, "1-receive");
    assert_eq!(quarantined[0].1, "1-receive: source-auth: bad-signature");
}

// --- 5. Replay through the pipeline ---------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replayed_frame_is_quarantined_as_replay_never_re_stored() {
    let h = harness("replayed_frame_is_quarantined_as_replay_never_re_stored", 0x18);
    let (signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        None,
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    // The same signed bytes offered twice. The first delivery stores and
    // advances the watermark to sequence 1 in its batch; the second is a
    // replay at the receive stage — before decode, dedup, or store.
    let frame = signed_frame(&signing, h.ctx.tenant_id(), 1, b"frame-1");
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![frame.clone(), frame],
    )
    .await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 1, "the first delivery stored once");
    assert_eq!(snapshot.records_quarantined, 1, "the replay was quarantined");
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1, "never re-stored");
    let quarantined = quarantine_stage_reasons(&h.store, &h.ctx);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].0, "1-receive");
    assert_eq!(
        quarantined[0].1, "1-receive: source-auth: replay-or-stale-sequence"
    );
}

// --- 6. The old behaviour is gone ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_source_id_without_a_signature_does_not_authenticate() {
    let h = harness("trusted_source_id_without_a_signature_does_not_authenticate", 0x1A);
    let (_signing, public) = ephemeral_key();
    register_key(
        &h.app_store,
        h.ctx.tenant_id(),
        SOURCE_ID,
        KEY_ID,
        &public,
        now_millis() - 3_600_000,
    );
    // The Phase 2 stub would have ACCEPTED this frame: the provider
    // declares exactly the configured trusted source id. With Patch B,
    // the allowlist alone is not authentication — the unsigned frame
    // must still be rejected.
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5)],
        Some(SOURCE_ID.to_string()),
        h.inbox.clone(),
        enforced_authenticator(&h.app_store),
    );
    let _report = drive(
        Arc::clone(&task),
        subscription_query(&h.ctx, h.asset_id),
        vec![b"unsigned frame from the trusted source".to_vec()],
    )
    .await;

    assert_eq!(
        metrics.snapshot().records_stored, 0,
        "the allowlist must not pass an unsigned frame"
    );
    assert_eq!(metrics.snapshot().records_quarantined, 1);
    assert_eq!(metrics.snapshot().frames_rejected_auth, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);
    let quarantined = quarantine_stage_reasons(&h.store, &h.ctx);
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].0, "1-receive");
    assert_eq!(
        quarantined[0].1, "1-receive: source-auth: malformed-envelope",
        "no signature at all is a malformed envelope, not a pass"
    );
}

// --- 7. The real simulator source, signing --------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simulator_signed_frame_verifies_against_its_dev_key() {
    let h = harness("simulator_signed_frame_verifies_against_its_dev_key", 0x1C);
    let (signing, public) = ephemeral_key();
    // The simulator source for the registered asset, with a signing key.
    // `with_signing_key` consumes the source (builder style, per the
    // brief) and `pull_frame` then returns envelope JSON bytes.
    let source = Arc::new(
        SimulatorTelemetrySource::new(
            SCENARIO_V1,
            vec![("site-north-bess-1".to_string(), h.asset_id)],
        )
        .expect("the exit-gate scenario plus the test mapping must build")
        .with_signing_key(signing.clone()),
    );
    source.start();
    let query = TelemetryQuery::new(
        &h.ctx,
        vec![h.asset_id],
        EventTime::from_unix_millis(JUN_2026_MS).unwrap(),
        EventTime::from_unix_millis(JUN_2026_MS + 86_400_000).unwrap(),
        vec![TelemetryQuantity::ActivePower],
    )
    .expect("valid subscription query");
    let subscription = source
        .subscribe(&query)
        .expect("the simulator subscribes the registered asset");
    let frame = source
        .pull_frame(&subscription)
        .expect("pull_frame")
        .expect("the first frame exists");
    let envelope: serde_json::Value =
        serde_json::from_slice(&frame).expect("a signed frame is envelope JSON");
    assert_eq!(envelope["key_id"], "dev", "the simulator signs as key_id \"dev\"");
    assert_eq!(
        envelope["sequence"], 1,
        "the per-frame sequence starts at 1"
    );
    assert_eq!(envelope["version"], 1);
    assert_eq!(
        envelope["source_id"], "sim://synthetic-portfolio-exit-gate",
        "the provider identity is the scenario source id"
    );
    // The digest is the SHA-256 of the inner payload bytes.
    let payload = STANDARD
        .decode(envelope["payload"].as_str().expect("base64 payload"))
        .expect("payload decodes");
    let digest: [u8; 32] = Sha256::digest(&payload).into();
    assert_eq!(
        envelope["payload_digest"].as_str().expect("hex digest"),
        hex_encode(&digest),
        "the envelope digest must be the SHA-256 of its payload"
    );
    // The signature verifies over the canonical bytes with the signing
    // key — independently of the authenticator.
    use flexcore_ports::source_auth::{canonical_signing_bytes, ENVELOPE_VERSION};
    let tenant_id = h.ctx.tenant_id();
    let signing_bytes = canonical_signing_bytes(
        ENVELOPE_VERSION,
        "dev",
        "sim://synthetic-portfolio-exit-gate",
        &tenant_id,
        1,
        envelope["event_time_unix_millis"]
            .as_i64()
            .expect("event_time is an integer"),
        &digest,
    );
    let signature = ed25519_dalek::Signature::from_bytes(
        &hex_decode(envelope["signature"].as_str().expect("hex signature"))
            .try_into()
            .expect("a 64-byte signature"),
    );
    assert!(
        signing
            .verifying_key()
            .verify(&signing_bytes, &signature)
            .is_ok(),
        "the simulator's signature must verify over the canonical bytes"
    );
    // And the authenticator accepts it in DevKey mode. The window is
    // wide (400 days) because the simulator runs on the scenario clock
    // (June 2026), not the wall clock — see the file header.
    let auth = PgSourceAuthenticator::new(
        h.app_store.clone(),
        SourceAuthMode::DevKey,
        Duration::from_secs(400 * 24 * 3600),
        vec![public],
    );
    let verified = match auth.authenticate(&h.ctx, &frame) {
        Ok(verified) => verified,
        Err(error) => panic!(
            "the simulator frame must authenticate in DevKey mode, got {}",
            error.quarantine_reason()
        ),
    };
    assert_eq!(verified.key_id, "dev");
    assert_eq!(verified.source_id, "sim://synthetic-portfolio-exit-gate");
    assert_eq!(verified.sequence, 1);
    assert!(verified.was_enveloped);
    assert_eq!(verified.payload, payload);
    // The verified inner payload decodes through the simulator into
    // canonical records for the registered asset.
    let records = source
        .decode(&subscription, &verified.payload)
        .expect("the verified inner payload decodes");
    assert!(
        !records.is_empty(),
        "the inner frame carries at least one record"
    );
    assert!(
        records.iter().all(|r| r.asset_id() == h.asset_id),
        "all decoded records are for the subscribed asset"
    );
}

// --- 8. The real simulator source, through the real pipeline ------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simulator_signed_frame_stores_good_through_the_real_pipeline() {
    let h = harness("simulator_signed_frame_stores_good_through_the_real_pipeline", 0x1E);
    let (signing, public) = ephemeral_key();
    let source = Arc::new(
        SimulatorTelemetrySource::new(
            SCENARIO_V1,
            vec![("site-north-bess-1".to_string(), h.asset_id)],
        )
        .expect("the exit-gate scenario plus the test mapping must build")
        .with_signing_key(signing),
    );
    source.start();
    let query = TelemetryQuery::new(
        &h.ctx,
        vec![h.asset_id],
        EventTime::from_unix_millis(JUN_2026_MS).unwrap(),
        EventTime::from_unix_millis(JUN_2026_MS + 86_400_000).unwrap(),
        vec![TelemetryQuantity::ActivePower],
    )
    .expect("valid subscription query");
    // This test's own subscription, used only to pull the frame: the run
    // loop subscribes separately inside `run` (the simulator supports
    // multiple subscriptions over its one deterministic stream).
    let pull_subscription = source
        .subscribe(&query)
        .expect("the simulator subscribes the registered asset");
    let frame = source
        .pull_frame(&pull_subscription)
        .expect("pull_frame")
        .expect("the first frame exists");
    // DevKey mode + wide window: the simulator signs as key_id "dev"
    // (rejected in Enforced mode) on the scenario clock (June 2026, far
    // outside the default 300 s window at wall-clock time). See the file
    // header.
    let authenticator: Arc<dyn SourceAuthenticator> = Arc::new(PgSourceAuthenticator::new(
        h.app_store.clone(),
        SourceAuthMode::DevKey,
        Duration::from_secs(400 * 24 * 3600),
        vec![public],
    ));
    let metrics = Arc::new(IngestionMetrics::default());
    let task = Arc::new(IngestionTask::new(
        Arc::clone(&source) as Arc<dyn TelemetrySource>,
        h.store.clone(),
        PgIngestionInbox::new(h.inbox.clone()),
        Arc::new(JsonEventPayloadEncoder),
        Arc::new(SystemClock),
        Arc::new(UuidV7Generator::new(h.store.pool().clone())),
        IngestionConfig {
            channel_capacity: 100,
            ..Default::default()
        },
        Arc::clone(&metrics),
        authenticator,
    ));
    // `process_frame` decodes the VERIFIED inner payload
    // (`VerifiedEnvelope.payload`) — confirmed in the sibling
    // implementation — so the simulator's format check passes on the
    // inner frame and the record stores.
    let _report = drive(Arc::clone(&task), query, vec![frame]).await;
    assert_eq!(
        metrics.snapshot().records_stored, 1,
        "the signed simulator frame stores as GOOD: {:?}",
        metrics.snapshot()
    );
    assert_eq!(metrics.snapshot().records_quarantined, 0);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
    // The stored row is the June 2026 active-power sample.
    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let rows = PgUnitOfWork::query_telemetry(
        &unit,
        &h.ctx,
        &[h.asset_id],
        EventTime::from_unix_millis(JUN_2026_MS).unwrap(),
        EventTime::from_unix_millis(JUN_2026_MS + 86_400_000).unwrap(),
        Some(&[PgQuantity::ActivePower]),
    )
    .expect("range query");
    unit.commit().expect("commit read");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].event_time,
        EventTime::from_unix_millis(JUN_2026_MS).unwrap()
    );
}
