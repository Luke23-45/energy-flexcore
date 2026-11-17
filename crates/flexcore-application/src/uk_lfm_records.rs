// P16.17 — UK/EU Local Flexibility Market (LFM) integration design
//         records: the Piclo registration/tender/utilization/settlement
//         flow DESIGNS, availability- and utilization-payment RECORD
//         types, and the P483 dual-participation stacking RECORDS
//         ([S-36] Piclo; [S-37] ENA flexibility overview; [S-114] NGED
//         flexibility procurement report 2025-26; [S-115] Elexon P483,
//         live 27 November 2025).
//
// Phase 16 plan §2 row P16.17; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 45_p16_17_uk_eu_lfm_records.md`.
//
// WHAT THIS IS (decided once):
//
// - **Flow DESIGNS, never executions**: the Piclo LFM journey is
//   recorded as four design stages (REGISTRATION → TENDER →
//   UTILIZATION → SETTLEMENT), each stage a design record naming its
//   inputs, outputs, and the platform record it produces. No tender is
//   placed, no registration is filed, no utilization is offered — the
//   stages carry the DECLARED status only, and no state vocabulary in
//   this module can represent an executed stage (structural).
// - **Payment records are RECORDED data, never issued payments**: the
//   availability-payment and utilization-payment record types carry
//   the amounts the UK LFM structure pays for (availability paid for
//   OFFERED availability; utilization paid for DELIVERED response
//   [S-36][S-114]) as exact fixed-decimal money with currency as data.
//   No payment record exists anywhere: no market is connected, no
//   tender is placed, no payment has been made or received. Fixtures
//   in the tests are synthetic.
// - **The P483 dual-participation stacking records mirror the
//   enrollment refusal** (plan risk R-P16-7): P483 (the Elexon dual
//   participation arrangements, live 27 November 2025 [S-115]) is
//   RECORDED DATA with the re-verify-at-use rule; a stacking
//   determination names the P16.02 `StackingRule` it applies, and this
//   module NEVER widens a rule and never allows a violating
//   combination — the refusal lives in the P16.02 enrollment store
//   (`MarketEnrollmentStore` refuses a violating PARTICIPATING
//   enrollment by `StackingRuleViolation`), consumed by citation.
// - **Money is the fixed-decimal layer** (plan §4; hard ban 9): every
//   amount is a [`MoneyAmount`] (exact i128 mantissa × 10^−scale +
//   currency code as data); no money arithmetic in floats anywhere.
// - **Every date is SOURCED DATA with the re-verify-at-use rule** (the
//   P16.16 `SourcedDate` type is reused — one home for the rule).
//
// Determinism: BTreeMap-backed stores, deterministic ordering, no
// wall-clock input.
//
// Control status: records for the simulation-only 0.x line. No LFM is
// contacted, no tender is placed, no availability is offered, and no
// payment is made or received anywhere.

use std::collections::BTreeMap;

use flexcore_domain::market_program_model::{AuditChainLink, ParticipationWindow};

use crate::capacity_market_records::SourcedDate;
use crate::fixed_decimal::MoneyAmount;

/// The UK/EU LFM records schema version (a change is a NEW version,
/// never a silent edit).
pub const UK_LFM_RECORDS_SCHEMA_VERSION: u32 = 1;

/// The flow-design statement (pinned by tests).
pub const FLOW_DESIGNS_NEVER_EXECUTED_STATEMENT: &str = "The Piclo LFM flow is recorded \
     as four DESIGN stages (REGISTRATION, TENDER, UTILIZATION, SETTLEMENT) carrying the \
     DECLARED status only. No registration is filed, no tender is placed, no \
     utilization is offered, and no settlement run is executed anywhere — no state \
     vocabulary in this module can represent an executed stage.";

/// The payment-record statement (pinned by tests).
pub const NO_PAYMENT_RECORD_EXISTS_STATEMENT: &str = "No availability- or \
     utilization-payment record exists anywhere: no LFM is contacted, no tender is \
     placed, and no payment has been made or received. The record types carry amounts \
     as exact fixed-decimal money with currency as data — never floats; fixtures in \
     the tests are synthetic.";

/// The P483 stacking statement (pinned by tests).
pub const P483_STACKING_REFUSES_STATEMENT: &str = "P483 dual-participation stacking \
     records are recorded DATA naming the P16.02 stacking rule they apply; the module \
     never widens a rule and never allows a violating combination — the refusal lives \
     in the P16.02 enrollment store (StackingRuleViolation), consumed by citation \
     (plan risk R-P16-7).";

// --- Errors ---------------------------------------------------------------------

/// Why a UK/EU LFM operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfmError {
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
    /// A domain shared-type constructor refused (carried).
    Domain(flexcore_domain::market_program_model::MarketProgramModelError),
    /// A decimal/money operation refused (carried from the
    /// fixed-decimal layer).
    Money(crate::fixed_decimal::FixedDecimalError),
    /// A design record was attempted at a stage whose predecessor has
    /// no design record (the flow is recorded in stage order — the
    /// design discipline, not an execution claim).
    FlowStageOrder {
        expected_predecessor: &'static str,
        given_stage: &'static str,
    },
    /// A stacking record names no P16.02 stacking rule (a stacking
    /// determination without the recorded rule it applies is refused).
    StackingRuleRefRequired,
    /// A stacking record claims a determination WITHOUT the rule's
    /// refusal path being the mechanism (the module records the rule
    /// application; it never records an "allowed" override of a
    /// recorded rule).
    StackingRuleCannotBeWidened {
        rule_id: String,
    },
    /// A duplicate identity was registered.
    DuplicateIdentity {
        record_id: String,
    },
}

impl LfmError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            LfmError::EmptyField { .. } => "LFM_EMPTY_FIELD",
            LfmError::FieldTooLong { .. } => "LFM_FIELD_TOO_LONG",
            LfmError::Domain(_) => "LFM_DOMAIN_ERROR",
            LfmError::Money(_) => "LFM_MONEY_ERROR",
            LfmError::FlowStageOrder { .. } => "LFM_FLOW_STAGE_ORDER",
            LfmError::StackingRuleRefRequired => "LFM_STACKING_RULE_REF_REQUIRED",
            LfmError::StackingRuleCannotBeWidened { .. } => "LFM_STACKING_RULE_CANNOT_BE_WIDENED",
            LfmError::DuplicateIdentity { .. } => "LFM_DUPLICATE_IDENTITY",
        }
    }
}

impl std::fmt::Display for LfmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LfmError::EmptyField { field } => {
                write!(f, "the LFM record field '{field}' must be non-empty")
            }
            LfmError::FieldTooLong { field, len, max } => write!(
                f,
                "the LFM record field '{field}' is {len} characters; the bound is {max}"
            ),
            LfmError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
            LfmError::Money(err) => write!(f, "money operation refused: {err}"),
            LfmError::FlowStageOrder {
                expected_predecessor,
                given_stage,
            } => write!(
                f,
                "the flow design for stage {given_stage} requires its predecessor \
                 {expected_predecessor} to carry a design record first (the design \
                 discipline — never an execution claim)"
            ),
            LfmError::StackingRuleRefRequired => write!(
                f,
                "a stacking determination names no recorded stacking rule; a P483 \
                 stacking record without the rule it applies is refused"
            ),
            LfmError::StackingRuleCannotBeWidened { rule_id } => write!(
                f,
                "the stacking record {rule_id} cannot be widened or overridden: the \
                 module records the rule application, and the refusal lives in the \
                 P16.02 enrollment store"
            ),
            LfmError::DuplicateIdentity { record_id } => {
                write!(f, "the LFM record identity {record_id} is already registered")
            }
        }
    }
}

impl std::error::Error for LfmError {}

// --- Shared helpers ---------------------------------------------------------------

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), LfmError> {
    if value.trim().is_empty() {
        return Err(LfmError::EmptyField { field });
    }
    if value.len() > max {
        return Err(LfmError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

// --- The Piclo flow-design stages ----------------------------------------------------

/// The closed flow-stage vocabulary (the Piclo journey as four design
/// stages; a fifth stage is a NEW design decision, never an extension
/// of this enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LfmFlowStage {
    /// Registration with the DSO's flexibility service (via the Piclo
    /// platform [S-36]).
    Registration,
    /// Tender response into a flexibility procurement round ([S-36]
    /// [S-114]).
    Tender,
    /// Availability and utilization delivery over the contract term
    /// ([S-114]).
    Utilization,
    /// Settlement of availability and utilization payments ([S-114]).
    Settlement,
}

impl LfmFlowStage {
    /// The canonical name.
    pub const fn name(self) -> &'static str {
        match self {
            LfmFlowStage::Registration => "REGISTRATION",
            LfmFlowStage::Tender => "TENDER",
            LfmFlowStage::Utilization => "UTILIZATION",
            LfmFlowStage::Settlement => "SETTLEMENT",
        }
    }

    /// The design-order predecessor (None for REGISTRATION — the flow
    /// is recorded in stage order).
    pub const fn predecessor(self) -> Option<LfmFlowStage> {
        match self {
            LfmFlowStage::Registration => None,
            LfmFlowStage::Tender => Some(LfmFlowStage::Registration),
            LfmFlowStage::Utilization => Some(LfmFlowStage::Tender),
            LfmFlowStage::Settlement => Some(LfmFlowStage::Utilization),
        }
    }
}

/// One flow-stage DESIGN record: the stage's declared inputs, outputs,
/// and the platform record it produces — design data only, status
/// DECLARED. No state in this module represents an executed stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfmFlowDesignRecord {
    design_id: String,
    tenant_id: String,
    stage: LfmFlowStage,
    /// The declared inputs of the stage (BTreeMap — deterministic;
    /// recorded data).
    declared_inputs: BTreeMap<String, String>,
    /// The declared outputs of the stage (BTreeMap — deterministic).
    declared_outputs: BTreeMap<String, String>,
    /// The source label the design was recorded from (mandatory — the
    /// Phase 9 scope rule).
    source_label: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl LfmFlowDesignRecord {
    /// Builds one design record (status DECLARED, structurally — there
    /// is no executed status to reach).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        design_id: String,
        tenant_id: String,
        stage: LfmFlowStage,
        declared_inputs: BTreeMap<String, String>,
        declared_outputs: BTreeMap<String, String>,
        source_label: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, LfmError> {
        validate_text("design_id", &design_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("source_label", &source_label, 256)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(LfmError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            design_id,
            tenant_id,
            stage,
            declared_inputs,
            declared_outputs,
            source_label,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn design_id(&self) -> &str {
        &self.design_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn stage(&self) -> LfmFlowStage {
        self.stage
    }

    pub fn declared_inputs(&self) -> &BTreeMap<String, String> {
        &self.declared_inputs
    }

    pub fn declared_outputs(&self) -> &BTreeMap<String, String> {
        &self.declared_outputs
    }

    pub fn source_label(&self) -> &str {
        &self.source_label
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

// --- Availability- and utilization-payment records ---------------------------------------

/// One availability-payment RECORD: paid for OFFERED availability over
/// a window (the UK LFM structure [S-36][S-114]). Recorded data only —
/// no payment is made, received, or issued by this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityPaymentRecord {
    record_id: String,
    tenant_id: String,
    asset_id: String,
    /// The tender/contract reference the availability was offered
    /// under (recorded data).
    tender_ref: String,
    /// The availability window (end-exclusive — the §10.3 convention).
    window: ParticipationWindow,
    /// The availability amount as recorded (exact decimal + currency
    /// as data).
    amount: MoneyAmount,
    /// The recorded basis (the service terms label the amount came
    /// from — recorded data, never interpreted).
    basis_label: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl AvailabilityPaymentRecord {
    /// Builds one availability-payment record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_id: String,
        tenant_id: String,
        asset_id: String,
        tender_ref: String,
        window: ParticipationWindow,
        amount: MoneyAmount,
        basis_label: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, LfmError> {
        validate_text("record_id", &record_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("tender_ref", &tender_ref, 256)?;
        validate_text("basis_label", &basis_label, 128)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(LfmError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            record_id,
            tenant_id,
            asset_id,
            tender_ref,
            window,
            amount,
            basis_label,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn tender_ref(&self) -> &str {
        &self.tender_ref
    }

    pub fn window(&self) -> ParticipationWindow {
        self.window
    }

    pub fn amount(&self) -> &MoneyAmount {
        &self.amount
    }

    pub fn basis_label(&self) -> &str {
        &self.basis_label
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

/// One utilization-payment RECORD: paid for DELIVERED utilization
/// against a utilization event (the UK LFM structure [S-36][S-114]).
/// Recorded data only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtilizationPaymentRecord {
    record_id: String,
    tenant_id: String,
    asset_id: String,
    /// The tender/contract reference (recorded data).
    tender_ref: String,
    /// The utilization event reference (recorded data — the event the
    /// delivery responded to).
    utilization_event_ref: String,
    /// The utilization window (end-exclusive).
    window: ParticipationWindow,
    /// The utilization amount as recorded (exact decimal + currency as
    /// data).
    amount: MoneyAmount,
    /// The recorded basis (recorded data).
    basis_label: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl UtilizationPaymentRecord {
    /// Builds one utilization-payment record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_id: String,
        tenant_id: String,
        asset_id: String,
        tender_ref: String,
        utilization_event_ref: String,
        window: ParticipationWindow,
        amount: MoneyAmount,
        basis_label: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, LfmError> {
        validate_text("record_id", &record_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("tender_ref", &tender_ref, 256)?;
        validate_text("utilization_event_ref", &utilization_event_ref, 256)?;
        validate_text("basis_label", &basis_label, 128)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(LfmError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            record_id,
            tenant_id,
            asset_id,
            tender_ref,
            utilization_event_ref,
            window,
            amount,
            basis_label,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn tender_ref(&self) -> &str {
        &self.tender_ref
    }

    pub fn utilization_event_ref(&self) -> &str {
        &self.utilization_event_ref
    }

    pub fn window(&self) -> ParticipationWindow {
        self.window
    }

    pub fn amount(&self) -> &MoneyAmount {
        &self.amount
    }

    pub fn basis_label(&self) -> &str {
        &self.basis_label
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

// --- P483 dual-participation stacking records ---------------------------------------------

/// The closed stacking-determination vocabulary. The ONLY determination
/// a stacking record can carry is RULE_APPLIED (the rule was applied
/// and the enrollment store's refusal is the enforcement mechanism) —
/// there is no ALLOWED/OVERRIDE state: a recorded rule is never widened
/// (the P16.02 rule, mirrored structurally).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackingDetermination {
    /// The named stacking rule was applied to the named combination.
    RuleApplied,
}

impl StackingDetermination {
    pub const fn name(self) -> &'static str {
        match self {
            StackingDetermination::RuleApplied => "RULE_APPLIED",
        }
    }
}

/// One P483 dual-participation stacking record: the recorded
/// determination that a named stacking rule governs the named
/// combination of programs for one asset (P483 live 27 November 2025
/// [S-115] as SOURCED DATA with the re-verify-at-use rule). The
/// refusal mechanism is the P16.02 enrollment store's
/// `StackingRuleViolation` — consumed by citation, never redefined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P483StackingRecord {
    record_id: String,
    tenant_id: String,
    asset_id: String,
    /// The P16.02 `StackingRule` identity this record applies
    /// (mandatory — a determination without the rule is refused).
    stacking_rule_id: String,
    determination: StackingDetermination,
    /// The P483 sourced date (the re-verify-at-use rule is carried on
    /// the date itself — the P16.16 `SourcedDate` type reused).
    p483_date: SourcedDate,
    /// The recorded determination text (data — the jurisdiction's
    /// wording, never FlexCore's).
    determination_text: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl P483StackingRecord {
    /// Builds one stacking record (the rule reference is mandatory;
    /// the determination vocabulary is closed at RULE_APPLIED).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_id: String,
        tenant_id: String,
        asset_id: String,
        stacking_rule_id: String,
        p483_date: SourcedDate,
        determination_text: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, LfmError> {
        validate_text("record_id", &record_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("asset_id", &asset_id, 64)?;
        validate_text("determination_text", &determination_text, 1024)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if stacking_rule_id.trim().is_empty() {
            return Err(LfmError::StackingRuleRefRequired);
        }
        if stacking_rule_id.len() > 64 {
            return Err(LfmError::FieldTooLong {
                field: "stacking_rule_id",
                len: stacking_rule_id.len(),
                max: 64,
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(LfmError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            record_id,
            tenant_id,
            asset_id,
            stacking_rule_id,
            determination: StackingDetermination::RuleApplied,
            p483_date,
            determination_text,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn stacking_rule_id(&self) -> &str {
        &self.stacking_rule_id
    }

    pub fn determination(&self) -> StackingDetermination {
        self.determination
    }

    pub fn p483_date(&self) -> &SourcedDate {
        &self.p483_date
    }

    pub fn determination_text(&self) -> &str {
        &self.determination_text
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

// --- The LFM record store -------------------------------------------------------------------

/// The BTreeMap-backed LFM record store: design records (registered in
/// stage order), availability- and utilization-payment records, and
/// P483 stacking records — register-only, deterministic.
#[derive(Debug, Default)]
pub struct LfmRecordStore {
    designs: BTreeMap<(String, String), LfmFlowDesignRecord>,
    availability_payments: BTreeMap<(String, String), AvailabilityPaymentRecord>,
    utilization_payments: BTreeMap<(String, String), UtilizationPaymentRecord>,
    stacking_records: BTreeMap<(String, String), P483StackingRecord>,
}

impl LfmRecordStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one flow-design record (the predecessor stage must
    /// already carry a design record — the design discipline).
    pub fn register_design(&mut self, design: LfmFlowDesignRecord) -> Result<(), LfmError> {
        let key = (design.tenant_id().to_string(), design.design_id().to_string());
        if self.designs.contains_key(&key) {
            return Err(LfmError::DuplicateIdentity {
                record_id: design.design_id().to_string(),
            });
        }
        if let Some(predecessor) = design.stage().predecessor() {
            let has_predecessor = self
                .designs
                .values()
                .any(|d| d.tenant_id() == design.tenant_id() && d.stage() == predecessor);
            if !has_predecessor {
                return Err(LfmError::FlowStageOrder {
                    expected_predecessor: predecessor.name(),
                    given_stage: design.stage().name(),
                });
            }
        }
        self.designs.insert(key, design);
        Ok(())
    }

    /// The flow-design records for one tenant (ascending design id —
    /// deterministic).
    pub fn designs_for_tenant(&self, tenant_id: &str) -> Vec<&LfmFlowDesignRecord> {
        self.designs
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .map(|(_, design)| design)
            .collect()
    }

    /// Registers one availability-payment record.
    pub fn register_availability_payment(
        &mut self,
        record: AvailabilityPaymentRecord,
    ) -> Result<(), LfmError> {
        let key = (record.tenant_id().to_string(), record.record_id().to_string());
        if self.availability_payments.contains_key(&key) {
            return Err(LfmError::DuplicateIdentity {
                record_id: record.record_id().to_string(),
            });
        }
        self.availability_payments.insert(key, record);
        Ok(())
    }

    /// Registers one utilization-payment record.
    pub fn register_utilization_payment(
        &mut self,
        record: UtilizationPaymentRecord,
    ) -> Result<(), LfmError> {
        let key = (record.tenant_id().to_string(), record.record_id().to_string());
        if self.utilization_payments.contains_key(&key) {
            return Err(LfmError::DuplicateIdentity {
                record_id: record.record_id().to_string(),
            });
        }
        self.utilization_payments.insert(key, record);
        Ok(())
    }

    /// Registers one P483 stacking record.
    pub fn register_stacking_record(&mut self, record: P483StackingRecord) -> Result<(), LfmError> {
        let key = (record.tenant_id().to_string(), record.record_id().to_string());
        if self.stacking_records.contains_key(&key) {
            return Err(LfmError::DuplicateIdentity {
                record_id: record.record_id().to_string(),
            });
        }
        self.stacking_records.insert(key, record);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture value is synthetic
// and carries no market meaning.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed_decimal::FixedDecimal;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    fn p483_date() -> SourcedDate {
        SourcedDate::new(
            "live 27 November 2025".to_string(),
            "[S-115] Elexon P483 (via industry coverage)".to_string(),
            crate::capacity_market_records::RESEARCH_CUTOFF.to_string(),
            true,
        )
        .expect("sourced date")
    }

    fn design(stage: LfmFlowStage, id: &str) -> LfmFlowDesignRecord {
        LfmFlowDesignRecord::new(
            id.to_string(),
            "tenant-1".to_string(),
            stage,
            BTreeMap::new(),
            BTreeMap::new(),
            "[S-36] Piclo; [S-114] NGED flexibility procurement report 2025-26"
                .to_string(),
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect("design record")
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(FLOW_DESIGNS_NEVER_EXECUTED_STATEMENT.contains("DECLARED status only"));
        assert!(NO_PAYMENT_RECORD_EXISTS_STATEMENT.contains("No availability- or"));
        assert!(P483_STACKING_REFUSES_STATEMENT.contains("never widens a rule"));
    }

    #[test]
    fn the_flow_vocabulary_is_exactly_four_stages_in_order() {
        assert_eq!(LfmFlowStage::Registration.name(), "REGISTRATION");
        assert_eq!(LfmFlowStage::Tender.name(), "TENDER");
        assert_eq!(LfmFlowStage::Utilization.name(), "UTILIZATION");
        assert_eq!(LfmFlowStage::Settlement.name(), "SETTLEMENT");
        assert_eq!(LfmFlowStage::Registration.predecessor(), None);
        assert_eq!(LfmFlowStage::Tender.predecessor(), Some(LfmFlowStage::Registration));
        assert_eq!(LfmFlowStage::Utilization.predecessor(), Some(LfmFlowStage::Tender));
        assert_eq!(LfmFlowStage::Settlement.predecessor(), Some(LfmFlowStage::Utilization));
    }

    #[test]
    fn the_flow_registers_in_stage_order_and_no_stage_executes() {
        let mut store = LfmRecordStore::new();
        // A tender design without a registration design is refused.
        assert_eq!(
            store
                .register_design(design(LfmFlowStage::Tender, "d-tender"))
                .expect_err("stage order")
                .name(),
            "LFM_FLOW_STAGE_ORDER"
        );
        store.register_design(design(LfmFlowStage::Registration, "d-reg")).expect("registered");
        store.register_design(design(LfmFlowStage::Tender, "d-tender")).expect("registered");
        store
            .register_design(design(LfmFlowStage::Utilization, "d-util"))
            .expect("registered");
        store
            .register_design(design(LfmFlowStage::Settlement, "d-settle"))
            .expect("registered");
        assert_eq!(store.designs_for_tenant("tenant-1").len(), 4);
        // Every design is DECLARED data; no executed status exists in
        // the type at all (structural: the record carries no status
        // field beyond its identity).
        for d in store.designs_for_tenant("tenant-1") {
            assert!(!d.design_id().is_empty());
        }
    }

    #[test]
    fn payment_records_carry_exact_money_and_register() {
        let mut store = LfmRecordStore::new();
        let window = ParticipationWindow::new(1_700_000_000_000, 1_700_000_900_000)
            .expect("window");
        let availability = AvailabilityPaymentRecord::new(
            "ap-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "tender-ref-synthetic".to_string(),
            window,
            MoneyAmount::parse("1200.50", "GBP".to_string()).expect("amount"),
            "SUSTAINED FLEXIBILITY AVAILABILITY (declared label)".to_string(),
            link(),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("record");
        assert_eq!(availability.amount().canonical_string(), "1200.50 GBP");
        store.register_availability_payment(availability).expect("registered");
        let utilization = UtilizationPaymentRecord::new(
            "up-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "tender-ref-synthetic".to_string(),
            "utilization-event-synthetic".to_string(),
            window,
            MoneyAmount::parse("85.25", "GBP".to_string()).expect("amount"),
            "UTILIZATION (declared label)".to_string(),
            link(),
            1_700_000_200_000,
            "recorder".to_string(),
        )
        .expect("record");
        assert_eq!(utilization.amount().value().mantissa(), 8525);
        assert_eq!(utilization.amount().value().scale(), 2);
        store.register_utilization_payment(utilization).expect("registered");
        // Duplicate identities refuse.
        let duplicate = AvailabilityPaymentRecord::new(
            "ap-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "tender-ref-synthetic".to_string(),
            window,
            MoneyAmount::parse("1.00", "GBP".to_string()).expect("amount"),
            "basis".to_string(),
            link(),
            1_700_000_300_000,
            "recorder".to_string(),
        )
        .expect("duplicate record");
        assert_eq!(
            store
                .register_availability_payment(duplicate)
                .expect_err("duplicate")
                .name(),
            "LFM_DUPLICATE_IDENTITY"
        );
        // Zero-scale money is exact too (never a float rendering).
        let whole = FixedDecimal::from_parts(42, 0).expect("exact");
        assert_eq!(
            MoneyAmount::new(whole, "GBP".to_string())
                .expect("money")
                .canonical_string(),
            "42 GBP"
        );
    }

    #[test]
    fn a_stacking_record_requires_the_rule_reference_and_carries_the_sourced_date() {
        let err = P483StackingRecord::new(
            "sr-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            String::new(),
            p483_date(),
            "determination".to_string(),
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect_err("rule ref mandatory");
        assert_eq!(err.name(), "LFM_STACKING_RULE_REF_REQUIRED");
        let record = P483StackingRecord::new(
            "sr-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "rule-p483-1".to_string(),
            p483_date(),
            "P483 dual-participation arrangements govern the named combination; the \
             enrollment store refuses a violating PARTICIPATING enrollment."
                .to_string(),
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect("record");
        assert_eq!(record.determination(), StackingDetermination::RuleApplied);
        assert_eq!(record.determination().name(), "RULE_APPLIED");
        assert!(record.p483_date().reverify_at_use());
        assert!(record.p483_date().date_text().contains("27 November 2025"));
        let mut store = LfmRecordStore::new();
        store.register_stacking_record(record).expect("registered");
        // The determination vocabulary is closed at RULE_APPLIED —
        // there is no ALLOWED/OVERRIDE variant to construct (the enum
        // has exactly one variant).
        let all = [StackingDetermination::RuleApplied];
        for determination in all {
            assert!(determination.name() != "ALLOWED");
            assert!(determination.name() != "OVERRIDE");
        }
    }
}
