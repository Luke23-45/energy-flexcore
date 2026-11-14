// P4.31 — Fault injection and dispatch resilience (report §19.2 layers 4
//         and 6, §19.1, §23 Stage 4, §24 steps 5–8; ledger P4.31 steps 1
//         and 4; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md`).
//
// PRE-REGISTERED FAULT MATRIX (expectations written BEFORE execution; all
// tests are UNRUN under the loop bans — the human build/test pass is the
// gate). Every fault's expected outcome is a named rejection, a hold, or a
// fallback — NEVER a silent pass. Faults are modelled at the
// adapter/outbox SEAM level with in-test fakes over the public ports (the
// P1.16 pattern; flexcore-application cannot name the broker, the
// simulator, or any infrastructure crate):
//
// | ID  | Fault                                          | Seam                          | Pre-registered outcome |
// |-----|------------------------------------------------|-------------------------------|------------------------|
// | F1  | Dropped NATS message (publish never acked)     | outbox seam (EventOutbox)     | the entry stays PENDING with its attempt count (no loss, never silently dropped); recovery drains it; the command state is store-authoritative and unaffected |
// | F1b | Duplicated NATS message (same idempotency key) | outbox seam                   | the second append is refused with the named DuplicateKey; exactly ONE effective entry per key |
// | F2  | Reordered delivery (seq 3 arrives before 2)    | receive-point transport       | the out-of-order envelope is refused as an application-layer NACK (`out_of_order_sequence`); the inner transport NEVER sees it; the guard never advances on a refusal |
// | F4  | Clock skew forward past validity, at issue     | controlled clock              | the issue is REFUSED (`ExpiredCommandRefused`); the sweep EXPIRES the command with the milestone not reached recorded; never completed |
// | F5  | Clock skew forward during tracking             | controlled clock              | the poll EXPIRES the command (`BeforeCompletion`) even with in-tolerance samples scripted — expiry wins over completion |
// | F6  | Stale telemetry at the feasibility check       | feasibility runner seam       | the FailClosed verdict REJECTS the command with `fail_closed:telemetry:STALE...` named — never a pass (§17.5 invariant 9, P4.08) |
// | F7  | Missing telemetry in tracking                  | simulator transport           | the poll is the named `NoTelemetryObserved` error with NO transition (hold); the sweep later EXPIRES the command — never a completion |
// | F8  | Unavailable asset at issue (transport down)    | simulator transport           | ISSUED → FAILED (`TransportFailure`); the site fallback policy ACTIVATES FALLBACK with its bounded hold; the hold ESCALATES (never indefinite); recovery requires a fresh command (no FALLBACK → ISSUED) |
// | F9a | Solver timeout (seam error)                    | feasibility runner seam       | the named `Seam` error surfaces with NO verdict, NO transition, NO event — the command HELD in VALIDATED, never a pass |
// | F9b | Solver timeout reported as a verdict           | feasibility runner seam       | the FailClosed verdict REJECTS with `fail_closed:validator_timeout...` named (the P4.08 policy) |
// | F10 | Plugin crash (panicking proposal source — NO plugin kernel exists yet, the crash is modelled as a panicking seam) | feasibility runner seam | the panic PROPAGATES (fail-closed by construction): no partial transition, no event, nothing issued; the supervisor owns containment (recorded limitation) |
// | F11 | Partial fleet response (3 assets: accept / NACK / timeout) | simulator transport + fleet acknowledgement | PER-ASSET acknowledgement (Accepted / Refused / TimedOut); the fleet state is PartiallyAccepted, never inferred from the first responder; fallback fires ONLY for the failed assets |
//
// Determinism: every timestamp comes from a controlled, advanceable
// clock (never wall-clock); the fakes are scripted tables; no randomness.
// Nothing here runs against a real broker, database, or simulator.

use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_expiry::{
    sweep_validity_windows, CommandExpiryService, EventScope, ExpiryCandidate,
};
use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthzAction, CommandAuthorizationEvaluator,
    CommandAuthorizationRequest, CommandFeasibilityRefs, CommandFeasibilityRunner,
    CommandGateError, CommandGatePolicy, CommandGateStore, CommandGateStoreError, CommandGateView,
    CommandGates, DeclaredQuantities, EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError,
    ModelVersionGateStatus, ModelVersionSource, ProposedCommandRecord, ProposeCommandInput,
    ScheduleEnvelopeSource, ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::command_record::{
    AssetAcknowledgementState, FleetAcknowledgement, FleetAcknowledgementState,
};
use flexcore_application::dispatch_security::{
    sign_dispatch_payload, DispatchPayloadBinding, PayloadBoundVerifier, VerifyingDispatchTransport,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, DispatchAdapterError, SimulatedAcceptance, SimulatedObservation,
    SimulationKeyring, SimulatorDispatchAdapter, SimulatorDispatchEnvelope,
    SimulatorDispatchTransport, SimulatorTargetValue, SimulatorTransportError,
    SimulatorTransportFailure, TrackingAcceptanceCriterion, TrackingTerminal, TrackedQuantity,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::fallback_policy::{
    BoundedHoldInterval, EscalationTarget, FallbackAction, FallbackActivationInput,
    FallbackActivationRecord, FallbackAuditError, FallbackAuditSink, FallbackDecision,
    FallbackEngine, FallbackEscalationRecord, FallbackPolicy, FallbackPolicyRegistry,
    FallbackScope, FallbackTrigger, HoldExpiryEvaluation, HoldValidityConditions,
    SimulatedOperatorReview,
};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, ExpiryMilestone, LifecycleClock,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::event_outbox::{EventOutboxError, OutboxWrite};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// ============================================================================
// Pre-registered constants
// ============================================================================

const MODEL_HASH: &str = "fnv1a64:p431faultmodelhash";
const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-admin";
const VALID_UNTIL_MILLIS: i64 = 70_000;

// ============================================================================
// Fixture identity
// ============================================================================

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[14] = 0xD1; // P4.31 fault-suite fixture marker
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
}

fn tenant_context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(uuid(1)),
        subject: "test:principal".to_string(),
    }))
}

fn asset_id(tag: u8) -> AssetId {
    AssetId::new(uuid(tag))
}

fn schedule_id_text() -> String {
    ScheduleId::new(uuid(6)).to_string()
}

fn scope(asset_tag: u8) -> EventScope {
    EventScope {
        portfolio_id: PortfolioId::new(uuid(2)),
        site_id: SiteId::new(uuid(3)),
        asset_id: asset_id(asset_tag),
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

// ============================================================================
// Controlled clock (never wall-clock)
// ============================================================================

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

// ============================================================================
// Seam fakes (fail closed by construction)
// ============================================================================

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

    fn seed_revision_active(&self, revision: u64) {
        self.revisions
            .lock()
            .expect("fake store")
            .insert((schedule_id_text(), revision), ScheduleRevisionUsability::Active);
    }

    fn journal_of(&self, command_id: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn state_of(&self, command_id: CommandId) -> CommandState {
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

/// The feasibility runner fake: a queue of scripted verdicts (or seam
/// errors). The DEFAULT is the passing verdict so the shared walk-to-
/// fixtures stay one line; fault tests script the fault verdict first.
struct ScriptedRunner {
    results: Mutex<VecDeque<Result<FeasibilityGateVerdict, GateSeamError>>>,
}

impl ScriptedRunner {
    fn with_default_pass() -> Self {
        Self {
            results: Mutex::new(VecDeque::new()),
        }
    }

    fn script(&self, result: Result<FeasibilityGateVerdict, GateSeamError>) {
        self.results
            .lock()
            .expect("scripted runner")
            .push_back(result);
    }
}

impl CommandFeasibilityRunner for ScriptedRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        self.results
            .lock()
            .expect("scripted runner")
            .pop_front()
            .unwrap_or_else(|| {
                Ok(FeasibilityGateVerdict::Passed {
                    attempt_id: "attempt-fixture".to_string(),
                    model_id: "feeder-a".to_string(),
                    model_content_hash: MODEL_HASH.to_string(),
                    margins: "min v_pu 0.97; max loading 71%".to_string(),
                    coverage: "balanced radial feeder-a; unbalanced deferred (P4.34)".to_string(),
                })
            })
    }
}

/// The plugin-crash fake (F10): the plugin kernel does NOT exist yet
/// (v0.5.0), so the crash is modelled as a PANICKING proposal source —
/// the same shape a research-plugin proposal path would present. The
/// pre-registered outcome: the panic propagates; NO partial transition,
/// NO event, NOTHING issued (fail-closed by construction; containment is
/// the supervisor's recorded duty).
struct PanickingRunner;

impl CommandFeasibilityRunner for PanickingRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        panic!("plugin proposal source crashed: poisoned proposal state (simulated plugin crash)");
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
                rule_reference: "policy:phase4:tenant-admin:command.approve:asset".to_string(),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
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

/// The fault-scripted simulator transport: a per-asset deliver queue
/// (default: application-layer ACCEPTED with a synthesized acceptance
/// record) and a global observation queue (default: no pending sample).
/// The delivered list records every envelope that REACHED the inner
/// transport — the receive-point refusal assertions read it.
struct FaultTransport {
    deliver_results:
        Mutex<HashMap<String, VecDeque<Result<SimulatedAcceptance, SimulatorTransportError>>>>,
    observations: Mutex<VecDeque<Option<SimulatedObservation>>>,
    delivered: Mutex<Vec<SimulatorDispatchEnvelope>>,
}

impl FaultTransport {
    fn new() -> Self {
        Self {
            deliver_results: Mutex::new(HashMap::new()),
            observations: Mutex::new(VecDeque::new()),
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn script_deliver_for_asset(
        &self,
        asset: AssetId,
        result: Result<SimulatedAcceptance, SimulatorTransportError>,
    ) {
        self.deliver_results
            .lock()
            .expect("fault transport")
            .entry(asset.to_string())
            .or_default()
            .push_back(result);
    }

    fn script_observations(&self, observations: &[Option<SimulatedObservation>]) {
        let mut queue = self.observations.lock().expect("fault transport");
        for observation in observations {
            queue.push_back(*observation);
        }
    }

    fn delivered(&self) -> Vec<SimulatorDispatchEnvelope> {
        self.delivered.lock().expect("fault transport").clone()
    }
}

impl SimulatorDispatchTransport for FaultTransport {
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError> {
        self.delivered
            .lock()
            .expect("fault transport")
            .push(envelope.clone());
        let scripted = self
            .deliver_results
            .lock()
            .expect("fault transport")
            .get_mut(&envelope.asset_id.to_string())
            .and_then(|queue| queue.pop_front());
        scripted.unwrap_or_else(|| {
            Ok(SimulatedAcceptance::Accepted {
                acceptance_reference: format!("acceptance:sim/{}", envelope.command_id),
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
            .expect("fault transport")
            .pop_front()
            .unwrap_or(None))
    }
}

/// The fallback audit fake: records every activation and escalation
/// (fail-closed: a refusal is an error, never a skipped record).
struct FakeFallbackAudit {
    activations: Mutex<Vec<String>>,
    escalations: Mutex<Vec<String>>,
}

impl FakeFallbackAudit {
    fn new() -> Self {
        Self {
            activations: Mutex::new(Vec::new()),
            escalations: Mutex::new(Vec::new()),
        }
    }

    fn activation_count(&self) -> usize {
        self.activations.lock().expect("audit fake").len()
    }

    fn escalation_count(&self) -> usize {
        self.escalations.lock().expect("audit fake").len()
    }
}

impl FallbackAuditSink for FakeFallbackAudit {
    fn record_activation(
        &self,
        record: &FallbackActivationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        self.activations
            .lock()
            .expect("audit fake")
            .push(format!("{}:{}", record.command_id, record.outcome));
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_activations".to_string(),
            StateChangeOperation::Insert,
            Some(record.tenant_id),
        ))
    }

    fn record_escalation(
        &self,
        record: &FallbackEscalationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        self.escalations
            .lock()
            .expect("audit fake")
            .push(format!("{}:{}", record.command_id, record.outcome));
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_escalations".to_string(),
            StateChangeOperation::Insert,
            Some(record.tenant_id),
        ))
    }
}

// ============================================================================
// Harness and fixtures
// ============================================================================

struct Harness {
    gates: CommandGates<'static>,
    expiry: CommandExpiryService<'static>,
    fallback: FallbackEngine<'static>,
    runner: Option<&'static ScriptedRunner>,
    store: &'static FakeStore,
    transport: &'static FaultTransport,
    audit: &'static FakeFallbackAudit,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
}

fn hold_then_return_registry() -> FallbackPolicyRegistry {
    // The §14.4 site policy of the fault suite: hold the last accepted
    // setpoint for a BOUNDED interval (validity conditions recorded),
    // escalate to return-to-local on expiry; activates on FAILED and
    // EXPIRED.
    let mut registry = FallbackPolicyRegistry::new();
    let policy = FallbackPolicy::new(
        "hold-then-return",
        FallbackScope::Site {
            site: SiteId::new(uuid(3)),
        },
        vec![FallbackAction::Hold {
            interval: BoundedHoldInterval::from_millis(60_000).expect("positive interval"),
            validity: HoldValidityConditions::new(vec![
                "topology unchanged since issue".to_string(),
                "asset state consistent with the held setpoint".to_string(),
                "no new network constraint recorded".to_string(),
            ])
            .expect("validity conditions are non-empty"),
            escalation: EscalationTarget::ReturnToLocalEms,
        }],
        vec![FallbackTrigger::OnFailure, FallbackTrigger::OnExpiry],
    )
    .expect("valid policy");
    registry.register_policy(policy).expect("the policy registers");
    registry
}

fn harness() -> Harness {
    let runner: &'static ScriptedRunner = Box::leak(Box::new(ScriptedRunner::with_default_pass()));
    build_harness(runner, Some(runner))
}

fn harness_with_panicking_runner() -> Harness {
    let runner: &'static PanickingRunner = Box::leak(Box::new(PanickingRunner));
    build_harness(runner, None)
}

fn build_harness(
    runner: &'static dyn CommandFeasibilityRunner,
    scripted: Option<&'static ScriptedRunner>,
) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels));
    let assets: &'static FakeAssets = Box::leak(Box::new(FakeAssets));
    let authz: &'static FakeAuthz = Box::leak(Box::new(FakeAuthz));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox =
        Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(5_000)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let transport: &'static FaultTransport = Box::leak(Box::new(FaultTransport::new()));
    let audit: &'static FakeFallbackAudit = Box::leak(Box::new(FakeFallbackAudit::new()));
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
    let expiry = CommandExpiryService {
        store,
        outbox,
        encoder,
        ids,
        clock,
    };
    let policies: &'static FallbackPolicyRegistry =
        Box::leak(Box::new(hold_then_return_registry()));
    let fallback = FallbackEngine {
        policies,
        store,
        audit,
        outbox,
        encoder,
        ids,
        clock,
    };
    Harness {
        gates,
        expiry,
        fallback,
        runner: scripted,
        store,
        transport,
        audit,
        outbox,
        clock,
        ids,
    }
}

impl Harness {
    /// The scripted runner of THIS harness (absent only on the
    /// panicking-runner harness, where nothing can be scripted).
    fn fault_runner(&self) -> &'static ScriptedRunner {
        self.runner
            .expect("this harness uses a scripted feasibility runner")
    }
}

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).unwrap(),
    )
    .unwrap()
}

fn propose_input(command_tag: u8, asset_tag: u8, sequence: u64) -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: CommandId::new(uuid(command_tag)),
        scope: CommandScope {
            tenant_id: TenantId::new(uuid(1)),
            portfolio_id: PortfolioId::new(uuid(2)),
            site_id: SiteId::new(uuid(3)),
            asset_id: asset_id(asset_tag),
        },
        source: SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        },
        schedule: ScheduleReference {
            schedule_id: ScheduleId::new(uuid(6)),
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
        idempotency_key: format!("idem:p431-op-{command_tag}"),
        evidence: EvidenceReferences {
            grid_model: "grid-model:feeder-a@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:p431-fixture".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p431".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

/// Walks one command through the REAL gate chain to APPROVED and returns
/// the command plus the P4.13 authorization decision record the adapter
/// cross-checks at issue.
fn walk_to_approved(
    h: &Harness,
    command_tag: u8,
    asset_tag: u8,
    sequence: u64,
) -> (DispatchCommand, AuthorizationDecisionRecord) {
    h.store.seed_revision_active(2);
    let proposed = h
        .gates
        .create_proposed_command(
            &tenant_context(),
            propose_input(command_tag, asset_tag, sequence),
        )
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed, "fixture validation must pass");
    let feasibility = h
        .gates
        .run_feasibility_gate(
            &tenant_context(),
            &proposed.command,
            &feasibility_refs(),
            &actor("svc:grid-gate"),
        )
        .expect("feasibility verdict");
    assert!(feasibility.result.passed, "fixture feasibility must pass");
    let request = CommandAuthorizationRequest {
        subject: APPROVER.to_string(),
        role: APPROVER_ROLE.to_string(),
        action: CommandAuthzAction::Approve,
        scope: ApprovalScope::Asset(asset_id(asset_tag)),
    };
    let approval = h
        .gates
        .record_command_approval(
            &tenant_context(),
            &proposed.command,
            &request,
            "limits:+-2.0 MW, Q +-0.5 MVAr",
            ValidUntil::from_unix_millis(100_000).unwrap(),
        )
        .expect("authorization verdict");
    assert!(approval.approved, "fixture approval must succeed");
    (
        proposed.command,
        approval
            .decision_record
            .expect("an approval carries its decision record"),
    )
}

fn event_names(h: &Harness) -> Vec<&'static str> {
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
        "DispatchIssued",
        "DispatchAcknowledged",
        "DispatchFailed",
        "DispatchCompleted",
        "DispatchExpired",
        "DispatchFallbackActivated",
        "FeasibilityCheckCompleted",
    ] {
        if debug_payload.starts_with(candidate) {
            return candidate;
        }
    }
    "unknown"
}

fn adapter(h: &Harness) -> SimulatorDispatchAdapter<'_> {
    let keyring = SimulationKeyring::new("p431-sim-key").expect("keyring builds");
    let criterion =
        TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 2, 5_000)
            .expect("criterion builds");
    SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: "sim://p431-fault-scenario".to_string(),
        },
        h.transport,
        keyring,
        criterion,
    )
    .expect("the simulator target is accepted")
}

fn simulated_review() -> SimulatedOperatorReview {
    SimulatedOperatorReview::new(
        "engineer:power-systems",
        "SIMULATED review of the hold-then-return policy for the fault suite",
        1_000,
    )
    .expect("review builds")
}

fn observation(value: f64, at_unix_millis: i64) -> SimulatedObservation {
    SimulatedObservation {
        quantity: TrackedQuantity::ActivePowerMW,
        value,
        observed_at_unix_millis: at_unix_millis,
    }
}

// ============================================================================
// F1 — dropped NATS message (outbox seam)
// ============================================================================

/// PRE-REGISTERED (F1): a publish that is attempted but NEVER acked
/// (the broker drop) leaves the entry PENDING with its attempt count —
/// no loss, no silent drop — and the command state is store-authoritative
/// and unaffected. Recovery drains the entry (at-least-once delivery).
#[test]
fn f1_a_dropped_publication_stays_pending_with_its_attempt_count_and_drains_on_recovery() {
    let h = harness();
    let (command, _authorization) = walk_to_approved(&h, 0x41, 4, 1);
    assert_eq!(h.store.state_of(command.command_id()), CommandState::Approved);

    // The gate events are in the outbox, pending, zero attempts.
    let pending = h.outbox.pending(100).expect("pending read");
    assert!(!pending.is_empty(), "the gate events are pending");
    assert!(pending.iter().all(|entry| entry.attempt_count() == 0));

    // FAULT: the publisher attempts the FIRST entry's publish and the
    // broker never acks (the drop). Attempt recorded, delivery NOT.
    let dropped_key = pending[0].idempotency_key().to_string();
    h.outbox
        .record_attempt(&dropped_key)
        .expect("the attempt is recorded");

    // FAULT ASSERTION: the entry is STILL PENDING with exactly one
    // attempt — never silently dropped, never marked delivered.
    let pending = h.outbox.pending(100).expect("pending read");
    let dropped = pending
        .iter()
        .find(|entry| entry.idempotency_key() == dropped_key)
        .expect("the dropped entry is still pending (no loss)");
    assert_eq!(dropped.attempt_count(), 1);

    // The command state is STORE-authoritative, not event-dependent: the
    // drop changed nothing downstream (hold, no state corruption).
    assert_eq!(h.store.state_of(command.command_id()), CommandState::Approved);

    // RECOVERY: the retry publishes and the broker acks; the entry leaves
    // the pending set; the write is retained (zero loss — redelivery is
    // the broker's at-least-once obligation, §15.4).
    h.outbox
        .mark_delivered(&dropped_key)
        .expect("the recovered publish is marked delivered");
    let pending = h.outbox.pending(100).expect("pending read");
    assert!(
        !pending
            .iter()
            .any(|entry| entry.idempotency_key() == dropped_key),
        "the recovered entry is no longer pending"
    );
    assert_eq!(
        h.outbox.writes().len(),
        pending.len() + 1,
        "the write is retained after delivery (no loss)"
    );
}

// ============================================================================
// F1b — duplicated NATS message (outbox seam)
// ============================================================================

/// PRE-REGISTERED (F1b): a duplicated publication under ONE idempotency
/// key is refused with the named `DuplicateKey` — exactly ONE effective
/// entry per key (the §15.4/P4.24 semantics at the seam).
#[test]
fn f1b_a_duplicated_publish_under_one_key_is_refused_exactly_one_effective_entry() {
    let h = harness();
    let receipt = StateChangeReceipt::new(
        COMMAND_ENTITY.to_string(),
        StateChangeOperation::Insert,
        Some(tenant_context().tenant_id()),
    );
    let key = "dispatch_proposed:p431-dup-1";
    let write = OutboxWrite::new(
        receipt,
        key.to_string(),
        "flexcore.v1.dispatch".to_string(),
        r#"{"event":"DispatchProposed"}"#.to_string(),
        EventTime::from_unix_millis(1_000).expect("valid"),
    )
    .expect("the outbox write is well-formed");

    // The FIRST publish under the key is accepted...
    h.outbox.append(write.clone()).expect("first publish");
    // ...the DUPLICATED publish (at-least-once redelivery or a replay) is
    // refused with the named error.
    assert_eq!(
        h.outbox.append(write),
        Err(EventOutboxError::DuplicateKey {
            idempotency_key: key.to_string()
        })
    );
    // Exactly ONE effective entry under the key.
    assert_eq!(
        h.outbox
            .pending(100)
            .expect("pending read")
            .iter()
            .filter(|entry| entry.idempotency_key() == key)
            .count(),
        1
    );
}

// ============================================================================
// F2 — reordered delivery (receive-point transport)
// ============================================================================

/// PRE-REGISTERED (F2): sequence 3 for an asset is delivered BEFORE
/// sequence 2. The receive point REFUSES the out-of-order envelope as an
/// application-layer NACK (`out_of_order_sequence`) — the inner transport
/// NEVER sees it, and the guard NEVER advances on a refusal (a rejected
/// sequence cannot move the window). The re-presented older envelope is
/// refused again; the next higher sequence is admitted.
#[test]
fn f2_a_reordered_delivery_is_refused_at_the_receive_point_and_the_guard_never_advances() {
    let inner = FaultTransport::new();
    let keyring = SimulationKeyring::new("p431-recv-key").expect("keyring builds");
    let receiving = VerifyingDispatchTransport::new(&inner, &PayloadBoundVerifier::new(&keyring));

    let make_envelope = |command_tag: u8, sequence: u64| {
        let envelope = SimulatorDispatchEnvelope {
            command_id: CommandId::new(uuid(command_tag)),
            asset_id: asset_id(4),
            sequence,
            target: SimulatorTargetValue::ActivePowerMW(1.5),
            signature_reference: String::new(),
            transport_identity: "sim://dispatch-adapter/p431-recv-key".to_string(),
        };
        // The signature is payload-bound over the envelope's own fields.
        let binding = DispatchPayloadBinding::from_envelope(&envelope);
        let signature = sign_dispatch_payload(&keyring, &binding);
        SimulatorDispatchEnvelope {
            signature_reference: signature,
            ..envelope
        }
    };

    let newer = make_envelope(0x52, 3);
    let older = make_envelope(0x51, 2);

    // FAULT: the NEWER envelope arrives first — the guard admits the
    // first sequence (any >= 1).
    assert!(matches!(
        receiving.deliver(&newer).expect("first delivery"),
        SimulatedAcceptance::Accepted { .. }
    ));
    assert_eq!(receiving.last_accepted_sequence(&asset_id(4)), Some(3));

    // FAULT ASSERTION: the OLDER envelope is REFUSED as out of order.
    match receiving.deliver(&older).expect("the refusal is a NACK value") {
        SimulatedAcceptance::Nack { reason } => {
            assert!(
                reason.contains("out_of_order_sequence"),
                "the refusal names the out-of-order case: {reason}"
            );
        }
        other => panic!("the reordered envelope must be NACKed, got {other:?}"),
    }
    // The refused envelope NEVER reached the inner transport.
    assert!(
        !inner
            .delivered()
            .iter()
            .any(|envelope| envelope.command_id == CommandId::new(uuid(0x51))),
        "a refused envelope never reaches the simulator"
    );
    // The guard did NOT advance on the refusal.
    assert_eq!(
        receiving.last_accepted_sequence(&asset_id(4)),
        Some(3),
        "a refusal never moves the window (no attacker-driven advance)"
    );

    // Re-presenting the older envelope is refused again; the next higher
    // sequence is admitted.
    assert!(matches!(
        receiving.deliver(&older).expect("second refusal"),
        SimulatedAcceptance::Nack { .. }
    ));
    let newer_still = make_envelope(0x53, 4);
    assert!(matches!(
        receiving.deliver(&newer_still).expect("seq 4 admitted"),
        SimulatedAcceptance::Accepted { .. }
    ));
    assert_eq!(receiving.last_accepted_sequence(&asset_id(4)), Some(4));
}

// ============================================================================
// F4 — clock skew forward past validity, at issue (controlled clock)
// ============================================================================

/// PRE-REGISTERED (F4): the clock jumps forward past the validity window
/// BEFORE issue. The adapter REFUSES the issue (`ExpiredCommandRefused`)
/// — expiry belongs to the sweep, never to the adapter — and the sweep
/// EXPIRES the command with the milestone not reached (`BeforeIssue`)
/// recorded. The command is never issued, acknowledged, or completed.
#[test]
fn f4_clock_skew_forward_past_validity_refuses_the_issue_and_the_sweep_expires() {
    let h = harness();
    let (command, authorization) = walk_to_approved(&h, 0x61, 4, 1);
    let command_id = command.command_id();

    // FAULT: the controlled clock jumps forward past valid-until.
    h.clock.advance_to(100_000);
    assert!(h.clock.now_unix_millis() >= VALID_UNTIL_MILLIS);

    // The adapter REFUSES the issue (never issues an expired command).
    let dispatch = adapter(&h);
    let error = dispatch
        .issue(&tenant_context(), &command, &authorization, &actor("svc:dispatch"))
        .expect_err("an expired command must be refused at issue");
    assert!(
        matches!(error, DispatchAdapterError::ExpiredCommandRefused { .. }),
        "the refusal names the expiry: {error}"
    );
    assert_eq!(h.store.state_of(command_id), CommandState::Approved);
    assert!(
        !event_names(&h).contains(&"DispatchIssued"),
        "nothing was issued"
    );

    // The sweep EXPIRES the command with the milestone not reached
    // recorded (`BeforeIssue` — APPROVED expired before the issue).
    let candidate = ExpiryCandidate {
        command_id,
        state: h.store.state_of(command_id),
        valid_until_millis: VALID_UNTIL_MILLIS,
    };
    let decisions = sweep_validity_windows(&[candidate], h.clock.now_unix_millis());
    assert_eq!(decisions.len(), 1, "the expired command is swept");
    assert_eq!(decisions[0].milestone, ExpiryMilestone::BeforeIssue);
    let outcome = h
        .expiry
        .apply_expiry(&tenant_context(), scope(4), command_id, &actor("svc:expiry"))
        .expect("the expiry applies");
    assert_eq!(outcome.milestone, ExpiryMilestone::BeforeIssue);
    assert_eq!(h.store.state_of(command_id), CommandState::Expired);
    assert!(event_names(&h).contains(&"DispatchExpired"));
}

// ============================================================================
// F5 — clock skew forward during tracking (controlled clock)
// ============================================================================

/// PRE-REGISTERED (F5): the clock jumps forward past the validity window
/// DURING tracking. The poll EXPIRES the command (`BeforeCompletion`) —
/// even with an in-tolerance observation scripted — expiry wins over
/// completion; a stale command is never completed.
#[test]
fn f5_clock_skew_during_tracking_expires_the_command_never_completes_it() {
    let h = harness();
    let (command, authorization) = walk_to_approved(&h, 0x71, 4, 1);
    let command_id = command.command_id();

    let dispatch = adapter(&h);
    let outcome = dispatch
        .issue(&tenant_context(), &command, &authorization, &actor("svc:dispatch"))
        .expect("the issue succeeds");
    assert!(outcome.failed.is_none(), "the fixture issue must not fail");
    assert_eq!(h.store.state_of(command_id), CommandState::Acknowledged);

    // Enter TRACKING: one in-tolerance sample (below the criterion's two
    // satisfied samples, so the poll keeps tracking).
    h.transport.script_observations(&[Some(observation(1.55, 10_000))]);
    let poll = dispatch
        .poll_tracking(&tenant_context(), &command, &actor("svc:tracking"))
        .expect("the poll succeeds");
    assert!(poll.terminal.is_none(), "the first poll keeps tracking");
    assert_eq!(h.store.state_of(command_id), CommandState::Tracking);

    // FAULT: the clock jumps forward past the validity window.
    h.clock.advance_to(100_000);

    // Another in-tolerance sample is scripted — and IRRELEVANT: the
    // expiry check precedes the observation pull.
    h.transport.script_observations(&[Some(observation(1.52, 100_000))]);
    let poll = dispatch
        .poll_tracking(&tenant_context(), &command, &actor("svc:tracking"))
        .expect("the poll expires");
    let Some(TrackingTerminal::Expired { transition }) = poll.terminal else {
        panic!(
            "the skewed poll must expire the command, got {:?}",
            poll.terminal
        );
    };
    assert_eq!(transition.to, CommandState::Expired);
    assert_eq!(h.store.state_of(command_id), CommandState::Expired);
    assert!(
        !event_names(&h).contains(&"DispatchCompleted"),
        "a stale command is never completed"
    );
}

// ============================================================================
// F6 — stale telemetry at the feasibility check (runner seam)
// ============================================================================

/// PRE-REGISTERED (F6): the telemetry feed behind the feasibility check
/// goes STALE. The §12.5 pipeline reports the FailClosed verdict (§17.5
/// invariant 9, the P4.08 fail-closed rule) and the command is REJECTED
/// with the stale-telemetry trigger NAMED — never a pass.
#[test]
fn f6_stale_telemetry_at_the_feasibility_check_fails_closed_with_the_trigger_named() {
    let h = harness();
    h.store.seed_revision_active(2);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(0x81, 4, 1))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed);

    // FAULT: the runner seam reports the stale-telemetry fail-closed
    // verdict (the scripted shape of the P4.08 trigger).
    h.fault_runner().script(Ok(FeasibilityGateVerdict::FailClosed {
        attempt_id: "attempt-stale".to_string(),
        model_id: Some("feeder-a".to_string()),
        model_content_hash: Some(MODEL_HASH.to_string()),
        reason: "telemetry:STALE:snapshot_age_95000ms_exceeds_max_60000ms".to_string(),
    }));
    let result = h
        .gates
        .run_feasibility_gate(
            &tenant_context(),
            &proposed.command,
            &feasibility_refs(),
            &actor("svc:grid-gate"),
        )
        .expect("a fail-closed verdict is a verdict, not an error");

    // PRE-REGISTERED OUTCOME: REJECTED, the trigger named, no feasibility
    // evidence, never a pass.
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("fail_closed:telemetry:STALE:snapshot_age_95000ms_exceeds_max_60000ms")
    );
    assert!(result.feasibility_evidence.is_none());
    assert_eq!(
        h.store.state_of(proposed.command.command_id()),
        CommandState::Rejected
    );
    let events = event_names(&h);
    assert!(events.contains(&"DispatchRejected"));
    assert!(
        events.contains(&"FeasibilityCheckCompleted"),
        "the check ran and completed (fail-closed is a verdict) — the completion fact is emitted"
    );
}

// ============================================================================
// F9a/F9b — solver timeout (runner seam)
// ============================================================================

/// PRE-REGISTERED (F9a): the solver times out (the runner seam FAILS).
/// The named `Seam` error surfaces with NO verdict, NO transition, NO
/// event — the command HELD in VALIDATED. A timeout is never "no
/// violation found" (the P4.08 risk note).
#[test]
fn f9a_solver_timeout_seam_error_holds_the_command_with_no_transition_and_no_event() {
    let h = harness();
    h.store.seed_revision_active(2);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(0x91, 4, 1))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed);
    let journal_before = h.store.journal_of(proposed.command.command_id()).len();
    let events_before = event_names(&h).len();

    // FAULT: the solver times out — the runner seam FAILS.
    h.fault_runner().script(Err(GateSeamError {
        seam: "feasibility_runner",
        detail: "solver timeout: the power-flow worker exceeded its 30000 ms budget".to_string(),
    }));

    // PRE-REGISTERED OUTCOME: the named error, NO verdict, NO transition,
    // NO event — the command is HELD in VALIDATED.
    let error = h
        .gates
        .run_feasibility_gate(
            &tenant_context(),
            &proposed.command,
            &feasibility_refs(),
            &actor("svc:grid-gate"),
        )
        .expect_err("a seam failure is an error, never a verdict");
    assert!(
        matches!(&error, CommandGateError::Seam(seam) if seam.seam == "feasibility_runner"),
        "the error names the failed seam: {error}"
    );
    assert!(error.to_string().contains("solver timeout"));
    assert_eq!(
        h.store.state_of(proposed.command.command_id()),
        CommandState::Validated,
        "the command is HELD, not passed and not rejected"
    );
    assert_eq!(
        h.store.journal_of(proposed.command.command_id()).len(),
        journal_before,
        "no transition was recorded"
    );
    assert_eq!(event_names(&h).len(), events_before, "no event was emitted");
}

/// PRE-REGISTERED (F9b): the timeout is reported as a fail-closed VERDICT
/// (the P4.08 policy table's reject branch): the command is REJECTED with
/// `fail_closed:validator_timeout...` named.
#[test]
fn f9b_solver_timeout_reported_as_a_verdict_rejects_with_the_reason_named() {
    let h = harness();
    h.store.seed_revision_active(2);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(0x92, 4, 1))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed);

    // FAULT: the timeout arrives as the pipeline's fail-closed verdict.
    h.fault_runner().script(Ok(FeasibilityGateVerdict::FailClosed {
        attempt_id: "attempt-timeout".to_string(),
        model_id: None,
        model_content_hash: None,
        reason: "validator_timeout:pandapower_worker_30000ms".to_string(),
    }));
    let result = h
        .gates
        .run_feasibility_gate(
            &tenant_context(),
            &proposed.command,
            &feasibility_refs(),
            &actor("svc:grid-gate"),
        )
        .expect("a fail-closed verdict is a verdict, not an error");
    assert!(!result.result.passed);
    assert_eq!(
        result.result.failing_check.as_deref(),
        Some("fail_closed:validator_timeout:pandapower_worker_30000ms")
    );
    assert_eq!(
        h.store.state_of(proposed.command.command_id()),
        CommandState::Rejected
    );
}

// ============================================================================
// F7 — missing telemetry in tracking (simulator transport)
// ============================================================================

/// PRE-REGISTERED (F7): the telemetry feed goes silent (no observation).
/// The poll is the NAMED `NoTelemetryObserved` error with NO transition
/// (the command HELD in ACKNOWLEDGED); the sweep later EXPIRES the
/// command (`BeforeTrackingCompletion`). A silent feed is never read as
/// completion.
#[test]
fn f7_missing_telemetry_holds_the_command_and_the_sweep_expires_it() {
    let h = harness();
    let (command, authorization) = walk_to_approved(&h, 0x93, 4, 1);
    let command_id = command.command_id();
    let dispatch = adapter(&h);
    let outcome = dispatch
        .issue(&tenant_context(), &command, &authorization, &actor("svc:dispatch"))
        .expect("the issue succeeds");
    assert!(outcome.failed.is_none());
    assert_eq!(h.store.state_of(command_id), CommandState::Acknowledged);

    // FAULT: the observation queue is EMPTY — no telemetry at all.
    let error = dispatch
        .poll_tracking(&tenant_context(), &command, &actor("svc:tracking"))
        .expect_err("entering tracking without telemetry is a named error");
    assert!(
        matches!(error, DispatchAdapterError::NoTelemetryObserved { .. }),
        "the error names the missing telemetry: {error}"
    );
    assert_eq!(
        h.store.state_of(command_id),
        CommandState::Acknowledged,
        "the command is HELD — no transition fired"
    );

    // The validity window ends while the feed is silent: the sweep
    // EXPIRES the command (BeforeTrackingCompletion).
    h.clock.advance_to(100_000);
    let candidate = ExpiryCandidate {
        command_id,
        state: h.store.state_of(command_id),
        valid_until_millis: VALID_UNTIL_MILLIS,
    };
    let decisions = sweep_validity_windows(&[candidate], h.clock.now_unix_millis());
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].milestone,
        ExpiryMilestone::BeforeTrackingCompletion
    );
    let outcome = h
        .expiry
        .apply_expiry(&tenant_context(), scope(4), command_id, &actor("svc:expiry"))
        .expect("the expiry applies");
    assert_eq!(outcome.milestone, ExpiryMilestone::BeforeTrackingCompletion);
    assert_eq!(h.store.state_of(command_id), CommandState::Expired);
    assert!(
        !event_names(&h).contains(&"DispatchCompleted"),
        "a silent feed is never read as completion"
    );
}

// ============================================================================
// F8 — unavailable asset at issue (transport down → fallback)
// ============================================================================

/// PRE-REGISTERED (F8): the target asset is UNAVAILABLE (the transport
/// cannot reach the simulator/site gateway). The issue records ISSUED →
/// FAILED with the frozen `TransportFailure` trigger, the site fallback
/// policy ACTIVATES FALLBACK (the frozen edge, audit record, event), the
/// bounded hold ESCALATES when its interval passes (never indefinite —
/// an audit-record escalation, NOT a transition), and recovery requires a
/// FRESH command through the full gate chain (no FALLBACK → ISSUED).
#[test]
fn f8_an_unavailable_asset_fails_the_issue_and_the_fallback_policy_activates_and_escalates() {
    let h = harness();
    let (command, authorization) = walk_to_approved(&h, 0x94, 4, 1);
    let command_id = command.command_id();

    // FAULT: the asset is unavailable — the transport cannot reach it.
    h.transport.script_deliver_for_asset(
        asset_id(4),
        Err(SimulatorTransportError {
            failure: SimulatorTransportFailure::Unreachable,
            detail: "site gateway unreachable for asset p431-4".to_string(),
        }),
    );
    let dispatch = adapter(&h);
    let outcome = dispatch
        .issue(&tenant_context(), &command, &authorization, &actor("svc:dispatch"))
        .expect("a transport failure is a recorded outcome, not an error");

    // PRE-REGISTERED OUTCOME 1: ISSUED → FAILED with the frozen trigger.
    assert_eq!(
        outcome.acknowledgement.state,
        AssetAcknowledgementState::TransportFailed
    );
    let failed = outcome.failed.expect("the FAILED transition is recorded");
    assert_eq!(failed.to, CommandState::Failed);
    assert_eq!(
        failed.reason.as_deref(),
        Some("transport failure"),
        "the journal names the frozen TransportFailure trigger"
    );
    assert_eq!(h.store.state_of(command_id), CommandState::Failed);
    assert!(event_names(&h).contains(&"DispatchFailed"));

    // PRE-REGISTERED OUTCOME 2: the site fallback policy ACTIVATES
    // FALLBACK (frozen edge 26, audit record, `DispatchFallbackActivated`).
    let decision = h
        .fallback
        .activate_fallback(
            &tenant_context(),
            FallbackActivationInput {
                command_id,
                portfolio_id: PortfolioId::new(uuid(2)),
                site_id: SiteId::new(uuid(3)),
                asset_id: asset_id(4),
                trigger: FallbackTrigger::OnFailure,
                simulated_operator_review: simulated_review(),
            },
        )
        .expect("the fallback evaluates");
    let FallbackDecision::Activated { record, transition } = decision else {
        panic!("the site policy requires FALLBACK on failure");
    };
    assert_eq!(record.policy_name, "hold-then-return");
    assert_eq!(record.bounded_hold_millis, Some(60_000));
    assert_eq!(
        record.validity_conditions.as_ref().expect("hold conditions").len(),
        3,
        "the hold states its validity conditions (§14.4)"
    );
    assert_eq!(transition.to, CommandState::Fallback);
    assert_eq!(h.store.state_of(command_id), CommandState::Fallback);
    assert!(event_names(&h).contains(&"DispatchFallbackActivated"));
    assert_eq!(h.audit.activation_count(), 1);

    // The bounded hold is ACTIVE inside its interval...
    let hold = h
        .fallback
        .evaluate_hold_expiration(&tenant_context(), &record)
        .expect("the hold evaluates");
    assert!(matches!(hold, HoldExpiryEvaluation::HoldActive { .. }));

    // ...and ESCALATES when the interval passes (never indefinite). The
    // escalation is an AUDIT record, NOT a command transition (FALLBACK
    // is terminal in the frozen table).
    h.clock.advance_to(5_000 + 61_000);
    let hold = h
        .fallback
        .evaluate_hold_expiration(&tenant_context(), &record)
        .expect("the hold evaluates");
    let HoldExpiryEvaluation::Escalated {
        target,
        record: escalation,
    } = hold
    else {
        panic!("the expired hold must escalate");
    };
    assert_eq!(target, EscalationTarget::ReturnToLocalEms);
    assert_eq!(h.audit.escalation_count(), 1);
    assert_eq!(
        h.store.state_of(command_id),
        CommandState::Fallback,
        "the escalation is an audit record, not a transition"
    );

    // PRE-REGISTERED OUTCOME 3: recovery requires a FRESH command — a
    // direct issue of the FALLBACK command is refused by state.
    let refused =
        dispatch.issue(&tenant_context(), &command, &authorization, &actor("svc:dispatch"));
    assert!(
        matches!(
            refused,
            Err(DispatchAdapterError::WrongState {
                current: CommandState::Fallback,
                operation: "issue"
            })
        ),
        "FALLBACK → ISSUED is refused: {refused:?}"
    );
}

// ============================================================================
// F10 — plugin crash (panicking proposal source; NO plugin kernel exists)
// ============================================================================

/// PRE-REGISTERED (F10): the proposal source CRASHES (a panicking seam —
/// the plugin kernel does not exist yet, so the crash is modelled at the
/// seam). The panic PROPAGATES: no partial transition, no event, nothing
/// issued — fail-closed by construction. Containment (catch + supervisor)
/// is the composition root's recorded duty, NOT silently swallowed here.
#[test]
fn f10_a_panicking_proposal_source_propagates_with_no_partial_transition_and_nothing_issued() {
    let h = harness_with_panicking_runner();
    h.store.seed_revision_active(2);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(0x95, 4, 1))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed);
    let journal_before = h.store.journal_of(proposed.command.command_id()).len();
    let events_before = event_names(&h).len();

    // FAULT: the proposal source crashes mid-gate.
    let result = catch_unwind(AssertUnwindSafe(|| {
        h.gates.run_feasibility_gate(
            &tenant_context(),
            &proposed.command,
            &feasibility_refs(),
            &actor("svc:grid-gate"),
        )
    }));
    assert!(result.is_err(), "the panic must propagate — never swallowed");

    // PRE-REGISTERED OUTCOME: no partial transition, no event, nothing
    // issued. The command is HELD in VALIDATED with its journal intact.
    assert_eq!(
        h.store.state_of(proposed.command.command_id()),
        CommandState::Validated
    );
    assert_eq!(
        h.store.journal_of(proposed.command.command_id()).len(),
        journal_before,
        "the crash recorded no partial transition"
    );
    assert_eq!(event_names(&h).len(), events_before, "the crash emitted no event");
    assert_ne!(
        h.store.state_of(proposed.command.command_id()),
        CommandState::Approved,
        "nothing reached APPROVED, so nothing can be issued"
    );
}

// ============================================================================
// F11 — partial fleet response
// ============================================================================

/// PRE-REGISTERED (F11): a three-asset fleet issue where asset A accepts,
/// asset B NACKs, and asset C times out. The acknowledgement is PER ASSET
/// (Accepted / Refused / TimedOut) — the fleet state is PartiallyAccepted
/// and is NEVER inferred from the first responding device (§14.3) — and
/// the fallback fires ONLY for the two failed assets, never for the
/// accepted one.
#[test]
fn f11_a_partial_fleet_response_is_per_asset_and_never_portfolio_inferred() {
    let h = harness();
    let (command_a, auth_a) = walk_to_approved(&h, 0xA1, 4, 1);
    let (command_b, auth_b) = walk_to_approved(&h, 0xA2, 5, 1);
    let (command_c, auth_c) = walk_to_approved(&h, 0xA3, 6, 1);

    // FAULT: asset B NACKs, asset C is unreachable (timeout).
    h.transport.script_deliver_for_asset(
        asset_id(5),
        Ok(SimulatedAcceptance::Nack {
            reason: "asset busy: local override active".to_string(),
        }),
    );
    h.transport.script_deliver_for_asset(
        asset_id(6),
        Err(SimulatorTransportError {
            failure: SimulatorTransportFailure::Timeout,
            detail: "no acknowledgement within the window".to_string(),
        }),
    );

    let dispatch = adapter(&h);
    let outcome_a = dispatch
        .issue(&tenant_context(), &command_a, &auth_a, &actor("svc:dispatch"))
        .expect("asset A accepts");
    let outcome_b = dispatch
        .issue(&tenant_context(), &command_b, &auth_b, &actor("svc:dispatch"))
        .expect("a NACK is a recorded outcome");
    let outcome_c = dispatch
        .issue(&tenant_context(), &command_c, &auth_c, &actor("svc:dispatch"))
        .expect("a timeout is a recorded outcome");

    // PRE-REGISTERED OUTCOME 1: the PER-ASSET acknowledgement.
    assert_eq!(outcome_a.acknowledgement.state, AssetAcknowledgementState::Accepted);
    assert_eq!(outcome_b.acknowledgement.state, AssetAcknowledgementState::Refused);
    assert_eq!(outcome_c.acknowledgement.state, AssetAcknowledgementState::TimedOut);

    // The fleet acknowledgement is computed from EVERY per-asset state —
    // PartiallyAccepted, never inferred from the first responder.
    let fleet = FleetAcknowledgement::new(vec![
        outcome_a.acknowledgement,
        outcome_b.acknowledgement,
        outcome_c.acknowledgement,
    ])
    .expect("the fleet acknowledgement builds");
    assert_eq!(fleet.total(), 3);
    assert_eq!(fleet.accepted_count(), 1);
    assert!(!fleet.fully_accepted());
    assert_eq!(
        fleet.portfolio_state(),
        FleetAcknowledgementState::PartiallyAccepted {
            accepted: 1,
            assets: 3
        }
    );

    // Per-asset lifecycle states: A acknowledged; B and C FAILED.
    assert_eq!(h.store.state_of(command_a.command_id()), CommandState::Acknowledged);
    assert_eq!(h.store.state_of(command_b.command_id()), CommandState::Failed);
    assert_eq!(h.store.state_of(command_c.command_id()), CommandState::Failed);

    // PRE-REGISTERED OUTCOME 2: the fallback fires ONLY for the failed
    // assets (the site policy activates on failure), never for the
    // accepted one.
    for (command, asset_tag) in [(&command_b, 5u8), (&command_c, 6u8)] {
        let decision = h
            .fallback
            .activate_fallback(
                &tenant_context(),
                FallbackActivationInput {
                    command_id: command.command_id(),
                    portfolio_id: PortfolioId::new(uuid(2)),
                    site_id: SiteId::new(uuid(3)),
                    asset_id: asset_id(asset_tag),
                    trigger: FallbackTrigger::OnFailure,
                    simulated_operator_review: simulated_review(),
                },
            )
            .expect("the fallback evaluates");
        assert!(
            matches!(decision, FallbackDecision::Activated { .. }),
            "the site policy activates FALLBACK for the failed asset {asset_tag}"
        );
    }
    assert_eq!(h.audit.activation_count(), 2, "exactly the failed assets");
    assert_eq!(
        h.store.state_of(command_a.command_id()),
        CommandState::Acknowledged,
        "the accepted asset is untouched by the others' fallback"
    );
}
