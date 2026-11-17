// P16.13 — Revenue-share billing basics: contract-configurable splits,
//         pooled-revenue disaggregation per asset, owner-statement
//         records, invoicing records, and reconciliation back to the
//         ISO/DSO statements (the VPP settlement-blueprint pattern
//         [S-118]).
//
// Phase 16 plan §2 row P16.13; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 41_p16_13_revenue_share_billing.md`.
//
// WHAT THIS IS (decided once):
//
// - **Revenue-share statements only — NOT a retail CIS** (plan §4;
//   gap analysis §8 recorded with revisit conditions). This module
//   records contract-configurable splits and disaggregates POOLED
//   PROGRAM revenue per asset for owner transparency. It is not a
//   customer-information system: no tariff engine, no retail rate
//   calculation, no customer account, no utility-bill replacement
//   exists anywhere in this module (ADR-011: jurisdiction-specific
//   records, never a generic tariff engine).
// - **Splits are contract-declared data, never FlexCore's choice**
//   (the P16.02 contract model's rule, mirrored downstream): the
//   revenue-share percentages enter as declared decimal-text recorded
//   ON the configuration, exactly as the contract states them. The
//   one validation this module performs is the closed-world check a
//   split MUST satisfy to be usable: the declared shares sum to
//   EXACTLY 100% — never silently normalized, never padded, never
//   rounded. A configuration that does not sum to 100% is refused by
//   name.
// - **No money arithmetic in floats — ever** (plan §4 non-goals): every
//   amount is the shared fixed-decimal layer
//   ([`crate::fixed_decimal`], i128 mantissa × 10^−scale, currency as
//   data). Disaggregation is EXACT: a share amount is
//   `total × percent` computed in checked integer arithmetic on the
//   decimal mantissas (scale grows additively), and the per-asset
//   shares are verified to sum back to the pooled total EXACTLY —
//   zero remainder at the common scale. There is no rounding step
//   anywhere: an exact product is representable because the scales
//   add, and an overflow is a named refusal, never a wrapped value.
// - **No invoice, statement, or figure is ISSUED** (honesty rules):
//   owner statements exist as DRAFT/RECONCILED records; the closed
//   statement vocabulary contains NO ISSUED, SENT, or DELIVERED state
//   (structural — the states do not exist to transition to). An
//   invoicing record is a RECORDED data row referencing a
//   counterparty-issued invoice reference; nothing is transmitted,
//   filed, or published by this module.
// - **Reconciliation is against the ISO/DSO statements, and every
//   mismatch is a named record** feeding the P16.12 dispute queue by
//   citation: the reason codes are the P16.09 queue vocabulary
//   (`LINE_AMOUNT_MISMATCH` / `LINE_MISSING_IN_STATEMENT` /
//   `LINE_UNEXPECTED_IN_STATEMENT`), never a generic "other". The
//   P16.09 reconciliation engine's pattern is mirrored, not
//   redefined; the P16.01 settlement rows (Agent A's store) are the
//   persisted-statement home, consumed by citation.
// - **Every financial record carries its P11 audit-chain LINK** (the
//   `AuditChainLink` type is the domain's, reused — one home; the
//   chain treatment itself stays in the P11 chain, consumed, never
//   redefined).
//
// Determinism: stores are BTreeMap-backed, register-only with the
// supersede model (a correction is a NEW record), no wall-clock input.
//
// Control status: records for the simulation-only 0.x line. No billing
// run has executed, no statement has been rendered, no figure exists.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use flexcore_domain::market_program_model::{AuditChainLink, ParticipationWindow};
use flexcore_domain::market_program_model::MarketProgramModelError;

use crate::fixed_decimal::{FixedDecimal, FixedDecimalError, MoneyAmount};

/// The revenue-share-billing schema version (a change is a NEW version,
/// never a silent edit).
pub const REVENUE_SHARE_BILLING_SCHEMA_VERSION: u32 = 1;

/// The not-a-retail-CIS statement (pinned by tests).
pub const REVENUE_SHARE_NOT_RETAIL_CIS_STATEMENT: &str = "Revenue-share billing here is \
     contract-configurable splits and pooled-revenue disaggregation for owner \
     transparency — NOT a retail billing system or CIS replacement: no tariff engine, no \
     retail rate calculation, no customer account, and no utility-bill replacement exists \
     in this module (phase-16 plan section 4; ADR-011).";

/// The no-issuance statement (pinned by tests).
pub const NO_STATEMENT_ISSUED_STATEMENT: &str = "No invoice, statement, or revenue figure \
     is issued anywhere: owner statements exist as DRAFT/RECONCILED records and the \
     closed statement vocabulary contains no ISSUED, SENT, or DELIVERED state; invoicing \
     records only RECORD a counterparty invoice reference. Nothing is transmitted, \
     filed, or published by this module.";

/// The exact-disaggregation statement (pinned by tests).
pub const EXACT_DISAGGREGATION_STATEMENT: &str = "Disaggregation is exact: a share amount \
     is total x percent computed in checked integer arithmetic on the decimal mantissas \
     (scales add, no division, no rounding), and the per-asset shares are verified to \
     sum back to the pooled total exactly (zero remainder at the common scale). An \
     overflow is a named refusal, never a wrapped or rounded value.";

/// The reconciliation mismatch statement (pinned by tests).
pub const EVERY_MISMATCH_NAMED_STATEMENT: &str = "Every reconciliation mismatch is a \
     named record carrying BOTH values verbatim and a P16.09 queue reason code — never \
     a silent adjustment, never a generic 'other'. Discrepancy records feed the P16.12 \
     dispute queue by citation; the module never settles or corrects in place.";

/// The P16.09 queue reason codes (consumed by citation — the spelling is
/// the P16.09 record's, one vocabulary across the phase).
pub const REASON_CODE_AMOUNT_MISMATCH: &str = "LINE_AMOUNT_MISMATCH";
pub const REASON_CODE_MISSING_IN_STATEMENT: &str = "LINE_MISSING_IN_STATEMENT";
pub const REASON_CODE_UNEXPECTED_IN_STATEMENT: &str = "LINE_UNEXPECTED_IN_STATEMENT";

// --- Errors -------------------------------------------------------------------

/// Why a revenue-share-billing operation was refused. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingError {
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
    /// A declared share was not a parseable decimal (the underlying
    /// decimal refusal is carried).
    UnparseableShare {
        asset_id: String,
        reason: String,
    },
    /// A declared share was outside [0, 100] (negative or > 100).
    ShareOutOfBounds {
        asset_id: String,
    },
    /// The declared shares did not sum to EXACTLY 100% at the common
    /// scale (a remainder — however small — is a refusal, never a
    /// normalization).
    SharesDoNotSumTo100 {
        remainder: String,
    },
    /// A share configuration carried no shares at all.
    EmptyShares,
    /// A statement line carried a currency different from another line
    /// of the same record (money of two currencies is never silently
    /// mixed — the fixed-decimal layer's rule, inherited).
    MixedCurrencies {
        line_ref: String,
    },
    /// The pooled total's scale combined with a share's scale would
    /// exceed the exact-decimal maximum — the computation is refused,
    /// never rounded.
    ShareScaleOverflow,
    /// The share multiplication overflowed the i128 mantissa.
    ShareMantissaOverflow,
    /// The per-asset shares did not sum back to the pooled total
    /// exactly (an internal-consistency refusal; this cannot occur for
    /// a 100%-summing configuration, and the check is performed
    /// anyway — fail-closed).
    DisaggregationDoesNotSum {
        remainder: String,
    },
    /// A decimal operation failed (carried from the fixed-decimal
    /// layer).
    Decimal(FixedDecimalError),
    /// A domain shared-type constructor refused (carried).
    Domain(MarketProgramModelError),
    /// The statement transition is not in the closed table (no
    /// RECONCILED without an all-matched outcome; no DRAFT from
    /// RECONCILED).
    IllegalStatementTransition {
        from: &'static str,
        to: &'static str,
    },
    /// A reconciliation outcome that is not all-matched was used to
    /// reconcile a statement (mismatches must be resolved through the
    /// dispute queue first — never absorbed).
    UnresolvedMismatches {
        count: usize,
    },
    /// A duplicate identity was registered.
    DuplicateIdentity {
        record_id: String,
    },
    /// The record this transition would apply to was not found.
    RecordNotFound {
        record_id: String,
    },
    /// The record is already superseded.
    RecordSuperseded {
        record_id: String,
    },
}

impl BillingError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            BillingError::EmptyField { .. } => "BILLING_EMPTY_FIELD",
            BillingError::FieldTooLong { .. } => "BILLING_FIELD_TOO_LONG",
            BillingError::UnparseableShare { .. } => "BILLING_UNPARSEABLE_SHARE",
            BillingError::ShareOutOfBounds { .. } => "BILLING_SHARE_OUT_OF_BOUNDS",
            BillingError::SharesDoNotSumTo100 { .. } => "BILLING_SHARES_DO_NOT_SUM_TO_100",
            BillingError::EmptyShares => "BILLING_EMPTY_SHARES",
            BillingError::MixedCurrencies { .. } => "BILLING_MIXED_CURRENCIES",
            BillingError::ShareScaleOverflow => "BILLING_SHARE_SCALE_OVERFLOW",
            BillingError::ShareMantissaOverflow => "BILLING_SHARE_MANTISSA_OVERFLOW",
            BillingError::DisaggregationDoesNotSum => "BILLING_DISAGGREGATION_DOES_NOT_SUM",
            BillingError::Decimal(_) => "BILLING_DECIMAL_ERROR",
            BillingError::Domain(_) => "BILLING_DOMAIN_ERROR",
            BillingError::IllegalStatementTransition { .. } => "BILLING_ILLEGAL_STATEMENT_TRANSITION",
            BillingError::UnresolvedMismatches { .. } => "BILLING_UNRESOLVED_MISMATCHES",
            BillingError::DuplicateIdentity { .. } => "BILLING_DUPLICATE_IDENTITY",
            BillingError::RecordNotFound { .. } => "BILLING_RECORD_NOT_FOUND",
            BillingError::RecordSuperseded { .. } => "BILLING_RECORD_SUPERSEDED",
        }
    }
}

impl std::fmt::Display for BillingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BillingError::EmptyField { field } => {
                write!(f, "the billing field '{field}' must be non-empty")
            }
            BillingError::FieldTooLong { field, len, max } => write!(
                f,
                "the billing field '{field}' is {len} characters; the bound is {max}"
            ),
            BillingError::UnparseableShare { asset_id, reason } => write!(
                f,
                "the declared share for asset {asset_id} is not a parseable decimal: \
                 {reason} — shares enter as declared decimal text, never guessed"
            ),
            BillingError::ShareOutOfBounds { asset_id } => write!(
                f,
                "the declared share for asset {asset_id} is outside [0, 100]; the \
                 configuration is refused, never clamped"
            ),
            BillingError::SharesDoNotSumTo100 { remainder } => write!(
                f,
                "the declared shares sum to 100 {remainder}; they must sum to EXACTLY \
                 100% — the configuration is refused, never silently normalized"
            ),
            BillingError::EmptyShares => {
                write!(f, "a revenue-share configuration carries no shares")
            }
            BillingError::MixedCurrencies { line_ref } => write!(
                f,
                "the statement line {line_ref} carries a currency different from the \
                 record's other lines; money of two currencies is never silently mixed"
            ),
            BillingError::ShareScaleOverflow => write!(
                f,
                "the pooled-total scale combined with the share scale exceeds the exact \
                 decimal maximum; the computation is refused, never rounded"
            ),
            BillingError::ShareMantissaOverflow => write!(
                f,
                "the share multiplication overflows the 128-bit mantissa; the \
                 computation is refused, never wrapped"
            ),
            BillingError::DisaggregationDoesNotSum { remainder } => write!(
                f,
                "the per-asset shares do not sum back to the pooled total exactly \
                 (remainder {remainder}); the disaggregation is refused, never padded"
            ),
            BillingError::Decimal(err) => write!(f, "decimal operation refused: {err}"),
            BillingError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
            BillingError::IllegalStatementTransition { from, to } => write!(
                f,
                "the statement transition {from} → {to} is not in the closed table; a \
                 statement is RECONCILED only with an all-matched outcome"
            ),
            BillingError::UnresolvedMismatches { count } => write!(
                f,
                "the reconciliation outcome carries {count} unresolved mismatches; a \
                 statement is reconciled only when every line matches — mismatches \
                 resolve through the dispute queue, never absorbed"
            ),
            BillingError::DuplicateIdentity { record_id } => write!(
                f,
                "the billing-record identity {record_id} is already registered; a \
                 correction is a NEW superseding record"
            ),
            BillingError::RecordNotFound { record_id } => {
                write!(f, "the billing record {record_id} was not found")
            }
            BillingError::RecordSuperseded { record_id } => write!(
                f,
                "the billing record {record_id} is already superseded; there is exactly \
                 one current version"
            ),
        }
    }
}

impl std::error::Error for BillingError {}

impl From<FixedDecimalError> for BillingError {
    fn from(err: FixedDecimalError) -> Self {
        BillingError::Decimal(err)
    }
}

impl From<MarketProgramModelError> for BillingError {
    fn from(err: MarketProgramModelError) -> Self {
        BillingError::Domain(err)
    }
}

// --- Shared helpers -----------------------------------------------------------

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), BillingError> {
    if value.trim().is_empty() {
        return Err(BillingError::EmptyField { field });
    }
    if value.len() > max {
        return Err(BillingError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

/// The exact share computation `total × percent` (percent in [0, 100],
/// expressed as the exact decimal it was declared as). Computed in
/// checked integer arithmetic on the mantissas: the mantissas multiply
/// (overflow-checked) and the scales ADD (+2 for the percent denominator
/// 100) — exact, no division, no rounding. A scale overflow beyond
/// [`crate::fixed_decimal::MAX_SCALE`] or a mantissa overflow is a named
/// refusal.
pub fn percent_of_amount(
    total: &MoneyAmount,
    percent: &FixedDecimal,
) -> Result<MoneyAmount, BillingError> {
    let zero = FixedDecimal::from_parts(0, 0).expect("zero is a valid decimal");
    let hundred = FixedDecimal::from_parts(100, 0).expect("100 is a valid decimal");
    if percent.partial_cmp(&zero) == Some(Ordering::Less) {
        return Err(BillingError::ShareOutOfBounds {
            asset_id: String::new(),
        });
    }
    if percent.partial_cmp(&hundred) == Some(Ordering::Greater) {
        return Err(BillingError::ShareOutOfBounds {
            asset_id: String::new(),
        });
    }
    let new_scale = total
        .value()
        .scale()
        .checked_add(percent.scale())
        .and_then(|s| s.checked_add(2))
        .ok_or(BillingError::ShareScaleOverflow)?;
    if new_scale > crate::fixed_decimal::MAX_SCALE {
        return Err(BillingError::ShareScaleOverflow);
    }
    let mantissa = total
        .value()
        .mantissa()
        .checked_mul(percent.mantissa())
        .ok_or(BillingError::ShareMantissaOverflow)?;
    let value = FixedDecimal::from_parts(mantissa, new_scale)?;
    Ok(MoneyAmount::new(value, total.currency_code().to_string())?)
}

/// Verifies that `shares` sum EXACTLY to `expected` by chained
/// overflow-checked subtraction (the fixed-decimal layer has no
/// accumulation add; a zero remainder after subtracting every share is
/// the exact-sum proof). Returns the refusal remainder rendering on
/// mismatch.
fn sums_exactly(shares: impl Iterator<Item = MoneyAmount>, expected: &MoneyAmount)
    -> Result<(), String>
{
    let mut remainder = expected.clone();
    for share in shares {
        remainder = remainder.checked_sub(share).map_err(|err| err.to_string())?;
    }
    if remainder.value().is_zero() {
        Ok(())
    } else {
        Err(remainder.canonical_string())
    }
}

// --- The revenue-share configuration -------------------------------------------

/// One contract-configurable revenue-share configuration: the shares are
/// DECLARED decimal-text percentages per asset, recorded exactly as the
/// contract states them. The single structural check is that the shares
/// sum to EXACTLY 100% (closed-world usability); nothing is normalized,
/// clamped, or defaulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevenueShareConfiguration {
    configuration_id: String,
    tenant_id: String,
    /// The contract this configuration interprets (the P16.02
    /// `MarketContract` identity; the contract's terms carry the split
    /// data upstream — this configuration records the parsed shares
    /// verbatim).
    contract_id: String,
    /// The declared basis label (WHICH revenue the split applies to —
    /// recorded data from the contract, e.g. a program's
    /// availability-payment pool; never interpreted here).
    basis_label: String,
    /// The declared shares per asset (BTreeMap — deterministic order;
    /// decimal-text percentages exactly as declared).
    shares: BTreeMap<String, FixedDecimal>,
    supersedes_configuration_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl RevenueShareConfiguration {
    /// Builds and validates one configuration: every share is parsed
    /// from declared decimal text, bounded to [0, 100], and the whole
    /// set sums to EXACTLY 100%.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        configuration_id: String,
        tenant_id: String,
        contract_id: String,
        basis_label: String,
        shares_text: BTreeMap<String, String>,
        supersedes_configuration_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, BillingError> {
        validate_text("configuration_id", &configuration_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("contract_id", &contract_id, 64)?;
        validate_text("basis_label", &basis_label, 128)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(BillingError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        if shares_text.is_empty() {
            return Err(BillingError::EmptyShares);
        }
        let mut shares = BTreeMap::new();
        for (asset_id, text) in &shares_text {
            validate_text("share asset_id", asset_id, 64)?;
            let percent = FixedDecimal::parse(text).map_err(|err| {
                BillingError::UnparseableShare {
                    asset_id: asset_id.clone(),
                    reason: err.to_string(),
                }
            })?;
            let zero = FixedDecimal::from_parts(0, 0).expect("zero is a valid decimal");
            let hundred = FixedDecimal::from_parts(100, 0).expect("100 is a valid decimal");
            if percent.partial_cmp(&zero) == Some(Ordering::Less)
                || percent.partial_cmp(&hundred) == Some(Ordering::Greater)
            {
                return Err(BillingError::ShareOutOfBounds {
                    asset_id: asset_id.clone(),
                });
            }
            shares.insert(asset_id.clone(), percent);
        }
        // The closed-world check: the shares sum to EXACTLY 100% —
        // never normalized.
        let hundred = MoneyAmount::new(
            FixedDecimal::from_parts(100, 0).expect("100 is a valid decimal"),
            "NOSPEC".to_string(),
        )
        .expect("synthetic currency for the percent-only check");
        sums_exactly(
            shares.values().map(|p| {
                MoneyAmount::new(*p, "NOSPEC".to_string()).expect("bounded share")
            }),
            &hundred,
        )
        .map_err(|remainder| {
            // The remainder is rendered as a percent number, not money.
            BillingError::SharesDoNotSumTo100 {
                remainder: remainder.split(' ').next().unwrap_or("").to_string(),
            }
        })?;
        Ok(Self {
            configuration_id,
            tenant_id,
            contract_id,
            basis_label,
            shares,
            supersedes_configuration_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn contract_id(&self) -> &str {
        &self.contract_id
    }

    /// The declared basis label (recorded data — which revenue the
    /// split applies to).
    pub fn basis_label(&self) -> &str {
        &self.basis_label
    }

    /// The declared shares per asset (deterministic order; the exact
    /// declared percentages).
    pub fn shares(&self) -> &BTreeMap<String, FixedDecimal> {
        &self.shares
    }

    pub fn supersedes_configuration_id(&self) -> Option<&str> {
        self.supersedes_configuration_id.as_deref()
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

// --- Disaggregation ------------------------------------------------------------

/// One pooled-revenue disaggregation record: the pooled amount from ONE
/// ISO/DSO settlement statement row (the P16.01 `MarketSettlementRow`
/// shape, consumed by citation — decimal-text amount + currency code as
/// data) disaggregated per asset against a recorded configuration. The
/// per-asset amounts are computed EXACTLY and verified to sum back to
/// the pooled total exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PooledRevenueDisaggregation {
    disaggregation_id: String,
    tenant_id: String,
    program_id: String,
    /// The ISO/DSO settlement statement row this pool came from (the
    /// P16.01 settlement-row identity — consumed by citation).
    settlement_statement_ref: String,
    configuration_id: String,
    /// The participation window the pool covers (end-exclusive — the
    /// §10.3 convention; the domain window type reused).
    window: ParticipationWindow,
    /// The pooled revenue total (exact decimal + currency as data).
    pooled_total: MoneyAmount,
    /// The per-asset disaggregated amounts (BTreeMap — deterministic).
    per_asset: BTreeMap<String, MoneyAmount>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl PooledRevenueDisaggregation {
    /// Computes one disaggregation from a recorded configuration and a
    /// pooled total: per-asset exact share amounts, verified to sum
    /// back to the total exactly (zero remainder).
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        disaggregation_id: String,
        tenant_id: String,
        program_id: String,
        settlement_statement_ref: String,
        configuration: &RevenueShareConfiguration,
        window: ParticipationWindow,
        pooled_total: MoneyAmount,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, BillingError> {
        validate_text("disaggregation_id", &disaggregation_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("program_id", &program_id, 64)?;
        validate_text("settlement_statement_ref", &settlement_statement_ref, 256)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(BillingError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        let mut per_asset = BTreeMap::new();
        for (asset_id, percent) in configuration.shares() {
            let share = percent_of_amount(&pooled_total, percent)?;
            per_asset.insert(asset_id.clone(), share);
        }
        sums_exactly(per_asset.values().cloned(), &pooled_total)
            .map_err(|remainder| BillingError::DisaggregationDoesNotSum { remainder })?;
        Ok(Self {
            disaggregation_id,
            tenant_id,
            program_id,
            settlement_statement_ref,
            configuration_id: configuration.configuration_id().to_string(),
            window,
            pooled_total,
            per_asset,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn disaggregation_id(&self) -> &str {
        &self.disaggregation_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn settlement_statement_ref(&self) -> &str {
        &self.settlement_statement_ref
    }

    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }

    pub fn window(&self) -> ParticipationWindow {
        self.window
    }

    pub fn pooled_total(&self) -> &MoneyAmount {
        &self.pooled_total
    }

    /// The per-asset disaggregated amounts (deterministic order; exact
    /// decimals — never rounded).
    pub fn per_asset(&self) -> &BTreeMap<String, MoneyAmount> {
        &self.per_asset
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

// --- Owner statements ----------------------------------------------------------

/// The closed owner-statement vocabulary. Deliberately NO ISSUED, SENT,
/// or DELIVERED state exists: no statement is issued anywhere in this
/// program (honesty rules; the structural proof is that the states do
/// not exist to transition to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerStatementState {
    /// The statement was recorded from a disaggregation (the initial
    /// and, before reconciliation, only state).
    Draft,
    /// Every line matched the ISO/DSO statement exactly (recorded via
    /// an all-matched reconciliation outcome).
    Reconciled,
}

impl OwnerStatementState {
    /// The canonical label.
    pub const fn name(self) -> &'static str {
        match self {
            OwnerStatementState::Draft => "DRAFT",
            OwnerStatementState::Reconciled => "RECONCILED",
        }
    }

    /// The closed transition table: DRAFT → RECONCILED (with an
    /// all-matched outcome only). RECONCILED is terminal.
    pub fn can_transition_to(self, to: OwnerStatementState) -> bool {
        matches!((self, to), (OwnerStatementState::Draft, OwnerStatementState::Reconciled))
    }
}

/// One owner-statement record: the per-owner revenue lines for a period
/// (from the disaggregation), recorded as data — never rendered,
/// printed, sent, or published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerStatementRecord {
    statement_id: String,
    tenant_id: String,
    /// The owner the statement is FOR (recorded reference — the owner
    /// registry is the tenant's own data, never invented here).
    owner_ref: String,
    program_id: String,
    statement_period: ParticipationWindow,
    /// The statement lines (declared line ref → exact amount; ONE
    /// currency across the record — mixed-currency statements refuse).
    lines: BTreeMap<String, MoneyAmount>,
    /// The disaggregation the lines came from (mandatory linkage — a
    /// statement is never self-invented).
    disaggregation_id: String,
    state: OwnerStatementState,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl OwnerStatementRecord {
    /// Builds one DRAFT statement from a disaggregation's per-asset
    /// lines (uniform currency enforced; the disaggregation linkage is
    /// mandatory).
    #[allow(clippy::too_many_arguments)]
    pub fn from_disaggregation(
        statement_id: String,
        tenant_id: String,
        owner_ref: String,
        program_id: String,
        statement_period: ParticipationWindow,
        disaggregation: &PooledRevenueDisaggregation,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, BillingError> {
        validate_text("statement_id", &statement_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("owner_ref", &owner_ref, 128)?;
        validate_text("program_id", &program_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(BillingError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        if disaggregation.per_asset().is_empty() {
            return Err(BillingError::EmptyShares);
        }
        // Uniform currency across the record.
        let mut currency: Option<String> = None;
        for (line_ref, amount) in disaggregation.per_asset() {
            match &currency {
                None => currency = Some(amount.currency_code().to_string()),
                Some(c) if c == amount.currency_code() => {}
                Some(_) => {
                    return Err(BillingError::MixedCurrencies {
                        line_ref: line_ref.clone(),
                    })
                }
            }
        }
        Ok(Self {
            statement_id,
            tenant_id,
            owner_ref,
            program_id,
            statement_period,
            lines: disaggregation.per_asset().clone(),
            disaggregation_id: disaggregation.disaggregation_id().to_string(),
            state: OwnerStatementState::Draft,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn statement_id(&self) -> &str {
        &self.statement_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn owner_ref(&self) -> &str {
        &self.owner_ref
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn statement_period(&self) -> ParticipationWindow {
        self.statement_period
    }

    /// The statement lines (deterministic order; exact decimals).
    pub fn lines(&self) -> &BTreeMap<String, MoneyAmount> {
        &self.lines
    }

    /// The disaggregation linkage (mandatory — a statement is never
    /// self-invented).
    pub fn disaggregation_id(&self) -> &str {
        &self.disaggregation_id
    }

    pub fn state(&self) -> OwnerStatementState {
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

    /// Transitions the statement to RECONCILED against an all-matched
    /// reconciliation outcome (consumes the outcome below). Any
    /// mismatch refuses — mismatches resolve through the dispute
    /// queue, never absorbed.
    pub fn reconcile(
        &mut self,
        outcome: &StatementReconciliationOutcome,
    ) -> Result<(), BillingError> {
        if !self.state.can_transition_to(OwnerStatementState::Reconciled) {
            return Err(BillingError::IllegalStatementTransition {
                from: self.state.name(),
                to: OwnerStatementState::Reconciled.name(),
            });
        }
        if outcome.mismatch_count() > 0 {
            return Err(BillingError::UnresolvedMismatches {
                count: outcome.mismatch_count(),
            });
        }
        self.state = OwnerStatementState::Reconciled;
        Ok(())
    }
}

// --- Reconciliation back to the ISO/DSO statements ------------------------------

/// One per-line reconciliation outcome (the P16.09 engine's outcome
/// shape mirrored over owner-statement lines; NOT a redefinition — the
/// P16.09 engine remains the statement-ingestion reconciliation home).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerStatementLineOutcome {
    /// The line matched the ISO/DSO statement line exactly (value
    /// equality at the common scale — the fixed-decimal rule).
    Matched,
    /// Both values carried, verbatim, plus the exact deviation
    /// (statement − ISO/DSO). Never corrected in place.
    AmountMismatch {
        statement_value: MoneyAmount,
        iso_dso_value: MoneyAmount,
        deviation: MoneyAmount,
    },
    /// The owner-statement line is missing from the ISO/DSO statement.
    MissingInStatement,
    /// The ISO/DSO statement carries a line the owner statement does
    /// not.
    UnexpectedInStatement,
}

/// One reconciliation run's outcome record: per-line outcomes over the
/// exact string line identity (the P8.11 discipline; near-miss
/// references FLAG, never match).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementReconciliationOutcome {
    statement_id: String,
    iso_dso_statement_ref: String,
    /// Per-line outcomes, ascending line-ref order (deterministic).
    lines: Vec<(String, OwnerStatementLineOutcome)>,
}

impl StatementReconciliationOutcome {
    /// Reconciles an owner statement's lines against the ISO/DSO
    /// statement's lines: exact string identity, exact value equality,
    /// cross-currency run refusal. Every mismatch carries BOTH values
    /// verbatim.
    pub fn run(
        statement: &OwnerStatementRecord,
        iso_dso_statement_ref: String,
        iso_dso_lines: &BTreeMap<String, MoneyAmount>,
    ) -> Result<Self, BillingError> {
        validate_text("iso_dso_statement_ref", &iso_dso_statement_ref, 256)?;
        // Cross-currency run refusal: every line on BOTH sides carries
        // one currency.
        let mut currency: Option<String> = None;
        for (line_ref, amount) in statement.lines() {
            match &currency {
                None => currency = Some(amount.currency_code().to_string()),
                Some(c) if c == amount.currency_code() => {}
                Some(_) => {
                    return Err(BillingError::MixedCurrencies {
                        line_ref: line_ref.clone(),
                    })
                }
            }
        }
        for (line_ref, amount) in iso_dso_lines {
            match &currency {
                None => currency = Some(amount.currency_code().to_string()),
                Some(c) if c == amount.currency_code() => {}
                Some(_) => {
                    return Err(BillingError::MixedCurrencies {
                        line_ref: line_ref.clone(),
                    })
                }
            }
        }
        let mut lines = Vec::new();
        for (line_ref, statement_value) in statement.lines() {
            match iso_dso_lines.get(line_ref) {
                Some(iso_dso_value) => {
                    if statement_value == iso_dso_value {
                        lines.push((line_ref.clone(), OwnerStatementLineOutcome::Matched));
                    } else {
                        let deviation = statement_value
                            .clone()
                            .checked_sub(iso_dso_value.clone())
                            .map_err(BillingError::Decimal)?;
                        lines.push((
                            line_ref.clone(),
                            OwnerStatementLineOutcome::AmountMismatch {
                                statement_value: statement_value.clone(),
                                iso_dso_value: iso_dso_value.clone(),
                                deviation,
                            },
                        ));
                    }
                }
                None => {
                    lines.push((line_ref.clone(), OwnerStatementLineOutcome::MissingInStatement));
                }
            }
        }
        for line_ref in iso_dso_lines.keys() {
            if !statement.lines().contains_key(line_ref) {
                lines.push((
                    line_ref.clone(),
                    OwnerStatementLineOutcome::UnexpectedInStatement,
                ));
            }
        }
        Ok(Self {
            statement_id: statement.statement_id().to_string(),
            iso_dso_statement_ref,
            lines,
        })
    }

    pub fn statement_id(&self) -> &str {
        &self.statement_id
    }

    pub fn iso_dso_statement_ref(&self) -> &str {
        &self.iso_dso_statement_ref
    }

    /// The per-line outcomes (ascending line-ref order).
    pub fn lines(&self) -> &[(String, OwnerStatementLineOutcome)] {
        &self.lines
    }

    /// The number of lines that did NOT match (mismatches, missing,
    /// unexpected).
    pub fn mismatch_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|(_, outcome)| !matches!(outcome, OwnerStatementLineOutcome::Matched))
            .count()
    }

    /// The dispute-queue entries the outcome feeds (the P16.09 reason
    /// codes — consumed by citation; the queue itself is P16.12's, and
    /// the challenge kind derivation lives there).
    pub fn discrepancy_reason_codes(&self) -> Vec<(String, &'static str)> {
        self.lines
            .iter()
            .filter_map(|(line_ref, outcome)| match outcome {
                OwnerStatementLineOutcome::Matched => None,
                OwnerStatementLineOutcome::AmountMismatch { .. } => {
                    Some((line_ref.clone(), REASON_CODE_AMOUNT_MISMATCH))
                }
                OwnerStatementLineOutcome::MissingInStatement => {
                    Some((line_ref.clone(), REASON_CODE_MISSING_IN_STATEMENT))
                }
                OwnerStatementLineOutcome::UnexpectedInStatement => {
                    Some((line_ref.clone(), REASON_CODE_UNEXPECTED_IN_STATEMENT))
                }
            })
            .collect()
    }
}

// --- Invoicing records -----------------------------------------------------------

/// The closed invoicing-record vocabulary. An invoicing record is a
/// RECORDED data row only: it references a counterparty-issued invoice
/// number. No CREATED, SENT, PAID, or VOID state exists — issuance and
/// transmission are the counterparty's process, never this module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoicingRecordState {
    /// The invoice reference was recorded as data.
    Recorded,
}

impl InvoicingRecordState {
    pub const fn name(self) -> &'static str {
        match self {
            InvoicingRecordState::Recorded => "RECORDED",
        }
    }
}

/// One invoicing record (data only): the recorded reference of an
/// invoice the COUNTERPARTY issued, tied to an owner statement. No
/// invoice is generated, rendered, sent, or filed by this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicingRecord {
    invoicing_record_id: String,
    tenant_id: String,
    /// The counterparty-issued invoice reference (recorded verbatim —
    /// never generated here).
    counterparty_invoice_ref: String,
    /// The owner statement the invoice settles (mandatory linkage).
    statement_id: String,
    /// The invoiced amount as recorded (exact decimal + currency).
    amount: MoneyAmount,
    state: InvoicingRecordState,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl InvoicingRecord {
    /// Records one invoicing row (validates shapes; uniform currency
    /// with the statement's lines is the caller's data discipline — the
    /// amount carries its own currency code as data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        invoicing_record_id: String,
        tenant_id: String,
        counterparty_invoice_ref: String,
        statement_id: String,
        amount: MoneyAmount,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, BillingError> {
        validate_text("invoicing_record_id", &invoicing_record_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("counterparty_invoice_ref", &counterparty_invoice_ref, 256)?;
        validate_text("statement_id", &statement_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(BillingError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            invoicing_record_id,
            tenant_id,
            counterparty_invoice_ref,
            statement_id,
            amount,
            state: InvoicingRecordState::Recorded,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn invoicing_record_id(&self) -> &str {
        &self.invoicing_record_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn counterparty_invoice_ref(&self) -> &str {
        &self.counterparty_invoice_ref
    }

    pub fn statement_id(&self) -> &str {
        &self.statement_id
    }

    pub fn amount(&self) -> &MoneyAmount {
        &self.amount
    }

    pub fn state(&self) -> InvoicingRecordState {
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

// --- The billing record store -----------------------------------------------------

/// The BTreeMap-backed billing-record store: register-only plus the
/// supersede model (a correction is a NEW record superseding the old;
/// every version retained; one current version per identity). No
/// update, no delete, no wall-clock input.
#[derive(Debug, Default)]
pub struct BillingRecordStore {
    configurations: BTreeMap<(String, String), RevenueShareConfiguration>,
    superseded_configurations: BTreeMap<(String, String), ()>,
    disaggregations: BTreeMap<(String, String), PooledRevenueDisaggregation>,
    statements: BTreeMap<(String, String), OwnerStatementRecord>,
    invoicing_records: BTreeMap<(String, String), InvoicingRecord>,
}

impl BillingRecordStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one revenue-share configuration (duplicate CURRENT
    /// identity refused; a change is a superseding configuration).
    pub fn register_configuration(
        &mut self,
        configuration: RevenueShareConfiguration,
    ) -> Result<(), BillingError> {
        let key = (
            configuration.tenant_id().to_string(),
            configuration.configuration_id().to_string(),
        );
        if self.superseded_configurations.contains_key(&key) {
            return Err(BillingError::RecordSuperseded {
                record_id: configuration.configuration_id().to_string(),
            });
        }
        if self.configurations.contains_key(&key) {
            return Err(BillingError::DuplicateIdentity {
                record_id: configuration.configuration_id().to_string(),
            });
        }
        if let Some(superseded) = configuration.supersedes_configuration_id() {
            let target_key = (configuration.tenant_id().to_string(), superseded.to_string());
            if !self.configurations.contains_key(&target_key) {
                return Err(BillingError::RecordNotFound {
                    record_id: superseded.to_string(),
                });
            }
            self.superseded_configurations.insert(target_key, ());
        }
        self.configurations.insert(key, configuration);
        Ok(())
    }

    /// The CURRENT configurations for one tenant, ascending
    /// configuration_id order (deterministic).
    pub fn current_configurations_for_tenant(
        &self,
        tenant_id: &str,
    ) -> Vec<&RevenueShareConfiguration> {
        self.configurations
            .range((tenant_id.to_string(), String::new())..)
            .take_while(|((tenant, _), _)| tenant == tenant_id)
            .filter(|(key, _)| !self.superseded_configurations.contains_key(key))
            .map(|(_, configuration)| configuration)
            .collect()
    }

    /// Registers one disaggregation record.
    pub fn register_disaggregation(
        &mut self,
        disaggregation: PooledRevenueDisaggregation,
    ) -> Result<(), BillingError> {
        let key = (
            disaggregation.tenant_id().to_string(),
            disaggregation.disaggregation_id().to_string(),
        );
        if self.disaggregations.contains_key(&key) {
            return Err(BillingError::DuplicateIdentity {
                record_id: disaggregation.disaggregation_id().to_string(),
            });
        }
        self.disaggregations.insert(key, disaggregation);
        Ok(())
    }

    /// Registers one owner statement (DRAFT).
    pub fn register_statement(
        &mut self,
        statement: OwnerStatementRecord,
    ) -> Result<(), BillingError> {
        let key = (
            statement.tenant_id().to_string(),
            statement.statement_id().to_string(),
        );
        if self.statements.contains_key(&key) {
            return Err(BillingError::DuplicateIdentity {
                record_id: statement.statement_id().to_string(),
            });
        }
        self.statements.insert(key, statement);
        Ok(())
    }

    /// The current owner statement for one tenant (identity lookup).
    pub fn statement(
        &self,
        tenant_id: &str,
        statement_id: &str,
    ) -> Option<&OwnerStatementRecord> {
        self.statements
            .get(&(tenant_id.to_string(), statement_id.to_string()))
    }

    /// The mutable statement accessor for the reconciliation
    /// transition (identity lookup; the record itself is updated
    /// in place ONLY through the closed transition — the supersede
    /// model governs content corrections, the state column carries the
    /// recorded reconciliation outcome).
    pub fn statement_mut(
        &mut self,
        tenant_id: &str,
        statement_id: &str,
    ) -> Option<&mut OwnerStatementRecord> {
        self.statements
            .get_mut(&(tenant_id.to_string(), statement_id.to_string()))
    }

    /// Registers one invoicing record.
    pub fn register_invoicing_record(
        &mut self,
        record: InvoicingRecord,
    ) -> Result<(), BillingError> {
        let key = (
            record.tenant_id().to_string(),
            record.invoicing_record_id().to_string(),
        );
        if self.invoicing_records.contains_key(&key) {
            return Err(BillingError::DuplicateIdentity {
                record_id: record.invoicing_record_id().to_string(),
            });
        }
        self.invoicing_records.insert(key, record);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the exact-decimal rules above. Nothing is
// fabricated; every fixture value is synthetic and carries no market
// meaning.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    fn window() -> ParticipationWindow {
        ParticipationWindow::new(1_700_000_000_000, 1_700_000_900_000).expect("valid window")
    }

    fn config(shares: BTreeMap<String, String>) -> RevenueShareConfiguration {
        RevenueShareConfiguration::new(
            "cfg-1".to_string(),
            "tenant-1".to_string(),
            "contract-1".to_string(),
            "availability-pool".to_string(),
            shares,
            None,
            link(),
            1_700_000_000_000,
            "recorder".to_string(),
        )
        .expect("valid configuration")
    }

    fn statement_and_iso()
        -> (OwnerStatementRecord, BTreeMap<String, MoneyAmount>)
    {
        let mut shares = BTreeMap::new();
        shares.insert("asset-a".to_string(), "60".to_string());
        shares.insert("asset-b".to_string(), "40".to_string());
        let configuration = config(shares);
        let total = MoneyAmount::parse("1000.00", "GBP".to_string()).expect("total");
        let disaggregation = PooledRevenueDisaggregation::compute(
            "dis-1".to_string(),
            "tenant-1".to_string(),
            "program-1".to_string(),
            "stmt-ref-1".to_string(),
            &configuration,
            window(),
            total,
            link(),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("disaggregation");
        let statement = OwnerStatementRecord::from_disaggregation(
            "stmt-1".to_string(),
            "tenant-1".to_string(),
            "owner-9".to_string(),
            "program-1".to_string(),
            window(),
            &disaggregation,
            link(),
            1_700_000_200_000,
            "recorder".to_string(),
        )
        .expect("statement");
        let mut iso = BTreeMap::new();
        iso.insert(
            "asset-a".to_string(),
            MoneyAmount::parse("600.00", "GBP".to_string()).expect("a"),
        );
        iso.insert(
            "asset-b".to_string(),
            MoneyAmount::parse("400.00", "GBP".to_string()).expect("b"),
        );
        (statement, iso)
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(REVENUE_SHARE_NOT_RETAIL_CIS_STATEMENT.contains("NOT a retail"));
        assert!(NO_STATEMENT_ISSUED_STATEMENT.contains("no ISSUED, SENT, or DELIVERED"));
        assert!(EXACT_DISAGGREGATION_STATEMENT.contains("no division, no rounding"));
        assert!(EVERY_MISMATCH_NAMED_STATEMENT.contains("BOTH values verbatim"));
    }

    #[test]
    fn percent_of_amount_is_exact() {
        let total = MoneyAmount::parse("125.75", "GBP".to_string()).expect("total");
        let share = percent_of_amount(&total, &FixedDecimal::parse("12.5").expect("p"))
            .expect("share");
        // 125.75 x 12.5% = 15.71875 exactly (scale 2 + 1 + 2 = 5).
        assert_eq!(share.value().mantissa(), 1_571_875);
        assert_eq!(share.value().scale(), 5);
        assert_eq!(share.currency_code(), "GBP");
        // Zero percent and 100 percent are exact bounds.
        let zero = percent_of_amount(&total, &FixedDecimal::parse("0").expect("p"))
            .expect("zero share");
        assert!(zero.value().is_zero());
        let all = percent_of_amount(&total, &FixedDecimal::parse("100").expect("p"))
            .expect("full share");
        assert_eq!(all, total);
    }

    #[test]
    fn out_of_bounds_and_overflow_shares_are_refused() {
        let total = MoneyAmount::parse("10.00", "GBP".to_string()).expect("total");
        assert_eq!(
            percent_of_amount(&total, &FixedDecimal::parse("-1").expect("p"))
                .expect_err("negative refused")
                .name(),
            "BILLING_SHARE_OUT_OF_BOUNDS"
        );
        assert_eq!(
            percent_of_amount(&total, &FixedDecimal::parse("100.1").expect("p"))
                .expect_err("over-100 refused")
                .name(),
            "BILLING_SHARE_OUT_OF_BOUNDS"
        );
        // A scale overflow is refused, never rounded.
        let wide = FixedDecimal::parse("50.1234567890123456789");
        assert!(wide.is_err(), "the decimal layer refuses scale > 18");
    }

    #[test]
    fn shares_must_sum_to_exactly_100() {
        let mut shares = BTreeMap::new();
        shares.insert("asset-a".to_string(), "60".to_string());
        shares.insert("asset-b".to_string(), "41".to_string());
        assert_eq!(
            config(shares).err().expect("101% refused").name(),
            "BILLING_SHARES_DO_NOT_SUM_TO_100"
        );
        let mut thirds = BTreeMap::new();
        thirds.insert("asset-a".to_string(), "33.33".to_string());
        thirds.insert("asset-b".to_string(), "33.33".to_string());
        thirds.insert("asset-c".to_string(), "33.34".to_string());
        // 33.33 + 33.33 + 33.34 = 100.00 EXACTLY: accepted.
        config(thirds).expect("exact 100 accepted");
        let mut padded = BTreeMap::new();
        padded.insert("asset-a".to_string(), "99.99".to_string());
        padded.insert("asset-b".to_string(), "0.01".to_string());
        config(padded).expect("exact 100.00 accepted at scale 2");
    }

    #[test]
    fn disaggregation_is_exact_and_sums_back() {
        let (statement, _iso) = statement_and_iso();
        // 1000.00 GBP: 60% = 600.00 (scale 2+0+2 = 4 → 600.0000), 40% =
        // 400.0000; the sum returns to exactly 1000.00 at the common
        // scale.
        let total = MoneyAmount::parse("1000.00", "GBP".to_string()).expect("t");
        let mut shares = BTreeMap::new();
        shares.insert("asset-a".to_string(), "60".to_string());
        shares.insert("asset-b".to_string(), "40".to_string());
        let configuration = config(shares);
        let dis = PooledRevenueDisaggregation::compute(
            "dis-2".to_string(),
            "tenant-1".to_string(),
            "program-1".to_string(),
            "stmt-ref-2".to_string(),
            &configuration,
            window(),
            total,
            link(),
            1_700_000_300_000,
            "recorder".to_string(),
        )
        .expect("disaggregation");
        assert_eq!(dis.per_asset()["asset-a"].canonical_string(), "600.0000");
        assert_eq!(dis.per_asset()["asset-b"].canonical_string(), "400.0000");
        // Value equality ignores scale: 600.0000 == 600.00.
        assert_eq!(dis.per_asset()["asset-a"], statement.lines()["asset-a"]);
    }

    #[test]
    fn statements_carry_no_issued_state_and_start_draft() {
        let (statement, _iso) = statement_and_iso();
        assert_eq!(statement.state(), OwnerStatementState::Draft);
        assert_eq!(statement.state().name(), "DRAFT");
        // The vocabulary is exactly two states — no ISSUED/SENT/DELIVERED
        // exists structurally.
        let all = [OwnerStatementState::Draft, OwnerStatementState::Reconciled];
        for state in all {
            assert!(
                state.name() != "ISSUED" && state.name() != "SENT" && state.name() != "DELIVERED"
            );
        }
        assert!(!OwnerStatementState::Reconciled.can_transition_to(OwnerStatementState::Draft));
    }

    #[test]
    fn reconciliation_flags_mismatches_with_both_values_and_queue_codes() {
        let (statement, mut iso) = statement_and_iso();
        let outcome =
            StatementReconciliationOutcome::run(&statement, "iso-1".to_string(), &iso)
                .expect("outcome");
        assert_eq!(outcome.mismatch_count(), 0);
        // A mismatching ISO/DSO statement: asset-b is 400.01 instead of
        // 400.00.
        iso.insert(
            "asset-b".to_string(),
            MoneyAmount::parse("400.01", "GBP".to_string()).expect("iso b"),
        );
        let outcome =
            StatementReconciliationOutcome::run(&statement, "iso-2".to_string(), &iso)
                .expect("outcome");
        assert_eq!(outcome.mismatch_count(), 1);
        match &outcome.lines()[1].1 {
            OwnerStatementLineOutcome::AmountMismatch {
                statement_value,
                iso_dso_value,
                deviation,
            } => {
                assert_eq!(statement_value.canonical_string(), "400.0000 GBP");
                assert_eq!(iso_dso_value.canonical_string(), "400.01 GBP");
                assert_eq!(deviation.canonical_string(), "-0.0100 GBP");
            }
            other => panic!("expected an amount mismatch, got {other:?}"),
        }
        let codes = outcome.discrepancy_reason_codes();
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].1, REASON_CODE_AMOUNT_MISMATCH);
    }

    #[test]
    fn missing_and_unexpected_lines_flag_never_match() {
        let (statement, mut iso) = statement_and_iso();
        iso.remove("asset-b");
        iso.insert(
            "asset-z".to_string(),
            MoneyAmount::parse("1.00", "GBP".to_string()).expect("z"),
        );
        let outcome =
            StatementReconciliationOutcome::run(&statement, "iso-3".to_string(), &iso)
                .expect("outcome");
        assert_eq!(outcome.mismatch_count(), 2);
        let codes = outcome.discrepancy_reason_codes();
        assert!(codes.iter().any(|(_, c)| *c == REASON_CODE_MISSING_IN_STATEMENT));
        assert!(codes.iter().any(|(_, c)| *c == REASON_CODE_UNEXPECTED_IN_STATEMENT));
        // The statement cannot reconcile with unresolved mismatches.
        let (mut statement, _iso) = statement_and_iso();
        let err = statement.reconcile(&outcome).expect_err("refused");
        assert_eq!(err.name(), "BILLING_UNRESOLVED_MISMATCHES");
    }

    #[test]
    fn all_matched_outcome_reconciles_the_statement() {
        let (mut statement, iso) = statement_and_iso();
        let outcome =
            StatementReconciliationOutcome::run(&statement, "iso-4".to_string(), &iso)
                .expect("outcome");
        statement.reconcile(&outcome).expect("reconciled");
        assert_eq!(statement.state(), OwnerStatementState::Reconciled);
        // RECONCILED is terminal.
        let err = statement.reconcile(&outcome).expect_err("terminal");
        assert_eq!(err.name(), "BILLING_ILLEGAL_STATEMENT_TRANSITION");
    }

    #[test]
    fn cross_currency_runs_are_refused() {
        let (statement, mut iso) = statement_and_iso();
        iso.insert(
            "asset-a".to_string(),
            MoneyAmount::parse("600.00", "EUR".to_string()).expect("eur"),
        );
        assert_eq!(
            StatementReconciliationOutcome::run(&statement, "iso-5".to_string(), &iso)
                .expect_err("mixed currencies refused")
                .name(),
            "BILLING_MIXED_CURRENCIES"
        );
    }

    #[test]
    fn invoicing_records_are_recorded_only() {
        let record = InvoicingRecord::new(
            "inv-1".to_string(),
            "tenant-1".to_string(),
            "INV-2026-0001".to_string(),
            "stmt-1".to_string(),
            MoneyAmount::parse("1000.00", "GBP".to_string()).expect("amount"),
            link(),
            1_700_000_400_000,
            "recorder".to_string(),
        )
        .expect("record");
        assert_eq!(record.state(), InvoicingRecordState::Recorded);
        assert_eq!(record.state().name(), "RECORDED");
    }

    #[test]
    fn the_store_is_register_only_and_refuses_duplicates() {
        let mut store = BillingRecordStore::new();
        let mut shares = BTreeMap::new();
        shares.insert("asset-a".to_string(), "100".to_string());
        let configuration = config(shares);
        store.register_configuration(configuration.clone()).expect("registered");
        assert_eq!(
            store
                .register_configuration(configuration.clone())
                .expect_err("duplicate refused")
                .name(),
            "BILLING_DUPLICATE_IDENTITY"
        );
        // A superseding configuration (a NEW identity) supersedes the
        // current one; only the successor stays current.
        let mut shares2 = BTreeMap::new();
        shares2.insert("asset-a".to_string(), "70".to_string());
        shares2.insert("asset-b".to_string(), "30".to_string());
        let configuration2 = RevenueShareConfiguration::new(
            "cfg-2".to_string(),
            "tenant-1".to_string(),
            "contract-1".to_string(),
            "availability-pool".to_string(),
            shares2,
            Some("cfg-1".to_string()),
            link(),
            1_700_000_500_000,
            "recorder".to_string(),
        )
        .expect("superseding configuration");
        store.register_configuration(configuration2).expect("supersede registered");
        let current = store.current_configurations_for_tenant("tenant-1");
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].configuration_id(), "cfg-2");
        // A transition on a superseded configuration is refused.
        let mut shares3 = BTreeMap::new();
        shares3.insert("asset-a".to_string(), "100".to_string());
        let configuration3 = RevenueShareConfiguration::new(
            "cfg-3".to_string(),
            "tenant-1".to_string(),
            "contract-1".to_string(),
            "availability-pool".to_string(),
            shares3,
            Some("cfg-1".to_string()),
            link(),
            1_700_000_600_000,
            "recorder".to_string(),
        )
        .expect("superseding configuration 3");
        assert_eq!(
            store
                .register_configuration(configuration3)
                .expect_err("superseded target refused")
                .name(),
            "BILLING_RECORD_SUPERSEDED"
        );
    }
}
