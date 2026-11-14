// P4.15 — Expiry, cancellation, supersede tests (report §14.1–§14.2;
//         ledger P4.15 acceptance criteria; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         14_16_lifecycle_completion.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Expiry fires at the right milestone in tests: the pure sweep decides
//   exactly the expirable states with lapsed windows (controlled clock);
//   each of the four expirable states expires with ITS milestone; expiry
//   never fires early (the domain guard); the expiry time and the
//   milestone not reached are recorded; `DispatchExpired` is emitted.
// - EXPIRED is NOT terminal and has ONE outgoing edge (→ FALLBACK) —
//   pinned against the frozen oracle (`try_transition`).
// - Cancellation requires an actor + reason and applies only from
//   APPROVED and ISSUED.
// - Supersede links both directions and retains the record; SUPERSEDED and
//   CANCELLED are distinguishable in audit output (different states,
//   different journal reasons, different frozen events).
// - Events are emitted (the Debug encoder names the frozen variant — no
//   serialization library in this crate).
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.

use std::collections::HashMap;
use std::sync::Mutex;

use flexcore_application::command_expiry::{
    sweep_validity_windows, CancellationOutcome, CommandExpiryError, CommandExpiryService,
    EventScope, ExpiryCandidate, ExpiryOutcome, SupersedeOutcome,
};
use flexcore_application::command_gates::{
    CommandGateStore, CommandGateStoreError, CommandGateView, ProposedCommandRecord,
    COMMAND_ENTITY,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, EvidenceReference, ExpiryMilestone, Gate, LifecycleClock,
    LifecycleError,
};
use flexcore_domain::command_state::{try_transition, CommandState};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7d04";

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
}

fn command_id(tag: u8) -> CommandId {
    CommandId::new(uuid(tag))
}

fn context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(uuid(1)),
        subject: "test:principal".to_string(),
    }))
}

fn scope() -> EventScope {
    EventScope {
        portfolio_id: PortfolioId::new(uuid(2)),
        site_id: SiteId::new(uuid(3)),
        asset_id: AssetId::new(uuid(4)),
    }
}

fn actor() -> Actor {
    Actor::new("test:operator").expect("a valid actor")
}

// --- Controlled clock (never wall-clock) ------------------------------------------

struct TestClock {
    now_unix_millis: Mutex<i64>,
}

impl TestClock {
    fn at(now_unix_millis: i64) -> Self {
        Self {
            now_unix_millis: Mutex::new(now_unix_millis),
        }
    }

    fn advance_to(&self, now_unix_millis: i64) {
        *self.now_unix_millis.lock().expect("test clock") = now_unix_millis;
    }
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

/// Renders the event's Debug form (which names the frozen variant) —
/// enough to assert WHICH event was emitted without a serialization
/// library.
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(
        &self,
        envelope: &flexcore_events::envelope::EventEnvelope,
    ) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- Store fake -------------------------------------------------------------------

struct FakeStore {
    records: Mutex<HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<HashMap<String, Vec<CommandTransition>>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            journals: Mutex::new(HashMap::new()),
        }
    }

    /// Seeds a command record (valid-until bound) and its journal up to
    /// `state` (a forward walk with the gate evidence the replay requires).
    fn seed(&self, command: CommandId, valid_until_millis: i64, state: CommandState) {
        self.records
            .lock()
            .expect("fake store")
            .insert(
                command.to_string(),
                ProposedCommandRecord {
                    tenant_id: TenantId::new(uuid(1)),
                    command_id: command,
                    portfolio_id: uuid(2).to_string(),
                    site_id: uuid(3).to_string(),
                    asset_id: uuid(4).to_string(),
                    schedule_id: ScheduleId::new(uuid(6)).to_string(),
                    schedule_revision: 1,
                    valid_until_millis,
                    supersedes_command_id: None,
                    command_payload_json: "{\"command\":\"fixture\"}".to_string(),
                },
            );
        let journal = forward_journal_to(command, state);
        if !journal.is_empty() {
            self.journals
                .lock()
                .expect("fake store")
                .insert(command.to_string(), journal);
        }
    }

    fn journal_of(&self, command: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command.to_string())
            .cloned()
            .unwrap_or_default()
    }

    /// Seeds a record with an EXPLICIT journal — the states the forward
    /// walk cannot reach (terminal states, FAILED, EXPIRED, CANCELLED,
    /// SUPERSEDED, FALLBACK). Every journal here replays cleanly through
    /// `replay_full_lifecycle` (evidence/reason rules honoured).
    fn seed_journal(
        &self,
        command: CommandId,
        valid_until_millis: i64,
        journal: Vec<CommandTransition>,
    ) {
        self.records
            .lock()
            .expect("fake store")
            .insert(
                command.to_string(),
                ProposedCommandRecord {
                    tenant_id: TenantId::new(uuid(1)),
                    command_id: command,
                    portfolio_id: uuid(2).to_string(),
                    site_id: uuid(3).to_string(),
                    asset_id: uuid(4).to_string(),
                    schedule_id: ScheduleId::new(uuid(6)).to_string(),
                    schedule_revision: 1,
                    valid_until_millis,
                    supersedes_command_id: None,
                    command_payload_json: "{\"command\":\"fixture\"}".to_string(),
                },
            );
        if !journal.is_empty() {
            self.journals
                .lock()
                .expect("fake store")
                .insert(command.to_string(), journal);
        }
    }
}

/// One journalled entry at the given controlled instant.
fn entry_at(
    command: CommandId,
    from: CommandState,
    to: CommandState,
    gate: Option<Gate>,
    at_unix_millis: i64,
    reason: Option<String>,
    evidence: Option<&str>,
) -> CommandTransition {
    CommandTransition {
        command_id: command,
        from,
        to,
        actor: Actor::new("test:gate").unwrap(),
        at_unix_millis,
        gate,
        reason,
        evidence: evidence.map(|reference| EvidenceReference::new(reference).unwrap()),
    }
}

/// PROPOSED → REJECTED at the validation gate (a gate rejection carries the
/// gate, the failing check, and the evidence).
fn journal_to_rejected(command: CommandId) -> Vec<CommandTransition> {
    vec![entry_at(
        command,
        CommandState::Proposed,
        CommandState::Rejected,
        Some(Gate::Validation),
        1_000,
        Some("fixture:validation-check-failed".to_string()),
        Some("validation:command/x@1:check/fixture"),
    )]
}

/// The forward walk to TRACKING plus TRACKING → COMPLETED.
fn journal_to_completed(command: CommandId) -> Vec<CommandTransition> {
    let mut journal = forward_journal_to(command, CommandState::Tracking);
    journal.push(entry_at(
        command,
        CommandState::Tracking,
        CommandState::Completed,
        None,
        7_000,
        None,
        Some("tracking:command/x@7:criterion-satisfied"),
    ));
    journal
}

/// The forward walk to ISSUED plus ISSUED → FAILED with the frozen NACK
/// trigger (the reason text is the domain engine's, so the replay maps it).
fn journal_to_failed(command: CommandId) -> Vec<CommandTransition> {
    let mut journal = forward_journal_to(command, CommandState::Issued);
    journal.push(entry_at(
        command,
        CommandState::Issued,
        CommandState::Failed,
        None,
        5_000,
        Some("NACK from gateway/adapter".to_string()),
        None,
    ));
    journal
}

/// The FAILED journal plus FAILED → FALLBACK (frozen edge 26).
fn journal_to_fallback(command: CommandId) -> Vec<CommandTransition> {
    let mut journal = journal_to_failed(command);
    journal.push(entry_at(
        command,
        CommandState::Failed,
        CommandState::Fallback,
        None,
        6_000,
        Some("asset/site fallback policy activated".to_string()),
        Some("fallback:policy/fixture-hold"),
    ));
    journal
}

/// The forward walk to APPROVED plus APPROVED → CANCELLED with a reason.
fn journal_to_cancelled(command: CommandId) -> Vec<CommandTransition> {
    let mut journal = forward_journal_to(command, CommandState::Approved);
    journal.push(entry_at(
        command,
        CommandState::Approved,
        CommandState::Cancelled,
        None,
        4_000,
        Some("operator cancelled the window".to_string()),
        None,
    ));
    journal
}

/// The forward walk to APPROVED plus APPROVED → SUPERSEDED with the
/// `command:<uuid>` backward link (D-P4.10-3 — the replay parses it).
fn journal_to_superseded(command: CommandId, replacement: CommandId) -> Vec<CommandTransition> {
    let mut journal = forward_journal_to(command, CommandState::Approved);
    journal.push(entry_at(
        command,
        CommandState::Approved,
        CommandState::Superseded,
        None,
        4_000,
        Some("invalidated by a newer approved schedule/command revision".to_string()),
        Some(&format!("command:{replacement}")),
    ));
    journal
}

/// The forward walk to APPROVED plus APPROVED → EXPIRED. The validity bound
/// is seeded BELOW the expiry instant so the domain guard replays cleanly.
fn journal_to_expired(command: CommandId) -> Vec<CommandTransition> {
    let mut journal = forward_journal_to(command, CommandState::Approved);
    journal.push(entry_at(
        command,
        CommandState::Approved,
        CommandState::Expired,
        None,
        4_000,
        Some(
            "validity ended before issue (milestone not reached: ISSUED)".to_string(),
        ),
        None,
    ));
    journal
}

impl CommandGateStore for FakeStore {
    fn insert_proposed(
        &self,
        _ctx: &TenantContext,
        _record: &ProposedCommandRecord,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        // The expiry suite seeds records directly (`seed`/`seed_journal`);
        // a creation attempt through this fake is a suite error, reported
        // as a backend failure — never a stubbed success.
        Err(CommandGateStoreError::Backend {
            code: "fixture".to_string(),
            message: "the expiry suite seeds records directly; insert_proposed is not used"
                .to_string(),
        })
    }

    fn journal_transition(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransition,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        let mut journals = self.journals.lock().expect("fake store");
        let entry = journals
            .entry(transition.command_id.to_string())
            .or_default();
        let current = entry.last().map(|t| t.to).unwrap_or(CommandState::Proposed);
        if current != transition.from {
            return Err(CommandGateStoreError::TransitionConflict {
                command_id: transition.command_id.to_string(),
                expected_from: transition.from.name().to_string(),
            });
        }
        entry.push(transition.clone());
        Ok(StateChangeReceipt::new(
            "dispatch_command_transitions".to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        ))
    }

    fn load_command(
        &self,
        _ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<Option<CommandGateView>, CommandGateStoreError> {
        let records = self.records.lock().expect("fake store");
        let Some(record) = records.get(&command_id.to_string()) else {
            return Ok(None);
        };
        let journal = self
            .journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default();
        Ok(Some(CommandGateView {
            valid_until_millis: record.valid_until_millis,
            journal,
        }))
    }

    fn schedule_revision_state(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
    ) -> Result<flexcore_application::command_gates::ScheduleRevisionUsability, CommandGateStoreError>
    {
        Ok(flexcore_application::command_gates::ScheduleRevisionUsability::Active)
    }
}

/// Builds the forward journal up to `state` — the gate evidence the replay
/// requires (validation / feasibility / authorization evidence, then the
/// issue/ack/tracking evidence), at strictly increasing controlled instants.
fn forward_journal_to(command: CommandId, state: CommandState) -> Vec<CommandTransition> {
    let mut journal = Vec::new();
    let mut at = 1_000i64;
    let mut push = |from: CommandState, to: CommandState, gate: Option<Gate>, evidence: &str| {
        journal.push(CommandTransition {
            command_id: command,
            from,
            to,
            actor: Actor::new("test:gate").unwrap(),
            at_unix_millis: at,
            gate,
            reason: None,
            evidence: Some(EvidenceReference::new(evidence).unwrap()),
        });
        at += 1_000;
    };
    let _ = &mut push;
    if state == CommandState::Proposed {
        return journal;
    }
    push(
        CommandState::Proposed,
        CommandState::Validated,
        Some(Gate::Validation),
        "validation:command/x@1",
    );
    if state == CommandState::Validated {
        return journal;
    }
    push(
        CommandState::Validated,
        CommandState::Feasible,
        Some(Gate::Feasibility),
        "feasibility:attempt/a:model/m:hash/h",
    );
    if state == CommandState::Feasible {
        return journal;
    }
    push(
        CommandState::Feasible,
        CommandState::Approved,
        Some(Gate::Authorization),
        "authz-decision:command/x@3",
    );
    if state == CommandState::Approved {
        return journal;
    }
    push(
        CommandState::Approved,
        CommandState::Issued,
        None,
        "sim-sig:key-1:x",
    );
    if state == CommandState::Issued {
        return journal;
    }
    push(
        CommandState::Issued,
        CommandState::Acknowledged,
        None,
        "acceptance:sim/x",
    );
    if state == CommandState::Acknowledged {
        return journal;
    }
    push(
        CommandState::Acknowledged,
        CommandState::Tracking,
        None,
        "telemetry:command/x@7",
    );
    journal
}

// --- Harness ----------------------------------------------------------------------

struct Harness {
    service: CommandExpiryService<'static>,
    store: &'static FakeStore,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox =
        Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let service = CommandExpiryService {
        store,
        outbox,
        encoder,
        ids,
        clock,
    };
    Harness {
        service,
        store,
        outbox,
        clock,
    }
}

fn event_names(harness: &Harness) -> Vec<String> {
    let writes = harness.outbox.writes();
    writes
        .iter()
        .map(|write| {
            let payload = write.payload();
            // The DebugEncoder renders the DomainEvent's Debug form, which
            // starts with the variant name (e.g. `DispatchExpired(`).
            let name = payload
                .split('(')
                .next()
                .unwrap_or_default()
                .to_string();
            name
        })
        .collect()
}

// --- The sweep (pure) ---------------------------------------------------------------

#[test]
fn the_sweep_decides_exactly_the_expirable_states_with_lapsed_windows() {
    let now = 50_000i64;
    let valid_until = 40_000i64; // lapsed at `now`
    let candidates: Vec<ExpiryCandidate> = CommandState::ALL
        .iter()
        .map(|&state| ExpiryCandidate {
            command_id: command_id(10),
            state,
            valid_until_millis: valid_until,
        })
        .collect();
    let decisions = sweep_validity_windows(&candidates, now);
    let states: Vec<CommandState> = decisions.iter().map(|d| d.from_state).collect();
    assert_eq!(
        states,
        vec![
            CommandState::Approved,
            CommandState::Issued,
            CommandState::Acknowledged,
            CommandState::Tracking,
        ],
        "exactly the four states with frozen EXPIRED edges are swept"
    );
    for decision in &decisions {
        assert_eq!(decision.valid_until_millis, valid_until);
        assert_eq!(decision.now_unix_millis, now);
        assert!(decision.milestone.reason_text().contains("validity ended"));
    }
    // A window that has NOT ended yields no decision, in any state.
    let fresh: Vec<ExpiryCandidate> = CommandState::ALL
        .iter()
        .map(|&state| ExpiryCandidate {
            command_id: command_id(10),
            state,
            valid_until_millis: now + 1,
        })
        .collect();
    assert!(sweep_validity_windows(&fresh, now).is_empty());
}

#[test]
fn the_sweep_boundary_is_end_exclusive_matching_the_domain_guard() {
    // now == valid_until: the window is over (§10.3), matching
    // `CommandLifecycle::validity_has_lapsed`.
    assert_eq!(sweep_validity_windows(
        &[ExpiryCandidate {
            command_id: command_id(10),
            state: CommandState::Approved,
            valid_until_millis: 1_000,
        }],
        1_000,
    )
    .len(), 1);
    assert!(sweep_validity_windows(
        &[ExpiryCandidate {
            command_id: command_id(10),
            state: CommandState::Approved,
            valid_until_millis: 1_000,
        }],
        999,
    )
    .is_empty());
}

// --- Expiry application (store + domain guard + event) ------------------------------

#[test]
fn expiry_moves_approved_to_expired_records_milestone_and_time_and_emits() {
    let harness = harness(10_000);
    let command = command_id(20);
    harness.store.seed(command, 9_000, CommandState::Approved);

    let outcome = harness
        .service
        .apply_expiry(&context(), scope(), command, &actor())
        .expect("the lapsed APPROVED command expires");
    let ExpiryOutcome {
        transition,
        milestone,
        expired_at_unix_millis,
    } = &outcome;
    assert_eq!(transition.from, CommandState::Approved);
    assert_eq!(transition.to, CommandState::Expired);
    assert_eq!(*milestone, ExpiryMilestone::BeforeIssue);
    assert_eq!(*expired_at_unix_millis, 10_000, "the expiry time is recorded");
    assert!(
        transition.reason.as_deref().unwrap_or_default().contains("ISSUE"),
        "the milestone not reached is the journal reason: {:?}",
        transition.reason
    );

    // The event is the frozen DispatchExpired.
    let names = event_names(&harness);
    assert_eq!(names, vec!["DispatchExpired".to_string()]);
}

#[test]
fn expiry_never_fires_before_the_window_ends() {
    let harness = harness(8_000);
    let command = command_id(20);
    harness.store.seed(command, 9_000, CommandState::Approved);
    let error = harness
        .service
        .apply_expiry(&context(), scope(), command, &actor())
        .expect_err("expiry never fires early");
    match error {
        CommandExpiryError::Lifecycle(LifecycleError::ValidityNotLapsed { .. }) => {}
        other => panic!("unexpected error {other:?}"),
    }
    // The command is untouched.
    assert_eq!(
        harness.store.journal_of(command).last().map(|t| t.to),
        Some(CommandState::Approved)
    );
}

#[test]
fn expiry_uses_the_milestone_of_each_expirable_state() {
    struct Case {
        state: CommandState,
        milestone: ExpiryMilestone,
        tag: u8,
    }
    let cases = [
        Case { state: CommandState::Approved, milestone: ExpiryMilestone::BeforeIssue, tag: 21 },
        Case {
            state: CommandState::Issued,
            milestone: ExpiryMilestone::BeforeAcknowledgement,
            tag: 22,
        },
        Case {
            state: CommandState::Acknowledged,
            milestone: ExpiryMilestone::BeforeTrackingCompletion,
            tag: 23,
        },
        Case {
            state: CommandState::Tracking,
            milestone: ExpiryMilestone::BeforeCompletion,
            tag: 24,
        },
    ];
    for case in cases {
        let harness = harness(50_000);
        let command = command_id(case.tag);
        harness.store.seed(command, 9_000, case.state);
        let outcome = harness
            .service
            .apply_expiry(&context(), scope(), command, &actor())
            .unwrap_or_else(|error| panic!("{}: {error}", case.state.name()));
        assert_eq!(outcome.milestone, case.milestone);
        assert_eq!(outcome.transition.from, case.state);
        assert_eq!(outcome.transition.to, CommandState::Expired);
    }
}

#[test]
fn expiry_refuses_the_non_expirable_states_with_a_named_error() {
    // Every non-expirable state, each seeded with a REAL journal that
    // replays cleanly (the terminal states via the explicit builders).
    for (tag, state, journal) in [
        (30u8, CommandState::Proposed, Vec::new()),
        (31, CommandState::Validated, forward_journal_to(command_id(31), CommandState::Validated)),
        (32, CommandState::Feasible, forward_journal_to(command_id(32), CommandState::Feasible)),
        (33, CommandState::Completed, journal_to_completed(command_id(33))),
        (34, CommandState::Rejected, journal_to_rejected(command_id(34))),
        (35, CommandState::Cancelled, journal_to_cancelled(command_id(35))),
        (36, CommandState::Superseded, journal_to_superseded(command_id(36), command_id(37))),
        (38, CommandState::Fallback, journal_to_fallback(command_id(38))),
        // EXPIRED needs a bound below its expiry entry for the replay.
        (39, CommandState::Expired, journal_to_expired(command_id(39))),
        (40, CommandState::Failed, journal_to_failed(command_id(40))),
    ] {
        let harness = harness(50_000);
        let command = command_id(tag);
        let valid_until = if state == CommandState::Expired { 3_500 } else { 9_000 };
        harness.store.seed_journal(command, valid_until, journal);
        // Sanity: the seeded view really is in `state`.
        let view = harness
            .store
            .load_command(&context(), command)
            .expect("load")
            .expect("seeded");
        assert_eq!(view.state(), state, "fixture seeding for {}", state.name());
        let error = harness
            .service
            .apply_expiry(&context(), scope(), command, &actor())
            .expect_err("non-expirable states are refused");
        match error {
            CommandExpiryError::WrongState { current, operation } => {
                assert_eq!(current, state);
                assert_eq!(operation, "apply_expiry");
            }
            other => panic!("{}: unexpected error {other:?}", state.name()),
        }
    }
}

#[test]
fn expired_has_exactly_one_outgoing_edge_to_fallback_and_is_not_terminal() {
    // Frozen-edge conformance (the P0.11 oracle): EXPIRED is NOT terminal
    // and its ONLY outgoing edge is → FALLBACK (frozen transition 27).
    assert!(!flexcore_domain::command_state::is_terminal(CommandState::Expired));
    for &to in CommandState::ALL.iter() {
        let result = try_transition(CommandState::Expired, to);
        if to == CommandState::Fallback {
            assert_eq!(result, Ok(CommandState::Fallback));
        } else {
            assert!(
                result.is_err(),
                "EXPIRED -> {} must not exist in the frozen table",
                to.name()
            );
        }
    }
}

#[test]
fn the_sweep_application_applies_exactly_the_pure_decisions() {
    let harness = harness(50_000);
    let lapsed = command_id(40);
    let fresh = command_id(41);
    harness.store.seed(lapsed, 9_000, CommandState::Issued);
    harness.store.seed(fresh, 90_000, CommandState::Issued);
    let candidates = vec![
        ExpiryCandidate {
            command_id: lapsed,
            state: CommandState::Issued,
            valid_until_millis: 9_000,
        },
        ExpiryCandidate {
            command_id: fresh,
            state: CommandState::Issued,
            valid_until_millis: 90_000,
        },
    ];
    let applied = harness
        .service
        .run_sweep(&context(), scope(), &candidates, &actor())
        .expect("the sweep applies");
    assert_eq!(applied.len(), 1, "only the lapsed candidate fires");
    assert_eq!(applied[0].transition.command_id, lapsed);
    // The fresh command is untouched.
    assert_eq!(
        harness.store.journal_of(fresh).last().map(|t| t.to),
        Some(CommandState::Issued)
    );
}

#[test]
fn expiry_of_an_unknown_command_is_a_named_not_found() {
    let harness = harness(50_000);
    let error = harness
        .service
        .apply_expiry(&context(), scope(), command_id(50), &actor())
        .expect_err("unknown command");
    assert!(matches!(error, CommandExpiryError::NotFound { .. }));
}

// --- Cancellation --------------------------------------------------------------------

#[test]
fn cancellation_applies_from_approved_and_issued_with_actor_and_reason() {
    for (tag, state) in [(51u8, CommandState::Approved), (52, CommandState::Issued)] {
        let harness = harness(10_000);
        let command = command_id(tag);
        harness.store.seed(command, 90_000, state);
        let outcome: CancellationOutcome = harness
            .service
            .cancel(&context(), scope(), command, &actor(), "operator cancelled the window")
            .unwrap_or_else(|error| panic!("{}: {error}", state.name()));
        assert_eq!(outcome.transition.from, state);
        assert_eq!(outcome.transition.to, CommandState::Cancelled);
        assert_eq!(
            outcome.transition.reason.as_deref(),
            Some("operator cancelled the window"),
            "the reason is recorded"
        );
        assert_eq!(outcome.transition.actor.identity(), "test:operator");
        let names = event_names(&harness);
        assert_eq!(names, vec!["DispatchCancelled".to_string()]);
    }
}

#[test]
fn cancellation_requires_a_reason_and_refuses_empty_ones() {
    let harness = harness(10_000);
    let command = command_id(53);
    harness.store.seed(command, 90_000, CommandState::Approved);
    let error = harness
        .service
        .cancel(&context(), scope(), command, &actor(), "   ")
        .expect_err("an empty reason is refused");
    match error {
        CommandExpiryError::Lifecycle(LifecycleError::EmptyField { field }) => {
            assert_eq!(field, "reason");
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The command is untouched.
    assert_eq!(
        harness.store.journal_of(command).last().map(|t| t.to),
        Some(CommandState::Approved)
    );
}

#[test]
fn cancellation_requires_an_actor() {
    let harness = harness(10_000);
    let command = command_id(54);
    harness.store.seed(command, 90_000, CommandState::Approved);
    // An empty actor identity cannot even be constructed (`Actor::new`
    // rejects empty/whitespace) and `CommandExpiryService::cancel` takes the
    // actor as a REQUIRED parameter — cancellation without a recorded actor
    // is unrepresentable at the type level. The runtime validation:
    assert!(Actor::new("").is_err());
    assert!(Actor::new("   ").is_err());
    // With a valid actor, the mandatory-reason rule is the one that fires
    // on a blank reason (covered above); with both present the cancellation
    // records the actor (covered in `cancellation_applies_from_approved_
    // and_issued_with_actor_and_reason`).
    let outcome = harness
        .service
        .cancel(&context(), scope(), command, &actor(), "setup for the actor record")
        .expect("a valid actor + reason cancels");
    assert_eq!(outcome.transition.actor.identity(), "test:operator");
}

#[test]
fn cancellation_refuses_every_other_state() {
    // Every state except APPROVED and ISSUED, each seeded with a REAL
    // journal that replays cleanly (terminal states via the explicit
    // builders). Cancellation applies from APPROVED and ISSUED only.
    for (tag, state, journal) in [
        (55u8, CommandState::Proposed, Vec::new()),
        (56, CommandState::Validated, forward_journal_to(command_id(56), CommandState::Validated)),
        (57, CommandState::Feasible, forward_journal_to(command_id(57), CommandState::Feasible)),
        (58, CommandState::Acknowledged, forward_journal_to(command_id(58), CommandState::Acknowledged)),
        (59, CommandState::Tracking, forward_journal_to(command_id(59), CommandState::Tracking)),
        (60, CommandState::Completed, journal_to_completed(command_id(60))),
        (61, CommandState::Rejected, journal_to_rejected(command_id(61))),
        (62, CommandState::Failed, journal_to_failed(command_id(62))),
        (63, CommandState::Expired, journal_to_expired(command_id(63))),
        (64, CommandState::Cancelled, journal_to_cancelled(command_id(64))),
        (65, CommandState::Superseded, journal_to_superseded(command_id(65), command_id(66))),
        (67, CommandState::Fallback, journal_to_fallback(command_id(67))),
    ] {
        let harness = harness(10_000);
        let command = command_id(tag);
        let valid_until = if state == CommandState::Expired { 3_500 } else { 90_000 };
        harness.store.seed_journal(command, valid_until, journal);
        let view = harness
            .store
            .load_command(&context(), command)
            .expect("load")
            .expect("seeded");
        assert_eq!(view.state(), state, "fixture seeding for {}", state.name());
        let error = harness
            .service
            .cancel(&context(), scope(), command, &actor(), "reason")
            .expect_err("only APPROVED and ISSUED can cancel");
        match error {
            CommandExpiryError::WrongState { current, operation } => {
                assert_eq!(current, state);
                assert_eq!(operation, "cancel");
            }
            other => panic!("{}: unexpected error {other:?}", state.name()),
        }
    }
}

// --- Supersede -----------------------------------------------------------------------

#[test]
fn supersede_links_bidirectionally_and_retains_the_record() {
    let harness = harness(10_000);
    let command = command_id(60);
    let replacement = command_id(61);
    harness.store.seed(command, 90_000, CommandState::Tracking);

    let outcome: SupersedeOutcome = harness
        .service
        .supersede(&context(), scope(), command, replacement, &actor())
        .expect("a TRACKING command can be superseded");
    assert_eq!(outcome.superseded, command);
    assert_eq!(outcome.superseded_by, replacement);
    assert_eq!(outcome.transition.to, CommandState::Superseded);

    // The backward link is the journal evidence `command:<uuid>`
    // (D-P4.10-3); the forward link is the replacement's §14.3 field 17 —
    // the persistence layer stores BOTH directions.
    let journal = harness.store.journal_of(command);
    let last = journal.last().expect("the record is RETAINED for audit");
    assert_eq!(last.to, CommandState::Superseded);
    assert_eq!(
        last.evidence.as_ref().map(|e| e.as_str()),
        Some(format!("command:{replacement}").as_str())
    );
    assert_eq!(
        last.reason.as_deref(),
        Some("invalidated by a newer approved schedule/command revision")
    );
    // Retained: the full history is still there.
    assert_eq!(journal.len(), 7);

    // The replayed aggregate carries the backward link.
    let view = harness
        .store
        .load_command(&context(), command)
        .expect("load")
        .expect("seeded");
    let lifecycle =
        flexcore_application::command_record::replay_full_lifecycle(command, &view).unwrap();
    assert_eq!(lifecycle.superseded_by(), Some(replacement));
    assert_eq!(lifecycle.state(), CommandState::Superseded);
}

#[test]
fn supersede_refuses_self_supersede_and_non_supersedeable_states() {
    let harness = harness(10_000);
    let command = command_id(62);
    harness.store.seed(command, 90_000, CommandState::Approved);
    let error = harness
        .service
        .supersede(&context(), scope(), command, command, &actor())
        .expect_err("self-supersede is refused");
    match error {
        CommandExpiryError::Lifecycle(LifecycleError::SelfSupersede { .. }) => {}
        other => panic!("unexpected error {other:?}"),
    }

    for (tag, state, journal) in [
        (63u8, CommandState::Proposed, Vec::new()),
        (68, CommandState::Completed, journal_to_completed(command_id(68))),
        (69, CommandState::Rejected, journal_to_rejected(command_id(69))),
    ] {
        let harness = harness(10_000);
        let command = command_id(tag);
        harness.store.seed_journal(command, 90_000, journal);
        let error = harness
            .service
            .supersede(&context(), scope(), command, command_id(64), &actor())
            .expect_err("non-supersedeable states are refused");
        match error {
            CommandExpiryError::WrongState { current, operation } => {
                assert_eq!(current, state);
                assert_eq!(operation, "supersede");
            }
            other => panic!("{}: unexpected error {other:?}", state.name()),
        }
    }
}

// --- SUPERSEDED vs CANCELLED: distinguishable audit records --------------------------

#[test]
fn superseded_and_cancelled_are_distinguishable_in_audit_output() {
    // Two identical APPROVED commands: one cancelled, one superseded.
    let harness = harness(10_000);
    let cancelled = command_id(70);
    let superseded = command_id(71);
    harness.store.seed(cancelled, 90_000, CommandState::Approved);
    harness.store.seed(superseded, 90_000, CommandState::Approved);

    harness
        .service
        .cancel(&context(), scope(), cancelled, &actor(), "operator cancelled the window")
        .expect("cancel");
    harness
        .service
        .supersede(&context(), scope(), superseded, command_id(72), &actor())
        .expect("supersede");

    let cancelled_entry = harness.store.journal_of(cancelled).last().unwrap().clone();
    let superseded_entry = harness.store.journal_of(superseded).last().unwrap().clone();

    // DIFFERENT states (§14.2; P0.11 §3.1: never merged).
    assert_eq!(cancelled_entry.to, CommandState::Cancelled);
    assert_eq!(superseded_entry.to, CommandState::Superseded);
    assert_ne!(cancelled_entry.to, superseded_entry.to);
    // DIFFERENT journal reasons (the cause is recorded).
    assert_ne!(cancelled_entry.reason, superseded_entry.reason);
    assert_eq!(cancelled_entry.reason.as_deref(), Some("operator cancelled the window"));
    assert!(superseded_entry
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("invalidated by"));
    // DIFFERENT frozen events.
    let names = event_names(&harness);
    assert!(names.contains(&"DispatchCancelled".to_string()));
    assert!(names.contains(&"DispatchSuperseded".to_string()));
    // Both records are retained (append-only journal).
    assert_eq!(harness.store.journal_of(cancelled).len(), 4);
    assert_eq!(harness.store.journal_of(superseded).len(), 4);
}

// --- The recorded decisions are stated ----------------------------------------------

#[test]
fn the_recorded_decision_constants_are_stated() {
    assert!(flexcore_application::command_expiry::EXPIRY_SWEEP_DECISION.contains("SWEEP"));
    assert!(flexcore_application::command_expiry::EXPIRY_EVENT_PAYLOAD_GAP_NOTE.contains(
        "journal"
    ));
}
