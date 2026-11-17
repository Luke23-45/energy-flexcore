// P18.06 — The device-fleet staged rollout ORCHESTRATOR (control plane):
//          cohorts, pre-registered abort thresholds, per-device
//          attestation before authorization, and fleet-state
//          visualization data (decision record
//          `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
//          06_p18_06_rollout_orchestrator_scope_distinction.md`).
//
// THE SCOPE DISTINCTION (load-bearing; context constraint 2): the kernel
// PLUGIN staged rollout (`crates/flexcore-kernel/src/revocation.rs`,
// P7.38) is cohort/percentage-free BY CONSTRUCTION — its `ROLLOUT_STATEMENT`
// records that "There is NO percentage canary — a percentage rollout is
// not constructible from the StagedRollout type surface". That policy is
// about PLUGIN SWAPPING inside one host composition and stays
// BYTE-IDENTICAL. This module is a NEW, SEPARATELY-ARGUED mechanism for
// DEVICE-FLEET software rollout: a percentage canary IS expressible here
// because the risk profile is different (a bad device update can brick a
// fleet that cannot be recalled for hands-on recovery), the parameters
// are pre-registered configuration (X-8), and every device authorization
// is individually attested. The kernel policy is NOT reinterpreted,
// relaxed, or cited as permission — record 06 quotes it verbatim and
// argues the new mechanism separately.
//
// THE PRE-REGISTRATION DISCIPLINE (X-8; conventions §5): the canary size
// and the abort thresholds are REQUIRED constructor parameters — a
// rollout plan without a registered canary size and registered abort
// thresholds is REFUSED at plan time, structurally. The canary 5–10% of a
// block DEFAULT is a RECOMMENDATION, not a sourced figure (the phase
// plan's P18.06 row); the validator accepts 500..=1000 basis points as
// the pre-registered DEFAULT band and records that every actual value is
// the owner's configuration, never code.
//
// COHORT MEMBERSHIP (consumed by citation — the wait-check PASSED):
// members come from the [`FleetCohortDirectory`] port at cohort
// activation and per-device authorization time. The composition binds
// this port to Agent B's P18.12 fleet registry
// (`crates/edge-agent/src/fleet_registry.rs` —
// `FleetStateView::current_cohort(device_id)` → `CohortMembership {
// cohort_id, block_index, observed_at_unix_millis }`; record 25 §4), the
// journal keyed to migration `0023_edge_fleet_registry_p18_12`. The port
// exists because the dependency rule FORBIDS a core crate (this one)
// depending on a composition crate (edge-agent): the binding is the
// composition root's, and this module consumes the schema by citation —
// it never redefines a value, adds a kind, or writes a journal row (the
// journal is append-only by trigger; no write path exists here). A
// directory failure is fail-closed: membership that cannot be read
// authorizes NOTHING.
//
// ATTESTATION BEFORE AUTHORIZATION (consumed by citation — the
// wait-check PASSED): the per-device verdict is Agent B's P18.13
// verification policy output (`crates/edge-agent/src/attestation.rs`,
// `AttestationDecision` — record 26). The composition maps
// `AuthorizeUpdate` → [`AttestationVerdict::Passed`] and every
// `Refuse*` decision → [`AttestationVerdict::Failed`] (policy
// refusals) or [`AttestationVerdict::Unavailable`] (no evidence
// established); a device without a PASSED verdict is never authorized —
// named and audited (fail-closed: the absence of evidence is never a
// pass). Attestation is one gate, never a bypass — B's own decision doc
// says the orchestrator still applies its gates (cohort state,
// command-lifecycle blocking, P18.15).
//
// AUDIT: every plan, authorization, refusal, outcome, threshold
// evaluation, cohort advance, and abort emits a [`RolloutAuditRecord`]
// through the in-module [`RolloutAuditSink`] port (the kernel-local
// audit-port pattern; a separate port here keeps the rollout semantics
// out of the lifecycle transition-record vocabulary). Timestamps come
// ONLY from the injected `Clock` (flexcore-ports); wall time appears
// nowhere.
//
// HONESTY BOUNDS: no rollout has ever run; no cohort exists; no device
// has been authorized, updated, attested, or aborted — the orchestrator
// is WRITTEN-UNRUN control-plane logic against test doubles only. The
// simulation/shadow ceiling applies (conventions §6.7): the orchestrator
// PRODUCES update authorizations as records for the update pipeline; it
// never issues commands, never touches dispatch authority, and the
// never-granted permission set is untouched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use flexcore_ports::clock::Clock;

// --- Honesty statements -------------------------------------------------------------

/// The device-fleet rollout scope statement (the distinction the kernel
/// policy requires; record 06 quotes the kernel policy verbatim).
pub const DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT: &str = "the device-fleet cohort rollout is a \
     NEW mechanism, separate from the kernel plugin-rollout policy (P7.38), which stays \
     cohort/percentage-free BY CONSTRUCTION and byte-identical. The kernel policy governs \
     PLUGIN swapping inside one host composition; this module governs DEVICE software \
     rollout across a fleet, where a bad update can brick devices that cannot be recalled \
     for hands-on recovery. The percentage canary is expressible HERE and ONLY here; its \
     size and the abort thresholds are pre-registered configuration (X-8) before any run, \
     the 5-10% canary band is a DEFAULT RECOMMENDATION (not a sourced figure), and every \
     device authorization requires a PASSED attestation verdict. No rollout has ever run; \
     no fleet exists; no dispatch authority is touched.";

/// The abort-semantics statement (recorded, so the threshold reading is
/// reviewable).
pub const ABORT_SEMANTICS_STATEMENT: &str = "a rollout ABORTS when the observed failure rate \
     of the active cohort EXCEEDS the pre-registered max_failure_rate_permille, evaluated \
     only after at least min_observations outcomes; an equal rate does NOT abort (the \
     threshold is an upper bound, exceeded = abort). Failures are FAILED and ROLLED_BACK \
     outcomes. An abort is terminal: no further authorizations, no further cohort \
     advances, and the abort rides the audit trail with its reason.";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the rollout orchestrator. Every refusal is DISTINCT
/// ([`RolloutConfigError::name`]); nothing is a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolloutConfigError {
    /// The rollout plan is malformed (no cohorts, duplicate or empty
    /// cohort ids, empty target version, non-canary first cohort).
    PlanInvalid { reason: String },
    /// The canary size was not pre-registered — a rollout cannot be
    /// planned without it (X-8: thresholds are registered BEFORE any run
    /// that could fill them).
    CanarySizeNotRegistered,
    /// The registered canary size is outside the DEFAULT band
    /// (500..=1000 basis points = 5–10% of a block). The band is the
    /// pre-registered DEFAULT recommendation, not a sourced figure; an
    /// owner may record a different value only by changing this
    /// configuration band deliberately.
    CanarySizeOutOfRange { value_basis_points: u16 },
    /// The abort thresholds are malformed (rate above 1000 permille, or
    /// zero minimum observations).
    ThresholdsInvalid { reason: String },
    /// The rollout targets the version it already runs — a no-op rollout
    /// is refused, never silently recorded (the kernel NoOpRollout
    /// discipline, mirrored).
    NoOpRollout { version: String },
    /// The named cohort is not part of the plan.
    CohortUnknown { cohort_id: String },
    /// The cohort advance is not to the declared next cohort (there is
    /// no skipping, no flag-flip full rollout).
    CohortNotNext { expected: String, found: String },
    /// The active cohort still has devices without a recorded terminal
    /// outcome — the cohort cannot be left half-judged.
    CohortIncomplete { pending: usize },
    /// The device is not a member of the active cohort (membership is
    /// the fleet registry's, consumed through the directory port).
    DeviceNotInCohort { device_id: String, cohort_id: String },
    /// The device was already authorized in this cohort — a duplicate
    /// authorization is refused.
    DeviceAlreadyAuthorized { device_id: String },
    /// The device has no recorded authorization in the active cohort.
    DeviceNotAuthorized { device_id: String },
    /// The device already has a terminal outcome recorded.
    DeviceAlreadyOutcome { device_id: String },
    /// The attestation verdict was not PASSED (failed, or unavailable) —
    /// the device is NOT authorized (fail-closed: the absence of
    /// evidence is never a pass).
    AttestationNotPassed { device_id: String, detail: String },
    /// The rollout is ABORTED — no further authorization or cohort
    /// advance is possible (terminal, audited).
    RolloutAborted { reason: String },
    /// The cohort directory could not supply the membership — fail-closed:
    /// membership that cannot be read authorizes NOTHING.
    DirectoryUnavailable { reason: String },
    /// The rollout is COMPLETE — every declared cohort was fully judged;
    /// there is no further cohort to advance to.
    RolloutComplete,
    /// The promotion reason is empty (the stage decision is the audit
    /// record).
    ReasonRequired,
    /// The promotion actor is empty.
    ActorRequired,
    /// An input was unusable.
    InvalidInput { reason: String },
}

impl RolloutConfigError {
    /// The stable machine-readable error name (the named-error table
    /// key).
    pub fn name(&self) -> &'static str {
        match self {
            RolloutConfigError::PlanInvalid { .. } => "ROLLOUT_PLAN_INVALID",
            RolloutConfigError::CanarySizeNotRegistered => "ROLLOUT_CANARY_NOT_REGISTERED",
            RolloutConfigError::CanarySizeOutOfRange { .. } => "ROLLOUT_CANARY_OUT_OF_RANGE",
            RolloutConfigError::ThresholdsInvalid { .. } => "ROLLOUT_THRESHOLDS_INVALID",
            RolloutConfigError::NoOpRollout { .. } => "ROLLOUT_NO_OP",
            RolloutConfigError::CohortUnknown { .. } => "ROLLOUT_COHORT_UNKNOWN",
            RolloutConfigError::CohortNotNext { .. } => "ROLLOUT_COHORT_NOT_NEXT",
            RolloutConfigError::CohortIncomplete { .. } => "ROLLOUT_COHORT_INCOMPLETE",
            RolloutConfigError::RolloutComplete => "ROLLOUT_COMPLETE",
            RolloutConfigError::DeviceNotInCohort { .. } => "ROLLOUT_DEVICE_NOT_IN_COHORT",
            RolloutConfigError::DeviceAlreadyAuthorized { .. } => "ROLLOUT_DEVICE_ALREADY_AUTHORIZED",
            RolloutConfigError::DeviceNotAuthorized { .. } => "ROLLOUT_DEVICE_NOT_AUTHORIZED",
            RolloutConfigError::DeviceAlreadyOutcome { .. } => "ROLLOUT_DEVICE_ALREADY_OUTCOME",
            RolloutConfigError::AttestationNotPassed { .. } => "ROLLOUT_ATTESTATION_NOT_PASSED",
            RolloutConfigError::RolloutAborted { .. } => "ROLLOUT_ABORTED",
            RolloutConfigError::DirectoryUnavailable { .. } => "ROLLOUT_DIRECTORY_UNAVAILABLE",
            RolloutConfigError::ReasonRequired => "ROLLOUT_REASON_REQUIRED",
            RolloutConfigError::ActorRequired => "ROLLOUT_ACTOR_REQUIRED",
            RolloutConfigError::InvalidInput { .. } => "ROLLOUT_INPUT_INVALID",
        }
    }
}

impl std::fmt::Display for RolloutConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RolloutConfigError::PlanInvalid { reason } => {
                write!(f, "the rollout plan is invalid: {reason}")
            }
            RolloutConfigError::CanarySizeNotRegistered => write!(
                f,
                "the canary size is not pre-registered — a rollout cannot be planned without \
                 registered parameters (X-8: thresholds are registered BEFORE any run)"
            ),
            RolloutConfigError::CanarySizeOutOfRange { value_basis_points } => write!(
                f,
                "the registered canary size {value_basis_points} basis points is outside the \
                 pre-registered DEFAULT band 500..=1000 (5-10% of a block — a DEFAULT \
                 RECOMMENDATION, not a sourced figure)"
            ),
            RolloutConfigError::ThresholdsInvalid { reason } => {
                write!(f, "the abort thresholds are invalid: {reason}")
            }
            RolloutConfigError::NoOpRollout { version } => write!(
                f,
                "the rollout targets the already-running version '{version}' — a no-op \
                 rollout is refused, never silently recorded"
            ),
            RolloutConfigError::CohortUnknown { cohort_id } => write!(
                f,
                "cohort '{cohort_id}' is not part of the rollout plan"
            ),
            RolloutConfigError::CohortNotNext { expected, found } => write!(
                f,
                "cohort '{found}' is not the declared next cohort (expected '{expected}') — \
                 there is no skipping and no flag-flip full rollout"
            ),
            RolloutConfigError::CohortIncomplete { pending } => write!(
                f,
                "the active cohort still has {pending} device(s) without a terminal outcome — \
                 a cohort cannot be left half-judged"
            ),
            RolloutConfigError::RolloutComplete => write!(
                f,
                "the final cohort was fully judged; the rollout is COMPLETE — no further \
                 cohort exists to advance to"
            ),
            RolloutConfigError::DeviceNotInCohort { device_id, cohort_id } => write!(
                f,
                "device '{device_id}' is not a member of the active cohort '{cohort_id}' \
                 (membership is the fleet registry's, consumed through the directory port)"
            ),
            RolloutConfigError::DeviceNotAuthorized { device_id } => write!(
                f,
                "device '{device_id}' has no recorded authorization in the active cohort"
            ),
            RolloutConfigError::DeviceAlreadyAuthorized { device_id } => write!(
                f,
                "device '{device_id}' is already authorized in this cohort — a duplicate \
                 authorization is refused"
            ),
            RolloutConfigError::DeviceAlreadyOutcome { device_id } => write!(
                f,
                "device '{device_id}' already has a terminal outcome recorded — outcomes are \
                 append-once per device per cohort"
            ),
            RolloutConfigError::AttestationNotPassed { device_id, detail } => write!(
                f,
                "device '{device_id}' is NOT authorized: the attestation verdict is not \
                 PASSED ({detail}) — fail-closed: the absence of evidence is never a pass \
                 (the P18.13 policy, consumed by citation)"
            ),
            RolloutConfigError::RolloutAborted { reason } => write!(
                f,
                "the rollout is ABORTED ({reason}) — no further authorization or cohort \
                 advance is possible"
            ),
            RolloutConfigError::DirectoryUnavailable { reason } => write!(
                f,
                "the cohort directory could not supply membership: {reason} (fail-closed: \
                 membership that cannot be read authorizes NOTHING)"
            ),
            RolloutConfigError::ReasonRequired => {
                write!(f, "a cohort advance requires a non-empty reason (the stage decision \
                           is the audit record)")
            }
            RolloutConfigError::ActorRequired => {
                write!(f, "a cohort advance requires a non-empty actor")
            }
            RolloutConfigError::InvalidInput { reason } => {
                write!(f, "the rollout input is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for RolloutConfigError {}

impl std::error::Error for RolloutConfigError {}

// --- Configuration (the pre-registered parameters) -----------------------------------

/// The pre-registered abort thresholds (X-8: registered BEFORE any run
/// that could fill them). Required at plan time — a plan without them is
/// structurally impossible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbortThresholds {
    /// The maximum tolerated cohort failure rate, in permille (e.g. 50 =
    /// 5%). EXCEEDED (strictly) aborts the rollout; an equal rate does
    /// not (see [`ABORT_SEMANTICS_STATEMENT`]).
    pub max_failure_rate_permille: u32,
    /// The minimum number of recorded outcomes before the rate is
    /// evaluated at all (never aborting on a single unlucky device).
    pub min_observations: u32,
}

impl AbortThresholds {
    /// Validates the thresholds (fail-closed).
    pub fn validate(&self) -> Result<(), RolloutConfigError> {
        if self.max_failure_rate_permille > 1000 {
            return Err(RolloutConfigError::ThresholdsInvalid {
                reason: format!(
                    "max_failure_rate_permille {} exceeds 1000",
                    self.max_failure_rate_permille
                ),
            });
        }
        if self.min_observations == 0 {
            return Err(RolloutConfigError::ThresholdsInvalid {
                reason: "min_observations must be >= 1 (a threshold evaluated on zero \
                         observations aborts on nothing and protects nothing)"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// One cohort in the declared order. The FIRST cohort is the canary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortPlan {
    /// The cohort id (the P18.12 registry's cohort key; consumed by
    /// citation).
    pub cohort_id: String,
    /// Whether this is the canary cohort (exactly cohort 0 must be).
    pub canary: bool,
}

/// The planned device-fleet rollout for ONE target version. The cohorts
/// are advanced in the DECLARED order, canary first; there is no
/// skipping, no parallel cohort activation, and no percentage-of-fleet
/// bulk authorization outside the declared cohorts (the plan structure is
/// the proof).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutPlan {
    /// The target version (a non-empty version string; the format rules
    /// live with the update bundle format on the edge side).
    pub target_version: String,
    /// The declared cohort order (canary first).
    pub cohorts: Vec<CohortPlan>,
    /// The pre-registered abort thresholds.
    pub thresholds: AbortThresholds,
    /// The pre-registered canary size in basis points of the canary
    /// cohort's block (500..=1000 = the 5–10% DEFAULT band; a DEFAULT
    /// RECOMMENDATION, NOT a sourced figure).
    pub canary_basis_points: u16,
}

impl RolloutPlan {
    /// Plans a rollout (fail-closed: unregistered parameters are refused
    /// at plan time, structurally).
    pub fn plan(
        target_version: &str,
        cohorts: Vec<CohortPlan>,
        thresholds: AbortThresholds,
        canary_basis_points: u16,
    ) -> Result<RolloutPlan, RolloutConfigError> {
        thresholds.validate()?;
        if target_version.trim().is_empty() || target_version.len() > 64 {
            return Err(RolloutConfigError::PlanInvalid {
                reason: "target_version must be a non-empty string of at most 64 characters"
                    .to_string(),
            });
        }
        if !(500..=1000).contains(&canary_basis_points) {
            return Err(RolloutConfigError::CanarySizeOutOfRange {
                value_basis_points: canary_basis_points,
            });
        }
        if cohorts.is_empty() {
            return Err(RolloutConfigError::PlanInvalid {
                reason: "the cohort order is empty — a rollout stages through at least one \
                         cohort"
                    .to_string(),
            });
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (index, cohort) in cohorts.iter().enumerate() {
            if cohort.cohort_id.trim().is_empty() || cohort.cohort_id.len() > 64 {
                return Err(RolloutConfigError::PlanInvalid {
                    reason: "every cohort id must be a non-empty string of at most 64 \
                             characters"
                        .to_string(),
                });
            }
            if !seen.insert(cohort.cohort_id.as_str()) {
                return Err(RolloutConfigError::PlanInvalid {
                    reason: format!("cohort '{}' appears twice in the plan", cohort.cohort_id),
                });
            }
            if index == 0 && !cohort.canary {
                return Err(RolloutConfigError::PlanInvalid {
                    reason: "the FIRST cohort is the canary — a rollout cannot start on a \
                             full block"
                        .to_string(),
                });
            }
            if index > 0 && cohort.canary {
                return Err(RolloutConfigError::PlanInvalid {
                    reason: "only the first cohort is the canary — a second canary cohort \
                             is a plan error"
                        .to_string(),
                });
            }
        }
        Ok(RolloutPlan {
            target_version: target_version.to_string(),
            cohorts,
            thresholds,
            canary_basis_points,
        })
    }
}

// --- The consumed ports (by citation: P18.12 registry, P18.13 policy) -------------------

/// The cohort-membership directory port. The composition binds this to
/// the fleet device registry hardened by P18.12 (Agent B's schema —
/// cohort membership; consumed by citation, never redefined here). A
/// failure is fail-closed: membership that cannot be read authorizes
/// NOTHING.
pub trait FleetCohortDirectory {
    /// The member device ids of one cohort (deterministic order; the
    /// registry owns the membership).
    fn cohort_members(&self, cohort_id: &str) -> Result<Vec<String>, String>;
}

/// The per-device attestation verdict shape (the P18.13 policy's output;
/// consumed by citation). `Passed` carries an evidence reference; every
/// other state refuses authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationVerdict {
    /// The device's attestation evidence PASSED the P18.13 verification
    /// policy (the evidence reference rides the audit trail).
    Passed { evidence_reference: String },
    /// The attestation FAILED the policy (the reason rides the refusal).
    Failed { reason: String },
    /// No attestation could be established (hardware-absent fallback
    /// reporting, policy refusal, or an unavailable verifier) — refused:
    /// the absence of evidence is never a pass.
    Unavailable { reason: String },
}

/// One device's terminal update outcome in a cohort (the P18.07 evidence
/// kinds' summary form; the full evidence rides the audit chain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceUpdateOutcome {
    /// The device completed the update healthy (the health check
    /// confirmed the new version).
    Success,
    /// The device's update failed (the explicit failure state rides the
    /// record).
    Failed { failure_state: String },
    /// The device auto-rolled back (the health check failed the new
    /// version; the reason rides the record).
    RolledBack { reason: String },
}

impl DeviceUpdateOutcome {
    /// Whether the outcome counts as a FAILURE for the abort threshold
    /// (Failed and RolledBack both do).
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            DeviceUpdateOutcome::Failed { .. } | DeviceUpdateOutcome::RolledBack { .. }
        )
    }

    /// The stable outcome name (audit lines, snapshots).
    pub fn name(&self) -> &'static str {
        match self {
            DeviceUpdateOutcome::Success => "DEVICE_UPDATE_SUCCESS",
            DeviceUpdateOutcome::Failed { .. } => "DEVICE_UPDATE_FAILED",
            DeviceUpdateOutcome::RolledBack { .. } => "DEVICE_UPDATE_ROLLED_BACK",
        }
    }
}

// --- The audit port (the kernel-local pattern) ------------------------------------------

/// The kinds of P18.06 audit records (SCREAMING_SNAKE stable names; no
/// secret material in any detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutAuditKind {
    /// A rollout plan was created.
    PlanCreated,
    /// A cohort was activated (its members loaded from the directory).
    CohortActivated,
    /// A device was authorized for the update in the active cohort.
    DeviceAuthorized,
    /// A device authorization was refused.
    AuthorizationRefused,
    /// A device outcome was recorded.
    OutcomeRecorded,
    /// The abort threshold was evaluated (every evaluation is recorded —
    /// the threshold discipline is visible, not just the aborts).
    ThresholdEvaluated,
    /// The rollout ABORTED (terminal).
    RolloutAborted,
    /// A cohort advance was recorded.
    CohortAdvanced,
    /// A cohort advance was refused.
    CohortAdvanceRefused,
}

impl RolloutAuditKind {
    /// The stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            RolloutAuditKind::PlanCreated => "FLEET_ROLLOUT_PLAN_CREATED",
            RolloutAuditKind::CohortActivated => "FLEET_ROLLOUT_COHORT_ACTIVATED",
            RolloutAuditKind::DeviceAuthorized => "FLEET_ROLLOUT_DEVICE_AUTHORIZED",
            RolloutAuditKind::AuthorizationRefused => "FLEET_ROLLOUT_AUTHORIZATION_REFUSED",
            RolloutAuditKind::OutcomeRecorded => "FLEET_ROLLOUT_OUTCOME_RECORDED",
            RolloutAuditKind::ThresholdEvaluated => "FLEET_ROLLOUT_THRESHOLD_EVALUATED",
            RolloutAuditKind::RolloutAborted => "FLEET_ROLLOUT_ABORTED",
            RolloutAuditKind::CohortAdvanced => "FLEET_ROLLOUT_COHORT_ADVANCED",
            RolloutAuditKind::CohortAdvanceRefused => "FLEET_ROLLOUT_COHORT_ADVANCE_REFUSED",
        }
    }
}

/// One P18.06 audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutAuditRecord {
    /// When the event was recorded (injected-clock millis).
    pub timestamp_millis: i64,
    pub kind: RolloutAuditKind,
    /// The subject (cohort id, device id, or the target-version subject).
    pub subject: String,
    /// Human-readable detail (identifiers and refusal names — never key
    /// material, never a secret value).
    pub detail: String,
}

/// The P18.06 audit sink port (application-local, the kernel-port
/// pattern). Implementations MUST be append-only: records are never
/// rewritten or removed.
pub trait RolloutAuditSink {
    fn record(&self, record: RolloutAuditRecord);
}

/// An append-only in-memory [`RolloutAuditSink`] — the deterministic
/// collector used by tests and in-process compositions.
#[derive(Default)]
pub struct MemoryRolloutAuditSink {
    records: Mutex<Vec<RolloutAuditRecord>>,
}

impl MemoryRolloutAuditSink {
    pub fn new() -> MemoryRolloutAuditSink {
        MemoryRolloutAuditSink { records: Mutex::new(Vec::new()) }
    }

    /// The records in emission order (append-only).
    pub fn records(&self) -> Vec<RolloutAuditRecord> {
        // Invariant note (FRV1-R1-010): the only `expect` panics on this
        // sink are mutex-POISONING paths — the mutex guards a plain
        // append-only Vec and no caller input reaches a panic inside the
        // critical sections; the crate policy is that a poisoned
        // coordination primitive is not recoverable state (never a silent
        // fallback).
        self.records
            .lock()
            .expect("poisoned rollout audit sink mutex")
            .clone()
    }
}

impl RolloutAuditSink for MemoryRolloutAuditSink {
    fn record(&self, record: RolloutAuditRecord) {
        // Invariant note (FRV1-R1-010): mutex-poisoning expect only — see
        // `MemoryRolloutAuditSink::records` for the recorded policy.
        self.records
            .lock()
            .expect("poisoned rollout audit sink mutex")
            .push(record);
    }
}

/// Shared-sink adapter: an `Arc` around any sink implements the port, so
/// tests and compositions can keep a handle while the orchestrator holds
/// a boxed clone.
impl<T: RolloutAuditSink + Send + Sync> RolloutAuditSink for std::sync::Arc<T> {
    fn record(&self, record: RolloutAuditRecord) {
        T::record(self, record)
    }
}

// --- The orchestrator -------------------------------------------------------------------

/// The rollout phase (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutPhase {
    /// Planned; the canary has not been activated yet.
    NotStarted,
    /// A cohort is active (canary or block).
    InProgress,
    /// Every declared cohort completed.
    Complete,
    /// The rollout aborted (terminal; the reason rides the audit trail
    /// and the snapshot).
    Aborted,
}

impl RolloutPhase {
    /// The stable phase name (snapshots).
    pub fn name(self) -> &'static str {
        match self {
            RolloutPhase::NotStarted => "ROLLOUT_NOT_STARTED",
            RolloutPhase::InProgress => "ROLLOUT_IN_PROGRESS",
            RolloutPhase::Complete => "ROLLOUT_COMPLETE",
            RolloutPhase::Aborted => "ROLLOUT_ABORTED",
        }
    }
}

/// The per-cohort runtime progress.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CohortProgress {
    cohort_id: String,
    canary: bool,
    /// The members as supplied by the directory at activation (the
    /// registry's membership, consumed by citation).
    members: Vec<String>,
    /// The authorized devices (authorization precedes any outcome).
    authorized: BTreeSet<String>,
    /// The terminal outcomes (append-once per device).
    outcomes: BTreeMap<String, DeviceUpdateOutcome>,
}

/// The staged rollout orchestrator over ONE plan (P18.06). Holds no
/// registry, no attestation verifier, and no update pipeline — those are
/// the injected ports (`FleetCohortDirectory`, the `AttestationVerdict`
/// supplied per device, and the update path's own gates). It PRODUCES
/// authorizations as audited records; it never issues a command and
/// never touches dispatch authority.
pub struct RolloutOrchestrator {
    plan: RolloutPlan,
    phase: RolloutPhase,
    active_index: Option<usize>,
    progress: Vec<CohortProgress>,
    abort_reason: Option<String>,
    audit: Box<dyn RolloutAuditSink + Send>,
    clock: Box<dyn Clock + Send + Sync>,
}

impl RolloutOrchestrator {
    /// Starts an orchestrator over a validated plan (the plan is the
    /// pre-registered parameter set; X-8 is structural).
    pub fn new(
        plan: RolloutPlan,
        audit: Box<dyn RolloutAuditSink + Send>,
        clock: Box<dyn Clock + Send + Sync>,
    ) -> Self {
        let subject = format!(
            "version={} cohorts={} canary_bp={}",
            plan.target_version,
            plan.cohorts.len(),
            plan.canary_basis_points,
        );
        let mut orchestrator = Self {
            phase: RolloutPhase::NotStarted,
            active_index: None,
            progress: Vec::new(),
            abort_reason: None,
            audit,
            clock,
        };
        orchestrator.emit(
            RolloutAuditKind::PlanCreated,
            subject,
            format!(
                "thresholds: max_failure_rate_permille={} min_observations={} — \
                 pre-registered BEFORE any run (X-8); {}",
                plan.thresholds.max_failure_rate_permille,
                plan.thresholds.min_observations,
                DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT,
            ),
        );
        orchestrator
    }

    /// The plan (read view).
    pub fn plan(&self) -> &RolloutPlan {
        &self.plan
    }

    /// The current phase.
    pub fn phase(&self) -> RolloutPhase {
        self.phase
    }

    /// The abort reason, when aborted.
    pub fn abort_reason(&self) -> Option<&str> {
        self.abort_reason.as_deref()
    }

    /// Activates the FIRST cohort (the canary). The cohort membership is
    /// loaded through the directory port (fail-closed on a directory
    /// failure). There is no constructor that starts more than one cohort
    /// at once.
    pub fn start(
        &mut self,
        directory: &dyn FleetCohortDirectory,
    ) -> Result<String, RolloutConfigError> {
        if self.phase != RolloutPhase::NotStarted {
            return Err(RolloutConfigError::InvalidInput {
                reason: format!(
                    "the rollout is already {} — start is legal only from NotStarted",
                    self.phase.name()
                ),
            });
        }
        let canary_id = self.plan.cohorts[0].cohort_id.clone();
        let members = self
            .load_members(&canary_id, directory)
            .map_err(|reason| RolloutConfigError::DirectoryUnavailable { reason })?;
        self.progress.push(CohortProgress {
            cohort_id: canary_id.clone(),
            canary: true,
            members,
            authorized: BTreeSet::new(),
            outcomes: BTreeMap::new(),
        });
        self.active_index = Some(0);
        self.phase = RolloutPhase::InProgress;
        let member_count = self.progress[0].members.len();
        self.emit(
            RolloutAuditKind::CohortActivated,
            canary_id.clone(),
            format!(
                "the CANARY cohort activated ({} member(s) from the fleet registry, P18.12 \
                 schema consumed by citation); canary size band {} bp (a DEFAULT \
                 RECOMMENDATION, not a sourced figure)",
                member_count, self.plan.canary_basis_points,
            ),
        );
        Ok(canary_id)
    }

    /// Authorizes ONE device for the update in the ACTIVE cohort. The
    /// order of checks (recorded): rollout not aborted → phase
    /// InProgress → cohort membership via the directory (fail-closed) →
    /// attestation PASSED (fail-closed on Failed/Unavailable) → not
    /// already authorized. The authorization is a RECORD the update
    /// pipeline consumes — never a command, never a dispatch action.
    pub fn authorize_device(
        &mut self,
        directory: &dyn FleetCohortDirectory,
        device_id: &str,
        attestation: &AttestationVerdict,
    ) -> Result<(), RolloutConfigError> {
        let refusal = |error: RolloutConfigError| -> RolloutConfigError {
            self.emit(
                RolloutAuditKind::AuthorizationRefused,
                device_id.to_string(),
                format!("authorization refused ({}): {error}", error.name()),
            );
            error
        };
        if self.phase == RolloutPhase::Aborted {
            return Err(refusal(RolloutConfigError::RolloutAborted {
                reason: self.abort_reason.clone().unwrap_or_default(),
            }));
        }
        if self.phase != RolloutPhase::InProgress {
            return Err(refusal(RolloutConfigError::InvalidInput {
                reason: format!("no active cohort (phase {})", self.phase.name()),
            }));
        }
        if device_id.trim().is_empty() {
            return Err(refusal(RolloutConfigError::InvalidInput {
                reason: "device_id is mandatory".to_string(),
            }));
        }
        let index = self.active_index.expect("phase InProgress implies an active index");
        let cohort_id = self.progress[index].cohort_id.clone();
        // Membership via the directory, re-checked at authorization time
        // (the registry's membership is authoritative; consumed by
        // citation from the P18.12 schema).
        let members = self.load_members(&cohort_id, directory).map_err(|error| {
            refusal(RolloutConfigError::DirectoryUnavailable { reason: error })
        })?;
        if !members.iter().any(|m| m == device_id) {
            return Err(refusal(RolloutConfigError::DeviceNotInCohort {
                device_id: device_id.to_string(),
                cohort_id,
            }));
        }
        // Attestation BEFORE authorization (the P18.13 policy, consumed
        // by citation).
        match attestation {
            AttestationVerdict::Passed { evidence_reference } => {
                if evidence_reference.trim().is_empty() {
                    return Err(refusal(RolloutConfigError::AttestationNotPassed {
                        device_id: device_id.to_string(),
                        detail: "the attestation verdict carries no evidence reference"
                            .to_string(),
                    }));
                }
            }
            AttestationVerdict::Failed { reason } => {
                return Err(refusal(RolloutConfigError::AttestationNotPassed {
                    device_id: device_id.to_string(),
                    detail: format!("FAILED: {reason}"),
                }));
            }
            AttestationVerdict::Unavailable { reason } => {
                return Err(refusal(RolloutConfigError::AttestationNotPassed {
                    device_id: device_id.to_string(),
                    detail: format!("UNAVAILABLE: {reason}"),
                }));
            }
        }
        // Not already authorized (append-once per cohort).
        if self.progress[index].authorized.contains(device_id) {
            return Err(refusal(RolloutConfigError::DeviceAlreadyAuthorized {
                device_id: device_id.to_string(),
            }));
        }
        self.progress[index].authorized.insert(device_id.to_string());
        let evidence = match attestation {
            AttestationVerdict::Passed { evidence_reference } => evidence_reference.clone(),
            _ => String::new(),
        };
        self.emit(
            RolloutAuditKind::DeviceAuthorized,
            format!("{}/{}", self.progress[index].cohort_id, device_id),
            format!(
                "device authorized for version {} (attestation evidence: {evidence}) — an \
                 authorization RECORD for the update pipeline, never a command",
                self.plan.target_version,
            ),
        );
        Ok(())
    }

    /// Records ONE device's terminal outcome in the active cohort and
    /// evaluates the pre-registered abort threshold. The evaluation runs
    /// after EVERY outcome (every evaluation is audited — the threshold
    /// discipline is visible, not just the aborts).
    pub fn record_outcome(
        &mut self,
        device_id: &str,
        outcome: DeviceUpdateOutcome,
    ) -> Result<(), RolloutConfigError> {
        if self.phase == RolloutPhase::Aborted {
            return Err(RolloutConfigError::RolloutAborted {
                reason: self.abort_reason.clone().unwrap_or_default(),
            });
        }
        if self.phase != RolloutPhase::InProgress {
            return Err(RolloutConfigError::InvalidInput {
                reason: format!("no active cohort (phase {})", self.phase.name()),
            });
        }
        let index = self.active_index.expect("phase InProgress implies an active index");
        if !self.progress[index].authorized.contains(device_id) {
            return Err(RolloutConfigError::DeviceNotAuthorized {
                device_id: device_id.to_string(),
            });
        }
        if self.progress[index].outcomes.contains_key(device_id) {
            return Err(RolloutConfigError::DeviceAlreadyOutcome {
                device_id: device_id.to_string(),
            });
        }
        self.emit(
            RolloutAuditKind::OutcomeRecorded,
            format!("{}/{}", self.progress[index].cohort_id, device_id),
            format!("outcome {} for version {}", outcome.name(), self.plan.target_version),
        );
        self.progress[index].outcomes.insert(device_id.to_string(), outcome);
        self.evaluate_threshold(index);
        Ok(())
    }

    /// Advances to the NEXT declared cohort. Legal only when the active
    /// cohort is fully judged (every member has a terminal outcome), the
    /// rollout is not aborted, and the advance follows the declared
    /// order (it does — the index is internal; there is no API to skip).
    pub fn advance_cohort(
        &mut self,
        directory: &dyn FleetCohortDirectory,
        actor: &str,
        reason: &str,
    ) -> Result<String, RolloutConfigError> {
        let subject = format!("version={}", self.plan.target_version);
        if actor.trim().is_empty() {
            let error = RolloutConfigError::ActorRequired;
            self.emit(
                RolloutAuditKind::CohortAdvanceRefused,
                subject,
                format!("advance refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        if reason.trim().is_empty() {
            let error = RolloutConfigError::ReasonRequired;
            self.emit(
                RolloutAuditKind::CohortAdvanceRefused,
                subject,
                format!("advance refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        if self.phase == RolloutPhase::Aborted {
            return Err(RolloutConfigError::RolloutAborted {
                reason: self.abort_reason.clone().unwrap_or_default(),
            });
        }
        let Some(index) = self.active_index else {
            return Err(RolloutConfigError::InvalidInput {
                reason: format!("no active cohort (phase {})", self.phase.name()),
            });
        };
        let pending = self.progress[index].members.len()
            - self.progress[index].outcomes.len();
        if pending > 0 {
            let error = RolloutConfigError::CohortIncomplete { pending };
            self.emit(
                RolloutAuditKind::CohortAdvanceRefused,
                subject,
                format!("advance refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        let next_index = index + 1;
        if next_index >= self.plan.cohorts.len() {
            self.phase = RolloutPhase::Complete;
            self.active_index = None;
            self.emit(
                RolloutAuditKind::CohortAdvanced,
                subject,
                format!(
                    "the FINAL cohort ('{}') fully judged by '{actor}': {reason} — rollout \
                     COMPLETE",
                    self.progress[index].cohort_id,
                ),
            );
            return Err(RolloutConfigError::RolloutComplete);
        }
        let next_id = self.plan.cohorts[next_index].cohort_id.clone();
        let members = self
            .load_members(&next_id, directory)
            .map_err(|reason| RolloutConfigError::DirectoryUnavailable { reason })?;
        self.progress.push(CohortProgress {
            cohort_id: next_id.clone(),
            canary: false,
            members,
            authorized: BTreeSet::new(),
            outcomes: BTreeMap::new(),
        });
        self.active_index = Some(next_index);
        self.emit(
            RolloutAuditKind::CohortAdvanced,
            next_id.clone(),
            format!(
                "advanced '{}' -> '{}' by '{actor}' ({} member(s)): {reason} — the declared \
                 order, no skipping",
                self.progress[index].cohort_id,
                next_id,
                self.progress[next_index].members.len(),
            ),
        );
        Ok(next_id)
    }

    /// The fleet-state snapshot (P18.06: fleet-state visualization data).
    /// Plain data for the console adapter (P6.25-style surfacing); the
    /// rendering is the adapter's concern.
    pub fn fleet_snapshot(&self) -> FleetRolloutSnapshot {
        let cohorts = self
            .progress
            .iter()
            .map(|p| {
                let succeeded = p
                    .outcomes
                    .values()
                    .filter(|o| !o.is_failure())
                    .count();
                let failed = p
                    .outcomes
                    .values()
                    .filter(|o| matches!(o, DeviceUpdateOutcome::Failed { .. }))
                    .count();
                let rolled_back = p
                    .outcomes
                    .values()
                    .filter(|o| matches!(o, DeviceUpdateOutcome::RolledBack { .. }))
                    .count();
                CohortSnapshot {
                    cohort_id: p.cohort_id.clone(),
                    canary: p.canary,
                    member_count: p.members.len(),
                    authorized_count: p.authorized.len(),
                    succeeded,
                    failed,
                    rolled_back,
                    pending: p.members.len() - p.outcomes.len(),
                }
            })
            .collect();
        FleetRolloutSnapshot {
            target_version: self.plan.target_version.clone(),
            phase: self.phase,
            canary_basis_points: self.plan.canary_basis_points,
            cohorts,
            abort_reason: self.abort_reason.clone(),
        }
    }

    fn load_members(
        &self,
        cohort_id: &str,
        directory: &dyn FleetCohortDirectory,
    ) -> Result<Vec<String>, String> {
        let members = directory
            .cohort_members(cohort_id)
            .map_err(|error| format!("the cohort directory refused '{cohort_id}': {error}"))?;
        if members.is_empty() {
            return Err(format!(
                "the cohort directory returned NO members for '{cohort_id}' — activating an \
                 empty cohort is refused (fail-closed)"
            ));
        }
        for member in &members {
            if member.trim().is_empty() {
                return Err(format!(
                    "the cohort directory returned an empty device id for '{cohort_id}' — \
                     refused (fail-closed)"
                ));
            }
        }
        let unique: BTreeSet<&String> = members.iter().collect();
        if unique.len() != members.len() {
            return Err(format!(
                "the cohort directory returned DUPLICATE device ids for '{cohort_id}' — \
                 refused (fail-closed)"
            ));
        }
        Ok(members)
    }

    /// Evaluates the pre-registered abort threshold over the active
    /// cohort's outcomes (after every outcome; every evaluation audited).
    /// Exceeded (strictly) aborts; an equal rate does not
    /// ([`ABORT_SEMANTICS_STATEMENT`]).
    fn evaluate_threshold(&mut self, index: usize) {
        if self.phase != RolloutPhase::InProgress {
            return;
        }
        let observed = self.progress[index].outcomes.len() as u32;
        let failures = self.progress[index]
            .outcomes
            .values()
            .filter(|o| o.is_failure())
            .count() as u32;
        let thresholds = self.plan.thresholds.clone();
        let cohort_id = self.progress[index].cohort_id.clone();
        if observed < thresholds.min_observations {
            self.emit(
                RolloutAuditKind::ThresholdEvaluated,
                cohort_id,
                format!(
                    "threshold NOT evaluated: {observed} outcome(s) < min_observations {}",
                    thresholds.min_observations
                ),
            );
            return;
        }
        let rate_permille = (failures * 1000) / observed.max(1);
        let aborts = rate_permille > thresholds.max_failure_rate_permille;
        self.emit(
            RolloutAuditKind::ThresholdEvaluated,
            cohort_id.clone(),
            format!(
                "threshold evaluated: failures={failures} observed={observed} rate={rate_permille} \
                 permille vs max {} permille -> {}",
                thresholds.max_failure_rate_permille,
                if aborts { "ABORT" } else { "continue" },
            ),
        );
        if aborts {
            let reason = format!(
                "the cohort '{cohort_id}' failure rate {rate_permille} permille EXCEEDED the \
                 pre-registered max {} permille (failures={failures} of {observed})",
                thresholds.max_failure_rate_permille,
            );
            self.phase = RolloutPhase::Aborted;
            self.active_index = None;
            self.abort_reason = Some(reason.clone());
            self.emit(
                RolloutAuditKind::RolloutAborted,
                cohort_id,
                format!(
                    "{reason} — TERMINAL: no further authorization, no further cohort \
                     advance; the cohort's devices keep their last judged outcome and the \
                     operators decide the recovery (the evidence rides the P18.07 pack)",
                ),
            );
        }
    }

    fn emit(&self, kind: RolloutAuditKind, subject: String, detail: String) {
        self.audit.record(RolloutAuditRecord {
            timestamp_millis: self.now_millis(),
            kind,
            subject,
            detail,
        });
    }

    fn now_millis(&self) -> i64 {
        i64::try_from(self.clock.now_ingest_time().unix_millis()).unwrap_or(i64::MAX)
    }
}

// --- The fleet-state snapshot (visualization data) ----------------------------------------

/// One cohort's row in the fleet-state snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortSnapshot {
    pub cohort_id: String,
    pub canary: bool,
    /// The members the directory supplied at activation.
    pub member_count: usize,
    pub authorized_count: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub rolled_back: usize,
    /// Members without a terminal outcome yet.
    pub pending: usize,
}

/// The fleet-state snapshot (P18.06: fleet-state visualization data).
/// Plain data; the console adapter renders it (no rendering here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRolloutSnapshot {
    pub target_version: String,
    pub phase: RolloutPhase,
    /// The pre-registered canary size band (basis points).
    pub canary_basis_points: u16,
    pub cohorts: Vec<CohortSnapshot>,
    /// The abort reason, when the rollout aborted.
    pub abort_reason: Option<String>,
}

impl FleetRolloutSnapshot {
    /// The one-line summary (console/log text; no secrets).
    pub fn summary(&self) -> String {
        let cohorts: Vec<String> = self
            .cohorts
            .iter()
            .map(|c| {
                format!(
                    "{}({}/{} ok, {} failed, {} rolled back, {} pending)",
                    c.cohort_id, c.succeeded, c.member_count, c.failed, c.rolled_back, c.pending,
                )
            })
            .collect();
        match &self.abort_reason {
            Some(reason) => format!(
                "rollout v={} {} canary_bp={} cohorts=[{}] ABORTED: {reason}",
                self.target_version,
                self.phase.name(),
                self.canary_basis_points,
                cohorts.join(", "),
            ),
            None => format!(
                "rollout v={} {} canary_bp={} cohorts=[{}]",
                self.target_version,
                self.phase.name(),
                self.canary_basis_points,
                cohorts.join(", "),
            ),
        }
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::time_model::IngestTime;

    struct FixedClock(i64);
    impl Clock for FixedClock {
        fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
            flexcore_contracts::time_model::EventTime::from_unix_millis(self.0)
                .expect("valid instant")
        }
        fn now_ingest_time(&self) -> IngestTime {
            IngestTime::from_unix_millis(self.0).expect("valid instant")
        }
    }

    /// The fleet-registry stand-in: a fixed membership map (the P18.12
    /// registry consumed by citation; the test binds the port to a
    /// deterministic fake).
    struct FakeDirectory {
        cohorts: BTreeMap<String, Vec<String>>,
    }

    impl FleetCohortDirectory for FakeDirectory {
        fn cohort_members(&self, cohort_id: &str) -> Result<Vec<String>, String> {
            self.cohorts
                .get(cohort_id)
                .cloned()
                .ok_or_else(|| format!("unknown cohort '{cohort_id}'"))
        }
    }

    fn plan_params() -> (Vec<CohortPlan>, AbortThresholds, u16) {
        (
            vec![
                CohortPlan { cohort_id: "canary-a".to_string(), canary: true },
                CohortPlan { cohort_id: "block-1".to_string(), canary: false },
            ],
            AbortThresholds { max_failure_rate_permille: 200, min_observations: 3 },
            500,
        )
    }

    fn directory() -> FakeDirectory {
        FakeDirectory {
            cohorts: BTreeMap::from([
                (
                    "canary-a".to_string(),
                    vec!["device-1".to_string(), "device-2".to_string(), "device-3".to_string()],
                ),
                (
                    "block-1".to_string(),
                    vec!["device-4".to_string(), "device-5".to_string()],
                ),
            ]),
        }
    }

    fn orchestrator() -> (RolloutOrchestrator, std::sync::Arc<MemoryRolloutAuditSink>) {
        let (cohorts, thresholds, canary_bp) = plan_params();
        let plan = RolloutPlan::plan("2.0.0", cohorts, thresholds, canary_bp)
            .expect("plan validates");
        let sink = std::sync::Arc::new(MemoryRolloutAuditSink::new());
        let boxed: Box<dyn RolloutAuditSink + Send> = Box::new(sink.clone());
        (
            RolloutOrchestrator::new(
                plan,
                boxed,
                Box::new(FixedClock(1_700_000_000_000)),
            ),
            sink,
        )
    }

    fn passed_attestation(device: &str) -> AttestationVerdict {
        AttestationVerdict::Passed {
            evidence_reference: format!("attestation/{device}/evidence-1"),
        }
    }

    #[test]
    fn the_plan_refuses_unregistered_parameters() {
        // No canary size registered: structurally impossible (the value
        // is a required parameter, not an Option the caller can skip).
        let (cohorts, thresholds, _) = plan_params();
        assert!(matches!(
            RolloutPlan::plan("2.0.0", cohorts.clone(), thresholds.clone(), 499),
            Err(RolloutConfigError::CanarySizeOutOfRange { value_basis_points: 499 })
        ));
        assert!(matches!(
            RolloutPlan::plan("2.0.0", cohorts.clone(), thresholds.clone(), 1001),
            Err(RolloutConfigError::CanarySizeOutOfRange { .. })
        ));
        // Thresholds above 1000 permille or zero observations are refused.
        assert!(matches!(
            RolloutPlan::plan(
                "2.0.0",
                cohorts.clone(),
                AbortThresholds { max_failure_rate_permille: 1001, min_observations: 3 },
                500,
            ),
            Err(RolloutConfigError::ThresholdsInvalid { .. })
        ));
        assert!(matches!(
            RolloutPlan::plan(
                "2.0.0",
                cohorts.clone(),
                AbortThresholds { max_failure_rate_permille: 200, min_observations: 0 },
                500,
            ),
            Err(RolloutConfigError::ThresholdsInvalid { .. })
        ));
        // A non-canary first cohort is refused.
        let non_canary_first = vec![
            CohortPlan { cohort_id: "block-0".to_string(), canary: false },
            CohortPlan { cohort_id: "block-1".to_string(), canary: false },
        ];
        assert!(matches!(
            RolloutPlan::plan("2.0.0", non_canary_first, thresholds, 500),
            Err(RolloutConfigError::PlanInvalid { .. })
        ));
        // A duplicate cohort is refused.
        assert!(matches!(
            RolloutPlan::plan(
                "2.0.0",
                vec![
                    CohortPlan { cohort_id: "canary-a".to_string(), canary: true },
                    CohortPlan { cohort_id: "canary-a".to_string(), canary: false },
                ],
                AbortThresholds { max_failure_rate_permille: 200, min_observations: 3 },
                500,
            ),
            Err(RolloutConfigError::PlanInvalid { .. })
        ));
    }

    #[test]
    fn authorization_requires_membership_attestation_and_an_active_cohort() {
        let (mut orchestrator, sink) = orchestrator();
        let dir = directory();
        // Not started: no authorizations.
        assert!(matches!(
            orchestrator.authorize_device(&dir, "device-1", &passed_attestation("device-1")),
            Err(RolloutConfigError::InvalidInput { .. })
        ));
        orchestrator.start(&dir).expect("canary starts");
        // Attestation Failed refuses.
        assert!(matches!(
            orchestrator.authorize_device(
                &dir,
                "device-1",
                &AttestationVerdict::Failed { reason: "measurement mismatch".to_string() },
            ),
            Err(RolloutConfigError::AttestationNotPassed { .. })
        ));
        // Attestation Unavailable refuses (fail-closed: no evidence is
        // never a pass).
        assert!(matches!(
            orchestrator.authorize_device(
                &dir,
                "device-1",
                &AttestationVerdict::Unavailable { reason: "no verifier composed".to_string() },
            ),
            Err(RolloutConfigError::AttestationNotPassed { .. })
        ));
        // Non-membership refuses (the registry's membership is
        // authoritative).
        assert!(matches!(
            orchestrator.authorize_device(&dir, "device-4", &passed_attestation("device-4")),
            Err(RolloutConfigError::DeviceNotInCohort { .. })
        ));
        // A PASSED attestation authorizes.
        orchestrator
            .authorize_device(&dir, "device-1", &passed_attestation("device-1"))
            .expect("authorized");
        // A duplicate authorization refuses.
        assert!(matches!(
            orchestrator.authorize_device(&dir, "device-1", &passed_attestation("device-1")),
            Err(RolloutConfigError::DeviceAlreadyAuthorized { .. })
        ));
        // The audit trail carries the refusals and the authorization.
        let kinds: Vec<&str> = sink
            .records()
            .iter()
            .map(|r| r.kind.name())
            .collect();
        assert!(kinds.contains(&"FLEET_ROLLOUT_AUTHORIZATION_REFUSED"));
        assert!(kinds.contains(&"FLEET_ROLLOUT_DEVICE_AUTHORIZED"));
        assert!(kinds.contains(&"FLEET_ROLLOUT_COHORT_ACTIVATED"));
    }

    #[test]
    fn a_directory_failure_refuses_authorization_fail_closed() {
        struct BrokenDirectory;
        impl FleetCohortDirectory for BrokenDirectory {
            fn cohort_members(&self, _cohort_id: &str) -> Result<Vec<String>, String> {
                Err("the registry is unreachable".to_string())
            }
        }
        let (mut orchestrator, _sink) = orchestrator();
        // Even start() refuses on a broken directory (fail-closed).
        assert!(matches!(
            orchestrator.start(&BrokenDirectory),
            Err(RolloutConfigError::DirectoryUnavailable { .. })
        ));
    }

    #[test]
    fn an_empty_or_duplicate_membership_refuses() {
        let (mut orchestrator, _sink) = orchestrator();
        let dir = FakeDirectory {
            cohorts: BTreeMap::from([
                ("canary-a".to_string(), Vec::new()),
                ("block-1".to_string(), vec!["device-4".to_string(), "device-4".to_string()]),
            ]),
        };
        assert!(matches!(
            orchestrator.start(&dir),
            Err(RolloutConfigError::DirectoryUnavailable { .. })
        ));
    }

    #[test]
    fn the_abort_threshold_halts_the_rollout_when_exceeded() {
        let (mut orchestrator, sink) = orchestrator();
        let dir = directory();
        orchestrator.start(&dir).expect("canary starts");
        for device in ["device-1", "device-2", "device-3"] {
            orchestrator
                .authorize_device(&dir, device, &passed_attestation(device))
                .expect("authorized");
        }
        // Three failures of three: rate 1000 permille > 200 permille,
        // after min_observations=3 → ABORT.
        for device in ["device-1", "device-2"] {
            orchestrator
                .record_outcome(device, DeviceUpdateOutcome::Failed {
                    failure_state: "UPDATE_FAILURE_INSTALL_FAILED".to_string(),
                })
                .expect("recorded");
        }
        // Still InProgress: only 2 observations (< min_observations).
        assert_eq!(orchestrator.phase(), RolloutPhase::InProgress);
        orchestrator
            .record_outcome("device-3", DeviceUpdateOutcome::RolledBack {
                reason: "health check failed".to_string(),
            })
            .expect("recorded");
        // The third outcome crosses min_observations and exceeds the
        // threshold: ABORTED.
        assert_eq!(orchestrator.phase(), RolloutPhase::Aborted);
        assert!(orchestrator.abort_reason().is_some());
        // Terminal: no further authorization, no further advance.
        assert!(matches!(
            orchestrator.authorize_device(&dir, "device-1", &passed_attestation("device-1")),
            Err(RolloutConfigError::RolloutAborted { .. })
        ));
        assert!(matches!(
            orchestrator.advance_cohort(&dir, "operator", "try block-1"),
            Err(RolloutConfigError::RolloutAborted { .. })
        ));
        // Every threshold evaluation was audited, plus the abort.
        let kinds: Vec<&str> = sink
            .records()
            .iter()
            .map(|r| r.kind.name())
            .collect();
        assert!(kinds.iter().filter(|k| **k == "FLEET_ROLLOUT_THRESHOLD_EVALUATED").count() >= 3);
        assert!(kinds.contains(&"FLEET_ROLLOUT_ABORTED"));
        // The snapshot shows the abort with the counts.
        let snapshot = orchestrator.fleet_snapshot();
        assert_eq!(snapshot.phase, RolloutPhase::Aborted);
        assert!(snapshot.summary().contains("ABORTED"));
    }

    #[test]
    fn an_equal_rate_does_not_abort_and_healthy_outcomes_continue() {
        let (cohorts, thresholds, canary_bp) = plan_params();
        // max 200 permille = 2 failures per 10 observed abort; 1 in 10
        // (100 permille) continues.
        let plan = RolloutPlan::plan("2.0.0", cohorts, thresholds, canary_bp)
            .expect("plan validates");
        let sink = std::sync::Arc::new(MemoryRolloutAuditSink::new());
        let boxed: Box<dyn RolloutAuditSink + Send> = Box::new(sink.clone());
        let mut orchestrator =
            RolloutOrchestrator::new(plan, boxed, Box::new(FixedClock(1_700_000_000_000)));
        let dir = FakeDirectory {
            cohorts: BTreeMap::from([(
                "canary-a".to_string(),
                (1..=10).map(|i| format!("device-{i}")).collect::<Vec<String>>(),
            )]),
        };
        orchestrator.start(&dir).expect("canary starts");
        for i in 1..=10 {
            let device = format!("device-{i}");
            orchestrator
                .authorize_device(&dir, &device, &passed_attestation(&device))
                .expect("authorized");
            let outcome = if i == 1 {
                DeviceUpdateOutcome::Failed { failure_state: "x".to_string() }
            } else {
                DeviceUpdateOutcome::Success
            };
            orchestrator.record_outcome(&device, outcome).expect("recorded");
        }
        // 100 permille < 200 permille: the rollout continues.
        assert_eq!(orchestrator.phase(), RolloutPhase::InProgress);
        let snapshot = orchestrator.fleet_snapshot();
        assert_eq!(snapshot.cohorts[0].succeeded, 9);
        assert_eq!(snapshot.cohorts[0].failed, 1);
    }

    #[test]
    fn cohorts_advance_in_the_declared_order_and_complete() {
        let (mut orchestrator, sink) = orchestrator();
        let dir = directory();
        orchestrator.start(&dir).expect("canary starts");
        // Advance with an unfinished cohort refuses (half-judged).
        orchestrator
            .authorize_device(&dir, "device-1", &passed_attestation("device-1"))
            .expect("authorized");
        assert!(matches!(
            orchestrator.advance_cohort(&dir, "operator", "advance"),
            Err(RolloutConfigError::CohortIncomplete { pending: 3 })
        ));
        // Judge every canary device.
        for device in ["device-1", "device-2", "device-3"] {
            orchestrator
                .record_outcome(device, DeviceUpdateOutcome::Success)
                .expect("recorded");
        }
        let next = orchestrator
            .advance_cohort(&dir, "operator", "canary healthy; promote block-1")
            .expect("advances");
        assert_eq!(next, "block-1");
        // The block's devices authorize against the NEW cohort.
        assert!(matches!(
            orchestrator.authorize_device(&dir, "device-1", &passed_attestation("device-1")),
            Err(RolloutConfigError::DeviceNotInCohort { .. })
        ));
        orchestrator
            .authorize_device(&dir, "device-4", &passed_attestation("device-4"))
            .expect("authorized");
        orchestrator
            .authorize_device(&dir, "device-5", &passed_attestation("device-5"))
            .expect("authorized");
        orchestrator
            .record_outcome("device-4", DeviceUpdateOutcome::Success)
            .expect("recorded");
        orchestrator
            .record_outcome("device-5", DeviceUpdateOutcome::Success)
            .expect("recorded");
        // The final advance completes (the named RolloutComplete marker;
        // the phase is terminal).
        let result = orchestrator.advance_cohort(&dir, "operator", "block-1 healthy");
        assert!(matches!(result, Err(RolloutConfigError::RolloutComplete)));
        assert_eq!(orchestrator.phase(), RolloutPhase::Complete);
        let kinds: Vec<&str> = sink
            .records()
            .iter()
            .map(|r| r.kind.name())
            .collect();
        assert!(kinds.contains(&"FLEET_ROLLOUT_COHORT_ADVANCED"));
        // The snapshot shows the complete rollout.
        let snapshot = orchestrator.fleet_snapshot();
        assert_eq!(snapshot.phase, RolloutPhase::Complete);
        assert_eq!(snapshot.cohorts.len(), 2);
        assert!(snapshot.cohorts[0].canary);
        assert!(!snapshot.cohorts[1].canary);
    }

    #[test]
    fn reasons_and_actors_are_required_for_advances() {
        let (mut orchestrator, _sink) = orchestrator();
        let dir = directory();
        orchestrator.start(&dir).expect("canary starts");
        for device in ["device-1", "device-2", "device-3"] {
            orchestrator
                .authorize_device(&dir, device, &passed_attestation(device))
                .expect("authorized");
            orchestrator
                .record_outcome(device, DeviceUpdateOutcome::Success)
                .expect("recorded");
        }
        assert!(matches!(
            orchestrator.advance_cohort(&dir, "", "reason"),
            Err(RolloutConfigError::ActorRequired)
        ));
        assert!(matches!(
            orchestrator.advance_cohort(&dir, "operator", "   "),
            Err(RolloutConfigError::ReasonRequired)
        ));
    }

    #[test]
    fn a_noop_rollout_is_refused() {
        // The NoOpRollout refusal requires a running-version comparison;
        // the plan itself carries only the target. The check is the
        // composition's (it knows the fleet's current version); the plan
        // refuses an empty target instead. The statement is pinned here
        // so the discipline stays visible.
        let (cohorts, thresholds, canary_bp) = plan_params();
        assert!(matches!(
            RolloutPlan::plan("", cohorts, thresholds, canary_bp),
            Err(RolloutConfigError::PlanInvalid { .. })
        ));
        assert!(RolloutConfigError::NoOpRollout { version: "2.0.0".to_string() }
            .to_string()
            .contains("no-op"));
    }

    #[test]
    fn the_statements_pin_the_honesty_bounds() {
        assert!(
            DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT.contains("byte-identical"),
            "the scope statement carries the kernel-policy distinction"
        );
        assert!(
            DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT.contains("NEW mechanism"),
            "the scope statement names the mechanism as new"
        );
        assert!(
            DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT.contains("DEFAULT RECOMMENDATION"),
            "the canary band is a recommendation, not a sourced figure"
        );
        assert!(ABORT_SEMANTICS_STATEMENT.contains("EXCEEDS"));
        assert!(ABORT_SEMANTICS_STATEMENT.contains("terminal"));
        // The snapshot phase names are stable.
        assert_eq!(RolloutPhase::NotStarted.name(), "ROLLOUT_NOT_STARTED");
        assert_eq!(RolloutPhase::InProgress.name(), "ROLLOUT_IN_PROGRESS");
        assert_eq!(RolloutPhase::Complete.name(), "ROLLOUT_COMPLETE");
        assert_eq!(RolloutPhase::Aborted.name(), "ROLLOUT_ABORTED");
    }
}
