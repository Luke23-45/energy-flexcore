// FIM-F12.03 + FIM-F12.05 (Wave 6, final implementation milestone) — the
//         OTA cohort rollout COORDINATOR for the secure-OTA pipeline
//         (feature F-12; the control-plane half of the P18.02-P18.06
//         implementation).
//
// IMPLEMENTED-UNDER-FIM-F12, WRITTEN-UNRUN: this module has never been
// compiled or run; no rollout has ever been started; no cohort exists; no
// fleet exists; no device report has ever been received. Deterministic
// and socket-free: the coordinator is a STATE MACHINE OVER RECORDED
// EVENTS driven by injected inputs, with the INJECTED clock only — no
// HTTP, no sockets, no wall time anywhere.
//
// WHAT IS COMPOSED (consumed, never re-implemented):
//
// - the cohort mechanism, the pre-registered abort thresholds (X-8 — a
//   plan without them is structurally impossible), the declared-order
//   staged progression (canary first, no skipping), the terminal audited
//   abort, and the per-device attestation-gated authorization are the
//   EXISTING P18.06 orchestrator
//   (`flexcore_application::rollout_orchestrator`); its
//   `FLEET_ROLLOUT_*` audit records ride inside every call. The scope
//   distinction vs the kernel plugin-rollout policy is that module's
//   `DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT` — the kernel policy stays
//   byte-identical and cohort/percentage-free BY CONSTRUCTION.
// - the per-attempt evidence is the EXISTING P18.07 vocabulary
//   (`flexcore_audit::update_evidence`: attempt / verify / install /
//   result / rollback riding SUCCESS/FAILED/REFUSED outcomes, digest-
//   chained per attempt). The chain key is DEPLOYMENT-SUPPLIED key
//   material (the P11.15 discipline — consumed, never generated); the
//   key ceremonies are HUMAN GATES (P18.03 / P11.15) and no key material
//   ships.
//
// WHAT IS NEW HERE (the F-12 additions):
//
// - explicit DEVICE-ID cohort definitions (`OtaCohortDefinition`): the
//   membership is declared as device ids at plan time — the fleet
//   registry's internals are NOT consumed (its schema is a deployment
//   concern; this coordinator takes ids only);
// - a per-cohort `max_failing_devices` tripwire IN ADDITION to the
//   orchestrator's pre-registered failure-rate threshold: whichever fires
//   first FREEZES the rollout (coordinator-level freeze: no further
//   authorization, report, or cohort advance is accepted; every freeze
//   is audited `OTA_ROLLOUT_FROZEN` with its reason). The
//   orchestrator-level abort remains the terminal orchestrator mechanism;
//   the coordinator mirrors it into the same frozen state.
// - the `OtaReportReceiver` PORT TRAIT: devices report through this
//   trait — never HTTP (the transport is the composition's concern; the
//   coordinator consumes typed reports);
// - the report → evidence → outcome mapping (recorded below), so every
//   attempt's every report lands in the audit trail.
//
// THE REPORT → EVIDENCE → OUTCOME MAPPING (recorded, deterministic):
//
// | Report state (the P18.04 vocabulary, consumed by citation)   | Evidence record          | Orchestrator outcome |
// |---|---|---|
// | UPDATE_IDLE                                                  | refused (not a reportable attempt state) | — |
// | UPDATE_DOWNLOADING (first report for the attempt)            | Attempt/SUCCESS          | — |
// | UPDATE_VERIFYING / UPDATE_INSTALLING / UPDATE_ROLLING_BACK   | progress only (audit)    | — |
// | UPDATE_VERIFIED_PENDING_INSTALL                              | Verify/SUCCESS           | — |
// | UPDATE_HEALTH_CHECK_PENDING                                  | Install/SUCCESS          | — |
// | UPDATE_ACTIVE (terminal)                                     | Result/SUCCESS           | Success |
// | UPDATE_FAILED (terminal)                                     | Result/FAILED            | Failed{failure_state} |
// | UPDATE_ROLLED_BACK (terminal)                                | Rollback/SUCCESS         | RolledBack{reason} |
//
// A rollback report is evidence-SUCCESS (the device recovered) but a
// rollout-FAILURE (`DeviceUpdateOutcome::is_failure`) — the abort
// thresholds count it, exactly like the P18.06 semantics.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use flexcore_application::rollout_orchestrator::{
    AbortThresholds, AttestationVerdict, CohortPlan, DeviceUpdateOutcome, FleetCohortDirectory,
    FleetRolloutSnapshot, RolloutAuditSink, RolloutOrchestrator, RolloutPhase, RolloutPlan,
    RolloutConfigError,
};
use flexcore_audit::chain::ChainKey;
use flexcore_audit::update_evidence::{
    UpdateEvidenceChain, UpdateEvidenceKind, UpdateEvidenceOutcome, UpdateEvidenceRecord,
};
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_ports::clock::Clock;

// --- Constants -----------------------------------------------------------------

/// The FIM-F12.03 honesty statement (pinned so tests and reviewers can
/// read what IS and IS NOT claimed).
pub const OTA_ROLLOUT_STATEMENT: &str = "the FIM-F12 rollout coordinator composes the \
     EXISTING P18.06 orchestrator (pre-registered thresholds, declared-order progression, \
     terminal audited abort — the kernel plugin-rollout policy stays byte-identical and \
     cohort/percentage-free) with explicit device-id cohort definitions, a per-cohort \
     max-failing-devices tripwire, and the OtaReportReceiver port trait (devices report \
     through the port, never HTTP). Every report and every attempt lands in the audit \
     trail: the orchestrator's FLEET_ROLLOUT_* records, the coordinator's OTA_ROLLOUT_* \
     lines, and the EXISTING P18.07 evidence-chain vocabulary. An abort or a threshold \
     tripwire FREEZES the rollout (no further authorization, report, or advance) and the \
     freeze is recorded with its reason. WRITTEN-UNRUN: no rollout has ever run; no \
     cohort exists; no fleet exists; no report has ever been received.";

/// The closed report-state vocabulary — the P18.04 flow-state names
/// (`crates/edge-agent/src/update_session.rs` `UpdateProgressState::name`),
/// CONSUMED BY CITATION: the control plane cannot depend on the edge crate
/// (the dependency rule), so the names travel as the wire strings, the
/// same discipline as the frozen §14.1 command-state names in the interlock.
/// A report whose state is outside this set is refused, never guessed at.
pub const OTA_REPORT_STATE_NAMES: [&str; 10] = [
    "UPDATE_IDLE",
    "UPDATE_DOWNLOADING",
    "UPDATE_VERIFYING",
    "UPDATE_VERIFIED_PENDING_INSTALL",
    "UPDATE_INSTALLING",
    "UPDATE_HEALTH_CHECK_PENDING",
    "UPDATE_ACTIVE",
    "UPDATE_ROLLING_BACK",
    "UPDATE_ROLLED_BACK",
    "UPDATE_FAILED",
];

/// The closed failure-state vocabulary — the P18.04 explicit failure
/// states (`crates/edge-agent/src/update_session.rs`), consumed by
/// citation. A `UPDATE_FAILED` report MUST carry one of these; a failure
/// state on any other report is malformed.
pub const OTA_REPORT_FAILURE_STATE_NAMES: [&str; 6] = [
    "UPDATE_FAILURE_DOWNLOAD_FAILED",
    "UPDATE_FAILURE_VERIFICATION_REFUSED",
    "UPDATE_FAILURE_INSTALL_FAILED",
    "UPDATE_FAILURE_HEALTH_CHECK_FAILED",
    "UPDATE_FAILURE_ROLLBACK_FAILED",
    "UPDATE_FAILURE_CANCELLED",
];

// --- Errors -----------------------------------------------------------------------

/// Named errors of the rollout coordinator. Every failure is named
/// ([`OtaRolloutError::name`]); nothing is a silent pass. The existing
/// orchestrator's refusal taxonomy is PRESERVED: its stable code rides
/// `OrchestratorRefused` verbatim (the `BaseVerificationRefused` pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtaRolloutError {
    /// An input was unusable (empty ids, a state outside the closed
    /// vocabulary, a malformed failure-state rule, …).
    InvalidInput { reason: String },
    /// The rollout is FROZEN (an abort fired, or the per-cohort
    /// max-failing-devices tripwire fired) — no further authorization,
    /// report, or cohort advance is accepted; the reason rides.
    FrozenRollout { reason: String },
    /// The report's state is outside the closed P18.04 vocabulary —
    /// refused, never guessed at.
    ReportStateUnknown { reason: String },
    /// The reporting device has no recorded cohort assignment — devices
    /// report only after an explicit assignment (fail-closed).
    DeviceNotAssigned { device_id: String },
    /// The EXISTING orchestrator refused — its stable code and text ride
    /// verbatim (the preserved P18.06 refusal order).
    OrchestratorRefused { code: String, reason: String },
    /// The P18.07 evidence chain refused the record — the attempt's
    /// evidence cannot be silently skipped (fail-closed: the report is
    /// refused and nothing progresses).
    EvidenceRefused { reason: String },
}

impl OtaRolloutError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            OtaRolloutError::InvalidInput { .. } => "OTA_ROLLOUT_INPUT_INVALID",
            OtaRolloutError::FrozenRollout { .. } => "OTA_ROLLOUT_FROZEN",
            OtaRolloutError::ReportStateUnknown { .. } => "OTA_ROLLOUT_REPORT_STATE_UNKNOWN",
            OtaRolloutError::DeviceNotAssigned { .. } => "OTA_ROLLOUT_DEVICE_NOT_ASSIGNED",
            OtaRolloutError::OrchestratorRefused { .. } => "OTA_ROLLOUT_ORCHESTRATOR_REFUSED",
            OtaRolloutError::EvidenceRefused { .. } => "OTA_ROLLOUT_EVIDENCE_REFUSED",
        }
    }
}

impl std::fmt::Display for OtaRolloutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaRolloutError::InvalidInput { reason } => {
                write!(f, "the OTA rollout input is invalid: {reason}")
            }
            OtaRolloutError::FrozenRollout { reason } => write!(
                f,
                "the rollout is FROZEN ({reason}) — no further authorization, report, or \
                 cohort advance is accepted (the freeze is recorded, never silent)"
            ),
            OtaRolloutError::ReportStateUnknown { reason } => {
                write!(f, "the device report is refused: {reason}")
            }
            OtaRolloutError::DeviceNotAssigned { device_id } => write!(
                f,
                "device '{device_id}' has no recorded cohort assignment — devices report \
                 only after an explicit per-device assignment (fail-closed)"
            ),
            OtaRolloutError::OrchestratorRefused { code, reason } => write!(
                f,
                "the existing orchestrator refused ({code}): {reason} — the preserved \
                 P18.06 refusal order, verbatim"
            ),
            OtaRolloutError::EvidenceRefused { reason } => write!(
                f,
                "the P18.07 evidence chain refused the record: {reason} — the attempt's \
                 evidence cannot be silently skipped (fail-closed)"
            ),
        }
    }
}

impl std::error::Error for OtaRolloutError {}

fn orchestrator_refused(error: RolloutConfigError) -> OtaRolloutError {
    OtaRolloutError::OrchestratorRefused {
        code: error.name().to_string(),
        reason: error.to_string(),
    }
}

// --- Cohort definitions (explicit device ids; no registry internals) ----

/// One cohort definition: an explicit DEVICE-ID membership (the fleet
/// registry's internals are not consumed — the composition supplies ids),
/// the canary flag, and the per-cohort max-failing-devices tripwire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaCohortDefinition {
    pub cohort_id: String,
    /// Whether this is the canary cohort (exactly the FIRST cohort may be).
    pub canary: bool,
    /// The explicit member device ids (non-empty; unique across ALL
    /// cohorts — a device updated twice by one rollout is a plan defect).
    pub device_ids: Vec<String>,
    /// The per-cohort tripwire: the rollout FREEZES when the cohort's
    /// failing-device count EXCEEDS this value (the recorded
    /// strictly-exceeded reading mirrors the orchestrator's
    /// `ABORT_SEMANTICS_STATEMENT`: an equal count does not freeze).
    /// None = the cohort relies on the plan's failure-rate threshold only.
    pub max_failing_devices: Option<u32>,
}

impl OtaCohortDefinition {
    fn validate_into(&self, problems: &mut Vec<String>) {
        if self.cohort_id.trim().is_empty() || self.cohort_id.len() > 64 {
            problems.push(format!(
                "cohort id '{}' must be a non-empty string of at most 64 characters",
                self.cohort_id
            ));
        }
        if self.device_ids.is_empty() {
            problems.push(format!(
                "cohort '{}' declares no device ids — an empty cohort activates nothing \
                 and is refused at plan time",
                self.cohort_id
            ));
        }
        for device_id in &self.device_ids {
            if device_id.trim().is_empty() || device_id.len() > 256 {
                problems.push(format!(
                    "cohort '{}' carries a device id that is empty or longer than 256 \
                     characters",
                    self.cohort_id
                ));
                break;
            }
        }
        let unique: BTreeSet<&String> = self.device_ids.iter().collect();
        if unique.len() != self.device_ids.len() {
            problems.push(format!(
                "cohort '{}' lists a device id twice",
                self.cohort_id
            ));
        }
    }
}

/// Builds the orchestrator's `RolloutPlan` from the definitions (the
/// pre-registered thresholds and canary band pass through to the
/// orchestrator's own fail-closed validation — X-8 is structural there).
/// The definitions are validated here too: unique cohort ids, exactly one
/// canary in the first position, no cross-cohort device overlap.
pub fn build_plan(
    target_version: &str,
    definitions: Vec<OtaCohortDefinition>,
    thresholds: AbortThresholds,
    canary_basis_points: u16,
) -> Result<(RolloutPlan, Vec<OtaCohortDefinition>), OtaRolloutError> {
    if definitions.is_empty() {
        return Err(OtaRolloutError::InvalidInput {
            reason: "the cohort definitions are empty — a rollout stages through at least \
                     one cohort"
                .to_string(),
        });
    }
    let mut problems: Vec<String> = Vec::new();
    let mut seen_cohorts: BTreeSet<&str> = BTreeSet::new();
    let mut seen_devices: BTreeSet<&str> = BTreeSet::new();
    for (index, definition) in definitions.iter().enumerate() {
        definition.validate_into(&mut problems);
        if !seen_cohorts.insert(definition.cohort_id.as_str()) {
            problems.push(format!("cohort '{}' appears twice", definition.cohort_id));
        }
        for device_id in &definition.device_ids {
            if !seen_devices.insert(device_id.as_str()) {
                problems.push(format!(
                    "device '{device_id}' appears in more than one cohort — a device \
                     updated twice by one rollout is a plan defect"
                ));
            }
        }
        if index == 0 && !definition.canary {
            problems.push("the FIRST cohort is the canary".to_string());
        }
        if index > 0 && definition.canary {
            problems.push("only the first cohort is the canary".to_string());
        }
    }
    if !problems.is_empty() {
        return Err(OtaRolloutError::InvalidInput {
            reason: problems.join("; "),
        });
    }
    let cohort_plans = definitions
        .iter()
        .map(|definition| CohortPlan {
            cohort_id: definition.cohort_id.clone(),
            canary: definition.canary,
        })
        .collect();
    let plan = RolloutPlan::plan(target_version, cohort_plans, thresholds, canary_basis_points)
        .map_err(orchestrator_refused)?;
    Ok((plan, definitions))
}

/// The cohort-membership directory over the EXPLICIT device-id
/// assignments: the implementation of the orchestrator's
/// `FleetCohortDirectory` port backed by the plan-time definitions — the
/// fleet registry's internals are not consumed anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticDeviceCohortDirectory {
    assignments: BTreeMap<String, Vec<String>>,
}

impl StaticDeviceCohortDirectory {
    /// Builds the directory from the validated definitions (members sorted
    /// for deterministic order).
    pub fn from_definitions(definitions: &[OtaCohortDefinition]) -> Self {
        let mut assignments = BTreeMap::new();
        for definition in definitions {
            let mut members = definition.device_ids.clone();
            members.sort();
            assignments.insert(definition.cohort_id.clone(), members);
        }
        Self { assignments }
    }

    /// The sorted member ids of one cohort (None = the cohort is unknown).
    pub fn members(&self, cohort_id: &str) -> Option<&Vec<String>> {
        self.assignments.get(cohort_id)
    }
}

impl FleetCohortDirectory for StaticDeviceCohortDirectory {
    fn cohort_members(&self, cohort_id: &str) -> Result<Vec<String>, String> {
        self.assignments
            .get(cohort_id)
            .cloned()
            .ok_or_else(|| {
                format!("no explicit device-id assignment exists for cohort '{cohort_id}'")
            })
    }
}

// --- The device report and the report-receiver port ----------------------

/// ONE device report (the wire shape the `OtaReportReceiver` port
/// consumes — the transport is the composition's concern; the coordinator
/// is reached through the port trait, never HTTP). The state names are the
/// closed P18.04 vocabulary (consumed by citation — see
/// [`OTA_REPORT_STATE_NAMES`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaDeviceReport {
    pub device_id: String,
    /// The update attempt this report belongs to (the P18.07
    /// evidence-chain key).
    pub attempt_id: String,
    /// The closed flow-state name (see [`OTA_REPORT_STATE_NAMES`]).
    pub state: String,
    /// The explicit failure state — MANDATORY for `UPDATE_FAILED` (one of
    /// [`OTA_REPORT_FAILURE_STATE_NAMES`]) and forbidden otherwise.
    pub failure_state: Option<String>,
    /// The version the report concerns.
    pub version: String,
    /// When the device reported (the report's event time; the evidence
    /// record's EventTime — the coordinator's injected clock supplies the
    /// IngestTime).
    pub reported_at_unix_millis: i64,
}

impl OtaDeviceReport {
    /// Validates the report (fail-closed): ids non-empty, the state inside
    /// the closed vocabulary, the failure-state rule, a positive instant.
    pub fn validate(&self) -> Result<(), OtaRolloutError> {
        if self.device_id.trim().is_empty() || self.device_id.len() > 256 {
            return Err(OtaRolloutError::InvalidInput {
                reason: "device_id must be a non-empty string of at most 256 characters"
                    .to_string(),
            });
        }
        if self.attempt_id.trim().is_empty() || self.attempt_id.len() > 256 {
            return Err(OtaRolloutError::InvalidInput {
                reason: "attempt_id must be a non-empty string of at most 256 characters"
                    .to_string(),
            });
        }
        if self.version.trim().is_empty() || self.version.len() > 64 {
            return Err(OtaRolloutError::InvalidInput {
                reason: "version must be a non-empty string of at most 64 characters"
                    .to_string(),
            });
        }
        if self.reported_at_unix_millis <= 0 {
            return Err(OtaRolloutError::InvalidInput {
                reason: "reported_at_unix_millis must be a positive unix-millis value"
                    .to_string(),
            });
        }
        if !OTA_REPORT_STATE_NAMES.contains(&self.state.as_str()) {
            return Err(OtaRolloutError::ReportStateUnknown {
                reason: format!(
                    "state '{}' is not one of the closed P18.04 flow states — refused, \
                     never guessed at",
                    self.state
                ),
            });
        }
        if self.state == "UPDATE_IDLE" {
            return Err(OtaRolloutError::InvalidInput {
                reason: "UPDATE_IDLE is not a reportable attempt state (a device reports \
                         the states of an ATTEMPT; idle is the absence of one)"
                    .to_string(),
            });
        }
        match &self.failure_state {
            Some(failure_state) => {
                if self.state != "UPDATE_FAILED" && self.state != "UPDATE_ROLLED_BACK" {
                    return Err(OtaRolloutError::InvalidInput {
                        reason: format!(
                            "a failure state on a '{}' report is malformed (failure states \
                             ride UPDATE_FAILED and UPDATE_ROLLED_BACK only)",
                            self.state
                        ),
                    });
                }
                if !OTA_REPORT_FAILURE_STATE_NAMES.contains(&failure_state.as_str()) {
                    return Err(OtaRolloutError::ReportStateUnknown {
                        reason: format!(
                            "failure state '{failure_state}' is not one of the closed P18.04 \
                             failure states — refused, never guessed at"
                        ),
                    });
                }
            }
            None => {
                if self.state == "UPDATE_FAILED" {
                    return Err(OtaRolloutError::InvalidInput {
                        reason: "an UPDATE_FAILED report without its explicit failure state \
                                 is refused (an unknown failure is not a recorded failure)"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The port trait devices report through (FIM-F12.03: a port-trait report
/// receiver, never HTTP). Implementations are the coordinator (the
/// production receiver) and test doubles; the transport that delivers the
/// typed report is the composition's concern.
pub trait OtaReportReceiver {
    /// Receives ONE device report. `Ok` = accepted (a receipt carries the
    /// recorded effects); `Err` = refused with a named
    /// [`OtaRolloutError`].
    fn receive(&mut self, report: &OtaDeviceReport) -> Result<OtaReportReceipt, OtaRolloutError>;
}

/// What one accepted report recorded (plain data — the caller's evidence
/// view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaReportReceipt {
    /// Whether a terminal outcome was recorded into the orchestrator (the
    /// abort thresholds evaluate per outcome).
    pub outcome_recorded: bool,
    /// The mapped orchestrator outcome's stable name (empty when no
    /// terminal outcome was recorded).
    pub outcome_name: String,
    /// The attempt's evidence-chain record count after this report.
    pub evidence_records: u64,
    /// The attempt's evidence-chain head digest after this report
    /// (lowercase hex).
    pub evidence_head_hex: String,
    /// The one-line summary (audit text).
    pub summary: String,
}

// --- The coordinator's audit port -------------------------------------------------

/// The kinds of the coordinator's own audit records (SCREAMING_SNAKE
/// stable names; the orchestrator's `FLEET_ROLLOUT_*` records ride in its
/// EXISTING shape — these are the coordinator's ADDITIONS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaRolloutAuditKind {
    /// A device report was accepted.
    ReportReceived,
    /// A device report was refused (the named refusal code rides).
    ReportRefused,
    /// A cohort's device-id assignment was recorded (cohort activation /
    /// advance).
    CohortAssigned,
    /// A per-device assignment was recorded (authorization).
    DeviceAssigned,
    /// The rollout was FROZEN (an abort or the count tripwire) — the
    /// reason rides.
    Frozen,
    /// A progress report was recorded (no terminal outcome).
    ProgressRecorded,
    /// An evidence record was appended to an attempt's chain.
    EvidenceRecorded,
}

impl OtaRolloutAuditKind {
    /// The stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            OtaRolloutAuditKind::ReportReceived => "OTA_ROLLOUT_REPORT_RECEIVED",
            OtaRolloutAuditKind::ReportRefused => "OTA_ROLLOUT_REPORT_REFUSED",
            OtaRolloutAuditKind::CohortAssigned => "OTA_ROLLOUT_COHORT_ASSIGNED",
            OtaRolloutAuditKind::DeviceAssigned => "OTA_ROLLOUT_DEVICE_ASSIGNED",
            OtaRolloutAuditKind::Frozen => "OTA_ROLLOUT_FROZEN",
            OtaRolloutAuditKind::ProgressRecorded => "OTA_ROLLOUT_PROGRESS_RECORDED",
            OtaRolloutAuditKind::EvidenceRecorded => "OTA_ROLLOUT_EVIDENCE_RECORDED",
        }
    }
}

/// One coordinator audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaRolloutAuditRecord {
    /// When the coordinator recorded it (the INJECTED clock).
    pub timestamp_millis: i64,
    pub kind: OtaRolloutAuditKind,
    /// The subject (device id, cohort id, or the target version).
    pub subject: String,
    /// Human-safe detail (ids and refusal names — never key material).
    pub detail: String,
}

/// The coordinator's audit-sink port (the kernel-local audit-port pattern;
/// implementations MUST be append-only).
pub trait OtaRolloutAuditSink: Send + Sync {
    fn record(&self, record: OtaRolloutAuditRecord);
}

/// An append-only in-memory [`OtaRolloutAuditSink`] — the deterministic
/// collector used by tests and in-process compositions.
#[derive(Default)]
pub struct MemoryOtaRolloutAuditSink {
    records: Mutex<Vec<OtaRolloutAuditRecord>>,
}

impl MemoryOtaRolloutAuditSink {
    pub fn new() -> MemoryOtaRolloutAuditSink {
        MemoryOtaRolloutAuditSink { records: Mutex::new(Vec::new()) }
    }

    /// The records in emission order (append-only).
    pub fn records(&self) -> Vec<OtaRolloutAuditRecord> {
        self.records
            .lock()
            .expect("poisoned OTA rollout audit sink mutex")
            .clone()
    }
}

impl OtaRolloutAuditSink for MemoryOtaRolloutAuditSink {
    fn record(&self, record: OtaRolloutAuditRecord) {
        self.records
            .lock()
            .expect("poisoned OTA rollout audit sink mutex")
            .push(record);
    }
}

impl OtaRolloutAuditSink for Arc<MemoryOtaRolloutAuditSink> {
    fn record(&self, record: OtaRolloutAuditRecord) {
        MemoryOtaRolloutAuditSink::record(self, record)
    }
}

/// Adapter that lets one `Arc`-held clock feed BOTH the composed
/// orchestrator (which takes a boxed clock) and the coordinator's own
/// stamps — one injected clock, no divergence.
struct SharedClock(Arc<dyn Clock + Send + Sync>);

impl Clock for SharedClock {
    fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
        self.0.now_event_time()
    }
    fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
        self.0.now_ingest_time()
    }
}

// --- The coordinator -----------------------------------------------------------------------

/// One attempt's per-attempt evidence chain (the P18.07 shape).
struct AttemptEvidence {
    chain: UpdateEvidenceChain,
}

/// The FIM-F12.03 OTA cohort rollout coordinator: a deterministic state
/// machine over recorded events, composing the EXISTING P18.06
/// orchestrator and the EXISTING P18.07 evidence vocabulary. Holds the
/// explicit device-id cohort definitions and directory, the per-device
/// assignment records, the per-cohort failing-device counts, and the
/// frozen state. Implements [`OtaReportReceiver`] — devices report
/// through the port, never HTTP. Produces authorization RECORDS (through
/// the orchestrator); it never issues a command and never touches
/// dispatch authority.
pub struct OtaRolloutCoordinator {
    definitions: Vec<OtaCohortDefinition>,
    directory: StaticDeviceCohortDirectory,
    orchestrator: RolloutOrchestrator,
    /// The ACTIVE cohort id (tracked by this coordinator; the
    /// orchestrator's phase remains authoritative).
    active_cohort_id: Option<String>,
    /// The per-device assignment records (device id → cohort id), appended
    /// at authorization.
    assignments: BTreeMap<String, String>,
    /// The devices with a recorded terminal outcome (append-once per
    /// device per rollout — the plan refuses cross-cohort duplicates).
    outcomes_recorded: BTreeSet<String>,
    /// The per-cohort failing-device counts (the count tripwire's input).
    failing_by_cohort: BTreeMap<String, u32>,
    /// The per-attempt evidence chains (lazily created at the first
    /// report).
    attempts: BTreeMap<String, AttemptEvidence>,
    /// The frozen state: Some(reason) refuses everything further.
    frozen_reason: Option<String>,
    namespace_key: ChainKey,
    audit: Arc<dyn OtaRolloutAuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl OtaRolloutCoordinator {
    /// Composes the coordinator over a validated plan (see [`build_plan`]),
    /// the orchestrator's EXISTING audit sink (the `FLEET_ROLLOUT_*`
    /// records), the coordinator's own audit sink (the `OTA_ROLLOUT_*`
    /// records), the DEPLOYMENT-SUPPLIED evidence namespace key (never
    /// generated here — the P11.15 discipline), and the injected clock.
    pub fn new(
        plan: RolloutPlan,
        definitions: Vec<OtaCohortDefinition>,
        orchestrator_audit: Box<dyn RolloutAuditSink + Send>,
        audit: Arc<dyn OtaRolloutAuditSink>,
        namespace_key: ChainKey,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let directory = StaticDeviceCohortDirectory::from_definitions(&definitions);
        let orchestrator = RolloutOrchestrator::new(
            plan,
            orchestrator_audit,
            Box::new(SharedClock(clock.clone())),
        );
        Self {
            definitions,
            directory,
            orchestrator,
            active_cohort_id: None,
            assignments: BTreeMap::new(),
            outcomes_recorded: BTreeSet::new(),
            failing_by_cohort: BTreeMap::new(),
            attempts: BTreeMap::new(),
            frozen_reason: None,
            namespace_key,
            audit,
            clock,
        }
    }

    /// The plan's target version (read view).
    pub fn target_version(&self) -> &str {
        self.orchestrator.plan().target_version.as_str()
    }

    /// The orchestrator's phase (authoritative for the rollout lifecycle).
    pub fn phase(&self) -> RolloutPhase {
        self.orchestrator.phase()
    }

    /// The active cohort id (Some while a cohort is in progress).
    pub fn active_cohort_id(&self) -> Option<&str> {
        self.active_cohort_id.as_deref()
    }

    /// The frozen reason, when frozen.
    pub fn freeze_reason(&self) -> Option<&str> {
        self.frozen_reason.as_deref()
    }

    /// Whether the rollout is frozen (an abort or the count tripwire
    /// fired).
    pub fn is_frozen(&self) -> bool {
        self.frozen_reason.is_some()
    }

    /// The fleet snapshot (the orchestrator's existing shape — plain data
    /// for the console adapter).
    pub fn fleet_snapshot(&self) -> FleetRolloutSnapshot {
        self.orchestrator.fleet_snapshot()
    }

    /// An attempt's evidence-chain record count and head digest (the
    /// evidence view for assertions and operator packs).
    pub fn attempt_evidence(&self, attempt_id: &str) -> Option<(u64, String)> {
        self.attempts
            .get(attempt_id)
            .map(|attempt| (attempt.chain.count(), attempt.chain.head_hex()))
    }

    fn check_frozen(&self) -> Result<(), OtaRolloutError> {
        match &self.frozen_reason {
            Some(reason) => Err(OtaRolloutError::FrozenRollout {
                reason: reason.clone(),
            }),
            None => Ok(()),
        }
    }

    fn freeze(&mut self, reason: String) {
        if self.frozen_reason.is_none() {
            self.frozen_reason = Some(reason.clone());
            self.audit_event(
                OtaRolloutAuditKind::Frozen,
                format!("version={}", self.target_version()),
                format!(
                    "the rollout is FROZEN: {reason} — no further authorization, report, \
                     or cohort advance is accepted (the abort/freeze is recorded, never \
                     silent)"
                ),
            );
        }
    }

    /// Activates the FIRST cohort (the canary). The membership comes from
    /// the explicit device-id directory; the assignment is recorded and
    /// audited (`OTA_ROLLOUT_COHORT_ASSIGNED`).
    pub fn start(&mut self) -> Result<String, OtaRolloutError> {
        self.check_frozen()?;
        let canary_id = self
            .orchestrator
            .start(&self.directory)
            .map_err(orchestrator_refused)?;
        self.active_cohort_id = Some(canary_id.clone());
        self.audit_cohort_assigned(&canary_id);
        Ok(canary_id)
    }

    /// Authorizes ONE device in the ACTIVE cohort (the attestation-gated
    /// RECORD through the orchestrator; the per-device assignment is
    /// recorded and audited — FIM-F12.05's cohort-assignment leg).
    pub fn authorize_device(
        &mut self,
        device_id: &str,
        attestation: &AttestationVerdict,
    ) -> Result<(), OtaRolloutError> {
        self.check_frozen()?;
        let cohort_id = self.active_cohort_id.clone().ok_or_else(|| OtaRolloutError::InvalidInput {
            reason: "no active cohort (start the rollout first)".to_string(),
        })?;
        // Cheap pre-check on the explicit directory (the orchestrator
        // re-checks at authorization time — its refusal rides verbatim).
        let members = self.directory.members(&cohort_id).ok_or_else(|| {
            OtaRolloutError::InvalidInput {
                reason: format!("the active cohort '{cohort_id}' has no explicit assignment"),
            }
        })?;
        if !members.iter().any(|member| member == device_id) {
            let refused = OtaRolloutError::DeviceNotAssigned {
                device_id: device_id.to_string(),
            };
            self.audit_event(
                OtaRolloutAuditKind::ReportRefused,
                device_id.to_string(),
                format!("authorization refused ({}): {refused}", refused.name()),
            );
            return Err(refused);
        }
        self.orchestrator
            .authorize_device(&self.directory, device_id, attestation)
            .map_err(orchestrator_refused)?;
        self.assignments
            .insert(device_id.to_string(), cohort_id.clone());
        self.audit_event(
            OtaRolloutAuditKind::DeviceAssigned,
            format!("{cohort_id}/{device_id}"),
            format!(
                "device '{device_id}' assigned to cohort '{cohort_id}' for version {} — the \
                 per-device assignment record (an authorization RECORD; never a command)",
                self.target_version(),
            ),
        );
        Ok(())
    }

    /// Advances to the NEXT declared cohort (the declared order is the
    /// orchestrator's; the new cohort's assignment is recorded and
    /// audited). A frozen rollout refuses.
    pub fn advance_cohort(&mut self, actor: &str, reason: &str) -> Result<String, OtaRolloutError> {
        self.check_frozen()?;
        let next_id = self
            .orchestrator
            .advance_cohort(&self.directory, actor, reason)
            .map_err(orchestrator_refused)?;
        self.active_cohort_id = Some(next_id.clone());
        self.audit_cohort_assigned(&next_id);
        Ok(next_id)
    }

    /// Records ONE device's terminal outcome directly (the composition's
    /// seam for outcomes that arrive outside the report path — e.g. the
    /// edge-agent's own uplink). Terminal outcomes evaluate the abort
    /// thresholds inside the orchestrator and the count tripwire here.
    pub fn record_outcome(
        &mut self,
        device_id: &str,
        outcome: DeviceUpdateOutcome,
    ) -> Result<(), OtaRolloutError> {
        self.check_frozen()?;
        self.orchestrator
            .record_outcome(device_id, outcome.clone())
            .map_err(orchestrator_refused)?;
        self.after_outcome(device_id, &outcome);
        Ok(())
    }

    fn after_outcome(&mut self, device_id: &str, outcome: &DeviceUpdateOutcome) {
        self.outcomes_recorded.insert(device_id.to_string());
        if outcome.is_failure() {
            let cohort_id = self
                .assignments
                .get(device_id)
                .cloned()
                .unwrap_or_default();
            let count = self
                .failing_by_cohort
                .entry(cohort_id.clone())
                .or_insert(0);
            *count += 1;
            let failing = *count;
            // The per-cohort count tripwire: strictly-exceeded freezes
            // (the recorded reading; an equal count does not).
            if let Some(definition) = self
                .definitions
                .iter()
                .find(|definition| definition.cohort_id == cohort_id)
            {
                if let Some(max_failing) = definition.max_failing_devices {
                    if failing > max_failing {
                        self.freeze(format!(
                            "cohort '{cohort_id}' reached {failing} failing device(s), \
                             exceeding the pre-registered max_failing_devices={max_failing}"
                        ));
                    }
                }
            }
        }
        // The orchestrator-level abort mirrors into the same frozen state
        // (the abort FREEZES the rollout and records it — FIM-F12.03).
        if self.orchestrator.phase() == RolloutPhase::Aborted {
            let reason = self
                .orchestrator
                .abort_reason()
                .unwrap_or("the orchestrator aborted the rollout")
                .to_string();
            self.freeze(format!("the rollout ABORTED: {reason}"));
        }
    }

    fn audit_cohort_assigned(&self, cohort_id: &str) {
        let members = self
            .directory
            .members(cohort_id)
            .map(|members| members.join(","))
            .unwrap_or_default();
        self.audit_event(
            OtaRolloutAuditKind::CohortAssigned,
            cohort_id.to_string(),
            format!(
                "cohort '{cohort_id}' assignment recorded: [{}] — explicit device ids (the \
                 fleet registry's internals are not consumed), version {}",
                members,
                self.target_version(),
            ),
        );
    }
}

impl OtaReportReceiver for OtaRolloutCoordinator {
    /// THE REPORT RECEIVER (the state machine over recorded events). The
    /// recorded evaluation order:
    ///
    /// 1. the report shape (ids, the closed state vocabulary, the
    ///    failure-state rule) — a malformed report is refused regardless
    ///    of the rollout state;
    /// 2. the frozen state — a frozen rollout refuses every report;
    /// 3. the orchestrator's ABORTED phase (mirrored into the same frozen
    ///    state — the abort freezes the rollout and records it);
    /// 4. the device's assignment (fail-closed on an unassigned device);
    /// 5. the evidence record(s) — the first report for an attempt lands
    ///    the Attempt record; the mapped states land their records (the
    ///    EXISTING P18.07 vocabulary); an evidence refusal refuses the
    ///    report (fail-closed — evidence is never silently skipped);
    /// 6. a terminal outcome — mapped and recorded into the orchestrator
    ///    (the thresholds evaluate inside; the count tripwire and the
    ///    abort mirror run after).
    fn receive(&mut self, report: &OtaDeviceReport) -> Result<OtaReportReceipt, OtaRolloutError> {
        // 1. Shape.
        if let Err(error) = report.validate() {
            self.audit_event(
                OtaRolloutAuditKind::ReportRefused,
                report.device_id.clone(),
                format!("report refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        // 2. Frozen.
        if let Err(error) = self.check_frozen() {
            self.audit_event(
                OtaRolloutAuditKind::ReportRefused,
                report.device_id.clone(),
                format!("report refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        // 3. The orchestrator's abort (mirrored into the frozen state).
        if self.orchestrator.phase() == RolloutPhase::Aborted {
            let reason = self
                .orchestrator
                .abort_reason()
                .unwrap_or("the orchestrator aborted the rollout")
                .to_string();
            self.freeze(format!("the rollout ABORTED: {reason}"));
            let error = OtaRolloutError::FrozenRollout { reason };
            self.audit_event(
                OtaRolloutAuditKind::ReportRefused,
                report.device_id.clone(),
                format!("report refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        // 4. Assignment.
        if !self.assignments.contains_key(&report.device_id) {
            let error = OtaRolloutError::DeviceNotAssigned {
                device_id: report.device_id.clone(),
            };
            self.audit_event(
                OtaRolloutAuditKind::ReportRefused,
                report.device_id.clone(),
                format!("report refused ({}): {error}", error.name()),
            );
            return Err(error);
        }
        // 5. Evidence (fail-closed: an evidence refusal refuses the
        //    report BEFORE any outcome is recorded).
        let evidence_head = self.append_evidence(report)?;
        // 6. Terminal outcome.
        let outcome = match report.state.as_str() {
            "UPDATE_ACTIVE" => Some(DeviceUpdateOutcome::Success),
            "UPDATE_FAILED" => Some(DeviceUpdateOutcome::Failed {
                failure_state: report
                    .failure_state
                    .clone()
                    .unwrap_or_else(|| "UPDATE_FAILURE_DOWNLOAD_FAILED".to_string()),
            }),
            "UPDATE_ROLLED_BACK" => Some(DeviceUpdateOutcome::RolledBack {
                reason: report
                    .failure_state
                    .clone()
                    .unwrap_or_else(|| "reported by the device".to_string()),
            }),
            _ => None,
        };
        let (outcome_recorded, outcome_name) = match outcome {
            Some(outcome) => {
                if self.outcomes_recorded.contains(&report.device_id) {
                    let error = OtaRolloutError::OrchestratorRefused {
                        code: "ROLLOUT_DEVICE_ALREADY_OUTCOME".to_string(),
                        reason: format!(
                            "device '{}' already has a terminal outcome recorded — \
                             outcomes are append-once (the P18.06 rule)",
                            report.device_id
                        ),
                    };
                    self.audit_event(
                        OtaRolloutAuditKind::ReportRefused,
                        report.device_id.clone(),
                        format!("report refused ({}): {error}", error.name()),
                    );
                    return Err(error);
                }
                let outcome_name = outcome.name().to_string();
                self.orchestrator
                    .record_outcome(&report.device_id, outcome.clone())
                    .map_err(orchestrator_refused)?;
                self.after_outcome(&report.device_id, &outcome);
                self.audit_event(
                    OtaRolloutAuditKind::ReportReceived,
                    format!("{}/{}", report.device_id, report.attempt_id),
                    format!(
                        "terminal report {} for version {} recorded as {outcome_name} — the \
                         abort thresholds evaluate per outcome",
                        report.state,
                        report.version,
                    ),
                );
                (true, outcome_name)
            }
            None => {
                self.audit_event(
                    OtaRolloutAuditKind::ProgressRecorded,
                    format!("{}/{}", report.device_id, report.attempt_id),
                    format!(
                        "progress report {} for version {} (no terminal outcome yet)",
                        report.state, report.version,
                    ),
                );
                (false, String::new())
            }
        };
        let (evidence_records, head_hex) = evidence_head;
        Ok(OtaReportReceipt {
            outcome_recorded,
            outcome_name,
            evidence_records,
            evidence_head_hex: head_hex,
            summary: format!(
                "device='{}' attempt='{}' state={} outcome={} evidence_records={}",
                report.device_id,
                report.attempt_id,
                report.state,
                if outcome_recorded { outcome_name.as_str() } else { "none" },
                evidence_records,
            ),
        })
    }
}

impl OtaRolloutCoordinator {
    /// Appends the mapped evidence records for one report (the EXISTING
    /// P18.07 vocabulary; the per-attempt chain is created lazily). The
    /// first report for an attempt lands the Attempt record; the mapped
    /// states land theirs; progress-only states append nothing beyond the
    /// first-seen Attempt. Returns the chain's (count, head) after the
    /// append.
    fn append_evidence(
        &mut self,
        report: &OtaDeviceReport,
    ) -> Result<(u64, String), OtaRolloutError> {
        let first_seen = !self.attempts.contains_key(&report.attempt_id);
        // The records are BUILT first (immutable borrows only), then
        // appended under the chain's mutable borrow — one ordering, no
        // borrow overlap.
        let mut records: Vec<UpdateEvidenceRecord> = Vec::new();
        if first_seen {
            records.push(self.evidence_record(
                report,
                UpdateEvidenceKind::Attempt,
                UpdateEvidenceOutcome::Success,
                format!("the attempt was first observed in state {}", report.state),
            )?);
        }
        // The mapped state records (see the module-header table).
        let mapped = match report.state.as_str() {
            "UPDATE_VERIFIED_PENDING_INSTALL" => Some((
                UpdateEvidenceKind::Verify,
                UpdateEvidenceOutcome::Success,
                "the device's verification PASSED (the gate verdict, reported)",
            )),
            "UPDATE_HEALTH_CHECK_PENDING" => Some((
                UpdateEvidenceKind::Install,
                UpdateEvidenceOutcome::Success,
                "the device installed the bundle; the health check is pending",
            )),
            "UPDATE_ACTIVE" => Some((
                UpdateEvidenceKind::Result,
                UpdateEvidenceOutcome::Success,
                "the device completed the update healthy",
            )),
            "UPDATE_FAILED" => Some((
                UpdateEvidenceKind::Result,
                UpdateEvidenceOutcome::Failed,
                report
                    .failure_state
                    .clone()
                    .unwrap_or_else(|| "the device reported the attempt failed".to_string()),
            )),
            "UPDATE_ROLLED_BACK" => Some((
                UpdateEvidenceKind::Rollback,
                UpdateEvidenceOutcome::Success,
                report
                    .failure_state
                    .clone()
                    .unwrap_or_else(|| "the device reported the rollback".to_string()),
            )),
            // Progress states (DOWNLOADING / VERIFYING / INSTALLING /
            // ROLLING_BACK) carry no additional evidence record beyond
            // the first-seen Attempt.
            _ => None,
        };
        if let Some((kind, outcome, detail)) = mapped {
            records.push(self.evidence_record(report, kind, outcome, detail)?);
        }
        // The mutable phase: the lazily-created per-attempt chain, then
        // the appends (fail-closed — an evidence refusal refuses the
        // report BEFORE any outcome is recorded).
        let chain = &mut self
            .attempts
            .entry(report.attempt_id.clone())
            .or_insert_with(|| AttemptEvidence {
                chain: UpdateEvidenceChain::new(
                    &self.namespace_key,
                    &report.attempt_id,
                    &report.device_id,
                )
                .expect("the attempt id and device id are validated non-empty"),
            });
        let mut appended = 0u64;
        for record in records {
            chain
                .append(record)
                .map_err(|error| OtaRolloutError::EvidenceRefused {
                    reason: error.to_string(),
                })?;
            appended += 1;
        }
        let count = chain.count();
        let head = chain.head_hex();
        if appended > 0 {
            self.audit_event(
                OtaRolloutAuditKind::EvidenceRecorded,
                format!("{}/{}", report.device_id, report.attempt_id),
                format!(
                    "{appended} evidence record(s) appended (kind(s) per the P18.07 \
                     vocabulary); chain records={count} head={head}"
                ),
            );
        }
        Ok((count, head))
    }

    fn evidence_record(
        &self,
        report: &OtaDeviceReport,
        kind: UpdateEvidenceKind,
        outcome: UpdateEvidenceOutcome,
        detail: String,
    ) -> Result<UpdateEvidenceRecord, OtaRolloutError> {
        UpdateEvidenceRecord::new(UpdateEvidenceRecord {
            attempt_id: report.attempt_id.clone(),
            device_id: report.device_id.clone(),
            version: report.version.clone(),
            kind,
            outcome,
            detail,
            event_time: EventTime::from_unix_millis(report.reported_at_unix_millis)
                .map_err(|error| OtaRolloutError::EvidenceRefused {
                    reason: format!("the report's event time is invalid: {error}"),
                })?,
            ingest_time: IngestTime::from_unix_millis(self.now_unix_millis())
                .map_err(|error| OtaRolloutError::EvidenceRefused {
                    reason: format!("the injected clock instant is invalid: {error}"),
                })?,
        })
        .map_err(|error| OtaRolloutError::EvidenceRefused {
            reason: error.to_string(),
        })
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    fn audit_event(&self, kind: OtaRolloutAuditKind, subject: String, detail: String) {
        self.audit.record(OtaRolloutAuditRecord {
            timestamp_millis: self.now_unix_millis(),
            kind,
            subject,
            detail,
        });
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_application::rollout_orchestrator::MemoryRolloutAuditSink;

    struct FixedClock(i64);
    impl Clock for FixedClock {
        fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
            flexcore_contracts::time_model::EventTime::from_unix_millis(self.0)
                .expect("valid instant")
        }
        fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
            flexcore_contracts::time_model::IngestTime::from_unix_millis(self.0)
                .expect("valid instant")
        }
    }

    /// The injected clock sits FAR AHEAD of the fixture report instants:
    /// the P18.07 evidence rule (ingest >= event) holds for every fixture
    /// report without the tests advancing the clock.
    const REMOTE_CLOCK_MILLIS: i64 = 5_000_000_000_000;

    /// The FIXED TEST-GRADE evidence namespace key (loudly labelled; never
    /// a deployment key — the P11.15 test discipline).
    fn test_namespace_key() -> ChainKey {
        ChainKey::from_bytes(&[0x6Bu8; 32]).expect("valid test key")
    }

    fn definition(id: &str, canary: bool, ids: &[&str], max_failing: Option<u32>)
    -> OtaCohortDefinition {
        OtaCohortDefinition {
            cohort_id: id.to_string(),
            canary,
            device_ids: ids.iter().map(|id| id.to_string()).collect(),
            max_failing_devices: max_failing,
        }
    }

    fn report(device: &str, attempt: &str, state: &str, at: i64) -> OtaDeviceReport {
        OtaDeviceReport {
            device_id: device.to_string(),
            attempt_id: attempt.to_string(),
            state: state.to_string(),
            failure_state: None,
            version: "2.0.0".to_string(),
            reported_at_unix_millis: at,
        }
    }

    fn failed_report(device: &str, attempt: &str, at: i64) -> OtaDeviceReport {
        OtaDeviceReport {
            device_id: device.to_string(),
            attempt_id: attempt.to_string(),
            state: "UPDATE_FAILED".to_string(),
            failure_state: Some("UPDATE_FAILURE_HEALTH_CHECK_FAILED".to_string()),
            version: "2.0.0".to_string(),
            reported_at_unix_millis: at,
        }
    }

    fn coordinator(
        definitions: Vec<OtaCohortDefinition>,
    ) -> (
        OtaRolloutCoordinator,
        Arc<MemoryOtaRolloutAuditSink>,
        Arc<MemoryRolloutAuditSink>,
    ) {
        let (plan, definitions) = build_plan(
            "2.0.0",
            definitions,
            AbortThresholds {
                max_failure_rate_permille: 500,
                min_observations: 2,
            },
            500,
        )
        .expect("plan builds");
        let own_sink = Arc::new(MemoryOtaRolloutAuditSink::new());
        let orchestrator_sink = Arc::new(MemoryRolloutAuditSink::new());
        let coordinator = OtaRolloutCoordinator::new(
            plan,
            definitions,
            Box::new(orchestrator_sink.clone()),
            own_sink.clone() as Arc<dyn OtaRolloutAuditSink>,
            test_namespace_key(),
            Arc::new(FixedClock(REMOTE_CLOCK_MILLIS)),
        );
        (coordinator, own_sink, orchestrator_sink)
    }

    fn passed_attestation() -> AttestationVerdict {
        AttestationVerdict::Passed {
            evidence_reference: "attest-evidence-1".to_string(),
        }
    }

    #[test]
    fn the_plan_requires_canary_first_and_unique_devices() {
        // No canary at the front.
        assert!(matches!(
            build_plan(
                "2.0.0",
                vec![
                    definition("block-1", false, &["d1"], None),
                    definition("canary", true, &["d2"], None),
                ],
                AbortThresholds { max_failure_rate_permille: 500, min_observations: 2 },
                500,
            ),
            Err(OtaRolloutError::InvalidInput { .. })
        ));
        // A device in two cohorts.
        assert!(matches!(
            build_plan(
                "2.0.0",
                vec![
                    definition("canary", true, &["d1"], None),
                    definition("block-1", false, &["d1"], None),
                ],
                AbortThresholds { max_failure_rate_permille: 500, min_observations: 2 },
                500,
            ),
            Err(OtaRolloutError::InvalidInput { .. })
        ));
        // An empty cohort.
        assert!(matches!(
            build_plan(
                "2.0.0",
                vec![definition("canary", true, &[], None)],
                AbortThresholds { max_failure_rate_permille: 500, min_observations: 2 },
                500,
            ),
            Err(OtaRolloutError::InvalidInput { .. })
        ));
    }

    #[test]
    fn the_report_shape_is_fail_closed() {
        // Unknown state.
        let mut unknown = report("d1", "a1", "UPDATE_SOMETHING_ELSE", 1_000);
        assert!(matches!(
            unknown.validate(),
            Err(OtaRolloutError::ReportStateUnknown { .. })
        ));
        // IDLE is not a reportable attempt state.
        assert!(matches!(
            report("d1", "a1", "UPDATE_IDLE", 1_000).validate(),
            Err(OtaRolloutError::InvalidInput { .. })
        ));
        // FAILED without a failure state.
        let mut missing = report("d1", "a1", "UPDATE_FAILED", 1_000);
        assert!(matches!(missing.validate(), Err(OtaRolloutError::InvalidInput { .. })));
        // A failure state on a progress report.
        let mut misplaced = report("d1", "a1", "UPDATE_DOWNLOADING", 1_000);
        misplaced.failure_state = Some("UPDATE_FAILURE_INSTALL_FAILED".to_string());
        assert!(matches!(misplaced.validate(), Err(OtaRolloutError::InvalidInput { .. })));
        // A failure state outside the closed set.
        let mut foreign = failed_report("d1", "a1", 1_000);
        foreign.failure_state = Some("SOME_VENDOR_FAILURE".to_string());
        assert!(matches!(
            foreign.validate(),
            Err(OtaRolloutError::ReportStateUnknown { .. })
        ));
        // A well-formed failure report validates.
        failed_report("d1", "a1", 1_000).validate().expect("validates");
    }

    #[test]
    fn the_happy_path_walks_canary_then_block_to_completion() {
        let (mut coordinator, own_sink, orchestrator_sink) = coordinator(
            vec![
                definition("canary", true, &["d1", "d2"], None),
                definition("block-1", false, &["d3", "d4"], None),
            ],
        );
        let canary = coordinator.start().expect("starts");
        assert_eq!(canary, "canary");
        for device in ["d1", "d2"] {
            coordinator
                .authorize_device(device, &passed_attestation())
                .expect("authorized");
        }
        // Progress + terminal reports for the canary.
        for device in ["d1", "d2"] {
            coordinator
                .receive(&report(device, &format!("attempt-{device}"), "UPDATE_DOWNLOADING", 1_100))
                .expect("progress accepted");
            coordinator
                .receive(&report(
                    device,
                    &format!("attempt-{device}"),
                    "UPDATE_VERIFIED_PENDING_INSTALL",
                    1_200,
                ))
                .expect("verify evidence accepted");
            coordinator
                .receive(&report(
                    device,
                    &format!("attempt-{device}"),
                    "UPDATE_HEALTH_CHECK_PENDING",
                    1_300,
                ))
                .expect("install evidence accepted");
            coordinator
                .receive(&report(device, &format!("attempt-{device}"), "UPDATE_ACTIVE", 1_400))
                .expect("terminal accepted");
        }
        assert_eq!(coordinator.phase(), RolloutPhase::InProgress);
        // The declared-order advance to the wider cohort.
        let next = coordinator
            .advance_cohort("J. Doe (ops)", "canary clean: 2/2 success")
            .expect("advances");
        assert_eq!(next, "block-1");
        for device in ["d3", "d4"] {
            coordinator
                .authorize_device(device, &passed_attestation())
                .expect("authorized");
            coordinator
                .receive(&report(device, &format!("attempt-{device}"), "UPDATE_ACTIVE", 1_500))
                .expect("terminal accepted");
        }
        // The final advance reports COMPLETE (the orchestrator's recorded
        // completion signal) and the phase is Complete.
        assert!(matches!(
            coordinator.advance_cohort("J. Doe (ops)", "block-1 clean"),
            Err(OtaRolloutError::OrchestratorRefused { code }) if code == "ROLLOUT_COMPLETE"
        ));
        assert_eq!(coordinator.phase(), RolloutPhase::Complete);
        assert!(!coordinator.is_frozen());
        // The audit trail: assignment, report, progress, evidence —
        // coordinator side; plan/cohort/auth/outcome/advance — the
        // orchestrator's own records.
        let own = own_sink.records();
        assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::CohortAssigned
            && r.subject == "canary"));
        assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::DeviceAssigned
            && r.subject == "canary/d1"));
        assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::ProgressRecorded));
        assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::EvidenceRecorded));
        assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::ReportReceived));
        let orchestrator_records = orchestrator_sink.records();
        assert!(orchestrator_records
            .iter()
            .any(|r| r.kind.name() == "FLEET_ROLLOUT_COHORT_ADVANCED"));
        // The per-attempt evidence chains: 4 records each (Attempt,
        // Verify, Install, Result).
        let (count, head) = coordinator
            .attempt_evidence("attempt-d1")
            .expect("the attempt's evidence exists");
        assert_eq!(count, 4);
        assert!(!head.is_empty());
    }

    #[test]
    fn the_rate_threshold_aborts_the_canary_and_the_abort_freezes_the_rollout() {
        let (mut coordinator, own_sink, orchestrator_sink) = coordinator(
            vec![
                definition("canary", true, &["d1", "d2", "d3"], None),
                definition("block-1", false, &["d4", "d5"], None),
            ],
        );
        coordinator.start().expect("starts");
        for device in ["d1", "d2", "d3"] {
            coordinator
                .authorize_device(device, &passed_attestation())
                .expect("authorized");
        }
        // d1 succeeds (1 success). d2 fails: 1 failure / 2 observations =
        // 500 permille = EQUAL to the threshold — the recorded semantics
        // keep going (an equal rate does not abort). d3 fails: 2 failures
        // / 3 observations = 666 permille > 500 — ABORT.
        coordinator
            .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
            .expect("success recorded");
        assert!(!coordinator.is_frozen(), "an equal rate does not abort");
        coordinator
            .receive(&failed_report("d2", "a2", 1_200))
            .expect("failure recorded");
        assert!(!coordinator.is_frozen(), "1/2 = 500 permille is not > 500");
        coordinator
            .receive(&failed_report("d3", "a3", 1_300))
            .expect("failure recorded");
        assert_eq!(coordinator.phase(), RolloutPhase::Aborted);
        assert!(coordinator.is_frozen());
        assert!(coordinator
            .freeze_reason()
            .expect("the freeze reason is recorded")
            .contains("ABORTED"));
        // THE FREEZE: no further authorization, report, or advance.
        assert!(matches!(
            coordinator.authorize_device("d4", &passed_attestation()),
            Err(OtaRolloutError::FrozenRollout { .. })
        ));
        assert!(matches!(
            coordinator.receive(&report("d4", "a4", "UPDATE_DOWNLOADING", 1_400)),
            Err(OtaRolloutError::FrozenRollout { .. })
        ));
        assert!(matches!(
            coordinator.advance_cohort("J. Doe (ops)", "ignored — frozen"),
            Err(OtaRolloutError::FrozenRollout { .. })
        ));
        // The orchestrator's own abort record AND the coordinator's freeze
        // record are both present (one event, two honest layers).
        assert!(orchestrator_sink
            .records()
            .iter()
            .any(|r| r.kind.name() == "FLEET_ROLLOUT_ABORTED"));
        assert!(own_sink
            .records()
            .iter()
            .any(|r| r.kind == OtaRolloutAuditKind::Frozen));
        // The failures landed as evidence (Result/FAILED records).
        let (count, _) = coordinator.attempt_evidence("a2").expect("evidence");
        assert_eq!(count, 2, "Attempt + Result for the failing attempt");
    }

    #[test]
    fn the_per_cohort_max_failing_devices_tripwire_freezes_independently() {
        // A high rate threshold (1000 permille never fires on its own at
        // these counts) + a strict per-cohort count cap of ZERO: the first
        // failing device freezes the rollout even though the rate
        // threshold does not.
        let (plan, definitions) = build_plan(
            "2.0.0",
            vec![
                definition("canary", true, &["d1", "d2"], Some(0)),
                definition("block-1", false, &["d3"], None),
            ],
            AbortThresholds { max_failure_rate_permille: 1000, min_observations: 1 },
            500,
        )
        .expect("plan builds");
        let own_sink = Arc::new(MemoryOtaRolloutAuditSink::new());
        let mut coordinator = OtaRolloutCoordinator::new(
            plan,
            definitions,
            Box::new(MemoryRolloutAuditSink::new()),
            own_sink.clone() as Arc<dyn OtaRolloutAuditSink>,
            test_namespace_key(),
            Arc::new(FixedClock(REMOTE_CLOCK_MILLIS)),
        );
        coordinator.start().expect("starts");
        for device in ["d1", "d2"] {
            coordinator
                .authorize_device(device, &passed_attestation())
                .expect("authorized");
        }
        coordinator
            .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
            .expect("success recorded");
        coordinator
            .receive(&failed_report("d2", "a2", 1_200))
            .expect("failure recorded");
        // The rate is 1/2 = 500 permille <= 1000 — the orchestrator did
        // NOT abort; the COUNT tripwire did (1 > 0).
        assert_eq!(coordinator.phase(), RolloutPhase::InProgress);
        assert!(coordinator.is_frozen());
        assert!(coordinator
            .freeze_reason()
            .expect("the freeze reason")
            .contains("max_failing_devices=0"));
        // Frozen: the third canary device's report is refused.
        assert!(matches!(
            coordinator.receive(&report("d1", "a3", "UPDATE_ACTIVE", 1_300)),
            Err(OtaRolloutError::FrozenRollout { .. })
        ));
    }

    #[test]
    fn a_rollback_report_counts_as_a_failure_and_lands_rollback_evidence() {
        let (mut coordinator, own_sink, _) = coordinator(
            vec![definition("canary", true, &["d1"], None)],
        );
        coordinator.start().expect("starts");
        coordinator
            .authorize_device("d1", &passed_attestation())
            .expect("authorized");
        let mut rolled = report("d1", "a1", "UPDATE_ROLLED_BACK", 1_100);
        rolled.failure_state = Some("UPDATE_FAILURE_HEALTH_CHECK_FAILED".to_string());
        let receipt = coordinator.receive(&rolled).expect("accepted");
        assert!(receipt.outcome_recorded);
        assert_eq!(receipt.outcome_name, "DEVICE_UPDATE_ROLLED_BACK");
        // The snapshot counts the rolled-back device.
        let snapshot = coordinator.fleet_snapshot();
        assert_eq!(snapshot.cohorts[0].rolled_back, 1);
        // The rollback evidence record is the P18.07 Rollback kind.
        let (count, _) = coordinator.attempt_evidence("a1").expect("evidence");
        assert_eq!(count, 2, "Attempt + Rollback");
        assert!(own_sink
            .records()
            .iter()
            .any(|r| r.kind == OtaRolloutAuditKind::EvidenceRecorded
                && r.detail.contains("records=2")));
    }

    #[test]
    fn unassigned_and_duplicate_reports_are_refused() {
        let (mut coordinator, own_sink, _) = coordinator(
            vec![definition("canary", true, &["d1"], None)],
        );
        coordinator.start().expect("starts");
        // An unassigned device (never authorized) reports — refused.
        assert!(matches!(
            coordinator.receive(&report("dX", "aX", "UPDATE_ACTIVE", 1_100)),
            Err(OtaRolloutError::DeviceNotAssigned { .. })
        ));
        coordinator
            .authorize_device("d1", &passed_attestation())
            .expect("authorized");
        coordinator
            .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
            .expect("first terminal accepted");
        // A duplicate terminal report for the same device — refused (the
        // append-once rule, pre-checked and backstopped by the
        // orchestrator).
        assert!(matches!(
            coordinator.receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_200)),
            Err(OtaRolloutError::OrchestratorRefused { code })
            if code == "ROLLOUT_DEVICE_ALREADY_OUTCOME"
        ));
        assert!(own_sink
            .records()
            .iter()
            .any(|r| r.kind == OtaRolloutAuditKind::ReportRefused));
    }

    #[test]
    fn the_statement_pins_the_honesty_bounds() {
        assert!(OTA_ROLLOUT_STATEMENT.contains("EXISTING P18.06 orchestrator"));
        assert!(OTA_ROLLOUT_STATEMENT.contains("never HTTP"));
        assert!(OTA_ROLLOUT_STATEMENT.contains("FREEZES the rollout"));
        assert!(OTA_ROLLOUT_STATEMENT.contains("no rollout has ever run"));
        assert!(OTA_ROLLOUT_STATEMENT.contains("byte-identical"));
        // The closed vocabularies are the P18.04 names, consumed by
        // citation.
        assert_eq!(
            OTA_REPORT_STATE_NAMES,
            [
                "UPDATE_IDLE",
                "UPDATE_DOWNLOADING",
                "UPDATE_VERIFYING",
                "UPDATE_VERIFIED_PENDING_INSTALL",
                "UPDATE_INSTALLING",
                "UPDATE_HEALTH_CHECK_PENDING",
                "UPDATE_ACTIVE",
                "UPDATE_ROLLING_BACK",
                "UPDATE_ROLLED_BACK",
                "UPDATE_FAILED",
            ]
        );
        assert_eq!(
            OTA_REPORT_FAILURE_STATE_NAMES,
            [
                "UPDATE_FAILURE_DOWNLOAD_FAILED",
                "UPDATE_FAILURE_VERIFICATION_REFUSED",
                "UPDATE_FAILURE_INSTALL_FAILED",
                "UPDATE_FAILURE_HEALTH_CHECK_FAILED",
                "UPDATE_FAILURE_ROLLBACK_FAILED",
                "UPDATE_FAILURE_CANCELLED",
            ]
        );
    }
}
