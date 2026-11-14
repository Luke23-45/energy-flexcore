// P4.29 — Invariant 7: no research-module direct dispatch
//         (report §17.5 invariant 7; §5.7 plugin authority model; §13.4
//         research boundary; ledger P4.29; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/29_safety_invariant_matrix.md`).
//
// §17.5 invariant 7: no command reaches a physical target directly from an
// optimizer, GNN, MARL, LLM, or plugin output — the full
// validation/feasibility/authorization chain is required, and the sole
// Phase 4 transport target is the SIMULATOR.
//
// What this file proves, and at which layer:
//
// 1. **Domain layer (the ledger's risk mitigation is tested here: the tests
//    call the domain directly, not only through the API).** A
//    research-plugin-shaped command enters the lifecycle ONLY at PROPOSED
//    (`CommandLifecycle::propose` is the sole constructor and the initial
//    marker `[*] → PROPOSED` is not a transition), and every gate-skip is
//    refused by the frozen-edge oracle: PROPOSED → ISSUED is not an edge
//    (`NotPermitted`), and the gate operations apply only from their own
//    states (`WrongState`).
// 2. **Application layer.** A research-plugin-shaped proposal created
//    through the real creation gate can reach ISSUED ONLY by walking
//    validation → feasibility → authorization → issue, and the issued
//    envelope goes ONLY to the injected simulator transport. There is no
//    source-based shortcut: `SourceOrigin` is recorded (§14.3) and confers
//    no authority — the gate chain is source-agnostic (§5.7: plugins may
//    propose and request validation but can never authorize, issue, or
//    select the control-authority mode).
// 3. **Deny-by-default authorization.** With no matching grant, the
//    research-shaped proposal is REJECTED at the authorization gate and
//    ISSUED stays unreachable.
// 4. **Invariant 6 re-verification (P4.16 target guard, D-P4.16-6).** The
//    adapter's transport target is the SIMULATOR ONLY: every
//    non-simulator target candidate is refused by the constructor.
//
// Control status: the command lifecycle is a protected platform invariant
// (§0.2, §5.7). The entire 0.x line is simulation-only (§21.4): an ISSUED
// command here was sent to the SIMULATOR and nothing else. FlexCore
// performs no primary frequency response, synthetic inertia, relay
// protection, inverter inner-loop control, or sub-cycle control (§14.5).
// No safety, certification, or transport-security claim is made.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthzAction, CommandAuthorizationEvaluator,
    CommandAuthorizationRequest, CommandFeasibilityRefs, CommandFeasibilityRunner,
    CommandGateError, CommandGatePolicy, CommandGateStore, CommandGateStoreError,
    CommandGateView, CommandGates, DeclaredQuantities, EnvelopeGateFreshness,
    FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus, ModelVersionSource,
    ProposeCommandInput, ProposedCommandRecord, ScheduleEnvelopeSource, ScheduleRevisionUsability,
    COMMAND_ENTITY,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, DispatchAdapterError, NonSimulatorTargetKind, SimulatedAcceptance,
    SimulatedObservation, SimulatorDispatchAdapter, SimulatorDispatchEnvelope,
    SimulatorDispatchTransport, SimulationKeyring, TrackingAcceptanceCriterion, TrackedQuantity,
    SIMULATION_ONLY_STATEMENT, SIMULATOR_TRANSPORT_IDENTITY_PREFIX,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandLifecycle, CommandTransition, EvidenceReference, Gate, LifecycleClock,
    LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, EvidenceReferences, ExecutionRequirements, RampLimits,
    ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c04";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c05";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c06";

// The research-plugin-shaped source (§13.4 boundary: a research output is a
// PROPOSAL SOURCE only; it holds no command authority).
const RESEARCH_SUBSYSTEM: &str = "research:gnn-marl-experimental";
const RESEARCH_PLUGIN_VERSION: &str = "plugin/0.0.0-unqualified";

const PROPOSER: &str = "service:research-orchestrator";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:modelhash01";
const SIMULATOR_ID: &str = "sim:phase4-vertical-slice";
const SIM_KEY_ID: &str = "sim-key-p429-nonproduction-test";

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

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
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
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

// --- Seam fakes (the same harness shape as tests/command_gates.rs) ----------------

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
            attempt_id: "attempt-p429".to_string(),
            model_id: "feeder-a".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.97; max loading 71%".to_string(),
            coverage: "balanced radial feeder-a; unbalanced deferred (P4.34)".to_string(),
        })
    }
}

/// Deny-by-default authorization evaluator: only the seeded roles are
/// allowed; everything else denies (the §17.3 deny-by-default surface).
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
                rule_reference: format!("policy:phase4:{}:command.approve:asset", request.role),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
    }
}

struct FakeTransport {
    delivered: Mutex<Vec<SimulatorDispatchEnvelope>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            delivered: Mutex::new(Vec::new()),
        }
    }
}

impl SimulatorDispatchTransport for FakeTransport {
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, flexcore_application::dispatch_simulator::SimulatorTransportError>
    {
        self.delivered
            .lock()
            .expect("fake transport")
            .push(envelope.clone());
        Ok(SimulatedAcceptance::Accepted {
            acceptance_reference: "sim-accept:p429-1".to_string(),
        })
    }

    fn observe(
        &self,
        _asset_id: AssetId,
        _at_unix_millis: i64,
    ) -> Result<Option<SimulatedObservation>, flexcore_application::dispatch_simulator::SimulatorTransportError>
    {
        Ok(None)
    }
}

/// Renders the event's Debug form (which names the frozen variant).
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(
        &self,
        envelope: &flexcore_events::envelope::EventEnvelope,
    ) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- The gate harness --------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    authz: &'static FakeAuthz,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
    transport: &'static FakeTransport,
}

fn harness(allowed_roles: &[&str]) -> Harness {
    let store = Box::leak(Box::new(FakeStore::new()));
    let envelopes = Box::leak(Box::new(FakeEnvelopes));
    let models = Box::leak(Box::new(FakeModels));
    let assets = Box::leak(Box::new(FakeAssets));
    let runner = Box::leak(Box::new(FakeRunner));
    let authz = Box::leak(Box::new(FakeAuthz {
        allowed_roles: allowed_roles.iter().map(|s| s.to_string()).collect(),
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
    let transport = Box::leak(Box::new(FakeTransport::new()));
    Harness {
        gates,
        store,
        authz,
        outbox,
        clock,
        transport,
    }
}

// --- The research-plugin-shaped proposal ---------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

/// The proposal a research plugin would submit: `SourceOrigin` names the
/// research subsystem and the unqualified plugin version (§14.3 field 3).
/// Everything about the PROPOSAL is plugin-shaped; everything about its
/// PATH is the platform's gate chain.
fn research_propose_input() -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: command_id(),
        scope: CommandScope {
            tenant_id: tenant(),
            portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
            site_id: SiteId::new(uuid(UUID_SITE)),
            asset_id: AssetId::new(uuid(UUID_ASSET)),
        },
        source: SourceOrigin {
            subsystem: RESEARCH_SUBSYSTEM.to_string(),
            component_version: RESEARCH_PLUGIN_VERSION.to_string(),
        },
        schedule: ScheduleReference {
            schedule_id: ScheduleId::new(uuid(UUID_SCHEDULE)),
            revision: 2,
        },
        intent: CommandIntent {
            reason: "GNN/MARL research candidate action".to_string(),
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
        idempotency_key: "idem:research-op-1".to_string(),
        evidence: EvidenceReferences {
            grid_model: "grid-model:feeder-a@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:research-1".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p429".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

fn criterion(min_satisfied_samples: u32) -> TrackingAcceptanceCriterion {
    TrackingAcceptanceCriterion::new(
        TrackedQuantity::ActivePowerMW,
        0.2,
        min_satisfied_samples,
        5_000,
    )
    .expect("a valid acceptance criterion")
}

fn adapter_for(h: &Harness) -> SimulatorDispatchAdapter<'_> {
    SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: SIMULATOR_ID.to_string(),
        },
        h.transport,
        SimulationKeyring::new(SIM_KEY_ID).expect("a non-empty simulation key id"),
        criterion(1),
    )
    .expect("the simulator target is the one accepted candidate")
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
        "DispatchIssued",
        "DispatchAcknowledged",
    ] {
        if debug_payload.starts_with(candidate) {
            return candidate;
        }
    }
    "unknown"
}

/// Creates the research-shaped proposal through the real creation gate
/// (the ONLY entry a plugin has) and seeds the active revision.
fn create_research_proposal(h: &Harness) -> flexcore_domain::dispatch_command::DispatchCommand {
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), research_propose_input())
        .expect("the research-shaped proposal enters at PROPOSED through the creation gate");
    assert_eq!(proposed.command.source().subsystem, RESEARCH_SUBSYSTEM);
    proposed.command
}

// ============================================================================
// Invariant 7 — the domain layer: every gate-skip is refused by the
// frozen-edge oracle (tests call the domain DIRECTLY, per the ledger risk
// mitigation — not only through the API layer).
// ============================================================================

#[test]
fn a_research_plugin_command_enters_only_at_proposed_and_no_direct_issue_edge_exists() {
    // The domain aggregate: `propose` is the ONLY constructor and the
    // initial marker `[*] → PROPOSED` is not a transition — there is no
    // public way to construct a lifecycle in ISSUED (or any non-PROPOSED)
    // state: the fields are private and `propose` fixes the state.
    let clock = TestClock::at(5_000);
    let mut lifecycle =
        CommandLifecycle::propose(command_id(), ValidUntil::from_unix_millis(70_000).unwrap())
            .expect("the lifecycle enters at PROPOSED");
    assert_eq!(lifecycle.state(), CommandState::Proposed);
    assert!(lifecycle.journal().is_empty());

    // PROPOSED → ISSUED is not a frozen edge: a "direct dispatch" that
    // skips validation, feasibility, AND authorization is refused.
    let direct_issue = lifecycle.record_issue(
        EvidenceReference::new("sig:bypass-attempt").expect("non-empty reference"),
        actor("research:gnn-marl-experimental"),
        &clock,
    );
    match direct_issue {
        Err(LifecycleError::NotPermitted { from, to }) => {
            assert_eq!(from, CommandState::Proposed);
            assert_eq!(to, CommandState::Issued);
        }
        other => panic!("a direct PROPOSED → ISSUED issue must be refused, got: {other:?}"),
    }
    assert_eq!(lifecycle.state(), CommandState::Proposed);

    // Each gate operation refuses to run out of order on the same
    // PROPOSED command: feasibility without validation, authorization
    // without feasibility.
    let mut lifecycle = CommandLifecycle::propose(
        command_id(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .expect("the lifecycle enters at PROPOSED");
    let passed = flexcore_domain::command_lifecycle::GateOutcome::pass(
        EvidenceReference::new("feasibility:forged").expect("non-empty reference"),
    );
    match lifecycle.record_feasibility(passed, actor("svc:grid-gate"), &clock) {
        Err(LifecycleError::WrongState { current, operation }) => {
            assert_eq!(current, CommandState::Proposed);
            assert_eq!(operation, "record_feasibility");
        }
        other => panic!("feasibility without validation must be refused, got: {other:?}"),
    }
    let approval = flexcore_domain::command_lifecycle::AuthorizationOutcome::approve(
        EvidenceReference::new("authz:forged").expect("non-empty reference"),
        actor(APPROVER),
    );
    match lifecycle.record_authorization(approval, &clock) {
        Err(LifecycleError::WrongState { current, operation }) => {
            assert_eq!(current, CommandState::Proposed);
            assert_eq!(operation, "record_authorization");
        }
        other => panic!("authorization without feasibility must be refused, got: {other:?}"),
    }
    // The journal is still empty: no forged transition was recorded.
    assert!(lifecycle.journal().is_empty());
}

// ============================================================================
// Invariant 7 — the application layer: the full gate chain is required and
// the only target is the simulator.
// ============================================================================

#[test]
fn a_research_plugin_proposal_reaches_issued_only_through_the_full_gate_chain() {
    let h = harness(&[APPROVER_ROLE]);
    let command = create_research_proposal(&h);

    // The application-layer skips are refused exactly as the domain ones:
    // feasibility on a PROPOSED command, approval on a PROPOSED command,
    // and issue on a PROPOSED command.
    let proposed_error = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect_err("feasibility on a PROPOSED command is refused");
    assert!(
        matches!(proposed_error, CommandGateError::WrongState { .. }),
        "{proposed_error}"
    );
    let approval_error = h
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
        .expect_err("approval on a PROPOSED command is refused");
    assert!(
        matches!(approval_error, CommandGateError::WrongState { .. }),
        "{approval_error}"
    );
    let authorization = AuthorizationDecisionRecord {
        decision_reference: "authz:forged".to_string(),
        authorizing_actor: APPROVER.to_string(),
        authorizing_role: APPROVER_ROLE.to_string(),
        approved_scope: "asset".to_string(),
        approved_limits: "±2.0 MW".to_string(),
        decided_at_unix_millis: 5_000,
        expiry_unix_millis: 200_000,
        separation_of_duties: flexcore_application::command_gates::AppliedSeparationOfDuties::Enforced,
        covered_validation_evidence: "validation:forged".to_string(),
        covered_feasibility_evidence: "feasibility:forged".to_string(),
    };
    let adapter = adapter_for(&h);
    let issue_error = adapter
        .issue(&context(), &command, &authorization, &actor("svc:dispatcher"))
        .expect_err("issue on a PROPOSED command is refused");
    assert!(
        matches!(issue_error, DispatchAdapterError::WrongState { .. }),
        "{issue_error}"
    );
    // Nothing was delivered by the skipped attempts.
    assert!(h.transport.delivered.lock().expect("transport").is_empty());

    // Now the chain, in order — the ONLY path to ISSUED.
    let validation = h
        .gates
        .run_validation_gate(&context(), &command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed);
    let feasibility = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("feasibility verdict");
    assert!(feasibility.result.passed);
    let feasibility_evidence = feasibility
        .feasibility_evidence
        .as_ref()
        .expect("feasibility evidence travels");
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
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the approval verdict");
    assert!(approval.approved);
    let decision: AuthorizationDecisionRecord = approval.decision_record.expect("the record");
    // The authorization decision covers the EXACT validation + feasibility
    // evidence the chain produced — approval cannot float free of the
    // evidence (§17.3; the P4.13 recorded decision fields).
    assert_eq!(decision.covered_validation_evidence, validation.evidence_reference);
    assert_eq!(
        decision.covered_feasibility_evidence,
        feasibility_evidence.reference
    );

    let outcome = adapter
        .issue(&context(), &command, &decision, &actor("svc:dispatcher"))
        .expect("the chain-completed command issues");
    assert_eq!(outcome.issued.from, CommandState::Approved);
    assert_eq!(outcome.issued.to, CommandState::Issued);

    // The journal names all THREE gates, in order, plus the issue.
    let journal = h.store.journal_of(command.command_id());
    let gates_in_order: Vec<Option<Gate>> = journal.iter().map(|t| t.gate).collect();
    assert_eq!(
        gates_in_order,
        vec![
            Some(Gate::Validation),
            Some(Gate::Feasibility),
            Some(Gate::Authorization),
            None, // ISSUED (not a gate transition)
            None, // ACKNOWLEDGED
        ]
    );

    // The ONLY deliveries went to the injected SIMULATOR transport, with
    // the simulation transport identity — never a field endpoint.
    let delivered = h.transport.delivered.lock().expect("transport");
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].command_id, command.command_id());
    assert!(
        delivered[0].transport_identity.starts_with(SIMULATOR_TRANSPORT_IDENTITY_PREFIX),
        "transport identity: {}",
        delivered[0].transport_identity
    );
    assert_eq!(
        delivered[0].target,
        flexcore_application::dispatch_simulator::SimulatorTargetValue::ActivePowerMW(1.5)
    );

    // The emitted event set is the frozen one for the walked chain.
    assert_eq!(
        emitted_events(&h),
        vec![
            "DispatchProposed",
            "DispatchApproved",
            "DispatchIssued",
            "DispatchAcknowledged",
        ]
    );
}

#[test]
fn the_research_plugin_source_confers_no_authority_deny_by_default_holds() {
    let h = harness(&[]); // NO grants: deny-by-default (§17.3)
    let command = create_research_proposal(&h);
    let validation = h
        .gates
        .run_validation_gate(&context(), &command, &actor("svc:validator"))
        .expect("validation verdict");
    assert!(validation.passed, "validation passes");
    let feasibility = h
        .gates
        .run_feasibility_gate(&context(), &command, &feasibility_refs(), &actor("svc:grid-gate"))
        .expect("feasibility verdict");
    assert!(feasibility.result.passed, "feasibility passes");

    // A source carrying no grant cannot approve itself or anyone: the
    // authorization gate denies and the command is REJECTED.
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &CommandAuthorizationRequest {
                subject: PROPOSER.to_string(),
                role: "research-plugin".to_string(),
                action: CommandAuthzAction::Approve,
                scope: ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET))),
            },
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);

    // REJECTED is terminal for issue purposes: the adapter refuses.
    let journal = h.store.journal_of(command.command_id());
    let last = journal.last().expect("the rejection transition");
    assert_eq!(last.to, CommandState::Rejected);
    assert_eq!(last.gate, Some(Gate::Authorization));
    let adapter = adapter_for(&h);
    let authorization = AuthorizationDecisionRecord {
        decision_reference: "authz:forged".to_string(),
        authorizing_actor: PROPOSER.to_string(),
        authorizing_role: "research-plugin".to_string(),
        approved_scope: "asset".to_string(),
        approved_limits: "±2.0 MW".to_string(),
        decided_at_unix_millis: 5_000,
        expiry_unix_millis: 200_000,
        separation_of_duties: flexcore_application::command_gates::AppliedSeparationOfDuties::Enforced,
        covered_validation_evidence: "validation:forged".to_string(),
        covered_feasibility_evidence: "feasibility:forged".to_string(),
    };
    let issue_error = adapter
        .issue(&context(), &command, &authorization, &actor("svc:dispatcher"))
        .expect_err("a REJECTED command cannot issue");
    assert!(
        matches!(issue_error, DispatchAdapterError::WrongState { .. }),
        "{issue_error}"
    );
    assert!(h.transport.delivered.lock().expect("transport").is_empty());
}

// ============================================================================
// Invariant 6 re-verification — the P4.16 target guard (D-P4.16-6): the
// transport target is the SIMULATOR ONLY.
// ============================================================================

#[test]
fn the_only_dispatch_target_is_the_simulator_every_non_simulator_target_is_refused() {
    let h = harness(&[APPROVER_ROLE]);
    // Every non-simulator candidate — gateway, protocol adapter, device,
    // field endpoint — is refused by the adapter constructor with the
    // recorded kind and endpoint.
    let kinds = [
        (NonSimulatorTargetKind::Gateway, "gateway"),
        (NonSimulatorTargetKind::ProtocolAdapter, "protocol_adapter"),
        (NonSimulatorTargetKind::Device, "device"),
        (NonSimulatorTargetKind::FieldEndpoint, "field_endpoint"),
    ];
    for (kind, label) in kinds {
        let endpoint = format!("{label}://field-endpoint.example");
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
        match error {
            DispatchAdapterError::NonSimulatorTargetRefused {
                kind: refused_kind,
                endpoint: refused_endpoint,
            } => {
                assert_eq!(refused_kind, label);
                assert_eq!(refused_endpoint, endpoint);
            }
            other => panic!("unexpected error for {label}: {other}"),
        }
    }
    // The simulator is the one accepted candidate.
    let adapter = adapter_for(&h);
    assert_eq!(adapter.simulator_id(), SIMULATOR_ID);
    // The simulation-only boundary is recorded in the public API.
    assert!(
        SIMULATION_ONLY_STATEMENT.contains("SIMULATOR ONLY"),
        "the recorded statement must name the simulator-only boundary: {SIMULATION_ONLY_STATEMENT}"
    );
    // No delivery happened for any refused target.
    assert!(h.transport.delivered.lock().expect("transport").is_empty());
}
