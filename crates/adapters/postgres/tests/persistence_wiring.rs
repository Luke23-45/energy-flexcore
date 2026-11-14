// P4.30 — Integration tests for the persistence wiring (report §15.5,
//         §16.1–§16.2, §21.3; ledger P4.30; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md`).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset):
//
// - **Transition + evidence persistence (§16.1)**: the command-record
//   state change, its immutable journal row (carrying the
//   validation/feasibility/authorization EVIDENCE REFERENCES — §14.1: the
//   three gates are distinct), and the §12.5 feasibility-check evidence
//   record (migration 0017) commit in ONE unit-of-work transaction and
//   read back through the tenant-scoped stores;
// - **Append-only + tenant isolation (§16.2)**: feasibility-check rows
//   refuse UPDATE and DELETE (grant + trigger), a duplicate (check_id,
//   tenant_id) is the named Conflict, and a foreign-tenant check reads as
//   `None` and leaks nothing into listings (P0.16 rule 5, RLS + explicit
//   filter);
// - **Outbox idempotent consumption (P4.24 semantics, §15.5)**: a
//   redelivered outbox write under an existing idempotency key is refused
//   with the durable DuplicateKey error — within one transaction and
//   across transactions — so exactly one effective row exists per key and
//   consumers can deduplicate durably.
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, EvidenceReference, Gate};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use flexcore_storage::command_store::{
    CommandListFilter, CommandRecordRow, CommandRecordStore, CommandStoreError,
    CommandTransitionRecord,
};
use flexcore_storage::feasibility_check::{
    FeasibilityCheckFilter, FeasibilityCheckRow, FeasibilityCheckStore,
    FeasibilityCheckStoreError,
};
use flexcore_storage::schedule_revision::ScheduleRevisionRow;
use flexcore_storage::schedule_store::{ScheduleRecordRow, ScheduleRecordStore};

// --- Harness helpers (the recorded P2.06/P3.21/P4.10 decisions) ---------------

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
    bytes[1] = 0xC3; // P4.30 fixture marker
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
            subject: "p4.30-persistence-test".to_string(),
        },
    ))
}

/// Seeds tenant + portfolio + site + asset (the dispatch_commands and
/// feasibility_check_records FK chain) in one committed unit of work.
fn seed_environment(
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
        "p4.30 tenant".to_string(),
        None,
        format!("idp-{tenant_uuid}"),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    flexcore_ports::repository::TenantRepository::insert(&unit, &tenant).expect("tenant inserts");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(portfolio_uuid),
        "p4.30 portfolio".to_string(),
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
        "p4.30 site".to_string(),
        None,
        "synthetic".to_string(),
        flexcore_contracts::time_model::SiteTimezone::new("UTC".to_string()).expect("UTC"),
        "pcc-fixture".to_string(),
        "site-grid-model-reference-v1".to_string(),
        "edge-fixture".to_string(),
        created,
    )
    .expect("site builds");
    flexcore_ports::repository::SiteRepository::insert(&unit, &ctx, &site).expect("site inserts");
    let asset = Asset::try_new(
        &site,
        AssetId::new(asset_uuid),
        "p4.30 BESS".to_string(),
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
    flexcore_ports::repository::AssetRepository::insert(&unit, &ctx, &asset)
        .expect("asset inserts");
    unit.commit().expect("committed");
    (ctx, portfolio_uuid, site_uuid, asset_uuid)
}

/// Seeds the P3.21 candidate record + the ACTIVE schedule revision 1 the
/// command's and the check record's composite FK requires (P3.22).
fn seed_schedule_revision(
    unit: &PgUnitOfWork,
    ctx: &TenantContext,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    schedule_uuid: FlexUuid,
    candidate_uuid: FlexUuid,
) {
    let candidate = ScheduleRecordRow::new(
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
        "schedule_input_snapshot:envelope-v1:forecast-v1:synthetic:fnv1a64:00000000000000b1"
            .to_string(),
        "grid-model-v1".to_string(),
        site_uuid.to_string(),
        "site-grid-model-reference-v1".to_string(),
        format!("schedule_proposed:{portfolio_uuid}:{site_uuid}:{candidate_uuid}"),
        "{\"status\":\"optimal\"}".to_string(),
    )
    .expect("the candidate record row builds");
    unit.insert_schedule_record(ctx, &candidate)
        .expect("the candidate record inserts");
    let revision = ScheduleRevisionRow::new(
        ctx.tenant_id(),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        schedule_uuid.to_string(),
        1,
        1_700_000_000_000,
        1_700_008_640_000,
        "initial_solve".to_string(),
        None,
        None,
        candidate_uuid.to_string(),
        "{\"revision\":1,\"intervals\":[{\"t\":0,\"setpoints\":[]}]}".to_string(),
    )
    .expect("the revision row builds");
    unit.insert_schedule_revision(ctx, &revision)
        .expect("revision 1 inserts (active)");
}

fn command_row(
    ctx: &TenantContext,
    command_uuid: FlexUuid,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    asset_uuid: FlexUuid,
    schedule_uuid: FlexUuid,
) -> CommandRecordRow {
    CommandRecordRow::new(
        ctx.tenant_id(),
        command_uuid.to_string(),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        asset_uuid.to_string(),
        schedule_uuid.to_string(),
        1,
        1_700_000_200_000,
        None,
        "{\"target_p_mw\":1.5,\"units\":\"MW\",\"sign\":\"export_positive\"}".to_string(),
    )
    .expect("the command row builds")
}

/// Builds one validated journal record through the storage constructor
/// (the frozen-edge and reason/evidence/gate rules are enforced there).
fn transition_record(
    ctx: &TenantContext,
    command: CommandId,
    from: CommandState,
    to: CommandState,
    at_unix_millis: i64,
    gate: Option<Gate>,
    evidence: &str,
) -> CommandTransitionRecord {
    let transition = CommandTransition {
        command_id: command,
        from,
        to,
        actor: Actor::new("svc:p4.30-test").expect("actor builds"),
        at_unix_millis,
        gate,
        reason: None,
        evidence: Some(EvidenceReference::new(evidence).expect("evidence builds")),
    };
    CommandTransitionRecord::from_transition(ctx.tenant_id(), &transition)
        .expect("the transition record builds (the edge is in the frozen table)")
}

fn check_row(
    ctx: &TenantContext,
    check_id: &str,
    command_uuid: FlexUuid,
    portfolio_uuid: FlexUuid,
    site_uuid: FlexUuid,
    asset_uuid: FlexUuid,
    schedule_uuid: FlexUuid,
    checked_at: i64,
) -> FeasibilityCheckRow {
    FeasibilityCheckRow::new(
        ctx.tenant_id(),
        check_id.to_string(),
        Some(command_uuid.to_string()),
        portfolio_uuid.to_string(),
        site_uuid.to_string(),
        asset_uuid.to_string(),
        schedule_uuid.to_string(),
        1,
        Some("feeder-a".to_string()),
        Some("grid-model-v1".to_string()),
        Some("fnv1a64:deadbeef".to_string()),
        "PASSED".to_string(),
        None,
        None,
        Some("margins within tolerance".to_string()),
        Some("balanced radial feeder".to_string()),
        "svc:grid-gate".to_string(),
        format!("trace-{check_id}"),
        checked_at,
    )
    .expect("the check row builds")
}

fn sample_outbox_write(receipt: StateChangeReceipt, key: &str) -> OutboxWrite {
    let payload = format!(
        r#"{{"schema_version": 1, "idempotency_key": "{key}", "event": {{"event_type": "FEASIBILITY_CHECK_COMPLETED"}}}}"#
    );
    OutboxWrite::new(
        receipt,
        key.to_string(),
        "flexcore.v1.validation".to_string(),
        payload,
        EventTime::from_unix_millis(1_700_000_100_000).expect("valid"),
    )
    .expect("the outbox write is well-formed")
}

// --- Tests ---------------------------------------------------------------------

#[test]
fn a_transition_its_evidence_and_the_check_record_persist_in_one_transaction() {
    let Some(url) =
        require_db("a_transition_its_evidence_and_the_check_record_persist_in_one_transaction")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xB1);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0xB5);
    let candidate = fixture_uuid(0xB6);
    let command = fixture_uuid(0xB7);
    let command_id = CommandId::new(command);

    // ONE unit of work: the VALIDATED transition, the FEASIBLE transition
    // (each journal row carrying its DISTINCT evidence reference — the
    // §14.1 anti-conflation rule), and the feasibility-check evidence
    // record (migration 0017). All commit together or not at all (§15.5).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule))
            .expect("the command inserts as PROPOSED");
        unit.record_command_transition(
            &ctx,
            &transition_record(
                &ctx,
                command_id,
                CommandState::Proposed,
                CommandState::Validated,
                1_700_000_100_000,
                Some(Gate::Validation),
                "evidence:validation-v1",
            ),
        )
        .expect("the validation transition commits");
        unit.record_command_transition(
            &ctx,
            &transition_record(
                &ctx,
                command_id,
                CommandState::Validated,
                CommandState::Feasible,
                1_700_000_101_000,
                Some(Gate::Feasibility),
                "feasibility:attempt/attempt-1:model/feeder-a:hash/fnv1a64:deadbeef",
            ),
        )
        .expect("the feasibility transition commits");
        unit.insert_feasibility_check(
            &ctx,
            &check_row(&ctx, "attempt-1", command, portfolio, site, asset, schedule, 1_700_000_101_500),
        )
        .expect("the check record commits in the SAME transaction");
        unit.commit().expect("committed");
    }

    // Everything reads back: the journal carries the DISTINCT evidence
    // references per gate, the command is FEASIBLE, the check record is
    // found and lists under its command and schedule.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let journal = unit
            .transition_journal(&ctx, &command.to_string())
            .expect("the journal reads");
        assert_eq!(journal.len(), 2);
        assert_eq!(journal[0].to_state, "VALIDATED");
        assert_eq!(journal[0].gate.as_deref(), Some("validation"));
        assert_eq!(journal[0].evidence_reference.as_deref(), Some("evidence:validation-v1"));
        assert_eq!(journal[1].to_state, "FEASIBLE");
        assert_eq!(journal[1].gate.as_deref(), Some("feasibility"));
        assert_eq!(
            journal[1].evidence_reference.as_deref(),
            Some("feasibility:attempt/attempt-1:model/feeder-a:hash/fnv1a64:deadbeef"),
            "the feasibility evidence is a SEPARATE reference (§14.1)"
        );
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the command is found");
        assert_eq!(record.state, "FEASIBLE");

        let check = unit
            .find_feasibility_check(&ctx, "attempt-1")
            .expect("read succeeds")
            .expect("the check record is found");
        assert_eq!(check.outcome, "PASSED");
        assert_eq!(check.command_id.as_deref(), Some(command.to_string()).as_deref());
        assert_eq!(check.model_content_hash.as_deref(), Some("fnv1a64:deadbeef"));
        assert_eq!(check.model_id.as_deref(), Some("feeder-a"));
        assert_eq!(check.model_version.as_deref(), Some("grid-model-v1"));
        assert_eq!(check.actor, "svc:grid-gate");
        assert_eq!(check.correlation_id, "trace-attempt-1");

        let listed = unit
            .list_feasibility_checks(
                &ctx,
                &FeasibilityCheckFilter {
                    command_id: Some(command.to_string()),
                    schedule_id: None,
                    after_check_id: None,
                },
                100,
            )
            .expect("the listing reads");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].check_id, "attempt-1");
        let by_schedule = unit
            .list_feasibility_checks(
                &ctx,
                &FeasibilityCheckFilter {
                    command_id: None,
                    schedule_id: Some(schedule.to_string()),
                    after_check_id: None,
                },
                100,
            )
            .expect("the listing reads");
        assert_eq!(by_schedule.len(), 1);

        // The command listing (the §15.2 resource's store surface) serves
        // the record, newest first, tenant-scoped.
        let commands = unit
            .list_commands(&ctx, &CommandListFilter::default(), 100)
            .expect("the command listing reads");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].row.command_id, command.to_string());
        let validated_only = unit
            .list_commands(
                &ctx,
                &CommandListFilter {
                    state: Some("VALIDATED".to_string()),
                    after_command_id: None,
                },
                100,
            )
            .expect("the state-filtered listing reads");
        assert!(validated_only.is_empty(), "the command is FEASIBLE, not VALIDATED");
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn feasibility_check_records_are_append_only_and_tenant_scoped() {
    let Some(url) = require_db("feasibility_check_records_are_append_only_and_tenant_scoped")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xC1);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0xC2);
    let candidate = fixture_uuid(0xC3);
    let command = fixture_uuid(0xC4);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule))
            .expect("the command inserts");
        unit.insert_feasibility_check(
            &ctx,
            &check_row(&ctx, "attempt-1", command, portfolio, site, asset, schedule, 1_700_000_101_500),
        )
        .expect("the check record inserts");
        // A duplicate (check_id, tenant_id) is the named immutability
        // conflict (check rows are immutable; a re-checked candidate is a
        // NEW check id).
        assert_eq!(
            unit.insert_feasibility_check(
                &ctx,
                &check_row(&ctx, "attempt-1", command, portfolio, site, asset, schedule, 1_700_000_101_500),
            ),
            Err(FeasibilityCheckStoreError::Conflict {
                check_id: "attempt-1".to_string(),
            })
        );
        unit.commit().expect("committed");
    }

    // A foreign-tenant check reads as None and leaks nothing into the
    // listings (P0.16 rule 5; RLS + the explicit tenant filter).
    let foreign_ctx = tenant_context(fixture_uuid(0xC5));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        assert!(
            unit.find_feasibility_check(&foreign_ctx, "attempt-1")
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant check reads as None"
        );
        assert!(
            unit.list_feasibility_checks(&foreign_ctx, &FeasibilityCheckFilter::default(), 100)
                .expect("read succeeds")
                .is_empty(),
            "existence never leaks across a tenant boundary"
        );
        unit.rollback().expect("read-only unit rolled back");
    }

    // Append-only at the DATABASE level (migration 0017's grant + trigger):
    // UPDATE and DELETE are refused, and an outcome outside the closed
    // vocabulary is refused by the CHECK. The failure CLASSES differ
    // (permission denied vs the trigger's raise); the assertion is on the
    // outcome, never the engine text.
    let pool: &sqlx::PgPool = store.pool();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let with_tenant = |statement: &'static str| {
        let tenant_text = ctx.tenant_id().uuid().to_string();
        runtime.block_on(async move {
            let mut tx = pool.begin().await.expect("transaction opens");
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(tenant_text)
                .execute(&mut *tx)
                .await
                .expect("tenant GUC sets");
            let result = sqlx::query(statement).execute(&mut *tx).await;
            let _ = tx.rollback().await;
            result
        })
    };
    assert!(
        with_tenant(
            "UPDATE feasibility_check_records SET outcome = 'PASSED' \
             WHERE check_id = 'attempt-1'"
        )
        .is_err(),
        "a check-record UPDATE must be rejected (append-only)"
    );
    assert!(
        with_tenant(
            "DELETE FROM feasibility_check_records WHERE check_id = 'attempt-1'"
        )
        .is_err(),
        "a check-record DELETE must be rejected (append-only)"
    );
    assert!(
        with_tenant(
            "INSERT INTO feasibility_check_records \
                 (check_id, tenant_id, command_id, portfolio_id, site_id, asset_id, \
                  schedule_id, schedule_revision, model_content_hash, outcome, actor, \
                  correlation_id, checked_at_unix_millis) \
             SELECT 'attempt-illegal', tenant_id, command_id, portfolio_id, site_id, \
                    asset_id, schedule_id, schedule_revision, model_content_hash, \
                    'PROBABLY_FINE', actor, correlation_id, checked_at_unix_millis \
             FROM feasibility_check_records WHERE check_id = 'attempt-1'"
        )
        .is_err(),
        "an outcome outside the closed vocabulary must be rejected"
    );

    // The row is unchanged after every refused statement.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let check = unit
            .find_feasibility_check(&ctx, "attempt-1")
            .expect("read succeeds")
            .expect("the check record is found");
        assert_eq!(check.outcome, "PASSED", "the outcome is unchanged");
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn an_outbox_key_is_consumed_idempotently_p4_24() {
    let Some(url) = require_db("an_outbox_key_is_consumed_idempotently_p4_24") else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xD1);
    let (ctx, _portfolio, _site, _asset) = seed_environment(&store, tenant_uuid);
    let key = format!("feasibility_check_completed:{}", fixture_uuid(0xD2));
    let receipt = StateChangeReceipt::new(
        "dispatch_commands".to_string(),
        StateChangeOperation::Insert,
        Some(ctx.tenant_id()),
    );

    // The FIRST write under the key is accepted; the redelivered write
    // (at-least-once delivery, §15.4) is refused with the durable
    // DuplicateKey (P1.12 §4.6 unique constraint; the P4.24 idempotent-
    // consumer semantics) — within one transaction...
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        EventOutbox::append(&unit, sample_outbox_write(receipt.clone(), &key))
            .expect("the first write under the key is accepted");
        assert!(
            unit.pending_entries(10)
                .expect("pending reads")
                .iter()
                .any(|e| e.idempotency_key() == key)
        );
        let replay = match EventOutbox::append(&unit, sample_outbox_write(receipt.clone(), &key)) {
            Err(EventOutboxError::DuplicateKey { idempotency_key }) => idempotency_key,
            other => panic!("the redelivered key must be refused, got {other:?}"),
        };
        assert_eq!(replay, key);
        unit.rollback().expect("rolled back (first write not kept)");
    }
    // ...and across transactions (durable uniqueness, not an in-memory
    // dedup): the first write COMMITS, then a redelivery in a NEW unit of
    // work is still refused, and exactly ONE row exists for the key.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        EventOutbox::append(&unit, sample_outbox_write(receipt.clone(), &key))
            .expect("the first committed write under the key");
        unit.commit().expect("committed");
    }
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let replay = match EventOutbox::append(&unit, sample_outbox_write(receipt.clone(), &key)) {
            Err(EventOutboxError::DuplicateKey { idempotency_key }) => idempotency_key,
            other => panic!("the redelivered key must be refused durably, got {other:?}"),
        };
        assert_eq!(replay, key);
        unit.rollback().expect("rolled back");
    }
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let pending = unit.pending_entries(100).expect("pending reads");
        assert_eq!(
            pending
                .iter()
                .filter(|e| e.idempotency_key() == key)
                .count(),
            1,
            "exactly ONE effective outbox row per idempotency key (P4.24)"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}
