// P5.14 — The scenario-run composition (§19.1, §8.1 `apps/sim-runner`;
//         report §24 vertical slice, extended from steps 5–8 to steps 9–10;
//         ledger P5.14).
//
// This module is the composition the P5.14 subtask delivers: a full
// scenario run — telemetry ingest → schedules → grid feasibility → dispatch
// to SIMULATOR — executed against the REAL Phase 2 simulator and the REAL
// Phase 4 adapter chain, in-process, deterministic, on the scenario clock.
// It extends the P2.24 replay path (`replay.rs`, §24 steps 1–2) with the
// dispatch half of the slice; §24 steps 9–10 (console display, replay
// verification) are the phase's exit demonstration (P5.13/P5.18), not this
// module's.
//
// ## The adapter inventory (the run record names real vs seam — P5.14)
//
// | Leg | Adapter | Implementation |
// |---|---|---|
// | telemetry ingest | `SimulatorTelemetrySource` (Phase 2 simulator) through the `TelemetrySource` port | **REAL** — the same provider the control plane's ingestion composes (`apps/worker` `compose_ingestion`) |
// | dispatch (issue/ack/tracking) | `SimulatorDispatchAdapter` (P4.16) over [`SimulatorTelemetryTransport`] | **REAL** — the transport is the composition-root implementation over `flexcore-simulator`; NO transport fake is used |
// | command gates (P4.11–P4.13) | `CommandGates` over the in-memory seams below | **SEAM-LEVEL** — the record-store/outbox/registry seams are the ports with in-memory composition support; the gates, validators, and lifecycle engine are the real application code |
// | grid feasibility | `run_feasibility_gate` (P4.04–P4.07) + `PandapowerGridProvider` over a selectable backend | **REAL** pipeline; the backend is `Scripted` (seam-level, in-process double) or `WorkerProcess` (the P5.15 Python leg — REAL, never executed here) |
// | schedule / rolling roll | `plan_rolling_horizon_roll` (P3.24) | **REAL** planner over realised values taken from the REAL simulator telemetry; the re-solve itself is the composition root's optimizer port and stays seam-level |
//
// The `ScenarioRunRecord` carries this inventory per run (P5.14
// acceptance: "the run record proves which adapters were real vs
// seam-level"), plus the simulator version, the scenario version, and the
// per-asset model identities (P5.14 step 3, the P5.19 reproducibility
// input).
//
// ## The §24 step 8 fault mapping (D-P4.16-4), implemented over the
// scenario configuration
//
// The recorded mapping is implemented by [`SimulatorTelemetryTransport`]:
//
// - `drop` → NO acceptance: a delivery attempted while the target asset is
//   inside an enabled drop window fails with the timeout kind (the
//   simulator does not answer — the frame would not exist either);
// - `source_invalid` → NACK at issue;
// - `out_of_range` → an out-of-tolerance tracking sample (the real
//   simulator emits the scaled value; the acceptance criterion sees it);
// - for TRACKING observations the mapping is driven by the REAL stream:
//   a drop window emits no frames, so the observation is genuinely absent
//   (never synthesized); a BAD (`source_invalid`) sample is not a usable
//   measurement and is skipped (it can neither satisfy nor violate the
//   criterion) — the poll proceeds and, if nothing usable arrives, the
//   recorded tracking timeout fires. No quality state is ever coerced.
//
// ## Determinism and clocks
//
// Every timestamp in a run comes from the scenario clock. The composition
// owns ONE controlled clock ([`RunClock`], the `LifecycleClock` the gates
// and the adapter read) and advances it together with the simulator's pump
// position via [`ScenarioRunHarness::advance_to`] — the adapter's `now`
// and the telemetry stream's release position never diverge. No wall clock
// is read anywhere in this module (P0.09 §5; the P4.16 controlled-clock
// discipline).
//
// ## Scope and control status
//
// Everything this module produces is synthetic, simulation-only (the
// entire 0.x line, §21.4): every issued command is issued to the SIMULATOR
// through the P4.16 adapter and nothing else; an acknowledgement is
// application-layer acceptance, NEVER physical delivery (§14.1); nothing
// here implies field control, safety, certification, or any
// protection-grade function (§14.5). A within-limits grid verdict means
// the candidate satisfied the declared model and limits within stated
// tolerances in simulation — not that it is safe in the field.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_application::command_gates::{
    ApprovalScope, AssetCapabilitySource, AssetGateStatus, AuthorizationDecision,
    AuthorizationDecisionRecord, CommandAuthorizationEvaluator, CommandAuthorizationRequest,
    CommandAuthzAction, CommandFeasibilityRefs, CommandFeasibilityRunner, CommandGatePolicy,
    CommandGateStore, CommandGateStoreError, CommandGateView, CommandGates, DeclaredQuantities,
    EnvelopeGateFreshness, FeasibilityGateVerdict, GateSeamError, ModelVersionGateStatus,
    ModelVersionSource, ProposedCommandRecord, ProposeCommandInput, ScheduleEnvelopeSource,
    ScheduleRevisionUsability, COMMAND_ENTITY,
};
use flexcore_application::dispatch_simulator::{
    AdapterTargetCandidate, SimulatedAcceptance, SimulatedObservation, SimulationKeyring,
    SimulatorDispatchAdapter, SimulatorDispatchEnvelope, SimulatorDispatchTransport,
    SimulatorTransportError, SimulatorTransportFailure, TrackingAcceptanceCriterion,
    TrackingTerminal, TrackingVerdict, TrackedQuantity, SIMULATOR_TRANSPORT_IDENTITY_PREFIX,
};
use flexcore_application::error::{ApplicationError, InfrastructureError};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_application::rolling_horizon::{
    plan_rolling_horizon_roll, AssetDropout, CarriedAssetState, PriorRevision, RealisedAssetValue,
    RealisedInterval, RealisedState, RollRequest, RollTrigger,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{Actor, LifecycleClock};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::envelope::EventEnvelope;
use flexcore_grid_api::adapter::{
    balanced_worker_scope, PandapowerGridProvider, PROVIDER_ID, PROVIDER_PINNED_VERSION,
};
use flexcore_grid_api::backend::{
    BackendDescription, BackendInjection, BackendPowerFlowOutcome, BackendRunOptions, GridModelBackend,
};
use flexcore_grid_api::feasibility::{
    run_feasibility_gate, CandidateSetpoint, CandidateValidationConfig, DeclaredBounds,
    EnvelopeFreshness, FeasibilityCandidate, GateAbort, GatePipelineOutcome, GateRejection,
    GateRunRequest, GateSources, RevisionState, SeamError, TelemeteredState,
};
use flexcore_grid_api::limits::{check_limits, GeneratorLimitSet, LimitCheckReport, LimitCheckStatus};
use flexcore_grid_api::model_health::{
    ModelCoverage, ModelHealthConfig, ModelLifecycleState, ModelVersionRegistry, RegistryElement,
    RegistryElementKind, VersionedModel,
};
use flexcore_grid_api::port::{
    BranchSpec, BusSpec, ExternalGridSpec, GridModelProvider, GridModelRef, NetworkLimits,
    TransformerSpec, VersionedNetworkModel,
};
use flexcore_ports::fakes::{CountingIdGenerator, FakeEventOutbox, FakeWriteJournal};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_ports::telemetry_source::{
    QuantityValue, TelemetryQuery, TelemetryQuantity, TelemetrySource, TelemetrySubscription,
};
use flexcore_simulator::scenario::FaultKind;
use flexcore_simulator::{load_scenario, Scenario, SimulatorTelemetrySource, SIMULATOR_CRATE_VERSION};
use serde::Serialize;

use crate::replay::quality_name;

// --- The recorded statements ---------------------------------------------------

/// The simulation-only boundary statement carried on every run record
/// (§21.4; §14.1; the P4.32 handover §2 boundary, restated for Phase 5).
pub const P5_14_SIMULATION_ONLY_STATEMENT: &str = "\
    Every command in a scenario run is issued to the SIMULATOR and nothing else; \
    an acknowledgement is application-layer acceptance, never physical delivery; \
    the entire run is synthetic simulation telemetry on the scenario clock (0.x line).";

/// The grid-leg solver-class statement (AGENTS.md rule 4; §12.5): what the
/// grid leg runs is an AC power-flow evaluation, never an AC-OPF.
pub const P5_GRID_SOLVER_STATEMENT: &str = "\
    The grid feasibility leg runs an AC power-flow evaluation of specified \
    injections and controls through the Phase 4 pipeline; it is NOT an AC-OPF \
    optimizer and the gate never optimizes.";

/// The grid-leg provider boundary statement, carried verbatim from the
/// P4.02 adapter (§12.4).
pub const P5_GRID_PROVIDER_BOUNDARY: &str = flexcore_grid_api::adapter::PROVIDER_BOUNDARY_STATEMENT;

/// The tracking-observation quality rule recorded once (module header,
/// fourth mapping bullet).
pub const TRACKING_QUALITY_RULE: &str = "\
    Tracking observations serve GOOD and SUSPECT samples only; a BAD sample is \
    not a usable measurement and is skipped (it can neither satisfy nor violate \
    the criterion) — no quality state is ever coerced.";

// --- The run record ------------------------------------------------------------

/// Whether a run leg executed through a real adapter or a seam-level
/// composition support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterImplementation {
    /// The real adapter (the platform's own implementation).
    Real,
    /// A seam-level composition support (an injected port over an
    /// in-memory stand-in; named, never hidden).
    Seam,
}

impl AdapterImplementation {
    /// The stable text form the record carries.
    pub fn as_str(self) -> &'static str {
        match self {
            AdapterImplementation::Real => "real",
            AdapterImplementation::Seam => "seam",
        }
    }
}

/// One adapter-inventory entry of the run record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterTrace {
    /// The run leg (`telemetry_ingest`, `dispatch`, `command_gates`,
    /// `grid_feasibility`, `schedule_roll`).
    pub leg: String,
    /// The adapter identity (the port implementation named).
    pub adapter: String,
    /// Real or seam-level.
    pub implementation: AdapterImplementation,
    /// The recorded detail (what the seam stands in for, or which real
    /// path executed).
    pub detail: String,
}

/// The per-asset model identity recorded per run (P5.14 step 3): the
/// asset models of the Phase 2 simulator ARE the scenario's versioned
/// behaviour profiles, so the scenario version pins them; the per-asset
/// identity (key, type, rated magnitudes) travels for the record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssetModelVersion {
    /// The stable scenario asset key.
    pub asset_key: String,
    /// The asset type (the parsed scenario enum's Debug form).
    pub asset_type: String,
    /// Rated active power in MW (rated peak for a flexible load).
    pub rated_power_mw: f64,
    /// Rated energy in MWh (BESS only).
    pub rated_energy_mwh: Option<f64>,
}

/// One recorded availability change (P5.14 step 4: the dropout is visible
/// in the run record as an availability change).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AvailabilityChange {
    /// The asset key.
    pub asset_key: String,
    /// The instant the availability changed (scenario clock, UTC millis).
    pub at_unix_millis: i64,
    /// The observed availability (a drop window ⇒ unavailable).
    pub available: bool,
    /// The evidence kind (`fault_plan_drop_window`).
    pub evidence: String,
}

/// One recorded dropout (the P3.24 roll trigger input).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordedDropout {
    /// The asset key.
    pub asset_key: String,
    /// The dropout start (start-inclusive, scenario clock millis).
    pub from_millis: i64,
}

/// A summary of the P3.24 roll plan the composition produced for a
/// dropout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RollPlanSummary {
    /// The trigger (`asset_dropout`).
    pub trigger: String,
    /// The trigger detail recorded with the revision.
    pub trigger_detail: String,
    /// The superseded revision identity (schedule id, revision).
    pub superseded: String,
    /// The new revision identity (schedule id, revision).
    pub new_revision: String,
    /// The re-optimization window `[start, end)` millis.
    pub reoptimize_start_millis: i64,
    pub reoptimize_end_millis: i64,
    /// The assets available to the re-optimized window.
    pub active_asset_keys: Vec<String>,
    /// The recorded notes (frozen rule, dropped assets).
    pub notes: Vec<String>,
}

/// One dispatch command's outcome summary in the run record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandRunSummary {
    /// The command id (canonical UUID text).
    pub command_id: String,
    /// The target asset key.
    pub asset_key: String,
    /// The final lifecycle state (e.g. COMPLETED / FAILED).
    pub final_state: String,
    /// The issue outcome (`accepted` / `nack` / `timeout` /
    /// `transport_failure` / `rejected:<check>`).
    pub issue_outcome: String,
    /// The acceptance reference when accepted (application-layer
    /// acceptance — never physical delivery, §14.1).
    pub acceptance_reference: Option<String>,
    /// The tracking outcome (`completed` / `deviation` / `timeout` /
    /// `keep_tracking` / terminal-state text), when tracking ran.
    pub tracking_verdict: Option<String>,
}

/// One grid feasibility check summary in the run record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GridCheckSummary {
    /// The attempt identifier.
    pub attempt_id: String,
    /// The backend that ran (`scripted` / `worker_process`).
    pub backend: String,
    /// The provider id + version the result names.
    pub provider_id_version: String,
    /// The verdict (`passed` / `limit_violated` / `fail_closed`).
    pub verdict: String,
    /// The constraint / fail-closed reason, when not passed.
    pub reason: Option<String>,
}

/// One recorded P4.37 closed-loop consumption decision of a run (P5.18):
/// the frozen D-P4.07-1 routing of the slice's pipeline outcome, and the
/// consumption action the P4.37 contract planned for it. The summary
/// travels ON the run record (`consumption_decisions`); the re-scheduling
/// consumer itself stays the existing P3.24 rolling-horizon trigger path
/// (the frozen trigger set is NOT modified — the trigger gap travels per
/// the P4.37 open-item statement, restated in the summary's statements).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumptionDecisionSummary {
    /// The command the slice dispatched.
    pub command_id: String,
    /// The frozen outcome code the routing produced (the
    /// `VerdictOutcome::as_str` vocabulary), or `not_routed` when the
    /// slice ended before the feasibility pipeline ran.
    pub outcome_code: String,
    /// The routing decision reference (D-P4.07-1) — on every routing.
    pub decision_ref: String,
    /// The P4.37 consumption action (`no_consumption` / `re_optimize` /
    /// `constrain_rerun` / `reject_unconsumable` / `not_planned`).
    pub action: String,
    /// The no-consumption route, when the action is `no_consumption`.
    pub route: Option<String>,
    /// The recorded reason (the verdict's own audit text, or the contract
    /// error / failure-mode reason).
    pub reason: String,
    /// The named violated constraints (empty when none).
    pub violated_constraints: Vec<String>,
    /// The per-constraint problem-input statements the contract produced
    /// (the re-optimization inputs).
    pub problem_inputs: Vec<String>,
    /// The feasibility-check evidence reference.
    pub check_id: String,
    /// The model id + version the evidence binds to.
    pub model_id_version: String,
    /// The recorded roll-trigger note (the P4.37 open item).
    pub roll_trigger_note: String,
    /// The recorded statements (the closed-loop wiring statement, the
    /// consumer-boundary statement).
    pub statements: Vec<String>,
}

impl GridCheckSummary {
    /// Attaches the attempt identity of a PASSED check (from the real
    /// feasibility evidence).
    fn with_attempt_id(mut self, attempt_id: String) -> Self {
        self.attempt_id = attempt_id;
        self
    }
}

/// The P5.14 scenario-run record: run identity, versions, adapter
/// inventory, telemetry quality accounting, availability changes, the
/// dropout roll, and the dispatch outcomes. Serializable (the run-record
/// artefact form; P5.18 consumes it as a replay input).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioRunRecord {
    /// The scenario id (from the validated artefact).
    pub scenario_id: String,
    /// The scenario version — pins the behaviour profiles (the asset
    /// models).
    pub scenario_version: String,
    /// The master seed in effect (D-SC4).
    pub master_seed: u64,
    /// The code version the run names (the sim-runner convention:
    /// `--code-version`, else `FLEXCORE_CODE_VERSION`, else
    /// `dev-unrecorded`).
    pub code_version: String,
    /// The simulator crate version (P5.14 step 3).
    pub simulator_crate_version: String,
    /// The telemetry source identity (`sim://<scenario_id>`).
    pub simulator_source_id: String,
    /// The per-asset model identities.
    pub asset_models: Vec<AssetModelVersion>,
    /// The adapter inventory: real vs seam-level, per leg.
    pub adapters: Vec<AdapterTrace>,
    /// The number of telemetry records ingested through the real source.
    pub records_ingested: usize,
    /// The quality-state accounting of the ingested records (exactly one
    /// state per value; the P5.03 display consumes the same flags).
    pub quality_counts: BTreeMap<String, usize>,
    /// The recorded availability changes (P5.14 step 4).
    pub availability_changes: Vec<AvailabilityChange>,
    /// The recorded dropouts (the P3.24 trigger input).
    pub dropouts: Vec<RecordedDropout>,
    /// The rolling-roll plan the composition produced (when a dropout
    /// fired).
    pub roll_plan: Option<RollPlanSummary>,
    /// The dispatch command outcomes.
    pub commands: Vec<CommandRunSummary>,
    /// The grid feasibility check outcomes.
    pub grid_checks: Vec<GridCheckSummary>,
    /// The P4.37 closed-loop consumption decisions the run recorded
    /// (P5.18: a RETURN/CONSTRAIN outcome inside a scenario run feeds
    /// `plan_consumption` and the decision is RECORDED here; the
    /// re-scheduling consumer stays the existing P3.24 trigger path).
    #[serde(default)]
    pub consumption_decisions: Vec<ConsumptionDecisionSummary>,
    /// The recorded statements (simulation-only boundary, solver class,
    /// provider boundary, tracking-quality rule, seam inventory).
    pub statements: Vec<String>,
    /// P5.21/P5.22 — the run's fault journal (§19.2 layer 6): one factual
    /// entry per scenario fault spec (the `scenario_faults` v3 slot or the
    /// 1:1 P2.15 fault-plan derivation), carrying fault id, injection
    /// point, observed effect, and the platform's response + evidence ref.
    /// `None` when the scenario carries no fault configuration (the field
    /// is omitted from the JSON then — old consumers unaffected). Built by
    /// [`crate::fault_injection::build_fault_journal`] and attached with
    /// [`ScenarioRunRecord::with_fault_journal`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault_journal: Option<crate::fault_injection::FaultJournal>,
    /// P5.23 — the run's per-leg wall-clock timings, when measured
    /// (additive; MEASUREMENT clock only — the run's STATE clock stays the
    /// scenario clock, P0.09). Omitted from the JSON when no leg was
    /// timed. Attached with [`ScenarioRunRecord::with_timings`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timings: Option<crate::metrics::ScenarioRunTimings>,
}

impl ScenarioRunRecord {
    /// Attaches the fault journal (P5.21/P5.22; additive — the existing
    /// `record()` construction is unchanged).
    pub fn with_fault_journal(mut self, journal: crate::fault_injection::FaultJournal) -> Self {
        self.fault_journal = Some(journal);
        self
    }

    /// Attaches the per-leg timings (P5.23; additive).
    pub fn with_timings(mut self, timings: crate::metrics::ScenarioRunTimings) -> Self {
        self.timings = Some(timings);
        self
    }
}

// --- The controlled clock ------------------------------------------------------

/// The run's controlled clock: the ONLY clock the gates and the dispatch
/// adapter read (`LifecycleClock`), and the same instant the composition
/// advances the simulator pump to. Never a wall clock (P0.09 §5).
#[derive(Debug, Default)]
pub struct RunClock {
    now_unix_millis: Mutex<i64>,
}

impl RunClock {
    /// Creates the clock pinned at `now_unix_millis`.
    pub fn at(now_unix_millis: i64) -> Self {
        Self {
            now_unix_millis: Mutex::new(now_unix_millis),
        }
    }

    /// The current instant.
    pub fn now(&self) -> i64 {
        *self.now_unix_millis.lock().expect("run clock mutex")
    }

    /// Sets the current instant. Called ONLY by
    /// [`ScenarioRunHarness::advance_to`], which pumps the simulator to
    /// the same instant — the composition invariant.
    pub(crate) fn set(&self, now_unix_millis: i64) {
        *self.now_unix_millis.lock().expect("run clock mutex") = now_unix_millis;
    }
}

impl LifecycleClock for RunClock {
    fn now_unix_millis(&self) -> i64 {
        self.now()
    }
}

// --- The event encoder -----------------------------------------------------------

/// The run's event-payload encoder: JSON over the frozen event payload.
/// The outbox it feeds is a seam-level in-memory fake (see the adapter
/// inventory), so the encoding only has to be faithful enough for the run
/// record to name WHICH events the gates emitted.
struct JsonEventEncoder;

impl EventPayloadEncoder for JsonEventEncoder {
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
        serde_json::to_string(&envelope.event).map_err(|error| {
            ApplicationError::Infrastructure(InfrastructureError::new(
                "E_EVENT_ENCODE".to_string(),
                format!("the event payload could not be encoded: {error}"),
            ))
        })
    }
}

// --- The in-memory gate seams (SEAM-LEVEL composition support) --------------------
//
// These stand in for the PostgreSQL/broker-backed stores the composition
// root of a deployment wires. They honour the same rules the real
// adapters must: tenant scoping (a foreign tenant's record does not
// exist), the compare-and-swap transition rule, deny-by-default
// authorization, and fail-closed seam errors. They are NAMED as
// seam-level in every run record.

/// The in-memory command-record store seam.
pub(crate) struct ScenarioCommandStore {
    records: Mutex<HashMap<String, ProposedCommandRecord>>,
    journals: Mutex<HashMap<String, Vec<flexcore_domain::command_lifecycle::CommandTransition>>>,
    revisions: Mutex<HashMap<(String, u64), ScheduleRevisionUsability>>,
}

impl ScenarioCommandStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            journals: Mutex::new(HashMap::new()),
            revisions: Mutex::new(HashMap::new()),
        }
    }

    /// Seeds a schedule revision's usability (the schedule the commands
    /// are created FROM — the approved-schedule creation gate reads it).
    pub fn seed_revision(
        &self,
        schedule_id: &ScheduleId,
        revision: u64,
        state: ScheduleRevisionUsability,
    ) {
        self.revisions
            .lock()
            .expect("scenario command store")
            .insert((schedule_id.to_string(), revision), state);
    }

    /// The command's current state, derived from the journal.
    pub fn current_state(&self, command_id: CommandId) -> CommandState {
        self.journals
            .lock()
            .expect("scenario command store")
            .get(&command_id.to_string())
            .and_then(|journal| journal.last().map(|entry| entry.to))
            .unwrap_or(CommandState::Proposed)
    }
}

impl CommandGateStore for ScenarioCommandStore {
    fn insert_proposed(
        &self,
        ctx: &TenantContext,
        record: &ProposedCommandRecord,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        let mut records = self.records.lock().expect("scenario command store");
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
        transition: &flexcore_domain::command_lifecycle::CommandTransition,
    ) -> Result<StateChangeReceipt, CommandGateStoreError> {
        let mut journals = self.journals.lock().expect("scenario command store");
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
        let records = self.records.lock().expect("scenario command store");
        let Some(record) = records.get(&command_id.to_string()) else {
            return Ok(None);
        };
        if record.tenant_id != ctx.tenant_id() {
            // Existence never leaks across a tenant boundary (P0.16 rule 5).
            return Ok(None);
        }
        let journal = self
            .journals
            .lock()
            .expect("scenario command store")
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
            .expect("scenario command store")
            .get(&(schedule_id.to_string(), revision))
            .copied()
            .unwrap_or(ScheduleRevisionUsability::Unknown))
    }
}

/// The envelope-freshness seam: the scenario schedule's envelope is fresh
/// (the composition fixture is a pinned, pre-registered schedule;
/// staleness of a fixture would be a composition configuration error,
/// recorded at composition time, not a runtime seam failure).
struct ScenarioEnvelopeSource;

impl ScheduleEnvelopeSource for ScenarioEnvelopeSource {
    fn freshness(
        &self,
        _ctx: &TenantContext,
        _schedule: &ScheduleReference,
    ) -> Result<EnvelopeGateFreshness, GateSeamError> {
        Ok(EnvelopeGateFreshness::Fresh)
    }
}

/// The model-version seam: the grid model the scenario references is
/// Active with the hash the versioning layer assigned (the composition
/// registers the same model with the grid leg, so the gate's evidence
/// binding and this seam agree by construction).
struct ScenarioModelSource {
    content_hash: String,
}

impl ModelVersionSource for ScenarioModelSource {
    fn status(
        &self,
        _ctx: &TenantContext,
        _grid_model_ref: &str,
    ) -> Result<ModelVersionGateStatus, GateSeamError> {
        Ok(ModelVersionGateStatus::Active {
            content_hash: self.content_hash.clone(),
        })
    }
}

/// The asset-capability seam: every scenario asset is usable with the
/// declared quantities and ramps of the composition configuration (the
/// Phase 2 registry + P3.01/04 capability data in a deployment).
struct ScenarioAssetSource {
    quantities: DeclaredQuantities,
}

impl AssetCapabilitySource for ScenarioAssetSource {
    fn check(
        &self,
        _ctx: &TenantContext,
        _asset: AssetId,
    ) -> Result<AssetGateStatus, GateSeamError> {
        Ok(AssetGateStatus::Usable(self.quantities.clone()))
    }
}

/// The authorization evaluator seam: deny-by-default; only the recorded
/// approver role is allowed the approve action (the composition's
/// stand-in for the P4.13 policy set).
struct ScenarioAuthzEvaluator {
    approver_role: String,
}

impl CommandAuthorizationEvaluator for ScenarioAuthzEvaluator {
    fn evaluate(
        &self,
        _ctx: &TenantContext,
        request: &CommandAuthorizationRequest,
    ) -> AuthorizationDecision {
        if request.role == self.approver_role {
            AuthorizationDecision::Allowed {
                rule_reference: "policy:scenario:approver:command.approve".to_string(),
            }
        } else {
            AuthorizationDecision::Denied {
                reason: format!("no matching rule for role {}", request.role),
            }
        }
    }
}

// --- The real dispatch transport over the simulator (REAL) ------------------------

/// One decoded telemetry sample of the pumped history (the observation
/// serving form; the canonical record it came from is long gone by this
/// point — the sample carries the comparison-relevant fields only).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    asset_id: AssetId,
    event_ms: i64,
    quantity: TelemetryQuantity,
    value: f64,
    quality: TelemetryQuality,
}

struct TransportState {
    history: Vec<Sample>,
    /// The scenario-clock position the pump has reached (the latest
    /// ingested record's ingest_time), or `None` before the first frame.
    pumped_until_ms: Option<i64>,
    exhausted: bool,
    /// The last SERVED observation instant per asset (observation walking).
    last_served: HashMap<AssetId, i64>,
}

/// The REAL `SimulatorDispatchTransport` (P4.16) implemented over the
/// Phase 2 simulator crate — the composition-root implementation the
/// adapter-inventory record names as the real dispatch leg.
///
/// - **Deliver** consults the scenario's enabled fault plan (faults are
///   configuration, P2.15): a delivery to an asset inside an enabled
///   `drop` window fails with the timeout kind (no acceptance exists —
///   the frame would not exist either); an enabled `source_invalid`
///   window answers NACK; otherwise the simulator ACCEPTS at the
///   application layer with a synthesized acceptance reference
///   (`sim-accept:<command>/<seq>@<instant>`). Acceptance is
///   application-layer acceptance, never physical delivery (§14.1).
/// - **Observe** serves samples from the REAL pumped telemetry stream:
///   `pump_until` drives the source through the `TelemetrySource` port
///   (the bounded, pull-based emission rule — nothing is generated
///   unless pulled), and `observe` walks each asset's samples forward.
///   A drop window emits no frames, so absence is REAL, never
///   synthesized. GOOD and SUSPECT samples are served (D-P4.16-4:
///   `out_of_range` → an out-of-tolerance sample); BAD samples are
///   skipped as unusable ([`TRACKING_QUALITY_RULE`]).
pub struct SimulatorTelemetryTransport {
    source: Arc<SimulatorTelemetrySource>,
    subscription: TelemetrySubscription,
    /// The asset-key mapping (fault plan lookups speak keys).
    asset_keys: HashMap<AssetId, String>,
    /// The enabled fault plan (clone of the validated configuration).
    fault_plan: flexcore_simulator::scenario::FaultPlan,
    state: Mutex<TransportState>,
}

impl SimulatorTelemetryTransport {
    fn new(
        source: Arc<SimulatorTelemetrySource>,
        subscription: TelemetrySubscription,
        asset_keys: HashMap<AssetId, String>,
        fault_plan: flexcore_simulator::scenario::FaultPlan,
    ) -> Self {
        Self {
            source,
            subscription,
            asset_keys,
            fault_plan,
            state: Mutex::new(TransportState {
                history: Vec::new(),
                pumped_until_ms: None,
                exhausted: false,
                last_served: HashMap::new(),
            }),
        }
    }

    /// Drives the real simulator stream forward to `until_ms` (scenario
    /// clock) through the port: pull one frame, decode it, repeat — the
    /// bounded pull-based emission rule. Stops at exhaustion or when the
    /// stream's release position passes `until_ms`. The port's errors map
    /// to the transport's simulator-error kind (the adapter decides the
    /// lifecycle consequence; the transport never transitions state).
    pub(crate) fn pump_until(&self, until_ms: i64) -> Result<(), SimulatorTransportError> {
        let mut st = self.state.lock().expect("transport state");
        if st.exhausted {
            return Ok(());
        }
        loop {
            let frame = self
                .source
                .pull_frame(&self.subscription)
                .map_err(|error| SimulatorTransportError {
                    failure: SimulatorTransportFailure::SimulatorError,
                    detail: format!("the simulator source failed: {error}"),
                })?;
            let Some(bytes) = frame else {
                // Exhausted (horizon reached, paused, or stopped) — a
                // paused/stopped source emits nothing; the stream stays
                // where it is.
                st.exhausted = true;
                return Ok(());
            };
            let records =
                TelemetrySource::decode(self.source.as_ref(), &self.subscription, &bytes)
                    .map_err(|error| SimulatorTransportError {
                        failure: SimulatorTransportFailure::SimulatorError,
                        detail: format!("the simulator frame failed to decode: {error}"),
                    })?;
            let frame_release = records
                .iter()
                .map(|record| record.ingest_time().unix_millis())
                .max()
                .unwrap_or(0);
            for record in records {
                // Only value-bearing records become samples.
                let Some(value) = record.value() else {
                    continue;
                };
                let value = match value {
                    QuantityValue::Power(power) => power.value(),
                    QuantityValue::Fraction(fraction) => fraction.value(),
                    QuantityValue::Reactive { power, .. } => power.value(),
                    QuantityValue::Energy(energy) => energy.value(),
                    QuantityValue::Flag(flag) => {
                        if *flag {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    QuantityValue::State(_) => continue,
                };
                st.history.push(Sample {
                    asset_id: record.asset_id(),
                    event_ms: record.event_time().unix_millis(),
                    quantity: record.quantity(),
                    value,
                    quality: record.quality(),
                });
            }
            st.pumped_until_ms = Some(frame_release);
            if frame_release > until_ms {
                return Ok(());
            }
        }
    }

    /// The pump position (the delivery instant of the fault-window checks).
    pub(crate) fn pumped_until(&self) -> Option<i64> {
        self.state.lock().expect("transport state").pumped_until_ms
    }

    /// True when an enabled fault of the given kind covers `asset_key` at
    /// `at_ms` (faults are scenario configuration, P2.15 — the mapping
    /// reads the configuration, never synthesizes faults in code).
    fn fault_covers(&self, kind: FaultKind, asset_key: &str, at_ms: i64) -> bool {
        self.fault_plan.enabled
            && self
                .fault_plan
                .injections
                .iter()
                .any(|spec| {
                    spec.kind == kind
                        && spec.asset_key == asset_key
                        && spec.from_ms <= at_ms
                        && at_ms < spec.until_ms
                })
    }

    /// The latest SOC sample per asset before `at_ms` (the carried-forward
    /// realised SOC of the rolling roll; `None` for assets without a
    /// storage state).
    pub(crate) fn latest_soc_before(&self, asset_id: AssetId, at_ms: i64) -> Option<f64> {
        let st = self.state.lock().expect("transport state");
        st.history
            .iter()
            .filter(|sample| {
                sample.asset_id == asset_id
                    && sample.quantity == TelemetryQuantity::Soc
                    && sample.event_ms < at_ms
            })
            .max_by_key(|sample| sample.event_ms)
            .map(|sample| sample.value)
    }

    /// The average active power of `asset_id` over `[from_ms, until_ms)`
    /// (the realised value of one executed interval).
    pub(crate) fn average_power_between(
        &self,
        asset_id: AssetId,
        from_ms: i64,
        until_ms: i64,
    ) -> Option<f64> {
        let st = self.state.lock().expect("transport state");
        let mut sum = 0.0;
        let mut count = 0usize;
        for sample in &st.history {
            if sample.asset_id == asset_id
                && sample.quantity == TelemetryQuantity::ActivePower
                && sample.event_ms >= from_ms
                && sample.event_ms < until_ms
            {
                sum += sample.value;
                count += 1;
            }
        }
        if count == 0 {
            None
        } else {
            Some(sum / count as f64)
        }
    }

    /// Every availability-flag transition of each asset at or before
    /// `at_ms` (the availability-change visibility, P5.14 step 4). A drop
    /// window emits NO frames at all, so a dropout shows up here only if
    /// the fault plan carries it — the record also carries the drop-window
    /// entries from the plan (see `ScenarioRunHarness::record`).
    pub(crate) fn availability_changes(&self, at_ms: i64) -> Vec<(AssetId, i64, bool)> {
        let st = self.state.lock().expect("transport state");
        let mut changes = Vec::new();
        let mut last_per_asset: HashMap<AssetId, bool> = HashMap::new();
        for sample in &st.history {
            if sample.event_ms > at_ms || sample.quantity != TelemetryQuantity::Availability {
                continue;
            }
            let available = sample.value >= 0.5;
            match last_per_asset.get(&sample.asset_id) {
                Some(previous) if *previous == available => {}
                _ => {
                    changes.push((sample.asset_id, sample.event_ms, available));
                    last_per_asset.insert(sample.asset_id, available);
                }
            }
        }
        changes
    }

    /// The quality-state accounting of the pumped history (the same flags
    /// the console's P5.03 view carries).
    pub(crate) fn quality_counts(&self) -> BTreeMap<String, usize> {
        let st = self.state.lock().expect("transport state");
        let mut counts = BTreeMap::new();
        for sample in &st.history {
            *counts
                .entry(quality_name(sample.quality).to_string())
                .or_insert(0usize) += 1;
        }
        counts
    }

    /// The number of pumped samples.
    pub(crate) fn sample_count(&self) -> usize {
        self.state.lock().expect("transport state").history.len()
    }

    /// The latest telemetered active-power state of an asset at or before
    /// `at_ms`, WITH its quality flag (the real ramp-baseline seam input:
    /// a BAD/MISSING/STALE flag travels to the gate and is blocked by the
    /// D-P4.04-3 policy — real simulator faults produce real fail-closed
    /// outcomes, never coerced values).
    pub(crate) fn telemetered_state(
        &self,
        asset_id: AssetId,
        at_ms: i64,
    ) -> Option<TelemeteredState> {
        let st = self.state.lock().expect("transport state");
        st.history
            .iter()
            .filter(|sample| {
                sample.asset_id == asset_id
                    && sample.quantity == TelemetryQuantity::ActivePower
                    && sample.event_ms <= at_ms
            })
            .max_by_key(|sample| sample.event_ms)
            .map(|sample| TelemeteredState {
                active_power_mw: sample.value,
                quality: sample.quality,
                observed_at_millis: sample.event_ms,
            })
    }
}

impl SimulatorDispatchTransport for SimulatorTelemetryTransport {
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError> {
        let now = self.pumped_until().unwrap_or(0);
        let Some(asset_key) = self.asset_keys.get(&envelope.asset_id) else {
            return Err(SimulatorTransportError {
                failure: SimulatorTransportFailure::Unreachable,
                detail: format!(
                    "the envelope names asset {} which is not part of this simulator's asset set",
                    envelope.asset_id
                ),
            });
        };
        // D-P4.16-4 mapping over the scenario configuration: drop → no
        // acceptance (timeout), source_invalid → NACK, otherwise accepted.
        if self.fault_covers(FaultKind::Drop, asset_key, now) {
            return Err(SimulatorTransportError {
                failure: SimulatorTransportFailure::Timeout,
                detail: format!(
                    "the simulator did not accept: asset '{asset_key}' is inside an enabled \
                     drop window at the delivery instant (D-P4.16-4 mapping; no acceptance \
                     exists — the frame would not exist either)"
                ),
            });
        }
        if self.fault_covers(FaultKind::SourceInvalid, asset_key, now) {
            return Ok(SimulatedAcceptance::Nack {
                reason: "SOURCE_INVALID".to_string(),
            });
        }
        Ok(SimulatedAcceptance::Accepted {
            acceptance_reference: format!(
                "sim-accept:{}/{}@{}",
                envelope.command_id, envelope.sequence, now
            ),
        })
    }

    fn observe(
        &self,
        asset_id: AssetId,
        at_unix_millis: i64,
    ) -> Result<Option<SimulatedObservation>, SimulatorTransportError> {
        // The real stream is pumped to the observation instant first.
        self.pump_until(at_unix_millis)?;
        let mut st = self.state.lock().expect("transport state");
        let served_before = st.last_served.get(&asset_id).copied().unwrap_or(i64::MIN);
        // The next active-power sample at or before `at`, walking forward
        // past everything already served for this asset.
        let next = st
            .history
            .iter()
            .filter(|sample| {
                sample.asset_id == asset_id
                    && sample.quantity == TelemetryQuantity::ActivePower
                    && sample.event_ms <= at_unix_millis
                    && sample.event_ms > served_before
            })
            .min_by_key(|sample| sample.event_ms);
        let Some(sample) = next else {
            return Ok(None);
        };
        // The quality rule: GOOD and SUSPECT are usable (SUSPECT is the
        // out-of-range mapping's out-of-tolerance sample); BAD, MISSING,
        // and STALE are not — skipped, never coerced.
        if !matches!(sample.quality, TelemetryQuality::Good | TelemetryQuality::Suspect) {
            st.last_served.insert(asset_id, sample.event_ms);
            return Ok(None);
        }
        st.last_served.insert(asset_id, sample.event_ms);
        Ok(Some(SimulatedObservation {
            quantity: TrackedQuantity::ActivePowerMW,
            value: sample.value,
            observed_at_unix_millis: sample.event_ms,
        }))
    }
}

// --- The grid feasibility leg (P5.15 wiring) -------------------------------------

/// The grid backend the feasibility leg uses.
#[derive(Debug, Clone, PartialEq)]
pub enum GridBackendSelection {
    /// The in-process scripted backend (the P4.02 seam double): the
    /// pipeline, provider, scope, translation, and verdict are REAL; the
    /// power-flow outcome itself is the recorded fixture. SEAM-LEVEL.
    Scripted,
    /// The real P5.15 Python worker leg (`apps/worker/python`), executed
    /// as a subprocess through the `WorkerProcessBackend`. REAL — and
    /// UNVERIFIED until the human's build/test pass executes it.
    WorkerProcess {
        /// The interpreter program to spawn (e.g. `python`).
        program: String,
        /// The worker entrypoint script path
        /// (`apps/worker/python/grid_worker.py`).
        script_path: String,
        /// The bounded wait for the worker's answer.
        timeout: Duration,
    },
}

impl GridBackendSelection {
    /// The stable backend name the run record carries.
    pub fn name(&self) -> &'static str {
        match self {
            GridBackendSelection::Scripted => "scripted",
            GridBackendSelection::WorkerProcess { .. } => "worker_process",
        }
    }
}

/// The synthetic grid-model fixture a run composes (P5.18/P5.20 additive
/// extension). The Phase 4 synthetic models are radial — the D3
/// recommended default's topology class — and the adapter's DECLARED
/// topology scope is `TopologyScope::RadialAndMeshed` (balanced solver
/// covers both; P4.34 record). The meshed variant is a NAMED fixture under
/// that declared scope (never a silent scope use), balanced only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridFixtureKind {
    /// The three-bus radial fixture the P5.14 composition has always used
    /// (`synthetic-feeder-a`; the default — unchanged behaviour).
    ThreeBusRadial,
    /// The meshed two-feeder fixture (P5.20 medium case): two LV feeders
    /// fed through one substation with mesh ties closing the ring — the
    /// §19.3 "multiple feeders/sites connected through a common substation
    /// constraint" case at fixture scale, balanced, synthetic only.
    MeshedTwoFeeder,
}

impl GridFixtureKind {
    /// The stable name the scenario record carries.
    pub fn as_str(self) -> &'static str {
        match self {
            GridFixtureKind::ThreeBusRadial => "three_bus_radial",
            GridFixtureKind::MeshedTwoFeeder => "meshed_two_feeder",
        }
    }

    /// Parses the fixture name a scenario record pins. An unknown name is
    /// an error, never a silent default (P5.18: replay never substitutes
    /// an input).
    pub fn from_name(name: &str) -> Result<Self, ScenarioRunError> {
        match name {
            "three_bus_radial" => Ok(GridFixtureKind::ThreeBusRadial),
            "meshed_two_feeder" => Ok(GridFixtureKind::MeshedTwoFeeder),
            other => Err(ScenarioRunError::Scenario(format!(
                "unknown grid-model fixture '{other}' (the named fixtures are: \
                 three_bus_radial, meshed_two_feeder)"
            ))),
        }
    }
}

/// The in-process scripted backend used when the grid leg runs
/// seam-level. It produces a converged, row-complete outcome for the
/// three-bus fixture WITHOUT computing physics. NOT a power-flow result:
/// the run record names the backend `scripted` so no scripted value can
/// be mistaken for a computed one (and the P4.06 loading cross-check
/// documents the loading definition it carries).
struct ScriptedScenarioBackend {
    provider_version_reported: String,
}

impl GridModelBackend for ScriptedScenarioBackend {
    fn run_power_flow(
        &self,
        model: &VersionedNetworkModel,
        _injections: &[BackendInjection],
        _options: &BackendRunOptions,
    ) -> Result<BackendPowerFlowOutcome, flexcore_grid_api::backend::BackendError> {
        use flexcore_grid_api::backend::{BackendBranchResult, BackendBusResult, BackendTransformerResult};
        // One row per model element (the worker contract: a missing or
        // duplicate row is a contract failure at the adapter).
        let bus_results = model
            .buses
            .iter()
            .map(|bus| BackendBusResult {
                bus_id: bus.bus_id.clone(),
                vm_pu: if bus.is_slack { 1.02 } else { 1.0 },
            })
            .collect();
        let branch_results = model
            .branches
            .iter()
            .map(|branch| BackendBranchResult {
                branch_id: branch.branch_id.clone(),
                i_from_ka: 0.0,
                i_to_ka: 0.0,
                // 60 percent of rating: within the 0.9 limit fixture.
                loading_percent: 60.0,
            })
            .collect();
        let transformer_results = model
            .transformers
            .iter()
            .map(|transformer| BackendTransformerResult {
                transformer_id: transformer.transformer_id.clone(),
                loading_percent: 62.5,
                i_hv_ka: 0.0,
                i_lv_ka: 0.0,
            })
            .collect();
        Ok(BackendPowerFlowOutcome {
            converged: true,
            reported_error: None,
            provider_version_reported: Some(self.provider_version_reported.clone()),
            bus_results,
            branch_results,
            transformer_results,
            iterations: Some(4),
            worker_status_text: "scripted scenario backend: converged fixture (NOT a computed \
                                 power flow; the run record names this backend 'scripted')"
                .to_string(),
            warnings: Vec::new(),
        })
    }

    fn describe(&self) -> BackendDescription {
        BackendDescription {
            name: "scripted-scenario-backend".to_string(),
            version: "0".to_string(),
            notes: vec![
                "in-process scenario double (P5.14 seam-level leg); the outcome is a \
                 recorded fixture, never a computed power flow"
                    .to_string(),
            ],
        }
    }
}

/// The closed-loop observation a dispatch slice captures (P5.18,
/// additive): the pipeline outcome (steps 1–4) and the step-5 limit
/// report, so the scenario-level P4.37 consumption planning can route the
/// SAME evidence through the frozen D-P4.07-1 rule
/// (`route_pipeline_outcome`) WITHOUT re-running anything. Observation
/// only — the command gate's own verdict is unchanged.
pub struct ClosedLoopObservation {
    /// The pipeline outcome the slice produced (steps 1–4).
    pub pipeline_outcome: GatePipelineOutcome,
    /// The step-5 limit report, when the pipeline completed an attempt.
    pub limit_report: Option<LimitCheckReport>,
    /// The gate instant (the controlled scenario clock).
    pub now_millis: i64,
}

/// The grid feasibility runner seam: runs the REAL Phase 4 pipeline
/// (`run_feasibility_gate`, steps 1–4) against the REAL
/// `PandapowerGridProvider` over the selected backend, then the REAL
/// step-5 limit check, and converts the evidence-backed outcome into the
/// command gate's verdict. No pipeline logic is reimplemented here (the
/// P5.15 mitigation: "the scenario path calls the platform pipeline; a
/// code review checks for duplicate logic").
pub(crate) struct ScenarioGridFeasibilityRunner {
    /// The registered versioned model (its hash is what the evidence
    /// binds to).
    pub versioned_model: VersionedModel,
    /// The registry (staleness source changes; empty for a fresh fixture).
    pub registry: ModelVersionRegistry,
    /// The network limits the step-5 check applies.
    pub limits: NetworkLimits,
    /// The declared capability bounds per asset id text (the composition's
    /// stand-in for the P3.04 capability store).
    pub capability_bounds: BTreeMap<String, DeclaredBounds>,
    /// The asset→bus mapping the candidate setpoints carry.
    pub asset_buses: BTreeMap<String, String>,
    /// The setpoint active power per asset id text (the candidate).
    pub setpoints_mw: BTreeMap<String, f64>,
    /// The setpoint reactive power per asset id text.
    pub setpoints_mvar: BTreeMap<String, f64>,
    /// The declared reactive convention of the setpoints.
    pub reactive_convention: ReactiveSignConvention,
    /// The backend selection (scripted = seam-level; worker process = the
    /// P5.15 real leg).
    pub backend: GridBackendSelection,
    /// The run's controlled clock.
    pub clock: Arc<RunClock>,
    /// The REAL telemetry transport (the ramp-baseline seam reads the
    /// REAL simulator states, quality flags included).
    pub transport: Arc<SimulatorTelemetryTransport>,
    /// The last slice's closed-loop observation (P5.18 capture; taken by
    /// the harness after the slice — see
    /// [`ScenarioRunHarness::take_closed_loop_observation`]).
    closed_loop: Mutex<Option<ClosedLoopObservation>>,
}

impl ScenarioGridFeasibilityRunner {
    /// The content hash the versioning layer assigned (the evidence
    /// binding hash).
    pub fn model_hash(&self) -> String {
        self.versioned_model.model_ref().content_hash.clone()
    }

    /// The grid model reference text the commands record (§14.3 field 12).
    pub fn grid_model_reference(&self) -> String {
        let model_ref = self.versioned_model.model_ref();
        format!("grid-model:{}@{}", model_ref.model_id, model_ref.version)
    }
}

impl CommandFeasibilityRunner for ScenarioGridFeasibilityRunner {
    fn run(
        &self,
        ctx: &TenantContext,
        schedule_id: &ScheduleId,
        revision: u64,
        refs: &CommandFeasibilityRefs,
    ) -> Result<FeasibilityGateVerdict, GateSeamError> {
        let _ = ctx;
        let now = self.clock.now();
        let model_ref = self.versioned_model.model_ref().clone();

        // The provider: REAL scope gating + translation + result mapping +
        // completeness gate over the selected backend.
        let backend: Arc<dyn GridModelBackend> = match &self.backend {
            GridBackendSelection::Scripted => Arc::new(ScriptedScenarioBackend {
                provider_version_reported: flexcore_grid_api::adapter::PROVIDER_PINNED_VERSION
                    .to_string(),
            }),
            GridBackendSelection::WorkerProcess {
                program,
                script_path,
                timeout,
            } => Arc::new(flexcore_grid_api::worker_backend::WorkerProcessBackend::new(
                program.clone(),
                vec![script_path.clone()],
                *timeout,
            )),
        };
        let provider = PandapowerGridProvider::new(backend, balanced_worker_scope()).map_err(
            |error| GateSeamError {
                seam: "feasibility_runner",
                detail: format!("the grid provider could not be composed: {error}"),
            },
        )?;

        // The candidate: one setpoint per configured asset at the current
        // gate instant (the composition's stand-in for the §12.6 candidate
        // record; the §24 slice needs ONE validated candidate per run).
        let candidate = FeasibilityCandidate {
            candidate_id: refs.candidate_id.clone(),
            schedule_id: schedule_id.to_string(),
            revision,
            envelope_version: refs.envelope_version.clone(),
            result_record_snapshot_ref: refs.result_record_snapshot_ref.clone(),
            created_at_millis: now,
            valid_from_millis: now,
            valid_until_millis: now + 3_600_000,
            stated_duration_millis: 3_600_000,
            setpoints: self
                .setpoints_mw
                .iter()
                .map(|(asset_id, p_mw)| {
                    let bus_id = self
                        .asset_buses
                        .get(asset_id)
                        .cloned()
                        .unwrap_or_default();
                    CandidateSetpoint {
                        setpoint_id: format!("sp:{asset_id}"),
                        asset_id: asset_id.clone(),
                        bus_id,
                        active_power: ActivePowerMW::new(*p_mw).map_err(|error| GateSeamError {
                            seam: "feasibility_runner",
                            detail: format!("the candidate setpoint is not canonical: {error}"),
                        })?,
                        reactive_power: ReactivePowerMVAr::new(
                            self.setpoints_mvar.get(asset_id).copied().unwrap_or(0.0),
                        )
                        .map_err(|error| GateSeamError {
                            seam: "feasibility_runner",
                            detail: format!("the candidate setpoint is not canonical: {error}"),
                        })?,
                        reactive_sign_convention: self.reactive_convention,
                        interval_start_millis: now,
                    }
                })
                .collect(),
        };

        // The injected sources: capability bounds from the composition
        // configuration; envelope fresh; revision active; the ramp
        // baseline from the REAL telemetry (quality flag included — the
        // D-P4.04-3 policy blocks BAD/MISSING/STALE, so a real simulator
        // fault produces a real fail-closed outcome).
        let bounds = self.capability_bounds.clone();
        let transport = Arc::clone(&self.transport);
        let sources = GateSources {
            capability_bounds: Arc::new(move |asset_id: &str, _interval: i64| {
                bounds.get(asset_id).cloned().ok_or_else(|| SeamError {
                    seam: "capability_bounds",
                    detail: format!("no declared bounds for asset '{asset_id}'"),
                })
            }),
            envelope_freshness: Arc::new(|_envelope_version: &str| Ok(EnvelopeFreshness::Fresh)),
            revision_superseded: Arc::new(|_schedule_id: &str, _revision: u64| {
                Ok(RevisionState::Active)
            }),
            ramp_baseline: Arc::new(move |asset_id: &str| {
                // The asset id text is the canonical UUID of a mapped
                // scenario asset; find its AssetId.
                let parsed = FlexUuid::parse(asset_id).ok_or_else(|| SeamError {
                    seam: "ramp_baseline",
                    detail: format!("asset id '{asset_id}' is not a canonical UUID"),
                })?;
                let id = AssetId::new(parsed);
                transport
                    .telemetered_state(id, transport.pumped_until().unwrap_or(0))
                    .ok_or_else(|| SeamError {
                        seam: "ramp_baseline",
                        detail: format!(
                            "no telemetered active-power state exists for asset '{asset_id}' \
                             at the evaluation instant"
                        ),
                    })
            }),
        };

        // The portfolio registry elements the coverage check verifies: the
        // fixture covers the feeder and one element; the composition
        // declares exactly what the fixture covers.
        let portfolio_elements = vec![RegistryElement {
            element_id: "feeder-a".to_string(),
            kind: RegistryElementKind::Feeder,
        }];

        let request = GateRunRequest {
            candidate: &candidate,
            sources: &sources,
            config: &CandidateValidationConfig::default(),
            versioned_model: &self.versioned_model,
            registry: &self.registry,
            portfolio_elements: &portfolio_elements,
            health_config: &ModelHealthConfig::default(),
            provider: &provider,
            limits: &self.limits,
            attempt_id: refs.candidate_id.clone(),
            now_millis: now,
        };

        // Steps 1–4 (REAL pipeline). The outcome is evidence, never a
        // verdict.
        let outcome = run_feasibility_gate(request);

        // P5.18 (additive closed-loop capture): the step-5 limit report is
        // computed ONCE here and, together with the pipeline outcome,
        // captured for the scenario-level P4.37 consumption planning (the
        // frozen `route_pipeline_outcome` + `plan_consumption` chain). The
        // command-gate verdict below is UNCHANGED — the capture is
        // observation only, never a second verdict.
        let limit_report = match &outcome {
            GatePipelineOutcome::Completed { attempt, .. } => Some(check_limits(
                attempt.result(),
                &self.limits,
                &GeneratorLimitSet { limits: Vec::new() },
                &balanced_worker_scope(),
            )),
            _ => None,
        };
        *self.closed_loop.lock().expect("closed-loop capture") = Some(ClosedLoopObservation {
            pipeline_outcome: outcome.clone(),
            limit_report: limit_report.clone(),
            now_millis: now,
        });

        match outcome {
            GatePipelineOutcome::Completed { attempt, evidence } => {
                let _ = evidence;
                // Step 5 (REAL limit check), then the composition-root
                // verdict conversion. An empty generator-limit set: the
                // fixture declares no inverter S limits.
                let report = limit_report
                    .expect("a completed attempt always produced its limit report");
                let model_id = model_ref.model_id.clone();
                let model_hash = model_ref.content_hash.clone();
                let attempt_id = attempt.attempt_id.clone();
                Ok(match report.status {
                    LimitCheckStatus::WithinLimits => FeasibilityGateVerdict::Passed {
                        attempt_id,
                        model_id,
                        model_content_hash: model_hash,
                        margins: format!(
                            "worst binding margin fraction: {}",
                            report
                                .diagnostics
                                .binding_constraint
                                .as_ref()
                                .map(|binding| binding.margin_fraction.to_string())
                                .unwrap_or_else(|| "not recorded".to_string())
                        ),
                        coverage: "balanced AC power flow; unbalanced three-phase, protection, \
                                   dynamics, sub-cycle control, and AC-OPF are NOT modelled \
                                   (the declared scope's limitation record)"
                            .to_string(),
                    },
                    LimitCheckStatus::Violated => FeasibilityGateVerdict::LimitViolated {
                        attempt_id,
                        model_id,
                        model_content_hash: model_hash,
                        constraint: report.violated_constraint_names().join("; "),
                    },
                    LimitCheckStatus::Inconclusive { reason }
                    | LimitCheckStatus::FailClosed { reason, .. } => {
                        FeasibilityGateVerdict::FailClosed {
                            attempt_id,
                            model_id: Some(model_id),
                            model_content_hash: Some(model_hash),
                            reason,
                        }
                    }
                })
            }
            GatePipelineOutcome::Rejected { rejection, .. } => {
                Ok(rejection_to_verdict(&model_ref, &rejection))
            }
            GatePipelineOutcome::Aborted { abort, .. } => {
                Ok(abort_to_verdict(&model_ref, &abort))
            }
        }
    }
}

/// The composition-root conversion of a pipeline REJECTION into the
/// command gate's verdict (fail-closed — a rejected candidate is never a
/// pass).
fn rejection_to_verdict(
    model_ref: &GridModelRef,
    rejection: &GateRejection,
) -> FeasibilityGateVerdict {
    FeasibilityGateVerdict::FailClosed {
        attempt_id: String::new(),
        model_id: Some(model_ref.model_id.clone()),
        model_content_hash: Some(model_ref.content_hash.clone()),
        reason: format!(
            "the candidate was rejected at step '{}', rule '{}', subject '{}': {}",
            rejection.step.as_str(),
            rejection.rule_code,
            rejection.subject,
            rejection.detail
        ),
    }
}

/// The composition-root conversion of a pipeline ABORT into the command
/// gate's verdict (fail-closed; the P4.08 policy table owns the
/// reject-vs-hold classification).
fn abort_to_verdict(model_ref: &GridModelRef, abort: &GateAbort) -> FeasibilityGateVerdict {
    let reason = match abort {
        GateAbort::ModelNotHealthy { findings } => format!(
            "the model health check refused with {} finding(s): {}",
            findings.len(),
            findings
                .iter()
                .map(|finding| finding.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ),
        GateAbort::ModelHashMismatch { recorded, computed } => format!(
            "recorded content hash '{recorded}' but the live model content hashes to \
             '{computed}' — the model changed mid-gate; aborting fail-closed"
        ),
        GateAbort::ProviderError { detail } => format!(
            "the grid port returned an error (fail-closed; the P4.08 policy table \
             classifies provider failures as HOLD): {detail}"
        ),
    };
    FeasibilityGateVerdict::FailClosed {
        attempt_id: String::new(),
        model_id: Some(model_ref.model_id.clone()),
        model_content_hash: Some(model_ref.content_hash.clone()),
        reason,
    }
}

// --- The harness -----------------------------------------------------------------

/// The scheduling-grid configuration of a run (P3.03 defaults: 24 h,
/// 15-minute intervals for the Phase 3 scenarios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingGridConfig {
    /// The interval length in millis (e.g. 900_000 for 15 minutes).
    pub interval_millis: i64,
}

/// The prior-schedule fixture a dropout roll rolls from (the P3.22
/// revision identity the composition supplies; the prior PLAN's setpoints
/// are the composition's recorded fixture — the plan is superseded, never
/// rewritten).
#[derive(Debug, Clone, PartialEq)]
pub struct PriorScheduleFixture {
    /// The schedule-chain id (canonical UUID text; D-P3.22-1).
    pub schedule_id: String,
    /// The prior revision number (>= 1).
    pub revision: u64,
    /// The §12.6 candidate id (canonical UUID text).
    pub candidate_id: String,
    /// The prior plan's flat setpoint per asset (MW), recorded fixture
    /// (BTreeMap for deterministic interval rendering).
    pub setpoints_mw: BTreeMap<String, f64>,
}

/// The dispatch command configuration of a run (§24 step 6: ONE PROPOSED
/// command demonstrated through the gate chain to the simulator).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRunConfig {
    /// The target asset key (a scenario asset).
    pub asset_key: String,
    /// The target active power (MW, canonical sign: export positive).
    pub target_p_mw: f64,
    /// The target reactive power (MVAr) under the asset's site convention.
    pub target_q_mvar: f64,
    /// The tracking acceptance criterion (DEPLOYMENT CONFIGURATION,
    /// D-P4.16-3 — never hard-coded; the composition supplies it here).
    pub tracking_tolerance_mw: f64,
    pub tracking_min_satisfied_samples: u32,
    pub tracking_timeout_millis: i64,
    /// The authorizing approver identity + role (§17.3; the composition's
    /// simulated operator).
    pub approver: String,
    pub approver_role: String,
}

impl CommandRunConfig {
    /// The tracking criterion built from the configuration (validated by
    /// the adapter's constructor — a configuration error is a named error,
    /// never a default).
    fn criterion(&self) -> Result<TrackingAcceptanceCriterion, ScenarioRunError> {
        TrackingAcceptanceCriterion::new(
            TrackedQuantity::ActivePowerMW,
            self.tracking_tolerance_mw,
            self.tracking_min_satisfied_samples,
            self.tracking_timeout_millis,
        )
        .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))
    }
}

/// A scenario-run composition error. Every variant names what failed;
/// nothing degrades to a partial run presented as complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioRunError {
    /// The scenario file was rejected (all-or-nothing, P2.13).
    Scenario(String),
    /// A run configuration referenced an asset the scenario does not have.
    UnknownAsset(String),
    /// The dispatch leg failed (the adapter's named errors travel).
    Dispatch(String),
    /// The rolling-roll plan failed (the P3.24 named errors travel).
    Roll(String),
}

impl std::fmt::Display for ScenarioRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScenarioRunError::Scenario(detail) => write!(f, "scenario rejected: {detail}"),
            ScenarioRunError::UnknownAsset(asset) => {
                write!(f, "run configuration names '{asset}', which is not a scenario asset")
            }
            ScenarioRunError::Dispatch(detail) => write!(f, "dispatch leg failed: {detail}"),
            ScenarioRunError::Roll(detail) => write!(f, "rolling roll failed: {detail}"),
        }
    }
}

impl std::error::Error for ScenarioRunError {}

/// The composed scenario run: the REAL simulator source, the REAL
/// transport, the gate chain over the in-memory seams, the grid leg, and
/// the run clock. Drive it with [`advance_to`](Self::advance_to) and the
/// run steps; finish with [`record`](Self::record).
pub struct ScenarioRunHarness {
    scenario: Scenario,
    source: Arc<SimulatorTelemetrySource>,
    transport: Arc<SimulatorTelemetryTransport>,
    clock: Arc<RunClock>,
    store: Arc<ScenarioCommandStore>,
    grid_runner: Arc<ScenarioGridFeasibilityRunner>,
    asset_ids: BTreeMap<String, AssetId>,
    /// The run's fixed identity fixtures (tenant, portfolio, site).
    tenant_context: TenantContext,
    portfolio_id: PortfolioId,
    site_id: SiteId,
    /// The scenario's cadence in millis.
    cadence_millis: i64,
    /// The grid configuration of the run.
    grid_config: SchedulingGridConfig,
    /// The prior-schedule fixture (when a dropout roll is configured).
    prior_schedule: Option<PriorScheduleFixture>,
    /// The SEAM-LEVEL gate seams (the in-memory composition support named
    /// in the adapter inventory).
    seam_envelopes: ScenarioEnvelopeSource,
    seam_models: ScenarioModelSource,
    seam_assets: ScenarioAssetSource,
    seam_authz: ScenarioAuthzEvaluator,
    outbox: FakeEventOutbox,
    ids: CountingIdGenerator,
    encoder: JsonEventEncoder,
}

impl ScenarioRunHarness {
    /// Composes the run: loads the scenario (all-or-nothing), builds the
    /// REAL telemetry source over the deterministic stand-in identity
    /// (the same derivation the P2.24 replay path uses — NOT
    /// registry-issued IDs, recorded as stand-ins), subscribes the full
    /// quantity set, and assembles the gate chain + grid leg.
    ///
    /// The grid-model fixture is the three-bus radial fixture (the
    /// recorded Phase 4 default, unchanged). For a named fixture
    /// (P5.20's meshed two-feeder case) use
    /// [`ScenarioRunHarness::new_with_grid_fixture`].
    pub fn new(
        scenario_json: &str,
        grid_config: SchedulingGridConfig,
        grid_backend: GridBackendSelection,
        prior_schedule: Option<PriorScheduleFixture>,
    ) -> Result<Self, ScenarioRunError> {
        Self::new_with_grid_fixture(
            scenario_json,
            grid_config,
            grid_backend,
            prior_schedule,
            GridFixtureKind::ThreeBusRadial,
        )
    }

    /// The full composition with an explicitly named synthetic grid-model
    /// fixture (P5.18/P5.20 — the scenario record pins the fixture; an
    /// unknown name is a named error, never a silent default). All other
    /// behaviour is identical to [`ScenarioRunHarness::new`].
    pub fn new_with_grid_fixture(
        scenario_json: &str,
        grid_config: SchedulingGridConfig,
        grid_backend: GridBackendSelection,
        prior_schedule: Option<PriorScheduleFixture>,
        grid_fixture: GridFixtureKind,
    ) -> Result<Self, ScenarioRunError> {
        let scenario = load_scenario(scenario_json)
            .map_err(|error| ScenarioRunError::Scenario(error.to_string()))?;

        // The deterministic stand-in identity (replay.rs derivation): a
        // sim-runner run is not a registration — the mapping key → AssetId
        // exists only at registration (P2.25 step 2); the stand-in is
        // recorded as such.
        let mut asset_ids: BTreeMap<String, AssetId> = BTreeMap::new();
        let mut mapping: Vec<(String, AssetId)> = Vec::new();
        for site in &scenario.sites {
            for asset in &site.assets {
                let id = crate::replay::stand_in_asset_id(&asset.asset_key);
                mapping.push((asset.asset_key.clone(), id));
                asset_ids.insert(asset.asset_key.clone(), id);
            }
        }
        let asset_keys: HashMap<AssetId, String> = asset_ids
            .iter()
            .map(|(key, id)| (*id, key.clone()))
            .collect();

        let source = Arc::new(
            SimulatorTelemetrySource::from_scenario(scenario.clone(), mapping)
                .map_err(|error| ScenarioRunError::Scenario(error.to_string()))?,
        );
        source.start();

        // The tenant-scoped subscription over the full frozen quantity set
        // (the exit-gate coverage convention; the run is tenant-scoped by
        // construction, P0.16).
        let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(
                    FlexUuid::parse(crate::replay::REPLAY_TENANT_UUID)
                        .expect("the replay tenant UUID is a valid constant"),
                ),
                subject: "sim-runner:scenario-run".to_string(),
            },
        ));
        let all_quantities: Vec<TelemetryQuantity> = vec![
            TelemetryQuantity::ActivePower,
            TelemetryQuantity::ReactivePower,
            TelemetryQuantity::Soc,
            TelemetryQuantity::Energy,
            TelemetryQuantity::Availability,
            TelemetryQuantity::OperatingMode,
            TelemetryQuantity::Health,
            TelemetryQuantity::ConnectionState,
            TelemetryQuantity::BaselineReference,
        ];
        let horizon_end = scenario
            .start_ms
            .checked_add(
                i64::try_from(scenario.total_ticks)
                    .ok()
                    .and_then(|ticks| ticks.checked_mul(scenario.cadence_seconds.saturating_mul(1_000)))
                    .ok_or_else(|| {
                        ScenarioRunError::Scenario(
                            "the scenario horizon overflows the scenario clock".to_string(),
                        )
                    })?,
            )
            .ok_or_else(|| {
                ScenarioRunError::Scenario("the scenario horizon overflows the scenario clock".to_string())
            })?;
        let query = TelemetryQuery::new(
            &ctx,
            asset_ids.values().copied().collect(),
            EventTime::from_unix_millis(scenario.start_ms)
                .map_err(|error| ScenarioRunError::Scenario(error.to_string()))?,
            EventTime::from_unix_millis(horizon_end)
                .map_err(|error| ScenarioRunError::Scenario(error.to_string()))?,
            all_quantities,
        )
        .map_err(|error| ScenarioRunError::Scenario(error.to_string()))?;
        let subscription = TelemetrySource::subscribe(source.as_ref(), &query).map_err(|error| {
            ScenarioRunError::Scenario(format!("the run subscription was rejected: {error}"))
        })?;

        let transport = Arc::new(SimulatorTelemetryTransport::new(
            Arc::clone(&source),
            subscription,
            asset_keys,
            scenario.faults.clone(),
        ));

        let clock = Arc::new(RunClock::at(scenario.start_ms));

        // The grid leg: the NAMED synthetic fixture (the recorded
        // magnitudes the grid crate's contract tests use for the radial
        // fixture), registered with the versioning layer so the hash is
        // ASSIGNED, not assumed. Both fixtures are SYNTHETIC — no IEEE
        // feeder is used or claimed (§19.3 discipline; P5.20 owns the
        // per-case record).
        let (model_ref, coverage) = match grid_fixture {
            GridFixtureKind::ThreeBusRadial => (
                GridModelRef {
                    model_id: "feeder-a".to_string(),
                    version: "1".to_string(),
                    content_hash: String::new(),
                },
                ModelCoverage {
                    covered_feeder_ids: vec!["feeder-a".to_string()],
                    covered_element_ids: vec!["bus_3".to_string()],
                    coverage_statement: "covers feeder-a (three-bus radial fixture) and the \
                                         bus_3 asset element; the scenario's synthetic \
                                         feeder reference"
                        .to_string(),
                },
            ),
            GridFixtureKind::MeshedTwoFeeder => (
                GridModelRef {
                    model_id: "synthetic-meshed-two-feeder".to_string(),
                    version: "1".to_string(),
                    content_hash: String::new(),
                },
                ModelCoverage {
                    covered_feeder_ids: vec!["feeder-a".to_string(), "feeder-b".to_string()],
                    covered_element_ids: vec!["bus_3".to_string(), "bus_5".to_string()],
                    coverage_statement: "covers feeder-a and feeder-b (the meshed two-feeder \
                                         synthetic fixture, BALANCED only — the adapter's \
                                         declared TopologyScope::RadialAndMeshed, P4.34) and \
                                         the bus_3 / bus_5 asset elements; a named fixture \
                                         under the declared scope, never a silent scope use"
                        .to_string(),
                },
            ),
        };
        let model = match grid_fixture {
            GridFixtureKind::ThreeBusRadial => three_bus_fixture(model_ref),
            GridFixtureKind::MeshedTwoFeeder => meshed_two_feeder_fixture(model_ref),
        };
        let versioned_model = VersionedModel::register(
            model,
            ModelLifecycleState::Active,
            coverage,
            scenario.start_ms,
        )
        .map_err(|error| {
            ScenarioRunError::Scenario(format!("the grid model fixture was rejected: {error}"))
        })?;
        let mut registry = ModelVersionRegistry::new();
        registry
            .register_version(versioned_model.clone())
            .map_err(|error| {
                ScenarioRunError::Scenario(format!(
                    "the grid model registry refused the fixture: {error}"
                ))
            })?;

        // The capability bounds per scenario asset (the composition's
        // stand-in for the P3.04 capability store; bounded by the rated
        // magnitudes the scenario declares). The candidate setpoint is
        // 0 MW (no injection) — the slice candidate is validated, not
        // optimized (the optimizer is out of this composition's scope).
        let mut capability_bounds: BTreeMap<String, DeclaredBounds> = BTreeMap::new();
        let mut asset_buses: BTreeMap<String, String> = BTreeMap::new();
        let mut setpoints_mw: BTreeMap<String, f64> = BTreeMap::new();
        for (site_index, site) in scenario.sites.iter().enumerate() {
            for asset in &site.assets {
                let id = asset_ids[&asset.asset_key];
                let id_text = id.uuid().to_hyphenated();
                capability_bounds.insert(
                    id_text.clone(),
                    DeclaredBounds {
                        p_min_mw: -asset.rated_power_mw,
                        p_max_mw: asset.rated_power_mw,
                        q_min_mvar: -0.5 * asset.rated_power_mw,
                        q_max_mvar: 0.5 * asset.rated_power_mw,
                        q_convention: site.reactive_convention,
                        ramp_mw_per_min: asset.rated_power_mw.max(0.1),
                    },
                );
                // The fixture's bus mapping: on the radial fixture every
                // asset applies at bus_3 (the LV bus it models); on the
                // meshed two-feeder fixture the FIRST site's assets apply
                // at bus_3 (feeder-a) and the SECOND site's at bus_5
                // (feeder-b) — the common-substation constraint case at
                // fixture scale. Deterministic per site order.
                let bus_id = match (grid_fixture, site_index) {
                    (GridFixtureKind::MeshedTwoFeeder, 1) => "bus_5".to_string(),
                    _ => "bus_3".to_string(),
                };
                asset_buses.insert(id_text.clone(), bus_id);
                setpoints_mw.insert(id_text, 0.0);
            }
        }

        let grid_runner = Arc::new(ScenarioGridFeasibilityRunner {
            versioned_model,
            registry,
            limits: NetworkLimits {
                voltage_min_pu: 0.9,
                voltage_max_pu: 1.1,
                max_branch_loading_fraction: 0.9,
                max_transformer_loading_fraction: 0.9,
            },
            capability_bounds,
            asset_buses,
            setpoints_mw,
            setpoints_mvar: BTreeMap::new(),
            reactive_convention: ReactiveSignConvention::InjectionPositive,
            backend: grid_backend,
            clock: Arc::clone(&clock),
            transport: Arc::clone(&transport),
            closed_loop: Mutex::new(None),
        });

        // The model-version seam agrees with the grid leg by construction:
        // the same registered hash the evidence binds to.
        let seam_models = ScenarioModelSource {
            content_hash: grid_runner.model_hash(),
        };

        Ok(Self {
            cadence_millis: scenario.cadence_seconds.saturating_mul(1_000),
            scenario,
            source,
            transport,
            clock,
            store: Arc::new(ScenarioCommandStore::new()),
            grid_runner,
            asset_ids,
            tenant_context: ctx,
            portfolio_id: PortfolioId::new(deterministic_uuid("flexcore-sim-runner/portfolio")),
            site_id: SiteId::new(deterministic_uuid("flexcore-sim-runner/site-north")),
            grid_config,
            prior_schedule,
            seam_envelopes: ScenarioEnvelopeSource,
            seam_models,
            seam_assets: ScenarioAssetSource {
                quantities: DeclaredQuantities {
                    active_power: true,
                    reactive_power: true,
                    soc: true,
                    max_up_ramp_mw_per_s: 2.0,
                    max_down_ramp_mw_per_s: 2.0,
                },
            },
            seam_authz: ScenarioAuthzEvaluator {
                approver_role: "scenario-approver".to_string(),
            },
            outbox: FakeEventOutbox::new(FakeWriteJournal::new()),
            ids: CountingIdGenerator::new(),
            encoder: JsonEventEncoder,
        })
    }

    /// The scenario identity.
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// The run clock.
    pub fn clock(&self) -> Arc<RunClock> {
        Arc::clone(&self.clock)
    }

    /// The REAL telemetry transport (the observation/inspection surface).
    pub fn transport(&self) -> Arc<SimulatorTelemetryTransport> {
        Arc::clone(&self.transport)
    }

    /// The grid feasibility runner (the P5.15 wiring surface).
    pub fn grid_runner(&self) -> Arc<ScenarioGridFeasibilityRunner> {
        Arc::clone(&self.grid_runner)
    }

    /// Takes the closed-loop observation the LAST dispatch slice captured
    /// (P5.18: the pipeline outcome + the step-5 limit report, for the
    /// scenario-level P4.37 consumption planning). Taking clears the
    /// capture, so a second slice always plans from ITS OWN evidence.
    /// `None` when no slice reached the feasibility pipeline.
    pub fn take_closed_loop_observation(&self) -> Option<ClosedLoopObservation> {
        self.grid_runner
            .closed_loop
            .lock()
            .expect("closed-loop capture")
            .take()
    }

    /// The command store (test/composition inspection).
    pub fn command_store(&self) -> Arc<ScenarioCommandStore> {
        Arc::clone(&self.store)
    }

    /// The scenario asset id for a key.
    pub fn asset_id(&self, asset_key: &str) -> Result<AssetId, ScenarioRunError> {
        self.asset_ids
            .get(asset_key)
            .copied()
            .ok_or_else(|| ScenarioRunError::UnknownAsset(asset_key.to_string()))
    }

    /// Advances the controlled clock to `now_unix_millis` AND pumps the
    /// real simulator stream to the same instant (the composition
    /// invariant: the adapter's `now` and the telemetry position never
    /// diverge).
    pub fn advance_to(&self, now_unix_millis: i64) -> Result<(), ScenarioRunError> {
        self.clock.set(now_unix_millis);
        self.transport
            .pump_until(now_unix_millis)
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))
    }

    /// Drives the whole horizon: advances the clock tick by tick (the
    /// bounded pull-based emission rule honoured), so the run's telemetry
    /// history is complete before the record is built. Starts from the
    /// CURRENT clock position (the composition invariant — the clock and
    /// the pump position never move backward), so it composes cleanly
    /// after a dispatch slice. Returns the final scenario-clock instant.
    pub fn run_to_horizon_end(&self) -> Result<i64, ScenarioRunError> {
        let end = self.horizon_end_millis()?;
        let mut now = self.clock.now().max(self.scenario.start_ms);
        while now < end {
            now += self.cadence_millis;
            self.advance_to(now.min(end))?;
        }
        Ok(now)
    }

    fn horizon_end_millis(&self) -> Result<i64, ScenarioRunError> {
        self.scenario
            .start_ms
            .checked_add(
                i64::try_from(self.scenario.total_ticks)
                    .ok()
                    .and_then(|ticks| {
                        ticks.checked_mul(self.scenario.cadence_seconds.saturating_mul(1_000))
                    })
                    .ok_or_else(|| {
                        ScenarioRunError::Scenario(
                            "the scenario horizon overflows the scenario clock".to_string(),
                        )
                    })?,
            )
            .ok_or_else(|| {
                ScenarioRunError::Scenario("the scenario horizon overflows the scenario clock".to_string())
            })
    }

    /// The composed gate chain over the in-memory seams (SEAM-LEVEL
    /// stores; REAL gates). The borrows all name harness fields, so the
    /// `CommandGates` view lives as long as `&self`.
    fn gates(&self) -> CommandGates<'_> {
        CommandGates {
            store: self.store.as_ref(),
            envelopes: &self.seam_envelopes,
            models: &self.seam_models,
            assets: &self.seam_assets,
            feasibility: self.grid_runner.as_ref(),
            authz: &self.seam_authz,
            outbox: &self.outbox,
            encoder: &self.encoder,
            ids: &self.ids,
            clock: self.clock.as_ref(),
            policy: CommandGatePolicy::default(),
        }
    }
}

/// One dispatch slice's outcome: the command summary AND the grid
/// feasibility summary the slice actually produced (P5.15: the run record
/// names the backend that ran and the verdict the REAL pipeline returned —
/// `not_run` when the slice ended before the feasibility gate was reached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchSliceOutcome {
    /// The command's lifecycle/issue/tracking summary.
    pub command: CommandRunSummary,
    /// The grid feasibility summary of the slice (verdict from the REAL
    /// Phase 4 pipeline; `not_run` when the slice never reached it).
    pub grid_check: GridCheckSummary,
}

impl ScenarioRunHarness {
    /// Runs the §24 steps 5–8 dispatch slice for one command: creates the
    /// PROPOSED command FROM the approved schedule, walks the REAL gate
    /// chain (validation → feasibility → authorization), issues through
    /// the REAL adapter to the REAL simulator transport, then polls
    /// tracking to a terminal state at the configured instants. Returns
    /// both summaries for the run record.
    pub fn run_dispatch_slice(
        &self,
        command_config: &CommandRunConfig,
        command_id: CommandId,
        schedule_id: &ScheduleId,
        schedule_revision: u64,
        issue_at_millis: i64,
        poll_at_millis: i64,
    ) -> Result<DispatchSliceOutcome, ScenarioRunError> {
        let asset_id = self.asset_id(&command_config.asset_key)?;
        // The gate chain runs AT THE ISSUE INSTANT: the controlled clock
        // and the REAL simulator stream advance together first, so the
        // feasibility ramp-baseline seam reads telemetered states that
        // actually exist (and are fresh AT the gate instant) — running the
        // gates at the start instant with an unpumped stream would be a
        // composition bug (a guaranteed no-baseline seam error), not an
        // honest gate outcome.
        self.advance_to(issue_at_millis)?;
        let gates = self.gates();

        // The approved schedule the creation gate requires (P4.11 step 1).
        self.store.seed_revision(
            schedule_id,
            schedule_revision,
            ScheduleRevisionUsability::Active,
        );

        let site_convention = self
            .scenario
            .sites
            .iter()
            .find(|site| {
                site.assets
                    .iter()
                    .any(|a| a.asset_key == command_config.asset_key)
            })
            .map(|site| site.reactive_convention)
            .unwrap_or(ReactiveSignConvention::InjectionPositive);
        let horizon_end = self.horizon_end_millis()?;

        let propose_input = ProposeCommandInput {
            command_id,
            scope: CommandScope {
                tenant_id: self.tenant_context.tenant_id(),
                portfolio_id: self.portfolio_id,
                site_id: self.site_id,
                asset_id,
            },
            source: SourceOrigin {
                subsystem: "sim-runner:scenario-run".to_string(),
                component_version: format!("flexcore-sim-runner/{}", env!("CARGO_PKG_VERSION")),
            },
            schedule: ScheduleReference {
                schedule_id: *schedule_id,
                revision: schedule_revision,
            },
            intent: CommandIntent {
                reason: "P5.14 scenario-run dispatch slice (§24 steps 6–8)".to_string(),
                action: "set-asset-power".to_string(),
                actor_or_service: "sim-runner:scenario-run".to_string(),
            },
            target: ControlTarget::PowerSetpoint {
                p: ActivePowerMW::new(command_config.target_p_mw).map_err(|error| {
                    ScenarioRunError::Dispatch(format!("the target P is not canonical: {error}"))
                })?,
                q: Some(ReactivePowerMVAr::new(command_config.target_q_mvar).map_err(|error| {
                    ScenarioRunError::Dispatch(format!("the target Q is not canonical: {error}"))
                })?),
                conventions: DeclaredSignConventions {
                    active: ActivePowerSignConvention::ExportPositive,
                    reactive: Some(site_convention),
                },
            },
            validity: ValidityWindow::new(
                ValidFrom::from_unix_millis(self.scenario.start_ms)
                    .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?,
                ValidUntil::from_unix_millis(horizon_end)
                    .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?,
            )
            .map_err(|error| {
                ScenarioRunError::Dispatch(format!("the validity window is invalid: {error}"))
            })?,
            execution: ExecutionRequirements {
                duration: Duration::from_secs(60),
                ramp: RampLimits {
                    max_up_mw_per_s: 2.0,
                    max_down_mw_per_s: 2.0,
                },
            },
            sequence: 1,
            idempotency_key: format!("idem:scenario-run/{command_id}"),
            evidence: EvidenceReferences {
                grid_model: self.grid_runner.grid_model_reference(),
                telemetry_snapshot: format!("scenario-clock:{}", self.scenario.start_ms),
                validation_evidence: "pending".to_string(),
            },
            authenticity: CommandAuthenticity::AuthenticatedTransport(format!(
                "{SIMULATOR_TRANSPORT_IDENTITY_PREFIX}/scenario-run"
            )),
            supersedes: None,
            command_payload_json: format!(
                "{{\"p_mw\":{},\"q_mvar\":{}}}",
                command_config.target_p_mw, command_config.target_q_mvar
            ),
        };

        // §24 step 6: create PROPOSED, then the REAL gate chain.
        let proposed = gates
            .create_proposed_command(&self.tenant_context, propose_input)
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
        let validation = gates
            .run_validation_gate(
                &self.tenant_context,
                &proposed.command,
                &actor("sim-runner:validator"),
            )
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
        if !validation.passed {
            return Ok(DispatchSliceOutcome {
                command: CommandRunSummary {
                    command_id: command_id.to_string(),
                    asset_key: command_config.asset_key.clone(),
                    final_state: self.store.current_state(command_id).name().to_string(),
                    issue_outcome: format!(
                        "rejected:{}",
                        validation.failing_check.unwrap_or_default()
                    ),
                    acceptance_reference: None,
                    tracking_verdict: None,
                },
                // The slice ended before the feasibility gate: the grid
                // summary honestly says NOT RUN (never a fabricated verdict).
                grid_check: self.grid_check_summary("not_run", validation.failing_check.clone()),
            });
        }
        let feasibility_refs = CommandFeasibilityRefs {
            candidate_id: deterministic_uuid(&format!(
                "flexcore-sim-runner/candidate/{command_id}"
            ))
            .to_hyphenated(),
            envelope_version: format!(
                "{}:{}",
                self.scenario.meta.scenario_id, self.scenario.meta.version
            ),
            result_record_snapshot_ref: format!(
                "schedule_input_snapshot:{}:{}",
                self.scenario.meta.scenario_id, self.scenario.meta.version
            ),
        };
        let feasibility = gates
            .run_feasibility_gate(
                &self.tenant_context,
                &proposed.command,
                &feasibility_refs,
                &actor("sim-runner:grid-gate"),
            )
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
        if !feasibility.result.passed {
            return Ok(DispatchSliceOutcome {
                command: CommandRunSummary {
                    command_id: command_id.to_string(),
                    asset_key: command_config.asset_key.clone(),
                    final_state: self.store.current_state(command_id).name().to_string(),
                    issue_outcome: format!(
                        "rejected:{}",
                        feasibility
                            .result
                            .failing_check
                            .unwrap_or_else(|| "network_gate".to_string())
                    ),
                    acceptance_reference: None,
                    tracking_verdict: None,
                },
                // The REAL pipeline refused the candidate: the summary is
                // fail-closed, never a pass (P4.08).
                grid_check: self.grid_check_summary(
                    "fail_closed",
                    feasibility.result.failing_check.clone(),
                ),
            });
        }
        // The feasibility attempt identity + verdict the REAL gate produced
        // (the run-record evidence; on a pass the attempt id comes from the
        // feasibility evidence reference).
        let grid_check = self.grid_check_summary(
            "passed",
            None,
        )
        .with_attempt_id(
            feasibility
                .feasibility_evidence
                .as_ref()
                .map(|evidence| evidence.attempt_id.clone())
                .unwrap_or_default(),
        );

        // §24 step 6: the REQUIRED authorization (P4.13 — no auto-approve).
        let request = CommandAuthorizationRequest {
            subject: command_config.approver.clone(),
            role: command_config.approver_role.clone(),
            action: CommandAuthzAction::Approve,
            scope: ApprovalScope::Asset(asset_id),
        };
        let approval = gates
            .record_command_approval(
                &self.tenant_context,
                &proposed.command,
                &request,
                "limits:simulated scenario run (P5.14); within the declared capability bounds",
                ValidUntil::from_unix_millis(horizon_end)
                    .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?,
            )
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
        if !approval.approved {
            return Ok(DispatchSliceOutcome {
                command: CommandRunSummary {
                    command_id: command_id.to_string(),
                    asset_key: command_config.asset_key.clone(),
                    final_state: self.store.current_state(command_id).name().to_string(),
                    issue_outcome: format!("denied:{}", approval.denial_reason.unwrap_or_default()),
                    acceptance_reference: None,
                    tracking_verdict: None,
                },
                // Feasibility PASSED before the denial: the grid summary
                // keeps that verdict with its attempt id.
                grid_check,
            });
        }
        let authorization_record = approval
            .decision_record
            .expect("an approval carries its decision record");

        // §24 steps 7–8: issue to the SIMULATOR through the REAL adapter
        // over the REAL transport, then track to a terminal state. (The
        // clock is already AT the issue instant — advanced before the gate
        // chain — so the adapter's deliver checks see the delivery instant
        // the fault plan intends.)
        let adapter = SimulatorDispatchAdapter::configure(
            &gates,
            AdapterTargetCandidate::Simulator {
                simulator_id: format!("sim://{}", self.scenario.meta.scenario_id),
            },
            self.transport.as_ref(),
            SimulationKeyring::new("scenario-run-sim-key")
                .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?,
            command_config.criterion()?,
        )
        .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
        let issue = adapter
            .issue(
                &self.tenant_context,
                &proposed.command,
                &authorization_record,
                &actor("sim-runner:dispatcher"),
            )
            .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;

        let mut issue_outcome = String::new();
        let mut acceptance_reference = None;
        if issue.acknowledged.is_some() {
            issue_outcome = "accepted".to_string();
            acceptance_reference = issue.acknowledgement.acceptance_reference.clone();
        } else if issue.failed.is_some() {
            issue_outcome = issue.acknowledgement.state.as_str().to_string();
        }

        // The tracking poll (one poll at the configured instant; the
        // criterion's evaluation order — deviation → FAILED, satisfied →
        // COMPLETED, timeout → FAILED — is the adapter's, not ours).
        let mut tracking_verdict = None;
        if issue.acknowledged.is_some() {
            self.advance_to(poll_at_millis)?;
            let poll = adapter
                .poll_tracking(
                    &self.tenant_context,
                    &proposed.command,
                    &actor("sim-runner:dispatcher"),
                )
                .map_err(|error| ScenarioRunError::Dispatch(error.to_string()))?;
            if let Some(verdict) = poll.verdict {
                tracking_verdict = Some(
                    match verdict {
                        TrackingVerdict::Completed { .. } => "completed",
                        TrackingVerdict::Deviation { .. } => "deviation",
                        TrackingVerdict::Timeout => "timeout",
                        TrackingVerdict::KeepTracking { .. } => "keep_tracking",
                    }
                    .to_string(),
                );
            }
            if let Some(terminal) = &poll.terminal {
                tracking_verdict = Some(
                    match terminal {
                        TrackingTerminal::Completed { .. } => "completed",
                        TrackingTerminal::Failed { .. } => "failed",
                        TrackingTerminal::Expired { .. } => "expired",
                    }
                    .to_string(),
                );
            }
        }

        Ok(DispatchSliceOutcome {
            command: CommandRunSummary {
                command_id: command_id.to_string(),
                asset_key: command_config.asset_key.clone(),
                final_state: self.store.current_state(command_id).name().to_string(),
                issue_outcome,
                acceptance_reference,
                tracking_verdict,
            },
            grid_check,
        })
    }

    /// The grid summary header every slice outcome carries: the backend
    /// that ran and the provider id + version (P5.15: the run record names
    /// which grid backend produced the verdict).
    fn grid_check_summary(&self, verdict: &str, reason: Option<String>) -> GridCheckSummary {
        GridCheckSummary {
            attempt_id: String::new(),
            backend: self.grid_runner.backend.name().to_string(),
            provider_id_version: format!("{PROVIDER_ID}@{PROVIDER_PINNED_VERSION}"),
            verdict: verdict.to_string(),
            reason,
        }
    }

    /// Plans the P3.24 rolling roll for the recorded dropouts, using the
    /// REAL telemetry as the realised state (the frozen past is the
    /// simulator's own record). Requires a [`PriorScheduleFixture`].
    pub fn plan_dropout_roll(
        &self,
        dropouts: &[RecordedDropout],
    ) -> Result<Option<RollPlanSummary>, ScenarioRunError> {
        let Some(prior_fixture) = &self.prior_schedule else {
            return Ok(None);
        };
        if dropouts.is_empty() {
            return Ok(None);
        }
        let first_dropout = dropouts
            .iter()
            .map(|dropout| dropout.from_millis)
            .min()
            .expect("checked non-empty");
        let horizon_end = self.horizon_end_millis()?;
        let interval_millis = self.grid_config.interval_millis;

        // The prior revision: the plan's intervals are grid-aligned across
        // the horizon with the fixture's flat setpoints (the plan is
        // superseded, never rewritten; P3.22/P3.24). Deterministic order
        // via the BTreeMap.
        let mut intervals = Vec::new();
        let mut interval_start = self.scenario.start_ms;
        while interval_start < horizon_end {
            intervals.push(flexcore_application::rolling_horizon::IntervalPlan {
                interval_start_millis: interval_start,
                interval_end_millis: interval_start + interval_millis,
                setpoints: prior_fixture
                    .setpoints_mw
                    .iter()
                    .map(|(asset_key, p_mw)| flexcore_application::rolling_horizon::AssetSetpoint {
                        asset_key: asset_key.clone(),
                        p_mw: *p_mw,
                    })
                    .collect(),
            });
            interval_start += interval_millis;
        }
        let prior = PriorRevision {
            schedule_id: prior_fixture.schedule_id.clone(),
            revision: prior_fixture.revision,
            candidate_id: prior_fixture.candidate_id.clone(),
            horizon_start_millis: self.scenario.start_ms,
            horizon_end_millis: horizon_end,
            interval_millis,
            asset_keys: self.asset_ids.keys().cloned().collect(),
            intervals,
        };

        // The realised state: executed_until at the first dropout instant;
        // carried SOC from the REAL telemetry; realised values per executed
        // interval from the REAL telemetry (the frozen past is the
        // simulator's own record — never re-derived, P3.24 step 2).
        let mut carried_assets = Vec::new();
        for (asset_key, asset_id) in &self.asset_ids {
            carried_assets.push(CarriedAssetState {
                asset_key: asset_key.clone(),
                realised_soc: self.transport.latest_soc_before(*asset_id, first_dropout),
            });
        }
        let mut realised_intervals = Vec::new();
        let mut interval_start = self.scenario.start_ms;
        while interval_start < first_dropout {
            let mut values = Vec::new();
            for (asset_key, asset_id) in &self.asset_ids {
                if let Some(p_mw) = self
                    .transport
                    .average_power_between(*asset_id, interval_start, interval_start + interval_millis)
                {
                    values.push(RealisedAssetValue {
                        asset_key: asset_key.clone(),
                        realised_p_mw: p_mw,
                    });
                }
            }
            realised_intervals.push(RealisedInterval {
                interval_start_millis: interval_start,
                interval_end_millis: interval_start + interval_millis,
                values,
            });
            interval_start += interval_millis;
        }
        let realised = RealisedState {
            executed_until_millis: first_dropout,
            carried_assets,
            realised_intervals,
            dropouts: dropouts
                .iter()
                .map(|dropout| AssetDropout {
                    asset_key: dropout.asset_key.clone(),
                    from_millis: dropout.from_millis,
                })
                .collect(),
        };

        let request = RollRequest {
            trigger: RollTrigger::AssetDropout,
            trigger_detail: format!(
                "simulator telemetry drop observed for '{}' from the scenario clock instant \
                 {first_dropout} (P5.14 step 4: the dropout is visible and triggers the \
                 platform's P3.24 roll path)",
                dropouts
                    .iter()
                    .map(|dropout| dropout.asset_key.as_str())
                    .collect::<Vec<_>>()
                    .join("', '")
            ),
            realised,
        };
        let plan = plan_rolling_horizon_roll(&prior, &request)
            .map_err(|error| ScenarioRunError::Roll(error.to_string()))?;
        Ok(Some(RollPlanSummary {
            trigger: plan.trigger.as_str().to_string(),
            trigger_detail: plan.trigger_detail.clone(),
            superseded: format!(
                "{}@{}",
                plan.superseded.schedule_id, plan.superseded.revision
            ),
            new_revision: format!(
                "{}@{}",
                plan.new_revision.schedule_id, plan.new_revision.revision
            ),
            reoptimize_start_millis: plan.reoptimize_start_millis,
            reoptimize_end_millis: plan.reoptimize_end_millis,
            active_asset_keys: plan.active_asset_keys.clone(),
            notes: plan.notes.clone(),
        }))
    }

    /// Builds the run record (the P5.14 deliverable: the run names its
    /// versions, its adapter inventory, its quality accounting, its
    /// availability changes, its dropouts/roll, and its dispatch and grid
    /// outcomes).
    pub fn record(
        &self,
        code_version: &str,
        commands: Vec<CommandRunSummary>,
        grid_checks: Vec<GridCheckSummary>,
        dropouts: Vec<RecordedDropout>,
    ) -> Result<ScenarioRunRecord, ScenarioRunError> {
        let roll_plan = self.plan_dropout_roll(&dropouts)?;
        let mut asset_models = Vec::new();
        for site in &self.scenario.sites {
            for asset in &site.assets {
                asset_models.push(AssetModelVersion {
                    asset_key: asset.asset_key.clone(),
                    asset_type: format!("{:?}", asset.asset_type),
                    rated_power_mw: asset.rated_power_mw,
                    rated_energy_mwh: asset.rated_energy_mwh,
                });
            }
        }
        let mut availability_changes = Vec::new();
        for (asset_id, at, available) in self.transport.availability_changes(self.clock.now()) {
            availability_changes.push(AvailabilityChange {
                asset_key: self
                    .asset_ids
                    .iter()
                    .find(|(_, id)| **id == asset_id)
                    .map(|(key, _)| key.clone())
                    .unwrap_or_default(),
                at_unix_millis: at,
                available,
                evidence: "telemetry_availability_flag".to_string(),
            });
        }
        for dropout in &dropouts {
            availability_changes.push(AvailabilityChange {
                asset_key: dropout.asset_key.clone(),
                at_unix_millis: dropout.from_millis,
                available: false,
                evidence: "fault_plan_drop_window".to_string(),
            });
        }

        Ok(ScenarioRunRecord {
            scenario_id: self.scenario.meta.scenario_id.clone(),
            scenario_version: self.scenario.meta.version.clone(),
            master_seed: self.scenario.master_seed,
            code_version: code_version.to_string(),
            simulator_crate_version: SIMULATOR_CRATE_VERSION.to_string(),
            simulator_source_id: format!("sim://{}", self.scenario.meta.scenario_id),
            asset_models,
            adapters: self.adapter_inventory(),
            records_ingested: self.transport.sample_count(),
            quality_counts: self.transport.quality_counts(),
            availability_changes,
            dropouts,
            roll_plan,
            commands,
            grid_checks,
            consumption_decisions: Vec::new(),
            statements: vec![
                P5_14_SIMULATION_ONLY_STATEMENT.to_string(),
                P5_GRID_SOLVER_STATEMENT.to_string(),
                P5_GRID_PROVIDER_BOUNDARY.to_string(),
                TRACKING_QUALITY_RULE.to_string(),
                "the command-store, outbox, envelope, model-version, asset-capability, \
                 and authorization seams of this composition are SEAM-LEVEL in-memory \
                 stand-ins named in the adapter inventory; the gates, validators, \
                 lifecycle engine, dispatch adapter, simulator, grid pipeline, and roll \
                 planner are the REAL platform code"
                    .to_string(),
            ],
            // P5.21–P5.23: attached by the caller (with_fault_journal /
            // with_timings) after the composition completes — never a
            // placeholder here.
            fault_journal: None,
            timings: None,
        })
    }

    /// The adapter inventory of THIS composition (the same list the record
    /// carries; public so tests and callers can assert it directly).
    pub fn adapter_inventory(&self) -> Vec<AdapterTrace> {
        vec![
            AdapterTrace {
                leg: "telemetry_ingest".to_string(),
                adapter: format!(
                    "SimulatorTelemetrySource ({})",
                    self.scenario.meta.scenario_id
                ),
                implementation: AdapterImplementation::Real,
                detail: "the Phase 2 simulator through the TelemetrySource port; the same \
                         provider the control-plane ingestion composes"
                    .to_string(),
            },
            AdapterTrace {
                leg: "dispatch".to_string(),
                adapter: "SimulatorDispatchAdapter (P4.16) over SimulatorTelemetryTransport"
                    .to_string(),
                implementation: AdapterImplementation::Real,
                detail: "the transport is the composition-root implementation over \
                         flexcore-simulator; no transport fake participates"
                    .to_string(),
            },
            AdapterTrace {
                leg: "command_gates".to_string(),
                adapter: "CommandGates (P4.11–P4.13) over in-memory seams".to_string(),
                implementation: AdapterImplementation::Seam,
                detail: "the record store, outbox, envelope, model-version, asset-capability, \
                         and authorization seams are in-memory composition support; the gate \
                         logic is the real application code"
                    .to_string(),
            },
            AdapterTrace {
                leg: "grid_feasibility".to_string(),
                adapter: format!(
                    "run_feasibility_gate (P4.04–P4.07) + PandapowerGridProvider over {}",
                    self.grid_runner.backend.name()
                ),
                implementation: match self.grid_runner.backend {
                    GridBackendSelection::Scripted => AdapterImplementation::Seam,
                    GridBackendSelection::WorkerProcess { .. } => AdapterImplementation::Real,
                },
                detail: match &self.grid_runner.backend {
                    GridBackendSelection::Scripted => {
                        "the pipeline, provider, scope, translation, and limit check are REAL; \
                         the power-flow outcome is a recorded in-process fixture (NOT a \
                         computed power flow)"
                            .to_string()
                    }
                    GridBackendSelection::WorkerProcess {
                        program,
                        script_path,
                        timeout,
                    } => format!(
                        "the REAL P5.15 Python worker leg ({program} {script_path}, timeout \
                         {timeout:?}); UNVERIFIED until a human build/test pass executes it"
                    ),
                },
            },
            AdapterTrace {
                leg: "schedule_roll".to_string(),
                adapter: "plan_rolling_horizon_roll (P3.24)".to_string(),
                implementation: AdapterImplementation::Real,
                detail: "the planner is REAL and the realised state comes from the REAL \
                         simulator telemetry; the re-solve itself is the composition root's \
                         optimizer port (out of this composition's scope)"
                    .to_string(),
            },
        ]
    }
}

// --- Helpers ----------------------------------------------------------------------

/// A deterministic UUID for the composition's fixed identity fixtures
/// (FNV-1a-64 over a domain-separated name, doubled into the 16 bytes —
/// the same derivation discipline the P2.24 stand-in identity uses). Public
/// so the CLI subcommand derives the SAME stand-in identities for a given
/// scenario (a run is reproducible, never a fresh random identity).
pub fn deterministic_uuid(name: &str) -> FlexUuid {
    fn fnv1a64(bytes: &[u8]) -> u64 {
        const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET_BASIS;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash
    }
    let h1 = fnv1a64(name.as_bytes());
    let h2 = fnv1a64(format!("flexcore-sim-runner/{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&h1.to_be_bytes());
    bytes[8..].copy_from_slice(&h2.to_be_bytes());
    FlexUuid::from_bytes(bytes)
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("a valid actor identity")
}

/// The three-bus grid-model fixture (the recorded magnitudes the grid
/// crate's `tests_support::three_bus_model` uses, restated here so the
/// sim-runner does not depend on the grid crate's test-support module).
/// The parameters are illustrative fixture magnitudes for the composition;
/// the physics (when the real worker backend is configured) runs in the
/// worker, never here.
fn three_bus_fixture(model_ref: GridModelRef) -> VersionedNetworkModel {
    VersionedNetworkModel {
        model_ref,
        base_mva: 100.0,
        buses: vec![
            BusSpec {
                bus_id: "bus_1".to_string(),
                nominal_kv: 20.0,
                is_slack: true,
            },
            BusSpec {
                bus_id: "bus_2".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
            BusSpec {
                bus_id: "bus_3".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
        ],
        branches: vec![BranchSpec {
            branch_id: "line_2_3".to_string(),
            from_bus: "bus_2".to_string(),
            to_bus: "bus_3".to_string(),
            r_ohm: 0.21,
            x_ohm: 0.07,
            rated_current_a: 500.0,
        }],
        transformers: vec![TransformerSpec {
            transformer_id: "trafo_1_2".to_string(),
            hv_bus: "bus_1".to_string(),
            lv_bus: "bus_2".to_string(),
            rated_mva: 0.4,
            vk_percent: 4.0,
            vk_r_percent: 1.2,
        }],
        external_grids: vec![ExternalGridSpec {
            bus_id: "bus_1".to_string(),
            vm_pu: 1.02,
            va_degree: 0.0,
        }],
    }
}

/// The meshed two-feeder grid-model fixture (P5.20 medium case): one MV
/// substation bus, two LV feeders, each behind its own transformer, with
/// two mesh ties closing the ring between the feeders — the §19.3
/// "multiple feeders/sites connected through a common substation
/// constraint" case at fixture scale. BALANCED only: the adapter's
/// declared topology scope is `TopologyScope::RadialAndMeshed` (the
/// balanced solver covers both; the P4.34 record) and the fixture is a
/// NAMED case under that declared scope. The parameters are illustrative
/// SYNTHETIC fixture magnitudes — no IEEE feeder is used or claimed
/// (§19.3; the P5.20 per-case record owns the honest entry).
fn meshed_two_feeder_fixture(model_ref: GridModelRef) -> VersionedNetworkModel {
    VersionedNetworkModel {
        model_ref,
        base_mva: 100.0,
        buses: vec![
            BusSpec {
                bus_id: "bus_1".to_string(),
                nominal_kv: 20.0,
                is_slack: true,
            },
            BusSpec {
                bus_id: "bus_2".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
            BusSpec {
                bus_id: "bus_3".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
            BusSpec {
                bus_id: "bus_4".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
            BusSpec {
                bus_id: "bus_5".to_string(),
                nominal_kv: 0.4,
                is_slack: false,
            },
        ],
        branches: vec![
            BranchSpec {
                branch_id: "line_2_3".to_string(),
                from_bus: "bus_2".to_string(),
                to_bus: "bus_3".to_string(),
                r_ohm: 0.21,
                x_ohm: 0.07,
                rated_current_a: 500.0,
            },
            BranchSpec {
                branch_id: "line_4_5".to_string(),
                from_bus: "bus_4".to_string(),
                to_bus: "bus_5".to_string(),
                r_ohm: 0.21,
                x_ohm: 0.07,
                rated_current_a: 500.0,
            },
            // The mesh ties: two branches closing the ring between the
            // feeders — this is what makes the fixture MESHED.
            BranchSpec {
                branch_id: "line_2_4".to_string(),
                from_bus: "bus_2".to_string(),
                to_bus: "bus_4".to_string(),
                r_ohm: 0.35,
                x_ohm: 0.12,
                rated_current_a: 400.0,
            },
            BranchSpec {
                branch_id: "line_3_5".to_string(),
                from_bus: "bus_3".to_string(),
                to_bus: "bus_5".to_string(),
                r_ohm: 0.35,
                x_ohm: 0.12,
                rated_current_a: 400.0,
            },
        ],
        transformers: vec![
            TransformerSpec {
                transformer_id: "trafo_1_2".to_string(),
                hv_bus: "bus_1".to_string(),
                lv_bus: "bus_2".to_string(),
                rated_mva: 0.4,
                vk_percent: 4.0,
                vk_r_percent: 1.2,
            },
            TransformerSpec {
                transformer_id: "trafo_1_4".to_string(),
                hv_bus: "bus_1".to_string(),
                lv_bus: "bus_4".to_string(),
                rated_mva: 0.63,
                vk_percent: 4.0,
                vk_r_percent: 1.2,
            },
        ],
        external_grids: vec![ExternalGridSpec {
            bus_id: "bus_1".to_string(),
            vm_pu: 1.02,
            va_degree: 0.0,
        }],
    }
}
