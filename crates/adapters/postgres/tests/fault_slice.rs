// P1.24 — Fault-injection test slice (database): outage and mid-workflow
//         failure, plus the clock-skew case (ledger P1.24 steps 5–6;
//         report §19.2 test layer 6, §6.3–§6.4, §21.2; P1.10/P1.13).
//
// GATING (mirrors crates/adapters/postgres/tests/integration.rs): the
// database tests are gated on `FLEXCORE_TEST_DATABASE_URL` and SKIP
// CLEANLY (with a printed reason) when the variable is unset or empty.
// The clock-skew test needs no infrastructure (it exercises the frozen
// flexcore-contracts time types directly) and therefore runs unguarded —
// noted per test below.
//
// Threats covered (ledger P0.17 §5 = report §17.4):
//   - Row 9 "Clock drift" — the time-model half: a future `event_time`
//     cannot be received before it occurred (ingest >= event), and clock
//     quality is a RECORDED field, never assumed.
//   - Row 10 "Central outage" — Phase 1 slice: a storage outage fails
//     cleanly (opaque error, no panic) and a mid-workflow failure leaves
//     ZERO partial writes (neither state change nor outbox row survives).
//
// Fault-injection assertions (ledger P1.24 risk note: each test asserts
// the fault condition FIRST):
//   (e) DB outage / mid-workflow failure:
//       - an unreachable URL is refused with an opaque, stable code;
//       - inside an open unit of work, an injected duplicate-key collision
//         actually fires (the port reports DuplicateKey), the unit of work
//         rolls back, and a fresh unit of work verifies that NEITHER the
//         state change NOR the outbox row survived — then a recovery unit
//         of work commits cleanly.
//   (f) Clock skew:
//       - a FAR-FUTURE event_time is REJECTED: ingest_time (now) precedes
//         it, and `ingest_time >= event_time` is the frozen P0.09 rule
//         (TimeModelError::IngestBeforeEvent) — never silently accepted;
//       - a FAR-PAST event_time passes the ordering rule (that is all the
//         frozen time model checks) — RECORDED GAP: P0.09 defines no
//         future/past window, no maximum-age rule, and no automatic
//         SUSPECT flagging; staleness is carried as data
//         (`TelemetryQuality::Stale` / `ClockQuality`) for downstream
//         gating, which is the §17.4 row 3 control (malicious/incorrect
//         telemetry: quality flags, stale-data gates — Phase 2
//         telemetry-quality work), not a time-model rejection. Asserted
//         honestly below: the ordering rule accepts it, and clock quality
//         IS recordable (never silently assumed good).
//
// Role model: the runtime role is `flexcore_app` (P1.12 §3.1); migrations
// need the bootstrap administrator. Test data is unique per run
// (timestamp-derived identifiers), so repeated runs against a persistent
// dev database never collide — same harness pattern as the P1.13
// integration tests (helpers duplicated here; no existing file is edited).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{
    ClockQuality, EventTime, IngestTime, TimeModelError, TimeRecord, ValidityWindow, ValidFrom,
    ValidUntil, validate_ingest_after_event,
};
use flexcore_domain::entities::Portfolio;
use flexcore_domain::ids::{PortfolioId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::clock::Clock as _;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::repository::{PortfolioRepository, TenantRepository};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};

// --- Harness helpers (mirroring the P1.13 integration file) ---------------------

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
    bytes[1] = 0x5B; // fault-slice fixture marker
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

/// The domain `TenantContext` through the only public construction path
/// (P0.16 rule 5).
fn context_for(tenant_id: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id,
        subject: "fault-test:tenant-isolation".to_string(),
    }))
}

fn sample_tenant_entity(tenant_id: TenantId) -> flexcore_domain::entities::Tenant {
    flexcore_domain::entities::Tenant::try_new(
        tenant_id,
        format!("fault-tenant-{tenant_id}"),
        None,
        format!("idp:fault-{tenant_id}"),
        "profile:default".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid test tenant")
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

fn ensure_test_roles(pool: &flexcore_postgres_adapter::FlexCoreStore) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let pool = pool.pool().clone();
    runtime
        .block_on(async {
            sqlx::query("ALTER ROLE flexcore_app WITH LOGIN PASSWORD 'flexcore_app_test_only'")
                .execute(&pool)
                .await?;
            sqlx::query(
                "ALTER ROLE flexcore_migrator WITH LOGIN PASSWORD 'flexcore_migrator_test_only'",
            )
            .execute(&pool)
            .await?;
            Ok::<(), sqlx::Error>(())
        })
        .expect("test roles must be grantable from the migration/admin connection");
}

fn seed_tenants(admin_store: &FlexCoreStore, tenants: &[TenantId]) {
    for tenant_id in tenants {
        let unit = admin_store.begin(None).expect("admin unit of work");
        let receipt = TenantRepository::insert(&unit, &sample_tenant_entity(*tenant_id))
            .expect("tenant insert");
        assert_eq!(receipt.entity(), "tenants");
        unit.commit().expect("commit tenant");
    }
}

fn sample_portfolio(parent_tenant: TenantId, portfolio_id: PortfolioId, name: &str) -> Portfolio {
    Portfolio::try_new(
        &sample_tenant_entity(parent_tenant),
        portfolio_id,
        name.to_string(),
        None,
        "Op".to_string(),
        "REGION-PLACEHOLDER".to_string(),
        "rules".to_string(),
        SystemClock.now_event_time(),
    )
    .expect("valid portfolio")
}

fn outbox_write_for(receipt: StateChangeReceipt, key: &str) -> OutboxWrite {
    OutboxWrite::new(
        receipt,
        key.to_string(),
        "flexcore.v1.registry.asset_registered".to_string(),
        format!(r#"{{"fault_slice": true, "key": "{key}"}}"#),
        SystemClock.now_event_time(),
    )
    .expect("valid outbox write")
}

// --- (e) DB outage: an unreachable backend fails cleanly -------------------------

/// The fault: the database is down (nothing listens at the URL). FIRST the
/// test asserts the outage actually held (connect failed), THEN that the
/// failure is CLEAN: an opaque, stable code + human-safe message, no panic,
/// no raw backend text.
#[test]
fn an_unreachable_database_fails_cleanly_with_an_opaque_error() {
    // Port 1 on loopback: nothing listens there — the outage proxy.
    let outage_url = "postgres://flexcore_app:wrong@127.0.0.1:1/no_such_database";
    let result = FlexCoreStore::connect(outage_url);
    // FAULT ASSERTION: the outage actually held — the connection failed.
    let err = match result {
        Ok(_) => panic!("the injected outage must be real: connecting to a dead endpoint must fail"),
        Err(err) => err,
    };
    // GUARD: the failure is clean and opaque.
    assert_eq!(
        err.code(),
        "E_STORE_UNAVAILABLE",
        "an unreachable backend maps to the stable unavailable code"
    );
    assert!(!err.message().is_empty());
    assert!(
        !err.message().to_lowercase().contains("postgres://"),
        "the message must not leak the URL or raw backend text"
    );
}

// --- (e) Mid-workflow failure: no partial writes, recovery commits ---------------

/// The fault: a failure INSIDE an open unit of work (an injected
/// duplicate-key collision — the same mechanism the P1.13 atomicity test
/// uses, exercised here as the P1.24 fault case). FIRST the test asserts
/// the failure actually fired inside the transaction, THEN that the
/// rollback leaves ZERO partial writes, THEN that a recovery unit of work
/// commits cleanly afterwards.
#[test]
fn a_mid_workflow_failure_rolls_back_leaving_no_partial_writes_and_recovers() {
    let admin_url = require_db("a_mid_workflow_failure_rolls_back_leaving_no_partial_writes_and_recovers");
    if skipped(&admin_url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&admin_url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(&admin_store);

    let tenant_a = fixture_tenant_id(0xA5);
    seed_tenants(&admin_store, &[tenant_a]);
    let ctx_a = context_for(tenant_a);

    let app_store = FlexCoreStore::connect(&url_as(
        &admin_url,
        "flexcore_app",
        "flexcore_app_test_only",
    ))
    .expect("app store");

    let baseline_pending = EventOutbox::pending(&app_store, 1000)
        .expect("pending scan")
        .len();
    let baseline_portfolios = PortfolioRepository::list_for_tenant(
        &begin_uow(&app_store, &ctx_a).expect("uow"),
        &ctx_a,
    )
    .expect("list")
    .len();

    let doomed_portfolio_id = PortfolioId::new(fixture_uuid(0x2F));
    let doomed_key = format!("fault-doomed-{:x}", run_key() ^ next_counter());

    // The doomed unit of work: state change, outbox append, then the
    // INJECTED failure (the same idempotency key again — the unique
    // constraint rejects it and the port reports DuplicateKey).
    let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
    let portfolio = sample_portfolio(tenant_a, doomed_portfolio_id, "Doomed portfolio");
    let receipt = PortfolioRepository::insert(&unit, &ctx_a, &portfolio)
        .expect("state change inside the doomed unit of work");
    EventOutbox::append(&unit, outbox_write_for(receipt, &doomed_key))
        .expect("first outbox append inside the doomed unit of work");
    let receipt = StateChangeReceipt::new(
        "portfolios".to_string(),
        StateChangeOperation::Insert,
        Some(tenant_a),
    );
    // FAULT ASSERTION: the injected failure actually fired inside the open
    // transaction.
    match EventOutbox::append(&unit, outbox_write_for(receipt, &doomed_key)) {
        Err(EventOutboxError::DuplicateKey { idempotency_key }) => {
            assert_eq!(idempotency_key, doomed_key, "the fault must be real");
        }
        other => panic!("expected the injected DuplicateKey failure, got {other:?}"),
    }
    // The fault response: ROLLBACK the whole unit of work.
    let finalized = unit.rollback().expect("rollback after the injected failure");
    assert_eq!(
        finalized.outcome(),
        flexcore_storage::unit_of_work::UnitOfWorkOutcome::RolledBack,
        "the fault response is an explicit rollback"
    );

    // ZERO partial writes: neither the state change nor the outbox row
    // survived.
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("unit of work");
        assert!(
            PortfolioRepository::find_by_id(&unit, &ctx_a, doomed_portfolio_id)
                .expect("find")
                .is_none(),
            "no partial state change may survive the rollback"
        );
        assert_eq!(
            PortfolioRepository::list_for_tenant(&unit, &ctx_a).expect("list").len(),
            baseline_portfolios,
            "the tenant's state is exactly as before the fault"
        );
        unit.commit().expect("commit the verification unit of work");
    }
    let pending = EventOutbox::pending(&app_store, 1000).expect("pending scan");
    assert_eq!(pending.len(), baseline_pending, "no outbox row may survive the rollback");
    assert!(!pending.iter().any(|e| e.idempotency_key() == doomed_key));

    // RECOVERY: a fresh unit of work commits cleanly — state change and
    // outbox row together, both visible afterwards.
    let recovered_portfolio_id = PortfolioId::new(fixture_uuid(0x3F));
    let recovered_key = format!("fault-recovered-{:x}", run_key() ^ next_counter());
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("recovery unit of work");
        let portfolio = sample_portfolio(tenant_a, recovered_portfolio_id, "Recovered portfolio");
        let receipt = PortfolioRepository::insert(&unit, &ctx_a, &portfolio)
            .expect("recovered state change");
        EventOutbox::append(&unit, outbox_write_for(receipt, &recovered_key))
            .expect("recovered outbox append");
        let finalized = unit.commit().expect("recovery commit");
        assert_eq!(
            finalized.outcome(),
            flexcore_storage::unit_of_work::UnitOfWorkOutcome::Committed
        );
    }
    {
        let unit = app_store.begin(Some(&ctx_a)).expect("verification unit of work");
        assert!(
            PortfolioRepository::find_by_id(&unit, &ctx_a, recovered_portfolio_id)
                .expect("find")
                .is_some(),
            "the recovery unit of work committed its state change"
        );
        unit.commit().expect("commit");
    }
    let pending = EventOutbox::pending(&app_store, 1000).expect("pending scan");
    assert_eq!(pending.len(), baseline_pending + 1, "exactly one new pending entry");
    assert!(pending.iter().any(|e| e.idempotency_key() == recovered_key));
}

/// Small helper so the baseline read can open and finalize a unit of work
/// in one expression.
fn begin_uow(store: &FlexCoreStore, ctx: &TenantContext) -> Result<PgUnitOfWork, BackendError> {
    store.begin(Some(ctx))
}

// --- (f) Clock skew (P0.09 time model; §17.4 row 9) ------------------------------
//
// Needs no infrastructure: this exercises the frozen flexcore-contracts
// time types directly, so it is NOT gated on FLEXCORE_TEST_DATABASE_URL
// (a gate would only skip a test that cannot fail on a missing database).

/// FAR-FUTURE event_time: the ingest rule rejects it — a value cannot be
/// received before it occurred (`ingest_time >= event_time`, P0.09), so a
/// future-skewed `event_time` is never silently accepted as current.
#[test]
fn a_far_future_event_time_is_rejected_by_the_ingest_rule() {
    // "Now": a fixed, sane reference (2026-01-01T00:00:00Z).
    let now_millis: i64 = 1_767_225_600_000;
    let now_event = EventTime::from_unix_millis(now_millis).expect("valid timestamp");
    let now_ingest = IngestTime::from_unix_millis(now_millis).expect("valid timestamp");

    // One hour in the future — a classic clock-skew forward fault.
    let future_event = EventTime::from_unix_millis(now_millis + 3_600_000).expect("valid");
    // FAULT ASSERTION: the skew actually held — the event claims to be an
    // hour ahead of its own ingest.
    assert!(
        future_event.timestamp() > now_ingest.timestamp(),
        "the fault must be real: event_time is in the future relative to ingest"
    );
    assert_eq!(
        validate_ingest_after_event(future_event, now_ingest),
        Err(TimeModelError::IngestBeforeEvent),
        "a future event_time must be REJECTED, never silently accepted as current"
    );
    // The full record validates through the same rule.
    let record = TimeRecord {
        event_time: future_event,
        ingest_time: now_ingest,
        validity: None,
        source_sequence: 1,
        site_timezone: None,
        clock_quality: ClockQuality::Unknown,
    };
    assert_eq!(record.validate(), Err(TimeModelError::IngestBeforeEvent));

    // Equal timestamps stay valid (the rule is >=, P0.09).
    let equal = TimeRecord {
        event_time: now_event,
        ingest_time: now_ingest,
        validity: None,
        source_sequence: 1,
        site_timezone: None,
        clock_quality: ClockQuality::Synchronized,
    };
    assert_eq!(equal.validate(), Ok(()));
}

/// FAR-PAST event_time: the frozen P0.09 ordering rule ACCEPTS it
/// (ingest >= event holds). RECORDED GAP (see this file's header): the
/// time model has no maximum-age or future/past window rule and no
/// automatic SUSPECT flagging — a stale timestamp is never REJECTED here.
/// What the freeze does guarantee is that the skew is CARRIED, not
/// silently assumed away: clock quality is a recorded field, so a skewed
/// source can be flagged `Unsynchronized`/`STALE` downstream (§17.4 rows
/// 3 and 9 controls — stale-data gates and time-quality monitoring are
/// the Phase 2 telemetry-quality work that consumes these fields).
#[test]
fn a_far_past_event_time_is_accepted_by_the_ordering_rule_and_the_skew_is_recordable() {
    let now_millis: i64 = 1_767_225_600_000;
    let ancient_event = EventTime::from_unix_millis(1_000).expect("valid timestamp"); // 1970
    let now_ingest = IngestTime::from_unix_millis(now_millis).expect("valid timestamp");

    // Honest assertion: the ordering rule that exists accepts the far-past
    // event — it is NOT rejected as stale by the time model.
    assert_eq!(
        validate_ingest_after_event(ancient_event, now_ingest),
        Ok(()),
        "recorded gap: P0.09 has no maximum-age rule; far-past events pass the ordering check"
    );
    let record = TimeRecord {
        event_time: ancient_event,
        ingest_time: now_ingest,
        validity: None,
        source_sequence: 1,
        site_timezone: None,
        clock_quality: ClockQuality::Unknown,
    };
    assert_eq!(record.validate(), Ok(()));

    // The skew is RECORDABLE, never silently assumed good: clock quality
    // and telemetry quality are explicit recorded fields a downstream
    // stale-data gate can key on.
    let flagged = TimeRecord {
        event_time: ancient_event,
        ingest_time: now_ingest,
        validity: None,
        source_sequence: 1,
        site_timezone: None,
        clock_quality: ClockQuality::Unsynchronized,
    };
    assert_eq!(flagged.clock_quality, ClockQuality::Unsynchronized);
    assert_ne!(flagged.clock_quality, ClockQuality::Synchronized);

    // A validity window bounds how long a skewed value may be considered
    // current: the frozen window rule still applies to skewed records.
    let window = ValidityWindow::new(
        ValidFrom::from_unix_millis(now_millis).expect("valid"),
        ValidUntil::from_unix_millis(now_millis + 60_000).expect("valid"),
    )
    .expect("valid window");
    let bounded = TimeRecord {
        event_time: ancient_event,
        ingest_time: now_ingest,
        validity: Some(window),
        source_sequence: 1,
        site_timezone: None,
        clock_quality: ClockQuality::Unsynchronized,
    };
    assert_eq!(bounded.validate(), Ok(()), "the window itself is well-formed");
}
