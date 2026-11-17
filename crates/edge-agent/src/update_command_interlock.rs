// P18.15 — The update-safety interlock: the update/command-lifecycle
//          interplay (phase-18 plan §2 row P18.15; decision record
//          `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/
//          records/28_p18_15_update_command_interlock.md`).
//
// THE BLOCKING RULE (the phase-plan P18.15 row, made REAL here — the wave-1
// `UpdateSessionStateMachine::begin_download` refusal consumed it by
// citation; this module is the device-state check that feeds it):
// updates are BLOCKED while any command on the device is in a NON-TERMINAL
// frozen §14.1 lifecycle state. The RECORDED citation names ISSUED/TRACKING
// (`UpdateSessionError::CommandsInFlight`, record 04 §1); the rule
// implemented here is a STRICT SUPERSET — it also blocks ACKNOWLEDGED,
// FAILED, and EXPIRED (the frozen table's non-terminal states the narrower
// citation would have missed: FAILED/EXPIRED exit only via FALLBACK, so a
// command in either is still in flight) and any pre-issue state observed on
// a device (itself a defect; refusing the update while it stands is the
// fail-closed reading). A command is DRAINED only when it reaches one of
// the five frozen TERMINAL states. The strengthening is recorded, never
// silent: record 28 §2 states the delta against the citation verbatim, and
// the refusal keeps the recorded stable name
// (`UPDATE_BLOCKED_COMMANDS_IN_FLIGHT`) so the wave-1 tests and P18.14's
// concurrent-command tests cite ONE code.
//
// DRAIN-BEFORE-UPDATE (report §5.6, cited, never rewritten): "For
// command-producing plugins, activation or upgrade must not occur in the
// middle of an active dispatch transaction. Drain the plugin, stop
// accepting new work, finish or cancel pending operations, record the
// version change, and then replace it." The device analog is the
// [`CommandDrainProcedure`] below: it OBSERVES the command path (it never
// transitions a command state — the "finish or cancel pending operations"
// leg is the operator's action through the normal frozen lifecycle, e.g.
// the frozen ISSUED → CANCELLED edge; an update path that wrote command
// states would bypass the command lifecycle, which plan §4 forbids), and
// exposes the "stop accepting new work" read
// ([`UpdateCommandInterlock::allows_new_command_work`]) for the
// composition to consult. The command path's own P6.14 semantics are
// UNTOUCHED by this module (additive only).
//
// THE DEVICE-STATE SOURCE (fail-closed): the interlock consumes the
// command path's recorded states through the [`DeviceCommandStateSource`]
// port — the production wiring binds it to the P6.16 engine's recorded
// §14.1 states (`crate::command_forward::CommandForwardingEngine::
// lifecycle_state_of` over the ids the composition issued; the engine's
// tracking map is private, so the itemized wiring is a coordinator item in
// record 28 §6). A source that CANNOT establish the device's command state
// refuses, and the interlock refuses the update with
// [`UpdateBlockRefusal::DeviceStateUnknown`] — an unobservable command
// state is never read as "no commands in flight". A state name outside the
// frozen 14-state vocabulary is likewise refused, never guessed at. The
// frozen 14-state/27-edge set is CITED (`crate::command_path::
// FROZEN_COMMAND_STATE_NAMES`; `flexcore_domain::command_state` is the
// normative source the edge cannot depend on — the P6.05 rule), never
// forked: the terminal set here is the frozen five.
//
// POST-UPDATE CONFORMANCE RE-CHECK (the P6.13 interplay): after an update
// session reaches `Active`, the device's adapter read path is re-validated
// against the §19.2(6) fault-injection set — the composition runs the
// EXISTING harness (`crate::conformance::ConformanceHarness::
// run_fault_injection_set`) and hands the report to
// [`UpdateCommandInterlock::record_conformance_report`], which consumes
// `ConformanceReport::is_clean()` (consumed, never reimplemented). Until a
// CLEAN report for the latest activation is recorded, further update
// authorization is refused: outstanding →
// [`UpdateBlockRefusal::ConformanceRecheckOutstanding`]; a VIOLATING report
// fails the re-check terminally (further updates refused,
// [`UpdateBlockRefusal::ConformanceRecheckFailed`]) until a RECORDED
// OVERRIDE ([`RecheckOverride`]: named actor + authority reference — the
// P9.34 `RollbackOverride` pattern, cited) clears it. The re-check is
// consumed-by-citation of the P6.13/P6.19 boundary: a clean report proves
// ONLY the §19.2(6) contract; it is NOT protocol conformance, NOT
// certification, and never quoted as either (the P6.19 claim boundary
// stands verbatim).
//
// COMPOSITION (the two-step surface P18.14's tests cite):
//
//   let auth = interlock.authorize_update_attempt()?;          // this module
//   session.begin_download(auth.in_flight_count, now)?;        // P18.04, unchanged
//
// `auth.in_flight_count` is 0 on every `Ok` (the rule passed), so the
// wave-1 session rule is fed EXACTLY the interlock's verdict; the two rules
// can never disagree in the safe direction.
//
// HONESTY BOUNDS: no command exists, no update has ever been applied, no
// fleet exists — every flow here is exercised only by WRITTEN-UNRUN tests
// against in-memory doubles (execution is the human gate). The P6.16
// engine's tracking state is in-memory and a restart clears it (the
// engine's own recorded limitation): after a restart the source reports
// the (possibly empty) RECORDED set, and a deployment that cannot
// reconstruct in-flight command state after restart must keep updates
// blocked until it can — recorded in record 28 §6 as a deployment
// consideration, not solved here. No certification or security claim is
// made; the conformance re-check makes no protocol-conformance claim.

use std::collections::BTreeMap;
use std::sync::Arc;

use flexcore_ports::clock::Clock;

use crate::command_path::FROZEN_COMMAND_STATE_NAMES;
use crate::conformance::ConformanceReport;
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Honesty statement -------------------------------------------------------------

/// The P18.15 statement (pinned so tests and reviewers can read what IS and
/// IS NOT claimed by the interlock).
pub const INTERLOCK_STATEMENT: &str = "the P18.15 interlock BLOCKS updates while any \
     command on the device is in a non-terminal frozen §14.1 lifecycle state (a strict \
     superset of the recorded ISSUED/TRACKING citation: ACKNOWLEDGED, FAILED, EXPIRED, \
     and any observed pre-issue state also block — the strengthening is recorded in \
     record 28 §2), refuses fail-closed when the device command state is unknown, \
     provides the report §5.6 drain-before-update procedure as an OBSERVER (it never \
     transitions a command state), and requires a clean post-update conformance \
     re-check (the P6.13 harness consumed, never reimplemented) after every activation. \
     No command exists, no update has ever been applied, no fleet exists; every exercise \
     is a WRITTEN-UNRUN test against in-memory doubles. A clean re-check proves ONLY the \
     §19.2(6) edge read-path contract (P6.13/P6.19) — never protocol conformance, never \
     certification.";

/// The frozen TERMINAL states of the §14.1 lifecycle (the drain targets).
/// CITED, never forked: `flexcore_domain::command_state::is_terminal` is
/// the normative source (the edge crate has no `flexcore-domain`
/// dependency — the P6.05 rule — so the names travel as the canonical
/// SCREAMING_SNAKE strings validated against
/// [`FROZEN_COMMAND_STATE_NAMES`]).
pub const FROZEN_TERMINAL_COMMAND_STATE_NAMES: [&str; 5] = [
    "COMPLETED",
    "REJECTED",
    "CANCELLED",
    "SUPERSEDED",
    "FALLBACK",
];

// --- The device command-state observation -------------------------------------------

/// One command observed on THIS device by the command path's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCommandObservation {
    /// The command id (the §14.1 subject; identifiers only).
    pub command_id: String,
    /// The frozen §14.1 lifecycle state name (validated against
    /// [`FROZEN_COMMAND_STATE_NAMES`] — an unknown name is refused, never
    /// carried).
    pub state_name: String,
}

impl DeviceCommandObservation {
    /// Validates the observation (fail-closed on an empty id or a state
    /// name outside the frozen vocabulary).
    pub fn validate(&self) -> Result<(), InterlockError> {
        if self.command_id.trim().is_empty() {
            return Err(InterlockError::InvalidInput {
                reason: "the observed command id is empty".to_string(),
            });
        }
        if !FROZEN_COMMAND_STATE_NAMES.contains(&self.state_name.as_str()) {
            return Err(InterlockError::InvalidInput {
                reason: format!(
                    "observed state '{}' is not one of the frozen §14.1 lifecycle states — \
                     refused, never guessed at",
                    self.state_name
                ),
            });
        }
        Ok(())
    }

    /// Whether the observation's state is one of the frozen TERMINAL
    /// states (the drain targets).
    pub fn is_terminal(&self) -> bool {
        FROZEN_TERMINAL_COMMAND_STATE_NAMES.contains(&self.state_name.as_str())
    }
}

/// Why the device command state could not be established. A source that
/// cannot establish the state REFUSES — the absence of an answer is never
/// read as "no commands in flight" (fail-closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterlockStateError {
    pub reason: String,
}

impl std::fmt::Display for InterlockStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the device command state is unknown: {}", self.reason)
    }
}

impl std::error::Error for InterlockStateError {}

/// The port the interlock consumes to read the command path's recorded
/// §14.1 states for THIS device. PRODUCTION WIRING (record 28 §6): the
/// composition root binds this to the P6.16 engine's recorded states
/// (`CommandForwardingEngine::lifecycle_state_of` over the ids it issued).
/// The port returns the FULL recorded set — terminal states included — so
/// the interlock applies the frozen terminal rule itself (the source never
/// filters; filtering in the source would hide the drain gap from the
/// audit).
pub trait DeviceCommandStateSource: Send + Sync {
    /// The command path's recorded command states for this device, or a
    /// named refusal when the state cannot be established.
    fn device_command_states(&self) -> Result<Vec<DeviceCommandObservation>, InterlockStateError>;
}

/// An in-memory [`DeviceCommandStateSource`] — the TEST DOUBLE the
/// written-unrun tests compose (labeled; never a production source; the
/// production wiring is the composition-root item in record 28 §6).
#[derive(Default)]
pub struct InMemoryCommandStateSource {
    observations: std::sync::Mutex<Vec<DeviceCommandObservation>>,
    refuse_reason: std::sync::Mutex<Option<String>>,
}

impl InMemoryCommandStateSource {
    /// A source reporting the given observations.
    pub fn reporting(observations: Vec<DeviceCommandObservation>) -> Self {
        Self {
            observations: std::sync::Mutex::new(observations),
            refuse_reason: std::sync::Mutex::new(None),
        }
    }

    /// Sets a named refusal: every read fails closed (the unobservable-
    /// device case).
    pub fn set_unobservable(&self, reason: String) {
        *self
            .refuse_reason
            .lock()
            .expect("poisoned test-source mutex") = Some(reason);
    }

    /// Replaces the observations (the drain-walk test double).
    pub fn set_observations(&self, observations: Vec<DeviceCommandObservation>) {
        *self
            .observations
            .lock()
            .expect("poisoned test-source mutex") = observations;
    }
}

impl DeviceCommandStateSource for InMemoryCommandStateSource {
    fn device_command_states(&self) -> Result<Vec<DeviceCommandObservation>, InterlockStateError> {
        if let Some(reason) = self
            .refuse_reason
            .lock()
            .expect("poisoned test-source mutex")
            .clone()
        {
            return Err(InterlockStateError { reason });
        }
        Ok(self
            .observations
            .lock()
            .expect("poisoned test-source mutex")
            .clone())
    }
}

// --- The interlock errors and refusals ----------------------------------------------

/// The named errors of the interlock's own inputs and procedures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterlockError {
    /// An input was unusable (empty attempt id, an unvalidated observation,
    /// a drain already active, …).
    InvalidInput { reason: String },
    /// The device command state could not be established (the drain cannot
    /// start or complete while unobservable — fail-closed).
    DeviceStateUnknown { reason: String },
}

impl InterlockError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            InterlockError::InvalidInput { .. } => "UPDATE_INTERLOCK_INPUT_INVALID",
            InterlockError::DeviceStateUnknown { .. } => "UPDATE_INTERLOCK_STATE_UNKNOWN",
        }
    }
}

impl std::fmt::Display for InterlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterlockError::InvalidInput { reason } => {
                write!(f, "the update interlock input is invalid: {reason}")
            }
            InterlockError::DeviceStateUnknown { reason } => write!(
                f,
                "the device command state is unknown — the interlock refuses fail-closed: \
                 {reason}"
            ),
        }
    }
}

impl std::error::Error for InterlockError {}

/// Why an update attempt was BLOCKED. Every variant is a NAMED, audited
/// refusal; the stable names align with the recorded wave-1 vocabulary
/// where the citation exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateBlockRefusal {
    /// The device command state could not be established — never read as
    /// "no commands in flight" (fail-closed).
    DeviceStateUnknown { reason: String },
    /// Commands are in flight on the device (non-terminal §14.1 states).
    /// The field name matches the wave-1 recorded citation
    /// (`UpdateSessionError::CommandsInFlight`); the count INCLUDES the
    /// drain-gap states (ACKNOWLEDGED/FAILED/EXPIRED) per the recorded
    /// strengthening (record 28 §2).
    CommandsInFlight { issued_or_tracking: usize },
    /// A previous update reached `Active` and its post-update conformance
    /// re-check has not been recorded yet.
    ConformanceRecheckOutstanding { attempt_id: String },
    /// The previous update's conformance re-check FAILED (a violating
    /// §19.2(6) report) — terminal until a recorded override clears it.
    ConformanceRecheckFailed { attempt_id: String, violations: usize },
}

impl UpdateBlockRefusal {
    /// The stable machine-readable refusal name (audit text). The
    /// in-flight name is the RECORDED wave-1 code — one citation target
    /// for the wave-1 tests and P18.14's concurrent-command tests.
    pub fn name(&self) -> &'static str {
        match self {
            UpdateBlockRefusal::DeviceStateUnknown { .. } => "UPDATE_INTERLOCK_STATE_UNKNOWN",
            UpdateBlockRefusal::CommandsInFlight { .. } => "UPDATE_BLOCKED_COMMANDS_IN_FLIGHT",
            UpdateBlockRefusal::ConformanceRecheckOutstanding { .. } => {
                "UPDATE_CONFORMANCE_RECHECK_OUTSTANDING"
            }
            UpdateBlockRefusal::ConformanceRecheckFailed { .. } => {
                "UPDATE_CONFORMANCE_RECHECK_FAILED"
            }
        }
    }
}

impl std::fmt::Display for UpdateBlockRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateBlockRefusal::DeviceStateUnknown { reason } => write!(
                f,
                "the update is BLOCKED: the device command state is unknown ({reason}) — \
                 never read as no-commands-in-flight (P18.15 fail-closed)"
            ),
            UpdateBlockRefusal::CommandsInFlight { issued_or_tracking } => write!(
                f,
                "the update is BLOCKED: {issued_or_tracking} command(s) are in a \
                 non-terminal §14.1 state on this device — drain to a terminal state \
                 first (report §5.6; P18.15 interlock; the recorded ISSUED/TRACKING \
                 citation plus the ACKNOWLEDGED/FAILED/EXPIRED drain gap)"
            ),
            UpdateBlockRefusal::ConformanceRecheckOutstanding { attempt_id } => write!(
                f,
                "the update is BLOCKED: the post-update conformance re-check for attempt \
                 '{attempt_id}' is OUTSTANDING — run the P6.13 harness and record the \
                 report first (P18.15)"
            ),
            UpdateBlockRefusal::ConformanceRecheckFailed { attempt_id, violations } => write!(
                f,
                "the update is BLOCKED: the post-update conformance re-check for attempt \
                 '{attempt_id}' FAILED with {violations} contract violation(s) — terminal \
                 until a recorded override clears it (P18.15)"
            ),
        }
    }
}

// --- The drain procedure (report §5.6, OBSERVER-only) ---------------------------------

/// The drain procedure's phase. The procedure OBSERVES the command path;
/// it never transitions a command state (the "finish or cancel pending
/// operations" leg is the operator's action through the normal frozen
/// lifecycle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainPhase {
    /// No drain is in progress.
    NotDraining,
    /// Draining: new command work should not be admitted (the composition
    /// consults [`UpdateCommandInterlock::allows_new_command_work`]); the
    /// recorded in-flight ids at drain start (sorted, deterministic).
    Draining {
        started_at_unix_millis: i64,
        commands_at_start: Vec<String>,
    },
    /// Every command observed at drain start has reached a frozen TERMINAL
    /// state (the §5.6 "finish or cancel pending operations" leg, observed
    /// complete).
    Drained { completed_at_unix_millis: i64 },
}

// --- The post-update conformance re-check state ---------------------------------------

/// The re-check phase for the LATEST update activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecheckPhase {
    /// No update has been activated through this interlock.
    NotRequired,
    /// The latest activation's re-check has not been recorded yet.
    Outstanding,
    /// A CLEAN §19.2(6) report was recorded for the latest activation
    /// (proves the edge read-path contract ONLY — the P6.19 claim boundary).
    Satisfied,
    /// A VIOLATING report was recorded — terminal until a recorded
    /// override clears it.
    Failed { violations: usize },
}

/// The recorded override that clears a FAILED conformance re-check (the
/// P9.34 `RollbackOverride` pattern, cited: a named actor + an authority
/// reference, never a silent clear).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecheckOverride {
    /// The operator/authority that signed the override.
    pub actor: String,
    /// The authority record/ticket the override is written under.
    pub authority_reference: String,
}

impl RecheckOverride {
    fn validate(&self) -> Result<(), InterlockError> {
        let mut problems: Vec<String> = Vec::new();
        if self.actor.trim().is_empty() {
            problems.push("actor is empty".to_string());
        }
        if self.authority_reference.trim().is_empty() {
            problems.push("authority_reference is empty".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(InterlockError::InvalidInput {
                reason: format!(
                    "the re-check override is invalid: {} (an override without a named \
                     actor and an authority reference is not a recorded override)",
                    problems.join("; ")
                ),
            })
        }
    }
}

/// The re-check state for one activation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckState {
    attempt_id: Option<String>,
    phase: RecheckPhase,
}

// --- The authorization verdict ---------------------------------------------------------

/// The update-attempt authorization: the ONLY `Ok` this interlock returns.
/// `in_flight_count` is 0 on every authorization (the rule passed) — the
/// exact value the composition hands to the wave-1
/// `UpdateSessionStateMachine::begin_download`, so the recorded session
/// rule and this interlock can never disagree in the unsafe direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAttemptAuthorization {
    /// Always 0 on `Ok` — handed verbatim to `begin_download`.
    pub in_flight_count: usize,
    /// The authorization instant (the INJECTED clock).
    pub at_unix_millis: i64,
    /// The recorded scope note (what this authorization does NOT mean).
    pub scope_note: String,
}

// --- The interlock ----------------------------------------------------------------------

/// The P18.15 update-safety interlock for ONE device: the blocking rule,
/// the drain-before-update procedure, and the post-update conformance
/// re-check. Read-only toward the command lifecycle (it never transitions
/// a command state) and additive toward the update path (the P6.14
/// receiver and the P18.04 session semantics are untouched — the
/// composition composes them through the two-step surface in the module
/// doc).
pub struct UpdateCommandInterlock {
    command_states: Arc<dyn DeviceCommandStateSource>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    drain: DrainPhase,
    recheck: RecheckState,
}

impl UpdateCommandInterlock {
    /// Composes the interlock over the device-state source, the audit
    /// sink, and the injected clock. No default source exists: an
    /// interlock that cannot read the command path's states refuses every
    /// update (fail-closed by composition).
    pub fn new(
        command_states: Arc<dyn DeviceCommandStateSource>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            command_states,
            audit,
            clock,
            drain: DrainPhase::NotDraining,
            recheck: RecheckState {
                attempt_id: None,
                phase: RecheckPhase::NotRequired,
            },
        }
    }

    /// The current drain phase (health/composition visibility).
    pub fn drain_phase(&self) -> &DrainPhase {
        &self.drain
    }

    /// The current re-check phase (health/composition visibility).
    pub fn recheck_phase(&self) -> &RecheckPhase {
        &self.recheck.phase
    }

    /// The latest activation's attempt id (the re-check subject).
    pub fn recheck_attempt_id(&self) -> Option<&str> {
        self.recheck.attempt_id.as_deref()
    }

    /// The report §5.6 "stop accepting new work" read: FALSE while a drain
    /// is in progress. The composition consults this before admitting new
    /// command work during a drain; the command path's own P6.14 semantics
    /// are untouched by this module.
    pub fn allows_new_command_work(&self) -> bool {
        !matches!(self.drain, DrainPhase::Draining { .. })
    }

    /// Reads and validates the device's command states. The shared
    /// fail-closed read for every rule below: a source error OR an
    /// unvalidatable observation is
    /// [`UpdateBlockRefusal::DeviceStateUnknown`], never "no commands".
    fn read_validated_states(
        &self,
    ) -> Result<Vec<DeviceCommandObservation>, InterlockError> {
        let observations = self
            .command_states
            .device_command_states()
            .map_err(|error| InterlockError::DeviceStateUnknown {
                reason: error.reason,
            })?;
        for observation in &observations {
            observation.validate().map_err(|error| {
                InterlockError::DeviceStateUnknown {
                    reason: error.to_string(),
                }
            })?;
        }
        Ok(observations)
    }

    /// The non-terminal (in-flight) observations, sorted by command id
    /// (BTreeMap determinism — the program's deterministic-collection
    /// discipline).
    fn in_flight(observations: &[DeviceCommandObservation]) -> Vec<DeviceCommandObservation> {
        let mut by_id: BTreeMap<&str, &DeviceCommandObservation> = BTreeMap::new();
        for observation in observations {
            if !observation.is_terminal() {
                by_id.insert(observation.command_id.as_str(), observation);
            }
        }
        by_id.into_values().cloned().collect()
    }

    /// THE BLOCKING RULE (the P18.15 row, made real). Evaluation order
    /// (recorded; deterministic refusal):
    ///
    /// 1. the post-update conformance re-check of the LATEST activation —
    ///    `Outstanding` → [`UpdateBlockRefusal::ConformanceRecheckOutstanding`];
    ///    `Failed` → [`UpdateBlockRefusal::ConformanceRecheckFailed`]
    ///    (terminal until a recorded override);
    /// 2. the device command states, read through the source —
    ///    unobservable or unvalidatable →
    ///    [`UpdateBlockRefusal::DeviceStateUnknown`] (fail-closed);
    /// 3. any command in a NON-TERMINAL frozen §14.1 state (the recorded
    ///    ISSUED/TRACKING citation PLUS the ACKNOWLEDGED/FAILED/EXPIRED
    ///    drain gap, record 28 §2) →
    ///    [`UpdateBlockRefusal::CommandsInFlight`] with the count;
    /// 4. otherwise authorized with `in_flight_count == 0` — the value the
    ///    composition hands to the wave-1 `begin_download`.
    ///
    /// Every refusal is audited (`EDGE_UPDATE_INTERLOCK_BLOCKED`, with the
    /// stable refusal name); the authorization is audited
    /// (`EDGE_UPDATE_INTERLOCK_AUTHORIZED`). The interlock holds NO update
    /// authority of its own: this verdict is ONE gate among the P18.13
    /// attestation gate, the P18.06 orchestrator, and the untouched P9.34
    /// verification-before-any-write surface.
    pub fn authorize_update_attempt(&self) -> Result<UpdateAttemptAuthorization, UpdateBlockRefusal> {
        // 1. The post-update re-check gate.
        match &self.recheck.phase {
            RecheckPhase::Outstanding => {
                let refusal = UpdateBlockRefusal::ConformanceRecheckOutstanding {
                    attempt_id: self
                        .recheck
                        .attempt_id
                        .clone()
                        .unwrap_or_default(),
                };
                self.audit_blocked(&refusal);
                return Err(refusal);
            }
            RecheckPhase::Failed { violations } => {
                let refusal = UpdateBlockRefusal::ConformanceRecheckFailed {
                    attempt_id: self.recheck.attempt_id.clone().unwrap_or_default(),
                    violations: *violations,
                };
                self.audit_blocked(&refusal);
                return Err(refusal);
            }
            RecheckPhase::NotRequired | RecheckPhase::Satisfied => {}
        }
        // 2. The device command states (fail-closed on unknown).
        let observations = match self.read_validated_states() {
            Ok(observations) => observations,
            Err(error) => {
                let refusal = UpdateBlockRefusal::DeviceStateUnknown {
                    reason: error.to_string(),
                };
                self.audit_blocked(&refusal);
                return Err(refusal);
            }
        };
        // 3. The blocking rule (the strict superset of the recorded
        //    citation).
        let in_flight = Self::in_flight(&observations);
        if !in_flight.is_empty() {
            let refusal = UpdateBlockRefusal::CommandsInFlight {
                issued_or_tracking: in_flight.len(),
            };
            self.audit_blocked(&refusal);
            return Err(refusal);
        }
        // 4. Authorized (one gate among several — see the doc).
        let now = self.now_unix_millis();
        let authorization = UpdateAttemptAuthorization {
            in_flight_count: 0,
            at_unix_millis: now,
            scope_note: "one gate among several: the P18.13 attestation gate, the P18.06 \
                         orchestrator, and the P9.34 verification-before-any-write surface \
                         still apply; this verdict is NOT an update authorization by itself"
                .to_string(),
        };
        self.audit_event(
            "EDGE_UPDATE_INTERLOCK_AUTHORIZED",
            format!(
                "device command state readable, 0 command(s) in a non-terminal §14.1 state, \
                 re-check phase {:?} — update attempt may proceed to the session rule \
                 (begin_download(0, now); P18.15)",
                self.recheck.phase,
            ),
        );
        Ok(authorization)
    }

    /// Begins the report §5.6 drain: records the in-flight set, stops new
    /// command work ([`UpdateCommandInterlock::allows_new_command_work`]
    /// reads FALSE from here), and waits for the operator's "finish or
    /// cancel pending operations" leg to drain them to terminal states.
    /// Fail-closed: a drain cannot START while the device state is
    /// unobservable. A device already fully drained ends in
    /// [`DrainPhase::Drained`] immediately. Audited. The procedure NEVER
    /// transitions a command state.
    pub fn begin_drain(&mut self) -> Result<&DrainPhase, InterlockError> {
        if matches!(self.drain, DrainPhase::Draining { .. }) {
            return Err(InterlockError::InvalidInput {
                reason: "a drain is already in progress".to_string(),
            });
        }
        let observations = self.read_validated_states()?;
        let in_flight = Self::in_flight(&observations);
        let now = self.now_unix_millis();
        let detail = if in_flight.is_empty() {
            self.drain = DrainPhase::Drained {
                completed_at_unix_millis: now,
            };
            "drain requested with 0 command(s) in a non-terminal state — nothing to drain \
             (report §5.6)"
                .to_string()
        } else {
            let commands_at_start: Vec<String> = in_flight
                .iter()
                .map(|o| o.command_id.clone())
                .collect();
            let detail = format!(
                "drain started with {} in-flight command(s) [{}]; new command work should not \
                 be admitted until they reach terminal states (report §5.6); the interlock \
                 only OBSERVES — finishing or cancelling them is the operator's action \
                 through the normal frozen lifecycle",
                commands_at_start.len(),
                commands_at_start.join(","),
            );
            self.drain = DrainPhase::Draining {
                started_at_unix_millis: now,
                commands_at_start,
            };
            detail
        };
        self.audit_event("EDGE_UPDATE_DRAIN_STARTED", detail);
        Ok(&self.drain)
    }

    /// Polls the drain: re-reads the device states. Still in-flight →
    /// [`DrainPhase::Draining`] retained; unobservable → the phase STAYS
    /// `Draining` and a NAMED error is returned (a drain cannot COMPLETE
    /// while the command state is unobservable — fail-closed); all
    /// terminal → [`DrainPhase::Drained`], audited.
    pub fn poll_drain(&mut self) -> Result<&DrainPhase, InterlockError> {
        let started = matches!(self.drain, DrainPhase::Draining { .. });
        if !started {
            return Err(InterlockError::InvalidInput {
                reason: "no drain is in progress".to_string(),
            });
        }
        match self.read_validated_states() {
            Err(error) => {
                self.audit_event(
                    "EDGE_UPDATE_DRAIN_OBSERVED_IN_FLIGHT",
                    format!(
                        "drain poll: the device command state is unobservable ({error}) — the \
                         drain cannot complete while unobservable (fail-closed); the phase \
                         is retained",
                    ),
                );
                Err(error)
            }
            Ok(observations) => {
                let in_flight = Self::in_flight(&observations);
                if in_flight.is_empty() {
                    let now = self.now_unix_millis();
                    self.drain = DrainPhase::Drained {
                        completed_at_unix_millis: now,
                    };
                    self.audit_event(
                        "EDGE_UPDATE_DRAIN_COMPLETE",
                        "every observed command has reached a frozen terminal §14.1 state — \
                         the drain is complete (report §5.6); the update attempt must STILL \
                         pass authorize_update_attempt (the state is re-read at authorize \
                         time)"
                            .to_string(),
                    );
                } else {
                    self.audit_event(
                        "EDGE_UPDATE_DRAIN_OBSERVED_IN_FLIGHT",
                        format!(
                            "drain poll: {} command(s) still in a non-terminal §14.1 state \
                             [{}] — the drain continues",
                            in_flight.len(),
                            in_flight
                                .iter()
                                .map(|o| o.command_id.as_str())
                                .collect::<Vec<_>>()
                                .join(","),
                        ),
                    );
                }
                Ok(&self.drain)
            }
        }
    }

    /// Records that an update session reached [`crate::update_session::
    /// UpdateProgressState::Active`] (the terminal success state) for
    /// `attempt_id`: the post-update conformance re-check becomes
    /// OUTSTANDING for that attempt and further update authorization is
    /// refused until a CLEAN P6.13 report is recorded. Each new activation
    /// supersedes the previous attempt's re-check state (a fresh re-check
    /// per update). Audited.
    pub fn record_update_activated(&mut self, attempt_id: &str) -> Result<(), InterlockError> {
        if attempt_id.trim().is_empty() {
            return Err(InterlockError::InvalidInput {
                reason: "the activated attempt id is mandatory".to_string(),
            });
        }
        self.recheck = RecheckState {
            attempt_id: Some(attempt_id.to_string()),
            phase: RecheckPhase::Outstanding,
        };
        self.audit_event(
            "EDGE_UPDATE_ACTIVATED_RECHECK_OUTSTANDING",
            format!(
                "update attempt '{attempt_id}' reached UPDATE_ACTIVE — the post-update \
                 conformance re-check (P6.13 harness, §19.2(6) set) is OUTSTANDING; further \
                 update authorization is refused until a clean report is recorded (P18.15)"
            ),
        );
        Ok(())
    }

    /// Records ONE §19.2(6) conformance report for the latest activation
    /// (the composition runs the EXISTING `ConformanceHarness` and hands
    /// the report here — consumed, never reimplemented). A CLEAN report →
    /// [`RecheckPhase::Satisfied`]; a VIOLATING report →
    /// [`RecheckPhase::Failed`] — terminal for this attempt until a
    /// [`RecheckOverride`] clears it (re-running the harness to fish for a
    /// pass is refused; each report is audited). Requires an OUTSTANDING
    /// re-check. The claim boundary is the P6.19 one, verbatim: a clean
    /// report proves ONLY the §19.2(6) edge read-path contract — never
    /// protocol conformance, never certification.
    pub fn record_conformance_report(
        &mut self,
        report: &ConformanceReport,
    ) -> Result<&RecheckPhase, InterlockError> {
        let attempt_id = self
            .recheck
            .attempt_id
            .clone()
            .ok_or_else(|| InterlockError::InvalidInput {
                reason: "no update activation is recorded — there is nothing to re-check"
                    .to_string(),
            })?;
        if self.recheck.phase != RecheckPhase::Outstanding {
            return Err(InterlockError::InvalidInput {
                reason: format!(
                    "the re-check for attempt '{attempt_id}' is {:?}, not Outstanding — a \
                     report can only be recorded against an outstanding re-check (a FAILED \
                     re-check is terminal until a recorded override)",
                    self.recheck.phase
                ),
            });
        }
        let violations = report.contract_violations();
        if violations.is_empty() {
            self.recheck.phase = RecheckPhase::Satisfied;
            self.audit_event(
                "EDGE_UPDATE_CONFORMANCE_RECHECK_PASSED",
                format!(
                    "attempt '{attempt_id}': the §19.2(6) report is CLEAN ({} case(s)) — the \
                     edge read-path contract holds after the update; this proves the P6.13 \
                     contract ONLY, never protocol conformance, never certification (P6.19 \
                     boundary)",
                    report.records.len(),
                ),
            );
        } else {
            let count = violations.len();
            self.recheck.phase = RecheckPhase::Failed {
                violations: count,
            };
            self.audit_event(
                "EDGE_UPDATE_CONFORMANCE_RECHECK_FAILED",
                format!(
                    "attempt '{attempt_id}': the §19.2(6) report shows {} contract \
                     violation(s), first: {} — further update authorization is REFUSED until \
                     a recorded override clears it (P18.15; P6.19 boundary)",
                    count,
                    violations.first().cloned().unwrap_or_default(),
                ),
            );
        }
        Ok(&self.recheck.phase)
    }

    /// Clears a FAILED re-check with a RECORDED override (named actor +
    /// authority reference — the P9.34 `RollbackOverride` pattern). The
    /// clear is audited with the override reference; it is never silent.
    /// After the clear the re-check phase returns to
    /// [`RecheckPhase::Outstanding`] for the SAME attempt — the device must
    /// still produce a CLEAN report before further updates are authorized
    /// (the override re-opens the re-check; it never grants a pass).
    pub fn clear_conformance_failure(&mut self, override_record: &RecheckOverride) -> Result<(), InterlockError> {
        override_record.validate()?;
        let attempt_id = self
            .recheck
            .attempt_id
            .clone()
            .ok_or_else(|| InterlockError::InvalidInput {
                reason: "no update activation is recorded — there is no re-check failure to \
                         clear"
                    .to_string(),
            })?;
        let RecheckPhase::Failed { violations } = self.recheck.phase else {
            return Err(InterlockError::InvalidInput {
                reason: format!(
                    "the re-check for attempt '{attempt_id}' is {:?}, not Failed — nothing to \
                     clear",
                    self.recheck.phase
                ),
            });
        };
        self.recheck.phase = RecheckPhase::Outstanding;
        self.audit_event(
            "EDGE_UPDATE_CONFORMANCE_FAILURE_CLEARED",
            format!(
                "attempt '{attempt_id}': the FAILED re-check ({} violation(s)) is cleared by \
                 the recorded override actor='{}' under '{}' — the re-check re-opens as \
                 OUTSTANDING (the override never grants a pass; a clean report is still \
                 required)",
                violations, override_record.actor, override_record.authority_reference,
            ),
        );
        Ok(())
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one interlock audit event through the edge `AuditSink` (the
    /// dedicated-event-name-leads-detail pattern on the closed
    /// `EdgeAuditKind` vocabulary, as in P6.14/P9.34/P18.05; classified
    /// `EdgeAuditKind::UpdateLifecycle` — FRV1-R6-005).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::UpdateLifecycle,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "update-command-interlock audit write FAILED");
        }
    }

    fn audit_blocked(&self, refusal: &UpdateBlockRefusal) {
        self.audit_event(
            "EDGE_UPDATE_INTERLOCK_BLOCKED",
            format!(
                "refusal code={} :: {refusal}",
                refusal.name(),
            ),
        );
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::{CaseOutcome, CaseRecord, ConformanceCase, ConformanceReport};
    use crate::ports::EdgeError;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use std::sync::Mutex;

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    // --- A fixed clock (the injected-clock discipline) ------------------

    struct FixedClock(i64);
    impl Clock for FixedClock {
        fn now_event_time(&self) -> EventTime {
            EventTime::from_unix_millis(self.0).expect("valid instant")
        }
        fn now_ingest_time(&self) -> IngestTime {
            IngestTime::from_unix_millis(self.0).expect("valid instant")
        }
    }

    // --- A collecting audit sink (in-memory; the ports-crate pattern) ---

    #[derive(Default)]
    struct CollectingAuditSink {
        events: Mutex<Vec<String>>,
    }

    impl CollectingAuditSink {
        fn lines(&self) -> Vec<String> {
            self.events
                .lock()
                .expect("poisoned test sink mutex")
                .clone()
        }
    }

    impl AuditSink for CollectingAuditSink {
        fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError> {
            self.events
                .lock()
                .expect("poisoned test sink mutex")
                .push(event.detail);
            Ok(())
        }
    }

    // --- Fixtures ---------------------------------------------------------

    fn observation(command_id: &str, state_name: &str) -> DeviceCommandObservation {
        DeviceCommandObservation {
            command_id: command_id.to_string(),
            state_name: state_name.to_string(),
        }
    }

    fn interlock(
        source: Arc<InMemoryCommandStateSource>,
    ) -> (UpdateCommandInterlock, Arc<CollectingAuditSink>) {
        let sink = Arc::new(CollectingAuditSink::default());
        let interlock = UpdateCommandInterlock::new(
            source,
            sink.clone() as Arc<dyn AuditSink>,
            Arc::new(FixedClock(BASE_MILLIS)),
        );
        (interlock, sink)
    }

    fn clean_report() -> ConformanceReport {
        ConformanceReport {
            records: vec![CaseRecord {
                case: ConformanceCase::Valid,
                point: "ess0/ActivePower".to_string(),
                outcome: CaseOutcome::Accepted,
            }],
        }
    }

    fn violating_report() -> ConformanceReport {
        ConformanceReport {
            records: vec![CaseRecord {
                case: ConformanceCase::Valid,
                point: "ess0/ActivePower".to_string(),
                outcome: CaseOutcome::Rejected {
                    diagnostic: "the updated adapter dropped the mapping".to_string(),
                },
            }],
        }
    }

    // --- The blocking rule -------------------------------------------------

    #[test]
    fn commands_in_issued_or_tracking_block_the_update() {
        // The RECORDED citation surface: ISSUED and TRACKING block with the
        // recorded stable refusal name.
        for state in ["ISSUED", "TRACKING"] {
            let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
                "cmd-1", state,
            )]));
            let (interlock, sink) = interlock(source);
            let refusal = interlock
                .authorize_update_attempt()
                .expect_err("blocked");
            assert_eq!(
                refusal,
                UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 }
            );
            assert_eq!(refusal.name(), "UPDATE_BLOCKED_COMMANDS_IN_FLIGHT");
            let lines = sink.lines();
            assert!(
                lines
                    .iter()
                    .any(|l| l.contains("EDGE_UPDATE_INTERLOCK_BLOCKED")
                        && l.contains("UPDATE_BLOCKED_COMMANDS_IN_FLIGHT")),
                "the blocking refusal is audited: {lines:?}"
            );
        }
    }

    #[test]
    fn the_drain_gap_states_block_too() {
        // The recorded strengthening (record 28 §2): ACKNOWLEDGED,
        // FAILED, and EXPIRED are non-terminal in the frozen table
        // (FAILED/EXPIRED exit only via FALLBACK) and block as well —
        // strictly safer than the ISSUED/TRACKING citation.
        for state in ["ACKNOWLEDGED", "FAILED", "EXPIRED"] {
            let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
                "cmd-1", state,
            )]));
            let (interlock, _sink) = interlock(source);
            assert!(matches!(
                interlock.authorize_update_attempt(),
                Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
            ));
        }
        // A pre-issue state observed on the device is itself a defect; the
        // fail-closed reading blocks the update while it stands.
        for state in ["PROPOSED", "VALIDATED", "FEASIBLE", "APPROVED"] {
            let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
                "cmd-1", state,
            )]));
            let (interlock, _sink) = interlock(source);
            assert!(matches!(
                interlock.authorize_update_attempt(),
                Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
            ));
        }
    }

    #[test]
    fn terminal_states_do_not_block() {
        // Each frozen terminal state alone authorizes (drained device).
        for state in FROZEN_TERMINAL_COMMAND_STATE_NAMES {
            let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
                "cmd-1", state,
            )]));
            let (interlock, _sink) = interlock(source);
            let authorization = interlock
                .authorize_update_attempt()
                .expect("terminal states are drained");
            assert_eq!(authorization.in_flight_count, 0);
        }
    }

    #[test]
    fn unknown_device_state_fails_closed() {
        // A source that cannot establish the state is NEVER read as
        // "no commands in flight".
        let source = Arc::new(InMemoryCommandStateSource::default());
        source.set_unobservable("the command-path tracking map is unreachable".to_string());
        let (interlock, sink) = interlock(source);
        let refusal = interlock.authorize_update_attempt().expect_err("blocked");
        assert!(matches!(refusal, UpdateBlockRefusal::DeviceStateUnknown { .. }));
        assert_eq!(refusal.name(), "UPDATE_INTERLOCK_STATE_UNKNOWN");
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_INTERLOCK_BLOCKED")));

        // A state name outside the frozen vocabulary is refused, never
        // guessed at.
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
            "cmd-1",
            "SOME_VENDOR_STATE",
        )]));
        let (interlock, _sink) = interlock(source);
        assert!(matches!(
            interlock.authorize_update_attempt(),
            Err(UpdateBlockRefusal::DeviceStateUnknown { .. })
        ));
    }

    #[test]
    fn an_empty_device_state_authorizes_and_feeds_the_session_rule() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (interlock, _sink) = interlock(source);
        let authorization = interlock
            .authorize_update_attempt()
            .expect("no commands — authorized");
        assert_eq!(authorization.in_flight_count, 0);
        // The two-step composition: the interlock verdict feeds the
        // UNCHANGED wave-1 session rule.
        let mut session = crate::update_session::UpdateSessionStateMachine::new(
            "attempt-1",
            "device-1",
            "1.2.0",
            3,
            BASE_MILLIS,
        )
        .expect("session opens");
        let state = session
            .begin_download(authorization.in_flight_count, BASE_MILLIS + 1)
            .expect("the session rule accepts the interlock verdict");
        assert_eq!(state, crate::update_session::UpdateProgressState::Downloading);
    }

    #[test]
    fn the_interlock_is_at_least_as_strict_as_the_session_rule() {
        // With a command in TRACKING, the interlock refuses AND the
        // wave-1 session rule refuses on the same count — the two rules
        // cannot disagree in the unsafe direction.
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
            observation("cmd-1", "ISSUED"),
            observation("cmd-2", "TRACKING"),
        ]));
        let (interlock, _sink) = interlock(source);
        assert!(matches!(
            interlock.authorize_update_attempt(),
            Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 2 })
        ));
        let mut session = crate::update_session::UpdateSessionStateMachine::new(
            "attempt-1",
            "device-1",
            "1.2.0",
            3,
            BASE_MILLIS,
        )
        .expect("session opens");
        assert!(matches!(
            session.begin_download(2, BASE_MILLIS + 1),
            Err(crate::update_session::UpdateSessionError::CommandsInFlight {
                issued_or_tracking: 2
            })
        ));
    }

    // --- The drain procedure (report §5.6) ---------------------------------

    #[test]
    fn drain_starts_only_from_a_readable_state_and_stops_new_work() {
        let source = Arc::new(InMemoryCommandStateSource::default());
        source.set_unobservable("unreachable".to_string());
        let (mut interlock, _sink) = interlock(source);
        assert!(matches!(
            interlock.begin_drain(),
            Err(InterlockError::DeviceStateUnknown { .. })
        ));
        assert_eq!(interlock.drain_phase(), &DrainPhase::NotDraining);

        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
            observation("cmd-2", "TRACKING"),
            observation("cmd-1", "ISSUED"),
        ]));
        let (mut interlock, sink) = interlock(source);
        let phase = interlock.begin_drain().expect("drain starts").clone();
        match &phase {
            DrainPhase::Draining {
                commands_at_start, ..
            } => {
                // Sorted, deterministic (BTreeMap discipline).
                assert_eq!(commands_at_start, &["cmd-1".to_string(), "cmd-2".to_string()]);
            }
            other => panic!("expected Draining, got {other:?}"),
        }
        assert!(!interlock.allows_new_command_work());
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_DRAIN_STARTED")));

        // A second begin is refused.
        assert!(matches!(
            interlock.begin_drain(),
            Err(InterlockError::InvalidInput { .. })
        ));
    }

    #[test]
    fn drain_completes_only_when_all_commands_reach_terminal_states() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
            observation("cmd-1", "ISSUED"),
        ]));
        let (mut interlock, sink) = interlock(source);
        interlock.begin_drain().expect("drain starts");

        // Still ISSUED: the drain continues.
        let phase = interlock.poll_drain().expect("observed").clone();
        assert_eq!(phase, DrainPhase::Draining {
            started_at_unix_millis: BASE_MILLIS,
            commands_at_start: vec!["cmd-1".to_string()],
        });
        assert!(!interlock.allows_new_command_work());

        // The device state becomes UNOBSERVABLE: the drain cannot complete
        // (fail-closed), the phase is retained, and the refusal is audited.
        source.set_unobservable("tracking map lost".to_string());
        assert!(matches!(
            interlock.poll_drain(),
            Err(InterlockError::DeviceStateUnknown { .. })
        ));
        assert!(matches!(interlock.drain_phase(), DrainPhase::Draining { .. }));
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_DRAIN_OBSERVED_IN_FLIGHT")));

        // The operator's lifecycle action drains the command
        // (ISSUED → CANCELLED — a frozen edge, performed by the operator,
        // never by the update path).
        source.set_observations(vec![observation("cmd-1", "CANCELLED")]);
        let phase = interlock.poll_drain().expect("observed").clone();
        assert!(matches!(phase, DrainPhase::Drained { .. }));
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_DRAIN_COMPLETE")));
        // The §5.6 "stop accepting new work" read lifts with the drain.
        assert!(interlock.allows_new_command_work());
    }

    #[test]
    fn a_drained_device_authorizes_the_update() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
            observation("cmd-1", "TRACKING"),
        ]));
        let (mut interlock, _sink) = interlock(source);
        assert!(interlock.authorize_update_attempt().is_err());
        interlock.begin_drain().expect("drain starts");
        source.set_observations(vec![observation("cmd-1", "COMPLETED")]);
        interlock.poll_drain().expect("drained");
        // The state is re-read AT AUTHORIZE TIME: authorized, and the
        // session rule accepts the verdict.
        let authorization = interlock.authorize_update_attempt().expect("drained");
        assert_eq!(authorization.in_flight_count, 0);
    }

    #[test]
    fn the_interlock_never_transitions_a_command_state() {
        // The update path is OBSERVER-only toward the command lifecycle:
        // begin/poll drain never change what the command path recorded.
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
            observation("cmd-1", "ISSUED"),
        ]));
        let (mut interlock, _sink) = interlock(source);
        interlock.begin_drain().expect("drain starts");
        interlock.poll_drain().expect_err("still ISSUED");
        interlock.poll_drain().expect_err("still ISSUED");
        let observed = source.device_command_states().expect("readable");
        assert_eq!(
            observed,
            vec![observation("cmd-1", "ISSUED")],
            "the interlock must not have written any command state"
        );
    }

    // --- The post-update conformance re-check (P6.13 interplay) ------------

    #[test]
    fn activation_requires_a_conformance_recheck() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        let refusal = interlock.authorize_update_attempt().expect_err("blocked");
        assert!(matches!(
            refusal,
            UpdateBlockRefusal::ConformanceRecheckOutstanding { attempt_id } if attempt_id == "attempt-1"
        ));
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_ACTIVATED_RECHECK_OUTSTANDING")));
    }

    #[test]
    fn a_clean_conformance_report_clears_the_recheck() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        let phase = interlock
            .record_conformance_report(&clean_report())
            .expect("recorded")
            .clone();
        assert_eq!(phase, RecheckPhase::Satisfied);
        let authorization = interlock.authorize_update_attempt().expect("re-checked");
        assert_eq!(authorization.in_flight_count, 0);
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_CONFORMANCE_RECHECK_PASSED")));
    }

    #[test]
    fn a_violating_report_fails_the_recheck_and_stays_refused() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        let phase = interlock
            .record_conformance_report(&violating_report())
            .expect("recorded")
            .clone();
        assert!(matches!(phase, RecheckPhase::Failed { violations: 1 }));
        // Further authorization is refused with the FAILED variant.
        let refusal = interlock.authorize_update_attempt().expect_err("blocked");
        assert!(matches!(
            refusal,
            UpdateBlockRefusal::ConformanceRecheckFailed { attempt_id, violations: 1 }
            if attempt_id == "attempt-1"
        ));
        // Re-running the harness to fish for a pass is refused — a FAILED
        // re-check is terminal until a recorded override clears it.
        assert!(matches!(
            interlock.record_conformance_report(&clean_report()),
            Err(InterlockError::InvalidInput { .. })
        ));
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_CONFORMANCE_RECHECK_FAILED")));
    }

    #[test]
    fn clearing_a_conformance_failure_requires_a_recorded_override() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        interlock
            .record_conformance_report(&violating_report())
            .expect("failed");

        // An override without a named actor/authority is not a recorded
        // override (the P9.34 RollbackOverride pattern).
        assert!(matches!(
            interlock.clear_conformance_failure(&RecheckOverride {
                actor: String::new(),
                authority_reference: "ticket-1".to_string(),
            }),
            Err(InterlockError::InvalidInput { .. })
        ));
        assert!(matches!(
            interlock.clear_conformance_failure(&RecheckOverride {
                actor: "J. Doe (ops)".to_string(),
                authority_reference: String::new(),
            }),
            Err(InterlockError::InvalidInput { .. })
        ));

        // A recorded override re-OPENS the re-check as OUTSTANDING — it
        // never grants a pass; a clean report is still required.
        interlock
            .clear_conformance_failure(&RecheckOverride {
                actor: "J. Doe (ops)".to_string(),
                authority_reference: "ticket-1".to_string(),
            })
            .expect("cleared");
        assert_eq!(interlock.recheck_phase(), &RecheckPhase::Outstanding);
        assert!(interlock.authorize_update_attempt().is_err());
        interlock
            .record_conformance_report(&clean_report())
            .expect("recorded");
        assert!(interlock.authorize_update_attempt().is_ok());
        assert!(sink
            .lines()
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_CONFORMANCE_FAILURE_CLEARED")
                && l.contains("ticket-1")));
    }

    #[test]
    fn each_new_activation_requires_a_fresh_recheck() {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, _sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        interlock
            .record_conformance_report(&clean_report())
            .expect("satisfied");
        assert_eq!(interlock.recheck_phase(), &RecheckPhase::Satisfied);
        // A NEW activation supersedes the previous re-check: fresh
        // re-check per update.
        interlock.record_update_activated("attempt-2").expect("recorded");
        assert_eq!(interlock.recheck_phase(), &RecheckPhase::Outstanding);
        assert_eq!(interlock.recheck_attempt_id(), Some("attempt-2"));
        let refusal = interlock.authorize_update_attempt().expect_err("blocked");
        assert!(matches!(
            refusal,
            UpdateBlockRefusal::ConformanceRecheckOutstanding { attempt_id } if attempt_id == "attempt-2"
        ));
    }

    #[test]
    fn the_recheck_consumes_the_real_p6_13_harness() {
        // The interplay is with the REAL conformance module: run the
        // actual §19.2(6) harness against the simulated adapter and feed
        // the report to the interlock (consumed, never reimplemented).
        use crate::adapters::acl::{AntiCorruptionLayer, NativeSign, PointMapping};
        use crate::adapters::port::PointKey;
        use crate::adapters::simulated::{SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture};
        use crate::conformance::{ConformanceHarness, ConformanceLimits};
        use flexcore_contracts::units::ReactiveSignConvention;

        let acl = AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
                native_unit: crate::adapters::acl::NativeUnit::Kw,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:simulated-test",
            vec![],
        )
        .expect("valid mapping table");
        let fixture = SimulatedFixture {
            points: vec![SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: crate::adapters::acl::NativeUnit::Kw,
                values_per_tick: vec![1_500.0],
                quality_per_tick: vec!["GOOD".to_string()],
            }],
            start_millis: BASE_MILLIS,
            step_millis: 1_000,
            reactive_source_convention: ReactiveSignConvention::InjectionPositive,
            cadence_millis: 5_000,
        };
        let harness = ConformanceHarness::new(
            acl,
            BASE_MILLIS + 5_000,
            ConformanceLimits {
                max_clock_skew_past_millis: 3_600_000,
            },
        )
        .expect("valid limits");
        let mut adapter = SimulatedEmsSource::new(fixture);
        let report = harness
            .run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
        assert!(report.is_clean(), "{}", report.render());

        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
        let (mut interlock, _sink) = interlock(source);
        interlock.record_update_activated("attempt-1").expect("recorded");
        interlock
            .record_conformance_report(&report)
            .expect("recorded");
        assert_eq!(interlock.recheck_phase(), &RecheckPhase::Satisfied);
        assert!(interlock.authorize_update_attempt().is_ok());
    }

    // --- Statement pins ------------------------------------------------------

    #[test]
    fn the_statement_pins_the_honesty_bounds() {
        assert!(INTERLOCK_STATEMENT.contains("strict superset"));
        assert!(INTERLOCK_STATEMENT.contains("No command exists"));
        assert!(INTERLOCK_STATEMENT.contains("no fleet exists"));
        assert!(INTERLOCK_STATEMENT.contains("never protocol conformance"));
        // The frozen vocabulary is CITED, never forked: the terminal set is
        // exactly the frozen five and every name is inside the frozen 14.
        assert_eq!(
            FROZEN_TERMINAL_COMMAND_STATE_NAMES,
            ["COMPLETED", "REJECTED", "CANCELLED", "SUPERSEDED", "FALLBACK"]
        );
        for terminal in FROZEN_TERMINAL_COMMAND_STATE_NAMES {
            assert!(
                FROZEN_COMMAND_STATE_NAMES.contains(&terminal),
                "terminal state {terminal} must be a frozen §14.1 state"
            );
        }
    }
}
