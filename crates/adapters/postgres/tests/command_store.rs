// P4.10 — Integration tests for the PostgreSQL command-record store
//         (migration 0016; report §14.1–§14.2, §16.1, §16.2; ledger
//         P4.10).
//
// Covered on real PostgreSQL (gated on FLEXCORE_TEST_DATABASE_URL, the
// recorded harness decision — every test skips cleanly when unset):
//
// - the FULL forward lifecycle (ledger P4.10 acceptance: all §14.1
//   transitions exist and work): PROPOSED → VALIDATED → FEASIBLE →
//   APPROVED → ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED, with every
//   step journalled (from, to, actor, timestamp, reason, evidence) and the
//   gate named exactly on the three gate transitions (§14.1: validation,
//   feasibility, authorization are distinct);
// - a gate rejection journals WHICH gate rejected and the failing check
//   (ledger P4.11: "REJECTED with the failing check and evidence");
// - the SUPERSEDED discipline (D-P4.10-3): bidirectional links (the
//   backward link set by the transition, the forward link declared on the
//   replacement), the record retained for audit, and SUPERSEDED and
//   CANCELLED producing DISTINCT journal records (P0.11 §3.1);
// - illegal transitions rejected with a named error and no state change /
//   no journal row (ledger P4.10 step 3): an edge outside the frozen table
//   fails at the storage constructor (the domain table is the oracle), a
//   compare-and-swap from the wrong state fails at the database;
// - the one-active-transition invariant and immutability at the DATABASE
//   level: a no-op state UPDATE, a content UPDATE, a non-PROPOSED initial
//   INSERT, and a journal UPDATE/DELETE are all rejected;
// - the P4.12 pre-wiring gate check (`ensure_schedule_revision_usable`):
//   active = usable, SUPERSEDED = rejected, unknown = rejected;
// - tenant isolation (P0.16 rule 5, §16.2 RLS): a foreign-tenant command
//   reads as `None` and its journal leaks nothing.
//
// The exhaustive 14×14 legal/illegal matrix is P4.33's (the domain-level
// legal-edge and illegal-edge suites live in
// `crates/flexcore-domain/tests/command_lifecycle.rs`); this file proves
// the persistence shape, not the whole matrix.
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
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, EvidenceReference, Gate,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use flexcore_storage::command_store::{
    CommandRecordRow, CommandRecordStore, CommandStoreError, CommandTransitionRecord,
};
use flexcore_storage::schedule_revision::{
    ScheduleRevisionChainStore, ScheduleRevisionRow, ScheduleSupersedeTarget,
};
use flexcore_storage::schedule_store::{ScheduleRecordRow, ScheduleRecordStore};

// --- Harness helpers (the recorded P2.06/P3.21/P3.22 decisions) ---------------

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
    bytes[1] = 0xB4; // P4.10 fixture marker
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
            subject: "p4.10-command-store-test".to_string(),
        },
    ))
}

/// Seeds tenant + portfolio + site + asset (the dispatch_commands FK
/// chain) in one committed unit of work.
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
        "p4.10 tenant".to_string(),
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
        "p4.10 portfolio".to_string(),
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
        "p4.10 site".to_string(),
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
        "p4.10 BESS".to_string(),
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
/// command's composite FK requires (P3.22 identity scheme).
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
        "schedule_input_snapshot:envelope-v1:forecast-v1:synthetic:fnv1a64:00000000000000a1"
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
    supersedes: Option<FlexUuid>,
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
        supersedes.map(|id| id.to_string()),
        "{\"target_p_mw\":1.5,\"units\":\"MW\",\"sign\":\"export_positive\"}".to_string(),
    )
    .expect("the command row builds")
}

/// Builds one validated journal record through the storage constructor
/// (the frozen-edge and reason/evidence/gate rules are enforced there —
/// an edge outside the P0.11 table cannot even reach the database).
fn transition_record(
    ctx: &TenantContext,
    command: CommandId,
    from: CommandState,
    to: CommandState,
    at_unix_millis: i64,
    gate: Option<Gate>,
    reason: Option<String>,
    evidence: Option<&str>,
) -> CommandTransitionRecord {
    let transition = CommandTransition {
        command_id: command,
        from,
        to,
        actor: Actor::new("svc:lifecycle-test").expect("actor builds"),
        at_unix_millis,
        gate,
        reason,
        evidence: evidence.map(|reference| {
            EvidenceReference::new(reference).expect("evidence reference builds")
        }),
    };
    CommandTransitionRecord::from_transition(ctx.tenant_id(), &transition)
        .expect("the transition record builds (the edge is in the frozen table)")
}

// --- Tests ---------------------------------------------------------------------

#[test]
fn the_full_forward_lifecycle_journalls_every_transition() {
    let Some(url) = require_db("the_full_forward_lifecycle_journalls_every_transition") else {
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

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule, None))
            .expect("the command inserts as PROPOSED");

        // The full forward path, ONE unit of work: each step is a frozen
        // edge, and each journal row commits with the state change.
        let steps = [
            (CommandState::Proposed, CommandState::Validated, Some(Gate::Validation), Some("evidence:validation-v1")),
            (CommandState::Validated, CommandState::Feasible, Some(Gate::Feasibility), Some("evidence:feasibility-v1")),
            (CommandState::Feasible, CommandState::Approved, Some(Gate::Authorization), Some("evidence:authorization-v1")),
            (CommandState::Approved, CommandState::Issued, None, Some("evidence:signature-v1")),
            (CommandState::Issued, CommandState::Acknowledged, None, Some("evidence:acceptance-v1")),
            (CommandState::Acknowledged, CommandState::Tracking, None, Some("evidence:telemetry-v1")),
            (CommandState::Tracking, CommandState::Completed, None, Some("evidence:criterion-v1")),
        ];
        for (index, (from, to, gate, evidence)) in steps.iter().enumerate() {
            let record = transition_record(
                &ctx,
                command_id,
                *from,
                *to,
                1_700_000_100_000 + index as i64 * 1_000,
                *gate,
                None,
                *evidence,
            );
            unit.record_command_transition(&ctx, &record)
                .expect("the transition commits with its journal row");
        }
        unit.commit().expect("committed");
    }

    // The command COMPLETED and the journal is complete, ordered, and
    // names the gate exactly on the three gate transitions.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the completed command is found");
        assert_eq!(record.state, "COMPLETED");
        assert!(record.superseded_by_command_id.is_none());
        assert_eq!(record.row.command_id, command.to_string());
        assert_eq!(record.row.schedule_revision, 1);
        assert!(record.row.command_payload_json.contains("target_p_mw"));

        let journal = unit
            .transition_journal(&ctx, &command.to_string())
            .expect("the journal reads");
        assert_eq!(journal.len(), 7, "every transition is journalled");
        let expected_pairs = [
            ("PROPOSED", "VALIDATED"),
            ("VALIDATED", "FEASIBLE"),
            ("FEASIBLE", "APPROVED"),
            ("APPROVED", "ISSUED"),
            ("ISSUED", "ACKNOWLEDGED"),
            ("ACKNOWLEDGED", "TRACKING"),
            ("TRACKING", "COMPLETED"),
        ];
        for (index, (from, to)) in expected_pairs.iter().enumerate() {
            assert_eq!(journal[index].from_state, *from);
            assert_eq!(journal[index].to_state, *to);
            assert_eq!(journal[index].actor, "svc:lifecycle-test");
            assert!(
                journal[index].evidence_reference.is_some(),
                "step {index} carries its evidence reference"
            );
            assert!(journal[index].reason.is_none());
            let expected_gate = match index {
                0 => Some("validation"),
                1 => Some("feasibility"),
                2 => Some("authorization"),
                _ => None,
            };
            assert_eq!(
                journal[index].gate.as_deref(),
                expected_gate,
                "the gate is named exactly on the three gate transitions (§14.1)"
            );
        }
        // Journal ordering is by the database-assigned sequence, matching
        // the transition timestamps.
        let stamps: Vec<i64> = journal.iter().map(|row| row.at_unix_millis).collect();
        let mut sorted = stamps.clone();
        sorted.sort();
        assert_eq!(stamps, sorted, "the journal is in transition order");
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn a_gate_rejection_journals_which_gate_rejected_and_the_failing_check() {
    let Some(url) =
        require_db("a_gate_rejection_journals_which_gate_rejected_and_the_failing_check")
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
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule, None))
            .expect("the command inserts");
        // The validation gate FAILS: PROPOSED → REJECTED with the gate and
        // the failing check named (ledger P4.11).
        let rejected = transition_record(
            &ctx,
            CommandId::new(command),
            CommandState::Proposed,
            CommandState::Rejected,
            1_700_000_100_000,
            Some(Gate::Validation),
            Some("units are kW, not MW".to_string()),
            Some("evidence:validation-failure-v1"),
        );
        unit.record_command_transition(&ctx, &rejected)
            .expect("the rejection commits");
        unit.commit().expect("committed");
    }

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the rejected command is found");
        assert_eq!(record.state, "REJECTED");
        let journal = unit
            .transition_journal(&ctx, &command.to_string())
            .expect("the journal reads");
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0].to_state, "REJECTED");
        assert_eq!(journal[0].gate.as_deref(), Some("validation"));
        assert_eq!(journal[0].reason.as_deref(), Some("units are kW, not MW"));
        assert_eq!(
            journal[0].evidence_reference.as_deref(),
            Some("evidence:validation-failure-v1"),
            "the rejection carries the failing-check evidence (ledger P4.11)"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn supersede_links_both_directions_and_stays_distinct_from_cancellation() {
    let Some(url) =
        require_db("supersede_links_both_directions_and_stays_distinct_from_cancellation")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xD1);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0xD2);
    let candidate = fixture_uuid(0xD3);
    let command_a = fixture_uuid(0xD4);
    let command_b = fixture_uuid(0xD5);
    let command_c = fixture_uuid(0xD6);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        // B declares its forward link to A at creation (§14.3 field 16).
        unit.insert_command_record(
            &ctx,
            &command_row(&ctx, command_a, portfolio, site, asset, schedule, None),
        )
        .expect("command A inserts");
        unit.insert_command_record(
            &ctx,
            &command_row(&ctx, command_b, portfolio, site, asset, schedule, Some(command_a)),
        )
        .expect("command B inserts (supersedes A)");
        unit.insert_command_record(
            &ctx,
            &command_row(&ctx, command_c, portfolio, site, asset, schedule, None),
        )
        .expect("command C inserts");

        // A: PROPOSED → VALIDATED → SUPERSEDED (by B; the domain engine
        // records the replacing command as `command:<uuid>` evidence).
        let validated = transition_record(
            &ctx,
            CommandId::new(command_a),
            CommandState::Proposed,
            CommandState::Validated,
            1_700_000_100_000,
            Some(Gate::Validation),
            None,
            Some("evidence:validation-a1"),
        );
        unit.record_command_transition(&ctx, &validated).expect("A validates");
        let superseded = transition_record(
            &ctx,
            CommandId::new(command_a),
            CommandState::Validated,
            CommandState::Superseded,
            1_700_000_101_000,
            None,
            Some("invalidated by a newer approved schedule/command revision".to_string()),
            Some(format!("command:{command_b}").as_str()),
        );
        unit.record_command_transition(&ctx, &superseded)
            .expect("A is superseded by B");

        // C: the explicit act — PROPOSED → VALIDATED → FEASIBLE → APPROVED
        // → CANCELLED with the operator reason (§14.2).
        let c_id = CommandId::new(command_c);
        let c_validated = transition_record(
            &ctx, c_id, CommandState::Proposed, CommandState::Validated,
            1_700_000_100_000, Some(Gate::Validation), None, Some("evidence:validation-c1"),
        );
        unit.record_command_transition(&ctx, &c_validated).expect("C validates");
        let c_feasible = transition_record(
            &ctx, c_id, CommandState::Validated, CommandState::Feasible,
            1_700_000_101_000, Some(Gate::Feasibility), None, Some("evidence:feasibility-c1"),
        );
        unit.record_command_transition(&ctx, &c_feasible).expect("C is feasible");
        let c_approved = transition_record(
            &ctx, c_id, CommandState::Feasible, CommandState::Approved,
            1_700_000_102_000, Some(Gate::Authorization), None, Some("evidence:authorization-c1"),
        );
        unit.record_command_transition(&ctx, &c_approved).expect("C is approved");
        let c_cancelled = transition_record(
            &ctx, c_id, CommandState::Approved, CommandState::Cancelled,
            1_700_000_103_000, None, Some("operator requested cancellation".to_string()), None,
        );
        unit.record_command_transition(&ctx, &c_cancelled).expect("C is cancelled");
        unit.commit().expect("committed");
    }

    // The links are complete in BOTH directions, the superseded record is
    // retained for audit, and SUPERSEDED and CANCELLED are distinct
    // journal records (P0.11 §3.1).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let superseded = unit
            .find_command(&ctx, &command_a.to_string())
            .expect("read succeeds")
            .expect("the SUPERSEDED record is retained for audit");
        assert!(superseded.is_superseded());
        assert_eq!(
            superseded.superseded_by_command_id,
            Some(command_b.to_string()),
            "the backward link names the replacement"
        );
        assert!(
            superseded.row.command_payload_json.contains("target_p_mw"),
            "the content is intact (retained, not rewritten)"
        );

        let replacement = unit
            .find_command(&ctx, &command_b.to_string())
            .expect("read succeeds")
            .expect("the replacement is found");
        assert_eq!(
            replacement.row.supersedes_command_id,
            Some(command_a.to_string()),
            "the forward link names the replaced command"
        );
        assert_eq!(replacement.state, "PROPOSED");

        let cancelled = unit
            .find_command(&ctx, &command_c.to_string())
            .expect("read succeeds")
            .expect("the cancelled command is found");
        assert_eq!(cancelled.state, "CANCELLED");
        assert!(
            cancelled.superseded_by_command_id.is_none(),
            "a cancelled command was not superseded"
        );

        let journal_a = unit
            .transition_journal(&ctx, &command_a.to_string())
            .expect("the journal reads");
        assert_eq!(journal_a.last().expect("entries").to_state, "SUPERSEDED");
        let journal_c = unit
            .transition_journal(&ctx, &command_c.to_string())
            .expect("the journal reads");
        assert_eq!(journal_c.last().expect("entries").to_state, "CANCELLED");
        assert_ne!(
            journal_a.last().expect("entries").to_state,
            journal_c.last().expect("entries").to_state,
            "SUPERSEDED and CANCELLED produce distinct audit records (§14.2)"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn illegal_and_racing_transitions_are_refused_and_leave_no_change() {
    let Some(url) =
        require_db("illegal_and_racing_transitions_are_refused_and_leave_no_change")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xE1);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0xE2);
    let candidate = fixture_uuid(0xE3);
    let command = fixture_uuid(0xE4);
    let command_id = CommandId::new(command);

    // An edge OUTSIDE the frozen table never reaches the database: the
    // storage constructor validates through the domain's P0.11 table (the
    // oracle). Pure constructor evidence — no DB required for this half.
    {
        let illegal = CommandTransition {
            command_id,
            from: CommandState::Proposed,
            to: CommandState::Approved,
            actor: Actor::new("svc:lifecycle-test").expect("actor builds"),
            at_unix_millis: 1_700_000_100_000,
            gate: Some(Gate::Authorization),
            reason: None,
            evidence: Some(EvidenceReference::new("evidence:skip").expect("evidence")),
        };
        assert!(
            matches!(
                CommandTransitionRecord::from_transition(ctx.tenant_id(), &illegal),
                Err(CommandStoreError::IllegalTransition { .. })
            ),
            "the PROPOSED -> APPROVED shortcut is rejected at the constructor (frozen table)"
        );
    }

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule, None))
            .expect("the command inserts");

        // A LEGAL edge attempted from the WRONG current state: the
        // compare-and-swap refuses it (the one-active-transition
        // invariant's first half) with the named conflict error.
        let racing = transition_record(
            &ctx,
            command_id,
            CommandState::Issued,
            CommandState::Acknowledged,
            1_700_000_100_000,
            None,
            None,
            Some("evidence:acceptance-v1"),
        );
        assert_eq!(
            unit.record_command_transition(&ctx, &racing),
            Err(CommandStoreError::TransitionConflict {
                command_id: command.to_string(),
                expected_from: "ISSUED".to_string(),
                actual_state: "PROPOSED".to_string(),
            })
        );

        // A transition on an unknown command is NotFound — never treated
        // as a state change.
        let ghost = transition_record(
            &ctx,
            CommandId::new(fixture_uuid(0xE5)),
            CommandState::Proposed,
            CommandState::Validated,
            1_700_000_100_000,
            Some(Gate::Validation),
            None,
            Some("evidence:validation-v1"),
        );
        assert!(matches!(
            unit.record_command_transition(&ctx, &ghost),
            Err(CommandStoreError::NotFound { .. })
        ));

        // Nothing moved: the state is unchanged and no journal row exists.
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the command is found");
        assert_eq!(record.state, "PROPOSED");
        assert!(
            unit.transition_journal(&ctx, &command.to_string())
                .expect("the journal reads")
                .is_empty(),
            "a refused transition leaves no journal entry"
        );
        unit.rollback().expect("rolled back");
    }
}

#[test]
fn the_schedule_revision_gate_check_rejects_superseded_and_unknown() {
    let Some(url) =
        require_db("the_schedule_revision_gate_check_rejects_superseded_and_unknown")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0xF1);
    let (ctx, portfolio, site, _asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0xF2);
    let candidate1 = fixture_uuid(0xF3);
    let candidate2 = fixture_uuid(0xF4);

    // A two-revision chain: revision 1 ACTIVE, then superseded by
    // revision 2 (the P3.22 flow).
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate1);
        unit.commit().expect("committed");
    }
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let candidate_record = ScheduleRecordRow::new(
            ctx.tenant_id(),
            portfolio.to_string(),
            site.to_string(),
            candidate2.to_string(),
            "optimal".to_string(),
            "lp".to_string(),
            "highs".to_string(),
            "1.15.1".to_string(),
            "tier1-python-worker".to_string(),
            "0.1.0".to_string(),
            Some(-40.0),
            Some("currency-units".to_string()),
            "schedule_input_snapshot:envelope-v1:forecast-v1:synthetic:fnv1a64:00000000000000a2"
                .to_string(),
            "grid-model-v1".to_string(),
            site.to_string(),
            "site-grid-model-reference-v1".to_string(),
            format!("schedule_proposed:{portfolio}:{site}:{candidate2}"),
            "{\"status\":\"optimal\"}".to_string(),
        )
        .expect("the candidate record row builds");
        unit.insert_schedule_record(&ctx, &candidate_record)
            .expect("the second candidate inserts");
        unit.mark_revision_superseded(
            &ctx,
            &schedule.to_string(),
            1,
            &ScheduleSupersedeTarget::new(2, candidate2.to_string()).expect("target"),
        )
        .expect("revision 1 is marked SUPERSEDED");
        let revision2 = ScheduleRevisionRow::new(
            ctx.tenant_id(),
            portfolio.to_string(),
            site.to_string(),
            schedule.to_string(),
            2,
            1_700_008_640_000,
            1_700_017_280_000,
            "forecast_update".to_string(),
            Some(1),
            Some(candidate1.to_string()),
            candidate2.to_string(),
            "{\"revision\":2,\"intervals\":[{\"t\":0,\"setpoints\":[]}]}".to_string(),
        )
        .expect("the revision row builds");
        unit.insert_schedule_revision(&ctx, &revision2)
            .expect("revision 2 inserts");
        unit.commit().expect("committed");
    }

    // The P4.12 pre-wiring gate check (D-P4.10-5): active = usable,
    // SUPERSEDED = rejected, unknown = ALSO rejected (unknown is never
    // "usable"). A transition on a command whose revision is superseded
    // is rejected at this check BEFORE any state change.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        unit.ensure_schedule_revision_usable(&ctx, &schedule.to_string(), 2)
            .expect("the active revision is usable");
        assert_eq!(
            unit.ensure_schedule_revision_usable(&ctx, &schedule.to_string(), 1),
            Err(CommandStoreError::ScheduleRevisionSuperseded {
                schedule_id: schedule.to_string(),
                revision: 1,
            })
        );
        assert_eq!(
            unit.ensure_schedule_revision_usable(&ctx, &schedule.to_string(), 9),
            Err(CommandStoreError::ScheduleRevisionUnknown {
                schedule_id: schedule.to_string(),
                revision: 9,
            })
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn a_foreign_tenant_command_reads_as_none_and_leaks_nothing() {
    let Some(url) = require_db("a_foreign_tenant_command_reads_as_none_and_leaks_nothing")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x11);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0x12);
    let candidate = fixture_uuid(0x13);
    let command = fixture_uuid(0x14);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule, None))
            .expect("the command inserts");
        let validated = transition_record(
            &ctx,
            CommandId::new(command),
            CommandState::Proposed,
            CommandState::Validated,
            1_700_000_100_000,
            Some(Gate::Validation),
            None,
            Some("evidence:validation-v1"),
        );
        unit.record_command_transition(&ctx, &validated).expect("validates");
        unit.commit().expect("committed");
    }

    let foreign_ctx = tenant_context(fixture_uuid(0x15));
    {
        let unit = store.begin(Some(&foreign_ctx)).expect("unit of work opens");
        assert!(
            unit.find_command(&foreign_ctx, &command.to_string())
                .expect("read succeeds")
                .is_none(),
            "a foreign-tenant command reads as None (P0.16 rule 5)"
        );
        assert!(
            unit.transition_journal(&foreign_ctx, &command.to_string())
                .expect("read succeeds")
                .is_empty(),
            "existence never leaks across a tenant boundary"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}

#[test]
fn command_content_and_the_journal_are_immutable_at_the_database() {
    let Some(url) =
        require_db("command_content_and_the_journal_are_immutable_at_the_database")
    else {
        return;
    };
    let store = FlexCoreStore::connect(&url).expect("connects");
    store.run_migrations().expect("migrations apply");
    let tenant_uuid = fixture_uuid(0x21);
    let (ctx, portfolio, site, asset) = seed_environment(&store, tenant_uuid);
    let schedule = fixture_uuid(0x22);
    let candidate = fixture_uuid(0x23);
    let command = fixture_uuid(0x24);

    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        seed_schedule_revision(&unit, &ctx, portfolio, site, schedule, candidate);
        unit.insert_command_record(&ctx, &command_row(&ctx, command, portfolio, site, asset, schedule, None))
            .expect("the command inserts");
        // One journalled transition, so the journal-immutability statements
        // below target an EXISTING row (a grant denial must not be masked
        // by a zero-row update).
        let validated = transition_record(
            &ctx,
            CommandId::new(command),
            CommandState::Proposed,
            CommandState::Validated,
            1_700_000_100_000,
            Some(Gate::Validation),
            None,
            Some("evidence:validation-v1"),
        );
        unit.record_command_transition(&ctx, &validated).expect("validates");
        unit.commit().expect("committed");
    }

    // Raw statements against the migration's guards. The failure CLASSES
    // differ (permission denied vs the trigger's raise exception); the
    // assertion is on the outcome, never the engine text (the
    // no-raw-storage-text rule).
    let pool: &sqlx::PgPool = store.pool();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let with_tenant = |statement: &'static str| {
        let command_text = command.to_string();
        let tenant_text = ctx.tenant_id().uuid().to_string();
        runtime.block_on(async move {
            let mut tx = pool.begin().await.expect("transaction opens");
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(tenant_text)
                .execute(&mut *tx)
                .await
                .expect("tenant GUC sets");
            let result = sqlx::query(statement)
                .bind(command_text)
                .bind(ctx.tenant_id().uuid().to_string())
                .execute(&mut *tx)
                .await;
            let _ = tx.rollback().await;
            result
        })
    };

    // Content has NO update path (the runtime role holds a column-level
    // UPDATE grant on exactly the two transition columns; the trigger is
    // the backstop for owner sessions).
    assert!(
        with_tenant(
            "UPDATE dispatch_commands SET command_payload = '{\"tampered\":true}'::jsonb \
             WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
        )
        .is_err(),
        "a command content UPDATE must be rejected"
    );
    // A no-op state rewrite is rejected: the row only ever moves through
    // ONE state transition (the one-active-transition invariant).
    assert!(
        with_tenant(
            "UPDATE dispatch_commands SET state = 'VALIDATED' \
             WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = 'VALIDATED'"
        )
        .is_err(),
        "a same-state rewrite must be rejected by the immutability trigger"
    );
    // The freeze's initial marker: no other initial state can be inserted.
    let illegal_insert = {
        let command_text = command.to_string();
        let tenant_text = ctx.tenant_id().uuid().to_string();
        let portfolio_text = portfolio.to_string();
        let site_text = site.to_string();
        let asset_text = asset.to_string();
        let schedule_text = schedule.to_string();
        runtime.block_on(async move {
            let mut tx = pool.begin().await.expect("transaction opens");
            sqlx::query("SELECT set_config('flexcore.tenant_id', $1, true)")
                .bind(tenant_text)
                .execute(&mut *tx)
                .await
                .expect("tenant GUC sets");
            let result = sqlx::query(
                "INSERT INTO dispatch_commands \
                     (command_id, tenant_id, portfolio_id, site_id, asset_id, schedule_id, \
                      schedule_revision, valid_until_millis, state, command_payload) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::uuid, \
                         1, 1700000200000, 'ISSUED', '{}'::jsonb)",
            )
            .bind(command_text)
            .bind(tenant_text)
            .bind(portfolio_text)
            .bind(site_text)
            .bind(asset_text)
            .bind(schedule_text)
            .execute(&mut *tx)
            .await;
            let _ = tx.rollback().await;
            result
        })
    };
    assert!(
        illegal_insert.is_err(),
        "a non-PROPOSED initial INSERT must be rejected (every command enters as PROPOSED)"
    );
    // The journal is INSERT-only for the runtime role: no update, no delete.
    assert!(
        with_tenant(
            "UPDATE dispatch_command_transitions SET reason = 'rewritten' \
             WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
        )
        .is_err(),
        "a journal UPDATE must be rejected"
    );
    assert!(
        with_tenant(
            "DELETE FROM dispatch_command_transitions \
             WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
        )
        .is_err(),
        "a journal DELETE must be rejected"
    );

    // The row is unchanged after every refused statement.
    {
        let unit = store.begin(Some(&ctx)).expect("unit of work opens");
        let record = unit
            .find_command(&ctx, &command.to_string())
            .expect("read succeeds")
            .expect("the command is found");
        assert_eq!(record.state, "VALIDATED", "the state is unchanged");
        assert!(
            record.row.command_payload_json.contains("target_p_mw"),
            "the content is unchanged"
        );
        unit.rollback().expect("read-only unit rolled back");
    }
}
