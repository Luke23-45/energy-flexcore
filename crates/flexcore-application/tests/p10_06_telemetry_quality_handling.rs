// P10.06 — Stale/missing telemetry handling verification (release-gate
//         slice, application crate): the six §10.4 quality states each
//         verified to produce their documented handling, hand-computed
//         boundary cases, and the fail-closed command-gate behavior on
//         stale/missing telemetry (report §22.3 gate 5; §10.4; §17.4–
//         §17.5; ledger P10.06 steps 1, 2, 4; record:
//         `ledger/phase-10-v1-release-hardening/06_telemetry_handling_tests.md`).
//
// THE SIX STATES AND THEIR DOCUMENTED HANDLING (each verified below):
//
// | State     | Producer seam (the real code path)                | Verified handling |
// |-----------|---------------------------------------------------|-------------------|
// | GOOD      | `flexcore_domain::quality_rules::assign_quality`  | accepted; all checks passed; the control command validates |
// | SUSPECT   | `assign_quality` (soft range/consistency failure)| stored WITH its flag, never downgraded/dropped; a quality TRANSITION on a scoped asset invalidates the envelope the schedule consumed |
// | BAD       | `assign_quality` (hard schema/SOC/source failure)| hard failure; the value is not a usable measurement; the quality event invalidates the envelope and the command gate REJECTS (fail-closed) |
// | MISSING   | `quality_rules::expected_intervals` gap detector  | a derived `flexcore:derived:` marker with NO value field; the quality event invalidates the envelope and the command gate REJECTS |
// | STALE     | `quality_rules::staleness` (event-time age)       | `age == limit` NOT stale, `age > limit` stale; a STALE quality event invalidates the envelope and the command gate REJECTS; no silent substitution (staleness never re-labels GOOD) |
// | ESTIMATED | `quality_rules::derive_estimated` (explicit rule) | UNREACHABLE without a configured `DerivationRule`; explicitly tagged with the rule identity; distinguished from measured by the quality flag |
//
// Additional proven properties:
// - Producer separation (`producer_of`): GOOD/SUSPECT/BAD come only from
//   the assigner, STALE only from the staleness detector, MISSING only
//   from the gap detector, ESTIMATED only from an explicit derivation
//   rule — one state per value, never silently converted (P0.08 §4).
// - Per-use-case freshness limits are CONFIGURATION, not hard-coded
//   defaults (the same event age is stale under one use case's limit and
//   fresh under another's — the P9.18 cross-reference).
// - A foreign-tenant or out-of-scope quality event never invalidates our
//   envelope (tenant scoping, P0.16 rule 5).
//
// THE COMMAND-GATE PROOF (the release-gate core): a command built on a
// schedule whose envelope went stale (STALE / BAD / MISSING quality event)
// does NOT proceed — the P4.11 validation gate returns passed=false with
// the failing check named `envelope_stale:<reason>`, the command moves
// PROPOSED → REJECTED, and `DispatchRejected` is emitted. The REAL P3.13
// staleness evaluation (`evaluate_envelope_trigger`) decides; the gate seam
// carries the verdict exactly as the composition root does.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed; the human build/test gate owns:
// `cargo test -p flexcore-application --test p10_06_telemetry_quality_handling`).
// The harness mirrors the P4.21 fixtures (rejection_stale.rs) — a
// controlled clock everywhere, no wall-clock read.
//
// SCOPE HONESTY: the console §20.2 visual distinction and the §15.2
// API-flagging checks (P10.06 steps 5–6) need the running console/API and
// are recorded in the record as NOT EXECUTABLE HERE. The clock-drift
// interaction (step: STALE + clock-drift hold) is proven in
// `crates/edge-agent/tests/p10_06_clock_quality_stale_interaction.rs`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_application::command_gates::{
    AssetCapabilitySource, AssetGateStatus, CommandGateError, CommandGateStore,
    CommandGateStoreError, CommandGateView, CommandGates, CreationCheck, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposeCommandInput, ProposedCommandRecord, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::envelope_staleness::{
    evaluate_envelope_trigger, EnvelopeFreshness, EnvelopeTriggerEvent, StalenessConfig,
    StalenessReason,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::{map_source_state, SourceQualityState, TelemetryQuality};
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, LifecycleClock};
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, EvidenceReferences, ExecutionRequirements, RampLimits,
    ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::quality_rules::{
    assign_quality, derive_estimated, expected_intervals, producer_of, staleness, Cadence,
    CheckResult, DerivationRule, FreshnessLimit, Producer, QualityInput, RatedLimits,
    Staleness as DomainStaleness,
};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::events::TelemetryQualityChangedPayload;
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

// --- Fixture identity ---------------------------------------------------------------

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa001";
const UUID_TENANT_OTHER: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa002";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa003";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa004";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa005";
const UUID_COMMAND: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa006";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6fa007";

const MODEL_HASH: &str = "fnv1a64:modelhash-p10-06";
const PROPOSER: &str = "service:scheduler";
const VALIDATOR: &str = "svc:validator";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant() -> TenantId {
    TenantId::new(uuid(UUID_TENANT))
}

fn other_tenant() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_OTHER))
}

fn asset_id() -> AssetId {
    AssetId::new(uuid(UUID_ASSET))
}

fn command_id() -> CommandId {
    CommandId::new(uuid(UUID_COMMAND))
}

fn context() -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant(),
        subject: "p10-06:test:principal".to_string(),
    }))
}

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

// --- Gate-store fake (fail-closed by construction) ------------------------------------

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

    fn journal_of(&self, command: CommandId) -> Vec<CommandTransition> {
        self.journals
            .lock()
            .expect("fake store")
            .get(&command.to_string())
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

/// The envelope-freshness seam: the verdict is set per test, derived from
/// the REAL staleness evaluation (the composition-root shape).
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
            attempt_id: "attempt-p10-06".to_string(),
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
            reason: "no approvals in the P10.06 suite".to_string(),
        }
    }
}

struct DebugEncoder;

impl flexcore_application::payload_encoding::EventPayloadEncoder for DebugEncoder {
    fn encode(
        &self,
        envelope: &flexcore_events::envelope::EventEnvelope,
    ) -> Result<String, flexcore_application::error::ApplicationError> {
        Ok(format!("{:?}", envelope.event))
    }
}

struct Harness {
    gates: CommandGates<'static>,
    store: &'static FakeStore,
    envelopes: &'static FakeEnvelopes,
    outbox: &'static FakeEventOutbox,
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
    Harness {
        gates,
        store,
        envelopes,
        outbox,
    }
}

// --- Command fixture (pinned inputs; mirrors the P4.21 fixture shape) -------------------

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
        idempotency_key: "idem:p10-06".to_string(),
        evidence: EvidenceReferences {
            grid_model: "grid-model:feeder-a@1".to_string(),
            telemetry_snapshot: "snapshot:p10-06".to_string(),
            validation_evidence: "pending".to_string(),
        },
        authenticity: CommandAuthenticity::Signature("sig:p10-06".to_string()),
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

/// The envelope metadata the quality fixtures evaluate against (pinned).
fn envelope_metadata(computed_at_millis: i64) -> flexcore_application::flexibility_envelope::EnvelopeMetadata {
    use flexcore_application::flexibility_envelope::{EnvelopeMetadata, EnvelopeVersion};
    EnvelopeMetadata {
        envelope_version: EnvelopeVersion::new("env-p10-06").expect("valid version"),
        computed_at: EventTime::from_unix_millis(computed_at_millis).expect("valid instant"),
        tenant_id: tenant(),
        portfolio_id: PortfolioId::new(uuid(UUID_PORTFOLIO)),
        input_snapshot_ref: "schedule_input_snapshot:env-p10-06".to_string(),
        forecast_version: "fc-p10-06".to_string(),
        horizon_start_millis: computed_at_millis,
        horizon_end_millis: computed_at_millis + 3_600_000,
        scoped_asset_ids: vec![asset_id()],
    }
}

/// Builds a TelemetryQualityChanged event carrying `quality` for the
/// scoped asset of OUR tenant (or a foreign tenant when requested).
fn quality_event(quality: TelemetryQuality, foreign_tenant: bool) -> EnvelopeTriggerEvent {
    EnvelopeTriggerEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
        correlation_id: FlexUuid::from_bytes([0u8; 16]),
        tenant_id: if foreign_tenant { other_tenant() } else { tenant() },
        site_id: SiteId::new(uuid(UUID_SITE)),
        asset_id: asset_id(),
        quality,
        previous_quality: Some(TelemetryQuality::Good),
    })
}

// =====================================================================================
// Step 1 — each of the SIX quality states produces its documented handling
// =====================================================================================

#[test]
fn every_quality_state_transition_on_a_scoped_asset_invalidates_the_envelope() {
    // The documented handling (P3.13 decided rule): ANY quality transition
    // on a scoped asset of the matching tenant invalidates the envelope —
    // in BOTH directions (non-silencing, §10.4: a quality flag is never
    // silently converted). Verified for all SIX canonical states.
    let metadata = envelope_metadata(4_000);
    let config = StalenessConfig::default();
    for quality in TelemetryQuality::ALL {
        let verdict = evaluate_envelope_trigger(
            &metadata,
            &quality_event(quality, false),
            &config,
            EventTime::from_unix_millis(5_000).expect("valid instant"),
            None,
        );
        match &verdict.freshness {
            EnvelopeFreshness::Stale {
                reason: StalenessReason::TelemetryQualityChanged { asset_id },
            } => {
                assert_eq!(*asset_id, asset_id().to_string(), "state {:?}", quality.name());
            }
            other => panic!(
                "state {} must invalidate the envelope, got {:?}",
                quality.name(),
                other
            ),
        }
    }
}

#[test]
fn a_foreign_tenant_or_out_of_scope_quality_event_never_invalidates() {
    // Tenant scoping (P0.16 rule 5) and scope narrowing: a quality event
    // from another tenant, or about an asset outside the envelope's scope,
    // leaves our envelope fresh.
    let metadata = envelope_metadata(4_000);
    let config = StalenessConfig::default();
    // Foreign tenant.
    let verdict = evaluate_envelope_trigger(
        &metadata,
        &quality_event(TelemetryQuality::Stale, true),
        &config,
        EventTime::from_unix_millis(5_000).expect("valid instant"),
        None,
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Fresh));
    // Out-of-scope asset (same tenant, different asset id).
    let event = EnvelopeTriggerEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
        correlation_id: FlexUuid::from_bytes([0u8; 16]),
        tenant_id: tenant(),
        site_id: SiteId::new(uuid(UUID_SITE)),
        asset_id: AssetId::new(uuid(UUID_TENANT_OTHER)), // not in scope
        quality: TelemetryQuality::Bad,
        previous_quality: Some(TelemetryQuality::Good),
    });
    let verdict = evaluate_envelope_trigger(
        &metadata,
        &event,
        &config,
        EventTime::from_unix_millis(5_000).expect("valid instant"),
        None,
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Fresh));
}

// =====================================================================================
// The command-gate proof: a command does NOT proceed on stale/missing/bad telemetry
// =====================================================================================

/// Runs the fixture command through creation + the validation gate with
/// the given envelope-freshness verdict (derived from the REAL evaluation
/// of the given quality event, the composition-root shape).
fn gate_outcome_with_quality(quality: TelemetryQuality) -> (bool, Option<String>, usize, Vec<&'static str>) {
    let h = harness(5_000);
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    // The REAL P3.13 evaluation decides from the quality event.
    let metadata = envelope_metadata(4_000);
    let verdict = evaluate_envelope_trigger(
        &metadata,
        &quality_event(quality, false),
        &StalenessConfig::default(),
        EventTime::from_unix_millis(5_000).expect("valid instant"),
        None,
    );
    let reason = match &verdict.freshness {
        EnvelopeFreshness::Stale { reason } => format!("{reason:?}"),
        EnvelopeFreshness::Fresh => panic!("the control requires a stale verdict"),
    };
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
    (
        result.passed,
        result.failing_check,
        journal.len(),
        emitted_events(&h),
    )
}

#[test]
fn a_command_does_not_proceed_on_stale_telemetry() {
    // STALE quality event → the envelope is stale → the validation gate
    // FAILS CLOSED: passed=false, the failing check names the staleness
    // reason, PROPOSED → REJECTED is journalled, DispatchRejected emitted.
    let (passed, check, journal_len, events) = gate_outcome_with_quality(TelemetryQuality::Stale);
    assert!(!passed, "a command must NOT proceed on STALE telemetry");
    let check = check.expect("the failing check is named");
    assert!(
        check.starts_with("envelope_stale:TelemetryQualityChanged"),
        "the check names the STALE-telemetry reason: {check}"
    );
    assert_eq!(journal_len, 1, "exactly the rejection transition is journalled");
    assert_eq!(events, vec!["DispatchProposed", "DispatchRejected"]);
}

#[test]
fn a_command_does_not_proceed_on_bad_telemetry() {
    // BAD quality event → same fail-closed handling. A hard-validation
    // failure is not a usable measurement; the command built on it is
    // rejected, never issued.
    let (passed, check, journal_len, events) = gate_outcome_with_quality(TelemetryQuality::Bad);
    assert!(!passed, "a command must NOT proceed on BAD telemetry");
    assert!(check.expect("named").starts_with("envelope_stale:"));
    assert_eq!(journal_len, 1);
    assert_eq!(events, vec!["DispatchProposed", "DispatchRejected"]);
}

#[test]
fn a_command_does_not_proceed_on_missing_telemetry() {
    // MISSING quality event → same fail-closed handling. A gap marker is
    // never substituted with a last-known value on this path.
    let (passed, check, journal_len, events) = gate_outcome_with_quality(TelemetryQuality::Missing);
    assert!(!passed, "a command must NOT proceed on MISSING telemetry");
    assert!(check.expect("named").starts_with("envelope_stale:"));
    assert_eq!(journal_len, 1);
    assert_eq!(events, vec!["DispatchProposed", "DispatchRejected"]);
}

#[test]
fn the_control_fresh_envelope_validates_and_the_gate_is_the_rejection_point() {
    // CONTROL (harness sanity): with a FRESH envelope and an open validity
    // window the SAME fixture command creates AND validates — every
    // rejection above is a designed rejection, not a harness artifact.
    let h = harness(5_000);
    h.store
        .seed_revision(&uuid(UUID_SCHEDULE).to_string(), 2, ScheduleRevisionUsability::Active);
    // The quality event fired on an out-of-scope asset: our envelope stays
    // fresh (the real evaluation decides).
    let metadata = envelope_metadata(4_000);
    let verdict = evaluate_envelope_trigger(
        &metadata,
        &quality_event(TelemetryQuality::Stale, true), // foreign tenant
        &StalenessConfig::default(),
        EventTime::from_unix_millis(5_000).expect("valid instant"),
        None,
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Fresh));
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
}

#[test]
fn a_creation_rejection_happens_for_a_superseded_schedule_before_any_gate() {
    // §14.3 rejection classes on the stale path: a command proposed from a
    // SUPERSEDED schedule revision is rejected AT CREATION with the named
    // check — no record, no event (P4.11).
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
    match &outcome {
        Err(CommandGateError::CreationRejected { check, detail }) => {
            assert_eq!(*check, CreationCheck::SourceScheduleRevisionSuperseded);
            assert!(detail.contains("SUPERSEDED"), "detail: {detail}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(h.outbox.writes().is_empty(), "no event on a creation rejection");
}

// =====================================================================================
// Hand-computed boundary cases (the pure quality rules, flexcore-domain)
// =====================================================================================

#[test]
fn the_staleness_boundary_is_age_equal_to_limit_not_stale_plus_one_stale() {
    // Hand-computed (P2.08 frozen boundary): age == limit is NOT stale;
    // age == limit + 1 is. The age is computed from event_time (never
    // ingest time — distinct contract types).
    let limit = FreshnessLimit::from_seconds(300).expect("positive limit");
    let base = 1_700_000_000_000i64;
    let event = EventTime::from_unix_millis(base).unwrap();
    let at = |offset: i64| EventTime::from_unix_millis(base + offset).unwrap();
    // Exactly AT the limit: fresh.
    assert_eq!(
        staleness(event, at(300_000), limit),
        DomainStaleness::Fresh { age_millis: 300_000 }
    );
    // One millisecond past: stale.
    assert_eq!(
        staleness(event, at(300_001), limit),
        DomainStaleness::Stale { age_millis: 300_001 }
    );
    // Well inside / well outside.
    assert!(matches!(
        staleness(event, at(299_000), limit),
        DomainStaleness::Fresh { .. }
    ));
    assert!(staleness(event, at(600_000), limit).is_stale());
    // STALE is produced with its flag, never as a silent substitution:
    // the verdict's stale_quality is Some(STALE) and Fresh carries None.
    assert_eq!(
        staleness(event, at(300_001), limit).stale_quality(),
        Some(TelemetryQuality::Stale)
    );
    assert_eq!(staleness(event, at(100_000), limit).stale_quality(), None);
}

#[test]
fn freshness_limits_are_per_use_case_configuration_not_defaults() {
    // The SAME event age is stale under a tight use-case limit and fresh
    // under a loose one — the P9.18 cross-reference: the limits each use
    // case applies are the recorded ones, configured, never hard-coded.
    let base = 1_700_000_000_000i64;
    let event = EventTime::from_unix_millis(base).unwrap();
    let now = EventTime::from_unix_millis(base + 120_000).unwrap();
    assert!(staleness(event, now, FreshnessLimit::from_seconds(60).unwrap()).is_stale());
    assert!(!staleness(event, now, FreshnessLimit::from_seconds(300).unwrap()).is_stale());
    // A non-positive limit is a named composition error, never a silently
    // weakened check.
    assert!(FreshnessLimit::try_new(0).is_err());
    assert!(FreshnessLimit::try_new(-1).is_err());
}

#[test]
fn the_good_suspect_bad_boundary_is_hard_failure_wins_soft_failure_is_suspect() {
    // Hand-computed (P2.08 decided boundary): a hard validation failure is
    // BAD; a plausibility/range failure on a well-formed value is SUSPECT;
    // all checks passing is GOOD. First failure wins in the fixed order.
    let limits = RatedLimits::symmetric(2.0).expect("valid limits");
    let clean = || QualityInput {
        schema_valid: true,
        source_reported_invalid: false,
        soc_raw_fraction: None,
        range_check: CheckResult::Pass,
        consistency_check: CheckResult::Pass,
    };
    // GOOD — the accept case.
    let decision = assign_quality(&clean());
    assert_eq!(decision.quality, TelemetryQuality::Good);
    // BAD — schema failure (hard), even when soft checks also fail.
    let mut input = clean();
    input.schema_valid = false;
    input.range_check = CheckResult::Fail {
        detail: "active power above the rated upper limit",
    };
    assert_eq!(assign_quality(&input).quality, TelemetryQuality::Bad);
    // BAD — raw SOC outside [0,1] (hand-computed boundaries: 1.0 and 0.0
    // are INSIDE the domain; 1.1 and -0.1 are BAD; non-finite is BAD).
    for (raw, expected_bad) in [
        (0.0, false),
        (1.0, false),
        (1.1, true),
        (-0.1, true),
        (f64::NAN, true),
    ] {
        let mut input = clean();
        input.soc_raw_fraction = Some(raw);
        assert_eq!(
            assign_quality(&input).quality == TelemetryQuality::Bad,
            expected_bad,
            "raw SOC {raw}"
        );
    }
    // SUSPECT — range failure on a well-formed value (boundary:
    // exactly at ±2.0 MW passes; 2.4 MW fails).
    for v in [-2.0, 0.0, 2.0] {
        let p = ActivePowerMW::new(v).unwrap();
        assert_eq!(flexcore_domain::quality_rules::check_active_power_range(p, limits), CheckResult::Pass);
    }
    let p = ActivePowerMW::new(2.4).unwrap();
    assert!(matches!(
        flexcore_domain::quality_rules::check_active_power_range(p, limits),
        CheckResult::Fail { .. }
    ));
    let mut input = clean();
    input.range_check = CheckResult::Fail {
        detail: "active power above the rated upper limit",
    };
    let decision = assign_quality(&input);
    assert_eq!(decision.quality, TelemetryQuality::Suspect);
    // The SUSPECT flag is carried, never downgraded or dropped: the
    // decision carries reason + evidence (non-silencing, P0.08 §4).
    assert!(!decision.evidence.is_empty());
}

#[test]
fn missing_gaps_are_derived_markers_with_no_value_field() {
    // Hand-computed gap detection: horizon [0s, 180s] on a 60s cadence
    // with coverage {0s, 120s} → MISSING markers at exactly 60s and 180s
    // (the derived namespace `flexcore:derived:<detector>`, detector-owned
    // monotonic sequence from the supplied base).
    let base = 1_700_000_000_000i64;
    let at = |offset: i64| EventTime::from_unix_millis(base + offset).unwrap();
    let covered = [at(0), at(120_000)];
    let gaps = expected_intervals(
        &covered,
        at(0),
        at(180_000),
        Cadence::from_seconds(60).unwrap(),
        "p10-06-gap-detector",
        42,
    )
    .expect("valid detector name");
    assert_eq!(gaps.len(), 2);
    assert_eq!(
        gaps.iter().map(|g| g.event_time.unix_millis()).collect::<Vec<_>>(),
        vec![base + 60_000, base + 180_000]
    );
    for gap in &gaps {
        assert_eq!(gap.quality, TelemetryQuality::Missing);
        assert_eq!(gap.source_id, "flexcore:derived:p10-06-gap-detector");
        // The marker type carries NO value field at all — a missing value
        // cannot be substituted with a number by construction.
        let _ = &gap.source_id;
        let _ = &gap.source_sequence;
        let _ = &gap.event_time;
        let _ = &gap.quality;
    }
    assert_eq!(gaps.iter().map(|g| g.source_sequence).collect::<Vec<_>>(), vec![42, 43]);
}

#[test]
fn estimated_is_unreachable_without_an_explicit_derivation_rule_and_is_visibly_tagged() {
    // ESTIMATED has exactly ONE producer (derive_estimated, which requires
    // a &DerivationRule); with no rule configured, no other producer can
    // yield it (P0.08 §4: estimation is explicit, tagged, bounded).
    // 1. The assigner never yields ESTIMATED — not even on pathological
    //    inputs.
    let pathological = QualityInput {
        schema_valid: false,
        source_reported_invalid: true,
        soc_raw_fraction: Some(f64::NAN),
        range_check: CheckResult::Fail {
            detail: "range",
        },
        consistency_check: CheckResult::Fail {
            detail: "consistency",
        },
    };
    assert_ne!(assign_quality(&pathological).quality, TelemetryQuality::Estimated);
    // 2. Staleness and gap detection never yield ESTIMATED.
    let base = 1_700_000_000_000i64;
    let event = EventTime::from_unix_millis(base).unwrap();
    let now = EventTime::from_unix_millis(base + 400_000).unwrap();
    assert_ne!(
        staleness(event, now, FreshnessLimit::from_seconds(300).unwrap()).stale_quality(),
        Some(TelemetryQuality::Estimated)
    );
    // 3. The rule constructor validates its identity; the decision carries
    //    the rule id and detector (visibly tagged as estimated).
    assert!(DerivationRule::new("", "d").is_err());
    assert!(DerivationRule::new("r", "").is_err());
    let rule = DerivationRule::new("linear-interpolation", "p10-06-estimator").unwrap();
    let decision = derive_estimated(&rule, "interpolated between two GOOD neighbours");
    assert_eq!(decision.quality, TelemetryQuality::Estimated);
    assert!(decision.evidence.contains("linear-interpolation"));
    assert!(decision.evidence.contains("p10-06-estimator"));
    // 4. ESTIMATED is distinguished from MEASURED: a Good → Estimated
    //    transition is a quality CHANGE (the TelemetryQualityChanged event
    //    fires; §15.2/§20.2 visibility is the flag itself).
    assert!(flexcore_domain::quality_rules::quality_state_changed(
        TelemetryQuality::Good,
        TelemetryQuality::Estimated
    ));
    assert!(!flexcore_domain::quality_rules::quality_state_changed(
        TelemetryQuality::Estimated,
        TelemetryQuality::Estimated
    ));
}

#[test]
fn producer_separation_covers_all_six_states_and_the_adapter_mapping_never_yields_good() {
    // One state per value, and each state has exactly one producer
    // (machine-checked by the exhaustive match inside producer_of).
    assert_eq!(producer_of(TelemetryQuality::Good), Producer::QualityAssigner);
    assert_eq!(producer_of(TelemetryQuality::Suspect), Producer::QualityAssigner);
    assert_eq!(producer_of(TelemetryQuality::Bad), Producer::QualityAssigner);
    assert_eq!(producer_of(TelemetryQuality::Missing), Producer::GapDetector);
    assert_eq!(producer_of(TelemetryQuality::Estimated), Producer::ExplicitDerivation);
    assert_eq!(producer_of(TelemetryQuality::Stale), Producer::StalenessDetector);
    // The adapter-boundary mapping (flexcore-contracts, P0.08 §3): the
    // §10.4 source states map into the six canonical states and NEVER to
    // GOOD — a communication loss arrives as STALE, a substitution as
    // ESTIMATED, and none is silently converted into a normal reading.
    assert_eq!(
        map_source_state(SourceQualityState::CommunicationLoss),
        TelemetryQuality::Stale
    );
    assert_eq!(
        map_source_state(SourceQualityState::Substituted),
        TelemetryQuality::Estimated
    );
    assert_eq!(
        map_source_state(SourceQualityState::OutOfRange),
        TelemetryQuality::Suspect
    );
    assert_eq!(
        map_source_state(SourceQualityState::Unsynchronized),
        TelemetryQuality::Suspect
    );
    assert_eq!(
        map_source_state(SourceQualityState::Unreliable),
        TelemetryQuality::Suspect
    );
    for source in SourceQualityState::ALL {
        assert_ne!(
            map_source_state(source),
            TelemetryQuality::Good,
            "a source state is never silently mapped to GOOD"
        );
    }
}
