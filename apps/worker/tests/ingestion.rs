// P2.09/P2.10 — Integration tests: the dedup/out-of-order handling and the
// ingestion pipeline end-to-end on real PostgreSQL (the Compose stack's
// service), through the SAME code paths the worker hosts.
//
// Gated on `FLEXCORE_TEST_DATABASE_URL` (the P1.13 harness convention);
// skips cleanly when unset (CI provides the service). NATS is NOT needed:
// the pipeline publishes through the OUTBOX port only (report §6.3) — the
// outbox → broker delivery is the P1.15 publisher's tested path.
//
// What is asserted (ledger P2.09 + P2.10 acceptance overlap):
// 1. Happy path: a decoded frame's records are stored and the
//    `TelemetryReceived` event lands in the outbox (never a direct broker
//    publish).
// 2. Mixed batch: valid records stored, an out-of-range record quarantined
//    with a reason (the quarantine count metric reflects it) — no invalid
//    record is stored as GOOD.
// 3. Duplicate batch: the second delivery is deduplicated — ONE row.
// 4. Out-of-order batch: the late older record is stored (valid history)
//    and the current-state read (latest `event_time`) never regresses; a
//    late newer record updates the state.
// 5. Redelivery: the inbox detects the redelivered record — skipped.
// 6. Source-auth failure: the stub authentication rejects an untrusted
//    source; the recorded STUB status is stated.
// 7. Decode failure: the undecodable frame is quarantined (stage 2), never
//    stored.
//
// The seeding pattern is the recorded P2.06 harness decision: fixed 2030
// test months, partitions created as the migrator role, tenant-owned rows
// through the runtime role with the tenant context. Requires a
// multi-thread tokio runtime (the pipeline drives the sync ports under
// `block_in_place` — the recorded P1.19 contract).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime, SiteTimezone};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    QuantityValue, SourceStateMapping, TelemetryQuery, TelemetryQuantity, TelemetrySource,
};
use flexcore_postgres_adapter::telemetry::Quantity as PgQuantity;
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};
use flexcore_worker::inbox::PgInboxStore;
use flexcore_worker::ingestion::{
    IngestionConfig, IngestionMetrics, IngestionRunOutcome, IngestionTask, JsonEventPayloadEncoder,
    PgIngestionInbox, UuidV7Generator,
};
use sqlx::PgPool;

use flexcore_application::telemetry_dedup::{CurrentStateTransition, RecordDisposition};

// --- Fixed test months (2030, UTC) — the recorded P2.06 pattern --------------

const JAN_2030_MS: i64 = 1_893_456_000_000;
const FEB_2030_MS: i64 = 1_896_134_400_000;
/// A month with NO partition: the out-of-range error path.
const JUN_2031_MS: i64 = 1_938_038_400_000;
/// January 2020 (P2.21 staleness tests): LONG PAST the wall clock, so a
/// stored record at this event_time is always stale relative to now.
const JAN_2020_MS: i64 = 1_577_836_800_000;

const PARTITION_MONTHS: [(&str, &str, &str); 5] = [
    ("telemetry_y2020m01", "2020-01-01 00:00:00+00", "2020-02-01 00:00:00+00"),
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
    ("telemetry_y2030m03", "2030-03-01 00:00:00+00", "2030-04-01 00:00:00+00"),
    ("telemetry_y2030m04", "2030-04-01 00:00:00+00", "2030-05-01 00:00:00+00"),
];

// --- Harness helpers ----------------------------------------------------------

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

fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0x2A; // P2.09/P2.10 fixture marker
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "integration-test:ingestion".to_string(),
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

/// Grants LOGIN and test-only passwords to the service roles the harness
/// connects as (credentials never live in a migration — the recorded
/// P2.06 harness deviation pattern).
fn ensure_test_roles(pool: &PgPool) {
    blocking_runtime()
        .block_on(async {
            for (role, password) in [
                ("flexcore_app", "flexcore_app_test_only"),
                ("flexcore_migrator", "flexcore_migrator_test_only"),
            ] {
                sqlx::query(&format!(
                    "ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"
                ))
                .execute(pool)
                .await?;
            }
            Ok::<(), sqlx::Error>(())
        })
        .expect("test roles must be grantable from the migration/admin connection");
}

/// Creates the fixed 2030 monthly partitions as the migrator role (the
/// parent table's owner). Idempotent across runs. MUST run after
/// [`ensure_test_roles`].
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
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
                 FOR VALUES FROM ('{from}') TO ('{to}')"
            ))
            .execute(&migrator_pool)
            .await
            .expect("partition DDL as the table owner");
        }
    });
}

/// Seeds one tenant with one portfolio/site/asset chain (the P2.06
/// pattern: tenant through the admin connection, tenant-owned rows through
/// the runtime role with the tenant context). Returns the tenant context.
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
            "Ingestion portfolio".to_string(),
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
            "Ingestion site".to_string(),
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

// --- Record fixtures (through the ports' canonical constructors) --------------

const SOURCE_ID: &str = "sim://exit-gate";

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

/// `count(*)` over telemetry under the tenant's RLS scope.
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

/// The pending outbox idempotency keys (the events the publisher will
/// deliver — published via the OUTBOX port only).
fn pending_outbox_keys(store: &FlexCoreStore) -> Vec<String> {
    store
        .pending(1000)
        .expect("outbox pending read")
        .into_iter()
        .map(|entry| entry.idempotency_key().to_string())
        .collect()
}

/// Builds an ingestion task over the real store and a controllable fake
/// source (the pipeline consumes ONLY the `TelemetrySource` port).
fn build_task(
    store: FlexCoreStore,
    records: Vec<NormalizedTelemetry>,
    trusted_source_id: Option<String>,
    inbox: PgInboxStore,
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
    ));
    (task, source, metrics)
}

/// A subscription query for the seeded asset (non-empty asset/quantity
/// sets; the constructor requires the tenant context, P2.11).
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

/// The shared harness prefix: roles, partitions, stores, registry seed,
/// inbox. Returns everything the tests need.
struct Harness {
    store: FlexCoreStore,
    ctx: TenantContext,
    asset_id: AssetId,
    inbox: PgInboxStore,
}

fn harness(test: &str, prefix: u8) -> Option<Harness> {
    let url = require_db(test)?;
    ensure_test_roles(
        &blocking_runtime()
            .block_on(PgPool::connect(&url))
            .expect("admin pool"),
    );
    ensure_partitions(&url);
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    let store = FlexCoreStore::connect(&url).expect("app store");
    let tenant_id = TenantId::new(fixture_uuid(prefix));
    let asset_id = AssetId::new(fixture_uuid(prefix + 1));
    let ctx = seed_registry(&admin_store, &store, tenant_id, asset_id);
    let inbox = PgInboxStore::new(store.pool().clone()).expect("inbox store");
    inbox.ensure_table().expect("inbox table");
    Some(Harness {
        store,
        ctx,
        asset_id,
        inbox,
    })
}

// --- Tests ---------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn happy_path_batch_stores_rows_and_publishes_via_the_outbox() {
    let Some(h) = harness("ingestion_happy_path", 0x10) else {
        return;
    };
    let record = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 1, 1.5);
    let (task, _source, metrics) = build_task(h.store.clone(), vec![record], None, h.inbox.clone());

    // The loop against the fake source; one frame offered through the
    // bounded queue (the source-bridge boundary).
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let run_task = Arc::clone(&task);
    let query = subscription_query(&h.ctx, h.asset_id);
    let run_handle = tokio::spawn(async move { run_task.run(query, shutdown_rx).await });

    let queue = task.queue();
    tokio::time::sleep(Duration::from_millis(300)).await; // the loop subscribes
    queue.offer(b"frame-happy".to_vec()).expect("offered");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 1, "one record stored: {snapshot:?}");
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);

    // The TelemetryReceived event went through the OUTBOX (never a direct
    // broker publish).
    let keys = pending_outbox_keys(&h.store);
    assert!(
        keys.iter().any(|k| k.starts_with("telemetry_received:")),
        "a telemetry_received outbox entry must exist: {keys:?}"
    );

    // Graceful shutdown: stop accepting, drain, exit.
    let _ = shutdown_tx.send(true);
    let report = tokio::time::timeout(Duration::from_secs(10), run_handle)
        .await
        .expect("the task drains within the timeout")
        .expect("the run task does not panic");
    assert_eq!(report.outcome, Some(IngestionRunOutcome::ShutdownDrained));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_batch_stores_valid_and_quarantines_out_of_range() {
    let Some(h) = harness("ingestion_mixed_batch", 0x12) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    // One valid record (January 2030, partition exists) and one whose
    // event_time falls outside every partition (June 2031) — the storage
    // adapter routes it to the out-of-range outcome; the pipeline
    // quarantines it with a reason (no-silent-skip).
    let valid = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 10, 1.0);
    let out_of_range = record(h.ctx.tenant_id(), h.asset_id, JUN_2031_MS, 11, 2.0);

    let report = task
        .process_batch(&h.ctx, &[valid, out_of_range])
        .expect("the batch commits (the rejection is per-record, not whole-batch)");

    assert_eq!(report.stored, 1);
    assert_eq!(report.out_of_range_quarantined, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);

    // The quarantine evidence trail names the stage and the reason.
    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let quarantined = PgUnitOfWork::list_telemetry_quarantine(&unit, &h.ctx, 100)
        .expect("quarantine list");
    unit.commit().expect("commit read");
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].entry.stage, "5-duplicate-handling");
    assert!(
        quarantined[0]
            .entry
            .reason
            .contains("outside every existing telemetry partition")
    );

    // The quarantine count metric reflects the rejected record.
    assert_eq!(metrics.snapshot().records_quarantined, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_batch_stores_one_row() {
    let Some(h) = harness("ingestion_duplicate_batch", 0x14) else {
        return;
    };
    let (task, _source, _metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let first = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 20, 1.5);
    let second = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 20, 1.5);

    let first_report = task.process_batch(&h.ctx, &[first]).expect("first batch");
    assert_eq!(first_report.stored, 1);
    let second_report = task.process_batch(&h.ctx, &[second]).expect("second batch");
    assert_eq!(second_report.exact_duplicates_acknowledged, 1);
    assert_eq!(second_report.stored, 0);

    // ONE row after the duplicate batch.
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn out_of_order_batch_is_stored_and_the_current_state_never_regresses() {
    let Some(h) = harness("ingestion_out_of_order", 0x16) else {
        return;
    };
    let (task, _source, _metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    // Arrival order: the CURRENT record (Feb), then a LATE OLDER record
    // (Jan), then a late NEWER record (Mar — partition y2030m03 exists).
    let current = record(h.ctx.tenant_id(), h.asset_id, FEB_2030_MS, 30, 2.0);
    let late_older = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 31, 1.0);
    let late_newer = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS + 86_400_000 * 60, 32, 3.0);

    let r1 = task.process_batch(&h.ctx, &[current]).expect("batch 1");
    assert_eq!(r1.state_updates, 1);
    let r2 = task.process_batch(&h.ctx, &[late_older]).expect("batch 2");
    assert_eq!(r2.stored, 1, "the late older record is stored (valid history)");
    assert_eq!(
        r2.dispositions[0],
        RecordDisposition::Stored {
            transition: CurrentStateTransition::RetainsState,
            previous_quality: Some(TelemetryQuality::Good),
            quality_changed: false,
        },
        "the current state is NOT regressed by the late older record"
    );
    let r3 = task.process_batch(&h.ctx, &[late_newer]).expect("batch 3");
    assert_eq!(r3.state_updates, 1, "the late newer record updates the state");

    // Three rows (valid history); the current-state read reports the NEWER
    // record (latest event_time, never latest arrival).
    assert_eq!(count_telemetry(&h.store, &h.ctx), 3);
    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let latest = PgUnitOfWork::latest_telemetry_per_asset(&unit, &h.ctx, &[h.asset_id], None)
        .expect("latest query");
    unit.commit().expect("commit read");
    let active: Vec<_> = latest
        .into_iter()
        .filter(|row| row.quantity == PgQuantity::ActivePower)
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0].event_time,
        EventTime::from_unix_millis(JAN_2030_MS + 86_400_000 * 60).unwrap()
    );
    assert_eq!(active[0].source_sequence, 32);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forced_redelivery_is_detected_by_the_inbox_and_not_re_stored() {
    let Some(h) = harness("ingestion_redelivery", 0x18) else {
        return;
    };
    let (task, _source, _metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let record = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 40, 1.5);
    let first = task.process_batch(&h.ctx, &[record.clone()]).expect("first");
    assert_eq!(first.stored, 1);

    // Forced redelivery of the SAME record (the pipeline recorded the
    // delivery post-commit): detected by the inbox, NOT re-stored.
    let again = task.process_batch(&h.ctx, &[record]).expect("redelivery");
    assert_eq!(again.redeliveries_skipped, 1);
    assert_eq!(again.stored, 0);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_auth_failure_rejects_the_source_and_records_the_stub_status() {
    let Some(h) = harness("ingestion_source_auth", 0x1A) else {
        return;
    };
    // A configured trusted source that does NOT match the fake provider's
    // declaration: the stage-1 stub authentication must reject the task at
    // startup, before any decode or store runs.
    let (task, _source, metrics) = build_task(
        h.store.clone(),
        vec![record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 50, 1.5)],
        Some("sim://other".to_string()),
        h.inbox.clone(),
    );

    let query = subscription_query(&h.ctx, h.asset_id);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let report = task.run(query, shutdown_rx).await;
    assert_eq!(
        report.outcome,
        Some(IngestionRunOutcome::SourceAuthFailed {
            reason: "the source identity 'sim://exit-gate' is not the configured trusted \
                     source 'sim://other'"
                .to_string()
        },
    ),
    "the untrusted source is rejected at the receive stage"
    );
    assert_eq!(metrics.snapshot().frames_rejected_auth, 1);
    // Nothing was stored.
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);
    // The stub status is recorded honestly (the compile-time constant).
    assert!(flexcore_worker::ingestion::SOURCE_AUTH_STATUS.contains("STUB"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decode_failure_is_quarantined_and_never_stored() {
    let Some(h) = harness("ingestion_decode_failure", 0x1C) else {
        return;
    };
    let (task, source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());
    source.set_fail_decode(true);

    // Stages 2+3 reject the undecodable frame through the SAME run-loop
    // path: quarantined with the reason, counted, never stored as GOOD
    // (no-silent-skip).
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let run_task = Arc::clone(&task);
    let query = subscription_query(&h.ctx, h.asset_id);
    let run_handle = tokio::spawn(async move { run_task.run(query, shutdown_rx).await });

    let queue = task.queue();
    tokio::time::sleep(Duration::from_millis(300)).await;
    queue.offer(b"garbage-frame".to_vec()).expect("offered");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = shutdown_tx.send(true);
    let report = tokio::time::timeout(Duration::from_secs(10), run_handle)
        .await
        .expect("the task drains within the timeout")
        .expect("the run task does not panic");
    assert_eq!(report.outcome, Some(IngestionRunOutcome::ShutdownDrained));

    assert_eq!(metrics.snapshot().records_quarantined, 1);
    assert_eq!(count_telemetry(&h.store, &h.ctx), 0);

    let unit = h.store.begin(Some(&h.ctx)).expect("unit of work");
    let quarantined = PgUnitOfWork::list_telemetry_quarantine(&unit, &h.ctx, 100)
        .expect("quarantine list");
    unit.commit().expect("commit read");
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].entry.stage, "2-validate");
}

// --- P2.16: telemetry event publication (outbox assertions) -------------------

fn record_with_quality(
    tenant_id: TenantId,
    asset_id: AssetId,
    event_millis: i64,
    sequence: u64,
    value_mw: f64,
    quality: TelemetryQuality,
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
        quality,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .unwrap()
}

fn availability_record(
    tenant_id: TenantId,
    asset_id: AssetId,
    event_millis: i64,
    sequence: u64,
    available: bool,
    quality: TelemetryQuality,
) -> NormalizedTelemetry {
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id,
        asset_id,
        quantity: TelemetryQuantity::Availability,
        value: Some(QuantityValue::Flag(available)),
        event_time: EventTime::from_unix_millis(event_millis).unwrap(),
        ingest_time: IngestTime::from_unix_millis(event_millis + 1_000).unwrap(),
        source_sequence: sequence,
        source_id: SOURCE_ID.to_string(),
        quality,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .unwrap()
}

/// Parses the serialized event envelope of one pending outbox entry.
fn envelope_of(entry: &flexcore_ports::event_outbox::PendingOutboxEntry) -> serde_json::Value {
    serde_json::from_str(entry.payload()).expect("the outbox payload is a serialized envelope")
}

/// Ledger P2.16 step 2: a quality change produces the event with BOTH the
/// previous and the new states (the P2.16 payload revision).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quality_change_event_carries_previous_and_new_states() {
    let Some(h) = harness("p216_quality_change_event", 0x1E) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let good = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 60, 1.5);
    task.process_batch(&h.ctx, &[good]).expect("batch 1");
    let suspect = record_with_quality(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS + 60_000,
        61,
        9.9,
        TelemetryQuality::Suspect,
    );
    task.process_batch(&h.ctx, &[suspect]).expect("batch 2");

    let entries = h.store.pending(1000).expect("pending outbox read");
    let keys: Vec<String> = entries
        .iter()
        .map(|e| e.idempotency_key().to_string())
        .collect();
    let changed: Vec<_> = entries
        .iter()
        .filter(|e| e.idempotency_key().starts_with("telemetry_quality_changed:"))
        .collect();
    assert_eq!(
        changed.len(),
        1,
        "exactly one quality-change event: {keys:?}"
    );
    let event = &envelope_of(changed[0])["event"];
    assert_eq!(event["event_type"], "TELEMETRY_QUALITY_CHANGED");
    assert_eq!(event["previous_quality"], "GOOD", "the previous state travels");
    assert_eq!(event["quality"], "SUSPECT", "the new state travels");
    assert_eq!(metrics.snapshot().quality_changed_events, 1);
}

/// Ledger P2.16 step 3: an availability change produces the event, and a
/// repeated identical signal does NOT produce another one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn availability_change_emits_the_event_only_on_a_change() {
    let Some(h) = harness("p216_availability_event", 0x20) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    // First signal: the asset reports available (GOOD, true). Pre-batch:
    // no availability record at all -> Unknown -> Available is a change.
    let up = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS,
        70,
        true,
        TelemetryQuality::Good,
    );
    task.process_batch(&h.ctx, &[up]).expect("batch 1");
    assert_eq!(metrics.snapshot().availability_changed_events, 1);

    // Second signal: the asset reports unavailable (GOOD, false).
    // Available -> Unavailable is a change.
    let down = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS + 60_000,
        71,
        false,
        TelemetryQuality::Good,
    );
    task.process_batch(&h.ctx, &[down]).expect("batch 2");
    assert_eq!(metrics.snapshot().availability_changed_events, 2);

    // Third signal: identical to the current state (GOOD, false).
    // Unavailable -> Unavailable is NOT a change: no third event.
    let down_again = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS + 120_000,
        72,
        false,
        TelemetryQuality::Good,
    );
    task.process_batch(&h.ctx, &[down_again]).expect("batch 3");
    assert_eq!(metrics.snapshot().availability_changed_events, 2);

    // Two events in the outbox, both `AssetAvailabilityChanged`.
    let entries = h.store.pending(1000).expect("pending outbox read");
    let events: Vec<_> = entries
        .iter()
        .filter(|e| e.idempotency_key().starts_with("asset_availability_changed:"))
        .collect();
    assert_eq!(events.len(), 2);
    for entry in &events {
        assert_eq!(
            envelope_of(entry)["event"]["event_type"],
            "ASSET_AVAILABILITY_CHANGED"
        );
    }
}

/// A STALE availability signal leaves the previous availability unchanged:
/// no `AssetAvailabilityChanged` event (the P2.16 rule's STALE branch).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_availability_signal_does_not_change_the_availability() {
    let Some(h) = harness("p216_availability_stale", 0x24) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let up = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS,
        75,
        true,
        TelemetryQuality::Good,
    );
    task.process_batch(&h.ctx, &[up]).expect("batch 1");
    assert_eq!(metrics.snapshot().availability_changed_events, 1);

    // A STALE availability record that moves the current state forward:
    // the rule says the previous availability STANDS — no change, no event.
    let stale = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS + 60_000,
        76,
        false,
        TelemetryQuality::Stale,
    );
    task.process_batch(&h.ctx, &[stale]).expect("batch 2");
    assert_eq!(
        metrics.snapshot().availability_changed_events,
        1,
        "STALE leaves the availability unchanged"
    );
}

/// Ledger P2.16 step 5: redelivery does not duplicate events — the second
/// delivery of the same record is skipped by the inbox before any outbox
/// append, and the deterministic idempotency keys would deduplicate in the
/// outbox regardless (report §6.3, P1.16).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redelivery_does_not_duplicate_events() {
    let Some(h) = harness("p216_redelivery_events", 0x26) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let first = availability_record(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS,
        80,
        true,
        TelemetryQuality::Good,
    );
    let report = task.process_batch(&h.ctx, &[first.clone()]).expect("first");
    assert_eq!(report.stored, 1);

    // Forced redelivery of the SAME record.
    let again = task.process_batch(&h.ctx, &[first]).expect("redelivery");
    assert_eq!(again.redeliveries_skipped, 1);
    assert_eq!(again.stored, 0);

    // The redelivered record produced NO second event of any kind.
    assert_eq!(metrics.snapshot().received_events, 1);
    assert_eq!(metrics.snapshot().availability_changed_events, 1);
    let entries = h.store.pending(1000).expect("pending outbox read");
    let keys: Vec<String> = entries
        .iter()
        .map(|e| e.idempotency_key().to_string())
        .collect();
    let unique: std::collections::HashSet<&String> = keys.iter().collect();
    assert_eq!(keys.len(), unique.len(), "no duplicate outbox keys: {keys:?}");
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("telemetry_received:")).count(),
        1
    );
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("asset_availability_changed:")).count(),
        1
    );
}

/// Ledger P2.16 step 5: a mixed batch produces the expected events in the
/// outbox — one `TelemetryReceived` per stored record, a quality event for
/// the degraded record (previous+new), and no events for the quarantined
/// conflicting duplicate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mixed_batch_produces_exactly_the_expected_outbox_events() {
    let Some(h) = harness("p216_mixed_batch_events", 0x28) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    // Establish the current state (GOOD, seq 90).
    let good = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 90, 1.0);
    task.process_batch(&h.ctx, &[good]).expect("batch 1");

    // Mixed batch 2: a GOOD record (no quality event) + a SUSPECT record
    // (quality event GOOD->SUSPECT) + a conflicting duplicate of seq 91's
    // identity (quarantined, no events) + an exact duplicate of seq 90
    // (acknowledged, no events).
    let fresh_good = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS + 60_000, 91, 1.2);
    let suspect = record_with_quality(
        h.ctx.tenant_id(),
        h.asset_id,
        JAN_2030_MS + 120_000,
        92,
        9.9,
        TelemetryQuality::Suspect,
    );
    let conflict = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS + 60_000, 91, 7.7);
    let exact_dup = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 90, 1.0);
    let report = task
        .process_batch(&h.ctx, &[fresh_good, suspect, conflict, exact_dup])
        .expect("batch 2");
    assert_eq!(report.stored, 2);
    assert_eq!(report.conflicting_quarantined, 1);
    assert_eq!(report.exact_duplicates_acknowledged, 1);

    let entries = h.store.pending(1000).expect("pending outbox read");
    let keys: Vec<String> = entries
        .iter()
        .map(|e| e.idempotency_key().to_string())
        .collect();
    // Exactly one TelemetryReceived per stored record: 3 stored rows total.
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("telemetry_received:")).count(),
        3
    );
    // Exactly one quality-change event (the SUSPECT record).
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("telemetry_quality_changed:")).count(),
        1
    );
    // No availability records were ingested: no availability event.
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("asset_availability_changed:")).count(),
        0
    );
    assert_eq!(metrics.snapshot().received_events, 3);
    assert_eq!(metrics.snapshot().quality_changed_events, 1);
}

// --- P2.21: pipeline metric instrumentation ------------------------------------

/// Ledger P2.21: the pipeline records the ingestion-delay summary and the
/// per-state quality counts over the STORED records on the SAME metrics
/// handle the run loop drives.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipeline_metrics_record_ingestion_delay_and_quality_states() {
    let Some(h) = harness("p221_delay_quality_metrics", 0x2C) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    // Two GOOD records store (both partitions exist); the delay summary
    // covers stored records only.
    let first = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 100, 1.5);
    let second = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS + 60_000, 101, 2.5);
    let report = task
        .process_batch(&h.ctx, &[first, second])
        .expect("both records store");
    assert_eq!(report.stored, 2);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.ingestion_delay_ms_count, 2, "one delay per stored record");
    assert!(
        snapshot.ingestion_delay_ms_sum > 0,
        "the records' ingest_time is 1s after their event_time (the fixture)"
    );
    // Per-state totals (the recorded cardinality decision): both GOOD.
    assert_eq!(snapshot.quality_states[0], 2, "two GOOD records");
    assert_eq!(snapshot.quality_states[1], 0, "no SUSPECT");
    assert_eq!(snapshot.quality_states[5], 0, "no STALE count here");

    // The rendered inventory carries the delay and quality families with
    // the canonical state labels (all six observable).
    let body = metrics.render_prometheus();
    assert!(body.contains("flexcore_telemetry_ingestion_delay_milliseconds_count 2"));
    for state in ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"] {
        assert!(body.contains(&format!("quality=\"{state}\"")), "state {state} observable");
    }
}

/// Ledger P2.21: the staleness signal. A record whose age exceeds the
/// DEPLOYMENT freshness limit increments the staleness metric and emits the
/// staleness log line; the stored quality state is NOT rewritten (P2.08:
/// staleness is a view-time signal) and the threshold is configuration,
/// never hard-coded.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_staleness_signal_fires_against_the_configured_freshness_limit() {
    let Some(h) = harness("p221_staleness_signal", 0x2E) else {
        return;
    };
    // A task WITHOUT the limit configured: no staleness evaluation happens.
    let (unlimited, _source, unlimited_metrics) =
        build_task(h.store.clone(), vec![], None, h.inbox.clone());
    // A task WITH the deployment freshness limit (one minute).
    let source = Arc::new(flexcore_ports::fakes::FakeTelemetrySource::new(
        provider_declaration(),
        vec![],
    ));
    let limited_metrics = Arc::new(IngestionMetrics::default());
    let limited = Arc::new(IngestionTask::new(
        Arc::clone(&source) as Arc<dyn TelemetrySource>,
        h.store.clone(),
        PgIngestionInbox::new(h.inbox.clone()),
        Arc::new(JsonEventPayloadEncoder),
        Arc::new(SystemClock),
        Arc::new(UuidV7Generator::new(h.store.pool().clone())),
        IngestionConfig {
            stale_freshness_limit_millis: Some(60_000),
            ..Default::default()
        },
        Arc::clone(&limited_metrics),
    ));

    // January 2020 is long past the wall clock: stale under any one-minute
    // limit. Two DISTINCT records (different event times/sequences).
    let stale_unlimited =
        record(h.ctx.tenant_id(), h.asset_id, JAN_2020_MS, 110, 1.0);
    let stale_limited =
        record(h.ctx.tenant_id(), h.asset_id, JAN_2020_MS + 60_000, 111, 2.0);

    unlimited
        .process_batch(&h.ctx, &[stale_unlimited])
        .expect("the 2020 record stores (its partition exists)");
    limited
        .process_batch(&h.ctx, &[stale_limited])
        .expect("the 2020 record stores");

    assert_eq!(
        unlimited_metrics.snapshot().stale_observations,
        0,
        "no limit configured: no staleness evaluation"
    );
    assert_eq!(
        limited_metrics.snapshot().stale_observations,
        1,
        "the aged record produced the staleness signal"
    );
    // The stored quality state was NOT rewritten by the signal (P2.08:
    // staleness never silently re-labels a stored value).
    assert_eq!(limited_metrics.snapshot().quality_states[0], 1, "still GOOD");
    assert_eq!(limited_metrics.snapshot().quality_states[5], 0, "not STALE-labelled");
    let body = limited_metrics.render_prometheus();
    assert!(body.contains("flexcore_telemetry_stale_observations_total 1"));
}

/// Ledger P2.21: an out-of-range record's quarantine is counted under its
/// closed reason class (the free-text rejection reason is never a label).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quarantine_metrics_carry_the_closed_reason_class() {
    let Some(h) = harness("p221_quarantine_reason_class", 0x30) else {
        return;
    };
    let (task, _source, metrics) = build_task(h.store.clone(), vec![], None, h.inbox.clone());

    let valid = record(h.ctx.tenant_id(), h.asset_id, JAN_2030_MS, 120, 1.0);
    let out_of_range = record(h.ctx.tenant_id(), h.asset_id, JUN_2031_MS, 121, 2.0);
    task.process_batch(&h.ctx, &[valid, out_of_range])
        .expect("the batch commits with the per-record rejection");

    let reasons = metrics.quarantine_reasons();
    assert_eq!(reasons.get("out_of_range_event_time"), Some(&1));
    assert_eq!(reasons.get("conflicting_duplicate"), None, "the closed classes stay separated");
    // The rendered family carries the closed class label, and every closed
    // class renders (zero included) — the inventory is stable.
    let body = metrics.render_prometheus();
    assert!(body.contains(
        "flexcore_telemetry_quarantine_by_reason_total{reason_class=\"out_of_range_event_time\"} 1"
    ));
    assert!(body.contains("reason_class=\"decode_failed\"} 0"));
}
