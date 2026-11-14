// P4.10 — Command record persistence contract (report §14.1–§14.2, §16.1,
//         §16.2; ledger P4.10; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/10_command_state_machine.md`).
//
// The append-oriented persistence contract for the protected command
// lifecycle: an append-only COMMAND RECORD (one row per command, created at
// PROPOSED and never rewritten in content) plus an append-only STATE-CHANGE
// JOURNAL (one immutable row per journalled transition: from, to, actor,
// timestamp, reason, evidence reference — ledger P4.10 step 6). The design
// mirrors the P3.22 schedule-revision pattern exactly: immutable rows,
// transition-only updates, bidirectional supersede links retained for audit.
//
// ## The write model, decided once (D-P4.10-2)
//
// - A command row is INSERTed exactly once, in the state `PROPOSED` (the
//   freeze's initial marker: every command enters as PROPOSED — the
//   database enforces this with a BEFORE INSERT trigger, mirroring the
//   `active`-only revision insert of P3.22).
// - The ONLY permitted update of a command row is a STATE TRANSITION: the
//   runtime role carries a COLUMN-LEVEL UPDATE grant on exactly the two
//   transition columns (`state`, `superseded_by_command_id` — the recorded
//   exception anticipated by D-P3.21-8, same shape as migration 0014), and
//   a BEFORE UPDATE trigger rejects any content change, any transition not
//   in the frozen §14.1 table, and any `SUPERSEDED` movement without the
//   superseded-by link. THE FROZEN 27-EDGE TABLE IS ENCODED IN THAT TRIGGER
//   AS DATABASE DEFENCE IN DEPTH ONLY — the single source of truth remains
//   the domain's P0.11 table (`flexcore_domain::command_state`); this
//   contract validates every transition through
//   `flexcore_domain::command_state::try_transition` at the row-constructor
//   level, and a divergence between the two encodings is a BUG against the
//   oracle, never an extension.
// - The ONE-ACTIVE-TRANSITION invariant per command is enforced in SQL by
//   the compare-and-swap shape of the adapter's transition statement
//   (`UPDATE ... WHERE state = <from>` — the transition commits only from
//   the state the caller saw; a concurrent transition makes the UPDATE
//   affect zero rows) plus the trigger's one-state-change-per-UPDATE rule.
// - The journal (`dispatch_command_transitions`) is INSERT-only for the
//   runtime role: journal rows are never updated or deleted. The SUPERSEDED
//   and CANCELLED audit records are therefore structurally distinct
//   (§14.2; P0.11 §3.1) and the full transition history is reconstructable.
//
// ## The P4.12 pre-wiring (no feasibility gate here)
//
// Commands reference schedule ID + revision (P3.22, D-P3.22-1). The
// identity-level gate check lives on
// [`CommandRecordStore::ensure_schedule_revision_usable`]: a transition
// whose command references a SUPERSEDED (or unknown) schedule revision is
// rejected, calling the P3.22 `is_revision_superseded` surface. The §12.5
// network feasibility pipeline itself is P4.12's — nothing of it is built
// here.
//
// ## What this module can and cannot do
//
// - It CAN make broken records unrepresentable in the row types: the
//   constructors enforce the closed state vocabulary, the frozen-edge rule,
//   the §14.1 reason/evidence rules (which transition must carry which),
//   and non-empty identities.
// - It CANNOT enforce atomicity, tenant scoping, or the compare-and-swap
//   transition. Those are adapter obligations (binding, stated on the
//   trait) and enforced by the adapter's DB-gated tests; the database adds
//   the mechanical backstop (the 0016 trigger + grants + RLS).
//
// Control status: persistence bookkeeping for the simulation-only 0.x line.
// No dispatch path, no device write, no acknowledgement-that-delivers: an
// ACKNOWLEDGED journal row records application-layer acceptance, never
// physical delivery (§14.1).

use flexcore_domain::command_lifecycle::{CommandTransition, Gate};
use flexcore_domain::command_state::{CommandState, try_transition};
use flexcore_domain::ids::TenantId;

/// The closed command-state vocabulary, as stored text — the canonical
/// SCREAMING_SNAKE names of the 14 frozen §14.2 states, in
/// `CommandState::ALL` order. Pinned against the domain vocabulary by a
/// test in this crate.
pub const COMMAND_STATES: [&str; 14] = [
    "PROPOSED",
    "VALIDATED",
    "FEASIBLE",
    "APPROVED",
    "ISSUED",
    "ACKNOWLEDGED",
    "TRACKING",
    "COMPLETED",
    "REJECTED",
    "FAILED",
    "EXPIRED",
    "CANCELLED",
    "SUPERSEDED",
    "FALLBACK",
];

/// The gate vocabulary, as stored text (the three §14.1 gates are distinct;
/// every gate transition and every gate rejection names its gate).
pub const COMMAND_GATES: [&str; 3] = ["validation", "feasibility", "authorization"];

/// The stored text of the initial state (the freeze's initial marker:
/// every command enters as PROPOSED).
pub const COMMAND_STATE_PROPOSED: &str = "PROPOSED";

/// The stored text of the §14.2 SUPERSEDED state (retained for audit,
/// distinct from CANCELLED).
pub const COMMAND_STATE_SUPERSEDED: &str = "SUPERSEDED";

/// States whose entry REQUIRES an evidence reference in the journal row:
/// the three gate passes (§6.4: each stage records evidence), the
/// evidence-carrying forward steps (signed issue, application-layer
/// acceptance, telemetry observed, satisfied criterion), the supersede
/// reference (§14.3 field 16), and the fallback policy reference (§14.4).
pub const COMMAND_STATES_REQUIRING_EVIDENCE: [&str; 9] = [
    "VALIDATED",
    "FEASIBLE",
    "APPROVED",
    "ISSUED",
    "ACKNOWLEDGED",
    "TRACKING",
    "COMPLETED",
    "SUPERSEDED",
    "FALLBACK",
];

/// States whose entry REQUIRES a reason in the journal row: the failing
/// check for REJECTED (§14.3), the trigger for FAILED, the milestone not
/// reached for EXPIRED (P4.15 step 1), the operator/policy reason for
/// CANCELLED (§14.2), and the policy that fired for FALLBACK (§14.4).
pub const COMMAND_STATES_REQUIRING_REASON: [&str; 5] = [
    "REJECTED", "FAILED", "EXPIRED", "CANCELLED", "FALLBACK",
];

/// States whose entry REQUIRES the gate to be named: the three gate passes
/// and the three gate rejections (§14.1: "validation, feasibility, and
/// authorization are distinct" — the audit record names WHICH gate).
pub const COMMAND_STATES_REQUIRING_GATE: [&str; 4] = [
    "VALIDATED", "FEASIBLE", "APPROVED", "REJECTED",
];

// --- Errors -------------------------------------------------------------------

/// Errors of the command-record store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStoreError {
    /// The row or transition argument is incomplete or malformed (`field`
    /// names the problem). Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A command with the same (command_id, tenant_id) already exists —
    /// command rows are immutable and a re-proposal is a new command.
    Conflict {
        command_id: String,
    },
    /// The command does not exist in the caller's tenant scope (existence
    /// never leaks across a tenant boundary, P0.16 rule 5).
    NotFound {
        command_id: String,
    },
    /// The transition is not in the frozen §14.1 table (validated through
    /// the domain's `try_transition` before anything is written).
    IllegalTransition {
        command_id: String,
        from: String,
        to: String,
    },
    /// The compare-and-swap transition failed: the command's stored state
    /// is not the `from` state the caller transitioned from (a concurrent
    /// transition won, or the caller's view was stale). Nothing was written.
    TransitionConflict {
        command_id: String,
        expected_from: String,
        actual_state: String,
    },
    /// The command's source schedule revision is SUPERSEDED — the P4.12
    /// pre-wiring gate check (a command referencing a superseded revision
    /// is rejected, P3.22 `PHASE4_GATE_NOTE`).
    ScheduleRevisionSuperseded {
        schedule_id: String,
        revision: u64,
    },
    /// The command's source schedule revision does not exist in the
    /// caller's tenant scope. Unknown is rejected, never treated as usable.
    ScheduleRevisionUnknown {
        schedule_id: String,
        revision: u64,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for CommandStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandStoreError::InvalidRecord { field } => {
                write!(f, "the command record is invalid: '{field}' is missing or malformed")
            }
            CommandStoreError::Conflict { command_id } => write!(
                f,
                "a command {command_id} already exists; command records are immutable and \
                 a re-proposal is a new command"
            ),
            CommandStoreError::NotFound { command_id } => write!(
                f,
                "command {command_id} does not exist in this tenant scope"
            ),
            CommandStoreError::IllegalTransition {
                command_id, from, to,
            } => write!(
                f,
                "command {command_id}: transition {from} -> {to} is not in the frozen \
                 command state machine"
            ),
            CommandStoreError::TransitionConflict {
                command_id,
                expected_from,
                actual_state,
            } => write!(
                f,
                "command {command_id} is in state {actual_state}, not {expected_from}; the \
                 transition applies only from the state the caller saw"
            ),
            CommandStoreError::ScheduleRevisionSuperseded { schedule_id, revision } => write!(
                f,
                "schedule {schedule_id} revision {revision} is SUPERSEDED; a command may \
                 not reference a superseded revision"
            ),
            CommandStoreError::ScheduleRevisionUnknown { schedule_id, revision } => write!(
                f,
                "schedule {schedule_id} revision {revision} does not exist in this tenant \
                 scope; an unknown revision is never usable"
            ),
            CommandStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for CommandStoreError {}

// --- Rows ---------------------------------------------------------------------

/// One command record to INSERT (the write row). A row is always created in
/// the state `PROPOSED` — the freeze's initial marker — and its content is
/// immutable from then on; the ONLY later write is the state transition
/// ([`CommandRecordStore::record_command_transition`]). Construction goes
/// exclusively through [`CommandRecordRow::new`].
///
/// Identifier fields are canonical hyphenated UUID text (the recorded
/// FlexUuid ↔ PG uuid mapping binds text and casts `::uuid` in SQL). The
/// full §14.3 command record travels in `command_payload_json` (opaque JSON
/// text here; the adapter validates it parses and binds jsonb) — the
/// complete §14.3 field-level enforcement is P4.14's; this row carries the
/// identity/reference fields the lifecycle and journal need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecordRow {
    /// The owning tenant (P0.16 rule 1 — `tenant_id` on every tenant-owned
    /// record, present on the row itself so RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The unique command ID (canonical UUID text; §14.3 field 1).
    pub command_id: String,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The target site (canonical UUID text).
    pub site_id: String,
    /// The target asset (canonical UUID text).
    pub asset_id: String,
    /// The source schedule chain the command realises (canonical UUID
    /// text; P3.22 identity scheme).
    pub schedule_id: String,
    /// The source schedule revision (≥ 1; P3.22).
    pub schedule_revision: u64,
    /// The valid-until bound (UTC millis) the EXPIRED state support
    /// compares against (§14.3 field 8).
    pub valid_until_millis: i64,
    /// The command THIS command replaces (§14.3 field 16, forward link;
    /// `None` when this command replaces nothing).
    pub supersedes_command_id: Option<String>,
    /// The full §14.3 command record as JSON text (opaque to this crate;
    /// immutable once written).
    pub command_payload_json: String,
}

impl CommandRecordRow {
    /// Builds and validates one command record row (always `PROPOSED`; see
    /// the type doc). Storage-level rules: every identifier/reference
    /// non-empty; `schedule_revision` in `1..=i64::MAX`; non-empty payload.
    pub fn new(
        tenant_id: TenantId,
        command_id: String,
        portfolio_id: String,
        site_id: String,
        asset_id: String,
        schedule_id: String,
        schedule_revision: u64,
        valid_until_millis: i64,
        supersedes_command_id: Option<String>,
        command_payload_json: String,
    ) -> Result<Self, CommandStoreError> {
        for (field, value) in [
            ("command_id", command_id.as_str()),
            ("portfolio_id", portfolio_id.as_str()),
            ("site_id", site_id.as_str()),
            ("asset_id", asset_id.as_str()),
            ("schedule_id", schedule_id.as_str()),
            ("command_payload_json", command_payload_json.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(CommandStoreError::InvalidRecord { field });
            }
        }
        if schedule_revision == 0 || schedule_revision > i64::MAX as u64 {
            return Err(CommandStoreError::InvalidRecord {
                field: "schedule_revision",
            });
        }
        if let Some(superseded) = supersedes_command_id.as_deref() {
            if superseded.trim().is_empty() {
                return Err(CommandStoreError::InvalidRecord {
                    field: "supersedes_command_id",
                });
            }
        }
        Ok(Self {
            tenant_id,
            command_id,
            portfolio_id,
            site_id,
            asset_id,
            schedule_id,
            schedule_revision,
            valid_until_millis,
            supersedes_command_id,
            command_payload_json,
        })
    }
}

/// One stored command record as READ (the immutable row plus its current
/// state and the superseded-by link). Constructed exclusively through
/// [`CommandRecord::new`], which validates the stored state against the
/// closed vocabulary and the state-link consistency rules (the adapter
/// fails closed on a malformed row rather than smuggling it through).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecord {
    /// The immutable command content.
    pub row: CommandRecordRow,
    /// The current state: one of [`COMMAND_STATES`].
    pub state: String,
    /// The command that replaced THIS command — the backward link of the
    /// bidirectional supersede pair (set exactly when `state` is
    /// `SUPERSEDED`; §14.3 field 16).
    pub superseded_by_command_id: Option<String>,
}

impl CommandRecord {
    /// Builds and validates one stored record. Consistency rules (mirrored
    /// by the database CHECK): the backward link is present exactly when
    /// the state is `SUPERSEDED`, and never names the command itself.
    pub fn new(
        row: CommandRecordRow,
        state: String,
        superseded_by_command_id: Option<String>,
    ) -> Result<Self, CommandStoreError> {
        if !COMMAND_STATES.contains(&state.as_str()) {
            return Err(CommandStoreError::InvalidRecord { field: "state" });
        }
        if (state == COMMAND_STATE_SUPERSEDED) != superseded_by_command_id.is_some() {
            return Err(CommandStoreError::InvalidRecord {
                field: "superseded_by_command_id",
            });
        }
        if let Some(by) = superseded_by_command_id.as_deref() {
            if by.trim().is_empty() || by == row.command_id {
                return Err(CommandStoreError::InvalidRecord {
                    field: "superseded_by_command_id",
                });
            }
        }
        Ok(Self {
            row,
            state,
            superseded_by_command_id,
        })
    }

    /// `true` when this command is SUPERSEDED (§14.2 — retained for audit,
    /// distinct from CANCELLED).
    pub fn is_superseded(&self) -> bool {
        self.state == COMMAND_STATE_SUPERSEDED
    }
}

/// One journalled state change (the immutable journal row the ledger P4.10
/// step 6 requires: from, to, actor, timestamp, reason, evidence reference).
/// Built from the domain's [`CommandTransition`] via
/// [`CommandTransitionRecord::from_transition`], which validates the
/// frozen-edge rule and the reason/evidence/gate rules BEFORE anything is
/// written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTransitionRecord {
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The command the transition belongs to (canonical UUID text).
    pub command_id: String,
    /// The state before the transition (one of [`COMMAND_STATES`]).
    pub from_state: String,
    /// The state after the transition (one of [`COMMAND_STATES`]).
    pub to_state: String,
    /// The actor identity recorded for the transition (non-empty).
    pub actor: String,
    /// The transition timestamp (UTC unix millis, from the controlled
    /// clock — never wall-clock).
    pub at_unix_millis: i64,
    /// The gate that produced the transition (one of [`COMMAND_GATES`];
    /// present exactly for the gate passes and gate rejections).
    pub gate: Option<String>,
    /// The reason, where the report requires one.
    pub reason: Option<String>,
    /// The evidence reference, where the transition rests on evidence.
    pub evidence_reference: Option<String>,
}

impl CommandTransitionRecord {
    /// Builds one journal record from the domain transition, validating:
    /// the command ID is non-empty; `from`/`to` are in the closed
    /// vocabulary; the edge is in the frozen table (via the domain's
    /// `try_transition` — the oracle, not a second table); the actor is
    /// non-empty; and the §14.1 reason/evidence/gate rules hold.
    pub fn from_transition(
        tenant_id: TenantId,
        transition: &CommandTransition,
    ) -> Result<Self, CommandStoreError> {
        let command_id = transition.command_id.to_string();
        if command_id.trim().is_empty() {
            return Err(CommandStoreError::InvalidRecord { field: "command_id" });
        }
        let from_state = transition.from.name().to_string();
        let to_state = transition.to.name().to_string();
        // The frozen table is the only edge authority (P0.11).
        try_transition(transition.from, transition.to).map_err(|_| {
            CommandStoreError::IllegalTransition {
                command_id: command_id.clone(),
                from: from_state.clone(),
                to: to_state.clone(),
            }
        })?;
        if transition.actor.identity().trim().is_empty() {
            return Err(CommandStoreError::InvalidRecord { field: "actor" });
        }
        let gate = transition.gate.map(|gate: Gate| gate.as_str().to_string());
        // Gate naming rule: the three gate passes and the gate rejections
        // name their gate; nothing else carries one.
        let requires_gate = COMMAND_STATES_REQUIRING_GATE.contains(&to_state.as_str());
        if requires_gate != gate.is_some() {
            return Err(CommandStoreError::InvalidRecord { field: "gate" });
        }
        if let Some(gate) = gate.as_deref() {
            if !COMMAND_GATES.contains(&gate) {
                return Err(CommandStoreError::InvalidRecord { field: "gate" });
            }
        }
        // Evidence rule (see COMMAND_STATES_REQUIRING_EVIDENCE). The nine
        // evidence-carrying states REQUIRE the reference; REJECTED MAY
        // carry it (the failing-check evidence — ledger P4.11: "REJECTED
        // with the failing check and evidence"; the domain engine records
        // it on every gate rejection, and migration 0016's evidence CHECK
        // encodes exactly this shape: required for the nine, unconstrained
        // for REJECTED). Every other state must NOT carry one.
        let evidence = transition
            .evidence
            .as_ref()
            .map(|evidence| evidence.as_str().to_string());
        let requires_evidence =
            COMMAND_STATES_REQUIRING_EVIDENCE.contains(&to_state.as_str());
        let evidence_allowed = requires_evidence || to_state == "REJECTED";
        if (requires_evidence && evidence.is_none())
            || (evidence.is_some() && !evidence_allowed)
        {
            return Err(CommandStoreError::InvalidRecord { field: "evidence_reference" });
        }
        if let Some(evidence) = evidence.as_deref() {
            if evidence.trim().is_empty() {
                return Err(CommandStoreError::InvalidRecord {
                    field: "evidence_reference",
                });
            }
        }
        // Reason rule (see COMMAND_STATES_REQUIRING_REASON).
        let reason = transition.reason.clone();
        let requires_reason = COMMAND_STATES_REQUIRING_REASON.contains(&to_state.as_str());
        if requires_reason != reason.is_some() {
            return Err(CommandStoreError::InvalidRecord { field: "reason" });
        }
        if let Some(reason) = reason.as_deref() {
            if reason.trim().is_empty() {
                return Err(CommandStoreError::InvalidRecord { field: "reason" });
            }
        }
        Ok(Self {
            tenant_id,
            command_id,
            from_state,
            to_state,
            actor: transition.actor.identity().to_string(),
            at_unix_millis: transition.at_unix_millis,
            gate,
            reason,
            evidence_reference: evidence,
        })
    }
}

/// The receipt of one persisted command record or one journalled
/// transition. Its existence proves the write was ISSUED (the same receipt
/// convention as the P3.21 record store).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReceipt {
    /// The command (canonical UUID text).
    pub command_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

impl CommandReceipt {
    /// Builds the receipt (adapter-side; values are taken as given from the
    /// executed statement).
    pub fn new(command_id: String, tenant_id: TenantId) -> Self {
        Self {
            command_id,
            tenant_id,
        }
    }
}

/// The receipt of one journalled transition, carrying the journal sequence
/// number the database assigned (per-command transition ordering is by
/// `(command_id, tenant_id, seq)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTransitionReceipt {
    /// The command (canonical UUID text).
    pub command_id: String,
    /// The journal sequence number of the recorded transition.
    pub seq: i64,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

// --- Listing (P4.30) -------------------------------------------------------------

/// The listing filter of [`CommandRecordStore::list_commands`]. Every
/// field is optional; an absent filter lists the tenant's records newest
/// first. There is NO unbounded query: the caller ALWAYS applies the page
/// size (the recorded P2.12/P3.28 listing convention).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandListFilter {
    /// Restrict to one stored state (one of [`COMMAND_STATES`]; any other
    /// value is [`CommandStoreError::InvalidRecord`]).
    pub state: Option<String>,
    /// Keyset cursor: return commands strictly BEFORE this command_id in
    /// the listing order (the opaque cursor of the previous page; the
    /// primary keys are UUIDv7s, so `command_id` order is a stable time
    /// order).
    pub after_command_id: Option<String>,
}

// --- The port -------------------------------------------------------------------

/// The command-record persistence contract (ledger P4.10 step 6; report
/// §14.1–§14.2, §16.1, §16.2). An append-oriented record + journal store:
///
/// - command rows are immutable in content (INSERT + SELECT; the ONLY write
///   besides the insert is the state transition below);
/// - every transition is journalled as an immutable row; the journal has NO
///   update and NO delete;
/// - the one-active-transition invariant per command is enforced in SQL: the
///   adapter's transition is a compare-and-swap on the current state
///   (`UPDATE ... WHERE state = <from>`), and the 0016 trigger admits
///   exactly one frozen-table state change per UPDATE;
/// - every operation runs inside the CALLER's unit of work (adapter
///   obligation, binding) — the command-row state change and its journal
///   row commit or roll back TOGETHER;
/// - every query is tenant-scoped: RLS via the transaction-scoped
///   `flexcore.tenant_id` setting (P1.12 §5.1) PLUS an explicit `tenant_id`
///   filter; a foreign-tenant command reads as `None` — existence never
///   leaks (P0.16 rule 5);
/// - storage failures surface as [`CommandStoreError::Backend`] with an
///   opaque code + message only.
pub trait CommandRecordStore {
    /// Inserts one NEW command record (always `PROPOSED`; the database
    /// rejects any other initial state — the freeze's initial marker). A
    /// (command_id, tenant_id) collision is [`CommandStoreError::Conflict`]
    /// (command rows are immutable; a re-proposal is a new command).
    fn insert_command_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &CommandRecordRow,
    ) -> Result<CommandReceipt, CommandStoreError>;

    /// Journalls ONE state change: a compare-and-swap state transition on
    /// the command row (only from `from_state` — a mismatch is
    /// [`CommandStoreError::TransitionConflict`]) plus the immutable
    /// journal row, in ONE transaction. The transition's edge and its
    /// reason/evidence/gate rules were validated by
    /// [`CommandTransitionRecord::from_transition`]; the database trigger
    /// re-checks the frozen edge as defence in depth.
    fn record_command_transition(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        transition: &CommandTransitionRecord,
    ) -> Result<CommandTransitionReceipt, CommandStoreError>;

    /// Looks up one command record (immutable content + current state +
    /// superseded-by link) in the caller's tenant scope. A foreign-tenant
    /// command reads as `None`.
    fn find_command(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandRecord>, CommandStoreError>;

    /// The full transition journal of one command, ordered by journal
    /// sequence — the audit view (every transition, never rewritten).
    fn transition_journal(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        command_id: &str,
    ) -> Result<Vec<CommandTransitionRecord>, CommandStoreError>;

    /// Lists command records in the caller's tenant scope, newest first
    /// (`command_id` DESC — the primary keys are UUIDv7s, so id order is a
    /// stable time order), applying the filter and the caller-supplied
    /// page bound. The §15.2 `/api/v1/dispatch-commands` read resource
    /// serves these records (read-only — no execute-command endpoint
    /// exists, §15.2). A state filter value outside [`COMMAND_STATES`] is
    /// [`CommandStoreError::InvalidRecord`].
    fn list_commands(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        filter: &CommandListFilter,
        limit: u32,
    ) -> Result<Vec<CommandRecord>, CommandStoreError>;

    /// The P4.12 pre-wiring gate check (cross-reference P3.22
    /// `PHASE4_GATE_NOTE`; NO feasibility gate here): rejects a command's
    /// source schedule revision when it is SUPERSEDED
    /// ([`CommandStoreError::ScheduleRevisionSuperseded`]) or unknown in
    /// the caller's tenant scope
    /// ([`CommandStoreError::ScheduleRevisionUnknown`]) — unknown is never
    /// "usable". Calls the P3.22 `is_revision_superseded` surface.
    fn ensure_schedule_revision_usable(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<(), CommandStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::command_lifecycle::{Actor, EvidenceReference};
    use flexcore_domain::ids::CommandId;

    fn tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 4;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn base_row() -> CommandRecordRow {
        CommandRecordRow::new(
            tenant(),
            "0192a5e0-0000-7000-8000-0000000000a1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000b1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000d1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000e1".to_string(),
            3,
            1_700_000_100_000,
            None,
            "{\"p_mw\":1.5}".to_string(),
        )
        .expect("base row builds")
    }

    fn transition(
        command_id: CommandId,
        from: CommandState,
        to: CommandState,
    ) -> CommandTransition {
        CommandTransition {
            command_id,
            from,
            to,
            actor: Actor::new("svc:validator").expect("actor builds"),
            at_unix_millis: 1_000,
            gate: None,
            reason: None,
            evidence: None,
        }
    }

    #[test]
    fn the_state_and_gate_vocabularies_are_closed_and_match_the_domain() {
        assert_eq!(COMMAND_STATES.len(), 14);
        for (stored, state) in COMMAND_STATES.iter().zip(CommandState::ALL.iter()) {
            assert_eq!(stored, &state.name(), "storage vocabulary mirrors the domain");
        }
        assert_eq!(COMMAND_GATES, ["validation", "feasibility", "authorization"]);
    }

    #[test]
    fn a_command_row_is_proposed_shaped_and_validated() {
        let row = base_row();
        assert_eq!(row.schedule_revision, 3);
        assert!(row.supersedes_command_id.is_none());
        for (field, corrupt) in [
            ("command_id", 0usize),
            ("portfolio_id", 1),
            ("site_id", 2),
            ("asset_id", 3),
            ("schedule_id", 4),
            ("command_payload_json", 5),
        ] {
            let mut fields = (
                row.command_id.clone(),
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                row.command_payload_json.clone(),
            );
            match corrupt {
                0 => fields.0 = " ".to_string(),
                1 => fields.1 = "".to_string(),
                2 => fields.2 = "  ".to_string(),
                3 => fields.3 = "".to_string(),
                4 => fields.4 = "".to_string(),
                _ => fields.5 = " ".to_string(),
            }
            assert_eq!(
                CommandRecordRow::new(
                    tenant(),
                    fields.0,
                    fields.1,
                    fields.2,
                    fields.3,
                    fields.4,
                    row.schedule_revision,
                    row.valid_until_millis,
                    None,
                    fields.5,
                ),
                Err(CommandStoreError::InvalidRecord { field }),
                "empty {field} must be rejected"
            );
        }
        assert_eq!(
            CommandRecordRow::new(
                tenant(),
                row.command_id.clone(),
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                0,
                row.valid_until_millis,
                None,
                row.command_payload_json.clone(),
            ),
            Err(CommandStoreError::InvalidRecord { field: "schedule_revision" })
        );
        assert_eq!(
            CommandRecordRow::new(
                tenant(),
                row.command_id.clone(),
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                row.schedule_revision,
                row.valid_until_millis,
                Some(" ".to_string()),
                row.command_payload_json.clone(),
            ),
            Err(CommandStoreError::InvalidRecord { field: "supersedes_command_id" })
        );
    }

    #[test]
    fn a_command_record_validates_state_and_the_superseded_link() {
        let record = CommandRecord::new(base_row(), COMMAND_STATE_PROPOSED.to_string(), None)
            .expect("proposed record builds");
        assert!(!record.is_superseded());
        let superseded = CommandRecord::new(
            base_row(),
            COMMAND_STATE_SUPERSEDED.to_string(),
            Some("0192a5e0-0000-7000-8000-0000000000a2".to_string()),
        )
        .expect("superseded record builds");
        assert!(superseded.is_superseded());
        // SUPERSEDED without the backward link (and the inverse) is malformed.
        assert_eq!(
            CommandRecord::new(base_row(), COMMAND_STATE_SUPERSEDED.to_string(), None),
            Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
        );
        assert_eq!(
            CommandRecord::new(
                base_row(),
                COMMAND_STATE_PROPOSED.to_string(),
                Some("0192a5e0-0000-7000-8000-0000000000a2".to_string()),
            ),
            Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
        );
        // A command cannot be its own replacement; states outside the closed
        // vocabulary are rejected.
        assert_eq!(
            CommandRecord::new(
                base_row(),
                COMMAND_STATE_SUPERSEDED.to_string(),
                Some(base_row().command_id.clone()),
            ),
            Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
        );
        assert_eq!(
            CommandRecord::new(base_row(), "SUPERCEDED".to_string(), None),
            Err(CommandStoreError::InvalidRecord { field: "state" })
        );
    }

    #[test]
    fn a_journal_record_validates_the_frozen_edge_and_the_reporting_rules() {
        let command = CommandId::new(
            FlexUuid::parse("0192a5e0-0000-7000-8000-0000000000a1").expect("uuid parses"),
        );
        // A legal gate pass carries gate + evidence.
        let mut gate_pass = transition(command, CommandState::Proposed, CommandState::Validated);
        gate_pass.gate = Some(Gate::Validation);
        gate_pass.evidence = Some(EvidenceReference::new("evidence:val-1").expect("evidence"));
        let record = CommandTransitionRecord::from_transition(tenant(), &gate_pass)
            .expect("gate pass records");
        assert_eq!(record.from_state, "PROPOSED");
        assert_eq!(record.to_state, "VALIDATED");
        assert_eq!(record.gate.as_deref(), Some("validation"));
        assert_eq!(record.evidence_reference.as_deref(), Some("evidence:val-1"));
        // A non-gate transition must NOT carry a gate.
        let mut issue = transition(command, CommandState::Approved, CommandState::Issued);
        issue.gate = Some(Gate::Validation);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &issue),
            Err(CommandStoreError::InvalidRecord { field: "gate" })
        );
        // An edge outside the frozen table is rejected before anything is
        // written (the domain table is the oracle).
        let skip = transition(command, CommandState::Proposed, CommandState::Approved);
        assert!(matches!(
            CommandTransitionRecord::from_transition(tenant(), &skip),
            Err(CommandStoreError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn journal_records_enforce_the_reason_evidence_and_gate_rules() {
        let command = CommandId::new(
            FlexUuid::parse("0192a5e0-0000-7000-8000-0000000000a1").expect("uuid parses"),
        );
        // REJECTED requires the gate AND the failing check (reason).
        let rejection = transition(command, CommandState::Proposed, CommandState::Rejected);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &rejection),
            Err(CommandStoreError::InvalidRecord { field: "gate" })
        );
        // FAILED requires a reason but no evidence.
        let failure = transition(command, CommandState::Issued, CommandState::Failed);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &failure),
            Err(CommandStoreError::InvalidRecord { field: "reason" })
        );
        // ISSUED requires evidence but no reason.
        let issue = transition(command, CommandState::Approved, CommandState::Issued);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &issue),
            Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
        );
        // COMPLETED requires evidence; EXPIRED requires the milestone reason.
        let completion = transition(command, CommandState::Tracking, CommandState::Completed);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &completion),
            Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
        );
        let expiry = transition(command, CommandState::Approved, CommandState::Expired);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &expiry),
            Err(CommandStoreError::InvalidRecord { field: "reason" })
        );
        // CANCELLED requires the reason; SUPERSEDED requires the reference.
        let cancel = transition(command, CommandState::Approved, CommandState::Cancelled);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &cancel),
            Err(CommandStoreError::InvalidRecord { field: "reason" })
        );
        let supersede = transition(command, CommandState::Validated, CommandState::Superseded);
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &supersede),
            Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
        );
        // A rejection MAY carry the failing-check evidence (ledger P4.11:
        // "REJECTED with the failing check and evidence") — and may also
        // omit it; both persist.
        let mut rejection_with_evidence =
            transition(command, CommandState::Proposed, CommandState::Rejected);
        rejection_with_evidence.gate = Some(Gate::Validation);
        rejection_with_evidence.reason = Some("units are kW, not MW".to_string());
        rejection_with_evidence.evidence =
            Some(EvidenceReference::new("evidence:val-1").expect("evidence"));
        let rejected = CommandTransitionRecord::from_transition(tenant(), &rejection_with_evidence)
            .expect("a rejection with evidence records");
        assert_eq!(rejected.evidence_reference.as_deref(), Some("evidence:val-1"));
        // FAILED carries neither evidence nor gate (the trigger reason only).
        let mut failure_with_evidence =
            transition(command, CommandState::Issued, CommandState::Failed);
        failure_with_evidence.reason = Some("NACK from gateway/adapter".to_string());
        failure_with_evidence.evidence =
            Some(EvidenceReference::new("evidence:x").expect("evidence"));
        assert_eq!(
            CommandTransitionRecord::from_transition(tenant(), &failure_with_evidence),
            Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
        );
    }

    #[test]
    fn the_trait_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn CommandRecordStore) {}
        let _ = takes_dyn_store as fn(&dyn CommandRecordStore);
    }
}
