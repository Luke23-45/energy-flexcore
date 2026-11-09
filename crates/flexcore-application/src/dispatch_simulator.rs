// P4.16 — Simulation-only dispatch adapter: issue, acknowledgement,
//         tracking (§14.1–§14.2, §19.1, §23 Stage 4, §24 steps 6–8; ledger
//         P4.16; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/14_16_lifecycle_completion.md`).
//
// The §8.1 adapter pattern applied to dispatch, with EXACTLY ONE Phase 4
// implementation: the simulator adapter. NO production protocol adapter
// exists in this phase — that is Stage 6, and field dispatch additionally
// requires Stage 9 qualification (the hard statement is
// [`SIMULATION_ONLY_STATEMENT`]).
//
// ## The seam statement
//
// flexcore-application may depend only on flexcore-contracts, flexcore-
// domain, flexcore-ports, flexcore-events, and flexcore-optimization-api
// (tools/check_dependency_rule.py; the manifest is not this subtask's
// file) — it CANNOT name flexcore-simulator. The simulator is therefore
// reached through the [`SimulatorDispatchTransport`] port declared HERE,
// implemented by the composition root over the Phase 2 simulator crate.
// The adapter drives the simulator through the port ONLY — never its
// internals. The port is deliberately narrow: deliver one signed envelope
// (the issue), pull observed samples (the tracking) — there is no device
// write method to grow, and the structural test in
// `tests/dispatch_simulator.rs` pins the port shape and scans this source
// for forbidden write-path identifiers.
//
// ## What issue/acknowledgement/tracking mean (all decided once)
//
// - **Issue (D-P4.16-1).** APPROVED → ISSUED only for a SIGNED, SEQUENCED
//   command. The signing identity is the [`SimulationKeyring`] — a
//   dev/SIMULATION keyring type, clearly labelled; it produces a
//   deterministic, non-cryptographic signature REFERENCE
//   (`sim-sig:<key_id>:<command_id>`). It is NOT a production PKI and MUST
//   be replaced by the Stage 6/9-qualified signing identity before any
//   non-simulation use. The signature/transport identity is recorded per
//   §14.3 field 15 as the ISSUED journal row's evidence reference. The
//   adapter REFUSES an unsigned, unsequenced, or expired command with
//   named errors ([`check_issue_refusals`]) — and the unsequenced/
//   unsigned cases are additionally UNREPRESENTABLE through the P1.06
//   constructor (defence in depth for the stored-record path).
//   Before ISSUED the adapter runs the P4.12 hook
//   (`CommandGates::ensure_command_issueable`: the feasibility evidence
//   must bind the ACTIVE model hash, §17.5 invariant 3) and the P4.14
//   ISSUE re-validation (`CommandRecordFields::validate_issue` — every
//   §14.3 field present, the authorization decision real, the
//   authorization not expired), cross-checking the caller's P4.13
//   authorization record against the journal's APPROVED evidence.
// - **Acknowledgement (D-P4.16-2).** ISSUED → ACKNOWLEDGED only on
//   application-layer ACCEPTANCE from the simulator; a NACK or a transport
//   timeout/failure moves ISSUED → FAILED with the frozen-table trigger.
//   Acknowledgement is receipt/acceptance, NEVER physical delivery
//   (§14.1) — every type, state, and docstring here says "acceptance".
// - **Tracking (D-P4.16-3).** ACKNOWLEDGED → TRACKING on observed
//   telemetry; the measured response is compared with the requested value
//   against the [`TrackingAcceptanceCriterion`] — DEFINED DEPLOYMENT
//   CONFIGURATION, never hard-coded (recorded decision: the criterion's
//   source is the per-asset-class configuration the composition root
//   supplies; this module only validates it). The recorded evaluation
//   order: (1) any observed sample outside the tolerance → FAILED
//   (tracking deviation — a recorded deviation fails the command even if
//   the asset recovers later); (2) otherwise, at least
//   `min_satisfied_samples` within-tolerance observations in this poll →
//   COMPLETED; (3) otherwise, the tracking timeout elapsed since the
//   → TRACKING journal timestamp → FAILED (tracking timeout);
//   (4) otherwise keep tracking. Validity ending during tracking →
//   EXPIRED (before entering tracking: the milestone
//   `BeforeTrackingCompletion`; during: `BeforeCompletion`) via the
//   domain guard. Replacement during tracking → SUPERSEDED via the P4.15
//   service (tested end-to-end in `tests/dispatch_simulator.rs`).
// - **§24 step 8 simulations (D-P4.16-4).** Acknowledgement, tracking,
//   partial failure, and timeout are driven through the transport port's
//   scripted results. The recorded mapping to the Phase 2 simulator's
//   fault vocabulary: `drop` → no acceptance (timeout),
//   `source_invalid` → NACK, `out_of_range` → a tracking sample outside
//   tolerance (deviation). The composition root's transport implements
//   this mapping over `flexcore-simulator`; the test fake scripts the
//   same semantics directly (flexcore-application cannot name the
//   simulator crate).
// - **Per-asset partial success (D-P4.16-5).** Every command is
//   single-asset (the §14.3 scope); a fleet operation is a set of
//   commands, and its acknowledgement is the P4.14
//   [`FleetAcknowledgement`](crate::command_record::FleetAcknowledgement)
//   — computed per asset, NEVER inferred from the first responding device.
//
// ## The hard target guard (D-P4.16-6)
//
// [`SimulatorDispatchAdapter::configure`] accepts ONLY the
// [`AdapterTargetCandidate::Simulator`] variant; any
// [`AdapterTargetCandidate::NonSimulator`] candidate (gateway, protocol
// adapter, device, field endpoint) is REFUSED with a named error, and the
// guard test exercises every non-simulator kind. There is NO device write
// path in this module: the transport port carries deliver + observe only.
//
// ## Control status
//
// The command lifecycle is a protected platform invariant (§0.2, §5.7).
// The entire 0.x line is simulation-only (§21.4): an ISSUED command here
// was sent to the SIMULATOR and nothing else; an ACKNOWLEDGED command was
// accepted at the application layer by a simulator and was NOT delivered
// to any physical device. FlexCore performs no primary frequency
// response, synthetic inertia, relay protection, inverter inner-loop
// control, or sub-cycle control (§14.5): setpoint delivery is supervisory
// and carries no latency guarantee.

use flexcore_contracts::time_model::EventTime;
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, EvidenceReference, FailureReason, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    CommandAuthenticity, ControlParameterValue, ControlTarget, DispatchCommand,
};
use flexcore_domain::ids::{AssetId, CommandId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{
    DispatchAcknowledgedPayload, DispatchCompletedPayload, DispatchFailedPayload,
    DispatchExpiredPayload, DispatchIssuedPayload, DispatchTrackingUpdatedPayload, DomainEvent,
};

use crate::command_gates::{
    CommandGateStoreError, CommandGateView, CommandGates, ModelVersionGateStatus,
};
use crate::command_record::{
    expiry_milestone_for, replay_full_lifecycle, AssetAcknowledgement, AssetAcknowledgementState,
    AuthorizationRecordFields, CommandRecordError, CommandRecordFields,
};
use crate::event_assembly::emit;
use flexcore_contracts::ids::FlexUuid;

/// The hard simulation-only statement (ledger P4.16 step 5): the adapter's
/// transport target is the SIMULATOR ONLY. Field dispatch requires Stage 6
/// (protocol adapters) plus Stage 9 qualification, and operator-approved
/// control is a v1.0.0 matter. NO device write path exists in this module.
pub const SIMULATION_ONLY_STATEMENT: &str = "the dispatch adapter's transport target is the \
     SIMULATOR ONLY (§21.4, §23 Stage 4); field dispatch requires Stage 6 protocol adapters \
     plus Stage 9 qualification and operator-approved control (v1.0.0); no device write path \
     exists in this code";

/// The transport-identity prefix recorded on issued envelopes (the §14.3
/// authenticated transport identity for the simulation channel).
pub const SIMULATOR_TRANSPORT_IDENTITY_PREFIX: &str = "sim://dispatch-adapter";

/// The upper bound on observations pulled per tracking poll (a fail-safe
/// against a misbehaving transport that never returns `None`; a real
/// tracking window is a handful of samples).
pub const MAX_OBSERVATIONS_PER_POLL: usize = 128;

// --- The target guard ----------------------------------------------------------------

/// The kind of a non-simulator target candidate. These variants exist so
/// the guard is EXERCISABLE: the adapter constructor refuses every one of
/// them with a named error, and the guard test tries each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NonSimulatorTargetKind {
    /// A site gateway/edge agent endpoint.
    Gateway,
    /// A production protocol adapter (OpenEMS, SunSpec/Modbus, … — Stage
    /// 6, not built here).
    ProtocolAdapter,
    /// A physical device endpoint.
    Device,
    /// Any other field/production endpoint.
    FieldEndpoint,
}

impl NonSimulatorTargetKind {
    /// The stable recorded text.
    pub fn label(self) -> &'static str {
        match self {
            NonSimulatorTargetKind::Gateway => "gateway",
            NonSimulatorTargetKind::ProtocolAdapter => "protocol_adapter",
            NonSimulatorTargetKind::Device => "device",
            NonSimulatorTargetKind::FieldEndpoint => "field_endpoint",
        }
    }
}

/// The transport-target candidate handed to
/// [`SimulatorDispatchAdapter::configure`]. Exactly one variant is
/// accepted: the simulator (the sole Phase 4 target, §21.4/§23 Stage 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterTargetCandidate {
    /// The Phase 2 telemetry simulator (`sim://<scenario/provider id>`) —
    /// the ONLY accepted target.
    Simulator {
        /// The simulator provider/scenario identity (recorded, informational).
        simulator_id: String,
    },
    /// Any non-simulator target — structurally REFUSED by the adapter
    /// constructor (D-P4.16-6; field dispatch requires Stage 6 + Stage 9).
    NonSimulator {
        /// Which kind of non-simulator target was attempted.
        kind: NonSimulatorTargetKind,
        /// The attempted endpoint (recorded in the refusal, audit text).
        endpoint: String,
    },
}

// --- The simulation signing identity (D-P4.16-1) ---------------------------------------

/// One signing key of the simulation keyring. DEV/SIMULATION ONLY: the
/// "signature" is a deterministic, non-cryptographic reference
/// (`sim-sig:<key_id>:<command_id>`) recorded per §14.3 field 15. A
/// production signing identity (real key custody, algorithm, verification)
/// is a Stage 6/9 qualification matter and MUST replace this type before
/// any non-simulation use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationSigningKey {
    /// The key identifier (recorded on every signature reference).
    pub key_id: String,
}

/// The dev/simulation signing keyring (D-P4.16-1). Clearly labelled by
/// name and by documentation: this is NOT a production PKI, and no
/// security property beyond traceability of the simulation signing
/// identity is claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationKeyring {
    active_key: SimulationSigningKey,
}

impl SimulationKeyring {
    /// Builds the keyring over one active simulation key. The key id must
    /// be non-empty (an unattributable signature reference is refused).
    pub fn new(key_id: impl Into<String>) -> Result<Self, DispatchAdapterError> {
        let key_id = key_id.into();
        if key_id.trim().is_empty() {
            return Err(DispatchAdapterError::InvalidCriterion {
                detail: "the simulation signing key id must not be empty".to_string(),
            });
        }
        Ok(Self {
            active_key: SimulationSigningKey { key_id },
        })
    }

    /// The active simulation key's identifier.
    pub fn active_key_id(&self) -> &str {
        &self.active_key.key_id
    }

    /// The deterministic SIMULATION signature reference for a command
    /// (`sim-sig:<key_id>:<command_id>`). Traceability only — no
    /// cryptographic property is claimed.
    pub fn sign_command(&self, command_id: CommandId) -> String {
        format!("sim-sig:{}:{}", self.active_key.key_id, command_id)
    }
}

// --- The transport port (the ONLY integration point with the simulator) ----------------

/// The requested value carried on the dispatch envelope. Every variant is
/// a canonical canonical-unit value (MW / MVAr / SOC fraction — §10.2);
/// the sign convention travels on the command's declared conventions, not
/// on the envelope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimulatorTargetValue {
    /// A target active power (MW; frozen `P>0=export`).
    ActivePowerMW(f64),
    /// A target reactive power (MVAr; declared convention on the command).
    ReactivePowerMVAr(f64),
    /// A target SOC fraction in [0,1].
    SocFraction(f64),
}

impl SimulatorTargetValue {
    /// The stable quantity label (matches [`TrackedQuantity::label`]).
    pub fn quantity(&self) -> &'static str {
        match self {
            SimulatorTargetValue::ActivePowerMW(_) => "active_power_mw",
            SimulatorTargetValue::ReactivePowerMVAr(_) => "reactive_power_mvar",
            SimulatorTargetValue::SocFraction(_) => "soc_fraction",
        }
    }

    /// The value.
    pub fn value(&self) -> f64 {
        match self {
            SimulatorTargetValue::ActivePowerMW(v)
            | SimulatorTargetValue::ReactivePowerMVAr(v)
            | SimulatorTargetValue::SocFraction(v) => *v,
        }
    }
}

/// The signed, sequenced dispatch envelope the adapter hands to the
/// simulator transport (§14.2 ISSUED: "signed, sequenced command sent";
/// §14.3 field 15: the signature/transport identity is RECORDED on the
/// envelope).
#[derive(Debug, Clone, PartialEq)]
pub struct SimulatorDispatchEnvelope {
    /// The command.
    pub command_id: CommandId,
    /// The target asset.
    pub asset_id: AssetId,
    /// The monotonic per-asset/channel sequence (§14.3 field 10).
    pub sequence: u64,
    /// The requested value (canonical units).
    pub target: SimulatorTargetValue,
    /// The simulation signature reference (D-P4.16-1).
    pub signature_reference: String,
    /// The authenticated transport identity of the simulation channel
    /// (§14.3 field 15).
    pub transport_identity: String,
}

/// The simulator-side failure kind a transport error reports. `Timeout` is
/// the §14.1 ISSUED → FAILED trigger "acknowledgement timeout"; the others
/// map to "transport failure".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulatorTransportFailure {
    /// The simulator did not answer within the acknowledgement window.
    Timeout,
    /// The transport could not reach the simulator.
    Unreachable,
    /// The simulator reported an internal error.
    SimulatorError,
}

/// A transport error (opaque code/detail; the ADAPTER decides the
/// lifecycle consequence — the transport never transitions state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatorTransportError {
    /// The failure kind.
    pub failure: SimulatorTransportFailure,
    /// The recorded detail (audit text).
    pub detail: String,
}

/// The simulator's application-layer acceptance result (§14.1 edge 13:
/// "device/gateway accepts" — acceptance at the application layer, never
/// physical delivery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulatedAcceptance {
    /// The simulator ACCEPTED the command at the application layer; the
    /// acceptance record reference is mandatory.
    Accepted {
        /// The acceptance record reference (journal evidence).
        acceptance_reference: String,
    },
    /// The simulator refused the command (NACK) with its reason.
    Nack {
        /// The recorded NACK reason.
        reason: String,
    },
}

/// One observed telemetry sample pulled from the simulator (§14.1 edge 18:
/// "telemetry observed"). The quantity tag must match the tracking
/// criterion's quantity — a mismatch is fail-closed, never coerced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulatedObservation {
    /// Which quantity was observed.
    pub quantity: TrackedQuantity,
    /// The observed value (canonical units).
    pub value: f64,
    /// The observation instant (scenario/controlled clock, UTC millis).
    pub observed_at_unix_millis: i64,
}

/// The quantity a tracking criterion compares (the tracked form of the
/// command's target).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackedQuantity {
    /// Active power (MW).
    ActivePowerMW,
    /// Reactive power (MVAr).
    ReactivePowerMVAr,
    /// SOC fraction in [0,1].
    SocFraction,
}

impl TrackedQuantity {
    /// The stable label (matches [`SimulatorTargetValue::quantity`]).
    pub fn label(self) -> &'static str {
        match self {
            TrackedQuantity::ActivePowerMW => "active_power_mw",
            TrackedQuantity::ReactivePowerMVAr => "reactive_power_mvar",
            TrackedQuantity::SocFraction => "soc_fraction",
        }
    }
}

/// The transport port to the SIMULATOR — the only integration point this
/// adapter has with the Phase 2 simulator crate (the composition root
/// implements it over `flexcore-simulator`). Deliberately NARROW: deliver
/// (issue) + observe (tracking). There is NO device-write method — the
/// structural test pins the port shape.
pub trait SimulatorDispatchTransport {
    /// Delivers the signed, sequenced envelope to the simulator and
    /// returns its application-layer acceptance. A transport-level failure
    /// (timeout/unreachable/simulator error) is an `Err` — the adapter
    /// maps it to the frozen ISSUED → FAILED trigger.
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<SimulatedAcceptance, SimulatorTransportError>;

    /// Pulls the next observed telemetry sample for the asset at the
    /// supplied (controlled) instant, or `None` when no sample is pending.
    /// The composition root's implementation maps the simulator's fault
    /// vocabulary per D-P4.16-4 (drop → absence, out_of_range → an
    /// out-of-tolerance sample).
    fn observe(
        &self,
        asset_id: AssetId,
        at_unix_millis: i64,
    ) -> Result<Option<SimulatedObservation>, SimulatorTransportError>;
}

// --- The acceptance criterion (D-P4.16-3, configuration not hard-coded) ----------------

/// The tracking acceptance criterion (§14.1 edge 22: "acceptance criterion
/// satisfied"). RECORDED DECISION: the criterion is DEPLOYMENT
/// CONFIGURATION supplied by the composition root (per asset class) — this
/// module validates it and applies it, but never hard-codes a threshold.
/// Semantics (D-P4.16-3, decided once):
///
/// 1. any observed sample with |measured − requested| > `tolerance` →
///    tracking deviation (FAILED);
/// 2. otherwise, at least `min_satisfied_samples` within-tolerance
///    observations in one poll → COMPLETED;
/// 3. otherwise, `tracking_timeout_millis` elapsed since the → TRACKING
///    journal timestamp → tracking timeout (FAILED).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackingAcceptanceCriterion {
    /// The quantity compared.
    pub quantity: TrackedQuantity,
    /// The absolute tolerance in the quantity's canonical unit (MW / MVAr
    /// / fraction). Finite and strictly positive.
    pub tolerance: f64,
    /// The minimum number of within-tolerance observations required in one
    /// poll (≥ 1).
    pub min_satisfied_samples: u32,
    /// The tracking timeout (milliseconds since the → TRACKING journal
    /// timestamp). Strictly positive.
    pub tracking_timeout_millis: i64,
}

impl TrackingAcceptanceCriterion {
    /// Validates and builds a criterion (configuration, not code).
    pub fn new(
        quantity: TrackedQuantity,
        tolerance: f64,
        min_satisfied_samples: u32,
        tracking_timeout_millis: i64,
    ) -> Result<Self, DispatchAdapterError> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(DispatchAdapterError::InvalidCriterion {
                detail: format!("the tracking tolerance must be finite and positive, got {tolerance}"),
            });
        }
        if min_satisfied_samples == 0 {
            return Err(DispatchAdapterError::InvalidCriterion {
                detail: "the minimum satisfied-sample count must be at least 1".to_string(),
            });
        }
        if tracking_timeout_millis <= 0 {
            return Err(DispatchAdapterError::InvalidCriterion {
                detail: "the tracking timeout must be positive".to_string(),
            });
        }
        Ok(Self {
            quantity,
            tolerance,
            min_satisfied_samples,
            tracking_timeout_millis,
        })
    }
}

/// The pure tracking evaluation verdict (see
/// [`evaluate_tracking`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackingVerdict {
    /// The acceptance criterion is satisfied — TRACKING → COMPLETED.
    Completed {
        /// The within-tolerance observation count that satisfied it.
        satisfied_samples: u32,
    },
    /// A sample deviated beyond the tolerance — TRACKING → FAILED
    /// (tracking deviation).
    Deviation {
        /// The deviating measured value.
        measured: f64,
        /// The requested value.
        requested: f64,
        /// The criterion tolerance.
        tolerance: f64,
    },
    /// The tracking window timed out — TRACKING → FAILED (tracking
    /// timeout).
    Timeout,
    /// Neither satisfied nor failed — the command stays in TRACKING.
    KeepTracking {
        /// The within-tolerance observation count so far in this poll.
        satisfied_samples: u32,
    },
}

/// Evaluates the acceptance criterion over one poll's observations
/// (D-P4.16-3 evaluation order: deviation, completion, timeout,
/// keep-tracking). `tracking_started_unix_millis` is the → TRACKING
/// journal timestamp (the evidence of record, not an adapter-side guess).
/// The observations must all carry the criterion's quantity — the adapter
/// checks that (fail-closed) before calling.
pub fn evaluate_tracking(
    criterion: &TrackingAcceptanceCriterion,
    requested: f64,
    observations: &[SimulatedObservation],
    tracking_started_unix_millis: i64,
    now_unix_millis: i64,
) -> TrackingVerdict {
    let mut satisfied: u32 = 0;
    for observation in observations {
        let delta = (observation.value - requested).abs();
        if delta > criterion.tolerance {
            return TrackingVerdict::Deviation {
                measured: observation.value,
                requested,
                tolerance: criterion.tolerance,
            };
        }
        satisfied += 1;
    }
    if satisfied >= criterion.min_satisfied_samples {
        return TrackingVerdict::Completed { satisfied_samples: satisfied };
    }
    if now_unix_millis - tracking_started_unix_millis >= criterion.tracking_timeout_millis {
        return TrackingVerdict::Timeout;
    }
    TrackingVerdict::KeepTracking { satisfied_samples: satisfied }
}

// --- The issue refusals (D-P4.16-1) -----------------------------------------------------

/// The inputs of the issue-refusal check (unsigned / unsequenced / expired,
/// §14.2 ISSUED + ledger P4.16 step 2). Broken out as a standalone input
/// struct so every refusal is testable: the unsigned and unsequenced cases
/// are unrepresentable through the P1.06 constructor, but the STORED-record
/// path (and future composition roots) must still be refused — defence in
/// depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRefusalInput<'a> {
    /// The command's authenticity attestation (§14.3 field 15).
    pub authenticity: &'a CommandAuthenticity,
    /// The command's sequence (§14.3 field 10).
    pub sequence: u64,
    /// The store-authoritative valid-until bound (UTC unix millis).
    pub valid_until_millis: i64,
    /// The controlled clock's current instant (UTC unix millis).
    pub now_unix_millis: i64,
}

/// The refusal that fired (independent of the command ID, so the check is
/// reusable; the adapter attaches the command to its named error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueRefusal {
    /// The command carries no signature or authenticated transport
    /// identity.
    Unsigned,
    /// The command carries no (or a zero) sequence.
    Unsequenced,
    /// The command's validity window has ended.
    Expired {
        /// The passed bound.
        valid_until_millis: i64,
        /// The refusing instant.
        now_unix_millis: i64,
    },
}

/// The issue refusals, in ledger order (unsigned, unsequenced, expired).
/// `Ok(())` means none fired.
pub fn check_issue_refusals(input: &IssueRefusalInput<'_>) -> Result<(), IssueRefusal> {
    let empty = match input.authenticity {
        CommandAuthenticity::Signature(s) => s.trim().is_empty(),
        CommandAuthenticity::AuthenticatedTransport(s) => s.trim().is_empty(),
    };
    if empty {
        return Err(IssueRefusal::Unsigned);
    }
    if input.sequence == 0 {
        return Err(IssueRefusal::Unsequenced);
    }
    if input.now_unix_millis >= input.valid_until_millis {
        return Err(IssueRefusal::Expired {
            valid_until_millis: input.valid_until_millis,
            now_unix_millis: input.now_unix_millis,
        });
    }
    Ok(())
}

// --- Errors -----------------------------------------------------------------------------

/// The named errors of the simulator dispatch adapter. Refusals are named
/// errors that BLOCK the transition — the adapter never silently ignores
/// or auto-corrects a refusal, and a refused command keeps its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchAdapterError {
    /// The command does not exist in the caller's tenant scope.
    NotFound {
        /// The command that was not found.
        command_id: String,
    },
    /// The operation does not apply to the command's current state.
    WrongState {
        /// The state the command is in.
        current: CommandState,
        /// Which operation was attempted.
        operation: &'static str,
    },
    /// The command carries no signature or authenticated transport
    /// identity — refused (§14.2 ISSUED: "signed … command sent").
    UnsignedCommandRefused {
        /// The refused command.
        command_id: String,
    },
    /// The command carries no (or a zero) sequence — refused.
    UnsequencedCommandRefused {
        /// The refused command.
        command_id: String,
    },
    /// The command's validity window has ended — refused (EXPIRED is the
    /// P4.15 sweep's transition; the adapter refuses, it does not expire).
    ExpiredCommandRefused {
        /// The refused command.
        command_id: String,
        /// The passed bound.
        valid_until_millis: i64,
        /// The refusing instant.
        now_unix_millis: i64,
    },
    /// The caller's P4.13 authorization record does not match the
    /// journal's APPROVED evidence — fail-closed.
    AuthorizationRecordMismatch {
        /// The command.
        command_id: String,
        /// The journal's APPROVED evidence reference.
        journal_reference: String,
        /// The supplied record's decision reference.
        record_reference: String,
    },
    /// The §14.3 issue re-validation failed (a named field error blocks
    /// the transition — ledger P4.14 step 2).
    Record(CommandRecordError),
    /// The referenced grid model version is not Active — fail-closed
    /// (P4.03: only an Active model may back a feasibility check).
    ModelNotActive {
        /// The grid-model reference.
        grid_model_ref: String,
        /// The model's lifecycle state.
        state: String,
    },
    /// The referenced grid model version is unknown — fail-closed.
    ModelUnknown {
        /// The grid-model reference.
        grid_model_ref: String,
    },
    /// A non-simulator target candidate was refused (D-P4.16-6: the
    /// transport target is the SIMULATOR ONLY).
    NonSimulatorTargetRefused {
        /// The attempted target kind.
        kind: &'static str,
        /// The attempted endpoint.
        endpoint: String,
    },
    /// The tracking acceptance criterion is invalid (configuration, not
    /// code).
    InvalidCriterion {
        /// The recorded detail.
        detail: String,
    },
    /// The observed samples' quantity does not match the tracking
    /// criterion — fail-closed, never coerced.
    ObservationMismatch {
        /// The command.
        command_id: String,
        /// The criterion's quantity.
        criterion_quantity: &'static str,
        /// The observed quantity.
        observation_quantity: &'static str,
    },
    /// No telemetry was observed, so ACKNOWLEDGED → TRACKING cannot fire
    /// (edge 18: "telemetry observed" — observation is required).
    NoTelemetryObserved {
        /// The command.
        command_id: String,
    },
    /// The store seam failed.
    Store(CommandGateStoreError),
    /// A domain lifecycle guard rejected the transition.
    Lifecycle(LifecycleError),
    /// The outbox write failed after the state change was recorded.
    Event {
        /// The recorded detail.
        detail: String,
    },
    /// A timestamp could not be represented.
    InvalidTimestamp {
        /// The recorded detail.
        detail: String,
    },
    /// The transport to the simulator failed (the adapter maps the kind to
    /// the frozen ISSUED → FAILED trigger).
    Transport(SimulatorTransportError),
}

impl std::fmt::Display for DispatchAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchAdapterError::NotFound { command_id } => {
                write!(f, "command {command_id} does not exist in this tenant scope")
            }
            DispatchAdapterError::WrongState { current, operation } => write!(
                f,
                "operation `{operation}` does not apply to a command in state {}",
                current.name()
            ),
            DispatchAdapterError::UnsignedCommandRefused { command_id } => write!(
                f,
                "command {command_id} is unsigned — refused (a signed, sequenced command is \
                 required for ISSUED)"
            ),
            DispatchAdapterError::UnsequencedCommandRefused { command_id } => write!(
                f,
                "command {command_id} is unsequenced — refused (a monotonic per-asset/channel \
                 sequence is required for ISSUED)"
            ),
            DispatchAdapterError::ExpiredCommandRefused {
                command_id,
                valid_until_millis,
                now_unix_millis,
            } => write!(
                f,
                "command {command_id} is expired (valid-until {valid_until_millis}, now \
                 {now_unix_millis}) — refused; the EXPIRED transition is the expiry sweep's"
            ),
            DispatchAdapterError::AuthorizationRecordMismatch {
                command_id,
                journal_reference,
                record_reference,
            } => write!(
                f,
                "command {command_id}: the supplied authorization record '{record_reference}' \
                 does not match the journal's APPROVED evidence '{journal_reference}'"
            ),
            DispatchAdapterError::Record(error) => {
                write!(f, "§14.3 issue re-validation: {error}")
            }
            DispatchAdapterError::ModelNotActive {
                grid_model_ref,
                state,
            } => write!(
                f,
                "grid model {grid_model_ref} is not Active (state {state}); fail-closed"
            ),
            DispatchAdapterError::ModelUnknown { grid_model_ref } => {
                write!(f, "grid model {grid_model_ref} is unknown; fail-closed")
            }
            DispatchAdapterError::NonSimulatorTargetRefused { kind, endpoint } => write!(
                f,
                "non-simulator dispatch target '{kind}' ({endpoint}) is REFUSED: the adapter's \
                 transport target is the simulator only; field dispatch requires Stage 6 + \
                 Stage 9 qualification"
            ),
            DispatchAdapterError::InvalidCriterion { detail } => write!(f, "{detail}"),
            DispatchAdapterError::ObservationMismatch {
                command_id,
                criterion_quantity,
                observation_quantity,
            } => write!(
                f,
                "command {command_id}: observed quantity {observation_quantity} does not match \
                 the tracking criterion's {criterion_quantity}; fail-closed"
            ),
            DispatchAdapterError::NoTelemetryObserved { command_id } => write!(
                f,
                "command {command_id}: no telemetry observed — ACKNOWLEDGED → TRACKING requires \
                 an observation"
            ),
            DispatchAdapterError::Store(error) => write!(f, "command store: {error}"),
            DispatchAdapterError::Lifecycle(error) => write!(f, "lifecycle guard: {error}"),
            DispatchAdapterError::Event { detail } => write!(f, "outbox write failed: {detail}"),
            DispatchAdapterError::InvalidTimestamp { detail } => write!(f, "{detail}"),
            DispatchAdapterError::Transport(error) => {
                write!(f, "simulator transport: {}", error.detail)
            }
        }
    }
}

impl std::error::Error for DispatchAdapterError {}

impl From<CommandRecordError> for DispatchAdapterError {
    fn from(error: CommandRecordError) -> Self {
        DispatchAdapterError::Record(error)
    }
}

impl From<LifecycleError> for DispatchAdapterError {
    fn from(error: LifecycleError) -> Self {
        DispatchAdapterError::Lifecycle(error)
    }
}

// --- The adapter ------------------------------------------------------------------------

/// The outcome of one issue (APPROVED → ISSUED plus the acknowledgement
/// consequence). The per-asset acknowledgement is the §14.3 unit of fleet
/// reporting — a fleet operation is a set of these, aggregated by the P4.14
/// [`FleetAcknowledgement`](crate::command_record::FleetAcknowledgement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueOutcome {
    /// The journalled APPROVED → ISSUED transition (evidence = the
    /// simulation signature reference, §14.3 field 15).
    pub issued: CommandTransition,
    /// The per-asset acknowledgement (Accepted / Refused / TimedOut /
    /// TransportFailed — acceptance, never physical delivery).
    pub acknowledgement: AssetAcknowledgement,
    /// The journalled ISSUED → ACKNOWLEDGED transition, exactly when the
    /// simulator accepted.
    pub acknowledged: Option<CommandTransition>,
    /// The journalled ISSUED → FAILED transition, exactly when the
    /// simulator NACKed or the transport failed.
    pub failed: Option<CommandTransition>,
}

/// The terminal tracking result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackingTerminal {
    /// TRACKING → COMPLETED (acceptance criterion satisfied).
    Completed {
        /// The journalled transition.
        transition: CommandTransition,
    },
    /// TRACKING → FAILED (deviation or timeout).
    Failed {
        /// The journalled transition.
        transition: CommandTransition,
        /// The frozen-table trigger.
        reason: FailureReason,
    },
    /// → EXPIRED (the validity window ended before completion — from
    /// ACKNOWLEDGED: `BeforeTrackingCompletion`; from TRACKING:
    /// `BeforeCompletion`).
    Expired {
        /// The journalled transition.
        transition: CommandTransition,
    },
}

/// The outcome of one tracking poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingOutcome {
    /// The journalled ACKNOWLEDGED → TRACKING transition, exactly when this
    /// poll entered tracking (first telemetry observation).
    pub tracking_started: Option<CommandTransition>,
    /// How many observations this poll pulled from the transport.
    pub observations: usize,
    /// The pure evaluation verdict (present exactly when observations were
    /// evaluated).
    pub verdict: Option<crate::dispatch_simulator::TrackingVerdict>,
    /// The terminal transition, when the poll completed, failed, or
    /// expired the command.
    pub terminal: Option<TrackingTerminal>,
}

/// The simulation-only dispatch adapter (P4.16). EXACTLY ONE Phase 4
/// dispatch implementation exists; its transport target is the SIMULATOR
/// ONLY ([`SIMULATION_ONLY_STATEMENT`]). All collaborators are injected;
/// timestamps come EXCLUSIVELY from the gates' controlled clock.
pub struct SimulatorDispatchAdapter<'a> {
    /// The command gates (the store seam, the model-version seam, the
    /// outbox, the encoder, the id generator, the controlled clock).
    gates: &'a CommandGates<'a>,
    /// The simulator transport port (the only integration point with the
    /// simulator).
    transport: &'a dyn SimulatorDispatchTransport,
    /// The dev/simulation signing keyring (D-P4.16-1).
    keyring: SimulationKeyring,
    /// The tracking acceptance criterion (deployment configuration).
    criterion: TrackingAcceptanceCriterion,
    /// The recorded simulator identity (informational; refused otherwise).
    target: SimulatorTargetRecord,
}

/// The accepted (and recorded) simulator target.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SimulatorTargetRecord {
    simulator_id: String,
}

impl<'a> SimulatorDispatchAdapter<'a> {
    /// Configures the adapter. The HARD GUARD (D-P4.16-6): only
    /// [`AdapterTargetCandidate::Simulator`] is accepted; every
    /// non-simulator candidate is refused with a named error — a test
    /// fails if any non-simulator target is configured, and no production
    /// protocol adapter exists in this phase (Stage 6; field dispatch
    /// additionally requires Stage 9 qualification).
    pub fn configure(
        gates: &'a CommandGates<'a>,
        target: AdapterTargetCandidate,
        transport: &'a dyn SimulatorDispatchTransport,
        keyring: SimulationKeyring,
        criterion: TrackingAcceptanceCriterion,
    ) -> Result<Self, DispatchAdapterError> {
        let accepted = match target {
            AdapterTargetCandidate::Simulator { simulator_id } => {
                if simulator_id.trim().is_empty() {
                    return Err(DispatchAdapterError::InvalidCriterion {
                        detail: "the simulator id must not be empty".to_string(),
                    });
                }
                SimulatorTargetRecord { simulator_id }
            }
            AdapterTargetCandidate::NonSimulator { kind, endpoint } => {
                return Err(DispatchAdapterError::NonSimulatorTargetRefused {
                    kind: kind.label(),
                    endpoint,
                });
            }
        };
        Ok(Self {
            gates,
            transport,
            keyring,
            criterion,
            target: accepted,
        })
    }

    /// The recorded simulator identity.
    pub fn simulator_id(&self) -> &str {
        &self.target.simulator_id
    }

    /// Loads a command view or reports `NotFound`.
    fn load(
        &self,
        ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<CommandGateView, DispatchAdapterError> {
        self.gates
            .store
            .load_command(ctx, command_id)
            .map_err(DispatchAdapterError::Store)?
            .ok_or(DispatchAdapterError::NotFound {
                command_id: command_id.to_string(),
            })
    }

    /// The controlled clock's current instant as an `EventTime`.
    fn event_time(&self) -> Result<EventTime, DispatchAdapterError> {
        EventTime::from_unix_millis(self.gates.clock.now_unix_millis()).map_err(|error| {
            DispatchAdapterError::InvalidTimestamp {
                detail: format!("the controlled clock instant is not representable: {error}"),
            }
        })
    }

    /// Persists one transition through the store seam.
    fn persist(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransition,
    ) -> Result<flexcore_ports::state_change::StateChangeReceipt, DispatchAdapterError> {
        self.gates
            .store
            .journal_transition(ctx, transition)
            .map_err(DispatchAdapterError::Store)
    }

    /// Emits one frozen dispatch-lifecycle event through the transactional
    /// outbox (state change first, then the outbox write — §6.3 step 1).
    fn emit_event(
        &self,
        state_change: flexcore_ports::state_change::StateChangeReceipt,
        event: DomainEvent,
        correlation: FlexUuid,
    ) -> Result<(), DispatchAdapterError> {
        emit(
            state_change,
            event,
            correlation,
            self.event_time()?,
            self.gates.encoder,
            self.gates.outbox,
        )
        .map_err(|error| DispatchAdapterError::Event {
            detail: error.to_string(),
        })
    }

    /// The frozen dispatch-payload scope of the command.
    fn emit_for_command(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        state_change: flexcore_ports::state_change::StateChangeReceipt,
        event: DomainEvent,
    ) -> Result<(), DispatchAdapterError> {
        let correlation = command.command_id().uuid();
        self.emit_event(state_change, event, correlation)
    }

    // ------------------------------------------------------------------
    // Issue + acknowledgement (§14.1 edges 9, 13, 14)
    // ------------------------------------------------------------------

    /// Issues an APPROVED command to the SIMULATOR and records the
    /// acknowledgement consequence (ledger P4.16 steps 2–3).
    ///
    /// Guards, in order — every refusal blocks the transition with its
    /// named error and leaves the command's state unchanged:
    ///
    /// 1. state APPROVED (store-authoritative);
    /// 2. the P4.14 issue refusals — unsigned, unsequenced, expired;
    /// 3. the P4.14 issue re-validation over the FULL §14.3 record
    ///    (`CommandRecordFields::validate_issue`), with the caller's P4.13
    ///    authorization record cross-checked against the journal's
    ///    APPROVED evidence;
    /// 4. the P4.12 issue hook (`ensure_command_issueable`): the
    ///    feasibility evidence must bind the ACTIVE model hash (§17.5
    ///    invariant 3).
    ///
    /// Then: the simulation keyring signs the command (D-P4.16-1), the
    /// signed/sequenced envelope is recorded on the ISSUED journal row,
    /// `DispatchIssued` is emitted, and the envelope is delivered through
    /// the transport. Acceptance → ISSUED → ACKNOWLEDGED with
    /// `DispatchAcknowledged`; NACK → `FailureReason::Nack`, transport
    /// timeout → `FailureReason::TransportTimeout`, other transport
    /// failures → `FailureReason::TransportFailure` — both → FAILED with
    /// `DispatchFailed`. Acknowledgement is application-layer ACCEPTANCE,
    /// never physical delivery (§14.1).
    pub fn issue(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        authorization: &crate::command_gates::AuthorizationDecisionRecord,
        actor: &Actor,
    ) -> Result<IssueOutcome, DispatchAdapterError> {
        let command_id = command.command_id();
        let view = self.load(ctx, command_id)?;
        if view.state() != CommandState::Approved {
            return Err(DispatchAdapterError::WrongState {
                current: view.state(),
                operation: "issue",
            });
        }

        // 2. The refusals (unsigned / unsequenced / expired, ledger order).
        let refusals = IssueRefusalInput {
            authenticity: command.authenticity(),
            sequence: command.sequence(),
            valid_until_millis: view.valid_until_millis,
            now_unix_millis: self.gates.clock.now_unix_millis(),
        };
        check_issue_refusals(&refusals).map_err(|refusal| match refusal {
            IssueRefusal::Unsigned => DispatchAdapterError::UnsignedCommandRefused {
                command_id: command_id.to_string(),
            },
            IssueRefusal::Unsequenced => DispatchAdapterError::UnsequencedCommandRefused {
                command_id: command_id.to_string(),
            },
            IssueRefusal::Expired {
                valid_until_millis,
                now_unix_millis,
            } => DispatchAdapterError::ExpiredCommandRefused {
                command_id: command_id.to_string(),
                valid_until_millis,
                now_unix_millis,
            },
        })?;

        // 3. The P4.14 issue re-validation + the authorization cross-check.
        let journal_authorization = view
            .journal
            .iter()
            .find(|entry| entry.to == CommandState::Approved)
            .and_then(|entry| entry.evidence.as_ref())
            .ok_or_else(|| DispatchAdapterError::Record(CommandRecordError::InconsistentJournal {
                command_id: command_id.to_string(),
                detail: "no APPROVED evidence in the journal of an APPROVED command".to_string(),
            }))?;
        if journal_authorization.as_str() != authorization.decision_reference {
            return Err(DispatchAdapterError::AuthorizationRecordMismatch {
                command_id: command_id.to_string(),
                journal_reference: journal_authorization.as_str().to_string(),
                record_reference: authorization.decision_reference.clone(),
            });
        }
        let authorization_fields = AuthorizationRecordFields {
            decision_reference: authorization.decision_reference.clone(),
            approved_scope: authorization.approved_scope.clone(),
            approved_limits: authorization.approved_limits.clone(),
            expiry_unix_millis: authorization.expiry_unix_millis,
        };
        let record = CommandRecordFields::from_command(command, &authorization_fields)?;
        record.validate_issue(self.gates.clock.now_unix_millis())?;

        // 4. The P4.12 issue hook: the feasibility evidence must bind the
        //    ACTIVE model version's hash (§17.5 invariant 3).
        let grid_model_ref = command.evidence().grid_model.clone();
        let active_hash = match self.gates.models.status(ctx, &grid_model_ref) {
            Ok(ModelVersionGateStatus::Active { content_hash }) => content_hash,
            Ok(ModelVersionGateStatus::NotActive { state }) => {
                return Err(DispatchAdapterError::ModelNotActive {
                    grid_model_ref,
                    state,
                });
            }
            Ok(ModelVersionGateStatus::Unknown) => {
                return Err(DispatchAdapterError::ModelUnknown { grid_model_ref });
            }
            Err(error) => {
                return Err(DispatchAdapterError::Record(
                    CommandRecordError::InconsistentJournal {
                        command_id: command_id.to_string(),
                        detail: format!("the model-version seam failed: {}", error.detail),
                    },
                ));
            }
        };
        self.gates
            .ensure_command_issueable(ctx, command_id, &active_hash)
            .map_err(|error| DispatchAdapterError::Record(
                CommandRecordError::InconsistentJournal {
                    command_id: command_id.to_string(),
                    detail: format!("the P4.12 issue hook refused: {error}"),
                },
            ))?;

        // Sign (D-P4.16-1) and record the ISSUED transition (evidence =
        // the §14.3 field-15 signature reference).
        let signature_reference = self.keyring.sign_command(command_id);
        let mut lifecycle = replay_full_lifecycle(command_id, &view)?;
        let issued_transition = lifecycle.record_issue(
            EvidenceReference::new(signature_reference.clone())?,
            actor.clone(),
            self.gates.clock,
        )?;
        let receipt = self.persist(ctx, &issued_transition)?;
        self.emit_for_command(
            ctx,
            command,
            receipt,
            DomainEvent::DispatchIssued(DispatchIssuedPayload {
                correlation_id: command_id.uuid(),
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                command_id: command_id.uuid(),
                event_time: self.event_time()?,
            }),
        )?;

        // Deliver to the SIMULATOR (the only target) and record the
        // acknowledgement consequence.
        let envelope = SimulatorDispatchEnvelope {
            command_id,
            asset_id: command.scope().asset_id,
            sequence: command.sequence(),
            target: requested_value(command.target())?,
            signature_reference: signature_reference.clone(),
            transport_identity: format!(
                "{SIMULATOR_TRANSPORT_IDENTITY_PREFIX}/{}",
                self.keyring.active_key_id()
            ),
        };
        match self.transport.deliver(&envelope) {
            Ok(SimulatedAcceptance::Accepted {
                acceptance_reference,
            }) => {
                let acceptance = EvidenceReference::new(acceptance_reference.clone())?;
                let acknowledged = lifecycle.record_acknowledgement(
                    acceptance,
                    actor.clone(),
                    self.gates.clock,
                )?;
                let receipt = self.persist(ctx, &acknowledged)?;
                self.emit_for_command(
                    ctx,
                    command,
                    receipt,
                    DomainEvent::DispatchAcknowledged(DispatchAcknowledgedPayload {
                        correlation_id: command_id.uuid(),
                        tenant_id: ctx.tenant_id(),
                        portfolio_id: command.scope().portfolio_id,
                        site_id: command.scope().site_id,
                        asset_id: command.scope().asset_id,
                        command_id: command_id.uuid(),
                        event_time: self.event_time()?,
                    }),
                )?;
                Ok(IssueOutcome {
                    issued: issued_transition,
                    acknowledgement: AssetAcknowledgement::new(
                        command_id,
                        command.scope().asset_id,
                        AssetAcknowledgementState::Accepted,
                        Some(acceptance_reference),
                        Some(acknowledged.at_unix_millis),
                    )?,
                    acknowledged: Some(acknowledged),
                    failed: None,
                })
            }
            Ok(SimulatedAcceptance::Nack { reason: _ }) => {
                let failed = lifecycle.record_failure(
                    FailureReason::Nack,
                    actor.clone(),
                    self.gates.clock,
                )?;
                let receipt = self.persist(ctx, &failed)?;
                self.emit_failed(ctx, command, receipt)?;
                Ok(IssueOutcome {
                    issued: issued_transition,
                    acknowledgement: AssetAcknowledgement::new(
                        command_id,
                        command.scope().asset_id,
                        AssetAcknowledgementState::Refused,
                        None,
                        Some(failed.at_unix_millis),
                    )?,
                    acknowledged: None,
                    failed: Some(failed),
                })
            }
            Err(transport_error) => {
                let reason = match transport_error.failure {
                    SimulatorTransportFailure::Timeout => FailureReason::TransportTimeout,
                    SimulatorTransportFailure::Unreachable
                    | SimulatorTransportFailure::SimulatorError => FailureReason::TransportFailure,
                };
                let failed =
                    lifecycle.record_failure(reason, actor.clone(), self.gates.clock)?;
                let receipt = self.persist(ctx, &failed)?;
                self.emit_failed(ctx, command, receipt)?;
                let state = match transport_error.failure {
                    SimulatorTransportFailure::Timeout => AssetAcknowledgementState::TimedOut,
                    _ => AssetAcknowledgementState::TransportFailed,
                };
                Ok(IssueOutcome {
                    issued: issued_transition,
                    acknowledgement: AssetAcknowledgement::new(
                        command_id,
                        command.scope().asset_id,
                        state,
                        None,
                        Some(failed.at_unix_millis),
                    )?,
                    acknowledged: None,
                    failed: Some(failed),
                })
            }
        }
    }

    /// Emits `DispatchFailed` (the frozen event for FAILED).
    fn emit_failed(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        state_change: flexcore_ports::state_change::StateChangeReceipt,
    ) -> Result<(), DispatchAdapterError> {
        self.emit_for_command(
            ctx,
            command,
            state_change,
            DomainEvent::DispatchFailed(DispatchFailedPayload {
                correlation_id: command.command_id().uuid(),
                tenant_id: ctx.tenant_id(),
                portfolio_id: command.scope().portfolio_id,
                site_id: command.scope().site_id,
                asset_id: command.scope().asset_id,
                command_id: command.command_id().uuid(),
                event_time: self.event_time()?,
            }),
        )
    }

    // ------------------------------------------------------------------
    // Tracking (§14.1 edges 18, 22, 23, 24)
    // ------------------------------------------------------------------

    /// Pulls the simulator's observations and advances tracking (ledger
    /// P4.16 step 4). Applies ONLY from ACKNOWLEDGED or TRACKING.
    ///
    /// Order of effects:
    ///
    /// 1. the validity window ended → the domain engine records
    ///    ACKNOWLEDGED → EXPIRED (`BeforeTrackingCompletion`) or TRACKING →
    ///    EXPIRED (`BeforeCompletion`); `DispatchExpired` is emitted;
    /// 2. from ACKNOWLEDGED, at least one observation is REQUIRED (edge
    ///    18: "telemetry observed") — none is a named error, no
    ///    transition; the first observation moves ACKNOWLEDGED → TRACKING
    ///    (`DispatchTrackingUpdated` is emitted — the frozen event for a
    ///    tracking update; the payload gap note applies);
    /// 3. further observations are pulled (bounded by
    ///    [`MAX_OBSERVATIONS_PER_POLL`]) and the acceptance criterion is
    ///    evaluated (D-P4.16-3 order: deviation → FAILED, satisfied →
    ///    COMPLETED, timeout → FAILED, else keep tracking).
    pub fn poll_tracking(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        actor: &Actor,
    ) -> Result<TrackingOutcome, DispatchAdapterError> {
        let command_id = command.command_id();
        let view = self.load(ctx, command_id)?;
        if !matches!(
            view.state(),
            CommandState::Acknowledged | CommandState::Tracking
        ) {
            return Err(DispatchAdapterError::WrongState {
                current: view.state(),
                operation: "poll_tracking",
            });
        }
        let mut lifecycle = replay_full_lifecycle(command_id, &view)?;
        let now = self.gates.clock.now_unix_millis();

        // 1. Validity ended → EXPIRED (milestone matched to the state).
        if now >= view.valid_until_millis {
            let milestone = expiry_milestone_for(view.state())
                .expect("checked above: the state has a frozen EXPIRED edge");
            let transition = lifecycle.record_expiry(milestone, actor.clone(), self.gates.clock)?;
            let receipt = self.persist(ctx, &transition)?;
            self.emit_for_command(
                ctx,
                command,
                receipt,
                DomainEvent::DispatchExpired(DispatchExpiredPayload {
                    correlation_id: command_id.uuid(),
                    tenant_id: ctx.tenant_id(),
                    portfolio_id: command.scope().portfolio_id,
                    site_id: command.scope().site_id,
                    asset_id: command.scope().asset_id,
                    command_id: command_id.uuid(),
                    event_time: self.event_time()?,
                }),
            )?;
            return Ok(TrackingOutcome {
                tracking_started: None,
                observations: 0,
                verdict: None,
                terminal: Some(TrackingTerminal::Expired { transition }),
            });
        }

        // 2. Pull observations; entering TRACKING requires one (edge 18).
        let mut observations: Vec<SimulatedObservation> = Vec::new();
        let mut tracking_started: Option<CommandTransition> = None;
        if view.state() == CommandState::Acknowledged {
            let first = self
                .transport
                .observe(command.scope().asset_id, now)
                .map_err(DispatchAdapterError::Transport)?;
            let first = first.ok_or(DispatchAdapterError::NoTelemetryObserved {
                command_id: command_id.to_string(),
            })?;
            observations.push(first);
            let telemetry_reference =
                EvidenceReference::new(format!("telemetry:command/{command_id}@{now}"))?;
            let started = lifecycle.record_tracking_started(
                telemetry_reference,
                actor.clone(),
                self.gates.clock,
            )?;
            let receipt = self.persist(ctx, &started)?;
            self.emit_for_command(
                ctx,
                command,
                receipt,
                DomainEvent::DispatchTrackingUpdated(DispatchTrackingUpdatedPayload {
                    correlation_id: command_id.uuid(),
                    tenant_id: ctx.tenant_id(),
                    portfolio_id: command.scope().portfolio_id,
                    site_id: command.scope().site_id,
                    asset_id: command.scope().asset_id,
                    command_id: command_id.uuid(),
                    event_time: self.event_time()?,
                }),
            )?;
            tracking_started = Some(started);
        }

        // 3. Pull further observations (bounded; possibly none on a later
        //    poll), then evaluate the acceptance criterion.
        while observations.len() < MAX_OBSERVATIONS_PER_POLL {
            match self
                .transport
                .observe(command.scope().asset_id, now)
                .map_err(DispatchAdapterError::Transport)?
            {
                Some(observation) => observations.push(observation),
                None => break,
            }
        }
        self.evaluate_and_terminate(
            ctx,
            command,
            &mut lifecycle,
            observations,
            now,
            actor,
            tracking_started,
        )
    }

    /// Evaluates the acceptance criterion over this poll's observations
    /// and records the terminal transition when one fires.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_and_terminate(
        &self,
        ctx: &TenantContext,
        command: &DispatchCommand,
        lifecycle: &mut flexcore_domain::command_lifecycle::CommandLifecycle,
        observations: Vec<SimulatedObservation>,
        now: i64,
        actor: &Actor,
        tracking_started: Option<CommandTransition>,
    ) -> Result<TrackingOutcome, DispatchAdapterError> {
        let command_id = command.command_id();
        // The criterion's quantity must match the command's target quantity
        // AND every observation's quantity — fail-closed, never coerced.
        let (target_quantity, requested) = requested_tracked(command.target())?;
        if target_quantity != self.criterion.quantity {
            return Err(DispatchAdapterError::ObservationMismatch {
                command_id: command_id.to_string(),
                criterion_quantity: self.criterion.quantity.label(),
                observation_quantity: target_quantity.label(),
            });
        }
        for observation in &observations {
            if observation.quantity != self.criterion.quantity {
                return Err(DispatchAdapterError::ObservationMismatch {
                    command_id: command_id.to_string(),
                    criterion_quantity: self.criterion.quantity.label(),
                    observation_quantity: observation.quantity.label(),
                });
            }
        }

        if observations.is_empty() {
            return Ok(TrackingOutcome {
                tracking_started,
                observations: 0,
                verdict: None,
                terminal: None,
            });
        }

        // The tracking-start timestamp is the → TRACKING journal entry (the
        // evidence of record), not an adapter-side guess.
        let started = lifecycle
            .journal()
            .iter()
            .find(|entry| entry.to == CommandState::Tracking)
            .map(|entry| entry.at_unix_millis)
            .ok_or_else(|| {
                DispatchAdapterError::Record(CommandRecordError::InconsistentJournal {
                    command_id: command_id.to_string(),
                    detail: "no → TRACKING entry in the journal of a TRACKING command"
                        .to_string(),
                })
            })?;
        let verdict = evaluate_tracking(
            &self.criterion,
            requested,
            &observations,
            started,
            now,
        );
        let terminal = match verdict {
            TrackingVerdict::KeepTracking { .. } => None,
            TrackingVerdict::Completed { .. } => {
                let evidence = EvidenceReference::new(format!(
                    "tracking:command/{command_id}@{now}:criterion-satisfied"
                ))?;
                let transition =
                    lifecycle.record_completion(evidence, actor.clone(), self.gates.clock)?;
                let receipt = self.persist(ctx, &transition)?;
                self.emit_for_command(
                    ctx,
                    command,
                    receipt,
                    DomainEvent::DispatchCompleted(DispatchCompletedPayload {
                        correlation_id: command_id.uuid(),
                        tenant_id: ctx.tenant_id(),
                        portfolio_id: command.scope().portfolio_id,
                        site_id: command.scope().site_id,
                        asset_id: command.scope().asset_id,
                        command_id: command_id.uuid(),
                        event_time: self.event_time()?,
                    }),
                )?;
                Some(TrackingTerminal::Completed { transition })
            }
            TrackingVerdict::Deviation { .. } => {
                let transition = lifecycle.record_failure(
                    FailureReason::TrackingDeviation,
                    actor.clone(),
                    self.gates.clock,
                )?;
                let receipt = self.persist(ctx, &transition)?;
                self.emit_failed(ctx, command, receipt)?;
                Some(TrackingTerminal::Failed {
                    transition,
                    reason: FailureReason::TrackingDeviation,
                })
            }
            TrackingVerdict::Timeout => {
                let transition = lifecycle.record_failure(
                    FailureReason::TrackingTimeout,
                    actor.clone(),
                    self.gates.clock,
                )?;
                let receipt = self.persist(ctx, &transition)?;
                self.emit_failed(ctx, command, receipt)?;
                Some(TrackingTerminal::Failed {
                    transition,
                    reason: FailureReason::TrackingTimeout,
                })
            }
        };
        Ok(TrackingOutcome {
            tracking_started,
            observations: observations.len(),
            verdict: Some(verdict),
            terminal,
        })
    }
}

/// Extracts the requested value from the command's target (canonical
/// units; the sign convention travels on the command's declared
/// conventions).
fn requested_value(target: &ControlTarget) -> Result<SimulatorTargetValue, DispatchAdapterError> {
    requested_tracked(target).map(|(quantity, value)| match quantity {
        TrackedQuantity::ActivePowerMW => SimulatorTargetValue::ActivePowerMW(value),
        TrackedQuantity::ReactivePowerMVAr => SimulatorTargetValue::ReactivePowerMVAr(value),
        TrackedQuantity::SocFraction => SimulatorTargetValue::SocFraction(value),
    })
}

/// Extracts the tracked quantity + requested value from the command's
/// target. `PowerSetpoint` always tracks its active power (the reactive
/// component, when present, is recorded on the command but not separately
/// tracked — a recorded limitation, decision D-P4.16-3).
fn requested_tracked(
    target: &ControlTarget,
) -> Result<(TrackedQuantity, f64), DispatchAdapterError> {
    Ok(match target {
        ControlTarget::PowerSetpoint { p, .. } => (TrackedQuantity::ActivePowerMW, p.value()),
        ControlTarget::SupportedParameter { value, .. } => match value {
            ControlParameterValue::ActivePower(p) => (TrackedQuantity::ActivePowerMW, p.value()),
            ControlParameterValue::ReactivePower(q, _) => {
                (TrackedQuantity::ReactivePowerMVAr, q.value())
            }
            ControlParameterValue::SocFraction(soc) => (TrackedQuantity::SocFraction, soc.value()),
        },
    })
}
