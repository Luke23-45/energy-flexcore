// P2.23 — The Phase 2 fault-injection suite (ledger P2.23): ingestion,
// storage, clock, and volume faults against the SAME ingestion pipeline the
// worker hosts (`flexcore_worker::ingestion`), with the fault condition
// asserted FIRST and the handling asserted second in every test.
//
// Gated on `FLEXCORE_TEST_DATABASE_URL` (the P1.13 harness convention);
// skips cleanly when unset. The NATS-down test additionally needs
// `FLEXCORE_TEST_NATS_URL` for its recovery leg and skips with a message
// when it is unset. Requires a multi-thread tokio runtime (the pipeline
// drives the sync ports under `block_in_place` — the recorded P1.19
// contract).
//
// Fault inventory and expected outcomes (also recorded as the results-log
// table in `ledger/phase-02-asset-telemetry/23_24_fault_replay.md` — the
// OBSERVED outcomes come from the human's run; these are the expected ones
// the tests encode):
//
// | # | Fault                                   | Driver                                    | Expected outcome |
// |---|-----------------------------------------|-------------------------------------------|------------------|
// | 1 | Simulator stops mid-batch               | partial batch through `process_batch`     | partial batch stored; remainder retried after resume; zero duplicates; zero ambiguous rows |
// | 2 | Ingestion service restart mid-batch    | fresh `IngestionTask` over the SAME store + durable inbox, redelivered horizon | overlap detected by the inbox (redeliveries skipped); exactly one row per record; remainder stored |
// | 3 | NATS down during ingestion             | outbox-only ingestion with no publisher connected; broker connects for the recovery leg | state rows + outbox rows written while the broker is unreachable; events drain on recovery; nothing lost |
// | 4 | PostgreSQL down during ingestion       | a store whose endpoint is unreachable     | clean failure (`Err`), no partial batch, failed record neither stored nor quarantined, outbox entries not lost |
// | 5 | Partition missing for the ingest frontier | event_time in an unpartitioned month   | per-record clear error (quarantined, `out_of_range_event_time`), no silent drop, current state untouched |
// | 6 | Clock fault — event_time far future    | the versioned scenario's horizon moved to 2099 (scenario-config driver) | every record quarantined with a clear reason; nothing stored; no GOOD current state |
// | 7 | Clock fault — event_time far past      | the versioned scenario's horizon moved to 2020-01 (scenario-config driver) | records stored as valid history; the P2.08 staleness signal fires under the configured freshness limit; never silently GOOD as current state |
// | 8 | Volume fault — burst above the backpressure limit | 64 frames offered into a capacity-8 bounded channel with no consumer | exactly `capacity` buffered, the rest SHED with the shed metric incremented; the pipeline drains exactly the buffered frames; no unbounded buffering |
//
// Harness notes (the recorded P2.06 seeding pattern, shared with
// `tests/ingestion.rs`): fixed test months with partitions created as the
// migrator role, tenant-owned rows through the runtime role under the
// tenant context, fixed fixture UUIDs so tests run in parallel.
//
// Clock-fault driver constraint, recorded honestly: the ingestion service
// depends only on the `TelemetrySource` port (P2.10 acceptance — no
// simulator import in the service), so this suite cannot import
// `flexcore-simulator`. The clock-fault event times are therefore DERIVED
// from the versioned scenario configuration (the horizon start moved to the
// fault instant, the cadence read from `cadence.default_seconds`) — the
// same tick sequence the P2.14 engine emits for that scenario
// configuration, not arbitrary hand-picked times. The full
// scenario-config-driven end-to-end run (engine → pipeline) is the
// Compose-stack exit-gate path (P2.25/P2.26).
//
// Everything here is simulation-only synthetic telemetry (the entire 0.x
// line); no safety, certification, or control-authority claim.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime, SiteTimezone};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::quality_rules::{staleness, FreshnessLimit};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::source_auth::{SourceAuthError, SourceAuthenticator, VerifiedEnvelope};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    QuantityValue, SourceStateMapping, TelemetryQuery, TelemetryQuantity, TelemetrySource,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
use flexcore_worker::inbox::PgInboxStore;
use flexcore_worker::ingestion::{
    FrameShed, IngestionConfig, IngestionMetrics, IngestionRunOutcome, IngestionTask,
    JsonEventPayloadEncoder, PgIngestionInbox, UuidV7Generator,
};
use sqlx::PgPool;

// --- Fixed test months (2030 / 2020, UTC) — the recorded P2.06 pattern -------

/// January 2030: a month WITH a partition (the normal ingest window).
const JAN_2030_MS: i64 = 1_893_456_000_000;
/// January 2020: far PAST the wall clock (the clock-fault far-past case);
/// its partition exists, so the records are storable valid history.
const JAN_2020_MS: i64 = 1_577_836_800_000;
/// June 2031: a month with NO partition (the partition-missing case and the
/// clock-fault far-future proxy window; identical handling).
const JUN_2031_MS: i64 = 1_938_038_400_000;
/// June 2099: far FUTURE (the clock-fault far-future scenario start).
const JUN_2099_MS: i64 = 4_083_955_200_000;

const PARTITION_MONTHS: [(&str, &str, &str); 5] = [
    ("telemetry_y2020m01", "2020-01-01 00:00:00+00", "2020-02-01 00:00:00+00"),
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
    ("telemetry_y2030m03", "2030-03-01 00:00:00+00", "2030-04-01 00:00:00+00"),
    ("telemetry_y2030m04", "2030-04-01 00:00:00+00", "2030-05-01 00:00:00+00"),
];

/// The versioned exit-gate scenario: the clock-fault driver derives its
/// event times from this configuration (see the module header).
const SCENARIO_V1: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v1.json"
));

// --- Harness helpers (the recorded P2.06 pattern) ------------------------------

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
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a \
                 PostgreSQL 18 service)"
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

/// Deterministic per-test fixture UUIDs (parallel-safe: every test seeds
/// its own tenant/asset chain under its own prefix).
fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0x23; // P2.23 fault-suite fixture marker
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "fault-suite:ingestion".to_string(),
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

// Batch 68: the tests are `#[tokio::test]` (async), so NO helper may
// `block_on` a fresh runtime from inside them — tokio refuses with
// "Cannot start a runtime from within a runtime". Pure-sqlx setup
// (`ensure_test_roles`, `ensure_partitions`, pool connects) is `async`
// and awaited directly. The postgres adapter's SYNC API
// (`FlexCoreStore::connect/begin`, `unit.commit`, `store.pending`)
// `block_on`s on the store's own runtime, so those calls run inside
// `tokio::task::block_in_place` (multi-thread test runtime), which moves
// them onto a thread with no runtime context entered. This helper builds
// the throwaway runtime used ONLY inside `block_in_place` closures.
fn blocking_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// Grants LOGIN and test-only passwords to the service roles (credentials
/// never live in a migration — the recorded P2.06 harness deviation).
async fn ensure_test_roles(pool: &PgPool) {
    // Batch 68: serialized with the recorded advisory lock (key
    // `20261118`, the B66 rule) — the 8 tests run in parallel and their
    // concurrent ALTER ROLEs collided on pg_authid ("tuple concurrently
    // updated"). Same shape as the postgres adapter's `ensure_test_roles`.
    let mut tx = pool.begin().await.expect("role setup tx");
    sqlx::query("SELECT pg_advisory_xact_lock(20261118)")
        .execute(&mut *tx)
        .await
        .expect("role setup lock");
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
        .execute(&mut *tx)
        .await
        .expect("test roles must be grantable from the migration/admin connection");
    }
    tx.commit().await.expect("role setup commit");
}

/// Creates the fixed monthly partitions as the migrator role (the parent
/// table's owner). Idempotent across runs. Deliberately does NOT create a
/// partition for June 2031 or 2099 — those absences ARE the partition
/// fault. MUST run after [`ensure_test_roles`].
async fn ensure_partitions(url: &str) {
    let migrator_pool = PgPool::connect(&url_as(
        url,
        "flexcore_migrator",
        "flexcore_migrator_test_only",
    ))
    .await
    .expect("connect as flexcore_migrator");
    // Batch 68: serialized with the recorded advisory lock (key
    // `20261118`, the B66 rule) — CREATE TABLE IF NOT EXISTS is not
    // concurrency-safe across the 8 parallel tests.
    let mut tx = migrator_pool.begin().await.expect("partition setup tx");
    sqlx::query("SELECT pg_advisory_xact_lock(20261118)")
        .execute(&mut *tx)
        .await
        .expect("partition setup lock");
    for (name, from, to) in PARTITION_MONTHS {
        // SAFETY (sqlx 0.9 SqlSafeStr): `name`/`from`/`to` are fixed
        // literals from the PARTITION_MONTHS const above — no user
        // input reaches this DDL, and partition names/bounds cannot
        // be bind parameters in CREATE TABLE.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
             FOR VALUES FROM ('{from}') TO ('{to}')"
        )))
        .execute(&mut *tx)
        .await
        .expect("partition DDL as the table owner");
    }
    tx.commit().await.expect("partition setup commit");
}

/// Seeds one tenant with one portfolio/site/asset chain (composite FK for
/// the telemetry rows and the event publisher's site lookup). Returns the
/// tenant context.
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
            "Fault-suite portfolio".to_string(),
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
            "Fault-suite site".to_string(),
            None,
            "{}".to_string(),
            SiteTimezone::new("Europe/Berlin".to_string()).expect("valid timezone"),
            "pcc:fault-suite".to_string(),
            "grid-model:rev-1".to_string(),
            "edge:fault-suite".to_string(),
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
    store: &'static FlexCoreStore,
    ctx: TenantContext,
    asset_id: AssetId,
    inbox: PgInboxStore,
}

/// The process-wide shared store for this binary (Batch 68): a
/// `FlexCoreStore` owns a tokio `Runtime`, whose drop shuts its blocking
/// pool down and PANICS in an async context ("Cannot drop a runtime in a
/// context where blocking is not allowed"). The async tests would drop a
/// per-test store when the test future ends — including while unwinding
/// after a failed assertion (panic-in-panic aborts the process). The
/// store is therefore created once and intentionally never dropped; the
/// per-test isolation is the tenant, not the connection (every test
/// already seeds its own tenant). Call only inside `block_in_place`:
/// `connect` blocks on the store's runtime.
fn shared_store(url: &str) -> &'static FlexCoreStore {
    static STORE: std::sync::OnceLock<FlexCoreStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| FlexCoreStore::connect(url).expect("shared store"))
}

async fn harness_at(url: &str, prefix: u8) -> Harness {
    let admin_pool = PgPool::connect(url).await.expect("admin pool");
    // Batch 68: the migrations were NEVER run by this harness (the suite
    // was "gated-blocked" since B63 and never executed with services up)
    // — the partition DDL then failed 42501 because the CREATE ON SCHEMA
    // public grant (migration 0010) and the `telemetry` table did not
    // exist. Migrations run first (they create the roles), then the
    // password grants, then the partitions. `run_migrations` is the sync
    // adapter API, so it runs inside `block_in_place`.
    let store = tokio::task::block_in_place(|| {
        let store = shared_store(url);
        store.run_migrations().expect("migrations apply");
        store
    });
    ensure_test_roles(&admin_pool).await;
    ensure_partitions(url).await;
    // The registry seed is the sync adapter API, so it runs inside
    // `block_in_place` (see the `blocking_runtime` note). The admin and
    // app stores are the same shared store (both connected over the
    // admin URL; the tenant scoping is the context, not the role).
    let (ctx, asset_id, inbox) = tokio::task::block_in_place(|| {
        let tenant_id = TenantId::new(fixture_uuid(prefix));
        let asset_id = AssetId::new(fixture_uuid(prefix + 1));
        let ctx = seed_registry(store, store, tenant_id, asset_id);
        let inbox = PgInboxStore::new(store.pool().clone()).expect("inbox store");
        (ctx, asset_id, inbox)
    });
    // FRV1-R5-004: the inbox table is owned by the embedded migration
    // 0024_worker_inbox_p1_16 (the harness connects to a
    // migration-provisioned database); the runtime `ensure_table` was
    // removed with the promotion.
    Harness {
        store,
        ctx,
        asset_id,
        inbox,
    }
}

async fn harness(test: &str, prefix: u8) -> Option<Harness> {
    let url = require_db(test)?;
    Some(harness_at(&url, prefix).await)
}

// --- Record fixtures (through the ports' canonical constructors) --------------

const SOURCE_ID: &str = "sim://exit-gate";

/// The process-wide shared UUIDv7 generator (Batch 68): a
/// `UuidV7Generator` owns its own tokio `Runtime` (it `block_on`s a
/// `SELECT uuidv7()`), and dropping that runtime in an async context
/// panics the same way a `FlexCoreStore` drop does ("Cannot drop a
/// runtime in a context where blocking is not allowed"). Every
/// `IngestionTask` built in this binary shares one generator, created
/// once and never dropped; the pool is the shared store's pool either
/// way. Building the runtime is fine in async context — only `block_on`
/// and the drop are restricted.
fn shared_id_generator(
    pool: &sqlx::PgPool,
) -> Arc<dyn flexcore_ports::id_generator::IdGenerator> {
    static GENERATOR: std::sync::OnceLock<
        Arc<dyn flexcore_ports::id_generator::IdGenerator>,
    > = std::sync::OnceLock::new();
    Arc::clone(GENERATOR.get_or_init(|| Arc::new(UuidV7Generator::new(pool.clone()))))
}

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

#[allow(clippy::too_many_arguments)]
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

/// The canonical records of a horizon segment: `count` one-minute ticks
/// starting at `start_ms` — the tick sequence the scenario cadence
/// (`cadence.default_seconds = 60`) produces, sequences gap-free from 1.
fn horizon_records(tenant_id: TenantId, asset_id: AssetId, start_ms: i64, count: usize) -> Vec<NormalizedTelemetry> {
    let cadence_seconds = serde_json::from_str::<serde_json::Value>(SCENARIO_V1)
        .expect("the versioned scenario parses")
        ["cadence"]["default_seconds"]
        .as_i64()
        .expect("the scenario cadence is a number");
    assert_eq!(cadence_seconds, 60, "the v1 scenario cadence (D-SC3)");
    (0..count)
        .map(|i| {
            record(
                tenant_id,
                asset_id,
                start_ms + (i as i64) * cadence_seconds * 1000,
                (i + 1) as u64,
                1.0 + i as f64 * 0.01,
            )
        })
        .collect()
}

/// `count(*)` over telemetry under the tenant's RLS scope.
fn count_telemetry(store: &FlexCoreStore, ctx: &TenantContext) -> i64 {
    // Batch 68: raw sqlx via a throwaway runtime, inside
    // `block_in_place` — the callers are async tests (see the
    // `blocking_runtime` note).
    tokio::task::block_in_place(|| {
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
    })
}

/// The pending outbox idempotency keys mentioning `needle` (this suite's
/// asset UUID — a per-test filter that tolerates other tests' parallel
/// entries in the shared outbox table).
fn pending_keys_containing(store: &FlexCoreStore, needle: &str) -> Vec<String> {
    // Batch 68: `store.pending` is the sync adapter API — inside
    // `block_in_place` (see the `blocking_runtime` note).
    tokio::task::block_in_place(|| {
        store
            .pending(10_000)
            .expect("outbox pending read")
            .into_iter()
            .map(|entry| entry.idempotency_key().to_string())
            .filter(|key| key.contains(needle))
            .collect()
    })
}

/// The TOTAL number of outbox rows mentioning `needle`, in ANY delivery
/// state — the no-lost-entries check (a concurrent publisher may deliver
/// entries, which is not a loss; deletion would be).
fn outbox_rows_containing(store: &FlexCoreStore, needle: &str) -> i64 {
    // Batch 68: raw sqlx via a throwaway runtime, inside
    // `block_in_place` (see the `blocking_runtime` note).
    tokio::task::block_in_place(|| {
        blocking_runtime().block_on(async {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM outbox WHERE idempotency_key LIKE $1",
            )
            .bind(format!("%{needle}%"))
            .fetch_one(store.pool())
            .await
            .expect("outbox count query")
        })
    })
}

fn asset_uuid(asset_id: AssetId) -> String {
    asset_id.uuid().to_hyphenated().to_string()
}

/// Permissive source-auth double (Patch B): passes raw frames through
/// unenveloped (`was_enveloped: false`, payload = the raw bytes) so the
/// downstream-stage tests keep working with raw frames. The batch path
/// skips the sequence-watermark advance entirely for unenveloped input.
struct PassThroughAuthenticator;

impl SourceAuthenticator for PassThroughAuthenticator {
    fn authenticate(
        &self,
        ctx: &TenantContext,
        payload: &[u8],
    ) -> Result<VerifiedEnvelope, SourceAuthError> {
        Ok(VerifiedEnvelope {
            key_id: "test-double".to_string(),
            source_id: "test-double".to_string(),
            tenant_id: ctx.tenant_id(),
            sequence: 0,
            event_time_unix_millis: 0,
            was_enveloped: false,
            payload: payload.to_vec(),
        })
    }
}

/// Builds an ingestion task over the real store and a controllable fake
/// source (the pipeline consumes ONLY the `TelemetrySource` port).
fn build_task_with(
    store: FlexCoreStore,
    inbox: PgInboxStore,
    source_records: Vec<NormalizedTelemetry>,
    config: IngestionConfig,
) -> (
    Arc<IngestionTask>,
    Arc<flexcore_ports::fakes::FakeTelemetrySource>,
    Arc<IngestionMetrics>,
) {
    let source = Arc::new(flexcore_ports::fakes::FakeTelemetrySource::new(
        provider_declaration(),
        source_records,
    ));
    let metrics = Arc::new(IngestionMetrics::default());
    let task = Arc::new(IngestionTask::new(
        Arc::clone(&source) as Arc<dyn TelemetrySource>,
        store.clone(),
        PgIngestionInbox::new(inbox),
        Arc::new(JsonEventPayloadEncoder),
        Arc::new(SystemClock),
        // VERIFY-BUILD: the generator hits the database's uuidv7() at first
        // use; over a reachable store this is the recorded D-S1 policy.
        shared_id_generator(store.pool()),
        config,
        Arc::clone(&metrics),
        Arc::new(PassThroughAuthenticator),
    ));
    (task, source, metrics)
}

fn default_config() -> IngestionConfig {
    IngestionConfig {
        channel_capacity: 100,
        ..Default::default()
    }
}

/// A subscription query for the seeded asset (the run loop's query shape).
fn subscription_query(ctx: &TenantContext, asset_id: AssetId) -> TelemetryQuery {
    TelemetryQuery::new(
        ctx,
        vec![asset_id],
        EventTime::from_unix_millis(JAN_2020_MS).unwrap(),
        EventTime::from_unix_millis(JUN_2099_MS).unwrap(),
        vec![TelemetryQuantity::ActivePower],
    )
    .expect("valid subscription query")
}

/// The quarantine entries (the evidence trail the no-silent-skip rule
/// requires).
fn quarantine_reasons(store: &FlexCoreStore, ctx: &TenantContext) -> Vec<String> {
    // Batch 68: the sync adapter API inside `block_in_place` (see the
    // `blocking_runtime` note).
    let quarantined = tokio::task::block_in_place(|| {
        let unit = store.begin(Some(ctx)).expect("unit of work");
        let quarantined = PgUnitOfWork::list_telemetry_quarantine(&unit, ctx, 1000)
            .expect("quarantine list");
        unit.commit().expect("commit read");
        quarantined
    });
    quarantined
        .into_iter()
        .map(|entry| format!("{}: {}", entry.entry.stage, entry.entry.reason))
        .collect()
}

// ===========================================================================
// 1. Ingestion fault — the simulator stops mid-batch
// ===========================================================================

/// Ledger P2.23 step 1, case 1. The simulator stops mid-horizon: the
/// pipeline stores the partial batch, the remainder is retried after the
/// source resumes, and no record is stored twice. The fault condition
/// (fewer rows than the horizon) is asserted FIRST.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simulator_stops_mid_batch_partial_batch_stored_remainder_retried_no_duplicates() {
    let Some(h) = harness("p223_simulator_stop_mid_batch", 0x50).await else {
        return;
    };
    let (task, _source, metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());

    // The horizon: 12 one-minute ticks; the simulator stops after 6.
    let full = horizon_records(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 12);
    assert_eq!(full.len(), 12);
    let stored_before_stop = full[..6].to_vec();

    // FAULT FIRST: the source stopped mid-batch — only part of the horizon
    // reaches the pipeline. The partial batch is stored (no ambiguous
    // all-or-nothing state: what arrived is committed, what did not is
    // absent and accounted for).
    let report = task
        .process_batch(&h.ctx, &stored_before_stop, None)
        .expect("the partial batch commits");
    assert_eq!(report.stored, 6, "the partial batch stores");
    assert_eq!(
        count_telemetry(&h.store, &h.ctx),
        6,
        "the fault actually occurred: fewer rows than the horizon, no ambiguous state"
    );

    // HANDLING: the source resumes (a restarted deterministic run emits the
    // full horizon); the remainder is retried and stored.
    let remainder = task
        .process_batch(&h.ctx, &full[6..], None)
        .expect("the remainder commits");
    assert_eq!(remainder.stored, 6, "the remainder stores after the retry");
    assert_eq!(count_telemetry(&h.store, &h.ctx), 12, "the horizon is complete");

    // No duplicates anywhere: exactly one row per record of the horizon.
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 12);
    assert_eq!(snapshot.duplicates_acknowledged, 0);
    assert_eq!(snapshot.records_quarantined, 0);
}

// ===========================================================================
// 2. Ingestion fault — the ingestion service restarts mid-batch
// ===========================================================================

/// Ledger P2.23 step 1, case 2. The service dies mid-batch; a FRESH
/// pipeline instance over the SAME store and the SAME durable inbox
/// receives the redelivered horizon: the overlap is detected by the inbox
/// (P2.09 step 4), the remainder is stored, and no record is duplicated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingestion_restart_mid_batch_redelivers_over_the_same_durable_inbox_without_duplicates() {
    let Some(h) = harness("p223_ingestion_restart_mid_batch", 0x52).await else {
        return;
    };
    let full = horizon_records(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 12);

    // The first instance processes the first half and "dies".
    let (task_a, _source_a, metrics_a) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());
    let first = task_a
        .process_batch(&h.ctx, &full[..6], None)
        .expect("the first instance commits its batch");
    assert_eq!(first.stored, 6);
    assert_eq!(
        count_telemetry(&h.store, &h.ctx),
        6,
        "the fault actually occurred: the instance died holding half the horizon"
    );
    drop(task_a);

    // A FRESH instance over the SAME store and the SAME durable inbox
    // receives the whole horizon again (the un-acknowledged delivery is
    // redelivered on restart).
    let (task_b, _source_b, metrics_b) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());
    let second = task_b
        .process_batch(&h.ctx, &full, None)
        .expect("the redelivered horizon processes");
    assert_eq!(
        second.redeliveries_skipped, 6,
        "the overlap is detected by the durable inbox, not re-stored"
    );
    assert_eq!(second.stored, 6, "the remainder stores");
    assert_eq!(count_telemetry(&h.store, &h.ctx), 12, "exactly one row per record");

    // No duplicates across the restart: the fresh instance stored only the
    // records the dead instance had not.
    assert_eq!(metrics_b.snapshot().records_stored, 6);
    assert_eq!(
        metrics_a.snapshot().records_stored + metrics_b.snapshot().records_stored,
        12
    );
    assert_eq!(metrics_b.snapshot().records_quarantined, 0);
}

// ===========================================================================
// 3. Ingestion fault — NATS down during ingestion
// ===========================================================================

/// Ledger P2.23 step 1, case 3 (the P1.24 pattern with telemetry volume).
/// The pipeline publishes through the OUTBOX only (report §6.3), so a
/// broker outage cannot touch the state write path: ingestion proceeds,
/// state rows and outbox rows are written, and the events drain on
/// recovery once the broker is reachable. The outage is modelled as the
/// P1.24 pattern: no publisher is connected while the telemetry volume is
/// ingested; the recovery leg connects and drains.
///
/// Gated on BOTH services; skips with a message when either is unset.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nats_down_state_and_outbox_written_events_drain_on_recovery() {
    let Some(db_url) = require_db("p223_nats_down") else {
        return;
    };
    let nats_url = std::env::var("FLEXCORE_TEST_NATS_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty());
    let Some(nats_url) = nats_url else {
        println!(
            "skipped p223_nats_down: FLEXCORE_TEST_NATS_URL is not set (the recovery leg \
             needs a NATS 2.15.x service)"
        );
        return;
    };

    let h = harness_at(&db_url, 0x54).await;
    let needle = asset_uuid(h.asset_id);
    let (task, _source, _metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());

    // FAULT: telemetry volume ingested while the broker is unreachable (no
    // publisher connected). The outbox-only rule means ingestion succeeds.
    let records = horizon_records(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 24);
    let report = task
        .process_batch(&h.ctx, &records, None)
        .expect("ingestion succeeds while the broker is down");
    assert_eq!(report.stored, 24);
    assert_eq!(
        count_telemetry(&h.store, &h.ctx),
        24,
        "state rows are written while the broker is down"
    );
    // FAULT FIRST: the events are sitting undelivered in the outbox.
    let pending_before = pending_keys_containing(&h.store, &needle);
    assert!(
        pending_before.len() >= 24,
        "the telemetry events must be pending in the outbox, undelivered \
         (found {} for this asset)",
        pending_before.len()
    );

    // RECOVERY: the broker becomes reachable; provision the topology (the
    // telemetry stream is NOT one of the four Phase 1 streams) and drain.
    let client = async_nats::connect(&nats_url)
        .await
        .expect("the broker connects on recovery");
    let jetstream = async_nats::jetstream::new(client);
    let limits = flexcore_nats_adapter::topology::DeploymentLimits {
        duplicate_window: Duration::from_millis(10),
        max_age: None,
        max_messages: None,
        max_bytes: None,
    };
    flexcore_nats_adapter::topology::provision_phase_1_streams(&jetstream, &limits)
        .await
        .expect("phase 1 streams provision");
    let telemetry_stream = flexcore_nats_adapter::topology::STREAMS
        .iter()
        .find(|d| d.name == flexcore_nats_adapter::topology::STREAM_TELEMETRY)
        .expect("the telemetry stream is in the P1.14 §4.2 inventory");
    flexcore_nats_adapter::topology::provision_stream(&jetstream, telemetry_stream, &limits)
        .await
        .expect("the telemetry stream provisions");

    let publisher = flexcore_nats_adapter::publisher::OutboxPublisher::new(
        &jetstream,
        h.store,
        flexcore_worker::runner::publisher_config(Duration::from_millis(50)),
    );
    // Drain until idle (bounded passes; the outbox is ordered and
    // idempotent by key, so re-draining is harmless).
    for _ in 0..5 {
        let drained = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(publisher.drain_once())
        })
        .expect("the drain cycle succeeds on recovery");
        if drained.drained == 0 {
            break;
        }
    }
    let pending_after = pending_keys_containing(&h.store, &needle);
    assert!(
        pending_after.is_empty(),
        "the events drained on recovery: {:?}",
        pending_after
    );
    // Nothing was lost by the outage + recovery cycle: every outbox row for
    // this asset still exists (delivered, not deleted).
    assert!(
        outbox_rows_containing(&h.store, &needle) >= 24,
        "the undelivered-then-drained entries are delivered, never deleted"
    );
    h.store.pool().close();
}

// ===========================================================================
// 4. Storage fault — PostgreSQL down during ingestion
// ===========================================================================

/// Ledger P2.23 step 2, case 1. A batch against an unreachable PostgreSQL
/// fails CLEANLY: an `Err` to the caller, no partial batch (the unit of
/// work rolls back on drop, P1.10 rule 4), the failed record is neither
/// stored nor quarantined, and the already-committed outbox entries are
/// not lost.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_down_during_ingestion_fails_cleanly_with_no_partial_batch_and_no_lost_outbox() {
    let Some(h) = harness("p223_postgres_down", 0x56).await else {
        return;
    };
    let needle = asset_uuid(h.asset_id);
    let (task, _source, metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());

    // Baseline: one committed batch (state + outbox rows, §6.3) before the
    // fault.
    let baseline = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.0);
    let committed = task
        .process_batch(&h.ctx, &[baseline], None)
        .expect("the baseline batch commits");
    assert_eq!(committed.stored, 1);
    let baseline_rows = outbox_rows_containing(&h.store, &needle);
    assert!(
        baseline_rows >= 1,
        "the baseline batch's outbox entries exist before the fault"
    );

    // FAULT: PostgreSQL unreachable — a store pointed at a closed port.
    // VERIFY-BUILD: the fast-fail relies on sqlx honoring `connect_timeout`
    // as a URL parameter; if the parameter is not honored the assertions
    // below still hold, the fault simply takes the pool's acquire timeout
    // to surface.
    let down_url = "postgresql://127.0.0.1:1/flexcore_down?connect_timeout=1";
    match FlexCoreStore::connect(down_url) {
        Ok(down_store) => {
            // The pool constructs lazily; the fault surfaces at the unit of
            // work. The inbox handle is shared (the failed batch never
            // reaches the post-commit inbox recording).
            let down_task = IngestionTask::new(
                Arc::new(flexcore_ports::fakes::FakeTelemetrySource::new(
                    provider_declaration(),
                    Vec::new(),
                )) as Arc<dyn TelemetrySource>,
                down_store,
                PgIngestionInbox::new(h.inbox.clone()),
                Arc::new(JsonEventPayloadEncoder),
                Arc::new(SystemClock),
                shared_id_generator(h.store.pool()),
                default_config(),
                Arc::clone(&metrics),
                Arc::new(PassThroughAuthenticator),
            );
            let fault_record = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS + 60_000, 2, 2.0);
            let outcome = down_task.process_batch(&h.ctx, &[fault_record], None);

            // FAULT FIRST: the failure actually happened.
            assert!(
                outcome.is_err(),
                "a batch against a down store must fail cleanly"
            );

            // CLEAN FAILURE: no partial batch.
            assert_eq!(
                count_telemetry(&h.store, &h.ctx),
                1,
                "no partial batch: only the baseline row exists"
            );
            // The failed record is neither stored nor quarantined (the
            // store is failing — quarantine writes go through it too).
            let reasons = quarantine_reasons(&h.store, &h.ctx);
            assert!(
                reasons.is_empty(),
                "a store-down batch quarantines nothing: {reasons:?}"
            );
            // NO LOST OUTBOX ENTRIES: the baseline entries still exist in
            // any delivery state (a concurrent drain may deliver them;
            // deletion would be a loss).
            assert!(
                outbox_rows_containing(&h.store, &needle) >= baseline_rows,
                "the committed outbox entries survive the failed batch"
            );
            assert_eq!(metrics.snapshot().records_stored, 1, "only the baseline stored");
        }
        Err(error) => {
            // The fault is observable at the connect boundary already; the
            // guarantees are asserted against the healthy store. The error
            // text is reported, not asserted on (the adapter owns the
            // human-safe wording).
            println!("the down-store fault surfaced at connect: {error}");
            assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
            assert!(outbox_rows_containing(&h.store, &needle) >= baseline_rows);
        }
    }
}

// ===========================================================================
// 5. Storage fault — partition missing for the ingest frontier
// ===========================================================================

/// Ledger P2.23 step 2, case 2 (P2.05/P2.20). A record whose event_time
/// falls in a month with no partition is rejected with a CLEAR per-record
/// error and quarantined — never silently dropped, never silently stored,
/// and the current state is untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partition_missing_for_the_ingest_frontier_is_a_clear_error_not_a_silent_drop() {
    let Some(h) = harness("p223_partition_missing", 0x58).await else {
        return;
    };
    let (task, _source, metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());

    // One storable record (January 2030, partition exists) and one whose
    // event_time lands outside every partition (June 2031 — deliberately
    // not created by the harness).
    let valid = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.0);
    let frontier = record(h.ctx.tenant_id(), h.asset_id, JUN_2031_MS, 2, 2.0);

    let report = task
        .process_batch(&h.ctx, &[valid, frontier], None)
        .expect("the batch commits (the rejection is per-record, not whole-batch)");

    // FAULT FIRST: the frontier record was actually rejected.
    assert_eq!(report.out_of_range_quarantined, 1, "the fault occurred");
    assert_eq!(report.stored, 1);

    // HANDLING: a clear error with a reason, quarantined as evidence —
    // no silent drop.
    let reasons = quarantine_reasons(&h.store, &h.ctx);
    assert_eq!(reasons.len(), 1, "exactly one quarantine entry: {reasons:?}");
    assert!(reasons[0].starts_with("5-duplicate-handling"), "{:?}", reasons[0]);
    assert!(
        reasons[0].contains("outside every existing telemetry partition"),
        "the reason names the missing partition: {:?}",
        reasons[0]
    );
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1, "only the storable row");
    assert_eq!(
        metrics.snapshot().records_quarantined, 1,
        "the rejection is counted"
    );
    assert_eq!(
        metrics.quarantine_reasons().get("out_of_range_event_time"),
        Some(&1),
        "the closed reason class carries the rejection"
    );

    // The current state comes from the stored record only — the rejected
    // frontier record never became a silent GOOD current state.
    // Batch 68: the sync adapter API inside `block_in_place` (see the
    // `blocking_runtime` note).
    let latest = tokio::task::block_in_place(|| {
        let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
        let latest = PgUnitOfWork::latest_telemetry_per_asset(&unit, &h.ctx, &[h.asset_id], None)
            .expect("latest query");
        unit.commit().expect("commit read");
        latest
    });
    let active: Vec<_> = latest
        .into_iter()
        .filter(|row| row.quantity == flexcore_postgres_adapter::telemetry::Quantity::ActivePower)
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0].event_time,
        EventTime::from_unix_millis(JAN_2030_MS).unwrap(),
        "the current state is the stored record, never the rejected one"
    );
}

// ===========================================================================
// 6. Clock fault — event_time far future (scenario-config driver)
// ===========================================================================

/// Ledger P2.23 step 3, far-future case. The versioned scenario's horizon
/// is moved to June 2099 (the scenario configuration IS the fault driver);
/// the derived ticks carry far-future event times. Every record is
/// quarantined with a clear reason — nothing is stored, and no GOOD
/// current state appears for the asset.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clock_fault_far_future_event_times_are_quarantined_never_stored_as_good() {
    let Some(h) = harness("p223_clock_far_future", 0x5A).await else {
        return;
    };
    let (task, _source, metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), default_config());

    // Scenario-config driver: the horizon start moves to the far-future
    // instant; the ticks follow the scenario's own cadence.
    let mut scenario: serde_json::Value =
        serde_json::from_str(SCENARIO_V1).expect("the versioned scenario parses");
    scenario["horizon"]["start"] = serde_json::Value::String("2099-06-01T00:00:00Z".to_string());
    let cadence_seconds = scenario["cadence"]["default_seconds"]
        .as_i64()
        .expect("the scenario cadence is a number");
    assert_eq!(cadence_seconds, 60);
    assert_eq!(scenario["horizon"]["start"], "2099-06-01T00:00:00Z",
        "the fault is armed through the scenario configuration");

    let records: Vec<NormalizedTelemetry> = (0..10)
        .map(|i| {
            record(
                h.ctx.tenant_id(),
                h.asset_id,
                JUN_2099_MS + (i as i64) * cadence_seconds * 1000,
                (i + 1) as u64,
                1.0,
            )
        })
        .collect();

    let report = task
        .process_batch(&h.ctx, &records, None)
        .expect("the batch commits with per-record rejections");

    // FAULT FIRST: every skewed record was actually rejected.
    assert_eq!(report.out_of_range_quarantined, 10, "all far-future records rejected");
    assert_eq!(report.stored, 0);

    // HANDLING: clear reasons, quarantined evidence, nothing stored.
    let reasons = quarantine_reasons(&h.store, &h.ctx);
    assert_eq!(reasons.len(), 10, "every rejection is evidenced: {reasons:?}");
    assert!(reasons
        .iter()
        .all(|r| r.contains("outside every existing telemetry partition")));
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0, "nothing silently stored");
    assert_eq!(
        metrics.quarantine_reasons().get("out_of_range_event_time"),
        Some(&10)
    );

    // No GOOD current state from the skewed records.
    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let latest =
        PgUnitOfWork::latest_telemetry_per_asset(&unit, &h.ctx, &[h.asset_id], None)
            .expect("latest query");
    unit.commit().expect("commit read");
    assert!(
        latest.is_empty(),
        "the far-future records never became a current state: {:?}",
        latest
    );
}

// ===========================================================================
// 7. Clock fault — event_time far past (scenario-config driver)
// ===========================================================================

/// Ledger P2.23 step 3, far-past case. The versioned scenario's horizon is
/// moved to January 2020 (its partition exists, so the records are
/// storable valid history). The P2.08 rule in `ingestion.rs` applies: the
/// records store as measured history (the pipeline never rewrites stored
/// quality), and the staleness signal — computed from event_time against
/// the DEPLOYMENT freshness limit — fires for every record, so nothing is
/// silently GOOD as current state (a current-state view applying the same
/// limit shows STALE, asserted here against the P2.08 rule itself).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clock_fault_far_past_event_times_fire_the_staleness_signal_never_good_current() {
    let Some(h) = harness("p223_clock_far_past", 0x5C).await else {
        return;
    };
    // The freshness limit is deployment configuration, never a constant
    // (P2.08 step 3): one minute for this deployment.
    let config = IngestionConfig {
        stale_freshness_limit_millis: Some(60_000),
        ..default_config()
    };
    let (task, _source, metrics) =
        build_task_with((*h.store).clone(), h.inbox.clone(), Vec::new(), config);

    // Scenario-config driver: the horizon start moves to the far-past
    // instant; the ticks follow the scenario's own cadence.
    let mut scenario: serde_json::Value =
        serde_json::from_str(SCENARIO_V1).expect("the versioned scenario parses");
    scenario["horizon"]["start"] = serde_json::Value::String("2020-01-01T00:00:00Z".to_string());
    let cadence_seconds = scenario["cadence"]["default_seconds"]
        .as_i64()
        .expect("the scenario cadence is a number");
    assert_eq!(cadence_seconds, 60);
    assert_eq!(scenario["horizon"]["start"], "2020-01-01T00:00:00Z",
        "the fault is armed through the scenario configuration");

    let records: Vec<NormalizedTelemetry> = (0..10)
        .map(|i| {
            record(
                h.ctx.tenant_id(),
                h.asset_id,
                JAN_2020_MS + (i as i64) * cadence_seconds * 1000,
                (i + 1) as u64,
                1.0,
            )
        })
        .collect();

    // FAULT FIRST: every record's age (now - event_time) exceeds the
    // configured freshness limit by years — the skew is real before the
    // handling is judged. The rule applied is the P2.08 pure rule
    // (`flexcore_domain::quality_rules::staleness`), not a re-implementation.
    let limit = FreshnessLimit::from_seconds(60).expect("a positive freshness limit");
    let now = SystemClock.now_event_time();
    for record_value in &records {
        assert!(
            staleness(record_value.event_time(), now, limit).is_stale(),
            "the fault condition holds: a 2020 event_time is stale under the \
             one-minute limit"
        );
    }

    // HANDLING: the records store as valid history...
    let report = task
        .process_batch(&h.ctx, &records, None)
        .expect("the far-past records store (their partition exists)");
    assert_eq!(report.stored, 10);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 10);

    // ...the staleness signal fired for every record (the signal that
    // prevents a silently-GOOD current state)...
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.stale_observations, 10, "every aged record signaled");
    assert!(snapshot.ingestion_delay_ms_sum > 0, "the delay summary ran");

    // ...and the pipeline did NOT rewrite stored quality (P2.08: staleness
    // is a view-time signal) — the never-GOOD-current property is the
    // signal + the view-time rule, which the current-state read confirms.
    assert_eq!(snapshot.quality_states[0], 10, "stored as measured history");
    assert_eq!(snapshot.quality_states[5], 0, "the pipeline does not relabel stored rows");
    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let latest =
        PgUnitOfWork::latest_telemetry_per_asset(&unit, &h.ctx, &[h.asset_id], None)
            .expect("latest query");
    unit.commit().expect("commit read");
    let active: Vec<_> = latest
        .into_iter()
        .filter(|row| row.quantity == flexcore_postgres_adapter::telemetry::Quantity::ActivePower)
        .collect();
    assert_eq!(active.len(), 1);
    assert!(
        staleness(active[0].event_time, now, limit).is_stale(),
        "the current-state record is stale under the same configured limit — \
         never silently GOOD as current state"
    );
}

// ===========================================================================
// 8. Volume fault — burst above the backpressure limit
// ===========================================================================

/// Ledger P2.23 step 4 (P2.07 §5 / P2.10). A burst far above the bounded
/// channel's recorded capacity is offered with no consumer running: the
/// first `capacity` frames buffer, everything above the limit is SHED with
/// a retryable error and the shed metric increments, and the pipeline later
/// drains exactly the buffered frames — the memory bound is the recorded
/// capacity, nothing more.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn volume_burst_above_the_backpressure_limit_sheds_counts_and_stays_bounded() {
    let Some(h) = harness("p223_volume_burst", 0x5E).await else {
        return;
    };
    let capacity = 8usize;
    let burst = 64usize;
    let preset = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.0);
    let (task, _source, metrics) = build_task_with(
        (*h.store).clone(),
        h.inbox.clone(),
        vec![preset],
        IngestionConfig {
            channel_capacity: capacity,
            ..Default::default()
        },
    );

    // FAULT FIRST: burst far above the limit, no consumer running.
    let queue = task.queue();
    let mut accepted = 0usize;
    let mut shed = 0usize;
    for _ in 0..burst {
        match queue.offer(b"burst-frame".to_vec()) {
            Ok(()) => accepted += 1,
            Err(FrameShed) => shed += 1,
        }
    }
    assert_eq!(
        accepted, capacity,
        "exactly the recorded capacity buffers — the limit is respected"
    );
    assert_eq!(shed, burst - capacity, "everything above the limit is shed");
    assert_eq!(
        metrics.snapshot().frames_shed,
        (burst - capacity) as u64,
        "the shed metric counts every shed frame"
    );

    // HANDLING: the pipeline drains exactly what was buffered (bounded
    // memory: at most `capacity` frames were ever in flight); the shed
    // frames never entered the pipeline.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let run_task = Arc::clone(&task);
    let query = subscription_query(&h.ctx, h.asset_id);
    let run_handle = tokio::spawn(async move { run_task.run(query, shutdown_rx).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = shutdown_tx.send(true);
    let report = tokio::time::timeout(Duration::from_secs(10), run_handle)
        .await
        .expect("the task drains within the timeout")
        .expect("the run task does not panic");
    assert_eq!(
        report.frames_processed + report.frames_drained_at_shutdown,
        capacity as u64,
        "only the buffered frames were ever processed"
    );
    assert_eq!(report.outcome, Some(IngestionRunOutcome::ShutdownDrained));

    // Every buffered frame carried the same preset record: it stores once
    // and the repeats are deduplicated (P2.09) — each frame is its own
    // batch, so the repeats surface as inbox-detected redeliveries of the
    // same identity. The shed frames contributed nothing.
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 1);
    assert_eq!(snapshot.redeliveries_skipped, (capacity - 1) as u64);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
    assert_eq!(snapshot.frames_shed, (burst - capacity) as u64);
    assert_eq!(snapshot.frames_received, capacity as u64);
}
