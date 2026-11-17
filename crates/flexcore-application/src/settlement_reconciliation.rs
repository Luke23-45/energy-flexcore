// P16.09 — Settlement reconciliation: statement→expected comparison with
//         the dispute queue (feeding P16.12).
//
// Phase 16 plan §2 row P16.09 (reconciliation half); blocking-order row
// §3 ("P16.09 reconciliation queue → P16.12 dispute queue, P16.13
// statement reconciliation, P16.21 parser fixtures"). Sources: [S-93]
// (the MSRS/BLI statement class), [S-97] (CAISO Tariff §11), [S-107]
// (EBGL Art. 52–55).
//
// WHAT THIS IS (decided once):
//
// - **A pure comparison engine.** The reconciliation compares the
//   EXPECTED record set (what the platform computed/anticipated per
//   contract or program — supplied by the caller) against the ingested
//   STATEMENT record set (Agent A's P16.01 settlement tables hold the
//   persisted records; this module consumes the record TYPES from
//   [`crate::settlement_models`]). It produces a per-line outcome report
//   and dispute-queue entries for every mismatch — it never settles,
//   never pays, never writes back a corrected number.
// - **Every mismatch is a queue entry, never a silent adjustment** (the
//   risk-register posture R-P16-2): the engine's only output is
//   evidence. A disputed line keeps BOTH values (expected and stated)
//   verbatim; no reconciliation "fix" is ever applied in place.
// - **A tolerance is DECLARED per run, never implied**: the caller names
//   the comparison tolerance as exact decimals per line. A zero
//   tolerance is valid (exact-match reconciliation); a missing
//   tolerance vocabulary is a design refusal, not a default.
// - **Line identity is EXACT string equality** (the P8.11/P15.15
//   exact-linkage discipline): a near-miss line ID is
//   MissingInStatement/UnexpectedInStatement — it FLAGS, never matches.
//
// Control status: pure evaluation for the simulation-only 0.x line. No
// statement exists to reconcile (none has ever been received); no
// dispute exists; no settlement number is claimed.

use std::collections::BTreeMap;

use crate::fixed_decimal::FixedDecimal;
use crate::settlement_models::PjmBliLineItem;

/// The reconciliation model schema version (a change is a NEW version).
pub const SETTLEMENT_RECONCILIATION_SCHEMA_VERSION: u32 = 1;

/// The every-mismatch-is-a-queue-entry statement (pinned by tests).
pub const NO_SILENT_ADJUSTMENT_STATEMENT: &str = "Every reconciliation mismatch becomes \
     a dispute-queue entry carrying BOTH values verbatim. The engine never adjusts, \
     corrects, or settles in place: its only output is evidence for the P16.12 dispute \
     workflow. A disputed line is resolved by the recorded human process, never by a \
     silent re-computation.";

/// The exact-line-identity statement (pinned by tests).
pub const EXACT_LINE_IDENTITY_STATEMENT: &str = "Line identity is EXACT string equality \
     on the line reference (the P8.11 exact-linkage discipline). A near-miss line ID \
     resolves to MissingInStatement or UnexpectedInStatement and FLAGS — it never \
     matches approximately, and two statements are never fuzzy-joined.";

/// The expected value of one settlement line (the caller's computed
/// record). Constructed fail-closed via [`ExpectedLine::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedLine {
    /// The line reference (exact-match identity).
    pub line_ref: String,
    /// The expected amount (exact decimal; the caller's currency is the
    /// record's — a mixed-currency comparison is refused downstream).
    pub expected_amount: crate::fixed_decimal::MoneyAmount,
}

impl ExpectedLine {
    /// Validates and constructs one expected line (fail-closed).
    pub fn new(
        line_ref: String,
        expected_amount: crate::fixed_decimal::MoneyAmount,
    ) -> Result<Self, ReconciliationError> {
        if line_ref.trim().is_empty() {
            return Err(ReconciliationError::EmptyReference {
                field: "line_ref",
            });
        }
        Ok(Self {
            line_ref,
            expected_amount,
        })
    }
}

/// Why a reconciliation was refused (as opposed to a per-line mismatch,
/// which is a normal outcome). Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationError {
    /// A reference field was empty.
    EmptyReference { field: &'static str },
    /// The expected set was empty — reconciling nothing against anything
    /// is a refusal, never a clean pass (fail-closed).
    EmptyExpectedSet,
    /// The statement set was empty — a statement with no lines cannot be
    /// reconciled (it is a structural anomaly and FLAGS, but the run
    /// itself is refused; the caller opens a dispute manually).
    EmptyStatementSet,
    /// The expected and statement currencies differ for the same line —
    /// money of two currencies is never compared (the fixed-decimal rule).
    CurrencyMismatch { line_ref: String },
    /// A tolerance or amount operation overflowed the decimal mantissa.
    DecimalInvalid { reason: String },
    /// The same line reference appeared twice in one set — duplicate
    /// identity is a named refusal, never a silent merge.
    DuplicateLineRef { line_ref: String },
}

impl ReconciliationError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ReconciliationError::EmptyReference { .. } => "RECON_EMPTY_REFERENCE",
            ReconciliationError::EmptyExpectedSet => "RECON_EMPTY_EXPECTED_SET",
            ReconciliationError::EmptyStatementSet => "RECON_EMPTY_STATEMENT_SET",
            ReconciliationError::CurrencyMismatch { .. } => "RECON_CURRENCY_MISMATCH",
            ReconciliationError::DecimalInvalid { .. } => "RECON_DECIMAL_INVALID",
            ReconciliationError::DuplicateLineRef { .. } => "RECON_DUPLICATE_LINE_REF",
        }
    }
}

impl std::fmt::Display for ReconciliationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconciliationError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            ReconciliationError::EmptyExpectedSet => {
                write!(f, "the expected set is empty; reconciling nothing against \
                           anything is a refusal, never a clean pass")
            }
            ReconciliationError::EmptyStatementSet => {
                write!(f, "the statement set is empty; a statement with no lines \
                           cannot be reconciled — it FLAGS for manual handling")
            }
            ReconciliationError::CurrencyMismatch { line_ref } => write!(
                f,
                "line {line_ref} carries different currencies on its expected and \
                 stated sides; money of two currencies is never compared"
            ),
            ReconciliationError::DecimalInvalid { reason } => {
                write!(f, "a decimal operation failed: {reason}")
            }
            ReconciliationError::DuplicateLineRef { line_ref } => write!(
                f,
                "the line reference '{line_ref}' appears twice in one set; duplicate \
                 identity is refused, never silently merged"
            ),
        }
    }
}

impl std::error::Error for ReconciliationError {}

/// The per-line reconciliation outcome (exhaustive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineOutcome {
    /// The stated amount equals the expected amount exactly (or within
    /// the declared tolerance — the caller declares per run whether the
    /// tolerance is zero).
    Matched,
    /// Both sides exist for the line but the amounts differ. Both values
    /// are carried verbatim; a dispute entry is produced.
    AmountMismatch {
        /// The expected amount (verbatim).
        expected: crate::fixed_decimal::MoneyAmount,
        /// The stated amount (verbatim).
        stated: crate::fixed_decimal::MoneyAmount,
        /// The exact deviation `stated − expected` (verbatim).
        deviation: crate::fixed_decimal::MoneyAmount,
    },
    /// The line exists in the expected set but not in the statement —
    /// the statement omitted it. FLAGS (never silently dropped).
    MissingInStatement,
    /// The line exists in the statement but not in the expected set —
    /// the statement carries an unexpected charge. FLAGS.
    UnexpectedInStatement,
}

impl LineOutcome {
    /// True when the outcome requires a dispute-queue entry.
    pub fn requires_dispute(&self) -> bool {
        !matches!(self, LineOutcome::Matched)
    }
}

/// One dispute-queue entry (the P16.09 → P16.12 handoff record). The
/// entry carries BOTH values verbatim and a stable reference; it never
/// carries a resolution — that is P16.12's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeQueueEntry {
    /// The queue entry reference (caller-assigned; non-empty).
    pub queue_entry_ref: String,
    /// The line reference under dispute (verbatim).
    pub line_ref: String,
    /// The statement reference the line belongs to (verbatim).
    pub statement_ref: String,
    /// Why the line entered the queue (the outcome's reason, rendered
    /// from the outcome kind — never a free-text guess).
    pub reason_code: String,
    /// The expected value, when one exists (verbatim).
    pub expected: Option<crate::fixed_decimal::MoneyAmount>,
    /// The stated value, when one exists (verbatim).
    pub stated: Option<crate::fixed_decimal::MoneyAmount>,
}

/// The reconciliation report (per-line outcomes, deterministic order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationReport {
    /// The statement reference the run reconciled.
    pub statement_ref: String,
    /// The per-line outcomes keyed by line reference (BTreeMap —
    /// deterministic iteration order, the kernel determinism rule).
    pub outcomes: BTreeMap<String, LineOutcome>,
    /// The dispute-queue entries, in line-reference order.
    pub dispute_queue: Vec<DisputeQueueEntry>,
    /// The counts by outcome kind (derived from `outcomes`, carried for
    /// the operator view).
    pub matched_count: usize,
    pub mismatch_count: usize,
}

/// Declares the comparison tolerance for a run (recorded data, never a
/// default). The v1 reconciliation rule compares EXACTLY (zero
/// tolerance); a non-zero tolerance is DECLARED per run by the caller
/// via [`Tolerance::within`] and applied as
/// `|stated − expected| ≤ tolerance`. The tolerance carries no currency
/// — it is validated against the line's currency at comparison time by
/// refusing cross-currency amounts upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tolerance {
    /// Exact equality (the default recorded posture).
    Exact,
    /// Within the declared absolute deviation (inclusive boundary — a
    /// deviation exactly equal to the tolerance is NOT a mismatch, the
    /// P2.08 equality convention).
    Within(FixedDecimal),
}

impl Tolerance {
    /// Declares a within-tolerance rule.
    pub fn within(deviation: FixedDecimal) -> Self {
        Self::Within(deviation)
    }
}

/// Reconciles the expected line set against the statement's billing line
/// items (the PJM BLI shape is the v1 carrier; CAISO/EBGL packs map onto
/// the same outcome vocabulary with their own record types — the outcome
/// and dispute vocabulary is shared, the line carrier is per-jurisdiction,
/// ADR-011). Rules:
///
/// 1. Both sets must be non-empty (fail-closed).
/// 2. Line identity is exact string equality ([`EXACT_LINE_IDENTITY_STATEMENT`]).
/// 3. A currency difference on the same line is a run-level refusal
///    ([`ReconciliationError::CurrencyMismatch`]) — never a per-line
///    outcome.
/// 4. Every non-matched outcome produces one dispute-queue entry
///    ([`NO_SILENT_ADJUSTMENT_STATEMENT`]).
pub fn reconcile_bli_statement(
    statement_ref: &str,
    expected: &[ExpectedLine],
    stated: &[PjmBliLineItem],
    tolerance: Tolerance,
) -> Result<ReconciliationReport, ReconciliationError> {
    if statement_ref.trim().is_empty() {
        return Err(ReconciliationError::EmptyReference {
            field: "statement_ref",
        });
    }
    if expected.is_empty() {
        return Err(ReconciliationError::EmptyExpectedSet);
    }
    if stated.is_empty() {
        return Err(ReconciliationError::EmptyStatementSet);
    }
    // Duplicate-identity refusals on both sides (never silent merges).
    let mut expected_by_ref: BTreeMap<&str, &ExpectedLine> = BTreeMap::new();
    for line in expected {
        if expected_by_ref
            .insert(line.line_ref.as_str(), line)
            .is_some()
        {
            return Err(ReconciliationError::DuplicateLineRef {
                line_ref: line.line_ref.clone(),
            });
        }
    }
    let mut stated_by_ref: BTreeMap<&str, &PjmBliLineItem> = BTreeMap::new();
    for line in stated {
        if stated_by_ref.insert(line.line_ref.as_str(), line).is_some() {
            return Err(ReconciliationError::DuplicateLineRef {
                line_ref: line.line_ref.clone(),
            });
        }
    }
    let mut outcomes = BTreeMap::new();
    let mut dispute_queue = Vec::new();
    let mut matched = 0usize;
    let mut mismatched = 0usize;
    // Deterministic order: expected-set order first (BTreeMap by ref),
    // then statement-only lines (also BTreeMap by ref).
    for (line_ref, expected_line) in expected_by_ref.iter() {
        let Some(stated_line) = stated_by_ref.get(line_ref) else {
            mismatched += 1;
            outcomes.insert(
                (*line_ref).to_string(),
                LineOutcome::MissingInStatement,
            );
            dispute_queue.push(DisputeQueueEntry {
                queue_entry_ref: format!("{statement_ref}#{line_ref}#missing"),
                line_ref: (*line_ref).to_string(),
                statement_ref: statement_ref.to_string(),
                reason_code: "LINE_MISSING_IN_STATEMENT".to_string(),
                expected: Some(expected_line.expected_amount.clone()),
                stated: None,
            });
            continue;
        };
        if expected_line.expected_amount.currency_code()
            != stated_line.amount.currency_code()
        {
            return Err(ReconciliationError::CurrencyMismatch {
                line_ref: (*line_ref).to_string(),
            });
        }
        let deviation = stated_line
            .amount
            .clone()
            .checked_sub(expected_line.expected_amount.clone())
            .map_err(|e| ReconciliationError::DecimalInvalid {
                reason: e.to_string(),
            })?;
        let within = match tolerance {
            Tolerance::Exact => deviation.value().is_zero(),
            Tolerance::Within(bound) => {
                // |deviation| ≤ bound, inclusive boundary.
                let magnitude = if deviation.value().mantissa() < 0 {
                    // Negate by parts (no unary minus helper exists —
                    // rebuild the decimal from the negated mantissa).
                    FixedDecimal::from_parts(
                        -deviation.value().mantissa(),
                        deviation.value().scale(),
                    )
                    .map_err(|e| ReconciliationError::DecimalInvalid {
                        reason: e.to_string(),
                    })?
                } else {
                    deviation.value()
                };
                // An unalignable comparison (mantissa overflow) is a run
                // refusal — never silently read as within tolerance.
                match magnitude.partial_cmp(&bound) {
                    Some(std::cmp::Ordering::Greater) => false,
                    Some(_) => true,
                    None => {
                        return Err(ReconciliationError::DecimalInvalid {
                            reason: "the deviation could not be aligned against the \
                                     declared tolerance bound"
                                .to_string(),
                        })
                    }
                }
            }
        };
        if within {
            matched += 1;
            outcomes.insert((*line_ref).to_string(), LineOutcome::Matched);
        } else {
            mismatched += 1;
            outcomes.insert(
                (*line_ref).to_string(),
                LineOutcome::AmountMismatch {
                    expected: expected_line.expected_amount.clone(),
                    stated: stated_line.amount.clone(),
                    deviation: deviation.clone(),
                },
            );
            dispute_queue.push(DisputeQueueEntry {
                queue_entry_ref: format!("{statement_ref}#{line_ref}#mismatch"),
                line_ref: (*line_ref).to_string(),
                statement_ref: statement_ref.to_string(),
                reason_code: "LINE_AMOUNT_MISMATCH".to_string(),
                expected: Some(expected_line.expected_amount.clone()),
                stated: Some(stated_line.amount.clone()),
            });
        }
    }
    for (line_ref, stated_line) in stated_by_ref.iter() {
        if !expected_by_ref.contains_key(line_ref) {
            mismatched += 1;
            outcomes.insert(
                (*line_ref).to_string(),
                LineOutcome::UnexpectedInStatement,
            );
            dispute_queue.push(DisputeQueueEntry {
                queue_entry_ref: format!("{statement_ref}#{line_ref}#unexpected"),
                line_ref: (*line_ref).to_string(),
                statement_ref: statement_ref.to_string(),
                reason_code: "LINE_UNEXPECTED_IN_STATEMENT".to_string(),
                expected: None,
                stated: Some(stated_line.amount.clone()),
            });
        }
    }
    Ok(ReconciliationReport {
        statement_ref: statement_ref.to_string(),
        outcomes,
        dispute_queue,
        matched_count: matched,
        mismatch_count: mismatched,
    })
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture below is SYNTHETIC
// (no market exists); every expected value is hand-derived from the
// rules above. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed_decimal::MoneyAmount;

    fn money(text: &str) -> MoneyAmount {
        MoneyAmount::parse(text, "USD".to_string()).expect("fixture amount")
    }

    fn expected(line_ref: &str, text: &str) -> ExpectedLine {
        ExpectedLine::new(line_ref.to_string(), money(text)).expect("valid expected line")
    }

    fn stated(line_ref: &str, text: &str) -> PjmBliLineItem {
        PjmBliLineItem {
            line_ref: line_ref.to_string(),
            charge_type: "SYN_CHARGE".to_string(),
            amount: money(text),
            statement_ref: "SYN-STMT".to_string(),
        }
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(NO_SILENT_ADJUSTMENT_STATEMENT.contains("BOTH values verbatim"));
        assert!(NO_SILENT_ADJUSTMENT_STATEMENT.contains("never adjusts"));
        assert!(EXACT_LINE_IDENTITY_STATEMENT.contains("EXACT string equality"));
    }

    #[test]
    fn a_clean_statement_matches_line_by_line() {
        let expected_set = vec![
            expected("SYN-L1", "100.00"),
            expected("SYN-L2", "50.50"),
        ];
        let stated_set = vec![
            stated("SYN-L1", "100.00"),
            stated("SYN-L2", "50.50"),
        ];
        let report = reconcile_bli_statement("SYN-STMT", &expected_set, &stated_set, Tolerance::Exact)
            .expect("reconciled");
        assert_eq!(report.matched_count, 2);
        assert_eq!(report.mismatch_count, 0);
        assert!(report.dispute_queue.is_empty());
        assert!(report
            .outcomes
            .values()
            .all(|o| *o == LineOutcome::Matched));
    }

    #[test]
    fn amount_mismatches_produce_queue_entries_with_both_values() {
        let expected_set = vec![expected("SYN-L1", "100.00")];
        let stated_set = vec![stated("SYN-L1", "97.25")];
        let report = reconcile_bli_statement("SYN-STMT", &expected_set, &stated_set, Tolerance::Exact)
            .expect("reconciled");
        assert_eq!(report.mismatch_count, 1);
        let Some(LineOutcome::AmountMismatch {
            expected,
            stated,
            deviation,
        }) = report.outcomes.get("SYN-L1")
        else {
            panic!("expected an AmountMismatch outcome");
        };
        assert_eq!(expected.canonical_string(), "100.00 USD");
        assert_eq!(stated.canonical_string(), "97.25 USD");
        assert_eq!(deviation.canonical_string(), "-2.75 USD");
        assert_eq!(report.dispute_queue.len(), 1);
        let entry = &report.dispute_queue[0];
        assert_eq!(entry.queue_entry_ref, "SYN-STMT#SYN-L1#mismatch");
        assert_eq!(entry.reason_code, "LINE_AMOUNT_MISMATCH");
        assert!(entry.expected.is_some() && entry.stated.is_some());
    }

    #[test]
    fn tolerance_is_inclusive_at_the_boundary() {
        // Declared tolerance 0.05; deviation exactly 0.05 is NOT a
        // mismatch (inclusive boundary, the P2.08 convention).
        let tolerance = Tolerance::within(FixedDecimal::parse("0.05").expect("bound"));
        let expected_set = vec![expected("SYN-L1", "100.00")];
        let stated_set = vec![stated("SYN-L1", "100.05")];
        let report = reconcile_bli_statement("SYN-STMT", &expected_set, &stated_set, tolerance)
            .expect("reconciled");
        assert_eq!(report.matched_count, 1);
        // One more cent exceeds the bound: a mismatch with a queue entry.
        let stated_set = vec![stated("SYN-L1", "100.06")];
        let report = reconcile_bli_statement("SYN-STMT", &expected_set, &stated_set, tolerance)
            .expect("reconciled");
        assert_eq!(report.mismatch_count, 1);
    }

    #[test]
    fn missing_and_unexpected_lines_flag_never_match() {
        let expected_set = vec![expected("SYN-L1", "10.00")];
        let stated_set = vec![stated("SYN-L2", "10.00")];
        let report = reconcile_bli_statement("SYN-STMT", &expected_set, &stated_set, Tolerance::Exact)
            .expect("reconciled");
        assert_eq!(report.outcomes.get("SYN-L1"), Some(&LineOutcome::MissingInStatement));
        assert_eq!(
            report.outcomes.get("SYN-L2"),
            Some(&LineOutcome::UnexpectedInStatement)
        );
        assert_eq!(report.dispute_queue.len(), 2);
        let reasons: Vec<&str> = report
            .dispute_queue
            .iter()
            .map(|e| e.reason_code.as_str())
            .collect();
        assert!(reasons.contains(&"LINE_MISSING_IN_STATEMENT"));
        assert!(reasons.contains(&"LINE_UNEXPECTED_IN_STATEMENT"));
    }

    #[test]
    fn run_level_refusals_are_named() {
        let err = reconcile_bli_statement("", &[expected("L", "1")], &[stated("L", "1")], Tolerance::Exact)
            .expect_err("empty statement ref refused");
        assert_eq!(err.name(), "RECON_EMPTY_REFERENCE");
        let err = reconcile_bli_statement("S", &[], &[stated("L", "1")], Tolerance::Exact)
            .expect_err("empty expected set refused");
        assert_eq!(err.name(), "RECON_EMPTY_EXPECTED_SET");
        let err = reconcile_bli_statement("S", &[expected("L", "1")], &[], Tolerance::Exact)
            .expect_err("empty statement set refused");
        assert_eq!(err.name(), "RECON_EMPTY_STATEMENT_SET");
        // Duplicate line identity is refused, never merged.
        let err = reconcile_bli_statement(
            "S",
            &[expected("L", "1"), expected("L", "2")],
            &[stated("L", "1")],
            Tolerance::Exact,
        )
        .expect_err("duplicate expected ref refused");
        assert_eq!(err.name(), "RECON_DUPLICATE_LINE_REF");
        // Cross-currency comparison is refused at run level.
        let eur = ExpectedLine::new(
            "L".to_string(),
            MoneyAmount::parse("1.00", "EUR".to_string()).expect("eur"),
        )
        .expect("line");
        let err = reconcile_bli_statement("S", &[eur], &[stated("L", "1.00")], Tolerance::Exact)
            .expect_err("currency mismatch refused");
        assert_eq!(err.name(), "RECON_CURRENCY_MISMATCH");
    }

    #[test]
    fn every_outcome_kind_maps_to_the_dispute_rule() {
        assert!(!LineOutcome::Matched.requires_dispute());
        assert!(LineOutcome::MissingInStatement.requires_dispute());
        assert!(LineOutcome::UnexpectedInStatement.requires_dispute());
        assert!(LineOutcome::AmountMismatch {
            expected: money("1.00"),
            stated: money("2.00"),
            deviation: money("1.00"),
        }
        .requires_dispute());
    }
}
