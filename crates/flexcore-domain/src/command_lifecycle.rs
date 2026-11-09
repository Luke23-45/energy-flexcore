// P4.10 — Command lifecycle engine over the frozen §14.1 edges (report
//         §14.1–§14.2; ledger P4.10; P0.11 freeze).
//
// The transition TABLE lives in `command_state.rs` (P0.11) and remains the
// single source of truth: this engine never re-encodes an edge. Every guard
// decision goes through `command_state::try_transition`, so an edge that is
// not in the frozen 27-edge table is impossible here, and a missing edge
// would be a BUG against the oracle — never an extension. What this module
// ADDS over the bare table is the operational discipline the report puts on
// every transition (§14.1; §6.4 state-machine row):
//
// - **Gate separation is structural.** Validation, feasibility, and
//   authorization are three DISTINCT gates (§14.1 closing statement), each
//   with its own transition function and its own evidence reference:
//   `record_validation` applies only from PROPOSED (→ VALIDATED or
//   REJECTED), `record_feasibility` only from VALIDATED (→ FEASIBLE or
//   REJECTED), `record_authorization` only from FEASIBLE (→ APPROVED or
//   REJECTED). APPROVED without FEASIBLE evidence is impossible by
//   construction: FEASIBLE is reachable ONLY through
//   `record_feasibility` with a passed outcome, and APPROVED is reachable
//   ONLY from FEASIBLE. REJECTED carries WHICH gate rejected (§14.3: the
//   three rejection points of the frozen table).
// - **Actor + timestamp on every transition.** Timestamps come from a
//   caller-supplied controlled clock ([`LifecycleClock`]) — never
//   wall-clock. The engine refuses a transition whose timestamp moves
//   backwards relative to the previous journal entry (controlled-clock
//   discipline, ledger P4.15 risk note on clock drift, §17.4).
// - **Reason where the report requires one.** REJECTED carries the failing
//   check; FAILED carries the frozen-table trigger (NACK / timeout /
//   transport failure; no expected response; tracking deviation / timeout);
//   EXPIRED carries the milestone not reached (P4.15 step 1);
//   CANCELLED carries the operator/policy reason (§14.2); FALLBACK carries
//   the policy reference (P4.17 owns the policies themselves; the STATE and
//   transition are implemented here).
// - **Acknowledgement is not physical delivery** (§14.1): `record_
//   acknowledgement` records application-layer acceptance only — ISSUED,
//   ACKNOWLEDGED, and TRACKING measure progressively more evidence, and
//   none of them is COMPLETED until the defined delivery criterion is
//   satisfied (§14.2).
// - **SUPERSEDED is not CANCELLED** (§14.2, P0.11 §3.1): `record_
//   superseded_by` is the replacement-driven invalidation (bidirectional
//   supersedes/superseded-by links, mirrored at the persistence layer like
//   the P3.22 revision chain) and `record_cancellation` is the explicit
//   operator/policy act. They produce distinct journal entries and are
//   never merged.
// - **Expiry is STATE SUPPORT only.** The EXPIRED edge (frozen transitions
//   10, 15, 20, 24), the validity comparison against the controlled clock
//   ([`CommandLifecycle::validity_has_lapsed`]), and the milestone-not-
//   reached record are implemented here; the expiry sweep/job/timer is
//   P4.15's.
// - **Schedule-revision pre-wiring (P4.12).** Commands reference schedule
//   ID + revision (P3.22). The identity-level gate check itself — a
//   transition whose command references a SUPERSEDED (or unknown) schedule
//   revision is rejected — lives on the persistence port
//   (`flexcore_storage::command_store::CommandRecordStore::
//   ensure_schedule_revision_usable`, which calls the P3.22
//   `is_revision_superseded` surface). NO feasibility gate is built here —
//   the §12.5 network pipeline is P4.12's.
//
// Control status: the command lifecycle is a protected platform invariant
// owned by the platform kernel (§0.2, §5.7); plugins may propose and
// request validation but can never authorize, issue, or select the
// control-authority mode. The entire 0.x line is simulation-only
// (§21.4): nothing here performs a device write, and an acknowledgement
// recorded through this engine is acceptance, never physical delivery.
// FlexCore performs no primary frequency response, synthetic inertia,
// relay protection, inverter inner-loop control, or sub-cycle control
// (§14.5).

use flexcore_contracts::time_model::ValidUntil;

use crate::command_state::{CommandState, TransitionError, is_terminal, try_transition};
use crate::ids::CommandId;

// --- Controlled clock --------------------------------------------------------

/// The controlled clock the engine timestamps transitions with.
///
/// This is the domain-side port for the report's time discipline (§10.3,
/// P0.09): production adapters implement it over a synchronized source (the
/// flexcore-ports `Clock` port is the infrastructure-side form; this crate
/// depends on flexcore-contracts only, so the trait is declared here and
/// bridged by the composition root). Tests MUST use a fixed/advanceable
/// implementation — wall-clock-dependent lifecycle tests are a recorded
/// defect (ledger P4.15 verification method: "controlled-clock tests are
/// independent of wall-clock time").
pub trait LifecycleClock {
    /// The current instant as UTC unix milliseconds.
    fn now_unix_millis(&self) -> i64;
}

// --- Value types --------------------------------------------------------------

/// The identity that performed (or on whose behalf the platform performed) a
/// transition. Required on every transition (§14.3 reason/action and actor
/// or service identity; §6.4: every transition has an audit event).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    identity: String,
}

impl Actor {
    /// Builds an actor identity; empty or whitespace-only identities are
    /// rejected.
    pub fn new(identity: impl Into<String>) -> Result<Self, LifecycleError> {
        let identity = identity.into();
        if identity.trim().is_empty() {
            return Err(LifecycleError::EmptyField { field: "actor" });
        }
        Ok(Self { identity })
    }

    /// The actor identity string.
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

/// A reference to the evidence record a transition rests on (§14.3
/// evidence-reference discipline; §6.4 pipeline row: "each stage records
/// evidence"). Non-empty by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReference {
    reference: String,
}

impl EvidenceReference {
    /// Builds an evidence reference; empty or whitespace-only references are
    /// rejected.
    pub fn new(reference: impl Into<String>) -> Result<Self, LifecycleError> {
        let reference = reference.into();
        if reference.trim().is_empty() {
            return Err(LifecycleError::EmptyField {
                field: "evidence_reference",
            });
        }
        Ok(Self { reference })
    }

    /// The reference string.
    pub fn as_str(&self) -> &str {
        &self.reference
    }
}

/// The three gates of §14.1, as distinct vocabulary. Stored on the journal
/// entry of every gate transition and every gate rejection, so an audit
/// record always names WHICH gate produced it (§14.1: "validation,
/// feasibility, and authorization are distinct").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gate {
    /// The validation gate (→ VALIDATED): schema, freshness, asset
    /// capability, ramp, SoC, and command-policy checks (§14.2).
    Validation,
    /// The feasibility gate (→ FEASIBLE): the relevant network model check
    /// within declared scope and tolerances (§14.2; §12.5). Not built here —
    /// the pipeline is P4.12's; the STATE and transition are.
    Feasibility,
    /// The authorization gate (→ APPROVED): required human/policy
    /// authorization (§14.2; §17.3). Network feasibility is NOT operator
    /// authorization and is never derived from a passing power-flow run.
    Authorization,
}

impl Gate {
    /// The stable text form stored in audit/journal records.
    pub fn as_str(self) -> &'static str {
        match self {
            Gate::Validation => "validation",
            Gate::Feasibility => "feasibility",
            Gate::Authorization => "authorization",
        }
    }
}

/// The outcome of the validation or feasibility gate (§6.4: per-check
/// results with evidence, not a single boolean). A pass MUST carry its
/// evidence reference; a failure MUST additionally name its failing check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    passed: bool,
    evidence: EvidenceReference,
    failing_check: Option<String>,
}

impl GateOutcome {
    /// A passed gate outcome with its evidence reference.
    pub fn pass(evidence: EvidenceReference) -> Self {
        Self {
            passed: true,
            evidence,
            failing_check: None,
        }
    }

    /// A failed gate outcome with its evidence reference and the named
    /// failing check (empty check names are rejected).
    pub fn fail(
        evidence: EvidenceReference,
        failing_check: impl Into<String>,
    ) -> Result<Self, LifecycleError> {
        let failing_check = failing_check.into();
        if failing_check.trim().is_empty() {
            return Err(LifecycleError::EmptyField {
                field: "failing_check",
            });
        }
        Ok(Self {
            passed: false,
            evidence,
            failing_check: Some(failing_check),
        })
    }

    /// Whether the gate passed.
    pub fn passed(&self) -> bool {
        self.passed
    }

    /// The evidence reference.
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }

    /// The failing check (present exactly when the gate failed).
    pub fn failing_check(&self) -> Option<&str> {
        self.failing_check.as_deref()
    }
}

/// The outcome of the authorization gate (§14.3 field 14: authorization
/// decision reference, including approved scope, limits, and expiry; the
/// full decision-record schema is P4.13's — the state transition and its
/// evidence/actor recording are implemented here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationOutcome {
    approved: bool,
    decision: EvidenceReference,
    actor: Actor,
    denial_reason: Option<String>,
}

impl AuthorizationOutcome {
    /// An approved authorization: the decision reference and the authorized
    /// actor are required.
    pub fn approve(decision: EvidenceReference, actor: Actor) -> Self {
        Self {
            approved: true,
            decision,
            actor,
            denial_reason: None,
        }
    }

    /// A denied authorization: the denial reason is required (§14.1 edge 7:
    /// authorization denied; the audit record must say why).
    pub fn deny(
        decision: EvidenceReference,
        actor: Actor,
        denial_reason: impl Into<String>,
    ) -> Result<Self, LifecycleError> {
        let denial_reason = denial_reason.into();
        if denial_reason.trim().is_empty() {
            return Err(LifecycleError::MissingFailingCheck {
                gate: Gate::Authorization,
            });
        }
        Ok(Self {
            approved: false,
            decision,
            actor,
            denial_reason: Some(denial_reason),
        })
    }

    /// Whether authorization was granted.
    pub fn approved(&self) -> bool {
        self.approved
    }

    /// The authorization decision reference.
    pub fn decision(&self) -> &EvidenceReference {
        &self.decision
    }

    /// The authorizing (or denying) actor.
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The denial reason (present exactly when denied).
    pub fn denial_reason(&self) -> Option<&str> {
        self.denial_reason.as_deref()
    }
}

/// The frozen-table failure triggers, per state (§14.1 edges 14, 19, 23).
/// Each variant carries the frozen annotation as its reason text; the engine
/// refuses a reason that does not belong to the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailureReason {
    /// ISSUED → FAILED: NACK from the gateway/adapter.
    Nack,
    /// ISSUED → FAILED: acknowledgement timeout.
    TransportTimeout,
    /// ISSUED → FAILED: transport failure.
    TransportFailure,
    /// ACKNOWLEDGED → FAILED: no expected response.
    NoExpectedResponse,
    /// TRACKING → FAILED: tracking deviation.
    TrackingDeviation,
    /// TRACKING → FAILED: tracking timeout.
    TrackingTimeout,
}

impl FailureReason {
    /// The frozen-table trigger text recorded as the journal reason.
    pub fn reason_text(self) -> &'static str {
        match self {
            FailureReason::Nack => "NACK from gateway/adapter",
            FailureReason::TransportTimeout => "acknowledgement timeout",
            FailureReason::TransportFailure => "transport failure",
            FailureReason::NoExpectedResponse => "no expected response",
            FailureReason::TrackingDeviation => "tracking deviation",
            FailureReason::TrackingTimeout => "tracking timeout",
        }
    }

    /// Whether this failure trigger is the one the frozen table annotates
    /// for `state` (ISSUED: NACK / timeout / transport failure; ACKNOWLEDGED:
    /// no expected response; TRACKING: tracking deviation / timeout).
    fn allowed_from(self, state: CommandState) -> bool {
        match (state, self) {
            (CommandState::Issued, FailureReason::Nack)
            | (CommandState::Issued, FailureReason::TransportTimeout)
            | (CommandState::Issued, FailureReason::TransportFailure)
            | (CommandState::Acknowledged, FailureReason::NoExpectedResponse)
            | (CommandState::Tracking, FailureReason::TrackingDeviation)
            | (CommandState::Tracking, FailureReason::TrackingTimeout) => true,
            _ => false,
        }
    }
}

/// The milestone that was NOT reached when the validity window ended
/// (§14.2 EXPIRED: "validity window ended before execution, application-
/// level acceptance, or completion"; P4.15 step 1: "record the expiry time
/// and the milestone not reached").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpiryMilestone {
    /// APPROVED → EXPIRED: validity ended before issue (frozen edge 10).
    BeforeIssue,
    /// ISSUED → EXPIRED: validity ended before acknowledgement (edge 15).
    BeforeAcknowledgement,
    /// ACKNOWLEDGED → EXPIRED: validity ended before tracking completes
    /// (edge 20).
    BeforeTrackingCompletion,
    /// TRACKING → EXPIRED: validity ended before completion (edge 24).
    BeforeCompletion,
}

impl ExpiryMilestone {
    /// The milestone text recorded as the journal reason.
    pub fn reason_text(self) -> &'static str {
        match self {
            ExpiryMilestone::BeforeIssue => {
                "validity ended before issue (milestone not reached: ISSUED)"
            }
            ExpiryMilestone::BeforeAcknowledgement => {
                "validity ended before acknowledgement (milestone not reached: ACKNOWLEDGED)"
            }
            ExpiryMilestone::BeforeTrackingCompletion => {
                "validity ended before tracking completes (milestone not reached: TRACKING)"
            }
            ExpiryMilestone::BeforeCompletion => {
                "validity ended before completion (milestone not reached: COMPLETED)"
            }
        }
    }

    /// The milestone the frozen table annotates for `state`.
    fn expected_for(state: CommandState) -> Option<Self> {
        match state {
            CommandState::Approved => Some(ExpiryMilestone::BeforeIssue),
            CommandState::Issued => Some(ExpiryMilestone::BeforeAcknowledgement),
            CommandState::Acknowledged => Some(ExpiryMilestone::BeforeTrackingCompletion),
            CommandState::Tracking => Some(ExpiryMilestone::BeforeCompletion),
            _ => None,
        }
    }
}

/// One journalled transition: the audit unit of the lifecycle (§6.4: every
/// transition has explicit preconditions and an audit event). The
/// persistence layer journals each of these as an immutable row (from, to,
/// actor, timestamp, reason, evidence reference — ledger P4.10 step 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTransition {
    /// The command the transition belongs to.
    pub command_id: CommandId,
    /// The state before the transition.
    pub from: CommandState,
    /// The state after the transition.
    pub to: CommandState,
    /// The actor identity recorded for the transition.
    pub actor: Actor,
    /// The transition timestamp (UTC unix millis, from the controlled clock).
    pub at_unix_millis: i64,
    /// The gate that produced the transition — present exactly for the
    /// three gate transitions (→ VALIDATED / FEASIBLE / APPROVED) and the
    /// three gate rejections (→ REJECTED).
    pub gate: Option<Gate>,
    /// The reason, where the report requires one (see the module header).
    pub reason: Option<String>,
    /// The evidence reference, where the transition rests on evidence.
    pub evidence: Option<EvidenceReference>,
}

// --- Errors -------------------------------------------------------------------

/// The named errors of the lifecycle engine. Illegal transitions are
/// REJECTED with a named error — never silently ignored, never
/// auto-corrected (ledger P4.10 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// The requested edge is not in the frozen §14.1 table (delegates to
    /// [`command_state::try_transition`]; no edge outside the table is
    /// permitted, including shortcuts and exits from terminal states).
    NotPermitted {
        /// The state the command was in.
        from: CommandState,
        /// The state the transition would have produced.
        to: CommandState,
    },
    /// The operation does not apply in the command's current state — the
    /// structural form of the gate separation (e.g. the authorization gate
    /// called on a VALIDATED command).
    WrongState {
        /// The state the command is in.
        current: CommandState,
        /// Which operation was attempted.
        operation: &'static str,
    },
    /// A required field (actor, reason, reference) was empty.
    EmptyField {
        /// Which field was empty.
        field: &'static str,
    },
    /// A failed gate outcome (or denial) did not name its failing check /
    /// denial reason.
    MissingFailingCheck {
        /// Which gate's outcome was incomplete.
        gate: Gate,
    },
    /// The failure trigger does not match the one the frozen table
    /// annotates for the current state.
    InvalidFailureReason {
        /// The state the command is in.
        current: CommandState,
        /// The trigger text that was refused.
        reason: &'static str,
    },
    /// The expiry milestone does not match the current state.
    InvalidExpiryMilestone {
        /// The state the command is in.
        current: CommandState,
        /// The milestone that was supplied.
        milestone: &'static str,
    },
    /// EXPIRED was requested before the validity window actually ended
    /// (controlled clock). Expiry never fires early.
    ValidityNotLapsed {
        /// The controlled clock's current time.
        now_unix_millis: i64,
        /// The validity window's end.
        valid_until_millis: i64,
    },
    /// A command cannot supersede itself.
    SelfSupersede {
        /// The command ID that named itself as its own replacement.
        command_id: CommandId,
    },
    /// The transition timestamp moved backwards relative to the previous
    /// journal entry (controlled-clock discipline; §17.4 clock-drift threat).
    TimestampRegression {
        /// The previous journal entry's timestamp.
        previous_unix_millis: i64,
        /// The proposed (earlier) timestamp.
        proposed_unix_millis: i64,
    },
    /// The validity bound could not be represented as unix milliseconds.
    InvalidValidityBound,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleError::NotPermitted { from, to } => write!(
                f,
                "transition {} -> {} is not in the frozen command state machine",
                from.name(),
                to.name()
            ),
            LifecycleError::WrongState { current, operation } => write!(
                f,
                "operation `{operation}` does not apply to a command in state {}",
                current.name()
            ),
            LifecycleError::EmptyField { field } => {
                write!(f, "field `{field}` must not be empty")
            }
            LifecycleError::MissingFailingCheck { gate } => {
                write!(f, "a failed {} outcome must name its failing check", gate.as_str())
            }
            LifecycleError::InvalidFailureReason { current, reason } => write!(
                f,
                "failure trigger '{reason}' does not apply to a command in state {}",
                current.name()
            ),
            LifecycleError::InvalidExpiryMilestone { current, milestone } => write!(
                f,
                "expiry milestone does not apply to a command in state {}: {milestone}",
                current.name()
            ),
            LifecycleError::ValidityNotLapsed {
                now_unix_millis,
                valid_until_millis,
            } => write!(
                f,
                "the validity window has not ended (now {now_unix_millis} < valid-until \
                 {valid_until_millis}); EXPIRED is never recorded early"
            ),
            LifecycleError::SelfSupersede { command_id } => {
                write!(f, "command {command_id} cannot supersede itself")
            }
            LifecycleError::TimestampRegression {
                previous_unix_millis,
                proposed_unix_millis,
            } => write!(
                f,
                "the transition timestamp {proposed_unix_millis} is earlier than the \
                 previous journal entry {previous_unix_millis}"
            ),
            LifecycleError::InvalidValidityBound => {
                write!(f, "the validity bound is out of range for unix milliseconds")
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

impl From<TransitionError> for LifecycleError {
    fn from(err: TransitionError) -> Self {
        match err {
            TransitionError::NotPermitted { from, to } => LifecycleError::NotPermitted { from, to },
        }
    }
}

// --- The lifecycle aggregate ---------------------------------------------------

/// The protected lifecycle of ONE dispatch command (report §14.1): the
/// command's current state, its §14.2 validity bound (for the EXPIRED state
/// support), its bidirectional supersede link, and the full transition
/// journal. Constructed exclusively through [`CommandLifecycle::propose`]
/// (the freeze's initial marker: every command enters as PROPOSED).
///
/// The engine holds NO command payload — the §14.3 command record is
/// `DispatchCommand` (P1.06); this type owns only the lifecycle discipline.
#[derive(Debug, Clone)]
pub struct CommandLifecycle {
    command_id: CommandId,
    valid_until_millis: i64,
    state: CommandState,
    journal: Vec<CommandTransition>,
    superseded_by: Option<CommandId>,
    rejected_by: Option<Gate>,
}

impl CommandLifecycle {
    /// Enters the command at PROPOSED (the freeze's initial marker
    /// `[*] → PROPOSED`: every command enters as PROPOSED — this is not a
    /// transition, so the journal starts empty and no timestamp is taken).
    /// `valid_until` is the §14.3 valid-until bound used by the EXPIRED
    /// state support.
    pub fn propose(command_id: CommandId, valid_until: ValidUntil) -> Result<Self, LifecycleError> {
        let valid_until_millis =
            i64::try_from(valid_until.unix_millis()).map_err(|_| LifecycleError::InvalidValidityBound)?;
        Ok(Self {
            command_id,
            valid_until_millis,
            state: CommandState::Proposed,
            journal: Vec::new(),
            superseded_by: None,
            rejected_by: None,
        })
    }

    /// The command identifier.
    pub fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// The current state.
    pub fn state(&self) -> CommandState {
        self.state
    }

    /// The valid-until bound (UTC unix millis).
    pub fn valid_until_millis(&self) -> i64 {
        self.valid_until_millis
    }

    /// The full transition journal, in order (the audit view; the
    /// persistence layer journals each entry as an immutable row).
    pub fn journal(&self) -> &[CommandTransition] {
        &self.journal
    }

    /// The superseding command, when this command was SUPERSEDED — the
    /// backward direction of the bidirectional supersedes/superseded-by
    /// pair (§14.3 field 16; the forward direction — what this command
    /// supersedes — is declared on the replacement's `DispatchCommand`).
    pub fn superseded_by(&self) -> Option<CommandId> {
        self.superseded_by
    }

    /// The gate that rejected the command, when it is REJECTED (§14.1: the
    /// three rejection points are the three gates).
    pub fn rejected_by(&self) -> Option<Gate> {
        self.rejected_by
    }

    /// Whether the current state is terminal in the frozen table.
    pub fn is_terminal(&self) -> bool {
        is_terminal(self.state)
    }

    /// The states the command may legally move to from its CURRENT state,
    /// derived from the frozen table (`try_transition` over all 14 states —
    /// the table is the single source of truth; nothing is added here).
    pub fn permitted_transitions(&self) -> Vec<CommandState> {
        CommandState::ALL
            .iter()
            .copied()
            .filter(|&to| try_transition(self.state, to).is_ok())
            .collect()
    }

    /// EXPIRED state support: whether the validity window has ended at the
    /// controlled clock's current time (§10.3 end-exclusive interval
    /// semantics: the window is over once `now >= valid_until`). The sweep
    /// job itself is P4.15's; this is the comparison the sweep and every
    /// transition-attempt check use.
    pub fn validity_has_lapsed(&self, clock: &dyn LifecycleClock) -> bool {
        clock.now_unix_millis() >= self.valid_until_millis
    }

    /// Records one transition through the frozen table. The guard order is:
    /// (1) the controlled clock is read; (2) timestamp monotonicity is
    /// enforced; (3) the edge is checked against the frozen table via
    /// `try_transition`; (4) the journal entry is appended.
    fn commit_transition(
        &mut self,
        to: CommandState,
        actor: Actor,
        clock: &dyn LifecycleClock,
        gate: Option<Gate>,
        reason: Option<String>,
        evidence: Option<EvidenceReference>,
    ) -> Result<CommandTransition, LifecycleError> {
        let at_unix_millis = clock.now_unix_millis();
        if let Some(last) = self.journal.last() {
            if at_unix_millis < last.at_unix_millis {
                return Err(LifecycleError::TimestampRegression {
                    previous_unix_millis: last.at_unix_millis,
                    proposed_unix_millis: at_unix_millis,
                });
            }
        }
        let from = self.state;
        try_transition(from, to)?; // the frozen table is the only guard of record
        let transition = CommandTransition {
            command_id: self.command_id,
            from,
            to,
            actor,
            at_unix_millis,
            gate,
            reason,
            evidence,
        };
        self.journal.push(transition.clone());
        self.state = to;
        Ok(transition)
    }

    // --- The three gates (structurally separate; see the module header) ----

    /// The VALIDATION gate (§14.1 edge 1/2): applies ONLY from PROPOSED.
    /// Pass → VALIDATED with the validation evidence reference; fail →
    /// REJECTED with the failing check named and the gate recorded.
    pub fn record_validation(
        &mut self,
        outcome: GateOutcome,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if self.state != CommandState::Proposed {
            return Err(LifecycleError::WrongState {
                current: self.state,
                operation: "record_validation",
            });
        }
        if outcome.passed {
            self.commit_transition(
                CommandState::Validated,
                actor,
                clock,
                Some(Gate::Validation),
                None,
                Some(outcome.evidence),
            )
        } else {
            let failing_check = outcome
                .failing_check
                .ok_or(LifecycleError::MissingFailingCheck { gate: Gate::Validation })?;
            let recorded = self.commit_transition(
                CommandState::Rejected,
                actor,
                clock,
                Some(Gate::Validation),
                Some(failing_check),
                Some(outcome.evidence),
            )?;
            self.rejected_by = Some(Gate::Validation);
            Ok(recorded)
        }
    }

    /// The FEASIBILITY gate (§14.1 edge 3/4): applies ONLY from VALIDATED.
    /// Pass → FEASIBLE with the feasibility evidence reference (distinct
    /// from the validation evidence, §14.1); fail → REJECTED (capability /
    /// network failure) with the failing check named. The §12.5 network
    /// pipeline that PRODUCES the evidence is P4.12's — this is the state
    /// transition.
    pub fn record_feasibility(
        &mut self,
        outcome: GateOutcome,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if self.state != CommandState::Validated {
            return Err(LifecycleError::WrongState {
                current: self.state,
                operation: "record_feasibility",
            });
        }
        if outcome.passed {
            self.commit_transition(
                CommandState::Feasible,
                actor,
                clock,
                Some(Gate::Feasibility),
                None,
                Some(outcome.evidence),
            )
        } else {
            let failing_check = outcome
                .failing_check
                .ok_or(LifecycleError::MissingFailingCheck { gate: Gate::Feasibility })?;
            let recorded = self.commit_transition(
                CommandState::Rejected,
                actor,
                clock,
                Some(Gate::Feasibility),
                Some(failing_check),
                Some(outcome.evidence),
            )?;
            self.rejected_by = Some(Gate::Feasibility);
            Ok(recorded)
        }
    }

    /// The AUTHORIZATION gate (§14.1 edge 6/7): applies ONLY from FEASIBLE —
    /// and FEASIBLE is reachable only through [`Self::record_feasibility`]
    /// with a passed outcome, so APPROVED without FEASIBLE evidence is
    /// impossible by construction. Pass → APPROVED with the authorization
    /// decision reference; denial → REJECTED with the denial reason and
    /// actor.
    pub fn record_authorization(
        &mut self,
        outcome: AuthorizationOutcome,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if self.state != CommandState::Feasible {
            return Err(LifecycleError::WrongState {
                current: self.state,
                operation: "record_authorization",
            });
        }
        if outcome.approved {
            self.commit_transition(
                CommandState::Approved,
                outcome.actor,
                clock,
                Some(Gate::Authorization),
                None,
                Some(outcome.decision),
            )
        } else {
            let denial_reason = outcome
                .denial_reason
                .ok_or(LifecycleError::MissingFailingCheck { gate: Gate::Authorization })?;
            let recorded = self.commit_transition(
                CommandState::Rejected,
                outcome.actor,
                clock,
                Some(Gate::Authorization),
                Some(denial_reason),
                Some(outcome.decision),
            )?;
            self.rejected_by = Some(Gate::Authorization);
            Ok(recorded)
        }
    }

    // --- The forward path past the gates -----------------------------------

    /// APPROVED → ISSUED (§14.1 edge 9: signed, sequenced command sent). The
    /// evidence reference is the signature or authenticated transport
    /// identity (§14.3 field 15); the adapter that verifies it is P4.16's.
    pub fn record_issue(
        &mut self,
        authenticity_reference: EvidenceReference,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        self.commit_transition(
            CommandState::Issued,
            actor,
            clock,
            None,
            None,
            Some(authenticity_reference),
        )
    }

    /// ISSUED → ACKNOWLEDGED (§14.1 edge 13: device/gateway accepts).
    /// Acknowledgement is acceptance at the application layer — NOT physical
    /// delivery (§14.1). The evidence reference identifies the acceptance
    /// record.
    pub fn record_acknowledgement(
        &mut self,
        acceptance_reference: EvidenceReference,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        self.commit_transition(
            CommandState::Acknowledged,
            actor,
            clock,
            None,
            None,
            Some(acceptance_reference),
        )
    }

    /// ACKNOWLEDGED → TRACKING (§14.1 edge 18: telemetry observed). The
    /// evidence reference identifies the telemetry observation.
    pub fn record_tracking_started(
        &mut self,
        telemetry_reference: EvidenceReference,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        self.commit_transition(
            CommandState::Tracking,
            actor,
            clock,
            None,
            None,
            Some(telemetry_reference),
        )
    }

    /// TRACKING → COMPLETED (§14.1 edge 22: acceptance criterion satisfied).
    /// The evidence reference identifies the satisfied-criterion record;
    /// partial fleet success stays per asset (§14.3) — this completes ONE
    /// asset's command, never a portfolio.
    pub fn record_completion(
        &mut self,
        acceptance_evidence: EvidenceReference,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        self.commit_transition(
            CommandState::Completed,
            actor,
            clock,
            None,
            None,
            Some(acceptance_evidence),
        )
    }

    /// ISSUED / ACKNOWLEDGED / TRACKING → FAILED (§14.1 edges 14, 19, 23).
    /// The trigger must be the one the frozen table annotates for the
    /// current state (a NACK cannot fail an ACKNOWLEDGED command; a tracking
    /// deviation cannot fail an ISSUED one).
    pub fn record_failure(
        &mut self,
        reason: FailureReason,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if !reason.allowed_from(self.state) {
            return Err(LifecycleError::InvalidFailureReason {
                current: self.state,
                reason: reason.reason_text(),
            });
        }
        self.commit_transition(
            CommandState::Failed,
            actor,
            clock,
            None,
            Some(reason.reason_text().to_string()),
            None,
        )
    }

    /// APPROVED / ISSUED / ACKNOWLEDGED / TRACKING → EXPIRED (§14.1 edges
    /// 10, 15, 20, 24). Guarded by the controlled clock: the validity
    /// window must actually have ended ([`Self::validity_has_lapsed`]) —
    /// expiry is never recorded early, and the milestone not reached must
    /// match the current state. The sweep/timer is P4.15's.
    pub fn record_expiry(
        &mut self,
        milestone: ExpiryMilestone,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if !self.validity_has_lapsed(clock) {
            return Err(LifecycleError::ValidityNotLapsed {
                now_unix_millis: clock.now_unix_millis(),
                valid_until_millis: self.valid_until_millis,
            });
        }
        match ExpiryMilestone::expected_for(self.state) {
            Some(expected) if expected == milestone => {}
            _ => {
                return Err(LifecycleError::InvalidExpiryMilestone {
                    current: self.state,
                    milestone: milestone.reason_text(),
                });
            }
        }
        self.commit_transition(
            CommandState::Expired,
            actor,
            clock,
            None,
            Some(milestone.reason_text().to_string()),
            None,
        )
    }

    /// APPROVED / ISSUED → CANCELLED (§14.1 edges 12, 17: operator/policy
    /// cancellation before issue or while supported). The reason is required
    /// (§14.2); cancellation is the explicit act and is NEVER used as a
    /// synonym for supersede (§14.2 SUPERSEDED row; P0.11 §3.1).
    pub fn record_cancellation(
        &mut self,
        reason: impl Into<String>,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(LifecycleError::EmptyField { field: "reason" });
        }
        self.commit_transition(
            CommandState::Cancelled,
            actor,
            clock,
            None,
            Some(reason),
            None,
        )
    }

    /// VALIDATED / FEASIBLE / APPROVED / ISSUED / ACKNOWLEDGED / TRACKING →
    /// SUPERSEDED (§14.1 edges 5, 8, 11, 16, 21, 25: a newer approved
    /// schedule/command revision replaces the command before completion).
    /// The record is retained for audit; the backward link
    /// ([`Self::superseded_by`]) is set here and the forward link lives on
    /// the replacement's `DispatchCommand::supersedes` — the persistence
    /// layer stores BOTH directions (D-P4.10-3, mirroring the P3.22
    /// revision-chain supersede discipline). DISTINCT from CANCELLED.
    pub fn record_superseded_by(
        &mut self,
        superseding: CommandId,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        if superseding == self.command_id {
            return Err(LifecycleError::SelfSupersede {
                command_id: self.command_id,
            });
        }
        let evidence = EvidenceReference::new(format!("command:{superseding}"))?;
        let recorded = self.commit_transition(
            CommandState::Superseded,
            actor,
            clock,
            None,
            Some("invalidated by a newer approved schedule/command revision".to_string()),
            Some(evidence),
        )?;
        self.superseded_by = Some(superseding);
        Ok(recorded)
    }

    /// FAILED / EXPIRED → FALLBACK (§14.1 edges 26, 27). The evidence
    /// reference identifies the fallback policy that fired (P4.17 owns the
    /// per-asset/site policies and the no-universal-fallback rule, §14.4);
    /// the STATE and the transition are implemented here. FALLBACK is
    /// terminal (the freeze draws no outgoing edge from it).
    pub fn record_fallback(
        &mut self,
        policy_reference: EvidenceReference,
        actor: Actor,
        clock: &dyn LifecycleClock,
    ) -> Result<CommandTransition, LifecycleError> {
        let reason = match self.state {
            CommandState::Failed => "asset/site fallback policy activated",
            CommandState::Expired => "fallback policy requires",
            _ => {
                return Err(LifecycleError::WrongState {
                    current: self.state,
                    operation: "record_fallback",
                });
            }
        };
        self.commit_transition(
            CommandState::Fallback,
            actor,
            clock,
            None,
            Some(reason.to_string()),
            Some(policy_reference),
        )
    }
}
