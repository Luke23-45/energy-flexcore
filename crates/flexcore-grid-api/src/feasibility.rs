// P4.04 + P4.05 — The feasibility-gate pipeline, steps 1–4 (report §12.5;
// ledger P4.04, P4.05). Decision record:
// `ledger/phase-04-grid-feasibility-dispatch/03_05_feasibility_gate.md`.
//
// ```text
// step 1  candidate intake + validation      (BEFORE any model is touched)
// step 2  per-asset injection derivation     (capability + ramp, quality-gated)
// step 3  apply injections to the versioned model   (health-gated, hash-verified)
// step 4  run the AC power flow through the port    (evidence, never a verdict)
// ```
//
// SOLVER-CLASS DISCIPLINE (absolute; AGENTS.md rule, §12.5): step 4 runs an
// **AC power-flow evaluation of specified injections and controls**. It is
// NOT an AC-OPF optimization and nothing in this pipeline optimizes. This
// is a power-flow CHECK of a candidate; the terms are never interchanged.
//
// EVIDENCE DISCIPLINE (§12.6 pattern; ledger P3.21 precedent): this
// pipeline produces EVIDENCE, never a feasibility verdict. Every
// rejection, abort, and inconclusive outcome is a recorded evidence row
// naming the step, the field/asset, and the rule — never a bare error.
// The reject/constrain/return policy (§12.5 step 8) and the limit checks
// (§12.5 step 5) are P4.06/P4.07's; nothing here claims a pass.
//
// FAIL-CLOSED DISCIPLINE (§12.5 final paragraph; §17.5 invariant 9):
// - a stale/incomplete/uncovered model refuses the run (step 3, P4.03);
// - a mid-gate model change (live content hash ≠ recorded hash) aborts;
// - non-convergence is an in-band INCONCLUSIVE — never a pass, never a
//   rejection-by-limit (P4.07 owns the verdict category), and the retry
//   policy is NONE (ledger P4.05 risk note);
// - a BAD/MISSING/STALE ramp baseline blocks the ramp check (the P4.08
//   pre-wiring; see `INJECTION_QUALITY_POLICY`);
// - every injected source seam failing or answering "unknown" rejects —
//   unknown is never "usable" (the P3.13 revision-check pattern).
//
// THE INJECTED SEAMS (recorded; grid-api depends only on flexcore-contracts
// + serde/serde_json/time, enforced by `tests/dependency_direction.rs`, so
// the Phase 3 stores are reached through caller-supplied closures — the
// composition root wires the real stores):
// 1. `capability_bounds` — per asset + interval, the DECLARED bounds. The
//    caller supplies bounds already validated by the P3.04 validator
//    (`flexcore_domain::capability_validate::validate_capability_model` —
//    REUSED, not reimplemented; the P3.01 `CapabilityModel` interval
//    constraints are the source of the numbers).
// 2. `envelope_freshness` — the P3.13 staleness gates
//    (`assert_usable_for_new_schedule` / `check_schedule_envelope_reference`
//    in `flexcore_application::envelope_staleness`), mirrored as
//    [`EnvelopeFreshness`].
// 3. `revision_superseded` — the P3.22 revision check, mirroring
//    `ScheduleRevisionChainStore::is_revision_superseded` (flexcore-storage):
//    `Some(true)` superseded / `Some(false)` active / `None` unknown. BOTH
//    superseded and unknown are rejected; `None` is never "usable".
// 4. `ramp_baseline` — the latest telemetered state for an asset WITH its
//    quality flag (the Phase 2 telemetry store projection).
//
// Control status: simulation-only 0.x line. This pipeline evaluates a
// CANDIDATE against a declared model in simulation; nothing here touches a
// device, authorizes field control, or claims a safe/certified outcome.

use crate::model_health::{
    assert_usable_for_feasibility, check_model_health, ModelHealthConfig, ModelVersionRegistry,
    RegistryElement, VersionedModel,
};
use crate::port::{
    CandidateInjection, GridModelProvider, GridPortError, NetworkLimits, PowerFlowResult,
    PowerFlowStatus,
};
use crate::translation::{candidate_to_backend, injection_from_backend};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::ReactiveSignConvention;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Recorded statements and policies
// ---------------------------------------------------------------------------

/// The solver-class statement carried on every attempt record.
pub const AC_POWER_FLOW_STATEMENT: &str = "Step 4 runs an AC power-flow \
     evaluation of specified injections and controls. It is NOT an AC-OPF \
     optimization; the gate never optimizes (§12.5, §7.4).";

/// The unbalanced/asymmetric limitation note carried on every attempt
/// record (§12.5 step 6; D3 OPEN — the balanced default is a
/// recommendation). The scope-declared limitation also travels on every
/// `PowerFlowResult` as a model-coverage limitation.
pub const UNBALANCED_DEFERRAL_NOTE: &str = "The declared model scope does NOT \
     model unbalanced three-phase asymmetry (balanced-radial default per the \
     recommended default; D3 OPEN — unbalanced enters from v0.4.0). This \
     limitation surfaces in the result's coverage limitations; the \
     unbalanced three-phase track is deferred to P4.34.";

/// The retry policy (ledger P4.05 risk note): non-convergence is a result,
/// never retried with different settings until it passes.
pub const NO_RETRY_POLICY: &str = "Retry policy: NONE. Non-convergence is \
     reported as an in-band inconclusive result with diagnostics and is \
     never silently retried with altered settings, never treated as \
     feasible (§12.5 step 6; ledger P4.05). No named, reviewed retry policy \
     exists; adding one is a recorded decision, not a code change.";

/// The injection-quality policy (D-P4.04-3; the P4.08 pre-wiring): the ramp
/// baseline is the latest telemetered state WITH its quality flag. Accepted:
/// `GOOD`, `SUSPECT`, `ESTIMATED`. Blocked (fail-closed): `BAD`, `MISSING`,
/// `STALE` — exactly the P4.08 fail-closed trigger enumeration. The quality
/// is recorded alongside every derived injection; a flag is never silently
/// converted into a normal reading (§10.4).
pub const INJECTION_QUALITY_POLICY: &str = "Ramp baseline quality policy: \
     GOOD, SUSPECT, and ESTIMATED baselines are accepted; BAD, MISSING, and \
     STALE baselines BLOCK the ramp check fail-closed (the P4.08 trigger \
     enumeration). The baseline quality is recorded alongside every derived \
     injection.";

/// The seam statement (recorded; see the module header).
pub const GATE_SEAM_STATEMENT: &str = "flexcore-grid-api depends only on \
     flexcore-contracts + serde/serde_json/time (the dependency-direction \
     test pins the set), so the Phase 3 sources are injected closures wired \
     by the composition root: capability bounds (P3.04-validated capability \
     data), envelope freshness (the P3.13 gates), revision superseded (the \
     P3.22 ScheduleRevisionChainStore check), and the ramp baseline (the \
     Phase 2 telemetry store, with its quality flag). A seam error or an \
     'unknown' answer rejects — unknown is never usable.";

// ---------------------------------------------------------------------------
// Steps, rules, evidence
// ---------------------------------------------------------------------------

/// The pipeline step an evidence row or rejection belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStep {
    /// Candidate intake + validation (P4.04 step 1).
    Step1CandidateValidation,
    /// Per-asset injection derivation (P4.04 step 2).
    Step2InjectionDerivation,
    /// Apply injections to the versioned model (P4.05 step 3).
    Step3ModelApplication,
    /// Run the AC power flow through the port (P4.05 step 4).
    Step4PowerFlow,
}

impl GateStep {
    /// The canonical step name for records.
    pub const fn as_str(self) -> &'static str {
        match self {
            GateStep::Step1CandidateValidation => "step_1_candidate_validation",
            GateStep::Step2InjectionDerivation => "step_2_injection_derivation",
            GateStep::Step3ModelApplication => "step_3_model_application",
            GateStep::Step4PowerFlow => "step_4_power_flow",
        }
    }
}

/// The outcome of one evidence row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    /// The named check passed (validation rows only — never a feasibility
    /// pass: a within-limits verdict is P4.06/P4.07's).
    Passed,
    /// The named check rejected the candidate.
    Rejected,
    /// A completed sub-step was recorded (e.g. a derived injection).
    Recorded,
    /// The outcome is inconclusive (non-convergence / provider error):
    /// recorded fail-closed, never a pass and never a rejection-by-limit.
    Inconclusive,
    /// The pipeline aborted (model/infrastructure condition, not a candidate
    /// fault); P4.08 owns the reject-vs-hold classification.
    Aborted,
}

/// One structured evidence row: which step, which field/asset, which rule,
/// which outcome (ledger P4.04 step 3: "structured, per-check results … a
/// single boolean 'valid' is not sufficient").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEvidenceRow {
    /// The pipeline step.
    pub step: GateStep,
    /// The field, asset, setpoint, or element the row is about.
    pub subject: String,
    /// The frozen machine-readable rule code ([`GateRule::code`]).
    pub rule_code: String,
    /// The row outcome.
    pub outcome: EvidenceOutcome,
    /// Human-readable detail (audit text; safe to log).
    pub detail: String,
}

impl GateEvidenceRow {
    fn new(
        step: GateStep,
        subject: impl Into<String>,
        rule: GateRule,
        outcome: EvidenceOutcome,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            step,
            subject: subject.into(),
            rule_code: rule.code().to_string(),
            outcome,
            detail: detail.into(),
        }
    }
}

/// The rule catalogue of gate steps 1–4. Each variant names ONE rule; the
/// code is frozen so evidence consumers can match on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateRule {
    // Step 1.
    /// A required reference or field is missing/empty (`subject` names it).
    RefMissing,
    /// A setpoint entry is incomplete (`subject` names the index/field).
    SetpointIncomplete,
    /// The candidate carries no setpoints.
    EmptySetpoints,
    /// The canonical-units check (structural: the frozen MW/MVAr contract
    /// types make kW/kWh unrepresentable at the boundary; §10.2).
    UnitsCanonical,
    /// The candidate timestamp is malformed (non-positive).
    TimestampMalformed,
    /// The candidate timestamp is in the past beyond the configured
    /// tolerance.
    TimestampExpired,
    /// `valid_from > valid_until` or a non-positive bound.
    ValidityWindowInverted,
    /// The window duration disagrees with the stated duration.
    DurationMismatch,
    /// A setpoint interval start lies outside the validity window.
    IntervalOutsideWindow,
    /// The candidate references a stale envelope (the P3.13 gate).
    EnvelopeStale,
    /// The envelope-freshness seam failed (fail-closed).
    EnvelopeCheckFailed,
    /// The referenced schedule revision is SUPERSEDED (P4.12 pre-wiring).
    RevisionSuperseded,
    /// The referenced schedule revision does not exist (unknown is never
    /// usable).
    RevisionUnknown,
    /// The revision seam failed (fail-closed).
    RevisionCheckFailed,
    // Step 2.
    /// The setpoint's bus does not exist in the model.
    BusUnresolved,
    /// No declared capability bounds for the asset at the candidate time —
    /// unknown is never unconstrained (the P3.02 rule carried forward).
    CapabilityUnavailable,
    /// The capability seam failed (fail-closed).
    CapabilityCheckFailed,
    /// The setpoint quantity exceeds the declared capability bounds.
    CapabilityExceeded,
    /// The setpoint's declared Q convention disagrees with the capability's
    /// declared convention (the Q sign is never inferred, §10.2).
    QConventionMismatch,
    /// The ramp-baseline seam failed (fail-closed).
    RampBaselineUnavailable,
    /// The baseline's telemetry quality is in the blocked set (BAD,
    /// MISSING, STALE — the P4.08 pre-wiring).
    RampBaselineBlocked,
    /// The requested change exceeds the ramp limit from the baseline.
    RampExceeded,
    /// The derived injection failed the owned translation round trip
    /// (a contract guard; never expected, always fatal when seen).
    DerivationMismatch,
    /// One derived injection (with its baseline quality) was recorded.
    DerivationRecorded,
    // Steps 3–4 (aborts and in-band outcomes; recorded as evidence).
    /// The model health check refused (stale/incomplete/uncovered).
    ModelNotHealthy,
    /// The live model content hash no longer matches the recorded hash
    /// (mid-gate change; fail-closed abort).
    ModelHashMismatch,
    /// The grid port returned an infrastructure/contract error.
    ProviderError,
    /// The power flow completed and the result was recorded (NO feasibility
    /// verdict is implied — limits are P4.06, routing is P4.07).
    PowerFlowEvaluated,
    /// Non-convergence / provider-reported error: recorded INCONCLUSIVE,
    /// fail-closed (§12.5 step 6; §17.5 invariant 9).
    PowerFlowInconclusive,
}

impl GateRule {
    /// The frozen machine-readable rule code.
    pub const fn code(self) -> &'static str {
        match self {
            GateRule::RefMissing => "REF_MISSING",
            GateRule::SetpointIncomplete => "SETPOINT_INCOMPLETE",
            GateRule::EmptySetpoints => "EMPTY_SETPOINTS",
            GateRule::UnitsCanonical => "UNITS_CANONICAL",
            GateRule::TimestampMalformed => "TIMESTAMP_MALFORMED",
            GateRule::TimestampExpired => "TIMESTAMP_EXPIRED",
            GateRule::ValidityWindowInverted => "VALIDITY_WINDOW_INVERTED",
            GateRule::DurationMismatch => "DURATION_MISMATCH",
            GateRule::IntervalOutsideWindow => "INTERVAL_OUTSIDE_WINDOW",
            GateRule::EnvelopeStale => "ENVELOPE_STALE",
            GateRule::EnvelopeCheckFailed => "ENVELOPE_CHECK_FAILED",
            GateRule::RevisionSuperseded => "REVISION_SUPERSEDED",
            GateRule::RevisionUnknown => "REVISION_UNKNOWN",
            GateRule::RevisionCheckFailed => "REVISION_CHECK_FAILED",
            GateRule::BusUnresolved => "BUS_UNRESOLVED",
            GateRule::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            GateRule::CapabilityCheckFailed => "CAPABILITY_CHECK_FAILED",
            GateRule::CapabilityExceeded => "CAPABILITY_EXCEEDED",
            GateRule::QConventionMismatch => "Q_CONVENTION_MISMATCH",
            GateRule::RampBaselineUnavailable => "RAMP_BASELINE_UNAVAILABLE",
            GateRule::RampBaselineBlocked => "RAMP_BASELINE_BLOCKED",
            GateRule::RampExceeded => "RAMP_EXCEEDED",
            GateRule::DerivationMismatch => "DERIVATION_MISMATCH",
            GateRule::DerivationRecorded => "DERIVATION_RECORDED",
            GateRule::ModelNotHealthy => "MODEL_NOT_HEALTHY",
            GateRule::ModelHashMismatch => "MODEL_HASH_MISMATCH",
            GateRule::ProviderError => "PROVIDER_ERROR",
            GateRule::PowerFlowEvaluated => "POWER_FLOW_EVALUATED",
            GateRule::PowerFlowInconclusive => "POWER_FLOW_INCONCLUSIVE",
        }
    }
}

/// A structured rejection: which step, which field/asset, which rule —
/// never a bare error (ledger P4.04 step 3). Recorded as an evidence row
/// by the pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateRejection {
    /// The step that rejected.
    pub step: GateStep,
    /// The field, asset, or setpoint the rejection is about.
    pub subject: String,
    /// The rule code.
    pub rule_code: String,
    /// Human-readable detail (audit text).
    pub detail: String,
}

impl GateRejection {
    fn from_row(row: &GateEvidenceRow) -> Self {
        Self {
            step: row.step,
            subject: row.subject.clone(),
            rule_code: row.rule_code.clone(),
            detail: row.detail.clone(),
        }
    }
}

/// A pipeline ABORT: a model or infrastructure condition that is not a
/// candidate fault. The reject-vs-hold split is P4.08's policy table; this
/// pipeline only stops and records (fail-closed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GateAbort {
    /// The model health check refused (all findings carried).
    ModelNotHealthy { findings: Vec<crate::model_health::HealthFinding> },
    /// The live model content no longer matches the recorded hash: a
    /// mid-gate model change aborts fail-closed.
    ModelHashMismatch { recorded: String, computed: String },
    /// The grid port returned an error (backend unavailable, contract
    /// violation, malformed input — the port's own fail-closed surface).
    ProviderError { detail: String },
}

impl GateAbort {
    fn rule(&self) -> GateRule {
        match self {
            GateAbort::ModelNotHealthy { .. } => GateRule::ModelNotHealthy,
            GateAbort::ModelHashMismatch { .. } => GateRule::ModelHashMismatch,
            GateAbort::ProviderError { .. } => GateRule::ProviderError,
        }
    }

    fn detail(&self) -> String {
        match self {
            GateAbort::ModelNotHealthy { findings } => {
                let joined: Vec<String> = findings.iter().map(|f| f.to_string()).collect();
                joined.join("; ")
            }
            GateAbort::ModelHashMismatch { recorded, computed } => format!(
                "recorded content hash '{recorded}' but the live model content hashes to \
                 '{computed}' — the model changed mid-gate; aborting fail-closed"
            ),
            GateAbort::ProviderError { detail } => detail.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Injected seams (see the module header for the recorded rationale)
// ---------------------------------------------------------------------------

/// The declared capability bounds for one asset at one interval, as
/// declared by the capability model (P3.01) and validated by the P3.04
/// validator. Every bound is REQUIRED: an undeclared dimension is never
/// treated as unconstrained (the P3.02 unknown rule carried forward), so
/// the source must refuse (error) rather than return absent bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclaredBounds {
    /// Active-power lower bound in MW, canonical sign (`P > 0` = export).
    pub p_min_mw: f64,
    /// Active-power upper bound in MW, canonical sign.
    pub p_max_mw: f64,
    /// Reactive-power lower bound in MVAr under `q_convention`.
    pub q_min_mvar: f64,
    /// Reactive-power upper bound in MVAr under `q_convention`.
    pub q_max_mvar: f64,
    /// The DECLARED reactive sign convention of the capability model
    /// (§10.2 — never assumed; the setpoint must declare the SAME one).
    pub q_convention: ReactiveSignConvention,
    /// Ramp rate in MW per minute (`ramp ≥ 0`, the §11.1 rule).
    pub ramp_mw_per_min: f64,
}

impl DeclaredBounds {
    /// Validates the bounds: ordered, finite, ramp non-negative (the
    /// §11.1/P3.04 rules the gate re-checks at the seam boundary — the
    /// source validated them with the P3.04 validator; the gate refuses
    /// structurally impossible bounds rather than comparing against them).
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("p_min_mw", self.p_min_mw),
            ("p_max_mw", self.p_max_mw),
            ("q_min_mvar", self.q_min_mvar),
            ("q_max_mvar", self.q_max_mvar),
            ("ramp_mw_per_min", self.ramp_mw_per_min),
        ] {
            if !value.is_finite() {
                return Err(format!("bound '{name}' is not finite"));
            }
        }
        if self.p_min_mw > self.p_max_mw {
            return Err(format!(
                "p_min_mw {} exceeds p_max_mw {}",
                self.p_min_mw, self.p_max_mw
            ));
        }
        if self.q_min_mvar > self.q_max_mvar {
            return Err(format!(
                "q_min_mvar {} exceeds q_max_mvar {}",
                self.q_min_mvar, self.q_max_mvar
            ));
        }
        if self.ramp_mw_per_min < 0.0 {
            return Err("ramp_mw_per_min is negative".to_string());
        }
        Ok(())
    }
}

/// The envelope-freshness answer, mirroring the P3.13 staleness verdict
/// (`flexcore_application::envelope_staleness::EnvelopeFreshness` — the
/// composition root converts; grid-api cannot name that crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeFreshness {
    /// The envelope is fresh at the evaluation time.
    Fresh,
    /// The envelope is stale; the recorded reason travels (a `StalenessReason`
    /// rendering from the P3.13 module).
    Stale { reason: String },
}

/// The schedule-revision state, mirroring
/// `ScheduleRevisionChainStore::is_revision_superseded` (flexcore-storage):
/// `Active` = `Some(false)`, `Superseded` = `Some(true)`, `Unknown` =
/// `None`. Superseded AND unknown are rejected; `None` is never "usable".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionState {
    /// The revision exists and is the active one.
    Active,
    /// The revision exists but is superseded by a newer revision.
    Superseded,
    /// The revision does not exist in the caller's tenant scope.
    Unknown,
}

/// The latest telemetered state of one asset, WITH its quality flag
/// (§10.4 — exactly one quality state per value; the flag travels with the
/// value and is never silently converted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemeteredState {
    /// Telemetered active power in MW, canonical sign (`P > 0` = export).
    pub active_power_mw: f64,
    /// The value's canonical quality state (§10.4).
    pub quality: TelemetryQuality,
    /// UTC millis the value was observed at (§10.3).
    pub observed_at_millis: i64,
}

/// Errors the injected seams may return. Any error is fail-closed: the
/// corresponding check REJECTS (it never degrades to a pass or a skip).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeamError {
    /// Which seam failed (`capability_bounds`, `envelope_freshness`,
    /// `revision_superseded`, `ramp_baseline`).
    pub seam: &'static str,
    /// The recorded failure detail (audit text).
    pub detail: String,
}

/// The injected source seams (see the module header and
/// [`GATE_SEAM_STATEMENT`]). All closures are sync; the composition root
/// wires the real Phase 2/3 stores.
#[derive(Clone)]
pub struct GateSources {
    /// Per asset id + interval-start millis, the declared capability bounds
    /// (P3.04-validated data).
    pub capability_bounds:
        Arc<dyn Fn(&str, i64) -> Result<DeclaredBounds, SeamError> + Send + Sync>,
    /// Per envelope version, the P3.13 freshness verdict.
    pub envelope_freshness:
        Arc<dyn Fn(&str) -> Result<EnvelopeFreshness, SeamError> + Send + Sync>,
    /// Per schedule id + revision, the P3.22 revision state.
    pub revision_superseded:
        Arc<dyn Fn(&str, u64) -> Result<RevisionState, SeamError> + Send + Sync>,
    /// Per asset id, the latest telemetered state with its quality flag.
    pub ramp_baseline: Arc<dyn Fn(&str) -> Result<TelemeteredState, SeamError> + Send + Sync>,
}

impl std::fmt::Debug for GateSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GateSources { capability_bounds, envelope_freshness, revision_superseded, ramp_baseline }")
    }
}

// ---------------------------------------------------------------------------
// The candidate
// ---------------------------------------------------------------------------

/// The Phase 3 candidate as the gate consumes it (ledger P4.04: "a Phase 3
/// candidate (schedule ID + revision + envelope version + result-record
/// snapshot ref)"). The Phase 3 frozen types
/// (`flexcore_domain::scheduling_time::ScheduleRevisionRef`,
/// `flexcore_application::flexibility_envelope::EnvelopeVersion`,
/// `flexcore_application::optimization_result_record::ScheduleResultRecord`)
/// are converted to these validated strings by the composition root —
/// grid-api cannot name those crates (the seam statement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeasibilityCandidate {
    /// The candidate identifier (the §12.6 output candidate id text).
    pub candidate_id: String,
    /// The schedule chain id (P3.22, canonical UUID text).
    pub schedule_id: String,
    /// The schedule revision number (monotonic per chain, first = 1).
    pub revision: u64,
    /// The envelope version the schedule was computed against (P3.12/13).
    pub envelope_version: String,
    /// The §12.6 result-record snapshot reference the candidate derives
    /// from (P3.21 `InputSnapshotRecord::reference()` output, or the
    /// persisted record reference).
    pub result_record_snapshot_ref: String,
    /// UTC millis the candidate was created (§10.3). Checked against the
    /// configured past-tolerance.
    pub created_at_millis: i64,
    /// Validity window start, inclusive (UTC millis).
    pub valid_from_millis: i64,
    /// Validity window end, inclusive (UTC millis).
    pub valid_until_millis: i64,
    /// The stated duration in millis; must equal
    /// `valid_until - valid_from` (ledger P4.04 step 1).
    pub stated_duration_millis: i64,
    /// The candidate setpoints (one per asset per interval).
    pub setpoints: Vec<CandidateSetpoint>,
}

/// One candidate setpoint for one asset at one interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateSetpoint {
    /// The setpoint identifier (unique within the candidate).
    pub setpoint_id: String,
    /// The asset the setpoint commands (registry id text).
    pub asset_id: String,
    /// The model bus the asset's injection applies at.
    pub bus_id: String,
    /// Active power in MW, canonical sign (`P > 0` = export) — the frozen
    /// contract type makes kW/kWh and non-canonical spellings
    /// unrepresentable at the boundary (§10.2; the P3.04 unit rule).
    pub active_power: ActivePowerMW,
    /// Reactive power in MVAr, interpreted ONLY under the declared
    /// convention (§10.2).
    pub reactive_power: ReactivePowerMVAr,
    /// The DECLARED reactive sign convention (no default, §10.2).
    pub reactive_sign_convention: ReactiveSignConvention,
    /// The interval start (UTC millis), inside the validity window.
    pub interval_start_millis: i64,
}

/// The step-1 configuration (ledger P4.04 step 1: "timestamp (not in the
/// past beyond a configured tolerance ...)").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateValidationConfig {
    /// How far in the past the candidate's creation timestamp may lie,
    /// in millis. `age == tolerance` is accepted; `age > tolerance` rejects
    /// (the recorded staleness convention). Default: 5 minutes
    /// (CONFIGURATION, recorded pending the P4.08 review).
    pub max_timestamp_age_millis: i64,
}

impl Default for CandidateValidationConfig {
    fn default() -> Self {
        // CONFIGURATION default (recorded): a five-minute past tolerance.
        // ASSUMPTION pending the P4.08 policy-table review.
        Self {
            max_timestamp_age_millis: 300_000,
        }
    }
}

// ---------------------------------------------------------------------------
// Step 1 — candidate intake + validation (BEFORE any model is touched)
// ---------------------------------------------------------------------------

/// Validates the candidate (§12.5 step 1; ledger P4.04 step 1): refs
/// present and resolvable, units canonical (enforced by the frozen contract
/// types), timestamps within tolerance, validity window consistent, setpoint
/// intervals inside the window, envelope fresh (the P3.13 gates), revision
/// current (the P3.22 check). Appends one evidence row per check.
///
/// This step NEVER touches the network model: the model IR is not even a
/// parameter here.
pub fn validate_candidate(
    candidate: &FeasibilityCandidate,
    sources: &GateSources,
    config: &CandidateValidationConfig,
    now_millis: i64,
    evidence: &mut Vec<GateEvidenceRow>,
) -> Result<(), GateRejection> {
    let step = GateStep::Step1CandidateValidation;

    // Refs present and non-empty.
    let refs = [
        ("candidate_id", candidate.candidate_id.as_str()),
        ("schedule_id", candidate.schedule_id.as_str()),
        ("envelope_version", candidate.envelope_version.as_str()),
        (
            "result_record_snapshot_ref",
            candidate.result_record_snapshot_ref.as_str(),
        ),
    ];
    for (field, value) in refs {
        let row = if value.trim().is_empty() {
            GateEvidenceRow::new(
                step,
                field,
                GateRule::RefMissing,
                EvidenceOutcome::Rejected,
                format!("required candidate reference '{field}' is missing or empty"),
            )
        } else {
            GateEvidenceRow::new(
                step,
                field,
                GateRule::RefMissing,
                EvidenceOutcome::Passed,
                "reference present",
            )
        };
        let rejected = row.outcome == EvidenceOutcome::Rejected;
        evidence.push(row);
        if rejected {
            return Err(rejection_of(evidence, step, field, GateRule::RefMissing));
        }
    }
    if candidate.revision == 0 {
        let row = GateEvidenceRow::new(
            step,
            "revision",
            GateRule::RefMissing,
            EvidenceOutcome::Rejected,
            "the schedule revision must be >= 1 (the first revision is 1)",
        );
        evidence.push(row);
        return Err(rejection_of(evidence, step, "revision", GateRule::RefMissing));
    }

    // Setpoints present and structurally complete.
    if candidate.setpoints.is_empty() {
        let row = GateEvidenceRow::new(
            step,
            "setpoints",
            GateRule::EmptySetpoints,
            EvidenceOutcome::Rejected,
            "the candidate carries no setpoints; an empty candidate is a caller \
             error, not an empty check",
        );
        evidence.push(row);
        return Err(rejection_of(evidence, step, "setpoints", GateRule::EmptySetpoints));
    }
    for (index, setpoint) in candidate.setpoints.iter().enumerate() {
        let subject = format!("setpoints[{index}]");
        let incomplete = setpoint.setpoint_id.trim().is_empty()
            || setpoint.asset_id.trim().is_empty()
            || setpoint.bus_id.trim().is_empty();
        let row = GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::SetpointIncomplete,
            if incomplete {
                EvidenceOutcome::Rejected
            } else {
                EvidenceOutcome::Passed
            },
            if incomplete {
                format!("setpoints[{index}] is incomplete: setpoint_id, asset_id, and \
                         bus_id must all be non-empty")
            } else {
                "setpoint identifiers complete".to_string()
            },
        );
        let rejected = row.outcome == EvidenceOutcome::Rejected;
        evidence.push(row);
        if rejected {
            return Err(rejection_of(
                evidence,
                step,
                &subject,
                GateRule::SetpointIncomplete,
            ));
        }
    }

    // Units canonical: the frozen contract types (`ActivePowerMW`,
    // `ReactivePowerMVAr`) make kW/kWh and non-canonical spellings
    // unrepresentable at this boundary (§10.2; the P3.04 unit rule is the
    // text-boundary counterpart, applied upstream by the adapter). The row
    // records that the canonical check is structural, not ad-hoc.
    evidence.push(GateEvidenceRow::new(
        step,
        "setpoints[].active_power, setpoints[].reactive_power",
        GateRule::UnitsCanonical,
        EvidenceOutcome::Passed,
        "units are canonical by construction: MW and MVAr frozen contract types \
         (kW/kWh unrepresentable at the boundary; §10.2)",
    ));

    // Timestamp: not in the past beyond the configured tolerance; not
    // malformed (non-positive).
    if candidate.created_at_millis <= 0 {
        let row = GateEvidenceRow::new(
            step,
            "created_at",
            GateRule::TimestampMalformed,
            EvidenceOutcome::Rejected,
            format!(
                "candidate timestamp {} is malformed (must be a positive UTC millis instant)",
                candidate.created_at_millis
            ),
        );
        evidence.push(row);
        return Err(rejection_of(evidence, step, "created_at", GateRule::TimestampMalformed));
    }
    let age = now_millis - candidate.created_at_millis;
    if age > config.max_timestamp_age_millis {
        let row = GateEvidenceRow::new(
            step,
            "created_at",
            GateRule::TimestampExpired,
            EvidenceOutcome::Rejected,
            format!(
                "candidate timestamp is {age} ms in the past, beyond the configured \
                 tolerance of {} ms",
                config.max_timestamp_age_millis
            ),
        );
        evidence.push(row);
        return Err(rejection_of(evidence, step, "created_at", GateRule::TimestampExpired));
    }
    evidence.push(GateEvidenceRow::new(
        step,
        "created_at",
        GateRule::TimestampExpired,
        EvidenceOutcome::Passed,
        "timestamp within the configured tolerance",
    ));

    // Validity window: both present (positive), ordered, duration
    // consistent with the stated duration.
    if candidate.valid_from_millis <= 0 || candidate.valid_until_millis <= 0 {
        let row = GateEvidenceRow::new(
            step,
            "validity window",
            GateRule::ValidityWindowInverted,
            EvidenceOutcome::Rejected,
            format!(
                "validity window bounds must be positive UTC millis; found \
                 valid_from = {}, valid_until = {}",
                candidate.valid_from_millis, candidate.valid_until_millis
            ),
        );
        evidence.push(row);
        return Err(rejection_of(
            evidence,
            step,
            "validity window",
            GateRule::ValidityWindowInverted,
        ));
    }
    if candidate.valid_from_millis > candidate.valid_until_millis {
        let row = GateEvidenceRow::new(
            step,
            "validity window",
            GateRule::ValidityWindowInverted,
            EvidenceOutcome::Rejected,
            format!(
                "valid_from ({}) is after valid_until ({})",
                candidate.valid_from_millis, candidate.valid_until_millis
            ),
        );
        evidence.push(row);
        return Err(rejection_of(
            evidence,
            step,
            "validity window",
            GateRule::ValidityWindowInverted,
        ));
    }
    let window_duration = candidate.valid_until_millis - candidate.valid_from_millis;
    if window_duration != candidate.stated_duration_millis {
        let row = GateEvidenceRow::new(
            step,
            "stated_duration",
            GateRule::DurationMismatch,
            EvidenceOutcome::Rejected,
            format!(
                "the window duration {window_duration} ms disagrees with the stated \
                 duration {} ms",
                candidate.stated_duration_millis
            ),
        );
        evidence.push(row);
        return Err(rejection_of(
            evidence,
            step,
            "stated_duration",
            GateRule::DurationMismatch,
        ));
    }
    evidence.push(GateEvidenceRow::new(
        step,
        "validity window",
        GateRule::ValidityWindowInverted,
        EvidenceOutcome::Passed,
        "window ordered and duration consistent with the stated duration",
    ));

    // Setpoint intervals inside the window.
    for (index, setpoint) in candidate.setpoints.iter().enumerate() {
        let subject = format!("setpoints[{}].interval_start", index);
        let inside = setpoint.interval_start_millis >= candidate.valid_from_millis
            && setpoint.interval_start_millis <= candidate.valid_until_millis;
        let row = GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::IntervalOutsideWindow,
            if inside {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if inside {
                "interval start inside the validity window".to_string()
            } else {
                format!(
                    "interval start {} is outside the validity window [{}, {}]",
                    setpoint.interval_start_millis,
                    candidate.valid_from_millis,
                    candidate.valid_until_millis
                )
            },
        );
        let rejected = row.outcome == EvidenceOutcome::Rejected;
        evidence.push(row);
        if rejected {
            return Err(rejection_of(
                evidence,
                step,
                &subject,
                GateRule::IntervalOutsideWindow,
            ));
        }
    }

    // Envelope freshness (the P3.13 staleness gates, via the seam).
    match (sources.envelope_freshness)(&candidate.envelope_version) {
        Ok(EnvelopeFreshness::Fresh) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "envelope_version",
                GateRule::EnvelopeStale,
                EvidenceOutcome::Passed,
                format!("envelope '{}' is fresh (the P3.13 gates)", candidate.envelope_version),
            ));
        }
        Ok(EnvelopeFreshness::Stale { reason }) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "envelope_version",
                GateRule::EnvelopeStale,
                EvidenceOutcome::Rejected,
                format!(
                    "envelope '{}' is stale: {reason} (a stale envelope is never used — \
                     recompute or fail loudly, the P3.13 policy)",
                    candidate.envelope_version
                ),
            ));
            return Err(rejection_of(
                evidence,
                step,
                "envelope_version",
                GateRule::EnvelopeStale,
            ));
        }
        Err(error) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "envelope_version",
                GateRule::EnvelopeCheckFailed,
                EvidenceOutcome::Rejected,
                format!(
                    "the envelope-freshness check failed (fail-closed): {} / {}",
                    error.seam, error.detail
                ),
            ));
            return Err(rejection_of(
                evidence,
                step,
                "envelope_version",
                GateRule::EnvelopeCheckFailed,
            ));
        }
    }

    // Revision current (the P3.22 check, via the seam). Superseded AND
    // unknown are rejected; `None` (unknown) is never usable.
    match (sources.revision_superseded)(&candidate.schedule_id, candidate.revision) {
        Ok(RevisionState::Active) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "schedule revision",
                GateRule::RevisionSuperseded,
                EvidenceOutcome::Passed,
                format!(
                    "schedule '{}' revision {} is the active revision",
                    candidate.schedule_id, candidate.revision
                ),
            ));
        }
        Ok(RevisionState::Superseded) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "schedule revision",
                GateRule::RevisionSuperseded,
                EvidenceOutcome::Rejected,
                format!(
                    "schedule '{}' revision {} is SUPERSEDED by a newer revision; a \
                     superseded revision never reaches the network check",
                    candidate.schedule_id, candidate.revision
                ),
            ));
            return Err(rejection_of(
                evidence,
                step,
                "schedule revision",
                GateRule::RevisionSuperseded,
            ));
        }
        Ok(RevisionState::Unknown) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "schedule revision",
                GateRule::RevisionUnknown,
                EvidenceOutcome::Rejected,
                format!(
                    "schedule '{}' revision {} does not exist in the caller's tenant \
                     scope; unknown is never treated as usable",
                    candidate.schedule_id, candidate.revision
                ),
            ));
            return Err(rejection_of(
                evidence,
                step,
                "schedule revision",
                GateRule::RevisionUnknown,
            ));
        }
        Err(error) => {
            evidence.push(GateEvidenceRow::new(
                step,
                "schedule revision",
                GateRule::RevisionCheckFailed,
                EvidenceOutcome::Rejected,
                format!(
                    "the revision check failed (fail-closed): {} / {}",
                    error.seam, error.detail
                ),
            ));
            return Err(rejection_of(
                evidence,
                step,
                "schedule revision",
                GateRule::RevisionCheckFailed,
            ));
        }
    }

    Ok(())
}

/// Builds the rejection matching the LAST evidence row (the failing check)
/// — the rejection and the row never diverge.
fn rejection_of(
    evidence: &[GateEvidenceRow],
    step: GateStep,
    subject: &str,
    rule: GateRule,
) -> GateRejection {
    let row = evidence.last().expect("the failing row was just appended");
    debug_assert_eq!(row.step, step);
    debug_assert_eq!(row.rule_code, rule.code());
    let _ = (subject, rule);
    GateRejection::from_row(row)
}

// ---------------------------------------------------------------------------
// Step 2 — per-asset injection derivation (capability + ramp)
// ---------------------------------------------------------------------------

/// One derived injection: the canonical candidate injection at the model
/// bus, plus the ramp evidence. The baseline quality is RECORDED alongside
/// the injection (the policy: [`INJECTION_QUALITY_POLICY`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedInjection {
    /// The setpoint this derivation came from.
    pub setpoint_id: String,
    /// The asset the setpoint commands.
    pub asset_id: String,
    /// The canonical injection at the model bus (sign/unit translation via
    /// the owned translation layer).
    pub injection: CandidateInjection,
    /// The ramp baseline: the latest telemetered state WITH its quality
    /// flag (recorded alongside the injection, §10.4).
    pub baseline: TelemeteredState,
    /// The requested change |P_candidate − P_baseline| in MW.
    pub ramp_delta_mw: f64,
    /// The allowed change in MW: `ramp_mw_per_min × interval minutes`.
    pub ramp_limit_mw: f64,
    /// The deterministic derivation reference (evidence consumers cite
    /// this; encodes the baseline instant and quality).
    pub derivation_ref: String,
}

/// The step-2 output: the derived injections for the whole candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedCandidate {
    /// The candidate the derivations belong to (identity refs only).
    pub candidate_id: String,
    pub schedule_id: String,
    pub revision: u64,
    /// The derived injections, in candidate setpoint order.
    pub injections: Vec<DerivedInjection>,
}

/// Derives the per-asset injections (§12.5 step 2; ledger P4.04 step 2):
/// setpoints → per-asset injections at model buses with sign/unit
/// translation via the OWNED translation layer, capability bounds checked
/// (the P3.04-validated data via the seam), and the ramp checked from the
/// latest telemetered state WITH its quality flag. Appends one evidence row
/// per check per setpoint.
///
/// NOTE on the model: this step reads the model IR (bus resolution and the
/// translation round trip) but does not APPLY anything — application is
/// step 3. Step 1 never receives the model at all.
pub fn derive_injections(
    candidate: &FeasibilityCandidate,
    sources: &GateSources,
    model: &crate::port::VersionedNetworkModel,
    evidence: &mut Vec<GateEvidenceRow>,
) -> Result<DerivedCandidate, GateRejection> {
    let step = GateStep::Step2InjectionDerivation;
    // The ramp limit spans the candidate's stated interval duration
    // (D-P4.04-4): delta limit = ramp_mw_per_min × (stated duration in
    // minutes). A finer per-interval ramp profile is the scheduler's
    // concern (P3.22); the gate checks the candidate-level change.
    let interval_minutes = candidate.stated_duration_millis as f64 / 60_000.0;

    let mut derived = Vec::with_capacity(candidate.setpoints.len());
    for setpoint in &candidate.setpoints {
        // Interval provenance: the setpoint's interval start converts to the
        // port's timestamp representation so the applied-injection record
        // carries it — a derived injection never silently loses its
        // interval (§10.3). Step 1 already bounds the value inside the
        // validity window; a conversion failure is a malformed instant and
        // rejects here (fail-closed).
        let interval_start = match flexcore_contracts::time_model::EventTime::from_unix_millis(
            setpoint.interval_start_millis,
        ) {
            Ok(instant) => Some(instant.timestamp()),
            Err(error) => {
                let subject = format!("{}.interval_start", setpoint.setpoint_id);
                evidence.push(GateEvidenceRow::new(
                    step,
                    subject.clone(),
                    GateRule::TimestampMalformed,
                    EvidenceOutcome::Rejected,
                    format!(
                        "interval start {} is not a representable UTC instant: {error}",
                        setpoint.interval_start_millis
                    ),
                ));
                return Err(rejection_of(
                    evidence,
                    step,
                    &subject,
                    GateRule::TimestampMalformed,
                ));
            }
        };

        // Bus resolution (the model IR, read-only).
        let bus_known = model.bus(&setpoint.bus_id).is_some();
        let subject = format!("{}@{}", setpoint.setpoint_id, setpoint.bus_id);
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::BusUnresolved,
            if bus_known {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if bus_known {
                format!("bus '{}' exists in model '{}'", setpoint.bus_id, model.model_ref.model_id)
            } else {
                format!(
                    "bus '{}' does not exist in model '{}'; the setpoint cannot be applied",
                    setpoint.bus_id, model.model_ref.model_id
                )
            },
        ));
        if !bus_known {
            return Err(rejection_of(evidence, step, &subject, GateRule::BusUnresolved));
        }

        // Declared capability bounds (via the seam; unknown is never
        // unconstrained — the P3.02 rule carried forward).
        let bounds = match (sources.capability_bounds)(&setpoint.asset_id, setpoint.interval_start_millis)
        {
            Ok(bounds) => match bounds.validate() {
                Ok(()) => bounds,
                Err(detail) => {
                    evidence.push(GateEvidenceRow::new(
                        step,
                        subject.clone(),
                        GateRule::CapabilityCheckFailed,
                        EvidenceOutcome::Rejected,
                        format!(
                            "the declared bounds for asset '{}' are structurally invalid \
                             (fail-closed): {detail}",
                            setpoint.asset_id
                        ),
                    ));
                    return Err(rejection_of(
                        evidence,
                        step,
                        &subject,
                        GateRule::CapabilityCheckFailed,
                    ));
                }
            },
            Err(error) => {
                evidence.push(GateEvidenceRow::new(
                    step,
                    subject.clone(),
                    GateRule::CapabilityUnavailable,
                    EvidenceOutcome::Rejected,
                    format!(
                        "no declared capability bounds for asset '{}' at interval {}: \
                         {} / {} (unknown is never unconstrained, the P3.02 rule)",
                        setpoint.asset_id, setpoint.interval_start_millis, error.seam, error.detail
                    ),
                ));
                return Err(rejection_of(
                    evidence,
                    step,
                    &subject,
                    GateRule::CapabilityUnavailable,
                ));
            }
        };
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::CapabilityUnavailable,
            EvidenceOutcome::Passed,
            format!(
                "declared bounds present for asset '{}' at interval {} (P3.04-validated data)",
                setpoint.asset_id, setpoint.interval_start_millis
            ),
        ));

        // P within bounds.
        let p = setpoint.active_power.value();
        let p_within = p >= bounds.p_min_mw && p <= bounds.p_max_mw;
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::CapabilityExceeded,
            if p_within {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if p_within {
                format!("active power {p} MW within [{}, {}] MW", bounds.p_min_mw, bounds.p_max_mw)
            } else {
                format!(
                    "active power {p} MW outside the declared bounds [{}, {}] MW \
                     (canonical sign: P > 0 = export)",
                    bounds.p_min_mw, bounds.p_max_mw
                )
            },
        ));
        if !p_within {
            return Err(rejection_of(evidence, step, &subject, GateRule::CapabilityExceeded));
        }

        // Q within bounds (canonical values under the declared convention).
        let q = setpoint.reactive_power.value();
        let q_within = q >= bounds.q_min_mvar && q <= bounds.q_max_mvar;
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::CapabilityExceeded,
            if q_within {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if q_within {
                format!(
                    "reactive power {q} MVAr within [{}, {}] MVAr",
                    bounds.q_min_mvar, bounds.q_max_mvar
                )
            } else {
                format!(
                    "reactive power {q} MVAr outside the declared bounds [{}, {}] MVAr",
                    bounds.q_min_mvar, bounds.q_max_mvar
                )
            },
        ));
        if !q_within {
            return Err(rejection_of(evidence, step, &subject, GateRule::CapabilityExceeded));
        }

        // Q convention: the setpoint must declare the SAME convention the
        // capability model declares — the sign is never inferred (§10.2).
        let convention_matches = setpoint.reactive_sign_convention == bounds.q_convention;
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::QConventionMismatch,
            if convention_matches {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if convention_matches {
                "declared Q convention matches the capability model's declaration".to_string()
            } else {
                format!(
                    "setpoint declares Q convention '{}' but the capability model declares \
                     '{}'; the Q sign is never inferred or converted silently (§10.2)",
                    match setpoint.reactive_sign_convention {
                        ReactiveSignConvention::InjectionPositive => "injection_positive",
                        ReactiveSignConvention::ConsumptionPositive => "consumption_positive",
                    },
                    match bounds.q_convention {
                        ReactiveSignConvention::InjectionPositive => "injection_positive",
                        ReactiveSignConvention::ConsumptionPositive => "consumption_positive",
                    }
                )
            },
        ));
        if !convention_matches {
            return Err(rejection_of(evidence, step, &subject, GateRule::QConventionMismatch));
        }

        // Ramp baseline: the latest telemetered state WITH its quality flag.
        let baseline = match (sources.ramp_baseline)(&setpoint.asset_id) {
            Ok(state) => {
                // Quality gate (D-P4.04-3; the P4.08 pre-wiring): BAD,
                // MISSING, STALE block. GOOD, SUSPECT, ESTIMATED accepted.
                let blocked = matches!(
                    state.quality,
                    TelemetryQuality::Bad | TelemetryQuality::Missing | TelemetryQuality::Stale
                );
                evidence.push(GateEvidenceRow::new(
                    step,
                    subject.clone(),
                    GateRule::RampBaselineBlocked,
                    if blocked {
                        EvidenceOutcome::Rejected
                    } else {
                        EvidenceOutcome::Passed
                    },
                    if blocked {
                        format!(
                            "ramp baseline for asset '{}' has quality {:?}; BAD/MISSING/STALE \
                             baselines BLOCK the check (fail-closed, the P4.08 pre-wiring)",
                            setpoint.asset_id,
                            state.quality
                        )
                    } else {
                        format!(
                            "ramp baseline for asset '{}' accepted with quality {:?} \
                             (observed at {})",
                            setpoint.asset_id, state.quality, state.observed_at_millis
                        )
                    },
                ));
                if blocked {
                    return Err(rejection_of(
                        evidence,
                        step,
                        &subject,
                        GateRule::RampBaselineBlocked,
                    ));
                }
                state
            }
            Err(error) => {
                evidence.push(GateEvidenceRow::new(
                    step,
                    subject.clone(),
                    GateRule::RampBaselineUnavailable,
                    EvidenceOutcome::Rejected,
                    format!(
                        "no ramp baseline available for asset '{}': {} / {} (fail-closed)",
                        setpoint.asset_id, error.seam, error.detail
                    ),
                ));
                return Err(rejection_of(
                    evidence,
                    step,
                    &subject,
                    GateRule::RampBaselineUnavailable,
                ));
            }
        };

        // Ramp check: |ΔP| ≤ ramp_mw_per_min × interval minutes.
        let delta = (p - baseline.active_power_mw).abs();
        let limit = bounds.ramp_mw_per_min * interval_minutes;
        let ramp_ok = delta <= limit;
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::RampExceeded,
            if ramp_ok {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if ramp_ok {
                format!(
                    "ramp |ΔP| = {delta:.6} MW within the limit {limit:.6} MW \
                     ({} MW/min × {interval_minutes:.3} min over the stated duration)",
                    bounds.ramp_mw_per_min
                )
            } else {
                format!(
                    "ramp |ΔP| = {delta:.6} MW from the baseline {:.6} MW exceeds the limit \
                     {limit:.6} MW ({} MW/min × {interval_minutes:.3} min over the stated \
                     duration)",
                    baseline.active_power_mw, bounds.ramp_mw_per_min
                )
            },
        ));
        if !ramp_ok {
            return Err(rejection_of(evidence, step, &subject, GateRule::RampExceeded));
        }

        // Build the canonical injection and prove the sign/unit translation
        // through the OWNED translation layer (both directions; the same
        // round trip `tests/translation_roundtrip.rs` pins with
        // hand-computed fixtures).
        let injection = CandidateInjection {
            injection_id: setpoint.setpoint_id.clone(),
            bus_id: setpoint.bus_id.clone(),
            active_power: setpoint.active_power,
            reactive_power: setpoint.reactive_power,
            reactive_sign_convention: setpoint.reactive_sign_convention,
            interval_start,
        };
        let round_trip_ok = candidate_to_backend(&injection)
            .ok()
            .and_then(|backend| injection_from_backend(&backend, setpoint.reactive_sign_convention).ok())
            .is_some_and(|back| back == injection);
        evidence.push(GateEvidenceRow::new(
            step,
            subject.clone(),
            GateRule::DerivationMismatch,
            if round_trip_ok {
                EvidenceOutcome::Passed
            } else {
                EvidenceOutcome::Rejected
            },
            if round_trip_ok {
                "derived injection round-trips the owned translation layer \
                 (canonical → worker → canonical, exact under the declared convention)"
                    .to_string()
            } else {
                "the derived injection failed the translation round trip; the sign/unit \
                 mapping is contract-broken (fail-closed)"
                    .to_string()
            },
        ));
        if !round_trip_ok {
            return Err(rejection_of(evidence, step, &subject, GateRule::DerivationMismatch));
        }

        let derivation_ref = format!(
            "derived_injection:{}:asset={}:baseline@{}:quality={}",
            setpoint.setpoint_id,
            setpoint.asset_id,
            baseline.observed_at_millis,
            baseline.quality.name(),
        );
        evidence.push(GateEvidenceRow::new(
            step,
            derivation_ref.clone(),
            GateRule::DerivationRecorded,
            EvidenceOutcome::Recorded,
            format!(
                "derived injection recorded (baseline quality {:?} travels with it, \
                 §10.4 non-silencing rule)",
                baseline.quality
            ),
        ));

        derived.push(DerivedInjection {
            setpoint_id: setpoint.setpoint_id.clone(),
            asset_id: setpoint.asset_id.clone(),
            injection,
            baseline,
            ramp_delta_mw: delta,
            ramp_limit_mw: limit,
            derivation_ref,
        });
    }

    Ok(DerivedCandidate {
        candidate_id: candidate.candidate_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        revision: candidate.revision,
        injections: derived,
    })
}

// ---------------------------------------------------------------------------
// Step 3 — apply to the versioned model; step 4 — run the AC power flow
// ---------------------------------------------------------------------------

/// The candidate reference block carried on every attempt record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateRefs {
    /// The candidate identifier.
    pub candidate_id: String,
    /// The schedule chain id.
    pub schedule_id: String,
    /// The schedule revision number.
    pub revision: u64,
    /// The envelope version.
    pub envelope_version: String,
    /// The result-record snapshot reference.
    pub result_record_snapshot_ref: String,
}

/// The attempt output: either the recorded power-flow result, or the
/// INCONCLUSIVE evidence (fail-closed). NEVER a feasibility verdict — the
/// limit check is P4.06, the reject/constrain/return routing is P4.07.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "output")]
pub enum AttemptOutput {
    /// The power flow converged; the full result is recorded as evidence.
    PowerFlowEvaluated { result: PowerFlowResult },
    /// The power flow did not converge, or the provider reported an error:
    /// recorded INCONCLUSIVE with diagnostics — never a pass, never a
    /// rejection-by-limit (P4.07 decides the verdict category).
    Inconclusive { result: PowerFlowResult, reason: String },
}

impl AttemptOutput {
    /// The carried power-flow result (either branch).
    pub fn result(&self) -> &PowerFlowResult {
        match self {
            AttemptOutput::PowerFlowEvaluated { result } => result,
            AttemptOutput::Inconclusive { result, .. } => result,
        }
    }
}

/// The record of ONE feasibility-gate attempt (ledger P4.05 step 4): the
/// evidence type P4.06, P4.07, and P4.12 consume. Inputs (candidate refs,
/// model version + hash, injection derivation refs), outputs (the
/// power-flow result or the inconclusive evidence), timestamps, provider
/// version. NO feasibility verdict is carried or implied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeasibilityAttemptRecord {
    /// The attempt identifier (caller-supplied, unique per attempt).
    pub attempt_id: String,
    /// UTC millis the attempt started.
    pub started_at_millis: i64,
    /// UTC millis the attempt finished.
    pub finished_at_millis: i64,
    /// The candidate references (identity block).
    pub candidate_refs: CandidateRefs,
    /// The versioned model the attempt evaluated against (identity +
    /// version + THE HASH — evidence references the hash).
    pub model_ref: crate::port::GridModelRef,
    /// The derivation references of every applied injection (baseline
    /// instant and quality encoded).
    pub injection_derivation_refs: Vec<String>,
    /// The attempt output (result or inconclusive evidence).
    pub output: AttemptOutput,
    /// Provider identifier (recorded from the result).
    pub provider_id: String,
    /// Provider version — REQUIRED (the P4.01 pin discipline).
    pub provider_version: String,
    /// The unbalanced/asymmetric limitation note (carried on every record).
    pub unbalanced_deferral_note: String,
}

/// Completeness violations of the attempt record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptRecordError {
    /// A required identification/reference field is empty (`field` names
    /// it).
    EmptyField { field: &'static str },
    /// The model reference is invalid.
    InvalidModelRef,
    /// No injection derivation reference is recorded.
    NoDerivationRefs,
    /// Timestamps are missing or inverted.
    InvalidTimestamps,
    /// The output branch disagrees with the carried result's status
    /// (a `PowerFlowEvaluated` must be converged; an `Inconclusive` must
    /// not be), or the inconclusive reason is empty.
    OutputInconsistent { detail: String },
    /// The carried power-flow result failed its own completeness gate.
    IncompleteResult(crate::port::ResultCompletenessError),
}

impl std::fmt::Display for AttemptRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttemptRecordError::EmptyField { field } => {
                write!(f, "attempt record field '{field}' is empty")
            }
            AttemptRecordError::InvalidModelRef => {
                write!(f, "attempt record carries an invalid model reference")
            }
            AttemptRecordError::NoDerivationRefs => {
                write!(f, "attempt record carries no injection derivation references")
            }
            AttemptRecordError::InvalidTimestamps => {
                write!(f, "attempt timestamps are invalid (started/finished must be positive and ordered)")
            }
            AttemptRecordError::OutputInconsistent { detail } => {
                write!(f, "attempt output is inconsistent: {detail}")
            }
            AttemptRecordError::IncompleteResult(error) => {
                write!(f, "attempt carries an incomplete power-flow result: {error}")
            }
        }
    }
}

impl std::error::Error for AttemptRecordError {}

impl FeasibilityAttemptRecord {
    /// Validates record completeness: every reference present, timestamps
    /// ordered, provider version REQUIRED (the P4.01 pin), and the output
    /// branch consistent with the carried result (a converged result under
    /// `PowerFlowEvaluated`; a non-converged/errored result under
    /// `Inconclusive` with a non-empty reason — silence is never legal).
    pub fn validate_complete(&self) -> Result<(), AttemptRecordError> {
        for (field, value) in [
            ("attempt_id", self.attempt_id.as_str()),
            ("candidate_refs.candidate_id", self.candidate_refs.candidate_id.as_str()),
            ("candidate_refs.schedule_id", self.candidate_refs.schedule_id.as_str()),
            ("candidate_refs.envelope_version", self.candidate_refs.envelope_version.as_str()),
            (
                "candidate_refs.result_record_snapshot_ref",
                self.candidate_refs.result_record_snapshot_ref.as_str(),
            ),
            ("provider_id", self.provider_id.as_str()),
            ("provider_version", self.provider_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(AttemptRecordError::EmptyField { field });
            }
        }
        if self.candidate_refs.revision == 0 {
            return Err(AttemptRecordError::EmptyField {
                field: "candidate_refs.revision",
            });
        }
        if self.model_ref.validate().is_err() {
            return Err(AttemptRecordError::InvalidModelRef);
        }
        if self.injection_derivation_refs.is_empty()
            || self.injection_derivation_refs.iter().any(|r| r.trim().is_empty())
        {
            return Err(AttemptRecordError::NoDerivationRefs);
        }
        if self.started_at_millis <= 0
            || self.finished_at_millis < self.started_at_millis
        {
            return Err(AttemptRecordError::InvalidTimestamps);
        }
        if self.unbalanced_deferral_note.trim().is_empty() {
            return Err(AttemptRecordError::EmptyField {
                field: "unbalanced_deferral_note",
            });
        }
        self.result()
            .validate_complete()
            .map_err(AttemptRecordError::IncompleteResult)?;
        match &self.output {
            AttemptOutput::PowerFlowEvaluated { .. } => {
                if self.result().status != PowerFlowStatus::Converged {
                    return Err(AttemptRecordError::OutputInconsistent {
                        detail: "a PowerFlowEvaluated output must carry a converged result"
                            .to_string(),
                    });
                }
            }
            AttemptOutput::Inconclusive { reason, .. } => {
                if self.result().status == PowerFlowStatus::Converged {
                    return Err(AttemptRecordError::OutputInconsistent {
                        detail: "an Inconclusive output must NOT carry a converged result"
                            .to_string(),
                    });
                }
                if reason.trim().is_empty() {
                    return Err(AttemptRecordError::OutputInconsistent {
                        detail: "an Inconclusive output must record a non-empty reason"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The carried power-flow result.
    pub fn result(&self) -> &PowerFlowResult {
        self.output.result()
    }
}

/// The step-3 + step-4 execution (§12.5 steps 3–4; ledger P4.05):
///
/// 1. **Step 3 — apply.** The model health check (P4.03) runs FIRST and
///    refuses a stale/incomplete/uncovered model (fail-closed); then the
///    pre-run content-hash hook re-verifies the live model against the
///    recorded hash — a mid-gate model change aborts fail-closed. The
///    derived injections are applied through the `GridModelProvider` PORT
///    (never a provider directly; the port applies them per element and
///    records the applied-injection rows on the result).
/// 2. **Step 4 — run.** An AC power-flow evaluation of the specified
///    injections (NOT AC-OPF). A converged result is recorded as evidence;
///    non-convergence / provider error is recorded INCONCLUSIVE — fail-
///    closed, never retried ([`NO_RETRY_POLICY`]), never a pass, and never
///    a rejection-by-limit (P4.07 owns the verdict category).
///
/// Appends the step-3 and step-4 evidence rows in order.
pub fn apply_and_run_power_flow(
    candidate: &FeasibilityCandidate,
    derived: &DerivedCandidate,
    versioned_model: &VersionedModel,
    registry: &ModelVersionRegistry,
    portfolio_elements: &[RegistryElement],
    health_config: &ModelHealthConfig,
    provider: &dyn GridModelProvider,
    limits: &NetworkLimits,
    attempt_id: &str,
    now_millis: i64,
    evidence: &mut Vec<GateEvidenceRow>,
) -> Result<FeasibilityAttemptRecord, (GateAbort, GateStep)> {
    // --- Step 3: apply -----------------------------------------------------
    let step3 = GateStep::Step3ModelApplication;

    // The network limits travel with the attempt for P4.06/P4.07 (the limit
    // CHECK itself is step 5, not here). The configuration is validated at
    // this boundary so an invalid limit set never enters an attempt record
    // (fail-closed; the same validation `evaluate_limits` will re-run).
    if let Err(error) = limits.validate() {
        let abort = GateAbort::ProviderError {
            detail: format!("the network-limit configuration is invalid: {error}"),
        };
        evidence.push(GateEvidenceRow::new(
            step3,
            "network_limits",
            abort.rule(),
            EvidenceOutcome::Aborted,
            abort.detail(),
        ));
        return Err((abort, step3));
    }

    // The model health check (P4.03) — the check the gate calls; fail-closed.
    let health =
        check_model_health(versioned_model, registry, portfolio_elements, health_config, now_millis);
    if let Err(error) = assert_usable_for_feasibility(&health) {
        let findings = match &error {
            crate::model_health::ModelHealthError::NotUsable { findings } => findings.clone(),
            other => {
                return Err((
                    GateAbort::ProviderError {
                        detail: format!("unexpected health-check error: {other}"),
                    },
                    step3,
                ));
            }
        };
        let abort = GateAbort::ModelNotHealthy { findings };
        evidence.push(GateEvidenceRow::new(
            step3,
            format!("model '{}'", versioned_model.model_ref().model_id),
            abort.rule(),
            EvidenceOutcome::Aborted,
            abort.detail(),
        ));
        return Err((abort, step3));
    }
    evidence.push(GateEvidenceRow::new(
        step3,
        format!(
            "model '{}' version '{}'",
            versioned_model.model_ref().model_id,
            versioned_model.model_ref().version
        ),
        GateRule::ModelNotHealthy,
        EvidenceOutcome::Passed,
        format!(
            "model health check passed (completeness, staleness, coverage; hash {}) — \
             the model is fit to evaluate against",
            versioned_model.model_ref().content_hash
        ),
    ));

    // The pre-run content-hash hook: a mid-gate model change aborts
    // fail-closed (the live artefact must still hash to the recorded hash).
    if !versioned_model.verify_content_hash() {
        let abort = GateAbort::ModelHashMismatch {
            recorded: versioned_model.model_ref().content_hash.clone(),
            computed: crate::model_health::model_content_hash(&versioned_model.model),
        };
        evidence.push(GateEvidenceRow::new(
            step3,
            format!("model '{}'", versioned_model.model_ref().model_id),
            abort.rule(),
            EvidenceOutcome::Aborted,
            abort.detail(),
        ));
        return Err((abort, step3));
    }
    evidence.push(GateEvidenceRow::new(
        step3,
        format!("model '{}'", versioned_model.model_ref().model_id),
        GateRule::ModelHashMismatch,
        EvidenceOutcome::Passed,
        "pre-run content-hash verification passed: the live model still hashes to \
         the recorded hash",
    ));

    // --- Step 4: run the AC power flow through the PORT ---------------------
    let step4 = GateStep::Step4PowerFlow;
    let injections: Vec<CandidateInjection> = derived
        .injections
        .iter()
        .map(|d| d.injection.clone())
        .collect();

    let started_at_millis = now_millis;
    let run = provider.run_power_flow(&versioned_model.model, &injections);
    let finished_at_millis = now_millis; // the gate stamps the record; the result carries its own runtime

    let output = match run {
        Ok(result) => match result.status {
            PowerFlowStatus::Converged => {
                evidence.push(GateEvidenceRow::new(
                    step4,
                    format!(
                        "attempt '{attempt_id}' on model '{}' version '{}'",
                        result.model_ref.model_id, result.model_ref.version
                    ),
                    GateRule::PowerFlowEvaluated,
                    EvidenceOutcome::Recorded,
                    format!(
                        "AC power flow converged and the result was recorded as EVIDENCE \
                         (bus voltages, branch/transformer loadings, residuals, coverage \
                         limitations, provider {} {}); NO feasibility verdict is implied — \
                         limits are P4.06, routing is P4.07",
                        result.provider_id, result.provider_version
                    ),
                ));
                AttemptOutput::PowerFlowEvaluated { result }
            }
            PowerFlowStatus::DidNotConverge => {
                let reason = "the power flow did not converge: recorded INCONCLUSIVE, \
                              fail-closed — never a pass, never a rejection-by-limit \
                              (P4.07 decides the verdict category); retry policy: NONE"
                    .to_string();
                evidence.push(GateEvidenceRow::new(
                    step4,
                    format!("attempt '{attempt_id}'"),
                    GateRule::PowerFlowInconclusive,
                    EvidenceOutcome::Inconclusive,
                    reason.clone(),
                ));
                AttemptOutput::Inconclusive { result, reason }
            }
            PowerFlowStatus::ProviderReportedError => {
                let reason = "the provider reported an evaluation error: recorded \
                              INCONCLUSIVE, fail-closed (no network verdict exists)"
                    .to_string();
                evidence.push(GateEvidenceRow::new(
                    step4,
                    format!("attempt '{attempt_id}'"),
                    GateRule::PowerFlowInconclusive,
                    EvidenceOutcome::Inconclusive,
                    reason.clone(),
                ));
                AttemptOutput::Inconclusive { result, reason }
            }
        },
        Err(error) => {
            // A port error is an infrastructure/contract failure, not a
            // power-flow outcome: abort (fail-closed), recorded.
            let abort = match error {
                GridPortError::BackendUnavailable(message)
                | GridPortError::BackendContractViolation(message) => {
                    GateAbort::ProviderError { detail: message }
                }
                other => GateAbort::ProviderError {
                    detail: other.to_string(),
                },
            };
            evidence.push(GateEvidenceRow::new(
                step4,
                format!("attempt '{attempt_id}'"),
                abort.rule(),
                EvidenceOutcome::Aborted,
                abort.detail(),
            ));
            return Err((abort, step4));
        }
    };

    let result = output.result();
    let record = FeasibilityAttemptRecord {
        attempt_id: attempt_id.to_string(),
        started_at_millis,
        finished_at_millis,
        candidate_refs: CandidateRefs {
            candidate_id: candidate.candidate_id.clone(),
            schedule_id: candidate.schedule_id.clone(),
            revision: candidate.revision,
            envelope_version: candidate.envelope_version.clone(),
            result_record_snapshot_ref: candidate.result_record_snapshot_ref.clone(),
        },
        model_ref: versioned_model.model_ref().clone(),
        injection_derivation_refs: derived
            .injections
            .iter()
            .map(|d| d.derivation_ref.clone())
            .collect(),
        output,
        provider_id: result.provider_id.clone(),
        provider_version: result.provider_version.clone(),
        unbalanced_deferral_note: UNBALANCED_DEFERRAL_NOTE.to_string(),
    };
    // The record must be complete before it leaves the pipeline: a
    // half-filled attempt record is a provider/pipeline bug, never
    // published evidence.
    if let Err(error) = record.validate_complete() {
        let abort = GateAbort::ProviderError {
            detail: format!("the assembled attempt record is incomplete: {error}"),
        };
        evidence.push(GateEvidenceRow::new(
            step4,
            format!("attempt '{attempt_id}'"),
            abort.rule(),
            EvidenceOutcome::Aborted,
            abort.detail(),
        ));
        return Err((abort, step4));
    }
    Ok(record)
}

// ---------------------------------------------------------------------------
// The steps 1–4 pipeline
// ---------------------------------------------------------------------------

/// Everything one gate run consumes. `now_millis` is the gate clock (UTC
/// millis, §10.3); the caller supplies it — the gate never reads a wall
/// clock, keeping runs deterministic and replayable.
pub struct GateRunRequest<'a> {
    /// The candidate (§12.5 step 1 input).
    pub candidate: &'a FeasibilityCandidate,
    /// The injected Phase 2/3 source seams.
    pub sources: &'a GateSources,
    /// The step-1 validation configuration.
    pub config: &'a CandidateValidationConfig,
    /// The versioned model to evaluate against (P4.03).
    pub versioned_model: &'a VersionedModel,
    /// The model-version registry (staleness source changes).
    pub registry: &'a ModelVersionRegistry,
    /// The registry elements the portfolio contains (the coverage check).
    pub portfolio_elements: &'a [RegistryElement],
    /// The model-health configuration.
    pub health_config: &'a ModelHealthConfig,
    /// The grid-model provider — through the PORT only (§12.4).
    pub provider: &'a dyn GridModelProvider,
    /// The network limits (carried onto the attempt inputs for P4.06; the
    /// limit CHECK itself is P4.06/P4.07, not here).
    pub limits: &'a NetworkLimits,
    /// The attempt identifier for the record.
    pub attempt_id: String,
    /// The gate clock (UTC millis).
    pub now_millis: i64,
}

/// The outcome of gate steps 1–4. Every variant carries the full evidence
/// row list; NOTHING here is a feasibility verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum GatePipelineOutcome {
    /// The candidate was rejected at the named step (a candidate fault:
    /// schema, capability, ramp). The rejection is the last rejection-
    /// outcome evidence row.
    Rejected {
        step: GateStep,
        rejection: GateRejection,
        evidence: Vec<GateEvidenceRow>,
    },
    /// The pipeline aborted at the named step (a model/infrastructure
    /// condition; P4.08 owns the reject-vs-hold classification).
    Aborted {
        step: GateStep,
        abort: GateAbort,
        evidence: Vec<GateEvidenceRow>,
    },
    /// Steps 1–4 completed: the attempt record is the evidence P4.06/07/12
    /// consume.
    Completed {
        attempt: FeasibilityAttemptRecord,
        evidence: Vec<GateEvidenceRow>,
    },
}

impl GatePipelineOutcome {
    /// All evidence rows of the run, in pipeline order.
    pub fn evidence(&self) -> &[GateEvidenceRow] {
        match self {
            GatePipelineOutcome::Rejected { evidence, .. }
            | GatePipelineOutcome::Aborted { evidence, .. }
            | GatePipelineOutcome::Completed { evidence, .. } => evidence,
        }
    }
}

/// Runs gate steps 1–4 in order (§12.5 steps 1–4): candidate validation,
/// injection derivation, model application (health- and hash-gated), and
/// the AC power-flow evaluation through the port. No step may be skipped,
/// the order is fixed, and every intermediate check is an evidence row —
/// the end-to-end test pins the row order.
///
/// Note again: the OUTCOME (pass / violations / reject / constrain /
/// return) is P4.06/P4.07's; this function produces evidence only.
pub fn run_feasibility_gate(request: GateRunRequest) -> GatePipelineOutcome {
    let mut evidence: Vec<GateEvidenceRow> = Vec::new();

    // Step 1 — before any model is touched.
    if let Err(rejection) = validate_candidate(
        request.candidate,
        request.sources,
        request.config,
        request.now_millis,
        &mut evidence,
    ) {
        return GatePipelineOutcome::Rejected {
            step: GateStep::Step1CandidateValidation,
            rejection,
            evidence,
        };
    }

    // Step 2 — derivation (model IR read-only: bus resolution + translation).
    let derived = match derive_injections(
        request.candidate,
        request.sources,
        &request.versioned_model.model,
        &mut evidence,
    ) {
        Ok(derived) => derived,
        Err(rejection) => {
            return GatePipelineOutcome::Rejected {
                step: GateStep::Step2InjectionDerivation,
                rejection,
                evidence,
            };
        }
    };

    // Steps 3–4 — apply and run, health- and hash-gated.
    match apply_and_run_power_flow(
        request.candidate,
        &derived,
        request.versioned_model,
        request.registry,
        request.portfolio_elements,
        request.health_config,
        request.provider,
        request.limits,
        &request.attempt_id,
        request.now_millis,
        &mut evidence,
    ) {
        Ok(attempt) => GatePipelineOutcome::Completed { attempt, evidence },
        Err((abort, step)) => GatePipelineOutcome::Aborted { step, abort, evidence },
    }
}

// ---------------------------------------------------------------------------
// In-crate unit tests (the integration tests live in tests/feasibility_gate.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_health::{ModelCoverage, ModelLifecycleState};
    use crate::tests_support::three_bus_model;
    use crate::port::GridModelRef;
    use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr};

    fn sources_accepting() -> GateSources {
        GateSources {
            capability_bounds: Arc::new(|_asset, _at| {
                Ok(DeclaredBounds {
                    p_min_mw: -5.0,
                    p_max_mw: 5.0,
                    q_min_mvar: -2.0,
                    q_max_mvar: 2.0,
                    q_convention: ReactiveSignConvention::InjectionPositive,
                    ramp_mw_per_min: 1.0,
                })
            }),
            envelope_freshness: Arc::new(|_| Ok(EnvelopeFreshness::Fresh)),
            revision_superseded: Arc::new(|_, _| Ok(RevisionState::Active)),
            ramp_baseline: Arc::new(|_| {
                Ok(TelemeteredState {
                    active_power_mw: 0.0,
                    quality: TelemetryQuality::Good,
                    observed_at_millis: 900,
                })
            }),
        }
    }

    fn candidate_fixture() -> FeasibilityCandidate {
        FeasibilityCandidate {
            candidate_id: "cand-1".to_string(),
            schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
            revision: 2,
            envelope_version: "env-v1".to_string(),
            result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
            created_at_millis: 1_000,
            valid_from_millis: 2_000,
            valid_until_millis: 5_000,
            stated_duration_millis: 3_000,
            setpoints: vec![CandidateSetpoint {
                setpoint_id: "sp-1".to_string(),
                asset_id: "asset-1".to_string(),
                bus_id: "bus_3".to_string(),
                active_power: ActivePowerMW::new(2.5).expect("valid"),
                reactive_power: ReactivePowerMVAr::new(0.5).expect("valid"),
                reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
                interval_start_millis: 2_000,
            }],
        }
    }

    fn versioned_model_fixture(now: i64) -> VersionedModel {
        let model = three_bus_model(GridModelRef {
            model_id: "feeder-a".to_string(),
            version: "1".to_string(),
            content_hash: String::new(),
        });
        let coverage = ModelCoverage {
            covered_feeder_ids: vec!["feeder-a".to_string()],
            covered_element_ids: vec!["asset-1".to_string()],
            coverage_statement: "covers feeder-a and asset-1 (fixture)".to_string(),
        };
        VersionedModel::register(model, ModelLifecycleState::Active, coverage, now)
            .expect("registers")
    }

    #[test]
    fn step1_passes_a_well_formed_candidate_with_rows_in_order() {
        let mut evidence = Vec::new();
        validate_candidate(
            &candidate_fixture(),
            &sources_accepting(),
            &CandidateValidationConfig::default(),
            1_500,
            &mut evidence,
        )
        .expect("well-formed candidate passes step 1");
        assert!(evidence.iter().all(|row| row.step == GateStep::Step1CandidateValidation));
        assert!(evidence.iter().any(|row| row.rule_code == "REF_MISSING"
            && row.outcome == EvidenceOutcome::Passed));
        assert!(evidence.iter().any(|row| row.rule_code == "ENVELOPE_STALE"
            && row.outcome == EvidenceOutcome::Passed));
        assert!(evidence.iter().any(|row| row.rule_code == "REVISION_SUPERSEDED"
            && row.outcome == EvidenceOutcome::Passed));
    }

    #[test]
    fn step2_derives_and_records_the_baseline_quality() {
        let model = versioned_model_fixture(1_000);
        let mut evidence = Vec::new();
        let derived = derive_injections(
            &candidate_fixture(),
            &sources_accepting(),
            &model.model,
            &mut evidence,
        )
        .expect("derives");
        assert_eq!(derived.injections.len(), 1);
        let d = &derived.injections[0];
        assert!((d.ramp_delta_mw - 2.5).abs() < 1.0e-12);
        // 1.0 MW/min × 3 min = 3.0 MW allowed over the stated duration.
        assert!((d.ramp_limit_mw - 3.0).abs() < 1.0e-12);
        assert_eq!(d.baseline.quality, TelemetryQuality::Good);
        assert!(d.derivation_ref.contains("quality=GOOD"));
        assert!(evidence.iter().any(|row| row.rule_code == "RAMP_BASELINE_BLOCKED"
            && row.outcome == EvidenceOutcome::Passed));
    }
}
