// Patch A — `TelemetryReader` error behaviour against real PostgreSQL.
//
// A storage failure maps to an opaque `TelemetryReadError::Backend(_)` whose
// Display carries no raw SQL, table name, or connection detail; and a query
// against an asset with no rows returns an empty page (not an error).
//
// Gating (REQUIRED DEVIATION from the P2.06 harness in `telemetry.rs`):
// these tests FAIL LOUDLY when `FLEXCORE_TEST_DATABASE_URL` is unset or
// empty — a DB-gated suite must never silently pass.
//
// Fixture prefix byte 0xC5 (tenant A): distinct from the other Patch A
// reader files so all five can run against one shared test database
// without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
use flexcore_ports::telemetry_read::{TelemetryReadError, TelemetryReadQuery, TelemetryReader};
use flexcore_postgres_adapter::telemetry::{
    NormalizedTelemetryRecord, Quantity, TelemetryValue,
};
use flexcore_postgres_adapter::telemetry_reader::PgTelemetryReader;
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use sqlx::PgPool;

// --- Fixed test months (2030, UTC) ---------------------------------------------

/// 2030-01-01T00:00:00Z (Unix seconds).
const JAN_2030: i64 = 1_893_456_000;

const PARTITION_MONTHS: [(&str, &str, &str); 4] = [
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
    ("telemetry_y2030m03", "2030-03-01 00:00:00+00", "2030-04-01 00:00:00+00"),
    ("telemetry_y2030m04", "2030-04-01 00:00:00+00", "2030-05-01 00:00:00+00"),
];

// --- Harness helpers (copied from the P2.06 harness in `telemetry.rs`) ----------

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
             telemetry-reader suite must not silently pass (test {test}); \
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
    bytes[1] = 0x6A; // P2.06 fixture marker
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

fn fixture_asset_id(prefix: u8) -> AssetId {
    AssetId::new(fixture_uuid(prefix))
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "integration-test:telemetry-reader-errors".to_string(),
        },
    ))
}

fn sample_tenant_entity(tenant_id: TenantId) -> Tenant {
    Tenant::try_new(
        tenant_id,
        format!("tenant-{tenant_id}"),
        None,
        format!("idp:{tenant_id}"),
        "profile:default".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid test tenant")
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

fn ensure_partitions(url: &str) -> PgPool {
    let migrator_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("connect as flexcore_migrator");
    test_runtime().block_on(async {
        // CREATE TABLE IF NOT EXISTS is not concurrency-safe: two
        // sessions can both pass the existence check and collide with
        // 42P07 "already exists". The same advisory lock as the role
        // setup serializes partition creation across test processes.
        let mut tx = migrator_pool.begin().await.expect("partition setup tx");
        sqlx::query("SELECT pg_advisory_xact_lock(20261118)")
            .execute(&mut *tx)
            .await
            .expect("partition setup lock");
        for (name, from, to) in PARTITION_MONTHS {
            // AUDITED (Batch 32+): partition names and calendar-formatted bounds
            // come from the file's own PARTITION_MONTHS const; identifiers and
            // literal bounds cannot be bind parameters.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
                 FOR VALUES FROM ('{from}') TO ('{to}')"
            )))
            .execute(&mut *tx)
            .await
            .expect("partition DDL as the table owner");
        }
        tx.commit().await.expect("partition setup commit");
    });
    migrator_pool
}

fn seed_registry(
    admin_store: &FlexCoreStore,
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    asset_id: AssetId,
) {
    let tenant_entity = sample_tenant_entity(tenant_id);
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
            "Telemetry portfolio".to_string(),
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
            "Telemetry site".to_string(),
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
}

/// A record fixture (kept for parity with the harness; unused here because
/// the empty-page test seeds no telemetry).
#[allow(dead_code)]
fn record(
    tenant_id: TenantId,
    asset_id: AssetId,
    quantity: Quantity,
    value: Option<TelemetryValue>,
    event_seconds: i64,
    sequence: i64,
) -> NormalizedTelemetryRecord {
    NormalizedTelemetryRecord {
        tenant_id,
        asset_id,
        quantity,
        value,
        event_time: EventTime::from_unix_millis(event_seconds * 1000)
            .expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(event_seconds * 1000 + 1_000)
            .expect("valid ingest time"),
        source_sequence: sequence,
        source_id: "sim://exit-gate".to_string(),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        q_sign_convention: if quantity == Quantity::ReactivePower {
            Some(ReactiveSignConvention::InjectionPositive)
        } else {
            None
        },
        clock_quality: ClockQuality::Synchronized,
    }
}

fn app_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store")
}

/// The opaque-error invariant: the rendered error must carry only the
/// stable opaque text — no raw SQL, no table/partition name, no connection
/// detail (host, port, URL, user, password), no engine error text.
fn assert_opaque_display(message: &str, what: &str) {
    assert!(
        !message.is_empty(),
        "{what}: the backend error must carry a non-empty opaque message"
    );
    assert!(
        message.starts_with("telemetry read backend failure: "),
        "{what}: the error must render as the stable opaque text; got: {message}"
    );
    assert_no_leak(message, what);
}

/// The leak check alone, for strings that are not the full Display
/// rendering (the `BackendError` code/message pair).
fn assert_no_leak(message: &str, what: &str) {
    for forbidden in [
        "SELECT",
        "INSERT",
        "UPDATE",
        "DELETE",
        "FROM telemetry",
        "telemetry_quarantine",
        "telemetry_y2030",
        "postgres://",
        "127.0.0.1",
        "password",
        "flexcore_query",
        "Connection refused",
        "connection refused",
        "sqlx::",
        "PoolClosed",
    ] {
        assert!(
            !message.contains(forbidden),
            "{what}: the opaque error must not contain {forbidden:?}; got: {message}"
        );
    }
}

fn assert_backend_error<T: std::fmt::Debug>(
    result: Result<T, TelemetryReadError>,
    what: &str,
) {
    match result {
        Err(TelemetryReadError::Backend(backend)) => {
            assert_opaque_display(
                &TelemetryReadError::Backend(backend.clone()).to_string(),
                what,
            );
            // The code/message accessors must be opaque too, not just the
            // Display rendering.
            assert_no_leak(
                &format!("{} {}", backend.code(), backend.message()),
                what,
            );
        }
        other => panic!("{what}: expected Err(TelemetryReadError::Backend(_)), got {other:?}"),
    }
}

fn full_range_query(asset: AssetId) -> TelemetryReadQuery {
    TelemetryReadQuery::new(
        asset,
        EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
        EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
        None,
        None,
        100,
        None,
    )
    .expect("valid query")
}

// --- 1. Storage failure maps to an opaque backend error -----------------------------

#[test]
fn storage_failure_maps_to_opaque_backend_error() {
    let url = require_db("storage_failure_maps_to_opaque_backend_error");
    let tenant_a = fixture_tenant_id(0xC5);
    let asset_a = fixture_asset_id(0x25);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(&url);
    seed_registry(&admin_store, &app_store(&url), tenant_a, asset_a);
    let ctx_a = context_for(tenant_a);

    // A store whose pool is closed: every read fails at the storage layer.
    let store = FlexCoreStore::connect(&url_as(&url, "flexcore_query", "flexcore_query_test_only"))
        .expect("query-role store");
    test_runtime().block_on(store.pool().close());
    let reader = store.telemetry_reader();

    assert_backend_error(
        reader.query(&ctx_a, full_range_query(asset_a)),
        "query against a closed pool",
    );
    assert_backend_error(
        reader.latest_state(&ctx_a, asset_a),
        "latest_state against a closed pool",
    );
    assert_backend_error(
        reader.list_quarantine(&ctx_a),
        "list_quarantine against a closed pool",
    );
}

// --- 2. Unreachable database URL fails closed ------------------------------------------

#[test]
fn unreachable_database_url_fails_closed() {
    let _url = require_db("unreachable_database_url_fails_closed");
    let tenant_a = fixture_tenant_id(0xC5);
    let asset_a = fixture_asset_id(0x25);
    let ctx_a = context_for(tenant_a);

    // A syntactically valid URL pointing at a port nothing listens on.
    // Pool construction is lazy, so either construction itself fails closed
    // or the reader's first use fails with the opaque backend error —
    // either way nothing succeeds against the unreachable database.
    match FlexCoreStore::connect("postgres://127.0.0.1:1/flexcore_unreachable") {
        Ok(store) => {
            let reader = store.telemetry_reader();
            assert_backend_error(
                reader.query(&ctx_a, full_range_query(asset_a)),
                "query against an unreachable database URL",
            );
            assert_backend_error(
                reader.latest_state(&ctx_a, asset_a),
                "latest_state against an unreachable database URL",
            );
            assert_backend_error(
                reader.list_quarantine(&ctx_a),
                "list_quarantine against an unreachable database URL",
            );
        }
        Err(error) => {
            // Construction failed closed before any read was attempted.
            // The construction error itself is a backend failure, never a
            // successful store.
            let _ = error;
        }
    }
}

// --- 3. Empty page, not an error ---------------------------------------------------------

#[test]
fn query_against_an_asset_with_no_rows_returns_an_empty_page() {
    let url = require_db("query_against_an_asset_with_no_rows_returns_an_empty_page");
    let tenant_a = fixture_tenant_id(0xC5);
    let asset_a = fixture_asset_id(0x25);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(&url);
    seed_registry(&admin_store, &app_store(&url), tenant_a, asset_a);
    let reader = {
        let store = FlexCoreStore::connect(&url_as(
            &url,
            "flexcore_query",
            "flexcore_query_test_only",
        ))
        .expect("query-role store");
        store.telemetry_reader()
    };
    let ctx_a = context_for(tenant_a);

    // The asset exists in the registry but has no telemetry rows.
    let page = reader
        .query(&ctx_a, full_range_query(asset_a))
        .expect("a query against an asset with no rows must succeed");
    assert!(
        page.records().is_empty(),
        "a query against an asset with no rows must return an empty page, not an error"
    );
    assert!(
        page.next_cursor().is_none(),
        "an empty page must carry no next cursor"
    );

    let latest = reader
        .latest_state(&ctx_a, asset_a)
        .expect("latest_state against an asset with no rows must succeed");
    assert!(
        latest.is_empty(),
        "latest_state against an asset with no rows must return an empty list, not an error"
    );

    let quarantine = reader
        .list_quarantine(&ctx_a)
        .expect("list_quarantine for a tenant with no entries must succeed");
    assert!(
        quarantine.is_empty(),
        "list_quarantine for a tenant with no entries must return an empty list, not an error"
    );
}
