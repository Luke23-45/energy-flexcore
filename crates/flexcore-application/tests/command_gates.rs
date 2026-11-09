// P4.11 / P4.12 / P4.13 — Command gate tests (report §14.1–§14.3, §12.5,
//         §17.3–§17.5; ledger P4.11–P4.13 acceptance criteria; decision
//         record: `ledger/phase-04-grid-feasibility-dispatch/
//         11_13_command_gates.md`).
//
// Coverage per the ledger acceptance criteria:
//
// P4.11 — commands carry schedule ID/revision and evidence refs; validation
//         failures name the failing check; a command from a SUPERSEDED
//         schedule revision is rejected AT CREATION (not merely at issue);
//         events emitted (`DispatchProposed`, `DispatchRejected`).
// P4.12 — feasibility evidence is a SEPARATE reference from validation
//         evidence (distinct journal rows, distinct fields, never merged);
//         a network failure rejects with the NAMED constraint; fail-closed
//         triggers apply; the revision check agrees at both layers (this
//         suite for the application gate, the postgres suite
//         `the_schedule_revision_gate_check_rejects_superseded_and_unknown`
//         for the persistence layer); feasibility evidence binds the model
//         hash and a model change blocks issue (the P4.16 hook).
// P4.13 — APPROVED requires a recorded authorized actor + scope + limits +
//         expiry; self-approval is blocked (separation of duties enforced);
//         an unauthorized role is rejected; APPROVED without FEASIBLE is
//         impossible (application-layer guard here; the domain guard test
//         is `flexcore-domain/tests/command_lifecycle.rs::
//         approved_without_feasible_evidence_is_impossible_by_construction`);
//         scope mismatch is rejected; expiry is recorded.
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    parse_feasibility_evidence_hash, AppliedSeparationOfDuties, ApprovalScope,
    AssetCapabilitySource, AssetGateStatus, AuthorizationDecision, AuthorizationDecisionRecord,
    CommandAuthzAction, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGateError, CommandGatePolicy,
    CommandGateStore, CommandGateStoreError, CommandGateView, CommandGates, CreationCheck,
    DeclaredQuantities, EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError,
    ModelVersionGateStatus, ModelVersionSource, ProposeCommandInput, ProposedCommandRecord,
    ScheduleEnvelopeSource, ScheduleRevisionUsability, SeparationOfDuties,
    FEASIBILITY_EVIDENCE_REF_PREFIX, COMMAND_ENTITY,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, Gate, LifecycleClock};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, EvidenceReferences, ExecutionRequirements, RampLimits,
    ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c04";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c05";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c06";
const UUID_OTHER_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c07";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant() -> TenantId {
    TenantId::new(uuid(UUID_TENANT))
}

fn context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant(),
        subject: "test:principal".to_string(),
    }))
}

fn command_id() -> CommandId {
    CommandId::new(uuid(UUID_COMMAND))
}

fn schedule_ref(revision: u64) -> ScheduleReference {
    ScheduleReference {
        schedule_id: ScheduleId::new(uuid(UUID_SCHEDULE)),
        revision,
    }
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:modelhash01";

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
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

// --- Seam fakes -------------------------------------------------------------------

struct FakeStore {
    records: Mutex<HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<HashMap<String, Vec<CommandTransition>>>,
    revisions: Mutex<HashMap<(String, u64), ScheduleRevisionUsability>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            journals: Mutex::new(HashMap::new()),
            revisions: Mutex::new(HashMap::new()),
        }
    }

    /// Seeds a revision state; a revision with no seeded state is Unknown.
    fn seed_revision(&self, schedule_id: &str, revision: u64, state: ScheduleRevisionUsability) {
        self.revisions
            .lock()
            .expect("fake store")
            .insert((schedule_id.to_string(), revision), state);
    }

    /// Flips a revision to superseded AFTER a command was created on it.
    fn supersede_revision(&self, schedule_id: &str, revision: u64) {
        self.seed_revision(schedule_id, revision, ScheduleRevisionUsability::Superseded);
    }

    fn journal_of(&self, command_id: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn has_record(&self, command_id: CommandId) -> bool {
        self.records
            .lock()
            .expect("fake store")
            .contains_key(&command_id.to_string())
    }
}

impl CommandGateStore for FakeStore {
    fn insert_proposed(
        &self,
        ctx: &TenantContext,
        record: &ProposedCommandRecord,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        let mut records = self.records.lock().expect("fake store");
        if records.contains_key(&record.command_id.to_string()) {
            return Err(CommandGateStoreError::Conflict {
                command_id: record.command_id.to_string(),
            });
        }
        records.insert(record.command_id.to_string(), record.clone());
        Ok(StateChangeReceipt::new(
            COMMAND_ENTITY.to_string(),
            StateChangeOperation::Insert,
            Some(ctx.tenant_id()),
        ))
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
        // Compare-and-swap: the transition applies only from the state the
        // caller saw (the P4.10 one-active-transition invariant).
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
        schedule_id: &ScheduleId,
        revision: u64,
    ) -> Result<ScheduleRevisionUsability, CommandGateStoreError> {
        Ok(self
            .revisions
            .lock()
            .expect("fake store")
            .get(&(schedule_id.to_string(), revision))
            .copied()
            .unwrap_or(ScheduleRevisionUsability::Unknown))
    }
}

struct FakeEnvelopes {
    verdict: Mutex<EnvelopeGateFreshness>,
}

impl FakeEnvelopes {
    fn set(&self, verdict: EnvelopeGateFreshness) {
        *self.verdict.lock().expect("fake envelopes") = verdict;
    }
}

impl ScheduleEnvelopeSource for FakeEnvelopes {
    fn freshness(
        &self,
        _ctx: &TenantContext,
        _schedule: &ScheduleReference,
    ) -> Result<EnvelopeGateFreshness, GateSeamError> {
        Ok(self.verdict.lock().expect("fake envelopes").clone())
    }
}

struct FakeModels {
    status: Mutex<ModelVersionGateStatus>,
}

impl FakeModels {
    fn set(&self, status: ModelVersionGateStatus) {
        *self.status.lock().expect("fake models") = status;
    }
}

impl ModelVersionSource for FakeModels {
    fn status(
        &self,
        _ctx: &TenantContext,
        _grid_model_ref: &str,
    ) -> Result<ModelVersionGateStatus, GateSeamError> {
        Ok(self.status.lock().expect("fake models").clone())
    }
}

struct FakeAssets {
    status: Mutex<AssetGateStatus>,
}

impl FakeAssets {
    fn set(&self, status: AssetGateStatus) {
        *self.status.lock().expect("fake assets") = status;
    }
}

impl AssetCapabilitySource for FakeAssets {
    fn check(
        &self,
        _ctx: &TenantContext,
        _asset: AssetId,
    ) -> Result<AssetGateStatus, GateSeamError> {
        Ok(self.status.lock().expect("fake assets").clone())
    }
}

struct FakeRunner {
    verdict: Mutex<FeasibilityGateVerdict>,
}

impl FakeRunner {
    fn set(&self, verdict: FeasibilityGateVerdict) {
        *self.verdict.lock().expect("fake runner") = verdict;
    }
}

impl CommandFeasibilityRunner for FakeRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        Ok(self.verdict.lock().expect("fake runner").clone())
    }
}

struct FakeAuthz {
    allowed_roles: Vec<String>,
}

impl CommandAuthorizationEvaluator for FakeAuthz {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision {
        if self.allowed_roles.contains(&request.role) {
            AuthorizationDecision::Allowed {
                rule_reference: format!(
                    "policy:phase4:{}:command.approve:{}",
                    request.role,
                    "asset"
                ),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
    }
}

/// Renders the event's Debug form (which names the frozen variant) — enough
/// to assert WHICH event was emitted without a serialization library.
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(&self, envelope: &flexcore_events::envelope::EventEnvelope) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- The gate harness --------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    envelopes: &'static FakeEnvelopes,
    models: &'static FakeModels,
    assets: &'static FakeAssets,
    runner: &'static FakeRunner,
    authz: &'static FakeAuthz,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
}

fn harness() -> Harness {
    let store = Box::leak(Box::new(FakeStore::new()));
    let envelopes = Box::leak(Box::new(FakeEnvelopes {
        verdict: Mutex::new(EnvelopeGateFreshness::Fresh),
    }));
    let models = Box::leak(Box::new(FakeModels {
        status: Mutex::new(ModelVersionGateStatus::Active {
            content_hash: MODEL_HASH.to_string(),
        }),
    }));
    let assets = Box::leak(Box::new(FakeAssets {
        status: Mutex::new(AssetGateStatus::Usable(DeclaredQuantities {
            active_power: true,
            reactive_power: true,
            soc: true,
            max_up_ramp_mw_per_s: 2.0,
            max_down_ramp_mw_per_s: 2.0,
        })),
    }));
    let runner = Box::leak(Box::new(FakeRunner {
        verdict: Mutex::new(FeasibilityGateVerdict::Passed {
            attempt_id: "attempt-1".to_string(),
            model_id: "feeder-a".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.97; max loading 71%".to_string(),
            coverage: "balanced radial feeder-a; unbalanced deferred (P4.34)".to_string(),
        }),
    }));
    let authz = Box::leak(Box::new(FakeAuthz {
        allowed_roles: vec![APPROVER_ROLE.to_string()],
    }));
    let journal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock = Box::leak(Box::new(TestClock::at(5_000)));
    let ids = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder = Box::leak(Box::new(DebugEncoder));
    let gates = CommandGates {
        store,
        envelopes,
        models,
        assets,
        feasibility: runner,
        authz,
        outbox,
        encoder,
        ids,
        clock,
        policy: CommandGatePolicy::default(),
    };
    Harness {
        gates,
        store,
        envelopes,
        models,
        assets,
        runner,
        authz,
        outbox,
        clock,
        ids,
    }
}

// --- Command fixture ----------------------------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

fn propose_input() -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: command_id(),
        scope: CommandScope {
            tenant_id: tenant(),
            portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
            site_id: SiteId::new(uuid(UUID_SITE)),
            asset_id: AssetId::new(uuid(UUID_ASSET)),
        },
        source: SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        },
        schedule: schedule_ref(2),
        intent: CommandIntent {
            reason: "peak-shaving window".to_string(),
            action: "set-bess-power".to_string(),
            actor_or_service: PROPOSER.to_string(),
        },
        target: ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(1.5).unwrap(),
            q: Some(ReactivePowerMVAr::new(-0.2).unwrap()),
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: Some(ReactiveSignConvention::InjectionPositive),
            },
        },
        validity: validity(),
        execution: ExecutionRequirements {
            duration: Duration::from_secs(30),
            ramp: RampLimits {
                max_up_mw_per_s: 0.5,
                max_down_mw_per_s: 0.5,
            },
        },
        sequence: 17,
        idempotency_key: "idem:op-42".to_string(),
        evidence: EvidenceReferences {
            grid_model: "grid-model:feeder-a@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:abc123".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-1".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

/// Creates a command and walks it to VALIDATED (the shared prefix of the
/// feasibility and authorization tests).
fn create_and_validate(h: &Harness) -> flexcore_domain::dispatch_command::DispatchCommand {
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(result.passed, "the shared-prefix validation must pass");
    proposed.command
}

/// Walks a command all the way to FEASIBLE.
fn create_validate_feasible(h: &Harness) -> flexcore_domain::dispatch_command::DispatchCommand {
    let command = create_and_validate(h);
    let result = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("feasibility verdict");
    assert!(result.result.passed, "the shared-prefix feasibility must pass");
    command
}

fn emitted_events(h: &Harness) -> Vec<&'static str> {
    h.outbox
        .writes()
        .iter()
        .map(|write| event_name_of(write.payload()))
        .collect()
}

fn event_name_of(debug_payload: &str) -> &'static str {
    for candidate in [
        "DispatchProposed",
        "DispatchRejected",
        "DispatchApproved",
    ] {
        if debug_payload.starts_with(candidate) {
            return candidate;
        }
    }
    "unknown"
}

// ============================================================================
// P4.11 — creation
// ============================================================================

#[test]
fn creation_from_the_active_revision_inserts_proposed_and_emits_dispatch_proposed() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    // The command carries the schedule ID/revision (P4.11 step 1).
    assert_eq!(proposed.command.schedule().revision, 2);
    assert_eq!(
        proposed.command.schedule().schedule_id.uuid(),
        uuid(UUID_SCHEDULE)
    );
    // The §14.3 evidence group travels.
    assert!(!proposed.command.evidence().grid_model.is_empty());
    // The store holds exactly one PROPOSED record; the journal is empty
    // (the freeze's initial marker is not a transition).
    assert!(h.store.has_record(command_id()));
    assert!(h.store.journal_of(command_id()).is_empty());
    // DispatchProposed was emitted.
    assert_eq!(emitted_events(&h), vec!["DispatchProposed"]);
}

#[test]
fn creation_from_a_superseded_revision_is_rejected_at_creation_not_merely_at_issue() {
    let h = harness();
    h.store.seed_revision(
        &uuid(UUID_SCHEDULE).to_string(),
        2,
        ScheduleRevisionUsability::Superseded,
    );
    let error = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect_err("superseded-revision creation must be rejected");
    match error {
        CommandGateError::CreationRejected { check, .. } => {
            assert_eq!(check, CreationCheck::SourceScheduleRevisionSuperseded);
        }
        other => panic!("unexpected error: {other}"),
    }
    // No record was inserted and no event was emitted (no command exists).
    assert!(!h.store.has_record(command_id()));
    assert!(h.outbox.is_empty());
}

#[test]
fn creation_from_an_unknown_revision_is_rejected_unknown_is_never_usable() {
    let h = harness();
    // No revision seeded → Unknown.
    let error = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect_err("unknown-revision creation must be rejected");
    match error {
        CommandGateError::CreationRejected { check, .. } => {
            assert_eq!(check, CreationCheck::SourceScheduleRevisionUnknown);
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(!h.store.has_record(command_id()));
}

#[test]
fn a_schema_invalid_command_is_rejected_by_the_dispatch_command_validators() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let mut input = propose_input();
    input.schedule.revision = 0; // revisions start at 1 (§14.3)
    let error = h
        .gates
        .create_proposed_command(&context(), input)
        .expect_err("zero revision must be rejected");
    assert!(matches!(error, CommandGateError::Schema(_)), "{error}");
    assert!(!h.store.has_record(command_id()));
}

#[test]
fn a_duplicate_creation_conflicts_and_creates_no_second_record() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    h.gates
        .create_proposed_command(&context(), propose_input())
        .expect("first creation succeeds");
    let error = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect_err("a re-proposal is a NEW command");
    match error {
        CommandGateError::Store(CommandGateStoreError::Conflict { .. }) => {}
        other => panic!("unexpected error: {other}"),
    }
}

// ============================================================================
// P4.11 — the validation gate
// ============================================================================

#[test]
fn the_validation_gate_moves_proposed_to_validated_and_journals_the_evidence() {
    let h = harness();
    let command = create_and_validate(&h);
    let journal = h.store.journal_of(command.command_id());
    assert_eq!(journal.len(), 1);
    let entry = &journal[0];
    assert_eq!(entry.from, CommandState::Proposed);
    assert_eq!(entry.to, CommandState::Validated);
    assert_eq!(entry.gate, Some(Gate::Validation));
    assert_eq!(entry.actor.identity(), "svc:validator");
    assert_eq!(entry.at_unix_millis, 5_000); // the controlled clock
    let evidence = entry.evidence.as_ref().expect("validation evidence");
    assert!(
        evidence.as_str().starts_with("validation:command/"),
        "validation evidence reference: {}",
        evidence.as_str()
    );
    // There is no DispatchValidated event in the frozen §15.3 vocabulary;
    // only the creation event exists so far.
    assert_eq!(emitted_events(&h), vec!["DispatchProposed"]);
}

#[test]
fn a_stale_envelope_rejects_at_the_validation_gate_with_the_check_named() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    h.envelopes.set(EnvelopeGateFreshness::Stale {
        reason: "max age exceeded".to_string(),
    });
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict (a rejection is not an error)");
    assert!(!result.passed);
    assert_eq!(
        result.failing_check.as_deref(),
        Some("envelope_stale:max age exceeded")
    );
    // The journal row names the gate and carries the failing check.
    let journal = h.store.journal_of(command_id());
    let entry = journal.last().expect("rejection journalled");
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.gate, Some(Gate::Validation));
    assert_eq!(entry.reason.as_deref(), Some("envelope_stale:max age exceeded"));
    // DispatchRejected was emitted after DispatchProposed.
    assert_eq!(
        emitted_events(&h),
        vec!["DispatchProposed", "DispatchRejected"]
    );
}

#[test]
fn a_seam_failure_is_fail_closed_never_a_pass() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    // A seam ERROR (not a stale verdict) must also reject — fail-closed.
    struct Broken;
    impl ScheduleEnvelopeSource for Broken {
        fn freshness(
            &self,
            _ctx: &TenantContext,
            _schedule: &ScheduleReference,
        ) -> Result<EnvelopeGateFreshness, GateSeamError> {
            Err(GateSeamError {
                seam: "schedule_envelope",
                detail: "backend unavailable".to_string(),
            })
        }
    }
    let broken: &'static dyn ScheduleEnvelopeSource = Box::leak(Box::new(Broken));
    let gates = CommandGates {
        envelopes: broken,
        ..rebuild(&h)
    };
    let result = gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert!(!result.passed);
    assert_eq!(
        result.failing_check.as_deref(),
        Some("envelope_check_failed:backend unavailable")
    );
}

/// Rebuilds the gate struct with one seam replaced (test-only convenience;
/// all other collaborators come from the harness).
fn rebuild(h: &Harness) -> CommandGates<'static> {
    CommandGates {
        store: h.store,
        envelopes: h.envelopes,
        models: h.models,
        assets: h.assets,
        feasibility: h.runner,
        authz: h.authz,
        outbox: h.outbox,
        encoder: Box::leak(Box::new(DebugEncoder)),
        ids: h.ids,
        clock: h.clock,
        policy: CommandGatePolicy::default(),
    }
}

#[test]
fn an_inactive_or_unknown_model_rejects_at_the_validation_gate() {
    for (status, expected) in [
        (
            ModelVersionGateStatus::NotActive {
                state: "retired".to_string(),
            },
            "model_version_not_active:retired",
        ),
        (ModelVersionGateStatus::Unknown, "model_version_unknown"),
    ] {
        let h = harness();
        h.store.seed_revision(
            &uuid(UUID_SCHEDULE).to_string(),
            2,
            ScheduleRevisionUsability::Active,
        );
        let proposed = h
            .gates
            .create_proposed_command(&context(), propose_input())
            .expect("creation succeeds");
        h.models.set(status);
        let result = h
            .gates
            .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
            .expect("the gate verdict");
        assert!(!result.passed);
        assert_eq!(result.failing_check.as_deref(), Some(expected));
    }
}

#[test]
fn an_unknown_asset_rejects_without_leaking_existence() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    h.assets.set(AssetGateStatus::UnknownInTenantScope);
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert!(!result.passed);
    // A foreign asset and a missing asset are indistinguishable (P0.16
    // rule 5): the check code names the tenant scope, never which.
    assert_eq!(
        result.failing_check.as_deref(),
        Some("asset_unknown_in_tenant_scope")
    );
}

#[test]
fn a_missing_capability_model_and_undeclared_quantities_are_rejected() {
    // Missing capability model.
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    h.assets.set(AssetGateStatus::CapabilityModelMissing);
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert_eq!(
        result.failing_check.as_deref(),
        Some("asset_capability_model_missing")
    );

    // The target controls Q but the capability does not declare Q —
    // undeclared is never unconstrained.
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    h.assets.set(AssetGateStatus::Usable(DeclaredQuantities {
        active_power: true,
        reactive_power: false,
        soc: true,
        max_up_ramp_mw_per_s: 2.0,
        max_down_ramp_mw_per_s: 2.0,
    }));
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert_eq!(
        result.failing_check.as_deref(),
        Some("quantity_not_declared:reactive_power")
    );
}

#[test]
fn ramp_and_duration_policy_checks_reject_with_named_checks() {
    // Ramp above the declared capability.
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    h.assets.set(AssetGateStatus::Usable(DeclaredQuantities {
        active_power: true,
        reactive_power: true,
        soc: true,
        max_up_ramp_mw_per_s: 0.1,
        max_down_ramp_mw_per_s: 2.0,
    }));
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert_eq!(
        result.failing_check.as_deref(),
        Some("ramp_up_exceeds_declared_capability")
    );

    // Duration longer than the validity window can never complete.
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let mut input = propose_input();
    input.execution.duration = Duration::from_secs(120); // window span is 60 s
    let proposed = h
        .gates
        .create_proposed_command(&context(), input)
        .expect("creation succeeds");
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert_eq!(
        result.failing_check.as_deref(),
        Some("duration_exceeds_validity_window")
    );

    // The deployment duration cap (a configured policy check).
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let gates = CommandGates {
        policy: CommandGatePolicy {
            separation_of_duties: SeparationOfDuties::Enforced,
            max_execution_duration: Some(Duration::from_secs(10)),
        },
        ..rebuild(&h)
    };
    let proposed = gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    let result = gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert_eq!(
        result.failing_check.as_deref(),
        Some("command_policy_duration_exceeded")
    );
}

#[test]
fn the_validation_gate_applies_only_from_proposed() {
    let h = harness();
    let command = create_and_validate(&h);
    let error = h
        .gates
        .run_validation_gate(&context(), &command, &actor("svc:validator"))
        .expect_err("validation applies only from PROPOSED");
    match error {
        CommandGateError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Validated);
            assert_eq!(operation, "run_validation_gate");
        }
        other => panic!("unexpected error: {other}"),
    }
}

// ============================================================================
// P4.12 — the feasibility gate
// ============================================================================

#[test]
fn the_feasibility_gate_moves_validated_to_feasible_with_separate_evidence() {
    let h = harness();
    let command = create_validate_feasible(&h);
    let journal = h.store.journal_of(command.command_id());
    assert_eq!(journal.len(), 2);
    // Validation evidence and feasibility evidence are DISTINCT references
    // on DISTINCT journal rows — never merged (the anti-conflation rule).
    let validation = journal
        .iter()
        .find(|e| e.to == CommandState::Validated)
        .expect("validation row");
    let feasibility = journal
        .iter()
        .find(|e| e.to == CommandState::Feasible)
        .expect("feasibility row");
    assert_eq!(validation.gate, Some(Gate::Validation));
    assert_eq!(feasibility.gate, Some(Gate::Feasibility));
    let validation_ref = validation.evidence.as_ref().expect("validation evidence");
    let feasibility_ref = feasibility.evidence.as_ref().expect("feasibility evidence");
    assert_ne!(validation_ref, feasibility_ref);
    assert!(validation_ref.as_str().starts_with("validation:command/"));
    assert!(feasibility_ref.as_str().starts_with(FEASIBILITY_EVIDENCE_REF_PREFIX));
    // The feasibility evidence references THE MODEL HASH, parseable.
    assert_eq!(
        parse_feasibility_evidence_hash(
            feasibility_ref.as_str()
        ),
        Some(MODEL_HASH)
    );
    assert_eq!(feasibility.actor.identity(), "svc:grid-gate");
    assert_eq!(feasibility.at_unix_millis, 5_000);
}

#[test]
fn a_transformer_limit_violation_rejects_with_the_constraint_named() {
    let h = harness();
    let command = create_and_validate(&h);
    // The validation PASSED; the power flow violates the transformer limit.
    h.runner.set(FeasibilityGateVerdict::LimitViolated {
        attempt_id: "attempt-2".to_string(),
        model_id: "feeder-a".to_string(),
        model_content_hash: MODEL_HASH.to_string(),
        constraint: "transformer_thermal_loading:T1".to_string(),
    });
    let result = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("the gate verdict");
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("network_limit:transformer_thermal_loading:T1")
    );
    assert!(result.feasibility_evidence.is_none());
    // REJECTED with the failing constraint in the journal + the event.
    let journal = h.store.journal_of(command.command_id());
    let entry = journal.last().expect("rejection journalled");
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.gate, Some(Gate::Feasibility));
    assert_eq!(
        entry.reason.as_deref(),
        Some("network_limit:transformer_thermal_loading:T1")
    );
    assert_eq!(
        emitted_events(&h),
        vec!["DispatchProposed", "DispatchRejected"]
    );
}

#[test]
fn a_fail_closed_verdict_rejects_never_passes() {
    let h = harness();
    let command = create_and_validate(&h);
    h.runner.set(FeasibilityGateVerdict::FailClosed {
        attempt_id: "attempt-3".to_string(),
        model_id: Some("feeder-a".to_string()),
        model_content_hash: Some(MODEL_HASH.to_string()),
        reason: "the power flow did not converge".to_string(),
    });
    let result = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("the gate verdict");
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("fail_closed:the power flow did not converge")
    );
    assert_eq!(
        h.store.journal_of(command.command_id())
            .last()
            .unwrap()
            .to,
        CommandState::Rejected
    );
}

#[test]
fn a_revision_superseded_after_validation_is_rejected_at_the_feasibility_gate() {
    let h = harness();
    let command = create_and_validate(&h);
    // The revision was ACTIVE at creation and at validation; it turned
    // SUPERSEDED before feasibility. Both layers agree: the application
    // gate rejects through the feasibility gate (this test), and the
    // persistence-layer check (P4.10 ensure_schedule_revision_usable, with
    // which this seam agrees by contract) is tested in the postgres suite.
    h.store.supersede_revision(&uuid(UUID_SCHEDULE).to_string(), 2);
    let result = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("the gate verdict");
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("source_schedule_revision_superseded")
    );
    assert_eq!(
        h.store.journal_of(command.command_id())
            .last()
            .unwrap()
            .to,
        CommandState::Rejected
    );
}

#[test]
fn an_unknown_revision_is_rejected_at_the_feasibility_gate_too() {
    let h = harness();
    let command = create_and_validate(&h);
    // Drop the seeded revision: the store now reports Unknown — never
    // "usable".
    h.store
        .revisions
        .lock()
        .expect("fake store")
        .remove(&(uuid(UUID_SCHEDULE).to_string(), 2));
    let result = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("the gate verdict");
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("source_schedule_revision_unknown")
    );
}

#[test]
fn the_feasibility_gate_applies_only_from_validated() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    // Still PROPOSED: the feasibility gate must refuse.
    let error = h
        .gates
        .run_feasibility_gate(&context(), &proposed.command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect_err("feasibility applies only from VALIDATED");
    match error {
        CommandGateError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Proposed);
            assert_eq!(operation, "run_feasibility_gate");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn feasibility_evidence_binds_the_model_hash_and_a_model_change_blocks_issue() {
    let h = harness();
    let command = create_validate_feasible(&h);
    // Walk to APPROVED so the issue-time hook is reachable (the hook is
    // what P4.16's adapter calls before ISSUED).
    let approval = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW, Q ±0.5 MVAr",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(approval.approved);

    // The evidence still binds to the current model hash: issueable.
    h.gates
        .ensure_command_issueable(&context(), command.command_id(), MODEL_HASH)
        .expect("the evidence matches the active model");

    // The model changed (P4.03 re-registration): the evidence is stale and
    // the command cannot be issued.
    let error = h
        .gates
        .ensure_command_issueable(&context(), command.command_id(), "fnv1a64:newmodelhash")
        .expect_err("a model change invalidates the feasibility evidence");
    match error {
        CommandGateError::ModelEvidenceStale {
            evidence_hash,
            active_hash,
        } => {
            assert_eq!(evidence_hash, MODEL_HASH);
            assert_eq!(active_hash, "fnv1a64:newmodelhash");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn the_issue_hook_requires_approved_and_fails_closed_on_missing_evidence() {
    let h = harness();
    let command = create_and_validate(&h); // VALIDATED, not APPROVED
    let error = h
        .gates
        .ensure_command_issueable(&context(), command.command_id(), MODEL_HASH)
        .expect_err("issue applies only from APPROVED");
    assert!(matches!(
        error,
        CommandGateError::WrongState { operation: "issue", .. }
    ));
}

// ============================================================================
// P4.13 — the authorization gate
// ============================================================================

#[test]
fn approval_by_a_distinct_authorized_actor_records_the_full_decision() {
    let h = harness();
    let command = create_validate_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW, Q ±0.5 MVAr",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(result.approved);
    let record: AuthorizationDecisionRecord = result.decision_record.expect("the decision record");
    // Actor, role, scope, limits, expiry, decision time (§17.3 / §14.3).
    assert_eq!(record.authorizing_actor, APPROVER);
    assert_eq!(record.authorizing_role, APPROVER_ROLE);
    assert_eq!(record.approved_scope, format!("asset:{}", uuid(UUID_ASSET)));
    assert_eq!(record.approved_limits, "±2.0 MW, Q ±0.5 MVAr");
    assert_eq!(record.expiry_unix_millis, 200_000);
    assert_eq!(record.decided_at_unix_millis, 5_000);
    assert_eq!(
        record.separation_of_duties,
        AppliedSeparationOfDuties::Enforced
    );
    // The decision covers the EXACT prior-gate evidence IDs (from the
    // journal — the evidence of record).
    let journal = h.store.journal_of(command.command_id());
    let validation_ref = journal
        .iter()
        .find(|e| e.to == CommandState::Validated)
        .unwrap()
        .evidence
        .as_ref()
        .unwrap()
        .as_str();
    let feasibility_ref = journal
        .iter()
        .find(|e| e.to == CommandState::Feasible)
        .unwrap()
        .evidence
        .as_ref()
        .unwrap()
        .as_str();
    assert_eq!(record.covered_validation_evidence, validation_ref);
    assert_eq!(record.covered_feasibility_evidence, feasibility_ref);
    // The APPROVED journal row names the authorization gate and carries
    // the decision reference as its evidence.
    let approved = journal.last().unwrap();
    assert_eq!(approved.to, CommandState::Approved);
    assert_eq!(approved.gate, Some(Gate::Authorization));
    assert_eq!(
        approved.evidence.as_ref().unwrap().as_str(),
        record.decision_reference
    );
    assert_eq!(approved.actor.identity(), APPROVER);
    // DispatchApproved was emitted.
    assert_eq!(
        emitted_events(&h),
        vec!["DispatchProposed", "DispatchApproved"]
    );
}

#[test]
fn self_approval_is_rejected_where_separation_of_duties_is_enforced() {
    let h = harness();
    let command = create_validate_feasible(&h);
    // The approver IS the proposing identity (§17.4; the P4.23 fixture).
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: PROPOSER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    assert_eq!(
        result.denial_reason.as_deref(),
        Some("self-approval prohibited (separation of duties, §17.4)")
    );
    // FEASIBLE → REJECTED with the denying actor recorded.
    let entry = h.store.journal_of(command.command_id()).last().unwrap();
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.gate, Some(Gate::Authorization));
    assert_eq!(entry.actor.identity(), PROPOSER);
    assert!(entry.reason.as_deref().unwrap().starts_with("self-approval prohibited"));
    assert_eq!(
        emitted_events(&h),
        vec!["DispatchProposed", "DispatchRejected"]
    );
}

#[test]
fn an_unauthorized_role_is_rejected_with_the_reason() {
    let h = harness();
    let command = create_validate_feasible(&h);
    // A role without the command.approve grant (deny-by-default; the
    // P4.23 fixture: approval attempted by an actor without the role).
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: "viewer".to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    assert_eq!(
        result.denial_reason.as_deref(),
        Some("authorization_denied:no matching rule for role viewer")
    );
    assert_eq!(
        h.store.journal_of(command.command_id())
            .last()
            .unwrap()
            .to,
        CommandState::Rejected
    );
}

#[test]
fn approval_is_impossible_without_feasible_the_guard_at_both_layers() {
    let h = harness();
    // Application layer: a VALIDATED command cannot be approved.
    let command = create_and_validate(&h);
    let error = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect_err("APPROVED without FEASIBLE is impossible");
    match error {
        CommandGateError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Validated);
            assert_eq!(operation, "record_command_approval");
        }
        other => panic!("unexpected error: {other}"),
    }
    // And a PROPOSED command cannot skip two gates either.
    let h2 = harness();
    h2.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h2
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("creation succeeds");
    let error = h2
        .gates
        .record_command_approval(
            &context(),
            &proposed.command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect_err("APPROVED without FEASIBLE is impossible");
    assert!(matches!(
        error,
        CommandGateError::WrongState { current: CommandState::Proposed, .. }
    ));
    // Domain layer (recorded, not duplicated): the frozen-table guard test
    // `approved_without_feasible_evidence_is_impossible_by_construction`
    // lives in flexcore-domain/tests/command_lifecycle.rs.
}

#[test]
fn an_approval_scope_that_does_not_cover_the_command_asset_is_rejected() {
    let h = harness();
    let command = create_validate_feasible(&h);
    // An asset-scoped approval naming a DIFFERENT asset.
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_OTHER_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    let expected_scope_denial = format!(
        "approval_scope_does_not_cover_command_asset:asset:{}",
        uuid(UUID_OTHER_ASSET)
    );
    assert_eq!(
        result.denial_reason.as_deref(),
        Some(expected_scope_denial.as_str())
    );
    // A site-scoped approval for another site does not cover it either.
    let h2 = harness();
    let command2 = create_validate_feasible(&h2);
    let result = h2
        .gates
        .record_command_approval(
            &context(),
            &command2,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Site(SiteId::new(uuid(UUID_OTHER_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    // Tenant scope covers every asset of the tenant.
    let h3 = harness();
    let command3 = create_validate_feasible(&h3);
    let result = h3
        .gates
        .record_command_approval(
            &context(),
            &command3,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Tenant,
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(result.approved);
    assert_eq!(result.decision_record.unwrap().approved_scope, "tenant");
}

#[test]
fn an_already_expired_authorization_is_rejected_and_expiry_is_recorded_on_approval() {
    let h = harness();
    let command = create_validate_feasible(&h);
    // The authorization expires at the controlled clock's current instant:
    // already expired (end-exclusive), rejected.
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(5_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    assert_eq!(
        result.denial_reason.as_deref(),
        Some("authorization_expired_before_decision")
    );

    // On the happy path the expiry is recorded verbatim (the P4.15 sweep
    // consumes it later).
    let h2 = harness();
    let command2 = create_validate_feasible(&h2);
    let result = h2
        .gates
        .record_command_approval(
            &context(),
            &command2,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(123_456).unwrap(),
        )
        .expect("the approval verdict");
    assert_eq!(result.decision_record.unwrap().expiry_unix_millis, 123_456);
}

#[test]
fn separation_of_duties_not_configured_is_recorded_not_silent() {
    let h = harness();
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let gates = CommandGates {
        policy: CommandGatePolicy {
            separation_of_duties: SeparationOfDuties::NotConfigured,
            max_execution_duration: None,
        },
        ..rebuild(&h)
    };
    let command = {
        let proposed = gates
            .create_proposed_command(&context(), propose_input())
            .expect("creation succeeds");
        gates
            .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
            .expect("validation");
        gates
            .run_feasibility_gate(&context(), &proposed.command, &feasibility_refs(), &actor("svc:grid-gate"))
            .expect("feasibility");
        proposed.command
    };
    // With SoD not configured, a same-identity approval is permitted —
    // but the decision record SAYS the policy was not configured.
    let result = gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: PROPOSER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(result.approved);
    assert_eq!(
        result.decision_record.unwrap().separation_of_duties,
        AppliedSeparationOfDuties::NotConfigured
    );
}

#[test]
fn every_gate_transition_journals_actor_and_controlled_clock_timestamp() {
    let h = harness();
    let command = create_validate_feasible(&h);
    h.gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    for entry in h.store.journal_of(command.command_id()) {
        assert!(!entry.actor.identity().is_empty(), "actor on every transition");
        assert!(entry.at_unix_millis > 0, "controlled-clock timestamp");
        assert!(entry.at_unix_millis <= 5_000, "never past the fixed clock");
    }
}
