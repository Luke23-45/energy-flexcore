// P3.22/P3.23 — Integration tests for the PostgreSQL schedule-revision
//         chain and comparison store (migration 0014; report §12.6, §14.2,
//         §16.1, §16.2; ledger P3.22/P3.23).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset):
//
// - the FULL revision chain (ledger P3.22 acceptance): solve (the P3.21
//   candidate records) → revise → the prior revision marked SUPERSEDED with
//   the bidirectional link COMPLETE IN BOTH DIRECTIONS → the old revision
//   still retrievable with its content intact; no in-place mutation
//   anywhere;
// - the gapless chain rule and the immutability conflict at the DATABASE
//   level (duplicate (schedule, revision) → Conflict; a revision > 1 whose
//   predecessor is still active → ChainBroken);
// - the SUPERSEDED transition is one-way (AlreadySuperseded) and revision
//   content is immutable (the migration's trigger + column grants reject a
//   raw payload UPDATE and a state regression; the row is unchanged after);
// - the Phase 4 gate check (`is_revision_superseded`): Some(true) superseded,
//   Some(false) active, None unknown; a foreign-tenant read is `None`
//   (existence never leaks, P0.16 rule 5);
// - the comparison record (P3.23): insert + find round-trip, duplicate
//   conflict, foreign-tenant read = `None`.
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Portfolio, Site, Tenant};
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use flexcore_storage::schedule_revision::{
    ScheduleRevisionChainStore, ScheduleRevisionError, ScheduleRevisionRow,
    ScheduleSupersedeTarget,
};
use flexcore_storage::schedule_store::{
    ScheduleComparisonRow, ScheduleComparisonStore, ScheduleRecordRow, ScheduleRecordStore,
};
use sqlx::PgPool;

// --- Harness helpers (the recorded P2.06/P3.21 decisions) -----------------------

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
    bytes[1] = 0xB3; // P3.22/P3.23 fixture marker
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
            subject: "p3.22-schedule-revisions-test".to_string(),
        },
    ))
}

fn seed_chain(store: &FlexCoreStore, tenant_uuid: FlexUuid) -> (TenantContext, FlexUuid, FlexUuid) {
    let ctx = tenant_context(tenant_uuid);
    let portfolio_uuid = fixture_uuid(0x32);
    let site_uuid = fixture_uuid(0x33);
    let created = EventTime::from_unix_millis(1_700_000_000_000).expect("valid");
    {
        let unit: PgUnitOfWork = store.begin(Some(&ctx)).expect("unit of work opens");
        let tenant = Tenant::try_new(
            TenantId::new(tenant_uuid),
            "p3.22 tenant".to_string(),
            None,
            format!("idp-{tenant_uuid}"),
            "default".to_string(),
            created,
        )
        .expect("tenant builds");
        flexcore_ports::repository::TenantRepository::insert(&unit, &tenant)
            .expect("tenant inserts");
        let portfolio = Portfolio::try_new(
            &tenant,
            PortfolioId::new(portfolio_uuid),
            "p3.22 portfolio".to_string(),
            None,
            "operator".to_string(),
            "none (D1 OPEN)".to_string(),
            "{}".to_string(),
            created,
        )
        .expect("portfolio builds");
        flexcore_ports::repository::PortfolioRepository::insert(&unit, &ctx, &portfolio)
            .expect("portfolio inserts");
        let site = Site::try_new(
            &portfolio,
            SiteId::new(site_uuid),
            "p3.22 site".to_string(),
            None,
            "synthetic".to_string(),
            SiteTimezone::new("UTC".to_string()).expect("UTC is valid"),
            "pcc-fixture".to_string(),
            "site-grid-model-reference-v1".to_string(),
            "edge-fixture".to_string(),
            created,
        )
        .expect("site builds");
        flexcore_ports::repository::SiteRepository::insert(&unit, &ctx, &site)
            .expect("site inserts");
        unit.commit().expect("committed");
    }
    (ctx, portfolio_uuid, site_uuid)
}

fn candidate_record_row(
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
        "{\"status\":\"optimal\"}".to_string(),
    )
    .expect("the candidate record row builds")
}

/// A revision row for `revision` of the given schedule chain, superseding
/// `prior_candidate` (None for revision 1) and realizing `candidate`.
fn revision_row(
    ctx: &TenantContext,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    schedule_uuid: FlexUuid,
    revision: u64,
    roll_trigger: &str,
    prior: Option<(u64, FlexUuid)>,
    candidate: FlexUuid,
) -> ScheduleRevisionRow {
    ScheduleRevisionRow::new(
        ctx.tenant_id(),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        schedule_uuid.to_string(),
        revision,
        1_700_000_000_000,
        1_700_008_640_000,
        roll_trigger.to_string(),
        prior.map(|(rev, _)| rev),
        prior.map(|(_, cand)| cand.to_string()),
        candidate.to_string(),
        format!(
            "{{\"revision\":{revision},\"intervals\":[{{\"t\":0,\"setpoints\":[]}}]}}"
        ),
    )
    .expect("the revision row builds")
}

/// Persists one P3.21 candidate record (the referential anchor for the
/// revision FK) inside the given unit of work.
fn insert_candidate(
    unit: &PgUnitOfWork,
    ctx: &TenantContext,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    candidate: FlexUuid,
) {
    unit.insert_schedule_record(
        ctx,
        &candidate_record_row(ctx, portfolio_uuid, site_uuid, candidate),
    )
    .expect("the candidate record inserts");
}

// --- Tests -----------------------------------------------------------------------

#[test]
fn the_full_revision_chain_supersedes_and_retains_the_old_revision() {
    let Some(url) = require_db("the_full_revision_chain_supersedes_and_retains_the_old_revision")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x31);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let schedule = fixture_uuid(0x34);
    let candidate1 = fixture_uuid(0x35);
    let candidate2 = fixture_uuid(0x36);

    // The full chain, ONE unit of work (the §15.5 transaction discipline):
    // the P3.21 candidate records, the SUPERSEDED marking of revision 1,
    // and the insert of revision 2.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate1);
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate2);

        let first = revision_row(
            &ctx,
            portfolio_uuid,
            site_uuid,
            schedule,
            1,
            "initial_solve",
            None,
            candidate1,
        );
        unit.insert_schedule_revision(&ctx, &first)
            .expect("revision 1 inserts (active)");

        // Revise: the prior revision is marked SUPERSEDED (the dedicated
        // transition) and the new revision inserted — same transaction.
        unit.mark_revision_superseded(
            &ctx,
            &schedule.to_string(),
            1,
            &ScheduleSupersedeTarget::new(2, candidate2.to_string()).expect("target"),
        )
        .expect("revision 1 is marked SUPERSEDED");
        let second = revision_row(
            &ctx,
            portfolio_uuid,
            site_uuid,
            schedule,
            2,
            "forecast_update",
            Some((1, candidate1)),
            candidate2,
        );
        unit.insert_schedule_revision(&ctx, &second)
            .expect("revision 2 inserts (active)");
        unit.commit().expect("committed");
    }

    // The old revision is STILL RETRIEVABLE with its content intact, the
    // supersede links are COMPLETE IN BOTH DIRECTIONS, and the chain is
    // queryable (retained for audit, §14.2).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let old = unit
            .find_revision(&ctx, &schedule.to_string(), 1)
            .expect("read succeeds")
            .expect("the superseded revision is retained for audit");
        assert!(old.is_superseded());
        assert_eq!(old.row.revision, 1);
        assert_eq!(old.row.candidate_id, candidate1.to_string());
        assert!(old.row.schedule_payload_json.contains("\"revision\":1"));
        assert_eq!(old.superseded_by_revision, Some(2));
        assert_eq!(old.superseded_by_candidate_id, Some(candidate2.to_string()));

        let new = unit
            .find_revision(&ctx, &schedule.to_string(), 2)
            .expect("read succeeds")
            .expect("the new revision is found");
        assert!(!new.is_superseded());
        assert_eq!(new.row.supersedes_revision, Some(1));
        assert_eq!(new.row.supersedes_candidate_id, Some(candidate1.to_string()));
        assert_eq!(new.row.roll_trigger, "forecast_update");

        let current = unit
            .current_revision(&ctx, &schedule.to_string())
            .expect("read succeeds")
            .expect("exactly one active revision");
        assert_eq!(current.row.revision, 2);

        let chain = unit
            .revision_chain(&ctx, &schedule.to_string())
            .expect("the chain reads");
        assert_eq!(
            chain.iter().map(|r| r.row.revision).collect::<Vec<_>>(),
            vec![1, 2]
        );

        // The Phase 4 gate check (P4.12 surface).
        assert_eq!(
            unit.is_revision_superseded(&ctx, &schedule.to_string(), 1)
                .expect("read succeeds"),
            Some(true)
        );
        assert_eq!(
            unit.is_revision_superseded(&ctx, &schedule.to_string(), 2)
                .expect("read succeeds"),
            Some(false)
        );
        assert_eq!(
            unit.is_revision_superseded(&ctx, &schedule.to_string(), 9)
                .expect("read succeeds"),
            None,
            "an unknown revision reads as None (the Phase 4 gate rejects it too)"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn a_duplicate_revision_conflicts_and_a_gapped_chain_is_rejected() {
    let Some(url) = require_db("a_duplicate_revision_conflicts_and_a_gapped_chain_is_rejected")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x41);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let schedule = fixture_uuid(0x42);
    let candidate1 = fixture_uuid(0x43);
    let candidate2 = fixture_uuid(0x44);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate1);
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate2);
        let first = revision_row(
            &ctx,
            portfolio_uuid,
            site_uuid,
            schedule,
            1,
            "initial_solve",
            None,
            candidate1,
        );
        unit.insert_schedule_revision(&ctx, &first)
            .expect("revision 1 inserts");

        // A duplicate (schedule, revision) pair conflicts — revisions are
        // immutable and a re-solve is a NEW revision.
        assert_eq!(
            unit.insert_schedule_revision(&ctx, &first),
            Err(ScheduleRevisionError::Conflict {
                schedule_id: schedule.to_string(),
                revision: 1,
            })
        );

        // Revision 2 while the predecessor is STILL ACTIVE is a broken
        // chain: the supersede marking and the insert belong to ONE
        // transaction.
        let gapped = revision_row(
            &ctx,
            portfolio_uuid,
            site_uuid,
            schedule,
            2,
            "forecast_update",
            Some((1, candidate1)),
            candidate2,
        );
        assert!(matches!(
            unit.insert_schedule_revision(&ctx, &gapped),
            Err(ScheduleRevisionError::ChainBroken { .. })
        ));
        unit.rollback().expect("rolled back");
    }
}

#[test]
fn the_supersede_transition_is_one_way_and_revision_content_is_immutable() {
    let Some(url) =
        require_db("the_supersede_transition_is_one_way_and_revision_content_is_immutable")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x51);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let schedule = fixture_uuid(0x52);
    let candidate1 = fixture_uuid(0x53);
    let candidate2 = fixture_uuid(0x54);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate1);
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate2);
        unit.insert_schedule_revision(
            &ctx,
            &revision_row(
                &ctx,
                portfolio_uuid,
                site_uuid,
                schedule,
                1,
                "initial_solve",
                None,
                candidate1,
            ),
        )
        .expect("revision 1 inserts");
        unit.mark_revision_superseded(
            &ctx,
            &schedule.to_string(),
            1,
            &ScheduleSupersedeTarget::new(2, candidate2.to_string()).expect("target"),
        )
        .expect("revision 1 marked SUPERSEDED");
        unit.insert_schedule_revision(
            &ctx,
            &revision_row(
                &ctx,
                portfolio_uuid,
                site_uuid,
                schedule,
                2,
                "forecast_update",
                Some((1, candidate1)),
                candidate2,
            ),
        )
        .expect("revision 2 inserts");
        unit.commit().expect("committed");
    }

    // One-way: marking revision 1 SUPERSEDED again is AlreadySuperseded.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        assert_eq!(
            unit.mark_revision_superseded(
                &ctx,
                &schedule.to_string(),
                1,
                &ScheduleSupersedeTarget::new(2, candidate2.to_string()).expect("target"),
            ),
            Err(ScheduleRevisionError::AlreadySuperseded {
                schedule_id: schedule.to_string(),
                revision: 1,
            })
        );
        unit.rollback().expect("rolled back");
    }

    // No in-place mutation anywhere: a raw payload UPDATE is rejected (the
    // runtime role has no grant on the content columns — the column-level
    // grant covers exactly the three transition columns), and a state
    // regression (superseded → active) is rejected by the immutability
    // trigger. In both cases the row is unchanged afterwards. The failure
    // classes differ (permission denied vs the trigger's raise exception);
    // the assertion is on the outcome, not the engine text (the
    // no-raw-storage-text rule).
    let pool: &PgPool = store.pool();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let payload_update = runtime.block_on(async {
        let mut tx = pool.begin().await.expect("transaction opens");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(ctx.tenant_id().uuid().to_string())
            .execute(&mut *tx)
            .await
            .expect("tenant GUC sets");
        let result = sqlx::query(
            "UPDATE schedule_revisions SET schedule_payload = '{\"tampered\":true}' \
             WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid AND revision = 1",
        )
        .bind(schedule.to_string())
        .bind(ctx.tenant_id().uuid().to_string())
        .execute(&mut *tx)
        .await;
        let _ = tx.rollback().await;
        result
    });
    assert!(payload_update.is_err(), "the payload UPDATE must be rejected");

    let state_regression = runtime.block_on(async {
        let mut tx = pool.begin().await.expect("transaction opens");
        sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
            .bind(ctx.tenant_id().uuid().to_string())
            .execute(&mut *tx)
            .await
            .expect("tenant GUC sets");
        let result = sqlx::query(
            "UPDATE schedule_revisions SET state = 'active', \
                     superseded_by_revision = NULL, superseded_by_candidate_id = NULL \
             WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid AND revision = 1",
        )
        .bind(schedule.to_string())
        .bind(ctx.tenant_id().uuid().to_string())
        .execute(&mut *tx)
        .await;
        let _ = tx.rollback().await;
        result
    });
    assert!(
        state_regression.is_err(),
        "a state regression must be rejected by the immutability trigger"
    );

    // The row is unchanged after both attempts.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let old = unit
            .find_revision(&ctx, &schedule.to_string(), 1)
            .expect("read succeeds")
            .expect("the revision is found");
        assert!(old.is_superseded(), "the state is unchanged");
        assert!(old.row.schedule_payload_json.contains("\"revision\":1"));
        assert!(!old.row.schedule_payload_json.contains("tampered"));
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn a_foreign_tenant_read_returns_none() {
    let Some(url) = require_db("a_foreign_tenant_read_returns_none") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x61);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let schedule = fixture_uuid(0x62);
    let candidate1 = fixture_uuid(0x63);
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        insert_candidate(&unit, &ctx, portfolio_uuid, site_uuid, candidate1);
        unit.insert_schedule_revision(
            &ctx,
            &revision_row(
                &ctx,
                portfolio_uuid,
                site_uuid,
                schedule,
                1,
                "initial_solve",
                None,
                candidate1,
            ),
        )
        .expect("revision 1 inserts");
        unit.commit().expect("committed");
    }

    let foreign_ctx = tenant_context(fixture_uuid(0x64));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        assert!(
            unit.find_revision(&foreign_ctx, &schedule.to_string(), 1)
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant revision reads as None"
        );
        assert_eq!(
            unit.is_revision_superseded(&foreign_ctx, &schedule.to_string(), 1)
                .expect("read succeeds"),
            None,
            "existence never leaks across a tenant boundary (P0.16 rule 5)"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn the_comparison_record_round_trips_and_is_tenant_scoped() {
    let Some(url) = require_db("the_comparison_record_round_trips_and_is_tenant_scoped") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x71);
    let (ctx, portfolio_uuid, site_uuid) = seed_chain(&store, tenant_uuid);
    let comparison = fixture_uuid(0x72);

    let row = ScheduleComparisonRow::new(
        ctx.tenant_id(),
        comparison.to_string(),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        "schedule_input_snapshot:env-v1:v1:synthetic:telemetry=t1".to_string(),
        "env-v1".to_string(),
        "v1".to_string(),
        "synthetic".to_string(),
        "default-synthetic-v1".to_string(),
        2,
        "[{\"candidate_id\":\"c1\",\"objective_value\":5.0,\"schedule_deviation_mw\":0.0,\
           \"feasible_interval_count\":1,\"runtime_ms\":3},\
          {\"candidate_id\":\"c2\",\"objective_value\":4.0,\"schedule_deviation_mw\":0.0,\
           \"feasible_interval_count\":1,\"runtime_ms\":5}]"
            .to_string(),
    )
    .expect("the comparison row builds");

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        unit.insert_schedule_comparison(&ctx, &row)
            .expect("the comparison inserts");
        unit.commit().expect("committed");
    }

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let found = unit
            .find_schedule_comparison(&ctx, &comparison.to_string())
            .expect("read succeeds")
            .expect("the committed comparison is visible");
        assert_eq!(found, row);
        unit.rollback().expect("read-only unit rolled back");
    }

    // Immutable: a duplicate identity conflicts.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        assert!(matches!(
            unit.insert_schedule_comparison(&ctx, &row),
            Err(flexcore_storage::schedule_store::ComparisonStoreError::Conflict { .. })
        ));
        unit.rollback().expect("rolled back");
    }

    // A foreign tenant reads `None` (P0.16 rule 5).
    let foreign_ctx = tenant_context(fixture_uuid(0x73));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        assert!(
            unit.find_schedule_comparison(&foreign_ctx, &comparison.to_string())
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant comparison reads as None"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}
