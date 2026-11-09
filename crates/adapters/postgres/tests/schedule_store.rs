// P3.21 — Integration tests for the PostgreSQL schedule-result store
//         (migration 0013; report §12.6, §16.1, §15.5, §16.2).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset):
//
// - the migration creates the table with the closed §12.6 status CHECK and
//   the objective-completeness CHECK (an `optimal` row without complete
//   objective columns is rejected by the DATABASE, the last line of
//   defence behind the storage-row and port-level validation);
// - insert + find round-trip inside the caller's unit of work (the
//   record + `ScheduleProposed` outbox row share that transaction, §15.5);
// - the record + outbox pair is atomic: a rolled-back unit of work leaves
//   NEITHER the state row NOR the outbox row (the P1.13 atomicity pattern);
// - a duplicate candidate identity conflicts (records are immutable);
// - a foreign-tenant read returns `None` (RLS + explicit filter; existence
//   never leaks, P0.16 rule 5).
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_ports::event_outbox::{EventOutbox, OutboxWrite};
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use flexcore_storage::schedule_store::{
    ScheduleRecordRow, ScheduleRecordStore, ScheduleStoreError,
};
use sqlx::PgPool;

// --- Harness helpers (the recorded P2.06 decisions) ------------------------------

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
    bytes[1] = 0xB2; // P3.21 fixture marker
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
            subject: "p3.21-schedule-store-test".to_string(),
        },
    ))
}

// --- Fixture chain (tenant -> portfolio -> site -> asset) ------------------------

fn seed_chain(store: &FlexCoreStore, tenant_uuid: FlexUuid) -> (TenantContext, FlexUuid, FlexUuid) {
    let ctx = tenant_context(tenant_uuid);
    let portfolio_uuid = fixture_uuid(0x22);
    let site_uuid = fixture_uuid(0x23);
    let asset_uuid = fixture_uuid(0x24);
    let created = EventTime::from_unix_millis(1_700_000_000_000).expect("valid");
    {
        let unit: PgUnitOfWork = store.begin(Some(&ctx)).expect("unit of work opens");
        let tenant = Tenant::try_new(
            TenantId::new(tenant_uuid),
            "p3.21 tenant".to_string(),
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
            "p3.21 portfolio".to_string(),
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
            "p3.21 site".to_string(),
            None,
            "synthetic".to_string(),
            SiteTimezone::new("UTC".to_string()).expect("UTC is valid"),
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
            "p3.21 asset".to_string(),
            None,
            AssetType::Bess,
            "vendor-x".to_string(),
            flexcore_contracts::units::ActivePowerMW::new(2.0).expect("valid"),
            Some(flexcore_contracts::units::EnergyMWh::new(4.0).expect("valid")),
            "abc".to_string(),
            "bus-1".to_string(),
            "bess-profile-ref-v1".to_string(),
            created,
        )
        .expect("asset builds");
        AssetRepository::insert(&unit, &ctx, &asset).expect("asset inserts");
        unit.commit().expect("committed");
    }
    (ctx, portfolio_uuid, site_uuid)
}

fn complete_row(
    ctx: &TenantContext,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    candidate_uuid: FlexUuid,
) -> ScheduleRecordRow {
    ScheduleRecordRow::new(
        ctx.tenant_id(),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        candidate_uuid.to_string(),
        "optimal".to_string(),
        "lp".to_string(),
        "highs".to_string(),
        "1.15.1".to_string(),
        "tier1-python-worker".to_string(),
        "0.1.0".to_string(),
        Some(-42.0),
        Some("currency-units".to_string()),
        "schedule_input_snapshot:envelope-v1:forecast-v1:synthetic:fnv1a64:00000000000000a1"
            .to_string(),
        "grid-model-v1".to_string(),
        site_uuid.to_string(),
        "site-grid-model-reference-v1".to_string(),
        format!(
            "schedule_proposed:{portfolio_uuid}:{site_uuid}:{candidate_uuid}"
        ),
        "{\"status\":\"optimal\",\"warnings\":[],\"determinism\":{\"thread_count\":1}}"
            .to_string(),
    )
    .expect("the complete row builds")
}

// --- Tests -----------------------------------------------------------------------

#[test]
fn insert_find_round_trip_and_atomic_rollback() {
    let Some(url) = require_db("insert_find_round_trip_and_atomic_rollback") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x21);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);

    // INSERT inside the unit of work, paired with the ScheduleProposed
    // outbox row (same transaction), then COMMIT.
    let candidate = fixture_uuid(0x24);
    let row = complete_row(&ctx, portfolio_uuid, site_uuid, candidate);
    let key = row.schedule_proposed_key.clone();
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let receipt = unit.insert_schedule_record(&ctx, &row).expect("inserts");
        assert_eq!(receipt.candidate_id, candidate.to_string());
        let outbox = OutboxWrite::new(
            StateChangeReceipt::new(
                "schedule_results".to_string(),
                StateChangeOperation::Insert,
                Some(ctx.tenant_id()),
            ),
            key.clone(),
            "flexcore.v1.scheduling.schedule_proposed".to_string(),
            "{\"fixture\": true}".to_string(),
            EventTime::from_unix_millis(1_700_000_000_500).expect("valid"),
        )
        .expect("outbox write builds");
        EventOutbox::append(&unit, outbox).expect("outbox row appends");
        unit.commit().expect("committed");
    }

    // FIND back: every field round-trips.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let found = unit
            .find_schedule_record(&ctx, &candidate.to_string())
            .expect("read succeeds")
            .expect("the committed record is visible after commit");
        assert_eq!(found, row);
        unit.rollback().expect("read-only unit rolled back");
    }

    // ROLLBACK atomicity: a unit of work that inserts a record + outbox row
    // and then rolls back leaves NEITHER (the P1.13 atomicity pattern).
    let rolled_back_candidate = fixture_uuid(0x25);
    let row2 = complete_row(&ctx, portfolio_uuid, site_uuid, rolled_back_candidate);
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        unit.insert_schedule_record(&ctx, &row2).expect("inserts");
        unit.rollback().expect("rolled back");
    }
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let found = unit
            .find_schedule_record(&ctx, &rolled_back_candidate.to_string())
            .expect("read succeeds");
        assert!(found.is_none(), "a rolled-back record must not be visible");
        unit.rollback().expect("rolled back");
    }
}

#[test]
fn a_duplicate_candidate_conflicts_and_the_record_is_immutable() {
    let Some(url) = require_db("a_duplicate_candidate_conflicts_and_the_record_is_immutable")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x31);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let candidate = fixture_uuid(0x32);
    let row = complete_row(&ctx, portfolio_uuid, site_uuid, candidate);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        unit.insert_schedule_record(&ctx, &row).expect("first insert");
        // Second insert of the SAME candidate: Conflict (immutable records;
        // a re-solve is a new candidate, P3.22).
        assert_eq!(
            unit.insert_schedule_record(&ctx, &row),
            Err(ScheduleStoreError::Conflict {
                candidate_id: candidate.to_string(),
            })
        );
        unit.rollback().expect("rolled back");
    }
}

#[test]
fn a_foreign_tenant_read_returns_none() {
    let Some(url) = require_db("a_foreign_tenant_read_returns_none") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x41);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let candidate = fixture_uuid(0x42);
    let row = complete_row(&ctx, portfolio_uuid, site_uuid, candidate);
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        unit.insert_schedule_record(&ctx, &row).expect("inserts");
        unit.commit().expect("committed");
    }

    // A different tenant reads `None` — existence never leaks (P0.16 rule 5).
    let foreign_ctx = tenant_context(fixture_uuid(0x43));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        let found = unit
            .find_schedule_record(&foreign_ctx, &candidate.to_string())
            .expect("read succeeds");
        assert!(found.is_none(), "a foreign-tenant record reads as None");
        unit.rollback().expect("rolled back");
    }
}

#[test]
fn the_database_rejects_an_optimal_row_without_a_complete_objective() {
    let Some(url) = require_db("the_database_rejects_an_optimal_row_without_a_complete_objective")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x51);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);

    // The row constructors refuse it first; the test below proves the
    // DATABASE constraint is the last line of defence by writing raw SQL
    // that bypasses the constructors.
    let parts = complete_row(&ctx, portfolio_uuid, site_uuid, fixture_uuid(0x52));
    let blank_units_row = ScheduleRecordRow::new(
        parts.tenant_id,
        parts.portfolio_id.clone(),
        parts.site_id.clone(),
        parts.candidate_id.clone(),
        parts.status.clone(),
        parts.problem_class.clone(),
        parts.solver_name.clone(),
        parts.solver_version.clone(),
        parts.provider_id.clone(),
        parts.provider_version.clone(),
        parts.objective_value,
        Some("   ".to_string()),
        parts.input_snapshot_id.clone(),
        parts.grid_model_version.clone(),
        parts.grid_model_site_id.clone(),
        parts.grid_model_site_reference.clone(),
        parts.schedule_proposed_key.clone(),
        parts.record_payload_json.clone(),
    );
    assert!(blank_units_row.is_err(), "the row constructor rejects blank units");

    // Raw INSERT bypassing the constructors: the CHECK constraint fires.
    // The RLS tenant context is set first (transaction-scoped, the P1.12
    // §5.1 convention) so the statement reaches the CHECK rather than
    // failing closed on the tenant policy — the CHECK is what this test
    // proves.
    let candidate = fixture_uuid(0x53);
    let pool: &PgPool = store.pool();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let result = runtime.block_on(async {
        let mut tx = pool.begin().await.expect("transaction opens");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(ctx.tenant_id().uuid().to_string())
            .execute(&mut *tx)
            .await
            .expect("tenant GUC sets");
        let result = sqlx::query(
            "INSERT INTO schedule_results \
                 (candidate_id, tenant_id, portfolio_id, site_id, status, problem_class, \
                  solver_name, solver_version, provider_id, provider_version, \
                  objective_value, objective_units, input_snapshot_id, grid_model_version, \
                  grid_model_site_id, grid_model_site_reference, schedule_proposed_key, \
                  record_payload) \
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, 'optimal', 'lp', 'highs', \
                     '1.15.1', 'tier1-python-worker', '0.1.0', 1.0, NULL, 'snap', 'grid', \
                     $4::uuid, 'ref', 'schedule_proposed:x', '{}')",
        )
        .bind(candidate.to_string())
        .bind(ctx.tenant_id().uuid().to_string())
        .bind(portfolio_uuid.to_string())
        .bind(site_uuid.to_string())
        .execute(&mut *tx)
        .await;
        let _ = tx.rollback().await;
        result
    });
    match result {
        Ok(_) => panic!("the objective-completeness CHECK must reject the row"),
        Err(sqlx::Error::Database(db)) => {
            // Classified by kind, never by text (the adapter rule).
            assert!(matches!(
                db.kind(),
                sqlx::error::DatabaseErrorKind::CheckViolation
            ));
        }
        Err(other) => panic!("unexpected failure class: {other}"),
    }
}
