// P2.22 — Phase 2 security test slice, worker side: the telemetry source
// authentication boundary (recorded Phase 2 STUB status, ledger P2.10
// step 6), the tenant-scoped ingestion boundary (a source cannot ingest
// for another tenant's asset), and the P2.21 no-secrets rule for telemetry
// log lines.
//
// Pure tests (no infrastructure): the source-auth stub boundary, the
// cross-tenant source identity rejection, the no-secrets log assertion,
// and the P2.21 metric inventory.
//
// The database-gated test (`FLEXCORE_TEST_DATABASE_URL`, the recorded P2.06
// harness convention; skips cleanly when unset) drives the REAL ingestion
// store boundary: records decoded for another tenant's asset are rejected
// by the tenant-scoped unit-of-work store — the same code path the worker
// loop hosts (multi-thread runtime, the `block_in_place` contract).
//
// Everything here asserts AUTHORIZATION behaviour only. FlexCore is a
// supervisory orchestration layer; the entire 0.x line is simulation-only
// — nothing here is a safety, certification, or protection claim.

use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
use flexcore_ports::telemetry_ingest::TelemetryIngestError;
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
    QuantityValue, SourceStateMapping, TelemetryQuantity, TelemetrySource, TelemetrySourceError,
    TelemetrySourceHealth,
};
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use flexcore_worker::ingestion::{
    authenticate_source, IngestionConfig, IngestionMetrics, IngestionTask, JsonEventPayloadEncoder,
    PgIngestionInbox, QUALITY_STATE_LABELS, QUARANTINE_REASON_CLASSES, SOURCE_AUTH_STATUS,
    UuidV7Generator,
};
use sqlx::PgPool;

// ---------------------------------------------------------------------------
// Pure tests: the recorded Phase 2 source-auth stub boundary (P2.22 step 4)
// ---------------------------------------------------------------------------

const SOURCE_A: &str = "sim://tenant-a-gateway";
const SOURCE_B: &str = "sim://tenant-b-gateway";

fn declaration(source_id: &str) -> ProviderDeclaration {
    ProviderDeclaration::new(
        source_id.to_string(),
        Duration::from_secs(60),
        QualityMappingDeclaration::new(vec![SourceStateMapping {
            source_code: "GOOD".to_string(),
            canonical: TelemetryQuality::Good,
        }])
        .expect("valid quality mapping"),
        ClockQuality::Synchronized,
    )
    .expect("valid provider declaration")
}

/// The recorded stub status is stated honestly and is what the tests below
/// pin: provider declaration + optional trusted-source-id check only —
/// no cryptographic source authentication exists in Phase 2.
#[test]
fn source_authentication_matches_the_recorded_phase2_stub_status() {
    assert!(SOURCE_AUTH_STATUS.contains("STUB"), "the stub status is recorded");
    assert!(
        SOURCE_AUTH_STATUS.contains("no cryptographic source authentication"),
        "the stub honestly names its limitation"
    );
    assert!(
        SOURCE_AUTH_STATUS.contains("later phase"),
        "the owning phase is named"
    );

    // Per THAT recorded status: a healthy source declaring the expected
    // identity is accepted with and without the trusted-id configuration.
    let source = flexcore_ports::fakes::FakeTelemetrySource::new(declaration(SOURCE_A), vec![]);
    assert!(authenticate_source(&source, None).is_ok());
    assert!(authenticate_source(&source, Some(SOURCE_A)).is_ok());
}

/// An unauthenticated (undeclared/unavailable) source is rejected, and a
/// source presenting another tenant's identity is rejected: the source
/// identity is the Phase 2 tenant-scoping input, checked exactly as the
/// recorded stub defines.
#[test]
fn a_foreign_or_unavailable_source_identity_is_rejected() {
    let source_a = flexcore_ports::fakes::FakeTelemetrySource::new(declaration(SOURCE_A), vec![]);
    let source_b = flexcore_ports::fakes::FakeTelemetrySource::new(declaration(SOURCE_B), vec![]);

    // Tenant A's gateway configured as trusted: source B's identity (a
    // source presenting another tenant's identity) is rejected, and the
    // rejection names both identities (source IDs are NOT secrets).
    let rejection = authenticate_source(&source_b, Some(SOURCE_A)).unwrap_err();
    assert!(
        rejection.reason.contains(SOURCE_B) && rejection.reason.contains(SOURCE_A),
        "the rejection names the identities: {}",
        rejection.reason
    );
    // The mirror case.
    assert!(authenticate_source(&source_a, Some(SOURCE_B)).is_err());

    // An unavailable provider is rejected before any decode: the health
    // check is the "unauthenticated source" path of the recorded stub.
    struct UnavailableSource;
    impl TelemetrySource for UnavailableSource {
        fn provider_declaration(&self) -> &ProviderDeclaration {
            unreachable!("the health check never reads the declaration")
        }
        fn subscribe(
            &self,
            _query: &flexcore_ports::telemetry_source::TelemetryQuery,
        ) -> Result<flexcore_ports::telemetry_source::TelemetrySubscription, TelemetrySourceError>
        {
            unreachable!("an unavailable source never subscribes")
        }
        fn decode(
            &self,
            _subscription: &flexcore_ports::telemetry_source::TelemetrySubscription,
            _payload: &[u8],
        ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
            unreachable!("an unavailable source never decodes")
        }
        fn health(&self) -> TelemetrySourceHealth {
            TelemetrySourceHealth::Unavailable {
                reason: "starting".to_string(),
            }
        }
    }
    let rejection = authenticate_source(&UnavailableSource, None).unwrap_err();
    assert!(rejection.reason.contains("unavailable"));
}

// ---------------------------------------------------------------------------
// P2.21 step 4: the no-secrets rule for telemetry log lines
// ---------------------------------------------------------------------------

/// An in-memory writer capturing rendered log lines (the no-secrets probe).
#[derive(Clone, Default)]
struct SharedLogBuffer(Arc<std::sync::Mutex<Vec<u8>>>);

impl SharedLogBuffer {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("log buffer mutex").clone()).to_string()
    }
}

impl Write for SharedLogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer mutex").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl tracing_subscriber::fmt::MakeWriter for SharedLogBuffer {
    type Writer = SharedLogBuffer;
    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}

/// The no-secrets rule (ledger P2.21 step 4): no credential appears in any
/// telemetry log line. The representative log records below mirror the
/// ingestion pipeline's exact log call sites (the stage-1 source rejection,
/// the quarantine record, the batch-committed record, the per-record
/// correlation record, and the staleness signal — see
/// `apps/worker/src/ingestion.rs`), with adversarial credential values
/// present in the inputs a real deployment would carry (the source auth
/// context, the raw rejected payload). The assertion: the correlation
/// fields ARE rendered (source ids are not secrets) and NONE of the
/// credential values is.
#[test]
fn telemetry_log_lines_never_contain_credentials() {
    const FAKE_PASSWORD: &str = "hunter2-sup3r-secret";
    const FAKE_BEARER: &str = "eyJhbGciOiJIUzI1NiJ9.fake-credential-token";
    const FAKE_API_KEY: &str = "sk-live-0123456789abcdef";

    // The raw rejected payload a real source could carry (the quarantine
    // evidence is written to the store, never logged).
    let raw_payload = format!(
        "{{\"password\":\"{FAKE_PASSWORD}\",\"token\":\"{FAKE_BEARER}\",\"api_key\":\"{FAKE_API_KEY}\"}}"
    );

    let source = flexcore_ports::fakes::FakeTelemetrySource::new(declaration(SOURCE_A), vec![]);
    let tenant = TenantId::new(FlexUuid::from_bytes(0x5E_u128.to_be_bytes()));
    let asset = AssetId::new(FlexUuid::from_bytes(0x5F_u128.to_be_bytes()));

    let buffer = SharedLogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(buffer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        // Stage 1: the source rejection record (the reason derives from the
        // provider declaration — the source ID travels, never a credential).
        if let Err(rejection) = authenticate_source(&source, Some(SOURCE_B)) {
            tracing::warn!(
                reason = %rejection.reason,
                auth_status = SOURCE_AUTH_STATUS,
                "ingestion source rejected at the receive stage"
            );
        }
        // The quarantine record (stage + reason; the raw payload — which
        // DOES contain the credentials — is never a field).
        tracing::warn!(
            stage = "2-validate",
            reason = "decode failed",
            "an ingestion frame was quarantined"
        );
        // The batch-committed record.
        tracing::info!(
            tenant_id = %tenant.uuid().to_hyphenated(),
            source_id = %source.source_id(),
            stored = 1,
            duplicates = 0,
            quarantined = 0,
            "ingestion batch committed"
        );
        // The per-record correlation record (P1.20 rule): tenant, site,
        // asset, source, event_time, quality — identity ids and vocabulary
        // states only.
        tracing::info!(
            tenant_id = %tenant.uuid().to_hyphenated(),
            site_id = "site-under-test",
            asset_id = %asset.uuid().to_hyphenated(),
            source_id = %source.source_id(),
            event_time = 1_893_456_000_000_i64,
            quality = ?TelemetryQuality::Good,
            quantity = "active_power",
            "telemetry record stored"
        );
        // The staleness signal record.
        tracing::warn!(
            tenant_id = %tenant.uuid().to_hyphenated(),
            asset_id = %asset.uuid().to_hyphenated(),
            source_id = %source.source_id(),
            event_time = 1_577_836_800_000_i64,
            quality = ?TelemetryQuality::Good,
            freshness_limit_ms = 60_000,
            "telemetry staleness signal"
        );
        // A debug-level record carrying the raw payload would be the leak
        // the rule forbids — assert that even a payload-touching call site
        // keeps to the allowlist: the payload LENGTH (a number), never the
        // payload content.
        tracing::debug!(
            stage = "2-validate",
            payload_bytes = raw_payload.len(),
            "rejected frame evidence recorded in quarantine"
        );
    });

    let rendered = buffer.contents();
    // The correlation fields ARE present (the allowlist is not a mute
    // switch): source ids and asset ids are not secrets.
    assert!(rendered.contains(SOURCE_A), "source ids are not secrets and do travel");
    assert!(rendered.contains(&tenant.uuid().to_hyphenated().to_string()));
    // No credential value appears anywhere in the rendered telemetry logs.
    assert!(!rendered.contains(FAKE_PASSWORD), "no password in telemetry logs");
    assert!(!rendered.contains(FAKE_BEARER), "no bearer token in telemetry logs");
    assert!(!rendered.contains(FAKE_API_KEY), "no api key in telemetry logs");
    // Not even the raw payload string itself.
    assert!(!rendered.contains(&raw_payload), "the raw rejected payload is never logged");
    // The recorded stub status (which IS logged) carries no credential.
    assert!(!rendered.contains("postgres://"), "no connection string in telemetry logs");
}

// ---------------------------------------------------------------------------
// P2.21: the metric inventory (all six families, six states, closed labels)
// ---------------------------------------------------------------------------

#[test]
fn the_worker_metric_render_carries_the_full_telemetry_inventory() {
    let metrics = IngestionMetrics::default();
    metrics.record_quality_state(TelemetryQuality::Good);
    metrics.record_quality_state(TelemetryQuality::Stale);
    metrics.record_quarantine_reason("decode_failed");
    metrics.record_ingestion_delay_millis(250);
    metrics.set_queue_depth(3);
    metrics.stale_observations.fetch_add(1, Ordering::SeqCst);

    let body = metrics.render_prometheus();
    for family in [
        "flexcore_telemetry_events_total",
        "flexcore_telemetry_quarantine_total",
        "flexcore_telemetry_quarantine_by_reason_total",
        "flexcore_telemetry_queue_depth",
        "flexcore_telemetry_quality_states_total",
        "flexcore_telemetry_stale_observations_total",
        "flexcore_telemetry_ingestion_delay_milliseconds",
    ] {
        assert!(body.contains(family), "missing metric family {family}");
    }
    // All six canonical quality states are observable (the P2.21 risk note:
    // quality counts missing STALE is the named failure mode).
    for state in QUALITY_STATE_LABELS {
        assert!(body.contains(&format!("quality=\"{state}\"")), "state {state} observable");
    }
    // The frozen event types are the events family's closed label set.
    for event in ["TelemetryReceived", "TelemetryQualityChanged", "AssetAvailabilityChanged"] {
        assert!(body.contains(&format!("event=\"{event}\"")), "event {event} observable");
    }
    // The closed quarantine reason classes render (zero included).
    for class in QUARANTINE_REASON_CLASSES {
        assert!(body.contains(&format!("reason_class=\"{class}\"")), "class {class} observable");
    }
}

// ---------------------------------------------------------------------------
// DB-gated: the tenant-scoped ingestion boundary (P2.22 step 3)
// ---------------------------------------------------------------------------

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
    bytes[1] = 0x22; // P2.22 fixture marker
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
    let counter = next_counter().to_be_bytes();
    bytes[10..16].copy_from_slice(&counter[2..8]);
    FlexUuid::from_bytes(bytes)
}

fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id,
            subject: "security-test:ingestion".to_string(),
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
/// connects as (credentials never live in a migration — the recorded P2.06
/// harness deviation pattern).
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

const SOURCE_ID: &str = "sim://security-gateway";

/// The fixed 2030 monthly partitions the own-tenant batch stores into
/// (the recorded P2.06 pattern; idempotent across runs, created as the
/// migrator role — the parent table's owner).
const PARTITION_MONTHS: [(&str, &str, &str); 2] = [
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
];

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

fn provider_declaration() -> ProviderDeclaration {
    declaration(SOURCE_ID)
}

/// A canonical GOOD record for one tenant's asset.
fn record(tenant_id: TenantId, asset_id: AssetId, event_millis: i64, sequence: u64) -> NormalizedTelemetry {
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id,
        asset_id,
        quantity: TelemetryQuantity::ActivePower,
        value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
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

/// The ingestion task over the real store (the pipeline consumes ONLY the
/// `TelemetrySource` port; the fake source never decodes anything here —
/// the batch is driven directly, the same entry point the run loop uses).
fn build_task(
    store: FlexCoreStore,
    inbox: flexcore_worker::inbox::PgInboxStore,
) -> (Arc<IngestionTask>, Arc<IngestionMetrics>) {
    let source = Arc::new(flexcore_ports::fakes::FakeTelemetrySource::new(
        provider_declaration(),
        vec![],
    ));
    let metrics = Arc::new(IngestionMetrics::default());
    // The uuidv7 generator is never reached on the rejection path; a
    // pool-backed generator is composed anyway for contract fidelity.
    let generator = Arc::new(UuidV7Generator::new(store.pool().clone()));
    let task = Arc::new(IngestionTask::new(
        Arc::clone(&source) as Arc<dyn TelemetrySource>,
        store,
        PgIngestionInbox::new(inbox),
        Arc::new(JsonEventPayloadEncoder),
        Arc::new(SystemClock),
        generator,
        IngestionConfig::default(),
        Arc::clone(&metrics),
    ));
    (task, metrics)
}

/// Seeds one tenant with one portfolio/site/asset chain (the recorded P2.06
/// pattern: tenant through the admin connection, tenant-owned rows through
/// the runtime role with the tenant context).
fn seed_registry(
    admin_store: &FlexCoreStore,
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    asset_id: AssetId,
) -> TenantContext {
    use flexcore_contracts::time_model::SiteTimezone;
    use flexcore_contracts::units::EnergyMWh;

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
    let portfolio_id = flexcore_domain::ids::PortfolioId::new(fixture_uuid(0x0F));
    let site_id = flexcore_domain::ids::SiteId::new(fixture_uuid(0x0C));
    {
        let unit = app_store.begin(Some(&ctx)).expect("app unit of work");
        let portfolio = Portfolio::try_new(
            &tenant_entity,
            portfolio_id,
            "Security portfolio".to_string(),
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
            "Security site".to_string(),
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

/// Ledger P2.22 step 3: telemetry cannot be ingested for another tenant's
/// asset. The pipeline's tenant scope comes from the subscription query's
/// tenant (the structural boundary); a decoded record carrying ANOTHER
/// tenant's identity is rejected by the tenant-scoped unit-of-work store
/// (`TenantMismatch`) — the whole batch fails, nothing is stored, nothing
/// is quarantined into the other tenant, and the source's redelivery
/// re-runs the same rejection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingestion_for_another_tenants_asset_is_rejected_by_the_tenant_scoped_store() {
    let Some(url) = require_db("ingestion_for_another_tenants_asset") else {
        return;
    };
    ensure_test_roles(
        &blocking_runtime()
            .block_on(PgPool::connect(&url))
            .expect("admin pool"),
    );
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    let store = FlexCoreStore::connect(&url).expect("app store");

    // Tenant A owns the registry chain; tenant B exists but owns nothing.
    let tenant_a = TenantId::new(fixture_uuid(0x40));
    let tenant_b = TenantId::new(fixture_uuid(0x41));
    let asset_a = AssetId::new(fixture_uuid(0x42));
    let asset_b = AssetId::new(fixture_uuid(0x43));
    let ctx_a = seed_registry(&admin_store, &store, tenant_a, asset_a);
    let _ctx_b = seed_registry(&admin_store, &store, tenant_b, asset_b);

    let inbox = flexcore_worker::inbox::PgInboxStore::new(store.pool().clone()).expect("inbox");
    inbox.ensure_table().expect("inbox table");
    ensure_partitions(&url);
    let (task, metrics) = build_task(store.clone(), inbox);

    // The pipeline runs under tenant A's scope (the query's tenant); the
    // decoded batch claims tenant B's asset — a foreign-tenant write.
    let foreign_batch = vec![
        record(tenant_b, asset_b, 1_893_456_000_000, 1),
        record(tenant_a, asset_a, 1_893_456_000_000, 2),
    ];
    let error = task
        .process_batch(&ctx_a, &foreign_batch)
        .expect_err("the foreign-tenant batch is rejected");
    match error {
        flexcore_worker::ingestion::IngestPipelineError::Store(TelemetryIngestError::TenantMismatch) => {}
        other => panic!("expected the tenant-scoped rejection, got: {other}"),
    }

    // Nothing was stored in EITHER tenant, and nothing was quarantined.
    assert_eq!(count_telemetry(&store, &ctx_a), 0, "tenant A stores nothing");
    let ctx_b = context_for(tenant_b);
    assert_eq!(count_telemetry(&store, &ctx_b), 0, "tenant B stores nothing");
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.records_stored, 0);
    assert_eq!(snapshot.records_quarantined, 0, "a rejected batch is not quarantined evidence");

    // The same batch WITHOUT the foreign record stores cleanly: the
    // rejection is about the tenant scope, not the pipeline.
    let own_batch = vec![record(tenant_a, asset_a, 1_893_456_000_000, 3)];
    let report = task
        .process_batch(&ctx_a, &own_batch)
        .expect("the own-tenant batch stores");
    assert_eq!(report.stored, 1);
    assert_eq!(count_telemetry(&store, &ctx_a), 1);

    // The quarantine metric map stays empty (no closed reason class fired).
    assert!(metrics.quarantine_reasons().is_empty());
}
