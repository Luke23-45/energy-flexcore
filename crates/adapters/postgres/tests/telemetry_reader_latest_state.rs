// Patch A — `TelemetryReader::latest_state` against real PostgreSQL.
//
// Per-quantity latest by `event_time` (ties: higher `source_sequence`),
// late-arrival advance / no-regress, absent quantities, cross-tenant.
//
// Gating (REQUIRED DEVIATION from the P2.06 harness in `telemetry.rs`):
// these tests FAIL LOUDLY when `FLEXCORE_TEST_DATABASE_URL` is unset or
// empty — a DB-gated suite must never silently pass.
//
// Fixture prefix byte 0xC2 (tenant A) / 0xD2 (tenant B): distinct from the
// other Patch A reader files so all five can run against one shared test
// database without colliding.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
use flexcore_ports::telemetry_read::TelemetryReader;
use flexcore_ports::telemetry_source::{NormalizedTelemetry, QuantityValue, TelemetryQuantity};
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
            subject: "integration-test:telemetry-reader-latest-state".to_string(),
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

// Batch 67 (same as Batch 66 fix D): a per-call runtime kills every pool
// created under it — `PoolOptions::connect` opens one connection eagerly
// against the creating runtime's I/O driver, and dropping that runtime
// makes the next query on the pooled connection fail with
// "a Tokio 1.x context was found, but it is being shutdown". One shared
// multi-thread runtime per binary, built once and never dropped, keeps
// every pool valid for the binary's life.
fn test_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    })
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

fn query_reader(url: &str) -> PgTelemetryReader {
    let store = FlexCoreStore::connect(&url_as(url, "flexcore_query", "flexcore_query_test_only"))
        .expect("query-role store");
    store.telemetry_reader()
}

fn app_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store")
}

fn setup(url: &str, tenant_id: TenantId, asset_id: AssetId) -> PgTelemetryReader {
    let admin_store = FlexCoreStore::connect(url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(url);
    seed_registry(&admin_store, &app_store(url), tenant_id, asset_id);
    query_reader(url)
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

fn latest_for(
    reader: &PgTelemetryReader,
    ctx: &TenantContext,
    asset: AssetId,
) -> Vec<NormalizedTelemetry> {
    reader
        .latest_state(ctx, asset)
        .expect("latest_state must succeed")
}

fn by_quantity<'a>(
    rows: &'a [NormalizedTelemetry],
    quantity: TelemetryQuantity,
) -> &'a NormalizedTelemetry {
    rows.iter()
        .find(|row| row.quantity() == quantity)
        .unwrap_or_else(|| panic!("latest state must contain a row for {quantity:?}"))
}

// --- 1. Per-quantity latest ----------------------------------------------------------

#[test]
fn latest_state_returns_one_row_per_quantity_at_latest_event_time() {
    let url = require_db("latest_state_returns_one_row_per_quantity_at_latest_event_time");
    let tenant_a = fixture_tenant_id(0xC2);
    let asset_a = fixture_asset_id(0x22);
    let reader = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // Active power Jan 5/6/7, SOC Jan 5/6, reactive power Jan 5.
    for (day, value, sequence) in [(5i64, 1.0, 1i64), (6, 2.0, 2), (7, 3.0, 3)] {
        insert_one(
            &ingest,
            &ctx_a,
            &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(value)), JAN_2030 + day * 86_400, sequence),
        );
    }
    for (day, value, sequence) in [(5i64, 0.6, 10i64), (6, 0.8, 11)] {
        insert_one(
            &ingest,
            &ctx_a,
            &record(tenant_a, asset_a, Quantity::Soc, Some(TelemetryValue::Numeric(value)), JAN_2030 + day * 86_400, sequence),
        );
    }
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ReactivePower, Some(TelemetryValue::Numeric(0.4)), JAN_2030 + 5 * 86_400, 20),
    );

    let latest = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(
        latest.len(),
        3,
        "latest state must hold exactly one row per quantity present (active_power, soc, reactive_power)"
    );

    let active = by_quantity(&latest, TelemetryQuantity::ActivePower);
    assert_eq!(
        active.event_time().unix_millis(),
        ((JAN_2030 + 7 * 86_400) as i128) * 1000,
        "active power latest must be the Jan 7 row (latest event_time)"
    );
    assert_eq!(
        active.value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(3.0).expect("finite"))),
        "active power latest must carry the Jan 7 value"
    );
    assert_eq!(active.source_sequence(), 3, "active power latest must be sequence 3");

    let soc = by_quantity(&latest, TelemetryQuantity::Soc);
    assert_eq!(
        soc.event_time().unix_millis(),
        ((JAN_2030 + 6 * 86_400) as i128) * 1000,
        "soc latest must be the Jan 6 row (latest event_time)"
    );
    assert_eq!(
        soc.value(),
        Some(&QuantityValue::Fraction(StateOfCharge::new(0.8).expect("fraction"))),
        "soc latest must carry the Jan 6 fraction"
    );

    let reactive = by_quantity(&latest, TelemetryQuantity::ReactivePower);
    assert_eq!(
        reactive.event_time().unix_millis(),
        ((JAN_2030 + 5 * 86_400) as i128) * 1000,
        "reactive power latest must be the Jan 5 row (its only row)"
    );
    assert_eq!(
        reactive.value(),
        Some(&QuantityValue::Reactive {
            power: ReactivePowerMVAr::new(0.4).expect("finite"),
            convention: ReactiveSignConvention::InjectionPositive,
        }),
        "reactive power latest must carry its value together with the declared sign convention"
    );
}

// --- 2. Tie-break: higher source_sequence --------------------------------------------

#[test]
fn latest_state_tie_breaks_on_higher_source_sequence() {
    let url = require_db("latest_state_tie_breaks_on_higher_source_sequence");
    let tenant_a = fixture_tenant_id(0xC2);
    let asset_a = fixture_asset_id(0x22);
    let reader = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // Same event_time (Jan 6), sequences 10 and 20: the higher sequence wins.
    // An older Jan 5 row with a much higher sequence (99) must NOT win —
    // the tie-break only applies at equal event_time.
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(9.0)), JAN_2030 + 5 * 86_400, 99),
    );
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(1.0)), JAN_2030 + 6 * 86_400, 10),
    );
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(2.0)), JAN_2030 + 6 * 86_400, 20),
    );

    let latest = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(latest.len(), 1, "one quantity present means one latest row");
    let active = &latest[0];
    assert_eq!(
        active.event_time().unix_millis(),
        ((JAN_2030 + 6 * 86_400) as i128) * 1000,
        "the tie-break winner must be at the latest event_time (Jan 6), not the Jan 5 row"
    );
    assert_eq!(
        active.source_sequence(),
        20,
        "at equal event_time the higher source_sequence wins"
    );
    assert_eq!(
        active.value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(2.0).expect("finite"))),
        "the tie-break winner must carry the sequence-20 value"
    );
}

// --- 3. Late arrival with a newer event_time advances the state -----------------------

#[test]
fn late_arrival_with_newer_event_time_advances_state() {
    let url = require_db("late_arrival_with_newer_event_time_advances_state");
    let tenant_a = fixture_tenant_id(0xC2);
    let asset_a = fixture_asset_id(0x22);
    let reader = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(1.0)), JAN_2030 + 5 * 86_400, 1),
    );
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(2.0)), JAN_2030 + 6 * 86_400, 2),
    );
    let before = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(before.len(), 1, "one quantity present before the late arrival");
    assert_eq!(
        before[0].event_time().unix_millis(),
        ((JAN_2030 + 6 * 86_400) as i128) * 1000,
        "state before the late arrival is the Jan 6 row"
    );

    // Arrives later (higher ingest order) with a NEWER event_time: state advances.
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(8.0)), JAN_2030 + 8 * 86_400, 3),
    );
    let after = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(after.len(), 1, "still one row per quantity after the late arrival");
    assert_eq!(
        after[0].event_time().unix_millis(),
        ((JAN_2030 + 8 * 86_400) as i128) * 1000,
        "a late arrival with a newer event_time must advance the state to Jan 8"
    );
    assert_eq!(
        after[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(8.0).expect("finite"))),
        "the advanced state must carry the Jan 8 value"
    );
}

// --- 4. Late arrival with an older event_time must not regress the state -------------

#[test]
fn late_arrival_with_older_event_time_does_not_regress_state() {
    let url = require_db("late_arrival_with_older_event_time_does_not_regress_state");
    let tenant_a = fixture_tenant_id(0xC2);
    let asset_a = fixture_asset_id(0x22);
    let reader = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(6.0)), JAN_2030 + 6 * 86_400, 6),
    );
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(7.0)), JAN_2030 + 7 * 86_400, 7),
    );
    let before = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(
        before[0].event_time().unix_millis(),
        ((JAN_2030 + 7 * 86_400) as i128) * 1000,
        "state before the late arrival is the Jan 7 row"
    );
    assert_eq!(
        before[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(7.0).expect("finite"))),
        "state before the late arrival carries the Jan 7 value"
    );

    // Arrives later but with an OLDER event_time (Jan 5): the state is
    // keyed on event_time, never on arrival — it must not regress.
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(5.0)), JAN_2030 + 5 * 86_400, 5),
    );
    let after = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(after.len(), 1, "still one row per quantity after the stale late arrival");
    assert_eq!(
        after[0].event_time().unix_millis(),
        ((JAN_2030 + 7 * 86_400) as i128) * 1000,
        "a late arrival with an older event_time must not move the state back from Jan 7"
    );
    assert_eq!(
        after[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(7.0).expect("finite"))),
        "the unmoved state must still carry the Jan 7 value"
    );
    assert_eq!(after[0].source_sequence(), 7, "the unmoved state keeps sequence 7");
}

// --- 5. Absent quantity: absent, never a zero value ----------------------------------

#[test]
fn absent_quantity_is_absent_never_a_zero_value() {
    let url = require_db("absent_quantity_is_absent_never_a_zero_value");
    let tenant_a = fixture_tenant_id(0xC2);
    let asset_a = fixture_asset_id(0x22);
    let reader = setup(&url, tenant_a, asset_a);
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);

    // Only active power is ever recorded (non-zero values).
    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(2.5)), JAN_2030 + 5 * 86_400, 1),
    );

    let latest = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(latest.len(), 1, "only the recorded quantity may appear in latest state");
    assert_eq!(latest[0].quantity(), TelemetryQuantity::ActivePower);
    assert!(
        !latest.iter().any(|r| r.quantity() == TelemetryQuantity::Soc),
        "a quantity with no record (soc) must be absent from latest state, never a zero value"
    );
    assert!(
        !latest.iter().any(|r| r.quantity() == TelemetryQuantity::Energy),
        "a quantity with no record (energy) must be absent from latest state, never a zero value"
    );
    assert!(
        !latest.iter().any(|r| r.quantity() == TelemetryQuantity::ReactivePower),
        "a quantity with no record (reactive_power) must be absent from latest state, never a zero value"
    );
    assert_eq!(
        latest[0].value(),
        Some(&QuantityValue::Power(ActivePowerMW::new(2.5).expect("finite"))),
        "the one present row must carry its real non-zero value"
    );
}

// --- 6. Cross-tenant -------------------------------------------------------------------

#[test]
fn cross_tenant_asset_returns_nothing_for_the_other_tenant() {
    let url = require_db("cross_tenant_asset_returns_nothing_for_the_other_tenant");
    let tenant_a = fixture_tenant_id(0xC2);
    let tenant_b = fixture_tenant_id(0xD2);
    let asset_a = fixture_asset_id(0x22);
    let asset_b = fixture_asset_id(0x23);
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

    insert_one(
        &ingest,
        &ctx_a,
        &record(tenant_a, asset_a, Quantity::ActivePower, Some(TelemetryValue::Numeric(1.5)), JAN_2030 + 5 * 86_400, 1),
    );
    insert_one(
        &ingest,
        &ctx_b,
        &record(tenant_b, asset_b, Quantity::ActivePower, Some(TelemetryValue::Numeric(2.5)), JAN_2030 + 6 * 86_400, 2),
    );

    // Tenant B asking for tenant A's asset state gets nothing.
    let foreign = latest_for(&reader, &ctx_b, asset_a);
    assert!(
        foreign.is_empty(),
        "tenant B must see no latest state for tenant A's asset"
    );

    // Tenant A asking for tenant B's asset state gets nothing either.
    let foreign_b = latest_for(&reader, &ctx_a, asset_b);
    assert!(
        foreign_b.is_empty(),
        "tenant A must see no latest state for tenant B's asset"
    );

    // Controls: each tenant sees its own asset's state.
    let own_a = latest_for(&reader, &ctx_a, asset_a);
    assert_eq!(own_a.len(), 1, "tenant A must see its own asset's latest state");
    assert_eq!(own_a[0].tenant_id(), tenant_a, "tenant A's state row belongs to tenant A");
    let own_b = latest_for(&reader, &ctx_b, asset_b);
    assert_eq!(own_b.len(), 1, "tenant B must see its own asset's latest state");
    assert_eq!(own_b[0].tenant_id(), tenant_b, "tenant B's state row belongs to tenant B");
}
