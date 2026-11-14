// P4.28 — Dispatch security controls tests (§17.3–§17.4, §17.5 inv. 8,
//         §16.2, §14.3 field 15, §19.2 layer 7; ledger P4.28; decision
//         record:
//         `ledger/phase-04-grid-feasibility-dispatch/28_security_controls.md`).
//
// Coverage per the ledger acceptance criteria (every expectation below is
// pre-registered BEFORE any run — the human build/test pass executes what
// this file pins; nothing here has ever been run):
//
// - Tampered payload: a signature that verified the ORIGINAL payload fails
//   on ANY changed field (value, sequence, transport identity, bound
//   command) — `TamperedPayload`, never a pass.
// - Missing signature: an empty/whitespace reference is refused
//   (`MissingSignature`), and the adapter layer refuses an unsigned
//   command (`IssueRefusal::Unsigned`, the P4.16 refusal re-proven here as
//   the security suite's own case).
// - Wrong key: a signature produced under a different key id is refused
//   (`ForeignKey`).
// - Altered sequence: a signature bound to sequence N fails on a binding
//   carrying sequence M ≠ N (`TamperedPayload`); a receive-point envelope
//   with an unsequenced (0) payload is refused.
// - Replayed sequence: equal to the last accepted → `Replayed`; the guard
//   state never advances on a refusal.
// - Out-of-order sequence: lower than the last accepted → `OutOfOrder`.
// - Cross-tenant read/approve/issue: every attempt is denied (existence
//   never leaks — the named `NotFound`-shaped refusal, P0.16 rule 5) and
//   each denial produces an appended `TenantDenialAuditRecord` (§16.2) via
//   the `SecurityAuditSink` seam.
// - No secrets: structural assertions — payloads, signature references,
//   transport identities, and audit texts contain no credential-shaped
//   material; test keys are generated IN-TEST with clearly-labelled
//   non-production simulation ids; the keyring layers hold key
//   IDENTIFIERS only (source-scanned).
// - Determinism: the whole attack surface run twice from fresh state
//   yields identical outcomes.
//
// Every timestamp comes from a controlled, advanceable clock (never
// wall-clock); every faked seam fails closed by construction.

use std::sync::Mutex;

use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandGatePolicy, CommandGateStore,
    CommandGateStoreError, CommandGateView, CommandGates, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposedCommandRecord, ProposeCommandInput, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::dispatch_security::{
    channel_key, record_tenant_denial, sign_dispatch_payload, verify_dispatch_payload,
    DispatchPayloadBinding, DispatchSecurityError, PayloadBoundVerifier, SecurityAuditError,
    SecurityAuditSink, SequenceGuard, SequenceRefusal, TenantDenialAuditRecord, TenantDenialKind,
    VerifyingDispatchTransport, DEFAULT_DISPATCH_CHANNEL, DISPATCH_SECURITY_STATEMENT,
    PAYLOAD_SIGNATURE_PREFIX, SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT, SIMULATION_DIGEST_ALG,
};
use flexcore_application::dispatch_simulator::{
    check_issue_refusals, AdapterTargetCandidate, DispatchAdapterError, IssueRefusal,
    IssueRefusalInput, SimulatedAcceptance, SimulatedObservation, SimulationKeyring,
    SimulatorDispatchAdapter, SimulatorDispatchEnvelope, SimulatorDispatchTransport,
    SimulatorTargetValue, SimulatorTransportError, TrackedQuantity, TrackingAcceptanceCriterion,
    SIMULATOR_TRANSPORT_IDENTITY_PREFIX,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, LifecycleClock};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::envelope::EventEnvelope;
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

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("a valid actor")
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-dispatcher";
const DISPATCHER: &str = "svc:dispatcher";
const MODEL_HASH: &str = "fnv1a64:sec-feeder-hash-01";
const SIMULATOR_ID: &str = "sim:p428-security-fixture";
/// The test key id: generated IN-TEST, clearly labelled non-production
/// (never a committed credential; the fixture carries an identifier only).
const SIM_KEY_ID: &str = "sim-key-p428-nonproduction-test";
/// A DIFFERENT simulation key (the wrong-key attack's signer).
const OTHER_KEY_ID: &str = "sim-key-p428-other-nonproduction-test";

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
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

/// Renders the event's Debug form (names the frozen variant; no
/// serialization library).
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- Store fake (tenant-scoped; existence never leaks) -------------------------------

struct FakeStore {
    records: Mutex<std::collections::HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<std::collections::HashMap<String, Vec<CommandTransition>>>,
    revisions: Mutex<std::collections::HashMap<(String, u64), ScheduleRevisionUsability>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(std::collections::HashMap::new()),
            journals: Mutex::new(std::collections::HashMap::new()),
            revisions: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn seed_revision(&self, schedule_id: &str, revision: u64, state: ScheduleRevisionUsability) {
        self.revisions
            .lock()
            .expect("fake store")
            .insert((schedule_id.to_string(), revision), state);
    }

    fn current_state(&self, command_id: CommandId) -> CommandState {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .and_then(|entries| entries.last())
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

/// The inner (simulator-side) transport: records every envelope that
/// REACHED it — the "did a refused envelope leak through" oracle.
struct InnerTransport {
    delivered: Mutex<Vec<SimulatorDispatchEnvelope>>,
}

impl InnerTransport {
    fn new() -> Self {
        Self {
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn delivered(&self) -> Vec<SimulatorDispatchEnvelope> {
        self.delivered.lock().expect("inner transport").clone()
    }
}

impl SimulatorDispatchTransport for InnerTransport {
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError> {
        self.delivered
            .lock()
            .expect("inner transport")
            .push(envelope.clone());
        Ok(SimulatedAcceptance::Accepted {
            acceptance_reference: format!("acceptance:sim/{}", envelope.command_id),
        })
    }

    fn observe(
        &self,
        _asset_id: AssetId,
        _at_unix_millis: i64,
    ) -> Result<Option<SimulatedObservation>, SimulatorTransportError> {
        Ok(None)
    }
}

/// The SecurityAuditSink fake: records every appended denial.
struct FakeSecurityAuditSink {
    records: Mutex<Vec<TenantDenialAuditRecord>>,
}

impl FakeSecurityAuditSink {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    fn records(&self) -> Vec<TenantDenialAuditRecord> {
        self.records.lock().expect("audit sink").clone()
    }
}

impl SecurityAuditSink for FakeSecurityAuditSink {
    fn append_tenant_denial(
        &self,
        record: &TenantDenialAuditRecord,
    ) -> Result<(), SecurityAuditError> {
        self.records
            .lock()
            .expect("audit sink")
            .push(record.clone());
        Ok(())
    }
}

// --- Harness ------------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    transport: &'static InnerTransport,
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
    let outbox: &'static FakeEventOutbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let transport: &'static InnerTransport = Box::leak(Box::new(InnerTransport::new()));
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
        transport,
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
            duration: std::time::Duration::from_secs(30),
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

/// Walks one command through the REAL gate chain to APPROVED.
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
        approval
            .decision_record
            .expect("an approval carries its decision record"),
    )
}

// --- Envelope fixture helpers ---------------------------------------------------------

fn test_keyring() -> SimulationKeyring {
    SimulationKeyring::new(SIM_KEY_ID).expect("a non-empty simulation key id")
}

fn other_keyring() -> SimulationKeyring {
    SimulationKeyring::new(OTHER_KEY_ID).expect("a non-empty simulation key id")
}

/// A valid, correctly signed envelope (the control case every attack
/// fixture is derived from by changing exactly one thing).
fn signed_envelope(command_tag: u8, asset_tag: u8, sequence: u64, p_mw: f64) -> SimulatorDispatchEnvelope {
    let keyring = test_keyring();
    let mut envelope = SimulatorDispatchEnvelope {
        command_id: CommandId::new(uuid(command_tag)),
        asset_id: asset_id(asset_tag),
        sequence,
        target: SimulatorTargetValue::ActivePowerMW(p_mw),
        signature_reference: String::new(),
        transport_identity: format!(
            "{SIMULATOR_TRANSPORT_IDENTITY_PREFIX}/{}",
            keyring.active_key_id()
        ),
    };
    envelope.signature_reference =
        sign_dispatch_payload(&keyring, &DispatchPayloadBinding::from_envelope(&envelope));
    envelope
}

fn verifying_transport() -> (
    &'static VerifyingDispatchTransport<'static>,
    &'static InnerTransport,
) {
    let inner: &'static InnerTransport = Box::leak(Box::new(InnerTransport::new()));
    let keyring: &'static SimulationKeyring = Box::leak(Box::new(test_keyring()));
    let verifier: &'static PayloadBoundVerifier<'static> =
        Box::leak(Box::new(PayloadBoundVerifier::new(keyring)));
    let decorated: &'static VerifyingDispatchTransport<'static> =
        Box::leak(Box::new(VerifyingDispatchTransport::new(inner, verifier)));
    (decorated, inner)
}

fn nack_reason(result: Result<SimulatedAcceptance, SimulatorTransportError>) -> String {
    match result {
        Ok(SimulatedAcceptance::Nack { reason }) => reason,
        Ok(SimulatedAcceptance::Accepted { .. }) => panic!("expected a NACK, got acceptance"),
        Err(error) => panic!("expected a NACK, got transport error {error}"),
    }
}

// ============================================================================
// The recorded statements (what IS and IS NOT claimed)
// ============================================================================

#[test]
fn the_security_statements_are_recorded() {
    // The control statement: simulation-layer code-level controls only, the
    // dev keyring must be replaced, mTLS NOT implemented, no claims.
    assert!(DISPATCH_SECURITY_STATEMENT.contains("SIMULATION layer"));
    assert!(DISPATCH_SECURITY_STATEMENT.contains("non-cryptographic"));
    assert!(DISPATCH_SECURITY_STATEMENT.contains("Stage 6/9"));
    assert!(DISPATCH_SECURITY_STATEMENT.contains("mTLS is NOT implemented"));
    assert!(DISPATCH_SECURITY_STATEMENT.contains("no certification"));
    // The secrets statement: rotation and short-lived credentials are
    // RECORDED deployment requirements, not implementations; no secret
    // store is consulted.
    assert!(SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT.contains("DEPLOYMENT REQUIREMENTS"));
    assert!(SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT.contains("NOT implemented"));
    assert!(SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT.contains("no real secret store"));
    // The digest is the recorded non-cryptographic pattern, labelled.
    assert_eq!(SIMULATION_DIGEST_ALG, "fnv1a64");
    assert_eq!(PAYLOAD_SIGNATURE_PREFIX, "sim-sig");
    assert_eq!(DEFAULT_DISPATCH_CHANNEL, "dispatch");
}

// ============================================================================
// Signed commands: tampered payload, missing signature, wrong key
// (ledger P4.28 step 1)
// ============================================================================

#[test]
fn a_tampered_payload_fails_the_signature_check() {
    let keyring = test_keyring();
    let original = signed_envelope(40, 4, 7, 1.5);

    // Control: the original verifies.
    verify_dispatch_payload(
        &keyring,
        &DispatchPayloadBinding::from_envelope(&original),
        &original.signature_reference,
    )
    .expect("the untampered envelope verifies");

    // Attack 1 — tamper the requested VALUE (the classic setpoint swap).
    let mut tampered = original.clone();
    tampered.target = SimulatorTargetValue::ActivePowerMW(9.9);
    let error = verify_dispatch_payload(
        &keyring,
        &DispatchPayloadBinding::from_envelope(&tampered),
        &original.signature_reference,
    )
    .expect_err("a value-tampered payload MUST fail the signature");
    match error {
        DispatchSecurityError::TamperedPayload { ref command_id, .. } => {
            assert_eq!(command_id, &original.command_id.to_string());
        }
        other => panic!("unexpected error {other:?}"),
    }

    // Attack 2 — tamper the transport identity (channel hijack).
    let mut tampered = original.clone();
    tampered.transport_identity = "sim://elsewhere/attacker".to_string();
    assert!(matches!(
        verify_dispatch_payload(
            &keyring,
            &DispatchPayloadBinding::from_envelope(&tampered),
            &original.signature_reference,
        ),
        Err(DispatchSecurityError::TamperedPayload { .. })
    ));

    // Attack 3 — tamper the bound command id (signature replayed onto
    // another command).
    let mut tampered = original.clone();
    tampered.command_id = CommandId::new(uuid(41));
    assert!(matches!(
        verify_dispatch_payload(
            &keyring,
            &DispatchPayloadBinding::from_envelope(&tampered),
            &original.signature_reference,
        ),
        Err(DispatchSecurityError::TamperedPayload { .. })
    ));

    // Attack 4 — tamper the asset (redirect the command to another asset).
    let mut tampered = original.clone();
    tampered.asset_id = asset_id(9);
    assert!(matches!(
        verify_dispatch_payload(
            &keyring,
            &DispatchPayloadBinding::from_envelope(&tampered),
            &original.signature_reference,
        ),
        Err(DispatchSecurityError::TamperedPayload { .. })
    ));

    // Attack 5 — the presented reference itself edited (digest digit
    // swapped): the reference is the 16-hex-digest tail, so flipping one
    // digit is a presented-digest mismatch.
    let mut patched = original.signature_reference.clone();
    let last_char = patched.pop().expect("the reference has a digest tail");
    patched.push(if last_char == '0' { '1' } else { '0' });
    assert!(matches!(
        verify_dispatch_payload(
            &keyring,
            &DispatchPayloadBinding::from_envelope(&original),
            &patched,
        ),
        Err(DispatchSecurityError::TamperedPayload { .. })
    ));
}

#[test]
fn a_missing_signature_is_rejected_at_both_layers() {
    let keyring = test_keyring();
    let binding = DispatchPayloadBinding {
        command_id: CommandId::new(uuid(42)),
        asset_id: asset_id(4),
        sequence: 7,
        quantity: TrackedQuantity::ActivePowerMW.label(),
        value: 1.5,
        transport_identity: SIMULATOR_TRANSPORT_IDENTITY_PREFIX.to_string(),
    };

    // Layer 1 — the signature verification surface.
    for presented in ["", "   "] {
        let error = verify_dispatch_payload(&keyring, &binding, presented)
            .expect_err("a missing signature is rejected");
        assert!(matches!(error, DispatchSecurityError::MissingSignature { .. }));
    }

    // Layer 2 — the adapter's issue refusal (the P4.16 check, re-proven as
    // this suite's own missing-signature case; both authenticity kinds).
    for unsigned in [
        CommandAuthenticity::Signature("   ".to_string()),
        CommandAuthenticity::AuthenticatedTransport(String::new()),
    ] {
        let error = check_issue_refusals(&IssueRefusalInput {
            authenticity: &unsigned,
            sequence: 7,
            valid_until_millis: 1_000,
            now_unix_millis: 500,
        })
        .expect_err("an unsigned command is refused at issue");
        assert!(matches!(error, IssueRefusal::Unsigned));
    }
}

#[test]
fn a_wrong_key_signature_is_refused() {
    let signer = other_keyring();
    let verifier = test_keyring();
    let envelope = {
        let mut envelope = SimulatorDispatchEnvelope {
            command_id: CommandId::new(uuid(43)),
            asset_id: asset_id(4),
            sequence: 7,
            target: SimulatorTargetValue::ActivePowerMW(1.5),
            signature_reference: String::new(),
            transport_identity: format!(
                "{SIMULATOR_TRANSPORT_IDENTITY_PREFIX}/{}",
                signer.active_key_id()
            ),
        };
        envelope.signature_reference =
            sign_dispatch_payload(&signer, &DispatchPayloadBinding::from_envelope(&envelope));
        envelope
    };
    let error = verify_dispatch_payload(
        &verifier,
        &DispatchPayloadBinding::from_envelope(&envelope),
        &envelope.signature_reference,
    )
    .expect_err("a signature from a foreign key is refused");
    match error {
        DispatchSecurityError::ForeignKey {
            expected_key_id,
            presented_key_id,
            ..
        } => {
            assert_eq!(expected_key_id, SIM_KEY_ID);
            assert_eq!(presented_key_id, OTHER_KEY_ID);
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The refusal text names both keys (audit-ready).
    assert!(error.to_string().contains(OTHER_KEY_ID));
    assert!(error.to_string().contains(SIM_KEY_ID));
}

#[test]
fn a_signature_that_does_not_cover_the_payload_is_refused() {
    // The legacy P4.16 keyring reference (`sim-sig:<key>:<command>`) is
    // traceability only — it does NOT bind the payload, so the verifying
    // transport refuses it (recorded wiring note; it cannot prove the
    // envelope untampered).
    let keyring = test_keyring();
    let envelope = signed_envelope(44, 4, 7, 1.5);
    let legacy = keyring.sign_command(envelope.command_id);
    let error = verify_dispatch_payload(
        &keyring,
        &DispatchPayloadBinding::from_envelope(&envelope),
        &legacy,
    )
    .expect_err("a non-payload-bound reference is refused");
    assert!(matches!(error, DispatchSecurityError::MalformedSignature { .. }));
}

// ============================================================================
// Sequence / replay (ledger P4.28 step 2)
// ============================================================================

#[test]
fn an_altered_sequence_fails_the_signature_check() {
    let keyring = test_keyring();
    let signed_at_7 = signed_envelope(45, 4, 7, 1.5);

    // The same signature presented for a binding carrying sequence 8 (an
    // attacker moving the sequence to jump the monotonic window) fails.
    let mut altered = DispatchPayloadBinding::from_envelope(&signed_at_7);
    altered.sequence = 8;
    assert!(matches!(
        verify_dispatch_payload(&keyring, &altered, &signed_at_7.signature_reference),
        Err(DispatchSecurityError::TamperedPayload { .. })
    ));
}

#[test]
fn the_sequence_guard_distinguishes_replay_from_out_of_order_and_never_advances_on_a_refusal() {
    let mut guard = SequenceGuard::new();
    let channel = channel_key(&asset_id(4), DEFAULT_DISPATCH_CHANNEL);

    // Sequence 0 is unsequenced, never admitted.
    assert_eq!(
        guard.admit(&channel, 0),
        Err(SequenceRefusal::InvalidSequence)
    );
    assert_eq!(guard.last_accepted(&channel), None, "a refused sequence never advances the window");

    // First admitted sequence: any ≥ 1.
    assert!(guard.admit(&channel, 5).is_ok());
    assert_eq!(guard.last_accepted(&channel), Some(5));

    // REPLAY: exactly the last accepted sequence again.
    assert_eq!(
        guard.admit(&channel, 5),
        Err(SequenceRefusal::Replayed { last_accepted: 5 })
    );
    assert_eq!(guard.last_accepted(&channel), Some(5), "the replay did not move the window");

    // OUT OF ORDER: lower than the last accepted.
    assert_eq!(
        guard.admit(&channel, 3),
        Err(SequenceRefusal::OutOfOrder { last_accepted: 5 })
    );
    assert_eq!(guard.last_accepted(&channel), Some(5), "the refused sequence did not move the window");

    // The window is still at 5 — a strictly greater sequence is admitted.
    assert!(guard.admit(&channel, 6).is_ok());
    assert_eq!(guard.last_accepted(&channel), Some(6));

    // Channels are independent (per asset/channel, §14.3 field 10).
    let other_channel = channel_key(&asset_id(5), DEFAULT_DISPATCH_CHANNEL);
    assert!(guard.admit(&other_channel, 1).is_ok(), "another asset's channel starts fresh");
    assert_eq!(
        guard.admit(&other_channel, 1),
        Err(SequenceRefusal::Replayed { last_accepted: 1 })
    );

    // The refusal codes are the stable audit vocabulary.
    assert_eq!(SequenceRefusal::InvalidSequence.code(), "unsequenced");
    assert_eq!(SequenceRefusal::Replayed { last_accepted: 1 }.code(), "replayed_sequence");
    assert_eq!(SequenceRefusal::OutOfOrder { last_accepted: 1 }.code(), "out_of_order_sequence");
}

// ============================================================================
// The verifying transport: refused envelopes never reach the simulator
// ============================================================================

#[test]
fn a_valid_signed_sequenced_envelope_is_delivered_and_a_tampered_one_is_not() {
    let (transport, inner) = verifying_transport();
    let good = signed_envelope(50, 4, 7, 1.5);

    // Control: the correctly signed, sequenced envelope passes and reaches
    // the inner transport exactly once.
    let result = transport.deliver(&good).expect("the control envelope delivers");
    assert!(matches!(result, SimulatedAcceptance::Accepted { .. }));
    assert_eq!(inner.delivered().len(), 1);
    assert_eq!(transport.last_accepted_sequence(&asset_id(4)), Some(7));

    // Attack: the SAME signature on a payload with a swapped setpoint —
    // refused as a NACK, and the inner transport is NEVER called again.
    let mut tampered = good.clone();
    tampered.target = SimulatorTargetValue::ActivePowerMW(-4.0);
    let reason = nack_reason(transport.deliver(&tampered));
    assert!(reason.contains("dispatch-security:"), "the refusal is a named security NACK");
    assert!(reason.contains("signature_refused"));
    assert!(reason.contains("integrity check failed"), "the refusal names the tamper");
    assert_eq!(inner.delivered().len(), 1, "the tampered envelope never reached the simulator");
    assert_eq!(
        transport.last_accepted_sequence(&asset_id(4)),
        Some(7),
        "a refused envelope never advances the sequence window"
    );
}

#[test]
fn the_verifying_transport_refuses_missing_wrong_key_and_unsequenced_envelopes() {
    let (transport, inner) = verifying_transport();

    // Missing signature.
    let mut missing = signed_envelope(51, 4, 8, 1.5);
    missing.signature_reference = "   ".to_string();
    let reason = nack_reason(transport.deliver(&missing));
    assert!(reason.contains("carries no signature"));
    assert!(inner.delivered().is_empty(), "an unsigned envelope never reaches the simulator");

    // Wrong key: signed by a different simulation key.
    let wrong = {
        let signer = other_keyring();
        let mut envelope = signed_envelope(52, 4, 8, 1.5);
        envelope.signature_reference =
            sign_dispatch_payload(&signer, &DispatchPayloadBinding::from_envelope(&envelope));
        envelope
    };
    let reason = nack_reason(transport.deliver(&wrong));
    assert!(reason.contains("is not the verifying key's"));
    assert!(inner.delivered().is_empty());

    // Unsequenced (sequence 0).
    let unsequenced = signed_envelope(53, 4, 0, 1.5);
    let reason = nack_reason(transport.deliver(&unsequenced));
    assert!(reason.contains("unsequenced"));
    assert!(inner.delivered().is_empty());

    // Malformed (a signature reference that is not the recorded scheme).
    let mut malformed = signed_envelope(54, 4, 8, 1.5);
    malformed.signature_reference = "not-a-signature".to_string();
    let reason = nack_reason(transport.deliver(&malformed));
    assert!(reason.contains("malformed"));
    assert!(inner.delivered().is_empty());
}

#[test]
fn the_verifying_transport_refuses_replayed_and_out_of_order_sequences() {
    let (transport, inner) = verifying_transport();

    // seq 7 accepted; the REPLAYED seq 7 is a NACK, the inner transport is
    // called exactly once.
    let first = signed_envelope(55, 4, 7, 1.5);
    let replay = first.clone();
    assert!(matches!(
        transport.deliver(&first).expect("the first delivery"),
        SimulatedAcceptance::Accepted { .. }
    ));
    let reason = nack_reason(transport.deliver(&replay));
    assert!(reason.contains("replayed_sequence"), "the refusal names the replay: {reason}");
    assert_eq!(inner.delivered().len(), 1);

    // OUT OF ORDER: seq 6 after 7 is refused (never "late but fine").
    let out_of_order = signed_envelope(56, 4, 6, 1.5);
    let reason = nack_reason(transport.deliver(&out_of_order));
    assert!(reason.contains("out_of_order_sequence"), "the refusal names the order fault: {reason}");
    assert_eq!(inner.delivered().len(), 1, "the out-of-order envelope never reached the simulator");

    // The window never moved: the next strictly greater sequence delivers.
    let next = signed_envelope(57, 4, 8, 1.5);
    assert!(matches!(
        transport.deliver(&next).expect("the strictly greater sequence delivers"),
        SimulatedAcceptance::Accepted { .. }
    ));
    assert_eq!(inner.delivered().len(), 2);
    assert_eq!(transport.last_accepted_sequence(&asset_id(4)), Some(8));
}

// ============================================================================
// Tenant isolation: cross-tenant read / approve / issue denied + audited
// (ledger P4.28 step 3; §16.2; the P4.23 recorded open item)
// ============================================================================

#[test]
fn a_cross_tenant_read_attempt_is_denied_existence_never_leaks_and_the_denial_is_audited() {
    let h = harness(5_000);
    let (command, _record) = walk_to_approved(&h, 60, 4, 17);
    let sink = FakeSecurityAuditSink::new();

    // The READ: a foreign caller's scoped load returns None — a foreign and
    // a missing command are indistinguishable (P0.16 rule 5).
    let view = h
        .store
        .load_command(&other_tenant_context(), command.command_id())
        .expect("the scoped read is a verdict, not an infrastructure failure");
    assert!(view.is_none(), "existence never leaks across the tenant boundary");
    // The command is untouched in its own tenant.
    assert!(h
        .store
        .load_command(&tenant_context(), command.command_id())
        .expect("the owner tenant reads its own command")
        .is_some());

    // The denial is audited (§16.2) with the caller and owner tenants named.
    let denial = record_tenant_denial(
        &other_tenant_context(),
        &command,
        TenantDenialKind::Read,
        5_000,
    );
    assert_eq!(denial.caller_tenant_id, other_tenant_context().tenant_id());
    assert_eq!(denial.owner_tenant_id, command.scope().tenant_id());
    assert_ne!(denial.caller_tenant_id, denial.owner_tenant_id);
    assert_eq!(denial.kind, TenantDenialKind::Read);
    assert!(denial.denial_reference.starts_with("tenant-denial:command/"));
    assert!(denial.denial_reference.ends_with("/read"));
    sink.append_tenant_denial(&denial).expect("the denial is appended");
    assert_eq!(sink.records().len(), 1);
}

#[test]
fn a_cross_tenant_approval_attempt_is_denied_and_audited() {
    let h = harness(5_000);
    let (command, _record) = walk_to_approved(&h, 61, 4, 17);
    let sink = FakeSecurityAuditSink::new();

    // The APPROVE: the tenant-scoped store seam refuses before any state
    // check — the named `NotFound` shape.
    let request = CommandAuthorizationRequest {
        subject: APPROVER.to_string(),
        role: APPROVER_ROLE.to_string(),
        action: CommandAuthzAction::Approve,
        scope: ApprovalScope::Asset(asset_id(4)),
    };
    let error = h
        .gates
        .record_command_approval(
            &other_tenant_context(),
            &command,
            &request,
            "limits:+-2.0 MW",
            ValidUntil::from_unix_millis(100_000).unwrap(),
        )
        .expect_err("a cross-tenant approval attempt is denied");
    assert!(
        error.to_string().contains("does not exist in this tenant scope"),
        "the denial is the named, audit-ready refusal: {error}"
    );
    // The command was NOT approved: still APPROVED (its state before the
    // attempt), no foreign journal row.
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);

    sink.append_tenant_denial(&record_tenant_denial(
        &other_tenant_context(),
        &command,
        TenantDenialKind::Approve,
        5_000,
    ))
    .expect("the denial is appended");
    assert_eq!(sink.records().len(), 1);
    assert_eq!(sink.records()[0].kind, TenantDenialKind::Approve);
}

#[test]
fn a_cross_tenant_issue_attempt_is_denied_and_audited() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 62, 4, 17);
    let sink = FakeSecurityAuditSink::new();
    let adapter = SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: SIMULATOR_ID.to_string(),
        },
        h.transport,
        test_keyring(),
        TrackingAcceptanceCriterion::new(
            TrackedQuantity::ActivePowerMW,
            0.2,
            1,
            5_000,
        )
        .expect("a valid criterion"),
    )
    .expect("the simulator target is the one accepted candidate");

    // The ISSUE: the adapter loads through the tenant-scoped seam — the
    // foreign command does not exist there.
    let error = adapter
        .issue(&other_tenant_context(), &command, &decision_record, &actor(DISPATCHER))
        .expect_err("a cross-tenant issue attempt is denied");
    assert!(
        matches!(error, DispatchAdapterError::NotFound { .. }),
        "existence never leaks: {error}"
    );
    // Nothing was issued, nothing reached the transport, the command is
    // untouched in its own tenant.
    assert!(h.transport.delivered().is_empty());
    assert_eq!(h.store.current_state(command.command_id()), CommandState::Approved);

    sink.append_tenant_denial(&record_tenant_denial(
        &other_tenant_context(),
        &command,
        TenantDenialKind::Issue,
        5_000,
    ))
    .expect("the denial is appended");
    let records = sink.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, TenantDenialKind::Issue);
    assert_eq!(records[0].command_id, command.command_id());
    assert_eq!(records[0].asset_id, command.scope().asset_id());

    // The audit text is stable, identifier-only, and never caller-visible
    // output (the caller saw only the named NotFound).
    let text = records[0].audit_text();
    assert!(text.contains("tenant_denial kind=issue"));
    assert!(text.contains(&command.command_id().to_string()));
    assert!(text.contains(&command.scope().asset_id().to_string()));
}

// ============================================================================
// Secrets: no credential-shaped material anywhere (ledger P4.28 step 4)
// ============================================================================

#[test]
fn payloads_signatures_transports_and_audit_texts_hold_no_credential_shaped_material() {
    const FORBIDDEN: [&str; 11] = [
        "password",
        "passwd",
        "secret=",
        "token:",
        "bearer ",
        "begin private key",
        "begin rsa",
        "api_key",
        "apikey",
        "private_key",
        "sk_live",
    ];

    let keyring = test_keyring();
    // The fixture surfaces an attacker would inspect: the command payload,
    // the signature reference, the transport identity, the audit texts.
    let (command, _record) = {
        let h = harness(5_000);
        walk_to_approved(&h, 70, 4, 17)
    };
    let envelope = signed_envelope(71, 4, 17, 1.5);
    let denial = record_tenant_denial(
        &other_tenant_context(),
        &command,
        TenantDenialKind::Read,
        5_000,
    );

    let surfaces: Vec<(&str, String)> = vec![
        // The §14.3 command payload exactly as proposed (the fixture JSON).
        ("command payload", propose_input(70, 4, 17).command_payload_json),
        (
            "signature reference",
            sign_dispatch_payload(&keyring, &DispatchPayloadBinding::from_envelope(&envelope)),
        ),
        ("transport identity", envelope.transport_identity.clone()),
        ("sequence refusal text", {
            let mut guard = SequenceGuard::new();
            let _ = guard.admit(&channel_key(&asset_id(4), DEFAULT_DISPATCH_CHANNEL), 5);
            format!(
                "{:?}",
                guard.admit(&channel_key(&asset_id(4), DEFAULT_DISPATCH_CHANNEL), 5)
            )
        }),
        ("tenant denial audit text", denial.audit_text()),
        ("tenant denial reference", denial.denial_reference.clone()),
    ];

    for (surface, text) in &surfaces {
        let lowered = text.to_lowercase();
        for marker in FORBIDDEN {
            assert!(
                !lowered.contains(marker),
                "credential-shaped material '{marker}' found on the {surface} surface"
            );
        }
    }

    // The test keys are generated IN-TEST and clearly labelled
    // non-production simulation keys (identifiers, never credentials).
    for key_id in [SIM_KEY_ID, OTHER_KEY_ID] {
        assert!(key_id.starts_with("sim-key-"), "simulation keys carry the sim-key- label");
        assert!(key_id.contains("nonproduction"), "the label states non-production: {key_id}");
    }
}

#[test]
fn the_keyring_layers_hold_no_key_material() {
    // Structural: the P4.16 keyring (extended, not edited, by P4.28) and
    // this module hold key IDENTIFIERS only — no bytes, no material.
    let adapter_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/dispatch_simulator.rs"
    ))
    .expect("the adapter source is readable");
    let marker = "pub struct SimulationSigningKey";
    let start = adapter_source.find(marker).expect("the signing key struct is declared");
    let end = start + adapter_source[start..].find('}').expect("the struct block closes");
    let block = &adapter_source[start..end];
    assert!(
        block.contains("pub key_id: String"),
        "the signing key is an identifier only"
    );
    // "pub " appears exactly twice: the struct declaration itself and the
    // single key-id field — a material field must not be added.
    assert_eq!(
        block.matches("pub ").count(),
        2,
        "the signing key struct has EXACTLY one public field (the key id); a material \
         field must not be added"
    );

    let security_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/dispatch_security.rs"
    ))
    .expect("the security module source is readable");
    for forbidden in ["private_key", "secret_key", "Vec<u8>", "seed:", "entropy"] {
        assert!(
            !security_source.contains(forbidden),
            "forbidden key-material identifier '{forbidden}' found in dispatch_security.rs"
        );
    }
}

// ============================================================================
// Determinism: the attack surface run twice yields identical outcomes
// ============================================================================

#[test]
fn the_attack_surface_is_deterministic_across_two_runs() {
    fn run_once() -> Vec<String> {
        let keyring = test_keyring();
        let original = signed_envelope(80, 4, 7, 1.5);
        let mut outcomes: Vec<String> = Vec::new();

        // Control verify.
        outcomes.push(format!(
            "{:?}",
            verify_dispatch_payload(
                &keyring,
                &DispatchPayloadBinding::from_envelope(&original),
                &original.signature_reference,
            )
            .is_ok()
        ));
        // Tampered value.
        let mut tampered = original.clone();
        tampered.target = SimulatorTargetValue::ActivePowerMW(9.9);
        outcomes.push(format!(
            "{:?}",
            verify_dispatch_payload(
                &keyring,
                &DispatchPayloadBinding::from_envelope(&tampered),
                &original.signature_reference,
            )
        ));
        // Missing signature.
        outcomes.push(format!(
            "{:?}",
            verify_dispatch_payload(&keyring, &DispatchPayloadBinding::from_envelope(&original), "")
        ));
        // Wrong key.
        outcomes.push(format!(
            "{:?}",
            verify_dispatch_payload(
                &other_keyring(),
                &DispatchPayloadBinding::from_envelope(&original),
                &original.signature_reference,
            )
        ));
        // Altered sequence.
        let mut altered = DispatchPayloadBinding::from_envelope(&original);
        altered.sequence = 9;
        outcomes.push(format!(
            "{:?}",
            verify_dispatch_payload(&keyring, &altered, &original.signature_reference)
        ));
        // Replay + out-of-order at the guard.
        let mut guard = SequenceGuard::new();
        let channel = channel_key(&asset_id(4), DEFAULT_DISPATCH_CHANNEL);
        outcomes.push(format!("{:?}", guard.admit(&channel, 7)));
        outcomes.push(format!("{:?}", guard.admit(&channel, 7)));
        outcomes.push(format!("{:?}", guard.admit(&channel, 3)));
        outcomes
    }

    let first = run_once();
    let second = run_once();
    assert_eq!(first, second, "the same attacks on the same inputs yield the same outcomes");
    // 8 outcomes: control verify (0), tampered (1), missing (2), wrong key
    // (3), altered sequence (4), first admission (5, the sequence control),
    // replay (6), out-of-order (7).
    assert_eq!(first.len(), 8);
    assert_eq!(first[0], "true", "the control verified");
    for outcome in &first[1..5] {
        assert!(outcome.starts_with("Err("), "every signature attack is a named refusal: {outcome}");
    }
    assert!(first[5].starts_with("Ok("), "the first admission is the sequence control");
    for outcome in &first[6..8] {
        assert!(outcome.starts_with("Err("), "replay and out-of-order are named refusals: {outcome}");
    }
}
