// P4.15 — Expiry, cancellation, supersede (§14.1–§14.2; ledger P4.15;
//         decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/14_16_lifecycle_completion.md`).
//
// The three non-forward lifecycle completions over the P4.10 engine, the
// P4.11–P4.13 store seam, and the P4.14 replay:
//
// - **EXPIRY (D-P4.15-1, decided once): a validity-window SWEEP.** The
//   sweep is a batch scan of the active command views at a controlled-clock
//   instant ([`sweep_validity_windows`] — pure), applied per command
//   through [`CommandExpiryService::apply_expiry`]. The alternative — only
//   a check at each transition attempt — was RECORDED but rejected as the
//   sole mechanism: a command nobody touches would never expire, and
//   §14.2 requires EXPIRED ("validity window ended before …") to be a real
//   state of the world, not only a guard. The domain's per-transition
//   guard (`CommandLifecycle::validity_has_lapsed`, `record_expiry`'s
//   `ValidityNotLapsed`) is RETAINED unchanged — the sweep can never fire
//   early because every application goes through it. The sweep moves
//   APPROVED / ISSUED / ACKNOWLEDGED / TRACKING commands to EXPIRED when
//   `valid-until` passes before the next milestone, records the expiry
//   time AND the milestone not reached ([`ExpiryOutcome`]), and NEVER
//   touches the other ten states — the frozen table has no EXPIRED edge
//   from them. EXPIRED has ONE outgoing edge (→ FALLBACK, frozen
//   transition 27, where the policy requires it — P4.17 owns the
//   policies); EXPIRED is NOT terminal, and the conformance test pins that
//   against the frozen oracle.
// - **CANCELLATION (D-P4.15-2):** an explicit operator/policy act, APPROVED
//   → CANCELLED and ISSUED → CANCELLED (the only states with the frozen
//   edge), with actor + reason MANDATORY (the domain engine enforces the
//   non-empty reason; this module refuses the other states with a named
//   error). Cancellation is NEVER a synonym for supersede: they are
//   different frozen states, different journal rows, different events, and
//   the audit records are DISTINGUISHABLE (tested).
// - **SUPERSEDE (D-P4.15-3):** a newer approved replacement moves a command
//   in VALIDATED / FEASIBLE / APPROVED / ISSUED / ACKNOWLEDGED / TRACKING
//   to SUPERSEDED with the bidirectional supersedes/superseded-by links
//   (the P3.22/P4.10 discipline: the backward link is the journal row's
//   `command:<uuid>` evidence, the forward link is the replacement's
//   §14.3 field 17). The record is RETAINED for audit (append-only journal;
//   nothing is deleted).
// - **EVENTS (ledger step 4):** `DispatchExpired`, `DispatchCancelled`,
//   `DispatchSuperseded` — the frozen §15.3 vocabulary — are emitted
//   through the transactional outbox, paired with the transition's store
//   receipt (state change first, then the outbox write).
//
// ## Recorded event-scope + payload gap (honest, not silently filled)
//
// The frozen §15.3 dispatch payloads carry correlation, tenant/portfolio/
// site/asset/command IDs, and `event_time` ONLY — no actor, reason, or
// reference fields (reshaping a frozen payload is a schema-version change
// this subtask must not make, P0.14/P1.09). Two recorded consequences:
//
// 1. **Actor/reason/references** travel in the transition JOURNAL ROW
//    (from, to, actor, timestamp, reason, evidence reference) — the
//    evidence of record; the event identifies the command so consumers
//    fetch the row. No payload field was invented.
// 2. **The portfolio/site/asset scope IDs** come from the command record
//    the CALLER holds (the store row's scope columns via
//    [`EventScope`]) — the store seam's view carries only the validity
//    bound + journal, and the service refuses to guess a scope it cannot
//    read (a wrong scope on an event would break tenant routing).
//
// ## Clock discipline
//
// Every timestamp comes from the injected `LifecycleClock` — never a wall
// clock. The ledger's clock-drift risk (§17.4) is met by the domain
// engine's timestamp-regression guard plus the sweep-only-at-controlled-
// instants design; time-quality monitoring is noted in the decision doc
// as a §17.4 follow-up.
//
// ## Control status
//
// Simulation and shadow only (§21.4). Cancellation and supersession are
// control-plane state changes over the audit journal — no device write,
// no field action, and no safety claim accompanies them. FlexCore performs
// no primary frequency response, synthetic inertia, relay protection,
// inverter inner-loop control, or sub-cycle control (§14.5).

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::command_lifecycle::{
    Actor, CommandLifecycle, CommandTransition, ExpiryMilestone, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{CommandId, PortfolioId, SiteId, TenantId, AssetId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{
    DispatchCancelledPayload, DispatchExpiredPayload, DispatchSupersededPayload, DomainEvent,
};
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::state_change::StateChangeReceipt;

use crate::command_gates::{CommandGateStore, CommandGateStoreError, CommandGateView};
use crate::command_record::{expiry_milestone_for, replay_full_lifecycle, CommandRecordError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;

/// The recorded D-P4.15-1 decision statement (the expiry mechanism, decided
/// once — see the module header).
pub const EXPIRY_SWEEP_DECISION: &str =
    "expiry is a validity-window SWEEP (batch scan at controlled-clock instants) applied \
     through the domain per-transition guard; a transition-attempt-only check was rejected \
     because an untouched command would never expire (D-P4.15-1)";

/// The recorded event-payload gap statement (see the module header).
pub const EXPIRY_EVENT_PAYLOAD_GAP_NOTE: &str = "the frozen §15.3 dispatch payloads carry \
     identity and time only; actor/reason/references travel in the transition journal row \
     (the evidence of record), fetched by command ID — no payload field was invented (P0.14)";

// --- Event scope --------------------------------------------------------------------

/// The portfolio/site/asset scope of the command a dispatch event is about.
/// The frozen dispatch payloads require these IDs; the caller supplies them
/// from the command record it holds (the store row's scope columns) — the
/// store seam's view carries only the validity bound + journal, and the
/// service refuses to guess a scope it cannot read (D-P4.15-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventScope {
    /// The command's portfolio.
    pub portfolio_id: PortfolioId,
    /// The command's site.
    pub site_id: SiteId,
    /// The command's asset.
    pub asset_id: AssetId,
}

// --- Errors -------------------------------------------------------------------------

/// The named errors of the expiry/cancellation/supersede service. A state
/// that cannot take the operation is a NAMED error — never silently
/// ignored, never coerced into a different transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandExpiryError {
    /// The command does not exist in the caller's tenant scope.
    NotFound {
        /// The command that was not found.
        command_id: String,
    },
    /// The operation does not apply to the command's current state (e.g.
    /// cancellation of a TRACKING command, supersede of a PROPOSED one,
    /// expiry of a COMPLETED one).
    WrongState {
        /// The state the command is in.
        current: CommandState,
        /// Which operation was attempted.
        operation: &'static str,
    },
    /// A domain lifecycle guard rejected the transition (the frozen-edge
    /// oracle or one of its reason/evidence/milestone rules).
    Lifecycle(LifecycleError),
    /// The store seam failed.
    Store(CommandGateStoreError),
    /// The outbox write failed after the state change was recorded (the
    /// composition root's unit of work resolves the pair).
    Event {
        /// The recorded detail.
        detail: String,
    },
    /// A timestamp could not be represented (controlled-clock discipline).
    InvalidTimestamp {
        /// The recorded detail.
        detail: String,
    },
    /// The persisted journal could not be replayed — fail-closed.
    InconsistentJournal {
        /// The command whose journal is inconsistent.
        command_id: String,
        /// The recorded detail.
        detail: String,
    },
}

impl std::fmt::Display for CommandExpiryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandExpiryError::NotFound { command_id } => {
                write!(f, "command {command_id} does not exist in this tenant scope")
            }
            CommandExpiryError::WrongState { current, operation } => write!(
                f,
                "operation `{operation}` does not apply to a command in state {}",
                current.name()
            ),
            CommandExpiryError::Lifecycle(error) => write!(f, "lifecycle guard: {error}"),
            CommandExpiryError::Store(error) => write!(f, "command store: {error}"),
            CommandExpiryError::Event { detail } => write!(f, "outbox write failed: {detail}"),
            CommandExpiryError::InvalidTimestamp { detail } => write!(f, "{detail}"),
            CommandExpiryError::InconsistentJournal { command_id, detail } => {
                write!(f, "command {command_id}: inconsistent journal: {detail}")
            }
        }
    }
}

impl std::error::Error for CommandExpiryError {}

impl From<CommandRecordError> for CommandExpiryError {
    fn from(error: CommandRecordError) -> Self {
        match error {
            CommandRecordError::InconsistentJournal { command_id, detail } => {
                CommandExpiryError::InconsistentJournal { command_id, detail }
            }
            CommandRecordError::InvalidTimestamp { detail } => {
                CommandExpiryError::InvalidTimestamp { detail }
            }
            CommandRecordError::Lifecycle(lifecycle) => CommandExpiryError::Lifecycle(lifecycle),
            other => CommandExpiryError::InconsistentJournal {
                command_id: String::new(),
                detail: format!("record validation failed during replay: {other}"),
            },
        }
    }
}

impl From<CommandGateStoreError> for CommandExpiryError {
    fn from(error: CommandGateStoreError) -> Self {
        CommandExpiryError::Store(error)
    }
}

// --- The pure validity-window sweep ---------------------------------------------------

/// One command view the sweep scans: the store's authoritative state and
/// validity bound (the sweep never trusts caller-side state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiryCandidate {
    /// The command.
    pub command_id: CommandId,
    /// The command's current state (store-authoritative).
    pub state: CommandState,
    /// The §14.2 valid-until bound (UTC unix millis).
    pub valid_until_millis: i64,
}

/// One sweep decision: the command to expire, the state it would expire
/// FROM, the milestone it did not reach, the bound, and the sweep instant.
/// The application ([`CommandExpiryService::apply_expiry`]) re-checks every
/// fact against the store and the domain guard, so a stale candidate can
/// never fire a wrong transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiryDecision {
    /// The command to expire.
    pub command_id: CommandId,
    /// The state the command was scanned in (frozen edge source).
    pub from_state: CommandState,
    /// The milestone the command did not reach (P4.15 step 1).
    pub milestone: ExpiryMilestone,
    /// The validity bound that passed (UTC unix millis).
    pub valid_until_millis: i64,
    /// The sweep instant (controlled clock, UTC unix millis).
    pub now_unix_millis: i64,
}

/// The pure validity-window sweep (D-P4.15-1; ledger P4.15 step 1). Scans
/// the candidates at one controlled-clock instant and returns a decision
/// for EVERY candidate that (a) is in a state with a frozen EXPIRED edge
/// (APPROVED / ISSUED / ACKNOWLEDGED / TRACKING — via
/// `expiry_milestone_for`) and (b) whose window has ended
/// (`now >= valid_until`, §10.3 end-exclusive, matching the domain guard).
/// Everything else yields NO decision — in particular PROPOSED / VALIDATED
/// / FEASIBLE can never expire (no frozen edge) and terminal states can
/// never leave.
pub fn sweep_validity_windows(
    candidates: &[ExpiryCandidate],
    now_unix_millis: i64,
) -> Vec<ExpiryDecision> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let milestone = expiry_milestone_for(candidate.state)?;
            (now_unix_millis >= candidate.valid_until_millis).then(|| ExpiryDecision {
                command_id: candidate.command_id,
                from_state: candidate.state,
                milestone,
                valid_until_millis: candidate.valid_until_millis,
                now_unix_millis,
            })
        })
        .collect()
}

// --- The service -----------------------------------------------------------------------

/// The expiry/cancellation/supersede service (P4.15). All collaborators are
/// injected; timestamps come EXCLUSIVELY from the controlled clock.
pub struct CommandExpiryService<'a> {
    /// The command-record store seam (the P4.10 persistence surface).
    pub store: &'a dyn CommandGateStore,
    /// The transactional outbox (events).
    pub outbox: &'a dyn EventOutbox,
    /// The event-payload encoder seam.
    pub encoder: &'a dyn EventPayloadEncoder,
    /// The identifier generator (event correlation ids).
    pub ids: &'a dyn IdGenerator,
    /// The controlled clock (every timestamp).
    pub clock: &'a dyn LifecycleClock,
}

/// The outcome of one applied expiry (P4.15 step 1: the expiry time and the
/// milestone not reached are RECORDED).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiryOutcome {
    /// The journalled EXPIRED transition (from, to, actor, timestamp,
    /// reason = the milestone not reached).
    pub transition: CommandTransition,
    /// The milestone the command did not reach.
    pub milestone: ExpiryMilestone,
    /// The expiry instant (the transition timestamp, controlled clock).
    pub expired_at_unix_millis: i64,
}

/// The outcome of one applied cancellation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationOutcome {
    /// The journalled CANCELLED transition (reason = the operator/policy
    /// reason; actor = the cancelling identity).
    pub transition: CommandTransition,
}

/// The outcome of one applied supersession.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersedeOutcome {
    /// The journalled SUPERSEDED transition on the REPLACED command.
    pub transition: CommandTransition,
    /// The replaced command.
    pub superseded: CommandId,
    /// The replacing command (the backward link recorded in the journal
    /// evidence as `command:<uuid>`; the forward link lives on the
    /// replacement's §14.3 field 17).
    pub superseded_by: CommandId,
}

impl<'a> CommandExpiryService<'a> {
    /// Loads a command view or reports `NotFound`.
    fn load(
        &self,
        ctx: &TenantContext,
        command_id: CommandId,
    ) -> Result<CommandGateView, CommandExpiryError> {
        self.store
            .load_command(ctx, command_id)
            .map_err(CommandExpiryError::Store)?
            .ok_or(CommandExpiryError::NotFound {
                command_id: command_id.to_string(),
            })
    }

    /// Replays the persisted journal through the domain engine (the P4.14
    /// full replay) so the next transition applies from the true current
    /// state.
    fn replay(
        &self,
        command_id: CommandId,
        view: &CommandGateView,
    ) -> Result<CommandLifecycle, CommandExpiryError> {
        replay_full_lifecycle(command_id, view).map_err(CommandExpiryError::from)
    }

    /// The controlled clock's current instant as an `EventTime`.
    fn event_time(&self) -> Result<EventTime, CommandExpiryError> {
        EventTime::from_unix_millis(self.clock.now_unix_millis()).map_err(|error| {
            CommandExpiryError::InvalidTimestamp {
                detail: format!("the controlled clock instant is not representable: {error}"),
            }
        })
    }

    /// Persists one transition through the store seam (the compare-and-swap
    /// state change plus the immutable journal row, in the adapter's one
    /// transaction), returning the receipt the event write pairs with. A
    /// lost race is [`CommandExpiryError::Store`] — the sweep decision is
    /// re-checked against the store, so a stale candidate can never fire a
    /// wrong transition.
    fn persist(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransition,
    ) -> Result<StateChangeReceipt, CommandExpiryError> {
        self.store
            .journal_transition(ctx, transition)
            .map_err(CommandExpiryError::Store)
    }

    /// Emits one frozen dispatch-lifecycle event through the transactional
    /// outbox (state change first, then the outbox write — §6.3 step 1).
    fn emit_event(
        &self,
        state_change: StateChangeReceipt,
        event: DomainEvent,
        correlation: FlexUuid,
    ) -> Result<(), CommandExpiryError> {
        emit(
            state_change,
            event,
            correlation,
            self.event_time()?,
            self.encoder,
            self.outbox,
        )
        .map_err(|error| CommandExpiryError::Event {
            detail: error.to_string(),
        })
    }

    // ------------------------------------------------------------------
    // Expiry
    // ------------------------------------------------------------------

    /// Applies one expiry (P4.15 step 1): loads the store view, replays
    /// the lifecycle, and lets the DOMAIN engine record APPROVED / ISSUED /
    /// ACKNOWLEDGED / TRACKING → EXPIRED — which re-checks (a) that the
    /// validity window has actually ended at the controlled clock
    /// (`ValidityNotLapsed` — expiry never fires early) and (b) that the
    /// milestone matches the current state. The journal reason is the
    /// milestone not reached; the expiry time is the transition timestamp.
    /// A command in any other state is a named `WrongState` error.
    /// `DispatchExpired` is emitted.
    pub fn apply_expiry(
        &self,
        ctx: &TenantContext,
        scope: EventScope,
        command_id: CommandId,
        actor: &Actor,
    ) -> Result<ExpiryOutcome, CommandExpiryError> {
        let view = self.load(ctx, command_id)?;
        let milestone = expiry_milestone_for(view.state()).ok_or(CommandExpiryError::WrongState {
            current: view.state(),
            operation: "apply_expiry",
        })?;
        let mut lifecycle = self.replay(command_id, &view)?;
        let transition = lifecycle.record_expiry(milestone, actor.clone(), self.clock)?;
        let receipt = self.persist(ctx, &transition)?;
        self.emit_event(
            receipt,
            DomainEvent::DispatchExpired(DispatchExpiredPayload {
                correlation_id: command_id.uuid(),
                tenant_id: ctx.tenant_id(),
                portfolio_id: scope.portfolio_id,
                site_id: scope.site_id,
                asset_id: scope.asset_id,
                command_id: command_id.uuid(),
                event_time: self.event_time()?,
            }),
            command_id.uuid(),
        )?;
        Ok(ExpiryOutcome {
            expired_at_unix_millis: transition.at_unix_millis,
            milestone,
            transition,
        })
    }

    /// The sweep application: runs the PURE sweep over the supplied
    /// candidates and applies every decision (each through the full
    /// store + domain guards). A candidate whose stored state moved on
    /// since the scan fails closed with its named error — the caller
    /// decides whether one stale candidate aborts the sweep or is logged;
    /// the decisions are independent.
    pub fn run_sweep(
        &self,
        ctx: &TenantContext,
        scope: EventScope,
        candidates: &[ExpiryCandidate],
        actor: &Actor,
    ) -> Result<Vec<ExpiryOutcome>, CommandExpiryError> {
        let now = self.clock.now_unix_millis();
        let decisions = sweep_validity_windows(candidates, now);
        let mut applied = Vec::with_capacity(decisions.len());
        for decision in decisions {
            applied.push(self.apply_expiry(ctx, scope, decision.command_id, actor)?);
        }
        Ok(applied)
    }

    // ------------------------------------------------------------------
    // Cancellation
    // ------------------------------------------------------------------

    /// Records an explicit operator/policy CANCELLATION (P4.15 step 2):
    /// APPROVED → CANCELLED and ISSUED → CANCELLED are the only frozen
    /// edges; any other state is a named [`CommandExpiryError::WrongState`]
    /// — never silently coerced, never used as a supersede synonym. The
    /// reason is MANDATORY (empty/whitespace reasons are refused by the
    /// domain engine); the actor is the cancelling identity.
    /// `DispatchCancelled` is emitted.
    pub fn cancel(
        &self,
        ctx: &TenantContext,
        scope: EventScope,
        command_id: CommandId,
        actor: &Actor,
        reason: impl Into<String>,
    ) -> Result<CancellationOutcome, CommandExpiryError> {
        let view = self.load(ctx, command_id)?;
        if !matches!(view.state(), CommandState::Approved | CommandState::Issued) {
            return Err(CommandExpiryError::WrongState {
                current: view.state(),
                operation: "cancel",
            });
        }
        let mut lifecycle = self.replay(command_id, &view)?;
        let transition = lifecycle.record_cancellation(reason, actor.clone(), self.clock)?;
        let receipt = self.persist(ctx, &transition)?;
        self.emit_event(
            receipt,
            DomainEvent::DispatchCancelled(DispatchCancelledPayload {
                correlation_id: command_id.uuid(),
                tenant_id: ctx.tenant_id(),
                portfolio_id: scope.portfolio_id,
                site_id: scope.site_id,
                asset_id: scope.asset_id,
                command_id: command_id.uuid(),
                event_time: self.event_time()?,
            }),
            command_id.uuid(),
        )?;
        Ok(CancellationOutcome { transition })
    }

    // ------------------------------------------------------------------
    // Supersede
    // ------------------------------------------------------------------

    /// Records a SUPERSESSION (P4.15 step 3): a newer approved replacement
    /// invalidates the command before completion. Allowed exactly from the
    /// six frozen states (VALIDATED / FEASIBLE / APPROVED / ISSUED /
    /// ACKNOWLEDGED / TRACKING); any other state is a named error. The
    /// backward link (`superseded_by`) is recorded in the journal evidence
    /// as `command:<uuid>` (D-P4.10-3) and on the replayed aggregate; the
    /// forward link lives on the replacement's §14.3 field 17 — the
    /// persistence layer stores BOTH directions. The record is RETAINED
    /// for audit (append-only journal). Self-supersession is refused by
    /// the domain engine. `DispatchSuperseded` is emitted — DISTINGUISHABLE
    /// from `DispatchCancelled` (different event, different state,
    /// different journal reason).
    pub fn supersede(
        &self,
        ctx: &TenantContext,
        scope: EventScope,
        command_id: CommandId,
        replacement: CommandId,
        actor: &Actor,
    ) -> Result<SupersedeOutcome, CommandExpiryError> {
        let view = self.load(ctx, command_id)?;
        if !matches!(
            view.state(),
            CommandState::Validated
                | CommandState::Feasible
                | CommandState::Approved
                | CommandState::Issued
                | CommandState::Acknowledged
                | CommandState::Tracking
        ) {
            return Err(CommandExpiryError::WrongState {
                current: view.state(),
                operation: "supersede",
            });
        }
        let mut lifecycle = self.replay(command_id, &view)?;
        let transition = lifecycle.record_superseded_by(replacement, actor.clone(), self.clock)?;
        let receipt = self.persist(ctx, &transition)?;
        self.emit_event(
            receipt,
            DomainEvent::DispatchSuperseded(DispatchSupersededPayload {
                correlation_id: command_id.uuid(),
                tenant_id: ctx.tenant_id(),
                portfolio_id: scope.portfolio_id,
                site_id: scope.site_id,
                asset_id: scope.asset_id,
                command_id: command_id.uuid(),
                event_time: self.event_time()?,
            }),
            command_id.uuid(),
        )?;
        Ok(SupersedeOutcome {
            transition,
            superseded: command_id,
            superseded_by: replacement,
        })
    }
}

// Tenant-context re-exports kept local (used by callers building scopes):
// the tenant of the scope is ALWAYS the caller's context tenant (P0.16
// rule 5 — the tenant comes from the authenticated context, never from a
// command field).
impl EventScope {
    /// The tenant the events for this scope must carry (the caller's
    /// authenticated tenant).
    pub fn tenant_of(&self, ctx: &TenantContext) -> TenantId {
        ctx.tenant_id()
    }
}
