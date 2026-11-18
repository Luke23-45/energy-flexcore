// Patch A — `TelemetryReader::list_quarantine` against real PostgreSQL.
//
// Tenant scoping, newest-first order, and exact round-trip of the evidence
// fields (raw payload, failure reason, source, asset, event time, attempt
// count).
//
// Gating (REQUIRED DEVIATION from the P2.06 harness in `telemetry.rs`):
// these tests FAIL LOUDLY when `FLEXCORE_TEST_DATABASE_URL` is unset or
// empty — a DB-gated suite must never silently pass.
//
// Fixture prefix byte 0xC3 (tenant A) / 0xD3 (tenant B): distinct from the
// other Patch A reader files so all five can run against one shared test
// database without colliding.
//
// NOTE on `failure_reason`: the brief records that the reader's
// `failure_reason` combines the stored `stage` and `reason`, but does not
// fix the exact joining format. These tests therefore assert that the
// returned reason contains BOTH the stage and the reason verbatim, rather
// than asserting one guessed separator.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::clock::Clock;
use flexcore_ports::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
use flexcore_ports::telemetry_read::TelemetryReader;
use flexcore_postgres_adapter::telemetry::TelemetryQuarantineEntry;
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
            subject: "integration-test:telemetry-reader-quarantine".to_string(),
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

/// One runtime for the whole test binary, shared by every helper (the
/// recorded `test_runtime` pattern, fixed): a sqlx pool opened under a
/// runtime keeps that runtime's I/O driver for its connections, so a pool
/// created under a fresh per-call runtime dies with it — the next query
/// on it fails with "A Tokio 1.x context was found, but it is being
/// shutdown". A multi-thread runtime also allows concurrent `block_on`
/// from the harness's parallel test threads.
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

fn quarantine_entry(
    tenant_id: TenantId,
    asset_id: AssetId,
    day: i64,
    sequence: i64,
    stage: &str,
    reason: &str,
    attempt_count: i32,
) -> TelemetryQuarantineEntry {
    TelemetryQuarantineEntry {
        tenant_id,
        asset_id: Some(asset_id),
        event_time: Some(
            EventTime::from_unix_millis((JAN_2030 + day * 86_400) * 1000).expect("time"),
        ),
        source_id: Some(format!("sim://exit-gate-{sequence}")),
        source_sequence: Some(sequence),
        stage: stage.to_string(),
        reason: reason.to_string(),
        raw_payload: serde_json::json!({
            "quantity": "active_power",
            "value": 2.5,
            "event_time": "2030-01-02T00:00:00Z",
            "sequence": sequence,
        }),
        attempt_count,
    }
}

fn quarantine_one(
    ingest: &FlexCoreStore,
    ctx: &TenantContext,
    entry: &TelemetryQuarantineEntry,
) {
    let unit = ingest.begin(Some(ctx)).expect("ingest unit of work");
    PgUnitOfWork::quarantine_telemetry(&unit, ctx, entry).expect("quarantine insert");
    unit.commit().expect("commit");
}

// --- 1. Tenant scoping + newest-first order + field round-trip ---------------------

#[test]
fn list_quarantine_returns_only_the_callers_tenant_rows_newest_first() {
    let url = require_db("list_quarantine_returns_only_the_callers_tenant_rows_newest_first");
    let tenant_a = fixture_tenant_id(0xC3);
    let tenant_b = fixture_tenant_id(0xD3);
    let asset_a = fixture_asset_id(0x23);
    let asset_b = fixture_asset_id(0x24);
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

    // Three entries for tenant A, inserted oldest-first with a pause
    // between commits so `quarantined_at` (set by the database) is strictly
    // increasing and newest-first order is unambiguous.
    let entries_a = [
        quarantine_entry(tenant_a, asset_a, 2, 77, "duplicate-out-of-order", "conflicting duplicate with different content (P2.09)", 0),
        quarantine_entry(tenant_a, asset_a, 3, 78, "validation", "value outside the declared range", 1),
        quarantine_entry(tenant_a, asset_a, 4, 79, "routing", "no partition for the event month", 2),
    ];
    for entry in &entries_a {
        quarantine_one(&ingest, &ctx_a, entry);
        std::thread::sleep(Duration::from_millis(25));
    }
    // Two entries for tenant B.
    let entries_b = [
        quarantine_entry(tenant_b, asset_b, 2, 91, "validation", "missing required field quantity", 0),
        quarantine_entry(tenant_b, asset_b, 3, 92, "duplicate-out-of-order", "conflicting duplicate with different content (P2.09)", 3),
    ];
    for entry in &entries_b {
        quarantine_one(&ingest, &ctx_b, entry);
        std::thread::sleep(Duration::from_millis(25));
    }

    let listed_a = reader
        .list_quarantine(&ctx_a)
        .expect("list_quarantine must succeed for tenant A");
    assert_eq!(
        listed_a.len(),
        3,
        "tenant A must see exactly its own three quarantine entries"
    );
    assert!(
        listed_a.iter().all(|e| e.tenant_id() == tenant_a),
        "tenant A's quarantine list must contain only tenant A rows"
    );

    // Newest first: the last-inserted entry comes first, the first-inserted
    // last; quarantined_at is non-increasing down the list.
    let mut prev_quarantined_at: Option<i128> = None;
    for row in &listed_a {
        let at = row.quarantined_at().unix_millis();
        if let Some(prev) = prev_quarantined_at {
            assert!(
                at <= prev,
                "quarantine list must be newest-first (quarantined_at DESC): {at} after {prev}"
            );
        }
        prev_quarantined_at = Some(at);
    }

    // Exact round-trip, newest first: entries_a[2], entries_a[1], entries_a[0].
    for (row, seeded) in listed_a.iter().zip(entries_a.iter().rev()) {
        assert_eq!(row.tenant_id(), seeded.tenant_id, "tenant_id must round-trip");
        assert_eq!(row.asset_id(), seeded.asset_id, "asset_id must round-trip");
        assert_eq!(row.event_time(), seeded.event_time, "event_time must round-trip");
        assert_eq!(
            row.source_id(),
            seeded.source_id.as_deref().expect("seeded source_id"),
            "source_id must round-trip"
        );
        assert_eq!(
            row.attempt_count(),
            seeded.attempt_count as u32,
            "attempt_count must round-trip"
        );
        // The failure reason combines the stored stage and reason.
        assert!(
            row.failure_reason().contains(&seeded.stage),
            "failure_reason must carry the pipeline stage: got {:?}",
            row.failure_reason()
        );
        assert!(
            row.failure_reason().contains(&seeded.reason),
            "failure_reason must carry the rejection reason: got {:?}",
            row.failure_reason()
        );
        // The raw payload round-trips as JSON (compared parsed, so key
        // order and whitespace in the stored JSONB do not matter).
        let payload: serde_json::Value = serde_json::from_str(row.raw_payload())
            .expect("raw_payload must be valid JSON");
        assert_eq!(
            payload, seeded.raw_payload,
            "raw_payload must round-trip exactly as JSON"
        );
        assert_eq!(
            payload.get("sequence"),
            Some(&serde_json::json!(seeded.source_sequence.expect("seeded sequence"))),
            "raw_payload must keep the seeded sequence value"
        );
    }

    // Tenant B sees only its own two entries, also newest first.
    let listed_b = reader
        .list_quarantine(&ctx_b)
        .expect("list_quarantine must succeed for tenant B");
    assert_eq!(
        listed_b.len(),
        2,
        "tenant B must see exactly its own two quarantine entries"
    );
    assert!(
        listed_b.iter().all(|e| e.tenant_id() == tenant_b),
        "tenant B's quarantine list must contain only tenant B rows"
    );
    assert!(
        listed_b[0].failure_reason().contains("duplicate-out-of-order"),
        "tenant B's newest entry (inserted last) must come first"
    );
    assert_eq!(
        listed_b[1].attempt_count(),
        0,
        "tenant B's older entry keeps attempt_count 0"
    );
    assert!(
        listed_b[0].quarantined_at().unix_millis() >= listed_b[1].quarantined_at().unix_millis(),
        "tenant B's list must also be newest-first"
    );
}

// --- 2. Empty quarantine ------------------------------------------------------------

#[test]
fn list_quarantine_is_empty_when_nothing_was_quarantined() {
    let url = require_db("list_quarantine_is_empty_when_nothing_was_quarantined");
    let tenant_a = fixture_tenant_id(0xC3);
    let asset_a = fixture_asset_id(0x23);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let _migrator_pool = ensure_partitions(&url);
    seed_registry(&admin_store, &app_store(&url), tenant_a, asset_a);
    let reader = query_reader(&url);
    let ctx_a = context_for(tenant_a);

    let listed = reader
        .list_quarantine(&ctx_a)
        .expect("list_quarantine must succeed for a tenant with no entries");
    assert!(
        listed.is_empty(),
        "a tenant with no quarantine entries must get an empty list, not an error"
    );
}
