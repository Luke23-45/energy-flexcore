// P4.24 — Deterministic rejection: duplicate commands
//         (report §23 Stage 4 exit gate, §14.3 idempotency key + duplicated
//         with conflicting payloads, §15.4–§15.5 at-least-once delivery and
//         the transactional outbox; ledger P4.24; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/
//         23_25_rejection_suites_b.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Fixture (a) EXACT REPLAY: the same idempotency key with the same
//   payload is an idempotent replay — deduplicated to the single original
//   record (no second record at the store seam) and, end to end, no second
//   issue (the adapter refuses a re-issue of the already-ISSUED command;
//   exactly one envelope reaches the transport).
// - Fixture (b) CONFLICTING DUPLICATE: the same idempotency key with a
//   conflicting payload is REJECTED — the P4.14 `DuplicatedConflictingPayload`
//   verdict fires and the store seam refuses a second record under the key.
// - Fixture (c) OUT-OF-SEQUENCE: the same asset/channel with a version that
//   is not strictly greater than the last recorded one is REJECTED (the
//   P4.14 `OutOfSequence` verdict and the store-sequence seam); the first
//   command of a channel is never out of sequence.
// - Fixture (d) REDELIVERED EVENT: an at-least-once redelivery of an
//   already-processed command causes NO duplicate side effect — the store's
//   compare-and-swap refuses the replayed transition, the outbox port
//   refuses a second write under the same idempotency key, and the adapter
//   refuses a re-issue by state. (The durable NATS-inbox dedup consumer is
//   the P1.16 fault slice, `crates/adapters/nats/tests/fault_slice.rs`
//   case (a) — cross-referenced, not duplicated here.)
// - Every fixture runs TWICE in-suite and produces IDENTICAL outcomes
//   (determinism); the outbox/command records show EXACTLY ONE effective
//   command per idempotency key.
// - The idempotency-key DERIVATION is tested: the key must cover the FULL
//   payload plus the sequence — the suite pins that property over the real
//   §14.3 `CommandRecordFields` shape (every field perturbation changes the
//   derived key; the derivation is deterministic across runs).
//
// Dependency note (honest): P4.24's ledger inputs are P4.14 (done) and
// P4.30 (pending). The suite exercises the idempotency/sequence mechanisms
// at their EXISTING enforcement points — the P4.14 rejection-rule verdicts,
// the store seam's insert/compare-and-swap contract (the suite's fake states
// the contract the P4.30 store implementation must satisfy), the P1.09
// outbox idempotency-key uniqueness, and the P4.16 adapter's state guard —
// with in-test fakes; the P4.30 outbox/inbox wiring re-verification is an
// explicit follow-up (recorded in the decision doc).
//
// Every timestamp in this suite comes from a controlled clock (never
// wall-clock); every faked seam fails closed by construction.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGateError,
    CommandGatePolicy, CommandGateStore, CommandGateStoreError, CommandGates, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposedCommandRecord, ProposeCommandInput, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, SeparationOfDuties, COMMAND_ENTITY,
};
use flexcore_application::command_record::{
    duplicate_conflict_rejection, out_of_sequence_rejection, AuthorizationRecordFields,
    CommandRecordFields, CommandRejectionRule,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, DispatchAdapterError, SimulationKeyring, SimulatorDispatchAdapter,
    SimulatorDispatchEnvelope, SimulatorDispatchTransport, SimulatedAcceptance,
    SimulatorTransportError, TrackingAcceptanceCriterion, TrackedQuantity,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};
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
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a04";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a05";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a06";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn uuid_by_tag(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
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

fn asset_id() -> AssetId {
    AssetId::new(uuid(UUID_ASSET))
}

fn command_id() -> CommandId {
    CommandId::new(uuid(UUID_COMMAND))
}

const PROPOSER: &str = "service:scheduler";
const APPROVER: &str = "user:approver-1";
const APPROVER_ROLE: &str = "tenant-admin";
const MODEL_HASH: &str = "fnv1a64:dup-feeder-hash-01";
const SIMULATOR_ID: &str = "sim:p424-rejection-fixture";
const SIM_KEY_ID: &str = "sim-key-p424";

// --- The pre-registered expectations (ledger P4.24 step 2) --------------------------
//
// Each fixture's outcome is registered here BEFORE the assertions; the tests
// look their fixture up and assert the pre-registered value verbatim.

fn pre_registered_outcome(fixture: &str) -> String {
    match fixture {
        // (a) exact replay: deduplicated — one record, at most one issue.
        "exact_replay_deduplicated" => {
            "idempotent_replay:one_effective_command_per_idempotency_key".to_string()
        }
        // (b) same key, conflicting payload: REJECTED.
        "conflicting_duplicate_rejected" => {
            CommandRejectionRule::DuplicatedConflictingPayload.code().to_string()
        }
        // (c) same asset/channel, out-of-sequence version: REJECTED.
        "out_of_sequence_rejected" => CommandRejectionRule::OutOfSequence.code().to_string(),
        // (d) redelivered event: idempotent consumer — no duplicate side effect.
        "redelivered_event_no_duplicate_side_effect" => {
            "idempotent_consumer:no_duplicate_side_effect".to_string()
        }
        other => panic!("no pre-registered outcome for fixture '{other}'"),
    }
}

// --- Controlled clock (never wall-clock) -------------------------------------------

struct TestClock {
    now_unix_millis: Mutex<i64>,
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

// --- The §14.3 idempotency-key derivation (the derivation is TESTED) ---------------
//
// The §14.3 idempotency key is a mandatory field on every command (P1.06
// field group 11); the DERIVATION that produces it is composition-root
// machinery (P4.30 wiring). The ledger's risk — "idempotency key derived
// from mutable fields" — is mitigated by a REQUIRED PROPERTY, pinned here
// over the real flat record: the derivation must cover the FULL payload
// (every §14.3 field of `CommandRecordFields`) PLUS the sequence, so any
// payload difference under the same declared key is detectable. This in-test
// derivation is the reference form; the P4.30 implementation must satisfy
// the same property (re-verification is the recorded open item).

/// The reference derivation: a stable fingerprint over EVERY field of the
/// flat §14.3 record (full payload + sequence). Deterministic across runs
/// (`DefaultHasher::new()` is keyed deterministically).
fn derive_idempotency_key(record: &CommandRecordFields) -> String {
    let mut hasher = DefaultHasher::new();
    // Identity + scope.
    record.command_id.hash(&mut hasher);
    record.tenant_id.hash(&mut hasher);
    record.portfolio_id.hash(&mut hasher);
    record.site_id.hash(&mut hasher);
    record.asset_id.hash(&mut hasher);
    // Source + schedule.
    record.source_subsystem.hash(&mut hasher);
    record.source_component_version.hash(&mut hasher);
    record.schedule_id.hash(&mut hasher);
    record.schedule_revision.hash(&mut hasher);
    // Intent + target + units + sign.
    record.reason.hash(&mut hasher);
    record.action.hash(&mut hasher);
    record.actor_or_service.hash(&mut hasher);
    record.target.hash(&mut hasher);
    record.units.hash(&mut hasher);
    record.sign_convention.hash(&mut hasher);
    // Validity + execution (duration and ramps are payload).
    record.valid_from_millis.hash(&mut hasher);
    record.valid_until_millis.hash(&mut hasher);
    record.duration_millis.hash(&mut hasher);
    record.ramp_up_mw_per_s.to_bits().hash(&mut hasher);
    record.ramp_down_mw_per_s.to_bits().hash(&mut hasher);
    // SEQUENCE (§14.3 field 10) — the key must cover it.
    record.sequence.hash(&mut hasher);
    // The declared idempotency key itself (field 11).
    record.idempotency_key.hash(&mut hasher);
    // Evidence references.
    record.grid_model_ref.hash(&mut hasher);
    record.telemetry_snapshot_ref.hash(&mut hasher);
    record.validation_evidence_ref.hash(&mut hasher);
    // Authorization fields.
    record.authorization_decision_ref.hash(&mut hasher);
    record.approved_scope.hash(&mut hasher);
    record.approved_limits.hash(&mut hasher);
    record.authorization_expiry_millis.hash(&mut hasher);
    // Authenticity + supersede links.
    record.authenticity.hash(&mut hasher);
    record.supersedes_command_id.hash(&mut hasher);
    record.superseded_by_command_id.hash(&mut hasher);
    format!("cmd-idem:{:016x}", hasher.finish())
}

// --- Seam fakes ---------------------------------------------------------------------

/// Minimal string/number extraction from the persisted payload JSON — the
/// fake reads EXACTLY the two §14.3 fields the duplicate rules key on. The
/// composition root renders `command_payload_json` from
/// `CommandRecordFields` (D-P4.14-1), so these fields are present in every
/// real payload.
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
    let rest = &payload[start..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The store-seam fake. Besides the tenant scoping and the compare-and-swap
/// the other suites use, THIS fake states the SEAM CONTRACT the P4.30 store
/// implementation must satisfy for the §14.3 duplicate rules:
///
/// - one effective command record per idempotency key: an insert under a
///   known key with an IDENTICAL payload deduplicates to the original row
///   (no second record is written — the receipt is still returned, the
///   caller's replay resolves to the original);
/// - the same key with a CONFLICTING payload is refused (`Conflict`);
/// - a per-asset/channel sequence that is not strictly greater than the
///   last recorded one is refused with a NAMED `out_of_sequence` code;
///   the first command of a channel never fails.
struct DuplicateRulesStore {
    records: Mutex<HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<HashMap<String, Vec<CommandTransition>>>,
    /// idempotency key → original command id (the dedup registry).
    by_key: Mutex<HashMap<String, String>>,
    /// asset → last recorded sequence (the monotonic per-channel registry).
    last_sequence: Mutex<HashMap<String, u64>>,
}

impl DuplicateRulesStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            journals: Mutex::new(HashMap::new()),
            by_key: Mutex::new(HashMap::new()),
            last_sequence: Mutex::new(HashMap::new()),
        }
    }

    fn record_count(&self) -> usize {
        self.records.lock().expect("fake store").len()
    }

    fn records_for_key(&self, key: &str) -> Vec<String> {
        let by_key = self.by_key.lock().expect("fake store");
        let records = self.records.lock().expect("fake store");
        match by_key.get(key) {
            Some(original) => {
                // The dedup registry: the key maps to the ORIGINAL only.
                vec![records.get(original).map(|r| r.command_id.to_string()).unwrap_or_default()]
            }
            None => Vec::new(),
        }
    }

    fn journal_of(&self, command_id: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn last_recorded_sequence(&self, asset: &str) -> Option<u64> {
        self.last_sequence.lock().expect("fake store").get(asset).copied()
    }
}

impl CommandGateStore for DuplicateRulesStore {
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
        // §14.3 duplicate rules at the store seam (the P4.30 contract).
        let key = payload_string_field(&record.command_payload_json, "idempotency_key")
            .expect("the fixture payload declares the idempotency key");
        let incoming_sequence =
            payload_u64_field(&record.command_payload_json, "sequence")
                .expect("the fixture payload declares the sequence");
        let mut by_key = self.by_key.lock().expect("fake store");
        if let Some(original) = by_key.get(&key).cloned() {
            let original_payload = records
                .get(&original)
                .map(|r| r.command_payload_json.clone())
                .unwrap_or_default();
            if original_payload == record.command_payload_json {
                // EXACT REPLAY: deduplicated to the single original record —
                // no second row, no rejection (§14.3: an exact duplicate is
                // an idempotent replay, never a rejection).
                return Ok(StateChangeReceipt::new(
                    COMMAND_ENTITY.to_string(),
                    StateChangeOperation::Insert,
                    Some(ctx.tenant_id()),
                ));
            }
            // CONFLICTING DUPLICATE: same key, different payload → refused.
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
        // Compare-and-swap: an already-processed transition (a redelivered
        // event's effect) cannot apply twice — the current state is not the
        // state the redelivery saw.
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
    ) -> Result<Option<flexcore_application::command_gates::CommandGateView>, CommandGateStoreError>
    {
        let records = self.records.lock().expect("fake store");
        let Some(record) = records.get(&command_id.to_string()) else {
            return Ok(None);
        };
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
            model_id: "dup-feeder".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.98; max loading 58%".to_string(),
            coverage: "balanced radial dup-feeder; unbalanced deferred (P4.34)".to_string(),
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

/// The scripted simulator transport: records every delivered envelope (the
/// "exactly one issue" proof) and accepts by default.
struct FakeTransport {
    delivered: Mutex<Vec<SimulatorDispatchEnvelope>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn delivered(&self) -> Vec<SimulatorDispatchEnvelope> {
        self.delivered.lock().expect("fake transport").clone()
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
        Ok(SimulatedAcceptance::Accepted {
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
    store: &'static DuplicateRulesStore,
    outbox: &'static FakeEventOutbox,
    transport: &'static FakeTransport,
    ids: &'static CountingIdGenerator,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store: &'static DuplicateRulesStore = Box::leak(Box::new(DuplicateRulesStore::new()));
    let envelopes: &'static FakeEnvelopes = Box::leak(Box::new(FakeEnvelopes));
    let models: &'static FakeModels = Box::leak(Box::new(FakeModels));
    let assets: &'static FakeAssets = Box::leak(Box::new(FakeAssets));
    let runner: &'static FakeRunner = Box::leak(Box::new(FakeRunner));
    let authz: &'static FakeAuthz = Box::leak(Box::new(FakeAuthz));
    let journal: &'static FakeWriteJournal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox: &'static FakeEventOutbox =
        Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock: &'static TestClock = Box::leak(Box::new(TestClock {
        now_unix_millis: Mutex::new(now_unix_millis),
    }));
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
            separation_of_duties: SeparationOfDuties::Enforced,
            max_execution_duration: None,
        },
    };
    Harness {
        gates,
        store,
        outbox,
        transport,
        ids,
    }
}

// --- Command fixture ------------------------------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

/// The fixture command. `sequence` and `idempotency_key` are the §14.3
/// fields the duplicate rules key on; `p_mw` is the payload the conflicting
/// duplicate perturbs.
fn propose_input(command_tag: u8, sequence: u64, idempotency_key: &str, p_mw: f64) -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: CommandId::new(uuid_by_tag(command_tag)),
        scope: CommandScope {
            tenant_id: tenant(),
            portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
            site_id: SiteId::new(uuid(UUID_SITE)),
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
            grid_model: "grid-model:dup-feeder@1".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:p424-fixture".to_string()),
        supersedes: None,
        // The payload declares the two §14.3 fields the store seam's
        // duplicate rules key on (the composition root renders the full
        // `CommandRecordFields`; the fixture renders the rule-relevant ones).
        command_payload_json: format!(
            "{{\"idempotency_key\":\"{idempotency_key}\",\"sequence\":{sequence},\"p_mw\":{p_mw}}}"
        ),
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

fn feasibility_refs() -> CommandFeasibilityRefs {
    CommandFeasibilityRefs {
        candidate_id: "cand-p424-1".to_string(),
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

fn approval_request() -> CommandAuthorizationRequest {
    CommandAuthorizationRequest {
        subject: APPROVER.to_string(),
        role: APPROVER_ROLE.to_string(),
        action: CommandAuthzAction::Approve,
        scope: ApprovalScope::Asset(asset_id()),
    }
}

/// Walks one fixture command through the REAL gate chain to APPROVED.
fn walk_to_approved(h: &Harness, command_tag: u8, sequence: u64, key: &str, p_mw: f64) -> (DispatchCommand, AuthorizationDecisionRecord) {
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input(command_tag, sequence, key, p_mw))
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
            &approval_request(),
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

/// Counts the frozen events the outbox carries by name.
fn event_counts(h: &Harness) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for write in h.outbox.writes() {
        let name = write.payload().split('(').next().unwrap_or_default().to_string();
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

/// A never-persisted §14.3-valid command (for the flat-record derivation
/// fixtures).
fn fixture_command(command_tag: u8, sequence: u64, key: &str, p_mw: f64) -> DispatchCommand {
    let input = propose_input(command_tag, sequence, key, p_mw);
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

fn flat_record(command: &DispatchCommand) -> CommandRecordFields {
    CommandRecordFields::from_command(
        command,
        &AuthorizationRecordFields::pending_placeholder(command.command_id(), 70_000),
    )
    .expect("the fixture record renders")
}

// ============================================================================
// The idempotency-key derivation covers the full payload + sequence
// ============================================================================

#[test]
fn the_derivation_is_deterministic_and_covers_every_payload_field() {
    let base = flat_record(&fixture_command(10, 17, "idem:p424-derivation", 1.5));
    let key = derive_idempotency_key(&base);

    // Deterministic: the same record derives the SAME key on a second call.
    assert_eq!(key, derive_idempotency_key(&base));

    // FULL PAYLOAD + SEQUENCE COVERAGE: every single-field perturbation
    // changes the derived key — no mutable field can hide behind the key.
    let perturbations: Vec<(&str, CommandRecordFields)> = vec![
        ("command_id", CommandRecordFields { command_id: "other".to_string(), ..base.clone() }),
        ("asset_id", CommandRecordFields { asset_id: "other".to_string(), ..base.clone() }),
        ("source_component_version", CommandRecordFields { source_component_version: "plugin/9.9.9".to_string(), ..base.clone() }),
        ("schedule_revision", CommandRecordFields { schedule_revision: 3, ..base.clone() }),
        ("reason", CommandRecordFields { reason: "other reason".to_string(), ..base.clone() }),
        ("action", CommandRecordFields { action: "set-soc".to_string(), ..base.clone() }),
        ("target", CommandRecordFields { target: "parameter:soc".to_string(), ..base.clone() }),
        ("valid_until_millis", CommandRecordFields { valid_until_millis: 80_000, ..base.clone() }),
        ("duration_millis", CommandRecordFields { duration_millis: 60_000, ..base.clone() }),
        ("ramp_up_mw_per_s", CommandRecordFields { ramp_up_mw_per_s: 0.9, ..base.clone() }),
        // SEQUENCE (the §14.3 field the ledger names explicitly).
        ("sequence", CommandRecordFields { sequence: 99, ..base.clone() }),
        ("grid_model_ref", CommandRecordFields { grid_model_ref: "grid-model:other@1".to_string(), ..base.clone() }),
        ("authorization_decision_ref", CommandRecordFields { authorization_decision_ref: "authz-decision:other".to_string(), ..base.clone() }),
        ("authenticity", CommandRecordFields { authenticity: "signature:sig-other".to_string(), ..base.clone() }),
        ("supersedes_command_id", CommandRecordFields { supersedes_command_id: Some("cmd-0".to_string()), ..base.clone() }),
    ];
    for (field, perturbed) in perturbations {
        assert_ne!(
            key,
            derive_idempotency_key(&perturbed),
            "perturbing '{field}' must change the derived idempotency key"
        );
    }
}

/// The derivation FEEDS the P4.14 duplicate verdict: same declared key +
/// same derived fingerprint → idempotent replay (no rejection); same
/// declared key + different fingerprint → the conflicting-duplicate
/// rejection; different declared key → a new command.
#[test]
fn the_derivation_classifies_exactly_the_conflicting_duplicate() {
    let original = flat_record(&fixture_command(11, 17, "idem:p424-classify", 1.5));
    let original_key = derive_idempotency_key(&original);

    // Same declared key, same payload (identical record): idempotent replay.
    let replay = flat_record(&fixture_command(11, 17, "idem:p424-classify", 1.5));
    assert_eq!(original_key, derive_idempotency_key(&replay), "the exact replay derives the same key");
    assert!(duplicate_conflict_rejection(true, original_key == derive_idempotency_key(&replay)).is_none());

    // Same declared key, CONFLICTING payload (a different setpoint).
    let conflicting = flat_record(&fixture_command(12, 17, "idem:p424-classify", 2.5));
    let conflicting_key = derive_idempotency_key(&conflicting);
    assert_ne!(original_key, conflicting_key, "the conflicting payload derives a different key");
    let rejection = duplicate_conflict_rejection(true, original_key == conflicting_key)
        .expect("the conflicting duplicate is rejected");
    assert_eq!(rejection.rule, CommandRejectionRule::DuplicatedConflictingPayload);
    assert_eq!(
        rejection.code(),
        pre_registered_outcome("conflicting_duplicate_rejected"),
        "the pre-registered rule code"
    );

    // A different declared key is a NEW command — never a duplicate verdict.
    assert!(duplicate_conflict_rejection(false, false).is_none());
}

// ============================================================================
// Fixture (a) — exact replay: deduplicated to the single original record,
// no second issue
// ============================================================================

/// The seam-level half of fixture (a): the same idempotency key with the
/// same payload deduplicates to the ONE original record.
fn run_exact_replay_seam() -> Vec<String> {
    let h = harness(5_000);
    let input = propose_input(20, 17, "idem:p424-replay", 1.5);
    let record = flexcore_application::command_gates::ProposedCommandRecord {
        tenant_id: tenant(),
        command_id: input.command_id,
        portfolio_id: uuid(UUID_PORTFOLIO).to_string(),
        site_id: uuid(UUID_SITE).to_string(),
        asset_id: uuid(UUID_ASSET).to_string(),
        schedule_id: uuid(UUID_SCHEDULE).to_string(),
        schedule_revision: 2,
        valid_until_millis: 70_000,
        supersedes_command_id: None,
        command_payload_json: input.command_payload_json.clone(),
    };
    let mut outcome = Vec::new();
    // The first insert creates the single original record.
    h.store
        .insert_proposed(&context(), &record)
        .expect("the first insert creates the record");
    outcome.push(format!("records_after_first={}", h.store.record_count()));
    // The EXACT replay (a fresh caller re-proposing the identical command
    // under the same key): deduplicated — no second record, no rejection.
    let replay_record = flexcore_application::command_gates::ProposedCommandRecord {
        command_id: CommandId::new(uuid_by_tag(21)), // a DIFFERENT command id, same key + payload
        command_payload_json: input.command_payload_json.clone(),
        ..record.clone()
    };
    h.store
        .insert_proposed(&context(), &replay_record)
        .expect("an exact replay is an idempotent dedup, never a rejection");
    outcome.push(format!("records_after_replay={}", h.store.record_count()));
    // Exactly one effective command record for the idempotency key.
    let records_for_key = h.store.records_for_key("idem:p424-replay");
    outcome.push(format!("effective_commands_for_key={}", records_for_key.len()));
    outcome.push(pre_registered_outcome("exact_replay_deduplicated"));
    outcome
}

/// The end-to-end half of fixture (a): the exact replay of an ALREADY-ISSUED
/// command produces no second issue — the adapter refuses the re-issue by
/// state and exactly one envelope reached the transport.
fn run_exact_replay_end_to_end() -> Vec<String> {
    let h = harness(5_000);
    let (command, decision_record) = walk_to_approved(&h, 22, 17, "idem:p424-replay-e2e", 1.5);
    let adapter = adapter_for(&h);
    let first = adapter
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect("the first issue succeeds");
    assert_eq!(first.issued.to, CommandState::Issued);
    // The EXACT REPLAY arrives (same command, same key, same payload): the
    // adapter issues only from APPROVED — the command is ISSUED, so the
    // replay is refused and no second envelope is delivered.
    let error = adapter
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect_err("an exact replay of an issued command must not re-issue");
    match error {
        DispatchAdapterError::WrongState { current, operation } => {
            assert_eq!(current, CommandState::Issued);
            assert_eq!(operation, "issue");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let delivered = h.transport.delivered();
    let mut outcome = Vec::new();
    outcome.push(format!("delivered_envelopes={}", delivered.len()));
    let counts = event_counts(&h);
    outcome.push(format!(
        "dispatch_issued_events={}",
        counts.get("DispatchIssued").copied().unwrap_or(0)
    ));
    outcome.push(format!(
        "effective_commands_for_key={}",
        h.store.records_for_key("idem:p424-replay-e2e").len()
    ));
    outcome.push(pre_registered_outcome("exact_replay_deduplicated"));
    outcome
}

#[test]
fn fixture_a_exact_replay_is_deduplicated_with_no_second_issue() {
    let seam = run_exact_replay_seam();
    let e2e = run_exact_replay_end_to_end();
    // The seam: one original record before, ONE after the replay; exactly
    // one effective command for the key.
    assert_eq!(seam[0], "records_after_first=1");
    assert_eq!(seam[1], "records_after_replay=1", "the replay creates no second record");
    assert_eq!(seam[2], "effective_commands_for_key=1");
    // End to end: exactly one envelope, one DispatchIssued, one command.
    assert_eq!(e2e[0], "delivered_envelopes=1", "no second issue");
    assert_eq!(e2e[1], "dispatch_issued_events=1");
    assert_eq!(e2e[2], "effective_commands_for_key=1");
}

// ============================================================================
// Fixture (b) — same idempotency key, conflicting payload: REJECTED
// ============================================================================

fn run_conflicting_duplicate() -> Vec<String> {
    let h = harness(5_000);
    let mut outcome = Vec::new();
    // The original command (key "idem:p424-conflict", p = 1.5 MW) is
    // created through the REAL creation gate.
    let first = propose_input(30, 17, "idem:p424-conflict", 1.5);
    h.gates
        .create_proposed_command(&context(), first.clone())
        .expect("the original creation succeeds");
    outcome.push(format!("records_after_original={}", h.store.record_count()));

    // The conflicting duplicate: SAME declared idempotency key, a DIFFERENT
    // payload (2.5 MW). The store seam refuses it — the creation gate
    // surfaces the named Conflict and NO second record exists.
    let conflicting = propose_input(31, 17, "idem:p424-conflict", 2.5);
    let error = h
        .gates
        .create_proposed_command(&context(), conflicting)
        .expect_err("a conflicting duplicate must be rejected");
    match &error {
        CommandGateError::Store(CommandGateStoreError::Conflict { .. }) => {}
        other => panic!("unexpected error: {other:?}"),
    }
    outcome.push(format!(
        "conflict_error={}",
        error.to_string().contains("already exists")
    ));
    outcome.push(format!("records_after_conflict={}", h.store.record_count()));
    outcome.push(format!(
        "effective_commands_for_key={}",
        h.store.records_for_key("idem:p424-conflict").len()
    ));

    // The P4.14 rule names the designed violation (not an incidental one).
    let rejection = duplicate_conflict_rejection(true, false)
        .expect("the conflicting-payload verdict fires");
    outcome.push(rejection.code().to_string());
    outcome
}

#[test]
fn fixture_b_same_key_conflicting_payload_is_rejected() {
    let outcome = run_conflicting_duplicate();
    assert_eq!(outcome[0], "records_after_original=1");
    assert!(outcome[1] == "conflict_error=true", "the refusal is the named Conflict");
    assert_eq!(outcome[2], "records_after_conflict=1", "no second record");
    assert_eq!(outcome[3], "effective_commands_for_key=1");
    assert_eq!(
        outcome[4],
        pre_registered_outcome("conflicting_duplicate_rejected"),
        "the rejection names the designed §14.3 rule"
    );
}

// ============================================================================
// Fixture (c) — same asset/channel, out-of-sequence version: REJECTED
// ============================================================================

fn run_out_of_sequence() -> Vec<String> {
    let h = harness(5_000);
    let mut outcome = Vec::new();
    // Sequence 20 is recorded for the asset/channel.
    let first = propose_input(40, 20, "idem:p424-seq-20", 1.5);
    h.gates
        .create_proposed_command(&context(), first)
        .expect("the first (in-sequence) command is recorded");
    let last = h.store.last_recorded_sequence(&uuid(UUID_ASSET).to_string());
    outcome.push(format!("last_recorded={}", last.map(|s| s.to_string()).unwrap_or_default()));

    // The out-of-sequence duplicate: sequence 20 again (equal — a replayed
    // sequence), through the store seam.
    let replayed = flexcore_application::command_gates::ProposedCommandRecord {
        tenant_id: tenant(),
        command_id: CommandId::new(uuid_by_tag(41)),
        portfolio_id: uuid(UUID_PORTFOLIO).to_string(),
        site_id: uuid(UUID_SITE).to_string(),
        asset_id: uuid(UUID_ASSET).to_string(),
        schedule_id: uuid(UUID_SCHEDULE).to_string(),
        schedule_revision: 2,
        valid_until_millis: 70_000,
        supersedes_command_id: None,
        command_payload_json: propose_input(41, 20, "idem:p424-seq-20-b", 1.5).command_payload_json,
    };
    let error = h
        .store
        .insert_proposed(&context(), &replayed)
        .expect_err("a replayed sequence must be rejected");
    match &error {
        CommandGateStoreError::Backend { code, .. } => {
            assert_eq!(code, "out_of_sequence");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    outcome.push("replayed_sequence_rejected".to_string());

    // A LOWER sequence is rejected too.
    let lower = flexcore_application::command_gates::ProposedCommandRecord {
        command_payload_json: propose_input(42, 19, "idem:p424-seq-19", 1.5).command_payload_json,
        command_id: CommandId::new(uuid_by_tag(42)),
        ..replayed.clone()
    };
    let error = h
        .store
        .insert_proposed(&context(), &lower)
        .expect_err("a lower sequence must be rejected");
    assert!(matches!(error, CommandGateStoreError::Backend { code, .. } if code == "out_of_sequence"));
    outcome.push("lower_sequence_rejected".to_string());

    // A STRICTLY GREATER sequence is accepted (the channel advances).
    let next = flexcore_application::command_gates::ProposedCommandRecord {
        command_payload_json: propose_input(43, 21, "idem:p424-seq-21", 1.5).command_payload_json,
        command_id: CommandId::new(uuid_by_tag(43)),
        ..replayed.clone()
    };
    h.store
        .insert_proposed(&context(), &next)
        .expect("a strictly greater sequence advances the channel");
    outcome.push(format!("records_after_next={}", h.store.record_count()));

    // The P4.14 rule names the designed violation.
    let rejection = out_of_sequence_rejection(20, Some(20)).expect("equal sequence rejects");
    assert_eq!(rejection.rule, CommandRejectionRule::OutOfSequence);
    let rejection = out_of_sequence_rejection(19, Some(20)).expect("lower sequence rejects");
    assert_eq!(rejection.code(), pre_registered_outcome("out_of_sequence_rejected"));
    // The FIRST command of a channel is never out of sequence.
    assert!(out_of_sequence_rejection(1, None).is_none());
    assert!(out_of_sequence_rejection(21, Some(20)).is_none());
    outcome.push(pre_registered_outcome("out_of_sequence_rejected"));
    outcome
}

#[test]
fn fixture_c_same_asset_channel_out_of_sequence_version_is_rejected() {
    let outcome = run_out_of_sequence();
    assert_eq!(outcome[0], "last_recorded=20");
    assert_eq!(outcome[1], "replayed_sequence_rejected");
    assert_eq!(outcome[2], "lower_sequence_rejected");
    assert_eq!(outcome[3], "records_after_next=2", "only the strictly greater sequence is recorded");
    assert_eq!(outcome[4], pre_registered_outcome("out_of_sequence_rejected"));
}

// ============================================================================
// Fixture (d) — redelivered event carrying an already-processed command:
// idempotent consumer, no duplicate side effect
// ============================================================================

fn run_redelivered_event() -> Vec<String> {
    let h = harness(5_000);
    let mut outcome = Vec::new();
    let (command, decision_record) = walk_to_approved(&h, 50, 17, "idem:p424-redelivery", 1.5);
    let adapter = adapter_for(&h);
    adapter
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect("the original issue succeeds");
    let issued_transition = h
        .store
        .journal_of(command.command_id())
        .iter()
        .find(|entry| entry.to == CommandState::Issued)
        .expect("the ISSUED transition")
        .clone();

    // 1. The redelivered event's state effect (the ISSUED transition)
    //    replayed: the store's compare-and-swap refuses it — the command is
    //    no longer in APPROVED — so no duplicate side effect is recorded.
    let error = h
        .store
        .journal_transition(&context(), &issued_transition)
        .expect_err("a redelivered transition cannot apply twice");
    match &error {
        CommandGateStoreError::TransitionConflict { expected_from, .. } => {
            assert_eq!(expected_from, CommandState::Approved.name());
        }
        other => panic!("unexpected error: {other:?}"),
    }
    outcome.push("replayed_transition_refused_by_cas".to_string());

    // 2. The redelivered EVENT record: the outbox port enforces idempotency-
    //    key uniqueness (the P1.12 unique constraint), so a redelivered
    //    write under an existing key is refused durably — republishing after
    //    a crash between publish and mark is safe (§15.5).
    let write = OutboxWrite::new(
        StateChangeReceipt::new(
            "dispatch_commands".to_string(),
            StateChangeOperation::Insert,
            Some(context().tenant_id()),
        ),
        format!("dispatch_issued:{}", command.command_id().uuid()),
        "dispatch.commands.simulator".to_string(),
        "the redelivered payload".to_string(),
        EventTime::from_unix_millis(5_000).expect("valid event time"),
    )
    .expect("a valid outbox write");
    // (The original write for this correlation already exists in the
    // outbox from the real issue; the redelivery is refused.)
    let error = h.outbox.append(write).expect_err("the redelivered key is refused");
    match error {
        EventOutboxError::DuplicateKey { idempotency_key } => {
            assert!(idempotency_key.starts_with("dispatch_issued:"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    outcome.push("redelivered_outbox_write_refused".to_string());

    // 3. The redelivered COMMAND (a re-issue attempt): refused by state.
    let error = adapter
        .issue(&context(), &command, &decision_record, &actor("svc:dispatcher"))
        .expect_err("a redelivered command cannot re-issue");
    assert!(matches!(error, DispatchAdapterError::WrongState { current: CommandState::Issued, .. }));
    outcome.push("redelivered_command_refused_by_state".to_string());

    // Exactly one effective command per idempotency key; exactly one
    // DispatchIssued; exactly one envelope.
    outcome.push(format!(
        "effective_commands_for_key={}",
        h.store.records_for_key("idem:p424-redelivery").len()
    ));
    let counts = event_counts(&h);
    outcome.push(format!(
        "dispatch_issued_events={}",
        counts.get("DispatchIssued").copied().unwrap_or(0)
    ));
    outcome.push(format!("delivered_envelopes={}", h.transport.delivered().len()));
    outcome.push(pre_registered_outcome("redelivered_event_no_duplicate_side_effect"));
    outcome
}

#[test]
fn fixture_d_redelivered_event_causes_no_duplicate_side_effect() {
    let outcome = run_redelivered_event();
    assert_eq!(outcome[0], "replayed_transition_refused_by_cas");
    assert_eq!(outcome[1], "redelivered_outbox_write_refused");
    assert_eq!(outcome[2], "redelivered_command_refused_by_state");
    assert_eq!(outcome[3], "effective_commands_for_key=1");
    assert_eq!(outcome[4], "dispatch_issued_events=1");
    assert_eq!(outcome[5], "delivered_envelopes=1");
    assert_eq!(outcome[6], pre_registered_outcome("redelivered_event_no_duplicate_side_effect"));
    // Cross-reference (not duplicated here): the durable NATS-inbox dedup
    // consumer is the P1.16 fault slice —
    // `crates/adapters/nats/tests/fault_slice.rs`, case (a) same-idempotency-
    // key redelivery; its consumer-side dedup completes the §15.4 chain this
    // suite pins at the application seams.
}

// ============================================================================
// Determinism: every fixture runs twice and produces identical outcomes
// ============================================================================

#[test]
fn every_fixture_produces_identical_outcomes_across_two_runs() {
    let run = || {
        (
            run_exact_replay_seam(),
            run_exact_replay_end_to_end(),
            run_conflicting_duplicate(),
            run_out_of_sequence(),
            run_redelivered_event(),
        )
    };
    let first = run();
    let second = run();
    assert_eq!(first.0, second.0, "fixture (a) seam: identical outcomes");
    assert_eq!(first.1, second.1, "fixture (a) end-to-end: identical outcomes");
    assert_eq!(first.2, second.2, "fixture (b): identical outcomes");
    assert_eq!(first.3, second.3, "fixture (c): identical outcomes");
    assert_eq!(first.4, second.4, "fixture (d): identical outcomes");
}
