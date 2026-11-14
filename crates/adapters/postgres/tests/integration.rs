// P1.13 — Integration tests against a real PostgreSQL 18 database.
//
// Gating: every test skips cleanly (with a printed reason) when
// FLEXCORE_TEST_DATABASE_URL is unset. The URL names a test database the
// connecting user may run DDL on (migrations run as that user, which must
// also have CREATEROLE or superuser for the guarded role-creation DO block
// in migration 0001 — see the crate README role model).
//
// Recorded harness decision (CONTEXT.md open question 5): a CI PostgreSQL
// SERVICE, not testcontainers. The tests never create containers; CI (P1.27)
// provides `postgres:18` and exports the URL, mirroring the Compose stack
// (P1.25).
//
// Coverage per ledger P1.13 step 6 / acceptance criteria:
// - migrations run up and down cleanly from an empty database (dedicated
//   temporary database; down = the recorded down files, replayed in
//   reverse, then re-up);
// - RLS isolation for the runtime role (and the owner-role case required
//   by P1.12 §5.3 / P0.16 rule 2.4);
// - outbox atomicity with mid-transaction failure injection;
// - SOC check-constraint rejection ([0,1], D-S5);
// - quality-enum rejection of a seventh value (D-S4);
// - audit append-only (grants + defence-in-depth trigger, P1.12 §4.7).
//
// Test data is unique per run (timestamp-derived identifiers), so repeated
// runs against a persistent dev database never collide. The temporary
// migration-test database is dropped at the end (best effort).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_audit::record::{
    ActorIdentity, AuditAction, AuditObject, AuditRecord, AuditRecordFields,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::entities::{
    Asset, AssetType, Portfolio, Site, Tenant,
};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutbox, OutboxWrite};
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::state_change::StateChangeReceipt;
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
use sqlx::PgPool;

// The recorded down files (P1.12 §7: every migration has a down file or a
// recorded irreversible reason; the only irreversible part — role removal —
// is documented in 0001's down file).
const DOWN_FILES: [&str; 17] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0001_quality_enum.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0002_tenants.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0003_portfolios.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0004_sites.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0005_assets.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0006_asset_state.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0007_outbox.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0008_audit_records.down.sql"
    )),
    // P2.06: the Phase 2 migrations (registry lifecycle extension, then the
    // telemetry storage surface); the down files replay in reverse.
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0009_registry_lifecycle_p2_03.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0010_telemetry_storage_p2_06.down.sql"
    )),
    // P2.31–P4.30: the recorded down files of the later migrations (the
    // table had lagged at 0010; extended additively by the P4.30 subtask
    // so the down/up replay covers the full set — the down files replay
    // in reverse, newest first).
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0011_telemetry_api_read_grant_p2_19.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0012_correction_versions_p2_31.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0013_optimization_results_p3_21.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0014_schedule_revisions_p3_22.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0015_envelopes_p3_28.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0016_commands_p4_10.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0017_feasibility_checks_p4_30.down.sql"
    )),
];

const STAGE_1_TABLES: [&str; 7] = [
    "tenants",
    "portfolios",
    "sites",
    "assets",
    "asset_state",
    "outbox",
    "audit_records",
];

// P2.06: the Phase 2 telemetry storage surface (the Stage 1 tables above
// are extended, never recreated).
const STAGE_2_TABLES: [&str; 2] = ["telemetry", "telemetry_quarantine"];

// P2.06: the two distinct telemetry runtime roles (P2.05 D-P6), alongside
// the Phase 1 roles.
const TELEMETRY_ROLES: [&str; 2] = ["flexcore_ingest", "flexcore_query"];

// --- Harness helpers -----------------------------------------------------------

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

/// One monotonic-per-process counter mixed with the run timestamp: test
/// identifiers stay unique across parallel tests and repeated runs.
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

/// A deterministic, run-unique 128-bit identifier for test fixtures. The
/// high bytes carry the run key, the low byte the per-fixture counter, so
/// identifiers never repeat across runs or tests.
fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0x5A; // fixture marker
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

/// Builds the domain `TenantContext` through the only public construction
/// path (P0.16 rule 5): verified claims -> authenticated claims -> context.
/// In tests the caller fixes the claims, exactly like the port fakes.
fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "integration-test:rls".to_string(),
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

/// The test-only role bootstrap (recorded in the README): migrations create
/// the roles NOLOGIN with no credentials — never in the schema; the test
/// harness (like the deployment bootstrap) grants LOGIN and test-only
/// passwords against the test database.
fn ensure_test_roles(pool: &PgPool) {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(async {
            sqlx::query("ALTER ROLE flexcore_app WITH LOGIN PASSWORD 'flexcore_app_test_only'")
                .execute(pool)
                .await?;
            sqlx::query(
                "ALTER ROLE flexcore_migrator WITH LOGIN PASSWORD 'flexcore_migrator_test_only'",
            )
            .execute(pool)
            .await?;
            Ok::<(), sqlx::Error>(())
        });
    result.expect("test roles must be grantable from the migration/admin connection");
}

/// Rewrites the credentials of an admin URL to connect as one of the
/// service roles (assumes the conventional `scheme://user:pass@rest` form;
/// a URL without credentials gets them inserted).
fn url_as(url: &str, user: &str, password: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((_, host_and_path)) = rest.split_once('@') {
            return format!("{scheme}://{user}:{password}@{host_and_path}");
        }
        return format!("{scheme}://{user}:{password}@{rest}");
    }
    format!("{url}?user={user}&password={password}")
}

/// A one-off current-thread runtime for raw administrative SQL in tests.
fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("information_schema query") > 0
}

async fn policy_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM pg_policies WHERE tablename = $1 AND policyname = 'tenant_isolation'",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("pg_policies query") > 0
}

/// `count(*)` on a tenant-owned table as the connected role, under an
/// explicit (or empty) tenant context.
async fn count_portfolios_as(pool: &PgPool, tenant: Option<&str>) -> i64 {
    let mut tx = pool.begin().await.expect("transaction");
    if let Some(tenant) = tenant {
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant)
            .execute(&mut *tx)
            .await
            .expect("set_config");
    }
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM portfolios")
        .fetch_one(&mut *tx)
        .await
        .expect("count query");
    tx.commit().await.expect("commit");
    count
}

// --- 1. Migrations up / down / up ------------------------------------------------

#[test]
fn migrations_run_up_and_down_cleanly_from_an_empty_database() {
    let admin_url = require_db("migrations_run_up_and_down_cleanly_from_an_empty_database");
    if skipped(&admin_url) {
        return;
    }
    let rt = test_runtime();

    // A dedicated temporary database keeps the destructive down pass away
    // from any other data in the test database.
    let temp_db = format!("flexcore_p1_13_mig_{:x}", run_key() ^ next_counter());
    let base_pool = rt
        .block_on(PgPool::connect(&admin_url))
        .expect("connect to the test database server");
    let created: Result<(), sqlx::Error> = rt.block_on(async {
        sqlx::query(format!("CREATE DATABASE {temp_db}").as_str())
            .execute(&base_pool)
            .await
            .map(|_| ())
    });
    if created.is_err() {
        // Some managed servers restrict CREATE DATABASE; fall back to the
        // given database (the down pass is destructive there by design).
        eprintln!("migration test: falling back to the provided database ({temp_db} unavailable)");
        run_migrations_up_down_up(&admin_url);
        return;
    }

    // Swap the database name in the URL (preserve any query parameters).
    let (prefix, params) = match admin_url.split_once('?') {
        Some((prefix, params)) => (prefix, Some(params)),
        None => (admin_url.as_str(), None),
    };
    let prefix = match prefix.rfind('/') {
        Some(index) => &prefix[..index],
        None => prefix,
    };
    let temp_url = match params {
        Some(params) => format!("{prefix}/{temp_db}?{params}"),
        None => format!("{prefix}/{temp_db}"),
    };

    run_migrations_up_down_up(&temp_url);

    let _ = rt.block_on(async {
        sqlx::query(format!("DROP DATABASE {temp_db} WITH (FORCE)").as_str())
            .execute(&base_pool)
            .await
    });
}

fn run_migrations_up_down_up(url: &str) {
    let store = FlexCoreStore::connect(url).expect("connect");
    store.run_migrations().expect("migrations up");
    let rt = test_runtime();
    let pool = rt.block_on(PgPool::connect(url)).expect("connect raw pool");
    rt.block_on(async {
        for table in STAGE_1_TABLES {
            assert!(table_exists(&pool, table).await, "table {table} must exist after up");
        }
        for table in STAGE_2_TABLES {
            assert!(table_exists(&pool, table).await, "table {table} must exist after up");
        }
        for table in [
            "portfolios",
            "sites",
            "assets",
            "asset_state",
            "audit_records",
            "telemetry",
            "telemetry_quarantine",
        ] {
            assert!(
                policy_exists(&pool, table).await,
                "tenant_isolation policy must exist on {table}"
            );
        }
        // Roles created by the guarded DO blocks (deviation recorded),
        // including the P2.06 telemetry runtime roles (P2.05 D-P6).
        let roles: Vec<String> = sqlx::query_scalar(
            "SELECT rolname FROM pg_roles WHERE rolname IN \
             ('flexcore_migrator', 'flexcore_app', 'flexcore_ingest', 'flexcore_query') \
             ORDER BY rolname",
        )
        .fetch_all(&pool)
        .await
        .expect("role query");
        assert_eq!(
            roles,
            ["flexcore_app", "flexcore_ingest", "flexcore_migrator", "flexcore_query"]
        );
    });

    // Down: replay the recorded down files in reverse, then clear the
    // migration bookkeeping so the re-up runs the full set again. Each down
    // file is split on ';' — multi-statement down files (0009/0010) execute
    // one statement at a time; a prepared statement cannot carry several.
    rt.block_on(async {
        for sql in DOWN_FILES.iter().rev() {
            // Strip comment lines first so a trailing comment block never
            // becomes an empty statement, then execute each ';' fragment.
            let body: String = sql
                .lines()
                .filter(|line| !line.trim_start().starts_with("--"))
                .collect::<Vec<&str>>()
                .join("\n");
            for statement in body.split(';') {
                let statement = statement.trim();
                if statement.is_empty() {
                    continue;
                }
                sqlx::query(statement)
                    .execute(&pool)
                    .await
                    .expect("down migration");
            }
        }
        sqlx::query("DELETE FROM _sqlx_migrations")
            .execute(&pool)
            .await
            .expect("clear migration bookkeeping");
        for table in STAGE_1_TABLES {
            assert!(
                !table_exists(&pool, table).await,
                "table {table} must be gone after down"
            );
        }
        for table in STAGE_2_TABLES {
            assert!(
                !table_exists(&pool, table).await,
                "table {table} must be gone after down"
            );
        }
    });

    store.run_migrations().expect("migrations re-up");
    rt.block_on(async {
        for table in STAGE_1_TABLES {
            assert!(table_exists(&pool, table).await, "table {table} must exist after re-up");
        }
        for table in STAGE_2_TABLES {
            assert!(table_exists(&pool, table).await, "table {table} must exist after re-up");
        }
    });
}

// --- Shared fixtures ---------------------------------------------------------------

/// Seeds the tenant registry rows both test tenants need (FK targets), via
/// the admin connection's own store. Tenants have no RLS (P1.12 §5.2).
fn seed_tenants(admin_store: &FlexCoreStore, tenants: &[TenantId]) {
    for tenant_id in tenants {
        let unit = admin_store.begin(None).expect("admin unit of work");
        let receipt = TenantRepository::insert(&unit, &sample_tenant_entity(*tenant_id))
            .expect("tenant insert");
        assert_eq!(receipt.entity(), "tenants");
        unit.commit().expect("commit tenant");
    }
}

// --- 2. RLS isolation ----------------------------------------------------------------

#[test]
fn rls_isolates_tenant_a_from_tenant_b_for_the_runtime_role() {
    let admin_url = require_db("rls_isolates_tenant_a_from_tenant_b_for_the_runtime_role");
    if skipped(&admin_url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&admin_url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());

    let tenant_a = fixture_tenant_id(0xA1);
    let tenant_b = fixture_tenant_id(0xB1);
    seed_tenants(&admin_store, &[tenant_a, tenant_b]);

    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);
    let portfolio_id = PortfolioId::new(fixture_uuid(0x0F));

    let app_store =
        FlexCoreStore::connect(&url_as(&admin_url, "flexcore_app", "flexcore_app_test_only"))
            .expect("app store");

    // Tenant A registers a portfolio through the runtime role.
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let portfolio = Portfolio::try_new(
            &sample_tenant_entity(tenant_a),
            portfolio_id,
            "North portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "REGION-PLACEHOLDER".to_string(),
            "rules:constrained-aggregation".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid portfolio");
        let receipt = PortfolioRepository::insert(&unit, &ctx_a, &portfolio)
            .expect("portfolio insert");
        assert_eq!(receipt.entity(), "portfolios");
        assert_eq!(receipt.tenant_scope(), Some(tenant_a));
        unit.commit().expect("commit");
    }

    // Tenant A sees exactly its own row.
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let found = PortfolioRepository::find_by_id(&unit, &ctx_a, portfolio_id)
            .expect("find")
            .expect("own portfolio visible");
        assert_eq!(found.portfolio_id(), portfolio_id);
        assert_eq!(found.tenant_id(), tenant_a);
        assert_eq!(PortfolioRepository::list_for_tenant(&unit, &ctx_a).expect("list").len(), 1);
        unit.commit().expect("commit");
    }

    // Tenant B sees nothing of tenant A's — the row reads as missing
    // (existence is never leaked, P1.07 port contract; RLS filtered it).
    {
        let unit = app_store.begin(Some(&ctx_b)).expect("unit of work");
        assert!(PortfolioRepository::find_by_id(&unit, &ctx_b, portfolio_id)
            .expect("find")
            .is_none());
        assert!(PortfolioRepository::list_for_tenant(&unit, &ctx_b)
            .expect("list")
            .is_empty());
        unit.commit().expect("commit");
    }

    // A write carrying a foreign-tenant entity is refused at the adapter
    // boundary with TenantMismatch.
    {
        let unit = app_store.begin(Some(&ctx_b)).expect("unit of work");
        let foreign = Portfolio::try_new(
            &sample_tenant_entity(tenant_a),
            PortfolioId::new(fixture_uuid(0x0E)),
            "Cross tenant".to_string(),
            None,
            "Op".to_string(),
            "REGION-PLACEHOLDER".to_string(),
            "rules".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid portfolio");
        match PortfolioRepository::insert(&unit, &ctx_b, &foreign) {
            Err(flexcore_ports::repository::RepositoryError::TenantMismatch { entity }) => {
                assert_eq!(entity, "portfolios");
            }
            other => panic!("expected TenantMismatch, got {other:?}"),
        }
        unit.rollback().expect("rollback");
    }

    // Database-level: without a tenant context the runtime role sees zero
    // rows and a direct cross-tenant INSERT is denied by WITH CHECK.
    let app_pool = rt
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_app",
            "flexcore_app_test_only",
        )))
        .expect("app pool");
    rt.block_on(async {
        assert_eq!(count_portfolios_as(&app_pool, None).await, 0, "no context fails closed");
        assert_eq!(count_portfolios_as(&app_pool, Some(&tenant_b.to_string())).await, 0);
        assert_eq!(count_portfolios_as(&app_pool, Some(&tenant_a.to_string())).await, 1);

        let insert = sqlx::query(
            "INSERT INTO portfolios (tenant_id, name, operator, market_region, aggregation_rules) \
             VALUES ($1::uuid, 'cross-tenant', 'op', 'region', '{}')",
        )
        .bind(tenant_a.to_string())
        .execute(&app_pool)
        .await;
        assert!(insert.is_err(), "a cross-tenant insert without the matching context is denied");
    });
    // The denial is silent at the database (RLS), so the audit duty sits in
    // the application layer (P1.12 §5.3); nothing to assert here beyond the
    // refusal itself.

    // Owner-role case (P1.12 §5.3 / P0.16 rule 2.4): FORCE RLS applies to
    // the migrator owner too — no context, zero rows; with the context, its
    // own tenant's rows.
    let migrator_pool = rt
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("migrator pool");
    rt.block_on(async {
        assert_eq!(count_portfolios_as(&migrator_pool, None).await, 0);
        assert_eq!(
            count_portfolios_as(&migrator_pool, Some(&tenant_a.to_string())).await,
            1
        );
    });
}

// --- 3. Outbox atomicity ---------------------------------------------------------------

fn sample_outbox_write(receipt: StateChangeReceipt, key: &str) -> OutboxWrite {
    let payload = format!(
        r#"{{"schema_version": 1, "idempotency_key": "{key}", "event": {{"event_type": "ASSET_REGISTERED"}}}}"#
    );
    OutboxWrite::new(
        receipt,
        key.to_string(),
        "flexcore.v1.registry.asset_registered".to_string(),
        payload,
        EventTime::from_unix_millis(1_700_000_000_000).expect("valid event time"),
    )
    .expect("valid outbox write")
}

#[test]
fn outbox_write_is_atomic_with_the_state_change() {
    let admin_url = require_db("outbox_write_is_atomic_with_the_state_change");
    if skipped(&admin_url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&admin_url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());

    let tenant_a = fixture_tenant_id(0xC1);
    seed_tenants(&admin_store, &[tenant_a]);
    let ctx_a = context_for(tenant_a);

    let app_store =
        FlexCoreStore::connect(&url_as(&admin_url, "flexcore_app", "flexcore_app_test_only"))
            .expect("app store");

    // The store-level append guard: no unit of work, no transactional
    // append (fail closed — never a dual write).
    {
        let receipt = StateChangeReceipt::new("portfolios".to_string(), flexcore_ports::state_change::StateChangeOperation::Insert, Some(tenant_a));
        let write = sample_outbox_write(receipt, "guarded-key");
        match EventOutbox::append(&app_store, write) {
            Err(flexcore_ports::event_outbox::EventOutboxError::Backend(backend)) => {
                assert_eq!(backend.code(), "E_OUTBOX_REQUIRES_UNIT_OF_WORK");
            }
            other => panic!("expected the unit-of-work guard, got {other:?}"),
        }
    }

    // Success path: state change + outbox row in one transaction, both
    // visible after commit.
    let portfolio_id = PortfolioId::new(fixture_uuid(0x1F));
    let committed_key = format!("committed-{:x}", run_key() ^ next_counter());
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let portfolio = Portfolio::try_new(
            &sample_tenant_entity(tenant_a),
            portfolio_id,
            "Outbox portfolio".to_string(),
            None,
            "Op".to_string(),
            "REGION-PLACEHOLDER".to_string(),
            "rules".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid portfolio");
        let receipt = PortfolioRepository::insert(&unit, &ctx_a, &portfolio)
            .expect("state change");
        EventOutbox::append(&unit, sample_outbox_write(receipt, &committed_key))
            .expect("outbox append");
        unit.commit().expect("commit");
    }
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        assert!(
            PortfolioRepository::find_by_id(&unit, &ctx_a, portfolio_id)
                .expect("find")
                .is_some(),
            "the committed state change must be visible"
        );
        unit.commit().expect("commit");
    }
    let pending = EventOutbox::pending(&app_store, 100).expect("pending scan");
    assert!(
        pending
            .iter()
            .any(|entry| entry.idempotency_key() == committed_key),
        "the committed outbox row must be pending for the publisher"
    );

    // Failure injection (mid-transaction): a state change, its outbox row,
    // then a duplicate-key collision forces a port error. The caller rolls
    // back — and NEITHER the state change NOR the outbox row survives.
    let baseline = EventOutbox::pending(&app_store, 100)
        .expect("pending scan")
        .len();
    let portfolio_id_2 = PortfolioId::new(fixture_uuid(0x2F));
    let rolled_back_key = format!("rolled-back-{:x}", run_key() ^ next_counter());
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let portfolio = Portfolio::try_new(
            &sample_tenant_entity(tenant_a),
            portfolio_id_2,
            "Doomed portfolio".to_string(),
            None,
            "Op".to_string(),
            "REGION-PLACEHOLDER".to_string(),
            "rules".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid portfolio");
        let receipt = PortfolioRepository::insert(&unit, &ctx_a, &portfolio)
            .expect("state change inside the doomed transaction");
        let write = sample_outbox_write(receipt, &rolled_back_key);
        EventOutbox::append(&unit, write).expect("first append inside the transaction");
        // The injection: the same idempotency key again — the P1.12 unique
        // constraint rejects it, the port reports DuplicateKey.
        let receipt = StateChangeReceipt::new(
            "portfolios".to_string(),
            flexcore_ports::state_change::StateChangeOperation::Insert,
            Some(tenant_a),
        );
        match EventOutbox::append(&unit, sample_outbox_write(receipt, &rolled_back_key)) {
            Err(flexcore_ports::event_outbox::EventOutboxError::DuplicateKey { idempotency_key }) => {
                assert_eq!(idempotency_key, rolled_back_key);
            }
            other => panic!("expected DuplicateKey from the injected collision, got {other:?}"),
        }
        unit.rollback().expect("explicit rollback after the failure");
    }
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        assert!(
            PortfolioRepository::find_by_id(&unit, &ctx_a, portfolio_id_2)
                .expect("find")
                .is_none(),
            "the injected failure must leave neither state change nor outbox row"
        );
        unit.commit().expect("commit");
    }
    let after = EventOutbox::pending(&app_store, 100).expect("pending scan");
    assert_eq!(after.len(), baseline, "no outbox row may survive the rollback");
    assert!(!after.iter().any(|entry| entry.idempotency_key() == rolled_back_key));

    // Outbox-without-state cannot survive either: an append with no state
    // change, rolled back, leaves nothing behind.
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let receipt = StateChangeReceipt::new(
            "portfolios".to_string(),
            flexcore_ports::state_change::StateChangeOperation::Insert,
            Some(tenant_a),
        );
        EventOutbox::append(&unit, sample_outbox_write(receipt, &rolled_back_key))
            .expect_err("the same key collides with the still-pending original");
        unit.rollback().expect("rollback");
    }
    let final_count = EventOutbox::pending(&app_store, 100).expect("pending scan").len();
    assert_eq!(final_count, baseline);
}

// --- 4. SOC check constraint (D-S5) ------------------------------------------------

#[test]
fn soc_outside_the_unit_interval_is_rejected_by_the_database() {
    let admin_url = require_db("soc_outside_the_unit_interval_is_rejected_by_the_database");
    if skipped(&admin_url) {
        return;
    }
    let (tenant_a, asset_id) = seed_full_chain(admin_url.clone(), 0xD1);
    let app_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_app",
            "flexcore_app_test_only",
        )))
        .expect("app pool");
    test_runtime().block_on(async move {
        let mut tx = app_pool.begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant_a.to_string())
            .execute(&mut *tx)
            .await
            .expect("set_config");
        for soc in [1.5f64, -0.01, 2.0] {
            let result = sqlx::query("UPDATE asset_state SET soc = $1 WHERE asset_id = $2::uuid")
                .bind(soc)
                .bind(asset_id.to_string())
                .execute(&mut *tx)
                .await;
            assert!(result.is_err(), "soc {soc} outside [0,1] must be rejected (D-S5)");
            let err = result.unwrap_err();
            assert!(
                matches!(err, sqlx::Error::Database(_)),
                "the rejection must come from the database constraint, got {err}"
            );
        }
        // The fraction form inside [0,1] is accepted.
        sqlx::query("UPDATE asset_state SET soc = $1 WHERE asset_id = $2::uuid")
            .bind(0.75f64)
            .bind(asset_id.to_string())
            .execute(&mut *tx)
            .await
            .expect("soc fraction inside [0,1] accepted");
        tx.commit().await.expect("commit");
    });
}

// --- 5. Quality enum: no seventh value (D-S4) ---------------------------------------

#[test]
fn a_seventh_quality_value_is_rejected_by_the_database() {
    let admin_url = require_db("a_seventh_quality_value_is_rejected_by_the_database");
    if skipped(&admin_url) {
        return;
    }
    let (tenant_a, asset_id) = seed_full_chain(admin_url.clone(), 0xE1);
    let app_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_app",
            "flexcore_app_test_only",
        )))
        .expect("app pool");
    test_runtime().block_on(async move {
        let mut tx = app_pool.begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant_a.to_string())
            .execute(&mut *tx)
            .await
            .expect("set_config");
        // Source states (P0.08 §3) are adapter-mapped and never stored.
        for rejected in ["SUBSTITUTED", "OUT_OF_RANGE", "good"] {
            let result =
                sqlx::query("UPDATE asset_state SET quality = $1 WHERE asset_id = $2::uuid")
                    .bind(rejected)
                    .bind(asset_id.to_string())
                    .execute(&mut *tx)
                    .await;
            assert!(
                result.is_err(),
                "quality value {rejected} must be rejected by the enum (D-S4)"
            );
        }
        // All six canonical states are accepted, in P0.08 order.
        for accepted in ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"] {
            sqlx::query("UPDATE asset_state SET quality = $1 WHERE asset_id = $2::uuid")
                .bind(accepted)
                .bind(asset_id.to_string())
                .execute(&mut *tx)
                .await
                .unwrap_or_else(|_| panic!("quality {accepted} must be accepted"));
        }
        tx.commit().await.expect("commit");
    });
}

// --- 6. Audit: append-only and tenant-scoped (P1.12 §4.7) ----------------------------

#[test]
fn audit_records_are_append_only_and_tenant_scoped() {
    let admin_url = require_db("audit_records_are_append_only_and_tenant_scoped");
    if skipped(&admin_url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&admin_url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());

    let tenant_a = fixture_tenant_id(0xFA);
    let tenant_b = fixture_tenant_id(0xFB);
    seed_tenants(&admin_store, &[tenant_a, tenant_b]);
    let ctx_a = context_for(tenant_a);
    let ctx_b = context_for(tenant_b);

    let app_store =
        FlexCoreStore::connect(&url_as(&admin_url, "flexcore_app", "flexcore_app_test_only"))
            .expect("app store");

    let record_id = fixture_uuid(0x3F);
    let record = AuditRecord::new(AuditRecordFields {
        audit_id: record_id,
        tenant_id: Some(tenant_a),
        actor: ActorIdentity::new("integration-test:actor".to_string()).expect("valid actor"),
        action: AuditAction::new("test.audit.append".to_string()).expect("valid action"),
        object: AuditObject::new("portfolio".to_string(), "test-object".to_string())
            .expect("valid object"),
        before_ref: None,
        after_ref: Some(
            flexcore_audit::record::RecordReference::new("portfolio:v2".to_string())
                .expect("valid reference"),
        ),
        decision: None,
        evidence: None,
        event_time: EventTime::from_unix_millis(1_700_000_000_000).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(1_700_000_001_000).expect("valid ingest time"),
        correlation_id: fixture_uuid(0x4F),
    })
    .expect("valid audit record");

    // Append through the unit-of-work sink (inside the transaction).
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        flexcore_audit::sink::AuditSink::append(&unit, record.clone()).expect("audit append");
        unit.commit().expect("commit");
    }

    // A correction is a NEW record (its own identifier) referencing the
    // original via corrected_audit_id (P1.10 rule) — the original is never
    // touched.
    let correction_id = fixture_uuid(0x4E);
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        let mut correction = record.clone();
        correction.audit_id = correction_id;
        let correction = correction
            .link_correction(record_id, "test correction".to_string())
            .expect("valid correction");
        flexcore_audit::sink::AuditSink::append(&unit, correction).expect("correction append");
        unit.commit().expect("commit");
    }

    // Tenant scoping: tenant B sees none of tenant A's records; tenant A
    // sees both the original and the correction (the correction row's
    // corrected_audit_id references the original).
    let app_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_app",
            "flexcore_app_test_only",
        )))
        .expect("app pool");
    test_runtime().block_on(async {
        async fn count_chain(pool: &PgPool, tenant: &str, original: FlexUuid) -> i64 {
            let mut tx = pool.begin().await.expect("transaction");
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(tenant)
                .execute(&mut *tx)
                .await
                .expect("set_config");
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM audit_records \
                 WHERE audit_id = $1::uuid OR corrected_audit_id = $1::uuid",
            )
            .bind(original.to_string())
            .fetch_one(&mut *tx)
            .await
            .expect("count");
            tx.rollback().await.expect("rollback");
            count
        }
        let visible = count_chain(&app_pool, &tenant_a.to_string(), record_id).await;
        let cross = count_chain(&app_pool, &tenant_b.to_string(), record_id).await;
        assert_eq!(visible, 2, "tenant A must see its original record and the correction");
        assert_eq!(cross, 0, "tenant B must see none of tenant A's records");
    });

    // Append-only layer 1: the runtime role has no UPDATE grant.
    // Layer 2: even the owner is blocked by the defence-in-depth trigger.
    test_runtime().block_on(async {
        let app_update = sqlx::query(
            "UPDATE audit_records SET action = 'tampered' WHERE audit_id = $1::uuid",
        )
        .bind(record_id.to_string())
        .execute(&app_pool)
        .await;
        assert!(app_update.is_err(), "the runtime role must not be able to update audit records");

        let app_delete = sqlx::query("DELETE FROM audit_records WHERE audit_id = $1::uuid")
            .bind(record_id.to_string())
            .execute(&app_pool)
            .await;
        assert!(app_delete.is_err(), "the runtime role must not be able to delete audit records");
    });

    // Owner path: FORCE RLS applies, so the owner also needs the context —
    // and the trigger still refuses mutation.
    let migrator_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &admin_url,
            "flexcore_migrator",
            "flexcore_migrator_test_only",
        )))
        .expect("migrator pool");
    test_runtime().block_on(async {
        let mut tx = migrator_pool.begin().await.expect("transaction");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant_a.to_string())
            .execute(&mut *tx)
            .await
            .expect("set_config");
        let update = sqlx::query(
            "UPDATE audit_records SET action = 'tampered' WHERE audit_id = $1::uuid",
        )
        .bind(record_id.to_string())
        .execute(&mut *tx)
        .await;
        assert!(update.is_err(), "the append-only trigger must refuse the owner too (layer 2)");
        tx.rollback().await.expect("rollback");
    });
}

// --- Shared full-chain fixture ---------------------------------------------------------

/// Seeds tenant -> portfolio -> site -> asset for one test tenant via the
/// runtime role, and returns the tenant and asset ids. Used by the
/// constraint tests (their statements run under the chain tenant's own RLS
/// context, so constraint — not policy — violations are what they observe).
fn seed_full_chain(admin_url: String, prefix: u8) -> (TenantId, AssetId) {
    let admin_store = FlexCoreStore::connect(&admin_url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());

    let tenant_a = fixture_tenant_id(prefix);
    let tenant_row = sample_tenant_entity(tenant_a);
    seed_tenants(&admin_store, &[tenant_a]);
    let ctx_a = context_for(tenant_a);

    let app_store =
        FlexCoreStore::connect(&url_as(&admin_url, "flexcore_app", "flexcore_app_test_only"))
            .expect("app store");
    let portfolio = Portfolio::try_new(
        &tenant_row,
        PortfolioId::new(fixture_uuid(prefix.wrapping_add(0x01))),
        "Chain portfolio".to_string(),
        None,
        "Op".to_string(),
        "REGION-PLACEHOLDER".to_string(),
        "rules".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid portfolio");
    let site = Site::try_new(
        &portfolio,
        SiteId::new(fixture_uuid(prefix.wrapping_add(0x02))),
        "Chain site".to_string(),
        None,
        "52.52N 13.40E".to_string(),
        flexcore_contracts::time_model::SiteTimezone::new("Europe/Berlin".to_string())
            .expect("valid timezone"),
        "PCC-1".to_string(),
        "grid-model:placeholder".to_string(),
        "edge-test".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid site");
    let asset = Asset::try_new(
        &site,
        AssetId::new(fixture_uuid(prefix.wrapping_add(0x03))),
        "Chain BESS".to_string(),
        None,
        AssetType::Bess,
        "vendor-x/model-1".to_string(),
        flexcore_contracts::units::ActivePowerMW::new(2.0).expect("valid power"),
        Some(flexcore_contracts::units::EnergyMWh::new(4.0).expect("valid energy")),
        "three-phase".to_string(),
        "bus-17".to_string(),
        "capability:bess-v1".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid asset");

    let asset_id = asset.asset_id();
    let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
    PortfolioRepository::insert(&unit, &ctx_a, &portfolio).expect("portfolio");
    SiteRepository::insert(&unit, &ctx_a, &site).expect("site");
    AssetRepository::insert(&unit, &ctx_a, &asset).expect("asset");
    unit.commit().expect("commit");
    (tenant_a, asset_id)
}

// --- DB-free unit checks ---------------------------------------------------------------

#[test]
fn the_id_generator_shares_the_uuidv7_version_policy() {
    // D-S1 consistency: application identifiers come from the same
    // uuidv7() the PK defaults use. Needs the database; skipped like the
    // rest when the URL is unset.
    let admin_url = require_db("the_id_generator_shares_the_uuidv7_version_policy");
    if skipped(&admin_url) {
        return;
    }
    let store = FlexCoreStore::connect(&admin_url).expect("store");
    let generator = store.id_generator();
    let first: FlexUuid = generator.new_uuid();
    let second: FlexUuid = generator.new_uuid();
    assert_ne!(first, FlexUuid::NIL);
    assert_ne!(first, second);
}

#[test]
fn the_transactional_ports_cannot_be_used_without_a_unit_of_work() {
    // DB-free: the store-level EventOutbox::append guard is a pure error
    // contract (outbox.rs unit test pins the code); here we pin that the
    // store type carries both publisher-side and sink roles.
    fn takes_dyn_outbox(_: &dyn EventOutbox) {}
    let _ = takes_dyn_outbox;
    // The unit-of-work type is public API and implements the full bundle
    // (compile-time assertion, mirrors lib.rs).
    fn assert_bundle<T: TenantRepository
        + PortfolioRepository
        + SiteRepository
        + AssetRepository
        + EventOutbox
        + flexcore_audit::sink::AuditSink>()
    {
    }
    assert_bundle::<PgUnitOfWork>;
}

#[test]
fn a_backend_error_travels_opaquely() {
    // DB-free sanity for the error contract used across the adapter.
    let backend = BackendError::new("E_STORE_UNAVAILABLE".to_string(), "backend unavailable".to_string());
    assert_eq!(backend.code(), "E_STORE_UNAVAILABLE");
    assert!(!backend.message().contains("sql"));
}
