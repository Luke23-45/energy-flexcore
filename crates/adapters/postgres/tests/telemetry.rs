// P2.06 — Integration tests for the telemetry storage: migrations (covered
// by `integration.rs`, extended with the Phase 2 tables/roles), partition
// routing, partition pruning (`EXPLAIN`), RLS isolation for the distinct
// ingest/query roles (P2.05 D-P6), duplicate-PK classification
// (exact vs conflicting, P2.09), the out-of-range event_time error path
// (P2.05 §7 item 2), the latest-value-per-asset query, and the quarantine
// write/list path (P2.07 §3).
//
// Gating is the recorded harness decision (CONTEXT.md open question 5):
// every test skips cleanly when FLEXCORE_TEST_DATABASE_URL is unset.
//
// Partition note (honesty, P2.05 §4.2): these tests prove the partitioning
// MECHANICS on real PostgreSQL 18 — routing, pruning, the out-of-range
// error path. They are correctness tests of the designed shape, not
// evidence that partitioning was a measured Phase 2 need.
//
// Test months are fixed constants in 2030/2031 (never `now()`-derived), so
// repeated runs against a persistent database do not collide with each
// other or with the pre-creation the P2.20 job will own later. Partitions
// are created with `IF NOT EXISTS` by the migrator role (the parent table's
// owner — the migration user lost ownership to `flexcore_migrator` in
// migration 0010).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_postgres_adapter::telemetry::{
    NormalizedTelemetryRecord, Quantity, StoredTelemetry, TelemetryError,
    TelemetryInsertOutcome, TelemetryValue,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
use sqlx::PgPool;

// --- Fixed test months (2030/2031, UTC) ------------------------------------------

/// 2030-01-01T00:00:00Z and friends (Unix seconds).
const JAN_2030: i64 = 1_893_456_000;
const FEB_2030: i64 = 1_896_134_400;
const MAR_2030: i64 = 1_898_553_600;
const APR_2030: i64 = 1_901_232_000;
/// 2031-06-01T00:00:00Z — a month with NO partition in the tests: the
/// out-of-range event_time path.
const JUN_2031: i64 = 1_938_038_400;

const PARTITION_MONTHS: [(&str, &str, &str); 4] = [
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
    ("telemetry_y2030m03", "2030-03-01 00:00:00+00", "2030-04-01 00:00:00+00"),
    ("telemetry_y2030m04", "2030-04-01 00:00:00+00", "2030-05-01 00:00:00+00"),
];

// --- Harness helpers (same recorded decisions as integration.rs) ------------------

fn test_url() -> Option<String> {
    std::env::var("FLEXCORE_TEST_DATABASE_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn require_db(test: &str) -> String {
    match test_url() {
        Some(url) => url,
        None => {
            println!(
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a PostgreSQL 18 service)"
            );
            String::new()
        }
    }
}

fn skipped(url: &str) -> bool {
    url.is_empty()
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
    bytes[9] = run_key() as u8;
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
            subject: "integration-test:telemetry".to_string(),
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

/// Grants LOGIN and test-only passwords to ALL service roles (the migration
/// DO blocks create them NOLOGIN — credentials never live in a migration;
/// the recorded harness deviation pattern from integration.rs, extended for
/// the P2.06 telemetry roles).
fn ensure_test_roles(pool: &PgPool) {
    let result = test_runtime().block_on(async {
        for (role, password) in [
            ("flexcore_app", "flexcore_app_test_only"),
            ("flexcore_migrator", "flexcore_migrator_test_only"),
            ("flexcore_ingest", "flexcore_ingest_test_only"),
            ("flexcore_query", "flexcore_query_test_only"),
        ] {
            sqlx::query(&format!("ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"))
                .execute(pool)
                .await?;
        }
        Ok::<(), sqlx::Error>(())
    });
    result.expect("test roles must be grantable from the migration/admin connection");
}

/// Creates the fixed 2030 monthly partitions as the migrator role (the
/// parent table's owner). Idempotent across runs.
fn ensure_partitions(url: &str) -> PgPool {
    let migrator_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("connect as flexcore_migrator");
    test_runtime().block_on(async {
        for (name, from, to) in PARTITION_MONTHS {
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
                 FOR VALUES FROM ('{from}') TO ('{to}')"
            ))
            .execute(&migrator_pool)
            .await
            .expect("partition DDL as the table owner");
        }
    });
    migrator_pool
}

/// Seeds one tenant with one portfolio/site/asset chain. Tenants (no RLS)
/// go through the admin connection; the tenant-owned registry rows go
/// through the runtime role with the tenant context (the same pattern as
/// the P1.13 RLS tests — the admin user is not the owner after migration
/// and is subject to RLS if not a superuser).
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

/// A record fixture for one (asset, quantity, event_time, sequence) with a
/// value or (for MISSING) none. `ingest_time` is `event_time + 1s` — the
/// P0.09 ordering rule allows it and the fixed future months need it.
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

/// `count(*)` over the whole telemetry table as the connected role under an
/// explicit tenant context (RLS scoping).
fn count_telemetry_as(pool: &PgPool, tenant_text: &str) -> i64 {
    test_runtime().block_on(async {
        let mut tx = pool.begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant_text)
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

/// Rows of one child partition, counted directly on the partition table
/// (the partition itself carries no RLS policy — it was created by the
/// owner after the fact — so this is the routing proof).
fn count_in_partition(pool: &PgPool, partition: &str, asset_text: &str) -> i64 {
    test_runtime().block_on(async {
        sqlx::query_scalar::<_, i64>(&format!(
            "SELECT count(*) FROM {partition} WHERE asset_id = $1::uuid"
        ))
        .bind(asset_text)
        .fetch_one(pool)
        .await
        .expect("partition count query")
    })
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

// --- 1. Partition routing ----------------------------------------------------------

#[test]
fn telemetry_rows_route_to_the_correct_monthly_partition() {
    let url = require_db("telemetry_rows_route_to_the_correct_monthly_partition");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA1);
    let asset_a = fixture_asset_id(0x0A);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    // One row in January, one in February, through the ingest role.
    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let jan = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(1.5)),
            JAN_2030 + 4 * 86_400,
            1,
        );
        let feb = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(2.5)),
            FEB_2030 + 9 * 86_400,
            2,
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &jan).expect("jan insert"),
            TelemetryInsertOutcome::Stored
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &feb).expect("feb insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit");
    }

    // Routing proof: each row is in its own month's partition child table.
    let asset_text = asset_a.uuid().to_hyphenated();
    assert_eq!(count_in_partition(&migrator_pool, "telemetry_y2030m01", &asset_text), 1);
    assert_eq!(count_in_partition(&migrator_pool, "telemetry_y2030m02", &asset_text), 1);
    // And nothing leaked into the sibling months.
    assert_eq!(count_in_partition(&migrator_pool, "telemetry_y2030m03", &asset_text), 0);
    assert_eq!(count_in_partition(&migrator_pool, "telemetry_y2030m04", &asset_text), 0);
}

// --- 2. Partition pruning (EXPLAIN) ------------------------------------------------

#[test]
fn time_bounded_query_explains_with_partition_pruning() {
    let url = require_db("time_bounded_query_explains_with_partition_pruning");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA2);
    let asset_a = fixture_asset_id(0x0B);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let march = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(3.0)),
            MAR_2030 + 4 * 86_400,
            3,
        );
        let april = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(4.0)),
            APR_2030 + 4 * 86_400,
            4,
        );
        PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &march).expect("march insert");
        PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &april).expect("april insert");
        unit.commit().expect("commit");
    }

    // A time-bounded query over March only. The query role (not the owner)
    // runs EXPLAIN with literal timestamps — parameterized bounds cannot be
    // pruned at plan time, and the test must show pruning. Literal values
    // are fixed constants, never caller text.
    let pool = query_pool(&url);
    let tenant_text = tenant_a.uuid().to_hyphenated();
    let asset_text = asset_a.uuid().to_hyphenated();
    let explain_sql = format!(
        "EXPLAIN SELECT telemetry_id FROM telemetry \
         WHERE tenant_id = '{tenant_text}'::uuid \
           AND asset_id = '{asset_text}'::uuid \
           AND event_time >= '2030-03-01 00:00:00+00' \
           AND event_time <= '2030-03-31 23:59:59+00'"
    );
    let plan = test_runtime().block_on(async {
        let mut tx = pool.begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(&tenant_text)
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let rows: Vec<String> = sqlx::query_scalar(&explain_sql)
            .fetch_all(&mut *tx)
            .await
            .expect("EXPLAIN query");
        tx.commit().await.expect("commit");
        rows.join("\n")
    });

    // The March partition is scanned; the April partition is pruned away.
    assert!(
        plan.contains("telemetry_y2030m03"),
        "the plan must scan the March partition; got:\n{plan}"
    );
    assert!(
        !plan.contains("telemetry_y2030m04"),
        "the plan must prune the April partition for a March-bounded query; got:\n{plan}"
    );
    assert!(
        !plan.contains("telemetry_y2030m01") && !plan.contains("telemetry_y2030m02"),
        "the plan must prune the January/February partitions; got:\n{plan}"
    );
}

// --- 3. RLS isolation for the distinct ingest/query roles (D-P6) --------------------

#[test]
fn rls_isolates_ingest_and_query_roles_per_tenant() {
    let url = require_db("rls_isolates_ingest_and_query_roles_per_tenant");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA3);
    let tenant_b = fixture_tenant_id(0xB3);
    let asset_a = fixture_asset_id(0x0C);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(
        &admin_store,
        &app_store,
        tenant_b,
        fixture_asset_id(0x0D),
    );

    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);
    let jan = record(
        tenant_a,
        asset_a,
        Quantity::ActivePower,
        Some(TelemetryValue::Numeric(1.25)),
        JAN_2030 + 86_400,
        10,
    );

    // Tenant A ingests through its context.
    let ingest = ingest_store(&url);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &jan).expect("insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit");
    }

    // Tenant A's query role sees the row; tenant B's query role sees
    // nothing (no existence leak).
    let pool_query = query_pool(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated();
    let tenant_b_text = tenant_b.uuid().to_hyphenated();
    assert_eq!(count_telemetry_as(&pool_query, &tenant_a_text), 1);
    assert_eq!(count_telemetry_as(&pool_query, &tenant_b_text), 0);

    // Tenant B's ingest role is equally blind to tenant A's rows (the
    // ingestion path is not an RLS bypass — the ledger risk note).
    let pool_ingest = test_runtime().block_on(PgPool::connect(&url_as(
        &url,
        "flexcore_ingest",
        "flexcore_ingest_test_only",
    )));
    let pool_ingest = pool_ingest.expect("ingest pool");
    assert_eq!(count_telemetry_as(&pool_ingest, &tenant_b_text), 0);

    // A record claiming tenant B's identity under tenant A's context is
    // refused at the repository boundary before any SQL runs.
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let foreign = record(
            tenant_b,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(9.0)),
            JAN_2030 + 2 * 86_400,
            11,
        );
        match PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &foreign) {
            Err(TelemetryError::TenantMismatch) => {}
            other => panic!("expected TenantMismatch, got {other:?}"),
        }
        unit.rollback().expect("rollback");
    }

    // The ingest role has no UPDATE/DELETE grant and the query role has no
    // INSERT grant — the privilege model under the RLS policy (D-P6).
    test_runtime().block_on(async {
        let mut tx = pool_ingest.begin().await.expect("tx");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(&tenant_a_text)
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let delete_result =
            sqlx::query("DELETE FROM telemetry WHERE asset_id = $1::uuid")
                .bind(asset_a.uuid().to_hyphenated())
                .execute(&mut *tx)
                .await;
        assert!(delete_result.is_err(), "flexcore_ingest must not DELETE");
        tx.rollback().await.expect("rollback");

        let mut tx = pool_query.begin().await.expect("tx");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(&tenant_a_text)
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let insert_result = sqlx::query(
            "INSERT INTO telemetry (tenant_id, asset_id, quantity, event_time, ingest_time, \
                 source_sequence, source_id, quality, unit, clock_quality) \
             VALUES ($1::uuid, $2::uuid, 'active_power', now(), now(), 1, 'sim://x', \
                     'GOOD', 'MW', 'SYNCHRONIZED')",
        )
        .bind(&tenant_a_text)
        .bind(asset_a.uuid().to_hyphenated())
        .execute(&mut *tx)
        .await;
        assert!(insert_result.is_err(), "flexcore_query must not INSERT");
        tx.rollback().await.expect("rollback");
    });
    let _ = migrator_pool; // partitions ensured above
}

// --- 4. Duplicate classification (exact vs conflicting, P2.09) ----------------------

#[test]
fn duplicate_primary_key_is_classified_exact_or_conflicting() {
    let url = require_db("duplicate_primary_key_is_classified_exact_or_conflicting");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA4);
    let asset_a = fixture_asset_id(0x0E);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    let original = record(
        tenant_a,
        asset_a,
        Quantity::ActivePower,
        Some(TelemetryValue::Numeric(1.5)),
        JAN_2030 + 86_400,
        20,
    );
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &original).expect("first insert"),
            TelemetryInsertOutcome::Stored
        );

        // Byte-identical resend: acknowledged, NOT stored twice (P2.09).
        let resend = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(1.5)),
            JAN_2030 + 86_400,
            20,
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &resend).expect("resend"),
            TelemetryInsertOutcome::ExactDuplicateAcknowledged
        );

        // Same identity, different value: conflicting duplicate — never
        // silently overwritten (P2.09).
        let conflicting = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(2.5)),
            JAN_2030 + 86_400,
            20,
        );
        match PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &conflicting) {
            Err(TelemetryError::ConflictingDuplicate) => {}
            other => panic!("expected ConflictingDuplicate, got {other:?}"),
        }

        // Same identity, different quantity: a NEW row of the same sample
        // (multi-quantity sample, D-T2) — stored, not a conflict.
        let sibling_quantity = record(
            tenant_a,
            asset_a,
            Quantity::Soc,
            Some(TelemetryValue::Numeric(0.75)),
            JAN_2030 + 86_400,
            20,
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &sibling_quantity)
                .expect("sibling quantity insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit");
    }

    // Exactly two rows exist (original + sibling quantity); nothing was
    // overwritten, and nothing landed in quarantine (quarantining a
    // conflicting duplicate is the CALLER's decision, P2.10). The count
    // runs through the query role under the tenant context — the owner
    // itself is subject to FORCE RLS (P2.05 §9) and sees nothing without
    // the GUC, which is exactly the fail-closed behaviour being asserted.
    let pool_query = query_pool(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated();
    let count = count_telemetry_as(&pool_query, &tenant_a_text);
    assert_eq!(count, 2, "no duplicate row, no overwrite");
    let quarantined = test_runtime().block_on(async {
        let mut tx = pool_query.begin().await.expect("tx");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(&tenant_a_text)
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM telemetry_quarantine")
            .fetch_one(&mut *tx)
            .await
            .expect("quarantine count");
        tx.commit().await.expect("commit");
        count
    });
    assert_eq!(quarantined, 0, "the repository does not quarantine by itself");
    let _ = migrator_pool;
}

// --- 5. Out-of-range event_time (P2.05 §7 item 2) ------------------------------------

#[test]
fn out_of_range_event_time_is_typed_and_does_not_poison_the_unit_of_work() {
    let url = require_db("out_of_range_event_time_is_typed_and_does_not_poison_the_unit_of_work");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    // Only the January partition exists for this scenario.
    let migrator_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("connect as flexcore_migrator");
    test_runtime().block_on(async {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS telemetry_y2030m01 PARTITION OF telemetry \
             FOR VALUES FROM ('2030-01-01 00:00:00+00') TO ('2030-02-01 00:00:00+00')",
        )
        .execute(&migrator_pool)
        .await
        .expect("partition DDL");
    });

    let tenant_a = fixture_tenant_id(0xA5);
    let asset_a = fixture_asset_id(0x1A);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        // June 2031 maps to no partition: the PostgreSQL routing error is
        // surfaced as the typed OutOfRangeEventTime (quarantine path, P2.05
        // §7 item 2).
        let unroutable = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(7.0)),
            JUN_2031,
            30,
        );
        match PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &unroutable) {
            Err(TelemetryError::OutOfRangeEventTime) => {}
            other => panic!("expected OutOfRangeEventTime, got {other:?}"),
        }
        // The savepoint rollback kept the transaction usable: a routable
        // record in the SAME unit of work still stores.
        let routable = record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(6.0)),
            JAN_2030 + 3 * 86_400,
            31,
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &routable).expect("routable insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit");
    }

    let pool_query = query_pool(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated();
    assert_eq!(count_telemetry_as(&pool_query, &tenant_a_text), 1);
}

// --- 6. Batch insert with per-record outcomes ----------------------------------------

#[test]
fn batch_insert_reports_per_record_duplicate_and_range_outcomes() {
    let url = require_db("batch_insert_reports_per_record_duplicate_and_range_outcomes");
    if skipped(&url) {
        return;
    }
    use flexcore_postgres_adapter::telemetry::{TelemetryBatchItem, TelemetryBatchOutcome};

    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA6);
    let asset_a = fixture_asset_id(0x1B);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let already_stored = record(
            tenant_a,
            asset_a,
            Quantity::ReactivePower,
            Some(TelemetryValue::Numeric(0.5)),
            JAN_2030 + 2 * 86_400,
            40,
        );
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &already_stored).expect("seed insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit");
    }

    let batch = vec![
        // New row.
        record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(1.0)),
            JAN_2030 + 2 * 86_400,
            41,
        ),
        // Exact duplicate of the seed insert above.
        record(
            tenant_a,
            asset_a,
            Quantity::ReactivePower,
            Some(TelemetryValue::Numeric(0.5)),
            JAN_2030 + 2 * 86_400,
            40,
        ),
        // Conflicting duplicate (same identity as the seed, other value).
        record(
            tenant_a,
            asset_a,
            Quantity::ReactivePower,
            Some(TelemetryValue::Numeric(0.75)),
            JAN_2030 + 2 * 86_400,
            40,
        ),
        // No partition for June 2031.
        record(
            tenant_a,
            asset_a,
            Quantity::ActivePower,
            Some(TelemetryValue::Numeric(2.0)),
            JUN_2031,
            42,
        ),
        // Another new row.
        record(
            tenant_a,
            asset_a,
            Quantity::Soc,
            Some(TelemetryValue::Numeric(0.5)),
            JAN_2030 + 2 * 86_400,
            43,
        ),
    ];
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let items = PgUnitOfWork::insert_telemetry_batch(&unit, &ctx_a, &batch)
            .expect("batch insert");
        let outcome = |index: usize| {
            items
                .iter()
                .find(|item: &&TelemetryBatchItem| item.index == index)
                .map(|item| item.outcome)
                .expect("outcome for every batch member")
        };
        assert_eq!(outcome(0), TelemetryBatchOutcome::Stored);
        assert_eq!(outcome(1), TelemetryBatchOutcome::ExactDuplicateAcknowledged);
        assert_eq!(outcome(2), TelemetryBatchOutcome::ConflictingDuplicate);
        assert_eq!(outcome(3), TelemetryBatchOutcome::OutOfRangeEventTime);
        assert_eq!(outcome(4), TelemetryBatchOutcome::Stored);
        unit.commit().expect("commit");
    }

    // Committed rows: the reactive seed, the new active_power, the new soc
    // — the conflicting duplicate and the unroutable record are absent.
    let pool_query = query_pool(&url);
    let tenant_a_text = tenant_a.uuid().to_hyphenated();
    assert_eq!(count_telemetry_as(&pool_query, &tenant_a_text), 3);
    let _ = migrator_pool;
}

// --- 7. Query by asset + time range; latest value per asset --------------------------

#[test]
fn time_range_query_and_latest_per_asset_are_correct_and_tenant_scoped() {
    let url = require_db("time_range_query_and_latest_per_asset_are_correct_and_tenant_scoped");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA7);
    let asset_a = fixture_asset_id(0x1C);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        // Three active_power samples in January (ascending), one reactive,
        // one soc.
        for (day, value) in [(5, 1.0), (6, 2.0), (7, 3.0)] {
            let sample = record(
                tenant_a,
                asset_a,
                Quantity::ActivePower,
                Some(TelemetryValue::Numeric(value)),
                JAN_2030 + day * 86_400,
                day as i64,
            );
            PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &sample).expect("sample insert");
        }
        let reactive = record(
            tenant_a,
            asset_a,
            Quantity::ReactivePower,
            Some(TelemetryValue::Numeric(0.4)),
            JAN_2030 + 5 * 86_400,
            500,
        );
        let soc = record(
            tenant_a,
            asset_a,
            Quantity::Soc,
            Some(TelemetryValue::Numeric(0.8)),
            JAN_2030 + 6 * 86_400,
            600,
        );
        PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &reactive).expect("reactive insert");
        PgUnitOfWork::insert_telemetry(&unit, &ctx_a, &soc).expect("soc insert");
        unit.commit().expect("commit");
    }

    // Full January range, all quantities: five rows, newest first.
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        let rows = PgUnitOfWork::query_telemetry(
            &unit,
            &ctx_a,
            &[asset_a],
            EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
            EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
            None,
        )
        .expect("range query");
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].quantity, Quantity::Soc);
        assert_eq!(rows[0].source_sequence, 600);
        // A quantity filter narrows to that quantity only.
        let reactive_rows = PgUnitOfWork::query_telemetry(
            &unit,
            &ctx_a,
            &[asset_a],
            EventTime::from_unix_millis(JAN_2030 * 1000).expect("from"),
            EventTime::from_unix_millis((JAN_2030 + 31 * 86_400) * 1000).expect("until"),
            Some(&[Quantity::ReactivePower]),
        )
        .expect("filtered query");
        assert_eq!(reactive_rows.len(), 1);
        assert_eq!(reactive_rows[0].quantity, Quantity::ReactivePower);
        assert_eq!(
            reactive_rows[0].q_sign_convention,
            Some(ReactiveSignConvention::InjectionPositive)
        );
        // The reactive convention and value round-trip (D-T4: the declared
        // convention travels on the row).
        match &reactive_rows[0].value {
            Some(TelemetryValue::Numeric(v)) => assert_eq!(*v, 0.4),
            other => panic!("expected a numeric value, got {other:?}"),
        }

        // Latest value per (asset, quantity): three rows — the newest
        // active_power (Jan 7), the reactive (Jan 5), the soc (Jan 6). The
        // read rule is latest event_time, never latest arrival (P2.07 §4).
        let latest = PgUnitOfWork::latest_telemetry_per_asset(&unit, &ctx_a, &[asset_a], None)
            .expect("latest query");
        assert_eq!(latest.len(), 3);
        let by_quantity = |quantity: Quantity| -> &StoredTelemetry {
            latest
                .iter()
                .find(|row| row.quantity == quantity)
                .unwrap_or_else(|| panic!("latest row for {quantity:?}"))
        };
        let active = by_quantity(Quantity::ActivePower);
        assert_eq!(
            active.event_time.timestamp().unix_timestamp(),
            JAN_2030 + 7 * 86_400
        );
        match &active.value {
            Some(TelemetryValue::Numeric(v)) => assert_eq!(*v, 3.0),
            other => panic!("expected the newest numeric value, got {other:?}"),
        }
        assert_eq!(
            by_quantity(Quantity::Soc).event_time.timestamp().unix_timestamp(),
            JAN_2030 + 6 * 86_400
        );
        assert_eq!(
            by_quantity(Quantity::ReactivePower)
                .event_time
                .timestamp()
                .unix_timestamp(),
            JAN_2030 + 5 * 86_400
        );

        // An inverted range is refused (the P2.11 TelemetryQuery contract
        // re-checked at the repository boundary).
        match PgUnitOfWork::query_telemetry(
            &unit,
            &ctx_a,
            &[asset_a],
            EventTime::from_unix_millis((JAN_2030 + 86_400) * 1000).expect("from"),
            EventTime::from_unix_millis(JAN_2030 * 1000).expect("until"),
            None,
        ) {
            Err(TelemetryError::Backend(_)) => {}
            other => panic!("expected an inverted-range rejection, got {other:?}"),
        }
        unit.commit().expect("commit");
    }
    let _ = migrator_pool;
}

// --- 8. Quarantine write/list (P2.07 §3) ----------------------------------------------

#[test]
fn quarantine_insert_and_list_are_tenant_scoped() {
    let url = require_db("quarantine_insert_and_list_are_tenant_scoped");
    if skipped(&url) {
        return;
    }
    use flexcore_postgres_adapter::telemetry::TelemetryQuarantineEntry;

    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA8);
    let tenant_b = fixture_tenant_id(0xB8);
    let asset_a = fixture_asset_id(0x1D);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x1E));

    let ingest = ingest_store(&url);
    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);

    let entry = TelemetryQuarantineEntry {
        tenant_id: tenant_a,
        asset_id: Some(asset_a),
        event_time: Some(EventTime::from_unix_millis((JAN_2030 + 86_400) * 1000).expect("time")),
        source_id: Some("sim://exit-gate".to_string()),
        source_sequence: Some(77),
        stage: "duplicate-out-of-order".to_string(),
        reason: "conflicting duplicate with different content (P2.09)".to_string(),
        raw_payload: serde_json::json!({
            "quantity": "active_power",
            "value": 2.5,
            "event_time": "2030-01-02T00:00:00Z"
        }),
        attempt_count: 0,
    };

    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        PgUnitOfWork::quarantine_telemetry(&unit, &ctx_a, &entry).expect("quarantine insert");
        let listed = PgUnitOfWork::list_telemetry_quarantine(&unit, &ctx_a, 100)
            .expect("quarantine list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].entry.stage, "duplicate-out-of-order");
        assert_eq!(listed[0].entry.source_sequence, Some(77));
        assert_eq!(listed[0].entry.asset_id, Some(asset_a));
        assert_eq!(
            listed[0].entry.raw_payload.get("quantity"),
            Some(&serde_json::json!("active_power"))
        );
        // A foreign-tenant entry is refused at the boundary.
        let foreign = TelemetryQuarantineEntry {
            tenant_id: tenant_b,
            ..entry.clone()
        };
        match PgUnitOfWork::quarantine_telemetry(&unit, &ctx_a, &foreign) {
            Err(TelemetryError::TenantMismatch) => {}
            other => panic!("expected TenantMismatch, got {other:?}"),
        }
        unit.commit().expect("commit");
    }

    // Tenant B sees nothing of tenant A's quarantine (RLS).
    {
        let unit = ingest.begin(Some(&ctx_b)).expect("ingest unit of work");
        let listed = PgUnitOfWork::list_telemetry_quarantine(&unit, &ctx_b, 100)
            .expect("quarantine list for tenant B");
        assert!(listed.is_empty(), "tenant B must not see tenant A's quarantine");
        unit.commit().expect("commit");
    }

    // An entry without stage or reason is refused (the no-silent-skip
    // evidence rule, P2.07 §2).
    let hollow = TelemetryQuarantineEntry {
        tenant_id: tenant_a,
        stage: "  ".to_string(),
        reason: "x".to_string(),
        ..entry.clone()
    };
    {
        let unit = ingest.begin(Some(&ctx_a)).expect("ingest unit of work");
        match PgUnitOfWork::quarantine_telemetry(&unit, &ctx_a, &hollow) {
            Err(TelemetryError::Backend(_)) => {}
            other => panic!("expected a stage/reason rejection, got {other:?}"),
        }
        unit.rollback().expect("rollback");
    }
}
