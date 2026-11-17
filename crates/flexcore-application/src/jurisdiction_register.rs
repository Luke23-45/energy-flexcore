// P16.18 — The market jurisdiction register: per-tenant market
//         enablement (NONE enabled by default), the regulatory-calendar
//         data (FERC 2222 timelines [S-26][S-27]), and the ERCOT ADER
//         pilot posture [S-100] — REGISTER AS DATA, with every date
//         stored as sourced data carrying the re-verify-at-use rule
//         (the P16.16 `SourcedDate` type reused; the checked-in data
//         lives in [`crate::jurisdiction_register_data`]).
//
// Phase 16 plan §2 row P16.18; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 46_p16_18_market_jurisdiction_register.md`.
//
// WHAT THIS IS (decided once):
//
// - **The register is DATA, never logic** (plan risk R-P16-1): market
//   timelines, calendar entries, and postures are records with source
//   labels and verification dates. No date is baked into any code
//   path; the re-verify-at-use rule travels ON every date
//   (unconstructable without it).
// - **NONE is enabled by default** (hard rule): the checked-in
//   register carries ZERO market enablements, and the enablement state
//   vocabulary is {DISABLED, PENDING_D1_DECISION, ENABLED_BY_D1} —
//   where ENABLED_BY_D1 is constructable ONLY with the human owner's
//   decision reference and the P11 audit-chain link (the D1-class
//   jurisdiction decision is the human owner's; conventions §4). A
//   default, silent, or inferred enablement is unconstructable.
// - **Jurisdiction specificity is the whole point** (ADR-011): the
//   register NAMES jurisdictions as data rows; it is never a generic
//   "Order 2222 adapter" or a universal market API. A new jurisdiction
//   is a NEW data row with its own sources.
// - **Conflicts are carried, never silently resolved** (the LR-075
//   pattern): where two program sources disagree on a date, BOTH
//   entries are carried with their sources and conflict notes.
// - **No market is connected** (honesty rules): the register enables
//   nothing by itself — an enablement row is the RECORDED outcome of a
//   human decision, and no enablement exists in the checked-in data.
//
// Determinism: BTreeMap-backed, deterministic ordering, no wall-clock
// input.
//
// Control status: register-as-data for the simulation-only 0.x line.
// Nothing here connects to a market, files anything, or enables
// participation.

use std::collections::BTreeMap;

use flexcore_domain::market_program_model::AuditChainLink;

use crate::capacity_market_records::SourcedDate;

/// The jurisdiction-register schema version (a change is a NEW
/// version, never a silent edit).
pub const JURISDICTION_REGISTER_SCHEMA_VERSION: u32 = 1;

/// The none-enabled statement (pinned by tests).
pub const NONE_ENABLED_BY_DEFAULT_STATEMENT: &str = "The jurisdiction register ships with \
     ZERO markets enabled: the checked-in data carries no enablement rows, the \
     enablement vocabulary is {DISABLED, PENDING_D1_DECISION, ENABLED_BY_D1}, and \
     ENABLED_BY_D1 is constructable ONLY with the human owner's decision reference and \
     the P11 audit-chain link (the D1-class jurisdiction decision is the human owner's). \
     No default, silent, or inferred enablement is constructable.";

/// The register-as-data statement (pinned by tests).
pub const REGISTER_AS_DATA_STATEMENT: &str = "The register is DATA, never logic: market \
     timelines, calendar entries, and postures are records with source labels and \
     verification dates, and every date carries the re-verify-at-use rule (plan risk \
     R-P16-1). Conflicts between sources are carried with conflict notes — never \
     silently resolved (the LR-075 pattern).";

/// The jurisdiction-specificity statement (pinned by tests).
pub const JURISDICTION_SPECIFIC_STATEMENT: &str = "The register NAMES jurisdictions as \
     data rows; it is never a generic 'Order 2222 adapter', a universal market API, or \
     a generic tariff engine (ADR-011). A new jurisdiction is a NEW data row with its \
     own sources.";

// --- Errors ---------------------------------------------------------------------

/// Why a register operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
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
    /// An ENABLED_BY_D1 enablement without the human owner's decision
    /// reference (no default, silent, or inferred enablement).
    D1DecisionRefRequired,
    /// An ENABLED_BY_D1 enablement without the P11 audit-chain link.
    AuditLinkRequired,
    /// A jurisdiction entry carries no timeline entry (a jurisdiction
    /// row without sourced dates is not a register row).
    TimelineRequired,
    /// The enablement references an unknown jurisdiction.
    UnknownJurisdiction {
        jurisdiction_id: String,
    },
    /// A duplicate identity was registered.
    DuplicateIdentity {
        record_id: String,
    },
    /// A capacity/money shared-type refused (carried — the sourced
    /// date's refusals arrive this way).
    SourcedDate(crate::capacity_market_records::CapacityRecordError),
    /// A domain shared-type refused (carried).
    Domain(flexcore_domain::market_program_model::MarketProgramModelError),
}

impl RegisterError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            RegisterError::EmptyField { .. } => "REGISTER_EMPTY_FIELD",
            RegisterError::FieldTooLong { .. } => "REGISTER_FIELD_TOO_LONG",
            RegisterError::D1DecisionRefRequired => "REGISTER_D1_DECISION_REF_REQUIRED",
            RegisterError::AuditLinkRequired => "REGISTER_AUDIT_LINK_REQUIRED",
            RegisterError::TimelineRequired => "REGISTER_TIMELINE_REQUIRED",
            RegisterError::UnknownJurisdiction { .. } => "REGISTER_UNKNOWN_JURISDICTION",
            RegisterError::DuplicateIdentity { .. } => "REGISTER_DUPLICATE_IDENTITY",
            RegisterError::SourcedDate(_) => "REGISTER_SOURCED_DATE_ERROR",
            RegisterError::Domain(_) => "REGISTER_DOMAIN_ERROR",
        }
    }
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterError::EmptyField { field } => {
                write!(f, "the register field '{field}' must be non-empty")
            }
            RegisterError::FieldTooLong { field, len, max } => write!(
                f,
                "the register field '{field}' is {len} characters; the bound is {max}"
            ),
            RegisterError::D1DecisionRefRequired => write!(
                f,
                "an ENABLED_BY_D1 enablement without the human owner's decision \
                 reference is refused: no default, silent, or inferred enablement \
                 exists (the D1-class decision is the human owner's)"
            ),
            RegisterError::AuditLinkRequired => write!(
                f,
                "an ENABLED_BY_D1 enablement without the P11 audit-chain link is \
                 refused: the enablement decision is a financial-adjacent record"
            ),
            RegisterError::TimelineRequired => write!(
                f,
                "a jurisdiction entry without a sourced timeline entry is not a \
                 register row: dates enter only as sourced data"
            ),
            RegisterError::UnknownJurisdiction { jurisdiction_id } => write!(
                f,
                "the enablement references the unknown jurisdiction {jurisdiction_id}; \
                 enablements are recorded only against named register rows"
            ),
            RegisterError::DuplicateIdentity { record_id } => {
                write!(f, "the register identity {record_id} is already registered")
            }
            RegisterError::SourcedDate(err) => write!(f, "sourced-date refused: {err}"),
            RegisterError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
        }
    }
}

impl std::error::Error for RegisterError {}

// --- Shared helpers ---------------------------------------------------------------

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), RegisterError> {
    if value.trim().is_empty() {
        return Err(RegisterError::EmptyField { field });
    }
    if value.len() > max {
        return Err(RegisterError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

// --- The closed vocabularies -------------------------------------------------------

/// The closed enablement-state vocabulary. NONE is enabled by default:
/// DISABLED is the resting state; PENDING_D1_DECISION records that the
/// jurisdiction is under the human owner's review; ENABLED_BY_D1 is
/// constructable ONLY with the decision reference and the audit link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnablementState {
    /// No market participation is enabled (the default).
    Disabled,
    /// The jurisdiction is under the human owner's D1-class review.
    PendingD1Decision,
    /// The human owner's D1-class decision ENABLED the jurisdiction
    /// (recorded with the decision reference and the audit link).
    EnabledByD1,
}

impl EnablementState {
    /// The canonical name.
    pub const fn name(self) -> &'static str {
        match self {
            EnablementState::Disabled => "DISABLED",
            EnablementState::PendingD1Decision => "PENDING_D1_DECISION",
            EnablementState::EnabledByD1 => "ENABLED_BY_D1",
        }
    }
}

/// The closed market-posture vocabulary (the posture the register
/// records per jurisdiction — recorded data, never a capability
/// claim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarketPosture {
    /// A FERC Order 2222 participation-model jurisdiction ([S-26]
    /// [S-27]).
    Ferc2222,
    /// The ERCOT ADER pilot posture (ERCOT is not under FERC
    /// jurisdiction; the ADER pilot is the participation path [S-100]).
    ErcotAderPilot,
    /// The UK Local Flexibility Market posture ([S-36][S-37]).
    UkLfm,
    /// The EU balancing/EBGL posture ([S-107]).
    EuBalancing,
}

impl MarketPosture {
    /// The canonical name.
    pub const fn name(self) -> &'static str {
        match self {
            MarketPosture::Ferc2222 => "FERC_2222",
            MarketPosture::ErcotAderPilot => "ERCOT_ADER_PILOT",
            MarketPosture::UkLfm => "UK_LFM",
            MarketPosture::EuBalancing => "EU_BALANCING",
        }
    }
}

// --- The timeline entry -------------------------------------------------------------

/// One timeline entry: a labeled sourced date plus an optional
/// conflict note (the LR-075 pattern — where sources disagree, both
/// entries are carried with notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    entry_label: String,
    date: SourcedDate,
    conflict_note: Option<String>,
}

impl TimelineEntry {
    /// Builds one timeline entry (the sourced date carries the
    /// mandatory re-verify rule internally).
    pub fn new(
        entry_label: String,
        date: SourcedDate,
        conflict_note: Option<String>,
    ) -> Result<Self, RegisterError> {
        validate_text("entry_label", &entry_label, 256)?;
        if let Some(note) = &conflict_note {
            if note.len() > 1024 {
                return Err(RegisterError::FieldTooLong {
                    field: "conflict_note",
                    len: note.len(),
                    max: 1024,
                });
            }
        }
        Ok(Self {
            entry_label,
            date,
            conflict_note,
        })
    }

    pub fn entry_label(&self) -> &str {
        &self.entry_label
    }

    pub fn date(&self) -> &SourcedDate {
        &self.date
    }

    pub fn conflict_note(&self) -> Option<&str> {
        self.conflict_note.as_deref()
    }
}

// --- The jurisdiction entry -----------------------------------------------------------

/// One jurisdiction row of the register: the named jurisdiction, its
/// participation-model posture (recorded data), and its sourced
/// timeline. The row is DATA — it enables nothing by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JurisdictionEntry {
    jurisdiction_id: String,
    display_name: String,
    /// The participation-model label as recorded (data — e.g. the
    /// ISO/RTO's participation-model name).
    participation_model_label: String,
    posture: MarketPosture,
    timeline: Vec<TimelineEntry>,
}

impl JurisdictionEntry {
    /// Builds one jurisdiction row (at least one sourced timeline
    /// entry — a row without dates is not a register row).
    pub fn new(
        jurisdiction_id: String,
        display_name: String,
        participation_model_label: String,
        posture: MarketPosture,
        timeline: Vec<TimelineEntry>,
    ) -> Result<Self, RegisterError> {
        validate_text("jurisdiction_id", &jurisdiction_id, 64)?;
        validate_text("display_name", &display_name, 128)?;
        validate_text("participation_model_label", &participation_model_label, 256)?;
        if timeline.is_empty() {
            return Err(RegisterError::TimelineRequired);
        }
        Ok(Self {
            jurisdiction_id,
            display_name,
            participation_model_label,
            posture,
            timeline,
        })
    }

    pub fn jurisdiction_id(&self) -> &str {
        &self.jurisdiction_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn participation_model_label(&self) -> &str {
        &self.participation_model_label
    }

    pub fn posture(&self) -> MarketPosture {
        self.posture
    }

    /// The sourced timeline (as recorded — conflicts carried, never
    /// resolved).
    pub fn timeline(&self) -> &[TimelineEntry] {
        &self.timeline
    }
}

// --- The tenant enablement --------------------------------------------------------------

/// One per-tenant market enablement row. The DISABLED and
/// PENDING_D1_DECISION states carry no decision data; ENABLED_BY_D1 is
/// constructable ONLY through [`TenantMarketEnablement::enabled_by_d1`]
/// with the human owner's decision reference and the P11 audit-chain
/// link — the single construction path for a market enablement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantMarketEnablement {
    tenant_id: String,
    jurisdiction_id: String,
    state: EnablementState,
    /// The human owner's D1-class decision reference (required exactly
    /// when the state is ENABLED_BY_D1).
    decision_ref: Option<String>,
    /// The P11 audit-chain LINK of the enablement decision's write
    /// (required exactly when the state is ENABLED_BY_D1).
    audit_chain_link: Option<AuditChainLink>,
    recorded_at_unix_ms: i64,
}

impl TenantMarketEnablement {
    fn validate_ids(
        tenant_id: &str,
        jurisdiction_id: &str,
        recorded_at_unix_ms: i64,
    ) -> Result<(), RegisterError> {
        validate_text("tenant_id", tenant_id, 64)?;
        validate_text("jurisdiction_id", jurisdiction_id, 64)?;
        if recorded_at_unix_ms < 0 {
            return Err(RegisterError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(())
    }

    /// Records a DISABLED enablement (the default posture, recorded
    /// explicitly when the tenant's posture is known).
    pub fn disabled(
        tenant_id: String,
        jurisdiction_id: String,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, RegisterError> {
        Self::validate_ids(&tenant_id, &jurisdiction_id, recorded_at_unix_ms)?;
        Ok(Self {
            tenant_id,
            jurisdiction_id,
            state: EnablementState::Disabled,
            decision_ref: None,
            audit_chain_link: None,
            recorded_at_unix_ms,
        })
    }

    /// Records a PENDING_D1_DECISION enablement (under human review).
    pub fn pending_d1_decision(
        tenant_id: String,
        jurisdiction_id: String,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, RegisterError> {
        Self::validate_ids(&tenant_id, &jurisdiction_id, recorded_at_unix_ms)?;
        Ok(Self {
            tenant_id,
            jurisdiction_id,
            state: EnablementState::PendingD1Decision,
            decision_ref: None,
            audit_chain_link: None,
            recorded_at_unix_ms,
        })
    }

    /// Records an ENABLED_BY_D1 enablement — the ONLY construction
    /// path for a market enablement, requiring the human owner's
    /// decision reference and the P11 audit-chain link.
    pub fn enabled_by_d1(
        tenant_id: String,
        jurisdiction_id: String,
        decision_ref: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, RegisterError> {
        Self::validate_ids(&tenant_id, &jurisdiction_id, recorded_at_unix_ms)?;
        if decision_ref.trim().is_empty() {
            return Err(RegisterError::D1DecisionRefRequired);
        }
        if decision_ref.len() > 256 {
            return Err(RegisterError::FieldTooLong {
                field: "decision_ref",
                len: decision_ref.len(),
                max: 256,
            });
        }
        Ok(Self {
            tenant_id,
            jurisdiction_id,
            state: EnablementState::EnabledByD1,
            decision_ref: Some(decision_ref),
            audit_chain_link: Some(audit_chain_link),
            recorded_at_unix_ms,
        })
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn jurisdiction_id(&self) -> &str {
        &self.jurisdiction_id
    }

    pub fn state(&self) -> EnablementState {
        self.state
    }

    pub fn decision_ref(&self) -> Option<&str> {
        self.decision_ref.as_deref()
    }

    pub fn audit_chain_link(&self) -> Option<&AuditChainLink> {
        self.audit_chain_link.as_ref()
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

// --- The register -------------------------------------------------------------------------

/// The market jurisdiction register: named jurisdiction rows plus
/// per-tenant enablement rows. The register is DATA — no enablement
/// exists unless a human decision is recorded, and the checked-in
/// register (see [`crate::jurisdiction_register_data`]) carries ZERO
/// enablements.
#[derive(Debug, Default)]
pub struct MarketJurisdictionRegister {
    entries: BTreeMap<String, JurisdictionEntry>,
    enablements: BTreeMap<(String, String), TenantMarketEnablement>,
}

impl MarketJurisdictionRegister {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one jurisdiction row.
    pub fn register_jurisdiction(&mut self, entry: JurisdictionEntry) -> Result<(), RegisterError> {
        let key = entry.jurisdiction_id().to_string();
        if self.entries.contains_key(&key) {
            return Err(RegisterError::DuplicateIdentity {
                record_id: key.clone(),
            });
        }
        self.entries.insert(key, entry);
        Ok(())
    }

    /// The jurisdiction rows (ascending jurisdiction id —
    /// deterministic).
    pub fn jurisdictions(&self) -> Vec<&JurisdictionEntry> {
        self.entries.values().collect()
    }

    /// The jurisdiction lookup.
    pub fn jurisdiction(&self, jurisdiction_id: &str) -> Option<&JurisdictionEntry> {
        self.entries.get(jurisdiction_id)
    }

    /// Records one enablement row (the jurisdiction must exist).
    pub fn record_enablement(
        &mut self,
        enablement: TenantMarketEnablement,
    ) -> Result<(), RegisterError> {
        let key = (
            enablement.tenant_id().to_string(),
            enablement.jurisdiction_id().to_string(),
        );
        if !self.entries.contains_key(&enablement.jurisdiction_id().to_string()) {
            return Err(RegisterError::UnknownJurisdiction {
                jurisdiction_id: enablement.jurisdiction_id().to_string(),
            });
        }
        if self.enablements.contains_key(&key) {
            return Err(RegisterError::DuplicateIdentity {
                record_id: format!("{}/{}", key.0, key.1),
            });
        }
        self.enablements.insert(key, enablement);
        Ok(())
    }

    /// The enablement lookup.
    pub fn enablement(&self, tenant_id: &str, jurisdiction_id: &str)
        -> Option<&TenantMarketEnablement>
    {
        self.enablements
            .get(&(tenant_id.to_string(), jurisdiction_id.to_string()))
    }

    /// The jurisdictions ENABLED for one tenant (a row is enabled only
    /// at ENABLED_BY_D1). For the checked-in register this is EMPTY —
    /// zero markets enabled.
    pub fn enabled_jurisdictions_for_tenant(&self, tenant_id: &str) -> Vec<&str> {
        self.enablements
            .iter()
            .filter(|((tenant, _), enablement)| {
                tenant == tenant_id && enablement.state() == EnablementState::EnabledByD1
            })
            .map(|((_, jurisdiction), _)| jurisdiction.as_str())
            .collect()
    }

    /// Validates the register's structural invariants: every
    /// jurisdiction row carries a timeline; every timeline date
    /// carries the re-verify rule (guaranteed by the `SourcedDate`
    /// constructor); every ENABLED_BY_D1 enablement carries the
    /// decision reference and the audit link; every enablement
    /// references a known jurisdiction.
    pub fn validate(&self) -> Result<(), RegisterError> {
        for entry in self.entries.values() {
            if entry.timeline().is_empty() {
                return Err(RegisterError::TimelineRequired);
            }
        }
        for enablement in self.enablements.values() {
            if !self.entries.contains_key(enablement.jurisdiction_id()) {
                return Err(RegisterError::UnknownJurisdiction {
                    jurisdiction_id: enablement.jurisdiction_id().to_string(),
                });
            }
            if enablement.state() == EnablementState::EnabledByD1 {
                if enablement.decision_ref().is_none() {
                    return Err(RegisterError::D1DecisionRefRequired);
                }
                if enablement.audit_chain_link().is_none() {
                    return Err(RegisterError::AuditLinkRequired);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. The checked-in data file's tests
// live with the data module; the tests here cover the register TYPES.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity_market_records::RESEARCH_CUTOFF;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    fn sourced_date(text: &str) -> SourcedDate {
        SourcedDate::new(
            text.to_string(),
            "[S-26] FERC Order 2222 explainer".to_string(),
            RESEARCH_CUTOFF.to_string(),
            true,
        )
        .expect("sourced date")
    }

    fn entry(id: &str) -> JurisdictionEntry {
        let timeline = vec![TimelineEntry::new(
            "participation timeline".to_string(),
            sourced_date("1 November 2024 (synthetic fixture date)"),
            None,
        )
        .expect("timeline entry")];
        JurisdictionEntry::new(
            id.to_string(),
            "Synthetic jurisdiction".to_string(),
            "synthetic participation model".to_string(),
            MarketPosture::Ferc2222,
            timeline,
        )
        .expect("jurisdiction entry")
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(NONE_ENABLED_BY_DEFAULT_STATEMENT.contains("ZERO markets enabled"));
        assert!(NONE_ENABLED_BY_DEFAULT_STATEMENT.contains("the D1-class jurisdiction \
             decision is the human owner's"));
        assert!(REGISTER_AS_DATA_STATEMENT.contains("DATA, never logic"));
        assert!(JURISDICTION_SPECIFIC_STATEMENT.contains("ADR-011"));
    }

    #[test]
    fn the_enablement_vocabulary_is_closed_and_default_absent() {
        assert_eq!(EnablementState::Disabled.name(), "DISABLED");
        assert_eq!(EnablementState::PendingD1Decision.name(), "PENDING_D1_DECISION");
        assert_eq!(EnablementState::EnabledByD1.name(), "ENABLED_BY_D1");
        // A fresh register carries no enablements: NONE enabled.
        let register = MarketJurisdictionRegister::new();
        assert!(register.enabled_jurisdictions_for_tenant("tenant-1").is_empty());
        assert_eq!(register.jurisdictions().len(), 0);
    }

    #[test]
    fn enabled_by_d1_requires_the_decision_reference_and_audit_link() {
        // The empty decision reference is refused.
        assert_eq!(
            TenantMarketEnablement::enabled_by_d1(
                "tenant-1".to_string(),
                "caiso".to_string(),
                String::new(),
                link(),
                1_700_000_000_000,
            )
            .expect_err("decision ref mandatory")
            .name(),
            "REGISTER_D1_DECISION_REF_REQUIRED"
        );
        let enablement = TenantMarketEnablement::enabled_by_d1(
            "tenant-1".to_string(),
            "caiso".to_string(),
            "D1-decision-ref-synthetic".to_string(),
            link(),
            1_700_000_000_000,
        )
        .expect("enabled with the decision data");
        assert_eq!(enablement.decision_ref(), Some("D1-decision-ref-synthetic"));
        assert!(enablement.audit_chain_link().is_some());
    }

    #[test]
    fn a_jurisdiction_row_without_timeline_is_not_a_register_row() {
        assert_eq!(
            JurisdictionEntry::new(
                "x".to_string(),
                "y".to_string(),
                "z".to_string(),
                MarketPosture::Ferc2222,
                Vec::new(),
            )
            .expect_err("timeline mandatory")
            .name(),
            "REGISTER_TIMELINE_REQUIRED"
        );
    }

    #[test]
    fn enablements_reference_known_jurisdictions_only() {
        let mut register = MarketJurisdictionRegister::new();
        register.register_jurisdiction(entry("caiso")).expect("registered");
        assert_eq!(
            register
                .record_enablement(
                    TenantMarketEnablement::disabled(
                        "tenant-1".to_string(),
                        "unknown-jurisdiction".to_string(),
                        1_700_000_000_000,
                    )
                    .expect("disabled row")
                )
                .expect_err("unknown jurisdiction")
                .name(),
            "REGISTER_UNKNOWN_JURISDICTION"
        );
        register
            .record_enablement(
                TenantMarketEnablement::disabled(
                    "tenant-1".to_string(),
                    "caiso".to_string(),
                    1_700_000_000_000,
                )
                .expect("disabled row"),
            )
            .expect("recorded");
        register.validate().expect("register validates");
        // Still ZERO enabled.
        assert!(register.enabled_jurisdictions_for_tenant("tenant-1").is_empty());
    }

    #[test]
    fn validation_catches_a_broken_enabled_row() {
        let mut register = MarketJurisdictionRegister::new();
        register.register_jurisdiction(entry("caiso")).expect("registered");
        // An ENABLED_BY_D1 row always carries its decision data by
        // construction; the structural check is that the ONLY path is
        // the constructor — there is no way to build an enabled row
        // without the reference (the variants below are refused at
        // construction).
        assert_eq!(
            TenantMarketEnablement::enabled_by_d1(
                "tenant-1".to_string(),
                "caiso".to_string(),
                "  ".to_string(),
                link(),
                1_700_000_000_000,
            )
            .expect_err("whitespace decision ref refused")
            .name(),
            "REGISTER_D1_DECISION_REF_REQUIRED"
        );
        // A valid enabled row records and validates.
        register
            .record_enablement(
                TenantMarketEnablement::enabled_by_d1(
                    "tenant-1".to_string(),
                    "caiso".to_string(),
                    "D1-ref".to_string(),
                    link(),
                    1_700_000_000_000,
                )
                .expect("enabled row"),
            )
            .expect("recorded");
        register.validate().expect("validates");
        assert_eq!(register.enabled_jurisdictions_for_tenant("tenant-1"), vec!["caiso"]);
    }

    #[test]
    fn conflicts_are_carried_with_notes_never_resolved() {
        let timeline = vec![
            TimelineEntry::new(
                "first source".to_string(),
                sourced_date("February 2027 (synthetic)"),
                Some("conflicts with the second source — BOTH carried".to_string()),
            )
            .expect("entry"),
            TimelineEntry::new(
                "second source".to_string(),
                sourced_date("February 2028 (synthetic)"),
                Some("conflicts with the first source — BOTH carried".to_string()),
            )
            .expect("entry"),
        ];
        let jurisdiction = JurisdictionEntry::new(
            "pjm".to_string(),
            "PJM (synthetic fixture)".to_string(),
            "synthetic".to_string(),
            MarketPosture::Ferc2222,
            timeline,
        )
        .expect("entry");
        assert_eq!(jurisdiction.timeline().len(), 2);
        assert!(jurisdiction.timeline()[0].conflict_note().is_some());
        assert!(jurisdiction.timeline()[1].conflict_note().is_some());
    }

    #[test]
    fn the_posture_vocabulary_is_closed() {
        assert_eq!(MarketPosture::Ferc2222.name(), "FERC_2222");
        assert_eq!(MarketPosture::ErcotAderPilot.name(), "ERCOT_ADER_PILOT");
        assert_eq!(MarketPosture::UkLfm.name(), "UK_LFM");
        assert_eq!(MarketPosture::EuBalancing.name(), "EU_BALANCING");
    }
}
