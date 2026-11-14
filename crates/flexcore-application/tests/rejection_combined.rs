// P4.26 — Deterministic rejection: combined and adversarial cases
//         (report §23 Stage 4 exit gate, §19.2 layer 6 fault injection,
//         §17.4; ledger P4.26; dependencies P4.20–P4.25 — all done;
//         decision record with the PRE-REGISTERED per-fixture expectation
//         table, written BEFORE the assertions and UNRUN at authoring time:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         26_33_combined_matrix.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Combined fixtures, each carrying TWO real faults simultaneously:
//   F-CM-1 stale + infeasible (real P3.13 max-age staleness + a scripted
//   limit-violated feasibility verdict); F-CM-2 duplicate + unauthorized
//   (the same idempotency key with a conflicting payload + an approval
//   attempted by a role with no grant); F-CM-3 expired + infeasible
//   (validity lapsed at APPROVED + an inactive model behind the issue
//   hook); F-CM-4 forged credential + stale (a forged authorization record
//   + a lapsed window; plus the pure refusal-order check for a forged
//   authenticity string); F-CM-5 replayed event + conflicting payload
//   (a replayed sequence number + a conflicting payload under the same
//   idempotency key, at the pure-verdict layer, the store seam, and the
//   outbox seam).
// - Every fixture's outcome is PRE-REGISTERED and asserted verbatim: the
//   rejection names at least one applicable failing check; where two
//   checks apply, the recorded reason IS one of them — never a pass.
// - The multi-check mitigation (ledger P4.26 risk note) is answered
//   HONESTLY, from the real surfaces: the validation gate is
//   first-failure-wins (one failing check per rejection, §14.2 order —
//   recorded in the decision note after reading `command_gates.rs`); the
//   PURE §14.3 verdict layer DOES evaluate each rule independently, so
//   F-CM-5 asserts BOTH failing checks are named there. No new enforcement
//   code was written (test-only suite).
// - Determinism: every fixture is run TWICE from clean state with
//   identical inputs and the two outcomes are asserted identical; the
//   aggregate exit check asserts NO combined fixture reaches ISSUED, and
//   the frozen-edge oracle (P0.11) pins that no REJECTED/EXPIRED → ISSUED
//   edge exists (the block is structural, not conventional).
// - Adversarial per §17.4: forged credential (the authorization-record
//   cross-check — the enforced forged-credential surface of the current
//   simulation stack; real signature verification is P4.28's, recorded as
//   an open item) and replayed sequence — never issue.
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.
// Simulation-only 0.x (§21.4): nothing here issues anything to anything.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_expiry::{
    CommandExpiryService, EventScope, ExpiryOutcome,
};
use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGateError,
    CommandGatePolicy, CommandGateStore, CommandGateStoreError, CommandGateView, CommandGates,
    DeclaredQuantities, EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError,
    ModelVersionGateStatus, ModelVersionSource, ProposedCommandRecord, ProposeCommandInput,
    ScheduleEnvelopeSource, ScheduleRevisionUsability, SeparationOfDuties, COMMAND_ENTITY,
};
use flexcore_application::command_record::{
    duplicate_conflict_rejection, out_of_sequence_rejection, CommandRejectionRule,
};
use flexcore_application::dispatch_simulator::{
    check_issue_refusals, AdapterTargetCandidate, DispatchAdapterError, IssueRefusal,
    IssueRefusalInput, SimulationKeyring, SimulatedAcceptance, SimulatedObservation,
    SimulatorDispatchAdapter, SimulatorDispatchTransport, SimulatorTransportError,
    TrackingAcceptanceCriterion, TrackedQuantity,
};
use flexcore_application::envelope_staleness::{
    evaluate_max_age, EnvelopeFreshness, StalenessConfig, StalenessReason,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::flexibility_envelope::{EnvelopeMetadata, EnvelopeVersion};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, ExpiryMilestone, Gate, LifecycleClock,
};
use flexcore_domain::command_state::{is_terminal, try_transition, CommandState};
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_BASE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8c00";

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[..12].copy_from_slice(UUID_BASE.as_bytes());
    bytes[12] = 0xC6; // P4.26 fixture marker
    bytes[13] = tag;
    FlexUuid::from_bytes(bytes)
}

fn schedule_uuid() -> FlexUuid {
    FlexUuid::parse(UUID_SCHEDULE).expect("valid test uuid")
}

fn tenant() -> TenantId {
    TenantId::new(uuid(1))
}

fn context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant(),
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
// The recorded Phase 4 default matrix (flexcore-authz `phase4_policies`):
// `command.approve` → TenantAdmin only, at site/asset scope; deny-by-default.
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:combined-fixture-hash-01";
const SIMULATOR_ID: &str = "sim:p426-combined-fixture";
const SIM_KEY_ID: &str = "sim-key-p426";
/// The configured maximum age for the staleness fault (the F-ST-4 recorded
/// configuration shape; DEFAULT label — see the decision note).
const CONFIGURED_MAX_AGE_MILLIS: i64 = 60_000;

// --- The pre-registered expectations (ledger P4.26 step 2) -------------------------
//
// Written BEFORE the assertions; each test asserts the pre-registered value
// verbatim. Where two faults apply, the pre-registered reason is ONE OF the
// applicable failing checks — never a pass.

/// The named rejection each combined fixture must produce.
fn pre_registered_rejection(fixture: &str) -> String {
    match fixture {
        // F-CM-1: stale + infeasible — the §14.2 validation order puts
        // freshness BEFORE feasibility, so the recorded reason is the
        // staleness check (an applicable failing check); the infeasible
        // verdict is present but not reached (the gate stops at the first
        // failure — recorded honestly in the decision note).
        "stale_plus_infeasible" => {
            "envelope_stale:MaxAgeExceeded { age_millis: 61000, max_age_millis: 60000 }".to_string()
        }
        // F-CM-2: duplicate + unauthorized — the approval attempt is the
        // fault that produces the REJECTED record; the conflicting-duplicate
        // fault fires at the store seam (refused there, named `Conflict`).
        "duplicate_plus_unauthorized" => {
            "authorization_denied:no matching rule for role viewer".to_string()
        }
        // F-CM-3: expired + infeasible — the adapter's refusal order is
        // unsigned → unsequenced → expired (ledger order), so the expired
        // refusal is recorded (the `ExpiredCommandRefused` Display text);
        // the inactive model is proven blocking on its own in the same test.
        "expired_plus_infeasible" => "is expired".to_string(),
        // F-CM-4: forged credential + stale — same refusal order; the
        // forged authorization record is proven blocking on its own
        // (`AuthorizationRecordMismatch`).
        "forged_credential_plus_stale" => "is expired".to_string(),
        // F-CM-5: replayed event + conflicting payload — the store seam
        // refusal names the conflict; BOTH pure verdicts are asserted to
        // fire (the pure layer names all failing checks found).
        "replayed_event_plus_conflicting_payload" => {
            CommandRejectionRule::DuplicatedConflictingPayload.code().to_string()
        }
        other => panic!("no pre-registered rejection for fixture '{other}'"),
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

// --- Payload field parsing (the store-seam duplicate contract) ----------------------

fn payload_string_field(payload: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = payload.find(&needle)? + needle.len();
    let rest = &payload[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn payload_u64_field(payload: &str, field: &str) -> Option<u64> {
    let needle = format!("\"{field}\":");
    let start = payload.find(&needle)? + needle.len();
    let digits: String = payload[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

// --- Seam fakes (fail-closed by construction) ---------------------------------------

/// The store seam with the §14.3 duplicate/sequence contract at the seam
/// (the D-P4.24-1 contract the P4.30 store must satisfy) plus the seeding
/// helpers the expiry fixtures need.
struct FakeStore {
    records: Mutex<HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<HashMap<String, Vec<CommandTransition>>>,
    revisions: Mutex<HashMap<(String, u64), ScheduleRevisionUsability>>,
    /// idempotency key → original command id (the dedup registry).
    by_key: Mutex<HashMap<String, String>>,
    /// asset → last recorded sequence (the monotonic per-channel registry).
    last_sequence: Mutex<HashMap<String, u64>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            journals: Mutex::new(HashMap::new()),
            revisions: Mutex::new(HashMap::new()),
            by_key: Mutex::new(HashMap::new()),
            last_sequence: Mutex::new(HashMap::new()),
        }
    }

    fn seed_revision(&self, schedule_id: &str, revision: u64, state: ScheduleRevisionUsability) {
        self.revisions
            .lock()
            .expect("fake store")
            .insert((schedule_id.to_string(), revision), state);
    }

    fn record_count(&self) -> usize {
        self.records.lock().expect("fake store").len()
    }

    fn journal_of(&self, command_id: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default()
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
        // §14.3 duplicate rules at the store seam (one effective command per
        // idempotency key; a conflicting payload is refused with Conflict).
        let key = payload_string_field(&record.command_payload_json, "idempotency_key")
            .expect("the fixture payload declares the idempotency key");
        let incoming_sequence = payload_u64_field(&record.command_payload_json, "sequence")
            .expect("the fixture payload declares the sequence");
        let mut by_key = self.by_key.lock().expect("fake store");
        if let Some(original) = by_key.get(&key).cloned() {
            let original_payload = records
                .get(&original)
                .map(|r| r.command_payload_json.clone())
                .unwrap_or_default();
            if original_payload == record.command_payload_json {
                // EXACT REPLAY: deduplicated to the single original record.
                return Ok(StateChangeReceipt::new(
                    COMMAND_ENTITY.to_string(),
                    StateChangeOperation::Insert,
                    Some(ctx.tenant_id()),
                ));
            }
            // CONFLICTING DUPLICATE: refused.
            return Err(CommandGateStoreError::Conflict {
                command_id: record.command_id.to_string(),
            });
        }
        // §14.3 out-of-sequence rule at the store seam (per asset/channel).
        let mut last_sequence = self.last_sequence.lock().expect("fake store");
        let asset = record.asset_id.clone();
        if let Some(&last) = last_sequence.get(&asset) {
            if incoming_sequence <= last {
                return Err(CommandGateStoreError::Backend {
                    code: "out_of_sequence".to_string(),
                    message: format!(
                        "incoming sequence {incoming_sequence} is not greater than the last \
                         recorded sequence {last} for asset {asset}"
                    ),
                });
            }
        }
        last_sequence.insert(asset, incoming_sequence);
        by_key.insert(key, record.command_id.to_string());
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
        // Compare-and-swap: a redelivered transition (a replayed event's
        // effect) cannot apply twice.
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

/// The envelope-freshness seam: scriptable per fixture; the verdict is
/// derived from the REAL P3.13 evaluation in the staleness fixtures.
struct FakeEnvelopes {
    verdict: Mutex<EnvelopeGateFreshness>,
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

/// The model-version seam: scriptable per fixture (Active by default;
/// NotActive is the fail-closed fault of F-CM-3).
struct FakeModels {
    status_value: Mutex<ModelVersionGateStatus>,
}

impl ModelVersionSource for FakeModels {
    fn status(
        &self,
        _ctx: &TenantContext,
        _grid_model_ref: &str,
    ) -> Result<ModelVersionGateStatus, GateSeamError> {
        Ok(self.status_value.lock().expect("fake models").clone())
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

/// The feasibility runner seam: SCRIPTABLE (the infeasible fault) and
/// COUNTING (F-CM-1 asserts the runner is never called while the staleness
/// fault masks it — the §14.2 first-failure order, proven not assumed).
struct ScriptedRunner {
    script: Mutex<VecDeque<FeasibilityGateVerdict>>,
    calls: Mutex<usize>,
}

impl ScriptedRunner {
    fn new(verdicts: Vec<FeasibilityGateVerdict>) -> Self {
        Self {
            script: Mutex::new(VecDeque::from(verdicts)),
            calls: Mutex::new(0),
        }
    }

    fn call_count(&self) -> usize {
        *self.calls.lock().expect("scripted runner")
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
        *self.calls.lock().expect("scripted runner") += 1;
        let mut script = self.script.lock().expect("scripted runner");
        match script.pop_front() {
            Some(verdict) => Ok(verdict),
            // Exhausted scripts default to the PASSING control verdict (a
            // fixture that reaches the runner more often than scripted is a
            // fixture bug, and the call count exposes it).
            None => Ok(FeasibilityGateVerdict::Passed {
                attempt_id: "attempt-control".to_string(),
                model_id: "combined-feeder".to_string(),
                model_content_hash: MODEL_HASH.to_string(),
                margins: "min v_pu 0.98; max loading 60%".to_string(),
                coverage: "balanced radial combined-feeder; unbalanced deferred (P4.34)".to_string(),
            }),
        }
    }
}

/// The authorization evaluator fake, mirroring the RECORDED Phase 4 default
/// grant matrix (`flexcore-authz::phase4_policies`; the real crate's tests
/// live in `crates/flexcore-authz/tests/phase4_policies.rs` — cited, not
/// duplicated; the dependency rule forbids naming that crate here).
struct Phase4MatrixAuthz;

impl CommandAuthorizationEvaluator for Phase4MatrixAuthz {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision {
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
                rule_reference: format!("policy:phase4:{APPROVER_ROLE}:command.approve:{scope_kind}"),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
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

/// Default-accepts deliveries; the recorded list is the "nothing was sent"
/// proof for the fixtures that must never deliver.
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

// --- The harness ---------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    expiry: CommandExpiryService<'static>,
    store: &'static FakeStore,
    envelopes: &'static FakeEnvelopes,
    models: &'static FakeModels,
    runner: &'static ScriptedRunner,
    outbox: &'static FakeEventOutbox,
    transport: &'static FakeTransport,
    clock: &'static TestClock,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static FakeStore = Box::leak(Box::new(FakeStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes {
        verdict: Mutex::new(EnvelopeGateFreshness::Fresh),
    }));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels {
        status_value: Mutex::new(ModelVersionGateStatus::Active {
            content_hash: MODEL_HASH.to_string(),
        }),
    }));
    let runner: &'static ScriptedRunner = Box::leak(Box::new(ScriptedRunner::new(Vec::new())));
    let authz: &'static Phase4MatrixAuthz = Box::leak(Box::new(Phase4MatrixAuthz));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
    let ids: &'static CountingIdGenerator = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder: &'static DebugEncoder = Box::leak(Box::new(DebugEncoder));
    let transport: &'static FakeTransport = Box::leak(Box::new(FakeTransport::new()));
    let gates = CommandGates {
        store,
        envelopes,
        models,
        assets: Box::leak(Box::new(FakeAssets)),
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
    let expiry = CommandExpiryService {
        store,
        outbox,
        encoder,
        ids,
        clock,
    };
    Harness {
        gates,
        expiry,
        store,
        envelopes,
        models,
        runner,
        outbox,
        transport,
        clock,
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
        TrackingAcceptanceCriterion::new(TrackedQuantity::ActivePowerMW, 0.2, 1, 5_000)
            .expect("a valid criterion"),
    )
    .expect("the simulator target is accepted")
}

// --- Command fixture (pinned inputs) ---------------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

fn propose_input(command_tag: u8, sequence: u64, idempotency_key: &str, p_mw: f64) -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: CommandId::new(uuid(command_tag)),
        scope: CommandScope {
            tenant_id: tenant(),
            portfolio_id: PortfolioId::new(uuid(2)),
            site_id: SiteId::new(uuid(3)),
            asset_id: asset_id(),
        },
        source: SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        },
        schedule: ScheduleReference {
            schedule_id: ScheduleId::new(schedule_uuid()),
            revision: 2,
        },
        intent: CommandIntent {
            reason: "peak-shaving window".to_string(),
            action: "set-bess-power".to_string(),
            actor_or_service: PROPOSER.to_string(),
        },
        target: ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(p_mw).unwrap(),
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
        idempotency_key: idempotency_key.to_string(),
        evidence: EvidenceReferences {
            grid_model: "grid-model:combined-feeder@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature(format!("sig:p426-{command_tag}")),
        supersedes: None,
        command_payload_json: format!(
            "{{\"idempotency_key\":\"{idempotency_key}\",\"sequence\":{sequence},\"p_mw\":{p_mw}}}"
        ),
    }
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p426-1".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

fn approval_request(subject: &str, role: &str) -> CommandAuthorizationRequest {
    CommandAuthorizationRequest {
        subject: subject.to_string(),
        role: role.to_string(),
        action: CommandAuthzAction::Approve,
        scope: ApprovalScope::Asset(asset_id()),
    }
}

fn approval(
    h: &Harness,
    command: &DispatchCommand,
) -> Result<
    flexcore_application::command_gates::AuthorizationGateResult,
    CommandGateError,
> {
    h.gates.record_command_approval(
        &context(),
        command,
        &approval_request(APPROVER, APPROVER_ROLE),
        "±2.0 MW, Q ±0.5 MVAr",
        ValidUntil::from_unix_millis(200_000).unwrap(),
    )
}

/// Creates the command and walks it through the REAL validation and
/// feasibility gates to FEASIBLE.
fn walk_to_feasible(h: &Harness, command_tag: u8, sequence: u64, key: &str) -> DispatchCommand {
    h.store.seed_revision(&schedule_uuid().to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input(command_tag, sequence, key, 1.5))
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
    proposed.command
}

/// Walks one fixture command to APPROVED (returns the command and the P4.13
/// decision record the adapter cross-checks at issue).
fn walk_to_approved(
    h: &Harness,
    command_tag: u8,
    sequence: u64,
    key: &str,
) -> (DispatchCommand, AuthorizationDecisionRecord) {
    let command = walk_to_feasible(h, command_tag, sequence, key);
    let result = approval(h, &command).expect("the control approval must succeed");
    assert!(result.approved, "fixture approval must succeed");
    (command, result.decision_record.expect("an approval carries its decision record"))
}

fn emitted_events(h: &Harness) -> Vec<&'static str> {
    h.outbox
        .writes()
        .iter()
        .map(|write| {
            let payload = write.payload();
            for candidate in [
                "DispatchProposed",
                "DispatchRejected",
                "DispatchApproved",
                "DispatchIssued",
                "DispatchExpired",
                "DispatchAcknowledged",
            ] {
                if payload.starts_with(candidate) {
                    return candidate;
                }
            }
            "unknown"
        })
        .collect()
}

fn event_counts(h: &Harness) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for write in h.outbox.writes() {
        let name = write.payload().split('(').next().unwrap_or_default().to_string();
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

/// The "no combined fixture reaches ISSUED" checks (ledger P4.26 step 3):
/// no ISSUED journal row, nothing delivered to the simulator, and the
/// frozen-edge oracle has no edge from the fixture's final state to ISSUED.
fn assert_never_issued(h: &Harness, command: &DispatchCommand, final_state: CommandState) {
    let journal = h.store.journal_of(command.command_id());
    assert!(
        !journal.iter().any(|entry| entry.to == CommandState::Issued),
        "the combined fixture must never reach ISSUED"
    );
    if is_terminal(final_state) {
        assert!(try_transition(final_state, CommandState::Issued).is_err());
    }
    // EXPIRED is not terminal, but its ONLY outgoing edge is → FALLBACK
    // (frozen edge 27) — never ISSUED.
    if final_state == CommandState::Expired {
        assert!(try_transition(CommandState::Expired, CommandState::Issued).is_err());
    }
    assert!(h.transport.delivered().is_empty(), "nothing may reach the simulator");
}

/// The §16.2 audit-denial record shared by the in-scope denial fixtures:
/// the FEASIBLE → REJECTED journal row names the authorization gate, carries
/// the `authz-denial:` evidence reference and the pre-registered reason, and
/// exactly one `DispatchRejected` was emitted.
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
        evidence.as_str().starts_with("authz-denial:command/"),
        "the denial record reference: {}",
        evidence.as_str()
    );
    let rejected = h
        .outbox
        .writes()
        .iter()
        .filter(|write| write.payload().starts_with("DispatchRejected"))
        .count();
    assert_eq!(rejected, 1, "exactly one DispatchRejected for the denial");
}

// ============================================================================
// F-CM-1 — stale + infeasible: the staleness fault is present AND the
// feasibility verdict is a limit violation. Pre-registered: REJECTED at the
// VALIDATION gate with the staleness check (the §14.2 order — freshness
// precedes feasibility); the feasibility runner is NEVER called (proven by
// the counting seam, not assumed); never ISSUED. The control half proves
// the infeasible fault is real and names its constraint when not masked.
// ============================================================================

#[test]
fn f_cm_1_stale_plus_infeasible_is_rejected_with_the_staleness_check_and_never_issued() {
    let run = |masked: bool| {
        // now = 65_000: the snapshot was computed at 4_000, so its age is
        // 61_000 > the configured 60_000 — the REAL P3.13 evaluation is
        // stale; the command window itself is still open.
        let h = harness(65_000);
        h.runner.script.lock().expect("script").push_back(
            FeasibilityGateVerdict::LimitViolated {
                attempt_id: "attempt-cm1".to_string(),
                model_id: "combined-feeder".to_string(),
                model_content_hash: MODEL_HASH.to_string(),
                constraint: "branch_thermal:line_2_3".to_string(),
            },
        );
        h.store.seed_revision(&schedule_uuid().to_string(), 2, ScheduleRevisionUsability::Active);
        if masked {
            // The staleness fault: derived from the REAL P3.13 max-age
            // evaluation, carried on the gate seam (the composition-root
            // shape, as in F-ST-4).
            let computed_at = 4_000i64;
            let metadata = EnvelopeMetadata {
                envelope_version: EnvelopeVersion::new("env-v1").expect("valid version"),
                computed_at: EventTime::from_unix_millis(computed_at).expect("valid instant"),
                tenant_id: tenant(),
                portfolio_id: PortfolioId::new(uuid(2)),
                input_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
                forecast_version: "fc-v1".to_string(),
                horizon_start_millis: computed_at,
                horizon_end_millis: computed_at + 3_600_000,
                scoped_asset_ids: vec![asset_id()],
            };
            let config = StalenessConfig {
                max_age_millis: CONFIGURED_MAX_AGE_MILLIS,
                min_recompute_interval_millis: 0,
            };
            let freshness = evaluate_max_age(
                &metadata,
                &config,
                EventTime::from_unix_millis(computed_at + CONFIGURED_MAX_AGE_MILLIS + 1_000)
                    .expect("valid instant"),
            );
            let reason = match &freshness {
                EnvelopeFreshness::Stale { reason: StalenessReason::MaxAgeExceeded { age_millis, max_age_millis } } => {
                    assert_eq!(*age_millis, CONFIGURED_MAX_AGE_MILLIS + 1_000);
                    assert_eq!(*max_age_millis, CONFIGURED_MAX_AGE_MILLIS);
                    format!("{reason:?}")
                }
                other => panic!("the staleness fault must be present, got: {other:?}"),
            };
            *h.envelopes.verdict.lock().expect("fake envelopes") =
                EnvelopeGateFreshness::Stale { reason };
        }
        let proposed = h
            .gates
            .create_proposed_command(&context(), propose_input(5, 10, "idem:p426-cm1", 1.5))
            .expect("creation succeeds");
        let validation = h
            .gates
            .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
            .expect("the gate verdict");
        let calls_after_validation = h.runner.call_count();
        let feasibility = if validation.passed {
            Some(
                h.gates
                    .run_feasibility_gate(&context(), &proposed.command, &feasibility_refs(), &actor("svc:grid-gate"))
                    .expect("the feasibility verdict"),
            )
        } else {
            None
        };
        let journal = h.store.journal_of(proposed.command.command_id());
        (
            validation.passed,
            validation.failing_check.clone(),
            feasibility.map(|f| (f.result.passed, f.result.failing_check.clone())),
            calls_after_validation,
            journal.last().map(|e| (e.from, e.to, e.reason.clone())),
            emitted_events(&h),
        )
    };

    // PRE-REGISTERED (UNRUN): the combined fixture is REJECTED at the
    // validation gate; the failing check IS the pre-registered staleness
    // check; the runner was never called (first-failure order proven); the
    // journal row is PROPOSED → REJECTED with the check as the reason;
    // DispatchProposed then DispatchRejected; never ISSUED.
    let (passed, check, feasibility, calls, last_row, events) = run(true);
    assert!(!passed);
    assert_eq!(check.as_deref(), Some(pre_registered_rejection("stale_plus_infeasible").as_str()));
    assert!(feasibility.is_none(), "the gate stops at the first failure — feasibility is not reached");
    assert_eq!(calls, 0, "the feasibility runner is never called behind the staleness fault");
    let (from, to, reason) = last_row.clone().expect("the rejection is journalled");
    assert_eq!((from, to), (CommandState::Proposed, CommandState::Rejected));
    assert_eq!(reason.as_deref(), Some(pre_registered_rejection("stale_plus_infeasible").as_str()));
    assert_eq!(events, vec!["DispatchProposed", "DispatchRejected"]);
    // Determinism: identical inputs → identical outcomes.
    let second = run(true);
    assert_eq!((passed, check, feasibility, calls, last_row, events.clone()), second);

    // The control half: with the staleness fault REMOVED, the SAME
    // infeasibility fault fires and names its constraint — the rejection
    // above masked a REAL fault, not a phantom.
    let (passed, check, feasibility, calls, last_row, events) = run(false);
    assert!(passed, "the fresh control validates");
    assert!(check.is_none());
    let (feas_passed, feas_check) = feasibility.expect("the feasibility gate runs");
    assert!(!feas_passed);
    assert_eq!(
        feas_check.as_deref(),
        Some("network_limit:branch_thermal:line_2_3"),
        "the infeasible fault names its constraint (the P4.22 mapping)"
    );
    assert_eq!(calls, 1, "the runner runs exactly once");
    let (from, to, reason) = last_row.expect("the rejection is journalled");
    assert_eq!((from, to), (CommandState::Validated, CommandState::Rejected));
    assert_eq!(reason.as_deref(), Some("network_limit:branch_thermal:line_2_3"));
    assert_eq!(events, vec!["DispatchProposed", "DispatchRejected"]);
}

#[test]
fn f_cm_1_never_reaches_issued() {
    let h = harness(65_000);
    h.runner.script.lock().expect("script").push_back(
        FeasibilityGateVerdict::LimitViolated {
            attempt_id: "attempt-cm1b".to_string(),
            model_id: "combined-feeder".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            constraint: "branch_thermal:line_2_3".to_string(),
        },
    );
    h.store.seed_revision(&schedule_uuid().to_string(), 2, ScheduleRevisionUsability::Active);
    let computed_at = 4_000i64;
    let metadata = EnvelopeMetadata {
        envelope_version: EnvelopeVersion::new("env-v1").expect("valid version"),
        computed_at: EventTime::from_unix_millis(computed_at).expect("valid instant"),
        tenant_id: tenant(),
        portfolio_id: PortfolioId::new(uuid(2)),
        input_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
        forecast_version: "fc-v1".to_string(),
        horizon_start_millis: computed_at,
        horizon_end_millis: computed_at + 3_600_000,
        scoped_asset_ids: vec![asset_id()],
    };
    let config = StalenessConfig {
        max_age_millis: CONFIGURED_MAX_AGE_MILLIS,
        min_recompute_interval_millis: 0,
    };
    let freshness = evaluate_max_age(
        &metadata,
        &config,
        EventTime::from_unix_millis(computed_at + CONFIGURED_MAX_AGE_MILLIS + 1_000)
            .expect("valid instant"),
    );
    let reason = match &freshness {
        EnvelopeFreshness::Stale { reason } => format!("{reason:?}"),
        EnvelopeFreshness::Fresh => panic!("the staleness fault must be present"),
    };
    *h.envelopes.verdict.lock().expect("fake envelopes") = EnvelopeGateFreshness::Stale { reason };
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input(6, 11, "idem:p426-cm1b", 1.5))
        .expect("creation succeeds");
    let validation = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the gate verdict");
    assert!(!validation.passed);
    assert_never_issued(&h, &proposed.command, CommandState::Rejected);
}

// ============================================================================
// F-CM-2 — duplicate + unauthorized: a command at FEASIBLE whose idempotency
// key is replayed with a CONFLICTING payload (refused at the store seam; the
// P4.14 verdict fires) while an approval is attempted by a role with NO
// grant. Pre-registered: the approval attempt is REJECTED with the named
// authorization error (FEASIBLE → REJECTED, §16.2 denial record); the
// conflicting duplicate is refused with `Conflict` at the seam; never ISSUED.
// ============================================================================

#[test]
fn f_cm_2_duplicate_plus_unauthorized_rejects_with_the_named_authorization_error() {
    let run = || {
        let h = harness(5_000);
        let command = walk_to_feasible(&h, 7, 10, "idem:p426-cm2");
        let writes_before = h.outbox.len();

        // Fault 1 — conflicting duplicate: the same idempotency key with a
        // different payload. The store seam refuses it; the P4.14 pure
        // verdict fires for the same classification.
        let conflict_attempt = h.gates.create_proposed_command(
            &context(),
            propose_input(8, 11, "idem:p426-cm2", 2.5), // same key, different payload
        );
        let seam_refused = matches!(
            &conflict_attempt,
            Err(CommandGateError::Store(CommandGateStoreError::Conflict { .. }))
        );
        let verdict_fires = duplicate_conflict_rejection(true, false).is_some();
        let records_after_duplicate_attempt = h.store.record_count();

        // Fault 2 — unauthorized approval: a role with NO command.approve
        // grant (deny-by-default).
        let denial = h
            .gates
            .record_command_approval(
                &context(),
                &command,
                &approval_request(APPROVER, "viewer"),
                "±2.0 MW",
                ValidUntil::from_unix_millis(200_000).unwrap(),
            )
            .expect("a denial is a verdict, not an infrastructure error");
        let journal = h.store.journal_of(command.command_id());
        (
            seam_refused,
            verdict_fires,
            records_after_duplicate_attempt,
            denial.approved,
            denial.denial_reason.clone(),
            journal.last().map(|e| (e.from, e.to, e.gate, e.reason.clone())),
            writes_before,
            h.outbox.len(),
        )
    };

    let (
        seam_refused,
        verdict_fires,
        records_after_duplicate_attempt,
        approved,
        denial_reason,
        last_row,
        writes_before,
        writes_after,
    ) = run();
    // Fault 1: refused at the seam; the pure verdict fires; no second
    // record exists for the key.
    assert!(seam_refused, "the conflicting duplicate is refused with Conflict");
    assert!(
        verdict_fires,
        "the P4.14 duplicated_conflicting_payload verdict fires"
    );
    assert_eq!(records_after_duplicate_attempt, 1, "one effective command per key");
    // Fault 2: the pre-registered denial.
    assert!(!approved);
    assert_eq!(
        denial_reason.as_deref(),
        Some(pre_registered_rejection("duplicate_plus_unauthorized").as_str())
    );
    let (from, to, gate, reason) = last_row.clone().expect("the denial is journalled");
    assert_eq!((from, to), (CommandState::Feasible, CommandState::Rejected));
    assert_eq!(gate, Some(Gate::Authorization));
    assert_eq!(reason.as_deref(), Some(pre_registered_rejection("duplicate_plus_unauthorized").as_str()));
    // The conflicting-duplicate attempt emitted no events of its own; the
    // walk already carried DispatchProposed, the denial adds DispatchRejected.
    assert_eq!(writes_after, writes_before + 1, "DispatchRejected is the only new write");
    // Determinism.
    assert_eq!(run(), (
        seam_refused,
        verdict_fires,
        records_after_duplicate_attempt,
        approved,
        denial_reason,
        last_row,
        writes_before,
        writes_after,
    ));
}

#[test]
fn f_cm_2_duplicate_plus_unauthorized_has_the_denial_record_and_never_issued() {
    let h = harness(5_000);
    let command = walk_to_feasible(&h, 9, 12, "idem:p426-cm2b");
    let conflict_attempt = h.gates.create_proposed_command(
        &context(),
        propose_input(10, 13, "idem:p426-cm2b", 3.0),
    );
    assert!(matches!(
        conflict_attempt,
        Err(CommandGateError::Store(CommandGateStoreError::Conflict { .. }))
    ));
    let denial = h
        .gates
        .record_command_approval(
            &context(),
            &command,
            &approval_request(APPROVER, "viewer"),
            "±2.0 MW",
            ValidUntil::from_unix_millis(200_000).unwrap(),
        )
        .expect("the denial verdict");
    assert!(!denial.approved);
    assert_denial_record(
        &h,
        &command,
        &pre_registered_rejection("duplicate_plus_unauthorized"),
        APPROVER,
    );
    assert_never_issued(&h, &command, CommandState::Rejected);
}

// ============================================================================
// F-CM-3 — expired + infeasible: a command already at APPROVED when its
// validity ends, with an INACTIVE model behind the issue hook. Both faults
// apply at the issue attempt; the adapter's ledger refusal order (unsigned
// → unsequenced → expired) puts the expired refusal first — the recorded
// reason IS one of the applicable failing checks. The P4.15 sweep then
// expires the command (APPROVED → EXPIRED, milestone recorded). The model
// fault is proven blocking on its own. Never ISSUED.
// ============================================================================

#[test]
fn f_cm_3_expired_plus_infeasible_refuses_the_issue_and_expires_with_the_milestone() {
    let run = || {
        let h = harness(5_000);
        let (command, decision_record) = walk_to_approved(&h, 11, 10, "idem:p426-cm3");
        // Fault 1 (expired): validity ends before issue — the clock is
        // advanced exactly to the window's end-exclusive boundary.
        // Fault 2 (infeasible/fail-closed): the grid model behind the issue
        // hook is no longer Active.
        h.clock.advance_to(70_000);
        *h.models.status_value.lock().expect("fake models") =
            ModelVersionGateStatus::NotActive { state: "retired".to_string() };
        // The issue attempt at now == 70_000 (the end-exclusive boundary).
        let issue_attempt = adapter_for(&h).issue(
            &context(),
            &command,
            &decision_record,
            &actor("svc:dispatcher"),
        );
        let issue_error_text = issue_attempt.as_ref().err().map(|e| e.to_string());
        let issue_is_expired = matches!(
            &issue_attempt,
            Err(DispatchAdapterError::ExpiredCommandRefused { .. })
        );
        // The P4.15 sweep expires the lapsed APPROVED command.
        let expiry = h.expiry.apply_expiry(&context(), scope(), command.command_id(), &actor("test:operator"));
        let journal = h.store.journal_of(command.command_id());
        (
            issue_is_expired,
            issue_error_text,
            expiry,
            journal.last().cloned(),
            emitted_events(&h),
        )
    };

    let (issue_is_expired, issue_error_text, expiry, last_row, events) = run();
    // Fault 1 is the recorded refusal (one of the two applicable checks).
    assert!(
        issue_is_expired,
        "the expired refusal fires first (ledger refusal order): {issue_error_text:?}"
    );
    assert!(
        issue_error_text
            .as_deref()
            .unwrap_or_default()
            .contains(&pre_registered_rejection("expired_plus_infeasible")),
        "the refusal text is the expired-command refusal: {issue_error_text:?}"
    );
    assert!(
        issue_error_text
            .as_deref()
            .unwrap_or_default()
            .contains("EXPIRED transition is the expiry sweep's"),
        "the refusal names the sweep as the expiry owner: {issue_error_text:?}"
    );
    // The sweep expires the command with the milestone recorded.
    let expiry = expiry.expect("the lapsed APPROVED command expires");
    let ExpiryOutcome {
        transition,
        milestone,
        expired_at_unix_millis,
    } = &expiry;
    assert_eq!(transition.from, CommandState::Approved);
    assert_eq!(transition.to, CommandState::Expired);
    assert_eq!(*expired_at_unix_millis, 70_000, "the end-exclusive boundary instant");
    let last = last_row.clone().expect("the expiry is journalled");
    assert_eq!(last.to, CommandState::Expired);
    assert_eq!(events.iter().filter(|e| **e == "DispatchExpired").count(), 1);
    // The milestone text is the journal reason (the P4.15 mapping).
    assert!(last
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("issue"), "the milestone not reached is recorded");
    // Determinism.
    let (issue_is_expired_2, issue_error_text_2, expiry_2, last_row_2, events_2) = run();
    assert_eq!(issue_is_expired, issue_is_expired_2);
    assert_eq!(issue_error_text, issue_error_text_2);
    assert_eq!(expiry.transition, expiry_2.transition);
    assert_eq!(expiry.milestone, expiry_2.milestone);
    assert_eq!(last_row, last_row_2);
    assert_eq!(events, events_2);
    // The milestone mapping is the pre-registered BeforeIssue.
    assert!(matches!(expiry.milestone, ExpiryMilestone::BeforeIssue));
    assert!(matches!(milestone, ExpiryMilestone::BeforeIssue));
}

/// The control half of F-CM-3: the SAME inactive-model fault, WITHOUT the
/// expired fault, blocks the issue on its own — the combined fixture's
/// recorded reason masked a REAL fault.
#[test]
fn f_cm_3_control_the_inactive_model_fault_alone_blocks_the_issue() {
    let h = harness(5_000); // validity still open
    let (command, decision_record) = walk_to_approved(&h, 12, 10, "idem:p426-cm3b");
    *h.models.status_value.lock().expect("fake models") =
        ModelVersionGateStatus::NotActive { state: "retired".to_string() };
    let error = adapter_for(&h)
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect_err("an inactive model blocks the issue");
    match &error {
        DispatchAdapterError::ModelNotActive { grid_model_ref, state } => {
            assert_eq!(grid_model_ref, "grid-model:combined-feeder@1");
            assert_eq!(state, "retired");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_never_issued(&h, &command, CommandState::Approved);
}

#[test]
fn f_cm_3_expired_plus_infeasible_never_reaches_issued() {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 13, 10, "idem:p426-cm3c");
    *h.models.status_value.lock().expect("fake models") =
        ModelVersionGateStatus::NotActive { state: "retired".to_string() };
    let attempt = adapter_for(&h).issue(&context(), &command, &decision_record, &actor("svc:dispatcher"));
    assert!(matches!(attempt, Err(DispatchAdapterError::ExpiredCommandRefused { .. })));
    let _ = h.expiry.apply_expiry(&context(), scope(), command.command_id(), &actor("test:operator"));
    let journal = h.store.journal_of(command.command_id());
    assert_eq!(journal.last().map(|e| e.to), Some(CommandState::Expired));
    assert_never_issued(&h, &command, CommandState::Expired);
}

// ============================================================================
// F-CM-4 — forged credential + stale (§17.4 forged-command threat): a
// command at APPROVED whose issue is attempted with a FORGED authorization
// decision record while its validity has lapsed. The refusal order puts the
// expired refusal first (the recorded reason is one of the two applicable
// checks); the forged credential is proven blocking on its own
// (`AuthorizationRecordMismatch`); the pure refusal-order check pins that a
// forged authenticity string with a lapsed window is refused EXPIRED, and a
// blank signature is refused UNSIGNED. Never ISSUED.
//
// Recorded honestly: the simulation stack does not cryptographically verify
// command signatures (D-P4.16-1 — the keyring signs at issue with a
// non-cryptographic reference); the enforced forged-credential surface is
// the P4.13 authorization-record cross-check. Real signature verification
// is P4.28's (open item, decision note §limitations).
// ============================================================================

fn forged_decision_record() -> AuthorizationDecisionRecord {
    AuthorizationDecisionRecord {
        decision_reference: "authz-decision:forged@0".to_string(),
        authorizing_actor: APPROVER.to_string(),
        authorizing_role: APPROVER_ROLE.to_string(),
        approved_scope: format!("asset:{}", uuid(4)),
        approved_limits: "±2.0 MW".to_string(),
        decided_at_unix_millis: 5_000,
        expiry_unix_millis: 200_000,
        separation_of_duties: flexcore_application::command_gates::AppliedSeparationOfDuties::Enforced,
        covered_validation_evidence: "validation:forged".to_string(),
        covered_feasibility_evidence: "feasibility:forged".to_string(),
    }
}

#[test]
fn f_cm_4_forged_credential_plus_stale_refuses_the_issue_and_never_issued() {
    let run = |now: i64, record: &AuthorizationDecisionRecord| {
        let h = harness(now);
        let (command, _real_record) = walk_to_approved(&h, 14, 10, "idem:p426-cm4");
        let attempt = adapter_for(&h).issue(&context(), &command, record, &actor("svc:dispatcher"));
        let journal = h.store.journal_of(command.command_id());
        let state = journal.last().map(|e| e.to).unwrap_or(CommandState::Proposed);
        (
            matches!(
                &attempt,
                Err(DispatchAdapterError::AuthorizationRecordMismatch { .. })
            ),
            attempt.err().map(|e| e.to_string()),
            state,
        )
    };

    // Combined: forged record + lapsed window → the expired refusal (first
    // in the ledger refusal order; one of the two applicable checks).
    let (is_mismatch, combined_error, state) = run(80_000, &forged_decision_record());
    assert!(!is_mismatch, "the expired refusal precedes the forged-record cross-check");
    assert!(
        combined_error
            .as_deref()
            .unwrap_or_default()
            .contains(&pre_registered_rejection("forged_credential_plus_stale")),
        "the combined refusal is the expired-command refusal: {combined_error:?}"
    );
    assert_eq!(state, CommandState::Approved, "the adapter refuses, it does not transition");

    // The forged credential ALONE (fresh window) blocks with its own named
    // error — the combined refusal masked a REAL fault.
    let (is_mismatch, forged_error, _) = run(5_000, &forged_decision_record());
    assert!(
        is_mismatch,
        "the forged-credential refusal is the AuthorizationRecordMismatch: {forged_error:?}"
    );
    assert!(
        forged_error.as_deref().unwrap_or_default().contains("does not match the journal"),
        "the forged-credential refusal names the mismatch: {forged_error:?}"
    );
}

/// The pure refusal-order check (the §14.3 field-15/§17.4 surface): with a
/// lapsed window, a forged (non-empty, unverifiable) authenticity string is
/// refused EXPIRED; with an open window it passes the pure check (the
/// cryptographic verification is P4.28's — recorded); a blank signature is
/// refused UNSIGNED at any clock.
#[test]
fn f_cm_4_the_pure_refusal_order_pins_forged_and_blank_authenticity() {
    let forged = CommandAuthenticity::Signature("forged-by-attacker".to_string());
    let blank = CommandAuthenticity::Signature("   ".to_string());
    let input = |authenticity: &CommandAuthenticity, now: i64| IssueRefusalInput {
        authenticity,
        sequence: 10,
        valid_until_millis: 70_000,
        now_unix_millis: now,
    };
    // Forged + stale: refused EXPIRED (the first applicable check in ledger
    // order — unsigned is non-firing for a non-empty string).
    let combined = check_issue_refusals(&input(&forged, 80_000));
    assert!(matches!(combined, Err(IssueRefusal::Expired { .. })));
    // Blank signature + stale: refused UNSIGNED (it precedes expired).
    let blank_stale = check_issue_refusals(&input(&blank, 80_000));
    assert!(matches!(blank_stale, Err(IssueRefusal::Unsigned)));
    // Blank signature alone: refused UNSIGNED.
    let blank_fresh = check_issue_refusals(&input(&blank, 5_000));
    assert!(matches!(blank_fresh, Err(IssueRefusal::Unsigned)));
    // Determinism.
    assert_eq!(combined, check_issue_refusals(&input(&forged, 80_000)));
}

// ============================================================================
// F-CM-5 — replayed event + conflicting payload: a replayed sequence number
// AND a conflicting payload under the same idempotency key. The PURE §14.3
// verdict layer evaluates each rule independently and BOTH failing checks
// are named (the honest multi-check surface); the store seam refuses the
// insert; the replayed EVENT half is refused at the outbox seam
// (`DuplicateKey` on the existing idempotency key). Never ISSUED.
// ============================================================================

#[test]
fn f_cm_5_replayed_event_plus_conflicting_payload_names_both_failing_checks() {
    // The PURE layer: both rules evaluated independently on the same
    // combined input — BOTH fire, and BOTH names are collected (the pure
    // layer lists all failing checks found; the gate/journal layer is
    // first-failure single-reason — recorded honestly in the decision note).
    let same_key = true;
    let same_payload = false; // conflicting payload
    let incoming_sequence = 10u64;
    let last_recorded = Some(10u64); // the replayed sequence number
    let duplicate = duplicate_conflict_rejection(same_key, same_payload);
    let sequence = out_of_sequence_rejection(incoming_sequence, last_recorded);
    let duplicate = duplicate.expect("the conflicting-payload verdict fires");
    let sequence = sequence.expect("the out-of-sequence verdict fires");
    assert_eq!(duplicate.rule, CommandRejectionRule::DuplicatedConflictingPayload);
    assert_eq!(duplicate.code(), pre_registered_rejection("replayed_event_plus_conflicting_payload"));
    assert_eq!(sequence.rule, CommandRejectionRule::OutOfSequence);
    assert!(sequence.detail.contains("not greater than"), "the replayed sequence is named: {}", sequence.detail);
    // The replayed event is an EXACT replay of a processed sequence — the
    // boundary: an exactly-equal sequence is already refused (strictly
    // monotonic), and a lower one too.
    assert!(out_of_sequence_rejection(9, last_recorded).is_some());
    // Determinism.
    assert_eq!(duplicate, duplicate_conflict_rejection(same_key, same_payload).unwrap());
    assert_eq!(sequence, out_of_sequence_rejection(incoming_sequence, last_recorded).unwrap());
}

#[test]
fn f_cm_5_the_store_seam_refuses_the_combined_replay_and_conflict() {
    let run = || {
        let h = harness(5_000);
        // The original command registers key K with sequence 10.
        let command = walk_to_feasible(&h, 15, 10, "idem:p426-cm5");
        // The combined fault: same key K, REPLAYED sequence 10, conflicting
        // payload. The seam refuses (the key conflict fires first in the
        // seam's check order; the sequence fault is proven at the pure
        // layer above).
        let attempt = h.gates.create_proposed_command(
            &context(),
            propose_input(16, 10, "idem:p426-cm5", 2.5),
        );
        let journal = h.store.journal_of(command.command_id());
        (
            attempt.err().map(|e| e.to_string()),
            h.store.record_count(),
            journal.iter().any(|e| e.to == CommandState::Issued),
        )
    };
    let (error, records, reached_issued) = run();
    assert!(
        error.as_deref().unwrap_or_default().contains("already exists"),
        "the seam refusal names the conflict: {error:?}"
    );
    assert_eq!(records, 1, "one effective command per idempotency key");
    assert!(!reached_issued);
    // Determinism.
    assert_eq!(run(), (error, records, reached_issued));
}

/// The replayed-EVENT half: re-delivering an outbox write under an existing
/// idempotency key is refused with the named `DuplicateKey` (the P1.12
/// unique constraint; republish-after-crash safe, §15.5).
#[test]
fn f_cm_5_a_replayed_outbox_event_is_refused_with_duplicate_key() {
    let run = || {
        let h = harness(5_000);
        let command = walk_to_feasible(&h, 17, 10, "idem:p426-cm5b");
        let first = h.outbox.writes().first().cloned().expect("the first write");
        let replay = OutboxWrite::new(
            StateChangeReceipt::new(COMMAND_ENTITY.to_string(), StateChangeOperation::Insert, Some(tenant())),
            first.idempotency_key().to_string(),
            first.destination_subject().to_string(),
            first.payload().to_string(),
            first.created_at(),
        )
        .expect("the replayed write is well-formed");
        let outcome = EventOutbox::append(&h.outbox, replay);
        (
            outcome.err(),
            h.outbox.len(),
            h.store
                .journal_of(command.command_id())
                .iter()
                .any(|e| e.to == CommandState::Issued),
        )
    };
    let (error, writes, reached_issued) = run();
    match &error {
        Some(EventOutboxError::DuplicateKey { idempotency_key }) => {
            assert!(idempotency_key.starts_with("dispatch_proposed:"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(writes, 1, "only the original DispatchProposed write exists");
    assert!(!reached_issued);
    // Determinism.
    assert_eq!(run(), (error, writes, reached_issued));
}

// ============================================================================
// The exit check in aggregate: every pre-registered outcome holds, the
// determinism discipline holds, and NO combined fixture reaches ISSUED
// ============================================================================

/// One fixture run's outcome summary (for the determinism + no-ISSUED
/// aggregate).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FixtureOutcome {
    fixture: &'static str,
    /// The rejection text the fixture produced (must name an applicable
    /// failing check; where two apply, one of them).
    outcome_text: String,
    /// The command's final state.
    final_state: CommandState,
    /// Whether the fixture reached ISSUED (must be false for all).
    reached_issued: bool,
}

fn run_combined_fixture(fixture: &'static str) -> FixtureOutcome {
    match fixture {
        "stale_plus_infeasible" => {
            let h = harness(65_000);
            h.store.seed_revision(&schedule_uuid().to_string(), 2, ScheduleRevisionUsability::Active);
            let computed_at = 4_000i64;
            let metadata = EnvelopeMetadata {
                envelope_version: EnvelopeVersion::new("env-v1").expect("valid version"),
                computed_at: EventTime::from_unix_millis(computed_at).expect("valid instant"),
                tenant_id: tenant(),
                portfolio_id: PortfolioId::new(uuid(2)),
                input_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
                forecast_version: "fc-v1".to_string(),
                horizon_start_millis: computed_at,
                horizon_end_millis: computed_at + 3_600_000,
                scoped_asset_ids: vec![asset_id()],
            };
            let config = StalenessConfig {
                max_age_millis: CONFIGURED_MAX_AGE_MILLIS,
                min_recompute_interval_millis: 0,
            };
            let freshness = evaluate_max_age(
                &metadata,
                &config,
                EventTime::from_unix_millis(computed_at + CONFIGURED_MAX_AGE_MILLIS + 1_000)
                    .expect("valid instant"),
            );
            let reason = match &freshness {
                EnvelopeFreshness::Stale { reason } => format!("{reason:?}"),
                EnvelopeFreshness::Fresh => panic!("the staleness fault must be present"),
            };
            *h.envelopes.verdict.lock().expect("fake envelopes") =
                EnvelopeGateFreshness::Stale { reason };
            let proposed = h
                .gates
                .create_proposed_command(&context(), propose_input(5, 10, "idem:p426-agg-1", 1.5))
                .expect("creation succeeds");
            let validation = h
                .gates
                .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
                .expect("the gate verdict");
            FixtureOutcome {
                fixture,
                outcome_text: validation.failing_check.unwrap_or_default(),
                final_state: h
                    .store
                    .journal_of(proposed.command.command_id())
                    .last()
                    .map(|e| e.to)
                    .unwrap_or(CommandState::Proposed),
                reached_issued: h
                    .store
                    .journal_of(proposed.command.command_id())
                    .iter()
                    .any(|e| e.to == CommandState::Issued),
            }
        }
        "duplicate_plus_unauthorized" => {
            let h = harness(5_000);
            let command = walk_to_feasible(&h, 7, 10, "idem:p426-agg-2");
            let _ = h.gates.create_proposed_command(
                &context(),
                propose_input(8, 11, "idem:p426-agg-2", 2.5),
            );
            let denial = h
                .gates
                .record_command_approval(
                    &context(),
                    &command,
                    &approval_request(APPROVER, "viewer"),
                    "±2.0 MW",
                    ValidUntil::from_unix_millis(200_000).unwrap(),
                )
                .expect("the denial verdict");
            FixtureOutcome {
                fixture,
                outcome_text: denial.denial_reason.unwrap_or_default(),
                final_state: h
                    .store
                    .journal_of(command.command_id())
                    .last()
                    .map(|e| e.to)
                    .unwrap_or(CommandState::Feasible),
                reached_issued: h
                    .store
                    .journal_of(command.command_id())
                    .iter()
                    .any(|e| e.to == CommandState::Issued),
            }
        }
        "expired_plus_infeasible" => {
            let h = harness(5_000);
            let (command, decision_record) = walk_to_approved(&h, 11, 10, "idem:p426-agg-3");
            h.clock.advance_to(70_000);
            *h.models.status_value.lock().expect("fake models") =
                ModelVersionGateStatus::NotActive { state: "retired".to_string() };
            let attempt = adapter_for(&h).issue(
                &context(),
                &command,
                &decision_record,
                &actor("svc:dispatcher"),
            );
            let _ = h.expiry.apply_expiry(&context(), scope(), command.command_id(), &actor("test:operator"));
            FixtureOutcome {
                fixture,
                outcome_text: attempt.err().map(|e| e.to_string()).unwrap_or_default(),
                final_state: h
                    .store
                    .journal_of(command.command_id())
                    .last()
                    .map(|e| e.to)
                    .unwrap_or(CommandState::Approved),
                reached_issued: h
                    .store
                    .journal_of(command.command_id())
                    .iter()
                    .any(|e| e.to == CommandState::Issued),
            }
        }
        "forged_credential_plus_stale" => {
            let h = harness(80_000);
            let (command, _real) = walk_to_approved(&h, 14, 10, "idem:p426-agg-4");
            let attempt = adapter_for(&h).issue(
                &context(),
                &command,
                &forged_decision_record(),
                &actor("svc:dispatcher"),
            );
            FixtureOutcome {
                fixture,
                outcome_text: attempt.err().map(|e| e.to_string()).unwrap_or_default(),
                final_state: h
                    .store
                    .journal_of(command.command_id())
                    .last()
                    .map(|e| e.to)
                    .unwrap_or(CommandState::Approved),
                reached_issued: h
                    .store
                    .journal_of(command.command_id())
                    .iter()
                    .any(|e| e.to == CommandState::Issued),
            }
        }
        "replayed_event_plus_conflicting_payload" => {
            let h = harness(5_000);
            let command = walk_to_feasible(&h, 15, 10, "idem:p426-agg-5");
            let attempt = h.gates.create_proposed_command(
                &context(),
                propose_input(16, 10, "idem:p426-agg-5", 2.5),
            );
            FixtureOutcome {
                fixture,
                outcome_text: attempt.err().map(|e| e.to_string()).unwrap_or_default(),
                final_state: h
                    .store
                    .journal_of(command.command_id())
                    .last()
                    .map(|e| e.to)
                    .unwrap_or(CommandState::Proposed),
                reached_issued: h
                    .store
                    .journal_of(command.command_id())
                    .iter()
                    .any(|e| e.to == CommandState::Issued),
            }
        }
        other => panic!("unknown combined fixture '{other}'"),
    }
}

#[test]
fn every_pre_registered_outcome_holds_and_no_combined_fixture_reaches_issued() {
    let fixtures = vec![
        run_combined_fixture("stale_plus_infeasible"),
        run_combined_fixture("duplicate_plus_unauthorized"),
        run_combined_fixture("expired_plus_infeasible"),
        run_combined_fixture("forged_credential_plus_stale"),
        run_combined_fixture("replayed_event_plus_conflicting_payload"),
    ];
    for outcome in &fixtures {
        // Every rejection names an applicable failing check — never empty,
        // never a pass.
        assert!(
            !outcome.outcome_text.is_empty(),
            "fixture {}: the rejection must name a failing check",
            outcome.fixture
        );
        // The recorded reason is ONE OF the applicable failing checks of
        // that fixture (asserted per-fixture in the dedicated tests; here
        // the structural form is checked: the text matches the
        // pre-registered family for the fixture).
        match outcome.fixture {
            "stale_plus_infeasible" => assert_eq!(
                outcome.outcome_text,
                pre_registered_rejection("stale_plus_infeasible")
            ),
            "duplicate_plus_unauthorized" => assert_eq!(
                outcome.outcome_text,
                pre_registered_rejection("duplicate_plus_unauthorized")
            ),
            "expired_plus_infeasible" => assert!(outcome.outcome_text.contains(
                &pre_registered_rejection("expired_plus_infeasible")
            )),
            "forged_credential_plus_stale" => assert!(outcome.outcome_text.contains(
                &pre_registered_rejection("forged_credential_plus_stale")
            )),
            "replayed_event_plus_conflicting_payload" => assert!(
                outcome.outcome_text.contains("conflict")
                    || outcome.outcome_text.contains("already exists"),
            ),
            other => panic!("unknown fixture {other}"),
        }
        // NO combined fixture reaches ISSUED (ledger P4.26 step 3).
        assert_eq!(
            outcome.final_state,
            match outcome.fixture {
                "stale_plus_infeasible" => CommandState::Rejected,
                "duplicate_plus_unauthorized" => CommandState::Rejected,
                "expired_plus_infeasible" => CommandState::Expired,
                "forged_credential_plus_stale" => CommandState::Approved,
                "replayed_event_plus_conflicting_payload" => CommandState::Feasible,
                other => panic!("unknown fixture {other}"),
            },
            "fixture {} ends in its pre-registered final state",
            outcome.fixture
        );
        assert!(
            !outcome.reached_issued,
            "fixture {} must never reach ISSUED",
            outcome.fixture
        );
    }

    // The frozen-edge oracle (P0.11): the block is structural — no edge
    // REJECTED → ISSUED, FEASIBLE → ISSUED, or EXPIRED → ISSUED exists in
    // the frozen table.
    for from in [
        CommandState::Rejected,
        CommandState::Feasible,
        CommandState::Expired,
    ] {
        assert!(
            try_transition(from, CommandState::Issued).is_err(),
            "{} → ISSUED must not exist in the frozen table",
            from.name()
        );
    }
}

/// Determinism (ledger P4.26 step 3): the combined fixtures run twice from
/// clean state and produce IDENTICAL outcomes.
#[test]
fn the_combined_fixtures_are_deterministic_across_two_runs() {
    let run = || vec![
        run_combined_fixture("stale_plus_infeasible"),
        run_combined_fixture("duplicate_plus_unauthorized"),
        run_combined_fixture("expired_plus_infeasible"),
        run_combined_fixture("forged_credential_plus_stale"),
        run_combined_fixture("replayed_event_plus_conflicting_payload"),
    ];
    let first = run();
    let second = run();
    assert_eq!(first, second, "identical combined fixtures must produce identical outcomes");
}
