// P2.19 — Integration tests for the historical view
// (`GET /api/v1/telemetry/history`): the range query returns exactly the
// expected records, the quality filter accepts only the six canonical
// states, the bucketed aggregation matches a hand-computed fixture exactly,
// the keyset pagination walk has no duplicates and no gaps, oversized
// requests are 422, and a cross-tenant asset is 404 (no existence leak).
//
// Composition (recorded): these tests build the REAL Postgres-variant
// backend (the historical route is served by the adapter's history methods
// inside one unit-of-work transaction — D-H5) against the same
// FLEXCORE_TEST_DATABASE_URL service the adapter suites use. Gated on that
// variable; skips cleanly when unset. The seeding pattern is the recorded
// P2.06 harness decision: fixed 2030 test months, partitions created as the
// migrator role, registry rows seeded through the runtime role with the
// tenant context, telemetry inserted through the ingest role.
//
// The pure limit-validation and cursor encode/decode tests at the bottom
// need no database.
//
// EXPLAIN note (ledger P2.19 acceptance 4): the loop bans forbid running
// EXPLAIN here; the representative pruning queries and the predicates that
// make pruning work are recorded in
// `ledger/phase-02-asset-telemetry/19_20_historical_retention.md` §D-H4 and
// the EXPLAIN run is the human's.

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactiveSignConvention};
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase2_default_policies};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_phase2::{
    history_bucket_count, validate_history_buckets, validate_history_limit, validate_history_range,
};
use flexcore_control_plane::state::AppState;
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_ports::fakes::CountingIdGenerator;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_postgres_adapter::telemetry::{
    HistoryBucketWidth, HistoryCursor, NormalizedTelemetryRecord, Quantity,
};
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use serde_json::Value;
use tower::ServiceExt;

// --- Fixed test months (2030, UTC) — the recorded P2.06 pattern ---------------

const JAN_2030: i64 = 1_893_456_000;
const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;

const PARTITION_MONTHS: [(&str, &str, &str); 2] = [
    ("telemetry_y2030m01", "2030-01-01 00:00:00+00", "2030-02-01 00:00:00+00"),
    ("telemetry_y2030m02", "2030-02-01 00:00:00+00", "2030-03-01 00:00:00+00"),
];

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
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

fn fixture_uuid(prefix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[0] = prefix;
    bytes[1] = 0x6C; // P2.19 fixture marker
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
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id,
        subject: "integration-test:telemetry-history".to_string(),
    }))
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

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// Grants LOGIN and test-only passwords to ALL service roles (the recorded
/// P1.13/P2.06 harness deviation pattern — credentials never live in a
/// migration).
fn ensure_test_roles(pool: &sqlx::PgPool) {
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

/// Creates the fixed 2030 monthly partitions as the migrator role
/// (idempotent across runs).
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

/// Seeds one tenant with one portfolio/site/asset chain (the recorded
/// P2.06 pattern: tenants through the admin connection, tenant-owned rows
/// through the runtime role with the tenant context).
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
            "Historical-view portfolio".to_string(),
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
            "Historical-view site".to_string(),
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

/// A record fixture: one (asset, quantity, event_time, sequence) with a
/// value, at the given quality. `ingest_time` is `event_time + 1s` (the
/// P0.09 ordering rule allows it; ingest_time is metadata and the
/// historical query never orders or filters by it).
#[allow(clippy::too_many_arguments)]
fn record(
    tenant_id: TenantId,
    asset_id: AssetId,
    quantity: Quantity,
    value: Option<f64>,
    quality: TelemetryQuality,
    event_seconds: i64,
    sequence: i64,
) -> NormalizedTelemetryRecord {
    NormalizedTelemetryRecord {
        tenant_id,
        asset_id,
        quantity,
        value: value.map(flexcore_postgres_adapter::telemetry::TelemetryValue::Numeric),
        event_time: EventTime::from_unix_millis(event_seconds * 1000).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(event_seconds * 1000 + 1_000)
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

/// Inserts records through the ingest role inside one unit of work.
fn ingest_records(ingest: &FlexCoreStore, ctx: &TenantContext, records: &[NormalizedTelemetryRecord]) {
    let unit = ingest.begin(Some(ctx)).expect("ingest unit of work");
    let items = flexcore_postgres_adapter::PgUnitOfWork::insert_telemetry_batch(&unit, ctx, records)
        .expect("batch insert");
    for item in &items {
        assert_eq!(
            item.outcome,
            flexcore_postgres_adapter::telemetry::TelemetryBatchOutcome::Stored,
            "fixture records must store cleanly"
        );
    }
    unit.commit().expect("commit ingest");
}

// --- API composition (the real Postgres backend, D-H5) ---------------------------

struct FixedIdentityResolver {
    callers: Vec<(String, Caller)>,
}

impl IdentityResolver for FixedIdentityResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        let token = raw
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidCredentials)?;
        self.callers
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, c)| c.clone())
            .ok_or(AuthError::InvalidCredentials)
    }
}

fn caller_for(token: &str, subject_tag: u8, tenant_id: TenantId, role: Role) -> (String, Caller) {
    let claims =
        AuthenticatedClaims::from_verified(VerifiedClaims {
            tenant_id,
            subject: format!("user:admin-{}", subject_tag as char),
        });
    (
        token.to_string(),
        Caller {
            ctx: TenantContext::from_authenticated_claims(claims),
            role,
        },
    )
}

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

/// Composes the router over the REAL Postgres backend (the app-role store:
/// migration 0011 grants it telemetry SELECT; registry reads were already
/// granted in Phase 1). Returns the router and the ingest-role store.
fn composed_router(
    url: &str,
    tenant_a: TenantId,
    tenant_b: TenantId,
) -> (Router, FlexCoreStore) {
    let app_store = FlexCoreStore::connect(&url_as(
        url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app-role store");
    let ingest = FlexCoreStore::connect(&url_as(
        url,
        "flexcore_ingest",
        "flexcore_ingest_test_only",
    ))
    .expect("ingest-role store");
    let (token_a, caller_a) = caller_for("caller-a", b'a', tenant_a, Role::TenantAdmin);
    let (token_b, caller_b) = caller_for("caller-b", b'b', tenant_b, Role::TenantAdmin);
    let resolver = FixedIdentityResolver {
        callers: vec![(token_a, caller_a), (token_b, caller_b)],
    };
    let state = AppState::new(
        flexcore_control_plane::state::Backend::Postgres {
            store: Arc::new(app_store.clone()),
            ids: Arc::new(CountingIdGenerator::new()),
            clock: Arc::new(SystemClock),
        },
        phase2_default_policies(),
        Arc::new(resolver),
        passing_readiness(),
        None,
        300_000,
    );
    (stage1_router(state), ingest)
}

/// Issues one GET against the historical route and returns (status, body).
async fn get_history(router: &Router, token: &str, query: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/telemetry/history?{query}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

fn event_time_text(seconds: i64) -> String {
    EventTime::from_unix_millis(seconds * 1000)
        .expect("valid event time")
        .timestamp()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC 3339 rendering")
}

fn record_value(entry: &Value) -> f64 {
    entry["value_num"].as_f64().expect("value_num present")
}

fn record_time(entry: &Value) -> &str {
    entry["event_time"].as_str().expect("event_time present")
}

// --- 1. Range query: exactly the expected records ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn historical_range_query_returns_exactly_the_expected_records() {
    let url = require_db("historical_range_query_returns_exactly_the_expected_records");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xA9);
    let tenant_b = fixture_tenant_id(0xB9);
    let asset_a = fixture_asset_id(0x11);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x12));
    let (router, ingest) = composed_router(&url, tenant_a, tenant_b);

    // Five January samples (values 1..=5 at day 1..5) plus one February
    // sample that the January window must NOT return.
    let mut records: Vec<_> = (1..=5)
        .map(|day| {
            record(
                tenant_a,
                asset_a,
                Quantity::ActivePower,
                Some(day as f64),
                TelemetryQuality::Good,
                JAN_2030 + day * DAY,
                day,
            )
        })
        .collect();
    records.push(record(
        tenant_a,
        asset_a,
        Quantity::ActivePower,
        Some(99.0),
        TelemetryQuality::Good,
        JAN_2030 + 32 * DAY,
        99,
    ));
    ingest_records(&ingest, &context_for(tenant_a), &records);

    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power&limit=1000",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030 + 2 * DAY),
            event_time_text(JAN_2030 + 4 * DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 3, "exactly the three in-window records: {body}");
    let values: Vec<f64> = data.iter().map(record_value).collect();
    assert_eq!(values, vec![2.0, 3.0, 4.0], "ascending event_time order");
    // The February record is absent; every record carries its canonical
    // unit, the simulated flag, and both timestamps (metadata, P0.09).
    for entry in data {
        assert_eq!(entry["unit"], "MW");
        assert_eq!(entry["quality"], "GOOD");
        assert_eq!(entry["simulated"], true);
        assert!(entry["event_time"].as_str().is_some());
        assert!(entry["ingest_time"].as_str().is_some());
    }
    assert!(data[0]["event_time"].as_str().unwrap() < data[2]["event_time"].as_str().unwrap());
}

// --- 2. Quality filter (the six canonical states only) -----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn historical_quality_filter_selects_only_the_named_state() {
    let url = require_db("historical_quality_filter_selects_only_the_named_state");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xAA);
    let tenant_b = fixture_tenant_id(0xBA);
    let asset_a = fixture_asset_id(0x13);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x14));
    let (router, ingest) = composed_router(&url, tenant_a, tenant_b);

    let records = vec![
        record(tenant_a, asset_a, Quantity::ActivePower, Some(1.0), TelemetryQuality::Good, JAN_2030 + 1 * DAY, 1),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(2.0), TelemetryQuality::Suspect, JAN_2030 + 2 * DAY, 2),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(3.0), TelemetryQuality::Good, JAN_2030 + 3 * DAY, 3),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(4.0), TelemetryQuality::Suspect, JAN_2030 + 4 * DAY, 4),
    ];
    ingest_records(&ingest, &context_for(tenant_a), &records);

    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quality=SUSPECT&limit=1000",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + 5 * DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 2, "only the SUSPECT records: {body}");
    let values: Vec<f64> = data.iter().map(record_value).collect();
    assert_eq!(values, vec![2.0, 4.0]);
    assert!(data.iter().all(|e| e["quality"] == "SUSPECT"));

    // A seventh quality value is 422 (the P0.08 vocabulary is closed).
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quality=UPTIME",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "validation_rejected");
}

// --- 3. Aggregation vs a hand-computed fixture (D-H2) -------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aggregation_matches_a_hand_computed_fixture() {
    let url = require_db("aggregation_matches_a_hand_computed_fixture");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xAB);
    let tenant_b = fixture_tenant_id(0xBB);
    let asset_a = fixture_asset_id(0x15);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x16));
    let (router, ingest) = composed_router(&url, tenant_a, tenant_b);

    // The fixture (all active_power, canonical MW; P > 0 = export):
    //   Jan 1 00:00 -> 1.0, Jan 1 06:00 -> 2.0, Jan 1 12:00 -> 3.0
    //     => 1d bucket Jan 1: count 3, mean 2.0, min 1.0, max 3.0
    //   Jan 2 00:00 -> 10.0, Jan 2 06:00 -> 20.0
    //     => 1d bucket Jan 2: count 2, mean 15.0, min 10.0, max 20.0
    //   Jan 3: no records => the bucket is absent (no synthetic zero-fill).
    let records = vec![
        record(tenant_a, asset_a, Quantity::ActivePower, Some(1.0), TelemetryQuality::Good, JAN_2030, 1),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(2.0), TelemetryQuality::Good, JAN_2030 + 6 * HOUR, 2),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(3.0), TelemetryQuality::Good, JAN_2030 + 12 * HOUR, 3),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(10.0), TelemetryQuality::Good, JAN_2030 + DAY, 4),
        record(tenant_a, asset_a, Quantity::ActivePower, Some(20.0), TelemetryQuality::Good, JAN_2030 + DAY + 6 * HOUR, 5),
    ];
    ingest_records(&ingest, &context_for(tenant_a), &records);

    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power&bucket=1d",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + 2 * DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["quantity"], "active_power");
    assert_eq!(body["unit"], "MW");
    assert_eq!(body["bucket_width"], "1d");
    let buckets = body["buckets"].as_array().expect("buckets array");
    assert_eq!(buckets.len(), 2, "empty buckets are absent: {body}");
    // HAND-COMPUTED expected numbers (literally, per the ledger): bucket 1.
    assert_eq!(buckets[0]["sample_count"], 3);
    assert_eq!(buckets[0]["mean"], 2.0);
    assert_eq!(buckets[0]["min"], 1.0);
    assert_eq!(buckets[0]["max"], 3.0);
    // Bucket 2.
    assert_eq!(buckets[1]["sample_count"], 2);
    assert_eq!(buckets[1]["mean"], 15.0);
    assert_eq!(buckets[1]["min"], 10.0);
    assert_eq!(buckets[1]["max"], 20.0);
    // The first bucket starts exactly at the query's `from` (the date_bin
    // origin), the second one width later.
    assert!(
        buckets[0]["bucket_start"]
            .as_str()
            .expect("bucket_start")
            .starts_with("2030-01-01T00:00:00"),
        "first bucket aligned to from: {body}"
    );
    assert!(
        buckets[1]["bucket_start"]
            .as_str()
            .expect("bucket_start")
            .starts_with("2030-01-02T00:00:00"),
        "second bucket one width later: {body}"
    );

    // A 1h bucket over Jan 1 13:00-14:00 covers a window with NO samples
    // (the fixture's nearest samples are at 12:00 and 06:00): HAND-COMPUTED
    // result — zero buckets (empty windows are absent, no zero-fill).
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power&bucket=1h",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030 + 13 * HOUR),
            event_time_text(JAN_2030 + 14 * HOUR),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let buckets = body["buckets"].as_array().expect("buckets array");
    assert_eq!(buckets.len(), 0, "an empty window has no buckets: {body}");

    // Non-numeric quantities have no numeric aggregate: 422.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=availability&bucket=1h",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + HOUR),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    // Two quantities in one aggregation would mix units: 422 (P0.07).
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power,soc&bucket=1h",
            asset_a.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + HOUR),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

// --- 4. Pagination walk: no duplicates, no gaps (D-H1) ------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pagination_walk_has_no_duplicates_or_gaps() {
    let url = require_db("pagination_walk_has_no_duplicates_or_gaps");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xAC);
    let tenant_b = fixture_tenant_id(0xBC);
    let asset_a = fixture_asset_id(0x17);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x18));
    let (router, ingest) = composed_router(&url, tenant_a, tenant_b);

    // Twelve hourly samples on Jan 1; a page limit of 5 forces three pages
    // (5 + 5 + 2).
    let records: Vec<_> = (0..12)
        .map(|hour| {
            record(
                tenant_a,
                asset_a,
                Quantity::ActivePower,
                Some(hour as f64),
                TelemetryQuality::Good,
                JAN_2030 + hour * HOUR,
                hour + 1,
            )
        })
        .collect();
    ingest_records(&ingest, &context_for(tenant_a), &records);

    let base = format!(
        "asset={}&from={}&until={}&quantities=active_power&limit=5",
        asset_a.uuid().to_hyphenated(),
        event_time_text(JAN_2030),
        event_time_text(JAN_2030 + DAY),
    );
    let mut walked: Vec<f64> = Vec::new();
    let mut query = base.clone();
    let mut pages = 0;
    loop {
        let (status, body) = get_history(&router, "caller-a", &query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let data = body["data"].as_array().expect("data array").clone();
        for entry in &data {
            let value = record_value(entry);
            assert!(
                !walked.contains(&value),
                "duplicate record across pages: {value} (page {pages})"
            );
            walked.push(value);
        }
        pages += 1;
        match body["next_cursor"].as_str() {
            None => break,
            Some(cursor) => {
                query = format!("{base}&cursor={cursor}");
            }
        }
        assert!(pages < 10, "the walk must terminate: pagination loop");
    }
    assert_eq!(pages, 3, "5 + 5 + 2 across three pages");
    // No gaps: the walk is exactly the fixture, in ascending order.
    let expected: Vec<f64> = (0..12).map(|hour| hour as f64).collect();
    assert_eq!(walked, expected);
    // The cursor is opaque and strictly versioned.
    let (status, body) = get_history(&router, "caller-a", &format!("{base}&cursor=garbage")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "validation_rejected");
}

// --- 5. Oversized range / bucket count -> 422 (D-H3) ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_requests_are_rejected_with_422() {
    let url = require_db("oversized_requests_are_rejected_with_422");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xAD);
    let tenant_b = fixture_tenant_id(0xBD);
    let asset_a = fixture_asset_id(0x19);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, fixture_asset_id(0x1A));
    let (router, _ingest) = composed_router(&url, tenant_a, tenant_b);
    let asset_text = asset_a.uuid().to_hyphenated();

    // A 9-day range exceeds the reused 7-day maximum.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={asset_text}&from={}&until={}",
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + 9 * DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "validation_rejected");

    // A missing range is 422 — never an implicit unbounded query.
    let (status, body) = get_history(&router, "caller-a", &format!("asset={asset_text}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // A page over the maximum is 422.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={asset_text}&from={}&until={}&limit=1001",
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + HOUR),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // An unknown bucket width (outside the closed D-H2 set) is 422...
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={asset_text}&from={}&until={}&quantities=active_power&bucket=2m",
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + HOUR),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    // ...and a 1m bucket over a 7-day range spans more than the recorded
    // maximum of 1000 buckets: 422.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={asset_text}&from={}&until={}&quantities=active_power&bucket=1m",
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + 7 * DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // Bucket mode forbids the raw-mode pagination parameters.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={asset_text}&from={}&until={}&quantities=active_power&bucket=1h&limit=5",
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

// --- 6. Cross-tenant denial: 404, no existence leak ----------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_tenant_asset_is_404_not_403() {
    let url = require_db("cross_tenant_asset_is_404_not_403");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    ensure_partitions(&url);

    let tenant_a = fixture_tenant_id(0xAE);
    let tenant_b = fixture_tenant_id(0xBE);
    let asset_a = fixture_asset_id(0x1B);
    let asset_b = fixture_asset_id(0x1C);
    let app_store = FlexCoreStore::connect(&url_as(
        &url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");
    seed_registry(&admin_store, &app_store, tenant_a, asset_a);
    seed_registry(&admin_store, &app_store, tenant_b, asset_b);
    let (router, ingest) = composed_router(&url, tenant_a, tenant_b);

    // Tenant B's asset carries telemetry; tenant A must not see it.
    ingest_records(
        &ingest,
        &context_for(tenant_b),
        &[record(
            tenant_b,
            asset_b,
            Quantity::ActivePower,
            Some(5.0),
            TelemetryQuality::Good,
            JAN_2030 + DAY,
            1,
        )],
    );

    // Raw mode: tenant A querying tenant B's asset is 404 (not 403, not an
    // empty page — the cross-tenant=404 rule).
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}",
            asset_b.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error"]["code"], "not_found");

    // Bucket mode: the same denial.
    let (status, body) = get_history(
        &router,
        "caller-a",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power&bucket=1h",
            asset_b.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    // And the tenant's OWN asset answers normally (control).
    let (status, body) = get_history(
        &router,
        "caller-b",
        &format!(
            "asset={}&from={}&until={}&quantities=active_power",
            asset_b.uuid().to_hyphenated(),
            event_time_text(JAN_2030),
            event_time_text(JAN_2030 + DAY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().expect("data").len(), 1);
}

// --- 7. Pure unit tests: limit validation and cursor encode/decode (no DB) -----------

#[test]
fn history_limit_validation_matches_the_recorded_d_h3_contract() {
    // Default 200; zero rejected; over the maximum rejected.
    assert_eq!(validate_history_limit(None), Ok(200));
    assert_eq!(validate_history_limit(Some(1)), Ok(1));
    assert_eq!(validate_history_limit(Some(1000)), Ok(1000));
    assert!(validate_history_limit(Some(0)).is_err());
    assert!(validate_history_limit(Some(1001)).is_err());
}

#[test]
fn history_range_validation_reuses_the_p2_12_seven_day_rule() {
    let from = EventTime::from_unix_millis(JAN_2030 * 1000).expect("from");
    let ok_until = EventTime::from_unix_millis((JAN_2030 + 7 * DAY) * 1000).expect("until");
    assert_eq!(validate_history_range(from, ok_until), Ok(()));
    let over_until = EventTime::from_unix_millis((JAN_2030 + 7 * DAY + 1) * 1000).expect("over");
    assert!(validate_history_range(from, over_until).is_err());
    assert!(validate_history_range(ok_until, from).is_err(), "inverted range");
}

#[test]
fn history_bucket_count_and_validation_match_the_recorded_d_h3_contract() {
    let from = EventTime::from_unix_millis(JAN_2030 * 1000).expect("from");
    let until_1d = EventTime::from_unix_millis((JAN_2030 + DAY) * 1000).expect("until");
    // A 1-day window is exactly one 1d bucket (the first starts at `from`).
    assert_eq!(history_bucket_count(from, until_1d, HistoryBucketWidth::OneDay), 1);
    // A 1-day window is 25 1h buckets (floor(86400000/3600000) + 1).
    assert_eq!(history_bucket_count(from, until_1d, HistoryBucketWidth::OneHour), 25);
    // 1000 buckets fit; 1001 do not.
    let span_999_buckets = 999 * HistoryBucketWidth::OneHour.millis();
    let until_1000 = EventTime::from_unix_millis(JAN_2030 * 1000 + span_999_buckets).expect("until");
    assert_eq!(validate_history_buckets(from, until_1000, HistoryBucketWidth::OneHour), Ok(()));
    let until_1001 =
        EventTime::from_unix_millis(JAN_2030 * 1000 + span_999_buckets + 1).expect("until");
    assert!(validate_history_buckets(from, until_1001, HistoryBucketWidth::OneHour).is_err());
}

#[test]
fn history_cursor_round_trips_through_the_wire_format() {
    let record = flexcore_postgres_adapter::telemetry::StoredTelemetry {
        telemetry_id: FlexUuid::from_bytes([9u8; 16]),
        tenant_id: TenantId::new(FlexUuid::from_bytes([1u8; 16])),
        asset_id: AssetId::new(FlexUuid::from_bytes([2u8; 16])),
        quantity: Quantity::ActivePower,
        value: None,
        event_time: EventTime::from_unix_millis(1_893_456_000_000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(1_893_456_001_000).expect("valid"),
        source_sequence: 7,
        source_id: "sim://exit-gate".to_string(),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        q_sign_convention: None,
        clock_quality: ClockQuality::Synchronized,
    };
    let cursor = HistoryCursor::for_record(&record);
    let encoded = cursor.encode();
    assert!(encoded.starts_with("hist-v1:"));
    assert_eq!(HistoryCursor::decode(&encoded), Some(cursor.clone()));
    // Malformed cursors are None (the API maps them to 422).
    assert_eq!(HistoryCursor::decode("hist-v1:1:2:active_power"), None);
    assert_eq!(HistoryCursor::decode("v1:x"), None);
    assert_eq!(HistoryCursor::decode(""), None);
    let _ = cursor;
}
