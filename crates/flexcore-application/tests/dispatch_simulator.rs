// P4.16 — Simulation-only dispatch adapter tests (report §14.1–§14.2,
//         §19.1, §23 Stage 4, §24 steps 6–8; ledger P4.16 acceptance
//         criteria; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         14_16_lifecycle_completion.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Full lifecycle PROPOSED → … → COMPLETED runs end-to-end against the
//   simulator (the real gate chain P4.11–P4.13, then the adapter's issue +
//   acknowledgement, then the tracking poll to completion).
// - Acknowledgement is application-layer ACCEPTANCE, never physical
//   delivery: the ISSUED → ACKNOWLEDGED evidence references the simulator's
//   acceptance record, and the acknowledgement vocabulary has no
//   "delivered" state.
// - NACK / transport timeout at issue → ISSUED → FAILED with the frozen
//   trigger; tracking deviation and tracking timeout → TRACKING → FAILED.
// - Expiry: an APPROVED command past its validity window is REFUSED by the
//   adapter (never issued) and moved to EXPIRED by the P4.15 sweep with the
//   milestone not reached recorded; validity ending during tracking moves
//   the command to EXPIRED at the poll (the check-at-transition path).
// - Replacement during tracking → SUPERSEDED through the P4.15 service,
//   and the adapter then refuses the superseded command by state.
// - Target guard: every non-simulator target candidate is refused at
//   `configure`; the transport port has exactly `deliver` + `observe` and
//   the source carries no device-write path (structural test).
// - Guard chain: unknown/unusable assets are rejected at the validation
//   gate (before any issue is possible); a cross-tenant caller cannot see
//   the command at all (existence never leaks across a tenant boundary).
// - Partial fleet failure is per asset: the fleet acknowledgement is
//   computed from every per-asset state, never inferred from the first
//   responding device.
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction; the
// transport fake scripts the §24 step 8 semantics (acceptance, NACK,
// timeout, tracking samples, deviation).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_expiry::{CommandExpiryService, EventScope, ExpiryOutcome};
use flexcore_application::command_gates::{
    AppliedSeparationOfDuties, ApprovalScope, AssetCapabilitySource, AssetGateStatus,
    AuthorizationDecision, AuthorizationDecisionRecord, CommandAuthorizationEvaluator,
    CommandAuthorizationRequest, CommandAuthzAction, CommandFeasibilityRefs,
    CommandFeasibilityRunner, CommandGatePolicy, CommandGateStore, CommandGateStoreError,
    CommandGateView, CommandGates, DeclaredQuantities, EnvelopeGateFreshness,
    FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus, ModelVersionSource,
    ProposedCommandRecord, ProposeCommandInput, ScheduleEnvelopeSource, ScheduleRevisionUsability,
    COMMAND_ENTITY,
};
use flexcore_application::command_record::{
    AssetAcknowledgementState, FleetAcknowledgement, FleetAcknowledgementState,
};
use flexcore_application::dispatch_simulator::{
    check_issue_refusals, evaluate_tracking, AdapterTargetCandidate, DispatchAdapterError,
    IssueRefusal, NonSimulatorTargetKind, SimulatedAcceptance, SimulatedObservation,
    SimulatorDispatchAdapter, SimulatorDispatchEnvelope, SimulatorDispatchTransport,
    SimulatorTargetValue, SimulatorTransportError, SimulatorTransportFailure, SimulationKeyring,
    TrackingAcceptanceCriterion, TrackingTerminal, TrackingVerdict, TrackedQuantity,
    SIMULATION_ONLY_STATEMENT, SIMULATOR_TRANSPORT_IDENTITY_PREFIX,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, FailureReason, LifecycleClock,
};
use flexcore_domain::command_state::CommandState;
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

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
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

fn other_tenant_context() -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(uuid(15)),
        subject: "test:other-principal".to_string(),
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
    Actor::new(name).expect("a valid actor")
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-dispatcher";
const DISPATCHER: &str = "svc:dispatcher";
const MODEL_HASH: &str = "fnv1a64:sim-feeder-hash-01";
const SIMULATOR_ID: &str = "sim:phase4-vertical-slice";
const SIM_KEY_ID: &str = "sim-key-1";

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

/// Renders the event's Debug form (which names the frozen variant) — enough
/// to assert WHICH event was emitted without a serialization library.
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(&self, envelope: &flexcore_events::envelope::EventEnvelope) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- Store fake (tenant-scoped; existence never leaks) -------------------------------

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

    fn seed_revision(&self, schedule_id: &str, revision: u64, state: ScheduleRevisionUsability) {
        self.revisions
            .lock()
            .expect("fake store")
            .insert((schedule_id.to_string(), revision), state);
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
        ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<Option<CommandGateView>, CommandGateStoreError> {
        let records = self.records.lock().expect("fake store");
        let Some(record) = records.get(&command_id.to_string()) else {
            return Ok(None);
        };
        // Tenant scoping: a command of another tenant does not exist here
        // (P0.16 rule 5 — existence never leaks across a tenant boundary).
        if record.tenant_id != ctx.tenant_id() {
            return Ok(None);
        }
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

// --- Seam fakes ---------------------------------------------------------------------

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

struct FakeModels {
    status: Mutex<ModelVersionGateStatus>,
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
            model_id: "sim-feeder".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.98; max loading 63%".to_string(),
            coverage: "balanced radial sim-feeder; unbalanced deferred (P4.34)".to_string(),
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
                rule_reference: "policy:phase4:tenant-dispatcher:command.approve".to_string(),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
    }
}

// --- The scripted simulator transport fake --------------------------------------------

/// Scripts the §24 step 8 simulation semantics: the deliver queue holds the
/// acceptance outcome per issue (default: application-layer ACCEPTED with a
/// synthesized acceptance record); the observation queue holds the tracking
/// samples per poll (default: no pending sample — the poll ends).
struct FakeTransport {
    deliver_results: Mutex<VecDeque<Result<SimulatedAcceptance, SimulatorTransportError>>>,
    observations: Mutex<VecDeque<Option<SimulatedObservation>>>,
    delivered: Mutex<Vec<SimulatorDispatchEnvelope>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            deliver_results: Mutex::new(VecDeque::new()),
            observations: Mutex::new(VecDeque::new()),
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn script_deliver(&self, result: Result<SimulatedAcceptance, SimulatorTransportError>) {
        self.deliver_results
            .lock()
            .expect("fake transport")
            .push_back(result);
    }

    fn script_observations(&self, observations: &[Option<SimulatedObservation>]) {
        let mut queue = self.observations.lock().expect("fake transport");
        for observation in observations {
            queue.push_back(*observation);
        }
    }

    fn delivered(&self) -> Vec<SimulatorDispatchEnvelope> {
        self.delivered.lock().expect("fake transport").clone()
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
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError> {
        self.delivered
            .lock()
            .expect("fake transport")
            .push(envelope.clone());
        self.deliver_results
            .lock()
            .expect("fake transport")
            .pop_front()
            .unwrap_or_else(|| {
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
            .expect("fake transport")
            .pop_front()
            .unwrap_or(None))
    }
}

// --- Harness ------------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    models: &'static FakeModels,
    assets: &'static FakeAssets,
    transport: &'static FakeTransport,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
    encoder: &'static DebugEncoder,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels {
        status: Mutex::new(ModelVersionGateStatus::Active {
            content_hash: MODEL_HASH.to_string(),
        }),
    }));
    let assets: &'static FakeAssets = Box::leak(Box::new(FakeAssets {
        status: Mutex::new(AssetGateStatus::Usable(DeclaredQuantities {
            active_power: true,
            reactive_power: true,
            soc: true,
            max_up_ramp_mw_per_s: 2.0,
            max_down_ramp_mw_per_s: 2.0,
        })),
    }));
    let runner: &'static FakeRunner = Box::leak(Box::new(FakeRunner));
    let authz: &'static FakeAuthz = Box::leak(Box::new(FakeAuthz));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox =
        Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let transport: &'static FakeTransport = Box::leak(Box::new(FakeTransport::new()));
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
        models,
        assets,
        transport,
        outbox,
        clock,
        ids,
        encoder,
    }
}

// --- Command fixture and the gate-chain walk -----------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
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
        idempotency_key: format!("idem:op-{command_tag}"),
        evidence: EvidenceReferences {
            grid_model: "grid-model:sim-feeder@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:sim-fixture".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-1".to_string(),
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
    h.store.seed_revision(&schedule_id_text(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(command_tag, asset_tag, sequence))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed, "fixture validation must pass");
    let feasibility = h
        .gates
        .run_feasibility_gate(&tenant_context(), &proposed.command, &feasibility_refs(), &actor("svc:grid-gate"))
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
        approval.decision_record.expect("an approval carries its decision record"),
    )
}

/// A never-persisted §14.3-valid command (for the not-found refusal).
fn fixture_command(command_tag: u8) -> DispatchCommand {
    let input = propose_input(command_tag, 4, 1);
    DispatchCommand::try_new(
        input.command_id,
        input.scope,
        input.source,
        input.schedule,
        input.intent,
        input.target,
        input.validity,
        input.execution,
        input.sequence,
        input.idempotency_key,
        input.evidence,
        flexcore_domain::dispatch_command::AuthorizationReference {
            decision_reference: "authz-decision:fixture@0".to_string(),
            approved_scope: "tenant".to_string(),
            approved_limits: "limits".to_string(),
            expiry: ValidUntil::from_unix_millis(100_000).unwrap(),
        },
        input.authenticity,
        None,
    )
    .expect("the fixture command is valid")
}

fn fixture_decision_record() -> AuthorizationDecisionRecord {
    AuthorizationDecisionRecord {
        decision_reference: "authz-decision:fixture@0".to_string(),
        authorizing_actor: APPROVER.to_string(),
        authorizing_role: APPROVER_ROLE.to_string(),
        approved_scope: "tenant".to_string(),
        approved_limits: "limits".to_string(),
        decided_at_unix_millis: 0,
        expiry_unix_millis: 100_000,
        separation_of_duties: AppliedSeparationOfDuties::NotConfigured,
        covered_validation_evidence: "validation:fixture".to_string(),
        covered_feasibility_evidence: "feasibility:fixture".to_string(),
    }
}

// --- Adapter and expiry-service builders ----------------------------------------------

fn criterion(min_satisfied_samples: u32) -> TrackingAcceptanceCriterion {
    TrackingAcceptanceCriterion::new(
        TrackedQuantity::ActivePowerMW,
        0.2,
        min_satisfied_samples,
        5_000,
    )
    .expect("a valid acceptance criterion")
}

fn adapter_for(h: &Harness, acceptance: TrackingAcceptanceCriterion) -> SimulatorDispatchAdapter<'_> {
    SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: SIMULATOR_ID.to_string(),
        },
        h.transport,
        SimulationKeyring::new(SIM_KEY_ID).expect("a non-empty simulation key id"),
        acceptance,
    )
    .expect("the simulator target is the one accepted candidate")
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

fn event_names(h: &Harness) -> Vec<String> {
    const CANDIDATES: [&str; 13] = [
        "DispatchProposed",
        "DispatchApproved",
        "DispatchRejected",
        "DispatchIssued",
        "DispatchAcknowledged",
        "DispatchTrackingUpdated",
        "DispatchCompleted",
        "DispatchFailed",
        "DispatchExpired",
        "DispatchCancelled",
        "DispatchSuperseded",
        "DispatchFallbackActivated",
        // P4.30: the §15.3 grid-validation event the feasibility gate
        // emits for every completed §12.5 check (D-P4.30-1).
        "FeasibilityCheckCompleted",
    ];
    h.outbox
        .writes()
        .iter()
        .map(|write| {
            let payload = write.payload();
            for candidate in CANDIDATES {
                if payload.starts_with(candidate) {
                    return candidate.to_string();
                }
            }
            "unknown".to_string()
        })
        .collect()
}

fn dispatch_simulator_source() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/dispatch_simulator.rs");
    std::fs::read_to_string(path).expect("the dispatch adapter source is readable")
}

// ============================================================================
// The hard simulation-only guard (D-P4.16-6)
// ============================================================================

#[test]
fn the_simulation_only_statement_is_recorded() {
    assert!(SIMULATION_ONLY_STATEMENT.contains("SIMULATOR ONLY"));
    assert!(SIMULATION_ONLY_STATEMENT.contains("no device write path"));
    assert!(SIMULATION_ONLY_STATEMENT.contains("Stage 6"));
    assert!(SIMULATION_ONLY_STATEMENT.contains("Stage 9"));
    assert!(SIMULATOR_TRANSPORT_IDENTITY_PREFIX.starts_with("sim://"));
}

#[test]
fn the_transport_port_has_exactly_deliver_and_observe_and_no_write_path() {
    let source = dispatch_simulator_source();
    // The port is declared and carries EXACTLY two methods: deliver + observe.
    let marker = "pub trait SimulatorDispatchTransport";
    let start = source.find(marker).expect("the transport port is declared");
    let end = start + source[start..].find('}').expect("the trait block closes");
    let block = &source[start..end];
    assert_eq!(
        block.matches("fn ").count(),
        2,
        "the transport port has exactly two methods (deliver + observe); a third method — \
         especially any write — must not be added"
    );
    assert!(block.contains("fn deliver("), "the issue method is `deliver`");
    assert!(block.contains("fn observe("), "the tracking method is `observe`");
    // No device-write path anywhere in the adapter source: no write-named
    // function, no write-method call, no protocol write primitive.
    for forbidden in ["fn write", ".write(", "write_coil", "write_register", "write_holding"] {
        assert!(
            !source.contains(forbidden),
            "forbidden write-path identifier '{forbidden}' found in dispatch_simulator.rs"
        );
    }
}

#[test]
fn the_target_guard_refuses_every_non_simulator_kind() {
    let h = harness(5_000);
    for kind in [
        NonSimulatorTargetKind::Gateway,
        NonSimulatorTargetKind::ProtocolAdapter,
        NonSimulatorTargetKind::Device,
        NonSimulatorTargetKind::FieldEndpoint,
    ] {
        let endpoint = format!("tcp://field-endpoint/{}", kind.label());
        let error = SimulatorDispatchAdapter::configure(
            &h.gates,
            AdapterTargetCandidate::NonSimulator {
                kind,
                endpoint: endpoint.clone(),
            },
            h.transport,
            SimulationKeyring::new(SIM_KEY_ID).unwrap(),
            criterion(1),
        )
        .expect_err("every non-simulator target is refused");
        let text = error.to_string();
        match error {
            DispatchAdapterError::NonSimulatorTargetRefused { kind: label, endpoint: recorded } => {
                assert_eq!(label, kind.label());
                assert_eq!(recorded, endpoint);
            }
            other => panic!("unexpected error {other:?}"),
        }
        assert!(text.contains("simulator only"), "the refusal names the boundary");
        assert!(text.contains("Stage 6"), "field dispatch requires Stage 6");
        assert!(text.contains("Stage 9"), "…and Stage 9 qualification");
    }

    // An empty simulator id is refused too (an unattributable target).
    let error = SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: "   ".to_string(),
        },
        h.transport,
        SimulationKeyring::new(SIM_KEY_ID).unwrap(),
        criterion(1),
    )
    .expect_err("an empty simulator id is refused");
    assert!(matches!(error, DispatchAdapterError::InvalidCriterion { .. }));

    // The accepted target records the simulator identity.
    let adapter = adapter_for(&h, criterion(1));
    assert_eq!(adapter.simulator_id(), SIMULATOR_ID);
}

// ============================================================================
// The issue refusals and the acceptance criterion (pure checks)
// ============================================================================

#[test]
fn issue_refusals_fire_unsigned_unsequenced_and_expired_in_ledger_order() {
    use flexcore_application::dispatch_simulator::IssueRefusalInput;

    let signed = CommandAuthenticity::Signature("sig:sim-fixture".to_string());
    let unsigned = CommandAuthenticity::Signature("   ".to_string());
    let unsigned_transport = CommandAuthenticity::AuthenticatedTransport(String::new());

    // Unsigned fires first (both authenticity kinds).
    let error = check_issue_refusals(&IssueRefusalInput {
        authenticity: &unsigned,
        sequence: 0,
        valid_until_millis: 1_000,
        now_unix_millis: 2_000,
    })
    .expect_err("an unsigned command is refused");
    assert!(matches!(error, IssueRefusal::Unsigned));
    let error = check_issue_refusals(&IssueRefusalInput {
        authenticity: &unsigned_transport,
        sequence: 0,
        valid_until_millis: 1_000,
        now_unix_millis: 2_000,
    })
    .expect_err("an empty transport identity is refused as unsigned");
    assert!(matches!(error, IssueRefusal::Unsigned));

    // Unsequenced fires before the expiry check.
    let error = check_issue_refusals(&IssueRefusalInput {
        authenticity: &signed,
        sequence: 0,
        valid_until_millis: 1_000,
        now_unix_millis: 2_000,
    })
    .expect_err("a zero sequence is refused");
    assert!(matches!(error, IssueRefusal::Unsequenced));

    // Expired (end-exclusive: now >= valid_until).
    let error = check_issue_refusals(&IssueRefusalInput {
        authenticity: &signed,
        sequence: 7,
        valid_until_millis: 1_000,
        now_unix_millis: 1_000,
    })
    .expect_err("the boundary instant is expired");
    assert_eq!(
        error,
        IssueRefusal::Expired {
            valid_until_millis: 1_000,
            now_unix_millis: 1_000,
        }
    );
    // Inside the window, signed and sequenced: no refusal.
    check_issue_refusals(&IssueRefusalInput {
        authenticity: &signed,
        sequence: 7,
        valid_until_millis: 1_000,
        now_unix_millis: 999,
    })
    .expect("a signed, sequenced, unexpired command passes the refusals");
}

#[test]
fn the_acceptance_criterion_is_validated_deployment_configuration() {
    // Tolerance must be finite and strictly positive.
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.0, 1, 1_000).is_err());
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, f64::NAN, 1, 1_000).is_err());
    // At least one satisfied sample is required.
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 0, 1_000).is_err());
    // The tracking timeout must be positive.
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 1, 0).is_err());
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 1, -1).is_err());
    // A valid criterion builds.
    assert!(TrackingAcceptanceCriterion::new(TrackedQuantity::SocFraction, 0.01, 3, 1_000).is_ok());
}

#[test]
fn evaluate_tracking_follows_the_recorded_evaluation_order() {
    let requested = 1.5;
    let started = 1_000;
    let obs = |value: f64| {
        FakeTransport::observation(TrackedQuantity::ActivePowerMW, value, started + 100)
    };

    // (1) Any deviating sample wins: FAILED even when earlier samples
    //     already satisfied the count.
    let single = criterion(1);
    let verdict = evaluate_tracking(&single, requested, &[obs(1.5), obs(1.9)], started, 1_500);
    assert_eq!(
        verdict,
        TrackingVerdict::Deviation { measured: 1.9, requested, tolerance: 0.2 }
    );

    // (2) Enough within-tolerance samples → COMPLETED. The tolerance is
    //     absolute; a delta exactly AT the tolerance is within it.
    let verdict = evaluate_tracking(&single, requested, &[obs(1.7)], started, 1_500);
    assert_eq!(verdict, TrackingVerdict::Completed { satisfied_samples: 1 });

    // (3) Under the count and the timeout elapsed → tracking timeout.
    let double = criterion(2);
    let verdict = evaluate_tracking(&double, requested, &[obs(1.5)], started, 6_000);
    assert_eq!(verdict, TrackingVerdict::Timeout);

    // (4) Under the count, timeout not elapsed → keep tracking.
    let verdict = evaluate_tracking(&double, requested, &[obs(1.5)], started, 3_000);
    assert_eq!(verdict, TrackingVerdict::KeepTracking { satisfied_samples: 1 });
}

// ============================================================================
// The full lifecycle: PROPOSED → … → COMPLETED in simulation
// ============================================================================

#[test]
fn the_full_lifecycle_runs_end_to_end_in_simulation_to_completed() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 10, 4, 17);
    let adapter = adapter_for(&h, criterion(1));

    // ISSUE (APPROVED → ISSUED): signed by the simulation keyring, sequenced,
    // delivered to the SIMULATOR ONLY.
    let outcome = adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("the approved command issues to the simulator");
    let issued = &outcome.issued;
    assert_eq!(issued.from, CommandState::Approved);
    assert_eq!(issued.to, CommandState::Issued);
    let signature_reference = format!("sim-sig:{SIM_KEY_ID}:{}", command.command_id());
    assert_eq!(
        issued.evidence.as_ref().map(|e| e.as_str()),
        Some(signature_reference.as_str()),
        "the ISSUED evidence is the §14.3 signature reference"
    );
    // The per-asset acknowledgement is ACCEPTANCE (never delivery).
    assert_eq!(outcome.acknowledgement.state, AssetAcknowledgementState::Accepted);
    assert_eq!(outcome.acknowledgement.asset_id, command.scope().asset_id);
    let acknowledged = outcome.acknowledged.as_ref().expect("acceptance acknowledges");
    assert_eq!(acknowledged.from, CommandState::Issued);
    assert_eq!(acknowledged.to, CommandState::Acknowledged);
    assert_eq!(
        acknowledged.evidence.as_ref().map(|e| e.as_str()),
        Some(format!("acceptance:sim/{}", command.command_id()).as_str()),
        "the ACKNOWLEDGED evidence references the simulator's ACCEPTANCE record"
    );
    assert!(outcome.failed.is_none());
    // The envelope carried signature, sequence, transport identity, and the
    // requested canonical-unit value.
    let delivered = h.transport.delivered();
    assert_eq!(delivered.len(), 1, "exactly one envelope reached the simulator");
    let envelope = &delivered[0];
    assert_eq!(envelope.command_id, command.command_id());
    assert_eq!(envelope.asset_id, command.scope().asset_id);
    assert_eq!(envelope.sequence, 17);
    assert_eq!(envelope.signature_reference, signature_reference);
    assert!(envelope.transport_identity.starts_with(SIMULATOR_TRANSPORT_IDENTITY_PREFIX));
    assert_eq!(envelope.target, SimulatorTargetValue::ActivePowerMW(1.5));

    // TRACKING (ACKNOWLEDGED → TRACKING → COMPLETED): one within-tolerance
    // observation satisfies the criterion (min 1 sample).
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    let poll = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("the tracking poll advances the command");
    let started = poll.tracking_started.as_ref().expect("the first observation enters tracking");
    assert_eq!(started.from, CommandState::Acknowledged);
    assert_eq!(started.to, CommandState::Tracking);
    assert_eq!(poll.observations, 1);
    assert_eq!(poll.verdict, Some(TrackingVerdict::Completed { satisfied_samples: 1 }));
    let terminal = poll.terminal.expect("the criterion is satisfied");
    match terminal {
        TrackingTerminal::Completed { transition } => {
            assert_eq!(transition.from, CommandState::Tracking);
            assert_eq!(transition.to, CommandState::Completed);
            assert!(transition
                .evidence
                .as_ref()
                .map(|e| e.as_str())
                .unwrap_or_default()
                .contains("criterion-satisfied"));
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    // The store-authoritative state is COMPLETED.
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Completed);
    // The frozen event set: Proposed, the completed-check event (P4.30,
    // D-P4.30-1), Approved, Issued, Acknowledged, TrackingUpdated,
    // Completed — in order. (VALIDATED and FEASIBLE still have
    // deliberately NO §15.3 event; the journal is the evidence of record.)
    assert_eq!(
        event_names(&h),
        vec![
            "DispatchProposed".to_string(),
            "FeasibilityCheckCompleted".to_string(),
            "DispatchApproved".to_string(),
            "DispatchIssued".to_string(),
            "DispatchAcknowledged".to_string(),
            "DispatchTrackingUpdated".to_string(),
            "DispatchCompleted".to_string(),
        ]
    );
}

#[test]
fn acknowledgement_is_application_layer_acceptance_never_physical_delivery() {
    // The module's recorded vocabulary disclaims delivery.
    let source = dispatch_simulator_source();
    assert!(
        source.contains("physical delivery"),
        "the adapter documents the acceptance-vs-delivery boundary"
    );
    // The per-asset acknowledgement states are acceptance vocabulary —
    // there is no "delivered" state (§14.1: acknowledgement is receipt/
    // acceptance, not physical delivery).
    assert_eq!(AssetAcknowledgementState::Accepted.as_str(), "accepted");
    assert_eq!(AssetAcknowledgementState::Refused.as_str(), "refused");
    assert_eq!(AssetAcknowledgementState::TimedOut.as_str(), "timed_out");
    assert_eq!(AssetAcknowledgementState::TransportFailed.as_str(), "transport_failed");
    assert_eq!(AssetAcknowledgementState::AwaitingAcceptance.as_str(), "awaiting_acceptance");

    // End to end: the ISSUED → ACKNOWLEDGED journal row references the
    // simulator's ACCEPTANCE record — and the ISSUED row references the
    // simulation SIGNATURE, not any delivery record.
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 12, 4, 17);
    let adapter = adapter_for(&h, criterion(1));
    let outcome = adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");
    assert_eq!(outcome.acknowledgement.state, AssetAcknowledgementState::Accepted);
    assert!(outcome
        .acknowledgement
        .acceptance_reference
        .as_deref()
        .unwrap_or_default()
        .starts_with("acceptance:"));
    assert!(outcome
        .issued
        .evidence
        .as_ref()
        .map(|e| e.as_str())
        .unwrap_or_default()
        .starts_with("sim-sig:"));
}

// ============================================================================
// Failure paths (NACK, timeout, deviation, timeout at tracking)
// ============================================================================

#[test]
fn nack_at_issue_moves_issued_to_failed_with_the_frozen_trigger() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 13, 4, 17);
    h.transport.script_deliver(Ok(SimulatedAcceptance::Nack {
        reason: "the simulator refused the setpoint (scripted NACK)".to_string(),
    }));
    let adapter = adapter_for(&h, criterion(1));
    let outcome = adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("a NACK is a verdict, not an infrastructure error");
    // The ISSUED transition is real (the envelope was sent); the
    // acknowledgement consequence is FAILED with the frozen NACK trigger.
    assert_eq!(outcome.issued.to, CommandState::Issued);
    let failed = outcome.failed.as_ref().expect("a NACK fails the command");
    assert_eq!(failed.from, CommandState::Issued);
    assert_eq!(failed.to, CommandState::Failed);
    assert_eq!(failed.reason.as_deref(), Some(FailureReason::Nack.reason_text()));
    assert_eq!(outcome.acknowledgement.state, AssetAcknowledgementState::Refused);
    assert!(outcome.acknowledged.is_none());
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Failed);
    let names = event_names(&h);
    assert!(names.contains(&"DispatchFailed".to_string()));
    assert!(
        !names.contains(&"DispatchAcknowledged".to_string()),
        "a NACKed command is never acknowledged"
    );
}

#[test]
fn transport_timeout_at_issue_moves_issued_to_failed_with_the_frozen_trigger() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 14, 4, 17);
    h.transport.script_deliver(Err(SimulatorTransportError {
        failure: SimulatorTransportFailure::Timeout,
        detail: "no acceptance within the acknowledgement window".to_string(),
    }));
    let adapter = adapter_for(&h, criterion(1));
    let outcome = adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("a timeout is a verdict, not an infrastructure error");
    let failed = outcome.failed.as_ref().expect("a timeout fails the command");
    assert_eq!(failed.reason.as_deref(), Some(FailureReason::TransportTimeout.reason_text()));
    assert_eq!(outcome.acknowledgement.state, AssetAcknowledgementState::TimedOut);
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Failed);
}

#[test]
fn tracking_deviation_moves_tracking_to_failed() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 16, 4, 17);
    let adapter = adapter_for(&h, criterion(2)); // needs 2 satisfied samples
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");

    // First poll: one within-tolerance observation — under the count, the
    // timeout not elapsed → keep tracking.
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    let poll = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("first poll");
    assert_eq!(poll.verdict, Some(TrackingVerdict::KeepTracking { satisfied_samples: 1 }));
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Tracking);

    // Second poll: a sample outside the tolerance → deviation → FAILED.
    h.clock.advance_to(7_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        3.0,
        7_000,
    ))]);
    let poll = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("second poll");
    let terminal = poll.terminal.expect("a deviation terminates");
    match terminal {
        TrackingTerminal::Failed { transition, reason } => {
            assert_eq!(reason, FailureReason::TrackingDeviation);
            assert_eq!(transition.from, CommandState::Tracking);
            assert_eq!(transition.to, CommandState::Failed);
            assert_eq!(transition.reason.as_deref(), Some(FailureReason::TrackingDeviation.reason_text()));
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Failed);
    assert!(event_names(&h).contains(&"DispatchFailed".to_string()));
}

#[test]
fn tracking_timeout_moves_tracking_to_failed() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 17, 4, 17);
    let adapter = adapter_for(&h, criterion(2));
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");

    // Enter tracking at 6_000 (the → TRACKING journal timestamp).
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("first poll");

    // At 11_500 the tracking timeout (5_000 since 6_000) has elapsed and
    // the count is still under 2 → tracking timeout → FAILED.
    h.clock.advance_to(11_500);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        11_500,
    ))]);
    let poll = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("second poll");
    match poll.terminal.expect("the timeout terminates") {
        TrackingTerminal::Failed { transition, reason } => {
            assert_eq!(reason, FailureReason::TrackingTimeout);
            assert_eq!(transition.reason.as_deref(), Some(FailureReason::TrackingTimeout.reason_text()));
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Failed);
}

#[test]
fn no_telemetry_at_acknowledged_is_a_named_error_and_no_transition() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 18, 4, 17);
    let adapter = adapter_for(&h, criterion(1));
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");
    // No scripted observations: edge 18 requires an observation.
    let error = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect_err("ACKNOWLEDGED → TRACKING requires telemetry");
    match error {
        DispatchAdapterError::NoTelemetryObserved { command_id } => {
            assert_eq!(command_id, command.command_id().to_string());
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The command is untouched.
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Acknowledged);
}

#[test]
fn an_observation_of_the_wrong_quantity_fails_closed() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 19, 4, 17);
    let adapter = adapter_for(&h, criterion(1));
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::SocFraction, // the command tracks active power
        0.7,
        6_000,
    ))]);
    let error = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect_err("a quantity mismatch is fail-closed, never coerced");
    match error {
        DispatchAdapterError::ObservationMismatch {
            criterion_quantity,
            observation_quantity,
            ..
        } => {
            assert_eq!(criterion_quantity, TrackedQuantity::ActivePowerMW.label());
            assert_eq!(observation_quantity, TrackedQuantity::SocFraction.label());
        }
        other => panic!("unexpected error {other:?}"),
    }
}

// ============================================================================
// Expiry (before issue via the sweep; during tracking at the poll)
// ============================================================================

#[test]
fn expired_approval_cannot_be_issued_and_expires_via_the_sweep() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 20, 4, 17);
    // The validity window (ends 70_000) passes before the issue.
    h.clock.advance_to(80_000);
    let adapter = adapter_for(&h, criterion(1));
    let error = adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect_err("an expired command is refused, never issued");
    match error {
        DispatchAdapterError::ExpiredCommandRefused {
            valid_until_millis,
            now_unix_millis,
            ..
        } => {
            assert_eq!(valid_until_millis, 70_000);
            assert_eq!(now_unix_millis, 80_000);
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The refusal leaves the command APPROVED (the EXPIRED transition is
    // the expiry sweep's — recorded decision D-P4.15-1).
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);
    assert!(!h
        .store
        .journal_of(command.command_id())
        .iter()
        .any(|entry| entry.to == CommandState::Issued));

    // The P4.15 sweep moves it: APPROVED → EXPIRED, milestone BeforeIssue,
    // expiry time recorded.
    let service = expiry_service(&h);
    let outcome: ExpiryOutcome = service
        .apply_expiry(&tenant_context(), scope(4), command.command_id(), &actor("svc:expiry-sweep"))
        .expect("the lapsed APPROVED command expires");
    assert_eq!(outcome.milestone, flexcore_domain::command_lifecycle::ExpiryMilestone::BeforeIssue);
    assert_eq!(outcome.expired_at_unix_millis, 80_000);
    assert_eq!(outcome.transition.from, CommandState::Approved);
    assert_eq!(outcome.transition.to, CommandState::Expired);
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Expired);
    assert!(event_names(&h).contains(&"DispatchExpired".to_string()));
}

#[test]
fn validity_ending_during_tracking_expires_at_the_poll() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 21, 4, 17);
    let adapter = adapter_for(&h, criterion(2));
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");

    // Enter tracking (check-at-transition path: the poll is a transition
    // attempt, and expiry is checked at it).
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("first poll");
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Tracking);

    // The validity window ends while tracking; the next poll expires the
    // command with the TRACKING milestone (BeforeCompletion).
    h.clock.advance_to(75_000);
    let poll = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("the expiry poll");
    match poll.terminal.expect("the lapse terminates") {
        TrackingTerminal::Expired { transition } => {
            assert_eq!(transition.from, CommandState::Tracking);
            assert_eq!(transition.to, CommandState::Expired);
            assert!(transition
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("COMPLETED"),
                "the milestone not reached is recorded");
        }
        other => panic!("unexpected terminal {other:?}"),
    }
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Expired);
    assert!(event_names(&h).contains(&"DispatchExpired".to_string()));
}

// ============================================================================
// Replacement during tracking → SUPERSEDED (via the P4.15 service)
// ============================================================================

#[test]
fn replacement_during_tracking_supersedes_and_the_adapter_refuses_the_superseded_command() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 22, 4, 17);
    let adapter = adapter_for(&h, criterion(2));
    adapter
        .issue(&tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect("issue");
    h.clock.advance_to(6_000);
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        6_000,
    ))]);
    adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect("first poll");

    // A newer approved replacement supersedes the command via the P4.15
    // service (the record is retained; the backward link is the journal
    // evidence `command:<uuid>`).
    let replacement = CommandId::new(uuid(23));
    let service = expiry_service(&h);
    let outcome = service
        .supersede(&tenant_context(), scope(4), command.command_id(), replacement, &actor(APPROVER))
        .expect("a TRACKING command can be superseded");
    assert_eq!(outcome.superseded, command.command_id());
    assert_eq!(outcome.superseded_by, replacement);
    assert_eq!(outcome.transition.to, CommandState::Superseded);
    assert_eq!(
        outcome.transition.evidence.as_ref().map(|e| e.as_str()),
        Some(format!("command:{replacement}").as_str())
    );
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Superseded);
    assert!(event_names(&h).contains(&"DispatchSuperseded".to_string()));

    // The adapter refuses the superseded command by state — no further
    // tracking, no resurrection.
    h.transport.script_observations(&[Some(FakeTransport::observation(
        TrackedQuantity::ActivePowerMW,
        1.5,
        7_000,
    ))]);
    let error = adapter
        .poll_tracking(&tenant_context(), &command, &actor(DISPATCHER))
        .expect_err("a superseded command leaves the tracking path");
    match error {
        DispatchAdapterError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Superseded);
            assert_eq!(operation, "poll_tracking");
        }
        other => panic!("unexpected error {other:?}"),
    }
}

// ============================================================================
// The guard chain: state, tenancy, and asset checks precede any issue
// ============================================================================

#[test]
fn issue_refuses_wrong_state_and_unknown_commands() {
    let h = harness(5_000);
    // A command that was never persisted: NotFound (existence never leaks).
    let phantom = fixture_command(24);
    let adapter = adapter_for(&h, criterion(1));
    let error = adapter
        .issue(&tenant_context(), &phantom, &fixture_decision_record(), &actor(DISPATCHER))
        .expect_err("an unknown command is not issuable");
    assert!(matches!(error, DispatchAdapterError::NotFound { .. }));

    // A PROPOSED command: the adapter issues only from APPROVED.
    h.store.seed_revision(&schedule_id_text(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(25, 4, 17))
        .expect("creation succeeds");
    let error = adapter
        .issue(&tenant_context(), &proposed.command, &fixture_decision_record(), &actor(DISPATCHER))
        .expect_err("a PROPOSED command is not issuable");
    match error {
        DispatchAdapterError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Proposed);
            assert_eq!(operation, "issue");
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_cross_tenant_caller_cannot_issue_a_foreign_command() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 26, 4, 17);
    let adapter = adapter_for(&h, criterion(1));
    let error = adapter
        .issue(&other_tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect_err("a foreign command does not exist in another tenant's scope");
    assert!(matches!(error, DispatchAdapterError::NotFound { .. }));
    // Untouched in its own tenant.
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);
}

#[test]
fn unknown_or_unusable_assets_are_rejected_at_the_validation_gate_before_any_issue() {
    // Unknown asset (does not exist in the caller's tenant scope — a
    // foreign asset and a missing asset are never distinguished, P0.16
    // rule 5): the validation gate rejects BEFORE the command can ever be
    // approved or issued.
    let h = harness(5_000);
    h.assets.set(AssetGateStatus::UnknownInTenantScope);
    h.store.seed_revision(&schedule_id_text(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(27, 9, 17))
        .expect("creation does not consult the asset seam");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("verdict");
    assert!(!validation.passed);
    assert_eq!(
        validation.failing_check.as_deref(),
        Some("asset_unknown_in_tenant_scope")
    );
    assert_eq!(h.store.current_state(proposed.command.command_id()), CommandState::Rejected);
    // The adapter refuses the rejected command — the guard chain precedes
    // any issue.
    let adapter = adapter_for(&h, criterion(1));
    let error = adapter
        .issue(&tenant_context(), &proposed.command, &fixture_decision_record(), &actor(DISPATCHER))
        .expect_err("a REJECTED command is not issuable");
    match error {
        DispatchAdapterError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Rejected);
            assert_eq!(operation, "issue");
        }
        other => panic!("unexpected error {other:?}"),
    }

    // Unusable asset (no capability model): the same chain rejects before
    // any issue is possible. (Asset availability/deactivation is the P2.16
    // availability rule upstream of the gates; the seam's unusable outcomes
    // are the two shown here.)
    let h = harness(5_000);
    h.assets.set(AssetGateStatus::CapabilityModelMissing);
    h.store.seed_revision(&schedule_id_text(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&tenant_context(), propose_input(28, 9, 17))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&tenant_context(), &proposed.command, &actor("svc:validator"))
        .expect("verdict");
    assert!(!validation.passed);
    assert_eq!(
        validation.failing_check.as_deref(),
        Some("asset_capability_model_missing")
    );
}

// ============================================================================
// Partial fleet acknowledgement is per asset (§14.3)
// ============================================================================

#[test]
fn partial_fleet_acknowledgement_is_per_asset_never_portfolio_inferred() {
    let h = harness(5_000);
    let adapter = adapter_for(&h, criterion(1));

    // Command A (asset 4): the simulator accepts.
    let (a, record_a) = walk_to_approved(&h, 30, 4, 17);
    let outcome_a = adapter
        .issue(&tenant_context(), &a, &record_a, &actor(DISPATCHER))
        .expect("issue a");
    assert_eq!(outcome_a.acknowledgement.state, AssetAcknowledgementState::Accepted);

    // Command B (asset 5): the simulator NACKs — a PER-ASSET failure.
    h.transport.script_deliver(Ok(SimulatedAcceptance::Nack {
        reason: "asset 5 refused the setpoint (scripted)".to_string(),
    }));
    let (b, record_b) = walk_to_approved(&h, 31, 5, 18);
    let outcome_b = adapter
        .issue(&tenant_context(), &b, &record_b, &actor(DISPATCHER))
        .expect("issue b");
    assert_eq!(outcome_b.acknowledgement.state, AssetAcknowledgementState::Refused);
    assert_eq!(h.store.current_state(b.command_id()), CommandState::Failed);
    assert_eq!(h.store.current_state(a.command_id()), CommandState::Acknowledged);

    // The fleet-level answer is COMPUTED from every per-asset state: the
    // first responding device (A, accepted) never implies a fleet-level
    // acknowledgement.
    let fleet = FleetAcknowledgement::new(vec![
        outcome_a.acknowledgement,
        outcome_b.acknowledgement,
    ])
    .expect("a two-asset fleet acknowledgement");
    assert_eq!(fleet.total(), 2);
    assert_eq!(fleet.accepted_count(), 1);
    assert!(!fleet.fully_accepted());
    assert_eq!(
        fleet.portfolio_state(),
        FleetAcknowledgementState::PartiallyAccepted { accepted: 1, assets: 2 }
    );
    // The failed asset stays visible per asset.
    assert_eq!(fleet.per_asset()[1].state, AssetAcknowledgementState::Refused);
}

// ============================================================================
// The recorded decisions are stated
// ============================================================================

#[test]
fn the_adapter_records_the_simulation_boundary_in_its_public_api() {
    // The configuration surface exposes ONLY the simulator-vs-not choice —
    // there is no "field mode" to select (a plugin can never select the
    // control-authority mode, §5.7).
    let source = dispatch_simulator_source();
    assert!(source.contains("pub enum AdapterTargetCandidate"));
    assert!(source.contains("NonSimulatorTargetKind::Gateway"));
    assert!(source.contains("NonSimulatorTargetKind::ProtocolAdapter"));
    assert!(source.contains("NonSimulatorTargetKind::Device"));
    assert!(source.contains("NonSimulatorTargetKind::FieldEndpoint"));
}
