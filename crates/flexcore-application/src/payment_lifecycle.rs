// P16.14 — Payment lifecycle transparency: the FORECAST → EARNED → PAID
//         state machine per asset/program (the Voltus payment-transparency
//         pattern [S-47]), statuses as a CLOSED vocabulary, and every
//         state transition chain-linked.
//
// Phase 16 plan §2 row P16.14; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 42_p16_14_payment_lifecycle.md`.
//
// WHAT THIS IS (decided once):
//
// - **A CLOSED vocabulary**: exactly three payment states — FORECASTED,
//   EARNED, PAID — and exactly two transitions (the frozen table). No
//   PENDING, PROCESSING, APPROVED, or "in-review" state exists: an owner
//   sees a forecast, then an earned figure, then a payment — nothing
//   between, nothing implied. The vocabulary is closed structurally
//   (the enum has three variants; there is no fourth state to reach).
// - **Every state transition is CHAIN-LINKED** (the P11 chain treatment,
//   context constraint 6): each transition record carries (a) the P11
//   audit-chain LINK for its own write and (b) the identity of the
//   PREVIOUS transition in the asset/program chain — the store refuses a
//   transition whose chain link does not match the chain's actual head.
//   The chain treatment itself stays in the P11 audit chain (consumed,
//   never redefined); the linkage here makes the payment history a
//   verifiable linear record per asset/program.
// - **Money is the fixed-decimal layer** (plan §4; hard ban 9): every
//   amount carried on a lifecycle is a [`MoneyAmount`] (exact i128
//   mantissa × 10^−scale + currency code as data). A chain carries ONE
//   currency end to end: a transition carrying a different currency
//   than the chain's established currency is refused — money of two
//   currencies is never silently mixed.
// - **A transition names its basis**: EARNED requires a non-empty
//   earned-basis reference (the settlement statement / owner-statement
//   line the earned figure came from — P16.13's records); PAID requires
//   a non-empty paid-basis reference (the payment record — the P16.13
//   invoicing record or the counterparty payment reference). A state is
//   never entered without the record that justifies it.
// - **No payment record exists** (honesty rules): no market is
//   connected, no statement has ever been received, so no lifecycle has
//   ever opened and no transition has ever run. Every fixture in the
//   tests is synthetic; the stores are empty everywhere.
//
// Determinism: BTreeMap-backed store, deterministic ordering, no
// wall-clock input (recorded timestamps are caller-supplied).
//
// Control status: records for the simulation-only 0.x line. No payment
// has been made, promised, or received anywhere.

use std::collections::BTreeMap;

use flexcore_domain::market_program_model::AuditChainLink;

use crate::fixed_decimal::MoneyAmount;
use crate::revenue_share_billing::BillingError;

/// The payment-lifecycle schema version (a change is a NEW version,
/// never a silent edit).
pub const PAYMENT_LIFECYCLE_SCHEMA_VERSION: u32 = 1;

/// The closed-vocabulary statement (pinned by tests).
pub const CLOSED_PAYMENT_VOCABULARY_STATEMENT: &str = "The payment vocabulary is CLOSED: \
     exactly FORECASTED, EARNED, and PAID, with exactly two transitions (FORECASTED → \
     EARNED, EARNED → PAID). No PENDING, PROCESSING, or review state exists between the \
     forecast and the earned figure or between the earned figure and the payment — the \
     enum has three variants and there is no fourth state to reach.";

/// The chain-linkage statement (pinned by tests).
pub const EVERY_TRANSITION_CHAIN_LINKED_STATEMENT: &str = "Every payment-state transition \
     is chain-linked: the transition record carries the P11 audit-chain LINK for its own \
     write AND the identity of the previous transition in the asset/program chain; the \
     store refuses a transition whose chain link does not match the chain's actual \
     head. The payment history per asset/program is a verifiable linear record.";

/// The no-payment statement (pinned by tests).
pub const NO_PAYMENT_RECORD_EXISTS_STATEMENT: &str = "No payment record exists anywhere: \
     no market is connected, no statement has ever been received, so no payment \
     lifecycle has ever opened and no transition has ever run. Every fixture value in \
     the tests is synthetic; no payment has been made, promised, or received.";

// --- The closed state vocabulary -------------------------------------------------

/// The closed payment-state vocabulary (exactly three states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaymentState {
    /// `FORECASTED` — the revenue forecast recorded for the asset/program
    /// (the forecast is a record, never a promise of a payment).
    Forecasted,
    /// `EARNED` — the earned figure was recorded against a named
    /// settlement/owner-statement basis.
    Earned,
    /// `PAID` — the payment was recorded against a named payment basis
    /// (terminal).
    Paid,
}

impl PaymentState {
    /// The canonical SCREAMING_SNAKE name.
    pub const fn name(self) -> &'static str {
        match self {
            PaymentState::Forecasted => "FORECASTED",
            PaymentState::Earned => "EARNED",
            PaymentState::Paid => "PAID",
        }
    }

    /// Whether the state is terminal (PAID is; the chain ends there).
    pub const fn is_terminal(self) -> bool {
        matches!(self, PaymentState::Paid)
    }
}

/// The frozen transition table: exactly these edges, no more, no fewer
/// (the P0.11 pattern). No FORECASTED → PAID shortcut exists — a payment
/// without a recorded earned basis is unrepresentable.
const PAYMENT_TRANSITIONS: &[(PaymentState, PaymentState)] = &[
    (PaymentState::Forecasted, PaymentState::Earned),
    (PaymentState::Earned, PaymentState::Paid),
];

/// The frozen-transition check: exactly the recorded edges, terminal
/// states stay terminal.
pub fn try_payment_transition(
    from: PaymentState,
    to: PaymentState,
) -> Result<PaymentState, PaymentLifecycleError> {
    if PAYMENT_TRANSITIONS
        .iter()
        .any(|(a, b)| *a == from && *b == to)
    {
        Ok(to)
    } else {
        Err(PaymentLifecycleError::InvalidTransition { from, to })
    }
}

// --- Errors ----------------------------------------------------------------------

/// Why a payment-lifecycle operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentLifecycleError {
    /// The transition is not in the frozen table (no shortcut edges,
    /// no exits from PAID).
    InvalidTransition {
        from: PaymentState,
        to: PaymentState,
    },
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
    /// The lifecycle for this (tenant, asset, program) already exists.
    LifecycleAlreadyOpen,
    /// No lifecycle is open for this (tenant, asset, program).
    LifecycleNotFound,
    /// The transition's `previous_transition_id` does not match the
    /// chain's actual head (the chain linkage is exact — a broken or
    /// fabricated chain is refused).
    ChainLinkMismatch {
        expected: Option<String>,
        given: Option<String>,
    },
    /// The EARNED transition carried no earned-basis reference.
    EarnedBasisRequired,
    /// The PAID transition carried no paid-basis reference.
    PaidBasisRequired,
    /// The transition carried a currency different from the chain's
    /// established currency (money of two currencies is never silently
    /// mixed).
    CurrencyMismatchInChain {
        chain_currency: String,
        given_currency: String,
    },
    /// A domain shared-type constructor refused (carried).
    Domain(flexcore_domain::market_program_model::MarketProgramModelError),
    /// A billing shared-type refused (carried — the fixed-decimal
    /// layer's refusals arrive this way).
    Billing(BillingError),
}

impl PaymentLifecycleError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PaymentLifecycleError::InvalidTransition { .. } => "PAYMENT_INVALID_TRANSITION",
            PaymentLifecycleError::EmptyField { .. } => "PAYMENT_EMPTY_FIELD",
            PaymentLifecycleError::FieldTooLong { .. } => "PAYMENT_FIELD_TOO_LONG",
            PaymentLifecycleError::LifecycleAlreadyOpen => "PAYMENT_LIFECYCLE_ALREADY_OPEN",
            PaymentLifecycleError::LifecycleNotFound => "PAYMENT_LIFECYCLE_NOT_FOUND",
            PaymentLifecycleError::ChainLinkMismatch { .. } => "PAYMENT_CHAIN_LINK_MISMATCH",
            PaymentLifecycleError::EarnedBasisRequired => "PAYMENT_EARNED_BASIS_REQUIRED",
            PaymentLifecycleError::PaidBasisRequired => "PAYMENT_PAID_BASIS_REQUIRED",
            PaymentLifecycleError::CurrencyMismatchInChain { .. } => {
                "PAYMENT_CURRENCY_MISMATCH_IN_CHAIN"
            }
            PaymentLifecycleError::Domain(_) => "PAYMENT_DOMAIN_ERROR",
            PaymentLifecycleError::Billing(_) => "PAYMENT_BILLING_ERROR",
        }
    }
}

impl std::fmt::Display for PaymentLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentLifecycleError::InvalidTransition { from, to } => write!(
                f,
                "the payment transition {} → {} is not in the frozen table; there are no \
                 shortcut edges (the P0.11 pattern) and PAID is terminal",
                from.name(),
                to.name()
            ),
            PaymentLifecycleError::EmptyField { field } => {
                write!(f, "the payment-lifecycle field '{field}' must be non-empty")
            }
            PaymentLifecycleError::FieldTooLong { field, len, max } => write!(
                f,
                "the payment-lifecycle field '{field}' is {len} characters; the bound is {max}"
            ),
            PaymentLifecycleError::LifecycleAlreadyOpen => write!(
                f,
                "a payment lifecycle is already open for this asset/program; there is \
                 exactly one lifecycle per (tenant, asset, program)"
            ),
            PaymentLifecycleError::LifecycleNotFound => write!(
                f,
                "no payment lifecycle is open for this asset/program; open one at \
                 FORECASTED first"
            ),
            PaymentLifecycleError::ChainLinkMismatch { expected, given } => write!(
                f,
                "the transition's chain link ({given:?}) does not match the chain's \
                 actual head ({expected:?}); the payment chain is linear and exact — a \
                 broken or fabricated chain is refused"
            ),
            PaymentLifecycleError::EarnedBasisRequired => write!(
                f,
                "the EARNED transition carries no earned-basis reference; a state is \
                 never entered without the record that justifies it"
            ),
            PaymentLifecycleError::PaidBasisRequired => write!(
                f,
                "the PAID transition carries no paid-basis reference; a state is never \
                 entered without the record that justifies it"
            ),
            PaymentLifecycleError::CurrencyMismatchInChain {
                chain_currency,
                given_currency,
            } => write!(
                f,
                "the transition carries {given_currency} but the payment chain was \
                 established in {chain_currency}; money of two currencies is never \
                 silently mixed"
            ),
            PaymentLifecycleError::Domain(err) => write!(f, "domain shared-type refused: {err}"),
            PaymentLifecycleError::Billing(err) => write!(f, "billing shared-type refused: {err}"),
        }
    }
}

impl std::error::Error for PaymentLifecycleError {}

// --- The transition record ---------------------------------------------------------

/// One recorded payment-state transition (immutable; the chain grows by
/// appending). The record carries: the frozen edge, the basis reference
/// for the destination state, the optional amount as recorded (exact
/// decimal + currency as data), the audit-chain LINK for its own write,
/// and the identity of the PREVIOUS transition in the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentTransitionRecord {
    transition_id: String,
    tenant_id: String,
    asset_id: String,
    program_id: String,
    from_state: PaymentState,
    to_state: PaymentState,
    /// The record the destination state is justified by (the
    /// settlement/owner-statement line for EARNED; the payment record
    /// for PAID). Required for both frozen edges.
    basis_ref: String,
    /// The amount as recorded (optional — recorded data; exact decimal
    /// + currency as data; never a float).
    amount: Option<MoneyAmount>,
    /// The P11 audit-chain LINK for this transition's own write.
    audit_chain_link: AuditChainLink,
    /// The identity of the previous transition in the chain (None for
    /// the chain's first transition only).
    previous_transition_id: Option<String>,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl PaymentTransitionRecord {
    /// Builds one transition record (validated; the frozen-edge and
    /// basis checks run at apply time where the chain is known).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        transition_id: String,
        tenant_id: String,
        asset_id: String,
        program_id: String,
        from_state: PaymentState,
        to_state: PaymentState,
        basis_ref: String,
        amount: Option<MoneyAmount>,
        audit_chain_link: AuditChainLink,
        previous_transition_id: Option<String>,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, PaymentLifecycleError> {
        Self::validate_text("transition_id", &transition_id, 64)?;
        Self::validate_text("tenant_id", &tenant_id, 64)?;
        Self::validate_text("asset_id", &asset_id, 64)?;
        Self::validate_text("program_id", &program_id, 64)?;
        Self::validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(PaymentLifecycleError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            transition_id,
            tenant_id,
            asset_id,
            program_id,
            from_state,
            to_state,
            basis_ref,
            amount,
            audit_chain_link,
            previous_transition_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), PaymentLifecycleError> {
        if value.trim().is_empty() {
            return Err(PaymentLifecycleError::EmptyField { field });
        }
        if value.len() > max {
            return Err(PaymentLifecycleError::FieldTooLong {
                field,
                len: value.len(),
                max,
            });
        }
        Ok(())
    }

    pub fn transition_id(&self) -> &str {
        &self.transition_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn from_state(&self) -> PaymentState {
        self.from_state
    }

    pub fn to_state(&self) -> PaymentState {
        self.to_state
    }

    pub fn basis_ref(&self) -> &str {
        &self.basis_ref
    }

    pub fn amount(&self) -> Option<&MoneyAmount> {
        self.amount.as_ref()
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn previous_transition_id(&self) -> Option<&str> {
        self.previous_transition_id.as_deref()
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- The lifecycle record -----------------------------------------------------------

/// One open payment lifecycle (per tenant/asset/program): opened at
/// FORECASTED with the forecast amount as recorded data. There is
/// exactly ONE lifecycle per (tenant, asset, program).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentLifecycle {
    tenant_id: String,
    asset_id: String,
    program_id: String,
    state: PaymentState,
    /// The forecast amount as recorded (exact decimal + currency as
    /// data; the chain's currency is established here).
    forecast_amount: MoneyAmount,
    /// The audit-chain LINK of the lifecycle's opening write.
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl PaymentLifecycle {
    fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), PaymentLifecycleError> {
        if value.trim().is_empty() {
            return Err(PaymentLifecycleError::EmptyField { field });
        }
        if value.len() > max {
            return Err(PaymentLifecycleError::FieldTooLong {
                field,
                len: value.len(),
                max,
            });
        }
        Ok(())
    }

    fn new(
        tenant_id: String,
        asset_id: String,
        program_id: String,
        forecast_amount: MoneyAmount,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, PaymentLifecycleError> {
        Self::validate_text("tenant_id", &tenant_id, 64)?;
        Self::validate_text("asset_id", &asset_id, 64)?;
        Self::validate_text("program_id", &program_id, 64)?;
        Self::validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(PaymentLifecycleError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            tenant_id,
            asset_id,
            program_id,
            state: PaymentState::Forecasted,
            forecast_amount,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- The lifecycle store --------------------------------------------------------------

/// The BTreeMap-backed payment-lifecycle store: one lifecycle per
/// (tenant, asset, program), an append-only transition chain per
/// lifecycle, and the closed-vocabulary / chain-linkage / basis /
/// currency rules enforced at apply time.
#[derive(Debug, Default)]
pub struct PaymentLifecycleStore {
    lifecycles: BTreeMap<(String, String, String), PaymentLifecycle>,
    /// The transition chains, in append order (the chain linkage
    /// makes the order verifiable, not just trusted).
    chains: BTreeMap<(String, String, String), Vec<PaymentTransitionRecord>>,
}

impl PaymentLifecycleStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens one lifecycle at FORECASTED (exactly one per
    /// tenant/asset/program; the forecast amount establishes the
    /// chain's currency).
    pub fn open_lifecycle(
        &mut self,
        tenant_id: String,
        asset_id: String,
        program_id: String,
        forecast_amount: MoneyAmount,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<(), PaymentLifecycleError> {
        let key = (tenant_id.clone(), asset_id.clone(), program_id.clone());
        if self.lifecycles.contains_key(&key) {
            return Err(PaymentLifecycleError::LifecycleAlreadyOpen);
        }
        let lifecycle = PaymentLifecycle::new(
            tenant_id,
            asset_id,
            program_id,
            forecast_amount,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        )?;
        self.lifecycles.insert(key, lifecycle);
        Ok(())
    }

    /// The current state for one asset/program.
    pub fn state_of(
        &self,
        tenant_id: &str,
        asset_id: &str,
        program_id: &str,
    ) -> Option<PaymentState> {
        self.lifecycles
            .get(&(
                tenant_id.to_string(),
                asset_id.to_string(),
                program_id.to_string(),
            ))
            .map(|lifecycle| lifecycle.state)
    }

    /// The forecast amount as recorded for one asset/program.
    pub fn forecast_amount_of(
        &self,
        tenant_id: &str,
        asset_id: &str,
        program_id: &str,
    ) -> Option<&MoneyAmount> {
        self.lifecycles
            .get(&(
                tenant_id.to_string(),
                asset_id.to_string(),
                program_id.to_string(),
            ))
            .map(|lifecycle| &lifecycle.forecast_amount)
    }

    /// The recorded transition chain for one asset/program (append
    /// order).
    pub fn chain_of(
        &self,
        tenant_id: &str,
        asset_id: &str,
        program_id: &str,
    ) -> Option<&[PaymentTransitionRecord]> {
        self.chains
            .get(&(
                tenant_id.to_string(),
                asset_id.to_string(),
                program_id.to_string(),
            ))
            .map(|chain| chain.as_slice())
    }

    /// Applies one transition: the frozen-edge check, the basis
    /// requirement for the destination state, the exact chain-linkage
    /// check, and the one-currency-per-chain check. The lifecycle's
    /// state advances ONLY through this method.
    pub fn apply_transition(
        &mut self,
        record: PaymentTransitionRecord,
    ) -> Result<(), PaymentLifecycleError> {
        // The lifecycle must exist and be at the transition's source.
        let key = (
            record.tenant_id().to_string(),
            record.asset_id().to_string(),
            record.program_id().to_string(),
        );
        let lifecycle = self
            .lifecycles
            .get(&key)
            .ok_or(PaymentLifecycleError::LifecycleNotFound)?;
        if lifecycle.state != record.from_state() {
            return Err(PaymentLifecycleError::InvalidTransition {
                from: lifecycle.state,
                to: record.to_state(),
            });
        }
        // The frozen edge.
        try_payment_transition(record.from_state(), record.to_state())?;
        // The destination state names its basis.
        if record.basis_ref().trim().is_empty() {
            return Err(match record.to_state() {
                PaymentState::Earned => PaymentLifecycleError::EarnedBasisRequired,
                PaymentState::Paid => PaymentLifecycleError::PaidBasisRequired,
                PaymentState::Forecasted => PaymentLifecycleError::EmptyField {
                    field: "basis_ref",
                },
            });
        }
        // The exact chain linkage.
        let chain = self.chains.entry(key.clone()).or_default();
        let expected: Option<String> = chain.last().map(|t| t.transition_id().to_string());
        if record.previous_transition_id() != expected.as_deref() {
            return Err(PaymentLifecycleError::ChainLinkMismatch {
                expected,
                given: record.previous_transition_id().map(|s| s.to_string()),
            });
        }
        // One currency per chain.
        if let Some(amount) = record.amount() {
            if amount.currency_code() != lifecycle.forecast_amount.currency_code() {
                return Err(PaymentLifecycleError::CurrencyMismatchInChain {
                    chain_currency: lifecycle.forecast_amount.currency_code().to_string(),
                    given_currency: amount.currency_code().to_string(),
                });
            }
        }
        // Append; advance the state.
        let to_state = record.to_state();
        chain.push(record);
        let lifecycle = self
            .lifecycles
            .get_mut(&key)
            .expect("checked above");
        lifecycle.state = to_state;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the closed vocabulary and chain rules above.
// Every fixture value is synthetic and carries no market meaning.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const AUDIT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn link() -> AuditChainLink {
        AuditChainLink::new(AUDIT.to_string()).expect("valid link")
    }

    fn open(store: &mut PaymentLifecycleStore) {
        let forecast = MoneyAmount::parse("250.00", "GBP".to_string()).expect("forecast");
        store
            .open_lifecycle(
                "tenant-1".to_string(),
                "asset-1".to_string(),
                "program-1".to_string(),
                forecast,
                link(),
                1_700_000_000_000,
                "recorder".to_string(),
            )
            .expect("opened");
    }

    fn transition(
        id: &str,
        from: PaymentState,
        to: PaymentState,
        basis: &str,
        previous: Option<&str>,
        amount_text: Option<&str>,
    ) -> PaymentTransitionRecord {
        let amount = amount_text.map(|t| MoneyAmount::parse(t, "GBP".to_string()).expect("amt"));
        PaymentTransitionRecord::new(
            id.to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "program-1".to_string(),
            from,
            to,
            basis.to_string(),
            amount,
            link(),
            previous.map(|s| s.to_string()),
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("valid transition record")
    }

    #[test]
    fn statements_pin_the_recorded_boundaries() {
        assert!(CLOSED_PAYMENT_VOCABULARY_STATEMENT.contains("CLOSED"));
        assert!(CLOSED_PAYMENT_VOCABULARY_STATEMENT.contains("exactly FORECASTED, EARNED, and PAID"));
        assert!(EVERY_TRANSITION_CHAIN_LINKED_STATEMENT.contains("chain-linked"));
        assert!(NO_PAYMENT_RECORD_EXISTS_STATEMENT.contains("No payment record exists"));
    }

    #[test]
    fn the_vocabulary_is_exactly_three_states() {
        assert_eq!(PaymentState::Forecasted.name(), "FORECASTED");
        assert_eq!(PaymentState::Earned.name(), "EARNED");
        assert_eq!(PaymentState::Paid.name(), "PAID");
        assert!(!PaymentState::Forecasted.is_terminal());
        assert!(!PaymentState::Earned.is_terminal());
        assert!(PaymentState::Paid.is_terminal());
        // The frozen table: exactly two edges.
        try_payment_transition(PaymentState::Forecasted, PaymentState::Earned)
            .expect("edge 1");
        try_payment_transition(PaymentState::Earned, PaymentState::Paid).expect("edge 2");
    }

    #[test]
    fn shortcut_edges_and_terminal_exits_are_refused() {
        // No FORECASTED → PAID shortcut: a payment without a recorded
        // earned basis is unrepresentable.
        assert_eq!(
            try_payment_transition(PaymentState::Forecasted, PaymentState::Paid)
                .expect_err("shortcut refused")
                .name(),
            "PAYMENT_INVALID_TRANSITION"
        );
        // No self-transitions, no backwards transitions.
        assert!(try_payment_transition(PaymentState::Earned, PaymentState::Forecasted).is_err());
        assert!(try_payment_transition(PaymentState::Paid, PaymentState::Earned).is_err());
        assert!(try_payment_transition(PaymentState::Paid, PaymentState::Paid).is_err());
    }

    #[test]
    fn the_lifecycle_opens_at_forecasted_with_the_forecast_amount() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store);
        assert_eq!(
            store.state_of("tenant-1", "asset-1", "program-1"),
            Some(PaymentState::Forecasted)
        );
        let forecast = store
            .forecast_amount_of("tenant-1", "asset-1", "program-1")
            .expect("forecast");
        assert_eq!(forecast.canonical_string(), "250.00 GBP");
        // Exactly one lifecycle per identity.
        assert_eq!(
            store
                .open_lifecycle(
                    "tenant-1".to_string(),
                    "asset-1".to_string(),
                    "program-1".to_string(),
                    MoneyAmount::parse("1.00", "GBP".to_string()).expect("x"),
                    link(),
                    1_700_000_200_000,
                    "recorder".to_string(),
                )
                .expect_err("already open")
                .name(),
            "PAYMENT_LIFECYCLE_ALREADY_OPEN"
        );
    }

    #[test]
    fn the_full_chain_runs_forecasted_earned_paid() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store);
        store
            .apply_transition(transition(
                "t-1",
                PaymentState::Forecasted,
                PaymentState::Earned,
                "owner-statement:stmt-1:asset-1",
                None,
                Some("150.00"),
            ))
            .expect("earned");
        assert_eq!(
            store.state_of("tenant-1", "asset-1", "program-1"),
            Some(PaymentState::Earned)
        );
        store
            .apply_transition(transition(
                "t-2",
                PaymentState::Earned,
                PaymentState::Paid,
                "payment-record:inv-2026-0001",
                Some("t-1"),
                Some("150.00"),
            ))
            .expect("paid");
        assert_eq!(
            store.state_of("tenant-1", "asset-1", "program-1"),
            Some(PaymentState::Paid)
        );
        let chain = store
            .chain_of("tenant-1", "asset-1", "program-1")
            .expect("chain");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].previous_transition_id(), None);
        assert_eq!(chain[1].previous_transition_id(), Some("t-1"));
        assert_eq!(chain[1].from_state(), PaymentState::Earned);
    }

    #[test]
    fn a_broken_or_fabricated_chain_link_is_refused() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store);
        // A first transition must carry NO previous link.
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-1",
                    PaymentState::Forecasted,
                    PaymentState::Earned,
                    "owner-statement:stmt-1:asset-1",
                    Some("fabricated-head"),
                    Some("150.00"),
                ))
                .expect_err("fabricated head refused")
                .name(),
            "PAYMENT_CHAIN_LINK_MISMATCH"
        );
        store
            .apply_transition(transition(
                "t-1",
                PaymentState::Forecasted,
                PaymentState::Earned,
                "owner-statement:stmt-1:asset-1",
                None,
                Some("150.00"),
            ))
            .expect("earned");
        // A second transition must name the ACTUAL head.
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-3",
                    PaymentState::Earned,
                    PaymentState::Paid,
                    "payment-record:inv-2026-0001",
                    Some("t-wrong"),
                    Some("150.00"),
                ))
                .expect_err("wrong head refused")
                .name(),
            "PAYMENT_CHAIN_LINK_MISMATCH"
        );
    }

    #[test]
    fn transitions_without_their_basis_are_refused() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store);
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-1",
                    PaymentState::Forecasted,
                    PaymentState::Earned,
                    "",
                    None,
                    Some("150.00"),
                ))
                .expect_err("no earned basis")
                .name(),
            "PAYMENT_EARNED_BASIS_REQUIRED"
        );
        store
            .apply_transition(transition(
                "t-1",
                PaymentState::Forecasted,
                PaymentState::Earned,
                "owner-statement:stmt-1:asset-1",
                None,
                Some("150.00"),
            ))
            .expect("earned");
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-2",
                    PaymentState::Earned,
                    PaymentState::Paid,
                    "",
                    Some("t-1"),
                    Some("150.00"),
                ))
                .expect_err("no paid basis")
                .name(),
            "PAYMENT_PAID_BASIS_REQUIRED"
        );
    }

    #[test]
    fn mixed_currencies_in_a_chain_are_refused() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store); // GBP chain
        let eur_transition = PaymentTransitionRecord::new(
            "t-1".to_string(),
            "tenant-1".to_string(),
            "asset-1".to_string(),
            "program-1".to_string(),
            PaymentState::Forecasted,
            PaymentState::Earned,
            "owner-statement:stmt-1:asset-1".to_string(),
            Some(MoneyAmount::parse("150.00", "EUR".to_string()).expect("eur")),
            link(),
            None,
            1_700_000_100_000,
            "recorder".to_string(),
        )
        .expect("record");
        assert_eq!(
            store
                .apply_transition(eur_transition)
                .expect_err("mixed currency refused")
                .name(),
            "PAYMENT_CURRENCY_MISMATCH_IN_CHAIN"
        );
    }

    #[test]
    fn paid_is_terminal_in_the_store() {
        let mut store = PaymentLifecycleStore::new();
        open(&mut store);
        store
            .apply_transition(transition(
                "t-1",
                PaymentState::Forecasted,
                PaymentState::Earned,
                "owner-statement:stmt-1:asset-1",
                None,
                Some("150.00"),
            ))
            .expect("earned");
        store
            .apply_transition(transition(
                "t-2",
                PaymentState::Earned,
                PaymentState::Paid,
                "payment-record:inv-2026-0001",
                Some("t-1"),
                Some("150.00"),
            ))
            .expect("paid");
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-3",
                    PaymentState::Paid,
                    PaymentState::Paid,
                    "anything",
                    Some("t-2"),
                    None,
                ))
                .expect_err("PAID is terminal")
                .name(),
            "PAYMENT_INVALID_TRANSITION"
        );
        // The chain holds exactly the two real transitions.
        let chain = store
            .chain_of("tenant-1", "asset-1", "program-1")
            .expect("chain");
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn no_lifecycle_no_transition() {
        let mut store = PaymentLifecycleStore::new();
        assert_eq!(
            store
                .apply_transition(transition(
                    "t-1",
                    PaymentState::Forecasted,
                    PaymentState::Earned,
                    "owner-statement:stmt-1:asset-1",
                    None,
                    Some("150.00"),
                ))
                .expect_err("nothing open")
                .name(),
            "PAYMENT_LIFECYCLE_NOT_FOUND"
        );
        assert_eq!(store.state_of("tenant-1", "asset-1", "program-1"), None);
    }
}
