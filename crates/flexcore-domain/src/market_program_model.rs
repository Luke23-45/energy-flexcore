// P16.02 — The market program/enrollment domain model: Program, Contract,
//         Enrollment, QualificationEvidence, and per-asset
//         ParticipationWindow types with lifecycle states (the GE Program
//         Manager analog [S-41]; the Uplight program-management pattern
//         [S-43]; the CPower curtailment-plan pattern [S-48]).
//
// Phase 16 plan §2 row P16.02; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 02_p16_02_program_enrollment_domain_model.md`.
//
// THE BOUNDARY, DECIDED ONCE:
//
// - **The program's rules are DATA, never FlexCore's decisions.** The
//   baseline-method declaration, the delivery windows, the accepted
//   evidence kinds — every program rule enters as declared configuration
//   recorded ON the program (LR-073: FlexCore never chooses the baseline
//   method; the declaration here is the typed carrier of the program's
//   choice). An empty or absent declaration is recorded as absent — it
//   is never defaulted.
// - **Records are immutable; state changes are new superseding records.**
//   A lifecycle transition produces a NEW record superseding the old one
//   (the P16.01 store's supersede model mirrored in the domain). The
//   closed transition tables below are the domain's oracle; nothing
//   outside them is representable.
// - **Participation windows are end-exclusive** (the §10.3 convention)
//   and a missing window is recorded as missing — never widened to
//   "always".
// - **Stacking rules are recorded per program pair, and the enrollment
//   model REFUSES a combination that violates a recorded rule** (plan
//   risk R-P16-7: multi-market enrollment and dual-participation
//   stacking create double-counting hazards). A refusal is named, never
//   absorbed.
// - **Every financial record carries its P11 audit-chain LINK** (the
//   `audit_record_id` of the P11 audit record appended for the write);
//   the chain treatment itself is the P11 chain's, consumed, never
//   redefined (see the P16.01 store).
// - **No market is connected, no enrollment is filed, no program
//   participation is claimed anywhere in this program** (ADR-011;
//   conventions §7.3). This module is a vocabulary and an in-memory
//   record model; it files nothing and contacts nothing.
//
// Determinism: stores are BTreeMap-backed with deterministic ordering
// and no wall-clock input — time enters only as caller-supplied recorded
// timestamps (the kernel pattern; the P16.04 engine injects the Clock
// where "now" is needed).
//
// Control status: records for the simulation-only 0.x line. No I/O, no
// filing path, no field-write path, and nothing here implies market
// participation capability.

use std::collections::BTreeMap;

/// The market-record schema version (a change is a NEW version, never a
/// silent edit).
pub const MARKET_PROGRAM_MODEL_SCHEMA_VERSION: u32 = 1;

/// The program-rules-are-data statement (LR-073; pinned by tests).
pub const PROGRAM_RULES_ARE_DATA_STATEMENT: &str = "The program's rules are DATA, never \
     FlexCore's decisions: the baseline-method declaration, delivery windows, and evidence \
     kinds enter as declared configuration recorded ON the program (LR-073 — FlexCore never \
     chooses the baseline method). An absent declaration is recorded as absent, never \
     defaulted.";

/// The immutability statement (pinned by tests).
pub const MARKET_RECORDS_IMMUTABLE_STATEMENT: &str = "A market record is immutable: a \
     lifecycle transition or a correction is a NEW record superseding the old one. Nothing \
     is edited in place.";

/// The no-participation statement (pinned by tests).
pub const NO_MARKET_PARTICIPATION_STATEMENT: &str = "No market is connected, no enrollment \
     is filed, and no program participation is claimed anywhere in this program (ADR-011; \
     program conventions §7.3). This module records and validates — it files nothing.";

// --- Errors -------------------------------------------------------------------

/// Why a market-program-model operation was refused. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketProgramModelError {
    /// A required text field was empty (`field` names it).
    EmptyField {
        field: &'static str,
    },
    /// A timestamp field carried a negative value — not an empty field, a
    /// negative one (named separately: FRV1-R7-007).
    NegativeTimestamp {
        field: &'static str,
    },
    /// A text field exceeded its bound (`field` names it).
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
    /// A participation window was inverted (end not strictly after
    /// start, end-exclusive semantics).
    InvertedWindow,
    /// The record this transition would apply to was not found.
    RecordNotFound {
        record_id: String,
    },
    /// The transition is not in the closed transition table for this
    /// record class (`from`/`to` name the states).
    IllegalTransition {
        from: String,
        to: String,
    },
    /// The supersede target does not belong to the same tenant as the
    /// new record (tenant scoping, P0.16 rule 5).
    SupersedeTenantMismatch,
    /// The record is already superseded — a transition on a superseded
    /// record is refused (there is exactly one current version).
    RecordSuperseded {
        record_id: String,
    },
    /// A duplicate identity was registered (the identity is already
    /// present as a current record).
    DuplicateIdentity {
        record_id: String,
    },
    /// The enrollment would violate a recorded stacking rule (plan risk
    /// R-P16-7 — the model refuses, never allows silently).
    StackingRuleViolation {
        asset_id: String,
        rule_id: String,
        conflicting_program_id: String,
    },
    /// A qualification-evidence list on a terminal-state enrollment is
    /// not recordable (terminal states carry no new evidence).
    TerminalStateEvidence,
}

impl std::fmt::Display for MarketProgramModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketProgramModelError::EmptyField { field } => {
                write!(f, "the market-record field '{field}' must be non-empty")
            }
            MarketProgramModelError::NegativeTimestamp { field } => {
                write!(f, "the market-record field '{field}' must be non-negative")
            }
            MarketProgramModelError::FieldTooLong { field, len, max } => write!(
                f,
                "the market-record field '{field}' is {len} characters; the bound is {max}"
            ),
            MarketProgramModelError::InvertedWindow => write!(
                f,
                "the participation window is inverted: the end must be strictly after the \
                 start (end-exclusive, §10.3)"
            ),
            MarketProgramModelError::RecordNotFound { record_id } => {
                write!(f, "the market record {record_id} was not found")
            }
            MarketProgramModelError::IllegalTransition { from, to } => write!(
                f,
                "the market-record transition {from} → {to} is not in the closed transition \
                 table for this record class"
            ),
            MarketProgramModelError::SupersedeTenantMismatch => write!(
                f,
                "the supersede target belongs to a different tenant (tenant scoping, P0.16 \
                 rule 5)"
            ),
            MarketProgramModelError::RecordSuperseded { record_id } => write!(
                f,
                "the market record {record_id} is already superseded; there is exactly one \
                 current version and transitions apply to it only"
            ),
            MarketProgramModelError::DuplicateIdentity { record_id } => write!(
                f,
                "the market-record identity {record_id} is already registered as a current \
                 record"
            ),
            MarketProgramModelError::StackingRuleViolation {
                asset_id,
                rule_id,
                conflicting_program_id,
            } => write!(
                f,
                "the enrollment of asset {asset_id} would violate the recorded stacking rule \
                 {rule_id} (conflicting program {conflicting_program_id}); the model refuses \
                 the combination rather than allow a double-counting hazard"
            ),
            MarketProgramModelError::TerminalStateEvidence => write!(
                f,
                "a terminal-state enrollment carries no new qualification evidence"
            ),
        }
    }
}

impl std::error::Error for MarketProgramModelError {}

// --- Shared value types ---------------------------------------------------------

/// The P11 audit-chain LINK carried by every financial record: the
/// canonical UUID text of the P11 `audit_records` row appended for the
/// write (the chain treatment is the P11 chain's, consumed, never
/// redefined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChainLink {
    audit_record_id: String,
}

impl AuditChainLink {
    /// Builds the link (non-empty; UUID-text validation is the
    /// persistence adapter's — the domain records the reference).
    pub fn new(audit_record_id: String) -> Result<Self, MarketProgramModelError> {
        if audit_record_id.trim().is_empty() {
            return Err(MarketProgramModelError::EmptyField {
                field: "audit_record_id",
            });
        }
        Ok(Self { audit_record_id })
    }

    /// The linked P11 audit-record identity (canonical UUID text).
    pub fn audit_record_id(&self) -> &str {
        &self.audit_record_id
    }
}

/// The per-asset participation window: END-EXCLUSIVE (§10.3), bounded on
/// both ends, never inverted. `None` windows are recorded as missing —
/// never widened to "always".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipationWindow {
    start_millis: i64,
    end_millis: i64,
}

impl ParticipationWindow {
    /// Builds a window (non-negative start; end strictly after start).
    pub fn new(start_millis: i64, end_millis: i64) -> Result<Self, MarketProgramModelError> {
        if start_millis < 0 || end_millis <= start_millis {
            return Err(MarketProgramModelError::InvertedWindow);
        }
        Ok(Self {
            start_millis,
            end_millis,
        })
    }

    /// The window start (unix millis, inclusive).
    pub fn start_millis(&self) -> i64 {
        self.start_millis
    }

    /// The window end (unix millis, END-EXCLUSIVE).
    pub fn end_millis(&self) -> i64 {
        self.end_millis
    }

    /// Whether a timestamp falls inside the window (start inclusive,
    /// end exclusive — `at == end` is OUTSIDE, the §10.3 convention).
    pub fn contains(&self, at_millis: i64) -> bool {
        at_millis >= self.start_millis && at_millis < self.end_millis
    }
}

/// One qualification-evidence reference on an enrollment (the evidence
/// ITEM itself lives where it was produced — this is the recorded
/// reference; the GE Program Manager analog keeps the evidence pack with
/// the enrollment [S-41], and this type is the reference slot for it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationEvidence {
    evidence_ref: String,
    /// The evidence kind as DECLARED by the program (recorded data —
    /// no universal evidence taxonomy is invented).
    evidence_kind: String,
    recorded_at_unix_ms: i64,
}

impl QualificationEvidence {
    /// Builds one evidence reference (non-empty ref and kind;
    /// non-negative timestamp).
    pub fn new(
        evidence_ref: String,
        evidence_kind: String,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, MarketProgramModelError> {
        if evidence_ref.trim().is_empty() {
            return Err(MarketProgramModelError::EmptyField {
                field: "evidence_ref",
            });
        }
        if evidence_ref.len() > 256 {
            return Err(MarketProgramModelError::FieldTooLong {
                field: "evidence_ref",
                len: evidence_ref.len(),
                max: 256,
            });
        }
        if evidence_kind.trim().is_empty() {
            return Err(MarketProgramModelError::EmptyField {
                field: "evidence_kind",
            });
        }
        if evidence_kind.len() > 64 {
            return Err(MarketProgramModelError::FieldTooLong {
                field: "evidence_kind",
                len: evidence_kind.len(),
                max: 64,
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketProgramModelError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            evidence_ref,
            evidence_kind,
            recorded_at_unix_ms,
        })
    }

    /// The referenced evidence's identity.
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    /// The declared evidence kind.
    pub fn evidence_kind(&self) -> &str {
        &self.evidence_kind
    }

    /// When the reference was recorded (unix millis).
    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

/// The program's OWN baseline-method declaration (LR-073: the method is
/// the program's, never FlexCore's). The P16.04 engine consumes the
/// method NAME and decides support; this type only carries the declared
/// name as recorded data. An absent declaration is absent — never
/// defaulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineMethodDeclaration {
    method_name: String,
}

impl BaselineMethodDeclaration {
    /// Builds the declaration (non-empty, bounded name; SUPPORT is the
    /// engine's decision, never this constructor's).
    pub fn new(method_name: String) -> Result<Self, MarketProgramModelError> {
        if method_name.trim().is_empty() {
            return Err(MarketProgramModelError::EmptyField { field: "method_name" });
        }
        if method_name.len() > 64 {
            return Err(MarketProgramModelError::FieldTooLong {
                field: "method_name",
                len: method_name.len(),
                max: 64,
            });
        }
        Ok(Self { method_name })
    }

    /// The declared method name (as the program wrote it — casing and
    /// spelling are preserved; interpretation is the engine's).
    pub fn method_name(&self) -> &str {
        &self.method_name
    }
}

// --- Program lifecycle ------------------------------------------------------------

/// The closed program lifecycle vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProgramState {
    Declared,
    Active,
    Suspended,
    Retired,
}

impl ProgramState {
    /// The canonical spelling (the migration's CHECK vocabulary).
    pub fn name(&self) -> &'static str {
        match self {
            ProgramState::Declared => "DECLARED",
            ProgramState::Active => "ACTIVE",
            ProgramState::Suspended => "SUSPENDED",
            ProgramState::Retired => "RETIRED",
        }
    }

    /// Parses the canonical spelling (fail-closed; no normalization).
    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "DECLARED" => Some(ProgramState::Declared),
            "ACTIVE" => Some(ProgramState::Active),
            "SUSPENDED" => Some(ProgramState::Suspended),
            "RETIRED" => Some(ProgramState::Retired),
            _ => None,
        }
    }

    /// The closed transition table (the domain oracle — nothing outside
    /// it is representable). RETIRED is terminal.
    pub fn can_transition_to(self, to: ProgramState) -> bool {
        matches!(
            (self, to),
            (ProgramState::Declared, ProgramState::Active)
                | (ProgramState::Active, ProgramState::Suspended)
                | (ProgramState::Suspended, ProgramState::Active)
                | (ProgramState::Active, ProgramState::Retired)
                | (ProgramState::Suspended, ProgramState::Retired)
        )
    }
}

/// One market-program record (immutable; a state change is a NEW record
/// superseding this one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketProgram {
    program_id: String,
    tenant_id: String,
    program_name: String,
    /// The jurisdiction as DECLARED data (recorded, with the record
    /// pack's source; ADR-011 — jurisdiction-specific records).
    jurisdiction: String,
    state: ProgramState,
    /// The program's OWN baseline-method declaration (LR-073); `None`
    /// means the program has declared none — recorded as absent, never
    /// defaulted.
    baseline_method_declaration: Option<BaselineMethodDeclaration>,
    /// The program's other declared rules, as declared key/value data
    /// (BTreeMap for deterministic ordering).
    declared_rules: BTreeMap<String, String>,
    supersedes_program_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl MarketProgram {
    /// Builds and validates one program record. Every text field is
    /// required and bounded; nothing is defaulted.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program_id: String,
        tenant_id: String,
        program_name: String,
        jurisdiction: String,
        baseline_method_declaration: Option<BaselineMethodDeclaration>,
        declared_rules: BTreeMap<String, String>,
        supersedes_program_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketProgramModelError> {
        Self::validate_text("program_id", &program_id, 64)?;
        Self::validate_text("tenant_id", &tenant_id, 64)?;
        Self::validate_text("program_name", &program_name, 256)?;
        Self::validate_text("jurisdiction", &jurisdiction, 64)?;
        Self::validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketProgramModelError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            program_id,
            tenant_id,
            program_name,
            jurisdiction,
            state: ProgramState::Declared,
            baseline_method_declaration,
            declared_rules,
            supersedes_program_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), MarketProgramModelError> {
        if value.trim().is_empty() {
            return Err(MarketProgramModelError::EmptyField { field });
        }
        if value.len() > max {
            return Err(MarketProgramModelError::FieldTooLong {
                field,
                len: value.len(),
                max,
            });
        }
        Ok(())
    }

    /// The program identity (canonical UUID text).
    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    /// The owning tenant (canonical UUID text).
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn program_name(&self) -> &str {
        &self.program_name
    }

    pub fn jurisdiction(&self) -> &str {
        &self.jurisdiction
    }

    pub fn state(&self) -> ProgramState {
        self.state
    }

    /// The program's baseline-method declaration (LR-073 carrier).
    pub fn baseline_method_declaration(&self) -> Option<&BaselineMethodDeclaration> {
        self.baseline_method_declaration.as_ref()
    }

    /// The program's declared rules (deterministic order).
    pub fn declared_rules(&self) -> &BTreeMap<String, String> {
        &self.declared_rules
    }

    /// The program record this one supersedes, if any.
    pub fn supersedes_program_id(&self) -> Option<&str> {
        self.supersedes_program_id.as_deref()
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

// --- Contract lifecycle ------------------------------------------------------------

/// The closed contract lifecycle vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractState {
    Recorded,
    Active,
    Terminated,
}

impl ContractState {
    pub fn name(&self) -> &'static str {
        match self {
            ContractState::Recorded => "RECORDED",
            ContractState::Active => "ACTIVE",
            ContractState::Terminated => "TERMINATED",
        }
    }

    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "RECORDED" => Some(ContractState::Recorded),
            "ACTIVE" => Some(ContractState::Active),
            "TERMINATED" => Some(ContractState::Terminated),
            _ => None,
        }
    }

    /// The closed transition table. TERMINATED is terminal.
    pub fn can_transition_to(self, to: ContractState) -> bool {
        matches!(
            (self, to),
            (ContractState::Recorded, ContractState::Active)
                | (ContractState::Active, ContractState::Terminated)
        )
    }
}

/// One market-contract record (immutable; a change is a NEW record
/// superseding this one). The terms are declared key/value data — the
/// revenue-share splits enter as recorded data when the P16.13 billing
/// configuration lands; this type never interprets them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketContract {
    contract_id: String,
    tenant_id: String,
    program_id: String,
    contract_ref: String,
    state: ContractState,
    terms: BTreeMap<String, String>,
    supersedes_contract_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl MarketContract {
    /// Builds and validates one contract record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_id: String,
        tenant_id: String,
        program_id: String,
        contract_ref: String,
        terms: BTreeMap<String, String>,
        supersedes_contract_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketProgramModelError> {
        MarketProgram::validate_text("contract_id", &contract_id, 64)?;
        MarketProgram::validate_text("tenant_id", &tenant_id, 64)?;
        MarketProgram::validate_text("program_id", &program_id, 64)?;
        MarketProgram::validate_text("contract_ref", &contract_ref, 256)?;
        MarketProgram::validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketProgramModelError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            contract_id,
            tenant_id,
            program_id,
            contract_ref,
            state: ContractState::Recorded,
            terms,
            supersedes_contract_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn contract_id(&self) -> &str {
        &self.contract_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn contract_ref(&self) -> &str {
        &self.contract_ref
    }

    pub fn state(&self) -> ContractState {
        self.state
    }

    /// The recorded terms (declared data, deterministic order).
    pub fn terms(&self) -> &BTreeMap<String, String> {
        &self.terms
    }

    pub fn supersedes_contract_id(&self) -> Option<&str> {
        self.supersedes_contract_id.as_deref()
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

// --- Enrollment lifecycle ------------------------------------------------------------

/// The closed enrollment lifecycle vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnrollmentState {
    Requested,
    Accepted,
    Participating,
    Suspended,
    Withdrawn,
    Refused,
}

impl EnrollmentState {
    pub fn name(&self) -> &'static str {
        match self {
            EnrollmentState::Requested => "REQUESTED",
            EnrollmentState::Accepted => "ACCEPTED",
            EnrollmentState::Participating => "PARTICIPATING",
            EnrollmentState::Suspended => "SUSPENDED",
            EnrollmentState::Withdrawn => "WITHDRAWN",
            EnrollmentState::Refused => "REFUSED",
        }
    }

    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "REQUESTED" => Some(EnrollmentState::Requested),
            "ACCEPTED" => Some(EnrollmentState::Accepted),
            "PARTICIPATING" => Some(EnrollmentState::Participating),
            "SUSPENDED" => Some(EnrollmentState::Suspended),
            "WITHDRAWN" => Some(EnrollmentState::Withdrawn),
            "REFUSED" => Some(EnrollmentState::Refused),
            _ => None,
        }
    }

    /// The closed transition table (the domain oracle). WITHDRAWN and
    /// REFUSED are terminal.
    pub fn can_transition_to(self, to: EnrollmentState) -> bool {
        matches!(
            (self, to),
            (EnrollmentState::Requested, EnrollmentState::Accepted)
                | (EnrollmentState::Requested, EnrollmentState::Refused)
                | (EnrollmentState::Accepted, EnrollmentState::Participating)
                | (EnrollmentState::Participating, EnrollmentState::Suspended)
                | (EnrollmentState::Suspended, EnrollmentState::Participating)
                | (EnrollmentState::Accepted, EnrollmentState::Withdrawn)
                | (EnrollmentState::Participating, EnrollmentState::Withdrawn)
                | (EnrollmentState::Suspended, EnrollmentState::Withdrawn)
        )
    }

    /// Whether the state is terminal (no outgoing edges).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            EnrollmentState::Withdrawn | EnrollmentState::Refused
        )
    }
}

/// One per-asset enrollment record (immutable; a transition is a NEW
/// record superseding this one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketEnrollment {
    enrollment_id: String,
    tenant_id: String,
    program_id: String,
    asset_id: String,
    state: EnrollmentState,
    /// The participation window; `None` = not declared (recorded as
    /// missing, never widened).
    participation_window: Option<ParticipationWindow>,
    qualification_evidence: Vec<QualificationEvidence>,
    supersedes_enrollment_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl MarketEnrollment {
    /// Builds and validates one enrollment record. Terminal-state
    /// enrollments carry no new qualification evidence (the
    /// `TerminalStateEvidence` rule).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enrollment_id: String,
        tenant_id: String,
        program_id: String,
        asset_id: String,
        participation_window: Option<ParticipationWindow>,
        qualification_evidence: Vec<QualificationEvidence>,
        supersedes_enrollment_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketProgramModelError> {
        MarketProgram::validate_text("enrollment_id", &enrollment_id, 64)?;
        MarketProgram::validate_text("tenant_id", &tenant_id, 64)?;
        MarketProgram::validate_text("program_id", &program_id, 64)?;
        MarketProgram::validate_text("asset_id", &asset_id, 64)?;
        MarketProgram::validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketProgramModelError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            enrollment_id,
            tenant_id,
            program_id,
            asset_id,
            state: EnrollmentState::Requested,
            participation_window,
            qualification_evidence,
            supersedes_enrollment_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn enrollment_id(&self) -> &str {
        &self.enrollment_id
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

    pub fn state(&self) -> EnrollmentState {
        self.state
    }

    /// The participation window (`None` = not declared).
    pub fn participation_window(&self) -> Option<ParticipationWindow> {
        self.participation_window
    }

    /// The qualification-evidence references (deterministic insertion
    /// order as recorded).
    pub fn qualification_evidence(&self) -> &[QualificationEvidence] {
        &self.qualification_evidence
    }

    pub fn supersedes_enrollment_id(&self) -> Option<&str> {
        self.supersedes_enrollment_id.as_deref()
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

// --- Stacking rules (R-P16-7) -----------------------------------------------------

/// One RECORDED stacking rule: for one asset, the named set of program
/// ids that may NOT hold simultaneous PARTICIPATING enrollments (plan
/// risk R-P16-7; the rules are recorded per jurisdiction/program pair —
/// ADR-011 — never invented universally).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackingRule {
    rule_id: String,
    asset_id: String,
    /// The program ids that may not hold simultaneous PARTICIPATING
    /// enrollments for this asset (BTreeSet for deterministic order —
    /// held in a BTreeMap-keyed rule registry).
    mutually_exclusive_program_ids: Vec<String>,
    recorded_at_unix_ms: i64,
}

impl StackingRule {
    /// Builds one stacking rule (non-empty rule id and program set; at
    /// least two programs — a rule over one program is not a stacking
    /// constraint).
    pub fn new(
        rule_id: String,
        asset_id: String,
        mutually_exclusive_program_ids: Vec<String>,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, MarketProgramModelError> {
        MarketProgram::validate_text("rule_id", &rule_id, 64)?;
        MarketProgram::validate_text("asset_id", &asset_id, 64)?;
        let mut seen = std::collections::BTreeSet::new();
        for program_id in &mutually_exclusive_program_ids {
            MarketProgram::validate_text("mutually_exclusive_program_ids", program_id, 64)?;
            if !seen.insert(program_id.clone()) {
                return Err(MarketProgramModelError::DuplicateIdentity {
                    record_id: program_id.clone(),
                });
            }
        }
        if mutually_exclusive_program_ids.len() < 2 {
            return Err(MarketProgramModelError::EmptyField {
                field: "mutually_exclusive_program_ids",
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketProgramModelError::NegativeTimestamp {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            rule_id,
            asset_id,
            mutually_exclusive_program_ids,
            recorded_at_unix_ms,
        })
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn mutually_exclusive_program_ids(&self) -> &[String] {
        &self.mutually_exclusive_program_ids
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

// --- The domain stores ---------------------------------------------------------------

/// The BTreeMap-backed program-record store: register-only plus
/// supersede-based transitions (no update, no delete; the immutability
/// statement mirrored). Keyed by (tenant_id, program_id) identity; every
/// version of a record is retained.
#[derive(Debug, Default)]
pub struct MarketProgramStore {
    records: BTreeMap<(String, String), MarketProgram>,
    /// The supersede graph: the superseded record identity → the
    /// identity of the record superseding it (one successor per record).
    superseded_by: BTreeMap<(String, String), (String, String)>,
}

impl MarketProgramStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one program record. A duplicate CURRENT identity is a
    /// refusal (a change is a superseding record, never an overwrite).
    pub fn register(&mut self, program: MarketProgram) -> Result<(), MarketProgramModelError> {
        let key = (program.tenant_id().to_string(), program.program_id().to_string());
        if self.superseded_by.contains_key(&key) {
            return Err(MarketProgramModelError::RecordSuperseded {
                record_id: program.program_id().to_string(),
            });
        }
        if self.records.contains_key(&key) {
            return Err(MarketProgramModelError::DuplicateIdentity {
                record_id: program.program_id().to_string(),
            });
        }
        if let Some(superseded) = program.supersedes_program_id() {
            let target_key = (program.tenant_id().to_string(), superseded.to_string());
            let target = self
                .records
                .get(&target_key)
                .ok_or_else(|| MarketProgramModelError::RecordNotFound {
                    record_id: superseded.to_string(),
                })?;
            if target.state.can_transition_to(program.state()) {
                self.superseded_by.insert(target_key, key.clone());
            } else {
                return Err(MarketProgramModelError::IllegalTransition {
                    from: target.state().name().to_string(),
                    to: program.state().name().to_string(),
                });
            }
        }
        self.records.insert(key, program);
        Ok(())
    }

    /// All retained program records for one tenant, ascending
    /// (program_id) order — deterministic.
    pub fn records_for_tenant(&self, tenant_id: &str) -> Vec<&MarketProgram> {
        self.records
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .map(|(_, program)| program)
            .collect()
    }

    /// The CURRENT (non-superseded) program records for one tenant,
    /// ascending program_id order.
    pub fn current_records_for_tenant(&self, tenant_id: &str) -> Vec<&MarketProgram> {
        self.records_for_tenant(tenant_id)
            .into_iter()
            .filter(|program| {
                let key = (
                    program.tenant_id().to_string(),
                    program.program_id().to_string(),
                );
                !self.superseded_by.contains_key(&key)
            })
            .collect()
    }
}

/// The BTreeMap-backed contract-record store (same register-only +
/// supersede discipline as [`MarketProgramStore`]).
#[derive(Debug, Default)]
pub struct MarketContractStore {
    records: BTreeMap<(String, String), MarketContract>,
    superseded_by: BTreeMap<(String, String), (String, String)>,
}

impl MarketContractStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, contract: MarketContract) -> Result<(), MarketProgramModelError> {
        let key = (
            contract.tenant_id().to_string(),
            contract.contract_id().to_string(),
        );
        if self.superseded_by.contains_key(&key) {
            return Err(MarketProgramModelError::RecordSuperseded {
                record_id: contract.contract_id().to_string(),
            });
        }
        if self.records.contains_key(&key) {
            return Err(MarketProgramModelError::DuplicateIdentity {
                record_id: contract.contract_id().to_string(),
            });
        }
        if let Some(superseded) = contract.supersedes_contract_id() {
            let target_key = (contract.tenant_id().to_string(), superseded.to_string());
            let target = self
                .records
                .get(&target_key)
                .ok_or_else(|| MarketProgramModelError::RecordNotFound {
                    record_id: superseded.to_string(),
                })?;
            if target.state.can_transition_to(contract.state()) {
                self.superseded_by.insert(target_key, key.clone());
            } else {
                return Err(MarketProgramModelError::IllegalTransition {
                    from: target.state().name().to_string(),
                    to: contract.state().name().to_string(),
                });
            }
        }
        self.records.insert(key, contract);
        Ok(())
    }

    /// All retained contract records for one tenant's program, ascending
    /// contract_id order.
    pub fn records_for_program(&self, tenant_id: &str, program_id: &str) -> Vec<&MarketContract> {
        self.records
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .filter(|(_, contract)| contract.program_id() == program_id)
            .map(|(_, contract)| contract)
            .collect()
    }

    /// The CURRENT contract records for one tenant's program.
    pub fn current_records_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Vec<&MarketContract> {
        self.records_for_program(tenant_id, program_id)
            .into_iter()
            .filter(|contract| {
                let key = (
                    contract.tenant_id().to_string(),
                    contract.contract_id().to_string(),
                );
                !self.superseded_by.contains_key(&key)
            })
            .collect()
    }
}

/// The BTreeMap-backed enrollment-record store (same discipline), WITH
/// the stacking-rule enforcement (R-P16-7): a PARTICIPATING enrollment
/// is refused when a recorded stacking rule for the asset names both its
/// program and another program that already holds a CURRENT PARTICIPATING
/// enrollment for the same asset.
#[derive(Debug, Default)]
pub struct MarketEnrollmentStore {
    records: BTreeMap<(String, String), MarketEnrollment>,
    superseded_by: BTreeMap<(String, String), (String, String)>,
    stacking_rules: BTreeMap<(String, String), StackingRule>,
}

impl MarketEnrollmentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one stacking rule (per asset, per recorded rule id — the
    /// rules arrive from jurisdiction record packs as DATA).
    pub fn record_stacking_rule(&mut self, rule: StackingRule) -> Result<(), MarketProgramModelError> {
        let key = (rule.asset_id().to_string(), rule.rule_id().to_string());
        if self.stacking_rules.contains_key(&key) {
            return Err(MarketProgramModelError::DuplicateIdentity {
                record_id: rule.rule_id().to_string(),
            });
        }
        self.stacking_rules.insert(key, rule);
        Ok(())
    }

    /// The recorded stacking rules for one asset, ascending rule_id
    /// order (deterministic).
    pub fn stacking_rules_for_asset(&self, asset_id: &str) -> Vec<&StackingRule> {
        self.stacking_rules
            .range((asset_id.to_string(), String::new())..)
            .take_while(|((asset, _), _)| asset == asset_id)
            .map(|(_, rule)| rule)
            .collect()
    }

    fn stacking_violation(
        &self,
        asset_id: &str,
        program_id: &str,
        state: EnrollmentState,
    ) -> Option<(String, String)> {
        if state != EnrollmentState::Participating {
            return None;
        }
        for rule in self.stacking_rules_for_asset(asset_id) {
            if !rule
                .mutually_exclusive_program_ids()
                .iter()
                .any(|p| p == program_id)
            {
                continue;
            }
            for conflicting_program_id in rule.mutually_exclusive_program_ids() {
                if conflicting_program_id == program_id {
                    continue;
                }
                let current = self.current_records_for_program_tenantless(conflicting_program_id);
                if current
                    .iter()
                    .any(|enrollment| {
                        enrollment.asset_id() == asset_id
                            && enrollment.state() == EnrollmentState::Participating
                    })
                {
                    return Some((rule.rule_id().to_string(), conflicting_program_id.clone()));
                }
            }
        }
        None
    }

    /// The current PARTICIPATING-candidate enrollments for one program
    /// across tenants (the stacking check is per asset, and a recorded
    /// rule may span tenants only within the same deployment — the rule
    /// records the programs, the asset match is exact).
    fn current_records_for_program_tenantless(&self, program_id: &str) -> Vec<&MarketEnrollment> {
        self.records
            .values()
            .filter(|enrollment| {
                enrollment.program_id() == program_id
                    && !self.superseded_by.contains_key(&(
                        enrollment.tenant_id().to_string(),
                        enrollment.enrollment_id().to_string(),
                    ))
            })
            .collect()
    }

    pub fn register(&mut self, enrollment: MarketEnrollment) -> Result<(), MarketProgramModelError> {
        let key = (
            enrollment.tenant_id().to_string(),
            enrollment.enrollment_id().to_string(),
        );
        if self.superseded_by.contains_key(&key) {
            return Err(MarketProgramModelError::RecordSuperseded {
                record_id: enrollment.enrollment_id().to_string(),
            });
        }
        if self.records.contains_key(&key) {
            return Err(MarketProgramModelError::DuplicateIdentity {
                record_id: enrollment.enrollment_id().to_string(),
            });
        }
        if let Some(superseded) = enrollment.supersedes_enrollment_id() {
            let target_key = (enrollment.tenant_id().to_string(), superseded.to_string());
            let target = self
                .records
                .get(&target_key)
                .ok_or_else(|| MarketProgramModelError::RecordNotFound {
                    record_id: superseded.to_string(),
                })?;
            if target.state.is_terminal() && !enrollment.qualification_evidence().is_empty() {
                return Err(MarketProgramModelError::TerminalStateEvidence);
            }
            if target.state.can_transition_to(enrollment.state()) {
                self.superseded_by.insert(target_key, key.clone());
            } else {
                return Err(MarketProgramModelError::IllegalTransition {
                    from: target.state().name().to_string(),
                    to: enrollment.state().name().to_string(),
                });
            }
        }
        if let Some((rule_id, conflicting)) = self.stacking_violation(
            enrollment.asset_id(),
            enrollment.program_id(),
            enrollment.state(),
        ) {
            return Err(MarketProgramModelError::StackingRuleViolation {
                asset_id: enrollment.asset_id().to_string(),
                rule_id,
                conflicting_program_id: conflicting,
            });
        }
        self.records.insert(key, enrollment);
        Ok(())
    }

    /// All retained enrollment records for one tenant's program,
    /// ascending enrollment_id order.
    pub fn records_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Vec<&MarketEnrollment> {
        self.records
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .filter(|(_, enrollment)| enrollment.program_id() == program_id)
            .map(|(_, enrollment)| enrollment)
            .collect()
    }

    /// The CURRENT enrollment records for one tenant's program.
    pub fn current_records_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Vec<&MarketEnrollment> {
        self.records_for_program(tenant_id, program_id)
            .into_iter()
            .filter(|enrollment| {
                let key = (
                    enrollment.tenant_id().to_string(),
                    enrollment.enrollment_id().to_string(),
                );
                !self.superseded_by.contains_key(&key)
            })
            .collect()
    }
}
