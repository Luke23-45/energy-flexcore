// P4.31 — Integration tests for the §24 vertical-slice dispatch path
//         (report §19.2 layer 4, §24 steps 5–8, §15.5, §16.1–§16.2; ledger
//         P4.31 step 2; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md`).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset; these
// tests are UNRUN in this subtask, the human/CI run is the gate):
//
// - **The §24 steps 5–8 path, end to end over PostgreSQL**: the command
//   is PERSISTED (PROPOSED row), the gate records commit with DISTINCT
//   per-gate evidence references (validation ≠ feasibility — the §14.1
//   anti-conflation rule), the §12.5 feasibility-check evidence record
//   (migration 0017) carries the grid-model version + hash + outcome, the
//   events sit in the TRANSACTIONAL OUTBOX (§15.5) as the idempotency-
//   keyed publish records, and everything READS BACK through the store
//   surface the §15.2 read APIs (`/api/v1/dispatch-commands`,
//   `/api/v1/feasibility-checks`, P4.30) are served from.
//
// ## The integration seams (recorded honestly — the ledger step names
//    PostgreSQL, NATS, the Python power-flow worker, and the simulator
//    adapter TOGETHER; each leg's real execution is a human-run gate
//    input, not part of this subtask's banned-run scope):
//
// - **PostgreSQL**: REAL — this suite runs against the service PostgreSQL
//   (CI provides it; locally the human sets FLEXCORE_TEST_DATABASE_URL).
// - **NATS**: the REAL SEAM is the outbox row surface — apps/worker's
//   `OutboxPublisher` (P1.15/P1.19) polls exactly `pending_entries` and
//   publishes each entry to JetStream under its idempotency key, marking
//   delivered only after the broker ack. This suite asserts the seam's
//   INPUT records (keys, subjects, attempt counts, zero-loss drain) and
//   the durable duplicate refusal; the REAL broker publish is the CI
//   integration-tests job (FLEXCORE_TEST_NATS_URL) — recorded, not faked.
// - **Python power-flow worker (pandapower 3.5.6)**: the REAL SEAM is the
//   feasibility-check record + the feasibility evidence reference the
//   gate journals — the record the grid-model port (flexcore-grid-api,
//   P4.02) produces per completed §12.5 check. The worker is PINNED but
//   NOT INSTALLED in this environment (recorded in the decision record):
//   the REAL pandapower call is the human's run — this suite asserts the
//   RECORD the port's result leaves behind, never a power-flow verdict of
//   its own. AC power flow is an evaluation, not AC-OPF; no solver-class
//   claim is made or tested here.
// - **Simulator adapter**: the §24 step 6–8 issue/ack/tracking machinery
//   is covered by the flexcore-application suites over the transport port
//   (P4.16); this suite covers the persistence half (the command rows,
//   journal, and evidence the adapter reads through the store seam).
//
// Roles: the runtime connects as the configured URL's role; the migration
// runner needs the owner role (`flexcore_migrator`) or a bootstrap
// administrator, per the P1.13 conventions (mirrors
// `persistence_wiring.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::command_lifecycle::{Actor, EvidenceReference, Gate};
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
};
use flexcore_storage::schedule_revision::ScheduleRevisionRow;
use flexcore_storage::schedule_store::{ScheduleRecordRow, ScheduleRecordStore};

// --- Harness helpers (the recorded P2.06/P3.21/P4.10/P4.30 decisions) ----------

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
    bytes[1] = 0xE1; // P4.31 integration fixture marker
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
            subject: "p4.31-integration-test".to_string(),
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
        "p4.31 tenant".to_string(),
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
        "p4.31 portfolio".to_string(),
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
        "p4.31 site".to_string(),
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
        "p4.31 BESS".to_string(),
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
    let transition = flexcore_domain::command_lifecycle::CommandTransition {
        command_id: command,
        from,
        to,
        actor: Actor::new("svc:p4.31-test").expect("actor builds"),
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
        Some("fnv1a64:p431modelhash".to_string()),
        "PASSED".to_string(),
        None,
        None,
        Some("min v_pu 0.97; max loading 71%".to_string()),
        Some("balanced radial feeder; unbalanced deferred (P4.34)".to_string()),
        "svc:grid-gate".to_string(),
        format!("trace-{check_id}"),
        checked_at,
    )
    .expect("the check row builds")
}

/// One §15.3 dispatch outbox write (the publish record the worker's
/// OutboxPublisher polls — the NATS seam input).
fn dispatch_outbox_write(receipt: StateChangeReceipt, key: &str, payload_event: &str) -> OutboxWrite {
    let payload = format!(
        r#"{{"schema_version": 1, "idempotency_key": "{key}", "event": {{"event_type": "{payload_event}"}}}}"#
    );
    OutboxWrite::new(
        receipt,
        key.to_string(),
        "flexcore.v1.dispatch".to_string(),
        payload,
        EventTime::from_unix_millis(1_700_000_300_000).expect("valid"),
    )
    .expect("the outbox write is well-formed")
}

// --- The vertical slice ---------------------------------------------------

/// The §24 steps 5–8 path, persisted end to end: command → gate records
/// (distinct evidence) → feasibility-check record → outbox publish
/// records → read back through the §15.2 store surface.
#[test]
fn the_p4_vertical_slice_persists_gates_outbox_and_reads_back_end_to_end() {
    let Some(url) =
        require_db("the_p4_vertical_slice_persists_gates_outbox_and_reads_back_end_to_end")
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

    // ONE unit of work per §6.3/§15.5: the command record, the gate
    // journal rows (DISTINCT evidence per gate — §14.1), the §12.5
    // feasibility-check record (the grid-model port's output record — the
    // pandapower worker leg's SEAM; the real 3.5.6 call is the human's
    // run), and the §15.3 outbox publish records (the NATS leg's SEAM).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        // §24 step 5 (command landed): the PROPOSED command record.
        unit.insert_command_record(
            &ctx,
            &command_row(&ctx, command, portfolio, site, asset, schedule),
        )
        .expect("the command inserts as PROPOSED");
        // §24 step 6 path: the validation gate record...
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
        // ...the §12.5 check record (the power-flow SEAM record)...
        unit.insert_feasibility_check(
            &ctx,
            &check_row(&ctx, "attempt-1", command, portfolio, site, asset, schedule, 1_700_000_101_500),
        )
        .expect("the check record commits in the SAME transaction");
        // ...and the feasibility gate record (evidence binds THE MODEL
        // HASH — §17.5 invariant 3), a SEPARATE reference (§14.1).
        unit.record_command_transition(
            &ctx,
            &transition_record(
                &ctx,
                command_id,
                CommandState::Validated,
                CommandState::Feasible,
                1_700_000_101_000,
                Some(Gate::Feasibility),
                "feasibility:attempt/attempt-1:model/feeder-a:hash/fnv1a64:p431modelhash",
            ),
        )
        .expect("the feasibility transition commits");
        // §24 step 7–8 seam: the §15.3 publish records in the
        // transactional outbox (the worker's OutboxPublisher drains these
        // into NATS under their idempotency keys).
        let receipt = StateChangeReceipt::new(
            "dispatch_commands".to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        );
        EventOutbox::append(
            &unit,
            dispatch_outbox_write(receipt.clone(), "dispatch_issued:p431-slice-1", "DISPATCH_ISSUED"),
        )
        .expect("the first publish record appends");
        EventOutbox::append(
            &unit,
            dispatch_outbox_write(receipt, "dispatch_acknowledged:p431-slice-1", "DISPATCH_ACKNOWLEDGED"),
        )
        .expect("the second publish record appends");
        unit.commit().expect("the whole slice commits atomically");
    }

    // READ BACK through the store surface the §15.2 read APIs are served
    // from (P4.30): the command + journal + check record + listings.
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
            Some("feasibility:attempt/attempt-1:model/feeder-a:hash/fnv1a64:p431modelhash"),
            "the feasibility evidence is a SEPARATE reference and binds the model hash (§14.1, §17.5 inv. 3)"
        );
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the command is found");
        assert_eq!(record.state, "FEASIBLE");

        // The §12.5 check record: the power-flow SEAM's output record
        // (model id/version/hash + outcome + coverage — no power-flow
        // verdict is produced HERE; the real pandapower call is the
        // human's run, recorded in the decision record).
        let check = unit
            .find_feasibility_check(&ctx, "attempt-1")
            .expect("read succeeds")
            .expect("the check record is found");
        assert_eq!(check.outcome, "PASSED");
        assert_eq!(check.command_id.as_deref(), Some(command.to_string()).as_deref());
        assert_eq!(check.model_id.as_deref(), Some("feeder-a"));
        assert_eq!(check.model_version.as_deref(), Some("grid-model-v1"));
        assert_eq!(check.model_content_hash.as_deref(), Some("fnv1a64:p431modelhash"));
        assert_eq!(
            check.coverage.as_deref(),
            Some("balanced radial feeder; unbalanced deferred (P4.34)")
        );
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

        // The command listing (GET /api/v1/dispatch-commands' store
        // surface) serves the record, tenant-scoped.
        let commands = unit
            .list_commands(&ctx, &CommandListFilter::default(), 100)
            .expect("the command listing reads");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].row.command_id, command.to_string());
        let feasible_only = unit
            .list_commands(
                &ctx,
                &CommandListFilter {
                    state: Some("FEASIBLE".to_string()),
                    after_command_id: None,
                },
                100,
            )
            .expect("the state-filtered listing reads");
        assert_eq!(feasible_only.len(), 1);
        unit.rollback().expect("read-only unit rolled back");
    }

    // THE NATS SEAM (input records): the publish records are PENDING with
    // their idempotency keys, destination subjects, and zero attempts —
    // exactly the surface apps/worker's OutboxPublisher polls. The REAL
    // broker publish is the CI/human run (recorded, not faked here).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let pending = unit.pending_entries(100).expect("pending reads");
        let keys: Vec<&str> = pending
            .iter()
            .map(|entry| entry.idempotency_key())
            .collect();
        assert!(keys.contains(&"dispatch_issued:p431-slice-1"));
        assert!(keys.contains(&"dispatch_acknowledged:p431-slice-1"));
        for entry in &pending {
            if entry.idempotency_key().starts_with("dispatch_") {
                assert_eq!(entry.destination_subject(), "flexcore.v1.dispatch");
                assert_eq!(entry.attempt_count(), 0);
            }
        }
        unit.rollback().expect("read-only unit rolled back");
    }
}

/// The slice's append-only + tenant-isolation guarantees over the §24
/// path: a foreign tenant reads nothing (existence never leaks, P0.16
/// rule 5), the compare-and-swap refuses a replayed transition (a
/// redelivered gate message cannot corrupt the journal), and the check
/// record refuses updates (append-only evidence).
#[test]
fn the_slice_is_append_only_and_tenant_scoped_under_fault_conditions() {
    let Some(url) = require_db("the_slice_is_append_only_and_tenant_scoped_under_fault_conditions")
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
    let command_id = CommandId::new(command);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(
            &ctx,
            &command_row(&ctx, command, portfolio, site, asset, schedule),
        )
        .expect("the command inserts");
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
        unit.insert_feasibility_check(
            &ctx,
            &check_row(&ctx, "attempt-1", command, portfolio, site, asset, schedule, 1_700_000_101_500),
        )
        .expect("the check record inserts");
        unit.commit().expect("committed");
    }

    // FAULT-adjacent guard 1: a REDELIVERED gate message (the same
    // transition replayed — the at-least-once shape) is refused by the
    // compare-and-swap: the command is already VALIDATED, so a second
    // PROPOSED → VALIDATED row cannot be journalled. No corruption.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        assert_eq!(
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
            ),
            Err(CommandStoreError::TransitionConflict { .. }),
            "a redelivered VALIDATED gate message is refused by the CAS"
        );
        unit.rollback().expect("rolled back");
    }

    // FAULT-adjacent guard 2: a foreign tenant reads NOTHING — the
    // command, its journal, and its check record do not exist across the
    // tenant boundary (P0.16 rule 5; RLS + explicit filters).
    let foreign_ctx = tenant_context(fixture_uuid(0xC5));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        assert!(
            unit.find_command(&foreign_ctx, &command.to_string())
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant command reads as None"
        );
        assert!(
            unit.transition_journal(&foreign_ctx, &command.to_string())
                .expect("read succeeds")
                .is_empty(),
            "a foreign-tenant journal leaks nothing"
        );
        assert!(
            unit.find_feasibility_check(&foreign_ctx, "attempt-1")
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant check record reads as None"
        );
        assert!(
            unit.list_commands(&foreign_ctx, &CommandListFilter::default(), 100)
                .expect("read succeeds")
                .is_empty(),
            "the command listing leaks nothing"
        );
        unit.rollback().expect("read-only unit rolled back");
    }

    // Append-only at the DATABASE level: the check record refuses UPDATE
    // and DELETE (the 0017 grant + trigger), so the evidence the slice
    // rests on cannot be rewritten after the fact.
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
            "UPDATE feasibility_check_records SET outcome = 'FAIL_CLOSED' \
             WHERE check_id = 'attempt-1'"
        )
        .is_err(),
        "a check-record UPDATE must be rejected (append-only evidence)"
    );
    assert!(
        with_tenant(
            "DELETE FROM feasibility_check_records WHERE check_id = 'attempt-1'"
        )
        .is_err(),
        "a check-record DELETE must be rejected (append-only evidence)"
    );
}

/// The slice's publish records drain deterministically with ZERO loss at
/// the NATS seam: a failed publish attempt leaves the entry PENDING with
/// its attempt count; recovery marks it delivered; a redelivered append
/// under an existing key is refused durably (DuplicateKey) — the P1.24
/// broker-fault semantics, over the REAL PostgreSQL outbox rows the
/// worker's OutboxPublisher polls.
#[test]
fn the_slice_publish_records_drain_with_zero_loss_and_refuse_redelivery() {
    let Some(url) =
        require_db("the_slice_publish_records_drain_with_zero_loss_and_refuse_redelivery")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xD1);
    let (ctx, _portfolio, _site, _asset) = seed_environment(&store, tenant_uuid);
    let key = "dispatch_issued:p431-drain-1";
    let receipt = StateChangeReceipt::new(
        "dispatch_commands".to_string(),
        StateChangeOperation::Insert,
        Some(ctx.tenant_id()),
    );

    // The publish record commits.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        EventOutbox::append(
            &unit,
            dispatch_outbox_write(receipt.clone(), key, "DISPATCH_ISSUED"),
        )
        .expect("the publish record appends");
        unit.commit().expect("committed");
    }

    // FAULT: the publish is ATTEMPTED and the broker never acks (the
    // drop). The attempt is recorded on the DURABLE row and the entry
    // stays PENDING — no loss, never silently dropped.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        EventOutbox::record_attempt(&unit, key).expect("the attempt is recorded");
        let pending = unit.pending_entries(100).expect("pending reads");
        let entry = pending
            .iter()
            .find(|entry| entry.idempotency_key() == key)
            .expect("the dropped entry stays pending on the durable row");
        assert_eq!(entry.attempt_count(), 1);
        unit.rollback().expect("rolled back");
    }

    // RECOVERY: the retry acks; the entry leaves the pending set.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        EventOutbox::mark_delivered(&unit, key).expect("the ack is recorded");
        assert!(
            !unit
                .pending_entries(100)
                .expect("pending reads")
                .iter()
                .any(|entry| entry.idempotency_key() == key),
            "the recovered entry is no longer pending (zero loss)"
        );
        // A REDELIVERED publish under the same key (the at-least-once
        // shape arriving late) is refused DURABLY — one effective row.
        assert_eq!(
            EventOutbox::append(
                &unit,
                dispatch_outbox_write(receipt, key, "DISPATCH_ISSUED"),
            ),
            Err(EventOutboxError::DuplicateKey {
                idempotency_key: key.to_string()
            })
        );
        unit.rollback().expect("rolled back");
    }
}
