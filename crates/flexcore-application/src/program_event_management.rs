// P16.15 — Program event management: the OpenADR event lifecycle linkage
//         to the command lifecycle, opt-in/out handling, and M&V report
//         delivery records ([S-116] OpenADR DR Program Guide; [S-117]
//         OpenLeadr reporting pattern).
//
// Phase 16 plan §2 row P16.15; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 43_p16_15_program_event_management.md`.
//
// WHAT THIS IS (decided once):
//
// - **An external event becomes PROPOSED commands per asset — NEVER an
//   issued command** (ADR-010; hard ban 11). This module consumes the
//   P14.05 OpenADR mapping (`crates/adapters/protocol-openadr/src/
//   event_mapping.rs`) BY CITATION: the adapter's
//   `ExternalCommandProposal` is the ONLY path an OpenADR event takes
//   into the command lifecycle, and its entry state is structurally
//   PROPOSED (`ExternalCommandProposal::entry_state`). This module
//   records the linkage (event ↔ command candidate ↔ asset) and
//   structurally mirrors that constraint: a linkage record's entry
//   state is a constant function returning
//   [`flexcore_domain::command_state::CommandState::Proposed`] — not a
//   field — so no code path here or in a caller can record a later
//   entry state from external input. The three gates (validation,
//   feasibility, authorization) remain the platform's, ahead of every
//   candidate.
// - **Opt-in is the default-absence rule, fail-closed**: an asset is
//   included in an event's command candidates ONLY with an active
//   opt-in whose effective window covers the event. No decision
//   recorded = NO candidate (an asset is never opted in by silence).
//   A recorded opt-out overrides an opt-in: the LATEST decision by
//   (recorded timestamp, decision id) wins, deterministically.
// - **Every refusal and every decision is a named record** — an
//   opted-out asset produces an `OptOutAppliedRecord`, never a silent
//   omission.
// - **Report delivery for M&V is a two-state closed record** ([S-116]
//   [S-117]): AWAITING_REPORT → REPORT_RECORDED, where the recorded
//   state carries the M&V report reference (the report itself is
//   P16.05's artifact — consumed by citation, never redefined). No
//   transmission happens in this module: a delivery is RECORDED by the
//   operator, never performed automatically.
// - **No market is connected, no event has ever arrived** (honesty
//   rules): the stores are empty; every fixture in the tests is
//   synthetic and carries no program meaning.
//
// Determinism: BTreeMap-backed stores, deterministic ordering, no
// wall-clock input.
//
// Control status: records for the simulation-only 0.x line. No event is
// received from any VTN, no command candidate is opened, no report is
// transmitted anywhere.

use std::collections::BTreeMap;

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::CommandId;
use flexcore_domain::market_program_model::{AuditChainLink, ParticipationWindow};

/// The program-event-management schema version (a change is a NEW
/// version, never a silent edit).
pub const PROGRAM_EVENT_MANAGEMENT_SCHEMA_VERSION: u32 = 1;

/// The PROPOSED-entry statement (pinned by tests; the ADR-010 rule).
pub const EXTERNAL_EVENTS_PROPOSED_ONLY_STATEMENT: &str = "An external program event \
     becomes PROPOSED commands per asset and NEVER an issued command: the only path into \
     the frozen command lifecycle is the P14.05 adapter mapping, whose entry state is \
     structurally PROPOSED, and this module's linkage records mirror that constraint — \
     the entry state is a constant, not a field. The validation, feasibility, and \
     authorization gates remain the platform's, ahead of every candidate (ADR-010).";

/// The opt-in default statement (pinned by tests).
pub const OPT_IN_IS_FAIL_CLOSED_STATEMENT: &str = "Opt-in is fail-closed: an asset is \
     included in an event's command candidates ONLY with an active opt-in whose \
     effective window covers the event; no decision recorded means NO candidate. A \
     recorded opt-out overrides an opt-in — the latest decision by (recorded timestamp, \
     decision id) wins — and every opt-out produces a named OptOutAppliedRecord, never \
     a silent omission.";

/// The no-event statement (pinned by tests).
pub const NO_EVENT_CONNECTED_STATEMENT: &str = "No market or program event has ever \
     arrived: no VTN is contacted, no event is received, no command candidate is opened, \
     and no report is transmitted anywhere in this program. The stores are empty; every \
     fixture value in the tests is synthetic.";

// --- Errors ---------------------------------------------------------------------

/// Why a program-event-management operation was refused. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramEventError {
    /// A required text field was empty (`field` names it).
    EmptyField {
        field: &'static str,
    },
    /// A text field exceeded its bound (`field` names it).
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
    /// A domain shared-type constructor refused (carried — the window
    /// and audit-link constructors).
    Domain(flexcore_domain::market_program_model::MarketProgramModelError),
    /// The event window does not cover the linkage interval (the
    /// candidate interval must fall inside the event window — an
    /// interval outside is refused, never clipped).
    IntervalOutsideEventWindow,
    /// No participant decision is recorded for the asset (fail-closed:
    /// no decision = no candidate).
    NoParticipantDecision {
        asset_id: String,
        event_id: String,
    },
    /// The latest recorded decision for the asset is an opt-out.
    OptOutActive {
        asset_id: String,
        decision_id: String,
    },
    /// An opt-out-applied record was offered for an asset whose latest
    /// decision is an opt-in — the record would misrecord reality and
    /// is refused.
    OptOutAppliedRealityMismatch {
        asset_id: String,
        decision_id: String,
    },
    /// A duplicate identity was registered.
    DuplicateIdentity {
        record_id: String,
    },
    /// The referenced event record was not found.
    EventNotFound {
        event_id: String,
    },
    /// The report transition is not in the closed two-state table.
    IllegalReportTransition {
        from: &'static str,
        to: &'static str,
    },
    /// A REPORT_RECORDED delivery without a report reference.
    ReportRefRequired,
}

impl ProgramEventError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ProgramEventError::EmptyField { .. } => "PROGRAM_EVENT_EMPTY_FIELD",
            ProgramEventError::FieldTooLong { .. } => "PROGRAM_EVENT_FIELD_TOO_LONG",
            ProgramEventError::Domain(_) => "PROGRAM_EVENT_DOMAIN_ERROR",
            ProgramEventError::IntervalOutsideEventWindow => {
                "PROGRAM_EVENT_INTERVAL_OUTSIDE_EVENT_WINDOW"
            }
            ProgramEventError::NoParticipantDecision { .. } => {
                "PROGRAM_EVENT_NO_PARTICIPANT_DECISION"
            }
            ProgramEventError::OptOutActive { .. } => "PROGRAM_EVENT_OPT_OUT_ACTIVE",
            ProgramEventError::OptOutAppliedRealityMismatch { .. } => {
                "PROGRAM_EVENT_OPT_OUT_APPLIED_REALITY_MISMATCH"
            }
            ProgramEventError::DuplicateIdentity { .. } => "PROGRAM_EVENT_DUPLICATE_IDENTITY",
            ProgramEventError::EventNotFound { .. } => "PROGRAM_EVENT_EVENT_NOT_FOUND",
            ProgramEventError::IllegalReportTransition { .. } => {
                "PROGRAM_EVENT_ILLEGAL_REPORT_TRANSITION"
            }
            ProgramEventError::ReportRefRequired => "PROGRAM_EVENT_REPORT_REF_REQUIRED",
        }
    }
}

impl std::fmt::Display for ProgramEventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramEventError::EmptyField { field } => {
                write!(f, "the program-event field '{field}' must be non-empty")
            }
            ProgramEventError::FieldTooLong { field, len, max } => write!(
                f,
                "the program-event field '{field}' is {len} characters; the bound is {max}"
            ),
            ProgramEventError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
            ProgramEventError::IntervalOutsideEventWindow => write!(
                f,
                "the candidate interval falls outside the event window; the linkage is \
                 refused, never clipped"
            ),
            ProgramEventError::NoParticipantDecision { asset_id, event_id } => write!(
                f,
                "no participant decision is recorded for asset {asset_id} under event \
                 {event_id}; opt-in is fail-closed — no decision means NO candidate"
            ),
            ProgramEventError::OptOutActive {
                asset_id,
                decision_id,
            } => write!(
                f,
                "asset {asset_id} is opted out of event coverage by decision \
                 {decision_id}; an OptOutAppliedRecord is written instead of a command \
                 candidate"
            ),
            ProgramEventError::OptOutAppliedRealityMismatch {
                asset_id,
                decision_id,
            } => write!(
                f,
                "the opt-out-applied record for asset {asset_id} names decision \
                 {decision_id}, which is an OPT_IN — the record would misrecord reality \
                 and is refused"
            ),
            ProgramEventError::DuplicateIdentity { record_id } => write!(
                f,
                "the program-event record identity {record_id} is already registered"
            ),
            ProgramEventError::EventNotFound { event_id } => {
                write!(f, "the program event {event_id} was not found")
            }
            ProgramEventError::IllegalReportTransition { from, to } => write!(
                f,
                "the M&V report-delivery transition {from} → {to} is not in the closed \
                 two-state table"
            ),
            ProgramEventError::ReportRefRequired => write!(
                f,
                "a REPORT_RECORDED delivery carries no M&V report reference; the \
                 recorded state requires the P16.05 report identity"
            ),
        }
    }
}

impl std::error::Error for ProgramEventError {}

// --- Shared helpers ---------------------------------------------------------------

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), ProgramEventError> {
    if value.trim().is_empty() {
        return Err(ProgramEventError::EmptyField { field });
    }
    if value.len() > max {
        return Err(ProgramEventError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

// --- The program-event record -------------------------------------------------------

/// One program-event record: an external program event as RECORDED data
/// (the event itself arrived through a protocol adapter — the OpenADR
/// adapter's `OpenAdrEvent` — and is recorded here under the
/// jurisdiction's declared event-kind label; ADR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEventRecord {
    event_id: String,
    tenant_id: String,
    program_id: String,
    /// The external event reference, verbatim (the adapter's source
    /// event id).
    external_ref: String,
    /// The event kind as DECLARED by the program (recorded data — no
    /// universal event taxonomy; ADR-011).
    event_kind: String,
    /// The event window (end-exclusive — the §10.3 convention; the
    /// domain window type reused).
    window: ParticipationWindow,
    /// Declared event data as recorded key/value pairs (BTreeMap —
    /// deterministic; never interpreted).
    declared_data: BTreeMap<String, String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl ProgramEventRecord {
    /// Builds and validates one event record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: String,
        tenant_id: String,
        program_id: String,
        external_ref: String,
        event_kind: String,
        window: ParticipationWindow,
        declared_data: BTreeMap<String, String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, ProgramEventError> {
        validate_text("event_id", &event_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("program_id", &program_id, 64)?;
        validate_text("external_ref", &external_ref, 256)?;
        validate_text("event_kind", &event_kind, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(ProgramEventError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            event_id,
            tenant_id,
            program_id,
            external_ref,
            event_kind,
            window,
            declared_data,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn external_ref(&self) -> &str {
        &self.external_ref
    }

    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    pub fn window(&self) -> ParticipationWindow {
        self.window
    }

    pub fn declared_data(&self) -> &BTreeMap<String, String> {
        &self.declared_data
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- Opt-in / opt-out ----------------------------------------------------------------

/// The closed participant-decision vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantDecision {
    /// The asset opts IN to event coverage.
    OptIn,
    /// The asset opts OUT of event coverage.
    OptOut,
}

impl ParticipantDecision {
    pub const fn name(self) -> &'static str {
        match self {
            ParticipantDecision::OptIn => "OPT_IN",
            ParticipantDecision::OptOut => "OPT_OUT",
        }
    }
}

/// The closed decision-scope vocabulary: a decision is either program
/// wide (covers every event of the program) or scoped to one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionScope {
    ProgramWide,
    EventScoped {
        event_id: String,
    },
}

/// One participant decision record (immutable; a change of mind is a
/// NEW decision — the latest by (recorded timestamp, decision id)
/// wins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantDecisionRecord {
    decision_id: String,
    tenant_id: String,
    program_id: String,
    asset_id: String,
    decision: ParticipantDecision,
    scope: DecisionScope,
    /// The effective window of the decision (end-exclusive — the §10.3
    /// convention; an OptIn outside its window is not active).
    effective_window: ParticipationWindow,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl ParticipantDecisionRecord {
    /// Builds and validates one decision record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        decision_id: String,
        tenant_id: String,
        program_id: String,
        asset_id: String,
        decision: ParticipantDecision,
        scope: DecisionScope,
        effective_window: ParticipationWindow,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, ProgramEventError> {
        validate_text("decision_id", &decision_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("program_id", &program_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(ProgramEventError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        if let DecisionScope::EventScoped { event_id } = &scope {
            validate_text("event_id", event_id, 64)?;
        }
        Ok(Self {
            decision_id,
            tenant_id,
            program_id,
            asset_id,
            decision,
            scope,
            effective_window,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn decision(&self) -> ParticipantDecision {
        self.decision
    }

    pub fn scope(&self) -> &DecisionScope {
        &self.scope
    }

    pub fn effective_window(&self) -> ParticipationWindow {
        self.effective_window
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- The PROPOSED-entry command linkage ------------------------------------------------

/// The closed linkage-record vocabulary: a linkage exists (the
/// candidate entered at PROPOSED) or was refused at the platform's
/// gates. NOTHING ELSE — the forward states are the protected gates',
/// never this module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandLinkageState {
    /// The adapter-mapped candidate was recorded as entering the
    /// command lifecycle at PROPOSED.
    ProposedEntered,
    /// The platform's gates rejected the candidate (the frozen
    /// PROPOSED → REJECTED edge — recorded as data; the rejection
    /// itself is the lifecycle's, never this module's).
    RejectedByGate,
}

impl CommandLinkageState {
    pub const fn name(self) -> &'static str {
        match self {
            CommandLinkageState::ProposedEntered => "PROPOSED_ENTERED",
            CommandLinkageState::RejectedByGate => "REJECTED_BY_GATE",
        }
    }
}

/// One event → command-candidate linkage per asset. This is the
/// module's ONLY output type for the event-to-command direction, and
/// its entry state is STRUCTURALLY PROPOSED (see
/// [`ProposedCommandLinkage::entry_state`]): the state is a constant
/// function, not a field, so no code path can record a later entry
/// state from external input (the P14.05 `ExternalCommandProposal`
/// pattern, mirrored — the adapter mapping is consumed by citation;
/// this record stores the candidate's identity and provenance).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedCommandLinkage {
    linkage_id: String,
    tenant_id: String,
    event_id: String,
    asset_id: String,
    /// The command id the application layer assigned to the
    /// adapter-mapped candidate (the lifecycle record is opened by the
    /// application/kernel side via `CommandLifecycle::propose` — never
    /// here).
    command_id: CommandId,
    /// The verbatim external event reference (audit trace: linkage ←
    /// adapter proposal ← event).
    source_event_ref: String,
    /// The adapter id that produced the candidate (the §14.3 source
    /// subsystem value recorded verbatim).
    source_subsystem: String,
    /// The interval of the candidate this linkage covers (inside the
    /// event window — enforced at construction).
    interval_window: ParticipationWindow,
    state: CommandLinkageState,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl ProposedCommandLinkage {
    /// The lifecycle state this linkage records the candidate as
    /// entering at. ALWAYS [`CommandState::Proposed`] — structurally
    /// constant, not a field.
    pub const fn entry_state() -> CommandState {
        CommandState::Proposed
    }

    /// The lifecycle states this module may record, and nothing else:
    /// the PROPOSED entry and the gate rejection. Every forward state
    /// is produced by the protected gates — a linkage record
    /// constructing one is a defect (the P14.02 rule, shared).
    pub const LINKAGE_CONSTRUCTIBLE_STATES: [CommandLinkageState; 2] = [
        CommandLinkageState::ProposedEntered,
        CommandLinkageState::RejectedByGate,
    ];

    /// Builds one linkage record: the candidate interval must fall
    /// INSIDE the event window (refused, never clipped).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        linkage_id: String,
        tenant_id: String,
        event: &ProgramEventRecord,
        asset_id: String,
        command_id: CommandId,
        source_subsystem: String,
        interval_window: ParticipationWindow,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, ProgramEventError> {
        validate_text("linkage_id", &linkage_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("source_subsystem", &source_subsystem, 128)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(ProgramEventError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        // The candidate interval must sit inside the event window
        // (end-exclusive both — the §10.3 convention).
        if interval_window.start_millis() < event.window().start_millis()
            || interval_window.end_millis() > event.window().end_millis()
        {
            return Err(ProgramEventError::IntervalOutsideEventWindow);
        }
        Ok(Self {
            linkage_id,
            tenant_id,
            event_id: event.event_id().to_string(),
            asset_id,
            command_id,
            source_event_ref: event.external_ref().to_string(),
            source_subsystem,
            interval_window,
            state: CommandLinkageState::ProposedEntered,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    /// Records a gate rejection against this linkage (the frozen
    /// PROPOSED → REJECTED edge, recorded as data; the rejection
    /// decision is the lifecycle's own).
    pub fn record_gate_rejection(&mut self) {
        self.state = CommandLinkageState::RejectedByGate;
    }

    pub fn linkage_id(&self) -> &str {
        &self.linkage_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn command_id(&self) -> &CommandId {
        &self.command_id
    }

    pub fn source_event_ref(&self) -> &str {
        &self.source_event_ref
    }

    pub fn source_subsystem(&self) -> &str {
        &self.source_subsystem
    }

    pub fn interval_window(&self) -> ParticipationWindow {
        self.interval_window
    }

    pub fn state(&self) -> CommandLinkageState {
        self.state
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- The opt-out-applied record ---------------------------------------------------------

/// One record that an opt-out was APPLIED for an asset under an event
/// (the named counterpart of a command linkage — an opted-out asset is
/// never silently omitted from the event's record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptOutAppliedRecord {
    applied_id: String,
    tenant_id: String,
    event_id: String,
    asset_id: String,
    /// The opt-out decision that was applied.
    decision_id: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl OptOutAppliedRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        applied_id: String,
        tenant_id: String,
        event_id: String,
        asset_id: String,
        decision_id: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, ProgramEventError> {
        validate_text("applied_id", &applied_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("event_id", &event_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("decision_id", &decision_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(ProgramEventError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            applied_id,
            tenant_id,
            event_id,
            asset_id,
            decision_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn applied_id(&self) -> &str {
        &self.applied_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- M&V report delivery ---------------------------------------------------------------

/// The closed M&V report-delivery vocabulary ([S-116][S-117]): a
/// delivery awaits the M&V report, or the report was recorded against
/// it. No TRANSMITTED/ACKNOWLEDGED state exists — transmission is not
/// performed by this module (the operator records the delivery).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MvDeliveryState {
    AwaitingReport,
    ReportRecorded,
}

impl MvDeliveryState {
    pub const fn name(self) -> &'static str {
        match self {
            MvDeliveryState::AwaitingReport => "AWAITING_REPORT",
            MvDeliveryState::ReportRecorded => "REPORT_RECORDED",
        }
    }

    /// The closed two-state transition table.
    pub fn can_transition_to(self, to: MvDeliveryState) -> bool {
        matches!(
            (self, to),
            (MvDeliveryState::AwaitingReport, MvDeliveryState::ReportRecorded)
        )
    }
}

/// One M&V report-delivery record per asset/event: the delivery
/// obligation for the program's reporting requirement ([S-116]), with
/// the recorded M&V report reference when the report exists (the
/// report itself is P16.05's artifact — consumed by citation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MvReportDeliveryRecord {
    delivery_id: String,
    tenant_id: String,
    event_id: String,
    asset_id: String,
    state: MvDeliveryState,
    /// The P16.05 M&V report reference (required exactly when the
    /// state is REPORT_RECORDED).
    mv_report_ref: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl MvReportDeliveryRecord {
    /// Builds one delivery record at AWAITING_REPORT.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        delivery_id: String,
        tenant_id: String,
        event_id: String,
        asset_id: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, ProgramEventError> {
        validate_text("delivery_id", &delivery_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("event_id", &event_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(ProgramEventError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            delivery_id,
            tenant_id,
            event_id,
            asset_id,
            state: MvDeliveryState::AwaitingReport,
            mv_report_ref: None,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    /// Records the M&V report against the delivery (the closed
    /// transition; the report reference is mandatory).
    pub fn record_report(&mut self, mv_report_ref: String) -> Result<(), ProgramEventError> {
        if !self.state.can_transition_to(MvDeliveryState::ReportRecorded) {
            return Err(ProgramEventError::IllegalReportTransition {
                from: self.state.name(),
                to: MvDeliveryState::ReportRecorded.name(),
            });
        }
        if mv_report_ref.trim().is_empty() {
            return Err(ProgramEventError::ReportRefRequired);
        }
        self.mv_report_ref = Some(mv_report_ref);
        self.state = MvDeliveryState::ReportRecorded;
        Ok(())
    }

    pub fn delivery_id(&self) -> &str {
        &self.delivery_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn state(&self) -> MvDeliveryState {
        self.state
    }

    pub fn mv_report_ref(&self) -> Option<&str> {
        self.mv_report_ref.as_deref()
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- The event-management store ----------------------------------------------------------

/// The BTreeMap-backed program-event store: events, participant
/// decisions, command linkages, opt-out-applied records, and M&V
/// report-delivery records — register-only, deterministic, no
/// wall-clock input. The decision resolution (opt-in gating) runs over
/// the recorded decisions with the exact (recorded timestamp, decision
/// id) ordering.
#[derive(Debug, Default)]
pub struct ProgramEventStore {
    events: BTreeMap<(String, String), ProgramEventRecord>,
    decisions: BTreeMap<(String, String), ParticipantDecisionRecord>,
    linkages: BTreeMap<(String, String), ProposedCommandLinkage>,
    opt_outs_applied: BTreeMap<(String, String), OptOutAppliedRecord>,
    mv_deliveries: BTreeMap<(String, String), MvReportDeliveryRecord>,
}

impl ProgramEventStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one event record.
    pub fn register_event(&mut self, event: ProgramEventRecord) -> Result<(), ProgramEventError> {
        let key = (event.tenant_id().to_string(), event.event_id().to_string());
        if self.events.contains_key(&key) {
            return Err(ProgramEventError::DuplicateIdentity {
                record_id: event.event_id().to_string(),
            });
        }
        self.events.insert(key, event);
        Ok(())
    }

    /// The event record lookup.
    pub fn event(&self, tenant_id: &str, event_id: &str) -> Option<&ProgramEventRecord> {
        self.events
            .get(&(tenant_id.to_string(), event_id.to_string()))
    }

    /// Registers one participant decision.
    pub fn register_decision(
        &mut self,
        decision: ParticipantDecisionRecord,
    ) -> Result<(), ProgramEventError> {
        let key = (
            decision.tenant_id().to_string(),
            decision.decision_id().to_string(),
        );
        if self.decisions.contains_key(&key) {
            return Err(ProgramEventError::DuplicateIdentity {
                record_id: decision.decision_id().to_string(),
            });
        }
        self.decisions.insert(key, decision);
        Ok(())
    }

    /// Resolves the LATEST decision for one asset under one event: the
    /// candidates are the decisions whose scope covers the event
    /// (program-wide or event-scoped to it) AND whose effective window
    /// contains the event's start. The winner is the maximum by
    /// (recorded timestamp, decision id) — deterministic. None means
    /// no decision covers the event (fail-closed).
    pub fn latest_decision_for(
        &self,
        tenant_id: &str,
        event: &ProgramEventRecord,
        asset_id: &str,
    ) -> Option<&ParticipantDecisionRecord> {
        self.decisions
            .values()
            .filter(|decision| {
                decision.tenant_id() == tenant_id
                    && decision.asset_id() == asset_id
                    && decision.program_id() == event.program_id()
                    && match decision.scope() {
                        DecisionScope::ProgramWide => true,
                        DecisionScope::EventScoped {
                            event_id: scoped,
                        } => scoped == event.event_id(),
                    }
                    && decision.effective_window().contains(event.window().start_millis())
            })
            .max_by(|a, b| {
                (a.recorded_at_unix_ms(), a.decision_id())
                    .cmp(&(b.recorded_at_unix_ms(), b.decision_id()))
            })
    }

    /// Registers one command linkage (the opt-in gate runs FIRST: the
    /// asset must have an active opt-in covering the event, and an
    /// opt-out wins over an opt-in).
    pub fn register_linkage(
        &mut self,
        linkage: ProposedCommandLinkage,
    ) -> Result<(), ProgramEventError> {
        let key = (
            linkage.tenant_id().to_string(),
            linkage.linkage_id().to_string(),
        );
        if self.linkages.contains_key(&key) {
            return Err(ProgramEventError::DuplicateIdentity {
                record_id: linkage.linkage_id().to_string(),
            });
        }
        let event = self
            .event(&linkage.tenant_id(), linkage.event_id())
            .ok_or_else(|| ProgramEventError::EventNotFound {
                event_id: linkage.event_id().to_string(),
            })?
            .clone();
        match self.latest_decision_for(&linkage.tenant_id(), &event, linkage.asset_id()) {
            None => {
                return Err(ProgramEventError::NoParticipantDecision {
                    asset_id: linkage.asset_id().to_string(),
                    event_id: event.event_id().to_string(),
                })
            }
            Some(decision) => {
                if decision.decision() == ParticipantDecision::OptOut {
                    return Err(ProgramEventError::OptOutActive {
                        asset_id: linkage.asset_id().to_string(),
                        decision_id: decision.decision_id().to_string(),
                    });
                }
            }
        }
        self.linkages.insert(key, linkage);
        Ok(())
    }

    /// Records an opt-out-applied record for an asset under an event
    /// (the named counterpart of a linkage; refuses when the asset is
    /// NOT actually opted out — the record must reflect reality).
    pub fn register_opt_out_applied(
        &mut self,
        applied: OptOutAppliedRecord,
    ) -> Result<(), ProgramEventError> {
        let key = (
            applied.tenant_id().to_string(),
            applied.applied_id().to_string(),
        );
        if self.opt_outs_applied.contains_key(&key) {
            return Err(ProgramEventError::DuplicateIdentity {
                record_id: applied.applied_id().to_string(),
            });
        }
        let event = self
            .event(&applied.tenant_id(), &applied.event_id)
            .ok_or_else(|| ProgramEventError::EventNotFound {
                event_id: applied.event_id.clone(),
            })?
            .clone();
        match self.latest_decision_for(&applied.tenant_id(), &event, &applied.asset_id) {
            None => {
                return Err(ProgramEventError::NoParticipantDecision {
                    asset_id: applied.asset_id.clone(),
                    event_id: event.event_id().to_string(),
                })
            }
            Some(decision) => {
                if decision.decision() != ParticipantDecision::OptOut {
                    return Err(ProgramEventError::OptOutAppliedRealityMismatch {
                        asset_id: applied.asset_id.clone(),
                        decision_id: decision.decision_id().to_string(),
                    });
                }
            }
        }
        self.opt_outs_applied.insert(key, applied);
        Ok(())
    }

    /// Registers one M&V report-delivery record.
    pub fn register_mv_delivery(
        &mut self,
        delivery: MvReportDeliveryRecord,
    ) -> Result<(), ProgramEventError> {
        let key = (
            delivery.tenant_id().to_string(),
            delivery.delivery_id().to_string(),
        );
        if self.mv_deliveries.contains_key(&key) {
            return Err(ProgramEventError::DuplicateIdentity {
                record_id: delivery.delivery_id().to_string(),
            });
        }
        self.mv_deliveries.insert(key, delivery);
        Ok(())
    }

    /// The mutable M&V delivery accessor for the record-report
    /// transition.
    pub fn mv_delivery_mut(
        &mut self,
        tenant_id: &str,
        delivery_id: &str,
    ) -> Option<&mut MvReportDeliveryRecord> {
        self.mv_deliveries
            .get_mut(&(tenant_id.to_string(), delivery_id.to_string()))
    }

    /// The command linkages recorded for one event (ascending linkage
    /// id — deterministic).
    pub fn linkages_for_event(&self, tenant_id: &str, event_id: &str) -> Vec<&ProposedCommandLinkage> {
        self.linkages
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .filter(|(_, linkage)| linkage.event_id() == event_id)
            .map(|(_, linkage)| linkage)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture value is synthetic
// and carries no program meaning.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    fn command_id(tag: u8) -> CommandId {
        CommandId::new(
            FlexUuid::parse(&format!("00000000-0000-0000-0000-0000000000{tag:02x}"))
                .expect("valid uuid"),
        )
    }

    fn event() -> ProgramEventRecord {
        let window =
            ParticipationWindow::new(1_700_000_000_000, 1_700_000_900_000).expect("window");
        ProgramEventRecord::new(
            "event-1".to_string(),
            "tenant-1".to_string(),
            "program-1".to_string(),
            "evt-1".to_string(),
            "DR_SHED".to_string(),
            window,
            BTreeMap::new(),
            link(),
            1_699_999_000_000,
            "recorder".to_string(),
        )
        .expect("event")
    }

    fn decision(id: &str, asset: &str, decision: ParticipantDecision, at: i64)
        -> ParticipantDecisionRecord
    {
        let window =
            ParticipationWindow::new(1_690_000_000_000, 1_800_000_000_000).expect("window");
        ParticipantDecisionRecord::new(
            id.to_string(),
            "tenant-1".to_string(),
            "program-1".to_string(),
            asset.to_string(),
            decision,
            DecisionScope::ProgramWide,
            window,
            link(),
            at,
            "recorder".to_string(),
        )
        .expect("decision")
    }

    fn linkage(event: &ProgramEventRecord, tag: u8) -> ProposedCommandLinkage {
        let interval = ParticipationWindow::new(
            event.window().start_millis(),
            event.window().start_millis() + 900_000,
        )
        .expect("interval");
        ProposedCommandLinkage::new(
            format!("linkage-{tag}"),
            "tenant-1".to_string(),
            event,
            "asset-1".to_string(),
            command_id(tag),
            "openadr-vtn".to_string(),
            interval,
            link(),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("linkage")
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(EXTERNAL_EVENTS_PROPOSED_ONLY_STATEMENT.contains("NEVER an issued command"));
        assert!(EXTERNAL_EVENTS_PROPOSED_ONLY_STATEMENT.contains("structurally PROPOSED"));
        assert!(OPT_IN_IS_FAIL_CLOSED_STATEMENT.contains("no decision means NO candidate"));
        assert!(NO_EVENT_CONNECTED_STATEMENT.contains("No market or program event has ever"));
    }

    #[test]
    fn the_entry_state_is_structurally_proposed_and_no_shortcut_exists() {
        // The linkage entry state is PROPOSED, structurally.
        assert_eq!(ProposedCommandLinkage::entry_state(), CommandState::Proposed);
        // The frozen machine has no PROPOSED → later shortcut: every
        // later state is unreachable from PROPOSED in one step except
        // through the gates (the P14.05 rule, mirrored).
        for later in [
            CommandState::Feasible,
            CommandState::Approved,
            CommandState::Issued,
            CommandState::Acknowledged,
            CommandState::Tracking,
            CommandState::Completed,
        ] {
            assert!(
                flexcore_domain::command_state::try_transition(CommandState::Proposed, later)
                    .is_err(),
                "no shortcut PROPOSED -> {} exists",
                later.name()
            );
        }
        // The linkage-constructible set is exactly {PROPOSED_ENTERED,
        // REJECTED_BY_GATE}.
        assert_eq!(
            ProposedCommandLinkage::LINKAGE_CONSTRUCTIBLE_STATES
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>(),
            vec!["PROPOSED_ENTERED", "REJECTED_BY_GATE"]
        );
    }

    #[test]
    fn an_interval_outside_the_event_window_is_refused_never_clipped() {
        let event = event();
        let outside = ParticipationWindow::new(
            event.window().end_millis(),
            event.window().end_millis() + 900_000,
        )
        .expect("outside window");
        let err = ProposedCommandLinkage::new(
            "linkage-x".to_string(),
            "tenant-1".to_string(),
            &event,
            "asset-1".to_string(),
            command_id(1),
            "openadr-vtn".to_string(),
            outside,
            link(),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect_err("outside refused");
        assert_eq!(err.name(), "PROGRAM_EVENT_INTERVAL_OUTSIDE_EVENT_WINDOW");
    }

    #[test]
    fn no_decision_means_no_candidate_fail_closed() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("event");
        let result = store.register_linkage(linkage(&event, 1));
        assert_eq!(
            result.expect_err("fail-closed").name(),
            "PROGRAM_EVENT_NO_PARTICIPANT_DECISION"
        );
    }

    #[test]
    fn an_active_opt_in_admits_the_candidate() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("event");
        store
            .register_decision(decision(
                "dec-1",
                "asset-1",
                ParticipantDecision::OptIn,
                1_699_998_000_000,
            ))
            .expect("decision");
        store.register_linkage(linkage(&event, 1)).expect("linkage");
        assert_eq!(store.linkages_for_event("tenant-1", "event-1").len(), 1);
        let stored = store.linkages_for_event("tenant-1", "event-1")[0];
        assert_eq!(stored.state(), CommandLinkageState::ProposedEntered);
        assert_eq!(stored.source_event_ref(), "evt-1");
        assert_eq!(stored.source_subsystem(), "openadr-vtn");
    }

    #[test]
    fn the_latest_opt_out_wins_over_an_earlier_opt_in() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("event");
        store
            .register_decision(decision(
                "dec-in",
                "asset-1",
                ParticipantDecision::OptIn,
                1_699_998_000_000,
            ))
            .expect("opt-in");
        store
            .register_decision(decision(
                "dec-out",
                "asset-1",
                ParticipantDecision::OptOut,
                1_699_999_000_000,
            ))
            .expect("opt-out");
        assert_eq!(
            store
                .register_linkage(linkage(&event, 1))
                .expect_err("opt-out wins")
                .name(),
            "PROGRAM_EVENT_OPT_OUT_ACTIVE"
        );
        // The opt-out is APPLIED as a named record.
        let applied = OptOutAppliedRecord::new(
            "applied-1".to_string(),
            "tenant-1".to_string(),
            "event-1".to_string(),
            "asset-1".to_string(),
            "dec-out".to_string(),
            link(),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("applied record");
        store.register_opt_out_applied(applied).expect("recorded");
        // A later opt-in (newer timestamp) re-admits the asset.
        store
            .register_decision(decision(
                "dec-in-2",
                "asset-1",
                ParticipantDecision::OptIn,
                1_700_000_200_000,
            ))
            .expect("later opt-in");
        store.register_linkage(linkage(&event, 2)).expect("linkage");
        assert_eq!(store.linkages_for_event("tenant-1", "event-1").len(), 1);
        // An opt-out-applied record for a NON-opted-out asset is
        // refused (it would misrecord reality).
        let wrong = OptOutAppliedRecord::new(
            "applied-2".to_string(),
            "tenant-1".to_string(),
            "event-1".to_string(),
            "asset-1".to_string(),
            "dec-in-2".to_string(),
            link(),
            1_700_000_300_000,
            "recorder".to_string(),
        )
        .expect("wrong record");
        assert_eq!(
            store
                .register_opt_out_applied(wrong)
                .expect_err("reality mismatch refused")
                .name(),
            "PROGRAM_EVENT_OPT_OUT_APPLIED_REALITY_MISMATCH"
        );
    }

    #[test]
    fn an_expired_opt_in_window_does_not_admit_the_candidate() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("event");
        // A decision whose effective window ended BEFORE the event.
        let expired_window = ParticipationWindow::new(
            1_690_000_000_000,
            event.window().start_millis(),
        )
        .expect("expired window");
        let decision = ParticipantDecisionRecord::new(
            "dec-expired".to_string(),
            "tenant-1".to_string(),
            "program-1".to_string(),
            "asset-1".to_string(),
            ParticipantDecision::OptIn,
            DecisionScope::ProgramWide,
            expired_window,
            link(),
            1_699_998_000_000,
            "recorder".to_string(),
        )
        .expect("decision");
        store.register_decision(decision).expect("decision");
        assert_eq!(
            store
                .register_linkage(linkage(&event, 1))
                .expect_err("expired opt-in")
                .name(),
            "PROGRAM_EVENT_NO_PARTICIPANT_DECISION"
        );
    }

    #[test]
    fn a_gate_rejection_is_recorded_as_data_never_as_this_module_s_decision() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("event");
        store
            .register_decision(decision(
                "dec-1",
                "asset-1",
                ParticipantDecision::OptIn,
                1_699_998_000_000,
            ))
            .expect("decision");
        let mut linkage = linkage(&event, 3);
        linkage.record_gate_rejection();
        store.register_linkage(linkage.clone()).expect("rejected linkage stored");
        let stored = store.linkages_for_event("tenant-1", "event-1")[0];
        assert_eq!(stored.state(), CommandLinkageState::RejectedByGate);
        // The rejection state is the ONLY non-entry state the module
        // can record — the forward states are the gates'.
        assert_ne!(stored.state(), CommandLinkageState::ProposedEntered);
    }

    #[test]
    fn mv_report_delivery_is_a_closed_two_state_record() {
        let mut store = ProgramEventStore::new();
        let delivery = MvReportDeliveryRecord::new(
            "mv-1".to_string(),
            "tenant-1".to_string(),
            "event-1".to_string(),
            "asset-1".to_string(),
            link(),
            1_700_000_400_000,
            "recorder".to_string(),
        )
        .expect("delivery");
        store.register_mv_delivery(delivery).expect("registered");
        assert_eq!(
            store
                .mv_delivery_mut("tenant-1", "mv-1")
                .expect("found")
                .record_report(String::new())
                .expect_err("empty report ref"),
            ProgramEventError::ReportRefRequired
        );
        let delivery_mut = store.mv_delivery_mut("tenant-1", "mv-1").expect("found");
        delivery_mut
            .record_report("mv-report-p16-05-ref".to_string())
            .expect("recorded");
        assert_eq!(delivery_mut.state(), MvDeliveryState::ReportRecorded);
        assert_eq!(delivery_mut.mv_report_ref(), Some("mv-report-p16-05-ref"));
        // REPORT_RECORDED is terminal.
        assert_eq!(
            delivery_mut
                .record_report("another".to_string())
                .expect_err("terminal")
                .name(),
            "PROGRAM_EVENT_ILLEGAL_REPORT_TRANSITION"
        );
        // The vocabulary carries no TRANSMITTED/ACKNOWLEDGED state.
        assert!(MvDeliveryState::AwaitingReport.name() != "TRANSMITTED");
        assert!(MvDeliveryState::ReportRecorded.name() != "TRANSMITTED");
    }

    #[test]
    fn duplicate_event_identities_are_refused() {
        let mut store = ProgramEventStore::new();
        let event = event();
        store.register_event(event.clone()).expect("registered");
        assert_eq!(
            store
                .register_event(event)
                .expect_err("duplicate")
                .name(),
            "PROGRAM_EVENT_DUPLICATE_IDENTITY"
        );
    }
}
