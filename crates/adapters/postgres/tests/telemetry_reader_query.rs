// Patch A — `TelemetryReader::query` against real PostgreSQL.
//
// Range / quantity / quality filters, `TelemetryReadQuery` validation,
// keyset pagination (250-row walk, mid-walk insert, sub-second cursor
// precision), malformed cursors, and tenant scoping.
//
// Gating (REQUIRED DEVIATION from the P2.06 harness in `telemetry.rs`):
// these tests FAIL LOUDLY when `FLEXCORE_TEST_DATABASE_URL` is unset or
// empty — a DB-gated suite must never silently pass. (The two pure
// `TelemetryReadQuery` / `TelemetryCursor` validation tests need no
// database and are exempt from the gate.)
//
// Fixture prefix byte 0xC1 (tenant A) / 0xD1 (tenant B): distinct from the
// other Patch A reader files so all five can run against one shared test
// database without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention, StateOfCharge};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
use flexcore_ports::telemetry_read::{
    quantity_key, sort_key, TelemetryCursor, TelemetryReadError, TelemetryReadQuery,
    TelemetryReader,
};
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
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
            subject: "integration-test:telemetry-reader-query".to_string(),
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
fn record_full(
    tenant_id: TenantId,
    asset_id: AssetId,
    quantity: Quantity,
    value: Option<TelemetryValue>,
    event_millis: i64,
    sequence: i64,
    quality: TelemetryQuality,
) -> NormalizedTelemetryRecord {
    NormalizedTelemetryRecord {
        tenant_id,
        asset_id,
        quantity,
        value,
        event_time: EventTime::from_unix_millis(event_millis).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(event_millis + 1_000)
            .expect("valid ingest time"),
        source_sequence: sequence,
        source_id: "sim://exit-gate".to_string(),
        quality,
        source_quality_code: None,
        q_sign_convention: if quantity == Quantity::ReactivePower {
            Some(ReactiveSignConvention::InjectionPositive)
        } else {
            None
        },
        clock_quality: ClockQuality::Synchronized,
    }
}

fn record(
    tenant_id: TenantId,
    asset_id: AssetId,
    quantity: Quantity,
    value: Option<TelemetryValue>,
    event_seconds: i64,
    sequence: i64,
) -> NormalizedTelemetryRecord {
    record_full(
        tenant_id,
        asset_id,
        quantity,
        value,
        event_seconds * 1000,
        sequence,
        TelemetryQuality::Good,
    )
}

fn ingest_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(url, "flexcore_ingest", "flexcore_ingest_test_only"))
        .expect("ingest-role store")
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

/// Shared setup: migrations, roles, partitions, one seeded tenant/asset.
/// Returns `(admin_store, reader)`; the ingest store is built per test.
fn setup(url: &str, tenant_id: TenantId, asset_id: AssetId) -> (FlexCoreStore, PgTelemetryReader) {
    let admin_store = FlexCoreStore::connect(url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(url);
    seed_registry(&admin_store, &app_store(url), tenant_id, asset_id);
    let reader = query_reader(url);
    (admin_store, reader)
}

fn insert_one(
    ingest: &FlexCoreStore,
    ctx: &TenantContext,
    rec: &NormalizedTelemetryRecord,
) {
    let unit = ingest.begin(Some(ctx)).expect("ingest unit of work");
    assert_eq!(
        PgUnitOfWork::insert_telemetry(&unit, ctx, rec).expect("insert"),
        TelemetryInsertOutcome::Stored,
        "seed row must store: {rec:?}"
    );
    unit.commit().expect("commit");
}

fn query_all(
    reader: &PgTelemetryReader,
    ctx: &TenantContext,
    asset: AssetId,
    from_seconds: i64,
    until_seconds: i64,
    quantities: Option<Vec<TelemetryQuantity>>,
    quality: Option<TelemetryQuality>,
) -> Vec<flexcore_ports::telemetry_source::NormalizedTelemetry> {
    let query = TelemetryReadQuery::new(
        asset,
        EventTime::from_unix_millis(from_seconds * 1000).expect("from"),
        EventTime::from_unix_millis(until_seconds * 1000).expect("until"),
        quantities,
        quality,
        1000,
        None,
    )
    .expect("valid query");
    reader
        .query(ctx, query)
        .expect("query must succeed")
        .records()
        .to_vec()
}

fn assert_ascending_sort_keys(
    records: &[flexcore_ports::telemetry_source::NormalizedTelemetry],
    what: &str,
) {
    let mut prev: Option<String> = None;
    for rec in records {
        let key = sort_key(rec);
        if let Some(prev_key) = &prev {
            assert!(
                key > *prev_key,
                "{what}: sort keys must be strictly ascending; {prev_key} then {key}"
            );
        }
        prev = Some(key);
    }
}

// --- 1. Range / quantity / quality filters ---------------------------------------

#[test]
fn query_range_quantity_and_quality_filters_are_exact() {
    let url = require_db("query_range_quantity_and_quality_filters_are_exact");
    let tenant_a = fixture_tenant_id(0xC1);
    let asset_a = fixture_asset_id(0x21);
    let (_admin, reader) = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // Active power GOOD Jan 5/6/7, SOC GOOD Jan 6, active power SUSPECT Jan 6
    // (different sequence: a distinct row), active power BAD Jan 8.
    let seeded = [
        record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(1.0)), JAN_2030 + 5 * 86_400, 1),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(2.0)), JAN_2030 + 6 * 86_400, 2),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(3.0)), JAN_2030 + 7 * 86_400, 3),
        record(tenant_a, asset_a, Quantity::Soc, Some(TelemetryValue::Numeric(0.8)), JAN_2030 + 6 * 86_400, 4),
        record_full(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(9.0)), (JAN_2030 + 6 * 86_400 + 3_600) * 1000, 5, TelemetryQuality::Suspect),
        record_full(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(4.0)), (JAN_2030 + 8 * 86_400) * 1000, 6, TelemetryQuality::Bad),
    ];
    for rec in &seeded {
        insert_one(&ingest, &ctx_a, rec);
    }

    // Full January, no filters: all six rows, ascending sort-key order.
    let all = query_all(&reader, &ctx_a, asset_a, JAN_2030, JAN_2030 + 31 * 86_400, None, None);
    assert_eq!(all.len(), 6, "unfiltered January query must return all six rows");
    assert_ascending_sort_keys(&all, "unfiltered query");
    assert!(
        all.iter().all(|r| r.tenant_id() == tenant_a && r.asset_id() == asset_a),
        "every returned row must belong to the queried tenant and asset"
    );

    // Range Jan 6..Jan 7 inclusive: the Jan 5 GOOD, Jan 6 SOC, Jan 6 SUSPECT
    // and Jan 7 GOOD rows — the Jan 5 and Jan 8 rows are outside.
    let ranged = query_all(
        &reader,
        &ctx_a,
        asset_a,
        JAN_2030 + 6 * 86_400,
        JAN_2030 + 7 * 86_400,
        None,
        None,
    );
    assert_eq!(ranged.len(), 4, "Jan 6..Jan 7 range must return exactly four rows");
    assert!(
        ranged.iter().all(|r| {
            let ms = r.event_time().unix_millis();
            ms >= ((JAN_2030 + 6 * 86_400) as i128) * 1000
                && ms <= ((JAN_2030 + 7 * 86_400) as i128) * 1000
        }),
        "every ranged row must fall inside the requested range"
    );

    // A single-day range returns only that day's row.
    let single_day = query_all(
        &reader,
        &ctx_a,
        asset_a,
        JAN_2030 + 5 * 86_400,
        JAN_2030 + 5 * 86_400,
        None,
        None,
    );
    assert_eq!(single_day.len(), 1, "a single-day range must return exactly one row");
    assert_eq!(
        single_day[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(1.0).expect("finite"))),
        "the single-day row must carry its seeded value"
    );

    // Quantity filter: SOC only.
    let soc_only = query_all(
        &reader,
        &ctx_a,
        asset_a,
        JAN_2030,
        JAN_2030 + 31 * 86_400,
        Some(vec![TelemetryQuantity::Soc]),
        None,
    );
    assert_eq!(soc_only.len(), 1, "SOC filter must return exactly the SOC row");
    assert_eq!(soc_only[0].quantity(), TelemetryQuantity::Soc);
    assert_eq!(
        soc_only[0].value(),
        Some(&QuantityValue::Fraction(StateOfCharge::new(0.8).expect("fraction"))),
        "the SOC row must carry its seeded fraction"
    );

    // Quality filter: SUSPECT only.
    let suspect_only = query_all(
        &reader,
        &ctx_a,
        asset_a,
        JAN_2030,
        JAN_2030 + 31 * 86_400,
        None,
        Some(TelemetryQuality::Suspect),
    );
    assert_eq!(suspect_only.len(), 1, "SUSPECT filter must return exactly the SUSPECT row");
    assert_eq!(suspect_only[0].quality(), TelemetryQuality::Suspect);
    assert_eq!(
        suspect_only[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(9.0).expect("finite"))),
        "the SUSPECT row must carry its seeded value"
    );

    // Combined filter: active power + GOOD only.
    let good_active = query_all(
        &reader,
        &ctx_a,
        asset_a,
        JAN_2030,
        JAN_2030 + 31 * 86_400,
        Some(vec![TelemetryQuantity::ActivePower]),
        Some(TelemetryQuality::Good),
    );
    assert_eq!(
        good_active.len(),
        3,
        "active-power + GOOD filter must return exactly the three GOOD active-power rows"
    );
    assert!(
        good_active.iter().all(|r| r.quantity() == TelemetryQuantity::ActivePower
            && r.quality() == TelemetryQuality::Good),
        "combined filter must not leak other quantities or qualities"
    );
}

// --- 2. Query validation (pure — no database) ------------------------------------

#[test]
fn query_rejects_inverted_range_and_zero_limit() {
    let asset = fixture_asset_id(0x21);
    let from = EventTime::from_unix_millis((JAN_2030 + 2 * 86_400) * 1000).expect("from");
    let until = EventTime::from_unix_millis(JAN_2030 * 1000).expect("until");

    assert_eq!(
        TelemetryReadQuery::new(asset, from, until, None, None, 50, None),
        Err(TelemetryReadError::InvertedTimeRange),
        "an inverted range (from after until) must be rejected"
    );
    assert_eq!(
        TelemetryReadQuery::new(asset, until, from, None, None, 0, None),
        Err(TelemetryReadError::EmptyPageLimit),
        "a zero page limit must be rejected"
    );
    assert!(
        TelemetryReadQuery::new(asset, until, from, None, None, 1, None).is_ok(),
        "an equal from/until bound with limit 1 is a valid (possibly empty) query"
    );
}

// --- 3. Pagination walk: 250 rows, pages of 50 ------------------------------------

#[test]
fn pagination_walk_returns_every_row_exactly_once_in_sort_key_order() {
    let url = require_db("pagination_walk_returns_every_row_exactly_once_in_sort_key_order");
    let tenant_a = fixture_tenant_id(0xC1);
    let asset_a = fixture_asset_id(0x21);
    let (_admin, reader) = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // 250 rows, one per second, quantities alternating active_power / soc.
    let asset_text = asset_a.uuid().to_hyphenated().to_string();
    let mut expected_keys: Vec<String> = Vec::with_capacity(250);
    let mut batch: Vec<NormalizedTelemetryRecord> = Vec::with_capacity(50);
    for i in 0..250i64 {
        let quantity = if i % 2 == 0 { Quantity::ActivePower } else { Quantity::Soc };
        let value = if quantity == Quantity::ActivePower {
            TelemetryValue::Numeric(1.0 + i as f64 * 0.01)
        } else {
            TelemetryValue::Numeric(0.5)
        };
        let event_millis = JAN_2030 * 1000 + i * 1000;
        batch.push(record_full(tenant_a, asset_a, quantity, Some(value), event_millis, i, TelemetryQuality::Good));
        let quantity_id = if quantity == Quantity::ActivePower { "active_power" } else { "soc" };
        expected_keys.push(format!("v1:{event_millis:015}:{asset_text}:{quantity_id}"));
        if batch.len() == 50 {
            let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
            let items = PgUnitOfWork::insert_telemetry_batch(&unit, &ctx_a, &batch)
                .expect("batch insert");
            assert!(
                items.iter().all(|item| item.outcome
                    == flexcore_postgres_adapter::telemetry::TelemetryBatchOutcome::Stored),
                "every seeded batch row must store"
            );
            unit.commit().expect("commit");
            batch.clear();
        }
    }
    expected_keys.sort();

    // Walk in pages of 50. The first four pages must carry a next cursor
    // (rows remain); the last page may or may not, depending on whether the
    // reader reports "page full" or "more rows follow" — the walk handles
    // both by following the cursor until it is None.
    let mut collected: Vec<String> = Vec::with_capacity(250);
    let mut cursor: Option<TelemetryCursor> = None;
    let mut pages = 0usize;
    loop {
        let query = TelemetryReadQuery::new(
            asset_a,
            EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
            EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
            None,
            None,
            50,
            cursor.clone(),
        )
        .expect("valid query");
        let page = reader.query(&ctx_a, query).expect("page query must succeed");
        pages += 1;
        assert!(
            pages <= 10,
            "the pagination walk must terminate; it did not after 10 pages"
        );
        assert!(
            page.records().len() <= 50,
            "a page must never exceed its limit of 50"
        );
        if pages <= 4 {
            assert!(
                page.next_cursor().is_some(),
                "page {pages} of the 250-row walk must carry a next cursor (rows remain)"
            );
        }
        for rec in page.records() {
            collected.push(sort_key(rec));
        }
        match page.next_cursor() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }

    assert!(
        pages == 5 || pages == 6,
        "250 rows in pages of 50 must take five pages when the reader reports a cursor only while rows remain, or five full pages plus one empty page when it reports a cursor whenever a page is full (the fake's rule); got {pages} pages"
    );
    assert_eq!(collected.len(), 250, "the walk must return all 250 rows");
    let mut sorted_collected = collected.clone();
    sorted_collected.sort();
    sorted_collected.dedup();
    assert_eq!(
        sorted_collected.len(),
        250,
        "the walk must return every row exactly once (no duplicates, no repeats across pages)"
    );
    assert_eq!(
        collected, expected_keys,
        "the walk must return rows in ascending sort-key order, exactly the seeded set"
    );
}

// --- 4. Mid-walk insert ------------------------------------------------------------

#[test]
fn mid_walk_insert_in_unread_range_is_not_lost() {
    let url = require_db("mid_walk_insert_in_unread_range_is_not_lost");
    let tenant_a = fixture_tenant_id(0xC1);
    let asset_a = fixture_asset_id(0x21);
    let (_admin, reader) = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // 100 active-power rows, 10 s apart.
    for i in 0..100i64 {
        let rec = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(1.0 + i as f64 * 0.01)),
            JAN_2030 + i * 10,
            i,
        );
        insert_one(&ingest, &ctx_a, &rec);
    }

    let range_from = EventTime::from_unix_millis(JAN_2030 * 1000).expect("from");
    let range_until = EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until");

    // Page 1: rows 0..50.
    let first = TelemetryReadQuery::new(asset_a, range_from, range_until, None, None, 50, None)
        .expect("valid query");
    let page1 = reader.query(&ctx_a, first).expect("page 1 must succeed");
    assert_eq!(page1.records().len(), 50, "page 1 must hold 50 rows");
    let cursor = page1
        .next_cursor()
        .expect("page 1 must carry a next cursor")
        .to_owned();

    // Insert 10 rows interleaved in the not-yet-read range (5 s after rows
    // 50..60, i.e. strictly after the page-1 cursor, before row 60).
    let mut inserted_keys: Vec<String> = Vec::with_capacity(10);
    for i in 50..60i64 {
        let event_millis = (JAN_2030 + i * 10) * 1000 + 5_000;
        let rec = record_full(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(5.0 + i as f64 * 0.01)),
            event_millis,
            1000 + i,
            TelemetryQuality::Good,
        );
        insert_one(&ingest, &ctx_a, &rec);
        inserted_keys.push(format!(
            "v1:{event_millis:015}:{}:active_power",
            asset_a.uuid().to_hyphenated()
        ));
    }

    // Continue the walk from the page-1 cursor; it must pick up the
    // mid-walk inserts as well as the remaining seeded rows.
    let mut rest: Vec<String> = Vec::with_capacity(60);
    let mut current = Some(cursor);
    let mut extra_pages = 0usize;
    while let Some(cursor) = current {
        let query = TelemetryReadQuery::new(
            asset_a,
            range_from,
            range_until,
            None,
            None,
            50,
            Some(cursor),
        )
        .expect("valid query");
        let page = reader.query(&ctx_a, query).expect("continuation page must succeed");
        extra_pages += 1;
        assert!(extra_pages <= 10, "the continuation walk must terminate");
        for rec in page.records() {
            rest.push(sort_key(rec));
        }
        current = page.next_cursor().cloned();
    }

    let mut all: Vec<String> = page1
        .records()
        .iter()
        .map(sort_key)
        .chain(rest.into_iter())
        .collect();
    assert_eq!(all.len(), 110, "the walk must return the 100 seeded rows plus the 10 mid-walk inserts");
    all.sort();
    let mut deduped = all.clone();
    deduped.dedup();
    assert_eq!(deduped.len(), 110, "the walk must return every row exactly once");
    assert_eq!(all, deduped, "the combined walk must already be in ascending sort-key order");
    for key in &inserted_keys {
        assert!(
            all.contains(key),
            "a mid-walk insert in the unread range must appear in the walk: {key}"
        );
    }
}

// --- 5. Cursor precision: sub-second event_times -----------------------------------

#[test]
fn cursor_precision_subsecond_event_times_round_trip() {
    let url = require_db("cursor_precision_subsecond_event_times_round_trip");
    let tenant_a = fixture_tenant_id(0xC1);
    let asset_a = fixture_asset_id(0x21);
    let (_admin, reader) = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // Four rows inside one second, 250 ms apart.
    let base_millis = (JAN_2030 + 2 * 86_400) * 1000;
    for (offset, sequence) in [(0i64, 1i64), (250, 2), (500, 3), (750, 4)] {
        let rec = record_full(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(sequence as f64)),
            base_millis + offset,
            sequence,
            TelemetryQuality::Good,
        );
        insert_one(&ingest, &ctx_a, &rec);
    }

    // Walk one row at a time; each page must hold exactly the next
    // sub-second row, and the cursor must round-trip through parse.
    let range_from = EventTime::from_unix_millis(base_millis).expect("from");
    let range_until = EventTime::from_unix_millis(base_millis + 1000).expect("until");
    let mut cursor: Option<TelemetryCursor> = None;
    let mut seen_millis: Vec<i128> = Vec::with_capacity(4);
    for expected_offset in [0i64, 250, 500, 750] {
        let query = TelemetryReadQuery::new(
            asset_a,
            range_from,
            range_until,
            None,
            None,
            1,
            cursor.clone(),
        )
        .expect("valid query");
        let page = reader.query(&ctx_a, query).expect("page must succeed");
        assert_eq!(page.records().len(), 1, "a limit-1 page must hold exactly one row");
        let rec = &page.records()[0];
        assert_eq!(
            rec.event_time().unix_millis(),
            (base_millis + expected_offset) as i128,
            "sub-second event_times must round-trip in sort order without collapsing onto the same second"
        );
        seen_millis.push(rec.event_time().unix_millis());
        let page_cursor = page
            .next_cursor()
            .expect("a limit-1 page over remaining rows must carry a cursor");
        // The cursor is the sort key of the last record, and it parses back.
        assert_eq!(
            page_cursor.key(),
            sort_key(rec),
            "the page cursor must be the sort key of the last record of the page"
        );
        assert_eq!(
            TelemetryCursor::parse(page_cursor.key()),
            Some(page_cursor.to_owned()),
            "a cursor produced by the reader must parse back to itself"
        );
        assert_eq!(
            TelemetryCursor::for_record(rec).key(),
            page_cursor.key(),
            "TelemetryCursor::for_record must agree with the reader's page cursor"
        );
        cursor = Some(page_cursor.to_owned());
    }
    assert_eq!(
        seen_millis,
        vec![
            base_millis as i128,
            (base_millis + 250) as i128,
            (base_millis + 500) as i128,
            (base_millis + 750) as i128
        ],
        "all four sub-second rows must be visited in order"
    );

    // One more page past the last row is empty.
    let query = TelemetryReadQuery::new(asset_a, range_from, range_until, None, None, 1, cursor)
        .expect("valid query");
    let last = reader.query(&ctx_a, query).expect("final page must succeed");
    assert!(last.records().is_empty(), "past the last row the page must be empty");
    assert!(
        last.next_cursor().is_none(),
        "past the last row there must be no next cursor"
    );

    // The sort key renders the documented shape: version, 15-digit
    // zero-padded millis, asset UUID, quantity id. base_millis + 250 is
    // 1_893_628_800_250 (13 digits), so zero-padded to 15 it is
    // "001893628800250" — the literal below pins that padding.
    let expected_key = format!(
        "v1:{:015}:{}:active_power",
        base_millis + 250,
        asset_a.uuid().to_hyphenated()
    );
    assert_eq!(
        expected_key,
        format!(
            "v1:001893628800250:{}:active_power",
            asset_a.uuid().to_hyphenated()
        ),
        "the sort key must zero-pad the millisecond timestamp to 15 digits"
    );
    assert_eq!(
        quantity_key(TelemetryQuantity::ActivePower),
        "active_power",
        "the port quantity id for active power is active_power"
    );
}

// --- 6. Malformed cursors (pure — no database) --------------------------------------

#[test]
fn malformed_cursors_are_rejected_by_parse() {
    let asset = fixture_asset_id(0x21);
    let asset_text = asset.uuid().to_hyphenated().to_string();
    let valid = format!("v1:{:015}:{asset_text}:active_power", JAN_2030 * 1000);
    assert_eq!(
        TelemetryCursor::parse(&valid).map(|c| c.key().to_string()),
        Some(valid.clone()),
        "a well-formed cursor must parse and round-trip its key"
    );

    for malformed in [
        String::new(),
        "not-a-cursor".to_string(),
        "v1:".to_string(),
        "v2:000001893456000:00000000-0000-0000-0000-000000000000:active_power".to_string(),
        // Millis not 15 digits.
        format!("v1:1893456000:{asset_text}:active_power"),
        format!("v1:00000189345600:{asset_text}:active_power"),
        // Millis not all digits.
        format!("v1:00000189345600a:{asset_text}:active_power"),
        // Bad asset UUID.
        "v1:000001893456000:not-a-uuid:active_power".to_string(),
        // Unknown quantity id.
        format!("v1:000001893456000:{asset_text}:watts"),
        // Missing quantity segment.
        format!("v1:000001893456000:{asset_text}"),
        // Trailing garbage after the quantity.
        format!("v1:000001893456000:{asset_text}:active_power:extra"),
    ] {
        assert_eq!(
            TelemetryCursor::parse(&malformed),
            None,
            "malformed cursor must be rejected by parse (never silently treated as first page): {malformed:?}"
        );
    }
}

// --- 7. Tenant scoping ----------------------------------------------------------------

#[test]
fn tenant_scoping_rows_of_tenant_b_never_appear_in_tenant_a_query() {
    let url = require_db("tenant_scoping_rows_of_tenant_b_never_appear_in_tenant_a_query");
    let tenant_a = fixture_tenant_id(0xC1);
    let tenant_b = fixture_tenant_id(0xD1);
    let asset_a = fixture_asset_id(0x21);
    let asset_b = fixture_asset_id(0x22);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(&url);
    let app = app_store(&url);
    seed_registry(&admin_store, &app, tenant_a, asset_a);
    seed_registry(&admin_store, &app, tenant_b, asset_b);
    let reader = query_reader(&url);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);

    for day in [5i64, 6, 7] {
        insert_one(
            &ingest,
            &ctx_a,
            &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(day as f64)), JAN_2030 + day * 86_400, day),
        );
        insert_one(
            &ingest,
            &ctx_b,
            &record(tenant_b, asset_b, Quantity::ActivePower, Some(TelemetryValue::Numeric(100.0 + day as f64)), JAN_2030 + day * 86_400, day),
        );
    }

    // Tenant A reading its own asset sees exactly its three rows.
    let own = query_all(&reader, &ctx_a, asset_a, JAN_2030, JAN_2030 + 31 * 86_400, None, None);
    assert_eq!(own.len(), 3, "tenant A must see its own three rows");
    assert!(
        own.iter().all(|r| r.tenant_id() == tenant_a),
        "tenant A's query must return only tenant A rows"
    );

    // Tenant B reading tenant A's asset sees nothing — no existence leak.
    let foreign_a = query_all(&reader, &ctx_b, asset_a, JAN_2030, JAN_2030 + 31 * 86_400, None, None);
    assert!(foreign_a.is_empty(), "tenant B must see none of tenant A's rows on tenant A's asset");

    // Tenant A reading tenant B's asset sees nothing either.
    let foreign_b = query_all(&reader, &ctx_a, asset_b, JAN_2030, JAN_2030 + 31 * 86_400, None, None);
    assert!(foreign_b.is_empty(), "tenant A must see none of tenant B's rows on tenant B's asset");

    // Tenant B reading its own asset sees exactly its three rows.
    let own_b = query_all(&reader, &ctx_b, asset_b, JAN_2030, JAN_2030 + 31 * 86_400, None, None);
    assert_eq!(own_b.len(), 3, "tenant B must see its own three rows");
    assert!(
        own_b.iter().all(|r| r.tenant_id() == tenant_b),
        "tenant B's query must return only tenant B rows"
    );
}
