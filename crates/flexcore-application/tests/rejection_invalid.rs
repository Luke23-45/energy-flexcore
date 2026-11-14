// P4.20 — Deterministic rejection: invalid commands (report §23 Stage 4
//         exit gate, §14.3, §12.5 step 1; ledger P4.20; dependencies
//         P4.04, P4.11, P4.14 — all done). Decision record with the
//         PRE-REGISTERED per-fixture expectation table (written BEFORE the
//         assertions, and UNRUN at authoring time):
//         `ledger/phase-04-grid-feasibility-dispatch/
//         20_22_rejection_suites_a.md`.
//
// Coverage per the ledger acceptance criteria:
//
// 1. A FIXED invalid-command fixture set (pinned inputs, no randomness):
//    malformed schema (a missing required §14.3 field), wrong units (a
//    kW-scaled value at the MW boundary), an inverted validity window,
//    unknown asset/tenant-scope IDs, a target P/Q outside the declared
//    capability, and a (zero — see F-IV-6) duration.
// 2. Each fixture runs through command creation and/or the validation
//    gate, with its rejection PRE-REGISTERED in the comment above the
//    assertions: the state (REJECTED or creation-refused), the failing
//    check NAMED, and the evidence reference asserted where a journal row
//    exists.
// 3. DETERMINISM: every fixture is run TWICE with identical inputs and
//    the two outcomes are asserted identical. All timestamps come from a
//    fixed controlled clock — no wall clock, no randomness.
// 4. CI wiring: an integration test under `crates/flexcore-application/
//    tests/` is CI-runnable by construction — the `ci` workflow's
//    `unit-tests` job runs `cargo test --workspace` (equivalently
//    `cargo test -p flexcore-application` for this suite), and the §22.2
//    `scenario-regression` job is the regression-gate job the ledger
//    names. No CI file was edited.
//
// Simulation-only 0.x (§21.4): nothing here issues anything to anything.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    AssetCapabilitySource, AssetGateStatus, CommandGateError, CommandGateStore,
    CommandGateStoreError, CommandGateView, CommandGates, CreationCheck, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposeCommandInput, ProposedCommandRecord,
    ScheduleEnvelopeSource, ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::command_record::{
    outside_capability_rejection, ramp_outside_capability_rejection, AssetCapabilityWindow,
    AuthorizationRecordFields, CommandRecordError, CommandRecordFields, CommandRejectionRule,
    RecordValidationPhase,
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
    DeclaredSignConventions, DispatchCommandError, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e04";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e05";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7e06";

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

const PROPOSER: &str = "service:scheduler";
const MODEL_HASH: &str = "fnv1a64:modelhash01";

/// The FIXED gate clock instant. Every timestamp in this suite is this
/// constant or an offset from it — the suite is wall-clock independent by
/// construction.
const NOW: i64 = 20_000;

// --- Controlled clock (never wall-clock) ------------------------------------------

struct FixedClock;

impl LifecycleClock for FixedClock {
    fn now_unix_millis(&self) -> i64 {
        NOW
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

/// The asset seam. `status` is a plain value so each fixture pins exactly
/// the registry answer it designs.
struct FakeAssets {
    status: AssetGateStatus,
}

impl AssetCapabilitySource for FakeAssets {
    fn check(
        &self,
        _ctx: &TenantContext,
        _asset: AssetId,
    ) -> Result<AssetGateStatus, GateSeamError> {
        Ok(self.status.clone())
    }
}

fn usable_asset() -> AssetGateStatus {
    AssetGateStatus::Usable(DeclaredQuantities {
        active_power: true,
        reactive_power: true,
        soc: true,
        max_up_ramp_mw_per_s: 2.0,
        max_down_ramp_mw_per_s: 2.0,
    })
}

struct FakeRunner;

impl flexcore_application::command_gates::CommandFeasibilityRunner for FakeRunner {
    fn run(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &ScheduleId,
        _revision: u64,
        _refs: &flexcore_application::command_gates::CommandFeasibilityRefs,
    ) -> Result<flexcore_application::command_gates::FeasibilityGateVerdict, GateSeamError> {
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
            reason: "no approvals in the invalid-command suite".to_string(),
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
    store: &'static FakeStore,
    outbox: &'static FakeEventOutbox,
}

fn harness(assets: AssetGateStatus) -> Harness {
    let store = Box::leak(Box::new(FakeStore::new()));
    let envelopes = Box::leak(Box::new(FakeEnvelopes));
    let models = Box::leak(Box::new(FakeModels));
    let assets = Box::leak(Box::new(FakeAssets { status: assets }));
    let runner = Box::leak(Box::new(FakeRunner));
    let authz = Box::leak(Box::new(FakeAuthz));
    let journal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock = Box::leak(Box::new(FixedClock));
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
    Harness { gates, store, outbox }
}

// --- Command fixture (pinned inputs) --------------------------------------------------

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(NOW + 10_000).unwrap(),
        ValidUntil::from_unix_millis(NOW + 70_000).unwrap(),
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
            for candidate in ["DispatchProposed", "DispatchRejected"] {
                if payload.starts_with(candidate) {
                    return candidate;
                }
            }
            "unknown"
        })
        .collect()
}

/// The declared capability window the P4.14 outside-capability fixtures
/// run against (configuration, recorded in the decision record).
fn capability_window() -> AssetCapabilityWindow {
    AssetCapabilityWindow {
        min_p_mw: -5.0,
        max_p_mw: 5.0,
        min_q_mvar: Some(-2.0),
        max_q_mvar: Some(2.0),
        max_up_ramp_mw_per_s: 2.0,
        max_down_ramp_mw_per_s: 2.0,
    }
}

/// A valid command (for the flat-record fixtures) with its pending
/// authorization placeholder fields.
fn valid_command_with_record() -> (flexcore_domain::dispatch_command::DispatchCommand, CommandRecordFields) {
    let input = propose_input();
    let authorization = flexcore_domain::dispatch_command::AuthorizationReference {
        decision_reference: flexcore_application::command_gates::pending_authorization_reference(
            input.command_id,
        ),
        approved_scope: "pending".to_string(),
        approved_limits: "pending".to_string(),
        expiry: ValidUntil::from_unix_millis(NOW + 70_000).unwrap(),
    };
    let command = flexcore_domain::dispatch_command::DispatchCommand::try_new(
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
        authorization,
        input.authenticity,
        input.supersedes,
    )
    .expect("the base fixture command is valid");
    let fields = CommandRecordFields::from_command(
        &command,
        &AuthorizationRecordFields::pending_placeholder(command.command_id(), NOW + 70_000),
    )
    .expect("the base fixture record renders");
    (command, fields)
}

// ============================================================================
// F-IV-1 — malformed schema: a missing required §14.3 field
// ============================================================================

#[test]
fn f_iv_1_a_command_missing_a_required_s143_field_is_rejected_with_the_field_named() {
    // PRE-REGISTERED EXPECTATION (decision record 20_22_rejection_suites_a.md,
    // F-IV-1; UNRUN at authoring time): REJECTED at creation, error
    // `Schema(EmptyField { field: "reason" })` — the §14.3 intent group
    // (group 5) requires `reason`; the P1.06 validator names the field.
    // No record is inserted and no event is emitted.
    let run = || {
        let h = harness(usable_asset());
        h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
        let mut input = propose_input();
        input.intent.reason = "   ".to_string(); // the required §14.3 reason
        let outcome = h.gates.create_proposed_command(&context(), input).map(|_| ());
        (outcome, h.store.has_record(command_id()), emitted_events(&h))
    };
    let (first, record_exists, events) = run();
    match &first {
        Err(CommandGateError::Schema(DispatchCommandError::EmptyField { field })) => {
            assert_eq!(*field, "reason");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(!record_exists, "no record may exist for a rejected creation");
    assert!(events.is_empty(), "no event for a rejected creation");

    // Determinism: the identical inputs give the identical outcome.
    let (second, _, _) = run();
    assert_eq!(first, second);
}

#[test]
fn f_iv_1b_the_flat_s143_record_names_the_missing_field_at_creation() {
    // PRE-REGISTERED EXPECTATION (F-IV-1b; UNRUN at authoring time):
    // REJECTED by the P4.14 creation validator with
    // `MissingField { phase: Creation, field: "idempotency_key" }` — the
    // §14.3 idempotency-key group (group 11). The validator walks the
    // groups in §14.3 order; the first missing group is reported,
    // deterministically.
    let run = || {
        let (_, mut fields) = valid_command_with_record();
        fields.idempotency_key = "  ".to_string();
        fields.validate_creation()
    };
    let first = run();
    match &first {
        Err(CommandRecordError::MissingField { phase, field }) => {
            assert_eq!(*phase, RecordValidationPhase::Creation);
            assert_eq!(*field, "idempotency_key");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    // Determinism.
    assert_eq!(first, run());
}

// ============================================================================
// F-IV-2 — wrong units (kW supplied where MW is required)
// ============================================================================

#[test]
fn f_iv_2_a_kw_scaled_value_at_the_mw_boundary_is_outside_capability_not_rescaled() {
    // PRE-REGISTERED EXPECTATION (F-IV-2; UNRUN at authoring time):
    // REJECTED with the §14.3 `outside_capability` rule. kW/kWh are
    // unrepresentable at the typed boundary (§10.2: `ActivePowerMW` is the
    // only active-power type; scaled units are display-only), so a caller
    // supplying the number 1500 "in kW" where MW is required hands the
    // gate 1500 MW — which the outside-capability rule rejects against the
    // declared window [-5, 5] MW. The value is NEVER silently rescaled.
    let run = || {
        let (_, fields) = valid_command_with_record();
        // The flat record renders CANONICAL units only.
        let units_canonical = fields.units == "MW + MVAr" && !fields.units.contains("kW");
        // The mis-scaled value (1500 "kW" handed to the MW boundary) is
        // 1500 MW — outside the declared window.
        let target = ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(1500.0).unwrap(),
            q: None,
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: None,
            },
        };
        let rejection =
            outside_capability_rejection(&target, &capability_window());
        (units_canonical, rejection)
    };
    let (first_units, first_rejection) = run();
    assert!(
        first_units,
        "the flat record renders canonical MW/MVAr units, never kW"
    );
    let rejection = first_rejection.expect("the mis-scaled value must be rejected");
    assert_eq!(rejection.rule, CommandRejectionRule::OutsideCapability);
    assert_eq!(rejection.code(), "outside_capability");
    assert!(
        rejection.detail.contains("1500"),
        "the detail names the measured value: {}",
        rejection.detail
    );
    // Determinism.
    let (second_units, second_rejection) = run();
    assert_eq!(first_units, second_units);
    assert_eq!(first_rejection, second_rejection);
}

// ============================================================================
// F-IV-3 — inverted validity window
// ============================================================================

#[test]
fn f_iv_3_an_inverted_validity_window_is_unrepresentable_and_rejected_in_the_record() {
    // PRE-REGISTERED EXPECTATION (F-IV-3; UNRUN at authoring time):
    // (a) at the TYPE boundary, `ValidityWindow::new` refuses
    //     valid_from > valid_until — the inverted window cannot reach the
    //     gate at all (the §12.5 step-1 rule enforced structurally);
    // (b) at the PERSISTED-RECORD boundary (defence in depth), the P4.14
    //     creation validator rejects the inverted window with
    //     `InvalidField { phase: Creation, field: "valid_from_millis" }`
    //     and a detail naming the inversion.
    let run = || {
        let inverted = ValidityWindow::new(
            ValidFrom::from_unix_millis(NOW + 70_000).unwrap(),
            ValidUntil::from_unix_millis(NOW + 10_000).unwrap(),
        );
        let (_, mut fields) = valid_command_with_record();
        fields.valid_from_millis = NOW + 70_000;
        fields.valid_until_millis = NOW + 10_000;
        (inverted.is_err(), fields.validate_creation())
    };
    let (first_unrepresentable, first_validation) = run();
    assert!(
        first_unrepresentable,
        "the inverted window must be refused at the contracts boundary"
    );
    match &first_validation {
        Err(CommandRecordError::InvalidField { phase, field, detail }) => {
            assert_eq!(*phase, RecordValidationPhase::Creation);
            assert_eq!(*field, "valid_from_millis");
            assert!(detail.contains("inverted"), "detail: {detail}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    // Determinism.
    let (second_unrepresentable, second_validation) = run();
    assert_eq!(first_unrepresentable, second_unrepresentable);
    assert_eq!(first_validation, second_validation);
}

// ============================================================================
// F-IV-4 — unknown asset / tenant-scope IDs
// ============================================================================

#[test]
fn f_iv_4_an_unknown_or_foreign_asset_is_rejected_at_the_validation_gate() {
    // PRE-REGISTERED EXPECTATION (F-IV-4a; UNRUN at authoring time):
    // REJECTED at the validation gate with the failing check
    // `asset_unknown_in_tenant_scope` — a foreign asset and a missing asset
    // are indistinguishable (P0.16 rule 5: existence never leaks across a
    // tenant boundary, which is why one fixture covers unknown asset, an
    // asset of an unknown site, and a foreign tenant's asset). The command
    // moves PROPOSED → REJECTED, the journal row names the check and
    // carries the validation evidence reference, and `DispatchRejected` is
    // emitted after `DispatchProposed`.
    let run = || {
        let h = harness(AssetGateStatus::UnknownInTenantScope);
        h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
        let proposed = h
            .gates
            .create_proposed_command(&context(), propose_input())
            .expect("creation succeeds (the asset check is the validation gate's)");
        let result = h
            .gates
            .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
            .expect("the gate verdict (a rejection is not an error)");
        let journal = h
            .store
            .journals
            .lock()
            .expect("fake store")
            .get(&command_id().to_string())
            .cloned()
            .unwrap_or_default();
        (result.passed, result.failing_check, result.evidence_reference, journal, emitted_events(&h))
    };
    let (first_passed, first_check, first_evidence, first_journal, first_events) = run();
    assert!(!first_passed);
    assert_eq!(
        first_check.as_deref(),
        Some("asset_unknown_in_tenant_scope")
    );
    assert!(
        first_evidence.contains("check/asset_unknown_in_tenant_scope"),
        "the evidence reference names the failing check: {first_evidence}"
    );
    let entry = first_journal.last().expect("the rejection is journalled");
    assert_eq!(entry.to, CommandState::Rejected);
    assert_eq!(entry.from, CommandState::Proposed);
    assert_eq!(
        entry.reason.as_deref(),
        Some("asset_unknown_in_tenant_scope")
    );
    assert_eq!(
        first_events,
        vec!["DispatchProposed", "DispatchRejected"]
    );
    // Determinism.
    let (second_passed, second_check, _, _, _) = run();
    assert_eq!(first_passed, second_passed);
    assert_eq!(first_check, second_check);
}

#[test]
fn f_iv_4b_a_schedule_unknown_in_the_callers_tenant_scope_rejects_creation() {
    // PRE-REGISTERED EXPECTATION (F-IV-4b; UNRUN at authoring time):
    // REJECTED at creation with `CreationRejected { check:
    // SourceScheduleRevisionUnknown }` — the tenant-scoped store reports
    // Unknown for a schedule (and therefore a tenant/portfolio/site chain)
    // that does not exist in the caller's tenant scope; unknown is never
    // usable. No record, no event.
    let run = || {
        let h = harness(usable_asset());
        // Nothing is seeded: the revision does not exist in this tenant
        // scope (the fixture for an unknown tenant/scope chain).
        let outcome = h.gates.create_proposed_command(&context(), propose_input());
        (outcome, h.store.has_record(command_id()), emitted_events(&h))
    };
    let (first, record_exists, events) = run();
    match &first {
        Err(CommandGateError::CreationRejected { check, detail }) => {
            assert_eq!(*check, CreationCheck::SourceScheduleRevisionUnknown);
            assert!(detail.contains("does not exist in this tenant scope"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(!record_exists);
    assert!(events.is_empty());
    // Determinism.
    let (second, _, _) = run();
    assert_eq!(first, second);
}

// ============================================================================
// F-IV-5 — target P/Q outside capability
// ============================================================================

#[test]
fn f_iv_5_a_target_outside_the_declared_capability_is_rejected_with_the_rule_named() {
    // PRE-REGISTERED EXPECTATION (F-IV-5; UNRUN at authoring time):
    // REJECTED with the §14.3 `outside_capability` rule for each case:
    // (a) P = 6.0 MW vs the window [-5, 5] MW;
    // (b) Q = 3.0 MVAr vs the declared range [-2, 2] MVAr;
    // (c) a reactive target where the window declares NO reactive range;
    // (d) a required ramp (3.0 MW/s) above the declared 2.0 MW/s.
    // Each detail names the measured value and the declared window.
    let run = || {
        let p_target = ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(6.0).unwrap(),
            q: None,
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: None,
            },
        };
        let q_target = ControlTarget::PowerSetpoint {
            p: ActivePowerMW::new(1.5).unwrap(),
            q: Some(ReactivePowerMVAr::new(3.0).unwrap()),
            conventions: DeclaredSignConventions {
                active: ActivePowerSignConvention::ExportPositive,
                reactive: Some(ReactiveSignConvention::InjectionPositive),
            },
        };
        let window = capability_window();
        let undeclared_q_window = AssetCapabilityWindow {
            min_q_mvar: None,
            max_q_mvar: None,
            ..window
        };
        (
            outside_capability_rejection(&p_target, &window),
            outside_capability_rejection(&q_target, &window),
            outside_capability_rejection(&q_target, &undeclared_q_window),
            ramp_outside_capability_rejection(3.0, 0.5, &window),
        )
    };
    let (first_p, first_q, first_undeclared, first_ramp) = run();
    for (case, rejection) in [
        ("p", first_p.as_ref()),
        ("q", first_q.as_ref()),
        ("undeclared-q", first_undeclared.as_ref()),
        ("ramp", first_ramp.as_ref()),
    ] {
        let rejection = rejection.unwrap_or_else(|| panic!("case {case} must be rejected"));
        assert_eq!(rejection.rule, CommandRejectionRule::OutsideCapability);
        assert_eq!(rejection.code(), "outside_capability");
    }
    assert!(first_p.unwrap().detail.contains("6"));
    assert!(first_q.unwrap().detail.contains("3"));
    assert!(first_undeclared.unwrap().detail.contains("no reactive"));
    assert!(first_ramp.unwrap().detail.contains("3"));
    // Determinism.
    let (second_p, second_q, second_undeclared, second_ramp) = run();
    assert_eq!(first_p, second_p);
    assert_eq!(first_q, second_q);
    assert_eq!(first_undeclared, second_undeclared);
    assert_eq!(first_ramp, second_ramp);
}

// ============================================================================
// F-IV-6 — (non-positive) duration
// ============================================================================

#[test]
fn f_iv_6_a_negative_duration_is_unrepresentable_and_a_zero_duration_is_rejected() {
    // PRE-REGISTERED EXPECTATION (F-IV-6; UNRUN at authoring time):
    // `std::time::Duration` is unsigned, so a NEGATIVE duration is
    // unrepresentable at the typed boundary (§14.3 group 9); the fixture
    // uses the expressible boundary violation, ZERO duration, and expects
    // REJECTED at creation with `Schema(InvalidDuration)` — the §14.3
    // duration rule. No record, no event. The flat-record validator adds
    // the same rule for the persisted shape (`InvalidField { field:
    // "duration_millis" }`).
    let run = || {
        let h = harness(usable_asset());
        h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
        let mut input = propose_input();
        input.execution.duration = Duration::ZERO;
        let outcome = h.gates.create_proposed_command(&context(), input).map(|_| ());
        // The flat-record half of the rule.
        let (_, mut fields) = valid_command_with_record();
        fields.duration_millis = 0;
        let record_verdict = fields.validate_creation();
        (outcome, h.store.has_record(command_id()), record_verdict)
    };
    let (first_outcome, first_record_exists, first_record_verdict) = run();
    match &first_outcome {
        Err(CommandGateError::Schema(DispatchCommandError::InvalidDuration)) => {}
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(!first_record_exists);
    match &first_record_verdict {
        Err(CommandRecordError::InvalidField { phase, field, .. }) => {
            assert_eq!(*phase, RecordValidationPhase::Creation);
            assert_eq!(*field, "duration_millis");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    // Determinism.
    let (second_outcome, _, second_record_verdict) = run();
    assert_eq!(first_outcome, second_outcome);
    assert_eq!(first_record_verdict, second_record_verdict);
}

// ============================================================================
// Control — the valid fixture CREATES AND VALIDATES (harness sanity)
// ============================================================================

/// A control, not a rejection fixture: the unmodified fixture command is
/// created and validated, proving every rejection above is a designed
/// rejection, not a harness artifact that rejects everything.
#[test]
fn the_control_fixture_creates_and_validates() {
    let h = harness(usable_asset());
    h.store.seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    let proposed = h
        .gates
        .create_proposed_command(&context(), propose_input())
        .expect("the control creation succeeds");
    let result = h
        .gates
        .run_validation_gate(&context(), &proposed.command, &actor("svc:validator"))
        .expect("the control validation verdict");
    assert!(result.passed, "the control must pass");
    assert!(result.failing_check.is_none());
    let entry = {
        let journals = h.store.journals.lock().expect("fake store");
        journals
            .get(&command_id().to_string())
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0].to, CommandState::Validated);
    assert_eq!(entry[0].at_unix_millis, NOW, "the fixed controlled clock");
}
