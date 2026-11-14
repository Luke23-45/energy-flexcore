// P4.25 — Deterministic rejection: expired commands
//         (report §23 Stage 4 exit gate, §14.1–§14.3, §15.3
//         `DispatchExpired`, §14.4 fallback; ledger P4.25; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         23_25_rejection_suites_b.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Fixtures expiring at EACH stage: APPROVED before issue; ISSUED before
//   acknowledgement; ACKNOWLEDGED before tracking completes; TRACKING
//   before completion — the four states with frozen EXPIRED edges
//   (P0.11 edges 10, 15, 20, 24).
// - Each fixture's expected state is PRE-REGISTERED: EXPIRED with the
//   milestone not reached recorded (the journal reason IS the milestone
//   text) and the expiry time recorded.
// - Controlled clock throughout: expiry never fires early (the domain
//   `ValidityNotLapsed` guard), the boundary is end-exclusive (§10.3), and
//   the check-at-transition path is exercised beside the P4.15 sweep (the
//   adapter's tracking poll expires the command at the poll).
// - NO expired command is issued, acknowledged-as-fresh, or completed:
//   asserted per fixture on the journal, on the adapter's post-expiry
//   refusals (issue/poll → WrongState), and on the frozen-edge oracle
//   (`try_transition` — no EXPIRED → ISSUED/ACKNOWLEDGED/COMPLETED edge;
//   EXPIRED's ONLY outgoing edge is → FALLBACK, where policy requires it).
// - `DispatchExpired` is emitted exactly once per case (§15.3).
// - The fallback consequence per P4.17–P4.18 fires WHERE THE POLICY
//   REQUIRES IT: a named OnExpiry policy activates (EXPIRED → FALLBACK
//   through the frozen edge 27, `DispatchFallbackActivated` emitted, the
//   activation audit-recorded); a scope with no named policy gets the
//   explicit default entry (operator intervention, NO transition).
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_expiry::{
    CommandExpiryError, CommandExpiryService, EventScope, ExpiryOutcome,
};
use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGatePolicy,
    CommandGateStore, CommandGateStoreError, CommandGates, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposedCommandRecord, ProposeCommandInput, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, SeparationOfDuties,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, DispatchAdapterError, SimulationKeyring, SimulatedAcceptance,
    SimulatedObservation, SimulatorDispatchAdapter, SimulatorDispatchTransport,
    SimulatorTransportError, TrackingAcceptanceCriterion, TrackingTerminal, TrackedQuantity,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::fallback_policy::{
    FallbackAction, FallbackActivationInput, FallbackAuditError, FallbackAuditSink,
    FallbackDecision, FallbackEngine, FallbackPolicy, FallbackPolicyRegistry, FallbackScope,
    FallbackTrigger, HoldValidityConditions, SimulatedOperatorReview,
};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, ExpiryMilestone, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::{try_transition, CommandState};
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6faa01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6faa02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6faa03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6faa04";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6faa06";

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
}

fn context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(uuid(1)),
        subject: "test:principal".to_string(),
    }))
}

fn asset_id() -> AssetId {
    AssetId::new(uuid(4))
}

fn scope() -> EventScope {
    EventScope {
        portfolio_id: PortfolioId::new(uuid(2)),
        site_id: SiteId::new(uuid(3)),
        asset_id: asset_id(),
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:expiry-feeder-hash-01";
const SIMULATOR_ID: &str = "sim:p425-rejection-fixture";
const SIM_KEY_ID: &str = "sim-key-p425";

// --- The pre-registered expectations (ledger P4.25 step 2) --------------------------
//
// Each fixture's expected state, milestone, and post-expiry refusals are
// registered here BEFORE the assertions; the tests assert the pre-registered
// values verbatim.

/// The pre-registered milestone per stage (the §14.2 "validity window ended
/// before …" milestone the P4.15 sweep and the domain guard record).
fn pre_registered_milestone(fixture: &str) -> ExpiryMilestone {
    match fixture {
        "approved_before_issue" => ExpiryMilestone::BeforeIssue,
        "issued_before_acknowledgement" => ExpiryMilestone::BeforeAcknowledgement,
        "acknowledged_before_tracking_completes" => ExpiryMilestone::BeforeTrackingCompletion,
        "tracking_before_completion" => ExpiryMilestone::BeforeCompletion,
        other => panic!("no pre-registered milestone for fixture '{other}'"),
    }
}

/// The stage state each fixture expires FROM.
fn pre_registered_from_state(fixture: &str) -> CommandState {
    match fixture {
        "approved_before_issue" => CommandState::Approved,
        "issued_before_acknowledgement" => CommandState::Issued,
        "acknowledged_before_tracking_completes" => CommandState::Acknowledged,
        "tracking_before_completion" => CommandState::Tracking,
        other => panic!("no pre-registered from-state for fixture '{other}'"),
    }
}

// --- Controlled clock (never wall-clock) -------------------------------------------

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

// --- Seam fakes ---------------------------------------------------------------------

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

    fn seed(&self, command: CommandId, valid_until_millis: i64) {
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
                    command_payload_json: "{\"command\":\"p425-fixture\"}".to_string(),
                },
            );
    }

    fn journal_of(&self, command_id: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn current_state(&self, command_id: CommandId) -> CommandState {
        self.journal_of(command_id)
            .last()
            .map(|entry| entry.to)
            .unwrap_or(CommandState::Proposed)
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
            "dispatch_commands".to_string(),
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
    ) -> Result<Option<flexcore_application::command_gates::CommandGateView>, CommandGateStoreError>
    {
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
        Ok(Some(flexcore_application::command_gates::CommandGateView {
            valid_until_millis: record.valid_until_millis,
            journal,
        }))
    }

    fn schedule_revision_state(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
    ) -> Result<ScheduleRevisionUsability, CommandGateStoreError> {
        Ok(ScheduleRevisionUsability::Active)
    }
}

struct FakeEnvelopes;
impl ScheduleEnvelopeSource for FakeEnvelopes {
    fn freshness(
        &self,
        _ctx: &TenantContext,
        _schedule: &ScheduleReference,
    ) -> Result<EnvelopeGateFreshness, GateSeamError> {
        Ok(EnvelopeGateFreshness::Fresh)
    }
}

struct FakeModels;
impl ModelVersionSource for FakeModels {
    fn status(
        &self,
        _ctx: &TenantContext,
        _grid_model_ref: &str,
    ) -> Result<ModelVersionGateStatus, GateSeamError> {
        Ok(ModelVersionGateStatus::Active {
            content_hash: MODEL_HASH.to_string(),
        })
    }
}

struct FakeAssets;
impl AssetCapabilitySource for FakeAssets {
    fn check(
        &self,
        _ctx: &TenantContext,
        _asset: AssetId,
    ) -> Result<AssetGateStatus, GateSeamError> {
        Ok(AssetGateStatus::Usable(DeclaredQuantities {
            active_power: true,
            reactive_power: true,
            soc: true,
            max_up_ramp_mw_per_s: 2.0,
            max_down_ramp_mw_per_s: 2.0,
        }))
    }
}

struct FakeRunner;
impl CommandFeasibilityRunner for FakeRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        Ok(FeasibilityGateVerdict::Passed {
            attempt_id: "attempt-1".to_string(),
            model_id: "expiry-feeder".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.97; max loading 66%".to_string(),
            coverage: "balanced radial expiry-feeder; unbalanced deferred (P4.34)".to_string(),
        })
    }
}

struct FakeAuthz;
impl CommandAuthorizationEvaluator for FakeAuthz {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision {
        if request.role == APPROVER_ROLE {
            AuthorizationDecision::Allowed {
                rule_reference: format!("policy:phase4:{APPROVER_ROLE}:command.approve:asset"),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
    }
}

struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(
        &self,
        envelope: &flexcore_events::envelope::EventEnvelope,
    ) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

/// The scripted simulator transport (the §24 step 8 semantics): a delivery
/// queue (default: application-layer ACCEPTED) and an observation queue
/// (default: empty).
struct FakeTransport {
    deliver_results: Mutex<VecDeque<Result<SimulatedAcceptance, SimulatorTransportError>>>,
    observations: Mutex<VecDeque<Option<SimulatedObservation>>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            deliver_results: Mutex::new(VecDeque::new()),
            observations: Mutex::new(VecDeque::new()),
        }
    }

    fn script_observations(&self, observations: &[Option<SimulatedObservation>]) {
        let mut queue = self.observations.lock().expect("fake transport");
        for observation in observations {
            queue.push_back(*observation);
        }
    }

    fn observation(quantity: TrackedQuantity, value: f64, at_unix_millis: i64) -> SimulatedObservation {
        SimulatedObservation {
            quantity,
            value,
            observed_at_unix_millis: at_unix_millis,
        }
    }
}

impl SimulatorDispatchTransport for FakeTransport {
    fn deliver(
        &self,
        _envelope: &flexcore_application::dispatch_simulator::SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError> {
        self.deliver_results
            .lock()
            .expect("fake transport")
            .pop_front()
            .unwrap_or_else(|| {
                Ok(SimulatedAcceptance::Accepted {
                    acceptance_reference: "acceptance:sim/p425-fixture".to_string(),
                })
            })
    }

    fn observe(
        &self,
        _asset_id: AssetId,
        _at_unix_millis: i64,
    ) -> Result<Option<SimulatedObservation>, SimulatorTransportError> {
        Ok(self
            .observations
            .lock()
            .expect("fake transport")
            .pop_front()
            .unwrap_or(None))
    }
}

/// The fallback audit sink fake (every activation/escalation is recorded;
/// the P4.30 audit wiring consumes these rows in production).
struct FakeFallbackAudit {
    activations: Mutex<Vec<String>>,
}

impl FallbackAuditSink for FakeFallbackAudit {
    fn record_activation(
        &self,
        record: &flexcore_application::fallback_policy::FallbackActivationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        self.activations
            .lock()
            .expect("fake audit")
            .push(format!("{}|{}", record.policy_name, record.trigger.as_str()));
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_activations".to_string(),
            StateChangeOperation::Insert,
            None,
        ))
    }

    fn record_escalation(
        &self,
        _record: &flexcore_application::fallback_policy::FallbackEscalationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_escalations".to_string(),
            StateChangeOperation::Insert,
            None,
        ))
    }
}

// --- The harness ---------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    outbox: &'static FakeEventOutbox,
    transport: &'static FakeTransport,
    audit: &'static FakeFallbackAudit,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
    encoder: &'static DebugEncoder,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels));
    let assets: &'static FakeAssets = Box::leak(Box::new(FakeAssets));
    let runner: &'static FakeRunner = Box::leak(Box::new(FakeRunner));
    let authz: &'static FakeAuthz = Box::leak(Box::new(FakeAuthz));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox =
        Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let transport: &'static FakeTransport = Box::leak(Box::new(FakeTransport::new()));
    let audit: &'static FakeFallbackAudit = Box::leak(Box::new(FakeFallbackAudit {
        activations: Mutex::new(Vec::new()),
    }));
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
        policy: CommandGatePolicy {
            separation_of_duties: SeparationOfDuties::Enforced,
            max_execution_duration: None,
        },
    };
    Harness {
        gates,
        store,
        outbox,
        transport,
        audit,
        clock,
        ids,
        encoder,
    }
}

fn expiry_service(h: &Harness) -> CommandExpiryService<'_> {
    CommandExpiryService {
        store: h.store,
        outbox: h.outbox,
        encoder: h.encoder,
        ids: h.ids,
        clock: h.clock,
    }
}

fn adapter_for(h: &Harness) -> SimulatorDispatchAdapter<'_> {
    SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: SIMULATOR_ID.to_string(),
        },
        h.transport,
        SimulationKeyring::new(SIM_KEY_ID).expect("a non-empty simulation key id"),
        TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 2, 5_000)
            .expect("a valid criterion"),
    )
    .expect("the simulator target is accepted")
}

/// The named OnExpiry fallback policy (the hold-last-accepted-setpoint
/// variant WITH its required validity conditions, §14.4; activates on
/// FAILED and EXPIRED).
fn expiry_fallback_policy() -> FallbackPolicy {
    FallbackPolicy::new(
        "hold-on-expiry-p425",
        FallbackScope::Asset {
            site: SiteId::new(uuid(3)),
            asset: asset_id(),
        },
        vec![FallbackAction::Declare(
            flexcore_application::fallback_policy::FallbackPolicyVariant::HoldLastAcceptedSetpoint,
        )],
        vec![FallbackTrigger::OnFailure, FallbackTrigger::OnExpiry],
    )
    .expect("a valid named fallback policy")
}

fn fallback_engine(h: &Harness, policies: FallbackPolicyRegistry) -> FallbackEngine<'_> {
    FallbackEngine {
        policies: Box::leak(Box::new(policies)),
        store: h.store,
        audit: h.audit,
        outbox: h.outbox,
        encoder: h.encoder,
        ids: h.ids,
        clock: h.clock,
    }
}

fn activation_input(command: CommandId, trigger: FallbackTrigger) -> FallbackActivationInput {
    FallbackActivationInput {
        command_id: command,
        portfolio_id: PortfolioId::new(uuid(2)),
        site_id: SiteId::new(uuid(3)),
        asset_id: asset_id(),
        trigger,
        simulated_operator_review: SimulatedOperatorReview::new(
            "engineer:power-systems",
            "policy reviewed against the site single-line diagram and asset limits",
            500,
        )
        .expect("a valid simulated-operator review"),
    }
}

// --- Command fixture and the gate-chain walk ------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

fn propose_input(command_tag: u8, sequence: u64) -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: CommandId::new(uuid(command_tag)),
        scope: CommandScope {
            tenant_id: TenantId::new(uuid(1)),
            portfolio_id: PortfolioId::new(uuid(2)),
            site_id: SiteId::new(uuid(3)),
            asset_id: asset_id(),
        },
        source: SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        },
        schedule: ScheduleReference {
            schedule_id: ScheduleId::new(uuid(UUID_SCHEDULE)),
            revision: 2,
        },
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
        sequence,
        idempotency_key: format!("idem:p425-op-{command_tag}"),
        evidence: EvidenceReferences {
            grid_model: "grid-model:expiry-feeder@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:p425-fixture".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p425-1".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

/// Walks one fixture command through the REAL gate chain to APPROVED.
fn walk_to_approved(h: &Harness, command_tag: u8, sequence: u64) -> (DispatchCommand, AuthorizationDecisionRecord) {
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input(command_tag, sequence))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed, "fixture validation must pass");
    let feasibility = h
        .gates
        .run_feasibility_gate(&context(), &proposed.command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("feasibility verdict");
    assert!(feasibility.result.passed, "fixture feasibility must pass");
    let approval = h
        .gates
        .record_command_approval(
            &context(),
            &proposed.command,
            &CommandAuthorizationRequest {
                subject: APPROVER.to_string(),
                role: APPROVER_ROLE.to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(asset_id()),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(approval.approved, "fixture approval must succeed");
    (
        proposed.command,
        approval.decision_record.expect("an approval carries its decision record"),
    )
}

/// Issue to the simulator (APPROVED → ISSUED, accepted → ACKNOWLEDGED).
fn issue_to_simulator(
    h: &Harness,
    command: &DispatchCommand,
    decision_record: &AuthorizationDecisionRecord,
) {
    let adapter = adapter_for(h);
    let outcome = adapter
        .issue(&context(), command, decision_record, &actor("svc:dispatcher"))
        .expect("the issue succeeds");
    assert_eq!(outcome.issued.to, CommandState::Issued);
    assert_eq!(
        outcome.acknowledged.as_ref().map(|t| t.to),
        Some(CommandState::Acknowledged)
    );
}

fn event_counts(h: &Harness) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for write in h.outbox.writes() {
        let name = write.payload().split('(').next().unwrap_or_default().to_string();
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

/// The shared post-expiry assertions (ledger P4.25 step 3): no expired
/// command is issued, acknowledged-as-fresh, or completed — on the journal,
/// on the adapter's post-expiry refusals, and on the frozen-edge oracle.
fn assert_no_progress_after_expiry(h: &Harness, command: &DispatchCommand, decision_record: &AuthorizationDecisionRecord) {
    // The journal ends at EXPIRED; no ISSUED / ACKNOWLEDGED / COMPLETED row
    // exists after the EXPIRED entry.
    let journal = h.store.journal_of(command.command_id());
    assert_eq!(journal.last().map(|e| e.to), Some(CommandState::Expired));
    let expired_index = journal
        .iter()
        .position(|e| e.to == CommandState::Expired)
        .expect("the EXPIRED entry");
    assert!(
        !journal[expired_index..]
            .iter()
            .any(|e| matches!(e.to, CommandState::Issued | CommandState::Acknowledged | CommandState::Completed)),
        "no expired command is issued, acknowledged-as-fresh, or completed"
    );

    // The adapter refuses every forward operation on the EXPIRED command.
    let adapter = adapter_for(h);
    let error = adapter
        .issue(&context(), command, decision_record, &actor("svc:dispatcher"))
        .expect_err("an EXPIRED command cannot be issued");
    assert!(matches!(
        error,
        DispatchAdapterError::WrongState { current: CommandState::Expired, operation: "issue" }
    ));
    let error = adapter
        .poll_tracking(&context(), command, &actor("svc:dispatcher"))
        .expect_err("an EXPIRED command cannot re-enter tracking");
    assert!(matches!(
        error,
        DispatchAdapterError::WrongState { current: CommandState::Expired, operation: "poll_tracking" }
    ));

    // The frozen-edge oracle (P0.11): EXPIRED's ONLY outgoing edge is
    // → FALLBACK (edge 27, where policy requires it); it is NOT terminal,
    // and there is no edge to ISSUED / ACKNOWLEDGED / COMPLETED.
    assert!(!flexcore_domain::command_state::is_terminal(CommandState::Expired));
    for &to in CommandState::ALL.iter() {
        let result = try_transition(CommandState::Expired, to);
        if to == CommandState::Fallback {
            assert_eq!(result, Ok(CommandState::Fallback));
        } else {
            assert!(result.is_err(), "EXPIRED -> {} must not exist", to.name());
        }
    }
}

/// The `DispatchExpired` event check (ledger P4.25 step 4): exactly ONE
/// DispatchExpired was emitted for the fixture.
fn assert_one_dispatch_expired(h: &Harness) {
    let counts = event_counts(h);
    assert_eq!(
        counts.get("DispatchExpired").copied().unwrap_or(0),
        1,
        "exactly one DispatchExpired per case; all events: {counts:?}"
    );
}

// ============================================================================
// Fixture 1 — APPROVED expiring before issue
// ============================================================================

#[test]
fn fixture_approved_expiring_before_issue_reaches_expired_with_the_milestone() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 10, 17);
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);

    // The controlled clock passes the validity window (ends 70_000). The
    // adapter REFUSES the issue (never issued) and leaves the state — the
    // EXPIRED transition is the expiry sweep's (recorded decision D-P4.15-1).
    h.clock.advance_to(80_000);
    let adapter = adapter_for(&h);
    let error = adapter
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect_err("an expired APPROVED command is refused, never issued");
    assert!(matches!(error, DispatchAdapterError::ExpiredCommandRefused { .. }));
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);

    // The P4.15 sweep applies the pre-registered expiry: APPROVED → EXPIRED,
    // milestone BeforeIssue, expiry time recorded.
    let outcome: ExpiryOutcome = expiry_service(&h)
        .apply_expiry(&context(), scope(), command.command_id(), &actor("svc:expiry-sweep"))
        .expect("the lapsed APPROVED command expires");
    assert_eq!(outcome.milestone, pre_registered_milestone("approved_before_issue"));
    assert_eq!(outcome.transition.from, pre_registered_from_state("approved_before_issue"));
    assert_eq!(outcome.transition.to, CommandState::Expired);
    assert_eq!(outcome.expired_at_unix_millis, 80_000, "the expiry time is recorded");
    assert_eq!(
        outcome.transition.reason.as_deref(),
        Some(pre_registered_milestone("approved_before_issue").reason_text()),
        "the journal reason IS the milestone not reached"
    );
    assert_one_dispatch_expired(&h);
    assert_no_progress_after_expiry(&h, &command, &decision_record);

    // Fallback where policy REQUIRES it: the named OnExpiry policy
    // activates through the frozen EXPIRED → FALLBACK edge (27).
    let mut registry = FallbackPolicyRegistry::new();
    registry
        .register_policy(expiry_fallback_policy())
        .expect("the asset entry registers");
    let engine = fallback_engine(&h, registry);
    let decision = engine
        .activate_fallback(&context(), activation_input(command.command_id(), FallbackTrigger::OnExpiry))
        .expect("the activation evaluates");
    match decision {
        FallbackDecision::Activated { transition, .. } => {
            assert_eq!(transition.from, CommandState::Expired);
            assert_eq!(transition.to, CommandState::Fallback);
        }
        other => panic!("the named OnExpiry policy must activate: {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Fallback);
    let counts = event_counts(&h);
    assert!(counts.contains_key("DispatchFallbackActivated"));
    assert_eq!(
        h.audit.activations.lock().expect("fake audit").len(),
        1,
        "the activation is audit-recorded"
    );
}

// ============================================================================
// Fixture 2 — ISSUED expiring before acknowledgement
// ============================================================================

#[test]
fn fixture_issued_expiring_before_acknowledgement_reaches_expired_with_the_milestone() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 11, 17);
    issue_to_simulator(&h, &command, &decision_record);
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Issued);

    // The window passes with no acknowledgement: the adapter cannot advance
    // the command (tracking applies only from ACKNOWLEDGED/TRACKING), and
    // nothing acknowledges it as fresh.
    h.clock.advance_to(80_000);
    let adapter = adapter_for(&h);
    let error = adapter
        .poll_tracking(&context(), &command, &actor("svc:dispatcher"))
        .expect_err("an expired ISSUED command leaves the tracking path");
    assert!(matches!(
        error,
        DispatchAdapterError::WrongState { current: CommandState::Issued, operation: "poll_tracking" }
    ));

    // The sweep: ISSUED → EXPIRED, milestone BeforeAcknowledgement.
    let outcome: ExpiryOutcome = expiry_service(&h)
        .apply_expiry(&context(), scope(), command.command_id(), &actor("svc:expiry-sweep"))
        .expect("the lapsed ISSUED command expires");
    assert_eq!(outcome.milestone, pre_registered_milestone("issued_before_acknowledgement"));
    assert_eq!(outcome.transition.from, pre_registered_from_state("issued_before_acknowledgement"));
    assert_eq!(outcome.transition.to, CommandState::Expired);
    assert_eq!(
        outcome.transition.reason.as_deref(),
        Some(pre_registered_milestone("issued_before_acknowledgement").reason_text())
    );
    assert_one_dispatch_expired(&h);
    assert_no_progress_after_expiry(&h, &command, &decision_record);

    // Fallback where policy requires it (the risky case: the command is
    // already at the asset — the OnExpiry policy activates).
    let mut registry = FallbackPolicyRegistry::new();
    registry
        .register_policy(expiry_fallback_policy())
        .expect("the asset entry registers");
    let engine = fallback_engine(&h, registry);
    let decision = engine
        .activate_fallback(&context(), activation_input(command.command_id(), FallbackTrigger::OnExpiry))
        .expect("the activation evaluates");
    assert!(matches!(decision, FallbackDecision::Activated { .. }));
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Fallback);
}

// ============================================================================
// Fixture 3 — ACKNOWLEDGED expiring before tracking completes
// ============================================================================

#[test]
fn fixture_acknowledged_expiring_before_tracking_completes_expires_at_the_poll() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 12, 17);
    issue_to_simulator(&h, &command, &decision_record);
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Acknowledged);

    // The window passes BEFORE any telemetry observation: the poll is a
    // transition attempt, and expiry is checked at it (the
    // check-at-transition path beside the sweep) — ACKNOWLEDGED → EXPIRED
    // with the BeforeTrackingCompletion milestone.
    h.clock.advance_to(80_000);
    let adapter = adapter_for(&h);
    let poll = adapter
        .poll_tracking(&context(), &command, &actor("svc:dispatcher"))
        .expect("the expiry poll");
    match poll.terminal.expect("the lapse terminates") {
        TrackingTerminal::Expired { transition } => {
            assert_eq!(transition.from, pre_registered_from_state("acknowledged_before_tracking_completes"));
            assert_eq!(transition.to, CommandState::Expired);
            assert_eq!(
                transition.reason.as_deref(),
                Some(pre_registered_milestone("acknowledged_before_tracking_completes").reason_text())
            );
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    assert_eq!(
        h.store.current_state(command.command_id()),
        CommandState::Expired
    );
    assert_one_dispatch_expired(&h);
    assert_no_progress_after_expiry(&h, &command, &decision_record);
    // The poll path recorded NO tracking entry (no telemetry was observed).
    assert!(
        !h.store
            .journal_of(command.command_id())
            .iter()
            .any(|e| e.to == CommandState::Tracking)
    );

    // Control: a scope with NO named policy gets the explicit default entry
    // — operator intervention required, NO state transition, the decision
    // audit-recorded (never silence, never an automatic setpoint change).
    let engine = fallback_engine(&h, FallbackPolicyRegistry::new());
    let decision = engine
        .activate_fallback(&context(), activation_input(command.command_id(), FallbackTrigger::OnExpiry))
        .expect("the default entry evaluates");
    match decision {
        FallbackDecision::InterventionRequired { .. } => {}
        other => panic!("the explicit default entry must apply: {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Expired);
}

// ============================================================================
// Fixture 4 — TRACKING expiring before completion
// ============================================================================

#[test]
fn fixture_tracking_expiring_before_completion_expires_at_the_poll() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 13, 17);
    issue_to_simulator(&h, &command, &decision_record);

    // Enter tracking at 6_000 (the first observation).
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    let adapter = adapter_for(&h);
    adapter
        .poll_tracking(&context(), &command, &actor("svc:dispatcher"))
        .expect("first poll");
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Tracking);

    // The window ends while tracking; the next poll expires the command —
    // TRACKING → EXPIRED, milestone BeforeCompletion. The criterion was
    // never satisfied: no COMPLETED row exists.
    h.clock.advance_to(75_000);
    let poll = adapter
        .poll_tracking(&context(), &command, &actor("svc:dispatcher"))
        .expect("the expiry poll");
    match poll.terminal.expect("the lapse terminates") {
        TrackingTerminal::Expired { transition } => {
            assert_eq!(transition.from, pre_registered_from_state("tracking_before_completion"));
            assert_eq!(transition.to, CommandState::Expired);
            assert_eq!(
                transition.reason.as_deref(),
                Some(pre_registered_milestone("tracking_before_completion").reason_text())
            );
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Expired);
    assert_one_dispatch_expired(&h);
    assert_no_progress_after_expiry(&h, &command, &decision_record);

    // Fallback where policy requires it: the named OnExpiry policy
    // activates EXPIRED → FALLBACK.
    let mut registry = FallbackPolicyRegistry::new();
    registry
        .register_policy(expiry_fallback_policy())
        .expect("the asset entry registers");
    let engine = fallback_engine(&h, registry);
    let decision = engine
        .activate_fallback(&context(), activation_input(command.command_id(), FallbackTrigger::OnExpiry))
        .expect("the activation evaluates");
    assert!(matches!(decision, FallbackDecision::Activated { .. }));
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Fallback);
}

// ============================================================================
// Controlled-clock discipline: expiry never fires early; the boundary is
// end-exclusive; a TriggerNotConfigured policy leaves EXPIRED untouched
// ============================================================================

#[test]
fn expiry_never_fires_before_the_window_ends_and_the_boundary_is_end_exclusive() {
    // 1 ms BEFORE the bound: the domain guard refuses (ValidityNotLapsed).
    let h = harness(8_000);
    let command = CommandId::new(uuid(14));
    h.store.seed(command, 9_000);
    h.store
        .journals
        .lock()
        .expect("fake store")
        .insert(command.to_string(), Vec::new());
    // Seed the APPROVED journal by walking the real gates is unnecessary
    // here: the fixture above covers the full walk; this fixture pins the
    // GUARD on a store-seeded APPROVED command.
    // A store-seeded command with an empty journal is PROPOSED — expiry
    // refuses it as a non-expirable state; the lapsed-guard case is pinned
    // end-to-end by `fixture_approved_expiring_before_issue...` (the adapter
    // refusal) and by the domain guard below.
    let error = expiry_service(&h)
        .apply_expiry(&context(), scope(), command, &actor("svc:expiry-sweep"))
        .expect_err("a PROPOSED command cannot expire");
    assert!(matches!(error, CommandExpiryError::WrongState { .. }));

    // The domain guard (the engine the sweep applies through): expiry never
    // fires before the window ends — re-pinned here directly.
    use flexcore_domain::command_lifecycle::CommandLifecycle;
    let mut lifecycle =
        CommandLifecycle::propose(command, ValidUntil::from_unix_millis(9_000).unwrap())
            .expect("propose");
    let error = lifecycle
        .record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), &TestClock {
            now_unix_millis: Mutex::new(8_000),
        })
        .expect_err("expiry never fires early");
    assert!(matches!(error, LifecycleError::ValidityNotLapsed { .. }));
    // At the boundary instant (now == valid_until, §10.3 end-exclusive) the
    // guard allows the expiry.
    let mut lifecycle =
        CommandLifecycle::propose(command, ValidUntil::from_unix_millis(9_000).unwrap())
            .expect("propose");
    let transition = lifecycle
        .record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), &TestClock {
            now_unix_millis: Mutex::new(9_000),
        })
        .expect("the boundary instant is expired");
    assert_eq!(transition.to, CommandState::Expired);
}

/// The P4.17 recorded non-activating outcome: a named policy exists but does
/// NOT list EXPIRED as a trigger → `TriggerNotConfigured` — no transition,
/// no event, the decision audit-recorded. EXPIRED → FALLBACK fires ONLY
/// where the policy requires it (P0.11 edge 27 discipline).
#[test]
fn fallback_fires_only_where_the_policy_requires_it() {
    // Build an EXPIRED command through the full fixture path first.
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 15, 17);
    h.clock.advance_to(80_000);
    expiry_service(&h)
        .apply_expiry(&context(), scope(), command.command_id(), &actor("svc:expiry-sweep"))
        .expect("the lapsed APPROVED command expires");

    // A FAILED-only policy evaluated on EXPIRED: no transition, no event.
    let failed_only = FallbackPolicy::new(
        "intervene-on-failure-only",
        FallbackScope::Asset {
            site: SiteId::new(uuid(3)),
            asset: asset_id(),
        },
        vec![FallbackAction::Declare(
            flexcore_application::fallback_policy::FallbackPolicyVariant::RequireOperatorIntervention,
        )],
        vec![FallbackTrigger::OnFailure],
    )
    .expect("a valid FAILED-only policy");
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(failed_only).expect("the asset entry registers");
    let writes_before = h.outbox.len();
    let engine = fallback_engine(&h, registry);
    let decision = engine
        .activate_fallback(&context(), activation_input(command.command_id(), FallbackTrigger::OnExpiry))
        .expect("the evaluation succeeds");
    match decision {
        FallbackDecision::TriggerNotConfigured { .. } => {}
        other => panic!("a FAILED-only policy must not fire on EXPIRED: {other:?}"),
    }
    // EXPIRED is untouched; no fallback event was emitted.
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Expired);
    assert_eq!(h.outbox.len(), writes_before);
    assert!(matches!(decision, FallbackDecision::TriggerNotConfigured { .. }));
    // The decision is still audit-visible.
    assert_eq!(h.audit.activations.lock().expect("fake audit").len(), 1);
}
