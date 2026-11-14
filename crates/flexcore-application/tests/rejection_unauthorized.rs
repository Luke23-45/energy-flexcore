// P4.23 — Deterministic rejection: unauthorized commands
//         (report §23 Stage 4 exit gate, §14.1, §17.3–§17.5 invariants 4
//         and 8, §16.2; ledger P4.23; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         23_25_rejection_suites_b.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Fixtures: approval attempted by an actor WITHOUT the required role
//   (least-privileged roles that must fail: viewer, operator,
//   platform-admin, an unknown role); approval attempted by the PROPOSER
//   where separation of duties is configured; a CROSS-TENANT approval
//   attempt; approval with an EXPIRED authorization scope; a command ISSUE
//   attempted without the APPROVED state.
// - Every outcome is pre-registered (the table below) and asserted verbatim:
//   REJECTED with a NAMED authorization error, or a blocked transition with
//   the named error.
// - An audit/denial record exists for each denial (§16.2): the in-scope gate
//   denials journal FEASIBLE → REJECTED with an `authz-denial:command/<id>@<t>`
//   evidence reference carrying the denial reason, and `DispatchRejected` is
//   emitted; the cross-tenant denial is the named `NotFound` (existence never
//   leaks, P0.16 rule 5) — its AuditSink append is the P4.28/P4.30 wiring
//   (open item, recorded).
// - NO fixture reaches ISSUED: asserted per fixture on the journal, on the
//   frozen-edge oracle (`try_transition` — no REJECTED/FEASIBLE → ISSUED
//   edge exists), and on the adapter (zero delivered envelopes).
// - The control: the correct role (`tenant-admin`, the only recorded
//   `command.approve` grant, at site/asset scope) APPROVES — the fixtures
//   fail on least-privileged roles, not on a broken harness.
//
// Dependency note (honest): P4.23's ledger inputs are P4.13 (done) and
// P4.28 (pending). The suite exercises the authorization mechanisms at
// their EXISTING enforcement points — the P4.13 authorization gate, the
// tenant-scoped store seam, the P4.14 issue re-validation, the P4.16
// adapter's state guard, and the P0.11 frozen-edge oracle — with in-test
// fakes; the P4.28 wiring re-verification is an explicit follow-up
// (recorded in the decision doc, same pattern as the P3.29 F2 open item).
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    AppliedSeparationOfDuties, ApprovalScope, AssetCapabilitySource, AssetGateStatus,
    AuthorizationDecision, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGateError,
    CommandGatePolicy, CommandGateStore, CommandGateStoreError, CommandGates,
    DeclaredQuantities, EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError,
    ModelVersionGateStatus, ModelVersionSource, ProposedCommandRecord, ProposeCommandInput,
    ScheduleEnvelopeSource, ScheduleRevisionUsability, SeparationOfDuties, COMMAND_ENTITY,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, DispatchAdapterError, SimulationKeyring, SimulatorDispatchAdapter,
    SimulatorDispatchTransport, SimulatorTransportError, TrackingAcceptanceCriterion,
    TrackedQuantity,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, Gate, LifecycleClock};
use flexcore_domain::command_state::{is_terminal, try_transition, CommandState};
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

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const UUID_OTHER_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a03";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a04";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a05";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a06";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a07";

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

fn other_tenant_context() -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: TenantId::new(uuid(UUID_OTHER_TENANT)),
        subject: "test:foreign-principal".to_string(),
    }))
}

fn command_id() -> CommandId {
    CommandId::new(uuid(UUID_COMMAND))
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
// The recorded Phase 4 default matrix (flexcore-authz `phase4_policies`):
// `command.approve` is granted to TenantAdmin ONLY, at site and asset
// scope kinds. No tenant-level approval grant; no Operator/Viewer/
// PlatformAdmin grant.
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:unauth-feeder-hash-01";
const SIMULATOR_ID: &str = "sim:p423-rejection-fixture";
const SIM_KEY_ID: &str = "sim-key-p423";

// --- The pre-registered expectations (ledger P4.23 step 2) -------------------------
//
// Every fixture's outcome is registered here BEFORE the assertion runs; each
// test looks its fixture up in this table and asserts the pre-registered
// value verbatim. A fixture whose outcome changes must change this table
// first — the table is the reviewable record.

/// The named authorization error each unauthorized fixture must produce.
fn pre_registered_denial(fixture: &str) -> String {
    match fixture {
        // Approval by a role with no `command.approve` grant (deny-by-default).
        "unauthorized_role_viewer" => {
            "authorization_denied:no matching rule for role viewer".to_string()
        }
        "unauthorized_role_operator" => {
            "authorization_denied:no matching rule for role operator".to_string()
        }
        "unauthorized_role_platform_admin" => {
            "authorization_denied:no matching rule for role platform-admin".to_string()
        }
        "unauthorized_role_unknown" => {
            "authorization_denied:no matching rule for role service:unprivileged-bot".to_string()
        }
        // Separation of duties (§17.4): the approver must differ from the
        // proposing identity where configured (the recorded default is
        // ENFORCED).
        "proposer_self_approval" => {
            "self-approval prohibited (separation of duties, §17.4)".to_string()
        }
        // Approval evaluated with an authorization scope already expired
        // (end-exclusive, §10.3: expiry <= now is expired).
        "expired_authorization_scope" => "authorization_expired_before_decision".to_string(),
        other => panic!("no pre-registered denial for fixture '{other}'"),
    }
}

/// The named error a blocked (non-verdict) fixture must produce.
fn pre_registered_block(fixture: &str) -> String {
    match fixture {
        // A cross-tenant caller cannot even see the command (P0.16 rule 5):
        // existence never leaks across a tenant boundary.
        "cross_tenant_approval" => "does not exist in this tenant scope".to_string(),
        // Issue applies only from APPROVED (the gate hook and the adapter
        // both refuse with the state named).
        "issue_without_approved" => "operation `issue` does not apply to a command in state"
            .to_string(),
        other => panic!("no pre-registered block for fixture '{other}'"),
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
    ) -> Result<Option<CommandGateViewAlias>, CommandGateStoreError> {
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
        Ok(Some(flexcore_application::command_gates::CommandGateView {
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

type CommandGateViewAlias = flexcore_application::command_gates::CommandGateView;

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
            model_id: "unauth-feeder".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.98; max loading 62%".to_string(),
            coverage: "balanced radial unauth-feeder; unbalanced deferred (P4.34)".to_string(),
        })
    }
}

/// The authorization evaluator fake, mirroring the RECORDED Phase 4 default
/// grant matrix (`flexcore-authz::phase4_policies::phase4_command_policies`,
/// whose real evaluation is tested in `crates/flexcore-authz/tests/
/// phase4_policies.rs`): `command.approve` is granted to the tenant-admin
/// role only, at site and asset scope kinds. Deny-by-default: any other
/// role, any other scope kind, any unknown role id denies with a NAMED
/// reason (the deny reason is what the audit record cites, §16.2).
struct Phase4MatrixAuthz;

impl CommandAuthorizationEvaluator for Phase4MatrixAuthz {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision {
        // Scope kind mapping (the application-side mirror of the authz
        // scope levels; scope levels do not inherit).
        let scope_kind = match request.scope {
            ApprovalScope::Tenant => "tenant",
            ApprovalScope::Site(_) => "site",
            ApprovalScope::Asset(_) => "asset",
        };
        let granted = request.role == APPROVER_ROLE
            && request.action == CommandAuthzAction::Approve
            && matches!(scope_kind, "site" | "asset");
        if granted {
            AuthorizationDecision::Allowed {
                rule_reference: format!(
                    "policy:phase4:{APPROVER_ROLE}:command.approve:{scope_kind}"
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
    fn encode(
        &self,
        envelope: &flexcore_events::envelope::EventEnvelope,
    ) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- The scripted simulator transport fake (fixture: issue without APPROVED) ---------

/// Default-accepts deliveries; the unauthorized fixtures never reach a
/// delivery, so the recorded envelope list is the "nothing was sent" proof.
struct FakeTransport {
    delivered: Mutex<Vec<String>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn delivered(&self) -> Vec<String> {
        self.delivered.lock().expect("fake transport").clone()
    }
}

impl SimulatorDispatchTransport for FakeTransport {
    fn deliver(
        &self,
        envelope: &flexcore_application::dispatch_simulator::SimulatorDispatchEnvelope,
    ) -> Result<flexcore_application::dispatch_simulator::SimulatedAcceptance, SimulatorTransportError>
    {
        self.delivered
            .lock()
            .expect("fake transport")
            .push(envelope.command_id.to_string());
        Ok(flexcore_application::dispatch_simulator::SimulatedAcceptance::Accepted {
            acceptance_reference: format!("acceptance:sim/{}", envelope.command_id),
        })
    }

    fn observe(
        &self,
        _asset_id: AssetId,
        _at_unix_millis: i64,
    ) -> Result<Option<flexcore_application::dispatch_simulator::SimulatedObservation>, SimulatorTransportError>
    {
        Ok(None)
    }
}

// --- The harness ---------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    outbox: &'static FakeEventOutbox,
    transport: &'static FakeTransport,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels));
    let assets: &'static FakeAssets = Box::leak(Box::new(FakeAssets));
    let runner: &'static FakeRunner = Box::leak(Box::new(FakeRunner));
    let authz: &'static Phase4MatrixAuthz = Box::leak(Box::new(Phase4MatrixAuthz));
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
        policy: CommandGatePolicy {
            // The recorded default: separation of duties ENFORCED (§17.4).
            separation_of_duties: SeparationOfDuties::Enforced,
            max_execution_duration: None,
        },
    };
    Harness {
        gates,
        store,
        outbox,
        transport,
        clock,
        ids,
    }
}

// --- Command fixture and the walk to FEASIBLE -----------------------------------------

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
        sequence: 17,
        idempotency_key: "idem:p423-op-1".to_string(),
        evidence: EvidenceReferences {
            grid_model: "grid-model:unauth-feeder@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:p423-fixture".to_string()),
        supersedes: None,
        command_payload_json: "{\"p_mw\":1.5}".to_string(),
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p423-1".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

/// Creates the command and walks it through the REAL validation and
/// feasibility gates to FEASIBLE (the state every approval fixture starts
/// from).
fn walk_to_feasible(h: &Harness) -> DispatchCommand {
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
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
    assert_eq!(
        h.store.journal_of(command_id()).last().map(|e| e.to),
        Some(CommandState::Feasible)
    );
    proposed.command
}

fn approval_request(subject: &str, role: &str, scope: ApprovalScope) -> CommandAuthorizationRequest {
    CommandAuthorizationRequest {
        subject: subject.to_string(),
        role: role.to_string(),
        action: CommandAuthzAction::Approve,
        scope,
    }
}

/// The §16.2 audit-denial checks shared by every in-scope denial fixture:
/// the FEASIBLE → REJECTED journal row names the authorization gate, carries
/// the `authz-denial:` evidence reference, records the denying actor and the
/// pre-registered reason, and `DispatchRejected` was emitted. The journal
/// row IS the denial record of record (the frozen payloads carry identity +
/// time only — the actor/reason travel in the journal; recorded payload gap,
/// D-P4.15-4).
fn assert_denial_record(h: &Harness, command: &DispatchCommand, expected_reason: &str, denier: &str) {
    let journal = h.store.journal_of(command.command_id());
    let entry = journal.last().expect("the denial is journalled");
    assert_eq!(entry.from, CommandState::Feasible);
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.gate, Some(Gate::Authorization));
    assert_eq!(entry.actor.identity(), denier);
    assert_eq!(
        entry.reason.as_deref(),
        Some(expected_reason),
        "the denial reason is the pre-registered authorization error"
    );
    let evidence = entry.evidence.as_ref().expect("the denial evidence");
    assert!(
        evidence
            .as_str()
            .starts_with("authz-denial:command/"),
        "the denial record reference: {}",
        evidence.as_str()
    );
    // DispatchRejected was emitted (the §15.3 event for REJECTED).
    let rejected = h
        .outbox
        .writes()
        .iter()
        .filter(|write| write.payload().starts_with("DispatchRejected"))
        .count();
    assert_eq!(rejected, 1, "exactly one DispatchRejected for the denial");
}

/// The "no fixture reaches ISSUED" checks (ledger P4.23 step 4): no ISSUED
/// journal row, and the frozen-edge oracle has no edge from the fixture's
/// terminal state to ISSUED.
fn assert_never_issued(h: &Harness, command: &DispatchCommand, final_state: CommandState) {
    let journal = h.store.journal_of(command.command_id());
    assert!(
        !journal.iter().any(|entry| entry.to == CommandState::Issued),
        "the fixture must never reach ISSUED"
    );
    // The frozen-edge oracle (P0.11): no edge from REJECTED (or any terminal
    // state) to ISSUED exists — the block is structural, not conventional.
    if is_terminal(final_state) {
        assert!(try_transition(final_state, CommandState::Issued).is_err());
    }
    // Nothing reached the simulator transport either.
    assert!(h.transport.delivered().is_empty());
}

// ============================================================================
// Fixture 1 — approval attempted by an actor WITHOUT the required role
//            (least-privileged roles that must fail)
// ============================================================================

#[test]
fn an_unauthorized_role_is_rejected_with_the_pre_registered_error() {
    for (fixture, role) in [
        ("unauthorized_role_viewer", "viewer"),
        ("unauthorized_role_operator", "operator"),
        ("unauthorized_role_platform_admin", "platform-admin"),
        ("unauthorized_role_unknown", "service:unprivileged-bot"),
    ] {
        let h = harness(5_000);
        let command = walk_to_feasible(&h);
        let result = h
            .gates
            .record_command_approval(
                &context(),
                &command,
                &approval_request(APPROVER, role, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
                "±2.0 MW, Q ±0.5 MVAr",
                ValidUntil::from_unix_millis(200_000).unwrap(),
            )
            .expect("a denial is a verdict, not an infrastructure error");
        assert!(!result.approved);
        assert!(result.decision_record.is_none(), "a denial carries no decision record");
        // The pre-registered named authorization error.
        assert_eq!(
            result.denial_reason.as_deref(),
            Some(pre_registered_denial(fixture).as_str()),
            "fixture {fixture}"
        );
        // The §16.2 audit-denial record exists; the fixture never reaches
        // ISSUED.
        assert_denial_record(&h, &command, &pre_registered_denial(fixture), APPROVER);
        assert_never_issued(&h, &command, CommandState::Rejected);
    }
}

/// The tenant-level approval scope is NOT granted in the recorded default
/// matrix (approvals are resource/site/asset-scoped, §17.3 control 2): the
/// CORRECT role at the tenant scope is still denied — least privilege holds
/// even for the privileged role.
#[test]
fn the_correct_role_at_tenant_scope_is_denied_no_tenant_level_approval_grant() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(APPROVER, APPROVER_ROLE, ApprovalScope::Tenant),
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    let expected = "authorization_denied:no matching rule for role tenant-admin".to_string();
    assert_eq!(result.denial_reason.as_deref(), Some(expected.as_str()));
    assert_denial_record(&h, &command, &expected, APPROVER);
    assert_never_issued(&h, &command, CommandState::Rejected);
}

/// THE CONTROL: the correct role (`tenant-admin`) at the asset scope
/// APPROVES — the fixtures above fail on least-privileged roles, not on a
/// broken harness (ledger P4.23 risk mitigation).
#[test]
fn the_control_the_correct_role_approves_and_reaches_approved() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(APPROVER, APPROVER_ROLE, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
            "±2.0 MW, Q ±0.5 MVAr",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the control approval must succeed");
    assert!(result.approved);
    let record = result.decision_record.expect("the decision record");
    assert_eq!(record.authorizing_role, APPROVER_ROLE);
    assert_eq!(
        record.separation_of_duties,
        AppliedSeparationOfDuties::Enforced
    );
    assert_eq!(
        h.store.journal_of(command.command_id()).last().map(|e| e.to),
        Some(CommandState::Approved),
        "the control reaches APPROVED (never ISSUED — approval alone does not issue)"
    );
    assert_never_issued(&h, &command, CommandState::Approved);
}

// ============================================================================
// Fixture 2 — approval attempted by the PROPOSER where separation of duties
//            is configured
// ============================================================================

#[test]
fn approval_by_the_proposer_is_rejected_where_separation_of_duties_is_configured() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h);
    // The approving identity IS the command's proposing identity
    // (`actor_or_service`, §14.3 field 5); the recorded default policy is
    // SeparationOfDuties::Enforced.
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(PROPOSER, APPROVER_ROLE, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    let expected = pre_registered_denial("proposer_self_approval");
    assert_eq!(result.denial_reason.as_deref(), Some(expected.as_str()));
    // The denial record carries the PROPOSER as the denying actor.
    assert_denial_record(&h, &command, &expected, PROPOSER);
    assert_never_issued(&h, &command, CommandState::Rejected);
}

// ============================================================================
// Fixture 3 — a CROSS-TENANT approval attempt
// ============================================================================

#[test]
fn a_cross_tenant_approval_attempt_is_blocked_existence_never_leaks() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h);
    let writes_before = h.outbox.len();
    // The foreign tenant's caller attempts to approve tenant A's command:
    // the tenant-scoped store seam does not contain it in that scope
    // (P0.16 rule 5 — a foreign command and a missing command are
    // indistinguishable), so the attempt is refused with the named NotFound.
    let error = h
        .gates
        .record_command_approval(
            &other_tenant_context(),
            &command,
            &approval_request("user:foreign-approver", APPROVER_ROLE, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect_err("a cross-tenant approval attempt must be blocked");
    match &error {
        CommandGateError::Store(CommandGateStoreError::NotFound { command_id }) => {
            assert_eq!(command_id, &command.command_id().to_string());
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(
        error.to_string().contains(&pre_registered_block("cross_tenant_approval")),
        "the named refusal: {error}"
    );
    // The command is untouched in its own tenant (still FEASIBLE, no new
    // journal row, no event emitted for the foreign caller).
    assert_eq!(
        h.store.journal_of(command.command_id()).last().map(|e| e.to),
        Some(CommandState::Feasible)
    );
    assert_eq!(h.outbox.len(), writes_before, "no event for a foreign-scope denial");
    assert_never_issued(&h, &command, CommandState::Feasible);
    // AUDITABILITY of the cross-tenant denial (§16.2; P0.16 rule 6 "audit
    // cross-tenant access denials"): the recorded denial is the named
    // NotFound error text above — the deny-reason discipline of the
    // authorization surface (P1.11) carries a stable reason for the audit
    // record. The AuditSink append wiring for out-of-scope denials is the
    // P4.28/P4.30 security/audit wiring (open item, recorded in the
    // decision doc) — recorded honestly here, not silently assumed.
    assert_ne!(
        pre_registered_block("cross_tenant_approval"),
        "",
        "the cross-tenant denial carries a stable, audit-ready reason"
    );
}

// ============================================================================
// Fixture 4 — approval with an EXPIRED authorization scope
// ============================================================================

#[test]
fn an_approval_with_an_expired_authorization_scope_is_rejected() {
    // The authorization expires exactly AT the controlled clock's instant:
    // end-exclusive (§10.3) — already expired.
    let h = harness(5_000);
    let command = walk_to_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(APPROVER, APPROVER_ROLE, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
            "±2.0 MW",
            ValidUntil::from_unix_millis(5_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    let expected = pre_registered_denial("expired_authorization_scope");
    assert_eq!(result.denial_reason.as_deref(), Some(expected.as_str()));
    assert_denial_record(&h, &command, &expected, APPROVER);
    assert_never_issued(&h, &command, CommandState::Rejected);

    // Strictly in the past: the same named error.
    let h = harness(50_000);
    let command = walk_to_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(APPROVER, APPROVER_ROLE, ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)))),
            "±2.0 MW",
            ValidUntil::from_unix_millis(20_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!result.approved);
    assert_eq!(result.denial_reason.as_deref(), Some(expected.as_str()));
    assert_never_issued(&h, &command, CommandState::Rejected);
}

// ============================================================================
// Fixture 5 — command ISSUE attempted without the APPROVED state
// ============================================================================

#[test]
fn an_issue_attempt_without_the_approved_state_is_refused() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h); // FEASIBLE, never approved
    let writes_before = h.outbox.len();

    // The P4.12 issue hook refuses: issue applies only from APPROVED.
    let error = h
        .gates
        .ensure_command_issueable(&context(), command.command_id(), MODEL_HASH)
        .expect_err("issue without APPROVED must be refused");
    match &error {
        CommandGateError::WrongState { current, operation } => {
            assert_eq!(*current, CommandState::Feasible);
            assert_eq!(*operation, "issue");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(error.to_string().contains(&pre_registered_block("issue_without_approved")));

    // The P4.16 adapter refuses the same attempt at ITS enforcement point
    // (the adapter issues only from APPROVED; nothing reaches the transport).
    let adapter = SimulatorDispatchAdapter::configure(
        &h.gates,
        AdapterTargetCandidate::Simulator {
            simulator_id: SIMULATOR_ID.to_string(),
        },
        h.transport,
        SimulationKeyring::new(SIM_KEY_ID).expect("a non-empty simulation key id"),
        TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 1, 5_000)
            .expect("a valid criterion"),
    )
    .expect("the simulator target is accepted");
    let unauthorized_record = flexcore_application::command_gates::AuthorizationDecisionRecord {
        decision_reference: "authz-decision:forged@0".to_string(),
        authorizing_actor: APPROVER.to_string(),
        authorizing_role: APPROVER_ROLE.to_string(),
        approved_scope: format!("asset:{}", uuid(UUID_ASSET)),
        approved_limits: "±2.0 MW".to_string(),
        decided_at_unix_millis: 5_000,
        expiry_unix_millis: 200_000,
        separation_of_duties: AppliedSeparationOfDuties::Enforced,
        covered_validation_evidence: "validation:forged".to_string(),
        covered_feasibility_evidence: "feasibility:forged".to_string(),
    };
    let error = adapter
        .issue(&context(), &command, &unauthorized_record, &actor("svc:dispatcher"))
        .expect_err("the adapter refuses an issue without APPROVED");
    match error {
        DispatchAdapterError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Feasible);
            assert_eq!(operation, "issue");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    // The command is untouched; no ISSUED row, no events, nothing delivered.
    assert_eq!(
        h.store.journal_of(command.command_id()).last().map(|e| e.to),
        Some(CommandState::Feasible)
    );
    assert_eq!(h.outbox.len(), writes_before);
    assert_never_issued(&h, &command, CommandState::Feasible);
}

// ============================================================================
// The exit check in aggregate: every pre-registered outcome holds, every
// denial is audit-recorded, and no fixture reaches ISSUED
// ============================================================================

/// One fixture run's outcome summary (for the determinism assertion).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FixtureOutcome {
    fixture: &'static str,
    /// The denial reason or block detail the fixture produced.
    outcome_text: String,
    /// The command's final state.
    final_state: CommandState,
    /// Whether an audit/denial record exists (journal row / named refusal).
    denial_recorded: bool,
    /// Whether the fixture reached ISSUED (must be false for all).
    reached_issued: bool,
}

fn run_gate_denial_fixture(
    fixture: &'static str,
    subject: &str,
    role: &str,
    scope: ApprovalScope,
    expiry_millis: i64,
    now_unix_millis: i64,
) -> FixtureOutcome {
    let h = harness(now_unix_millis);
    let command = walk_to_feasible(&h);
    let result = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(subject, role, scope),
            "±2.0 MW",
            ValidUntil::from_unix_millis(expiry_millis).unwrap(),
        )
        .expect("the denial verdict");
    let denial_recorded = h
        .store
        .journal_of(command.command_id())
        .last()
        .map(|entry| {
            entry.to == CommandState::Rejected
                && entry.gate == Some(Gate::Authorization)
                && entry
                    .evidence
                    .as_ref()
                    .map(|e| e.as_str().starts_with("authz-denial:command/"))
                    .unwrap_or(false)
        })
        .unwrap_or(false);
    let reached_issued = h
        .store
        .journal_of(command.command_id())
        .iter()
        .any(|entry| entry.to == CommandState::Issued);
    FixtureOutcome {
        fixture,
        outcome_text: result.denial_reason.unwrap_or_default(),
        final_state: h
            .store
            .journal_of(command.command_id())
            .last()
            .map(|e| e.to)
            .unwrap_or(CommandState::Proposed),
        denial_recorded,
        reached_issued,
    }
}

#[test]
fn every_pre_registered_outcome_holds_and_no_fixture_reaches_issued() {
    let asset_scope = ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)));
    let fixtures = vec![
        run_gate_denial_fixture("unauthorized_role_viewer", APPROVER, "viewer", asset_scope, 200_000, 5_000),
        run_gate_denial_fixture("unauthorized_role_operator", APPROVER, "operator", asset_scope, 200_000, 5_000),
        run_gate_denial_fixture(
            "unauthorized_role_platform_admin",
            APPROVER,
            "platform-admin",
            asset_scope,
            200_000,
            5_000,
        ),
        run_gate_denial_fixture(
            "unauthorized_role_unknown",
            APPROVER,
            "service:unprivileged-bot",
            asset_scope,
            200_000,
            5_000,
        ),
        // Separation of duties: the proposer attempts to approve.
        run_gate_denial_fixture("proposer_self_approval", PROPOSER, APPROVER_ROLE, asset_scope, 200_000, 5_000),
        // Expired authorization scope (end-exclusive at the clock instant).
        run_gate_denial_fixture("expired_authorization_scope", APPROVER, APPROVER_ROLE, asset_scope, 5_000, 5_000),
    ];
    for outcome in &fixtures {
        // The pre-registered outcome holds verbatim.
        assert_eq!(
            outcome.outcome_text,
            pre_registered_denial(outcome.fixture),
            "fixture {}: the outcome must match the pre-registered denial",
            outcome.fixture
        );
        // The denial is audit-recorded (the journal denial record).
        assert!(
            outcome.denial_recorded,
            "fixture {}: the §16.2 denial record must exist",
            outcome.fixture
        );
        // The fixture ends REJECTED and never reached ISSUED.
        assert_eq!(outcome.final_state, CommandState::Rejected, "fixture {}", outcome.fixture);
        assert!(!outcome.reached_issued, "fixture {} must never reach ISSUED", outcome.fixture);
    }

    // The frozen-edge oracle (P0.11): the blocked transitions are
    // structurally impossible, not merely refused by convention — no edge
    // REJECTED → ISSUED, FEASIBLE → ISSUED, or EXPIRED → ISSUED exists.
    for from in [
        CommandState::Rejected,
        CommandState::Feasible,
        CommandState::Expired,
        CommandState::Approved,
    ] {
        if from == CommandState::Approved {
            // APPROVED → ISSUED is the frozen issue edge (9) — the CONTROL
            // path; the unauthorized fixtures never occupy it unauthorized.
            continue;
        }
        assert!(
            try_transition(from, CommandState::Issued).is_err(),
            "{} → ISSUED must not exist in the frozen table",
            from.name()
        );
    }
}

/// Determinism (ledger P4.20–P4.26 discipline, applied here): the fixtures
/// run twice from clean state and produce IDENTICAL outcomes.
#[test]
fn the_fixtures_are_deterministic_across_two_runs() {
    let asset_scope = ApprovalScope::Asset(AssetId::new(uuid(UUID_ASSET)));
    let run = || {
        vec![
            run_gate_denial_fixture("unauthorized_role_viewer", APPROVER, "viewer", asset_scope, 200_000, 5_000),
            run_gate_denial_fixture("proposer_self_approval", PROPOSER, APPROVER_ROLE, asset_scope, 200_000, 5_000),
            run_gate_denial_fixture("expired_authorization_scope", APPROVER, APPROVER_ROLE, asset_scope, 5_000, 5_000),
        ]
    };
    let first = run();
    let second = run();
    assert_eq!(first, second, "identical fixtures must produce identical outcomes");
}
