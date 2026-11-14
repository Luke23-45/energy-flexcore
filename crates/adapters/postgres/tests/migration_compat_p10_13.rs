// P10.13 — Schema-migration compatibility tests (ledger P10.13; report
//          §22.2 CI gate "migration tests and schema compatibility"; §10.5;
//          §21.2 "schema migrations with compatibility windows").
//
// HONEST STATUS (binding): this suite is WRITTEN, never executed. The loop
// bans (no builds, no test runs, no installs) were held; per
// ledger/phase-10-v1-release-hardening/ENTRY_NOTE.md boundary 3 this is
// "implemented, awaiting human build/test". Zero observed failures are NOT
// claimed from an unrun suite — the run is the human gate. Nothing in this
// file records a pass; the run record lives in
// ledger/phase-10-v1-release-hardening/13_schema_migration_compat.md §7 and
// is EMPTY until a human executes the suite against a PostgreSQL 18 service.
//
// What this suite adds over tests/integration.rs (which owns the up/down/up
// replay on an EMPTY database and the RLS/outbox/SOC/enum/audit slices):
//
// 1. STRUCTURAL enumeration of the whole migration set (no DB needed):
//    the embedded migrator must carry EXACTLY versions 1..=18 in ascending
//    order with the recorded descriptions, and every up file must have a
//    down file (the include_str! enumeration below fails the build if a
//    down file is missing — the "every migration is reversible or its
//    irreversibility is recorded" invariant, checked structurally).
// 2. FORWARD APPLY + IDEMPOTENCY + DRIFT DETECTION (the sqlx framework
//    semantics): the full chain applies in order from an empty database;
//    re-running is a no-op (applied migrations are SKIPPED by version, never
//    re-executed — that is the framework's idempotency contract); a tampered
//    checksum in _sqlx_migrations is refused (the VersionMismatch class the
//    adapter maps to "an applied migration no longer matches the deployed
//    binary").
// 3. FORWARD MIGRATION OVER PILOT-SHAPED DATA (the ledger's "production-
//    shaped, not empty" requirement, simulated): stage 0001..0008, seed a
//    registry chain (tenant → portfolio → site → asset) THROUGH THE
//    REPOSITORIES (the application shape, not bare SQL) plus one audit row
//    through the runtime INSERT grant, then apply the remaining chain
//    (0009..0018) OVER the populated schema and prove the data survives with
//    its meaning intact (0009's ADD COLUMN ... DEFAULT backfills; 0012's PK
//    extension still serves the duplicate classification and accepts
//    versioned corrections without mutating the original row).
// 4. APPEND-ONLY INVARIANTS ACROSS THE FULL CHAIN, verified BY SQL BEHAVIOUR
//    against the real schema: 0008's trigger refuses UPDATE/DELETE for the
//    OWNER too (grants cannot bind the owner; the trigger does); the
//    no-DELETE/no-UPDATE grant matrix holds for flexcore_app on every
//    append-only surface of the chain (0008 audit_records, 0010
//    telemetry_quarantine, 0016 dispatch_command_transitions, 0017
//    feasibility_check_records, 0018 edge_device_registry +
//    edge_enrollment_journal) and the column-level UPDATE exception on
//    dispatch_commands is EXACTLY (state, superseded_by_command_id); 0018's
//    one-ACTIVE-term partial unique index refuses a second ACTIVE term and
//    admits rotation after REVOKED; 0016's database defence-in-depth keeps
//    the §14.1 gate ORDER — no UPDATE ever moves a row into ISSUED except
//    from APPROVED (the schema-level half of the P10.15 cross-reference:
//    no command reaches ISSUED without an APPROVED state).
//
// DOWNGRADE IS NOT SUPPORTED BY THE RUNTIME PATH (recorded, not hidden):
// the embedded `sqlx::migrate::Migrator` applies UP migrations only; the
// recorded down files are dev/CI replay artifacts exercised by
// tests/integration.rs (up/down/up on an empty database) — never an
// operational rollback path. A rollback over a data-bearing database
// therefore means RESTORE FROM BACKUP (P10.11), never an improvised
// down-migration. The irreversibility register (0001 roles, 0012 correction
// rows) is in the record §6, not here.
//
// Gating (the recorded harness decision, CONTEXT.md open question 5): every
// test skips cleanly (with a printed reason) when FLEXCORE_TEST_DATABASE_URL
// is unset; CI provides a PostgreSQL 18 service. Test data is unique per run
// (timestamp-derived identifiers + fixture markers). Temporary migration
// databases are dropped at the end (best effort).
//
// KNOWN-RISK NOTE FOR THE FIRST HUMAN RUN (recorded, not assumed): the
// stage-1 bookkeeping inserts into `_sqlx_migrations` name the sqlx 0.9
// bookkeeping columns (version, description, installed_on, success,
// checksum, execution_time) and reuse the EMBEDDED migrator's own checksum
// values (`Migration::checksum`) so the framework's checksum validation
// stays consistent. If the 0.9 bookkeeping schema differs, the failure is a
// loud, first-statement error in test 3 — a schema pin to fix then, not a
// silent drift.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use sqlx::PgPool;

// --- The FULL migration set, enumerated (P10.13 ledger step 1) -------------------
//
// Every up and down file of the v1.0.0 candidate, pinned by include_str! —
// a missing or renamed file is a COMPILE error, so the enumeration cannot
// rot silently. The record §4 carries the per-migration compatibility
// windows; this is the set the windows are recorded FOR.

const UP_FILES: [&str; 18] = [
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0001_quality_enum.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0002_tenants.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0003_portfolios.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0004_sites.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0005_assets.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0006_asset_state.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0007_outbox.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0008_audit_records.up.sql")),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0009_registry_lifecycle_p2_03.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0010_telemetry_storage_p2_06.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0011_telemetry_api_read_grant_p2_19.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0012_correction_versions_p2_31.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0013_optimization_results_p3_21.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0014_schedule_revisions_p3_22.up.sql"
    )),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0015_envelopes_p3_28.up.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0016_commands_p4_10.up.sql")),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0017_feasibility_checks_p4_30.up.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0018_edge_device_registry_p6_06.up.sql"
    )),
];

// The recorded down files (P1.12 §7: every migration has a down file or a
// recorded irreversible reason — 0001's is roles-only, recorded in the down
// file itself). include_str! pins their presence at compile time. These are
// REPLAY ARTIFACTS (dev/CI), never the runtime rollback path — see the
// module header's downgrade statement.
const DOWN_FILES: [&str; 18] = [
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0001_quality_enum.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0002_tenants.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0003_portfolios.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0004_sites.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0005_assets.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0006_asset_state.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0007_outbox.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0008_audit_records.down.sql")),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0009_registry_lifecycle_p2_03.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0010_telemetry_storage_p2_06.down.sql"
    )),
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
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0015_envelopes_p3_28.down.sql")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0016_commands_p4_10.down.sql")),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0017_feasibility_checks_p4_30.down.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0018_edge_device_registry_p6_06.down.sql"
    )),
];

/// The recorded descriptions (sqlx derives them from the file names). Pinned
/// so a rename or a re-order is a test failure, never a silent drift.
const EXPECTED_DESCRIPTIONS: [&str; 18] = [
    "quality_enum",
    "tenants",
    "portfolios",
    "sites",
    "assets",
    "asset_state",
    "outbox",
    "audit_records",
    "registry_lifecycle_p2_03",
    "telemetry_storage_p2_06",
    "telemetry_api_read_grant_p2_19",
    "correction_versions_p2_31",
    "optimization_results_p3_21",
    "schedule_revisions_p3_22",
    "envelopes_p3_28",
    "commands_p4_10",
    "feasibility_checks_p4_30",
    "edge_device_registry_p6_06",
];

/// The append-only surfaces of the chain and the grants flexcore_app must
/// (and must NOT) hold on them. 0008's audit_records header documents the
/// three-layer enforcement; 0016/0018 repeat the grant layer. telemetry
/// itself is not in this list (its INSERT path is flexcore_ingest only, and
/// 0011 grants flexcore_app exactly one SELECT).
const APPEND_ONLY_TABLES: [&str; 6] = [
    "audit_records",
    "dispatch_command_transitions",
    "feasibility_check_records",
    "edge_device_registry",
    "edge_enrollment_journal",
    "telemetry_quarantine",
];

// --- Harness helpers (the recorded P1.13/P2.06/P4.10 decisions) ------------------

fn test_url() -> Option<String> {
    std::env::var("FLEXCORE_TEST_DATABASE_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn require_db(test: &str) -> Option<String> {
    match test_url() {
        Some(url) => Some(url),
        None => {
            println!(
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a PostgreSQL 18 service)"
            );
            None
        }
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
    bytes[1] = 0xD1; // P10.13 fixture marker
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

fn tenant_context(uuid: FlexUuid) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(uuid),
            subject: "p10.13-migration-compat-test".to_string(),
        },
    ))
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// Rewrites the credentials of an admin URL to connect as one of the
/// service roles (the recorded tests/integration.rs helper).
fn url_as(url: &str, user: &str, password: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((_, host_and_path)) = rest.split_once('@') {
            return format!("{scheme}://{user}:{password}@{host_and_path}");
        }
        return format!("{scheme}://{user}:{password}@{rest}");
    }
    format!("{url}?user={user}&password={password}")
}

/// Grants LOGIN + test-only passwords to the NOLOGIN service roles the
/// migrations create (the recorded tests/corrections.rs helper).
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

/// Executes a multi-statement SQL script by splitting on `;` OUTSIDE
/// dollar-quoted bodies and comment lines (migrations 0001 and 0008 carry
/// `DO $$ ... $$;` blocks whose bodies contain semicolons — a naive split
/// would execute DO-body fragments as standalone statements).
/// Statement-by-statement execution is the recorded pattern of
/// tests/integration.rs's down replay (a prepared statement cannot carry
/// several statements).
fn execute_script(pool: &PgPool, script: &str) {
    let mut current = String::new();
    let mut in_dollar_body = false;
    for line in script.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") && !in_dollar_body {
            continue;
        }
        // The migration files 0001..0018 use only the bare `$$` delimiter;
        // a tagged delimiter would need the full tag scanned — none exists
        // (verified by inspection, recorded in the record §4). Process the
        // line character by character with a peekable iterator.
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'$') {
                chars.next();
                in_dollar_body = !in_dollar_body;
                current.push_str("$$");
                continue;
            }
            if ch == ';' && !in_dollar_body {
                let statement = current.trim().to_string();
                if !statement.is_empty() {
                    test_runtime()
                        .block_on(sqlx::query(&statement).execute(pool))
                        .expect("script statement must execute");
                }
                current.clear();
                continue;
            }
            current.push(ch);
        }
        current.push('\n');
    }
    let statement = current.trim().to_string();
    if !statement.is_empty() {
        test_runtime()
            .block_on(sqlx::query(&statement).execute(pool))
            .expect("final script statement must execute");
    }
}

/// Creates a dedicated temporary database (the recorded
/// tests/integration.rs pattern) and returns its URL. Falling back to the
/// provided database is NOT an option here: tests 2..4 seed data and rely
/// on an isolated schema.
fn temp_database(admin_url: &str, label: &str) -> String {
    let temp_db = format!("flexcore_p10_13_{label}_{:x}", run_key() ^ next_counter());
    let rt = test_runtime();
    let base_pool = rt
        .block_on(PgPool::connect(admin_url))
        .expect("connect to the test database server");
    rt.block_on(async {
        sqlx::query(format!("CREATE DATABASE {temp_db}").as_str())
            .execute(&base_pool)
            .await
    })
    .expect("temporary database must be creatable (the CI service grants CREATE DATABASE)");
    let (prefix, params) = match admin_url.split_once('?') {
        Some((prefix, params)) => (prefix, Some(params)),
        None => (admin_url, None),
    };
    let prefix = match prefix.rfind('/') {
        Some(index) => &prefix[..index],
        None => prefix,
    };
    match params {
        Some(params) => format!("{prefix}/{temp_db}?{params}"),
        None => format!("{prefix}/{temp_db}"),
    }
}

fn drop_database(admin_url: &str, temp_url: &str) {
    let db_name = temp_url
        .split('/')
        .next_back()
        .and_then(|tail| tail.split('?').next())
        .expect("temp url carries a database name")
        .to_string();
    let rt = test_runtime();
    let base_pool = rt.block_on(PgPool::connect(admin_url)).expect("reconnect base");
    let _ = rt.block_on(async {
        sqlx::query(format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)").as_str())
            .execute(&base_pool)
            .await
    });
}

async fn count_migrations(pool: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .expect("bookkeeping table exists after a migration run")
}

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("information_schema query")
        > 0
}

/// Runs one statement inside a transaction with the tenant GUC set (the
/// recorded command_store.rs `with_tenant` pattern); returns the outcome so
/// the caller asserts refused-or-accepted. The transaction is rolled back
/// either way, so a refused attempt leaves zero rows. NEVER call this from
/// inside another runtime's block_on — it owns its own current-thread
/// runtime, like every other sync helper in this file.
fn with_tenant(
    pool: &PgPool,
    tenant_id: &str,
    statement: String,
    binds: Vec<String>,
) -> Result<(), sqlx::Error> {
    test_runtime().block_on(async move {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await?;
        let mut query = sqlx::query(&statement);
        for bind in &binds {
            query = query.bind(bind);
        }
        let result = query.execute(&mut *tx).await.map(|_| ());
        let _ = tx.rollback().await;
        result
    })
}

/// Reads rows under an explicit tenant context (RLS-protected tables filter
/// EVERY role — FORCE — including the owner, so all reads of tenant-owned
/// rows go through this). Returns the fetched rows for scalar extraction.
async fn tenant_rows(
    pool: &PgPool,
    tenant_id: &str,
    statement: String,
    binds: Vec<String>,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    let mut query = sqlx::query(&statement);
    for bind in &binds {
        query = query.bind(bind);
    }
    let rows = query.fetch_all(&mut *tx).await;
    let _ = tx.rollback().await;
    rows
}

/// Creates the fixed 2030-01 telemetry partition as the migrator role (the
/// recorded tests/corrections.rs pattern; no migration creates partitions).
fn ensure_january_2030_partition(temp_url: &str) {
    let migrator_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            temp_url,
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
        .expect("partition DDL as the table owner");
    });
}

/// Seeds the tenant → portfolio → site → asset FK chain THROUGH THE
/// REPOSITORIES (the application shape; the recorded command_store.rs
/// pattern, not bare SQL).
fn seed_registry_chain(
    store: &FlexCoreStore,
    tenant_uuid: FlexUuid,
) -> (TenantContext, FlexUuid, FlexUuid, FlexUuid) {
    let ctx = tenant_context(tenant_uuid);
    let portfolio_uuid = fixture_uuid(0xA2);
    let site_uuid = fixture_uuid(0xA3);
    let asset_uuid = fixture_uuid(0xA4);
    let created = EventTime::from_unix_millis(1_700_000_000_000).expect("valid");
    let unit: PgUnitOfWork = store.begin(Some(&ctx)).expect("unit of work opens");
    let tenant = Tenant::try_new(
        TenantId::new(tenant_uuid),
        "p10.13 tenant".to_string(),
        None,
        format!("idp-{tenant_uuid}"),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    TenantRepository::insert(&unit, &tenant).expect("tenant inserts");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(portfolio_uuid),
        "p10.13 portfolio".to_string(),
        None,
        "operator".to_string(),
        "none (D1 OPEN)".to_string(),
        "{}".to_string(),
        created,
    )
    .expect("portfolio builds");
    PortfolioRepository::insert(&unit, &ctx, &portfolio).expect("portfolio inserts");
    let site = Site::try_new(
        &portfolio,
        SiteId::new(site_uuid),
        "p10.13 site".to_string(),
        None,
        "synthetic".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC"),
        "pcc-fixture".to_string(),
        "site-grid-model-reference-v1".to_string(),
        "edge-fixture".to_string(),
        created,
    )
    .expect("site builds");
    SiteRepository::insert(&unit, &ctx, &site).expect("site inserts");
    let asset = Asset::try_new(
        &site,
        AssetId::new(asset_uuid),
        "p10.13 BESS".to_string(),
        None,
        AssetType::Bess,
        "vendor-x/model-1".to_string(),
        ActivePowerMW::new(2.0).expect("valid power"),
        Some(EnergyMWh::new(4.0).expect("valid energy")),
        "three-phase".to_string(),
        "bus-17".to_string(),
        "capability:bess-v1".to_string(),
        created,
    )
    .expect("asset builds");
    AssetRepository::insert(&unit, &ctx, &asset).expect("asset inserts");
    unit.commit().expect("committed");
    (ctx, portfolio_uuid, site_uuid, asset_uuid)
}

// --- 1. Structural enumeration (NO database needed) ------------------------------

#[test]
fn the_migration_set_is_exactly_18_paired_and_ordered() {
    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/");

    // The embedded set is EXACTLY the enumerated 18, in ascending version
    // order (sqlx sorts by version — the forward-apply order), with the
    // recorded descriptions.
    let migrations = &*MIGRATOR.migrations;
    assert_eq!(
        migrations.len(),
        18,
        "the v1.0.0 candidate carries exactly 18 migrations"
    );
    for (index, migration) in migrations.iter().enumerate() {
        assert_eq!(
            migration.version,
            (index + 1) as i64,
            "migration versions are dense 1..=18 in file order"
        );
        assert_eq!(
            migration.description,
            EXPECTED_DESCRIPTIONS[index],
            "migration {} carries the recorded description",
            index + 1
        );
        assert!(
            !migration.sql.trim().is_empty(),
            "migration {} carries non-empty up SQL",
            index + 1
        );
        assert!(
            !UP_FILES[index].trim().is_empty(),
            "the enumerated up file {} is non-empty",
            index + 1
        );
    }
    // Every migration has a recorded down file (the compile-time
    // include_str! enumeration above proves presence; this asserts the
    // pairing count).
    assert_eq!(DOWN_FILES.len(), UP_FILES.len());
}

// --- 2. Forward apply from empty: ordering, idempotency, drift detection ---------

#[test]
fn forward_apply_is_ordered_idempotent_and_drift_detecting() {
    let Some(admin_url) = require_db("forward_apply_is_ordered_idempotent_and_drift_detecting")
    else {
        return;
    };
    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/");
    let temp_url = temp_database(&admin_url, "idem");
    let store = FlexCoreStore::connect(&temp_url).expect("connect");
    store
        .run_migrations()
        .expect("migrations apply forward from empty");

    let pool = test_runtime().block_on(PgPool::connect(&temp_url)).expect("raw pool");
    test_runtime().block_on(async {
        // Every version applied exactly once, in success state.
        let versions: Vec<i64> = sqlx::query_scalar(
            "SELECT version FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("bookkeeping query");
        assert_eq!(
            versions,
            (1..=18).collect::<Vec<i64>>(),
            "the bookkeeping records exactly versions 1..=18 applied"
        );
        let failures: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM _sqlx_migrations WHERE success = false",
        )
        .fetch_one(&pool)
        .await
        .expect("bookkeeping query");
        assert_eq!(failures, 0, "no migration is recorded as failed");
        // The chain's surfaces exist (spot checks across the whole chain:
        // one per stage).
        for table in [
            "tenants",
            "outbox",
            "audit_records",
            "telemetry",
            "schedule_revisions",
            "dispatch_commands",
            "edge_device_registry",
        ] {
            assert!(
                table_exists(&pool, table).await,
                "table {table} exists after the full up"
            );
        }
    });

    // IDEMPOTENCY (the framework's contract): a second run SKIPS applied
    // migrations — no error, no re-execution, no bookkeeping growth.
    store
        .run_migrations()
        .expect("a second migration run is a no-op over the applied set");
    test_runtime().block_on(async {
        assert_eq!(
            count_migrations(&pool).await,
            18,
            "the second run does not re-apply or duplicate bookkeeping rows"
        );
    });

    // DRIFT DETECTION: a tampered checksum for an applied version is
    // refused on the next run (the sqlx VersionMismatch class; the adapter
    // maps it to the recorded store.rs message "an applied migration no
    // longer matches the deployed binary" and the E_STORE_MIGRATION code —
    // both mirrored here as string literals because the adapter's `codes`
    // module is crate-private).
    let baseline_checksum: Vec<u8> = {
        let migration = MIGRATOR
            .migrations
            .iter()
            .find(|migration| migration.version == 1)
            .expect("version 1 is in the embedded set");
        migration.checksum.to_vec()
    };
    test_runtime().block_on(async {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = 1")
            .bind(vec![0u8; baseline_checksum.len()])
            .execute(&pool)
            .await
            .expect("checksum tamper applies");
    });
    let drifted = store.run_migrations();
    assert!(
        drifted.is_err(),
        "a tampered bookkeeping checksum must be refused, never silently ignored"
    );
    if let Err(error) = drifted {
        let rendered = format!("{}: {}", error.code(), error.message());
        assert!(
            rendered.contains("E_STORE_MIGRATION")
                || rendered.contains("no longer matches the deployed binary"),
            "the drift error is the recorded VersionMismatch class, got: {rendered}"
        );
    }

    // Restore the true checksum and prove the set applies cleanly again
    // (the tamper, not the migration set, was the fault).
    test_runtime().block_on(async {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = 1")
            .bind(baseline_checksum.clone())
            .execute(&pool)
            .await
            .expect("checksum restore applies");
    });
    store
        .run_migrations()
        .expect("the restored set applies as a no-op again");
    test_runtime().block_on(async {
        assert_eq!(count_migrations(&pool).await, 18);
    });
    drop_database(&admin_url, &temp_url);
}

// --- 3. Forward migration over PILOT-SHAPED data (0001..0008 seeded, then 0009..0018) ---

#[test]
fn forward_migration_over_pilot_shaped_data_preserves_integrity() {
    let Some(admin_url) =
        require_db("forward_migration_over_pilot_shaped_data_preserves_integrity")
    else {
        return;
    };
    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/");
    let temp_url = temp_database(&admin_url, "shaped");

    // STAGE 1: apply 0001..0008 manually (the ledger's "copy of
    // production-shaped data" simulation: data exists BEFORE the later
    // chain runs over it).
    let stage_pool = test_runtime().block_on(PgPool::connect(&temp_url)).expect("stage pool");
    for script in UP_FILES.iter().take(8) {
        execute_script(&stage_pool, script);
    }
    // Record stage-1 versions in the framework's bookkeeping with the
    // EMBEDDED checksums, so the subsequent run validates them normally and
    // applies exactly the remaining chain (see the module-header risk note:
    // the _sqlx_migrations column list is the sqlx 0.9 shape, pinned at the
    // first human run).
    test_runtime().block_on(async {
        for migration in MIGRATOR.migrations.iter().take(8) {
            sqlx::query(
                "INSERT INTO _sqlx_migrations \
                     (version, description, installed_on, success, checksum, execution_time) \
                 VALUES ($1, $2, now(), true, $3, 0)",
            )
            .bind(migration.version)
            .bind(migration.description.to_string())
            .bind(migration.checksum.to_vec())
            .execute(&stage_pool)
            .await
            .expect("stage-1 bookkeeping row inserts");
        }
    });

    // SEED: the registry chain through the repositories (application
    // shape), plus one audit row through the runtime INSERT grant path (the
    // append-only surface is fed through its grant, not bypassed).
    let store = FlexCoreStore::connect(&temp_url).expect("connect over the stage-1 schema");
    ensure_test_roles(store.pool());
    let tenant_uuid = fixture_uuid(0xB1);
    let (ctx, _portfolio, _site, asset_uuid) = seed_registry_chain(&store, tenant_uuid);
    let tenant_text = ctx.tenant_id().uuid().to_string();
    let app_url = url_as(&temp_url, "flexcore_app", "flexcore_app_test_only");
    let ingest_url = url_as(&temp_url, "flexcore_ingest", "flexcore_ingest_test_only");
    let app_pool = test_runtime().block_on(PgPool::connect(&app_url)).expect("app pool");
    let ingest_pool =
        test_runtime().block_on(PgPool::connect(&ingest_url)).expect("ingest pool");
    ensure_january_2030_partition(&temp_url);
    let audit_id = fixture_uuid(0xA9);
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO audit_records \
             (audit_id, tenant_id, actor, action, object_type, object_id, event_time, \
              correlation_id) \
         VALUES ($1::uuid, $2::uuid, 'svc:p10.13', 'registry.create', 'asset', $3, now(), \
                 'p10.13-seed-1')"
            .to_string(),
        vec![audit_id.to_string(), tenant_text.clone(), asset_uuid.to_string()],
    )
    .expect("the audit row inserts through the INSERT grant");

    // FORWARD MIGRATION: apply the REMAINING chain (0009..0018) over the
    // populated schema. The framework skips the recorded stage-1 versions
    // and runs the rest — this is the compatibility exercise.
    store
        .run_migrations()
        .expect("the remaining chain applies over pilot-shaped data");

    // (a) The full bookkeeping: exactly 18 versions, none re-applied.
    let versions: Vec<i64> = test_runtime().block_on(async {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&app_pool)
            .await
            .expect("bookkeeping query")
    });
    assert_eq!(versions, (1..=18).collect::<Vec<i64>>());

    // (b) The seeded data SURVIVED with its meaning intact: the audit row
    // is unchanged where it was written (0009..0018 are additive to the
    // registry/telemetry/dispatch/edge surfaces; none touches
    // audit_records' content).
    let audit_rows = test_runtime().block_on(tenant_rows(
        &app_pool,
        &tenant_text,
        "SELECT count(*) AS audit_count FROM audit_records \
         WHERE audit_id = $1::uuid AND actor = 'svc:p10.13' AND object_id = $2::text"
            .to_string(),
        vec![audit_id.to_string(), asset_uuid.to_string()],
    ))
    .expect("the seeded audit row is readable back under its tenant context");
    let audit_count: i64 = audit_rows[0].try_get("audit_count").expect("count column");
    assert_eq!(audit_count, 1, "the seeded audit row survives, unchanged");

    // (c) 0009's ADD COLUMN ... NOT NULL DEFAULT backfilled the seeded
    // rows: the seeded asset reads lifecycle_status = 'active' without any
    // data rewrite.
    let lifecycle_rows = test_runtime().block_on(tenant_rows(
        &app_pool,
        &tenant_text,
        "SELECT lifecycle_status FROM assets WHERE asset_id = $1::uuid AND tenant_id = $2::uuid"
            .to_string(),
        vec![asset_uuid.to_string(), tenant_text.clone()],
    ))
    .expect("the seeded asset carries the backfilled 0009 column");
    let lifecycle: String = lifecycle_rows[0].try_get("lifecycle_status").expect("status column");
    assert_eq!(lifecycle, "active", "0009's default backfills pre-existing rows");

    // (d) 0012's PK extension still serves the duplicate classification:
    // the ORIGINAL v1 row inserts cleanly; a conflicting re-ingest of the
    // same identity is a duplicate-key refusal (the P2.09 classification
    // matches telemetry_pkey by NAME).
    with_tenant(
        &ingest_pool,
        &tenant_text,
        "INSERT INTO telemetry \
             (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
              source_id, quality, value_num, unit, clock_quality) \
         VALUES ($1::uuid, $2::uuid, 'active_power', '2030-01-15T10:00:00Z', \
                 '2030-01-15T10:00:01Z', 7, 'sim://p10.13', 'GOOD', 1.25, 'MW', \
                 'SYNCHRONIZED')"
            .to_string(),
        vec![tenant_text.clone(), asset_uuid.to_string()],
    )
    .expect("the ORIGINAL v1 telemetry row inserts cleanly under the extended PK");
    let conflict = with_tenant(
        &ingest_pool,
        &tenant_text,
        "INSERT INTO telemetry \
             (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
              source_id, quality, value_num, unit, clock_quality) \
         VALUES ($1::uuid, $2::uuid, 'active_power', '2030-01-15T10:00:00Z', \
                 '2030-01-15T10:00:02Z', 7, 'sim://p10.13', 'GOOD', 9.99, 'MW', \
                 'SYNCHRONIZED')"
            .to_string(),
        vec![tenant_text.clone(), asset_uuid.to_string()],
    );
    assert!(
        conflict.is_err(),
        "a conflicting re-ingest of the same identity is a duplicate-key refusal \
         (telemetry_pkey, extended by 0012, still enforces it)"
    );

    // (e) 0012's correction surface: a versioned correction is a NEW row
    // (the original is never mutated), carrying full metadata.
    let original_rows = test_runtime().block_on(tenant_rows(
        &app_pool,
        &tenant_text,
        "SELECT telemetry_id FROM telemetry \
         WHERE asset_id = $1::uuid AND tenant_id = $2::uuid AND source_sequence = 7 \
           AND quantity = 'active_power' AND telemetry_version = 1"
            .to_string(),
        vec![asset_uuid.to_string(), tenant_text.clone()],
    ))
    .expect("the original telemetry row is readable back");
    let original_id: sqlx::types::Uuid = original_rows[0]
        .try_get("telemetry_id")
        .expect("telemetry_id column");
    with_tenant(
        &ingest_pool,
        &tenant_text,
        "INSERT INTO telemetry \
             (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
              source_id, quality, value_num, unit, clock_quality, telemetry_version, \
              correction_of, correction_reason, corrected_by, corrected_at) \
         VALUES ($1::uuid, $2::uuid, 'active_power', '2030-01-15T10:00:00Z', \
                 '2030-01-15T11:00:00Z', 7, 'sim://p10.13', 'ESTIMATED', 1.20, 'MW', \
                 'SYNCHRONIZED', 2, $3::uuid, 'sensor recalibration', 'svc:p10.13', now())"
            .to_string(),
        vec![
            tenant_text.clone(),
            asset_uuid.to_string(),
            original_id.to_string(),
        ],
    )
    .expect("the v2 correction row inserts with full metadata under the extended PK");

    // The original row is UNCHANGED: still version 1, still its value, no
    // correction metadata — the correction appended, never mutated.
    let original_state_rows = test_runtime().block_on(tenant_rows(
        &app_pool,
        &tenant_text,
        "SELECT telemetry_version, value_num, correction_of FROM telemetry \
         WHERE telemetry_id = $1::uuid AND asset_id = $2::uuid AND tenant_id = $3::uuid"
            .to_string(),
        vec![
            original_id.to_string(),
            asset_uuid.to_string(),
            tenant_text.clone(),
        ],
    ))
    .expect("the original row is re-read");
    let original_version: i32 = original_state_rows[0]
        .try_get("telemetry_version")
        .expect("version column");
    let original_value: f64 = original_state_rows[0].try_get("value_num").expect("value column");
    let original_correction_of: Option<sqlx::types::Uuid> = original_state_rows[0]
        .try_get("correction_of")
        .expect("correction_of column");
    assert_eq!(original_version, 1, "the original row keeps version 1");
    assert!(
        (original_value - 1.25).abs() < f64::EPSILON,
        "the original row's value is never mutated by the correction"
    );
    assert!(
        original_correction_of.is_none(),
        "the original row carries no correction metadata"
    );

    // A v2 row WITHOUT the mandatory metadata is refused by the 0012 CHECK
    // (corrections carry reason + actor, mandatory).
    let metadata_less = with_tenant(
        &ingest_pool,
        &tenant_text,
        "INSERT INTO telemetry \
             (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
              source_id, quality, value_num, unit, clock_quality, telemetry_version) \
         VALUES ($1::uuid, $2::uuid, 'active_power', '2030-01-15T10:00:00Z', \
                 '2030-01-15T12:00:00Z', 7, 'sim://p10.13', 'GOOD', 1.10, 'MW', \
                 'SYNCHRONIZED', 2)"
            .to_string(),
        vec![tenant_text.clone(), asset_uuid.to_string()],
    );
    assert!(
        metadata_less.is_err(),
        "a version-2 row without correction metadata is refused by telemetry_correction_metadata"
    );

    // (f) The no-DELETE/no-UPDATE grant matrix across the chain (layer 1 of
    // the append-only discipline; layer 2 — the triggers — is test 4).
    test_runtime().block_on(async {
        for table in APPEND_ONLY_TABLES {
            let delete_grant: bool = sqlx::query_scalar(
                "SELECT has_table_privilege('flexcore_app', $1::text, 'DELETE')",
            )
            .bind(table)
            .fetch_one(&app_pool)
            .await
            .expect("privilege probe");
            assert!(!delete_grant, "flexcore_app must hold NO DELETE on {table}");
            let update_grant: bool = sqlx::query_scalar(
                "SELECT has_table_privilege('flexcore_app', $1::text, 'UPDATE')",
            )
            .bind(table)
            .fetch_one(&app_pool)
            .await
            .expect("privilege probe");
            assert!(
                !update_grant,
                "flexcore_app must hold NO table-level UPDATE on {table}"
            );
        }
        // telemetry: flexcore_app is a READER (0011) and nothing else.
        let app_telemetry_insert: bool = sqlx::query_scalar(
            "SELECT has_table_privilege('flexcore_app', 'telemetry', 'INSERT')",
        )
        .fetch_one(&app_pool)
        .await
        .expect("privilege probe");
        assert!(
            !app_telemetry_insert,
            "flexcore_app must not INSERT telemetry (flexcore_ingest is the only INSERT path)"
        );
        let app_telemetry_delete: bool = sqlx::query_scalar(
            "SELECT has_table_privilege('flexcore_app', 'telemetry', 'DELETE')",
        )
        .fetch_one(&app_pool)
        .await
        .expect("privilege probe");
        assert!(!app_telemetry_delete, "flexcore_app holds NO DELETE on telemetry");
        // dispatch_commands: the COLUMN-LEVEL exception is EXACTLY the two
        // transition columns (0016); content columns stay unwritable.
        for column in ["state", "superseded_by_command_id"] {
            let column_update: bool = sqlx::query_scalar(
                "SELECT has_column_privilege('flexcore_app', 'dispatch_commands', $1::text, 'UPDATE')",
            )
            .bind(column)
            .fetch_one(&app_pool)
            .await
            .expect("column privilege probe");
            assert!(column_update, "flexcore_app holds the column-level UPDATE on {column}");
        }
        for column in ["command_payload", "schedule_revision", "valid_until_millis"] {
            let column_update: bool = sqlx::query_scalar(
                "SELECT has_column_privilege('flexcore_app', 'dispatch_commands', $1::text, 'UPDATE')",
            )
            .bind(column)
            .fetch_one(&app_pool)
            .await
            .expect("column privilege probe");
            assert!(
                !column_update,
                "flexcore_app holds NO UPDATE on the content column {column}"
            );
        }
    });

    drop_database(&admin_url, &temp_url);
}

// --- 4. Append-only and gate-ordering invariants, verified BY SQL BEHAVIOUR -------

#[test]
fn append_only_and_gate_ordering_invariants_hold_across_the_chain() {
    let Some(admin_url) =
        require_db("append_only_and_gate_ordering_invariants_hold_across_the_chain")
    else {
        return;
    };
    let temp_url = temp_database(&admin_url, "invariants");
    let store = FlexCoreStore::connect(&temp_url).expect("connect");
    store.run_migrations().expect("migrations apply");
    ensure_test_roles(store.pool());
    let tenant_uuid = fixture_uuid(0xC1);
    let (ctx, _portfolio, _site, _asset) = seed_registry_chain(&store, tenant_uuid);
    let tenant_text = ctx.tenant_id().uuid().to_string();
    let owner_pool = test_runtime().block_on(PgPool::connect(&temp_url)).expect("owner pool");
    let app_pool = test_runtime()
        .block_on(PgPool::connect(&url_as(
            &temp_url,
            "flexcore_app",
            "flexcore_app_test_only",
        )))
        .expect("app pool");

    // (a) 0008 layer 2: the trigger refuses UPDATE and DELETE for the OWNER
    // too (grants cannot bind the owner; the trigger does — the recorded
    // three-layer enforcement's second layer). The FORCE-RLS row is only
    // reachable with the tenant context set, so the attempts run under it.
    let audit_id = fixture_uuid(0xC9);
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO audit_records \
             (audit_id, tenant_id, actor, action, object_type, object_id, event_time, \
              correlation_id) \
         VALUES ($1::uuid, $2::uuid, 'svc:p10.13', 'registry.create', 'asset', $3, now(), \
                 'p10.13-inv-1')"
            .to_string(),
        vec![audit_id.to_string(), tenant_text.clone(), audit_id.to_string()],
    )
    .expect("the audit row seeds");
    let owner_update = with_tenant(
        &owner_pool,
        &tenant_text,
        "UPDATE audit_records SET action = 'tampered' WHERE audit_id = $1::uuid".to_string(),
        vec![audit_id.to_string()],
    );
    assert!(
        owner_update.is_err(),
        "the 0008 trigger refuses UPDATE for the OWNER (append-only layer 2)"
    );
    let owner_delete = with_tenant(
        &owner_pool,
        &tenant_text,
        "DELETE FROM audit_records WHERE audit_id = $1::uuid".to_string(),
        vec![audit_id.to_string()],
    );
    assert!(
        owner_delete.is_err(),
        "the 0008 trigger refuses DELETE for the OWNER (append-only layer 2)"
    );

    // (b) 0018's one-ACTIVE-term rule: a second ACTIVE term for the same
    // device identity is refused by the partial unique index; rotation
    // after REVOKED appends legally.
    let device_id = format!("edge-p10.13-{:x}", run_key());
    let insert_edge_term = |status: &str, seq: i64| {
        with_tenant(
            &owner_pool,
            &tenant_text,
            format!(
                "INSERT INTO edge_device_registry \
                     (device_id, tenant_id, site_id, enrollment_seq, status, \
                      enrolled_at_unix_millis) \
                 SELECT $1::text, s.tenant_id, s.site_id, {seq}, '{status}', 1700000000000 \
                 FROM sites s WHERE s.tenant_id = $2::uuid LIMIT 1"
            ),
            vec![device_id.clone(), tenant_text.clone()],
        )
    };
    insert_edge_term("ACTIVE", 1)
        .expect("the first ACTIVE term enrolls");
    let second_active = insert_edge_term("ACTIVE", 2);
    assert!(
        second_active.is_err(),
        "a second ACTIVE term for the same device is refused (one-ACTIVE-term index)"
    );
    insert_edge_term("REVOKED", 2)
        .expect("a REVOKED term appends legally");
    insert_edge_term("ACTIVE", 3)
        .expect("rotation after REVOKED appends a legal new ACTIVE term");

    // (c) 0016's database defence-in-depth keeps the §14.1 gate ORDER: no
    // UPDATE ever moves a row into ISSUED except from APPROVED. Seed the
    // schedule anchor (0013 result + 0014 active revision — the P3.22
    // identity scheme the command FK requires), then the command, then
    // drive the edges by raw UPDATE (the schema-level half of P10.15's
    // bypass analysis).
    let command_id = fixture_uuid(0xCA);
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO schedule_results \
             (candidate_id, tenant_id, portfolio_id, site_id, status, problem_class, \
              solver_name, solver_version, provider_id, provider_version, objective_value, \
              objective_units, input_snapshot_id, grid_model_version, grid_model_site_id, \
              grid_model_site_reference, schedule_proposed_key, record_payload) \
         SELECT $1::uuid, p.tenant_id, p.portfolio_id, s.site_id, 'optimal', 'lp', 'highs', \
                '1.15.1', 'tier1-python-worker', '0.1.0', -42.0, 'currency-units', \
                'schedule_input_snapshot:p10.13', 'grid-model-v1', s.site_id, \
                'site-grid-model-reference-v1', 'schedule_proposed:p10.13', \
                '{\"status\":\"optimal\"}'::jsonb \
         FROM portfolios p \
         JOIN sites s ON s.portfolio_id = p.portfolio_id AND s.tenant_id = p.tenant_id \
         WHERE p.tenant_id = $2::uuid LIMIT 1"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    )
    .expect("the 0013 schedule-result candidate seeds");
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO schedule_revisions \
             (schedule_id, revision, tenant_id, portfolio_id, site_id, horizon_start_millis, \
              horizon_end_millis, roll_trigger, state, candidate_id, schedule_payload) \
         SELECT $1::uuid, 1, $2::uuid, p.portfolio_id, s.site_id, 1700000000000, 1700086400000, \
                'initial_solve', 'active', $1::uuid, '{\"revision\":1}'::jsonb \
         FROM portfolios p \
         JOIN sites s ON s.portfolio_id = p.portfolio_id AND s.tenant_id = p.tenant_id \
         WHERE p.tenant_id = $2::uuid LIMIT 1"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    )
    .expect("the active 0014 schedule revision seeds");
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO dispatch_commands \
             (command_id, tenant_id, portfolio_id, site_id, asset_id, schedule_id, \
              schedule_revision, valid_until_millis, state, command_payload) \
         SELECT $1::uuid, p.tenant_id, p.portfolio_id, s.site_id, a.asset_id, $1::uuid, 1, \
                1700000200000, 'PROPOSED', '{\"target_p_mw\":1.0,\"units\":\"MW\"}'::jsonb \
         FROM portfolios p \
         JOIN sites s ON s.portfolio_id = p.portfolio_id AND s.tenant_id = p.tenant_id \
         JOIN assets a ON a.site_id = s.site_id AND a.tenant_id = s.tenant_id \
         WHERE p.tenant_id = $2::uuid LIMIT 1"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    )
    .expect("the command seeds as PROPOSED");

    // Gate-skipping shortcuts are refused at the DATABASE level (the frozen
    // edge table, encoded in 0016): PROPOSED → ISSUED and PROPOSED →
    // APPROVED are not legal edges — no command reaches ISSUED or APPROVED
    // except through the frozen chain (the schema-level statement P10.15
    // consumes).
    let proposed_to_issued = with_tenant(
        &app_pool,
        &tenant_text,
        "UPDATE dispatch_commands SET state = 'ISSUED' \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = 'PROPOSED'"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        proposed_to_issued.is_err(),
        "PROPOSED → ISSUED is refused: no command reaches ISSUED without APPROVED (schema level)"
    );
    let proposed_to_approved = with_tenant(
        &app_pool,
        &tenant_text,
        "UPDATE dispatch_commands SET state = 'APPROVED' \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = 'PROPOSED'"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        proposed_to_approved.is_err(),
        "PROPOSED → APPROVED is refused: the authorization gate sits behind validation and feasibility"
    );

    // The legal chain walks, ONE transition per UPDATE (the
    // one-active-transition invariant), each gate-class step journalled
    // with its gate and evidence.
    let mut current_state = "PROPOSED";
    for (to_state, gate, evidence) in [
        ("VALIDATED", Some("validation"), "evidence:validation-p10.13"),
        ("FEASIBLE", Some("feasibility"), "evidence:feasibility-p10.13"),
        ("APPROVED", Some("authorization"), "evidence:authorization-p10.13"),
    ] {
        let from_state = current_state;
        let state_walk = with_tenant(
            &app_pool,
            &tenant_text,
            format!(
                "UPDATE dispatch_commands SET state = '{to_state}' \
                 WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = '{from_state}'"
            ),
            vec![command_id.to_string(), tenant_text.clone()],
        );
        assert!(
            state_walk.is_ok(),
            "the frozen edge {from_state} → {to_state} is admitted (one transition per UPDATE)"
        );
        let gate_fragment = match gate {
            Some(gate) => format!("'{gate}'::text"),
            None => "NULL".to_string(),
        };
        let journal = with_tenant(
            &app_pool,
            &tenant_text,
            format!(
                "INSERT INTO dispatch_command_transitions \
                     (command_id, tenant_id, from_state, to_state, actor, at_unix_millis, \
                      gate, evidence_reference) \
                 SELECT command_id, tenant_id, '{from_state}', '{to_state}', 'svc:p10.13', \
                        1700000100000, {gate_fragment}, '{evidence}' \
                 FROM dispatch_commands \
                 WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            ),
            vec![command_id.to_string(), tenant_text.clone()],
        );
        journal
            .unwrap_or_else(|error| panic!("the journal row for {from_state} → {to_state} inserts with its gate and evidence: {error}"));
        current_state = to_state;
    }

    // APPROVED → ISSUED is a legal edge; the ISSUED journal row REQUIRES an
    // evidence reference (0016's evidence rule — §14.3 field-16 discipline).
    let issued = with_tenant(
        &app_pool,
        &tenant_text,
        "UPDATE dispatch_commands SET state = 'ISSUED' \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = 'APPROVED'"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        issued.is_ok(),
        "APPROVED → ISSUED is the only route into ISSUED and is admitted"
    );
    let issued_journal_without_evidence = with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO dispatch_command_transitions \
             (command_id, tenant_id, from_state, to_state, actor, at_unix_millis, gate) \
         SELECT command_id, tenant_id, 'APPROVED', 'ISSUED', 'svc:p10.13', 1700000150000, NULL \
         FROM dispatch_commands \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        issued_journal_without_evidence.is_err(),
        "the ISSUED journal row without evidence is refused (0016 evidence rule)"
    );
    with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO dispatch_command_transitions \
             (command_id, tenant_id, from_state, to_state, actor, at_unix_millis, \
              evidence_reference) \
         SELECT command_id, tenant_id, 'APPROVED', 'ISSUED', 'svc:p10.13', 1700000150000, \
                'evidence:issue-signature-p10.13' \
         FROM dispatch_commands \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    )
    .expect("the ISSUED journal row inserts with its evidence reference");

    // A gate-class transition WITHOUT its gate named is refused (0016's
    // gate rule: validation, feasibility, and authorization are distinct,
    // §14.1).
    let gateless = with_tenant(
        &app_pool,
        &tenant_text,
        "INSERT INTO dispatch_command_transitions \
             (command_id, tenant_id, from_state, to_state, actor, at_unix_millis, \
              evidence_reference) \
         SELECT command_id, tenant_id, 'PROPOSED', 'VALIDATED', 'svc:p10.13', 1700000160000, \
                'evidence:gateless-p10.13' \
         FROM dispatch_commands \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        gateless.is_err(),
        "a gate-class transition without its gate named is refused (validation/feasibility/authorization are distinct)"
    );

    // The journal is INSERT-only for the runtime role (grant layer; the
    // 0016 journal has NO trigger — recorded honestly in the record §6 as
    // a grant-layer-only immutability, unlike 0008/0018's trigger layer).
    let journal_update = with_tenant(
        &app_pool,
        &tenant_text,
        "UPDATE dispatch_command_transitions SET reason = 'rewritten' \
         WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            .to_string(),
        vec![command_id.to_string(), tenant_text.clone()],
    );
    assert!(
        journal_update.is_err(),
        "the journal UPDATE is refused for the runtime role (no grant; grant-layer-only guard)"
    );

    drop_database(&admin_url, &temp_url);
}
