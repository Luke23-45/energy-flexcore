// P4.21 — Deterministic rejection: stale commands (report §23 Stage 4
//         exit gate, §14.3, §17.5 invariant 5, §10.4; ledger P4.21;
//         dependencies P4.11, P4.14–P4.15 — all done). Decision record
//         with the PRE-REGISTERED per-fixture expectation table (written
//         BEFORE the assertions, and UNRUN at authoring time):
//         `ledger/phase-04-grid-feasibility-dispatch/
//         20_22_rejection_suites_a.md`.
//
// Coverage per the ledger acceptance criteria:
//
// 1. The fixtures: a command whose `valid-until` has passed before
//    validation (F-ST-1); a command built on a SUPERSEDED schedule
//    revision (F-ST-2); a command built on STALE telemetry (quality STALE
//    at the snapshot) where freshness is required (F-ST-3); a telemetry
//    snapshot older than the CONFIGURED maximum age (F-ST-4).
// 2. Pre-registered outcomes: REJECTED vs EXPIRED, with the reason
//    recorded — a command still at PROPOSED when its window ends is
//    REJECTED (the §14.3 STALE rule; PROPOSED has no frozen EXPIRED edge),
//    while a command already APPROVED/ISSUED/ACKNOWLEDGED/TRACKING when
//    validity ended moves to EXPIRED with the milestone not reached
//    (F-ST-5 demonstrates the APPROVED case).
// 3. CONTROLLED CLOCK EVERYWHERE: every timestamp in this suite comes
//    from the injected, advanceable `TestClock` — no wall-clock read
//    exists in the suite, so the tests are wall-clock independent by
//    construction (the ledger's flake risk is structurally excluded).
// 4. Clock skew: a command timestamp in the future beyond the configured
//    tolerance is rejected, not accepted (F-ST-6). The tolerance the
//    platform implements is ZERO at the time-model boundary (the ingest
//    ordering rule: `ingest_time ≥ event_time` — any future-dated
//    timestamp is refused with the named `IngestBeforeEvent` error); the
//    limitation this records is in the decision record.
//
// Determinism: every fixture is run TWICE with identical inputs and the
// two outcomes are asserted identical. CI wiring: CI-runnable by
// construction through the `ci` workflow's `unit-tests` job
// (`cargo test --workspace`; equivalently `cargo test -p
// flexcore-application`). Simulation-only 0.x (§21.4): nothing here
// issues anything to anything.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_expiry::{
    sweep_validity_windows, CommandExpiryError, CommandExpiryService, EventScope, ExpiryCandidate,
    ExpiryOutcome,
};
use flexcore_application::command_gates::{
    AssetCapabilitySource, AssetGateStatus, CommandGateError, CommandGateStore,
    CommandGateStoreError, CommandGateView, CommandGates, CreationCheck, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposeCommandInput, ProposedCommandRecord, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::command_record::{stale_rejection, CommandRejectionRule};
use flexcore_application::envelope_staleness::{
    evaluate_envelope_trigger, evaluate_max_age, EnvelopeFreshness, EnvelopeTriggerEvent,
    StalenessConfig, StalenessReason,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::flexibility_envelope::{EnvelopeMetadata, EnvelopeVersion};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{
    validate_ingest_after_event, EventTime, IngestTime, TimeModelError, ValidFrom, ValidUntil,
    ValidityWindow,
};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, EvidenceReference, ExpiryMilestone, Gate, LifecycleClock,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, EvidenceReferences, ExecutionRequirements, RampLimits,
    ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::events::TelemetryQualityChangedPayload;
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f04";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f05";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7f06";

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

fn asset_id() -> AssetId {
    AssetId::new(uuid(UUID_ASSET))
}

const PROPOSER: &str = "service:scheduler";
const MODEL_HASH: &str = "fnv1a64:modelhash01";
const VALIDATOR: &str = "svc:validator";

/// The configured maximum age for F-ST-4 (a CONFIGURATION value recorded
/// in the decision record; DEFAULT label — the one-hour recorded default
/// of `DEFAULT_MAX_AGE_MILLIS` is pinned by its own test).
const CONFIGURED_MAX_AGE_MILLIS: i64 = 60_000;

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

// --- Seam fakes (fail-closed by construction) --------------------------------------

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

    /// Seeds a record + forward journal up to `state` (the expiry fixture
    /// needs a command already at APPROVED when validity ends). Every
    /// journal replays cleanly through the P4.14 replay (the gate evidence
    /// is present at every gate row).
    fn seed_at_state(&self, command: CommandId, valid_until_millis: i64, state: CommandState) {
        self.records
            .lock()
            .expect("fake store")
            .insert(
                command.to_string(),
                ProposedCommandRecord {
                    tenant_id: tenant(),
                    command_id: command,
                    portfolio_id: uuid(UUID_PORTFOLIO).to_string(),
                    site_id: uuid(UUID_SITE).to_string(),
                    asset_id: uuid(UUID_ASSET).to_string(),
                    schedule_id: ScheduleId::new(uuid(UUID_SCHEDULE)).to_string(),
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
}

/// One journalled entry at a strictly increasing controlled instant.
fn entry_at(
    command: CommandId,
    from: CommandState,
    to: CommandState,
    gate: Option<Gate>,
    at_unix_millis: i64,
    evidence: &str,
) -> CommandTransition {
    CommandTransition {
        command_id: command,
        from,
        to,
        actor: Actor::new("test:gate").expect("actor"),
        at_unix_millis,
        gate,
        reason: None,
        evidence: Some(EvidenceReference::new(evidence).expect("evidence")),
    }
}

/// The forward gate walk to `state` (the evidence the P4.14 replay
/// requires at every gate row).
fn forward_journal_to(command: CommandId, state: CommandState) -> Vec<CommandTransition> {
    let mut journal = Vec::new();
    let mut at = 1_000i64;
    let mut push = |from: CommandState, to: CommandState, gate: Option<Gate>, evidence: &str| {
        journal.push(entry_at(command, from, to, gate, at, evidence));
        at += 1_000;
    };
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
    journal
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

/// The envelope-freshness seam: the verdict is set per fixture, derived
/// from the REAL staleness evaluation (the composition-root shape: the
/// pure evaluation decides, the seam carries it to the gate).
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

impl flexcore_application::command_gates::CommandFeasibilityRunner for FakeRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &flexcore_application::command_gates::CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        Ok(FeasibilityGateVerdict::Passed {
            attempt_id: "attempt-1".to_string(),
            model_id: "feeder-a".to_string(),
            model_content_hash: MODEL_HASH.to_string(),
            margins: "min v_pu 0.97".to_string(),
            coverage: "balanced radial feeder-a".to_string(),
        })
    }
}

struct FakeAuthz;

impl flexcore_application::command_gates::CommandAuthorizationEvaluator for FakeAuthz {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        _request: &flexcore_application::command_gates::CommandAuthorizationRequest,
    ) -> flexcore_application::command_gates::AuthorizationDecision {
        flexcore_application::command_gates::AuthorizationDecision::Denied {
            reason: "no approvals in the stale-command suite".to_string(),
        }
    }
}

/// Renders the event's Debug form (which names the frozen variant).
struct DebugEncoder;

impl EventPayloadEncoder for DebugEncoder {
    fn encode(&self, envelope: &flexcore_events::envelope::EventEnvelope) -> Result<String, ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

// --- Harness ------------------------------------------------------------------------

struct Harness {
    gates: CommandGates<'static>,
    expiry: CommandExpiryService<'static>,
    store: &'static FakeStore,
    envelopes: &'static FakeEnvelopes,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
}

fn harness(now_unix_millis: i64) -> Harness {
    let store = Box::leak(Box::new(FakeStore::new()));
    let envelopes = Box::leak(Box::new(FakeEnvelopes {
        verdict: Mutex::new(EnvelopeGateFreshness::Fresh),
    }));
    let models = Box::leak(Box::new(FakeModels));
    let assets = Box::leak(Box::new(FakeAssets));
    let runner = Box::leak(Box::new(FakeRunner));
    let authz = Box::leak(Box::new(FakeAuthz));
    let journal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock = Box::leak(Box::new(TestClock::at(now_unix_millis)));
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
        policy: flexcore_application::command_gates::CommandGatePolicy::default(),
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
        outbox,
        clock,
    }
}

// --- Command fixture (pinned inputs) --------------------------------------------------

fn validity_window(valid_from_millis: i64, valid_until_millis: i64) -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(valid_from_millis).unwrap(),
        ValidUntil::from_unix_millis(valid_until_millis).unwrap(),
    )
    .unwrap()
}

fn propose_input(valid_until_millis: i64) -> ProposeCommandInput {
    ProposeCommandInput {
        command_id: command_id(),
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
            p: ActivePowerMW::new(1.5).unwrap(),
            q: Some(ReactivePowerMVAr::new(-0.2).unwrap()),
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: Some(ReactiveSignConvention::InjectionPositive),
            },
        },
        validity: validity_window(valid_until_millis - 60_000, valid_until_millis),
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

fn emitted_events(h: &Harness) -> Vec<&'static str> {
    h.outbox
        .writes()
        .iter()
        .map(|write| {
            let payload = write.payload();
            for candidate in ["DispatchProposed", "DispatchRejected", "DispatchExpired"] {
                if payload.starts_with(candidate) {
                    return candidate;
                }
            }
            "unknown"
        })
        .collect()
}

fn scope() -> EventScope {
    EventScope {
        portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
        site_id: SiteId::new(uuid(UUID_SITE)),
        asset_id: asset_id(),
    }
}

/// The envelope metadata the staleness fixtures evaluate against (pinned:
/// computed at `computed_at_millis`, scoped to the fixture asset).
fn envelope_metadata(computed_at_millis: i64) -> EnvelopeMetadata {
    EnvelopeMetadata {
        envelope_version: EnvelopeVersion::new("env-v1").expect("valid version"),
        computed_at: EventTime::from_unix_millis(computed_at_millis).expect("valid instant"),
        tenant_id: tenant(),
        portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
        input_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
        forecast_version: "fc-v1".to_string(),
        horizon_start_millis: computed_at_millis,
        horizon_end_millis: computed_at_millis + 3_600_000,
        scoped_asset_ids: vec![asset_id()],
    }
}

// ============================================================================
// F-ST-1 — valid-until passed before validation: REJECTED (not EXPIRED)
// ============================================================================

#[test]
fn f_st_1_a_command_whose_window_ended_before_validation_is_rejected_stale_not_expired() {
    // PRE-REGISTERED EXPECTATION (decision record 20_22_rejection_suites_a.md,
    // F-ST-1; UNRUN at authoring time): REJECTED with the §14.3 STALE rule
    // (code `stale`), NOT EXPIRED — the command is still at PROPOSED when
    // its window ends, and PROPOSED has no frozen EXPIRED edge (only
    // APPROVED / ISSUED / ACKNOWLEDGED / TRACKING expire, P4.15). The
    // §10.3 end-exclusive convention applies: the window is over once
    // `now >= valid_until`.
    let valid_until = 70_000i64;
    let run = |now: i64| {
        let open = stale_rejection(valid_until, now);
        let sweep = sweep_validity_windows(
            &[ExpiryCandidate {
                command_id: command_id(),
                state: CommandState::Proposed,
                valid_until_millis: valid_until,
            }],
            now,
        );
        (open, sweep)
    };

    // Before the window ends: no stale verdict (the boundary control).
    let (open_before, sweep_before) = run(valid_until - 1);
    assert!(open_before.is_none(), "the window is still open");
    assert!(sweep_before.is_empty());

    // At and after the boundary: the STALE verdict fires, end-exclusive.
    let (first_at, sweep_at) = run(valid_until);
    let rejection = first_at.as_ref().expect("stale at the end-exclusive boundary");
    assert_eq!(rejection.rule, CommandRejectionRule::Stale);
    assert_eq!(rejection.code(), "stale");
    assert!(
        rejection.detail.contains("end-exclusive"),
        "the detail records the convention: {}",
        rejection.detail
    );
    // The command is at PROPOSED, so the sweep yields NO expiry decision —
    // the recorded reason REJECTED applies, not EXPIRED.
    assert!(
        sweep_at.is_empty(),
        "PROPOSED has no frozen EXPIRED edge; the sweep must not fire"
    );

    // Determinism.
    let (second_at, second_sweep) = run(valid_until);
    assert_eq!(first_at, second_at);
    assert_eq!(sweep_at, second_sweep);
}

#[test]
fn f_st_1b_expiry_refuses_the_proposed_state_with_a_named_error() {
    // PRE-REGISTERED EXPECTATION (F-ST-1b; UNRUN at authoring time): the
    // expiry service refuses a PROPOSED command with `WrongState { current:
    // Proposed, operation: "apply_expiry" }` — reinforcing that EXPIRED is
    // unreachable from PROPOSED, so the pre-registered outcome for F-ST-1
    // is REJECTED, never EXPIRED.
    let run = || {
        let h = harness(80_000);
        h.store.seed_at_state(command_id(), 70_000, CommandState::Proposed);
        let outcome = h.expiry.apply_expiry(&context(), scope(), command_id(), &actor("test:operator"));
        let view_state = h
            .store
            .load_command(&context(), command_id())
            .expect("load")
            .expect("seeded")
            .state();
        (outcome, view_state)
    };
    let (first, first_state) = run();
    match &first {
        Err(CommandExpiryError::WrongState { current, operation }) => {
            assert_eq!(*current, CommandState::Proposed);
            assert_eq!(*operation, "apply_expiry");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(first_state, CommandState::Proposed, "the command is untouched");
    // Determinism.
    let (second, second_state) = run();
    assert_eq!(
        matches!(first, Err(CommandExpiryError::WrongState { .. })),
        matches!(second, Err(CommandExpiryError::WrongState { .. }))
    );
    assert_eq!(first_state, second_state);
}

// ============================================================================
// F-ST-2 — command built on a superseded schedule revision
// ============================================================================

#[test]
fn f_st_2_a_command_on_a_superseded_revision_is_rejected_at_creation() {
    // PRE-REGISTERED EXPECTATION (F-ST-2; UNRUN at authoring time):
    // REJECTED AT CREATION with `CreationRejected { check:
    // SourceScheduleRevisionSuperseded }` — a command may only be created
    // from the ACTIVE revision (P4.11; not merely at issue). No record, no
    // event.
    let run = || {
        let h = harness(5_000);
        h.store.seed_revision(
            &uuid(UUID_SCHEDULE).to_string(),
            2,
            ScheduleRevisionUsability::Superseded,
        );
        let outcome = h
            .gates
            .create_proposed_command(&context(), propose_input(70_000))
            .map(|_| ());
        (outcome, h.store.records.lock().expect("s").is_empty(), emitted_events(&h))
    };
    let (first, first_empty, first_events) = run();
    match &first {
        Err(CommandGateError::CreationRejected { check, detail }) => {
            assert_eq!(*check, CreationCheck::SourceScheduleRevisionSuperseded);
            assert!(detail.contains("SUPERSEDED"), "detail: {detail}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(first_empty, "no record may exist");
    assert!(first_events.is_empty());
    // Determinism.
    let (second, _, _) = run();
    assert_eq!(first, second);
}

// ============================================================================
// F-ST-3 — command built on STALE telemetry (quality STALE at the snapshot)
// ============================================================================

#[test]
fn f_st_3_a_stale_telemetry_quality_event_makes_the_envelope_stale_and_the_gate_rejects() {
    // PRE-REGISTERED EXPECTATION (F-ST-3; UNRUN at authoring time):
    // REJECTED at the validation gate with the failing check
    // `envelope_stale:<reason>` — the command's source schedule was
    // computed on an envelope whose scoped asset's telemetry turned STALE
    // (§10.4 quality state; §17.5 invariant 5). The staleness verdict is
    // produced by the REAL P3.13 evaluation (`evaluate_envelope_trigger`
    // with the frozen `TelemetryQualityChanged` payload carrying quality
    // STALE); the gate seam carries that verdict, as the composition root
    // does. The command moves PROPOSED → REJECTED and `DispatchRejected`
    // is emitted.
    let run = || {
        let h = harness(5_000);
        h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
        // The REAL staleness evaluation: a quality change to STALE on the
        // scoped asset invalidates the envelope the schedule was computed
        // against.
        let metadata = envelope_metadata(4_000);
        let event = EnvelopeTriggerEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
            correlation_id: FlexUuid::from_bytes([0u8; 16]),
            tenant_id: tenant(),
            site_id: SiteId::new(uuid(UUID_SITE)),
            asset_id: asset_id(),
            quality: TelemetryQuality::Stale,
            previous_quality: Some(TelemetryQuality::Good),
        });
        let verdict = evaluate_envelope_trigger(
            &metadata,
            &event,
            &StalenessConfig::default(),
            EventTime::from_unix_millis(5_000).expect("valid instant"),
            None,
        );
        let reason = match &verdict.freshness {
            EnvelopeFreshness::Stale { reason } => format!("{reason:?}"),
            EnvelopeFreshness::Fresh => panic!("the STALE telemetry event must invalidate"),
        };
        // The composition-root shape: the pure verdict travels on the gate
        // seam.
        *h.envelopes.verdict.lock().expect("fake envelopes") =
            EnvelopeGateFreshness::Stale { reason: reason.clone() };
        let proposed = h
            .gates
            .create_proposed_command(&context(), propose_input(70_000))
            .expect("creation succeeds");
        let result = h
            .gates
            .run_validation_gate(&context(), &proposed.command, &actor(VALIDATOR))
            .expect("the gate verdict");
        let journal = h.store.journal_of(command_id());
        (result.passed, result.failing_check, journal, emitted_events(&h))
    };
    let (first_passed, first_check, first_journal, first_events) = run();
    assert!(!first_passed);
    let check = first_check.expect("the failing check is named");
    assert!(
        check.starts_with("envelope_stale:TelemetryQualityChanged"),
        "the check names the STALE-telemetry staleness reason: {check}"
    );
    let entry = first_journal.last().expect("the rejection is journalled");
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.from, CommandState::Proposed);
    assert_eq!(entry.reason.as_deref(), Some(check.as_str()));
    assert_eq!(first_events, vec!["DispatchProposed", "DispatchRejected"]);
    // Determinism.
    let (second_passed, second_check, _, _) = run();
    assert_eq!(first_passed, second_passed);
    assert_eq!(first_check, second_check);
}

// ============================================================================
// F-ST-4 — telemetry snapshot older than the configured max age
// ============================================================================

#[test]
fn f_st_4_a_snapshot_older_than_the_configured_max_age_rejects_and_the_boundary_is_exclusive() {
    // PRE-REGISTERED EXPECTATION (F-ST-4; UNRUN at authoring time):
    // REJECTED at the validation gate with `envelope_stale:MaxAgeExceeded
    // { .. }` — the snapshot's age exceeds the CONFIGURED bound
    // (`CONFIGURED_MAX_AGE_MILLIS`, recorded in the decision record). The
    // P2.08 staleness convention is pinned at the boundary: age == max_age
    // is NOT stale; age == max_age + 1 is. The REAL P3.13 max-age
    // evaluation decides; the gate seam carries the verdict.
    let computed_at = 5_000i64;
    let run = |age: i64| {
        let h = harness(computed_at + age);
        h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
        let metadata = envelope_metadata(computed_at);
        let config = StalenessConfig {
            max_age_millis: CONFIGURED_MAX_AGE_MILLIS,
            min_recompute_interval_millis: 0,
        };
        let freshness = evaluate_max_age(
            &metadata,
            &config,
            EventTime::from_unix_millis(computed_at + age).expect("valid instant"),
        );
        if age <= CONFIGURED_MAX_AGE_MILLIS {
            // Boundary control: at or under the bound the envelope is
            // fresh and the validation gate PASSES.
            assert_eq!(freshness, EnvelopeFreshness::Fresh);
            *h.envelopes.verdict.lock().expect("fake envelopes") =
                EnvelopeGateFreshness::Fresh;
        } else {
            let reason = match &freshness {
                EnvelopeFreshness::Stale { reason } => format!("{reason:?}"),
                EnvelopeFreshness::Fresh => panic!("age {age} must be stale"),
            };
            match &freshness {
                EnvelopeFreshness::Stale {
                    reason: StalenessReason::MaxAgeExceeded { age_millis, max_age_millis },
                } => {
                    assert_eq!(*age_millis, age);
                    assert_eq!(*max_age_millis, CONFIGURED_MAX_AGE_MILLIS);
                }
                other => panic!("unexpected staleness reason: {other:?}"),
            }
            *h.envelopes.verdict.lock().expect("fake envelopes") =
                EnvelopeGateFreshness::Stale { reason };
        }
        let proposed = h
            .gates
            .create_proposed_command(&context(), propose_input(70_000))
            .expect("creation succeeds");
        let result = h
            .gates
            .run_validation_gate(&context(), &proposed.command, &actor(VALIDATOR))
            .expect("the gate verdict");
        (freshness, result.passed, result.failing_check)
    };

    // Boundary control: exactly AT the configured bound, fresh → passed.
    let (fresh_at_bound, passed_at_bound, check_at_bound) = run(CONFIGURED_MAX_AGE_MILLIS);
    assert_eq!(fresh_at_bound, EnvelopeFreshness::Fresh);
    assert!(passed_at_bound, "age == max_age is NOT stale (P2.08)");
    assert!(check_at_bound.is_none());

    // One millisecond beyond: stale → REJECTED with the check named.
    let (first_stale, first_passed, first_check) = run(CONFIGURED_MAX_AGE_MILLIS + 1);
    assert!(matches!(
        first_stale,
        EnvelopeFreshness::Stale { reason: StalenessReason::MaxAgeExceeded { .. } }
    ));
    assert!(!first_passed);
    let check = first_check.expect("the failing check is named");
    assert!(
        check.starts_with("envelope_stale:MaxAgeExceeded"),
        "the check names the configured max-age reason: {check}"
    );
    // Determinism.
    let (second_stale, second_passed, second_check) = run(CONFIGURED_MAX_AGE_MILLIS + 1);
    assert_eq!(first_stale, second_stale);
    assert_eq!(first_passed, second_passed);
    assert_eq!(first_check, second_check);
}

// ============================================================================
// F-ST-5 — already APPROVED when validity ended: EXPIRED (with milestone)
// ============================================================================

#[test]
fn f_st_5_a_command_already_approved_when_validity_ended_expires_with_the_milestone() {
    // PRE-REGISTERED EXPECTATION (F-ST-5; UNRUN at authoring time):
    // EXPIRED — the command was already APPROVED when the validity window
    // ended, and APPROVED carries the frozen APPROVED → EXPIRED edge with
    // the milestone BeforeIssue (issue never happened). The expiry time
    // and the milestone not reached are recorded; `DispatchExpired` is
    // emitted.
    let run = || {
        let h = harness(80_000);
        h.store.seed_at_state(command_id(), 70_000, CommandState::Approved);
        let outcome = h
            .expiry
            .apply_expiry(&context(), scope(), command_id(), &actor("test:operator"))
            .expect("the lapsed APPROVED command expires");
        let journal = h.store.journal_of(command_id());
        (outcome, journal, emitted_events(&h))
    };
    let (first, first_journal, first_events) = run();
    let ExpiryOutcome {
        transition,
        milestone,
        expired_at_unix_millis,
    } = &first;
    assert_eq!(transition.from, CommandState::Approved);
    assert_eq!(transition.to, CommandState::Expired);
    assert_eq!(*milestone, ExpiryMilestone::BeforeIssue);
    assert_eq!(*expired_at_unix_millis, 80_000, "the controlled clock");
    assert!(
        transition.reason.as_deref().unwrap_or_default().contains("ISSUE"),
        "the milestone not reached is the journal reason"
    );
    let entry = first_journal.last().expect("the expiry is journalled");
    assert_eq!(entry.to, CommandState::Expired);
    assert_eq!(first_events, vec!["DispatchExpired"]);
    // Determinism (fresh harness each run; identical inputs).
    let (second, _, second_events) = run();
    assert_eq!(first.transition, second.transition);
    assert_eq!(first.milestone, second.milestone);
    assert_eq!(first_events, second_events);
}

// ============================================================================
// F-ST-6 — clock skew: a future-dated timestamp is rejected, not accepted
// ============================================================================

#[test]
fn f_st_6_a_timestamp_in_the_future_beyond_the_tolerance_is_rejected() {
    // PRE-REGISTERED EXPECTATION (F-ST-6; UNRUN at authoring time):
    // REJECTED at the time-model boundary with the named
    // `TimeModelError::IngestBeforeEvent` error. The tolerance the
    // platform implements is ZERO (the ingest ordering rule
    // `ingest_time ≥ event_time`: a value cannot be received before it
    // occurred), so a command timestamp one hour in the future — the
    // classic clock-skew forward fault (§17.4) — is refused before any
    // command can be recorded. The boundary is inclusive at zero skew
    // (ingest == event is accepted). Limitation recorded in the decision
    // record: a NON-zero configurable future tolerance is not implemented
    // in the current Phase 4 surfaces.
    let run = |event_time_millis: i64| {
        let event_time = EventTime::from_unix_millis(event_time_millis).expect("valid instant");
        let ingest_time = IngestTime::from_unix_millis(5_000).expect("valid instant");
        validate_ingest_after_event(event_time, ingest_time)
    };
    // One hour of forward clock skew: REJECTED.
    let first = run(5_000 + 3_600_000);
    assert_eq!(first, Err(TimeModelError::IngestBeforeEvent));
    // Boundary: zero skew (ingest == event) is accepted; one millisecond
    // of skew is not.
    assert_eq!(run(5_000), Ok(()));
    assert_eq!(run(5_001), Err(TimeModelError::IngestBeforeEvent));
    // Determinism.
    assert_eq!(first, run(5_000 + 3_600_000));
}

// ============================================================================
// Control — the fresh fixture creates and validates (harness sanity)
// ============================================================================

/// A control, not a rejection fixture: with a FRESH envelope and an open
/// validity window, the fixture command is created and validated — every
/// rejection above is a designed rejection, not a harness artifact.
#[test]
fn the_control_fixture_creates_and_validates() {
    let h = harness(5_000);
    h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input(70_000))
        .expect("the control creation succeeds");
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor(VALIDATOR))
        .expect("the control validation verdict");
    assert!(result.passed, "the control must pass");
    assert!(result.failing_check.is_none());
    let entry = h.store.journal_of(command_id());
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0].to, CommandState::Validated);
    assert_eq!(entry[0].at_unix_millis, 5_000, "the controlled clock");
}
