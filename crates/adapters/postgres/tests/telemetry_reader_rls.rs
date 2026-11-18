// Patch A — RLS backstop for the `TelemetryReader` against real PostgreSQL.
//
// (a) No tenant context: the query role sees zero telemetry rows.
// (b) Tenant B's context: zero of tenant A's rows.
// (c) The reader driven with tenant B's context returns zero of tenant A's
//     rows, and the equivalent raw SELECT with the tenant predicate
//     deliberately removed still returns zero of tenant A's rows under
//     tenant B's context — the RLS policy, not the predicate, is the
//     backstop.
// (d) The table owner (`flexcore_migrator`, FORCE ROW LEVEL SECURITY) with
//     no tenant set also sees zero rows.
//
// Gating (REQUIRED DEVIATION from the P2.06 harness in `telemetry.rs`):
// these tests FAIL LOUDLY when `FLEXCORE_TEST_DATABASE_URL` is unset or
// empty — a DB-gated suite must never silently pass.
//
// Fixture prefix byte 0xC4 (tenant A) / 0xD4 (tenant B): distinct from the
// other Patch A reader files so all five can run against one shared test
// database without colliding.

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
use flexcore_ports::telemetry_read::{TelemetryReadQuery, TelemetryReader};
use flexcore_postgres_adapter::telemetry::{
    NormalizedTelemetryRecord, Quantity, TelemetryInsertOutcome, TelemetryValue,
};
use flexcore_postgres_adapter::telemetry_reader::PgTelemetryReader;
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
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
            subject: "integration-test:telemetry-reader-rls".to_string(),
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

/// A record fixture. `ingest_time` is `event_time + 1s` (P0.09 ordering).
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

fn ingest_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(url, "flexcore_ingest", "flexcore_ingest_test_only"))
        .expect("ingest-role store")
}

fn query_pool(url: &str) -> PgPool {
    test_runtime()
        .block_on(PgPool::connect(&url_as(
            url,
            "flexcore_query",
            "flexcore_query_test_only",
        )))
        .expect("query-role pool")
}

fn query_reader(url: &str) -> PgTelemetryReader {
    let store = FlexCoreStore::connect(&url_as(url, "flexcore_query", "flexcore_query_test_only"))
        .expect("query-role store");
    store.telemetry_reader()
}

fn app_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store")
}

/// `count(*)` over the telemetry table as the connected role, with an
/// optional tenant context (RLS scoping). `tenant_text == None` means no
/// `flexcore.tenant_id` is set at all.
fn count_telemetry_as(pool: &PgPool, tenant_text: Option<&str>) -> i64 {
    count_telemetry_where(pool, tenant_text, "TRUE")
}

/// `count(*)` over the telemetry table with an extra SQL predicate
/// (fixed test SQL, never caller text).
fn count_telemetry_where(pool: &PgPool, tenant_text: Option<&str>, predicate: &str) -> i64 {
    test_runtime().block_on(async {
        let mut tx = pool.begin().await.expect("transaction");
        if let Some(tenant) = tenant_text {
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(tenant)
                .execute(&mut *tx)
                .await
                .expect("set_config");
        }
        // AUDITED (Batch 32+): the predicate is fixed test SQL built in-test
        // from hyphenated UUIDs generated here (or the literal "TRUE"); the
        // tenant context is a bound $1 via set_config above, never text.
        let count = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM telemetry WHERE {predicate}"
        )))
        .fetch_one(&mut *tx)
        .await
        .expect("count query");
        tx.commit().await.expect("commit");
        count
    })
}

/// The seeded fixture: tenant A with 3 rows on its asset, tenant B with 2
/// rows on its own asset. Returns `(tenant_a, tenant_b, asset_a, asset_b)`.
fn setup_two_tenants(url: &str) -> (TenantId, TenantId, AssetId, AssetId) {
    let tenant_a = fixture_tenant_id(0xC4);
    let tenant_b = fixture_tenant_id(0xD4);
    let asset_a = fixture_asset_id(0x24);
    let asset_b = fixture_asset_id(0x25);
    let admin_store = FlexCoreStore::connect(url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(url);
    let app = app_store(url);
    seed_registry(&admin_store, &app, tenant_a, asset_a);
    seed_registry(&admin_store, &app, tenant_b, asset_b);
    let ingest = ingest_store(url);
    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);
    for day in [5i64, 6, 7] {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let rec = record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(day as f64)), JAN_2030 + day * 86_400, day);
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &rec).expect("insert"),
            TelemetryInsertOutcome::Stored,
            "tenant A seed row must store"
        );
        unit.commit().expect("commit");
    }
    for day in [5i64, 6] {
        let unit = ingest.begin(Some(&ctx_b)).expect("ingest unit of work");
        let rec = record(tenant_b, asset_b, Quantity::ActivePower, Some(TelemetryValue::Numeric(100.0 + day as f64)), JAN_2030 + day * 86_400, day);
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_b, &rec).expect("insert"),
            TelemetryInsertOutcome::Stored,
            "tenant B seed row must store"
        );
        unit.commit().expect("commit");
    }
    (tenant_a, tenant_b, asset_a, asset_b)
}

// --- (a) No tenant context: the query role sees zero rows -------------------------

#[test]
fn rls_no_tenant_context_sees_zero_rows() {
    let url = require_db("rls_no_tenant_context_sees_zero_rows");
    let (_tenant_a, _tenant_b, _asset_a, _asset_b) = setup_two_tenants(&url);
    let pool = query_pool(&url);

    // No flexcore.tenant_id set anywhere in the session: the fail-closed
    // policy must hide every row, including the whole-table count.
    assert_eq!(
        count_telemetry_as(&pool, None),
        0,
        "with no tenant context set, a direct SELECT on telemetry must return zero rows"
    );
}

// --- (b) Tenant B's context: zero of tenant A's rows --------------------------------

#[test]
fn rls_tenant_b_context_sees_zero_of_tenant_a_rows() {
    let url = require_db("rls_tenant_b_context_sees_zero_of_tenant_a_rows");
    let (tenant_a, tenant_b, asset_a, _asset_b) = setup_two_tenants(&url);
    let pool = query_pool(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated().to_string();
    let tenant_b_text = tenant_b.uuid().to_hyphenated().to_string();
    let asset_a_predicate = format!("asset_id = '{asset_a_text}'::uuid", asset_a_text = asset_a.uuid().to_hyphenated());

    // Control first: tenant A's own context sees its three rows.
    assert_eq!(
        count_telemetry_where(&pool, Some(&tenant_a_text), &asset_a_predicate),
        3,
        "control: tenant A's context must see its own three rows on its asset"
    );
    // Tenant B's context sees none of tenant A's rows on tenant A's asset.
    assert_eq!(
        count_telemetry_where(&pool, Some(&tenant_b_text), &asset_a_predicate),
        0,
        "tenant B's context must see zero of tenant A's rows"
    );
    // And tenant B's whole-table count is exactly its own two rows.
    assert_eq!(
        count_telemetry_as(&pool, Some(&tenant_b_text)),
        2,
        "tenant B's context must see only its own two rows in total"
    );
}

// --- (c) The reader under tenant B + the predicate-free raw query ---------------------

#[test]
fn rls_reader_under_tenant_b_returns_zero_of_tenant_a_rows_and_predicate_free_select_agrees() {
    let url = require_db("rls_reader_under_tenant_b_returns_zero_of_tenant_a_rows_and_predicate_free_select_agrees");
    let (tenant_a, tenant_b, asset_a, _asset_b) = setup_two_tenants(&url);
    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);
    let reader = query_reader(&url);

    // The reader itself, driven with tenant B's TenantContext, returns zero
    // of tenant A's rows — even though its SQL also carries an explicit
    // tenant predicate.
    let query = TelemetryReadQuery::new(
        asset_a,
        EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
        EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
        None,
        None,
        100,
        None,
    )
    .expect("valid query");
    let page = reader
        .query(&ctx_b, query)
        .expect("reader query under tenant B must succeed");
    assert!(
        page.records().is_empty(),
        "the reader driven with tenant B's context must return zero of tenant A's rows"
    );
    assert!(
        page.next_cursor().is_none(),
        "an empty page must carry no next cursor"
    );

    // Control: the same reader under tenant A returns all three rows.
    let query_a = TelemetryReadQuery::new(
        asset_a,
        EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
        EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
        None,
        None,
        100,
        None,
    )
    .expect("valid query");
    let page_a = reader
        .query(&ctx_a, query_a)
        .expect("reader query under tenant A must succeed");
    assert_eq!(
        page_a.records().len(),
        3,
        "control: the reader driven with tenant A's context must return tenant A's three rows"
    );

    // The RLS backstop, isolated: the equivalent raw SELECT with the tenant
    // predicate DELIBERATELY REMOVED (only the asset predicate remains)
    // under tenant B's context still returns zero of tenant A's rows. If
    // only the explicit predicate were doing the isolation, this query
    // would leak all three.
    let pool = query_pool(&url);
    let tenant_b_text = tenant_b.uuid().to_hyphenated().to_string();
    let tenant_a_text = tenant_a.uuid().to_hyphenated().to_string();
    let asset_only = format!(
        "asset_id = '{asset}'::uuid",
        asset = asset_a.uuid().to_hyphenated()
    );
    assert_eq!(
        count_telemetry_where(&pool, Some(&tenant_b_text), &asset_only),
        0,
        "RLS backstop: a predicate-free SELECT (no tenant filter) under tenant B's context must still return zero of tenant A's rows"
    );
    // Control: the same predicate-free SELECT under tenant A's context
    // returns the three rows (the query itself is capable of finding them).
    assert_eq!(
        count_telemetry_where(&pool, Some(&tenant_a_text), &asset_only),
        3,
        "control: the predicate-free SELECT under tenant A's context must return tenant A's three rows"
    );
}

// --- (d) The table owner is still subject to the policy -------------------------------

#[test]
fn rls_table_owner_with_no_tenant_set_sees_zero_rows() {
    let url = require_db("rls_table_owner_with_no_tenant_set_sees_zero_rows");
    let (tenant_a, _tenant_b, _asset_a, _asset_b) = setup_two_tenants(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated().to_string();

    // A session as the table owner (flexcore_migrator) with no tenant set.
    // FORCE ROW LEVEL SECURITY means the owner gets no bypass: zero rows.
    let owner_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("owner-role pool");
    assert_eq!(
        count_telemetry_as(&owner_pool, None),
        0,
        "the table owner with no tenant set must also see zero rows (FORCE ROW LEVEL SECURITY)"
    );

    // Control: the same owner session WITH tenant A's context sees tenant
    // A's three rows — the policy hides rows, it does not revoke the
    // owner's read access outright.
    assert_eq!(
        count_telemetry_as(&owner_pool, Some(&tenant_a_text)),
        3,
        "control: the table owner with tenant A's context must see tenant A's three rows"
    );
}
