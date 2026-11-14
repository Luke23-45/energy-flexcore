// P4.17 / P4.18 — Fallback policy tests (report §14.4, §14.1–§14.2, §17.5;
//         ledger P4.17–P4.18 acceptance criteria; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/17_19_fallback_timing.md`).
//
// Coverage per the ledger acceptance criteria:
//
// P4.17 — at least three policy variants tested (hold, return-to-local,
//         require-operator-intervention); NO universal zero-power fallback
//         exists in code (the structural source scan at the bottom of this
//         file); every asset/site has a named policy OR the explicit
//         "no fallback configured — require operator intervention" entry
//         (the default is intervention, recorded — never silence); FALLBACK
//         entry from FAILED and EXPIRED where the policy requires it, with
//         policy, parameters, and bounded interval recorded; hold policies
//         state their validity conditions (empty validity conditions are a
//         construction error); `DispatchFallbackActivated` is emitted; the
//         simulated operator review is labelled SIMULATED-OPERATOR.
// P4.18 — the bounded hold expires after its configured interval and
//         escalates per policy (controlled clock; never indefinite); the
//         escalation is an audit record, NOT a command transition (FALLBACK
//         is terminal in the frozen table — frozen-edge conformance);
//         recovery after FALLBACK requires a NEW command through the FULL
//         PROPOSED → VALIDATED → FEASIBLE → APPROVED chain; a direct
//         FALLBACK → ISSUED attempt is rejected; activations and escalations
//         are audit-visible with policy, parameters, and outcome.
//
// Every timestamp in this suite comes from a controlled, advanceable clock
// (never wall-clock); every faked seam fails closed by construction.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use flexcore_application::command_gates::{CommandGateStore, CommandGateStoreError, CommandGateView};
use flexcore_application::error::ApplicationError;
use flexcore_application::fallback_policy::{
    BoundedHoldInterval, EscalationTarget, FallbackAction, FallbackActivationInput,
    FallbackAuditError, FallbackAuditSink, FallbackDecision, FallbackEngine, FallbackPolicy,
    FallbackPolicyEntry, FallbackPolicyError, FallbackPolicyRegistry, FallbackScope,
    FallbackTrigger, HoldValidityConditions, SimulatedOperatorReview,
    DEFAULT_ENTRY_STATEMENT, DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE, FALLBACK_POLICY_REF_PREFIX,
    NO_UNIVERSAL_FALLBACK_STATEMENT, RECOVERY_RULE_STATEMENT, SIMULATED_OPERATOR_REVIEW_LABEL,
    VALIDITY_CONDITIONS_RULE_STATEMENT,
};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::ValidUntil;
use flexcore_domain::command_lifecycle::{
    Actor, AuthorizationOutcome, CommandLifecycle, CommandTransition, EvidenceReference,
    ExpiryMilestone, FailureReason, GateOutcome, LifecycleClock,
};
use flexcore_domain::command_state::{CommandState, try_transition};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c01";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c04";
const UUID_COMMAND_BASE: u128 = 0x0192a5e0_7e2f_9a1b_3c4d_5e6f7c0a0000;

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant() -> TenantId {
    TenantId::new(uuid(UUID_TENANT))
}

fn ctx() -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant(),
        subject: "test:principal".to_string(),
    }))
}

fn site() -> SiteId {
    SiteId::new(uuid(UUID_SITE))
}

fn asset() -> AssetId {
    AssetId::new(uuid(UUID_ASSET))
}

fn command_id(n: u8) -> CommandId {
    CommandId::new(FlexUuid::from_bytes((UUID_COMMAND_BASE + n as u128).to_be_bytes()))
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

    fn advance_to(&self, now_unix_millis: i64) {
        *self.now_unix_millis.lock().expect("test clock") = now_unix_millis;
    }
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

// --- Seam fakes -------------------------------------------------------------------

/// In-memory command store over the REUSED P4.13 `CommandGateStore` seam.
/// Seeded directly with a record row and a persisted journal (the journals
/// the tests seed are produced by driving the P4.10 domain engine, so the
/// entries are frozen-table-faithful).
struct FakeStore {
    records: Mutex<Vec<(String, i64)>>,
    journals: Mutex<std::collections::HashMap<String, Vec<CommandTransition>>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            journals: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Seeds one command: the validity bound and the journal entries
    /// produced by the domain-engine chain run.
    fn seed(&self, command_id: CommandId, valid_until_millis: i64, journal: Vec<CommandTransition>) {
        self.records
            .lock()
            .expect("fake store")
            .push((command_id.to_string(), valid_until_millis));
        self.journals
            .lock()
            .expect("fake store")
            .insert(command_id.to_string(), journal);
    }

    fn journal_len(&self, command_id: CommandId) -> usize {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command_id.to_string())
            .map(|journal| journal.len())
            .unwrap_or(0)
    }
}

impl CommandGateStore for FakeStore {
    fn insert_proposed(
        &self,
        _ctx: &TenantContext,
        _record: &flexcore_application::command_gates::ProposedCommandRecord,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        // The fallback engine never creates commands (P4.11's gate does).
        Err(CommandGateStoreError::Backend {
            code: "NOT_USED".to_string(),
            message: "the fallback engine does not create command records".to_string(),
        })
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
        _ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<Option<CommandGateView>, CommandGateStoreError> {
        let records = self.records.lock().expect("fake store");
        let Some((_, valid_until_millis)) = records
            .iter()
            .find(|(id, _)| *id == command_id.to_string())
            .map(|(_, until)| (true, *until))
        else {
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
            valid_until_millis,
            journal,
        }))
    }

    fn schedule_revision_state(
        &self,
        _ctx: &TenantContext,
        _schedule_id: &flexcore_domain::ids::ScheduleId,
        _revision: u64,
    ) -> Result<flexcore_application::command_gates::ScheduleRevisionUsability, CommandGateStoreError>
    {
        Err(CommandGateStoreError::Backend {
            code: "NOT_USED".to_string(),
            message: "the fallback engine does not check schedule revisions".to_string(),
        })
    }
}

/// In-memory audit sink: every activation and escalation decision is
/// recorded here (the P4.30 wiring consumes these rows in production).
struct FakeAudit {
    activations: Mutex<Vec<flexcore_application::fallback_policy::FallbackActivationRecord>>,
    escalations: Mutex<Vec<flexcore_application::fallback_policy::FallbackEscalationRecord>>,
    fail: Mutex<bool>,
}

impl FakeAudit {
    fn new() -> Self {
        Self {
            activations: Mutex::new(Vec::new()),
            escalations: Mutex::new(Vec::new()),
            fail: Mutex::new(false),
        }
    }

    fn set_fail(&self, fail: bool) {
        *self.fail.lock().expect("fake audit") = fail;
    }
}

impl FallbackAuditSink for FakeAudit {
    fn record_activation(
        &self,
        record: &flexcore_application::fallback_policy::FallbackActivationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        if *self.fail.lock().expect("fake audit") {
            return Err(FallbackAuditError {
                code: "E_AUDIT_DOWN".to_string(),
                message: "audit backend unavailable".to_string(),
            });
        }
        self.activations
            .lock()
            .expect("fake audit")
            .push(record.clone());
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_activations".to_string(),
            StateChangeOperation::Insert,
            None,
        ))
    }

    fn record_escalation(
        &self,
        record: &flexcore_application::fallback_policy::FallbackEscalationRecord,
    ) -> Result<StateChangeReceipt, FallbackAuditError> {
        if *self.fail.lock().expect("fake audit") {
            return Err(FallbackAuditError {
                code: "E_AUDIT_DOWN".to_string(),
                message: "audit backend unavailable".to_string(),
            });
        }
        self.escalations
            .lock()
            .expect("fake audit")
            .push(record.clone());
        Ok(StateChangeReceipt::new(
            "dispatch_fallback_escalations".to_string(),
            StateChangeOperation::Insert,
            None,
        ))
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

// --- The engine harness --------------------------------------------------------------

struct Harness {
    engine: FallbackEngine<'static>,
    store: &'static FakeStore,
    audit: &'static FakeAudit,
    outbox: &'static FakeEventOutbox,
    clock: &'static TestClock,
    ids: &'static CountingIdGenerator,
}

fn harness(policies: FallbackPolicyRegistry, start_millis: i64) -> Harness {
    let policies = Box::leak(Box::new(policies));
    let store = Box::leak(Box::new(FakeStore::new()));
    let audit = Box::leak(Box::new(FakeAudit::new()));
    let journal = Box::leak(Box::new(FakeWriteJournal::new()));
    let outbox = Box::leak(Box::new(FakeEventOutbox::new(journal.clone())));
    let clock = Box::leak(Box::new(TestClock::at(start_millis)));
    let ids = Box::leak(Box::new(CountingIdGenerator::new()));
    let encoder = Box::leak(Box::new(DebugEncoder));
    let engine = FallbackEngine {
        policies,
        store,
        audit,
        outbox,
        encoder,
        ids,
        clock,
    };
    Harness {
        engine,
        store,
        audit,
        outbox,
        clock,
        ids,
    }
}

// --- The §14.1 chain fixture (driven through the P4.10 domain engine) -----------------

const VALID_UNTIL_MILLIS: i64 = 70_000;

/// Drives ONE command through the frozen §14.1 chain to FAILED (ISSUED →
/// FAILED on a NACK), journalling every transition with controlled-clock
/// timestamps. Returns the journal for the store seed.
fn run_chain_to_failed(command_id: CommandId, clock: &TestClock) -> Vec<CommandTransition> {
    let valid_until = ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(command_id, valid_until).expect("propose");
    let mut journal = Vec::new();
    let mut step = |lifecycle: &mut CommandLifecycle, at: i64, apply: &dyn Fn(&mut CommandLifecycle, &TestClock)| {
        clock.advance_to(at);
        apply(lifecycle, clock);
        journal.push(lifecycle.journal().last().expect("one transition").clone());
    };
    let actor = || Actor::new("test:driver").expect("actor");
    step(&mut lifecycle, 1_000, &|lifecycle, clock| {
        lifecycle
            .record_validation(
                GateOutcome::pass(EvidenceReference::new("validation:test").expect("evidence")),
                actor(),
                clock,
            )
            .expect("validation");
    });
    step(&mut lifecycle, 2_000, &|lifecycle, clock| {
        lifecycle
            .record_feasibility(
                GateOutcome::pass(EvidenceReference::new("feasibility:test").expect("evidence")),
                actor(),
                clock,
            )
            .expect("feasibility");
    });
    step(&mut lifecycle, 3_000, &|lifecycle, clock| {
        lifecycle
            .record_authorization(
                AuthorizationOutcome::approve(
                    EvidenceReference::new("authz:test").expect("evidence"),
                    actor(),
                ),
                clock,
            )
            .expect("authorization");
    });
    step(&mut lifecycle, 4_000, &|lifecycle, clock| {
        lifecycle
            .record_issue(
                EvidenceReference::new("signature:test").expect("evidence"),
                actor(),
                clock,
            )
            .expect("issue");
    });
    step(&mut lifecycle, 5_000, &|lifecycle, clock| {
        lifecycle
            .record_acknowledgement(
                EvidenceReference::new("acceptance:test").expect("evidence"),
                actor(),
                clock,
            )
            .expect("acknowledgement");
    });
    step(&mut lifecycle, 6_000, &|lifecycle, clock| {
        lifecycle
            .record_failure(FailureReason::Nack, actor(), clock)
            .expect("failure");
    });
    assert_eq!(lifecycle.state(), CommandState::Failed);
    journal
}

/// Drives ONE command through the frozen chain into TRACKING and then
/// EXPIRED (validity ends before completion), journalling every transition.
fn run_chain_to_expired(command_id: CommandId, clock: &TestClock) -> Vec<CommandTransition> {
    let valid_until = ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(command_id, valid_until).expect("propose");
    let mut journal = Vec::new();
    let mut step = |lifecycle: &mut CommandLifecycle, at: i64, apply: &dyn Fn(&mut CommandLifecycle, &TestClock)| {
        clock.advance_to(at);
        apply(lifecycle, clock);
        journal.push(lifecycle.journal().last().expect("one transition").clone());
    };
    let actor = || Actor::new("test:driver").expect("actor");
    step(&mut lifecycle, 1_000, &|lifecycle, clock| {
        lifecycle
            .record_validation(
                GateOutcome::pass(EvidenceReference::new("validation:test").expect("evidence")),
                actor(),
                clock,
            )
            .expect("validation");
    });
    step(&mut lifecycle, 2_000, &|lifecycle, clock| {
        lifecycle
            .record_feasibility(
                GateOutcome::pass(EvidenceReference::new("feasibility:test").expect("evidence")),
                actor(),
                clock,
            )
            .expect("feasibility");
    });
    step(&mut lifecycle, 3_000, &|lifecycle, clock| {
        lifecycle
            .record_authorization(
                AuthorizationOutcome::approve(
                    EvidenceReference::new("authz:test").expect("evidence"),
                    actor(),
                ),
                clock,
            )
            .expect("authorization");
    });
    step(&mut lifecycle, 4_000, &|lifecycle, clock| {
        lifecycle
            .record_issue(
                EvidenceReference::new("signature:test").expect("evidence"),
                actor(),
                clock,
            )
            .expect("issue");
    });
    step(&mut lifecycle, 5_000, &|lifecycle, clock| {
        lifecycle
            .record_acknowledgement(
                EvidenceReference::new("acceptance:test").expect("evidence"),
                actor(),
                clock,
            )
            .expect("acknowledgement");
    });
    step(&mut lifecycle, 6_000, &|lifecycle, clock| {
        lifecycle
            .record_tracking_started(
                EvidenceReference::new("telemetry:test").expect("evidence"),
                actor(),
                clock,
            )
            .expect("tracking");
    });
    step(&mut lifecycle, 80_000, &|lifecycle, clock| {
        lifecycle
            .record_expiry(ExpiryMilestone::BeforeCompletion, actor(), clock)
            .expect("expiry");
    });
    assert_eq!(lifecycle.state(), CommandState::Expired);
    journal
}

fn simulated_review(at: i64) -> SimulatedOperatorReview {
    SimulatedOperatorReview::new(
        "engineer:power-systems",
        "policy reviewed against the site single-line diagram and asset limits",
        at,
    )
    .expect("valid review")
}

fn activation_input(command: CommandId, trigger: FallbackTrigger) -> FallbackActivationInput {
    FallbackActivationInput {
        command_id: command,
        portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
        site_id: site(),
        asset_id: asset(),
        trigger,
        simulated_operator_review: simulated_review(500),
    }
}

// --- The three policy variants (P4.17 acceptance: >= 3 variants tested) ---------------

/// Variant 1 — hold the last accepted setpoint for a BOUNDED interval, WITH
/// its required validity conditions (§14.4) and its escalation target
/// (P4.18): expires to require-operator-intervention.
fn hold_policy() -> FallbackPolicy {
    FallbackPolicy::new(
        "hold-bess-60s",
        FallbackScope::Asset {
            site: site(),
            asset: asset(),
        },
        vec![
            FallbackAction::Hold {
                interval: BoundedHoldInterval::from_millis(60_000).expect("positive interval"),
                validity: HoldValidityConditions::new(vec![
                    "feeder topology unchanged since the accepted setpoint".to_string(),
                    "asset state within its declared capability".to_string(),
                    "network constraints unchanged since the last feasibility check".to_string(),
                ])
                .expect("non-empty validity conditions"),
                escalation: EscalationTarget::RequireOperatorIntervention,
            },
            FallbackAction::Declare(FallbackPolicyVariant::PreserveReserveSocConstraints),
        ],
        vec![FallbackTrigger::OnFailure, FallbackTrigger::OnExpiry],
    )
    .expect("valid hold policy")
}

/// Variant 2 — return to local EMS autonomous control.
fn return_to_local_policy() -> FallbackPolicy {
    FallbackPolicy::new(
        "return-to-local-ems",
        FallbackScope::Site { site: site() },
        vec![FallbackAction::Declare(
            FallbackPolicyVariant::ReturnToLocalEmsAutonomousControl,
        )],
        vec![FallbackTrigger::OnFailure],
    )
    .expect("valid return-to-local policy")
}

/// Variant 3 — require operator intervention (a NAMED policy that fires).
fn require_intervention_policy() -> FallbackPolicy {
    FallbackPolicy::new(
        "intervene-on-failure",
        FallbackScope::Asset {
            site: site(),
            asset: asset(),
        },
        vec![FallbackAction::Declare(FallbackPolicyVariant::RequireOperatorIntervention)],
        vec![FallbackTrigger::OnFailure],
    )
    .expect("valid require-intervention policy")
}

// --- P4.17: policy configuration ------------------------------------------------------

#[test]
fn three_policy_variants_are_configured_and_their_parameters_recorded() {
    for policy in [hold_policy(), return_to_local_policy(), require_intervention_policy()] {
        assert!(!policy.parameters_recorded_text().is_empty());
        assert!(policy
            .policy_reference()
            .starts_with(FALLBACK_POLICY_REF_PREFIX));
    }
    assert_eq!(hold_policy().name(), "hold-bess-60s");
    assert_eq!(return_to_local_policy().name(), "return-to-local-ems");
    assert_eq!(
        require_intervention_policy().name(),
        "intervene-on-failure"
    );
}

#[test]
fn a_hold_policy_without_validity_conditions_is_a_construction_error() {
    // §14.4: a last-known setpoint is not automatically safe when topology,
    // asset state, or network constraints have changed — the validity
    // conditions are REQUIRED, and empty is a construction error.
    assert_eq!(
        HoldValidityConditions::new(Vec::new()),
        Err(FallbackPolicyError::HoldPolicyRequiresValidityConditions)
    );
    assert_eq!(
        HoldValidityConditions::new(vec!["   ".to_string()]),
        Err(FallbackPolicyError::EmptyField {
            field: "validity_condition"
        })
    );
    assert!(VALIDITY_CONDITIONS_RULE_STATEMENT.contains("not automatically safe"));
}

#[test]
fn a_hold_policy_records_its_validity_conditions_and_escalation() {
    let policy = hold_policy();
    let (interval, validity, escalation) = policy
        .bounded_hold()
        .expect("the hold policy declares a bounded hold");
    assert_eq!(interval.millis(), 60_000);
    assert_eq!(validity.conditions().len(), 3);
    assert_eq!(escalation, EscalationTarget::RequireOperatorIntervention);
    assert!(policy.parameters_recorded_text().contains("validity conditions"));
}

#[test]
fn every_asset_resolves_to_a_named_policy_or_the_explicit_default_entry() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(return_to_local_policy()).expect("site entry");

    // An asset WITH a site-level named policy resolves to it.
    let entry = registry.entry_for(site(), asset());
    assert_eq!(entry.named_policy().expect("named").name(), "return-to-local-ems");

    // An asset with NO entry at all resolves to the EXPLICIT default entry —
    // recorded, never silence.
    let other_site = SiteId::new(uuid("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c09"));
    let other_asset = AssetId::new(uuid("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7c0a"));
    let default = registry.entry_for(other_site, other_asset);
    assert!(default.named_policy().is_none());
    match &default {
        FallbackPolicyEntry::RequireIntervention {
            recorded_reason, ..
        } => assert_eq!(recorded_reason, DEFAULT_ENTRY_STATEMENT),
        FallbackPolicyEntry::Named(_) => panic!("must be the explicit default entry"),
    }
}

#[test]
fn an_asset_level_policy_overrides_the_site_level_policy() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(return_to_local_policy()).expect("site entry");
    registry.register_policy(hold_policy()).expect("asset entry");
    let entry = registry.entry_for(site(), asset());
    assert_eq!(entry.named_policy().expect("named").name(), "hold-bess-60s");
}

#[test]
fn a_duplicate_policy_scope_is_a_configuration_error() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(return_to_local_policy()).expect("first entry");
    assert!(registry.register_policy(return_to_local_policy()).is_err());
}

// --- P4.17: FALLBACK entry from FAILED and EXPIRED ------------------------------------

#[test]
fn fallback_activates_from_failed_for_a_hold_policy_with_full_record() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(hold_policy()).expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(1);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("activation");
    let FallbackDecision::Activated { record, transition } = &decision else {
        panic!("the hold policy requires FALLBACK on FAILED");
    };

    // The frozen edge (26): FAILED → FALLBACK, with the policy reference as
    // the evidence.
    assert_eq!((transition.from, transition.to), (CommandState::Failed, CommandState::Fallback));
    assert_eq!(
        transition.evidence.as_ref().expect("policy evidence").as_str(),
        format!("{FALLBACK_POLICY_REF_PREFIX}hold-bess-60s")
    );
    // The record carries WHICH policy fired, its parameters, and the bounded
    // interval.
    assert_eq!(record.policy_name, "hold-bess-60s");
    assert!(record.parameters.contains("hold last accepted setpoint for 60000 ms"));
    assert!(record.parameters.contains("validity conditions:"));
    assert_eq!(record.bounded_hold_millis, Some(60_000));
    assert_eq!(record.hold_escalation, Some(EscalationTarget::RequireOperatorIntervention));
    assert_eq!(record.validity_conditions.as_ref().expect("conditions").len(), 3);
    // The review is recorded as a SIMULATED-OPERATOR review — labelled,
    // never a real operator sign-off.
    let review = record
        .simulated_operator_review
        .as_ref()
        .expect("recorded review");
    assert!(review.as_recorded_text().contains("SIMULATED-OPERATOR"));
    assert!(review.as_recorded_text().contains("not a real operator sign-off"));
    assert!(SIMULATED_OPERATOR_REVIEW_LABEL.contains("SIMULATED-OPERATOR"));

    // The activation is audit-visible and the event was emitted.
    assert_eq!(harness.audit.activations.lock().expect("audit").len(), 1);
    assert_eq!(harness.outbox.len(), 1);
    let write = &harness.outbox.writes()[0];
    assert!(write.payload().contains("DispatchFallbackActivated"));
    // The frozen payload-fit gap is recorded (identity+time payload only).
    assert!(DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE.contains("no field for the policy name"));
}

#[test]
fn fallback_activates_from_expired_where_the_policy_requires_it() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(hold_policy()).expect("entry");
    let harness = harness(registry, 81_000);
    let command = command_id(2);
    let journal = run_chain_to_expired(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnExpiry))
        .expect("activation");
    let FallbackDecision::Activated { transition, .. } = &decision else {
        panic!("the hold policy requires FALLBACK on EXPIRED");
    };
    // The frozen edge (27): EXPIRED → FALLBACK ("policy requires").
    assert_eq!((transition.from, transition.to), (CommandState::Expired, CommandState::Fallback));
    assert!(transition.reason.as_deref().expect("reason").contains("policy requires"));
}

#[test]
fn a_named_require_intervention_policy_fires_and_enters_fallback() {
    let mut registry = FallbackPolicyRegistry::new();
    registry
        .register_policy(require_intervention_policy())
        .expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(3);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("activation");
    let FallbackDecision::Activated { record, transition } = &decision else {
        panic!("the named require-intervention policy fires on FAILED");
    };
    assert_eq!((transition.from, transition.to), (CommandState::Failed, CommandState::Fallback));
    assert!(record.parameters.contains("require operator intervention"));
    assert!(record.outcome.contains("FALLBACK entered"));
}

#[test]
fn an_unconfigured_asset_records_the_explicit_intervention_default_and_no_transition() {
    let registry = FallbackPolicyRegistry::new(); // deliberately empty
    let harness = harness(registry, 7_000);
    let command = command_id(4);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);
    let journal_len_before = harness.store.journal_len(command);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("decision");
    let FallbackDecision::InterventionRequired { record } = &decision else {
        panic!("an unconfigured asset resolves to the explicit default entry");
    };
    assert!(record.is_default_entry);
    assert_eq!(record.policy_name, DEFAULT_ENTRY_STATEMENT);
    assert!(record.outcome.contains("operator intervention required"));
    // NO state transition, NO event — but the decision IS audit-visible.
    assert_eq!(harness.store.journal_len(command), journal_len_before);
    assert_eq!(harness.outbox.len(), 0);
    assert_eq!(harness.audit.activations.lock().expect("audit").len(), 1);
    assert!(NO_UNIVERSAL_FALLBACK_STATEMENT.contains("no universal"));
}

#[test]
fn a_policy_that_does_not_require_fallback_on_the_trigger_records_the_decision() {
    // The return-to-local policy activates on FAILED only; the command here
    // is EXPIRED.
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(return_to_local_policy()).expect("entry");
    let harness = harness(registry, 81_000);
    let command = command_id(5);
    let journal = run_chain_to_expired(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);
    let journal_len_before = harness.store.journal_len(command);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnExpiry))
        .expect("decision");
    let FallbackDecision::TriggerNotConfigured { record } = &decision else {
        panic!("the policy does not require FALLBACK on EXPIRED");
    };
    assert!(record.outcome.contains("does not require FALLBACK"));
    assert_eq!(harness.store.journal_len(command), journal_len_before);
    assert_eq!(harness.outbox.len(), 0);
    assert_eq!(harness.audit.activations.lock().expect("audit").len(), 1);
}

#[test]
fn a_trigger_that_does_not_match_the_actual_state_is_rejected_fail_closed() {
    // The command is FAILED; the caller claims EXPIRED. The journal is the
    // evidence — the input cannot spoof the state.
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(hold_policy()).expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(6);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let error = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnExpiry))
        .expect_err("trigger/state mismatch is a named error");
    assert!(matches!(
        error,
        FallbackPolicyError::InconsistentJournal { .. }
    ));
}

#[test]
fn an_audit_sink_failure_is_fail_closed_never_a_skipped_record() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(hold_policy()).expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(7);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);
    harness.audit.set_fail(true);

    let error = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect_err("the audit failure is an error, never a skipped record");
    assert!(matches!(error, FallbackPolicyError::Audit(_)));
}

// --- P4.18: bounded hold, escalation, recovery ----------------------------------------

#[test]
fn the_bounded_hold_is_active_before_its_interval_and_escalates_after_it() {
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(hold_policy()).expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(8);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("activation");
    let FallbackDecision::Activated { record, .. } = &decision else {
        panic!("activated");
    };

    // Still inside the 60 s interval (activated at 7_000; now 20_000).
    harness.clock.advance_to(20_000);
    match harness
        .engine
        .evaluate_hold_expiration(&ctx(), record)
        .expect("evaluation")
    {
        flexcore_application::fallback_policy::HoldExpiryEvaluation::HoldActive {
            expires_at_unix_millis,
        } => assert_eq!(expires_at_unix_millis, 67_000),
        other => panic!("the hold must still be active; got {other:?}"),
    }

    // At the expiry instant (7_000 + 60_000 = 67_000) the hold ESCALATES —
    // it never holds indefinitely (controlled clock; enforced in code).
    harness.clock.advance_to(67_000);
    match harness
        .engine
        .evaluate_hold_expiration(&ctx(), record)
        .expect("evaluation")
    {
        flexcore_application::fallback_policy::HoldExpiryEvaluation::Escalated {
            target,
            record: escalation,
        } => {
            assert_eq!(target, EscalationTarget::RequireOperatorIntervention);
            assert_eq!(escalation.escalated_at_unix_millis, 67_000);
            assert!(escalation.outcome.contains("bounded hold expired after 60000 ms"));
            assert!(escalation.outcome.contains("require operator intervention"));
        }
        other => panic!("the hold must escalate at its expiry; got {other:?}"),
    }
    assert_eq!(harness.audit.escalations.lock().expect("audit").len(), 1);

    // The escalation is an AUDIT record, NOT a command transition: FALLBACK
    // is terminal in the frozen table (frozen-edge conformance).
    assert_eq!(harness.store.journal_len(command), 7); // 6 chain + 1 FALLBACK
    let journals = harness
        .store
        .journals
        .lock()
        .expect("fake store");
    let journal = &journals[&command.to_string()];
    assert_eq!(journal.last().expect("last").to, CommandState::Fallback);
}

#[test]
fn a_hold_can_escalate_to_return_to_local_ems_per_policy() {
    let policy = FallbackPolicy::new(
        "hold-then-return",
        FallbackScope::Asset {
            site: site(),
            asset: asset(),
        },
        vec![FallbackAction::Hold {
            interval: BoundedHoldInterval::from_millis(30_000).expect("positive interval"),
            validity: HoldValidityConditions::new(vec![
                "topology unchanged".to_string(),
            ])
            .expect("non-empty validity conditions"),
            escalation: EscalationTarget::ReturnToLocalEms,
        }],
        vec![FallbackTrigger::OnFailure],
    )
    .expect("valid policy");
    let mut registry = FallbackPolicyRegistry::new();
    registry.register_policy(policy).expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(9);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("activation");
    let FallbackDecision::Activated { record, .. } = &decision else {
        panic!("activated");
    };
    harness.clock.advance_to(37_000);
    match harness
        .engine
        .evaluate_hold_expiration(&ctx(), record)
        .expect("evaluation")
    {
        flexcore_application::fallback_policy::HoldExpiryEvaluation::Escalated { target, .. } => {
            assert_eq!(target, EscalationTarget::ReturnToLocalEms);
        }
        other => panic!("must escalate to return-to-local; got {other:?}"),
    }
}

#[test]
fn an_activation_without_a_bounded_hold_has_nothing_to_expire() {
    let mut registry = FallbackPolicyRegistry::new();
    registry
        .register_policy(require_intervention_policy())
        .expect("entry");
    let harness = harness(registry, 7_000);
    let command = command_id(10);
    let journal = run_chain_to_failed(command, harness.clock);
    harness.store.seed(command, VALID_UNTIL_MILLIS, journal);

    let decision = harness
        .engine
        .activate_fallback(&ctx(), activation_input(command, FallbackTrigger::OnFailure))
        .expect("activation");
    let FallbackDecision::Activated { record, .. } = &decision else {
        panic!("activated");
    };
    assert!(matches!(
        harness
            .engine
            .evaluate_hold_expiration(&ctx(), record)
            .expect("evaluation"),
        flexcore_application::fallback_policy::HoldExpiryEvaluation::NoBoundedHold
    ));
}

#[test]
fn recovery_after_fallback_requires_a_new_command_through_the_full_gate_chain() {
    // The recovery rule (P4.18 step 2): after FALLBACK, a NEW command for
    // the asset goes through the FULL PROPOSED → VALIDATED → FEASIBLE →
    // APPROVED path. The test drives the fresh command through the P4.10
    // domain engine and observes every gate in order.
    assert!(RECOVERY_RULE_STATEMENT.contains("FULL PROPOSED → VALIDATED → FEASIBLE → APPROVED"));
    assert!(RECOVERY_RULE_STATEMENT.contains("No shortcut from FALLBACK back to ISSUED"));

    let fresh = command_id(11);
    let clock = TestClock::at(100_000);
    let valid_until = ValidUntil::from_unix_millis(170_000).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(fresh, valid_until).expect("propose");
    let actor = Actor::new("test:driver").expect("actor");
    lifecycle
        .record_validation(
            GateOutcome::pass(EvidenceReference::new("validation:fresh").expect("evidence")),
            actor.clone(),
            &clock,
        )
        .expect("validation");
    lifecycle
        .record_feasibility(
            GateOutcome::pass(EvidenceReference::new("feasibility:fresh").expect("evidence")),
            actor.clone(),
            &clock,
        )
        .expect("feasibility");
    lifecycle
        .record_authorization(
            AuthorizationOutcome::approve(
                EvidenceReference::new("authz:fresh").expect("evidence"),
                actor.clone(),
            ),
            &clock,
        )
        .expect("authorization");
    lifecycle
        .record_issue(
            EvidenceReference::new("signature:fresh").expect("evidence"),
            actor,
            &clock,
        )
        .expect("issue");
    // Recovery works — but ONLY through the full chain (every gate is in
    // the journal, in order).
    let gates: Vec<_> = lifecycle
        .journal()
        .iter()
        .filter_map(|entry| entry.gate)
        .collect();
    assert_eq!(gates.len(), 3, "validation, feasibility, authorization — in order");
    assert_eq!(lifecycle.state(), CommandState::Issued);
}

#[test]
fn a_direct_fallback_to_issued_transition_is_rejected_by_the_frozen_edge_oracle() {
    // Build a command that reached FALLBACK, then attempt the illegal
    // shortcut FALLBACK → ISSUED directly against the domain engine.
    let command = command_id(12);
    let clock = TestClock::at(1_000);
    let valid_until = ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(command, valid_until).expect("propose");
    let actor = Actor::new("test:driver").expect("actor");
    lifecycle
        .record_validation(
            GateOutcome::pass(EvidenceReference::new("validation:t").expect("evidence")),
            actor.clone(),
            &clock,
        )
        .expect("validation");
    lifecycle
        .record_feasibility(
            GateOutcome::pass(EvidenceReference::new("feasibility:t").expect("evidence")),
            actor.clone(),
            &clock,
        )
        .expect("feasibility");
    lifecycle
        .record_authorization(
            AuthorizationOutcome::approve(
                EvidenceReference::new("authz:t").expect("evidence"),
                actor.clone(),
            ),
            &clock,
        )
        .expect("authorization");
    lifecycle
        .record_issue(
            EvidenceReference::new("signature:t").expect("evidence"),
            actor.clone(),
            &clock,
        )
        .expect("issue");
    lifecycle
        .record_failure(FailureReason::TransportTimeout, actor.clone(), &clock)
        .expect("failure");
    lifecycle
        .record_fallback(
            EvidenceReference::new(format!("{FALLBACK_POLICY_REF_PREFIX}hold-bess-60s"))
                .expect("evidence"),
            actor.clone(),
            &clock,
        )
        .expect("fallback");
    assert_eq!(lifecycle.state(), CommandState::Fallback);

    // The frozen table gives FALLBACK NO outgoing edge: the shortcut is
    // rejected by the oracle, and FALLBACK is terminal.
    assert!(matches!(
        try_transition(CommandState::Fallback, CommandState::Issued),
        Err(flexcore_domain::command_state::TransitionError::NotPermitted { .. })
    ));
    assert!(lifecycle.permitted_transitions().is_empty());
    assert!(lifecycle.is_terminal());
    let error = lifecycle
        .record_issue(
            EvidenceReference::new("signature:shortcut").expect("evidence"),
            actor,
            &clock,
        )
        .expect_err("FALLBACK → ISSUED does not exist in the frozen table");
    assert!(matches!(
        error,
        flexcore_domain::command_lifecycle::LifecycleError::NotPermitted { .. }
    ));
    // The journal was not touched by the rejected attempt.
    assert_eq!(
        lifecycle.journal().last().expect("last").to,
        CommandState::Fallback
    );
}

// --- The structural no-universal-zero-fallback scan (P4.17 acceptance) ----------------

/// Scans EVERY source file of this crate for a universal "set power to zero"
/// fallback: no default zero-power setpoint constructor, no universal
/// fallback function, no zero-power constant may exist. The §14.4 statement
/// constant is the POSITIVE half of the assertion. (The scan is scoped to
/// this crate's `src/` tree — the recorded in-test form of the ledger's
/// "code search for a universal fallback" verification.)
#[test]
fn no_universal_zero_power_fallback_exists_in_code() {
    assert!(NO_UNIVERSAL_FALLBACK_STATEMENT.contains("no universal"));
    assert!(NO_UNIVERSAL_FALLBACK_STATEMENT.contains("set power to zero"));

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut scanned = 0usize;
    let forbidden: [&str; 6] = [
        "fn universal",
        "UniversalFallback",
        "universal_zero",
        "ZERO_POWER",
        "zero_power",
        "ActivePowerMW::new(0.0)",
    ];
    let entries = fs::read_dir(&src).expect("crate src directory");
    for entry in entries {
        let entry = entry.expect("readable entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            scanned += 1;
            let source = fs::read_to_string(&path).expect("readable source");
            for marker in forbidden {
                assert!(
                    !source.contains(marker),
                    "file {} contains the forbidden universal-fallback marker `{marker}`",
                    path.display()
                );
            }
        }
    }
    assert!(scanned >= 15, "the scan must cover the crate sources; scanned {scanned}");
}
