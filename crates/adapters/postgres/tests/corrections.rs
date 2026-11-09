// P2.31 — Integration tests for versioned telemetry corrections and as-of
//         (point-in-time) reads on real PostgreSQL (ledger P2.31 step 6):
//
// 1. A correction creates a NEW versioned row and the original stays intact
//    and queryable (version 1 unchanged).
// 2. The as-of read returns the pre-correction value at T1 and the
//    corrected value at T2; the default (no as-of) returns the latest.
// 3. The current-state view (P2.18 `latest_telemetry_per_asset`) reads the
//    latest version only — a superseded version never reaches it.
// 4. The historical as-of read never shows a superseded value next to its
//    correction, and the keyset pagination of D-H1 still walks cleanly.
// 5. Unique-constraint behaviour: the extended `telemetry_pkey` still
//    serves the P2.09 duplicate classification for the plain ingest path
//    (exact duplicate of the ORIGINAL row is acknowledged; a conflicting
//    re-ingest is a conflicting duplicate — corrections never change what
//    the pipeline sees), and the correction procedure enforces the per-
//    identity corrected_at monotonicity.
//
// Gating is the recorded harness decision (the same as tests/telemetry.rs):
// every test skips cleanly when FLEXCORE_TEST_DATABASE_URL is unset. Fixed
// 2030 test months, partitions created as the migrator role, registry rows
// seeded through the runtime role, telemetry inserted through the ingest
// role. Everything here is simulation-only synthetic telemetry (0.x line).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_postgres_adapter::corrections::{
    CorrectionOutcome, CorrectionTarget, TelemetryCorrection,
};
use flexcore_postgres_adapter::telemetry::{
    NormalizedTelemetryRecord, Quantity, TelemetryInsertOutcome, TelemetryValue,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};

// --- Fixed test month (2030, UTC) — the recorded P2.06 pattern ---------------

const JAN_2030: i64 = 1_893_456_000;
const HOUR: i64 = 3_600;

const PARTITION_MONTHS: [(&str, &str, &str); 1] =
    [("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00")];

// --- Harness helpers (same recorded decisions as tests/telemetry.rs) ----------

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
    bytes[1] = 0x6B; // P2.31 fixture marker
    bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
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
            subject: "integration-test:corrections".to_string(),
        },
    ))
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

fn ensure_test_roles(pool: &sqlx::PgPool) {
    let result = test_runtime().block_on(async {
        for (role, password) in [
            ("flexcore_app", "flexcore_app_test_only"),
            ("flexcore_migrator", "flexcore_migrator_test_only"),
            ("flexcore_ingest", "flexcore_ingest_test_only"),
        ] {
            sqlx::query(&format!("ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"))
                .execute(pool)
                .await?;
        }
        Ok::<(), sqlx::Error>(())
    });
    result.expect("test roles must be grantable from the migration/admin connection");
}

fn ensure_partitions(url: &str) {
    let migrator_pool = test_runtime()
        .block_on(sqlx::PgPool::connect(&url_as(
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
}

/// Seeds one tenant with one portfolio/site/asset chain (the recorded
/// tests/telemetry.rs pattern: tenant via the admin connection, the
/// tenant-owned rows via the runtime role).
fn seed_registry(
    admin_store: &FlexCoreStore,
    app_store: &FlexCoreStore,
    tenant_id: TenantId,
    asset_id: AssetId,
) {
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
            "Corrections portfolio".to_string(),
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
            "Corrections site".to_string(),
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

/// The original record fixture: `active_power` = 1.5 MW at T0, ingested 1 s
/// later (the recorded P0.09-legal ordering of tests/telemetry.rs).
fn original_record(tenant_id: TenantId, asset_id: AssetId, event_seconds: i64) -> NormalizedTelemetryRecord {
    NormalizedTelemetryRecord {
        tenant_id,
        asset_id,
        quantity: Quantity::ActivePower,
        value: Some(TelemetryValue::Numeric(1.5)),
        event_time: EventTime::from_unix_millis(event_seconds * 1000).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(event_seconds * 1000 + 1_000)
            .expect("valid ingest time"),
        source_sequence: 1,
        source_id: "sim://exit-gate".to_string(),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        q_sign_convention: None,
        clock_quality: ClockQuality::Synchronized,
    }
}

/// The correction fixture: the same identity, value corrected to 2.0 MW,
/// `corrected_at` = `corrected_seconds` (the as-of visibility key), received
/// 1 s later.
fn correction_fixture(
    tenant_id: TenantId,
    asset_id: AssetId,
    event_seconds: i64,
    corrected_seconds: i64,
) -> TelemetryCorrection {
    TelemetryCorrection {
        target: CorrectionTarget {
            asset_id,
            event_time: EventTime::from_unix_millis(event_seconds * 1000).expect("valid"),
            source_sequence: 1,
            quantity: Quantity::ActivePower,
        },
        value: Some(TelemetryValue::Numeric(2.0)),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        q_sign_convention: None,
        reason: "sensor recalibration (simulated test correction)".to_string(),
        corrected_by: "operator:test".to_string(),
        corrected_at: IngestTime::from_unix_millis(corrected_seconds * 1000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(corrected_seconds * 1000 + 1_000).expect("valid"),
    }
}

/// Stores the original record and one correction; returns the two as-of
/// instants: T1 (after the original is known, before the correction) and
/// T2 (after the correction).
fn seed_original_and_correction(
    url: &str,
    tenant_id: TenantId,
    asset_id: AssetId,
    event_seconds: i64,
) -> (IngestTime, IngestTime) {
    let ingest = ingest_store(url);
    let ctx = context_for(tenant_id);
    {
        let unit = ingest.begin(Some(&ctx)).expect("ingest unit of work");
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx, &original_record(tenant_id, asset_id, event_seconds))
                .expect("original insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit original");
    }
    // T1: 30 minutes after the original is known (its ingest_time), before
    // the correction; T2: 1 hour after, past the correction.
    let t1 = event_seconds + 1 + 30 * 60;
    let t2 = event_seconds + 1 + HOUR;
    {
        let unit = ingest.begin(Some(&ctx)).expect("ingest unit of work");
        assert_eq!(
            PgUnitOfWork::insert_telemetry_correction(
                &unit,
                &ctx,
                &correction_fixture(tenant_id, asset_id, event_seconds, t2)
            )
            .expect("correction insert"),
            CorrectionOutcome::Stored { version: 2 }
        );
        unit.commit().expect("commit correction");
    }
    (
        IngestTime::from_unix_millis(t1 * 1000).expect("valid T1"),
        IngestTime::from_unix_millis(t2 * 1000).expect("valid T2"),
    )
}

fn numeric(record: &flexcore_postgres_adapter::telemetry::StoredTelemetry) -> f64 {
    match record.value {
        Some(TelemetryValue::Numeric(n)) => n,
        other => panic!("expected a numeric value, got {other:?}"),
    }
}

// --- 1. Correction creates a new version; the original stays intact ----------

#[test]
fn correction_creates_a_new_version_and_the_original_stays_intact() {
    let url = require_db("correction_creates_a_new_version_and_the_original_stays_intact");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB1);
    let asset = fixture_asset_id(0xB1);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 4 * HOUR;
    let (_t1, t2) = seed_original_and_correction(&url, tenant, asset, event);

    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);
    let unit = ingest.begin(Some(&ctx)).expect("read unit of work");
    let versions = PgUnitOfWork::query_telemetry_versions(
        &unit,
        &ctx,
        &CorrectionTarget {
            asset_id: asset,
            event_time: EventTime::from_unix_millis(event * 1000).expect("valid"),
            source_sequence: 1,
            quantity: Quantity::ActivePower,
        },
    )
    .expect("version listing");
    assert_eq!(versions.len(), 2, "original + one correction");
    // The ORIGINAL is intact: same telemetry row identity (version 1), same
    // value, no correction metadata.
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].value, Some(TelemetryValue::Numeric(1.5)));
    assert_eq!(versions[0].corrected_at, None);
    assert_eq!(versions[0].correction_reason, None);
    assert_eq!(versions[0].corrected_by, None);
    // The correction is version 2 with its metadata attached.
    assert_eq!(versions[1].version, 2);
    assert_eq!(versions[1].value, Some(TelemetryValue::Numeric(2.0)));
    assert_eq!(
        versions[1].correction_reason.as_deref(),
        Some("sensor recalibration (simulated test correction)")
    );
    assert_eq!(versions[1].corrected_by.as_deref(), Some("operator:test"));
    assert_eq!(versions[1].corrected_at, Some(t2));
    unit.commit().expect("commit read");
}

// --- 2. As-of read: T1 pre-correction, T2 corrected, default latest ----------

#[test]
fn as_of_query_returns_pre_correction_at_t1_and_corrected_at_t2() {
    let url = require_db("as_of_query_returns_pre_correction_at_t1_and_corrected_at_t2");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB2);
    let asset = fixture_asset_id(0xB2);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 5 * HOUR;
    let (t1, t2) = seed_original_and_correction(&url, tenant, asset, event);

    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);
    let unit = ingest.begin(Some(&ctx)).expect("read unit of work");
    let from = EventTime::from_unix_millis((event - HOUR) * 1000).expect("valid");
    let until = EventTime::from_unix_millis((event + HOUR) * 1000).expect("valid");

    // T1 (pre-correction): the ORIGINAL value.
    let at_t1 = PgUnitOfWork::query_telemetry_as_of(
        &unit, &ctx, &[asset], from, until, None, Some(t1), 100,
    )
    .expect("as-of T1 read");
    assert_eq!(at_t1.len(), 1);
    assert_eq!(numeric(&at_t1[0]), 1.5, "T1 must see the pre-correction value");

    // T2 (post-correction): the CORRECTED value.
    let at_t2 = PgUnitOfWork::query_telemetry_as_of(
        &unit, &ctx, &[asset], from, until, None, Some(t2), 100,
    )
    .expect("as-of T2 read");
    assert_eq!(at_t2.len(), 1);
    assert_eq!(numeric(&at_t2[0]), 2.0, "T2 must see the corrected value");

    // Default (no as-of): the LATEST version.
    let latest = PgUnitOfWork::query_telemetry_as_of(
        &unit, &ctx, &[asset], from, until, None, None, 100,
    )
    .expect("latest read");
    assert_eq!(latest.len(), 1);
    assert_eq!(numeric(&latest[0]), 2.0);

    // An as-of BEFORE the original was even known still shows the original
    // (version 1 is always visible — the view decides WHICH version, never
    // whether a record existed).
    let before_all = IngestTime::from_unix_millis((event - HOUR) * 1000).expect("valid");
    let early = PgUnitOfWork::query_telemetry_as_of(
        &unit, &ctx, &[asset], from, until, None, Some(before_all), 100,
    )
    .expect("early as-of read");
    assert_eq!(early.len(), 1);
    assert_eq!(numeric(&early[0]), 1.5);
    unit.commit().expect("commit read");
}

// --- 3. Current state reads the latest version only ----------------------------

#[test]
fn superseded_version_never_reaches_the_current_state() {
    let url = require_db("superseded_version_never_reaches_the_current_state");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB3);
    let asset = fixture_asset_id(0xB3);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 6 * HOUR;
    seed_original_and_correction(&url, tenant, asset, event);

    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);
    let unit = ingest.begin(Some(&ctx)).expect("read unit of work");
    let latest = PgUnitOfWork::latest_telemetry_per_asset(&unit, &ctx, &[asset], None)
        .expect("current-state read");
    assert_eq!(
        latest.len(),
        1,
        "two versions of one identity must surface as ONE current-state row"
    );
    assert_eq!(numeric(&latest[0]), 2.0, "the latest VERSION wins the DISTINCT ON");
    unit.commit().expect("commit read");
}

// --- 4. Historical as-of read + keyset pagination ------------------------------

#[test]
fn history_as_of_shows_one_version_per_identity_and_paginates_cleanly() {
    let url = require_db("history_as_of_shows_one_version_per_identity_and_paginates_cleanly");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB4);
    let asset = fixture_asset_id(0xB4);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 7 * HOUR;
    let (t1, _t2) = seed_original_and_correction(&url, tenant, asset, event);
    // A second, uncorrected record so pagination has more than one row.
    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);
    {
        let unit = ingest.begin(Some(&ctx)).expect("ingest unit of work");
        let mut second = original_record(tenant, asset, event + 60);
        second.source_sequence = 2;
        second.value = Some(TelemetryValue::Numeric(1.6));
        assert_eq!(
            PgUnitOfWork::insert_telemetry(&unit, &ctx, &second).expect("second insert"),
            TelemetryInsertOutcome::Stored
        );
        unit.commit().expect("commit second");
    }

    let from = EventTime::from_unix_millis((event - HOUR) * 1000).expect("valid");
    let until = EventTime::from_unix_millis((event + 2 * HOUR) * 1000).expect("valid");

    // Without as-of: exactly one row per identity — the corrected value for
    // the corrected identity, the raw value for the untouched one.
    let unit = ingest.begin(Some(&ctx)).expect("read unit of work");
    let (page, mut cursor) = PgUnitOfWork::query_telemetry_history_as_of(
        &unit, &ctx, asset, from, until, None, None, 1, None, None,
    )
    .expect("history-as-of first page");
    assert_eq!(page.len(), 1);
    let mut seen = vec![(
        page[0].source_sequence,
        numeric(&page[0]),
    )];
    while let Some(c) = cursor {
        let (next_page, next_cursor) = PgUnitOfWork::query_telemetry_history_as_of(
            &unit, &ctx, asset, from, until, None, None, 1, Some(c), None,
        )
        .expect("history-as-of next page");
        if next_page.is_empty() {
            break;
        }
        seen.push((next_page[0].source_sequence, numeric(&next_page[0])));
        cursor = next_cursor;
    }
    assert_eq!(seen.len(), 2, "one row per identity, no version duplicates across pages");
    assert!(seen.contains(&(1, 2.0)), "corrected identity shows its latest version");
    assert!(seen.contains(&(2, 1.6)), "uncorrected identity unchanged");

    // With as-of T1: the corrected identity shows its pre-correction value.
    let (t1_page, _) = PgUnitOfWork::query_telemetry_history_as_of(
        &unit, &ctx, asset, from, until, None, None, 100, None, Some(t1),
    )
    .expect("history-as-of at T1");
    let corrected_at_t1 = t1_page
        .iter()
        .find(|r| r.source_sequence == 1)
        .expect("corrected identity present at T1");
    assert_eq!(numeric(corrected_at_t1), 1.5);
    unit.commit().expect("commit read");
}

// --- 5. Unique-constraint / duplicate-classification behaviour -----------------

#[test]
fn the_extended_pkey_still_classifies_plain_ingest_duplicates() {
    let url = require_db("the_extended_pkey_still_classifies_plain_ingest_duplicates");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB5);
    let asset = fixture_asset_id(0xB5);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 8 * HOUR;
    seed_original_and_correction(&url, tenant, asset, event);

    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);
    let unit = ingest.begin(Some(&ctx)).expect("ingest unit of work");
    // Re-ingesting the ORIGINAL content after a correction exists: the
    // plain INSERT (version 1 default) collides with the ORIGINAL row on
    // the extended telemetry_pkey and is acknowledged as an exact duplicate
    // — corrections do not change what the P2.09 pipeline classification
    // sees (P2.31: the PK semantics stay intact, D-P3/D-P4).
    assert_eq!(
        PgUnitOfWork::insert_telemetry(&unit, &ctx, &original_record(tenant, asset, event))
            .expect("duplicate insert"),
        TelemetryInsertOutcome::ExactDuplicateAcknowledged
    );
    // A conflicting re-ingest (same identity, different value) is still a
    // conflicting duplicate — quarantined by the caller, never stored.
    let mut conflicting = original_record(tenant, asset, event);
    conflicting.value = Some(TelemetryValue::Numeric(9.9));
    let conflict = PgUnitOfWork::insert_telemetry(&unit, &ctx, &conflicting)
        .expect_err("conflicting duplicate must be rejected");
    assert!(
        matches!(
            conflict,
            flexcore_postgres_adapter::telemetry::TelemetryError::ConflictingDuplicate
        ),
        "a conflicting re-ingest must classify as a conflicting duplicate, got {conflict}"
    );
    unit.commit().expect("commit");
}

#[test]
fn correction_without_original_is_first_record_and_monotonicity_is_enforced() {
    let url = require_db("correction_without_original_is_first_record_and_monotonicity_is_enforced");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations (incl. 0012)");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant = fixture_tenant_id(0xB6);
    let asset = fixture_asset_id(0xB6);
    let app_store = FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
        .expect("app store");
    seed_registry(&admin_store, &app_store, tenant, asset);

    let event = JAN_2030 + 9 * HOUR;
    let (t1, t2) = (event + 30 * 60, event + HOUR);
    let ingest = ingest_store(&url);
    let ctx = context_for(tenant);

    // A correction for an identity with NO stored original: OriginalMissing
    // — the late-arrival rule stores it as the FIRST record through the
    // normal ingest path instead.
    let unit = ingest.begin(Some(&ctx)).expect("ingest unit of work");
    assert_eq!(
        PgUnitOfWork::insert_telemetry_correction(
            &unit,
            &ctx,
            &correction_fixture(tenant, asset, event, t2)
        )
        .expect("correction on missing original"),
        CorrectionOutcome::OriginalMissing
    );
    // The monotonicity rule needs an existing corrected record first.
    assert_eq!(
        PgUnitOfWork::insert_telemetry(&unit, &ctx, &original_record(tenant, asset, event))
            .expect("original insert"),
        TelemetryInsertOutcome::Stored
    );
    assert_eq!(
        PgUnitOfWork::insert_telemetry_correction(
            &unit,
            &ctx,
            &correction_fixture(tenant, asset, event, t2)
        )
        .expect("correction insert"),
        CorrectionOutcome::Stored { version: 2 }
    );
    // A correction whose corrected_at PRECEDES the version it supersedes is
    // rejected (the per-identity as-of ordering must stay monotone).
    let backwards = correction_fixture(tenant, asset, event, t1);
    let rejection = PgUnitOfWork::insert_telemetry_correction(&unit, &ctx, &backwards)
        .expect_err("backwards corrected_at must be rejected");
    assert!(
        matches!(rejection, flexcore_postgres_adapter::telemetry::TelemetryError::Backend(_)),
        "the monotonicity rejection is a repository rejection, got {rejection}"
    );
    unit.commit().expect("commit");
}
