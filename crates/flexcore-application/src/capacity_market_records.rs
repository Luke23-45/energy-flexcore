// P16.16 — Capacity-market participation records, RECORD-ONLY depth:
//         qualification-evidence CHECKLISTS for PJM RPM, ISO-NE FCA19,
//         and ISO-NE DECR, with every date stored as SOURCED DATA
//         carrying the re-verify-at-use rule ([S-26] FERC Order 2222
//         explainer; [S-103] ISO-NE Order 2222 participant readiness;
//         the Phase 9 capture template
//         `ledger/phase-09-pilot-qualification/
//         11_market_program_requirements.md` §7).
//
// Phase 16 plan §2 row P16.16; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 44_p16_16_capacity_market_records.md`.
//
// WHAT THIS IS (decided once):
//
// - **RECORD-ONLY depth** (plan §2 class note): checklists and sourced
//   dates, prepared as records — never a qualification. No checklist
//   state, transition, or vocabulary implies qualification (there is
//   NO QUALIFIED state anywhere in this module — structural). No
//   qualification is claimed, no filing is prepared for submission,
//   no auction is entered.
// - **Every date is SOURCED DATA with the re-verify-at-use rule** (plan
//   risk R-P16-1; the Phase 9 §7 rule inherited): a date enters this
//   module only through [`SourcedDate`], whose constructor REQUIRES
//   the source label, the verification date (the research cut-off or a
//   later human check), and the re-verify flag — a date without the
//   rule is UNCONSTRUCTABLE. No date is baked into logic anywhere.
// - **The LR-075 PJM date conflict is resolved BY RECORD, not by
//   silently picking a date**: the record below carries BOTH sourced
//   dates with their sources and states the open verification task.
//   Neither date is selected; the conflict is re-verified against
//   PJM's tariff before any use (the Phase 9 §7 rule verbatim).
// - **Every checklist item is requirement TEXT recorded from a named
//   source** (the Phase 9 scope rule: a rule enters only from program
//   documentation — where the detail is not verified here it is
//   labeled UNVERIFIED, never invented). Item evidence starts EMPTY
//   everywhere: no evidence exists, so every item starts UNSATISFIED.
// - **No market is connected, no qualification is claimed** (ADR-011;
//   conventions §7.3): the checklists are preparation records for the
//   human-gated qualification processes.
//
// Control status: records for the simulation-only 0.x line. Nothing
// here enters an auction, files anything, or claims qualification.

use std::collections::BTreeMap;

use flexcore_domain::market_program_model::AuditChainLink;

/// The capacity-market-records schema version (a change is a NEW
/// version, never a silent edit).
pub const CAPACITY_MARKET_RECORDS_SCHEMA_VERSION: u32 = 1;

/// The record-only statement (pinned by tests).
pub const RECORD_ONLY_NO_QUALIFICATION_STATEMENT: &str = "Capacity-market participation \
     records are RECORD-ONLY: qualification-evidence checklists and sourced dates, \
     prepared as records. No checklist state implies qualification — there is NO \
     QUALIFIED state in any vocabulary here — and no qualification, filing, or auction \
     entry is claimed or performed anywhere.";

/// The sourced-dates statement (pinned by tests).
pub const DATES_ARE_SOURCED_DATA_STATEMENT: &str = "Every date is SOURCED DATA with the \
     re-verify-at-use rule: a date enters only through a type whose constructor REQUIRES \
     the source label, the verification date, and the re-verify flag. No date is baked \
     into logic; conflicts between sources are resolved BY RECORD (both dates carried, \
     neither selected), never by silently picking one.";

/// The research cut-off the register's dates were checked against (the
/// report's cut-off; a later human check updates the entry's
/// verification date — never silently).
pub const RESEARCH_CUTOFF: &str = "2026-11-07";

/// The LR-075 conflict-record identifier (the context's resolution
/// disposition: "Resolved by record with sourced dates (P16.16),
/// re-verified at use").
pub const LR_075_CONFLICT_RECORD_ID: &str = "LR-075-PJM-CAPACITY-DATE-CONFLICT";

// --- Errors ---------------------------------------------------------------------

/// Why a capacity-market-record operation was refused. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapacityRecordError {
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
    /// A sourced date was built with `reverify_at_use == false` — a
    /// date without the re-verify rule is UNCONSTRUCTABLE.
    ReverifyFlagRequired,
    /// A checklist item carries no source label (a requirement enters
    /// only from a named source — the Phase 9 scope rule).
    SourceLabelRequired,
    /// An evidence record on an item whose evidence reference is empty
    /// (recording evidence requires the reference).
    EvidenceRefRequired,
    /// A duplicate identity was registered.
    DuplicateIdentity {
        record_id: String,
    },
    /// The checklist was not found.
    ChecklistNotFound {
        checklist_id: String,
    },
    /// A domain shared-type constructor refused (carried).
    Domain(flexcore_domain::market_program_model::MarketProgramModelError),
}

impl CapacityRecordError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            CapacityRecordError::EmptyField { .. } => "CAPACITY_RECORD_EMPTY_FIELD",
            CapacityRecordError::FieldTooLong { .. } => "CAPACITY_RECORD_FIELD_TOO_LONG",
            CapacityRecordError::ReverifyFlagRequired => "CAPACITY_RECORD_REVERIFY_FLAG_REQUIRED",
            CapacityRecordError::SourceLabelRequired => "CAPACITY_RECORD_SOURCE_LABEL_REQUIRED",
            CapacityRecordError::EvidenceRefRequired => "CAPACITY_RECORD_EVIDENCE_REF_REQUIRED",
            CapacityRecordError::DuplicateIdentity { .. } => "CAPACITY_RECORD_DUPLICATE_IDENTITY",
            CapacityRecordError::ChecklistNotFound { .. } => "CAPACITY_RECORD_CHECKLIST_NOT_FOUND",
            CapacityRecordError::Domain(_) => "CAPACITY_RECORD_DOMAIN_ERROR",
        }
    }
}

impl std::fmt::Display for CapacityRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapacityRecordError::EmptyField { field } => {
                write!(f, "the capacity-record field '{field}' must be non-empty")
            }
            CapacityRecordError::FieldTooLong { field, len, max } => write!(
                f,
                "the capacity-record field '{field}' is {len} characters; the bound is {max}"
            ),
            CapacityRecordError::ReverifyFlagRequired => write!(
                f,
                "a sourced date with reverify_at_use = false is unconstructable: the \
                 re-verify-at-use rule is mandatory on every date (plan risk R-P16-1)"
            ),
            CapacityRecordError::SourceLabelRequired => write!(
                f,
                "a checklist requirement carries no source label; a rule enters only \
                 from a named source (the Phase 9 scope rule)"
            ),
            CapacityRecordError::EvidenceRefRequired => write!(
                f,
                "recording evidence on a checklist item requires a non-empty evidence \
                 reference; no evidence exists by default and none is invented"
            ),
            CapacityRecordError::DuplicateIdentity { record_id } => write!(
                f,
                "the capacity-record identity {record_id} is already registered"
            ),
            CapacityRecordError::ChecklistNotFound { checklist_id } => {
                write!(f, "the checklist {checklist_id} was not found")
            }
            CapacityRecordError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
        }
    }
}

impl std::error::Error for CapacityRecordError {}

// --- Shared helpers ---------------------------------------------------------------

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), CapacityRecordError> {
    if value.trim().is_empty() {
        return Err(CapacityRecordError::EmptyField { field });
    }
    if value.len() > max {
        return Err(CapacityRecordError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

// --- The sourced date ----------------------------------------------------------------

/// One sourced date: the date TEXT exactly as published, the source
/// label (an [S-n] citation id or a ledger-record path), the date the
/// value was verified (the research cut-off or a later human check),
/// and the MANDATORY re-verify-at-use flag. A date is CONSTRUCTED ONLY
/// through [`SourcedDate::new`], which refuses
/// `reverify_at_use == false` — the rule is structural, not
/// conventional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcedDate {
    date_text: String,
    source_label: String,
    verified_on: String,
    reverify_at_use: bool,
}

impl SourcedDate {
    /// Builds one sourced date. `reverify_at_use == false` is a
    /// REFUSAL — no date without the rule exists.
    pub fn new(
        date_text: String,
        source_label: String,
        verified_on: String,
        reverify_at_use: bool,
    ) -> Result<Self, CapacityRecordError> {
        validate_text("date_text", &date_text, 128)?;
        validate_text("source_label", &source_label, 256)?;
        validate_text("verified_on", &verified_on, 32)?;
        if !reverify_at_use {
            return Err(CapacityRecordError::ReverifyFlagRequired);
        }
        Ok(Self {
            date_text,
            source_label,
            verified_on,
            reverify_at_use,
        })
    }

    pub fn date_text(&self) -> &str {
        &self.date_text
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    pub fn verified_on(&self) -> &str {
        &self.verified_on
    }

    pub fn reverify_at_use(&self) -> bool {
        self.reverify_at_use
    }
}

// --- The date-conflict record ----------------------------------------------------------

/// One recorded conflict between SOURCED DATES (the LR-075 pattern,
/// generalized): BOTH dates are carried with their sources, NEITHER is
/// selected, and the conflict is re-verified at use against the
/// authority the record names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateConflictRecord {
    conflict_id: String,
    description: String,
    /// The conflicting sourced dates (at least two — a conflict over
    /// one date is not a conflict).
    dates: Vec<SourcedDate>,
    /// The authority the conflict is re-verified against before any
    /// use (e.g. "PJM's tariff" — recorded text).
    reverify_authority: String,
}

impl DateConflictRecord {
    /// Builds one conflict record (at least two sourced dates; both
    /// carry the mandatory re-verify rule).
    pub fn new(
        conflict_id: String,
        description: String,
        dates: Vec<SourcedDate>,
        reverify_authority: String,
    ) -> Result<Self, CapacityRecordError> {
        validate_text("conflict_id", &conflict_id, 128)?;
        validate_text("description", &description, 1024)?;
        validate_text("reverify_authority", &reverify_authority, 256)?;
        if dates.len() < 2 {
            return Err(CapacityRecordError::EmptyField {
                field: "dates (a conflict carries at least two sourced dates)",
            });
        }
        Ok(Self {
            conflict_id,
            description,
            dates,
            reverify_authority,
        })
    }

    pub fn conflict_id(&self) -> &str {
        &self.conflict_id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// The conflicting sourced dates (as recorded — NEITHER selected).
    pub fn dates(&self) -> &[SourcedDate] {
        &self.dates
    }

    pub fn reverify_authority(&self) -> &str {
        &self.reverify_authority
    }
}

/// The LR-075 PJM capacity-market date conflict, RESOLVED BY RECORD
/// (the phase-16 context disposition): both sourced dates carried,
/// neither selected, re-verified against PJM's tariff before use.
///
/// The two sides of the conflict:
/// - **"February 2027 (proposed)"** — the FERC 2222 timeline row of
///   the gap analysis (`docs/phases-11-20/00_GAP_ANALYSIS.md` §4, the
///   [S-26] class), which lists PJM as "Feb 2027/28".
/// - **"1 February 2028 (energy participation expected)"** — the
///   Phase 9 capture template §7 (`ledger/
///   phase-09-pilot-qualification/11_market_program_requirements.md`),
///   checked 2026-11-07, which records the PJM capacity-market date as
///   CONFLICTING BETWEEN SOURCES and names it an OPEN VERIFICATION
///   task.
///
/// Both are carried verbatim with their sources; the conflict stands
/// until a human re-verification against PJM's tariff [S-90] closes
/// it. No code path, checklist item, or register entry may select one
/// side.
pub fn lr_075_pjm_capacity_date_conflict() -> DateConflictRecord {
    let first = SourcedDate::new(
        "February 2027 (proposed)".to_string(),
        "docs/phases-11-20/00_GAP_ANALYSIS.md section 4 timeline row [S-26] (PJM Feb 2027/28)"
            .to_string(),
        RESEARCH_CUTOFF.to_string(),
        true,
    )
    .expect("sourced date with the mandatory re-verify rule");
    let second = SourcedDate::new(
        "1 February 2028 (energy participation expected)".to_string(),
        "ledger/phase-09-pilot-qualification/11_market_program_requirements.md section 7 \
         (checked 2026-11-07)"
            .to_string(),
        RESEARCH_CUTOFF.to_string(),
        true,
    )
    .expect("sourced date with the mandatory re-verify rule");
    DateConflictRecord::new(
        LR_075_CONFLICT_RECORD_ID.to_string(),
        "The PJM capacity-market participation date conflicts between the program's \
         recorded sources: the gap-analysis timeline row carries February 2027 \
         (proposed) and the Phase 9 capture template carries 1 February 2028 (energy) \
         with the capacity date explicitly conflicting between sources. Both dates are \
         carried as sourced data; NEITHER is selected by any code path; the conflict is \
         re-verified against PJM's tariff before any use (plan risk R-P16-1; the LR-075 \
         disposition in the phase-16 context)."
            .to_string(),
        vec![first, second],
        "PJM's tariff and FERC filings (primary sources — [S-90] class)".to_string(),
    )
    .expect("the recorded conflict")
}

// --- The closed market vocabulary -------------------------------------------------------

/// The closed capacity-market vocabulary (the record packs this module
/// names; a fourth market is a NEW pack — ADR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapacityMarket {
    /// PJM Reliability Pricing Model (capacity auctions).
    PjmRpm,
    /// ISO-NE Forward Capacity Auction 19 (February 2026 [S-103]).
    IsoNeFca19,
    /// ISO-NE Distributed Energy Capacity Resource (the Order 2222
    /// capacity-market participation class [S-103]; details UNVERIFIED
    /// here — re-verified at use).
    IsoNeDecr,
}

impl CapacityMarket {
    /// The canonical name.
    pub const fn name(self) -> &'static str {
        match self {
            CapacityMarket::PjmRpm => "PJM_RPM",
            CapacityMarket::IsoNeFca19 => "ISO_NE_FCA19",
            CapacityMarket::IsoNeDecr => "ISO_NE_DECR",
        }
    }
}

// --- The checklist -----------------------------------------------------------------------

/// The closed checklist-item vocabulary. There is NO QUALIFIED state:
/// an item is UNSATISFIED (no evidence) or RECORDED (evidence recorded
/// as data). Recording evidence is a RECORD — never a validated claim
/// (the P16.03 attestation rule inherited).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecklistItemState {
    /// No evidence exists for the item (the starting state of EVERY
    /// item).
    Unsatisfied,
    /// Evidence was recorded against the item (a reference, carried as
    /// data — never a validated claim).
    EvidenceRecorded,
}

impl ChecklistItemState {
    pub const fn name(self) -> &'static str {
        match self {
            ChecklistItemState::Unsatisfied => "UNSATISFIED",
            ChecklistItemState::EvidenceRecorded => "EVIDENCE_RECORDED",
        }
    }
}

/// One checklist item: the requirement text recorded from a named
/// source, its state, and — when evidence exists — the evidence
/// reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationChecklistItem {
    item_id: String,
    requirement_text: String,
    /// The source label the requirement was recorded from (mandatory —
    /// the Phase 9 scope rule).
    source_label: String,
    state: ChecklistItemState,
    evidence_ref: Option<String>,
    recorded_at_unix_ms: i64,
}

impl QualificationChecklistItem {
    /// Builds one UNSATISFIED item (requirement text + mandatory
    /// source label; no evidence exists).
    pub fn new(
        item_id: String,
        requirement_text: String,
        source_label: String,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, CapacityRecordError> {
        validate_text("item_id", &item_id, 64)?;
        validate_text("requirement_text", &requirement_text, 1024)?;
        if source_label.trim().is_empty() {
            return Err(CapacityRecordError::SourceLabelRequired);
        }
        if source_label.len() > 256 {
            return Err(CapacityRecordError::FieldTooLong {
                field: "source_label",
                len: source_label.len(),
                max: 256,
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(CapacityRecordError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            item_id,
            requirement_text,
            source_label,
            state: ChecklistItemState::Unsatisfied,
            evidence_ref: None,
            recorded_at_unix_ms,
        })
    }

    /// Records evidence against the item (a non-empty reference,
    /// carried as data — never a validated claim).
    pub fn record_evidence(&mut self, evidence_ref: String) -> Result<(), CapacityRecordError> {
        if evidence_ref.trim().is_empty() {
            return Err(CapacityRecordError::EvidenceRefRequired);
        }
        self.evidence_ref = Some(evidence_ref);
        self.state = ChecklistItemState::EvidenceRecorded;
        Ok(())
    }

    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    pub fn requirement_text(&self) -> &str {
        &self.requirement_text
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    pub fn state(&self) -> ChecklistItemState {
        self.state
    }

    pub fn evidence_ref(&self) -> Option<&str> {
        self.evidence_ref.as_deref()
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

/// One qualification-evidence CHECKLIST for one capacity market:
/// requirement items recorded from named sources, every item starting
/// UNSATISFIED. A checklist implies NOTHING about qualification — the
/// statement above is structural (no QUALIFIED state exists).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityQualificationChecklist {
    checklist_id: String,
    tenant_id: String,
    market: CapacityMarket,
    items: BTreeMap<String, QualificationChecklistItem>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl CapacityQualificationChecklist {
    /// Builds one checklist from its items (non-empty; every item
    /// starts UNSATISFIED — the constructor refuses an item already
    /// carrying evidence, keeping the record-only honesty at
    /// construction time: evidence is recorded through the store after
    /// a human review, never smuggled in).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        checklist_id: String,
        tenant_id: String,
        market: CapacityMarket,
        items: Vec<QualificationChecklistItem>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, CapacityRecordError> {
        validate_text("checklist_id", &checklist_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(CapacityRecordError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        if items.is_empty() {
            return Err(CapacityRecordError::EmptyField {
                field: "items (a checklist carries at least one recorded requirement)",
            });
        }
        let mut map = BTreeMap::new();
        for item in items {
            if map.insert(item.item_id().to_string(), item).is_some() {
                return Err(CapacityRecordError::DuplicateIdentity {
                    record_id: format!("checklist item under {checklist_id}"),
                });
            }
        }
        Ok(Self {
            checklist_id,
            tenant_id,
            market,
            items: map,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn checklist_id(&self) -> &str {
        &self.checklist_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn market(&self) -> CapacityMarket {
        self.market
    }

    /// The items (ascending item id — deterministic).
    pub fn items(&self) -> &BTreeMap<String, QualificationChecklistItem> {
        &self.items
    }

    /// The mutable item accessor for the evidence transition.
    pub fn item_mut(
        &mut self,
        item_id: &str,
    ) -> Option<&mut QualificationChecklistItem> {
        self.items.get_mut(item_id)
    }

    /// Whether EVERY item carries recorded evidence. This is a fact
    /// about the RECORDS — it implies NOTHING about qualification
    /// (the statement above is structural).
    pub fn all_evidence_recorded(&self) -> bool {
        self.items
            .values()
            .all(|item| item.state() == ChecklistItemState::EvidenceRecorded)
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

// --- The record pack builders (DECLARED data with named sources) --------------------------

/// The PJM RPM qualification-evidence checklist items, RECORDED from
/// the program's named sources. Every requirement carries its source;
/// items whose detail is not verified in this repository are labeled
/// UNVERIFIED and re-verified at use.
pub fn pjm_rpm_checklist_items() -> Vec<QualificationChecklistItem> {
    vec![
        QualificationChecklistItem::new(
            "RPM-1".to_string(),
            "RERRA concurrence for the DER aggregation jurisdiction recorded before any \
             capacity participation preparation (the PJM Order 2222 participation-model \
             overview names the RERRA requirement)"
                .to_string(),
            "[S-90] PJM FERC 2222 participation model overview".to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
        QualificationChecklistItem::new(
            "RPM-2".to_string(),
            "The PJM capacity-market participation date is the LR-075 CONFLICT record \
             (both sourced dates carried, neither selected) — re-verified against \
             PJM's tariff before any use"
                .to_string(),
            "LR-075 conflict record (P16.16); ledger/phase-09-pilot-qualification/\
             11_market_program_requirements.md section 7"
                .to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
        QualificationChecklistItem::new(
            "RPM-3".to_string(),
            "Telemetry posture for >= 10 MW aggregations declared per the PJM Order \
             2222 overview (the cadence detail is UNVERIFIED here — re-verified at use \
             against the primary source)"
                .to_string(),
            "[S-90] PJM FERC 2222 participation model overview (detail UNVERIFIED)"
                .to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
    ]
}

/// The ISO-NE FCA19 qualification-evidence checklist items.
pub fn iso_ne_fca19_checklist_items() -> Vec<QualificationChecklistItem> {
    vec![
        QualificationChecklistItem::new(
            "FCA19-1".to_string(),
            "FCA19 timing recorded as SOURCED DATA: February 2026 (the ISO-NE Order \
             2222 participant-readiness source names the Forward Capacity Auction \
             schedule) — re-verified at use"
                .to_string(),
            "[S-103] ISO-NE Order 2222 participant readiness".to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
        QualificationChecklistItem::new(
            "FCA19-2".to_string(),
            "The metering and telemetry checklist the ISO-NE Order 2222 material names \
             for capacity participation is inventoried (its line items are UNVERIFIED \
             here — re-verified at use against the primary checklist document)"
                .to_string(),
            "[S-104] ISO-NE metering & telemetry checklist (detail UNVERIFIED)"
                .to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
    ]
}

/// The ISO-NE DECR qualification-evidence checklist items.
pub fn iso_ne_decr_checklist_items() -> Vec<QualificationChecklistItem> {
    vec![
        QualificationChecklistItem::new(
            "DECR-1".to_string(),
            "The Distributed Energy Capacity Resource (DECR) qualification \
             requirements are inventoried as a RECORD-ONLY checklist (the requirement \
             detail is UNVERIFIED here — re-verified against ISO-NE's primary Order \
             2222 materials before any use)"
                .to_string(),
            "[S-103] ISO-NE Order 2222 participant readiness (detail UNVERIFIED)"
                .to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
        QualificationChecklistItem::new(
            "DECR-2".to_string(),
            "Energy and ancillary-services participation targeted/effective 1 November \
             2026 is recorded as SOURCED DATA — re-verified at use"
                .to_string(),
            "[S-103] ISO-NE Order 2222 participant readiness; \
             docs/phases-11-20/00_GAP_ANALYSIS.md section 4"
                .to_string(),
            1_700_000_000_000,
        )
        .expect("recorded requirement"),
    ]
}

// --- The checklist store -------------------------------------------------------------------

/// The BTreeMap-backed checklist store: register-only, deterministic,
/// no update, no delete. Evidence recording goes through
/// [`CapacityQualificationChecklist::item_mut`] after the checklist is
/// registered — a human-gated record act, never a qualification.
#[derive(Debug, Default)]
pub struct CapacityChecklistStore {
    checklists: BTreeMap<(String, String), CapacityQualificationChecklist>,
    conflicts: BTreeMap<(String, String), DateConflictRecord>,
}

impl CapacityChecklistStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one checklist.
    pub fn register_checklist(
        &mut self,
        checklist: CapacityQualificationChecklist,
    ) -> Result<(), CapacityRecordError> {
        let key = (
            checklist.tenant_id().to_string(),
            checklist.checklist_id().to_string(),
        );
        if self.checklists.contains_key(&key) {
            return Err(CapacityRecordError::DuplicateIdentity {
                record_id: checklist.checklist_id().to_string(),
            });
        }
        self.checklists.insert(key, checklist);
        Ok(())
    }

    /// The checklist lookup.
    pub fn checklist(
        &self,
        tenant_id: &str,
        checklist_id: &str,
    ) -> Option<&CapacityQualificationChecklist> {
        self.checklists
            .get(&(tenant_id.to_string(), checklist_id.to_string()))
    }

    /// The mutable checklist accessor for evidence recording.
    pub fn checklist_mut(
        &mut self,
        tenant_id: &str,
        checklist_id: &str,
    ) -> Option<&mut CapacityQualificationChecklist> {
        self.checklists
            .get_mut(&(tenant_id.to_string(), checklist_id.to_string()))
    }

    /// Records one date-conflict record (both dates carried; neither
    /// selected).
    pub fn register_conflict(
        &mut self,
        conflict: DateConflictRecord,
    ) -> Result<(), CapacityRecordError> {
        let key = (
            "program".to_string(),
            conflict.conflict_id().to_string(),
        );
        if self.conflicts.contains_key(&key) {
            return Err(CapacityRecordError::DuplicateIdentity {
                record_id: conflict.conflict_id().to_string(),
            });
        }
        self.conflicts.insert(key, conflict);
        Ok(())
    }

    /// The recorded conflicts (ascending conflict id — deterministic).
    pub fn conflicts(&self) -> Vec<&DateConflictRecord> {
        self.conflicts.values().collect()
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture value is synthetic;
// every date is sourced data with the mandatory re-verify rule.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(RECORD_ONLY_NO_QUALIFICATION_STATEMENT.contains("RECORD-ONLY"));
        assert!(RECORD_ONLY_NO_QUALIFICATION_STATEMENT.contains("NO QUALIFIED state"));
        assert!(DATES_ARE_SOURCED_DATA_STATEMENT.contains("re-verify-at-use"));
        assert!(DATES_ARE_SOURCED_DATA_STATEMENT.contains("resolved BY RECORD"));
    }

    #[test]
    fn a_date_without_the_reverify_rule_is_unconstructable() {
        let err = SourcedDate::new(
            "1 November 2024".to_string(),
            "[S-26]".to_string(),
            RESEARCH_CUTOFF.to_string(),
            false,
        )
        .expect_err("flag mandatory");
        assert_eq!(err.name(), "CAPACITY_RECORD_REVERIFY_FLAG_REQUIRED");
        let date = SourcedDate::new(
            "1 November 2024".to_string(),
            "[S-26] FERC Order 2222 explainer; [S-97] CAISO Tariff section 11".to_string(),
            RESEARCH_CUTOFF.to_string(),
            true,
        )
        .expect("constructed with the rule");
        assert!(date.reverify_at_use());
    }

    #[test]
    fn the_lr_075_conflict_carries_both_dates_and_neither_is_selected() {
        let conflict = lr_075_pjm_capacity_date_conflict();
        assert_eq!(conflict.conflict_id(), LR_075_CONFLICT_RECORD_ID);
        assert_eq!(conflict.dates().len(), 2);
        assert!(conflict.dates()[0].date_text().contains("February 2027 (proposed)"));
        assert!(conflict.dates()[1].date_text().contains("1 February 2028"));
        // Both sides carry the mandatory re-verify rule.
        for date in conflict.dates() {
            assert!(date.reverify_at_use());
        }
        // The record names the re-verification authority and does NOT
        // carry a selection field at all (structural: there is none).
        assert!(conflict.reverify_authority().contains("PJM's tariff"));
        assert!(
            conflict.description().contains("NEITHER is selected"),
            "the resolution is by record, never by silently picking a date"
        );
    }

    #[test]
    fn a_conflict_with_one_date_is_not_a_conflict() {
        let one = SourcedDate::new(
            "x".to_string(),
            "[S-26]".to_string(),
            RESEARCH_CUTOFF.to_string(),
            true,
        )
        .expect("date");
        assert_eq!(
            DateConflictRecord::new(
                "c-1".to_string(),
                "d".to_string(),
                vec![one],
                "authority".to_string()
            )
            .expect_err("one date is not a conflict")
            .name(),
            "CAPACITY_RECORD_EMPTY_FIELD"
        );
    }

    #[test]
    fn every_item_starts_unsatisfied_and_evidence_requires_a_reference() {
        let items = pjm_rpm_checklist_items();
        assert_eq!(items.len(), 3);
        for item in &items {
            assert_eq!(item.state(), ChecklistItemState::Unsatisfied);
            assert!(item.evidence_ref().is_none());
        }
        let mut item = pjm_rpm_checklist_items().remove(0);
        assert_eq!(
            item.record_evidence(String::new())
                .expect_err("empty evidence ref refused")
                .name(),
            "CAPACITY_RECORD_EVIDENCE_REF_REQUIRED"
        );
        item.record_evidence("evidence-ref-synthetic".to_string())
            .expect("evidence recorded");
        assert_eq!(item.state(), ChecklistItemState::EvidenceRecorded);
    }

    #[test]
    fn a_requirement_without_a_source_label_is_refused() {
        assert_eq!(
            QualificationChecklistItem::new(
                "X-1".to_string(),
                "requirement".to_string(),
                String::new(),
                0
            )
            .expect_err("source mandatory")
            .name(),
            "CAPACITY_RECORD_SOURCE_LABEL_REQUIRED"
        );
    }

    #[test]
    fn the_market_vocabulary_is_exactly_three_markets() {
        assert_eq!(CapacityMarket::PjmRpm.name(), "PJM_RPM");
        assert_eq!(CapacityMarket::IsoNeFca19.name(), "ISO_NE_FCA19");
        assert_eq!(CapacityMarket::IsoNeDecr.name(), "ISO_NE_DECR");
    }

    #[test]
    fn checklists_register_and_all_evidence_recorded_is_a_record_fact_only() {
        let mut store = CapacityChecklistStore::new();
        let checklist = CapacityQualificationChecklist::new(
            "cl-1".to_string(),
            "tenant-1".to_string(),
            CapacityMarket::IsoNeFca19,
            iso_ne_fca19_checklist_items(),
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect("checklist");
        assert!(!checklist.all_evidence_recorded());
        store.register_checklist(checklist).expect("registered");
        // Record evidence on every item (a record act, not a claim).
        let item_ids: Vec<String> = store
            .checklist("tenant-1", "cl-1")
            .expect("found")
            .items()
            .keys()
            .cloned()
            .collect();
        for item_id in item_ids {
            let checklist_mut = store.checklist_mut("tenant-1", "cl-1").expect("found");
            let item = checklist_mut.item_mut(&item_id).expect("item");
            item.record_evidence(format!("evidence-ref-synthetic-{item_id}"))
                .expect("evidence recorded");
        }
        let checklist = store.checklist("tenant-1", "cl-1").expect("found");
        assert!(checklist.all_evidence_recorded());
        // ...and the fact stays a RECORD fact: no QUALIFIED state
        // exists anywhere in the module's vocabularies.
        for state in [ChecklistItemState::Unsatisfied, ChecklistItemState::EvidenceRecorded] {
            assert!(state.name() != "QUALIFIED");
        }
        for market in [
            CapacityMarket::PjmRpm,
            CapacityMarket::IsoNeFca19,
            CapacityMarket::IsoNeDecr,
        ] {
            assert!(market.name() != "QUALIFIED");
        }
    }

    #[test]
    fn duplicate_checklist_identities_are_refused() {
        let mut store = CapacityChecklistStore::new();
        let checklist = CapacityQualificationChecklist::new(
            "cl-2".to_string(),
            "tenant-1".to_string(),
            CapacityMarket::PjmRpm,
            pjm_rpm_checklist_items(),
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect("checklist");
        store.register_checklist(checklist.clone()).expect("registered");
        assert_eq!(
            store
                .register_checklist(checklist)
                .expect_err("duplicate")
                .name(),
            "CAPACITY_RECORD_DUPLICATE_IDENTITY"
        );
    }

    #[test]
    fn the_lr_075_conflict_registers_once() {
        let mut store = CapacityChecklistStore::new();
        store
            .register_conflict(lr_075_pjm_capacity_date_conflict())
            .expect("registered");
        assert_eq!(
            store
                .register_conflict(lr_075_pjm_capacity_date_conflict())
                .expect_err("duplicate")
                .name(),
            "CAPACITY_RECORD_DUPLICATE_IDENTITY"
        );
        assert_eq!(store.conflicts().len(), 1);
    }
}
